//! Pass-8 rolling frontier: a bounded owned-body proof for the shared
//! `recordPixelVectorSample` vector-retention algorithm, in the variant that
//! pushes the sliced copy by value.
//!
//! Recognition uses only scoped symbol identity, exact statement structure and
//! the unshadowed global `Array.prototype.slice` path. Generated spellings,
//! source bodies, byte offsets, content hashes, reference labels and
//! full-body-lookup dictionaries never identify the role. The two formals and
//! every local are plain, single-origin bindings; the stored vector and its
//! counter are members of the second formal, so no captured data origin is
//! claimed. This matches the established canonical family proved by the root
//! `p13_graphics_canvas` finder and the pass-7 catalog; it is the equivalent
//! small variant, not a new domain guess.
use super::*;
use oxc_semantic::SymbolId;
use oxc_syntax::operator::LogicalOperator;

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e, model.scoping)
}

fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = e else {
        return None;
    };
    (!m.optional && m.property.name == key).then_some(&m.object)
}

/// The one static member name of `object`, e.g. `state.<key>`.
fn member_key(model: &Model<'_, '_>, e: &Expression<'_>, object: SymbolId) -> Option<String> {
    let Expression::StaticMemberExpression(m) = e else {
        return None;
    };
    (!m.optional && symbol(model, &m.object) == Some(object)).then(|| m.property.name.to_string())
}

fn global_path(model: &Model<'_, '_>, e: &Expression<'_>, path: &[&str]) -> bool {
    let Some((last, prefix)) = path.split_last() else {
        return false;
    };
    match (e, prefix) {
        (Expression::Identifier(identifier), []) => {
            identifier.name.as_str() == *last
                && crate::analysis::identifier_symbol(identifier, model.scoping).is_none()
        }
        (Expression::StaticMemberExpression(member), _) if !member.optional => {
            member.property.name.as_str() == *last && global_path(model, &member.object, prefix)
        }
        _ => false,
    }
}

fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(b.body.as_slice())
}

fn binary<'a>(e: &'a Expression<'a>, op: &str) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::BinaryExpression(b) = e else {
        return None;
    };
    (b.operator.as_str() == op).then_some((&b.left, &b.right))
}

fn number(e: &Expression<'_>, n: f64) -> bool {
    matches!(e, Expression::NumericLiteral(v) if v.value == n)
}

fn is_void_zero(e: &Expression<'_>) -> bool {
    matches!(e, Expression::UnaryExpression(u)
        if u.operator.as_str() == "void" && number(&u.argument, 0.0))
}

fn logical_or<'a>(e: &'a Expression<'a>) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::LogicalExpression(logical) = e else {
        return None;
    };
    (logical.operator == LogicalOperator::Or).then_some((&logical.left, &logical.right))
}

/// The exact left-nested three-way `a || b || c`, so reorders and repeats are
/// refused.
fn or3<'a>(
    e: &'a Expression<'a>,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>, &'a Expression<'a>)> {
    let (left, right) = logical_or(e)?;
    let (first, second) = logical_or(left)?;
    Some((first, second, right))
}

fn null_equals(model: &Model<'_, '_>, e: &Expression<'_>, s: SymbolId) -> bool {
    matches!(e, Expression::BinaryExpression(b)
        if b.operator.as_str() == "=="
            && symbol(model, &b.left) == Some(s)
            && matches!(&b.right, Expression::NullLiteral(_)))
}

fn increment_target(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<SymbolId> {
    let Expression::UpdateExpression(update) = e else {
        return None;
    };
    if update.operator.as_str() != "++" {
        return None;
    }
    match &update.argument {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => {
            crate::analysis::identifier_symbol(id, model.scoping)
        }
        _ => None,
    }
}

fn plain_parameter_assignment<'a>(
    statement: &'a ExpressionStatement<'a>,
    model: &Model<'_, '_>,
) -> Option<(SymbolId, &'a Expression<'a>)> {
    let Expression::AssignmentExpression(a) = &statement.expression else {
        return None;
    };
    if a.operator.as_str() != "=" {
        return None;
    }
    let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
        return None;
    };
    Some((
        crate::analysis::identifier_symbol(id, model.scoping)?,
        &a.right,
    ))
}

