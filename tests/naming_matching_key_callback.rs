use shape::naming::name_threads;

const ROLE: &str = "invokeCapturedCallbackOnMatchingKey";
const BODY: &str = "if(event.key===\"retained\"){callback();}";

fn check(source: &str, expected: bool) {
    let result = name_threads(source, true).unwrap();
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_99_1")
        .unwrap();
    assert_eq!(
        outcome.role_name.as_deref() == Some(ROLE),
        expected,
        "{source}\n{outcome:?}"
    );
    if expected {
        assert!(outcome.applied, "{outcome:?}");
        let mut replay = source.to_owned();
        let mut edits = result.report.edits.clone();
        edits.sort_by_key(|e| std::cmp::Reverse(e.start));
        for edit in edits {
            replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
        }
        assert_eq!(replay, result.source);
    }
}
fn source(body: &str, before: &str, after: &str) -> String {
    format!("function bootstrap(){{function origin(){{return 7;}}{before}function $THREAD_99_1(event){{{body}}}{after}}}bootstrap();")
}

#[test]
fn matching_key_positive_literal_and_origin_variants() {
    for (before, after) in [
        ("const callback=origin;", "return $THREAD_99_1;"),
        ("let callback;", "callback=origin;return $THREAD_99_1;"),
        (
            "let callback=function(){return 7;};",
            "return $THREAD_99_1;",
        ),
        ("function callback(){return 7;}", "return $THREAD_99_1;"),
    ] {
        check(&source(BODY, before, after), true);
    }
    // A different literal remains data, not a vendor/key dictionary match.
    check(
        &source(
            &BODY.replace("retained", "second"),
            "const callback=origin;",
            "return $THREAD_99_1;",
        )
        .replace("second", "retained-second"),
        true,
    );
    // An unreferenced declaration (the primary cached witness) is still owned
    // behavior; no claim that it runs is made.
    check(&source(BODY, "let callback;", "callback=origin;"), true);
}

#[test]
fn matching_key_rejects_owned_body_near_misses() {
    for body in [
        BODY.replace("===", "!=="),
        BODY.replace("===", "=="),
        BODY.replace(".key", ".other"),
        BODY.replace("callback()", "callback(event)"),
        BODY.replace("callback()", "callback(...event)"),
        BODY.replace("callback()", "callback?.()"),
        BODY.replace("callback()", "other()"),
        BODY.replace("callback();", "if(false){callback();}"),
        BODY.replace("callback();", "function donor(){callback();}"),
        BODY.replace("callback();", "let callback=origin;callback();"),
        BODY.replace("\"retained\"", "event.value"),
        format!("{BODY}else{{origin();}}"),
        format!("event=other;{BODY}"),
        format!("{BODY}origin();"),
    ] {
        check(
            &source(&body, "const callback=origin;", "return $THREAD_99_1;"),
            false,
        );
    }
    for parameters in ["event,other", "event=origin()", "...event", "{key}"] {
        check(
            &source(BODY, "const callback=origin;", "return $THREAD_99_1;")
                .replace("_1(event)", &format!("_1({parameters})")),
            false,
        );
    }
}

#[test]
fn matching_key_rejects_mutation_escape_and_early_use() {
    for (before, after) in [
        (
            "let callback=origin;",
            "callback=other;return $THREAD_99_1;",
        ),
        (
            "let callback;",
            "$THREAD_99_1({key:'retained'});callback=origin;return $THREAD_99_1;",
        ),
        (
            "let callback;",
            "const alias=$THREAD_99_1;alias({key:'retained'});callback=origin;return alias;",
        ),
        (
            "const callback=origin;",
            "sink(callback);return $THREAD_99_1;",
        ),
        (
            "const callback=origin;",
            "const escape=callback;return $THREAD_99_1;",
        ),
        ("const callback=origin;", "return [callback,$THREAD_99_1];"),
        (
            "const callback=origin;",
            "origin=other;return $THREAD_99_1;",
        ),
        (
            "let callback;",
            "if(flag){callback=origin;}return $THREAD_99_1;",
        ),
        ("const callback=unknown;", "return $THREAD_99_1;"),
        ("", "return $THREAD_99_1;"),
    ] {
        check(&source(BODY, before, after), false);
    }
    check(
        &source(BODY, "", "return $THREAD_99_1;")
            .replace("function bootstrap()", "function bootstrap(callback)"),
        false,
    );
}

#[test]
fn matching_key_preserves_existing_readable_names() {
    let input = source(BODY, "const callback=origin;", "return $THREAD_99_1;")
        .replace("$THREAD_99_1", "alreadyNamed");
    let result = name_threads(&input, true).unwrap();
    assert_eq!(result.source, input);
}
