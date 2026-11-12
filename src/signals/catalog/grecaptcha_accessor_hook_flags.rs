//! `GrecaptchaAccessorHookFlags`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafOp, SignalLayout, SlotView};
use buffer_writes::slot_info;
use crate::signals::analysis::{buffer_writes, leaf_checks::*, flag_bytes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `GrecaptchaAccessorHookFlags`. Card: [`CARD`].
pub(crate) fn recognize_grecaptcha_accessor_hook_flags(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    if !plain_ops(view, &[LeafOp::Byte, LeafOp::Byte]) {
        return None;
    }
    let info = slot_info(view, ctx);
    let features = info
        .facts
        .count_strings(&["FeaturePolicy", "LayoutShiftAttribution", "CompressionStream"]);
    // `String.fromCharCode(…)` or `String.fromCharCode.apply(null, codes)`.
    let decoded = info.facts.call("fromCharCode")
        || (info.facts.member("fromCharCode") && info.facts.call("apply"))
        || view.leaves().any(|leaf| has_transform(leaf, "String.fromCharCode.apply"));
    (features >= 2 && decoded)
        .then(|| claim(&["own FeaturePolicy/LayoutShiftAttribution/CompressionStream", "decoded accessor markers"]))
        .flatten()
}

pub(crate) fn bind_grecaptcha_accessor_hook_flags(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_bytes_in_order(layout, &["hookFlagsA", "hookFlagsB"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `GrecaptchaAccessorHookFlags`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "GrecaptchaAccessorHookFlags",
    summary: "Walks window and its prototype chain to find where the 'grecaptcha' property (Google reCAPTCHA) is defined, without triggering any getter. At that level it checks whether grecaptcha is a getter/setter accessor rather than a plain value, and whether the accessor's real source text contains 'BrowserAutomationStudio'. It also records whether window has its own FeaturePolicy, LayoutShiftAttribution and CompressionStream properties. Runs once at startup; all 11 answers are yes/no flags packed into two bytes.",
    why: "Browser Automation Studio and similar bot tools hook grecaptcha with accessors to solve or intercept CAPTCHAs, so these flags likely expose that tool directly; the three feature bits are likely browser family/version hints.",
    sample_payload: Some(r#"{
  "hookChecks": [
    {
      "compressionStream": true,
      "descriptorPresent": false,
      "getterHasBasMarker": false,
      "getterIsFunction": false,
      "layoutShiftAttribution": true,
      "ownFound": false,
      "setterHasToString": false
    },
    {
      "featurePolicy": true,
      "getterHasToString": false,
      "setterHasBasMarker": false,
      "setterIsFunction": false
    }
  ]
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::byte(
            "hookFlagsA",
            "Seven flags: featurePolicy (window has own FeaturePolicy), getterHasBasMarker (getter source contains 'BrowserAutomationStudio'), layoutShiftAttribution (own LayoutShiftAttribution), compressionStream (own CompressionStream), getterHasToString (getter's toString is a function), descriptorPresent (a property descriptor was found), setterHasToString (setter's toString is a function). Bit 7 is 0.",
        )
        .flags(&[
            "featurePolicy",
            "getterHasBasMarker",
            "layoutShiftAttribution",
            "compressionStream",
            "getterHasToString",
            "descriptorPresent",
            "setterHasToString",
        ]),
        CardField::byte(
            "hookFlagsB",
            "Four flags: setterHasBasMarker (setter source contains 'BrowserAutomationStudio'), setterIsFunction (grecaptcha has a setter), ownFound (grecaptcha found on window or one of its prototypes), getterIsFunction (grecaptcha has a getter). Bits 4-7 are 0. No reCAPTCHA: all 0; normal reCAPTCHA: only ownFound (and descriptorPresent) set.",
        )
        .flags(&["setterHasBasMarker", "setterIsFunction", "ownFound", "getterIsFunction"]),
    ]),
};
