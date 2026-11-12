//! Synthetic regressions for explicitly opted-in source-format recipes.
use oxc_span::Span;
use serde_json::{json, Value};
use shape::signals::{
    decode::{
        direct, source_compiler, CaptureHeaders, CompileOptions, DecodeArtifact, DecodeOptions,
        ValidatedArtifact,
    },
    extract::{
        contracts::{
            extract_signal_contracts, ChoiceArm, ChoiceKind, EmissionContract, GrammarNode as G,
            WriteNode,
        },
        layout::{EmissionVariant, ObserverRoute},
    },
    payload::{extract::Session, Headers, Report},
    schema::SourceSpan,
    wire::{
        explicit_initializer as codec, AlgorithmProof, AlgorithmReport, CounterInput,
        FloatByteOrder, Naming, Op, PackedPhase, Program, SourcePlan, Support, WireValue,
    },
};

fn write(value: &str, start: u32) -> WriteNode {
    serde_json::from_value(json!({
        "span":[start,start+1], "value_span":[start,start+1], "value":value,
        "mask":"unmasked", "mask_source":null, "xor_literal":null,
        "position":null
    }))
    .unwrap()
}
fn emission(grammar: G) -> EmissionContract {
    EmissionContract {
        site: SourceSpan { start: 0, end: 200 },
        forwarding_sites: vec![],
        quote: String::new(),
        route: ObserverRoute::Direct,
        phase: None,
        payload: EmissionVariant::RawBuffer,
        stream: None,
        rendered: String::new(),
        grammar,
    }
}
fn algorithms() -> AlgorithmReport {
    let proof = AlgorithmProof {
        codec: None,
        span: None,
        sha256: None,
        reason: Some("synthetic uncertified writer".into()),
    };
    AlgorithmReport {
        hash: proof.clone(),
        stream: proof.clone(),
        integer: proof.clone(),
        float_prefix: proof.clone(),
        string: proof.clone(),
        float: proof,
    }
}
fn compile(e: &EmissionContract) -> Program {
    compile_from_source(e, "")
}
fn compile_from_source(e: &EmissionContract, source: &str) -> Program {
    let body = direct::compile(
        &e.grammar,
        e,
        &algorithms(),
        &SourcePlan::default(),
        source,
        vec![],
        None,
        &mut vec![],
        &direct::CompileContext {
            direct: true,
            worker: false,
            counts: &[],
            loops: Default::default(),
            last_count: Default::default(),
        },
    )
    .unwrap();
    Program {
        prefix_byte_order: "be32".into(),
        seed_source: "captured_prefix".into(),
        stream_scope: "collector_local".into(),
        string_scope: "collector_local".into(),
        salt: 17,
        body,
    }
}
fn chosen(arm: usize, value: WireValue) -> WireValue {
    WireValue::Choice {
        arm,
        value: Box::new(value),
    }
}

#[test]
fn packed_counts_below_and_above_one_byte_are_synthesized_and_exact() {
    shape::run_with_large_stack(packed_source_roundtrip);
}

