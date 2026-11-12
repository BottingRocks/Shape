use shape::naming::name_threads;

const INSTALLER: &str = r#"function main(){
function $THREAD_40_4100(){
function $THREAD_41_4101(target, instanceMembers, staticMembers){
if(instanceMembers){let j=0;for(;j<instanceMembers.length;j++){
instanceMembers[j].enumerable=instanceMembers[j].enumerable||false;
instanceMembers[j].configurable=true;
if("value" in instanceMembers[j]){instanceMembers[j].writable=true;}
host.Object.defineProperty(target.prototype,instanceMembers[j].key,instanceMembers[j]);
}}
if(staticMembers){let i=0;for(;i<staticMembers.length;i++){
staticMembers[i].enumerable=staticMembers[i].enumerable||false;
staticMembers[i].configurable=true;
if("value" in staticMembers[i]){staticMembers[i].writable=true;}
host.Object.defineProperty(target,staticMembers[i].key,staticMembers[i]);
}}
return target;
}
return $THREAD_41_4101;
}
return $THREAD_40_4100;
}
main();"#;

const REGISTER: &str = r#"function main(){
function $THREAD_50_5000(key, detector, serialiser, deserialiser){
this.extensions[key] = { detector: detector, serialiser: serialiser, deserialiser: deserialiser };
}
return $THREAD_50_5000;
}
main();"#;

const EVENT: &str = r#"function main(){
function $THREAD_60_6000(){
let event = host.document.createEvent("CustomEvent");
event.initCustomEvent("afterReady", true, true, {});
host.dispatchEvent.call(void 0, event);
}
return $THREAD_60_6000;
}
main();"#;

const READY: &str = r#"function main(){
function fire(){}
function $THREAD_70_7000(){
if(host.document.readyState === "interactive"){fire();}
}
return $THREAD_70_7000;
}
main();"#;

const ELAPSED: &str = r#"function main(){
function $THREAD_20_1000(context){
let clock;
if(context && context.performance && typeof context.performance.now === "function"){
clock = context.performance.now.bind(context.performance);
}else{
function fallback(){return Number(new b.Date());}
clock = fallback;
}
let read = clock;
let start = clock();
function $THREAD_21_2000(){return read() - start;}
$THREAD_21_2000.start = start;
return $THREAD_21_2000;
}
return $THREAD_20_1000;
}
main();"#;

const READ_ELAPSED: &str = r#"function main(){
let clock = function(){return 0;};
let baseline = clock();
function $THREAD_22_2200(){return clock() - baseline;}
return $THREAD_22_2200;
}
main();"#;

const SERIALISATION: &str = r#"function main(){
function $THREAD_80_8000(){
throw new host.Error("serialisation is hard-coded into the encoder");
}
return $THREAD_80_8000;
}
main();"#;

const NO_GLOBAL: &str = r#"function main(){
function $THREAD_90_9000(){
host.console.error("no global");
}
return $THREAD_90_9000;
}
main();"#;

fn named(source: &str, expected: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report
            .outcomes
            .iter()
            .any(|o| o.applied && o.proposed_name.as_deref() == Some(expected))
    })
}
fn named_any(source: &str, expected: &[&str]) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report.outcomes.iter().any(|o| {
            o.applied
                && o.proposed_name
                    .as_deref()
                    .is_some_and(|n| expected.contains(&n))
        })
    })
}
fn named_by(source: &str, expected: &str, evaluator: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report.function_roles.iter().any(|role| {
            role.candidates
                .iter()
                .any(|candidate| candidate == expected)
                && role.evaluators.iter().any(|claimed| claimed == evaluator)
        })
    })
}
fn refuses(source: &str, role: &str) {
    assert!(!named(source, role), "unexpected {role} for {source}");
}
fn parses(source: &str) -> bool {
    name_threads(source, true).is_ok()
}

#[test]
fn installer_and_register_accept_generated_and_alpha_variants() {
    for source in [
        INSTALLER.to_string(),
        INSTALLER
            .replace("$THREAD_40_4100", "$THREAD_991_12001")
            .replace("$THREAD_41_4101", "$THREAD_992_12002")
            .replace("instanceMembers", "ownMembers")
            .replace("staticMembers", "classMembers")
            .replace("target", "constructor")
            .replace("host.Object", "Object"),
    ] {
        assert!(named(&source, "createClassMembersInstaller"), "{source}");
        assert!(
            named_any(
                &source,
                &["defineClassMembers", "installPropertyDescriptors"]
            ),
            "{source}"
        );
    }
    for source in [
        REGISTER.to_string(),
        REGISTER
            .replace("$THREAD_50_5000", "$THREAD_993_12003")
            .replace("key", "name"),
    ] {
        assert!(named(&source, "registerExtension"), "{source}");
    }
}

