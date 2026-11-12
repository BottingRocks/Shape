use shape::naming::labels::build_label_reference;
use shape::naming::{name_threads, name_threads_with_labels};

fn named(source: &str, name: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|o| o.applied && o.proposed_name.as_deref() == Some(name))
}

const BROADCAST: &str = r#"function main(){let listeners=[], definitions=[], states=[];
function $THREAD_6_131817(event){
let kind=event.type;let status=event.status;let value=event.value;let index=event.signalIdx;
listeners.forEach(listener=>{let record={type:kind,status:status,value:value,signalIdx:index,totalSignals:definitions.length};
record.collectedSignals=states.filter(item=>item.hasCollected).length;
record.erroredSignals=states.filter(item=>item.hasErrored).length;listener(record);});
}return $THREAD_6_131817;}main();"#;

#[test]
fn notification_broadcast_proves_record_callback_and_table_identities() {
    for source in [
        BROADCAST.to_string(),
        BROADCAST
            .replace("$THREAD_6_131817", "$THREAD_999_878787_2")
            .replace("event", "$KEY_900")
            .replace("record", "notification")
            .replace("states", "signalStates")
            .replace("listener", "subscriber"),
        BROADCAST.replace(
            "type:kind,status:status,value:value,signalIdx:index,totalSignals:definitions.length",
            "totalSignals:definitions.length,signalIdx:index,value:value,status:status,type:kind",
        ),
    ] {
        assert!(named(&source, "broadcastSignalNotification"));
    }
    let human = BROADCAST.replace("$THREAD_6_131817", "notifySubscribers");
    assert_eq!(name_threads(&human, true).unwrap().source, human);
}

