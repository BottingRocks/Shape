//! Bounded owned-body proof for the curried seeded string-writer / clock
//! emitter variant.
//!
//! The family is the one-parameter callback that prepares a bounded string
//! payload, draws one 32-bit `Math.random` word, builds the four-byte
//! big-endian header, reseeds an independently proved keystream generator with
//! `seed ^ constant`, then drives the independently proved curried interned
//! string writer:
//!
//! ```text
//! function $THREAD_6_48236($CK) {
//!   let $KEY_2 = $KEY_3.slice(0, 50);                 // bounded payload
//!   let $KEY_5 = Math.random() * 4294967296 | 0;      // 32-bit draw
//!   let $CL = [$KEY_5 >>> 24, $KEY_5 >> 16 & 255,
//!              $KEY_5 >> 8 & 255, $KEY_5 & 255];      // big-endian header
//!   let $KEY_11 = $KEY_10($KEY_5 ^ 581381427);        // seedable keystream
//!   $KEY_2 = $KEY_78($KEY_11)(
//!       $KEY_2, $CL, $KEY_174($KEY_11, $CK, $CL, 3));
//!   $KEY_174($KEY_11, $KEY_8.now() - $KEY_6, $CL, $KEY_2);
//!   $KEY_9.next([[], $CL]);
//!   $KEY_9.complete();
//! }
//! ```
//!
//! The proof is a direct, ordered dataflow check over the callback's own body
//! plus its captured lexical participants:
//!
//! * exactly one plain formal, no directives, not async or a generator, no
//!   nested function or arrow owned by the site, and exactly eight statements;
//! * the first local is a `capturedSource.slice(0, 50)` payload; that same
//!   symbol is the writer's first argument, is overwritten exactly once by the
//!   writer's returned cursor, and is then the numeric writer's cursor;
//! * the second local is exactly `Math.random() * 4294967296 | 0`, and the
//!   third local is the exact four-lane `>>> 24 / >> 16 & 255 / >> 8 & 255 /
//!   & 255` big-endian header over that same seed symbol;
//! * the fourth local is a captured, single-origin factory applied to
//!   `seed ^ <numeric constant>`; the factory's single top-level return is the
//!   independently proved `nextKeystreamByte` (the existing p05 helper role),
//!   lexically owned by that factory, and the factory's plain formal is
//!   connected to that helper's own state operation (a direct live use or one
//!   bounded factory-local seed hop that initializes closure state read into a
//!   lane-producing return), so the emitted byte source is tied by identity to
//!   the known keystream helper rather than to a name and the seed argument
//!   cannot be ignored by an empty control-flow test, a discarded call, a dead
//!   local, a nested dead function or a dead alias;
//! * the same captured keystream is the inner argument of the curried factory
//!   `capturedFactory(keystream)`, whose single top-level return is the
//!   independently proved, lexically owned `writeInternedString` whose plain
//!   formal or one factory-local hop is the callable seed source it invokes
//!   (`seed()`) into its byte-writing element store; the immediately returned writer is
//!   called in place with `(payload, header, cursor)` where the cursor is the
//!   four-argument result of the source-proven number writer over the same
//!   keystream, formal slot 0, header and literal `3`;
//! * the following statement calls that same captured number writer with the
//!   same keystream and header, the captured
//!   `clock.now() - capturedInit` elapsed value and the returned payload
//!   cursor;
//! * the tail is `capturedObserver.next([[], header])` followed by
//!   `capturedObserver.complete()` on the same captured observer.
//!
//! Every identifier comparison is a scoped `SymbolId` comparison. `Math` is
//! only accepted as an unshadowed ambient path, and the two factory identities
//! are tied through the already-proved `nextKeystreamByte` / `writeInternedString`
//! helper roles. A captured factory binding that the shared stable-callable
//! gate refuses solely because it has one later write is accepted only when the
//! initializer is still the one stable function, that initializer is a direct
//! `VariableDeclaration` statement of the same function body before the call
//! edge, and the write is a plain assignment in a sibling direct statement that
//! cannot run on any path to the call edge: the reference must be in the
//! consequent of an `if` whose last direct statement is an unconditional
//! `return`, and the write must be a later direct statement of the same
//! function body. The declaration must be owned by that same common function,
//! so an outer/program-scope binding shared across invocations is not accepted
//! on per-invocation branch exclusivity alone, and the initializer must
//! dominate the branch as an unconditional direct statement rather than merely
//! precede it in bytes. The real `$KEY_10` shape is
//! exactly this: the factory alias is initialised once, and the recovered base36
//! decoder tail writes `$KEY_10 = $KEY_25` only in the alternate branch, after a
//! branch that returns when the emitter's factory branch is taken. The write is
//! therefore not a second callable origin, and no execution reaches both the
//! call edge and the write. A write before the reference, a write in the same
//! branch, a write in a nested function, a branch without a terminal return, a
//! declaration owned by another function, a conditional `var` initializer or a
//! loop-owned declaration, or any additional write is refused; the shared
//! `stable_function` gate itself is unchanged. The numeric writer is not
//! accepted by name or arity: the
//! resolved callee site is re-proved synchronously by the accepted shared
//! number-writer proof (`numeric_writer::evaluate`) and must produce
//! that proof's `writeNumber` claim. That direct identity check requires the
//! accepted numeric-mixed-indices 003 source (`numeric_writer.rs`
//! reviewed sha256 `60b99f26...`), whose compiled artifact (`df5950c6...`)
//! carries the declarator-step glue that extends the number-writer proof to the
//! holder form used by the real 3851-byte `fn@17750..21601` fixture; the
//! compiled artifact must be kept as-is and never replaced by the raw reviewed
//! source. The factory formal's feed is a bounded, role-directed state
//! connection, not a syntactic consumer: bare control-flow tests, conditional
//! tests, throws, discarded call arguments, unread dead locals,
//! nested-function references, dead one-hop aliases and a seed overwritten by
//! an unrelated capture before the meaningful read are all refused. No source
//! name, generated spelling, span, hash, vendor, label search or generic
//! cross-function dataflow framework is used: the full-node passes are
//! mechanical scans of the factory's own one-hop writes and of references owned
//! by the single already-resolved helper. This is a bounded check of the
//! callback and its directly named helpers, and it runs only after every
//! earlier stage has declined the function.

