//! `GlobalOwnPropertyBits`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_messages_in_order, BindRefusal, Claim, FieldBinding, FinderCtx, LeafOp, SignalLayout, SlotView};
use crate::signals::analysis::{LayoutNode, LeafKind};
use crate::signals::analysis::{leaf_checks::*, property_census::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `GlobalOwnPropertyBits`. Card: [`CARD`].
pub(crate) fn recognize_global_own_property_bits(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let census = census(view, ctx);
    // The bits come from `hasOwnProperty.call(global, name)` in the value
    // provenance, packed by a byte loop.
    let own_property = view
        .leaves()
        .filter(|leaf| leaf.op == LeafOp::Byte)
        .any(|leaf| leaf.transforms.iter().any(|t| t.ends_with("hasOwnProperty.call")));
    let byte_loop = view.leaves().any(|leaf| leaf.op == LeafOp::Byte && leaf.repeat_depth == 1);
    (count_then_bit_bytes(view) && byte_loop && own_property && !census.string_containing("xn--"))
        .then(|| Claim::new(["hasOwnProperty(global, name) per listed name", "count + packed bit bytes"]))
}

pub(crate) fn bind_global_own_property_bits(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    // probeCount, then the packed bits: the byte loop (`ownPropertyBits`)
    // and the trailing partial byte written after it (`ownPropertyBitsTail`).
    // Some builds write no trailing byte after the loop (the program text
    // has no flush after it); their message is count and loop only.
    // A packed byte is written plainly or under its flush test (`if (bits)
    // write`): a byte, or a choice of a byte and nothing.
    let packed_byte = |node: &LayoutNode| match node {
        LayoutNode::Leaf(leaf) => leaf.kind == LeafKind::Byte,
        LayoutNode::Choice { arms } => {
            arms.iter().any(|arm| matches!(&arm.node, LayoutNode::Leaf(_)))
                && arms.iter().all(|arm| match &arm.node {
                    LayoutNode::Leaf(leaf) => leaf.kind == LeafKind::Byte,
                    LayoutNode::Empty => true,
                    _ => false,
                })
        }
        _ => false,
    };
    if layout.messages.is_empty() {
        return Err(BindRefusal::new("no message layout"));
    }
    const NAMES: [&str; 3] = ["probeCount", "ownPropertyBits", "ownPropertyBitsTail"];
    let mut names: Vec<&[&str]> = Vec::new();
    for message in &layout.messages {
        let kinds: Vec<_> = message.fields.iter().map(|field| &field.node).collect();
        match kinds.as_slice() {
            [LayoutNode::Leaf(count), LayoutNode::Array { item }, tail]
                if count.kind == LeafKind::Number && packed_byte(item) && packed_byte(tail) =>
            {
                names.push(&NAMES);
            }
            [LayoutNode::Leaf(count), LayoutNode::Array { item }]
                if count.kind == LeafKind::Number && packed_byte(item) =>
            {
                names.push(&NAMES[..2]);
            }
            _ => return Err(BindRefusal::new("expected count, byte loop, trailing byte")),
        }
    }
    bind_messages_in_order(layout, &names)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `GlobalOwnPropertyBits`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "GlobalOwnPropertyBits",
    summary: "Checks a fixed list of 44 browser global names (recent or engine-specific interfaces, functions and event-handler properties) and records, for each one, whether the global object (window) has it as its own property. A check that throws counts as 'no'. It sends the number of checks followed by the yes/no answers packed 8 per byte.",
    why: "Which of these newer or vendor-specific APIs exist is a likely fingerprint of browser engine and version; it also likely catches globals that were deleted or moved onto a prototype by automation or privacy tools.",
    sample_payload: Some(r#"{
  "ownPropertyBits": {
    "AbstractRange": true,
    "ApplicationCache": false,
    "AudioData": true,
    "AudioTrack": false,
    "BarcodeDetector": true,
    "BrowserCaptureMediaStreamTrack": true,
    "CSSCounterStyleRule": true,
    "CSSFontPaletteValuesRule": true,
    "CSSLayerBlockRule": true,
    "CSSMathClamp": true,
    "DOMRectList": true,
    "EventCounts": true,
    "EyeDropper": true,
    "FileSystemDirectoryHandle": true,
    "FinalizationRegistry": true,
    "FontData": true,
    "FragmentDirective": true,
    "HID": true,
    "HTMLDialogElement": true,
    "HTMLMenuItemElement": false,
    "IDBFileHandle": false,
    "IdleDetector": true,
    "LaunchParams": true,
    "Lock": true,
    "NavigationPreloadManager": true,
    "PerformancePaintTiming": true,
    "RTCDtlsTransport": true,
    "Sanitizer": true,
    "ServiceWorkerContainer": true,
    "SharedArrayBuffer": false,
    "VisualViewport": true,
    "WritableStreamDefaultController": true,
    "XRHand": true,
    "XRHitTestResult": true,
    "content": false,
    "crossOriginIsolated": true,
    "onbeforeinput": true,
    "onsecuritypolicyviolation": true,
    "onselectionchange": true,
    "structuredClone": true
  },
  "ownPropertyBitsTail": {
    "MediaSourceHandle": true,
    "OffscreenCanvas": true,
    "ReadableByteStreamController": true,
    "XRCamera": true
  },
  "probeCount": 44
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number("probeCount", "Number of names checked; always 44."),
        CardField::byte(
            "ownPropertyBits",
            "One bit per name: 1 if window has that name as an own property, 0 if not (or if the check threw).",
        )
        .repeat("5 bytes from the packing loop, 8 checks per byte, filled from bit 0 in the build's iteration order over the list below (ascending in some builds, descending in others)")
        .flags(&[
            "XRHitTestResult",
            "crossOriginIsolated",
            "AbstractRange",
            "RTCDtlsTransport",
            "BarcodeDetector",
            "EventCounts",
            "SharedArrayBuffer",
            "VisualViewport",
            "FinalizationRegistry",
            "WritableStreamDefaultController",
            "HTMLMenuItemElement",
            "HID",
            "CSSCounterStyleRule",
            "FileSystemDirectoryHandle",
            "PerformancePaintTiming",
            "onbeforeinput",
            "ApplicationCache",
            "AudioData",
            "AudioTrack",
            "DOMRectList",
            "FragmentDirective",
            "Sanitizer",
            "onselectionchange",
            "onsecuritypolicyviolation",
            "structuredClone",
            "XRHand",
            "Lock",
            "EyeDropper",
            "CSSLayerBlockRule",
            "ServiceWorkerContainer",
            "IdleDetector",
            "HTMLDialogElement",
            "CSSFontPaletteValuesRule",
            "CSSMathClamp",
            "NavigationPreloadManager",
            "LaunchParams",
            "content",
            "IDBFileHandle",
            "BrowserCaptureMediaStreamTrack",
            "FontData",
            "ReadableByteStreamController",
            "OffscreenCanvas",
            "MediaSourceHandle",
            "XRCamera",
        ]),
        CardField::byte(
            "ownPropertyBitsTail",
            "The trailing byte written after the loop, holding the remaining 4 checks (bits 4-7 are 0): ReadableByteStreamController, OffscreenCanvas, MediaSourceHandle, XRCamera bits 0-3 in ascending builds; RTCDtlsTransport, AbstractRange, crossOriginIsolated, XRHitTestResult in bits 0-3 in descending builds. 1 if window has that name as an own property, 0 if not (or if the check threw).",
        ),
    ]),
};
