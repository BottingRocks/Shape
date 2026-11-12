//! `ConstantEmptyString`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_in_order, BindRefusal, Claim, FieldBinding, FinderCtx, LeafOp, LeafView, SignalLayout, SlotView};
use crate::signals::analysis::{shape_text::*, leaf_checks::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `ConstantEmptyString` evidence: one STRING written from a literal only.
pub(crate) fn is_constant_string(shapes: &[&str], leaves: &[&LeafView]) -> bool {
    top_count_is(shapes, 1)
        && !leaves.is_empty()
        && leaves.iter().all(|leaf| {
            leaf.op == LeafOp::String
                && literal_only(leaf)
                && leaf.transforms.is_empty()
                && leaf.apis.is_empty()
        })
}

/// `ConstantEmptyString`. Card: [`CARD`].
pub(crate) fn recognize_constant_empty_string(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    if is_constant_string(&view.shapes(), &leaves_of(view)) {
        return claim(&["one STRING", "literal only (no host source, no transform)"]);
    }
    None
}

pub(crate) fn bind_constant_empty_string(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_in_order(layout, &["constant"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `ConstantEmptyString`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "ConstantEmptyString",
    summary: "Sends a single fixed empty string and nothing else. It reads no browser data. Before sending, it makes 8 to 14 random throwaway calls to built-in functions (such as isFinite, JSON.parse, new Audio, new MessageChannel) whose results are discarded.",
    why: "It carries no information about the browser; it is likely a decoy or padding message, or a check that the client's encoder and mask stream work (a guess).",
    sample_payload: None,
    by_name: false,
    body: CardBody::Fields(&[
        CardField::string("constant", "Always the empty string \"\"."),
    ]),
};
