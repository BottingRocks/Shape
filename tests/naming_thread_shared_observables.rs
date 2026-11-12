use shape::naming::name_threads;

fn claimed(source: &str, name: &str, evaluator: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|o| {
            o.applied
                && o.proposed_name.as_deref() == Some(name)
                && o.evaluator.as_deref() == Some(evaluator)
        })
}
fn forward(source: &str) -> bool {
    claimed(source, "forwardError", "shared_observer_error_forwarder")
}
fn factory(source: &str) -> bool {
    claimed(source, "createSignalObserver", "shared_observer_factory")
}

const FORWARD: &str = "function main(sink){function $THREAD_7_192_2(error){let receiver=sink;let method=receiver.error;method.call(receiver,error);}return $THREAD_7_192_2;}main();";
const TUPLE: &str = "function main(sink){function $THREAD_7_192_2(error){let pair=[sink,sink.error];pair[1].call(pair[0],error);}return $THREAD_7_192_2;}main();";
#[test]
fn error_forwarder_supports_stable_aliases_and_lowered_slots() {
    for s in [
        FORWARD.to_string(),
        TUPLE.to_string(),
        FORWARD
            .replace("$THREAD_7_192_2", "$THREAD_900_817_3")
            .replace("error)", "$KEY_18)")
            .replace(",error)", ",$KEY_18)"),
        FORWARD.replace("receiver.error", "receiver['error']"),
        FORWARD.replace(
            "let method=receiver.error;method.call(receiver,error)",
            "let value=error;receiver.error(value)",
        ),
        TUPLE
            .replace("let pair=", "const pair=")
            .replace("pair[1].call", "pair[1]['call']"),
    ] {
        assert!(forward(&s), "{s}");
    }
}
#[test]
fn error_forwarder_refuses_changed_api_arguments_order_returns_and_identity() {
    for (from, to) in [
        ("receiver.error", "receiver.next"),
        ("method.call", "method.apply"),
        ("method.call(receiver,error)", "method.call(error,receiver)"),
        ("method.call(receiver,error)", "method.call(other,error)"),
        ("method.call(receiver,error)", "method.call(receiver,other)"),
        (
            "method.call(receiver,error)",
            "method.call(receiver,error,1)",
        ),
        (
            "method.call(receiver,error)",
            "return method.call(receiver,error)",
        ),
        ("method.call", "method?.call"),
        (
            "method.call(receiver,error)",
            "method.call(receiver,...error)",
        ),
        ("let receiver=sink;", "let receiver=sink;receiver=other;"),
        (
            "let method=receiver.error;",
            "let method=receiver.error;method=other;",
        ),
        ("let receiver=sink;", "var receiver=sink;var receiver;"),
        ("let receiver=sink;", "let receiver=sink;sink=other;"),
        ("(error){", "(error=other){"),
        ("(error){", "(...error){"),
        ("(error){", "({error}){"),
        ("(error){", "(error,receiver){"),
        ("let receiver=sink;", "let receiver=sink;error=other;"),
        (
            "let receiver=sink;",
            "let receiver=sink;let unused=[evil(),sink.error];",
        ),
        (
            "let receiver=sink;",
            "let receiver=sink;let unused=other.error;",
        ),
    ] {
        let s = FORWARD.replace(from, to);
        assert!(!forward(&s), "{s}");
    }
    for (from, to) in [
        ("sink.error", "other.error"),
        ("pair[1]", "pair[0]"),
        ("pair[0],error", "pair[1],error"),
        ("[sink,sink.error]", "[sink,sink.error,0]"),
        ("[sink,sink.error]", "[sink,evil()]"),
        ("pair[1]", "pair['1']"),
        ("pair[1].call", "pair[1]?.call"),
    ] {
        let s = TUPLE.replace(from, to);
        assert!(!forward(&s), "{s}");
    }
}

