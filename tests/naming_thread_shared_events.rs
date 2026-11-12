use shape::naming::name_threads;

const COOKIE: &str = r#"function main(){let doc;function setup(context){doc=context.global.document;}
function $THREAD_71_2468(observer){let sink=observer;
doc.cookie.split('; ').forEach(entry=>{let parts=entry.split('=');
if(parts.length>0&&parts[0]==='session'){sink.next({value:parts[1]});sink.complete();}});
sink.next({});sink.complete();}return $THREAD_71_2468;}main();"#;
const MESSAGE: &str = "function main(){function send(record){return record;}function $THREAD_86_7531(value){send({message:value});}return $THREAD_86_7531;}main();";

fn named(source: &str, expected: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report
            .outcomes
            .iter()
            .any(|o| o.applied && o.proposed_name.as_deref() == Some(expected))
    })
}
fn refuses(source: &str, role: &str) {
    assert!(!named(source, role), "unexpected {role} for {source}");
}

#[test]
fn cookie_scoped_callback_and_alpha_variants() {
    for source in [
        COOKIE.to_string(),
        COOKIE
            .replace("$THREAD_71_2468", "$THREAD_903_71902_2")
            .replace("observer", "$KEY_400")
            .replace("entry", "item")
            .replace("parts", "pieces")
            .replace("doc", "capturedDocument")
            .replace("session", "other-cookie"),
        COOKIE
            .replace("let sink=observer;", "")
            .replace("sink.", "observer."),
        COOKIE.replace(
            "let doc;function setup(context){doc=context.global.document;}",
            "const doc=window.document;",
        ),
        COOKIE.replace("let parts", "const parts"),
    ] {
        assert!(named(&source, "emitMatchingCookieValue"), "{source}");
    }
}

#[test]
fn cookie_api_arguments_order_literals_and_returns_are_proven() {
    for (from, to) in [
        ("doc.cookie", "doc.storage"),
        ("split('; ')", "split(';')"),
        ("split('=')", "split(':')"),
        (".forEach(", ".map("),
        ("parts.length>0", "parts.length>=0"),
        ("parts[0]===", "parts[1]==="),
        ("==='session'", "==session"),
        ("==='session'", "===''"),
        ("{value:parts[1]}", "{value:parts[0]}"),
        ("sink.complete();}}", "sink.error();}}"),
        (
            "sink.next({});sink.complete();",
            "sink.complete();sink.next({});",
        ),
        (
            "sink.next({value:parts[1]})",
            "return sink.next({value:parts[1]})",
        ),
        (
            "sink.next({value:parts[1]})",
            "sink.next({value:parts[1]},0)",
        ),
        ("entry.split('=')", "entry?.split('=')"),
        ("sink.next({})", "sink?.next({})"),
        ("sink.next({})", "sink.next({...other})"),
        (
            "sink.next({value:parts[1]})",
            "sink.next({['value']:parts[1]})",
        ),
    ] {
        refuses(&COOKIE.replace(from, to), "emitMatchingCookieValue");
    }
}

#[test]
fn cookie_rejects_binding_mutations_and_wrong_capture_targets() {
    for (from, to) in [
        (
            "sink.next({value:parts[1]})",
            "other.next({value:parts[1]})",
        ),
        ("entry.split('=')", "other.split('=')"),
        ("let sink=observer;", "let sink=other;"),
        ("let sink=observer;", "let sink=observer;sink=other;"),
        (
            "let parts=entry.split('=');",
            "var parts=entry.split('=');var parts;",
        ),
        ("let doc;", "var doc;var doc;"),
        (
            "doc=context.global.document;",
            "doc=context.global.document;doc=other;",
        ),
        ("entry=>", "(entry,extra)=>"),
        ("entry=>", "async entry=>"),
        ("entry=>", "(entry=other)=>"),
        ("entry=>", "(...entry)=>"),
        ("entry=>", "({entry})=>"),
        ("(observer)", "(observer=other)"),
        ("(observer)", "(...observer)"),
        ("(observer)", "({observer})"),
        ("function main(){let doc;", "function main(doc){"),
    ] {
        refuses(&COOKIE.replace(from, to), "emitMatchingCookieValue");
    }
}

#[test]
fn cookie_preserves_human_names_and_collisions() {
    let source = COOKIE.replace("$THREAD_71_2468", "readCookie");
    assert_eq!(name_threads(&source, true).unwrap().source, source);
    refuses(
        &COOKIE.replace("let doc;", "let doc;let emitMatchingCookieValue=1;"),
        "emitMatchingCookieValue",
    );
    refuses(
        &COOKIE.replace(
            "let sink=observer;",
            "let sink=observer;function emitMatchingCookieValue(){}",
        ),
        "emitMatchingCookieValue",
    );
}

