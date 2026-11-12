//! Actual analyzer JSON and public session-bound loader; source is never executed.
use shape::signals::{
    analysis::{analyze_program_signals, SignalFinderOptions},
    payload::{extract::Session, pin, Report},
    wire::{Decoder, InstanceReadiness, Outcome, SchemaDocument, Support, WireError},
};

fn source() -> String {
    let template = include_str!("fixtures/signal-wire-roundtrip/program.js");
    let prefix = &template[..template.find("function Session(token)").unwrap()];
    let source = format!(
        "{prefix}{}",
        r#"
function collect(emitters, context, onInit) {
  let document={hasStorageAccess:true};
  onInit(function callback(){
    let seed=Math.random()*4294967296|0;
    let mask=streamFactory(seed^17);
    let b=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];
    b[4]=(("hasStorageAccess" in document)|("requestStorageAccess" in document)<<1)^mask();
    emitters[0].next([[],b]);
  });
}
let registry=[{ctor:null,priority:false}];
let waves=[collect], rows=[[0]], flags=[false];
function coordinate(emitters, factory, context, first, second, flag) {
  for(let i=0;i<emitters.length;++i){let descriptor=emitters[i];}
  let stored;
  factory(emitters,context,function(cb){stored=cb;},function(){},function(){},function(){});
  stored();
  return function unsubscribe(){};
}
function connect(){
  waves.forEach(function(factory,i){
    let descriptors=rows[i].map(index=>registry[index]);
    coordinate(descriptors,factory,{},function(){},function(){},flags[i]);
  });
  registry.forEach(entry=>{let task=entry.ctor;if(task)task();});
}
let filter=[0];
function route(options){let f=options.signalFilter?options.signalFilter:filter;return registry[f[0]];}
let indices=[],table=[];indices.map(i=>table[i]);
function Session(token){
  let context={global:globalThis,transactionToken:void 0};
  context.transactionToken=token;
  context.c="";context.d="";context.z="";
  function initialize(){tokenState=tokenHash(context.transactionToken);connect();}
  send("x-example-"+"a","payload",1000);
  return {init:initialize,teardown:function(){}};
}
function publicEntry(token){let created=new Session(token);created.init();}
return {init:publicEntry};
}
main();
"#
    );
    source
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
    Session {
        transaction_token: "ab".into(),
        alphabet: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".into(),
        shuffle_key: vec![],
        keys: [0; 8],
        nonce: None,
    }
}
const INIT: &str = "event.detail.init('ab');";

