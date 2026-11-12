use super::*;
use shape::signals::{
    decode::{DecodeOptions, ValidatedArtifact},
    wire::{Decoder, WireError},
};

fn native_source() -> String {
    let bindings = (0..32)
        .map(|i| format!("scope{i}={i}"))
        .collect::<Vec<_>>()
        .join(",");
    let reads = (0..32)
        .map(|i| format!("scope{i}"))
        .collect::<Vec<_>>()
        .join(",");
    source().replace("function Session(token) {", &format!("function Session(token) {{let {bindings};void [{reads}];")).replace("function integerWriter(","const local10=new globalThis.Float64Array(1);const local11=globalThis.Uint8Array;const local12=[];function nativeWriterWitness(m,v,b,p){return integerWriter(m,v,b,p);}function integerWriter(")
        .replace("const navigator={deviceMemory:9};","const navigator=globalThis.navigator;")
}
fn artifact(source: &str, order: FloatByteOrder) -> DecodeArtifact {
    DecodeArtifact::compile(source, &session(), "example".into(), order).unwrap()
}
fn raw(bits: u64, order: FloatByteOrder) -> Vec<u8> {
    // Known IEEE vectors supplied independently of the canonical number writer.
    let mut w = shape::signals::codec::SignalWriter::new(
        shape::signals::codec::hash_string_token("ab"),
        123,
        17,
    );
    w.byte(128);
    for byte in if order == FloatByteOrder::Big {
        bits.to_be_bytes()
    } else {
        bits.to_le_bytes()
    } {
        w.byte(byte)
    }
    w.finish()
}
fn with_raw(bytes: Vec<u8>) -> Report {
    let mut r = report();
    r.positional_signals[1].as_mut().unwrap().1 = bytes;
    r
}
fn same_headers(a: &CaptureHeaders, b: &CaptureHeaders) {
    assert_eq!(
        serde_json::to_value(a).unwrap(),
        serde_json::to_value(b).unwrap()
    )
}

