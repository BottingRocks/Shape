//! `HistoricDateToString`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_in_order, BindRefusal, Claim, FieldBinding, FinderCtx, LeafView, SignalLayout, SlotView};
use crate::signals::analysis::{shape_text::*, leaf_checks::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `HistoricDateToString` evidence: one STRING, the plain date rendering.
pub(crate) fn is_historic_date(shapes: &[&str], leaves: &[&LeafView]) -> bool {
    top_count_is(shapes, 1)
        && !leaves.is_empty()
        && leaves.iter().all(|leaf| date_to_string(leaf) && !shuffled(leaf))
}

/// `HistoricDateToString`. Card: [`CARD`].
pub(crate) fn recognize_historic_date_to_string(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    if is_historic_date(&view.shapes(), &leaves_of(view)) {
        return claim(&["one STRING", "new Date(K).toString()", "no shuffle"]);
    }
    None
}

pub(crate) fn bind_historic_date_to_string(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_in_order(layout, &["historicDateString"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `HistoricDateToString`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "HistoricDateToString",
    summary: "Formats one fixed moment in the past, 1945-08-05 23:16 UTC, with new Date(...).toString() and sends the resulting text. Because toString uses the device's local time zone, the text shows the local date and time for that moment, the historical GMT offset, and the browser's name for the time zone. Runs once at startup.",
    why: "Likely a time-zone fingerprint: zones that share an offset today often had different rules in 1945, and the zone display name depends on engine and locale, so it can likely expose time-zone spoofing that only patches the current offset.",
    sample_payload: Some(r#"{
  "historicDateString": "Sun Aug 05 1945 16:16:00 GMT-0700 (Pacific Daylight Time)"
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::string(
            "historicDateString",
            "Date.prototype.toString of 1945-08-05T23:16:00Z in local time, e.g. 'Mon Aug 06 1945 08:16:00 GMT+0900 (Japan Standard Time)'.",
        ),
    ]),
};
