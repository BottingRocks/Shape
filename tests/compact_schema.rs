//! Public export invariants, independent of private capture data.
use serde_json::{json, Value};
use shape::signals::wire::SchemaDocument;

/// The verified, full-metadata decode these tests were written against.
const FULL: shape::signals::decode::DecodeOptions = shape::signals::decode::DecodeOptions {
    verify: true,
    metadata: true,
};

#[test]
fn allowlist_routes_xors_masks_tags_and_null_shapes() {
    let schema: SchemaDocument = serde_json::from_value(json!({
        "source":{"sha256":"unused","bytes":0}, "diagnostics":{"secret":"must not leak"},
        "signals": {
            "Named":{"address":"direct:7","type":"name","name":"real_source_key","mask":0,
                "order":["a","b","c","d","nullValue"],"sourcePlan":"must not leak",
                "fields":{
                    "a":{"type":"number","mask":null,"maskMode":"inherit","xor":0},
                    "b":{"type":"number","mask":null,"maskMode":"none"},
                    "c":{"type":"number","mask":123,"maskMode":"nested","xor":17},
                    "d":{"type":"choice","optional":true,"nullable":true,"arms":[
                        {"type":"byte","byte":255,"logicalType":"null","valueMappings":[
                            {"emission":0,"mapping":{"cases":[{"byte":255,"value":null}],"source":{"start":0,"end":1},"condition":"secret"}},
                            {"emission":1,"mapping":{"cases":[{"byte":255,"value":null}],"source":{"start":2,"end":3},"condition":"another secret"}}
                        ]}, {"type":"empty"}]},
                    "nullValue":{"type":"record","logicalType":"null","order":["marker","flag"],"fields":{
                        "marker":{"type":"byte","byte":130},"flag":{"type":"byte","byte":1}}}
                }},
            "Keyed":{"address":"direct:8","type":"name","name":18446744073709551615u64,"fields":{},"order":[]},
            "Position":{"address":"wave:0:0","type":"positional","positional":9,"mask":null,"fields":{},"order":[]}
        }
    })).unwrap();
    let public = serde_json::to_value(schema.compact_schema().unwrap()).unwrap();
    assert_eq!(public["Named"]["key"], "real_source_key");
    assert_eq!(public["Keyed"]["key"], json!(u64::MAX));
    assert_eq!(public["Position"]["positionalSignal"], 9);
    assert!(public["Position"].get("xor").is_none());
    assert_eq!(public["Named"]["xor"], 0);
    let f = &public["Named"]["fields"];
    assert_eq!(f["a"], json!({"type":"number","xor":0}));
    assert_eq!(f["b"], json!({"type":"number","maskMode":"none"}));
    assert_eq!(
        f["c"],
        json!({"type":"number","maskMode":"nested","mask":123,"xor":17})
    );
    assert_eq!(f["d"]["values"], json!({"null":255}));
    assert_eq!(f["d"]["optional"], true);
    assert_eq!(f["d"]["nullable"], true);
    assert_eq!(f["nullValue"]["type"], "null");
    assert_eq!(
        f["nullValue"]["values"]["null"],
        json!([{"byte":130},{"byte":1}])
    );
    let text = serde_json::to_string(&public).unwrap();
    for forbidden in [
        "secret",
        "address",
        "sourcePlan",
        "condition",
        "diagnostics",
        "\"source\":",
        "wave:",
    ] {
        assert!(!text.contains(forbidden), "{forbidden}");
    }
    assert!(SchemaDocument::from_json(&serde_json::to_vec(&public).unwrap()).is_err());
}

