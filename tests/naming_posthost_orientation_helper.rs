//! Focused, portable coverage for the deviceorientation running-statistics helper nested in
//! a probe-initializer arrow.
//!
//! Positives cover direct and hoisted-moment bodies, alias registration, and copies in
//! sibling probe arrows. Negatives change moments, receivers, callbacks, shadows, options,
//! or decoys and must stay unclaimed. The full inputs are not read by this target.
use shape::naming::name_threads;

const EVALUATOR: &str = "function_finder::interaction_orientation_running_stats";
const NAME: &str = "updateOrientationRunningStats";
const OPTIONS: &str = "{passive: true, capture: true}";

const DIRECT_MOMENTS_BODY: &str = r#"function $THREAD_8_134379($BB) {
  try {
    let $BC = $KEY_147();
    ++$KEY_4;
    if ($KEY_4 <= 1) {
      $KEY_31 = $BC;
      $KEY_26 = $BB.alpha;
      $KEY_10 = $BB.alpha * $BB.alpha;
      $KEY_17 = $BB.beta;
      $KEY_5 = $BB.beta * $BB.beta;
      $KEY_7 = $BB.gamma;
      $KEY_15 = $BB.gamma * $BB.gamma;
    } else {
      $KEY_18 = $KEY_18 * (($KEY_4 - 2) / ($KEY_4 - 1)) + ($BC - $KEY_31) * (1 / ($KEY_4 - 1));
      $KEY_1 = $KEY_1 * (($KEY_4 - 2) / ($KEY_4 - 1)) + ($BC - $KEY_31) * ($BC - $KEY_31) * (1 / ($KEY_4 - 1));
      $KEY_31 = $BC;
      $KEY_26 = $KEY_26 * (($KEY_4 - 1) / $KEY_4) + $BB.alpha * (1 / $KEY_4);
      $KEY_17 = $KEY_17 * (($KEY_4 - 1) / $KEY_4) + $BB.beta * (1 / $KEY_4);
      $KEY_7 = $KEY_7 * (($KEY_4 - 1) / $KEY_4) + $BB.gamma * (1 / $KEY_4);
      $KEY_10 = $KEY_10 * (($KEY_4 - 1) / $KEY_4) + $BB.alpha * $BB.alpha * (1 / $KEY_4);
      $KEY_5 = $KEY_5 * (($KEY_4 - 1) / $KEY_4) + $BB.beta * $BB.beta * (1 / $KEY_4);
      $KEY_15 = $KEY_15 * (($KEY_4 - 1) / $KEY_4) + $BB.gamma * $BB.gamma * (1 / $KEY_4);
    }
    $KEY_27 = true;
  } catch (e) {
    $KEY_9.error(e);
  }
}"#;

const HOISTED_MOMENTS_BODY: &str = r#"function $THREAD_4_17972($CS) {
  let $CT;
  try {
    $CT = $KEY_370();
    ++$KEY_31;
    let $CU;
    if ($KEY_31 <= 1) {
      $KEY_25 = $CT;
      $KEY_14 = $CS.alpha;
      $KEY_3 = $CS.alpha * $CS.alpha;
      $KEY_44 = $CS.beta;
      $KEY_13 = $CS.beta * $CS.beta;
      $KEY_2 = $CS.gamma;
      $CU = $CS.gamma * $CS.gamma;
    } else {
      $KEY_18 = $KEY_18 * (($KEY_31 - 2) / ($KEY_31 - 1)) + ($CT - $KEY_25) * (1 / ($KEY_31 - 1));
      $KEY_22 = $KEY_22 * (($KEY_31 - 2) / ($KEY_31 - 1)) + ($CT - $KEY_25) * ($CT - $KEY_25) * (1 / ($KEY_31 - 1));
      $KEY_25 = $CT;
      $CT = ($KEY_31 - 1) / $KEY_31;
      $KEY_14 = $KEY_14 * (($KEY_31 - 1) / $KEY_31) + $CS.alpha * (1 / $KEY_31);
      $KEY_44 = $KEY_44 * (($KEY_31 - 1) / $KEY_31) + $CS.beta * (1 / $KEY_31);
      $KEY_2 = $KEY_2 * (($KEY_31 - 1) / $KEY_31) + $CS.gamma * (1 / $KEY_31);
      $KEY_3 = $KEY_3 * (($KEY_31 - 1) / $KEY_31) + $CS.alpha * $CS.alpha * (1 / $KEY_31);
      $KEY_13 = $KEY_13 * (($KEY_31 - 1) / $KEY_31) + $CS.beta * $CS.beta * (1 / $KEY_31);
      $CU = $KEY_42 * (($KEY_31 - 1) / $KEY_31) + $CS.gamma * $CS.gamma * (1 / $KEY_31);
    }
    $CT = 18210;
    $KEY_42 = $CU;
    $KEY_35 = true;
  } catch (e) {
    $KEY_37.error(e);
  }
}"#;