use super::*;
use oxc_semantic::SymbolId;
use oxc_span::{GetSpan, Span};

type Symbol = SymbolId;

const NAME: &str = "emitSeededHashEncoding";
const EVALUATOR: &str = "thread_posthost_curried_seeded_emitter";
const DETAIL: &str = "exact 32-bit Math.random word, four-byte big-endian header over that seed, seed^constant keystream from a factory-owned helper whose plain formal (or one factory-local hop) is state-directed into that helper's byte/state operation (closure state read into a lane-producing return for nextKeystreamByte, or the invoked callable seed source of the written byte for writeInternedString; empty tests, throws, conditional tests, discarded call arguments, dead locals, nested functions and dead aliases are refused, and an overwritten seed is refused), curried factory-owned interned string writer over the bounded capture.slice(0, 50) payload and the same source-proven writeNumber identity at both numeric call sites (writeNumber(hash, formal, header, 3) then writeNumber(hash, clock.now()-init, header, payload)), followed by next([[], header]) and complete() on one captured observer; a captured factory binding with one later write is accepted only when its initializer is a direct earlier VariableDeclaration statement of the same function body, and that write is a plain assignment in a sibling branch of that body whose consequent returns before the write (for example a base36 decoder tail), so the binding is freshly initialized in the invocation that claims it and no execution reaches both the call edge and the write; an outer/shared, conditional or loop-owned declaration is refused";

fn symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(expression.without_parentheses(), model.scoping)
}

fn call<'a>(expression: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    match expression.without_parentheses() {
        Expression::CallExpression(call) if !call.optional => Some(call),
        _ => None,
    }
}

/// Exact argument list. A spread or other non-expression argument changes the
/// list length and refuses the shape.
fn arguments<'a>(call: &'a CallExpression<'a>, count: usize) -> Option<Vec<&'a Expression<'a>>> {
    let values = call_arguments(call);
    (call.arguments.len() == count && values.len() == count).then_some(values)
}

fn member<'a>(expression: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    match expression.without_parentheses() {
        Expression::StaticMemberExpression(member)
            if !member.optional && member.property.name == key =>
        {
            Some(&member.object)
        }
        _ => None,
    }
}

fn numeric(expression: &Expression<'_>) -> Option<f64> {
    match expression.without_parentheses() {
        Expression::NumericLiteral(literal) => Some(literal.value),
        _ => None,
    }
}

fn number(expression: &Expression<'_>, expected: f64) -> bool {
    numeric(expression) == Some(expected)
}

fn empty_array(expression: &Expression<'_>) -> bool {
    matches!(expression.without_parentheses(), Expression::ArrayExpression(array) if array.elements.is_empty())
}

fn expression<'a>(statement: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    match statement {
        Statement::ExpressionStatement(statement) => Some(&statement.expression),
        _ => None,
    }
}

fn statement_call<'a>(statement: &'a Statement<'a>) -> Option<&'a CallExpression<'a>> {
    call(expression(statement)?)
}

fn assignment<'a>(statement: &'a Statement<'a>) -> Option<&'a AssignmentExpression<'a>> {
    match expression(statement)? {
        Expression::AssignmentExpression(assignment) if assignment.operator.as_str() == "=" => {
            Some(assignment)
        }
        _ => None,
    }
}

fn assign_symbol(model: &Model<'_, '_>, assignment: &AssignmentExpression<'_>) -> Option<Symbol> {
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

fn plain_local(model: &Model<'_, '_>, variable: &VariableDeclarator<'_>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(identifier) = &variable.id else {
        return None;
    };
    let value = identifier.symbol_id.get()?;
    model
        .scoping
        .symbol_redeclarations(value)
        .is_empty()
        .then_some(value)
}

fn single_return<'a, 'b>(
    model: &Model<'_, '_>,
    factory: FunctionSite,
    body: &'a FunctionBody<'b>,
) -> Option<&'a Expression<'b>> {
    let Statement::ReturnStatement(returned) = body.statements.last()? else {
        return None;
    };
    // Only this factory's exits count. Its child has its own byte/cursor
    // returns, and its enclosing caller may have branch-exclusive rebinding.
    for node in model.nodes.iter() {
        if model.owning_site(node.id()).map(|owner| owner.node_id) != Some(factory.node_id) {
            continue;
        }
        match node.kind() {
            AstKind::ReturnStatement(other) if other.span != returned.span => return None,
            AstKind::ThrowStatement(_) => return None,
            _ => {}
        }
    }
    returned.argument.as_ref()
}

fn directives(body: &FunctionBody<'_>) -> bool {
    body.directives.is_empty()
        || (body.directives.len() == 1 && body.directives[0].directive == "use strict")
}

fn unchanged(model: &Model<'_, '_>, value: Symbol) -> bool {
    model.binding_writes(value) == 0 && model.scoping.symbol_redeclarations(value).is_empty()
}

fn plain_params(model: &Model<'_, '_>, params: &FormalParameters<'_>) -> bool {
    params.rest.is_none()
        && params.items.iter().all(|parameter| {
            parameter.initializer.is_none()
                && matches!(&parameter.pattern, BindingPattern::BindingIdentifier(identifier)
                    if identifier.symbol_id.get().is_some_and(|value| unchanged(model, value)))
        })
}