/// Opt-in private regression: never changes the input artifact or capture.
#[test]
fn private_capture_export_covers_every_descriptor_and_roundtrips() {
    let Some(root) = std::env::var_os("SHAPE_COMPACT_CAPTURE") else {
        return;
    };
    shape::run_with_large_stack(|| {
        use shape::signals::decode::{CaptureHeaders, DecodeArtifact};
        let root = std::path::PathBuf::from(root);
        let bytes = std::fs::read(root.join("artifact.json")).unwrap();
        let artifact = DecodeArtifact::from_json(&bytes).unwrap();
        let document = serde_json::to_value(artifact.compact_schema().unwrap()).unwrap();
        let public = &document["signals"];
        assert_eq!(public.as_object().unwrap().len(), 77);
        for (name, source) in &artifact.schema.signals {
            let source = serde_json::to_value(source).unwrap();
            let target = &public[name];
            assert_eq!(target["positionalSignal"], source["positional"]);
            assert_eq!(
                target["key"],
                source.get("name").cloned().unwrap_or(Value::Null)
            );
        }
        for slot in &artifact.schema.wire.as_ref().unwrap().slots {
            let Some(n) = &slot.naming else {
                continue;
            };
            for part in &n.parts {
                let signal = &public[&n.schema_key];
                let m = signal.get("messages").map_or(signal, |m| &m[part.message]);
                assert!(
                    m.get("emissions").is_none() && part.emission == 0 && part.part == 0
                        || m["emissions"]
                            .as_array()
                            .is_some_and(|e| e.contains(&json!([part.emission, part.part])))
                );
                for field in &part.fields {
                    if let shape::signals::wire::named::Node::FixedArray { items } = &field.node {
                        assert_eq!(
                            m["fields"][&field.name]["items"].as_array().unwrap().len(),
                            items.len()
                        );
                        let paths = m["writeOrder"].as_array().unwrap();
                        for (i, item) in items.iter().enumerate() {
                            let shape::signals::wire::named::Node::Leaf { leaf, .. } = item.node
                            else {
                                panic!()
                            };
                            let index = part.part_leaves.iter().position(|l| l == &leaf).unwrap();
                            assert_eq!(paths[index], json!([field.name, i]));
                        }
                    }
                }
            }
        }
        assert!(public["ShuffledDateString"]["fields"].get("zero").is_some());
        let capture: CaptureHeaders =
            serde_json::from_slice(&std::fs::read(root.join("headers.json")).unwrap()).unwrap();
        let decoded = artifact.decode_with(&capture, None, FULL).unwrap();
        assert_eq!(decoded.metadata.decoded, 75);
        assert!(decoded.metadata.exact_reencode);
        let old: Value =
            serde_json::from_slice(&std::fs::read(root.join("decoded.json")).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(&decoded).unwrap(), old);
        let mut slim = serde_json::to_vec_pretty(&decoded.positional_signals).unwrap();
        slim.push(b'\n');
        assert_eq!(
            serde_json::from_slice::<Value>(&slim).unwrap(),
            serde_json::from_slice::<Value>(&std::fs::read(root.join("signals.json")).unwrap())
                .unwrap()
        );
        assert_eq!(
            decoded.positional_signals["MatchMedia"]
                .values()
                .filter(|v| v.as_object().is_some_and(|o| o.is_empty()))
                .count(),
            20
        );
        assert_eq!(std::fs::read(root.join("artifact.json")).unwrap(), bytes);
    });
}

