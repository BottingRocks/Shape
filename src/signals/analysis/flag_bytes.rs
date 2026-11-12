//! Flag-byte slots: byte and field counts per emission, binding flag bytes
//! by build position (`integrityFlags1`, `proxyFlags2`, …: which flags land
//! in which byte changes per build), and binding named fields from per-leaf
//! [`Hint`]s derived from [`buffer_writes`](super::buffer_writes).
//!
//! Used by: `BrowserApiOwnPropertyFlags`, `BrowserApiProxyFlags`,
//! `BuiltinProbeChecksum`, `CleanConsoleStackAccessFlags`,
//! `ConsoleStackAccessFlags`, `FingerprintApiIntegrity`,
//! `GrecaptchaAccessorHookFlags`, `NativeApiIntegrityFlags`,
//! `NullDerefTypeErrorFields`, `ProxyTamperFlags`, `VmSourceIntegrity`.

use super::buffer_writes::{self, slot_info, SlotInfo};
use super::layout_nodes::{leaf_of, node_leaf_count, same_message};
use super::{BindRefusal, FieldBinding, FinderCtx, LeafKind, LeafOp, LeafView, SignalLayout, SlotView};

/// Top-level fields per emission (plain leaves plus non-byte choice arms).
pub(crate) fn top_field_counts(layout_or_view: &SlotView) -> Vec<usize> {
    layout_or_view
        .emissions()
        .iter()
        .map(|emission| {
            // A top-level field is a leaf outside any choice/repeat or one
            // choice; count choice groups by their first literal-marker arm.
            let plain = emission
                .leaves
                .iter()
                .filter(|leaf| leaf.repeat_depth == 0 && leaf.choice_depth == 0)
                .count();
            let choices = emission
                .leaves
                .iter()
                .filter(|leaf| leaf.choice_depth == 1 && leaf.op != LeafOp::Byte)
                .count();
            plain + choices
        })
        .collect()
}

/// Number leaves inside a choice: the optional source-hash slots.
pub(crate) fn hash_slots(view: &SlotView) -> usize {
    view.leaves()
        .filter(|leaf| leaf.choice_depth == 1 && leaf.op == LeafOp::Number)
        .count()
}

/// Flag bytes per emission (top-level bytes; for a split gated choice the
/// arm's bytes).
pub(crate) fn byte_counts(view: &SlotView) -> Vec<usize> {
    view.emissions()
        .iter()
        .map(|emission| {
            let top = emission
                .leaves
                .iter()
                .filter(|leaf| leaf.repeat_depth == 0 && leaf.choice_depth == 0)
                .count();
            // `if (gate) {RAW4 B B (B)} else {RAW4 B B (B)} (B)`: one arm's
            // bytes plus the bytes written after the `if`.
            let gated = emission
                .leaves
                .iter()
                .filter(|leaf| leaf.repeat_depth == 0 && leaf.choice_depth == 1)
                .count();
            top + gated / 2
        })
        .collect()
}

/// Name the top-level fields of every message in order; every field must
/// be a single byte.
pub(crate) fn bind_bytes_in_order(layout: &SignalLayout, names: &[&str]) -> Result<FieldBinding, BindRefusal> {
    bind_flag_bytes(layout, names, false)
}

/// [`bind_bytes_in_order`]; with `constant_messages`, a message whose every
/// byte is a literal (a gated "all flags clear" arm) is named the same way.
pub(crate) fn bind_flag_bytes(layout: &SignalLayout, names: &[&str], constant_messages: bool) -> Result<FieldBinding, BindRefusal> {
    if layout.messages.is_empty() {
        return Err(BindRefusal::new("no message layout"));
    }
    let mut binding = FieldBinding::new();
    for (index, message) in layout.messages.iter().enumerate() {
        if message.fields.len() != names.len() {
            return Err(BindRefusal::new(format!(
                "message {index}: expected {} flag bytes, found {} fields",
                names.len(),
                message.fields.len()
            )));
        }
        // A closed gate writes `ks() ^ 0`: every byte constant, or (an arm
        // message of a split gate choice whose trailing byte is shared with
        // the open arm) some bytes constant.
        let gate_arm = layout.messages.len() > 1 && message.when.iter().all(|when| when.contains(':'));
        let constant = constant_messages
            && (gate_arm
                || message
                    .fields
                    .iter()
                    .all(|field| leaf_of(&field.node).is_some_and(|leaf| leaf.kind == LeafKind::Byte && leaf.byte.is_some())));
        for (field, name) in message.fields.iter().zip(names) {
            match leaf_of(&field.node) {
                Some(leaf) if leaf.kind == LeafKind::Byte && (leaf.byte.is_none() || constant) => {
                    binding.insert(index, field.key.clone(), *name)
                }
                _ => {
                    return Err(BindRefusal::new(format!(
                        "message {index}: field {} is not a flag byte",
                        field.key
                    )))
                }
            }
        }
    }
    Ok(binding)
}

