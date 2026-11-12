//! Late, whole-owned-body browser utility contracts. The Proxy trap is named
//! only through its exact parent registration, never by a nested-body donor.
use super::*;
use oxc_syntax::operator::{BinaryOperator, UnaryOperator};

pub(in crate::naming) fn sym(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e.without_parentheses(), model.scoping)
}
pub(in crate::naming) fn stable(model: &Model<'_, '_>, s: SymbolId) -> bool {
    model.binding_writes(s) == 0 && model.scoping.symbol_redeclarations(s).is_empty()
}
pub(in crate::naming) fn expr<'a>(
    s: &'a Statement<'a>,
) -> Option<&'a Expression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    Some(s.expression.without_parentheses())
}
fn returned<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ReturnStatement(s) = s else {
        return None;
    };
    s.argument.as_ref().map(Expression::without_parentheses)
}
pub(in crate::naming) fn call<'a>(
    e: &'a Expression<'a>,
) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(c) = e.without_parentheses() else {
        return None;
    };
    (!c.optional).then_some(c)
}
pub(in crate::naming) fn member<'a>(
    e: &'a Expression<'a>,
    key: &str,
) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = e.without_parentheses() else {
        return None;
    };
    (!m.optional && m.property.name == key).then_some(&m.object)
}
pub(in crate::naming) fn ambient(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    name: &str,
) -> bool {
    matches!(e.without_parentheses(), Expression::Identifier(i)
        if i.name == name && crate::analysis::identifier_symbol(i, model.scoping).is_none())
}
pub(in crate::naming) fn number(e: &Expression<'_>, value: f64) -> bool {
    matches!(e.without_parentheses(), Expression::NumericLiteral(n) if n.value == value)
}
fn boolean(e: &Expression<'_>, value: bool) -> bool {
    matches!(e.without_parentheses(), Expression::BooleanLiteral(b) if b.value == value)
}
fn empty_object(e: &Expression<'_>) -> bool {
    matches!(e.without_parentheses(), Expression::ObjectExpression(o) if o.properties.is_empty())
}
fn plain_function(function: &Function<'_>) -> bool {
    !function.r#async
        && !function.generator
        && function.params.rest.is_none()
        && function
            .params
            .items
            .iter()
            .all(|p| p.initializer.is_none())
}
fn local<'a>(
    model: &Model<'_, 'a>,
    s: &'a Statement<'a>,
) -> Option<(SymbolId, &'a Expression<'a>)> {
    let Statement::VariableDeclaration(d) = s else {
        return None;
    };
    let [v] = d.declarations.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(id) = &v.id else {
        return None;
    };
    let symbol = id.symbol_id.get()?;
    model
        .scoping
        .symbol_redeclarations(symbol)
        .is_empty()
        .then_some((symbol, v.init.as_ref()?))
}
pub(in crate::naming) fn block<'a>(
    s: &'a Statement<'a>,
) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(&b.body)
}

