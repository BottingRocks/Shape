//! Focused, portable coverage for the loop/tail duplicate registration spelling
//! of the `keyup` hold-statistics recorder:
//!
//! ```js
//! let listener = recordKeyupHoldStatistics;
//! queue(() => {
//!   host.addEventListener("keyup", listener);
//!   if (flag) {
//!     host.addEventListener("keyup", listener);
//!   }
//! });
//! ```
//!
//! The exact keyup recorder body appears in both loop and tail copies. They
//! register the same recorder through one listener alias on the same host in
//! one queue task. The repeated-recorder fallback applies only when every
//! registration agrees on recorder, host, alias, and task, without capture.
//! Single registration retains its claim. Host, listener, body, capture,
//! alias-write, event, ownership, and alias mutations must refuse.
//!
//! The portable sources are wrapped in a minimal carrier; the full cached seeds
//! are not read by this target.
use shape::naming::name_threads;

const HOLD: &str = "recordKeyupHoldStatistics";
const EVALUATOR: &str = "function_finder::interaction_keyup_hold_statistics_recorder";

/// Exact keyup recorder body for repeated listener-alias registration.
const REPEATED_KEYUP: &str = r#"function $THREAD_1_28867($BV) {
						let $KEY_4 = Date.now();
						let $BW = $KEY_12[$BV.keyCode];
						++$KEY_37;
						if ($BW) {
							let $BX = $KEY_20;
							$KEY_20 = $KEY_20 + ($KEY_4 - $BW - $BX) / $KEY_37;
							$KEY_3 = $KEY_3 + ($KEY_4 - $BW - $BX) * ($KEY_4 - $BW - $KEY_20);
						}
						$KEY_24 = $KEY_4;
						let $BY = $KEY_22.indexOf($BV.keyCode);
						if ($BY !== -1) {
							++$KEY_26;
							$KEY_31[$BY] = Number($KEY_31[$BY]) + 1;
						}
						$KEY_12[$BV.keyCode] = null;
						$KEY_13 = true;
					}"#;

/// Exact keyup recorder body for single listener-alias registration.
const SINGLE_KEYUP_DONOR: &str = r#"function $THREAD_0_23152($BV) {
				let $KEY_3 = Date.now();
				let $BW = $KEY_10[$BV.keyCode];
				++$KEY_43;
				if ($BW) {
					let $BX = $KEY_36;
					$KEY_36 = $KEY_36 + ($KEY_3 - $BW - $BX) / $KEY_43;
					$KEY_48 = $KEY_48 + ($KEY_3 - $BW - $BX) * ($KEY_3 - $BW - $KEY_36);
				}
				$KEY_47 = $KEY_3;
				let $BY = $KEY_22.indexOf($BV.keyCode);
				if ($BY !== -1) {
					++$KEY_11;
					$KEY_16[$BY] = Number($KEY_16[$BY]) + 1;
				}
				$KEY_10[$BV.keyCode] = null;
				$KEY_4 = true;
			}"#;

/// The captured bindings the recorder reads, the two event hosts, the queue
/// helper and a foreign `keyCode` recorder used by the listener near miss.
const DEPS: &str = "let $KEY_3=0;let $KEY_4=0;let $KEY_10={};let $KEY_11=0;let $KEY_12={};let $KEY_13=false;let $KEY_16=[];let $KEY_20=0;let $KEY_22=[];let $KEY_24=0;let $KEY_26=0;let $KEY_31=[];let $KEY_36=0;let $KEY_37=0;let $KEY_43=0;let $KEY_47=0;let $KEY_48=0;let $KEY_14={addEventListener(){},removeEventListener(){}};let $KEY_15={addEventListener(){},removeEventListener(){}};let $BS=(task)=>task();";

/// The foreign keyCode recorder used by the wrong-listener near miss.
const FOREIGN: &str = "function $THREAD_9_99999($BV){$KEY_12[$BV.keyCode]=null;}\n";

/// The emitted loop/tail duplicate: both registrations share one captured host,
/// one single-origin listener alias and one queue task.
const DOUBLE_TASK: &str = "let $KEY_2=Math.random()*4|0;if($KEY_2>0){$KEY_14.addEventListener(\"keyup\",$KEY_1);}$KEY_14.addEventListener(\"keyup\",$KEY_1);";

/// One plain registration of the same alias on the same host.
const SINGLE_TASK: &str = "$KEY_14.addEventListener(\"keyup\",$KEY_1);";

/// One direct registration of the recorder itself, with no listener alias.
const DIRECT_TASK: &str = "$KEY_14.addEventListener(\"keyup\",$THREAD_1_28867);";

fn function_name(body: &str) -> &str {
    body.split_once('(')
        .map(|(prefix, _)| prefix.split_whitespace().last().unwrap_or(""))
        .expect("function body")
}

fn carrier(body: &str, extra: &str, task: &str) -> String {
    let mut source = String::from("(function main(){");
    source.push_str(DEPS);
    source.push_str(extra);
    source.push_str(body);
    source.push('\n');
    source.push_str("let $KEY_1=");
    source.push_str(function_name(body));
    source.push_str(";\n$BS(()=>{");
    source.push_str(task);
    source.push_str("});\n})();");
    source
}

fn program(body: &str, task: &str) -> String {
    carrier(body, "", task)
}

fn claims(source: &str, name: &str, evaluator: &str) -> usize {
    name_threads(source, true)
        .unwrap()
        .report
        .function_roles
        .iter()
        .filter(|role| {
            role.candidates.iter().any(|candidate| candidate == name)
                && role.evaluators.iter().any(|found| found == evaluator)
        })
        .count()
}

