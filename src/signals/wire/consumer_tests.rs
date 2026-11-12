//! Synthetic source -> extractor -> existing schema renderer -> saved JSON ->
//! typed loader/routing -> native execution. The full source/pin validation
//! path additionally has refusal tests; these fixtures isolate the signal IR
//! from the unrelated outer payload/header recognizers.
use super::*;
use crate::signals::{
    extract::{contracts::*, encoding::extract_encoding_properties, layout::*},
    schema::{project_contract, render_signal, Routing, SourceSpan},
    wire::{
        algorithms::{float_prefix_proof, proof, AlgorithmProof, AlgorithmReport},
        program::*,
        FloatByteOrder, NamedDocument, TypedDecoderOptions, WireSlot,
    },
};
use oxc_span::Span;
use serde_json::json;

fn synthetic(body: &str) -> (String, SchemaDocument, SeedLayout) {
    synthetic_with_setup(body, "")
}
fn synthetic_with_setup(body: &str, setup: &str) -> (String, SchemaDocument, SeedLayout) {
    synthetic_protocol_options(body, setup, false, false, None)
}
fn synthetic_protocol(
    body: &str,
    setup: &str,
    coalesced: bool,
) -> (String, SchemaDocument, SeedLayout) {
    synthetic_protocol_options(body, setup, coalesced, false, None)
}
fn synthetic_with_number_factory(
    body: &str,
    setup: &str,
    native: bool,
) -> (String, SchemaDocument, SeedLayout) {
    synthetic_protocol_options(body, setup, false, native, None)
}
fn synthetic_protocol_options(
    body: &str,
    setup: &str,
    coalesced: bool,
    native: bool,
    collector: Option<&str>,
) -> (String, SchemaDocument, SeedLayout) {
    let stream = include_str!("reference/stream.js")
        .replace("local0(", "streamFactory(")
        .replace("local9", "tokenState");
    let number = include_str!("reference/number.js").replace("local0(", "integerWriter(");
    let number_source = if native {
        format!(
            "{}\nconst integerWriter=createNativeNumberWriter();",
            include_str!("reference/number-native.js").replace("local0", "integerWriter")
        )
    } else {
        number.clone()
    };
    let string = include_str!("reference/string.js")
        .replace("local0(", "stringFactory(")
        .replace("local10(", "integerWriter(")
        .replace("local15(", "codeUnit(");
    let hash = include_str!("reference/hash.js").replace("local0(", "tokenHash(");
    let writer = if body.contains("sw") {
        "let sw=stringFactory(m);"
    } else {
        ""
    };
    let prefix = "let b=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];";
    let protocol = if coalesced {
        format!("{prefix}seed=streamFactory(seed^17);{setup}")
    } else {
        format!("let m=streamFactory(seed^17);{writer}{setup}{prefix}")
    };
    let default =
        format!("let seed=Math.random()*4294967296|0;{protocol}let p=3;{body}obs.next([[],b]);");
    let collector = collector.unwrap_or(&default);
    let source=format!("let tokenState=6357090;let codeUnit=Array.call.bind(String.prototype.charCodeAt);\n{stream}\n{number_source}\n{string}\n{hash}\nfunction collect(obs){{{collector}}}");
    let span = |name: &str, text: &str| {
        let s = source.find(&format!("function {name}(")).unwrap() as u32;
        SourceSpan::new(s, s + text.len() as u32)
    };
    let stream_span = span("streamFactory", &stream);
    let number_span = span("integerWriter", &number);
    let string_span = span("stringFactory", &string);
    let hash_span = span("tokenHash", &hash);
    let unsupported = AlgorithmProof {
        codec: None,
        span: None,
        sha256: None,
        reason: Some("float path is not certified".into()),
    };
    let algorithms = AlgorithmReport {
        hash: proof(
            Some((hash_span, &hash)),
            include_str!("reference/hash.js"),
            "paired-utf16-h31/1",
            false,
        ),
        stream: proof(
            Some((stream_span, &stream)),
            include_str!("reference/stream.js"),
            "sum-rot26-9-13-le-burn4/1",
            false,
        ),
        integer: proof(
            Some((number_span, &number)),
            include_str!("reference/number.js"),
            "sign-magnitude-5-7/1",
            true,
        ),
        float_prefix: float_prefix_proof(
            Some((number_span, &number)),
            include_str!("reference/number.js"),
        ),
        string: proof(
            Some((string_span, &string)),
            include_str!("reference/string.js"),
            "modified-utf8-intern-129/1",
            false,
        ),
        float: unsupported,
    };
    assert!(
        algorithms.hash.codec.is_some()
            && algorithms.stream.codec.is_some()
            && algorithms.integer.codec.is_some()
            && algorithms.string.codec.is_some()
    );
    let collector = source.find("function collect(").unwrap() as u32;
    let mut number_helpers = vec![Span::new(number_span.start, number_span.end)];
    if native {
        let alias = source
            .find("const integerWriter=createNativeNumberWriter();")
            .unwrap() as u32
            + 6;
        number_helpers.push(Span::new(alias, alias + "integerWriter".len() as u32));
    }
    let mut report = extract_signal_layouts(&SignalLayoutRequest {
        source: &source,
        helpers: HelperBindings {
            number_writer: number_helpers,
            string_writer: vec![Span::new(string_span.start, string_span.end)],
            prng_factory: vec![Span::new(stream_span.start, stream_span.end)],
            ..Default::default()
        },
        collectors: vec![CollectorRequest {
            function: Span::new(collector, source.len() as u32),
            observers: ObserverWiring::Direct { signal: 23 },
            site: EmissionSite::ObserverNext,
            forwarded_parameter: None,
        }],
        limits: SignalLayoutLimits::default(),
    })
    .unwrap();
    if native {
        // This IR unit fixture supplies the exact factory-return linkage that
        // the layout extractor does not yet follow. It is NOT a public-loader
        // proof: production compilation must continue to refuse that missing
        // linkage until the primary extractor proves it independently.
        fn link(node: &mut GrammarNode, helper: SourceSpan) {
            match node {
                GrammarNode::Number(w) => w.writer_factory = Some(helper),
                GrammarNode::Sequence { items } => {
                    for item in items {
                        link(item, helper);
                    }
                }
                _ => {}
            }
        }
        link(&mut report.signals[0].emissions[0].grammar, number_span);
    }
    let contract = SignalContract {
        descriptor: 23,
        producer: Producer {
            kind: ProducerKind::Direct { wrapper: None },
            body: Some(SourceSpan::new(collector, source.len() as u32)),
            registrations: vec![],
            arguments: vec![],
        },
        status: ContractStatus::Complete,
        reasons: vec![],
        emissions: report.signals[0]
            .emissions
            .iter()
            .map(|emission| EmissionContract {
                site: emission.origin.into(),
                forwarding_sites: vec![],
                quote: String::new(),
                route: emission.route.clone(),
                phase: None,
                payload: emission.variant.clone(),
                stream: emission.mask.as_ref().map(|mask| StreamContract {
                    argument: mask.argument.clone(),
                    seed_word: mask.seed_word.clone(),
                    salt: mask.xor_source.clone(),
                    salt_span: Some(mask.xor_span.into()),
                    instance: mask.instance,
                    seed_binding: mask.seed_binding,
                }),
                rendered: emission.grammar.render(),
                grammar: emission.grammar.clone(),
            })
            .collect(),
        lifecycle: vec![],
        accounting: vec![],
        input_classes: vec![],
    };
    let programs = crate::signals::wire::programs_for(&contract, &algorithms, &source);
    let mut second = contract.clone();
    second.descriptor = 7;
    let identity = SourceIdentity {
        sha256: algorithms::digest(source.as_bytes()),
        bytes: source.len(),
    };
    let wire = WireSchema {
        initialization: None,
        version: WIRE_VERSION,
        source: identity.clone(),
        encoding: extract_encoding_properties(&source).unwrap(),
        number_float: Some(super::super::float::certify(&source, &algorithms.integer)),
        algorithms: algorithms.clone(),
        slots: vec![
            WireSlot {
                address: "direct:23".into(),
                routing: WireRoute::Positional { positional: 0 },
                contract: contract.clone(),
                source_plan: contract
                    .emissions
                    .iter()
                    .map(|emission| {
                        crate::signals::wire::control::source_plan(&source, emission, &algorithms)
                    })
                    .collect(),
                programs,
                naming: None,
            },
            WireSlot {
                address: "direct:7".into(),
                routing: WireRoute::Positional { positional: 1 },
                contract: second,
                source_plan: vec![],
                programs: vec![],
                naming: None,
            },
        ],
    };
    let mut old =
        render_signal(&project_contract(&contract), &Routing::Positional(0), None).unwrap();
    old["address"] = json!("direct:23");
    let description: DescriptiveSignal = serde_json::from_value(old).unwrap();
    let document = SchemaDocument {
        seed: None,
        source: identity.clone(),
        signals: BTreeMap::from([("z-first".into(), description)]),
        diagnostics: json!({}),
        wire: Some(wire),
    };
    let pin:SeedLayout=serde_json::from_value(json!({"format":"shape-seed-layout/1","source":identity,"session":{"transactionToken":"ab","alphabet":"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/","shuffleKey":[],"keys":[0,0,0,0,0,0,0,0]},"cipher":{"ivs":[0,0,0,0],"counter":0,"double_rounds":1},"header":{"name":"x-example-a","prefix":"x-example-","checksum":"unused","b":false,"f":false,"c":"","d":"","z":""},"keysets":[],"report":{"errors":[],"positionalSignals":[{"slot":0,"descriptor":23,"ctor":true,"priority":false},{"slot":1,"descriptor":7,"ctor":true,"priority":false}],"signals":[],"keyedSignals":[]}})).unwrap();
    (source, document, pin)
}
fn load(document: &SchemaDocument, pin: &SeedLayout) -> Decoder {
    let saved = serde_json::to_vec(document).unwrap();
    let loaded = SchemaDocument::from_json(&saved).unwrap_or_else(|_| {
        panic!(
            "synthetic schema parser: {}",
            serde_json::from_slice::<SchemaDocument>(&saved).unwrap_err()
        )
    });
    Decoder::validated(loaded.signals, loaded.wire.unwrap(), pin).unwrap()
}
fn report(bytes: Vec<u8>) -> Report {
    Report {
        errors: vec![],
        positional_signals: vec![Some((vec![], bytes)), None],
        signals: vec![],
        keyed_signals: vec![],
        seeds: [91, 92],
        keysets: vec![],
    }
}
fn sequence(values: Vec<WireValue>) -> WireValue {
    WireValue::Sequence(values)
}