/// Returns the exact child span only when the whole parent and trap agree on
/// the false->true flag and fresh prototype object, by SymbolId.
fn proxy_contract(model: &Model<'_, '_>, function: &Function<'_>) -> Option<oxc_span::Span> {
    if !plain_function(function) || !function.params.items.is_empty() {
        return None;
    }
    let [Statement::IfStatement(branch)] = function.body.as_ref()?.statements.as_slice() else {
        return None;
    };
    let Expression::BinaryExpression(test) = branch.test.without_parentheses() else {
        return None;
    };
    let Expression::UnaryExpression(kind) = test.left.without_parentheses() else {
        return None;
    };
    if test.operator != BinaryOperator::StrictInequality
        || kind.operator != UnaryOperator::Typeof
        || !ambient(model, &kind.argument, "Proxy")
        || !matches!(test.right.without_parentheses(), Expression::StringLiteral(s) if s.value == "function")
    {
        return None;
    }
    let [no] = block(&branch.consequent)? else {
        return None;
    };
    if !boolean(returned(no)?, false) {
        return None;
    }
    let [flag, object, Statement::FunctionDeclaration(trap), tail @ ..] =
        block(branch.alternate.as_ref()?)?
    else {
        return None;
    };
    let (flag, initial) = local(model, flag)?;
    let (object, initial_object) = local(model, object)?;
    if !boolean(initial, false)
        || !empty_object(initial_object)
        || !stable(model, object)
        || model.binding_writes(flag) != 1
        || !plain_function(trap)
        || !trap.params.items.is_empty()
    {
        return None;
    }
    let trap_symbol = trap.id.as_ref()?.symbol_id.get()?;
    if !stable(model, trap_symbol) {
        return None;
    }
    let [mark, ret] = trap.body.as_ref()?.statements.as_slice() else {
        return None;
    };
    let Expression::AssignmentExpression(mark) = expr(mark)? else {
        return None;
    };
    if mark.operator != AssignmentOperator::Assign
        || assignment_target_symbol(model, &mark.left) != Some(flag)
        || !boolean(&mark.right, true)
        || sym(model, returned(ret)?) != Some(object)
    {
        return None;
    }
    // Both lowerings return the local flag only when prototype identity agrees.
    // The explicit branches must each contain exactly the proved return;
    // evaluating a branch with extra effects cannot borrow the conjunction proof.
    let (result, explicit) = match tail {
        [result] => (result, None),
        [result_decl, alias_decl, Statement::IfStatement(absent), result] => {
            let (result_symbol, result_value) = local(model, result_decl)?;
            let (alias, alias_value) = local(model, alias_decl)?;
            if !stable(model, result_symbol) || !stable(model, alias)
                || sym(model, alias_value) != Some(object) || absent.alternate.is_some()
            { return None; }
            let Expression::UnaryExpression(test) = &absent.test else { return None; };
            if test.operator != UnaryOperator::LogicalNot || sym(model, &test.argument) != Some(alias) { return None; }
            let [Statement::ThrowStatement(throw)] = block(&absent.consequent)? else { return None; };
            let Expression::NewExpression(error) = &throw.argument else { return None; };
            if !ambient(model, &error.callee, "TypeError") || error.arguments.len() != 1
                || !matches!(error.arguments[0].as_expression()?, Expression::StringLiteral(s) if s.value == "Cannot set properties of undefined (setting 'value')")
            { return None; }
            (result, Some((result_symbol, alias, result_value)))
        }
        _ => return None,
    };
    let equality = match result {
        Statement::IfStatement(branch) => {
            let yes = match (block(&branch.consequent)?, explicit.is_some()) {
                ([yes], _) => yes,
                ([Statement::IfStatement(check), yes], true) => {
                    let Expression::BinaryExpression(test) = &check.test else { return None; };
                    if test.operator != BinaryOperator::StrictInequality
                        || sym(model, &test.left) != Some(flag)
                        || !matches!(&test.right, Expression::NullLiteral(_))
                        || check.alternate.is_some() || !block(&check.consequent)?.is_empty()
                    { return None; }
                    yes
                }
                _ => return None,
            };
            let [no] = block(branch.alternate.as_ref()?)? else { return None; };
            if sym(model, returned(yes)?) != Some(flag) || !boolean(returned(no)?, false) {
                return None;
            }
            &branch.test
        }
        _ => {
            let Expression::LogicalExpression(result) = returned(result)? else { return None; };
            if result.operator != LogicalOperator::And || sym(model, &result.right) != Some(flag) {
                return None;
            }
            &result.left
        }
    };
    let Expression::BinaryExpression(equal) = equality.without_parentheses() else {
        return None;
    };
    if equal.operator != BinaryOperator::StrictEquality
        || sym(model, &equal.right) != Some(explicit.map_or(object, |(_, alias, _)| alias))
    {
        return None;
    }
    let get_value = if let Some((result_symbol, _, value)) = explicit {
        if sym(model, &equal.left) != Some(result_symbol) { return None; }
        value
    } else { &equal.left };
    let get = call(get_value)?;
    if !ambient(model, member(&get.callee, "getPrototypeOf")?, "Object") || get.arguments.len() != 1
    {
        return None;
    }
    let args = call_arguments(get);
    if args.len() != 1 {
        return None;
    }
    let Expression::NewExpression(proxy) = args[0].without_parentheses() else {
        return None;
    };
    if !ambient(model, &proxy.callee, "Proxy") || proxy.arguments.len() != 2 {
        return None;
    }
    let target = proxy.arguments[0].as_expression()?;
    let Expression::ObjectExpression(handler) =
        proxy.arguments[1].as_expression()?.without_parentheses()
    else {
        return None;
    };
    let [ObjectPropertyKind::ObjectProperty(property)] = handler.properties.as_slice() else {
        return None;
    };
    if !empty_object(target)
        || property.computed
        || property.method
        || property.kind != PropertyKind::Init
        || property_key_name(&property.key).as_deref() != Some("getPrototypeOf")
        || sym(model, &property.value) != Some(trap_symbol)
    {
        return None;
    }
    Some(trap.span)
}
pub(in crate::naming) fn assignment_target_symbol(
    model: &Model<'_, '_>,
    target: &AssignmentTarget<'_>,
) -> Option<SymbolId> {
    let AssignmentTarget::AssignmentTargetIdentifier(id) = target else {
        return None;
    };
    crate::analysis::identifier_symbol(id, model.scoping)
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return vec![];
    };
    if !plain_function(function) {
        return vec![];
    }
    if proxy_contract(model, function).is_some() {
        return vec![proposal("supportsProxyGetPrototypeOf", "shared_browser_proxy_probe",
            "whole owned guard and fresh-object Proxy construction; exact registered child writes the local flag and returns the same prototype tested by strict equality and flag conjunction", f)];
    }
    if function.params.items.is_empty()
        && function
            .body
            .as_ref()
            .is_some_and(|b| b.statements.len() == 2)
    {
        for parent in model
            .all_sites()
            .iter()
            .filter(|p| p.span != site.span && p.span.contains_inclusive(site.span))
        {
            if let AstKind::Function(parent) = model.nodes.kind(parent.node_id) {
                if proxy_contract(model, parent) == Some(site.span) {
                    return vec![proposal("markProxyPrototypeTrap", "shared_browser_proxy_probe",
                        "explicit parent-child contract: this exact function is the new Proxy getPrototypeOf trap; writes parent's false-initialized flag true and returns parent's fresh prototype object", f)];
                }
            }
        }
    }
    super::super::scheduling::frame_sampler::evaluate(model, site, function, f)
}
