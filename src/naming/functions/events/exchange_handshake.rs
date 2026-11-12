//! Bounded owned-body proof for the custom-event exchange handshake bootstrap.
//!
//! One function's own statements are proved from source and the exact
//! scope-resolved participants; nothing is read from a generated spelling, a
//! thread id, a source position, a vendor or a fingerprint:
//!
//! ```text
//! function bootstrap(f0, f1, ...) {
//!   function preventDefaultListener(event) { event.preventDefault(); }   // owned
//!   let receiver = document.createEvent("CustomEvent");                  // local
//!   let slotA = [], slotB = [], slotC = 0;                               // closure slots
//!   function exchange(values, key, stamp) {                              // owned callback
//!     slotA = values; slotB = key; slotC = stamp;
//!   }
//!   let detail = { exchange: exchange };                                 // or placeholder + store
//!   let cursor = 0;
//!   while (cursor < names.length) {                                      // captured array
//!     receiver.initCustomEvent(names[cursor], false, true, detail);
//!     if (dispatchEvent(receiver)) { ++cursor; continue; }
//!     else { return void 0; }                                            // cancel aborts
//!   }
//!   let i = 0;
//!   for (; i < names.length; i++) {
//!     addEventListener(names[i], preventDefaultListener, true);          // capture phase
//!   }
//!   let session = new SessionClass(f0, ..., slotA, slotB, slotC);
//!   session.init();
//! }
//! ```
//!
//! Identity is by `SymbolId` and control-flow ownership only. The created
//! receiver must be a local whose value is `document.createEvent("CustomEvent")`;
//! the instrumented event-name array must be a captured binding that both the
//! probe loop and the capture-phase loop index; the `detail` must carry exactly
//! one `exchange` slot bound to a nested callback owned by this same function,
//! and that callback's own body must copy its formals into closure variables that
//! the session constructor then receives. The capture-phase listener must be an
//! owned nested function installed with the literal capture flag: either the
//! `preventDefault` stub, or the owned exchange-teardown sibling whose own body
//! removes itself from the same captured name list and emits the guarded
//! `formal.detail.exchange(...)`, proven by that body and symbol identity rather
//! than by its name. A direct `receiver.initCustomEvent(...)` call and the
//! lowered `[receiver, receiver.initCustomEvent].call(receiver, ...)` node shape
//! are the same operation. Every identity is distinct, and only the outer
//! function's own statements are scanned, so a nested decoy or an unrelated
//! helper cannot donate evidence.
use super::*;
use oxc_semantic::SymbolId;

fn symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(expression.without_parentheses(), model.scoping)
}

fn member<'a>(expression: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = expression.without_parentheses() else {
        return None;
    };
    (!m.optional && m.property.name == key).then_some(&m.object)
}

fn call<'a>(expression: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(c) = expression.without_parentheses() else {
        return None;
    };
    (!c.optional).then_some(c)
}

fn statement_call<'a>(statement: &'a Statement<'a>) -> Option<&'a CallExpression<'a>> {
    let Statement::ExpressionStatement(s) = statement else {
        return None;
    };
    call(&s.expression)
}

fn block<'a>(statement: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = statement else {
        return None;
    };
    Some(b.body.as_slice())
}

fn boolean(expression: &Expression<'_>, value: bool) -> bool {
    matches!(expression.without_parentheses(), Expression::BooleanLiteral(b) if b.value == value)
}

fn number(expression: &Expression<'_>, value: f64) -> bool {
    matches!(expression.without_parentheses(), Expression::NumericLiteral(n) if n.value == value)
}

fn is_global(model: &Model<'_, '_>, expression: &Expression<'_>, name: &str) -> bool {
    matches!(expression.without_parentheses(), Expression::Identifier(i)
        if i.name == name && crate::analysis::identifier_symbol(i, model.scoping).is_none())
}

fn returns_undefined(statement: &Statement<'_>) -> bool {
    let Statement::ReturnStatement(r) = statement else {
        return false;
    };
    match r.argument.as_ref().map(|e| e.without_parentheses()) {
        None => true,
        Some(Expression::UnaryExpression(u)) => u.operator.as_str() == "void",
        Some(Expression::Identifier(i)) => i.name == "undefined",
        _ => false,
    }
}

