//! `VisibilityEventLog`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafKind, LeafOp, SignalLayout, SlotView};
use value_facts::slot_facts;
use crate::signals::analysis::{value_facts, shape_text::*, leaf_checks::*, described_fields::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `VisibilityEventLog`. Card: [`CARD`].
pub(crate) fn recognize_visibility_event_log(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    // One record list per message (a top-level choice may hold two
    // messages), no strings.
    if repeat_count(view) == 0 || repeat_count(view) > 2 || max_repeat_depth(view) != 1 || view.leaf_count(LeafOp::String) != 0 {
        return None;
    }
    let facts = slot_facts(view, ctx);
    // The written record values read the visibility state directly, or (fresh
    // traces: the record ring is read back through `.toArray()`) the
    // `visibilitychange` handler filling the ring reads it.
    let handler_reads = facts
        .handler_members
        .get("visibilitychange")
        .is_some_and(|members| members.contains("visibilityState") || members.contains("hidden"));
    let visibility = leaf_transform(view, ".visibilityState") || leaf_transform(view, ".hidden") || handler_reads;
    if !visibility {
        return None;
    }
    (facts.has_event(&["visibilitychange"]) || ctx.has_role(view, "interaction_visibility_sample_recorder"))
        .then(|| Claim::new(["visibilitychange record ring", "document.visibilityState"]))
}

pub(crate) fn bind_visibility_event_log(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let mut binder = Binder::new(view, layout, ctx)?;
    binder.top(|field| match (field.shape, field.kind()) {
        (Shape::Array, _) => Some("records"),
        (_, Some(LeafKind::Number)) if field.merged().counter && !field.merged().has_member("length") => Some("visibilityChangeCount"),
        (_, Some(LeafKind::Number)) if field.merged().has_member("length") => Some("recordCount"),
        (_, Some(LeafKind::Byte)) if !field.is_tag() => Some("state"),
        // The literal tag of the unknown-visibility message.
        (Shape::Scalar(LeafKind::Byte), _) if field.is_tag() => Some("stateNullTag"),
        _ => None,
    })?;
    binder.records(|records, facts| {
        records.name_by_kind(|field| match field.kind() {
            Some(LeafKind::Number) => Some("timestamp"),
            Some(LeafKind::Byte) => Some("visible"),
            _ => None,
        })
    })?;
    binder.finish()
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `VisibilityEventLog`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "VisibilityEventLog",
    summary: "Listens for visibilitychange events on the page. Each event bumps a total counter and adds a record (visible or hidden, plus a timestamp) to a ring that keeps the last 20. Just before a payload is built it re-checks the current visibility and, if anything changed since the last report, sends the records, the total count and the current/initial visibility.",
    why: "Real users switch tabs and minimize windows; the pattern and timing of visibility changes, and a page that is never visible, likely help separate humans from automated or background sessions.",
    sample_payload: Some(r#"{
  "records": [
    {
      "timestamp": 36164.39999997616,
      "visible": false
    },
    {
      "timestamp": 47865.39999997616,
      "visible": true
    }
  ],
  "state": {
    "currentlyVisible": true,
    "initiallyVisible": true
  },
  "visibilityChangeCount": 2
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number("recordCount", "Number of records that follow, 0 to 20."),
        CardField::group("records", &[
            CardField::boolean(
                "visible",
                "Tag byte: one value means the page became visible, another means it became hidden (16 / 17 in the main build).",
            ),
            CardField::number(
                "timestamp",
                "Milliseconds since the script started when the change happened (usually fractional).",
            ),
        ])
        .repeat("once per kept visibility change, oldest to newest"),
        CardField::number(
            "visibilityChangeCount",
            "Total visibilitychange events seen since start (not capped at 20).",
        ),
        CardField::null(
            "stateNullTag",
            "Tag byte written just before state only when the current visibility is unknown (null): 130 or 131 depending on the build, and state then carries only the initially-visible flag. In the analyzed builds this branch is never taken, since the current visibility is always re-sampled as a boolean.",
        ),
        CardField::byte(
            "state",
            "Packed flags: currently visible (page visible right now), initially visible (page was visible when the script started).",
        )
        .flags(&["currentlyVisible", "initiallyVisible"]),
    ]),
};
