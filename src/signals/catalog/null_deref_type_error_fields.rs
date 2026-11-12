//! `NullDerefTypeErrorFields`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafOp, SignalLayout, SlotView};
use crate::signals::extract::contracts::SourceClass;
use buffer_writes::slot_info;
use crate::signals::analysis::{buffer_writes, leaf_checks::*, flag_bytes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


const ERROR_FIELDS: &[&str] = &["stacktrace", "message", "name", "number", "stack", "description"];

/// `NullDerefTypeErrorFields`. Card: [`CARD`].
pub(crate) fn recognize_null_deref_type_error_fields(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let direct = |transform: &str| {
        view.leaves().any(|leaf| {
            leaf.transforms == [transform] && leaf.classes == [SourceClass::CaughtError]
        })
    };
    // Shape: two plain strings (message, name) and four choices of a null
    // marker or a value (stacktrace, number, stack, description). Choice
    // values may carry polluted provenance (register reuse), so only one of
    // the legacy properties must be read directly.
    let shaped = view.emissions().iter().all(|emission| {
        let plain = emission.leaves.iter().filter(|leaf| leaf.choice_depth == 0 && leaf.repeat_depth == 0);
        let plain_strings = plain.clone().filter(|leaf| leaf.op == LeafOp::String).count();
        let chosen = emission.leaves.iter().filter(|leaf| leaf.choice_depth == 1 && leaf.repeat_depth == 0).count();
        plain.count() == 2 && plain_strings == 2 && chosen == 8 && emission.leaves.len() == 10
    });
    if view.emissions().is_empty() || !(shaped && direct(".message") && direct(".name")) {
        return None;
    }
    let legacy_direct = [".stacktrace", ".description", ".number"]
        .iter()
        .any(|transform| direct(transform));
    // The binder names fields from the slot-local write slices; they also
    // prove the legacy properties when the provenance is polluted.
    let info = slot_info(view, ctx);
    let legacy_slices = ["stacktrace", "number", "stack", "description"]
        .iter()
        .filter(|property| info.facts.member(property))
        .count();
    (legacy_direct || legacy_slices >= 3)
        .then(|| claim(&["six properties of a caught error object"]))
        .flatten()
}

pub(crate) fn bind_null_deref_type_error_fields(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let info = slot_info_for_binder(view, ctx)?;
    let hints = emission_hints(&info, view, |write, _| {
        if write.literal {
            return None;
        }
        let read: Vec<&str> = ERROR_FIELDS
            .iter()
            .copied()
            .filter(|field| write.facts.member(field))
            .collect();
        match read.as_slice() {
            [one] => Some(Hint::Name(one.to_string())),
            // A write that reads no error property is a missing-value marker
            // (`ks ^ 133` with the keystream value in a local); it names
            // nothing, so a field made only of such writes stays unnamed.
            [] => None,
            _ => Some(Hint::Name(format!("?{read:?}"))),
        }
    });
    let fields = message_hints(layout, &hints)?;
    if fields
        .iter()
        .flatten()
        .any(|hint| matches!(hint, Some(Hint::Name(name)) if name.starts_with('?')))
    {
        return Err(BindRefusal::new("a field reads no single error property"));
    }
    bind_named(layout, &fields)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `NullDerefTypeErrorFields`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "NullDerefTypeErrorFields",
    summary: "Deliberately runs null[0]() to throw a TypeError, catches it, and sends six properties of that error object: stacktrace, message, name, number, a cleaned-up stack, and description.",
    why: "Error message wording, stack format and the presence of legacy properties (stacktrace, number, description) differ by JavaScript engine and version, so this likely fingerprints the engine and exposes user-agent spoofing or a patched Error object.",
    sample_payload: Some(r#"{
  "description": null,
  "message": "Cannot read properties of null (reading '0')",
  "name": "TypeError",
  "number": null,
  "stack": "TypeError: Cannot read properties of null (reading '0')\n    at URL",
  "stacktrace": null
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::string(
            "stacktrace",
            "The error's stacktrace property (old Opera only), or a null marker if it is missing.",
        ).nullable(),
        CardField::string(
            "message",
            "The error message text, e.g. \"Cannot read properties of null (reading '0')\" (Chrome) or \"null has no properties\" (Firefox). Always present.",
        ),
        CardField::string("name", "The error name, normally \"TypeError\". Always present."),
        CardField::number(
            "number",
            "The error's number property (old IE/JScript error code), or a null marker if it is missing.",
        ).nullable(),
        CardField::string(
            "stack",
            "The first two lines of the stack, with every URL replaced by \"URL\" and cut to at most 1000 characters, or a null marker if there is no stack.",
        ).nullable(),
        CardField::string(
            "description",
            "The error's description property (old IE/legacy Edge), or a null marker if it is missing.",
        ).nullable(),
    ]),
};
