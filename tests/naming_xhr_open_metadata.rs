use shape::naming::name_threads;
const EVAL: &str = "shared_lowercase_method_url_metadata";
const BODY: &str = include_str!("fixtures/naming/xhr_open_metadata/owned-body.js");
fn fixture() -> String {
    format!("function $THREAD_0_0() {{ let $KEY_346 = ['opaque']; function report(x) {{ sink(x); }} const $KEY_102 = report; function install(channel) {{ let $KEY_0 = ''; let $KEY_2 = ''; {BODY} channel.register($THREAD_3_133961); }} install(channel); }}")
}
fn count(s: &str) -> usize {
    name_threads(s, true)
        .expect("valid complete scope")
        .report
        .outcomes
        .iter()
        .filter(|o| o.applied && o.evaluator.as_deref() == Some(EVAL))
        .count()
}
#[test]
fn local_method_message_only_variant_and_writable_captures() {
    let local = BODY
        .replace("$KEY_0 = \"\" +", "let $KEY_0 = \"\" +")
        .replace("let $DZ =", "$KEY_0 =")
        .replace("$DY.value = $DZ", "$DY.value = $KEY_0")
        .replace("message: \"2.31\",", "")
        .replace("stack: void 0", "")
        .replace("$EA.stack = e + \"\"", "$EA.message = \"2.31:\" + e");
    let s = fixture().replace(BODY, &local);
    assert_eq!(count(&s), 1);
    assert_eq!(
        count(&s.replace("let $KEY_0 = \"\" +", "const $KEY_0 = \"\" +")),
        0
    );
    assert_eq!(
        count(&fixture().replace("let $KEY_2 = '';", "const $KEY_2 = '';")),
        0
    );
    assert_eq!(count(&fixture().replace("['opaque']", "null")), 0);
    assert_eq!(
        count(&fixture().replace(
            "const $KEY_102 = report;",
            "let $KEY_102 = function privateReporter(x) { sink(x); };"
        )),
        0
    );
}
#[test]
fn complete_body_alpha_and_same_binding_negatives() {
    let s = fixture();
    assert_eq!(count(&s), 1, "complete original body with sealed captures");
    assert_eq!(
        count(
            &s.replace("$KEY_346", "markerKeys")
                .replace("$KEY_102", "diagnosticReporter")
                .replace("$DW", "invocation")
                .replace("$THREAD_3_133961", "$THREAD_3_999991")
        ),
        1
    );
    for (from,to) in [
        ("$DX.bypassResult != null","$DX.bypassResult == null"),
        ("$DW.args.length < 2","$DW.args.length < 1"),
        (".toLowerCase()",".toUpperCase()"),
        ("url: $KEY_2","url: $KEY_0"),
        ("$KEY_2 = $DW.args[1]","$KEY_2 = $DW.args[0]"),
        ("enumerable: false","enumerable: true"),
        ("$DW.thisObj","$DW.args"),
        ("$KEY_346[0]","$KEY_346[1]"),
        ("$EA.stack = e + \"\"","$EA.stack = \"\" + e"),
        ("$KEY_102($EA)","$KEY_102(e)"),
        ("return $DX;","effect(); return $DX;"),
        ("channel.register($THREAD_3_133961);","channel.register($THREAD_3_133961); $KEY_2 = 3;"),
        ("let $KEY_346 = ['opaque'];","let $KEY_346 = ['opaque']; leak($KEY_346);"),
        ("let $KEY_346 = ['opaque'];","let $KEY_346 = ['opaque']; $KEY_346[0] = 'altered';"),
        ("function $THREAD_0_0() {","function $THREAD_0_0() { install(channel);"),
        ("const $KEY_102 = report;","const $KEY_102 = 0;"),
        ("const $KEY_102 = report;","let $KEY_102; if (ready) { $KEY_102 = report; }"),
        ("let $KEY_346 = ['opaque'];","let $KEY_346; if (ready) { $KEY_346 = ['opaque']; }"),
        ("function install(channel) {","function install(channel) { let Object = replacement;"),
        ("function install(channel) { let $KEY_0 = ''; let $KEY_2 = '';", "function install(channel) { channel.register($THREAD_3_133961); let $KEY_0 = ''; let $KEY_2 = '';"),
        ("function $THREAD_3_133961(","const hidden = function $THREAD_3_133961("),
    ] {
        assert!(s.contains(from),"missing control {from}");
        // The private-expression control needs a complete statement, preserving
        // the private binding while exposing the actual external binding.
        let mutated=if from=="function $THREAD_3_133961(" {
            s.replace(BODY,&format!("const hidden = {};",BODY.replacen("function $THREAD_3_133961(","function $THREAD_3_133961(",1)))
             .replace("channel.register($THREAD_3_133961)","channel.register(hidden)")
        } else {s.replace(from,to)};
        assert_eq!(count(&mutated),0,"accepted same-target control {from} => {to}");
    }
}
#[test]
fn selected_witnesses_in_order() {
    for (key, source) in [
        (
            "single-guarded-registry",
            include_str!("fixtures/naming/xhr_open_metadata/stateful-parent.js"),
        ),
        (
            "sample-expansion-parent",
            include_str!("fixtures/naming/xhr_open_metadata/canonical-parent.js"),
        ),
    ] {
        let result = name_threads(source, true).expect("selected source parses");
        let found: Vec<_> = result
            .report
            .outcomes
            .iter()
            .filter(|o| o.applied && o.evaluator.as_deref() == Some(EVAL))
            .map(|o| o.function_id.clone())
            .collect();
        eprintln!(
            "ACTUAL {key}: {found:?}; generated={} applied={} classes_remaining={}",
            result.report.generated_thread_count,
            result.report.applied_count,
            result.report.unresolved_class_count
        );
        if key.starts_with("console-stack-access-flags-match-media-layouts") {
            assert!(
                !found.is_empty(),
                "actual first witness must close before credit"
            );
        }
    }
}
