//! Fast-loop fallback for the 5x5 canvas image probe.
//!
//! The owned family is one outer setup function plus the nested callback it
//! installs as `image.onload`:
//!
//! * the outer function creates a canvas through a captured `document`-like
//!   receiver, writes `width = height = 5`, takes a
//!   `getContext("2d", { willReadFrequently: true })` context, constructs the
//!   5x5 `Image` through a captured constructor, stores the first formal in
//!   `image.src`, and installs the nested function as `image.onload`;
//! * the owned callback draws that same image onto that same context, reads
//!   `getImageData(0, 0, 5, 5).data`, walks one 4-byte pixel slice per pixel
//!   and hands it to a captured collector together with a field of the outer
//!   config formal, and its `finally` increments the shared load counter and,
//!   once the counter reaches four, builds four random bytes from
//!   `Math.random() * 4294967296`, seeds a captured hash helper with
//!   `random ^ constant`, and emits the byte array through a captured
//!   observer's `next`/`complete`.
//!
//! The rule is a *widening* of `p13_graphics_canvas`'s `startCanvasPixelProbe`
//! / `handleCanvasProbeImageLoad` roles. Those roles only exist inside a
//! registered canvas collector closure; this fallback proves the same two
//! owned bodies when the setup function is reached some other way. The names
//! are therefore reused rather than invented, and the older package stays
//! untouched.
//!
//! Every participant is a scope-resolved symbol. The loop index may be spelled
//! `i`, `j` or a generated local; the loop may be `for` (separate or inline
//! index declaration) or `while`; the 4-byte slice may be a direct
//! `data.slice(a, a + 4)` or the equivalent `[data, data.slice]` holder called
//! through `Function.prototype.call`; the context/image locals may be declared
//! inline or pre-declared in one grouped `let`. The random word may be
//! truncated with `| 0`, either once at its declaration or at each use site
//! (the four byte lanes and the hash-state XOR); both spellings denote the same
//! 32-bit word. None of the hash-update helper bodies are
//! claimed: only their captured identity and the dataflow through them are
//! used, so the numeric encoder and the payload finalizer remain other
//! families' roles.
//!
//! A separate bounded callback grammar also admits two or four uninitialized
//! catch locals. Direct numeric stores in the catch must pair with the same
//! locals in undefined/TypeError guards in the finalizer, including guards after
//! observation. The encoder walk permits only owned local updates and writes
//! to the emitted bytes, with one captured encoder sharing the seeded state.
//!
//! Generated spellings, source offsets, vendor data, body hashes and inherited
//! labels never participate. A function whose direct body does not prove the
//! complete setup or the complete pixel-hash callback is refused.

use super::*;
use crate::signals::extract::bindings::{array_elements, new_arguments};
use oxc_semantic::SymbolId;

type Symbol = SymbolId;

const STARTER: &str = "startCanvasPixelProbe";
const HANDLER: &str = "handleCanvasProbeImageLoad";
const EVALUATOR: &str = "shared_posthost_canvas_image";

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(e, model.scoping)
}

fn is_symbol(model: &Model<'_, '_>, e: &Expression<'_>, s: Symbol) -> bool {
    symbol(model, e.without_parentheses()) == Some(s)
}

fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = e.without_parentheses() else {
        return None;
    };
    (!m.optional && m.property.name == key).then_some(&m.object)
}

fn target_member<'a, 'b>(
    target: &'b AssignmentTarget<'a>,
    key: &str,
) -> Option<&'b Expression<'a>> {
    let AssignmentTarget::StaticMemberExpression(m) = target else {
        return None;
    };
    (!m.optional && m.property.name == key).then_some(&m.object)
}

fn call<'a>(e: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    match e.without_parentheses() {
        Expression::CallExpression(c) if !c.optional => Some(c),
        _ => None,
    }
}

fn new_call<'a>(e: &'a Expression<'a>) -> Option<&'a NewExpression<'a>> {
    match e.without_parentheses() {
        Expression::NewExpression(n) => Some(n),
        _ => None,
    }
}

fn string_value<'a>(e: &'a Expression<'a>) -> Option<&'a str> {
    match e.without_parentheses() {
        Expression::StringLiteral(literal) => Some(literal.value.as_str()),
        _ => None,
    }
}

fn number(e: &Expression<'_>, expected: f64) -> bool {
    matches!(e.without_parentheses(), Expression::NumericLiteral(literal) if literal.value == expected)
}

fn is_numeric(e: &Expression<'_>) -> bool {
    matches!(e.without_parentheses(), Expression::NumericLiteral(_))
}

fn is_void_zero(e: &Expression<'_>) -> bool {
    matches!(e.without_parentheses(), Expression::UnaryExpression(unary)
        if unary.operator.as_str() == "void" && number(&unary.argument, 0.0))
}

fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(b.body.as_slice())
}

fn one_variable<'a>(s: &'a Statement<'a>) -> Option<&'a VariableDeclarator<'a>> {
    let Statement::VariableDeclaration(d) = s else {
        return None;
    };
    let [v] = d.declarations.as_slice() else {
        return None;
    };
    Some(v)
}

fn plain_binding(model: &Model<'_, '_>, p: &BindingPattern<'_>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(id) = p else {
        return None;
    };
    let s = id.symbol_id.get()?;
    model
        .scoping
        .symbol_redeclarations(s)
        .is_empty()
        .then_some(s)
}

fn plain_variable_kind(kind: VariableDeclarationKind) -> bool {
    matches!(
        kind,
        VariableDeclarationKind::Var
            | VariableDeclarationKind::Let
            | VariableDeclarationKind::Const
    )
}

fn plain_parameters(
    model: &Model<'_, '_>,
    function: &Function<'_>,
    arity: usize,
) -> Option<Vec<Symbol>> {
    if function.params.rest.is_some() || function.params.items.len() != arity {
        return None;
    }
    let mut params = Vec::new();
    for parameter in &function.params.items {
        if parameter.initializer.is_some() {
            return None;
        }
        let s = plain_binding(model, &parameter.pattern)?;
        if model.binding_writes(s) != 0 {
            return None;
        }
        params.push(s);
    }
    Some(params)
}

/// A symbol captured from a scope outside `site`, never shadowed by a nested
/// redeclaration. This deliberately allows a declaration that appears after the
/// function text: function declarations are hoisted and the callback only runs
/// after the enclosing setup has completed.
fn captured(model: &Model<'_, '_>, site: FunctionSite, s: Symbol) -> bool {
    !site.span.contains_inclusive(model.scoping.symbol_span(s))
        && model.scoping.symbol_redeclarations(s).is_empty()
}

struct Setup {
    callback: FunctionSite,
}

struct Read {
    context: Symbol,
    image: Symbol,
    config: Symbol,
}

/// `Math.random()` whose `Math` receiver is the unresolved ambient name.
fn is_math_random(model: &Model<'_, '_>, e: &Expression<'_>) -> bool {
    let Some(random) = call(e) else {
        return false;
    };
    if !random.arguments.is_empty() {
        return false;
    }
    let Some(receiver) = member(&random.callee, "random") else {
        return false;
    };
    let Expression::Identifier(identifier) = receiver.without_parentheses() else {
        return false;
    };
    identifier.name == "Math"
        && crate::analysis::identifier_symbol(identifier, model.scoping).is_none()
}

