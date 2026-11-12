use shape::naming::{name_threads, ThreadName};

#[path = "naming_captured_observer_forwarding.rs"]
mod return_controls;

const ORIGINAL: &str = include_str!("fixtures/discarded_result_captured_next/owning-scope.js");
const TARGET: &str = "$THREAD_0_19761";
const CALL: &str = "$KEY_8.next($CI);";

fn check(source: &str, expected: Option<&str>) -> ThreadName {
    let result = name_threads(source, true).expect("mutation must parse and analyze");
    let candidates: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|o| o.binding.name == TARGET)
        .collect();
    assert_eq!(
        candidates.len(),
        1,
        "target must be enumerated exactly once"
    );
    let outcome = candidates[0];
    if let Some(role) = expected {
        assert!(outcome.applied, "{outcome:?}");
        assert_eq!(outcome.role_name.as_deref(), Some(role), "{outcome:?}");
        assert_eq!(
            outcome.evaluator.as_deref(),
            Some("shared_captured_observer_forwarding")
        );
        if role == "callCapturedNext" {
            assert!(result.source.contains("$KEY_8.next($CI);"));
            assert!(outcome
                .explanation
                .iter()
                .any(|e| e.contains("discards the result")));
        }
    } else {
        assert_ne!(
            outcome.role_name.as_deref(),
            Some("callCapturedNext"),
            "{outcome:?}"
        );
    }
    outcome.clone()
}
fn mutate(source: &str, from: &str, to: &str) -> String {
    if matches!(from, CALL | "vmPostToWorker(7, 93340, {") {
        // The sibling arrow has the same call. Mutate only this declaration
        // and its own registration, never that sibling's evidence.
        let start = source.find("function $THREAD_0_19761").unwrap();
        let end =
            source.find("}, 461, $THREAD_0_19761);").unwrap() + "}, 461, $THREAD_0_19761);".len();
        assert_eq!(source[start..end].matches(from).count(), 1);
        return format!(
            "{}{}{}",
            &source[..start],
            source[start..end].replacen(from, to, 1),
            &source[end..]
        );
    }
    assert_eq!(source.matches(from).count(), 1, "mutation anchor: {from}");
    source.replacen(from, to, 1)
}
#[test]
fn full_actual_owner_and_alpha_and_return_control() {
    assert_eq!(ORIGINAL.len(), 6643);
    let outcome = check(ORIGINAL, Some("callCapturedNext"));
    assert_eq!(outcome.reference_edits, 1);
    let alias = mutate(
        ORIGINAL,
        "$KEY_8 = $BO[2];",
        "const originAlias = $BO[2]; $KEY_8 = originAlias;",
    );
    check(&alias, Some("callCapturedNext"));
    let alpha = ORIGINAL
        .replace("$KEY_8", "alphaReceiver")
        .replace("$CI", "alphaValue")
        .replace("$BO", "alphaInputs");
    let result = name_threads(&alpha, true).unwrap();
    let outcomes: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|o| o.binding.name == TARGET)
        .collect();
    assert_eq!(outcomes.len(), 1);
    assert!(outcomes[0].applied);
    assert_eq!(outcomes[0].role_name.as_deref(), Some("callCapturedNext"));
    let returned = mutate(ORIGINAL, CALL, "return $KEY_8.next($CI);");
    let old = check(&returned, Some("forwardValueToCapturedObserver"));
    assert!(old
        .explanation
        .iter()
        .any(|e| e.starts_with("whole owned body returns exactly")));
}
#[test]
fn exact_call_and_function_shape_negatives() {
    for replacement in [
        "$KEY_8.next($CI);extra();",
        "$KEY_8.next($BO);",
        "other.next($CI);",
        "$KEY_8.error($CI);",
        "$KEY_8?.next($CI);",
        "$KEY_8.next?.($CI);",
        "$KEY_8.next(...$CI);",
        "$KEY_8.next($CI,$CI);",
        "$KEY_8.next();",
        "return void $KEY_8.next($CI);",
        "$CI=other;$KEY_8.next($CI);",
        "'use strict';$KEY_8.next($CI);",
    ] {
        check(&mutate(ORIGINAL, CALL, replacement), None);
    }
    for replacement in [
        "async function $THREAD_0_19761($CI)",
        "function* $THREAD_0_19761($CI)",
        "function $THREAD_0_19761($CI=other)",
        "function $THREAD_0_19761(...$CI)",
        "function $THREAD_0_19761({$CI})",
    ] {
        check(
            &mutate(ORIGINAL, "function $THREAD_0_19761($CI)", replacement),
            None,
        );
    }
    // Keep the private self candidate present; registration uses the real outer binding.
    let expression = mutate(
        ORIGINAL,
        "function $THREAD_0_19761($CI)",
        "const external = function $THREAD_0_19761($CI)",
    );
    let expression = mutate(
        &expression,
        "}, 461, $THREAD_0_19761);",
        "}, 461, external);",
    );
    check(&expression, None);
}
#[test]
fn actual_capture_origin_all_writers_and_scheduler_paths() {
    for replacement in [
        "if (ready) $KEY_8 = $BO[2];",
        "function deferredStore() { $KEY_8 = $BO[2]; }",
        "$KEY_8 = $BO[2]; $KEY_8 = other;",
        "$KEY_8 = unknown;",
        "$KEY_8 = $BO[2]; $BO = other;",
        "$KEY_8 = lateAlias; let lateAlias = $BO[2];",
    ] {
        check(&mutate(ORIGINAL, "$KEY_8 = $BO[2];", replacement), None);
    }
    let late = mutate(ORIGINAL, "$KEY_8 = $BO[2];", "");
    // Both scheduling calls are opaque and may run synchronously.
    for anchor in ["$BR(() => {", "$BQ(() => {"] {
        check(
            &mutate(&late, anchor, &format!("{anchor} $KEY_8 = $BO[2];")),
            None,
        );
    }
    check(&late, None);
    let no_store = mutate(ORIGINAL, "$KEY_8 = $BO[2];", "");
    let late_store = mutate(
        &no_store,
        "vmPostToWorker(7, 93340, {",
        "$KEY_8 = $BO[2]; vmPostToWorker(7, 93340, {",
    );
    check(&late_store, None);
}
#[test]
fn actual_target_early_registration_alias_and_unused_not_vacuous() {
    for replacement in [
        "vmWorkerCallbacks.push($THREAD_0_19761); vmPostToWorker(7, 93340, {",
        "const callbackAlias=$THREAD_0_19761; vmWorkerCallbacks.push(callbackAlias); vmPostToWorker(7, 93340, {",
    ] {
        // Early exposure means before setup in the outer factory, not merely
        // an earlier statement after setup inside the registered callback.
        let no_store = mutate(ORIGINAL, "$KEY_8 = $BO[2];", "");
        let source = mutate(&no_store, "vmPostToWorker(7, 93340, {", replacement);
        let source = mutate(&source, "}, 461, $THREAD_0_19761);", "}, 461, $THREAD_0_19761); $KEY_8 = $BO[2];");
        check(&source, None);
    }
    let unused = mutate(
        ORIGINAL,
        "}, 461, $THREAD_0_19761);",
        "}, 461, opaqueCallback);",
    );
    check(&unused, None);
    for prelude in [
        "register($THREAD_0_19761);",
        "const alias=$THREAD_0_19761;register(alias);",
    ] {
        let source = format!("function factory(input){{{prelude} let receiver=input;function {TARGET}(value){{receiver.next(value);}}register({TARGET});}}factory(opaque);");
        check(&source, None);
    }
}
