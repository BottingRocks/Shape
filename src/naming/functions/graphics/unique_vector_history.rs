use super::*;
use oxc_semantic::SymbolId;
use oxc_syntax::operator::LogicalOperator;

use crate::naming::support::selector_graphics::*;

fn member_key(model: &Model<'_, '_>, e: &Expression<'_>, object: SymbolId) -> Option<String> {
    let Expression::StaticMemberExpression(m) = e else {
        return None;
    };
    (!m.optional && symbol(model, &m.object) == Some(object)).then(|| m.property.name.to_string())
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

/// The null guard may be lowered into one local boolean. Own both arms and
/// every write to that local; neither a sibling nor a nested callable can
/// provide its value. Return the exact final length-check branch.
fn array_comparison_guard<'a>(
    model: &Model<'_, '_>,
    callback: &'a FunctionBody<'a>,
    copy: SymbolId,
    stored: SymbolId,
) -> Option<&'a IfStatement<'a>> {
    match callback.statements.as_slice() {
        [Statement::IfStatement(guard)] => {
            let (first, second, _) = or3(&guard.test)?;
            (null_equals(model, first, copy) && null_equals(model, second, stored)).then_some(guard)
        }
        [Statement::VariableDeclaration(declaration), Statement::IfStatement(null_guard), Statement::IfStatement(guard)] =>
        {
            if !plain_variable_kind(declaration.kind) {
                return None;
            }
            let [declaration] = declaration.declarations.as_slice() else {
                return None;
            };
            let BindingPattern::BindingIdentifier(id) = &declaration.id else {
                return None;
            };
            let flag = id.symbol_id.get()?;
            if declaration.init.is_some()
                || model.binding_writes(flag) != 2
                || !model.scoping.symbol_redeclarations(flag).is_empty()
                || !null_equals(model, &null_guard.test, copy)
            {
                return None;
            }
            let [Statement::ExpressionStatement(yes)] = block(&null_guard.consequent)? else {
                return None;
            };
            let [Statement::ExpressionStatement(no)] = block(null_guard.alternate.as_ref()?)?
            else {
                return None;
            };
            let (yes_target, yes_value) = assignment(yes, model)?;
            let (no_target, no_value) = assignment(no, model)?;
            if yes_target != flag
                || no_target != flag
                || !matches!(yes_value, Expression::BooleanLiteral(b) if b.value)
                || !null_equals(model, no_value, stored)
            {
                return None;
            }
            let Expression::LogicalExpression(or) = &guard.test else {
                return None;
            };
            (or.operator.as_str() == "||" && symbol(model, &or.left) == Some(flag)).then_some(guard)
        }
        _ => None,
    }
}

// A lowering materializes the complete null-or-length rejection in one flag.
// Consume both arms, the flag's entire write inventory and its sole test.
fn merged_rejection_flag<'a>(
    model: &Model<'_, '_>, callback: &'a FunctionBody<'a>, copy: SymbolId, stored: SymbolId,
) -> Option<&'a IfStatement<'a>> {
    let [Statement::VariableDeclaration(d), Statement::IfStatement(nulls), Statement::IfStatement(guard)] = callback.statements.as_slice() else { return None; };
    if !plain_variable_kind(d.kind) { return None; }
    let [d] = d.declarations.as_slice() else { return None; };
    let BindingPattern::BindingIdentifier(id) = &d.id else { return None; };
    let flag = id.symbol_id.get()?;
    if d.init.is_some() || model.binding_writes(flag)!=2 || model.binding_references(flag)!=3
        || !model.scoping.symbol_redeclarations(flag).is_empty() || symbol(model,&guard.test)!=Some(flag) { return None; }
    let (a,b)=logical_or(&nulls.test)?;
    if !null_equals(model,a,copy) || !null_equals(model,b,stored) { return None; }
    let [Statement::ExpressionStatement(yes)] = block(&nulls.consequent)? else { return None; };
    let [Statement::ExpressionStatement(no)] = block(nulls.alternate.as_ref()?)? else { return None; };
    let (ys,yv)=assignment(yes,model)?; let (ns,nv)=assignment(no,model)?;
    if ys!=flag || ns!=flag || !matches!(yv,Expression::BooleanLiteral(b) if b.value) { return None; }
    let (a,b)=binary(nv,"!==")?;
    if symbol(model,member(a,"length")?)!=Some(copy) || symbol(model,member(b,"length")?)!=Some(stored) { return None; }
    Some(guard)
}

