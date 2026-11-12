//! Package-finder family: the observer constructor stubs of the host-constructor
//! probe table.
//!
//! The emitted shape writes the row's constructor and callback slots after the
//! row template instead of placing the constructor inside it:
//!
//! ```js
//! let row = [true];
//! function stub() {
//! }
//! row[1] = capturedHost.IntersectionObserver;
//! row[2] = [stub];
//! table[n] = row;
//! ```
//!
//! The empty function is role-bearing only because it is the exact sole first
//! constructor argument of that same row, the row has unique agreeing slot
//! writes, the constructor is a member of the captured host the table's own
//! rows use, and one table consumer places the row in an array-template table.
//! A generic empty body, a wrong row/slot/constructor, conflicting writes, an
//! effectful callback, an unrelated or shadowed host and a nested donor all stay
//! unnamed. The referenced stub keeps its declaration and every reference.

use shape::naming::name_threads;
use std::cmp::Reverse;

const INTERSECTION_EVALUATOR: &str = "function_finder::intersection_observer_stub";
const MUTATION_EVALUATOR: &str = "function_finder::mutation_observer_stub";
const PERFORMANCE_EVALUATOR: &str = "function_finder::performance_observer_stub";

const INTERSECTION: &str = "stubIntersectionObserver";
const MUTATION: &str = "stubMutationObserver";
const PERFORMANCE: &str = "stubPerformanceObserver";

/// The probe-table shape appends three observer rows to one host-constructor table, then
/// writes each constructor into a later slot.
const THREE_ROWS: &str = r#"function main(){
let host = {IntersectionObserver:function(){},MutationObserver:function(){},PerformanceObserver:function(){},MediaStream:function(){},Path2D:function(){},ReadableStream:function(){}};
let probes = [[true, host.MediaStream, []], [true, host.Path2D, []], [true, host.ReadableStream, []]];
let intersection = [true];
function $THREAD_3_68687() {
}
intersection[1] = host.IntersectionObserver;
intersection[2] = [$THREAD_3_68687];
probes[3] = intersection;
let mutation = [true];
function $THREAD_9_59929() {
}
mutation[1] = host.MutationObserver;
mutation[2] = [$THREAD_9_59929];
probes[4] = mutation;
let performance = [true];
function $THREAD_0_90024() {
}
performance[1] = host.PerformanceObserver;
performance[2] = [$THREAD_0_90024];
probes[5] = performance;
return probes;
}
main();"#;

/// One self-contained carrier for the negative cases.
const ONE_ROW: &str = r#"function main(){
let host = {IntersectionObserver:function(){},MediaStream:function(){}};
let probes = [[true, host.MediaStream, []]];
let row = [true];
function $THREAD_3_68687() {
}
row[1] = host.IntersectionObserver;
row[2] = [$THREAD_3_68687];
probes[1] = row;
return probes;
}
main();"#;

/// The same one-carrier shape with opaque generated spellings, a captured host
/// path and a computed constructor member.
const ONE_ROW_ALPHA: &str = r#"function main(){
const $KEY_1 = {global:{IntersectionObserver:function(){},MediaStream:function(){}}};
var $KEY_2 = [[true, $KEY_1.global.MediaStream, []]];
let $AX = [true];
function $THREAD_3_68687() {
}
$AX[1] = $KEY_1.global["IntersectionObserver"];
$AX[2] = ([$THREAD_3_68687]);
$KEY_2[1] = $AX;
return $KEY_2;
}
main();"#;

/// The older literal-row form: the constructor is inside the row template and
/// only the callback slot is written later. It must stay named exactly as
/// before this family's addition.
const LITERAL_ROW: &str = r#"function main(){
let host = {IntersectionObserver:function(){},MediaStream:function(){}};
let probes = [[true, host.MediaStream, []]];
let row = [true, host.IntersectionObserver];
function $THREAD_3_68687() {
}
row[2] = [$THREAD_3_68687];
probes[1] = row;
return probes;
}
main();"#;

fn named(source: &str, expected: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result
            .report
            .outcomes
            .iter()
            .any(|outcome| outcome.applied && outcome.proposed_name.as_deref() == Some(expected))
    })
}

fn claimed(source: &str, evaluator: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result
            .report
            .function_roles
            .iter()
            .any(|role| role.evaluators.iter().any(|found| found == evaluator))
    })
}

fn accepted(source: &str, expected: &str, evaluator: &str) {
    assert!(named(source, expected), "not named `{expected}`: {source}");
    assert!(
        claimed(source, evaluator),
        "no `{evaluator}` claim: {source}"
    );
}

