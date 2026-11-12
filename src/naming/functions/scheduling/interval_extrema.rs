//! Complete elapsed-interval recorder with mutually exclusive extrema updates.
use crate::naming::support::media_entropy::*;
use crate::naming::support::ordered_numeric::{eq, op, s, E};
use super::*;
fn write(model: &Model<'_, '_>, st: &Statement<'_>, id: Symbol, value: E) -> Option<()> {
    let a = assignment(st)?;
    (assigned_symbol(model, &a.left) == Some(id) && eq(model, &a.right, value)).then_some(())
}
fn prove(model: &Model<'_, '_>, site: FunctionSite) -> Option<bool> {
    let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
        return None;
    };
    let b = f.body.as_ref()?;
    if f.r#async || f.generator || !b.directives.is_empty() || !plain_params(model, f)?.is_empty() {
        return None;
    }
    let (clock, old, save, put, init, inc, completion) = match b.statements.as_slice() {
        [clock, old, save, put, Statement::IfStatement(init), inc] => (clock, old, save, put, init, inc, None),
        [clock, old, save, put, Statement::IfStatement(init), inc, done] => (clock, old, save, put, init, inc, Some(done)),
        _ => return None,
    };
    let v = one_variable(clock)?;
    let now = plain_binding(model, v, Some(0))?;
    let call_clock = call(v.init.as_ref()?)?;
    if !call_clock.arguments.is_empty()
        || !crate::naming::support::ordered_numeric::clock_origin(model, site, &call_clock.callee)
    {
        return None;
    }
    let v = one_variable(old)?;
    let old = plain_binding(model, v, Some(0))?;
    // Snapshot lowering and precomputed-delta lowering have different second
    // declarations. Both preserve the read-before-save timestamp ordering.
    let (previous, delta) = if let Some(previous) = capture(model, site, v.init.as_ref()?) {
        (previous, op("-", s(now), s(old)))
    } else {
        let Expression::BinaryExpression(sub) = v.init.as_ref()?.without_parentheses() else { return None; };
        let previous = capture(model, site, &sub.right)?;
        if sub.operator.as_str() != "-" || !eq(model, &sub.left, s(now)) { return None; }
        (previous, s(old))
    };
    write(model, save, previous, s(now))?;
    let put = call(expression_statement(put)?)?;
    let ring = capture(model, site, member(&put.callee, "put")?)?;
    let args = call_arguments(put);
    if args.len() != 1 || args.len() != put.arguments.len() || !eq(model, args[0], delta.clone()) {
        return None;
    }
    let first = block(&init.consequent)?;
    let [max_st, min_st] = first else {
        return None;
    };
    let max = capture_symbol(
        model,
        site,
        assigned_symbol(model, &assignment(max_st)?.left)?,
    )?;
    let min = capture_symbol(
        model,
        site,
        assigned_symbol(model, &assignment(min_st)?.left)?,
    )?;
    if !eq(
        model,
        &init.test,
        op("&&", op("===", s(max), E::Void), op("===", s(min), E::Void)),
    ) {
        return None;
    }
    write(model, max_st, max, delta.clone())?;
    write(model, min_st, min, delta.clone())?;
    let Statement::IfStatement(high) = init.alternate.as_ref()? else {
        return None;
    };
    if !eq(model, &high.test, op(">", delta.clone(), s(max))) {
        return None;
    }
    let [st] = block(&high.consequent)? else {
        return None;
    };
    write(model, st, max, delta.clone())?;
    let Statement::IfStatement(low) = high.alternate.as_ref()? else {
        return None;
    };
    if low.alternate.is_some() || !eq(model, &low.test, op("<", delta.clone(), s(min))) {
        return None;
    }
    let [st] = block(&low.consequent)? else {
        return None;
    };
    write(model, st, min, delta)?;
    let Expression::UpdateExpression(u) = expression_statement(inc)?.without_parentheses() else {
        return None;
    };
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &u.argument else {
        return None;
    };
    if !u.prefix || u.operator.as_str() != "++" {
        return None;
    }
    let count = capture_symbol(
        model,
        site,
        crate::analysis::identifier_symbol(id, model.scoping)?,
    )?;
    let distinct: BTreeSet<_> = [previous, ring, max, min, count].into_iter().collect();
    if distinct.len() != 5 { return None; }
    if let Some(done) = completion {
        let a = assignment(done)?;
        let flag = capture_symbol(model, site, assigned_symbol(model, &a.left)?)?;
        if distinct.contains(&flag) || !matches!(a.right.without_parentheses(), Expression::BooleanLiteral(b) if b.value) { return None; }
    }
    Some(completion.is_some())
}
pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    if let Some(completed) = prove(model, site) {
        if completed {
            let mut claim = proposal("recordIntervalAndExtrema", "scheduling_interval_extrema_completion", "Complete zero-formal callback: source-proven ordered clock, exact elapsed difference read before previous timestamp update, one ring put, initialize both undefined extrema or mutually exclusive maximum/minimum update, prefix increment of distinct captured counter, then true assignment to distinct completion flag; every statement, scoped receiver, value and effect order checked", f);
            claim.evidence = vec![site.span.into()];
            return vec![claim];
        }
        vec![proposal("recordIntervalAndExtrema","scheduling_interval_extrema","complete zero-formal callback: sample ordered clock, save previous captured timestamp, put exact elapsed delta, initialize both undefined extrema or update maximum then minimum in mutually exclusive branches, increment distinct sample counter; exact scoped identities and effect order",f)]
    } else {
        Vec::new()
    }
}
