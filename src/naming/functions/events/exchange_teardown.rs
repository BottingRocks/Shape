//! Bounded owned-body proof for the custom-event exchange teardown.
//!
//! One function's own statements are proved from source and the exact
//! scope-resolved participants; nothing is read from a generated spelling, a
//! thread id, a source position, a vendor, a hash or a pushed token literal:
//!
//! ```text
//! function teardown(event) {
//!   let i = 0;
//!   for (; i < names.length; i++) {
//!     removeEventListener(names[i], handler, true);
//!   }
//!   clearTimeout(timer);                     // or clearTimeout(0)
//!   const minutes = start
//!     ? base + (Date.now() - start) / 1e3 / 60
//!     : -1;
//!   if (event.detail != null
//!       && typeof event.detail.exchange === "function") {
//!     outA.push(<token>);                    // string literal or capture
//!     outB.push(payload);
//!     event.detail.exchange(outA, outB, minutes);
//!   }
//!   runtime.teardown();
//! }
//! ```
//!
//! Identity is by `SymbolId` only. `names`, `handler`, `timer`, `start`,
//! `base`, `outA`, `outB`, `payload` and `runtime` are plain variable bindings
//! declared outside the owned function. `handler` must resolve back to this
//! same function, so the removal loop can only remove the owned teardown and
//! never an arbitrary captured callback. The two output arrays are the exact
//! symbols the guarded `detail.exchange` call receives; the token is opaque
//! data. `Date`, `removeEventListener` and `clearTimeout` must be unshadowed
//! globals. Only the function's own direct statements are scanned, so a nested
//! decoy or an unrelated helper can never donate evidence.
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

fn unset(expression: &Expression<'_>) -> bool {
    match expression.without_parentheses() {
        Expression::NullLiteral(_) => true,
        Expression::UnaryExpression(u) => u.operator.as_str() == "void",
        Expression::Identifier(i) => i.name == "undefined",
        _ => false,
    }
}

fn negative_one(expression: &Expression<'_>) -> bool {
    match expression.without_parentheses() {
        Expression::NumericLiteral(n) => n.value == -1.0,
        Expression::UnaryExpression(u) => {
            u.operator.as_str() == "-"
                && matches!(&u.argument, Expression::NumericLiteral(n) if n.value == 1.0)
        }
        _ => false,
    }
}

fn division<'a>(
    expression: &'a Expression<'a>,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::BinaryExpression(b) = expression.without_parentheses() else {
        return None;
    };
    (b.operator.as_str() == "/").then_some((&b.left, &b.right))
}

/// A plain `var`/`let`/`const` binding declared outside the owned function.
/// Parameters, catch bindings, loop targets, function and class declarations
/// never establish a captured identity.
fn captured_symbol(model: &Model<'_, '_>, site: FunctionSite, captured: SymbolId) -> bool {
    if site
        .span
        .contains_inclusive(model.scoping.symbol_span(captured))
        || !model.scoping.symbol_redeclarations(captured).is_empty()
    {
        return false;
    }
    let declaration = model.scoping.symbol_declaration(captured);
    let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else {
        return false;
    };
    matches!(&variable.id, BindingPattern::BindingIdentifier(id)
            if id.symbol_id.get() == Some(captured))
        && matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(_)
        )
        && !matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
}

fn captured(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
) -> Option<SymbolId> {
    let captured = symbol(model, expression)?;
    captured_symbol(model, site, captured).then_some(captured)
}

fn for_loop<'a>(statement: &'a Statement<'a>) -> Option<(&'a Expression<'a>, &'a Statement<'a>)> {
    let Statement::ForStatement(f) = statement else {
        return None;
    };
    Some((f.test.as_ref()?, &f.body))
}

