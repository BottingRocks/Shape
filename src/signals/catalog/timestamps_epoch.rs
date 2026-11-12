//! `TimestampsEpoch`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use value_facts::slot_facts;
use crate::signals::analysis::{value_facts, leaf_checks::*, described_fields::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `TimestampsEpoch`. Card: [`CARD`].
pub(crate) fn recognize_timestamps_epoch(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    if !all_numbers(view, 1) {
        return None;
    }
    let facts = slot_facts(view, ctx);
    let epoch = facts.all_matched() && facts.descs().all(|desc| desc.reads_date() && !desc.has_op("-") && !desc.program_call);
    epoch.then(|| Claim::new(["one Date epoch read", "RAW4 NUMBER"]))
}

pub(crate) fn bind_timestamps_epoch(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let mut binder = Binder::new(view, layout, ctx)?;
    binder.top(|_| Some("epochMs"))?;
    binder.finish()
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `TimestampsEpoch`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "TimestampsEpoch",
    summary: "Reads the current wall-clock time once, as Number(new Date()), when the script starts, and sends it.",
    why: "Lets the server compare the client clock with its own time and with other timing signals; a skewed or faked clock is likely a sign of emulation, replay or tampering.",
    sample_payload: Some(r#"{
  "epochMs": 1794273726990
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "epochMs",
            "Milliseconds since the Unix epoch (about 1.7e12 in 2026). Whatever a patched Date returns if the page overrode it.",
        ),
    ]),
};
