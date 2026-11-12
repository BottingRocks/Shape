//! Tests over the whole signal catalog: the registry, the structural rules
//! of every recognition source, and corpus runs of all signals (or the
//! signals named by `SHAPE_SIGNAL=<Name>[,<Name>]`) on the verified vendor
//! seeds of the frozen corpus (`SHAPE_FROZEN_ROOT`, default
//! `/tmp/shape-corpus-ten-fresh`; `SHAPE_VENDORS=<profile>` and
//! `SHAPE_SEEDS=baseline` pick the seeds). Corpus tests are `#[ignore]`d and
//! skip seeds whose frozen program is absent:
//!
//! ```text
//! cargo test --profile iter --lib catalog_tests -- --ignored --nocapture
//! SHAPE_SIGNAL=FontMetrics cargo test --profile iter --lib signals_on_vendor_seeds -- --ignored --nocapture
//! ```
//!
//! Truth: the verified answer key under `tests/fixtures/signals/verified/`
//! (per vendor `slot-map.json` and trimmed `schemas/<label>.json`), the
//! analysts' field orders (`tests/fixtures/signals/field-orders.json`), the
//! canonical registry and the cards. Tests may read them; recognition
//! never does.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::{analyze_program_signals, SignalAnalysis, SignalFinderOptions, SIGNALS};

/// Paths and truth files of the frozen corpus and the verified alignment.
pub(crate) mod testing {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;

