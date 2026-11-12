//! Bounded owned-body proof for the signal-codec number writer.
//!
//! The family is the four-parameter helper that encodes one JavaScript number
//! into a caller-supplied byte buffer.  Integer values use the sign/magnitude
//! varint ladder (a tag byte plus 1..5 continuation bytes); non-int32 values
//! write the `128` marker and then eight IEEE-754 little-endian bytes.  Every
//! stored byte is XORed with a zero-argument keystream callback.
//!
//! The proof is a direct dataflow check over the function's own body:
//!
//! * exactly four plain formals, no rest/initializer/directive, not async or a
//!   generator, and no nested function or arrow owned by the site;
//! * the value formal is tested by `(v | 0) === v`;
//! * a paired sign branch assigns `64`/`0` to the sign-code local and `-v`/`v`
//!   to the magnitude local (either assignment order);
//! * one `if`/`else if` ladder compares that magnitude against exactly `32`,
//!   `4096`, `524288`, `67108864` and stores 1..5 bytes whose payloads are the
//!   tag, `v >> 5`, `v >>> 12`, `v >>> 19`, `v >>> 26` with the
//!   `& 31 | 32` and `& 127 | 128` continuation masks;
//! * the alternate branch stores the `128` marker, builds eight bytes from the
//!   `Math.pow(2, 52)` / `Math.pow(2, -1074)` / `Math.log` / `Math.LN2` double
//!   conversion, pushes them through the array itself (directly in
//!   `bytes.push(...)`, or through an extracted `[bytes, bytes.push]` pair),
//!   then loops exactly eight times storing `bytes[i] ^ mask()`;
//! * the position returned by the function is a local declared in the body and
//!   is seeded through `Number(positionFormal)`.
//!
//! The same bounded proof also accepts the mixed continuation-index spelling:
//! a tier may advance a second body-local position with
//! `position = Number(previous) + 1` and then store `buffer[position]`, next to
//! the prefix `buffer[++position]` form, and either sign flag spelling (`let`
//! or `const`) is accepted because the flag is never reassigned.  The alternate
//! branch's eight-byte array may likewise be produced through the extracted
//! `[array, array.push]` pair in its declarator or its plain assignment
//! spelling, as long as the pair aliases the array the final loop reads.
//!
//! A third spelling carries the continuation cursor and the output array
//! through a two-element tuple snapshot.  After a tier's first store,
//! `holder = [cursor, buffer]` runs with the right side evaluated *before* the
//! assignment overwrites the holder, so `holder[0]` is the pre-overwrite
//! cursor; `next = holder[0] + 1` and `holder[1][next] = payload ^ mask()` are
//! the owned use.  The normalization recovers that same logical cursor and
//! output array locally and rejects a swapped tuple, a foreign array, a cursor
//! read from the wrong field, a reassigned holder, a misordered read, or an
//! intervening write.
//!
//! Every identifier comparison resolves through the existing scope index.  The
//! buffer may be read through a plain local that only ever holds formal slot 2
//! (`Model::site_slot_value`), while the mask must be the formal slot 0 itself.
//! Nothing here matches names, spans, hashes, vendors or labels, and the older
//! classifiers are not modified: this rule runs only after every earlier stage
//! has declined the function.

use super::*;
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UnaryOperator};

type Symbol = SymbolId;

const NAME: &str = "writeNumber";
const EVALUATOR: &str = "thread_posthost_numeric";

fn symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(expression.without_parentheses(), model.scoping)
}

fn is_symbol(model: &Model<'_, '_>, expression: &Expression<'_>, expected: Symbol) -> bool {
    symbol(model, expression) == Some(expected)
}

fn numeric(expression: &Expression<'_>) -> Option<f64> {
    match expression.without_parentheses() {
        Expression::NumericLiteral(literal) => Some(literal.value),
        Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::UnaryNegation => {
            numeric(&unary.argument).map(|value| -value)
        }
        _ => None,
    }
}

fn is_numeric(expression: &Expression<'_>, expected: f64) -> bool {
    numeric(expression) == Some(expected)
}

fn block<'a>(statement: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(block) = statement else {
        return None;
    };
    Some(block.body.as_slice())
}

fn declared_in_body(model: &Model<'_, '_>, site: FunctionSite, value: Symbol) -> bool {
    let body = site.body_span.unwrap_or(site.span);
    let span = model.scoping.symbol_span(value);
    span.start >= body.start && span.end <= body.end
}

