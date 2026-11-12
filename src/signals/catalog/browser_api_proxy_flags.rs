//! `BrowserApiProxyFlags`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use buffer_writes::slot_info;
use crate::signals::analysis::{buffer_writes, leaf_checks::*, native_checks::*, flag_bytes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


// ===========================================================================
// Recognizers and binders
// ===========================================================================

/// `BrowserApiProxyFlags`. Card: [`CARD`].
pub(crate) fn recognize_browser_api_proxy_flags(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    if !only_bytes(view) {
        return None;
    }
    let info = slot_info(view, ctx);
    (proxy_kind(&byte_counts(view), &info.facts, &info.store_facts()) == Some(ProxyKind::ApiDetectors))
        .then(|| claim(&["flag bytes of typeof object/function detector checks", "Proxy availability"]))
        .flatten()
}

pub(crate) fn bind_browser_api_proxy_flags(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    // Gated builds send `ks() ^ 0` bytes when the gate is closed.
    bind_flag_bytes(layout, &["proxyFlags1", "proxyFlags2", "proxyFlags3"], true)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `BrowserApiProxyFlags`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "BrowserApiProxyFlags",
    summary: "Checks 17 browser API objects and functions to see if any of them is a Proxy or otherwise behaves abnormally when its prototype is changed. For each one it tries to build a prototype cycle and watches whether the engine's normal cycle error happens. It also records whether Proxy exists. The 18 results are packed into three bytes of flags.",
    why: "Anti-fingerprinting extensions, automation frameworks and spoofing tools often wrap APIs like canvas readback, navigator properties or Function.prototype.toString in a Proxy to fake their values. In an unmodified browser all detector flags should be 0 (and the Proxy flag 1), so any set flag likely marks a tampered environment.",
    sample_payload: Some(r#"{
  "proxyChecks": [
    {
      "CanvasRenderingContext2D.prototype.getImageData": false,
      "Function.prototype.toString": false,
      "HTMLCanvasElement.prototype.toDataURL": false,
      "Intl.supportedValuesOf": false,
      "Navigator.prototype.hardwareConcurrency": false,
      "Proxy": true,
      "navigator.mimeTypes": false
    },
    {
      "HTMLMediaElement.prototype.canPlayType": false,
      "Intl": false,
      "Navigator.prototype.languages": false,
      "Navigator.prototype.plugins": false,
      "Permissions.prototype.canPlayType": false,
      "document.createElement": false,
      "navigator.getBattery": false
    },
    {
      "WebGLRenderingContext.prototype.getParameter": false,
      "navigator": false,
      "navigator.plugins": false,
      "speechSynthesis": false
    }
  ]
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::byte(
            "proxyFlags1",
            "7 of the 18 flags in the low 7 bits. A detector flag is 1 if the API value exists and behaves like a Proxy or has a locked or abnormal prototype, 0 otherwise. Detector flags cover: HTMLMediaElement.prototype.canPlayType, speechSynthesis, the Navigator.prototype getters for languages, hardwareConcurrency and plugins, HTMLCanvasElement.prototype.toDataURL, navigator.mimeTypes, navigator.plugins, navigator.getBattery, navigator itself, Function.prototype.toString, Intl, Intl.supportedValuesOf, document.createElement, WebGLRenderingContext.prototype.getParameter, CanvasRenderingContext2D.prototype.getImageData and Permissions.prototype.canPlayType (normally missing, so 0). The 18th flag is 1 if Proxy is available.",
        )
        .flags(&[
            "HTMLMediaElement.prototype.canPlayType",
            "speechSynthesis",
            "Navigator.prototype.languages",
            "Navigator.prototype.hardwareConcurrency",
            "Navigator.prototype.plugins",
            "HTMLCanvasElement.prototype.toDataURL",
            "navigator.mimeTypes",
            "navigator.plugins",
            "navigator.getBattery",
            "navigator",
            "Function.prototype.toString",
            "Intl",
            "Intl.supportedValuesOf",
            "document.createElement",
            "WebGLRenderingContext.prototype.getParameter",
            "CanvasRenderingContext2D.prototype.getImageData",
            "Permissions.prototype.canPlayType",
            "Proxy",
        ]),
        CardField::byte(
            "proxyFlags2",
            "Another 7 of the 18 flags in the low 7 bits; same meaning as proxyFlags1.",
        ),
        CardField::byte(
            "proxyFlags3",
            "The last 4 of the 18 flags in the low 4 bits; same meaning as proxyFlags1. The upper 4 bits are 0.",
        ),
    ]),
};