fn unset(expression: &Expression<'_>) -> bool {
    match expression.without_parentheses() {
        Expression::NullLiteral(_) => true,
        Expression::UnaryExpression(u) => u.operator.as_str() == "void",
        Expression::Identifier(i) => i.name == "undefined",
        _ => false,
    }
}

fn plain_formals(model: &Model<'_, '_>, params: &FormalParameters<'_>) -> Option<Vec<SymbolId>> {
    if params.rest.is_some() {
        return None;
    }
    let mut symbols = Vec::new();
    for parameter in &params.items {
        if parameter.initializer.is_some() {
            return None;
        }
        let BindingPattern::BindingIdentifier(id) = &parameter.pattern else {
            return None;
        };
        let s = id.symbol_id.get()?;
        if model.binding_writes(s) != 0 || !model.scoping.symbol_redeclarations(s).is_empty() {
            return None;
        }
        symbols.push(s);
    }
    Some(symbols)
}

/// A plain local declared inside this function's own body. Parameters, loop
/// targets, function declarations and any redeclaration are refused.
fn body_local(model: &Model<'_, '_>, site: FunctionSite, symbol: SymbolId) -> bool {
    if model.formal_symbol_slot(symbol).is_some()
        || !site
            .span
            .contains_inclusive(model.scoping.symbol_span(symbol))
        || !model.scoping.symbol_redeclarations(symbol).is_empty()
    {
        return false;
    }
    let declaration = model.scoping.symbol_declaration(symbol);
    matches!(
        model.nodes.kind(declaration),
        AstKind::VariableDeclarator(_)
    ) && matches!(
        model.nodes.parent_kind(declaration),
        AstKind::VariableDeclaration(_)
    ) && !matches!(
        model.nodes.parent_kind(model.nodes.parent_id(declaration)),
        AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
    )
}

/// A consumer-provided binding declared outside this function: parameters,
/// catches, loop targets and redeclarations never qualify.
fn is_captured(model: &Model<'_, '_>, site: FunctionSite, symbol: SymbolId) -> bool {
    if site
        .span
        .contains_inclusive(model.scoping.symbol_span(symbol))
        || model.formal_symbol_slot(symbol).is_some()
        || !model.scoping.symbol_redeclarations(symbol).is_empty()
    {
        return false;
    }
    matches!(
        model.nodes.kind(model.scoping.symbol_declaration(symbol)),
        AstKind::VariableDeclarator(_) | AstKind::Function(_) | AstKind::Class(_)
    )
}

fn created_custom_event(model: &Model<'_, '_>, expression: &Expression<'_>) -> bool {
    let Some(c) = call(expression) else {
        return false;
    };
    let Some(object) = member(&c.callee, "createEvent") else {
        return false;
    };
    if !is_global(model, object, "document") {
        return false;
    }
    let arguments = call_arguments(c);
    let [kind] = arguments.as_slice() else {
        return false;
    };
    matches!(kind.without_parentheses(), Expression::StringLiteral(s) if s.value == "CustomEvent")
}

/// The locally created `CustomEvent` receiver: a plain local of this function
/// whose effective value is one `document.createEvent("CustomEvent")` call.
fn created_receiver(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<SymbolId> {
    for statement in &body.statements {
        let Statement::VariableDeclaration(declaration) = statement else {
            continue;
        };
        for variable in &declaration.declarations {
            let BindingPattern::BindingIdentifier(id) = &variable.id else {
                continue;
            };
            let Some(s) = id.symbol_id.get() else {
                continue;
            };
            if !body_local(model, site, s) {
                continue;
            }
            let writes = model.binding_writes(s);
            if !((writes == 0 && variable.init.is_some())
                || (writes == 1 && variable.init.is_none()))
            {
                continue;
            }
            if model
                .symbol_value(s)
                .is_some_and(|value| created_custom_event(model, value))
            {
                return Some(s);
            }
        }
    }
    None
}

/// A nested callback owned by this exact function, resolved by symbol identity.
fn owned_callback(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
) -> Option<SymbolId> {
    let s = symbol(model, expression)?;
    let target = model.function_from_expression(expression)?;
    if target.span == site.span {
        return None;
    }
    (model
        .enclosing_function(target.node_id)
        .map(|parent| parent.span)
        == Some(site.span))
    .then_some(s)
}

/// The single `exchange` value of a literal object, if it has exactly that key.
fn exchange_value<'a>(object: &'a ObjectExpression<'a>) -> Option<&'a Expression<'a>> {
    let [ObjectPropertyKind::ObjectProperty(p)] = object.properties.as_slice() else {
        return None;
    };
    if p.computed || p.method || p.kind != PropertyKind::Init {
        return None;
    }
    (property_key_name(&p.key)?.as_str() == "exchange").then_some(&p.value)
}

