use shape::naming::name_threads;

const SOURCE: &str = r#"function main(){let listeners=[], definitions=[], states=[];
function $THREAD_7_901(event){
let kind=event.type;let status=event.status;let value=event.value;
listeners.forEach(listener=>{let record={type:kind,status:status,value:value,totalSignals:definitions.length,collectedSignals:void 0,erroredSignals:void 0};
record.collectedSignals=states.filter(item=>item.hasCollected).length;
record.erroredSignals=states.filter(item=>item.hasErrored).length;listener(record);});
}return $THREAD_7_901;}main();"#;

fn broadcasts(source: &str) -> bool {
    name_threads(source, true)
        .expect("fixture must parse")
        .report
        .outcomes
        .iter()
        .any(|o| {
            o.binding.name == "$THREAD_7_901"
                && o.applied
                && o.role_name.as_deref() == Some("broadcastSignalNotification")
        })
}

#[test]
fn compact_notification_accepts_owned_binding_and_record_variants() {
    for source in [
        SOURCE.to_string(),
        SOURCE
            .replace("event", "$KEY_801")
            .replace("record", "notification")
            .replace("listener", "subscriber")
            .replace("states", "signalStates"),
        SOURCE.replace(
            "type:kind,status:status,value:value,totalSignals:definitions.length,collectedSignals:void 0,erroredSignals:void 0",
            "erroredSignals:void 0,value:value,collectedSignals:void 0,totalSignals:definitions.length,status:status,type:kind",
        ),
        SOURCE.replace("let ", "const "),
        SOURCE.replace("let ", "var "),
    ] {
        assert!(broadcasts(&source), "{source}");
    }
    let renumbered = SOURCE.replace("$THREAD_7_901", "$THREAD_999_123456_2");
    let result = name_threads(&renumbered, true).unwrap();
    assert!(result.report.outcomes.iter().any(|o| {
        o.binding.name == "$THREAD_999_123456_2"
            && o.applied
            && o.role_name.as_deref() == Some("broadcastSignalNotification")
    }));
}

#[test]
fn compact_notification_rejects_near_misses_and_binding_clobbers() {
    for (from, to) in [
        ("event.type", "other.type"),
        ("event.status", "event.value"),
        ("type:kind", "type:value"),
        ("type:kind", "unexpected:kind"),
        ("type:kind", "['type']:kind"),
        ("collectedSignals:void 0", "collectedSignals:undefined"),
        (
            "collectedSignals:void 0",
            "collectedSignals:void sideEffect()",
        ),
        ("collectedSignals:void 0", "collectedSignals:void 1"),
        ("collectedSignals:void 0,", ""),
        ("erroredSignals:void 0", "collectedSignals:void 0"),
        ("erroredSignals:void 0", "erroredSignals:void 0,extra:0"),
        ("definitions.length", "states.length"),
        ("definitions.length", "listeners.length"),
        ("listeners.forEach", "listeners.map"),
        ("listeners.forEach(", "listeners.forEach?.("),
        ("listener=>", "async listener=>"),
        ("listener=>", "(listener,extra)=>"),
        ("states.filter", "states.map"),
        ("item.hasCollected", "other.hasCollected"),
        ("item.hasCollected", "item.hasErrored"),
        ("item.hasCollected", "item.hasCollected()"),
        ("item.hasCollected", "(sideEffect(),item.hasCollected)"),
        (
            "record.erroredSignals=states",
            "record.erroredSignals=definitions",
        ),
        ("record.erroredSignals", "other.erroredSignals"),
        ("listener(record)", "other(record)"),
        ("listener(record)", "listener(other)"),
        ("listener(record)", "listener(record,0)"),
        ("listener(record)", "listener?.(record)"),
        ("listener(record)", "return listener(record)"),
        ("listener(record)", "record=other;listener(record)"),
        ("listener(record)", "listener=other;listener(record)"),
        ("listener(record)", "value=other;listener(record)"),
        ("listener(record)", "event=other;listener(record)"),
        ("let kind=event.type;", "var kind=event.type;var kind;"),
        ("let kind=event.type;", "let kind=event.type;sideEffect();"),
        (
            "function $THREAD_7_901(event)",
            "function $THREAD_7_901(event,other)",
        ),
        (
            "function $THREAD_7_901(event)",
            "function $THREAD_7_901(event=other)",
        ),
        (
            "let listeners=[], definitions=[], states=[];",
            "var listeners=[], definitions=[], states=[];var states;",
        ),
    ] {
        assert!(SOURCE.contains(from), "missing mutation anchor {from}");
        assert!(
            !broadcasts(&SOURCE.replace(from, to)),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn compact_notification_rejects_nested_and_sibling_donor_evidence() {
    let missing_count = SOURCE.replace(
        "record.collectedSignals=states.filter(item=>item.hasCollected).length;",
        "function donor(){record.collectedSignals=states.filter(item=>item.hasCollected).length;}",
    );
    assert!(!broadcasts(&missing_count));
    let donor = SOURCE.replace("$THREAD_7_901", "readableDonor");
    let candidate = "function $THREAD_7_901(event){let kind=event.type;let status=event.status;let value=event.value;listeners.forEach(listener=>{listener(event);});}";
    assert!(!broadcasts(&format!("{donor}{candidate}")));
    let delegated = SOURCE.replace(
        "record.collectedSignals=states.filter(item=>item.hasCollected).length;",
        "record.collectedSignals=countCollected();",
    );
    assert!(!broadcasts(&format!(
        "{delegated} function countCollected(){{return states.filter(item=>item.hasCollected).length;}}"
    )));
}

#[test]
fn indexed_notification_grammar_is_preserved_and_hybrids_rejected() {
    let indexed = SOURCE
        .replace(
            "let value=event.value;",
            "let value=event.value;let index=event.signalIdx;",
        )
        .replace(
            "collectedSignals:void 0,erroredSignals:void 0",
            "signalIdx:index",
        );
    assert!(broadcasts(&indexed));
    assert!(!broadcasts(&indexed.replace("signalIdx:index", "")));
    assert!(!broadcasts(&indexed.replace(
        "signalIdx:index",
        "collectedSignals:void 0,erroredSignals:void 0"
    )));
    assert!(!broadcasts(&SOURCE.replace(
        "collectedSignals:void 0,erroredSignals:void 0",
        "signalIdx:value"
    )));
}

#[test]
fn readable_compact_notification_is_preserved() {
    let source = SOURCE.replace("$THREAD_7_901", "notifySubscribers");
    assert_eq!(name_threads(&source, true).unwrap().source, source);
}
