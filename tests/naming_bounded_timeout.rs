use shape::naming::name_threads;

// The owned body has one local base delay, an empty-catch retry recomputation through the
// unresolved global Math provider, an independently ordered zero-argument producer, and one
// provider timer store. Provider, value, and handle are captured plain data bindings.
const JITTER: &str = r#"function main(){let provider={},handle=null,callback,jitter=()=>1;
function $THREAD_9_142882(){let delay=2000;try{delay=Math.floor(Math.max(2000,jitter()%4000));}catch(error){}handle=provider.setTimeout(callback,delay);}
callback=$THREAD_9_142882;$THREAD_9_142882();return $THREAD_9_142882;}main();"#;

const ROLE: &str = "scheduleBoundedTimeout";

fn named(source: &str, target: &str, name: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|o| o.binding.name == target && o.applied && o.proposed_name.as_deref() == Some(name))
}

fn outcome<'a>(
    report: &'a shape::naming::ThreadNamingReport,
    target: &str,
) -> Option<&'a shape::naming::ThreadName> {
    report.outcomes.iter().find(|o| o.binding.name == target)
}

#[test]
fn names_exact_timer_adapter_with_generated_ids_and_alpha_renamed_bindings() {
    for (source, target) in [
        (JITTER.to_string(), "$THREAD_9_142882"),
        (
            JITTER.replace("$THREAD_9_142882", "$THREAD_912_99881_2"),
            "$THREAD_912_99881_2",
        ),
        (
            JITTER
                .replace("$THREAD_9_142882", "$THREAD_44_7")
                .replace("provider", "$KEY_5")
                .replace("handle", "$KEY_8")
                .replace("callback", "$KEY_7")
                .replace("jitter", "$KEY_191")
                .replace("delay", "$CA")
                .replace("error", "e"),
            "$THREAD_44_7",
        ),
        (
            JITTER.replace("jitter=()=>1", "jitter=function(){return 1;}"),
            "$THREAD_9_142882",
        ),
        (
            JITTER.replace(
                "let provider={},handle=null,callback,jitter=()=>1;",
                "const provider={};let handle=null,callback;var jitter=()=>1;",
            ),
            "$THREAD_9_142882",
        ),
        // Exponent-form constants decode to the same numeric literals.
        (
            JITTER.replace("2000", "2e3").replace("4000", "4e3"),
            "$THREAD_9_142882",
        ),
        // The delay producer may be an ordered alias chain.
        (
            JITTER.replace(
                "let provider={},handle=null,callback,jitter=()=>1;",
                "let provider={},handle=null,callback,producer=()=>1,jitter=producer;",
            ),
            "$THREAD_9_142882",
        ),
    ] {
        assert!(named(&source, target, ROLE), "{source}");
    }
}

#[test]
fn rejects_changed_operations_arguments_and_control_flow() {
    for (from, to) in [
        ("Math.floor", "Math.round"),
        ("Math.max", "Math.min"),
        ("Math.max(2000,", "Math.max(1999,"),
        ("%4000", "+4000"),
        ("%4000", "%-4000"),
        ("4000", "4001"),
        ("jitter()%4000", "jitter(1)%4000"),
        ("setTimeout", "setInterval"),
        (
            "provider.setTimeout(callback,delay)",
            "provider.setTimeout(callback)",
        ),
        (
            "provider.setTimeout(callback,delay)",
            "provider.setTimeout(callback,delay,0)",
        ),
        (
            "provider.setTimeout(callback,delay)",
            "provider?.setTimeout(callback,delay)",
        ),
        (
            "provider.setTimeout(callback,delay)",
            "provider.setTimeout(callback,2000)",
        ),
        ("handle=provider.setTimeout", "return provider.setTimeout"),
        (
            "handle=provider.setTimeout(callback,delay);",
            "handle=provider.setTimeout(callback,delay);handle=null;",
        ),
        ("try{delay=", "try{handle=null;delay="),
        ("delay=Math.floor", "delay+=Math.floor"),
        ("let delay=2000;", "let delay=3000;"),
        (
            "handle=provider.setTimeout(callback,delay);",
            "delay=1;handle=provider.setTimeout(callback,delay);",
        ),
        ("catch(error){}", "catch(error){handle=null;}"),
        ("catch(error){}", "catch(error){delay=1;}"),
        (
            "function $THREAD_9_142882()",
            "function $THREAD_9_142882(extra)",
        ),
        (
            "function $THREAD_9_142882()",
            "function $THREAD_9_142882(...extra)",
        ),
        (
            "function $THREAD_9_142882()",
            "async function $THREAD_9_142882()",
        ),
        (
            "function $THREAD_9_142882(){",
            "function $THREAD_9_142882(){\"use asm\";",
        ),
    ] {
        let source = JITTER.replace(from, to);
        assert!(!named(&source, "$THREAD_9_142882", ROLE), "{from} -> {to}");
    }
}

