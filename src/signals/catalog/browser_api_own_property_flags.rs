//! `BrowserApiOwnPropertyFlags`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafOp, SignalLayout, SlotView};
use buffer_writes::slot_info;
use crate::signals::analysis::{buffer_writes, leaf_checks::*, flag_bytes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `BrowserApiOwnPropertyFlags`. Card: [`CARD`].
pub(crate) fn recognize_browser_api_own_property_flags(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    if !plain_ops(view, &[LeafOp::Byte]) {
        return None;
    }
    let info = slot_info(view, ctx);
    let names = info
        .facts
        .count_strings(&["clipboard", "ServiceWorker", "ApplePaySession", "WebkitOverflowScrolling"]);
    (names >= 2)
        .then(|| claim(&["hasOwnProperty of clipboard/ServiceWorker/ApplePaySession/WebkitOverflowScrolling"]))
        .flatten()
}

pub(crate) fn bind_browser_api_own_property_flags(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_bytes_in_order(layout, &["ownPropertyFlags"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `BrowserApiOwnPropertyFlags`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "BrowserApiOwnPropertyFlags",
    summary: "At init, checks with hasOwnProperty whether four names are defined directly on specific objects: \"clipboard\" on Navigator.prototype, \"ServiceWorker\" and \"ApplePaySession\" on the global object, and \"WebkitOverflowScrolling\" on CSSStyleDeclaration.prototype. Only own properties count; inherited values or shims added on instances give 0, and no property value is read. The four results are sent as one byte of flags.",
    why: "Which of these APIs a browser defines, and where, depends on the engine, platform and secure-context state; two of them are Apple/WebKit-specific. This likely helps confirm the claimed browser and OS (for example real Safari) and catch user-agent spoofing.",
    sample_payload: Some(r#"{
  "ApplePaySession": false,
  "ServiceWorker": true,
  "WebkitOverflowScrolling": false,
  "clipboard": true
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::byte(
            "ownPropertyFlags",
            "Four packed flags, each 1 if the property is an own property, 0 otherwise: Navigator.prototype has clipboard, global has ServiceWorker, global has ApplePaySession, CSSStyleDeclaration.prototype has WebkitOverflowScrolling. The upper 4 bits are always 0.",
        )
        .flags(&["clipboard", "ServiceWorker", "ApplePaySession", "WebkitOverflowScrolling"]),
    ]),
};