/// The mask callback is the formal at slot 0, called with no arguments.
fn is_mask_call(model: &Model<'_, '_>, site: FunctionSite, expression: &Expression<'_>) -> bool {
    let Expression::CallExpression(call) = expression.without_parentheses() else {
        return false;
    };
    !call.optional
        && call.arguments.is_empty()
        && model.site_parameter_slot(&call.callee, site) == Some(0)
}

/// A plain `target = value` assignment whose left side is `slot[index]`.
fn buffer_write_payload<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &'a Expression<'a>,
) -> Option<&'a Expression<'a>> {
    let Expression::AssignmentExpression(assignment) = expression else {
        return None;
    };
    if assignment.operator != AssignmentOperator::Assign {
        return None;
    }
    let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left else {
        return None;
    };
    if model.site_slot_value(&member.object, site) != Some(2) {
        return None;
    }
    let Expression::BinaryExpression(xor) = assignment.right.without_parentheses() else {
        return None;
    };
    if xor.operator != BinaryOperator::BitwiseXOR {
        return None;
    }
    if is_mask_call(model, site, &xor.left) {
        return Some(&xor.right);
    }
    if is_mask_call(model, site, &xor.right) {
        return Some(&xor.left);
    }
    None
}

fn statement_buffer_payload<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &'a Statement<'a>,
) -> Option<&'a Expression<'a>> {
    let Statement::ExpressionStatement(statement) = statement else {
        return None;
    };
    buffer_write_payload(model, site, &statement.expression)
}

fn branch_payloads<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &'a [Statement<'a>],
) -> Vec<&'a Expression<'a>> {
    let mut payloads: Vec<&'a Expression<'a>> = Vec::new();
    let mut index = 0;
    while index < body.len() {
        let statement: &'a Statement<'a> = &body[index];
        if let Some(payload) = statement_buffer_payload(model, site, statement) {
            payloads.push(payload);
            index += 1;
            continue;
        }
        // A tuple-snapshot continuation store: `holder = [cursor, buffer]`,
        // then `next = holder[0] + 1`, then `holder[1][next] = payload ^ mask()`.
        // The three statements must be contiguous and in that order, so a read
        // before the snapshot or any intervening write cannot be normalized.
        if index + 2 < body.len() {
            if let Some(holder) = tuple_snapshot(model, site, statement) {
                if let Some(cursor) = tuple_cursor_step(model, site, holder, &body[index + 1]) {
                    if let Some(payload) =
                        tuple_write_payload(model, site, holder, cursor, &body[index + 2])
                    {
                        if tuple_assignment_count(model, body, holder) == 1
                            && holder_not_reassigned(model, body, index + 2, holder)
                        {
                            payloads.push(payload);
                            index += 3;
                            continue;
                        }
                    }
                }
            }
        }
        index += 1;
    }
    payloads
}

fn simple_assignment<'a>(
    model: &Model<'_, '_>,
    statement: &'a Statement<'a>,
) -> Option<(Symbol, &'a Expression<'a>)> {
    let Statement::ExpressionStatement(statement) = statement else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = &statement.expression else {
        return None;
    };
    if assignment.operator != AssignmentOperator::Assign {
        return None;
    }
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
        return None;
    };
    let value = crate::analysis::identifier_symbol(target, model.scoping)?;
    Some((value, &assignment.right))
}

/// A two-element `[cursor, outputArray]` tuple snapshot stored in `holder`.
///
/// The right-hand side is evaluated before the assignment overwrites the
/// holder, so `holder[0]` is the cursor value read *before* the overwrite.
/// The snapshot must be a plain `=` assignment to a body-local symbol, its
/// first element must be a body-local cursor, and its second element must be
/// the same logical output array the branch stores into (formal slot 2, never
/// a foreign array that merely looks like one).  Returns the holder symbol.
fn tuple_snapshot<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &'a Statement<'a>,
) -> Option<Symbol> {
    let (holder, assigned) = simple_assignment(model, statement)?;
    if !declared_in_body(model, site, holder) {
        return None;
    }
    let Expression::ArrayExpression(array) = assigned.without_parentheses() else {
        return None;
    };
    let [first, second] = array.elements.as_slice() else {
        return None;
    };
    let (Some(first), Some(second)) = (first.as_expression(), second.as_expression()) else {
        return None;
    };
    let cursor = symbol(model, first)?;
    let buffer = symbol(model, second)?;
    if !declared_in_body(model, site, cursor)
        || cursor == buffer
        || buffer == holder
        || model.site_slot_value(second, site) != Some(2)
    {
        return None;
    }
    Some(holder)
}

