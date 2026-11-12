//! `MathOwnPropertyNames`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_in_order, top_level_ops, BindRefusal, Claim, FieldBinding, FinderCtx, LeafOp, SignalLayout, SlotView};
use crate::signals::analysis::LayoutNode;
use crate::signals::analysis::{property_census::*, census_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `MathOwnPropertyNames`. Card: [`CARD`].
pub(crate) fn recognize_math_own_property_names(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let census = census(view, ctx);
    (probes(view, &census, "Math")
        && census.calls("getOwnPropertyNames")
        && top_level_ops(view) == [LeafOp::Number, LeafOp::Number]
        && view.leaves().any(|leaf| leaf.op == LeafOp::String && leaf.repeat_depth == 1))
        .then(|| Claim::new(["Object.getOwnPropertyNames(Math)", "hash, count, repeated names"]))
}

pub(crate) fn bind_math_own_property_names(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let leaves = first_emission_top_leaves(view);
    let mut names = Vec::new();
    let mut next_leaf = 0;
    for field in layout.messages.first().map(|m| m.fields.as_slice()).unwrap_or_default() {
        names.push(match &field.node {
            LayoutNode::Array { item } if matches!(**item, LayoutNode::Leaf(_)) => "propertyName",
            LayoutNode::Leaf(_) => {
                let leaf = leaves.get(next_leaf).ok_or_else(|| BindRefusal::new("leaf/field mismatch"))?;
                next_leaf += 1;
                if leaf.transforms.iter().any(|t| t == ".join()") {
                    "namesHash"
                } else if leaf.transforms.first().map(String::as_str) == Some(".length") {
                    "nameCount"
                } else {
                    return Err(BindRefusal::new("number field is neither the names hash nor the count"));
                }
            }
            _ => return Err(BindRefusal::new("unexpected Math field shape")),
        });
    }
    bind_in_order(layout, &names)
}

// ===========================================================================
// Binder helpers
// ===========================================================================

fn first_emission_top_leaves(view: &SlotView) -> Vec<&crate::signals::analysis::LeafView> {
    view.emissions()
        .first()
        .map(|emission| {
            emission
                .leaves
                .iter()
                .filter(|leaf| leaf.repeat_depth == 0 && leaf.choice_depth == 0)
                .collect()
        })
        .unwrap_or_default()
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `MathOwnPropertyNames`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "MathOwnPropertyNames",
    summary: "Lists every own property name of the page's built-in Math object (Object.getOwnPropertyNames(Math), e.g. \"abs\", \"PI\", \"sumPrecise\") and sends them three ways: a hash of the whole list, the number of names, and every name in full. It never reads Math values or calls Math functions. It runs once when the collector starts.",
    why: "The set and order of Math members differ between browser engines and versions, so this likely helps confirm the claimed browser and spot patched or polyfilled environments (extra or missing members).",
    sample_payload: Some(r#"{
  "namesHash": -297135577,
  "propertyName": [
    "abs",
    "acos",
    "acosh",
    "asin",
    "asinh",
    "atan",
    "atanh",
    "atan2",
    "ceil",
    "cbrt",
    "expm1",
    "clz32",
    "cos",
    "cosh",
    "exp",
    "floor",
    "f16round",
    "fround",
    "hypot",
    "imul",
    "log",
    "log1p",
    "log2",
    "log10",
    "max",
    "min",
    "pow",
    "random",
    "round",
    "sign",
    "sin",
    "sinh",
    "sqrt",
    "sumPrecise",
    "tan",
    "tanh",
    "trunc",
    "E",
    "LN10",
    "LN2",
    "LOG10E",
    "LOG2E",
    "PI",
    "SQRT1_2",
    "SQRT2"
  ]
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "namesHash",
            "Signed 32-bit hash of all the names joined with a NUL character, in engine order.",
        ),
        CardField::number(
            "nameCount",
            "How many own property names Math has (typically around 40-45 in current browsers).",
        ),
        CardField::string("propertyName", "One Math property name, e.g. \"abs\" or \"E\".")
            .repeat("once per name, nameCount times, in engine order (some builds send them in reverse)"),
    ]),
};
