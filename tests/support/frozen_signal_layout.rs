//! Frozen corpus-ten corpus helpers shared by the env-gated signal-finder
//! tests. Tests may read the verified answer key
//! (`tests/fixtures/signals/verified/`);
//! recognition never does.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn frozen_root() -> PathBuf {
    std::env::var_os("SHAPE_FROZEN_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp/shape-corpus-ten-fresh"))
}

pub fn verified_layout() -> PathBuf {
    repo_root()
        .join("tests/fixtures/signals/verified/emoji-channel-sample-halves-match-media-layouts")
}

pub fn read_json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

pub fn read_json_gz(path: &Path) -> Value {
    let output = std::process::Command::new("gzip")
        .arg("-dc")
        .arg(path)
        .output()
        .unwrap_or_else(|e| panic!("gzip {}: {e}", path.display()));
    assert!(output.status.success(), "gzip -dc {}", path.display());
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The program of a verified layout (`emoji-channel-sample-halves-match-media-layout`), when the frozen corpus exists.
pub fn layout_program(label: &str) -> Option<PathBuf> {
    let schema = read_json(&verified_layout().join(format!("schemas/{label}.json")));
    let seed = schema["seed"].as_str()?;
    // Locate by the exact seed so existing external corpus directories remain usable.
    std::fs::read_dir(frozen_root())
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("programs").join(format!("{seed}.js")))
        .find(|path| path.is_file())
}

/// `address -> canonical identity` of one verified layout.
pub fn layout_identities(label: &str) -> BTreeMap<String, String> {
    let map = read_json(&verified_layout().join("slot-map.json"));
    map[label]
        .as_object()
        .expect("label")
        .iter()
        .map(|(address, identity)| {
            let name = identity.as_str().expect("signal identity");
            assert!(
                shape::signals::catalog::card(name).is_some(),
                "unknown signal {name}"
            );
            (address.clone(), name.to_string())
        })
        .collect()
}

/// `address -> proof json` of one verified layout.
pub fn layout_proofs(label: &str) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    let suffix = ".json.gz";
    for entry in std::fs::read_dir(verified_layout().join("proofs")).expect("proofs") {
        let path = entry.expect("entry").path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.ends_with(suffix) {
            let proof = read_json_gz(&path);
            if proof["label"].as_str() != Some(label) {
                continue;
            }
            let address = proof["address"].as_str().expect("address").to_string();
            out.insert(address, proof);
        }
    }
    out
}
