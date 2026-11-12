use super::*;
use oxc_semantic::SymbolId;
use oxc_syntax::operator::UpdateOperator;

use crate::naming::support::reassembly::*;

fn statement_call<'a>(s: &'a Statement<'a>) -> Option<&'a CallExpression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    call(&s.expression)
}

// A non-optional static member `state.<name>`.
fn state_prop<'a>(
    model: &Model<'_, '_>,
    e: &'a Expression<'a>,
    state: SymbolId,
) -> Option<(&'a Expression<'a>, &'a str)> {
    let (object, name) = member_parts(e)?;
    (symbol(model, object) == Some(state)).then_some((object, name))
}

fn state_target_prop<'a>(
    model: &Model<'_, '_>,
    target: &'a AssignmentTarget<'a>,
    state: SymbolId,
) -> Option<&'a str> {
    let AssignmentTarget::StaticMemberExpression(m) = target else {
        return None;
    };
    (!m.optional && symbol(model, &m.object) == Some(state)).then_some(m.property.name.as_str())
}

fn void_zero(e: &Expression<'_>) -> bool {
    matches!(e, Expression::UnaryExpression(u)
        if u.operator.as_str() == "void"
            && matches!(&u.argument, Expression::NumericLiteral(n) if n.value == 0.0))
}

fn void_return(s: &Statement<'_>) -> bool {
    matches!(s, Statement::ReturnStatement(r) if r.argument.as_ref().is_some_and(void_zero))
}

fn length_of(model: &Model<'_, '_>, e: &Expression<'_>, owner: SymbolId) -> bool {
    member_parts(e)
        .is_some_and(|(object, name)| name == "length" && symbol(model, object) == Some(owner))
}

fn indexed_by(model: &Model<'_, '_>, e: &Expression<'_>, owner: SymbolId, index: SymbolId) -> bool {
    matches!(e, Expression::ComputedMemberExpression(m)
        if !m.optional
            && symbol(model, &m.object) == Some(owner)
            && symbol(model, &m.expression) == Some(index))
}

fn null_comparison(model: &Model<'_, '_>, e: &Expression<'_>, owner: SymbolId) -> bool {
    matches!(e, Expression::BinaryExpression(b)
        if b.operator.as_str() == "=="
            && symbol(model, &b.left) == Some(owner)
            && matches!(&b.right, Expression::NullLiteral(_)))
}

fn boolean_assignment(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    target: SymbolId,
    value: bool,
) -> bool {
    let Expression::AssignmentExpression(a) = e else {
        return false;
    };
    a.operator.as_str() == "="
        && matches!(&a.left, AssignmentTarget::AssignmentTargetIdentifier(i)
            if crate::analysis::identifier_symbol(i, model.scoping) == Some(target))
        && matches!(&a.right, Expression::BooleanLiteral(b) if b.value == value)
}

fn logical_or_parts<'a>(e: &'a Expression<'a>, out: &mut Vec<&'a Expression<'a>>) {
    if let Expression::LogicalExpression(l) = e {
        if l.operator.as_str() == "||" {
            logical_or_parts(&l.left, out);
            logical_or_parts(&l.right, out);
            return;
        }
    }
    out.push(e);
}

