//! `VmSourceIntegrity`: the slot recognizer and the field binder.

#![allow(unused_variables)]
use crate::signals::analysis::{BindRefusal, Claim, FieldBinding, FinderCtx, LeafOp, LeafView, SignalLayout, SlotView};
use crate::signals::extract::contracts::SourceClass;
use buffer_writes::slot_info;
use crate::signals::analysis::{buffer_writes, leaf_checks::*, flag_bytes::*};
use crate::signals::catalog::card::{Card, CardBody, CardField};


/// `VmSourceIntegrity`. Card: [`CARD`].
pub(crate) fn recognize_vm_source_integrity(view: &SlotView, ctx: &FinderCtx) -> Option<Claim> {
    let shaped = view.emissions().iter().all(|emission| {
        let strings: Vec<&LeafView> = emission.leaves.iter().filter(|leaf| leaf.op == LeafOp::String).collect();
        let numbers = emission.leaves.iter().filter(|leaf| leaf.op == LeafOp::Number).count();
        strings.len() == 1
            && strings[0].classes == [SourceClass::Literal]
            && numbers >= 3
            && emission.leaves.len() == numbers + 1
            && emission.leaves.iter().all(|leaf| leaf.repeat_depth == 0 && leaf.choice_depth == 0)
    });
    if view.emissions().is_empty() || !shaped {
        return None;
    }
    let measured = view.leaves().any(|leaf| has_transform(leaf, ".lastIndexOf()"))
        && view.leaves().any(|leaf| has_transform(leaf, ".match()"));
    // The binder names fields from the slot-local write slices; they also
    // prove the measurement when the provenance stops at shared state.
    let info = slot_info(view, ctx);
    let sliced = info.facts.call("lastIndexOf") && info.facts.call("match");
    // Builds whose measurement the tracer folded to literals: every emission
    // has three padded values bound to literals (see `vm_source_folded`).
    let folded = info
        .emissions
        .iter()
        .all(|candidates| !candidates.is_empty() && candidates.iter().all(|emission| vm_source_folded(emission).is_some()));
    if !(measured || sliced || folded) {
        return None;
    }
    claim(&["build marker string", "padded body length / whitespace / punctuation counts of a function source"])
}

fn vm_source_name(write: &buffer_writes::WriteInfo, leaf: &LeafView) -> Option<Hint> {
    if leaf.op == LeafOp::String {
        return Some(Hint::Name("buildMarker".into()));
    }
    if write.fraction {
        return Some(Hint::Name("nonce".into()));
    }
    let padded = write.padded.as_ref()?;
    if padded.call("match") {
        let patterns = padded.regex.iter().chain(padded.strings.iter());
        let whitespace = patterns.clone().any(|p| p.contains("\\s"));
        return Some(Hint::Name(if whitespace { "whitespaceCount" } else { "punctuationCount" }.into()));
    }
    (padded.call("substring") || padded.member("length")).then(|| Hint::Name("bodyLength".into()))
}

/// Names of the padded writes of one emission whose measurement the tracer
/// folded to literals (`len = 337841; ws = 1203; punct = 20103;`). The
/// measurement template computes the body length, then the whitespace match
/// count, then the punctuation match count (224 of 224 unfolded frozen
/// measurements), so the folded definitions keep that order; the body length
/// also bounds both counts. `None` unless exactly three padded writes are all
/// folded.
fn vm_source_folded(emission: &buffer_writes::EmissionInfo) -> Option<Vec<Option<&'static str>>> {
    let padded: Vec<(usize, (u32, f64))> = emission
        .writes
        .iter()
        .enumerate()
        .filter(|(_, write)| write.padded.is_some())
        .map(|(index, write)| write.folded.map(|folded| (index, folded)))
        .collect::<Option<_>>()?;
    if padded.len() != 3 {
        return None;
    }
    let mut ordered = padded.clone();
    ordered.sort_by_key(|(_, (start, _))| *start);
    ordered.dedup_by_key(|(_, (start, _))| *start);
    if ordered.len() != 3 {
        return None;
    }
    let length = ordered[0].1 .1;
    if ordered[1..].iter().any(|(_, (_, count))| *count > length) {
        return None;
    }
    let mut names = vec![None; emission.writes.len()];
    for ((index, _), name) in ordered.iter().zip(["bodyLength", "whitespaceCount", "punctuationCount"]) {
        names[*index] = Some(name);
    }
    Some(names)
}