#[test]
fn notification_broadcast_rejects_changed_operations_and_scoped_identities() {
    for (from, to) in [
        ("event.type", "other.type"),
        ("event.status", "event.value"),
        ("type:kind", "type:value"),
        ("type:kind", "unexpected:0"),
        ("type:kind", "['type']:kind"),
        (
            "totalSignals:definitions.length",
            "totalSignals:states.length",
        ),
        (
            "totalSignals:definitions.length",
            "totalSignals:listeners.length",
        ),
        ("listeners.forEach", "listeners.map"),
        ("listener=>", "(listener,extra)=>"),
        ("listener=>", "async listener=>"),
        ("states.filter", "states.map"),
        ("item.hasCollected", "item.hasErrored"),
        ("item.hasCollected", "other.hasCollected"),
        ("item.hasCollected", "item.hasCollected()"),
        (
            "record.erroredSignals=states",
            "record.erroredSignals=definitions",
        ),
        ("record.erroredSignals", "other.erroredSignals"),
        ("listener(record)", "other(record)"),
        ("listener(record)", "listener(other)"),
        ("listener(record)", "listener(record,1)"),
        ("listener(record)", "return listener(record)"),
        ("listener(record)", "listener?.(record)"),
        ("listener(record)", "record=other;listener(record)"),
        ("let kind=event.type;", "var kind=event.type;var kind;"),
        (
            "function $THREAD_6_131817(event)",
            "function $THREAD_6_131817(event,other)",
        ),
        (
            "function $THREAD_6_131817(event)",
            "function $THREAD_6_131817(event=other)",
        ),
        (
            "listener=>{let record",
            "listener=>{let listener;let record",
        ),
        (
            "let listeners=[], definitions=[], states=[];",
            "var listeners=[], definitions=[], states=[];var states;",
        ),
    ] {
        let source = BROADCAST.replace(from, to);
        // A syntactically invalid duplicate parameter is also a refusal.
        assert!(
            !name_threads(&source, true).is_ok_and(|result| result
                .report
                .outcomes
                .iter()
                .any(|o| o.applied
                    && o.proposed_name.as_deref() == Some("broadcastSignalNotification"))),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn exact_rgba_channels_survive_generated_id_and_parameter_changes() {
    let source = r#"function main(){function $THREAD_7_78621(channels){return "rgba("+channels[0]+","+channels[1]+","+channels[2]+","+channels[3]+")";}return $THREAD_7_78621;}main();"#;
    for text in [
        source.to_string(),
        source
            .replace("$THREAD_7_78621", "$THREAD_92_881234")
            .replace("channels", "$KEY_994"),
    ] {
        assert!(named(&text, "formatRgbaChannels"));
        let result = name_threads(&text, true).unwrap();
        let mut replay = text.clone();
        for e in result.report.edits.iter().rev() {
            replay.replace_range(e.start as usize..e.end as usize, &e.replacement);
        }
        assert_eq!(replay, result.source);
    }
    for changed in [
        source.replace("rgba(", "rgb("),
        source.replace("channels[3]", "channels[2]"),
        source.replace("channels[0]", "other[0]"),
        source
            .replace("return \"rgba(", "return helper(\"rgba(")
            .replace("+\")\";", "+\")\");"),
    ] {
        assert!(!named(&changed, "formatRgbaChannels"));
    }
    let human = source.replace("$THREAD_7_78621", "formatColor");
    assert_eq!(name_threads(&human, true).unwrap().source, human);
}

#[test]
fn conditional_empty_call_requires_exact_receiver_arguments_and_return_semantics() {
    let source="function main(){let queue=createQueue();function $THREAD_1_9(){if(queue){queue.empty();}}return $THREAD_1_9;}main();";
    assert!(named(source, "callEmptyIfPresent"));
    for text in [
        source.replace("queue.empty()", "other.empty()"),
        source.replace("queue.empty()", "queue.clear()"),
        source.replace("queue.empty()", "queue.empty(1)"),
        source.replace("queue.empty();", "return queue.empty();"),
        source.replace("if(queue)", "if(other)"),
        source.replace("return $THREAD_1_9", "queue=other;return $THREAD_1_9"),
    ] {
        assert!(!named(&text, "callEmptyIfPresent"));
    }
}

const BYTE_FACTORY: &str = r#"function main(){let salt=123;
function $THREAD_1_100(seed){let a,b,c,d;
function $THREAD_2_200(){let old=d;d=(old+1)%4;
if((old+1)%4===0){b=c+a;let x=a;a=x^c;c=x^c^(c<<26|c>>>6)^a<<9;a=a<<13|a>>>19;return b&255;}
if((old+1)%4===1){return b>>8&255;}
if((old+1)%4===2){return b>>16&255;}
if((old+1)%4===3){return b>>>24&255;}else{return void 0;}}
c=salt;a=seed;d=-1;$THREAD_2_200();$THREAD_2_200();$THREAD_2_200();$THREAD_2_200();return $THREAD_2_200;}
return $THREAD_1_100;}main();"#;
#[test]
fn warmed_factory_uses_proven_nested_byte_role_and_exact_calls() {
    assert!(named(BYTE_FACTORY, "createKeystreamByteSource"));
    assert!(named(
        &BYTE_FACTORY
            .replace("$THREAD_1_100", "$THREAD_991_887")
            .replace("seed", "initialState"),
        "createKeystreamByteSource"
    ));
    for text in [
        BYTE_FACTORY.replacen("$THREAD_2_200();", "other();", 1),
        BYTE_FACTORY.replacen("$THREAD_2_200();", "$THREAD_2_200(1);", 1),
        BYTE_FACTORY.replace("b>>>24&255", "b>>>23&255"),
        BYTE_FACTORY.replace("return $THREAD_2_200;", "return other;"),
        BYTE_FACTORY.replace("c=salt;", "$THREAD_2_200=other;c=salt;"),
    ] {
        assert!(!named(&text, "createKeystreamByteSource"));
    }
}

const EQUALITY: &str = r#"function main(){
let tags={UINT14_BASE:0,UINT16:1,UINT24:2,UINT32:3,UINT64:4,NINT4_BASE:5,FLOAT32:6,DOUBLE64:7,STR5_BASE:8,STRREF:9,STRLUT:10,ARRAY8:11,BARRAY8:12,MAP_:13,BMAP_:14,EXTENSION:15};
function $THREAD_1_10(a,b){let other=b;if(a.length===b.length){return a.every((item,index)=>other[index]===item);}else{return false;}}
let compare=$THREAD_1_10;let target=[];
function $THREAD_2_20(value){return compare(value,target);}
return [$THREAD_1_10,$THREAD_2_20];}main();"#;
#[test]
fn resolved_array_comparison_refuses_mutable_and_shadowed_aliases() {
    assert!(named(EQUALITY, "matchesArrayContents"));
    for source in [
        EQUALITY.replace(
            "return compare(value,target)",
            "return compare(target,value)",
        ),
        EQUALITY.replace(
            "return compare(value,target)",
            "return !compare(value,target)",
        ),
        EQUALITY.replace("let target=[];", "let target=[];compare=other;"),
        EQUALITY.replace(
            "function $THREAD_2_20(value)",
            "function $THREAD_2_20(value,compare)",
        ),
        EQUALITY.replace(
            "let compare=$THREAD_1_10;",
            "let compare=$THREAD_1_10;$THREAD_1_10=other;",
        ),
    ] {
        assert!(!named(&source, "matchesArrayContents"));
    }
}

#[test]
fn unrelated_inherited_claim_still_blocks_an_independently_proven_role() {
    let human = BYTE_FACTORY.replace("$THREAD_1_100", "otherPurpose");
    let reference = build_label_reference(BYTE_FACTORY, &human).unwrap();
    let result = name_threads_with_labels(BYTE_FACTORY, true, Some(&reference)).unwrap();
    let factory = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_1_100")
        .unwrap();
    assert!(!factory.applied);
    assert!(factory.reason.as_deref().unwrap().contains("conflicting"));
}

const OBSERVER: &str = r#"function main(){let values=[],statuses=[],report={scrBytes:{data:[],sidx:[]}};
function $THREAD_5_31572(index){let slot=index;let observer={next:void 0,error:void 0,complete:void 0};
function $THREAD_4_3288(value){values[slot]=value;statuses[slot].hasCollected=true;notify({type:"collect",status:statuses[slot],value:value,signalIdx:slot});}
function $THREAD_5_95511(error){logError(slot,false,error);statuses[slot].hasErrored=true;notify({type:"error",status:statuses[slot],value:error,signalIdx:slot});}
function $THREAD_4_32367(){let value=values[slot];if(Array.isArray(value)&&value.length>=2){report.scrBytes.sidx.push(slot);let data=report.scrBytes.data;data.push([value[0].slice(0,8),value[1].slice(0,8)]);}}
observer.next=$THREAD_4_3288;observer.error=$THREAD_5_95511;observer.complete=$THREAD_4_32367;return observer;}
return $THREAD_5_31572;}main();"#;
#[test]
fn observer_factory_requires_proven_callbacks_on_the_returned_object() {
    assert!(named(OBSERVER, "createSignalObserver"));
    let literal_next = OBSERVER
        .replace("next:void 0", "next:$THREAD_4_3288")
        .replace("observer.next=$THREAD_4_3288;", "");
    assert!(named(&literal_next, "createSignalObserver"));
    assert!(!named(
        &OBSERVER.replace(
            "observer.next=$THREAD_4_3288;",
            "if(enabled){observer.next=$THREAD_4_3288;}"
        ),
        "createSignalObserver"
    ));
    assert!(named(
        &OBSERVER
            .replace("$THREAD_5_31572", "$THREAD_88_123456")
            .replace("observer", "$KEY_555"),
        "createSignalObserver"
    ));
    for source in [
        OBSERVER.replace(
            "observer.error=$THREAD_5_95511",
            "observer.error=$THREAD_4_3288",
        ),
        OBSERVER.replace("return observer", "return other"),
        OBSERVER.replace(
            "observer.complete=$THREAD_4_32367",
            "observer.complete=other",
        ),
        OBSERVER.replace("let slot=index;", "let slot=index;$THREAD_4_3288=other;"),
    ] {
        assert!(!named(&source, "createSignalObserver"));
    }
}

