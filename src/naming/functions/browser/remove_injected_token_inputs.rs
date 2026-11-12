//! Pass-8 frontier: exact, owned-body cleanup of injected token
//! inputs.
//!
//! This module owns one small still-untargeted family from the fixed
//! seed: `removeInjectedTokenInputs`, the canonical role already
//! present in the legacy `request_output_carriers` catalog. The existing pass-6 record and
//! the pass-7 module leave it unnamed.
//!
//! The proof is a whole-body shape proof, not a bag of `Facts` presence. It
//! checks the exact statement order, the single formal alias, the captured
//! registry and captured body root, the owned `Object.keys(...).forEach(...)`
//! callback, the containment-test receiver protocol, the exact selector
//! concatenation, and the child removal through the receiver's own
//! `parentNode`. The opaque name prefix in the selector is deliberately not
//! part of the proof: only the structural `input[type=hidden][name="..."]`
//! selector and the `prefix + key` concatenation shape are required.
//!
//! No unresolved spelling is promoted to a native/global role. `Object`,
//! `Node` and `DOCUMENT_POSITION_CONTAINED_BY` are accepted only as unresolved
//! global paths, and the captured DOM root is treated as captured DATA, never
//! as a proven `document` host object. Claims are returned for normal
//! ambiguity resolution; this module has no first-winner priority.

use super::*;
use oxc_semantic::SymbolId;

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e, model.scoping)
}

fn binding_symbol(model: &Model<'_, '_>, p: &BindingPattern<'_>) -> Option<SymbolId> {
    let BindingPattern::BindingIdentifier(id) = p else {
        return None;
    };
    let symbol = id.symbol_id.get()?;
    model
        .scoping
        .symbol_redeclarations(symbol)
        .is_empty()
        .then_some(symbol)
}

fn plain_kind(kind: VariableDeclarationKind) -> bool {
    matches!(
        kind,
        VariableDeclarationKind::Var
            | VariableDeclarationKind::Let
            | VariableDeclarationKind::Const
    )
}

fn single_declarator<'a>(
    declaration: &'a VariableDeclaration<'a>,
) -> Option<(&'a VariableDeclarator<'a>, VariableDeclarationKind)> {
    let [variable] = declaration.declarations.as_slice() else {
        return None;
    };
    Some((variable, declaration.kind))
}

fn plain_local(
    model: &Model<'_, '_>,
    variable: &VariableDeclarator<'_>,
    kind: VariableDeclarationKind,
) -> Option<SymbolId> {
    if !plain_kind(kind) {
        return None;
    }
    binding_symbol(model, &variable.id)
}

fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    match e {
        Expression::StaticMemberExpression(member)
            if !member.optional && member.property.name == key =>
        {
            Some(&member.object)
        }
        Expression::ComputedMemberExpression(member)
            if !member.optional
                && matches!(&member.expression, Expression::StringLiteral(s) if s.value == key) =>
        {
            Some(&member.object)
        }
        _ => None,
    }
}

fn indexed(model: &Model<'_, '_>, e: &Expression<'_>, object: SymbolId, index: f64) -> bool {
    matches!(e, Expression::ComputedMemberExpression(member)
        if !member.optional
            && symbol(model, &member.object) == Some(object)
            && matches!(&member.expression, Expression::NumericLiteral(n) if n.value == index))
}

fn unchanged(model: &Model<'_, '_>, symbol: SymbolId) -> bool {
    model.binding_writes(symbol) == 0 && model.scoping.symbol_redeclarations(symbol).is_empty()
}

// A captured DATA receiver. Its value may legitimately be reset by setup, so
// writes are allowed; only the declaration form, the declared-before-site
// identity and an unsupported `for-in`/`for-of` origin are refused. This never
// establishes a callable or a native host role.
fn captured_data(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
) -> Option<SymbolId> {
    let symbol = symbol(model, e)?;
    if site
        .span
        .contains_inclusive(model.scoping.symbol_span(symbol))
        || !model.scoping.symbol_redeclarations(symbol).is_empty()
    {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(symbol);
    let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&variable.id, BindingPattern::BindingIdentifier(id)
            if id.symbol_id.get() == Some(symbol))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(d)
                if matches!(
                    d.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                )
        )
        || matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
        || model.scoping.symbol_span(symbol).end >= site.span.start
    {
        return None;
    }
    Some(symbol)
}

