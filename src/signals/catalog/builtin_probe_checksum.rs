//! `BuiltinProbeChecksum`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafOp, LeafView, SignalLayout, SlotView};
use buffer_writes::slot_info;
use crate::signals::analysis::{buffer_writes, leaf_checks::*, flag_bytes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `x % m` over a first-char-code sum. The `.apply()` of the probed
/// built-ins is on the leaf's provenance, or (when the contract pass stops at
/// the shared accumulator) on the slot-local slice of the written value.
fn is_probe_checksum(leaf: &LeafView, write: Option<&buffer_writes::WriteInfo>) -> bool {
    has_transform(leaf, "%")
        && (has_transform(leaf, ".charCodeAt()") || has_transform(leaf, ".charCodeAt"))
        && (has_transform(leaf, ".apply()")
            || write.is_some_and(|write| write.facts.call("apply") && calls_method(&write.facts, "charCodeAt")))
}

/// `name` is called on the slice: `x.name(…)`, or the method read off and
/// invoked through `Function.prototype.call`/`apply`
/// (`x.name.call(x, 0)`).
fn calls_method(facts: &buffer_writes::Facts, name: &str) -> bool {
    facts.call(name) || (facts.member(name) && (facts.call("call") || facts.call("apply")))
}

fn is_fraction(leaf: &LeafView) -> bool {
    has_transform(leaf, "select") && has_transform(leaf, "<")
}

/// Per view emission, which leaves are the probe checksum (`None`: the
/// emission's AST candidates disagree).
fn probe_checksum_leaves(view: &SlotView, ctx: &FinderCtx) -> Vec<Option<Vec<bool>>> {
    let needs_slice = view
        .emissions()
        .iter()
        .any(|emission| !emission.leaves.iter().any(|leaf| is_probe_checksum(leaf, None)));
    let info = needs_slice.then(|| slot_info(view, ctx));
    view.emissions()
        .iter()
        .enumerate()
        .map(|(index, emission)| {
            let direct: Vec<bool> = emission.leaves.iter().map(|leaf| is_probe_checksum(leaf, None)).collect();
            if direct.iter().any(|flag| *flag) {
                return Some(direct);
            }
            let candidates = info.as_ref()?.emissions.get(index)?;
            let mut out: Option<Vec<bool>> = None;
            for candidate in candidates {
                let flags: Vec<bool> = emission
                    .leaves
                    .iter()
                    .enumerate()
                    .map(|(at, leaf)| is_probe_checksum(leaf, candidate.writes.get(at)))
                    .collect();
                match &out {
                    None => out = Some(flags),
                    Some(previous) if *previous == flags => {}
                    Some(_) => return None,
                }
            }
            out
        })
        .collect()
}

/// `BuiltinProbeChecksum`. Card: [`CARD`].
pub(crate) fn recognize_builtin_probe_checksum(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    if !plain_ops(view, &[LeafOp::Number, LeafOp::Number]) {
        return None;
    }
    // Cheap pre-filter before any slice walk.
    let shaped = view.emissions().iter().all(|emission| {
        emission.leaves.iter().any(|leaf| has_transform(leaf, "%") && (has_transform(leaf, ".charCodeAt()") || has_transform(leaf, ".charCodeAt")))
    });
    if !shaped {
        return None;
    }
    let flags = probe_checksum_leaves(view, ctx);
    let ok = view.emissions().iter().zip(&flags).all(|(emission, flags)| {
        let Some(flags) = flags else {
            return false;
        };
        let checksum = flags.iter().filter(|flag| **flag).count();
        let seed = emission
            .leaves
            .iter()
            .zip(flags)
            .filter(|(leaf, flag)| !**flag && is_fraction(leaf))
            .count();
        checksum == 1 && seed == 1
    });
    ok.then(|| claim(&["first-char-code sum of applied built-in probes mod 65353", "generator seed fraction"]))
        .flatten()
}

pub(crate) fn bind_builtin_probe_checksum(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let hints: Vec<Option<Vec<Option<Hint>>>> = probe_checksum_leaves(view, ctx)
        .into_iter()
        .map(|flags| {
            flags.map(|flags| {
                flags
                    .into_iter()
                    .map(|checksum| Some(Hint::Name(if checksum { "checksum" } else { "probeSeed" }.to_string())))
                    .collect()
            })
        })
        .collect();
    let fields = message_hints(layout, &hints)?;
    bind_named(layout, &fields)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `BuiltinProbeChecksum`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "BuiltinProbeChecksum",
    summary: "Keeps a table of 58 built-in functions and constructors (isNaN, parseInt, encodeURI, JSON.parse, typed arrays, Map, Blob, DOMParser, Image, XMLHttpRequest and so on). Using a seeded random generator, it calls a random selection of them twice, 8 to 14 calls at init and again at collection. For each call that works it adds the character code of the first character of the result (as a string) to a running sum. It sends the generator's seed and the checksum.",
    why: "With the seed, the server can likely replay which built-ins were picked and compare the checksum with what the claimed browser should produce. Missing, throwing or non-native (patched) built-ins change the sum, which exposes spoofed or emulated environments.",
    sample_payload: Some(r#"{
  "checksum": 1846,
  "probeSeed": 0.9550309971885809
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "probeSeed",
            "Seed of the random generator that chose the probes; a fraction from 0 up to (not including) 1.",
        ),
        CardField::number(
            "checksum",
            "2 plus the sum of first-character codes of all successful probe results, modulo 65353; range 2 to 65354.",
        ),
    ]),
};
