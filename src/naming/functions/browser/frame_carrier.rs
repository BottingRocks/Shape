//! Bounded own-body proof for the hidden nested-frame bridge carrier.
//!
//! The claimed parent is the carrier that prepares the hidden iframe/style pair
//! before the already-proven `startNestedFrameBridge` child runs. Its own body:
//!
//! * guards on the captured `createElement` / `appendChild` and on the captured
//!   id `get`/`set`, textContent `set` and document `get` descriptor objects,
//!   returning `false` from the guard branch;
//! * creates `"iframe"` and `"style"` through the same opaque captured applier
//!   and the captured `createElement`, passing the unshadowed global `document`;
//! * writes a random `x`+base36 id through the captured id setter and builds the
//!   paired style textContent `"#" + <id getter(iframe)> + "{display:none
//!   !important;}"`, so the hidden style hides exactly that iframe;
//! * reads the page document through the captured document getter applied to
//!   `window.document`;
//! * selects one random `x`+base36 global key and the already-proven
//!   `removeInjectedGlobalAndNotifyCleanup` callback (both nested children are
//!   proven by their own evaluators, never re-named here);
//! * schedules the already-proven zero-formal `startNestedFrameBridge` child
//!   through the immediate / captured-timer-or-global-`setTimeout(…, 0)` /
//!   captured-queue `.push` branches;
//! * returns `true`.
//!
//! The two child functions are direct `FunctionDeclaration` children owned by
//! this carrier, matched by AST node identity. This rule proves the parent's own
//! body; it does not claim the children again and it never accepts a function
//! expression, nested declaration or any other indirect donor that merely lies
//! inside the carrier body. The child proofs are reused directly from the
//! existing owning modules (`browser::frame_start` and
//! `browser::injected_global_removal`), and the bridge receiver is tied to the exact
//! `SymbolId` the carrier read through its captured document getter. Recognition
//! itself is scope-resolved symbol identity plus host/CSS structure. Generated
//! spellings, offsets, vendors, hashes, bodies and inherited labels never
//! participate.
use super::*;
use oxc_semantic::SymbolId;

type Symbol = SymbolId;

const NAME: &str = "createHiddenFrameBridgeCarrier";
const EVALUATOR: &str = "thread_posthost_frame_carrier";

fn symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(expression.without_parentheses(), model.scoping)
}

fn string_value<'a>(expression: &'a Expression<'a>) -> Option<&'a str> {
    match expression.without_parentheses() {
        Expression::StringLiteral(literal) => Some(literal.value.as_str()),
        _ => None,
    }
}

fn is_null(expression: &Expression<'_>) -> bool {
    matches!(expression.without_parentheses(), Expression::NullLiteral(_))
}

fn numeric_value(expression: &Expression<'_>) -> Option<f64> {
    match expression.without_parentheses() {
        Expression::NumericLiteral(literal) => Some(literal.value),
        _ => None,
    }
}

fn plain_call<'a>(expression: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    match expression.without_parentheses() {
        Expression::CallExpression(call) if !call.optional => Some(call),
        _ => None,
    }
}

/// Exact argument list. A spread or other non-expression argument changes the
/// list length and refuses the shape.
fn exact_arguments<'a>(
    call: &'a CallExpression<'a>,
    count: usize,
) -> Option<Vec<&'a Expression<'a>>> {
    let values = call_arguments(call);
    (call.arguments.len() == count && values.len() == count).then_some(values)
}

fn unbound_global(model: &Model<'_, '_>, expression: &Expression<'_>, name: &str) -> bool {
    matches!(
        expression.without_parentheses(),
        Expression::Identifier(identifier)
            if identifier.name == name
                && crate::analysis::identifier_symbol(identifier, model.scoping).is_none()
    )
}

/// A captured plain slot: declared before the proven function, never a formal
/// and never a redeclaration. Mutability is not required; only lexical origin
/// is proved.
fn captured(model: &Model<'_, '_>, site: FunctionSite, value: Symbol) -> bool {
    let span = model.scoping.symbol_span(value);
    !site.span.contains_inclusive(span)
        && span.end <= site.span.start
        && model.formal_symbol_slot(value).is_none()
        && model.scoping.symbol_redeclarations(value).is_empty()
}

