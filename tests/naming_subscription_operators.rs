use oxc_ast::AstKind;
use shape::naming::name_threads;
const SEPARATE_CLEANUP: &str = include_str!("fixtures/naming/subscription_operators/subscription-parent-alternate.js");
const CAPTURED_CLEANUP: &str = include_str!("fixtures/naming/subscription_operators/subscription-parent-baseline.js");
const SCAN: &str = "shared_observable_scan_operator";
const LAST: &str = "shared_observable_last_operator";
const CALLBACK: &str = "shared_callback_subscription_adapter";
fn expected(source: &str) {
    let result = name_threads(source, true).unwrap();
    for (evaluator, names) in [
        (SCAN, ["createScanOperator", "scanOperatorFunction"]),
        (LAST, ["createLastOperator", "lastOperatorFunction"]),
        (
            CALLBACK,
            [
                "createCallbackObservableAdapter",
                "callbackObservableInvoker",
            ],
        ),
    ] {
        let got: Vec<_> = result
            .report
            .outcomes
            .iter()
            .filter(|o| o.applied && o.evaluator.as_deref() == Some(evaluator))
            .map(|o| o.proposed_name.as_deref().unwrap())
            .collect();
        assert_eq!(got, names, "{evaluator}");
    }
    let mut replay = source.to_string();
    for e in result.report.edits.iter().rev() {
        replay.replace_range(e.start as usize..e.end as usize, &e.replacement);
    }
    assert_eq!(replay, result.source);
}
fn changed(source: &str, name: &str, from: &str, to: &str) -> String {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = shape::analysis::parse(&allocator, source);
    assert!(parsed.errors.is_empty());
    let built = oxc_semantic::SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    assert!(built.errors.is_empty());
    let span = built
        .semantic
        .nodes()
        .iter()
        .find_map(|n| {
            let AstKind::Function(f) = n.kind() else {
                return None;
            };
            f.id.as_ref().filter(|i| i.name == name).map(|_| f.span)
        })
        .expect("actual full-span target");
    let own = &source[span.start as usize..span.end as usize];
    assert_eq!(own.matches(from).count(), 1, "{name} {from}");
    let mut out = source.to_string();
    out.replace_range(
        span.start as usize..span.end as usize,
        &own.replacen(from, to, 1),
    );
    out
}
fn denied(source: &str, name: &str) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = shape::analysis::parse(&allocator, source);
    assert!(parsed.errors.is_empty());
    let result = name_threads(source, true).unwrap();
    let target = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == name)
        .unwrap();
    assert!(
        !target.applied,
        "accepted actual altered target {name}: {target:?}"
    );
}
#[test]
fn whole_originals_alpha_and_neutral_scratch_declarations() {
    expected(SEPARATE_CLEANUP);
    expected(CAPTURED_CLEANUP);
    expected(
        &SEPARATE_CLEANUP
            .replace("$BO", "firstFormal")
            .replace("$KEY_67", "noSeedSentinel")
            .replace("$KEY_49", "noLastSentinel"),
    );
    let variant = changed(
        SEPARATE_CLEANUP,
        "$THREAD_3_143686",
        "let $KEY_4, $KEY_5;",
        "let $KEY_4, $KEY_5, untouchedLocal;",
    );
    expected(&variant);
}
#[test]
fn scan_rejects_altered_state_order_capture_and_effects() {
    let name = "$THREAD_3_143686";
    for (from, to) in [
        ("$KEY_1 = $KEY_67;", "$KEY_1 = other;"),
        ("$KEY_4 === $KEY_67", "$KEY_4 !== $KEY_67"),
        ("$KEY_4 = $BS;", "$KEY_4 = $BS; $KEY_3.next($BS);"),
        ("$KEY_0($KEY_4, $BS)", "$KEY_0($BS, $KEY_4)"),
        ("$KEY_3.error(e);", "$KEY_3.error($BS);"),
        ("$KEY_4 = $KEY_1;", "$KEY_4 = $KEY_1; extraEffect();"),
        (
            "return $KEY_5.unsubscribe();",
            "return other.unsubscribe();",
        ),
    ] {
        denied(&changed(SEPARATE_CLEANUP, name, from, to), name);
    }
}
#[test]
fn last_rejects_wrong_predicate_default_error_and_cleanup() {
    let name = "$THREAD_7_25773";
    for (from, to) in [
        ("$KEY_0($BS)", "$KEY_0($KEY_1)"),
        (
            "typeof $KEY_0 !== \"function\"",
            "typeof $KEY_0 === \"function\"",
        ),
        ("$KEY_5.error(e);", "$KEY_5.error($BS);"),
        ("No elements in sequence", "Different failure"),
        ("$KEY_5.next($KEY_1);", "$KEY_5.next($KEY_3);"),
        (
            "return $KEY_4.unsubscribe();",
            "return other.unsubscribe();",
        ),
    ] {
        denied(&changed(SEPARATE_CLEANUP, name, from, to), name);
    }
}
#[test]
fn callback_adapter_rejects_receiver_argument_order_and_extra_effects() {
    let name = "$THREAD_9_7360";
    for (from, to) in [
        ("$KEY_0.apply(null,", "$KEY_0.apply(this,"),
        ("new TypeError()", "new TypeError(\"changed\")"),
        ("$KEY_1.next($BQ);", "$KEY_1.next($BQ); extraEffect();"),
        ("$KEY_1.error($BQ);", "$KEY_1.error($KEY_0);"),
        ("arguments[i];", "arguments[0];"),
        ("i < $KEY_6", "i <= $KEY_6"),
    ] {
        denied(&changed(SEPARATE_CLEANUP, name, from, to), name);
    }
}
#[test]
fn sentinel_and_constructor_preinit_later_writer_branch_and_reentrant_negatives() {
    let name = "$THREAD_3_143686";
    for replacement in [
        "$KEY_67 = other;",
        "$KEY_67 = {}; $KEY_67 = other;",
        "if (flag) { $KEY_67 = {}; }",
        "$KEY_209(reducer)(source); $KEY_67 = {};",
        "register($KEY_209); $KEY_67 = {};",
    ] {
        assert_eq!(SEPARATE_CLEANUP.matches("$KEY_67 = {};").count(), 1);
        denied(&SEPARATE_CLEANUP.replacen("$KEY_67 = {};", replacement, 1), name);
    }
    for replacement in [
        "$THREAD_9_7360(fn)(); $KEY_315 = $THREAD_9_123048();",
        "if(flag){$KEY_315 = $THREAD_9_123048();}",
        "$KEY_315 = $THREAD_9_123048(); $KEY_315 = other;",
    ] {
        denied(
            &SEPARATE_CLEANUP.replacen("$KEY_315 = $THREAD_9_123048();", replacement, 1),
            "$THREAD_9_7360",
        );
    }
}