/// `Math.random() * 4294967296`, optionally immediately coerced with `| 0`.
fn is_random_32(model: &Model<'_, '_>, e: &Expression<'_>) -> bool {
    let e = e.without_parentheses();
    let product = match e {
        Expression::BinaryExpression(bit_or) if bit_or.operator.as_str() == "|" => {
            if !number(&bit_or.right, 0.0) {
                return false;
            }
            bit_or.left.without_parentheses()
        }
        _ => e,
    };
    let Expression::BinaryExpression(product) = product else {
        return false;
    };
    product.operator.as_str() == "*"
        && is_math_random(model, &product.left)
        && number(&product.right, 4294967296.0)
}

fn expr_contains(model: &Model<'_, '_>, e: &Expression<'_>, target: Symbol) -> bool {
    match e.without_parentheses() {
        Expression::Identifier(identifier) => {
            crate::analysis::identifier_symbol(identifier, model.scoping) == Some(target)
        }
        Expression::BinaryExpression(binary) => {
            expr_contains(model, &binary.left, target)
                || expr_contains(model, &binary.right, target)
        }
        Expression::UnaryExpression(unary) => expr_contains(model, &unary.argument, target),
        _ => false,
    }
}

/// The random word itself, possibly truncated with `| 0` at the use site:
/// `word` or `word | 0` (either operand order). The operand must still be the
/// exact random binding, never an expression that merely contains it.
fn coerced_word(model: &Model<'_, '_>, e: &Expression<'_>, random: Symbol) -> bool {
    let e = e.without_parentheses();
    if is_symbol(model, e, random) {
        return true;
    }
    let Expression::BinaryExpression(binary) = e else {
        return false;
    };
    binary.operator.as_str() == "|"
        && ((is_symbol(model, &binary.left, random) && number(&binary.right, 0.0))
            || (number(&binary.left, 0.0) && is_symbol(model, &binary.right, random)))
}

/// `random ^ <integer literal>` on either side, where the random word may be
/// truncated with `| 0` at the XOR use site. The word operand must be the
/// random binding itself, not an expression that merely contains it.
fn is_random_xor(model: &Model<'_, '_>, e: &Expression<'_>, random: Symbol) -> bool {
    let Expression::BinaryExpression(binary) = e.without_parentheses() else {
        return false;
    };
    binary.operator.as_str() == "^"
        && ((coerced_word(model, &binary.left, random) && is_numeric(&binary.right))
            || (is_numeric(&binary.left) && coerced_word(model, &binary.right, random)))
}

/// The state-seeding call whose only argument is the random-word XOR.
fn seeds_hash_state(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &[Statement<'_>],
    random: Symbol,
) -> bool {
    body.iter().any(|statement| {
        let Statement::VariableDeclaration(declaration) = statement else {
            return false;
        };
        declaration.declarations.iter().any(|declarator| {
            let Some(init) = declarator.init.as_ref() else {
                return false;
            };
            let Some(seed) = call(init) else {
                return false;
            };
            let Some(callee) = symbol(model, &seed.callee) else {
                return false;
            };
            captured(model, site, callee)
                && seed.arguments.len() == 1
                && is_random_xor(model, call_arguments(seed)[0], random)
        })
    })
}

/// `[ [], <bytes> ]` handed to the observer's `next`.
fn emits_bytes(model: &Model<'_, '_>, e: &Expression<'_>, bytes: Symbol) -> bool {
    let Some(payload) = array_elements(e) else {
        return false;
    };
    let [group, result] = payload.as_slice() else {
        return false;
    };
    array_elements(*group).is_some_and(|elements| elements.is_empty())
        && is_symbol(model, *result, bytes)
}

fn increment_prefix(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Symbol> {
    let Expression::UpdateExpression(update) = e.without_parentheses() else {
        return None;
    };
    if update.operator.as_str() != "++" || !update.prefix {
        return None;
    }
    match &update.argument {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            crate::analysis::identifier_symbol(identifier, model.scoping)
        }
        _ => None,
    }
}

fn increment_only(model: &Model<'_, '_>, e: &Expression<'_>, index: Symbol) -> bool {
    let Expression::UpdateExpression(update) = e.without_parentheses() else {
        return false;
    };
    if update.operator.as_str() != "++" {
        return false;
    }
    match &update.argument {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            crate::analysis::identifier_symbol(identifier, model.scoping) == Some(index)
        }
        _ => false,
    }
}

/// Two occurrences of the same offset expression. Identifier references are
/// compared through the scope-resolved symbol rather than their source spans:
/// the two `offset` references in `slice(offset, offset + 4)` are distinct AST
/// nodes at distinct spans even though they denote the same binding.
fn same_offset(model: &Model<'_, '_>, left: &Expression<'_>, right: &Expression<'_>) -> bool {
    match (left.without_parentheses(), right.without_parentheses()) {
        (Expression::Identifier(a), Expression::Identifier(b)) => {
            match (
                crate::analysis::identifier_symbol(a, model.scoping),
                crate::analysis::identifier_symbol(b, model.scoping),
            ) {
                (Some(a), Some(b)) => a == b,
                (None, None) => a.name == b.name,
                _ => false,
            }
        }
        (Expression::NumericLiteral(a), Expression::NumericLiteral(b)) => a.value == b.value,
        (Expression::BinaryExpression(a), Expression::BinaryExpression(b)) => {
            a.operator.as_str() == b.operator.as_str()
                && same_offset(model, &a.left, &b.left)
                && same_offset(model, &a.right, &b.right)
        }
        _ => false,
    }
}

/// `current + 4` where `current` is the same offset expression already seen.
fn plus_four(model: &Model<'_, '_>, e: &Expression<'_>, current: &Expression<'_>) -> bool {
    let Expression::BinaryExpression(binary) = e.without_parentheses() else {
        return false;
    };
    binary.operator.as_str() == "+"
        && number(&binary.right, 4.0)
        && same_offset(model, &binary.left, current)
}

/// `index * 4` (either operand order).
fn index_times_four(model: &Model<'_, '_>, e: &Expression<'_>, index: Symbol) -> bool {
    let Expression::BinaryExpression(binary) = e.without_parentheses() else {
        return false;
    };
    binary.operator.as_str() == "*"
        && ((is_symbol(model, &binary.left, index) && number(&binary.right, 4.0))
            || (number(&binary.left, 4.0) && is_symbol(model, &binary.right, index)))
}

/// `let offset = index * 4`.
fn offset_declaration(model: &Model<'_, '_>, s: &Statement<'_>, index: Symbol) -> Option<Symbol> {
    let variable = one_variable(s)?;
    let offset = plain_binding(model, &variable.id)?;
    index_times_four(model, variable.init.as_ref()?, index).then_some(offset)
}

