use shape::naming::name_threads;
const BODY: &str = include_str!("fixtures/persistent-token-init-inline.js");
const EVALUATOR: &str = "persistent_token_init_variants";

// Local protocol carrier for the complete initializer, including post-store clobbers.
// Browser-origin evidence is tested separately.
fn source() -> String {
    format!(
        r#"function bootstrap(token, store, moduleList, consumer, continuation, stream, report) {{
let $KEY_15={{persistentId:{{key:"key"}},transactionToken:token}};
let $KEY_137=false, $KEY_67=store, $KEY_365=null, $KEY_142=0;
let $KEY_61=null, $KEY_378=null, $KEY_53=consumer, $KEY_115=continuation;
let $KEY_10=moduleList, $KEY_307=stream, $KEY_424=false, $KEY_436=[], $KEY_311=report;
{BODY}
return {{init:$THREAD_8_61920}};
}} bootstrap();"#
    )
}
fn check(source: &str, expected: bool) {
    let named = name_threads(source, true).expect("fixture parses");
    let outcome = named
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_8_61920")
        .unwrap();
    assert_eq!(
        outcome.evaluator.as_deref() == Some(EVALUATOR),
        expected,
        "{outcome:?}"
    );
    if expected {
        assert!(outcome.applied);
    }
}
#[test]
fn complete_owned_inline_protocol_and_alpha() {
    check(&source(), true);
    let alpha = source()
        .replace("$KEY_15", "context")
        .replace("$CK", "scratch")
        .replace("$KEY_10", "plugins");
    check(&alpha, true);
}
#[test]
fn declaration_can_precede_initialization_without_exposure() {
    let s = source();
    let start = s.find("let $KEY_15").unwrap();
    let end = s.find(BODY).unwrap();
    let setup = &s[start..end];
    let moved = s
        .replacen(setup, "", 1)
        .replace("return {init:", &format!("{setup}return {{init:"));
    check(&moved, true);
}
#[test]
fn ordered_seed_and_effect_refusals() {
    for (from, to) in [
        (
            "$KEY_61 = \"\" + $CL + $CK;",
            "$CK = 2; $KEY_61 = \"\" + $CL + $CK;",
        ),
        ("$KEY_61 = \"\" + $CL + $CK;", "$KEY_61 = \"\" + $CL + $CJ;"),
        (
            "$CK = ($CN ^ $CO ^ $KEY_142) >>> 0;",
            "if (coin) $CK = ($CN ^ $CO ^ $KEY_142) >>> 0;",
        ),
        ("++$KEY_142", "--$KEY_142"),
        ("1103515245", "1103515246"),
        ("\"l_\"", "\"lease_\""),
        ("$KEY_53($KEY_15.transactionToken)", "$KEY_53($KEY_61)"),
        ("$KEY_115();", "$KEY_115(); $KEY_115();"),
        ("$CQ.init($KEY_307());", "$CQ.init($KEY_307()); effect();"),
        ("$CQ.afterReady.call(void 0)", "$CQ.afterReady.call($CQ)"),
        ("3.1:", "3.2:"),
        ("$KEY_67.setItem($CH, $CI)", "$KEY_67.setItem($CH, $CH)"),
    ] {
        assert!(source().contains(from), "missing mutation {from}");
        check(&source().replacen(from, to, 1), false);
    }
}
#[test]
fn capture_shadow_and_wrong_storage_refusals() {
    check(
        &source().replace("let $CH =", "let Math = fake; let $CH ="),
        false,
    );
    check(
        &source().replace("$KEY_67.setItem($CH, $CI)", "other.setItem($CH, $CI)"),
        false,
    );
    check(
        &source().replace("let $CH =", "let $KEY_15 = other; let $CH ="),
        false,
    );
}
#[test]
fn early_call_alias_and_synchronous_callback_exposure_refusals() {
    for early in [
        "$THREAD_8_61920();",
        "let alias=$THREAD_8_61920;",
        "dispatch(()=> $THREAD_8_61920());",
        "eval('hidden');",
    ] {
        check(
            &source().replace("let $KEY_15", &format!("{early}let $KEY_15")),
            false,
        );
    }
}
#[test]
fn conditional_setup_and_later_capture_write_refusals() {
    check(
        &source().replace(
            "let $KEY_10=moduleList",
            "let $KEY_10; if (coin) $KEY_10=moduleList",
        ),
        false,
    );
    check(
        &source().replace("return {init:", "$KEY_15=other; return {init:"),
        false,
    );
}
#[test]
fn original_complete_owning_runtime() {
    check(
        include_str!("fixtures/persistent-token-init-storage-guards.js"),
        true,
    );
}
#[test]
fn original_runtime_early_activation_and_branch_only_setup() {
    let original = include_str!("fixtures/persistent-token-init-storage-guards.js");
    check(
        &original.replace(
            "$KEY_340 = $THREAD_3_108133;",
            "$KEY_340 = $THREAD_3_108133; $THREAD_3_108133();",
        ),
        false,
    );
    check(
        &original.replace("$KEY_15 = $CC;", "if (unknown) $KEY_15 = $CC;"),
        false,
    );
    check(
        &original.replace(
            "$KEY_340 = $THREAD_3_108133;",
            "eval(hidden); $KEY_340 = $THREAD_3_108133;",
        ),
        false,
    );
    check(
        &original.replace(
            "$KEY_10 = Array.prototype.map.call",
            "if (unknown) $KEY_10 = Array.prototype.map.call",
        ),
        false,
    );
    check(
        &original.replace(
            "$KEY_15 = $CC;",
            "$KEY_15 = $CC; dispatch(() => $THREAD_8_61920());",
        ),
        false,
    );
}
#[test]
fn two_other_original_ordered_inline_transports() {
    for (source, name) in [
        (
            include_str!("fixtures/persistent-token-init-storage-guards-alternate.js"),
            "$THREAD_4_139360",
        ),
        (
            include_str!("fixtures/persistent-token-init-storage-guards-heldout.js"),
            "$THREAD_4_149804",
        ),
    ] {
        let named = name_threads(source, true).unwrap();
        let o = named
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == name)
            .unwrap();
        assert_eq!(o.evaluator.as_deref(), Some(EVALUATOR), "{o:?}");
        assert!(o.applied);
    }
}
#[test]
fn canonical_migration_control_keeps_its_earlier_claim() {
    let result = name_threads(
        include_str!("fixtures/program/thread-persistent-id-migration.js"),
        true,
    )
    .unwrap();
    let o = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_6_57781")
        .unwrap();
    assert!(o.applied);
    assert_eq!(
        o.role_name.as_deref(),
        Some("migrateAndInitializePersistentId")
    );
    assert_ne!(o.evaluator.as_deref(), Some(EVALUATOR));
}

