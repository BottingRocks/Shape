use shape::naming::name_threads;

const NATIVE: &str = r#"let matcher=RegExp("{\\s*\\[\\s*native\\s+code\\s*]\\s*}\\s*$");let pattern;pattern=matcher;
function $THREAD_71_8123(value){if(typeof value === "function" && value.toString === Function.prototype.toString){return !!pattern.exec(value.toString());}else{return false;}}"#;
const UNITS: &str = r#"function $THREAD_71_8123(input){let output=[];if(input != null){input.forEach(code=>{output.push(String.fromCharCode(code));});}return output.join("");}"#;
const PREVENT: &str = "function $THREAD_71_8123(evt){evt.preventDefault();}";
const PROTO: &str = "function $THREAD_71_8123(object,proto){let setter;if(Object.setPrototypeOf){setter=Object.setPrototypeOf.bind();}else{function fallback(target,parent){target.__proto__=parent;return target;}setter=fallback;}return setter(object,proto);}";
fn wrap(body: &str) -> String {
    format!("function main(){{{body}return $THREAD_71_8123;}}main();")
}
fn named(source: &str, expected: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report
            .outcomes
            .iter()
            .any(|o| o.applied && o.proposed_name.as_deref() == Some(expected))
    })
}
fn refuses(template: &str, expected: &str, changes: &[(&str, &str)]) {
    for &(from, to) in changes {
        assert!(template.contains(from), "missing replacement {from}");
        let source = wrap(&template.replace(from, to));
        assert!(!named(&source, expected), "accepted {from} -> {to}");
    }
}
#[test]
fn portable_runtime_roles_and_alpha_renaming() {
    for (body, name) in [
        (NATIVE, "isNativeFunction"),
        (UNITS, "codeUnitsToString"),
        (PREVENT, "preventDefaultHandler"),
        (PROTO, "setObjectPrototype"),
    ] {
        let source = wrap(body);
        assert!(named(&source, name), "missing {name}");
        assert!(named(
            &source
                .replace("$THREAD_71_8123", "$THREAD_900_777_2")
                .replace("value", "$KEY_902")
                .replace("matcher", "$KEY_15")
                .replace("pattern", "alias")
                .replace("input", "$KEY_400")
                .replace("output", "buffer")
                .replace("evt", "$KEY_888")
                .replace("setter", "selected")
                .replace("fallback", "helper")
                .replace("target", "item")
                .replace("parent", "base"),
            name
        ));
    }
    assert!(named(
        &wrap(&UNITS.replace(
            "code=>{output.push(String.fromCharCode(code));}",
            "code=>output.push(String.fromCharCode(code))"
        )),
        "codeUnitsToString"
    ));
    assert!(named(
        &wrap(&NATIVE.replace("let pattern;pattern=matcher;", "let pattern=matcher;")),
        "isNativeFunction"
    ));
}
#[test]
fn native_predicate_requires_exact_guard_calls_returns_and_regex_origin() {
    refuses(
        NATIVE,
        "isNativeFunction",
        &[
            ("typeof value", "typeof other"),
            ("=== \"function\"", "=== \"object\""),
            ("value.toString ===", "other.toString ==="),
            ("&&", "||"),
            ("Function.prototype.toString", "Function.prototype.call"),
            ("pattern.exec", "pattern.test"),
            ("value.toString()", "other.toString()"),
            ("value.toString()", "value.toString(1)"),
            ("return !!pattern", "return !pattern"),
            ("return false", "return true"),
            (
                "let pattern;pattern=matcher;",
                "let pattern;pattern=matcher;pattern=other;",
            ),
            (
                "let pattern;pattern=matcher;",
                "var pattern=matcher;var pattern;",
            ),
            (
                "let pattern;pattern=matcher;",
                "let pattern;for(pattern of values){}",
            ),
            ("let matcher=RegExp", "let Function={};let matcher=RegExp"),
            ("let matcher=RegExp", "let RegExp=other;let matcher=RegExp"),
            ("code", "bytecode"),
            ("value){", "value=other){"),
            ("value){", "...value){"),
            ("value){", "{value}){"),
            (
                "pattern.exec(value.toString())",
                "pattern.exec(value.toString(),1)",
            ),
            (
                "let pattern;pattern=matcher;",
                "let pattern=matcher;matcher=other;",
            ),
            (
                "function $THREAD_71_8123(value)",
                "function $THREAD_71_8123(value,pattern)",
            ),
        ],
    );
}
#[test]
fn code_unit_conversion_owns_callback_and_output_identity() {
    refuses(
        UNITS,
        "codeUnitsToString",
        &[
            ("input != null", "other != null"),
            ("input != null", "input == null"),
            ("input.forEach", "input.map"),
            ("code=>", "(code,extra)=>"),
            ("code=>", "(code=0)=>"),
            ("code=>", "(...code)=>"),
            ("code=>", "({code})=>"),
            ("code=>", "async code=>"),
            ("output.push", "other.push"),
            ("fromCharCode(code)", "fromCharCode(other)"),
            ("fromCharCode(code)", "fromCharCode(code,0)"),
            ("String.fromCharCode", "String.fromCodePoint"),
            ("let output=[]", "let String=other;let output=[]"),
            ("return output.join", "return other.join"),
            ("join(\"\")", "join(\",\")"),
            ("let output=[]", "var output=[];var output"),
            ("output.push", "code=0;output.push"),
            ("let output=[]", "let output=[0]"),
            ("input){", "input=[]) {"),
            ("input){", "...input){"),
            ("input){", "{input}){"),
            ("output.push", "return output.push"),
        ],
    );
}
#[test]
fn event_cancellation_requires_sole_action_on_sole_plain_parameter() {
    refuses(
        PREVENT,
        "preventDefaultHandler",
        &[
            ("evt.preventDefault", "other.preventDefault"),
            ("preventDefault", "stopPropagation"),
            ("preventDefault()", "preventDefault(1)"),
            ("evt.preventDefault();", "return evt.preventDefault();"),
            ("evt){", "evt=other){"),
            ("evt){", "...evt){"),
            ("evt){", "{evt}){"),
            ("evt.preventDefault();", "evt=other;evt.preventDefault();"),
            ("evt.preventDefault();", "var evt;evt.preventDefault();"),
            ("evt.preventDefault();", "evt.preventDefault();other();"),
        ],
    );
}
#[test]
fn prototype_selector_proves_nested_fallback_and_branch_local_identity() {
    refuses(
        PROTO,
        "setObjectPrototype",
        &[
            ("if(Object.setPrototypeOf)", "if(Object.getPrototypeOf)"),
            (
                "Object.setPrototypeOf.bind()",
                "Object.getPrototypeOf.bind()",
            ),
            (
                "Object.setPrototypeOf.bind()",
                "Object.setPrototypeOf.bind(other)",
            ),
            ("target.__proto__=parent", "target.__proto__=target"),
            ("return target", "return parent"),
            ("setter=fallback", "setter=other"),
            ("setter(object,proto)", "setter(proto,object)"),
            ("setter(object,proto)", "setter(object)"),
            ("let setter;", "let Object=other;let setter;"),
            ("let setter;", "var setter;var setter;"),
            ("setter=fallback;", "setter=fallback;fallback=other;"),
            (
                "function fallback(target,parent)",
                "function fallback(target,parent=other)",
            ),
            (
                "function fallback(target,parent)",
                "function fallback(target,...parent)",
            ),
            (
                "function fallback(target,parent)",
                "function fallback({target},parent)",
            ),
            ("target.__proto__=parent", "other.__proto__=parent"),
            ("object,proto){", "object,proto=other){"),
        ],
    );
}
#[test]
fn human_and_collision_names_stay_protected() {
    for (body, name) in [
        (PREVENT, "preventDefaultHandler"),
        (UNITS, "codeUnitsToString"),
        (NATIVE, "isNativeFunction"),
        (PROTO, "setObjectPrototype"),
    ] {
        let source = wrap(body).replace("$THREAD_71_8123", "humanReadable");
        assert_eq!(name_threads(&source, true).unwrap().source, source);
        assert!(!named(&wrap(&format!("let {name}=42;{body}")), name));
    }
}