fn captured_identifier(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
) -> Option<Symbol> {
    let value = symbol(model, expression)?;
    captured(model, site, value).then_some(value)
}

/// A local declared by the proven function itself.
fn owned_local(model: &Model<'_, '_>, site: FunctionSite, value: Symbol) -> bool {
    let body = site.body_span.unwrap_or(site.span);
    let span = model.scoping.symbol_span(value);
    span.start >= body.start
        && span.end <= body.end
        && model.formal_symbol_slot(value).is_none()
        && model.scoping.symbol_redeclarations(value).is_empty()
}

fn merge(slot: &mut Option<Symbol>, value: Symbol) -> Option<()> {
    match *slot {
        None => {
            *slot = Some(value);
            Some(())
        }
        Some(existing) if existing == value => Some(()),
        _ => None,
    }
}

fn set_once<T>(slot: &mut Option<T>, value: T) -> Option<()> {
    if slot.is_some() {
        None
    } else {
        *slot = Some(value);
        Some(())
    }
}

/// The opaque applied function reused from the existing child proof: the same
/// single-origin function the child invokes on the captured applied operation.
fn resolve_function(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<FunctionSite> {
    super::stable_function(model, expression, 0)
}

/// `descriptor.get` / `descriptor.set` with a resolved captured descriptor.
fn descriptor_member<'a>(
    model: &Model<'_, '_>,
    expression: &'a Expression<'a>,
) -> Option<(Symbol, &'a str)> {
    let Expression::StaticMemberExpression(member) = expression.without_parentheses() else {
        return None;
    };
    if member.optional {
        return None;
    }
    let object = symbol(model, &member.object)?;
    Some((object, member.property.name.as_str()))
}

/// `applier(createElement, document, "iframe" | "style")`.
fn create_call<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &'a Expression<'a>,
) -> Option<(Symbol, Symbol, &'a str)> {
    let call = plain_call(expression)?;
    let arguments = exact_arguments(call, 3)?;
    let applier = symbol(model, &call.callee)?;
    if !captured(model, site, applier) {
        return None;
    }
    let create = symbol(model, &arguments[0])?;
    if !captured(model, site, create) {
        return None;
    }
    if !unbound_global(model, &arguments[1], "document") {
        return None;
    }
    let kind = string_value(&arguments[2])?;
    Some((applier, create, kind))
}

/// `applier(documentDescriptor.get, window.document)`.
fn document_get_call(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
) -> Option<(Symbol, Symbol)> {
    let call = plain_call(expression)?;
    let arguments = exact_arguments(call, 2)?;
    let applier = symbol(model, &call.callee)?;
    if !captured(model, site, applier) {
        return None;
    }
    let (descriptor, property) = descriptor_member(model, &arguments[0])?;
    if property != "get" || !captured(model, site, descriptor) {
        return None;
    }
    let Expression::StaticMemberExpression(document) = arguments[1].without_parentheses() else {
        return None;
    };
    if document.optional
        || document.property.name != "document"
        || !unbound_global(model, &document.object, "window")
    {
        return None;
    }
    Some((applier, descriptor))
}

/// An `"x"`-prefixed base36 token derived from `Math.random()`: the random id
/// and the random global key share this exact semantic shape. Only the random
/// origin and the base36 slice are asserted; the literal prefix bytes are not a
/// gate.
fn base36_random(model: &Model<'_, '_>, expression: &Expression<'_>) -> bool {
    let Expression::CallExpression(slice) = expression.without_parentheses() else {
        return false;
    };
    if slice.optional {
        return false;
    }
    let Expression::StaticMemberExpression(slice_member) = slice.callee.without_parentheses()
    else {
        return false;
    };
    let slice_arguments = call_arguments(slice);
    if slice_member.optional
        || slice_member.property.name != "slice"
        || slice.arguments.len() != 1
        || slice_arguments.len() != 1
        || numeric_value(slice_arguments[0]).is_none()
    {
        return false;
    }
    let Expression::CallExpression(to_string) = slice_member.object.without_parentheses() else {
        return false;
    };
    let to_string_arguments = call_arguments(to_string);
    if to_string.optional
        || to_string.arguments.len() != 1
        || to_string_arguments.len() != 1
        || numeric_value(to_string_arguments[0]) != Some(36.0)
    {
        return false;
    }
    let Expression::StaticMemberExpression(to_string_member) =
        to_string.callee.without_parentheses()
    else {
        return false;
    };
    if to_string_member.optional || to_string_member.property.name != "toString" {
        return false;
    }
    let Expression::CallExpression(random) = to_string_member.object.without_parentheses() else {
        return false;
    };
    !random.optional
        && random.arguments.is_empty()
        && expression_path(model, &random.callee) == ["Math", "random"]
}

