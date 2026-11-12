//! Bounded, owned-body proof for the UI-event metadata descriptor that the
//! interaction sample recorders consume.
//!
//! The logical family is one descriptor helper. It observes the event object
//! through the exact formal, builds an owned `{ id, name, height, width }`
//! target snapshot beside a two-field `{ instanceOfUIEvent, markedAsTrusted }`
//! record, optionally classifies `key`/`which`/`keyCode`, optionally rounds the
//! pointer geometry, and hands the resulting sample to one captured ring-buffer
//! `put` or returns it.
//!
//! Three source shapes are proved, each with the semantic role it actually
//! proves:
//!
//! ```text
//! function keyboardSample(typeCode, event) {          // recordKeyboardEventSample
//!   <target snapshot: event.target id/name/offsetHeight/offsetWidth or null>
//!   let meta = { instanceOfUIEvent: event instanceof UIEvent,
//!                markedAsTrusted: !!event.isTrusted };
//!   <key/which/keyCode classification -> key>
//!   let payload = { a: <boolean tape>, b: <captured clock>() };
//!   payload.c = [!!event.ctrlKey, !!event.metaKey, !!event.altKey, !!event.shiftKey];
//!   payload.d = key;
//!   payload.e = target.id;
//!   payload.f = target.name;
//!   payload.g = meta.instanceOfUIEvent;
//!   payload.h = meta.markedAsTrusted;
//!   ring.put.call(ring, payload);
//! }
//!
//! function mouseSample(event) {                       // recordMouseEventSample
//!   <target snapshot>
//!   let meta = <the same two-field record>;
//!   <bounds = target instanceof Element ? target.getBoundingClientRect() : null>
//!   <client/page-rounded coordinates>
//!   ring.put({ a: clock(), b/c: page rounding, d: booleans,
//!              e/f: meta, g/h/i/l: target box, j/k: rounded client,
//!              m: event.button });
//! }
//!
//! function pointerSample(event) {                     // recordPointerEventSample
//!   let meta = <the same two-field record>;
//!   ring.put({ a: clock(), b: event.pageX, c: event.pageY,
//!              d/e: meta.instanceOfUIEvent / meta.markedAsTrusted });
//! }
//! ```
//!
//! A fourth observed shape is the vendor-neutral touch-sample variant of the
//! same descriptor: it selects `changedTouches`/`touches` from the event, walks
//! each touch's `pageX`/`pageY`/`clientX`/`clientY`/`force` sample, and puts a
//! payload whose `c` list is that sample array with the same two-field record in
//! `d`/`e`. It keeps the existing readable role `recordTouchSample`.
//!
//! Every participant is a scope-resolved symbol. The event formal is tied by
//! [`SymbolId`] to each `target.id`/`name`/`offsetHeight`/`offsetWidth` read,
//! to the `instanceof UIEvent` / `!!isTrusted` record, to the keyboard
//! classification and to the geometry coordinates; a local may stand between
//! the formal and the read only when its preceding writes are all that formal's
//! field or the null fallback. The ring buffer may be read directly or through
//! a single-origin alias, and its `put` may be invoked directly or through the
//! recovered `put.call(receiver, payload)` form. `UIEvent`, `Element`, `Math`,
//! `RegExp` and friends must be the unshadowed globals.
//!
//! A nested function or arrow owned by the site refuses the whole proof, so an
//! outer container never borrows the descriptor from a child. The payload seed
//! and its later field writes form one record symbol; every expected key is
//! checked exactly once and no other write may be attached. Emitting the record
//! is proved either by the captured `put`/`put.call` above or by returning it as
//! the sole final statement.
//!
//! Names, vendors, thread ids, source offsets, body hashes and inherited labels
//! never participate. The older stages are untouched: this rule is only reached
//! after every earlier stage has declined the function.
use super::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::SymbolId;
use oxc_span::{GetSpan, Span};

type Symbol = SymbolId;

const KEYBOARD_SAMPLE: &str = "recordKeyboardEventSample";
const MOUSE_SAMPLE: &str = "recordMouseEventSample";
const POINTER_SAMPLE: &str = "recordPointerEventSample";
const TOUCH_SAMPLE: &str = "recordTouchSample";
const EVALUATOR: &str = "thread_posthost_event_metadata";

// ===========================================================================
// Scoped expression helpers
// ===========================================================================

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(e.without_parentheses(), model.scoping)
}

fn is_symbol(model: &Model<'_, '_>, e: &Expression<'_>, expected: Symbol) -> bool {
    symbol(model, e) == Some(expected)
}

fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = e.without_parentheses() else {
        return None;
    };
    (!m.optional && m.property.name.as_str() == key).then_some(&m.object)
}

fn indexed<'a>(e: &'a Expression<'a>) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::ComputedMemberExpression(m) = e.without_parentheses() else {
        return None;
    };
    (!m.optional).then_some((&m.object, &m.expression))
}

fn call<'a>(e: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(c) = e.without_parentheses() else {
        return None;
    };
    (!c.optional).then_some(c)
}

fn conditional<'a>(e: &'a Expression<'a>) -> Option<&'a ConditionalExpression<'a>> {
    let Expression::ConditionalExpression(c) = e.without_parentheses() else {
        return None;
    };
    Some(c)
}

fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(b.body.as_slice())
}

fn number(e: &Expression<'_>, n: f64) -> bool {
    matches!(e.without_parentheses(), Expression::NumericLiteral(v) if v.value == n)
}

fn boolean(e: &Expression<'_>, value: bool) -> bool {
    matches!(e.without_parentheses(), Expression::BooleanLiteral(v) if v.value == value)
}

fn string(e: &Expression<'_>, value: &str) -> bool {
    matches!(e.without_parentheses(), Expression::StringLiteral(v) if v.value == value)
}

fn null_literal(e: &Expression<'_>) -> bool {
    matches!(e.without_parentheses(), Expression::NullLiteral(_))
}

fn empty_array(e: &Expression<'_>) -> bool {
    matches!(e.without_parentheses(), Expression::ArrayExpression(a) if a.elements.is_empty())
}

fn binary<'a>(e: &'a Expression<'a>, op: &str) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::BinaryExpression(b) = e.without_parentheses() else {
        return None;
    };
    (b.operator.as_str() == op).then_some((&b.left, &b.right))
}

fn logical<'a>(
    e: &'a Expression<'a>,
    op: &str,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::LogicalExpression(l) = e.without_parentheses() else {
        return None;
    };
    (l.operator.as_str() == op).then_some((&l.left, &l.right))
}