#[test]
fn installer_rejects_changed_descriptors_api_order_and_returns() {
    for (from, to) in [
        ("defineProperty(target.prototype", "set(target.prototype"),
        (
            "target.prototype,instanceMembers[j].key",
            "target.prototype,instanceMembers[j].name",
        ),
        (
            "target.prototype,instanceMembers[j].key,instanceMembers[j]",
            "target,instanceMembers[j].key,instanceMembers[j]",
        ),
        (
            "target,staticMembers[i].key,staticMembers[i]",
            "target.prototype,staticMembers[i].key,staticMembers[i]",
        ),
        (
            "instanceMembers[j].configurable=true;",
            "instanceMembers[j].configurable=false;",
        ),
        (
            "instanceMembers[j].enumerable||false",
            "instanceMembers[j].enumerable||true",
        ),
        (
            "if(\"value\" in instanceMembers[j])",
            "if(\"written\" in instanceMembers[j])",
        ),
        ("if(instanceMembers){", "if(instanceMembers && true){"),
        ("return target;", "return target.prototype;"),
        ("return $THREAD_41_4101;", "return target;"),
        ("let j=0;", "using j=0;"),
    ] {
        refuses(&INSTALLER.replace(from, to), "createClassMembersInstaller");
    }
    let resource_cursor = INSTALLER.replace("let j=0;", "using j=0;");
    assert!(parses(&resource_cursor), "{resource_cursor}");
    refuses(&resource_cursor, "createClassMembersInstaller");
    // A local Object-like provider is not the unshadowed Object.defineProperty
    // protocol, even though the member spelling matches.
    let shadowed_provider = INSTALLER.replace(
        "function main(){",
        "function main(){let host={Object:{defineProperty(){}}};",
    );
    assert!(parses(&shadowed_provider), "{shadowed_provider}");
    refuses(&shadowed_provider, "createClassMembersInstaller");
}

#[test]
fn register_extension_rejects_changed_fields_receiver_and_mutation() {
    for (from, to) in [
        ("detector: detector", "detector: serialiser"),
        ("serialiser: serialiser", "serialiser: detector"),
        (
            "{ detector: detector, serialiser: serialiser, deserialiser: deserialiser }",
            "{ detector: detector, serialiser: serialiser }",
        ),
        (
            "{ detector: detector, serialiser: serialiser, deserialiser: deserialiser }",
            "{ detector: detector, serialiser: serialiser, deserialiser: deserialiser, extra: 1 }",
        ),
        ("this.extensions[key]", "other.extensions[key]"),
        ("this.extensions[key]", "this.extensions[detector]"),
        ("this.extensions[key] =", "return this.extensions[key] ="),
        ("this.extensions[key] =", "this.extensions[key] += "),
        (
            "function $THREAD_50_5000(key, detector",
            "function $THREAD_50_5000(key, detector = 0",
        ),
        (
            "function $THREAD_50_5000(key, detector",
            "function $THREAD_50_5000({key}, detector",
        ),
    ] {
        refuses(&REGISTER.replace(from, to), "registerExtension");
    }
}