/// `let holder = [data, data.slice];`
fn slice_holder(model: &Model<'_, '_>, s: &Statement<'_>, pixels: Symbol) -> Option<Symbol> {
    let variable = one_variable(s)?;
    let holder = plain_binding(model, &variable.id)?;
    let elements = array_elements(variable.init.as_ref()?)?;
    let [data, method] = elements.as_slice() else {
        return None;
    };
    let slice = member(*method, "slice")?;
    (is_symbol(model, *data, pixels) && is_symbol(model, slice, pixels)).then_some(holder)
}

/// `data.slice(a, a + 4)` or `holder[1].call(holder[0], a, a + 4)`.
fn slice_pixel(
    model: &Model<'_, '_>,
    init: &Expression<'_>,
    pixels: Symbol,
    index: Symbol,
    offset: Option<Symbol>,
    expected_holder: Option<Symbol>,
) -> Option<Symbol> {
    let slice = call(init)?;
    let arguments = call_arguments(slice);
    let (first, second) = match (&slice.callee.without_parentheses(), arguments.as_slice()) {
        (Expression::StaticMemberExpression(method), [first, second])
            if !method.optional && method.property.name == "slice" =>
        {
            if expected_holder.is_some() || !is_symbol(model, &method.object, pixels) {
                return None;
            }
            (*first, *second)
        }
        (Expression::StaticMemberExpression(method), [receiver, first, second])
            if !method.optional && method.property.name == "call" =>
        {
            let Expression::ComputedMemberExpression(indexed) = method.object.without_parentheses()
            else {
                return None;
            };
            if indexed.optional || !number(&indexed.expression, 1.0) {
                return None;
            }
            let Some(holder) = symbol(model, &indexed.object) else {
                return None;
            };
            if expected_holder != Some(holder) {
                return None;
            }
            let Expression::ComputedMemberExpression(argument_receiver) =
                receiver.without_parentheses()
            else {
                return None;
            };
            if argument_receiver.optional
                || !number(&argument_receiver.expression, 0.0)
                || !is_symbol(model, &argument_receiver.object, holder)
            {
                return None;
            }
            (*first, *second)
        }
        _ => return None,
    };
    let first_ok = match offset {
        Some(offset) => is_symbol(model, first, offset),
        None => index_times_four(model, first, index),
    };
    if !first_ok || !plus_four(model, second, first) {
        return None;
    }
    Some(pixels)
}

struct PixelLoop {
    config: Symbol,
}