fn plain_formal(
    model: &Model<'_, '_>,
    params: &FormalParameters<'_>,
    expected: usize,
) -> Option<Vec<SymbolId>> {
    if params.rest.is_some() || params.items.len() != expected {
        return None;
    }
    let mut symbols = Vec::with_capacity(expected);
    for parameter in &params.items {
        if parameter.initializer.is_some() {
            return None;
        }
        let symbol = binding_symbol(model, &parameter.pattern)?;
        if !unchanged(model, symbol) {
            return None;
        }
        symbols.push(symbol);
    }
    Some(symbols)
}

fn boolean(e: &Expression<'_>, value: bool) -> bool {
    matches!(e, Expression::BooleanLiteral(b) if b.value == value)
}

// The owned body of the per-key cleanup callback. `input` is the outer formal
// alias, `body_root` the captured body root already resolved by the caller.
fn cleanup_callback_body(
    model: &Model<'_, '_>,
    site: FunctionSite,
    callback: &ArrowFunctionExpression<'_>,
    input: SymbolId,
    body_root: SymbolId,
) -> Option<()> {
    if callback.r#async
        || callback.expression
        || callback.params.rest.is_some()
        || callback.params.items.len() != 1
        || !callback.body.directives.is_empty()
    {
        return None;
    }
    let parameter = &callback.params.items[0];
    if parameter.initializer.is_some() {
        return None;
    }
    let key = binding_symbol(model, &parameter.pattern)?;
    if !unchanged(model, key) {
        return None;
    }

    let [Statement::VariableDeclaration(element_declaration), Statement::IfStatement(containment), Statement::VariableDeclaration(selector_declaration), Statement::ExpressionStatement(selection), Statement::IfStatement(removal)] =
        callback.body.statements.as_slice()
    else {
        return None;
    };

    let (element, element_kind) = single_declarator(element_declaration)?;
    let element_symbol = plain_local(model, element, element_kind)?;
    if element.init.is_some() {
        return None;
    }

    // if (input.elements.length !== 0) { let pair = [input, input.compareDocumentPosition]; element = pair[1].call(pair[0], input.elements[0]) & Node.DOCUMENT_POSITION_CONTAINED_BY; } else { element = false; }
    let Expression::BinaryExpression(containment_test) = &containment.test else {
        return None;
    };
    if containment_test.operator.as_str() != "!=="
        || !matches!(&containment_test.right, Expression::NumericLiteral(n) if n.value == 0.0)
    {
        return None;
    }
    let elements = member(&containment_test.left, "length")?;
    let alias = member(elements, "elements")?;
    if symbol(model, alias) != Some(input) {
        return None;
    }
    let Statement::BlockStatement(containment_body) = &containment.consequent else {
        return None;
    };
    let [Statement::VariableDeclaration(pair_declaration), Statement::ExpressionStatement(assign)] =
        containment_body.body.as_slice()
    else {
        return None;
    };
    let (pair, pair_kind) = single_declarator(pair_declaration)?;
    let pair_symbol = plain_local(model, pair, pair_kind)?;
    let Expression::ArrayExpression(pair_values) = pair.init.as_ref()? else {
        return None;
    };
    let [first, second] = pair_values.elements.as_slice() else {
        return None;
    };
    let first = first.as_expression()?;
    let second = second.as_expression()?;
    if symbol(model, first) != Some(input) {
        return None;
    }
    let compared = member(second, "compareDocumentPosition")?;
    if symbol(model, compared) != Some(input) {
        return None;
    }
    let Expression::AssignmentExpression(assign) = &assign.expression else {
        return None;
    };
    if assign.operator.as_str() != "="
        || !matches!(&assign.left, AssignmentTarget::AssignmentTargetIdentifier(target)
            if crate::analysis::identifier_symbol(target, model.scoping) == Some(element_symbol))
    {
        return None;
    }
    let Expression::BinaryExpression(contained) = &assign.right else {
        return None;
    };
    if contained.operator.as_str() != "&" {
        return None;
    }
    let Expression::CallExpression(compare_call) = &contained.left else {
        return None;
    };
    let call_method = member(&compare_call.callee, "call")?;
    if !indexed(model, call_method, pair_symbol, 1.0) {
        return None;
    }
    let compare_arguments = call_arguments(compare_call);
    if compare_arguments.len() != 2 || !indexed(model, compare_arguments[0], pair_symbol, 0.0) {
        return None;
    }
    let Expression::ComputedMemberExpression(element_index) = compare_arguments[1] else {
        return None;
    };
    let element_object = member(&element_index.object, "elements")?;
    if symbol(model, element_object) != Some(input)
        || !matches!(&element_index.expression, Expression::NumericLiteral(n) if n.value == 0.0)
    {
        return None;
    }
    if expression_path(model, &contained.right) != ["Node", "DOCUMENT_POSITION_CONTAINED_BY"] {
        return None;
    }
    let Statement::BlockStatement(alternate) = containment.alternate.as_ref()? else {
        return None;
    };
    let [Statement::ExpressionStatement(not_contained)] = alternate.body.as_slice() else {
        return None;
    };
    let Expression::AssignmentExpression(not_contained) = &not_contained.expression else {
        return None;
    };
    if not_contained.operator.as_str() != "="
        || !matches!(&not_contained.left, AssignmentTarget::AssignmentTargetIdentifier(target)
            if crate::analysis::identifier_symbol(target, model.scoping) == Some(element_symbol))
        || !boolean(&not_contained.right, false)
    {
        return None;
    }

    // let selector = (element ? input : bodyRoot.body).querySelector;
    let (selector, selector_kind) = single_declarator(selector_declaration)?;
    let selector_symbol = plain_local(model, selector, selector_kind)?;
    if !unchanged(model, selector_symbol) {
        return None;
    }
    let Expression::StaticMemberExpression(queried) = selector.init.as_ref()? else {
        return None;
    };
    if queried.optional || queried.property.name != "querySelector" {
        return None;
    }
    let Expression::ConditionalExpression(receiver_choice) = &queried.object else {
        return None;
    };
    if symbol(model, &receiver_choice.test) != Some(element_symbol)
        || symbol(model, &receiver_choice.consequent) != Some(input)
    {
        return None;
    }
    let receiver_body = member(&receiver_choice.alternate, "body")?;
    if captured_data(model, site, receiver_body)? != body_root {
        return None;
    }

    // element = selector.call(element ? input : bodyRoot.body, "input[type=hidden][name=\"" + (namePrefix + key) + "\"]");
    let Expression::AssignmentExpression(selection) = &selection.expression else {
        return None;
    };
    if selection.operator.as_str() != "="
        || !matches!(&selection.left, AssignmentTarget::AssignmentTargetIdentifier(target)
            if crate::analysis::identifier_symbol(target, model.scoping) == Some(element_symbol))
    {
        return None;
    }
    let Expression::CallExpression(selector_call) = &selection.right else {
        return None;
    };
    if selector_call.optional {
        return None;
    }
    let selector_arguments = call_arguments(selector_call);
    if selector_arguments.len() != 2 {
        return None;
    }
    let selector_target = member(&selector_call.callee, "call")?;
    if symbol(model, selector_target) != Some(selector_symbol) {
        return None;
    }
    let Expression::ConditionalExpression(call_choice) = selector_arguments[0] else {
        return None;
    };
    if symbol(model, &call_choice.test) != Some(element_symbol)
        || symbol(model, &call_choice.consequent) != Some(input)
    {
        return None;
    }
    let Expression::StaticMemberExpression(call_body) = &call_choice.alternate else {
        return None;
    };
    if call_body.optional || call_body.property.name != "body" {
        return None;
    }
    if captured_data(model, site, &call_body.object)? != body_root {
        return None;
    }

    selector_name(model, selector_arguments[1], key)?;

    remove_found(model, removal, element_symbol)
}