#[test]
fn custom_event_and_ready_handler_accept_variants_and_refuse_near_misses() {
    for source in [
        EVENT.to_string(),
        EVENT
            .replace("$THREAD_60_6000", "$THREAD_994_12004")
            .replace("event", "notification")
            .replace("afterReady", "ready")
            .replace("host.document", "document")
            .replace("host.dispatchEvent", "document.dispatchEvent"),
        EVENT.replace(
            "host.dispatchEvent.call(void 0, event)",
            "host.dispatchEvent(event)",
        ),
    ] {
        assert!(named(&source, "dispatchBubblingCustomEvent"), "{source}");
        assert!(
            named_by(
                &source,
                "dispatchBubblingCustomEvent",
                "shared_custom_event_dispatch"
            ),
            "not the corrected module claim: {source}"
        );
    }
    for source in [
        READY.to_string(),
        READY
            .replace("$THREAD_70_7000", "$THREAD_995_12005")
            .replace("fire", "dispatcher")
            .replace("host.document", "root.document"),
    ] {
        assert!(named(&source, "handleDocumentReadyStateChange"), "{source}");
        assert!(
            named_by(
                &source,
                "handleDocumentReadyStateChange",
                "shared_ready_state_handler"
            ),
            "not the corrected module claim: {source}"
        );
    }
    for (from, to) in [
        ("\"CustomEvent\"", "\"Event\""),
        ("true, true, {}", "false, true, {}"),
        ("true, true, {}", "true, false, {}"),
        ("true, true, {}", "true, true, {detail:1}"),
        (
            "host.dispatchEvent.call(void 0, event)",
            "host.dispatchEvent.call(void 0, other)",
        ),
        ("event.initCustomEvent", "other.initCustomEvent"),
        ("host.dispatchEvent.call", "host.dispatchEvent"),
        ("let event =", "let event; event ="),
        ("host.dispatchEvent", "target.dispatchEvent"),
        ("host.document.createEvent", "host.createEvent"),
    ] {
        refuses(&EVENT.replace(from, to), "dispatchBubblingCustomEvent");
    }
    refuses(
        &EVENT.replace("function main(){", "function main(){let host = {};"),
        "dispatchBubblingCustomEvent",
    );
    let shadowed_document = EVENT
        .replace("host.document", "document")
        .replace("host.dispatchEvent", "document.dispatchEvent");
    refuses(
        &shadowed_document.replace("function main(){", "function main(){let document = {};"),
        "dispatchBubblingCustomEvent",
    );
    for (from, to) in [
        ("\"interactive\"", "\"complete\""),
        ("readyState", "state"),
        (
            "if(host.document.readyState === \"interactive\"){fire();}",
            "if(host.document.readyState === \"interactive\"){fire(0);}",
        ),
        (
            "if(host.document.readyState === \"interactive\"){fire();}",
            "if(host.document.readyState === \"interactive\"){fire();fire();}",
        ),
        (
            "if(host.document.readyState === \"interactive\"){fire();}",
            "if(host.document.readyState === \"interactive\"){fire();}else{fire();}",
        ),
        (
            "function $THREAD_70_7000(){",
            "function $THREAD_70_7000(fire){",
        ),
        ("function fire(){}", "var fire;"),
        ("function fire(){}", "let fire = 0;"),
        ("function fire(){}", "if(enabled){function fire(){}}"),
        (
            "function fire(){}",
            "for(let i=0;i<1;i++){function fire(){}}",
        ),
        (
            "function fire(){}",
            "let fire; setup(function(){fire=function(){};});",
        ),
        ("function fire(){}", "using fire = function(){};"),
    ] {
        refuses(&READY.replace(from, to), "handleDocumentReadyStateChange");
    }
    let resource_handler = READY.replace("function fire(){}", "using fire = function(){};");
    assert!(parses(&resource_handler), "{resource_handler}");
    refuses(&resource_handler, "handleDocumentReadyStateChange");
    let future_handler = READY.replace("function fire(){}\n", "").replace(
        "return $THREAD_70_7000;",
        "return $THREAD_70_7000;\nvar fire = function(){};",
    );
    refuses(&future_handler, "handleDocumentReadyStateChange");
    refuses(
        &READY.replace("function main(){", "function main(){let host = {};"),
        "handleDocumentReadyStateChange",
    );
}

