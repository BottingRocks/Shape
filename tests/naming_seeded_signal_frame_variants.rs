//! Full cached sources retain bootstrap, carriers, callable aliases, and actual
//! observer consumers. No abbreviated body can hide a full-source mismatch.
//! Checks are owned by the integrator; these fixtures are only two witnesses.
use shape::naming::name_threads;

const REUSED: &str = include_str!("fixtures/program/seeded-signal-frame/separate-nonce.js");
const SEPARATE: &str = include_str!("fixtures/program/seeded-signal-frame/reused-nonce.js");
const TARGET: &str = "$THREAD_4_44304";
const NAME: &str = "emitSeededSignalFrame";
const EVALUATOR: &str = "signals_seeded_signal_frame";

fn check(source: &str, target: &str, expected: bool) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::unambiguous()).parse();
    assert!(
        !parsed.panicked && parsed.errors.is_empty(),
        "{:?}",
        parsed.errors
    );
    // name_threads parses, scopes, and builds Model before evaluating any rule.
    // A refused input must pass all those steps and retain the intended target.
    let result = name_threads(source, true).expect("source must build a valid Model");
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == target)
        .expect("fixture must retain the callback under test");
    let claimed = outcome.role_name.as_deref() == Some(NAME)
        && outcome.evaluator.as_deref() == Some(EVALUATOR)
        && outcome.applied;
    assert_eq!(claimed, expected, "target {target}: {outcome:?}");
    if expected {
        assert!(outcome
            .proposed_name
            .as_deref()
            .is_some_and(|name| name.starts_with(NAME)));
        assert!(!outcome.evidence.is_empty());
    }
}

fn changed_global(from: &str, to: &str) -> String {
    assert_eq!(
        REUSED.matches(from).count(),
        1,
        "mutation must have one source site: {from}"
    );
    let result = REUSED.replacen(from, to, 1);
    assert_ne!(result, REUSED);
    result
}

fn changed(from: &str, to: &str) -> String {
    let start = REUSED.find("function $THREAD_4_44304(").unwrap();
    let end = start + REUSED[start..].find("\n\t\t\t\t\tlet $CB =").unwrap();
    let body = &REUSED[start..end];
    assert_eq!(
        body.matches(from).count(),
        1,
        "mutation must have one callback site: {from}"
    );
    let mut result = REUSED.to_owned();
    result.replace_range(start..end, &body.replacen(from, to, 1));
    result
}

#[test]
fn real_nonce_reuse() {
    check(REUSED, TARGET, true);
}

#[test]
fn separate_nonce_counterpart_preserves_old_route() {
    check(SEPARATE, "$THREAD_0_38208", true);
}

#[test]
fn callback_identity_is_not_a_source_name_or_pc() {
    check(
        &REUSED.replace(TARGET, "$THREAD_71_998877"),
        "$THREAD_71_998877",
        true,
    );
}

#[test]
fn nonce_header_reseed_loop_and_observer_mutants_are_valid_but_refused() {
    // Keep each previous negative independently: no mutation replaces another.
    let mutations = [
        ("let $CD = [];", "let $CD = [1];"),
        (
            "$KEY_5 >>> 24, $KEY_5 >> 16 & 255, $KEY_5 >> 8 & 255, $KEY_5 & 255",
            "$KEY_5 >>> 24, $KEY_5 >> 8 & 255, $KEY_5 >> 16 & 255, $KEY_5 & 255",
        ),
        (
            "$KEY_5 = $KEY_141($KEY_5 ^ 2582463591);",
            "$KEY_5 = $KEY_141(2582463591);",
        ),
        (
            "$KEY_5 = $KEY_141($KEY_5 ^ 2582463591);",
            "$KEY_5 = $KEY_141($KEY_5 ^ 2582463591); $KEY_5 = 7;",
        ),
        (
            "$KEY_5 = $KEY_141($KEY_5 ^ 2582463591);",
            "$KEY_5 = $KEY_141($KEY_5 ^ 2582463591); let lateNumeric = $KEY_5 >>> 24;",
        ),
        (
            "$KEY_5 = $KEY_141($KEY_5 ^ 2582463591);",
            "$KEY_5 = $KEY_141($KEY_5 ^ 2582463591); function hidden(){return $KEY_5;}",
        ),
        ("let $CF = $CC.length;", "let $CF = 3;"),
        (
            "$KEY_250($KEY_5, $CF, $CE, 3)",
            "$KEY_250($KEY_5, $CF, $CE, 4)",
        ),
        ("while ($CF - 1 >= 0)", "while ($CF - 1 > 0)"),
        (
            "$KEY_250($KEY_5, $CC[$CF - 1], $CE, $CG)",
            "$KEY_250($KEY_5, $CC[$CF], $CE, $CG)",
        ),
        (
            "$KEY_250($KEY_5, $CC[$CF - 1], $CE, $CG)",
            "$KEY_141($KEY_5, $CC[$CF - 1], $CE, $CG)",
        ),
        ("--$CF;", "--$CG;"),
        ("$KEY_10.next([$CD, $CE]);", "$KEY_10.next([$CE, $CD]);"),
        (
            "$KEY_10.next([$CD, $CE]);",
            "$KEY_10.next([$CD, $CE]); return;",
        ),
    ];
    for (from, to) in mutations {
        check(&changed(from, to), TARGET, false);
    }
}

#[test]
fn captured_factories_must_have_their_own_proven_behavior() {
    // State formal ignored by the real factory, not just a fake arity stub.
    check(
        &changed_global(
            "$KEY_5 = $KEY_317;\n\t\t\t$KEY_4 = $BT;\n\t\t\t$KEY_1 = -1;",
            "$KEY_5 = $KEY_317;\n\t\t\t$KEY_4 = 7;\n\t\t\t$KEY_1 = -1;",
        ),
        TARGET,
        false,
    );
    // Replacing the factory's single returned helper with a donated function
    // retains the factory body and warm-up effects but breaks lexical ownership.
    check(
        &changed_global("return $THREAD_2_94277;", "return $THREAD_2_54438;"),
        TARGET,
        false,
    );
    // Both writer calls still have identical identity and arity. That alone
    // cannot make the foreign callable a number writer.
    check(
        &changed_global(
            "$KEY_250 = $THREAD_2_54438;",
            "$KEY_250 = $THREAD_6_123968;",
        ),
        TARGET,
        false,
    );
}

#[test]
fn factory_requires_four_own_warmups() {
    let four = "$THREAD_2_94277();\n\t\t\t$THREAD_2_94277();\n\t\t\t$THREAD_2_94277();\n\t\t\t$THREAD_2_94277();";
    let three = "$THREAD_2_94277();\n\t\t\t$THREAD_2_94277();\n\t\t\t$THREAD_2_94277();";
    check(&changed_global(four, three), TARGET, false);
    let five = format!("{four}\n\t\t\t$THREAD_2_94277();");
    check(&changed_global(four, &five), TARGET, false);
    // Four total calls in the text are insufficient: only three are factory
    // owned; the fourth belongs to an uncalled nested sibling of the helper.
    let nested = format!("{three}\n\t\t\tfunction uncalledWarmup(){{ $THREAD_2_94277(); }}");
    check(&changed_global(four, &nested), TARGET, false);
}