#[test]
fn native_regex_alias_cannot_copy_a_future_initializer() {
    let changed = NATIVE
        .replace("let matcher=", "var pattern=matcher;var matcher=")
        .replace("let pattern;pattern=matcher;", "");
    assert!(!named(&wrap(&changed), "isNativeFunction"));
}

#[test]
fn epoch_clock_proves_host_and_constructor_alias_origins() {
    for body in [
        "function $THREAD_71_8123(){return Date.now();}",
        "function $THREAD_71_8123(){return Number(new Date());}",
        "const host=globalThis;const Calendar=host.Date;const tick=Calendar.now;function $THREAD_71_8123(){return tick();}",
        "const host=window;const Calendar=host.Date;function $THREAD_71_8123(){return Number(new Calendar());}",
        "let Calendar;Calendar=self.Date;function $THREAD_71_8123(){return Number(new Calendar());}",
    ] {
        let source = wrap(body);
        assert!(named(&source, "getEpochMilliseconds"), "{source}");
        assert!(named(&source.replace("Calendar", "Renamed").replace("$THREAD_71_8123", "$THREAD_929_48484"), "getEpochMilliseconds"));
    }
}

#[test]
fn epoch_clock_refuses_wrong_operations_arguments_and_unproven_origins() {
    let body="const host=globalThis;const Calendar=host.Date;function $THREAD_71_8123(){return Number(new Calendar());}";
    refuses(
        body,
        "getEpochMilliseconds",
        &[
            ("new Calendar()", "new Calendar(1)"),
            ("Number(new Calendar())", "Number(new Calendar(),1)"),
            ("Number(new Calendar())", "String(new Calendar())"),
            ("return Number", "Number"),
            ("host.Date", "host.Other"),
            ("host.Date", "host?.Date"),
            ("const host=globalThis;", "const host=unknown;"),
            ("const host=globalThis;", "const host={Date:Date};"),
            ("const host=globalThis;", "let host=globalThis;host=other;"),
            ("const host=globalThis;", "var host=globalThis;var host;"),
            ("const host=globalThis;", ""),
            ("const Calendar=host.Date;", "let {Calendar}=host;"),
            ("const Calendar=host.Date;", "for(var Calendar of values){}"),
            ("const Calendar=host.Date;", "for(var Calendar in values){}"),
            (
                "const Calendar=host.Date;",
                "let Calendar=host.Date;Calendar=other;",
            ),
            ("(){return", "(Calendar){return"),
            ("(){return", "(Number){return"),
            ("(){return", "(){let Number=other;return"),
        ],
    );
    for source in [
        wrap("function $THREAD_71_8123(){return Date.now(1);}"),
        wrap("function $THREAD_71_8123(){return Date.now?.();}"),
        wrap("function $THREAD_71_8123(){return Number(new Calendar());}const Calendar=Date;"),
        wrap(body).replace("main(){", "main(globalThis){"),
        wrap(body)
            .replace("main(){", "main(host){")
            .replace("const host=globalThis;", "host=globalThis;"),
        wrap(body)
            .replace("main(){", "main({host}){")
            .replace("const host=globalThis;", "host=globalThis;"),
    ] {
        assert!(!named(&source, "getEpochMilliseconds"), "{source}");
    }
}