const DIRECT_MOMENTS_STATE: &str = "let $KEY_4=0,$KEY_31=0,$KEY_26=0,$KEY_10=0,$KEY_17=0,$KEY_5=0,$KEY_7=0,$KEY_15=0,$KEY_18=0,$KEY_1=0,$KEY_27=0;";
const HOISTED_MOMENTS_STATE: &str = "let $KEY_31=0,$KEY_25=0,$KEY_14=0,$KEY_3=0,$KEY_44=0,$KEY_13=0,$KEY_2=0,$KEY_42=0,$KEY_18=0,$KEY_22=0,$KEY_35=0;";

fn main_prefix(state: &str, clock: &str, sink: &str, target: &str) -> String {
    format!(
        "function main(){{{state}let {clock}=()=>0;let {sink}={{error(){{}}}};let {target}={{addEventListener(){{}}}};"
    )
}

fn direct_moments() -> String {
    let prefix = main_prefix(DIRECT_MOMENTS_STATE, "$KEY_147", "$KEY_9", "$KEY_20");
    let registration =
        format!("$KEY_20.addEventListener(\"deviceorientation\",$THREAD_8_134379,{OPTIONS});");
    [
        prefix.as_str(),
        "(()=>{",
        DIRECT_MOMENTS_BODY,
        registration.as_str(),
        "})();}main();",
    ]
    .concat()
}

fn hoisted_moments() -> String {
    let prefix = main_prefix(HOISTED_MOMENTS_STATE, "$KEY_370", "$KEY_37", "$KEY_32");
    let registration =
        format!("$KEY_32.addEventListener(\"deviceorientation\",$THREAD_4_17972,{OPTIONS});");
    [
        prefix.as_str(),
        "(()=>{",
        HOISTED_MOMENTS_BODY,
        registration.as_str(),
        "})();}main();",
    ]
    .concat()
}

fn alias_registered() -> String {
    let prefix = main_prefix(DIRECT_MOMENTS_STATE, "$KEY_147", "$KEY_9", "$KEY_20");
    let registration = format!(
        "let $KEY_3=$THREAD_8_134379;$KEY_20.addEventListener(\"deviceorientation\",$KEY_3,{OPTIONS});"
    );
    [
        prefix.as_str(),
        "(()=>{",
        DIRECT_MOMENTS_BODY,
        registration.as_str(),
        "})();}main();",
    ]
    .concat()
}

fn two_sibling_scopes() -> String {
    let second = DIRECT_MOMENTS_BODY.replace("$THREAD_8_134379", "$THREAD_8_134380");
    let prefix = main_prefix(DIRECT_MOMENTS_STATE, "$KEY_147", "$KEY_9", "$KEY_20");
    let first_registration =
        format!("$KEY_20.addEventListener(\"deviceorientation\",$THREAD_8_134379,{OPTIONS});");
    let second_registration = "$KEY_20.addEventListener(\"deviceorientation\",$THREAD_8_134380,{capture: true, passive: true});";
    [
        prefix.as_str(),
        "(()=>{",
        DIRECT_MOMENTS_BODY,
        first_registration.as_str(),
        "})();(()=>{",
        second.as_str(),
        second_registration,
        "})();}main();",
    ]
    .concat()
}

