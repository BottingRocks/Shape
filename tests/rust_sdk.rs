//! Public Rust SDK contracts using the same synthetic documents as the bindings.
use serde::Serialize;
use serde_json::{json, Value};
use shape::signals::{
    decode::{CaptureHeaders, CompactSeedSchema, DecodeArtifact, DecodeOptions},
    payload::extract::Session,
    wire::{Op, Support},
};
use shape::{sdk, Generator};
use std::sync::OnceLock;

fn value(input: impl Serialize) -> Value {
    serde_json::to_value(input).unwrap()
}

struct Fixture {
    generator: Generator,
    capture: CaptureHeaders,
}

fn fixture() -> &'static Fixture {
    static FIXTURE: OnceLock<Fixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let artifact = sdk::call(
            "artifact.compile",
            json!({
                "source": include_str!("fixtures/sdk/decode/program.js"),
                "session": include_str!("fixtures/sdk/decode/session.json"),
                "site_id": "example"
            }),
        )
        .unwrap();
        Fixture {
            generator: Generator::load(&serde_json::to_vec(&artifact).unwrap()).unwrap(),
            capture: serde_json::from_str(include_str!("fixtures/sdk/decode/headers.json"))
                .unwrap(),
        }
    })
}

const FULL: DecodeOptions = DecodeOptions {
    verify: true,
    metadata: true,
};

#[test]
fn typed_and_json_schema_decode_explain_and_full_encode_agree() {
    let Fixture { generator, capture } = fixture();
    let schema: CompactSeedSchema = generator.schema().unwrap();
    assert_eq!(value(schema), generator.call("schema", json!({})).unwrap());
    assert_eq!(
        value(generator.schema().unwrap()),
        sdk::call(
            "artifact.compact_schema",
            json!({"artifact": value(generator.artifact()).to_string()}),
        )
        .unwrap()
    );

    let lazy = generator
        .decode(capture, None, DecodeOptions::default())
        .unwrap();
    let full = generator.decode(capture, None, FULL).unwrap();
    assert!(lazy.metadata.signals.is_empty());
    assert!(!lazy.metadata.exact_reencode);
    assert!(full.metadata.exact_reencode);
    assert_eq!(lazy.metadata.decoded, 2);
    assert_eq!(lazy.metadata.missing, 0);
    assert_eq!(lazy.positional_signals, full.positional_signals);
    assert_eq!(lazy.metadata.original_report, full.metadata.original_report);
    assert_eq!(
        value(&lazy),
        generator
            .call("decode", json!({"capture": value(capture)}))
            .unwrap()
    );
    assert_eq!(
        value(&lazy),
        generator
            .call(
                "decode",
                json!({
                    "capture": value(capture), "session": null, "verify": null, "metadata": null
                })
            )
            .unwrap()
    );
    assert_eq!(
        value(&full),
        generator
            .call(
                "decode",
                json!({"capture": value(capture).to_string(), "verify": true, "metadata": true})
            )
            .unwrap()
    );
    for (name, detail) in &full.metadata.signals {
        let typed = generator.explain(&lazy, name, None, true).unwrap();
        assert_eq!(value(&typed), value(detail));
        assert_eq!(
            value(typed),
            generator
                .call(
                    "explain",
                    json!({"decoded": value(&lazy).to_string(), "signal": name, "verify": true})
                )
                .unwrap()
        );
    }
    let typed = generator.encode_decoded(&lazy, None).unwrap();
    assert_eq!(value(&typed), value(capture));
    assert_eq!(
        value(&typed),
        value(generator.encode(&value(&lazy), None).unwrap())
    );
    assert_eq!(
        value(typed),
        generator
            .call("encode", json!({"decoded": value(&lazy).to_string()}))
            .unwrap()
    );
}

#[test]
fn edits_and_signals_only_encode_preserve_fields_without_mutating_inputs() {
    let Fixture { generator, capture } = fixture();
    let original = generator
        .decode(capture, None, DecodeOptions::default())
        .unwrap();
    let mut edited = original.clone();
    edited
        .positional_signals
        .get_mut("DeviceMemory")
        .unwrap()
        .insert("deviceMemory".into(), json!(8));
    let headers = generator.encode_decoded(&edited, None).unwrap();
    let again = generator.decode(&headers, None, FULL).unwrap();
    assert_eq!(again.positional_signals, edited.positional_signals);
    assert_eq!(
        again.metadata.original_report.positional_signals[0],
        original.metadata.original_report.positional_signals[0]
    );
    assert_eq!(
        original.positional_signals["DeviceMemory"]["deviceMemory"],
        9
    );
    let bare = json!({"positionalSignals": edited.positional_signals});
    let typed = generator.encode(&bare, None).unwrap();
    let json: CaptureHeaders = serde_json::from_value(
        generator
            .call(
                "encode",
                json!({"signals": bare["positionalSignals"].to_string()}),
            )
            .unwrap(),
    )
    .unwrap();
    // Fresh prefixes need not match; the decoded documents' fields must.
    for headers in [typed, json] {
        let decoded = generator.decode(&headers, None, FULL).unwrap();
        assert_eq!(decoded.positional_signals, edited.positional_signals);
    }
    assert_eq!(
        value(generator.encode_decoded(&original, None).unwrap()),
        value(capture)
    );
}