/// `NAMES.length` where `NAMES` is the exact captured name list.
fn name_list(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<SymbolId> {
    symbol(model, member(expression, "length")?)
}

/// The removal loop: `for (; i < names.length; i++) { removeEventListener(
/// names[i], handler, true); }` with one captured name list and one captured
/// handler that resolves back to this same owned function.
fn removal_loop(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
) -> Option<(SymbolId, SymbolId)> {
    let (test, body) = for_loop(statement)?;
    let Expression::BinaryExpression(b) = test.without_parentheses() else {
        return None;
    };
    if b.operator.as_str() != "<" {
        return None;
    }
    let counter = symbol(model, &b.left)?;
    let names = name_list(model, &b.right)?;
    if !captured_symbol(model, site, names) {
        return None;
    }
    let [statement] = block(body)? else {
        return None;
    };
    let c = statement_call(statement)?;
    if !is_global(model, &c.callee, "removeEventListener") {
        return None;
    }
    let arguments = call_arguments(c);
    let [name, handler, capture] = arguments.as_slice() else {
        return None;
    };
    if !boolean(capture, true) {
        return None;
    }
    let Expression::ComputedMemberExpression(indexed) = name.without_parentheses() else {
        return None;
    };
    if indexed.optional
        || symbol(model, &indexed.object) != Some(names)
        || crate::analysis::expression_symbol(&indexed.expression, model.scoping) != Some(counter)
    {
        return None;
    }
    let handler = captured(model, site, handler)?;
    if model
        .function_from_symbol(handler)
        .map(|target| target.span)
        != Some(site.span)
    {
        return None;
    }
    Some((names, handler))
}

/// `clearTimeout(timer)` with either a captured timer binding or the literal
/// zero variant. `Some(None)` records the literal-zero variant.
fn clear_timeout(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
) -> Option<Option<SymbolId>> {
    let c = statement_call(statement)?;
    if !is_global(model, &c.callee, "clearTimeout") {
        return None;
    }
    let arguments = call_arguments(c);
    let [timer] = arguments.as_slice() else {
        return None;
    };
    if number(timer, 0.0) {
        return Some(None);
    }
    Some(Some(captured(model, site, timer)?))
}

/// `Date.now()` with the unshadowed global `Date`.
fn date_now(model: &Model<'_, '_>, expression: &Expression<'_>) -> bool {
    let Some(c) = call(expression) else {
        return false;
    };
    member(&c.callee, "now").is_some_and(|object| is_global(model, object, "Date"))
        && c.arguments.is_empty()
}

/// `const minutes = start ? base + (Date.now() - start) / 1e3 / 60 : -1;`
/// where the same captured `start` feeds the test and the subtraction.
fn elapsed_minutes(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
) -> Option<(SymbolId, SymbolId, SymbolId)> {
    let Statement::VariableDeclaration(declaration) = statement else {
        return None;
    };
    let [variable] = declaration.declarations.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(id) = &variable.id else {
        return None;
    };
    let elapsed = id.symbol_id.get()?;
    if !site
        .span
        .contains_inclusive(model.scoping.symbol_span(elapsed))
        || !model.scoping.symbol_redeclarations(elapsed).is_empty()
    {
        return None;
    }
    let Expression::ConditionalExpression(conditional) =
        variable.init.as_ref()?.without_parentheses()
    else {
        return None;
    };
    let start = captured(model, site, &conditional.test)?;
    let Expression::BinaryExpression(plus) = conditional.consequent.without_parentheses() else {
        return None;
    };
    if plus.operator.as_str() != "+" {
        return None;
    }
    let base = captured(model, site, &plus.left)?;
    let (after_milliseconds, seconds) = division(&plus.right)?;
    let (clock_delta, milliseconds) = division(after_milliseconds)?;
    if !number(seconds, 60.0) || !number(milliseconds, 1000.0) {
        return None;
    }
    let Expression::BinaryExpression(subtraction) = clock_delta.without_parentheses() else {
        return None;
    };
    if subtraction.operator.as_str() != "-"
        || !date_now(model, &subtraction.left)
        || symbol(model, &subtraction.right) != Some(start)
        || !negative_one(&conditional.alternate)
    {
        return None;
    }
    Some((elapsed, start, base))
}

/// `event.detail` where `event` is the exact formal.
fn detail(model: &Model<'_, '_>, expression: &Expression<'_>, event: SymbolId) -> bool {
    member(expression, "detail").and_then(|object| symbol(model, object)) == Some(event)
}

/// `event.detail != null` (the unset forms are the same null guard).
fn detail_null_guard(model: &Model<'_, '_>, expression: &Expression<'_>, event: SymbolId) -> bool {
    let Expression::BinaryExpression(b) = expression.without_parentheses() else {
        return false;
    };
    matches!(b.operator.as_str(), "!=" | "!==") && detail(model, &b.left, event) && unset(&b.right)
}

/// `typeof event.detail.exchange === "function"`.
fn detail_function_guard(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    event: SymbolId,
) -> bool {
    let Expression::BinaryExpression(b) = expression.without_parentheses() else {
        return false;
    };
    if !matches!(b.operator.as_str(), "==" | "===") {
        return false;
    }
    let Expression::UnaryExpression(typeof_) = b.left.without_parentheses() else {
        return false;
    };
    if typeof_.operator.as_str() != "typeof" {
        return false;
    }
    let Some(target) = member(&typeof_.argument, "exchange") else {
        return false;
    };
    detail(model, target, event)
        && matches!(b.right.without_parentheses(), Expression::StringLiteral(s) if s.value == "function")
}

/// `array.push(value)` whose target is one captured output array.
fn push_call<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &'a Statement<'a>,
) -> Option<(SymbolId, &'a Expression<'a>)> {
    let c = statement_call(statement)?;
    let array = captured(model, site, member(&c.callee, "push")?)?;
    let arguments = call_arguments(c);
    let [value] = arguments.as_slice() else {
        return None;
    };
    Some((array, value))
}

