//! `AnchorProperties`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_in_order, BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, property_census::*, census_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


// ===========================================================================
// Signals
// ===========================================================================

/// `AnchorProperties`. Card: [`CARD`].
pub(crate) fn recognize_anchor_properties(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let census = census(view, ctx);
    (one_number(view) && anchor_key_walk(&census) && !census.frame_realm)
        .then(|| Claim::new(["createElement(\"a\") in the producer document", "for-in key walk", "one hash"]))
}

pub(crate) fn bind_anchor_properties(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_in_order(layout, &["keysHash"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `AnchorProperties`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "AnchorProperties",
    summary: "Creates one detached <a> element and lists every enumerable property name that for-in finds on it and its prototype chain (HTMLAnchorElement, HTMLElement, Element, Node, EventTarget, Object), in the engine's enumeration order. It joins the names with spaces, hashes the string to a 32-bit number, and sends only that hash. If the element cannot be created, the signal completes without a payload.",
    why: "The set and order of DOM properties differs by browser engine, version and enabled features, and it changes when automation tools or extensions add enumerable properties to element or Object prototypes. This likely makes it a compact engine fingerprint and a tampering check.",
    sample_payload: Some(r#"{
  "keysHash": 1865700265
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "keysHash",
            "Signed 32-bit hash of every enumerable property name on a new <a> element, each preceded by a space (empty string if there are none).",
        ),
    ]),
};
