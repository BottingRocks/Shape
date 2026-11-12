//! `ProxyTamperFlags`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafOp, SignalLayout, SlotView};
use buffer_writes::slot_info;
use crate::signals::analysis::{buffer_writes, leaf_checks::*, native_checks::*, flag_bytes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `ProxyTamperFlags`. Card: [`CARD`].
pub(crate) fn recognize_proxy_tamper_flags(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    if !plain_ops(view, &[LeafOp::Byte]) {
        return None;
    }
    let info = slot_info(view, ctx);
    (proxy_kind(&byte_counts(view), &info.facts, &info.store_facts()) == Some(ProxyKind::BrandErrors))
        .then(|| claim(&["one flag byte set on Proxy brand-check error messages"]))
        .flatten()
}

pub(crate) fn bind_proxy_tamper_flags(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_bytes_in_order(layout, &["proxyFlags"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `ProxyTamperFlags`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "ProxyTamperFlags",
    summary: "Checks whether the page's global object and its navigator are wrapped in a JavaScript Proxy. It calls Date.prototype.toString on each and looks for 'Proxy.toString' or 'incompatible Proxy' in the error; for navigator it also tries structuredClone(navigator) and looks for '#<Navigator>' in the error.",
    why: "Anti-detect and stealth tools often wrap window or navigator in a Proxy to fake properties, so this likely detects spoofed browser environments.",
    sample_payload: Some(r#"{
  "globalLooksLikeProxy": false,
  "navigatorLooksLikeProxy": false
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::byte(
            "proxyFlags",
            "Packed flags (order swapped in some builds): global object looks like a Proxy; navigator looks like a Proxy. Value 0 to 3, other bits 0.",
        )
        .flags(&["globalLooksLikeProxy", "navigatorLooksLikeProxy"]),
    ]),
};