#[test]
fn logical_values_preserve_variants_multibyte_nulls_and_per_tag_masks() {
    let mapping = |emission, yes, no| json!({"emission":emission,"mapping":{"cases":[{"byte":yes,"value":true},{"byte":no,"value":false}]}});
    let schema:SchemaDocument=serde_json::from_value(json!({"source":{"sha256":"unused","bytes":0},"diagnostics":{},"signals":{"Example":{"address":"direct:0","type":"positional","positional":8,"mask":17,"order":["flag","variant","nothing"],"fields":{
        "flag":{"type":"choice","logicalType":"boolean","arms":[{"type":"byte","byte":79,"xor":3,"maskMode":"none"},{"type":"byte","byte":78}],"valueMappings":[mapping(0,79,78)]},
        "variant":{"type":"byte","logicalType":"boolean","valueMappings":[mapping(0,1,2),mapping(1,3,4)]},
        "ambiguous":{"type":"choice","logicalType":"boolean","arms":[{"type":"byte","byte":79,"xor":3},{"type":"byte","byte":79,"xor":4},{"type":"byte","byte":78}],"valueMappings":[mapping(0,79,78)]},
        "nothing":{"type":"choice","nullable":true,"arms":[
            {"type":"record","logicalType":"null","order":["a","b"],"fields":{"a":{"type":"byte","byte":162},"b":{"type":"byte","byte":1,"maskMode":"none","xor":9}}},
            {"type":"record","logicalType":"null","order":["a","b"],"fields":{"a":{"type":"byte","byte":161},"b":{"type":"byte","byte":1}}}
        ]}
    }}}})).unwrap();
    let compact = serde_json::to_value(schema.compact_schema().unwrap()).unwrap();
    let f = &compact["Example"]["fields"];
    assert_eq!(
        f["flag"],
        json!({"type":"boolean","values":{"true":{"byte":79,"xor":3,"maskMode":"none"},"false":78}})
    );
    assert_eq!(
        f["variant"]["variants"],
        json!([
            {"emissions":[0],"values":{"true":1,"false":2}},
            {"emissions":[1],"values":{"true":3,"false":4}}
        ])
    );
    assert_eq!(
        f["nothing"]["cases"][0]["values"]["null"],
        json!([{"byte":162},{"byte":1,"maskMode":"none","xor":9}])
    );
    assert_eq!(
        f["nothing"]["cases"][1]["values"]["null"],
        json!([{"byte":161},{"byte":1}])
    );
    assert_eq!(f["ambiguous"]["cases"][0]["xor"], 3);
    assert_eq!(f["ambiguous"]["cases"][1]["xor"], 4);
    assert_eq!(f["ambiguous"]["values"], json!({"true":79,"false":78}));
    let text = serde_json::to_string(&compact).unwrap();
    for forbidden in ["arms", "logicalType", "valueMappings", "tags", "source"] {
        assert!(!text.contains(forbidden), "{forbidden}");
    }
}