fn original_mutation(from: &str, to: &str) -> String {
    let original = include_str!("fixtures/persistent-token-init-storage-guards.js");
    assert_eq!(original.matches(from).count(), 1, "unique mutation anchor");
    let mutated = original.replacen(from, to, 1);
    // The factory still has its original single direct value reference. These
    // regressions must exercise alias/writer/path closure, not cardinality.
    assert_eq!(mutated.matches("= $THREAD_3_108133;").count(), 1);
    mutated
}

#[test]
fn original_nested_return_helper_cannot_establish_readiness() {
    let api = "return {\n\t\t\t\t\t\t\tinit: $THREAD_8_61920,\n\t\t\t\t\t\t\tteardown: $THREAD_4_163497\n\t\t\t\t\t\t};";
    let s = original_mutation(api,
        "return reviewEarlyApi(); function reviewEarlyApi() { return { init: $THREAD_8_61920, teardown: $THREAD_4_163497 }; }");
    let marker = "} else {\n\t\t\t\t\t\tlet $CC = {\n\t\t\t\t\t\t\tglobal: $KEY_91,";
    assert_eq!(s.matches(marker).count(), 1);
    let s = s.replacen(marker,
        "} else {\nif (reviewEarly) return reviewEarlyApi();\n\t\t\t\t\t\tlet $CC = {\n\t\t\t\t\t\t\tglobal: $KEY_91,", 1);
    assert_eq!(s.matches("init: $THREAD_8_61920").count(), 1);
    check(&s, false);
}

