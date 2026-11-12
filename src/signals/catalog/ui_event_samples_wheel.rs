//! `UiEventSamplesWheel`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use value_facts::{slot_facts, Guard};
use crate::signals::analysis::{value_facts, leaf_checks::*, described_fields::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `UiEventSamplesWheel`. Card: [`CARD`].
pub(crate) fn recognize_ui_event_samples_wheel(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    if !all_numbers(view, 2) {
        return None;
    }
    let facts = slot_facts(view, ctx);
    // One throttled counter per message.
    let throttled = facts.emissions.iter().flatten().all(|descs| descs.iter().filter(|desc| desc.guards.contains(&Guard::Throttle)).count() == 1)
        && facts.emissions.iter().flatten().count() > 0;
    let counters = facts.descs().all(|desc| desc.counter);
    (facts.has_event(&["wheel"]) && !facts.has_event(&["mousemove"]) && throttled && counters)
        .then(|| Claim::new(["wheel listener counters", "one throttled counter"]))
}

pub(crate) fn bind_ui_event_samples_wheel(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let mut binder = Binder::new(view, layout, ctx)?;
    binder.top(|field| {
        let desc = field.merged();
        if !desc.counter {
            None
        } else if desc.guards.contains(&Guard::Throttle) {
            Some("wheelSampleCount")
        } else {
            Some("wheelEventCount")
        }
    })?;
    binder.finish()
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `UiEventSamplesWheel`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "UiEventSamplesWheel",
    summary: "Listens for wheel events on the document and keeps two counters: all wheel events, and wheel 'bursts' (an event counted only if more than 500 ms passed since the last counted one). Sends both counters after every wheel event. The event's details (deltas, position, isTrusted) are not read.",
    why: "Real users produce wheel activity in bursts while scrolling; likely used to tell human scrolling from no scrolling or scripted scrolling.",
    sample_payload: None,
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "wheelSampleCount",
            "Number of wheel bursts so far (events more than 500 ms after the previous counted one; the first event always counts). Integer >= 1.",
        ),
        CardField::number(
            "wheelEventCount",
            "Total number of wheel events so far, including this one. Integer >= 1.",
        ),
    ]),
};