#[test]
fn elapsed_factory_and_reading_accept_generated_and_alpha_variants() {
    for source in [
        ELAPSED.to_string(),
        ELAPSED
            .replace("$THREAD_20_1000", "$THREAD_996_12006")
            .replace("$THREAD_21_2000", "$THREAD_997_12007")
            .replace("context", "host")
            .replace("clock", "now")
            .replace("read", "reader"),
        ELAPSED.replace(
            "return Number(new b.Date());",
            "return Number(new globalThis.Date());",
        ),
    ] {
        assert!(named(&source, "createElapsedTimer"), "{source}");
        assert!(named(&source, "readElapsedTime"), "{source}");
        assert!(
            named_by(&source, "createElapsedTimer", "shared_elapsed_timer"),
            "not the corrected module claim: {source}"
        );
        assert!(
            named_by(&source, "readElapsedTime", "shared_elapsed_timer"),
            "not the corrected module claim: {source}"
        );
    }
    for source in [
        READ_ELAPSED.to_string(),
        READ_ELAPSED
            .replace("$THREAD_22_2200", "$THREAD_998_12010")
            .replace("clock", "now")
            .replace("baseline", "origin"),
    ] {
        assert!(named(&source, "readElapsedTime"), "{source}");
        assert!(
            named_by(&source, "readElapsedTime", "shared_elapsed_timer"),
            "not the corrected module claim: {source}"
        );
    }
    for (from, to) in [
        ("return clock() - baseline", "return clock() + baseline"),
        ("return clock() - baseline", "return clock - baseline"),
        ("return clock() - baseline", "return clock() - baseline()"),
        (
            "return clock() - baseline;",
            "return clock() - baseline;clock();",
        ),
        (
            "let baseline = clock();",
            "let baseline = clock();baseline = 0;",
        ),
        ("let baseline = clock();", "let baseline = clock;"),
        ("let baseline = clock();", "let baseline = other();"),
    ] {
        refuses(&READ_ELAPSED.replace(from, to), "readElapsedTime");
    }
    for (from, to) in [
        ("context.performance", "context.metric"),
        (
            "context.performance.now.bind(context.performance)",
            "context.performance.now",
        ),
        (
            "$THREAD_21_2000.start = start;",
            "$THREAD_21_2000.origin = start;",
        ),
        ("return read() - start;", "return read() + start;"),
        ("return $THREAD_21_2000;", "return $THREAD_20_1000;"),
        ("let read = clock;", "let read = function(){return 0;};"),
    ] {
        refuses(&ELAPSED.replace(from, to), "createElapsedTimer");
    }
}

#[test]
fn elapsed_guard_order_and_date_fallback_are_exact() {
    for (from, to) in [
        (
            "context && context.performance && typeof context.performance.now === \"function\"",
            "context.performance && context && typeof context.performance.now === \"function\"",
        ),
        (
            "context && context.performance && typeof context.performance.now === \"function\"",
            "context && context.performance && context.performance && typeof context.performance.now === \"function\"",
        ),
        (
            "context && context.performance && typeof context.performance.now === \"function\"",
            "context && context.performance && true && typeof context.performance.now === \"function\"",
        ),
        (
            "context && context.performance && typeof context.performance.now === \"function\"",
            "context.performance && typeof context.performance.now === \"function\"",
        ),
        ("return Number(new b.Date());", "console.log(1);return Number(new b.Date());"),
        ("return Number(new b.Date());", "return 0;"),
        ("return Number(new b.Date());", "return Number(new b.Date()) + 0;"),
        ("return Number(new b.Date());", "return String(new b.Date());"),
        ("return Number(new b.Date());", "return Number(new b.Date(1));"),
        ("return Number(new b.Date());", "return Number(new b.Date(), 1);"),
        ("return Number(new b.Date());", "return Number(new Error());"),
        (
            "clock = fallback;",
            "clock = fallback; fallback = function(){return 0;};",
        ),
        (
            "let read = clock;",
            "clock = function(){return 0;}; let read = clock;",
        ),
    ] {
        refuses(&ELAPSED.replace(from, to), "createElapsedTimer");
    }
}

#[test]
fn standalone_elapsed_reader_requires_ordered_callable_origin() {
    for (from, to) in [
        ("let clock = function(){return 0;};", "let clock = 0;"),
        ("let clock = function(){return 0;};", "let clock;"),
        ("let clock = function(){return 0;};", "let clock = {v:0};"),
        (
            "let clock = function(){return 0;};",
            "if(enabled){var clock = function(){return 0;};}",
        ),
        (
            "let clock = function(){return 0;};",
            "for(let i=0;i<1;i++){var clock = function(){return 0;};}",
        ),
        (
            "let clock = function(){return 0;};",
            "let clock; setup(function(){ clock = function(){return 0;}; });",
        ),
        (
            "let clock = function(){return 0;};",
            "using clock = function(){return 0;};",
        ),
        (
            "let baseline = clock();",
            "if(enabled){var baseline = clock();}",
        ),
        (
            "let baseline = clock();",
            "for(let i=0;i<1;i++){var baseline = clock();}",
        ),
    ] {
        refuses(&READ_ELAPSED.replace(from, to), "readElapsedTime");
    }
    let resource = READ_ELAPSED.replace(
        "let clock = function(){return 0;};",
        "using clock = function(){return 0;};",
    );
    assert!(parses(&resource), "{resource}");
    refuses(&resource, "readElapsedTime");
    let future = READ_ELAPSED.replace(
        "let clock = function(){return 0;};\nlet baseline = clock();",
        "let baseline = clock();\nlet clock = function(){return 0;};",
    );
    refuses(&future, "readElapsedTime");
}