#[test]
fn saved_schema_preserves_old_fields_and_routes_by_explicit_index() {
    let (_, document, pin) = synthetic("p=integerWriter(m,value,b,p);b[++p]=5^m();");
    let saved = serde_json::to_value(&document).unwrap();
    assert_eq!(saved["signals"]["z-first"]["order"], json!(["f1", "f2"]));
    assert_eq!(saved["signals"]["z-first"]["mask"], 17);
    assert_eq!(
        saved["signals"]["z-first"]["fields"]["f1"]["type"],
        "number"
    );
    assert_eq!(
        saved["signals"]["z-first"]["fields"]["f1"]["maskMode"],
        "inherit"
    );
    assert_eq!(saved["wire"]["version"], 1);
    let decoder = load(&document, &pin);
    let value = sequence(vec![
        WireValue::NumberBits((-0.0f64).to_bits()),
        WireValue::Byte(5),
    ]);
    let bytes = decoder.encode_slot(0, &value, 0x01020304).unwrap();
    let decoded = decoder.decode_report(&report(bytes.clone())).unwrap();
    assert_eq!(
        decoded[0].outcome,
        Outcome::Decoded {
            value,
            exact_reencode: true
        }
    );
    assert_eq!(decoded[0].captured_seed, Some(0x01020304));
    assert_eq!(decoded[0].raw, Some(bytes));
    assert_eq!(decoded[1].outcome, Outcome::Missing);
    assert_eq!(decoded[1].raw, None);
}
#[test]
fn known_stream_and_sign_magnitude_vector_is_independent_of_interpreter() {
    let (_, document, pin) = synthetic("p=integerWriter(m,value,b,p);b[++p]=5^m();");
    let decoder = load(&document, &pin);
    // Golden vector calculated from the published recurrence, one burned
    // little-endian word, and plaintext sign/magnitude bytes [0x40, 0x05].
    let golden = vec![1, 2, 3, 4, 0xe2, 0x4c];
    let decoded = decoder.decode_report(&report(golden.clone())).unwrap();
    assert_eq!(
        decoded[0].outcome,
        Outcome::Decoded {
            value: sequence(vec![
                WireValue::NumberBits((-0.0f64).to_bits()),
                WireValue::Byte(5)
            ]),
            exact_reencode: true
        }
    );
}
#[test]
fn exact_consumption_literal_truncation_type_and_number_refusals() {
    let (_, document, pin) = synthetic("p=integerWriter(m,value,b,p);b[++p]=5^m();");
    let decoder = load(&document, &pin);
    let value = sequence(vec![
        WireValue::NumberBits(3f64.to_bits()),
        WireValue::Byte(5),
    ]);
    let bytes = decoder.encode_slot(0, &value, 4).unwrap();
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(matches!(
        decoder.decode_report(&report(trailing)).unwrap()[0].outcome,
        Outcome::Invalid {
            reason: WireError::Trailing(1)
        }
    ));
    assert!(matches!(
        decoder.decode_report(&report(bytes[..5].to_vec())).unwrap()[0].outcome,
        Outcome::Invalid {
            reason: WireError::Truncated
        }
    ));
    let mut wrong = bytes;
    *wrong.last_mut().unwrap() ^= 1;
    assert_eq!(
        decoder.decode_report(&report(wrong)).unwrap()[0].outcome,
        Outcome::Invalid {
            reason: WireError::Literal
        }
    );
    assert_eq!(
        decoder.encode_slot(0, &WireValue::Byte(5), 4),
        Err(WireError::Type)
    );
    for number in [1.5, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            decoder.encode_slot(
                0,
                &sequence(vec![
                    WireValue::NumberBits(number.to_bits()),
                    WireValue::Byte(5)
                ]),
                4
            ),
            Err(WireError::Unsupported(_))
        ));
    }
    assert_eq!(
        decoder.decode_report(&report(vec![1, 2, 3])).unwrap()[0].outcome,
        Outcome::Invalid {
            reason: WireError::Truncated
        }
    );
}
#[test]
fn static_xor_does_not_consume_shared_stream_and_seed_resets() {
    let (_, document, pin) = synthetic("b[++p]=value^m();b[++p]=flag^170;b[++p]=value^m();");
    let decoder = load(&document, &pin);
    let value = sequence(vec![
        WireValue::Byte(1),
        WireValue::Byte(0x81),
        WireValue::Byte(2),
    ]);
    let bytes = decoder.encode_slot(0, &value, 1).unwrap();
    assert_eq!(bytes[5], 0x81 ^ 170);
    for _ in 0..2 {
        assert_eq!(decoder.encode_slot(0, &value, 1).unwrap(), bytes);
        assert!(matches!(
            decoder.decode_report(&report(bytes.clone())).unwrap()[0].outcome,
            Outcome::Decoded {
                exact_reencode: true,
                ..
            }
        ));
    }
    assert_ne!(decoder.encode_slot(0, &value, 2).unwrap(), bytes);
}
#[test]
fn strings_use_one_lossless_utf16_intern_table_per_message() {
    let (_, document, pin) = synthetic("p=sw(first,b,p);p=sw(second,b,p);");
    let decoder = load(&document, &pin);
    for units in [vec![0, 65, 0xd800], vec![0xd83d, 0xde00], vec![]] {
        let value = sequence(vec![
            WireValue::StringUnits(units.clone()),
            WireValue::StringUnits(units),
        ]);
        let bytes = decoder.encode_slot(0, &value, 0x11223344).unwrap();
        assert_eq!(
            decoder.decode_report(&report(bytes.clone())).unwrap()[0].outcome,
            Outcome::Decoded {
                value: value.clone(),
                exact_reencode: true
            }
        );
        assert_eq!(decoder.encode_slot(0, &value, 0x11223344).unwrap(), bytes);
    }
}
#[test]
fn repeats_preserve_fixed_count_direction_without_length_prefix() {
    let (_, document, pin) = synthetic("for(let i=2;i>=0;--i){b[++p]=i^m();}");
    let decoder = load(&document, &pin);
    let mut repeats = vec![];
    document.wire.as_ref().unwrap().slots[0].contract.emissions[0]
        .grammar
        .walk(&mut |node| {
            if let GrammarNode::Repeat {
                count, direction, ..
            } = node
            {
                repeats.push((*count, *direction));
            }
        });
    assert_eq!(repeats, vec![(Some(3), LoopDirection::Descending)]);
    let value = sequence(vec![WireValue::Repeat(vec![
        WireValue::Byte(2),
        WireValue::Byte(1),
        WireValue::Byte(0),
    ])]);
    let bytes = decoder.encode_slot(0, &value, 8).unwrap();
    assert_eq!(bytes.len(), 7);
    assert_eq!(
        decoder.decode_report(&report(bytes)).unwrap()[0].outcome,
        Outcome::Decoded {
            value,
            exact_reencode: true
        }
    );
    let (_, external, pin) = synthetic("for(let i=0;i<externalCount;++i){b[++p]=i^m();}");
    assert!(matches!(
        load(&external, &pin)
            .decode_report(&report(vec![0; 9]))
            .unwrap()[0]
            .outcome,
        Outcome::Unsupported { .. }
    ));
}
#[test]
fn choices_use_disjoint_literal_tags_and_refuse_external_choices() {
    let (_, document, pin) = synthetic(
        "if(flag){b[++p]=1^m();p=integerWriter(m,value,b,p);}else{b[++p]=2^m();b[++p]=9^m();}",
    );
    let decoder = load(&document, &pin);
    let value = sequence(vec![WireValue::Choice {
        arm: 1,
        value: Box::new(sequence(vec![WireValue::Byte(2), WireValue::Byte(9)])),
    }]);
    let bytes = decoder.encode_slot(0, &value, 5).unwrap();
    assert_eq!(
        decoder.decode_report(&report(bytes)).unwrap()[0].outcome,
        Outcome::Decoded {
            value,
            exact_reencode: true
        }
    );
    let (_, external, pin) =
        synthetic("if(flag){p=integerWriter(m,value,b,p);}else{b[++p]=9^m();}");
    assert!(matches!(
        &external.wire.as_ref().unwrap().slots[0].programs[0],
        Support::Unsupported { minimum_input: Some(input), .. }
            if input == "source branch outcome at this grammar choice"
    ));
    assert!(matches!(
        load(&external, &pin)
            .decode_report(&report(vec![0; 9]))
            .unwrap()[0]
            .outcome,
        Outcome::Unsupported { .. }
    ));
}
#[test]
fn distinct_same_salt_streams_discarded_draws_and_shared_tables_refuse() {
    for body in [
        "m();b[++p]=value^m();",
        "let other=streamFactory(seed^17);b[++p]=value^other();b[++p]=value^m();",
        "let other=stringFactory(m);p=sw(first,b,p);p=other(second,b,p);",
    ] {
        let (_, document, pin) = synthetic(body);
        assert!(matches!(
            load(&document, &pin)
                .decode_report(&report(vec![0; 9]))
                .unwrap()[0]
                .outcome,
            Outcome::Unsupported { .. }
        ));
    }
}
#[test]
fn schema_version_identity_routing_and_bounds_fail_without_exposing_data() {
    let (source, mut document, mut pin) = synthetic("b[++p]=value^m();");
    document.wire.as_mut().unwrap().version = 99;
    assert!(matches!(
        Decoder::load(&serde_json::to_vec(&document).unwrap(), &source, &pin),
        Err(WireError::Version)
    ));
    document.wire.as_mut().unwrap().version = 1;
    assert!(matches!(
        Decoder::load(
            &serde_json::to_vec(&document).unwrap(),
            "different source",
            &pin
        ),
        Err(WireError::Source)
    ));
    pin.report.positional_signals[0].descriptor = 42;
    let loaded = SchemaDocument::from_json(&serde_json::to_vec(&document).unwrap()).unwrap();
    assert!(matches!(
        Decoder::validated(loaded.signals, loaded.wire.unwrap(), &pin),
        Err(WireError::Routing)
    ));
    assert!(SchemaDocument::from_json(b"{} trailing").is_err());
}
#[test]
fn additive_mask_modes_do_not_collapse_none_inherit_nested_or_unresolved() {
    let (_, document, _) = synthetic(
        "b[++p]=value^m();b[++p]=flag^170;let other=streamFactory(seed^19);b[++p]=value^other();",
    );
    let fields = &document.signals["z-first"].description["fields"];
    assert_eq!(fields["f1"]["maskMode"], "nested");
    assert_eq!(fields["f1"]["mask"], 17);
    assert_eq!(fields["f2"]["maskMode"], "none");
    assert!(fields["f2"]["mask"].is_null());
    assert_eq!(fields["f3"]["maskMode"], "inherit");
    assert!(fields["f3"]["mask"].is_null());
    let masks = vec![
        WireMask::Inherit,
        WireMask::None,
        WireMask::Nested { salt: 17 },
        WireMask::Unresolved,
    ];
    assert_eq!(
        serde_json::from_slice::<Vec<WireMask>>(&serde_json::to_vec(&masks).unwrap()).unwrap(),
        masks
    );
}