fn global_path(model: &Model<'_, '_>, e: &Expression<'_>, path: &[&str]) -> bool {
    expression_path(model, e) == path
}

fn object_entries<'a>(
    object: &'a ObjectExpression<'a>,
) -> Option<Vec<(String, &'a Expression<'a>)>> {
    let mut keys = BTreeSet::new();
    let mut entries = Vec::with_capacity(object.properties.len());
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(p) = property else {
            return None;
        };
        if p.computed || p.method || p.kind != PropertyKind::Init {
            return None;
        }
        let key = property_key_name(&p.key)?;
        if !keys.insert(key.clone()) {
            return None;
        }
        entries.push((key, &p.value));
    }
    Some(entries)
}

// ===========================================================================
// Scoped binding helpers
// ===========================================================================

fn plain_declarator(model: &Model<'_, '_>, d: &VariableDeclarator<'_>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(id) = &d.id else {
        return None;
    };
    let s = id.symbol_id.get()?;
    let declaration = model.scoping.symbol_declaration(s);
    if !matches!(
        model.nodes.kind(declaration),
        AstKind::VariableDeclarator(_)
    ) || !matches!(
        model.nodes.parent_kind(declaration),
        AstKind::VariableDeclaration(_)
    ) || matches!(
        model.nodes.parent_kind(model.nodes.parent_id(declaration)),
        AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
    ) || !model.scoping.symbol_redeclarations(s).is_empty()
    {
        return None;
    }
    Some(s)
}

fn declared_in_body(model: &Model<'_, '_>, site: FunctionSite, s: Symbol) -> bool {
    let span = model.scoping.symbol_span(s);
    let body = site.body_span.unwrap_or(site.span);
    span.start >= body.start && span.end <= body.end
}

/// A plain single-origin local of the site's own body.
fn local_symbol(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<Symbol> {
    let s = symbol(model, e)?;
    if !matches!(
        model.nodes.kind(model.scoping.symbol_declaration(s)),
        AstKind::VariableDeclarator(_)
    ) || !declared_in_body(model, site, s)
        || model.formal_symbol_slot(s).is_some()
        || !model.scoping.symbol_redeclarations(s).is_empty()
    {
        return None;
    }
    Some(s)
}

/// A receiver/callable identity declared before the site and never redeclared.
fn capture(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<Symbol> {
    let s = symbol(model, e)?;
    let span = model.scoping.symbol_span(s);
    (!site.span.contains_inclusive(span)
        && span.end < site.span.start
        && model.scoping.symbol_redeclarations(s).is_empty())
    .then_some(s)
}

/// Every value written to a binding: its initializer plus each plain `=`
/// assignment. Compound, update or destructuring writes refuse the binding.
fn write_values<'a>(
    model: &Model<'_, 'a>,
    symbol: Symbol,
) -> Option<Vec<(&'a Expression<'a>, Span)>> {
    let mut values = Vec::new();
    if let Some(init) = model.initializer(symbol) {
        values.push((init, init.span()));
    }
    for reference in model.scoping.get_resolved_references(symbol) {
        if !reference.is_write() {
            continue;
        }
        let parent = model.nodes.parent_id(reference.node_id());
        let AstKind::AssignmentExpression(assignment) = model.nodes.kind(parent) else {
            return None;
        };
        if assignment.operator.as_str() != "=" {
            return None;
        }
        let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
            return None;
        };
        if crate::analysis::identifier_symbol(target, model.scoping) != Some(symbol) {
            return None;
        }
        values.push((&assignment.right, assignment.right.span()));
    }
    Some(values)
}

/// The nearest write whose value ends before `point`.
fn nearest<'a>(values: &[(&'a Expression<'a>, Span)], point: Span) -> Option<&'a Expression<'a>> {
    let mut best: Option<(&'a Expression<'a>, Span)> = None;
    for (value, span) in values {
        if span.end > point.start {
            continue;
        }
        if best.is_none_or(|(_, current)| span.start >= current.start) {
            best = Some((*value, *span));
        }
    }
    best.map(|(value, _)| value)
}

// ===========================================================================
// Owned statement scan
// ===========================================================================

/// One owned body reduced to the statement-level constructions the descriptor
/// is built from. Nested functions are refused before this scan runs.
#[derive(Default)]
struct BodyScan<'a> {
    declarators: Vec<&'a VariableDeclarator<'a>>,
    assignments: Vec<&'a AssignmentExpression<'a>>,
    calls: Vec<&'a CallExpression<'a>>,
    returns: Vec<&'a Expression<'a>>,
    fors: Vec<&'a ForStatement<'a>>,
}

fn collect_body<'a>(body: &'a FunctionBody<'a>) -> BodyScan<'a> {
    let mut scan = BodyScan::default();
    for statement in &body.statements {
        collect_statement(statement, &mut scan);
    }
    scan
}

fn collect_statement<'a>(statement: &'a Statement<'a>, scan: &mut BodyScan<'a>) {
    match statement {
        Statement::VariableDeclaration(declaration) => {
            for declarator in &declaration.declarations {
                scan.declarators.push(declarator);
            }
        }
        Statement::ExpressionStatement(statement) => {
            let expression = &statement.expression;
            if let Expression::AssignmentExpression(assignment) = expression {
                scan.assignments.push(assignment);
            } else if let Expression::CallExpression(call) = expression {
                scan.calls.push(call);
            }
        }
        Statement::IfStatement(branch) => {
            collect_statement(&branch.consequent, scan);
            if let Some(alternate) = branch.alternate.as_ref() {
                collect_statement(alternate, scan);
            }
        }
        Statement::BlockStatement(block) => {
            for statement in &block.body {
                collect_statement(statement, scan);
            }
        }
        Statement::TryStatement(tried) => {
            for statement in &tried.block.body {
                collect_statement(statement, scan);
            }
            if let Some(handler) = tried.handler.as_ref() {
                for statement in &handler.body.body {
                    collect_statement(statement, scan);
                }
            }
            if let Some(finalizer) = tried.finalizer.as_ref() {
                for statement in &finalizer.body {
                    collect_statement(statement, scan);
                }
            }
        }
        Statement::ForStatement(loop_) => {
            scan.fors.push(&**loop_);
            collect_statement(&loop_.body, scan);
        }
        Statement::ForInStatement(loop_) => collect_statement(&loop_.body, scan),
        Statement::ForOfStatement(loop_) => collect_statement(&loop_.body, scan),
        Statement::WhileStatement(loop_) => collect_statement(&loop_.body, scan),
        Statement::DoWhileStatement(loop_) => collect_statement(&loop_.body, scan),
        Statement::LabeledStatement(labeled) => collect_statement(&labeled.body, scan),
        Statement::SwitchStatement(switch) => {
            for case in &switch.cases {
                for statement in &case.consequent {
                    collect_statement(statement, scan);
                }
            }
        }
        Statement::ReturnStatement(returned) => {
            if let Some(argument) = returned.argument.as_ref() {
                scan.returns.push(argument);
            }
        }
        _ => (),
    }
}

