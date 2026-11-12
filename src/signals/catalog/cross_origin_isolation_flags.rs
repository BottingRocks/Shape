//! `CrossOriginIsolationFlags`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_in_order, BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, property_census::*, census_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `CrossOriginIsolationFlags`. Card: [`CARD`].
pub(crate) fn recognize_cross_origin_isolation_flags(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let census = census(view, ctx);
    let value = value_members(view);
    let in_value = value.contains("crossOriginIsolated") || value.contains("SharedArrayBuffer");
    (one_byte(view)
        && in_value
        && probes_all(view, &census, &["crossOriginIsolated", "SharedArrayBuffer"])
        && !outranked(&census, "CrossOriginIsolationFlags"))
        .then(|| Claim::new(["crossOriginIsolated / typeof SharedArrayBuffer", "one flag byte"]))
}

pub(crate) fn bind_cross_origin_isolation_flags(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_in_order(layout, &["isolationFlags"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `CrossOriginIsolationFlags`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "CrossOriginIsolationFlags",
    summary: "Checks two facts about the page's cross-origin isolation and sends them once as a packed byte: whether window.crossOriginIsolated is true, and whether the SharedArrayBuffer constructor is available.",
    why: "Browsers expose SharedArrayBuffer only on cross-origin isolated pages (with some legacy and flag exceptions), so comparing the two bits likely reveals the engine/version or a tampered environment.",
    sample_payload: Some(r#"{
  "crossOriginIsolated": false,
  "sharedArrayBufferAvailable": false
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::byte(
            "isolationFlags",
            "Two flags, other bits 0: sharedArrayBufferAvailable (typeof SharedArrayBuffer is 'function'), crossOriginIsolated (window.crossOriginIsolated is true; 0 if false or missing). Each bit is 1 if true, 0 if false.",
        )
        .flags(&["sharedArrayBufferAvailable", "crossOriginIsolated"]),
    ]),
};