#[test]
fn string_wire_golden_contains_modified_utf8_and_an_intern_reference() {
    let (_, document, pin) = synthetic("p=sw(first,b,p);p=sw(second,b,p);");
    let decoder = load(&document, &pin);
    // Paired-UTF16 hash("ab"), seed 0x01020304, salt 17. Plaintext
    // C0 80 41 ED A0 80 00 81 00 includes NUL, a lone surrogate,
    // the zero terminator, and an intern-table reference to index zero.
    let bytes = vec![
        0x01, 0x02, 0x03, 0x04, 0x62, 0xc9, 0x92, 0x42, 0x40, 0x72, 0x50, 0xb5, 0xee,
    ];
    let value = sequence(vec![
        WireValue::StringUnits(vec![0, 65, 0xd800]),
        WireValue::StringUnits(vec![0, 65, 0xd800]),
    ]);
    assert_eq!(
        decoder.decode_report(&report(bytes.clone())).unwrap()[0].outcome,
        Outcome::Decoded {
            value: value.clone(),
            exact_reencode: true
        }
    );
    assert_eq!(decoder.encode_slot(0, &value, 0x01020304).unwrap(), bytes);
}
#[test]
fn changing_seed_between_stream_creation_and_prefix_is_not_a_codec_proof() {
    let (_, document, pin) = synthetic_with_setup("b[++p]=value^m();", "seed=0;");
    assert!(matches!(
        load(&document, &pin)
            .decode_report(&report(vec![0; 5]))
            .unwrap()[0]
            .outcome,
        Outcome::Unsupported { .. }
    ));
}
#[test]
fn malformed_utf8_and_unproven_float_path_return_errors_without_values() {
    let (_, document, pin) = synthetic("p=sw(first,b,p);p=sw(second,b,p);");
    let decoder = load(&document, &pin);
    let bad = crate::signals::codec::encode_signal(hash_string_token("ab"), 3, 17, |w| {
        for b in [0xf0, 0x80, 0x80, 0x80, 0] {
            w.byte(b);
        }
    });
    assert_eq!(
        decoder.decode_report(&report(bad)).unwrap()[0].outcome,
        Outcome::Invalid {
            reason: WireError::Codec
        }
    );
    let (_, document, pin) = synthetic("p=integerWriter(m,value,b,p);");
    let decoder = load(&document, &pin);
    let float =
        crate::signals::codec::encode_signal(hash_string_token("ab"), 3, 17, |w| w.number(1.5));
    assert!(matches!(
        decoder.decode_report(&report(float)).unwrap()[0].outcome,
        Outcome::Unsupported { .. }
    ));
    let mut wrong = report(vec![0; 4]);
    wrong.positional_signals.pop();
    assert_eq!(decoder.decode_report(&wrong), Err(WireError::Bounds));
}

#[test]
fn prefix_seed_holder_can_be_reused_for_the_stream_after_rhs_evaluation() {
    let (_, document, pin) =
        synthetic_protocol("p=integerWriter(seed,value,b,p);b[++p]=5^seed();", "", true);
    let decoder = load(&document, &pin);
    let golden = vec![1, 2, 3, 4, 0xe2, 0x4c];
    assert_eq!(
        decoder.decode_report(&report(golden.clone())).unwrap()[0].outcome,
        Outcome::Decoded {
            value: sequence(vec![
                WireValue::NumberBits((-0.0f64).to_bits()),
                WireValue::Byte(5)
            ]),
            exact_reencode: true
        }
    );
}
#[test]
fn mutated_or_abrupt_repeat_counters_never_receive_closed_counts() {
    for body in [
        "for(let i=0;i<3;++i){b[++p]=value^m();++i;}",
        "for(let i=0;i<3;++i){b[++p]=value^m();if(flag)break;}",
    ] {
        let (_, document, pin) = synthetic(body);
        assert!(matches!(
            load(&document, &pin)
                .decode_report(&report(vec![0; 8]))
                .unwrap()[0]
                .outcome,
            Outcome::Unsupported { .. }
        ));
    }
}

#[test]
fn value_arguments_cannot_hide_unaccounted_stream_draws_or_leaks() {
    for body in [
        "p=integerWriter(m,consume(m),b,p);",
        "p=integerWriter(m,m(),b,p);",
        "b[++p]=(value^m())^m();",
    ] {
        let (_, document, pin) = synthetic(body);
        assert!(matches!(
            load(&document, &pin)
                .decode_report(&report(vec![0; 8]))
                .unwrap()[0]
                .outcome,
            Outcome::Unsupported { .. }
        ));
    }
}

#[test]
fn nullable_strings_use_certified_prefix_sets_without_trial_decoding() {
    let (_, document, pin) = synthetic("if(flag){p=sw(value,b,p);}else{b[++p]=130^m();}");
    let decoder = load(&document, &pin);
    for (arm, body) in [
        (0, WireValue::StringUnits(vec![0, 0xd800])),
        (1, WireValue::Byte(130)),
    ] {
        let value = sequence(vec![WireValue::Choice {
            arm,
            value: Box::new(body),
        }]);
        let bytes = decoder.encode_slot(0, &value, 9).unwrap();
        assert_eq!(
            decoder.decode_report(&report(bytes)).unwrap()[0].outcome,
            Outcome::Decoded {
                value,
                exact_reencode: true
            }
        );
    }
    let (_, overlap, pin) = synthetic("if(flag){p=sw(value,b,p);}else{b[++p]=129^m();}");
    assert!(matches!(
        load(&overlap, &pin)
            .decode_report(&report(vec![0; 5]))
            .unwrap()[0]
            .outcome,
        Outcome::Unsupported { .. }
    ));
}
#[test]
fn nullable_number_selectors_require_a_disjoint_float_prefix_but_never_decode_floats() {
    let (_, document, pin) = synthetic("if(flag){p=integerWriter(m,-0,b,p);}else{b[++p]=130^m();}");
    let decoder = load(&document, &pin);
    let value = sequence(vec![WireValue::Choice {
        arm: 0,
        value: Box::new(WireValue::NumberBits((-0.0f64).to_bits())),
    }]);
    let bytes = decoder.encode_slot(0, &value, 7).unwrap();
    assert_eq!(
        decoder.decode_report(&report(bytes)).unwrap()[0].outcome,
        Outcome::Decoded {
            value,
            exact_reencode: true
        }
    );
    let (_, unknown, pin) =
        synthetic("if(flag){p=integerWriter(m,value,b,p);}else{b[++p]=130^m();}");
    let float =
        crate::signals::codec::encode_signal(hash_string_token("ab"), 7, 17, |w| w.number(1.5));
    assert!(matches!(
        load(&unknown, &pin).decode_report(&report(float)).unwrap()[0].outcome,
        Outcome::Unsupported { .. }
    ));
}

#[test]
fn configured_native_float_consumer_and_named_json_keep_mode_bits_and_restore() {
    let (_, mut document, mut pin) =
        synthetic_with_number_factory("p=integerWriter(m,value,b,p);", "", true);
    use crate::signals::wire::named::{Field, Naming, Node, Part, Step};
    document.wire.as_mut().unwrap().slots[0].naming = Some(Naming {
        counted_arrays: true,
        version: 1,
        schema_key: "Synthetic".into(),
        canonical_name: "Synthetic".into(),
        mapping_error: None,
        parts: vec![Part {
            emission: 0,
            part: 0,
            message: 0,
            stream_salt: Some(17),
            part_leaves: vec![0],
            split_choice: None,
            fields: vec![Field {
                field_key: "f1".into(),
                name: "reading".into(),
                bits: None,
                node: Node::Leaf {
                    leaf: 0,
                    grammar_path: vec![Step::Child(1)],
                    kind: "number".into(),
                    mask: json!({"mode":"inherit"}),
                    static_xor: None,
                    byte_mapping: None,
                },
            }],
        }],
    });
    // A zero-state stream permits independent IEEE wire vectors in this fixture.
    pin.session.transaction_token.clear();
    let default = load(&document, &pin);
    assert!(document
        .wire
        .as_ref()
        .unwrap()
        .number_float
        .as_ref()
        .unwrap()
        .native_dependencies
        .codec
        .is_some());
    for order in [FloatByteOrder::Little, FloatByteOrder::Big] {
        let options = TypedDecoderOptions {
            float_byte_order: order,
        };
        let decoder = load(&document, &pin).with_options(options);
        for (bits, be) in [
            (0x3ff8000000000000, [0x3f, 0xf8, 0, 0, 0, 0, 0, 0]),
            (0x7ff8000000000021, [0x7f, 0xf8, 0, 0, 0, 0, 0, 0x21]),
            (0x0000000000000001, [0, 0, 0, 0, 0, 0, 0, 1]),
        ] {
            // salt=17, seed=17, token=0 => initial all-zero PRNG state.
            let mut golden = vec![0, 0, 0, 17, 128];
            if order == FloatByteOrder::Big {
                golden.extend(be);
            } else {
                golden.extend(be.into_iter().rev());
            }
            let original = report(golden.clone());
            assert!(matches!(
                default.decode_report(&original).unwrap()[0].outcome,
                Outcome::Unsupported { .. }
            ));
            let decoded = decoder.decode_report(&original).unwrap();
            let value = sequence(vec![WireValue::NumberBits(bits)]);
            assert_eq!(
                decoded[0].outcome,
                Outcome::Decoded {
                    value: value.clone(),
                    exact_reencode: true
                }
            );
            assert_eq!(decoder.encode_slot(0, &value, 17).unwrap(), golden);
            let named = decoder.decode_named_report(&original).unwrap();
            assert_eq!(named.metadata.decoder_options, options);
            let signal = &named.signals["Synthetic"];
            assert_eq!(signal.status, crate::signals::wire::NamedStatus::Decoded);
            let expected = if bits == 0x7ff8000000000021 {
                json!({"numberBits":bits,"special":"nan"})
            } else {
                json!(f64::from_bits(bits))
            };
            assert_eq!(signal.fields.as_ref().unwrap()["reading"], expected);
            assert_eq!(
                signal.metadata["projectionCandidates"][0]["fields"][0]["numberBits"],
                json!(bits)
            );
            let json = serde_json::to_vec(&named).unwrap();
            let restored = NamedDocument::from_json(&json).unwrap();
            assert_eq!(restored.metadata.decoder_options, options);
            assert_eq!(restored.restore_original_report(), original);
        }
    }
}

#[test]
fn resource_holders_end_after_plain_same_block_assignment_rhs() {
    for body in [
        "p=sw(value,b,p);sw=3;let unrelated=sw;",
        "p=sw(value,b,p);sw=sw(value,b,p);let unrelated=sw;",
    ] {
        let (_, document, pin) = synthetic(body);
        assert!(matches!(
            document.wire.as_ref().unwrap().slots[0].programs[0],
            Support::Supported { .. }
        ));
        let _ = load(&document, &pin);
    }
    for body in [
        "let alias=sw;sw=3;p=alias(value,b,p);",
        "if(flag)sw=3;consume(sw);p=sw(value,b,p);",
        "let leak=()=>sw;sw=3;p=sw(value,b,p);",
    ] {
        let (_, document, _) = synthetic(body);
        assert!(matches!(
            document.wire.as_ref().unwrap().slots[0].programs[0],
            Support::Unsupported { .. }
        ));
    }
}

#[test]
fn immediate_string_table_and_fresh_iteration_keep_resource_identity() {
    for (body, setup) in [
        ("p=stringFactory(m)(value,b,p);", ""),
        ("p=sw(value,b,p);", ""),
    ] {
        let (_, document, pin) = synthetic_with_setup(body, setup);
        let decoder = load(&document, &pin);
        let value = sequence(vec![WireValue::StringUnits(vec![65, 0xd800])]);
        let bytes = decoder.encode_slot(0, &value, 7).unwrap();
        assert!(matches!(
            decoder.decode_report(&report(bytes)).unwrap()[0].outcome,
            Outcome::Decoded {
                exact_reencode: true,
                ..
            }
        ));
    }
}