/// A leaf name hint: a card name, or a role the binder resolves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Hint {
    Name(String),
    /// A build-ordinal flag byte.
    Flag,
}

/// Per top-level field of each message, the one hint of its leaves.
/// `hints[e]` lists one entry per view leaf of emission `e`.
pub(crate) fn message_hints(
    layout: &SignalLayout,
    hints: &[Option<Vec<Option<Hint>>>],
) -> Result<Vec<Vec<Option<Hint>>>, BindRefusal> {
    let mut out = Vec::new();
    for (index, message) in layout.messages.iter().enumerate() {
        let mut candidates: Vec<Vec<Option<Hint>>> = Vec::new();
        for emission in &layout.emissions {
            let Some(Some(leaf_hints)) = hints.get(emission.emission) else {
                continue;
            };
            if !emission.covers(leaf_hints.len()) {
                return Err(BindRefusal::new("leaf hints do not match the layout"));
            }
            for (part_index, (_, part)) in emission.parts.iter().enumerate() {
                // A part's leaves: one run, or (after a split choice) its
                // arm's run followed by the shared trailing items.
                let part_hints: Vec<Option<Hint>> = emission
                    .part_leaves(part_index)
                    .iter()
                    .map(|leaf| leaf_hints[*leaf].clone())
                    .collect();
                let mut cursor = 0;
                let mut fields = Vec::new();
                for field in &part.fields {
                    let count = node_leaf_count(&field.node);
                    let Some(slice) = part_hints.get(cursor..cursor + count) else {
                        return Err(BindRefusal::new("leaf hints do not cover the layout"));
                    };
                    cursor += count;
                    let mut distinct: Vec<&Hint> = Vec::new();
                    for hint in slice.iter().flatten() {
                        if !distinct.contains(&hint) {
                            distinct.push(hint);
                        }
                    }
                    if distinct.len() > 1 {
                        return Err(BindRefusal::new(format!(
                            "message {index}: field {} reads more than one card field",
                            field.key
                        )));
                    }
                    fields.push(distinct.first().map(|hint| (*hint).clone()));
                }
                if same_message(part, message) {
                    candidates.push(fields);
                }
            }
        }
        let Some(first) = candidates.first().cloned() else {
            return Err(BindRefusal::new(format!(
                "message {index}: no emission with slot-local write facts"
            )));
        };
        if candidates.iter().any(|other| *other != first) {
            return Err(BindRefusal::new(format!(
                "message {index}: emissions sharing this layout read different fields"
            )));
        }
        out.push(first);
    }
    Ok(out)
}

/// Leaf hints per view emission from its AST candidates: `name(write, leaf)`
/// for every write; `None` when no candidate matched or candidates disagree.
pub(crate) fn emission_hints(
    info: &SlotInfo,
    view: &SlotView,
    name: impl Fn(&buffer_writes::WriteInfo, &LeafView) -> Option<Hint>,
) -> Vec<Option<Vec<Option<Hint>>>> {
    info.emissions
        .iter()
        .zip(view.emissions())
        .map(|(candidates, view_emission)| {
            let mut out: Option<Vec<Option<Hint>>> = None;
            for candidate in candidates {
                let hints: Vec<Option<Hint>> = candidate
                    .writes
                    .iter()
                    .zip(&view_emission.leaves)
                    .map(|(write, leaf)| name(write, leaf))
                    .collect();
                match &out {
                    None => out = Some(hints),
                    Some(previous) if *previous == hints => {}
                    Some(_) => return None,
                }
            }
            out
        })
        .collect()
}

/// The memoized [`SlotInfo`] of `view` for a binder.
pub(crate) fn slot_info_for_binder(view: &SlotView, ctx: &FinderCtx) -> Result<SlotInfo, BindRefusal> {
    Ok(slot_info(view, ctx))
}

/// Bind messages whose every top-level field resolved to one card name.
pub(crate) fn bind_named(layout: &SignalLayout, fields: &[Vec<Option<Hint>>]) -> Result<FieldBinding, BindRefusal> {
    let mut binding = FieldBinding::new();
    for (index, (message, hints)) in layout.messages.iter().zip(fields).enumerate() {
        for (field, hint) in message.fields.iter().zip(hints) {
            match hint {
                Some(Hint::Name(name)) => binding.insert(index, field.key.clone(), name.clone()),
                _ => {
                    return Err(BindRefusal::new(format!(
                        "message {index}: field {} has no card name",
                        field.key
                    )))
                }
            }
        }
    }
    Ok(binding)
}
