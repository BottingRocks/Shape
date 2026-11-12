//! Focused, portable coverage for the runtime's logging-only global-null
//! lifecycle pair (`noopTeardown`).
//!
//! The representative carrier is a self-contained runtime factory: when the
//! captured global is absent it builds `let pair = { init: loggingInit }`,
//! declares an empty stub, assigns `pair.teardown = stub` after the literal and
//! returns that same pair. Some builds lower the bootstrap in a way that never
//! reaches this package's shell/handshake, so the branch itself is
//! reconstructed from the pair the branch returns. The alpha variant respells
//! every binding, mirrors the null test and uses the computed member spelling.
//! Negatives change the holder, the returned binding, the initializer, the
//! teardown body or the write set; a decoy no-op pair with an empty init stays
//! unclaimed because it belongs to the fetch/XHR no-op families.
use shape::naming::semantic_names::finder_audit;
use shape::naming::name_threads;

const EVALUATOR: &str = "function_finder::no_global_teardown";
const NAME: &str = "noopTeardown";

const WITNESS: &str = r#"function host(){
function buildRuntime(capturedGlobal){
if (capturedGlobal == null){
let pair = { init: loggingInit };
function $THREAD_6_80664(){
}
pair.teardown = $THREAD_6_80664;
return pair;
} else {
return { init: startRuntime, teardown: stopRuntime };
}
}
function loggingInit(){
console.error("no global");
}
function startRuntime(){
console.log("start");
}
function stopRuntime(){
console.log("stop");
}
let runtime = buildRuntime(null);
runtime.init();
}
host();
"#;

// Alpha/spelling variant: every binding is respelled, the null test is
// mirrored (`null === captured`), the teardown member is written with the
// computed spelling, and the declarations are reordered. Only structure may
// decide the family.
const ALPHA: &str = r#"function host(){
function $BUILD($CAPTURED){
if (null === $CAPTURED){
let $PAIR = { init: $LOG };
function $THREAD_9_424242(){
}
$PAIR["teardown"] = $THREAD_9_424242;
return $PAIR;
} else {
return { init: $START, teardown: $STOP };
}
}
function $LOG(){
console.error("no global");
}
function $START(){
console.log("start");
}
function $STOP(){
console.log("stop");
}
let $RUN = $BUILD(null);
$RUN.init();
}
host();
"#;

// A pair returned directly from a non-null branch, with the same empty
// teardown, is not this family: the returned binding and the post-literal
// write must agree.
const GENERIC_RETURN: &str = r#"function host(){
function factory(capturedGlobal){
if (capturedGlobal == null){
return { init: loggingInit, teardown: $THREAD_6_80664 };
} else {
return { init: startRuntime, teardown: stopRuntime };
}
function $THREAD_6_80664(){
}
}
function loggingInit(){
console.error("no global");
}
function startRuntime(){
console.log("start");
}
function stopRuntime(){
console.log("stop");
}
let runtime = factory(null);
runtime.init();
}
host();
"#;

// The fetch/XHR no-op pairs are empty init plus empty teardown. They must stay
// with their own packages even though the returned-teardown shape is close.
const EMPTY_INIT_DECOY: &str = r#"function host(){
function factory(flag){
if (flag == null){
let pair = { init: emptyInit };
function $THREAD_6_80664(){
}
pair.teardown = $THREAD_6_80664;
return pair;
} else {
return { init: realInit, teardown: realTeardown };
}
}
function emptyInit(){
}
function realInit(){
}
function realTeardown(){
console.log("stop");
}
let runtime = factory(null);
runtime.init();
}
host();
"#;

// The empty stub lives inside a nested donor, so the branch does not directly
// own the teardown write of the pair it returns.
const NESTED_DONOR: &str = r#"function host(){
function buildRuntime(capturedGlobal){
if (capturedGlobal == null){
let pair = { init: loggingInit };
function outer(){
function $THREAD_6_80664(){
}
pair.teardown = $THREAD_6_80664;
}
outer();
return pair;
} else {
return { init: startRuntime, teardown: stopRuntime };
}
}
function loggingInit(){
console.error("no global");
}
function startRuntime(){
console.log("start");
}
function stopRuntime(){
console.log("stop");
}
let runtime = buildRuntime(null);
runtime.init();
}
host();
"#;

