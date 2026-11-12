//! Optional frozen-source regression; no network or runtime capture is required.
use serde_json::{json, Value};
use shape::signals::{
    decode::CompactSeedSchema,
    payload::{extract::pin_source, report::Report, superpack::Value as Raw},
};
use std::{fs, path::Path, process::Command};

fn public_fields(v: &Value, top: bool) -> Value {
    let mut out = serde_json::Map::new();
    for key in ["order", "key"] {
        if let Some(value) = v.get(key) {
            out.insert(key.into(), value.clone());
        }
    }
    if !top {
        out.insert("type".into(), v["type"].clone());
    }
    if let Some(fields) = v["fields"].as_object() {
        out.insert(
            "fields".into(),
            fields
                .iter()
                .map(|(k, v)| (k.clone(), public_fields(v, false)))
                .collect(),
        );
    }
    if let Some(messages) = v["messages"].as_array() {
        out.insert(
            "messages".into(),
            messages.iter().map(|v| public_fields(v, true)).collect(),
        );
    }
    if top && v.get("name").is_some() {
        out.insert("key".into(), v["name"].clone());
    }
    Value::Object(out)
}
fn pair(key: &str, value: Raw) -> Raw {
    Raw::Array(vec![Raw::String(key.into()), value])
}

/// Exercises all four channels, typed keys, duplicates, raw payloads, missing
/// versus null versus empty containers, and the frozen sources' field objects.
fn constructed_report(schema: &Value) {
    let names = &schema["signals"];
    let mut entries = vec![];
    for name in ["ViewportDimensions", "SelectMetaInjectionFlags"] {
        let contract = &names[name];
        let mut fields: Vec<(String, Raw)> = contract["order"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let field = &contract["fields"][f.as_str().unwrap()];
                assert_eq!(field["type"], "value");
                (
                    field["key"].as_str().unwrap().into(),
                    Raw::from((i + 1) as f64),
                )
            })
            .collect();
        // Generic superpack sorts object keys; construct its canonical object
        // representation while keeping source producer order in the schema.
        fields.sort_by(|(a, _), (b, _)| a.cmp(b));
        entries.push(pair(contract["key"].as_str().unwrap(), Raw::Object(fields)));
    }
    let cookie = names["CorsCookieValue"]["key"].as_str().unwrap();
    entries.push(pair(
        cookie,
        Raw::Object(vec![("value".into(), Raw::String("cookie".into()))]),
    ));
    entries.push(pair(cookie, Raw::Object(vec![])));
    entries.push(pair(
        names["SensorReadyEvent"]["key"].as_str().unwrap(),
        Raw::MissingSentinel,
    ));
    entries.extend([
        pair("unknown", Raw::Null),
        pair("unknown", Raw::MissingSentinel),
        pair("17", Raw::Array(vec![])),
        pair(
            "unknown",
            Raw::Object(vec![("raw".into(), Raw::Binary(vec![0, 255]))]),
        ),
    ]);
    let report = Report {
        errors: vec![
            Raw::Object(vec![
                ("message".into(), Raw::String("constructed".into())),
                ("unknown".into(), Raw::Bool(true)),
            ]),
            Raw::String("raw error".into()),
        ],
        positional_signals: vec![None; 80],
        signals: entries,
        keyed_signals: vec![
            (17, None),
            (
                17,
                Some((vec![Raw::Null, Raw::Object(vec![])], vec![0, 255, 1])),
            ),
            (u32::MAX, Some((vec![Raw::Array(vec![])], vec![]))),
        ],
        seeds: [0x12345678, 0xabcdef01],
        keysets: vec![],
    };
    let payload = report.to_payload().unwrap();
    let bytes = payload.plaintext().unwrap();
    let layout: shape::signals::payload::extract::SeedLayout =
        serde_json::from_str(include_str!("fixtures/generator/seed-pinned-layout.json")).unwrap();
    let codec = shape::signals::payload::PayloadCodec::new(layout.profile().into()).unwrap();
    let encoded = codec.encode_report(&report).unwrap();
    let decoded = codec.decode_report(&encoded, 80, vec![]).unwrap();
    assert_eq!(encoded, codec.encode_report(&decoded).unwrap());
    assert_eq!(report, decoded);
    assert_eq!(bytes, decoded.to_payload().unwrap().plaintext().unwrap());
    let saved = serde_json::to_vec(&report).unwrap();
    let reloaded: Report = serde_json::from_slice(&saved).unwrap();
    assert_eq!(report, reloaded);
}