const ELAPSED: &str = "const clock=performance.now.bind(performance);const baseline=clock();function $THREAD_71_8123(){return clock()-baseline;}";

#[test]
fn elapsed_clock_requires_same_proven_callable_and_baseline() {
    for body in [
        ELAPSED.to_string(),
        ELAPSED.replace("performance.now.bind(performance)", "Date.now"),
        ELAPSED.replace("performance.now.bind(performance)", "Date.now.bind(Date)"),
        ELAPSED.replace("const clock=performance.now.bind(performance);", "function clock(){return Number(new Date());}"),
        ELAPSED.replace("const clock=performance.now.bind(performance);", "const host=globalThis;const receiver=host.performance;const clock=receiver.now.bind(receiver);"),
    ] {
        let source=wrap(&body);
        assert!(named(&source, "getElapsedMilliseconds"), "{source}");
        assert!(named(&source.replace("baseline", "origin").replace("clock", "readTime").replace("$THREAD_71_8123", "$THREAD_19_7747"), "getElapsedMilliseconds"));
    }
}

#[test]
fn elapsed_clock_refuses_receiver_capture_and_dataflow_near_misses() {
    refuses(
        ELAPSED,
        "getElapsedMilliseconds",
        &[
            ("clock()-baseline", "baseline-clock()"),
            ("clock()-baseline", "clock()+baseline"),
            ("clock()-baseline", "clock()-other"),
            ("clock()-baseline", "clock(1)-baseline"),
            ("baseline=clock()", "baseline=clock(1)"),
            ("baseline=clock()", "baseline=Date.now()"),
            (
                "performance.now.bind(performance)",
                "performance.now.bind(other)",
            ),
            (
                "performance.now.bind(performance)",
                "performance.now.bind(performance,1)",
            ),
            ("performance.now.bind(performance)", "performance.now"),
            (
                "performance.now.bind(performance)",
                "performance.other.bind(performance)",
            ),
            (
                "const clock=performance.now.bind(performance);",
                "let clock=performance.now.bind(performance);clock=other;",
            ),
            (
                "const clock=performance.now.bind(performance);",
                "var clock=performance.now.bind(performance);var clock;",
            ),
            (
                "const clock=performance.now.bind(performance);",
                "const {clock}=performance;",
            ),
            (
                "const baseline=clock();",
                "let baseline=clock();baseline=0;",
            ),
            ("(){return", "(clock){return"),
            ("(){return", "(baseline){return"),
            ("return clock()-baseline;", "clock()-baseline;"),
        ],
    );
    let future = wrap(ELAPSED)
        .replace("const clock=performance.now.bind(performance);", "")
        .replace(
            "return $THREAD_71_8123;",
            "const clock=performance.now.bind(performance);return $THREAD_71_8123;",
        );
    assert!(!named(&future, "getElapsedMilliseconds"));
    assert!(!named(
        &wrap(ELAPSED).replace("main(){", "main(performance){"),
        "getElapsedMilliseconds"
    ));
}

