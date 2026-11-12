//! Generic source and independent golden signal bytes exercise the capture boundary.
use serde_json::{json, Value};
use shape::signals::{
    decode::{CaptureHeaders, DecodeArtifact},
    payload::{extract::Session, superpack::Value as Raw, Headers, Report},
    wire::FloatByteOrder,
};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

/// The verified, full-metadata decode these tests were written against.
const FULL: shape::signals::decode::DecodeOptions = shape::signals::decode::DecodeOptions {
    verify: true,
    metadata: true,
};
fn source() -> String {
    let source = include_str!("fixtures/signal-wire-roundtrip/program.js")
        .replace(
            "__HASH__",
            &include_str!("../src/signals/wire/reference/hash.js").replace("local0(", "tokenHash("),
        )
        .replace(
            "__STREAM__",
            &include_str!("../src/signals/wire/reference/stream.js")
                .replace("local0(", "streamFactory(")
                .replace("local9", "tokenState"),
        );
    let writer = include_str!("../src/signals/wire/reference/number.js")
        .replace("local0(", "integerWriter(");
    source.replacen("function main() {", &format!("function main() {{\n{writer}"),1)
        .replace("function Session(token) {", "function Session(token) {\nlet sharedNumber; sharedNumber=integerWriter; const navigator={deviceMemory:9};")
        .replace("b[4]=9^mask();", "let callable; callable=sharedNumber; callable=callable(mask,navigator.deviceMemory,b,3); callable=0;")
}
fn session() -> Session {
    serde_json::from_value(json!({"transactionToken":"ab","alphabet":"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/","shuffleKey":[],"keys":[0,0,0,0,0,0,0,0]})).unwrap()
}
fn report() -> Report {
    Report {
        errors: vec![Raw::Null],
        positional_signals: vec![
            Some((vec![], vec![1, 2, 3, 4, 0xa3])),
            Some((vec![], vec![1, 2, 3, 4, 0xab])),
        ],
        signals: vec![Raw::Bool(true)],
        keyed_signals: vec![(17, None)],
        seeds: [19, 23],
        keysets: vec![],
    }
}
fn capture(artifact: &DecodeArtifact, report: &Report) -> CaptureHeaders {
    let headers = artifact
        .layout
        .encode_headers(&report.channels_owned(), report.seeds)
        .unwrap();
    CaptureHeaders {
        site_id: "example".into(),
        headers: Headers::from_entries(headers.iter().map(|(name, value)| {
            (
                name.strip_prefix(&artifact.layout.header.prefix)
                    .unwrap()
                    .to_owned(),
                value.to_owned(),
            )
        }))
        .unwrap(),
    }
}
fn compiled() -> DecodeArtifact {
    DecodeArtifact::compile(
        &source(),
        &session(),
        "example".into(),
        FloatByteOrder::Little,
    )
    .unwrap()
}

#[test]
fn packed_explicit_empty_else_extracts_compiles_loads_and_reads_exactly() {
    use shape::signals::wire::{Decoder, Outcome, Support, WireValue};
    shape::run_with_large_stack(|| {
        let source = source().replace(
            "let callable; callable=sharedNumber; callable=callable(mask,navigator.deviceMemory,b,3); callable=0;",
            "let items=[];let size=items.length;let pack=0;let bits=0;let scratch=0;let p=3;\
             p=integerWriter(mask,size,b,p);let i=size;for(;;){--i;if(!(i>=0))break;\
             bits|=1<<pack;++pack;if(8===pack){b[++p]=bits^mask();pack=0;bits=0;}\
             else{scratch=bits;}}if(pack!==0){b[++p]=bits^mask();}",
        );
        let artifact = DecodeArtifact::compile(
            &source,
            &session(),
            "example".into(),
            FloatByteOrder::Little,
        )
        .unwrap();
        assert!(!artifact.direct);
        let loaded = DecodeArtifact::from_json(&serde_json::to_vec(&artifact).unwrap()).unwrap();
        let wire = loaded.schema.wire.as_ref().unwrap();
        let slot_index = wire
            .slots
            .iter()
            .position(|s| {
                matches!(
                    s.routing,
                    shape::signals::wire::WireRoute::Positional { positional: 1 }
                )
            })
            .unwrap();
        let slot = &wire.slots[slot_index];
        assert!(matches!(slot.programs[0], Support::Supported { .. }));
        assert_eq!(slot.source_plan[0].counter_choices.len(), 2);
        let decoder = Decoder::load(
            &serde_json::to_vec(&loaded.schema).unwrap(),
            &source,
            &loaded.layout,
        )
        .unwrap();
        for length in [0usize, 1, 7, 8, 9, 16, 17] {
            let bytes = shape::signals::codec::encode_signal(
                shape::signals::codec::hash_string_token("ab"),
                123,
                17,
                |w| {
                    w.number(length as f64);
                    for _ in 0..length / 8 {
                        w.byte(255);
                    }
                    if length % 8 != 0 {
                        w.byte((1u16 << (length % 8)) as u8 - 1);
                    }
                },
            );
            let mut original = report();
            original.positional_signals[1].as_mut().unwrap().1 = bytes.clone();
            let decoded = decoder.decode_report(&original).unwrap();
            let Outcome::Decoded {
                value,
                exact_reencode: true,
            } = &decoded[slot_index].outcome
            else {
                panic!("{:?}", decoded[slot_index].outcome);
            };
            assert_eq!(decoder.encode_slot(slot_index, value, 123).unwrap(), bytes);
            let WireValue::Sequence(values) = value else {
                panic!("sequence")
            };
            let WireValue::Repeat(iterations) = &values[1] else {
                panic!("repeat")
            };
            assert_eq!(iterations.len(), length);
            // A non-flush iteration retains its actual empty arm; no tag or byte is invented.
            for (i, iteration) in iterations.iter().enumerate() {
                assert!(
                    matches!(iteration, WireValue::Choice { arm, .. } if *arm == usize::from((i+1)%8 != 0))
                );
            }
        }
        // Strict source loading must reject forged empty arms and leaf metadata.
        for mutation in 0..5 {
            let mut schema = serde_json::to_value(&loaded.schema).unwrap();
            let choice = &mut schema["wire"]["slots"][slot_index]["contract"]["emissions"][0]
                ["grammar"]["items"][2]["body"];
            match mutation {
                0 => choice["arms"] = json!([]),
                1 => choice["arms"][0]["body"] = json!({"op":"sequence","items":[]}),
                2 => choice["arms"][0]["body"]["mask"] = json!("unmasked"),
                3 => choice["arms"][0]["body"]["span"] = json!([0, 1]),
                _ => schema["wire"]["slots"][slot_index]["contract"]["emissions"][0]
                    ["grammar"]["items"][0]["elements"][0] = json!("seed >>> 16"),
            }
            assert!(Decoder::load(
                &serde_json::to_vec(&schema).unwrap(),
                &source,
                &loaded.layout
            )
            .is_err());
        }
    });
}