// Two independent runtime factories cannot be told apart, so neither is
// claimed.
const TWO_RUNTIMES: &str = r#"function host(){
function buildOne(capturedGlobal){
if (capturedGlobal == null){
let pair = { init: loggingInit };
function $THREAD_6_80664(){
}
pair.teardown = $THREAD_6_80664;
return pair;
} else {
return { init: startRuntime, teardown: stopRuntime };
}
}
function buildTwo(capturedGlobal){
if (capturedGlobal == null){
let pair = { init: loggingInit };
function $THREAD_7_777777(){
}
pair.teardown = $THREAD_7_777777;
return pair;
} else {
return { init: startRuntime, teardown: stopRuntime };
}
}
function loggingInit(){
console.error("no global");
}
function startRuntime(){
console.log("start");
}
function stopRuntime(){
console.log("stop");
}
let first = buildOne(null);
let second = buildTwo(null);
first.init();
second.init();
}
host();
"#;

fn witness() -> String {
    WITNESS.to_string()
}

fn alpha() -> String {
    ALPHA.to_string()
}

fn claims(source: &str) -> usize {
    name_threads(source, true)
        .unwrap()
        .report
        .function_roles
        .iter()
        .filter(|role| {
            role.candidates.iter().any(|candidate| candidate == NAME)
                && role.evaluators.iter().any(|evaluator| evaluator == EVALUATOR)
        })
        .count()
}

fn applied(source: &str) -> usize {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .filter(|outcome| {
            outcome.applied
                && outcome.proposed_name.as_deref() == Some(NAME)
                && outcome.evaluator.as_deref() == Some(EVALUATOR)
        })
        .count()
}

#[test]
fn global_null_pair_teardown_is_claimed_and_renamed() {
    let source = witness();
    assert_eq!(claims(&source), 1, "{source}");
    assert_eq!(applied(&source), 1, "{source}");
    let renamed = name_threads(&source, true).unwrap();
    assert!(renamed.source.contains("function noopTeardown("), "{}", renamed.source);
    assert!(!renamed.source.contains("$THREAD_6_80664"), "{}", renamed.source);
    // The referenced stub is renamed in place, never removed.
    assert!(renamed.source.contains(".teardown = noopTeardown;"), "{}", renamed.source);
    assert!(renamed
        .report
        .removals
        .iter()
        .all(|removal| removal.name != NAME || !removal.applied));
}

#[test]
fn alpha_postliteral_spelling_is_claimed() {
    let source = alpha();
    assert_eq!(claims(&source), 1, "{source}");
    assert_eq!(applied(&source), 1, "{source}");
    let renamed = name_threads(&source, true).unwrap();
    assert!(renamed.source.contains("function noopTeardown("), "{}", renamed.source);
    assert!(!renamed.source.contains("$THREAD_9_424242"), "{}", renamed.source);
    assert!(renamed.source.contains("[\"teardown\"] = noopTeardown;"), "{}", renamed.source);
}

#[test]
fn wrong_holder_return_and_initializer_are_refused() {
    let wrong_holder = WITNESS
        .replace(
            "let pair = { init: loggingInit };",
            "let pair = { init: loggingInit };\nlet other = { init: loggingInit };",
        )
        .replace("pair.teardown = $THREAD_6_80664;", "other.teardown = $THREAD_6_80664;");
    assert_eq!(claims(&wrong_holder), 0, "{wrong_holder}");

    let wrong_return = WITNESS
        .replace(
            "let pair = { init: loggingInit };",
            "let pair = { init: loggingInit };\nlet other = { init: loggingInit };",
        )
        .replace("return pair;", "return other;");
    assert_eq!(claims(&wrong_return), 0, "{wrong_return}");

    let no_init = WITNESS.replace("let pair = { init: loggingInit };", "let pair = {};");
    assert_eq!(claims(&no_init), 0, "{no_init}");

    let wrong_log = WITNESS.replace("console.error(\"no global\")", "console.warn(\"no global\")");
    assert_eq!(claims(&wrong_log), 0, "{wrong_log}");

    let bound_log = WITNESS.replace("console.error(\"no global\")", "host.console.error(\"no global\")");
    assert_eq!(claims(&bound_log), 0, "{bound_log}");
}

#[test]
fn effectful_conflicting_and_nested_teardowns_are_refused() {
    let effectful = WITNESS.replace(
        "function $THREAD_6_80664(){\n}",
        "function $THREAD_6_80664(){\ncleanup();\n}",
    );
    assert_eq!(claims(&effectful), 0, "{effectful}");

    let conflicting = WITNESS.replace(
        "pair.teardown = $THREAD_6_80664;",
        "pair.teardown = $THREAD_6_80664;\nfunction $THREAD_9_999999(){\n}\npair.teardown = $THREAD_9_999999;",
    );
    assert_eq!(claims(&conflicting), 0, "{conflicting}");

    assert_eq!(claims(NESTED_DONOR), 0, "{NESTED_DONOR}");
}

