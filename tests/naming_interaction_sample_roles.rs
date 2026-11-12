use shape::naming::name_threads;

const FRAME: &str = r#"function main(){
let observer={next(value){},complete(){}};
function stream(key){return function(){return key;};}
function write(state,value,header){return 3;}
function $THREAD_4_100(input){
let seed=Math.random()*4294967296|0;
let header=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];
seed=stream(seed^123456);
let cursor=write(seed,input[0],header,3);
cursor=write(seed,input[1],header,cursor);
observer.next([[],header]);
observer.complete();
}
return $THREAD_4_100;}main();"#;

const TOUCH: &str = r#"function main(){
let store={put(value){}};
let onError={error(value){}};
let flag=false;
function clock(){return 1;}
function $THREAD_0_77(input){
try{
let state={instanceOfUIEvent:input instanceof UIEvent,markedAsTrusted:!!input.isTrusted};
let list=[];
let touches=input[input.touches&&input.touches.length===0?"changedTouches":"touches"];
if(touches&&touches.length!==0){
let count=touches.length;
let i=0;
for(;i<count;i++){
let point=touches[i];
let target=point.target;
target=target instanceof Element?target.getBoundingClientRect():null;
const dx=target==null?0:Math.round(point.clientX-target.left);
target=target==null?0:Math.round(point.clientY-target.top);
list.push({a:Math.round(point.pageX),b:Math.round(point.pageY),c:dx,d:target,e:point.force});
}
}
store.put({a:[true,false],b:clock(),c:list,d:state.instanceOfUIEvent,e:state.markedAsTrusted});
flag=true;
}catch(error){
onError.error(error);
}
}
return $THREAD_0_77;}main();"#;

const ANIMATION: &str = r#"function main(){
let samples=[];
let interval=16;
let active=false;
let host={requestAnimationFrame(fn){}};
let next=function(){};
function $THREAD_2_88(delta){
samples.push(Math.round(delta/interval));
active=true;
if(samples.length<20){host.requestAnimationFrame(next);}
}
return $THREAD_2_88;}main();"#;

const SELECTOR: &str = r#"function main(){
let counter=0;
let selectors=[".a",".b"];
let reporter={error(value){}};
function $THREAD_3_99(event){
try{
if(event.target){
if(event.target.closest(selectors.join(","))==null){
++counter;
}
}
}catch(error){
reporter.error(error);
}
}
return $THREAD_3_99;}main();"#;

const TRAVEL: &str = r#"function main(){
let lastX=0;
let lastY=0;
let total=0;
let math=Math;
let reporter={error(value){}};
function $THREAD_4_111(event){
try{
let distance=event.clientY-lastY;
distance=math.sqrt(math.pow(event.clientX-lastX,2)+math.pow(distance,2));
lastX=event.clientX;
lastY=event.clientY;
total=total+(isNaN(distance)?0:distance);
}catch(error){
reporter.error(error);
}
}
return $THREAD_4_111;}main();"#;

fn expected_evaluators(name: &str) -> &'static [&'static str] {
    match name {
        "emitSeededSignalFrame" => &["signals_seeded_signal_frame"],
        "recordTouchSample" => &["events_touch_sample"],
        "recordAnimationFrameSample" => &["graphics_animation_frame_sample"],
        "countEventsOutsideSelector" => &["events_selector_event_count"],
        "accumulatePointerTravelDistance" => &["events_pointer_travel_distance"],
        _ => &[],
    }
}

fn named(source: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report.function_roles.iter().any(|role| {
            role.candidates.iter().any(|candidate| candidate == name)
                && role
                    .evaluators
                    .iter()
                    .any(|evaluator| expected_evaluators(name).contains(&evaluator.as_str()))
                && r.report
                    .outcomes
                    .iter()
                    .any(|o| o.applied && o.proposed_name.as_deref() == Some(name))
        })
    })
}

fn replay_matches(source: &str) {
    let r = name_threads(source, true).unwrap();
    let mut replay = source.to_owned();
    let mut edits = r.report.edits.clone();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    for edit in edits {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, r.source);
}