#[test]
fn sdk_generator_documents_defaults_and_input_errors_are_uniform() {
    let artifact = shape::run_with_large_stack(compiled);
    let generator = shape::sdk::Generator::load(&serde_json::to_vec(&artifact).unwrap()).unwrap();
    let input = serde_json::to_value(capture(&artifact, &report())).unwrap();
    let decoded = generator
        .call("decode", json!({"capture": input}))
        .unwrap();
    let from_documents = generator
        .call(
            "decode",
            json!({"capture": input.to_string(), "session": serde_json::to_string(&session()).unwrap()}),
        )
        .unwrap();
    assert_eq!(decoded, from_documents);
    assert_eq!(decoded["metadata"]["exactReencode"], false);
    let headers = generator
        .call("encode", json!({"decoded": decoded.to_string()}))
        .unwrap();
    let again = generator.call("decode", json!({"capture": headers})).unwrap();
    assert_eq!(again["positionalSignals"], decoded["positionalSignals"]);

    for (method, request) in [
        ("decode", json!({"capture": {"siteId": "wrong-site", "headers": {}}})),
        ("encode", json!({"signals": []})),
        ("encode", json!({"decoded": {"siteId": "wrong-site"}})),
        ("explain", json!({"decoded": decoded, "signal": "UnknownSignal"})),
        ("schema", json!([])),
    ] {
        let error = generator.call(method, request.clone()).unwrap_err();
        assert_eq!(error.code, "invalid_request", "{method}: {error}");
        let envelope: Value =
            serde_json::from_str(&generator.call_json(method, &request.to_string())).unwrap();
        assert_eq!(envelope["error"]["code"], "invalid_request");
    }
}

#[test]
fn artifact_roundtrip_slim_fields_missing_and_fresh_session() {
    shape::run_with_large_stack(|| {
        let artifact = compiled();
        // Loading never takes a source path or reparses the source.
        let loaded = DecodeArtifact::from_json(&serde_json::to_vec(&artifact).unwrap()).unwrap();
        let original = report();
        let input = capture(&loaded, &original);
        let output = loaded.decode_with(&input, None, FULL).unwrap();
        assert_eq!(
            serde_json::to_value(&output.positional_signals).unwrap(),
            json!({
                "DeviceMemory":{"deviceMemory":9},"StorageAccessApiPresence":{"hasStorageAccess":true,"requestStorageAccess":false}
            })
        );
        assert_eq!(output.metadata.original_report.report(), original);
        assert_eq!(output.metadata.decoded, 2);
        assert_eq!(output.metadata.missing, 0);
        assert!(output.metadata.exact_reencode);
        assert_eq!(
            output.metadata.signals["StorageAccessApiPresence"].metadata["capturedSeed"],
            json!(0x01020304u32)
        );
        let mut missing = original.clone();
        missing.positional_signals[1] = None;
        let output = loaded.decode_with(&capture(&loaded, &missing), None, FULL).unwrap();
        assert_eq!(output.metadata.missing, 1);
        assert_eq!(output.positional_signals.len(), 1);
        assert!(!output.positional_signals.contains_key("DeviceMemory"));
        // Change outer encryption/session defaults without recompiling recipes.
        let mut fresh = session();
        fresh.keys = [7; 8];
        fresh.alphabet = fresh.alphabet.chars().rev().collect();
        fresh.transaction_token = "another-session".into();
        let mut fresh_artifact = loaded.clone();
        fresh_artifact.layout.session = fresh.clone();
        let mut fresh_report = original.clone();
        let token = shape::signals::codec::hash_string_token(&fresh.transaction_token);
        fresh_report.positional_signals[0].as_mut().unwrap().1 =
            shape::signals::codec::encode_signal(token, 0x01020304, 17, |w| w.byte(1));
        fresh_report.positional_signals[1].as_mut().unwrap().1 =
            shape::signals::codec::encode_signal(token, 0x01020304, 17, |w| w.number(9.0));
        let fresh_input = capture(&fresh_artifact, &fresh_report);
        assert!(loaded.decode_with(&fresh_input, None, FULL).is_err());
        let output = loaded.decode_with(&fresh_input, Some(&fresh), FULL).unwrap();
        assert_eq!(output.positional_signals.len(), 2);
        assert!(output.metadata.exact_reencode);
    });
}