fn selector_name(model: &Model<'_, '_>, selector: &Expression<'_>, key: SymbolId) -> Option<()> {
    let Expression::BinaryExpression(selector_outer) = selector else {
        return None;
    };
    if selector_outer.operator.as_str() != "+" {
        return None;
    }
    let Expression::StringLiteral(suffix) = &selector_outer.right else {
        return None;
    };
    if suffix.value != "\"]" {
        return None;
    }
    let Expression::BinaryExpression(selector_inner) = &selector_outer.left else {
        return None;
    };
    if selector_inner.operator.as_str() != "+" {
        return None;
    }
    let Expression::StringLiteral(template) = &selector_inner.left else {
        return None;
    };
    if template.value != "input[type=hidden][name=\"" {
        return None;
    }
    let Expression::BinaryExpression(name) = &selector_inner.right else {
        return None;
    };
    if name.operator.as_str() != "+" {
        return None;
    }
    let Expression::StringLiteral(name_prefix) = &name.left else {
        return None;
    };
    // The actual prefix is a captured-build spelling and is never evidence.
    if name_prefix.value.is_empty() || symbol(model, &name.right) != Some(key) {
        return None;
    }

    Some(())
}

fn remove_found(
    model: &Model<'_, '_>,
    removal: &IfStatement<'_>,
    element_symbol: SymbolId,
) -> Option<()> {
    // if (element && element.parentNode != null) { element.parentNode.removeChild(element); }
    let Expression::LogicalExpression(removal_test) = &removal.test else {
        return None;
    };
    if removal_test.operator.as_str() != "&&"
        || symbol(model, &removal_test.left) != Some(element_symbol)
    {
        return None;
    }
    let Expression::BinaryExpression(parent_test) = &removal_test.right else {
        return None;
    };
    if parent_test.operator.as_str() != "!="
        || !matches!(&parent_test.right, Expression::NullLiteral(_))
    {
        return None;
    }
    let parent = member(&parent_test.left, "parentNode")?;
    if symbol(model, parent) != Some(element_symbol) {
        return None;
    }
    if removal.alternate.is_some() {
        return None;
    }
    let Statement::BlockStatement(removal_body) = &removal.consequent else {
        return None;
    };
    let [Statement::ExpressionStatement(remove)] = removal_body.body.as_slice() else {
        return None;
    };
    let Expression::CallExpression(remove) = &remove.expression else {
        return None;
    };
    if remove.optional || remove.arguments.len() != 1 {
        return None;
    }
    let remove_arguments = call_arguments(remove);
    if remove_arguments.len() != 1 || symbol(model, remove_arguments[0]) != Some(element_symbol) {
        return None;
    }
    let removed_parent = member(&remove.callee, "removeChild")?;
    let removed_from = member(removed_parent, "parentNode")?;
    if symbol(model, removed_from) != Some(element_symbol) {
        return None;
    }
    Some(())
}

