//! `MouseTravelAndClickCount`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use value_facts::slot_facts;
use crate::signals::analysis::{value_facts, leaf_checks::*, described_fields::*};
use crate::signals::catalog::card::{Card, CardBody, CardField, CardMessage};


/// `MouseTravelAndClickCount`. Card: [`CARD`].
pub(crate) fn recognize_mouse_travel_and_click_count(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    if !all_numbers(view, 2) {
        return None;
    }
    let facts = slot_facts(view, ctx);
    let travel = facts
        .descs()
        .any(|desc| (desc.has_call("sqrt") || desc.has_call("pow")) && (desc.has_member("clientX") || desc.has_member("clientY") || desc.has_member("pageX") || desc.has_member("pageY") || desc.has_member("screenX")));
    (travel && facts.has_event(&["mousemove"]) && facts.has_event(&["click"]))
        .then(|| Claim::new(["mousemove distance and click listeners", "RAW4 NUMBER NUMBER"]))
}

pub(crate) fn bind_mouse_travel_and_click_count(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let mut binder = Binder::new(view, layout, ctx)?;
    binder.top(|field| {
        let desc = field.merged();
        if desc.has_call("sqrt") || desc.has_call("pow") || desc.has_call("toFixed") {
            Some("mouseTravelDistance")
        } else if desc.counter {
            Some("backgroundClickCount")
        } else {
            None
        }
    })?;
    binder.finish()
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `MouseTravelAndClickCount`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "MouseTravelAndClickCount",
    summary: "Watches the mouse for the whole time the page is open. It counts clicks that land outside interactive elements (not on or inside an input, select, option, textarea, button or link), and adds up how far the mouse pointer travelled across mousemove events, in CSS pixels. At startup it sends a placeholder of two zeros; at the end of the page's life it sends the real totals.",
    why: "Real users move the mouse a fair distance and click in empty areas now and then; scripted sessions often show no movement, no clicks, or implausible totals. This is likely used as a simple human-versus-bot behaviour check.",
    sample_payload: None,
    by_name: false,
    body: CardBody::Messages(&[
        CardMessage {
            when: "At startup (placeholder, before any activity is measured)",
            fields: &[
                CardField::number("backgroundClickCount", "Always 0."),
                CardField::number("mouseTravelDistance", "Always 0."),
            ],
        },
        CardMessage {
            when: "At the end of the page's life (last chance), with the real totals",
            fields: &[
                CardField::number(
                    "backgroundClickCount",
                    "Number of clicks on the page that did not hit or sit inside an input, select, option, textarea, button or link. 0 or more, never reset.",
                ),
                CardField::number(
                    "mouseTravelDistance",
                    "Total mouse travel in CSS pixels, summed straight-line distance between consecutive mousemove positions, rounded to 2 decimals. The first step is measured from (0,0). 0 if the mouse never moved.",
                ),
            ],
        },
    ]),
};