#[test]
fn original_ancestor_consumer_writers_close_all_epochs() {
    for extra in [
        "$KEY_53 = void 0;",
        "reviewClobber(); function reviewClobber() { $KEY_53 = void 0; }",
    ] {
        check(
            &original_mutation(
                "$KEY_340 = $THREAD_3_108133;",
                &format!("$KEY_340 = $THREAD_3_108133; {extra}"),
            ),
            false,
        );
    }
}

#[test]
fn original_factory_alias_early_and_reentrant_calls_are_refused() {
    check(
        &original_mutation(
            "$KEY_340 = $THREAD_3_108133;",
            "$KEY_340 = $THREAD_3_108133; new $KEY_340();",
        ),
        false,
    );
    check(
        &original_mutation(
            "$KEY_15 = $CC;",
            "reviewReenter(); function reviewReenter() { new $KEY_340(); } $KEY_15 = $CC;",
        ),
        false,
    );
    check(
        &original_mutation(
            "$KEY_340 = $THREAD_3_108133;",
            "$KEY_340 = $THREAD_3_108133; let reviewAlias = $KEY_340; reviewAlias();",
        ),
        false,
    );
}

#[test]
fn original_counter_and_queue_replacements_are_refused() {
    for extra in ["$KEY_142 = void 0;", "$KEY_436 = {};", "$KEY_424 = void 0;"] {
        check(
            &original_mutation(
                "$KEY_340 = $THREAD_3_108133;",
                &format!("$KEY_340 = $THREAD_3_108133; {extra}"),
            ),
            false,
        );
    }
}

#[test]
fn original_outer_api_helper_escape_is_refused() {
    check(
        &original_mutation(
            "return { init: $THREAD_8_159340 };",
            "return reviewOuter(); function reviewOuter() { return { init: $THREAD_8_159340 }; }",
        ),
        false,
    );
}

#[test]
fn original_deferred_writer_in_another_initializing_declarator_is_refused() {
    let s = original_mutation(
        "let $KEY_53 = $THREAD_2_41874;",
        "let $KEY_53 = $THREAD_2_41874, reviewDeferredClobber = function () { $KEY_53 = void 0; };",
    )
    .replacen(
        "$KEY_340 = $THREAD_3_108133;",
        "$KEY_340 = $THREAD_3_108133; reviewDeferredClobber();",
        1,
    );
    assert!(
        s.contains(BODY.trim()),
        "original owned initializer body retained"
    );
    check(&s, false);
}

#[test]
fn original_deferred_writer_inside_consumer_initializer_is_refused() {
    let original = include_str!("fixtures/persistent-token-init-storage-guards.js");
    let start = original.find("function $THREAD_2_41874($BQ) {").unwrap();
    let end = original[start..]
        .find("let $KEY_53 = $THREAD_2_41874;")
        .unwrap()
        + start;
    let original_consumer = original[start..end].trim_end();
    let consumer = original_consumer.replacen(
        "function $THREAD_2_41874($BQ) {",
        "let $KEY_53 = function $THREAD_2_41874($BQ) { function reviewInsideClobber() { $KEY_53 = void 0; } if ($BQ === null) return reviewInsideClobber;", 1,
    );
    let s = format!(
        "{}{};\n{}",
        &original[..start],
        consumer,
        &original[end + "let $KEY_53 = $THREAD_2_41874;".len()..]
    )
    .replacen(
        "$KEY_340 = $THREAD_3_108133;",
        "$KEY_340 = $THREAD_3_108133; $KEY_53(null)();",
        1,
    );
    assert_eq!(s.matches("= $THREAD_3_108133;").count(), 1);
    assert!(
        s.contains(BODY.trim()),
        "original owned initializer body retained"
    );
    check(&s, false);
}

#[test]
fn initializing_statement_does_not_exempt_synchronous_replacement() {
    check(
        &source().replace(
            "$KEY_53=consumer,",
            "$KEY_53=consumer, reviewClobber=($KEY_53=void 0),",
        ),
        false,
    );
}