fn one_plain_formal(model: &Model<'_, '_>, params: &FormalParameters<'_>) -> bool {
    params.rest.is_none()
        && params.items.len() == 1
        && params.items[0].initializer.is_none()
        && matches!(&params.items[0].pattern, BindingPattern::BindingIdentifier(identifier)
            if identifier.symbol_id.get().is_some_and(|value| unchanged(model, value)))
}

/// A captured data binding: declared before the use, outside this function's
/// own body, never a formal and never a redeclaration.
fn captured_data(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
) -> Option<Symbol> {
    let value = symbol(model, expression)?;
    let span = model.scoping.symbol_span(value);
    (!site.span.contains_inclusive(span)
        && span.end <= expression.span().start
        && model.formal_symbol_slot(value).is_none()
        && model.scoping.symbol_redeclarations(value).is_empty()
        && matches!(
            model.nodes.kind(model.scoping.symbol_declaration(value)),
            AstKind::VariableDeclarator(_)
        ))
    .then_some(value)
}

/// The identifier-reference node behind a callee expression.
fn reference_node(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
) -> Option<oxc_semantic::NodeId> {
    let Expression::Identifier(identifier) = expression.without_parentheses() else {
        return None;
    };
    Some(
        model
            .scoping
            .get_reference(identifier.reference_id.get()?)
            .node_id(),
    )
}

/// The index of the direct statement of `statements` whose span covers `span`.
fn statement_index(statements: &[Statement<'_>], span: Span) -> Option<usize> {
    statements.iter().position(|statement| {
        let outer = statement.span();
        outer.start <= span.start && span.end <= outer.end
    })
}

/// True when the sole write of the captured callable binding cannot run on any
/// path that reaches the call reference **in the invocation that creates the
/// captured value**. The declaration, the reference and the write must all be
/// owned by one common function body. The reference must sit in the consequent
/// of an `if` whose last direct statement is an unconditional `return`, and the
/// write must be a later direct statement of that same body: entering the
/// consequent returns before the write, while not entering it never reaches the
/// reference. The write must be a plain `=` assignment, so it has no hidden read
/// of the previous callable value.
///
/// Byte order is not dominance. The initializer must be freshly bound in that
/// same invocation: the declaration must be owned by the common function and its
/// covering direct statement must be the earlier `VariableDeclaration` itself,
/// not an enclosing `if`/loop/switch/try and not an outer or program scope
/// shared across invocations. Otherwise `run(false)` could mutate the shared
/// binding before `run(true)` creates the emitter.
///
/// This is a lexical control-flow check over the already-resolved binding; it
/// does not relax the shared stable-callable gate. A write before the
/// reference, a write in the reference's own branch, a write in a nested
/// function, a branch without a terminal return, a declaration owned by another
/// function and a conditional/loop-owned initializer are refused.
fn exclusive_write(
    model: &Model<'_, '_>,
    declaration: oxc_semantic::NodeId,
    reference: oxc_semantic::NodeId,
    write: oxc_semantic::NodeId,
) -> bool {
    let write_parent = model.nodes.parent_id(write);
    if !matches!(
        model.nodes.kind(write_parent),
        AstKind::AssignmentExpression(assignment) if assignment.operator.as_str() == "="
    ) {
        return false;
    }
    let reference_span = model.nodes.kind(reference).span();
    let write_span = model.nodes.kind(write).span();
    let mut current = reference;
    for _ in 0..512 {
        let parent = model.nodes.parent_id(current);
        if parent == current {
            return false;
        }
        current = parent;
        let body = match model.nodes.kind(current) {
            AstKind::Function(function) => match function.body.as_deref() {
                Some(body) => body,
                None => continue,
            },
            AstKind::ArrowFunctionExpression(arrow) => arrow.body.as_ref(),
            _ => continue,
        };
        if write_span.start < body.span.start || write_span.end > body.span.end {
            continue;
        }
        // The write must belong to this function body itself, not to an
        // escaping nested function or callback whose deferred invocation could
        // run it after this body returns.
        if model.owning_site(write).map(|owner| owner.node_id) != Some(current) {
            return false;
        }
        let Some(reference_index) = statement_index(&body.statements, reference_span) else {
            return false;
        };
        let Some(write_index) = statement_index(&body.statements, write_span) else {
            return false;
        };
        if write_index <= reference_index {
            return false;
        }
        // The captured value must be freshly bound in this same invocation. The
        // declaration must be owned by this common function, and its covering
        // direct statement must be the earlier VariableDeclaration itself. A
        // declaration in an enclosing/program scope shared across invocations,
        // under an `if`, or inside a loop is byte-earlier only and may run zero
        // times or be mutated between invocations; it is refused.
        if model.owning_site(declaration).map(|owner| owner.node_id) != Some(current) {
            return false;
        }
        let declaration_span = model.nodes.kind(declaration).span();
        let Some(declaration_index) = statement_index(&body.statements, declaration_span) else {
            return false;
        };
        if declaration_index >= reference_index
            || !matches!(
                &body.statements[declaration_index],
                Statement::VariableDeclaration(_)
            )
        {
            return false;
        }
        let Statement::IfStatement(branch) = &body.statements[reference_index] else {
            return false;
        };
        let consequent = branch.consequent.span();
        if reference_span.start < consequent.start || reference_span.end > consequent.end {
            return false;
        }
        let Statement::BlockStatement(block) = &branch.consequent else {
            return false;
        };
        return matches!(block.body.last(), Some(Statement::ReturnStatement(_)));
    }
    false
}

/// A captured single-origin callable resolved through the shared stable-callable
/// gate. The primary path is unchanged: a zero-write function declaration or a
/// one-origin variable initializer. A second, deliberately narrow path accepts
/// a captured callable whose initializer is the stable function and whose one
/// later write is provably control-flow disjoint from this call edge (see
/// [`exclusive_write`]): the real `$KEY_10` factory is initialised once,
/// but the recovered base36 decoder tail assigns a number to the same outer
/// binding in a sibling branch that returns before that assignment whenever the
/// emitter's branch runs. The declaration must still be a direct earlier
/// `VariableDeclaration` statement of the same function body that owns the
/// branch and the write, so an outer/shared binding, a conditional `var`
/// initializer and a loop-owned declaration are refused. A genuine second
/// origin or any ambiguous write is refused, so the stable-function guarantee
/// is preserved. Only the directly named helper is inspected; no returned value
/// is chased beyond one factory return.
fn captured_callable(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
) -> Option<FunctionSite> {
    if let Some(target) = super::stable_function(model, expression, 0) {
        return (!site.span.contains_inclusive(target.span)
            && target.span.end <= expression.span().start)
            .then_some(target);
    }

    // Control-flow-disjoint sole-write path. The shared gate already refused
    // the binding because it has one write and an initializer; accept it only
    // when the write and this call edge are in exclusive branches of the same
    // function body.
    let value = symbol(model, expression)?;
    if !model.scoping.symbol_redeclarations(value).is_empty() {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(value);
    let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&variable.id, BindingPattern::BindingIdentifier(identifier)
        if identifier.symbol_id.get() == Some(value))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(_)
        )
        || matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
    {
        return None;
    }
    let init = variable.init.as_ref()?;
    let target = super::stable_function(model, init, 0)?;
    let declaration_span = model.scoping.symbol_span(value);
    if site.span.contains_inclusive(target.span)
        || target.span.end > expression.span().start
        || declaration_span.end > expression.span().start
        || model.binding_writes(value) != 1
    {
        return None;
    }
    let reference = reference_node(model, expression)?;
    let write = model
        .scoping
        .get_resolved_references(value)
        .find(|reference| reference.is_write())?
        .node_id();
    exclusive_write(model, declaration, reference, write).then_some(target)
}