#[test]
fn serialisation_error_and_console_report_are_namespace_checked() {
    for source in [
        SERIALISATION.to_string(),
        SERIALISATION.replace("$THREAD_80_8000", "$THREAD_998_12008"),
    ] {
        assert!(
            named(&source, "throwHardcodedSerializationError"),
            "{source}"
        );
    }
    for source in [
        NO_GLOBAL.to_string(),
        NO_GLOBAL.replace("$THREAD_90_9000", "$THREAD_999_12009"),
        NO_GLOBAL.replace("host.console.error", "console.error"),
    ] {
        assert!(named(&source, "reportErrorToConsole"), "{source}");
        assert!(
            named_by(
                &source,
                "reportErrorToConsole",
                "shared_console_error_report"
            ),
            "not the normalized console claim: {source}"
        );
        assert!(!named(&source, "initWithoutGlobal"), "{source}");
    }
    for (from, to) in [
        (
            "\"serialisation is hard-coded into the encoder\"",
            "\"serialisation failed\"",
        ),
        ("new host.Error(", "new host.TypeError("),
        ("throw new host.Error", "return new host.Error"),
    ] {
        refuses(
            &SERIALISATION.replace(from, to),
            "throwHardcodedSerializationError",
        );
    }
    refuses(
        &SERIALISATION.replace("function main(){", "function main(){let host = {};"),
        "throwHardcodedSerializationError",
    );
    for (from, to) in [
        ("\"no global\"", "\"global missing\""),
        ("host.console.error", "host.console.warn"),
        ("host.console.error", "host.console"),
        (
            "function $THREAD_90_9000(){",
            "function $THREAD_90_9000(x){",
        ),
    ] {
        refuses(&NO_GLOBAL.replace(from, to), "reportErrorToConsole");
    }
    refuses(
        &NO_GLOBAL.replace("function main(){", "function main(){let host = {};"),
        "reportErrorToConsole",
    );
    let shadowed_console = NO_GLOBAL
        .replace("host.console.error", "console.error")
        .replace(
            "function main(){",
            "function main(){let console = {error(){}};",
        );
    refuses(&shadowed_console, "reportErrorToConsole");
}

#[test]
fn meaningful_names_and_collision_guards_are_preserved() {
    for source in [
        ELAPSED
            .replace("$THREAD_20_1000", "createTimer")
            .replace("$THREAD_21_2000", "readTimer"),
        INSTALLER
            .replace("$THREAD_40_4100", "createInstaller")
            .replace("$THREAD_41_4101", "defineClassMembers"),
        REGISTER.replace("$THREAD_50_5000", "registerExtension"),
        EVENT.replace("$THREAD_60_6000", "dispatchBubblingCustomEvent"),
        READY.replace("$THREAD_70_7000", "handleDocumentReadyStateChange"),
        SERIALISATION.replace("$THREAD_80_8000", "throwHardcodedSerializationError"),
        NO_GLOBAL.replace("$THREAD_90_9000", "reportErrorToConsole"),
    ] {
        assert_eq!(name_threads(&source, true).unwrap().source, source);
    }
    refuses(
        &REGISTER.replace(
            "function main(){",
            "function main(){let registerExtension=1;",
        ),
        "registerExtension",
    );
    refuses(
        &EVENT.replace(
            "function main(){",
            "function main(){let dispatchBubblingCustomEvent=1;",
        ),
        "dispatchBubblingCustomEvent",
    );
    refuses(
        &ELAPSED.replace(
            "function main(){",
            "function main(){let createElapsedTimer=1;",
        ),
        "createElapsedTimer",
    );
    refuses(
        &NO_GLOBAL.replace(
            "function main(){",
            "function main(){let reportErrorToConsole=1;",
        ),
        "reportErrorToConsole",
    );
}