#[test]
fn source_number_public_native_vectors_full_metadata_off_explain_and_transport() {
    shape::run_with_large_stack(|| {
        for order in [FloatByteOrder::Little, FloatByteOrder::Big] {
            let a = artifact(&native_source(), order);
            assert!(a
                .schema
                .diagnostics
                .get("sourceNumberRepresentation")
                .is_some());
            let slot = a
                .schema
                .wire
                .as_ref()
                .unwrap()
                .slots
                .iter()
                .find(|s| {
                    matches!(
                        s.routing,
                        shape::signals::wire::WireRoute::Positional { positional: 1 }
                    )
                })
                .unwrap();
            let shape::signals::extract::contracts::GrammarNode::Sequence { items } =
                &slot.contract.emissions[0].grammar
            else {
                panic!()
            };
            let shape::signals::extract::contracts::GrammarNode::Number(w) = &items[1] else {
                panic!()
            };
            assert!(
                w.writer_factory.is_none(),
                "exercise concrete call recovery, not a supplied factory"
            );
            let v = ValidatedArtifact::from_json(&serde_json::to_vec(&a).unwrap()).unwrap();
            for bits in [
                0,
                0x8000000000000000,
                0x3ff0000000000000,
                0x41dfffffffc00000,
                0xc1e0000000000000,
                0x3ff8000000000000,
                0x41e0000000000000,
                1,
                0x7ff0000000000000,
                0xfff0000000000000,
                0x7ff8000000000021,
            ] {
                let mut r = with_raw(raw(bits, order));
                r.positional_signals[1].as_mut().unwrap().0 = vec![Raw::Bool(true)];
                let input = capture(&a, &r);
                for metadata in [false, true] {
                    let output = v
                        .decode(
                            &input,
                            None,
                            DecodeOptions {
                                verify: true,
                                metadata,
                            },
                        )
                        .unwrap();
                    assert!(output.metadata.exact_reencode);
                    assert_eq!(output.metadata.original_report.report(), r);
                    if !metadata {
                        assert!(output.metadata.signals.is_empty());
                    }
                    assert_eq!(
                        v.explain(&output, "DeviceMemory", None, true)
                            .unwrap()
                            .status,
                        shape::signals::wire::NamedStatus::Decoded
                    );
                    let json = serde_json::to_vec(&output).unwrap();
                    let transported: Value = serde_json::from_slice(&json).unwrap();
                    same_headers(&v.encode_named(&transported, None).unwrap(), &input);
                    // Mutable and cached paths use the same validated policy.
                    assert!(
                        a.decode_with(
                            &input,
                            None,
                            DecodeOptions {
                                verify: true,
                                metadata
                            }
                        )
                        .unwrap()
                        .metadata
                        .exact_reencode
                    );
                }
            }
        }
    });
}
#[test]
fn source_number_equal_edits_changed_edits_and_bare_map_have_distinct_forms() {
    shape::run_with_large_stack(|| {
        let a = artifact(&native_source(), FloatByteOrder::Little);
        let v = ValidatedArtifact::new(a.clone()).unwrap();
        let input = capture(&a, &with_raw(raw(1f64.to_bits(), FloatByteOrder::Little)));
        let decoded = v.decode(&input, None, FULL).unwrap();
        let mut equal = serde_json::to_value(&decoded).unwrap();
        equal["positionalSignals"]["DeviceMemory"]["deviceMemory"] = json!(1.0);
        same_headers(&v.encode_named(&equal, None).unwrap(), &input);
        let mut edited = equal.clone();
        edited["positionalSignals"]["DeviceMemory"]["deviceMemory"] = json!(3);
        let headers = v.encode_named(&edited, None).unwrap();
        let again = v.decode(&headers, None, FULL).unwrap();
        assert_eq!(
            again.positional_signals["DeviceMemory"]["deviceMemory"],
            json!(3)
        );
        assert_eq!(
            again.metadata.original_report.positional_signals[1]
                .as_ref()
                .unwrap()
                .1
                .len(),
            5
        );
        assert_eq!(
            again.metadata.original_report.positional_signals[0],
            decoded.metadata.original_report.positional_signals[0]
        );
        let bare = serde_json::to_value(&decoded.positional_signals).unwrap();
        let headers = v.encode_named(&bare, None).unwrap();
        let again = v.decode(&headers, None, FULL).unwrap();
        assert_eq!(again.positional_signals, decoded.positional_signals);
        assert_eq!(
            again.metadata.original_report.positional_signals[1]
                .as_ref()
                .unwrap()
                .1
                .len(),
            5
        );
        let mut corrupt = equal;
        corrupt["metadata"]["originalReport"]["positionalSignals"] = json!([]);
        assert!(v.encode_named(&corrupt, None).is_err());
        let sdk = shape::sdk::Generator::load(&serde_json::to_vec(&a).unwrap()).unwrap();
        let decoded = sdk
            .call(
                "decode",
                json!({"capture":serde_json::to_value(&input).unwrap()}),
            )
            .unwrap();
        let encoded = sdk.call("encode", json!({"decoded":decoded})).unwrap();
        let encoded: CaptureHeaders = serde_json::from_value(encoded).unwrap();
        same_headers(&encoded, &input);
    });
}
#[test]
fn source_number_legacy_restrictions_and_missing_proofs_remain_refusals() {
    shape::run_with_large_stack(|| {
        let src = native_source();
        let a = artifact(&src, FloatByteOrder::Little);
        let input = capture(&a, &with_raw(raw(1f64.to_bits(), FloatByteOrder::Little)));
        let mut legacy = a.clone();
        legacy
            .schema
            .diagnostics
            .as_object_mut()
            .unwrap()
            .remove("sourceNumberRepresentation");
        let legacy = DecodeArtifact::from_json(&serde_json::to_vec(&legacy).unwrap()).unwrap();
        assert!(legacy.decode_with(&input, None, FULL).is_err());
        let literal = src.replace(
            "callable(mask,navigator.deviceMemory,b,3)",
            "callable(mask,1,b,3)",
        );
        let restricted = artifact(&literal, FloatByteOrder::Little);
        assert!(restricted
            .decode_with(
                &capture(
                    &restricted,
                    &with_raw(raw(1f64.to_bits(), FloatByteOrder::Little))
                ),
                None,
                FULL
            )
            .is_err());
        for bad in [
            src.replace(
                "new globalThis.Float64Array(1)",
                "new globalThis.Float32Array(1)",
            ),
            src.replace("local1() ^ 128", "local1() ^ 129"),
            src.replace("b,3); callable=0;", "b,4); callable=0;"),
        ] {
            let bad = artifact(&bad, FloatByteOrder::Little);
            assert!(bad
                .decode_with(
                    &capture(&bad, &with_raw(raw(1f64.to_bits(), FloatByteOrder::Little))),
                    None,
                    FULL
                )
                .is_err());
        }
        for bytes in [
            vec![0, 0, 0, 0],
            vec![0, 0, 0, 0, 129],
            vec![0, 0, 0, 0, 128, 0],
        ] {
            assert!(a
                .decode_with(&capture(&a, &with_raw(bytes)), None, FULL)
                .is_err());
        }
        let mut bytes = raw(1f64.to_bits(), FloatByteOrder::Little);
        bytes.push(0);
        assert!(a
            .decode_with(&capture(&a, &with_raw(bytes)), None, FULL)
            .is_err());
        let mut unknown = a.clone();
        unknown.float_byte_order = FloatByteOrder::Unknown;
        assert!(unknown.decode_with(&input, None, FULL).is_err());
    });
}
#[test]
fn source_number_namespace_validation_regeneration_and_unrelated_json() {
    shape::run_with_large_stack(|| {
        let src = native_source();
        let mut a = artifact(&src, FloatByteOrder::Little);
        a.schema.diagnostics["unrelatedExtension"] = json!({"keep":[true,{"text":"public-only"}]});
        let saved = serde_json::to_vec(&a).unwrap();
        let loaded = DecodeArtifact::from_json(&saved).unwrap();
        assert_eq!(
            loaded.schema.diagnostics["unrelatedExtension"],
            a.schema.diagnostics["unrelatedExtension"]
        );
        // Source-present regeneration runs even though the raw typed API stays canonical.
        assert!(!matches!(
            Decoder::load(&serde_json::to_vec(&a.schema).unwrap(), &src, &a.layout),
            Err(WireError::Evidence)
        ));
        for mutation in 0..13 {
            let mut bad = serde_json::to_value(&a).unwrap();
            let doc = &mut bad["schema"]["diagnostics"]["sourceNumberRepresentation"];
            match mutation {
                0 => doc["version"] = json!(99),
                1 => doc["source"]["sha256"] = json!("bad"),
                2 => doc["native"]["width"] = json!(4),
                3 => doc["native"]["nativeDependencies"]["codec"] = json!("unrelated"),
                4 => {
                    let entry = doc["entries"][0].clone();
                    doc["entries"].as_array_mut().unwrap().push(entry);
                }
                5 => doc["entries"][0]["program"] = json!([999]),
                6 => doc["entries"][0]["emission"] = json!(999),
                7 => doc["entries"][0]["root"] = json!(0),
                8 => doc["entries"][0]["writer"]["sha256"] = json!("bad"),
                9 => doc["entries"][0]["call"]["start"] = json!(0),
                10 => doc["entries"][0]["value"]["start"] = json!(0),
                11 => doc["entries"][0]["contract_digest"] = json!("bad"),
                _ => doc["entries"][0]["grammar"] = json!([]),
            }
            assert!(
                DecodeArtifact::from_json(&serde_json::to_vec(&bad).unwrap()).is_err(),
                "mutation {mutation}"
            );
            let mutable: DecodeArtifact = serde_json::from_value(bad).unwrap();
            assert!(mutable.validate().is_err());
        }
        let saved = String::from_utf8(saved).unwrap();
        let duplicate = saved.replacen("\"version\":1", "\"version\":1,\"version\":1", 1);
        assert!(DecodeArtifact::from_json(duplicate.as_bytes()).is_err());
        let mut forged = a.schema.clone();
        forged.diagnostics["sourceNumberRepresentation"]["entries"][0]["domain"] =
            json!("canonical");
        assert!(matches!(
            Decoder::load(&serde_json::to_vec(&forged).unwrap(), &src, &a.layout),
            Err(WireError::Evidence)
        ));
    });
}

