use shape::naming::name_threads;
fn applied(source: &str, identity: &str, expected: Option<&str>) {
    let result = name_threads(source, true).expect("negative must parse and index");
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == identity)
        .expect("the actual target must survive, not an unrelated donor");
    assert_eq!(
        outcome.applied,
        expected.is_some(),
        "{}",
        serde_json::to_string(outcome).unwrap()
    );
    assert_eq!(outcome.proposed_name.as_deref(), expected);
}

fn refuses(base: &str, identity: &str, from: &str, to: &str) {
    assert!(base.contains(from));
    let source = base.replacen(from, to, 1);
    assert_ne!(source, base);
    applied(&source, identity, None);
}

#[test]
fn href_accessors_prove_only_own_property_read_or_write() {
    let read = "function main(){ function $THREAD_1_100(object){return object.href;} return $THREAD_1_100;}main();";
    let write = "function main(){ function $THREAD_1_101(object,value){object.href=value;} return $THREAD_1_101;}main();";
    applied(read, "$THREAD_1_100", Some("readHrefProperty"));
    applied(write, "$THREAD_1_101", Some("writeHrefProperty"));
    for (from, to) in [
        ("object.href", "object?.href"),
        ("return object.href", "object.touch();return object.href"),
        ("return object.href", "return donor.href"),
    ] {
        refuses(read, "$THREAD_1_100", from, to);
    }
    for (from, to) in [
        ("object.href=value", "object.href+=value"),
        ("object.href=value", "value.href=object"),
        ("object.href=value;", "object.href=value;return value;"),
        ("object.href=value", "donor.href=value"),
    ] {
        refuses(write, "$THREAD_1_101", from, to);
    }
}

#[test]
fn actual_whole_scope_href_accessors_and_alpha_variant() {
    let original = include_str!("fixtures/naming/adapter_protocols/adapter-whole-scope.js");
    for source in [
        original.to_owned(),
        original
            .replace("$AW", "$alphaObject")
            .replace("$AX", "$alphaValue"),
    ] {
        let result = name_threads(&source, true).unwrap();
        let own: Vec<_> = result
            .report
            .outcomes
            .iter()
            .filter(|o| o.applied && o.evaluator.as_deref() == Some("browser_href_accessors"))
            .collect();
        assert_eq!(own.len(), 4);
        if source == original {
            for start in [400963, 401034, 596766, 596836] {
                assert!(own.iter().any(|o| o.binding.declaration.start == start));
            }
        }
    }
}

#[test]
fn actual_whole_scope_json_and_alpha_variant() {
    let original = include_str!("fixtures/naming/adapter_protocols/adapter-whole-scope-alternate.js");
    for source in [original.to_owned(), original.replace("$CB", "$alphaInput")] {
        let result = name_threads(&source, true).unwrap();
        let own: Vec<_> = result
            .report
            .outcomes
            .iter()
            .filter(|o| {
                o.applied && o.evaluator.as_deref() == Some("json_roundtrip_with_invalid_error")
            })
            .collect();
        assert_eq!(own.len(), 1);
        assert_eq!(
            own[0].role_name.as_deref(),
            Some("normalizeJsonOrReportInvalid")
        );
    }
}

#[test]
fn json_exact_throw_return_and_capture_protocol() {
    let source="function main(){function $THREAD_0_30(input){try{return JSON.stringify(JSON.parse(input));}catch(error){return \"INVALID:\"+error;}}return $THREAD_0_30;}main();";
    applied(source, "$THREAD_0_30", Some("normalizeJsonOrReportInvalid"));
    for (from, to) in [
        ("JSON.parse(input)", "JSON.parse(donor)"),
        (
            "JSON.stringify(JSON.parse(input))",
            "JSON.parse(JSON.stringify(input))",
        ),
        ("\"INVALID:\"+error", "\"INVALID:\"+input"),
        ("\"INVALID:\"+error", "\"INVALID\"+error"),
        (
            "return JSON.stringify",
            "input.touch();return JSON.stringify",
        ),
        ("JSON.parse(input)", "JSON.parse?.(input)"),
        ("function main(){", "function main(){let JSON=donor;"),
        ("catch(error){", "catch(error){input.touch();"),
    ] {
        refuses(source, "$THREAD_0_30", from, to);
    }
}

const ENVELOPE: &str = r#"function main(){
function consume(record){return record;}
let consumer=consume;
function $THREAD_6_73862(event){
 let record={event:void 0,target:void 0};
 record.event=event;
 record.target=event.target;
 return consumer(record);
}
return $THREAD_6_73862;
}main();"#;

#[test]
fn event_envelope_actual_whole_scope_and_alpha_variant() {
    let original = include_str!("fixtures/naming/adapter_protocols/adapter-whole-scope.js");
    for source in [
        original.to_owned(),
        original
            .replace("$AW", "$alphaEvent")
            .replace("$AX", "$alphaRecord"),
    ] {
        let result = name_threads(&source, true).unwrap();
        let own: Vec<_> = result
            .report
            .outcomes
            .iter()
            .filter(|o| o.applied && o.evaluator.as_deref() == Some("events_event_target_envelope"))
            .collect();
        assert_eq!(own.len(), 1);
        assert_eq!(own[0].role_name.as_deref(), Some("forwardEventAndTarget"));
        if source == original {
            assert_eq!(own[0].binding.declaration.start, 78261);
        }
    }
}

#[test]
fn event_envelope_consumer_identity_and_executed_store_epochs() {
    applied(ENVELOPE, "$THREAD_6_73862", Some("forwardEventAndTarget"));
    for (from, to) in [
        (
            "let consumer=consume;",
            "let consumer; function deferred(){consumer=consume;}",
        ),
        (
            "let consumer=consume;",
            "let consumer; if(flag){consumer=consume;}",
        ),
        (
            "let consumer=consume;",
            "$THREAD_6_73862({});let consumer=consume;",
        ),
        (
            "let consumer=consume;",
            "register($THREAD_6_73862);let consumer=consume;",
        ),
        (
            "let consumer=consume;",
            "let consumer=register($THREAD_6_73862);",
        ),
        (
            "return $THREAD_6_73862;",
            "consumer=other;return $THREAD_6_73862;",
        ),
        (
            "return $THREAD_6_73862;",
            "function deferred(){consumer=other;}return $THREAD_6_73862;",
        ),
        ("return consumer(record)", "return donor(record)"),
        ("return consumer(record)", "return consumer?.(record)"),
        (
            "record.target=event.target;",
            "record.target=event.currentTarget;",
        ),
        (
            "record.target=event.target;",
            "record.target=event.target;record.event=null;",
        ),
        (
            "record.event=event;\n record.target=event.target;",
            "record.target=event.target;\n record.event=event;",
        ),
        ("target:void 0", "target:null"),
        ("return consumer(record)", "return consumer(event)"),
        (
            "function consume(record){return record;}",
            "function consume(record){return record;} consume=other;",
        ),
    ] {
        refuses(ENVELOPE, "$THREAD_6_73862", from, to);
    }
}