/// `next = holder[0] + 1`: the next cursor is read out of the snapshot's first
/// field, so the output array (field 1) can never masquerade as the cursor.
/// The assigned symbol must be a distinct body-local.
fn tuple_cursor_step<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    holder: Symbol,
    statement: &'a Statement<'a>,
) -> Option<Symbol> {
    let (target, assigned) = simple_assignment(model, statement).or_else(|| {
        let variable = one_variable(statement)?;
        Some((plain_local(model, variable)?, variable.init.as_ref()?))
    })?;
    if !declared_in_body(model, site, target) || target == holder {
        return None;
    }
    let Expression::BinaryExpression(binary) = assigned.without_parentheses() else {
        return None;
    };
    if binary.operator != BinaryOperator::Addition {
        return None;
    }
    let member = if is_numeric(&binary.right, 1.0) {
        binary.left.without_parentheses()
    } else if is_numeric(&binary.left, 1.0) {
        binary.right.without_parentheses()
    } else {
        return None;
    };
    let Expression::ComputedMemberExpression(member) = member else {
        return None;
    };
    if symbol(model, &member.object)? != holder || !is_numeric(&member.expression, 0.0) {
        return None;
    }
    Some(target)
}

/// The owned use `holder[1][next] = payload ^ mask()`: the write receiver is the
/// snapshot's second field (the recovered output array), the index is exactly
/// the recovered `next` cursor, and the payload is still XORed with the slot-0
/// keystream callback.  A write through field 0, another index, or a foreign
/// receiver is refused.
fn tuple_write_payload<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    holder: Symbol,
    cursor: Symbol,
    statement: &'a Statement<'a>,
) -> Option<&'a Expression<'a>> {
    let Statement::ExpressionStatement(statement) = statement else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = &statement.expression else {
        return None;
    };
    if assignment.operator != AssignmentOperator::Assign {
        return None;
    }
    let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left else {
        return None;
    };
    let Expression::ComputedMemberExpression(inner) = member.object.without_parentheses() else {
        return None;
    };
    if symbol(model, &inner.object)? != holder || !is_numeric(&inner.expression, 1.0) {
        return None;
    }
    if symbol(model, &member.expression)? != cursor {
        return None;
    }
    let Expression::BinaryExpression(xor) = assignment.right.without_parentheses() else {
        return None;
    };
    if xor.operator != BinaryOperator::BitwiseXOR {
        return None;
    }
    if is_mask_call(model, site, &xor.left) {
        return Some(&xor.right);
    }
    if is_mask_call(model, site, &xor.right) {
        return Some(&xor.left);
    }
    None
}

/// Count the plain assignments in this branch that store an array literal into
/// `holder`.  A well-formed snapshot is the only such assignment; a decoy or a
/// trailing reassignment makes the count larger and is refused.
fn tuple_assignment_count<'a>(
    model: &Model<'_, '_>,
    body: &'a [Statement<'a>],
    holder: Symbol,
) -> usize {
    body.iter()
        .filter(|&statement| {
            simple_assignment(model, statement).is_some_and(|(target, assigned)| {
                target == holder
                    && matches!(
                        assigned.without_parentheses(),
                        Expression::ArrayExpression(_)
                    )
            })
        })
        .count()
}

/// No statement after the normalized write may assign the holder again.
fn holder_not_reassigned<'a>(
    model: &Model<'_, '_>,
    body: &'a [Statement<'a>],
    write_index: usize,
    holder: Symbol,
) -> bool {
    body[write_index + 1..].iter().all(|statement| {
        simple_assignment(model, statement).is_none_or(|(target, _)| target != holder)
    })
}

fn is_negated(model: &Model<'_, '_>, expression: &Expression<'_>, value: Symbol) -> bool {
    let Expression::UnaryExpression(unary) = expression.without_parentheses() else {
        return false;
    };
    unary.operator == UnaryOperator::UnaryNegation && is_symbol(model, &unary.argument, value)
}

/// `(v | 0) === v` (or the mirrored comparison), with `v` the value formal.
fn is_integer_test(model: &Model<'_, '_>, expression: &Expression<'_>, value: Symbol) -> bool {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return false;
    };
    if binary.operator != BinaryOperator::StrictEquality {
        return false;
    }
    let truncated = |side: &Expression<'_>| {
        let Expression::BinaryExpression(or) = side.without_parentheses() else {
            return false;
        };
        or.operator == BinaryOperator::BitwiseOR
            && ((is_symbol(model, &or.left, value) && is_numeric(&or.right, 0.0))
                || (is_symbol(model, &or.right, value) && is_numeric(&or.left, 0.0)))
    };
    (truncated(&binary.left) && is_symbol(model, &binary.right, value))
        || (is_symbol(model, &binary.left, value) && truncated(&binary.right))
}

