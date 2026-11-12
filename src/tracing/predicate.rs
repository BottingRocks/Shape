//! Deciding conditional-jump tests whose value is not a concrete fact.
//!
//! Shape's bytecode contains fake branches: a conditional jump whose test
//! always has the same outcome, while the other successor leads into code
//! that was never meant to run. Forking there traces garbage. A test is often
//! decidable from what is known about its operands even when they are program
//! values: an object literal is truthy, `typeof []` is `"object"`, a string is
//! never `===` a number, `x == null` is false when `x` is a number.
//!
//! This module evaluates a trace node to an abstract value, a set of possible
//! JavaScript types plus a concrete value when there is one, following `let`
//! bindings back to their values. It only answers when JavaScript semantics
//! guarantee the result; anything else stays undecided and the driver forks.
//! No global is assumed to be the built-in it is named after: a call's result
//! is unknown.

use std::collections::{BTreeMap, BTreeSet};

use crate::operations::templates::PrimitiveOperator;

use super::symbolic::{
    fold_binary, fold_unary, parse_number_text, same_fact, TraceArena, TraceNodeId,
};
use super::symbolic::{NodeMap, SymbolicValue, TraceNodeKind};

/// The kernel function that decodes a VM string constant.
const VM_DECODE_STRING: &str = "vmDecodeString";
/// The kernel's immutable table of encoded strings: every entry is a string.
const VM_STRING_TABLE: &str = "vmStringTable";

/// Nodes deeper than this are undecided.
const MAX_DEPTH: usize = 64;

/// The JavaScript types a value may have, as a bit set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Types(u16);

impl Types {
    const UNDEFINED: Self = Self(1);
    const NULL: Self = Self(1 << 1);
    const BOOLEAN: Self = Self(1 << 2);
    const NUMBER: Self = Self(1 << 3);
    const STRING: Self = Self(1 << 4);
    const BIGINT: Self = Self(1 << 5);
    const SYMBOL: Self = Self(1 << 6);
    /// An ordinary object: never `document.all`, so always truthy and never
    /// loosely equal to `null`.
    const OBJECT: Self = Self(1 << 7);
    const FUNCTION: Self = Self(1 << 8);
    const ANY: Self = Self((1 << 9) - 1);

    const fn or(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    const fn within(self, other: Self) -> bool {
        self.0 & !other.0 == 0
    }
    const fn disjoint(self, other: Self) -> bool {
        self.0 & other.0 == 0
    }

    /// Of `types`, those a list of constants can stand for: the primitive
    /// ones, when the value is a constant at all.
    const fn listable(types: Self) -> Self {
        if types.0 == Self::ANY.0 {
            Self(0)
        } else {
            Self(types.0 & !Self::OBJECTS.0)
        }
    }

    /// Types whose arithmetic never produces a BigInt.
    const NOT_BIGINT: Self =
        Self(Self::UNDEFINED.0 | Self::NULL.0 | Self::BOOLEAN.0 | Self::NUMBER.0 | Self::STRING.0);
    const NULLISH: Self = Self(Self::UNDEFINED.0 | Self::NULL.0);
    const OBJECTS: Self = Self(Self::OBJECT.0 | Self::FUNCTION.0);

    fn of(value: &SymbolicValue) -> Self {
        match value {
            SymbolicValue::Undefined => Self::UNDEFINED,
            SymbolicValue::Null => Self::NULL,
            SymbolicValue::Boolean(_) => Self::BOOLEAN,
            SymbolicValue::Number(_) => Self::NUMBER,
            SymbolicValue::String(_) => Self::STRING,
            SymbolicValue::Object { .. } => Self::OBJECTS,
            _ => Self::ANY,
        }
    }

    /// The types whose `typeof` is `name`.
    fn of_typeof_name(name: &str) -> Option<Self> {
        Some(match name {
            "undefined" => Self::UNDEFINED,
            "object" => Self::NULL.or(Self::OBJECT),
            "boolean" => Self::BOOLEAN,
            "number" => Self::NUMBER,
            "string" => Self::STRING,
            "bigint" => Self::BIGINT,
            "symbol" => Self::SYMBOL,
            "function" => Self::FUNCTION,
            _ => return None,
        })
    }

    /// Every string `typeof` of a value with these types can be.
    fn typeof_names(self) -> Vec<String> {
        [
            (Self::UNDEFINED, "undefined"),
            (Self::NULL.or(Self::OBJECT), "object"),
            (Self::BOOLEAN, "boolean"),
            (Self::NUMBER, "number"),
            (Self::STRING, "string"),
            (Self::BIGINT, "bigint"),
            (Self::SYMBOL, "symbol"),
            (Self::FUNCTION, "function"),
        ]
        .into_iter()
        .filter(|(types, _)| !self.disjoint(*types))
        .map(|(_, name)| name.to_string())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
    }
}

/// What is known about one node's value.
/// The most constants an abstract value lists: past that, only its types.
const MAX_VALUES: usize = 16;

#[derive(Debug, Clone)]
struct Abstract {
    fact: Option<SymbolicValue>,
    types: Types,
    /// The constants it can be, when there are a few: a join of constants,
    /// `typeof` of a value of a few types, what an operator makes of such.
    values: Option<Vec<SymbolicValue>>,
    /// The primitive types (`listed`) every value of which it can be is
    /// one of `some`, when `values` cannot list them all: a join of an
    /// object and a string constant lists the string.
    listed: Types,
    some: Vec<SymbolicValue>,
}

impl Abstract {
    fn unknown() -> Self {
        Self::of_type(Types::ANY)
    }
    fn of_type(types: Types) -> Self {
        Self {
            fact: None,
            types,
            values: None,
            listed: Types(0),
            some: Vec::new(),
        }
    }
    fn known(value: SymbolicValue) -> Self {
        let types = Types::of(&value);
        Self {
            listed: Types::listable(types),
            some: vec![value.clone()],
            types,
            values: Some(vec![value.clone()]),
            fact: Some(value),
        }
    }
    /// A value that is one of `values`.
    fn one_of(values: Vec<SymbolicValue>) -> Self {
        let mut distinct: Vec<SymbolicValue> = Vec::new();
        for value in values {
            if !distinct.iter().any(|known| same_fact(known, &value)) {
                distinct.push(value);
            }
        }
        let types = distinct
            .iter()
            .fold(Types(0), |types, value| types.or(Types::of(value)));
        match distinct.len() {
            0 => Self::unknown(),
            1 => Self::known(distinct.remove(0)),
            n if n > MAX_VALUES => Self::of_type(types),
            _ => Self {
                fact: None,
                types,
                listed: if distinct.iter().any(|value| Types::of(value) == Types::ANY) {
                    Types(0)
                } else {
                    Types::listable(types)
                },
                some: distinct.clone(),
                values: Some(distinct),
            },
        }
    }
    fn boolean(value: Option<bool>) -> Self {
        value.map_or(Self::of_type(Types::BOOLEAN), |value| {
            Self::known(SymbolicValue::Boolean(value))
        })
    }

    /// What is known of the value once it is known to have one of `types`.
    fn narrowed(self, types: Types) -> Self {
        let narrowed = Types(self.types.0 & types.0);
        if narrowed == self.types || narrowed.0 == 0 {
            return self;
        }
        match self.values {
            Some(values) => Self::one_of(
                values
                    .into_iter()
                    .filter(|value| !Types::of(value).disjoint(narrowed))
                    .collect(),
            ),
            None => {
                let listed = Types(self.listed.0 & narrowed.0);
                Self {
                    some: self
                        .some
                        .into_iter()
                        .filter(|value| !Types::of(value).disjoint(listed))
                        .collect(),
                    listed,
                    ..Self::of_type(narrowed)
                }
            }
        }
    }

    /// What is known of a value that is either `self` or `other`.
    fn union(&self, other: &Self) -> Self {
        match (&self.values, &other.values) {
            (Some(left), Some(right)) => Self::one_of(left.iter().chain(right).cloned().collect()),
            _ => {
                // A type stays listed when every side that may have it lists it.
                let unlisted = Types(
                    (self.types.0 & !self.listed.0) | (other.types.0 & !other.listed.0),
                );
                let listed = Types((self.listed.0 | other.listed.0) & !unlisted.0);
                let mut some: Vec<SymbolicValue> = Vec::new();
                for value in self.some.iter().chain(&other.some) {
                    if !Types::of(value).disjoint(listed)
                        && !some.iter().any(|known| same_fact(known, value))
                    {
                        some.push(value.clone());
                    }
                }
                if some.len() > MAX_VALUES {
                    return Self::of_type(self.types.or(other.types));
                }
                Self {
                    listed,
                    some,
                    ..Self::of_type(self.types.or(other.types))
                }
            }
        }
    }

