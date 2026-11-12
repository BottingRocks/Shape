//! Keystroke timing sample recorders.
//!
//! Two complete owned bodies are recognized on their own proof:
//!
//! * keydown: read `Date.now()`, store it in a captured per-keyCode map, seed
//!   a captured first-timestamp slot once, then accumulate the inter-key gap
//!   below 2000 ms into a captured running total and otherwise into a captured
//!   overflow total.
//! * keyup: read the captured per-keyCode press timestamp, increment a captured
//!   sample counter, fold the hold duration into a captured running mean and
//!   second moment, store the release clock, count the keyCode when it is
//!   present in a captured tracked-key list, clear the stored timestamp and set
//!   the captured active flag.
//!
//! Every participant is scope-resolved. Captured bindings are plain
//! `var`/`let`/`const` declarations outside the owned body and before their
//! use; locals are plain declarations of the owned body. No vendor, thread id,
//! KEY spelling, source span or body hash participates, and the arithmetic
//! order and literals are matched exactly.
use super::*;
use crate::naming::shared::{one_variable, plain_local};
use crate::naming::support::seeded_flow::*;
use oxc_span::GetSpan;
use oxc_syntax::operator::{BinaryOperator, UnaryOperator};

fn one_formal(model: &Model<'_, '_>, params: &FormalParameters<'_>) -> Option<SymbolId> {
    if params.rest.is_some() {
        return None;
    }
    let [p] = params.items.as_slice() else {
        return None;
    };
    if p.initializer.is_some() {
        return None;
    }
    let BindingPattern::BindingIdentifier(id) = &p.pattern else {
        return None;
    };
    let s = id.symbol_id.get()?;
    unchanged(model, s).then_some(s)
}

fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    match s {
        Statement::BlockStatement(b) => Some(b.body.as_slice()),
        _ => None,
    }
}

/// `Date.now()` with an unshadowed ambient root.
fn date_now(model: &Model<'_, '_>, e: &Expression<'_>) -> bool {
    let Some(c) = call(e.without_parentheses()) else {
        return false;
    };
    arity(c, 0).is_some() && is_global(model, &c.callee, &["Date", "now"])
}

/// `event.keyCode`.
fn event_keycode(model: &Model<'_, '_>, e: &Expression<'_>, event: SymbolId) -> bool {
    member(e.without_parentheses(), "keyCode").is_some_and(|o| symbol(model, o) == Some(event))
}

fn captured(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<SymbolId> {
    data_capture(model, site, e.without_parentheses())
}

fn local_initialized<'a>(
    model: &Model<'_, '_>,
    s: &'a Statement<'a>,
) -> Option<(SymbolId, &'a Expression<'a>)> {
    let v = one_variable(s)?;
    let local = plain_local(model, v)?;
    Some((local, v.init.as_ref()?))
}

fn distinct(symbols: &[SymbolId]) -> bool {
    for (i, a) in symbols.iter().enumerate() {
        if symbols[i + 1..].contains(a) {
            return false;
        }
    }
    true
}

/// A numeric literal, accepting the unary-negation form the parser keeps for a
/// negative literal.
fn signed_number(e: &Expression<'_>, expected: f64) -> bool {
    match e.without_parentheses() {
        Expression::NumericLiteral(n) => n.value == expected,
        Expression::UnaryExpression(u) if u.operator == UnaryOperator::UnaryNegation => {
            matches!(u.argument.without_parentheses(), Expression::NumericLiteral(n) if -n.value == expected)
        }
        _ => false,
    }
}

fn subtract_pair(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    left: SymbolId,
    right: SymbolId,
) -> bool {
    matches!(e.without_parentheses(), Expression::BinaryExpression(b)
        if b.operator == BinaryOperator::Subtraction
            && symbol(model, &b.left) == Some(left)
            && symbol(model, &b.right) == Some(right))
}

