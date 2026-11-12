//! Pass-8 rolling frontier: one bounded, owned-body timer adapter.
//!
//! The rule deliberately distinguishes DATA identity from a proven callee role.
//! The scheduled value, the provider object and the stored handle are captured
//! plain `var`/`let`/`const` data bindings; only the delay producer is required
//! to be an independently ordered callable. The claim is therefore the exact
//! provider protocol (`capturedProvider.setTimeout(value, boundedDelay)`), never
//! a browser-native assertion about the scheduled value.
use super::*;

type Symbol = oxc_semantic::SymbolId;

fn plain_math_member(model: &Model<'_, '_>, e: &Expression<'_>, method: &str) -> bool {
    let Expression::StaticMemberExpression(member) = e else {
        return false;
    };
    !member.optional
        && member.property.name == method
        && matches!(&member.object, Expression::Identifier(id)
            if id.name == "Math" && crate::analysis::identifier_symbol(id, model.scoping).is_none())
}

// A captured plain data binding: an ordinary `var`/`let`/`const` declarator
// outside the owned body and declared before it. Receivers may legitimately be
// reset by setup, so this is not a callable-origin proof and is never used as
// one.
fn plain_data_binding(model: &Model<'_, '_>, site: FunctionSite, s: Symbol) -> Option<()> {
    let span = model.scoping.symbol_span(s);
    if !model.scoping.symbol_redeclarations(s).is_empty()
        || site.span.contains_inclusive(span)
        || span.end >= site.span.start
    {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(declarator) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&declarator.id, BindingPattern::BindingIdentifier(id)
        if id.symbol_id.get() == Some(s))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(d)
                if matches!(
                    d.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                )
        )
        || matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
    {
        return None;
    }
    Some(())
}