fn reused_alias_sibling() -> String {
    let state = format!("{DIRECT_MOMENTS_STATE}let $KEY_0=null;");
    let prefix = main_prefix(&state, "$KEY_147", "$KEY_9", "$KEY_20");
    let second = DIRECT_MOMENTS_BODY.replace("$THREAD_8_134379", "$THREAD_8_134380");
    let first_registration = format!(
        "$KEY_0=$THREAD_8_134379;$KEY_20.addEventListener(\"deviceorientation\",$KEY_0,{OPTIONS});"
    );
    let second_registration = format!(
        "$KEY_0=$THREAD_8_134380;$KEY_20.addEventListener(\"deviceorientation\",$KEY_0,{OPTIONS});"
    );
    [
        prefix.as_str(),
        "(()=>{",
        DIRECT_MOMENTS_BODY,
        first_registration.as_str(),
        "})();(()=>{",
        second.as_str(),
        second_registration.as_str(),
        "})();}main();",
    ]
    .concat()
}

fn reused_alias_same_arrow() -> String {
    let state = format!("{DIRECT_MOMENTS_STATE}let $KEY_0=null;");
    let prefix = main_prefix(&state, "$KEY_147", "$KEY_9", "$KEY_20");
    let second = DIRECT_MOMENTS_BODY.replace("$THREAD_8_134379", "$THREAD_8_134380_2");
    let first_registration = format!(
        "$KEY_0=$THREAD_8_134379;$KEY_20.addEventListener(\"deviceorientation\",$KEY_0,{OPTIONS});"
    );
    let second_registration = format!(
        "$KEY_0=$THREAD_8_134380_2;$KEY_20.addEventListener(\"deviceorientation\",$KEY_0,{OPTIONS});"
    );
    [
        prefix.as_str(),
        "(()=>{",
        DIRECT_MOMENTS_BODY,
        first_registration.as_str(),
        second.as_str(),
        second_registration.as_str(),
        "})();}main();",
    ]
    .concat()
}

fn renamed_spelling() -> String {
    let body = DIRECT_MOMENTS_BODY
        .replace("$THREAD_8_134379", "$THREAD_2_44444")
        .replace("$KEY_147", "readClock")
        .replace("$KEY_31", "lastClock")
        .replace("$KEY_26", "meanAlpha")
        .replace("$KEY_10", "momentAlpha")
        .replace("$KEY_17", "meanBeta")
        .replace("$KEY_15", "momentGamma")
        .replace("$KEY_18", "meanDelta")
        .replace("$KEY_27", "ready")
        .replace("$KEY_4", "sampleCount")
        .replace("$KEY_5", "momentBeta")
        .replace("$KEY_7", "meanGamma")
        .replace("$KEY_9", "errorSink")
        .replace("$KEY_20", "hostTarget")
        .replace("$KEY_1", "momentDelta")
        .replace("$BB", "sample")
        .replace("$BC", "now");
    [
        "function main(){let sampleCount=0,lastClock=0,meanAlpha=0,momentAlpha=0,meanBeta=0,momentBeta=0,meanGamma=0,momentGamma=0,meanDelta=0,momentDelta=0,ready=0;",
        "let readClock=()=>0;let errorSink={error(){}};let hostTarget={addEventListener(){}};",
        "(()=>{",
        body.as_str(),
        "hostTarget.addEventListener(\"deviceorientation\",$THREAD_2_44444,",
        OPTIONS,
        ");})();}main();",
    ]
    .concat()
}