/// `magnitude < threshold` (or `threshold > magnitude`); returns the magnitude
/// local.
fn threshold_test(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    threshold: f64,
) -> Option<Symbol> {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return None;
    };
    match binary.operator {
        BinaryOperator::LessThan => {
            if !is_numeric(&binary.right, threshold) {
                return None;
            }
            symbol(model, &binary.left)
        }
        BinaryOperator::GreaterThan => {
            if !is_numeric(&binary.left, threshold) {
                return None;
            }
            symbol(model, &binary.right)
        }
        _ => None,
    }
}

/// Collect the five integer branches and the shared magnitude local.
fn threshold_branches<'a>(
    model: &Model<'_, '_>,
    first: &'a IfStatement<'a>,
) -> Option<(Symbol, Vec<&'a [Statement<'a>]>)> {
    const THRESHOLDS: [f64; 4] = [32.0, 4096.0, 524_288.0, 67_108_864.0];
    let mut branches = Vec::with_capacity(5);
    let mut magnitude = None;
    let mut current = first;
    for (index, threshold) in THRESHOLDS.iter().enumerate() {
        let found = threshold_test(model, &current.test, *threshold)?;
        if magnitude.is_some_and(|existing| existing != found) {
            return None;
        }
        magnitude = Some(found);
        let Statement::BlockStatement(block) = &current.consequent else {
            return None;
        };
        branches.push(block.body.as_slice());
        if index + 1 < THRESHOLDS.len() {
            let Statement::IfStatement(next) = current.alternate.as_ref()? else {
                return None;
            };
            current = next;
        } else {
            let Statement::BlockStatement(block) = current.alternate.as_ref()? else {
                return None;
            };
            branches.push(block.body.as_slice());
        }
    }
    Some((magnitude?, branches))
}