fn captured_data(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<Symbol> {
    let s = crate::analysis::expression_symbol(e, model.scoping)?;
    plain_data_binding(model, site, s).map(|()| s)
}

// `Math.floor(Math.max(2000, ordered() % 4000))` exactly. `Math` must be the
// unresolved global identifier (a local shadow is refused), and the producer
// must be an independently ordered callable, not a parameter or opaque value.
fn bounded_delay(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<()> {
    let Expression::CallExpression(floor) = e else {
        return None;
    };
    if floor.optional || !plain_math_member(model, &floor.callee, "floor") {
        return None;
    }
    let [floor_argument] = floor.arguments.as_slice() else {
        return None;
    };
    let Expression::CallExpression(max) = floor_argument.as_expression()? else {
        return None;
    };
    if max.optional || !plain_math_member(model, &max.callee, "max") {
        return None;
    }
    let [lower, remainder] = max.arguments.as_slice() else {
        return None;
    };
    let Expression::NumericLiteral(lower) = lower.as_expression()? else {
        return None;
    };
    if lower.value != 2_000.0 {
        return None;
    }
    let Expression::BinaryExpression(binary) = remainder.as_expression()? else {
        return None;
    };
    if binary.operator.as_str() != "%" {
        return None;
    }
    let Expression::NumericLiteral(span) = &binary.right else {
        return None;
    };
    if span.value != 4_000.0 {
        return None;
    }
    let Expression::CallExpression(producer) = &binary.left else {
        return None;
    };
    if producer.optional || !producer.arguments.is_empty() {
        return None;
    }
    let target = crate::naming::resolve::ordered_callable(model, &producer.callee, 0)?;
    // A self-recursive delay producer is a different operation.
    if target.span == site.span {
        return None;
    }
    Some(())
}

fn bounded_timeout(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [Statement::VariableDeclaration(declaration), Statement::TryStatement(retry), Statement::ExpressionStatement(store)] =
        body.statements.as_slice()
    else {
        return None;
    };
    if !matches!(
        declaration.kind,
        VariableDeclarationKind::Var
            | VariableDeclarationKind::Let
            | VariableDeclarationKind::Const
    ) {
        return None;
    }
    let [delay_declaration] = declaration.declarations.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(delay_id) = &delay_declaration.id else {
        return None;
    };
    let delay = delay_id.symbol_id.get()?;
    if model.binding_writes(delay) != 1
        || !site
            .span
            .contains_inclusive(model.scoping.symbol_span(delay))
        || !model.scoping.symbol_redeclarations(delay).is_empty()
        || !matches!(delay_declaration.init.as_ref(), Some(Expression::NumericLiteral(base))
            if base.value == 2_000.0)
    {
        return None;
    }

    // The owned retry block is exactly one plain assignment to the local delay.
    if retry.finalizer.is_some() {
        return None;
    }
    let [Statement::ExpressionStatement(retry_statement)] = retry.block.body.as_slice() else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = &retry_statement.expression else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(assigned) = &assignment.left else {
        return None;
    };
    if assignment.operator.as_str() != "="
        || crate::analysis::identifier_symbol(assigned, model.scoping) != Some(delay)
    {
        return None;
    }
    bounded_delay(model, site, &assignment.right)?;

    // The catch is empty and does not mutate the delay or capture anything.
    let handler = retry.handler.as_ref()?;
    if !handler.body.body.is_empty() {
        return None;
    }
    let BindingPattern::BindingIdentifier(caught) = &handler.param.as_ref()?.pattern else {
        return None;
    };
    let caught = caught.symbol_id.get()?;
    if model.binding_writes(caught) != 0 || !model.scoping.symbol_redeclarations(caught).is_empty()
    {
        return None;
    }

    // `handle = provider.setTimeout(value, delay)` with distinct captured data
    // identities. The scheduled value's callability is intentionally not
    // asserted: it is opaque captured DATA for this provider protocol.
    let Expression::AssignmentExpression(store) = &store.expression else {
        return None;
    };
    if store.operator.as_str() != "=" {
        return None;
    }
    let AssignmentTarget::AssignmentTargetIdentifier(handle) = &store.left else {
        return None;
    };
    let handle = crate::analysis::identifier_symbol(handle, model.scoping)?;
    if handle == delay {
        return None;
    }
    plain_data_binding(model, site, handle)?;
    let Expression::CallExpression(timer) = &store.right else {
        return None;
    };
    if timer.optional {
        return None;
    }
    let Expression::StaticMemberExpression(member) = &timer.callee else {
        return None;
    };
    if member.optional || member.property.name != "setTimeout" {
        return None;
    }
    let provider = captured_data(model, site, &member.object)?;
    let [scheduled, delay_argument] = timer.arguments.as_slice() else {
        return None;
    };
    let value = captured_data(model, site, scheduled.as_expression()?)?;
    if crate::analysis::expression_symbol(delay_argument.as_expression()?, model.scoping)
        != Some(delay)
        || provider == handle
        || provider == value
        || value == handle
    {
        return None;
    }
    Some(())
}

fn plain_formals(model: &Model<'_, '_>, function: &Function<'_>) -> bool {
    function.params.rest.is_none()
        && function.params.items.iter().all(|parameter| {
            parameter.initializer.is_none()
                && matches!(&parameter.pattern, BindingPattern::BindingIdentifier(id)
                if id.symbol_id.get().is_some_and(|s| {
                    model.binding_writes(s) == 0
                        && model.scoping.symbol_redeclarations(s).is_empty()
                }))
        })
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return vec![];
    };
    let Some(body) = function.body.as_ref() else {
        return vec![];
    };
    if function.r#async
        || function.generator
        || !function.params.items.is_empty()
        || !plain_formals(model, function)
        || !(body.directives.is_empty()
            || (body.directives.len() == 1 && body.directives[0].directive == "use strict"))
        || bounded_timeout(model, site, body).is_none()
    {
        return vec![];
    }
    vec![proposal(
        "scheduleBoundedTimeout",
        "scheduling_bounded_timeout",
        "exact owned body: local base 2000; one empty-catch try recomputes it as Math.floor(Math.max(2000, orderedCallable() % 4000)) where Math is the unresolved global provider and orderedCallable is independently ordered; then one store capturedHandle = capturedProvider.setTimeout(capturedValue, base). Provider/value/handle are distinct captured plain var/let/const bindings declared before the owned body; the scheduled value is opaque captured DATA (its callability is not asserted), so this is the complete provider protocol, not a native timer or callee assertion.",
        f,
    )]
}
