use super::*;

use crate::naming::support::media_entropy::*;

fn static_property<'a>(target: &'a AssignmentTarget<'a>) -> Option<(&'a Expression<'a>, &'a str)> {
    let AssignmentTarget::StaticMemberExpression(m) = target else {
        return None;
    };
    (!m.optional).then_some((&m.object, m.property.name.as_str()))
}

fn touch_event_object(model: &Model<'_, '_>, init: &Expression<'_>, input: Symbol) -> Option<()> {
    let Expression::ObjectExpression(object) = init.without_parentheses() else {
        return None;
    };
    let entries = object_entries(object)?;
    if entries.len() != 2 {
        return None;
    }
    let mut trusted = false;
    let mut ui_event = false;
    for (key, value) in entries {
        match key.as_str() {
            "instanceOfUIEvent" => {
                let Some((left, right)) = binary(value, "instanceof") else {
                    return None;
                };
                if symbol(model, left) != Some(input)
                    || expression_path(model, right) != ["UIEvent"]
                {
                    return None;
                }
                ui_event = true;
            }
            "markedAsTrusted" => {
                let Expression::UnaryExpression(outer) = value.without_parentheses() else {
                    return None;
                };
                let Expression::UnaryExpression(inner) = outer.argument.without_parentheses()
                else {
                    return None;
                };
                if outer.operator.as_str() != "!"
                    || inner.operator.as_str() != "!"
                    || !member(&inner.argument, "isTrusted")
                        .is_some_and(|receiver| symbol(model, receiver) == Some(input))
                {
                    return None;
                }
                trusted = true;
            }
            _ => return None,
        }
    }
    (ui_event && trusted).then_some(())
}

fn touch_round(
    value: &Expression<'_>,
    model: &Model<'_, '_>,
    source: Symbol,
    property: &str,
) -> bool {
    let Some(round) = call(value) else {
        return false;
    };
    expression_path(model, &round.callee) == ["Math", "round"] && round.arguments.len() == 1 && call_arguments(round).len() == 1 && {
        let args = call_arguments(round);
        member(args[0], property).is_some_and(|receiver| symbol(model, receiver) == Some(source))
    }
}

