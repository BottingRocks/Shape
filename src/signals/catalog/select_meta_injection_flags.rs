//! `SelectMetaInjectionFlags`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::{property_census::*, census_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `SelectMetaInjectionFlags`. Card: [`CARD`].
const SELECT_META_KEYS: [&str; 4] = ["selectMeta", "animationStart", "measurementDiv", "animationStyle"];

pub(crate) fn recognize_select_meta_injection_flags(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let census = census(view, ctx);
    let keyed = view.is_generic_object()
        && view.emissions().iter().all(|emission| {
            emission.leaves.len() == SELECT_META_KEYS.len()
                && emission
                    .leaves
                    .iter()
                    .all(|leaf| leaf.generic_key.as_deref().is_some_and(|key| SELECT_META_KEYS.contains(&key)))
        });
    (keyed && probes(view, &census, "_selectMeta"))
        .then(|| Claim::new(["window._selectMeta trap", "four keyed marker flags"]))
}

pub(crate) fn bind_select_meta_injection_flags(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    // The wire keys are the card names.
    crate::signals::analysis::bind_by_object_key(layout, &SELECT_META_KEYS.map(|key| (key, key)))
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `SelectMetaInjectionFlags`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "SelectMetaInjectionFlags",
    summary: "Watches the page for four fixed traces of one particular injected script or extension that customises <select> dropdowns: a _selectMeta global with a known set of keys, an animationstart listener whose code mentions _selectOpen/animationName/wasOpened, a hidden measurement DIV with a known inline style added under <html> or <body>, and a known STYLE block added there. Each flag starts false and turns true (and stays true) when its trace appears; every new match sends the object with all four flags.",
    why: "Likely detects a specific browser extension or injected tool (the product is not identified), which can mark automation setups or tampered browsers.",
    sample_payload: None,
    by_name: true,
    body: CardBody::Fields(&[
        CardField::byte(
            "selectMeta",
            "Boolean: true once page code assigns a window._selectMeta object whose sorted key list matches a known hash.",
        ),
        CardField::byte(
            "animationStart",
            "Boolean: true once addEventListener('animationstart', fn) is called with a function whose source contains _selectOpen, animationName and wasOpened.",
        ),
        CardField::byte(
            "measurementDiv",
            "Boolean: true once a DIV whose inline style matches known hashes is added directly under <html> or <body>.",
        ),
        CardField::byte(
            "animationStyle",
            "Boolean: true once a STYLE element whose text matches a known hash is added directly under <html> or <body>.",
        ),
    ]),
};
