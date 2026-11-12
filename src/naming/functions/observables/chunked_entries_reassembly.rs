use super::*;
use crate::signals::extract::bindings::array_elements;
use oxc_semantic::SymbolId;
use oxc_syntax::operator::UpdateOperator;

use crate::naming::support::reassembly::*;

// `prefix + key` -> (prefix, key).
fn prefixed_key<'a>(model: &Model<'_, '_>, e: &'a Expression<'a>) -> Option<(String, SymbolId)> {
    let Expression::BinaryExpression(add) = e else {
        return None;
    };
    if add.operator.as_str() != "+" {
        return None;
    }
    let Expression::StringLiteral(prefix) = &add.left else {
        return None;
    };
    let key = symbol(model, &add.right)?;
    Some((prefix.value.to_string(), key))
}

// `table[prefix + key]` -> (prefix, key).
fn base_key<'a>(
    model: &Model<'_, '_>,
    e: &'a Expression<'a>,
    table: SymbolId,
) -> Option<(String, SymbolId)> {
    let Expression::ComputedMemberExpression(m) = e else {
        return None;
    };
    if m.optional || symbol(model, &m.object) != Some(table) {
        return None;
    }
    prefixed_key(model, &m.expression)
}

// `table[prefix + key + index]` -> (prefix, key, index).
fn continuation_key<'a>(
    model: &Model<'_, '_>,
    e: &'a Expression<'a>,
    table: SymbolId,
) -> Option<(String, SymbolId, SymbolId)> {
    let Expression::ComputedMemberExpression(m) = e else {
        return None;
    };
    if m.optional || symbol(model, &m.object) != Some(table) {
        return None;
    }
    let Expression::BinaryExpression(add) = &m.expression else {
        return None;
    };
    if add.operator.as_str() != "+" {
        return None;
    }
    let index = symbol(model, &add.right)?;
    let (prefix, key) = prefixed_key(model, &add.left)?;
    Some((prefix, key, index))
}

fn arrow_value<'a>(arrow: &'a ArrowFunctionExpression<'a>) -> Option<&'a Expression<'a>> {
    match arrow.body.statements.as_slice() {
        [Statement::ReturnStatement(r)] => r.argument.as_ref(),
        [Statement::ExpressionStatement(s)] if arrow.expression => Some(&s.expression),
        _ => None,
    }
}