fn constrain_slot_number(a: &mut DecodeArtifact, positional: usize, bits: u64) -> String {
    use shape::signals::wire::{Op, Support, WireRoute};
    let slot = a
        .schema
        .wire
        .as_mut()
        .unwrap()
        .slots
        .iter_mut()
        .find(|s| matches!(s.routing, WireRoute::Positional { positional: p } if p == positional))
        .unwrap();
    let program = match &mut slot.programs[0] {
        Support::Supported { program } | Support::RequiresInitializer { program, .. } => program,
        _ => panic!("public fixture needs a compiled number"),
    };
    fn constrain(op: &mut Op, bits: u64) -> usize {
        match op {
            Op::Number { literal, .. } => {
                *literal = Some(bits);
                1
            }
            Op::Sequence { items } => items.iter_mut().map(|op| constrain(op, bits)).sum(),
            _ => 0,
        }
    }
    assert_eq!(constrain(&mut program.body, bits), 1);
    slot.naming.as_ref().unwrap().schema_key.clone()
}

#[test]
fn source_number_legacy_constraints_ignore_absent_or_empty_form_policies() {
    shape::run_with_large_stack(|| {
        let a = artifact(&native_source(), FloatByteOrder::Little);
        let original = a.decode_with(&capture(&a, &report()), None, FULL).unwrap();
        let bits = (-1.5000000000000002f64).to_bits();
        for absent in [true, false] {
            let mut constrained = a.clone();
            let key = constrain_slot_number(&mut constrained, 1, bits);
            if absent {
                constrained
                    .schema
                    .diagnostics
                    .as_object_mut()
                    .unwrap()
                    .remove("sourceNumberRepresentation");
            } else {
                // A recognized namespace can contain no native-input authority.
                constrained.schema.diagnostics["sourceNumberRepresentation"]["entries"] = json!([]);
            }
            let loaded =
                ValidatedArtifact::from_json(&serde_json::to_vec(&constrained).unwrap()).unwrap();
            let mut edited = original.clone();
            edited
                .positional_signals
                .get_mut(&key)
                .unwrap()
                .insert("deviceMemory".into(), json!(f64::from_bits(bits)));
            let encoded = loaded
                .encode_named(&serde_json::to_value(&edited).unwrap(), None)
                .unwrap();
            let again = loaded.decode(&encoded, None, FULL).unwrap();
            assert_eq!(
                again.positional_signals[&key]["deviceMemory"]
                    .as_f64()
                    .unwrap()
                    .to_bits(),
                bits
            );
            assert!(again.metadata.exact_reencode);
            // The legacy reconstruction still enforces the new literal.
            edited
                .positional_signals
                .get_mut(&key)
                .unwrap()
                .insert("deviceMemory".into(), json!(2));
            assert!(loaded
                .encode_named(&serde_json::to_value(&edited).unwrap(), None)
                .is_err());
        }
    });
}