#[test]
fn frozen_ten_sources_match_independent_census_and_named_contracts() {
    let Some(root) = std::env::var_os("SHAPE_FROZEN_INPUTS") else {
        return;
    };
    shape::run_with_large_stack(move || {
        use sha2::{Digest, Sha256};
        let root = Path::new(&root);
        let inventory: Value =
            serde_json::from_slice(&fs::read(root.join("audit-inventory.json")).unwrap()).unwrap();
        let output = root.parent().unwrap().join("frozen-keyed");
        fs::create_dir_all(output.join("schemas")).unwrap();
        let seeds = inventory["seeds"].as_array().unwrap();
        assert_eq!(seeds.len(), 10);
        let mut summaries = vec![];
        for seed in seeds {
            let id = seed["seedId"].as_str().unwrap();
            let source =
                fs::read_to_string(root.join("programs").join(format!("{id}.js"))).unwrap();
            let hash: String = Sha256::digest(source.as_bytes())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            assert_eq!(hash, seed["frozen_program"]["sha256"]);
            let routing = Command::new("gzip")
                .arg("-dc")
                .arg(
                    root.join("routing")
                        .join(format!("{}.json.gz", seed["label"].as_str().unwrap())),
                )
                .output()
                .unwrap();
            assert!(routing.status.success());
            let census: Value = serde_json::from_slice(&routing.stdout).unwrap();
            assert_eq!(hash, census["source_sha256"]);
            assert_eq!(id, census["seed"]);
            let facts = pin_source(&source).unwrap();
            assert_eq!(facts.source.sha256, hash);
            assert_eq!(facts.keysets, Vec::<Vec<String>>::new());
            assert_eq!(json!(facts.keysets), seed["ordered_omitted_keysets"]);
            assert_eq!(facts.report.positional_signals.len(), 80);
            assert_eq!(facts.report.signals.len(), 4);
            assert!(facts.report.keyed_signals.is_empty());
            for (i, slot) in facts.report.positional_signals.iter().enumerate() {
                assert_eq!(slot.slot, i);
                assert_eq!(slot.descriptor, i);
            }
            let schema = CompactSeedSchema::from_source(&source).unwrap();
            let document = serde_json::to_value(&schema).unwrap();
            assert_eq!(document["seedId"], id);
            assert_eq!(
                document["encoding"]["header"]["c"],
                json!({"$ref":"#/seedId"})
            );
            assert_eq!(
                document["reportRoutes"]["errors"]["value"],
                "superpack array"
            );
            let routes = &document["reportRoutes"];
            let entries = census["entries"].as_array().unwrap();
            assert_eq!(entries.len(), 84);
            for entry in entries {
                let descriptor = entry["descriptor"].as_u64().unwrap() as usize;
                let channel = if descriptor < 80 {
                    "positionalSignals"
                } else {
                    "signals"
                };
                let route = routes[channel]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["descriptor"] == descriptor)
                    .unwrap();
                if descriptor >= 80 {
                    assert_eq!(route["key"], entry["name"]);
                    assert_eq!(route["contracts"].as_array().unwrap().len(), 1);
                }
            }
            for name in [
                "ViewportDimensions",
                "SelectMetaInjectionFlags",
                "CorsCookieValue",
            ] {
                assert_eq!(
                    public_fields(&document["signals"][name], true),
                    public_fields(&seed["nonpositional_descriptions"][name], true),
                    "{id} {name}"
                );
            }
            let completion = &document["signals"]["SensorReadyEvent"];
            assert_eq!(completion["behavior"], "completion_only");
            assert_eq!(completion["payload"], "none");
            assert_eq!(
                completion["effect"],
                seed["nonpositional_descriptions"]["SensorReadyEvent"]["effect"]
            );
            assert!(
                completion.get("fields").is_none(),
                "completion is not an empty measurement"
            );
            assert_eq!(document["unroutedSignals"], json!([]));
            constructed_report(&document);
            let pretty = serde_json::to_vec_pretty(&schema).unwrap();
            let text = std::str::from_utf8(&pretty).unwrap();
            for forbidden in [
                "\"sourcePlan\"",
                "\"programs\"",
                "\"address\"",
                "\"diagnostics\"",
                "\"span\"",
                "\"alphabet\":",
                "\"keys\":",
                "\"transactionToken\":",
            ] {
                assert!(!text.contains(forbidden), "{forbidden}");
            }
            assert!(pretty.len() < 300_000, "compact schema: {}", pretty.len());
            fs::write(output.join("schemas").join(format!("{id}.json")), &pretty).unwrap();
            summaries.push(json!({"seedId":id,"sourceSha256":hash,"registrations":84,"positional":80,"named":4,"numericKeyed":0,"namedPayloadContracts":3,"completionOnly":1,"keysets":[],"prettyBytes":pretty.len(),"sourceCensusMatch":true,"constructedReportRoundtrip":true}));
        }
        fs::write(
            output.join("inventory.json"),
            serde_json::to_vec_pretty(&summaries).unwrap(),
        )
        .unwrap();
    });
}