/// Find the sign/magnitude pair: one branch assigns `64` to the sign code and
/// `-value` to the magnitude, the alternate assigns `0`/`value` to the same
/// symbols.  The two assignments may appear in either order.
fn sign_pair(
    model: &Model<'_, '_>,
    value: Symbol,
    magnitude: Symbol,
    body: &[Statement<'_>],
) -> Option<Symbol> {
    for statement in body {
        let Statement::IfStatement(branch) = statement else {
            continue;
        };
        let Some(consequent) = block(&branch.consequent) else {
            continue;
        };
        let Some(alternate) = branch.alternate.as_ref().and_then(block) else {
            continue;
        };
        let mut sign = None;
        let mut negative_magnitude = None;
        for statement in consequent {
            let Some((target, assigned)) = simple_assignment(model, statement) else {
                continue;
            };
            if is_numeric(assigned, 64.0) {
                sign = Some(target);
            }
            if is_negated(model, assigned, value) {
                negative_magnitude = Some(target);
            }
        }
        let Some(sign) = sign else {
            continue;
        };
        if negative_magnitude != Some(magnitude) || sign == magnitude || sign == value {
            continue;
        }
        let mut sign_zero = false;
        let mut magnitude_value = false;
        for statement in alternate {
            let Some((target, assigned)) = simple_assignment(model, statement) else {
                continue;
            };
            if target == sign && is_numeric(assigned, 0.0) {
                sign_zero = true;
            }
            if target == magnitude && is_symbol(model, assigned, value) {
                magnitude_value = true;
            }
        }
        if sign_zero && magnitude_value {
            return Some(sign);
        }
    }
    None
}

/// First varint byte: `sign | magnitude` for the one-byte branch, otherwise
/// `sign | (magnitude & 31 | 32)`.
fn is_tag_payload(
    model: &Model<'_, '_>,
    payload: &Expression<'_>,
    sign: Symbol,
    magnitude: Symbol,
    extended: bool,
) -> bool {
    let Expression::BinaryExpression(or) = payload.without_parentheses() else {
        return false;
    };
    if or.operator != BinaryOperator::BitwiseOR {
        return false;
    }
    let mut sign_seen = false;
    let mut magnitude_side = None;
    for side in [&or.left, &or.right] {
        if is_symbol(model, side, sign) {
            if sign_seen {
                return false;
            }
            sign_seen = true;
        } else {
            magnitude_side = Some(side);
        }
    }
    let (true, Some(magnitude_side)) = (sign_seen, magnitude_side) else {
        return false;
    };
    if !extended {
        return is_symbol(model, magnitude_side, magnitude);
    }
    let Expression::BinaryExpression(or) = magnitude_side.without_parentheses() else {
        return false;
    };
    if or.operator != BinaryOperator::BitwiseOR {
        return false;
    }
    let masked = if is_numeric(&or.right, 32.0) {
        &or.left
    } else if is_numeric(&or.left, 32.0) {
        &or.right
    } else {
        return false;
    };
    let Expression::BinaryExpression(and) = masked.without_parentheses() else {
        return false;
    };
    and.operator == BinaryOperator::BitwiseAnd
        && ((is_symbol(model, &and.left, magnitude) && is_numeric(&and.right, 31.0))
            || (is_symbol(model, &and.right, magnitude) && is_numeric(&and.left, 31.0)))
}

fn is_shift_of(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    magnitude: Symbol,
    amount: f64,
) -> bool {
    let Expression::BinaryExpression(shift) = expression.without_parentheses() else {
        return false;
    };
    matches!(
        shift.operator,
        BinaryOperator::ShiftRight | BinaryOperator::ShiftRightZeroFill
    ) && is_symbol(model, &shift.left, magnitude)
        && is_numeric(&shift.right, amount)
}

/// Continuation byte: `magnitude >> amount`, optionally wrapped in
/// `(value & 127) | 128`.
fn is_shift_payload(
    model: &Model<'_, '_>,
    payload: &Expression<'_>,
    magnitude: Symbol,
    amount: f64,
    masked: bool,
) -> bool {
    if !masked {
        return is_shift_of(model, payload, magnitude, amount);
    }
    let Expression::BinaryExpression(or) = payload.without_parentheses() else {
        return false;
    };
    if or.operator != BinaryOperator::BitwiseOR {
        return false;
    }
    let inner = if is_numeric(&or.right, 128.0) {
        &or.left
    } else if is_numeric(&or.left, 128.0) {
        &or.right
    } else {
        return false;
    };
    let Expression::BinaryExpression(and) = inner.without_parentheses() else {
        return false;
    };
    if and.operator != BinaryOperator::BitwiseAnd {
        return false;
    }
    let shifted = if is_numeric(&and.right, 127.0) {
        &and.left
    } else if is_numeric(&and.left, 127.0) {
        &and.right
    } else {
        return false;
    };
    is_shift_of(model, shifted, magnitude, amount)
}

fn valid_integer_branches(
    model: &Model<'_, '_>,
    site: FunctionSite,
    branches: &[&[Statement<'_>]],
    sign: Symbol,
    magnitude: Symbol,
) -> bool {
    if branches.len() != 5 {
        return false;
    }
    for (index, body) in branches.iter().enumerate() {
        let payloads = branch_payloads(model, site, body);
        if payloads.len() != index + 1 {
            return false;
        }
        for (byte, payload) in payloads.iter().enumerate() {
            let matches = match byte {
                0 => is_tag_payload(model, payload, sign, magnitude, index > 0),
                1 => is_shift_payload(model, payload, magnitude, 5.0, index > 1),
                2 => is_shift_payload(model, payload, magnitude, 12.0, index > 2),
                3 => is_shift_payload(model, payload, magnitude, 19.0, index > 3),
                4 => is_shift_payload(model, payload, magnitude, 26.0, false),
                _ => false,
            };
            if !matches {
                return false;
            }
        }
    }
    true
}

fn is_math_global(model: &Model<'_, '_>, expression: &Expression<'_>) -> bool {
    matches!(
        expression.without_parentheses(),
        Expression::Identifier(identifier)
            if identifier.name == "Math"
                && crate::analysis::identifier_symbol(identifier, model.scoping).is_none()
    )
}

fn is_math_method(model: &Model<'_, '_>, callee: &Expression<'_>, method: &str) -> bool {
    let Expression::StaticMemberExpression(member) = callee.without_parentheses() else {
        return false;
    };
    !member.optional && member.property.name == method && is_math_global(model, &member.object)
}

fn is_pow_two(model: &Model<'_, '_>, call: &CallExpression<'_>, exponent: f64) -> bool {
    is_math_method(model, &call.callee, "pow") && !call.optional && call.arguments.len() == 2 && {
        let arguments = call_arguments(call);
        numeric(arguments[0]) == Some(2.0) && numeric(arguments[1]) == Some(exponent)
    }
}

fn has_math_call(facts: &Facts<'_>, model: &Model<'_, '_>, method: &str) -> bool {
    facts
        .calls
        .iter()
        .any(|call| !call.optional && is_math_method(model, &call.callee, method))
}

fn is_global_call(model: &Model<'_, '_>, call: &CallExpression<'_>, name: &str) -> bool {
    matches!(
        call.callee.without_parentheses(),
        Expression::Identifier(identifier)
            if identifier.name == name
                && crate::analysis::identifier_symbol(identifier, model.scoping).is_none()
    )
}

fn is_number_call_on_position(model: &Model<'_, '_>, facts: &Facts<'_>, position: Symbol) -> bool {
    facts.calls.iter().any(|call| {
        !call.optional
            && is_global_call(model, call, "Number")
            && call.arguments.len() == 1
            && call_arguments(call)
                .first()
                .is_some_and(|argument| is_symbol(model, argument, position))
    })
}

/// `bytes[index]` where `bytes` is a body-local and `index` is the loop index.
fn indexed_bytes(
    model: &Model<'_, '_>,
    site: FunctionSite,
    payload: &Expression<'_>,
    index: Symbol,
) -> Option<Symbol> {
    let Expression::ComputedMemberExpression(member) = payload.without_parentheses() else {
        return None;
    };
    let bytes = symbol(model, &member.object)?;
    (declared_in_body(model, site, bytes) && symbol(model, &member.expression) == Some(index))
        .then_some(bytes)
}

/// The loop index of `index < 8` (or `8 > index`).
fn loop_index(model: &Model<'_, '_>, test: &Expression<'_>) -> Option<Symbol> {
    let Expression::BinaryExpression(binary) = test.without_parentheses() else {
        return None;
    };
    match binary.operator {
        BinaryOperator::LessThan => {
            if !is_numeric(&binary.right, 8.0) {
                return None;
            }
            symbol(model, &binary.left)
        }
        BinaryOperator::GreaterThan => {
            if !is_numeric(&binary.left, 8.0) {
                return None;
            }
            symbol(model, &binary.right)
        }
        _ => None,
    }
}

/// The final eight-iteration loop stores `bytes[index] ^ mask()` into the
/// buffer.  Both `for` and `while` spellings are accepted; the loop body may
/// also contain the index step when it is not part of the loop header.  The
/// returned symbol is the body-local array those eight bytes are read from.
fn final_byte_array(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
) -> Option<Symbol> {
    let (test, body) = match statement {
        Statement::ForStatement(statement) => {
            let test = statement.test.as_ref()?;
            (test, &statement.body)
        }
        Statement::WhileStatement(statement) => (&statement.test, &statement.body),
        _ => return None,
    };
    let index = loop_index(model, test)?;
    let body = block(body)?;
    let mut bytes = None;
    for statement in body {
        let Some(payload) = statement_buffer_payload(model, site, statement) else {
            continue;
        };
        if bytes.is_some() {
            return None;
        }
        bytes = Some(indexed_bytes(model, site, payload, index)?);
    }
    bytes
}

fn valid_float_branch(
    model: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
    body: &[Statement<'_>],
) -> bool {
    let mut marker = None;
    let mut final_loop = None;
    let mut bytes = None;
    for statement in body {
        if let Some(payload) = statement_buffer_payload(model, site, statement) {
            if is_numeric(payload, 128.0) {
                marker = Some(payload);
            }
        }
        if let Some(found) = final_byte_array(model, site, statement) {
            final_loop = Some(statement);
            bytes = Some(found);
        }
    }
    let (Some(marker), Some(final_loop), Some(bytes)) = (marker, final_loop, bytes) else {
        return false;
    };
    if marker.span().start >= final_loop.span().start {
        return false;
    }
    if !facts.calls.iter().any(|call| is_pow_two(model, call, 52.0)) {
        return false;
    }
    if !facts
        .calls
        .iter()
        .any(|call| is_pow_two(model, call, -1074.0))
    {
        return false;
    }
    if !has_math_call(facts, model, "log") || !has_math_call(facts, model, "abs") {
        return false;
    }
    if !facts.members.contains("LN2") || !facts.members.contains("floor") {
        return false;
    }
    let Some((start, end)) = statements_span(body) else {
        return false;
    };
    push_builds_bytes(model, site, facts, start, end, bytes)
}

fn statements_span(body: &[Statement<'_>]) -> Option<(u32, u32)> {
    Some((body.first()?.span().start, body.last()?.span().end))
}

/// Follow body-local alias assignments from `from` toward `to`.  Every hop
/// stays inside the function body and the walk is bounded, so a host value or
/// an unrelated outer binding cannot satisfy the proof.
fn body_alias_reaches(model: &Model<'_, '_>, site: FunctionSite, from: Symbol, to: Symbol) -> bool {
    let mut current = from;
    for _ in 0..8 {
        if current == to {
            return true;
        }
        if !declared_in_body(model, site, current)
            || !model.scoping.symbol_redeclarations(current).is_empty()
        {
            return false;
        }
        let Some(value) = model.symbol_value(current) else {
            return false;
        };
        let Some(next) = symbol(model, value) else {
            return false;
        };
        current = next;
    }
    false
}

/// An extracted `[array, array.push]` pair where `array` is the array the final
/// loop reads, or a bounded body-local alias of it.  The pair is accepted in
/// both its declarator (`let pair = [array, array.push]`) and its plain
/// assignment (`pair = [array, array.push]`) spelling; the push receiver must be
/// the very symbol paired with `array.push`, and the final-loop array is reached
/// through the same bounded alias chain used for direct `.push` calls.
fn pair_builds_bytes(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &Expression<'_>,
    bytes: Symbol,
) -> bool {
    let Expression::ArrayExpression(array) = expression.without_parentheses() else {
        return false;
    };
    let [first, second] = array.elements.as_slice() else {
        return false;
    };
    let (Some(first), Some(second)) = (first.as_expression(), second.as_expression()) else {
        return false;
    };
    let Some(array) = symbol(model, first) else {
        return false;
    };
    let Expression::StaticMemberExpression(member) = second.without_parentheses() else {
        return false;
    };
    !member.optional
        && member.property.name == "push"
        && symbol(model, &member.object) == Some(array)
        && (body_alias_reaches(model, site, bytes, array)
            || body_alias_reaches(model, site, array, bytes))
}

/// The eight output bytes are produced by `bytes.push(...)`, or by an extracted
/// `[bytes, bytes.push]` pair, where `bytes` is the body-local array the final
/// loop reads.  The receiver is proven by scoped identity (with a short
/// body-local alias chain for the branching vendor spelling), never by a member
/// spelling alone.
fn push_builds_bytes(
    model: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
    start: u32,
    end: u32,
    bytes: Symbol,
) -> bool {
    for call in &facts.calls {
        if call.span.start < start || call.span.end > end {
            continue;
        }
        let Expression::StaticMemberExpression(member) = call.callee.without_parentheses() else {
            continue;
        };
        if member.optional || member.property.name != "push" {
            continue;
        }
        let Some(receiver) = symbol(model, &member.object) else {
            continue;
        };
        if body_alias_reaches(model, site, bytes, receiver)
            || body_alias_reaches(model, site, receiver, bytes)
        {
            return true;
        }
    }
    for node in model.nodes.iter() {
        if node.span().start < start || node.span().end > end {
            continue;
        }
        let init = match node.kind() {
            AstKind::VariableDeclarator(declarator) => declarator.init.as_ref(),
            AstKind::AssignmentExpression(assignment) => {
                (assignment.operator == AssignmentOperator::Assign).then_some(&assignment.right)
            }
            _ => None,
        };
        if init.is_some_and(|value| pair_builds_bytes(model, site, value, bytes)) {
            return true;
        }
    }
    false
}

fn valid_returns(
    model: &Model<'_, '_>,
    site: FunctionSite,
    declared: &[Symbol],
    top_return: Option<&ReturnStatement<'_>>,
    consequent: &[Statement<'_>],
    alternate: &[Statement<'_>],
    returns: &[&Expression<'_>],
) -> bool {
    match (top_return, returns.len()) {
        (Some(top), 1) => {
            let Some(result) = top.argument.as_ref().and_then(|value| symbol(model, value)) else {
                return false;
            };
            if !declared_in_body(model, site, result) {
                return false;
            }
            if !declared.is_empty() && !declared.contains(&result) {
                return false;
            }
            returns
                .iter()
                .all(|value| symbol(model, value) == Some(result))
        }
        (None, 2) => {
            let branch_symbol = |body: &[Statement<'_>]| {
                let Statement::ReturnStatement(statement) = body.last()? else {
                    return None;
                };
                let value = statement.argument.as_ref()?;
                let result = symbol(model, value)?;
                declared_in_body(model, site, result).then_some(result)
            };
            let Some(left) = branch_symbol(consequent) else {
                return false;
            };
            let Some(right) = branch_symbol(alternate) else {
                return false;
            };
            let mut returned = returns.iter().filter_map(|value| symbol(model, value));
            returned.next() == Some(left)
                && returned.next() == Some(right)
                && returned.next().is_none()
        }
        _ => false,
    }
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
    let Some(body) = function.body.as_ref() else {
        return vec![];
    };
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !body.directives.is_empty()
        || function.params.items.len() != 4
    {
        return vec![];
    }
    let mut formals = Vec::with_capacity(4);
    for parameter in &function.params.items {
        if parameter.initializer.is_some() {
            return vec![];
        }
        let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
            return vec![];
        };
        let Some(value) = identifier.symbol_id.get() else {
            return vec![];
        };
        if !model.scoping.symbol_redeclarations(value).is_empty()
            || model.binding_writes(value) != 0
        {
            return vec![];
        }
        formals.push(value);
    }
    let [mask, value, buffer, position] = formals.as_slice() else {
        return vec![];
    };
    let (mask, value, buffer, position) = (*mask, *value, *buffer, *position);

    // Nested callbacks are never silently absorbed into this proof.
    let body_span = site.body_span.unwrap_or(site.span);
    if model.all_sites().iter().any(|other| {
        other.span != site.span
            && other.span.start >= body_span.start
            && other.span.end <= body_span.end
            && other.span.start >= site.span.start
            && other.span.end <= site.span.end
    }) {
        return vec![];
    }

    let statements = body.statements.as_slice();
    let (declared, integer_if, top_return) = match statements {
        [Statement::VariableDeclaration(declaration), Statement::IfStatement(integer_if)] => {
            (Some(declaration), integer_if, None)
        }
        [Statement::VariableDeclaration(declaration), Statement::IfStatement(integer_if), Statement::ReturnStatement(top)] => {
            (Some(declaration), integer_if, Some(top))
        }
        [Statement::IfStatement(integer_if)] => (None, integer_if, None),
        _ => return vec![],
    };
    let declared_symbols: Vec<Symbol> = match declared {
        Some(declaration) => {
            let mut symbols = Vec::with_capacity(declaration.declarations.len());
            for declarator in &declaration.declarations {
                if declarator.init.is_some() {
                    return vec![];
                }
                let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                    return vec![];
                };
                let Some(symbol) = identifier.symbol_id.get() else {
                    return vec![];
                };
                if !declared_in_body(model, site, symbol) {
                    return vec![];
                }
                symbols.push(symbol);
            }
            if symbols.is_empty() {
                return vec![];
            }
            symbols
        }
        None => Vec::new(),
    };
    if !is_integer_test(model, &integer_if.test, value) {
        return vec![];
    }
    let Some(consequent) = block(&integer_if.consequent) else {
        return vec![];
    };
    let Some(alternate) = integer_if.alternate.as_ref().and_then(block) else {
        return vec![];
    };
    let Some(threshold) = consequent.iter().find_map(|statement| {
        let Statement::IfStatement(candidate) = statement else {
            return None;
        };
        threshold_test(model, &candidate.test, 32.0).map(|_| candidate)
    }) else {
        return vec![];
    };
    let Some((magnitude, branches)) = threshold_branches(model, threshold) else {
        return vec![];
    };
    if magnitude == value || magnitude == mask || magnitude == buffer || magnitude == position {
        return vec![];
    }
    let Some(sign) = sign_pair(model, value, magnitude, consequent) else {
        return vec![];
    };
    if !valid_integer_branches(model, site, &branches, sign, magnitude) {
        return vec![];
    }
    if !is_number_call_on_position(model, f, position) {
        return vec![];
    }
    if !valid_float_branch(model, site, f, alternate) {
        return vec![];
    }
    if !valid_returns(
        model,
        site,
        &declared_symbols,
        top_return.map(|value| &**value),
        consequent,
        alternate,
        &f.returns,
    ) {
        return vec![];
    }

    vec![proposal(
        NAME,
        EVALUATOR,
        "owned-body proof of the signal codec number writer: the value formal's \
         `(v|0)===v` integer test selects a sign/magnitude varint ladder whose \
         four thresholds (32, 4096, 524288, 67108864) store 1..5 bytes with the \
         `with &31|32`/`&127|128` continuation masks and `>>5`/`>>>12`/`>>>19`/`>>>26` \
         shifts, while the alternate branch stores the 128 marker and eight \
         IEEE-754 bytes built through `Math.pow(2,52)`/`Math.pow(2,-1074)`/`Math.log`; \
         every stored byte is XORed with the slot-0 keystream callback and the \
         returned position is a body-local seeded from `Number(slot3)`; a \
         continuation cursor and output array carried through a two-element \
         `[cursor, buffer]` tuple snapshot are normalized back to the same \
         logical cursor and buffer at their owned `holder[1][next]` store",
        f,
    )]
}