pub(crate) fn bind_vm_source_integrity(
    view: &SlotView,
    layout: &SignalLayout,
    ctx: &FinderCtx,
) -> Result<FieldBinding, BindRefusal> {
    let info = slot_info_for_binder(view, ctx)?;
    let mut hints = emission_hints(&info, view, vm_source_name);
    // Folded measurements: the padded fields take their names from the fold
    // order; candidates must agree.
    for (index, (candidates, view_emission)) in info.emissions.iter().zip(view.emissions()).enumerate() {
        let folded: Vec<Vec<Option<&'static str>>> = candidates.iter().filter_map(vm_source_folded).collect();
        if folded.is_empty() || folded.len() != candidates.len() || folded.iter().any(|names| *names != folded[0]) {
            continue;
        }
        let names = &folded[0];
        let merged: Option<Vec<Option<Hint>>> = candidates[0]
            .writes
            .iter()
            .zip(&view_emission.leaves)
            .enumerate()
            .map(|(at, (write, leaf))| match names[at] {
                Some(name) => Some(Some(Hint::Name(name.into()))),
                None => Some(vm_source_name(write, leaf)),
            })
            .collect();
        if let Some(slot) = hints.get_mut(index) {
            *slot = merged;
        }
    }
    let fields = message_hints(layout, &hints)?;
    for (index, message) in fields.iter().enumerate() {
        let mut names: Vec<&Hint> = message.iter().flatten().collect();
        let total = names.len();
        names.sort_by_key(|hint| format!("{hint:?}"));
        names.dedup();
        if names.len() != total {
            return Err(BindRefusal::new(format!("message {index}: two fields resolve to one card field")));
        }
    }
    bind_named(layout, &fields)
}

// ===========================================================================
// Card
// ===========================================================================

/// The approved card of `VmSourceIntegrity`: what it collects, why, and its fields
/// (names, wire types, flag names). Field names in the schema come from here.
pub(crate) const CARD: Card = Card {
    signal: "VmSourceIntegrity",
    summary: "Reads the source text of the obfuscated script's own VM interpreter function with Function.prototype.toString, keeps the body between the first '{' and the last '}', and measures its length, whitespace count and punctuation count. Only the low 8 bits of each count are sent, each hidden behind a random pad byte, together with a build marker string and a random nonce.",
    why: "Pretty-printing, re-minifying or editing the script changes these counts, so this likely detects researchers or bots that have tampered with or reformatted the anti-bot script; the marker tells the server which build's expected values to compare against.",
    sample_payload: Some(r#"{
  "bodyLength": 36773,
  "buildMarker": "e5cfce5772068bdc",
  "nonce": 0.515963662070013,
  "punctuationCount": 29783,
  "whitespaceCount": 6660
}"#),
    by_name: false,
    body: CardBody::Fields(&[
        CardField::number(
            "bodyLength",
            "Low 8 bits of the function body length (UTF-16 units), sent as pad*256 + (low8 XOR pad) with a random pad 0-255.",
        ),
        CardField::number(
            "nonce",
            "Fractional part of a random/clock mix, 0 to <1 (usually a float). Absent in some builds.",
        ),
        CardField::number(
            "whitespaceCount",
            "Low 8 bits of the number of whitespace characters in the body, padded the same way.",
        ),
        CardField::number(
            "punctuationCount",
            "Low 8 bits of the number of . { ( [ , ; = / characters in the body, padded the same way.",
        ),
        CardField::string(
            "buildMarker",
            "Fixed 16-hex-digit string unique to the script build, e.g. \"a8243d24060c1c6b\".",
        ),
    ]),
};
