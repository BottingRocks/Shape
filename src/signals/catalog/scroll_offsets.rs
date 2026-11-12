//! `ScrollOffsets`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use value_facts::slot_facts;
use crate::signals::analysis::{value_facts, leaf_checks::*, described_fields::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `ScrollOffsets`. Card: [`CARD`].
pub(crate) fn recognize_scroll_offsets(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    if !all_numbers(view, 2) {
        return None;
    }
    let facts = slot_facts(view, ctx);
    let ok = facts.all_matched()
        && facts
            .descs()
            .all(|desc| (desc.has_member("pageYOffset") || desc.has_member("scrollY")) && !desc.counter);
    ok.then(|| Claim::new(["two pageYOffset readings", "RAW4 NUMBER NUMBER"]))
}

pub(crate) fn bind_scroll_offsets(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let mut binder = Binder::new(view, layout, ctx)?;
    binder.top(|field| {
        let desc = field.merged();
        if desc.phases.contains("setup") && desc.phases.len() == 1 {
            Some("initialScrollY")
        } else if !desc.phases.contains("setup") {
            Some("currentScrollY")
        } else {
            None
        }
    })?;
    binder.finish()
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `ScrollOffsets`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "ScrollOffsets",
    summary: "Reads the page's vertical scroll position (window.pageYOffset, rounded to a whole number) twice: once when the script sets up, and again when the report is built. Both readings are sent.",
    why: "Shows whether and how far the page was scrolled between load and report; a real visitor usually scrolls, while many bots never do (likely used as a simple human-activity hint).",
    sample_payload: None,
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "initialScrollY",
            "Vertical scroll offset in CSS pixels at setup time, rounded to an integer. 0 on an unscrolled page; negative during overscroll; NaN if pageYOffset is missing.",
        ),
        CardField::number(
            "currentScrollY",
            "Vertical scroll offset in CSS pixels when the report is built, rounded the same way.",
        ),
    ]),
};