fn one_variable<'a>(statement: &'a Statement<'a>) -> Option<&'a VariableDeclarator<'a>> {
    let Statement::VariableDeclaration(declaration) = statement else {
        return None;
    };
    let [declarator] = declaration.declarations.as_slice() else {
        return None;
    };
    Some(declarator)
}

// ===========================================================================
// The two-field UIEvent record
// ===========================================================================

fn instance_of_event(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    event: Symbol,
    global: &str,
) -> bool {
    let Some((left, right)) = binary(e, "instanceof") else {
        return false;
    };
    is_symbol(model, left, event) && global_path(model, right, &[global])
}

fn double_not_member(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    receiver: Symbol,
    name: &str,
) -> bool {
    let Expression::UnaryExpression(outer) = e.without_parentheses() else {
        return false;
    };
    if outer.operator.as_str() != "!" {
        return false;
    }
    let Expression::UnaryExpression(inner) = outer.argument.without_parentheses() else {
        return false;
    };
    if inner.operator.as_str() != "!" {
        return false;
    }
    member(&inner.argument, name).is_some_and(|object| is_symbol(model, object, receiver))
}

fn meta_object(model: &Model<'_, '_>, object: &ObjectExpression<'_>, event: Symbol) -> bool {
    let Some(entries) = object_entries(object) else {
        return false;
    };
    if entries.len() != 2 {
        return false;
    }
    let mut ui_event = false;
    let mut trusted = false;
    for (key, value) in entries {
        match key.as_str() {
            "instanceOfUIEvent" => ui_event = instance_of_event(model, value, event, "UIEvent"),
            "markedAsTrusted" => {
                trusted = double_not_member(model, value, event, "isTrusted");
            }
            _ => return false,
        }
    }
    ui_event && trusted
}

/// The one owned `{ instanceOfUIEvent, markedAsTrusted }` record declaration.
fn meta_record(
    model: &Model<'_, '_>,
    site: FunctionSite,
    scan: &BodyScan<'_>,
    event: Symbol,
) -> Option<(Symbol, Span)> {
    let mut found = Vec::new();
    for &declarator in &scan.declarators {
        let Some(init) = declarator.init.as_ref() else {
            continue;
        };
        let Expression::ObjectExpression(object) = init.without_parentheses() else {
            continue;
        };
        if !meta_object(model, object, event) {
            continue;
        }
        let Some(s) = plain_declarator(model, declarator) else {
            continue;
        };
        if declared_in_body(model, site, s) && model.binding_writes(s) == 0 {
            found.push((s, object.span));
        }
    }
    found.sort_unstable_by_key(|(s, _)| *s);
    found.dedup_by_key(|(s, _)| *s);
    let [(s, span)] = found.as_slice() else {
        return None;
    };
    Some((*s, *span))
}

// ===========================================================================
// The target snapshot { id, name, height, width }
// ===========================================================================

fn direct_event_target(model: &Model<'_, '_>, e: &Expression<'_>, source: Symbol) -> bool {
    member(e, "target").is_some_and(|receiver| is_symbol(model, receiver, source))
}

fn direct_source_field(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    source: Symbol,
    field: &str,
) -> bool {
    member(e, field).is_some_and(|target| direct_event_target(model, target, source))
}

fn guarded_source_field(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    source: Symbol,
    field: &str,
) -> bool {
    let Some(choice) = conditional(e) else {
        return false;
    };
    let Some((left, right)) = binary(&choice.test, "!=") else {
        return false;
    };
    direct_event_target(model, left, source)
        && null_literal(right)
        && direct_source_field(model, &choice.consequent, source, field)
        && null_literal(&choice.alternate)
}

/// A target snapshot value: the same source field with a null fallback, directly
/// (guarded conditional) or through one preceding local write chain.
fn target_field(
    model: &Model<'_, '_>,
    site: FunctionSite,
    source: Symbol,
    e: &Expression<'_>,
    field: &str,
    point: Span,
    depth: usize,
) -> bool {
    if depth > 3 {
        return false;
    }
    if guarded_source_field(model, e, source, field) {
        return true;
    }
    let Some(local) = local_symbol(model, site, e) else {
        return false;
    };
    let Some(values) = write_values(model, local) else {
        return false;
    };
    let mut read = false;
    let mut fallback = false;
    for (value, span) in values {
        if span.end > point.start {
            continue;
        }
        if null_literal(value) {
            fallback = true;
            continue;
        }
        if direct_source_field(model, value, source, field) {
            read = true;
            continue;
        }
        if guarded_source_field(model, value, source, field) {
            read = true;
            continue;
        }
        if target_field(model, site, source, value, field, span, depth + 1) {
            read = true;
            continue;
        }
        return false;
    }
    read && fallback
}

fn target_is_size_field(key: &str) -> Option<&'static str> {
    match key {
        "id" => Some("id"),
        "name" => Some("name"),
        "height" => Some("offsetHeight"),
        "width" => Some("offsetWidth"),
        _ => None,
    }
}

fn target_snapshot(
    model: &Model<'_, '_>,
    site: FunctionSite,
    object: &ObjectExpression<'_>,
    event: Symbol,
) -> bool {
    let Some(entries) = object_entries(object) else {
        return false;
    };
    if entries.len() != 4 {
        return false;
    }
    for (key, value) in entries {
        let Some(field) = target_is_size_field(&key) else {
            return false;
        };
        if !target_field(model, site, event, value, field, value.span(), 0) {
            return false;
        }
    }
    true
}