/// The token is opaque data: either a string literal or a captured binding.
/// A captured token keeps its identity so it can be kept distinct from every
/// other participant.
fn token(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
) -> Option<Option<SymbolId>> {
    if matches!(
        expression.without_parentheses(),
        Expression::StringLiteral(_)
    ) {
        return Some(None);
    }
    Some(Some(captured(model, site, expression)?))
}

/// The guarded exchange branch, with the exact two captured output arrays and
/// the elapsed value.
fn exchange_branch(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    event: SymbolId,
    elapsed: SymbolId,
) -> Option<(SymbolId, SymbolId, Option<SymbolId>, SymbolId)> {
    let Statement::IfStatement(branch) = statement else {
        return None;
    };
    if branch.alternate.is_some() {
        return None;
    }
    let Expression::LogicalExpression(and) = branch.test.without_parentheses() else {
        return None;
    };
    if and.operator.as_str() != "&&" {
        return None;
    }
    let null_first = detail_null_guard(model, &and.left, event)
        && detail_function_guard(model, &and.right, event);
    let function_first = detail_function_guard(model, &and.left, event)
        && detail_null_guard(model, &and.right, event);
    if !null_first && !function_first {
        return None;
    }
    let [push_a, push_b, exchange] = block(&branch.consequent)? else {
        return None;
    };
    let (out_a, token_value) = push_call(model, site, push_a)?;
    let token_identity = token(model, site, token_value)?;
    let (out_b, payload) = push_call(model, site, push_b)?;
    let payload = captured(model, site, payload)?;
    if out_a == out_b {
        return None;
    }
    let c = statement_call(exchange)?;
    let Some(target) = member(&c.callee, "exchange") else {
        return None;
    };
    if !detail(model, target, event) {
        return None;
    }
    let arguments = call_arguments(c);
    let [first, second, minutes] = arguments.as_slice() else {
        return None;
    };
    if symbol(model, first) != Some(out_a)
        || symbol(model, second) != Some(out_b)
        || symbol(model, minutes) != Some(elapsed)
    {
        return None;
    }
    Some((out_a, out_b, token_identity, payload))
}

/// `runtime.teardown()` with no arguments and a captured runtime.
fn runtime_teardown(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
) -> Option<SymbolId> {
    let c = statement_call(statement)?;
    if !c.arguments.is_empty() {
        return None;
    }
    captured(model, site, member(&c.callee, "teardown")?)
}

fn exchange_teardown(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if function.r#async || function.generator || function.params.rest.is_some() {
        return None;
    }
    let [parameter] = function.params.items.as_slice() else {
        return None;
    };
    if parameter.initializer.is_some() {
        return None;
    }
    let BindingPattern::BindingIdentifier(id) = &parameter.pattern else {
        return None;
    };
    let event = id.symbol_id.get()?;
    if model.binding_writes(event) != 0 || !model.scoping.symbol_redeclarations(event).is_empty() {
        return None;
    }

    let (names, handler) = body
        .statements
        .iter()
        .find_map(|statement| removal_loop(model, site, statement))?;
    let timer = body
        .statements
        .iter()
        .find_map(|statement| clear_timeout(model, site, statement))?;
    let (elapsed, start, base) = body
        .statements
        .iter()
        .find_map(|statement| elapsed_minutes(model, site, statement))?;
    let (out_a, out_b, token_identity, payload) = body
        .statements
        .iter()
        .find_map(|statement| exchange_branch(model, site, statement, event, elapsed))?;
    let runtime = runtime_teardown(model, site, body.statements.last()?)?;

    let mut distinct = BTreeSet::new();
    for identity in [names, handler, out_a, out_b, payload, start, base, runtime] {
        if !distinct.insert(identity) {
            return None;
        }
    }
    if let Some(timer) = timer {
        if !distinct.insert(timer) {
            return None;
        }
    }
    if let Some(token_identity) = token_identity {
        if !distinct.insert(token_identity) {
            return None;
        }
    }
    (!distinct.contains(&elapsed)).then_some(())
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
    if !body.directives.is_empty() || exchange_teardown(model, site, function, body).is_none() {
        return Vec::new();
    }
    vec![proposal(
        "emitExchangeTeardown",
        "posthost_exchange_teardown",
        "whole owned body: one formal event; a removal loop over a captured name list with the same captured handler resolving back to this owned function and the literal capture flag; clearTimeout of a captured timer or literal zero; a Date.now() elapsed-minutes ternary sharing one captured start and a captured base; a null/typeof-function guard on the formal's detail.exchange; two distinct captured output arrays filled from an opaque token and a captured payload, then passed unchanged with the elapsed value to formal.detail.exchange; and a final captured runtime.teardown() with no arguments; local spellings, token literals, vendors, spans and hashes are not evidence",
        f,
    )]
}