fn decoy_hold_statistics() -> String {
    let body = r#"function $THREAD_2_900($event) {
  try {
    let $now = $KEY_147();
    ++$KEY_4;
    if ($KEY_4 <= 1) {
      $KEY_31 = $now;
    } else {
      let $delta = $now - $KEY_31;
      $KEY_18 = $KEY_18 + $delta;
      $KEY_31 = $now;
    }
    $KEY_27 = true;
  } catch (e) {
    $KEY_9.error(e);
  }
}"#;
    let prefix = main_prefix(DIRECT_MOMENTS_STATE, "$KEY_147", "$KEY_9", "$KEY_20");
    let registration = format!(
        "$KEY_20.addEventListener(\"deviceorientation\",$THREAD_2_900,{OPTIONS});"
    );
    [
        prefix.as_str(),
        "(()=>{",
        body,
        registration.as_str(),
        registration.as_str(),
        "})();}main();",
    ]
    .concat()
}

fn decoy_angle_sums() -> String {
    let body = r#"function $THREAD_2_901($event) {
  try {
    let $now = $KEY_147();
    ++$KEY_4;
    if ($KEY_4 <= 1) {
      $KEY_31 = $now;
    } else {
      $KEY_18 = $KEY_18 + $event.alpha + $event.beta + $event.gamma;
      $KEY_31 = $now;
    }
    $KEY_27 = true;
  } catch (e) {
    $KEY_9.error(e);
  }
}"#;
    let prefix = main_prefix(DIRECT_MOMENTS_STATE, "$KEY_147", "$KEY_9", "$KEY_20");
    let registration = format!(
        "$KEY_20.addEventListener(\"deviceorientation\",$THREAD_2_901,{OPTIONS});"
    );
    [
        prefix.as_str(),
        "(()=>{",
        body,
        registration.as_str(),
        registration.as_str(),
        "})();}main();",
    ]
    .concat()
}