#[test]
fn discarded_certified_table_constructor_is_capture_only() {
    let (_, document, pin) = synthetic_with_setup("b[++p]=value^m();", "stringFactory(m);");
    let decoder = load(&document, &pin);
    let value = sequence(vec![WireValue::Byte(5)]);
    let bytes = decoder.encode_slot(0, &value, 7).unwrap();
    assert!(matches!(
        decoder.decode_report(&report(bytes)).unwrap()[0].outcome,
        Outcome::Decoded {
            exact_reencode: true,
            ..
        }
    ));
    for setup in [
        "unknownFactory(m);",
        "let escaped=stringFactory(m);consume(escaped);",
        "m();",
    ] {
        let (_, document, _) = synthetic_with_setup("b[++p]=value^m();", setup);
        assert!(matches!(
            document.wire.as_ref().unwrap().slots[0].programs[0],
            Support::Unsupported { .. }
        ));
    }
}

#[test]
fn adjacent_retained_draw_is_physical_byte_consumption() {
    let (_, document, pin) = synthetic("m=m();b[++p]=5^m;");
    let decoder = load(&document, &pin);
    // Independent first draw for the synthetic token/seed/salt fixture is 0xa2.
    let bytes = vec![1, 2, 3, 4, 0xa7];
    assert_eq!(
        decoder.decode_report(&report(bytes.clone())).unwrap()[0].outcome,
        Outcome::Decoded {
            value: sequence(vec![WireValue::Byte(5)]),
            exact_reencode: true
        }
    );
    assert_eq!(
        decoder
            .encode_slot(0, &sequence(vec![WireValue::Byte(5)]), 0x01020304)
            .unwrap(),
        bytes
    );
    // A retained draw survives inert local rebinding before its store
    // (`let m = ks(); f = f | 0; buf[i] = m ^ 136`).
    let (_, document, pin) = synthetic("let k=1;let draw=m();k=k|2<<1;let q=!k&&k;b[++p]=5^draw;");
    assert_eq!(
        load(&document, &pin).decode_report(&report(bytes)).unwrap()[0].outcome,
        Outcome::Decoded {
            value: sequence(vec![WireValue::Byte(5)]),
            exact_reencode: true
        }
    );
    for body in [
        "if(flag){m=m();}b[++p]=5^m;",
        "let draw;flag && (draw=m());b[++p]=5^draw;",
        "let draw;function mutate(){draw=0;return 9}draw=m();b[++p]=mutate()^draw;",
        "m=m();unknown();b[++p]=5^m;",
        "m=5;m=m();b[++p]=5^m;",
        "let saved=m();m();b[++p]=5^saved;",
        // Only inert local rebinding may sit between the draw and the store.
        "let draw=m();draw=draw|0;b[++p]=5^draw;",
        "let draw=m();let k=b[0];b[++p]=5^draw;",
        "let draw=m();b[0]=1;b[++p]=5^draw;",
        "let draw=m();undeclared=1;b[++p]=5^draw;",
        "let draw=m();let k='x' in b;b[++p]=5^draw;",
        "let draw=m();let k=m();b[++p]=5^draw;",
        "let draw;let k=0;function mutate(){draw=0}draw=m();k=k|1;b[++p]=mutate()^draw;",
    ] {
        let (_, document, _) = synthetic(body);
        assert!(
            matches!(
                document.wire.as_ref().unwrap().slots[0].programs[0],
                Support::Unsupported { .. }
            ),
            "{body}"
        );
    }
}

#[test]
fn loop_scope_checks_follow_the_constructor_activation() {
    let (_, document, _) = synthetic("for(let i=0;i<2;++i){let s=Math.random()*4294967296|0;let fresh=streamFactory(s^17);let inner=[s>>>24,s>>16&255,s>>8&255,s&255];inner[4]=5^fresh();obs.next([[],inner]);}");
    assert!(matches!(
        document.wire.as_ref().unwrap().slots[0].programs[0],
        Support::Supported { .. }
    ));
    let (_, document, _) = synthetic("for(let i=0;i<2;++i){b[4]=5^m();obs.next([[],b]);}");
    assert!(matches!(
        document.wire.as_ref().unwrap().slots[0].programs[0],
        Support::Unsupported { .. }
    ));
    let (_, document, _) =
        synthetic("let fresh;if(flag)fresh=streamFactory(seed^17);b[4]=5^fresh();");
    assert!(matches!(
        document.wire.as_ref().unwrap().slots[0].programs[0],
        Support::Unsupported { .. }
    ));
}

#[test]
fn physical_cursor_proofs_reject_gaps_overwrites_and_reverse_stores() {
    for body in [
        "p=integerWriter(m,value,b,99);",
        "p=integerWriter(m,3,b,p);b[p]=value^m();",
        "b[--p]=value^m();",
        "b[6]=value^m();",
    ] {
        let (_, document, pin) = synthetic(body);
        assert!(matches!(
            load(&document, &pin)
                .decode_report(&report(vec![0; 8]))
                .unwrap()[0]
                .outcome,
            Outcome::Unsupported { .. }
        ));
    }
}
#[test]
fn physical_cursor_proofs_follow_nested_writer_return_positions() {
    let (_, document, pin) = synthetic("p=integerWriter(m,2,b,integerWriter(m,1,b,p));");
    let decoder = load(&document, &pin);
    let bytes = crate::signals::codec::encode_signal(hash_string_token("ab"), 5, 17, |w| {
        w.number(1.);
        w.number(2.);
    });
    let value = sequence(vec![
        WireValue::NumberBits(1f64.to_bits()),
        WireValue::NumberBits(2f64.to_bits()),
    ]);
    assert_eq!(
        decoder.decode_report(&report(bytes)).unwrap()[0].outcome,
        Outcome::Decoded {
            value,
            exact_reencode: true
        }
    );
}

#[test]
fn const_caller_state_is_not_treated_as_a_mutable_cursor_or_stream_holder() {
    let (_, document, pin) =
        synthetic_with_setup("cursor=integerWriter(m,value,b,cursor);", "const cursor=3;");
    assert!(matches!(
        load(&document, &pin)
            .decode_report(&report(vec![0; 5]))
            .unwrap()[0]
            .outcome,
        Outcome::Unsupported { .. }
    ));
    let (_, document, pin) =
        synthetic("const bad=m;bad=streamFactory(seed^17);b[++p]=value^bad();");
    assert!(matches!(
        load(&document, &pin)
            .decode_report(&report(vec![0; 5]))
            .unwrap()[0]
            .outcome,
        Outcome::Unsupported { .. }
    ));
}

#[test]
fn resource_construction_and_emission_share_one_exception_region() {
    let fresh="let s=Math.random()*4294967296|0;let fresh=streamFactory(s^17);let inner=[s>>>24,s>>16&255,s>>8&255,s&255];inner[4]=5^fresh();obs.next([[],inner]);";
    for body in [
        format!("try{{{fresh}}}catch(error){{}}"),
        format!("try{{throw 0;}}catch(error){{{fresh}}}"),
    ] {
        let (_, document, _) = synthetic(&body);
        assert!(matches!(
            document.wire.as_ref().unwrap().slots[0].programs[0],
            Support::Supported { .. }
        ));
    }
    let (_,document,_)=synthetic("let inner,fresh;let s=Math.random()*4294967296|0;try{fresh=streamFactory(s^17);inner=[s>>>24,s>>16&255,s>>8&255,s&255];}finally{inner[4]=5^fresh();obs.next([[],inner]);}");
    assert!(matches!(
        document.wire.as_ref().unwrap().slots[0].programs[0],
        Support::Unsupported { .. }
    ));
}

#[test]
fn cursor_holder_overwrites_after_last_position_read_do_not_change_bytes() {
    let (_, document, pin) = synthetic("p=integerWriter(m,3,b,p);p={};consume(p);");
    let decoder = load(&document, &pin);
    let expected = sequence(vec![WireValue::NumberBits(3f64.to_bits())]);
    let golden = vec![1, 2, 3, 4, 0xa1];
    assert_eq!(
        decoder.decode_report(&report(golden.clone())).unwrap()[0].outcome,
        Outcome::Decoded {
            value: expected.clone(),
            exact_reencode: true
        }
    );
    assert_eq!(
        decoder.encode_slot(0, &expected, 0x01020304).unwrap(),
        golden
    );
    let (_, document, _) =
        synthetic("p=integerWriter(m,3,b,p);let cursor=p;p=99;b[++cursor]=5^m();");
    assert!(matches!(
        document.wire.as_ref().unwrap().slots[0].programs[0],
        Support::Supported { .. }
    ));
    for body in [
        "let q=integerWriter(m,value,b,p);if(flag)p=q;b[++p]=5^m();",
        "let q=integerWriter(m,value,b,p);flag&&(p=q);b[++p]=5^m();",
        "let q=integerWriter(m,value,b,p);if(flag){p=q;}b[++p]=5^m();",
        "let q=integerWriter(m,value,b,p);for(let i=0;i<count;++i)p=q;b[++p]=5^m();",
    ] {
        let (_, document, _) = synthetic(body);
        assert!(matches!(
            document.wire.as_ref().unwrap().slots[0].programs[0],
            Support::Unsupported { .. }
        ));
    }
}

#[test]
fn cursor_reaching_definitions_must_dominate_reads_before_and_after_prefix() {
    let (_, document, _) = synthetic_with_setup("b[++cursor]=5^m();", "let cursor=99;cursor=3;");
    assert!(matches!(
        document.wire.as_ref().unwrap().slots[0].programs[0],
        Support::Supported { .. }
    ));
    for setup in [
        "let cursor=3;if(flag)cursor=7;",
        "let cursor=3;flag&&(cursor=7);",
        "let cursor=3;for(let i=0;i<count;++i)cursor=7;",
    ] {
        let (_, document, _) = synthetic_with_setup("b[++cursor]=5^m();", setup);
        assert!(matches!(
            document.wire.as_ref().unwrap().slots[0].programs[0],
            Support::Unsupported { .. }
        ));
    }
}

#[test]
fn stream_holder_can_be_replaced_by_its_capturing_string_table() {
    let (_, document, pin) = synthetic("m=stringFactory(m);p=m(value,b,p);");
    let decoder = load(&document, &pin);
    let value = sequence(vec![WireValue::StringUnits(vec![65, 0xd800])]);
    let bytes = decoder.encode_slot(0, &value, 7).unwrap();
    assert_eq!(
        decoder.decode_report(&report(bytes)).unwrap()[0].outcome,
        Outcome::Decoded {
            value,
            exact_reencode: true
        }
    );
    let (_, document, _) = synthetic("m=5;b[++p]=value^m();");
    assert!(matches!(
        document.wire.as_ref().unwrap().slots[0].programs[0],
        Support::Unsupported { .. }
    ));
}

#[test]
fn inline_store_cursor_uses_the_proven_caller_argument_identity() {
    let body =
        "((target,index,value,result)=>{target[index]=value;return result;})(b,++p,5^m(),0);";
    let (_, document, pin) = synthetic(body);
    let decoder = load(&document, &pin);
    let value = sequence(vec![WireValue::Byte(5)]);
    let golden = vec![1, 2, 3, 4, 0xa7];
    assert_eq!(
        decoder.decode_report(&report(golden.clone())).unwrap()[0].outcome,
        Outcome::Decoded {
            value: value.clone(),
            exact_reencode: true
        }
    );
    assert_eq!(decoder.encode_slot(0, &value, 0x01020304).unwrap(), golden);
    for bad in [
        body.replace("++p", "p"),
        body.replace("++p", "p+=2"),
        body.replace("5^m()", "5^m()^m()"),
        body.replace(
            "target[index]=value;",
            "target[index]=value;target[index+1]=value;",
        ),
    ] {
        let (_, document, _) = synthetic(&format!("{bad}b[++p]=9^m();"));
        assert!(matches!(
            document.wire.as_ref().unwrap().slots[0].programs[0],
            Support::Unsupported { .. }
        ));
    }
}

