use super::*;

use crate::naming::support::interaction::*;

fn record_animation_frame_sample(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<bool> {
    if function.params.rest.is_some() || function.params.items.len() != 1 {
        return None;
    }
    let input = plain_param(model, &function.params.items[0])?;
    let (push, flag, guard) = match body.statements.as_slice() {
        [push, flag, guard] => (push, Some(flag), guard),
        [push, guard] => (push, None, guard),
        _ => return None,
    };
    let push = call(expression_statement(push)?)?;
    let list = captured_symbol(model, site, method_receiver(model, push, "push")?)?;
    let push_args = arguments(push)?;
    let [value] = push_args.as_slice() else {
        return None;
    };
    let value = call(value)?;
    let value_args = arguments(value)?;
    let [ratio] = value_args.as_slice() else {
        return None;
    };
    let (left, right) = binary(ratio, "/")?;
    let interval = captured_symbol(model, site, symbol(model, right)?)?;
    if expression_path(model, &value.callee) != ["Math", "round"]
        || symbol(model, left) != Some(input)
    {
        return None;
    }
    let flag = if let Some(flag) = flag {
        let flag_assignment = assignment(expression_statement(flag)?)?;
        if !boolean(&flag_assignment.right, true) { return None; }
        Some(captured_symbol(model, site, target_symbol(model, &flag_assignment.left)?)?)
    } else { None };
    let Statement::IfStatement(guard) = guard else {
        return None;
    };
    if guard.alternate.is_some() {
        return None;
    }
    let (left, right) = binary(&guard.test, "<")?;
    if member(left, "length").and_then(|e| symbol(model, e)) != Some(list) || !number(right, 20.0) {
        return None;
    }
    let Statement::BlockStatement(block) = &guard.consequent else {
        return None;
    };
    let [Statement::ExpressionStatement(reschedule)] = block.body.as_slice() else {
        return None;
    };
    let reschedule = call(&reschedule.expression)?;
    let object = captured_symbol(
        model,
        site,
        method_receiver(model, reschedule, "requestAnimationFrame")?,
    )?;
    let reschedule_args = arguments(reschedule)?;
    let [handler] = reschedule_args.as_slice() else {
        return None;
    };
    let handler = captured_symbol(model, site, symbol(model, handler)?)?;
    if object == list
        || Some(object) == flag
        || object == handler
        || object == interval
        || Some(list) == flag
        || list == handler
        || list == interval
    {
        return None;
    }
    Some(flag.is_some())
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
    let route = record_animation_frame_sample(model, site, function, body);
    if route == Some(true) {
        claims.push(proposal(
            "recordAnimationFrameSample",
            "graphics_animation_frame_sample",
            "pushes Math.round(formal / captured divisor) into one captured sample array, sets a distinct captured flag to true, and requests exactly one more frame through a distinct captured receiver with a distinct captured handler only while the captured sample array is shorter than twenty entries; all captures are plain var/let/const ancestor declarators",
            f,
        ));
    }
    // The older self-registration grammar keeps its proposal and metadata.
    // This fallback describes an opaque handler only when that proof declines.
    if route == Some(false)
        && super::super::scheduling::frame_sampler::evaluate(model, site, function, f).is_empty()
    {
        claims.push(proposal(
            "recordScaledFrameAndRequestCallback",
            "graphics_animation_frame_sample",
            "entire two-statement owned body appends Math.round(the unchanged formal divided by a captured divisor) to one captured list, then requests one animation frame on a distinct captured receiver with a distinct captured handler only while that same list has fewer than twenty entries; all captures are plain lexical ancestor declarators; callback identity and native scheduler origin remain opaque; no self-rescheduling claim",
            f,
        ));
    }
    claims
}