fn packed_source_roundtrip() {
    // Real extraction supplies the grammar, source spans and condition text.
    // A dynamic count prevents the extractor from unrolling a constant loop.
    let number = include_str!("../src/signals/wire/reference/number.js")
        .replace("local0(", "integerWriter(");
    let bindings = (0..32)
        .map(|i| format!("scope{i}={i}"))
        .collect::<Vec<_>>()
        .join(",");
    let reads = (0..32)
        .map(|i| format!("scope{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let source = source()
        .replace("function Session(token) {", &format!("function Session(token) {{let {bindings};void [{reads}];"))
        .replace("let tokenState;", &format!("let tokenState;{number}"))
        .replace("b[4]=9^mask();", "let p=3;let n=globalThis.sampleCount;let packed=0;p=integerWriter(mask,n,b,p);for(let i=0;i<n;++i){++packed;if(packed === 8){b[++p]=173;packed=0;}}if(packed !== 0){b[++p]=174;}");
    let contracts = extract_signal_contracts(&source).unwrap();
    let e = &contracts.signals[1].emissions[0];
    let program = compile_from_source(e, &source);
    let token = shape::signals::codec::hash_string_token("ab");
    let Op::Sequence { items } = &program.body else {
        panic!("sequence")
    };
    let Op::CountedRepeat { body, .. } = &items[1] else {
        panic!("counted repeat")
    };
    let Op::CounterChoice { proof, .. } = body.as_ref() else {
        panic!("counter choice")
    };
    assert!(matches!(
        &proof.input,
        CounterInput::Packed {
            modulus: 8,
            phase: PackedPhase::BeforeFlush,
            ..
        }
    ));
    assert!(
        matches!(&items[2],Op::CounterChoice {proof,..} if matches!(&proof.input, CounterInput::Packed {modulus:8,phase:PackedPhase::AfterLoop,..}))
    );
    assert!(source_compiler::compile(
        &e.grammar,
        e,
        &algorithms(),
        &SourcePlan::default(),
        &source,
        vec![],
        None,
        &mut vec![],
        &source_compiler::CompileContext {
            worker: true,
            counts: &[]
        }
    )
    .is_err());
    for count in [3usize, 8, 11, 16] {
        let value = WireValue::Sequence(vec![
            WireValue::NumberBits((count as f64).to_bits()),
            WireValue::Repeat(
                (0..count)
                    .map(|i| {
                        if (i + 1) % 8 == 0 {
                            chosen(0, WireValue::Byte(173))
                        } else {
                            chosen(1, WireValue::Sequence(vec![]))
                        }
                    })
                    .collect(),
            ),
            if count % 8 != 0 {
                chosen(0, WireValue::Byte(174))
            } else {
                chosen(1, WireValue::Sequence(vec![]))
            },
        ]);
        let bytes =
            codec::encode(&program, &value, 0x01020304, token, FloatByteOrder::Little).unwrap();
        // The number writer consumes the fixture stream's first byte (0xa2).
        let mut expected = vec![1, 2, 3, 4, count as u8 ^ 0xa2];
        expected.extend(std::iter::repeat_n(173, count / 8));
        if count % 8 != 0 {
            expected.push(174)
        }
        assert_eq!(bytes, expected, "count {count}");
        let decoded = codec::decode(&program, &bytes, token, FloatByteOrder::Little).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(
            codec::encode(
                &program,
                &decoded,
                0x01020304,
                token,
                FloatByteOrder::Little
            )
            .unwrap(),
            bytes
        );
    }
}

fn source() -> String {
    include_str!("fixtures/signal-wire-roundtrip/program.js")
        .replace(
            "__HASH__",
            &include_str!("../src/signals/wire/reference/hash.js").replace("local0(", "tokenHash("),
        )
        .replace(
            "__STREAM__",
            &include_str!("../src/signals/wire/reference/stream.js")
                .replace("local0(", "streamFactory(")
                .replace("local9", "tokenState"),
        )
}
fn session() -> Session {
    serde_json::from_value(json!({"transactionToken":"ab","alphabet":"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/","shuffleKey":[],"keys":[0,0,0,0,0,0,0,0]})).unwrap()
}
fn artifact(direct: bool) -> DecodeArtifact {
    DecodeArtifact::compile_with_options(
        &source(),
        &session(),
        "example.test".into(),
        FloatByteOrder::Little,
        CompileOptions { direct },
    )
    .unwrap()
}

#[test]
fn opt_in_is_persisted_and_legacy_artifact_json_keeps_default_policy() {
    shape::run_with_large_stack(|| {
        assert!(!CompileOptions::default().direct);
        let default = DecodeArtifact::compile(
            &source(),
            &session(),
            "example.test".into(),
            FloatByteOrder::Little,
        )
        .unwrap();
        let value = serde_json::to_value(&default).unwrap();
        assert!(value.get("direct").is_none());
        assert!(
            !DecodeArtifact::from_json(&serde_json::to_vec(&value).unwrap())
                .unwrap()
                .direct
        );
        let direct = artifact(true);
        let mut value = serde_json::to_value(&direct).unwrap();
        assert_eq!(value["direct"], true);
        assert!(
            DecodeArtifact::from_json(&serde_json::to_vec(&value).unwrap())
                .unwrap()
                .direct
        );
        value.as_object_mut().unwrap().remove("direct");
        assert!(DecodeArtifact::from_json(&serde_json::to_vec(&value).unwrap()).is_err());
        value["direct"] = json!(true);
        value["schema"]["diagnostics"]["directDecode"] = json!(false);
        assert!(DecodeArtifact::from_json(&serde_json::to_vec(&value).unwrap()).is_err());
        let strict = default.schema.wire.as_ref().unwrap();
        let opted = direct.schema.wire.as_ref().unwrap();
        for (before, after) in strict.slots.iter().zip(&opted.slots) {
            for (before, after) in before.programs.iter().zip(&after.programs) {
                if matches!(before, Support::Supported { .. }) {
                    assert_eq!(before, after)
                }
            }
        }
    });
}

fn local_artifact() -> DecodeArtifact {
    let mut artifact = artifact(true);
    let wire = artifact.schema.wire.as_mut().unwrap();
    let slot = &mut wire.slots[0];
    let previous = slot.naming.as_ref().unwrap().schema_key.clone();
    let description = artifact.schema.signals.remove(&previous).unwrap();
    artifact
        .schema
        .signals
        .insert("LocalStoragePersistentId".into(), description);
    let e = emission(G::Choice {
        kind: ChoiceKind::If,
        span: Span::new(0, 150),
        condition: Some("storedId == null".into()),
        arms: vec![
            ChoiceArm {
                label: "then".into(),
                body: G::Sequence {
                    items: vec![G::Byte(write("130", 10)), G::Number(write("status", 20))],
                },
            },
            ChoiceArm {
                label: "else".into(),
                body: G::Sequence {
                    items: vec![
                        G::String(write("storedId", 30)),
                        G::Number(write("status", 40)),
                    ],
                },
            },
        ],
    });
    let program = compile(&e);
    slot.contract.emissions = vec![e];
    slot.source_plan = vec![SourcePlan::default()];
    slot.programs = vec![Support::Supported { program }];
    slot.naming = Some(Naming {
        version: 1,
        counted_arrays: false,
        schema_key: "LocalStoragePersistentId".into(),
        canonical_name: "LocalStoragePersistentId".into(),
        parts: vec![],
        mapping_error: None,
    });
    artifact.validate().unwrap();
    artifact
}
fn capture(artifact: &DecodeArtifact, raw: Vec<u8>) -> CaptureHeaders {
    let mut positional = vec![None; artifact.layout.report.positional_signals.len()];
    positional[0] = Some((vec![], raw));
    let report = Report {
        errors: vec![],
        positional_signals: positional,
        signals: vec![],
        keyed_signals: vec![],
        seeds: [19, 23],
        keysets: artifact.layout.keysets.clone(),
    };
    let headers = artifact
        .layout
        .encode_headers(&report.channels_owned(), report.seeds)
        .unwrap();
    CaptureHeaders {
        site_id: artifact.site_id.clone(),
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

#[test]
fn direct_retains_unnamed_bytes_without_inventing_fields() {
    shape::run_with_large_stack(|| {
        let mut artifact = artifact(true);
        artifact.schema.wire.as_mut().unwrap().slots[0].naming = None;
        let capture = capture(&artifact, vec![1, 2, 3, 4, 99]);
        let decoded = artifact
            .decode_with(
                &capture,
                None,
                DecodeOptions {
                    verify: true,
                    metadata: true,
                },
            )
            .unwrap();
        assert!(decoded.positional_signals.is_empty());
        assert_eq!(decoded.metadata.decoded, 0);
        // The other fixture slot is unnamed too, so it is not a missing named signal.
        assert_eq!(decoded.metadata.missing, 0);
        assert!(decoded.metadata.exact_reencode);
        assert!(decoded.metadata.original_report.positional_signals[0].is_some());
        assert!(decoded.metadata.original_report.positional_signals[1].is_none());
        artifact.direct = false;
        artifact
            .schema
            .diagnostics
            .as_object_mut()
            .unwrap()
            .remove("directDecode");
        assert!(artifact.decode(&capture, None).is_err());
    });
}

#[test]
fn storage_root_projection_is_direct_only_and_edits_are_inverted_or_refused() {
    shape::run_with_large_stack(|| {
        let artifact = local_artifact();
        let program = match &artifact.schema.wire.as_ref().unwrap().slots[0].programs[0] {
            Support::Supported { program } => program,
            _ => panic!("program"),
        };
        for (arm, stored, expected) in [
            (0, WireValue::Byte(130), Value::Null),
            (
                1,
                WireValue::StringUnits("synthetic-id".encode_utf16().collect()),
                json!("synthetic-id"),
            ),
        ] {
            let typed = chosen(
                arm,
                WireValue::Sequence(vec![stored, WireValue::NumberBits(3f64.to_bits())]),
            );
            let bytes =
                codec::encode(program, &typed, 0x01020304, 7, FloatByteOrder::Little).unwrap();
            let capture = capture(&artifact, bytes);
            let options = DecodeOptions {
                verify: true,
                metadata: true,
            };
            let decoded = artifact.decode_with(&capture, None, options).unwrap();
            assert_eq!(
                decoded.positional_signals["LocalStoragePersistentId"]["storedId"],
                expected
            );
            assert_eq!(
                decoded.positional_signals["LocalStoragePersistentId"]["storageStatus"].as_f64(),
                Some(3.0)
            );
            let validated = ValidatedArtifact::new(artifact.clone()).unwrap();
            let mut payload = serde_json::to_value(&decoded).unwrap();
            let encoded = validated.encode_named(&payload, None).unwrap();
            assert_eq!(
                serde_json::to_value(&encoded).unwrap(),
                serde_json::to_value(&capture).unwrap()
            );
            payload["positionalSignals"]["LocalStoragePersistentId"]["storageStatus"] = json!(5);
            payload["positionalSignals"]["LocalStoragePersistentId"]["storedId"] =
                json!("edited-id");
            let edited = validated.encode_named(&payload, None).unwrap();
            let edited = validated.decode(&edited, None, options).unwrap();
            assert_eq!(
                edited.positional_signals["LocalStoragePersistentId"]["storedId"],
                "edited-id"
            );
            assert_eq!(
                edited.positional_signals["LocalStoragePersistentId"]["storageStatus"].as_f64(),
                Some(5.0)
            );
            payload["positionalSignals"]["LocalStoragePersistentId"]["unknown"] = json!(true);
            assert!(validated.encode_named(&payload, None).is_err());
            let mut conservative = artifact.clone();
            conservative.direct = false;
            conservative
                .schema
                .diagnostics
                .as_object_mut()
                .unwrap()
                .remove("directDecode");
            assert!(conservative.decode_with(&capture, None, options).is_err());
        }
    });
}