#[test]
fn proven_descriptor_role_disambiguates_only_known_inherited_templates() {
    let original = r#"function main(){function $THREAD_1_1(){function $THREAD_2_2(cls,proto,statics){for(let i=0;i<proto.length;i++){let d=proto[i];d.enumerable=d.enumerable||false;d.configurable=true;if("value" in d)d.writable=true;Object.defineProperty(cls.prototype,d.key,d);}for(let i=0;i<statics.length;i++){let d=statics[i];d.enumerable=d.enumerable||false;d.configurable=true;if("value" in d)d.writable=true;Object.defineProperty(cls,d.key,d);}return cls;}return $THREAD_2_2;}return $THREAD_1_1;}main();"#;
    assert!(named(original, "installPropertyDescriptors"));
    let human = original
        .replace("$THREAD_1_1", "createClassMembersInstaller")
        .replace("$THREAD_2_2", "defineClassMembers");
    let reference = build_label_reference(original, &human).unwrap();
    let mut reference = reference;
    let mut extra = reference
        .labels
        .iter()
        .find(|l| l.name == "defineClassMembers")
        .unwrap()
        .clone();
    extra.name = "defineClassProperties".into();
    reference.labels.push(extra);
    let result = name_threads_with_labels(original, true, Some(&reference)).unwrap();
    let inner = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_2_2")
        .unwrap();
    assert!(inner.applied);
    assert_eq!(
        inner.proposed_name.as_deref(),
        Some("installPropertyDescriptors")
    );
    assert!(inner
        .explanation
        .iter()
        .any(|e| e.contains("weaker inherited body-template")));
    let mut wrong = reference;
    wrong
        .labels
        .iter_mut()
        .for_each(|label| label.name = "unrelatedPurpose".into());
    let result = name_threads_with_labels(original, true, Some(&wrong)).unwrap();
    assert!(result.report.outcomes.iter().all(|o| !o.applied));
}

