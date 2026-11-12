//! `LocationPropertyNames`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_in_order, BindRefusal, Claim, FieldBinding, FinderCtx, LeafOp, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, property_census::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `LocationPropertyNames`. Card: [`CARD`].
pub(crate) fn recognize_location_property_names(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let census = census(view, ctx);
    // The string is built from a key-code table lookup
    // (`(table[key] || "u") + "\0"`, or the fallback as an `if`): in the
    // value provenance, or (when the lookup is a local of the loop body,
    // `let c = table[key] || "u"; s = s + (c + "\0")`) in the census as a
    // table indexed by the `for…in` key.
    let code_lookup = census.key_lookup
        || view
            .leaves()
            .any(|leaf| leaf.transforms.iter().any(|t| t == "[index]"));
    (plain_ops(view, &[LeafOp::String])
        && code_lookup
        && (census.member("location") || view.value_reads_api("location"))
        && (census.calls("keys") || view.leaves().any(|leaf| leaf.transforms.iter().any(|t| t == "Object.keys"))))
        .then(|| Claim::new(["for-in over location", "one key-code string"]))
}

pub(crate) fn bind_location_property_names(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_in_order(layout, &["locationKeyCodes"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `LocationPropertyNames`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "LocationPropertyNames",
    summary: "Lists the property names on window.location (not their values) in the order the browser enumerates them. Each known name becomes one letter: ancestorOrigins a, assign b, fragmentDirective c, hash d, host e, hostname f, href g, origin h, pathname i, port j, protocol k, reload l, replace m, search n, toString o. Any other name becomes u. If enumeration finds more names than Object.keys(location) reports (capped at 30), the list is cut off after one extra entry.",
    why: "The set and order of location properties differ between browser engines, and extra or unknown names reveal a spoofed, proxied or patched location object, as used by some bot frameworks (likely).",
    sample_payload: Some(r#"{
  "locationKeyCodes": "a\u0000g\u0000h\u0000k\u0000e\u0000f\u0000j\u0000i\u0000n\u0000d\u0000b\u0000l\u0000m\u0000o\u0000"
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::string(
            "locationKeyCodes",
            "One letter code per enumerated location property, each followed by a NUL separator, e.g. \"a\\0g\\0h\\0k\\0…\". Empty string if location is missing.",
        ),
    ]),
};
