//! Bounded owned-body proof for the storage-removal seeded frame emitter.
//!
//! This is *not* the reviewed WebRTC-candidate seeded emitter and not the
//! cookie, URL-query or worker stages.  The owning binding is registered by its
//! enclosing scope as
//!
//! ```text
//! global.addEventListener("storage", <this function>);
//! ```
//!
//! (`uhc-uhc-final01`, `fn@155103..156463`: `$KEY_23 = $THREAD_2_111500` and
//! `$KEY_31.global.addEventListener("storage", $KEY_23)`), so the one plain
//! formal is a `StorageEvent`-like record whose `key`, `newValue` and
//! `oldValue` are read.  The purpose is therefore confirmed by the actual
//! parent registration, not inferred from the member names alone.
//!
//! The owned callback has exactly one statement, a `try`/`catch`, and no
//! `finally`:
//!
//! ```text
//! function storageCallback(event) {
//!   try {
//!     if (event.key === null ||
//!         event.key === <capturedKey> && event.newValue === null) {
//!       <capturedRemovalFlag> = true;                 // the removal signal
//!       <capturedOldValue> = event.oldValue ?? null;
//!       let nonce = Math.random() * 2^32 | 0;
//!       let header = [nonce >>> 24, nonce >> 16 & 255,
//!                     nonce >> 8 & 255, nonce & 255];
//!       nonce = <seedFactory>(nonce ^ <number>);      // reseed -> stream
//!       let writer = <writerFactory>(nonce);
//!       let flagAlias = <capturedRemovalFlag>;
//!       let oldAlias = <capturedOldValue>;
//!       let cursor;
//!       if (oldAlias == null) {
//!         header[4] = nonce() ^ <tag>;                // literal tag byte
//!         cursor = 4;
//!       } else {
//!         writer = [writer, oldAlias, header];
//!         cursor = writer[0](writer[1], writer[2], 3);// holder-mediated write
//!       }
//!       header[cursor + 1] =
//!         (flagAlias | 0 | <capturedErrorFlag> << 1 |
//!          <capturedSecondFlag> << 2) ^ nonce();
//!       <capturedObserver>.next([[], header]);
//!     }
//!   } catch (e) {
//!     <capturedErrorFlag> = true;
//!     let nonce = Math.random() * 2^32;               // same frame, re-emitted
//!     ... exact same ordered dataflow through the same captured slots,
//!         the same header layout, the same seed/writer factories and the
//!         same stream/cursor use ...
//!     <capturedObserver>.next([[], header]);
//!   }
//! }
//! ```
//!
//! The body is a streaming update: it never calls `complete()`.  A body that
//! completes is not this family, and the exact statement layout below refuses
//! it.  The `catch` block is one frame re-emission, not a second role: both
//! blocks are proved and must name the exact same captured state, key,
//! observer, flags, old value, nonce derivation, factories, tag and cursor
//! semantics.
//!
//! The seed and writer factories are not matched by arity or by a name.  Each
//! call is resolved through the shared source-backed
//! [`super::super::resolve::ordered_callable`] proof, and the callee must be a
//! scoped input declared outside this callback.  Each resolved factory must
//! immediately own the child callable it returns, and that child must consume
//! the factory's own plain formal through at most one factory-local reaching
//! alias.  For the seeded stream the seed value must reach a returned `& 255`
//! byte lane of the child and the child must never invoke it; for the writer the
//! child must invoke the seed in a stored or returned byte value.  A dead local
//! initializer, a never-called nested function, a bare or write-only mention, an
//! empty control test, a discarded call, a dead-overwritten alias, a donated
//! global child and a swapped factory role all decline.  The invoked child is the
//! same value the body calls (`stream()` and the holder's `writer[0](...)`).
//!
//! The same family also has a second bounded variant in which the writer
//! binding doubles as the cursor and the writer is re-invoked directly, with the
//! removal/error flag lanes shifted by one bit:
//!
//! ```text
//!   ... same guard, removal flag, old value, nonce/header/reseed ...
//!   let writer = <writerFactory>(nonce);
//!   let payload = <capturedOldValue>;
//!   if (payload == null) {
//!     header[4] = nonce() ^ <tag>;
//!     writer = 4;                            // writer binding is the cursor
//!   } else {
//!     writer = writer(payload, header, 3);   // direct re-invocation, no holder
//!   }
//!   header[writer + 1] =
//!     (<auxFlag> | 0 | <capturedRemovalFlag> << 1 |
//!      <capturedErrorFlag> << 2) ^ nonce();  // removal/error lanes shifted
//!   <capturedObserver>.next([[], header]);
//! ```
//!
//! This variant (the real slice `fn@191473..192638`) has no separate flag
//! alias and no bare `let cursor;`; the writer binding is assigned exactly once
//! in each branch (`4` or the direct call result) and is then read as the
//! cursor.  It keeps every other identity — the guard, the old-value alias, the
//! header, the reseed literal, the tag, the observer and both factories — and
//! the same `catch` re-emit, and it is proved by the same state-directed factory
//! checks and the same two-frame equality, only with the three shifted lanes
//! compared in order and the captured removal flag at bit 1 and the captured
//! error flag at bit 2.  A body whose tail does not fit either exact statement
//! layout is not this family.
//!
//! Two further bounded variants of the same family are accepted, each as its own
//! exact statement grammar tried only after the earlier grammars decline.  The
//! fixed-slot variant (the real `fn@146063..147212` callback) appends the
//! shifted lanes at the literal header slot `4` *before* the `old == null` test,
//! writes the literal tag byte at slot `5` in the null branch and invokes the
//! writer with the literal cursor `4` in the other branch; the writer binding is
//! never reassigned and there is no cursor binding.  The late-flags holder
//! variant (the real `fn@311194..312517` callback) keeps the reviewed
//! holder branch and a separate `let cursor;`, but has no captured-flag alias:
//! the holder `flag | 0 | error << 1 | second << 2` lanes are appended at
//! `header[cursor + 1]` after the branch, reading the captured removal flag
//! directly at the low lane.  Both variants keep the same guard,
//! removal/old prefix, nonce/header, reseed, source-backed factories, observer
//! and no-`complete()` streaming behaviour, and both require the catch re-emit to
//! match the same variant.
//!
//! Every compared value is a scoped symbol.  This rule reads no generated
//! spelling, vendor name, span or hash, does not rename anything, and runs only
//! in the final post-host fallback stage after every earlier source-backed
//! evaluator, including the URL query merger, has declined.

use super::*;
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;

type Symbol = SymbolId;

const NAME: &str = "emitStorageRemovalSignal";
const EVALUATOR: &str = "thread_posthost_storage_removal_emitter";

// ===========================================================================
// Scoped expression and statement readers
// ===========================================================================

pub(super) fn symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(expression.without_parentheses(), model.scoping)
}

pub(super) fn member<'a>(expression: &'a Expression<'a>, name: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(member) = expression.without_parentheses() else {
        return None;
    };
    (!member.optional && member.property.name == name).then_some(&member.object)
}

pub(super) fn computed<'a>(
    expression: &'a Expression<'a>,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::ComputedMemberExpression(member) = expression.without_parentheses() else {
        return None;
    };
    (!member.optional).then_some((&member.object, &member.expression))
}

pub(super) fn member_target<'e, 'a>(
    target: &'e AssignmentTarget<'a>,
) -> Option<(&'e Expression<'a>, &'e Expression<'a>)> {
    let AssignmentTarget::ComputedMemberExpression(member) = target else {
        return None;
    };
    (!member.optional).then_some((&member.object, &member.expression))
}

pub(super) fn call<'a>(expression: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(call) = expression.without_parentheses() else {
        return None;
    };
    (!call.optional).then_some(call)
}

pub(super) fn number(expression: &Expression<'_>, expected: f64) -> bool {
    matches!(expression.without_parentheses(), Expression::NumericLiteral(value) if value.value == expected)
}

pub(super) fn binary<'a>(
    expression: &'a Expression<'a>,
    operator: &str,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return None;
    };
    (binary.operator.as_str() == operator).then_some((&binary.left, &binary.right))
}

fn logical<'a>(
    expression: &'a Expression<'a>,
    operator: &str,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::LogicalExpression(logical) = expression.without_parentheses() else {
        return None;
    };
    (logical.operator.as_str() == operator).then_some((&logical.left, &logical.right))
}

pub(super) fn is_null(expression: &Expression<'_>) -> bool {
    matches!(expression.without_parentheses(), Expression::NullLiteral(_))
}

fn boolean_true(expression: &Expression<'_>) -> bool {
    matches!(expression.without_parentheses(), Expression::BooleanLiteral(value) if value.value)
}

pub(super) fn expression_statement<'a>(statement: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ExpressionStatement(statement) = statement else {
        return None;
    };
    Some(&statement.expression)
}

pub(super) fn assignment<'a>(
    expression: &'a Expression<'a>,
) -> Option<&'a AssignmentExpression<'a>> {
    let Expression::AssignmentExpression(assignment) = expression.without_parentheses() else {
        return None;
    };
    (assignment.operator.as_str() == "=").then_some(assignment)
}

pub(super) fn assigned_symbol(
    model: &Model<'_, '_>,
    assignment: &AssignmentExpression<'_>,
) -> Option<Symbol> {
    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
        return None;
    };
    crate::analysis::identifier_symbol(identifier, model.scoping)
}

fn one_variable<'a>(statement: &'a Statement<'a>) -> Option<&'a VariableDeclarator<'a>> {
    let Statement::VariableDeclaration(declaration) = statement else {
        return None;
    };
    let [variable] = declaration.declarations.as_slice() else {
        return None;
    };
    Some(variable)
}

pub(super) fn block<'a>(statement: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(block) = statement else {
        return None;
    };
    Some(block.body.as_slice())
}

/// A plain `var`/`let`/`const` declarator bound to exactly this symbol.
pub(super) fn plain_local(model: &Model<'_, '_>, variable: &VariableDeclarator<'_>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(identifier) = &variable.id else {
        return None;
    };
    let local = identifier.symbol_id.get()?;
    if !model.scoping.symbol_redeclarations(local).is_empty() {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(local);
    let AstKind::VariableDeclarator(declared) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&declared.id, BindingPattern::BindingIdentifier(found)
        if found.symbol_id.get() == Some(local))
    {
        return None;
    }
    if !matches!(
        model.nodes.parent_kind(declaration),
        AstKind::VariableDeclaration(declaration)
            if matches!(
                declaration.kind,
                VariableDeclarationKind::Var
                    | VariableDeclarationKind::Let
                    | VariableDeclarationKind::Const
            )
    ) {
        return None;
    }
    Some(local)
}

/// A captured slot: a plain local declared outside the owned body and never
/// redeclared.  The callback reads it by scope identity, never by spelling.
pub(super) fn captured_local(
    model: &Model<'_, '_>,
    site: FunctionSite,
    candidate: Symbol,
) -> Option<Symbol> {
    if site
        .span
        .contains_inclusive(model.scoping.symbol_span(candidate))
        || !model.scoping.symbol_redeclarations(candidate).is_empty()
    {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(candidate);
    let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&variable.id, BindingPattern::BindingIdentifier(found)
        if found.symbol_id.get() == Some(candidate))
    {
        return None;
    }
    if !matches!(
        model.nodes.parent_kind(declaration),
        AstKind::VariableDeclaration(declaration)
            if matches!(
                declaration.kind,
                VariableDeclarationKind::Var
                    | VariableDeclarationKind::Let
                    | VariableDeclarationKind::Const
            )
    ) {
        return None;
    }
    Some(candidate)
}

fn method_receiver(
    model: &Model<'_, '_>,
    call: &CallExpression<'_>,
    method: &str,
) -> Option<Symbol> {
    let Expression::StaticMemberExpression(member) = call.callee.without_parentheses() else {
        return None;
    };
    (!member.optional && member.property.name == method).then(|| symbol(model, &member.object))?
}