fn denied(source: &str) {
    assert!(
        name_threads(source, true).is_ok(),
        "fixture did not parse: {source}"
    );
    assert!(
        !named(source, INTERSECTION),
        "unexpected `{INTERSECTION}`: {source}"
    );
    assert!(
        !claimed(source, INTERSECTION_EVALUATOR),
        "unexpected observer-stub claim: {source}"
    );
}

/// Replace one exact fragment, failing loudly when the base fixture drifted.
fn mutated(source: &str, from: &str, to: &str) -> String {
    assert!(source.contains(from), "missing fragment `{from}`");
    source.replacen(from, to, 1)
}

#[test]
fn observer_stub_rows_accepts_the_three_observer_rows() {
    for (expected, evaluator) in [
        (INTERSECTION, INTERSECTION_EVALUATOR),
        (MUTATION, MUTATION_EVALUATOR),
        (PERFORMANCE, PERFORMANCE_EVALUATOR),
    ] {
        accepted(THREE_ROWS, expected, evaluator);
    }
}

#[test]
fn observer_stub_rows_accepts_one_portable_carrier() {
    accepted(ONE_ROW, INTERSECTION, INTERSECTION_EVALUATOR);
    accepted(ONE_ROW_ALPHA, INTERSECTION, INTERSECTION_EVALUATOR);
    accepted(LITERAL_ROW, INTERSECTION, INTERSECTION_EVALUATOR);
}

#[test]
fn observer_stub_rows_accepts_source_node_shapes() {
    let parenthesized = mutated(
        ONE_ROW,
        "row[2] = [$THREAD_3_68687];",
        "row[2] = ([$THREAD_3_68687]);",
    );
    let computed = mutated(
        ONE_ROW,
        "row[1] = host.IntersectionObserver;",
        "row[1] = host[\"IntersectionObserver\"];",
    );
    let declarations = ONE_ROW
        .replace("let host =", "const host =")
        .replace("let probes =", "var probes =")
        .replace("let row =", "var row =");
    // The emitted form rebuilds the table once from its own binding.
    let reassembled = mutated(
        ONE_ROW,
        "let probes = [[true, host.MediaStream, []]];",
        "let probes = [];\nprobes = [probes, [true, host.MediaStream, []]];",
    );
    for source in [parenthesized, computed, declarations, reassembled] {
        accepted(&source, INTERSECTION, INTERSECTION_EVALUATOR);
    }
}

#[test]
fn observer_stub_rows_refuses_a_different_row_slot_or_constructor() {
    // The constructor slot belongs to another row.
    let wrong_row = ONE_ROW
        .replace(
            "let row = [true];",
            "let row = [true];\nlet other = [true];",
        )
        .replace(
            "row[1] = host.IntersectionObserver;",
            "other[1] = host.IntersectionObserver;",
        );
    // The stub is not the callback slot (2) any more.
    let wrong_slot = mutated(
        ONE_ROW,
        "row[2] = [$THREAD_3_68687];",
        "row[3] = [$THREAD_3_68687];",
    );
    // The callback array carries more than the sole first argument.
    let extra_argument = mutated(
        ONE_ROW,
        "row[2] = [$THREAD_3_68687];",
        "row[2] = [$THREAD_3_68687, host.MediaStream];",
    );
    // The row names a different host member.
    let wrong_constructor = mutated(ONE_ROW, "host.IntersectionObserver", "host.MediaStream");
    for source in [wrong_row, wrong_slot, extra_argument, wrong_constructor] {
        denied(&source);
    }
}

#[test]
fn observer_stub_rows_refuses_conflicting_writes() {
    let constructor_twice = mutated(
        ONE_ROW,
        "row[1] = host.IntersectionObserver;",
        "row[1] = host.IntersectionObserver;\nrow[1] = host.IntersectionObserver;",
    );
    let callback_twice = mutated(
        ONE_ROW,
        "row[2] = [$THREAD_3_68687];",
        "row[2] = [$THREAD_3_68687];\nrow[2] = [];",
    );
    let extra_slot = mutated(
        ONE_ROW,
        "row[1] = host.IntersectionObserver;",
        "row[0] = false;\nrow[1] = host.IntersectionObserver;",
    );
    let reassigned_row = mutated(
        ONE_ROW,
        "probes[1] = row;",
        "row = [true, host.MediaStream];\nprobes[1] = row;",
    );
    for source in [
        constructor_twice,
        callback_twice,
        extra_slot,
        reassigned_row,
    ] {
        denied(&source);
    }
}