#[test]
fn source_number_unrelated_slot_policy_preserves_legacy_fallback_and_other_forms() {
    shape::run_with_large_stack(|| {
        use shape::signals::wire::WireRoute;
        let extra = "let extra=wrap(function(o){let seed=Math.random()*4294967296|0;let mask=streamFactory(seed^17);let b=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];integerWriter(mask,navigator.deviceMemory,b,3);o.next([[],b]);});";
        let src = native_source()
            .replace("let registry=[", &format!("{extra}let registry=["))
            .replace(
                "{ctor:literal,priority:false}]",
                "{ctor:literal,priority:false},{ctor:extra,priority:false}]",
            )
            .replace("let filter=[0,1];", "let filter=[0,1,2];");
        let a = artifact(&src, FloatByteOrder::Little);
        let mut r = report();
        r.positional_signals
            .push(Some((vec![], raw(1f64.to_bits(), FloatByteOrder::Little))));
        let original = a.decode_with(&capture(&a, &r), None, FULL).unwrap();
        let wire = a.schema.wire.as_ref().unwrap();
        let target = wire
            .slots
            .iter()
            .find(|s| matches!(s.routing, WireRoute::Positional { positional: 1 }))
            .unwrap()
            .contract
            .descriptor;
        let other = wire
            .slots
            .iter()
            .find(|s| matches!(s.routing, WireRoute::Positional { positional: 2 }))
            .unwrap()
            .contract
            .descriptor;
        let mut constrained = a.clone();
        let bits = (-1.5000000000000002f64).to_bits();
        let key = constrain_slot_number(&mut constrained, 1, bits);
        let entries = constrained.schema.diagnostics["sourceNumberRepresentation"]["entries"]
            .as_array_mut()
            .unwrap();
        entries.retain(|entry| entry["descriptor"].as_u64() != Some(target as u64));
        assert!(entries
            .iter()
            .any(|entry| entry["descriptor"].as_u64() == Some(other as u64)
                && entry["domain"] == "native_input"));
        let loaded =
            ValidatedArtifact::from_json(&serde_json::to_vec(&constrained).unwrap()).unwrap();
        let mut edited = original.clone();
        edited
            .positional_signals
            .get_mut(&key)
            .unwrap()
            .insert("deviceMemory".into(), json!(f64::from_bits(bits)));
        let encoded = loaded
            .encode_named(&serde_json::to_value(&edited).unwrap(), None)
            .unwrap();
        let again = loaded.decode(&encoded, None, FULL).unwrap();
        assert_eq!(
            again.positional_signals[&key]["deviceMemory"]
                .as_f64()
                .unwrap()
                .to_bits(),
            bits
        );
        // Another slot's active IEEE authority neither blocks this legacy slot
        // nor loses its own retained integer-valued IEEE representation.
        assert_eq!(
            again.metadata.original_report.positional_signals[2],
            original.metadata.original_report.positional_signals[2]
        );
        assert_eq!(
            again.metadata.original_report.positional_signals[0],
            original.metadata.original_report.positional_signals[0]
        );
    });
}