#[test]
fn non_runtime_and_noop_families_are_refused() {
    let no_started_pair = WITNESS.replace(
        "return { init: startRuntime, teardown: stopRuntime };",
        "return null;",
    );
    assert_eq!(claims(&no_started_pair), 0, "{no_started_pair}");

    assert_eq!(claims(GENERIC_RETURN), 0, "{GENERIC_RETURN}");
    assert_eq!(claims(EMPTY_INIT_DECOY), 0, "{EMPTY_INIT_DECOY}");
    assert_eq!(claims(TWO_RUNTIMES), 0, "{TWO_RUNTIMES}");
}

#[test]
fn edits_replay_in_byte_order() {
    let source = witness();
    let result = name_threads(&source, true).unwrap();
    let mut replay = source.clone();
    let mut edits = result.report.edits.clone();
    edits.sort_by_key(|edit| (edit.start, edit.end));
    for edit in edits.iter().rev() {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
}

// ===========================================================================
// Listener-style entry chain.
// ===========================================================================

const ENTRY_EVALUATOR: &str = "late_entry_chain::";

// The program wrapper only calls its registrar. The registrar installs a receiver for one
// literal event-name list; the receiver removes those listeners and starts the nested MAIN
// factory from the dispatched event detail.
const ENTRY: &str = r#"function $THREAD_0_0() {
	function $THREAD_5_47809() {
		let $KEY_1 = ["qMvzRDrkm"];
		function $THREAD_8_19176($A) {
			"use strict";
			let i = 0;
			for (; i < $KEY_1.length; i++) {
				removeEventListener($KEY_1[i], $THREAD_8_19176);
			}
			function $THREAD_5_2811() {
				let marker = 0;
				return { init: $THREAD_1_28594 };
			}
			function $THREAD_1_28594() {
				return 1;
			}
			$THREAD_5_2811(Object(this)).init.apply(Object(this), $A.detail);
		}
		let i = 0;
		for (; i < $KEY_1.length; i++) {
			addEventListener($KEY_1[i], $THREAD_8_19176);
		}
	}
	$THREAD_5_47809();
}
$THREAD_0_0();"#;

// The older publisher shell stays exactly as it was: root early, MAIN late.
const PUBLISHER: &str = r#"function $THREAD_0_0() {
	function $THREAD_5_2811() {
		return { init: $THREAD_1_28594 };
	}
	function $THREAD_1_28594() {
		return 1;
	}
	let $KEY_1 = new CustomEvent("ready");
	let $KEY_2 = $THREAD_5_2811();
	$KEY_1.initCustomEvent("ready", false, false, $KEY_2);
}
function keepMe() {
	return 1;
}
$THREAD_0_0();"#;

fn late_entry_names(source: &str) -> Vec<String> {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .filter(|outcome| {
            outcome.applied
                && outcome
                    .evaluator
                    .as_deref()
                    .is_some_and(|evaluator| evaluator.starts_with(ENTRY_EVALUATOR))
        })
        .filter_map(|outcome| outcome.proposed_name.clone())
        .collect()
}

fn p01_roles(source: &str) -> Vec<String> {
    finder_audit(source)
        .unwrap()
        .into_iter()
        .filter(|entry| entry.package == "p01_entry_runtime")
        .map(|entry| entry.role)
        .collect()
}

#[test]
fn listener_entry_chain_names_the_four_root_functions() {
    let names = late_entry_names(ENTRY);
    for expected in [
        "bootstrapProgram",
        "installProgramInitListeners",
        "initializeProgramFromEvent",
        "createProgramRuntime",
    ] {
        assert!(names.iter().any(|name| name == expected), "{names:?}");
    }
    assert_eq!(names.len(), 4, "{names:?}");
    let renamed = name_threads(ENTRY, true).unwrap();
    for expected in [
        "function bootstrapProgram(",
        "function installProgramInitListeners(",
        "function initializeProgramFromEvent(",
        "function createProgramRuntime(",
    ] {
        assert!(renamed.source.contains(expected), "{}", renamed.source);
    }
    assert!(!renamed.source.contains("$THREAD_0_0"));
    assert!(!renamed.source.contains("$THREAD_5_2811"));
}

