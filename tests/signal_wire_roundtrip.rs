//! Hermetic source -> actual per-seed JSON -> public supported/named decoder.
//! Golden signal inputs are independently calculated, never encoder output.
use serde_json::{json, Value};
use shape::signals::{
    analysis::{analyze_program_signals, SignalFinderOptions},
    payload::{extract::Session, pin, superpack::Value as Raw, Report, SeedLayout},
    wire::{
        Decoder, NamedDocument, NamedStatus, Outcome, SchemaDocument, Support, WireError, WireValue,
    },
};

const SEED: u32 = 0x01020304;
// See the fixture README for the independent hash/recurrence calculation.
// The mock document has hasStorageAccess, but lacks requestStorageAccess:
// flags = 1, masked by 0xa2. The other collector writes literal 9.
const FLAGS: [u8; 5] = [1, 2, 3, 4, 0xa3];
const LITERAL: [u8; 5] = [1, 2, 3, 4, 0xab];

fn source() -> String {
    let hash =
        include_str!("../src/signals/wire/reference/hash.js").replace("local0(", "tokenHash(");
    let stream = include_str!("../src/signals/wire/reference/stream.js")
        .replace("local0(", "streamFactory(")
        .replace("local9", "tokenState");
    include_str!("fixtures/signal-wire-roundtrip/program.js")
        .replace("__HASH__", &hash)
        .replace("__STREAM__", &stream)
}
fn session() -> Session {
    Session {
        transaction_token: "ab".into(),
        alphabet: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".into(),
        shuffle_key: vec![],
        keys: [0; 8],
        nonce: None,
    }
}
fn schema(source: &str) -> Vec<u8> {
    // Exactly `shape signals <program> --json --layouts-only`: no projection,
    // synthetic wire metadata, overwritten certificates or routing injection.
    let analysis = analyze_program_signals(
        source,
        &SignalFinderOptions {
            layouts_only: true,
            finder_roles: true,
        },
    )
    .unwrap();
    serde_json::to_vec(&analysis.report).unwrap()
}
fn fixture() -> (String, Vec<u8>, SeedLayout, Decoder) {
    let source = source();
    let pin = pin(&source, &session()).unwrap().layout;
    let saved = schema(&source);
    let document = SchemaDocument::from_json(&saved).unwrap();
    let wire = document.wire.as_ref().unwrap();
    assert_eq!(wire.slots.len(), 2);
    assert!(
        wire.algorithms.hash.codec.is_some(),
        "{:?}",
        wire.algorithms.hash
    );
    assert!(
        wire.algorithms.stream.codec.is_some(),
        "{:?}",
        wire.algorithms.stream
    );
    for slot in &wire.slots {
        assert_eq!(slot.programs.len(), 1);
        assert!(
            matches!(&slot.programs[0], Support::Supported { program } if program.salt == 17),
            "{:?}",
            slot.programs
        );
    }
    assert_eq!(
        serde_json::to_value(&document).unwrap(),
        serde_json::from_slice::<Value>(&saved).unwrap()
    );
    let decoder = Decoder::load(&saved, &source, &pin).unwrap();
    (source, saved, pin, decoder)
}
fn report() -> Report {
    Report {
        errors: vec![Raw::Null],
        positional_signals: vec![
            Some((vec![], FLAGS.to_vec())),
            Some((vec![], LITERAL.to_vec())),
        ],
        signals: vec![Raw::Bool(true)],
        keyed_signals: vec![(17, None), (18, Some((vec![Raw::Null], vec![6, 7, 8, 9])))],
        seeds: [19, 23],
        keysets: vec![],
    }
}
fn restore_named(decoder: &Decoder, report: &Report) -> Report {
    let named = decoder.decode_named_report(report).unwrap();
    NamedDocument::from_json(&serde_json::to_vec(&named).unwrap())
        .unwrap()
        .restore_original_report()
}

#[test]
fn public_number_factory_resolves_reused_callable_before_returned_cursor() {
    let writer = include_str!("../src/signals/wire/reference/number.js")
        .replace("local0(", "integerWriter(");
    let source = source()
        .replacen(
            "function main() {",
            &format!("function main() {{\n{writer}"),
            1,
        )
        .replace(
            "function Session(token) {",
            "function Session(token) {\n\
             let sharedNumber; sharedNumber=integerWriter;\n\
             const navigator={deviceMemory:9};",
        )
        .replace(
            "b[4]=9^mask();",
            "let callable; callable=sharedNumber;\n\
            callable=callable(mask,navigator.deviceMemory,b,3); callable=0;",
        );
    let saved = schema(&source);
    let pin = pin(&source, &session()).unwrap().layout;
    let decoder = Decoder::load(&saved, &source, &pin).unwrap();
    let report = report();
    let values = decoder.decode_report(&report).unwrap();
    // Independent golden: integer 9 is one sign/magnitude byte, masked by
    // the first stream byte 0xa2 (the fixture README derives that recurrence).
    let value = WireValue::Sequence(vec![WireValue::NumberBits(9f64.to_bits())]);
    assert_eq!(
        values[1].outcome,
        Outcome::Decoded {
            value: value.clone(),
            exact_reencode: true,
        }
    );
    assert_eq!(decoder.encode_slot(1, &value, SEED).unwrap(), LITERAL);
    let named = decoder.decode_named_report(&report).unwrap();
    let memory = &named.signals["DeviceMemory"];
    assert_eq!(memory.status, NamedStatus::Decoded);
    assert_eq!(memory.fields.as_ref().unwrap()["deviceMemory"], json!(9));
    assert_eq!(restore_named(&decoder, &report), report);
}

