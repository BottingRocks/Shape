use shape::naming::name_threads;

const EVENT: &str = r#"function main(){let selectors=["INPUT","BUTTON"];let outside=0;let observer={error(e){}};function $THREAD_2_49889(event){try{if(event.target){if(event.target.closest(selectors.join(","))==null){++outside;}}}catch(e){observer.error(e);}}return $THREAD_2_49889;}main();"#;

const POINTER: &str = r#"function main(){let lastX=0;let lastY=0;let total=0;let math=Math;let observer={error(e){}};function $THREAD_3_52081(event){try{let delta=event.clientY-lastY;delta=math.sqrt(Math.pow(event.clientX-lastX,2)+Math.pow(delta,2));lastX=event.clientX;lastY=event.clientY;total=total+(isNaN(delta)?0:delta);}catch(e){observer.error(e);}}return $THREAD_3_52081;}main();"#;

const FRAME: &str = r#"function main(){let samples=[];let divisor=2;let recording=false;let scheduler={requestAnimationFrame(f){}};let callback=$THREAD_6_164111;function $THREAD_6_164111(timestamp){samples.push(Math.round(timestamp/divisor));recording=true;if(samples.length<20){scheduler.requestAnimationFrame(callback);}}return $THREAD_6_164111;}main();"#;

fn named(source: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report
            .outcomes
            .iter()
            .any(|o| o.applied && o.proposed_name.as_deref() == Some(name))
    })
}

fn selector_graphics_evaluators(name: &str) -> &'static [&'static str] {
    match name {
        "countEventsOutsideSelector" => &["events_selector_exclusion_count"],
        "accumulatePointerTravelDistance" => &["events_pointer_travel_accumulation"],
        "recordAnimationFrameSample" => &["graphics_scaled_animation_frame_sample"],
        "recordPixelVectorSample" => &["graphics_unique_vector_history"],
        _ => &[],
    }
}

/// True only when this evaluator itself produces a claim for name, even when an overlapping
/// rule wins ambiguity resolution. Keep the test scoped to its evaluator.
fn selector_graphics_claims(source: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report.function_roles.iter().any(|role| {
            role.candidates.iter().any(|candidate| candidate == name)
                && role
                    .evaluators
                    .iter()
                    .any(|evaluator| selector_graphics_evaluators(name).contains(&evaluator.as_str()))
        })
    })
}

