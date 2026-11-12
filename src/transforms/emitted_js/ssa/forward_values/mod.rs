//! Forward generated `$SSA_`/`$CARRY_` values to their reads in one parse.
//!
//! A candidate is `let|const $SSA_n = init;` (one plain binding, never
//! written, no comment). Two rules decide whether its binding can go:
//!
//! 1. **Stable value.** `init` is a primitive literal (or the unshadowed
//!    `undefined`, `NaN`, `Infinity`), an identifier bound to a never-written
//!    local that is not a `$KEY_`/`$CARRY_` slot (a parameter only when its
//!    function never mentions `arguments`), or `x[k]` where `x` holds a fresh
//!    helper tuple that is only ever read at in-range literal indexes. Nothing
//!    can change such a value, so it replaces every read. Reads must be in the
//!    same function, after the declaration, not shorthand properties, and the
//!    substituted names must still resolve to the same bindings there.
//! 2. **Next door.** Any `init` read exactly once, where the read sits in the
//!    next statement that does anything: statements in between may only
//!    declare or assign stable values to locals that `init` does not read and
//!    no closure touches. Everything the read's statement evaluates before the
//!    read must be stable, and the read must be evaluated exactly once there
//!    (not in a branch arm, short-circuit operand, optional chain, loop, `try`
//!    or closure). `init` then runs at the same point relative to every effect.
//!
//! 3. **Duplicate slots.** In `let [a, b] = H(…)` where `H` is a fresh-tuple
//!    helper returning one identifier in every slot (`return [v, v]`), `b` is
//!    the same value as `a`: its slot becomes a hole and its reads read `a`.
//! 4. **Pass-through slots.** A slot that `H` fills with an unwritten
//!    parameter (`return [c, a, b, a]`) is the call's argument; a stable
//!    argument replaces the slot's reads. When `H` only shuffles its
//!    parameters and every slot goes, the whole call goes; a value that
//!    would move into that call (or write a slot early) waits a round.
//!
//! 5. **Slot snapshots.** `let $SSA_n = $KEY_k;` (or a `$CARRY_` copy) when
//!    the slot is declared once, before the copy, in the same function; no
//!    closure writes it; nothing writes it between the copy and the last read
//!    (a write whose own block jumps away with `return`, `throw` or `continue`
//!    before a read it could reach does not count); and no read sits in a loop
//!    the copy is not in. The reads then see the same slot value and read the
//!    slot directly. A snapshot read never moves inside another forwarded
//!    value, which could carry it past a write.
//!
//! 6. **Slot handoff.** A value read several times whose first read is the
//!    store of the next statement (`var $KEY_k = v;` or `$KEY_k = v;`) moves
//!    into that store under rule 2, and its later reads read the slot, which
//!    must stay unchanged until the last of them by the rule 5 checks.
//!
//! 7. **Early slot.** A value stored by a later sibling statement into a
//!    `var` slot (`$KEY_k = v;` / `var $KEY_k = v;`), or into a `let` carry
//!    declared before the value (a branch or loop join), is written to the slot
//!    where it is computed, the store goes, and every read reads the slot.
//!    Nothing may read or store the slot between the definition and the last
//!    read (and nothing may run code there if another function sees the
//!    slot); no `try` in the function may observe the early write after a
//!    throw; and no read may sit in a loop the definition is not in.
//!
//! In a function with no parameters whose `arguments` is only read at
//! literal indexes (`dead_ssa::readonly_arguments`), `arguments[n]` cannot
//! change during the call: it is a stable value like a fresh tuple slot.
//!
//! Operators over primitives of known type (`primitives.rs`) run no code,
//! so they are not effects; a pure computation read once is stable; and a
//! discarded pure expression statement — the VM's decoys such as `x + 2;` —
//! is deleted, its reads ignored.
//!
//! An internal `$KEY_` slot — one `var` declaration with an initializer,
//! never stored again, outside any loop, and referenced only by its own
//! function (a thread that does not export it) — is an immutable local:
//! it is a candidate like `$SSA_` and counts as stable.
//!
//! 8. **Store to load.** After `$KEY_k = v;` (or `var $KEY_k = v;`) with a
//!    stable `v`, a read of the slot in a later statement of the same list,
//!    before any other store, reads `v` when the rule 5 checks hold (no code
//!    runs in between if another function sees the slot; no read in a loop
//!    the store is not in). The store stays; a later stage drops it once
//!    nothing reads it.
//!
//! 9. **Branch-known booleans.** Inside the arms of `if (r)` / `r ? a : b`
//!    (or `!r`), a never-written boolean `r` is exactly `true` in one arm and
//!    `false` in the other; its reads there become that literal, leaving the
//!    test as its only read.
//!
//! 10. **Fresh allocations.** An initializer that allocates (`[]`, `{ k: v }`,
//!     a function) is never copied into several reads, which would split one
//!     identity; read once, it moves to that read anywhere in its function
//!     except into a loop the declaration is not in.
//!
//! A named property of a standard built-in (`Object.defineProperty`,
//! `Function.prototype.toString`) reads a fixed value. A binding only its
//! declaring function's own code writes is settled inside a closure: the
//! declaring frame is suspended or finished while the closure runs (no
//! `await`/`yield` in the file; a `let` must be that function's first
//! statement). A next-door move may also cross reads of a local that only its
//! own function writes, that the moved value does not write, and that is
//! initialized before it: no call the value makes can change what they read.
//!
//! Reading `.call`/`.apply` off a plain name in order to call it is taken
//! as running no code — the standard `Function.prototype` methods, the same
//! assumption the program's unbound calls make.
//!
//! A stable value never carries a moved (rule 2) value inside it, which
//! would evaluate that value once per read.
//!
//! Chains compose: a value forwarded into another forwarded initializer moves
//! with it, so one call removes a whole chain. Composed text is parenthesized
//! (a member callee becomes `(0, value)` to keep a bare-call `this`). `with`
//! and `eval` decline.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, NodeId, ReferenceId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UnaryOperator};
use oxc_syntax::scope::ScopeFlags;