/// One factory-owned store to a binding: a declarator initializer or a plain
/// `=` assignment. `value` is the assigned expression's symbol, when it is a
/// bare identifier.
struct FactoryWrite {
    start: u32,
    target: Symbol,
    value: Option<Symbol>,
}

/// True when `symbol` is declared outside the returned helper, so the helper
/// reads it as captured closure state.
fn helper_closure_binding(model: &Model<'_, '_>, returned: FunctionSite, symbol: Symbol) -> bool {
    !returned
        .span
        .contains_inclusive(model.scoping.symbol_span(symbol))
}

/// True when the returned expression reads at least one closure binding of the
/// helper. This is the helper's byte/state operation: a lane return reads its
/// state, whereas `void 0` reads nothing.
fn expression_reads_helper_state(
    model: &Model<'_, '_>,
    returned: FunctionSite,
    expression: &Expression<'_>,
) -> bool {
    let span = expression.span();
    model.nodes.iter().any(|node| {
        let AstKind::IdentifierReference(identifier) = node.kind() else {
            return false;
        };
        let reference_span = node.kind().span();
        span.start <= reference_span.start
            && reference_span.end <= span.end
            && model.owning_site(node.id()).map(|owner| owner.span.start)
                == Some(returned.span.start)
            && crate::analysis::identifier_symbol(identifier, model.scoping)
                .is_some_and(|symbol| helper_closure_binding(model, returned, symbol))
    })
}

/// Walks one own reference outward. The value only counts when it reaches a
/// lane-producing return of the returned helper through stores and transparent
/// expressions. A control-flow test, switch case, throw, conditional test, call
/// argument, discarded call or nested function stops the flow: none of those is
/// a state operation of the helper.
fn state_reference_reaches_lane(
    model: &Model<'_, '_>,
    returned: FunctionSite,
    reference: oxc_semantic::NodeId,
    visiting: &mut Vec<Symbol>,
) -> bool {
    let mut current = reference;
    for _ in 0..512 {
        let parent = model.nodes.parent_id(current);
        if parent == current {
            return false;
        }
        let current_span = model.nodes.kind(current).span();
        match model.nodes.kind(parent) {
            // The helper evaluates the expression for its side effect only.
            AstKind::ExpressionStatement(_) => return false,
            // A lane return reads helper state; a bare `return;` or `void 0`
            // fallback does not.
            AstKind::ReturnStatement(statement) => {
                return statement.argument.as_ref().is_some_and(|argument| {
                    expression_reads_helper_state(model, returned, argument)
                });
            }
            // A store is live only when the stored binding is itself read by a
            // lane return.
            AstKind::VariableDeclarator(variable) => {
                if variable.init.as_ref().map(|init| init.span()) != Some(current_span) {
                    return false;
                }
                let BindingPattern::BindingIdentifier(identifier) = &variable.id else {
                    return false;
                };
                return identifier.symbol_id.get().is_some_and(|value| {
                    state_symbol_reaches_lane(model, returned, value, visiting)
                });
            }
            AstKind::AssignmentExpression(assignment) => {
                if assignment.operator.as_str() != "=" || assignment.right.span() != current_span {
                    return false;
                }
                let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left
                else {
                    return false;
                };
                return crate::analysis::identifier_symbol(identifier, model.scoping).is_some_and(
                    |value| state_symbol_reaches_lane(model, returned, value, visiting),
                );
            }
            // A call argument, receiver, control-flow test, switch case, throw,
            // conditional test or nested function body is not a state operation,
            // even when it syntactically mentions the value.
            AstKind::CallExpression(_)
            | AstKind::NewExpression(_)
            | AstKind::TaggedTemplateExpression(_)
            | AstKind::IfStatement(_)
            | AstKind::WhileStatement(_)
            | AstKind::DoWhileStatement(_)
            | AstKind::ForStatement(_)
            | AstKind::ForInStatement(_)
            | AstKind::ForOfStatement(_)
            | AstKind::SwitchStatement(_)
            | AstKind::SwitchCase(_)
            | AstKind::ThrowStatement(_)
            | AstKind::Function(_)
            | AstKind::ArrowFunctionExpression(_) => return false,
            // The test is not a state operation; the branches keep flowing.
            AstKind::ConditionalExpression(conditional) => {
                if conditional.test.span() == current_span {
                    return false;
                }
                current = parent;
            }
            // Only the last comma operand is the sequence value.
            AstKind::SequenceExpression(sequence) => {
                if sequence
                    .expressions
                    .last()
                    .is_some_and(|last| last.span() == current_span)
                {
                    current = parent;
                } else {
                    return false;
                }
            }
            // Transparent value-flow expressions: keep walking to the consumer.
            AstKind::ParenthesizedExpression(_)
            | AstKind::UnaryExpression(_)
            | AstKind::UpdateExpression(_)
            | AstKind::BinaryExpression(_)
            | AstKind::LogicalExpression(_)
            | AstKind::StaticMemberExpression(_)
            | AstKind::ComputedMemberExpression(_)
            | AstKind::PrivateFieldExpression(_)
            | AstKind::AwaitExpression(_)
            | AstKind::SpreadElement(_)
            | AstKind::TemplateLiteral(_)
            | AstKind::ArrayExpression(_)
            | AstKind::ObjectExpression(_)
            | AstKind::ObjectProperty(_)
            | AstKind::TSAsExpression(_)
            | AstKind::TSSatisfiesExpression(_)
            | AstKind::TSNonNullExpression(_) => {
                current = parent;
            }
            _ => return false,
        }
    }
    false
}