#[test]
fn standalone_cursor_increment_is_a_source_effect_before_its_store() {
    for body in [
        "p++;b[p]=5^m();",
        "++p;b[p]=5^m();",
        "p++;((target,index,value,result)=>{target[index]=value;return result;})(b,p,5^m(),0);",
    ] {
        let (_, document, pin) = synthetic(body);
        let decoder = load(&document, &pin);
        let golden = vec![1, 2, 3, 4, 0xa7];
        assert_eq!(
            decoder.decode_report(&report(golden)).unwrap()[0].outcome,
            Outcome::Decoded {
                value: sequence(vec![WireValue::Byte(5)]),
                exact_reencode: true
            }
        );
    }
    for body in [
        "p++;p++;b[p]=5^m();",
        "if(flag)p++;b[p]=5^m();",
        "p--;b[p]=5^m();",
    ] {
        let (_, document, _) = synthetic(body);
        assert!(matches!(
            document.wire.as_ref().unwrap().slots[0].programs[0],
            Support::Unsupported { .. }
        ));
    }
}

#[test]
fn prior_array_count_is_consumed_once_with_trailers_and_source_order() {
    for (header, direction) in [
        ("let i=0;i<items.length;++i", LoopDirection::Ascending),
        ("let i=items.length-1;i>=0;--i", LoopDirection::Descending),
        ("let i=0;i<size;++i", LoopDirection::Ascending),
    ] {
        let body = format!("p=integerWriter(m,items.length,b,p);for({header}){{p=integerWriter(m,items[i],b,p);}}b[++p]=7^m();");
        let (_, document, pin) =
            synthetic_with_setup(&body, "let items=[3,4];let size=items.length;");
        let slot = &document.wire.as_ref().unwrap().slots[0];
        assert_eq!(slot.source_plan[0].counts.len(), 1);
        assert_eq!(slot.source_plan[0].counts[0].direction, direction);
        let decoder = load(&document, &pin);
        for count in [0, 1, 3] {
            let values: Vec<_> = (0..count)
                .map(|i| WireValue::NumberBits((i as f64 + 1.).to_bits()))
                .collect();
            let value = sequence(vec![
                WireValue::NumberBits((count as f64).to_bits()),
                WireValue::Repeat(values),
                WireValue::Byte(7),
            ]);
            // Independent primitive encoding: there is exactly one count.
            let mut writer =
                crate::signals::codec::SignalWriter::new(hash_string_token("ab"), 91, 17);
            writer.number(count as f64);
            for i in 0..count {
                writer.number(i as f64 + 1.);
            }
            writer.byte(7);
            let bytes = writer.finish();
            assert_eq!(decoder.encode_slot(0, &value, 91).unwrap(), bytes);
            assert_eq!(
                decoder.decode_report(&report(bytes)).unwrap()[0].outcome,
                Outcome::Decoded {
                    value: value.clone(),
                    exact_reencode: true
                }
            );
            let mismatch = sequence(vec![
                WireValue::NumberBits((count as f64 + 1.).to_bits()),
                match value {
                    WireValue::Sequence(v) => v[1].clone(),
                    _ => unreachable!(),
                },
                WireValue::Byte(7),
            ]);
            assert!(decoder.encode_slot(0, &mismatch, 91).is_err());
        }
        for n in [-0f64, -1., 0.5, 65537.] {
            let value = sequence(vec![
                WireValue::NumberBits(n.to_bits()),
                WireValue::Repeat(vec![]),
                WireValue::Byte(7),
            ]);
            assert!(decoder.encode_slot(0, &value, 91).is_err());
        }
    }
}

#[test]
fn prior_count_refuses_bound_mutation_aliases_and_unaccounted_calls() {
    for body in [
        "p=integerWriter(m,items.length,b,p);for(let i=1;i<items.length;++i){b[++p]=items[i]^m();}",
        "p=integerWriter(m,items.length,b,p);for(let i=0;i<items.length;++i){items.length=1;b[++p]=items[i]^m();}",
        "p=integerWriter(m,items.length,b,p);for(let i=0;i<items.length;++i){++i;b[++p]=items[i]^m();}",
        "p=integerWriter(m,items.length,b,p);for(let i=0;i<items.length;++i){unknown();b[++p]=items[i]^m();}",
        "let alias=items;p=integerWriter(m,items.length,b,p);for(let i=0;i<items.length;++i){b[++p]=items[i]^m();}",
        "p=integerWriter(m,other.length,b,p);for(let i=0;i<items.length;++i){b[++p]=items[i]^m();}",
    ] {
        let (_, document, _) = synthetic_with_setup(body, "let items=[1,2];let other=[1,2];");
        assert!(document.wire.unwrap().slots[0].source_plan[0].counts.is_empty());
    }
}

#[test]
fn nullable_domains_peek_once_and_preserve_literal_as_byte() {
    for (body, arm, value, plain) in [
        (
            "if(value==null){b[++p]=145^m();}else{p=integerWriter(m,value,b,p);}b[++p]=7^m();",
            0,
            WireValue::Byte(145),
            vec![145],
        ),
        (
            "if(value==null){b[++p]=145^m();}else{p=integerWriter(m,value,b,p);}b[++p]=7^m();",
            1,
            WireValue::NumberBits(3f64.to_bits()),
            vec![3],
        ),
        (
            "if(value==null){b[++p]=145^m();}else{p=sw(value,b,p);}b[++p]=7^m();",
            1,
            WireValue::StringUnits(vec![65]),
            vec![65, 0],
        ),
    ] {
        let (_, document, pin) = synthetic(body);
        let decoder = load(&document, &pin);
        let value = sequence(vec![
            WireValue::Choice {
                arm,
                value: Box::new(value),
            },
            WireValue::Byte(7),
        ]);
        let mut w = crate::signals::codec::SignalWriter::new(hash_string_token("ab"), 7, 17);
        for b in plain {
            w.byte(b);
        }
        w.byte(7);
        let bytes = w.finish();
        assert_eq!(decoder.encode_slot(0, &value, 7).unwrap(), bytes);
        assert_eq!(
            decoder.decode_report(&report(bytes)).unwrap()[0].outcome,
            Outcome::Decoded {
                value,
                exact_reencode: true
            }
        );
    }
    for body in [
        "if(flag){b[++p]=128^m();}else{p=integerWriter(m,value,b,p);}",
        "if(flag){b[++p]=129^m();}else{p=sw(value,b,p);}",
        "if(flag){b[++p]=65^m();}else{p=sw(value,b,p);}",
    ] {
        let (_, document, _) = synthetic(body);
        assert!(matches!(
            &document.wire.unwrap().slots[0].programs[0],
            Support::Unsupported { .. }
        ));
    }
}

fn only_program(document: &SchemaDocument) -> Program {
    match &document.wire.as_ref().unwrap().slots[0].programs[0] {
        Support::Supported { program } => program.clone(),
        Support::Unsupported { reason, .. } => panic!("synthetic unsupported: {reason}"),
        Support::RequiresInitializer { .. } => {
            panic!("synthetic initializer unexpectedly required")
        }
    }
}

#[test]
fn finite_source_message_membership_is_closed_and_canonical() {
    let (_, d, _) = synthetic("p=integerWriter(m,3,b,p);");
    let three = only_program(&d);
    let (_, d, _) = synthetic("p=integerWriter(m,4,b,p);");
    let four = only_program(&d);
    let supports = vec![
        Support::Supported {
            program: three.clone(),
        },
        Support::Supported { program: four },
    ];
    let bytes = three
        .encode(
            &sequence(vec![WireValue::NumberBits(3f64.to_bits())]),
            19,
            0,
        )
        .unwrap();
    assert_eq!(select(&supports, &bytes, 0).unwrap().1.emissions, vec![0]);
    let mut unknown = supports.clone();
    unknown.push(Support::Unsupported {
        reason: "external count".into(),
        minimum_input: None,
    });
    assert!(matches!(
        select(&unknown, &bytes, 0),
        Err(WireError::Unsupported(_))
    ));
    assert!(matches!(
        select(&unknown, &[0, 0, 0], 0),
        Err(WireError::Unsupported(_))
    ));
    let (_, d, _) = synthetic("p=integerWriter(m,value,b,p);");
    let number = only_program(&d);
    assert_eq!(
        select(
            &[
                supports[0].clone(),
                Support::Supported {
                    program: number.clone()
                }
            ],
            &bytes,
            0
        ),
        Err(WireError::Ambiguous)
    );
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(select(&supports, &trailing, 0).is_err());
    assert!(select(&supports, &bytes[..4], 0).is_err());
    // One candidate accepts BYTE(128); the NUMBER candidate is UNKNOWN, so
    // accepting the byte candidate would turn this into a trial-parse oracle.
    let (_, d, _) = synthetic("b[++p]=128^m();");
    let byte = only_program(&d);
    let bytes = byte
        .encode(&sequence(vec![WireValue::Byte(128)]), 19, 0)
        .unwrap();
    assert!(matches!(
        select(
            &[
                Support::Supported { program: byte },
                Support::Supported { program: number }
            ],
            &bytes,
            0
        ),
        Err(WireError::Unsupported(_))
    ));
}

#[test]
fn root_case_length_selects_before_reading_any_salt() {
    use crate::signals::wire::{RootCase, RootCasePlan};
    let (_, a, _) = synthetic("b[++p]=value^m();p=integerWriter(m,value,b,p);");
    let a = only_program(&a);
    assert_eq!(a.length_bounds(), (6, Some(14)));
    let (_, b, _) = synthetic("p=sw(value,b,p);p=integerWriter(m,value,b,p);");
    let mut b = only_program(&b);
    b.salt = 31;
    let make_case = |arm, program: Program| {
        let (minimum_length, maximum_length) = program.length_bounds();
        RootCase {
            plan: RootCasePlan {
                source_choice: None,
                arm,
                prefix: SourceSpan::new(1, 2),
                stream: SourceSpan::new(3, 4),
                seed_binding: SourceSpan::new(5, 6),
                salt: program.salt,
                minimum_length,
                maximum_length,
            },
            support: Box::new(Support::Supported { program }),
        }
    };
    let root = Program {
        body: Op::RootChoice {
            cases: vec![make_case(0, a.clone()), make_case(1, b.clone())],
        },
        ..a
    };
    // 88 ASCII units + terminator + NUMBER + RAW4 = 94 whole bytes.
    let inner = sequence(vec![
        WireValue::StringUnits(vec![65; 88]),
        WireValue::NumberBits(3f64.to_bits()),
    ]);
    let bytes = b.encode(&inner, 123, 0).unwrap();
    assert_eq!(bytes.len(), 94);
    let value = WireValue::Choice {
        arm: 1,
        value: Box::new(inner),
    };
    assert_eq!(root.decode(&bytes, 0).unwrap(), value);
    assert_eq!(
        select(
            &[Support::Supported {
                program: root.clone()
            }],
            &bytes,
            0
        )
        .unwrap()
        .1
        .root_case
        .unwrap()
        .arm,
        1
    );
    assert_eq!(root.encode(&value, 123, 0).unwrap(), bytes);
    assert_eq!(root.decode(&vec![0; 14], 0), Err(WireError::Ambiguous));
    let mut unknown = root;
    let Op::RootChoice { cases } = &mut unknown.body else {
        unreachable!()
    };
    cases[0].support = Box::new(Support::Unsupported {
        reason: "unknown scope".into(),
        minimum_input: None,
    });
    assert_eq!(unknown.decode(&bytes, 0).unwrap(), value); // excluded by proof
    let Op::RootChoice { cases } = &mut unknown.body else {
        unreachable!()
    };
    cases[1].support = Box::new(Support::Unsupported {
        reason: "unknown scope".into(),
        minimum_input: None,
    });
    assert!(matches!(
        unknown.decode(&bytes, 0),
        Err(WireError::Unsupported(_))
    ));
}

