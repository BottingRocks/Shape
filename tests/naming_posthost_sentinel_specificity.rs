//! Focused proofs for the generic captured-state versus immutable-sentinel
//! specificity rule.
//!
//! `thread_shared_state` proves the generic `readCapturedState` /
//! `isCapturedState` roles for any plain captured variable. `thread_shared_wrappers`
//! independently proves `getNullPrototypeSentinel` / `isNullPrototypeSentinel`
//! when that same captured value is an exact immutable `Object.create(null)`
//! sentinel. The sentinel proof is strictly stronger, so the same-function pair
//! collapses onto the sentinel role. A solo generic claim, a mutable captured
//! object, the mutable triple storage role, a genuine third conflict, shadowed
//! `Object`, runtime capture origins, scope collisions and existing readable
//! names must all stay exactly as they were.
use shape::naming::labels::build_label_reference;
use shape::naming::{name_threads, name_threads_with_labels, NamedThreads, ThreadName};

const PREDICATE: &str = r#"function main(){const token=Object.create(null);function $THREAD_470_662(value){return value===token;}return $THREAD_470_662;}main();"#;
const GETTER: &str = r#"function main(){const token=Object.create(null);function $THREAD_470_662(){return token;}return $THREAD_470_662;}main();"#;
const TRIPLE: &str = r#"function main(){let first=Object.create(null),second=[],third=0;function $THREAD_23_94(a,b,c){first=a;second=b;third=c;}return $THREAD_23_94;}main();"#;

fn outcome<'a>(result: &'a NamedThreads, target: &str) -> &'a ThreadName {
    result
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == target)
        .unwrap_or_else(|| panic!("missing outcome for {target}"))
}

fn role_candidates(result: &NamedThreads, target: &str) -> Vec<String> {
    result
        .report
        .function_roles
        .iter()
        .find(|role| role.current_name.as_deref() == Some(target))
        .map(|role| role.candidates.clone())
        .unwrap_or_else(|| panic!("missing function role for {target}"))
}

fn role_evaluators(result: &NamedThreads, target: &str) -> Vec<String> {
    result
        .report
        .function_roles
        .iter()
        .find(|role| role.current_name.as_deref() == Some(target))
        .map(|role| role.evaluators.clone())
        .unwrap_or_else(|| panic!("missing function role for {target}"))
}

fn role_status(result: &NamedThreads, target: &str) -> String {
    result
        .report
        .function_roles
        .iter()
        .find(|role| role.current_name.as_deref() == Some(target))
        .map(|role| role.status.clone())
        .unwrap_or_else(|| panic!("missing function role for {target}"))
}

fn assert_applied(source: &str, target: &str, name: &str, evaluator: &str) -> NamedThreads {
    let result = name_threads(source, true).unwrap_or_else(|error| panic!("{error}\n{source}"));
    let applied = outcome(&result, target);
    assert!(
        applied.applied && applied.proposed_name.as_deref() == Some(name),
        "expected {target} as {name}, got {:?} applied={} reason={:?}\n{source}",
        applied.proposed_name,
        applied.applied,
        applied.reason
    );
    assert_eq!(applied.evaluator.as_deref(), Some(evaluator), "{source}");
    assert_eq!(role_candidates(&result, target), vec![name.to_string()], "{source}");
    assert_eq!(role_evaluators(&result, target), vec![evaluator.to_string()], "{source}");
    assert_eq!(role_status(&result, target), "identified", "{source}");
    result
}

#[test]
fn exact_immutable_predicate_prefers_sentinel_specialization() {
    let result = assert_applied(
        PREDICATE,
        "$THREAD_470_662",
        "isNullPrototypeSentinel",
        "shared_immutable_wrapper",
    );
    assert!(result.source.contains("function isNullPrototypeSentinel("));
    assert!(outcome(&result, "$THREAD_470_662")
        .explanation
        .iter()
        .any(|line| line.contains("supersedes the generic captured-state")));
}

#[test]
fn exact_immutable_getter_prefers_getter_specialization() {
    let result = assert_applied(
        GETTER,
        "$THREAD_470_662",
        "getNullPrototypeSentinel",
        "shared_immutable_wrapper",
    );
    assert!(result.source.contains("function getNullPrototypeSentinel("));
}

#[test]
fn generic_captured_state_without_sentinel_proof_is_unchanged() {
    for (source, target, name) in [
        (
            "function main(){let state;function $THREAD_1_2(){return state;}return $THREAD_1_2;}main();",
            "$THREAD_1_2",
            "readCapturedState",
        ),
        (
            "function main(){let state;function $THREAD_11_21(value){return value===state;}return $THREAD_11_21;}main();",
            "$THREAD_11_21",
            "isCapturedState",
        ),
        (
            "function main(){const state=[0];function $THREAD_1_3(){return state;}return $THREAD_1_3;}main();",
            "$THREAD_1_3",
            "readCapturedState",
        ),
        (
            "function main(){const state={};function $THREAD_11_22(value){return value===state;}return $THREAD_11_22;}main();",
            "$THREAD_11_22",
            "isCapturedState",
        ),
    ] {
        assert_applied(source, target, name, "shared_state_slots");
    }
}