use super::super::dead_ssa::readonly_arguments;
use super::super::primitives::{self, Types};
use super::super::tuple_unpack::fresh_tuple_helper;
use crate::analysis;
use super::super::support::whole_line;

mod scan;
mod decide;
mod stability;
mod slots;
mod edits;
mod holders;
#[cfg(test)]
mod tests;

use holders::{FixedArguments, Holders};
use crate::transforms::emitted_js::support::is_loop;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ForwardValuesResult {
    pub code: String,
    pub forwarded: usize,
}

pub(crate) fn forward_values(source: &str) -> Result<ForwardValuesResult, String> {
    forward(source, false)
}

/// One forwarding pass, for callers that already go around until nothing
/// changes.
pub(crate) fn forward_values_once(source: &str) -> Result<ForwardValuesResult, String> {
    forward_once(source, false)
}

/// Forwarding with rule 12 on. Copying property reads hides the plain names
/// resugaring folds (`o.m.call(o, …)`, `c ? c : b`), so it runs once those
/// folds are done.
pub(crate) fn forward_values_and_property_reads(source: &str) -> Result<ForwardValuesResult, String> {
    forward(source, true)
}

fn forward(source: &str, property_reads: bool) -> Result<ForwardValuesResult, String> {
    // A moved value can make the next one movable (a chain of offsets, each
    // read by the next): go around until nothing moves.
    let mut result = forward_once(source, property_reads)?;
    let mut last = result.forwarded;
    for _ in 0..64 {
        if last == 0 {
            break;
        }
        let next = forward_once(&result.code, property_reads)?;
        last = next.forwarded;
        result.forwarded += next.forwarded;
        result.code = next.code;
    }
    Ok(result)
}

