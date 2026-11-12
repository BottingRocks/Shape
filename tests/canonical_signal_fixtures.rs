//! Offline consistency checks for every published signal identity fixture.
#[path = "support/frozen_signal_layout.rs"]
mod frozen_signal_layout;

use std::collections::BTreeSet;
use std::path::Path;

use frozen_signal_layout::{read_json, read_json_gz, repo_root};
use serde_json::Value;
use shape::signals::catalog::{card, cards};

fn registered(name: &str) {
    assert!(card(name).is_some(), "unknown signal identity {name}");
}

fn nested_identities(value: &Value) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if key == "signal" {
                    registered(child.as_str().expect("signal name"));
                }
                nested_identities(child);
            }
        }
        Value::Array(array) => array.iter().for_each(nested_identities),
        _ => {}
    }
}

fn json_files(dir: &Path) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .expect("fixture directory")
        .map(|entry| entry.expect("fixture entry").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect()
}

#[test]
fn all_published_signal_fixtures_use_registered_identities_and_resolve_layouts() {
    let names: BTreeSet<_> = cards().map(|card| card.signal).collect();
    assert_eq!(names.len(), 93);
    let root = repo_root().join("tests/fixtures/signals");
    let orders = read_json(&root.join("field-orders.json"));
    for name in orders.as_object().expect("field order identities").keys() {
        registered(name);
    }
    let mut known_labels = BTreeSet::new();
    let (mut profiles, mut layouts, mut slots, mut grammars, mut proofs) = (0, 0, 0, 0, 0);
    for entry in std::fs::read_dir(root.join("verified")).expect("verified profiles") {
        let profile = entry.expect("profile").path();
        if !profile.is_dir() {
            continue;
        }
        profiles += 1;
        let slot_map = read_json(&profile.join("slot-map.json"));
        let labels = slot_map.as_object().expect("layout map");
        assert_eq!(json_files(&profile.join("schemas")).len(), labels.len());
        for (label, addresses) in labels {
            layouts += 1;
            known_labels.insert(label.clone());
            let addresses = addresses.as_object().expect("slot identities");
            for identity in addresses.values() {
                registered(identity.as_str().expect("identity"));
                slots += 1;
            }
            let schema = read_json(&profile.join(format!("schemas/{label}.json")));
            assert!(!schema["seed"].as_str().expect("schema seed").is_empty());
            nested_identities(&schema);
            for address in schema["signals"].as_object().expect("schema slots").keys() {
                assert!(
                    addresses.contains_key(address),
                    "{label}: unknown schema address {address}"
                );
                grammars += 1;
            }
        }
        let proof_dir = profile.join("proofs");
        if !proof_dir.is_dir() {
            continue;
        }
        let mut seen = BTreeSet::new();
        for entry in std::fs::read_dir(&proof_dir).expect("proof directory") {
            let path = entry.expect("proof").path();
            if path.extension().is_none_or(|extension| extension != "gz") {
                continue;
            }
            proofs += 1;
            let proof = read_json_gz(&path);
            nested_identities(&proof);
            let name = proof["signal"].as_str().expect("proof identity");
            assert_eq!(
                path.file_name().unwrap().to_str().unwrap(),
                format!("{name}.json.gz")
            );
            let label = proof["label"].as_str().expect("proof label");
            let address = proof["address"].as_str().expect("proof address");
            assert!(
                seen.insert((label.to_string(), address.to_string())),
                "duplicate proof address"
            );
            assert_eq!(slot_map[label][address].as_str(), Some(name));
            let schema_path = proof_dir.join(proof["schema"].as_str().expect("proof schema link"));
            assert_eq!(
                schema_path.file_name().unwrap().to_str().unwrap(),
                format!("{label}.json")
            );
            let schema = read_json(&schema_path);
            assert_eq!(proof["seed"], schema["seed"]);
            assert!(proof["field_set"].is_array(), "retained field evidence");
        }
        let proof_labels: BTreeSet<_> = seen.iter().map(|(label, _)| label).collect();
        for label in proof_labels {
            assert_eq!(
                seen.iter()
                    .filter(|(proof_label, _)| proof_label == label)
                    .count(),
                labels[label].as_object().unwrap().len(),
                "every mapped slot of proof layout {label} has one proof"
            );
        }
    }
    fn check_order_labels(value: &Value, known: &BTreeSet<String>) {
        if let Some(object) = value.as_object() {
            for (key, child) in object {
                if key.ends_with("-layout") {
                    assert!(known.contains(key), "unknown field-order layout {key}");
                }
                check_order_labels(child, known);
            }
        }
    }
    check_order_labels(&orders, &known_labels);
    assert_eq!((profiles, layouts, proofs), (12, 120, 83));
    eprintln!("validated profiles={profiles} layouts={layouts} slots={slots} grammar_records={grammars} proofs={proofs}");
}

#[test]
#[should_panic(expected = "unknown signal identity UnknownSignalIdentifier")]
fn unknown_signal_identifier_is_rejected() {
    nested_identities(&serde_json::json!({"signal": "UnknownSignalIdentifier"}));
}
