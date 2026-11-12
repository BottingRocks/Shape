//! `SeleniumIdeArtifactFlags`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_in_order, BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, property_census::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `SeleniumIdeArtifactFlags`. Card: [`CARD`].
pub(crate) fn recognize_selenium_ide_artifact_flags(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let census = census(view, ctx);
    let artifacts = ["sideexplayingflag", "prompt\\.js", "from-page-runscript"];
    (one_byte(view) && artifacts.iter().filter(|needle| census.string_containing(needle)).count() >= 2)
        .then(|| Claim::new(["SideeX / Selenium IDE page artifacts", "one flag byte"]))
}

pub(crate) fn bind_selenium_ide_artifact_flags(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_in_order(layout, &["automationFlags"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `SeleniumIdeArtifactFlags`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "SeleniumIdeArtifactFlags",
    summary: "Looks for traces of the SideeX / Selenium IDE browser-automation extension: it watches document.body for a 'sideexplayingflag' attribute change, and checks for an extension-served prompt.js script or a global 'error' value matching 'from-page-runscript'. Both flags are sticky once set, and the byte is resent when a flag changes.",
    why: "Directly flags test-automation tooling (Selenium IDE / SideeX), a common way to drive scripted browsers.",
    sample_payload: Some(r#"{
  "promptScript": false,
  "sideexPlaying": false
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::byte(
            "automationFlags",
            "Two flag bits (0..3): 'SideeX playing' (the sideexplayingflag attribute was added, changed or removed on body) and 'prompt script / runscript' (a chrome-extension:// or moz-extension:// prompt.js script is on the page, or the global error matches from-page-runscript). 0 means neither was seen.",
        )
        .flags(&["sideexPlaying", "promptScript"]),
    ]),
};