#[test]
fn string_factory_requires_independent_writer_and_null_prototype_allocation() {
    let source = include_str!("fixtures/program/shared-interned-writer.js");
    assert!(named(source, "createInternedStringWriter"));
    assert!(named(
        &source
            .replace("$THREAD_8_51922", "$THREAD_91_66544")
            .replace("$KEY_0", "cacheNextIndex"),
        "createInternedStringWriter"
    ));
    for changed in [
        source.replace("Object.create(null)", "Object.create({})"),
        source.replace("Object.create(null)", "Other.create(null)"),
        source.replace("return $THREAD_1_19915;", "return other;"),
        source.replace(
            "return $THREAD_1_19915;",
            "$THREAD_1_19915=other;return $THREAD_1_19915;",
        ),
    ] {
        assert!(!named(&changed, "createInternedStringWriter"));
    }
}

#[test]
fn written_parameters_and_other_runtime_origins_cannot_be_single_origin_aliases() {
    let parameter = EQUALITY
        .replace("function main()", "function main(compare)")
        .replace("let compare=$THREAD_1_10;", "")
        .replace(
            "return [$THREAD_1_10",
            "callBeforeAssignment($THREAD_2_20);compare=$THREAD_1_10;return [$THREAD_1_10",
        );
    for source in [
        parameter.clone(),
        EQUALITY.replace(
            "let compare=$THREAD_1_10;",
            "var compare=externalComparator;var compare;compare=$THREAD_1_10;",
        ),
        parameter.replace("main(compare)", "main(compare=externalComparator)"),
        parameter.replace("main(compare)", "main({compare})"),
        parameter.replace("let target=[];", "var compare;let target=[];"),
        parameter.replace("main(compare)", "main(...compare)"),
        EQUALITY.replace(
            "let compare=$THREAD_1_10;",
            "let {compare}=externalObject;compare=$THREAD_1_10;",
        ),
        EQUALITY.replace(
            "let compare=$THREAD_1_10;",
            "let [compare]=externalArray;compare=$THREAD_1_10;",
        ),
        parameter
            .replace(
                "function main(compare){",
                "function main(){try{throw externalComparator;}catch(compare){",
            )
            .replace("}main();", "}}main();"),
        parameter
            .replace(
                "function main(compare){",
                "function main(){for(let compare of externalComparators){",
            )
            .replace("}main();", "}}main();"),
        parameter
            .replace(
                "function main(compare){",
                "function main(){for(let compare in externalComparators){",
            )
            .replace("}main();", "}}main();"),
    ] {
        let result = name_threads(&source, true).unwrap();
        let wrapper = result
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == "$THREAD_2_20")
            .unwrap();
        assert!(
            !wrapper.applied,
            "runtime-origin alias was trusted: {source}"
        );
        assert!(result.source.contains("function $THREAD_2_20(value)"));
    }
}

#[test]
fn plain_single_assignment_local_aliases_keep_their_proven_function_origin() {
    for declaration in ["let", "var"] {
        let source = EQUALITY.replace(
            "let compare=$THREAD_1_10;",
            &format!("{declaration} compare;compare=$THREAD_1_10;"),
        );
        assert!(named(&source, "matchesArrayContents"));
    }
    let chain = EQUALITY.replace(
        "let compare=$THREAD_1_10;",
        "const helper=$THREAD_1_10;let compare;compare=helper;",
    );
    assert!(named(&chain, "matchesArrayContents"));
    assert!(named(EQUALITY, "matchesArrayContents"));
}

#[test]
fn duplicate_callee_and_redeclared_aliases_do_not_prove_wrapper_roles() {
    for source in [
        EQUALITY.replace(
            "let compare=$THREAD_1_10;",
            "function $THREAD_1_10(a,b){return false;}let compare=$THREAD_1_10;",
        ),
        EQUALITY.replace(
            "let compare=$THREAD_1_10;",
            "var compare=$THREAD_1_10;var compare;",
        ),
    ] {
        let result = name_threads(&source, true).unwrap();
        let wrapper = result
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == "$THREAD_2_20")
            .unwrap();
        // The array-comparison role needs the callee's single origin. The
        // structural forwarder rule (sole formal and one captured value passed
        // to a captured callable) does not, and may still name the body.
        let generic_forwarder = wrapper.evaluator.as_deref()
            == Some("runtime_captured_callee_wrappers")
            && wrapper.proposed_name.as_deref()
                == Some("forwardFormalAndCapturedToCapturedCallable");
        assert!(
            !wrapper.applied || generic_forwarder,
            "ambiguous declaration origin was trusted: {source}"
        );
        assert!(!result.source.contains("matchesArrayContents"));
        assert!(
            result.source.contains("function $THREAD_2_20(value)")
                || result
                    .source
                    .contains("function forwardFormalAndCapturedToCapturedCallable(value)")
        );
    }
}

