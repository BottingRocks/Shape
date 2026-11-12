use shape::naming::name_threads;

fn named(source: &str, expected: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report
            .outcomes
            .iter()
            .any(|o| o.applied && o.proposed_name.as_deref() == Some(expected))
    })
}

const SEARCH: &str = r#"function main(){let table={};
function $THREAD_8_919(needle,key){
if(table[key]==null){return null;}
let cursor=0;
while(cursor<table[key].length){
if(needle===table[key][cursor].object){return table[key][cursor];}
else{++cursor;continue;}
}
return null;
}return $THREAD_8_919;}main();"#;

#[test]
fn bucket_search_proves_scoped_table_key_cursor_and_record() {
    for source in [
        SEARCH.to_owned(),
        SEARCH
            .replace("$THREAD_8_919", "$THREAD_923_182734_2")
            .replace("table", "$KEY_900")
            .replace("needle", "subject")
            .replace("cursor", "offset")
            .replace("key", "category"),
        SEARCH.replace("else{++cursor;continue;}", "cursor++;"),
        SEARCH.replace("else{++cursor;continue;}", "++cursor;"),
        SEARCH.replace("let table={}", "const table=[]"),
        SEARCH.replace(
            "let table={};",
            "let table;table={};function reset(next){table=next;}",
        ),
        SEARCH.replace("let table={};", "let table={};table=other;"),
    ] {
        assert!(named(&source, "findObjectRecordInBucket"), "{source}");
        let r = name_threads(&source, true).unwrap();
        let mut replay = source;
        for e in r.report.edits.iter().rev() {
            replay.replace_range(e.start as usize..e.end as usize, &e.replacement);
        }
        assert_eq!(replay, r.source);
    }
}

#[test]
fn bucket_search_rejects_changed_operations_bounds_and_returns() {
    for (from, to) in [
        ("table[key]==null", "table[key]!=null"),
        ("table[key]==null", "table[key]===null"),
        ("return null;}", "return undefined;}"),
        ("let cursor=0", "let cursor=1"),
        ("cursor<table[key].length", "cursor<=table[key].length"),
        ("cursor<table[key].length", "cursor<table[needle].length"),
        ("cursor<table[key].length", "cursor<other[key].length"),
        ("needle===", "needle=="),
        ("needle===", "key==="),
        (".object", ".value"),
        ("table[key][cursor].object", "table[needle][cursor].object"),
        ("table[key][cursor].object", "table[key][key].object"),
        (
            "return table[key][cursor]",
            "return table[key][cursor].object",
        ),
        ("return table[key][cursor]", "return other[key][cursor]"),
        ("++cursor", "--cursor"),
        ("++cursor", "++key"),
        ("continue;", "break;"),
        ("continue;", "continue outer;"),
        ("continue;", "sideEffect();continue;"),
        ("return null;\n}", "return false;\n}"),
        ("if(table[key]", "sideEffect();if(table[key]"),
        ("[cursor].object", "[cursor]?.object"),
    ] {
        let changed = SEARCH.replace(from, to);
        assert_ne!(changed, SEARCH, "missing mutation {from}");
        assert!(
            !named(&changed, "findObjectRecordInBucket"),
            "accepted {from} -> {to}"
        );
    }
}

#[test]
fn bucket_search_rejects_unstable_or_ambiguous_binding_origins() {
    for (from, to) in [
        ("let table={};", "var table={};var table;"),
        ("let table={};", "let {table}={table:{}};"),
        ("function main()", "function main(table)"),
        ("let cursor=0;", "var cursor=0;var cursor;"),
        ("(needle,key)", "(needle=other,key)"),
        ("(needle,key)", "(needle,...key)"),
        ("(needle,key)", "({needle},key)"),
        ("(needle,key)", "(needle,key,extra)"),
        ("(needle,key)", "(needle,needle)"),
        ("function $THREAD_8_919", "async function $THREAD_8_919"),
        ("function $THREAD_8_919", "function* $THREAD_8_919"),
        ("if(table[key]", "\"use strict\";if(table[key]"),
    ] {
        assert!(
            !named(&SEARCH.replace(from, to), "findObjectRecordInBucket"),
            "accepted {from} -> {to}"
        );
    }
    let parameter = SEARCH.replace("function main(){let table={};", "function main(table){");
    assert!(!named(&parameter, "findObjectRecordInBucket"));
    let caught = SEARCH
        .replace("let table={};", "try{throw {};}catch(table){")
        .replace("}return $THREAD_8_919;", "}return $THREAD_8_919;}");
    assert!(!named(&caught, "findObjectRecordInBucket"));
    let loop_target = SEARCH
        .replace("let table={};", "for(let table of tables){")
        .replace("}return $THREAD_8_919;", "}return $THREAD_8_919;}");
    assert!(!named(&loop_target, "findObjectRecordInBucket"));
}

