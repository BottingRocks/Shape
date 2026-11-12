use super::*;
#[path="options_envelope.rs"]
mod options_envelope;

use crate::naming::support::media_entropy::*;

/// Additive cursor spelling for the complete snapshot proof. This reads the
/// original AST; it does not hoist or rewrite a lexical loop declaration.
/// The shared proof still owns the test, update, captures, both routing arms,
/// method/receiver/payload identities and the complete returned report.
pub(super) fn inline_snapshot_cursor(
    model: &Model<'_, '_>,
    scan: &ForStatement<'_>,
) -> Option<Symbol> {
    let ForStatementInit::VariableDeclaration(declaration) = scan.init.as_ref()? else {
        return None;
    };
    if declaration.kind != VariableDeclarationKind::Let {
        return None;
    }
    let [variable] = declaration.declarations.as_slice() else {
        return None;
    };
    let cursor = plain_binding(model, variable, Some(1))?;
    if !number(variable.init.as_ref()?, 0.0) {
        return None;
    }

    // Only the holder form is added here. Reading `.push` and reading the
    // descriptor/value payload can invoke getters, so do not treat their
    // relative order as interchangeable merely because the final locals agree.
    let [
        _,
        Statement::VariableDeclaration(holders),
        Statement::IfStatement(partition),
        _,
    ] = block(&scan.body)?
    else {
        return None;
    };
    let [target, push, payload] = holders.declarations.as_slice() else {
        return None;
    };
    let target = plain_binding(model, target, Some(2))?;
    let push = plain_binding(model, push, Some(2))?;
    let payload = plain_binding(model, payload, Some(2))?;
    for (branch, named) in [
        (block(&partition.consequent)?, true),
        (block(partition.alternate.as_ref()?)?, false),
    ] {
        let [
            Statement::ExpressionStatement(first),
            Statement::ExpressionStatement(second),
            Statement::ExpressionStatement(last),
        ] = branch
        else {
            return None;
        };
        let first = assigned_symbol(model, &assignment_in(first)?.left)?;
        let second_assignment = assignment_in(second)?;
        let second = assigned_symbol(model, &second_assignment.left)?;
        let last = assigned_symbol(model, &assignment_in(last)?.left)?;
        if named
            && symbol(model, member(&second_assignment.right, "push")?) != Some(target)
        {
            return None;
        }
        if last != payload
            || if named {
                first != target || second != push
            } else {
                !((first == target && second == push) || (first == push && second == target))
            }
        {
            return None;
        }
    }
    Some(cursor)
}

fn member_is(model: &Model<'_, '_>, e: &Expression<'_>, root: Symbol, key: &str) -> bool {
    member(e, key).is_some_and(|receiver| symbol(model, receiver) == Some(root))
}

fn indexed_is(model: &Model<'_, '_>, e: &Expression<'_>, root: Symbol, key: Symbol) -> bool {
    indexed(e).is_some_and(|(object, index)| {
        symbol(model, object) == Some(root) && symbol(model, index) == Some(key)
    })
}

fn arrow_expressions<'a>(
    model: &Model<'_, '_>,
    arg: &'a Argument<'a>,
) -> Option<(&'a ArrowFunctionExpression<'a>, Symbol)> {
    let Argument::ArrowFunctionExpression(arrow) = arg else {
        return None;
    };
    if arrow.r#async || !arrow.body.directives.is_empty() {
        return None;
    }
    let params = plain_formals(model, &arrow.params)?;
    let [param] = params.as_slice() else {
        return None;
    };
    Some((arrow, *param))
}