fn plain_parameters(model: &Model<'_, '_>, function: &Function<'_>) -> Option<Vec<SymbolId>> {
    if function.params.rest.is_some() {
        return None;
    }
    function
        .params
        .items
        .iter()
        .map(|p| {
            if p.initializer.is_some() {
                return None;
            }
            let BindingPattern::BindingIdentifier(id) = &p.pattern else {
                return None;
            };
            let s = id.symbol_id.get()?;
            (model.binding_writes(s) == 0 && model.scoping.symbol_redeclarations(s).is_empty())
                .then_some(s)
        })
        .collect()
}

/// The owned equality predicate of the `some` callback:
/// `let equal; if (copy == null || stored == null || copy.length !== stored.length) {
/// equal = false; } else { let cursor = 0; for (;;) { if (cursor >= copy.length) {
/// equal = true; break; } if (copy[cursor] === stored[cursor]) { ++cursor; continue; }
/// equal = false; break; } } return equal;`
///
/// Every compared element, index, length and branch result is an exact scoped
/// identity; the local `equal` and loop `cursor` are plain single-origin
/// bindings and every branch must assign `equal` before the sole return.
fn equal_arrays(
    model: &Model<'_, '_>,
    callback: &FunctionBody<'_>,
    copy: SymbolId,
    stored: SymbolId,
) -> Option<()> {
    let [Statement::VariableDeclaration(equal_declaration), Statement::IfStatement(guard), Statement::ReturnStatement(result)] =
        callback.statements.as_slice()
    else {
        return None;
    };
    // let equal;
    let [equal_declaration] = equal_declaration.declarations.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(equal_id) = &equal_declaration.id else {
        return None;
    };
    let equal = equal_id.symbol_id.get()?;
    if equal_declaration.init.is_some() || !model.scoping.symbol_redeclarations(equal).is_empty() {
        return None;
    }
    // if (copy == null || stored == null || copy.length !== stored.length) { equal = false; }
    let (first, second, third) = or3(&guard.test)?;
    if !null_equals(model, first, copy) || !null_equals(model, second, stored) {
        return None;
    }
    let (copy_length, stored_length) = binary(third, "!==")?;
    if symbol(model, member(copy_length, "length")?) != Some(copy)
        || symbol(model, member(stored_length, "length")?) != Some(stored)
    {
        return None;
    }
    let [Statement::ExpressionStatement(fallthrough)] = block(&guard.consequent)? else {
        return None;
    };
    let (cleared, cleared_value) = plain_parameter_assignment(fallthrough, model)?;
    if cleared != equal || !matches!(cleared_value, Expression::BooleanLiteral(b) if !b.value) {
        return None;
    }
    // else { let cursor = 0; for (;;) { ... } }
    let [Statement::VariableDeclaration(cursor_declaration), Statement::ForStatement(loop_statement)] =
        block(guard.alternate.as_ref()?)?
    else {
        return None;
    };
    let [cursor_declaration] = cursor_declaration.declarations.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(cursor_id) = &cursor_declaration.id else {
        return None;
    };
    let cursor = cursor_id.symbol_id.get()?;
    if !number(cursor_declaration.init.as_ref()?, 0.0)
        || model.binding_writes(cursor) != 1
        || !model.scoping.symbol_redeclarations(cursor).is_empty()
    {
        return None;
    }
    if loop_statement.init.is_some()
        || loop_statement.test.is_some()
        || loop_statement.update.is_some()
    {
        return None;
    }
    let [Statement::IfStatement(end), Statement::IfStatement(same), Statement::ExpressionStatement(differ), Statement::BreakStatement(stop)] =
        block(&loop_statement.body)?
    else {
        return None;
    };
    // if (cursor >= copy.length) { equal = true; break; }
    let (index, length) = binary(&end.test, ">=")?;
    if symbol(model, index) != Some(cursor)
        || symbol(model, member(length, "length")?) != Some(copy)
        || end.alternate.is_some()
    {
        return None;
    }
    let [Statement::ExpressionStatement(finish), Statement::BreakStatement(end_stop)] =
        block(&end.consequent)?
    else {
        return None;
    };
    let (finished, finished_value) = plain_parameter_assignment(finish, model)?;
    if finished != equal
        || !matches!(finished_value, Expression::BooleanLiteral(b) if b.value)
        || end_stop.label.is_some()
    {
        return None;
    }
    // if (copy[cursor] === stored[cursor]) { ++cursor; continue; }
    let (index_a, index_b) = binary(&same.test, "===")?;
    let Expression::ComputedMemberExpression(index_a) = index_a else {
        return None;
    };
    let Expression::ComputedMemberExpression(index_b) = index_b else {
        return None;
    };
    if index_a.optional
        || index_b.optional
        || symbol(model, &index_a.object) != Some(copy)
        || symbol(model, &index_b.object) != Some(stored)
        || symbol(model, &index_a.expression) != Some(cursor)
        || symbol(model, &index_b.expression) != Some(cursor)
        || same.alternate.is_some()
    {
        return None;
    }
    let [Statement::ExpressionStatement(step), Statement::ContinueStatement(again)] =
        block(&same.consequent)?
    else {
        return None;
    };
    if increment_target(model, &step.expression) != Some(cursor) || again.label.is_some() {
        return None;
    }
    // equal = false; break;
    let (differed, differed_value) = plain_parameter_assignment(differ, model)?;
    if differed != equal
        || !matches!(differed_value, Expression::BooleanLiteral(b) if !b.value)
        || stop.label.is_some()
    {
        return None;
    }
    (symbol(model, result.argument.as_ref()?) == Some(equal)).then_some(())
}

