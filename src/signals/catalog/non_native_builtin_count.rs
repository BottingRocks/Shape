//! `NonNativeBuiltinCount`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use buffer_writes::slot_info;
use crate::signals::analysis::{buffer_writes, leaf_checks::*, native_checks::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `NonNativeBuiltinCount`. Card: [`CARD`].
pub(crate) fn recognize_non_native_builtin_count(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    if native_count(view) == Some(NativeCount::NonNativeBuiltins) {
        return claim(&["length of a native-code filter over a captured built-in list"]);
    }
    // The captured list may be opaque (a runtime-provided array): then the
    // value is `.length` of `list.filter(f => !/native code/.test(…))`, read
    // from the write's own slice.
    let leaf = single_number(view)?;
    let filtered_length = has_transform(leaf, ".length")
        && !has_transform(leaf, "count")
        && (has_transform(leaf, ".filter()") || leaf.apis.iter().any(|api| crate::signals::analysis::api_matches(api, "filter()")));
    if !filtered_length || native_count(view).is_some() {
        return None;
    }
    let info = slot_info(view, ctx);
    let native_test = info.facts.call("filter")
        && info.facts.callback_calls.contains("test")
        && info.facts.callback_regex.iter().any(|pattern| pattern.contains("native"));
    native_test
        .then(|| claim(&["length of list.filter(!native-code regex test)"]))
        .flatten()
}

pub(crate) fn bind_non_native_builtin_count(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    crate::signals::analysis::bind_in_order(layout, &["nonNativeCount"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `NonNativeBuiltinCount`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "NonNativeBuiltinCount",
    summary: "At startup the script captures a fixed list of 27 built-in functions (TypeError, Object, RegExp, Array, Function.prototype.bind/call/apply, Array.prototype.push/slice/join/map, JSON.stringify, Object.defineProperty, Math.floor, String.prototype.indexOf, Uint8Array and others). It prints each one with Function.prototype.toString and counts how many do NOT look like '{ [native code] }'.",
    why: "A non-zero count means some core built-ins were already replaced by JavaScript (polyfills, monkey-patches, automation or instrumentation shims), which likely flags a tampered or instrumented environment.",
    sample_payload: Some(r#"{
  "nonNativeCount": 0
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "nonNativeCount",
            "How many of the 27 listed built-ins have source text that does not look native, 0 to 27. 0 on an unmodified browser.",
        ),
    ]),
};
