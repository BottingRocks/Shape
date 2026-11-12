//! `OuterArgsGlobalProcess`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafKind, LeafOp, LeafView, SignalLayout, SlotView};
use crate::signals::analysis::{shape_text::*, leaf_checks::*, field_solver::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `OuterArgsGlobalProcess`. Card: [`CARD`].
pub(crate) fn recognize_outer_args_global_process(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let leaves = leaves_of(view);
    let json = leaves.iter().any(|leaf| {
        leaf.op == LeafOp::String
            && (has_transform(leaf, "context.outerArgs") || has_transform(leaf, ".outerArgs"))
            && has_transform(leaf, "JSON.stringify")
    });
    if json && top_count_is(&view.shapes(), 2) {
        return claim(&["string is JSON.stringify(context.outerArgs)", "two fields"]);
    }
    // A string computed inside a `try` reaches the contract opaque; its
    // write-site slice still stringifies the outer arguments.
    if top_count_is(&view.shapes(), 2) && leaves.iter().any(|leaf| leaf.op == LeafOp::String && outer_args_json(ctx, leaf)) {
        return claim(&["string's write-site slice is JSON.stringify(outerArgs)", "two fields"]);
    }
    None
}

/// The leaf's write-site slice calls `JSON.stringify` on `.outerArgs`.
fn outer_args_json(ctx: &FinderCtx, leaf: &LeafView) -> bool {
    ctx.value_slice(leaf, 6)
        .is_some_and(|slice| slice.calls_method("stringify") && slice.members.contains_key("outerArgs"))
}

pub(crate) fn bind_outer_args_global_process(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let json = |f: &Field| {
        f.writes(LeafKind::String)
            && (f.tr("JSON.stringify")
                || f.leaves.iter().any(|leaf| leaf.op == LeafOp::String && outer_args_json(ctx, leaf)))
    };
    let flags = |f: &Field| f.is_plain(LeafKind::Byte);
    solve_all_messages(view, layout, &[("outerArgsJson", &json), ("flags", &flags)])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `OuterArgsGlobalProcess`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "OuterArgsGlobalProcess",
    summary: "Serialises the arguments/config passed to the outer loader (outerArgs) with JSON.stringify, keeps the first 256 characters, and adds a flags byte recording whether outerArgs was defined, whether stringify threw, and whether the global object looks like Node.js (global.global === global, global.process exists).",
    why: "global.global and process are Node.js markers, so this likely detects scripts run under Node, jsdom, Electron or test harnesses instead of a real browser; the outerArgs JSON likely checks how the loader was invoked.",
    sample_payload: Some(r#"{
  "flags": {
    "globalIsSelf": false,
    "outerArgsDefined": false,
    "processDefined": false,
    "stringifyThrew": false
  },
  "outerArgsJson": null
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::string(
            "outerArgsJson",
            "First 256 characters of JSON.stringify(outerArgs) (\"null\" if outerArgs is null), or a null marker if outerArgs is undefined, cannot be serialised, or stringify threw.",
        ).nullable(),
        CardField::byte(
            "flags",
            "Packed flags (positions vary per build): outerArgs is defined; JSON.stringify threw; global.global === global; global.process is defined. Other bits 0.",
        )
        .flags(&["outerArgsDefined", "stringifyThrew", "globalIsSelf", "processDefined"]),
    ]),
};