    /// ToBoolean, when it is guaranteed.
    fn truthiness(&self) -> Option<bool> {
        if let Some(values) = &self.values {
            let truthy: Option<Vec<bool>> = values
                .iter()
                .map(
                    |value| match fold_unary(PrimitiveOperator::LogicalNot, Some(value)) {
                        Some(SymbolicValue::Boolean(falsy)) => Some(!falsy),
                        _ => None,
                    },
                )
                .collect();
            if let Some(truthy) = truthy {
                if truthy.iter().all(|&value| value == truthy[0]) {
                    return truthy.first().copied();
                }
            }
        }
        let by_types = |types: Types| {
            if types.within(Types::OBJECTS.or(Types::SYMBOL)) {
                Some(true)
            } else if types.within(Types::NULLISH) {
                Some(false)
            } else {
                None
            }
        };
        // A value of an unlisted type, or one of the constants listed for
        // the others (a join of a made object and `true`): decided when
        // every one of those is truthy alike.
        let unlisted = Types(self.types.0 & !self.listed.0);
        if self.values.is_none() && self.listed.0 != 0 && unlisted.0 != 0 {
            let listed = self
                .some
                .iter()
                .filter(|value| !Types::of(value).disjoint(self.listed))
                .map(
                    |value| match fold_unary(PrimitiveOperator::LogicalNot, Some(value)) {
                        Some(SymbolicValue::Boolean(falsy)) => Some(!falsy),
                        _ => None,
                    },
                );
            let truthy = by_types(unlisted);
            if truthy.is_some() && listed.clone().all(|value| value == truthy) {
                return truthy;
            }
        }
        by_types(self.types)
    }
}

/// Decide whether `test` is truthy, when that is guaranteed. `aliases` maps
/// each `let`-bound identifier node to its value node, `joins` each variable
/// a join made to the value each arm assigned it.
/// Decide whether `test` is truthy, when that is guaranteed (see the module
/// documentation), with the variables whose types it read when it decided
/// (or, undecided, those it read with no types known).
pub(super) fn decide_typed(
    arena: &TraceArena,
    aliases: &NodeMap<TraceNodeId>,
    joins: &BTreeMap<TraceNodeId, Vec<TraceNodeId>>,
    returns: &BTreeMap<TraceNodeId, Vec<TraceNodeId>>,
    type_facts: &BTreeMap<String, u16>,
    concrete: Option<&BTreeMap<TraceNodeId, TraceNodeId>>,
    test: TraceNodeId,
) -> (Option<bool>, Vec<String>) {
    let mut evaluator = Evaluator {
        arena,
        aliases,
        joins,
        returns,
        type_facts,
        concrete,
        sets: None,
        used: Vec::new(),
        typed: Vec::new(),
        wanted: Vec::new(),
        memo: BTreeMap::new(),
    };
    let decided = evaluator.evaluate(test, 0).truthiness();
    match decided {
        Some(_) => (decided, evaluator.typed),
        // The variables whose types might have decided it, marked apart.
        None => (
            decided,
            evaluator
                .wanted
                .into_iter()
                .map(|name| format!("wanted {name}"))
                .collect(),
        ),
    }
}

/// The identity of a branch test: a key naming exactly the value it tests,
/// and whether the test is that value negated. Two tests with the same key
/// test the same value: they are built from the same traced values with the
/// same operators, so a test repeated on one path has one outcome. Leaves are
/// node identities after following `let` bindings, never source text, so two
/// separately evaluated calls are different values.
pub(super) fn test_key(
    arena: &TraceArena,
    aliases: &NodeMap<TraceNodeId>,
    test: TraceNodeId,
) -> Option<(String, bool)> {
    let mut node = resolve_alias(aliases, test);
    let mut negated = false;
    while let Some(TraceNodeKind::Unary {
        operator: PrimitiveOperator::LogicalNot,
        value,
    }) = arena.nodes.get(node as usize).map(|node| &node.kind)
    {
        negated = !negated;
        node = resolve_alias(aliases, *value);
    }
    Some((value_key(arena, aliases, node, 0)?, negated))
}

/// A type test: `typeof value` compared (`===`, `==`, `!==`, `!=`, either
/// way round) with a type's name, or `value` compared (`<`, `<=`, `>`, `>=`)
/// with a number, which `undefined` never passes (it compares as `NaN`);
/// possibly negated, or a call of an outlined function that returns one.
/// Returns the tested value's node and the types it has where the test is
/// truthy and where it is falsy: each side of a branch on it knows the
/// value's type that much better.
pub(super) fn type_test(
    arena: &TraceArena,
    aliases: &NodeMap<TraceNodeId>,
    returns: &BTreeMap<TraceNodeId, Vec<TraceNodeId>>,
    test: TraceNodeId,
) -> Option<(TraceNodeId, u16, u16)> {
    let kind = |node: TraceNodeId| arena.nodes.get(node as usize).map(|node| &node.kind);
    let mut node = resolve_alias(aliases, test);
    let mut negated = false;
    for _ in 0..MAX_DEPTH {
        match kind(node)? {
            TraceNodeKind::Unary {
                operator: PrimitiveOperator::LogicalNot,
                value,
            } => {
                negated = !negated;
                node = resolve_alias(aliases, *value);
            }
            TraceNodeKind::Call { .. } if returns.get(&node).is_some_and(|r| r.len() == 1) => {
                node = resolve_alias(aliases, returns[&node][0]);
            }
            _ => break,
        }
    }
    let TraceNodeKind::Binary {
        operator,
        left,
        right,
    } = kind(node)?
    else {
        return None;
    };
    let number = |side: TraceNodeId| {
        matches!(
            kind(resolve_alias(aliases, side)),
            Some(TraceNodeKind::Number(_))
        )
    };
    let equal = match operator {
        PrimitiveOperator::StrictEqual | PrimitiveOperator::Equal => true,
        PrimitiveOperator::StrictNotEqual | PrimitiveOperator::NotEqual => false,
        PrimitiveOperator::LessThan
        | PrimitiveOperator::LessThanOrEqual
        | PrimitiveOperator::GreaterThan
        | PrimitiveOperator::GreaterThanOrEqual => {
            let value = match (number(*left), number(*right)) {
                (true, false) => *right,
                (false, true) => *left,
                _ => return None,
            };
            let defined = Types::ANY.0 & !Types::UNDEFINED.0;
            return Some(if negated {
                (value, Types::ANY.0, defined)
            } else {
                (value, defined, Types::ANY.0)
            });
        }
        _ => return None,
    };
    let typeof_of = |side: TraceNodeId| match kind(resolve_alias(aliases, side)) {
        Some(TraceNodeKind::Unary {
            operator: PrimitiveOperator::Typeof,
            value,
        }) => Some(*value),
        _ => None,
    };
    let name_of = |side: TraceNodeId| match kind(resolve_alias(aliases, side)) {
        Some(TraceNodeKind::String(name)) => Types::of_typeof_name(name),
        _ => None,
    };
    let (value, types) = match (typeof_of(*left), typeof_of(*right)) {
        (Some(value), None) => (value, name_of(*right)?),
        (None, Some(value)) => (value, name_of(*left)?),
        _ => return None,
    };
    let other = Types(Types::ANY.0 & !types.0);
    let (truthy, falsy) = if equal ^ negated {
        (types, other)
    } else {
        (other, types)
    };
    Some((value, truthy.0, falsy.0))
}

/// Whether `test` has the shape of the kernels' opaque predicates: a
/// `typeof` compared with a type's name, or a value compared for equality
/// with a string, number or boolean, possibly negated or returned by an
/// outlined function. A `== null` test is not: of a cell it is real code's
/// lazy initialisation as often (a closure fills the cell).
/// Only such a test is decided on what the path concretely wrote.
pub(super) fn opaque_shape(
    arena: &TraceArena,
    aliases: &NodeMap<TraceNodeId>,
    returns: &BTreeMap<TraceNodeId, Vec<TraceNodeId>>,
    test: TraceNodeId,
) -> bool {
    let kind = |node: TraceNodeId| arena.nodes.get(node as usize).map(|node| &node.kind);
    let mut node = resolve_alias(aliases, test);
    for _ in 0..MAX_DEPTH {
        match kind(node) {
            Some(TraceNodeKind::Unary {
                operator: PrimitiveOperator::LogicalNot,
                value,
            }) => node = resolve_alias(aliases, *value),
            Some(TraceNodeKind::Call { .. })
                if returns.get(&node).is_some_and(|r| r.len() == 1) =>
            {
                node = resolve_alias(aliases, returns[&node][0]);
            }
            _ => break,
        }
    }
    let Some(TraceNodeKind::Binary {
        operator:
            PrimitiveOperator::StrictEqual
            | PrimitiveOperator::Equal
            | PrimitiveOperator::StrictNotEqual
            | PrimitiveOperator::NotEqual,
        left,
        right,
    }) = kind(node)
    else {
        return false;
    };
    let typeof_of = |side: TraceNodeId| {
        matches!(
            kind(resolve_alias(aliases, side)),
            Some(TraceNodeKind::Unary {
                operator: PrimitiveOperator::Typeof,
                ..
            })
        )
    };
    let name = |side: TraceNodeId| {
        matches!(
            kind(resolve_alias(aliases, side)),
            Some(TraceNodeKind::String(_))
        )
    };
    // A string, number or boolean: `null` and `undefined` are left out
    // (see above).
    let literal = |side: TraceNodeId| {
        matches!(
            kind(resolve_alias(aliases, side)),
            Some(TraceNodeKind::String(_) | TraceNodeKind::Number(_) | TraceNodeKind::Boolean(_))
        )
    };
    (typeof_of(*left) && name(*right))
        || (name(*left) && typeof_of(*right))
        || literal(*left)
        || literal(*right)
}

/// [`decide`] for an opaque predicate's shape (see [`opaque_shape`]), on
/// what the path concretely put in cells and the value sets of its loop and
/// `try` variables. Returns the variables whose sets it read: a set that
/// grows later was incomplete for the decision.
#[allow(clippy::too_many_arguments)]
pub(super) fn decide_opaque(
    arena: &TraceArena,
    aliases: &NodeMap<TraceNodeId>,
    joins: &BTreeMap<TraceNodeId, Vec<TraceNodeId>>,
    returns: &BTreeMap<TraceNodeId, Vec<TraceNodeId>>,
    type_facts: &BTreeMap<String, u16>,
    concrete: &BTreeMap<TraceNodeId, TraceNodeId>,
    sets: &BTreeMap<String, std::sync::Arc<[SymbolicValue]>>,
    test: TraceNodeId,
) -> (Option<bool>, Vec<String>) {
    let mut evaluator = Evaluator {
        arena,
        aliases,
        joins,
        returns,
        type_facts,
        concrete: Some(concrete),
        sets: Some(sets),
        used: Vec::new(),
        typed: Vec::new(),
        wanted: Vec::new(),
        memo: BTreeMap::new(),
    };
    let decided = evaluator.evaluate(test, 0).truthiness();
    let mut used = evaluator.used;
    used.extend(evaluator.typed);
    used.sort();
    used.dedup();
    (decided, if decided.is_some() { used } else { Vec::new() })
}

/// Whether `node`, concretely (see [`decide_opaque`]), has a type that is
/// neither a number nor `undefined`: no VM position, nor the `undefined` a
/// dispatch jumps to to throw.
pub(super) fn never_a_position(
    arena: &TraceArena,
    aliases: &NodeMap<TraceNodeId>,
    joins: &BTreeMap<TraceNodeId, Vec<TraceNodeId>>,
    returns: &BTreeMap<TraceNodeId, Vec<TraceNodeId>>,
    type_facts: &BTreeMap<String, u16>,
    concrete: &BTreeMap<TraceNodeId, TraceNodeId>,
    node: TraceNodeId,
) -> bool {
    let types = Evaluator {
        arena,
        aliases,
        joins,
        returns,
        type_facts,
        concrete: Some(concrete),
        sets: None,
        used: Vec::new(),
        typed: Vec::new(),
        wanted: Vec::new(),
        memo: BTreeMap::new(),
    }
    .evaluate(node, 0)
    .types;
    types.0 != 0 && types.disjoint(Types::NUMBER.or(Types::UNDEFINED))
}

/// The types `node` may have, as [`type_test`]'s bit sets.
pub(super) fn value_types(
    arena: &TraceArena,
    aliases: &NodeMap<TraceNodeId>,
    joins: &BTreeMap<TraceNodeId, Vec<TraceNodeId>>,
    returns: &BTreeMap<TraceNodeId, Vec<TraceNodeId>>,
    type_facts: &BTreeMap<String, u16>,
    node: TraceNodeId,
) -> u16 {
    Evaluator {
        arena,
        aliases,
        joins,
        returns,
        type_facts,
        concrete: None,
        sets: None,
        used: Vec::new(),
        typed: Vec::new(),
        wanted: Vec::new(),
        memo: BTreeMap::new(),
    }
    .evaluate(node, 0)
    .types
    .0
}

/// Every type.
pub(super) const ANY_TYPES: u16 = Types::ANY.0;

/// The types of the known value `value`, as [`type_test`]'s bit sets.
pub(super) fn fact_types(value: &SymbolicValue) -> u16 {
    Types::of(value).0
}

/// The type facts' key for every read of the variable `name`: for a
/// variable all of whose values are known (a `try` variable, whose values
/// are its entry's and the body's), true wherever it is read.
pub(super) fn variable_key(name: &str) -> String {
    format!("variable {name}")
}

/// A `null` test: `value == null` (`===`, `!=`, `!==`, against `null` or
/// `undefined`, either way round), or a test of `value` itself (its
/// truthiness), possibly negated or returned by an outlined function, whose
/// `value` is concretely (`concrete`, see [`opaque_shape`]) an object or
/// function the path made: never nullish, always truthy.
/// The exception to `opaque_shape` leaving `null` tests out: a cell real
/// code fills lazily holds `undefined` or what a closure put there, not an
/// object this path made.
pub(super) fn made_object_null_test(
    arena: &TraceArena,
    aliases: &NodeMap<TraceNodeId>,
    returns: &BTreeMap<TraceNodeId, Vec<TraceNodeId>>,
    concrete: &BTreeMap<TraceNodeId, TraceNodeId>,
    test: TraceNodeId,
) -> bool {
    let kind = |node: TraceNodeId| arena.nodes.get(node as usize).map(|node| &node.kind);
    let mut node = resolve_alias(aliases, test);
    for _ in 0..MAX_DEPTH {
        match kind(node) {
            Some(TraceNodeKind::Unary {
                operator: PrimitiveOperator::LogicalNot,
                value,
            }) => node = resolve_alias(aliases, *value),
            Some(TraceNodeKind::Call { .. })
                if returns.get(&node).is_some_and(|r| r.len() == 1) =>
            {
                node = resolve_alias(aliases, returns[&node][0]);
            }
            _ => break,
        }
    }
    let nullish = |side: TraceNodeId| {
        matches!(
            kind(resolve_alias(aliases, side)),
            Some(TraceNodeKind::Null | TraceNodeKind::Undefined)
        )
    };
    let value = match kind(node) {
        Some(TraceNodeKind::Binary {
            operator:
                PrimitiveOperator::StrictEqual
                | PrimitiveOperator::Equal
                | PrimitiveOperator::StrictNotEqual
                | PrimitiveOperator::NotEqual,
            left,
            right,
        }) => match (nullish(*left), nullish(*right)) {
            (false, true) => *left,
            (true, false) => *right,
            _ => return false,
        },
        // Its truthiness (`if (v)`, `!v`): the same question, an object is
        // never nullish and always truthy.
        Some(TraceNodeKind::Binary { .. } | TraceNodeKind::Unary { .. }) | None => return false,
        Some(_) => node,
    };
    // What the path concretely put there, a node it made.
    let made = resolve_held(aliases, concrete, value);
    matches!(
        kind(made),
        Some(
            TraceNodeKind::VmFunction { .. }
                | TraceNodeKind::Function { .. }
                | TraceNodeKind::ObjectLiteral { .. }
                | TraceNodeKind::ArrayLiteral { .. }
        )
    )
}

/// A strict or loose `null` test: `value === null`, `value !== undefined`,
/// `value == null` (either way round), or a test of `value` itself (its
/// truthiness), possibly negated or returned by an outlined function. Gives
/// `value` and the types the test asks about: a value of none of them
/// makes the test come out one way (an object is always truthy).
pub(super) fn null_test(
    arena: &TraceArena,
    aliases: &NodeMap<TraceNodeId>,
    returns: &BTreeMap<TraceNodeId, Vec<TraceNodeId>>,
    test: TraceNodeId,
) -> Option<(TraceNodeId, u16)> {
    let kind = |node: TraceNodeId| arena.nodes.get(node as usize).map(|node| &node.kind);
    let mut node = resolve_alias(aliases, test);
    for _ in 0..MAX_DEPTH {
        match kind(node) {
            Some(TraceNodeKind::Unary {
                operator: PrimitiveOperator::LogicalNot,
                value,
            }) => node = resolve_alias(aliases, *value),
            Some(TraceNodeKind::Call { .. })
                if returns.get(&node).is_some_and(|r| r.len() == 1) =>
            {
                node = resolve_alias(aliases, returns[&node][0]);
            }
            _ => break,
        }
    }
    let (operator, left, right) = match kind(node) {
        Some(TraceNodeKind::Binary {
            operator,
            left,
            right,
        }) => (operator, left, right),
        Some(TraceNodeKind::Unary { .. }) | None => return None,
        Some(_) => return Some((node, Types::ANY.0 & !Types::OBJECTS.0)),
    };
    let strict = match operator {
        PrimitiveOperator::StrictEqual | PrimitiveOperator::StrictNotEqual => true,
        PrimitiveOperator::Equal | PrimitiveOperator::NotEqual => false,
        _ => return None,
    };
    let nullish = |side: TraceNodeId| match kind(resolve_alias(aliases, side)) {
        Some(TraceNodeKind::Null) => Some(Types::NULL),
        Some(TraceNodeKind::Undefined) => Some(Types::UNDEFINED),
        _ => None,
    };
    let (value, literal) = match (nullish(*left), nullish(*right)) {
        (None, Some(literal)) => (*left, literal),
        (Some(literal), None) => (*right, literal),
        _ => return None,
    };
    Some((value, if strict { literal.0 } else { Types::NULLISH.0 }))
}

/// The types `node` may have, reading a shared cell's read as what the path
/// concretely put there (see [`decide_opaque`]).
pub(super) fn concrete_types(
    arena: &TraceArena,
    aliases: &NodeMap<TraceNodeId>,
    joins: &BTreeMap<TraceNodeId, Vec<TraceNodeId>>,
    returns: &BTreeMap<TraceNodeId, Vec<TraceNodeId>>,
    type_facts: &BTreeMap<String, u16>,
    concrete: &BTreeMap<TraceNodeId, TraceNodeId>,
    node: TraceNodeId,
) -> u16 {
    Evaluator {
        arena,
        aliases,
        joins,
        returns,
        type_facts,
        concrete: Some(concrete),
        sets: None,
        used: Vec::new(),
        typed: Vec::new(),
        wanted: Vec::new(),
        memo: BTreeMap::new(),
    }
    .evaluate(node, 0)
    .types
    .0
}

/// Narrow `facts` by a branch on `test`: on the side where it is `truthy`.
/// The fact is kept for the value itself and, for a read of a shared
/// cell, for what the path concretely put there (see [`decide_opaque`]): a
/// later read of the cell is that value too.
pub(super) fn learn_type(
    arena: &TraceArena,
    aliases: &NodeMap<TraceNodeId>,
    returns: &BTreeMap<TraceNodeId, Vec<TraceNodeId>>,
    concrete: &BTreeMap<TraceNodeId, TraceNodeId>,
    facts: &mut std::sync::Arc<BTreeMap<String, u16>>,
    test: TraceNodeId,
    truthy: bool,
) {
    let Some((value, when_truthy, when_falsy)) = type_test(arena, aliases, returns, test) else {
        return;
    };
    let types = if truthy { when_truthy } else { when_falsy };
    if types == Types::ANY.0 {
        return;
    }
    let held = resolve_held(aliases, concrete, value);
    for node in [value, held] {
        if let Some(key) = value_key(arena, aliases, node, 0) {
            let known = facts.get(&key).copied().unwrap_or(Types::ANY.0);
            std::sync::Arc::make_mut(facts).insert(key, known & types);
        }
    }
}

/// Follow `let` bindings to the node that computes the value. A chain of
/// bindings is no nesting of expressions: a value rebound once per element
/// written (an array of fifty readback samples) is a chain longer than
/// [`MAX_DEPTH`], so it is followed to its end. Each step follows one entry,
/// so a chain without a cycle ends within [`NodeMap::span`] steps.
fn resolve_alias(aliases: &NodeMap<TraceNodeId>, mut node: TraceNodeId) -> TraceNodeId {
    for _ in 0..=aliases.span() {
        match aliases.get(&node) {
            Some(value) => node = *value,
            None => break,
        }
    }
    node
}

/// [`resolve_alias`], also following a read of a shared cell to what the
/// path concretely put there (see [`decide_opaque`]).
fn resolve_held(
    aliases: &NodeMap<TraceNodeId>,
    concrete: &BTreeMap<TraceNodeId, TraceNodeId>,
    mut node: TraceNodeId,
) -> TraceNodeId {
    for _ in 0..=aliases.span() + concrete.len() {
        match concrete.get(&node).or_else(|| aliases.get(&node)) {
            Some(next) => node = *next,
            None => break,
        }
    }
    node
}

fn value_key(
    arena: &TraceArena,
    aliases: &NodeMap<TraceNodeId>,
    node: TraceNodeId,
    depth: usize,
) -> Option<String> {
    if depth > MAX_DEPTH {
        return None;
    }
    let node = resolve_alias(aliases, node);
    let key = |child: TraceNodeId| value_key(arena, aliases, child, depth + 1);
    Some(match &arena.nodes.get(node as usize)?.kind {
        TraceNodeKind::Undefined => "undefined".into(),
        TraceNodeKind::Null => "null".into(),
        TraceNodeKind::Boolean(value) => value.to_string(),
        TraceNodeKind::Number(text) => text.clone(),
        TraceNodeKind::String(text) => format!("{text:?}"),
        TraceNodeKind::Binary {
            operator,
            left,
            right,
        } => format!("({} {operator:?} {})", key(*left)?, key(*right)?),
        TraceNodeKind::Unary { operator, value } => format!("({operator:?} {})", key(*value)?),
        _ => format!("#{node}"),
    })
}

struct Evaluator<'a> {
    arena: &'a TraceArena,
    aliases: &'a NodeMap<TraceNodeId>,
    joins: &'a BTreeMap<TraceNodeId, Vec<TraceNodeId>>,
    /// What calls of outlined functions return (see `call_returns` in the
    /// executor).
    returns: &'a BTreeMap<TraceNodeId, Vec<TraceNodeId>>,
    /// The types this path's type tests left each value (see [`type_test`]),
    /// by [`value_key`].
    type_facts: &'a BTreeMap<String, u16>,
    /// Reads of shared cells to what the path concretely last put there.
    concrete: Option<&'a BTreeMap<TraceNodeId, TraceNodeId>>,
    /// The value sets of loop and `try` variables, and the names whose set
    /// a decision read (see [`decide_opaque`]).
    sets: Option<&'a BTreeMap<String, std::sync::Arc<[SymbolicValue]>>>,
    used: Vec<String>,
    /// The variables whose types (see [`variable_key`]) a decision read.
    typed: Vec<String>,
    /// The variables a test reads whose types are not known.
    wanted: Vec<String>,
    memo: BTreeMap<TraceNodeId, Abstract>,
}