#[test]
fn observer_stub_rows_refuses_a_different_or_shadowed_host() {
    // The row's constructor comes from a different captured host object.
    let other_host = ONE_ROW
        .replace(
            "let host = {",
            "let other = {IntersectionObserver:function(){},MediaStream:function(){}};\nlet host = {",
        )
        .replace(
            "row[1] = host.IntersectionObserver;",
            "row[1] = other.IntersectionObserver;",
        );
    // A block-local host shadows the captured host used by the table rows.
    let shadowed = mutated(
        ONE_ROW,
        "row[1] = host.IntersectionObserver;\nrow[2] = [$THREAD_3_68687];\nprobes[1] = row;",
        "{\nlet host = {IntersectionObserver:function(){},MediaStream:function(){}};\nrow[1] = host.IntersectionObserver;\nrow[2] = [$THREAD_3_68687];\nprobes[1] = row;\n}",
    );
    // The table itself has no row naming that captured host.
    let unrelated_table = ONE_ROW
        .replace(
            "let probes = [[true, host.MediaStream, []]];",
            "let probes = [[]];",
        )
        .replace("probes[1] = row;", "let fake = [];\nfake[1] = row;");
    for source in [other_host, shadowed, unrelated_table] {
        denied(&source);
    }
}

#[test]
fn observer_stub_rows_refuses_nested_donors_and_generic_empties() {
    // The row is assembled in a nested function, not where the stub is declared.
    let nested_donor = mutated(
        ONE_ROW,
        "row[1] = host.IntersectionObserver;\nrow[2] = [$THREAD_3_68687];\nprobes[1] = row;",
        "function assemble() {\nrow[1] = host.IntersectionObserver;\nrow[2] = [$THREAD_3_68687];\nprobes[1] = row;\n}\nassemble();",
    );
    // The stub is declared but no longer referenced by any row.
    let unreferenced = mutated(ONE_ROW, "row[2] = [$THREAD_3_68687];", "row[2] = [];");
    // An empty function used as a generic value is not a constructor stub.
    let generic_empty = r#"function main(){
let host = {IntersectionObserver:function(){},MediaStream:function(){}};
let values = [];
function $THREAD_3_68687() {
}
values.push($THREAD_3_68687);
return values;
}
main();"#;
    // A reference outside the observer row breaks the unique agreeing writes.
    let extra_reference = mutated(
        ONE_ROW,
        "probes[1] = row;",
        "probes[1] = row;\nprobes[2] = $THREAD_3_68687;",
    );
    // The callback is effectful, so it is not an empty constructor stub.
    let effectful = mutated(
        ONE_ROW,
        "function $THREAD_3_68687() {\n}",
        "function $THREAD_3_68687() {\nhost.MediaStream;\n}",
    );
    for source in [
        nested_donor,
        unreferenced,
        generic_empty.to_string(),
        extra_reference,
        effectful,
    ] {
        denied(&source);
    }
}

#[test]
fn observer_stub_rows_preserves_human_names_and_collisions() {
    let human = THREE_ROWS.replace("$THREAD_3_68687", INTERSECTION);
    let result = name_threads(&human, true).expect("human fixture parses");
    assert!(result.source.contains(&format!("function {INTERSECTION}(")));
    assert!(result.source.contains(&format!("[{INTERSECTION}]")));
    assert!(!result.report.outcomes.iter().any(|outcome| {
        outcome.binding.name == INTERSECTION
            && outcome.applied
            && outcome.proposed_name.as_deref() == Some(INTERSECTION)
    }));

    let reserved = THREE_ROWS.replace(
        "function main(){",
        &format!("function main(){{let {INTERSECTION} = 17;useValue({INTERSECTION});"),
    );
    assert!(name_threads(&reserved, true).is_ok());
    assert!(!named(&reserved, INTERSECTION));
}

#[test]
fn observer_stub_rows_keeps_referenced_functions_live() {
    let result = name_threads(THREE_ROWS, true).expect("fixture parses");
    for name in [INTERSECTION, MUTATION, PERFORMANCE] {
        assert!(
            result.source.contains(&format!("function {name}(")),
            "declaration for `{name}` was removed: {}",
            result.source
        );
        assert!(
            result.source.contains(&format!("[{name}]")),
            "reference to `{name}` was not rewritten: {}",
            result.source
        );
        assert_eq!(
            result.source.matches(name).count(),
            2,
            "`{name}` must keep exactly its declaration and one reference"
        );
    }
}

#[test]
fn observer_stub_rows_edits_replay_in_sorted_byte_order() {
    let result = name_threads(THREE_ROWS, true).expect("fixture parses");
    let applied: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|outcome| {
            outcome.applied
                && outcome
                    .proposed_name
                    .as_deref()
                    .is_some_and(|name| [INTERSECTION, MUTATION, PERFORMANCE].contains(&name))
        })
        .collect();
    assert_eq!(applied.len(), 3, "expected one applied target per row");
    let mut edits = result.report.edits.clone();
    edits.sort_by_key(|edit| Reverse(edit.start));
    let mut replay = THREE_ROWS.to_string();
    for edit in edits {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
}