#[test]
fn supplied_target_capture_retains_approved_fields_and_exact_headers() {
    let Some(root) = std::env::var_os("SHAPE_TARGET_CAPTURE") else {
        return;
    };
    shape::run_with_large_stack(move || {
        use shape::signals::decode::{CaptureHeaders, DecodeArtifact};
        let root = Path::new(&root);
        let read = |file| fs::read(root.join(file)).unwrap();
        let original = DecodeArtifact::from_json(&read("artifact.json")).unwrap();
        let capture: CaptureHeaders = serde_json::from_slice(&read("headers.json")).unwrap();
        let baseline: Value = serde_json::from_slice(&read("decoded.json")).unwrap();
        let source = String::from_utf8(read("program.js")).unwrap();
        let session = serde_json::from_slice(&read("session.json")).unwrap();
        let fresh = DecodeArtifact::compile(
            &source,
            &session,
            original.site_id.clone(),
            original.float_byte_order,
        )
        .unwrap();
        let decoded = fresh.decode(&capture, None).unwrap();
        assert_eq!(decoded.metadata.decoded, 75);
        assert!(decoded.metadata.exact_reencode);
        let actual = serde_json::to_value(&decoded).unwrap();
        for key in [
            "positionalSignals",
            "errors",
            "signals",
            "keyedSignals",
            "seeds",
            "keysets",
        ] {
            assert_eq!(actual[key], baseline[key], "{key}");
        }
        let source_schema =
            serde_json::to_value(CompactSeedSchema::from_source(&source).unwrap()).unwrap();
        let runtime_schema = serde_json::to_value(fresh.compact_schema().unwrap()).unwrap();
        assert_eq!(source_schema["signals"], runtime_schema["signals"]);
        assert_eq!(
            source_schema["signals"],
            serde_json::to_value(original.compact_schema().unwrap()).unwrap()["signals"]
        );
        assert_eq!(
            source_schema["reportRoutes"],
            runtime_schema["reportRoutes"]
        );
        let fields = &actual["positionalSignals"];
        assert!(fields["ShuffledDateString"].get("zero").is_some());
        assert_eq!(
            fields["MatchMedia"]
                .as_object()
                .unwrap()
                .values()
                .filter(|v| **v == json!({}))
                .count(),
            20
        );
    });
}
