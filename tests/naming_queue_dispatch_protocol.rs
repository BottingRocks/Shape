use shape::naming::name_threads;
const SOURCE: &str = r#"function main(){let callbacks=[],slot;
function $THREAD_1_30(provider,value){let host=provider;slot=value;callbacks.forEach(item=>{host.setTimeout(item,0,slot);});}
return $THREAD_1_30;}main();"#;
fn named(source: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|o| {
            o.binding.name == "$THREAD_1_30"
                && o.applied
                && o.proposed_name.as_deref() == Some("scheduleCapturedCallbacksWithArgument")
        })
}
#[test]
fn dispatch_complete_protocol() {
    assert!(named(SOURCE));
    assert!(named(
        &SOURCE
            .replace("callbacks", "pending")
            .replace("slot", "argument")
            .replace("host", "timer")
    ));
    for (from, to) in [
        ("slot=value", "slot=provider"),
        ("item,0,slot", "slot,0,item"),
        ("item,0,slot", "item,1,slot"),
        ("host.setTimeout", "provider.setTimeout"),
        (
            "host.setTimeout(item,0,slot);",
            "host.setTimeout(item,0,slot);sideEffect();",
        ),
        ("callbacks.forEach", "slot.forEach"),
        ("slot=value;callbacks.forEach", "callbacks.forEach"),
    ] {
        assert!(SOURCE.contains(from));
        assert!(!named(&SOURCE.replace(from, to)), "{from} -> {to}");
    }
}