#[test]
fn grouped_flush_counts_iterations_including_empty_choice_arms() {
    for descending in [false, true] {
        let header = if descending {
            "let i=items.length-1;i>=0;--i"
        } else {
            "let i=0;i<items.length;++i"
        };
        let body = format!("p=integerWriter(m,items.length,b,p);for({header}){{if((i&7)===0){{b[++p]=5^m();}}}}b[++p]=9^m();");
        let (_, document, pin) = synthetic_with_setup(&body, "let items=[];");
        let plan = &document.wire.as_ref().unwrap().slots[0].source_plan[0];
        assert_eq!(plan.counts.len(), 1);
        assert_eq!(plan.counter_choices.len(), 1);
        let decoder = load(&document, &pin);
        for length in [0usize, 1, 8, 9, 17] {
            let mut w = crate::signals::codec::SignalWriter::new(hash_string_token("ab"), 37, 17);
            w.number(length as f64);
            let values = (0..length)
                .map(|index| {
                    let counter = if descending {
                        length - 1 - index
                    } else {
                        index
                    };
                    let flush = counter & 7 == 0;
                    if flush {
                        w.byte(5);
                    }
                    WireValue::Choice {
                        arm: usize::from(!flush),
                        value: Box::new(if flush {
                            WireValue::Byte(5)
                        } else {
                            sequence(vec![])
                        }),
                    }
                })
                .collect();
            w.byte(9);
            let bytes = w.finish();
            assert_eq!(bytes.len(), 6 + length.div_ceil(8));
            let value = sequence(vec![
                WireValue::NumberBits((length as f64).to_bits()),
                WireValue::Repeat(values),
                WireValue::Byte(9),
            ]);
            assert_eq!(decoder.encode_slot(0, &value, 37).unwrap(), bytes);
            assert_eq!(
                decoder.decode_report(&report(bytes)).unwrap()[0].outcome,
                Outcome::Decoded {
                    value,
                    exact_reencode: true
                }
            );
        }
    }
}

#[test]
fn root_cases_retain_source_branch_salts_prefixes_and_machine_bounds() {
    let (source, document, _) = synthetic_collector("", "", Some("let seed,m,b,p,sw;if(flag){seed=Math.random()*4294967296|0;m=streamFactory(seed^17);b=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];p=3;b[++p]=value^m();p=integerWriter(m,value,b,p);}else{seed=Math.random()*4294967296|0;m=streamFactory(seed^31);sw=stringFactory(m);b=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];p=3;p=sw(value,b,p);p=integerWriter(m,value,b,p);}obs.next([[],b]);"));
    let document = SchemaDocument::from_json(&serde_json::to_vec(&document).unwrap()).unwrap();
    let slot = &document.wire.as_ref().unwrap().slots[0];
    let cases = &slot.source_plan[0].root_cases;
    assert_eq!(cases.len(), 2);
    assert_eq!(
        (
            cases[0].arm,
            cases[0].salt,
            cases[0].minimum_length,
            cases[0].maximum_length
        ),
        (0, 17, 6, Some(14))
    );
    assert_eq!(
        (
            cases[1].arm,
            cases[1].salt,
            cases[1].minimum_length,
            cases[1].maximum_length
        ),
        (1, 31, 6, None)
    );
    assert_ne!(cases[0].prefix, cases[1].prefix);
    assert_ne!(cases[0].stream, cases[1].stream);
    let mut unsafe_source = source.clone();
    unsafe_source.push_str("\nNumber=function(n){return n;};");
    let unsafe_plan = crate::signals::wire::control::source_plan(
        &unsafe_source,
        &slot.contract.emissions[0],
        &document.wire.as_ref().unwrap().algorithms,
    );
    assert!(unsafe_plan
        .root_cases
        .iter()
        .all(|c| c.maximum_length.is_none()));

    let Support::Supported { program } = &slot.programs[0] else {
        panic!("root selector must survive guarded case support")
    };
    let Op::RootChoice { cases: compiled } = &program.body else {
        panic!("root prefix is not a nested reset")
    };
    assert_eq!(
        compiled.iter().map(|c| &c.plan).collect::<Vec<_>>(),
        cases.iter().collect::<Vec<_>>()
    );
}

#[test]
fn cached_count_and_external_zero_initializers_cover_for_and_while() {
    for loop_body in [
        "let i=0;for(;i<size;++i){p=integerWriter(m,items[i],b,p);}",
        "let i=0;while(i<size){p=integerWriter(m,items[i],b,p);++i;}",
        "let i=0;while(i<size){p=integerWriter(m,items[i++],b,p);}",
    ] {
        let body = format!("p=integerWriter(m,size,b,p);{loop_body}b[++p]=9^m();");
        let (_, document, pin) =
            synthetic_with_setup(&body, "let items=externalArray;let size=items.length;");
        let plan = &document.wire.as_ref().unwrap().slots[0].source_plan[0];
        assert_eq!(plan.counts.len(), 1, "{loop_body}: {plan:?}");
        assert!(plan.counts[0].same_cached_evaluation);
        let decoder = load(&document, &pin);
        let value = sequence(vec![
            WireValue::NumberBits(2f64.to_bits()),
            WireValue::Repeat(vec![
                WireValue::NumberBits(5f64.to_bits()),
                WireValue::NumberBits(6f64.to_bits()),
            ]),
            WireValue::Byte(9),
        ]);
        let bytes = decoder.encode_slot(0, &value, 17).unwrap();
        assert_eq!(bytes.len(), 8);
        assert_eq!(
            decoder.decode_report(&report(bytes)).unwrap()[0].outcome,
            Outcome::Decoded {
                value,
                exact_reencode: true
            }
        );
    }
}

#[test]
fn packed_counter_recurrence_proves_flush_and_partial_tail() {
    for (descending, alternate) in [
        (false, ""),
        (true, ""),
        (false, "else{scratch=pack;}"),
        (true, "else{scratch=pack;}"),
    ] {
        let loop_body = if descending {
            "let i=size;for(;;){--i;if(!(i>=0))break;++pack;if(8===pack){b[++p]=5^m();pack=0;}}"
        } else {
            "for(let i=0;i<size;++i){++pack;if(8===pack){b[++p]=5^m();pack=0;}}"
        };
        let loop_body = loop_body.replace("pack=0;}", &format!("pack=0;}}{alternate}"));
        let body = format!(
            "p=integerWriter(m,size,b,p);{loop_body}if(0!==pack){{b[++p]=6^m();}}b[++p]=9^m();"
        );
        let (_, document, pin) =
            synthetic_with_setup(&body, "let items=[];let size=items.length;let pack=0;let scratch=0;");
        let plan = &document.wire.as_ref().unwrap().slots[0].source_plan[0];
        assert_eq!(plan.counts.len(), 1, "{plan:?}");
        assert_eq!(plan.counter_choices.len(), 2, "{plan:?}");
        let decoder = load(&document, &pin);
        for length in [0usize, 1, 7, 8, 9, 16, 17] {
            let mut w = crate::signals::codec::SignalWriter::new(hash_string_token("ab"), 123, 17);
            w.number(length as f64);
            let values = (0..length)
                .map(|i| {
                    let flush = (i + 1) % 8 == 0;
                    if flush {
                        w.byte(5)
                    }
                    WireValue::Choice {
                        arm: usize::from(!flush),
                        value: Box::new(if flush {
                            WireValue::Byte(5)
                        } else {
                            sequence(vec![])
                        }),
                    }
                })
                .collect();
            let tail = length % 8 != 0;
            if tail {
                w.byte(6)
            }
            w.byte(9);
            let bytes = w.finish();
            assert_eq!(bytes.len(), 6 + length.div_ceil(8));
            let value = sequence(vec![
                WireValue::NumberBits((length as f64).to_bits()),
                WireValue::Repeat(values),
                WireValue::Choice {
                    arm: usize::from(!tail),
                    value: Box::new(if tail {
                        WireValue::Byte(6)
                    } else {
                        sequence(vec![])
                    }),
                },
                WireValue::Byte(9),
            ]);
            assert_eq!(decoder.encode_slot(0, &value, 123).unwrap(), bytes);
            assert_eq!(
                decoder.decode_report(&report(bytes)).unwrap()[0].outcome,
                Outcome::Decoded {
                    value,
                    exact_reencode: true
                }
            );
        }
    }
}

#[test]
fn packed_explicit_else_keeps_mutation_and_write_refusals() {
    for alternate in [
        "pack=1;",
        "++pack;",
        "if(flag)pack=0;",
        "unknown();",
        "b[++p]=6^m();",
        "b[p+2]=6^m();",
        "scratch=m();",
    ] {
        let body = format!("p=integerWriter(m,size,b,p);for(let i=0;i<size;++i){{++pack;if(8===pack){{b[++p]=5^m();pack=0;}}else{{{alternate}}}}}if(pack!==0){{b[++p]=6^m();}}");
        let (_, document, pin) = synthetic_with_setup(
            &body,
            "let items=[];let size=items.length;let pack=0;let scratch=0;",
        );
        assert!(
            matches!(
                document.wire.as_ref().unwrap().slots[0].programs[0],
                Support::Unsupported { .. }
            ),
            "alternate: {alternate}"
        );
        assert!(matches!(
            load(&document, &pin)
                .decode_report(&report(vec![0; 6]))
                .unwrap()[0]
                .outcome,
            Outcome::Unsupported { .. }
        ));
    }
}

#[test]
fn packed_recurrence_refuses_extra_writes_and_wrong_resets() {
    for extra in ["pack=1;", "++pack;", "if(flag)pack=0;", "unknown();"] {
        let body=format!("p=integerWriter(m,size,b,p);for(let i=0;i<size;++i){{++pack;{extra}if(8===pack){{b[++p]=5^m();pack=0;}}}}if(pack!==0){{b[++p]=6^m();}}");
        let (_, document, _) =
            synthetic_with_setup(&body, "let items=[];let size=items.length;let pack=0;");
        assert!(document.wire.as_ref().unwrap().slots[0].source_plan[0]
            .counter_choices
            .is_empty());
        assert!(matches!(
            document.wire.as_ref().unwrap().slots[0].programs[0],
            Support::Unsupported { .. }
        ));
    }
}

