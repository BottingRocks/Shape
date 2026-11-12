use shape::naming::name_threads;

const FIXTURE: &str = include_str!("fixtures/submit-notification/stateful-notification.js");
const NOTIFIER: &str = "$THREAD_4_139072";
const FORWARDERS: [&str; 2] = ["$THREAD_1_86348", "$THREAD_7_110216"];
const EVALUATOR: &str = "submit_notification_protocol";

fn check(source: &str, wanted: &[&str]) {
    let result = name_threads(source, true).expect("valid scoped program");
    for name in [NOTIFIER, FORWARDERS[0], FORWARDERS[1]] {
        let o = result
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == name)
            .unwrap();
        assert_eq!(
            o.evaluator.as_deref() == Some(EVALUATOR),
            wanted.contains(&name),
            "{name}: {o:?}"
        );
    }
    let mut replay = source.to_string();
    let mut edits = result.report.edits.clone();
    edits.sort_by_key(|e| std::cmp::Reverse(e.start));
    for e in edits {
        replay.replace_range(e.start as usize..e.end as usize, &e.replacement);
    }
    assert_eq!(replay, result.source);
    for key in [
        "removeEventListener",
        "submit",
        "defaultPrevented",
        "notify",
        "form:",
        "event:",
    ] {
        assert_eq!(
            source.matches(key).count(),
            result.source.matches(key).count()
        );
    }
}

#[test]
fn original_and_alpha_protocol() {
    check(FIXTURE, &[NOTIFIER, FORWARDERS[0], FORWARDERS[1]]);
    let alpha = FIXTURE
        .replace("$CG", "renamedEvent")
        .replace("$KEY_9", "capturedTarget")
        .replace("$KEY_7", "publishedNotifier")
        .replace("$KEY_4", "firstForwarderSlot")
        .replace("$KEY_5", "secondForwarderSlot")
        .replace("$KEY_240", "notifications");
    check(&alpha, &[NOTIFIER, FORWARDERS[0], FORWARDERS[1]]);
}

#[test]
fn changed_notifier_rejects_whole_linked_family() {
    for (from, to) in [
        ("$CG.defaultPrevented", "$CG.cancelable"),
        ("$CG != null &&", "$CG == null &&"),
        ("form: $KEY_9", "form: $KEY_3"),
        ("event: $CG", "event: $KEY_9"),
        ("$KEY_240.notify({", "$KEY_240.emit({"),
        (
            "$KEY_390.notify(\"2.27:\" + e)",
            "$KEY_390.notify(\"2.27:\" + $KEY_9)",
        ),
        ("$KEY_7 = $THREAD_4_139072", "$KEY_7 = unrelated"),
        ("$KEY_240,$KEY_390", "$KEY_390"),
    ] {
        assert!(FIXTURE.contains(from));
        check(&FIXTURE.replace(from, to), &[]);
    }
}

#[test]
fn forwarder_semantics_and_exact_publication() {
    for (from, to) in [
        ("$CG.type !== \"submit\"", "$CG.type === \"submit\""),
        ("$CG.target !== $KEY_9", "$CG.target !== $KEY_3"),
        ("$KEY_7($CG)", "$KEY_7($KEY_9)"),
        ("$KEY_4 = $THREAD_1_86348", "$KEY_4 = $THREAD_7_110216"),
        ("$KEY_5 = $THREAD_7_110216", "$KEY_5 = $THREAD_1_86348"),
    ] {
        check(&FIXTURE.replace(from, to), &[NOTIFIER]);
    }
}

#[test]
fn shadow_sibling_and_late_origin_do_not_donate_proof() {
    let shadow = FIXTURE.replace(
        "function $THREAD_1_86348($CG) {",
        "function $THREAD_1_86348($CG) { let $KEY_7;",
    );
    check(&shadow, &[NOTIFIER, FORWARDERS[1]]);
    let sibling = FIXTURE.replace(
        "$KEY_7 = $THREAD_4_139072;",
        "function sibling(){let $KEY_7; $KEY_7 = $THREAD_4_139072;}",
    );
    check(&sibling, &[]);
    let late = FIXTURE.replace("let $KEY_7;", "").replace(
        "$KEY_7 = $THREAD_4_139072;",
        "let $KEY_7; $KEY_7 = $THREAD_4_139072;",
    );
    check(&late, &[]);
}

