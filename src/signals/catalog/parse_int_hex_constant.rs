//! `ParseIntHexConstant`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::extract::contracts::SourceClass;
use crate::signals::analysis::leaf_checks::*;
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `ParseIntHexConstant`. Card: [`CARD`].
pub(crate) fn recognize_parse_int_hex_constant(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let leaf = single_number(view)?;
    (leaf.transforms == ["parseInt"] && leaf.classes == [SourceClass::Literal] && leaf.apis.is_empty())
        .then(|| claim(&["parseInt of literals only"]))
        .flatten()
}

pub(crate) fn bind_parse_int_hex_constant(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    crate::signals::analysis::bind_in_order(layout, &["parsedValue"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `ParseIntHexConstant`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "ParseIntHexConstant",
    summary: "Sends the result of parseInt(\"0xFFFFFFFFFFFFFBFF\", 16). It reads no browser state; on any standard engine the result is the same fixed number.",
    why: "Likely either a fixed sentinel/version marker or a canary that reveals a hooked or replaced parseInt or a non-conforming number parser.",
    sample_payload: Some(r#"{
  "parsedValue": 18446744073709549568
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "parsedValue",
            "The parseInt result as a double; normally 18446744073709549568 (2^64 - 2048). Any other value means parseInt was tampered with.",
        ),
    ]),
};