impl Evaluator<'_> {
    /// Follow `let` bindings to the node that computes the value.
    fn resolve(&self, mut node: TraceNodeId) -> TraceNodeId {
        let bound = self.aliases.span() + self.concrete.map_or(0, BTreeMap::len);
        for _ in 0..=bound {
            // What the path concretely put in a cell before the read's own
            // binding: the read is that value.
            match self
                .concrete
                .and_then(|concrete| concrete.get(&node))
                .or_else(|| self.aliases.get(&node))
            {
                Some(value) => node = *value,
                None => break,
            }
        }
        node
    }

    fn evaluate(&mut self, node: TraceNodeId, depth: usize) -> Abstract {
        let node = self.resolve(node);
        if let Some(known) = self.memo.get(&node) {
            return known.clone();
        }
        let mut value = if depth > MAX_DEPTH {
            Abstract::unknown()
        } else {
            self.compute(node, depth + 1)
        };
        if !self.type_facts.is_empty() {
            let known = value_key(self.arena, self.aliases, node, 0)
                .and_then(|key| self.type_facts.get(&key).copied());
            if let Some(types) = known {
                value = value.narrowed(Types(types));
            }
            match self.arena.nodes.get(node as usize).map(|node| &node.kind) {
                Some(TraceNodeKind::Identifier { name }) => {
                    match self.type_facts.get(&variable_key(name)) {
                        Some(&types) => {
                            value = value.narrowed(Types(types));
                            self.typed.push(format!("typed {name}"));
                        }
                        None => self.wanted.push(name.clone()),
                    }
                }
                _ => {}
            }
        }
        self.memo.insert(node, value.clone());
        value
    }

    /// The type `Number`, `String` or `Boolean` called as `callee` returns.
    fn conversion(&self, callee: TraceNodeId) -> Option<Types> {
        match self
            .arena
            .nodes
            .get(self.resolve(callee) as usize)
            .map(|node| &node.kind)
        {
            Some(TraceNodeKind::Global { name }) => match name.as_str() {
                "Number" => Some(Types::NUMBER),
                "String" => Some(Types::STRING),
                "Boolean" => Some(Types::BOOLEAN),
                _ => None,
            },
            _ => None,
        }
    }

    fn compute(&mut self, node: TraceNodeId, depth: usize) -> Abstract {
        let Some(kind) = self.arena.nodes.get(node as usize).map(|node| &node.kind) else {
            return Abstract::unknown();
        };
        match kind {
            TraceNodeKind::Undefined => Abstract::known(SymbolicValue::Undefined),
            TraceNodeKind::Null => Abstract::known(SymbolicValue::Null),
            TraceNodeKind::Boolean(value) => Abstract::known(SymbolicValue::Boolean(*value)),
            TraceNodeKind::Number(text) => parse_number_text(text)
                .map_or_else(Abstract::unknown, |value| {
                    Abstract::known(SymbolicValue::Number(value))
                }),
            TraceNodeKind::String(value) => Abstract::known(SymbolicValue::String(value.clone())),
            TraceNodeKind::Object { .. } => Abstract::of_type(Types::OBJECTS),
            TraceNodeKind::ObjectLiteral { .. } | TraceNodeKind::ArrayLiteral { .. } => {
                Abstract::of_type(Types::OBJECT)
            }
            TraceNodeKind::VmFunction { .. } | TraceNodeKind::Function { .. } => {
                Abstract::of_type(Types::FUNCTION)
            }
            // A variable a join made holds what one of its arms assigned.
            // A loop or `try` variable with a known value set: one of them.
            TraceNodeKind::Identifier { name }
                if self.sets.is_some_and(|sets| sets.contains_key(name)) =>
            {
                let set = self.sets.expect("checked")[name].to_vec();
                self.used.push(format!("set {name}"));
                Abstract::one_of(set)
            }
            TraceNodeKind::Identifier { .. } if self.joins.contains_key(&node) => {
                let arms = self.joins[&node].clone();
                let mut values = arms.iter().map(|arm| self.evaluate(*arm, depth));
                let first = values.next().unwrap_or_else(Abstract::unknown);
                values.fold(first, |merged, value| merged.union(&value))
            }
            // A constructor call returns an object or throws.
            TraceNodeKind::New { .. } => Abstract::of_type(Types::OBJECTS),
            // A call of an outlined function returns what its body returns
            // with the call's arguments.
            TraceNodeKind::Call { .. } if self.returns.get(&node).is_some_and(|r| r.len() == 1) => {
                let value = self.returns[&node][0];
                self.evaluate(value, depth)
            }
            // The kernel's string decoder returns the decoded string, built
            // with `+=` from `""` or taken from its private cache of those,
            // or throws.
            TraceNodeKind::Call { callee, .. }
                if matches!(
                    self.arena.nodes.get(self.resolve(*callee) as usize).map(|node| &node.kind),
                    Some(TraceNodeKind::Global { name }) if name == VM_DECODE_STRING
                ) =>
            {
                Abstract::of_type(Types::STRING)
            }
            // The standard conversions, called, return their primitive type
            // or throw.
            TraceNodeKind::Call { callee, .. } if self.conversion(*callee).is_some() => {
                Abstract::of_type(self.conversion(*callee).expect("a conversion"))
            }
            TraceNodeKind::Unary { operator, value } => {
                let value = self.evaluate(*value, depth);
                unary(*operator, &value)
            }
            TraceNodeKind::Binary {
                operator,
                left,
                right,
            } => {
                let same = self.resolve(*left) == self.resolve(*right);
                let left = self.evaluate(*left, depth);
                let right = self.evaluate(*right, depth);
                binary(*operator, &left, &right, same)
            }
            // One of the values an outlined function returns.
            TraceNodeKind::PropertyRead { object, key }
                if self
                    .returns
                    .get(&self.resolve(*object))
                    .is_some_and(|r| r.len() > 1)
                    && matches!(
                        self.arena
                            .nodes
                            .get(self.resolve(*key) as usize)
                            .map(|node| &node.kind),
                        Some(TraceNodeKind::Number(_))
                    ) =>
            {
                let returns = &self.returns[&self.resolve(*object)];
                let index = match &self.arena.nodes[self.resolve(*key) as usize].kind {
                    TraceNodeKind::Number(text) => parse_number_text(text),
                    _ => None,
                };
                match index.filter(|index| index.fract() == 0.0 && *index >= 0.0) {
                    Some(index) if (index as usize) < returns.len() => {
                        let value = returns[index as usize];
                        self.evaluate(value, depth)
                    }
                    _ => Abstract::unknown(),
                }
            }
            // An entry of the kernel's string table, at an index the walk
            // does not know: a string all the same.
            TraceNodeKind::PropertyRead { object, .. }
                if matches!(
                    self.arena.nodes.get(self.resolve(*object) as usize).map(|node| &node.kind),
                    Some(TraceNodeKind::Global { name }) if name == VM_STRING_TABLE
                ) =>
            {
                Abstract::of_type(Types::STRING)
            }
            TraceNodeKind::PropertyRead { object, key } => {
                let object = self.evaluate(*object, depth);
                let key = self.evaluate(*key, depth);
                match (object.types.within(Types::STRING), &key.fact) {
                    (true, Some(SymbolicValue::String(key))) if key == "length" => {
                        Abstract::of_type(Types::NUMBER)
                    }
                    _ => Abstract::unknown(),
                }
            }
            _ => Abstract::unknown(),
        }
    }
}