#[test]
fn later_assigned_data_receivers_do_not_claim_native_origin() {
    let assigned = FIXTURE.replace(
        "\n}main();",
        "\n$KEY_240 = makeReceiver(); $KEY_390 = makeReporter();\n}main();",
    );
    assert_ne!(assigned, FIXTURE);
    check(&assigned, &[NOTIFIER, FORWARDERS[0], FORWARDERS[1]]);
}

const DIRECT: &str = include_str!("fixtures/submit-notification/direct-notification.js");
const DIRECT_NAMES: [&str; 4] = [
    "$THREAD_8_52378",
    "$THREAD_5_46332",
    "$THREAD_0_121572",
    "$THREAD_7_25094",
];
fn direct_check(source: &str, expected: bool) {
    let r = name_threads(source, true).unwrap();
    for (i, name) in DIRECT_NAMES.iter().enumerate() {
        let o = r
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == *name)
            .unwrap();
        assert_eq!(
            o.evaluator.as_deref()
                == Some(if i == 3 {
                    "submit_trusted_notification_protocol"
                } else {
                    EVALUATOR
                }),
            expected,
            "{name}: {o:?}"
        );
    }
    let mut replay = source.to_owned();
    let mut edits = r.report.edits.clone();
    edits.sort_by_key(|e| std::cmp::Reverse(e.start));
    for e in edits {
        replay.replace_range(e.start as usize..e.end as usize, &e.replacement);
    }
    assert_eq!(replay, r.source);
}
#[test]
fn actual_direct_call_parent_and_alpha() {
    direct_check(DIRECT, true);
    direct_check(
        &DIRECT
            .replace("$CF", "otherEvent")
            .replace("$KEY_7", "originalForm")
            .replace("$KEY_8", "publishedNotification")
            .replace("$CC", "selectedReceiver")
            .replace("$KEY_92", "opaqueReporter"),
        true,
    );
}
#[test]
fn direct_parent_refusals() {
    for (from, to) in [
        ("2.27:", "2.28:"),
        ("2.28:", "2.27:"),
        ("$CB.target", "$CB.event"),
        ("$CB.event == null", "$CB.target == null"),
        ("$CD.parentNode;", "$CD.ownerDocument;"),
        ("$CE.nodeName !== \"FORM\"", "$CE.nodeName === \"FORM\""),
        ("$CC = $CD;", "$CC = $CB;"),
        ("$KEY_3.removeEventListener", "$KEY_7.removeEventListener"),
        ("event: $CF", "event: $KEY_7"),
        ("form: $KEY_7", "target: $KEY_7"),
        ("$KEY_176.notify({", "$KEY_92.notify({"),
        ("$KEY_37.notify($KEY_7)", "$KEY_176.notify($KEY_7)"),
        (
            "!$CF.isTrusted && !$CF.cancelable",
            "!$CF.isTrusted || !$CF.cancelable",
        ),
        ("$KEY_8($CF)", "$KEY_8($CB)"),
        ("$THREAD_8_52378();", "$THREAD_8_52378(); sideEffect();"),
        (
            "$KEY_8 = $THREAD_8_52378;",
            "$KEY_8 = $THREAD_8_52378; $KEY_8 = other;",
        ),
        ("let $KEY_3 = $CC;", "let $KEY_3 = $CC; $KEY_3 = other;"),
        (
            "$KEY_1 = $THREAD_7_25094;",
            "$KEY_1 = $THREAD_7_25094; return 1;",
        ),
        ("let $KEY_8;", "let $KEY_8 = source.callback;"),
        ("$KEY_8 = $THREAD_8_52378;", "$KEY_8 = source.callback;"),
        (
            "let $KEY_8;",
            "opaqueReporter.notify(\"early\"); let $KEY_8;",
        ),
    ] {
        assert!(DIRECT.contains(from), "missing mutation {from}");
        direct_check(&DIRECT.replace(from, to), false);
    }
}