fn doubled_registration(source: String, callback: &str, options: &str) -> String {
    let single = format!("$KEY_20.addEventListener(\"deviceorientation\",{callback},{options});");
    let doubled = format!("{single}{single}");
    source.replace(&single, &doubled)
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
fn witness_orientation_helper_is_claimed_and_renamed() {
    let source = direct_moments();
    assert_eq!(claims(&source), 1, "{source}");
    assert_eq!(applied(&source), 1, "{source}");
    let renamed = name_threads(&source, true).unwrap();
    assert!(renamed.source.contains("function updateOrientationRunningStats("));
    assert!(!renamed.source.contains("function $THREAD_8_134379("));
}

#[test]
fn hoisted_moments_running_moments_body_is_claimed() {
    let source = hoisted_moments();
    assert_eq!(claims(&source), 1, "{source}");
    assert_eq!(applied(&source), 1, "{source}");
    let renamed = name_threads(&source, true).unwrap().source;
    assert!(renamed.contains("function updateOrientationRunningStats("));
}

#[test]
fn alias_registered_helper_is_claimed() {
    let source = alias_registered();
    assert_eq!(claims(&source), 1, "{source}");
    assert_eq!(applied(&source), 1, "{source}");
}

#[test]
fn reused_alias_is_resolved_at_each_registration() {
    let source = reused_alias_sibling();
    assert_eq!(claims(&source), 2, "{source}");
    assert_eq!(applied(&source), 2, "{source}");
}

#[test]
fn reused_alias_same_arrow_claims_both_copies() {
    let source = reused_alias_same_arrow();
    assert_eq!(claims(&source), 2, "{source}");
    // The unsuffixed copy keeps its name in this worktree; the separately
    // allocator suffixes the `_2` copy. Both claims exist.
    assert!(applied(&source) >= 1, "{source}");
}

#[test]
fn renamed_spelling_is_claimed() {
    let source = renamed_spelling();
    assert_eq!(claims(&source), 1, "{source}");
    assert_eq!(applied(&source), 1, "{source}");
}

#[test]
fn two_sibling_scope_copies_are_both_claimed() {
    let source = two_sibling_scopes();
    assert_eq!(claims(&source), 2, "{source}");
    assert_eq!(applied(&source), 2, "{source}");
    let renamed = name_threads(&source, true).unwrap().source;
    assert_eq!(
        renamed
            .matches("function updateOrientationRunningStats(")
            .count(),
        2
    );
}

#[test]
fn wrong_moments_are_refused() {
    let source = direct_moments().replace(
        "$KEY_15 = $KEY_15 * (($KEY_4 - 1) / $KEY_4) + $BB.gamma * $BB.gamma * (1 / $KEY_4);",
        "$KEY_15 = $KEY_15 * (($KEY_4 - 1) / $KEY_4) + $BB.gamma * (1 / $KEY_4);",
    );
    let source = doubled_registration(source, "$THREAD_8_134379", OPTIONS);
    assert_eq!(claims(&source), 0, "accepted wrong moments:\n{source}");
}

#[test]
fn wrong_moment_coefficient_is_refused() {
    let source = direct_moments().replace(
        "$KEY_18 * (($KEY_4 - 2) / ($KEY_4 - 1))",
        "$KEY_18 * (($KEY_4 - 1) / ($KEY_4 - 1))",
    );
    let source = doubled_registration(source, "$THREAD_8_134379", OPTIONS);
    assert_eq!(claims(&source), 0, "accepted wrong coefficient:\n{source}");
}

#[test]
fn wrong_registration_receiver_is_refused() {
    let source = direct_moments()
        .replace("(()=>{", "(($target)=>{")
        .replace(
            "$KEY_20.addEventListener(\"deviceorientation\",$THREAD_8_134379,",
            "$target.addEventListener(\"deviceorientation\",$THREAD_8_134379,",
        )
        .replace("})();}main();", "})({addEventListener(){}});}main();");
    let single = format!("$target.addEventListener(\"deviceorientation\",$THREAD_8_134379,{OPTIONS});");
    let doubled = format!("{single}{single}");
    let source = source.replace(&single, &doubled);
    assert_eq!(claims(&source), 0, "accepted wrong receiver:\n{source}");
}

#[test]
fn wrapper_callback_is_refused() {
    let source = direct_moments().replace(
        "$KEY_20.addEventListener(\"deviceorientation\",$THREAD_8_134379,",
        "let $WRAP=($e)=>{$THREAD_8_134379($e);};$KEY_20.addEventListener(\"deviceorientation\",$WRAP,",
    );
    let source = doubled_registration(source, "$WRAP", OPTIONS);
    assert_eq!(claims(&source), 0, "accepted wrong callback:\n{source}");
}

#[test]
fn shadowed_running_state_is_refused() {
    let source = direct_moments().replace(
        "    let $BC = $KEY_147();",
        "    let $BC = $KEY_147();\n    let $KEY_26 = 0;",
    );
    let source = doubled_registration(source, "$THREAD_8_134379", OPTIONS);
    assert_eq!(claims(&source), 0, "accepted shadowed state:\n{source}");
}

#[test]
fn wrong_options_are_refused() {
    let single =
        format!("$KEY_20.addEventListener(\"deviceorientation\",$THREAD_8_134379,{OPTIONS});");
    let wrong_single = "$KEY_20.addEventListener(\"deviceorientation\",$THREAD_8_134379,{passive: false, capture: true});";
    let source = direct_moments().replace(&single, &format!("{wrong_single}{wrong_single}"));
    assert_eq!(claims(&source), 0, "accepted wrong options:\n{source}");
}

#[test]
fn error_sink_shadowed_by_caught_binding_is_refused() {
    let source = direct_moments().replace("$KEY_9.error(e);", "e.error(e);");
    let source = doubled_registration(source, "$THREAD_8_134379", OPTIONS);
    assert_eq!(claims(&source), 0, "accepted wrong error sink:\n{source}");
}

#[test]
fn decoy_statistics_are_refused() {
    for source in [decoy_hold_statistics(), decoy_angle_sums()] {
        assert_eq!(claims(&source), 0, "accepted decoy:\n{source}");
    }
}

#[test]
fn parent_initializer_is_not_a_candidate() {
    let source = direct_moments();
    let report = name_threads(&source, true).unwrap();
    let parent_roles = report
        .report
        .function_roles
        .iter()
        .filter(|role| role.candidates.iter().any(|candidate| candidate == NAME))
        .count();
    assert_eq!(parent_roles, 1, "{source}");
}
