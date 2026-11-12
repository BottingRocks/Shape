//! `StorageAccessApiPresence`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{bind_in_order, BindRefusal, Claim, FieldBinding, FinderCtx, SignalLayout, SlotView};
use crate::signals::analysis::{leaf_checks::*, property_census::*, census_probes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `StorageAccessApiPresence`. Card: [`CARD`].
pub(crate) fn recognize_storage_access_api_presence(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let census = census(view, ctx);
    (one_byte(view)
        && probes_all(view, &census, &["hasStorageAccess", "requestStorageAccess"])
        && !outranked(&census, "StorageAccessApiPresence"))
        .then(|| Claim::new(["Document.prototype hasStorageAccess / requestStorageAccess", "one flag byte"]))
}

pub(crate) fn bind_storage_access_api_presence(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    bind_in_order(layout, &["apiFlags"])
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `StorageAccessApiPresence`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "StorageAccessApiPresence",
    summary: "Checks, without calling anything, whether three API methods exist as own properties of their prototypes: hasStorageAccess and requestStorageAccess on Document.prototype (Storage Access API), and getHostEnvironmentValue on External.prototype (old EdgeHTML-only API). Sends the three answers as bits of one byte.",
    why: "Which of these APIs are present tells the browser engine and version apart, so it catches user-agent spoofing (likely also catches shims added to document rather than the prototype).",
    sample_payload: Some(r#"{
  "getHostEnvironmentValue": false,
  "hasStorageAccess": true,
  "requestStorageAccess": true
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::byte(
            "apiFlags",
            "Three flag bits (0..7): hasStorageAccess present on Document.prototype, requestStorageAccess present on Document.prototype, getHostEnvironmentValue present on External.prototype. A missing XMLDocument or External counts as not present.",
        )
        .flags(&["hasStorageAccess", "requestStorageAccess", "getHostEnvironmentValue"]),
    ]),
};
