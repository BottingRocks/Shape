use shape::naming::name_threads;

fn named(source: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result
            .report
            .outcomes
            .iter()
            .any(|outcome| outcome.applied && outcome.proposed_name.as_deref() == Some(name))
    })
}

const CLEAR: &str = r#"function main(){let queue=[],running=true,callbacks=[];
function $THREAD_7_101(){if(queue.length>0){queue.length=0;running=false;callbacks.forEach(item=>{setTimeout(item,0);});}}
return $THREAD_7_101;}main();"#;

const EMPTY: &str = r#"function main(){let callbacks=[],running=true,queue=[];
function $THREAD_7_102(item){callbacks.push(item);if(running&&queue.length===0){setTimeout(item,0);}}
return $THREAD_7_102;}main();"#;

const ERROR: &str = r#"function main(){let failed=false,pending=[],errorValue=0;
function $THREAD_7_103(item){let callback=item;if(failed){setTimeout(()=>{callback(errorValue);},0);}else{pending.push(callback);}}
return $THREAD_7_103;}main();"#;

const START: &str = r#"function main(){let running=false;function runner(){}
function $THREAD_7_104(){if(running){return void 0;}else{running=true;setTimeout(runner,0);return void 0;}}
return $THREAD_7_104;}main();"#;

#[test]
fn names_the_bounded_scheduler_family() {
    assert!(named(CLEAR, "clearQueuedTasks"));
    assert!(named(EMPTY, "notifyQueueEmpty"));
    assert!(named(ERROR, "notifyQueueError"));
    assert!(named(START, "startTaskQueue"));
}

#[test]
fn alpha_renaming_preserves_scoped_scheduler_proofs() {
    let clear = CLEAR
        .replace("$THREAD_7_101", "$THREAD_991_22001_3")
        .replace("queue", "$KEY_1")
        .replace("running", "$KEY_2")
        .replace("callbacks", "$KEY_3")
        .replace("item", "$KEY_4");
    assert!(named(&clear, "clearQueuedTasks"));

    let empty = EMPTY
        .replace("$THREAD_7_102", "$THREAD_991_22002_4")
        .replace("callbacks", "$KEY_5")
        .replace("running", "$KEY_6")
        .replace("queue", "$KEY_7")
        .replace("item", "$KEY_8");
    assert!(named(&empty, "notifyQueueEmpty"));
}

#[test]
fn clear_and_empty_reject_changed_operations_and_control_flow() {
    for changed in [
        CLEAR.replace("queue.length=0", "queue.length=1"),
        CLEAR.replace("running=false", "running=true"),
        CLEAR.replace("callbacks.forEach", "callbacks.map"),
        CLEAR.replace("setTimeout(item,0)", "setTimeout(item,1)"),
        CLEAR.replace("if(queue.length>0)", "if(queue.length>=0)"),
        CLEAR.replace("function $THREAD_7_101()", "async function $THREAD_7_101()"),
        EMPTY.replace("callbacks.push(item)", "queue.push(item)"),
        EMPTY.replace("running&&queue.length===0", "!running&&queue.length===0"),
        EMPTY.replace("setTimeout(item,0)", "setTimeout(other,0)"),
        EMPTY.replace(
            "if(running&&queue.length===0)",
            "if(running||queue.length===0)",
        ),
        EMPTY.replace("(item)", "(item=other)"),
    ] {
        assert!(!named(&changed, "clearQueuedTasks"));
        assert!(!named(&changed, "notifyQueueEmpty"));
    }
}

#[test]
fn error_and_start_reject_alias_mutation_and_bad_callbacks() {
    for changed in [
        ERROR.replace("pending.push(callback)", "pending.push(item)"),
        ERROR.replace("callback(errorValue)", "callback(item)"),
        ERROR.replace("setTimeout(()=>", "setTimeout(other=>"),
        ERROR.replace("if(failed)", "if(!failed)"),
        START.replace("running=true", "running=false"),
        START.replace("setTimeout(runner,0)", "setTimeout(other,0)"),
        START.replace("return void 0;}", "return false;}"),
    ] {
        assert!(!named(&changed, "notifyQueueError"));
        assert!(!named(&changed, "startTaskQueue"));
    }
    let parameter = CLEAR.replace("function main()", "function main(queue)");
    assert!(!named(&parameter, "clearQueuedTasks"));
    let shadowed = CLEAR.replace(
        "function $THREAD_7_101(){",
        "function $THREAD_7_101(){let setTimeout=other;",
    );
    assert!(!named(&shadowed, "clearQueuedTasks"));
}

#[test]
fn start_requires_an_ordered_captured_callable_origin() {
    for source in [
        START.replace(
            "function runner(){}",
            "var runner;if(enabled){runner=function(){};}",
        ),
        START.replace(
            "function runner(){}",
            "var runner;setup(function(){runner=function(){};});",
        ),
        START.replace(
            "function runner(){}",
            "var runner;for(const item of items){runner=function(){};}",
        ),
        START.replace("function runner(){}", "using runner=function(){};"),
        START.replace("function runner(){}", "var runner={};"),
    ] {
        assert!(!named(&source, "startTaskQueue"), "{source}");
    }
    let future = START.replace("function runner(){}", "").replace(
        "return $THREAD_7_104;",
        "var runner=function(){};return $THREAD_7_104;",
    );
    assert!(!named(&future, "startTaskQueue"));
    let assigned = START.replace("function runner(){}", "var runner;runner=function(){};");
    assert!(named(&assigned, "startTaskQueue"));
}

#[test]
fn readable_names_and_related_collisions_are_preserved() {
    let readable = CLEAR.replace("$THREAD_7_101", "clearQueue");
    let result = name_threads(&readable, true).unwrap();
    assert_eq!(result.source, readable);

    let collision = CLEAR.replace(
        "let queue=[],running=true,callbacks=[];",
        "let queue=[],running=true,callbacks=[];let clearQueuedTasks=1;",
    );
    assert!(!named(&collision, "clearQueuedTasks"));
}
