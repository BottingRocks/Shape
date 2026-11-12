//! `ShuffledDateString`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafKind, LeafOp, LeafView, SignalLayout, SlotView};
use crate::signals::analysis::{shape_text::*, leaf_checks::*, field_solver::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `ShuffledDateString` evidence: NUMBER, NUMBER and the shuffled rendering
/// (per emission), one of the numbers a bare literal (the constant zero).
pub(crate) fn is_shuffled_date(shapes: &[&str], leaves: &[&LeafView]) -> bool {
    let strings = leaves.iter().filter(|leaf| leaf.op == LeafOp::String).count();
    let numbers = leaves.iter().filter(|leaf| leaf.op == LeafOp::Number).count();
    top_count_is(shapes, 3)
        && strings > 0
        && numbers == 2 * strings
        && strings + numbers == leaves.len()
        && leaves
            .iter()
            .filter(|leaf| leaf.op == LeafOp::String)
            .all(|leaf| date_to_string(leaf) && shuffled(leaf))
        && leaves
            .iter()
            .any(|leaf| leaf.op == LeafOp::Number && literal_only(leaf) && leaf.transforms.is_empty())
}

/// `ShuffledDateString`. Card: [`CARD`].
pub(crate) fn recognize_shuffled_date_string(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    if is_shuffled_date(&view.shapes(), &leaves_of(view)) {
        return claim(&["NUMBER NUMBER STRING", "Array.from(new Date(K).toString()) shuffled", "literal zero"]);
    }
    None
}

pub(crate) fn bind_shuffled_date_string(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let zero = |f: &Field| {
        f.is_plain(LeafKind::Number) && f.literal_only() && f.leaves.iter().all(|leaf| leaf.transforms.is_empty())
    };
    let seed = |f: &Field| f.is_plain(LeafKind::Number) && !f.literal_only();
    let string = |f: &Field| f.is_plain(LeafKind::String) && f.tr(".toString()");
    solve_all_messages(
        view,
        layout,
        &[("zero", &zero), ("shuffleSeed", &seed), ("shuffledDateString", &string)],
    )
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `ShuffledDateString`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "ShuffledDateString",
    summary: "Renders one fixed historical instant (1945-08-05T23:16:00Z) with Date.prototype.toString in the browser's local time zone, shuffles the characters of that string with a seeded random generator, and sends the shuffled string together with the generator's seed so the receiver can undo the shuffle. A constant 0 is sent alongside.",
    why: "The local rendering of a 1945 date depends on the host's historical time-zone rules and zone naming, so it checks that the claimed time zone and tz database are genuine (likely harder to fake than today's offset); the shuffle just hides the plain string.",
    sample_payload: Some(r#"{
  "shuffleSeed": 0.04153433085533542,
  "shuffledDateString": " ul1Th041-0M6 GTi765ae09g:iP aAc (ni  0f1m ci0tD yu:0)5gS",
  "zero": 0
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number("zero", "Always 0 (likely a format or version marker)."),
        CardField::string(
            "shuffledDateString",
            "The characters of new Date(-770172240000).toString() in shuffled order, e.g. a scrambled 'Sun Aug 05 1945 ... GMT+0100 (zone name)'.",
        ),
        CardField::number(
            "shuffleSeed",
            "Seed of the random generator used for the shuffle (a fractional number); lets the receiver replay and reverse the shuffle.",
        ),
    ]),
};