fn sum_pair(model: &Model<'_, '_>, e: &Expression<'_>, left: SymbolId, right: SymbolId) -> bool {
    matches!(e.without_parentheses(), Expression::BinaryExpression(b)
        if b.operator == BinaryOperator::Addition
            && symbol(model, &b.left) == Some(left)
            && symbol(model, &b.right) == Some(right))
}

/// `((clock - press) - delta)`.
fn hold_span(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    clock: SymbolId,
    press: SymbolId,
    delta: SymbolId,
) -> bool {
    let Expression::BinaryExpression(outer) = e.without_parentheses() else {
        return false;
    };
    outer.operator == BinaryOperator::Subtraction
        && subtract_pair(model, &outer.left, clock, press)
        && symbol(model, &outer.right) == Some(delta)
}

// ---------------------------------------------------------------------------
// keydown: store the press clock and accumulate the inter-key gap.
// ---------------------------------------------------------------------------
fn keydown(model: &Model<'_, '_>, site: FunctionSite, f: &Function<'_>) -> Option<()> {
    let event = one_formal(model, &f.params)?;
    let body = f.body.as_ref()?;
    let [clock_decl, store, branch] = body.statements.as_slice() else {
        return None;
    };
    let (clock, initial) = local_initialized(model, clock_decl)?;
    if model.binding_writes(clock) != 1 || !date_now(model, initial) {
        return None;
    }
    let store = assignment(store)?;
    let AssignmentTarget::ComputedMemberExpression(target) = &store.left else {
        return None;
    };
    let map = captured(model, site, &target.object)?;
    if symbol(model, &store.right) != Some(clock)
        || !event_keycode(model, &target.expression, event)
    {
        return None;
    }
    let Statement::IfStatement(branch) = branch else {
        return None;
    };
    let Expression::BinaryExpression(test) = branch.test.without_parentheses() else {
        return None;
    };
    if test.operator != BinaryOperator::StrictEquality {
        return None;
    }
    let base = captured(model, site, &test.left)?;
    if !number(test.right.without_parentheses(), 0.0) {
        return None;
    }
    let [then] = block(&branch.consequent)? else {
        return None;
    };
    let then = assignment(then)?;
    if assign_symbol(model, then) != Some(base) || symbol(model, &then.right) != Some(clock) {
        return None;
    }
    let [delta, inner] = block(branch.alternate.as_ref()?)? else {
        return None;
    };
    let delta = assignment(delta)?;
    if assign_symbol(model, delta) != Some(clock) {
        return None;
    }
    let Expression::ConditionalExpression(cond) = delta.right.without_parentheses() else {
        return None;
    };
    let previous = captured(model, site, &cond.test)?;
    if !subtract_pair(model, &cond.consequent, clock, previous)
        || !number(cond.alternate.without_parentheses(), 0.0)
    {
        return None;
    }
    let Statement::IfStatement(inner) = inner else {
        return None;
    };
    let Expression::BinaryExpression(inner_test) = inner.test.without_parentheses() else {
        return None;
    };
    if inner_test.operator != BinaryOperator::LessThan
        || symbol(model, &inner_test.left) != Some(clock)
        || !number(inner_test.right.without_parentheses(), 2000.0)
    {
        return None;
    }
    let [accumulate, count] = block(&inner.consequent)? else {
        return None;
    };
    let accumulate = assignment(accumulate)?;
    let accumulator = assign_symbol(model, accumulate)?;
    if !sum_pair(model, &accumulate.right, accumulator, clock) {
        return None;
    }
    let counter = update_captured(model, site, count, "++")?;
    let [overflow] = block(inner.alternate.as_ref()?)? else {
        return None;
    };
    let overflow = assignment(overflow)?;
    let overflow_slot = assign_symbol(model, overflow)?;
    if !sum_pair(model, &overflow.right, overflow_slot, clock) {
        return None;
    }
    distinct(&[map, base, previous, accumulator, counter, overflow_slot]).then_some(())
}