fn object_exchange<'a>(expression: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    let Expression::ObjectExpression(object) = expression.without_parentheses() else {
        return None;
    };
    exchange_value(object)
}

/// A later `detail.exchange = callback` store into the same detail object.
fn exchange_store(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
    detail: SymbolId,
) -> Option<SymbolId> {
    let mut found = None;
    for statement in &body.statements {
        let Statement::ExpressionStatement(s) = statement else {
            continue;
        };
        let Expression::AssignmentExpression(a) = s.expression.without_parentheses() else {
            continue;
        };
        let AssignmentTarget::StaticMemberExpression(target) = &a.left else {
            continue;
        };
        if a.operator.as_str() != "="
            || target.optional
            || target.property.name != "exchange"
            || symbol(model, &target.object) != Some(detail)
        {
            continue;
        }
        let callback = owned_callback(model, site, &a.right)?;
        if found.replace(callback).is_some() {
            return None;
        }
    }
    found
}

/// The `{ exchange: <owned callback> }` detail, published either as a literal
/// object or as an `{ exchange: void 0 }` placeholder with one later store.
fn exchange_detail(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<(SymbolId, SymbolId)> {
    for statement in &body.statements {
        let Statement::VariableDeclaration(declaration) = statement else {
            continue;
        };
        for variable in &declaration.declarations {
            let BindingPattern::BindingIdentifier(id) = &variable.id else {
                continue;
            };
            let Some(d) = id.symbol_id.get() else {
                continue;
            };
            let Some(init) = variable.init.as_ref() else {
                continue;
            };
            if !body_local(model, site, d) {
                continue;
            }
            let Some(value) = object_exchange(init) else {
                continue;
            };
            if let Some(callback) = owned_callback(model, site, value) {
                return Some((d, callback));
            }
            if unset(value) {
                if let Some(callback) = exchange_store(model, site, body, d) {
                    return Some((d, callback));
                }
            }
        }
    }
    None
}

/// The closure variables the owned exchange callback fills from its own formals.
/// Its whole body must be those copies, so a decoy that computes a value cannot
/// stand in for the exchange slot writes.
fn callback_slots(
    model: &Model<'_, '_>,
    site: FunctionSite,
    callback: SymbolId,
) -> Option<BTreeSet<SymbolId>> {
    let target = model.function_from_symbol(callback)?;
    if target.span == site.span {
        return None;
    }
    let AstKind::Function(function) = model.nodes.kind(target.node_id) else {
        return None;
    };
    if function.r#async || function.generator {
        return None;
    }
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let params = plain_formals(model, &function.params)?;
    if params.is_empty() {
        return None;
    }
    let formals: BTreeSet<SymbolId> = params.into_iter().collect();
    let mut slots = BTreeSet::new();
    for statement in &body.statements {
        let Statement::ExpressionStatement(s) = statement else {
            return None;
        };
        let Expression::AssignmentExpression(a) = s.expression.without_parentheses() else {
            return None;
        };
        if a.operator.as_str() != "=" {
            return None;
        }
        let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
            return None;
        };
        let slot = crate::analysis::identifier_symbol(id, model.scoping)?;
        let value = symbol(model, &a.right)?;
        // The written slot is a closure variable of the callback, never one of
        // its own formals.
        if !formals.contains(&value)
            || target
                .span
                .contains_inclusive(model.scoping.symbol_span(slot))
        {
            return None;
        }
        slots.insert(slot);
    }
    (!slots.is_empty()).then_some(slots)
}