fn target_object(
    model: &Model<'_, '_>,
    site: FunctionSite,
    scan: &BodyScan<'_>,
    event: Symbol,
) -> Option<(Symbol, Span)> {
    let mut found: Vec<(Symbol, Span)> = Vec::new();
    for &declarator in &scan.declarators {
        let Some(init) = declarator.init.as_ref() else {
            continue;
        };
        let Expression::ObjectExpression(object) = init.without_parentheses() else {
            continue;
        };
        if !target_snapshot(model, site, object, event) {
            continue;
        }
        if let Some(s) = plain_declarator(model, declarator) {
            if declared_in_body(model, site, s) {
                found.push((s, object.span));
            }
        }
    }
    for &assignment in &scan.assignments {
        let Expression::ObjectExpression(object) = assignment.right.without_parentheses() else {
            continue;
        };
        let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
            continue;
        };
        let Some(s) = crate::analysis::identifier_symbol(target, model.scoping) else {
            continue;
        };
        if !declared_in_body(model, site, s) || !target_snapshot(model, site, object, event) {
            continue;
        }
        found.push((s, object.span));
    }
    found.sort_unstable_by_key(|(s, _)| *s);
    found.dedup_by_key(|(s, _)| *s);
    let [(s, span)] = found.as_slice() else {
        return None;
    };
    Some((*s, *span))
}

// ===========================================================================
// Geometry: target box, page/client rounding
// ===========================================================================

fn round_member(model: &Model<'_, '_>, e: &Expression<'_>, source: Symbol, axis: &str) -> bool {
    let Some(round) = call(e) else {
        return false;
    };
    if !global_path(model, &round.callee, &["Math", "round"]) || round.arguments.len() != 1 {
        return false;
    }
    let args = call_arguments(round);
    args.first()
        .and_then(|value| member(value, axis))
        .is_some_and(|receiver| is_symbol(model, receiver, source))
}

/// One page coordinate: the event field itself, its `Math.round`, or the
/// `field == null ? null : Math.round(field)` fallback.
fn page_value(model: &Model<'_, '_>, e: &Expression<'_>, source: Symbol, axis: &str) -> bool {
    if member(e, axis).is_some_and(|receiver| is_symbol(model, receiver, source)) {
        return true;
    }
    if round_member(model, e, source, axis) {
        return true;
    }
    let Some(choice) = conditional(e) else {
        return false;
    };
    let Some((read, null)) = binary(&choice.test, "==") else {
        return false;
    };
    member(read, axis).is_some_and(|receiver| is_symbol(model, receiver, source))
        && null_literal(null)
        && null_literal(&choice.consequent)
        && round_member(model, &choice.alternate, source, axis)
}

fn page_before(
    model: &Model<'_, '_>,
    site: FunctionSite,
    source: Symbol,
    e: &Expression<'_>,
    axis: &str,
) -> bool {
    if page_value(model, e, source, axis) {
        return true;
    }
    let Some(local) = local_symbol(model, site, e) else {
        return false;
    };
    let Some(values) = write_values(model, local) else {
        return false;
    };
    nearest(&values, e.span()).is_some_and(|value| page_value(model, value, source, axis))
}

/// The `bounds instanceof Element ? bounds.getBoundingClientRect() : null`
/// step, with `bounds` or its alias holding the source's own `target`.
fn bounds_conditional(
    model: &Model<'_, '_>,
    source: Symbol,
    e: &Expression<'_>,
    point: Span,
) -> bool {
    let Some(choice) = conditional(e) else {
        return false;
    };
    let Some((target, global)) = binary(&choice.test, "instanceof") else {
        return false;
    };
    if !global_path(model, global, &["Element"]) {
        return false;
    }
    let Some(target) = symbol(model, target) else {
        return false;
    };
    let Some(rect) = call(&choice.consequent) else {
        return false;
    };
    if !rect.arguments.is_empty()
        || !member(&rect.callee, "getBoundingClientRect")
            .is_some_and(|receiver| is_symbol(model, receiver, target))
        || !null_literal(&choice.alternate)
    {
        return false;
    }
    let Some(values) = write_values(model, target) else {
        return false;
    };
    nearest(&values, point).is_some_and(|value| direct_event_target(model, value, source))
}

fn bounds_before(model: &Model<'_, '_>, source: Symbol, bounds: Symbol, point: Span) -> bool {
    let Some(values) = write_values(model, bounds) else {
        return false;
    };
    let mut boxed = false;
    for (value, span) in values {
        if span.end > point.start {
            continue;
        }
        if null_literal(value) || direct_event_target(model, value, source) {
            continue;
        }
        if bounds_conditional(model, source, value, span) {
            boxed = true;
            continue;
        }
        return false;
    }
    boxed
}

/// `bounds == null [|| source.axis == null] ? 0 : Math.round(source.axis - bounds.side)`
fn coordinate_value(
    model: &Model<'_, '_>,
    site: FunctionSite,
    source: Symbol,
    e: &Expression<'_>,
    axis: &str,
    side: &str,
) -> bool {
    let Some(choice) = conditional(e) else {
        return false;
    };
    let bounds = if let Some((left, right)) = logical(&choice.test, "||") {
        let Some((bounds, null)) = binary(left, "==") else {
            return false;
        };
        let Some((read, axis_null)) = binary(right, "==") else {
            return false;
        };
        if !null_literal(null)
            || !null_literal(axis_null)
            || !member(read, axis).is_some_and(|receiver| is_symbol(model, receiver, source))
        {
            return false;
        }
        bounds
    } else {
        let Some((bounds, null)) = binary(&choice.test, "==") else {
            return false;
        };
        if !null_literal(null) {
            return false;
        }
        bounds
    };
    let Some(bounds) = local_symbol(model, site, bounds) else {
        return false;
    };
    if !number(&choice.consequent, 0.0) {
        return false;
    }
    let Some(round) = call(&choice.alternate) else {
        return false;
    };
    if !global_path(model, &round.callee, &["Math", "round"]) || round.arguments.len() != 1 {
        return false;
    }
    let args = call_arguments(round);
    let Some((read, left)) = args.first().and_then(|value| binary(value, "-")) else {
        return false;
    };
    member(read, axis).is_some_and(|receiver| is_symbol(model, receiver, source))
        && member(left, side).is_some_and(|receiver| is_symbol(model, receiver, bounds))
        && bounds_before(model, source, bounds, e.span())
}

fn coordinate_before(
    model: &Model<'_, '_>,
    site: FunctionSite,
    source: Symbol,
    e: &Expression<'_>,
    axis: &str,
    side: &str,
) -> bool {
    if coordinate_value(model, site, source, e, axis, side) {
        return true;
    }
    let Some(local) = local_symbol(model, site, e) else {
        return false;
    };
    let Some(values) = write_values(model, local) else {
        return false;
    };
    nearest(&values, e.span())
        .is_some_and(|value| coordinate_value(model, site, source, value, axis, side))
}