pub(super) fn arity<'a>(
    call: &'a CallExpression<'a>,
    count: usize,
) -> Option<Vec<&'a Expression<'a>>> {
    let arguments = call_arguments(call);
    (arguments.len() == count && call.arguments.len() == count).then_some(arguments)
}

fn empty_array(expression: &Expression<'_>) -> bool {
    matches!(expression.without_parentheses(), Expression::ArrayExpression(array) if array.elements.is_empty())
}

/// An unshadowed global namespace path: a bound root is never a host global.
fn is_global(model: &Model<'_, '_>, expression: &Expression<'_>, path: &[&str]) -> bool {
    expression_path(model, expression)
        .iter()
        .map(String::as_str)
        .eq(path.iter().copied())
}

// ===========================================================================
// Source-backed factory roles
// ===========================================================================

/// Which seeded consumer a factory serves.  The seeded stream consumes the seed
/// word as data and returns it through its byte lanes; the writer invokes that
/// stream to draw keystream bytes.  The two roles are told apart by how the
/// returned child consumes the factory formal, never by a name, arity or hash.
#[derive(Clone, Copy)]
enum FactoryUse {
    Stream,
    Writer,
}

/// A call whose callee is a scoped input of the callback and resolves, through
/// the shared source-backed callable proof, to one independent function.  The
/// callee binding itself must be declared outside the owned callback body, so a
/// helper introduced inside the callback is never mistaken for a captured
/// factory input.  This is never an arity-only or by-name match: a bound alias,
/// a stable function declaration and a single origin are all required.
fn factory_call<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &'a Expression<'a>,
) -> Option<(&'a CallExpression<'a>, Symbol, FunctionSite)> {
    let callee = call(expression)?;
    let callee_symbol = symbol(model, &callee.callee)?;
    if site
        .span
        .contains_inclusive(model.scoping.symbol_span(callee_symbol))
        || !model
            .scoping
            .symbol_redeclarations(callee_symbol)
            .is_empty()
    {
        return None;
    }
    let factory = crate::naming::resolve::ordered_callable(model, &callee.callee, 0)?;
    Some((callee, callee_symbol, factory))
}

/// True when `node_id` sits directly in the own body of `site`: the nearest
/// owning function or arrow is exactly `site`.  A node inside a nested function
/// or arrow (including a never-called sibling helper) is therefore excluded.
fn own_body_node(model: &Model<'_, '_>, node_id: oxc_semantic::NodeId, site: FunctionSite) -> bool {
    model
        .owning_site(node_id)
        .is_some_and(|owner| owner.span == site.span)
}

/// The factory's plain formal plus each factory-local binding whose latest write
/// in the factory's own body is that formal.  An alias written inside the
/// returned child or a never-called sibling nested function is not part of the
/// factory's own setup, and a later overwrite makes the earlier formal write
/// dead, so neither becomes a carrier.
fn factory_carriers(
    model: &Model<'_, '_>,
    factory: FunctionSite,
    formal: Symbol,
) -> BTreeSet<Symbol> {
    let factory_body = factory.body_span.unwrap_or(factory.span);
    let mut candidates: BTreeSet<Symbol> = BTreeSet::new();
    let mut latest_write: BTreeMap<Symbol, (u32, bool)> = BTreeMap::new();
    for node in model.nodes.iter() {
        if !own_body_node(model, node.id(), factory) {
            continue;
        }
        let span = node.kind().span();
        if span.start < factory_body.start || span.end > factory_body.end {
            continue;
        }
        match node.kind() {
            AstKind::VariableDeclarator(variable) => {
                let Some(init) = &variable.init else {
                    continue;
                };
                let BindingPattern::BindingIdentifier(identifier) = &variable.id else {
                    continue;
                };
                let Some(target) = identifier.symbol_id.get() else {
                    continue;
                };
                let from_formal = symbol(model, init) == Some(formal);
                record_factory_write(&mut latest_write, target, span.start, from_formal);
                if from_formal {
                    candidates.insert(target);
                }
            }
            AstKind::AssignmentExpression(assignment) if assignment.operator.as_str() == "=" => {
                let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left
                else {
                    continue;
                };
                let Some(target) = crate::analysis::identifier_symbol(identifier, model.scoping)
                else {
                    continue;
                };
                let from_formal = symbol(model, &assignment.right) == Some(formal);
                record_factory_write(&mut latest_write, target, span.start, from_formal);
                if from_formal {
                    candidates.insert(target);
                }
            }
            _ => {}
        }
    }

    let mut carriers = BTreeSet::new();
    carriers.insert(formal);
    for candidate in candidates {
        if candidate == formal
            || !factory_body.contains_inclusive(model.scoping.symbol_span(candidate))
        {
            continue;
        }
        if latest_write
            .get(&candidate)
            .is_some_and(|(_, from_formal)| *from_formal)
        {
            carriers.insert(candidate);
        }
    }
    carriers
}

/// Keep the latest factory-own-body write to `target`, so an alias that is later
/// overwritten with a non-formal value is not a live carrier.
fn record_factory_write(
    writes: &mut BTreeMap<Symbol, (u32, bool)>,
    target: Symbol,
    start: u32,
    from_formal: bool,
) {
    let entry = writes.entry(target).or_insert((start, from_formal));
    if start >= entry.0 {
        *entry = (start, from_formal);
    }
}

