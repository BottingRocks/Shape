//! `CurrentScriptAsyncPolicyState`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use std::collections::BTreeSet;
use oxc_semantic::SymbolId;
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafKind, SignalLayout, SlotView};
use value_facts::slot_facts;
use crate::signals::analysis::{value_facts, leaf_checks::*, described_fields::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `CurrentScriptAsyncPolicyState`. Card: [`CARD`].
pub(crate) fn recognize_current_script_async_policy_state(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    if !per_emission_counts(view, 1, 0..=0) || view.has_repeat() || !leaf_transform(view, ".policyHash") {
        return None;
    }
    let facts = slot_facts(view, ctx);
    facts
        .any_member("policyHash")
        .then(|| Claim::new(["script policy record policyHash", "currentScript.async packed byte"]))
}

pub(crate) fn bind_current_script_async_policy_state(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let mut binder = Binder::new(view, layout, ctx)?;
    let fields = binder.top_count();
    if fields.iter().any(|count| !(5..=6).contains(count)) {
        return Err(BindRefusal::new(format!("layout has {fields:?} top-level fields; the card names 5 or 6")));
    }
    // Optional tags (choices of literal bytes) are told apart by their
    // guard: the async-absent tag tests the `async` read; the policy-absent
    // tag tests the binding whose `.policyHash` the string field writes.
    // Two tags of one name refuse (duplicate).
    let hash_symbols: Vec<BTreeSet<SymbolId>> = binder
        .messages
        .iter()
        .map(|message| {
            message
                .iter()
                .filter(|field| field.leaf_kinds().contains(&LeafKind::String))
                .flat_map(|field| field.value_symbols(ctx, 3))
                .collect()
        })
        .collect();
    // The async choice may carry the async null marker and the flags of the
    // unknown-async case as one arm (`async == null ? (tag, flags) :
    // flags`): the arm's literal tag is `scriptAsync`, its other byte and
    // the other arm's byte are `scriptFlags`.
    let mut async_groups: Vec<(usize, String, Vec<(String, &'static str)>)> = Vec::new();
    for (message, fields) in binder.messages.iter().enumerate() {
        for field in fields.iter().filter(|field| field.shape == Shape::Choice && !field.arms.is_empty()) {
            let guard = field.guard_slice(ctx, 4);
            let script = guard.members.contains_key("async") || guard.members.contains_key("currentScript");
            let mut arm_names = Vec::new();
            for arm in &field.arms {
                match arm.as_slice() {
                    [] => {}
                    [marker, flags] if marker.is_tag() && flags.kind() == Some(LeafKind::Byte) => {
                        arm_names.push((marker.key.clone(), "scriptAsync"));
                        arm_names.push((flags.key.clone(), "scriptFlags"));
                    }
                    _ => {
                        return Err(BindRefusal::new(format!(
                            "message {message}: record choice {} is not the async marker + flags",
                            field.key
                        )))
                    }
                }
            }
            if !script || arm_names.is_empty() {
                return Err(BindRefusal::new(format!(
                    "message {message}: record choice {} is not guarded by the async read",
                    field.key
                )));
            }
            async_groups.push((message, field.key.clone(), arm_names));
        }
    }
    let mut tag_choices = Vec::new();
    for (message, fields) in binder.messages.iter().enumerate() {
        for field in fields.iter().filter(|field| field.shape == Shape::Choice && field.is_tag()) {
            let guard = field.guard_slice(ctx, 4);
            let policy = guard.symbols.iter().any(|(symbol, _)| hash_symbols[message].contains(symbol));
            let script = guard.members.contains_key("async") || guard.members.contains_key("currentScript");
            // The async read is direct evidence; a shared register can put
            // the record's binding into the async guard's slice too.
            let name = match (script, policy) {
                (true, _) => "scriptAsync",
                (false, true) => "policyAbsentTag",
                (false, false) => {
                    return Err(BindRefusal::new(format!(
                        "message {message}: optional tag {} is guarded by neither the policy record nor the async read",
                        field.key
                    )))
                }
            };
            tag_choices.push((message, field.key.clone(), name));
        }
    }
    let mut unconditional = 0;
    binder.top_at(|message, fields, index| {
        let field = &fields[index];
        if index == 0 {
            unconditional = 0;
        }
        if async_groups.iter().any(|(m, key, _)| *m == message && *key == field.key) {
            Some("asyncState")
        } else if field.leaf_kinds().contains(&LeafKind::String) {
            Some("policyHash")
        } else if field.shape == Shape::Choice {
            tag_choices.iter().find(|(m, key, _)| *m == message && *key == field.key).map(|(_, _, name)| *name)
        } else if !field.is_tag() {
            Some("scriptFlags")
        } else {
            unconditional += 1;
            match unconditional {
                1 => Some("status1"),
                2 => Some("status2"),
                _ => None,
            }
        }
    })?;
    for (message, _, names) in async_groups {
        for (key, name) in names {
            binder.binding.insert(message, key, name);
        }
    }
    binder.finish()
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `CurrentScriptAsyncPolicyState`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "CurrentScriptAsyncPolicyState",
    summary: "Records whether the <script> element that loaded the collector (document.currentScript) has its async flag set. The message also has room for a script policy record and its policyHash, but in all analyzed builds that record is always empty, so the signal always reports it as missing. The rest is constant status bytes.",
    why: "How the vendor script was loaded (async or not, or with no currentScript at all) likely shows whether it was injected or loaded in an unusual way, for example by automation, a proxy or a script re-hosting the collector.",
    sample_payload: None,
    by_name: false,
    body: CardBody::Fields(&[
        CardField::null(
            "scriptAsync",
            "Only present as a null marker when there is no currentScript or its async property is missing; when async is known, nothing is written here and the value goes into scriptFlags instead.",
        ),
        CardField::byte(
            "status1",
            "Constant status tag, always the same value; meaning unknown (likely a field that is always missing in these builds).",
        ),
        CardField::byte(
            "status2",
            "Second constant status tag; meaning unknown (likely another always-missing field).",
        ),
        CardField::null(
            "policyAbsentTag",
            "Tag byte meaning the script policy record is absent (135 in the main build). Always written in the analyzed builds, because the record is never set.",
        ),
        CardField::string(
            "policyHash",
            "Hash string of the script policy record (or a policyHash-absent marker), written only when the record is present instead of policyAbsentTag; never reached in the analyzed builds.",
        ).nullable(),
        CardField::byte(
            "scriptFlags",
            "Packed byte holding the async flag (1 = async script, 0 = not async; 0 when unknown) plus fixed constant bits whose meaning is unknown.",
        )
        .flags(&["async"]),
        // Some builds group the marker and packed flags under one condition.
        CardField::group("asyncState", &[
            CardField::null("scriptAsync", "Null when currentScript.async is unknown."),
            CardField::byte("scriptFlags", "Packed async flag and fixed bits, as above.")
                .flags(&["async"]),
        ]),
    ]),
};