// Chunked-entry reassembly: Object.fromEntries(list.filter(present).map(read))
// where read concatenates table[prefix+key] with table[prefix+key+i] until the
// continuation chunk is missing, then emits [prefix+key, value].
fn reassemble_chunked_entries(
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
    let table = immutable(model, &p0.pattern)?;
    let keys = immutable(model, &p1.pattern)?;
    if table == keys {
        return None;
    }
    let [alias_stmt, result] = body.statements.as_slice() else {
        return None;
    };
    let alias_variable = one_variable(alias_stmt)?;
    let alias = plain_local(model, alias_variable)?;
    if symbol(model, alias_variable.init.as_ref()?) != Some(table)
        || model.binding_writes(alias) != 0
    {
        return None;
    }
    let Statement::ReturnStatement(result) = result else {
        return None;
    };
    let entries = call(result.argument.as_ref()?)?;
    if expression_path(model, &entries.callee) != ["Object", "fromEntries"]
        || entries.arguments.len() != 1
    {
        return None;
    }
    let mapped = call(entries.arguments[0].as_expression()?)?;
    let (filter_call, map_method) = member_parts(&mapped.callee)?;
    if map_method != "map" || mapped.arguments.len() != 1 {
        return None;
    }
    let filter = call(filter_call)?;
    let (keys_expression, filter_method) = member_parts(&filter.callee)?;
    if filter_method != "filter"
        || symbol(model, keys_expression) != Some(keys)
        || filter.arguments.len() != 1
    {
        return None;
    }

    // filter callback: key => table[prefix + key] != null
    let [Argument::ArrowFunctionExpression(predicate)] = filter.arguments.as_slice() else {
        return None;
    };
    let [predicate_param] = predicate.params.items.as_slice() else {
        return None;
    };
    if predicate.r#async
        || !predicate.expression
        || predicate.params.rest.is_some()
        || predicate_param.initializer.is_some()
        || !predicate.body.directives.is_empty()
    {
        return None;
    }
    let filter_key = immutable(model, &predicate_param.pattern)?;
    let predicate_value = arrow_value(predicate)?;
    let Expression::BinaryExpression(present) = predicate_value else {
        return None;
    };
    if present.operator.as_str() != "!=" || !matches!(&present.right, Expression::NullLiteral(_)) {
        return None;
    }
    let (prefix, key) = base_key(model, &present.left, alias)?;
    if key != filter_key {
        return None;
    }

    // map callback: key => { let index = 0; let value = table[prefix+key];
    //   for (; table[prefix+key+index] != null; index++) value = value + table[...];
    //   value = [prefix+key, value]; return value; }
    let [Argument::ArrowFunctionExpression(mapper)] = mapped.arguments.as_slice() else {
        return None;
    };
    let [mapper_param] = mapper.params.items.as_slice() else {
        return None;
    };
    if mapper.r#async
        || mapper.expression
        || mapper.params.rest.is_some()
        || mapper_param.initializer.is_some()
        || !mapper.body.directives.is_empty()
    {
        return None;
    }
    let map_key = immutable(model, &mapper_param.pattern)?;
    if map_key == filter_key || map_key == table || map_key == keys {
        return None;
    }
    let [index_stmt, value_stmt, loop_stmt, entry_stmt, return_stmt] =
        mapper.body.statements.as_slice()
    else {
        return None;
    };
    let index_variable = one_variable(index_stmt)?;
    let index = plain_local(model, index_variable)?;
    if !number(index_variable.init.as_ref()?, 0.0)
        || index == map_key
        || index == filter_key
        || index == alias
    {
        return None;
    }
    let value_variable = one_variable(value_stmt)?;
    let value = plain_local(model, value_variable)?;
    if value == index || value == map_key || value == alias {
        return None;
    }
    let (value_prefix, value_key) = base_key(model, value_variable.init.as_ref()?, alias)?;
    if value_prefix != prefix || value_key != map_key {
        return None;
    }

    let Statement::ForStatement(loop_stmt) = loop_stmt else {
        return None;
    };
    if loop_stmt.init.is_some() {
        return None;
    }
    let Some(loop_test) = loop_stmt.test.as_ref() else {
        return None;
    };
    let Expression::BinaryExpression(continuation) = loop_test else {
        return None;
    };
    if continuation.operator.as_str() != "!="
        || !matches!(&continuation.right, Expression::NullLiteral(_))
    {
        return None;
    }
    let (loop_prefix, loop_key, loop_index) = continuation_key(model, &continuation.left, alias)?;
    if loop_prefix != prefix || loop_key != map_key || loop_index != index {
        return None;
    }
    let Some(loop_update) = loop_stmt.update.as_ref() else {
        return None;
    };
    if !matches!(loop_update, Expression::UpdateExpression(u)
        if u.operator == UpdateOperator::Increment
            && !u.prefix
            && update_symbol(model, &u.argument) == Some(index))
    {
        return None;
    }
    let Statement::BlockStatement(loop_block) = &loop_stmt.body else {
        return None;
    };
    let [Statement::ExpressionStatement(append)] = loop_block.body.as_slice() else {
        return None;
    };
    let Expression::AssignmentExpression(append) = &append.expression else {
        return None;
    };
    if append.operator.as_str() != "="
        || !matches!(&append.left, AssignmentTarget::AssignmentTargetIdentifier(i)
            if crate::analysis::identifier_symbol(i, model.scoping) == Some(value))
    {
        return None;
    }
    let Expression::BinaryExpression(concat) = &append.right else {
        return None;
    };
    if concat.operator.as_str() != "+" || symbol(model, &concat.left) != Some(value) {
        return None;
    }
    let (append_prefix, append_key, append_index) = continuation_key(model, &concat.right, alias)?;
    if append_prefix != prefix || append_key != map_key || append_index != index {
        return None;
    }

    let Statement::ExpressionStatement(entry_stmt) = entry_stmt else {
        return None;
    };
    let Expression::AssignmentExpression(entry) = &entry_stmt.expression else {
        return None;
    };
    if entry.operator.as_str() != "="
        || !matches!(&entry.left, AssignmentTarget::AssignmentTargetIdentifier(i)
            if crate::analysis::identifier_symbol(i, model.scoping) == Some(value))
    {
        return None;
    }
    let elements = array_elements(&entry.right)?;
    let [base, accumulated] = elements.as_slice() else {
        return None;
    };
    let (entry_prefix, entry_key) = prefixed_key(model, base)?;
    if entry_prefix != prefix || entry_key != map_key || symbol(model, accumulated) != Some(value) {
        return None;
    }
    let Statement::ReturnStatement(return_stmt) = return_stmt else {
        return None;
    };
    if symbol(model, return_stmt.argument.as_ref()?) != Some(value) {
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
    if reassemble_chunked_entries(model, function, body).is_some() {
        claims.push(proposal(
            "reassembleChunkedEntries",
            "signals_chunked_entries_reassembly",
            "two plain formals; Object.fromEntries filters the key list to entries whose prefix+key lookup exists, then concatenates the base value with indexed continuation chunks until the next lookup is null and returns [prefix+key, value]; the same prefix literal and key expression are structurally required at every occurrence without using its bytes as a role signal",
            f,
        ));
    }
    claims
}