#[test]
fn equal_spellings_in_distinct_scopes_are_independent_function_origins() {
    let source=EQUALITY.replace("let compare=$THREAD_1_10;", "function otherScope(){function $THREAD_1_10(a,b){return false;}return $THREAD_1_10;}let compare=$THREAD_1_10;");
    let result = name_threads(&source, true).unwrap();
    let wrapper = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_2_20")
        .unwrap();
    assert!(wrapper.applied);
    assert_eq!(
        wrapper.proposed_name.as_deref(),
        Some("matchesArrayContents")
    );
    let same_spelling: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|o| o.binding.name == "$THREAD_1_10")
        .collect();
    assert_eq!(same_spelling.len(), 2);
    assert_ne!(
        same_spelling[0].binding.symbol,
        same_spelling[1].binding.symbol
    );
    assert!(same_spelling.iter().all(|o| o.applied));
}

#[test]
fn partial_observer_literals_require_unique_local_object_and_callbacks() {
    let partial = OBSERVER
        .replace(
            "next:void 0,error:void 0,complete:void 0",
            "next:$THREAD_4_3288",
        )
        .replace("observer.next=$THREAD_4_3288;", "");
    for source in [
        partial.clone(),
        partial
            .replace("$THREAD_5_31572", "$THREAD_41_99881_2")
            .replace("observer", "result"),
    ] {
        assert!(named(&source, "createSignalObserver"));
    }
    for source in [
        partial.replace(
            "next:$THREAD_4_3288",
            "next:$THREAD_4_3288,next:$THREAD_4_3288",
        ),
        partial.replace(
            "observer.error=",
            "observer.next=$THREAD_4_3288;observer.error=",
        ),
        partial.replace("observer.error=", "observer.error=other;observer.error="),
        partial.replace("observer.error", "observer[key]"),
        partial.replace("return observer", "let escaped=observer;return observer"),
        partial.replace("return observer", "return other"),
        partial.replace("return observer", "observer={};return observer"),
        partial.replace("let observer=", "observer="),
        partial.replace("let observer=", "var observer;var observer="),
        partial.replace(
            "observer.error=$THREAD_5_95511",
            "observer.error=$THREAD_4_3288",
        ),
        partial.replace("hasErrored=true", "hasCollected=true"),
        partial
            .replace(
                "let observer={next:$THREAD_4_3288};",
                "observer.error=$THREAD_5_95511;let observer={next:$THREAD_4_3288};",
            )
            .replace(
                "observer.error=$THREAD_5_95511;observer.complete",
                "observer.complete",
            ),
        partial.replace("return observer", "delete observer.next;return observer"),
        partial.replace("return observer", "observer.extra=other;return observer"),
        partial.replace("let observer=", "let {observer}="),
        partial.replace(
            "observer.complete=$THREAD_4_32367;return observer",
            "return observer;observer.complete=$THREAD_4_32367;",
        ),
    ] {
        assert!(!named(&source, "createSignalObserver"), "{source}");
    }
}

const HASH_DISPATCH: &str = include_str!("fixtures/program/shared-hash-dispatch.js");

#[test]
fn hash_dispatch_requires_independent_callees_and_ordered_arguments() {
    for source in [
        HASH_DISPATCH.to_string(),
        HASH_DISPATCH
            .replace("$THREAD_13_130", "$THREAD_73_66712_3")
            .replace("captured389", "stringHasher"),
    ] {
        assert!(named(&source, "hashStringOrArrayBufferViewAsync"));
    }
    for source in [
        HASH_DISPATCH.replace(
            "captured389($AP, $AQ, $AR, $AS)",
            "captured389($AQ, $AP, $AR, $AS)",
        ),
        HASH_DISPATCH.replace(
            "captured301($AP, $AQ, $AR, $AS)",
            "captured389($AP, $AQ, $AR, $AS)",
        ),
        HASH_DISPATCH.replace("ArrayBuffer.isView", "ArrayBuffer.isArray"),
        HASH_DISPATCH.replace("typeof $AP === \"string\"", "typeof $AQ === \"string\""),
        HASH_DISPATCH.replace("typeof $AP === \"string\"", "typeof $AP === \"number\""),
        HASH_DISPATCH.replace("new TypeError()", "new Error()"),
        HASH_DISPATCH.replace("$AS(new TypeError())", "$AR(new TypeError())"),
        HASH_DISPATCH.replace(
            "captured389($AP, $AQ, $AR, $AS);",
            "return captured389($AP, $AQ, $AR, $AS);",
        ),
        HASH_DISPATCH.replace(
            "let captured389=$THREAD_12_120;",
            "let captured389=$THREAD_12_120;captured389=other;",
        ),
        HASH_DISPATCH.replace(
            "let captured389=$THREAD_12_120;",
            "var captured389;var captured389=$THREAD_12_120;",
        ),
        HASH_DISPATCH.replace(
            "function $THREAD_13_130($AP, $AQ, $AR, $AS)",
            "function $THREAD_13_130($AP, $AQ, $AR, $AS, captured389)",
        ),
        HASH_DISPATCH.replace(
            "let captured389=$THREAD_12_120;",
            "let captured389=hashStringAsync;",
        ),
        HASH_DISPATCH.replace(
            "function $THREAD_13_130($AP",
            "function $THREAD_13_130(ArrayBuffer",
        ),
    ] {
        assert!(
            !named(&source, "hashStringOrArrayBufferViewAsync"),
            "{source}"
        );
    }
    let human = HASH_DISPATCH.replace("$THREAD_13_130", "humanHashDispatcher");
    let result = name_threads(&human, true).unwrap();
    assert!(result.source.contains("function humanHashDispatcher("));
    let collision = HASH_DISPATCH
        .replace(
            "function main(){",
            "function main(){let hashStringOrArrayBufferViewAsync=42;",
        )
        .replace(
            "return $THREAD_13_130",
            "consume(hashStringOrArrayBufferViewAsync);return $THREAD_13_130",
        );
    assert!(!named(&collision, "hashStringOrArrayBufferViewAsync"));
}