fn replay(source: &str) -> String {
    let r = name_threads(source, true).unwrap();
    let mut text = source.to_owned();
    let mut edits: Vec<_> = r.report.edits.iter().collect();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    for edit in edits {
        text.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(text, r.source);
    text
}

// ---------------------------------------------------------------------------
// countEventsOutsideSelector
// ---------------------------------------------------------------------------

#[test]
fn selector_accounting_proves_guarded_closest_and_catch_forwarding() {
    for source in [
        EVENT.to_owned(),
        EVENT
            .replace("$THREAD_2_49889", "$THREAD_88_7612_3")
            .replace("selectors", "$KEY_421")
            .replace("outside", "missed")
            .replace("observer", "$KEY_5")
            .replace("event", "$KEY_7")
            .replace("catch(e)", "catch(error)")
            .replace("(e)", "(error)"),
    ] {
        assert!(
            selector_graphics_claims(&source, "countEventsOutsideSelector"),
            "{source}"
        );
        replay(&source);
    }
    let postfix = EVENT.replace("++outside;", "outside++;");
    assert!(selector_graphics_claims(&postfix, "countEventsOutsideSelector"));
}

#[test]
fn selector_accounting_refuses_changed_operations_and_origins() {
    for (from, to) in [
        ("==null", "===null"),
        ("closest", "matches"),
        ("selectors.join(\",\")", "selectors.join()"),
        ("selectors.join(\",\")", "selectors.join(\"\")"),
        ("selectors.join(\",\")", "selectors.concat(\",\")"),
        ("++outside", "--outside"),
        ("observer.error(e)", "observer.error(other)"),
        ("observer.error(e)", "observer.report(e)"),
        ("event.target.closest", "event.currentTarget.closest"),
        ("catch(e)", "catch({message})"),
        ("catch(e){observer.error(e);}", "catch(e){}"),
        ("try{", "try{sideEffect();"),
        ("if(event.target){", "if(1){"),
        ("let observer=", "let observer=null;observer="),
        (
            "let selectors=[\"INPUT\",\"BUTTON\"];",
            "let selectors=[\"INPUT\",\"BUTTON\"];selectors=[];",
        ),
        (
            "let selectors=[\"INPUT\",\"BUTTON\"];",
            "var selectors=[\"INPUT\",\"BUTTON\"];var selectors;",
        ),
        (
            "let selectors=[\"INPUT\",\"BUTTON\"];",
            "let selectors;if(flag){selectors=[\"INPUT\"];}",
        ),
        (
            "let selectors=[\"INPUT\",\"BUTTON\"];",
            "let selectors=[];for(;;){selectors=[\"INPUT\"];break;}",
        ),
        (
            "function main(){let selectors",
            "function main(selectors){let unused",
        ),
        (
            "function $THREAD_2_49889(event){",
            "function $THREAD_2_49889(event){let selectors=[];",
        ),
    ] {
        let source = EVENT.replace(from, to);
        assert!(
            !selector_graphics_claims(&source, "countEventsOutsideSelector"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn selector_accounting_refuses_future_initialization() {
    let source = r#"function main(){let selectors;let outside=0;let observer={error(e){}};function $THREAD_2_49889(event){try{if(event.target){if(event.target.closest(selectors.join(","))==null){++outside;}}}catch(e){observer.error(e);}}selectors=["INPUT"];
return $THREAD_2_49889;}main();"#;
    assert!(!selector_graphics_claims(source, "countEventsOutsideSelector"));
}

// ---------------------------------------------------------------------------
// accumulatePointerTravelDistance
// ---------------------------------------------------------------------------

#[test]
fn pointer_distance_proves_ordered_delta_and_total() {
    for source in [
        POINTER.to_owned(),
        POINTER
            .replace("$THREAD_3_52081", "$THREAD_55_9912_4")
            .replace("lastX", "$KEY_8")
            .replace("lastY", "$KEY_9")
            .replace("total", "$KEY_10")
            .replace("math", "$KEY_19")
            .replace("observer", "$KEY_4")
            .replace("event", "$KEY_7")
            .replace("delta", "$KEY_3")
            .replace("catch(e)", "catch(error)")
            .replace("(e)", "(error)"),
    ] {
        assert!(
            selector_graphics_claims(&source, "accumulatePointerTravelDistance"),
            "{source}"
        );
        replay(&source);
    }
}

#[test]
fn pointer_distance_refuses_changed_fields_math_and_writes() {
    for (from, to) in [
        ("event.clientY-lastY", "event.pageY-lastY"),
        ("event.clientX-lastX", "event.pageX-lastX"),
        ("math.sqrt", "math.hypot"),
        (
            "Math.pow(event.clientX-lastX,2)",
            "Math.pow(event.clientX-lastX,3)",
        ),
        ("Math.pow(delta,2)", "Math.pow(delta,1)"),
        ("lastX=event.clientX", "lastX=event.clientY"),
        ("lastY=event.clientY", "lastY=event.clientX"),
        ("total=total+", "total=other+"),
        ("(isNaN(delta)?0:delta)", "(Number.isNaN(delta)?0:delta)"),
        ("(isNaN(delta)?0:delta)", "(isNaN(delta)?1:delta)"),
        ("(isNaN(delta)?0:delta)", "(isNaN(other)?0:delta)"),
        ("observer.error(e)", "observer.report(e)"),
        ("catch(e){observer.error(e);}", "catch(e){}"),
        ("let math=Math;", "let math=Math;math=other;"),
        ("let math=Math;", "var math=Math;var math;"),
        (
            "function main(){let lastX=0;",
            "function main(lastX){let unused0;",
        ),
        (
            "function main(){let lastX=0;",
            "function main(){let lastX;if(flag){lastX=0;}",
        ),
        ("try{", "try{sideEffect();"),
    ] {
        let source = POINTER.replace(from, to);
        assert!(
            !selector_graphics_claims(&source, "accumulatePointerTravelDistance"),
            "accepted {from} -> {to}"
        );
    }
}

// ---------------------------------------------------------------------------
// recordAnimationFrameSample
// ---------------------------------------------------------------------------

#[test]
fn animation_sampling_proves_rounding_limit_and_self_rescheduling() {
    for source in [
        FRAME.to_owned(),
        FRAME
            .replace("$THREAD_6_164111", "$THREAD_901_83_5")
            .replace("samples", "$KEY_13")
            .replace("divisor", "$KEY_2")
            .replace("recording", "$KEY_16")
            .replace("scheduler", "$KEY_7")
            .replace("callback", "$KEY_20")
            .replace("timestamp", "$KEY_CE")
            .replace("<20", "<7"),
    ] {
        assert!(
            selector_graphics_claims(&source, "recordAnimationFrameSample"),
            "{source}"
        );
        replay(&source);
    }
    let direct = FRAME.replace("let callback=$THREAD_6_164111;", "").replace(
        "requestAnimationFrame(callback)",
        "requestAnimationFrame($THREAD_6_164111)",
    );
    assert!(selector_graphics_claims(&direct, "recordAnimationFrameSample"));
}

#[test]
fn animation_sampling_refuses_changed_api_arguments_and_origins() {
    for (from, to) in [
        ("Math.round", "Math.floor"),
        ("timestamp/divisor", "timestamp*divisor"),
        ("samples.length<20", "samples.length<=20"),
        ("requestAnimationFrame", "setTimeout"),
        ("recording=true", "recording=false"),
        ("samples.push", "other.push"),
        (
            "requestAnimationFrame(callback)",
            "requestAnimationFrame(otherCallback)",
        ),
        (
            "requestAnimationFrame(callback)",
            "requestAnimationFrame(function(){})",
        ),
        ("let divisor=2;", "let divisor=2;divisor=3;"),
        ("let samples=[];", "let samples=[];samples=[];"),
        ("let scheduler=", "let scheduler=null;scheduler="),
        ("let samples=[];", "var samples=[];var samples;"),
        (
            "function main(){let samples=[];",
            "function main(samples){let unused;",
        ),
        (
            "function main(){let samples=[];",
            "function main(){let samples;if(flag){samples=[];}",
        ),
        ("if(samples.length<20)", "if(samples.length<20&&flag)"),
    ] {
        let source = FRAME.replace(from, to);
        assert!(
            !selector_graphics_claims(&source, "recordAnimationFrameSample"),
            "accepted {from} -> {to}"
        );
    }
    let other_array = FRAME
        .replace("let samples=[];", "let samples=[];let other=[];")
        .replace("samples.push", "other.push");
    assert!(
        !selector_graphics_claims(&other_array, "recordAnimationFrameSample"),
        "accepted samples.push -> other.push"
    );
}

#[test]
fn anchor_selector_graphics_preserves_human_names_and_collisions() {
    for source in [
        EVENT.replace("$THREAD_2_49889", "countOutsideClicks"),
        POINTER.replace("$THREAD_3_52081", "trackPointerTravel"),
        FRAME.replace("$THREAD_6_164111", "sampleAnimationFrame"),
    ] {
        assert_eq!(name_threads(&source, true).unwrap().source, source);
    }
    for (source, name) in [
        (EVENT, "countEventsOutsideSelector"),
        (POINTER, "accumulatePointerTravelDistance"),
        (FRAME, "recordAnimationFrameSample"),
    ] {
        let collision = source.replace(
            "main(){",
            &format!("main(){{let {name}=17;useValue({name});"),
        );
        assert!(!named(&collision, name), "collision accepted: {name}");
    }
}

// ---------------------------------------------------------------------------
// recordPixelVectorSample
// ---------------------------------------------------------------------------

const UNIQUE: &str = r#"function main(){function $THREAD_3_44049(record,state){let copy=record;if(state.a.some(stored=>{if(copy==null||stored==null||copy.length!==stored.length){return false;}else{let i=0;let equal;for(;;){equal=void 0;if(i>=copy.length){equal=true;break;}if(copy[i]===stored[i]){++i;continue;}equal=false;break;}return equal;}})){return void 0;}state.b=Number(state.b)+1;if(state.a.length>5){return void 0;}else{let saved=Array.prototype.slice.call(copy);state.a.push(saved);return void 0;}}return $THREAD_3_44049;}main();"#;

#[test]
fn unique_sample_proves_dedup_counter_cap_and_copy() {
    for source in [
        UNIQUE.to_owned(),
        UNIQUE
            .replace("$THREAD_3_44049", "$THREAD_72_551_6")
            .replace("record", "$KEY_9")
            .replace("state", "$KEY_11")
            .replace("copy", "$KEY_0")
            .replace("stored", "$KEY_BS")
            .replace("saved", "$KEY_BR")
            .replace("equal", "$KEY_BU")
            .replace("let i=0", "let $KEY_BT=0")
            .replace("[i]", "[$KEY_BT]")
            .replace("i>=", "$KEY_BT>=")
            .replace("++i", "++$KEY_BT")
            .replace(">5", ">9"),
    ] {
        assert!(selector_graphics_claims(&source, "recordPixelVectorSample"), "{source}");
        replay(&source);
    }
    let postfix = UNIQUE.replace("++i", "i++");
    assert!(selector_graphics_claims(&postfix, "recordPixelVectorSample"));
    let renamed_slots = UNIQUE
        .replace("state.a", "state.items")
        .replace("state.b", "state.total");
    assert!(selector_graphics_claims(&renamed_slots, "recordPixelVectorSample"));
}

#[test]
fn unique_sample_refuses_changed_equality_counter_cap_and_copy() {
    for (from, to) in [
        ("state.a.some", "state.a.filter"),
        ("copy[i]===stored[i]", "copy[i]==stored[i]"),
        ("copy.length!==stored.length", "copy.length===stored.length"),
        ("copy==null", "copy!=null"),
        ("state.b=Number(state.b)+1", "state.b=Number(state.b)+2"),
        ("state.a.length>5", "state.a.length>=5"),
        ("state.a.length>5", "state.b.length>5"),
        (
            "Array.prototype.slice.call(copy)",
            "Array.prototype.slice.apply(copy)",
        ),
        ("Array.prototype.slice.call(copy)", "array.slice.call(copy)"),
        ("state.a.push(saved)", "state.a.push(copy)"),
        ("state.a.push(saved)", "state.b.push(saved)"),
        ("state.a.some", "state.other.some"),
        ("state.b=Number(state.b)+1", "state.a=Number(state.a)+1"),
        ("record,state", "record,state=0"),
        ("record,state", "record,{state}"),
        ("stored=>", "(stored=0)=>"),
        ("copy[i]", "copy[0]"),
        ("stored[i]", "stored[0]"),
        ("let copy=record;", "let copy=record;copy=other;"),
        ("if(state.a.length>5){return void 0;}else{", ""),
        (
            "return void 0;}else{let saved",
            "return void 0;}else{let saved0",
        ),
        (
            "function main(){function $THREAD_3_44049",
            "function main(){let Array=custom;function $THREAD_3_44049",
        ),
    ] {
        let source = UNIQUE.replace(from, to);
        assert!(
            !selector_graphics_claims(&source, "recordPixelVectorSample"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn unique_sample_preserves_existing_name_and_collisions() {
    let human = UNIQUE.replace("$THREAD_3_44049", "rememberUniqueSample");
    assert_eq!(name_threads(&human, true).unwrap().source, human);
    let collision = UNIQUE.replace(
        "main(){",
        "main(){let recordPixelVectorSample=17;useValue(recordPixelVectorSample);",
    );
    assert!(!named(&collision, "recordPixelVectorSample"));
}

// The cap alternate may inline the slice copy in the push argument instead of
// hoisting it into a plain local. Both spellings are the same unique-vector
// recorder: same formal alias, same element-wise callback, same counter, same
// numeric cap and the same void-0 returns.
const UNIQUE_INLINE: &str = r#"function main(){function $THREAD_3_44049(record,state){let copy=record;if(state.a.some(stored=>{if(copy==null||stored==null||copy.length!==stored.length){return false;}else{let i=0;let equal;for(;;){equal=void 0;if(i>=copy.length){equal=true;break;}if(copy[i]===stored[i]){++i;continue;}equal=false;break;}return equal;}})){return void 0;}state.b=Number(state.b)+1;if(state.a.length>5){return void 0;}else{state.a.push(Array.prototype.slice.call(copy));return void 0;}}return $THREAD_3_44049;}main();"#;

#[test]
fn unique_sample_proves_inline_slice_cap_alternate() {
    for source in [
        UNIQUE_INLINE.to_owned(),
        UNIQUE_INLINE
            .replace("$THREAD_3_44049", "$THREAD_8_128425")
            .replace("record", "$KEY_9")
            .replace("state", "$KEY_11")
            .replace("copy", "$KEY_2")
            .replace("stored", "$KEY_BS")
            .replace("equal", "$KEY_BU")
            .replace("let i=0", "let $KEY_BT=0")
            .replace("[i]", "[$KEY_BT]")
            .replace("i>=", "$KEY_BT>=")
            .replace("++i", "++$KEY_BT")
            .replace(">5", ">9"),
        UNIQUE_INLINE.replace("++i", "i++"),
        UNIQUE_INLINE
            .replace("state.a", "state.items")
            .replace("state.b", "state.total"),
    ] {
        assert!(selector_graphics_claims(&source, "recordPixelVectorSample"), "{source}");
        replay(&source);
    }
}

#[test]
fn unique_sample_refuses_inline_slice_alterations() {
    for (from, to) in [
        // wrong copy: push the formal vector itself, an aliased receiver or a
        // plain second array instead of the fresh Array.prototype.slice copy
        (
            "state.a.push(Array.prototype.slice.call(copy))",
            "state.a.push(copy)",
        ),
        (
            "state.a.push(Array.prototype.slice.call(copy))",
            "state.a.push(Array.prototype.slice.call(record))",
        ),
        (
            "state.a.push(Array.prototype.slice.call(copy))",
            "state.a.push(array.slice.call(copy))",
        ),
        (
            "state.a.push(Array.prototype.slice.call(copy))",
            "state.a.push(Array.prototype.slice.apply(copy))",
        ),
        (
            "state.a.push(Array.prototype.slice.call(copy))",
            "state.a.push(Array.prototype.slice.call(copy,0))",
        ),
        (
            "state.a.push(Array.prototype.slice.call(copy))",
            "state.a.push(arraySlice(copy))",
        ),
        // different record array or two pushes in one cap arm
        (
            "state.a.push(Array.prototype.slice.call(copy))",
            "state.b.push(Array.prototype.slice.call(copy))",
        ),
        (
            "state.a.push(Array.prototype.slice.call(copy))",
            "state.a.push(Array.prototype.slice.call(copy));state.a.push(Array.prototype.slice.call(copy))",
        ),
        // cap counter: the counter slot, its Number() origin and the +1 step
        ("state.b=Number(state.b)+1", "state.b=Number(state.b)+2"),
        ("state.b=Number(state.b)+1", "state.b=Number(state.a)+1"),
        ("state.b=Number(state.b)+1", "state.a=Number(state.a)+1"),
        // cap bound and the record list it measures
        ("state.a.length>5", "state.a.length>=5"),
        ("state.a.length>5", "state.b.length>5"),
        // equality callback guards, operator and compared identity
        ("copy[i]===stored[i]", "copy[i]===stored[0]"),
        ("copy[i]===stored[i]", "copy[i]==stored[i]"),
        ("copy.length!==stored.length", "copy.length===stored.length"),
        ("copy==null", "copy!=null"),
        ("state.a.some", "state.a.filter"),
        // formal identity and the unshadowed copy alias
        ("record,state", "record,{state}"),
        ("stored=>", "(stored=0)=>"),
        ("let copy=record;", "let copy=record;copy=other;"),
        (
            "function main(){function $THREAD_3_44049",
            "function main(){let Array=custom;function $THREAD_3_44049",
        ),
        // nested donor: the slice copy or the push is produced by a nested
        // function/arrow instead of the owned cap arm
        (
            "else{state.a.push(Array.prototype.slice.call(copy));return void 0;}",
            "else{let make=()=>Array.prototype.slice.call(copy);state.a.push(make());return void 0;}",
        ),
        (
            "else{state.a.push(Array.prototype.slice.call(copy));return void 0;}",
            "else{state.a.push(makeCopy(copy));return void 0;}",
        ),
        (
            "else{state.a.push(Array.prototype.slice.call(copy));return void 0;}",
            "else{state.a.push(Array.prototype.slice.call(copy));",
        ),
    ] {
        let source = UNIQUE_INLINE.replace(from, to);
        assert!(
            !selector_graphics_claims(&source, "recordPixelVectorSample"),
            "accepted {from} -> {to}"
        );
    }
}

// ---------------------------------------------------------------------------
// Origin checks: plain-only declarations, completion before use and
// same-statement-list assignments at every alias hop.
// ---------------------------------------------------------------------------

#[test]
fn selector_and_pointer_origins_accept_delayed_direct_setup() {
    let delayed_selector = EVENT.replace(
        "let selectors=[\"INPUT\",\"BUTTON\"];",
        "let selectors;selectors=[\"INPUT\",\"BUTTON\"];",
    );
    assert!(selector_graphics_claims(&delayed_selector, "countEventsOutsideSelector"));
    let forwarded_selector = EVENT.replace(
        "let selectors=[\"INPUT\",\"BUTTON\"];",
        "let base=[\"INPUT\",\"BUTTON\"];let selectors=base;",
    );
    assert!(selector_graphics_claims(
        &forwarded_selector,
        "countEventsOutsideSelector"
    ));
    let delayed_sqrt = POINTER.replace("let math=Math;", "let math;math=Math;");
    assert!(selector_graphics_claims(&delayed_sqrt, "accumulatePointerTravelDistance"));
    let forwarded_sqrt = POINTER.replace("let math=Math;", "let mathBase=Math;let math=mathBase;");
    assert!(selector_graphics_claims(
        &forwarded_sqrt,
        "accumulatePointerTravelDistance"
    ));
}

#[test]
fn animation_origin_accepts_forward_alias_and_refuses_future_setup() {
    let forwarded = r#"function main(){let samples=[];let divisor=2;let recording=false;let scheduler={requestAnimationFrame(f){}};let base=$THREAD_6_164111;let callback=base;function $THREAD_6_164111(timestamp){samples.push(Math.round(timestamp/divisor));recording=true;if(samples.length<20){scheduler.requestAnimationFrame(callback);}}return $THREAD_6_164111;}main();"#;
    assert!(selector_graphics_claims(forwarded, "recordAnimationFrameSample"));
    let future_assignment = r#"function main(){let samples=[];let divisor=2;let recording=false;let scheduler={requestAnimationFrame(f){}};function $THREAD_6_164111(timestamp){samples.push(Math.round(timestamp/divisor));recording=true;if(samples.length<20){scheduler.requestAnimationFrame(callback);}}let callback;callback=$THREAD_6_164111;return $THREAD_6_164111;}main();"#;
    assert!(!selector_graphics_claims(future_assignment, "recordAnimationFrameSample"));
    let future_initializer = r#"function main(){let samples=[];let divisor=2;let recording=false;let scheduler={requestAnimationFrame(f){}};function $THREAD_6_164111(timestamp){samples.push(Math.round(timestamp/divisor));recording=true;if(samples.length<20){scheduler.requestAnimationFrame(callback);}}let callback=$THREAD_6_164111;return $THREAD_6_164111;}main();"#;
    assert!(!selector_graphics_claims(
        future_initializer,
        "recordAnimationFrameSample"
    ));
}

#[test]
fn origin_helpers_refuse_conditional_loop_setup_and_future_setup() {
    for replacement in [
        "let selectors;if(flag){selectors=[\"INPUT\"];}",
        "let selectors;for(;;){selectors=[\"INPUT\"];break;}",
        "let selectors;function setup(){selectors=[\"INPUT\"];}",
    ] {
        let source = EVENT.replace("let selectors=[\"INPUT\",\"BUTTON\"];", replacement);
        assert!(
            !selector_graphics_claims(&source, "countEventsOutsideSelector"),
            "accepted selector origin {replacement}"
        );
    }
    let future_selector = r#"function main(){let selectors;let outside=0;let observer={error(e){}};function $THREAD_2_49889(event){try{if(event.target){if(event.target.closest(selectors.join(","))==null){++outside;}}}catch(e){observer.error(e);}}selectors=["INPUT","BUTTON"];return $THREAD_2_49889;}main();"#;
    assert!(!selector_graphics_claims(future_selector, "countEventsOutsideSelector"));
    let future_literal = r#"function main(){let outside=0;let observer={error(e){}};function $THREAD_2_49889(event){try{if(event.target){if(event.target.closest(selectors.join(","))==null){++outside;}}}catch(e){observer.error(e);}}let selectors=["INPUT","BUTTON"];return $THREAD_2_49889;}main();"#;
    assert!(!selector_graphics_claims(future_literal, "countEventsOutsideSelector"));

    for replacement in [
        "let scheduler;if(flag){scheduler={requestAnimationFrame(f){}};}",
        "let scheduler;for(;;){scheduler={requestAnimationFrame(f){}};break;}",
        "let scheduler;function setup(){scheduler={requestAnimationFrame(f){}};}",
    ] {
        let source = FRAME.replace("let scheduler={requestAnimationFrame(f){}};", replacement);
        assert!(
            !selector_graphics_claims(&source, "recordAnimationFrameSample"),
            "accepted scheduler origin {replacement}"
        );
    }
    let future_scheduler = r#"function main(){let samples=[];let divisor=2;let recording=false;let scheduler;let callback=$THREAD_6_164111;function $THREAD_6_164111(timestamp){samples.push(Math.round(timestamp/divisor));recording=true;if(samples.length<20){scheduler.requestAnimationFrame(callback);}}scheduler={requestAnimationFrame(f){}};return $THREAD_6_164111;}main();"#;
    assert!(!selector_graphics_claims(future_scheduler, "recordAnimationFrameSample"));

    for replacement in [
        "let callback;if(flag){callback=$THREAD_6_164111;}",
        "let callback;for(;;){callback=$THREAD_6_164111;break;}",
        "let callback;function setup(){callback=$THREAD_6_164111;}",
    ] {
        let source = FRAME.replace("let callback=$THREAD_6_164111;", replacement);
        assert!(
            !selector_graphics_claims(&source, "recordAnimationFrameSample"),
            "accepted callback origin {replacement}"
        );
    }
    let prior_reference = r#"function main(){let samples=[];let divisor=2;let recording=false;let scheduler={requestAnimationFrame(f){}};function $THREAD_6_164111(timestamp){samples.push(Math.round(timestamp/divisor));recording=true;if(samples.length<20){scheduler.requestAnimationFrame(callback);}}use($THREAD_6_164111);let callback;callback=$THREAD_6_164111;return $THREAD_6_164111;}main();"#;
    assert!(!selector_graphics_claims(prior_reference, "recordAnimationFrameSample"));
}

#[test]
fn origin_helpers_refuse_resource_and_redeclared_origins() {
    for (source, from, to, name) in [
        (
            EVENT,
            "let selectors=[\"INPUT\",\"BUTTON\"];",
            "using selectors=[\"INPUT\",\"BUTTON\"];",
            "countEventsOutsideSelector",
        ),
        (
            FRAME,
            "let scheduler={requestAnimationFrame(f){}};",
            "using scheduler={requestAnimationFrame(f){}};",
            "recordAnimationFrameSample",
        ),
        (
            FRAME,
            "let callback=$THREAD_6_164111;",
            "using callback=$THREAD_6_164111;",
            "recordAnimationFrameSample",
        ),
        (
            FRAME,
            "let callback=$THREAD_6_164111;",
            "var callback=$THREAD_6_164111;var callback;",
            "recordAnimationFrameSample",
        ),
        (
            FRAME,
            "let scheduler={requestAnimationFrame(f){}};",
            "var scheduler={requestAnimationFrame(f){}};var scheduler;",
            "recordAnimationFrameSample",
        ),
        (
            POINTER,
            "let math=Math;",
            "using math=Math;",
            "accumulatePointerTravelDistance",
        ),
    ] {
        let replaced = source.replace(from, to);
        assert!(
            name_threads(&replaced, true).is_ok(),
            "fixture should parse: {to}"
        );
        assert!(
            !selector_graphics_claims(&replaced, name),
            "accepted resource/redeclaration {to}"
        );
    }
}
