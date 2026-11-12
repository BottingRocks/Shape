//! `PersistentStorageState`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafKind, LeafOp, SignalLayout, SlotView};
use value_facts::slot_facts;
use crate::signals::analysis::{value_facts, leaf_checks::*, described_fields::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `PersistentStorageState`. Card: [`CARD`].
pub(crate) fn recognize_persistent_storage_state(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    // One removed-value string and no numbers; the fresh traces also emit
    // the record from paths where the string arm is statically absent
    // (only the null-marker byte and the flags), so string-free emissions
    // are accepted next to at least one with the string.
    let strings = |emission: &crate::signals::analysis::EmissionView| emission.leaves.iter().filter(|leaf| leaf.op == LeafOp::String).count();
    let numbers = |emission: &crate::signals::analysis::EmissionView| emission.leaves.iter().filter(|leaf| leaf.op == LeafOp::Number).count();
    let counts_ok = !view.emissions().is_empty()
        && view.emissions().iter().all(|emission| strings(emission) <= 1 && numbers(emission) == 0)
        && view.emissions().iter().any(|emission| strings(emission) == 1);
    if !counts_ok || view.has_repeat() || !leaf_transform(view, ".oldValue") {
        return None;
    }
    let facts = slot_facts(view, ctx);
    (facts.has_event(&["storage"]) || facts.any_member("oldValue"))
        .then(|| Claim::new(["storage event oldValue", "packed flag byte"]))
}

pub(crate) fn bind_persistent_storage_state(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let mut binder = Binder::new(view, layout, ctx)?;
    binder.top(|field| {
        if field.leaf_kinds().contains(&LeafKind::String) {
            Some("removedValue")
        } else if matches!(field.shape, Shape::Choice) && field.is_tag() {
            // A message whose removed value is statically absent: only the
            // null marker (or nothing) of the value's choice remains.
            Some("removedValue")
        } else if field.kind() == Some(LeafKind::Byte) && !field.is_tag() {
            Some("flags")
        } else {
            None
        }
    })?;
    binder.finish()
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `PersistentStorageState`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "PersistentStorageState",
    summary: "Watches the vendor's persistent-ID entry in localStorage. It reports once at startup whether storage works and whether an ID survived from a previous visit, and again each time another tab clears localStorage or removes the ID, including the removed value.",
    why: "Lets the vendor likely detect first visits, storage being blocked, and users or bots that wipe the tracking ID to look like a new visitor.",
    sample_payload: None,
    by_name: false,
    body: CardBody::Fields(&[
        CardField::string(
            "removedValue",
            "The persistent-ID value that was just removed or cleared, or a null marker if nothing was removed (always null in the startup message).",
        ).nullable(),
        CardField::byte(
            "flags",
            "Packed flags (positions vary per build): localStorage access failed or threw; no persistent ID was inherited from an earlier visit; a removal or clear of the ID was seen. Other bits 0.",
        )
        .flags(&["storageAccessFailed", "noInheritedId", "idRemovalSeen"]),
    ]),
};