#[test]
fn generated_collision_suffix_metadata_and_actual_binding_counts_agree() {
    let source = r#"function main(){function $THREAD_4_44(x){return x;}function $THREAD_4_44_2(e){sink.error(e);}function $THREAD_4_44_3(x){unknown(x);}function $THREAD_4_44_custom(x){return x;}function $THREAD_4_44_1(x){return x;}function $THREAD_4_44_02(x){return x;}function human_4_44_2(x){return x;}function $THREAD_bad(){}return [$THREAD_4_44,$THREAD_4_44_2,$THREAD_4_44_3,$THREAD_4_44_custom,$THREAD_4_44_1,$THREAD_4_44_02,human_4_44_2];}main();"#;
    let result = name_threads(source, true).unwrap();
    assert_eq!(result.report.generated_thread_count, 7);
    let clone = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_4_44_2")
        .unwrap();
    assert_eq!(
        (clone.thread_id, clone.pc, clone.disambiguator),
        (Some(4), Some(44), Some(2))
    );
    assert!(clone.applied);
    assert_eq!(clone.proposed_name.as_deref(), Some("forwardError"));
    let metadata = result
        .report
        .collectors
        .functions
        .iter()
        .find(|f| f.original_thread_name.as_deref() == Some("$THREAD_4_44_2"))
        .unwrap();
    assert_eq!(
        (metadata.thread_id, metadata.pc, metadata.disambiguator),
        (Some(4), Some(44), Some(2))
    );
    for suffix in ["custom", "1", "02"] {
        let o = result
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == format!("$THREAD_4_44_{suffix}"))
            .unwrap();
        assert!(!o.applied);
        assert_eq!(o.thread_id, None);
        assert!(o.reason.as_deref().unwrap().contains("unsupported"));
    }
    let allocator = oxc_allocator::Allocator::default();
    let program = shape::analysis::parse(&allocator, &result.source).program;
    let (_, nodes) = shape::analysis::analyze(&program);
    let count = nodes.iter().filter(|n|matches!(n.kind(),oxc_ast::AstKind::Function(f) if f.id.as_ref().is_some_and(|i|i.name.starts_with("$THREAD_")))).count();
    assert_eq!(count, 5);
    assert_eq!(
        count,
        result.report.outcomes.iter().filter(|o| !o.applied).count()
    );
    assert!(result.source.contains("function human_4_44_2("));
}

#[test]
fn suffix_collisions_allocate_ordinals_and_allow_independent_scopes() {
    let source = r#"function main(){function $THREAD_1_10(e){sink.error(e);}function $THREAD_2_20_2(e){sink.error(e);}return [$THREAD_1_10,$THREAD_2_20_2];}main();"#;
    let result = name_threads(source, true).unwrap();
    let original = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_1_10")
        .unwrap();
    assert!(original.applied);
    assert_eq!(original.proposed_name.as_deref(), Some("forwardError"));
    let clone = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_2_20_2")
        .unwrap();
    assert!(clone.applied);
    assert_eq!(clone.proposed_name.as_deref(), Some("forwardError2"));
    let nested = r#"function main(){function $THREAD_1_10(e){sink.error(e);}function wrap(){function $THREAD_2_20_2(e){sink.error(e);}return [$THREAD_1_10,$THREAD_2_20_2];}return [$THREAD_1_10,wrap];}main();"#;
    let result = name_threads(nested, true).unwrap();
    assert_eq!(
        result
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == "$THREAD_1_10")
            .unwrap()
            .proposed_name
            .as_deref(),
        Some("forwardError")
    );
    assert!(
        result
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == "$THREAD_2_20_2")
            .unwrap()
            .applied
    );
    let siblings = r#"function main(){function a(){function $THREAD_1_10(e){sink.error(e);}return $THREAD_1_10;}function b(){function $THREAD_2_20_2(e){sink.error(e);}return $THREAD_2_20_2;}return [a,b];}main();"#;
    let result = name_threads(siblings, true).unwrap();
    assert!(result.report.outcomes.iter().all(|o| o.applied));
}