fn flatten_plus<'a>(expression: &'a Expression<'a>, parts: &mut Vec<&'a Expression<'a>>) {
    if let Expression::BinaryExpression(binary) = expression.without_parentheses() {
        if binary.operator.as_str() == "+" {
            flatten_plus(&binary.left, parts);
            flatten_plus(&binary.right, parts);
            return;
        }
    }
    parts.push(expression);
}

fn random_id(model: &Model<'_, '_>, expression: &Expression<'_>) -> bool {
    let mut parts = Vec::new();
    flatten_plus(expression, &mut parts);
    parts.len() == 2
        && matches!(parts[0].without_parentheses(), Expression::StringLiteral(_))
        && base36_random(model, parts[1])
}

/// `"#" + <idGetter(iframe)> + "{display:none !important;}"`: the style hides
/// exactly the iframe whose id was just written, through the same id getter.
fn hide_text(
    model: &Model<'_, '_>,
    applier: Symbol,
    frame: Symbol,
    id_descriptor: Symbol,
    expression: &Expression<'_>,
) -> Option<()> {
    let mut parts = Vec::new();
    flatten_plus(expression, &mut parts);
    if parts.len() != 3 {
        return None;
    }
    if string_value(parts[0])? != "#" {
        return None;
    }
    let call = plain_call(parts[1])?;
    let arguments = exact_arguments(call, 2)?;
    if symbol(model, &call.callee)? != applier {
        return None;
    }
    let (descriptor, property) = descriptor_member(model, &arguments[0])?;
    if property != "get" || descriptor != id_descriptor {
        return None;
    }
    if symbol(model, &arguments[1])? != frame {
        return None;
    }
    let css = string_value(parts[2])?;
    if !css.contains("display:none") || !css.contains("!important") {
        return None;
    }
    Some(())
}

fn bridge_shape(model: &Model<'_, '_>, bridge: FunctionSite) -> bool {
    let AstKind::Function(function) = model.nodes.kind(bridge.node_id) else {
        return false;
    };
    let Some(body) = function.body.as_ref() else {
        return false;
    };
    !function.r#async
        && !function.generator
        && function.params.rest.is_none()
        && function.params.items.is_empty()
        && body.directives.is_empty()
        && super::frame_start::nested_frame_bridge(model, bridge, body).is_some()
}

fn cleanup_shape(model: &Model<'_, '_>, cleanup: FunctionSite) -> bool {
    let AstKind::Function(function) = model.nodes.kind(cleanup.node_id) else {
        return false;
    };
    let Some(body) = function.body.as_ref() else {
        return false;
    };
    super::injected_global_removal::remove_injected_global_and_notify_cleanup(
        model, cleanup, function, body,
    )
    .is_some()
}

/// The captured `appendChild` the already-proven bridge child applies and the
/// receiver page it appends to: the first and second arguments shared by its
/// first two applier calls.
fn bridge_applied(model: &Model<'_, '_>, bridge: FunctionSite) -> Option<(Symbol, Symbol)> {
    let AstKind::Function(function) = model.nodes.kind(bridge.node_id) else {
        return None;
    };
    let body = function.body.as_ref()?;
    let [Statement::ExpressionStatement(first), Statement::ExpressionStatement(second), ..] =
        body.statements.as_slice()
    else {
        return None;
    };
    let first_call = plain_call(&first.expression)?;
    let second_call = plain_call(&second.expression)?;
    let first_arguments = exact_arguments(first_call, 3)?;
    let second_arguments = exact_arguments(second_call, 3)?;
    let applied = symbol(model, first_arguments[0])?;
    let receiver = symbol(model, first_arguments[1])?;
    let same_applied = symbol(model, second_arguments[0]) == Some(applied);
    let same_receiver = symbol(model, second_arguments[1]) == Some(receiver);
    (same_applied && same_receiver).then_some((applied, receiver))
}

