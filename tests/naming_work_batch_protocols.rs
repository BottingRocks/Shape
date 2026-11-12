use shape::naming::name_threads;

const WHOLE: &str = include_str!("fixtures/naming/work_batch_protocols/work-batch-whole-scope.js");

#[test]
fn referenced_noops_in_whole_original_scope_have_exact_evidence() {
    let result = name_threads(WHOLE, true).unwrap();
    let noops: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|o| o.applied && o.evaluator.as_deref() == Some("referenced_ordinary_noop"))
        .collect();
    // Original INPUT has 39 empty identities, including three producer-suffixed
    // bindings that the first navigation grouping counted separately.
    assert_eq!(noops.len(), 39);
    for o in noops {
        assert_eq!(o.role_name.as_deref(), Some("noOp"));
        assert_eq!(o.evidence, vec![o.binding.declaration]);
        assert!(o.reference_edits > 0);
        assert_eq!(o.shorthand_edits, 0);
    }
    assert_eq!(
        result.report.removals.iter().filter(|r| r.applied).count(),
        2
    );
    role(
        &WHOLE.replace("$THREAD_6_11282", "$THREAD_9_123456"),
        "$THREAD_9_123456",
        "referenced_ordinary_noop",
        true,
    );
}

#[test]
fn noop_refuses_parameter_effects_directives_and_nonempty_bodies() {
    let original = "function $THREAD_6_11282()";
    for replacement in [
        "async function $THREAD_6_11282()",
        "function* $THREAD_6_11282()",
        "function $THREAD_6_11282(x=reportEffect())",
        "function $THREAD_6_11282(...args)",
        "function $THREAD_6_11282({x})",
        "function $THREAD_6_11282(x)",
    ] {
        assert!(WHOLE.contains(original));
        role(
            &WHOLE.replace(original, replacement),
            "$THREAD_6_11282",
            "referenced_ordinary_noop",
            false,
        );
    }
    for replacement in [
        "function $THREAD_6_11282() { reportEffect();",
        "function $THREAD_6_11282() { return;",
        "function $THREAD_6_11282() { 'use strict';",
    ] {
        role(
            &WHOLE.replace("function $THREAD_6_11282() {", replacement),
            "$THREAD_6_11282",
            "referenced_ordinary_noop",
            false,
        );
    }
}

#[test]
fn noop_preserves_unused_policy_and_refuses_reassigned_target_or_donor() {
    let source="function main(){function $THREAD_9_10(){} function $THREAD_9_11(){} return $THREAD_9_11;}main();";
    let r = name_threads(source, true).unwrap();
    assert!(r
        .report
        .removals
        .iter()
        .any(|x| x.name == "$THREAD_9_10" && x.applied));
    role(source, "$THREAD_9_11", "referenced_ordinary_noop", true);
    let reassigned="function main(){function $THREAD_9_11(){} $THREAD_9_11=callback;return $THREAD_9_11;}main();";
    role(
        reassigned,
        "$THREAD_9_11",
        "referenced_ordinary_noop",
        false,
    );
    let donor="function main(){function $THREAD_9_10(){} function $THREAD_9_11(){effect();}return [$THREAD_9_10,$THREAD_9_11];}main();";
    role(donor, "$THREAD_9_11", "referenced_ordinary_noop", false);
    let collision="function main(){function noOp(){} function $THREAD_9_11(){} return [noOp,$THREAD_9_11];}main();";
    let r = name_threads(collision, true).unwrap();
    assert!(r.source.contains("function noOp(){}"));
    assert!(r.source.contains("[noOp,"));
    // A solo generated label can refuse a collision; old readable names stay.
    assert!(!r
        .report
        .edits
        .iter()
        .any(|e| &collision[e.start as usize..e.end as usize] == "noOp"));
}

fn role(source: &str, target: &str, evaluator: &str, expected: bool) {
    let result = name_threads(source, true).expect("valid whole-scope source");
    let outcomes: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|o| o.binding.name == target)
        .collect();
    assert!(!outcomes.is_empty(), "actual target must remain present");
    for o in outcomes {
        assert_eq!(
            o.applied && o.evaluator.as_deref() == Some(evaluator),
            expected,
            "{o:?}"
        );
    }
}

#[test]
fn actual_whole_scope_and_alpha_variants() {
    role(WHOLE, "$THREAD_2_64751", "indexed_null_prototype_map", true);
    let alpha = WHOLE
        .replace("$HM", "mapKeysAlpha")
        .replace("$HN", "mapValuesAlpha")
        .replace("$HO", "mapEntryAlpha")
        .replace("$HP", "mapIndexAlpha");
    role(
        &alpha,
        "$THREAD_2_64751",
        "indexed_null_prototype_map",
        true,
    );
}

#[test]
fn indexed_map_refuses_changed_own_protocols_in_whole_scope() {
    for (from, to) in [
        ("$KEY_0[$HO] = $KEY_1[$HP];", "$KEY_0[$HP] = $KEY_1[$HO];"),
        (
            "$KEY_0[$HO] = $KEY_1[$HP];",
            "$KEY_0[$HO] = $KEY_1[$HP]; reportExtraEffect();",
        ),
        (
            "$KEY_0[$HO] = $KEY_1[$HP];",
            "$KEY_0[$HO] = $KEY_1[$HP + 1];",
        ),
        ("$HM.forEach(($HO, $HP)", "$HM.map(($HO, $HP)"),
        ("$KEY_0[$HO] = $KEY_1[$HP];", "$KEY_0[$HO] += $KEY_1[$HP];"),
        (
            "$KEY_0[$HO] = $KEY_1[$HP];",
            "$KEY_1 = []; $KEY_0[$HO] = $KEY_1[$HP];",
        ),
    ] {
        assert!(WHOLE.contains(from));
        role(
            &WHOLE.replace(from, to),
            "$THREAD_2_64751",
            "indexed_null_prototype_map",
            false,
        );
    }
}

