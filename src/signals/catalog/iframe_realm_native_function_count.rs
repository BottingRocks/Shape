//! `IframeRealmNativeFunctionCount`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, native_checks::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `IframeRealmNativeFunctionCount`. Card: [`CARD`].
pub(crate) fn recognize_iframe_realm_native_function_count(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    (native_count(view) == Some(NativeCount::IframeRealm))
        .then(|| claim(&["counted native-code test over a nested realm's own names (csp styleNonce realm helper)"]))
        .flatten()
}

pub(crate) fn bind_iframe_realm_native_function_count(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    crate::signals::analysis::bind_in_order(layout, &["nativeFunctionCount"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `IframeRealmNativeFunctionCount`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "IframeRealmNativeFunctionCount",
    summary: "Creates a hidden nested iframe to get a fresh JavaScript realm, then takes the first 2000 own property names of that iframe's window and keeps the non-enumerable data properties. After the iframe is removed, it counts (in small time slices) how many of those values are functions whose source text reads as native code. It sends that count.",
    why: "A fresh iframe realm shows the browser's untouched built-in surface, so comparing this count with the main window's likely reveals the engine/version and patches or automation frameworks that also tamper with new frames.",
    sample_payload: Some(r#"{
  "nativeFunctionCount": 979
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "nativeFunctionCount",
            "Number of non-enumerable, function-valued global properties in the fresh iframe realm that stringify as native code; an integer from 0 to 2000 (0 if nothing qualified). If the hidden iframe cannot be created or never reports back, nothing is sent.",
        ),
    ]),
};
