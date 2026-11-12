//! Synthetic source and independent logical expectations; no captured inputs.
use super::*;
use crate::signals::{
    extract::{
        contracts::*,
        layout::{EmissionVariant, LoopKind, ObserverRoute},
    },
    schema::{project_contract, Routing, SourceSpan},
    wire::{CountExpr, CountRef, FloatByteOrder, Op, Program, Support, WireMask, WireSlot},
};
use oxc_span::Span;

fn byte() -> GrammarNode {
    GrammarNode::Byte(
        serde_json::from_value(json!({
            "span":[0,1],"value_span":[0,1],"value":"input","mask":"unmasked",
            "mask_source":null,"xor_literal":null,"position":null
        }))
        .unwrap(),
    )
}
fn seq(items: Vec<GrammarNode>) -> GrammarNode {
    GrammarNode::Sequence { items }
}
fn repeat(body: GrammarNode) -> GrammarNode {
    GrammarNode::Repeat {
        loop_kind: LoopKind::For,
        direction: LoopDirection::Ascending,
        span: Span::default(),
        header: "i < 2".into(),
        count: Some(2),
        body: Box::new(body),
    }
}
fn choice(arms: Vec<GrammarNode>) -> GrammarNode {
    GrammarNode::Choice {
        kind: ChoiceKind::If,
        span: Span::default(),
        condition: Some("condition".into()),
        arms: arms
            .into_iter()
            .map(|body| ChoiceArm {
                label: "same label".into(),
                body,
            })
            .collect(),
    }
}
fn contract(grammar: GrammarNode) -> SignalContract {
    SignalContract {
        descriptor: 0,
        producer: Producer {
            kind: ProducerKind::Direct { wrapper: None },
            body: None,
            registrations: vec![],
            arguments: vec![],
        },
        status: ContractStatus::Complete,
        reasons: vec![],
        lifecycle: vec![],
        accounting: vec![],
        input_classes: vec![],
        emissions: vec![EmissionContract {
            site: SourceSpan::new(0, 1),
            forwarding_sites: vec![],
            quote: String::new(),
            route: ObserverRoute::Direct,
            phase: None,
            payload: EmissionVariant::RawBuffer,
            stream: None,
            rendered: String::new(),
            grammar,
        }],
    }
}
fn naming(contract: &SignalContract, binding: FieldBinding) -> Naming {
    naming_as(contract, binding, "Synthetic")
}
fn naming_as(contract: &SignalContract, binding: FieldBinding, canonical: &'static str) -> Naming {
    let slot = SlotSummary {
        address: "direct:0".into(),
        descriptor: 0,
        routing: Routing::Positional(0),
        contract_status: ContractStatus::Complete,
        producer: None,
        callbacks: vec![],
        slot_roles: vec![],
        roles_inside: vec![],
        roles_enclosing: vec![],
        layout: project_contract(contract),
        candidates: vec![canonical],
        emitted_as: Some(canonical.into()),
        binding: Some(binding),
    };
    from_slot(&slot, contract).unwrap().unwrap()
}

#[test]
fn named_construction_retains_unproven_flag_bytes_across_emissions_and_nested_controls() {
    let grammar = seq(vec![
        byte(),
        repeat(choice(vec![byte(), byte()])),
        byte(),
        byte(),
    ]);
    let mut c = contract(grammar.clone());
    c.emissions.push(c.emissions[0].clone());
    let mut binding = FieldBinding::new()
        .name(0, "f1", "integrityFlags1")
        .name(0, "f2", "samples")
        .name(0, "f3", "integrityFlags2")
        .name(0, "f4", "integrityFlags3");
    binding.set_flags(0, "f1", BTreeMap::from([("first".into(), 0)]));
    binding.set_flags(0, "f4", BTreeMap::from([("last".into(), 1)]));
    let names = naming_as(&c, binding, "NativeApiIntegrityFlags");
    assert_eq!(names.mapping_error, None);
    assert_eq!(names.parts.len(), 2);
    for (emission, part) in names.parts.iter().enumerate() {
        assert_eq!(part.emission, emission);
        assert_eq!(part.part_leaves, vec![0, 1, 2, 3, 4]);
        assert!(matches!(&part.fields[2].node, Node::Leaf { leaf: 3, kind, .. } if kind == "byte"));
        assert_eq!(part.fields[2].bits, None);
    }
    for version in [1, 2] {
        let mut compatible = names.clone();
        compatible.version = version;
        super::super::semantic::validate(&compatible, &c).unwrap();
    }
    let value = WireValue::Sequence(vec![
        WireValue::Byte(1),
        WireValue::Repeat(vec![
            WireValue::Choice {
                arm: 0,
                value: Box::new(WireValue::Byte(17)),
            },
            WireValue::Choice {
                arm: 1,
                value: Box::new(WireValue::Byte(29)),
            },
        ]),
        WireValue::Byte(203),
        WireValue::Byte(2),
    ]);
    let projected = fields(&grammar, &names, &value);
    assert_eq!(projected["integrityFlags1"], json!({"first":true}));
    assert_eq!(projected["integrityFlags2"], json!(203));
    assert_eq!(projected["integrityFlags3"], json!({"last":true}));
    assert!(!projected.contains_key("integrityChecks"));
}

fn ungrouped_flag_fixture() -> (SignalContract, Naming) {
    ungrouped_flag_fixture_with_middle(byte())
}

fn ungrouped_flag_fixture_with_middle(middle: GrammarNode) -> (SignalContract, Naming) {
    let c = contract(seq(vec![byte(), middle, byte()]));
    let mut binding = FieldBinding::new()
        .name(0, "f1", "integrityFlags1")
        .name(0, "f2", "integrityFlags2")
        .name(0, "f3", "integrityFlags3");
    binding.set_flags(0, "f1", BTreeMap::from([("first".into(), 0)]));
    binding.set_flags(0, "f3", BTreeMap::from([("last".into(), 1)]));
    let names = naming_as(&c, binding, "NativeApiIntegrityFlags");
    (c, names)
}

fn misplaced_flag_fields(names: &Naming) -> Vec<(&'static str, Naming)> {
    let mut nodes = names.clone();
    // Both complete leaf/path nodes remain valid individually and cover the
    // same leaves, but are now attached to the wrong physical keys.
    let first = nodes.parts[0].fields[0].node.clone();
    nodes.parts[0].fields[0].node = nodes.parts[0].fields[1].node.clone();
    nodes.parts[0].fields[1].node = first;
    let mut missing = names.clone();
    missing.parts[0].fields[0].field_key = "f99".into();
    let mut keys = names.clone();
    // Swap keys so they still exist and remain unique: duplicate-key rejection
    // cannot account for rejecting this mutation.
    keys.parts[0].fields[0].field_key = "f2".into();
    keys.parts[0].fields[1].field_key = "f1".into();
    vec![
        ("swapped nodes", nodes),
        ("missing key", missing),
        ("swapped keys", keys),
    ]
}

#[test]
fn ungrouped_flags_validate_physical_keys_without_optional_bit_names() {
    let (c, names) = ungrouped_flag_fixture();
    validate_ungrouped_flag_fixture(c, names);
}

#[test]
fn byte_select_ungrouped_flags_validate_physical_keys_without_optional_bit_names() {
    let (c, names) = byte_select_ungrouped_flag_fixture();
    validate_ungrouped_flag_fixture(c, names);
}

fn byte_select_ungrouped_flag_fixture() -> (SignalContract, Naming) {
    let (c, names) = ungrouped_flag_fixture_with_middle(tagged("condition ? 203 : 0"));
    let layout = project_contract(&c);
    assert!(matches!(
        crate::signals::schema::find_key(&layout.messages[0].fields, "f2"),
        Some((LayoutNode::ByteSelect { leaf, .. }, 1)) if leaf.kind == LeafKind::Byte
    ));
    assert_eq!(names.parts[0].fields[1].bits, None);
    (c, names)
}

fn validate_ungrouped_flag_fixture(c: SignalContract, mut names: Naming) {
    for version in [1, 2] {
        names.version = version;
        super::super::semantic::validate(&names, &c).unwrap();
        let projected = fields(
            &c.emissions[0].grammar,
            &names,
            &WireValue::Sequence(vec![
                WireValue::Byte(0),
                WireValue::Byte(203),
                WireValue::Byte(2),
            ]),
        );
        assert_eq!(projected["integrityFlags1"], json!({"first":false}));
        assert_eq!(projected["integrityFlags2"], json!(203));
        for (case, forged) in misplaced_flag_fields(&names) {
            assert!(
                super::super::semantic::validate(&forged, &c).is_err(),
                "version {version}: {case}"
            );
        }
    }
}

#[test]
fn public_artifact_loader_rejects_misplaced_ungrouped_flag_fields() {
    let (c, names) = ungrouped_flag_fixture();
    validate_ungrouped_flag_artifact(c, names);
}

#[test]
fn public_artifact_loader_rejects_misplaced_byte_select_ungrouped_flag_fields() {
    let (c, names) = byte_select_ungrouped_flag_fixture();
    validate_ungrouped_flag_artifact(c, names);
}

fn validate_ungrouped_flag_artifact(mut c: SignalContract, mut names: Naming) {
    use crate::signals::decode::DecodeArtifact;
    // Reuse the public synthetic artifact envelope; no source compilation or
    // seed generation is needed. This checks consistency, not authenticity.
    let mut artifact = DecodeArtifact::from_json(include_bytes!(
        "../../../sdk/c/tests/fixtures/artifact.json"
    ))
    .unwrap();
    let slot = &mut artifact.schema.wire.as_mut().unwrap().slots[0];
    c.descriptor = slot.contract.descriptor;
    names.schema_key = slot.naming.as_ref().unwrap().schema_key.clone();
    slot.contract = c;
    slot.naming = Some(names.clone());
    slot.programs = vec![Support::Supported {
        program: Program {
            prefix_byte_order: "be32".into(),
            seed_source: "captured_prefix".into(),
            stream_scope: "emission_shared_cursor".into(),
            string_scope: "emission_local_writer_instance".into(),
            salt: 17,
            body: Op::Sequence {
                items: (0..3)
                    .map(|_| Op::Byte {
                        mask: WireMask::None,
                        xor: 0,
                        allowed: None,
                    })
                    .collect(),
            },
        },
    }];
    for version in [1, 2] {
        names.version = version;
        artifact.schema.wire.as_mut().unwrap().slots[0].naming = Some(names.clone());
        DecodeArtifact::from_json(&serde_json::to_vec(&artifact).unwrap()).unwrap();
        for (case, forged) in misplaced_flag_fields(&names) {
            let mut malformed = artifact.clone();
            malformed.schema.wire.as_mut().unwrap().slots[0].naming = Some(forged);
            assert!(
                DecodeArtifact::from_json(&serde_json::to_vec(&malformed).unwrap()).is_err(),
                "version {version}: {case}"
            );
        }
    }
}

fn fields(grammar: &GrammarNode, naming: &Naming, value: &WireValue) -> Map<String, Value> {
    let saved = serde_json::to_vec(naming).unwrap();
    let names: Naming = strict_json(&saved).unwrap();
    let mut runtime = Runtime::default();
    runtime.walk(grammar, value, vec![], 0, &[]).unwrap();
    runtime
        .fields(&names.parts[0].fields, &[], &mut vec![])
        .unwrap()
}