#[test]
fn callback_copy_refuses_explicit_arguments_scratch_holder() {
    let name = "$THREAD_9_7360";
    let source = changed(
        SEPARATE_CLEANUP,
        name,
        "let $KEY_6 = arguments.length;",
        "var arguments; let $KEY_6 = arguments.length;",
    );
    let source = changed(
        &source,
        name,
        "let $KEY_5 = new Array($KEY_6);",
        "arguments = new Array($KEY_6);",
    );
    let source = changed(
        &source,
        name,
        "$KEY_5[i] = arguments[i];",
        "arguments[i] = arguments[i];",
    );
    let source = changed(&source, name, "$KEY_5.concat(", "arguments.concat(");
    denied(&source, name);
}

fn two_stage_expected(source: &str) {
    expected(source);
    let result = name_threads(source, true).unwrap();
    let got: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|o| o.applied && o.evaluator.as_deref() == Some("shared_two_stage_pipe_operator"))
        .map(|o| o.role_name.as_deref().unwrap())
        .collect();
    assert_eq!(
        got,
        ["createTwoStagePipeOperator", "applyTwoStagePipeOperator"]
    );
}
#[test]
fn two_stage_originals_alpha_capture_order_and_opaque_helpers() {
    two_stage_expected(SEPARATE_CLEANUP);
    two_stage_expected(CAPTURED_CLEANUP);
    two_stage_expected(
        &SEPARATE_CLEANUP
            .replace("$BQ", "sourceFormal")
            .replace("$KEY_209", "firstHelperCapture"),
    );
    let variant = changed(
        SEPARATE_CLEANUP,
        "$THREAD_3_132516",
        "$KEY_209($KEY_1, $KEY_0), $KEY_205(void 0, $KEY_0)",
        "$KEY_205($KEY_1, $KEY_0), $KEY_209(void 0, $KEY_0)",
    );
    two_stage_expected(&variant);
}
#[test]
fn two_stage_rejects_changed_receiver_order_effects_and_capture_readiness() {
    let name = "$THREAD_3_132516";
    for (from, to) in [
        ("$KEY_209($KEY_1, $KEY_0)", "$KEY_209($KEY_0, $KEY_1)"),
        ("$KEY_205(void 0, $KEY_0)", "$KEY_205(1, $KEY_0)"),
        ("$KEY_205(void 0, $KEY_0)", "$KEY_205(void 0, $KEY_1)"),
        ("return $BQ.pipe(", "return $KEY_1.pipe("),
        ("return $BQ.pipe(", "extraEffect(); return $BQ.pipe("),
        ("return $BQ.pipe(", "return $BQ.pipe?.("),
        ("let $KEY_1 = $BO;", "let $KEY_1 = $BO; $KEY_1 = $BP;"),
    ] {
        denied(&changed(SEPARATE_CLEANUP, name, from, to), name);
    }
    let original = "let $KEY_209 = $THREAD_3_143686;";
    assert_eq!(SEPARATE_CLEANUP.matches(original).count(), 1);
    for replacement in [
        "$THREAD_3_132516(null,null)({pipe(){}}); let $KEY_209 = $THREAD_3_143686;",
        "register($THREAD_3_132516); let $KEY_209 = $THREAD_3_143686;",
        "let $KEY_209; if(flag){ $KEY_209 = $THREAD_3_143686; }",
        "let $KEY_209 = $THREAD_3_143686; $KEY_209 = other;",
        "let earlyStage=lateStage; let $KEY_209=earlyStage; let lateStage=$THREAD_3_143686;",
    ] {
        denied(&SEPARATE_CLEANUP.replacen(original, replacement, 1), name);
    }
}