#[test]
fn rejects_unproven_or_mutated_delay_producer_and_shadowed_math() {
    for source in [
        // The producer's initializer is textually after the owned use.
        JITTER
            .replace(
                "let provider={},handle=null,callback,jitter=()=>1;",
                "let provider={},handle=null,callback,jitter;",
            )
            .replace(
                "callback=$THREAD_9_142882;",
                "jitter=()=>1;callback=$THREAD_9_142882;",
            ),
        // A conditional producer has no single ordered origin.
        JITTER.replace("jitter=()=>1", "jitter=flag?()=>1:()=>2"),
        // A parameter is a runtime origin, never an ordered callable proof.
        JITTER
            .replace("function main(){", "function main(jitter){")
            .replace(",jitter=()=>1", ""),
        // A locally shadowed Math is not the unresolved global provider.
        JITTER.replace(
            "function main(){",
            "function main(){let Math={floor:x=>x,max:x=>x};",
        ),
        // The producer itself must not alias through a redeclared binding.
        JITTER.replace("function main(){", "function main(){jitter=()=>1;"),
        // A second write to the producer breaks the single ordered origin.
        JITTER.replace(
            "callback=$THREAD_9_142882;",
            "jitter=()=>2;callback=$THREAD_9_142882;",
        ),
    ] {
        assert!(!named(&source, "$THREAD_9_142882", ROLE), "{source}");
    }
}

#[test]
fn rejects_unproven_provider_value_and_handle_data_identities() {
    for source in [
        // Provider, value and handle must be distinct captured bindings.
        JITTER
            .replace(
                "let provider={},handle=null,callback,jitter=()=>1;",
                "let provider={},callback,jitter=()=>1;",
            )
            .replace("$THREAD_9_142882();", "$THREAD_9_142882();let handle=null;"),
        // A parameter of the owned function is not a captured outer binding.
        JITTER.replace(
            "function $THREAD_9_142882(){",
            "function $THREAD_9_142882(provider){",
        ),
        // A body-local shadow of the provider is not a captured binding.
        JITTER.replace(
            "function $THREAD_9_142882(){",
            "function $THREAD_9_142882(){let provider={};",
        ),
        // Reassigning the captured value at the call site to a non-captured name.
        JITTER.replace(
            "handle=provider.setTimeout(callback,delay);",
            "handle=provider.setTimeout(missing,delay);",
        ),
        // A destructured capture never establishes the plain data binding.
        JITTER.replace(
            "let provider={},handle=null,callback,jitter=()=>1;",
            "let {provider}={},handle=null,callback,jitter=()=>1;",
        ),
    ] {
        assert!(!named(&source, "$THREAD_9_142882", ROLE), "{source}");
    }
}

#[test]
fn preserves_readable_names_and_normal_collision_resolution() {
    // A readable existing name is never renamed.
    let readable = JITTER.replace("$THREAD_9_142882", "jitteredTimer");
    assert!(!named(&readable, "jitteredTimer", ROLE));
    let report = name_threads(&readable, true).unwrap().report;
    assert!(outcome(&report, "jitteredTimer").is_none());

    // Two generated roles share one declaration scope. The older outer role
    // keeps the meaningful base and the duplicate gets a deterministic ordinal
    // instead of both being refused.
    let related = JITTER.replace(
        "callback=$THREAD_9_142882;$THREAD_9_142882();return $THREAD_9_142882;",
        "callback=$THREAD_9_142882;$THREAD_9_142882();function $THREAD_9_14(){let delay=2000;try{delay=Math.floor(Math.max(2000,jitter()%4000));}catch(error){}handle=provider.setTimeout(callback,delay);}return $THREAD_9_14;",
    );
    let report = name_threads(&related, true).unwrap().report;
    let base = outcome(&report, "$THREAD_9_142882").expect("original outcome");
    assert!(base.applied);
    assert_eq!(base.proposed_name.as_deref(), Some(ROLE));
    let duplicate = outcome(&report, "$THREAD_9_14").expect("duplicate outcome");
    assert!(duplicate.applied);
    assert_eq!(
        duplicate.proposed_name.as_deref(),
        Some("scheduleBoundedTimeout2")
    );
}