#[test]
fn optional_string_intern_marker_peek_and_source_null_guard_are_distinct() {
    let (_, document, pin) = synthetic_with_setup(
        "p=sw(value,b,p);if(value==null){b[++p]=145^m();}else{p=sw(value,b,p);}b[++p]=7^m();",
        "let value=observed;",
    );
    let choices = &document.wire.as_ref().unwrap().slots[0].source_plan[0].choices;
    assert_eq!(choices.len(), 1);
    assert!(matches!(
        choices[0].semantics,
        Some(crate::signals::wire::ChoiceSemantics::Nullish { literal_arm: 0, .. })
    ));
    let decoder = load(&document, &pin);
    let value = sequence(vec![
        WireValue::StringUnits(vec![65]),
        WireValue::Choice {
            arm: 1,
            value: Box::new(WireValue::StringUnits(vec![65])),
        },
        WireValue::Byte(7),
    ]);
    let mut w = crate::signals::codec::SignalWriter::new(hash_string_token("ab"), 11, 17);
    w.string_units(&[65]);
    w.string_units(&[65]);
    w.byte(7);
    let bytes = w.finish();
    assert_eq!(bytes.len(), 9);
    assert_eq!(decoder.encode_slot(0, &value, 11).unwrap(), bytes);
    assert_eq!(
        decoder.decode_report(&report(bytes)).unwrap()[0].outcome,
        Outcome::Decoded {
            value,
            exact_reencode: true
        }
    );
}

#[test]
fn integer_certificate_cannot_invent_a_float_byte_envelope() {
    let (source, document, _) =
        synthetic("if(value==null){b[++p]=145^m();}else{p=integerWriter(m,value,b,p);}");
    let a = &document.wire.as_ref().unwrap().algorithms;
    let span = a.integer.span.unwrap();
    assert!(crate::signals::wire::control::number_envelope(&source, span).is_some());
    let changed = source.replacen("local1() ^ 128", "local1() ^ 130", 1);
    assert_ne!(changed, source);
    let body = &changed[span.start as usize..span.end as usize];
    assert!(proof(
        Some((span, body)),
        include_str!("reference/number.js"),
        "integer-only",
        true
    )
    .codec
    .is_some());
    assert!(crate::signals::wire::control::number_envelope(&changed, span).is_none());
    let compiled = crate::signals::wire::programs_for(
        &document.wire.as_ref().unwrap().slots[0].contract,
        a,
        &changed,
    );
    assert!(matches!(&compiled[0], Support::Unsupported { .. }));
}

#[test]
fn source_guard_dominating_the_whole_loop_does_not_make_its_update_conditional() {
    let (_,document,pin)=synthetic_collector("","",Some("if(enabled){let seed=Math.random()*4294967296|0;let m=streamFactory(seed^17);let items=[];let size=items.length;let b=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];let p=3;p=integerWriter(m,size,b,p);let i=0;while(i<size){p=integerWriter(m,items[i],b,p);++i;}obs.next([[],b]);}"));
    assert_eq!(
        document.wire.as_ref().unwrap().slots[0].source_plan[0]
            .counts
            .len(),
        1
    );
    let decoder = load(&document, &pin);
    let value = sequence(vec![
        WireValue::NumberBits(2f64.to_bits()),
        WireValue::Repeat(vec![
            WireValue::NumberBits(3f64.to_bits()),
            WireValue::NumberBits(4f64.to_bits()),
        ]),
    ]);
    let bytes = decoder.encode_slot(0, &value, 17).unwrap();
    assert_eq!(bytes.len(), 7);
    assert_eq!(
        decoder.decode_report(&report(bytes)).unwrap()[0].outcome,
        Outcome::Decoded {
            value,
            exact_reencode: true
        }
    );
}

#[test]
fn structural_occurrences_disambiguate_shared_source_loop_spans() {
    use crate::signals::wire::ControlStep;
    let (source,document,_) = synthetic_with_setup("p=integerWriter(m,items.length,b,p);for(let i=0;i<items.length;++i){p=integerWriter(m,items[i],b,p);}","let items=[];");
    let wire = document.wire.unwrap();
    let mut e = wire.slots[0].contract.emissions[0].clone();
    // Model two inline occurrences of the same helper source in an IR-only
    // compiler test. Public loading separately checks the entire contract join.
    let GrammarNode::Sequence { items } = &mut e.grammar else {
        unreachable!()
    };
    let number = items[1].clone();
    let repeat = items[2].clone();
    items.extend([number, repeat]);
    let plan = crate::signals::wire::control::source_plan(&source, &e, &wire.algorithms);
    assert_eq!(plan.counts.len(), 2);
    assert_eq!(plan.counts[0].loop_span, plan.counts[1].loop_span);
    assert_eq!(plan.counts[0].path, vec![1]);
    assert_eq!(plan.counts[1].path, vec![3]);
    assert_eq!(plan.counts[0].repeat_path, vec![ControlStep::Child(2)]);
    assert_eq!(plan.counts[1].repeat_path, vec![ControlStep::Child(4)]);
    let program = compile(&e, &wire.algorithms, &source).unwrap();
    let value = sequence(vec![
        WireValue::NumberBits(1f64.to_bits()),
        WireValue::Repeat(vec![WireValue::NumberBits(5f64.to_bits())]),
        WireValue::NumberBits(2f64.to_bits()),
        WireValue::Repeat(vec![
            WireValue::NumberBits(6f64.to_bits()),
            WireValue::NumberBits(7f64.to_bits()),
        ]),
    ]);
    let mut w = crate::signals::codec::SignalWriter::new(0, 17, 17);
    for n in [1., 5., 2., 6., 7.] {
        w.number(n)
    }
    let bytes = w.finish();
    assert_eq!(program.encode(&value, 17, 0).unwrap(), bytes);
    assert_eq!(program.decode(&bytes, 0).unwrap(), value);
}

#[test]
fn zero_assignment_inside_an_earlier_maybe_empty_loop_is_not_an_initializer() {
    let (_,document,_)=synthetic_with_setup("let i=externalStart;for(;flag;)i=0;p=integerWriter(m,size,b,p);while(i<size){p=integerWriter(m,items[i],b,p);++i;}","let items=[];let size=items.length;");
    let plan = &document.wire.as_ref().unwrap().slots[0].source_plan[0];
    assert!(plan.counts.is_empty());
    assert!(matches!(
        document.wire.as_ref().unwrap().slots[0].programs[0],
        Support::Unsupported { .. }
    ));
}

#[test]
fn packed_tail_proof_tracks_the_read_before_a_binding_is_reused_in_arms() {
    for (between, expected) in [("", 2), ("pack=0;", 1)] {
        let body=format!("p=integerWriter(m,size,b,p);for(let i=0;i<size;++i){{++pack;if(8===pack){{b[++p]=5^m();pack=0;}}}}{between}if(0!==pack){{pack=p;b[++p]=6^m();}}else{{pack=p;}}b[++p]=9^m();");
        let (_, document, pin) =
            synthetic_with_setup(&body, "let items=[];let size=items.length;let pack=0;");
        let plan = &document.wire.as_ref().unwrap().slots[0].source_plan[0];
        assert_eq!(plan.counter_choices.len(), expected);
        if !between.is_empty() {
            assert!(matches!(
                document.wire.as_ref().unwrap().slots[0].programs[0],
                Support::Unsupported { .. }
            ));
            continue;
        }
        let decoder = load(&document, &pin);
        for length in [0usize, 1, 8, 9] {
            let values = (0..length)
                .map(|i| {
                    let flush = (i + 1) % 8 == 0;
                    WireValue::Choice {
                        arm: usize::from(!flush),
                        value: Box::new(if flush {
                            WireValue::Byte(5)
                        } else {
                            sequence(vec![])
                        }),
                    }
                })
                .collect();
            let tail = length % 8 != 0;
            let value = sequence(vec![
                WireValue::NumberBits((length as f64).to_bits()),
                WireValue::Repeat(values),
                WireValue::Choice {
                    arm: usize::from(!tail),
                    value: Box::new(if tail {
                        WireValue::Byte(6)
                    } else {
                        sequence(vec![])
                    }),
                },
                WireValue::Byte(9),
            ]);
            let mut w = crate::signals::codec::SignalWriter::new(hash_string_token("ab"), 17, 17);
            w.number(length as f64);
            for _ in 0..length / 8 {
                w.byte(5)
            }
            if tail {
                w.byte(6)
            }
            w.byte(9);
            let bytes = w.finish();
            assert_eq!(decoder.encode_slot(0, &value, 17).unwrap(), bytes);
            assert_eq!(
                decoder.decode_report(&report(bytes)).unwrap()[0].outcome,
                Outcome::Decoded {
                    value,
                    exact_reencode: true
                }
            );
        }
    }
}

#[test]
fn fixed_number_envelope_is_independent_of_float_arithmetic_and_rejects_extra_effects() {
    use crate::signals::wire::control::{number_envelope, EnvelopeProof};
    let reference = include_str!("reference/number.js");
    let changed = reference.replacen("Math.log", "Math.sqrt", 1);
    let envelope = number_envelope(&changed, SourceSpan::new(0, changed.len() as u32)).unwrap();
    assert_eq!(
        (envelope.first_byte_maximum, envelope.maximum_bytes),
        (128, 9)
    );
    assert!(matches!(envelope.proof, EnvelopeProof::FixedWrites { .. }));
    for changed in [
        changed.replacen(
            "local3[local5] = local1() ^ 128;",
            "local3[local5] = local1() ^ 128;local1();",
            1,
        ),
        changed.replacen(
            "local3[local5] = local1() ^ 128;",
            "local3[local5] = local1() ^ 128;local3[local5]=0;",
            1,
        ),
        changed.replacen(
            "local3[local5] = local1() ^ 128;",
            "local3[local5] = local1() ^ 128;local5=3;",
            1,
        ),
        changed.replacen("j < 8", "j < 9", 1),
    ] {
        assert!(number_envelope(&changed, SourceSpan::new(0, changed.len() as u32)).is_none());
    }
}

#[test]
fn null_semantics_require_the_same_value_at_the_guard_and_writer_read() {
    for (body,certified) in [
        ("if(value==null){b[++p]=145^m();}else{value=integerWriter(m,value,b,p);}",true),
        ("if(value==null){b[++p]=145^m();}else{value=other;p=integerWriter(m,value,b,p);}",false),
        ("function mutate(){value=other;}if(value==null){b[++p]=145^m();}else{mutate();p=integerWriter(m,value,b,p);}",false),
        ("if(value==null){b[++p]=145^m();}else{var value=other;p=integerWriter(m,value,b,p);}",false),
    ] {
        let setup=if body.contains("var value") {"var value=observed;"}else{"let value=observed;"};
        let (_,document,_)=synthetic_with_setup(body,setup);
        let plan=&document.wire.as_ref().unwrap().slots[0].source_plan[0];
        assert_eq!(plan.choices.len(),1);
        assert_eq!(plan.choices[0].semantics.is_some(),certified);
    }
}

fn synthetic_collector(
    body: &str,
    setup: &str,
    collector: Option<&str>,
) -> (String, SchemaDocument, SeedLayout) {
    synthetic_protocol_options(body, setup, false, false, collector)
}

