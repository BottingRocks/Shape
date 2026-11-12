//! `LifecycleEntropySamples`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use std::collections::BTreeSet;
use crate::signals::analysis::{top_level_ops, BindRefusal, Claim, FieldBinding, FinderCtx, LeafOp, SignalLayout, SlotView};
use value_facts::slot_facts;
use crate::signals::analysis::{value_facts, leaf_checks::*, described_fields::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `LifecycleEntropySamples`. Card: [`CARD`].
pub(crate) fn recognize_lifecycle_entropy_samples(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    if top_level_ops(view) != [LeafOp::Number]
        // One count and one repeated word per message (a message may be
        // emitted from more than one site).
        || !view.emissions().iter().all(|emission| emission.leaves.len() == 2)
        || max_repeat_depth(view) != 1
        || view.leaves().any(|leaf| leaf.op != LeafOp::Number)
    {
        return None;
    }
    // The list gets one random word per lifecycle step: pushes from at
    // least two lifecycle callbacks, each value drawn from the RNG.
    let facts = slot_facts(view, ctx);
    let phases: BTreeSet<&str> = facts
        .appends
        .iter()
        .filter(|(phase, desc)| {
            matches!(*phase, "init" | "collection" | "last_chance")
                && (desc.has_call("getRandomValues") || desc.has_call("random") || desc.program_call)
        })
        .map(|(phase, _)| *phase)
        .collect();
    (phases.len() >= 2).then(|| Claim::new(["one entropy word pushed per lifecycle step", "RAW4 NUMBER REPEAT{NUMBER}"]))
}

pub(crate) fn bind_lifecycle_entropy_samples(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let mut binder = Binder::new(view, layout, ctx)?;
    binder.top(|field| match field.shape {
        Shape::Array => Some("samples"),
        Shape::Scalar(_) => Some("sampleCount"),
        _ => None,
    })?;
    binder.finish()
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `LifecycleEntropySamples`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "LifecycleEntropySamples",
    summary: "Each time one of the script's lifecycle steps runs (startup, data collection, and the final step before sending), it draws a random 32-bit number and adds it to a list. The random number mixes Math.random, performance.now, the current date and crypto.getRandomValues. At the final step it sends how many numbers it has, then all of them. The numbers carry no device data; what matters is mostly how many lifecycle steps actually ran. A background timer that re-arms itself every 2–4 s also draws numbers, but it does not add them to the list.",
    why: "Likely a liveness and ordering check: the count shows whether every lifecycle step ran normally, and the random values tie the report to one real page session, so replayed or partly executed scripts stand out.",
    sample_payload: None,
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "sampleCount",
            "How many random words follow. Normally 3 (startup, collection, final). Less if collection failed or had not run yet, 4 if collection ran twice after an error.",
        ),
        CardField::group("samples", &[
            CardField::number(
                "entropyWord",
                "A random unsigned 32-bit number (0 to 4294967295). The last one is always from the final step.",
            ),
        ])
        .repeat("once per lifecycle step that ran, in the order they ran"),
    ]),
};
