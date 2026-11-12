//! Integration checks run the full namer only on the two original witnesses.
//! Mutation/capture cases live in the private own-evaluator unit module.
use shape::naming::name_threads;
const FOUR_CLEANUP_DECLARATIONS: &str = include_str!("fixtures/naming/captured_handle_termination/stateful-parent.js");
const CANONICAL_SOURCE: &str = include_str!("fixtures/naming/captured_handle_termination/canonical-parent.js");
#[test]
fn whole_four_cleanup_declarations_namer_reaches_all_four_exact_cleanup_declarations() {
    let r = name_threads(FOUR_CLEANUP_DECLARATIONS, true).unwrap();
    for start in [368218, 789487, 1200102, 1521869] {
        let o = r
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.declaration.start == start)
            .unwrap();
        assert!(
            o.applied
                && o.role_name.as_deref() == Some("terminateCapturedHandleIfPresentThenComplete")
                && o.evaluator.as_deref() == Some("guarded_handle_completion"),
            "{o:?}"
        );
    }
}
#[test]
fn canonical_cleanup_keeps_priority() {
    let r = name_threads(CANONICAL_SOURCE, true).unwrap();
    let o = r
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.declaration.start == 397936)
        .unwrap();
    assert!(o.applied, "{o:?}");
    assert_ne!(o.evaluator.as_deref(), Some("guarded_handle_completion"));
}