#[test]
fn lazy_decode_matches_full_and_explains_on_demand() {
    use shape::signals::decode::{DecodeOptions, ValidatedArtifact};
    shape::run_with_large_stack(|| {
        let artifact = ValidatedArtifact::new(compiled()).unwrap();
        let input = capture(artifact.artifact(), &report());
        let full = artifact.decode(&input, None, FULL).unwrap();
        for verify in [false, true] {
            let lazy = artifact
                .decode(&input, None, DecodeOptions { verify, metadata: false })
                .unwrap();
            assert_eq!(lazy.positional_signals, full.positional_signals);
            assert!(lazy.metadata.signals.is_empty());
            assert_eq!(lazy.metadata.decoded, full.metadata.decoded);
            assert_eq!(lazy.metadata.exact_reencode, verify);
            assert_eq!(lazy.metadata.original_report, full.metadata.original_report);
            for (name, want) in &full.metadata.signals {
                let got = artifact.explain(&lazy, name, None, verify).unwrap();
                assert_eq!(
                    serde_json::to_value(&got).unwrap(),
                    serde_json::to_value(want).unwrap()
                );
            }
        }
        assert!(artifact.explain(&full, "NoSuchSignal", None, false).is_err());
        let mut missing = report();
        missing.positional_signals[1] = None;
        let lazy = artifact
            .decode(&capture(artifact.artifact(), &missing), None, DecodeOptions::default())
            .unwrap();
        assert!(artifact.explain(&lazy, "DeviceMemory", None, false).is_err());
    });
}

#[test]
fn numeric_header_chunks_and_native_float_order() {
    shape::run_with_large_stack(|| {
        let mut artifact = DecodeArtifact::compile(
            &source().replace("deviceMemory:9", "deviceMemory:1.5"),
            &session(),
            "example".into(),
            FloatByteOrder::Little,
        )
        .unwrap();
        artifact.layout.header.chunk_size = Some(8);
        let mut report = report();
        report.positional_signals[1].as_mut().unwrap().1 = shape::signals::codec::encode_signal(
            shape::signals::codec::hash_string_token("ab"),
            0x01020304,
            17,
            |w| w.number(1.5),
        );
        let input = capture(&artifact, &report);
        assert!(input.headers.get("a0").is_some());
        let result = artifact.decode_with(&input, None, FULL).unwrap();
        assert_eq!(
            result.positional_signals["DeviceMemory"]["deviceMemory"],
            json!(1.5)
        );
        artifact.float_byte_order = FloatByteOrder::Unknown;
        assert!(artifact.decode_with(&input, None, FULL).is_err());
    });
}

#[test]
fn prefix_is_source_derived_including_non_x_families() {
    shape::run_with_large_stack(|| {
        for prefix in ["e-build-", "q-build-"] {
            let artifact = DecodeArtifact::compile(
                &source().replace("x-example-", prefix),
                &session(),
                "example".into(),
                FloatByteOrder::Little,
            )
            .unwrap();
            assert_eq!(artifact.layout.header.prefix, prefix);
            let input = capture(&artifact, &report());
            let expanded = input.expanded(prefix).unwrap();
            assert!(expanded.get(&format!("{prefix}a")).is_some());
            assert!(expanded.get("x-example-a").is_none());
            let loaded =
                DecodeArtifact::from_json(&serde_json::to_vec(&artifact).unwrap()).unwrap();
            assert_eq!(loaded.decode_with(&input, None, FULL).unwrap().metadata.decoded, 2);
        }
    });
}