// ===========================================================================
// Keyboard classification and boolean tapes
// ===========================================================================

struct KeyboardReads<'m, 's, 'a> {
    model: &'m Model<'s, 'a>,
    event: Symbol,
    found: bool,
}

impl<'m, 's, 'a> Visit<'a> for KeyboardReads<'m, 's, 'a> {
    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        let name = member.property.name.as_str();
        if !member.optional
            && (name == "key" || name == "which" || name == "keyCode")
            && symbol(self.model, &member.object) == Some(self.event)
        {
            self.found = true;
        }
        walk::walk_static_member_expression(self, member);
    }
}

fn keyboard_read<'a>(model: &Model<'_, 'a>, event: Symbol, e: &'a Expression<'a>) -> bool {
    let mut reads = KeyboardReads {
        model,
        event,
        found: false,
    };
    reads.visit_expression(e);
    reads.found
}

/// The key/which/keyCode classification reaches the event formal, directly or
/// through one single-origin local hop.
fn keyboard_derived(
    model: &Model<'_, '_>,
    site: FunctionSite,
    event: Symbol,
    e: &Expression<'_>,
    depth: usize,
) -> bool {
    if depth > 3 {
        return false;
    }
    let Some(local) = local_symbol(model, site, e) else {
        return false;
    };
    let Some(values) = write_values(model, local) else {
        return false;
    };
    for (value, _) in values {
        if keyboard_read(model, event, value) {
            return true;
        }
        if matches!(value.without_parentheses(), Expression::Identifier(_))
            && keyboard_derived(model, site, event, value, depth + 1)
        {
            return true;
        }
    }
    false
}

fn boolean_tape(e: &Expression<'_>) -> bool {
    let Expression::ArrayExpression(array) = e.without_parentheses() else {
        return false;
    };
    if array.elements.is_empty() || array.elements.len() > 2 {
        return false;
    }
    array.elements.iter().all(|element| {
        element
            .as_expression()
            .is_some_and(|value| boolean(value, true) || boolean(value, false))
    })
}

fn boolean_tape_before(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
    point: Span,
) -> bool {
    if boolean_tape(e) {
        return true;
    }
    let Some(local) = local_symbol(model, site, e) else {
        return false;
    };
    let Some(values) = write_values(model, local) else {
        return false;
    };
    let mut tape = false;
    for (value, span) in values {
        if span.end > point.start {
            continue;
        }
        if !boolean_tape(value) {
            return false;
        }
        tape = true;
    }
    tape
}

fn modifier_tape(model: &Model<'_, '_>, e: &Expression<'_>, event: Symbol) -> bool {
    let Expression::ArrayExpression(array) = e.without_parentheses() else {
        return false;
    };
    let keys = ["ctrlKey", "metaKey", "altKey", "shiftKey"];
    if array.elements.len() != keys.len() {
        return false;
    }
    array.elements.iter().zip(keys).all(|(element, key)| {
        element
            .as_expression()
            .is_some_and(|value| double_not_member(model, value, event, key))
    })
}

fn modifier_tape_before(
    model: &Model<'_, '_>,
    site: FunctionSite,
    event: Symbol,
    e: &Expression<'_>,
    point: Span,
) -> bool {
    if modifier_tape(model, e, event) {
        return true;
    }
    let Some(local) = local_symbol(model, site, e) else {
        return false;
    };
    let Some(values) = write_values(model, local) else {
        return false;
    };
    nearest(&values, point).is_some_and(|value| modifier_tape(model, value, event))
}

/// The sample timestamp provider: a zero-argument call of a value that is
/// already declared before the site (the captured clock provider) or of an
/// unshadowed global path. A local of the descriptor body never supplies it.
fn zero_arg_callable(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> bool {
    let Some(call) = call(e) else {
        return false;
    };
    if !call.arguments.is_empty() || !call_arguments(call).is_empty() {
        return false;
    }
    let callee = call.callee.without_parentheses();
    capture(model, site, callee).is_some() || !expression_path(model, callee).is_empty()
}

fn target_read(model: &Model<'_, '_>, e: &Expression<'_>, target: Symbol, key: &str) -> bool {
    member(e, key).is_some_and(|receiver| is_symbol(model, receiver, target))
}

fn meta_read(model: &Model<'_, '_>, e: &Expression<'_>, meta: Symbol, key: &str) -> bool {
    member(e, key).is_some_and(|receiver| is_symbol(model, receiver, meta))
}

// ===========================================================================
// Record assembly and the captured ring-buffer put
// ===========================================================================

#[derive(Clone, Copy)]
enum Origin<'a> {
    Value,
    Argument(&'a CallExpression<'a>),
    Return,
}

struct Record<'a> {
    symbol: Option<Symbol>,
    object: &'a ObjectExpression<'a>,
    origin: Origin<'a>,
}

fn records<'a>(model: &Model<'_, 'a>, site: FunctionSite, scan: &BodyScan<'a>) -> Vec<Record<'a>> {
    let mut out = Vec::new();
    for &declarator in &scan.declarators {
        let Some(init) = declarator.init.as_ref() else {
            continue;
        };
        let Expression::ObjectExpression(object) = init.without_parentheses() else {
            continue;
        };
        let Some(symbol) = plain_declarator(model, declarator) else {
            continue;
        };
        if declared_in_body(model, site, symbol) {
            out.push(Record {
                symbol: Some(symbol),
                object,
                origin: Origin::Value,
            });
        }
    }
    for &assignment in &scan.assignments {
        let Expression::ObjectExpression(object) = assignment.right.without_parentheses() else {
            continue;
        };
        let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
            continue;
        };
        let Some(symbol) = crate::analysis::identifier_symbol(target, model.scoping) else {
            continue;
        };
        if declared_in_body(model, site, symbol) {
            out.push(Record {
                symbol: Some(symbol),
                object,
                origin: Origin::Value,
            });
        }
    }
    for &call in &scan.calls {
        for argument in call_arguments(call) {
            let Expression::ObjectExpression(object) = argument.without_parentheses() else {
                continue;
            };
            out.push(Record {
                symbol: None,
                object,
                origin: Origin::Argument(call),
            });
        }
    }
    for &returned in &scan.returns {
        let Expression::ObjectExpression(object) = returned.without_parentheses() else {
            continue;
        };
        out.push(Record {
            symbol: None,
            object,
            origin: Origin::Return,
        });
    }
    out
}