/// Independent source-audit oracle, opt-in because capture data is private.
#[test]
fn private_counted_capture_matches_every_audited_migration_and_physical_baseline() {
    let Some(root) = std::env::var_os("SHAPE_COUNTED_CAPTURE") else {
        return;
    };
    shape::run_with_large_stack(|| {
        use shape::signals::decode::{CaptureHeaders, DecodeArtifact};
        let root = std::path::PathBuf::from(root);
        let read = |p: &str| -> Value {
            serde_json::from_slice(&std::fs::read(root.join(p)).unwrap()).unwrap()
        };
        let old = read("decoded.json");
        let new = read("counted-arrays/decoded.json");
        let inventory = read("count-field-audit/inventory.json");
        fn normalize(v: &Value, signal: &str, path: &str, inventory: &Value) -> Value {
            match v {
                Value::Array(items) => Value::Array(
                    items
                        .iter()
                        .map(|v| normalize(v, signal, &format!("{path}[]"), inventory))
                        .collect(),
                ),
                Value::Object(fields) => {
                    let mut result: serde_json::Map<String, Value> = fields
                        .iter()
                        .map(|(k, v)| {
                            (
                                k.clone(),
                                normalize(
                                    v,
                                    signal,
                                    &if path.is_empty() {
                                        k.clone()
                                    } else {
                                        format!("{path}.{k}")
                                    },
                                    inventory,
                                ),
                            )
                        })
                        .collect();
                    for c in inventory["counts"].as_array().unwrap().iter().filter(|c| {
                        c["signal"] == signal
                            && c["classification"]
                                .as_str()
                                .unwrap()
                                .starts_with("safe_hide")
                    }) {
                        let count_path = c["countPath"].as_str().unwrap();
                        let parent = count_path.rsplit_once('.').map_or("", |(p, _)| p);
                        if parent != path {
                            continue;
                        }
                        let count = c["countFieldName"].as_str().unwrap();
                        let array = c["arrayFieldName"].as_str().unwrap();
                        if !fields.contains_key(count) && !fields.contains_key(array) {
                            continue;
                        }
                        let length = fields[array].as_array().unwrap().len();
                        assert_eq!(fields[count], json!(length), "{signal}.{count_path}");
                        result.remove(count);
                        if c["wrapper"] == true {
                            assert_eq!(fields.len(), 2);
                            return result.remove(array).unwrap();
                        }
                    }
                    Value::Object(result)
                }
                _ => v.clone(),
            }
        }
        let expected: serde_json::Map<String, Value> = old["positionalSignals"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, v)| (name.clone(), normalize(v, name, "", &inventory)))
            .collect();
        assert_eq!(new["positionalSignals"], Value::Object(expected));
        for (key, value) in old.as_object().unwrap() {
            if key != "positionalSignals" && key != "metadata" {
                assert_eq!(&new[key], value, "{key}");
            }
        }
        for (key, value) in old["metadata"].as_object().unwrap() {
            if key != "signals" {
                assert_eq!(&new["metadata"][key], value, "{key}");
            }
        }
        for (name, signal) in old["metadata"]["signals"].as_object().unwrap() {
            let actual = &new["metadata"]["signals"][name];
            assert_eq!(
                actual["fields"],
                normalize(&signal["fields"], name, "", &inventory)
            );
            assert_eq!(actual["status"], signal["status"]);
            let mut actual = actual["metadata"].clone();
            actual["projection"]
                .as_object_mut()
                .unwrap()
                .remove("countedArrays");
            for candidate in actual["projectionCandidates"].as_array_mut().unwrap() {
                candidate["fields"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|v| v.get("countLeaf").is_none());
            }
            assert_eq!(
                actual, signal["metadata"],
                "all original bits, bytes, controls and leaf paths: {name}"
            );
        }
        let baseline = read("artifact.json");
        let mut fresh = read("counted-arrays/artifact.json");
        for slot in fresh["schema"]["wire"]["slots"].as_array_mut().unwrap() {
            slot["naming"]
                .as_object_mut()
                .unwrap()
                .remove("countedArrays");
        }
        assert_eq!(
            baseline, fresh,
            "physical programs, contracts, descriptions, source and session are unchanged"
        );
        let artifact = DecodeArtifact::from_json(
            &std::fs::read(root.join("counted-arrays/artifact.json")).unwrap(),
        )
        .unwrap();
        let capture: CaptureHeaders = serde_json::from_value(read("headers.json")).unwrap();
        assert_eq!(
            serde_json::to_value(artifact.decode_with(&capture, None, FULL).unwrap()).unwrap(),
            new
        );
        let document = serde_json::to_value(artifact.compact_schema().unwrap()).unwrap();
        let schema = &document["signals"];
        assert_eq!(*schema, read("counted-arrays/schema.json"));
        assert_eq!(schema.as_object().unwrap().len(), 77);
        fn arrays(
            v: &Value,
            signal: &str,
            path: &str,
            found: &mut std::collections::BTreeSet<(String, String)>,
        ) {
            if v["type"] == "array" {
                if v.get("count").is_some() {
                    assert_eq!(v["count"]["type"], "number");
                    assert!(v["count"]["position"].is_u64());
                    found.insert((signal.into(), path.into()));
                }
                arrays(&v["item"], signal, &format!("{path}[]"), found);
            }
            if let Some(fields) = v["fields"].as_object() {
                for (name, v) in fields {
                    arrays(
                        v,
                        signal,
                        &if path.is_empty() {
                            name.clone()
                        } else {
                            format!("{path}.{name}")
                        },
                        found,
                    );
                }
            }
            for key in ["messages", "cases"] {
                if let Some(cases) = v[key].as_array() {
                    for v in cases {
                        arrays(v, signal, path, found);
                    }
                }
            }
        }
        let mut found = std::collections::BTreeSet::new();
        for (name, v) in schema.as_object().unwrap() {
            arrays(v, name, "", &mut found);
        }
        let expected: std::collections::BTreeSet<_> = inventory["counts"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| {
                c["classification"]
                    .as_str()
                    .unwrap()
                    .starts_with("safe_hide")
            })
            .map(|c| {
                let path = c["arrayPath"].as_str().unwrap();
                (
                    c["signal"].as_str().unwrap().to_owned(),
                    if c["wrapper"] == true {
                        path.rsplit_once('.').unwrap().0.to_owned()
                    } else {
                        path.to_owned()
                    },
                )
            })
            .collect();
        assert_eq!(found,expected,"every audited hidden length has compact NUMBER encoding metadata, including initializer-dependent recipes");
        assert_eq!(found.len(), 35);
        for (name, s) in &artifact.schema.signals {
            let raw = serde_json::to_value(s).unwrap();
            assert_eq!(schema[name]["positionalSignal"], raw["positional"]);
            assert_eq!(schema[name]["key"], raw["name"]);
        }
        assert_eq!(
            schema["UiEventIsTrusted"]["fields"]["isTrusted"],
            json!({"type":"boolean","values":{"true":79,"false":78}})
        );
        let p = &new["positionalSignals"];
        assert_eq!(p["WebRtcIceCandidateAddresses"], json!({"candidates":[]}));
        assert_eq!(p["UiEventSamplesTouch"], json!({"records":[]}));
        for key in ["brands", "fullVersionList"] {
            assert_eq!(p["UserAgentData"][key].as_array().unwrap().len(), 3);
        }
        assert_eq!(
            p["WebGlContext"]["shaderPrecisions"]
                .as_array()
                .unwrap()
                .len(),
            12
        );
        for key in ["shaderPrecisions", "supportedExtensions"] {
            assert!(p["WebGlContext"][key]
                .as_array()
                .unwrap()
                .iter()
                .all(Value::is_string));
        }
        assert_eq!(p["AnimationFrameTimings"]["timeQuantum"], 1);
        assert!(p["AnimationFrameTimings"].get("sampleCount").is_none());
        assert_eq!(
            p["MatchMedia"]
                .as_object()
                .unwrap()
                .values()
                .filter(|v| **v == json!({}))
                .count(),
            20
        );
        assert!(p["ShuffledDateString"].get("zero").is_some());
        assert!(
            std::fs::metadata(root.join("counted-arrays/schema.json"))
                .unwrap()
                .len()
                < 227051
        );
    });
}