fn indexed(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    owner: SymbolId,
    index: SymbolId,
) -> bool {
    matches!(expression.without_parentheses(), Expression::ComputedMemberExpression(m)
        if !m.optional
            && symbol(model, &m.object) == Some(owner)
            && crate::analysis::expression_symbol(&m.expression, model.scoping) == Some(index))
}

/// The lowered `[receiver, receiver.initCustomEvent].call(receiver, ...)` node
/// shape is the same initCustomEvent operation as the direct member call.
fn lowered_init_custom_event(
    model: &Model<'_, '_>,
    site: FunctionSite,
    call: &CallExpression<'_>,
    receiver: SymbolId,
    names: SymbolId,
    counter: SymbolId,
    detail: SymbolId,
) -> bool {
    let Some(index_one) = member(&call.callee, "call") else {
        return false;
    };
    let Expression::ComputedMemberExpression(one) = index_one.without_parentheses() else {
        return false;
    };
    if one.optional || !number(&one.expression, 1.0) {
        return false;
    }
    let Some(list) = symbol(model, &one.object) else {
        return false;
    };
    if !body_local(model, site, list) {
        return false;
    }
    let Some(Expression::ArrayExpression(array)) =
        model.initializer(list).map(|e| e.without_parentheses())
    else {
        return false;
    };
    if array.elements.len() != 2 {
        return false;
    }
    let (Some(first), Some(second)) = (
        array.elements[0].as_expression(),
        array.elements[1].as_expression(),
    ) else {
        return false;
    };
    if symbol(model, first) != Some(receiver)
        || member(second, "initCustomEvent").and_then(|o| symbol(model, o)) != Some(receiver)
    {
        return false;
    }
    let arguments = call_arguments(call);
    let [self_arg, name, bubbles, cancelable, value] = arguments.as_slice() else {
        return false;
    };
    let Expression::ComputedMemberExpression(zero) = self_arg.without_parentheses() else {
        return false;
    };
    !zero.optional
        && number(&zero.expression, 0.0)
        && symbol(model, &zero.object) == Some(list)
        && indexed(model, name, names, counter)
        && boolean(bubbles, false)
        && boolean(cancelable, true)
        && symbol(model, value) == Some(detail)
}

fn init_custom_event(
    model: &Model<'_, '_>,
    site: FunctionSite,
    call: &CallExpression<'_>,
    receiver: SymbolId,
    names: SymbolId,
    counter: SymbolId,
    detail: SymbolId,
) -> bool {
    if member(&call.callee, "initCustomEvent").and_then(|o| symbol(model, o)) == Some(receiver) {
        let arguments = call_arguments(call);
        let [name, bubbles, cancelable, value] = arguments.as_slice() else {
            return false;
        };
        return indexed(model, name, names, counter)
            && boolean(bubbles, false)
            && boolean(cancelable, true)
            && symbol(model, value) == Some(detail);
    }
    lowered_init_custom_event(model, site, call, receiver, names, counter, detail)
}

/// A probe loop over the captured names whose body initializes the event and
/// aborts to `undefined` when a page listener cancels the dispatch.
fn dispatch_loop(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    receiver: SymbolId,
    detail: SymbolId,
) -> Option<SymbolId> {
    let (test, body) = loop_parts(statement)?;
    let Expression::BinaryExpression(b) = test.without_parentheses() else {
        return None;
    };
    if b.operator.as_str() != "<" {
        return None;
    }
    let counter = symbol(model, &b.left)?;
    let names = symbol(model, member(&b.right, "length")?)?;
    if !is_captured(model, site, names) {
        return None;
    }
    let statements = block(body)?;
    let initializes = statements.iter().any(|s| {
        statement_call(s)
            .is_some_and(|c| init_custom_event(model, site, c, receiver, names, counter, detail))
    });
    let aborts = statements
        .iter()
        .any(|s| dispatch_abort(model, s, receiver));
    (initializes && aborts).then_some(names)
}