#[test]
fn session_replacements_are_per_call_and_shared_by_all_generator_methods() {
    let Fixture { generator, capture } = fixture();
    let decoded = generator
        .decode(capture, None, DecodeOptions::default())
        .unwrap();
    let mut fresh: Session = generator.artifact().layout.session.clone();
    fresh.keys = [7; 8];
    fresh.alphabet = fresh.alphabet.chars().rev().collect();
    fresh.transaction_token = "another-session".into();
    let bare = json!({"positionalSignals": decoded.positional_signals});
    let headers = generator.encode(&bare, Some(&fresh)).unwrap();
    assert!(generator.decode(&headers, None, FULL).is_err());
    let output = generator.decode(&headers, Some(&fresh), FULL).unwrap();
    assert_eq!(output.positional_signals, decoded.positional_signals);
    assert_eq!(
        value(&output),
        generator
            .call(
                "decode",
                json!({
                    "capture": value(&headers), "session": value(&fresh).to_string(),
                    "metadata": true, "verify": true
                })
            )
            .unwrap()
    );
    assert_eq!(
        value(generator.encode_decoded(&output, Some(&fresh)).unwrap()),
        value(&headers)
    );
    assert_eq!(
        generator
            .call(
                "encode",
                json!({"decoded": value(&output), "session": value(&fresh)})
            )
            .unwrap(),
        value(&headers)
    );
    assert_eq!(
        value(generator.explain(&output, "DeviceMemory", Some(&fresh), true).unwrap()),
        generator.call("explain", json!({"decoded": value(&output), "signal": "DeviceMemory", "session": value(&fresh), "verify": true})).unwrap()
    );
    assert_eq!(
        value(
            generator
                .decode(capture, None, DecodeOptions::default())
                .unwrap()
        ),
        value(decoded)
    );
}