#[test]
fn shadowed_global_and_wrong_callback_capture_refuse() {
    let source = "function main(Object){function $THREAD_2_64751(names,values){let alias=values;let out=Object.create(null);names.forEach((key,index)=>{out[key]=alias[index];});return out;}return $THREAD_2_64751;}main();";
    role(
        source,
        "$THREAD_2_64751",
        "indexed_null_prototype_map",
        false,
    );
    let wrong_capture = "function main(values){function $THREAD_2_64751(names,other){let alias=values;let out=Object.create(null);names.forEach((key,index)=>{out[key]=alias[index];});return out;}return $THREAD_2_64751;}main();";
    role(
        wrong_capture,
        "$THREAD_2_64751",
        "indexed_null_prototype_map",
        false,
    );
}

#[test]
fn indexed_map_preserves_original_initializer_metadata_and_separates_split_proof() {
    let original_proof = "entire capture-free body: immutable values alias, global Object.create(null), formal names.forEach with exact output[name]=values[index] callback, then return output; external formal method implementation is unproved";
    let original = name_threads(WHOLE, true).unwrap();
    let maps: Vec<_> = original.report.outcomes.iter().filter(|o| o.applied && o.evaluator.as_deref() == Some("indexed_null_prototype_map")).collect();
    assert_eq!(maps.len(), 3);
    for outcome in maps { assert_eq!(outcome.explanation, vec![original_proof]); }
    let split = WHOLE.replace("let $KEY_0 = Object.create(null);", "let $KEY_0; $KEY_0 = Object.create(null);");
    let result = name_threads(&split, true).unwrap();
    let outcome = result.report.outcomes.iter().find(|o| o.binding.name == "$THREAD_2_64751").unwrap();
    assert!(outcome.applied);
    assert!(outcome.explanation.iter().any(|p| p.contains("uninitialized output declaration immediately followed by sole direct")));
}

#[test]
fn split_map_store_is_immediate_and_unique() {
    let split = WHOLE.replace(
        "let $KEY_0 = Object.create(null);",
        "let $KEY_0; $KEY_0 = Object.create(null);",
    );
    role(
        &split,
        "$THREAD_2_64751",
        "indexed_null_prototype_map",
        true,
    );
    for replacement in [
        "let $KEY_0; if (ready) $KEY_0 = Object.create(null);",
        "let $KEY_0; invokeReentrantCallback(); $KEY_0 = Object.create(null);",
        "let $KEY_0; $KEY_0 = Object.create(null); later(()=>{$KEY_0={};});",
    ] {
        role(
            &WHOLE.replace("let $KEY_0 = Object.create(null);", replacement),
            "$THREAD_2_64751",
            "indexed_null_prototype_map",
            false,
        );
    }
}

#[test]
fn batch_methods_in_actual_scope() {
    role(
        WHOLE,
        "$THREAD_5_31180",
        "batch_scheduler_owned_schedule",
        true,
    );
    role(
        WHOLE,
        "$THREAD_0_70936",
        "batch_scheduler_owned_process",
        true,
    );
    role(WHOLE, "$THREAD_6_69054", "batch_scheduler_owned_now", true);
    let alpha = WHOLE
        .replace("$GP", "alphaInput")
        .replace("$GQ", "alphaBudget")
        .replace("$GR", "alphaStarted")
        .replace("$GS", "alphaAverage");
    role(
        &alpha,
        "$THREAD_0_70936",
        "batch_scheduler_owned_process",
        true,
    );
}

#[test]
fn batch_loop_refuses_numeric_order_branch_and_extra_effect_changes() {
    for (from, to) in [
        (
            "($GS * i + ($GW - $GV)) / (i + 1)",
            "($GS * i + ($GW - $GV)) / (i + 2)",
        ),
        ("$GT = $GP();", "reportExtraEffect(); $GT = $GP();"),
        ("$GT = $GP();", "$GT = $GP.call(this);"),
        ("$GU = Number(i) + 1;", "$GU = Number(i) + 2;"),
        ("if ($GT) {", "if (!$GT) {"),
        ("for (;; i = $GU)", "for (;; i = $GT)"),
    ] {
        assert!(
            WHOLE.contains(from),
            "missing actual mutation anchor {from}"
        );
        role(
            &WHOLE.replace(from, to),
            "$THREAD_0_70936",
            "batch_scheduler_owned_process",
            false,
        );
    }
}

#[test]
fn timer_receiver_delay_and_internal_call_effects_are_owned() {
    role(
        &WHOLE.replace(
            "setTimeout(this._process.bind(this, $GP), 0)",
            "setTimeout(this._process.bind(null, $GP), 0)",
        ),
        "$THREAD_5_31180",
        "batch_scheduler_owned_schedule",
        false,
    );
    role(
        &WHOLE.replace(
            "setTimeout(this._process.bind(this, $GP), 0)",
            "setTimeout(this._process.bind(this, $GP), 1)",
        ),
        "$THREAD_5_31180",
        "batch_scheduler_owned_schedule",
        false,
    );
    role(
        &WHOLE.replace(
            "return this._internalNow();",
            "reportExtraEffect(); return this._internalNow();",
        ),
        "$THREAD_6_69054",
        "batch_scheduler_owned_now",
        false,
    );
}