fn unary(operator: PrimitiveOperator, value: &Abstract) -> Abstract {
    if let Some(fact) = fold_unary(operator, value.fact.as_ref()) {
        return Abstract::known(fact);
    }
    if let Some(values) = &value.values {
        let folded: Option<Vec<SymbolicValue>> = values
            .iter()
            .map(|value| fold_unary(operator, Some(value)))
            .collect();
        if let Some(folded) = folded {
            return Abstract::one_of(folded);
        }
    }
    match operator {
        PrimitiveOperator::LogicalNot => Abstract::boolean(value.truthiness().map(|v| !v)),
        PrimitiveOperator::Typeof => Abstract::one_of(
            value
                .types
                .typeof_names()
                .into_iter()
                .map(SymbolicValue::String)
                .collect(),
        ),
        PrimitiveOperator::Void => Abstract::known(SymbolicValue::Undefined),
        PrimitiveOperator::UnaryPlus => Abstract::of_type(Types::NUMBER),
        _ if value.types.within(Types::NOT_BIGINT) => Abstract::of_type(Types::NUMBER),
        _ => Abstract::of_type(Types::NUMBER.or(Types::BIGINT)),
    }
}

/// `same` is true when both operands are the same node, so the same value.
fn binary(operator: PrimitiveOperator, left: &Abstract, right: &Abstract, same: bool) -> Abstract {
    if let Some(fact) = fold_binary(operator, left.fact.as_ref(), right.fact.as_ref()) {
        return Abstract::known(fact);
    }
    // Each constant one side can be with each the other can be (the same
    // node is one value on both sides).
    if let (Some(lefts), Some(rights)) = (&left.values, &right.values) {
        if lefts.len() * rights.len() <= MAX_VALUES * MAX_VALUES {
            let pairs: Vec<(&SymbolicValue, &SymbolicValue)> = if same {
                lefts.iter().zip(lefts).collect()
            } else {
                lefts
                    .iter()
                    .flat_map(|l| rights.iter().map(move |r| (l, r)))
                    .collect()
            };
            let folded: Option<Vec<SymbolicValue>> = pairs
                .into_iter()
                .map(|(l, r)| {
                    // Two strings compare loosely as strictly.
                    let operator = match (operator, l, r) {
                        (
                            PrimitiveOperator::Equal,
                            SymbolicValue::String(_),
                            SymbolicValue::String(_),
                        ) => PrimitiveOperator::StrictEqual,
                        (
                            PrimitiveOperator::NotEqual,
                            SymbolicValue::String(_),
                            SymbolicValue::String(_),
                        ) => PrimitiveOperator::StrictNotEqual,
                        _ => operator,
                    };
                    fold_binary(operator, Some(l), Some(r))
                })
                .collect();
            if let Some(folded) = folded {
                return Abstract::one_of(folded);
            }
        }
    }
    match operator {
        PrimitiveOperator::JsAdd => {
            if left.types.within(Types::STRING) || right.types.within(Types::STRING) {
                Abstract::of_type(Types::STRING)
            } else if left.types.within(Types::NOT_BIGINT.or(Types::NULLISH))
                && right.types.within(Types::NOT_BIGINT.or(Types::NULLISH))
                && left.types.disjoint(Types::STRING)
                && right.types.disjoint(Types::STRING)
            {
                Abstract::of_type(Types::NUMBER)
            } else {
                Abstract::of_type(Types::NUMBER.or(Types::STRING).or(Types::BIGINT))
            }
        }
        PrimitiveOperator::ShiftRightZeroFill => Abstract::of_type(Types::NUMBER),
        // `a - b` is a BigInt only when both are: a BigInt with any other
        // type throws. Two values the walk does not know are taken not to
        // both be BigInts. The kernels subtract numbers (timings, lengths,
        // offsets), and their opaque predicates count on it: `typeof
        // (performance.timing[e] - performance.timing.navigationStart) !==
        // "number"` guards junk, which no browser value takes.
        PrimitiveOperator::Subtract => {
            if left.types.within(Types::BIGINT) && right.types.within(Types::BIGINT) {
                Abstract::of_type(Types::BIGINT)
            } else {
                Abstract::of_type(Types::NUMBER)
            }
        }
        // Bit operations and the other arithmetic likewise: the kernels
        // mask, shift, scale and combine numbers (`h | b << s`, `t * 1e3`),
        // and a value they compute that way is a number to their opaque
        // predicates.
        PrimitiveOperator::Multiply
        | PrimitiveOperator::Divide
        | PrimitiveOperator::Remainder
        | PrimitiveOperator::Exponent
        | PrimitiveOperator::ShiftLeft
        | PrimitiveOperator::ShiftRight
        | PrimitiveOperator::BitwiseOr
        | PrimitiveOperator::BitwiseXor
        | PrimitiveOperator::BitwiseAnd => {
            if left.types.within(Types::BIGINT) && right.types.within(Types::BIGINT) {
                Abstract::of_type(Types::BIGINT)
            } else {
                Abstract::of_type(Types::NUMBER)
            }
        }
        PrimitiveOperator::StrictEqual => Abstract::boolean(strict_equal(left, right, same)),
        PrimitiveOperator::StrictNotEqual => {
            Abstract::boolean(strict_equal(left, right, same).map(|v| !v))
        }
        PrimitiveOperator::Equal => Abstract::boolean(loose_equal(left, right, same)),
        PrimitiveOperator::NotEqual => {
            Abstract::boolean(loose_equal(left, right, same).map(|v| !v))
        }
        _ => Abstract::of_type(Types::BOOLEAN),
    }
}