#[test]
fn suffixed_block_functions_respect_annex_b_and_strict_sibling_scopes() {
    let source = r#"function main(){let callbacks=[];if(first){function $THREAD_1_10(e){sink.error(e);}callbacks.push($THREAD_1_10);}if(second){function $THREAD_2_20_2(e){sink.error(e);}callbacks.push($THREAD_2_20_2);}return callbacks;}main();"#;
    let result = name_threads(source, true).unwrap();
    assert_eq!(
        result
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == "$THREAD_1_10")
            .unwrap()
            .proposed_name
            .as_deref(),
        Some("forwardError")
    );
    assert!(
        result
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == "$THREAD_2_20_2")
            .unwrap()
            .applied
    );
    let strict = format!("\"use strict\";{source}");
    let result = name_threads(&strict, true).unwrap();
    assert!(result.report.outcomes.iter().all(|o| o.applied));
}

const COMPLETION_ADAPTER: &str =
    "function main(){function $THREAD_1_2(task, done) { try { task(); done(); } catch (error) { done(error); } }return $THREAD_1_2;}main();";
const DESCRIPTOR_SEARCH: &str = "function main(){function $THREAD_1_2(obj,key) { let cursor=obj; while(cursor!=null) { let desc=Object.getOwnPropertyDescriptor(cursor,key); if(desc!=null) { cursor={containingObj:cursor,desc:desc}; return cursor; } else { cursor=Object.getPrototypeOf(cursor); continue; } } return null; }return $THREAD_1_2;}main();";
const DOCUMENT_CAPTURE: &str = "function main(){let doc;function $THREAD_1_2(context){doc=context.global.document;}return $THREAD_1_2;}main();";

#[test]
fn completion_adapter_proves_formal_and_catch_identity() {
    for source in [
        COMPLETION_ADAPTER.to_string(),
        COMPLETION_ADAPTER
            .replace("$THREAD_1_2", "$THREAD_998_777_22")
            .replace("task", "work")
            .replace("done", "finish")
            .replace("error", "caught"),
        COMPLETION_ADAPTER
            .replace("catch (error)", "catch ($KEY_500)")
            .replace("done(error)", "done($KEY_500)"),
    ] {
        assert!(named(&source, "runTaskWithCompletionCallback"));
    }
    for (from, to) in [
        ("task()", "task(0)"),
        ("task()", "done()"),
        ("done();", "return done();"),
        ("done(error)", "done(task)"),
        ("done(error)", "other(error)"),
        ("done(error)", "done(error,0)"),
        ("task(); done();", "task(); done(); done();"),
        ("task();", "task?.();"),
        ("done(error)", "done({error:error})"),
        ("done(error)", "done(null)"),
        ("task, done", "task, done=other"),
        ("task(); done();", "var task; task(); done();"),
        ("done(error)", "var done; done(error)"),
        ("catch (error)", "catch ({error})"),
    ] {
        assert!(
            !named(
                &COMPLETION_ADAPTER.replace(from, to),
                "runTaskWithCompletionCallback"
            ),
            "{from} -> {to}"
        );
    }
    assert!(!named(
        &format!("{COMPLETION_ADAPTER} function other(){{}}")
            .replace("done(error)", "other(error)"),
        "runTaskWithCompletionCallback"
    ));
}

