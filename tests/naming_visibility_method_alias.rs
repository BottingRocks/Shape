use shape::naming::name_threads;

fn named(source: &str, name: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|o| o.applied && o.proposed_name.as_deref() == Some(name))
}

fn applied_for(source: &str, binding: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == binding)
        .map(|o| o.applied)
        .unwrap_or(false)
}

// The installed anchor visibility rule accepts any captured zero-argument clock, including
// mutable or non-context aliases. This package requires stronger evidence. Scope refusal
// assertions to its evaluator so overlap cannot hide a regression.
fn named_by(source: &str, name: &str, evaluator: &str) -> bool {
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

// A portable, reachable `main` wrapper. `timeElapsedSinceInit` is the bounded
// session-context member the recorder consumes; the event object is an opaque
// runtime callback input read only as data.
const VISIBILITY: &str = r#"function main(){
let ring=makeRing(20);
let dirty=true;
let count=0;
let sink={error(e){}};
let context={timeElapsedSinceInit(){return 0;}};
function $THREAD_0_23746(event){
try{
++count;
let receiver=ring;
let method=ring.put;
method.call(receiver,{visible:event.target.visibilityState==="visible",timestamp:context.timeElapsedSinceInit()});
dirty=true;
}catch(e){sink.error(e);}
}
return $THREAD_0_23746;
}
main();"#;

#[test]
fn visibility_recorder_proves_owned_body_and_context_clock_protocol() {
    assert!(named(VISIBILITY, "recordVisibilityChangeSample"));
    let renamed = VISIBILITY
        .replace("$THREAD_0_23746", "$THREAD_88_123456")
        .replace("event", "evt")
        .replace("ring", "buffer")
        .replace("method", "putAlias")
        .replace("count", "ticks")
        .replace("dirty", "pending")
        .replace("sink", "errors")
        .replace("context", "ctx");
    assert!(named(&renamed, "recordVisibilityChangeSample"));
    // The clock may instead be a captured immutable alias of the context member.
    let aliased = VISIBILITY
        .replace(
            "let context={timeElapsedSinceInit(){return 0;}};",
            "let context={timeElapsedSinceInit(){return 0;}};let clock=context.timeElapsedSinceInit;",
        )
        .replace(
            "timestamp:context.timeElapsedSinceInit()",
            "timestamp:clock()",
        );
    assert!(named(&aliased, "recordVisibilityChangeSample"));
    // A meaningful existing name is preserved and never rewritten.
    let human = VISIBILITY.replace("$THREAD_0_23746", "recordVisibilityChangeSample");
    assert_eq!(name_threads(&human, true).unwrap().source, human);
}