fn applied(source: &str, name: &str, evaluator: &str) -> usize {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .filter(|outcome| {
            outcome.applied
                && outcome.proposed_name.as_deref() == Some(name)
                && outcome.evaluator.as_deref() == Some(evaluator)
        })
        .count()
}

fn refused(source: &str) -> bool {
    claims(source, HOLD, EVALUATOR) == 0 && applied(source, HOLD, EVALUATOR) == 0
}

#[test]
fn witness_fixtures_are_the_exact_source_bodies() {
    assert_eq!(REPEATED_KEYUP.len(), 533);
    assert_eq!(SINGLE_KEYUP_DONOR.len(), 500);
    assert!(REPEATED_KEYUP.starts_with("function $THREAD_1_28867($BV) {"));
    assert!(REPEATED_KEYUP.contains("$KEY_12[$BV.keyCode]"));
    assert!(SINGLE_KEYUP_DONOR.starts_with("function $THREAD_0_23152($BV) {"));
    assert!(SINGLE_KEYUP_DONOR.contains("$KEY_10[$BV.keyCode]"));
}

#[test]
fn exact_repeated_keyup_alias_registration_is_claimed() {
    let source = program(REPEATED_KEYUP, DOUBLE_TASK);
    assert_eq!(claims(&source, HOLD, EVALUATOR), 1, "{source}");
    assert_eq!(applied(&source, HOLD, EVALUATOR), 1, "{source}");
    let renamed = name_threads(&source, true).unwrap().source;
    assert!(
        renamed.contains("function recordKeyupHoldStatistics("),
        "{renamed}"
    );
    assert!(!renamed.contains("function $THREAD_1_28867("), "{renamed}");
}

#[test]
fn alpha_renamed_duplicate_alias_is_claimed() {
    let body = REPEATED_KEYUP
        .replace("$THREAD_1_28867", "$THREAD_7_424242")
        .replace("$BV", "$CF");
    assert!(body.starts_with("function $THREAD_7_424242($CF) {"), "{body}");
    let source = program(&body, DOUBLE_TASK);
    assert_eq!(claims(&source, HOLD, EVALUATOR), 1, "{source}");
    assert_eq!(applied(&source, HOLD, EVALUATOR), 1, "{source}");
}

#[test]
fn applied_single_keyup_donor_single_alias_registration_is_claimed() {
    let source = program(SINGLE_KEYUP_DONOR, SINGLE_TASK);
    assert_eq!(claims(&source, HOLD, EVALUATOR), 1, "{source}");
    assert_eq!(applied(&source, HOLD, EVALUATOR), 1, "{source}");
}

#[test]
fn direct_single_registration_base_is_preserved() {
    let source = program(REPEATED_KEYUP, DIRECT_TASK);
    assert_eq!(claims(&source, HOLD, EVALUATOR), 1, "{source}");
    assert_eq!(applied(&source, HOLD, EVALUATOR), 1, "{source}");
}

#[test]
fn duplicate_near_misses_are_refused() {
    let cases: [(&str, &str, &str); 6] = [
        // foreign host: the second copy registers on another captured host
        (
            "",
            "$KEY_14.addEventListener(\"keyup\",$KEY_1);$KEY_15.addEventListener(\"keyup\",$KEY_1);",
            "foreign host",
        ),
        // foreign listener: the second copy registers a different keyCode body
        (
            FOREIGN,
            "$KEY_14.addEventListener(\"keyup\",$KEY_1);$KEY_14.addEventListener(\"keyup\",$THREAD_9_99999);",
            "foreign listener",
        ),
        // capture option: the second copy adds the capture flag
        (
            "",
            "$KEY_14.addEventListener(\"keyup\",$KEY_1);$KEY_14.addEventListener(\"keyup\",$KEY_1,true);",
            "capture option",
        ),
        // dead registration: the second copy is owned by an uncalled donor
        (
            "",
            "$KEY_14.addEventListener(\"keyup\",$KEY_1);function donor(){$KEY_14.addEventListener(\"keyup\",$KEY_1);}",
            "nested donor",
        ),
        // changed event: neither copy is a keyup registration
        (
            "",
            "$KEY_14.addEventListener(\"pointerup\",$KEY_1);$KEY_14.addEventListener(\"pointerup\",$KEY_1);",
            "changed event",
        ),
        // missing alias: the recorder itself is registered directly twice
        (
            "",
            "$KEY_14.addEventListener(\"keyup\",$THREAD_1_28867);$KEY_14.addEventListener(\"keyup\",$THREAD_1_28867);",
            "missing alias",
        ),
    ];
    for (extra, task, label) in cases {
        let source = carrier(REPEATED_KEYUP, extra, task);
        assert!(refused(&source), "accepted {label}:\n{source}");
    }
}

#[test]
fn alias_overwrite_is_refused() {
    let source = program(REPEATED_KEYUP, DOUBLE_TASK).replace(
        "let $KEY_1=$THREAD_1_28867;",
        "let $KEY_1=$THREAD_1_28867;function $THREAD_9_99999($BV){$KEY_12[$BV.keyCode]=null;}$KEY_1=$THREAD_9_99999;",
    );
    assert!(source.contains("$KEY_1=$THREAD_9_99999;"), "{source}");
    assert!(refused(&source), "{source}");
}

#[test]
fn foreign_body_is_refused() {
    let body = REPEATED_KEYUP.replace(".keyCode", ".key");
    assert!(!body.contains(".keyCode"), "{body}");
    let source = program(&body, DOUBLE_TASK);
    assert!(refused(&source), "{source}");
}