fn named_expression(source: &str, name: &str, invoke: bool) -> String {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = shape::analysis::parse(&allocator, source);
    assert!(parsed.errors.is_empty());
    let built = oxc_semantic::SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    assert!(built.errors.is_empty());
    let span = built
        .semantic
        .nodes()
        .iter()
        .find_map(|n| {
            let AstKind::Function(f) = n.kind() else {
                return None;
            };
            f.id.as_ref().filter(|i| i.name == name).map(|_| f.span)
        })
        .unwrap();
    let own = &source[span.start as usize..span.end as usize];
    // Same spelling, distinct external var and private self SymbolIds.
    let mut replacement = format!("var {name} = {own};");
    if invoke {
        replacement += &format!(" {name}(null,void 0)(null);");
    }
    let mut out = source.to_string();
    out.replace_range(span.start as usize..span.end as usize, &replacement);
    out
}
#[test]
fn named_function_expression_private_self_is_not_external_exposure() {
    for name in [
        "$THREAD_7_35867",
        "$THREAD_3_143686",
        "$THREAD_7_25773",
        "$THREAD_9_7360",
        "$THREAD_3_132516",
    ] {
        denied(
            &named_expression(SEPARATE_CLEANUP, name, name != "$THREAD_3_132516"),
            name,
        );
    }
}
#[test]
fn constructor_alias_copy_requires_producer_at_the_executed_read() {
    let original = "$KEY_315 = $THREAD_9_123048();";
    assert_eq!(SEPARATE_CLEANUP.matches(original).count(), 1);
    let positive = SEPARATE_CLEANUP.replacen(
        original,
        "let ctorProducer=$THREAD_9_123048(); $KEY_315=ctorProducer;",
        1,
    );
    expected(&positive);
    let map = name_threads(&positive, true).unwrap();
    assert_eq!(
        map.report
            .outcomes
            .iter()
            .filter(
                |o| o.applied && o.evaluator.as_deref() == Some("shared_observable_map_operator")
            )
            .count(),
        3
    );
    let cleanup = map
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_7_25410")
        .unwrap();
    assert_eq!(
        cleanup.evaluator.as_deref(),
        Some("captured_subscription_cleanup")
    );
    assert_eq!(
        cleanup.proposed_name.as_deref(),
        Some("unsubscribeCapturedSource")
    );
    let negative = SEPARATE_CLEANUP.replacen(
        original,
        "$KEY_315=ctorProducer; let ctorProducer=$THREAD_9_123048();",
        1,
    );
    for name in [
        "$THREAD_7_35867",
        "$THREAD_3_143686",
        "$THREAD_7_25773",
        "$THREAD_9_7360",
    ] {
        denied(&negative, name);
    }
    let between_stores = SEPARATE_CLEANUP.replacen(
        original,
        "let ctorProducer=$THREAD_9_123048(); $THREAD_7_35867(null)(null); $KEY_315=ctorProducer;",
        1,
    );
    denied(&between_stores, "$THREAD_7_35867");
}

