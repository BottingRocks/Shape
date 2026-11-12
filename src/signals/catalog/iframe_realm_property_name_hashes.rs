//! `IframeRealmPropertyNameHashes`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, property_census::*, census_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `IframeRealmPropertyNameHashes`. Card: [`CARD`].
pub(crate) fn recognize_iframe_realm_property_name_hashes(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let census = census(view, ctx);
    (five_numbers(view) && namespace_table(&census).is_some() && census.frame_realm)
        .then(|| Claim::new(["namespace path table walked in a nested frame realm", "five hashes"]))
}

pub(crate) fn bind_iframe_realm_property_name_hashes(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_by_path(view, layout, ctx, &[
        (&["safari"], "safariHash"),
        (&["chrome", "app"], "chromeAppHash"),
        (&["CanvasRenderingContext2D", "prototype"], "canvasContext2dHash"),
        (&["ApplePaySetup", "prototype"], "applePaySetupHash"),
        (&["Navigator", "prototype"], "navigatorHash"),
    ])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `IframeRealmPropertyNameHashes`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "IframeRealmPropertyNameHashes",
    summary: "Creates a hidden nested iframe to get a fresh JavaScript realm and looks up five objects there: Navigator.prototype, CanvasRenderingContext2D.prototype, chrome.app, safari and ApplePaySetup.prototype. For each, it lists the property names a for...in loop yields (own and inherited, in engine order), joins them and hashes the list. It sends the five hashes; a missing object gives an empty list.",
    why: "Which properties these objects have, and in what order, likely identifies browser family and version (chrome.app for Chromium, safari / ApplePaySetup for Safari), taken from a realm page scripts have not touched so it can be compared with the main window.",
    sample_payload: Some(r#"{
  "applePaySetupHash": 2147483648,
  "canvasContext2dHash": 2167857330,
  "chromeAppHash": 2147483648,
  "navigatorHash": 949388909,
  "safariHash": 2147483648
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "safariHash",
            "32-bit hash of the enumerable property names of window.safari in the iframe, shifted to the range 0 to 2^32-1; exactly 2147483648 if the object is missing.",
        ),
        CardField::number(
            "chromeAppHash",
            "Same hash for chrome.app (normally present only in Chromium browsers); 2147483648 if missing.",
        ),
        CardField::number(
            "canvasContext2dHash",
            "Same hash for CanvasRenderingContext2D.prototype; 2147483648 if missing.",
        ),
        CardField::number(
            "applePaySetupHash",
            "Same hash for ApplePaySetup.prototype (Safari with Apple Pay); 2147483648 if missing.",
        ),
        CardField::number("navigatorHash", "Same hash for Navigator.prototype; 2147483648 if missing."),
    ]),
};
