//! Bounded owned-body proof for the extracted entropy-seed derivation helper.
//!
//! The family is the zero-argument helper that derives one 32-bit codec seed by
//! mixing four entropy sources, folding them with the LCG multiplier, merging
//! them through `isNaN`-guarded ternaries and adding one captured counter:
//!
//! ```text
//! function derive() {
//!   let r = Math.random() * 4294967296 >>> 0;        // random word
//!   let t = 0;
//!   try { t = performance.now() >>> 0; } catch (e) {}   // clock (swallowed)
//!   let d = Number(new Date()) >>> 0;                // timestamp
//!   let c = 0;
//!   try { c = crypto.getRandomValues(new Uint32Array(1))[0]; } catch (e) {}  // crypto
//!   r = r ^ 1103515245;                              // LCG fold
//!   t = t === t ? r ^ t : r;                         // NaN-guarded merge
//!   d = d === d ? t ^ d : t;
//!   ++counter;                                       // captured module counter
//!   return (d ^ c ^ counter) >>> 0;                  // exact connected mix
//! }
//! ```
//!
//! The proof is a direct dataflow check over the function's own body:
//!
//! * exactly zero plain formals, no rest/initializer/directive, not async or a
//!   generator, and no nested function or arrow owned by the site;
//! * one local declared from `Math.random() * 4294967296` coerced with `>>> 0`
//!   (or `| 0`), one local declared from `Number(new Date()) >>> 0`, and two
//!   zero-initialized locals that are later assigned inside a *swallowing*
//!   `try/catch`: `performance.now() >>> 0` and
//!   `crypto.getRandomValues(new Uint32Array(1))[0]`;
//! * the random local is folded once with the exact `1103515245` LCG
//!   multiplier (`x ^ 1103515245`, either operand order);
//! * the clock and timestamp locals are each merged with the running
//!   accumulator through `x === x ? acc ^ x : acc` (the `isNaN` guard), so the
//!   chain is proved connected by scoped identity and not by "the call appears
//!   somewhere in the body";
//! * one captured (outer-declared) counter is pre-incremented `++counter` and is
//!   the third leaf of the returned mix;
//! * the return is exactly `(timestamp ^ crypto ^ counter) >>> 0` with three
//!   distinct scoped leaves.
//!
//! `Math`, `performance`, `crypto`, `Number`, `Date` and `Uint32Array` are only
//! accepted as unshadowed ambient names (the shared scope-resolved
//! `expression_path` yields nothing for a shadowing local).  The constants
//! `4294967296` (2^32), `1103515245` (the LCG multiplier) and `0`/`1` are the
//! algorithm, not a seed or vendor value.  No literal spelling, member name,
//! span, thread id, KEY spelling, hash or vendor participates.  The helper is
//! independent of the seeded-emitter family: it has no `.next(...)`,
//! field-index mix or `complete()`, so the emitter proofs never see it, and it
//! is independent of the containers (`createSignalSerializer` /
//! `createSignalBundleEncoder` / bootstrap) that inline the same mix, because
//! those bodies are larger and are recursively rejected here as soon as a nested
//! function is owned by the candidate.

use super::*;
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;

type Symbol = SymbolId;

const NAME: &str = "deriveSignalCodecSeed";
const EVALUATOR: &str = "posthost_entropy_seed";
const DETAIL: &str = "zero-formal owned body: one local is `Math.random() * 4294967296 >>> 0`, one is `Number(new Date()) >>> 0`, and two zero-initialized locals are assigned `performance.now() >>> 0` and `crypto.getRandomValues(new Uint32Array(1))[0]` inside swallowing try/catch blocks; the random local is folded once with the 1103515245 LCG multiplier; the clock and timestamp locals are each merged with the running accumulator through an `x === x ? acc ^ x : acc` NaN guard; one captured counter is pre-incremented and is the third leaf of the returned `(timestamp ^ crypto ^ counter) >>> 0` mix; Math/performance/crypto/Number/Date/Uint32Array must be unshadowed ambient names and no nested callback may donate evidence";

