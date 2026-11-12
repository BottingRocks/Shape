//! `NestedFrameAnchorPropertyNamesHash`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_in_order, BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, property_census::*, census_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `NestedFrameAnchorPropertyNamesHash`. Card: [`CARD`].
pub(crate) fn recognize_nested_frame_anchor_property_names_hash(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let census = census(view, ctx);
    (one_number(view) && anchor_key_walk(&census) && census.frame_realm)
        .then(|| Claim::new(["createElement(\"a\") in a nested frame document", "for-in key walk", "one hash"]))
}

pub(crate) fn bind_nested_frame_anchor_property_names_hash(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_in_order(layout, &["keysHash"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `NestedFrameAnchorPropertyNamesHash`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "NestedFrameAnchorPropertyNamesHash",
    summary: "Creates a hidden iframe that loads a second, nested iframe, then creates an <a> element inside that nested frame's document. It lists every enumerable property name of the anchor (own and inherited, in for-in order), joins them into one string with a space before each name, and sends a 32-bit hash of that string. The frames are removed right afterwards.",
    why: "The set and order of anchor properties differ between browser engines and versions, so the hash is a compact engine fingerprint; doing it in a fresh nested frame likely bypasses property patches that stealth tools apply to the top-level page.",
    sample_payload: Some(r#"{
  "keysHash": 1865700265
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "keysHash",
            "Signed 32-bit hash of the string \" key1 key2 ... keyN\" built from the anchor's enumerable property names. 0 if the anchor has no enumerable keys.",
        ),
    ]),
};
