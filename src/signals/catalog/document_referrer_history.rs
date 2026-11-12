//! `DocumentReferrerHistory`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafKind, LeafOp, LeafView, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, field_solver::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `DocumentReferrerHistory` evidence: history.length and document.referrer.
pub(crate) fn is_document_referrer(leaves: &[&LeafView]) -> bool {
    leaves
        .iter()
        .any(|leaf| leaf.op == LeafOp::Number && has_api(leaf, "history.length"))
        && any_api(leaves, "document.referrer")
        && !any_api(leaves, "location.protocol")
}

/// `DocumentReferrerHistory`. Card: [`CARD`].
pub(crate) fn recognize_document_referrer_history(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    if is_document_referrer(&leaves_of(view)) {
        return claim(&["values read history.length and document.referrer"]);
    }
    None
}

pub(crate) fn bind_document_referrer_history(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    // The referrer text is absent in some builds.
    let length = |f: &Field| f.writes(LeafKind::Number) && f.api("history.length");
    let empty = |f: &Field| {
        f.writes(LeafKind::Byte) && !f.writes(LeafKind::String) && f.api("document.referrer")
    };
    // The cut is the call path itself or a transform of the referrer read.
    let prefix = |f: &Field| {
        f.writes(LeafKind::String)
            && (f.api("document.referrer.substring()") || (f.api("document.referrer") && f.tr(".substring()")))
    };
    solve_all_messages(
        view,
        layout,
        &[("historyLength", &length), ("referrerEmpty", &empty), ("referrerPrefix", &prefix)],
    )
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `DocumentReferrerHistory`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "DocumentReferrerHistory",
    summary: "Reads, once at init, how many entries the tab's session history has, whether document.referrer is empty, and in most vendor builds the first 100 characters of the referrer itself.",
    why: "Bots and scripted sessions often open pages directly in a fresh tab (history length 1, no referrer) or with a referrer that does not fit the navigation path; real users usually arrive with history and a plausible referrer (likely).",
    sample_payload: Some(r#"{
  "historyLength": 2,
  "referrerEmpty": false,
  "referrerPrefix": "https://www.target.com/"
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "historyLength",
            "history.length: number of entries in the tab's session history (1 or more; browsers usually cap it at 50).",
        ),
        CardField::boolean(
            "referrerEmpty",
            "Tag byte: one build-specific value means document.referrer is empty, the other means it is not.",
        ),
        CardField::string(
            "referrerPrefix",
            "First 100 characters of document.referrer. Sent only by builds that include it (most vendors); the main build and all puma, starbucks and tdbank builds leave it out.",
        ),
    ]),
};