/// The exact LCG multiplier that folds the random word into the seed chain.
const LCG_MULTIPLIER: f64 = 1_103_515_245.0;
/// 2^32, the modulus `Math.random()` is scaled by before the unsigned coercion.
const UINT32_SPAN: f64 = 4_294_967_296.0;

// ===========================================================================
// Small typed views of the source operations
// ===========================================================================

fn symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(expression.without_parentheses(), model.scoping)
}

fn is_symbol(model: &Model<'_, '_>, expression: &Expression<'_>, expected: Symbol) -> bool {
    symbol(model, expression) == Some(expected)
}

fn numeric(expression: &Expression<'_>) -> Option<f64> {
    match expression.without_parentheses() {
        Expression::NumericLiteral(literal) => Some(literal.value),
        Expression::UnaryExpression(unary) if unary.operator.as_str() == "-" => {
            numeric(&unary.argument).map(|value| -value)
        }
        _ => None,
    }
}

fn is_numeric(expression: &Expression<'_>, expected: f64) -> bool {
    numeric(expression) == Some(expected)
}

fn is_zero(expression: &Expression<'_>) -> bool {
    is_numeric(expression, 0.0)
}

/// The unshadowed ambient member chain a global access denotes, compared against
/// an expected path.  A shadowing local yields no path, so it can never stand in
/// for `Math`, `performance`, `crypto`, `Number`, `Date` or `Uint32Array`.
fn path_is(model: &Model<'_, '_>, expression: &Expression<'_>, path: &[&str]) -> bool {
    let actual = expression_path(model, expression.without_parentheses());
    actual.len() == path.len()
        && actual
            .iter()
            .zip(path)
            .all(|(actual, want)| actual.as_str() == *want)
}

fn new_arguments<'a>(new: &'a NewExpression<'a>) -> Vec<&'a Expression<'a>> {
    new.arguments
        .iter()
        .filter_map(Argument::as_expression)
        .collect()
}

// ===========================================================================
// The four entropy sources
// ===========================================================================

/// `value >>> 0` or `value | 0`: the unsigned 32-bit coercion, returning the
/// coerced operand.
fn uint_coerce<'a, 'e>(expression: &'a Expression<'e>) -> Option<&'a Expression<'e>> {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return None;
    };
    let operator = binary.operator.as_str();
    ((operator == ">>>" || operator == "|") && is_zero(&binary.right)).then_some(&binary.left)
}

/// `Math.random() * 4294967296` (either operand order), then `>>> 0` / `| 0`.
fn is_random_word(model: &Model<'_, '_>, init: &Expression<'_>) -> bool {
    let Some(inner) = uint_coerce(init) else {
        return false;
    };
    let Expression::BinaryExpression(product) = inner.without_parentheses() else {
        return false;
    };
    if product.operator.as_str() != "*" {
        return false;
    }
    let random = if is_numeric(&product.left, UINT32_SPAN) {
        &product.right
    } else if is_numeric(&product.right, UINT32_SPAN) {
        &product.left
    } else {
        return false;
    };
    let Expression::CallExpression(call) = random.without_parentheses() else {
        return false;
    };
    !call.optional && call.arguments.is_empty() && path_is(model, &call.callee, &["Math", "random"])
}

/// `Number(new Date()) >>> 0`.
fn is_date_word(model: &Model<'_, '_>, init: &Expression<'_>) -> bool {
    let Some(inner) = uint_coerce(init) else {
        return false;
    };
    let Expression::CallExpression(call) = inner.without_parentheses() else {
        return false;
    };
    if call.optional || !path_is(model, &call.callee, &["Number"]) {
        return false;
    }
    let arguments = call_arguments(call);
    if arguments.len() != 1 || call.arguments.len() != 1 {
        return false;
    }
    let Expression::NewExpression(new) = arguments[0].without_parentheses() else {
        return false;
    };
    new.arguments.is_empty() && path_is(model, &new.callee, &["Date"])
}