#[test]
fn referenced_repeat_cursor_invariant_refuses_absolute_indices_and_backedge_resets() {
    for loop_body in ["b[4]=5^m();", "b[++p]=5^m();p=3;", "p++;b[++p]=5^m();"] {
        let body = format!(
            "p=integerWriter(m,items.length,b,p);for(let i=0;i<items.length;++i){{{loop_body}}}"
        );
        let (_, document, _) = synthetic_with_setup(&body, "let items=[];");
        assert!(matches!(
            document.wire.as_ref().unwrap().slots[0].programs[0],
            Support::Unsupported { .. }
        ));
    }
    let (_, document, _) = synthetic("for(let i=0;i<3;++i){b[++p]=5^m();p=3;}");
    assert!(matches!(
        document.wire.as_ref().unwrap().slots[0].programs[0],
        Support::Unsupported { .. }
    ));
}

#[test]
fn equivalent_prior_count_executions_keep_all_source_emissions() {
    let arm = "let seed=Math.random()*4294967296|0;let m=streamFactory(seed^17);let b=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];let p=3;let items=[];p=integerWriter(m,items.length,b,p);for(let i=0;i<items.length;++i){p=integerWriter(m,items[i],b,p);}obs.next([[],b]);";
    let (_, document, pin) = synthetic_collector("", "", Some(&format!("{{{arm}}}{{{arm}}}")));
    let decoder = load(&document, &pin);
    let slot = &document.wire.as_ref().unwrap().slots[0];
    assert_eq!(slot.programs.len(), 2);
    assert_ne!(slot.programs[0], slot.programs[1]);
    let mut writer = crate::signals::codec::SignalWriter::new(hash_string_token("ab"), 123, 17);
    for number in [2., 5., 9.] {
        writer.number(number);
    }
    let golden = writer.finish();
    let decoded = decoder.decode_report(&report(golden.clone())).unwrap();
    let Outcome::Decoded {
        value,
        exact_reencode: true,
    } = &decoded[0].outcome
    else {
        panic!("{:?}", decoded[0].outcome)
    };
    let selection = decoded[0].selection.as_ref().unwrap();
    assert_eq!(selection.emissions, [0, 1]);
    assert_eq!(selection.proof, SelectionProof::EquivalentCountPrograms);
    assert_eq!(decoder.encode_slot(0, value, 123).unwrap(), golden);
    let (Support::Supported { program: a }, Support::Supported { program: b }) =
        (&slot.programs[0], &slot.programs[1])
    else {
        panic!()
    };
    assert!(a.same_execution(b));
    for mutation in 0..5 {
        let mut changed = b.clone();
        let Op::Sequence { items } = &mut changed.body else {
            panic!()
        };
        let Op::CountedRepeat {
            count: crate::signals::wire::CountExpr::PriorNumber { reference },
            direction,
            ..
        } = &mut items[1]
        else {
            panic!()
        };
        match mutation {
            0 => reference.path = vec![123],
            1 => reference.maximum -= 1,
            2 => reference.reject_negative_zero = !reference.reject_negative_zero,
            3 => *direction = LoopDirection::Descending,
            _ => changed.salt ^= 1,
        }
        assert!(!a.same_execution(&changed));
    }
}

#[test]
fn cursor_assignment_commits_after_the_accounted_writer_rhs() {
    for body in [
        "let q=integerWriter(m,value,b,p)+1;b[q]=5^m();",
        "p=integerWriter(m,value,b,p)+1;b[p]=5^m();",
    ] {
        let (_, document, pin) = synthetic(body);
        let decoder = load(&document, &pin);
        let golden = crate::signals::codec::encode_signal(hash_string_token("ab"), 123, 17, |w| {
            w.number(2.);
            w.byte(5);
        });
        let decoded = decoder.decode_report(&report(golden.clone())).unwrap();
        let Outcome::Decoded {
            value,
            exact_reencode: true,
        } = &decoded[0].outcome
        else {
            panic!("{:?}", decoded[0].outcome)
        };
        assert_eq!(decoder.encode_slot(0, value, 123).unwrap(), golden);
    }
    for body in [
        "let q=integerWriter(m,value,b,p)+1+1;b[q]=5^m();",
        "let q=integerWriter(m,value,b,p)+1;--q;b[q]=5^m();",
    ] {
        let (_, document, pin) = synthetic(body);
        assert!(matches!(
            load(&document, &pin)
                .decode_report(&report(vec![0; 6]))
                .unwrap()[0]
                .outcome,
            Outcome::Unsupported { .. }
        ));
    }
}

#[test]
fn source_affine_byte_positions_preserve_a_fixed_base_without_gaps() {
    let (_, document, pin) =
        synthetic("p=integerWriter(m,value,b,p);b[p+1]=5^m();b[p+2]=6^m();b[p+3]=7^m();");
    let decoder = load(&document, &pin);
    let golden = crate::signals::codec::encode_signal(hash_string_token("ab"), 123, 17, |w| {
        w.number(2.);
        for b in [5, 6, 7] {
            w.byte(b);
        }
    });
    let decoded = decoder.decode_report(&report(golden.clone())).unwrap();
    let Outcome::Decoded {
        value,
        exact_reencode: true,
    } = &decoded[0].outcome
    else {
        panic!("{:?}", decoded[0].outcome)
    };
    assert_eq!(decoder.encode_slot(0, value, 123).unwrap(), golden);
    for body in [
        "p=integerWriter(m,value,b,p);b[p+1]=5^m();b[p+3]=7^m();",
        "p=integerWriter(m,value,b,p);b[p+1]=5^m();b[p+1]=7^m();",
        "p=integerWriter(m,value,b,p);b[p+2]=5^m();",
        "let q=p;p=integerWriter(m,value,b,p);b[q+1]=5^m();",
    ] {
        let (_, document, pin) = synthetic(body);
        assert!(matches!(
            load(&document, &pin)
                .decode_report(&report(vec![0; 8]))
                .unwrap()[0]
                .outcome,
            Outcome::Unsupported { .. }
        ));
    }
}

#[test]
fn source_conditional_result_joins_only_its_accounted_arm_cursors() {
    let body = "let q=flag?(b[4]=130^m(),4):integerWriter(m,value,b,3);b[q+1]=5^m();";
    let (_, document, pin) = synthetic(body);
    let decoder = load(&document, &pin);
    for literal in [true, false] {
        let golden = crate::signals::codec::encode_signal(hash_string_token("ab"), 123, 17, |w| {
            if literal {
                w.byte(130)
            } else {
                w.number(2.)
            }
            w.byte(5)
        });
        let decoded = decoder.decode_report(&report(golden.clone())).unwrap();
        let Outcome::Decoded {
            value,
            exact_reencode: true,
        } = &decoded[0].outcome
        else {
            panic!("{:?}", decoded[0].outcome)
        };
        assert_eq!(decoder.encode_slot(0, value, 123).unwrap(), golden);
    }
    for wrong in [
        "let q=flag?(b[4]=130^m(),5):integerWriter(m,value,b,3);b[q+1]=5^m();",
        "let q=flag?(b[4]=130^m(),4):integerWriter(m,value,b,3);++q;b[q+1]=5^m();",
    ] {
        let (_, document, pin) = synthetic(wrong);
        assert!(matches!(
            load(&document, &pin)
                .decode_report(&report(vec![0; 6]))
                .unwrap()[0]
                .outcome,
            Outcome::Unsupported { .. }
        ));
    }
}

#[test]
fn count_execution_limits_are_unknown_not_source_length_disproof() {
    let (_, document, _) = synthetic_with_setup(
        "p=integerWriter(m,size,b,p);for(let i=0;i<size;++i){b[++p]=items[i]^m();}",
        "let items=[];let size=items.length;",
    );
    let Support::Supported { program: counted } =
        &document.wire.as_ref().unwrap().slots[0].programs[0]
    else {
        panic!()
    };
    assert_eq!(counted.length_bounds().1, None);
    let (_, document, _) = synthetic("b[++p]=a^m();b[++p]=c^m();b[++p]=d^m();p=sw(value,b,p);");
    let Support::Supported { program: other } =
        &document.wire.as_ref().unwrap().slots[0].programs[0]
    else {
        panic!()
    };
    let token = hash_string_token("ab");
    let bytes = crate::signals::codec::encode_signal(token, 123, 17, |w| {
        w.number(70000.);
        for _ in 0..69999 {
            w.byte(65)
        }
        w.byte(0);
    });
    let value = other.decode(&bytes, token).unwrap();
    assert_eq!(other.encode(&value, 123, token).unwrap(), bytes);
    assert_eq!(
        select(
            &[
                Support::Supported {
                    program: counted.clone()
                },
                Support::Supported {
                    program: other.clone()
                }
            ],
            &bytes,
            token
        ),
        Err(WireError::Bounds)
    );
}

#[test]
fn reused_predrawn_scalar_is_not_misrepresented_as_a_new_logical_field() {
    let (_, document, pin) = synthetic("let draw=m();b[++p]=draw^5;b[++p]=draw^9;");
    // Independent reference masks: the source reuses 0xa2, so actual bytes are
    // a7,ab. A second field of 162 with an exact reencode is still incorrect.
    assert!(matches!(
        load(&document, &pin)
            .decode_report(&report(vec![1, 2, 3, 4, 0xa7, 0xab]))
            .unwrap()[0]
            .outcome,
        Outcome::Unsupported { .. }
    ));
    let (_, document, pin) = synthetic("let draw=m();b[++p]=draw^5;draw=m();b[++p]=draw^9;");
    let decoder = load(&document, &pin);
    let golden = vec![1, 2, 3, 4, 0xa7, 0x40];
    let decoded = decoder.decode_report(&report(golden.clone())).unwrap();
    let Outcome::Decoded {
        value,
        exact_reencode: true,
    } = &decoded[0].outcome
    else {
        panic!("{:?}", decoded[0].outcome)
    };
    assert_eq!(
        *value,
        sequence(vec![WireValue::Byte(5), WireValue::Byte(9)])
    );
    assert_eq!(decoder.encode_slot(0, value, 0x01020304).unwrap(), golden);
}

#[test]
fn no_data_descriptive_entry_keeps_its_routing_and_round_trips() {
    let entry = json!({
        "address": "direct:83",
        "type": "no_data",
        "name": "qsnbepcestpp",
        "fields": {},
        "effect": {"windowFlag": "f5cd836ea16e4a77", "event": "f5cd836ea16e4a77"}
    });
    let parsed: DescriptiveSignal = serde_json::from_value(entry.clone()).unwrap();
    assert!(parsed.no_data);
    assert_eq!(parsed.routing, WireRoute::Name { name: json!("qsnbepcestpp") });
    assert_eq!(serde_json::to_value(&parsed).unwrap(), entry);
    let positional = json!({"address": "wave:1:2", "type": "no_data", "positional": 4, "fields": {}});
    let parsed: DescriptiveSignal = serde_json::from_value(positional.clone()).unwrap();
    assert_eq!(parsed.routing, WireRoute::Positional { positional: 4 });
    assert_eq!(serde_json::to_value(&parsed).unwrap(), positional);
    // Without its routing member a no-data entry is still refused.
    assert!(serde_json::from_value::<DescriptiveSignal>(json!({"address": "direct:1", "type": "no_data"})).is_err());
}