#[test]
fn mutable_captured_objects_and_field_storage_are_not_sentinel() {
    // A mutable object, even one allocated with Object.create({}), keeps the
    // generic captured read: only Object.create(null) establishes the sentinel.
    for (source, target, name) in [
        (
            "function main(){const token=Object.create({});function $THREAD_1_4(){return token;}return $THREAD_1_4;}main();",
            "$THREAD_1_4",
            "readCapturedState",
        ),
        (
            "function main(){let token=Object.create({});token.value=1;function $THREAD_1_5(){return token;}return $THREAD_1_5;}main();",
            "$THREAD_1_5",
            "readCapturedState",
        ),
        (
            "function main(){const token={};function $THREAD_11_23(value){return value===token;}return $THREAD_11_23;}main();",
            "$THREAD_11_23",
            "isCapturedState",
        ),
    ] {
        let result = assert_applied(source, target, name, "shared_state_slots");
        assert!(
            !role_candidates(&result, target)
                .iter()
                .any(|candidate| candidate.ends_with("NullPrototypeSentinel")),
            "{source}"
        );
    }
    // The mutable three-slot storage role is a field role, never a sentinel.
    assert_applied(
        TRIPLE,
        "$THREAD_23_94",
        "setCapturedStateTriple",
        "shared_state_slots",
    );
}

#[test]
fn preexisting_solo_sentinel_roles_are_preserved() {
    for (source, target, name) in [
        (
            "function main(){const token=Object.create(null);function $THREAD_1_1(){const captured=token;return captured;}return $THREAD_1_1;}main();",
            "$THREAD_1_1",
            "getNullPrototypeSentinel",
        ),
        (
            "function main(){const token=Object.create(null);function $THREAD_11_21(value){const item=value;return item===token;}return $THREAD_11_21;}main();",
            "$THREAD_11_21",
            "isNullPrototypeSentinel",
        ),
    ] {
        assert_applied(source, target, name, "shared_immutable_wrapper");
    }
}

#[test]
fn real_third_conflict_still_declines() {
    // The independent structural label is a genuine third candidate. The
    // specificity rule removes only the generic captured-state claim; it never
    // removes the third name, so the binding stays honestly ambiguous.
    let human = PREDICATE.replace("$THREAD_470_662", "thirdConflictingRole");
    let reference = build_label_reference(PREDICATE, &human).unwrap();
    let result = name_threads_with_labels(PREDICATE, true, Some(&reference)).unwrap();
    let target = "$THREAD_470_662";
    let conflicted = outcome(&result, target);
    assert!(!conflicted.applied, "{:?}", conflicted.reason);
    assert!(
        conflicted
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("conflicting")),
        "{:?}",
        conflicted.reason
    );
    assert_eq!(role_status(&result, target), "ambiguous");
    assert_eq!(
        role_candidates(&result, target),
        vec![
            "isNullPrototypeSentinel".to_string(),
            "thirdConflictingRole".to_string()
        ]
    );
    assert!(!role_candidates(&result, target).contains(&"isCapturedState".to_string()));
    assert_eq!(result.source, PREDICATE);
}

#[test]
fn shadow_ownership_collision_and_human_names_preserve_invariants() {
    let target = "$THREAD_470_662";

    // A shadowed Object cannot prove the null-prototype origin, so the generic
    // captured predicate remains the honest role.
    let shadowed = PREDICATE.replace(
        "const token=Object.create(null);",
        "const Object=custom;const token=Object.create(null);",
    );
    let result = name_threads(&shadowed, true).unwrap();
    let generic = outcome(&result, target);
    assert!(generic.applied, "{shadowed}");
    assert_eq!(generic.proposed_name.as_deref(), Some("isCapturedState"));
    assert!(!role_candidates(&result, target)
        .contains(&"isNullPrototypeSentinel".to_string()));

    // Parameter, destructuring and loop origins are not captured plain
    // variables, so neither the generic state role nor the sentinel role fires.
    for source in [
        PREDICATE.replace(
            "function main(){const token=Object.create(null);",
            "function main(token){",
        ),
        PREDICATE.replace(
            "const token=Object.create(null);",
            "const {token}=configuration;",
        ),
        PREDICATE.replace(
            "function main(){const token=Object.create(null);",
            "function main(){for(const token of [Object.create(null)]){",
        )
        .replace("}main();", "}}main();"),
    ] {
        let result = name_threads(&source, true).unwrap_or_else(|error| panic!("{error}\n{source}"));
        let runtime = outcome(&result, target);
        assert!(!runtime.applied, "{source}");
        assert!(runtime.proposed_name.is_none(), "{source}");
    }

    // An existing unsuffixed role in a related scope keeps the reservation:
    // the sentinel role must not be applied as a shadowing collision.
    let collision = PREDICATE.replace(
        "function main(){",
        "function main(){let isNullPrototypeSentinel=0;consume(isNullPrototypeSentinel);",
    );
    let result = name_threads(&collision, true).unwrap();
    let reserved = outcome(&result, target);
    assert!(!reserved.applied, "{:?}", reserved.reason);
    assert!(
        reserved.reason.as_deref().is_some_and(|reason| reason
            .to_ascii_lowercase()
            .contains("collision")),
        "{:?}",
        reserved.reason
    );
    assert_eq!(result.source, collision);

    // Existing human names and KEY placeholders are never rename targets.
    for human in [
        PREDICATE.replace(target, "chosenSentinelHelper"),
        PREDICATE.replace(target, "$KEY_812"),
    ] {
        assert_eq!(name_threads(&human, true).unwrap().source, human);
    }
}

#[test]
fn edit_replay_is_sorted_by_byte_offset() {
    let result = name_threads(PREDICATE, true).unwrap();
    let mut edits = result.report.edits.clone();
    edits.sort_by_key(|edit| edit.start);
    let mut replay = PREDICATE.to_string();
    for edit in edits.iter().rev() {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
}