/// True when `symbol` has a reachable own state read inside the returned helper
/// whose value reaches a lane-producing return. Only the helper's own nodes are
/// followed, so a nested function never qualifies.
fn state_symbol_reaches_lane(
    model: &Model<'_, '_>,
    returned: FunctionSite,
    symbol: Symbol,
    visiting: &mut Vec<Symbol>,
) -> bool {
    if visiting.len() >= 64 || visiting.contains(&symbol) {
        return false;
    }
    visiting.push(symbol);
    let mut live = false;
    for node in model.nodes.iter() {
        let AstKind::IdentifierReference(identifier) = node.kind() else {
            continue;
        };
        if crate::analysis::identifier_symbol(identifier, model.scoping) != Some(symbol) {
            continue;
        }
        if model.owning_site(node.id()).map(|owner| owner.span.start) != Some(returned.span.start) {
            continue;
        }
        if state_reference_reaches_lane(model, returned, node.id(), visiting) {
            live = true;
            break;
        }
    }
    visiting.pop();
    live
}

/// The factory-owned stores in the factory's own body, outside the returned
/// helper. A write in a nested function or inside the helper is not factory
/// state initialization.
fn factory_writes(
    model: &Model<'_, '_>,
    factory: FunctionSite,
    returned: FunctionSite,
) -> Vec<FactoryWrite> {
    let factory_body = factory.body_span.unwrap_or(factory.span);
    let mut writes = Vec::new();
    for node in model.nodes.iter() {
        let span = node.kind().span();
        if span.start < factory_body.start
            || span.end > factory_body.end
            || returned.span.contains_inclusive(span)
        {
            continue;
        }
        if model.owning_site(node.id()).map(|owner| owner.span.start) != Some(factory.span.start) {
            continue;
        }
        match node.kind() {
            AstKind::VariableDeclarator(variable) => {
                let BindingPattern::BindingIdentifier(identifier) = &variable.id else {
                    continue;
                };
                let Some(target) = identifier.symbol_id.get() else {
                    continue;
                };
                writes.push(FactoryWrite {
                    start: span.start,
                    target,
                    value: variable.init.as_ref().and_then(|init| symbol(model, init)),
                });
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
                writes.push(FactoryWrite {
                    start: span.start,
                    target,
                    value: symbol(model, &assignment.right),
                });
            }
            _ => {}
        }
    }
    writes
}

/// The formal plus every one-hop factory binding seeded from it. These are the
/// values that may carry the seed into the helper's state.
fn seed_sources(writes: &[FactoryWrite], formal: Symbol) -> Vec<Symbol> {
    let mut seeds = vec![formal];
    for write in writes {
        if write.value == Some(formal) && !seeds.contains(&write.target) {
            seeds.push(write.target);
        }
    }
    seeds
}

/// The seed value must still be the one the helper reads: the seed binding's
/// last factory-owned write must itself come from a seed source. Otherwise an
/// unrelated capture overwrote the seed before the meaningful read.
fn seed_is_current(
    writes: &[FactoryWrite],
    seeds: &[Symbol],
    formal: Symbol,
    seed: Symbol,
) -> bool {
    if seed == formal {
        return true;
    }
    writes
        .iter()
        .filter(|write| write.target == seed)
        .max_by_key(|write| write.start)
        .is_some_and(|write| write.value.is_some_and(|value| seeds.contains(&value)))
}

/// The keystream factory's plain formal must feed state the returned
/// `nextKeystreamByte` helper actually reads into a lane-producing return.
fn keystream_formal_feeds_state(
    model: &Model<'_, '_>,
    factory: FunctionSite,
    formal: Symbol,
    returned: FunctionSite,
) -> bool {
    let writes = factory_writes(model, factory, returned);
    let seeds = seed_sources(&writes, formal);
    seeds.iter().any(|&seed| {
        helper_closure_binding(model, returned, seed)
            && seed_is_current(&writes, &seeds, formal, seed)
            && state_symbol_reaches_lane(model, returned, seed, &mut Vec::new())
    })
}