fn apply(source: &str) -> String {
    name_threads(source, true).unwrap().source
}

#[test]
fn seeded_frame_proves_header_stream_writer_and_observer() {
    for source in [
        FRAME.to_owned(),
        FRAME
            .replace("$THREAD_4_100", "$THREAD_71_887766_2")
            .replace("input", "$KEY_5")
            .replace("seed", "$KEY_61")
            .replace("header", "$KEY_62")
            .replace("cursor", "$KEY_63")
            .replace("observer", "$KEY_64")
            .replace("stream", "$KEY_65")
            .replace("write", "$KEY_66"),
        FRAME.replace(
            "cursor=write(seed,input[1],header,cursor);",
            "write(seed,input[1],header,cursor);",
        ),
        FRAME
            .replace(
                "function stream(key){return function(){return key;};}",
                "function stream(key){return function(){return key;};}\nlet streamAlias=stream;",
            )
            .replace(
                "function write(state,value,header){return 3;}",
                "function write(state,value,header){return 3;}\nlet writeAlias;writeAlias=write;",
            )
            .replace("seed=stream(", "seed=streamAlias(")
            .replace("write(seed,", "writeAlias(seed,"),
    ] {
        assert!(named(&source, "emitSeededSignalFrame"), "{source}");
        replay_matches(&source);
    }
}

#[test]
fn seeded_frame_uses_a_declared_stream_state() {
    let source = FRAME.replace(
        "seed=stream(seed^123456);",
        "let streamState=stream(seed^123456);",
    );
    let source = source.replace("write(seed,", "write(streamState,");
    assert!(named(&source, "emitSeededSignalFrame"), "{source}");
}