fn ordinary_source() -> String {
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

#[test]
fn public_loaders_require_the_token_property_value_at_the_hash_read() {
    shape::run_with_large_stack(|| {
        // The source writes flags byte 1 using hash("cd") after the mutation.
        // Reencoding with a decoder's own incorrect state is not semantic proof.
        let wrong_token_blob = shape::signals::codec::encode_signal(
            shape::signals::codec::hash_string_token("cd"),
            0x01020304,
            17,
            |w| w.byte(1),
        );
        assert_ne!(
            wrong_token_blob,
            report().positional_signals[0].as_ref().unwrap().1
        );
        let changes = [
            ("computed store", "context['transactionToken']='cd';"),
            ("static store", "context.transactionToken='cd';"),
            (
                "alias store",
                "let alias=context;alias['transactionToken']='cd';",
            ),
            (
                "alias chain",
                "let alias=context;let other=alias;other.transactionToken='cd';",
            ),
            (
                "mutable alias",
                "let alias=context;alias.transactionToken='cd';alias={};",
            ),
            ("delete", "delete context.transactionToken;"),
            ("computed delete", "delete context['transactionToken'];"),
            ("update", "++context.transactionToken;"),
            ("computed update", "context['transactionToken']++;"),
            ("dynamic store", "let key=chooseKey();context[key]='cd';"),
            ("dynamic delete", "let key=chooseKey();delete context[key];"),
            (
                "helper mutator",
                "function mutate(value){value.transactionToken='cd';}mutate(context);",
            ),
            (
                "captured mutator",
                "function mutate(){context.transactionToken='cd';}mutate();",
            ),
            ("external mutator", "mutate(context);"),
            (
                "descriptor mutator",
                "Object.defineProperty(context,'transactionToken',{value:'cd'});",
            ),
            (
                "method mutator",
                "context.m=function(){this.transactionToken='cd';};context.m();",
            ),
            (
                "parenthesized method",
                "context.m=function(){this.transactionToken='cd';};(context.m)();",
            ),
            (
                "tagged method",
                "context.m=function(){this.transactionToken='cd';};context.m`value`;",
            ),
            (
                "formal overwrite",
                "token='cd';context.transactionToken=token;",
            ),
            ("destructured store", "[context.transactionToken]=['cd'];"),
            (
                "iteration store",
                "for(context.transactionToken of ['cd']){}",
            ),
        ];
        for (initializer, base) in [(true, source()), (false, ordinary_source())] {
            let mut cases: Vec<_> = changes
                .into_iter()
                .map(|(name, mutation)| {
                    (
                        name,
                        base.replace(
                            "context.transactionToken=token;",
                            &format!("context.transactionToken=token;{mutation}"),
                        ),
                    )
                })
                .collect();
            cases.push((
                "constructor formal",
                base.replace(
                    "let context={global:globalThis,transactionToken:void 0};",
                    "token='cd';let context={global:globalThis,transactionToken:void 0};",
                ),
            ));
            cases.push((
                "public formal",
                if initializer {
                    base.replace(
                        "function publicEntry(token){",
                        "function publicEntry(token){token='cd';",
                    )
                } else {
                    base.replace("init:function(token){", "init:function(token){token='cd';")
                },
            ));
            cases.push((
                "arguments alias",
                base.replace(
                    "let context={global:globalThis,transactionToken:void 0};",
                    "arguments[0]='cd';let context={global:globalThis,transactionToken:void 0};",
                ),
            ));
            cases.push(("arrow arguments alias", base.replace("let context={global:globalThis,transactionToken:void 0};",
                "(()=>{arguments[0]='cd';})();let context={global:globalThis,transactionToken:void 0};")));
            for (name, changed) in cases {
                let pin = pin(&changed, &session()).unwrap().layout;
                let analysis =
                    analyze_program_signals(&changed, &SignalFinderOptions::default()).unwrap();
                let saved = serde_json::to_vec(&analysis.report).unwrap();
                let decoder = if initializer {
                    Decoder::load_with_initializer(&saved, &changed, &pin, INIT)
                } else {
                    Decoder::load(&saved, &changed, &pin)
                }
                .unwrap();
                for blob in [
                    report().positional_signals[0].as_ref().unwrap().1.clone(),
                    wrong_token_blob.clone(),
                ] {
                    let mut actual = Report {
                        positional_signals: vec![Some((vec![], blob))],
                        ..report()
                    };
                    if !initializer {
                        actual.positional_signals.push(None);
                    }
                    assert!(
                        matches!(
                            decoder.decode_report(&actual).unwrap()[0].outcome,
                            Outcome::Unsupported { .. }
                        ),
                        "initializer={initializer}, mutation={name}"
                    );
                    assert_ne!(
                        decoder.decode_named_report(&actual).unwrap().signals
                            ["StorageAccessApiPresence"]
                            .status,
                        shape::signals::wire::NamedStatus::Decoded,
                        "initializer={initializer}, mutation={name}"
                    );
                }
            }
        }
    });
}

#[test]
fn public_token_value_proof_preserves_irrelevant_writes_and_aliases() {
    shape::run_with_large_stack(|| {
        for (initializer, base) in [(true, source()), (false, ordinary_source())] {
            for (name, changed) in [
                ("baseline", base.clone()),
                ("unrelated property", base.replace("context.transactionToken=token;", "context.transactionToken=token;context.unrelated='cd';")),
                ("computed public value", base.replace("context.transactionToken=token;", "context['transactionToken']=token;")),
                ("read alias", base.replace("context.transactionToken=token;", "context.transactionToken=token;let alias=context;")
                    .replace("tokenHash(context.transactionToken)", "tokenHash(alias.transactionToken)")),
                ("after final read", base.replace("tokenState=tokenHash(context.transactionToken);", "tokenState=tokenHash(context.transactionToken);context['transactionToken']='cd';")),
                ("helper after final read", base.replace("tokenState=tokenHash(context.transactionToken);", "function late(){context['transactionToken']='cd';}tokenState=tokenHash(context.transactionToken);late();")),
            ] {
                let pin = pin(&changed, &session()).unwrap().layout;
                let analysis = analyze_program_signals(&changed, &SignalFinderOptions::default()).unwrap();
                let saved = serde_json::to_vec(&analysis.report).unwrap();
                let decoder = if initializer {
                    Decoder::load_with_initializer(&saved, &changed, &pin, INIT)
                } else { Decoder::load(&saved, &changed, &pin) }.unwrap();
                let mut actual = report();
                if !initializer { actual.positional_signals.push(None); }
                let decoded = decoder.decode_report(&actual).unwrap();
                let Outcome::Decoded { value, exact_reencode: true } = &decoded[0].outcome else {
                    panic!("initializer={initializer}, case={name}: {:?}", decoded[0].outcome);
                };
                assert_eq!(*value, shape::signals::wire::WireValue::Sequence(vec![shape::signals::wire::WireValue::Byte(1)]));
                assert_eq!(decoder.encode_slot(0, value, 0x01020304).unwrap(), [1,2,3,4,0xa3]);
            }
        }
    });
}

#[test]
fn saved_readiness_regeneration_rechecks_the_hash_input_value() {
    shape::run_with_large_stack(|| {
        let baseline = source();
        let pin = pin(&baseline, &session()).unwrap().layout;
        let analysis = analyze_program_signals(&baseline, &SignalFinderOptions::default()).unwrap();
        let mut saved =
            SchemaDocument::from_json(&serde_json::to_vec(&analysis.report).unwrap()).unwrap();
        saved.bind_initializer(&baseline, &pin, INIT).unwrap();
        let InstanceReadiness::Ready { witness } = &saved
            .wire
            .as_ref()
            .unwrap()
            .initialization
            .as_ref()
            .unwrap()
            .emissions[0]
            .outcome
        else {
            panic!("baseline witness must be ready");
        };
        let witness = serde_json::from_slice(&serde_json::to_vec(witness).unwrap()).unwrap();
        let changed = baseline.replace(
            "context.transactionToken=token;",
            "context.transactionToken=token;context['transactionToken']='cd';",
        );
        let analysis = analyze_program_signals(&changed, &SignalFinderOptions::default()).unwrap();
        let encoding =
            shape::signals::extract::encoding::extract_encoding_properties(&changed).unwrap();
        let wiring = shape::signals::extract::wiring::extract_program_wiring(&changed).unwrap();
        let context =
            shape::signals::wire::InitializationContext::from_initializer(INIT, "ab").unwrap();
        let error = analysis
            .report
            .wire
            .algorithms
            .verify_stream_readiness(
                &witness,
                &changed,
                &encoding,
                &wiring,
                &analysis.report.wire.slots[0].contract.emissions[0],
                &context,
            )
            .unwrap_err();
        // Failure occurs while regenerating the value proof, before comparing
        // the serialized witness's old source identity or spans.
        assert_eq!(error.0, "hash token property value is overwritten");
    });
}
fn report() -> Report {
    Report {
        errors: vec![],
        positional_signals: vec![Some((vec![], vec![1, 2, 3, 4, 0xa3]))],
        signals: vec![],
        keyed_signals: vec![],
        seeds: [19, 23],
        keysets: vec![],
    }
}
#[test]
fn public_callback_loader_uses_only_revalidated_instance_witnesses() {
    shape::run_with_large_stack(|| {
        let source = source();
        let pin = pin(&source, &session()).unwrap().layout;
        let analysis = analyze_program_signals(&source, &SignalFinderOptions::default()).unwrap();
        let saved = serde_json::to_vec(&analysis.report).unwrap();
        let mut document = SchemaDocument::from_json(&saved).unwrap();
        assert!(document
            .wire
            .as_ref()
            .unwrap()
            .algorithms
            .stream
            .codec
            .is_none());
        assert!(
            matches!(
                document.wire.as_ref().unwrap().slots[0].programs[0],
                Support::RequiresInitializer { .. }
            ),
            "{:?}; {:?}",
            document.wire.as_ref().unwrap().slots[0].programs,
            document.wire.as_ref().unwrap().algorithms
        );
        let uninitialized = Decoder::load(&saved, &source, &pin).unwrap();
        assert!(matches!(
            uninitialized.decode_report(&report()).unwrap()[0].outcome,
            Outcome::Unsupported { .. }
        ));
        document.bind_initializer(&source, &pin, INIT).unwrap();
        let census = document
            .wire
            .as_ref()
            .unwrap()
            .initialization
            .as_ref()
            .unwrap();
        assert_eq!(census.emissions.len(), 1);
        assert!(
            matches!(census.emissions[0].outcome, InstanceReadiness::Ready { .. }),
            "{census:?}"
        );
        let saved = serde_json::to_vec(&document).unwrap();
        let decoder = Decoder::load_with_initializer(&saved, &source, &pin, INIT).unwrap();
        let physical = decoder.decode_report(&report()).unwrap();
        let Outcome::Decoded {
            value,
            exact_reencode: true,
        } = &physical[0].outcome
        else {
            panic!("{:?}", physical[0].outcome)
        };
        assert_eq!(
            decoder.encode_slot(0, value, 0x01020304).unwrap(),
            [1, 2, 3, 4, 0xa3]
        );
        let named = decoder.decode_named_report(&report()).unwrap();
        assert_eq!(named.signals.len(), 1);
        let signal = &named.signals["StorageAccessApiPresence"];
        assert_eq!(signal.status, shape::signals::wire::NamedStatus::Decoded);
        assert_eq!(
            signal.fields.as_ref().unwrap()["apiFlags"],
            serde_json::json!({
                "hasStorageAccess":true, "requestStorageAccess":false
            })
        );
        assert!(named.metadata.initialization.is_some());
        assert_eq!(
            named
                .metadata
                .initialization
                .as_ref()
                .unwrap()
                .initializer_sha256
                .len(),
            64
        );
        assert_eq!(named.restore_original_report(), report());
        assert!(Decoder::load(&saved, &source, &pin).is_err());
        assert!(matches!(
            Decoder::load_with_initializer(&saved, &source, &pin, "event.detail.init('other');"),
            Err(WireError::Pin)
        ));
        if let InstanceReadiness::Ready { witness } = &mut document
            .wire
            .as_mut()
            .unwrap()
            .initialization
            .as_mut()
            .unwrap()
            .emissions[0]
            .outcome
        {
            witness.initializer_sha256 = "0".repeat(64);
        }
        assert!(matches!(
            Decoder::load_with_initializer(
                &serde_json::to_vec(&document).unwrap(),
                &source,
                &pin,
                INIT
            ),
            Err(WireError::Evidence)
        ));
        for replacement in [
            "tokenState=tokenHash(context.transactionToken);tokenState^=1;",
            "tokenState=tokenHash(context.transactionToken);++tokenState;",
        ] {
            let changed = source.replace(
                "tokenState=tokenHash(context.transactionToken);",
                replacement,
            );
            let changed_pin = shape::signals::payload::pin(&changed, &session())
                .unwrap()
                .layout;
            let analysis =
                analyze_program_signals(&changed, &SignalFinderOptions::default()).unwrap();
            let bytes = serde_json::to_vec(&analysis.report).unwrap();
            let decoder =
                Decoder::load_with_initializer(&bytes, &changed, &changed_pin, INIT).unwrap();
            assert!(matches!(
                decoder.decode_report(&report()).unwrap()[0].outcome,
                Outcome::Unsupported { .. }
            ));
        }
    });
}

#[test]
fn public_root_cases_have_separate_session_witnesses_and_source_branch_scope() {
    shape::run_with_large_stack(|| {
        let mut source = source();
        let start = source
            .find("    let seed=Math.random()*4294967296|0;")
            .unwrap();
        let end = source.find("    emitters[0].next([[],b]);").unwrap()
            + "    emitters[0].next([[],b]);".len();
        source.replace_range(start..end,r#"
    let seed=Math.random()*4294967296|0;let b,mask;
    if(flag){
      b=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];mask=streamFactory(seed^17);b[4]=5^mask();
    }else{
      b=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];mask=streamFactory(seed^19);b[4]=7^mask();b[5]=8^mask();
    }
    emitters[0].next([[],b]);
"#);
        let pin = pin(&source, &session()).unwrap().layout;
        let analysis = analyze_program_signals(&source, &SignalFinderOptions::default()).unwrap();
        let mut document =
            SchemaDocument::from_json(&serde_json::to_vec(&analysis.report).unwrap()).unwrap();
        document.bind_initializer(&source, &pin, INIT).unwrap();
        let census = document
            .wire
            .as_ref()
            .unwrap()
            .initialization
            .as_ref()
            .unwrap();
        assert_eq!(census.emissions.len(), 2);
        assert_eq!(
            census
                .emissions
                .iter()
                .map(|entry| entry.root_case)
                .collect::<Vec<_>>(),
            [Some(0), Some(1)]
        );
        assert!(
            census
                .emissions
                .iter()
                .all(|entry| matches!(entry.outcome, InstanceReadiness::Ready { .. })),
            "{census:?}"
        );
        let saved = serde_json::to_vec(&document).unwrap();
        let decoder = Decoder::load_with_initializer(&saved, &source, &pin, INIT).unwrap();
        // hash("ab")=0x00610062; seed^salt equals the hash. The independent
        // reference recurrence (including its four warmup draws) gives 1,132.
        for (arm, seed, golden) in [
            (0, 0x00610073, vec![0, 0x61, 0, 0x73, 4]),
            (1, 0x00610071, vec![0, 0x61, 0, 0x71, 6, 140]),
        ] {
            let mut report = report();
            report.positional_signals[0] = Some((vec![], golden.clone()));
            let decoded = decoder.decode_report(&report).unwrap();
            let Outcome::Decoded {
                value: shape::signals::wire::WireValue::Choice { arm: selected, .. },
                exact_reencode: true,
            } = &decoded[0].outcome
            else {
                panic!("{:?}", decoded[0].outcome)
            };
            assert_eq!(*selected, arm);
            let Outcome::Decoded { value, .. } = &decoded[0].outcome else {
                unreachable!()
            };
            assert_eq!(
                decoder.encode_slot_for_emission(0, 0, value, seed).unwrap(),
                golden
            );
            assert_eq!(
                decoded[0]
                    .selection
                    .as_ref()
                    .unwrap()
                    .root_case
                    .as_ref()
                    .unwrap()
                    .arm,
                arm
            );
        }
        document
            .wire
            .as_mut()
            .unwrap()
            .initialization
            .as_mut()
            .unwrap()
            .emissions[0]
            .root_case = Some(1);
        assert!(matches!(
            Decoder::load_with_initializer(
                &serde_json::to_vec(&document).unwrap(),
                &source,
                &pin,
                INIT
            ),
            Err(WireError::Evidence)
        ));
    });
}

