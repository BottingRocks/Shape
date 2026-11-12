//! Whole ordered first/second-moment update, including guarded error forwarding.
use crate::naming::support::media_entropy::*;
use crate::naming::support::ordered_numeric::{eq, field, n, op, s};
use super::*;
fn target(model: &Model<'_, '_>, site: FunctionSite, st: &Statement<'_>) -> Option<Symbol> {
    capture_symbol(model, site, assigned_symbol(model, &assignment(st)?.left)?)
}
fn write(
    model: &Model<'_, '_>,
    st: &Statement<'_>,
    id: Symbol,
    value: crate::naming::support::ordered_numeric::E,
) -> Option<()> {
    let a = assignment(st)?;
    (assigned_symbol(model, &a.left) == Some(id) && eq(model, &a.right, value)).then_some(())
}
fn increment(model: &Model<'_, '_>, site: FunctionSite, st: &Statement<'_>) -> Option<Symbol> {
    let Expression::UpdateExpression(u) = expression_statement(st)?.without_parentheses() else {
        return None;
    };
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &u.argument else {
        return None;
    };
    if !u.prefix || u.operator.as_str() != "++" {
        return None;
    }
    capture_symbol(
        model,
        site,
        crate::analysis::identifier_symbol(id, model.scoping)?,
    )
}
fn prove(model: &Model<'_, '_>, site: FunctionSite) -> Option<()> {
    let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
        return None;
    };
    let b = f.body.as_ref()?;
    if f.r#async || f.generator || !b.directives.is_empty() {
        return None;
    }
    let params = plain_params(model, f)?;
    let [event] = params.as_slice() else {
        return None;
    };
    let [Statement::TryStatement(t)] = b.statements.as_slice() else {
        return None;
    };
    if t.finalizer.is_some() {
        return None;
    }
    let h = t.handler.as_ref()?;
    let BindingPattern::BindingIdentifier(err) = &h.param.as_ref()?.pattern else {
        return None;
    };
    let [report] = h.body.body.as_slice() else {
        return None;
    };
    let report = call(expression_statement(report)?)?;
    let args = call_arguments(report);
    if args.len() != 1
        || args.len() != report.arguments.len()
        || symbol(model, args[0]) != err.symbol_id.get()
    {
        return None;
    }
    let sink = capture(model, site, member(&report.callee, "error")?)?;
    let [clock, count, Statement::IfStatement(branch)] = t.block.body.as_slice() else {
        return None;
    };
    let v = one_variable(clock)?;
    let now = plain_binding(model, v, Some(0))?;
    let clock = call(v.init.as_ref()?)?;
    if !clock.arguments.is_empty()
        || !crate::naming::support::ordered_numeric::clock_origin(model, site, &clock.callee)
    {
        return None;
    }
    let count = increment(model, site, count)?;
    if !eq(model, &branch.test, op("<=", s(count), n(1.0))) {
        return None;
    }
    let first = block(&branch.consequent)?;
    if first.len() != 7 {
        return None;
    }
    let time = target(model, site, &first[0])?;
    write(model, &first[0], time, s(now))?;
    let mut means = Vec::new();
    let mut squares = Vec::new();
    for (i, axis) in ["alpha", "beta", "gamma"].iter().enumerate() {
        let mean = target(model, site, &first[1 + 2 * i])?;
        let square = target(model, site, &first[2 + 2 * i])?;
        let value = field(s(*event), axis);
        write(model, &first[1 + 2 * i], mean, value.clone())?;
        write(
            model,
            &first[2 + 2 * i],
            square,
            op("*", value.clone(), value),
        )?;
        means.push(mean);
        squares.push(square);
    }
    let later = block(branch.alternate.as_ref()?)?;
    if later.len() != 9 {
        return None;
    }
    let dt_mean = target(model, site, &later[0])?;
    let dt_square = target(model, site, &later[1])?;
    let mut distinct = BTreeSet::new();
    for id in [count, time, sink, dt_mean, dt_square]
        .into_iter()
        .chain(means.iter().copied())
        .chain(squares.iter().copied())
    {
        if !distinct.insert(id) {
            return None;
        }
    }
    let denominator = op("-", s(count), n(1.0));
    let decay = op("/", op("-", s(count), n(2.0)), denominator.clone());
    let weight = op("/", n(1.0), denominator);
    let dt = op("-", s(now), s(time));
    write(
        model,
        &later[0],
        dt_mean,
        op(
            "+",
            op("*", s(dt_mean), decay.clone()),
            op("*", dt.clone(), weight.clone()),
        ),
    )?;
    write(
        model,
        &later[1],
        dt_square,
        op(
            "+",
            op("*", s(dt_square), decay),
            op("*", op("*", dt.clone(), dt), weight),
        ),
    )?;
    write(model, &later[2], time, s(now))?;
    let decay = op("/", op("-", s(count), n(1.0)), s(count));
    let weight = op("/", n(1.0), s(count));
    for (i, axis) in ["alpha", "beta", "gamma"].iter().enumerate() {
        let v = field(s(*event), axis);
        write(
            model,
            &later[3 + i],
            means[i],
            op(
                "+",
                op("*", s(means[i]), decay.clone()),
                op("*", v.clone(), weight.clone()),
            ),
        )?;
        write(
            model,
            &later[6 + i],
            squares[i],
            op(
                "+",
                op("*", s(squares[i]), decay.clone()),
                op("*", op("*", v.clone(), v), weight.clone()),
            ),
        )?;
    }
    Some(())
}
pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    if prove(model, site).is_some() {
        vec![proposal("recordOrientationMoments","events_orientation_moments","complete guarded orientation callback: sample clock, increment captured count, initialize time and alpha/beta/gamma first and second moments on first sample; otherwise ordered elapsed-time and angle moment recurrences with exact scoped slots and operators; forward caught error to distinct sink",f)]
    } else {
        Vec::new()
    }
}