#[test]
fn full_current_parent_cleanup_outcomes_and_alpha_keep_exact_ownership() {
    let old: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/naming/subscription_operators/parent-outcomes.json"
    ))
    .unwrap();
    for (key, source, binding) in [
        ("subscription-parent-alternate", SEPARATE_CLEANUP, "$THREAD_7_25410"),
        ("subscription-parent-baseline", CAPTURED_CLEANUP, "$THREAD_6_139757"),
    ] {
        let result = name_threads(source, true).unwrap();
        let outcome = result
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == binding)
            .unwrap();
        assert_eq!(
            serde_json::to_value(outcome).unwrap(),
            old[key],
            "fresh Root outcome must be retained exactly"
        );
        let mapped: Vec<_> = result
            .report
            .outcomes
            .iter()
            .filter(|o| {
                o.applied && o.evaluator.as_deref() == Some("shared_observable_map_operator")
            })
            .map(|o| o.role_name.as_deref().unwrap())
            .collect();
        assert_eq!(
            mapped,
            ["createMapOperator", "mapOperatorFunction", "mapNextHandler"]
        );
        let alpha = source
            .replace("$BQ", "sourceFormalAlpha")
            .replace("$KEY_315", "constructorAliasAlpha");
        let alpha_result = name_threads(&alpha, true).unwrap();
        let alpha_outcome = alpha_result
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == binding)
            .unwrap();
        for field in [
            "evaluator",
            "proposed_name",
            "role_name",
            "explanation",
            "confidence",
        ] {
            assert_eq!(
                serde_json::to_value(alpha_outcome).unwrap()[field],
                old[key][field]
            );
        }
    }
}

#[test]
fn additional_map_proof_refuses_projection_callback_and_receiver_changes() {
    let name = "$THREAD_7_35867";
    for (from, to) in [
        ("$KEY_2.next($KEY_0($BS));", "$KEY_2.next($BS);"),
        ("$KEY_2.error(e);", "$KEY_2.error($BS);"),
        ("$KEY_1.subscribe($BR)", "other.subscribe($BR)"),
        (
            "return $KEY_3.unsubscribe();",
            "return other.unsubscribe();",
        ),
        (
            "$BR.complete = $THREAD_9_123041;",
            "$BR.complete = $THREAD_4_36343;",
        ),
    ] {
        denied(&changed(SEPARATE_CLEANUP, name, from, to), name);
    }
}