/// `performance.now() >>> 0`.
fn is_clock_word(model: &Model<'_, '_>, value: &Expression<'_>) -> bool {
    let Some(inner) = uint_coerce(value) else {
        return false;
    };
    let Expression::CallExpression(call) = inner.without_parentheses() else {
        return false;
    };
    !call.optional
        && call.arguments.is_empty()
        && path_is(model, &call.callee, &["performance", "now"])
}

/// `crypto.getRandomValues(new Uint32Array(1))[0]`.
fn is_crypto_word(model: &Model<'_, '_>, value: &Expression<'_>) -> bool {
    let Expression::ComputedMemberExpression(member) = value.without_parentheses() else {
        return false;
    };
    if member.optional || !is_numeric(&member.expression, 0.0) {
        return false;
    }
    let Expression::CallExpression(call) = member.object.without_parentheses() else {
        return false;
    };
    if call.optional || !path_is(model, &call.callee, &["crypto", "getRandomValues"]) {
        return false;
    }
    let arguments = call_arguments(call);
    if arguments.len() != 1 || call.arguments.len() != 1 {
        return false;
    }
    let Expression::NewExpression(new) = arguments[0].without_parentheses() else {
        return false;
    };
    if !path_is(model, &new.callee, &["Uint32Array"]) {
        return false;
    }
    let ctor_arguments = new_arguments(new);
    ctor_arguments.len() == 1 && is_numeric(ctor_arguments[0], 1.0)
}

// ===========================================================================
// The fold, the merges and the connected return
// ===========================================================================

/// `x ^ 1103515245` (either operand order) on the random local.
fn is_lcg_fold(model: &Model<'_, '_>, value: &Expression<'_>, random: Symbol) -> bool {
    let Expression::BinaryExpression(xor) = value.without_parentheses() else {
        return false;
    };
    xor.operator.as_str() == "^"
        && ((is_symbol(model, &xor.left, random) && is_numeric(&xor.right, LCG_MULTIPLIER))
            || (is_symbol(model, &xor.right, random) && is_numeric(&xor.left, LCG_MULTIPLIER)))
}

/// `acc ^ self` (either operand order).
fn is_xor_pair(
    model: &Model<'_, '_>,
    value: &Expression<'_>,
    self_slot: Symbol,
    accumulator: Symbol,
) -> bool {
    let Expression::BinaryExpression(xor) = value.without_parentheses() else {
        return false;
    };
    xor.operator.as_str() == "^"
        && ((is_symbol(model, &xor.left, self_slot) && is_symbol(model, &xor.right, accumulator))
            || (is_symbol(model, &xor.left, accumulator)
                && is_symbol(model, &xor.right, self_slot)))
}

/// `x === x ? accumulator ^ x : accumulator`: the `isNaN`-guarded merge of one
/// source into the running accumulator.
fn merge_shape(
    model: &Model<'_, '_>,
    value: &Expression<'_>,
    self_slot: Symbol,
    accumulator: Symbol,
) -> bool {
    let Expression::ConditionalExpression(conditional) = value.without_parentheses() else {
        return false;
    };
    let Expression::BinaryExpression(test) = conditional.test.without_parentheses() else {
        return false;
    };
    if test.operator.as_str() != "==="
        || !is_symbol(model, &test.left, self_slot)
        || !is_symbol(model, &test.right, self_slot)
    {
        return false;
    }
    is_xor_pair(model, &conditional.consequent, self_slot, accumulator)
        && is_symbol(model, &conditional.alternate, accumulator)
}

/// Flatten a `^` chain into its leaves.  The walk is bounded so a pathological
/// expression decides deterministically.
fn xor_leaves<'a>(
    expression: &'a Expression<'a>,
    leaves: &mut Vec<&'a Expression<'a>>,
    depth: usize,
) {
    if depth > 8 {
        leaves.push(expression);
        return;
    }
    match expression.without_parentheses() {
        Expression::BinaryExpression(binary) if binary.operator.as_str() == "^" => {
            xor_leaves(&binary.left, leaves, depth + 1);
            xor_leaves(&binary.right, leaves, depth + 1);
        }
        other => leaves.push(other),
    }
}

