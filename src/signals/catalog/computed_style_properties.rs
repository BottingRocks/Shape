//! `ComputedStyleProperties`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_in_order, top_level_ops, BindRefusal, Claim, FieldBinding, FinderCtx, LeafOp, SignalLayout, SlotView};
use crate::signals::analysis::property_census::*;
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `ComputedStyleProperties`. Card: [`CARD`].
pub(crate) fn recognize_computed_style_properties(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let census = census(view, ctx);
    let ops = top_level_ops(view);
    (census.calls("getComputedStyle")
        && ops.iter().filter(|op| **op == LeafOp::Number).count() == 2
        && ops.iter().filter(|op| **op == LeafOp::String).count() == 1
        && ops.len() == 3
        // The joined length: a number read through `.join()` (a shared
        // producer's census also reaches a neighbouring `N S N` slot).
        && view
            .leaves()
            .any(|leaf| leaf.op == LeafOp::Number && leaf.transforms.iter().any(|t| t == ".join()")))
        .then(|| Claim::new(["getComputedStyle census", "count, joined length, sampled string"]))
}

pub(crate) fn bind_computed_style_properties(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    // By provenance: the string is the sampled characters, the number whose
    // value went through `.join()` is the joined length, the other number is
    // the property count.
    bind_top_leaves(view, layout, |leaf| match leaf.op {
        LeafOp::String => Some("sampledChars"),
        LeafOp::Number if leaf.transforms.iter().any(|t| t == ".join()") => Some("joinedLength"),
        LeafOp::Number => Some("propertyCount"),
        LeafOp::Byte => None,
    })
}

/// Name a single message of plain top-level leaves by each leaf's provenance.
fn bind_top_leaves(
    view: &SlotView,
    layout: &SignalLayout,
    name: impl Fn(&crate::signals::analysis::LeafView) -> Option<&'static str>,
) -> Result<FieldBinding, BindRefusal> {
    let mut names = Vec::new();
    for emission in view.emissions() {
        let mut these = Vec::new();
        for leaf in &emission.leaves {
            if leaf.repeat_depth != 0 || leaf.choice_depth != 0 {
                return Err(BindRefusal::new("nested leaf in a flat layout"));
            }
            these.push(name(leaf).ok_or_else(|| BindRefusal::new("leaf without a card name"))?);
        }
        if names.is_empty() {
            names = these;
        } else if names != these {
            return Err(BindRefusal::new("emissions disagree on field provenance"));
        }
    }
    bind_in_order(layout, &names)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `ComputedStyleProperties`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "ComputedStyleProperties",
    summary: "Creates a detached <a> element, calls getComputedStyle on it, and lists every CSS property name on the resulting style object (dropping the numbered index entries). It joins the names into one string with no separator and sends the name count, the string length, and a sample of about 16 characters taken at even spacing through the string. It never reads style values.",
    why: "The set and order of supported CSS properties depends on the browser engine, version and flags, so it likely identifies the real engine/version; the character sample makes a faked property list hard to pass without matching the real one exactly.",
    sample_payload: Some(r#"{
  "joinedLength": 10959,
  "propertyCount": 749,
  "sampledChars": "tnoitixaetltktrs"
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "propertyCount",
            "Number of CSS property names found on the computed style (after removing the numbered index entries); 0 if the style object was null.",
        ),
        CardField::number(
            "joinedLength",
            "Length in characters of all property names joined together with no separator.",
        ),
        CardField::string(
            "sampledChars",
            "About 16 characters picked at even intervals from the joined names string (every character if it is shorter than 32 characters).",
        ),
    ]),
};