fn dispatch_abort(model: &Model<'_, '_>, statement: &Statement<'_>, receiver: SymbolId) -> bool {
    let Statement::IfStatement(branch) = statement else {
        return false;
    };
    let Some(test) = call(&branch.test) else {
        return false;
    };
    if expression_path(model, &test.callee) != ["dispatchEvent"] {
        return false;
    }
    let arguments = call_arguments(test);
    if arguments.len() != 1 || symbol(model, arguments[0]) != Some(receiver) {
        return false;
    }
    let Some(consequent) = block(&branch.consequent) else {
        return false;
    };
    if !consequent
        .iter()
        .any(|s| matches!(s, Statement::ContinueStatement(_)))
    {
        return false;
    }
    let Some(alternate) = branch.alternate.as_ref().and_then(block) else {
        return false;
    };
    alternate.len() == 1 && returns_undefined(&alternate[0])
}

fn prevent_default_listener(
    model: &Model<'_, '_>,
    site: FunctionSite,
    listener: SymbolId,
) -> Option<()> {
    let target = model.function_from_symbol(listener)?;
    if target.span == site.span {
        return None;
    }
    if model
        .enclosing_function(target.node_id)
        .map(|parent| parent.span)
        != Some(site.span)
    {
        return None;
    }
    let AstKind::Function(function) = model.nodes.kind(target.node_id) else {
        return None;
    };
    if function.r#async || function.generator {
        return None;
    }
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let formals = plain_formals(model, &function.params)?;
    let [formal] = formals.as_slice() else {
        return None;
    };
    let [statement] = body.statements.as_slice() else {
        return None;
    };
    let c = statement_call(statement)?;
    if !c.arguments.is_empty() {
        return None;
    }
    (member(&c.callee, "preventDefault").and_then(|o| symbol(model, o)) == Some(*formal))
        .then_some(())
}

/// The capture handler binding has one stable function origin: a nested function
/// declaration that is never reassigned, or a plain local of this function with
/// exactly one write (an initializer, or one later plain assignment). A binding
/// written twice, redeclared, or declared elsewhere cannot smuggle an aliased
/// teardown past the owned-handler proof.
fn stable_listener(model: &Model<'_, '_>, site: FunctionSite, listener: SymbolId) -> bool {
    if !model.scoping.symbol_redeclarations(listener).is_empty() {
        return false;
    }
    let declaration = model.scoping.symbol_declaration(listener);
    match model.nodes.kind(declaration) {
        AstKind::Function(_) => model.binding_writes(listener) == 0,
        AstKind::VariableDeclarator(variable) => {
            body_local(model, site, listener)
                && ((model.binding_writes(listener) == 0 && variable.init.is_some())
                    || (model.binding_writes(listener) == 1 && variable.init.is_none()))
        }
        _ => false,
    }
}

/// The `removeEventListener(names[i], handler, true)` loop of an owned teardown
/// whose handler is the exact installed capture-listener symbol and whose name
/// list is a captured binding; the removed list identity is returned so the
/// caller can require the same list the handshake installs on.
fn self_removal(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    listener: SymbolId,
) -> Option<SymbolId> {
    let (test, body) = loop_parts(statement)?;
    let Expression::BinaryExpression(b) = test.without_parentheses() else {
        return None;
    };
    if b.operator.as_str() != "<" {
        return None;
    }
    let counter = symbol(model, &b.left)?;
    let names = symbol(model, member(&b.right, "length")?)?;
    if !is_captured(model, site, names) {
        return None;
    }
    for statement in block(body)? {
        let Some(c) = statement_call(statement) else {
            continue;
        };
        if !is_global(model, &c.callee, "removeEventListener") {
            continue;
        }
        let arguments = call_arguments(c);
        let [name, handler, capture] = arguments.as_slice() else {
            continue;
        };
        if boolean(capture, true)
            && indexed(model, name, names, counter)
            && symbol(model, handler) == Some(listener)
        {
            return Some(names);
        }
    }
    None
}