/// The state symbol read by a returned byte lane: `state & 255`,
/// `state >> N & 255` or `state >>> N & 255`.
fn byte_lane_state(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<Symbol> {
    let (left, mask) = binary(expression, "&")?;
    if !number(mask, 255.0) {
        return None;
    }
    let base = match binary(left, ">>").or_else(|| binary(left, ">>>")) {
        Some((base, shift))
            if matches!(shift.without_parentheses(), Expression::NumericLiteral(_)) =>
        {
            base
        }
        Some(_) => return None,
        None => left,
    };
    symbol(model, base)
}

/// Ordered reaching state for the bounded stream body. An assignment replaces
/// its target's dependency; it never adds a historical fact. Unsupported
/// effects/control flow decline rather than borrowing a later or nested write.
fn stream_value_reads(
    model: &Model<'_, '_>,
    value: &Expression<'_>,
    live: &BTreeSet<Symbol>,
    depth: u32,
) -> Option<bool> {
    if depth == 0 {
        return None;
    }
    let next = depth - 1;
    match value.without_parentheses() {
        Expression::Identifier(_) => Some(symbol(model, value).is_some_and(|s| live.contains(&s))),
        Expression::NumericLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::NullLiteral(_) => Some(false),
        Expression::BinaryExpression(binary) => Some(
            stream_value_reads(model, &binary.left, live, next)?
                | stream_value_reads(model, &binary.right, live, next)?,
        ),
        Expression::UnaryExpression(unary) if unary.operator.as_str() != "delete" => {
            stream_value_reads(model, &unary.argument, live, next)
        }
        // A conditional dependency must survive either result. Tests and
        // discarded sequence operands do not supply returned data.
        Expression::ConditionalExpression(branch) => {
            stream_value_reads(model, &branch.test, live, next)?;
            Some(
                stream_value_reads(model, &branch.consequent, live, next)?
                    & stream_value_reads(model, &branch.alternate, live, next)?,
            )
        }
        _ => None,
    }
}

fn replace_stream_state(live: &mut BTreeSet<Symbol>, target: Symbol, dependent: bool) {
    live.remove(&target);
    if dependent {
        live.insert(target);
    }
}

/// Constant truthiness for pure literal tests. The caller validates the test
/// with `stream_value_reads` first, including operands discarded by `void`.
/// Unknown stream-state tests retain the existing two-arm analysis.
fn stream_literal_truthiness(test: &Expression<'_>, depth: u32) -> Option<bool> {
    if depth == 0 {
        return None;
    }
    match test.without_parentheses() {
        Expression::BooleanLiteral(value) => Some(value.value),
        Expression::NumericLiteral(value) => Some(value.value != 0.0 && !value.value.is_nan()),
        Expression::StringLiteral(value) => Some(!value.value.as_str().is_empty()),
        Expression::NullLiteral(_) => Some(false),
        Expression::UnaryExpression(unary) => match unary.operator.as_str() {
            "!" => stream_literal_truthiness(&unary.argument, depth - 1).map(|value| !value),
            "void" => Some(false),
            "+" | "-" => match unary.argument.without_parentheses() {
                Expression::NumericLiteral(value) => {
                    Some(value.value != 0.0 && !value.value.is_nan())
                }
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

/// The real stream rotates one computed word through four byte lanes across
/// invocations. Recognize only its complete terminal six-statement suffix;
/// validate the update against the incoming state before accepting any lane.
/// This does not make a return elsewhere evidence for these cached lanes.
fn stream_rotating_bytes(
    model: &Model<'_, '_>,
    statements: &[Statement<'_>],
    live: &BTreeSet<Symbol>,
    depth: u32,
) -> Option<()> {
    let [declaration, advance, first, second, third, fourth] = statements else {
        return None;
    };
    let variable = one_variable(declaration)?;
    let BindingPattern::BindingIdentifier(alias) = &variable.id else {
        return None;
    };
    let alias = alias.symbol_id.get()?;
    let counter = symbol(model, variable.init.as_ref()?)?;
    if alias == counter {
        return None;
    }
    let write = assignment(expression_statement(advance)?)?;
    if assigned_symbol(model, write)? != counter {
        return None;
    }
    let rotation = |value: &Expression<'_>| {
        let Some((sum, modulus)) = binary(value, "%") else {
            return false;
        };
        let Some((input, step)) = binary(sum, "+") else {
            return false;
        };
        symbol(model, input) == Some(alias) && number(step, 1.0) && number(modulus, 4.0)
    };
    if !rotation(&write.right) {
        return None;
    }
    let mut updated = None;
    let mut word = None;
    for (index, statement) in [first, second, third, fourth].into_iter().enumerate() {
        let Statement::IfStatement(branch) = statement else {
            return None;
        };
        let (test, lane) = binary(&branch.test, "===")?;
        if !rotation(test) || !number(lane, index as f64) {
            return None;
        }
        if index == 3 {
            let [Statement::ReturnStatement(exit)] = block(branch.alternate.as_ref()?)? else {
                return None;
            };
            let Expression::UnaryExpression(exit) = exit.argument.as_ref()?.without_parentheses()
            else {
                return None;
            };
            if exit.operator.as_str() != "void" || !number(&exit.argument, 0.0) {
                return None;
            }
        } else if branch.alternate.is_some() {
            return None;
        }
        let (returned, prefix) = block(&branch.consequent)?.split_last()?;
        if index == 0 {
            let (continuation, returned_seed) =
                stream_statements(model, prefix, live.clone(), depth)?;
            if returned_seed {
                return None;
            }
            updated = Some(continuation?);
        } else if !prefix.is_empty() {
            return None;
        }
        let Statement::ReturnStatement(returned) = returned else {
            return None;
        };
        let value = returned.argument.as_ref()?;
        let state = byte_lane_state(model, value)?;
        if !updated.as_ref()?.contains(&state) {
            return None;
        }
        let (lane, _) = binary(value, "&")?;
        if index == 0 {
            if symbol(model, lane) != Some(state) {
                return None;
            }
            word = Some(state);
        } else {
            let (base, shift) = binary(lane, if index == 3 { ">>>" } else { ">>" })?;
            if symbol(model, base) != word || !number(shift, (index * 8) as f64) {
                return None;
            }
        }
    }
    Some(())
}

/// `None` as the continuation means the path returned. Every admitted return
/// must itself be a seeded byte lane, apart from the rotating suffix's exact
/// void sentinel; unknown branches cannot donate evidence to another return.
/// Branch continuations
/// intersect: a write in only one arm cannot seed a later read in both arms.
fn stream_statements(
    model: &Model<'_, '_>,
    statements: &[Statement<'_>],
    mut live: BTreeSet<Symbol>,
    depth: u32,
) -> Option<(Option<BTreeSet<Symbol>>, bool)> {
    if depth == 0 {
        return None;
    }
    let mut returned_seed = false;
    for (index, statement) in statements.iter().enumerate() {
        if stream_rotating_bytes(model, &statements[index..], &live, depth - 1).is_some() {
            return Some((None, true));
        }
        match statement {
            Statement::FunctionDeclaration(_) | Statement::EmptyStatement(_) => {}
            Statement::BlockStatement(block) => {
                let (rest, seeded) = stream_statements(model, &block.body, live, depth - 1)?;
                returned_seed |= seeded;
                let Some(rest) = rest else {
                    return Some((None, returned_seed));
                };
                live = rest;
            }
            Statement::VariableDeclaration(declaration) => {
                for variable in &declaration.declarations {
                    let BindingPattern::BindingIdentifier(identifier) = &variable.id else {
                        return None;
                    };
                    let target = identifier.symbol_id.get()?;
                    let dependent = match &variable.init {
                        Some(init) => stream_value_reads(model, init, &live, 32)?,
                        None => false,
                    };
                    replace_stream_state(&mut live, target, dependent);
                }
            }
            Statement::ExpressionStatement(statement) => {
                let assignment = assignment(&statement.expression)?;
                let target = assigned_symbol(model, assignment)?;
                let dependent = stream_value_reads(model, &assignment.right, &live, 32)?;
                replace_stream_state(&mut live, target, dependent);
            }
            Statement::ReturnStatement(returned) => {
                let value = returned.argument.as_ref()?;
                stream_value_reads(model, value, &live, 32)?;
                let state = byte_lane_state(model, value)?;
                if !live.contains(&state) {
                    return None;
                }
                return Some((None, true));
            }
            Statement::IfStatement(branch) => {
                stream_value_reads(model, &branch.test, &live, 32)?;
                let constant = stream_literal_truthiness(&branch.test, 32);
                let yes = if constant == Some(false) {
                    (Some(live.clone()), false)
                } else {
                    stream_statements(
                        model,
                        std::slice::from_ref(&branch.consequent),
                        live.clone(),
                        depth - 1,
                    )?
                };
                let no = if constant == Some(true) {
                    (Some(live.clone()), false)
                } else if let Some(alternate) = &branch.alternate {
                    stream_statements(
                        model,
                        std::slice::from_ref(alternate),
                        live.clone(),
                        depth - 1,
                    )?
                } else {
                    (Some(live.clone()), false)
                };
                let (continuation, seeded) = match constant {
                    Some(true) => yes,
                    Some(false) => no,
                    None => {
                        let joined = match (yes.0, no.0) {
                            (Some(a), Some(b)) => Some(a.intersection(&b).copied().collect()),
                            (Some(a), None) | (None, Some(a)) => Some(a),
                            (None, None) => None,
                        };
                        (joined, yes.1 || no.1)
                    }
                };
                returned_seed |= seeded;
                let Some(rest) = continuation else {
                    return Some((None, returned_seed));
                };
                live = rest;
            }
            _ => return None,
        }
    }
    Some((Some(live), returned_seed))
}

fn stream_seed_reaches_returned_bytes(
    model: &Model<'_, '_>,
    child: FunctionSite,
    carriers: &BTreeSet<Symbol>,
) -> bool {
    let AstKind::Function(function) = model.nodes.kind(child.node_id) else {
        return false;
    };
    let Some(body) = &function.body else {
        return false;
    };
    stream_statements(model, &body.statements, carriers.clone(), 64)
        .is_some_and(|(continuation, seeded)| continuation.is_none() && seeded)
}

/// True when the child reaches a carrier through an invocation position: a
/// direct callee, a member callee object (`carrier.call(...)` /
/// `carrier[0](...)`) or the effective last sequence operand of a callee
/// (`(0, carrier)()`).  A seed-mode child that really invokes the formal through
/// a non-identifier form therefore never passes the stream/writer split.
fn invocation_callee_mentions(
    model: &Model<'_, '_>,
    callee: &Expression<'_>,
    carriers: &BTreeSet<Symbol>,
) -> bool {
    match callee.without_parentheses() {
        Expression::Identifier(identifier) => {
            crate::analysis::identifier_symbol(identifier, model.scoping)
                .is_some_and(|symbol| carriers.contains(&symbol))
        }
        Expression::StaticMemberExpression(member) => {
            symbol(model, &member.object).is_some_and(|symbol| carriers.contains(&symbol))
        }
        Expression::ComputedMemberExpression(member) => {
            symbol(model, &member.object).is_some_and(|symbol| carriers.contains(&symbol))
        }
        Expression::SequenceExpression(sequence) => sequence
            .expressions
            .last()
            .is_some_and(|last| invocation_callee_mentions(model, last, carriers)),
        _ => false,
    }
}

/// True when the returned child invokes a carrier anywhere in its own body.
fn child_invokes_carrier(
    model: &Model<'_, '_>,
    child: FunctionSite,
    carriers: &BTreeSet<Symbol>,
) -> bool {
    for node in model.nodes.iter() {
        if !own_body_node(model, node.id(), child) {
            continue;
        }
        match node.kind() {
            AstKind::CallExpression(call) => {
                if invocation_callee_mentions(model, &call.callee, carriers) {
                    return true;
                }
            }
            AstKind::NewExpression(new) => {
                if invocation_callee_mentions(model, &new.callee, carriers) {
                    return true;
                }
            }
            AstKind::TaggedTemplateExpression(tagged) => {
                if invocation_callee_mentions(model, &tagged.tag, carriers) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// True when a direct carrier invocation's value reaches a plain store or a
/// returned value through value-preserving operators.  Only arithmetic/bitwise
/// operands count: a discarded call, a control-flow test and a call argument do
/// not.  This is the real writer's `$KEY_2() ^ <byte>` stored into the output.
fn call_result_reaches_store_or_return(
    model: &Model<'_, '_>,
    call_id: oxc_semantic::NodeId,
) -> bool {
    let mut current = call_id;
    for _ in 0..32 {
        let parent = model.nodes.parent_id(current);
        if parent == current {
            return false;
        }
        match model.nodes.kind(parent) {
            AstKind::ParenthesizedExpression(_)
            | AstKind::BinaryExpression(_)
            | AstKind::UnaryExpression(_) => {
                current = parent;
            }
            AstKind::AssignmentExpression(assignment) => {
                return assignment.operator.as_str() == "="
                    && assignment.right.span() == model.nodes.get_node(current).span();
            }
            AstKind::ReturnStatement(returned) => {
                return returned.argument.as_ref().is_some_and(|argument| {
                    argument.span() == model.nodes.get_node(current).span()
                });
            }
            _ => return false,
        }
    }
    false
}

/// True when the returned child invokes a carrier directly and that byte value
/// is stored or returned, e.g. the real writer's `$KEY_2() ^ <byte>`.
fn writer_seed_reaches_byte_store(
    model: &Model<'_, '_>,
    child: FunctionSite,
    carriers: &BTreeSet<Symbol>,
) -> bool {
    for node in model.nodes.iter() {
        if !own_body_node(model, node.id(), child) {
            continue;
        }
        let AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        if !symbol(model, &call.callee).is_some_and(|callee| carriers.contains(&callee)) {
            continue;
        }
        if call_result_reaches_store_or_return(model, node.id()) {
            return true;
        }
    }
    false
}

/// Proves that the factory's own plain formal is actually consumed by the
/// returned child, through at most one factory-local reaching alias.  For the
/// seeded stream the carrier value must flow into a returned `& 255` byte lane
/// and the child must never invoke it; for the writer the child must invoke the
/// carrier in a stored or returned byte value.  A dead local initializer, a
/// never-called nested function, a bare or write-only mention, an empty control
/// test, a discarded call and a dead-overwritten alias all decline.
fn factory_formal_feeds_child(
    model: &Model<'_, '_>,
    factory: FunctionSite,
    formal: Symbol,
    child: FunctionSite,
    usage: FactoryUse,
) -> bool {
    let AstKind::Function(function) = model.nodes.kind(factory.node_id) else {
        return false;
    };
    let Some(body) = &function.body else {
        return false;
    };
    // The alias setup is only proved for unconditional direct statements.
    if body.statements.iter().any(|statement| {
        !matches!(
            statement,
            Statement::VariableDeclaration(_)
                | Statement::FunctionDeclaration(_)
                | Statement::ExpressionStatement(_)
                | Statement::ReturnStatement(_)
                | Statement::EmptyStatement(_)
        )
    }) {
        return false;
    }
    let carriers = factory_carriers(model, factory, formal);

    match usage {
        FactoryUse::Stream => {
            stream_seed_reaches_returned_bytes(model, child, &carriers)
                && !child_invokes_carrier(model, child, &carriers)
        }
        FactoryUse::Writer => {
            // Writer control flow includes loops. Until an ordered loop proof
            // is available, any own write to a callable carrier invalidates it.
            let clobbered = model.nodes.iter().any(|node| {
                own_body_node(model, node.id(), child)
                    && match node.kind() {
                        AstKind::AssignmentExpression(write) => assigned_symbol(model, write)
                            .is_some_and(|target| carriers.contains(&target)),
                        AstKind::UpdateExpression(update) => match &update.argument {
                            SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                                crate::analysis::identifier_symbol(identifier, model.scoping)
                                    .is_some_and(|target| carriers.contains(&target))
                            }
                            _ => false,
                        },
                        _ => false,
                    }
            });
            !clobbered && writer_seed_reaches_byte_store(model, child, &carriers)
        }
    }
}

/// The resolved factory must immediately own the nested child callable it
/// returns, and that child must run on the factory's live formal.  A plain
/// helper of the right shape, a nested no-op, a dead formal mention, a donated
/// global child and a function whose result is not a callable all decline here.
fn factory_returns_live_child(
    model: &Model<'_, '_>,
    factory: FunctionSite,
    usage: FactoryUse,
) -> bool {
    let AstKind::Function(function) = model.nodes.kind(factory.node_id) else {
        return false;
    };
    let Some(body) = &function.body else {
        return false;
    };
    let [parameter] = function.params.items.as_slice() else {
        return false;
    };
    if function.params.rest.is_some() || parameter.initializer.is_some() {
        return false;
    }
    let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
        return false;
    };
    let Some(formal) = identifier.symbol_id.get() else {
        return false;
    };
    if model.binding_writes(formal) != 0 || !model.scoping.symbol_redeclarations(formal).is_empty()
    {
        return false;
    }
    let Some(slot) = model.formal_symbol_slot(formal) else {
        return false;
    };
    if slot.owner != factory.node_id {
        return false;
    }
    let Some(Statement::ReturnStatement(returned)) = body.statements.last() else {
        return false;
    };
    let Some(argument) = returned.argument.as_ref() else {
        return false;
    };
    // A sole final exit is the supported factory shape. Child returns and
    // returns in the enclosing callback are deliberately outside this scope.
    for node in model.nodes.iter() {
        if !own_body_node(model, node.id(), factory) {
            continue;
        }
        match node.kind() {
            AstKind::ReturnStatement(other) if other.span != returned.span => return false,
            AstKind::ThrowStatement(_) => return false,
            _ => {}
        }
    }
    let Some(child) = crate::naming::resolve::ordered_callable(model, argument, 0) else {
        return false;
    };
    if child.span == factory.span
        || !factory.span.contains_inclusive(child.span)
        || !model
            .enclosing_function(child.node_id)
            .is_some_and(|owner| owner.span == factory.span)
    {
        return false;
    }
    factory_formal_feeds_child(model, factory, formal, child, usage)
}

// ===========================================================================
// Four-byte big-endian nonce/header and seeded stream
// ===========================================================================

/// `seed` or `(seed | 0)`.
fn seed_base(model: &Model<'_, '_>, expression: &Expression<'_>, seed: Symbol) -> bool {
    if symbol(model, expression) == Some(seed) {
        return true;
    }
    matches!(binary(expression, "|"),
        Some((left, right)) if number(right, 0.0) && symbol(model, left) == Some(seed))
}

/// `Math.random() * 4294967296`, optionally `| 0`.
fn random_seed(model: &Model<'_, '_>, expression: &Expression<'_>) -> bool {
    let base = match binary(expression, "|") {
        Some((left, right)) if number(right, 0.0) => left,
        _ => expression.without_parentheses(),
    };
    let Some((random, scale)) = binary(base, "*") else {
        return false;
    };
    number(scale, 4_294_967_296.0)
        && call(random).is_some_and(|call| {
            call.arguments.is_empty() && is_global(model, &call.callee, &["Math", "random"])
        })
}

fn header_slot(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    seed: Symbol,
    slot: usize,
) -> bool {
    match slot {
        0 => matches!(binary(expression, ">>>"),
            Some((left, right)) if seed_base(model, left, seed) && number(right, 24.0)),
        1 | 2 => {
            let shift = if slot == 1 { 16.0 } else { 8.0 };
            matches!(binary(expression, "&"),
                Some((left, right))
                    if number(right, 255.0)
                        && matches!(binary(left, ">>"),
                            Some((shifted, by))
                                if seed_base(model, shifted, seed) && number(by, shift)))
        }
        3 => matches!(binary(expression, "&"),
            Some((left, right)) if number(right, 255.0) && seed_base(model, left, seed)),
        _ => false,
    }
}

/// The exact four-byte most-significant-byte-first header of the nonce.
fn header_bytes(model: &Model<'_, '_>, expression: &Expression<'_>, seed: Symbol) -> bool {
    let Expression::ArrayExpression(array) = expression.without_parentheses() else {
        return false;
    };
    let mut slots = Vec::new();
    for element in &array.elements {
        let Some(value) = element.as_expression() else {
            return false;
        };
        slots.push(value);
    }
    slots.len() == 4
        && slots
            .iter()
            .enumerate()
            .all(|(slot, value)| header_slot(model, value, seed, slot))
}

pub(super) fn stream_call(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    stream: Symbol,
) -> bool {
    call(expression).is_some_and(|call| {
        call.arguments.is_empty() && symbol(model, &call.callee) == Some(stream)
    })
}

// ===========================================================================
// StorageEvent-like guard
// ===========================================================================

/// `<formal>.<name>`.
fn event_member(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    input: Symbol,
    name: &str,
) -> bool {
    member(expression, name).is_some_and(|object| symbol(model, object) == Some(input))
}

/// `<formal>.<name> === null` (loose or strict).
fn null_member_test(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    input: Symbol,
    name: &str,
) -> bool {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return false;
    };
    if !matches!(binary.operator.as_str(), "==" | "===") {
        return false;
    }
    event_member(model, &binary.left, input, name) && is_null(&binary.right)
}

/// `<formal>.key === <captured key>`.
fn key_test(model: &Model<'_, '_>, expression: &Expression<'_>, input: Symbol) -> Option<Symbol> {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return None;
    };
    if !matches!(binary.operator.as_str(), "==" | "===") {
        return None;
    }
    if !event_member(model, &binary.left, input, "key") {
        return None;
    }
    symbol(model, &binary.right)
}

/// `formal.key === null || (formal.key === capturedKey &&
/// formal.newValue === null)`, the exact removal-or-keyed-removal test.
pub(super) fn storage_guard(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    input: Symbol,
) -> Option<Symbol> {
    let (key_cleared, rest) = logical(expression, "||")?;
    if !null_member_test(model, key_cleared, input, "key") {
        return None;
    }
    let (keyed, removed) = logical(rest, "&&")?;
    let key = key_test(model, keyed, input)?;
    if !null_member_test(model, removed, input, "newValue") {
        return None;
    }
    Some(key)
}

// ===========================================================================
// Frame emission (shared by the `try` success path and the `catch` re-emit)
// ===========================================================================

#[derive(Clone, Copy)]
struct Frame {
    flag_source: Symbol,
    old_source: Symbol,
    observer: Symbol,
    bit_error: Symbol,
    bit_second: Symbol,
    seed_factory: Symbol,
    writer_factory: Symbol,
    reseed: f64,
    tag: f64,
}

/// The ordered frame of the direct/cursor-shared variant: the writer binding is
/// also the cursor and the flag lanes are `aux | 0 | removal << 1 | error << 2`,
/// so the three captured bit slots are compared in lane order rather than as the
/// holder variant's `flag`/`error`/`second` roles.
#[derive(Clone, Copy)]
struct DirectFrame {
    old_source: Symbol,
    observer: Symbol,
    bit_aux: Symbol,
    bit_removal: Symbol,
    bit_error: Symbol,
    seed_factory: Symbol,
    writer_factory: Symbol,
    reseed: f64,
    tag: f64,
}

pub(super) fn seed_declaration(model: &Model<'_, '_>, statement: &Statement<'_>) -> Option<Symbol> {
    let variable = one_variable(statement)?;
    let seed = plain_local(model, variable)?;
    if !random_seed(model, variable.init.as_ref()?) {
        return None;
    }
    // The declaration reads the fresh nonce and the single later assignment
    // reseeds that exact local into the stream.
    if model.binding_writes(seed) != 1 {
        return None;
    }
    Some(seed)
}

pub(super) fn header_declaration(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    seed: Symbol,
) -> Option<Symbol> {
    let variable = one_variable(statement)?;
    let header = plain_local(model, variable)?;
    if header == seed || model.binding_writes(header) != 0 {
        return None;
    }
    if !header_bytes(model, variable.init.as_ref()?, seed) {
        return None;
    }
    Some(header)
}

/// `seed = <resolved seed factory>(seed ^ <number>)`; returns the factory
/// symbol and the reseed literal so both blocks can be compared.
pub(super) fn reseed_declaration(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    seed: Symbol,
) -> Option<(Symbol, f64)> {
    let assignment = assignment(expression_statement(statement)?)?;
    if assigned_symbol(model, assignment) != Some(seed) {
        return None;
    }
    let (call, factory, factory_site) = factory_call(model, site, &assignment.right)?;
    if !factory_returns_live_child(model, factory_site, FactoryUse::Stream) {
        return None;
    }
    let arguments = arity(call, 1)?;
    let (base, literal) = binary(arguments[0], "^")?;
    if !seed_base(model, base, seed) {
        return None;
    }
    let Expression::NumericLiteral(literal) = literal.without_parentheses() else {
        return None;
    };
    Some((factory, literal.value))
}

/// `let writer = <resolved writer factory>(stream)`.
pub(super) fn writer_declaration(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    seed: Symbol,
    header: Symbol,
) -> Option<(Symbol, Symbol)> {
    let variable = one_variable(statement)?;
    let writer = plain_local(model, variable)?;
    if writer == seed || writer == header || model.binding_writes(writer) != 1 {
        return None;
    }
    let (call, factory, factory_site) = factory_call(model, site, variable.init.as_ref()?)?;
    if !factory_returns_live_child(model, factory_site, FactoryUse::Writer) {
        return None;
    }
    let arguments = arity(call, 1)?;
    if symbol(model, arguments[0]) != Some(seed) {
        return None;
    }
    Some((writer, factory))
}

/// `let writer = <resolved writer factory>(stream)` for the fixed-slot variant,
/// where the writer binding is never reassigned: the else branch invokes it
/// directly with the literal cursor `4` and discards the call result, so exactly
/// zero write references are expected.
pub(super) fn pure_writer_declaration(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    seed: Symbol,
    header: Symbol,
) -> Option<(Symbol, Symbol)> {
    let variable = one_variable(statement)?;
    let writer = plain_local(model, variable)?;
    if writer == seed || writer == header || model.binding_writes(writer) != 0 {
        return None;
    }
    let (call, factory, factory_site) = factory_call(model, site, variable.init.as_ref()?)?;
    if !factory_returns_live_child(model, factory_site, FactoryUse::Writer) {
        return None;
    }
    let arguments = arity(call, 1)?;
    if symbol(model, arguments[0]) != Some(seed) {
        return None;
    }
    Some((writer, factory))
}

/// `let writer = <resolved writer factory>(stream)` for the direct variant, where
/// the same binding is also the cursor: it is re-assigned exactly once in each
/// branch of the `old == null` test (the literal cursor `4` and the direct writer
/// re-invocation result), so exactly two write references are expected and the
/// three-element holder is not required.
pub(super) fn cursor_writer_declaration(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    seed: Symbol,
    header: Symbol,
) -> Option<(Symbol, Symbol)> {
    let variable = one_variable(statement)?;
    let writer = plain_local(model, variable)?;
    if writer == seed || writer == header || model.binding_writes(writer) != 2 {
        return None;
    }
    let (call, factory, factory_site) = factory_call(model, site, variable.init.as_ref()?)?;
    if !factory_returns_live_child(model, factory_site, FactoryUse::Writer) {
        return None;
    }
    let arguments = arity(call, 1)?;
    if symbol(model, arguments[0]) != Some(seed) {
        return None;
    }
    Some((writer, factory))
}

/// `let alias = <captured slot>`.
pub(super) fn alias_declaration(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    excluded: &[Symbol],
) -> Option<(Symbol, Symbol)> {
    let variable = one_variable(statement)?;
    let alias = plain_local(model, variable)?;
    if excluded.contains(&alias) || model.binding_writes(alias) != 0 {
        return None;
    }
    let source = symbol(model, variable.init.as_ref()?)?;
    let source = captured_local(model, site, source)?;
    if source == alias {
        return None;
    }
    Some((alias, source))
}

/// `let cursor;` written exactly once in each branch.
pub(super) fn cursor_declaration(
    model: &Model<'_, '_>,
    statement: &Statement<'_>,
    excluded: &[Symbol],
) -> Option<Symbol> {
    let variable = one_variable(statement)?;
    let cursor = plain_local(model, variable)?;
    if variable.init.is_some() || excluded.contains(&cursor) {
        return None;
    }
    if model.binding_writes(cursor) != 2 {
        return None;
    }
    Some(cursor)
}

/// The `old == null` branch: `header[4] = stream() ^ <tag>; cursor = 4;`.
pub(super) fn null_branch(
    model: &Model<'_, '_>,
    header: Symbol,
    seed: Symbol,
    cursor: Symbol,
    statements: &[Statement<'_>],
) -> Option<f64> {
    let [write, position] = statements else {
        return None;
    };
    let write = assignment(expression_statement(write)?)?;
    let (object, index) = member_target(&write.left)?;
    if symbol(model, object) != Some(header) || !number(index, 4.0) {
        return None;
    }
    let (value, tag) = binary(&write.right, "^")?;
    if !stream_call(model, value, seed) {
        return None;
    }
    let Expression::NumericLiteral(tag) = tag.without_parentheses() else {
        return None;
    };
    let position = assignment(expression_statement(position)?)?;
    if assigned_symbol(model, position) != Some(cursor) || !number(&position.right, 4.0) {
        return None;
    }
    Some(tag.value)
}

/// The holder branch: `writer = [writer, old, header]; cursor =
/// writer[0](writer[1], writer[2], 3);`.
fn holder_branch(
    model: &Model<'_, '_>,
    writer: Symbol,
    old: Symbol,
    header: Symbol,
    cursor: Symbol,
    statements: &[Statement<'_>],
) -> Option<()> {
    let [holder, position] = statements else {
        return None;
    };
    let holder = assignment(expression_statement(holder)?)?;
    if assigned_symbol(model, holder) != Some(writer) {
        return None;
    }
    let Expression::ArrayExpression(array) = holder.right.without_parentheses() else {
        return None;
    };
    let mut elements = Vec::new();
    for element in &array.elements {
        elements.push(element.as_expression()?);
    }
    let [first, second, third] = elements.as_slice() else {
        return None;
    };
    if symbol(model, first) != Some(writer)
        || symbol(model, second) != Some(old)
        || symbol(model, third) != Some(header)
    {
        return None;
    }

    let position = assignment(expression_statement(position)?)?;
    if assigned_symbol(model, position) != Some(cursor) {
        return None;
    }
    let call = call(&position.right)?;
    let (object, index) = computed(&call.callee)?;
    if symbol(model, object) != Some(writer) || !number(index, 0.0) {
        return None;
    }
    let arguments = arity(call, 3)?;
    let (first_object, first_index) = computed(arguments[0])?;
    let (second_object, second_index) = computed(arguments[1])?;
    if symbol(model, first_object) != Some(writer) || !number(first_index, 1.0) {
        return None;
    }
    if symbol(model, second_object) != Some(writer) || !number(second_index, 2.0) {
        return None;
    }
    if !number(arguments[2], 3.0) {
        return None;
    }
    Some(())
}

/// The direct writer branch: `writer = writer(old, header, 3);`.  The writer
/// value is re-invoked directly instead of through `[writer, old, header]`, and
/// the numeric cursor result is stored back into the same writer binding.
pub(super) fn direct_writer_branch(
    model: &Model<'_, '_>,
    writer: Symbol,
    old: Symbol,
    header: Symbol,
    statements: &[Statement<'_>],
) -> Option<()> {
    let [position] = statements else {
        return None;
    };
    let position = assignment(expression_statement(position)?)?;
    if assigned_symbol(model, position) != Some(writer) {
        return None;
    }
    let call = call(&position.right)?;
    if symbol(model, &call.callee) != Some(writer) {
        return None;
    }
    let arguments = arity(call, 3)?;
    if symbol(model, arguments[0]) != Some(old) {
        return None;
    }
    if symbol(model, arguments[1]) != Some(header) {
        return None;
    }
    if !number(arguments[2], 3.0) {
        return None;
    }
    Some(())
}

/// The fixed-slot null branch: `header[5] = stream() ^ <tag>;`.  This is the
/// next real sibling of the direct variant: the flag append is emitted at the
/// literal slot `4` *before* the `old == null` test, so the null branch writes
/// the tag at the literal slot `5` and the other branch invokes the writer with
/// the literal cursor `4`.  There is no cursor binding in this variant.
pub(super) fn fixed_null_branch(
    model: &Model<'_, '_>,
    header: Symbol,
    seed: Symbol,
    statements: &[Statement<'_>],
) -> Option<f64> {
    let [write] = statements else {
        return None;
    };
    let write = assignment(expression_statement(write)?)?;
    let (object, index) = member_target(&write.left)?;
    if symbol(model, object) != Some(header) || !number(index, 5.0) {
        return None;
    }
    let (value, tag) = binary(&write.right, "^")?;
    if !stream_call(model, value, seed) {
        return None;
    }
    let Expression::NumericLiteral(tag) = tag.without_parentheses() else {
        return None;
    };
    Some(tag.value)
}

/// The fixed-slot writer branch: the writer value is invoked directly with the
/// literal cursor `4`, and the call result is discarded (the header slot was
/// already written before the branch).
fn fixed_writer_branch(
    model: &Model<'_, '_>,
    writer: Symbol,
    old: Symbol,
    header: Symbol,
    statements: &[Statement<'_>],
) -> Option<()> {
    let [position] = statements else {
        return None;
    };
    let call = call(expression_statement(position)?)?;
    if symbol(model, &call.callee) != Some(writer) {
        return None;
    }
    let arguments = arity(call, 3)?;
    if symbol(model, arguments[0]) != Some(old) {
        return None;
    }
    if symbol(model, arguments[1]) != Some(header) {
        return None;
    }
    if !number(arguments[2], 4.0) {
        return None;
    }
    Some(())
}

fn shifted(model: &Model<'_, '_>, expression: &Expression<'_>, amount: f64) -> Option<Symbol> {
    let (left, right) = binary(expression, "<<")?;
    number(right, amount).then(|| symbol(model, left))?
}

/// `flag | 0 | error << 1 | second << 2`; returns the two captured bit slots.
fn flags_expression(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    flag: Symbol,
) -> Option<(Symbol, Symbol)> {
    let (left, right) = binary(expression, "|")?;
    let second = shifted(model, right, 2.0)?;
    let (left, right) = binary(left, "|")?;
    let error = shifted(model, right, 1.0)?;
    let (left, right) = binary(left, "|")?;
    if symbol(model, left) != Some(flag) || !number(right, 0.0) {
        return None;
    }
    Some((error, second))
}

/// `flag | 0 | error << 1 | second << 2`; returns the low-lane flag, the bit-1
/// and bit-2 slots without requiring a pre-known low symbol.  The late-flags
/// holder variant reads the captured removal flag directly at the low lane,
/// instead of through the reviewed holder grammar's factory-local flag alias.
pub(super) fn captured_flags_expression(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
) -> Option<(Symbol, Symbol, Symbol)> {
    let (left, right) = binary(expression, "|")?;
    let second = shifted(model, right, 2.0)?;
    let (left, right) = binary(left, "|")?;
    let error = shifted(model, right, 1.0)?;
    let (left, right) = binary(left, "|")?;
    if !number(right, 0.0) {
        return None;
    }
    let flag = symbol(model, left)?;
    Some((flag, error, second))
}

/// `aux | 0 | removal << 1 | error << 2`; returns the three captured bit slots in
/// lane order.  This is the direct variant's layout: the captured removal flag is
/// laned at bit 1 and the captured error flag at bit 2, with a distinct captured
/// auxiliary flag at bit 0.  The scalar lane order is not interchangeable with
/// [`flags_expression`], so each variant keeps its own exact parser.
fn shifted_flags_expression(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
) -> Option<(Symbol, Symbol, Symbol)> {
    let (left, right) = binary(expression, "|")?;
    let error = shifted(model, right, 2.0)?;
    let (left, right) = binary(left, "|")?;
    let removal = shifted(model, right, 1.0)?;
    let (left, right) = binary(left, "|")?;
    if !number(right, 0.0) {
        return None;
    }
    let aux = symbol(model, left)?;
    Some((aux, removal, error))
}

pub(super) fn next_frame(
    model: &Model<'_, '_>,
    site: FunctionSite,
    header: Symbol,
    statement: &Statement<'_>,
) -> Option<Symbol> {
    let call = call(expression_statement(statement)?)?;
    let receiver = method_receiver(model, call, "next")?;
    let receiver = captured_local(model, site, receiver)?;
    let arguments = arity(call, 1)?;
    let Expression::ArrayExpression(pair) = arguments[0].without_parentheses() else {
        return None;
    };
    let [first, second] = pair.elements.as_slice() else {
        return None;
    };
    if !empty_array(first.as_expression()?) {
        return None;
    }
    if symbol(model, second.as_expression()?) != Some(header) {
        return None;
    }
    Some(receiver)
}

/// One ordered frame emission.  Both the `try` success path and the `catch`
/// re-emit must satisfy this exact grammar; the caller then compares the
/// captured slots and the source-backed factories across the two frames.
fn frame_tail(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statements: &[Statement<'_>],
) -> Option<Frame> {
    let [seed_statement, header_statement, reseed_statement, writer_statement, flag_statement, old_statement, cursor_statement, branch, append, next] =
        statements
    else {
        return None;
    };
    let seed = seed_declaration(model, seed_statement)?;
    let header = header_declaration(model, header_statement, seed)?;
    let (seed_factory, reseed) = reseed_declaration(model, site, reseed_statement, seed)?;
    let (writer, writer_factory) = writer_declaration(model, site, writer_statement, seed, header)?;
    if seed_factory == writer_factory {
        return None;
    }
    let (flag_alias, flag_source) =
        alias_declaration(model, site, flag_statement, &[seed, header, writer])?;
    let (old_alias, old_source) = alias_declaration(
        model,
        site,
        old_statement,
        &[seed, header, writer, flag_alias],
    )?;
    let cursor = cursor_declaration(
        model,
        cursor_statement,
        &[seed, header, writer, flag_alias, old_alias],
    )?;

    let Statement::IfStatement(branch) = branch else {
        return None;
    };
    let alternate = branch.alternate.as_ref()?;
    let Expression::BinaryExpression(test) = branch.test.without_parentheses() else {
        return None;
    };
    if !matches!(test.operator.as_str(), "==" | "===") {
        return None;
    }
    if symbol(model, &test.left) != Some(old_alias) || !is_null(&test.right) {
        return None;
    }
    let tag = null_branch(model, header, seed, cursor, block(&branch.consequent)?)?;
    holder_branch(model, writer, old_alias, header, cursor, block(alternate)?)?;

    let append = assignment(expression_statement(append)?)?;
    let (object, index) = member_target(&append.left)?;
    if symbol(model, object) != Some(header) {
        return None;
    }
    let (cursor_term, one) = binary(index, "+")?;
    if symbol(model, cursor_term) != Some(cursor) || !number(one, 1.0) {
        return None;
    }
    let (flags, stream) = binary(&append.right, "^")?;
    let (bit_error, bit_second) = flags_expression(model, flags, flag_alias)?;
    if !stream_call(model, stream, seed) {
        return None;
    }
    if captured_local(model, site, bit_error).is_none() {
        return None;
    }
    if captured_local(model, site, bit_second).is_none() {
        return None;
    }
    let observer = next_frame(model, site, header, next)?;

    Some(Frame {
        flag_source,
        old_source,
        observer,
        bit_error,
        bit_second,
        seed_factory,
        writer_factory,
        reseed,
        tag,
    })
}

/// One ordered frame emission of the direct/cursor-shared variant.  It shares the
/// seed, header, reseed, writer and observer readers with [`frame_tail`], but the
/// tail has exactly eight statements: the old-value alias is the only alias, the
/// writer binding is the cursor (assigned `4` in the null branch and the direct
/// call result otherwise), the writer is re-invoked without a holder, and the
/// flag lanes are `aux | 0 | removal << 1 | error << 2`.  Both the `try` success
/// path and the `catch` re-emit must satisfy this exact grammar and agree on
/// every captured slot and source-backed factory.
fn direct_frame_tail(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statements: &[Statement<'_>],
) -> Option<DirectFrame> {
    let [seed_statement, header_statement, reseed_statement, writer_statement, old_statement, branch, append, next] =
        statements
    else {
        return None;
    };
    let seed = seed_declaration(model, seed_statement)?;
    let header = header_declaration(model, header_statement, seed)?;
    let (seed_factory, reseed) = reseed_declaration(model, site, reseed_statement, seed)?;
    let (writer, writer_factory) =
        cursor_writer_declaration(model, site, writer_statement, seed, header)?;
    if seed_factory == writer_factory {
        return None;
    }
    let (old_alias, old_source) =
        alias_declaration(model, site, old_statement, &[seed, header, writer])?;

    let Statement::IfStatement(branch) = branch else {
        return None;
    };
    let alternate = branch.alternate.as_ref()?;
    let Expression::BinaryExpression(test) = branch.test.without_parentheses() else {
        return None;
    };
    if !matches!(test.operator.as_str(), "==" | "===") {
        return None;
    }
    if symbol(model, &test.left) != Some(old_alias) || !is_null(&test.right) {
        return None;
    }
    let tag = null_branch(model, header, seed, writer, block(&branch.consequent)?)?;
    direct_writer_branch(model, writer, old_alias, header, block(alternate)?)?;

    let append = assignment(expression_statement(append)?)?;
    let (object, index) = member_target(&append.left)?;
    if symbol(model, object) != Some(header) {
        return None;
    }
    let (cursor_term, one) = binary(index, "+")?;
    if symbol(model, cursor_term) != Some(writer) || !number(one, 1.0) {
        return None;
    }
    let (flags, stream) = binary(&append.right, "^")?;
    let (bit_aux, bit_removal, bit_error) = shifted_flags_expression(model, flags)?;
    if !stream_call(model, stream, seed) {
        return None;
    }
    if captured_local(model, site, bit_aux).is_none() {
        return None;
    }
    if captured_local(model, site, bit_removal).is_none() {
        return None;
    }
    if captured_local(model, site, bit_error).is_none() {
        return None;
    }
    let observer = next_frame(model, site, header, next)?;

    Some(DirectFrame {
        old_source,
        observer,
        bit_aux,
        bit_removal,
        bit_error,
        seed_factory,
        writer_factory,
        reseed,
        tag,
    })
}

/// Exact direct-writer variant with an immutable, precombined low/two-bit flag
/// value. Both frame paths must preserve the same lane order and captured slots.
fn combined_flags_frame_tail(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statements: &[Statement<'_>],
    layout: u8,
) -> Option<DirectFrame> {
    let [seed_statement, header_statement, reseed_statement, writer_statement, flag_statement, old_statement, branch, append, next] =
        statements
    else {
        return None;
    };
    let seed = seed_declaration(model, seed_statement)?;
    let header = header_declaration(model, header_statement, seed)?;
    let (seed_factory, reseed) = reseed_declaration(model, site, reseed_statement, seed)?;
    let (writer, writer_factory) =
        cursor_writer_declaration(model, site, writer_statement, seed, header)?;
    if seed_factory == writer_factory {
        return None;
    }
    let flag_declaration = one_variable(flag_statement)?;
    let flag_alias = plain_local(model, flag_declaration)?;
    if model.binding_writes(flag_alias) != 0 {
        return None;
    }
    let (low, middle) = binary(flag_declaration.init.as_ref()?, "|")?;
    let bit_one = shifted(model, middle, 1.0)?;
    let (aux, zero) = binary(low, "|")?;
    if !number(zero, 0.0) {
        return None;
    }
    let bit_zero = symbol(model, aux)?;
    let (old_alias, old_source) =
        alias_declaration(model, site, old_statement, &[seed, header, writer])?;

    let Statement::IfStatement(branch) = branch else {
        return None;
    };
    let alternate = branch.alternate.as_ref()?;
    let Expression::BinaryExpression(test) = branch.test.without_parentheses() else {
        return None;
    };
    if !matches!(test.operator.as_str(), "==" | "===") {
        return None;
    }
    if symbol(model, &test.left) != Some(old_alias) || !is_null(&test.right) {
        return None;
    }
    let tag = null_branch(model, header, seed, writer, block(&branch.consequent)?)?;
    direct_writer_branch(model, writer, old_alias, header, block(alternate)?)?;

    let append = assignment(expression_statement(append)?)?;
    let (object, index) = member_target(&append.left)?;
    if symbol(model, object) != Some(header) {
        return None;
    }
    let (cursor_term, one) = binary(index, "+")?;
    if symbol(model, cursor_term) != Some(writer) || !number(one, 1.0) {
        return None;
    }
    let (flags, stream) = binary(&append.right, "^")?;
    let (alias, high) = binary(flags, "|")?;
    if symbol(model, alias) != Some(flag_alias) {
        return None;
    }
    let bit_two = shifted(model, high, 2.0)?;
    let (bit_aux, bit_removal, bit_error) = match layout {
        0 => (bit_zero, bit_one, bit_two),
        1 => (bit_zero, bit_two, bit_one),
        2 => (bit_two, bit_one, bit_zero),
        _ => return None,
    };

    if !stream_call(model, stream, seed) {
        return None;
    }
    if captured_local(model, site, bit_aux).is_none() {
        return None;
    }
    if captured_local(model, site, bit_removal).is_none() {
        return None;
    }
    if captured_local(model, site, bit_error).is_none() {
        return None;
    }
    let observer = next_frame(model, site, header, next)?;

    Some(DirectFrame {
        old_source,
        observer,
        bit_aux,
        bit_removal,
        bit_error,
        seed_factory,
        writer_factory,
        reseed,
        tag,
    })
}

// Independent generic schema: removal/error are snapshotted before the value
// writer; auxiliary is read after it. This parser is never a specialized route.
fn keyed_partial_frame(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statements: &[Statement<'_>],
) -> Option<DirectFrame> {
    let [seed_statement, header_statement, reseed_statement, writer_statement, flag_statement, old_statement, branch, append, next] =
        statements
    else {
        return None;
    };
    let seed = seed_declaration(model, seed_statement)?;
    let header = header_declaration(model, header_statement, seed)?;
    let (seed_factory, reseed) = reseed_declaration(model, site, reseed_statement, seed)?;
    let (writer, writer_factory) =
        cursor_writer_declaration(model, site, writer_statement, seed, header)?;
    if seed_factory == writer_factory {
        return None;
    }
    let flag_declaration = one_variable(flag_statement)?;
    let flag_alias = plain_local(model, flag_declaration)?;
    if model.binding_writes(flag_alias) != 0 {
        return None;
    }
    let (low, middle) = binary(flag_declaration.init.as_ref()?, "|")?;
    let bit_one = shifted(model, middle, 1.0)?;
    let (aux, zero) = binary(low, "|")?;
    if !number(zero, 0.0) {
        return None;
    }
    let bit_zero = symbol(model, aux)?;
    let (old_alias, old_source) =
        alias_declaration(model, site, old_statement, &[seed, header, writer])?;

    let Statement::IfStatement(branch) = branch else {
        return None;
    };
    let alternate = branch.alternate.as_ref()?;
    let Expression::BinaryExpression(test) = branch.test.without_parentheses() else {
        return None;
    };
    if test.operator.as_str() != "==" {
        return None;
    }
    if symbol(model, &test.left) != Some(old_alias) || !is_null(&test.right) {
        return None;
    }
    let tag = null_branch(model, header, seed, writer, block(&branch.consequent)?)?;
    direct_writer_branch(model, writer, old_alias, header, block(alternate)?)?;

    let append = assignment(expression_statement(append)?)?;
    let (object, index) = member_target(&append.left)?;
    if symbol(model, object) != Some(header) {
        return None;
    }
    let (cursor_term, one) = binary(index, "+")?;
    if symbol(model, cursor_term) != Some(writer) || !number(one, 1.0) {
        return None;
    }
    let (flags, stream) = binary(&append.right, "^")?;
    let (alias, high) = binary(flags, "|")?;
    if symbol(model, alias) != Some(flag_alias) {
        return None;
    }
    let bit_two = shifted(model, high, 2.0)?;
    let (bit_removal, bit_error, bit_aux) = (bit_zero, bit_one, bit_two);

    if !stream_call(model, stream, seed) {
        return None;
    }
    if captured_local(model, site, bit_aux).is_none() {
        return None;
    }
    if captured_local(model, site, bit_removal).is_none() {
        return None;
    }
    if captured_local(model, site, bit_error).is_none() {
        return None;
    }
    let observer = next_frame(model, site, header, next)?;

    Some(DirectFrame {
        old_source,
        observer,
        bit_aux,
        bit_removal,
        bit_error,
        seed_factory,
        writer_factory,
        reseed,
        tag,
    })
}

/// One ordered frame emission of the fixed-slot variant (the real
/// callback).  It shares the seed, header, reseed, writer, old-value and
/// observer readers with [`frame_tail`], but the tail has exactly eight
/// statements and the append comes *before* the `old == null` test: the flag
/// lanes are written at the literal slot `4` first, then the branch writes the
/// literal tag byte at slot `5` or invokes the writer directly with the literal
/// cursor `4`.  The writer binding is never reassigned and no cursor binding is
/// introduced.  Both the `try` success path and the `catch` re-emit must satisfy
/// this exact grammar and agree on every captured slot and source-backed
/// factory.
fn fixed_frame_tail(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statements: &[Statement<'_>],
) -> Option<DirectFrame> {
    let [seed_statement, header_statement, reseed_statement, writer_statement, flags_statement, old_statement, branch, next] =
        statements
    else {
        return None;
    };
    let seed = seed_declaration(model, seed_statement)?;
    let header = header_declaration(model, header_statement, seed)?;
    let (seed_factory, reseed) = reseed_declaration(model, site, reseed_statement, seed)?;
    let (writer, writer_factory) =
        pure_writer_declaration(model, site, writer_statement, seed, header)?;
    if seed_factory == writer_factory {
        return None;
    }
    let (old_alias, old_source) =
        alias_declaration(model, site, old_statement, &[seed, header, writer])?;

    // The flag lanes are appended at the literal header slot 4 before the
    // `old == null` test, unlike the direct variant where the same lanes are
    // appended at `header[writer + 1]` after the branch.
    let append = assignment(expression_statement(flags_statement)?)?;
    let (object, index) = member_target(&append.left)?;
    if symbol(model, object) != Some(header) || !number(index, 4.0) {
        return None;
    }
    let (flags, stream) = binary(&append.right, "^")?;
    let (bit_aux, bit_removal, bit_error) = shifted_flags_expression(model, flags)?;
    if !stream_call(model, stream, seed) {
        return None;
    }
    if captured_local(model, site, bit_aux).is_none() {
        return None;
    }
    if captured_local(model, site, bit_removal).is_none() {
        return None;
    }
    if captured_local(model, site, bit_error).is_none() {
        return None;
    }

    let Statement::IfStatement(branch) = branch else {
        return None;
    };
    let alternate = branch.alternate.as_ref()?;
    let Expression::BinaryExpression(test) = branch.test.without_parentheses() else {
        return None;
    };
    if !matches!(test.operator.as_str(), "==" | "===") {
        return None;
    }
    if symbol(model, &test.left) != Some(old_alias) || !is_null(&test.right) {
        return None;
    }
    let tag = fixed_null_branch(model, header, seed, block(&branch.consequent)?)?;
    fixed_writer_branch(model, writer, old_alias, header, block(alternate)?)?;

    let observer = next_frame(model, site, header, next)?;

    Some(DirectFrame {
        old_source,
        observer,
        bit_aux,
        bit_removal,
        bit_error,
        seed_factory,
        writer_factory,
        reseed,
        tag,
    })
}

/// One ordered frame emission of the late-flags holder variant (the real
/// "more" callback).  It reuses the reviewed holder branch and cursor readers,
/// but it has no captured-flag alias statement: the tail has exactly nine
/// statements and the `flag | 0 | error << 1 | second << 2` lanes are appended at
/// `header[cursor + 1]` after the branch, reading the captured removal flag
/// directly at the low lane.  Both the `try` success path and the `catch`
/// re-emit must satisfy this exact grammar and agree on every captured slot and
/// source-backed factory.
fn holder_late_flags_frame_tail(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statements: &[Statement<'_>],
) -> Option<Frame> {
    let [seed_statement, header_statement, reseed_statement, writer_statement, old_statement, cursor_statement, branch, append, next] =
        statements
    else {
        return None;
    };
    let seed = seed_declaration(model, seed_statement)?;
    let header = header_declaration(model, header_statement, seed)?;
    let (seed_factory, reseed) = reseed_declaration(model, site, reseed_statement, seed)?;
    let (writer, writer_factory) = writer_declaration(model, site, writer_statement, seed, header)?;
    if seed_factory == writer_factory {
        return None;
    }
    let (old_alias, old_source) =
        alias_declaration(model, site, old_statement, &[seed, header, writer])?;
    let cursor = cursor_declaration(model, cursor_statement, &[seed, header, writer, old_alias])?;

    let Statement::IfStatement(branch) = branch else {
        return None;
    };
    let alternate = branch.alternate.as_ref()?;
    let Expression::BinaryExpression(test) = branch.test.without_parentheses() else {
        return None;
    };
    if !matches!(test.operator.as_str(), "==" | "===") {
        return None;
    }
    if symbol(model, &test.left) != Some(old_alias) || !is_null(&test.right) {
        return None;
    }
    let tag = null_branch(model, header, seed, cursor, block(&branch.consequent)?)?;
    holder_branch(model, writer, old_alias, header, cursor, block(alternate)?)?;

    let append = assignment(expression_statement(append)?)?;
    let (object, index) = member_target(&append.left)?;
    if symbol(model, object) != Some(header) {
        return None;
    }
    let (cursor_term, one) = binary(index, "+")?;
    if symbol(model, cursor_term) != Some(cursor) || !number(one, 1.0) {
        return None;
    }
    let (flags, stream) = binary(&append.right, "^")?;
    let (flag_source, bit_error, bit_second) = captured_flags_expression(model, flags)?;
    if !stream_call(model, stream, seed) {
        return None;
    }
    if captured_local(model, site, flag_source).is_none() {
        return None;
    }
    if captured_local(model, site, bit_error).is_none() {
        return None;
    }
    if captured_local(model, site, bit_second).is_none() {
        return None;
    }
    let observer = next_frame(model, site, header, next)?;

    Some(Frame {
        flag_source,
        old_source,
        observer,
        bit_error,
        bit_second,
        seed_factory,
        writer_factory,
        reseed,
        tag,
    })
}

// ===========================================================================
// Owned callback body
// ===========================================================================

/// The success prefix: the removal flag and the null-coalesced old value.
pub(super) fn removal_state(
    model: &Model<'_, '_>,
    site: FunctionSite,
    input: Symbol,
    first: &Statement<'_>,
    second: &Statement<'_>,
) -> Option<(Symbol, Symbol)> {
    let flag_assignment = assignment(expression_statement(first)?)?;
    let removal = assigned_symbol(model, flag_assignment)?;
    if !boolean_true(&flag_assignment.right) {
        return None;
    }
    let removal = captured_local(model, site, removal)?;

    let old_assignment = assignment(expression_statement(second)?)?;
    let old = assigned_symbol(model, old_assignment)?;
    if old == removal {
        return None;
    }
    let old = captured_local(model, site, old)?;
    let (value, null) = logical(&old_assignment.right, "??")?;
    if !event_member(model, value, input, "oldValue") || !is_null(null) {
        return None;
    }
    Some((removal, old))
}

/// The catch prefix: the error flag.
pub(super) fn error_state(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
) -> Option<Symbol> {
    let flag_assignment = assignment(expression_statement(statement)?)?;
    let error = assigned_symbol(model, flag_assignment)?;
    if !boolean_true(&flag_assignment.right) {
        return None;
    }
    captured_local(model, site, error)
}

fn same_frames(success: &Frame, catch: &Frame) -> bool {
    success.flag_source == catch.flag_source
        && success.old_source == catch.old_source
        && success.observer == catch.observer
        && success.bit_error == catch.bit_error
        && success.bit_second == catch.bit_second
        && success.seed_factory == catch.seed_factory
        && success.writer_factory == catch.writer_factory
        && success.reseed == catch.reseed
        && success.tag == catch.tag
}

fn same_direct_frames(success: &DirectFrame, catch: &DirectFrame) -> bool {
    success.old_source == catch.old_source
        && success.observer == catch.observer
        && success.bit_aux == catch.bit_aux
        && success.bit_removal == catch.bit_removal
        && success.bit_error == catch.bit_error
        && success.seed_factory == catch.seed_factory
        && success.writer_factory == catch.writer_factory
        && success.reseed == catch.reseed
        && success.tag == catch.tag
}

pub(super) fn distinct(symbols: &[Symbol]) -> bool {
    symbols.iter().copied().collect::<BTreeSet<_>>().len() == symbols.len()
}

fn storage_removal(
    model: &Model<'_, '_>,
    site: FunctionSite,
    input: Symbol,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [Statement::TryStatement(try_)] = body.statements.as_slice() else {
        return None;
    };
    if try_.finalizer.is_some() {
        return None;
    }
    let [Statement::IfStatement(guard)] = try_.block.body.as_slice() else {
        return None;
    };
    if guard.alternate.is_some() {
        return None;
    }
    let key = storage_guard(model, &guard.test, input)?;
    let key = captured_local(model, site, key)?;

    let success = block(&guard.consequent)?;
    let [removal, old, success_tail @ ..] = success else {
        return None;
    };
    let (removal_flag, old_slot) = removal_state(model, site, input, removal, old)?;

    let handler = try_.handler.as_ref()?;
    let catch_parameter = handler.param.as_ref()?;
    let BindingPattern::BindingIdentifier(catch_identifier) = &catch_parameter.pattern else {
        return None;
    };
    let caught = catch_identifier.symbol_id.get()?;
    if model.binding_writes(caught) != 0 {
        return None;
    }

    let catch = handler.body.body.as_slice();
    let [error, catch_tail @ ..] = catch else {
        return None;
    };
    let error_flag = error_state(model, site, error)?;

    // Variant A is the reviewed holder form: a captured flag alias, a separate
    // never-initialized cursor, `[writer, old, header]` and the
    // `flag|0|error<<1|second<<2` lanes.  It stays the first-choice grammar, so
    // the existing positives and negatives are decided exactly as before.  The
    // late-flags holder form (the real "more" callback) is the same
    // holder grammar with the captured removal flag read directly at the low
    // lane instead of through a factory-local alias; it is tried only after the
    // reviewed form declines and its catch re-emit must match the same form.
    let holder = frame_tail(model, site, success_tail)
        .map(|frame| (frame, 0u8))
        .or_else(|| {
            holder_late_flags_frame_tail(model, site, success_tail).map(|frame| (frame, 1))
        });
    if let Some((success_frame, holder_variant)) = holder {
        if success_frame.flag_source != removal_flag || success_frame.old_source != old_slot {
            return None;
        }
        let catch_frame = match holder_variant {
            0 => frame_tail(model, site, catch_tail),
            _ => holder_late_flags_frame_tail(model, site, catch_tail),
        }?;
        if catch_frame.bit_error != error_flag {
            return None;
        }
        if !same_frames(&success_frame, &catch_frame) {
            return None;
        }

        // The same captured state must be six distinct stable slots, and the two
        // source-backed factories are their own slots, never borrowed state.
        let state = [
            key,
            success_frame.flag_source,
            success_frame.old_source,
            success_frame.bit_error,
            success_frame.bit_second,
            success_frame.observer,
        ];
        if !distinct(&state) {
            return None;
        }
        if state.contains(&success_frame.seed_factory)
            || state.contains(&success_frame.writer_factory)
        {
            return None;
        }
        return Some(());
    }

    // Variant B is the direct/cursor-shared form (the real slice): the
    // writer binding is the cursor, the writer is re-invoked directly as
    // `writer(old, header, 3)`, and the lanes are `aux|0|removal<<1|error<<2`.
    // Variant C is the fixed-slot form (the real callback): the flag
    // lanes are appended at the literal slot 4 before the branch, the null
    // branch writes the tag at slot 5 and the other branch invokes the writer
    // with the literal cursor 4.  Each is reached only when every earlier exact
    // grammar declined, and the catch re-emit must match the same variant, so no
    // existing claim is reordered or weakened.
    let (success_frame, variant): (DirectFrame, u8) = if let Some(frame) =
        direct_frame_tail(model, site, success_tail)
    {
        (frame, 0)
    } else if let Some(frame) = fixed_frame_tail(model, site, success_tail) {
        (frame, 1)
    } else {
        (0..=2).find_map(|layout| {
            combined_flags_frame_tail(model, site, success_tail, layout)
                .filter(|frame| frame.bit_removal == removal_flag && frame.bit_error == error_flag)
                .map(|frame| (frame, layout + 2))
        })?
    };
    if success_frame.bit_removal != removal_flag || success_frame.old_source != old_slot {
        return None;
    }
    let catch_frame = match variant {
        0 => direct_frame_tail(model, site, catch_tail),
        1 => fixed_frame_tail(model, site, catch_tail),
        _ => combined_flags_frame_tail(model, site, catch_tail, variant - 2),
    }?;
    if catch_frame.bit_error != error_flag {
        return None;
    }
    if !same_direct_frames(&success_frame, &catch_frame) {
        return None;
    }
    let state = [
        key,
        success_frame.bit_removal,
        success_frame.old_source,
        success_frame.bit_aux,
        success_frame.bit_error,
        success_frame.observer,
    ];
    if !distinct(&state) {
        return None;
    }
    if state.contains(&success_frame.seed_factory) || state.contains(&success_frame.writer_factory)
    {
        return None;
    }
    Some(())
}

/// Direct unconditional initialization in the callback's actual enclosing body.
/// A declaration alone is not initialization; no child/sibling assignment counts.
fn parent_initialization<'a>(
    model: &Model<'_, '_>,
    parent: FunctionSite,
    statements: &'a [Statement<'a>],
    slot: Symbol,
) -> Option<(&'a Expression<'a>, u32)> {
    for statement in statements {
        if let Statement::VariableDeclaration(declaration) = statement {
            for variable in &declaration.declarations {
                if matches!(&variable.id, BindingPattern::BindingIdentifier(id)
                    if id.symbol_id.get() == Some(slot))
                {
                    if let Some(init) = &variable.init {
                        return Some((init, statement.span().end));
                    }
                }
            }
        }
        if let Some(write) = expression_statement(statement).and_then(assignment) {
            if assigned_symbol(model, write) == Some(slot) {
                // The enclosing-function identity is checked independently of text.
                if !own_body_node(model, model.scoping.symbol_declaration(slot), parent) {
                    return None;
                }
                return Some((&write.right, statement.span().end));
            }
        }
    }
    None
}

/// Origin for an opaque input, not a native/global identity: a stable parent
/// formal, or an immutable alias of it, with static/numeric member projections.
fn parent_input_origin(
    model: &Model<'_, '_>,
    parent: FunctionSite,
    expression: &Expression<'_>,
    before: u32,
    depth: usize,
) -> bool {
    if depth > 4 {
        return false;
    }
    if let Some(slot) = symbol(model, expression) {
        if let Some(formal) = model.formal_symbol_slot(slot) {
            return formal.owner == parent.node_id && model.binding_writes(slot) == 0;
        }
        if model.binding_writes(slot) != 0 {
            return false;
        }
        let AstKind::VariableDeclarator(variable) =
            model.nodes.kind(model.scoping.symbol_declaration(slot))
        else {
            return false;
        };
        return variable.span.end < before
            && own_body_node(model, model.scoping.symbol_declaration(slot), parent)
            && variable.init.as_ref().is_some_and(|init| {
                parent_input_origin(model, parent, init, variable.span.start, depth + 1)
            });
    }
    match expression.without_parentheses() {
        Expression::StaticMemberExpression(member) if !member.optional => {
            parent_input_origin(model, parent, &member.object, before, depth + 1)
        }
        Expression::ComputedMemberExpression(member)
            if !member.optional
                && matches!(
                    member.expression.without_parentheses(),
                    Expression::NumericLiteral(_)
                ) =>
        {
            parent_input_origin(model, parent, &member.object, before, depth + 1)
        }
        _ => false,
    }
}

/// A late lexical read inside a hoisted helper is not proof of late execution.
/// Walk its own binding references until the setup-owning function is reached.
/// Arrows/anonymous expressions also require their creation after the boundary.
fn exposure_after_setup(
    model: &Model<'_, '_>,
    node: oxc_semantic::NodeId,
    setup_owner: FunctionSite,
    boundary: u32,
    depth: usize,
) -> bool {
    if depth > 12 || model.nodes.kind(node).span().start < boundary {
        return false;
    }
    let Some(owner) = model.owning_site(node) else {
        return false;
    };
    if owner.span == setup_owner.span {
        return true;
    }
    if let Some(binding) = owner.binding_symbol {
        return model.scoping.get_resolved_references(binding).all(|r| {
            r.is_read()
                && exposure_after_setup(model, r.node_id(), setup_owner, boundary, depth + 1)
        });
    }
    exposure_after_setup(model, owner.node_id, setup_owner, boundary, depth + 1)
}

/// All reads of the hoisted callback and its single local alias are checked.
/// Creating a local alias before initialization is allowed; calling, returning,
/// registering or otherwise exposing either binding before setup is refused.
/// Reads in children defined earlier are conservatively refused too.
fn initialized_keyed_captures(
    model: &Model<'_, '_>,
    site: FunctionSite,
    key: Symbol,
    frame: &DirectFrame,
) -> Option<()> {
    let parent = model.enclosing_function(site.node_id)?;
    let AstKind::Function(parent_fn) = model.nodes.kind(parent.node_id) else {
        return None;
    };
    let statements = parent_fn.body.as_ref()?.statements.as_slice();
    let slots = [
        key,
        frame.observer,
        frame.bit_removal,
        frame.bit_error,
        frame.bit_aux,
        frame.old_source,
    ];
    let mut ready = 0;
    for (index, slot) in slots.into_iter().enumerate() {
        captured_local(model, site, slot)?;
        if !own_body_node(model, model.scoping.symbol_declaration(slot), parent) {
            return None;
        }
        let (init, end) = parent_initialization(model, parent, statements, slot)?;
        ready = ready.max(end);
        if index < 2 {
            // Stable key and observer origins must be actual parent inputs.
            if !parent_input_origin(model, parent, init, end, 0) {
                return None;
            }
            let initialized_in_declaration = matches!(model.nodes.kind(model.scoping.symbol_declaration(slot)),
                AstKind::VariableDeclarator(v) if v.init.is_some());
            if model.binding_writes(slot) != usize::from(!initialized_in_declaration) {
                return None;
            }
        } else if index < 5 {
            if !matches!(init.without_parentheses(), Expression::BooleanLiteral(_)) {
                return None;
            }
            // State may change in lifecycle children, but all writes must retain
            // Boolean values. Symbol identity excludes same-text child locals.
            for reference in model.scoping.get_resolved_references(slot) {
                if !reference.is_write() {
                    continue;
                }
                let AstKind::AssignmentExpression(write) =
                    model.nodes.parent_kind(reference.node_id())
                else {
                    return None;
                };
                if assigned_symbol(model, write) != Some(slot)
                    || !matches!(
                        write.right.without_parentheses(),
                        Expression::BooleanLiteral(_)
                    )
                {
                    return None;
                }
                if !site.span.contains_inclusive(write.span)
                    && write.span.start < end
                    && write.right.span() != init.span()
                {
                    return None;
                }
            }
        } else if !is_null(init) {
            return None;
        }
    }
    // Exact source-backed factory dataflow is already established by each
    // frame. Also require immutable, initialized callable bindings before setup.
    for factory in [frame.seed_factory, frame.writer_factory] {
        if model.binding_writes(factory) != 0 {
            return None;
        }
        let declaration = model.scoping.symbol_declaration(factory);
        match model.nodes.kind(declaration) {
            AstKind::Function(_) => (),
            AstKind::VariableDeclarator(v)
                if v.init.is_some() && v.span.end < parent.span.start =>
            {
                let setup_owner = model.owning_site(declaration)?;
                let binding = parent.binding_symbol?;
                if !model.scoping.get_resolved_references(binding).all(|r| {
                    r.is_read()
                        && exposure_after_setup(model, r.node_id(), setup_owner, v.span.end, 0)
                }) {
                    return None;
                }
            }
            _ => return None,
        }
    }
    let callback = site.binding_symbol?;
    let mut alias = None;
    for reference in model.scoping.get_resolved_references(callback) {
        if !reference.is_read() {
            return None;
        }
        if model.nodes.kind(reference.node_id()).span().start >= ready {
            if !exposure_after_setup(model, reference.node_id(), parent, ready, 0) {
                return None;
            }
            continue;
        }
        if !own_body_node(model, reference.node_id(), parent) {
            return None;
        }
        let (target, initialized) = match model.nodes.parent_kind(reference.node_id()) {
            AstKind::AssignmentExpression(write)
                if symbol(model, &write.right) == Some(callback)
                    && matches!(
                        model
                            .nodes
                            .parent_kind(model.nodes.parent_id(reference.node_id())),
                        AstKind::ExpressionStatement(_)
                    ) =>
            {
                (assigned_symbol(model, write)?, false)
            }
            AstKind::VariableDeclarator(v)
                if v.init.as_ref().and_then(|e| symbol(model, e)) == Some(callback) =>
            {
                (plain_local(model, v)?, true)
            }
            _ => return None,
        };
        if alias.replace(target).is_some() {
            return None;
        }
        captured_local(model, site, target)?;
        if !own_body_node(model, model.scoping.symbol_declaration(target), parent)
            || model.binding_writes(target) != usize::from(!initialized)
        {
            return None;
        }
        let AstKind::VariableDeclarator(v) =
            model.nodes.kind(model.scoping.symbol_declaration(target))
        else {
            return None;
        };
        if v.init.is_some() != initialized {
            return None;
        }
    }
    if let Some(alias) = alias {
        for reference in model.scoping.get_resolved_references(alias) {
            if reference.is_read()
                && !exposure_after_setup(model, reference.node_id(), parent, ready, 0)
            {
                return None;
            }
        }
    }
    Some(())
}

/// Only the closed direct/direct E0/C1/A2 schema. No other lane permutation or
/// staged/holder/lowered-next grammar is inferred from matching arm text.
fn keyed_deletion_state_frame(
    model: &Model<'_, '_>,
    site: FunctionSite,
    input: Symbol,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [Statement::TryStatement(try_)] = body.statements.as_slice() else {
        return None;
    };
    if try_.finalizer.is_some() {
        return None;
    }
    let [Statement::IfStatement(guard)] = try_.block.body.as_slice() else {
        return None;
    };
    if guard.alternate.is_some() {
        return None;
    }
    let key = storage_guard(model, &guard.test, input)?;
    let [removal, old, tail @ ..] = block(&guard.consequent)? else {
        return None;
    };
    let (removal, old) = removal_state(model, site, input, removal, old)?;
    let handler = try_.handler.as_ref()?;
    let BindingPattern::BindingIdentifier(caught) = &handler.param.as_ref()?.pattern else {
        return None;
    };
    if model.binding_writes(caught.symbol_id.get()?) != 0 {
        return None;
    }
    let [error, catch_tail @ ..] = handler.body.body.as_slice() else {
        return None;
    };
    let error = error_state(model, site, error)?;
    let success = keyed_partial_frame(model, site, tail)?;
    let catch = keyed_partial_frame(model, site, catch_tail)?;
    if !same_direct_frames(&success, &catch)
        || success.tag != 130.0
        || success.bit_removal != removal
        || success.bit_error != error
        || success.old_source != old
    {
        return None;
    }
    let state = [
        key,
        old,
        removal,
        error,
        success.bit_aux,
        success.observer,
        success.seed_factory,
        success.writer_factory,
    ];
    if !distinct(&state) {
        return None;
    }
    initialized_keyed_captures(model, site, key, &success)
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
    let [parameter] = function.params.items.as_slice() else {
        return Vec::new();
    };
    if function.params.rest.is_some() || parameter.initializer.is_some() {
        return Vec::new();
    }
    let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
        return Vec::new();
    };
    let Some(input) = identifier.symbol_id.get() else {
        return Vec::new();
    };
    if model.binding_writes(input) != 0 || !model.scoping.symbol_redeclarations(input).is_empty() {
        return Vec::new();
    }
    use super::storage_removal_emitter_variants::Route;
    let route = if storage_removal(model, site, input, body).is_some() {
        Route::Existing
    } else {
        let Some(route) =
            super::storage_removal_emitter_variants::storage_removal(model, site, input, body)
        else {
            return if keyed_deletion_state_frame(model, site, input, body).is_some() {
                vec![proposal("emitKeyedDeletionStateFrame", "thread_keyed_deletion_state_frame",
                    "Own keyed-deletion state frame; external/native Storage identity and lifecycle purpose explicitly unresolved. \
                     Same scoped parent-input key and observer, initialized Boolean state before callback exposure, and live \
                     immediately-owned stream/writer factories. Both arms preserve their salt and exact big-endian nonce/header; \
                     direct writer/cursor at offset 3, null tag 130 at slot 4, value before flags. Removal at bit 0 and error at bit 1 \
                     are snapshotted before value serialization; auxiliary at bit 2 is read afterward with original coercions. \
                     Catch sets error and can observe earlier previous state if the event guard/property access threw before assignment; \
                     next([[], header]) retains its receiver. No byte equivalence with other lane permutations is asserted.", f)]
            } else {
                Vec::new()
            };
        };
        route
    };
    if matches!(route, Route::FixedPort) {
        return vec![proposal(NAME, EVALUATOR, "whole keyed-removal try/catch frame with identical source-proven live factories, salt, tag and captures in both arms; all ordered removal/error/auxiliary flag lanes are emitted at fixed header slot 4 before nullable old-value serialization at cursor 4 or null tag at slot 5; exact four-formal assignment-and-return IIFE constructs [[], header], passed by the captured next method.call on its original receiver; no completion or additional effects; native storage identity explicitly unproved", f)];
    }
    if matches!(route, Route::Extended) {
        return vec![proposal(NAME, EVALUATOR, "whole keyed-removal try/catch frame; each exact own arm draws and reseeds a big-endian random header through live source-proven stream/writer factories; removal and auxiliary flags are snapshotted before nullable old-value serialization, error is read afterward; exact holder cursor, null tag, lane order and same observer next emission; external native identity explicitly unproved", f)];
    }
    if matches!(route, Route::AliasedDirect) {
        return vec![proposal(
            NAME,
            EVALUATOR,
            "storage removal callback with two exact aliased direct frames: \
             key-cleared or matching-key/null-newValue guard sets removal and \
             oldValue ?? null; each frame packs a Math.random()*2^32 nonce \
             big-endian, resolves its own seed and writer factories, snapshots \
             immutable removal/old-value aliases, and writes nullable old value \
             before flags using the exact emitted array and writer/cursor. \
             Ordered lanes are removal at bit 0, error at bit 1, and auxiliary \
             at bit 2; catch sets that error slot and re-emits identical ordered \
             lanes, seed/writer origins, reseed literal, null tag, old-value \
             source, and observer receiver with next([[], header]). \
             No unaliased direct lane permutation is inferred from two-arm equality",
            f,
        )];
    }
    vec![proposal(
        NAME,
        EVALUATOR,
        "storage `addEventListener` callback: one plain StorageEvent-like formal \
         is tested for key-cleared or keyed-removal (`key === null` or \
         `key === capturedKey && newValue === null`), the removal flag is set and \
         `oldValue ?? null` is read, then one Math.random()*2^32 nonce is packed \
         big-endian into a four-byte header, one `seed ^ number` reseed derives \
         the stream and one resolved writer factory derives the writer; the \
         null-old branch writes the literal tag byte `stream() ^ <tag>` at slot 4 \
         and the other branch re-invokes the writer through the three-element \
         `[writer, old, header]` holder with cursor 3, the same header/cursor \
         then receive `flag|0|error<<1|second<<2 ^ stream()`, and the same \
         captured observer receives `next([[], header])` with no `complete()`; \
         the catch block sets the same captured error flag and re-emits the exact \
         same state/header/factory frame.  The bounded direct variant accepts the \
         same family when the writer binding is also the cursor and the writer is \
         re-invoked directly as `writer(old, header, 3)` with the lanes \
         `aux|0|removal<<1|error<<2`; a second fixed-slot variant accepts the same \
         family when the shifted lanes are appended at the literal slot 4 before \
         the `old == null` test, the null branch writes the tag at slot 5 and the \
         other branch invokes the writer with the literal cursor 4; a third \
         late-flags holder variant accepts the same family when a separate cursor \
         and the `[writer, old, header]` holder are used and the holder \
         `flag|0|error<<1|second<<2` lanes are appended at `header[cursor + 1]` \
         after the branch, reading the captured removal flag at the low lane.  \
         An additional exact fallback proves fixed-slot holder calls and the \
         two mixed holder/direct frame pairs with identical ordered flag lanes, \
         including an immutable captured low-lane alias. \
         Every captured slot, the old-value alias, the header, the reseed \
         literal, the tag, the observer and both source-backed factories must \
         still agree across the two frames of whichever variant matched",
        f,
    )]
}