fn forward_once(source: &str, property_reads: bool) -> Result<ForwardValuesResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut holders = Holders::default();
    holders.visit_program(&program);
    let mut fixed = FixedArguments::default();
    fixed.visit_program(&program);
    let types = Types::build(&program, &scoping, &nodes);
    let tuple_slots = holders.read_only(&scoping, &nodes);
    let duplicate_helpers = holders
        .duplicates
        .iter()
        .copied()
        .filter(|helper| !scoping.symbol_is_mutated(*helper))
        .collect();
    let passthrough = holders.passthrough(&scoping);
    let fresh_helpers = holders.fresh(&scoping);
    let mut scan = Scan {
        property_reads,
        scoping: Some(&scoping),
        owners: vec![PROGRAM_OWNER],
        tuple_slots,
        duplicate_helpers,
        passthrough,
        fresh_helpers,
        comments: program.comments.iter().map(|comment| comment.span.start).collect(),
        fixed_arguments: fixed.owners,
        types: Some(&types),
        ..Scan::default()
    };
    scan.visit_program(&program);
    if scan.dynamic {
        return Ok(ForwardValuesResult {
            code: source.into(),
            forwarded: 0,
        });
    }
    scan.effects.sort_unstable();
    scan.mutations.sort_unstable();
    scan.runs.sort_unstable();
    scan.idents.sort_unstable();
    scan.noops.sort_unstable_by_key(|span| span.start);
    scan.arguments.sort_unstable();
    scan.suspends.sort_unstable();
    let mut plan = Plan {
        source,
        scoping: &scoping,
        nodes: &nodes,
        scan: &scan,
        forwarded: BTreeMap::new(),
        blocked: BTreeSet::new(),
        vanishing: BTreeSet::new(),
        moved: BTreeMap::new(),
        lookups: std::cell::RefCell::new(None),
    };
    // A placement whose names would resolve differently blocks that value;
    // deciding again settles the values composed with it.
    let edits = loop {
        plan.decide();
        match plan.edits() {
            Ok(edits) => break edits,
            Err(symbols) => plan.blocked.extend(symbols),
        }
    };
    if edits.is_empty() {
        return Ok(ForwardValuesResult {
            code: source.into(),
            forwarded: 0,
        });
    }
    let forwarded = plan.forwarded.len()
        + scan.noops.len()
        + plan.store_loads(&[]).len()
        + plan.branch_booleans(&[]).len();
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("forwarded value edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("forwarded value output: {error}"))?;
    Ok(ForwardValuesResult {
        code: applied.output,
        forwarded,
    })
}

/// The owner of top-level code; functions are keyed by their span start.
const PROGRAM_OWNER: u32 = u32::MAX;

#[derive(Clone, Copy, PartialEq, Eq)]
enum InitKind {
    /// A literal that reads correctly anywhere (string, boolean, null).
    Word,
    /// A non-negative numeric literal: needs parentheses as a member object.
    Number,
    /// `-1`, `void 0`: a unary over a numeric literal.
    UnaryLiteral,
    Identifier,
    /// `x[k]` of a read-only fresh tuple: as stable as an identifier.
    TupleSlot,
    Computed,
}

struct Candidate {
    list: usize,
    position: usize,
    statement: Span,
    symbol: SymbolId,
    init: Span,
    kind: InitKind,
    /// `init` is a member expression: as a callee it needs `(0, …)`.
    member: bool,
    /// `init` is a comma expression: it needs parentheses everywhere.
    sequence: bool,
    /// The reference of an identifier `init`.
    init_reference: Option<ReferenceId>,
    /// A duplicate tuple slot: the first named slot it equals. `init` is
    /// then the slot's own span.
    alias_of: Option<SymbolId>,
    /// A tuple pattern slot: removing the binding turns it into a hole.
    slot: Option<Span>,
    /// The helper whose parameter fills a pass-through slot.
    helper: Option<SymbolId>,
    /// `init` has no effect, cannot throw (`primitives.rs`) and allocates
    /// nothing, so copies of it keep one identity.
    pure: bool,
    /// `init` allocates (`[]`, `{ k: 1 }`) and is otherwise pure: it moves
    /// to one read anywhere it runs once, but is never copied.
    fresh: bool,
    /// `init` only reads properties (`o.p`, `o[k].q`) off a name with keys
    /// of known primitive type: with no getters, it reads the same value
    /// wherever no code runs in between (rule 12).
    property_read: bool,
    /// `x ± n`, `x * n`, `x / n` or a conditional: a counter step or a
    /// choice keeps its one name rather than being copied to several reads.
    named: bool,
}

/// `$KEY_k = v;` / `var $KEY_k = v;` in a statement list.
struct SlotStore {
    list: usize,
    position: usize,
    statement: Span,
    slot: SymbolId,
    value: Span,
    kind: InitKind,
    member: bool,
    value_reference: Option<ReferenceId>,
}

struct Statement {
    span: Span,
    owner: u32,
    /// Symbols written by a declaration/assignment of stable values, or
    /// `None` when the statement has another form.
    writes: Option<Vec<SymbolId>>,
}