/// The guarded `formal.detail != null && typeof formal.detail.exchange ===
/// "function"` branch whose body calls `formal.detail.exchange(...)`, so the
/// owned handler is proven to actually emit the exchange rather than merely
/// carrying a name.
fn detail_exchange_emission(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    event: SymbolId,
) -> bool {
    let Statement::IfStatement(branch) = statement else {
        return false;
    };
    let Expression::LogicalExpression(and) = branch.test.without_parentheses() else {
        return false;
    };
    if and.operator.as_str() != "&&" {
        return false;
    }
    let null_guard = |expression: &Expression<'_>| {
        matches!(expression.without_parentheses(), Expression::BinaryExpression(b)
            if matches!(b.operator.as_str(), "!=" | "!==")
                && member(&b.left, "detail").and_then(|o| symbol(model, o)) == Some(event)
                && unset(&b.right))
    };
    let function_guard = |expression: &Expression<'_>| {
        let Expression::BinaryExpression(b) = expression.without_parentheses() else {
            return false;
        };
        if !matches!(b.operator.as_str(), "==" | "===") {
            return false;
        }
        let Expression::UnaryExpression(typeof_) = b.left.without_parentheses() else {
            return false;
        };
        typeof_.operator.as_str() == "typeof"
            && member(&typeof_.argument, "exchange")
                .and_then(|o| member(o, "detail"))
                .and_then(|o| symbol(model, o))
                == Some(event)
            && matches!(b.right.without_parentheses(), Expression::StringLiteral(s) if s.value == "function")
    };
    if !((null_guard(&and.left) && function_guard(&and.right))
        || (function_guard(&and.left) && null_guard(&and.right)))
    {
        return false;
    }
    let Some(consequent) = block(&branch.consequent) else {
        return false;
    };
    consequent.iter().any(|statement| {
        statement_call(statement).is_some_and(|c| {
            member(&c.callee, "exchange")
                .and_then(|o| member(o, "detail"))
                .and_then(|o| symbol(model, o))
                == Some(event)
        })
    })
}

/// The capture-phase handler is the owned exchange-teardown sibling of this same
/// function: resolved by symbol identity to a nested function of `site`, it
/// removes itself from the very captured name list the handshake installs on
/// (`removeEventListener(names[i], <the installed handler>, true)`) and emits
/// `formal.detail.exchange(...)` on its one formal under the null/typeof guard.
/// The handler's own body and identity decide the family; the installed name,
/// span, vendor and fingerprint never do, and a captured helper, a native
/// function or a twice-written binding cannot donate the handler.
fn teardown_listener(
    model: &Model<'_, '_>,
    site: FunctionSite,
    listener: SymbolId,
    names: SymbolId,
) -> Option<()> {
    if !stable_listener(model, site, listener) {
        return None;
    }
    let target = model.function_from_symbol(listener)?;
    if target.span == site.span
        || model
            .enclosing_function(target.node_id)
            .map(|parent| parent.span)
            != Some(site.span)
    {
        return None;
    }
    let AstKind::Function(function) = model.nodes.kind(target.node_id) else {
        return None;
    };
    if function.r#async || function.generator {
        return None;
    }
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let formals = plain_formals(model, &function.params)?;
    let [event] = formals.as_slice() else {
        return None;
    };
    let removes = body
        .statements
        .iter()
        .filter_map(|statement| self_removal(model, site, statement, listener))
        .any(|removed| removed == names);
    if !removes
        || !body
            .statements
            .iter()
            .any(|statement| detail_exchange_emission(model, statement, *event))
    {
        return None;
    }
    Some(())
}

/// The capture-phase `addEventListener(names[i], listener, true)` loop over the
/// same captured names.
fn listener_loop(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    names: SymbolId,
) -> Option<SymbolId> {
    let (test, body) = loop_parts(statement)?;
    let Expression::BinaryExpression(b) = test.without_parentheses() else {
        return None;
    };
    if b.operator.as_str() != "<" {
        return None;
    }
    let counter = symbol(model, &b.left)?;
    if symbol(model, member(&b.right, "length")?)? != names {
        return None;
    }
    for statement in block(body)? {
        let Some(c) = statement_call(statement) else {
            continue;
        };
        if !is_global(model, &c.callee, "addEventListener") {
            continue;
        }
        let arguments = call_arguments(c);
        let [name, listener, capture] = arguments.as_slice() else {
            continue;
        };
        if !boolean(capture, true) || !indexed(model, name, names, counter) {
            continue;
        }
        let Some(listener) = symbol(model, listener) else {
            continue;
        };
        if prevent_default_listener(model, site, listener).is_some()
            || teardown_listener(model, site, listener, names).is_some()
        {
            return Some(listener);
        }
    }
    None
}