// Minimal portable observer-store protocol; no captured program fixture.
const OBSERVER: &str = r#"function main(notify,logError){let values=[],statuses=[],report={scrBytes:{data:[],sidx:[]}};
function $THREAD_5_31572(index){let slot=index;
function $THREAD_4_3288(value){values[slot]=value;statuses[slot].hasCollected=true;notify({type:"collect",status:statuses[slot],value:value,signalIdx:slot});}
function $THREAD_5_95511(error){logError(slot,false,error);statuses[slot].hasErrored=true;notify({type:"error",status:statuses[slot],value:error,signalIdx:slot});}
function $THREAD_4_32367(){let value=values[slot];if(Array.isArray(value)&&value.length>=2){report.scrBytes.sidx.push(slot);let data=report.scrBytes.data;data.push([value[0].slice(0,8),value[1].slice(0,8)]);}}
let observer={};let result=observer;result['next']=$THREAD_4_3288;result['error']=$THREAD_5_95511;result['complete']=$THREAD_4_32367;return result;}
return $THREAD_5_31572;}main();"#;
#[test]
fn observer_factory_proves_alias_object_and_static_string_slots() {
    for s in [
        OBSERVER.to_string(),
        OBSERVER
            .replace("$THREAD_5_31572", "$THREAD_871_9823_2")
            .replace("slot", "$KEY_938")
            .replace("result", "facade"),
        OBSERVER.replace("let result=observer;", "let result;result=observer;"),
        OBSERVER.replace(
            "let observer={};let result=observer;",
            "let observer;observer={};let result=observer;",
        ),
        OBSERVER.replace(
            "let observer={};let result=observer;",
            "let observer={next:void 0,error:void 0,complete:void 0};let result=observer;",
        ),
    ] {
        assert!(factory(&s), "{s}");
    }
}
#[test]
fn observer_factory_refuses_mutated_aliases_unknown_slots_and_callback_roles() {
    for (from, to) in [
        ("let result=observer;", "let result=observer;result=other;"),
        ("let result=observer;", "var result=observer;var result;"),
        ("let result=observer;", "let {result}=observer;"),
        ("result['next']", "result[key]"),
        ("result['next']", "other['next']"),
        ("return result", "return other"),
        ("return result", "escape(result);return result"),
        ("return result", "let unused=result;return result"),
        ("return result", "result.extra=other;return result"),
        ("return result", "delete result.next;return result"),
        (
            "result['next']=$THREAD_4_3288;",
            "result['next']=$THREAD_4_3288;result['next']=$THREAD_4_3288;",
        ),
        (
            "result['error']=$THREAD_5_95511",
            "result['error']=$THREAD_4_3288",
        ),
        ("let slot=index;", "let slot=index;$THREAD_5_95511=other;"),
        ("(index){", "(index=0){"),
        ("(index){", "(...index){"),
    ] {
        let s = OBSERVER.replace(from, to);
        assert!(!factory(&s), "{s}");
    }
}
#[test]
fn human_names_and_collisions_remain_guarded() {
    let human = FORWARD.replace("$THREAD_7_192_2", "keepMyName");
    assert_eq!(name_threads(&human, true).unwrap().source, human);
    let collision = FORWARD.replace(
        "function $THREAD_7_192_2",
        "let forwardError=1;function $THREAD_7_192_2",
    );
    assert!(!forward(&collision));
    let collision = OBSERVER.replace(
        "let slot=index;",
        "let createSignalObserver=1;let slot=index;",
    );
    assert!(!factory(&collision));
}

#[test]
fn owned_callback_bodies_refuse_semantic_perturbations_and_shadowed_array() {
    for (from, to) in [
        ("values[slot]=value", "values[index]=value"),
        ("values[slot]=value", "values[slot]=other"),
        ("hasCollected=true", "hasCollected=false"),
        ("hasErrored=true", "hasCollected=true"),
        ("type:\"collect\"", "type:\"error\""),
        ("status:statuses[slot]", "status:values[slot]"),
        ("signalIdx:slot", "signalIdx:index"),
        ("logError(slot,false,error)", "logError(slot,true,error)"),
        ("logError(slot,false,error)", "logError(error,false,slot)"),
        ("logError(slot,false,error)", "logError(slot,false,error,0)"),
        ("notify({type:\"error\"", "logError({type:\"error\""),
        ("Array.isArray(value)", "Array.from(value)"),
        (
            "function main(notify,logError)",
            "function main(notify,logError,Array)",
        ),
        ("value.length>=2", "value.length>2"),
        (
            "report.scrBytes.sidx.push(slot)",
            "report.scrBytes.sidx.push(index)",
        ),
        ("report.scrBytes.data", "report.other.data"),
        ("slice(0,8)", "slice(0,7)"),
        ("value[1].slice", "value[0].slice"),
        (
            "function $THREAD_4_3288(value)",
            "function $THREAD_4_3288(value=0)",
        ),
        (
            "function $THREAD_5_95511(error)",
            "function $THREAD_5_95511(...error)",
        ),
        (
            "function $THREAD_5_95511(error)",
            "function $THREAD_5_95511({error})",
        ),
        (
            "function $THREAD_4_3288(value){",
            "function $THREAD_4_3288(value){return;",
        ),
        ("let slot=index;", "var slot=index;var slot;"),
        ("let slot=index;", "let slot=index;slot=other;"),
    ] {
        let s = OBSERVER.replace(from, to);
        assert!(!factory(&s), "{s}");
    }
}

#[test]
fn callback_and_result_aliases_require_sequential_single_origins() {
    let aliased = OBSERVER.replace(
        "result['next']=$THREAD_4_3288",
        "let callback=$THREAD_4_3288;result['next']=callback",
    );
    assert!(factory(&aliased));
    let assigned = aliased.replace(
        "let callback=$THREAD_4_3288;",
        "let callback;callback=$THREAD_4_3288;",
    );
    assert!(factory(&assigned));
    for (from, to) in [
        (
            "let callback=$THREAD_4_3288;",
            "let callback=$THREAD_4_3288;callback=other;",
        ),
        (
            "let callback=$THREAD_4_3288;",
            "var callback=$THREAD_4_3288;var callback;",
        ),
        (
            "let callback=$THREAD_4_3288;result['next']=callback",
            "result['next']=callback;let callback=$THREAD_4_3288",
        ),
        (
            "let callback=$THREAD_4_3288;",
            "let callback=later;let later=$THREAD_4_3288;",
        ),
    ] {
        let s = aliased.replace(from, to);
        assert!(!factory(&s), "{s}");
    }
    for s in [
        FORWARD.replace("method.call(receiver,error)", "method(error)"),
        FORWARD.replace("let receiver=sink;", "let receiver=later;let later=sink;"),
        TUPLE.replace("let pair=", "let pair=later;let later="),
    ] {
        assert!(!forward(&s), "{s}");
    }
}