    pub(crate) fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    /// Every field name of a card (every message, nested group fields).
    pub(crate) fn card_field_names(canonical: &str) -> BTreeSet<String> {
        crate::signals::catalog::card(canonical)
            .map(|card| {
                card.all_fields()
                    .into_iter()
                    .map(|field| field.name.to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn frozen_root() -> PathBuf {
        std::env::var_os("SHAPE_FROZEN_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp/shape-corpus-ten-fresh"))
    }

    /// The verified alignment directory of `vendor`.
    pub(crate) fn verified_vendor(vendor: &str) -> PathBuf {
        repo_root()
            .join("tests/fixtures/signals/verified")
            .join(vendor)
    }

    pub(crate) fn read_layout_labels(profile: &str) -> Vec<String> {
        let text = std::fs::read_to_string(verified_vendor(profile).join("slot-map.json"))
            .expect("verified slot map");
        let map: serde_json::Value = serde_json::from_str(&text).expect("slot map json");
        map.as_object()
            .expect("layout labels")
            .keys()
            .cloned()
            .collect()
    }

    /// The analysts' recorded field orders (`tests/fixtures/signals/field-orders.json`).
    pub(crate) fn field_orders() -> serde_json::Value {
        let path = repo_root().join("tests/fixtures/signals/field-orders.json");
        serde_json::from_str(&std::fs::read_to_string(path).expect("field-orders.json"))
            .expect("json")
    }

    /// The verified schema of seed `label` (`emoji-channel-sample-halves-match-media-layout`), when present.
    pub(crate) fn vendor_schema(vendor: &str, label: &str) -> Option<serde_json::Value> {
        let text = std::fs::read_to_string(
            verified_vendor(vendor)
                .join("schemas")
                .join(format!("{label}.json")),
        )
        .ok()?;
        serde_json::from_str(&text).ok()
    }

    /// The frozen program of seed `label` (`<signal-order>-layout`), when the frozen
    /// corpus is present.
    pub(crate) fn vendor_program(vendor: &str, label: &str) -> Option<PathBuf> {
        let schema = vendor_schema(vendor, label)?;
        let seed = schema["seed"].as_str()?;
        // Corpus directory names are operational data; the verified seed is the identity.
        std::fs::read_dir(frozen_root())
            .ok()?
            .filter_map(Result::ok)
            .map(|entry| entry.path().join("programs").join(format!("{seed}.js")))
            .find(|path| path.is_file())
    }

    fn identity_map(vendor: &str) -> Option<serde_json::Value> {
        let text = std::fs::read_to_string(verified_vendor(vendor).join("slot-map.json")).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Expected canonical identity per slot address of seed `label` (test
    /// truth from the verified alignment; recognition never reads it).
    /// Empty when the map or the seed is missing.
    pub(crate) fn vendor_identities(vendor: &str, label: &str) -> BTreeMap<String, String> {
        let Some(map) = identity_map(vendor) else {
            return BTreeMap::new();
        };
        map[label]
            .as_object()
            .map(|slots| {
                slots
                    .iter()
                    .map(|(address, identity)| {
                        let name = identity.as_str().expect("signal identity");
                        assert!(
                            crate::signals::catalog::card(name).is_some(),
                            "unknown signal {name}"
                        );
                        (address.clone(), name.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Every name listed in an `order` array of a rendered signal.
    pub(crate) fn collect_order_names(value: &serde_json::Value, out: &mut Vec<String>) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, child) in map {
                    if key == "order" {
                        out.extend(
                            child
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter_map(|n| n.as_str().map(str::to_string)),
                        );
                    } else {
                        collect_order_names(child, out);
                    }
                }
            }
            serde_json::Value::Array(array) => array
                .iter()
                .for_each(|child| collect_order_names(child, out)),
            _ => {}
        }
    }
}

// ===========================================================================
// Selection and per-seed checks
// ===========================================================================

/// The twelve vendors of the frozen corpus.
const VENDORS: &[&str] = &[
    "emoji-channel-sample-halves-match-media-layouts",
    "nested-frame-anchor-property-names-hash-incognito-storage-probe-layouts",
    "web-gl-render-hash-vm-source-integrity-layouts",
    "emoji-glyph-rgba-samples-match-media-layouts",
    "canvas-readback-probe-builtin-probe-checksum-layouts",
    "web-gl-render-hash-navigator-plugins-layouts",
    "console-member-nativeness-hash-pair-host-environment-layouts",
    "match-media-lifecycle-phase-timings-layouts",
    "feature-policy-builtin-probe-checksum-layouts",
    "console-stack-access-flags-match-media-layouts",
    "clean-console-stack-access-flags-proxy-tamper-flags-layouts",
    "emoji-channel-samples-feature-policy-layouts",
];

/// Close pairs and triples: a slot of one member is claimed by that member
/// alone, never by another member of its group.
const CLOSE_PAIRS: &[&[&str]] = &[
    // Property census.
    &["AnchorProperties", "NestedFrameAnchorPropertyNamesHash"],
    &[
        "NamespacePropertyNameHashes",
        "IframeRealmPropertyNameHashes",
    ],
    &[
        "ConsolePropertyNamesHash",
        "ConsoleMemberNativenessHashPair",
    ],
    &[
        "GlobalOwnPropertyBits",
        "GlobalPropertyChecks",
        "GlobalPropertiesCount",
    ],
    // Static values.
    &["HistoricDateToString", "ShuffledDateString"],
    &["HistoricDateToString", "ConstantEmptyString"],
    &["ShuffledDateString", "ConstantEmptyString"],
    &["LocationUrlComponents", "DocumentReferrerHistory"],
    // Interaction and timing.
    &["UiEventSamplesWheel", "VisibilityEventLog"],
    &["KeyboardEventSamples", "KeystrokeDynamics"],
    &["TimestampsEpoch", "LifecyclePhaseTimings"],
    &["TimestampsEpoch", "NavigationTimingOffsets"],
    &["LifecyclePhaseTimings", "NavigationTimingOffsets"],
    &["LocalStoragePersistentId", "PersistentStorageState"],
    &["UiEventSamplesMouse", "MouseTravelAndClickCount"],
];

/// The signals under check: every registered signal, or the comma-separated
/// canonical names of `SHAPE_SIGNAL`.
fn selected_signals() -> BTreeSet<&'static str> {
    let registered: BTreeSet<&'static str> = SIGNALS.iter().map(|entry| entry.canonical).collect();
    match std::env::var("SHAPE_SIGNAL") {
        Ok(list) => {
            let wanted: BTreeSet<&'static str> = registered
                .iter()
                .copied()
                .filter(|name| list.split(',').any(|want| want.trim() == *name))
                .collect();
            assert!(
                !wanted.is_empty(),
                "SHAPE_SIGNAL={list}: no registered signal"
            );
            wanted
        }
        Err(_) => registered,
    }
}

/// The frozen layouts under check: verified labels for every selected profile
/// (`SHAPE_VENDORS`) and comma-separated layout names selected by `SHAPE_SEEDS`
/// (default `baseline`; `all` selects every layout).
fn selected_seeds() -> Vec<(&'static str, String)> {
    let vendors: Vec<&'static str> = match std::env::var("SHAPE_VENDORS") {
        Ok(list) => VENDORS
            .iter()
            .copied()
            .filter(|vendor| list.split(',').any(|v| v.trim() == *vendor))
            .collect(),
        Err(_) => VENDORS.to_vec(),
    };
    let requested = std::env::var("SHAPE_SEEDS").unwrap_or_else(|_| "baseline".into());
    vendors
        .into_iter()
        .flat_map(|vendor| {
            let baseline = format!("{}-layout", vendor.trim_end_matches("-layouts"));
            let labels = testing::read_layout_labels(vendor);
            labels
                .into_iter()
                .filter(|label| {
                    requested.split(',').any(|want| {
                        let want = want.trim();
                        want == "all" || want == label || (want == "baseline" && *label == baseline)
                    })
                })
                .map(move |label| (vendor, label))
                .collect::<Vec<_>>()
        })
        .collect()
}

static ANALYSES: Mutex<BTreeMap<String, Arc<SignalAnalysis>>> = Mutex::new(BTreeMap::new());

/// The analysis of one frozen program, computed once per test process.
fn analysis_of(label: &str, program: &Path) -> Arc<SignalAnalysis> {
    let mut cache = ANALYSES.lock().unwrap_or_else(|poison| poison.into_inner());
    cache
        .entry(label.to_string())
        .or_insert_with(|| {
            let source = std::fs::read_to_string(program).expect("program");
            Arc::new(
                analyze_program_signals(&source, &SignalFinderOptions::default())
                    .expect("analysis"),
            )
        })
        .clone()
}

/// Per-signal counts over the checked seeds.
#[derive(Default, Debug)]
struct SignalTally {
    present: usize,
    ok: usize,
    refused: usize,
    missed: usize,
    wrong: usize,
}

/// What one seed check found.
#[derive(Default, Debug)]
struct SeedCheck {
    /// Wrong or ambiguous claims, emitted names off the card, proof mismatches.
    wrong: Vec<String>,
    /// Field orders that disagree with the analysis evidence.
    misordered: Vec<String>,
    /// Close-pair slots not claimed by exactly their own member.
    pairs: Vec<String>,
    /// Bound fields confirmed against the verified proof text.
    proof_fields: usize,
}

/// Check the selected signals on one verified seed against its identities:
/// every slot of a selected signal is claimed by that signal alone (`ok`, or
/// `refused` when the binder declined), no selected signal claims another
/// slot or shares a slot (`wrong`), every emitted field name is on the card,
/// field orders agree with the analysis evidence and proof text, and close
/// pairs stay apart. `None` when the seed or its frozen program is missing.
fn check_seed(
    label: &str,
    vendor: &str,
    selected: &BTreeSet<&'static str>,
    per_signal: &mut BTreeMap<&'static str, SignalTally>,
) -> Option<SeedCheck> {
    let program = testing::vendor_program(vendor, label)?;
    let expected = testing::vendor_identities(vendor, label);
    if expected.is_empty() {
        return None;
    }
    let schema = testing::vendor_schema(vendor, label).unwrap_or_default();
    let analysis = analysis_of(label, &program);
    let mut check = SeedCheck::default();
    for slot in &analysis.slots {
        let want = expected
            .get(&slot.address)
            .map(String::as_str)
            .unwrap_or("-");
        let ours: Vec<&str> = slot
            .candidates
            .iter()
            .copied()
            .filter(|name| selected.contains(name))
            .collect();
        let want_ours = selected.get(want).copied();
        if want_ours.is_none() && ours.is_empty() {
            continue;
        }
        let refused = analysis
            .report
            .diagnostics
            .refused
            .iter()
            .find(|diagnostic| diagnostic.address == slot.address)
            .map(|diagnostic| diagnostic.reason.clone());
        let verdict = match (want_ours, ours.as_slice()) {
            (Some(_), [one]) if *one == want && slot.candidates.len() == 1 => {
                if slot.emitted_as.is_some() {
                    "ok"
                } else {
                    "refused"
                }
            }
            (Some(_), []) => "missed",
            _ => "WRONG",
        };
        if let Some(signal) = want_ours {
            let entry = per_signal.entry(signal).or_default();
            entry.present += 1;
            match verdict {
                "ok" => entry.ok += 1,
                "refused" => entry.refused += 1,
                "missed" => entry.missed += 1,
                _ => entry.wrong += 1,
            }
        }
        if verdict == "WRONG" {
            check.wrong.push(format!(
                "{label} {} want={want} candidates={:?}",
                slot.address, slot.candidates
            ));
        }
        eprintln!(
            "{label:<13} {:<12} want={want:<36} claimed={ours:?} emitted={:?} {verdict}{}",
            slot.address,
            slot.emitted_as,
            refused
                .map(|reason| format!(" ({reason})"))
                .unwrap_or_default()
        );
        for group in CLOSE_PAIRS {
            if !group.iter().any(|member| selected.contains(member)) {
                continue;
            }
            let claimed: Vec<&str> = slot
                .candidates
                .iter()
                .copied()
                .filter(|name| group.contains(name))
                .collect();
            if (group.contains(&want) || !claimed.is_empty()) && claimed != [want] {
                check.pairs.push(format!(
                    "{label} {}: close pair {group:?} want={want} claimed={claimed:?}",
                    slot.address
                ));
            }
        }
    }
    for slot in &analysis.slots {
        let (Some(key), [canonical]) = (&slot.emitted_as, slot.candidates.as_slice()) else {
            continue;
        };
        if !selected.contains(canonical) {
            continue;
        }
        let rendered = &analysis.report.signals[key.as_str()];
        let card = testing::card_field_names(canonical);
        let mut names = Vec::new();
        testing::collect_order_names(rendered, &mut names);
        eprintln!(
            "{label:<13} {:<12} {key}: {}",
            slot.address,
            names.join(" ")
        );
        for name in names {
            if !card.contains(&name) {
                check.wrong.push(format!(
                    "{label} {} {key}: field {name} is not on the {canonical} card",
                    slot.address
                ));
            }
        }
        let proof = &schema["signals"][slot.address.as_str()];
        let (checked, bad) = check_against_proof(canonical, rendered, proof);
        check.proof_fields += checked;
        for message in bad {
            check
                .wrong
                .push(format!("{label} {} {key}: {message}", slot.address));
        }
        let order = emitted_order(rendered);
        if let Some((_, names)) = PATH_CARD_NAMES
            .iter()
            .find(|(signal, _)| signal == canonical)
        {
            if let Some(permutations) = proof_permutations(canonical, label) {
                let expected: Vec<Vec<String>> = permutations
                    .iter()
                    .map(|permutation| {
                        permutation
                            .iter()
                            .map(|index| names[*index].to_string())
                            .collect()
                    })
                    .collect();
                if !expected.contains(&order) {
                    check.misordered.push(format!(
                        "{label} {} {canonical}: {order:?} vs proof {expected:?}",
                        slot.address
                    ));
                }
            }
        }
        if *canonical == "ConsoleMemberNativenessHashPair" {
            if let Some(recorded) = console_pair_order(label) {
                let first = match recorded.as_str() {
                    "nonnative_first" => "nonNativeNamesHash",
                    _ => "nativeNamesHash",
                };
                if order.first().map(String::as_str) != Some(first) {
                    check.misordered.push(format!(
                        "{label} {} {canonical}: {order:?} vs analysis {recorded}",
                        slot.address
                    ));
                }
            }
        }
    }
    for refused in &analysis.report.diagnostics.refused {
        if refused
            .candidates
            .iter()
            .any(|name| selected.contains(name.as_str()))
        {
            eprintln!(
                "{label:<13} refused {} {:?}: {}",
                refused.address, refused.candidates, refused.reason
            );
        }
    }
    Some(check)
}

/// Run [`check_seed`] over `seeds`; print the per-signal tally.
fn check_seeds(seeds: &[(&str, String)]) -> Vec<SeedCheck> {
    let selected = selected_signals();
    let mut per_signal = BTreeMap::new();
    let mut checks = Vec::new();
    for (vendor, label) in seeds {
        match check_seed(label, vendor, &selected, &mut per_signal) {
            Some(check) => checks.push(check),
            None => eprintln!(
                "skip {label}: verified identity or frozen program not found under {}",
                testing::frozen_root().display()
            ),
        }
    }
    for (signal, tally) in &per_signal {
        eprintln!(
            "  {signal:<36} present={} ok={} refused={} missed={} wrong={}",
            tally.present, tally.ok, tally.refused, tally.missed, tally.wrong
        );
    }
    let proof: usize = checks.iter().map(|check| check.proof_fields).sum();
    eprintln!(
        "seeds={} bound fields confirmed against verified proof text: {proof}",
        checks.len()
    );
    checks
}

// ===========================================================================
// Field-order evidence (test-only truth; recognition never reads it)
// ===========================================================================

/// The path index order of both namespace path tables (analysis
/// `NamespacePropertyNameHashes` section 1; identical in every build).
const PATH_CARD_NAMES: &[(&str, [&str; 5])] = &[
    (
        "NamespacePropertyNameHashes",
        [
            "navigatorProtoHash",
            "canvas2dProtoHash",
            "chromeAppHash",
            "safariHash",
            "applePaySetupProtoHash",
        ],
    ),
    (
        "IframeRealmPropertyNameHashes",
        [
            "navigatorHash",
            "canvasContext2dHash",
            "chromeAppHash",
            "safariHash",
            "applePaySetupHash",
        ],
    ),
];

/// The proof field permutations of a namespace path-table signal for one
/// seed (the analysis evidence's
/// `proof.computation_evidence.details.field_permutations`; `None` when not
/// recorded).
fn proof_permutations(signal: &str, seed: &str) -> Option<Vec<Vec<usize>>> {
    serde_json::from_value(testing::field_orders()[signal][seed].clone()).ok()
}

/// `field_order` (`nonnative_first` / `other_first`) recorded for one seed
/// in the ConsoleMemberNativenessHashPair validation.
fn console_pair_order(seed: &str) -> Option<String> {
    testing::field_orders()["ConsoleMemberNativenessHashPair"][seed]
        .as_str()
        .map(str::to_string)
}

/// The top-level `order` names of a rendered signal.
fn emitted_order(signal: &serde_json::Value) -> Vec<String> {
    signal["order"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|name| name.as_str().map(str::to_string))
        .collect()
}

/// Proof-text keywords of top-level card fields (test-only check of the
/// binders against the verified proofs' resolved write expressions).
const PROOF_KEYWORDS: &[(&str, &str, &str)] = &[
    ("DeviceMemory", "deviceMemory", "deviceMemory"),
    ("NavigatorPlugins", "pluginsLength", "plugins"),
    ("ShuffledDateString", "shuffledDateString", "toString"),
    ("WorkerRealmProfile", "origin", "location.origin"),
    ("WorkerRealmProfile", "userAgent", "navigator.userAgent"),
    ("WorkerRealmProfile", "platform", "navigator.platform"),
    ("WorkerRealmProfile", "globalPropertyCount", "(self)"),
    ("FeaturePolicy", "featureListHash", ".join("),
    ("FeaturePolicy", "allowedFeatureCount", "allowedFeatures"),
    ("FeaturePolicy", "featureCount", ".features"),
    ("FeaturePolicy", "samplingSeed", "< 1"),
    ("LocationUrlComponents", "hostname", "location.hostname"),
    ("LocationUrlComponents", "href", "location.href"),
    ("LocationUrlComponents", "protocol", "location.protocol"),
    ("LocationUrlComponents", "port", "location.port"),
    ("DocumentReferrerHistory", "historyLength", "history.length"),
    ("DocumentReferrerHistory", "referrerEmpty", "referrer ==="),
    (
        "DocumentReferrerHistory",
        "referrerPrefix",
        "referrer.substring",
    ),
    ("OuterArgsGlobalProcess", "outerArgsJson", "outerArgs"),
    ("UiEventIsTrusted", "isTrusted", "isTrusted"),
];

/// All resolved/condition text under one verified grammar item.
fn proof_text(item: &serde_json::Value, out: &mut String) {
    match item {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                match (key.as_str(), value) {
                    ("resolved" | "condition" | "value", serde_json::Value::String(text)) => {
                        out.push_str(text);
                        out.push('\n');
                    }
                    (_, serde_json::Value::Array(_) | serde_json::Value::Object(_))
                        if key != "mask_evidence" =>
                    {
                        proof_text(value, out)
                    }
                    _ => {}
                }
            }
        }
        serde_json::Value::Array(items) => items.iter().for_each(|item| proof_text(item, out)),
        _ => {}
    }
}

/// Compare a rendered single-message signal's top-level names with the
/// verified proof's write expressions, field by field. Returns the number
/// of confirmed fields and the mismatches. A field whose proof text holds
/// no keyword of the signal (an aliased or opaque proof) is not checked.
fn check_against_proof(
    canonical: &str,
    rendered: &serde_json::Value,
    proof: &serde_json::Value,
) -> (usize, Vec<String>) {
    let keywords: Vec<(&str, &str)> = PROOF_KEYWORDS
        .iter()
        .filter(|(signal, _, _)| *signal == canonical)
        .map(|(_, name, keyword)| (*name, *keyword))
        .collect();
    let Some(order) = rendered["order"].as_array() else {
        return (0, Vec::new());
    };
    let Some(items) = proof["emissions"][0]["grammar"]["items"].as_array() else {
        return (0, Vec::new());
    };
    let items: Vec<&serde_json::Value> = items
        .iter()
        .filter(|item| item["op"] != "raw_prefix")
        .collect();
    if keywords.is_empty() || items.len() != order.len() {
        return (0, Vec::new());
    }
    let mut checked = 0;
    let mut bad = Vec::new();
    for (name, item) in order.iter().zip(items) {
        let name = name.as_str().unwrap_or_default();
        let mut text = String::new();
        proof_text(item, &mut text);
        let hits: Vec<&str> = keywords
            .iter()
            .filter(|(_, keyword)| text.contains(keyword))
            .map(|(field, _)| *field)
            .collect();
        if hits.is_empty() {
            continue;
        }
        if hits.contains(&name) {
            checked += 1;
        } else {
            bad.push(format!(
                "field {name} but proof text names {hits:?}: {}",
                text.replace('\n', " | ")
            ));
        }
    }
    (checked, bad)
}

// ===========================================================================
// Tests
// ===========================================================================

#[test]
fn registry_matches_canonical_names() {
    let registered: Vec<&str> = SIGNALS.iter().map(|entry| entry.canonical).collect();
    let unique: BTreeSet<&str> = registered.iter().copied().collect();
    assert_eq!(unique.len(), registered.len(), "duplicate registry entries");
    assert_eq!(SIGNALS.len(), 93);
    for entry in SIGNALS {
        assert_eq!(
            entry.card.signal, entry.canonical,
            "card of {}",
            entry.canonical
        );
    }
}

/// The analysis helper modules recognizers and binders build on; they obey
/// the same structural rules as the catalog.
const RECOGNITION_HELPERS: &[&str] = &[
    "buffer_writes.rs",
    "census_probes.rs",
    "described_fields.rs",
    "field_solver.rs",
    "flag_bytes.rs",
    "layout_nodes.rs",
    "leaf_checks.rs",
    "native_checks.rs",
    "property_census.rs",
    "render_probes.rs",
    "shape_text.rs",
    "slot_closures.rs",
    "value_facts.rs",
];

/// Recognizer, binder and flag sources never reach for capture-local join
/// facts or generated spellings: the signal catalog (one file per signal),
/// the flag decoders and the analysis helper modules they use.
#[test]
fn catalog_sources_stay_structural() {
    fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                rust_files(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/signals");
    let mut paths = Vec::new();
    rust_files(&root.join("catalog"), &mut paths);
    rust_files(&root.join("flags"), &mut paths);
    for helper in RECOGNITION_HELPERS {
        let path = root.join("analysis").join(helper);
        assert!(path.exists(), "{}", path.display());
        paths.push(path);
    }
    paths.sort();
    assert!(
        paths.len() >= 93 + 1 + 4 + RECOGNITION_HELPERS.len(),
        "catalog, flags and helper sources: {}",
        paths.len()
    );
    let sources: Vec<(String, String)> = paths
        .iter()
        .map(|p| {
            let name = p.strip_prefix(&root).unwrap().display().to_string();
            (name, std::fs::read_to_string(p).unwrap())
        })
        .collect();
    const FORBIDDEN: &[&str] = &[
        ".descriptor",
        ".address(",
        ".contract(",
        ".salt",
        "mask_source",
        "$KEY_",
        "$THREAD_",
        "signal-names.json",
        "slot-map",
        ".routing",
        "Routing::",
    ];
    for (file, text) in &sources {
        // Comments may mention the rules.
        for (number, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            for forbidden in FORBIDDEN {
                assert!(
                    !code.contains(forbidden),
                    "{file}:{}: recognition must not read `{forbidden}`",
                    number + 1
                );
            }
        }
    }
}

/// The selected signals on the frozen emoji-channel sample-halves layout: no wrong claim, every
/// emitted field name on its card. `SHAPE_SIGNAL=<Name>` narrows the check;
/// skips when the frozen corpus (`SHAPE_FROZEN_ROOT`) is absent.
#[test]
#[ignore = "needs the frozen corpus-ten corpus"]
fn signals_on_frozen_emoji_channel_sample_halves_layout() {
    let checks = check_seeds(&[(
        "emoji-channel-sample-halves-match-media-layouts",
        "emoji-channel-sample-halves-match-media-layout".to_string(),
    )]);
    let wrong: Vec<&String> = checks.iter().flat_map(|check| &check.wrong).collect();
    assert!(
        wrong.is_empty(),
        "wrong claims or names on emoji-channel sample-halves layout:\n{}",
        wrong
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The selected signals on one verified seed per vendor (`SHAPE_VENDORS`,
/// `SHAPE_SEEDS`, `SHAPE_SIGNAL` narrow it): every slot claimed by its own
/// signal alone or missed, no wrong or ambiguous claim, every emitted field
/// name on its card, bound fields agreeing with the verified proof text and
/// field-order evidence, and close pairs kept apart.
#[test]
#[ignore = "needs the frozen corpus-ten corpus"]
fn signals_on_vendor_seeds() {
    let checks = check_seeds(&selected_seeds());
    let wrong: Vec<&String> = checks
        .iter()
        .flat_map(|check| {
            check
                .wrong
                .iter()
                .chain(&check.misordered)
                .chain(&check.pairs)
        })
        .collect();
    assert!(
        wrong.is_empty(),
        "wrong claims, names, orders or close pairs:\n{}",
        wrong
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Close pairs on the vendor seeds: every slot of a pair member is claimed
/// by that member alone, and the order evidence of the pair members holds.
#[test]
#[ignore = "needs the frozen corpus-ten corpus"]
fn close_pairs_on_vendor_seeds() {
    let checks = check_seeds(&selected_seeds());
    let failures: Vec<&String> = checks
        .iter()
        .flat_map(|check| check.pairs.iter().chain(&check.misordered))
        .collect();
    assert!(
        failures.is_empty(),
        "close pairs not separated:\n{}",
        failures
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}