#[test]
fn visibility_recorder_refuses_changed_operations_arguments_and_order() {
    for (from, to) in [
        (
            r#"visibilityState==="visible""#,
            r#"visibilityState=="visible""#,
        ),
        (
            r#"target.visibilityState==="visible""#,
            r#"target.visibilityState==="shown""#,
        ),
        (
            "event.target.visibilityState",
            "other.target.visibilityState",
        ),
        ("method.call(", "method.apply("),
        ("method.call(receiver,", "method.call(other,"),
        ("{visible:", "{shown:"),
        ("timestamp:context", "time:context"),
        ("context.timeElapsedSinceInit()", "context.now()"),
        ("++count;", "--count;"),
        ("++count;", "count++;"),
        ("dirty=true;", "dirty=false;"),
        ("sink.error(e);", "sink.fail(e);"),
        ("sink.error(e);", "sink.error();"),
        ("sink.error(e);", "sink.error(e);sink.error(e);"),
        (
            "function $THREAD_0_23746(event){",
            "function $THREAD_0_23746(event,extra){",
        ),
        ("catch(e){", "catch({e}){"),
        ("let method=ring.put;", "let method=ring.put;method=other;"),
        ("let receiver=ring;", "let receiver=ring;receiver=other;"),
        (
            "method.call(receiver,{visible:",
            "method.call(receiver,{other:0,visible:",
        ),
        (
            r#"timestamp:context.timeElapsedSinceInit()});"#,
            r#"timestamp:context.timeElapsedSinceInit()});extra();"#,
        ),
    ] {
        let source = VISIBILITY.replace(from, to);
        assert!(
            !named(&source, "recordVisibilityChangeSample"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn visibility_recorder_refuses_mutable_and_non_context_clock_aliases() {
    // A mutable clock alias, a clock alias of a non-context origin, and an
    // alias reassigned after use all refuse.
    let mutable = VISIBILITY
        .replace(
            "let context={timeElapsedSinceInit(){return 0;}};",
            "let context={timeElapsedSinceInit(){return 0;}};let clock=context.timeElapsedSinceInit;clock=other;",
        )
        .replace(
            "timestamp:context.timeElapsedSinceInit()",
            "timestamp:clock()",
        );
    let unrelated = VISIBILITY
        .replace(
            "let context={timeElapsedSinceInit(){return 0;}};",
            "let context={timeElapsedSinceInit(){return 0;}};let clock=externalClock;",
        )
        .replace(
            "timestamp:context.timeElapsedSinceInit()",
            "timestamp:clock()",
        );
    for source in [mutable, unrelated] {
        assert!(
            !named_by(
                &source,
                "recordVisibilityChangeSample",
                "events_visibility_method_alias"
            ),
            "{source}"
        );
    }
}

#[test]
fn visibility_recorder_refuses_extra_or_missing_owned_statements() {
    let extra_top = VISIBILITY.replace("try{", "var count;try{");
    let extra_try = VISIBILITY.replace("++count;\n", "++count;extra();\n");
    let missing_flag = VISIBILITY.replace("\ndirty=true;\n}catch", "\n}catch");
    for source in [extra_top, extra_try, missing_flag] {
        assert!(!named(&source, "recordVisibilityChangeSample"), "{source}");
    }
}

// The exact owned body of the residual seed function, in a minimal reachable
// wrapper. This is a portable fixture for the rule, not a captured program.
const REAL_HANDLER: &str = r#"function $THREAD_0_23746($CA) {
					try {
						++$KEY_3;
						let $CB = $KEY_13;
						let $CC = $KEY_13.put;
						$CC.call($CB, {
							visible: $CA.target.visibilityState === "visible",
							timestamp: $KEY_6()
						});
						$KEY_14 = true;
					} catch (e) {
						$KEY_23.error(e);
					}
				}"#;

#[test]
fn exact_seed_visibility_handler_owned_body_matches() {
    let source = format!(
        "function main(){{let $KEY_21={{}};let $KEY_6=$KEY_21.timeElapsedSinceInit;let $KEY_13=new $KEY_98(20);let $KEY_3=0;let $KEY_14=true;let $KEY_23={{error(e){{}}}};{REAL_HANDLER}return $THREAD_0_23746;}}main();"
    );
    assert!(named(&source, "recordVisibilityChangeSample"));
}

#[test]
fn suffixed_duplicate_recorder_gets_a_distinct_ordinal() {
    let second = "function $THREAD_0_23746_2(event){try{++count;let receiver=ring;let method=ring.put;method.call(receiver,{visible:event.target.visibilityState===\"visible\",timestamp:context.timeElapsedSinceInit()});dirty=true;}catch(e){sink.error(e);}}";
    let source = VISIBILITY.replace(
        "return $THREAD_0_23746;",
        &format!("{second}return $THREAD_0_23746;"),
    );
    assert!(named(&source, "recordVisibilityChangeSample"));
    assert!(
        applied_for(&source, "$THREAD_0_23746_2"),
        "the suffixed duplicate gets a distinct concrete name, not the base"
    );
    let report = name_threads(&source, true).unwrap();
    let clone = report
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == "$THREAD_0_23746_2")
        .expect("clone outcome");
    assert_eq!(
        clone.proposed_name.as_deref(),
        Some("recordVisibilityChangeSample2")
    );
}