/// The key and cleanup callback the bridge child installs on `window`:
/// `window[key] = callback`.
fn bridge_installed(model: &Model<'_, '_>, bridge: FunctionSite) -> Option<(Symbol, Symbol)> {
    let AstKind::Function(function) = model.nodes.kind(bridge.node_id) else {
        return None;
    };
    let body = function.body.as_ref()?;
    let install = body.statements.get(3)?;
    let Statement::ExpressionStatement(install) = install else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = &install.expression else {
        return None;
    };
    if assignment.operator.as_str() != "=" {
        return None;
    }
    let AssignmentTarget::ComputedMemberExpression(target) = &assignment.left else {
        return None;
    };
    if target.optional || !unbound_global(model, &target.object, "window") {
        return None;
    }
    Some((
        symbol(model, &target.expression)?,
        symbol(model, &assignment.right)?,
    ))
}

/// The key the cleanup child deletes: `delete window[key]`.
fn cleanup_deleted_key(model: &Model<'_, '_>, cleanup: FunctionSite) -> Option<Symbol> {
    let AstKind::Function(function) = model.nodes.kind(cleanup.node_id) else {
        return None;
    };
    let body = function.body.as_ref()?;
    let [Statement::ExpressionStatement(deletion), ..] = body.statements.as_slice() else {
        return None;
    };
    let Expression::UnaryExpression(unary) = &deletion.expression else {
        return None;
    };
    if unary.operator.as_str() != "delete" {
        return None;
    }
    let Expression::ComputedMemberExpression(member) = unary.argument.without_parentheses() else {
        return None;
    };
    if member.optional || !unbound_global(model, &member.object, "window") {
        return None;
    }
    symbol(model, &member.expression)
}

/// One statement's expression, optionally wrapped in a block.
fn one_expression<'a>(statement: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    match statement {
        Statement::ExpressionStatement(expression) => Some(&expression.expression),
        Statement::BlockStatement(block) => {
            let [Statement::ExpressionStatement(expression)] = block.body.as_slice() else {
                return None;
            };
            Some(&expression.expression)
        }
        _ => None,
    }
}

fn boolean_return(statement: &ReturnStatement<'_>, expected: bool) -> bool {
    matches!(
        statement.argument.as_ref(),
        Some(Expression::BooleanLiteral(boolean)) if boolean.value == expected
    )
}

fn returns_boolean(statement: &Statement<'_>, expected: bool) -> bool {
    match statement {
        Statement::ReturnStatement(ret) => boolean_return(ret, expected),
        Statement::BlockStatement(block) => {
            let [Statement::ReturnStatement(ret)] = block.body.as_slice() else {
                return false;
            };
            boolean_return(ret, expected)
        }
        _ => false,
    }
}

/// `if (<guard>) { return false; } else { <body> }`, tolerating the split
/// prelude that computes a boolean before the guard.
fn carrier_guard<'a>(body: &'a FunctionBody<'a>) -> Option<&'a [Statement<'a>]> {
    let mut found = None;
    for statement in &body.statements {
        let Statement::IfStatement(branch) = statement else {
            continue;
        };
        if !returns_boolean(&branch.consequent, false) {
            continue;
        }
        let Some(alternate) = branch.alternate.as_ref() else {
            continue;
        };
        let Statement::BlockStatement(else_block) = alternate else {
            continue;
        };
        if found.is_some() {
            return None;
        }
        found = Some(else_block.body.as_slice());
    }
    found
}