#[derive(Default)]
struct Scan<'s> {
    /// Rule 12 is on.
    property_reads: bool,
    scoping: Option<&'s Scoping>,
    owners: Vec<u32>,
    lists: Vec<Vec<Statement>>,
    candidates: Vec<Candidate>,
    /// Nodes that evaluate user-observable work: (start, end, owner).
    effects: Vec<(u32, u32, u32)>,
    /// Effects other than property reads and stores to plain names: the
    /// ones that can change a property.
    mutations: Vec<(u32, u32, u32)>,
    /// Calls and constructions, by span.
    calls: BTreeSet<(u32, u32)>,
    /// Nodes that can run code with no getters or setters and primitive
    /// operands: calls, `new`, tagged templates, `import()`, `await`,
    /// `yield`, spreads (iterators) and classes. Sorted by start.
    runs: Vec<(u32, u32, u32)>,
    /// Stores to a property, by span, with the name the target starts from.
    member_stores: BTreeMap<(u32, u32), Option<SymbolId>>,
    /// Operator effects by span: the operand that is not of known type, when
    /// only one is. Such an operator runs only that operand's conversion.
    conversions: BTreeMap<(u32, u32), Span>,
    /// Identifier references: (start, reference, owner).
    idents: Vec<(u32, ReferenceId, u32)>,
    /// Starts of `arguments` references.
    arguments: Vec<u32>,
    /// `await`/`yield` expressions: (start, end, owner). A suspended frame
    /// lets an enclosing function store into its slots.
    suspends: Vec<(u32, u32, u32)>,
    /// Bindings holding a fresh helper tuple read only at in-range indexes,
    /// with the tuple length.
    tuple_slots: BTreeMap<SymbolId, usize>,
    /// Never-reassigned fresh-tuple helpers.
    fresh_helpers: BTreeSet<SymbolId>,
    /// Never-reassigned fresh-tuple helpers that return one identifier in
    /// every slot.
    duplicate_helpers: BTreeSet<SymbolId>,
    /// Fresh-tuple helpers with parameter-filled slots.
    passthrough: BTreeMap<SymbolId, PassThrough>,
    /// Pass-through tuple declarations by start: (named slots, removable).
    groups: BTreeMap<u32, (usize, bool)>,
    /// Sorted comment starts.
    comments: Vec<u32>,
    /// Functions (by span start) whose `arguments[n]` reads cannot change.
    fixed_arguments: BTreeSet<u32>,
    types: Option<&'s Types<'s>>,
    /// Discarded pure expression statements: decoys to delete.
    noops: Vec<Span>,
    /// Stores of a stable-looking value into a slot (rule 8).
    stores: Vec<SlotStore>,
    /// Branches on a plain name (rule 9): the name's reference, whether the
    /// test negates it, the two arm spans, and the owner.
    branches: Vec<(ReferenceId, bool, Span, Option<Span>, u32)>,
    dynamic: bool,
}

/// A helper's return slots: `Some(i)` when slot `k` is unwritten parameter `i`.
#[derive(Clone)]
struct PassThrough {
    slots: Vec<Option<usize>>,
    /// The body is only `return [params and literals]`.
    shuffle: bool,
    span: Span,
}

#[derive(Clone, PartialEq)]
struct Forward {
    candidate: usize,
    /// Rule 1: the value is stable and may be read anywhere it resolves.
    stable: bool,
    /// Rule 5: a slot snapshot, valid only at its reads' own positions.
    snapshot: bool,
    /// Rule 6: the slot the value is stored into, and the store's read.
    handoff: Option<(SymbolId, NodeId)>,
    /// Rule 7: the slot written at the definition, and the store statement
    /// that goes (with whether it was a `var` declaration).
    early: Option<(SymbolId, Span, bool)>,
    /// Every symbol the composed value reads (unresolved names as `None`).
    reads: Vec<(String, Option<SymbolId>)>,
}