// Bounded distinct-vector sampler. Two plain formals, an immutable alias of the
// record, an owned duplicate equality callback, one Number counter bump, a
// positive list-length cap and one slice snapshot appended through the same
// list property. Every property spelling is free but must stay consistent.
fn distinct_vector_sample(
    model: &Model<'_, '_>,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [p0, p1] = function.params.items.as_slice() else {
        return None;
    };
    if function.params.rest.is_some() || p0.initializer.is_some() || p1.initializer.is_some() {
        return None;
    }
    let record = immutable(model, &p0.pattern)?;
    let state = immutable(model, &p1.pattern)?;
    if record == state {
        return None;
    }
    let [alias_stmt, duplicate, counter_stmt, cap] = body.statements.as_slice() else {
        return None;
    };
    let alias_variable = one_variable(alias_stmt)?;
    let alias = plain_local(model, alias_variable)?;
    if symbol(model, alias_variable.init.as_ref()?) != Some(record)
        || model.binding_writes(alias) != 0
    {
        return None;
    }

    // Duplicate guard: if (state.<list>.some(stored => equality(stored))) return void 0;
    let Statement::IfStatement(duplicate_if) = duplicate else {
        return None;
    };
    if duplicate_if.alternate.is_some() {
        return None;
    }
    let some = call(&duplicate_if.test)?;
    let (list_expression, method) = member_parts(&some.callee)?;
    if method != "some" || some.arguments.len() != 1 {
        return None;
    }
    let list = state_prop(model, list_expression, state)?.1.to_string();
    let [Argument::ArrowFunctionExpression(callback)] = some.arguments.as_slice() else {
        return None;
    };
    let [stored] = callback.params.items.as_slice() else {
        return None;
    };
    if callback.r#async
        || callback.expression
        || callback.params.rest.is_some()
        || stored.initializer.is_some()
        || !callback.body.directives.is_empty()
    {
        return None;
    }
    let stored = immutable(model, &stored.pattern)?;
    if stored == record || stored == state {
        return None;
    }
    let [result_stmt, guard, result_return] = callback.body.statements.as_slice() else {
        return None;
    };
    let result_variable = one_variable(result_stmt)?;
    let result = plain_local(model, result_variable)?;
    if result_variable.init.is_some() {
        return None;
    }
    let Statement::IfStatement(guard_if) = guard else {
        return None;
    };
    let Statement::ReturnStatement(result_return) = result_return else {
        return None;
    };
    if symbol(model, result_return.argument.as_ref()?) != Some(result) {
        return None;
    }
    let mut guard_parts = Vec::new();
    logical_or_parts(&guard_if.test, &mut guard_parts);
    let [first_null, second_null, length_test] = guard_parts.as_slice() else {
        return None;
    };
    if !null_comparison(model, first_null, alias) || !null_comparison(model, second_null, stored) {
        return None;
    }
    let Expression::BinaryExpression(length_binary) = length_test else {
        return None;
    };
    if length_binary.operator.as_str() != "!=="
        || !length_of(model, &length_binary.left, alias)
        || !length_of(model, &length_binary.right, stored)
    {
        return None;
    }
    let Statement::BlockStatement(reject_block) = &guard_if.consequent else {
        return None;
    };
    let [Statement::ExpressionStatement(reject)] = reject_block.body.as_slice() else {
        return None;
    };
    if !boolean_assignment(model, &reject.expression, result, false) {
        return None;
    }
    let Statement::BlockStatement(scan) = guard_if.alternate.as_ref()? else {
        return None;
    };
    let [index_stmt, scan_loop] = scan.body.as_slice() else {
        return None;
    };
    let index_variable = one_variable(index_stmt)?;
    let index = plain_local(model, index_variable)?;
    if !number(index_variable.init.as_ref()?, 0.0) {
        return None;
    }
    if index == result || index == stored || index == alias {
        return None;
    }
    let Statement::ForStatement(for_) = scan_loop else {
        return None;
    };
    if for_.init.is_some() || for_.test.is_some() || for_.update.is_some() {
        return None;
    }
    let Statement::BlockStatement(scan_body) = &for_.body else {
        return None;
    };
    let [end_if, equal_if, reject_scan, scan_break] = scan_body.body.as_slice() else {
        return None;
    };

    // if (index >= alias.length) { result = true; break; }
    let Statement::IfStatement(end_if) = end_if else {
        return None;
    };
    if end_if.alternate.is_some() {
        return None;
    }
    let Expression::BinaryExpression(end) = &end_if.test else {
        return None;
    };
    if end.operator.as_str() != ">="
        || symbol(model, &end.left) != Some(index)
        || !length_of(model, &end.right, alias)
    {
        return None;
    }
    let Statement::BlockStatement(end_block) = &end_if.consequent else {
        return None;
    };
    let [Statement::ExpressionStatement(end_true), Statement::BreakStatement(end_break)] =
        end_block.body.as_slice()
    else {
        return None;
    };
    if end_break.label.is_some() || !boolean_assignment(model, &end_true.expression, result, true) {
        return None;
    }

    // if (alias[index] === stored[index]) { ++index; continue; }
    let Statement::IfStatement(equal_if) = equal_if else {
        return None;
    };
    if equal_if.alternate.is_some() {
        return None;
    }
    let Expression::BinaryExpression(equal) = &equal_if.test else {
        return None;
    };
    if equal.operator.as_str() != "==="
        || !indexed_by(model, &equal.left, alias, index)
        || !indexed_by(model, &equal.right, stored, index)
    {
        return None;
    }
    let Statement::BlockStatement(equal_block) = &equal_if.consequent else {
        return None;
    };
    let [Statement::ExpressionStatement(increment), Statement::ContinueStatement(continue_)] =
        equal_block.body.as_slice()
    else {
        return None;
    };
    if continue_.label.is_some()
        || !matches!(&increment.expression, Expression::UpdateExpression(u)
            if u.operator == UpdateOperator::Increment
                && u.prefix
                && update_symbol(model, &u.argument) == Some(index))
    {
        return None;
    }

    // result = false; break;
    let Statement::ExpressionStatement(reject_scan) = reject_scan else {
        return None;
    };
    let Statement::BreakStatement(scan_break) = scan_break else {
        return None;
    };
    if scan_break.label.is_some()
        || !boolean_assignment(model, &reject_scan.expression, result, false)
    {
        return None;
    }

    // Duplicate branch returns exactly void 0.
    let Statement::BlockStatement(duplicate_block) = &duplicate_if.consequent else {
        return None;
    };
    if duplicate_block.body.len() != 1 || !void_return(&duplicate_block.body[0]) {
        return None;
    }

    // Counter: state.<counter> = Number(state.<counter>) + 1
    let Statement::ExpressionStatement(counter_stmt) = counter_stmt else {
        return None;
    };
    let Expression::AssignmentExpression(counter) = &counter_stmt.expression else {
        return None;
    };
    if counter.operator.as_str() != "=" {
        return None;
    }
    let counter_name = state_target_prop(model, &counter.left, state)?.to_string();
    if counter_name == list {
        return None;
    }
    let Expression::BinaryExpression(bump) = &counter.right else {
        return None;
    };
    if bump.operator.as_str() != "+" || !number(&bump.right, 1.0) {
        return None;
    }
    let number_call = call(&bump.left)?;
    if expression_path(model, &number_call.callee) != ["Number"]
        || number_call.arguments.len() != 1
        || state_prop(model, number_call.arguments[0].as_expression()?, state)?.1
            != counter_name.as_str()
    {
        return None;
    }

    // Cap: if (state.<list>.length > positive) return void 0; else retain.
    let Statement::IfStatement(cap_if) = cap else {
        return None;
    };
    let Expression::BinaryExpression(bound) = &cap_if.test else {
        return None;
    };
    if bound.operator.as_str() != ">" {
        return None;
    }
    let (list_length, length_name) = member_parts(&bound.left)?;
    if length_name != "length" || state_prop(model, list_length, state)?.1 != list.as_str() {
        return None;
    }
    if !matches!(&bound.right, Expression::NumericLiteral(n) if n.value.is_finite() && n.value > 0.0)
    {
        return None;
    }
    let Statement::BlockStatement(cap_block) = &cap_if.consequent else {
        return None;
    };
    if cap_block.body.len() != 1 || !void_return(&cap_block.body[0]) {
        return None;
    }
    let Statement::BlockStatement(retain) = cap_if.alternate.as_ref()? else {
        return None;
    };
    let [copy_stmt, push_stmt, retain_return] = retain.body.as_slice() else {
        return None;
    };
    let copy_variable = one_variable(copy_stmt)?;
    let copy = plain_local(model, copy_variable)?;
    if model.binding_writes(copy) != 0 || copy == alias || copy == record || copy == state {
        return None;
    }
    let slice_call = call(copy_variable.init.as_ref()?)?;
    let (slice, call_name) = member_parts(&slice_call.callee)?;
    if call_name != "call"
        || expression_path(model, slice) != ["Array", "prototype", "slice"]
        || slice_call.arguments.len() != 1
        || symbol(model, slice_call.arguments[0].as_expression()?) != Some(alias)
    {
        return None;
    }
    let push = statement_call(push_stmt)?;
    let (push_list, push_name) = member_parts(&push.callee)?;
    if push_name != "push"
        || push.arguments.len() != 1
        || state_prop(model, push_list, state)?.1 != list.as_str()
        || symbol(model, push.arguments[0].as_expression()?) != Some(copy)
    {
        return None;
    }
    if !void_return(retain_return) {
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
    if distinct_vector_sample(model, function, body).is_some() {
        claims.push(proposal(
            "recordDistinctVectorSample",
            "graphics_distinct_vector_copy",
            "two plain formals; an owned equality callback suppresses an already stored vector, the same state counter is bumped through Number, and only below a positive list-length cap is a sliced copy appended through the same list property; equality operators, loop cursor, property identities and return values are proved, with property spellings free but consistent",
            f,
        ));
    }
    claims
}