/// The already-proven bridge child scheduled from the `typeof setTimeout`
/// immediate / timer / queue branches. Every branch must target that exact
/// child.
fn scheduling(
    model: &Model<'_, '_>,
    site: FunctionSite,
    branch: &IfStatement<'_>,
) -> Option<FunctionSite> {
    let Expression::BinaryExpression(test) = branch.test.without_parentheses() else {
        return None;
    };
    if test.operator.as_str() != "!==" {
        return None;
    }
    let Expression::UnaryExpression(typeof_) = test.left.without_parentheses() else {
        return None;
    };
    if typeof_.operator.as_str() != "typeof"
        || !unbound_global(model, &typeof_.argument, "setTimeout")
        || string_value(&test.right)? != "function"
    {
        return None;
    }
    let immediate = plain_call(one_expression(&branch.consequent)?)?;
    if !immediate.arguments.is_empty() {
        return None;
    }
    let bridge = resolve_function(model, &immediate.callee)?;

    let Statement::IfStatement(timer) = branch.alternate.as_ref()? else {
        return None;
    };
    captured_identifier(model, site, &timer.test)?;
    let timer_call = plain_call(one_expression(&timer.consequent)?)?;
    let timer_arguments = exact_arguments(timer_call, 2)?;
    if resolve_function(model, timer_arguments[0])?.span != bridge.span
        || numeric_value(timer_arguments[1]) != Some(0.0)
    {
        return None;
    }
    if !unbound_global(model, &timer_call.callee, "setTimeout")
        && captured_identifier(model, site, &timer_call.callee).is_none()
    {
        return None;
    }

    let Statement::IfStatement(queue) = timer.alternate.as_ref()? else {
        return None;
    };
    if queue.alternate.is_some() {
        return None;
    }
    let queue_symbol = captured_identifier(model, site, &queue.test)?;
    let queue_call = plain_call(one_expression(&queue.consequent)?)?;
    let queue_arguments = exact_arguments(queue_call, 1)?;
    let Expression::StaticMemberExpression(push) = queue_call.callee.without_parentheses() else {
        return None;
    };
    if push.optional
        || push.property.name != "push"
        || symbol(model, &push.object)? != queue_symbol
        || resolve_function(model, queue_arguments[0])?.span != bridge.span
    {
        return None;
    }
    Some(bridge)
}

fn nonce_if(statement: &Statement<'_>) -> bool {
    let Statement::IfStatement(branch) = statement else {
        return false;
    };
    if branch.alternate.is_some() {
        return false;
    }
    let Expression::BinaryExpression(test) = branch.test.without_parentheses() else {
        return false;
    };
    if !matches!(test.operator.as_str(), "!=" | "!==")
        || (!is_null(&test.right) && !is_null(&test.left))
    {
        return false;
    }
    let Some(expression) = one_expression(&branch.consequent) else {
        return false;
    };
    let Expression::AssignmentExpression(assignment) = expression.without_parentheses() else {
        return false;
    };
    if assignment.operator.as_str() != "=" {
        return false;
    }
    let AssignmentTarget::StaticMemberExpression(target) = &assignment.left else {
        return false;
    };
    !target.optional && target.property.name == "nonce"
}

#[derive(Default)]
struct Carrier<'a> {
    applier: Option<Symbol>,
    create: Option<Symbol>,
    frame: Option<Symbol>,
    style: Option<Symbol>,
    id_descriptor: Option<Symbol>,
    text_descriptor: Option<Symbol>,
    document_descriptor: Option<Symbol>,
    document_local: Option<Symbol>,
    key: Option<Symbol>,
    cleanup: Option<FunctionSite>,
    cleanup_local: Option<Symbol>,
    bridge: Option<FunctionSite>,
    text_value: Option<&'a Expression<'a>>,
    children: Vec<FunctionSite>,
    returned: bool,
}

fn local_assignment<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &'a Statement<'a>,
) -> Option<(Symbol, &'a Expression<'a>)> {
    match statement {
        Statement::VariableDeclaration(declaration) => {
            let [variable] = declaration.declarations.as_slice() else {
                return None;
            };
            let BindingPattern::BindingIdentifier(identifier) = &variable.id else {
                return None;
            };
            let item = identifier.symbol_id.get()?;
            if !owned_local(model, site, item) {
                return None;
            }
            Some((item, variable.init.as_ref()?))
        }
        Statement::ExpressionStatement(expression) => {
            let Expression::AssignmentExpression(assignment) = &expression.expression else {
                return None;
            };
            if assignment.operator.as_str() != "=" {
                return None;
            }
            let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
                return None;
            };
            let item = crate::analysis::identifier_symbol(identifier, model.scoping)?;
            if !owned_local(model, site, item) {
                return None;
            }
            Some((item, &assignment.right))
        }
        _ => None,
    }
}