fn signal_report(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let params = plain_params(model, function)?;
    let [input] = params.as_slice() else {
        return None;
    };
    let [
        options_statement,
        filter_statement,
        Statement::IfStatement(notify),
        values_statement,
        Statement::IfStatement(include),
        signals_statement,
        positional_statement,
        keyed_statement,
        index_statement,
        Statement::ForStatement(scan),
        end,
    ] = body.statements.as_slice()
    else {
        return None;
    };
    let options_variable = one_variable(options_statement)?;
    let options = plain_binding(model, options_variable, Some(0))?;
    let Expression::ConditionalExpression(options_default) =
        options_variable.init.as_ref()?.without_parentheses()
    else {
        return None;
    };
    let Some((options_test, options_void)) = binary(&options_default.test, "===") else {
        return None;
    };
    if symbol(model, options_test) != Some(*input)
        || !void_zero(options_void)
        || !matches!(&options_default.consequent.without_parentheses(),
            Expression::ObjectExpression(o) if o.properties.is_empty())
        || symbol(model, &options_default.alternate) != Some(*input)
    {
        return None;
    }
    let filter_variable = one_variable(filter_statement)?;
    let filter = plain_binding(model, filter_variable, Some(0))?;
    let Expression::ConditionalExpression(filter_default) =
        filter_variable.init.as_ref()?.without_parentheses()
    else {
        return None;
    };
    let Some((filter_test, filter_void)) = binary(&filter_default.test, "===") else {
        return None;
    };
    if !member_is(model, filter_test, options, "signalFilter")
        || !void_zero(filter_void)
        || capture(model, site, &filter_default.consequent).is_none()
        || !member_is(model, &filter_default.alternate, options, "signalFilter")
    {
        return None;
    }
    let Some((notify_or, notify_value)) = logical(&notify.test, "||") else {
        return None;
    };
    let Some((notify_void, notify_option)) = binary(notify_or, "===") else {
        return None;
    };
    if notify.alternate.is_some()
        || !member_is(model, notify_void, options, "notifyLastChance")
        || !void_zero(notify_option)
        || !member_is(model, notify_value, options, "notifyLastChance")
    {
        return None;
    }
    let notify_body = block(&notify.consequent)?;
    let [Statement::ExpressionStatement(notify_call)] = notify_body else {
        return None;
    };
    let Some(notify_call) = call(&notify_call.expression) else {
        return None;
    };
    if notify_call.arguments.len() != 1
        || !member(&notify_call.callee, "forEach")
            .is_some_and(|receiver| symbol(model, receiver) == Some(filter))
    {
        return None;
    }
    let (notify_arrow, listener) = arrow_expressions(model, &notify_call.arguments[0])?;
    if notify_arrow.expression {
        return None;
    }
    let [Statement::IfStatement(listener_if)] = notify_arrow.body.statements.as_slice() else {
        return None;
    };
    let Some((listener_left, listener_right)) = binary(&listener_if.test, "!=") else {
        return None;
    };
    if !null_literal(listener_right)
        || listener_if.alternate.is_some()
        || !indexed(listener_left).is_some_and(|(object, key)| {
            symbol(model, key) == Some(listener) && capture(model, site, object).is_some()
        })
    {
        return None;
    }
    let listener_body = block(&listener_if.consequent)?;
    let [Statement::ExpressionStatement(invoke)] = listener_body else {
        return None;
    };
    let Some(invoke) = call(&invoke.expression) else {
        return None;
    };
    let Some(receiver) = member(&invoke.callee, "call") else {
        return None;
    };
    let invoke_args = call_arguments(invoke);
    if invoke.arguments.len() != 1
        || !void_zero(invoke_args[0])
        || !indexed(receiver).is_some_and(|(object, key)| {
            symbol(model, key) == Some(listener) && capture(model, site, object).is_some()
        })
    {
        return None;
    }
    let values_variable = one_variable(values_statement)?;
    let values = plain_binding(model, values_variable, Some(2))?;
    if values_variable.init.is_some() {
        return None;
    }
    let Some((include_and, include_value)) = logical(&include.test, "&&") else {
        return None;
    };
    let Some((include_void, include_option)) = binary(include_and, "!==") else {
        return None;
    };
    if !member_is(model, include_void, options, "includeDebugNames")
        || !void_zero(include_option)
        || !member_is(model, include_value, options, "includeDebugNames")
    {
        return None;
    }
    let (debug_map, plain_map) = (
        block(&include.consequent)?,
        block(include.alternate.as_ref()?)?,
    );
    let [Statement::ExpressionStatement(debug_assign)] = debug_map else {
        return None;
    };
    let debug_assign = assignment_in(debug_assign)?;
    if assigned_symbol(model, &debug_assign.left) != Some(values) {
        return None;
    }
    let Some(debug_map_call) = call(&debug_assign.right) else {
        return None;
    };
    if !member(&debug_map_call.callee, "map")
        .is_some_and(|receiver| symbol(model, receiver) == Some(filter))
        || debug_map_call.arguments.len() != 1
    {
        return None;
    }
    let (debug_arrow, debug_item) = arrow_expressions(model, &debug_map_call.arguments[0])?;
    if debug_arrow.expression {
        return None;
    }
    let [Statement::ReturnStatement(debug_record)] = debug_arrow.body.statements.as_slice() else {
        return None;
    };
    let Expression::ObjectExpression(debug_object) =
        debug_record.argument.as_ref()?.without_parentheses()
    else {
        return None;
    };
    let debug_entries = object_entries(debug_object)?;
    if debug_entries.len() != 2 {
        return None;
    }
    for (key, value) in debug_entries {
        let Some((object, index)) = indexed(value) else {
            return None;
        };
        if symbol(model, index) != Some(debug_item)
            || capture(model, site, object).is_none()
            || !matches!(key.as_str(), "value" | "signal")
        {
            return None;
        }
    }
    let [Statement::ExpressionStatement(plain_assign)] = plain_map else {
        return None;
    };
    let plain_assign = assignment_in(plain_assign)?;
    if assigned_symbol(model, &plain_assign.left) != Some(values) {
        return None;
    }
    let Some(plain_map_call) = call(&plain_assign.right) else {
        return None;
    };
    if !member(&plain_map_call.callee, "map")
        .is_some_and(|receiver| symbol(model, receiver) == Some(filter))
        || plain_map_call.arguments.len() != 1
    {
        return None;
    }
    let (plain_arrow, plain_item) = arrow_expressions(model, &plain_map_call.arguments[0])?;
    if !plain_arrow.expression {
        return None;
    }
    let [Statement::ExpressionStatement(plain_value)] = plain_arrow.body.statements.as_slice()
    else {
        return None;
    };
    let Some((plain_object, plain_index)) = indexed(&plain_value.expression) else {
        return None;
    };
    if symbol(model, plain_index) != Some(plain_item)
        || capture(model, site, plain_object).is_none()
    {
        return None;
    }
    let signals_variable = one_variable(signals_statement)?;
    let signals = plain_binding(model, signals_variable, Some(0))?;
    if !empty_array(signals_variable.init.as_ref()?) {
        return None;
    }
    let positional_variable = one_variable(positional_statement)?;
    let positional = plain_binding(model, positional_variable, Some(0))?;
    if !empty_array(positional_variable.init.as_ref()?) {
        return None;
    }
    let keyed_variable = one_variable(keyed_statement)?;
    let keyed = plain_binding(model, keyed_variable, Some(0))?;
    if !empty_array(keyed_variable.init.as_ref()?) {
        return None;
    }
    let index_variable = one_variable(index_statement)?;
    let index = plain_binding(model, index_variable, Some(1))?;
    if !number(index_variable.init.as_ref()?, 0.0) || scan.init.is_some() {
        return None;
    }
    let Some((bound, length)) = binary(scan.test.as_ref()?, "<") else {
        return None;
    };
    if symbol(model, bound) != Some(index) || !member_is(model, length, values, "length") {
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
    let loop_body = block(&scan.body)?;
    let [
        record_statement,
        sinks_statement,
        Statement::IfStatement(found),
        Statement::ExpressionStatement(call_statement),
    ] = loop_body
    else {
        return None;
    };
    let record_variable = one_variable(record_statement)?;
    let record = plain_binding(model, record_variable, Some(0))?;
    let Some((table, lookup)) = indexed(record_variable.init.as_ref()?) else {
        return None;
    };
    if !indexed(lookup).is_some_and(|(list, item)| {
        symbol(model, list) == Some(filter) && symbol(model, item) == Some(index)
    }) || capture(model, site, table).is_none()
    {
        return None;
    }
    let Statement::VariableDeclaration(declaration) = sinks_statement else {
        return None;
    };
    let [sink_variable, push_variable, item_variable] = declaration.declarations.as_slice() else {
        return None;
    };
    let sink = plain_binding(model, sink_variable, Some(2))?;
    let push = plain_binding(model, push_variable, Some(2))?;
    let item = plain_binding(model, item_variable, Some(2))?;
    if sink_variable.init.is_some() || push_variable.init.is_some() || item_variable.init.is_some()
    {
        return None;
    }
    let Some(found_call) = call(&found.test) else {
        return None;
    };
    if expression_path(model, &found_call.callee)
        != ["Object", "prototype", "hasOwnProperty", "call"]
        || found_call.arguments.len() != 2
    {
        return None;
    }
    let found_args = call_arguments(found_call);
    if symbol(model, found_args[0]) != Some(record) || !string(found_args[1], "name") {
        return None;
    }
    let named = block(&found.consequent)?;
    let [
        Statement::ExpressionStatement(set_sink),
        Statement::ExpressionStatement(set_push),
        Statement::ExpressionStatement(set_item),
    ] = named
    else {
        return None;
    };
    let set_sink = assignment_in(set_sink)?;
    if assigned_symbol(model, &set_sink.left) != Some(sink)
        || symbol(model, &set_sink.right) != Some(keyed)
    {
        return None;
    }
    let set_push = assignment_in(set_push)?;
    if assigned_symbol(model, &set_push.left) != Some(push)
        || !member(&set_push.right, "push")
            .is_some_and(|receiver| symbol(model, receiver) == Some(sink))
    {
        return None;
    }
    let set_item = assignment_in(set_item)?;
    let Expression::ArrayExpression(item_array) = set_item.right.without_parentheses() else {
        return None;
    };
    let [item_name, item_value] = item_array.elements.as_slice() else {
        return None;
    };
    if assigned_symbol(model, &set_item.left) != Some(item)
        || !member(item_name.as_expression()?, "name")
            .is_some_and(|receiver| symbol(model, receiver) == Some(record))
        || !indexed_is(model, item_value.as_expression()?, values, index)
    {
        return None;
    }
    let positional_found = block(found.alternate.as_ref()?)?;
    let [
        Statement::ExpressionStatement(set_push),
        Statement::ExpressionStatement(set_sink),
        Statement::ExpressionStatement(set_item),
    ] = positional_found
    else {
        return None;
    };
    let set_push = assignment_in(set_push)?;
    if assigned_symbol(model, &set_push.left) != Some(push)
        || !member(&set_push.right, "push")
            .is_some_and(|receiver| symbol(model, receiver) == Some(positional))
    {
        return None;
    }
    let set_sink = assignment_in(set_sink)?;
    if assigned_symbol(model, &set_sink.left) != Some(sink)
        || symbol(model, &set_sink.right) != Some(positional)
    {
        return None;
    }
    let set_item = assignment_in(set_item)?;
    if assigned_symbol(model, &set_item.left) != Some(item)
        || !indexed_is(model, &set_item.right, values, index)
    {
        return None;
    }
    let Some(invoke) = call(&call_statement.expression) else {
        return None;
    };
    let Some(invoke_receiver) = member(&invoke.callee, "call") else {
        return None;
    };
    let invoke_args = call_arguments(invoke);
    if invoke.arguments.len() != 2
        || symbol(model, invoke_receiver) != Some(push)
        || symbol(model, invoke_args[0]) != Some(sink)
        || symbol(model, invoke_args[1]) != Some(item)
    {
        return None;
    }
    let Expression::ObjectExpression(report) = returned(end)?.without_parentheses() else {
        return None;
    };
    let entries = object_entries(report)?;
    if entries.len() != 4 {
        return None;
    }
    for (key, value) in entries {
        match key.as_str() {
            "signals" => {
                if symbol(model, value) != Some(signals) {
                    return None;
                }
            }
            "positionalSignals" => {
                if symbol(model, value) != Some(positional) {
                    return None;
                }
            }
            "keyedSignals" => {
                if symbol(model, value) != Some(keyed) {
                    return None;
                }
            }
            "errors" => {
                let Some(to_array) = call(value) else {
                    return None;
                };
                if !to_array.arguments.is_empty()
                    || !member(&to_array.callee, "toArray")
                        .and_then(|e| capture(model, site, e))
                        .is_some()
                {
                    return None;
                }
            }
            _ => return None,
        }
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
    if function.params.items.len() == 1 && signal_report(model, site, function, body).is_some() {
        claims.push(proposal(
            "buildSignalReport",
            "signals_signal_report_builder",
            "builds the exact signal report object from options/filter/debug-name branches: owned signals/positionalSignals/keyedSignals arrays, Object.prototype.hasOwnProperty name routing and a captured errors.toArray() sink",
            f,
        ));
    }
    if claims.is_empty() {
        options_envelope::evaluate(model, site, f)
    } else {
        claims
    }
}