struct Plan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    scan: &'s Scan<'s>,
    forwarded: BTreeMap<SymbolId, Forward>,
    blocked: BTreeSet<SymbolId>,
    /// Starts of declarations a previous round forwarded as stable values:
    /// they evaluate nothing, so a move may cross them once they are gone.
    vanishing: BTreeSet<u32>,
    /// Declarations a previous round moved (rule 2), by start: the start of
    /// the statement they moved into and of their read there. A move into the
    /// same statement at a later read keeps both values in order.
    moved: BTreeMap<u32, (u32, u32)>,
    /// While `consider` runs: every lookup into the decision state it makes,
    /// with what it returned.
    lookups: std::cell::RefCell<Option<Lookups>>,
}

/// What one `consider` call read from the decisions and assumptions of the
/// round so far. The same reads giving the same answers give the same result.
#[derive(Default)]
struct Lookups {
    seen: std::collections::HashSet<(u8, u32)>,
    forwarded: Vec<(SymbolId, Option<Forward>)>,
    vanishing: Vec<(u32, bool)>,
    moved: Vec<(u32, Option<(u32, u32)>)>,
}

impl Lookups {
    fn hold(&self, plan: &Plan<'_, '_>) -> bool {
        self.forwarded.iter().all(|(symbol, seen)| plan.forwarded.get(symbol) == seen.as_ref())
            && self.vanishing.iter().all(|(start, seen)| plan.vanishing.contains(start) == *seen)
            && self.moved.iter().all(|(start, seen)| plan.moved.get(start) == seen.as_ref())
    }
}


/// Non-writable, non-configurable globals: read unshadowed, they are constants.
/// `r` or `!r` as a test: the name's reference and whether it is negated.
fn tested_name(test: &Expression<'_>) -> Option<(ReferenceId, bool)> {
    match test {
        Expression::Identifier(identifier) => Some((identifier.reference_id.get()?, false)),
        Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::LogicalNot => match &unary.argument {
            Expression::Identifier(identifier) => Some((identifier.reference_id.get()?, true)),
            _ => None,
        },
        _ => None,
    }
}

fn is_constant_global(name: &str) -> bool {
    super::super::primitives::is_stable_global(name)
}

/// A position that takes any non-comma expression without parentheses.
fn is_isolated_slot(parent: AstKind<'_>, read: Span) -> bool {
    match parent {
        AstKind::CallExpression(call) => call.callee.span() != read,
        AstKind::NewExpression(call) => call.callee.span() != read,
        AstKind::ReturnStatement(_)
        | AstKind::ThrowStatement(_)
        | AstKind::VariableDeclarator(_)
        | AstKind::ArrayExpression(_)
        | AstKind::TemplateLiteral(_) => true,
        AstKind::AssignmentExpression(assignment) => assignment.right.span() == read,
        AstKind::IfStatement(statement) => statement.test.span() == read,
        AstKind::ObjectProperty(property) => property.value.span() == read && !property.shorthand,
        _ => false,
    }
}

fn is_member_object(parent: AstKind<'_>, read: Span) -> bool {
    match parent {
        AstKind::StaticMemberExpression(member) => member.object.span() == read,
        AstKind::ComputedMemberExpression(member) => member.object.span() == read,
        AstKind::PrivateFieldExpression(member) => member.object.span() == read,
        _ => false,
    }
}

fn is_statement(kind: AstKind<'_>) -> bool {
    matches!(
        kind,
        AstKind::BlockStatement(_)
            | AstKind::ForStatement(_)
            | AstKind::ForInStatement(_)
            | AstKind::ForOfStatement(_)
            | AstKind::WhileStatement(_)
            | AstKind::DoWhileStatement(_)
            | AstKind::TryStatement(_)
            | AstKind::SwitchStatement(_)
            | AstKind::LabeledStatement(_)
            | AstKind::IfStatement(_)
            | AstKind::ExpressionStatement(_)
            | AstKind::VariableDeclaration(_)
            | AstKind::ReturnStatement(_)
            | AstKind::ThrowStatement(_)
            | AstKind::FunctionBody(_)
    )
}

/// `o.p`, `o[k]`, `o[k].q` off a plain name or `this`, not optional, with
/// literal keys or keys of known primitive type (no `toString` to run).
fn property_read(expression: &Expression<'_>, types: &Types<'_>) -> bool {
    property_tree(expression, types) && reads_a_property(expression)
}

