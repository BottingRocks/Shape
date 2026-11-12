//! `KeystrokeDynamics`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{top_level_ops, BindRefusal, Claim, FieldBinding, FinderCtx, LeafOp, SignalLayout, SlotView};
use value_facts::slot_facts;
use crate::signals::analysis::{value_facts, shape_text::*, leaf_checks::*, described_fields::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `KeystrokeDynamics`. Card: [`CARD`].
pub(crate) fn recognize_keystroke_dynamics(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    // Numbers only; one repeat of single counters.
    if view.leaves().any(|leaf| leaf.op != LeafOp::Number)
        || repeat_count(view) != 1
        || max_repeat_depth(view) != 1
        || view.leaves().filter(|leaf| leaf.repeat_depth == 1).count() != 1
        || top_level_ops(view).len() < 5
    {
        return None;
    }
    let facts = slot_facts(view, ctx);
    (facts.has_event(&["keydown"]) && facts.has_event(&["keyup"]) && facts.any_member("keyCode"))
        .then(|| Claim::new(["keydown/keyup timing state", "special-key counter list"]))
}

pub(crate) fn bind_keystroke_dynamics(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let mut binder = Binder::new(view, layout, ctx)?;
    binder.top(|field| {
        if field.shape == Shape::Array {
            return Some("specialKeyCounts");
        }
        let desc = field.merged();
        let clock = desc.has_call("now") || desc.news.contains("Date");
        if !clock && desc.has_member("length") && !desc.counter {
            Some("specialKeySlotCount")
        } else if !clock && desc.counter {
            Some("specialKeyTotal")
        } else if clock && desc.numbers.contains("60") {
            Some("keystrokesPerMinute")
        } else if clock && !desc.squared.is_empty() {
            Some("dwellVariance")
        } else if clock && desc.conditional {
            Some("meanShortGapMs")
        } else if clock {
            Some("meanDwellMs")
        } else {
            None
        }
    })?;
    binder.finish()
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `KeystrokeDynamics`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "KeystrokeDynamics",
    summary: "Listens to keydown and keyup on the page and keeps running typing statistics: typing speed, how long keys are held down and how much that varies, the gap between releasing one key and pressing the next, and how often certain special keys are used (Ctrl, C, Meta/Windows, Backspace, Escape, Alt, Tab, ContextMenu, F1–F12). It reports these just before the payload is sent, but only if new keys were released since the last report. The first report is always sent, even with no typing.",
    why: "Human typing has natural rhythm and variance, while scripts type at fixed or impossible speeds. Copy/paste and editing key use (Ctrl, C, Meta, Backspace) also hints at pasted credentials (likely).",
    sample_payload: Some(r#"{
  "dwellVariance": 1200.5,
  "keystrokesPerMinute": 0,
  "meanDwellMs": 24.5,
  "meanShortGapMs": 0,
  "specialKeyCounts": [
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0,
    0
  ],
  "specialKeyTotal": 0
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "keystrokesPerMinute",
            "Key releases per minute of typing, with pauses of 2 s or more left out. 0 when there have been 5 or fewer key releases.",
        ),
        CardField::number(
            "dwellVariance",
            "Sample variance of key hold time (keydown to keyup of the same key), in ms². 0 with fewer than 2 key releases.",
        ),
        CardField::number("specialKeySlotCount", "Number of special-key counters that follow; always 22."),
        CardField::group("specialKeyCounts", &[
            CardField::number("count", "How many times this key was released."),
        ])
        .repeat("once per special key, 22 in this order: Ctrl, C, Meta/Win left, Win right, Meta (Firefox), Backspace, Escape, Alt, Tab, ContextMenu, F1–F12"),
        CardField::number("specialKeyTotal", "Total key releases of any of the 22 special keys."),
        CardField::number(
            "meanShortGapMs",
            "Average time from one key release to the next key press, in ms, counting only gaps under 2000 ms. 0 if there are none.",
        ),
        CardField::number(
            "meanDwellMs",
            "Average key hold time (keydown to keyup), in ms. 0 if nothing was typed.",
        ),
    ]),
};