/// Walks an invocation result outward through transparent expressions and
/// accepts only a store into an indexed/member output buffer.
fn invocation_result_feeds_byte_write(model: &Model<'_, '_>, call: oxc_semantic::NodeId) -> bool {
    let mut current = call;
    for _ in 0..256 {
        let parent = model.nodes.parent_id(current);
        if parent == current {
            return false;
        }
        let current_span = model.nodes.kind(current).span();
        match model.nodes.kind(parent) {
            AstKind::AssignmentExpression(assignment) => {
                if assignment.operator.as_str() != "=" || assignment.right.span() != current_span {
                    return false;
                }
                return matches!(
                    &assignment.left,
                    AssignmentTarget::ComputedMemberExpression(_)
                        | AssignmentTarget::StaticMemberExpression(_)
                );
            }
            AstKind::ParenthesizedExpression(_)
            | AstKind::UnaryExpression(_)
            | AstKind::BinaryExpression(_)
            | AstKind::LogicalExpression(_) => {
                current = parent;
            }
            AstKind::SequenceExpression(sequence) => {
                if sequence
                    .expressions
                    .last()
                    .is_some_and(|last| last.span() == current_span)
                {
                    current = parent;
                } else {
                    return false;
                }
            }
            AstKind::ConditionalExpression(conditional) => {
                if conditional.test.span() == current_span {
                    return false;
                }
                current = parent;
            }
            // A call result passed to another call, returned, discarded or
            // stored in a local is not the writer's byte-writing operation.
            _ => return false,
        }
    }
    false
}

/// True when an own `callee()` invocation of `symbol` inside the returned
/// helper feeds a byte-writing store (an element/member assignment). This is the
/// interned-string writer's byte source: the seeded callable is invoked, and its
/// result is assigned or XORed into the output buffer.
fn writer_invocation_feeds_byte(
    model: &Model<'_, '_>,
    returned: FunctionSite,
    symbol: Symbol,
) -> bool {
    for node in model.nodes.iter() {
        let AstKind::IdentifierReference(identifier) = node.kind() else {
            continue;
        };
        if crate::analysis::identifier_symbol(identifier, model.scoping) != Some(symbol) {
            continue;
        }
        if model.owning_site(node.id()).map(|owner| owner.span.start) != Some(returned.span.start) {
            continue;
        }
        let parent = model.nodes.parent_id(node.id());
        let AstKind::CallExpression(call) = model.nodes.kind(parent) else {
            continue;
        };
        if call.callee.span() != node.kind().span() {
            continue;
        }
        if invocation_result_feeds_byte_write(model, parent) {
            return true;
        }
    }
    false
}

/// The interned-string-writer factory's plain formal must be the callable seed
/// source the returned `writeInternedString` helper invokes to produce bytes.
fn writer_formal_feeds_byte(
    model: &Model<'_, '_>,
    factory: FunctionSite,
    formal: Symbol,
    returned: FunctionSite,
) -> bool {
    let writes = factory_writes(model, factory, returned);
    let seeds = seed_sources(&writes, formal);
    seeds.iter().any(|&seed| {
        helper_closure_binding(model, returned, seed)
            && seed_is_current(&writes, &seeds, formal, seed)
            && writer_invocation_feeds_byte(model, returned, seed)
    })
}

/// Proves that a factory's plain formal genuinely feeds the returned helper's
/// own byte/state operation. The accepted p05 helper roles only describe the
/// returned helper; they do not prove that the factory argument was used. This
/// check is deliberately role-directed and bounded:
///
/// * `nextKeystreamByte`: the formal or one factory-local hop must initialize
///   closure state that the helper itself reads into a lane-producing return;
/// * `writeInternedString`: the formal or one hop must be the callable the
///   helper invokes as the source of its written byte value.
///
/// A bare `if (seed) {}`, a discarded `f(seed)`, a conditional test, a throw, an
/// unread local, a dead alias, a nested function body and an unrelated call
/// argument cannot satisfy either form, and a seed overwritten by an unrelated
/// capture before the helper reads it is refused.
fn factory_formal_feeds_helper(
    model: &Model<'_, '_>,
    factory: FunctionSite,
    formal: Symbol,
    returned: FunctionSite,
    expected: &str,
) -> bool {
    match expected {
        "nextKeystreamByte" => keystream_formal_feeds_state(model, factory, formal, returned),
        "writeInternedString" => writer_formal_feeds_byte(model, factory, formal, returned),
        _ => false,
    }
}

/// Ties a captured factory's single top-level returned function to an already
/// proved helper role. This is deliberately one bounded return hop, not a
/// generic callable-result dataflow analysis. The returned helper must be
/// lexically owned by the captured factory (not a donated global or sibling
/// helper) and the factory's formal must feed its state.
pub(in crate::naming) fn factory_role(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
    expected: &str,
) -> Option<()> {
    let target = captured_callable(model, site, expression)?;
    let AstKind::Function(function) = model.nodes.kind(target.node_id) else {
        return None;
    };
    let body = function.body.as_ref()?;
    if function.r#async
        || function.generator
        || !directives(body)
        || !one_plain_formal(model, &function.params)
    {
        return None;
    }
    let BindingPattern::BindingIdentifier(formal) = &function.params.items[0].pattern else {
        return None;
    };
    let formal = formal.symbol_id.get()?;
    let returned = single_return(model, target, body)?;
    let returned_site = super::stable_function(model, returned, 0)?;
    if !model
        .enclosing_function(returned_site.node_id)
        .is_some_and(|owner| owner.span == target.span)
    {
        return None;
    }
    if super::role(model, returned, proposals) != Some(expected) {
        return None;
    }
    factory_formal_feeds_helper(model, target, formal, returned_site, expected).then_some(())
}