/// A captured clock is opaque data, but its own binding must be immutable and
/// initialized before this callback from the explicit clock property.
fn split_clock(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> bool {
    if crate::naming::resolve::ordered_callable(model, e, 0).is_some() { return true; }
    let Some(id) = capture(model, site, e) else { return false; };
    let AstKind::VariableDeclarator(v) = model.nodes.kind(model.scoping.symbol_declaration(id)) else { return false; };
    plain_binding(model, v, Some(0)) == Some(id)
        && v.init.as_ref().is_some_and(|init| member(init, "timeElapsedSinceInit").and_then(|object| capture(model, site, object)).is_some())
}

fn touch_payload(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let params = plain_params(model, function)?;
    let [input] = params.as_slice() else {
        return None;
    };
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
    let [Statement::ExpressionStatement(report)] = handler.body.body.as_slice() else {
        return None;
    };
    let Some(report_call) = call(&report.expression) else {
        return None;
    };
    if report_call.arguments.len() != 1
        || call_arguments(report_call).len() != 1
        || symbol(model, call_arguments(report_call)[0]) != Some(error)
        || member(&report_call.callee, "error")
            .and_then(|e| capture(model, site, e))
            .is_none()
    {
        return None;
    }
    let report_sink = capture(model, site, member(&report_call.callee, "error")?)?;
    let statements = t.block.body.as_slice();
    let (first, second, touches_statement, branch, ring_alias, payload_statement, field_b, field_c, field_d, field_e, put_statement, flag_statement) = match statements {
        [first, second, touches, Statement::IfStatement(branch), payload, c, d, e, put, flag] =>
            (first, second, touches, branch, None, payload, None, c, d, e, put, Some(flag)),
        [first, second, touches, Statement::IfStatement(branch), ring, payload, b, c, d, e, put] =>
            (first, second, touches, branch, Some(ring), payload, Some(b), c, d, e, put, None),
        _ => return None,
    };
    let first_variable = one_variable(first)?;
    let second_variable = one_variable(second)?;
    let first_is_metadata = first_variable
        .init
        .as_ref()
        .is_some_and(|e| matches!(e.without_parentheses(), Expression::ObjectExpression(_)));
    let (metadata_variable, list_variable) = if first_is_metadata {
        (first_variable, second_variable)
    } else {
        (second_variable, first_variable)
    };
    let metadata = plain_binding(model, metadata_variable, Some(0))?;
    let list = plain_binding(model, list_variable, Some(0))?;
    if !empty_array(list_variable.init.as_ref()?)
        || touch_event_object(model, metadata_variable.init.as_ref()?, *input).is_none()
    {
        return None;
    }
    let touches_variable = one_variable(touches_statement)?;
    let touches = plain_binding(model, touches_variable, Some(0))?;
    let Some((selected, selector)) = indexed(touches_variable.init.as_ref()?) else {
        return None;
    };
    if symbol(model, selected) != Some(*input) {
        return None;
    }
    let Expression::ConditionalExpression(selector) = selector.without_parentheses() else {
        return None;
    };
    let Some((has_touches, empty)) = logical(&selector.test, "&&") else {
        return None;
    };
    if !member(has_touches, "touches")
        .is_some_and(|receiver| symbol(model, receiver) == Some(*input))
        || !matches!(binary(empty, "==="), Some((left, right))
            if member(left, "length")
                .and_then(|length| member(length, "touches"))
                .is_some_and(|receiver| symbol(model, receiver) == Some(*input))
                && number(right, 0.0))
        || !string(&selector.consequent, "changedTouches")
        || !string(&selector.alternate, "touches")
    {
        return None;
    }
    let Some((present, nonempty)) = logical(&branch.test, "&&") else {
        return None;
    };
    if symbol(model, present) != Some(touches)
        || !matches!(binary(nonempty, "!=="), Some((left, right))
            if member(left, "length").is_some_and(|receiver| symbol(model, receiver) == Some(touches))
                && number(right, 0.0))
        || branch.alternate.is_some()
    {
        return None;
    }
    let records = block(&branch.consequent)?;
    let [length_init, index_init, Statement::ForStatement(scan)] = records else {
        return None;
    };
    let length_variable = one_variable(length_init)?;
    let length = plain_binding(model, length_variable, Some(0))?;
    if !member(length_variable.init.as_ref()?, "length")
        .is_some_and(|receiver| symbol(model, receiver) == Some(touches))
    {
        return None;
    }
    let index_variable = one_variable(index_init)?;
    let index = plain_binding(model, index_variable, Some(1))?;
    if !number(index_variable.init.as_ref()?, 0.0) || scan.init.is_some() || scan.update.is_none() {
        return None;
    }
    let Some((left, right)) = binary(scan.test.as_ref()?, "<") else {
        return None;
    };
    if symbol(model, left) != Some(index) || symbol(model, right) != Some(length) {
        return None;
    }
    let Expression::UpdateExpression(update) = scan.update.as_ref()?.without_parentheses() else {
        return None;
    };
    if update.operator.as_str() != "++"
        || !matches!(&update.argument,
            SimpleAssignmentTarget::AssignmentTargetIdentifier(id)
                if crate::analysis::identifier_symbol(id, model.scoping) == Some(index))
    {
        return None;
    }
    let iteration = block(&scan.body)?;
    let (touch_init, target_init, bounds_statement, x_init, target_y_statement, record_statement, push) = match iteration {
        [touch, target, Statement::ExpressionStatement(bounds), x, Statement::ExpressionStatement(y), Statement::ExpressionStatement(push)] =>
            (touch, target, bounds, x, y, None, push),
        [touch, target, Statement::ExpressionStatement(bounds), x, Statement::ExpressionStatement(y), record, Statement::ExpressionStatement(push)] if ring_alias.is_some() =>
            (touch, target, bounds, x, y, Some(record), push),
        _ => return None,
    };
    let touch_variable = one_variable(touch_init)?;
    let touch = plain_binding(model, touch_variable, Some(0))?;
    if !indexed(touch_variable.init.as_ref()?).is_some_and(|(object, key)| {
        symbol(model, object) == Some(touches) && symbol(model, key) == Some(index)
    }) {
        return None;
    }
    let target_variable = one_variable(target_init)?;
    let target = plain_binding(model, target_variable, Some(if ring_alias.is_some() { 3 } else { 2 }))?;
    if !member(target_variable.init.as_ref()?, "target")
        .is_some_and(|receiver| symbol(model, receiver) == Some(touch))
    {
        return None;
    }
    let bounds = assignment_in(bounds_statement)?;
    let Expression::ConditionalExpression(bounds_choice) = bounds.right.without_parentheses()
    else {
        return None;
    };
    if assigned_symbol(model, &bounds.left) != Some(target)
        || !matches!(&bounds_choice.test.without_parentheses(),
            Expression::BinaryExpression(b) if b.operator.as_str() == "instanceof"
                && symbol(model, &b.left) == Some(target)
                && expression_path(model, &b.right) == ["Element"])
        || !call(&bounds_choice.consequent).is_some_and(|c| {
            c.arguments.is_empty()
                && member(&c.callee, "getBoundingClientRect")
                    .is_some_and(|receiver| symbol(model, receiver) == Some(target))
        })
        || !null_literal(&bounds_choice.alternate)
    {
        return None;
    }
    let x_variable = one_variable(x_init)?;
    let x = plain_binding(model, x_variable, Some(0))?;
    let Expression::ConditionalExpression(x_choice) =
        x_variable.init.as_ref()?.without_parentheses()
    else {
        return None;
    };
    if !matches!(binary(&x_choice.test, "=="), Some((left, right))
            if symbol(model, left) == Some(target) && null_literal(right))
        || !number(&x_choice.consequent, 0.0)
    {
        return None;
    }
    let Some(x_round) = call(&x_choice.alternate) else {
        return None;
    };
    if expression_path(model, &x_round.callee) != ["Math", "round"] || x_round.arguments.len() != 1 || call_arguments(x_round).len() != 1
    {
        return None;
    }
    let x_args = call_arguments(x_round);
    let Some((x_client, x_left)) = binary(x_args[0], "-") else {
        return None;
    };
    if !member(x_client, "clientX").is_some_and(|receiver| symbol(model, receiver) == Some(touch))
        || !member(x_left, "left").is_some_and(|receiver| symbol(model, receiver) == Some(target))
    {
        return None;
    }
    let target_y = assignment_in(target_y_statement)?;
    let Expression::ConditionalExpression(y_choice) = target_y.right.without_parentheses() else {
        return None;
    };
    if assigned_symbol(model, &target_y.left) != Some(target)
        || !matches!(binary(&y_choice.test, "=="), Some((left, right))
            if symbol(model, left) == Some(target) && null_literal(right))
        || !number(&y_choice.consequent, 0.0)
    {
        return None;
    }
    let Some(y_round) = call(&y_choice.alternate) else {
        return None;
    };
    if expression_path(model, &y_round.callee) != ["Math", "round"] || y_round.arguments.len() != 1 || call_arguments(y_round).len() != 1
    {
        return None;
    }
    let y_args = call_arguments(y_round);
    let Some((y_client, y_top)) = binary(y_args[0], "-") else {
        return None;
    };
    if !member(y_client, "clientY").is_some_and(|receiver| symbol(model, receiver) == Some(touch))
        || !member(y_top, "top").is_some_and(|receiver| symbol(model, receiver) == Some(target))
    {
        return None;
    }
    let Some(push_call) = call(&push.expression) else {
        return None;
    };
    if push_call.arguments.len() != 1
        || call_arguments(push_call).len() != 1
        || !member(&push_call.callee, "push")
            .is_some_and(|receiver| symbol(model, receiver) == Some(list))
    {
        return None;
    }
    let record_expression = if let Some(statement) = record_statement {
        let a = assignment(statement)?;
        if assigned_symbol(model, &a.left) != Some(target)
            || symbol(model, call_arguments(push_call)[0]) != Some(target) { return None; }
        &a.right
    } else { call_arguments(push_call)[0] };
    let Expression::ObjectExpression(record) = record_expression.without_parentheses()
    else {
        return None;
    };
    let entries = object_entries(record)?;
    if entries.len() != 5 {
        return None;
    }
    for (key, value) in entries {
        match key.as_str() {
            "c" => {
                if symbol(model, value) != Some(x) {
                    return None;
                }
            }
            "d" => {
                if symbol(model, value) != Some(target) {
                    return None;
                }
            }
            "e" => {
                if !member(value, "force")
                    .is_some_and(|receiver| symbol(model, receiver) == Some(touch))
                {
                    return None;
                }
            }
            "a" => {
                if !touch_round(value, model, touch, "pageX") {
                    return None;
                }
            }
            "b" => {
                if !touch_round(value, model, touch, "pageY") {
                    return None;
                }
            }
            _ => return None,
        }
    }
    let payload_variable = one_variable(payload_statement)?;
    let payload = plain_binding(model, payload_variable, Some(0))?;
    let Expression::ObjectExpression(payload_object) =
        payload_variable.init.as_ref()?.without_parentheses()
    else {
        return None;
    };
    let entries = object_entries(payload_object)?;
    if entries.len() != if field_b.is_some() { 1 } else { 2 } {
        return None;
    }
    let mut booleans = false;
    let mut sampled = field_b.is_some();
    for (key, value) in entries {
        match key.as_str() {
            "a" => {
                let Expression::ArrayExpression(a) = value.without_parentheses() else {
                    return None;
                };
                let [first, second] = a.elements.as_slice() else {
                    return None;
                };
                let first = first.as_expression()?;
                let second = second.as_expression()?;
                if !(boolean(first, true) || boolean(first, false))
                    || !(boolean(second, true) || boolean(second, false))
                {
                    return None;
                }
                booleans = true;
            }
            "b" => {
                let Some(sample) = call(value) else {
                    return None;
                };
                if !sample.arguments.is_empty()
                    || crate::naming::resolve::ordered_callable(model, &sample.callee, 0).is_none()
                {
                    return None;
                }
                sampled = true;
            }
            _ => return None,
        }
    }
    if !booleans || !sampled {
        return None;
    }
    if let Some(statement) = field_b {
        let a = assignment(statement)?;
        let (object, key) = static_property(&a.left)?;
        let clock = call(&a.right)?;
        if key != "b" || symbol(model, object) != Some(payload)
            || !clock.arguments.is_empty()
            || !split_clock(model, site, &clock.callee) {
            return None;
        }
    }
    for (field_statement, expected) in [
        (field_c, ("c", list)),
        (field_d, ("d", metadata)),
        (field_e, ("e", metadata)),
    ] {
        let field = assignment(field_statement)?;
        let Some((object, property)) = static_property(&field.left) else {
            return None;
        };
        if property != expected.0 || symbol(model, object) != Some(payload) {
            return None;
        }
        let valid = if expected.0 == "c" {
            symbol(model, &field.right) == Some(expected.1)
        } else if expected.0 == "d" {
            member(&field.right, "instanceOfUIEvent")
                .is_some_and(|receiver| symbol(model, receiver) == Some(expected.1))
        } else {
            member(&field.right, "markedAsTrusted")
                .is_some_and(|receiver| symbol(model, receiver) == Some(expected.1))
        };
        if !valid {
            return None;
        }
    }
    let Some(put) = call(expression_statement(put_statement)?) else {
        return None;
    };
    if put.arguments.len() != 1
        || call_arguments(put).len() != 1
        || symbol(model, call_arguments(put)[0]) != Some(payload)
        || !member(&put.callee, "put").is_some_and(|receiver| {
            if let Some(statement) = ring_alias {
                one_variable(statement).is_some_and(|v| {
                    plain_binding(model, v, Some(0)).is_some_and(|id| symbol(model, receiver) == Some(id))
                        && v.init.as_ref().and_then(|e| capture(model, site, e)).is_some_and(|ring| ring != report_sink)
                })
            } else { capture(model, site, receiver).is_some() }
        })
    {
        return None;
    }
    let Some(flag_statement) = flag_statement else { return Some(()); };
    let flag = assignment(flag_statement)?;
    let target = assigned_symbol(model, &flag.left)?;
    capture_symbol(model, site, target)?;
    boolean(&flag.right, true).then_some(())
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
    if function.params.items.len() == 1 && touch_payload(model, site, function, body).is_some() {
        claims.push(proposal(
            "recordTouchSample",
            "events_touch_payload_record",
            "guarded touch-event reader selects changedTouches/touches, maps UIEvent/Element-rounded coordinates into an owned b/c/d/e payload and puts it into a captured data receiver; catch path reports through a distinct captured error sink",
            f,
        ));
    }
    claims
}