#[test]
fn seed_export_keeps_all_typed_routes_order_keysets_and_opaque_entries() {
    use shape::signals::{
        decode::{DecodeArtifact, COMPACT_SEED_SCHEMA_FORMAT},
        payload::extract::SeedLayout,
        wire::FloatByteOrder,
    };
    let mut layout: SeedLayout =
        serde_json::from_str(include_str!("fixtures/generator/seed-pinned-layout.json")).unwrap();
    // Export preserves even empty/repeated lists and keys verbatim; it does not
    // normalize or validate them as part of describing an in-memory layout.
    layout.keysets = vec![vec![], vec!["z".into(), "a".into(), "z".into()], vec![]];
    layout.report = serde_json::from_value(json!({"errors":[],
        "positionalSignals":[{"slot":0,"descriptor":8,"ctor":true,"priority":false}],
        "signals":[{"descriptor":19,"name":"17"},{"descriptor":13,"name":"opaque"},{"descriptor":19,"name":"17"}],
        "keyedSignals":[{"descriptor":4,"name":17},{"descriptor":3,"name":4294967295u32}]
    })).unwrap();
    let schema: SchemaDocument = serde_json::from_value(json!({
        "source":{"sha256":"excluded","bytes":1},"diagnostics":{"secret":"excluded"},
        "signals":{
            "String":{"address":"direct:19","type":"name","name":"17","order":[],"fields":{}},
            "Number":{"address":"direct:4","type":"name","name":17,"order":[],"fields":{}},
            "WrongDescriptor":{"address":"direct:1","type":"name","name":"17","order":[],"fields":{}},
            "Absent":{"address":"direct:0","type":"name","name":"absent","order":[],"fields":{}},
            "Position":{"address":"wave:0:0","type":"positional","positional":0,"order":[],"fields":{}}
        }
    })).unwrap();
    let mut artifact = DecodeArtifact {
        direct: false,
        format: shape::signals::decode::ARTIFACT_FORMAT.into(),
        site_id: "site".into(),
        layout,
        schema,
        float_byte_order: FloatByteOrder::Big,
    };
    let public = serde_json::to_value(artifact.compact_schema().unwrap()).unwrap();
    assert_eq!(public["format"], COMPACT_SEED_SCHEMA_FORMAT);
    assert_eq!(public["keysets"], json!(artifact.layout.keysets));
    let routes = &public["reportRoutes"];
    assert_eq!(
        routes["positionalSignals"],
        json!([{"positionalSignal":0,"descriptor":8,"ctor":true,"priority":false,"contracts":["Position"]}])
    );
    assert_eq!(
        routes["signals"],
        json!([
            {"key":"17","descriptor":19,"contracts":["String"]},
            {"key":"opaque","descriptor":13,"contracts":[]},
            {"key":"17","descriptor":19,"contracts":["String"]}
        ])
    );
    assert_eq!(
        routes["keyedSignals"],
        json!([
            {"key":17,"descriptor":4,"contracts":["Number"]},
            {"key":4294967295u32,"descriptor":3,"contracts":[]}
        ])
    );
    assert_eq!(
        public["unroutedSignals"],
        json!(["Absent", "WrongDescriptor"])
    );
    assert_eq!(
        public["signals"],
        serde_json::to_value(artifact.schema.compact_schema().unwrap()).unwrap()
    );
    assert_eq!(public["encoding"]["cipher"], json!(artifact.layout.cipher));
    assert!(public.get("siteId").is_none());
    assert_eq!(public["seedId"], artifact.layout.header.c);
    assert_eq!(
        public["encoding"]["header"]["c"],
        json!({"$ref":"#/seedId"})
    );
    let mut resolved_header = public["encoding"]["header"].clone();
    resolved_header["c"] = public["seedId"].clone();
    assert_eq!(resolved_header, json!(artifact.layout.header));
    assert_eq!(public["encoding"]["float"]["callerByteOrder"], "big");
    assert_eq!(public["encoding"]["entry"]["missing"], Value::Null);
    assert_eq!(
        public["encoding"]["entry"]["shape"],
        json!(["values", "extra"])
    );
    // Changing every session-only value must leave the public document identical.
    artifact.layout.session = serde_json::from_value(json!({"alphabet":"private alphabet",
        "transactionToken":"private token","keys":[1,2,3,4,5,6,7,8],"shuffleKey":[99],"nonce":"private nonce"})).unwrap();
    artifact.site_id = "another caller identity".into();
    assert_eq!(
        public,
        serde_json::to_value(artifact.compact_schema().unwrap()).unwrap()
    );
    let text = serde_json::to_string(&public).unwrap();
    for key in [
        "source",
        "address",
        "wire",
        "programs",
        "evidence",
        "diagnostics",
        "session",
        "sourcePlan",
    ] {
        assert!(!public.as_object().unwrap().contains_key(key));
    }
    for forbidden in [
        "private token",
        "private alphabet",
        "private nonce",
        "excluded",
        "keysetIndex",
        "\"xor\"",
    ] {
        assert!(!text.contains(forbidden));
    }
    // Route existence is source layout, not capture presence. Empty arrays
    // describe absent routes; a null entry belongs to a runtime report only.
    artifact.layout.report.signals.clear();
    artifact.layout.report.keyed_signals.clear();
    let empty = serde_json::to_value(artifact.compact_schema().unwrap()).unwrap();
    assert_eq!(empty["reportRoutes"]["signals"], json!([]));
    assert_eq!(empty["reportRoutes"]["keyedSignals"], json!([]));
    assert_eq!(
        empty["unroutedSignals"],
        json!(["Absent", "Number", "String", "WrongDescriptor"])
    );
}