fn equal_arrays(
    model: &Model<'_, '_>,
    callback: &FunctionBody<'_>,
    copy: SymbolId,
    stored: SymbolId,
) -> Option<()> {
    let guard = if let Some(guard) = merged_rejection_flag(model, callback, copy, stored) { guard } else {
        let guard = array_comparison_guard(model, callback, copy, stored)?;
        let Expression::LogicalExpression(or) = &guard.test else { return None; };
        let (copy_length, stored_length) = binary(&or.right, "!==")?;
        if symbol(model, member(copy_length, "length")?) != Some(copy)
            || symbol(model, member(stored_length, "length")?) != Some(stored) { return None; }
        guard
    };
    let [Statement::ReturnStatement(false_branch)] = block(&guard.consequent)? else {
        return None;
    };
    if !matches!(false_branch.argument.as_ref()?, Expression::BooleanLiteral(b) if !b.value) {
        return None;
    }
    let [Statement::VariableDeclaration(cursor_declaration), Statement::VariableDeclaration(equal_declaration), Statement::ForStatement(loop_statement), Statement::ReturnStatement(result)] =
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
    if loop_statement.init.is_some()
        || loop_statement.test.is_some()
        || loop_statement.update.is_some()
    {
        return None;
    }
    let [Statement::ExpressionStatement(clear), Statement::IfStatement(end), Statement::IfStatement(same), Statement::ExpressionStatement(differ), Statement::BreakStatement(stop)] =
        block(&loop_statement.body)?
    else {
        return None;
    };
    let (cleared, cleared_value) = assignment(clear, model)?;
    if cleared != equal
        || !matches!(cleared_value, Expression::UnaryExpression(u)
        if u.operator.as_str() == "void" && number(&u.argument, 0.0))
    {
        return None;
    }
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
    let (finished, finished_value) = assignment(finish, model)?;
    if finished != equal
        || !matches!(finished_value, Expression::BooleanLiteral(b) if b.value)
        || end_stop.label.is_some()
    {
        return None;
    }
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
    let (differed, differed_value) = assignment(differ, model)?;
    if differed != equal
        || !matches!(differed_value, Expression::BooleanLiteral(b) if !b.value)
        || stop.label.is_some()
    {
        return None;
    }
    (symbol(model, result.argument.as_ref()?) == Some(equal)).then_some(())
}