fn update_captured(
    model: &Model<'_, '_>,
    site: FunctionSite,
    s: &Statement<'_>,
    operator: &str,
) -> Option<SymbolId> {
    let target = update_symbol(model, expression(s)?, true, operator)?;
    captured_identity(model, site, target, s.span().start).then_some(target)
}

fn captured_identity(
    model: &Model<'_, '_>,
    site: FunctionSite,
    s: SymbolId,
    use_start: u32,
) -> bool {
    capture_identity(model, site, s, use_start)
}

// ---------------------------------------------------------------------------
// keyup: fold the hold duration into the running mean/second moment.
// ---------------------------------------------------------------------------
fn keyup(model: &Model<'_, '_>, site: FunctionSite, f: &Function<'_>) -> Option<()> {
    let event = one_formal(model, &f.params)?;
    let body = f.body.as_ref()?;
    let [clock_decl, press_decl, count_decl, branch, first, key_decl, lookup, clear, flag] =
        body.statements.as_slice()
    else {
        return None;
    };
    let (clock, initial) = local_initialized(model, clock_decl)?;
    if model.binding_writes(clock) != 0 || !date_now(model, initial) {
        return None;
    }
    // let press = MAP[event.keyCode]
    let (press, press_value) = local_initialized(model, press_decl)?;
    let Expression::ComputedMemberExpression(press_read) = press_value.without_parentheses() else {
        return None;
    };
    let map = captured(model, site, &press_read.object)?;
    if !event_keycode(model, &press_read.expression, event) {
        return None;
    }
    let samples = update_captured(model, site, count_decl, "++")?;
    // if (press) { ... }
    let Statement::IfStatement(branch) = branch else {
        return None;
    };
    if symbol(model, &branch.test) != Some(press) {
        return None;
    }
    let [mean_decl, mean_write, moment_write] = block(&branch.consequent)? else {
        return None;
    };
    let (delta, mean_value) = local_initialized(model, mean_decl)?;
    let mean = captured(model, site, mean_value)?;
    if model.binding_writes(delta) != 0 {
        return None;
    }
    // MEAN = MEAN + hold / SAMPLES
    let mean_write = assignment(mean_write)?;
    if assign_symbol(model, mean_write) != Some(mean) {
        return None;
    }
    let Expression::BinaryExpression(mean_sum) = mean_write.right.without_parentheses() else {
        return None;
    };
    let Expression::BinaryExpression(mean_div) = mean_sum.right.without_parentheses() else {
        return None;
    };
    if mean_sum.operator != BinaryOperator::Addition
        || symbol(model, &mean_sum.left) != Some(mean)
        || mean_div.operator != BinaryOperator::Division
        || !hold_span(model, &mean_div.left, clock, press, delta)
        || symbol(model, &mean_div.right) != Some(samples)
    {
        return None;
    }
    // M2 = M2 + hold * (t - press - MEAN)
    let moment_write = assignment(moment_write)?;
    let moment = assign_symbol(model, moment_write)?;
    let Expression::BinaryExpression(moment_sum) = moment_write.right.without_parentheses() else {
        return None;
    };
    let Expression::BinaryExpression(moment_mul) = moment_sum.right.without_parentheses() else {
        return None;
    };
    if moment_sum.operator != BinaryOperator::Addition
        || symbol(model, &moment_sum.left) != Some(moment)
        || moment_mul.operator != BinaryOperator::Multiplication
        || !hold_span(model, &moment_mul.left, clock, press, delta)
        || !hold_span(model, &moment_mul.right, clock, press, mean)
    {
        return None;
    }
    // FIRST = clock
    let first = assignment(first)?;
    let first_slot = assign_symbol(model, first)?;
    if symbol(model, &first.right) != Some(clock) {
        return None;
    }
    // let index = KEYS.indexOf(event.keyCode)
    let (index, lookup_init) = local_initialized(model, key_decl)?;
    let Some(lookup_call) = call(lookup_init.without_parentheses()) else {
        return None;
    };
    let Some(keys) = member(&lookup_call.callee, "indexOf") else {
        return None;
    };
    let keys = captured(model, site, keys)?;
    let args = arity(lookup_call, 1)?;
    if !event_keycode(model, args[0], event) {
        return None;
    }
    // if (index !== -1) { ++N; COATS[index] = Number(COATS[index]) + 1; }
    let Statement::IfStatement(lookup) = lookup else {
        return None;
    };
    let Expression::BinaryExpression(lookup_test) = lookup.test.without_parentheses() else {
        return None;
    };
    if lookup_test.operator != BinaryOperator::StrictInequality
        || symbol(model, &lookup_test.left) != Some(index)
        || !signed_number(&lookup_test.right, -1.0)
    {
        return None;
    }
    let [bump, tally] = block(&lookup.consequent)? else {
        return None;
    };
    let seen = update_captured(model, site, bump, "++")?;
    let tally = assignment(tally)?;
    let AssignmentTarget::ComputedMemberExpression(tally_target) = &tally.left else {
        return None;
    };
    let counts = captured(model, site, &tally_target.object)?;
    if symbol(model, &tally_target.expression) != Some(index) {
        return None;
    }
    let Expression::BinaryExpression(tally_sum) = tally.right.without_parentheses() else {
        return None;
    };
    let Expression::CallExpression(number_call) = tally_sum.left.without_parentheses() else {
        return None;
    };
    if tally_sum.operator != BinaryOperator::Addition
        || !number(tally_sum.right.without_parentheses(), 1.0)
        || !is_global(model, &number_call.callee, &["Number"])
    {
        return None;
    }
    let inner_args = arity(number_call, 1)?;
    let [inner_value] = inner_args.as_slice() else {
        return None;
    };
    let Expression::ComputedMemberExpression(inner_read) = inner_value.without_parentheses() else {
        return None;
    };
    if symbol(model, &inner_read.object) != Some(counts)
        || symbol(model, &inner_read.expression) != Some(index)
    {
        return None;
    }
    // MAP[event.keyCode] = null
    let clear = assignment(clear)?;
    let AssignmentTarget::ComputedMemberExpression(clear_target) = &clear.left else {
        return None;
    };
    if symbol(model, &clear_target.object) != Some(map)
        || !event_keycode(model, &clear_target.expression, event)
        || !matches!(clear.right.without_parentheses(), Expression::NullLiteral(_))
    {
        return None;
    }
    // FLAG = true
    let flag = assignment(flag)?;
    let flag_slot = assign_symbol(model, flag)?;
    if !matches!(flag.right.without_parentheses(), Expression::BooleanLiteral(b) if b.value) {
        return None;
    }
    distinct(&[
        map, samples, mean, moment, first_slot, keys, seen, counts, flag_slot,
    ])
    .then_some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return Vec::new();
    };
    let Some(body) = function.body.as_ref() else {
        return Vec::new();
    };
    if function.r#async || function.generator || !directives(body) {
        return Vec::new();
    }
    if keydown(model, site, function).is_some() {
        return vec![proposal(
            "recordKeydownTimingSample",
            "events_keydown_timing_recorder",
            "owned keydown body reads Date.now, stores it per event keyCode in a captured map, seeds one captured first-timestamp slot strictly equal to zero and, in the else path, keeps the inter-key gap in a captured running total below 2000 ms and in a captured overflow total otherwise; complete ordered call/capture proof",
            facts,
        )];
    }
    if keyup(model, site, function).is_some() {
        return vec![proposal(
            "recordKeyupHoldStatistics",
            "events_keyup_hold_statistics_recorder",
            "owned keyup body reads the captured per-keyCode press clock, counts the sample, folds the hold duration into a captured running mean and second moment, stores one captured first timestamp, counts the keyCode when present in captured tracked keys, clears the stored timestamp and sets the captured active flag; complete ordered call/capture proof",
            facts,
        )];
    }
    Vec::new()
}