#[test]
fn public_equivalent_count_emissions_retain_each_source_witness() {
    shape::run_with_large_stack(|| {
        let mut source = source();
        let start = source
            .find("    let seed=Math.random()*4294967296|0;")
            .unwrap();
        let end = source.find("    emitters[0].next([[],b]);").unwrap()
            + "    emitters[0].next([[],b]);".len();
        let arm="let seed=Math.random()*4294967296|0;let mask=streamFactory(seed^17);let b=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];let p=3;let items=[];p=integerWriter(mask,items.length,b,p);for(let i=0;i<items.length;++i){p=integerWriter(mask,items[i],b,p);}emitters[0].next([[],b]);";
        source.replace_range(start..end, &format!("{{{arm}}}{{{arm}}}"));
        source = source.replace(
            "function collect(",
            &format!(
                "{}\nfunction collect(",
                include_str!("../src/signals/wire/reference/number.js")
                    .replace("local0(", "integerWriter(")
            ),
        );
        let pin = pin(&source, &session()).unwrap().layout;
        let analysis = analyze_program_signals(&source, &SignalFinderOptions::default()).unwrap();
        let saved = serde_json::to_vec(&analysis.report).unwrap();
        let mut document = SchemaDocument::from_json(&saved).unwrap();
        document.bind_initializer(&source, &pin, INIT).unwrap();
        assert_eq!(
            document
                .wire
                .as_ref()
                .unwrap()
                .initialization
                .as_ref()
                .unwrap()
                .emissions
                .len(),
            2
        );
        let decoder = Decoder::load_with_initializer(
            &serde_json::to_vec(&document).unwrap(),
            &source,
            &pin,
            INIT,
        )
        .unwrap();
        // Independent reviewed recurrence gives masks 162,73,211, applied to
        // ordinary one-byte NUMBER tags 2,5,9. These bytes are not encoder output.
        let golden = vec![1, 2, 3, 4, 160, 76, 218];
        let mut report = report();
        report.positional_signals[0] = Some((vec![], golden.clone()));
        let decoded = decoder.decode_report(&report).unwrap();
        let Outcome::Decoded {
            value,
            exact_reencode: true,
        } = &decoded[0].outcome
        else {
            panic!("{:?}", decoded[0].outcome)
        };
        let selection = decoded[0].selection.as_ref().unwrap();
        assert_eq!(selection.emissions, [0, 1]);
        assert_eq!(
            selection.proof,
            shape::signals::wire::SelectionProof::EquivalentCountPrograms
        );
        assert_eq!(decoder.encode_slot(0, value, 0x01020304).unwrap(), golden);
    });
}
