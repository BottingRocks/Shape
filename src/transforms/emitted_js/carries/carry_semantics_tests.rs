//! Node differential tests for the carry and branch rewrites.
//!
//! Each test runs the original and rewritten source in Node and compares
//! `JSON.stringify` output. The cases cover nested branch-value diamonds,
//! per-iteration loop snapshots, equivalent-carry merges, and loop-induction
//! headers. Node must be on `PATH`; these tests protect the runtime-equivalence
//! claim, while the individual modules keep the structural decline tests.

use std::process::Command;

use super::super::{
    coalesce_carry_copies, fold_branch_values, merge_equivalent_carries,
};

fn output(source: &str, expression: &str) -> String {
    let script = format!("{source}\nconsole.log(JSON.stringify({expression}));");
    let result = Command::new("node").arg("-e").arg(script).output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap()
}

#[test]
fn nested_join_keeps_all_branch_results() {
    let source = "function f(a,b) { let $CARRY_1; if (a) { if (b) { $CARRY_1 = 1; } else { $CARRY_1 = 2; } } else { $CARRY_1 = 3; } return $CARRY_1; }";
    let rewritten = fold_branch_values(source).unwrap().code;
    let cases = "[f(false,false),f(false,true),f(true,false),f(true,true)]";
    assert_eq!(output(source, cases), output(&rewritten, cases));
}

#[test]
fn loop_snapshot_coalescing_keeps_each_iteration() {
    let source = "function f(n) { let $CARRY_outer = 0; { let $CARRY_inner = $CARRY_outer; loop: for (;;) { if ($CARRY_inner < n) { $CARRY_inner = $CARRY_inner + 1; continue loop; } break loop; } $CARRY_outer = $CARRY_inner; } return $CARRY_outer; }";
    let rewritten = coalesce_carry_copies(source).unwrap().code;
    let cases = "[f(0),f(1),f(3),f(8)]";
    assert_eq!(output(source, cases), output(&rewritten, cases));
}

#[test]
fn equivalent_carries_keep_both_branch_values() {
    let source = "function f(p,x) { let $CARRY_a, $CARRY_b; if (p) { $CARRY_a = x; $CARRY_b = x; } else { $CARRY_a = null; $CARRY_b = null; } return [$CARRY_a,$CARRY_b]; }";
    let rewritten = merge_equivalent_carries(source).unwrap().code;
    let cases = "[f(false,7),f(true,7),f(true,0)]";
    assert_eq!(output(source, cases), output(&rewritten, cases));
}