#[test]
fn same_type_permutations_follow_source_keys_after_sorted_json_reload() {
    let c = contract(seq(vec![byte(), byte()]));
    let value = WireValue::Sequence(vec![WireValue::Byte(19), WireValue::Byte(41)]);
    let a = naming(
        &c,
        FieldBinding::new()
            .name(0, "f1", "zeta")
            .name(0, "f2", "alpha"),
    );
    let b = naming(
        &c,
        FieldBinding::new()
            .name(0, "f1", "alpha")
            .name(0, "f2", "zeta"),
    );
    assert_eq!(
        fields(&c.emissions[0].grammar, &a, &value),
        json!({"zeta":19,"alpha":41}).as_object().unwrap().clone()
    );
    assert_eq!(
        fields(&c.emissions[0].grammar, &b, &value),
        json!({"alpha":19,"zeta":41}).as_object().unwrap().clone()
    );
}

#[test]
fn byte_five_is_three_independently_named_booleans() {
    let c = contract(seq(vec![byte()]));
    let mut binding = FieldBinding::new().name(0, "f1", "capabilities");
    binding.set_flags(
        0,
        "f1",
        BTreeMap::from([("alpha".into(), 0), ("beta".into(), 1), ("gamma".into(), 2)]),
    );
    let names = naming(&c, binding);
    let value = WireValue::Sequence(vec![WireValue::Byte(5)]);
    assert_eq!(
        fields(&c.emissions[0].grammar, &names, &value)["capabilities"],
        json!({"alpha":true,"beta":false,"gamma":true})
    );
    let program = Program {
        prefix_byte_order: "be32".into(),
        seed_source: "captured_prefix".into(),
        stream_scope: "emission_shared_cursor".into(),
        string_scope: "emission_local_writer_instance".into(),
        salt: 17,
        body: Op::Sequence {
            items: vec![Op::Byte {
                mask: WireMask::None,
                xor: 0,
                allowed: None,
            }],
        },
    };
    let raw = program.encode(&value, 23, 31).unwrap();
    let decoded = DecodedSlot {
        selection: None,
        positional: 0,
        descriptor: 0,
        address: "direct:0".into(),
        captured_seed: Some(23),
        raw: Some(raw.clone()),
        outcome: Outcome::Decoded {
            value: program.decode(&raw, 31).unwrap(),
            exact_reencode: true,
        },
    };
    let slot = WireSlot {
        address: "direct:0".into(),
        routing: super::super::WireRoute::Positional { positional: 0 },
        contract: c,
        source_plan: vec![],
        programs: vec![Support::Supported { program }],
        naming: Some(names),
    };
    let projected = project(&slot, &decoded, 31, &Err(mapping()));
    assert_eq!(projected.status, NamedStatus::Decoded);
    assert_eq!(
        Value::Object(projected.fields.unwrap()),
        json!({"alpha":true,"beta":false,"gamma":true})
    );
    assert_eq!(projected.metadata["capturedSeed"], 23);
    assert_eq!(projected.metadata["bytes"], json!(raw));
}

#[test]
fn duplicate_canonical_suffix_collisions_never_overwrite() {
    let reserved = BTreeSet::from(["Probe", "Probe_2", "Probe_3"]);
    let mut used = Map::new();
    let mut counts = BTreeMap::<&str, usize>::new();
    let mut keys = vec![];
    for canonical in ["Probe", "Probe", "Probe_2", "Probe", "Probe_3", "Probe_2"] {
        let key = emitted_key(
            canonical,
            counts.entry(canonical).or_default(),
            &reserved,
            &used,
        );
        assert!(used.insert(key.clone(), json!(canonical)).is_none());
        keys.push(key);
    }
    assert_eq!(
        keys,
        vec![
            "Probe",
            "Probe_4",
            "Probe_2",
            "Probe_5",
            "Probe_3",
            "Probe_2_2"
        ]
    );
}

#[test]
fn nested_records_repeats_choices_and_scalar_items_keep_occurrences() {
    let grammar = seq(vec![
        repeat(seq(vec![byte(), choice(vec![byte(), byte()])])),
        repeat(byte()),
        byte(),
    ]);
    let c = contract(grammar.clone());
    let names = naming(
        &c,
        FieldBinding::new()
            .name(0, "f1", "samples")
            .name(0, "f1.item.f1", "count")
            .name(0, "f1.item.f2", "selected")
            .name(0, "f2", "codes")
            .name(0, "f3", "tail"),
    );
    assert_eq!(names.mapping_error, None);
    let value = WireValue::Sequence(vec![
        WireValue::Repeat(vec![
            WireValue::Sequence(vec![
                WireValue::Byte(10),
                WireValue::Choice {
                    arm: 1,
                    value: Box::new(WireValue::Byte(20)),
                },
            ]),
            WireValue::Sequence(vec![
                WireValue::Byte(11),
                WireValue::Choice {
                    arm: 0,
                    value: Box::new(WireValue::Byte(21)),
                },
            ]),
        ]),
        WireValue::Repeat(vec![WireValue::Byte(7), WireValue::Byte(8)]),
        WireValue::Byte(99),
    ]);
    assert_eq!(fields(&grammar,&names,&value),json!({"samples":[{"count":10,"selected":20},{"count":11,"selected":21}],"codes":[7,8],"tail":99}).as_object().unwrap().clone());
}

#[test]
fn empty_repeat_and_absent_branch_are_not_null() {
    let mut empty = repeat(byte());
    if let GrammarNode::Repeat { count, .. } = &mut empty {
        *count = Some(0);
    }
    let grammar = seq(vec![empty, choice(vec![GrammarNode::empty(), byte()])]);
    let c = contract(grammar.clone());
    let names = naming(
        &c,
        FieldBinding::new()
            .name(0, "f1", "items")
            .name(0, "f2", "optional"),
    );
    let value = WireValue::Sequence(vec![
        WireValue::Repeat(vec![]),
        WireValue::Choice {
            arm: 0,
            value: Box::new(WireValue::Sequence(vec![])),
        },
    ]);
    let result = fields(&grammar, &names, &value);
    assert_eq!(result["items"], json!([]));
    assert!(!result.contains_key("optional"));
}

#[test]
fn split_choice_parts_retain_shared_tail_edges_and_message_provenance() {
    let prefix = || {
        GrammarNode::RawPrefix(PrefixNode {
            span: Span::default(),
            spelling: PrefixSpelling::ArrayLiteral,
            seed_word: "seed".into(),
            elements: vec![
                "seed>>>24".into(),
                "seed>>16&255".into(),
                "seed>>8&255".into(),
                "seed&255".into(),
            ],
            seed_binding: None,
        })
    };
    let grammar = seq(vec![
        choice(vec![
            seq(vec![prefix(), byte()]),
            seq(vec![prefix(), byte()]),
        ]),
        byte(),
    ]);
    let c = contract(grammar);
    let names = naming(
        &c,
        FieldBinding::new()
            .name(0, "f1", "branchValue")
            .name(0, "f2", "tail"),
    );
    assert_eq!(names.mapping_error, None);
    assert_eq!(names.parts.len(), 2);
    assert_eq!(names.parts[0].part_leaves, vec![0, 2]);
    assert_eq!(names.parts[1].part_leaves, vec![1, 2]);
    assert_eq!(names.parts[0].message, names.parts[1].message);
    assert_eq!(names.parts[0].split_choice, Some(vec![Step::Child(0)]));
    assert!(matches!(
        &names.parts[0].fields[1].node,
        Node::Leaf { leaf: 2, .. }
    ));
    assert!(matches!(
        &names.parts[1].fields[1].node,
        Node::Leaf { leaf: 2, .. }
    ));
}

#[test]
fn duplicate_sibling_bindings_are_explicit_mapping_errors() {
    let c = contract(seq(vec![byte(), byte()]));
    let names = naming(
        &c,
        FieldBinding::new()
            .name(0, "f1", "same")
            .name(0, "f2", "same"),
    );
    assert!(names.mapping_error.is_some());
    assert!(names.parts.is_empty());
}

#[test]
fn finite_numbers_and_utf16_are_readable_with_lossless_special_policy() {
    let leaf = Node::Leaf {
        leaf: 0,
        grammar_path: vec![],
        kind: "number".into(),
        mask: json!({"mode":"none"}),
        static_xor: None,
        byte_mapping: None,
    };
    for (v, expected) in [
        (WireValue::NumberBits(42f64.to_bits()), json!(42)),
        (
            WireValue::StringUnits("hello 🦀".encode_utf16().collect()),
            json!("hello 🦀"),
        ),
        (
            WireValue::StringUnits(vec![0xd800]),
            json!({"utf16Units":[0xd800]}),
        ),
        (
            WireValue::NumberBits((-0f64).to_bits()),
            json!({"numberBits":(-0f64).to_bits(),"special":"negative_zero"}),
        ),
        (
            WireValue::NumberBits(f64::INFINITY.to_bits()),
            json!({"numberBits":f64::INFINITY.to_bits(),"special":"positive_infinity"}),
        ),
        (
            WireValue::NumberBits(0x7ff8000000000021),
            json!({"numberBits":0x7ff8000000000021u64,"special":"nan"}),
        ),
    ] {
        let mut r = Runtime::default();
        r.leaves.insert((0, vec![]), &v);
        let mut metadata = vec![];
        assert_eq!(r.node(&leaf, &[], &mut metadata).unwrap(), expected);
        assert!(!metadata.is_empty());
    }
}

