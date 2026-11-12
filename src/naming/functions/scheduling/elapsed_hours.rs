//! Bounded owned-body proof for a captured record field written with elapsed
//! hours.
//!
//! The complete owned body must be exactly one expression statement:
//!
//! ```text
//! function f() {
//!   record.field = Number((Number(new Ctor()) - start) / 1000 / 60 / 60);
//! }
//! ```
//!
//! `record`, `Ctor` and `start` are plain variable bindings declared outside the
//! owned function. `Number` is the unshadowed global (or a strict single-origin
//! alias of it). The constructor and start captures are bounded to exactly one
//! source origin, while the record stays DATA: the rule proves a member write
//! and never invents callability for it.
use super::*;
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;

fn symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(expression, model.scoping)
}

/// A plain captured local: declared outside the owned function as a variable
/// declarator. Parameters, catch bindings and function declarations never
/// establish a captured record/value role.
fn captured(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
) -> Option<SymbolId> {
    let captured = symbol(model, expression)?;
    if site
        .span
        .contains_inclusive(model.scoping.symbol_span(captured))
        || !model.scoping.symbol_redeclarations(captured).is_empty()
    {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(captured);
    let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&variable.id, BindingPattern::BindingIdentifier(id)
            if id.symbol_id.get() == Some(captured))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(_)
        )
        || matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
    {
        return None;
    }
    Some(captured)
}

/// A captured local with exactly one source origin. The origin is outside the
/// owned function. Source order is not required: recovered setup often assigns
/// the budget, constructor and baseline after the nested function declaration,
/// so uniqueness bounds the value instead. This claims no native, sentinel or
/// callable identity.
fn stable_capture(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
) -> Option<SymbolId> {
    let captured = captured(model, site, expression)?;
    let declaration = model.scoping.symbol_declaration(captured);
    let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else {
        return None;
    };
    let writes = model.binding_writes(captured);
    if !(writes == 0 && variable.init.is_some() || writes == 1 && variable.init.is_none()) {
        return None;
    }
    let value = model.symbol_value(captured)?;
    if site.span.contains_inclusive(value.span()) {
        return None;
    }
    Some(captured)
}

/// Resolve a plain identifier through single-origin variable aliases. An
/// identifier with no symbol is the unshadowed global root; any scoped name must
/// be a plain, earlier, immutable `var`/`let`/`const` of the same source value.
fn alias_value<'a>(
    model: &Model<'_, 'a>,
    expression: &'a Expression<'a>,
    depth: usize,
) -> Option<&'a Expression<'a>> {
    if depth > 8 {
        return None;
    }
    let Expression::Identifier(_) = expression else {
        return Some(expression);
    };
    let Some(captured) = symbol(model, expression) else {
        return Some(expression);
    };
    if !model.scoping.symbol_redeclarations(captured).is_empty() {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(captured);
    let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&variable.id, BindingPattern::BindingIdentifier(id)
            if id.symbol_id.get() == Some(captured))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(declaration)
                if matches!(
                    declaration.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                )
        )
        || matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
        || model.binding_writes(captured) != 0
        || variable.init.is_none()
    {
        return None;
    }
    let value = model.symbol_value(captured)?;
    (value.span().end <= expression.span().start)
        .then(|| alias_value(model, value, depth + 1))
        .flatten()
}

/// The expression resolves to the unshadowed global root `Number`.
fn is_global_number(model: &Model<'_, '_>, expression: &Expression<'_>) -> bool {
    match alias_value(model, expression, 0) {
        Some(Expression::Identifier(identifier)) => {
            crate::analysis::identifier_symbol(identifier, model.scoping).is_none()
                && identifier.name == "Number"
        }
        _ => false,
    }
}

fn numeric(expression: &Expression<'_>, value: f64) -> bool {
    matches!(expression, Expression::NumericLiteral(number) if number.value == value)
}

fn division<'a>(
    expression: &'a Expression<'a>,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::BinaryExpression(binary) = expression else {
        return None;
    };
    (binary.operator.as_str() == "/").then_some((&binary.left, &binary.right))
}

/// `Number(argument)` with the exact unshadowed global `Number`.
fn number_argument<'a>(
    model: &Model<'_, 'a>,
    expression: &'a Expression<'a>,
) -> Option<&'a Expression<'a>> {
    let Expression::CallExpression(call) = expression else {
        return None;
    };
    if call.optional || call.arguments.len() != 1 || !is_global_number(model, &call.callee) {
        return None;
    }
    call_arguments(call).into_iter().next()
}

/// `Number(new Ctor())` where `Ctor` is a bounded captured constructor value.
fn constructed_capture(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
) -> Option<SymbolId> {
    let Expression::NewExpression(construction) = number_argument(model, expression)? else {
        return None;
    };
    if !construction.arguments.is_empty() {
        return None;
    }
    stable_capture(model, site, &construction.callee)
}

fn elapsed_hours(model: &Model<'_, '_>, site: FunctionSite, body: &FunctionBody<'_>) -> Option<()> {
    let [Statement::ExpressionStatement(statement)] = body.statements.as_slice() else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = &statement.expression else {
        return None;
    };
    if assignment.operator.as_str() != "=" {
        return None;
    }
    let AssignmentTarget::StaticMemberExpression(target) = &assignment.left else {
        return None;
    };
    if target.optional {
        return None;
    }
    captured(model, site, &target.object)?;

    let (after_milliseconds, hours) = division(number_argument(model, &assignment.right)?)?;
    let (after_minutes, minutes) = division(after_milliseconds)?;
    let (delta, milliseconds) = division(after_minutes)?;
    if !numeric(hours, 60.0) || !numeric(minutes, 60.0) || !numeric(milliseconds, 1000.0) {
        return None;
    }
    let Expression::BinaryExpression(subtraction) = delta else {
        return None;
    };
    if subtraction.operator.as_str() != "-" {
        return None;
    }
    constructed_capture(model, site, &subtraction.left)?;
    stable_capture(model, site, &subtraction.right)?;
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
    if function.r#async
        || function.generator
        || !body.directives.is_empty()
        || !function.params.items.is_empty()
        || function.params.rest.is_some()
    {
        return Vec::new();
    }
    elapsed_hours(model, site, body)
        .is_some()
        .then(|| {
            proposal(
                "updateElapsedHours",
                "shared_elapsed_hours_state_write",
                "sole statement assigns one captured record's static field Number((Number(new bounded captured constructor()) - bounded captured baseline) / 1000 / 60 / 60); unshadowed global Number, exactly one subtraction and three left-associated divisions by 1000/60/60, all captures declared outside the owned function; the record remains mutable DATA and no callability is claimed",
                f,
            )
        })
        .into_iter()
        .collect()
}
