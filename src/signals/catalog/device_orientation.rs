//! `DeviceOrientation`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use value_facts::slot_facts;
use crate::signals::analysis::{value_facts, leaf_checks::*, described_fields::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `DeviceOrientation`. Card: [`CARD`].
pub(crate) fn recognize_device_orientation(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    if !all_numbers(view, 9) {
        return None;
    }
    let facts = slot_facts(view, ctx);
    (facts.has_event(&["deviceorientation"]) && facts.any_member("alpha") && facts.any_member("beta") && facts.any_member("gamma"))
        .then(|| Claim::new(["deviceorientation listener state", "alpha/beta/gamma running statistics"]))
}

pub(crate) fn bind_device_orientation(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let mut binder = Binder::new(view, layout, ctx)?;
    binder.top(|field| {
        let desc = field.merged();
        let angles: Vec<&str> = ["alpha", "beta", "gamma"]
            .into_iter()
            .filter(|angle| desc.has_member(angle))
            .collect();
        match angles.as_slice() {
            [angle] => {
                let squared = desc.squared.contains(*angle);
                Some(match (*angle, squared) {
                    ("alpha", false) => "meanAlpha",
                    ("alpha", true) => "meanAlphaSquared",
                    ("beta", false) => "meanBeta",
                    ("beta", true) => "meanBetaSquared",
                    ("gamma", false) => "meanGamma",
                    _ => "meanGammaSquared",
                })
            }
            [] if desc.counter && !desc.has_op("*") && !desc.has_op("/") => Some("eventCount"),
            [] if !desc.squared.is_empty() => Some("meanSquaredIntervalMs"),
            [] if desc.has_op("-") => Some("meanIntervalMs"),
            _ => None,
        }
    })?;
    binder.finish()
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `DeviceOrientation`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "DeviceOrientation",
    summary: "Listens to deviceorientation events for the whole page visit and keeps running averages of the alpha, beta and gamma angles, of their squares, and of the time between events. At the last-chance phase it sends these nine statistics once, if at least one event arrived.",
    why: "Real phones and tablets produce a stream of small, noisy orientation readings; emulators, desktops and bots produce none, constant or impossible values, so the averages and variances (mean of squares minus squared mean) help tell real handheld devices from fakes (likely).",
    sample_payload: Some(r#"{
  "eventCount": 2,
  "meanAlpha": 0,
  "meanAlphaSquared": 0,
  "meanBeta": 0,
  "meanBetaSquared": 0,
  "meanGamma": 0,
  "meanGammaSquared": 0,
  "meanIntervalMs": 47880,
  "meanSquaredIntervalMs": 2292494400
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "meanBeta",
            "Average beta angle (front-back tilt, degrees, about -180..180) over all events; float 0 if the browser reported null angles.",
        ),
        CardField::number(
            "meanSquaredIntervalMs",
            "Average of the squared time gap between consecutive events, in ms squared; 0 with fewer than two events.",
        ),
        CardField::number("meanGamma", "Average gamma angle (left-right tilt, degrees, about -90..90)."),
        CardField::number(
            "meanBetaSquared",
            "Average of beta squared; with meanBeta it gives the beta variance.",
        ),
        CardField::number(
            "meanAlphaSquared",
            "Average of alpha squared; with meanAlpha it gives the alpha variance.",
        ),
        CardField::number(
            "meanGammaSquared",
            "Average of gamma squared; with meanGamma it gives the gamma variance.",
        ),
        CardField::number(
            "eventCount",
            "Number of deviceorientation events handled (always at least 1 when sent).",
        ),
        CardField::number("meanAlpha", "Average alpha angle (compass heading, degrees, 0..360)."),
        CardField::number(
            "meanIntervalMs",
            "Average time between consecutive events, in whole-millisecond clock units; 0 with fewer than two events.",
        ),
    ]),
};
