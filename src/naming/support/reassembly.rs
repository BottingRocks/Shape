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

// A plain local declaration: an identifier binding in a var/let/const
// declaration that is not a for-in/of head and is not redeclared.
pub(in crate::naming) fn call<'a>(
    e: &'a Expression<'a>,
) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(c) = e else {
        return None;
    };
    (!c.optional).then_some(c)
}

pub(in crate::naming) fn member_parts<'a>(
    e: &'a Expression<'a>,
) -> Option<(&'a Expression<'a>, &'a str)> {
    let Expression::StaticMemberExpression(m) = e else {
        return None;
    };
    (!m.optional).then_some((&m.object, m.property.name.as_str()))
}

pub(in crate::naming) fn number(e: &Expression<'_>, expected: f64) -> bool {
    matches!(e, Expression::NumericLiteral(n) if n.value == expected)
}

pub(in crate::naming) fn update_symbol(
    model: &Model<'_, '_>,
    target: &SimpleAssignmentTarget<'_>,
) -> Option<SymbolId> {
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) = target else {
        return None;
    };
    crate::analysis::identifier_symbol(identifier, model.scoping)
}
