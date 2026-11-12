//! Static source -> saved schema -> public consumer position regressions.
//! Golden masks are the independent stream word 0xafd349a2 (low byte first),
//! following the fixture README's recurrence and four-byte burn.
use shape::signals::{
    analysis::{analyze_program_signals, SignalFinderOptions},
    payload::{extract::Session, pin, Report},
    wire::{Decoder, NamedDocument, Outcome, SchemaDocument, WireValue},
};

fn fixture(body: &str) -> (String, Vec<u8>, shape::signals::payload::SeedLayout) {
    let hash =
        include_str!("../src/signals/wire/reference/hash.js").replace("local0(", "tokenHash(");
    let stream = include_str!("../src/signals/wire/reference/stream.js")
        .replace("local0(", "streamFactory(")
        .replace("local9", "tokenState");
    let number = include_str!("../src/signals/wire/reference/number.js")
        .replace("local0(", "integerWriter(");
    // Keep the registry-owning session broader than the canonical writer's
    // local binding set, as required by the public main-scope recognizer.
    let bindings = (0..32)
        .map(|i| format!("scope{i}={i}"))
        .collect::<Vec<_>>()
        .join(",");
    let reads = (0..32)
        .map(|i| format!("scope{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let source = include_str!("fixtures/signal-wire-roundtrip/program.js")
        .replace(
            "function Session(token) {",
            &format!("function Session(token) {{let {bindings};void [{reads}];"),
        )
        .replace("__HASH__", &hash)
        .replace("__STREAM__", &stream)
        .replace("let tokenState;", &format!("let tokenState;{number}"))
        .replace("b[4]=9^mask();", &format!("let p=3;{body}"));
    let session = Session {
        transaction_token: "ab".into(),
        alphabet: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".into(),
        shuffle_key: vec![],
        keys: [0; 8],
        nonce: None,
    };
    let pin = pin(&source, &session).unwrap().layout;
    let analysis = analyze_program_signals(
        &source,
        &SignalFinderOptions {
            layouts_only: true,
            finder_roles: true,
        },
    )
    .unwrap();
    let saved = serde_json::to_vec(&analysis.report).unwrap();
    // The actual analyzer JSON supplies all layouts and source-plan evidence.
    let doc = SchemaDocument::from_json(&saved).unwrap();
    assert_eq!(
        serde_json::to_value(&doc).unwrap(),
        serde_json::from_slice::<serde_json::Value>(&saved).unwrap()
    );
    (source, saved, pin)
}
fn report(bytes: Vec<u8>) -> Report {
    Report {
        positional_signals: vec![None, Some((vec![], bytes))],
        errors: vec![],
        signals: vec![],
        keyed_signals: vec![],
        seeds: [0; 2],
        keysets: vec![],
    }
}
fn golden(body: &str, tail: &[u8], values: Vec<WireValue>) {
    let (source, saved, pin) = fixture(body);
    let decoder = Decoder::load(&saved, &source, &pin).unwrap();
    let bytes: Vec<_> = [1, 2, 3, 4]
        .into_iter()
        .chain(tail.iter().copied())
        .collect();
    let report = report(bytes.clone());
    let slots = decoder.decode_report(&report).unwrap();
    let expected = WireValue::Sequence(values);
    assert_eq!(
        slots[1].outcome,
        Outcome::Decoded {
            value: expected.clone(),
            exact_reencode: true
        },
        "{body}"
    );
    assert_eq!(
        decoder.encode_slot(1, &expected, 0x01020304).unwrap(),
        bytes
    );
    let named = decoder.decode_named_report(&report).unwrap();
    let restored = NamedDocument::from_json(&serde_json::to_vec(&named).unwrap())
        .unwrap()
        .restore_original_report();
    assert_eq!(restored, report);
}
fn number(n: f64) -> WireValue {
    WireValue::NumberBits(n.to_bits())
}
fn refused(body: &str) {
    let (source, saved, pin) = fixture(body);
    let decoder = Decoder::load(&saved, &source, &pin).unwrap();
    assert!(
        matches!(
            decoder
                .decode_report(&report(vec![1, 2, 3, 4, 0xa7]))
                .unwrap()[1]
                .outcome,
            Outcome::Unsupported { .. }
        ),
        "{body}"
    );
}

#[test]
fn writer_return_arithmetic_is_cached_before_next_store() {
    golden(
        "let q=integerWriter(mask,3,b,p)+1;b[q]=5^mask();b[q+1]=9^mask();",
        &[0xa1, 0x4c, 0xda],
        vec![number(3.), WireValue::Byte(5), WireValue::Byte(9)],
    );
}
#[test]
fn fixed_offsets_keep_the_original_index_value_version() {
    golden(
        "p=integerWriter(mask,3,b,p);let q=p+1;b[q]=5^mask();b[q+1]=9^mask();b[q+2]=7^mask();",
        &[0xa1, 0x4c, 0xda, 0xa8],
        vec![
            number(3.),
            WireValue::Byte(5),
            WireValue::Byte(9),
            WireValue::Byte(7),
        ],
    );
}
#[test]
fn constant_append_and_postincrement_have_distinct_indices() {
    golden(
        "p=4;b[p++]=3^mask();b[p++]=5^mask();b[p]=9^mask();",
        &[0xa1, 0x4c, 0xda],
        vec![WireValue::Byte(3), WireValue::Byte(5), WireValue::Byte(9)],
    );
}
#[test]
fn conditional_result_cursor_survives_join_and_arithmetic() {
    golden(
        "let v=null;p=(v==null?(b[4]=130^mask(),4):integerWriter(mask,v,b,3))+1;b[p]=5^mask();",
        &[0x20, 0x4c],
        vec![
            WireValue::Choice {
                arm: 0,
                value: Box::new(WireValue::Byte(130)),
            },
            WireValue::Byte(5),
        ],
    );
}
#[test]
fn cursor_holder_kills_do_not_extend_dead_value_lifetimes() {
    golden(
        "p=integerWriter(mask,3,b,p);let q=p;p={};p=integerWriter(mask,5,b,q);b[++p]=9^mask();",
        &[0xa1, 0x4c, 0xda],
        vec![number(3.), number(5.), WireValue::Byte(9)],
    );
}
#[test]
fn reverse_nested_source_loops_still_append_in_wire_order() {
    golden(
        "for(let i=2;i>0;--i){for(let j=0;j<2;++j){b[++p]=3^mask();}}",
        &[0xa1, 0x4a, 0xd0, 0xac],
        vec![WireValue::Repeat(vec![
            WireValue::Repeat(vec![WireValue::Byte(3), WireValue::Byte(3)]),
            WireValue::Repeat(vec![WireValue::Byte(3), WireValue::Byte(3)]),
        ])],
    );
}
#[test]
fn true_gaps_overwrites_and_unaccounted_draws_remain_refused() {
    for body in [
        "b[5]=5^mask();",
        "b[4]=3^mask();b[4]=5^mask();",
        "let q=integerWriter(mask,3,b,p)+2;b[q]=5^mask();",
        "mask();b[++p]=5^mask();",
    ] {
        refused(body);
    }
}
#[test]
fn captured_cursor_alias_and_conditional_clobbers_remain_refused() {
    for body in [
        "let q=p;function mutate(){q=99;}mutate();b[++q]=5^mask();",
        "let q=p;if(flag)q=99;b[++q]=5^mask();",
        "let q=p;consume(()=>q++);b[++q]=5^mask();",
    ] {
        refused(body);
    }
}

#[test]
fn packed_flush_and_partial_tail_keep_one_cursor_across_empty_arms() {
    for descending in [false, true] {
        let loop_body = if descending {
            "let i=size;for(;;){--i;if(!(i>=0))break;++pack;if(8===pack){b[++p]=5^mask();pack=0;}}"
        } else {
            "for(let i=0;i<size;++i){++pack;if(8===pack){b[++p]=5^mask();pack=0;}}"
        };
        let body = format!("let items=[];let size=items.length;let pack=0;p=integerWriter(mask,size,b,p);{loop_body}if(0!==pack){{b[++p]=6^mask();}}b[++p]=9^mask();");
        let iterations = (0..9)
            .map(|i| WireValue::Choice {
                arm: usize::from(i != 7),
                value: Box::new(if i == 7 {
                    WireValue::Byte(5)
                } else {
                    WireValue::Sequence(vec![])
                }),
            })
            .collect();
        golden(
            &body,
            &[0xab, 0x4c, 0xd5, 0xa6],
            vec![
                number(9.),
                WireValue::Repeat(iterations),
                WireValue::Choice {
                    arm: 0,
                    value: Box::new(WireValue::Byte(6)),
                },
                WireValue::Byte(9),
            ],
        );
    }
}
#[test]
fn cursor_compound_advance_after_fixed_append_is_proven() {
    golden(
        "p=integerWriter(mask,3,b,p);b[p+1]=5^mask();p+=2;b[p]=9^mask();",
        &[0xa1, 0x4c, 0xda],
        vec![number(3.), WireValue::Byte(5), WireValue::Byte(9)],
    );
}
#[test]
fn nested_writer_returns_and_same_holder_rhs_precede_lhs_kill() {
    golden("p=integerWriter(mask,2,b,integerWriter(mask,1,b,p));let cached=p;p=null;b[cached+1]=5^mask();", &[0xa3,0x4b,0xd6],vec![number(1.),number(2.),WireValue::Byte(5)]);
}
#[test]
fn additional_rhs_draw_is_not_hidden_by_index_proof() {
    refused("b[++p]=5^mask()^mask();");
}

#[test]
fn source_bound_position_evidence_cannot_be_retargeted_in_saved_json() {
    let (source, saved, pin) = fixture("let q=integerWriter(mask,3,b,p)+1;b[q]=5^mask();");
    let mut document: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    let position = document
        .pointer_mut("/wire/slots/1/contract/emissions/0/grammar/items/2/position")
        .unwrap();
    *position = serde_json::json!("999");
    assert!(Decoder::load(&serde_json::to_vec(&document).unwrap(), &source, &pin).is_err());
}