fn bare_declaration(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
) -> Option<()> {
    let Statement::VariableDeclaration(declaration) = statement else {
        return None;
    };
    for variable in &declaration.declarations {
        if variable.init.is_some() {
            return None;
        }
        let BindingPattern::BindingIdentifier(identifier) = &variable.id else {
            return None;
        };
        if !owned_local(model, site, identifier.symbol_id.get()?) {
            return None;
        }
    }
    Some(())
}

fn classify_assignment<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    carrier: &mut Carrier<'a>,
    item: Symbol,
    value: &'a Expression<'a>,
) -> Option<()> {
    if let Some((applier, create, kind)) = create_call(model, site, value) {
        merge(&mut carrier.applier, applier)?;
        merge(&mut carrier.create, create)?;
        return match kind {
            "iframe" => merge(&mut carrier.frame, item),
            "style" => merge(&mut carrier.style, item),
            _ => None,
        };
    }
    if let Some((applier, descriptor)) = document_get_call(model, site, value) {
        merge(&mut carrier.applier, applier)?;
        set_once(&mut carrier.document_local, item)?;
        set_once(&mut carrier.document_descriptor, descriptor)?;
        return Some(());
    }
    if random_id(model, value) {
        return set_once(&mut carrier.key, item);
    }
    let child = resolve_function(model, value)?;
    if !cleanup_shape(model, child) {
        return None;
    }
    set_once(&mut carrier.cleanup, child)?;
    set_once(&mut carrier.cleanup_local, item)
}

fn classify_bare<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    carrier: &mut Carrier<'a>,
    call: &'a CallExpression<'a>,
) -> Option<()> {
    let arguments = exact_arguments(call, 3)?;
    let applier = symbol(model, &call.callee)?;
    if !captured(model, site, applier) {
        return None;
    }
    let (descriptor, property) = descriptor_member(model, &arguments[0])?;
    if property != "set" || !captured(model, site, descriptor) {
        return None;
    }
    let target = symbol(model, &arguments[1])?;
    let value = arguments[2];
    merge(&mut carrier.applier, applier)?;
    if random_id(model, value) {
        set_once(&mut carrier.id_descriptor, descriptor)?;
        return merge(&mut carrier.frame, target);
    }
    set_once(&mut carrier.text_descriptor, descriptor)?;
    merge(&mut carrier.style, target)?;
    if carrier.text_value.is_some() {
        return None;
    }
    carrier.text_value = Some(value);
    Some(())
}

fn carrier_body<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statements: &'a [Statement<'a>],
) -> Option<Carrier<'a>> {
    let last = statements.len().checked_sub(1)?;
    let mut carrier = Carrier::default();
    for (index, statement) in statements.iter().enumerate() {
        match statement {
            Statement::EmptyStatement(_) => {}
            Statement::ReturnStatement(ret) => {
                if index != last || !boolean_return(ret, true) {
                    return None;
                }
                carrier.returned = true;
            }
            Statement::FunctionDeclaration(function) => {
                carrier.children.push(model.site_by_span(function.span)?);
            }
            Statement::IfStatement(branch) => {
                if let Some(bridge) = scheduling(model, site, branch) {
                    set_once(&mut carrier.bridge, bridge)?;
                } else if !nonce_if(statement) {
                    return None;
                }
            }
            Statement::ExpressionStatement(expression) => {
                if let Expression::CallExpression(call) =
                    expression.expression.without_parentheses()
                {
                    classify_bare(model, site, &mut carrier, call)?;
                } else {
                    let (item, value) = local_assignment(model, site, statement)?;
                    classify_assignment(model, site, &mut carrier, item, value)?;
                }
            }
            Statement::VariableDeclaration(_) => {
                if let Some((item, value)) = local_assignment(model, site, statement) {
                    classify_assignment(model, site, &mut carrier, item, value)?;
                } else {
                    bare_declaration(model, site, statement)?;
                }
            }
            _ => return None,
        }
    }
    carrier.returned.then_some(carrier)
}