/// The `for`/`while` pixel walk. Accepts exactly one 4-byte slice per iteration
/// and one collector call whose second argument is a config field.
fn pixel_loop(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statements: &[Statement<'_>],
    pixels: Symbol,
) -> Option<PixelLoop> {
    let (loop_statement, separate_index) = match statements {
        [Statement::VariableDeclaration(declaration), loop_statement, Statement::ReturnStatement(end)] =>
        {
            if !plain_variable_kind(declaration.kind) {
                return None;
            }
            let [variable] = declaration.declarations.as_slice() else {
                return None;
            };
            let index = plain_binding(model, &variable.id)?;
            if !matches!(variable.init.as_ref()?, Expression::NumericLiteral(literal) if literal.value == 0.0)
            {
                return None;
            }
            if !is_void_zero(end.argument.as_ref()?) {
                return None;
            }
            (loop_statement, Some(index))
        }
        [loop_statement, Statement::ReturnStatement(end)] => {
            if !is_void_zero(end.argument.as_ref()?) {
                return None;
            }
            (loop_statement, None)
        }
        _ => return None,
    };
    match loop_statement {
        Statement::ForStatement(loop_) => {
            let index = match (loop_.init.as_ref(), separate_index) {
                (Some(ForStatementInit::VariableDeclaration(declaration)), None) => {
                    if !plain_variable_kind(declaration.kind) {
                        return None;
                    }
                    let [variable] = declaration.declarations.as_slice() else {
                        return None;
                    };
                    let index = plain_binding(model, &variable.id)?;
                    if !matches!(variable.init.as_ref()?, Expression::NumericLiteral(literal) if literal.value == 0.0)
                    {
                        return None;
                    }
                    index
                }
                (None, Some(index)) => index,
                _ => return None,
            };
            if !test_quarter(model, loop_.test.as_ref()?, index, pixels) {
                return None;
            }
            let update = loop_.update.as_ref()?;
            if !increment_only(model, update, index) {
                return None;
            }
            loop_body(model, site, block(&loop_.body)?, index, pixels)
        }
        Statement::WhileStatement(loop_) => {
            let index = separate_index?;
            if !test_quarter(model, &loop_.test, index, pixels) {
                return None;
            }
            let body = block(&loop_.body)?;
            let (end, rest) = body.split_last()?;
            let Statement::ExpressionStatement(step) = end else {
                return None;
            };
            if !increment_only(model, &step.expression, index) {
                return None;
            }
            loop_body(model, site, rest, index, pixels)
        }
        _ => return None,
    }
}

/// `index < data.length / 4`.
fn test_quarter(
    model: &Model<'_, '_>,
    test: &Expression<'_>,
    index: Symbol,
    pixels: Symbol,
) -> bool {
    let Expression::BinaryExpression(compare) = test.without_parentheses() else {
        return false;
    };
    if compare.operator.as_str() != "<" || !is_symbol(model, &compare.left, index) {
        return false;
    }
    let Expression::BinaryExpression(divide) = compare.right.without_parentheses() else {
        return false;
    };
    divide.operator.as_str() == "/"
        && number(&divide.right, 4.0)
        && member(&divide.left, "length").is_some_and(|data| is_symbol(model, data, pixels))
}

fn loop_body(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statements: &[Statement<'_>],
    index: Symbol,
    pixels: Symbol,
) -> Option<PixelLoop> {
    let mut remaining = statements;
    let offset = remaining
        .first()
        .and_then(|statement| offset_declaration(model, statement, index));
    if offset.is_some() {
        remaining = &remaining[1..];
    }
    let holder = remaining
        .first()
        .and_then(|statement| slice_holder(model, statement, pixels));
    if holder.is_some() {
        remaining = &remaining[1..];
    }
    let [pixel_statement, collector_statement] = remaining else {
        return None;
    };
    let variable = one_variable(pixel_statement)?;
    let pixel = plain_binding(model, &variable.id)?;
    let Some(pixel_init) = variable.init.as_ref() else {
        return None;
    };
    let sliced = slice_pixel(model, pixel_init, pixels, index, offset, holder)?;
    if sliced == pixel {
        return None;
    }
    let Statement::ExpressionStatement(emit) = collector_statement else {
        return None;
    };
    let collector = call(&emit.expression)?;
    let arguments = call_arguments(collector);
    let [value, field] = arguments.as_slice() else {
        return None;
    };
    if !is_symbol(model, value, pixel) {
        return None;
    }
    let Expression::StaticMemberExpression(field) = field.without_parentheses() else {
        return None;
    };
    if field.optional {
        return None;
    }
    let config = symbol(model, &field.object)?;
    let collector = symbol(model, &collector.callee)?;
    if !captured(model, site, collector) || !captured(model, site, config) {
        return None;
    }
    if pixel == pixels || pixel == config {
        return None;
    }
    let _ = collector;
    Some(PixelLoop { config })
}

/// The `finally` counter guard and the random-byte observation.
fn finalizer(
    model: &Model<'_, '_>,
    site: FunctionSite,
    finalizer: &BlockStatement<'_>,
) -> Option<()> {
    let [Statement::ExpressionStatement(increment), Statement::IfStatement(guard)] =
        finalizer.body.as_slice()
    else {
        return None;
    };
    let counter = increment_prefix(model, &increment.expression)?;
    if !captured(model, site, counter) || guard.alternate.is_some() {
        return None;
    }
    let Expression::BinaryExpression(test) = guard.test.without_parentheses() else {
        return None;
    };
    if test.operator.as_str() != ">="
        || !is_symbol(model, &test.left, counter)
        || !number(&test.right, 4.0)
    {
        return None;
    }
    let body = block(&guard.consequent)?;
    let random_variable = one_variable(body.first()?)?;
    let random = plain_binding(model, &random_variable.id)?;
    if !is_random_32(model, random_variable.init.as_ref()?) {
        return None;
    }
    let bytes_variable = one_variable(body.get(1)?)?;
    let bytes = plain_binding(model, &bytes_variable.id)?;
    let elements = array_elements(bytes_variable.init.as_ref()?)?;
    if elements.len() != 4
        || !elements
            .iter()
            .all(|element| expr_contains(model, element, random))
    {
        return None;
    }
    if !seeds_hash_state(model, site, body, random) {
        return None;
    }
    let mut emitter: Option<Symbol> = None;
    let mut next_seen = false;
    let mut complete_seen = false;
    for statement in body {
        let Statement::ExpressionStatement(expression) = statement else {
            continue;
        };
        let Some(observe) = call(&expression.expression) else {
            continue;
        };
        let Some(receiver) =
            member(&observe.callee, "next").or_else(|| member(&observe.callee, "complete"))
        else {
            continue;
        };
        let Some(object) = symbol(model, receiver) else {
            continue;
        };
        if !captured(model, site, object)
            || object == counter
            || object == random
            || object == bytes
        {
            return None;
        }
        if emitter.is_some_and(|previous| previous != object) {
            return None;
        }
        emitter = Some(object);
        if member(&observe.callee, "next").is_some() {
            if observe.arguments.len() != 1
                || !emits_bytes(model, call_arguments(observe)[0], bytes)
            {
                return None;
            }
            next_seen = true;
        } else {
            if !observe.arguments.is_empty() {
                return None;
            }
            complete_seen = true;
        }
    }
    if !next_seen || !complete_seen {
        return None;
    }
    let _ = emitter;
    let _ = counter;
    Some(())
}

/// Catch stores are direct, single writes to the callback's otherwise
/// uninitialized locals. They are consumed only by matching undefined/throw
/// guards in the finalizer, never by another function or by nested writes.
fn assignment_symbol(model: &Model<'_, '_>, target: &AssignmentTarget<'_>) -> Option<Symbol> {
    let AssignmentTarget::AssignmentTargetIdentifier(id) = target else {
        return None;
    };
    crate::analysis::identifier_symbol(id, model.scoping)
}

fn catch_states(model: &Model<'_, '_>, stores: &[Statement<'_>], states: &[Symbol]) -> Option<()> {
    if stores.len() != states.len() {
        return None;
    }
    let mut seen = Vec::new();
    for store in stores {
        let Statement::ExpressionStatement(store) = store else {
            return None;
        };
        let Expression::AssignmentExpression(store) = &store.expression else {
            return None;
        };
        let state = assignment_symbol(model, &store.left)?;
        let Expression::NumericLiteral(value) = store.right.without_parentheses() else {
            return None;
        };
        if store.operator.as_str() != "="
            || !states.contains(&state)
            || seen.contains(&state)
            || !value.value.is_finite()
        {
            return None;
        }
        seen.push(state);
    }
    Some(())
}

/// A separate, bounded grammar for the stateful finalizer. All expressions and
/// effects are checked, including both sides of branches and loop bodies. No
/// function declarations, deferred callbacks, arbitrary calls or captured
/// writes are allowed. Encoder calls share the exact seeded state and bytes.
struct StateEncoder<'m, 's, 'a> {
    model: &'m Model<'s, 'a>,
    site: FunctionSite,
    states: &'m [Symbol],
    checked: Vec<Symbol>,
    state_roles: Vec<(Symbol, bool)>,
    guards_allowed: bool,
    in_sample_loop: bool,
    sample_loop_seen: bool,
    observer_seen: bool,
    allow_initial_encoder: bool,
    loop_entry_cursors: Option<Vec<Symbol>>,
    locals: Vec<Symbol>,
    cursors: Vec<Symbol>,
    random: Symbol,
    seed: Symbol,
    bytes: Symbol,
    encoder: Option<Symbol>,
    budget: usize,
}

impl StateEncoder<'_, '_, '_> {
    fn expression(&mut self, e: &Expression<'_>, depth: usize) -> bool {
        if depth > 24 {
            return false;
        }
        match e.without_parentheses() {
            Expression::NumericLiteral(_) | Expression::NullLiteral(_) => true,
            Expression::Identifier(_) => symbol(self.model, e)
                .is_some_and(|s| self.locals.contains(&s) && !self.states.contains(&s)),
            Expression::StaticMemberExpression(m) => {
                !m.optional
                    && matches!(m.property.name.as_str(), "a" | "b" | "c" | "length")
                    && self.receiver(&m.object, depth + 1)
            }
            Expression::ComputedMemberExpression(m) => {
                !m.optional
                    && self.receiver(&m.object, depth + 1)
                    && self.expression(&m.expression, depth + 1)
            }
            Expression::BinaryExpression(b) => {
                matches!(
                    b.operator.as_str(),
                    "+" | "-" | "*" | "/" | "^" | "&" | ">>" | ">>>" | "|" | "<" | ">=" | "=="
                ) && self.expression(&b.left, depth + 1)
                    && self.expression(&b.right, depth + 1)
            }
            Expression::UnaryExpression(u) => {
                u.operator.as_str() == "!" && self.expression(&u.argument, depth + 1)
            }
            Expression::ConditionalExpression(c) => {
                let initial = self.allow_initial_encoder;
                self.allow_initial_encoder = false;
                let valid = self.expression(&c.test, depth + 1)
                    && self.expression(&c.consequent, depth + 1)
                    && self.expression(&c.alternate, depth + 1);
                self.allow_initial_encoder = initial;
                valid
            }
            Expression::CallExpression(c) if !c.optional => {
                let args = call_arguments(c);
                if args.len() != c.arguments.len() {
                    return false;
                }
                if is_symbol(self.model, &c.callee, self.seed) {
                    return args.is_empty();
                }
                if let Expression::Identifier(id) = c.callee.without_parentheses() {
                    if id.name == "Number" && symbol(self.model, &c.callee).is_none() {
                        return args.len() == 1 && self.expression(args[0], depth + 1);
                    }
                }
                let Some(helper) = symbol(self.model, &c.callee) else {
                    return false;
                };
                if !captured(self.model, self.site, helper)
                    || self.encoder.is_some_and(|s| s != helper)
                    || args.len() != 4
                    || !is_symbol(self.model, args[0], self.seed)
                    || !is_symbol(self.model, args[2], self.bytes)
                {
                    return false;
                }
                let initial = self.encoder.is_none();
                if initial && !self.allow_initial_encoder {
                    return false;
                }
                self.encoder = Some(helper);
                (if initial {
                    number(args[3], 3.0)
                } else {
                    self.cursor(args[3])
                }) && self.expression(args[1], depth + 1)
                    && self.expression(args[3], depth + 1)
            }
            _ => false,
        }
    }

    /// Encoder results, aliases of those results and the one-byte marker
    /// advance are the only output cursors. Loop indices and uninitialized
    /// locals cannot become an encoder offset merely by being in scope.
    fn cursor(&self, e: &Expression<'_>) -> bool {
        self.cursor_at(e, 0)
    }

    fn cursor_at(&self, e: &Expression<'_>, depth: usize) -> bool {
        if depth > 24 {
            return false;
        }
        if let Some(s) = symbol(self.model, e) {
            return self.cursors.contains(&s);
        }
        match e.without_parentheses() {
            Expression::BinaryExpression(b) => {
                b.operator.as_str() == "+"
                    && self.cursor_at(&b.left, depth + 1)
                    && number(&b.right, 1.0)
            }
            Expression::CallExpression(c) => {
                let args = call_arguments(c);
                !c.optional
                    && args.len() == 4
                    && c.arguments.len() == 4
                    && symbol(self.model, &c.callee) == self.encoder
                    && is_symbol(self.model, args[0], self.seed)
                    && is_symbol(self.model, args[2], self.bytes)
                    && (number(args[3], 3.0) || self.cursor_at(args[3], depth + 1))
            }
            _ => false,
        }
    }

    fn set_cursor(&mut self, local: Symbol, value: &Expression<'_>) {
        let is_cursor = self.cursor(value);
        self.cursors.retain(|s| *s != local);
        if is_cursor {
            self.cursors.push(local);
        }
    }

    fn receiver(&mut self, e: &Expression<'_>, depth: usize) -> bool {
        if let Some(s) = symbol(self.model, e) {
            return !self.states.contains(&s)
                && (self.locals.contains(&s) || captured(self.model, self.site, s));
        }
        self.expression(e, depth)
    }

    fn effect(&mut self, e: &Expression<'_>) -> bool {
        match e.without_parentheses() {
            Expression::AssignmentExpression(a) if a.operator.as_str() == "=" => {
                if let Some(s) = assignment_symbol(self.model, &a.left) {
                    if !self.locals.contains(&s)
                        || s == self.random
                        || s == self.seed
                        || s == self.bytes
                        || self.states.contains(&s)
                        || !self.expression(&a.right, 0)
                    {
                        return false;
                    }
                    self.set_cursor(s, &a.right);
                    return true;
                }
                // The only property write in this grammar appends the null
                // marker to the very byte vector that is emitted.
                let AssignmentTarget::ComputedMemberExpression(m) = &a.left else {
                    return false;
                };
                let Expression::BinaryExpression(xor) = a.right.without_parentheses() else {
                    return false;
                };
                let Some(random) = call(&xor.left) else {
                    return false;
                };
                !m.optional
                    && is_symbol(self.model, &m.object, self.bytes)
                    && self.cursor(&m.expression)
                    && self.expression(&m.expression, 0)
                    && xor.operator.as_str() == "^"
                    && is_symbol(self.model, &random.callee, self.seed)
                    && random.arguments.is_empty()
                    && number(&xor.right, 130.0)
            }
            Expression::UpdateExpression(u) => match &u.argument {
                SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => {
                    crate::analysis::identifier_symbol(id, self.model.scoping).is_some_and(|s| {
                        self.locals.contains(&s)
                            && s != self.random
                            && s != self.seed
                            && s != self.bytes
                    }) && u.operator.as_str() == "++"
                }
                _ => false,
            },
            _ => self.expression(e, 0),
        }
    }

    fn statements(&mut self, body: &[Statement<'_>], depth: usize) -> bool {
        if depth > 8 {
            return false;
        }
        for (position, statement) in body.iter().enumerate() {
            if self.budget == 0 {
                return false;
            }
            self.budget -= 1;
            let valid = match statement {
                Statement::VariableDeclaration(d) if plain_variable_kind(d.kind) => {
                    d.declarations.iter().all(|v| {
                        let Some(s) = plain_binding(self.model, &v.id) else {
                            return false;
                        };
                        if self.locals.contains(&s) || self.states.contains(&s) {
                            return false;
                        }
                        if v.init.as_ref().is_some_and(|e| !self.expression(e, 0)) {
                            return false;
                        }
                        if let Some(init) = v.init.as_ref() {
                            self.set_cursor(s, init);
                        }
                        self.locals.push(s);
                        true
                    })
                }
                Statement::ExpressionStatement(e) => self.effect(&e.expression),
                Statement::BlockStatement(b) => {
                    let saved = self.locals.len();
                    let valid = self.statements(&b.body, depth + 1);
                    self.locals.truncate(saved);
                    self.cursors.retain(|s| self.locals.contains(s));
                    valid
                }
                Statement::IfStatement(i) => {
                    if let Expression::BinaryExpression(test) = i.test.without_parentheses() {
                        if let Some(state) =
                            symbol(self.model, &test.left).filter(|s| self.states.contains(s))
                        {
                            if test.operator.as_str() != "==="
                                || !is_void_zero(&test.right)
                                || !self.guards_allowed
                                || self.checked.contains(&state)
                                || !self.state_roles.iter().any(|(s, indexed)| {
                                    *s == state
                                        && undefined_throw(self.model, &i.consequent, *indexed)
                                })
                            {
                                return false;
                            }
                            // Two-state protocols check both locals directly in
                            // the sample walk. The four-state protocol also has
                            // its first pair directly after observer completion.
                            let position = self
                                .state_roles
                                .iter()
                                .position(|(s, _)| *s == state)
                                .unwrap();
                            let sample_guard = self.states.len() == 2 || position >= 2;
                            if sample_guard != self.in_sample_loop
                                || (!sample_guard && !self.observer_seen)
                            {
                                return false;
                            }
                            self.checked.push(state);
                            i.alternate
                                .as_ref()
                                .is_none_or(|s| self.statements(std::slice::from_ref(s), depth + 1))
                        } else {
                            self.branch(i, depth)
                        }
                    } else {
                        self.branch(i, depth)
                    }
                }
                Statement::WhileStatement(w) => {
                    // The only while protocol is the direct sample walk, not
                    // an arbitrary wrapper around mandatory guard evidence.
                    let Expression::BinaryExpression(test) = w.test.without_parentheses() else {
                        return false;
                    };
                    if depth != 0
                        || self.sample_loop_seen
                        || self.observer_seen
                        || self.encoder.is_none()
                        || test.operator.as_str() != "<"
                        || !symbol(self.model, &test.left).is_some_and(|s| self.locals.contains(&s))
                        || !symbol(self.model, &test.right)
                            .is_some_and(|s| self.locals.contains(&s))
                    {
                        return false;
                    }
                    self.sample_loop_seen = true;
                    self.expression(&w.test, 0) && self.loop_body(&w.body, None, depth, true)
                }
                Statement::ForStatement(f) => {
                    let initial = self.allow_initial_encoder;
                    self.allow_initial_encoder = false;
                    let valid = f.init.is_none()
                        && f.test.as_ref().is_none_or(|e| self.expression(e, 0))
                        && self.loop_body(&f.body, f.update.as_ref(), depth, false);
                    self.allow_initial_encoder = initial;
                    valid
                }
                Statement::ContinueStatement(c) => {
                    c.label.is_none()
                        && position + 1 == body.len()
                        && depth > 0
                        && self
                            .loop_entry_cursors
                            .as_ref()
                            .is_some_and(|entry| entry.iter().all(|s| self.cursors.contains(s)))
                }

                _ => false,
            };
            if !valid {
                return false;
            }
        }
        true
    }

    /// Body effects must hold on the zero-iteration path too. Existing cursor
    /// roles may survive a loop, but a body cannot establish new exit facts.
    /// Required sample guards are structural evidence at this one protocol
    /// position, not evidence that a conditional sample walk executed.
    fn loop_body(
        &mut self,
        body: &Statement<'_>,
        update: Option<&Expression<'_>>,
        depth: usize,
        sample: bool,
    ) -> bool {
        let before = self.cursors.clone();
        let outer_entry = self.loop_entry_cursors.replace(before.clone());
        let initial = self.allow_initial_encoder;
        let guards = self.guards_allowed;
        let outer_sample = self.in_sample_loop;
        self.allow_initial_encoder = false;
        self.guards_allowed = sample && guards;
        self.in_sample_loop = sample;
        let valid = self.statements(std::slice::from_ref(body), depth + 1)
            && update.is_none_or(|e| self.effect(e));
        self.cursors.retain(|s| before.contains(s));
        self.loop_entry_cursors = outer_entry;
        self.allow_initial_encoder = initial;
        self.guards_allowed = guards;
        self.in_sample_loop = outer_sample;
        valid
    }

    fn branch(&mut self, i: &IfStatement<'_>, depth: usize) -> bool {
        if let Statement::BreakStatement(b) = &i.consequent {
            let Expression::UnaryExpression(negated) = i.test.without_parentheses() else {
                return false;
            };
            let Expression::BinaryExpression(test) = negated.argument.without_parentheses() else {
                return false;
            };
            return depth > 0
                && self
                    .loop_entry_cursors
                    .as_ref()
                    .is_some_and(|entry| entry.iter().all(|s| self.cursors.contains(s)))
                && b.label.is_none()
                && i.alternate.is_none()
                && negated.operator.as_str() == "!"
                && test.operator.as_str() == ">="
                && number(&test.right, 0.0)
                && self.expression(&test.left, 0);
        }
        let allowed = self.guards_allowed;
        let initial = self.allow_initial_encoder;
        self.allow_initial_encoder = false;
        self.guards_allowed = false;
        let before = self.cursors.clone();
        let test = self.expression(&i.test, 0);
        let consequent = self.statements(std::slice::from_ref(&i.consequent), depth + 1);
        let after_consequent = self.cursors.clone();
        self.cursors = before;
        let alternate = i
            .alternate
            .as_ref()
            .is_none_or(|s| self.statements(std::slice::from_ref(s), depth + 1));
        self.cursors.retain(|s| after_consequent.contains(s));
        let valid = test && consequent && alternate;
        self.guards_allowed = allowed;
        self.allow_initial_encoder = initial;
        valid
    }
}

fn undefined_throw(model: &Model<'_, '_>, statement: &Statement<'_>, indexed: bool) -> bool {
    let Some([Statement::ThrowStatement(t)]) = block(statement) else {
        return false;
    };
    let Some(error) = new_call(&t.argument) else {
        return false;
    };
    let Expression::Identifier(id) = error.callee.without_parentheses() else {
        return false;
    };
    let args = new_arguments(error);
    id.name == "TypeError"
        && symbol(model, &error.callee).is_none()
        && args.len() == 1
        && string_value(args[0])
            == Some(if indexed {
                "Cannot read properties of undefined (reading '0')"
            } else {
                "Cannot read properties of undefined (reading 'undefined')"
            })
}

fn random_byte_lane(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    random: Symbol,
    shift: f64,
    masked: bool,
) -> bool {
    let e = if masked {
        let Expression::BinaryExpression(mask) = e.without_parentheses() else {
            return false;
        };
        if mask.operator.as_str() != "&" || !number(&mask.right, 255.0) {
            return false;
        }
        &mask.left
    } else {
        e
    };
    matches!(e.without_parentheses(), Expression::BinaryExpression(b)
        if b.operator.as_str() == (if masked { ">>" } else { ">>>" })
            && coerced_word(model, &b.left, random) && number(&b.right, shift))
}

fn state_finalizer(
    model: &Model<'_, '_>,
    site: FunctionSite,
    finalizer: &BlockStatement<'_>,
    states: &[Symbol],
    stores: &[Statement<'_>],
) -> Option<()> {
    let [_, Statement::IfStatement(guard)] = finalizer.body.as_slice() else {
        return None;
    };
    let body = block(&guard.consequent)?;
    let random = plain_binding(model, &one_variable(body.first()?)?.id)?;
    let bytes = plain_binding(model, &one_variable(body.get(1)?)?.id)?;
    let elements = array_elements(one_variable(body.get(1)?)?.init.as_ref()?)?;
    if elements.len() != 4
        || !random_byte_lane(model, elements[0], random, 24.0, false)
        || !random_byte_lane(model, elements[1], random, 16.0, true)
        || !random_byte_lane(model, elements[2], random, 8.0, true)
        || !matches!(elements[3].without_parentheses(), Expression::BinaryExpression(b)
            if b.operator.as_str() == "&" && coerced_word(model, &b.left, random)
                && number(&b.right, 255.0))
    {
        return None;
    }
    let seed_var = one_variable(body.get(2)?)?;
    let seed = plain_binding(model, &seed_var.id)?;
    let seed_call = call(seed_var.init.as_ref()?)?;
    let args = call_arguments(seed_call);
    if args.len() != 1
        || !is_random_xor(model, args[0], random)
        || !captured(model, site, symbol(model, &seed_call.callee)?)
    {
        return None;
    }
    // Stores occur in tag/exception-flag pairs. Their values merely prove
    // defined state; generated numeric constants never identify the family.
    let mut checker = StateEncoder {
        model,
        site,
        states,
        checked: Vec::new(),
        guards_allowed: true,
        in_sample_loop: false,
        sample_loop_seen: false,
        observer_seen: false,
        allow_initial_encoder: true,
        loop_entry_cursors: None,
        state_roles: stores
            .iter()
            .enumerate()
            .filter_map(|(index, s)| {
                let Statement::ExpressionStatement(s) = s else {
                    return None;
                };
                let Expression::AssignmentExpression(a) = &s.expression else {
                    return None;
                };
                Some((assignment_symbol(model, &a.left)?, index % 2 == 0))
            })
            .collect(),
        locals: vec![random, bytes, seed],
        cursors: Vec::new(),
        random,
        seed,
        bytes,
        encoder: None,
        budget: 160,
    };
    let mut index = 3;
    // Observation must be a consecutive direct next/complete pair. The
    // existing finalizer proof ties both calls to the same captured observer.
    let mut observed = false;
    while index < body.len() {
        if let Statement::ExpressionStatement(e) = &body[index] {
            if let Some(c) = call(&e.expression) {
                if member(&c.callee, "next").is_some() {
                    if observed {
                        return None;
                    }
                    let Statement::ExpressionStatement(done) = body.get(index + 1)? else {
                        return None;
                    };
                    let done = call(&done.expression)?;
                    if member(&done.callee, "complete").is_none() {
                        return None;
                    }
                    observed = true;
                    checker.observer_seen = true;
                    index += 2;
                    continue;
                }
            }
        }
        if !checker.statements(&body[index..index + 1], 0) {
            return None;
        }
        index += 1;
    }
    (observed
        && checker.sample_loop_seen
        && checker.encoder.is_some()
        && checker.checked.len() == states.len())
    .then_some(())
}

/// The owned on-load pixel-hash callback, proved from its own direct body.
fn image_load(model: &Model<'_, '_>, site: FunctionSite, function: &Function<'_>) -> Option<Read> {
    if function.r#async || function.generator {
        return None;
    }
    let body = function.body.as_ref()?;
    if !plain_parameters(model, function, 0)?.is_empty() || !body.directives.is_empty() {
        return None;
    }
    let (attempt, states) = match body.statements.as_slice() {
        [Statement::TryStatement(attempt)] => (attempt, Vec::new()),
        [Statement::VariableDeclaration(locals), Statement::TryStatement(attempt)] => {
            if !plain_variable_kind(locals.kind) || !matches!(locals.declarations.len(), 2 | 4) {
                return None;
            }
            let mut states = Vec::new();
            for local in &locals.declarations {
                let state = plain_binding(model, &local.id)?;
                if local.init.is_some() || model.binding_writes(state) != 1 {
                    return None;
                }
                states.push(state);
            }
            (attempt, states)
        }
        _ => return None,
    };
    let [Statement::ExpressionStatement(draw), Statement::TryStatement(read)] =
        attempt.block.body.as_slice()
    else {
        return None;
    };
    let catch = attempt.handler.as_ref()?;
    let (caught, stores) = catch.body.body.as_slice().split_last()?;
    let Statement::ReturnStatement(caught) = caught else {
        return None;
    };
    if states.is_empty() {
        if !stores.is_empty() {
            return None;
        }
    } else {
        catch_states(model, stores, &states)?;
    }
    if !is_void_zero(caught.argument.as_ref()?) {
        return None;
    }
    let draw = call(&draw.expression)?;
    let context = symbol(model, member(&draw.callee, "drawImage")?)?;
    let draw_arguments = call_arguments(draw);
    let [image, x, y] = draw_arguments.as_slice() else {
        return None;
    };
    let image = symbol(model, image)?;
    if !number(x, 0.0) || !number(y, 0.0) {
        return None;
    }
    if !captured(model, site, context) || !captured(model, site, image) {
        return None;
    }
    if read.finalizer.is_some() {
        return None;
    }
    let read_catch = read.handler.as_ref()?;
    let [Statement::ReturnStatement(failed)] = read_catch.body.body.as_slice() else {
        return None;
    };
    if !is_void_zero(failed.argument.as_ref()?) {
        return None;
    }
    let statements = read.block.body.as_slice();
    let pixels_variable = one_variable(statements.first()?)?;
    let pixels = plain_binding(model, &pixels_variable.id)?;
    let data = member(pixels_variable.init.as_ref()?, "data")?;
    let read_call = call(data)?;
    if symbol(model, member(&read_call.callee, "getImageData")?) != Some(context) {
        return None;
    }
    let read_arguments = call_arguments(read_call);
    let [x, y, width, height] = read_arguments.as_slice() else {
        return None;
    };
    if !number(x, 0.0) || !number(y, 0.0) || !number(width, 5.0) || !number(height, 5.0) {
        return None;
    }
    let pixel_walk = pixel_loop(model, site, &statements[1..], pixels)?;
    let attempt_finalizer = attempt.finalizer.as_ref()?;
    finalizer(model, site, attempt_finalizer)?;
    if !states.is_empty() {
        state_finalizer(model, site, attempt_finalizer, &states, stores)?;
    }
    Some(Read {
        context,
        image,
        config: pixel_walk.config,
    })
}

fn will_read_frequently(e: &Expression<'_>) -> bool {
    let Expression::ObjectExpression(object) = e.without_parentheses() else {
        return false;
    };
    let [property] = object.properties.as_slice() else {
        return false;
    };
    let ObjectPropertyKind::ObjectProperty(property) = property else {
        return false;
    };
    !property.computed
        && !property.method
        && !property.shorthand
        && property_key_name(&property.key).as_deref() == Some("willReadFrequently")
        && matches!(&property.value, Expression::BooleanLiteral(literal) if literal.value)
}

/// `canvas.getContext("2d", { willReadFrequently: true })`.
fn get_context(model: &Model<'_, '_>, e: &Expression<'_>, canvas: Symbol) -> Option<()> {
    let context = call(e)?;
    if symbol(model, member(&context.callee, "getContext")?) != Some(canvas) {
        return None;
    }
    let arguments = call_arguments(context);
    let [kind, options] = arguments.as_slice() else {
        return None;
    };
    (string_value(kind) == Some("2d") && will_read_frequently(options)).then_some(())
}

/// `new Image(5, 5)` through a captured constructor.
fn image_constructor(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<()> {
    let image = new_call(e)?;
    let constructor = symbol(model, &image.callee)?;
    if !captured(model, site, constructor) {
        return None;
    }
    let arguments = new_arguments(image);
    let [width, height] = arguments.as_slice() else {
        return None;
    };
    (number(width, 5.0) && number(height, 5.0)).then_some(())
}

/// The outer 5x5 canvas image probe setup.
fn probe_setup(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
) -> Option<Setup> {
    if function.r#async || function.generator {
        return None;
    }
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let parameters = plain_parameters(model, function, 2)?;
    let [source, config_formal] = parameters.as_slice() else {
        return None;
    };
    let statements = body.statements.as_slice();
    // The callback declaration is owned by this body.
    let Statement::FunctionDeclaration(callback) = statements.first()? else {
        return None;
    };
    let callback_symbol = callback.id.as_ref()?.symbol_id.get()?;
    let callback_site = model.function_from_symbol(callback_symbol)?;
    if callback_site.span == site.span || !body.span.contains_inclusive(callback_site.span) {
        return None;
    }
    let AstKind::Function(callback_function) = model.nodes.kind(callback_site.node_id) else {
        return None;
    };
    // `let config = configFormal;` immediately after the declaration.
    let config_variable = one_variable(statements.get(1)?)?;
    let config = plain_binding(model, &config_variable.id)?;
    if config == *source || config == *config_formal {
        return None;
    }
    if model.binding_writes(config) != 0
        || !is_symbol(model, config_variable.init.as_ref()?, *config_formal)
    {
        return None;
    }
    let mut index = 2;
    let mut predeclared: Vec<Symbol> = Vec::new();
    if let Some(Statement::VariableDeclaration(declaration)) = statements.get(index) {
        if declaration
            .declarations
            .iter()
            .all(|variable| variable.init.is_none())
        {
            if !plain_variable_kind(declaration.kind) {
                return None;
            }
            for variable in &declaration.declarations {
                let predeclared_symbol = plain_binding(model, &variable.id)?;
                if predeclared_symbol == config {
                    return None;
                }
                predeclared.push(predeclared_symbol);
            }
            index += 1;
        }
    }
    // Canvas creation through the captured document-like receiver.
    let canvas_variable = one_variable(statements.get(index)?)?;
    index += 1;
    let canvas = plain_binding(model, &canvas_variable.id)?;
    if canvas == config || model.binding_writes(canvas) != 0 {
        return None;
    }
    let create = call(canvas_variable.init.as_ref()?)?;
    let document = symbol(model, member(&create.callee, "createElement")?)?;
    let create_arguments = call_arguments(create);
    let [element] = create_arguments.as_slice() else {
        return None;
    };
    if string_value(element) != Some("canvas") || !captured(model, site, document) {
        return None;
    }
    // `canvas.width = 5; canvas.height = 5;`
    let Statement::ExpressionStatement(width) = statements.get(index)? else {
        return None;
    };
    index += 1;
    let Statement::ExpressionStatement(height) = statements.get(index)? else {
        return None;
    };
    index += 1;
    let size = |statement: &ExpressionStatement<'_>, key: &str| {
        let Expression::AssignmentExpression(assignment) = &statement.expression else {
            return false;
        };
        assignment.operator.as_str() == "="
            && target_member(&assignment.left, key)
                .is_some_and(|object| is_symbol(model, object, canvas))
            && number(&assignment.right, 5.0)
    };
    if !size(width, "width") || !size(height, "height") {
        return None;
    }
    // Context: inline `let ctx = ...` or assignment to one predeclared local.
    let (context, next) = match statements.get(index)? {
        Statement::VariableDeclaration(declaration) => {
            if !plain_variable_kind(declaration.kind) {
                return None;
            }
            let [variable] = declaration.declarations.as_slice() else {
                return None;
            };
            let context = plain_binding(model, &variable.id)?;
            if get_context(model, variable.init.as_ref()?, canvas).is_none() {
                return None;
            }
            (context, index + 1)
        }
        Statement::ExpressionStatement(statement) => {
            let Expression::AssignmentExpression(assignment) = &statement.expression else {
                return None;
            };
            let context = match &assignment.left {
                AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                    crate::analysis::identifier_symbol(identifier, model.scoping)?
                }
                _ => return None,
            };
            if assignment.operator.as_str() != "="
                || !predeclared.contains(&context)
                || get_context(model, &assignment.right, canvas).is_none()
            {
                return None;
            }
            (context, index + 1)
        }
        _ => return None,
    };
    index = next;
    // Image: inline `let image = new Image(5, 5)` or assignment to a
    // predeclared local.
    let (image, next) = match statements.get(index)? {
        Statement::VariableDeclaration(declaration) => {
            if !plain_variable_kind(declaration.kind) {
                return None;
            }
            let [variable] = declaration.declarations.as_slice() else {
                return None;
            };
            let image = plain_binding(model, &variable.id)?;
            image_constructor(model, site, variable.init.as_ref()?)?;
            (image, index + 1)
        }
        Statement::ExpressionStatement(statement) => {
            let Expression::AssignmentExpression(assignment) = &statement.expression else {
                return None;
            };
            let image = match &assignment.left {
                AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                    crate::analysis::identifier_symbol(identifier, model.scoping)?
                }
                _ => return None,
            };
            if assignment.operator.as_str() != "="
                || !predeclared.contains(&image)
                || image == context
                || image_constructor(model, site, &assignment.right).is_none()
            {
                return None;
            }
            (image, index + 1)
        }
        _ => return None,
    };
    index = next;
    if image == canvas || image == context || context == canvas {
        return None;
    }
    // `image.src = source;`
    let Statement::ExpressionStatement(src) = statements.get(index)? else {
        return None;
    };
    index += 1;
    let Expression::AssignmentExpression(src) = &src.expression else {
        return None;
    };
    if src.operator.as_str() != "="
        || target_member(&src.left, "src").is_none_or(|object| !is_symbol(model, object, image))
        || !is_symbol(model, &src.right, *source)
    {
        return None;
    }
    // `image.onload = callback;`
    let Statement::ExpressionStatement(onload) = statements.get(index)? else {
        return None;
    };
    index += 1;
    let Expression::AssignmentExpression(onload) = &onload.expression else {
        return None;
    };
    if onload.operator.as_str() != "="
        || target_member(&onload.left, "onload")
            .is_none_or(|object| !is_symbol(model, object, image))
        || !is_symbol(model, &onload.right, callback_symbol)
    {
        return None;
    }
    if index != statements.len() {
        return None;
    }
    // The predeclared locals are exactly the context/image locals that were
    // assigned later; any other predeclared local is unexplained.
    let expected: Vec<Symbol> = [context, image]
        .into_iter()
        .filter(|local| predeclared.contains(local))
        .collect();
    if predeclared.len() != expected.len() {
        return None;
    }
    // The owned callback must prove the pixel read over the same objects and
    // the same outer config.
    let read = image_load(model, callback_site, callback_function)?;
    if read.context != context || read.image != image || read.config != config {
        return None;
    }
    Some(Setup {
        callback: callback_site,
    })
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
    if probe_setup(model, site, function).is_some() {
        // The outer probe only claims the setup when its nested callback is
        // exactly the independently proven pixel-hash handler.
        return vec![proposal(
            STARTER,
            EVALUATOR,
            "owned 5x5 canvas image probe setup: creates the canvas through a \
             captured document receiver, fixes width/height to 5, takes a \
             willReadFrequently 2d context, constructs the 5x5 Image through a \
             captured constructor, stores the first formal in image.src and \
             installs the proved nested pixel-hash callback as image.onload",
            f,
        )];
    }
    // The owned on-load callback is claimed only when an enclosing function is
    // the same proved canvas probe and installs this exact function.
    let Some(owner) = model.enclosing_function(site.node_id) else {
        return vec![];
    };
    let AstKind::Function(owner_function) = model.nodes.kind(owner.node_id) else {
        return vec![];
    };
    let Some(setup) = probe_setup(model, owner, owner_function) else {
        return vec![];
    };
    if setup.callback.span != site.span {
        return vec![];
    }
    vec![proposal(
        HANDLER,
        EVALUATOR,
        "owned image.onload callback of the proved 5x5 canvas image probe: \
         draws the setup image onto the setup context, reads \
         getImageData(0, 0, 5, 5).data, hands each 4-byte pixel slice to the \
         captured collector with a field of the setup config, and emits four \
         random bytes through the captured observer once the shared load \
         counter reaches four",
        f,
    )]
}