#[test]
fn clone_and_shared_references_survive_drop_and_concurrent_edits() {
    let Fixture { generator, capture } = fixture();
    let owned = Generator::load(&serde_json::to_vec(generator.artifact()).unwrap()).unwrap();
    let surviving = owned.clone();
    assert!(std::ptr::eq(owned.artifact(), surviving.artifact()));
    drop(owned);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (10..14)
            .map(|n| {
                let clone = surviving.clone();
                let shared = &surviving;
                scope.spawn(move || {
                    let mut decoded = clone
                        .decode(capture, None, DecodeOptions::default())
                        .unwrap();
                    decoded
                        .positional_signals
                        .get_mut("DeviceMemory")
                        .unwrap()
                        .insert("deviceMemory".into(), json!(n));
                    let encoded = clone.encode_decoded(&decoded, None).unwrap();
                    assert_eq!(
                        shared
                            .decode(&encoded, None, FULL)
                            .unwrap()
                            .positional_signals,
                        decoded.positional_signals
                    );
                    assert_eq!(
                        value(clone.schema().unwrap()),
                        value(shared.schema().unwrap())
                    );
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
    });
    assert_eq!(
        surviving
            .decode(capture, None, FULL)
            .unwrap()
            .positional_signals["DeviceMemory"]["deviceMemory"],
        9
    );
}

#[test]
fn u64_number_bits_survive_artifact_load_json_explain_and_encode() {
    let Fixture { generator, capture } = fixture();
    // Pin the fixture's numeric writer to an exact literal with the low bit set.
    // This is a real recipe constraint, not an unrelated JSON sentinel.
    let bits = (-1.5000000000000002f64).to_bits();
    assert!(bits > i64::MAX as u64);
    assert_ne!(bits, (bits as f64) as u64);
    let mut artifact: DecodeArtifact = generator.artifact().clone();
    let slot = &mut artifact.schema.wire.as_mut().unwrap().slots[1];
    let program = match &mut slot.programs[0] {
        Support::Supported { program } | Support::RequiresInitializer { program, .. } => program,
        Support::Unsupported { .. } => panic!("fixture writer must be supported"),
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
    let document = value(&artifact);
    assert!(serde_json::to_string(&document)
        .unwrap()
        .contains(&bits.to_string()));
    let loaded = Generator::load(&serde_json::to_vec(&document).unwrap()).unwrap();
    assert_eq!(value(loaded.artifact()), document);
    let named = sdk::artifact::from_artifact(artifact).unwrap();
    let named_reload = sdk::artifact::load(&named.to_json().unwrap()).unwrap();
    assert_eq!(value(named_reload.artifact()), document);
    let original = generator.decode(capture, None, FULL).unwrap();
    let mut payload = original;
    payload
        .positional_signals
        .get_mut("DeviceMemory")
        .unwrap()
        .insert("deviceMemory".into(), json!(f64::from_bits(bits)));
    let encoded = loaded.encode_decoded(&payload, None).unwrap();
    let decoded = loaded.decode(&encoded, None, FULL).unwrap();
    assert_eq!(
        decoded.positional_signals["DeviceMemory"]["deviceMemory"]
            .as_f64()
            .unwrap()
            .to_bits(),
        bits
    );
    let explained = loaded
        .explain(&decoded, "DeviceMemory", None, true)
        .unwrap();
    assert_eq!(
        value(&explained),
        loaded
            .call(
                "explain",
                json!({"decoded": value(&decoded), "signal": "DeviceMemory", "verify": true})
            )
            .unwrap()
    );
    assert!(value(explained).to_string().contains(&bits.to_string()));
    assert_eq!(
        value(loaded.encode_decoded(&decoded, None).unwrap()),
        value(&encoded)
    );
    assert_eq!(
        value(sdk::encode(&named_reload, &decoded, None).unwrap()),
        value(&encoded)
    );
    assert_eq!(
        sdk::decode(&named_reload, &encoded, None, FULL)
            .unwrap()
            .positional_signals["DeviceMemory"]["deviceMemory"]
            .as_f64()
            .unwrap()
            .to_bits(),
        bits
    );
}

#[test]
fn parsing_validation_and_error_codes_are_stable() {
    let Fixture { generator, capture } = fixture();
    for request in ["{", "null", "[]", "true", "1", "\"{}\""] {
        for response in [
            sdk::call_json("version", request),
            generator.call_json("schema", request),
        ] {
            let response: Value = serde_json::from_str(&response).unwrap();
            assert_eq!(response["ok"], false);
            assert_eq!(response["error"]["code"], "invalid_request");
        }
    }
    for request in ["", " \n ", "{}"] {
        let response: Value =
            serde_json::from_str(&generator.call_json("schema", request)).unwrap();
        assert_eq!(response["ok"], true);
        assert_eq!(response["value"], value(generator.schema().unwrap()));
    }
    let decoded = generator
        .decode(capture, None, DecodeOptions::default())
        .unwrap();
    for (method, request) in [
        ("decode", json!({})),
        ("decode", json!({"capture": "{"})),
        ("decode", json!({"capture": value(capture), "verify": 1})),
        ("decode", json!({"capture": value(capture), "session": []})),
        (
            "decode",
            json!({"capture": {"siteId": "wrong", "headers": {}}}),
        ),
        (
            "explain",
            json!({"decoded": value(&decoded), "signal": "Unknown"}),
        ),
        (
            "explain",
            json!({"decoded": "null", "signal": "DeviceMemory"}),
        ),
        ("encode", json!({"decoded": {}, "signals": {}})),
        ("encode", json!({"signals": "{"})),
        ("encode", json!({"signals": []})),
        ("encode", json!({"decoded": {"siteId": "wrong"}})),
    ] {
        let error = generator.call(method, request.clone()).unwrap_err();
        assert_eq!(error.code, "invalid_request", "{method}: {error}");
        let response: Value =
            serde_json::from_str(&generator.call_json(method, &request.to_string())).unwrap();
        assert_eq!(response["error"], value(error));
    }
    let mut damaged = decoded.clone();
    damaged.metadata.original_report.positional_signals[1]
        .as_mut()
        .unwrap()
        .1
        .clear();
    assert_eq!(
        generator
            .explain(&damaged, "DeviceMemory", None, true)
            .unwrap_err()
            .code,
        "failed"
    );
    let mut wrong_site = decoded;
    wrong_site.site_id = "wrong".into();
    assert_eq!(
        generator
            .explain(&wrong_site, "DeviceMemory", None, false)
            .unwrap_err()
            .code,
        "invalid_request"
    );
    assert_eq!(
        generator
            .call(
                "explain",
                json!({"decoded": value(&wrong_site), "signal": "DeviceMemory"})
            )
            .unwrap_err()
            .code,
        "invalid_request"
    );
    assert_eq!(
        generator
            .encode_decoded(&wrong_site, None)
            .unwrap_err()
            .code,
        "invalid_request"
    );
    for method in ["unknown", "schema\0ignored"] {
        assert_eq!(
            generator.call(method, json!({})).unwrap_err().code,
            "unknown_command"
        );
    }
    for bytes in [b"".as_slice(), b"{", b"null", b"{}"] {
        assert_eq!(Generator::load(bytes).err().unwrap().code, "failed");
    }
    let compact = serde_json::to_vec(&generator.schema().unwrap()).unwrap();
    assert_eq!(Generator::load(&compact).err().unwrap().code, "failed");
}

#[test]
fn every_advertised_stateless_command_is_accessible() {
    assert_eq!(sdk::COMMANDS.len(), 21);
    let version = sdk::call("version", json!({})).unwrap();
    assert_eq!(version["commands"], json!(sdk::COMMANDS));
    for command in sdk::COMMANDS {
        if let Err(error) = sdk::call(command, json!({})) {
            assert_ne!(error.code, "unknown_command", "{command}");
            assert_ne!(error.code, "panic", "{command}: {error}");
        }
    }
}