#[test]
fn source_number_active_slot_policy_refuses_failed_retained_decode() {
    shape::run_with_large_stack(|| {
        let a = artifact(&native_source(), FloatByteOrder::Little);
        let v = ValidatedArtifact::new(a.clone()).unwrap();
        let input = capture(&a, &with_raw(raw(1f64.to_bits(), FloatByteOrder::Little)));
        let original = v.decode(&input, None, FULL).unwrap();
        same_headers(
            &v.encode_named(&serde_json::to_value(&original).unwrap(), None)
                .unwrap(),
            &input,
        );
        let mut damaged = original.clone();
        let mut w = shape::signals::codec::SignalWriter::new(
            shape::signals::codec::hash_string_token("ab"),
            123,
            17,
        );
        w.byte(129); // Valid prefix and mask, invalid number marker.
        damaged.metadata.original_report.positional_signals[1]
            .as_mut()
            .unwrap()
            .1 = w.finish();
        assert!(v
            .encode_named(&serde_json::to_value(&damaged).unwrap(), None)
            .is_err());
        // Bare construction is otherwise possible, so the refusal above tests
        // the retained-form boundary rather than missing projection support.
        let bare = serde_json::to_value(&original.positional_signals).unwrap();
        let encoded = v.encode_named(&bare, None).unwrap();
        assert_eq!(
            v.decode(&encoded, None, FULL).unwrap().positional_signals,
            original.positional_signals
        );
    });
}