fn record_fields<'a>(
    model: &Model<'_, '_>,
    scan: &BodyScan<'a>,
    object: &'a ObjectExpression<'a>,
    record: Option<Symbol>,
) -> Option<BTreeMap<String, &'a Expression<'a>>> {
    let mut fields = BTreeMap::new();
    for (key, value) in object_entries(object)? {
        fields.insert(key, value);
    }
    if let Some(record_symbol) = record {
        for &assignment in &scan.assignments {
            let AssignmentTarget::StaticMemberExpression(member) = &assignment.left else {
                continue;
            };
            if member.optional || symbol(model, &member.object) != Some(record_symbol) {
                continue;
            }
            let key = member.property.name.to_string();
            if fields.contains_key(&key) {
                return None;
            }
            fields.insert(key, &assignment.right);
        }
    }
    Some(fields)
}

fn keys_are(fields: &BTreeMap<String, &Expression<'_>>, expected: &[&str]) -> bool {
    fields.len() == expected.len() && expected.iter().all(|key| fields.contains_key(*key))
}

fn payload_matches(model: &Model<'_, '_>, e: &Expression<'_>, payload: Payload) -> bool {
    match payload {
        Payload::Symbol(symbol) => is_symbol(model, e, symbol),
        Payload::Object(span) => {
            let value = e.without_parentheses();
            value.span().start == span.start && value.span().end == span.end
        }
    }
}

#[derive(Clone, Copy)]
enum Payload {
    Symbol(Symbol),
    Object(Span),
}

fn receiver_value(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
    depth: usize,
) -> Option<Symbol> {
    if depth > 3 {
        return None;
    }
    if let Some(captured) = capture(model, site, e) {
        return Some(captured);
    }
    let local = local_symbol(model, site, e)?;
    let values = write_values(model, local)?;
    if values.len() != 1 {
        return None;
    }
    let (value, _) = values[0];
    receiver_value(model, site, value, depth + 1)
}

fn ring_receiver(
    model: &Model<'_, '_>,
    site: FunctionSite,
    call: &CallExpression<'_>,
) -> Option<Symbol> {
    if call.optional {
        return None;
    }
    let Expression::StaticMemberExpression(callee) = call.callee.without_parentheses() else {
        return None;
    };
    if callee.optional {
        return None;
    }
    match callee.property.name.as_str() {
        "put" => receiver_value(model, site, &callee.object, 0),
        "call" => {
            if let Expression::StaticMemberExpression(method) = callee.object.without_parentheses()
            {
                if !method.optional && method.property.name.as_str() == "put" {
                    return receiver_value(model, site, &method.object, 0);
                }
            }
            let holder = local_symbol(model, site, &callee.object)?;
            let values = write_values(model, holder)?;
            if values.len() != 1 {
                return None;
            }
            let (value, _) = values[0];
            let Expression::StaticMemberExpression(method) = value.without_parentheses() else {
                return None;
            };
            if method.optional || method.property.name.as_str() != "put" {
                return None;
            }
            receiver_value(model, site, &method.object, 0)
        }
        _ => None,
    }
}

fn ring_put(
    model: &Model<'_, '_>,
    site: FunctionSite,
    call: &CallExpression<'_>,
    payload: Payload,
) -> bool {
    let Some(receiver) = ring_receiver(model, site, call) else {
        return false;
    };
    let Expression::StaticMemberExpression(callee) = call.callee.without_parentheses() else {
        return false;
    };
    let arguments = call_arguments(call);
    if arguments.len() != call.arguments.len() {
        return false;
    }
    match callee.property.name.as_str() {
        "put" => arguments.len() == 1 && payload_matches(model, arguments[0], payload),
        "call" => {
            arguments.len() == 2
                && receiver_value(model, site, arguments[0], 0) == Some(receiver)
                && payload_matches(model, arguments[1], payload)
        }
        _ => false,
    }
}

fn returns_payload(
    model: &Model<'_, '_>,
    body: &FunctionBody<'_>,
    scan: &BodyScan<'_>,
    payload: Payload,
) -> bool {
    let [returned] = scan.returns.as_slice() else {
        return false;
    };
    let Some(Statement::ReturnStatement(last)) = body.statements.last() else {
        return false;
    };
    last.argument
        .as_ref()
        .is_some_and(|argument| argument.span() == returned.span())
        && payload_matches(model, returned, payload)
}

fn emitted(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
    scan: &BodyScan<'_>,
    record: &Record<'_>,
) -> bool {
    let payload = match record.symbol {
        Some(symbol) => Payload::Symbol(symbol),
        None => Payload::Object(record.object.span),
    };
    match record.origin {
        Origin::Argument(call) => {
            // Direct put checks this argument's exact object span; put.call
            // additionally checks that its explicit receiver is the same sink.
            // The record object is one of this call's own arguments, so it is
            // already inside the call: there is no separate earlier write to
            // order against. The captured receiver and the payload identity
            // are still proved by `ring_put`.
            ring_put(model, site, call, payload)
        }
        Origin::Return => returns_payload(model, body, scan, payload),
        Origin::Value => {
            scan.calls.iter().any(|call| {
                call.span.start > record.object.span.end && ring_put(model, site, call, payload)
            }) || returns_payload(model, body, scan, payload)
        }
    }
}

// ===========================================================================
// Variant proofs
// ===========================================================================

fn keyboard_sample<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    body: &'a FunctionBody<'a>,
    scan: &BodyScan<'a>,
    event: Symbol,
) -> bool {
    let Some((meta, meta_span)) = meta_record(model, site, scan, event) else {
        return false;
    };
    let Some((target, target_span)) = target_object(model, site, scan, event) else {
        return false;
    };
    let mut found = 0;
    for record in records(model, site, scan) {
        if !matches!(record.origin, Origin::Value) {
            continue;
        }
        let Some(record_symbol) = record.symbol else {
            continue;
        };
        let Some(fields) = record_fields(model, scan, record.object, Some(record_symbol)) else {
            continue;
        };
        if !keys_are(&fields, &["a", "b", "c", "d", "e", "f", "g", "h"]) {
            continue;
        }
        if meta_span.end > record.object.span.start || target_span.end > record.object.span.start {
            continue;
        }
        let (
            Some(tape),
            Some(clock),
            Some(modifiers),
            Some(classification),
            Some(id),
            Some(name),
            Some(ui),
            Some(trusted),
        ) = (
            fields.get("a"),
            fields.get("b"),
            fields.get("c"),
            fields.get("d"),
            fields.get("e"),
            fields.get("f"),
            fields.get("g"),
            fields.get("h"),
        )
        else {
            continue;
        };
        if !boolean_tape_before(model, site, tape, tape.span())
            || !zero_arg_callable(model, site, clock)
            || !modifier_tape_before(model, site, event, modifiers, modifiers.span())
            || !keyboard_derived(model, site, event, classification, 0)
            || !target_read(model, id, target, "id")
            || !target_read(model, name, target, "name")
            || !meta_read(model, ui, meta, "instanceOfUIEvent")
            || !meta_read(model, trusted, meta, "markedAsTrusted")
            || !emitted(model, site, body, scan, &record)
        {
            continue;
        }
        found += 1;
    }
    found == 1
}

