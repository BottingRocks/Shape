//! `GlobalPropertyChecks`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_in_order, top_level_ops, BindRefusal, Claim, FieldBinding, FinderCtx, LeafOp, SignalLayout, SlotView};
use crate::signals::analysis::{LayoutNode, LeafKind};
use crate::signals::analysis::{property_census::*, census_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


fn probe_count(view: &SlotView, census: &Census, names: &[&str]) -> usize {
    names.iter().filter(|name| probes(view, census, name)).count()
}

/// The flags of the global presence tests (a sample of the card list).
const PRESENCE_TESTS: &[&str] = &[
    "callPhantom",
    "_phantom",
    "__webdriver_script_fn",
    "__fxdriver_unwrapped",
    "_Selenium_IDE_Recorder",
    "ActiveXObject",
    "XDomainRequest",
    "mozRTCPeerConnection",
    "webkitRTCPeerConnection",
    "Sequentum",
    "documentMode",
    "globalStorage",
];

/// `GlobalPropertyChecks`. Card: [`CARD`].
pub(crate) fn recognize_global_property_checks(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let census = census(view, ctx);
    let ops = top_level_ops(view);
    (ops.len() >= 5
        && ops.iter().all(|op| *op == LeafOp::Byte)
        && probe_count(view, &census, PRESENCE_TESTS) >= 6)
        .then(|| Claim::new(["automation / legacy global presence tests", "packed presence bytes"]))
}

pub(crate) fn bind_global_property_checks(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    // Nine plain bytes written in wire order: presenceBits1 (first) ..
    // presenceBits9 (last, the 3-test byte). The card numbers them by wire
    // position, so the order needs no per-build mapping.
    const NAMES: [&str; 9] = [
        "presenceBits1",
        "presenceBits2",
        "presenceBits3",
        "presenceBits4",
        "presenceBits5",
        "presenceBits6",
        "presenceBits7",
        "presenceBits8",
        "presenceBits9",
    ];
    let message = layout.messages.first().ok_or_else(|| BindRefusal::new("no message layout"))?;
    let bytes = message
        .fields
        .iter()
        .all(|field| matches!(&field.node, LayoutNode::Leaf(leaf) if leaf.kind == LeafKind::Byte));
    if !bytes || message.fields.len() != NAMES.len() {
        return Err(BindRefusal::new(format!(
            "expected nine plain presence bytes, found {} fields",
            message.fields.len()
        )));
    }
    bind_in_order(layout, &NAMES)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `GlobalPropertyChecks`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "GlobalPropertyChecks",
    summary: "Runs 59 'name in object' presence tests against window, document, document.body, navigator, window.crypto and window.external, looking for automation markers (Selenium, ChromeDriver, PhantomJS, WebDriver, Sequentum), old IE / Gecko / WebKit APIs and capability APIs. Inherited properties count and no getters run; if document.body is missing, its tests are 0. The 59 yes/no answers are packed 7 per byte into nine bytes, presenceBits1 to presenceBits9, written in that order.",
    why: "Likely to catch automation and headless tools directly through their leftover globals, and to check that the claimed browser engine matches which legacy and vendor APIs are present.",
    sample_payload: Some(r#"{
  "presenceBits": [
    {
      "body.contextMenu": false,
      "body.webkitRequestFullScreen": true,
      "navigator.bluetooth": true,
      "navigator.credentials": true,
      "navigator.vibrate": true,
      "window.ApplePaySession": false,
      "window.event": true
    },
    {
      "document.characterSet": true,
      "document.charset": true,
      "document.compatMode": true,
      "window.attachEvent": false,
      "window.fireEvent": false,
      "window.frameElement": true,
      "window.netscape": false
    },
    {
      "body.requestFullScreen": false,
      "document._Selenium_IDE_Recorder": false,
      "window.Notification": true,
      "window.__fxdriver_unwrapped": false,
      "window._phantom": false,
      "window.postMessage": true,
      "window.webkitResolveLocalFileSystemURL": true
    },
    {
      "crypto.subtle": true,
      "document.__webdriver_script_fn": false,
      "document.all": true,
      "document.images": true,
      "window.XDomainRequest": false,
      "window.callPhantom": false,
      "window.sessionStorage": true
    },
    {
      "document.layers": false,
      "external.Sequentum": false,
      "navigator.webdriver": true,
      "window.MutationObserver": true,
      "window.TouchEvent": true,
      "window.mozRTCPeerConnection": false,
      "window.registerProtocolHandler": false
    },
    {
      "body.innerText": true,
      "body.mozRequestFullScreen": false,
      "document.$cdc_asdjflasutopfhvcZLmcfl_": false,
      "navigator.storage": true,
      "window.phantom": false,
      "window.sidebar": false,
      "window.webkitRequestAnimationFrame": true
    },
    {
      "document.documentMode": false,
      "window.ActiveXObject": false,
      "window.BluetoothUUID": true,
      "window.File": true,
      "window.createPopup": false,
      "window.localStorage": true,
      "window.webkitRTCPeerConnection": true
    },
    {
      "document.__fxdriver_unwrapped": false,
      "window.PushManager": true,
      "window._Selenium_IDE_Recorder": false,
      "window.detachEvent": false,
      "window.external": true,
      "window.globalStorage": false,
      "window.mozRequestAnimationFrame": false
    },
    {
      "navigator.requestMediaKeySystemAccess": true,
      "window.SharedWorker": true,
      "window.requestAnimationFrame": true
    }
  ]
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::byte(
            "presenceBits1",
            "Byte 1 of 9 (wire byte 4 after the 4-byte seed). Carries 7 presence tests in bits 0-6; bit 7 is always 0. One bit per test: 1 if the property name is present on its object (own or inherited), 0 if not or if the object is missing. Which tests this byte packs, and in which bits, varies per build; the 59 tests are listed in the flags of presenceBits1.",
        )
        .flags(&[
            "window.ActiveXObject",
            "window.ApplePaySession",
            "window.File",
            "window.MutationObserver",
            "window.Notification",
            "window.SharedWorker",
            "window.TouchEvent",
            "window.XDomainRequest",
            "window._phantom",
            "window.attachEvent",
            "window.callPhantom",
            "window.createPopup",
            "window.detachEvent",
            "window.event",
            "window.external",
            "window.fireEvent",
            "window.frameElement",
            "window.globalStorage",
            "window.localStorage",
            "window.mozRTCPeerConnection",
            "window.mozRequestAnimationFrame",
            "window.phantom",
            "window.postMessage",
            "window.PushManager",
            "window.registerProtocolHandler",
            "window.requestAnimationFrame",
            "window.sessionStorage",
            "window.sidebar",
            "window.webkitRequestAnimationFrame",
            "window.webkitResolveLocalFileSystemURL",
            "window.webkitRTCPeerConnection",
            "window.BluetoothUUID",
            "window.netscape",
            "window.__fxdriver_unwrapped",
            "window._Selenium_IDE_Recorder",
            "document._Selenium_IDE_Recorder",
            "document.all",
            "document.characterSet",
            "document.charset",
            "document.compatMode",
            "document.documentMode",
            "document.images",
            "document.layers",
            "document.$cdc_asdjflasutopfhvcZLmcfl_",
            "document.__fxdriver_unwrapped",
            "document.__webdriver_script_fn",
            "body.contextMenu",
            "body.innerText",
            "body.mozRequestFullScreen",
            "body.requestFullScreen",
            "body.webkitRequestFullScreen",
            "navigator.vibrate",
            "navigator.webdriver",
            "navigator.credentials",
            "navigator.storage",
            "navigator.requestMediaKeySystemAccess",
            "navigator.bluetooth",
            "crypto.subtle",
            "external.Sequentum",
        ]),
        CardField::byte(
            "presenceBits2",
            "Byte 2 of 9 (wire byte 5 after the 4-byte seed). Carries 7 presence tests in bits 0-6; bit 7 is always 0. One bit per test: 1 if the property name is present on its object (own or inherited), 0 if not or if the object is missing. Which tests this byte packs, and in which bits, varies per build; the 59 tests are listed in the flags of presenceBits1.",
        ),
        CardField::byte(
            "presenceBits3",
            "Byte 3 of 9 (wire byte 6 after the 4-byte seed). Carries 7 presence tests in bits 0-6; bit 7 is always 0. One bit per test: 1 if the property name is present on its object (own or inherited), 0 if not or if the object is missing. Which tests this byte packs, and in which bits, varies per build; the 59 tests are listed in the flags of presenceBits1.",
        ),
        CardField::byte(
            "presenceBits4",
            "Byte 4 of 9 (wire byte 7 after the 4-byte seed). Carries 7 presence tests in bits 0-6; bit 7 is always 0. One bit per test: 1 if the property name is present on its object (own or inherited), 0 if not or if the object is missing. Which tests this byte packs, and in which bits, varies per build; the 59 tests are listed in the flags of presenceBits1.",
        ),
        CardField::byte(
            "presenceBits5",
            "Byte 5 of 9 (wire byte 8 after the 4-byte seed). Carries 7 presence tests in bits 0-6; bit 7 is always 0. One bit per test: 1 if the property name is present on its object (own or inherited), 0 if not or if the object is missing. Which tests this byte packs, and in which bits, varies per build; the 59 tests are listed in the flags of presenceBits1.",
        ),
        CardField::byte(
            "presenceBits6",
            "Byte 6 of 9 (wire byte 9 after the 4-byte seed). Carries 7 presence tests in bits 0-6; bit 7 is always 0. One bit per test: 1 if the property name is present on its object (own or inherited), 0 if not or if the object is missing. Which tests this byte packs, and in which bits, varies per build; the 59 tests are listed in the flags of presenceBits1.",
        ),
        CardField::byte(
            "presenceBits7",
            "Byte 7 of 9 (wire byte 10 after the 4-byte seed). Carries 7 presence tests in bits 0-6; bit 7 is always 0. One bit per test: 1 if the property name is present on its object (own or inherited), 0 if not or if the object is missing. Which tests this byte packs, and in which bits, varies per build; the 59 tests are listed in the flags of presenceBits1.",
        ),
        CardField::byte(
            "presenceBits8",
            "Byte 8 of 9 (wire byte 11 after the 4-byte seed). Carries 7 presence tests in bits 0-6; bit 7 is always 0. One bit per test: 1 if the property name is present on its object (own or inherited), 0 if not or if the object is missing. Which tests this byte packs, and in which bits, varies per build; the 59 tests are listed in the flags of presenceBits1.",
        ),
        CardField::byte(
            "presenceBits9",
            "Byte 9 of 9, the last (wire byte 12). Carries the remaining 3 presence tests in bits 0-2; bits 3-7 are always 0. One bit per test: 1 if the property name is present on its object (own or inherited), 0 if not or if the object is missing. Which tests this byte packs, and in which bits, varies per build; the 59 tests are listed in the flags of presenceBits1.",
        ),
    ]),
};
