//! `GlobalPropertiesCount`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_in_order, BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, property_census::*, census_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `GlobalPropertiesCount`. Card: [`CARD`].
pub(crate) fn recognize_global_properties_count(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let census = census(view, ctx);
    let counts_names = view.leaves().all(|leaf| {
        leaf.transforms.first().map(String::as_str) == Some(".length")
            && leaf.transforms.iter().any(|t| t == "Object.getOwnPropertyNames")
    });
    // The counted object is the global realm by the value's own provenance;
    // without that, a Math or console probe in the producer means the count
    // may be of that namespace.
    let counts_global = view.leaves().all(|leaf| {
        leaf.apis.iter().any(|api| matches!(api.as_str(), "window" | "self" | "global"))
            && !leaf.apis.iter().any(|api| api.rsplit('.').next().is_some_and(|last| last == "Math" || last == "console"))
    });
    (one_number(view)
        && counts_names
        && census.calls("getOwnPropertyNames")
        && (counts_global || (!probes(view, &census, "Math") && !probes(view, &census, "console"))))
        .then(|| Claim::new(["Object.getOwnPropertyNames(global).length", "one count"]))
}

pub(crate) fn bind_global_properties_count(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_in_order(layout, &["globalPropertyCount"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `GlobalPropertiesCount`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "GlobalPropertiesCount",
    summary: "Counts the own property names of the global object (window), using Object.getOwnPropertyNames(window).length. This includes enumerable and non-enumerable names, but not symbols or properties inherited from Window's prototypes. It runs once at startup and sends that one number.",
    why: "The count depends on browser engine and version, and it rises when the page, extensions or automation tools add globals, so it likely serves as both an environment fingerprint and a tampering/automation hint.",
    sample_payload: Some(r#"{
  "globalPropertyCount": 1261
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "globalPropertyCount",
            "Number of own string-named properties on window; a non-negative integer, usually around a thousand in a real browser.",
        ),
    ]),
};