#[test]
fn public_source_json_load_decodes_supported_physical_and_named_golden() {
    let (_, _, pin, decoder) = fixture();
    let report = report();
    let physical = decoder.decode_report(&report).unwrap();
    for (index, (byte, golden)) in [(1, FLAGS), (9, LITERAL)].into_iter().enumerate() {
        let value = WireValue::Sequence(vec![WireValue::Byte(byte)]);
        assert_eq!(physical[index].positional, index);
        assert_eq!(physical[index].descriptor, index as u32);
        assert_eq!(physical[index].captured_seed, Some(SEED));
        assert_eq!(physical[index].raw.as_deref(), Some(golden.as_slice()));
        assert_eq!(
            physical[index].outcome,
            Outcome::Decoded {
                value: value.clone(),
                exact_reencode: true
            }
        );
        assert_eq!(decoder.encode_slot(index, &value, SEED).unwrap(), golden);
    }
    let named = decoder.decode_named_report(&report).unwrap();
    let flags = &named.signals["StorageAccessApiPresence"];
    assert_eq!(flags.status, NamedStatus::Decoded);
    assert_eq!(
        serde_json::to_value(flags.fields.as_ref().unwrap()).unwrap(),
        json!({
            "hasStorageAccess": true, "requestStorageAccess": false
        })
    );
    assert_eq!(flags.metadata["capturedSeed"], SEED);
    assert_eq!(flags.metadata["bytes"], json!(FLAGS));
    let restored = restore_named(&decoder, &report);
    assert_eq!(restored, report);
    assert_eq!(restored.to_payload().unwrap(), report.to_payload().unwrap());

    let payload = pin
        .encode_payload(&report.channels_owned(), report.seeds)
        .unwrap();
    let decoded = pin.decode_payload(&payload).unwrap();
    assert_eq!(decoded, report);
    let restored = restore_named(&decoder, &decoded);
    assert_eq!(
        pin.encode_payload(&restored.channels_owned(), restored.seeds)
            .unwrap(),
        payload
    );
    let headers = pin
        .encode_headers(&report.channels_owned(), report.seeds)
        .unwrap();
    let decoded_headers = pin.decode_headers(&headers).unwrap();
    assert_eq!(decoded_headers.report(), report);
    assert_eq!(
        pin.encode_headers(&restored.channels_owned(), restored.seeds)
            .unwrap(),
        headers
    );
}

#[test]
fn null_slots_and_generic_channels_survive_named_serialization() {
    let (_, _, pin, decoder) = fixture();
    let mut report = report();
    report.positional_signals[0] = None;
    // Retain a generic value channel even in a routed physical slot.
    report.positional_signals[1].as_mut().unwrap().0 = vec![Raw::Bool(false), Raw::Null];
    let physical = decoder.decode_report(&report).unwrap();
    assert_eq!(physical[0].outcome, Outcome::Missing);
    assert!(
        matches!(&physical[1].outcome, Outcome::Unsupported { reason: WireError::Unsupported(reason) }
        if reason == "generic value channel is not a signal byte program")
    );
    let named = decoder.decode_named_report(&report).unwrap();
    assert_eq!(
        named.signals["StorageAccessApiPresence"].status,
        NamedStatus::Missing
    );
    let restored = restore_named(&decoder, &report);
    assert_eq!(restored, report);
    let payload = pin
        .encode_payload(&report.channels_owned(), report.seeds)
        .unwrap();
    assert_eq!(pin.decode_payload(&payload).unwrap(), report);
    assert_eq!(
        pin.encode_payload(&restored.channels_owned(), restored.seeds)
            .unwrap(),
        payload
    );
}

#[test]
fn public_loader_refuses_stale_source_pin_and_forged_metadata() {
    let (source, saved, pin, _) = fixture();
    assert!(matches!(
        Decoder::load(&saved, &(source.clone() + " "), &pin),
        Err(WireError::Source)
    ));
    let mut wrong_pin = pin.clone();
    wrong_pin.cipher.counter += 1;
    assert!(matches!(
        Decoder::load(&saved, &source, &wrong_pin),
        Err(WireError::Pin)
    ));
    for (path, replacement, expected) in [
        ("/source/sha256", json!("stale"), WireError::Source),
        (
            "/wire/slots/0/routing/positional",
            json!(99),
            WireError::Routing,
        ),
        (
            "/wire/algorithms/stream/codec",
            json!("forged"),
            WireError::Evidence,
        ),
    ] {
        let mut changed: Value = serde_json::from_slice(&saved).unwrap();
        *changed.pointer_mut(path).unwrap() = replacement;
        assert!(
            matches!(Decoder::load(&serde_json::to_vec(&changed).unwrap(), &source, &pin), Err(error) if error == expected),
            "mutation {path}"
        );
    }
}

