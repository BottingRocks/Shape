//! `DeviceMemory`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_in_order, BindRefusal, Claim, FieldBinding, FinderCtx, LeafOp, LeafView, SignalLayout, SlotView};
use crate::signals::analysis::{shape_text::*, leaf_checks::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


// ===========================================================================
// Recognizers and binders
// ===========================================================================

/// `DeviceMemory`. Card: [`CARD`].
pub(crate) fn recognize_device_memory(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    // The memory value is a number; a string leaf's deviceMemory api is
    // register-reuse pollution (the location census reuses the same local).
    let numbers: Vec<&LeafView> = leaves_of(view).into_iter().filter(|leaf| leaf.op == LeafOp::Number).collect();
    if top_count_is(&view.shapes(), 1) && any_api(&numbers, "navigator.deviceMemory") {
        return claim(&["one field", "value reads navigator.deviceMemory"]);
    }
    None
}

pub(crate) fn bind_device_memory(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_in_order(layout, &["deviceMemory"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `DeviceMemory`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "DeviceMemory",
    summary: "Reads navigator.deviceMemory once and sends it. If the property is missing (null or undefined), it sends a missing marker instead.",
    why: "Device memory is a coarse hardware trait that adds entropy to a fingerprint and can be cross-checked against the claimed device and browser (e.g. it does not exist in Firefox or Safari).",
    sample_payload: Some(r#"{
  "deviceMemory": 32
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "deviceMemory",
            "Approximate RAM in gigabytes as reported by the browser (Chrome reports buckets such as 0.25, 0.5, 1, 2, 4, 8), or a null marker if navigator.deviceMemory is missing.",
        ).nullable(),
    ]),
};
