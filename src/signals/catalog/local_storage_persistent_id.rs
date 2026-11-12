//! `LocalStoragePersistentId`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafKind, LeafOp, SignalLayout, SlotView};
use crate::signals::extract::contracts::SourceClass;
use value_facts::slot_facts;
use crate::signals::analysis::{value_facts, leaf_checks::*, described_fields::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `LocalStoragePersistentId`. Card: [`CARD`].
pub(crate) fn recognize_local_storage_persistent_id(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    // One NUMBER status and the stored string (or its null tag) per message.
    if !(per_emission_counts(view, 1, 1..=2) || split_null_tag_messages(view)) || view.has_repeat() {
        return None;
    }
    // The string is read back with `getItem` (the persistent-ID entry).
    let facts = slot_facts(view, ctx);
    let reads = facts.descs().any(|desc| desc.has_call("getItem"))
        || leaf_transform(view, ".getItem()") && (leaf_transform(view, "context.persistentId") || leaf_transform(view, ".key"));
    (reads && !facts.has_event(&["storage"]) && !facts.any_member("oldValue"))
        .then(|| Claim::new(["localStorage.getItem(persistent-ID key)", "status number"]))
}

/// The stored-string and null-tag arms sent as separate messages (each
/// branch emits its own buffer): every message is the status number with
/// either the string or one literal tag byte in its place, and some message
/// carries the string.
fn split_null_tag_messages(view: &SlotView) -> bool {
    let count = |emission: &crate::signals::analysis::slot_view::EmissionView, op: LeafOp| {
        emission.leaves.iter().filter(|leaf| leaf.op == op).count()
    };
    !view.emissions().is_empty()
        && view.emissions().iter().any(|emission| count(emission, LeafOp::String) == 1)
        && view.emissions().iter().all(|emission| {
            let tag = || {
                emission.leaves.iter().filter(|leaf| leaf.op == LeafOp::Byte).all(|leaf| {
                    leaf.transforms.is_empty() && leaf.classes.iter().all(|class| *class == SourceClass::Literal)
                })
            };
            (1..=2).contains(&count(emission, LeafOp::Number))
                && match (count(emission, LeafOp::String), count(emission, LeafOp::Byte)) {
                    (1, 0) => true,
                    (0, 1) => tag(),
                    _ => false,
                }
        })
}

pub(crate) fn bind_local_storage_persistent_id(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let mut binder = Binder::new(view, layout, ctx)?;
    binder.top(|field| match field.kind() {
        Some(LeafKind::Number) => Some("storageStatus"),
        Some(LeafKind::String) | Some(LeafKind::Byte) if field.leaf_kinds().contains(&LeafKind::String) || field.is_tag() => {
            Some("storedId")
        }
        _ => None,
    })?;
    binder.finish()
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `LocalStoragePersistentId`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "LocalStoragePersistentId",
    summary: "Checks whether the browser has localStorage and whether the script managed to save its persistent visitor ID there. Earlier, the script saves the current transaction token under a fixed key, unless a value is already stored. If saving worked, this signal reads back what is stored under that key. On a first visit that is the current token; on a return visit it is the ID from an earlier session. Some builds also carry over an older legacy ID.",
    why: "A persistent ID in localStorage recognizes the same browser across visits even after cookies are cleared. Missing or failing storage points to privacy modes or locked-down automation browsers (likely).",
    sample_payload: Some(r#"{
  "storageStatus": 3,
  "storedId": "A7dnGmKdAQAAhZWcRWlWhl2h6vRTaGVX6Z9OSxaXqGmLDUfl8GFN27kLCff6AUm92wCucphhwH9eCOfvosJeCA=="
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::string(
            "storedId",
            "The value stored under the persistent-ID key, or a null marker if storage is missing, saving failed, or the read returned nothing.",
        ).nullable(),
        CardField::number(
            "storageStatus",
            "1 = no localStorage property; 2 = localStorage exists but saving failed (access threw, quota exceeded or disabled); 3 = the ID is saved and storedId holds it.",
        ),
    ]),
};
