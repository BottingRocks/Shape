//! `PasteCountUnfocusedInputFlag`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{shape_is, BindRefusal, Claim, FieldBinding, FinderCtx, LeafKind, LeafOp, SignalLayout, SlotView};
use value_facts::slot_facts;
use crate::signals::analysis::{value_facts, leaf_checks::*, described_fields::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `PasteCountUnfocusedInputFlag`. Card: [`CARD`].
pub(crate) fn recognize_paste_count_unfocused_input_flag(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    // RAW4 NUMBER BYTE in either order; the flag byte may be a choice of
    // two literal bytes (`flag ? a : b`).
    let flag_choice = !view.emissions().is_empty()
        && view.emissions().iter().all(|emission| {
            let top: Vec<LeafOp> = emission.leaves.iter().filter(|leaf| leaf.choice_depth == 0).map(|leaf| leaf.op).collect();
            let chosen: Vec<&crate::signals::analysis::LeafView> = emission.leaves.iter().filter(|leaf| leaf.choice_depth > 0).collect();
            top == [LeafOp::Number]
                && chosen.len() == 2
                && chosen.iter().all(|leaf| leaf.op == LeafOp::Byte && leaf.choice_depth == 1 && leaf.repeat_depth == 0)
        });
    if !(shape_is(view, "RAW4 NUMBER BYTE") || shape_is(view, "RAW4 BYTE NUMBER") || flag_choice) {
        return None;
    }
    let facts = slot_facts(view, ctx);
    // `inputType` is read by the value or by the input handler that
    // counts (`if (e.inputType === "insertFromPaste") ++count`).
    let handler_reads = facts.handler_members.get("input").is_some_and(|members| members.contains("inputType"));
    (facts.has_event(&["input"]) && (facts.any_member("inputType") || leaf_transform(view, ".inputType") || handler_reads))
        .then(|| Claim::new(["input listener counts inputType pastes", "RAW4 NUMBER BYTE"]))
}

pub(crate) fn bind_paste_count_unfocused_input_flag(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let mut binder = Binder::new(view, layout, ctx)?;
    binder.top(|field| match field.kind() {
        Some(LeafKind::Number) => Some("pastedInputCount"),
        Some(LeafKind::Byte) => Some("unfocusedInputSeen"),
        _ => None,
    })?;
    binder.finish()
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `PasteCountUnfocusedInputFlag`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "PasteCountUnfocusedInputFlag",
    summary: "Listens to every input event on <input> elements for the page's lifetime. It counts pastes into inputs and remembers whether any other input event happened on an input that did not have focus. The values are sent at the end of collection.",
    why: "Pasting into fields (e.g. credentials) is a behaviour signal, and text arriving in an input that never had focus is a strong likely sign of scripted or automated form filling.",
    sample_payload: Some(r#"{
  "pastedInputCount": 1,
  "unfocusedInputSeen": false
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number("pastedInputCount", "Number of paste insertions into <input> elements, 0 or more."),
        CardField::boolean(
            "unfocusedInputSeen",
            "One of two fixed tag values: one means a non-paste input event hit an <input> that was not the focused element at least once, the other means it never happened (the tag values vary per build, e.g. 17 = yes, 77 = no).",
        ),
    ]),
};