#[test]
fn typed_capture_and_artifact_reject_invalid_identity_and_routes() {
    shape::run_with_large_stack(|| {
        assert!(serde_json::from_str::<CaptureHeaders>(
            r#"{"siteId":"example","headers":{"a":"one","a":"two"}}"#
        )
        .is_err());
        assert!(serde_json::from_str::<CaptureHeaders>(
            r#"{"siteId":"example","headers":{"a":"one","A":"two"}}"#
        )
        .is_err());
        for headers in [
            json!({"authorization":"x"}),
            json!({"aa":"x"}),
            json!({"a":"line\nvalue"}),
        ] {
            let value = json!({"siteId":"example","headers":headers});
            let result = serde_json::from_value::<CaptureHeaders>(value);
            assert!(result.is_err() || result.unwrap().validate().is_err());
        }
        let artifact = compiled();
        let mut input = capture(&artifact, &report());
        input.site_id = "other".into();
        assert!(artifact
            .decode_with(&input, None, FULL)
            .unwrap_err()
            .to_string()
            .contains("siteId"));
        let mut bad = artifact.clone();
        bad.format = "unknown".into();
        assert!(bad.validate().is_err());
        let mut bad = artifact.clone();
        bad.schema.source.sha256 = "wrong".into();
        assert!(bad.validate().is_err());
        let mut bad = artifact.clone();
        let wire = bad.schema.wire.as_mut().unwrap();
        wire.slots[1].routing = wire.slots[0].routing.clone();
        assert!(bad.validate().is_err());
        let mut bad = artifact.clone();
        let wire = bad.schema.wire.as_mut().unwrap();
        wire.slots[1].naming.as_mut().unwrap().schema_key =
            wire.slots[0].naming.as_ref().unwrap().schema_key.clone();
        assert!(bad.validate().is_err());
        let mut invalid = report();
        invalid.positional_signals[0].as_mut().unwrap().1.push(77);
        assert!(artifact
            .decode_with(&capture(&artifact, &invalid), None, FULL)
            .is_err());
        let mut unknown = artifact.clone();
        unknown.schema.wire.as_mut().unwrap().slots[0].programs[0] =
            shape::signals::wire::Support::Unsupported {
                reason: "unproved encoding".into(),
                minimum_input: None,
            };
        assert!(unknown.decode_with(&capture(&unknown, &report()), None, FULL).is_err());
    });
}
#[test]
fn named_signals_encode_back_and_accept_edits() {
    use shape::signals::decode::ValidatedArtifact;
    shape::run_with_large_stack(|| {
        let artifact = ValidatedArtifact::new(compiled()).unwrap();
        let input = capture(artifact.artifact(), &report());
        let decoded = artifact.decode(&input, None, FULL).unwrap();
        let payload = serde_json::to_value(&decoded).unwrap();
        // Re-encoding the decoded capture reproduces the input bytes exactly.
        let encoded = artifact.encode_named(&payload, None).unwrap();
        assert_eq!(encoded.site_id, input.site_id);
        assert_eq!(
            encoded.headers.iter().collect::<Vec<_>>(),
            input.headers.iter().collect::<Vec<_>>()
        );

        let fields = decoded.positional_signals.clone();
        let mut edited = decoded.clone();
        edited
            .positional_signals
            .get_mut("DeviceMemory")
            .unwrap()
            .insert("deviceMemory".into(), json!(8));
        let encoded = artifact
            .encode_named(&serde_json::to_value(&edited).unwrap(), None)
            .unwrap();
        let decoded_edit = artifact.decode(&encoded, None, FULL).unwrap();
        assert_eq!(
            decoded_edit.positional_signals["DeviceMemory"]["deviceMemory"],
            json!(8)
        );
        assert_eq!(
            decoded_edit.positional_signals["StorageAccessApiPresence"],
            fields["StorageAccessApiPresence"]
        );

        // A float spelling of an integral value names the same wire value; the
        // decoder renders it as an integer, and the encoder compares it that way.
        edited
            .positional_signals
            .get_mut("DeviceMemory")
            .unwrap()
            .insert("deviceMemory".into(), json!(8.0));
        let encoded = artifact
            .encode_named(&serde_json::to_value(&edited).unwrap(), None)
            .unwrap();
        let decoded_edit = artifact.decode(&encoded, None, FULL).unwrap();
        assert_eq!(
            decoded_edit.positional_signals["DeviceMemory"]["deviceMemory"],
            json!(8)
        );

        // Bare fields carry no retained bytes; they need only decode back to
        // the same values under fresh prefixes.
        let bare = json!({"positionalSignals": fields});
        let encoded = artifact.encode_named(&bare, None).unwrap();
        let decoded_bare = artifact.decode(&encoded, None, FULL).unwrap();
        assert_eq!(decoded_bare.positional_signals, fields);
    });
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "shape-decode-workflow-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn run(&self, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_shape"))
            .current_dir(&self.0)
            .args(args)
            .output()
            .unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn cli_first_decode_then_artifact_only_and_safe_outputs() {
    shape::run_with_large_stack(|| {
        let temp = Temp::new();
        let artifact = compiled();
        assert_eq!(source(), include_str!("fixtures/sdk/decode/program.js"));
        assert_eq!(
            serde_json::to_value(session()).unwrap(),
            serde_json::from_str::<Value>(include_str!("fixtures/sdk/decode/session.json")).unwrap()
        );
        assert_eq!(
            serde_json::to_value(capture(&artifact, &report())).unwrap(),
            serde_json::from_str::<Value>(include_str!("fixtures/sdk/decode/headers.json")).unwrap()
        );
        fs::write(temp.0.join("program.js"), source()).unwrap();
        fs::write(
            temp.0.join("session.json"),
            serde_json::to_vec(&session()).unwrap(),
        )
        .unwrap();
        fs::write(
            temp.0.join("headers.json"),
            serde_json::to_vec(&capture(&artifact, &report())).unwrap(),
        )
        .unwrap();
        let args = [
            "decode",
            "--seed",
            "program.js",
            "--input-kind",
            "program",
            "--headers",
            "headers.json",
            "--session",
            "session.json",
            "--float-byte-order",
            "little",
            "--save-artifact",
            "artifact.json",
            "--schema-out",
            "schema.json",
            "--out",
            "decoded.json",
        ];
        let result = temp.run(&args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        fs::remove_file(temp.0.join("program.js")).unwrap();
        fs::remove_file(temp.0.join("session.json")).unwrap();
        let public: Value =
            serde_json::from_slice(&fs::read(temp.0.join("schema.json")).unwrap()).unwrap();
        assert_eq!(
            public,
            serde_json::to_value(artifact.compact_schema().unwrap()).unwrap()
        );
        assert!(DecodeArtifact::from_json(&fs::read(temp.0.join("schema.json")).unwrap()).is_err());
        assert!(shape::signals::wire::SchemaDocument::from_json(
            &fs::read(temp.0.join("schema.json")).unwrap()
        )
        .is_err());
        for schema_path in [
            "schema.json",
            "headers.json",
            "artifact.json",
            "decoded.json",
            "-",
        ] {
            let mut command = vec![
                "decode",
                "--artifact",
                "artifact.json",
                "--headers",
                "headers.json",
                "--out",
                "decoded.json",
                "--schema-out",
                schema_path,
            ];
            if schema_path != "schema.json" {
                command.push("--force");
            }
            assert!(!temp.run(&command).status.success(), "{schema_path}");
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                temp.0.join("headers.json"),
                temp.0.join("schema-link.json"),
            )
            .unwrap();
            assert!(!temp
                .run(&[
                    "decode",
                    "--artifact",
                    "artifact.json",
                    "--headers",
                    "headers.json",
                    "--schema-out",
                    "schema-link.json",
                    "--force"
                ])
                .status
                .success());
        }
        let forced = temp.run(&[
            "decode",
            "--artifact",
            "artifact.json",
            "--headers",
            "headers.json",
            "--schema-out",
            "schema.json",
            "--out",
            "decoded.json",
            "--force",
        ]);
        assert!(
            forced.status.success(),
            "{}",
            String::from_utf8_lossy(&forced.stderr)
        );
        let result = temp.run(&[
            "decode",
            "--artifact",
            "artifact.json",
            "--headers",
            "headers.json",
            // Compare like with like: decoded.json came from a verified run.
            "--verify",
            "--timings",
        ]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let reused: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert!(String::from_utf8_lossy(&result.stderr).contains("shape decode timings (ms):"));
        assert_eq!(
            reused,
            serde_json::from_slice::<Value>(&fs::read(temp.0.join("decoded.json")).unwrap())
                .unwrap()
        );
        let before = fs::read(temp.0.join("headers.json")).unwrap();
        for args in [
            vec![
                "decode",
                "--artifact",
                "artifact.json",
                "--headers",
                "headers.json",
                "--out",
                "headers.json",
                "--force",
            ],
            vec![
                "decode",
                "--artifact",
                "artifact.json",
                "--headers",
                "headers.json",
                "--out",
                "decoded.json",
            ],
            vec![
                "decode",
                "--artifact",
                "artifact.json",
                "--seed",
                "missing.js",
                "--headers",
                "headers.json",
            ],
        ] {
            assert!(!temp.run(&args).status.success());
        }
        assert_eq!(before, fs::read(temp.0.join("headers.json")).unwrap());
        fs::write(temp.0.join("program.js"), source()).unwrap();
        fs::write(
            temp.0.join("session.json"),
            serde_json::to_vec(&session()).unwrap(),
        )
        .unwrap();
        let mut invalid = report();
        invalid.positional_signals[0].as_mut().unwrap().1.push(77);
        fs::write(
            temp.0.join("invalid.json"),
            serde_json::to_vec(&capture(&artifact, &invalid)).unwrap(),
        )
        .unwrap();
        let failed = temp.run(&[
            "decode",
            "--seed",
            "program.js",
            "--input-kind",
            "program",
            "--session",
            "session.json",
            "--headers",
            "invalid.json",
            "--out",
            "failed.json",
            "--save-artifact",
            "failed-artifact.json",
            "--schema-out",
            "failed-schema.json",
        ]);
        assert!(!failed.status.success());
        assert!(!temp.0.join("failed.json").exists());
        assert!(!temp.0.join("failed-artifact.json").exists());
        assert!(!temp.0.join("failed-schema.json").exists());
    });
}

#[test]
fn borrowed_projection_preserves_unselected_emission_provenance() {
    use shape::signals::{codec::hash_string_token, wire::*};
    shape::run_with_large_stack(|| {
        let artifact = compiled();
        let wire = artifact.schema.wire.as_ref().unwrap();
        let mut slot = wire.slots[0].clone();
        let program = match &slot.programs[0] {
            Support::Supported { program } | Support::RequiresInitializer { program, .. } => {
                program.clone()
            }
            other => panic!("fixture recipe unsupported: {other:?}"),
        };
        // An unrelated conditional recipe must not become a supported metadata entry.
        slot.contract
            .emissions
            .push(slot.contract.emissions[0].clone());
        slot.programs.push(Support::RequiresInitializer {
            program: program.clone(),
            stream_body: wire.algorithms.stream.clone(),
            required_context:
                InitializationEpoch::SuppliedPublicInitializerWithoutInterveningInitialization,
        });
        let raw = report().positional_signals[0].as_ref().unwrap().1.clone();
        let token = hash_string_token("ab");
        let value =
            explicit_initializer::decode(&program, &raw, token, FloatByteOrder::Little).unwrap();
        let decoded = DecodedSlot {
            positional: 0,
            descriptor: slot.contract.descriptor,
            address: slot.address.clone(),
            captured_seed: Some(0x01020304),
            raw: Some(raw),
            outcome: Outcome::Decoded {
                value,
                exact_reencode: true,
            },
            selection: Some(Selection {
                emissions: vec![0],
                proof: SelectionProof::IdenticalPrograms,
                root_case: None,
            }),
        };
        let mut copied = slot.clone();
        copied.programs[0] = Support::Supported {
            program: program.clone(),
        };
        let before =
            explicit_initializer::project(&copied, &decoded, token, FloatByteOrder::Little);
        let after = explicit_initializer::project_with_program(
            &slot,
            &decoded,
            0,
            &program,
            token,
            FloatByteOrder::Little,
        );
        assert_eq!(
            serde_json::to_value(before).unwrap(),
            serde_json::to_value(&after).unwrap()
        );
        assert!(after.metadata["streamSalts"][1].is_null());
    });
}

#[test]
fn per_seed_boolean_schema_abstracts_tags_and_retains_exact_headers() {
    shape::run_with_large_stack(|| {
        for (yes, no) in [(31u8, 210u8), (0, 5)] {
            let source=source().replace("const document={hasStorageAccess:true};","const document=globalThis.document;")
                .replace("b[4]=((\"hasStorageAccess\" in document)|(\"requestStorageAccess\" in document)<<1)^mask();",
                    &format!("b[4]=(document.createEvent(\"UIEvent\").isTrusted ? {yes} : {no})^mask();"));
            let artifact = DecodeArtifact::compile(
                &source,
                &session(),
                "example".into(),
                FloatByteOrder::Little,
            )
            .unwrap();
            let schema = serde_json::to_value(&artifact.schema).unwrap();
            assert_eq!(
                schema["signals"]["UiEventIsTrusted"]["fields"]["isTrusted"]["logicalType"],
                json!("boolean")
            );
            let loaded =
                DecodeArtifact::from_json(&serde_json::to_vec(&artifact).unwrap()).unwrap();
            let slot_index = loaded
                .schema
                .wire
                .as_ref()
                .unwrap()
                .slots
                .iter()
                .position(|s| {
                    s.naming
                        .as_ref()
                        .is_some_and(|n| n.canonical_name == "UiEventIsTrusted")
                })
                .unwrap();
            let mut forged = serde_json::to_value(&artifact).unwrap();
            let mapping = &mut forged["schema"]["wire"]["slots"][slot_index]["naming"]["parts"][0]
                ["fields"][0]["node"]["byte_mapping"];
            assert!(mapping.is_object());
            mapping["cases"][0]["byte"] = json!(44);
            assert!(DecodeArtifact::from_json(&serde_json::to_vec(&forged).unwrap()).is_err());
            // The strict source-bound loader also rederives the mappings.
            let mut schema = serde_json::to_value(&artifact.schema).unwrap();
            schema["wire"]["slots"][slot_index]["naming"]["parts"][0]["fields"][0]["node"]
                ["byte_mapping"]["cases"][0]["byte"] = json!(44);
            assert!(shape::signals::wire::Decoder::load(
                &serde_json::to_vec(&schema).unwrap(),
                &source,
                &artifact.layout
            )
            .is_err());
            for (tag, logical) in [(yes, true), (no, false)] {
                let mut r = report();
                r.positional_signals[0].as_mut().unwrap().1[4] = tag ^ 0xa2;
                let decoded = loaded.decode_with(&capture(&loaded, &r), None, FULL).unwrap();
                assert_eq!(
                    decoded.positional_signals["UiEventIsTrusted"]["isTrusted"],
                    json!(logical)
                );
                assert_eq!(decoded.metadata.original_report.report(), r);
                assert!(decoded.metadata.exact_reencode);
            }
        }
    });
}

#[test]
fn nullable_value_schema_roundtrips_null_and_numeric_data_with_same_tag_value() {
    use shape::signals::wire::{named::Node, semantic::ByteMapping};
    fn null_mapping(node: &Node) -> Option<&ByteMapping> {
        match node {
            Node::Leaf {
                byte_mapping: Some(m),
                ..
            } => Some(m),
            Node::Choice { arms, .. } => arms.iter().find_map(null_mapping),
            _ => None,
        }
    }
    shape::run_with_large_stack(|| {
        for tag in [133u8, 199u8] {
            let s=source().replace("let callable; callable=sharedNumber; callable=callable(mask,navigator.deviceMemory,b,3); callable=0;",
                &format!("let observed=navigator.deviceMemory; if(observed==null){{b[4]={tag}^mask();}}else{{let callable;callable=sharedNumber;callable=callable(mask,observed,b,3);callable=0;}}"));
            let artifact =
                DecodeArtifact::compile(&s, &session(), "example".into(), FloatByteOrder::Little)
                    .unwrap();
            let loaded =
                DecodeArtifact::from_json(&serde_json::to_vec(&artifact).unwrap()).unwrap();
            let wire = loaded.schema.wire.as_ref().unwrap();
            let slot = wire
                .slots
                .iter()
                .find(|s| {
                    s.naming
                        .as_ref()
                        .is_some_and(|n| n.schema_key == "DeviceMemory")
                })
                .unwrap();
            let map = slot
                .naming
                .as_ref()
                .unwrap()
                .parts
                .iter()
                .flat_map(|p| &p.fields)
                .find_map(|f| null_mapping(&f.node))
                .unwrap();
            let token = shape::signals::codec::hash_string_token("ab");
            let mut r = report();
            r.positional_signals[1].as_mut().unwrap().1 =
                shape::signals::codec::encode_signal(token, 0x01020304, 17, |w| w.byte(tag));
            let decoded = loaded.decode_with(&capture(&loaded, &r), None, FULL).unwrap();
            let logical = &decoded.positional_signals["DeviceMemory"]["deviceMemory"];
            assert!(logical.is_null());
            let physical = map.encode(logical).unwrap();
            assert_eq!(
                shape::signals::codec::encode_signal(token, 0x01020304, 17, |w| w.byte(physical)),
                r.positional_signals[1].as_ref().unwrap().1
            );
            r.positional_signals[1].as_mut().unwrap().1 =
                shape::signals::codec::encode_signal(token, 0x01020304, 17, |w| {
                    w.number(tag as f64)
                });
            let decoded = loaded.decode_with(&capture(&loaded, &r), None, FULL).unwrap();
            assert_eq!(
                decoded.positional_signals["DeviceMemory"]["deviceMemory"],
                json!(tag)
            );
            assert!(decoded.metadata.exact_reencode);
        }
    });
}

#[test]
fn artifact_only_rejects_malformed_and_aliased_fixed_flag_members() {
    shape::run_with_large_stack(|| {
        let artifact = compiled();
        for malformed_name in [false, true] {
            let mut saved = serde_json::to_value(&artifact).unwrap();
            let naming = &mut saved["schema"]["wire"]["slots"][0]["naming"];
            naming["version"] = json!(2);
            naming["canonicalName"] = json!("GrecaptchaAccessorHookFlags");
            for part in naming["parts"].as_array_mut().unwrap() {
                let original = part["fields"][0].clone();
                let mut first = original.clone();
                first["name"] = json!("hookFlagsA");
                let mut second = original;
                second["name"] = json!(if malformed_name {
                    "hookFlagsC"
                } else {
                    "hookFlagsB"
                });
                // The well-spelled case still aliases the first physical leaf.
                part["fields"] = json!([{"fieldKey":"f1","name":"hookChecks","bits":null,
                    "node":{"node":"fixed_array","items":[first,second]}}]);
            }
            assert!(DecodeArtifact::from_json(&serde_json::to_vec(&saved).unwrap()).is_err());
        }
    });
}

#[test]
fn compact_grouped_index_maps_remainders_and_noncontiguous_writes() {
    use shape::signals::wire::named::{Field, Node};
    shape::run_with_large_stack(|| {
        let mut artifact = compiled();
        let naming = artifact.schema.wire.as_mut().unwrap().slots[0]
            .naming
            .as_mut()
            .unwrap();
        let name = naming.schema_key.clone();
        let mut first = naming.parts[0].fields[0].clone();
        first.name = "oldA".into();
        first.bits = Some([("fullByteFlag".into(), 7)].into());
        let mut second = first.clone();
        second.name = "oldB".into();
        second.bits = Some([("remainder".into(), 0)].into());
        if let Node::Leaf { leaf, .. } = &mut second.node {
            *leaf = 2;
        }
        let mut middle = first.clone();
        middle.name = "middle".into();
        middle.bits = None;
        if let Node::Leaf { leaf, .. } = &mut middle.node {
            *leaf = 1;
        }
        naming.parts[0].part_leaves = vec![0, 1, 2];
        naming.parts[0].fields = vec![
            Field {
                field_key: "f1".into(),
                name: "checks".into(),
                bits: None,
                node: Node::FixedArray {
                    items: vec![first, second],
                },
            },
            middle,
        ];
        let description = json!({"mask":0,"order":["checks","middle"],"fields":{
            "checks":{"type":"fixed_array","length":2,"items":[
                {"type":"flags","bits":{"fullByteFlag":7},"sourceName":"oldA","fieldKey":"f1"},
                {"type":"flags","bits":{"remainder":0},"sourceName":"oldB","fieldKey":"f3"}]},
            "middle":{"type":"number"}}});
        artifact.schema.signals.get_mut(&name).unwrap().description =
            serde_json::from_value(description).unwrap();
        let document = serde_json::to_value(artifact.compact_schema().unwrap()).unwrap();
        let schema = &document["signals"];
        assert_eq!(
            schema[&name]["writeOrder"],
            json!([["checks", 0], ["middle"], ["checks", 1]])
        );
        assert_eq!(
            schema[&name]["fields"]["checks"]["items"],
            json!([
            {"type":"flags","bits":{"fullByteFlag":7}}, {"type":"flags","bits":{"remainder":0}}])
        );
        assert_eq!(schema[&name]["xor"], 0);
        assert!(schema[&name].get("emissions").is_none());
    });
}

#[test]
fn source_pinned_named_and_numeric_routes_export_without_guessing_fields() {
    shape::run_with_large_stack(|| {
        let program = source()
            .replace("{ctor:literal,priority:false}]", "{ctor:literal,priority:false},{name:\"17\"},{name:17},{name:\"opaque\"},{name:4294967295}]")
            .replace("let filter=[0,1];", "let filter=[0,1,2,3,4,5];")
            .replace("let indices=[],table=[];", "let indices=[1,0,1],table=[[],[\"beta\",\"alpha\"]];");
        let pinned = shape::signals::payload::extract::pin(&program, &session()).unwrap();
        let artifact = DecodeArtifact::compile(
            &program,
            &session(),
            "example".into(),
            FloatByteOrder::Little,
        )
        .unwrap();
        assert_eq!(artifact.layout, pinned.layout);
        let public = serde_json::to_value(artifact.compact_schema().unwrap()).unwrap();
        assert_eq!(
            public["keysets"],
            json!([["beta", "alpha"], [], ["beta", "alpha"]])
        );
        assert_eq!(
            public["reportRoutes"]["signals"],
            json!([
                {"descriptor":2,"key":"17","contracts":[]},
                {"descriptor":4,"key":"opaque","contracts":[]}
            ])
        );
        assert_eq!(
            public["reportRoutes"]["keyedSignals"],
            json!([
                {"descriptor":3,"key":17,"contracts":[]},
                {"descriptor":5,"key":4294967295u32,"contracts":[]}
            ])
        );
        assert_eq!(
            public["reportRoutes"]["positionalSignals"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        // Encoding unknown valid keys must not be restricted to the route table.
        let mut r = report();
        r.keysets = artifact.layout.keysets.clone();
        r.keyed_signals
            .push((123456, Some((vec![Raw::Bool(true)], vec![7, 8]))));
        let input = capture(&artifact, &r);
        let output = artifact.decode_with(&input, None, FULL).unwrap();
        assert_eq!(output.keyed_signals, r.keyed_signals);
        assert_eq!(output.metadata.original_report.report(), r);
    });
}

#[test]
fn cli_source_only_schema_requires_neither_session_nor_capture() {
    shape::run_with_large_stack(|| {
        let temp = Temp::new();
        let source = source();
        fs::write(temp.0.join("program.js"), &source).unwrap();
        let args = ["decode", "--schema-only", "--seed", "program.js", "--input-kind", "program", "--schema-out", "schema.json"];
        let result = temp.run(&args);
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
        assert!(result.stdout.is_empty());
        let exported: Value = serde_json::from_slice(&fs::read(temp.0.join("schema.json")).unwrap()).unwrap();
        let expected = serde_json::to_value(shape::signals::decode::CompactSeedSchema::from_source(&source).unwrap()).unwrap();
        assert_eq!(exported, expected);
        assert!(exported.get("siteId").is_none());
        assert!(exported["seedId"].is_string());
        assert!(!temp.run(&args).status.success());
        assert!(!temp.run(&["decode", "--schema-only", "--seed", "program.js", "--input-kind", "program", "--schema-out", "program.js", "--force"]).status.success());
        assert_eq!(fs::read_to_string(temp.0.join("program.js")).unwrap(),source);
    });
}

#[path = "support/source_number_workflow.rs"]
mod source_number_workflow;
