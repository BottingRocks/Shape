use super::*;
use oxc_semantic::SymbolId;

pub(in crate::naming) fn symbol(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e, model.scoping)
}

pub(in crate::naming) fn immutable(
    model: &Model<'_, '_>,
    p: &BindingPattern<'_>,
) -> Option<SymbolId> {
    let BindingPattern::BindingIdentifier(id) = p else {
        return None;
    };
    let s = id.symbol_id.get()?;
    (model.binding_writes(s) == 0 && model.scoping.symbol_redeclarations(s).is_empty()).then_some(s)
}

pub(in crate::naming) fn call<'a>(
    e: &'a Expression<'a>,
) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(c) = e else {
        return None;
    };
    (!c.optional).then_some(c)
}

pub(in crate::naming) fn statement_call<'a>(
    s: &'a Statement<'a>,
) -> Option<&'a CallExpression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    call(&s.expression)
}

pub(in crate::naming) fn local<'a>(
    model: &Model<'_, '_>,
    s: &'a Statement<'a>,
) -> Option<(SymbolId, &'a Expression<'a>)> {
    let v = one_variable(s)?;
    Some((immutable(model, &v.id)?, v.init.as_ref()?))
}

pub(in crate::naming) fn object_field<'a>(
    e: &'a Expression<'a>,
    key: &str,
) -> Option<&'a Expression<'a>> {
    let Expression::ObjectExpression(o) = e else {
        return None;
    };
    let [ObjectPropertyKind::ObjectProperty(p)] = o.properties.as_slice() else {
        return None;
    };
    (!p.computed
        && !p.method
        && p.kind == PropertyKind::Init
        && property_key_name(&p.key)?.as_str() == key)
        .then_some(&p.value)
}
