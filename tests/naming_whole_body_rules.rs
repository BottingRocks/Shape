use shape::naming::name_threads;

const FIXTURE: &str = include_str!("fixtures/naming/whole_body_rules/basic.js");

const WRAPPERS: &str = "runtime_captured_callee_wrappers";
const THROW: &str = "runtime_undefined_value_read_error";
const NATIVE: &str = "runtime_native_function_source_check";
const INSTALLER: &str = "shared_extension_registry_installer";
const VISIBILITY: &str = "events_visibility_change_sample";
const CLOCK: &str = "runtime_batch_clock_constructor";

fn names(source: &str, evaluator: &str) -> Vec<String> {
    let result = name_threads(source, true).unwrap();
    let mut replay = source.to_string();
    for edit in result.report.edits.iter().rev() {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
    result
        .report
        .outcomes
        .iter()
        .filter(|outcome| outcome.applied && outcome.evaluator.as_deref() == Some(evaluator))
        .map(|outcome| outcome.proposed_name.clone().unwrap_or_default())
        .collect()
}

fn has(source: &str, evaluator: &str, name: &str) -> bool {
    names(source, evaluator).iter().any(|candidate| candidate == name)
}

fn edit(from: &str, to: &str) -> String {
    assert_eq!(FIXTURE.matches(from).count(), 1, "unique mutation {from}");
    FIXTURE.replacen(from, to, 1)
}

#[test]
fn whole_own_bodies_are_named() {
    for (evaluator, name) in [
        (WRAPPERS, "invokeCapturedCallableDiscardResult"),
        (WRAPPERS, "forwardFormalAndCapturedToCapturedCallable"),
        (WRAPPERS, "appendFormalToCapturedList"),
        (WRAPPERS, "invokeFormalAndReportCaughtErrorToCapturedObservers"),
        (WRAPPERS, "resetCapturedSlotsAndInvokeOptionalDisposer"),
        (THROW, "throwUndefinedValueReadTypeError"),
        (NATIVE, "isNativeFunctionSource"),
        (INSTALLER, "installExtensionRegistry"),
        (VISIBILITY, "recordVisibilityChangeSample"),
        (CLOCK, "initializeBatchClockState"),
    ] {
        assert!(has(FIXTURE, evaluator, name), "expected {evaluator}:{name}");
    }
}

#[test]
fn alpha_layouts_still_match() {
    let source = FIXTURE
        .replace("$HC", "value")
        .replace("$KEY_32", "notifyReady")
        .replace("$KEY_8", "observerList")
        .replace("$KEY_115", "worker")
        .replace("$KEY_77", "emitter")
        .replace("$THREAD_9_9000", "$THREAD_1_7000")
        .replace("$THREAD_8_70754", "$THREAD_2_4242")
        .replace("$THREAD_9_14073", "$THREAD_3_4343");
    for (evaluator, name) in [
        (WRAPPERS, "invokeCapturedCallableDiscardResult"),
        (WRAPPERS, "forwardFormalAndCapturedToCapturedCallable"),
        (WRAPPERS, "appendFormalToCapturedList"),
        (INSTALLER, "installExtensionRegistry"),
    ] {
        assert!(has(&source, evaluator, name), "alpha layout {evaluator}:{name}");
    }
}

#[test]
fn wrapper_near_misses_are_refused() {
    assert!(!has(
        &edit("$KEY_32();", "$KEY_32(1);"),
        WRAPPERS,
        "invokeCapturedCallableDiscardResult"
    ));
    assert!(!has(
        &edit("$KEY_32();", "$KEY_32;"),
        WRAPPERS,
        "invokeCapturedCallableDiscardResult"
    ));
    assert!(!has(
        &edit(
            "return $KEY_0($HC, $KEY_5);",
            "return $KEY_0($KEY_5, $HC);"
        ),
        WRAPPERS,
        "forwardFormalAndCapturedToCapturedCallable"
    ));
    assert!(!has(
        &edit("return $KEY_0($HC, $KEY_5);", "return $KEY_0($HC, $HC);"),
        WRAPPERS,
        "forwardFormalAndCapturedToCapturedCallable"
    ));
    assert!(!has(
        &edit("$KEY_8.push($GW);", "$KEY_8.unshift($GW);"),
        WRAPPERS,
        "appendFormalToCapturedList"
    ));
    assert!(!has(
        &edit(
            "$KEY_8.forEach($GX => $GX.error($KEY_0));",
            "$KEY_8.forEach($GX => $GX.warn($KEY_0));"
        ),
        WRAPPERS,
        "invokeFormalAndReportCaughtErrorToCapturedObservers"
    ));
    assert!(!has(
        &edit("$KEY_12 = $KEY_208;", "$KEY_12 = $KEY_208 + 1;"),
        WRAPPERS,
        "resetCapturedSlotsAndInvokeOptionalDisposer"
    ));
    assert!(!has(
        &edit("$KEY_12 = $KEY_208;", "$KEY_12 = $KEY_208; $KEY_2();"),
        WRAPPERS,
        "resetCapturedSlotsAndInvokeOptionalDisposer"
    ));
}

#[test]
fn literal_and_guard_near_misses_are_refused() {
    assert!(!has(
        &edit(
            "Cannot read properties of undefined (reading 'value')",
            "Cannot read properties of undefined (reading 'name')"
        ),
        THROW,
        "throwUndefinedValueReadTypeError"
    ));
    assert!(!has(
        &edit("Function.prototype.toString", "Object.prototype.toString"),
        NATIVE,
        "isNativeFunctionSource"
    ));
    assert!(!has(
        &edit("{ key: \"extend\" }", "{ key: \"exteNd\" }"),
        INSTALLER,
        "installExtensionRegistry"
    ));
    assert!(!has(
        &edit("{ key: \"extend\" }", "{ key: \"extend\", value: 0 }"),
        INSTALLER,
        "installExtensionRegistry"
    ));
    assert!(!has(
        &edit("return $KEY_0;\n}", "return $KEY_0.extensions;\n}"),
        INSTALLER,
        "installExtensionRegistry"
    ));
    assert!(!has(
        &edit("\"use strict\";\n\tthis.targetBatchTimeMs", "this.targetBatchTimeMs"),
        CLOCK,
        "initializeBatchClockState"
    ));
    assert!(!has(
        &edit("this._internalNow = $KEY_1260(", "this.targetBatchTimeMs = $KEY_1260("),
        CLOCK,
        "initializeBatchClockState"
    ));
}

#[test]
fn undefined_value_throw_has_owned_role_evidence_and_exact_protocol() {
    let source = "function main(){function $THREAD_91_1(){throw new TypeError(\"Cannot read properties of undefined (reading 'value')\");}return $THREAD_91_1;}main();";
    let result = name_threads(source, true).unwrap();
    let outcome = result.report.outcomes.iter().find(|o| o.binding.name == "$THREAD_91_1").unwrap();
    assert!(outcome.applied);
    assert_eq!(outcome.evaluator.as_deref(), Some(THROW));
    let role = result.report.function_roles.iter().find(|r| {
        r.function_span.start == outcome.binding.declaration.start
            && r.function_span.end == outcome.binding.declaration.end
    }).unwrap();
    assert_eq!(role.status, "identified");
    assert!(role.candidates.iter().any(|n| n == "throwUndefinedValueReadTypeError"));
    assert!(role.evaluators.iter().any(|e| e == THROW));
    assert_eq!(role.evidence, outcome.evidence);
    let evidence: Vec<_> = role.evidence.iter().map(|s| &source[s.start as usize..s.end as usize]).collect();
    assert_eq!(evidence, vec![
        "throw new TypeError(\"Cannot read properties of undefined (reading 'value')\");",
        "new TypeError(\"Cannot read properties of undefined (reading 'value')\")",
        "TypeError",
        "\"Cannot read properties of undefined (reading 'value')\"",
    ]);
    for changed in [
        source.replace("reading 'value'", "reading 'name'"),
        source.replace("function main(){", "function main(){let TypeError = other;"),
        source.replace("(){throw", "(){extra();throw"),
        source.replace("$THREAD_91_1()", "$THREAD_91_1(value)"),
        source.replace("$THREAD_91_1()", "$THREAD_91_1(...values)"),
        source.replace("function $THREAD", "async function $THREAD"),
        source.replace("function $THREAD", "function* $THREAD"),
        source.replace("new TypeError", "new errors.TypeError"),
    ] {
        assert!(!has(&changed, THROW, "throwUndefinedValueReadTypeError"));
    }
}