#[derive(Default)]
struct Checks {
    bare: BTreeSet<Symbol>,
    members: BTreeSet<(Symbol, String)>,
}

fn record_checked(model: &Model<'_, '_>, expression: &Expression<'_>, checks: &mut Checks) {
    match expression.without_parentheses() {
        Expression::Identifier(identifier) => {
            if let Some(value) = crate::analysis::identifier_symbol(identifier, model.scoping) {
                checks.bare.insert(value);
            }
        }
        Expression::StaticMemberExpression(member) if !member.optional => {
            if let Some(value) = symbol(model, &member.object) {
                checks
                    .members
                    .insert((value, member.property.name.to_string()));
            }
        }
        _ => {}
    }
}

fn collect_checks_expression(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    checks: &mut Checks,
    depth: usize,
) {
    if depth > 64 {
        return;
    }
    match expression.without_parentheses() {
        Expression::BinaryExpression(binary) => {
            if matches!(binary.operator.as_str(), "==" | "===") {
                if is_null(&binary.right) {
                    record_checked(model, &binary.left, checks);
                } else if is_null(&binary.left) {
                    record_checked(model, &binary.right, checks);
                }
            }
            collect_checks_expression(model, &binary.left, checks, depth + 1);
            collect_checks_expression(model, &binary.right, checks, depth + 1);
        }
        Expression::LogicalExpression(logical) => {
            collect_checks_expression(model, &logical.left, checks, depth + 1);
            collect_checks_expression(model, &logical.right, checks, depth + 1);
        }
        Expression::UnaryExpression(unary) => {
            collect_checks_expression(model, &unary.argument, checks, depth + 1);
        }
        Expression::AssignmentExpression(assignment) => {
            collect_checks_expression(model, &assignment.right, checks, depth + 1);
        }
        Expression::CallExpression(call) => {
            for argument in call_arguments(call) {
                collect_checks_expression(model, argument, checks, depth + 1);
            }
        }
        _ => {}
    }
}

fn collect_checks(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    checks: &mut Checks,
    depth: usize,
) {
    if depth > 64 {
        return;
    }
    match statement {
        Statement::BlockStatement(block) => {
            for item in &block.body {
                collect_checks(model, item, checks, depth + 1);
            }
        }
        Statement::IfStatement(branch) => {
            collect_checks_expression(model, &branch.test, checks, depth + 1);
            collect_checks(model, &branch.consequent, checks, depth + 1);
            if let Some(alternate) = branch.alternate.as_ref() {
                collect_checks(model, alternate, checks, depth + 1);
            }
        }
        Statement::ExpressionStatement(expression) => {
            collect_checks_expression(model, &expression.expression, checks, depth + 1);
        }
        Statement::VariableDeclaration(declaration) => {
            for variable in &declaration.declarations {
                if let Some(initializer) = variable.init.as_ref() {
                    collect_checks_expression(model, initializer, checks, depth + 1);
                }
            }
        }
        Statement::ReturnStatement(ret) => {
            if let Some(argument) = ret.argument.as_ref() {
                collect_checks_expression(model, argument, checks, depth + 1);
            }
        }
        _ => {}
    }
}