fn mouse_sample<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    body: &'a FunctionBody<'a>,
    scan: &BodyScan<'a>,
    event: Symbol,
) -> bool {
    let Some((meta, meta_span)) = meta_record(model, site, scan, event) else {
        return false;
    };
    let Some((target, target_span)) = target_object(model, site, scan, event) else {
        return false;
    };
    let mut found = 0;
    for record in records(model, site, scan) {
        let Some(fields) = record_fields(model, scan, record.object, record.symbol) else {
            continue;
        };
        if !keys_are(
            &fields,
            &[
                "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m",
            ],
        ) {
            continue;
        }
        if meta_span.end > record.object.span.start || target_span.end > record.object.span.start {
            continue;
        }
        let (
            Some(clock),
            Some(page_x),
            Some(page_y),
            Some(tape),
            Some(ui),
            Some(trusted),
            Some(height),
            Some(id),
            Some(name),
            Some(x),
            Some(y),
            Some(width),
            Some(button),
        ) = (
            fields.get("a"),
            fields.get("b"),
            fields.get("c"),
            fields.get("d"),
            fields.get("e"),
            fields.get("f"),
            fields.get("g"),
            fields.get("h"),
            fields.get("i"),
            fields.get("j"),
            fields.get("k"),
            fields.get("l"),
            fields.get("m"),
        )
        else {
            continue;
        };
        if !zero_arg_callable(model, site, clock)
            || !page_before(model, site, event, page_x, "pageX")
            || !page_before(model, site, event, page_y, "pageY")
            || !boolean_tape(tape)
            || !meta_read(model, ui, meta, "instanceOfUIEvent")
            || !meta_read(model, trusted, meta, "markedAsTrusted")
            || !target_read(model, height, target, "height")
            || !target_read(model, id, target, "id")
            || !target_read(model, name, target, "name")
            || !target_read(model, width, target, "width")
            || !coordinate_before(model, site, event, x, "clientX", "left")
            || !coordinate_before(model, site, event, y, "clientY", "top")
            || !member(button, "button").is_some_and(|receiver| is_symbol(model, receiver, event))
            || !emitted(model, site, body, scan, &record)
        {
            continue;
        }
        found += 1;
    }
    found == 1
}

fn pointer_sample<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    body: &'a FunctionBody<'a>,
    scan: &BodyScan<'a>,
    event: Symbol,
) -> bool {
    let Some((meta, meta_span)) = meta_record(model, site, scan, event) else {
        return false;
    };
    let mut found = 0;
    for record in records(model, site, scan) {
        let Some(fields) = record_fields(model, scan, record.object, record.symbol) else {
            continue;
        };
        if !keys_are(&fields, &["a", "b", "c", "d", "e"]) {
            continue;
        }
        if meta_span.end > record.object.span.start {
            continue;
        }
        let (Some(clock), Some(page_x), Some(page_y), Some(ui), Some(trusted)) = (
            fields.get("a"),
            fields.get("b"),
            fields.get("c"),
            fields.get("d"),
            fields.get("e"),
        ) else {
            continue;
        };
        if !zero_arg_callable(model, site, clock)
            || !page_value(model, page_x, event, "pageX")
            || !page_value(model, page_y, event, "pageY")
            || !meta_read(model, ui, meta, "instanceOfUIEvent")
            || !meta_read(model, trusted, meta, "markedAsTrusted")
            || !emitted(model, site, body, scan, &record)
        {
            continue;
        }
        found += 1;
    }
    found == 1
}

fn touch_list_value(model: &Model<'_, '_>, e: &Expression<'_>, event: Symbol) -> bool {
    let Some((object, key)) = indexed(e) else {
        return false;
    };
    if !is_symbol(model, object, event) {
        return false;
    }
    let Some(choice) = conditional(key) else {
        return false;
    };
    if !string(&choice.consequent, "changedTouches") || !string(&choice.alternate, "touches") {
        return false;
    }
    let Some((has_touches, empty)) = logical(&choice.test, "&&") else {
        return false;
    };
    if !member(has_touches, "touches").is_some_and(|receiver| is_symbol(model, receiver, event)) {
        return false;
    }
    let Some((length, zero)) = binary(empty, "===") else {
        return false;
    };
    number(zero, 0.0)
        && member(length, "length")
            .and_then(|touches| member(touches, "touches"))
            .is_some_and(|receiver| is_symbol(model, receiver, event))
}

fn touch_list_selection(model: &Model<'_, '_>, list: Symbol, event: Symbol, point: Span) -> bool {
    let Some(values) = write_values(model, list) else {
        return false;
    };
    nearest(&values, point).is_some_and(|value| touch_list_value(model, value, event))
}

fn touch_object(
    model: &Model<'_, '_>,
    site: FunctionSite,
    object: &ObjectExpression<'_>,
    touch: Symbol,
) -> bool {
    let Some(entries) = object_entries(object) else {
        return false;
    };
    if entries.len() != 5 {
        return false;
    }
    for (key, value) in entries {
        let valid = match key.as_str() {
            "a" => round_member(model, value, touch, "pageX"),
            "b" => round_member(model, value, touch, "pageY"),
            "c" => coordinate_before(model, site, touch, value, "clientX", "left"),
            "d" => coordinate_before(model, site, touch, value, "clientY", "top"),
            "e" => member(value, "force").is_some_and(|receiver| is_symbol(model, receiver, touch)),
            _ => false,
        };
        if !valid {
            return false;
        }
    }
    true
}