// ===========================================================================
// Read-only owned-body census
// ===========================================================================

/// The declarations, assignments, updates and returns of this exact function
/// body.  Nested functions and classes are counted and never entered, so a
/// decoy nested callback cannot donate the operations.
#[derive(Default)]
struct Census<'a> {
    declared: Vec<&'a VariableDeclarator<'a>>,
    /// `target = value` identifier assignments, with whether they sit in a
    /// swallowing `try`.
    assigned: Vec<(&'a AssignmentExpression<'a>, bool)>,
    updates: Vec<&'a UpdateExpression<'a>>,
    returns: Vec<&'a Expression<'a>>,
    nested: usize,
}

impl<'a> Census<'a> {
    fn body(&mut self, body: &'a FunctionBody<'a>) {
        self.statements(&body.statements, false);
    }

    fn statements(&mut self, statements: &'a [Statement<'a>], guarded: bool) {
        for statement in statements {
            self.statement(statement, guarded);
        }
    }

    fn statement(&mut self, statement: &'a Statement<'a>, guarded: bool) {
        match statement {
            Statement::VariableDeclaration(declaration) => {
                for declarator in &declaration.declarations {
                    self.declared.push(declarator);
                }
            }
            Statement::ExpressionStatement(statement) => {
                match statement.expression.without_parentheses() {
                    Expression::AssignmentExpression(assignment) => {
                        self.assigned.push((assignment, guarded));
                    }
                    Expression::UpdateExpression(update) => self.updates.push(update),
                    _ => {}
                }
            }
            Statement::ReturnStatement(statement) => {
                if let Some(argument) = &statement.argument {
                    self.returns.push(argument);
                }
            }
            Statement::TryStatement(attempt) => {
                let swallow = attempt
                    .handler
                    .as_ref()
                    .is_some_and(|handler| handler.body.body.is_empty());
                self.statements(&attempt.block.body, swallow);
            }
            Statement::BlockStatement(block) => self.statements(&block.body, guarded),
            Statement::IfStatement(branch) => {
                self.statement(&branch.consequent, guarded);
                if let Some(alternate) = &branch.alternate {
                    self.statement(alternate, guarded);
                }
            }
            Statement::FunctionDeclaration(_) | Statement::ClassDeclaration(_) => self.nested += 1,
            _ => {}
        }
    }
}

// ===========================================================================
// The proof
// ===========================================================================

fn declarator_symbol(declarator: &VariableDeclarator<'_>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
        return None;
    };
    identifier.symbol_id.get()
}

fn assignment_target(
    model: &Model<'_, '_>,
    assignment: &AssignmentExpression<'_>,
) -> Option<Symbol> {
    if assignment.operator.as_str() != "=" {
        return None;
    }
    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
        return None;
    };
    crate::analysis::identifier_symbol(identifier, model.scoping)
}

fn update_target(model: &Model<'_, '_>, update: &UpdateExpression<'_>) -> Option<Symbol> {
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) = &update.argument else {
        return None;
    };
    crate::analysis::identifier_symbol(identifier, model.scoping)
}

/// True when the binding is declared strictly outside the owned body (a
/// captured module/closure counter).
fn declared_outside(model: &Model<'_, '_>, site: FunctionSite, value: Symbol) -> bool {
    if !model.scoping.symbol_redeclarations(value).is_empty() {
        return false;
    }
    let declaration = model.scoping.symbol_declaration(value);
    let body = site.body_span.unwrap_or(site.span);
    model.nodes.get_node(declaration).span().end <= body.start
}