const GUARDED: &str = "function main(sink){function deliver(tag,value){return value;}let invoke=deliver;function $THREAD_14_902(value){try{invoke(2,value);}catch(error){sink.error(error);}}return $THREAD_14_902;}main();";
fn guarded(source: &str, role: &str) -> bool {
    claimed(source, role, "shared_guarded_forwarder")
}
#[test]
fn guarded_forwarder_proves_tag_value_and_caught_error() {
    for source in [
        GUARDED.to_string(),
        GUARDED
            .replace("$THREAD_14_902", "$THREAD_98_723_2")
            .replace("value", "payload")
            .replace("sink", "observer")
            .replace("error", "failure")
            .replace(".failure(", ".error("),
        GUARDED.replace("invoke(2,value)", "invoke(8,value)"),
        GUARDED.replace("let invoke=deliver;", "let invoke;invoke=deliver;"),
        GUARDED
            .replace("let invoke=deliver;", "")
            .replace("invoke(2,value)", "deliver(2,value)"),
        GUARDED.replace("sink.error(error)", "sink['error'](error)"),
    ] {
        assert!(guarded(&source, "forwardTaggedValueOrError"), "{source}");
    }
    assert!(guarded(
        &GUARDED.replace("invoke(2,value)", "invoke(value)"),
        "forwardValueOrError"
    ));
}
#[test]
fn guarded_forwarder_refuses_near_misses_and_unproven_origins() {
    for (from, to) in [
        ("invoke(2,value)", "return invoke(2,value)"),
        ("invoke(2,value)", "invoke(value,2)"),
        ("invoke(2,value)", "invoke(2,other)"),
        ("invoke(2,value)", "invoke(2,value,0)"),
        ("invoke(2,value)", "invoke?.(2,value)"),
        ("invoke(2,value)", "invoke(...value)"),
        ("sink.error(error)", "sink.next(error)"),
        ("sink.error(error)", "sink.error(error);sink=other"),
        ("(value){try", "(value,sink){try"),
        ("function main(sink)", "function main()"),
        ("sink.error(error)", "sink.error(value)"),
        ("sink.error(error)", "sink.error(error,0)"),
        ("sink.error(error)", "return sink.error(error)"),
        (
            "sink.error(error);}",
            "sink.error(error);}finally{cleanup();}",
        ),
        ("let invoke=deliver;", "let invoke=deliver;invoke=other;"),
        ("let invoke=deliver;", "var invoke=deliver;var invoke;"),
        ("let invoke=deliver;", "let {invoke}=other;"),
        (
            "let invoke=deliver;",
            "let invoke=future;let future=deliver;",
        ),
        (
            "let invoke=deliver;",
            "let invoke;function setup(){invoke=deliver;}",
        ),
        ("function main(sink)", "function main(sink,invoke)"),
        ("(value){try", "(value,invoke){try"),
        ("catch(error){", "catch(error){error=other;"),
        (
            "return $THREAD_14_902;",
            "let forwardTaggedValueOrError=0;return $THREAD_14_902;",
        ),
    ] {
        let source = GUARDED.replace(from, to);
        assert!(!guarded(&source, "forwardTaggedValueOrError"), "{source}");
    }
    for source in [
        GUARDED
            .replace("function main(sink)", "function main()")
            .replace(
                "return $THREAD_14_902;",
                "const sink=other;return $THREAD_14_902;",
            ),
        GUARDED
            .replace("function main(sink)", "function main()")
            .replace(
                "let invoke=deliver;",
                "let sink;function setup(){sink=other;}let invoke=deliver;",
            ),
        GUARDED.replace(
            "let invoke=deliver;",
            "let invoke;for(invoke of [deliver]){}",
        ),
        GUARDED.replace(
            "let invoke=deliver;",
            "let invoke;try{fail();}catch(invoke){invoke=deliver;}",
        ),
    ] {
        assert!(!guarded(&source, "forwardTaggedValueOrError"), "{source}");
    }
    let human = GUARDED.replace("$THREAD_14_902", "handleInput");
    assert_eq!(name_threads(&human, true).unwrap().source, human);
}

#[test]
fn guarded_callable_origins_refuse_conditional_loop_and_resource_declarations() {
    for origin in [
        "var invoke;if(ready){invoke=deliver;}",
        "var invoke;for(;ready;){invoke=deliver;}",
        "if(ready){var invoke=deliver;}",
        "using invoke=deliver;",
    ] {
        let source = GUARDED.replace("let invoke=deliver;", origin);
        assert!(!guarded(&source, "forwardTaggedValueOrError"), "{source}");
    }
}