#[test]
fn bucket_search_keeps_distinct_shadowed_capture_identity() {
    let changed = SEARCH.replace("if(needle===", "if(needle===").replace(
        "return table[key][cursor];",
        "{let table=other;return table[key][cursor];}",
    );
    assert!(!named(&changed, "findObjectRecordInBucket"));
    let changed = SEARCH.replace(
        "return table[key][cursor];",
        "{let cursor=key;return table[key][cursor];}",
    );
    assert!(!named(&changed, "findObjectRecordInBucket"));
}

fn action(body: &str) -> String {
    format!(
        "function main(){{function $THREAD_51_730(event){{{body}}}return $THREAD_51_730;}}main();"
    )
}

#[test]
fn prevent_default_requires_exact_receiver_call_arguments_and_void_completion() {
    let source = action("event.preventDefault();");
    assert!(named(&source, "preventDefaultHandler"));
    assert!(named(
        &source
            .replace("(event)", "($KEY_399)")
            .replace("event.", "$KEY_399.")
            .replace("$THREAD_51_730", "$THREAD_91_77_3"),
        "preventDefaultHandler"
    ));
    for body in [
        "other.preventDefault();",
        "event.stopPropagation();",
        "event.preventDefault(1);",
        "return event.preventDefault();",
        "event?.preventDefault();",
        "event.preventDefault?.();",
        "event.preventDefault();return false;",
        "event=other;event.preventDefault();",
        "{let event=other;event.preventDefault();}",
    ] {
        assert!(!named(&action(body), "preventDefaultHandler"), "{body}");
    }
    for formal in ["event=other", "...event", "{event}", "event,other"] {
        assert!(!named(
            &source.replace("(event)", &format!("({formal})")),
            "preventDefaultHandler"
        ));
    }
}

#[test]
fn scalar_dom_readers_prove_complete_path_and_formal_identity() {
    for (path, name) in [
        ("target.value", "readEventTargetValue"),
        ("target.checked", "readEventTargetChecked"),
        ("target.visibilityState", "readEventTargetVisibilityState"),
        ("parentNode", "getParentNode"),
        ("childNodes", "getChildNodes"),
    ] {
        let source = action(&format!("return event.{path};"));
        assert!(named(&source, name));
        assert!(named(
            &source
                .replace("event", "$KEY_38")
                .replace("$THREAD_51_730", "$THREAD_88_910_2"),
            name
        ));
        for changed in [
            source.replace("return event.", "return other."),
            source.replace("return event.", "return event?."),
            source.replace(&format!(".{path};"), &format!(".{path}();")),
            source.replace("return event.", "event=other;return event."),
            source.replace("return event.", "return !event."),
            source.replace("(event)", "(event={})"),
        ] {
            assert!(!named(&changed, name), "{changed}");
        }
    }
    assert!(!named(
        &action("return event.currentTarget.value;"),
        "readEventTargetValue"
    ));
    assert!(!named(
        &action("return event.target.checked;"),
        "readEventTargetValue"
    ));
}

#[test]
fn human_names_and_ancestor_collisions_remain_untouched() {
    let source = SEARCH.replace("$THREAD_8_919", "lookupRecord");
    assert_eq!(name_threads(&source, true).unwrap().source, source);
    let collision = SEARCH.replace(
        "let table={};",
        "let table={};let findObjectRecordInBucket=1;",
    );
    assert!(!named(&collision, "findObjectRecordInBucket"));
    let descendant = SEARCH.replace(
        "let cursor=0;",
        "let cursor=0;function findObjectRecordInBucket(){};",
    );
    assert!(!named(&descendant, "findObjectRecordInBucket"));
}