#[test]
fn seeded_frame_refuses_changed_bytes_stream_writer_observer_and_setup() {
    for (from, to) in [
        ("seed>>>24", "seed>>>16"),
        ("seed>>16&255", "seed>>15&255"),
        ("seed>>8&255", "seed>>9&255"),
        ("seed&255", "seed&127"),
        ("Math.random()*4294967296|0", "Date.now()|0"),
        ("seed^123456", "seed+123456"),
        ("stream(seed^123456)", "stream(seed)"),
        (
            "write(seed,input[0],header,3)",
            "write(input[0],seed,header,3)",
        ),
        (
            "write(seed,input[1],header,cursor)",
            "write(seed,input[1],other,cursor)",
        ),
        (
            "write(seed,input[1],header,cursor)",
            "write(seed,input[1],header,3)",
        ),
        ("observer.next([[],header])", "observer.next([[header],[]])"),
        ("observer.next([[],header])", "observer.send([[],header])"),
        ("observer.complete()", "observer.done()"),
        ("observer.next([[],header]);", "other.next([[],header]);"),
        ("header]", "other]"),
        ("seed=stream", "seed=other"),
        (
            "cursor=write(seed,input[1],header,cursor);",
            "cursor=write(seed,input[1],header,cursor);if(input){return;}",
        ),
        (
            "let cursor=write(seed,input[0],header,3);",
            "var cursor=write(seed,input[0],header,3);var cursor;",
        ),
        (
            "let seed=Math.random()*4294967296|0;",
            "var seed=Math.random()*4294967296|0;var seed;",
        ),
    ] {
        let source = FRAME.replace(from, to);
        assert!(
            !named(&source, "emitSeededSignalFrame"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn touch_samples_prove_selection_loop_record_and_guarded_put() {
    for source in [
        TOUCH.to_owned(),
        TOUCH
            .replace("$THREAD_0_77", "$THREAD_12_345_3")
            .replace("input", "$KEY_3")
            .replace("point", "$KEY_4")
            .replace("list", "$KEY_6")
            .replace("state", "$KEY_7")
            .replace("store", "$KEY_8")
            .replace("onError", "$KEY_9")
            .replace("flag", "$KEY_10")
            .replace("dx", "$DX"),
        TOUCH.replace(
            "function clock(){return 1;}",
            "function makeClock(){return function(){return 1;};}\nlet clock=makeClock();",
        ),
    ] {
        assert!(named(&source, "recordTouchSample"), "{source}");
        replay_matches(&source);
    }
}

#[test]
fn touch_samples_refuse_changed_touch_record_loop_and_catch() {
    for (from, to) in [
        ("\"changedTouches\"", "\"touches\""),
        ("UIEvent", "CustomEvent"),
        ("input.isTrusted", "input.trusted"),
        ("input.touches&&input.touches.length===0", "input.touches"),
        ("touches.length!==0", "touches.length===0"),
        ("touches[i]", "touches[0]"),
        ("point.clientX-target.left", "point.clientX-target.right"),
        ("point.clientY-target.top", "point.clientY-target.bottom"),
        ("target instanceof Element", "target instanceof Node"),
        ("getBoundingClientRect", "getClientRects"),
        ("e:point.force", "e:point.pressure"),
        ("a:Math.round(point.pageX)", "a:Math.round(point.screenX)"),
        ("list.push(", "other.push("),
        ("c:dx", "c:point"),
        ("a:[true,false]", "a:[true,false,true]"),
        ("c:list", "c:point"),
        ("d:state.instanceOfUIEvent", "d:state.instanceOfTouch"),
        ("flag=true", "flag=false"),
        ("onError.error(error)", "onError.notify(error)"),
        ("++", "--"),
        ("let list=[];", "let list=[0];"),
        ("let state={", "var state=make();var state={"),
        ("input){", "input){input=other;"),
    ] {
        let source = TOUCH.replace(from, to);
        assert!(
            !named(&source, "recordTouchSample"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn touch_samples_refuse_runtime_origins() {
    for source in [
        TOUCH
            .replace("function main(){", "function main(store){")
            .replace("let store={put(value){}};", ""),
        TOUCH.replace("let store={put(value){}};", "let {store}=provider;"),
        TOUCH.replace("let flag=false;", "let [flag]=provider;"),
        TOUCH
            .replace("function main(){", "function main(clock){")
            .replace("function clock(){return 1;}", ""),
    ] {
        assert!(!named(&source, "recordTouchSample"), "{source}");
    }
}

#[test]
fn animation_frame_sample_proves_flag_and_reschedule() {
    for source in [
        ANIMATION.to_owned(),
        ANIMATION
            .replace("$THREAD_2_88", "$THREAD_8_551_4")
            .replace("delta", "$KEY_7")
            .replace("samples", "$KEY_8")
            .replace("active", "$KEY_9")
            .replace("host", "$KEY_10")
            .replace("next", "$KEY_11"),
    ] {
        assert!(named(&source, "recordAnimationFrameSample"), "{source}");
        replay_matches(&source);
    }
}

#[test]
fn animation_frame_sample_refuses_changed_ratio_flag_and_scheduler() {
    for (from, to) in [
        ("Math.round(delta/interval)", "Math.round(delta*interval)"),
        ("delta/interval", "delta/other"),
        ("samples.push", "other.push"),
        ("samples.length<20", "samples.length<=20"),
        ("samples.length<20", "samples.length<19"),
        ("host.requestAnimationFrame(next)", "host.setTimeout(next)"),
        (
            "host.requestAnimationFrame(next)",
            "host.requestAnimationFrame(other)",
        ),
        ("active=true", "active=false"),
        ("let interval=16;", "var interval=16;var interval;"),
        ("delta){", "delta=0){"),
    ] {
        let source = ANIMATION.replace(from, to);
        assert!(
            !named(&source, "recordAnimationFrameSample"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn selector_counter_proves_target_closest_and_guard() {
    for source in [
        SELECTOR.to_owned(),
        SELECTOR
            .replace("$THREAD_3_99", "$THREAD_9_1201_2")
            .replace("event", "$KEY_3")
            .replace("counter", "$KEY_4")
            .replace("selectors", "$KEY_5")
            .replace("reporter", "$KEY_6"),
    ] {
        assert!(named(&source, "countEventsOutsideSelector"), "{source}");
        replay_matches(&source);
    }
}

#[test]
fn selector_counter_refuses_changed_guard_join_and_target() {
    for (from, to) in [
        ("closest", "matches"),
        ("selectors.join(\",\")", "selectors.join(\";\")"),
        ("==null", "===null"),
        ("event.target", "event.currentTarget"),
        ("++counter", "--counter"),
        ("++counter", "counter=1"),
        ("reporter.error(error)", "reporter.log(error)"),
        ("if(event.target){", "if(true){"),
        ("reporter.error(error)", "reporter.error(error);log(error);"),
    ] {
        let source = SELECTOR.replace(from, to);
        assert!(
            !named(&source, "countEventsOutsideSelector"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn pointer_travel_proves_distance_totals_and_guard() {
    for source in [
        TRAVEL.to_owned(),
        TRAVEL
            .replace("$THREAD_4_111", "$THREAD_5_908_3")
            .replace("event", "$KEY_3")
            .replace("distance", "$KEY_4")
            .replace("lastX", "$KEY_5")
            .replace("lastY", "$KEY_6")
            .replace("total", "$KEY_7")
            .replace("math", "$KEY_8")
            .replace("reporter", "$KEY_9"),
    ] {
        assert!(
            named(&source, "accumulatePointerTravelDistance"),
            "{source}"
        );
        replay_matches(&source);
    }
}

#[test]
fn pointer_travel_refuses_changed_math_order_and_accumulation() {
    for (from, to) in [
        ("math.sqrt", "math.cbrt"),
        ("math.pow", "math.max"),
        ("clientX-lastX", "clientX-lastY"),
        ("clientY-lastY", "clientY-lastX"),
        ("clientX", "screenX"),
        ("lastX=event.clientX", "lastX=event.pageX"),
        ("isNaN(distance)", "Number.isNaN(distance)"),
        ("?0:distance", "?1:distance"),
        ("total+(isNaN", "other+(isNaN"),
        ("event){", "event=null){"),
        (
            "let distance=event.clientY-lastY;",
            "let distance=event.clientY-lastY;if(distance){return;}",
        ),
    ] {
        let source = TRAVEL.replace(from, to);
        assert!(
            !named(&source, "accumulatePointerTravelDistance"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn rules_refuse_future_alias_and_shadowed_builtins() {
    let future_alias = FRAME
        .replace("function stream(key){return function(){return key;};}", "")
        .replace("seed=stream(", "seed=streamAlias(")
        .replace(
            "return $THREAD_4_100;}main();",
            "let streamAlias=function(key){return function(){return key;};};\nreturn $THREAD_4_100;}main();",
        );
    assert!(
        !named(&future_alias, "emitSeededSignalFrame"),
        "{future_alias}"
    );

    let shadowed_math = FRAME.replace(
        "function main(){",
        "function main(){\nlet Math={random(){return 0;},round(value){return value;}};",
    );
    assert!(
        !named(&shadowed_math, "emitSeededSignalFrame"),
        "{shadowed_math}"
    );

    let shadowed_is_nan = TRAVEL.replace(
        "function main(){",
        "function main(){\nlet isNaN=function(value){return false;};",
    );
    assert!(
        !named(&shadowed_is_nan, "accumulatePointerTravelDistance"),
        "{shadowed_is_nan}"
    );
}

#[test]
fn frame_writers_refuse_future_intermediate_conditional_loop_resource_and_setup() {
    // An intermediate alias copied before that intermediate binding receives
    // its function value is not an initialized callable at the alias hop.
    let future_intermediate = FRAME
        .replace(
            "function stream(key){return function(){return key;};}",
            "let streamAlias=streamLater;",
        )
        .replace("seed=stream(", "seed=streamAlias(")
        .replace(
            "return $THREAD_4_100;}main();",
            "let streamLater=function(key){return function(){return key;};};\nreturn $THREAD_4_100;}main();",
        );
    assert!(
        !named(&future_intermediate, "emitSeededSignalFrame"),
        "{future_intermediate}"
    );

    let future_writer = FRAME
        .replace(
            "function write(state,value,header){return 3;}",
            "let write=laterWrite;",
        )
        .replace(
            "return $THREAD_4_100;}main();",
            "let laterWrite=function(state,value,header){return 3;};\nreturn $THREAD_4_100;}main();",
        );
    assert!(
        !named(&future_writer, "emitSeededSignalFrame"),
        "{future_writer}"
    );

    for origin in [
        "var write;if(ready){write=function(state,value,header){return 3;};}",
        "var write;while(ready){write=function(state,value,header){return 3;};break;}",
        "var write;setup(function(){write=function(state,value,header){return 3;};});",
        "using write=function(state,value,header){return 3;};",
    ] {
        let source = FRAME
            .replace("function main(){", "function main(){\nlet ready=true;")
            .replace("function write(state,value,header){return 3;}", origin);
        assert!(
            !named(&source, "emitSeededSignalFrame"),
            "accepted {origin}\n{source}"
        );
    }
}

#[test]
fn touch_samples_refuse_unproved_clock_origins() {
    for clock in [
        "let clock=0;",
        "let clock=provider.time;",
        "var clock;if(ready){clock=function(){return 1;};}",
        "var clock;while(ready){clock=function(){return 1;};break;}",
        "var clock;setup(function(){clock=function(){return 1;};});",
        "using clock=function(){return 1;};",
        "function makeClock(){sideEffect();return function(){return 1;};}\nlet clock=makeClock();",
        "function makeClock(){return provider.clock;}\nlet clock=makeClock();",
        "var makeClock;if(ready){makeClock=function(){return function(){return 1;};};}\nlet clock=makeClock();",
    ] {
        let source = TOUCH
            .replace("function main(){", "function main(){\nlet ready=true;")
            .replace("function clock(){return 1;}", clock);
        assert!(
            !named(&source, "recordTouchSample"),
            "accepted {clock}\n{source}"
        );
    }

    let future_intermediate = TOUCH
        .replace("function clock(){return 1;}", "let clock=laterClock;")
        .replace(
            "return $THREAD_0_77;}main();",
            "let laterClock=function(){return 1;};\nreturn $THREAD_0_77;}main();",
        );
    assert!(
        !named(&future_intermediate, "recordTouchSample"),
        "{future_intermediate}"
    );
}

#[test]
fn meaningful_names_are_preserved_and_readable_bindings_are_untouched() {
    for (source, name) in [
        (FRAME, "emitSeededSignalFrame"),
        (TOUCH, "recordTouchSample"),
        (ANIMATION, "recordAnimationFrameSample"),
        (SELECTOR, "countEventsOutsideSelector"),
        (TRAVEL, "accumulatePointerTravelDistance"),
    ] {
        let human = source.replace("function $THREAD_", "function humanHelper");
        assert_eq!(apply(&human), human, "{name}");
    }
}

#[test]
fn same_role_suffixed_clone_gets_a_distinct_ordinal() {
    let source = FRAME.replace(
        "return $THREAD_4_100;}main();",
        "function $THREAD_4_101_2(input){\n\
let seed=Math.random()*4294967296|0;\n\
let header=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];\n\
seed=stream(seed^654321);\n\
let cursor=write(seed,input[0],header,3);\n\
write(seed,input[1],header,cursor);\n\
observer.next([[],header]);\n\
observer.complete();\n\
}\n\
return $THREAD_4_100;}main();",
    );
    let report = name_threads(&source, true).unwrap();
    let base = report
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.identifier.start == source.find("$THREAD_4_100").unwrap() as u32)
        .expect("base outcome");
    assert!(base.applied);
    assert_eq!(base.proposed_name.as_deref(), Some("emitSeededSignalFrame"));
    let clone = report
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.identifier.start == source.find("$THREAD_4_101_2").unwrap() as u32)
        .expect("clone outcome");
    assert!(clone.applied);
    assert_eq!(
        clone.proposed_name.as_deref(),
        Some("emitSeededSignalFrame2")
    );
    assert_eq!(clone.role_name.as_deref(), Some("emitSeededSignalFrame"));
}