// Direct receiver lowering: the same conditional containment protocol is
// evaluated once, followed by one query and one conditional removal. Preserve
// its short-circuit order; do not normalize arbitrary method calls to `.call`.
fn direct_cleanup(
    model: &Model<'_, '_>,
    site: FunctionSite,
    callback: &ArrowFunctionExpression<'_>,
    input: SymbolId,
) -> Option<()> {
    if callback.r#async || callback.expression || !callback.body.directives.is_empty() {
        return None;
    }
    let key = plain_formal(model, &callback.params, 1)?[0];
    let [Statement::VariableDeclaration(receiver), Statement::VariableDeclaration(found), Statement::IfStatement(removal)] =
        callback.body.statements.as_slice()
    else {
        return None;
    };
    let (receiver, kind) = single_declarator(receiver)?;
    let receiver_id = plain_local(model, receiver, kind)?;
    if !unchanged(model, receiver_id) {
        return None;
    }
    let Expression::ConditionalExpression(choice) = receiver.init.as_ref()? else {
        return None;
    };
    let Expression::LogicalExpression(test) = &choice.test else {
        return None;
    };
    let Expression::BinaryExpression(nonempty) = &test.left else {
        return None;
    };
    if test.operator.as_str() != "&&"
        || nonempty.operator.as_str() != "!=="
        || !matches!(&nonempty.right, Expression::NumericLiteral(n) if n.value == 0.0)
        || symbol(
            model,
            member(member(&nonempty.left, "length")?, "elements")?,
        ) != Some(input)
        || symbol(model, &choice.consequent) != Some(input)
    {
        return None;
    }
    let Expression::BinaryExpression(contained) = &test.right else {
        return None;
    };
    if contained.operator.as_str() != "&"
        || expression_path(model, &contained.right) != ["Node", "DOCUMENT_POSITION_CONTAINED_BY"]
    {
        return None;
    }
    let Expression::CallExpression(compare) = &contained.left else {
        return None;
    };
    if compare.optional
        || symbol(model, member(&compare.callee, "compareDocumentPosition")?) != Some(input)
    {
        return None;
    }
    let [Argument::ComputedMemberExpression(first)] = compare.arguments.as_slice() else {
        return None;
    };
    if first.optional
        || symbol(model, member(&first.object, "elements")?) != Some(input)
        || !matches!(&first.expression, Expression::NumericLiteral(n) if n.value == 0.0)
    {
        return None;
    }
    captured_data(model, site, member(&choice.alternate, "body")?)?;
    let (found, kind) = single_declarator(found)?;
    let found_id = plain_local(model, found, kind)?;
    if !unchanged(model, found_id) {
        return None;
    }
    let Expression::CallExpression(query) = found.init.as_ref()? else {
        return None;
    };
    if query.optional
        || symbol(model, member(&query.callee, "querySelector")?) != Some(receiver_id)
        || query.arguments.len() != 1
    {
        return None;
    }
    selector_name(model, query.arguments[0].as_expression()?, key)?;
    remove_found(model, removal, found_id)
}