fn touch_pushes(
    model: &Model<'_, '_>,
    site: FunctionSite,
    scan: &BodyScan<'_>,
    event: Symbol,
    samples: Symbol,
    point: Span,
) -> bool {
    for &loop_ in &scan.fors {
        if loop_.span.end > point.start {
            continue;
        }
        let Some(statements) = block(&loop_.body) else {
            continue;
        };
        let Some(push) = scan.calls.iter().copied().find(|call| {
            call.span.start >= loop_.span.start
                && call.span.end <= loop_.span.end
                && symbol_of_push(model, call) == Some(samples)
        }) else {
            continue;
        };
        let mut touch = None;
        let mut iterated = None;
        for statement in statements {
            let Some(declarator) = one_variable(statement) else {
                continue;
            };
            let Some(init) = declarator.init.as_ref() else {
                continue;
            };
            let Some((object, _)) = indexed(init) else {
                continue;
            };
            let Some(local) = plain_declarator(model, declarator) else {
                continue;
            };
            touch = Some((local, init.span()));
            iterated = symbol(model, object);
            break;
        }
        let (Some((touch, touch_point)), Some(iterated)) = (touch, iterated) else {
            continue;
        };
        if !touch_list_selection(model, iterated, event, touch_point) {
            continue;
        }
        let arguments = call_arguments(push);
        if arguments.len() != 1 || arguments.len() != push.arguments.len() {
            continue;
        }
        let Expression::ObjectExpression(object) = arguments[0].without_parentheses() else {
            continue;
        };
        if touch_object(model, site, object, touch) {
            return true;
        }
    }
    false
}

fn symbol_of_push(model: &Model<'_, '_>, call: &CallExpression<'_>) -> Option<Symbol> {
    let Expression::StaticMemberExpression(callee) = call.callee.without_parentheses() else {
        return None;
    };
    if callee.optional || callee.property.name.as_str() != "push" {
        return None;
    }
    symbol(model, &callee.object)
}

fn touch_sample<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    body: &'a FunctionBody<'a>,
    scan: &BodyScan<'a>,
    event: Symbol,
) -> bool {
    let Some((meta, meta_span)) = meta_record(model, site, scan, event) else {
        return false;
    };
    let mut found = 0;
    for record in records(model, site, scan) {
        // Inline arguments and local records must reach the same captured put.
        // A bare object without that consumer is never sufficient evidence.
        if !matches!(record.origin, Origin::Value | Origin::Argument(_)) {
            continue;
        }
        let Some(fields) = record_fields(model, scan, record.object, record.symbol) else {
            continue;
        };
        if !keys_are(&fields, &["a", "b", "c", "d", "e"]) {
            continue;
        }
        if meta_span.end > record.object.span.start {
            continue;
        }
        let (Some(tape), Some(clock), Some(list), Some(ui), Some(trusted)) = (
            fields.get("a"),
            fields.get("b"),
            fields.get("c"),
            fields.get("d"),
            fields.get("e"),
        ) else {
            continue;
        };
        let Some(samples) = local_symbol(model, site, list) else {
            continue;
        };
        if !boolean_tape_before(model, site, tape, tape.span())
            || !zero_arg_callable(model, site, clock)
            || !empty_array_binding(model, samples)
            || !touch_pushes(model, site, scan, event, samples, list.span())
            || !meta_read(model, ui, meta, "instanceOfUIEvent")
            || !meta_read(model, trusted, meta, "markedAsTrusted")
            || !emitted(model, site, body, scan, &record)
        {
            continue;
        }
        found += 1;
    }
    found == 1
}

fn empty_array_binding(model: &Model<'_, '_>, binding: Symbol) -> bool {
    let Some(values) = write_values(model, binding) else {
        return false;
    };
    !values.is_empty() && values.iter().all(|(value, _)| empty_array(value))
}

// ===========================================================================
// Public evaluator
// ===========================================================================

fn owned_nested_function(model: &Model<'_, '_>, site: FunctionSite) -> bool {
    let body = site.body_span.unwrap_or(site.span);
    model.all_sites().iter().any(|other| {
        other.span != site.span && other.span.start >= body.start && other.span.end <= body.end
    })
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
    let Some(params) = plain_params(model, function) else {
        return Vec::new();
    };
    if owned_nested_function(model, site) {
        return Vec::new();
    }
    let scan = collect_body(body);

    if let [_, event] = params.as_slice() {
        if keyboard_sample(model, site, body, &scan, *event) {
            return vec![proposal(
                KEYBOARD_SAMPLE,
                EVALUATOR,
                "owned keyboard-event sample: the event formal's target id/name/offsetHeight/offsetWidth snapshot with the null fallback, the exact two-field instanceof-UIEvent / !!isTrusted record, a key/which/keyCode classification derived from the same formal, the four-modifier boolean tape, and one captured ring-buffer put (direct or recovered put.call) of the a..h sample reading that target and record",
                f,
            )];
        }
    }
    let [event] = params.as_slice() else {
        return Vec::new();
    };
    if mouse_sample(model, site, body, &scan, *event) {
        return vec![proposal(
            MOUSE_SAMPLE,
            EVALUATOR,
            "owned mouse-event sample: the event formal's target snapshot, the exact two-field UIEvent/isTrusted record, the Element-guarded getBoundingClientRect bounds, the pageX/pageY null-round fallback, the clientX/clientY rounded offset pair and the button, all read through the a..m record and handed to one captured ring-buffer put (direct or recovered put.call) or returned",
            f,
        )];
    }
    if pointer_sample(model, site, body, &scan, *event) {
        return vec![proposal(
            POINTER_SAMPLE,
            EVALUATOR,
            "owned pointer-position sample: the exact two-field UIEvent/isTrusted record, the event formal's pageX/pageY position and one captured ring-buffer put (direct or recovered put.call) or return of the five-field sample; no target snapshot is invented",
            f,
        )];
    }
    if touch_sample(model, site, body, &scan, *event) {
        return vec![proposal(
            TOUCH_SAMPLE,
            EVALUATOR,
            "owned touch-event sample: the event's changedTouches/touches list selected from the event formal, one per-touch pageX/pageY/clientX/clientY/force scan pushed into the owned sample list, and one captured ring-buffer put of the a..e payload carrying that list and the exact two-field UIEvent/isTrusted record",
            f,
        )];
    }
    Vec::new()
}

fn plain_params(model: &Model<'_, '_>, function: &Function<'_>) -> Option<Vec<Symbol>> {
    if function.params.rest.is_some() {
        return None;
    }
    let mut formals = Vec::new();
    for parameter in &function.params.items {
        if parameter.initializer.is_some() {
            return None;
        }
        let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
            return None;
        };
        let symbol = identifier.symbol_id.get()?;
        if model.binding_writes(symbol) != 0
            || !model.scoping.symbol_redeclarations(symbol).is_empty()
        {
            return None;
        }
        formals.push(symbol);
    }
    Some(formals)
}