/// Opt-in verification of fresh CLI exports. All payload JSON stays in Rust,
/// preserving arbitrary precision integers and the original IEEE metadata.
#[test]
fn private_fresh_seed_schema_inventory() {
    let Some(root) = std::env::var_os("SHAPE_SEED_SCHEMA_CAPTURE") else {
        return;
    };
    shape::run_with_large_stack(|| {
        use sha2::{Digest, Sha256};
        use shape::signals::decode::{CaptureHeaders, DecodeArtifact, DecodedPayload};
        let root = std::path::PathBuf::from(root);
        let out = root.join("seed-schema");
        let read = |path: &std::path::Path| -> Value {
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
        };
        let bytes = std::fs::read(out.join("artifact.json")).unwrap();
        let artifact = DecodeArtifact::from_json(&bytes).unwrap();
        let document = read(&out.join("schema.json"));
        assert_eq!(
            document,
            serde_json::to_value(artifact.compact_schema().unwrap()).unwrap()
        );
        assert_eq!(
            std::fs::read(out.join("schema.json")).unwrap(),
            std::fs::read(out.join("reload-schema.json")).unwrap()
        );
        assert_eq!(
            std::fs::read(out.join("decoded.json")).unwrap(),
            std::fs::read(out.join("reload-decoded.json")).unwrap()
        );
        let baseline =
            DecodeArtifact::from_json(&std::fs::read(root.join("artifact.json")).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(&artifact).unwrap(),
            serde_json::to_value(&baseline).unwrap()
        );
        assert_eq!(
            document["signals"],
            serde_json::to_value(baseline.schema.compact_schema().unwrap()).unwrap()
        );
        assert_eq!(document["signals"], read(&root.join("schema.json")));
        assert_eq!(
            read(&out.join("decoded.json")),
            read(&root.join("decoded.json"))
        );
        let capture: CaptureHeaders =
            serde_json::from_slice(&std::fs::read(root.join("headers.json")).unwrap()).unwrap();
        let decoded: DecodedPayload =
            serde_json::from_slice(&std::fs::read(out.join("decoded.json")).unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(artifact.decode_with(&capture, None, FULL).unwrap()).unwrap(),
            serde_json::to_value(&decoded).unwrap()
        );
        assert_eq!(decoded.metadata.decoded, 75);
        assert!(decoded.metadata.exact_reencode);
        let expanded = capture.expanded(&artifact.layout.header.prefix).unwrap();
        assert_eq!(
            artifact
                .layout
                .generator()
                .unwrap()
                .encode_report_headers(&decoded.metadata.original_report)
                .unwrap(),
            expanded
        );
        let original = artifact.layout.decode_headers(&expanded).unwrap();
        assert_eq!(original, decoded.metadata.original_report);
        let mut slim = serde_json::to_vec_pretty(&decoded.positional_signals).unwrap();
        slim.push(b'\n');
        let original_slim = std::fs::read(root.join("signals.json")).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&slim).unwrap(),
            read(&root.join("signals.json"))
        );
        // Keep the approved JSON presentation after verifying freshly decoded
        // values, including empty arrays. Never normalize it through JS numbers.
        std::fs::write(out.join("signals.json"), original_slim).unwrap();
        assert_eq!(
            decoded.positional_signals["MatchMedia"]
                .values()
                .filter(|v| v.as_object().is_some_and(|o| o.is_empty()))
                .count(),
            20
        );
        assert!(document["signals"]["ShuffledDateString"]["fields"]
            .get("zero")
            .is_some());
        let layout = serde_json::to_value(&artifact.layout).unwrap();
        let mut inventory = vec![];
        for channel in ["positionalSignals", "signals", "keyedSignals"] {
            let original = layout["report"][channel].as_array().unwrap();
            let exported = document["reportRoutes"][channel].as_array().unwrap();
            assert_eq!(original.len(), exported.len());
            for (index, (before, after)) in original.iter().zip(exported).enumerate() {
                let mut comparable = after.clone();
                let map = comparable.as_object_mut().unwrap();
                map.remove("contracts");
                if channel == "positionalSignals" {
                    let slot = map.remove("positionalSignal").unwrap();
                    map.insert("slot".into(), slot);
                } else {
                    let name = map.remove("key").unwrap();
                    map.insert("name".into(), name);
                }
                assert_eq!(*before, comparable);
                inventory.push(json!({"channel":channel,"index":index,"original":before,"exported":after,"exact":true}));
            }
        }
        assert_eq!(document["keysets"], layout["keysets"]);
        assert_eq!(document["encoding"]["cipher"], layout["cipher"]);
        assert_eq!(document["encoding"]["header"], layout["header"]);
        assert_eq!(
            document["encoding"]["float"]["callerByteOrder"],
            json!(artifact.float_byte_order)
        );
        assert_eq!(document["unroutedSignals"], json!([]));
        fn count_arrays(v: &Value) -> usize {
            match v {
                Value::Object(o) => {
                    usize::from(o.get("type") == Some(&json!("array")) && o.contains_key("count"))
                        + o.values().map(count_arrays).sum::<usize>()
                }
                Value::Array(a) => a.iter().map(count_arrays).sum(),
                _ => 0,
            }
        }
        // One logical counted path appears in multiple alternatives; retain
        // all 36 annotations for the 35 approved hidden count paths.
        assert_eq!(count_arrays(&document["signals"]), 36);
        fn counted_paths(v: &Value, path: &str, paths: &mut std::collections::BTreeSet<String>) {
            if v["type"] == "array" && v.get("count").is_some() {
                paths.insert(path.into());
            }
            if let Some(fields) = v["fields"].as_object() {
                for (name, value) in fields {
                    counted_paths(value, &format!("{path}.{name}"), paths);
                }
            }
            if let Some(item) = v.get("item") {
                counted_paths(item, &format!("{path}[]"), paths);
            }
            for key in ["cases", "messages"] {
                if let Some(variants) = v[key].as_array() {
                    for value in variants {
                        counted_paths(value, path, paths);
                    }
                }
            }
        }
        let mut paths = std::collections::BTreeSet::new();
        for (name, value) in document["signals"].as_object().unwrap() {
            counted_paths(value, name, &mut paths);
        }
        assert_eq!(paths.len(), 35);
        // Allowlist boundary: inspect structural keys recursively, not strings
        // such as external input names that intentionally describe prerequisites.
        fn no_internals(v: &Value) {
            match v {
                Value::Object(o) => {
                    for (k, v) in o {
                        assert!(
                            ![
                                "source",
                                "sourcePlan",
                                "wire",
                                "programs",
                                "evidence",
                                "diagnostics",
                                "address",
                                "contract",
                                "grammar",
                                "writer",
                                "nativeDependencies",
                                "provenance"
                            ]
                            .contains(&k.as_str()),
                            "{k}"
                        );
                        no_internals(v);
                    }
                }
                Value::Array(a) => {
                    for v in a {
                        no_internals(v)
                    }
                }
                _ => {}
            }
        }
        no_internals(&document);
        let baseline_hashes: serde_json::Map<String, Value> = [
            "program.js",
            "headers.json",
            "session.json",
            "artifact.json",
            "decoded.json",
            "signals.json",
            "schema.json",
        ]
        .into_iter()
        .map(|name| {
            (
                name.into(),
                json!(Sha256::digest(std::fs::read(root.join(name)).unwrap())
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>()),
            )
        })
        .collect();
        let verification = json!({
            "format":"shape-seed-schema-verification/1", "baselineSha256":baseline_hashes,
            "schemaBytes":std::fs::metadata(out.join("schema.json")).unwrap().len(),
            "counts":{"logicalContracts":document["signals"].as_object().unwrap().len(),"positionalRoutes":artifact.layout.report.positional_signals.len(),"namedRoutes":artifact.layout.report.signals.len(),"keyedRoutes":artifact.layout.report.keyed_signals.len(),"keysets":artifact.layout.keysets.len(),"presentDecoded":decoded.metadata.decoded,"missingEntries":decoded.metadata.missing,"hiddenCounts":paths.len(),"countAnnotations":count_arrays(&document["signals"]),"retainedPackedCounts":21},
            "countedPaths":paths,
            "checks":{"freshArtifactEqualsBaseline":true,"collectorContractsUnchanged":true,"decodedUnchanged":true,"slimBytesUnchanged":true,"artifactOnlySchemaByteIdentical":true,"artifactOnlyDecodedByteIdentical":true,"originalReportExact":true,"allHeadersByteIdentical":true,"allPresentSignalsExactReencode":true,"keysetsExact":true,"cipherExact":true,"headerPolicyExact":true,"floatChoiceExact":true,"noCompilerOrSessionData":true},
            "keysets":{"original":layout["keysets"],"exported":document["keysets"]},
            "routes":inventory,"unroutedSignals":document["unroutedSignals"],
            "limits":["This capture has no named or numeric keyed routes and no omitted keysets; populated cases are fixture-tested, not capture observations.","Compact descriptors are descriptive, not an executable codec.","Session values and per-message entropy remain external.","Source software float fallback is not certified; native path requires caller byte order and runtime prerequisites."]
        });
        let mut bytes = serde_json::to_vec_pretty(&verification).unwrap();
        bytes.push(b'\n');
        std::fs::write(out.join("verification.json"), bytes).unwrap();
    });
}