// ---------------------------------------------------------------------------
// let copy = record;
// if (state.a.some(stored => equalArrays(copy, stored))) return void 0;
// state.b = Number(state.b) + 1;
// if (state.a.length > bound) return void 0;
// else { let saved = Array.prototype.slice.call(copy); state.a.push(saved); return void 0; }
// or  { state.a.push(Array.prototype.slice.call(copy)); return void 0; }
// ---------------------------------------------------------------------------
fn unique_sample(
    model: &Model<'_, '_>,
    body: &FunctionBody<'_>,
    params: &[SymbolId],
) -> Option<()> {
    let [record, state] = params else { return None };
    let [Statement::VariableDeclaration(copy_declaration), Statement::IfStatement(duplicate), Statement::ExpressionStatement(count), Statement::IfStatement(cap)] =
        body.statements.as_slice()
    else {
        return None;
    };
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
    let Argument::ArrowFunctionExpression(predicate) = &some.arguments[0] else {
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
        || expression_path(model, &number_call.callee) != ["Number"]
        || number_call.arguments.len() != 1
    {
        return None;
    }
    let Expression::StaticMemberExpression(slot) = call_arguments(number_call).first()? else {
        return None;
    };
    if slot.optional
        || slot.property.name.as_str() != counter_key
        || symbol(model, &slot.object) != Some(*state)
    {
        return None;
    }
    let [Statement::ReturnStatement(cap_void)] = block(&cap.consequent)? else {
        return None;
    };
    if !matches!(cap_void.argument.as_ref(), Some(e) if is_void_zero(e)) {
        return None;
    }
    unique_sample_cap(
        model,
        block(cap.alternate.as_ref()?)?,
        *state,
        &list_key,
        copy,
    )
}

/// The cap alternate appends the unique-vector copy to the record list. The
/// copy is either hoisted into one write-free plain local
/// (`let saved = slice; list.push(saved); return void 0;`) or inlined directly
/// in the push argument (`list.push(slice); return void 0;`). Both spellings
/// must push into the same record list and end with the same `void 0` return.
fn unique_sample_cap(
    model: &Model<'_, '_>,
    statements: &[Statement<'_>],
    state: SymbolId,
    list_key: &str,
    copy: SymbolId,
) -> Option<()> {
    match statements {
        [Statement::VariableDeclaration(saved_declaration), Statement::ExpressionStatement(save), Statement::ReturnStatement(saved_void)] =>
        {
            if !matches!(saved_void.argument.as_ref(), Some(e) if is_void_zero(e)) {
                return None;
            }
            let [saved_declaration] = saved_declaration.declarations.as_slice() else {
                return None;
            };
            let BindingPattern::BindingIdentifier(saved_id) = &saved_declaration.id else {
                return None;
            };
            let saved = saved_id.symbol_id.get()?;
            if model.binding_writes(saved) != 0
                || !model.scoping.symbol_redeclarations(saved).is_empty()
            {
                return None;
            }
            let Expression::CallExpression(slice) = saved_declaration.init.as_ref()? else {
                return None;
            };
            vector_copy_slice(model, slice, copy)?;
            let Expression::CallExpression(push) = &save.expression else {
                return None;
            };
            let push_list = member(&push.callee, "push")?;
            if push.optional
                || member_key(model, push_list, state)? != list_key
                || push.arguments.len() != 1
                || !matches!(&push.arguments[0], Argument::Identifier(id)
                    if crate::analysis::identifier_symbol(id, model.scoping) == Some(saved))
            {
                return None;
            }
        }
        [Statement::ExpressionStatement(save), Statement::ReturnStatement(inline_void)] => {
            if !matches!(inline_void.argument.as_ref(), Some(e) if is_void_zero(e)) {
                return None;
            }
            let Expression::CallExpression(push) = &save.expression else {
                return None;
            };
            let push_list = member(&push.callee, "push")?;
            if push.optional
                || push.arguments.len() != 1
                || member_key(model, push_list, state)? != list_key
            {
                return None;
            }
            let Argument::CallExpression(slice) = &push.arguments[0] else {
                return None;
            };
            vector_copy_slice(model, slice, copy)?;
        }
        _ => return None,
    }
    Some(())
}

/// `Array.prototype.slice.call(vector)` — a fresh copy of the exact formal
/// vector alias. The alias is already proven write-free and unshadowed by the
/// caller, so the slice argument must resolve to that same symbol.
fn vector_copy_slice(
    model: &Model<'_, '_>,
    slice: &CallExpression<'_>,
    vector: SymbolId,
) -> Option<()> {
    let slice_method = member(&slice.callee, "call")?;
    let prototype = member(slice_method, "slice")?;
    let array = member(prototype, "prototype")?;
    if slice.optional
        || expression_path(model, array) != ["Array"]
        || slice.arguments.len() != 1
        || symbol(model, call_arguments(&slice).first()?) != Some(vector)
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
    if unique_sample(model, body, &params).is_some() {
        claims.push(proposal(
            "recordPixelVectorSample",
            "graphics_unique_vector_history",
            "deduplicates the unchanged formal vector against the exact stored-array slot by null/length guards and element-wise strict identity, increments the same state counter from Number(slot)+1, and appends an Array.prototype.slice copy only while the stored list is within its numeric bound",
            f,
        ));
    }
    claims
}
