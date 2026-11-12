//! An exact timestamp->rounded sample->same-list bound->self reschedule flow.
//! The receiver is a DATA protocol object; no native/global identity is claimed.
use super::super::browser::proxy_probe::{
    ambient, assignment_target_symbol, block, call, expr, member, number, stable, sym,
};
use super::*;
use oxc_span::GetSpan;

fn captured(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<SymbolId> {
    let symbol = sym(model, e)?;
    if site
        .span
        .contains_inclusive(model.scoping.symbol_span(symbol))
        || !model.scoping.symbol_redeclarations(symbol).is_empty()
    {
        return None;
    }
    let AstKind::VariableDeclarator(v) = model.nodes.kind(model.scoping.symbol_declaration(symbol))
    else {
        return None;
    };
    matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get()==Some(symbol))
        .then_some(symbol)
}
fn formal(function: &Function<'_>) -> Option<SymbolId> {
    let [p] = function.params.items.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(id) = &p.pattern else {
        return None;
    };
    id.symbol_id.get()
}
/// The alias's only assignment must be the next sibling after this exact
/// declaration, followed immediately by initial scheduling of this function.
/// This refuses nested, stale, branch-incomplete and cross-callable origins.
fn registration(
    model: &Model<'_, '_>,
    site: FunctionSite,
    alias: SymbolId,
    receiver: SymbolId,
) -> Option<()> {
    let own = site.binding_symbol?;
    if !stable(model, own) || model.binding_writes(alias) != 1 || model.initializer(alias).is_some()
    {
        return None;
    }
    let AstKind::BlockStatement(parent) = model.nodes.parent_kind(site.node_id) else {
        return None;
    };
    let index = parent.body.iter().position(|s| s.span() == site.span)?;
    let Expression::AssignmentExpression(assign) = expr(parent.body.get(index + 1)?)? else {
        return None;
    };
    if assign.operator != AssignmentOperator::Assign
        || assignment_target_symbol(model, &assign.left) != Some(alias)
        || sym(model, &assign.right) != Some(own)
    {
        return None;
    }
    let initial = call(expr(parent.body.get(index + 2)?)?)?;
    let args = call_arguments(initial);
    (initial.arguments.len() == 1
        && args.len() == 1
        && sym(model, args[0]) == Some(own)
        && sym(model, member(&initial.callee, "requestAnimationFrame")?) == Some(receiver))
    .then_some(())
}
fn contract(model: &Model<'_, '_>, site: FunctionSite, function: &Function<'_>) -> Option<()> {
    let input = formal(function)?;
    if !stable(model, input) {
        return None;
    }
    let [push, Statement::IfStatement(branch)] = function.body.as_ref()?.statements.as_slice()
    else {
        return None;
    };
    let push = call(expr(push)?)?;
    let samples = captured(model, site, member(&push.callee, "push")?)?;
    let args = call_arguments(push);
    if push.arguments.len() != 1 || args.len() != 1 {
        return None;
    }
    let round = call(args[0])?;
    if !ambient(model, member(&round.callee, "round")?, "Math") || round.arguments.len() != 1 {
        return None;
    }
    let args = call_arguments(round);
    if args.len() != 1 {
        return None;
    }
    let Expression::BinaryExpression(divide) = args[0].without_parentheses() else {
        return None;
    };
    if divide.operator != BinaryOperator::Division || sym(model, &divide.left) != Some(input) {
        return None;
    }
    let _scale = captured(model, site, &divide.right)?;
    let Expression::BinaryExpression(bound) = branch.test.without_parentheses() else {
        return None;
    };
    if branch.alternate.is_some()
        || bound.operator != BinaryOperator::LessThan
        || sym(model, member(&bound.left, "length")?) != Some(samples)
        || !number(&bound.right, 20.0)
    {
        return None;
    }
    let [schedule] = block(&branch.consequent)? else {
        return None;
    };
    let schedule = call(expr(schedule)?)?;
    let receiver = captured(
        model,
        site,
        member(&schedule.callee, "requestAnimationFrame")?,
    )?;
    let args = call_arguments(schedule);
    if schedule.arguments.len() != 1 || args.len() != 1 {
        return None;
    }
    let alias = captured(model, site, args[0])?;
    registration(model, site, alias, receiver)
}
pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    f: &Facts<'_>,
) -> Vec<Proposal> {
    if contract(model, site, function).is_none() {
        return vec![];
    }
    vec![proposal("sampleNormalizedAnimationFrame", "shared_browser_frame_sampler",
        "entire owned body appends Math.round(formal/capturedScale) to one captured list; same-list length <20 gates requestAnimationFrame on a captured DATA receiver; exact adjacent owner registration establishes the sole callback alias as this function and initially schedules this same binding on the same receiver",f)]
}
