use shape::naming::name_threads;

// Whole prepared inputs retain the real bootstrap, carrier, callable aliases,
// constructor aliases and consumers needed for a valid Model. No JS executes.
const INLINE_QUEUE_SOURCE: &str = include_str!("fixtures/task_queue_inline/mean-before-guard.js");
const ASSIGNED_QUEUE_SOURCE: &str = include_str!("fixtures/task_queue_inline/mean-committed-in-guard.js");

fn recognizes(source: &str, target: &str) -> bool {
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::unambiguous()).parse();
    assert!(
        parsed.errors.is_empty(),
        "fixture must parse: {:?}",
        parsed.errors
    );
    // name_threads builds Model before returning; Err is never refusal proof.
    let result = name_threads(source, true).expect("fixture must build a valid Model and analyze");
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == target)
        .expect("the original candidate must remain present");
    outcome.applied
        && outcome.proposed_name.as_deref() == Some("drainTaskQueue")
        && outcome.evaluator.as_deref() == Some("function_finder::task_queue_drain")
}

#[test]
fn inline_mean_before_guard_from_real_factory() {
    assert!(recognizes(INLINE_QUEUE_SOURCE, "$THREAD_4_20616"));
}

#[test]
fn inline_mean_committed_in_guard_from_real_factory() {
    assert!(recognizes(ASSIGNED_QUEUE_SOURCE, "$THREAD_0_17550"));
}

#[test]
fn alpha_renaming_keeps_scoped_transport_and_order() {
    let source = INLINE_QUEUE_SOURCE
        .replace("$THREAD_4_20616", "$THREAD_941_882221")
        .replace("$KEY_14", "$QUEUE_TASKS")
        .replace("$KEY_12", "$QUEUE_CLOCK");
    assert!(recognizes(&source, "$THREAD_941_882221"));
}

// Nine refusals + three positives = twelve unique witnesses in this round.
#[test]
fn refuses_wrong_arguments_receivers_origins_order_and_aliases() {
    for (from, to) in [
        ("$KEY_3($BV);", "$KEY_3($BW);"),
        ("$KEY_14.shift()", "$KEY_13.shift()"),
        ("$KEY_14.length = 0;", "$KEY_14.length = 1;"),
        ("$BU * $BT + ($BX - $BW)", "$BU * $BT + ($BW - $BX)"),
        ("$BX + $BU < $BS + $KEY_1", "$BX + $BU <= $BS + $KEY_1"),
        (
            "$KEY_9 = $THREAD_4_20616;",
            "$KEY_9 = $THREAD_4_20616; $KEY_9 = other;",
        ),
        ("let $KEY_3 = $BM;", "let $KEY_3 = $BM; $KEY_3 = other;"),
        (
            "$KEY_14 = $BQ.slice();",
            "$KEY_14 = $BQ.slice(); escape($KEY_14);",
        ),
    ] {
        assert!(
            INLINE_QUEUE_SOURCE.contains(from),
            "mutation must touch its intended witness: {from}"
        );
        let source = INLINE_QUEUE_SOURCE.replace(from, to);
        assert!(
            !recognizes(&source, "$THREAD_4_20616"),
            "accepted mutation {from} -> {to}"
        );
    }
    let source = ASSIGNED_QUEUE_SOURCE.replace(
        "$AO = ($AO * $AP + ($AS - $AR)) / ($AP + 1);",
        "$AO = ($AO * $AP + ($AS - $AR)) / ($AP + 2);",
    );
    assert_ne!(source, ASSIGNED_QUEUE_SOURCE);
    assert!(!recognizes(&source, "$THREAD_0_17550"));
}

// Additional unit controls retain complete naming coverage; the portable example count must
// never become a whole-input naming cap.
const WORKER_FORWARD: &str =
    include_str!("fixtures/task_queue_inline/worker-forward-var-alias.UNEXECUTED.js");
const BUDGET_FORWARD: &str =
    include_str!("fixtures/task_queue_inline/budget-forward-var-alias.UNEXECUTED.js");

fn changed_once(source: &str, from: &str, to: &str) -> String {
    assert_eq!(
        source.matches(from).count(),
        1,
        "mutation must be scoped: {from}"
    );
    source.replacen(from, to, 1)
}


#[test]
fn temporal_worker_forward_var_alias_requires_refusal() {
    // Reproduce the callback-scope counterexample against the supported source.
    assert!(!recognizes(WORKER_FORWARD, "$THREAD_4_20616")); // Worker alias must refuse.
}


#[test]
fn temporal_budget_forward_var_alias_requires_refusal() {
    assert!(!recognizes(BUDGET_FORWARD, "$THREAD_4_20616")); // Budget alias must refuse.
}


#[test]
fn temporal_ordered_multihop_worker_and_budget_aliases_remain_positive() {
    let source = changed_once(INLINE_QUEUE_SOURCE, "let $KEY_3 = $BM;",
        "var reviewWorker = $BM; var reviewWorkerNext = reviewWorker; var $KEY_3 = reviewWorkerNext;");
    let source = changed_once(&source, "let $KEY_1 = $BN;\n\t\t\tlet $KEY_2;",
        "var reviewBudget = $BN; var reviewBudgetNext = reviewBudget; var $KEY_1 = reviewBudgetNext;\n\t\t\tlet $KEY_2;");
    let source = source
        .replace("$THREAD_4_20616", "$THREAD_941_882221")
        .replace("$KEY_14", "$QUEUE_TASKS")
        .replace("$KEY_12", "$QUEUE_CLOCK");
    assert!(recognizes(&source, "$THREAD_941_882221")); // Alpha-renaming preserves the claim.
}


#[test]
fn captured_list_calls_require_exact_receiver_arity_and_no_member_replacement() {
    // The onEmpty consumer must check receiver and arity rather than just the member
    // name: a borrowed push or a two-argument push must refuse.
    for to in [
        "other($KEY_13.push, $BS);",
        "$KEY_13.push($BS, other);",
        "$KEY_13.push = other;",
    ] {
        let source = changed_once(INLINE_QUEUE_SOURCE, "$KEY_13.push($BS);", to);
        assert!(!recognizes(&source, "$THREAD_4_20616"), "accepted {to}");
    }
}

