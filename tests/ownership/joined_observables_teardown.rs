use super::{name_threads, named_expression};
const JOINED_OBSERVABLES: &str = include_str!("../fixtures/naming/joined_observables/joined-state-parent-alternate.js");
const TARGET: &str = include_str!("../fixtures/naming/joined_observables/joined-state-parent-baseline.js");
const EVAL: &str = "shared_observable_fork_join";
const ROLE: &str = "unsubscribeJoinedSources";

fn check(source: &str, name: &str, expected: bool) {
    let result = name_threads(source, true).unwrap();
    let matches: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|o| o.binding.name == name)
        .collect();
    assert_eq!(matches.len(), 1, "nonvacuous target {name}");
    let o = matches[0];
    assert_eq!(
        o.applied && o.evaluator.as_deref() == Some(EVAL) && o.role_name.as_deref() == Some(ROLE),
        expected,
        "{name}: {o:?}"
    );
    if !expected {
        assert!(
            !o.applied,
            "unsupported target acquired a different label: {o:?}"
        );
    }
}

#[test]
fn actual_original_spans_and_alpha_renaming() {
    for (source, name, start, end, owner) in [
        (JOINED_OBSERVABLES, "$THREAD_3_81450", 17566, 17647, "$THREAD_7_93275"),
        (TARGET, "$THREAD_6_137886", 5677, 5759, "$THREAD_7_134167"),
    ] {
        let result = name_threads(source, true).unwrap();
        let target = result
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == name)
            .unwrap();
        assert_eq!(
            (
                target.binding.declaration.start,
                target.binding.declaration.end
            ),
            (start, end)
        );
        assert_eq!(target.reference_edits, 1);
        assert!(source[start as usize..end as usize].starts_with(&format!("function {name}()")));
        assert_eq!(target.role_name.as_deref(), Some(ROLE));
        assert_eq!(target.evaluator.as_deref(), Some(EVAL));
        assert!(target.applied);
        let parent = result
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == owner)
            .unwrap();
        assert_eq!(
            parent.role_name.as_deref(),
            Some("joinObservableLastValues")
        );
        let alpha = source
            .replace(name, "$THREAD_9_999995")
            .replace("$KEY_3", "joinedCapture")
            .replace("$BQ", "joinedElement")
            .replace("$AT", "joinedSource");
        check(&alpha, "$THREAD_9_999995", true);
    }
    // Earlier generic return-result cleanup keeps its established name/evaluator.
    let result = name_threads(TARGET, true).unwrap();
    let old = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_1_56901")
        .unwrap();
    assert_eq!(
        old.role_name.as_deref(),
        Some("unsubscribeEachCapturedHandle")
    );
    assert_eq!(
        old.evaluator.as_deref(),
        Some("shared_captured_observer_forwarding")
    );
}

fn joined_observables_owner(replacement: impl FnOnce(&str) -> String) -> String {
    let own = &JOINED_OBSERVABLES[16709..17686];
    let changed = replacement(own);
    assert_ne!(changed, own, "control must alter actual owner");
    let mut source = JOINED_OBSERVABLES.to_owned();
    source.replace_range(16709..17686, &changed);
    source
}

#[test]
fn exact_teardown_and_whole_join_gates_refuse_mutations() {
    for (from, to) in [
        (
            "$KEY_3.forEach($BQ => $BQ.unsubscribe());",
            "$KEY_4.forEach($BQ => $BQ.unsubscribe());",
        ),
        ("$BQ => $BQ.unsubscribe()", "$BQ => $BO.unsubscribe()"),
        ("$BQ => $BQ.unsubscribe()", "$BQ => $BQ.unsubscribe(0)"),
        ("$BQ => $BQ.unsubscribe()", "$BQ => $BQ.complete()"),
        (
            "$BQ => $BQ.unsubscribe()",
            "$BQ => { $BQ.unsubscribe(); sideEffect(); }",
        ),
        (
            "$KEY_3.forEach($BQ => $BQ.unsubscribe());",
            "$KEY_3.forEach($BQ => $BQ.unsubscribe(), 0);",
        ),
        (
            "$KEY_3.forEach($BQ => $BQ.unsubscribe());",
            "return $KEY_3.forEach($BQ => $BQ.unsubscribe());",
        ),
        (
            "$KEY_3.forEach($BQ => $BQ.unsubscribe());",
            "$KEY_3.forEach($BQ => $BQ.unsubscribe()); sideEffect();",
        ),
        ("return $THREAD_3_81450;", "return other;"),
        (
            "return $THREAD_3_81450;",
            "expose($THREAD_3_81450); return $THREAD_3_81450;",
        ),
        ("--$KEY_1;", "++$KEY_1;"),
        ("$KEY_1 <= 0", "$KEY_1 < 0"),
        ("$KEY_2.error($BT);", "$KEY_2.error(0);"),
        ("$KEY_3 = [];", "if (ready) { $KEY_3 = []; }"),
        ("$KEY_3 = [];", "$KEY_3 = []; $KEY_3 = other;"),
        ("$KEY_2 = $BP", "$KEY_2 = other"),
        ("new $KEY_315(", "new other("),
    ] {
        let changed = joined_observables_owner(|own| {
            assert!(own.contains(from), "{from}");
            own.replace(from, to)
        });
        check(&changed, "$THREAD_3_81450", false);
    }
}

#[test]
fn valid_sibling_cannot_donate_teardown_proof() {
    let source = joined_observables_owner(|own| {
        let donor = own
            .replace("$THREAD_7_93275", "$THREAD_9_999996")
            .replace("$THREAD_3_81450", "$THREAD_9_999997");
        format!(
            "{}\n{}",
            own.replace("$BQ => $BQ.unsubscribe()", "$BQ => $BO.unsubscribe()"),
            donor
        )
    });
    check(&source, "$THREAD_3_81450", false);
    check(&source, "$THREAD_9_999997", true);
}

#[test]
fn private_expression_self_is_not_a_returned_declaration_or_factory() {
    let teardown = named_expression(JOINED_OBSERVABLES, "$THREAD_3_81450");
    check(&teardown, "$THREAD_9_999994", false);
    let parent = named_expression(JOINED_OBSERVABLES, "$THREAD_7_93275");
    check(&parent, "$THREAD_3_81450", false);
}
