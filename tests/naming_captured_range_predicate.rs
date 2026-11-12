use shape::naming::name_threads;
const SPLIT_EPOCH: &str = include_str!("fixtures/captured_range_predicate/initialized-bounds-parent.js");
const FOUR_EPOCH: &str = include_str!("fixtures/captured_range_predicate/multi-epoch-constructor.js");
const EVALUATOR: &str = "signals_captured_range_predicate";
const LETS: &str = "let $KEY_34, $KEY_250, $KEY_262, $KEY_317, $KEY_410, $KEY_421;";
const OWNER: &str = "$THREAD_5_48943";
fn check(source: &str, accepted: bool) {
    let result = name_threads(source, true).unwrap();
    let outcomes: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|o| o.applied && o.evaluator.as_deref() == Some(EVALUATOR))
        .collect();
    assert_eq!(outcomes.len(), usize::from(accepted), "{outcomes:?}");
    if accepted {
        assert_eq!(
            outcomes[0].proposed_name.as_deref(),
            Some("capturedTwoRangeOr45Predicate")
        );
        assert!(outcomes[0]
            .explanation
            .iter()
            .any(|s| s.contains("undefined")));
    }
    let mut replay = source.to_string();
    for edit in result.report.edits.iter().rev() {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
}
fn edit(from: &str, to: &str) -> String {
    assert_eq!(SPLIT_EPOCH.matches(from).count(), 1, "unique test mutation {from}");
    SPLIT_EPOCH.replacen(from, to, 1)
}
#[test]
fn actual_whole_saved_context_and_alpha_layouts() {
    check(SPLIT_EPOCH, true);
    check(
        &SPLIT_EPOCH.replace("$BH", "datum")
            .replace("$KEY_421", "firstLower")
            .replace("$KEY_250", "firstUpper")
            .replace("$KEY_34", "secondLower")
            .replace("$KEY_317", "secondUpper")
            .replace(OWNER, "$THREAD_1_111"),
        true,
    );
    check(&edit("return $BH >= $KEY_421 && $BH <= $KEY_250 || $BH >= $KEY_34 && $BH <= $KEY_317 || $BH === 45;",
        "return (($BH >= $KEY_421 && $BH <= $KEY_250) || ($BH >= $KEY_34 && $BH <= $KEY_317)) || ($BH === 45);"), true);
    // Operational role is independent of downstream domain interpretation.
    check(
        &edit("Maximum domain length", "Maximum opaque input length"),
        true,
    );
}
#[test]
fn ordered_calls_and_early_before_initialization() {
    check(&edit(LETS, &format!("{LETS}\n{OWNER}();")), true);
    check(&edit(LETS, &format!("{OWNER}();\n{LETS}")), false);
    check(
        &edit(LETS, &format!("const alias = {OWNER};\n{LETS}")),
        false,
    );
    check(
        &edit(LETS, &format!("{LETS}\nconst alias = {OWNER}; alias();")),
        false,
    );
}
#[test]
fn stores_branches_and_reentry_are_refused() {
    for suffix in [
        "$KEY_421 = 97;",
        "if (flag) $KEY_421 = 97;",
        "function later() { $KEY_250 = 122; }",
        "Object.assign({}, {set: () => $KEY_34++});",
    ] {
        check(&edit(LETS, &format!("{LETS}\n{suffix}")), false);
    }
    check(&edit(LETS, &format!("if (flag) {{ {LETS} }}")), false);
    check(
        &edit(
            "function $THREAD_5_48943($V, $W, $X, $Y) {",
            "function $THREAD_5_48943($V, $W, $X, $Y) { $THREAD_5_48943();",
        ),
        false,
    );
    check(
        &edit(
            LETS,
            "let $KEY_34, $KEY_250, $KEY_262, $KEY_317, $KEY_410, $KEY_421 = 97;",
        ),
        false,
    );
}
#[test]
fn foreign_bounds_operators_formals_literals_and_extra_effects() {
    for (from, to) in [
        ("$BH >= $KEY_421", "$BH >= globalThis.bound"),
        ("$BH >= $KEY_421", "$BH >= foreignBound"),
        ("$BH >= $KEY_421", "$KEY_421 <= $BH"),
        ("$BH >= $KEY_421", "$BH > $KEY_421"),
        ("$BH <= $KEY_250", "$V <= $KEY_250"),
        ("$BH === 45", "$BH == 45"),
        ("$BH === 45", "$BH === 46"),
        ("$BH === 45", "$BH === 45 || sideEffect()"),
        ("return $BH >=", "sideEffect(); return $BH >="),
        (
            "function $THREAD_5_41252($BH)",
            "function $THREAD_5_41252($BH = 0)",
        ),
    ] {
        check(&edit(from, to), false);
    }
}
#[test]
fn dynamic_scope_shadowing_and_exposure_refused() {
    check(&edit(LETS, &format!("{LETS}\neval('');")), false);
    check(
        &edit(
            "return $BH >= $KEY_421",
            "let $KEY_421 = 97; return $BH >= $KEY_421",
        ),
        false,
    );
    check(
        &edit(
            "$AO.every($THREAD_5_41252)",
            "$AO.every($THREAD_5_41252, extra)",
        ),
        false,
    );
    check(
        &edit("$AO.every($THREAD_5_41252)", "expose($THREAD_5_41252)"),
        false,
    );
    check(&edit(LETS, &format!("{LETS}\n{OWNER}.call(null);")), false);
}
#[test]
fn actual_multi_epoch_candidate_is_not_equivalent() {
    check(FOUR_EPOCH, false);
}
