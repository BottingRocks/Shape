use super::*;
use oxc_semantic::SymbolId;

use crate::naming::support::vector_state::*;

// Only a plain var/let/const declaration creates an ordinary data binding;
// `using`/`await using` resource declarations are refused here even though the
// shared permissive helper would accept any variable declaration.
fn plain_local(model: &Model<'_, '_>, declarator: &VariableDeclarator<'_>) -> Option<SymbolId> {
    let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
        return None;
    };
    let symbol = identifier.symbol_id.get()?;
    if model.scoping.symbol_redeclarations(symbol).is_empty()
        && matches!(
            model
                .nodes
                .parent_kind(model.scoping.symbol_declaration(symbol)),
            AstKind::VariableDeclaration(declaration)
                if matches!(
                    declaration.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                )
        )
        && !matches!(
            model.nodes.parent_kind(
                model
                    .nodes
                    .parent_id(model.scoping.symbol_declaration(symbol))
            ),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
    {
        Some(symbol)
    } else {
        None
    }
}

/// A captured data slot is allowed to be mutable, but it must still be a
/// resolved plain variable.  This is deliberately not used for callable or
/// host-API proof.
fn captured_data(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
) -> Option<SymbolId> {
    captured_symbol(model, site, symbol(model, expression)?)
}

fn captured_symbol(
    model: &Model<'_, '_>,
    site: FunctionSite,
    symbol: SymbolId,
) -> Option<SymbolId> {
    if site
        .span
        .contains_inclusive(model.scoping.symbol_span(symbol))
        || model.scoping.symbol_span(symbol).end >= site.span.start
        || !model.scoping.symbol_redeclarations(symbol).is_empty()
    {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(symbol);
    let AstKind::VariableDeclarator(declarator) = model.nodes.kind(declaration) else {
        return None;
    };
    plain_local(model, declarator).filter(|_| model.formal_symbol_slot(symbol).is_none())
}

fn emit_visibility_change_state(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let parameters = plain_function_parameters(model, function, 1)?;
    let [input] = parameters.as_slice() else {
        return None;
    };
    let [Statement::TryStatement(try_)] = body.statements.as_slice() else {
        return None;
    };
    if try_.finalizer.is_some() {
        return None;
    }
    let [increment, receiver, method, emit, flag] = try_.block.body.as_slice() else {
        return None;
    };
    let counter = match increment {
        Statement::ExpressionStatement(statement) => match &statement.expression {
            Expression::UpdateExpression(update) if update.operator.as_str() == "++" => {
                let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) =
                    &update.argument
                else {
                    return None;
                };
                crate::analysis::identifier_symbol(identifier, model.scoping)?
            }
            _ => return None,
        },
        _ => return None,
    };
    let counter = captured_symbol(model, site, counter)?;
    let receiver = one_variable(receiver)?;
    let receiver_symbol = plain_local(model, receiver)?;
    if model.binding_writes(receiver_symbol) != 0 {
        return None;
    }
    let receiver_capture = captured_data(model, site, receiver.init.as_ref()?)?;
    let method = one_variable(method)?;
    let method_symbol = plain_local(model, method)?;
    if model.binding_writes(method_symbol) != 0
        || !same_member(model, method.init.as_ref()?, receiver_capture, "put")
    {
        return None;
    }
    let emit = match emit {
        Statement::ExpressionStatement(statement) => call(&statement.expression)?,
        _ => return None,
    };
    let call_receiver = static_member(&emit.callee, "call")?;
    if symbol(model, call_receiver) != Some(method_symbol)
        || emit.arguments.len() != 2
        || symbol(model, call_arguments(emit).first()?) != Some(receiver_symbol)
    {
        return None;
    }
    let Argument::ObjectExpression(record) = &emit.arguments[1] else {
        return None;
    };
    if record.properties.len() != 2 {
        return None;
    }
    let mut visible = None;
    let mut timestamp = None;
    for property in &record.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        if property.computed || property.method || property.kind != PropertyKind::Init {
            return None;
        }
        match property_key_name(&property.key)?.as_str() {
            "visible" => visible = Some(&property.value),
            "timestamp" => timestamp = Some(&property.value),
            _ => return None,
        }
    }
    let visible = visible?;
    let Expression::BinaryExpression(visible) = visible else {
        return None;
    };
    if visible.operator.as_str() != "==="
        || !matches!(&visible.right, Expression::StringLiteral(value) if value.value == "visible")
        || !matches!(
            &visible.left,
            Expression::StaticMemberExpression(state)
                if !state.optional
                    && state.property.name == "visibilityState"
                    && matches!(&state.object, Expression::StaticMemberExpression(target)
                        if !target.optional
                            && target.property.name == "target"
                            && symbol(model, &target.object) == Some(*input))
        )
    {
        return None;
    }
    let timestamp = call(timestamp?)?;
    if !timestamp.arguments.is_empty() {
        return None;
    }
    let Expression::Identifier(_) = &timestamp.callee else {
        return None;
    };
    let clock = symbol(model, &timestamp.callee)?;
    if site
        .span
        .contains_inclusive(model.scoping.symbol_span(clock))
        || !model.scoping.symbol_redeclarations(clock).is_empty()
    {
        return None;
    }
    // Only an independently ordered callable is accepted as the clock origin.
    // A mere initialized/unwritten capture (for example `let clock = 0`) or an
    // unknown member/factory producer is refused rather than assumed native.
    let clock_site = crate::naming::resolve::ordered_callable(model, &timestamp.callee, 0)?;
    if clock_site.span == site.span {
        return None;
    }
    let Statement::ExpressionStatement(flag) = flag else {
        return None;
    };
    let Expression::AssignmentExpression(flag) = &flag.expression else {
        return None;
    };
    if flag.operator.as_str() != "="
        || !matches!(&flag.left, AssignmentTarget::AssignmentTargetIdentifier(identifier)
            if crate::analysis::identifier_symbol(identifier, model.scoping)
                .and_then(|symbol| captured_symbol(model, site, symbol))
                .is_some())
        || !matches!(&flag.right, Expression::BooleanLiteral(value) if value.value)
    {
        return None;
    }
    let flag_symbol = match &flag.left {
        AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            crate::analysis::identifier_symbol(identifier, model.scoping)?
        }
        _ => return None,
    };
    if flag_symbol == counter {
        return None;
    }
    let handler = try_.handler.as_ref()?;
    let error = plain_parameter(model, &handler.param.as_ref()?.pattern)?;
    let [Statement::ExpressionStatement(report)] = handler.body.body.as_slice() else {
        return None;
    };
    let report = call(&report.expression)?;
    let report_receiver = static_member(&report.callee, "error")?;
    let report_owner = captured_data(model, site, report_receiver)?;
    if report.arguments.len() != 1
        || symbol(model, call_arguments(report).first()?) != Some(error)
        || symbol(model, report_receiver) != Some(report_owner)
    {
        return None;
    }
    Some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return Vec::new();
    };
    let Some(body) = function.body.as_ref() else {
        return Vec::new();
    };
    if function.r#async || function.generator || !body.directives.is_empty() {
        return Vec::new();
    }
    let mut claims = Vec::new();
    if emit_visibility_change_state(model, site, function, body).is_some() {
        claims.push(proposal(
            "recordVisibilityChangeSample",
            "shared_anchor_visibility_publish",
            "sole try increments a captured counter, forwards {visible: input.target.visibilityState === \"visible\", timestamp: orderedCapturedCallable()} to a captured stream method through receiver-preserving .call, sets a captured flag and reports the caught binding to a captured error sink; all callee and data identities plain, ordered and captured",
            f,
        ));
    }
    claims
}