#[test]
fn duplicate_keys_rejected_in_every_consumed_map() {
    use crate::signals::wire::SchemaDocument;
    let source = json!({"sha256":"x","bytes":1});
    for signals in [
        r#"{"same":{},"same":{}}"#,
        r#"{"x":{"fields":{"a":1,"a":2}}}"#,
        r#"{"x":{"bits":{"a":0,"a":1}}}"#,
    ] {
        let text = format!(r#"{{"source":{source},"signals":{signals},"diagnostics":{{}}}}"#);
        assert!(matches!(
            SchemaDocument::from_json(text.as_bytes()),
            Err(WireError::Json)
        ));
    }
    assert!(
        strict_json::<Value>(br#"{"wire":{"slots":[{"naming":{"fields":{"x":1,"x":2}}}]}}"#)
            .is_err()
    );
    assert!(strict_json::<Value>(br#"{"signals":{},"signals":{}}"#).is_err());
    // Escaped spellings are the same JSON key.
    assert!(strict_json::<Value>(br#"{"a":0,"\u0061":1}"#).is_err());
}

#[test]
fn literal_marker_is_a_number_and_missing_slot_has_no_fields() {
    let c = contract(seq(vec![byte()]));
    let names = naming(&c, FieldBinding::new().name(0, "f1", "marker"));
    assert_eq!(
        fields(
            &c.emissions[0].grammar,
            &names,
            &WireValue::Sequence(vec![WireValue::Byte(130)])
        )["marker"],
        json!(130)
    );
    let slot = WireSlot {
        address: "direct:0".into(),
        routing: super::super::WireRoute::Positional { positional: 0 },
        contract: c,
        source_plan: vec![],
        programs: vec![],
        naming: Some(names),
    };
    let missing = DecodedSlot {
        selection: None,
        positional: 0,
        descriptor: 0,
        address: "direct:0".into(),
        captured_seed: None,
        raw: None,
        outcome: Outcome::Missing,
    };
    let result = project(&slot, &missing, 0, &Err(mapping()));
    assert_eq!(result.status, NamedStatus::Missing);
    assert!(result.fields.is_none());
}

#[test]
fn metadata_masks_static_xor_and_keystream_do_not_move_decoder_cursor() {
    let program = Program {
        prefix_byte_order: "be32".into(),
        seed_source: "captured_prefix".into(),
        stream_scope: "emission_shared_cursor".into(),
        string_scope: "emission_local_writer_instance".into(),
        salt: 17,
        body: Op::Sequence {
            items: vec![
                Op::Byte {
                    mask: WireMask::Inherit,
                    xor: 3,
                    allowed: None,
                },
                Op::Byte {
                    mask: WireMask::None,
                    xor: 7,
                    allowed: None,
                },
                Op::Byte {
                    mask: WireMask::Inherit,
                    xor: 0,
                    allowed: None,
                },
            ],
        },
    };
    let value = WireValue::Sequence(vec![
        WireValue::Byte(5),
        WireValue::Byte(9),
        WireValue::Byte(11),
    ]);
    let raw = program.encode(&value, 23, 31).unwrap();
    let before = program.decode(&raw, 31).unwrap();
    let details = physical(&program, &value, &raw, 31, &Err(mapping())).unwrap();
    assert_eq!(details[0]["staticXorAppliedBytes"], json!([6]));
    assert_eq!(details[1]["bytes"], json!([14]));
    assert_eq!(details[1]["keystream"], json!([]));
    assert_eq!(details[2]["offset"], 6);
    assert_eq!(program.decode(&raw, 31).unwrap(), before);
    assert_eq!(program.encode(&before, 23, 31).unwrap(), raw);
}

#[test]
fn full_report_restores_after_serializing_named_document_and_display_edits() {
    use crate::signals::payload::superpack::Value as Raw;
    let report = Report {
        errors: vec![Raw::Null],
        positional_signals: vec![None, Some((vec![Raw::Bool(false)], vec![1, 2, 3, 4, 255]))],
        signals: vec![Raw::Bool(true)],
        keyed_signals: vec![(9, None), (11, Some((vec![Raw::Null], vec![4, 3, 2, 1])))],
        seeds: [19, 23],
        keysets: vec![vec!["x".into()]],
    };
    let doc = NamedDocument {
        format_version: 1,
        signals: BTreeMap::new(),
        unresolved_slots: vec![],
        metadata: RoundTrip {
            initialization: None,
            decoder_options: super::super::TypedDecoderOptions::default(),
            original_report: report.clone(),
        },
    };
    let mut saved = serde_json::to_value(&doc).unwrap();
    saved["signals"] =
        json!({"Synthetic":{"status":"decoded","fields":{"edited":123},"metadata":{}}});
    let restored = NamedDocument::from_json(&serde_json::to_vec(&saved).unwrap())
        .unwrap()
        .restore_original_report();
    assert_eq!(restored, report);
    assert_eq!(restored.to_payload().unwrap(), report.to_payload().unwrap());
}

#[test]
fn checked_construction_refuses_missing_or_reordered_summaries() {
    use crate::signals::extract::{
        contracts::signal_contracts_from_inputs, inputs::extract_program_inputs,
    };
    let source="function main(){function wrap(t){return function(o){t(o);};}let task=wrap(function(o){o.next([[],[1,2,3,4]]);});let registry=[{ctor:task,priority:false}];registry.forEach(e=>{let t=e.ctor;if(t)t();});}main();";
    let inputs = extract_program_inputs(source).unwrap();
    let contracts = signal_contracts_from_inputs(source, &inputs).unwrap();
    assert_eq!(contracts.signals.len(), 1);
    assert!(matches!(
        super::super::WireSchema::from_analysis(source, &inputs, &contracts, &[]),
        Err(WireError::Evidence)
    ));
    let analysis =
        crate::signals::analysis::analyze_program_signals(source, &Default::default()).unwrap();
    let mut two = contracts.clone();
    let mut other = two.signals[0].clone();
    other.descriptor = 1;
    two.signals.push(other);
    let first = analysis.slots[0].clone();
    let mut second = first.clone();
    second.descriptor = 1;
    second.address = "direct:1".into();
    assert!(matches!(
        super::super::WireSchema::from_analysis(source, &inputs, &two, &[second, first]),
        Err(WireError::Evidence)
    ));
}

/// This small static fixture satisfies pin extraction without executing JS.
/// The slot intentionally has an uncertified codec: public load still must
/// preserve its mapping/unsupported status and every original report channel.
fn public_source() -> String {
    r#"function main(){
function cipher(keys,a,b){let state=[1,2,3,4,keys[0],keys[1],keys[2],keys[3],keys[4],keys[5],keys[6],keys[7],0,0,a,b];let counter=5;state[12]=counter|0;while(a){a=a<<16;a=a<<16;a=a<<16;a=a<<16;a=a<<16;a=a<<16;a=a<<16;a=a<<16;}}
function wrap(t){return function(o){t(o);};}
let task=wrap(function(o){let seed=Math.random()*4294967296|0;let b=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];b[4]=("hasStorageAccess" in document)|("requestStorageAccess" in document)<<1;o.next([[],b]);});
let registry=[{ctor:task,priority:false}];registry.forEach(e=>{let t=e.ctor;if(t)t();});
let filter=[0];function route(options){let f=options.signalFilter?options.signalFilter:filter;return registry[f[0]];}
let indices=[],table=[];indices.map(i=>table[i]);
let context={};context.c="";context.d="";context.z="";send("x-example-"+"a","payload",1000);
}main();"#.into()
}

#[test]
fn analysis_saved_json_public_load_and_all_channel_retention() {
    use crate::signals::{
        analysis::{analyze_program_signals, SignalFinderOptions},
        payload::{extract::Session, pin},
        wire::{Decoder, SchemaDocument},
    };
    let source = public_source();
    let session = Session {
        transaction_token: "synthetic".into(),
        alphabet: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".into(),
        shuffle_key: vec![],
        keys: [0; 8],
        nonce: None,
    };
    let pinned = pin(&source, &session).unwrap();
    let analysis = analyze_program_signals(&source, &SignalFinderOptions::default()).unwrap();
    assert_eq!(analysis.report.wire.slots.len(), 1);
    assert!(analysis.report.wire.slots[0].naming.is_some());
    let saved = serde_json::to_vec(&analysis.report).unwrap();
    let parsed = SchemaDocument::from_json(&saved).unwrap();
    assert_eq!(
        serde_json::to_value(&parsed).unwrap(),
        serde_json::from_slice::<Value>(&saved).unwrap()
    );
    let decoder = Decoder::load(&saved, &source, &pinned.layout).unwrap();
    let options = super::super::TypedDecoderOptions {
        float_byte_order: super::super::FloatByteOrder::Big,
    };
    let configured = Decoder::load_with_options(&saved, &source, &pinned.layout, options).unwrap();
    let mut old = serde_json::from_slice::<Value>(&saved).unwrap();
    old["wire"].as_object_mut().unwrap().remove("number_float");
    assert!(Decoder::load_with_options(
        &serde_json::to_vec(&old).unwrap(),
        &source,
        &pinned.layout,
        options
    )
    .is_ok());
    for (field, value) in [
        ("marker", json!(127)),
        ("width", json!(4)),
        (
            "writer",
            json!({"codec":"forged","span":null,"sha256":null,"reason":null}),
        ),
    ] {
        let mut forged = serde_json::from_slice::<Value>(&saved).unwrap();
        forged["wire"]["number_float"][field] = value;
        assert!(matches!(
            Decoder::load_with_options(
                &serde_json::to_vec(&forged).unwrap(),
                &source,
                &pinned.layout,
                options
            ),
            Err(WireError::Evidence)
        ));
    }

    let signal = serde_json::to_string(parsed.signals.values().next().unwrap()).unwrap();
    let duplicate = format!(
        r#"{{"source":{},"signals":{{"same":{signal},"same":{signal}}},"diagnostics":{{}}}}"#,
        serde_json::to_string(&parsed.source).unwrap()
    );
    assert!(matches!(
        SchemaDocument::from_json(duplicate.as_bytes()),
        Err(WireError::Json)
    ));
    let nested = String::from_utf8(saved.clone()).unwrap().replacen(
        "\"fields\":{",
        "\"fields\":{\"duplicate\":0,\"duplicate\":1,",
        1,
    );
    assert!(matches!(
        Decoder::load(nested.as_bytes(), &source, &pinned.layout),
        Err(WireError::Json)
    ));
    use crate::signals::payload::superpack::Value as Raw;
    let report = Report {
        errors: vec![Raw::Null],
        positional_signals: vec![Some((vec![Raw::Bool(false)], vec![1, 2, 3, 4]))],
        signals: vec![Raw::Bool(true)],
        keyed_signals: vec![(17, None), (18, Some((vec![Raw::Null], vec![6, 7, 8, 9])))],
        seeds: [17, 18],
        keysets: vec![],
    };
    let configured_named = configured.decode_named_report(&report).unwrap();
    assert_eq!(configured_named.metadata.decoder_options, options);
    assert_eq!(configured_named.restore_original_report(), report);
    let named = decoder.decode_named_report(&report).unwrap();
    assert_eq!(named.unresolved_slots.len() + named.signals.len(), 1);
    assert_eq!(
        named.signals.values().next().unwrap().status,
        NamedStatus::Unsupported
    );
    assert_eq!(
        NamedDocument::from_json(&serde_json::to_vec(&named).unwrap())
            .unwrap()
            .restore_original_report(),
        report
    );
    let mut altered = source.clone();
    altered.push(' ');
    assert!(matches!(
        Decoder::load(&saved, &altered, &pinned.layout),
        Err(WireError::Source)
    ));
    let mut badpin = pinned.layout.clone();
    badpin.cipher.counter += 1;
    assert!(matches!(
        Decoder::load(&saved, &source, &badpin),
        Err(WireError::Pin)
    ));
    let mut forged = serde_json::from_slice::<Value>(&saved).unwrap();
    forged["wire"]["slots"][0]["naming"]["parts"][0]["fields"][0]["name"] = json!("forged");
    assert!(matches!(
        Decoder::load(
            &serde_json::to_vec(&forged).unwrap(),
            &source,
            &pinned.layout
        ),
        Err(WireError::Evidence)
    ));
}

#[test]
fn public_loader_rederives_even_guarded_control_metadata() {
    use crate::signals::{
        analysis::analyze_program_signals,
        payload::{extract::Session, pin},
        wire::Decoder,
    };
    let source = public_source();
    let session = Session {
        transaction_token: "synthetic".into(),
        alphabet: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".into(),
        shuffle_key: vec![],
        keys: [0; 8],
        nonce: None,
    };
    let pinned = pin(&source, &session).unwrap();
    let analysis = analyze_program_signals(&source, &Default::default()).unwrap();
    let mut saved = serde_json::to_value(&analysis.report).unwrap();
    assert!(Decoder::load(
        &serde_json::to_vec(&saved).unwrap(),
        &source,
        &pinned.layout
    )
    .is_ok());
    // Unsupported algorithms do not grant an exemption from source-plan checks.
    saved["wire"]["slots"][0]["source_plan"][0]["root_cases"] = json!([{"arm":0,"prefix":{"start":1,"end":2},"stream":{"start":3,"end":4},"seed_binding":{"start":5,"end":6},"salt":17,"minimum_length":4,"maximum_length":5}]);
    assert!(matches!(
        Decoder::load(
            &serde_json::to_vec(&saved).unwrap(),
            &source,
            &pinned.layout
        ),
        Err(WireError::Evidence)
    ));
}

#[test]
fn packed_flags_skip_only_certified_empty_flushes_and_keep_occurrences() {
    let grammar = seq(vec![repeat(choice(vec![byte(), seq(vec![])]))]);
    let mut binding = FieldBinding::new().name(0, "f1", "capabilities");
    binding.set_flags(
        0,
        "f1",
        BTreeMap::from([("alpha".into(), 0), ("beta".into(), 1)]),
    );
    let names = naming(&contract(grammar.clone()), binding);
    let value = WireValue::Sequence(vec![WireValue::Repeat(vec![
        WireValue::Choice {
            arm: 1,
            value: Box::new(WireValue::Sequence(vec![])),
        },
        WireValue::Choice {
            arm: 0,
            value: Box::new(WireValue::Byte(1)),
        },
    ])]);
    let mut runtime = Runtime::default();
    runtime.walk(&grammar, &value, vec![], 0, &[]).unwrap();
    assert!(runtime
        .fields(&names.parts[0].fields, &[], &mut vec![])
        .is_err());
    let path = vec![Step::Child(0), Step::Body];
    runtime.packed_choices.insert(path.clone());
    let mut details = Vec::new();
    let fields = runtime
        .fields(&names.parts[0].fields, &[], &mut details)
        .unwrap();
    assert_eq!(fields["capabilities"], json!({"alpha":true,"beta":false}));
    assert_eq!(runtime.controls[&(vec![Step::Child(0)], vec![])], 2);
    assert_eq!(runtime.order, vec![(0, vec![1])]);
    assert!(details
        .iter()
        .any(|d| d["packedFlush"] == true && d["occurrences"] == json!([0])));
    // A certificate for a different source occurrence cannot authorize this one.
    runtime.packed_choices.clear();
    runtime
        .packed_choices
        .insert(vec![Step::Child(1), Step::Body]);
    assert!(runtime
        .fields(&names.parts[0].fields, &[], &mut vec![])
        .is_err());
}

fn semantic_names(c: &SignalContract, name: &str, field: &str) -> Naming {
    let mut n = naming(c, FieldBinding::new().name(0, "f1", field));
    n.canonical_name = name.into();
    crate::signals::wire::semantic::bind(&mut n, c);
    n
}
fn tagged(value: &str) -> GrammarNode {
    let mut g = byte();
    if let GrammarNode::Byte(w) = &mut g {
        w.value = value.into();
    }
    g
}
#[test]
fn source_boolean_tags_are_reversible_even_when_true_is_zero() {
    for (condition, yes, no) in [
        ("trusted", 0, 5),
        ("!trusted", 210, 31),
        ("trusted === false", 99, 17),
        ("!!trusted", 9, 240),
    ] {
        let g = seq(vec![tagged(&format!("{condition} ? {yes} : {no}"))]);
        let c = contract(g.clone());
        let names = semantic_names(&c, "UiEventIsTrusted", "isTrusted");
        let Node::Leaf {
            byte_mapping: Some(mapping),
            ..
        } = &names.parts[0].fields[0].node
        else {
            panic!("boolean mapping absent")
        };
        let positive = condition == "trusted" || condition == "!!trusted";
        for (byte, expected) in [(yes, positive), (no, !positive)] {
            let logical = fields(
                &g,
                &names,
                &WireValue::Sequence(vec![WireValue::Byte(byte)]),
            )["isTrusted"]
                .clone();
            assert_eq!(logical, json!(expected));
            assert_eq!(mapping.encode(&logical).unwrap(), byte);
        }
        assert!(mapping.decode(42).is_err());
        assert!(mapping.encode(&json!(1)).is_err());
    }
}
#[test]
fn nullable_source_guards_decode_marker_to_null_and_preserve_real_values() {
    for (condition, null_index, tag) in [
        ("value == null", 0, 133),
        ("null === value", 0, 199),
        ("value != null", 1, 131),
        ("!(value !== null)", 0, 130),
        ("!(void 0 == value)", 1, 0),
    ] {
        let mut value = byte();
        if let GrammarNode::Byte(w) = value {
            value = GrammarNode::Number(w);
        }
        let mut g = choice(vec![tagged(&tag.to_string()), value]);
        if let GrammarNode::Choice {
            condition: c, arms, ..
        } = &mut g
        {
            *c = Some(condition.into());
            if null_index == 1 {
                arms.swap(0, 1);
            }
        }
        let GrammarNode::Byte(w) = byte() else {
            unreachable!()
        };
        let grammar = seq(vec![GrammarNode::String(w), g]);
        let c = contract(grammar.clone());
        let mut names = naming(
            &c,
            FieldBinding::new()
                .name(0, "f1", "prefix")
                .name(0, "f2", "number"),
        );
        names.canonical_name = "NullDerefTypeErrorFields".into();
        crate::signals::wire::semantic::bind(&mut names, &c);
        let null = WireValue::Sequence(vec![
            WireValue::StringUnits(vec![120]),
            WireValue::Choice {
                arm: null_index,
                value: Box::new(WireValue::Byte(tag)),
            },
        ]);
        assert_eq!(fields(&grammar, &names, &null)["number"], Value::Null);
        // The same numeric value is data on the non-null arm, not a null marker.
        let number = WireValue::Sequence(vec![
            WireValue::StringUnits(vec![120]),
            WireValue::Choice {
                arm: 1 - null_index,
                value: Box::new(WireValue::NumberBits((tag as f64).to_bits())),
            },
        ]);
        assert_eq!(fields(&grammar, &names, &number)["number"], json!(tag));
        let Node::Choice { arms, .. } = &names.parts[0].fields[1].node else {
            panic!("choice missing")
        };
        let Node::Leaf {
            byte_mapping: Some(map),
            ..
        } = &arms[null_index]
        else {
            panic!("null mapping missing")
        };
        assert_eq!(map.encode(&Value::Null).unwrap(), tag);
        assert!(map.encode(&json!(false)).is_err());
    }
}

#[test]
fn semantic_inverse_changes_actual_encoded_bytes() {
    let g = seq(vec![tagged("trusted ? 0 : 5")]);
    let c = contract(g.clone());
    let names = semantic_names(&c, "UiEventIsTrusted", "isTrusted");
    let value = WireValue::Sequence(vec![WireValue::Byte(0)]);
    let mut runtime = Runtime::default();
    runtime.walk(&g, &value, vec![], 0, &[]).unwrap();
    let mut details = vec![];
    let logical = runtime
        .fields(&names.parts[0].fields, &[], &mut details)
        .unwrap();
    assert_eq!(logical["isTrusted"], json!(true));
    let program = Program {
        prefix_byte_order: "be32".into(),
        seed_source: "captured_prefix".into(),
        stream_scope: "collector_local".into(),
        string_scope: "collector_local".into(),
        salt: 17,
        body: Op::Sequence {
            items: vec![Op::Byte {
                mask: WireMask::Inherit,
                xor: 19,
                allowed: Some(vec![0, 5]),
            }],
        },
    };
    let original = program.encode(&value, 23, 31).unwrap();
    let rebuilt = rebuild_tags(&value, &runtime.order, &details).unwrap();
    assert_eq!(program.encode(&rebuilt, 23, 31).unwrap(), original);
    details[0]["logicalValue"] = json!(false);
    let edited = rebuild_tags(&value, &runtime.order, &details).unwrap();
    assert_eq!(edited, WireValue::Sequence(vec![WireValue::Byte(5)]));
    let bytes = program.encode(&edited, 23, 31).unwrap();
    assert_ne!(bytes, original);
    assert_eq!(program.decode(&bytes, 31).unwrap(), edited);
    details[0]["logicalValue"] = json!(5);
    assert!(rebuild_tags(&value, &runtime.order, &details).is_err());
}

#[test]
fn numeric_bytes_and_unproven_null_records_remain_numeric() {
    // A ternary does not turn a genuine byte counter into a boolean.
    let g = seq(vec![tagged("count ? 0 : 5")]);
    let c = contract(g.clone());
    let n = semantic_names(&c, "DeviceMemory", "deviceMemory");
    assert_eq!(
        fields(&g, &n, &WireValue::Sequence(vec![WireValue::Byte(5)]))["deviceMemory"],
        json!(5)
    );
    // A guard authorizes only a standalone marker, never all bytes in a record.
    let mut guarded = choice(vec![seq(vec![tagged("130"), tagged("131")]), byte()]);
    if let GrammarNode::Choice { condition, .. } = &mut guarded {
        *condition = Some("other == null".into());
    }
    let g = seq(vec![guarded]);
    let c = contract(g.clone());
    let mut n = naming(
        &c,
        FieldBinding::new()
            .name(0, "f1", "optional")
            .name(0, "f1.arm1.f1", "first")
            .name(0, "f1.arm1.f2", "second"),
    );
    n.canonical_name = "NullDerefTypeErrorFields".into();
    crate::signals::wire::semantic::bind(&mut n, &c);
    assert_eq!(n.mapping_error, None);
    assert!(!n.parts.is_empty());
    fn no_mapping(n: &Node) {
        match n {
            Node::Leaf { byte_mapping, .. } => assert!(byte_mapping.is_none()),
            Node::Choice { arms, .. } => arms.iter().for_each(no_mapping),
            Node::Record { fields } => fields.iter().for_each(|f| no_mapping(&f.node)),
            _ => {}
        }
    }
    n.parts
        .iter()
        .flat_map(|p| &p.fields)
        .for_each(|f| no_mapping(&f.node));
}

#[test]
fn serialized_semantic_mappings_reject_ambiguous_and_forged_cases() {
    use crate::signals::wire::semantic::{self, ByteMapping};
    let c = contract(seq(vec![tagged("trusted ? 0 : 5")]));
    let n = semantic_names(&c, "UiEventIsTrusted", "isTrusted");
    assert!(semantic::validate(&n, &c).is_ok());
    let Node::Leaf {
        byte_mapping: Some(map),
        ..
    } = &n.parts[0].fields[0].node
    else {
        panic!()
    };
    for cases in [
        json!([]),
        json!([{"byte":0,"value":true},{"byte":0,"value":false}]),
        json!([{"byte":0,"value":true},{"byte":5,"value":true}]),
    ] {
        let mut saved = serde_json::to_value(map).unwrap();
        saved["cases"] = cases;
        assert!(serde_json::from_value::<ByteMapping>(saved).is_err());
    }
    let mut forged = n.clone();
    let Node::Leaf {
        byte_mapping: Some(map),
        ..
    } = &mut forged.parts[0].fields[0].node
    else {
        panic!()
    };
    map.cases[0].byte = 99;
    assert_eq!(semantic::validate(&forged, &c), Err(WireError::Evidence));
}

#[test]
fn nested_nullable_records_keep_payload_values_and_occurrence_identity() {
    let GrammarNode::Byte(w) = byte() else {
        unreachable!()
    };
    let mut nullable = choice(vec![tagged("(130)"), GrammarNode::Number(w)]);
    if let GrammarNode::Choice { condition, .. } = &mut nullable {
        *condition = Some("!(sample != null)".into());
    }
    let g = seq(vec![repeat(seq(vec![tagged("130"), nullable]))]);
    let c = contract(g.clone());
    let mut names = naming(
        &c,
        FieldBinding::new()
            .name(0, "f1", "samples")
            .name(0, "f1.item.f1", "count")
            .name(0, "f1.item.f2", "value"),
    );
    crate::signals::wire::semantic::bind(&mut names, &c);
    assert_eq!(names.mapping_error, None);
    let value = WireValue::Sequence(vec![WireValue::Repeat(vec![
        WireValue::Sequence(vec![
            WireValue::Byte(130),
            WireValue::Choice {
                arm: 0,
                value: Box::new(WireValue::Byte(130)),
            },
        ]),
        WireValue::Sequence(vec![
            WireValue::Byte(130),
            WireValue::Choice {
                arm: 1,
                value: Box::new(WireValue::NumberBits(130f64.to_bits())),
            },
        ]),
    ])]);
    let mut runtime = Runtime::default();
    runtime.walk(&g, &value, vec![], 0, &[]).unwrap();
    let mut details = vec![];
    assert_eq!(
        runtime
            .fields(&names.parts[0].fields, &[], &mut details)
            .unwrap(),
        json!({"samples":[{"count":130,"value":null},{"count":130,"value":130}]})
            .as_object()
            .unwrap()
            .clone()
    );
    assert_eq!(
        rebuild_tags(&value, &runtime.order, &details).unwrap(),
        value
    );
    assert_eq!(
        details
            .iter()
            .filter(|d| d.get("byteMapping").is_some())
            .count(),
        1
    );
}

#[test]
fn documented_null_fields_are_scoped_to_the_card_message() {
    let c = contract(seq(vec![tagged("130")]));
    let mut names = semantic_names(&c, "HostEnvironment", "errorMessage");
    let value = WireValue::Sequence(vec![WireValue::Byte(130)]);
    assert_eq!(
        fields(&c.emissions[0].grammar, &names, &value)["errorMessage"],
        Value::Null
    );
    names.parts[0].message = 1;
    crate::signals::wire::semantic::bind(&mut names, &c);
    assert_eq!(
        fields(&c.emissions[0].grammar, &names, &value)["errorMessage"],
        json!(130)
    );
}
#[test]
fn unavailable_measurement_slots_invert_build_specific_tags_without_changing_status() {
    for tags in [[130, 131, 132], [0, 255, 17], [99, 8, 203]] {
        let GrammarNode::Byte(w) = tagged("3") else {
            unreachable!()
        };
        let g = seq(vec![
            GrammarNode::Number(w),
            tagged(&tags[0].to_string()),
            tagged(&tags[1].to_string()),
            tagged(&tags[2].to_string()),
        ]);
        let c = contract(g.clone());
        let mut names = naming(
            &c,
            FieldBinding::new()
                .name(0, "f1", "status")
                .name(0, "f2", "totalBytesPlaceholder")
                .name(0, "f3", "breakdownCountPlaceholder")
                .name(0, "f4", "entriesPlaceholder"),
        );
        names.canonical_name = "MemoryBreakdownSummary".into();
        crate::signals::wire::semantic::bind(&mut names, &c);
        let value = WireValue::Sequence(vec![
            WireValue::NumberBits(3f64.to_bits()),
            WireValue::Byte(tags[0]),
            WireValue::Byte(tags[1]),
            WireValue::Byte(tags[2]),
        ]);
        let mut runtime = Runtime::default();
        runtime.walk(&g, &value, vec![], 0, &[]).unwrap();
        let mut details = vec![];
        assert_eq!(
            Value::Object(
                runtime
                    .fields(&names.parts[0].fields, &[], &mut details)
                    .unwrap()
            ),
            json!({"status":3,"totalBytesPlaceholder":null,"breakdownCountPlaceholder":null,"entriesPlaceholder":null})
        );
        assert_eq!(
            rebuild_tags(&value, &runtime.order, &details).unwrap(),
            value
        );
        // The same field names in a result message do not acquire init evidence.
        names.parts[0].message = 1;
        crate::signals::wire::semantic::bind(&mut names, &c);
        assert_eq!(
            fields(&g, &names, &value)["totalBytesPlaceholder"],
            json!(tags[0])
        );
    }
}

#[test]
fn host_success_and_failure_null_slots_keep_the_other_string() {
    for (message, absent, present) in [
        (0, "errorMessage", "hostEnvironmentValue"),
        (1, "hostEnvironmentValue", "errorMessage"),
    ] {
        let GrammarNode::Byte(w) = byte() else {
            unreachable!()
        };
        let g = seq(vec![tagged("203"), GrammarNode::String(w)]);
        let c = contract(g.clone());
        let mut n = naming(
            &c,
            FieldBinding::new()
                .name(0, "f1", absent)
                .name(0, "f2", present),
        );
        n.canonical_name = "HostEnvironment".into();
        n.parts[0].message = message;
        crate::signals::wire::semantic::bind(&mut n, &c);
        let value = WireValue::Sequence(vec![
            WireValue::Byte(203),
            WireValue::StringUnits(vec![111, 107]),
        ]);
        assert_eq!(
            fields(&g, &n, &value),
            Map::from_iter([(absent.into(), Value::Null), (present.into(), json!("ok"))])
        );
        let Node::Leaf {
            byte_mapping: Some(mapping),
            ..
        } = &n.parts[0].fields[0].node
        else {
            panic!()
        };
        assert_eq!(mapping.encode(&Value::Null).unwrap(), 203);
        n.parts[0].message = 1 - message;
        crate::signals::wire::semantic::bind(&mut n, &c);
        assert_eq!(fields(&g, &n, &value)[absent], json!(203));
    }
}

#[test]
fn scalar_null_components_use_only_their_scoped_card_evidence() {
    let g = seq(vec![choice(vec![
        seq(vec![tagged("7"), tagged("1")]),
        tagged("7"),
    ])]);
    let c = contract(g.clone());
    for signal in ["NavigatorProperties", "WebGlContext", "DeviceMemory"] {
        let mut n = naming(
            &c,
            FieldBinding::new()
                .name(0, "f1", "innerWidth")
                .name(0, "f1.arm1.f1", "nullMarker")
                .name(0, "f1.arm1.f2", "nullFlag"),
        );
        n.canonical_name = signal.into();
        crate::signals::wire::semantic::bind(&mut n, &c);
        let value = WireValue::Sequence(vec![WireValue::Choice {
            arm: 0,
            value: Box::new(WireValue::Sequence(vec![
                WireValue::Byte(7),
                WireValue::Byte(1),
            ])),
        }]);
        let expected = if signal == "NavigatorProperties" {
            Value::Null
        } else {
            json!({"nullMarker":7,"nullFlag":1})
        };
        assert_eq!(fields(&g, &n, &value)["innerWidth"], expected);
        let mut runtime = Runtime::default();
        runtime.walk(&g, &value, vec![], 0, &[]).unwrap();
        let mut details = vec![];
        runtime
            .fields(&n.parts[0].fields, &[], &mut details)
            .unwrap();
        assert_eq!(
            rebuild_tags(&value, &runtime.order, &details).unwrap(),
            value
        );
    }
}

#[test]
fn single_list_cards_apply_to_later_repeated_visibility_records() {
    let GrammarNode::Byte(w) = byte() else {
        unreachable!()
    };
    let g = seq(vec![repeat(seq(vec![
        tagged("visible ? 0 : 203"),
        GrammarNode::Number(w),
    ]))]);
    let c = contract(g.clone());
    let mut n = naming(
        &c,
        FieldBinding::new()
            .name(0, "f1", "records")
            .name(0, "f1.item.f1", "visible")
            .name(0, "f1.item.f2", "timestamp"),
    );
    n.canonical_name = "VisibilityEventLog".into();
    n.parts[0].message = 1;
    crate::signals::wire::semantic::bind(&mut n, &c);
    let value = WireValue::Sequence(vec![WireValue::Repeat(vec![
        WireValue::Sequence(vec![
            WireValue::Byte(0),
            WireValue::NumberBits(1f64.to_bits()),
        ]),
        WireValue::Sequence(vec![
            WireValue::Byte(203),
            WireValue::NumberBits(2f64.to_bits()),
        ]),
    ])]);
    assert_eq!(
        fields(&g, &n, &value)["records"],
        json!([{"visible":true,"timestamp":1},{"visible":false,"timestamp":2}])
    );
}

#[test]
fn nullable_card_preserves_real_numbers_equal_to_missing_tags() {
    for tag in [0, 130, 131, 132, 255] {
        let GrammarNode::Byte(w) = byte() else {
            unreachable!()
        };
        let g = seq(vec![choice(vec![
            tagged(&tag.to_string()),
            GrammarNode::Number(w),
        ])]);
        let c = contract(g.clone());
        let n = semantic_names(&c, "NavigatorProperties", "devicePixelRatio");
        for arm in [0, 1] {
            let payload = if arm == 0 {
                WireValue::Byte(tag)
            } else {
                WireValue::NumberBits((tag as f64).to_bits())
            };
            let value = WireValue::Sequence(vec![WireValue::Choice {
                arm,
                value: Box::new(payload),
            }]);
            assert_eq!(
                fields(&g, &n, &value)["devicePixelRatio"],
                if arm == 0 { Value::Null } else { json!(tag) }
            );
        }
    }
}

#[test]
fn nested_attribution_markers_are_reversible_and_do_not_reach_unlisted_children() {
    let g = seq(vec![repeat(seq(vec![
        tagged("17"),
        repeat(seq(vec![tagged("17"), tagged("17")])),
    ]))]);
    let c = contract(g.clone());
    let mut n = naming(
        &c,
        FieldBinding::new()
            .name(0, "f1", "entry")
            .name(0, "f1.item.f1", "attributionCount")
            .name(0, "f1.item.f2", "attribution")
            .name(0, "f1.item.f2.item.f1", "containerMarker")
            .name(0, "f1.item.f2.item.f2", "count"),
    );
    assert_eq!(n.mapping_error, None);
    n.canonical_name = "MemoryBreakdownSummary".into();
    n.parts[0].message = 1;
    crate::signals::wire::semantic::bind(&mut n, &c);
    let attribution = WireValue::Sequence(vec![WireValue::Byte(17), WireValue::Byte(17)]);
    let value = WireValue::Sequence(vec![WireValue::Repeat(vec![WireValue::Sequence(vec![
        WireValue::Byte(17),
        WireValue::Repeat(vec![attribution.clone(), attribution]),
    ])])]);
    let mut runtime = Runtime::default();
    runtime.walk(&g, &value, vec![], 0, &[]).unwrap();
    let mut details = vec![];
    assert_eq!(
        Value::Object(
            runtime
                .fields(&n.parts[0].fields, &[], &mut details)
                .unwrap()
        ),
        json!({"entry":[{"attributionCount":17,"attribution":[{"containerMarker":null,"count":17},{"containerMarker":null,"count":17}]}]})
    );
    assert_eq!(
        rebuild_tags(&value, &runtime.order, &details).unwrap(),
        value
    );
    assert_eq!(
        details
            .iter()
            .filter(|d| d.get("byteMapping").is_some())
            .count(),
        2
    );
}
#[test]
fn grouped_async_null_marker_does_not_consume_packed_flags() {
    let g = seq(vec![choice(vec![
        seq(vec![tagged("203"), tagged("131")]),
        tagged("131"),
    ])]);
    let c = contract(g.clone());
    let mut n = naming(
        &c,
        FieldBinding::new()
            .name(0, "f1", "asyncState")
            .name(0, "f1.arm1.f1", "scriptAsync")
            .name(0, "f1.arm1.f2", "scriptFlags"),
    );
    n.canonical_name = "CurrentScriptAsyncPolicyState".into();
    crate::signals::wire::semantic::bind(&mut n, &c);
    let value = WireValue::Sequence(vec![WireValue::Choice {
        arm: 0,
        value: Box::new(WireValue::Sequence(vec![
            WireValue::Byte(203),
            WireValue::Byte(131),
        ])),
    }]);
    assert_eq!(
        fields(&g, &n, &value)["asyncState"],
        json!({"scriptAsync":null,"scriptFlags":131})
    );
}

#[test]
fn all_six_fixed_flag_families_keep_numeric_index_schema_and_physical_order() {
    use crate::signals::wire::{readable, semantic};
    for rule in readable::GROUPS {
        // Deliberately place chunk 10 before chunk 2, with siblings interleaved.
        let mut order: Vec<_> = (0..rule.count).collect();
        order.reverse();
        let mut binding = FieldBinding::new();
        let mut grammar = Vec::new();
        let mut values = Vec::new();
        for (position, index) in order.iter().enumerate() {
            let key = format!("f{}", position * 2 + 1);
            binding.insert(0, key.clone(), rule.old(*index));
            let count = if *index == rule.count - 1 { 4 } else { 7 };
            binding.set_flags(
                0,
                key,
                (0..count)
                    .map(|bit| (format!("check{index}_{bit}"), bit))
                    .collect(),
            );
            binding.insert(
                0,
                format!("f{}", position * 2 + 2),
                format!("sibling{index}"),
            );
            grammar.extend([byte(), byte()]);
            values.extend([WireValue::Byte(0xf5), WireValue::Byte(*index as u8)]);
        }
        let mut c = contract(seq(grammar));
        c.emissions.push(c.emissions[0].clone());
        let layout = project_contract(&c);
        let mut described =
            crate::signals::schema::render_signal(&layout, &Routing::Positional(0), Some(&binding))
                .unwrap();
        let old_schema = described.clone();
        let mut n = naming(&c, binding);
        let old = fields(
            &c.emissions[0].grammar,
            &n,
            &WireValue::Sequence(values.clone()),
        );
        n.canonical_name = rule.signal.into();
        readable::group(&mut n).unwrap();
        semantic::validate(&n, &c).unwrap();
        let grouped = fields(
            &c.emissions[0].grammar,
            &n,
            &WireValue::Sequence(values.clone()),
        );
        assert_eq!(grouped[rule.parent].as_array().unwrap().len(), rule.count);
        for i in 0..rule.count {
            assert_eq!(grouped[rule.parent][i], old[&rule.old(i)]);
            assert_eq!(grouped[&format!("sibling{i}")], old[&format!("sibling{i}")]);
        }
        readable::describe(&n, &mut described);
        let schema = if described.get("messages").is_some() {
            &described["messages"][0]
        } else {
            &described
        };
        let old_schema = if old_schema.get("messages").is_some() {
            &old_schema["messages"][0]
        } else {
            &old_schema
        };
        for i in 0..rule.count {
            assert_eq!(
                schema["fields"][rule.parent]["items"][i]["bits"],
                old_schema["fields"][rule.old(i)]["bits"]
            );
        }
        // Projection keeps every concrete byte, including unused high bits.
        let mut runtime = Runtime::default();
        let value = WireValue::Sequence(values);
        runtime
            .walk(&c.emissions[0].grammar, &value, vec![], 0, &[])
            .unwrap();
        let mut details = vec![];
        runtime
            .fields(&n.parts[0].fields, &[], &mut details)
            .unwrap();
        assert_eq!(
            details
                .iter()
                .filter(|d| d.get("logicalBytes").is_some())
                .count(),
            rule.count
        );
        assert!(details
            .iter()
            .filter_map(|d| d.get("logicalBytes"))
            .all(|b| b == &json!([245])));
        assert_eq!(
            rebuild_tags(&value, &runtime.order, &details).unwrap(),
            value
        );
        let program = Program {
            prefix_byte_order: "be32".into(),
            seed_source: "captured_prefix".into(),
            stream_scope: "emission_shared_cursor".into(),
            string_scope: "emission_local_writer_instance".into(),
            salt: 17,
            body: Op::Sequence {
                items: (0..rule.count * 2)
                    .map(|_| Op::Byte {
                        mask: WireMask::None,
                        xor: 73,
                        allowed: None,
                    })
                    .collect(),
            },
        };
        let raw = program.encode(&value, 23, 31).unwrap();
        assert_eq!(
            program
                .encode(
                    &rebuild_tags(&value, &runtime.order, &details).unwrap(),
                    23,
                    31
                )
                .unwrap(),
            raw
        );
        let mut malformed = n.clone();
        let Node::FixedArray { items } = &mut malformed.parts[0].fields[0].node else {
            panic!()
        };
        items.swap(0, 1);
        assert!(semantic::validate(&malformed, &c).is_err());
        let mut duplicate = n.clone();
        let Node::FixedArray { items } = &mut duplicate.parts[0].fields[0].node else {
            panic!()
        };
        items[1].node = items[0].node.clone();
        assert!(semantic::validate(&duplicate, &c).is_err());
        let mut wrong_key = n.clone();
        let Node::FixedArray { items } = &mut wrong_key.parts[0].fields[0].node else {
            panic!()
        };
        items[1].field_key = "f2".into();
        assert!(semantic::validate(&wrong_key, &c).is_err());
    }
}

#[test]
fn grouped_flags_do_not_disturb_repeated_choice_occurrences() {
    use crate::signals::wire::{readable, semantic};
    let g = seq(vec![byte(), repeat(choice(vec![byte(), byte()])), byte()]);
    let c = contract(g.clone());
    let mut binding = FieldBinding::new()
        .name(0, "f1", "hookFlagsB")
        .name(0, "f2", "samples")
        .name(0, "f3", "hookFlagsA");
    binding.set_flags(0, "f1", BTreeMap::from([("remainder".into(), 3)]));
    binding.set_flags(0, "f3", BTreeMap::from([("first".into(), 6)]));
    let mut n = naming(&c, binding);
    n.canonical_name = "GrecaptchaAccessorHookFlags".into();
    readable::group(&mut n).unwrap();
    semantic::validate(&n, &c).unwrap();
    let v = WireValue::Sequence(vec![
        WireValue::Byte(0xf8),
        WireValue::Repeat(vec![
            WireValue::Choice {
                arm: 0,
                value: Box::new(WireValue::Byte(19)),
            },
            WireValue::Choice {
                arm: 1,
                value: Box::new(WireValue::Byte(20)),
            },
        ]),
        WireValue::Byte(0x40),
    ]);
    assert_eq!(
        fields(&g, &n, &v),
        json!({"hookChecks":[{"first":true},{"remainder":true}],"samples":[19,20]})
            .as_object()
            .unwrap()
            .clone()
    );
}

#[test]
fn nested_optional_omits_only_selected_empty_fields_and_retains_empty_arrays() {
    let mut empty = repeat(byte());
    if let GrammarNode::Repeat { count, .. } = &mut empty {
        *count = Some(0);
    }
    let g = seq(vec![repeat(seq(vec![
        choice(vec![GrammarNode::empty(), tagged("130")]),
        empty,
    ]))]);
    let c = contract(g.clone());
    let n = naming(
        &c,
        FieldBinding::new()
            .name(0, "f1", "records")
            .name(0, "f1.item.f1", "optional")
            .name(0, "f1.item.f2", "empty"),
    );
    let v = WireValue::Sequence(vec![WireValue::Repeat(vec![
        WireValue::Sequence(vec![
            WireValue::Choice {
                arm: 0,
                value: Box::new(WireValue::Sequence(vec![])),
            },
            WireValue::Repeat(vec![]),
        ]),
        WireValue::Sequence(vec![
            WireValue::Choice {
                arm: 1,
                value: Box::new(WireValue::Byte(130)),
            },
            WireValue::Repeat(vec![]),
        ]),
    ])]);
    assert_eq!(
        fields(&g, &n, &v)["records"],
        json!([{"empty":[]},{"optional":130,"empty":[]}])
    );
}

#[test]
fn flatten_only_whole_single_byte_flag_messages() {
    use crate::signals::wire::readable;
    let c = contract(seq(vec![byte()]));
    let mut b = FieldBinding::new().name(0, "f1", "consoleStackFlags");
    b.set_flags(
        0,
        "f1",
        BTreeMap::from([
            ("iframeConsoleStackRead".into(), 0),
            ("pageConsoleStackRead".into(), 1),
            ("sentryOriginalLogCalled".into(), 2),
        ]),
    );
    let n = naming(&c, b);
    assert!(readable::flatten(&n.parts[0].fields));
    let mut schema = json!({"order":["consoleStackFlags"],"fields":{"consoleStackFlags":{"type":"flags","bits":{"iframeConsoleStackRead":0,"pageConsoleStackRead":1,"sentryOriginalLogCalled":2}}}});
    readable::describe(&n, &mut schema);
    assert_eq!(
        schema["fields"]["iframeConsoleStackRead"]["type"],
        "boolean"
    );
    assert!(schema["fields"].get("consoleStackFlags").is_none());
    let mut mixed = n.parts[0].fields.clone();
    mixed.push(mixed[0].clone());
    assert!(!readable::flatten(&mixed));
    let mut repeated = n.parts[0].fields.clone();
    repeated[0].node = Node::Array {
        control: vec![],
        item: Box::new(repeated[0].node.clone()),
    };
    assert!(!readable::flatten(&repeated));
}

#[test]
fn empty_packed_stream_is_a_present_object_and_empty_array_is_present() {
    let mut empty = repeat(byte());
    if let GrammarNode::Repeat { count, .. } = &mut empty {
        *count = Some(0);
    }
    let g = seq(vec![empty.clone(), empty, byte()]);
    let c = contract(g.clone());
    let mut b = FieldBinding::new()
        .name(0, "f1", "prefersReducedMotionBits")
        .name(0, "f2", "samples")
        .name(0, "f3", "checks");
    b.set_flags(0, "f1", BTreeMap::new());
    b.set_flags(0, "f3", BTreeMap::from([("supported".into(), 0)]));
    let n = naming(&c, b);
    let v = WireValue::Sequence(vec![
        WireValue::Repeat(vec![]),
        WireValue::Repeat(vec![]),
        WireValue::Byte(0),
    ]);
    assert_eq!(
        fields(&g, &n, &v),
        json!({"prefersReducedMotionBits":{},"samples":[],"checks":{"supported":false}})
            .as_object()
            .unwrap()
            .clone()
    );
}

#[test]
fn user_agent_optional_placeholders_differ_from_written_null_markers() {
    let g = seq(vec![
        choice(vec![GrammarNode::empty(), tagged("203")]),
        choice(vec![GrammarNode::empty(), tagged("79")]),
        byte(),
    ]);
    let c = contract(g.clone());
    let mut b = FieldBinding::new()
        .name(0, "f1", "wow64Placeholder")
        .name(0, "f2", "mobilePlaceholder")
        .name(0, "f3", "flags");
    b.set_flags(
        0,
        "f3",
        BTreeMap::from([("wow64".into(), 0), ("mobile".into(), 1)]),
    );
    let mut n = naming(&c, b);
    n.canonical_name = "UserAgentData".into();
    crate::signals::wire::semantic::bind(&mut n, &c);
    crate::signals::wire::semantic::validate(&n, &c).unwrap();
    for arm in [0, 1] {
        let value = WireValue::Sequence(vec![
            WireValue::Choice {
                arm,
                value: Box::new(if arm == 0 {
                    WireValue::Sequence(vec![])
                } else {
                    WireValue::Byte(203)
                }),
            },
            WireValue::Choice {
                arm,
                value: Box::new(if arm == 0 {
                    WireValue::Sequence(vec![])
                } else {
                    WireValue::Byte(79)
                }),
            },
            WireValue::Byte(3),
        ]);
        let expected = if arm == 0 {
            json!({"flags":{"wow64":true,"mobile":true}})
        } else {
            json!({"wow64Placeholder":null,"mobilePlaceholder":null,"flags":{"wow64":true,"mobile":true}})
        };
        assert_eq!(Value::Object(fields(&g, &n, &value)), expected);
        let mut runtime = Runtime::default();
        runtime.walk(&g, &value, vec![], 0, &[]).unwrap();
        let mut details = vec![];
        runtime
            .fields(&n.parts[0].fields, &[], &mut details)
            .unwrap();
        assert_eq!(
            rebuild_tags(&value, &runtime.order, &details).unwrap(),
            value
        );
        assert_eq!(
            details
                .iter()
                .filter(|d| d.get("absent") == Some(&json!(true)))
                .count(),
            if arm == 0 { 2 } else { 0 }
        );
        let choices: Vec<_> = details
            .iter()
            .filter(|d| d.get("choice").is_some())
            .collect();
        assert_eq!(choices.len(), 2);
        assert!(choices
            .iter()
            .all(|d| d["arm"] == json!(arm) && d["occurrences"] == json!([])));
    }
}

#[test]
fn scalar_null_pair_rebuilds_both_tags_and_preserves_scalar_value_arms() {
    let GrammarNode::Byte(w) = byte() else {
        unreachable!()
    };
    let g = seq(vec![choice(vec![
        seq(vec![tagged("203"), tagged("17")]),
        GrammarNode::Number(w.clone()),
        GrammarNode::String(w),
    ])]);
    let c = contract(g.clone());
    let mut n = naming(
        &c,
        FieldBinding::new()
            .name(0, "f1", "innerWidth")
            .name(0, "f1.arm1.f1", "nullMarker")
            .name(0, "f1.arm1.f2", "nullFlag"),
    );
    n.canonical_name = "NavigatorProperties".into();
    crate::signals::wire::semantic::bind(&mut n, &c);
    let n: Naming = strict_json(&serde_json::to_vec(&n).unwrap()).unwrap();
    crate::signals::wire::semantic::validate(&n, &c).unwrap();
    let null = WireValue::Sequence(vec![WireValue::Choice {
        arm: 0,
        value: Box::new(WireValue::Sequence(vec![
            WireValue::Byte(203),
            WireValue::Byte(17),
        ])),
    }]);
    assert_eq!(fields(&g, &n, &null)["innerWidth"], Value::Null);
    for (arm, value, expected) in [
        (1, WireValue::NumberBits(17f64.to_bits()), json!(17)),
        (2, WireValue::StringUnits(vec![111, 107]), json!("ok")),
    ] {
        assert_eq!(
            fields(
                &g,
                &n,
                &WireValue::Sequence(vec![WireValue::Choice {
                    arm,
                    value: Box::new(value)
                }])
            )["innerWidth"],
            expected
        );
    }
    let mut runtime = Runtime::default();
    runtime.walk(&g, &null, vec![], 0, &[]).unwrap();
    let mut details = vec![];
    runtime
        .fields(&n.parts[0].fields, &[], &mut details)
        .unwrap();
    assert_eq!(
        details
            .iter()
            .filter_map(|d| d.get("tagByte"))
            .cloned()
            .collect::<Vec<_>>(),
        vec![json!(203), json!(17)]
    );
    let rebuilt = rebuild_tags(&null, &runtime.order, &details).unwrap();
    assert_eq!(rebuilt, null);
    let program = Program {
        prefix_byte_order: "be32".into(),
        seed_source: "captured_prefix".into(),
        stream_scope: "emission_shared_cursor".into(),
        string_scope: "emission_local_writer_instance".into(),
        salt: 17,
        body: Op::Sequence {
            items: vec![
                Op::Byte {
                    mask: WireMask::None,
                    xor: 91,
                    allowed: None,
                },
                Op::Byte {
                    mask: WireMask::None,
                    xor: 37,
                    allowed: None,
                },
            ],
        },
    };
    // Encode the retained null subtree through the ordinary physical writer.
    let WireValue::Sequence(v) = &rebuilt else {
        panic!()
    };
    let WireValue::Choice { value, .. } = &v[0] else {
        panic!()
    };
    let bytes = program.encode(value, 23, 31).unwrap();
    assert_eq!(&bytes[4..], &[203 ^ 91, 17 ^ 37]);
    let mut forged = n.clone();
    forged.canonical_name = "DeviceMemory".into();
    assert!(crate::signals::wire::semantic::validate(&forged, &c).is_err());
}

#[test]
fn whole_readable_numbers_use_exact_integer_tokens_without_saturating_boundaries() {
    let n = Node::Leaf {
        leaf: 0,
        grammar_path: vec![],
        kind: "number".into(),
        mask: json!({"mode":"none"}),
        static_xor: None,
        byte_mapping: None,
    };
    for (number, expected) in [
        (0.0, "0"),
        (1.0, "1"),
        (32.0, "32"),
        (0.04153433085533542, "0.04153433085533542"),
        (9007199254740994.0, "9007199254740994"),
        (i64::MIN as f64, "-9223372036854775808"),
        (i64::MAX as f64, "9223372036854775808"),
        (
            f64::from_bits((u64::MAX as f64).to_bits() - 1),
            "18446744073709549568",
        ),
    ] {
        let physical = WireValue::NumberBits(number.to_bits());
        let mut runtime = Runtime::default();
        runtime.leaves.insert((0, vec![]), &physical);
        let mut details = vec![];
        let value = runtime.node(&n, &[], &mut details).unwrap();
        assert_eq!(serde_json::to_string(&value).unwrap(), expected);
        assert_eq!(details[0]["numberBits"], json!(number.to_bits()));
        assert_eq!(value.as_f64().unwrap().to_bits(), number.to_bits());
    }
    for number in [
        u64::MAX as f64,
        f64::from_bits((i64::MIN as f64).to_bits() + 1),
        1e100,
    ] {
        let value = finite_number(number);
        assert!(value.as_i64().is_none() && value.as_u64().is_none());
        assert_eq!(value.as_f64().unwrap().to_bits(), number.to_bits());
    }
}

fn counted_fixture(nested: bool, siblings: bool, strings: bool) -> (GrammarNode, Naming, Program) {
    let GrammarNode::Byte(write) = tagged("input") else {
        unreachable!()
    };
    let mut array = repeat(if strings {
        GrammarNode::String(write.clone())
    } else {
        GrammarNode::Number(write.clone())
    });
    if let GrammarNode::Repeat { count, .. } = &mut array {
        *count = None;
    }
    let mut items = vec![GrammarNode::Number(write), array];
    if siblings {
        items.push(tagged("7"));
    }
    let prefix = GrammarNode::RawPrefix(PrefixNode {
        span: Span::default(),
        spelling: PrefixSpelling::ArrayLiteral,
        seed_word: "seed".into(),
        elements: vec![],
        seed_binding: None,
    });
    let g = if nested {
        seq(vec![
            prefix,
            choice(vec![seq(items), tagged("141"), seq(vec![])]),
        ])
    } else {
        let mut all = vec![prefix];
        all.extend(items);
        seq(all)
    };
    let c = contract(g.clone());
    let binding = if nested {
        let mut b = FieldBinding::new()
            .name(0, "f1", "container")
            .name(0, "f1.arm1.f1", "encodedSize")
            .name(0, "f1.arm1.f2", "entries");
        if siblings {
            b = b.name(0, "f1.arm1.f3", "statistic");
        }
        b
    } else {
        let mut b = FieldBinding::new()
            .name(0, "f1", "encodedSize")
            .name(0, "f2", "entries");
        if siblings {
            b = b.name(0, "f3", "statistic");
        }
        b
    };
    let names = naming(&c, binding);
    let path = if nested { json!([1, 0, 0]) } else { json!([1]) };
    let repeat_path = if nested {
        json!([{"child":1},{"arm":0},{"child":1}])
    } else {
        json!([{"child":2}])
    };
    let reference: CountRef = serde_json::from_value(json!({"path":path,"repeat_path":repeat_path,
        "write":{"start":1,"end":2},"collection_binding":{"start":2,"end":3},"counter_binding":{"start":3,"end":4},"update":{"start":4,"end":5},"update_in_body":true,"predecrement_exit":null,"length_evaluation":{"start":5,"end":6},"same_cached_evaluation":true,"loop_span":{"start":6,"end":7},"direction":"ascending","maximum":65536,"reject_negative_zero":true})).unwrap();
    let number = || Op::Number {
        envelope: None,
        mask: WireMask::Inherit,
        literal: None,
    };
    let mut ops = vec![
        number(),
        Op::CountedRepeat {
            count: CountExpr::PriorNumber { reference },
            direction: LoopDirection::Ascending,
            body: Box::new(if strings {
                Op::String {
                    mask: WireMask::None,
                    literal: None,
                }
            } else {
                number()
            }),
        },
    ];
    if siblings {
        ops.push(Op::Byte {
            mask: WireMask::None,
            xor: 23,
            allowed: None,
        });
    }
    let body = if nested {
        Op::Sequence {
            items: vec![Op::Choice {
                discriminants: vec![],
                mask: WireMask::Inherit,
                xor: 0,
                arms: vec![
                    Op::Sequence { items: ops },
                    Op::Byte {
                        mask: WireMask::Inherit,
                        xor: 0,
                        allowed: None,
                    },
                    Op::Sequence { items: vec![] },
                ],
            }],
        }
    } else {
        Op::Sequence { items: ops }
    };
    (
        g,
        names,
        Program {
            prefix_byte_order: "be32".into(),
            seed_source: "captured_prefix".into(),
            stream_scope: "emission_shared_cursor".into(),
            string_scope: "emission_local_writer_instance".into(),
            salt: 17,
            body,
        },
    )
}
fn counted_projection(
    g: &GrammarNode,
    n: &Naming,
    p: &Program,
    v: &WireValue,
) -> Result<(Value, Vec<Value>, WireValue), WireError> {
    let mut runtime = Runtime::default();
    runtime.counted = super::super::counted::Rules::new(p, &n.parts[0].fields);
    runtime.walk(g, v, vec![], 0, &[])?;
    let mut details = vec![];
    let fields = runtime.fields(&n.parts[0].fields, &[], &mut details)?;
    let rebuilt = rebuild_tags(v, &runtime.order, &details)?;
    Ok((Value::Object(fields), details, rebuilt))
}
#[test]
fn counted_arrays_reconstruct_original_number_writes_and_keep_semantic_siblings() {
    for nested in [false, true] {
        for siblings in [false, true] {
            for strings in [false, true] {
                for count in [0, 3] {
                    let (g, n, p) = counted_fixture(nested, siblings, strings);
                    let elements: Vec<_> = (0..count)
                        .map(|i| {
                            if strings {
                                WireValue::StringUnits(
                                    format!("shader{i}").encode_utf16().collect(),
                                )
                            } else {
                                WireValue::NumberBits((i as f64 + 0.25).to_bits())
                            }
                        })
                        .collect();
                    let logical: Vec<_> = (0..count)
                        .map(|i| {
                            if strings {
                                json!(format!("shader{i}"))
                            } else {
                                json!(i as f64 + 0.25)
                            }
                        })
                        .collect();
                    let mut physical = vec![
                        WireValue::NumberBits((count as f64).to_bits()),
                        WireValue::Repeat(elements),
                    ];
                    if siblings {
                        physical.push(WireValue::Byte(7));
                    }
                    let value = if nested {
                        WireValue::Sequence(vec![WireValue::Choice {
                            arm: 0,
                            value: Box::new(WireValue::Sequence(physical)),
                        }])
                    } else {
                        WireValue::Sequence(physical)
                    };
                    let (actual, details, rebuilt) =
                        counted_projection(&g, &n, &p, &value).unwrap();
                    let mut expected = json!({"entries":logical});
                    if siblings {
                        expected["statistic"] = json!(7);
                    }
                    if nested {
                        expected =
                            json!({"container":if siblings {expected} else {json!(logical)}});
                    }
                    assert_eq!(actual, expected);
                    assert_eq!(rebuilt, value);
                    assert!(details.iter().any(|d| d["arrayLength"] == count));
                    let bytes = p
                        .encode_with_float(&rebuilt, 91, 31, &Ok(FloatByteOrder::Little))
                        .unwrap();
                    assert_eq!(
                        bytes,
                        p.encode_with_float(&value, 91, 31, &Ok(FloatByteOrder::Little))
                            .unwrap()
                    );
                    // Destroy the old count: reconstruction must replace it from the logical length.
                    let mut destroyed = value.clone();
                    fn poison(v: &mut WireValue) {
                        match v {
                            WireValue::Sequence(v) => poison(&mut v[0]),
                            WireValue::Choice { value, .. } => poison(value),
                            WireValue::NumberBits(b) => *b = f64::NAN.to_bits(),
                            _ => panic!(),
                        }
                    }
                    poison(&mut destroyed);
                    let mut runtime = Runtime::default();
                    runtime.walk(&g, &value, vec![], 0, &[]).unwrap();
                    assert_eq!(
                        rebuild_tags(&destroyed, &runtime.order, &details).unwrap(),
                        value
                    );
                }
            }
        }
    }
}
#[test]
fn counted_arrays_reject_noninvertible_lengths_and_retain_uncertified_shared_counts() {
    let (g, n, p) = counted_fixture(false, false, false);
    for count in [-1., -0., 0.5, 65537., f64::INFINITY, f64::NAN, 2.] {
        let v = WireValue::Sequence(vec![
            WireValue::NumberBits(count.to_bits()),
            WireValue::Repeat(vec![]),
        ]);
        assert!(counted_projection(&g, &n, &p, &v).is_err(), "{count}");
    }
    let mut unproved = p.clone();
    let Op::Sequence { items } = &mut unproved.body else {
        panic!()
    };
    let Op::CountedRepeat {
        body, direction, ..
    } = items[1].clone()
    else {
        panic!()
    };
    items[1] = Op::Repeat {
        count: 0,
        body,
        direction,
    };
    assert!(
        super::super::counted::Rules::new(&unproved, &n.parts[0].fields)
            .0
            .is_empty()
    );
    let mut shared = p.clone();
    let Op::Sequence { items } = &mut shared.body else {
        panic!()
    };
    items.push(items[1].clone());
    assert!(
        super::super::counted::Rules::new(&shared, &n.parts[0].fields)
            .0
            .is_empty()
    );
    let mut wrong = p.clone();
    let Op::Sequence { items } = &mut wrong.body else {
        panic!()
    };
    let Op::CountedRepeat {
        count: CountExpr::PriorNumber { reference },
        ..
    } = &mut items[1]
    else {
        panic!()
    };
    reference.path = vec![9];
    assert!(
        super::super::counted::Rules::new(&wrong, &n.parts[0].fields)
            .0
            .is_empty()
    );
}
#[test]
fn counted_arrays_preserve_explicit_null_optional_absence_and_real_empty_records() {
    let (g, mut n, p) = counted_fixture(true, false, true);
    let Node::Choice { arms, .. } = &mut n.parts[0].fields[0].node else {
        panic!()
    };
    let Node::Leaf { byte_mapping, .. } = &mut arms[1] else {
        panic!()
    };
    *byte_mapping = Some(serde_json::from_value(json!({"cases":[{"byte":141,"value":null}],"condition":"documented null marker","source":{"start":1,"end":2}})).unwrap());
    for (arm, value, expected) in [
        (1, WireValue::Byte(141), json!({"container":null})),
        (2, WireValue::Sequence(vec![]), json!({})),
    ] {
        let v = WireValue::Sequence(vec![WireValue::Choice {
            arm,
            value: Box::new(value),
        }]);
        assert_eq!(counted_projection(&g, &n, &p, &v).unwrap().0, expected);
    }
    // Empty records are data. Empty/no-write anonymous items cannot certify length.
    let mut runtime = Runtime::default();
    runtime.controls.insert((vec![], vec![]), 1);
    assert_eq!(
        runtime
            .node(
                &Node::Array {
                    control: vec![],
                    item: Box::new(Node::Record { fields: vec![] })
                },
                &[],
                &mut vec![]
            )
            .unwrap(),
        json!([{}])
    );
}

fn approved_count_example(
    root: &str,
    count_name: &str,
    array_name: &str,
    size: usize,
    nested: bool,
    strings: bool,
    sibling: Option<&str>,
) {
    let (g, mut n, p) = counted_fixture(nested, sibling.is_some(), strings);
    let fields = if nested {
        n.parts[0].fields[0].name = root.into();
        let Node::Choice { arms, .. } = &mut n.parts[0].fields[0].node else {
            panic!()
        };
        let Node::Record { fields } = &mut arms[0] else {
            panic!()
        };
        fields
    } else {
        &mut n.parts[0].fields
    };
    fields[0].name = count_name.into();
    fields[1].name = array_name.into();
    if let Some(s) = sibling {
        fields[2].name = s.into();
    }
    let items: Vec<_> = (0..size)
        .map(|i| {
            if strings {
                WireValue::StringUnits(format!("value{i}").encode_utf16().collect())
            } else {
                WireValue::NumberBits((i as f64).to_bits())
            }
        })
        .collect();
    let expected: Vec<_> = (0..size)
        .map(|i| {
            if strings {
                json!(format!("value{i}"))
            } else {
                json!(i)
            }
        })
        .collect();
    let mut values = vec![
        WireValue::NumberBits((size as f64).to_bits()),
        WireValue::Repeat(items),
    ];
    if sibling.is_some() {
        values.push(WireValue::Byte(1));
    }
    let value = if nested {
        WireValue::Sequence(vec![WireValue::Choice {
            arm: 0,
            value: Box::new(WireValue::Sequence(values)),
        }])
    } else {
        WireValue::Sequence(values)
    };
    let (actual, _, rebuilt) = counted_projection(&g, &n, &p, &value).unwrap();
    let mut logical = if nested {
        json!({root:expected})
    } else {
        json!({array_name:expected})
    };
    if let Some(s) = sibling {
        logical[s] = json!(1);
    }
    assert_eq!(actual, logical);
    assert_eq!(
        p.encode_with_float(&rebuilt, 7, 19, &Ok(FloatByteOrder::Little))
            .unwrap(),
        p.encode_with_float(&value, 7, 19, &Ok(FloatByteOrder::Little))
            .unwrap()
    );
}
#[test]
fn approved_full_version_list_is_a_direct_array() {
    approved_count_example(
        "fullVersionList",
        "fullVersionList",
        "fullVersionEntries",
        3,
        true,
        false,
        None,
    )
}
#[test]
fn approved_brands_is_a_direct_array() {
    approved_count_example("brands", "brands", "brandEntries", 3, true, false, None)
}
#[test]
fn approved_shader_precisions_keep_string_items() {
    approved_count_example(
        "shaderPrecisions",
        "shaderPrecisions",
        "shaderPrecision",
        12,
        true,
        true,
        None,
    )
}
#[test]
fn approved_supported_extensions_keep_string_items() {
    approved_count_example(
        "supportedExtensions",
        "supportedExtensions",
        "extension",
        4,
        true,
        true,
        None,
    )
}
#[test]
fn approved_empty_ice_candidates_keep_the_root_object() {
    approved_count_example(
        "WebRtcIceCandidateAddresses",
        "recordCount",
        "candidates",
        0,
        false,
        false,
        None,
    )
}
#[test]
fn approved_empty_touch_records_keep_the_root_object() {
    approved_count_example(
        "UiEventSamplesTouch",
        "recordCount",
        "records",
        0,
        false,
        false,
        None,
    )
}
#[test]
fn approved_animation_frame_timings_keep_time_quantum() {
    approved_count_example(
        "AnimationFrameTimings",
        "sampleCount",
        "frameTime",
        20,
        false,
        false,
        Some("timeQuantum"),
    )
}
