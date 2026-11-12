//! `UiEventIsTrusted`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_in_order, BindRefusal, Claim, FieldBinding, FinderCtx, LeafOp, SignalLayout, SlotView};
use crate::signals::analysis::{shape_text::*, leaf_checks::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `UiEventIsTrusted`. Card: [`CARD`].
pub(crate) fn recognize_ui_event_is_trusted(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let leaves = leaves_of(view);
    let trusted = leaves.iter().all(|leaf| {
        leaf.op == LeafOp::Byte
            && has_transform(leaf, ".isTrusted")
            && (has_api(leaf, "createEvent()") || has_transform(leaf, ".createEvent()"))
    });
    if top_count_is(&view.shapes(), 1) && !leaves.is_empty() && trusted {
        return claim(&["one BYTE", "document.createEvent(...).isTrusted"]);
    }
    None
}

pub(crate) fn bind_ui_event_is_trusted(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_in_order(layout, &["isTrusted"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `UiEventIsTrusted`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "UiEventIsTrusted",
    summary: "Creates a UIEvent with document.createEvent('UIEvent') and reads its isTrusted property; the event is never dispatched. Sends whether it claims to be trusted.",
    why: "A script-made event should never be trusted, so a 'trusted' answer means isTrusted was patched or spoofed, a typical trick of automation tools that fake user input.",
    sample_payload: Some(r#"{
  "isTrusted": false
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::boolean(
            "isTrusted",
            "One of two tag values: 'not trusted' (normal; also used when UIEvent does not exist) or 'trusted' (isTrusted spoofed). The two tag numbers change per build.",
        ),
    ]),
};