fn callback_body_root(
    model: &Model<'_, '_>,
    site: FunctionSite,
    callback: &ArrowFunctionExpression<'_>,
) -> Option<SymbolId> {
    let [_, _, Statement::VariableDeclaration(selector), _, _] =
        callback.body.statements.as_slice()
    else {
        return None;
    };
    let (selector, _) = single_declarator(selector)?;
    let Expression::StaticMemberExpression(queried) = selector.init.as_ref()? else {
        return None;
    };
    if queried.optional || queried.property.name != "querySelector" {
        return None;
    }
    let Expression::ConditionalExpression(choice) = &queried.object else {
        return None;
    };
    let Expression::StaticMemberExpression(body_member) = &choice.alternate else {
        return None;
    };
    if body_member.optional || body_member.property.name != "body" {
        return None;
    }
    captured_data(model, site, &body_member.object)
}

fn remove_injected_token_inputs(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if function.r#async || function.generator || !body.directives.is_empty() {
        return None;
    }
    plain_formal(model, &function.params, 1)?;

    // let alias = formal;
    let [Statement::VariableDeclaration(alias_declaration), Statement::IfStatement(guard)] =
        body.statements.as_slice()
    else {
        return None;
    };
    let (alias, alias_kind) = single_declarator(alias_declaration)?;
    let alias_symbol = plain_local(model, alias, alias_kind)?;
    if !unchanged(model, alias_symbol) {
        return None;
    }
    let slot = model.site_parameter_slot(alias.init.as_ref()?, site);
    if slot != Some(0) {
        return None;
    }

    // if (registry != null) { Object.keys(registry).forEach(key => { ... }); }
    let Expression::BinaryExpression(guard_test) = &guard.test else {
        return None;
    };
    if guard_test.operator.as_str() != "!="
        || !matches!(&guard_test.right, Expression::NullLiteral(_))
        || guard.alternate.is_some()
    {
        return None;
    }
    let registry = captured_data(model, site, &guard_test.left)?;
    let Statement::BlockStatement(guard_body) = &guard.consequent else {
        return None;
    };
    let [Statement::ExpressionStatement(each_statement)] = guard_body.body.as_slice() else {
        return None;
    };
    let Expression::CallExpression(each) = &each_statement.expression else {
        return None;
    };
    if each.optional || each.arguments.len() != 1 {
        return None;
    }
    let Expression::StaticMemberExpression(each_method) = &each.callee else {
        return None;
    };
    if each_method.optional || each_method.property.name != "forEach" {
        return None;
    }
    let Expression::CallExpression(keys) = &each_method.object else {
        return None;
    };
    if keys.optional
        || expression_path(model, &keys.callee) != ["Object", "keys"]
        || keys.arguments.len() != 1
    {
        return None;
    }
    let key_arguments = call_arguments(keys);
    if key_arguments.len() != 1 || symbol(model, key_arguments[0]) != Some(registry) {
        return None;
    }
    let [Argument::ArrowFunctionExpression(callback)] = each.arguments.as_slice() else {
        return None;
    };
    if direct_cleanup(model, site, callback, alias_symbol).is_none() {
        let body_root = callback_body_root(model, site, callback)?;
        cleanup_callback_body(model, site, callback, alias_symbol, body_root)?;
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
    if function.r#async
        || function.generator
        || !body.directives.is_empty()
        || function.params.rest.is_some()
        || function.params.items.len() != 1
    {
        return vec![];
    }
    if remove_injected_token_inputs(model, site, function, body).is_none() {
        return vec![];
    }
    vec![proposal(
        "removeInjectedTokenInputs",
        "browser_remove_injected_token_inputs",
        "whole owned body: for each key of the captured token registry, selects the hidden input named by a captured prefix plus that key from the form when it contains the first form element, otherwise from the captured body root, and removes the found element through its own parentNode; exact alias, callback, selector concat, containment test, statement order and absence of returns are proven",
        f,
    )]
}
