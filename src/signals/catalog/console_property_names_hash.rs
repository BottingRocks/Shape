//! `ConsolePropertyNamesHash`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_in_order, BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, property_census::*, census_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `ConsolePropertyNamesHash`. Card: [`CARD`].
pub(crate) fn recognize_console_property_names_hash(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let census = census(view, ctx);
    let sorted_names = view.leaves().all(|leaf| {
        leaf.transforms.iter().any(|t| t == ".sort()") && leaf.transforms.iter().any(|t| t == ".join()")
    });
    (one_number(view) && sorted_names && probes(view, &census, "console") && census.list_native.is_empty())
        .then(|| Claim::new(["console property names", "one sorted-names hash"]))
}

pub(crate) fn bind_console_property_names_hash(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_in_order(layout, &["consoleNamesHash"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `ConsolePropertyNamesHash`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "ConsolePropertyNamesHash",
    summary: "Lists every enumerable property name of the page's console object (own and inherited), sorts them, joins them with NUL characters and sends one 32-bit hash. Property values are never read. If console is missing, the hash is 0.",
    why: "The set of console methods differs between browsers, versions and environments (and changes when scripts or tools add members), so it likely helps identify the real browser and spot modified environments.",
    sample_payload: Some(r#"{
  "consoleNamesHash": -930873437
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "consoleNamesHash",
            "32-bit hash of the sorted, NUL-joined enumerable property names of console; 0 if the list is empty.",
        ),
    ]),
};