/// `left === right`, when guaranteed.
fn strict_equal(left: &Abstract, right: &Abstract, same: bool) -> Option<bool> {
    if left.types.disjoint(right.types) {
        return Some(false);
    }
    // Of the types both may have, every value is listed on both sides, and
    // no listed value on one side is `===` one on the other: a join of an
    // object and `"object"` is never `null` or `"boolean"`.
    let common = Types(left.types.0 & right.types.0);
    if common.within(Types(left.listed.0 & right.listed.0)) {
        let of_common = |side: &Abstract| -> Vec<SymbolicValue> {
            side.some
                .iter()
                .filter(|value| !Types::of(value).disjoint(common))
                .cloned()
                .collect()
        };
        let (ours, theirs) = (of_common(left), of_common(right));
        let differ = ours.iter().all(|a| {
            theirs.iter().all(|b| {
                matches!(
                    fold_binary(PrimitiveOperator::StrictEqual, Some(a), Some(b)),
                    Some(SymbolicValue::Boolean(false))
                )
            })
        });
        if differ {
            return Some(false);
        }
    }
    // A value is itself, unless it may be NaN.
    (same && left.types.disjoint(Types::NUMBER)).then_some(true)
}

/// `left == right`, when guaranteed. Only nullish comparisons and disjoint
/// non-coercing cases are decided.
fn loose_equal(left: &Abstract, right: &Abstract, same: bool) -> Option<bool> {
    let decided = |nullish: &Abstract, other: &Abstract| {
        if !nullish.types.within(Types::NULLISH) {
            return None;
        }
        if other.types.within(Types::NULLISH) {
            Some(true)
        } else if other.types.disjoint(Types::NULLISH) {
            // `null`/`undefined` are loosely equal only to each other (and
            // `document.all`, which `OBJECT` excludes).
            Some(false)
        } else {
            None
        }
    };
    decided(left, right)
        .or_else(|| decided(right, left))
        .or_else(|| (same && left.types.disjoint(Types::NUMBER)).then_some(true))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [`decide_typed`] with no type facts or concrete values.
    fn decide(
        arena: &TraceArena,
        aliases: &NodeMap<TraceNodeId>,
        joins: &BTreeMap<TraceNodeId, Vec<TraceNodeId>>,
        returns: &BTreeMap<TraceNodeId, Vec<TraceNodeId>>,
        test: TraceNodeId,
    ) -> Option<bool> {
        decide_typed(arena, aliases, joins, returns, &BTreeMap::new(), None, test).0
    }
    use crate::operations::table_scan::SpanRecord;
    use crate::tracing::symbolic::{TraceNode, TraceProvenance};

    /// An arena under construction; `push` returns the node id.
    struct Arena(TraceArena);

    impl Arena {
        fn new() -> Self {
            Self(TraceArena::default())
        }
        fn push(&mut self, kind: TraceNodeKind) -> TraceNodeId {
            let id = self.0.nodes.len() as TraceNodeId;
            self.0.nodes.push(TraceNode {
                id,
                kind,
                provenance: TraceProvenance {
                    handler_index: 0,
                    table_key: 0,
                    semantic_identity: "".into(),
                    step_order: 0,
                    trace_step: 0,
                    span: SpanRecord { start: 0, end: 0 },
                },
            });
            id
        }
        fn binary(&mut self, operator: PrimitiveOperator, left: u32, right: u32) -> u32 {
            self.push(TraceNodeKind::Binary {
                operator,
                left,
                right,
            })
        }
        fn decide(&self, node: TraceNodeId) -> Option<bool> {
            decide_typed(
                &self.0,
                &NodeMap::new(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                &BTreeMap::new(),
                None,
                node,
            )
            .0
        }
    }

    fn unknown(arena: &mut Arena, name: &str) -> TraceNodeId {
        arena.push(TraceNodeKind::External { name: name.into() })
    }

    #[test]
    fn objects_and_functions_are_truthy_and_nullish_values_falsy() {
        let mut arena = Arena::new();
        let array = arena.push(TraceNodeKind::ArrayLiteral { elements: vec![] });
        let function = arena.push(TraceNodeKind::VmFunction {
            descriptor: 0,
            byte_offset: 0,
            dispatch_state: 0,
            memory: None,
        });
        let void = arena.push(TraceNodeKind::Unary {
            operator: PrimitiveOperator::Void,
            value: array,
        });
        assert_eq!(arena.decide(array), Some(true));
        assert_eq!(arena.decide(function), Some(true));
        assert_eq!(arena.decide(void), Some(false));
    }

    #[test]
    fn a_value_may_be_nan_so_it_is_not_known_to_equal_itself() {
        let mut arena = Arena::new();
        let x = unknown(&mut arena, "x");
        let same = arena.binary(PrimitiveOperator::StrictEqual, x, x);
        assert_eq!(arena.decide(same), None);

        // A string is itself.
        let text = arena.push(TraceNodeKind::Unary {
            operator: PrimitiveOperator::Typeof,
            value: x,
        });
        let empty = arena.push(TraceNodeKind::String(String::new()));
        let concat = arena.binary(PrimitiveOperator::JsAdd, x, empty);
        let same = arena.binary(PrimitiveOperator::StrictEqual, concat, concat);
        assert_eq!(arena.decide(same), Some(true));
        let same = arena.binary(PrimitiveOperator::Equal, text, text);
        assert_eq!(arena.decide(same), Some(true));
    }

    #[test]
    fn loose_equality_between_primitives_coerces_and_stays_undecided() {
        let mut arena = Arena::new();
        let x = unknown(&mut arena, "x");
        let number = arena.binary(PrimitiveOperator::Subtract, x, x);
        let text = arena.push(TraceNodeKind::Unary {
            operator: PrimitiveOperator::Typeof,
            value: x,
        });
        // `1 == "1"` is true: a number and a string may be loosely equal.
        let loose = arena.binary(PrimitiveOperator::Equal, number, text);
        assert_eq!(arena.decide(loose), None);
        // Strictly, they never are.
        let strict = arena.binary(PrimitiveOperator::StrictEqual, number, text);
        assert_eq!(arena.decide(strict), Some(false));
    }

    #[test]
    fn a_join_variable_holds_what_one_of_its_arms_assigned() {
        let mut arena = Arena::new();
        let typeof_concat = |arena: &mut Arena, name: &str| {
            let x = unknown(arena, name);
            let text = arena.push(TraceNodeKind::String("://".into()));
            let sum = arena.binary(PrimitiveOperator::JsAdd, x, text);
            arena.push(TraceNodeKind::Unary {
                operator: PrimitiveOperator::Typeof,
                value: sum,
            })
        };
        let left = typeof_concat(&mut arena, "a");
        let right = typeof_concat(&mut arena, "b");
        let merged = arena.push(TraceNodeKind::Identifier {
            name: "trace_value_9".into(),
        });
        let string = arena.push(TraceNodeKind::String("string".into()));
        let test = arena.binary(PrimitiveOperator::StrictEqual, merged, string);
        let joins = BTreeMap::from([(merged, vec![left, right])]);
        assert_eq!(
            decide(&arena.0, &NodeMap::new(), &joins, &BTreeMap::new(), test),
            Some(true)
        );
        // Arms of different types: only the union of the types is known.
        let number = arena.push(TraceNodeKind::Number("1".into()));
        let mixed = arena.push(TraceNodeKind::Identifier {
            name: "trace_value_10".into(),
        });
        let type_of = arena.push(TraceNodeKind::Unary {
            operator: PrimitiveOperator::Typeof,
            value: mixed,
        });
        let test = arena.binary(PrimitiveOperator::StrictEqual, type_of, string);
        let joins = BTreeMap::from([(mixed, vec![left, number])]);
        assert_eq!(
            decide(&arena.0, &NodeMap::new(), &joins, &BTreeMap::new(), test),
            None
        );
    }

    #[test]
    fn a_join_of_a_made_object_and_a_truthy_constant_is_truthy() {
        // `v = [] or true`: `!v` is false on either arm; `v = [] or 0` is
        // either.
        let mut arena = Arena::new();
        let array = arena.push(TraceNodeKind::ArrayLiteral { elements: vec![] });
        let truthy = arena.push(TraceNodeKind::Boolean(true));
        let zero = arena.push(TraceNodeKind::Number("0".into()));
        let merged = arena.push(TraceNodeKind::Identifier {
            name: "trace_value_11".into(),
        });
        let mixed = arena.push(TraceNodeKind::Identifier {
            name: "trace_value_12".into(),
        });
        let not = |arena: &mut Arena, value| {
            arena.push(TraceNodeKind::Unary {
                operator: PrimitiveOperator::LogicalNot,
                value,
            })
        };
        let test = not(&mut arena, merged);
        let open = not(&mut arena, mixed);
        let joins = BTreeMap::from([(merged, vec![array, truthy]), (mixed, vec![array, zero])]);
        assert_eq!(
            decide(&arena.0, &NodeMap::new(), &joins, &BTreeMap::new(), test),
            Some(false)
        );
        assert_eq!(
            decide(&arena.0, &NodeMap::new(), &joins, &BTreeMap::new(), open),
            None
        );
    }

    #[test]
    fn joins_whose_listed_constants_never_meet_are_never_strictly_equal() {
        // `a = [] or typeof []`, `b = null or "boolean"`: `a === b` is false
        // in every combination; `"object"` instead of `"boolean"` is not.
        let mut arena = Arena::new();
        let array = arena.push(TraceNodeKind::ArrayLiteral {
            elements: Vec::new(),
        });
        let type_of = arena.push(TraceNodeKind::Unary {
            operator: PrimitiveOperator::Typeof,
            value: array,
        });
        let nothing = arena.push(TraceNodeKind::Null);
        let boolean = arena.push(TraceNodeKind::String("boolean".into()));
        let object = arena.push(TraceNodeKind::String("object".into()));
        let a = arena.push(TraceNodeKind::Identifier {
            name: "trace_value_1".into(),
        });
        let b = arena.push(TraceNodeKind::Identifier {
            name: "trace_value_2".into(),
        });
        let c = arena.push(TraceNodeKind::Identifier {
            name: "trace_value_3".into(),
        });
        let joins = BTreeMap::from([
            (a, vec![array, type_of]),
            (b, vec![nothing, boolean]),
            (c, vec![nothing, object]),
        ]);
        let never = arena.binary(PrimitiveOperator::StrictEqual, a, b);
        assert_eq!(
            decide(&arena.0, &NodeMap::new(), &joins, &BTreeMap::new(), never),
            Some(false)
        );
        let maybe = arena.binary(PrimitiveOperator::StrictEqual, a, c);
        assert_eq!(
            decide(&arena.0, &NodeMap::new(), &joins, &BTreeMap::new(), maybe),
            None
        );
        // An unknown arm lists nothing.
        let unknown_value = unknown(&mut arena, "x");
        let d = arena.push(TraceNodeKind::Identifier {
            name: "trace_value_4".into(),
        });
        let joins = BTreeMap::from([(a, vec![array, type_of]), (d, vec![unknown_value, boolean])]);
        let open = arena.binary(PrimitiveOperator::StrictEqual, a, d);
        assert_eq!(
            decide(&arena.0, &NodeMap::new(), &joins, &BTreeMap::new(), open),
            None
        );
    }

    #[test]
    fn a_decoded_vm_string_is_a_string() {
        let mut arena = Arena::new();
        let decode = arena.push(TraceNodeKind::Global {
            name: "vmDecodeString".into(),
        });
        let this_value = arena.push(TraceNodeKind::Undefined);
        let call = arena.push(TraceNodeKind::Call {
            callee: decode,
            this_value,
            arguments: vec![],
        });
        let x = unknown(&mut arena, "x");
        let sum = arena.binary(PrimitiveOperator::JsAdd, x, call);
        let type_of = arena.push(TraceNodeKind::Unary {
            operator: PrimitiveOperator::Typeof,
            value: sum,
        });
        let number = arena.push(TraceNodeKind::String("number".into()));
        let test = arena.binary(PrimitiveOperator::StrictNotEqual, type_of, number);
        assert_eq!(arena.decide(test), Some(true));
        // Another function's result says nothing.
        let other = arena.push(TraceNodeKind::Global { name: "f".into() });
        let call = arena.push(TraceNodeKind::Call {
            callee: other,
            this_value,
            arguments: vec![],
        });
        let sum = arena.binary(PrimitiveOperator::JsAdd, x, call);
        let type_of = arena.push(TraceNodeKind::Unary {
            operator: PrimitiveOperator::Typeof,
            value: sum,
        });
        let test = arena.binary(PrimitiveOperator::StrictNotEqual, type_of, number);
        assert_eq!(arena.decide(test), None);
    }

    #[test]
    fn a_conversion_call_is_its_type() {
        let mut arena = Arena::new();
        let this_value = arena.push(TraceNodeKind::Undefined);
        let x = unknown(&mut arena, "x");
        for (name, expected) in [
            ("Number", "number"),
            ("String", "string"),
            ("Boolean", "boolean"),
        ] {
            let callee = arena.push(TraceNodeKind::Global { name: name.into() });
            let call = arena.push(TraceNodeKind::Call {
                callee,
                this_value,
                arguments: vec![x],
            });
            let type_of = arena.push(TraceNodeKind::Unary {
                operator: PrimitiveOperator::Typeof,
                value: call,
            });
            let undefined = arena.push(TraceNodeKind::String("undefined".into()));
            let test = arena.binary(PrimitiveOperator::StrictNotEqual, type_of, undefined);
            assert_eq!(arena.decide(test), Some(true), "{name}");
            let expected = arena.push(TraceNodeKind::String(expected.into()));
            let test = arena.binary(PrimitiveOperator::StrictEqual, type_of, expected);
            assert_eq!(arena.decide(test), Some(true), "{name}");
        }
    }

    #[test]
    fn typeof_is_known_for_known_types_only() {
        let mut arena = Arena::new();
        let x = unknown(&mut arena, "x");
        let object = arena.push(TraceNodeKind::String("object".into()));
        let array = arena.push(TraceNodeKind::ArrayLiteral { elements: vec![] });
        let of_array = arena.push(TraceNodeKind::Unary {
            operator: PrimitiveOperator::Typeof,
            value: array,
        });
        let is_object = arena.binary(PrimitiveOperator::StrictEqual, of_array, object);
        assert_eq!(arena.decide(is_object), Some(true));

        let of_x = arena.push(TraceNodeKind::Unary {
            operator: PrimitiveOperator::Typeof,
            value: x,
        });
        let unknown_type = arena.binary(PrimitiveOperator::StrictEqual, of_x, object);
        assert_eq!(arena.decide(unknown_type), None);
    }

    #[test]
    fn a_join_of_constants_is_compared_with_each() {
        // (c ? 95 : 110) >= 117 is false; >= 100 is either.
        let mut arena = Arena::new();
        let low = arena.push(TraceNodeKind::Number("95".into()));
        let high = arena.push(TraceNodeKind::Number("110".into()));
        let merged = arena.push(TraceNodeKind::Identifier {
            name: "trace_value_3".into(),
        });
        let joins = BTreeMap::from([(merged, vec![low, high])]);
        let bound = arena.push(TraceNodeKind::Number("117".into()));
        let above = arena.binary(PrimitiveOperator::GreaterThanOrEqual, merged, bound);
        assert_eq!(
            decide(&arena.0, &NodeMap::new(), &joins, &BTreeMap::new(), above),
            Some(false)
        );
        let middle = arena.push(TraceNodeKind::Number("100".into()));
        let either = arena.binary(PrimitiveOperator::GreaterThanOrEqual, merged, middle);
        assert_eq!(
            decide(&arena.0, &NodeMap::new(), &joins, &BTreeMap::new(), either),
            None
        );
    }

    #[test]
    fn typeof_of_a_few_types_is_one_of_their_names() {
        // typeof (x * y): "number", never "string".
        let mut arena = Arena::new();
        let (x, y) = (unknown(&mut arena, "x"), unknown(&mut arena, "y"));
        let difference = arena.binary(PrimitiveOperator::Multiply, x, y);
        let of_difference = arena.push(TraceNodeKind::Unary {
            operator: PrimitiveOperator::Typeof,
            value: difference,
        });
        let string = arena.push(TraceNodeKind::String("string".into()));
        let is_string = arena.binary(PrimitiveOperator::StrictEqual, of_difference, string);
        assert_eq!(arena.decide(is_string), Some(false));
        let loosely = arena.binary(PrimitiveOperator::Equal, of_difference, string);
        assert_eq!(arena.decide(loosely), Some(false));
        let number = arena.push(TraceNodeKind::String("number".into()));
        let is_number = arena.binary(PrimitiveOperator::StrictEqual, of_difference, number);
        assert_eq!(arena.decide(is_number), Some(true));
    }

    #[test]
    fn a_string_table_entry_is_a_string() {
        let mut arena = Arena::new();
        let table = arena.push(TraceNodeKind::Global {
            name: VM_STRING_TABLE.into(),
        });
        let index = unknown(&mut arena, "i");
        let entry = arena.push(TraceNodeKind::PropertyRead {
            object: table,
            key: index,
        });
        let of_entry = arena.push(TraceNodeKind::Unary {
            operator: PrimitiveOperator::Typeof,
            value: entry,
        });
        let string = arena.push(TraceNodeKind::String("string".into()));
        let is_string = arena.binary(PrimitiveOperator::StrictEqual, of_entry, string);
        assert_eq!(arena.decide(is_string), Some(true));
    }

    #[test]
    fn arithmetic_with_a_number_is_a_number() {
        // `typeof (0 | x) !== "number"`: a BigInt `x` throws, so it never is.
        let mut arena = Arena::new();
        let x = unknown(&mut arena, "x");
        let zero = arena.push(TraceNodeKind::Number("0".into()));
        let or = arena.binary(PrimitiveOperator::BitwiseOr, zero, x);
        let of_or = arena.push(TraceNodeKind::Unary {
            operator: PrimitiveOperator::Typeof,
            value: or,
        });
        let number = arena.push(TraceNodeKind::String("number".into()));
        let not_number = arena.binary(PrimitiveOperator::StrictNotEqual, of_or, number);
        assert_eq!(arena.decide(not_number), Some(false));
        // Bit operations on values it does not know are numbers too.
        let w = unknown(&mut arena, "w");
        let shifted = arena.binary(PrimitiveOperator::ShiftLeft, x, w);
        let combined = arena.binary(PrimitiveOperator::BitwiseOr, w, shifted);
        let of_combined = arena.push(TraceNodeKind::Unary {
            operator: PrimitiveOperator::Typeof,
            value: combined,
        });
        let bits_number = arena.binary(PrimitiveOperator::StrictEqual, of_combined, number);
        assert_eq!(arena.decide(bits_number), Some(true));
        // So is arithmetic on two values it does not know.
        let y = unknown(&mut arena, "y");
        let product = arena.binary(PrimitiveOperator::Multiply, x, y);
        let of_product = arena.push(TraceNodeKind::Unary {
            operator: PrimitiveOperator::Typeof,
            value: product,
        });
        let is_number = arena.binary(PrimitiveOperator::StrictEqual, of_product, number);
        assert_eq!(arena.decide(is_number), Some(true));
    }

    #[test]
    fn a_difference_of_unknown_values_is_a_number() {
        // The opaque predicate `typeof (t[e] - t.navigationStart) !== "number"`.
        let mut arena = Arena::new();
        let (timing, start) = (unknown(&mut arena, "timing"), unknown(&mut arena, "start"));
        let difference = arena.binary(PrimitiveOperator::Subtract, timing, start);
        let of_difference = arena.push(TraceNodeKind::Unary {
            operator: PrimitiveOperator::Typeof,
            value: difference,
        });
        let number = arena.push(TraceNodeKind::String("number".into()));
        let not_number = arena.binary(PrimitiveOperator::StrictNotEqual, of_difference, number);
        assert_eq!(arena.decide(not_number), Some(false));
        // With one side a number, it is a number even with no assumption.
        let one = arena.push(TraceNodeKind::Number("1".into()));
        let less_one = arena.binary(PrimitiveOperator::Subtract, timing, one);
        let of_less_one = arena.push(TraceNodeKind::Unary {
            operator: PrimitiveOperator::Typeof,
            value: less_one,
        });
        let is_number = arena.binary(PrimitiveOperator::StrictEqual, of_less_one, number);
        assert_eq!(arena.decide(is_number), Some(true));
    }

    #[test]
    fn a_property_read_is_unknown_except_a_string_length() {
        let mut arena = Arena::new();
        let x = unknown(&mut arena, "x");
        let key = arena.push(TraceNodeKind::String("length".into()));
        let read = arena.push(TraceNodeKind::PropertyRead { object: x, key });
        let nothing = arena.push(TraceNodeKind::Null);
        let is_null = arena.binary(PrimitiveOperator::Equal, read, nothing);
        assert_eq!(arena.decide(is_null), None);

        let empty = arena.push(TraceNodeKind::String(String::new()));
        let text = arena.binary(PrimitiveOperator::JsAdd, x, empty);
        let length = arena.push(TraceNodeKind::PropertyRead { object: text, key });
        let is_null = arena.binary(PrimitiveOperator::Equal, length, nothing);
        assert_eq!(arena.decide(is_null), Some(false));
    }

    #[test]
    fn let_bindings_are_followed_to_their_values() {
        let mut arena = Arena::new();
        let array = arena.push(TraceNodeKind::ArrayLiteral { elements: vec![] });
        let binding = arena.push(TraceNodeKind::Identifier {
            name: "trace_value_0".into(),
        });
        let aliases = NodeMap::from_iter([(binding, array)]);
        assert_eq!(
            decide(
                &arena.0,
                &aliases,
                &BTreeMap::new(),
                &BTreeMap::new(),
                binding
            ),
            Some(true)
        );
        assert_eq!(arena.decide(binding), None);
    }

    /// An array rebound once for each element written (fifty readback
    /// samples, southwest 02bd wave:4:3) is a chain of bindings longer than
    /// the expression depth: `typeof` of its last binding is still known.
    #[test]
    fn a_long_chain_of_bindings_is_followed_to_its_value() {
        let mut arena = Arena::new();
        let mut node = arena.push(TraceNodeKind::ArrayLiteral { elements: vec![] });
        let mut aliases = NodeMap::new();
        for serial in 0..(4 * MAX_DEPTH) {
            let binding = arena.push(TraceNodeKind::Identifier {
                name: format!("trace_value_{serial}"),
            });
            aliases.insert(binding, node);
            node = binding;
        }
        let kind = arena.push(TraceNodeKind::Unary {
            operator: PrimitiveOperator::Typeof,
            value: node,
        });
        let object = arena.push(TraceNodeKind::String("object".into()));
        let test = arena.binary(PrimitiveOperator::StrictNotEqual, kind, object);
        let decided = decide(&arena.0, &aliases, &BTreeMap::new(), &BTreeMap::new(), test);
        assert_eq!(decided, Some(false));
    }
}
