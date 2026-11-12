//! `LifecyclePhaseTimings`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use std::collections::BTreeSet;
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use value_facts::{slot_facts, Desc};
use crate::signals::analysis::{value_facts, leaf_checks::*, described_fields::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// The phase whose callback assigns this lifecycle timestamp.
fn phase_field(desc: &Desc) -> Option<&'static str> {
    let phases: Vec<&str> = desc
        .phases
        .iter()
        .copied()
        .filter(|phase| matches!(*phase, "init" | "collection" | "last_chance"))
        .collect();
    match phases.as_slice() {
        [] if desc.reads_date() && !desc.has_op("-") => Some("startEpochMs"),
        ["init"] if desc.has_op("-") => Some("initElapsedMs"),
        ["collection"] if desc.has_op("-") => Some("collectionElapsedMs"),
        ["last_chance"] if desc.has_op("-") => Some("lastChanceElapsedMs"),
        _ => None,
    }
}

/// `LifecyclePhaseTimings`. Card: [`CARD`].
pub(crate) fn recognize_lifecycle_phase_timings(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    if !all_numbers(view, 4) || view.emissions().len() < 2 {
        return None;
    }
    let facts = slot_facts(view, ctx);
    if facts.emissions.iter().flatten().count() == 0 {
        return None;
    }
    // Every matched emission: one setup Date epoch and three phase deltas.
    let named = facts.emissions.iter().flatten().all(|descs| {
        let names: BTreeSet<&str> = descs.iter().filter_map(phase_field).collect();
        names.len() == 4
    });
    named.then(|| Claim::new(["setup Date epoch", "Date deltas assigned in init/collection/last_chance"]))
}

pub(crate) fn bind_lifecycle_phase_timings(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let mut binder = Binder::new(view, layout, ctx)?;
    binder.top_each(phase_field)?;
    binder.finish()
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `LifecyclePhaseTimings`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "LifecyclePhaseTimings",
    summary: "Records the time (from Date) when the script is set up, then how many milliseconds later each of its three lifecycle steps ran: startup (init), data collection, and the final step before sending (last chance). It sends one report at each of the three steps, so each report shows which steps have run so far and how far apart they were.",
    why: "How long the script took to reach each step, and the absolute clock time, show unusually fast or slow execution, cut-short page visits and clock tampering, which are typical of headless or scripted sessions (likely).",
    sample_payload: Some(r#"{
  "collectionElapsedMs": 192,
  "initElapsedMs": 0,
  "lastChanceElapsedMs": 49815,
  "startEpochMs": 1794273726998
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "collectionElapsedMs",
            "Milliseconds from setup to the collection step, or -3600001 if collection has not run yet.",
        ),
        CardField::number(
            "startEpochMs",
            "Date.now()-style timestamp of setup, in milliseconds since 1970 (about 1.7e12).",
        ),
        CardField::number(
            "lastChanceElapsedMs",
            "Milliseconds from setup to the final step before sending, or -3600001 if it has not run yet.",
        ),
        CardField::number(
            "initElapsedMs",
            "Milliseconds from setup to the startup (init) step, or -3600001 if it has not run yet.",
        ),
    ]),
};