fn carrier_proof<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &'a FunctionBody<'a>,
) -> Option<()> {
    let statements = carrier_guard(body)?;
    let carrier = carrier_body(model, site, statements)?;

    let applier = carrier.applier?;
    let create = carrier.create?;
    let frame = carrier.frame?;
    let style = carrier.style?;
    let id_descriptor = carrier.id_descriptor?;
    let text_descriptor = carrier.text_descriptor?;
    let document_descriptor = carrier.document_descriptor?;
    let document_local = carrier.document_local?;
    let key = carrier.key?;
    let cleanup = carrier.cleanup?;
    let cleanup_local = carrier.cleanup_local?;
    let bridge = carrier.bridge?;
    let text_value = carrier.text_value?;

    if hide_text(model, applier, frame, id_descriptor, text_value).is_none() {
        return None;
    }
    if !bridge_shape(model, bridge) || !cleanup_shape(model, cleanup) {
        return None;
    }
    if bridge.span.start == cleanup.span.start {
        return None;
    }
    let body_span = site.body_span.unwrap_or(site.span);
    if bridge.span.start < body_span.start
        || bridge.span.end > body_span.end
        || cleanup.span.start < body_span.start
        || cleanup.span.end > body_span.end
    {
        return None;
    }
    // Both selected sites must be direct `FunctionDeclaration` children of this
    // carrier body, matched by AST node identity. A function expression, a
    // nested declaration or any other indirect donor that merely lies inside
    // the carrier body has a different parent site and never lends a child
    // proof to the parent.
    let mut direct_bridge = false;
    let mut direct_cleanup = false;
    for child in &carrier.children {
        if child.node_id == bridge.node_id {
            direct_bridge = true;
        } else if child.node_id == cleanup.node_id {
            direct_cleanup = true;
        } else {
            return None;
        }
    }
    if !direct_bridge || !direct_cleanup {
        return None;
    }

    // The one random global key is exactly what the bridge installs and what
    // the cleanup deletes; the bridge installs exactly the selected cleanup
    // callback.
    let (installed_key, installed_callback) = bridge_installed(model, bridge)?;
    let deleted_key = cleanup_deleted_key(model, cleanup)?;
    if installed_key != key || deleted_key != key || installed_callback != cleanup_local {
        return None;
    }

    // Every owned local is a distinct identity and no host capture collides
    // with one of them.
    let locals = [frame, style, document_local, key, cleanup_local];
    for (index, value) in locals.iter().enumerate() {
        if locals[index + 1..].contains(value) {
            return None;
        }
    }
    let captures = [
        applier,
        create,
        id_descriptor,
        text_descriptor,
        document_descriptor,
    ];
    for (index, value) in captures.iter().enumerate() {
        if captures[index + 1..].contains(value) || locals.contains(value) {
            return None;
        }
    }

    // The captured host null guards: createElement and the id get/set, the
    // textContent set and the document get descriptors. The appendChild the
    // bridge child applies must be null-guarded as well, and the bridge must
    // append through exactly the page the carrier read with its captured
    // document getter: the same `SymbolId`, never another unrelated page.
    let (append, receiver) = bridge_applied(model, bridge)?;
    if receiver != document_local || !captured(model, site, append) {
        return None;
    }
    let mut checks = Checks::default();
    for statement in &body.statements {
        collect_checks(model, statement, &mut checks, 0);
    }
    if !checks.bare.contains(&create) || !checks.bare.contains(&append) {
        return None;
    }
    let required: [(Symbol, &[&str]); 3] = [
        (id_descriptor, &["get", "set"]),
        (text_descriptor, &["set"]),
        (document_descriptor, &["get"]),
    ];
    for (descriptor, members) in required {
        if !checks.bare.contains(&descriptor) {
            return None;
        }
        for member in members {
            if !checks
                .members
                .contains(&(descriptor, (*member).to_string()))
            {
                return None;
            }
        }
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
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || function.params.items.len() > 3
        || !body.directives.is_empty()
    {
        return Vec::new();
    }
    for parameter in &function.params.items {
        if parameter.initializer.is_some()
            || !matches!(&parameter.pattern, BindingPattern::BindingIdentifier(_))
        {
            return Vec::new();
        }
    }
    if carrier_proof(model, site, body).is_none() {
        return Vec::new();
    }
    vec![proposal(
        NAME,
        EVALUATOR,
        "owned hidden nested-frame bridge carrier: guards the captured createElement, \
         appendChild and id get/set, textContent set and document get descriptors; \
         creates the iframe and its paired hidden style through the same opaque \
         applier, writes a random x-prefixed base36 id and a style textContent that \
         hides exactly that id; reads window.document through the captured document \
         getter; selects a random global key and the proved cleanup child; schedules \
         the proved zero-formal startNestedFrameBridge child through the immediate, \
         timer and queue branches; returns true",
        f,
    )]
}