fn prove(model: &Model<'_, '_>, site: FunctionSite, census: &Census<'_>) -> Option<()> {
    // Each local declaration with a matching initializer.
    let random = census.declared.iter().find_map(|declarator| {
        let value = declarator_symbol(declarator)?;
        is_random_word(model, declarator.init.as_ref()?).then_some(value)
    })?;
    let timestamp = census.declared.iter().find_map(|declarator| {
        let value = declarator_symbol(declarator)?;
        is_date_word(model, declarator.init.as_ref()?).then_some(value)
    })?;
    let zero_initialized: Vec<Symbol> = census
        .declared
        .iter()
        .filter_map(|declarator| {
            let value = declarator_symbol(declarator)?;
            is_zero(declarator.init.as_ref()?).then_some(value)
        })
        .collect();

    // The clock and crypto locals each start at zero and are assigned their
    // ambient source only inside a swallowing try/catch.
    let clock = census.assigned.iter().find_map(|(assignment, guarded)| {
        if !*guarded {
            return None;
        }
        let value = assignment_target(model, assignment)?;
        is_clock_word(model, &assignment.right).then_some(value)
    })?;
    let crypto = census.assigned.iter().find_map(|(assignment, guarded)| {
        if !*guarded {
            return None;
        }
        let value = assignment_target(model, assignment)?;
        is_crypto_word(model, &assignment.right).then_some(value)
    })?;
    if !zero_initialized.contains(&clock) || !zero_initialized.contains(&crypto) {
        return None;
    }

    // Four distinct source slots.
    let mut distinct = BTreeSet::new();
    for value in [random, timestamp, clock, crypto] {
        if !distinct.insert(value) {
            return None;
        }
    }

    // The LCG fold of the random word, then the guarded merges up the chain.
    if !census.assigned.iter().any(|(assignment, _)| {
        assignment_target(model, assignment) == Some(random)
            && is_lcg_fold(model, &assignment.right, random)
    }) {
        return None;
    }
    if !census.assigned.iter().any(|(assignment, _)| {
        assignment_target(model, assignment) == Some(clock)
            && merge_shape(model, &assignment.right, clock, random)
    }) {
        return None;
    }
    if !census.assigned.iter().any(|(assignment, _)| {
        assignment_target(model, assignment) == Some(timestamp)
            && merge_shape(model, &assignment.right, timestamp, clock)
    }) {
        return None;
    }

    // The single return is exactly `(timestamp ^ crypto ^ counter) >>> 0`.
    let returned = match census.returns.as_slice() {
        [only] => *only,
        _ => return None,
    };
    let Expression::BinaryExpression(shift) = returned.without_parentheses() else {
        return None;
    };
    if shift.operator.as_str() != ">>>" || !is_zero(&shift.right) {
        return None;
    }
    let mut leaves = Vec::new();
    xor_leaves(&shift.left, &mut leaves, 0);
    if leaves.len() != 3 {
        return None;
    }
    let mut returned_slots = Vec::with_capacity(3);
    for leaf in &leaves {
        returned_slots.push(symbol(model, *leaf)?);
    }
    let returned_set: BTreeSet<Symbol> = returned_slots.iter().copied().collect();
    if returned_set.len() != 3
        || !returned_set.contains(&timestamp)
        || !returned_set.contains(&crypto)
    {
        return None;
    }
    let counter = *returned_slots
        .iter()
        .find(|value| **value != timestamp && **value != crypto)?;

    // The counter is a captured binding, pre-incremented exactly once.
    if !declared_outside(model, site, counter) {
        return None;
    }
    if census.updates.len() != 1 {
        return None;
    }
    let update = census.updates[0];
    if update.operator.as_str() != "++" || !update.prefix {
        return None;
    }
    if update_target(model, update) != Some(counter) {
        return None;
    }
    Some(())
}

fn has_owned_nested(model: &Model<'_, '_>, site: FunctionSite) -> bool {
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
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !function.params.items.is_empty()
    {
        return Vec::new();
    }
    let Some(body) = function.body.as_ref() else {
        return Vec::new();
    };
    if !body.directives.is_empty() {
        return Vec::new();
    }
    // No nested function or arrow may donate evidence, and the nested-body
    // container form (createSignalSerializer / bootstrap) is refused here.
    if has_owned_nested(model, site) {
        return Vec::new();
    }
    let mut census = Census::default();
    census.body(body);
    if census.nested != 0 || prove(model, site, &census).is_none() {
        return Vec::new();
    }
    vec![proposal(NAME, EVALUATOR, DETAIL, f)]
}
