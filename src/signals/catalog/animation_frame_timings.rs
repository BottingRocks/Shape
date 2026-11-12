//! `AnimationFrameTimings`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{top_level_ops, BindRefusal, Claim, FieldBinding, FinderCtx, LeafOp, SignalLayout, SlotView};
use value_facts::slot_facts;
use crate::signals::analysis::{value_facts, leaf_checks::*, described_fields::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


// ===========================================================================
// Recognizers and binders
// ===========================================================================

/// `AnimationFrameTimings`. Card: [`CARD`].
pub(crate) fn recognize_animation_frame_timings(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    // RAW4 NUMBER NUMBER REPEAT{NUMBER}: quantum, count, frame times.
    if top_level_ops(view) != [LeafOp::Number, LeafOp::Number]
        || view.leaves().count() != 3
        || max_repeat_depth(view) != 1
        || view.leaves().any(|leaf| leaf.op != LeafOp::Number)
    {
        return None;
    }
    let facts = slot_facts(view, ctx);
    facts
        .timers
        .contains("requestAnimationFrame")
        .then(|| Claim::new(["requestAnimationFrame callback fills the sampled list", "RAW4 NUMBER NUMBER REPEAT{NUMBER}"]))
}

pub(crate) fn bind_animation_frame_timings(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let mut binder = Binder::new(view, layout, ctx)?;
    binder.top(|field| match field.shape {
        Shape::Array => Some("frameTime"),
        Shape::Scalar(_) if field.literal_only() => Some("timeQuantum"),
        Shape::Scalar(_) if field.merged().has_member("length") => Some("sampleCount"),
        _ => None,
    })?;
    binder.finish()
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `AnimationFrameTimings`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "AnimationFrameTimings",
    summary: "Runs a requestAnimationFrame loop during collection and records up to 20 frame timestamps, rounded to whole milliseconds. At the last-chance phase it sends the first timestamp followed by the interval between each pair of consecutive frames. If requestAnimationFrame or cancelAnimationFrame is missing, nothing is sampled.",
    why: "Real browsers paint at the display refresh rate (for example about 16.7 ms per frame at 60 Hz) with natural jitter. Headless, background-tab and virtualised environments likely show missing, throttled or unnaturally regular frames, so the timing helps tell real users from bots.",
    sample_payload: Some(r#"{
  "frameTime": [
    9,
    8,
    9,
    8,
    9,
    8,
    8,
    8,
    9,
    8,
    9,
    8,
    8,
    8,
    8,
    9,
    8,
    8,
    10,
    1056
  ],
  "timeQuantum": 1
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number("timeQuantum", "Rounding unit for the timestamps in milliseconds; always 1."),
        CardField::number("sampleCount", "Number of frame samples collected, 0 to 20."),
        CardField::number(
            "frameTime",
            "First entry: the first frame's timestamp in whole milliseconds since page time origin. Each later entry: milliseconds since the previous frame (about 16-17 at 60 Hz). With only one sample it is just that timestamp.",
        )
        .repeat("once per sample, up to 20"),
    ]),
};