/// `let copy = record; if (state.<list>.some(stored => equalArrays(copy, stored)))
/// return void 0; state.<counter> = Number(state.<counter>) + 1;
/// if (state.<list>.length > bound) return void 0;
/// else { state.<list>.push(Array.prototype.slice.call(copy)); return void 0; }`
///
/// The list member and counter member are distinct members of the same second
/// formal; the bound is a positive numeric literal; the appended value is
/// exactly an `Array.prototype.slice` copy of the unchanged first formal.
fn record_pixel_vector_sample(
    model: &Model<'_, '_>,
    body: &FunctionBody<'_>,
    params: &[SymbolId],
) -> Option<()> {
    let [record, state] = params else {
        return None;
    };
    let [Statement::VariableDeclaration(copy_declaration), Statement::IfStatement(duplicate), Statement::ExpressionStatement(count), Statement::IfStatement(cap)] =
        body.statements.as_slice()
    else {
        return None;
    };
    // let copy = record;
    let [copy_declaration] = copy_declaration.declarations.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(copy_id) = &copy_declaration.id else {
        return None;
    };
    let copy = copy_id.symbol_id.get()?;
    if model.binding_writes(copy) != 0
        || !model.scoping.symbol_redeclarations(copy).is_empty()
        || symbol(model, copy_declaration.init.as_ref()?) != Some(*record)
    {
        return None;
    }
    // if (state.<list>.some(stored => equalArrays(copy, stored))) return void 0;
    if duplicate.alternate.is_some() {
        return None;
    }
    let Expression::CallExpression(some) = &duplicate.test else {
        return None;
    };
    if some.optional || some.arguments.len() != 1 {
        return None;
    }
    let list = member(&some.callee, "some")?;
    let list_key = member_key(model, list, *state)?;
    let [predicate_argument] = some.arguments.as_slice() else {
        return None;
    };
    let Some(Expression::ArrowFunctionExpression(predicate)) = predicate_argument.as_expression()
    else {
        return None;
    };
    if predicate.r#async
        || predicate.expression
        || predicate.params.rest.is_some()
        || !predicate.body.directives.is_empty()
    {
        return None;
    }
    let [parameter] = predicate.params.items.as_slice() else {
        return None;
    };
    if parameter.initializer.is_some() {
        return None;
    }
    let BindingPattern::BindingIdentifier(stored_id) = &parameter.pattern else {
        return None;
    };
    let stored = stored_id.symbol_id.get()?;
    if model.binding_writes(stored) != 0 || !model.scoping.symbol_redeclarations(stored).is_empty()
    {
        return None;
    }
    equal_arrays(model, &predicate.body, copy, stored)?;
    let [Statement::ReturnStatement(duplicate_void)] = block(&duplicate.consequent)? else {
        return None;
    };
    if !matches!(duplicate_void.argument.as_ref(), Some(e) if is_void_zero(e)) {
        return None;
    }
    // state.<counter> = Number(state.<counter>) + 1;  with a counter member
    // distinct from the list member.
    let Expression::AssignmentExpression(counter) = &count.expression else {
        return None;
    };
    let AssignmentTarget::StaticMemberExpression(target) = &counter.left else {
        return None;
    };
    if counter.operator.as_str() != "="
        || target.optional
        || symbol(model, &target.object) != Some(*state)
    {
        return None;
    }
    let counter_key = target.property.name.to_string();
    if counter_key == list_key {
        return None;
    }
    let Expression::BinaryExpression(sum) = &counter.right else {
        return None;
    };
    if sum.operator.as_str() != "+" || !number(&sum.right, 1.0) {
        return None;
    }
    let Expression::CallExpression(number_call) = &sum.left else {
        return None;
    };
    if number_call.optional
        || !global_path(model, &number_call.callee, &["Number"])
        || number_call.arguments.len() != 1
    {
        return None;
    }
    let [slot_argument] = number_call.arguments.as_slice() else {
        return None;
    };
    let Some(Expression::StaticMemberExpression(slot)) = slot_argument.as_expression() else {
        return None;
    };
    if slot.optional
        || slot.property.name.as_str() != counter_key
        || symbol(model, &slot.object) != Some(*state)
    {
        return None;
    }
    // if (state.<list>.length > bound) return void 0;
    if cap.alternate.is_none() {
        return None;
    }
    let (length, bound) = binary(&cap.test, ">")?;
    let cap_list = member(length, "length")?;
    if member_key(model, cap_list, *state)? != list_key
        || !matches!(bound, Expression::NumericLiteral(n) if n.value > 0.0)
    {
        return None;
    }
    let [Statement::ReturnStatement(cap_void)] = block(&cap.consequent)? else {
        return None;
    };
    if !matches!(cap_void.argument.as_ref(), Some(e) if is_void_zero(e)) {
        return None;
    }
    // else { state.<list>.push(Array.prototype.slice.call(copy)); return void 0; }
    let [Statement::ExpressionStatement(save), Statement::ReturnStatement(saved_void)] =
        block(cap.alternate.as_ref()?)?
    else {
        return None;
    };
    if !matches!(saved_void.argument.as_ref(), Some(e) if is_void_zero(e)) {
        return None;
    }
    let Expression::CallExpression(push) = &save.expression else {
        return None;
    };
    let push_list = member(&push.callee, "push")?;
    if push.optional
        || member_key(model, push_list, *state)? != list_key
        || push.arguments.len() != 1
    {
        return None;
    }
    let [slice_argument] = push.arguments.as_slice() else {
        return None;
    };
    let Some(Expression::CallExpression(slice)) = slice_argument.as_expression() else {
        return None;
    };
    let slice_method = member(&slice.callee, "call")?;
    let prototype = member(slice_method, "slice")?;
    let array = member(prototype, "prototype")?;
    let [copy_argument] = slice.arguments.as_slice() else {
        return None;
    };
    if slice.optional
        || !global_path(model, array, &["Array"])
        || symbol(model, copy_argument.as_expression()?) != Some(copy)
    {
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
    let Some(params) = plain_parameters(model, function) else {
        return Vec::new();
    };
    let mut claims = Vec::new();
    if record_pixel_vector_sample(model, body, &params).is_some() {
        claims.push(proposal(
            "recordPixelVectorSample",
            "shared_frontier_unique_sample",
            "deduplicates the unchanged slot-0 formal vector against the exact list member of the slot-1 state formal by null/length guards and element-wise strict identity, increments a distinct counter member of the same state from Number(counter)+1, and appends an Array.prototype.slice copy through the same list member only while its length is within a positive literal bound; every identity is a plain, single-origin binding",
            f,
        ));
    }
    claims
}
