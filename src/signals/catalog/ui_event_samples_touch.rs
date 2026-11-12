//! `UiEventSamplesTouch`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{top_level_ops, BindRefusal, Claim, FieldBinding, FinderCtx, LeafKind, LeafOp, SignalLayout, SlotView};
use value_facts::slot_facts;
use crate::signals::analysis::{value_facts, leaf_checks::*, described_fields::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};

const TOUCH_EVENTS: &[&str] = &["touchstart", "touchmove", "touchend"];

/// `UiEventSamplesTouch`. Card: [`CARD`].
pub(crate) fn recognize_ui_event_samples_touch(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    // RAW4 NUMBER REPEAT{... REPEAT{points} ...}.
    if top_level_ops(view) != [LeafOp::Number] || max_repeat_depth(view) != 2 {
        return None;
    }
    let facts = slot_facts(view, ctx);
    let touch_roles = ctx.slot_roles(view).iter().any(|role| role.starts_with("interaction_touch"));
    let touched = facts.has_event(TOUCH_EVENTS) || touch_roles;
    // The last-chance callback is shared with the mouse rings in some
    // builds; the point list itself reads touch points.
    let points = facts.literals.iter().any(|literal| literal.fields.values().any(|desc| desc.has_member("force")));
    (touched || points).then(|| Claim::new(["touch listeners fill the record ring", "nested point list"]))
}

pub(crate) fn bind_ui_event_samples_touch(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let mut binder = Binder::new(view, layout, ctx)?;
    binder.top(|field| match field.shape {
        Shape::Array => Some("records"),
        Shape::Scalar(_) => Some("recordCount"),
        _ => None,
    })?;
    binder.records(|records, facts| {
        let outer = records.name_by_labels(facts, |field, meaning| match (field.shape, field.kind()) {
            (Shape::Array, _) => Some("points"),
            (_, Some(LeafKind::Byte)) => Some("flags"),
            (_, Some(LeafKind::Number)) if is_clock_value(meaning) => Some("time"),
            // The count of the record's point list (`rec.c.length`, `c` a
            // list in the record literal).
            (_, Some(LeafKind::Number)) if meaning.has_op("array") && field.merged().has_member("length") => Some("pointCount"),
            _ => None,
        });
        outer?;
        records.nested(facts, |field, meaning| {
            if meaning.has_member("force") {
                Some("force")
            } else if meaning.has_member("clientX") || (meaning.has_member("left") && meaning.has_member("pageX")) {
                Some("targetRelX")
            } else if meaning.has_member("clientY") || (meaning.has_member("top") && meaning.has_member("pageY")) {
                Some("targetRelY")
            } else if meaning.has_member("pageX") {
                Some("pageX")
            } else if meaning.has_member("pageY") {
                Some("pageY")
            } else {
                None
            }
        })
    })?;
    binder.finish()
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `UiEventSamplesTouch`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "UiEventSamplesTouch",
    summary: "Records touch activity on the document: one record per touchstart, touchmove or touchend event, keeping the newest 20. Each record has a time, the list of touch points (finger positions on the page and relative to the touched element, plus pressure), the event type and two trust flags. All records are sent in one message when the report is built; each record's time is sent as the gap since the previous record, and within a record each point after the first is sent as the difference from the point before it.",
    why: "Touch paths, timing and pressure are strong human-vs-bot behaviour signals on mobile; isTrusted and instanceof UIEvent also expose touch events synthesised by scripts or emulators.",
    sample_payload: Some(r#"{
  "records": [

  ]
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number("recordCount", "Number of touch records that follow (up to 20)."),
        CardField::group("records", &[
            CardField::number(
                "time",
                "Milliseconds since the script started (usually fractional); for later records, time since the previous record.",
            ),
            CardField::number(
                "pointCount",
                "Number of touch points that follow: event.touches, or event.changedTouches when no fingers remain (so touchend lists the lifted fingers).",
            ),
            CardField::group("points", &[
                CardField::number(
                    "force",
                    "touch.force (pressure, usually 0..1, fractional); after the first point, difference from the previous point. Or a null marker if force is missing.",
                ).nullable(),
                CardField::number(
                    "targetRelY",
                    "Touch Y relative to the touched element's top edge, rounded (0 if the target is not an element); after the first point, difference from the previous point.",
                ),
                CardField::number(
                    "pageY",
                    "Touch Y on the page in CSS pixels, rounded; after the first point, difference from the previous point.",
                ),
                CardField::number(
                    "pageX",
                    "Touch X on the page in CSS pixels, rounded; after the first point, difference from the previous point.",
                ),
                CardField::number(
                    "targetRelX",
                    "Touch X relative to the touched element's left edge, rounded (0 if the target is not an element); after the first point, difference from the previous point.",
                ),
                CardField::number(
                    "pairedFields",
                    "Holder, in some builds, of two neighbouring record fields written under one condition: when the first field is missing it carries that field's null marker and then the second field, otherwise both values. The two fields keep their own names inside it.",
                ),
            ])
            .repeat("once per touch point in the event"),
            CardField::byte(
                "flags",
                "Flag bits: is touchmove; is touchstart (neither set means touchend); event.isTrusted is true; event is an instanceof UIEvent.",
            )
            .flags(&["isTouchmove", "isTouchstart", "isTrusted", "instanceOfUIEvent"]),
        ])
        .repeat("once per touch event, oldest first, up to 20"),
    ]),
};