/// Mechanical facts for one resolved helper, limited to the call, return and
/// static-member facts that the accepted shared number-writer proof reads.
/// This is the same grouping the shared pass performs, scoped to one target.
fn owned_facts<'a>(model: &Model<'_, 'a>, target: FunctionSite) -> Facts<'a> {
    let nodes = model.nodes;
    let mut facts = Facts::default();
    for node in nodes.iter() {
        let Some(owner) = model.owning_site(node.id()) else {
            continue;
        };
        if owner.span.start != target.span.start {
            continue;
        }
        match node.kind() {
            AstKind::CallExpression(call) => facts.calls.push(call),
            AstKind::ReturnStatement(statement) => {
                if let Some(value) = &statement.argument {
                    facts.returns.push(value);
                }
            }
            AstKind::StaticMemberExpression(member) => {
                facts.members.insert(member.property.name.to_string());
            }
            _ => {}
        }
    }
    facts
}

/// The accepted source-proven number writer used by the variant. The
/// resolved callee site is re-proved synchronously with the shared
/// `thread_posthost_numeric` evaluator and must produce that evaluator's
/// `writeNumber` claim. Name, spelling, arity alone or a foreign four-formal
/// function is never sufficient; a foreign writer fails this identity check
/// even when both numeric call sites use it.
pub(in crate::naming) fn numeric_writer(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Option<Symbol> {
    let value = symbol(model, expression)?;
    let target = captured_callable(model, site, expression)?;
    let AstKind::Function(function) = model.nodes.kind(target.node_id) else {
        return None;
    };
    let body = function.body.as_ref()?;
    if function.r#async
        || function.generator
        || !directives(body)
        || !plain_params(model, &function.params)
        || function.params.items.len() != 4
    {
        return None;
    }
    let facts = owned_facts(model, target);
    let claims = super::numeric_writer::evaluate(model, target, &facts, proposals);
    claims
        .iter()
        .any(|claim| claim.name == "writeNumber" && claim.evaluator == "thread_posthost_numeric")
        .then_some(value)
}

fn global_path(model: &Model<'_, '_>, expression: &Expression<'_>) -> Vec<String> {
    match expression.without_parentheses() {
        Expression::Identifier(identifier) => {
            if crate::analysis::identifier_symbol(identifier, model.scoping).is_some() {
                Vec::new()
            } else {
                vec![identifier.name.to_string()]
            }
        }
        Expression::StaticMemberExpression(member) if !member.optional => {
            let mut path = global_path(model, &member.object);
            if !path.is_empty() {
                path.push(member.property.name.to_string());
            }
            path
        }
        _ => Vec::new(),
    }
}

fn is_global(model: &Model<'_, '_>, expression: &Expression<'_>, path: &[&str]) -> bool {
    let found = global_path(model, expression);
    found.len() == path.len()
        && found
            .iter()
            .zip(path.iter())
            .all(|(actual, expected)| actual.as_str() == *expected)
}

fn random_seed(model: &Model<'_, '_>, expression: &Expression<'_>) -> bool {
    let Expression::BinaryExpression(or) = expression.without_parentheses() else {
        return false;
    };
    if or.operator.as_str() != "|" || !number(&or.right, 0.0) {
        return false;
    }
    let Expression::BinaryExpression(product) = or.left.without_parentheses() else {
        return false;
    };
    if product.operator.as_str() != "*" || !number(&product.right, 4_294_967_296.0) {
        return false;
    }
    let Some(random) = call(&product.left) else {
        return false;
    };
    is_global(model, &random.callee, &["Math", "random"]) && random.arguments.is_empty()
}

fn seed_base(model: &Model<'_, '_>, expression: &Expression<'_>, seed: Symbol) -> bool {
    if symbol(model, expression) == Some(seed) {
        return true;
    }
    let Expression::BinaryExpression(or) = expression.without_parentheses() else {
        return false;
    };
    or.operator.as_str() == "|" && number(&or.right, 0.0) && symbol(model, &or.left) == Some(seed)
}

pub(in crate::naming) fn big_endian_header(model: &Model<'_, '_>, expression: &Expression<'_>, seed: Symbol) -> bool {
    let Expression::ArrayExpression(array) = expression.without_parentheses() else {
        return false;
    };
    let mut elements = Vec::new();
    for element in &array.elements {
        let Some(value) = element.as_expression() else {
            return false;
        };
        elements.push(value);
    }
    if elements.len() != 4 {
        return false;
    }
    let Expression::BinaryExpression(shift) = elements[0].without_parentheses() else {
        return false;
    };
    if shift.operator.as_str() != ">>>"
        || !seed_base(model, &shift.left, seed)
        || !number(&shift.right, 24.0)
    {
        return false;
    }
    for (slot, shift_value) in [(1usize, 16.0f64), (2, 8.0)] {
        let Expression::BinaryExpression(mask) = elements[slot].without_parentheses() else {
            return false;
        };
        if mask.operator.as_str() != "&" || !number(&mask.right, 255.0) {
            return false;
        }
        let Expression::BinaryExpression(inner) = mask.left.without_parentheses() else {
            return false;
        };
        if inner.operator.as_str() != ">>"
            || !seed_base(model, &inner.left, seed)
            || !number(&inner.right, shift_value)
        {
            return false;
        }
    }
    let Expression::BinaryExpression(mask) = elements[3].without_parentheses() else {
        return false;
    };
    mask.operator.as_str() == "&"
        && seed_base(model, &mask.left, seed)
        && number(&mask.right, 255.0)
}

fn has_owned_nested(model: &Model<'_, '_>, site: FunctionSite) -> bool {
    let body = site.body_span.unwrap_or(site.span);
    model.all_sites().iter().any(|other| {
        other.span != site.span && other.span.start >= body.start && other.span.end <= body.end
    })
}

fn prove(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Option<()> {
    let [payload_statement, seed_statement, header_statement, hash_statement, writer_statement, clock_statement, next_statement, complete_statement] =
        body.statements.as_slice()
    else {
        return None;
    };

    // 1. Bounded payload: capturedSource.slice(0, 50).
    let payload_value = one_variable(payload_statement)?;
    let payload = plain_local(model, payload_value)?;
    let slice_call = call(payload_value.init.as_ref()?)?;
    let _source = captured_data(model, site, member(&slice_call.callee, "slice")?)?;
    let slice_arguments = arguments(slice_call, 2)?;
    if !number(slice_arguments[0], 0.0) || !number(slice_arguments[1], 50.0) {
        return None;
    }
    if model.binding_writes(payload) != 1 {
        return None;
    }

    // 2. One 32-bit Math.random draw.
    let seed_value = one_variable(seed_statement)?;
    let seed = plain_local(model, seed_value)?;
    if !unchanged(model, seed) || !random_seed(model, seed_value.init.as_ref()?) {
        return None;
    }

    // 3. Four-byte big-endian header over that same seed.
    let header_value = one_variable(header_statement)?;
    let header = plain_local(model, header_value)?;
    if !unchanged(model, header) || !big_endian_header(model, header_value.init.as_ref()?, seed) {
        return None;
    }

    // 4. Captured keystream factory applied to seed ^ constant.
    let hash_value = one_variable(hash_statement)?;
    let hash = plain_local(model, hash_value)?;
    if !unchanged(model, hash) {
        return None;
    }
    let hash_call = call(hash_value.init.as_ref()?)?;
    factory_role(
        model,
        site,
        &hash_call.callee,
        proposals,
        "nextKeystreamByte",
    )?;
    let hash_arguments = arguments(hash_call, 1)?;
    let Expression::BinaryExpression(xor) = hash_arguments[0].without_parentheses() else {
        return None;
    };
    if xor.operator.as_str() != "^"
        || symbol(model, &xor.left) != Some(seed)
        || !matches!(
            xor.right.without_parentheses(),
            Expression::NumericLiteral(_)
        )
    {
        return None;
    }

    // 5. Curried interned string writer over the same payload, header and
    //    numeric-writer cursor.
    let payload_assignment = assignment(writer_statement)?;
    if assign_symbol(model, payload_assignment) != Some(payload) {
        return None;
    }
    let writer_call = call(&payload_assignment.right)?;
    let factory_call = call(&writer_call.callee)?;
    factory_role(
        model,
        site,
        &factory_call.callee,
        proposals,
        "writeInternedString",
    )?;
    let factory_arguments = arguments(factory_call, 1)?;
    if symbol(model, factory_arguments[0]) != Some(hash) {
        return None;
    }
    let writer_arguments = arguments(writer_call, 3)?;
    if symbol(model, writer_arguments[0]) != Some(payload)
        || symbol(model, writer_arguments[1]) != Some(header)
    {
        return None;
    }
    let writer_number = call(writer_arguments[2])?;
    let numeric_writer_symbol = numeric_writer(model, site, &writer_number.callee, proposals)?;
    let first_number_arguments = arguments(writer_number, 4)?;
    if symbol(model, first_number_arguments[0]) != Some(hash)
        || model.site_parameter_slot(first_number_arguments[1], site) != Some(0)
        || symbol(model, first_number_arguments[2]) != Some(header)
        || !number(first_number_arguments[3], 3.0)
    {
        return None;
    }

    // 6. Same numeric writer over the captured clock delta and returned cursor.
    let clock_call = statement_call(clock_statement)?;
    if symbol(model, &clock_call.callee) != Some(numeric_writer_symbol) {
        return None;
    }
    let clock_arguments = arguments(clock_call, 4)?;
    if symbol(model, clock_arguments[0]) != Some(hash)
        || symbol(model, clock_arguments[2]) != Some(header)
        || symbol(model, clock_arguments[3]) != Some(payload)
    {
        return None;
    }
    let Expression::BinaryExpression(delta) = clock_arguments[1].without_parentheses() else {
        return None;
    };
    if delta.operator.as_str() != "-" {
        return None;
    }
    let now_call = call(&delta.left)?;
    let clock = captured_data(model, site, member(&now_call.callee, "now")?)?;
    arguments(now_call, 0)?;
    let init = captured_data(model, site, &delta.right)?;
    if clock == init {
        return None;
    }

    // The five body locals must be distinct, so no alias can donate one role
    // to another.
    if payload == seed
        || payload == header
        || payload == hash
        || seed == header
        || seed == hash
        || header == hash
    {
        return None;
    }

    // 7. Same captured observer: next([[], header]) then complete().
    let next_call = statement_call(next_statement)?;
    let observer = captured_data(model, site, member(&next_call.callee, "next")?)?;
    let next_arguments = arguments(next_call, 1)?;
    let Expression::ArrayExpression(next_payload) = next_arguments[0].without_parentheses() else {
        return None;
    };
    if next_payload.elements.len() != 2 {
        return None;
    }
    let first = next_payload.elements[0].as_expression()?;
    let second = next_payload.elements[1].as_expression()?;
    if !empty_array(first) || symbol(model, second) != Some(header) {
        return None;
    }
    let complete_call = statement_call(complete_statement)?;
    if symbol(model, member(&complete_call.callee, "complete")?) != Some(observer) {
        return None;
    }
    arguments(complete_call, 0)?;
    Some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return Vec::new();
    };
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !one_plain_formal(model, &function.params)
    {
        return Vec::new();
    }
    let Some(body) = function.body.as_ref() else {
        return Vec::new();
    };
    if !directives(body) || has_owned_nested(model, site) {
        return Vec::new();
    }
    if prove(model, site, body, proposals).is_none() {
        return Vec::new();
    }
    vec![proposal(NAME, EVALUATOR, DETAIL, f)]
}
