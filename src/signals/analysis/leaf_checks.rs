//! Predicates over a slot's emission leaves: leaf-op sequences (`one
//! number`, `only bytes`, `count then bit bytes`), value provenance
//! (transforms, host API paths, source classes) and the claim constructor.
//!
//! Helps recognizers that decide from the leaves the slot writes rather than
//! from parse walks. Used by: 74 of the 92 catalog signals; helper modules
//! [`census_probes`](super::census_probes),
//! [`field_solver`](super::field_solver),
//! [`native_checks`](super::native_checks).

use std::collections::BTreeSet;

use crate::signals::extract::contracts::SourceClass;

use super::{api_matches, Claim, LeafOp, LeafView, SlotView};

/// Some transform of `leaf`'s value provenance is `transform` (`.toString()`).
pub(crate) fn has_transform(leaf: &LeafView, transform: &str) -> bool {
    leaf.transforms.iter().any(|t| t == transform)
}

/// Some leaf of `leaves` has provenance transform `transform`.
pub(crate) fn any_transform(leaves: &[&LeafView], transform: &str) -> bool {
    leaves.iter().any(|leaf| has_transform(leaf, transform))
}

/// Every emission is `RAW4` followed by exactly `ops` (no repeat or choice).
pub(crate) fn plain_ops(view: &SlotView, ops: &[LeafOp]) -> bool {
    !view.emissions().is_empty()
        && view.emissions().iter().all(|emission| {
            emission.leaves.len() == ops.len()
                && emission
                    .leaves
                    .iter()
                    .zip(ops)
                    .all(|(leaf, op)| leaf.op == *op && leaf.repeat_depth == 0 && leaf.choice_depth == 0)
        })
}

/// A high-confidence claim with `evidence` lines.
pub(crate) fn claim(evidence: &[&str]) -> Option<Claim> {
    Some(Claim::new(evidence.iter().copied()))
}

/// Some API path of `leaf`'s provenance matches `suffix` ([`api_matches`]).
pub(crate) fn has_api(leaf: &LeafView, suffix: &str) -> bool {
    leaf.apis.iter().any(|api| api_matches(api, suffix))
}

/// Some leaf of `leaves` reads an API path matching `suffix`.
pub(crate) fn any_api(leaves: &[&LeafView], suffix: &str) -> bool {
    leaves.iter().any(|leaf| has_api(leaf, suffix))
}

/// `leaf`'s provenance includes source class `class`.
pub(crate) fn has_class(leaf: &LeafView, class: SourceClass) -> bool {
    leaf.classes.contains(&class)
}

/// A literal written by the producer with no host or loop source.
pub(crate) fn literal_only(leaf: &LeafView) -> bool {
    !leaf.classes.is_empty() && leaf.classes.iter().all(|class| *class == SourceClass::Literal)
}

/// Every leaf of every emission, in order.
pub(crate) fn leaves_of<'v>(view: &'v SlotView) -> Vec<&'v LeafView> {
    view.leaves().collect()
}

/// The leaves of the first emission (one emission's worth of fields).
pub(crate) fn first_emission_leaves<'v>(view: &'v SlotView) -> Vec<&'v LeafView> {
    view.emissions()
        .first()
        .map(|emission| emission.leaves.iter().collect())
        .unwrap_or_default()
}

/// Some written leaf's provenance applies transform `name` (`.oldValue`).
pub(crate) fn leaf_transform(view: &SlotView, name: &str) -> bool {
    view.leaves().any(|leaf| leaf.transforms.iter().any(|t| t == name))
}

/// Every emission writes only bytes (any count, choices allowed).
pub(crate) fn only_bytes(view: &SlotView) -> bool {
    !view.emissions().is_empty() && view.leaves().all(|leaf| leaf.op == LeafOp::Byte)
}

/// The leaf of a slot whose every emission writes exactly one plain number.
pub(crate) fn single_number(view: &SlotView) -> Option<&LeafView> {
    plain_ops(view, &[LeafOp::Number]).then(|| view.leaves().next()).flatten()
}

/// Every emission writes exactly one plain number.
pub(crate) fn one_number(view: &SlotView) -> bool {
    plain_ops(view, &[LeafOp::Number])
}

/// Every emission writes exactly one plain byte.
pub(crate) fn one_byte(view: &SlotView) -> bool {
    plain_ops(view, &[LeafOp::Byte])
}

/// Every emission writes exactly five plain numbers.
pub(crate) fn five_numbers(view: &SlotView) -> bool {
    plain_ops(view, &[LeafOp::Number; 5])
}

/// A count followed by bit-packed bytes: `NUMBER REPEAT{CHOICE{BYTE}}
/// CHOICE{BYTE}` (or fixed bytes), one number per emission.
pub(crate) fn count_then_bit_bytes(view: &SlotView) -> bool {
    !view.emissions().is_empty()
        && view.emissions().iter().all(|emission| {
            let first = emission.leaves.first();
            first.is_some_and(|leaf| leaf.op == LeafOp::Number && leaf.repeat_depth == 0 && leaf.choice_depth == 0)
                && emission.leaves.len() >= 2
                && emission.leaves[1..].iter().all(|leaf| leaf.op == LeafOp::Byte)
        })
}

/// Member names read by the written values (`.crossOriginIsolated` in the
/// provenance transforms) plus the census names.
pub(crate) fn value_members(view: &SlotView) -> BTreeSet<String> {
    view.leaves()
        .flat_map(|leaf| leaf.transforms.iter())
        .filter_map(|transform| transform.strip_prefix('.'))
        .filter(|name| !name.contains('(') && !name.is_empty())
        .map(str::to_string)
        .collect()
}

/// Every emission writes exactly `n` top-level numbers and nothing else.
pub(crate) fn all_numbers(view: &SlotView, n: usize) -> bool {
    !view.emissions().is_empty()
        && view.emissions().iter().all(|emission| {
            emission.leaves.len() == n
                && emission
                    .leaves
                    .iter()
                    .all(|leaf| leaf.op == LeafOp::Number && leaf.repeat_depth == 0 && leaf.choice_depth == 0)
        })
}

/// Every emission writes `strings` strings and `numbers` numbers (any
/// nesting).
pub(crate) fn per_emission_counts(view: &SlotView, strings: usize, numbers: std::ops::RangeInclusive<usize>) -> bool {
    !view.emissions().is_empty()
        && view.emissions().iter().all(|emission| {
            let count = |op: LeafOp| emission.leaves.iter().filter(|leaf| leaf.op == op).count();
            count(LeafOp::String) == strings && numbers.contains(&count(LeafOp::Number))
        })
}

/// The deepest repeat nesting of any leaf (0 without repeats).
pub(crate) fn max_repeat_depth(view: &SlotView) -> usize {
    view.leaves().map(|leaf| leaf.repeat_depth).max().unwrap_or(0)
}

/// The local rendering of a fixed date: `new Date(K).toString()`.
pub(crate) fn date_to_string(leaf: &LeafView) -> bool {
    leaf.op == LeafOp::String && has_transform(leaf, ".toString()") && has_transform(leaf, "new Date")
}

/// A string whose characters were shuffled (`Array.from(s)` ... `.join()`).
pub(crate) fn shuffled(leaf: &LeafView) -> bool {
    has_transform(leaf, "Array.from") || has_transform(leaf, ".join()")
}
