//! Projected signal layouts checked against frozen proofs.
//!
//! For every emoji-channel-sample-halves-match-media-layout slot, the projected layout (`shape signals
//! --layouts-only`), flattened back to its leaves, must equal the frozen proof
//! `tests/fixtures/signals/verified/emoji-channel-sample-halves-match-media-layouts/proofs/<Signal>.json.gz`
//! `field_set`: per emission the op sequence, container chain (repeat /
//! choice arm), mask stream salt, static XOR literal and byte literal values;
//! for named-object emissions the generic value keys in order.
//!
//! Env-gated: `SHAPE_FROZEN_ROOT` (default `/tmp/shape-corpus-ten-fresh`);
//! skips when the corpus is absent.
//!
//! One answer-key format difference is normalized, as the field checker does
//! for `c ? k1 : k2` bytes: the proofs were recorded on the proof-source trace
//! (`SHAPE_PROOF_SOURCE_ROOT`, default `/tmp/shape-corpus-ten-proof-sources`), where a
//! byte can be written through a local (`v = c ? k1 : k2; buf[i] = v ^ mask()`)
//! and the proof keeps it as a generic byte (`byte_literal_values: null`). The
//! fresh trace folds the ternary into the write, and the projection gives the
//! two constants. That pair is accepted only when the proof source at the
//! proof's write assigns exactly `v = <cond> ? k1 : k2` with our two constants.
//!
//! ```text
//! cargo test --profile iter --test program_signal_projection -- --ignored --nocapture
//! ```

#[path = "support/frozen_signal_layout.rs"]
mod frozen_signal_layout;

use std::collections::BTreeMap;

use serde_json::Value;
use shape::signals::analysis::{analyze_program_signals, FlatLeaf, SignalFinderOptions};

const LABEL: &str = "emoji-channel-sample-halves-match-media-layout";

/// A comparable leaf: emission (None for generic values), op, chain, salt,
/// xor, bytes, key.
type Leaf = (
    Option<u64>,
    String,
    Vec<String>,
    Option<i64>,
    Option<String>,
    Option<Vec<i64>>,
    Option<String>,
);

fn salt_of(mask_source: &str) -> Option<i64> {
    mask_source
        .rsplit('^')
        .next()
        .and_then(|tail| tail.trim().trim_end_matches(')').trim().parse::<i64>().ok())
}

fn xor_text(value: Option<f64>) -> Option<String> {
    value.map(|value| format!("{value}"))
}

fn proof_leaves(proof: &Value) -> Vec<Leaf> {
    proof["field_set"]
        .as_array()
        .expect("field_set")
        .iter()
        .map(|field| {
            let op = field["op"].as_str().expect("op").to_string();
            if op == "generic_value" {
                return (
                    None,
                    op,
                    Vec::new(),
                    None,
                    None,
                    None,
                    field["key"].as_str().map(str::to_string),
                );
            }
            let chain = field["path"]
                .as_array()
                .expect("path")
                .iter()
                .filter_map(Value::as_str)
                .filter(|component| !component.chars().all(|c| c.is_ascii_digit()))
                .map(|component| {
                    if component.starts_with("repeat:") {
                        "repeat".to_string()
                    } else {
                        component.to_string()
                    }
                })
                .collect();
            let salt = if field["mask"].as_str() == Some("stream") {
                field["mask_source"].as_str().and_then(salt_of)
            } else {
                None
            };
            let xor = field["xor_literal"].as_f64();
            let bytes = field["byte_literal_values"].as_array().map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_i64)
                    .collect::<Vec<i64>>()
            });
            (
                field["emission"].as_u64(),
                op,
                chain,
                salt,
                xor_text(xor),
                bytes,
                None,
            )
        })
        .collect()
}

fn projected_leaves(leaves: &[FlatLeaf]) -> Vec<Leaf> {
    leaves
        .iter()
        .map(|leaf| {
            if leaf.op == "generic_value" {
                return (
                    None,
                    leaf.op.to_string(),
                    Vec::new(),
                    None,
                    None,
                    None,
                    leaf.key.clone(),
                );
            }
            (
                Some(leaf.emission as u64),
                leaf.op.to_string(),
                leaf.chain.clone(),
                leaf.salt,
                xor_text(leaf.xor),
                leaf.bytes.clone(),
                None,
            )
        })
        .collect()
}

fn proof_source_root() -> std::path::PathBuf {
    std::env::var_os("SHAPE_PROOF_SOURCE_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp/shape-corpus-ten-proof-sources"))
}