fn loop_parts<'a>(statement: &'a Statement<'a>) -> Option<(&'a Expression<'a>, &'a Statement<'a>)> {
    match statement {
        Statement::WhileStatement(w) => Some((&w.test, &w.body)),
        Statement::ForStatement(f) => Some((f.test.as_ref()?, &f.body)),
        _ => None,
    }
}

/// The captured session construction whose arguments carry the exchange slots
/// and at least one of this function's own formals.
fn session_from_body(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
    slots: &BTreeSet<SymbolId>,
) -> Option<SymbolId> {
    for statement in &body.statements {
        let Statement::VariableDeclaration(declaration) = statement else {
            continue;
        };
        for variable in &declaration.declarations {
            let BindingPattern::BindingIdentifier(id) = &variable.id else {
                continue;
            };
            let Some(s) = id.symbol_id.get() else {
                continue;
            };
            if !body_local(model, site, s) {
                continue;
            }
            let writes = model.binding_writes(s);
            if !((writes == 0 && variable.init.is_some())
                || (writes == 1 && variable.init.is_none()))
            {
                continue;
            }
            let Some(Expression::NewExpression(new)) =
                model.symbol_value(s).map(|e| e.without_parentheses())
            else {
                continue;
            };
            let Some(callee) = symbol(model, &new.callee) else {
                continue;
            };
            if !is_captured(model, site, callee) {
                continue;
            }
            let arguments: Vec<&Expression<'_>> = new
                .arguments
                .iter()
                .filter_map(Argument::as_expression)
                .collect();
            if arguments.is_empty() {
                continue;
            }
            let present: BTreeSet<SymbolId> =
                arguments.iter().filter_map(|e| symbol(model, e)).collect();
            if slots.iter().all(|slot| present.contains(slot))
                && arguments
                    .iter()
                    .any(|e| model.site_parameter_slot(e, site).is_some())
            {
                return Some(s);
            }
        }
    }
    None
}

fn session_init(model: &Model<'_, '_>, body: &FunctionBody<'_>, session: SymbolId) -> bool {
    body.statements.iter().any(|statement| {
        statement_call(statement).is_some_and(|call| {
            call.arguments.is_empty()
                && member(&call.callee, "init").and_then(|o| symbol(model, o)) == Some(session)
        })
    })
}

fn handshake(model: &Model<'_, '_>, site: FunctionSite, function: &Function<'_>) -> Option<()> {
    if function.r#async || function.generator {
        return None;
    }
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    if plain_formals(model, &function.params)?.is_empty() {
        return None;
    }

    let receiver = created_receiver(model, site, body)?;
    let (detail, callback) = exchange_detail(model, site, body)?;
    let slots = callback_slots(model, site, callback)?;

    let names = body
        .statements
        .iter()
        .find_map(|s| dispatch_loop(model, site, s, receiver, detail))?;
    let listener = body
        .statements
        .iter()
        .find_map(|s| listener_loop(model, site, s, names))?;
    let session = session_from_body(model, site, body, &slots)?;
    if !session_init(model, body, session) {
        return None;
    }

    let mut distinct = BTreeSet::new();
    for identity in [receiver, names, detail, callback, listener, session] {
        if !distinct.insert(identity) {
            return None;
        }
    }
    if slots.iter().any(|slot| distinct.contains(slot)) {
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
    if function.body.is_none() || handshake(model, site, function).is_none() {
        return Vec::new();
    }
    vec![proposal(
        "bootstrapEventExchangeHandshake",
        "posthost_event_exchange_handshake",
        "whole owned body: a locally created CustomEvent receiver; a captured instrumented event-name array; an owned exchange callback published as the {exchange: ...} detail whose body copies its formals into the closure slots; a probe loop initializing the event on that receiver and aborting to undefined when a page listener cancels dispatchEvent; capture-phase listeners on the same names that are either an owned preventDefault stub or the owned exchange-teardown sibling proven by its own self-removal from that same captured name list plus a guarded formal.detail.exchange emission (identity from the body and symbol, never the installed name); and the captured session construction fed by the exchange slots plus its init call",
        f,
    )]
}