#[test]
fn changed_stream_helper_cannot_fabricate_success_with_fresh_analysis() {
    let (source, saved, pin, _) = fixture();
    let changed = source.replacen("local8 << 26", "local8 << 25", 1);
    assert_ne!(changed, source);
    assert!(matches!(
        Decoder::load(&saved, &changed, &pin),
        Err(WireError::Source)
    ));
    let pin = shape::signals::payload::pin(&changed, &session())
        .unwrap()
        .layout;
    let saved = schema(&changed);
    let document = SchemaDocument::from_json(&saved).unwrap();
    assert!(document
        .wire
        .as_ref()
        .unwrap()
        .algorithms
        .stream
        .codec
        .is_none());
    let decoder = Decoder::load(&saved, &changed, &pin).unwrap();
    for slot in decoder.decode_report(&report()).unwrap() {
        assert!(
            matches!(slot.outcome, Outcome::Unsupported { reason: WireError::Unsupported(ref reason) }
            if reason == "source token hash or stream algorithm is not certified")
        );
    }
    assert!(matches!(
        decoder.encode_slot(0, &WireValue::Sequence(vec![WireValue::Byte(1)]), SEED),
        Err(WireError::Unsupported(_))
    ));
}

#[test]
fn public_decode_refuses_literal_trailing_and_truncated_bytes_precisely() {
    let (_, _, _, decoder) = fixture();
    for (bytes, expected) in [
        (vec![1, 2, 3, 4, 0xaa], WireError::Literal),
        (vec![1, 2, 3, 4, 0xab, 0], WireError::Trailing(1)),
        (vec![1, 2, 3, 4], WireError::Truncated),
        (vec![1, 2, 3], WireError::Truncated),
    ] {
        let mut report = report();
        report.positional_signals[1].as_mut().unwrap().1 = bytes;
        let physical = decoder.decode_report(&report).unwrap();
        assert!(matches!(
            physical[0].outcome,
            Outcome::Decoded {
                exact_reencode: true,
                ..
            }
        ));
        assert_eq!(physical[1].outcome, Outcome::Invalid { reason: expected });
        assert_eq!(restore_named(&decoder, &report), report);
    }
}

#[test]
fn public_inline_store_uses_saved_caller_index_evidence_and_golden_flags() {
    let source=source().replace(
        "b[4]=((\"hasStorageAccess\" in document)|(\"requestStorageAccess\" in document)<<1)^mask();",
        "let cursor=3;((target,index,value,result)=>{target[index]=value;return result;})(b,++cursor,((\"hasStorageAccess\" in document)|(\"requestStorageAccess\" in document)<<1)^mask(),0);",
    );
    let pin = pin(&source, &session()).unwrap().layout;
    let saved = schema(&source);
    let decoder = Decoder::load(&saved, &source, &pin).unwrap();
    let named = decoder.decode_named_report(&report()).unwrap();
    assert_eq!(
        named.signals["StorageAccessApiPresence"].status,
        NamedStatus::Decoded
    );
    assert_eq!(
        serde_json::to_value(
            named.signals["StorageAccessApiPresence"]
                .fields
                .as_ref()
                .unwrap()
        )
        .unwrap(),
        json!({"hasStorageAccess":true,"requestStorageAccess":false})
    );
    let decoded = decoder.decode_report(&report()).unwrap();
    let Outcome::Decoded { value, .. } = &decoded[0].outcome else {
        panic!("{:?}", decoded[0].outcome)
    };
    assert_eq!(decoder.encode_slot(0, value, SEED).unwrap(), FLAGS);
    let mut document = SchemaDocument::from_json(&saved).unwrap();
    let mut changed = false;
    for slot in &mut document.wire.as_mut().unwrap().slots {
        for emission in &mut slot.contract.emissions {
            fn mutate(node: &mut shape::signals::extract::contracts::GrammarNode) -> bool {
                use shape::signals::extract::contracts::GrammarNode;
                match node {
                    GrammarNode::Byte(write) if write.position_span.is_some() => {
                        write.position_span.as_mut().unwrap().end += 1;
                        true
                    }
                    GrammarNode::Sequence { items } => items.iter_mut().any(mutate),
                    _ => false,
                }
            }
            changed |= mutate(&mut emission.grammar);
        }
    }
    assert!(changed);
    assert!(matches!(
        Decoder::load(&serde_json::to_vec(&document).unwrap(), &source, &pin),
        Err(WireError::Evidence)
    ));
}