#[test]
fn listener_entry_chain_is_finder_early_but_main_is_late() {
    let early = p01_roles(ENTRY);
    assert!(early.iter().any(|role| role == "bootstrap_shell"));
    assert!(early.iter().any(|role| role == "install_program_init_listeners"));
    assert!(early.iter().any(|role| role == "initialize_program_from_event"));
    // MAIN is deliberately only a late claim so a built-in main role wins.
    assert!(!early.iter().any(|role| role == "create_program_runtime"));
    assert!(late_entry_names(ENTRY)
        .iter()
        .any(|name| name == "createProgramRuntime"));
}

#[test]
fn publisher_shell_keeps_old_name_and_gets_late_main() {
    let early = p01_roles(PUBLISHER);
    assert!(early.iter().any(|role| role == "bootstrap_shell"));
    assert!(!early.iter().any(|role| role == "create_program_runtime"));
    let renamed = name_threads(PUBLISHER, true).unwrap();
    assert!(
        renamed.source.contains("function bootstrapProgram("),
        "{}",
        renamed.source
    );
    assert!(
        renamed.source.contains("function createProgramRuntime("),
        "{}",
        renamed.source
    );
    assert!(
        renamed.source.contains("function keepMe("),
        "{}",
        renamed.source
    );
    // The old publisher claim still wins the root binding unchanged.
    assert!(renamed.report.outcomes.iter().any(|outcome| {
        outcome.applied
            && outcome.proposed_name.as_deref() == Some("bootstrapProgram")
            && outcome.evaluator.as_deref() == Some("function_finder::bootstrap_shell")
    }));
}

#[test]
fn listener_entry_chain_refuses_changed_callbacks_names_and_flags() {
    let wrong_callback = ENTRY
        .replace(
            "function $THREAD_8_19176($A) {",
            "function $THREAD_9_404($A) {\n\t\t\t\treturn 0;\n\t\t\t}\n\t\t\tfunction $THREAD_8_19176($A) {",
        )
        .replace(
            "addEventListener($KEY_1[i], $THREAD_8_19176);",
            "addEventListener($KEY_1[i], $THREAD_9_404);",
        );
    assert!(late_entry_names(&wrong_callback).is_empty(), "{wrong_callback}");

    let wrong_names = ENTRY.replace(
        "removeEventListener($KEY_1[i], $THREAD_8_19176);",
        "removeEventListener($KEY_2[i], $THREAD_8_19176);",
    );
    assert!(late_entry_names(&wrong_names).is_empty(), "{wrong_names}");

    let capture_flag = ENTRY.replace(
        "addEventListener($KEY_1[i], $THREAD_8_19176);",
        "addEventListener($KEY_1[i], $THREAD_8_19176, true);",
    );
    assert!(late_entry_names(&capture_flag).is_empty(), "{capture_flag}");
}

#[test]
fn listener_entry_chain_refuses_shadowed_object_mismatched_factory_and_detail() {
    let shadowed = ENTRY.replace(
        "\"use strict\";\n\t\t\tlet i = 0;",
        "\"use strict\";\n\t\t\tlet Object = function(value) {\n\t\t\t\treturn value;\n\t\t\t};\n\t\t\tlet i = 0;",
    );
    assert!(late_entry_names(&shadowed).is_empty(), "{shadowed}");

    let mismatched = ENTRY.replace(
        "return { init: $THREAD_1_28594 };",
        "return { teardown: $THREAD_1_28594 };",
    );
    assert!(late_entry_names(&mismatched).is_empty(), "{mismatched}");

    let wrong_detail = ENTRY.replace("$A.detail", "$A.clientX");
    assert!(late_entry_names(&wrong_detail).is_empty(), "{wrong_detail}");
}

#[test]
fn listener_entry_chain_refuses_nested_donor_and_two_entries() {
    // These invalid entry graphs can be rejected before the late finder runs.
    // Only the established main-scope ambiguity is an accepted early refusal;
    // malformed source and unrelated errors must still fail this test.
    let refuses = |source: &str| match name_threads(source, true) {
        Ok(result) => assert!(!result.report.outcomes.iter().any(|outcome| {
            outcome.applied && outcome.evaluator.as_deref()
                .is_some_and(|evaluator| evaluator.starts_with(ENTRY_EVALUATOR))
        }), "{source}"),
        Err(error) => assert!(error.starts_with("program-properties ambiguity for main program scope"), "{error}"),
    };
    let donor = ENTRY.replace(
        "$THREAD_5_2811(Object(this)).init.apply(Object(this), $A.detail);",
        "function $THREAD_9_505() {\n\t\t\t\treturn { init: $THREAD_1_28594 };\n\t\t\t}",
    );
    refuses(&donor);

    let two = format!("{ENTRY}\n{}", ENTRY.replace("$THREAD_", "second_"));
    refuses(&two);
}
