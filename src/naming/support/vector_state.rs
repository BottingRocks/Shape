use super::*;
use oxc_semantic::SymbolId;

pub(in crate::naming) fn symbol(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
) -> Option<SymbolId> {
    crate::analysis::expression_symbol(expression, model.scoping)
}

pub(in crate::naming) fn static_member<'a>(
    expression: &'a Expression<'a>,
    name: &str,
) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(member) = expression else {
        return None;
    };
    (!member.optional && member.property.name == name).then_some(&member.object)
}

pub(in crate::naming) fn call<'a>(
    expression: &'a Expression<'a>,
) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(call) = expression else {
        return None;
    };
    (!call.optional).then_some(call)
}

pub(in crate::naming) fn plain_parameter(
    model: &Model<'_, '_>,
    parameter: &BindingPattern<'_>,
) -> Option<SymbolId> {
    let BindingPattern::BindingIdentifier(identifier) = parameter else {
        return None;
    };
    let symbol = identifier.symbol_id.get()?;
    (model.binding_writes(symbol) == 0 && model.scoping.symbol_redeclarations(symbol).is_empty())
        .then_some(symbol)
}

pub(in crate::naming) fn plain_function_parameters(
    model: &Model<'_, '_>,
    function: &Function<'_>,
    count: usize,
) -> Option<Vec<SymbolId>> {
    if function.r#async || function.generator || function.params.rest.is_some() {
        return None;
    }
    if function.params.items.len() != count {
        return None;
    }
    function
        .params
        .items
        .iter()
        .map(|parameter| {
            if parameter.initializer.is_some() {
                return None;
            }
            plain_parameter(model, &parameter.pattern)
        })
        .collect()
}

pub(in crate::naming) fn same_member(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    owner: SymbolId,
    name: &str,
) -> bool {
    static_member(expression, name).and_then(|object| symbol(model, object)) == Some(owner)
}