#[test]
fn independent_candidates_are_collected_without_displacing_legacy_claims() {
    let source = "function main(){function $THREAD_1_100(x){return x;}function $THREAD_2_101(e){e.preventDefault();}function $THREAD_3_102(e){return e.target.value;}return [$THREAD_1_100,$THREAD_2_101,$THREAD_3_102];}main();";
    let result = name_threads(source, true).unwrap();
    for name in ["identity", "preventDefaultHandler", "readEventTargetValue"] {
        assert!(
            result
                .report
                .outcomes
                .iter()
                .any(|o| o.applied && o.proposed_name.as_deref() == Some(name)),
            "{name}"
        );
    }
    assert!(result.report.removals.is_empty());
}

const DEFERRED: &str = "function main(){let target=null,clock=window;function $THREAD_27_73(event){target=event.target;clock.setTimeout(()=>{target=null;},0);}return $THREAD_27_73;}main();";

#[test]
fn deferred_target_proves_same_slot_and_zero_delay_callback() {
    for source in [
        DEFERRED.to_owned(),
        DEFERRED
            .replace("$THREAD_27_73", "$THREAD_812_991_2")
            .replace("target=", "$KEY_712=")
            .replace("event.target", "event.target")
            .replace("event", "input")
            .replace("clock", "scheduler"),
        DEFERRED.replace("()=>{target=null;}", "()=>target=null"),
    ] {
        assert!(named(&source, "captureEventTargetUntilTimeout"), "{source}");
    }
}

#[test]
fn deferred_target_refuses_callback_receiver_arguments_and_completion_changes() {
    for (from, to) in [
        ("event.target", "event.currentTarget"),
        ("event.target", "event?.target"),
        ("target=event", "target+=event"),
        ("target=null;},0", "target=false;},0"),
        ("target=null;},0", "other=null;},0"),
        ("target=null;},0", "target=null;sideEffect();},0"),
        ("()=>{", "async ()=>{"),
        ("()=>{", "(event)=>{"),
        ("()=>{target=null;}", "()=>{let target=null;}"),
        ("()=>{target=null;}", "()=>{return target=null;}"),
        ("},0)", "},1)"),
        ("},0)", "},0,event)"),
        ("clock.setTimeout", "clock.setInterval"),
        ("clock.setTimeout", "clock?.setTimeout"),
        ("clock.setTimeout(", "clock.setTimeout?.("),
        ("clock.setTimeout", "target.setTimeout"),
        ("clock.setTimeout", "timer"),
        ("clock.setTimeout", "return clock.setTimeout"),
        ("target=event.target;", "event=other;target=event.target;"),
        (
            "let target=null,clock=window;",
            "var target=null,clock=window;var target;",
        ),
        (
            "let target=null,clock=window;",
            "var target=null,clock=window;var clock;",
        ),
        (
            "function main(){let target=null,clock=window;",
            "function main(target,clock){",
        ),
        (
            "let target=null,clock=window;",
            "let {target,clock}=provider;",
        ),
    ] {
        assert!(
            !named(
                &DEFERRED.replace(from, to),
                "captureEventTargetUntilTimeout"
            ),
            "{from} -> {to}"
        );
    }
    for setup in [
        "let timer=clock.setTimeout;",
        "let timer=clock.setTimeout;timer=other;",
        "var timer=clock.setTimeout;var timer;",
        "let [timer]=callbacks;",
    ] {
        let source = DEFERRED
            .replace("function $THREAD", &format!("{setup}function $THREAD"))
            .replace("clock.setTimeout(", "timer(");
        assert!(!named(&source, "captureEventTargetUntilTimeout"));
    }
    let future = DEFERRED.replace("clock.setTimeout(", "timer(").replace(
        "return $THREAD_27_73;",
        "return $THREAD_27_73;let timer=clock.setTimeout;",
    );
    assert!(!named(&future, "captureEventTargetUntilTimeout"));
}

#[test]
fn deferred_target_preserves_human_names_and_collision_guards() {
    let human = DEFERRED.replace("$THREAD_27_73", "rememberTarget");
    assert_eq!(name_threads(&human, true).unwrap().source, human);
    for source in [
        DEFERRED.replace("let target", "let captureEventTargetUntilTimeout=0;useValue(captureEventTargetUntilTimeout);let target"),
        DEFERRED.replace("target=event.target;", "function captureEventTargetUntilTimeout(){}target=event.target;"),
    ] { assert!(!named(&source,"captureEventTargetUntilTimeout")); }
}
