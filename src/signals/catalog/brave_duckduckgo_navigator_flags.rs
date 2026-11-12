//! `BraveDuckDuckGoNavigatorFlags`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_in_order, BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, property_census::*, census_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `BraveDuckDuckGoNavigatorFlags`. Card: [`CARD`].
pub(crate) fn recognize_brave_duckduckgo_navigator_flags(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let census = census(view, ctx);
    (one_byte(view) && probes_all(view, &census, &["brave", "duckduckgo"]) && !outranked(&census, "BraveDuckDuckGoNavigatorFlags"))
        .then(|| Claim::new(["navigator.brave / navigator.duckduckgo", "one flag byte"]))
}

pub(crate) fn bind_brave_duckduckgo_navigator_flags(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_in_order(layout, &["browserFlags"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `BraveDuckDuckGoNavigatorFlags`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "BraveDuckDuckGoNavigatorFlags",
    summary: "At init, reads navigator.brave and navigator.duckduckgo, converts each value to an integer, and packs the results into one byte of two flags. The conversion is not a real presence test: undefined, null, false and objects (including the real Brave/DuckDuckGo objects) all give 0. Only primitive values such as true or 1 give 1.",
    why: "Likely meant to detect privacy-focused browsers (Brave, DuckDuckGo) that block or randomise fingerprinting, so their other signals can be judged differently. As written, it mostly catches pages, extensions or spoofing tools that set these properties to true or 1.",
    sample_payload: None,
    by_name: false,
    body: CardBody::Fields(&[
        CardField::byte(
            "browserFlags",
            "Two packed flags: navigator.brave converted to an integer, and navigator.duckduckgo converted the same way (each 1 only if the value is true or an odd number, else 0). Other bits are normally 0.",
        )
        .flags(&["navigator.brave", "navigator.duckduckgo"]),
    ]),
};