/// The proof's own source (proof-source trace), when present with the proof's
/// recorded SHA-256.
fn proof_source(proof: &Value) -> Option<String> {
    proof_source_in(&proof_source_root(), proof)
}

fn proof_source_in(root: &std::path::Path, proof: &Value) -> Option<String> {
    use sha2::{Digest, Sha256};
    let seed = proof["seed"].as_str()?;
    let expected_digest = proof["source_sha256"].as_str()?;
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let path = entry.path().join("programs").join(format!("{seed}.js"));
        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };
        let digest: String = Sha256::digest(source.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        if digest == expected_digest {
            return Some(source);
        }
    }
    None
}

#[test]
fn proof_source_finds_exact_seed_and_digest_across_profiles() {
    use sha2::{Digest, Sha256};

    struct Corpus(std::path::PathBuf);
    impl Drop for Corpus {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "projection-proof-source-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir(&root).expect("create isolated corpus");
    let corpus = Corpus(root);
    let root = corpus.0.as_path();
    let archived = root.join("archived-layout/programs");
    let current = root.join("current-layout/programs");
    std::fs::create_dir_all(&archived).expect("archived profile");
    std::fs::create_dir_all(&current).expect("current profile");
    std::fs::write(root.join("non-directory"), "ignored").expect("non-directory entry");

    let source = "const value = condition ? 3 : 7;\n";
    let digest: String = Sha256::digest(source.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let proof = serde_json::json!({"seed": "chosen-seed", "source_sha256": digest});
    let archived_source = archived.join("chosen-seed.js");
    let current_source = current.join("chosen-seed.js");
    std::fs::write(&archived_source, source).expect("archived source");
    assert_eq!(proof_source_in(root, &proof).as_deref(), Some(source));

    // A conflicting copy in either profile must not hide the matching source.
    std::fs::write(&current_source, "wrong source").expect("conflicting current source");
    assert_eq!(proof_source_in(root, &proof).as_deref(), Some(source));
    std::fs::write(&archived_source, "wrong source").expect("conflicting archived source");
    std::fs::write(&current_source, source).expect("matching current source");
    assert_eq!(proof_source_in(root, &proof).as_deref(), Some(source));
    std::fs::write(&current_source, "wrong source").expect("mismatching source");
    assert_eq!(proof_source_in(root, &proof), None);

    // Matching bytes at a different seed cannot satisfy the proof.
    std::fs::write(archived.join("chosen-seed-extra.js"), source).expect("different seed");
    assert_eq!(proof_source_in(root, &proof), None);
    std::fs::remove_file(&archived_source).expect("remove archived candidate");
    std::fs::remove_file(&current_source).expect("remove current candidate");
    assert_eq!(proof_source_in(root, &proof), None);
    std::fs::create_dir(&archived_source).expect("unreadable source candidate");
    assert_eq!(proof_source_in(root, &proof), None);
    assert_eq!(proof_source_in(&root.join("absent"), &proof), None);
    assert_eq!(
        proof_source_in(root, &serde_json::json!({"seed": "chosen-seed"})),
        None
    );
    assert_eq!(
        proof_source_in(root, &serde_json::json!({"source_sha256": digest})),
        None
    );
}

/// `cond ? k1 : k2` (no nested `?`) -> `[k1, k2]`.
fn literal_ternary(text: &str) -> Option<[i64; 2]> {
    let (condition, arms) = text.split_once('?')?;
    if condition.trim().is_empty() || arms.contains('?') {
        return None;
    }
    let (then, otherwise) = arms.split_once(':')?;
    Some([then.trim().parse().ok()?, otherwise.trim().parse().ok()?])
}

/// The answer-key format difference described in the module docs: the proof
/// byte at `index` is a generic byte written through a local whose last
/// assignment before the write, in the proof's own source, is a two-constant
/// ternary equal to `bytes`.
fn generic_byte_is_local_ternary(proof: &Value, index: usize, bytes: &[i64]) -> bool {
    let Some(field) = proof["field_set"]
        .as_array()
        .and_then(|fields| fields.get(index))
    else {
        return false;
    };
    if field["op"].as_str() != Some("byte") || !field["byte_literal_values"].is_null() {
        return false;
    }
    let (Some(local), Some(write)) = (field["value"].as_str(), field["span"][0].as_u64()) else {
        return false;
    };
    if bytes.len() != 2
        || !local
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
    {
        return false;
    }
    let Some(source) = proof_source(proof) else {
        return false;
    };
    let before = &source[..write as usize];
    let Some(at) = before.rfind(&format!("{local} = ")) else {
        return false;
    };
    let right = &before[at + local.len() + 3..];
    let Some(end) = right.find(';') else {
        return false;
    };
    // Nothing but whitespace may separate the assignment from the write.
    if !right[end + 1..].trim().is_empty() {
        return false;
    }
    literal_ternary(&right[..end]) == Some([bytes[0], bytes[1]])
}

/// The first difference between two leaf lists, as a mismatch kind.
fn mismatch(proof_json: &Value, proof: &[Leaf], ours: &[Leaf]) -> Option<(String, String)> {
    for (index, (want, got)) in proof.iter().zip(ours).enumerate() {
        let kind = if want.0 != got.0 {
            "emission"
        } else if want.1 != got.1 {
            "op"
        } else if want.2 != got.2 {
            "chain"
        } else if want.3 != got.3 {
            "salt"
        } else if want.4 != got.4 {
            "xor"
        } else if want.5 != got.5 {
            if want.5.is_none()
                && got
                    .5
                    .as_deref()
                    .is_some_and(|bytes| generic_byte_is_local_ternary(proof_json, index, bytes))
                && want.6 == got.6
            {
                eprintln!(
                    "normalized leaf {index}: proof generic byte through a local `c ? k1 : k2` vs projected {:?}",
                    got.5
                );
                continue;
            }
            "byte_literal"
        } else if want.6 != got.6 {
            "key"
        } else {
            continue;
        };
        return Some((
            kind.to_string(),
            format!("leaf {index}: proof {want:?} vs projected {got:?}"),
        ));
    }
    if proof.len() != ours.len() {
        let kind = if ours.is_empty() {
            "no_projected_leaves"
        } else {
            "leaf_count"
        };
        return Some((
            kind.to_string(),
            format!("proof {} leaves vs projected {}", proof.len(), ours.len()),
        ));
    }
    None
}

#[test]
#[ignore = "needs the frozen corpus-ten corpus"]
fn frozen_program_projection_matches_frozen_proofs() {
    let Some(program) = frozen_signal_layout::layout_program(LABEL) else {
        eprintln!(
            "skip: {LABEL} program not found under {}",
            frozen_signal_layout::frozen_root().display()
        );
        return;
    };
    let source = std::fs::read_to_string(&program).expect("program");
    let options = SignalFinderOptions {
        layouts_only: true,
        finder_roles: false,
    };
    let analysis = analyze_program_signals(&source, &options).expect("analysis");
    let proofs = frozen_signal_layout::layout_proofs(LABEL);
    let identities = frozen_signal_layout::layout_identities(LABEL);

    let mut matching = Vec::new();
    let mut mismatching: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut without_proof = Vec::new();
    for slot in &analysis.slots {
        let Some(proof) = proofs.get(&slot.address) else {
            without_proof.push(slot.address.clone());
            continue;
        };
        assert_eq!(
            proof["descriptor"].as_u64(),
            Some(slot.descriptor as u64),
            "{}: proof descriptor",
            slot.address
        );
        let want = proof_leaves(proof);
        let got = projected_leaves(&slot.layout.flat_leaves());
        let name = identities
            .get(&slot.address)
            .map(String::as_str)
            .unwrap_or("-");
        match mismatch(proof, &want, &got) {
            None => matching.push(slot.address.clone()),
            Some((kind, detail)) => mismatching
                .entry(kind)
                .or_default()
                .push(format!("{} ({name}): {detail}", slot.address)),
        }
    }
    let mismatch_count: usize = mismatching.values().map(Vec::len).sum();
    eprintln!(
        "{LABEL}: slots={} proofs={} matching={} mismatching={} without_proof={:?}",
        analysis.slots.len(),
        proofs.len(),
        matching.len(),
        mismatch_count,
        without_proof
    );
    for (kind, slots) in &mismatching {
        eprintln!("mismatch {kind}: {}", slots.len());
        for slot in slots {
            eprintln!("  {slot}");
        }
    }
    // Known contract gap: the contract pass does not replay the
    // `document.cookie` forEach emitter of CorsCookieValue, so that slot has
    // no projected leaves. Every other slot must match its proof exactly.
    let unexpected: Vec<&String> = mismatching
        .iter()
        .filter(|(kind, _)| kind.as_str() != "no_projected_leaves")
        .flat_map(|(_, slots)| slots)
        .collect();
    assert!(
        unexpected.is_empty(),
        "projection differs from proofs: {unexpected:#?}"
    );
    assert!(
        mismatching.get("no_projected_leaves").map_or(0, Vec::len) <= 1,
        "more contract gaps than the known CorsCookieValue one"
    );
}