#[test]
fn message_envelope_accepts_stable_scoped_origins() {
    for source in [
        MESSAGE.to_string(),
        MESSAGE
            .replace("$THREAD_86_7531", "$THREAD_999_123_3")
            .replace("value", "$KEY_321")
            .replace("send", "deliver"),
        MESSAGE
            .replace("send({message:value})", "alias({message:value})")
            .replace("function $THREAD", "const alias=send;function $THREAD"),
        MESSAGE
            .replace("send({message:value})", "alias({message:value})")
            .replace("function $THREAD", "let alias;alias=send;function $THREAD"),
        MESSAGE.replace(
            "function send(record){return record;}",
            "const send=function(record){return record;};",
        ),
    ] {
        assert!(named(&source, "forwardMessageRecord"), "{source}");
    }
}

#[test]
fn message_envelope_rejects_changed_record_call_and_formals() {
    for (from, to) in [
        ("{message:value}", "{value:value}"),
        ("{message:value}", "{message:other}"),
        ("{message:value}", "{message:value,extra:1}"),
        ("{message:value}", "{['message']:value}"),
        ("send({message:value})", "return send({message:value})"),
        ("send({message:value})", "send({message:value},0)"),
        ("send({message:value})", "send?.({message:value})"),
        ("send({message:value})", "$THREAD_86_7531({message:value})"),
        ("(value)", "(value=other)"),
        ("(value)", "(...value)"),
        ("(value)", "({value})"),
        ("send({message:value})", "value=other;send({message:value})"),
    ] {
        refuses(&MESSAGE.replace(from, to), "forwardMessageRecord");
    }
}

#[test]
fn message_callable_origins_refuse_parameters_redeclarations_and_mutation() {
    for source in [
        MESSAGE.replace(
            "function main(){function send(record){return record;}",
            "function main(send){send=function(record){return record;};",
        ),
        MESSAGE.replace("function $THREAD", "send=other;function $THREAD"),
        MESSAGE.replace("function $THREAD", "function send(){}function $THREAD"),
        MESSAGE.replace("function send(record){return record;}", "let {send}=other;"),
        MESSAGE.replace("function send(record){return record;}", "let send;"),
        MESSAGE
            .replace("send({message:value})", "alias({message:value})")
            .replace(
                "function $THREAD",
                "let alias=send;alias=other;function $THREAD",
            ),
        MESSAGE
            .replace("send({message:value})", "alias({message:value})")
            .replace(
                "function $THREAD",
                "var alias=send;var alias;function $THREAD",
            ),
        MESSAGE.replace(
            "function send(record){return record;}",
            "function other(){}let send=other;send=other;",
        ),
    ] {
        refuses(&source, "forwardMessageRecord");
    }
}

#[test]
fn message_envelope_preserves_human_names_and_collision_guards() {
    let source = MESSAGE.replace("$THREAD_86_7531", "notify");
    assert_eq!(name_threads(&source, true).unwrap().source, source);
    refuses(
        &MESSAGE.replace(
            "function main(){",
            "function main(){let forwardMessageRecord=1;",
        ),
        "forwardMessageRecord",
    );
    refuses(
        &MESSAGE.replace(
            "send({message:value})",
            "function forwardMessageRecord(){}send({message:value})",
        ),
        "forwardMessageRecord",
    );
}

#[test]
fn message_record_and_value_aliases_are_sequential_and_scoped() {
    for body in [
        "const payload=value;send({message:payload});",
        "const record={message:value};send(record);",
        "const payload=value;const record={message:payload};const envelope=record;send(envelope);",
    ] {
        let source = MESSAGE.replace("send({message:value});", body);
        assert!(named(&source, "forwardMessageRecord"), "{source}");
        assert!(named(
            &source
                .replace("value", "item")
                .replace("record", "packet")
                .replace("$THREAD_86_7531", "$THREAD_901_282_2"),
            "forwardMessageRecord"
        ));
    }
}
#[test]
fn message_aliases_refuse_future_init_mutation_and_wrong_record() {
    for body in [
        "const record={message:payload};const payload=value;send(record);",
        "const payload=other;send({message:payload});",
        "let payload=value;payload=other;send({message:payload});",
        "var payload=value;var payload;send({message:payload});",
        "const {payload}=value;send({message:payload});",
        "const record={message:value,extra:0};send(record);",
        "const record={message:value};return send(record);",
        "const record={message:value};send(other);",
        "const record={message:value};send(record,0);",
    ] {
        refuses(
            &MESSAGE.replace("send({message:value});", body),
            "forwardMessageRecord",
        );
    }
    refuses(
        &MESSAGE
            .replace(
                "function $THREAD",
                "const alias=future;const future=send;function $THREAD",
            )
            .replace("send({message:value})", "alias({message:value})"),
        "forwardMessageRecord",
    );
    refuses(
        &MESSAGE
            .replace(
                "return $THREAD_86_7531;",
                "let send=function(record){return record;};return $THREAD_86_7531;",
            )
            .replace("function send(record){return record;}", ""),
        "forwardMessageRecord",
    );
}
