//! `NamespacePropertyNameHashes`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, property_census::*, census_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `NamespacePropertyNameHashes`. Card: [`CARD`].
pub(crate) fn recognize_namespace_property_name_hashes(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let census = census(view, ctx);
    (five_numbers(view) && namespace_table(&census).is_some() && !census.frame_realm)
        .then(|| Claim::new(["namespace path table walked from the context global", "five hashes"]))
}

pub(crate) fn bind_namespace_property_name_hashes(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_by_path(view, layout, ctx, &[
        (&["Navigator", "prototype"], "navigatorProtoHash"),
        (&["CanvasRenderingContext2D", "prototype"], "canvas2dProtoHash"),
        (&["chrome", "app"], "chromeAppHash"),
        (&["ApplePaySetup", "prototype"], "applePaySetupProtoHash"),
        (&["safari"], "safariHash"),
    ])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `NamespacePropertyNameHashes`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "NamespacePropertyNameHashes",
    summary: "For five fixed browser objects (Navigator.prototype, CanvasRenderingContext2D.prototype, chrome.app, safari and ApplePaySetup.prototype), it collects every enumerable property name (for...in), joins them with a NUL character and hashes the result. An object that does not exist counts as an empty list. The five hashes are sent as five numbers once all are done.",
    why: "The presence and API surface of these objects show the browser vendor and version (chrome.app only in Chrome, safari and ApplePaySetup only in Safari) and likely reveal tampering such as injected or hidden properties on navigator or canvas.",
    sample_payload: Some(r#"{
  "applePaySetupProtoHash": 2147483648,
  "canvas2dProtoHash": 2167857330,
  "chromeAppHash": 3451360684,
  "navigatorProtoHash": 949388909,
  "safariHash": 2147483648
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "navigatorProtoHash",
            "Hash of the enumerable property names of Navigator.prototype, as an unsigned 32-bit value (0 to 4294967295). An empty or missing object gives 2147483648.",
        ),
        CardField::number(
            "canvas2dProtoHash",
            "Same hash for CanvasRenderingContext2D.prototype. 2147483648 if missing or empty.",
        ),
        CardField::number(
            "chromeAppHash",
            "Same hash for window.chrome.app (Chrome only). 2147483648 if missing or empty.",
        ),
        CardField::number(
            "applePaySetupProtoHash",
            "Same hash for ApplePaySetup.prototype (Safari only). 2147483648 if missing or empty.",
        ),
        CardField::number(
            "safariHash",
            "Same hash for window.safari (Safari only). 2147483648 if missing or empty.",
        ),
    ]),
};
