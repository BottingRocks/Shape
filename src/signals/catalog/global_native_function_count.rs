//! `GlobalNativeFunctionCount`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, native_checks::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `GlobalNativeFunctionCount`. Card: [`CARD`].
pub(crate) fn recognize_global_native_function_count(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    (native_count(view) == Some(NativeCount::GlobalRealm))
        .then(|| claim(&["counted native-code test over context.global own non-enumerable values"]))
        .flatten()
}

pub(crate) fn bind_global_native_function_count(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    crate::signals::analysis::bind_in_order(layout, &["nativeFunctionCount"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `GlobalNativeFunctionCount`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "GlobalNativeFunctionCount",
    summary: "Takes the first 2000 own property names of the global object (window) and counts those that are non-enumerable data properties holding a function whose source text still reads as native code (\"{ [native code] }\"). That covers built-ins like Object, Array and parseInt and DOM interfaces like HTMLElement; enumerable globals such as fetch, setTimeout or page-defined functions are not counted.",
    why: "The count is characteristic of the browser engine and version, and drops when natives are deleted or replaced with non-native wrappers, so it likely helps verify the claimed browser and spot tampered or emulated environments.",
    sample_payload: Some(r#"{
  "nativeFunctionCount": 977
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "nativeFunctionCount",
            "Number of non-enumerable, function-valued global properties that stringify as native code; an integer from 0 to 2000 (typically several hundred in a real browser).",
        ),
    ]),
};
