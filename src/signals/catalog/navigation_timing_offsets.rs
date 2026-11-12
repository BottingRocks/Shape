//! `NavigationTimingOffsets`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use std::collections::BTreeSet;
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafOp, SignalLayout, SlotView};
use value_facts::{slot_facts, Desc};
use crate::signals::analysis::{value_facts, described_fields::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


// ===========================================================================
// Shared recognizer helpers
// ===========================================================================

/// Top-level leaf ops of every emission.
fn emission_tops(view: &SlotView) -> Vec<Vec<LeafOp>> {
    view.emissions()
        .iter()
        .map(|emission| {
            emission
                .leaves
                .iter()
                .filter(|leaf| leaf.repeat_depth == 0 && leaf.choice_depth == 0)
                .map(|leaf| leaf.op)
                .collect()
        })
        .collect()
}

const NAVIGATION_TIMING: &[&str] = &[
    "navigationStart",
    "unloadEventStart",
    "unloadEventEnd",
    "redirectStart",
    "redirectEnd",
    "fetchStart",
    "domainLookupStart",
    "domainLookupEnd",
    "connectStart",
    "connectEnd",
    "secureConnectionStart",
    "requestStart",
    "responseStart",
    "responseEnd",
    "domLoading",
    "domInteractive",
    "domContentLoadedEventStart",
    "domContentLoadedEventEnd",
    "domComplete",
    "loadEventStart",
    "loadEventEnd",
];

/// The navigation milestone a value reads (an offset reads it and
/// `navigationStart`).
fn milestone(desc: &Desc) -> Option<&'static str> {
    let read: Vec<&'static str> = NAVIGATION_TIMING.iter().copied().filter(|name| desc.has_member(name)).collect();
    match read.as_slice() {
        ["navigationStart"] => Some("navigationStart"),
        [one, "navigationStart"] | ["navigationStart", one] => Some(one),
        [one] if *one != "navigationStart" => Some(one),
        _ => {
            let others: Vec<&&str> = read.iter().filter(|name| **name != "navigationStart").collect();
            match others.as_slice() {
                [one] => Some(one),
                _ => None,
            }
        }
    }
}

/// `NavigationTimingOffsets`. Card: [`CARD`].
pub(crate) fn recognize_navigation_timing_offsets(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let tops = emission_tops(view);
    if tops.is_empty() || tops.iter().any(|ops| ops.len() < 15) || view.leaves().any(|leaf| leaf.op != LeafOp::Number) {
        return None;
    }
    let facts = slot_facts(view, ctx);
    let named: BTreeSet<&str> = facts.descs().filter_map(milestone).collect();
    (named.len() >= 15 && named.contains("navigationStart"))
        .then(|| Claim::new(["performance.timing milestones", "navigationStart offsets"]))
}

pub(crate) fn bind_navigation_timing_offsets(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let mut binder = Binder::new(view, layout, ctx)?;
    binder.top(|field| milestone(&field.merged()))?;
    binder.finish()
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `NavigationTimingOffsets`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "NavigationTimingOffsets",
    summary: "Reads the page-load timeline from performance.timing at the end of the page's life. It sends 20 milestones (DNS lookup, connect, request, response, DOM loading, load event and so on) as milliseconds after navigationStart, plus navigationStart itself as an absolute time. If performance.timing does not exist, nothing is sent.",
    why: "A real page load has a plausible, consistent timeline; headless or scripted loads, replayed pages and spoofed timing objects likely show missing, zero or impossible values. The absolute navigationStart also gives the page-load time on the client clock.",
    sample_payload: None,
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "domLoading",
            "Milliseconds from navigationStart to domLoading; 0 if not reached or missing.",
        ),
        CardField::number(
            "connectStart",
            "Milliseconds from navigationStart to connectStart; 0 if not reached or missing.",
        ),
        CardField::number(
            "domainLookupEnd",
            "Milliseconds from navigationStart to domainLookupEnd; 0 if not reached or missing.",
        ),
        CardField::number(
            "unloadEventEnd",
            "Milliseconds from navigationStart to the previous page's unloadEventEnd; 0 if there was none.",
        ),
        CardField::number(
            "unloadEventStart",
            "Milliseconds from navigationStart to the previous page's unloadEventStart; 0 if there was none.",
        ),
        CardField::number(
            "navigationStart",
            "navigationStart itself as an absolute time, milliseconds since 1970 (around 1.7e12). No zero fallback.",
        ),
        CardField::number(
            "connectEnd",
            "Milliseconds from navigationStart to connectEnd; 0 if not reached or missing.",
        ),
        CardField::number(
            "fetchStart",
            "Milliseconds from navigationStart to fetchStart; 0 if not reached or missing.",
        ),
        CardField::number(
            "loadEventStart",
            "Milliseconds from navigationStart to loadEventStart; 0 if the load event has not started.",
        ),
        CardField::number(
            "loadEventEnd",
            "Milliseconds from navigationStart to loadEventEnd; 0 if the load event has not finished.",
        ),
        CardField::number(
            "domContentLoadedEventEnd",
            "Milliseconds from navigationStart to domContentLoadedEventEnd; 0 if not reached or missing.",
        ),
        CardField::number(
            "redirectEnd",
            "Milliseconds from navigationStart to redirectEnd; 0 if there was no redirect.",
        ),
        CardField::number(
            "responseStart",
            "Milliseconds from navigationStart to responseStart; 0 if not reached or missing.",
        ),
        CardField::number(
            "requestStart",
            "Milliseconds from navigationStart to requestStart; 0 if not reached or missing.",
        ),
        CardField::number(
            "domComplete",
            "Milliseconds from navigationStart to domComplete; 0 if not reached or missing.",
        ),
        CardField::number(
            "domContentLoadedEventStart",
            "Milliseconds from navigationStart to domContentLoadedEventStart; 0 if not reached or missing.",
        ),
        CardField::number(
            "domainLookupStart",
            "Milliseconds from navigationStart to domainLookupStart; 0 if not reached or missing.",
        ),
        CardField::number(
            "domInteractive",
            "Milliseconds from navigationStart to domInteractive; 0 if not reached or missing.",
        ),
        CardField::number(
            "redirectStart",
            "Milliseconds from navigationStart to redirectStart; 0 if there was no redirect.",
        ),
        CardField::number(
            "secureConnectionStart",
            "Milliseconds from navigationStart to secureConnectionStart; 0 on plain HTTP or a reused connection.",
        ),
        CardField::number(
            "responseEnd",
            "Milliseconds from navigationStart to responseEnd; 0 if not reached or missing.",
        ),
    ]),
};
