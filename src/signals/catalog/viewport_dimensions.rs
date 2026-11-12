//! `ViewportDimensions`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_by_object_key, BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::leaf_checks::*;
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// The viewport census's object keys (part of its wire format).
const VIEWPORT_KEYS: &[&str] = &[
    "innerWidth",
    "innerHeight",
    "outerHeight",
    "outerWidth",
    "screenWidth",
    "screenHeight",
    "screenAvailWidth",
    "screenAvailHeight",
    "visualViewportWidth",
    "visualViewportHeight",
    "visualViewportScale",
];

/// `ViewportDimensions`. Card: [`CARD`].
pub(crate) fn recognize_viewport_dimensions(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let leaves = leaves_of(view);
    let reads = any_api(&leaves, "visualViewport.scale") && any_api(&leaves, "screen.availHeight");
    if view.is_generic_object() && reads {
        return claim(&["named object", "values read screen.* and visualViewport.*"]);
    }
    None
}

pub(crate) fn bind_viewport_dimensions(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let names: Vec<(&str, &str)> = VIEWPORT_KEYS.iter().map(|key| (*key, *key)).collect();
    bind_by_object_key(layout, &names)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `ViewportDimensions`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "ViewportDimensions",
    summary: "At the last moment before the payload is built, reads eleven window, screen and visual-viewport sizes and hands them as one plain object to the shared encoder. If any read throws (e.g. no visualViewport), nothing is sent and an error is reported instead.",
    why: "Comparing these sizes exposes window chrome (outer minus inner), OS taskbars (screen minus avail) and zoom state, and impossible combinations likely reveal headless browsers or device emulation.",
    sample_payload: None,
    by_name: true,
    body: CardBody::Fields(&[
        CardField::number("innerWidth", "window.innerWidth: layout viewport width in CSS pixels."),
        CardField::number("innerHeight", "window.innerHeight: layout viewport height in CSS pixels."),
        CardField::number(
            "outerHeight",
            "window.outerHeight: full browser window height in CSS pixels, including toolbars.",
        ),
        CardField::number("outerWidth", "window.outerWidth: full browser window width in CSS pixels."),
        CardField::number("screenWidth", "screen.width: screen width in CSS pixels."),
        CardField::number("screenHeight", "screen.height: screen height in CSS pixels."),
        CardField::number(
            "screenAvailWidth",
            "screen.availWidth: screen width minus OS bars such as a side dock.",
        ),
        CardField::number(
            "screenAvailHeight",
            "screen.availHeight: screen height minus the OS taskbar/dock/menu bar.",
        ),
        CardField::number(
            "visualViewportWidth",
            "visualViewport.width: visible area width after pinch-zoom (may be fractional).",
        ),
        CardField::number(
            "visualViewportHeight",
            "visualViewport.height: visible area height after pinch-zoom (may be fractional).",
        ),
        CardField::number(
            "visualViewportScale",
            "visualViewport.scale: pinch-zoom factor, 1 when not zoomed.",
        ),
    ]),
};
