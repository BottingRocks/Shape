//! Small rotation rules for complete, source-backed helper protocols.
//!
//! This module deliberately identifies a generic key/value mapper rather than
//! inferring a browser API, vendor, generated identifier, or data domain.
use super::*;

type Symbol = oxc_semantic::SymbolId;

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(e, model.scoping)
}

fn plain_unchanged(model: &Model<'_, '_>, pattern: &BindingPattern<'_>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(id) = pattern else {
        return None;
    };
    let symbol = id.symbol_id.get()?;
    (model.binding_writes(symbol) == 0 && model.scoping.symbol_redeclarations(symbol).is_empty())
        .then_some(symbol)
}

fn static_member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(member) = e else {
        return None;
    };
    (!member.optional && member.property.name == key).then_some(&member.object)
}

fn call<'a>(e: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(call) = e else {
        return None;
    };
    (!call.optional).then_some(call)
}

fn statement_expression<'a>(statement: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ExpressionStatement(statement) = statement else {
        return None;
    };
    Some(&statement.expression)
}

fn one_declarator<'a>(statement: &'a Statement<'a>) -> Option<&'a VariableDeclarator<'a>> {
    let Statement::VariableDeclaration(declaration) = statement else {
        return None;
    };
    let [declarator] = declaration.declarations.as_slice() else {
        return None;
    };
    Some(declarator)
}

fn returned<'a>(statement: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ReturnStatement(statement) = statement else {
        return None;
    };
    statement.argument.as_ref()
}

fn map_keys_to_object(
    model: &Model<'_, '_>,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if function.r#async
        || function.generator
        || !body.directives.is_empty()
        || function.params.rest.is_some()
    {
        return None;
    }
    let [keys_param, values_param] = function.params.items.as_slice() else {
        return None;
    };
    if keys_param.initializer.is_some() || values_param.initializer.is_some() {
        return None;
    }
    let keys = plain_unchanged(model, &keys_param.pattern)?;
    let values = plain_unchanged(model, &values_param.pattern)?;
    let [source_alias, result, iteration, end] = body.statements.as_slice() else {
        return None;
    };

    let source_alias = one_declarator(source_alias)?;
    let source = plain_unchanged(model, &source_alias.id)?;
    if source_alias.init.as_ref().and_then(|e| symbol(model, e)) != Some(values) {
        return None;
    }

    let result = one_declarator(result)?;
    let result_symbol = plain_unchanged(model, &result.id)?;
    let create = call(result.init.as_ref()?)?;
    if create.arguments.len() != 1
        || !matches!(
            call_arguments(create).as_slice(),
            [Expression::NullLiteral(_)]
        )
        || static_member(&create.callee, "create")
            .and_then(|object| static_member(object, "Object"))
            .is_none()
    {
        return None;
    }

    let each = call(statement_expression(iteration)?)?;
    if each.arguments.len() != 1
        || static_member(&each.callee, "forEach").and_then(|object| symbol(model, object))
            != Some(keys)
    {
        return None;
    }
    let [Argument::ArrowFunctionExpression(callback)] = each.arguments.as_slice() else {
        return None;
    };
    if callback.expression
        || callback.r#async
        || callback.params.rest.is_some()
        || !callback.body.directives.is_empty()
    {
        return None;
    }
    let [key_param, index_param] = callback.params.items.as_slice() else {
        return None;
    };
    if key_param.initializer.is_some() || index_param.initializer.is_some() {
        return None;
    }
    let key = plain_unchanged(model, &key_param.pattern)?;
    let index = plain_unchanged(model, &index_param.pattern)?;
    let [assignment_statement] = callback.body.statements.as_slice() else {
        return None;
    };
    let assignment = statement_expression(assignment_statement)?;
    let Expression::AssignmentExpression(assignment) = assignment else {
        return None;
    };
    if assignment.operator.as_str() != "=" {
        return None;
    }
    let AssignmentTarget::ComputedMemberExpression(destination) = &assignment.left else {
        return None;
    };
    if destination.optional
        || symbol(model, &destination.object) != Some(result_symbol)
        || symbol(model, &destination.expression) != Some(key)
    {
        return None;
    }
    let value = match &assignment.right {
        Expression::ComputedMemberExpression(value) if !value.optional => value,
        _ => return None,
    };
    if symbol(model, &value.object) != Some(source)
        || symbol(model, &value.expression) != Some(index)
    {
        return None;
    }

    let end = returned(end)?;
    if symbol(model, end) != Some(result_symbol) {
        return None;
    }
    // The callback writes only through the result object; the named locals and
    // all four parameters must remain stable for the full protocol.
    if model.binding_writes(source) != 0
        || model.binding_writes(result_symbol) != 0
        || model.binding_writes(keys) != 0
        || model.binding_writes(values) != 0
        || model.binding_writes(key) != 0
        || model.binding_writes(index) != 0
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
        return vec![];
    };
    let Some(body) = &function.body else {
        return vec![];
    };
    map_keys_to_object(model, function, body)
        .is_some()
        .then(|| {
            proposal(
                "mapKeysToObject",
                "runtime_map_keys_to_object",
                "complete two-array mapper: a result initialized by the exact one-argument `.Object.create(null)` call shape receives each value at the exact key and index supplied by the owned forEach callback, then that result is returned; the namespace/constructor is not claimed as native",
                f,
            )
        })
        .into_iter()
        .collect()
}