#[test]
fn source_number_direct_preserves_conditional_core_recipes_and_forms() {
    shape::run_with_large_stack(|| {
        use shape::signals::{
            analysis::{analyze_program_signals, SignalFinderOptions},
            decode::CompileOptions,
            wire::{Op, SchemaDocument, Support, WireRoute},
        };
        // An extra, value-preserving state write prevents the stronger
        // single-write lifetime proof. The source-format body remains certified.
        let src = native_source().replace(
            "tokenState=tokenHash(context.transactionToken);",
            "tokenState=tokenHash(context.transactionToken);tokenState=tokenState;",
        );
        let analysis = analyze_program_signals(
            &src,
            &SignalFinderOptions {
                layouts_only: true,
                finder_roles: true,
            },
        )
        .unwrap();
        let core: SchemaDocument =
            serde_json::from_value(serde_json::to_value(&analysis.report).unwrap()).unwrap();
        let slot = core
            .wire
            .as_ref()
            .unwrap()
            .slots
            .iter()
            .find(|s| matches!(s.routing, WireRoute::Positional { positional: 1 }))
            .unwrap();
        let Support::RequiresInitializer {
            program: proven, ..
        } = &slot.programs[0]
        else {
            panic!("fixture must exercise conditional core preservation")
        };
        let Op::Sequence { items } = &proven.body else {
            panic!("sequence")
        };
        assert!(matches!(
            &items[0],
            Op::Number {
                envelope: Some(_),
                ..
            }
        ));
        for order in [FloatByteOrder::Little, FloatByteOrder::Big] {
            let conservative = artifact(&src, order);
            for direct in [false, true] {
                let a = DecodeArtifact::compile_with_options(
                    &src,
                    &session(),
                    "example".into(),
                    order,
                    CompileOptions { direct },
                )
                .unwrap();
                assert_eq!(a.direct, direct);
                assert_eq!(
                    a.schema.diagnostics.get("directDecode"),
                    direct.then_some(&Value::Bool(true))
                );
                let actual = a
                    .schema
                    .wire
                    .as_ref()
                    .unwrap()
                    .slots
                    .iter()
                    .find(|s| matches!(s.routing, WireRoute::Positional { positional: 1 }))
                    .unwrap();
                assert_eq!(actual.programs, slot.programs);
                assert_eq!(
                    a.schema.diagnostics["sourceNumberRepresentation"],
                    conservative.schema.diagnostics["sourceNumberRepresentation"]
                );
                let v = ValidatedArtifact::from_json(&serde_json::to_vec(&a).unwrap()).unwrap();
                for bits in [
                    0,
                    0x8000000000000000,
                    1f64.to_bits(),
                    0x41dfffffffc00000,
                    0xc1e0000000000000,
                ] {
                    let input = capture(&a, &with_raw(raw(bits, order)));
                    for metadata in [false, true] {
                        let decoded = v
                            .decode(
                                &input,
                                None,
                                DecodeOptions {
                                    verify: true,
                                    metadata,
                                },
                            )
                            .unwrap();
                        assert!(decoded.metadata.exact_reencode);
                        same_headers(
                            &v.encode_named(&serde_json::to_value(&decoded).unwrap(), None)
                                .unwrap(),
                            &input,
                        );
                        assert_eq!(
                            v.explain(&decoded, "DeviceMemory", None, true)
                                .unwrap()
                                .status,
                            shape::signals::wire::NamedStatus::Decoded
                        );
                    }
                }
                let input = capture(&a, &with_raw(raw(1f64.to_bits(), order)));
                let decoded = v.decode(&input, None, FULL).unwrap();
                let mut equal = serde_json::to_value(&decoded).unwrap();
                equal["positionalSignals"]["DeviceMemory"]["deviceMemory"] = json!(1.0);
                same_headers(&v.encode_named(&equal, None).unwrap(), &input);
                equal["positionalSignals"]["DeviceMemory"]["deviceMemory"] = json!(3);
                let encoded = v.encode_named(&equal, None).unwrap();
                let again = v.decode(&encoded, None, FULL).unwrap();
                assert_eq!(
                    again.positional_signals["DeviceMemory"]["deviceMemory"],
                    json!(3)
                );
                assert_eq!(
                    again.metadata.original_report.positional_signals[1]
                        .as_ref()
                        .unwrap()
                        .1
                        .len(),
                    5
                );
                assert_eq!(
                    again.metadata.original_report.positional_signals[0],
                    decoded.metadata.original_report.positional_signals[0]
                );
                let reloaded =
                    ValidatedArtifact::from_json(&serde_json::to_vec(v.artifact()).unwrap())
                        .unwrap();
                assert_eq!(
                    reloaded
                        .decode(&input, None, FULL)
                        .unwrap()
                        .positional_signals,
                    decoded.positional_signals
                );
            }
        }
    });
}
