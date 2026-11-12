//! `IntervalClockDriftStats`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use std::collections::BTreeSet;
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafOp, SignalLayout, SlotView};
use value_facts::{slot_facts, Guard};
use crate::signals::analysis::{value_facts, described_fields::*};
use crate::signals::catalog::card::{Card, CardBody, CardField, CardMessage};


/// `IntervalClockDriftStats`. Card: [`CARD`].
pub(crate) fn recognize_interval_clock_drift_stats(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    // Four numbers per message (a top-level choice may send the placeholder
    // and the statistics from one emission).
    let numbers = view.emissions().iter().all(|emission| {
        let count = emission.leaves.len();
        emission.leaves.iter().all(|leaf| leaf.op == LeafOp::Number && leaf.repeat_depth == 0)
            && {
                // Four per message: top-level leaves plus one arm of a
                // two-arm choice (`placeholder | statistics`, with the
                // shared fields after the choice).
                let shared = emission.leaves.iter().filter(|leaf| leaf.choice_depth == 0).count();
                let armed = emission.leaves.iter().filter(|leaf| leaf.choice_depth == 1).count();
                (armed == 0 && count == 4)
                    || (shared + armed == count && armed % 2 == 0 && armed > 0 && shared + armed / 2 == 4)
            }
    });
    if view.emissions().is_empty() || !numbers {
        return None;
    }
    let facts = slot_facts(view, ctx);
    let guards: BTreeSet<Guard> = facts.descs().flat_map(|desc| desc.guards.iter().copied()).collect();
    (facts.timers.contains("setInterval") && guards.contains(&Guard::Max) && guards.contains(&Guard::Min))
        .then(|| Claim::new(["setInterval tick state", "running min/max of tick deltas"]))
}

pub(crate) fn bind_interval_clock_drift_stats(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let mut binder = Binder::new(view, layout, ctx)?;
    binder.top(|field| {
        let desc = field.merged();
        if desc.guards.contains(&Guard::Max) {
            Some("maxTickDeltaMs")
        } else if desc.guards.contains(&Guard::Min) {
            Some("minTickDeltaMs")
        } else if desc.has_op("/") && desc.has_member("length") {
            Some("meanTickDeltaMs")
        } else if desc.counter && !desc.has_op("/") {
            Some("tickCount")
        } else {
            None
        }
    })?;
    binder.finish()
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `IntervalClockDriftStats`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "IntervalClockDriftStats",
    summary: "Starts a timer that should fire once per second (setInterval at 1000 ms) and measures, with Date, how many milliseconds really passed between ticks. It keeps the last 10 tick gaps, plus the smallest gap, the largest gap and the number of ticks. At startup it sends a placeholder record. Just before the payload is sent, it reports the statistics if new ticks have arrived since the last report.",
    why: "Real timer gaps show background-tab throttling, event-loop stalls, system clock jumps, and fake or sped-up timers in automation and emulation setups (likely).",
    sample_payload: Some(r#"{
  "maxTickDeltaMs": 1838,
  "meanTickDeltaMs": 909.7,
  "minTickDeltaMs": 185,
  "tickCount": 49
}"#),
    by_name: false,
    body: CardBody::Messages(&[
        CardMessage {
            when: "At startup, before any tick (placeholder)",
            fields: &[
                CardField::number("meanTickDeltaMs", "Always -1 (no data yet)."),
                CardField::number("minTickDeltaMs", "Always -1 (no data yet)."),
                CardField::number("maxTickDeltaMs", "Always -1 (no data yet)."),
                CardField::number("tickCount", "Always 0."),
            ],
        },
        CardMessage {
            when: "Before the payload is sent, if at least one tick happened and new ticks arrived since the last report",
            fields: &[
                CardField::number(
                    "meanTickDeltaMs",
                    "Average of the last (up to 10) tick-to-tick gaps, in milliseconds. About 1000 normally; may be fractional.",
                ),
                CardField::number(
                    "minTickDeltaMs",
                    "Smallest tick-to-tick gap seen, in whole milliseconds. Can be negative if the system clock jumped backwards.",
                ),
                CardField::number(
                    "maxTickDeltaMs",
                    "Largest tick-to-tick gap seen, in whole milliseconds. Large values mean throttling or stalls.",
                ),
                CardField::number(
                    "tickCount",
                    "Total number of timer ticks since the start (one per ~second).",
                ),
            ],
        },
    ]),
};