#[test]
fn descriptor_search_proves_prototype_walk_and_exact_record() {
    for source in [
        DESCRIPTOR_SEARCH.to_string(),
        DESCRIPTOR_SEARCH
            .replace("$THREAD_1_2", "$THREAD_80_909_2")
            .replace("cursor", "current")
            .replace("obj", "input")
            .replace("key", "property"),
        DESCRIPTOR_SEARCH.replace(
            "{containingObj:cursor,desc:desc}",
            "{desc:desc,containingObj:cursor}",
        ),
    ] {
        assert!(named(&source, "findPropertyDescriptorInPrototypeChain"));
    }
    for (from, to) in [
        ("getOwnPropertyDescriptor", "getOwnPropertyNames"),
        ("getPrototypeOf", "getOwnPropertyDescriptor"),
        ("cursor,key)", "obj,key)"),
        ("cursor,key)", "cursor,obj)"),
        ("cursor,key)", "cursor,key,0)"),
        ("return null", "return false"),
        ("containingObj:cursor", "containingObj:obj"),
        ("desc:desc", "desc:cursor"),
        ("containingObj:cursor", "other:cursor"),
        ("cursor!=null", "cursor===null"),
        ("desc!=null", "desc==null"),
        ("let cursor=obj;", "var cursor=obj;var cursor;"),
        ("let desc=Object", "var desc; var desc=Object"),
        ("obj,key", "obj,key=other"),
        ("let cursor=obj;", "let cursor=other;"),
        (
            "Object.getPrototypeOf(cursor)",
            "Object.getPrototypeOf(obj)",
        ),
        ("return cursor;", "return desc;"),
        ("return cursor;", "return {containingObj:cursor,desc:desc};"),
    ] {
        assert!(
            !named(
                &DESCRIPTOR_SEARCH.replace(from, to),
                "findPropertyDescriptorInPrototypeChain"
            ),
            "{from} -> {to}"
        );
    }
    let shadowed = DESCRIPTOR_SEARCH.replace("(obj,key)", "(obj,key,Object)");
    assert!(!named(&shadowed, "findPropertyDescriptorInPrototypeChain"));
}

#[test]
fn document_capture_proves_exact_path_and_captured_plain_binding() {
    for source in [
        DOCUMENT_CAPTURE.to_string(),
        DOCUMENT_CAPTURE
            .replace("$THREAD_1_2", "$THREAD_991_123_3")
            .replace("doc;", "documentSlot;")
            .replace("doc=", "documentSlot=")
            .replace("context", "environment"),
        DOCUMENT_CAPTURE.replace("let doc;", "var doc=null;"),
    ] {
        assert!(named(&source, "captureGlobalDocument"));
    }
    for (from, to) in [
        ("global.document", "global.navigator"),
        ("global.document", "document"),
        ("global.document", "global.document()"),
        ("doc=context", "doc=other"),
        (
            "doc=context.global.document;",
            "return context.global.document;",
        ),
        (
            "doc=context.global.document;",
            "doc=context.global.document;return doc;",
        ),
        (
            "doc=context.global.document;",
            "doc={value:context.global.document};",
        ),
        ("doc=context.global.document;", "doc=0;"),
        ("let doc;", "var doc;var doc;"),
        ("let doc;", ""),
        ("function main()", "function main(doc)"),
        ("doc=context", "let doc=context"),
        ("(context)", "(context=other)"),
        ("(context)", "({context})"),
        ("global.document", "global?.document"),
    ] {
        assert!(
            !named(&DOCUMENT_CAPTURE.replace(from, to), "captureGlobalDocument"),
            "{from} -> {to}"
        );
    }
}

const PROPERTY_COUNT: &str = "function main(){let recur;function $THREAD_1_2(obj){if(obj==null){return 0;}else{return Object.getOwnPropertyNames(obj).length+recur(Object.getPrototypeOf(obj));}}recur=$THREAD_1_2;return recur;}main();";
#[test]
fn recursive_property_count_requires_exact_self_origin_and_prototype_step() {
    for source in [
        PROPERTY_COUNT.to_string(),
        PROPERTY_COUNT
            .replace("$THREAD_1_2", "$THREAD_987_654_3")
            .replace("recur", "walk")
            .replace("obj", "value"),
        PROPERTY_COUNT.replace("recur(Object", "$THREAD_1_2(Object"),
    ] {
        assert!(named(&source, "countOwnPropertiesAlongPrototypeChain"));
    }
    for (from, to) in [
        ("getOwnPropertyNames", "keys"),
        ("getPrototypeOf", "getOwnPropertyNames"),
        ("return 0", "return 1"),
        (".length+", ".size+"),
        (".length+", ".length-"),
        ("Names(obj)", "Names(other)"),
        ("Of(obj)", "Of(other)"),
        ("Of(obj)", "Of(obj,0)"),
        ("recur(Object", "other(Object"),
        ("return 0;", "return {};"),
        ("let recur;", "var recur; var recur;"),
        ("recur=$THREAD_1_2;", "recur=$THREAD_1_2; recur=other;"),
        ("(obj){", "(obj=other){"),
        ("(obj){", "(obj){var obj;"),
        ("function main()", "function main(recur)"),
        ("recur=$THREAD_1_2;", "recur=other;"),
    ] {
        let source = PROPERTY_COUNT.replace(from, to);
        assert!(
            !named(&source, "countOwnPropertiesAlongPrototypeChain"),
            "{from} -> {to}"
        );
    }
    let source = PROPERTY_COUNT.replace("let recur;", "let recur; let Object=other;");
    assert!(!named(&source, "countOwnPropertiesAlongPrototypeChain"));
}
