use super::*;

use crate::naming::support::interaction::*;

fn count_events_outside_selector(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if function.params.rest.is_some() || function.params.items.len() != 1 {
        return None;
    }
    let input = plain_param(model, &function.params.items[0])?;
    let [Statement::TryStatement(t)] = body.statements.as_slice() else {
        return None;
    };
    if t.finalizer.is_some() {
        return None;
    }
    let handler = t.handler.as_ref()?;
    let BindingPattern::BindingIdentifier(error) = &handler.param.as_ref()?.pattern else {
        return None;
    };
    let error = error.symbol_id.get()?;
    if model.binding_writes(error) != 0 || !model.scoping.symbol_redeclarations(error).is_empty() {
        return None;
    }
    let [Statement::ExpressionStatement(failed)] = handler.body.body.as_slice() else {
        return None;
    };
    let failed = call(&failed.expression)?;
    captured_symbol(model, site, method_receiver(model, failed, "error")?)?;
    let failed_args = arguments(failed)?;
    if failed_args.len() != 1 || symbol(model, failed_args[0]) != Some(error) {
        return None;
    }
    let [Statement::IfStatement(outer)] = t.block.body.as_slice() else {
        return None;
    };
    if outer.alternate.is_some()
        || member(&outer.test, "target").and_then(|e| symbol(model, e)) != Some(input)
    {
        return None;
    }
    let Statement::BlockStatement(inner_block) = &outer.consequent else {
        return None;
    };
    let [Statement::IfStatement(inner)] = inner_block.body.as_slice() else {
        return None;
    };
    if inner.alternate.is_some() {
        return None;
    }
    let (left, right) = binary(&inner.test, "==")?;
    if !matches!(right, Expression::NullLiteral(_)) {
        return None;
    }
    let closest = call(left)?;
    let Expression::StaticMemberExpression(closest_member) = &closest.callee else {
        return None;
    };
    if closest_member.optional
        || closest_member.property.name != "closest"
        || member(&closest_member.object, "target").and_then(|e| symbol(model, e)) != Some(input)
    {
        return None;
    }
    let closest_args = arguments(closest)?;
    let [selector] = closest_args.as_slice() else {
        return None;
    };
    let selector = call(selector)?;
    let selectors = captured_symbol(model, site, method_receiver(model, selector, "join")?)?;
    let selector_args = arguments(selector)?;
    if !matches!(selector_args.as_slice(), [Expression::StringLiteral(s)] if s.value == ",") {
        return None;
    }
    let Statement::BlockStatement(increment) = &inner.consequent else {
        return None;
    };
    let [Statement::ExpressionStatement(increment)] = increment.body.as_slice() else {
        return None;
    };
    let Expression::UpdateExpression(increment) = &increment.expression else {
        return None;
    };
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(counter) = &increment.argument else {
        return None;
    };
    let counter = captured_symbol(
        model,
        site,
        crate::analysis::identifier_symbol(counter, model.scoping)?,
    )?;
    if increment.operator.as_str() != "++" || counter == selectors {
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
    if count_events_outside_selector(model, site, function, body).is_some() {
        claims.push(proposal(
            "countEventsOutsideSelector",
            "events_selector_event_count",
            "catch-guarded handler increments the captured counter only when the formal target exists and its closest(joined captured selectors) result is null; errors forward the exact caught binding to a captured error receiver",
            f,
        ));
    }
    claims
}