/// Operators over property reads, names and literals (no calls, stores or
/// allocations).
fn property_tree(expression: &Expression<'_>, types: &Types<'_>) -> bool {
    match expression.without_parentheses() {
        Expression::StaticMemberExpression(_) | Expression::ComputedMemberExpression(_) => member_read(expression, types),
        Expression::BinaryExpression(binary) => {
            !matches!(binary.operator, BinaryOperator::Instanceof)
                && property_tree(&binary.left, types)
                && property_tree(&binary.right, types)
        }
        Expression::LogicalExpression(logical) => property_tree(&logical.left, types) && property_tree(&logical.right, types),
        Expression::ConditionalExpression(choice) => {
            property_tree(&choice.test, types) && property_tree(&choice.consequent, types) && property_tree(&choice.alternate, types)
        }
        Expression::UnaryExpression(unary) => {
            unary.operator != UnaryOperator::Delete && property_tree(&unary.argument, types)
        }
        Expression::Identifier(_)
        | Expression::NumericLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::ThisExpression(_) => true,
        _ => false,
    }
}

fn reads_a_property(expression: &Expression<'_>) -> bool {
    struct Member(bool);
    impl<'a> Visit<'a> for Member {
        fn visit_member_expression(&mut self, _: &MemberExpression<'a>) {
            self.0 = true;
        }
    }
    let mut member = Member(false);
    member.visit_expression(expression);
    member.0
}

fn member_read(expression: &Expression<'_>, types: &Types<'_>) -> bool {
    let base = |object: &Expression<'_>| {
        matches!(object, Expression::Identifier(_) | Expression::ThisExpression(_)) || member_read(object, types)
    };
    match expression {
        Expression::StaticMemberExpression(member) => !member.optional && base(&member.object),
        Expression::ComputedMemberExpression(member) => {
            !member.optional
                && base(&member.object)
                && (matches!(member.expression, Expression::StringLiteral(_) | Expression::NumericLiteral(_))
                    // Keys convert without running code, as for `in`.
                    || (types.pure(&member.expression) && !primitives::allocates(&member.expression))
                    || member_read(&member.expression, types)
                    || matches!(&member.expression, Expression::ComputedMemberExpression(key)
                        if matches!(&key.object, Expression::Identifier(object) if object.name == "arguments")
                            && matches!(key.expression, Expression::NumericLiteral(_))))
        }
        _ => false,
    }
}

/// A counter step or a conditional choice (shape/'s redundant-memory rule
/// leaves these named too).
fn keeps_name(expression: &Expression<'_>) -> bool {
    // A constant (`0 * 4`) computes to a literal wherever it goes.
    struct Names(bool);
    impl<'a> Visit<'a> for Names {
        fn visit_identifier_reference(&mut self, _: &IdentifierReference<'a>) {
            self.0 = true;
        }
    }
    let mut names = Names(false);
    names.visit_expression(expression);
    if !names.0 {
        return false;
    }
    match expression.without_parentheses() {
        Expression::ConditionalExpression(_) => true,
        Expression::BinaryExpression(binary) => {
            matches!(
                binary.operator,
                BinaryOperator::Addition
                    | BinaryOperator::Subtraction
                    | BinaryOperator::Multiplication
                    | BinaryOperator::Division
            ) && (matches!(binary.right.without_parentheses(), Expression::NumericLiteral(_))
                || matches!(binary.left.without_parentheses(), Expression::NumericLiteral(_)))
        }
        _ => false,
    }
}

/// `a.b?.["c"]` and the like: property reads off a name with literal keys.
fn property_path(expression: &Expression<'_>) -> bool {
    match expression.without_parentheses() {
        Expression::ChainExpression(chain) => match &chain.expression {
            ChainElement::StaticMemberExpression(member) => property_path(&member.object),
            ChainElement::ComputedMemberExpression(member) => {
                matches!(member.expression, Expression::StringLiteral(_) | Expression::NumericLiteral(_))
                    && property_path(&member.object)
            }
            _ => false,
        },
        Expression::StaticMemberExpression(member) => matches!(member.object.without_parentheses(), Expression::Identifier(_)) || property_path(&member.object),
        Expression::ComputedMemberExpression(member) => {
            matches!(member.expression, Expression::StringLiteral(_) | Expression::NumericLiteral(_))
                && (matches!(member.object.without_parentheses(), Expression::Identifier(_)) || property_path(&member.object))
        }
        _ => false,
    }
}
