//! `WorkerRealmProfile`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafKind, SignalLayout, SlotView};
use crate::signals::analysis::{shape_text::*, leaf_checks::*, field_solver::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `WorkerRealmProfile`. Card: [`CARD`].
pub(crate) fn recognize_worker_realm_profile(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let leaves = leaves_of(view);
    let reads = [
        "location.origin",
        "navigator.userAgent",
        "navigator.platform",
        "navigator.hardwareConcurrency",
    ]
    .iter()
    .all(|api| any_api(&leaves, api));
    if reads && top_count_is(&view.shapes(), 5) {
        return claim(&["five fields", "location.origin, navigator.userAgent/platform/hardwareConcurrency"]);
    }
    None
}

pub(crate) fn bind_worker_realm_profile(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let concurrency = |f: &Field| {
        !f.is_plain(LeafKind::Number) && f.writes(LeafKind::Number) && f.api("navigator.hardwareConcurrency")
    };
    let count = |f: &Field| f.is_plain(LeafKind::Number) && !f.api("navigator.hardwareConcurrency");
    let origin = |f: &Field| f.is_plain(LeafKind::String) && f.api("location.origin");
    let user_agent = |f: &Field| f.is_plain(LeafKind::String) && f.api("navigator.userAgent");
    let platform = |f: &Field| f.is_plain(LeafKind::String) && f.api("navigator.platform");
    solve_all_messages(
        view,
        layout,
        &[
            ("hardwareConcurrency", &concurrency),
            ("globalPropertyCount", &count),
            ("origin", &origin),
            ("userAgent", &user_agent),
            ("platform", &platform),
        ],
    )
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `WorkerRealmProfile`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "WorkerRealmProfile",
    summary: "Runs inside a Web Worker and reads five values from the worker's own global scope: the CPU core count, the origin, a count of global property names, the user agent and the platform. The worker builds the record and posts it back to the page.",
    why: "Comparing worker values with the same values read on the page likely catches browsers that spoof navigator properties only on the main page, and the property count fingerprints the worker realm and detects patched or emulated workers.",
    sample_payload: Some(r#"{
  "globalPropertyCount": 401,
  "hardwareConcurrency": 18,
  "origin": "https://www.target.com",
  "platform": "MacIntel",
  "userAgent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/156.0.0.0 Safari/537.36"
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "hardwareConcurrency",
            "navigator.hardwareConcurrency in the worker: logical CPU cores, e.g. 8, or a null marker if null/undefined.",
        ).nullable(),
        CardField::string("origin", "location.origin of the worker, e.g. \"https://www.example.com\"."),
        CardField::number(
            "globalPropertyCount",
            "Total number of own property names on the worker global (self) and every object on its prototype chain.",
        ),
        CardField::string("userAgent", "navigator.userAgent as seen inside the worker."),
        CardField::string(
            "platform",
            "navigator.platform as seen inside the worker, e.g. \"Win32\", \"MacIntel\".",
        ),
    ]),
};
