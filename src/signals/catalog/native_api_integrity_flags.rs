//! `NativeApiIntegrityFlags`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use buffer_writes::slot_info;
use crate::signals::analysis::{buffer_writes, leaf_checks::*, native_checks::*, flag_bytes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField, CardMessage};


/// `NativeApiIntegrityFlags`. Card: [`CARD`].
pub(crate) fn recognize_native_api_integrity_flags(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    if !only_bytes(view) {
        return None;
    }
    let info = slot_info(view, ctx);
    (integrity_kind(&top_field_counts(view), hash_slots(view), &info.facts) == Some(IntegrityKind::ThreeBytes))
        .then(|| claim(&["three flag bytes of native-getter and class-tag checks"]))
        .flatten()
}

pub(crate) fn bind_native_api_integrity_flags(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_bytes_in_order(layout, &["integrityFlags1", "integrityFlags2", "integrityFlags3"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `NativeApiIntegrityFlags`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "NativeApiIntegrityFlags",
    summary: "Runs 20 yes/no checks that key browser built-ins are still the browser's own native code and have their normal shape: screen and navigator getters, document.createElement/createElementNS, canvas and 2D-context methods and style setters, Function.prototype.toString, and the class tags of navigator.plugins and navigator.mimeTypes. The 20 results are packed into three flag bytes (7 + 7 + 6 bits); 1 means the API looks native. It sends them once at startup and again at the end of the page's life, where a check only stays 1 if it was 1 at startup and still passes.",
    why: "Bots, anti-fingerprinting extensions and spoofing tools often override these APIs (user agent, screen size, plugins, canvas output). A 0 bit likely flags such tampering; an unmodified browser should give all ones.",
    sample_payload: Some(r#"{
  "integrityChecks": [
    {
      "createElementNative": true,
      "functionToStringNative": true,
      "mimeTypesGetterNative": true,
      "pluginsGetterNative": true,
      "pluginsTagNotPlain": true,
      "screenHeightGetterNative": true,
      "strokeStyleAccessorNative": true
    },
    {
      "createElementNSNative": true,
      "documentProtoNoOwnCreateElement": true,
      "fillTextNative": true,
      "fontAccessorNative": true,
      "getImageDataNative": true,
      "screenWidthGetterNative": true,
      "toDataURLNative": true
    },
    {
      "documentNoOwnCreateElement": true,
      "documentPrototypeHasOwnCreateElement": true,
      "fillStyleAccessorNative": true,
      "getContextNative": true,
      "mimeTypesTagNotPlain": true,
      "userAgentGetterNative": true
    }
  ]
}"#),
    by_name: false,
    body: CardBody::Messages(&[
        CardMessage {
            when: "At startup (init) and again at the end of the page's life (last chance); same layout, last-chance bits are 1 only if the check passed both times",
            fields: &[
                CardField::byte(
                    "integrityFlags1",
                    "7 flags, 1 = looks native: screen.width getter native; document.createElementNS native; document's prototype has no own createElement; screen.height getter native; navigator.mimeTypes getter native; navigator.plugins getter native; CanvasRenderingContext2D fillText native. Which flags land in which byte changes per build.",
                )
                .flags(&[
                    "screenWidthGetterNative",
                    "createElementNSNative",
                    "documentProtoNoOwnCreateElement",
                    "screenHeightGetterNative",
                    "mimeTypesGetterNative",
                    "pluginsGetterNative",
                    "fillTextNative",
                    "pluginsTagNotPlain",
                    "getImageDataNative",
                    "userAgentGetterNative",
                    "createElementNative",
                    "mimeTypesTagNotPlain",
                    "strokeStyleAccessorNative",
                    "fillStyleAccessorNative",
                    "toDataURLNative",
                    "functionToStringNative",
                    "documentNoOwnCreateElement",
                    "getContextNative",
                    "fontAccessorNative",
                    "documentPrototypeHasOwnCreateElement",
                ]),
                CardField::byte(
                    "integrityFlags2",
                    "7 flags, 1 = looks native: navigator.plugins class tag is not a plain Object/Array/Function; getImageData native; navigator.userAgent getter native; document.createElement native; navigator.mimeTypes class tag not plain; strokeStyle getter and setter native; fillStyle getter and setter native.",
                ),
                CardField::byte(
                    "integrityFlags3",
                    "6 flags, 1 = looks native: HTMLCanvasElement toDataURL native; Function.prototype.toString native; document has no own createElement; HTMLCanvasElement getContext native; 2D-context font getter and setter native; Document.prototype has its own createElement.",
                ),
            ],
        },
    ]),
};
