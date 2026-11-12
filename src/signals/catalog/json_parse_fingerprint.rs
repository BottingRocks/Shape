//! `JsonParseFingerprint`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::extract::contracts::SourceClass;
use crate::signals::analysis::{bind_in_order, BindRefusal, Claim, FieldBinding, FinderCtx, LeafOp, SignalLayout, SlotView};
use crate::signals::analysis::{shape_text::*, leaf_checks::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `JsonParseFingerprint`. Card: [`CARD`].
pub(crate) fn recognize_json_parse_fingerprint(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let leaves = leaves_of(view);
    let digest = !leaves.is_empty()
        && leaves.iter().all(|leaf| {
            leaf.op == LeafOp::Number
                && has_transform(leaf, ".map()")
                && has_transform(leaf, ".join()")
                && leaf.apis.is_empty()
                && !has_class(leaf, SourceClass::HostObservation)
        });
    let json = view.reads_global("JSON") && !view.call_string_args("parse").is_empty();
    if top_count_is(&view.shapes(), 1) && digest && json {
        return claim(&["one NUMBER", "hash of a mapped table joined", "producer calls JSON.parse"]);
    }
    None
}

pub(crate) fn bind_json_parse_fingerprint(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_in_order(layout, &["jsonOutcomeHash"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `JsonParseFingerprint`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "JsonParseFingerprint",
    summary: "Runs JSON.parse then JSON.stringify on four fixed test strings: one valid object whose key contains a NUL character, and three malformed inputs (a missing comma, and inputs starting with odd Unicode characters). For each it keeps either the re-serialized JSON or \"INVALID:\" plus the error text, joins the four results and sends a hash of them.",
    why: "SyntaxError messages are worded differently in each JavaScript engine and version, so the hash identifies the real engine. It also catches patched or polyfilled JSON and Error functions (likely).",
    sample_payload: None,
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "jsonOutcomeHash",
            "32-bit signed hash of the four joined parse/stringify outcomes. It is mostly determined by the engine's JSON error messages.",
        ),
    ]),
};