#[test]
fn host_aliases_extend_existing_native_and_code_unit_proofs() {
    for body in [
        format!("const host=globalThis;{NATIVE}")
            .replace("Function.prototype", "host.Function.prototype")
            .replace("=RegExp(", "=host.RegExp("),
        format!("const host=window;const Text=host.String;{UNITS}")
            .replace("String.fromCharCode", "Text.fromCharCode"),
    ] {
        let expected = if body.contains("native") {
            "isNativeFunction"
        } else {
            "codeUnitsToString"
        };
        assert!(named(&wrap(&body), expected));
        assert!(!named(
            &wrap(&body.replace("const host=", "let host=")).replace(
                "return $THREAD_71_8123;",
                "host=other;return $THREAD_71_8123;"
            ),
            expected
        ));
    }
}

#[test]
fn clock_names_preserve_meaningful_names_and_scope_collisions() {
    let body = "function $THREAD_71_8123(){return Number(new Date());}";
    let human = wrap(&body.replace("$THREAD_71_8123", "humanClock"));
    assert_eq!(name_threads(&human, true).unwrap().source, human);
    assert!(!named(
        &wrap(&format!("let getEpochMilliseconds=1;{body}")),
        "getEpochMilliseconds"
    ));
    assert!(!named(
        &wrap(ELAPSED).replace("main(){", "main(getElapsedMilliseconds){"),
        "getElapsedMilliseconds"
    ));
}

#[test]
fn native_api_and_clock_origins_refuse_setup_conditional_and_loop_assignments() {
    for origin in [
        "let Calendar;function setup(){Calendar=Date;}",
        "var Calendar;for(;ready;){Calendar=Date;}",
        "var Calendar;if(ready){Calendar=Date;}",
        "if(ready){var Calendar=Date;}",
        "using Calendar=Date;",
    ] {
        let source = wrap(&format!(
            "{origin}function $THREAD_71_8123(){{return Number(new Calendar());}}"
        ));
        assert!(!named(&source, "getEpochMilliseconds"), "{source}");
    }
    for origin in [
        "let clock;function setup(){clock=Date.now;}",
        "var clock;if(ready){clock=Date.now;}",
        "var clock;for(;ready;){clock=Date.now;}",
    ] {
        let source =
            wrap(&ELAPSED.replace("const clock=performance.now.bind(performance);", origin));
        assert!(!named(&source, "getElapsedMilliseconds"), "{source}");
    }
}
