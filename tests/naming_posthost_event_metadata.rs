//! Focused, portable coverage for the UI-event metadata descriptor family.
//!
//! The positives are real cleaned-source bodies from the discovery evidence:
//! the full keyboard sample, the inline and local/aliased geometry samples, the
//! inline and returned-position pointer samples, and the touch-sample variant,
//! plus one alpha spelling body. Negatives are meaningful receiver, shadow,
//! property, disconnected-record and nested-container changes that must stay
//! unclaimed.
//!
//! The source is only parsed; the captured sinks and clocks are never called.
use shape::naming::name_threads;

const EVALUATOR: &str = "thread_posthost_event_metadata";
const KEYBOARD: &str = "recordKeyboardEventSample";
const MOUSE: &str = "recordMouseEventSample";
const POINTER: &str = "recordPointerEventSample";
const TOUCH: &str = "recordTouchSample";
const KEYBOARD_BODY: &str = r#"function $THREAD_7_14760($AU, $AV) {
				let $AW, $AX, $AY, $AZ;
				if ($AV.target != null) {
					$AZ = $AV.target.id;
					$AX = $AV.target.name;
					$AW = $AV.target.offsetHeight;
					$AY = $AV.target.offsetWidth;
				} else {
					$AW = null;
					$AX = null;
					$AY = null;
					$AZ = null;
				}
				$AZ = {
					id: $AZ,
					name: $AX,
					height: $AW,
					width: $AY
				};
				let $KEY_1 = {
					instanceOfUIEvent: $AV instanceof UIEvent,
					markedAsTrusted: !!$AV.isTrusted
				};
				const $BA = $AU === 3 ? $KEY_346($AV.key || "") : typeof ($AV.which || $AV.keyCode) !== "number" ? 0 : $AV.which || $AV.keyCode;
				const $BB = ($AV.key || "") === "Unidentified" ? 1 : !$AV.ctrlKey && !$AV.metaKey ? $AV.altKey ? !RegExp("^[\\x20-\\x7E]$").test($AV.key || "") || $AV.shiftKey && ($AV.key || "") === "`" ? 1 : $BA : ($AV.key || "").length === 0 || ($AV.key || "").length <= 2 && RegExp("^(?:[\\0-\\t\\x0B\\f\\x0E-\\u2027\\u202A-\\uD7FF\\uE000-\\uFFFF]|[\\uD800-\\uDBFF][\\uDC00-\\uDFFF]|[\\uD800-\\uDBFF](?![\\uDC00-\\uDFFF])|(?:[^\\uD800-\\uDBFF]|^)[\\uDC00-\\uDFFF])$").test($AV.key || "") || $KEY_148.indexOf($AV.key || "") >= 0 ? 1 : $AU !== 3 && ($BA === 32 || $BA >= 48 && $BA <= 57 || $BA >= 58 && $BA <= 64 || $BA >= 65 && $BA <= 90 || $BA >= 96 && $BA <= 111 || $BA >= 160 && $BA <= 176 || $BA >= 186 && $BA <= 192 || $BA >= 219 && $BA <= 222) ? 1 : $BA : $BA;
				let $BC, $BD;
				if ($AU === 1) {
					$BC = [true];
					$BD = false;
				} else {
					$BD = $AU === 2 && true;
					$BC = [false];
				}
				$BC[1] = $BD;
				let $BE = $KEY_7;
				let $BF = $KEY_7.put;
				$BC = {
					a: $BC,
					b: $KEY_16()
				};
				let $BG = [!!$AV.ctrlKey, !!$AV.metaKey, !!$AV.altKey, !!$AV.shiftKey];
				$BC.c = $BG;
				$BC.d = $BB;
				$BC.e = $AZ.id;
				$BC.f = $AZ.name;
				$BC.g = $KEY_1.instanceOfUIEvent;
				$BC.h = $KEY_1.markedAsTrusted;
				$BF.call($BE, $BC);
				$KEY_0 = true;
			}"#;
const GEOMETRY_BODY: &str = r#"function $THREAD_0_101014($CT) {
					try {
						let $CU = {
							id: $CT.target != null ? $CT.target.id : null,
							name: $CT.target != null ? $CT.target.name : null,
							height: $CT.target != null ? $CT.target.offsetHeight : null,
							width: $CT.target != null ? $CT.target.offsetWidth : null
						};
						let $CV = {
							instanceOfUIEvent: $CT instanceof UIEvent,
							markedAsTrusted: !!$CT.isTrusted
						};
						let $CW = $CT.target;
						$CW = $CW instanceof Element ? $CW.getBoundingClientRect() : null;
						const $CX = $CW == null || $CT.clientX == null ? 0 : Math.round($CT.clientX - $CW.left);
						$CW = $CW == null || $CT.clientY == null ? 0 : Math.round($CT.clientY - $CW.top);
						$KEY_11.put({
							a: $KEY_1(),
							b: $CT.pageX == null ? null : Math.round($CT.pageX),
							c: $CT.pageY == null ? null : Math.round($CT.pageY),
							d: [true, false],
							e: $CV.instanceOfUIEvent,
							f: $CV.markedAsTrusted,
							g: $CU.height,
							h: $CU.id,
							i: $CU.name,
							j: $CX,
							k: $CW,
							l: $CU.width,
							m: $CT.button
						});
						$KEY_17 = true;
					} catch (e) {
						$KEY_25.error(e);
					}
				}"#;
const GEOMETRY_LOCAL_BODY: &str = r#"function $THREAD_1_110482($CW) {
					try {
						let $CX, $CY, $CZ, $DA;
						if ($CW.target != null) {
							$CY = $CW.target.id;
							$DA = $CW.target.name;
							$CX = $CW.target.offsetHeight;
							$CZ = $CW.target.offsetWidth;
						} else {
							$CX = null;
							$CY = null;
							$CZ = null;
							$DA = null;
						}
						$CY = {
							id: $CY,
							name: $DA,
							height: $CX,
							width: $CZ
						};
						let $DB = {
							instanceOfUIEvent: $CW instanceof UIEvent,
							markedAsTrusted: !!$CW.isTrusted
						};
						let $DC = $CW.target;
						$DC = $DC instanceof Element ? $DC.getBoundingClientRect() : null;
						const $DD = $DC == null || $CW.clientX == null ? 0 : Math.round($CW.clientX - $DC.left);
						$DC = $DC == null || $CW.clientY == null ? 0 : Math.round($CW.clientY - $DC.top);
						let $DE = $KEY_23;
						$DC = {
							a: $KEY_29(),
							b: $CW.pageX == null ? null : Math.round($CW.pageX),
							c: $CW.pageY == null ? null : Math.round($CW.pageY),
							d: [false, true],
							e: $DB.instanceOfUIEvent,
							f: $DB.markedAsTrusted,
							g: $CY.height,
							h: $CY.id,
							i: $CY.name,
							j: $DD,
							k: $DC,
							l: $CY.width,
							m: $CW.button
						};
						$DE.put($DC);
						$KEY_19 = true;
					} catch (e) {
						$KEY_10.error(e);
					}
				}"#;
const POINTER_BODY: &str = r#"function $THREAD_5_87976($DA) {
					try {
						let $DB = {
							instanceOfUIEvent: $DA instanceof UIEvent,
							markedAsTrusted: !!$DA.isTrusted
						};
						$KEY_16.put({
							a: $KEY_19(),
							b: $DA.pageX,
							c: $DA.pageY,
							d: $DB.instanceOfUIEvent,
							e: $DB.markedAsTrusted
						});
						$KEY_1 = true;
						$KEY_34($DA);
					} catch (e) {
						$KEY_29.error(e);
					}
				}"#;
const POINTER_LOCAL_BODY: &str = r#"function $THREAD_6_112407($CT) {
								try {
									let $CU = {
										instanceOfUIEvent: $CT instanceof UIEvent,
										markedAsTrusted: !!$CT.isTrusted
									};
									let $CV = { a: $KEY_1() };
									$CV.b = $CT.pageX;
									$CV.c = $CT.pageY;
									$CV.d = $CU.instanceOfUIEvent;
									$CV.e = $CU.markedAsTrusted;
									$KEY_0.put($CV);
									$KEY_17 = true;
									$KEY_28($CT);
								} catch (e) {
									$KEY_25.error(e);
								}
							}"#;
const TOUCH_BODY: &str = r#"function $THREAD_1_144804($CK) {
					try {
						let $CL = [];
						let $CM = {
							instanceOfUIEvent: $CK instanceof UIEvent,
							markedAsTrusted: !!$CK.isTrusted
						};
						let $CN = $CK[$CK.touches && $CK.touches.length === 0 ? "changedTouches" : "touches"];
						if ($CN && $CN.length !== 0) {
							let $CO = $CN.length;
							let i = 0;
							for (; i < $CO; i++) {
								let $CP = $CN[i];
								let $CQ = $CP.target;
								$CQ = $CQ instanceof Element ? $CQ.getBoundingClientRect() : null;
								const $CR = $CQ == null ? 0 : Math.round($CP.clientX - $CQ.left);
								$CQ = $CQ == null ? 0 : Math.round($CP.clientY - $CQ.top);
								$CL.push({
									a: Math.round($CP.pageX),
									b: Math.round($CP.pageY),
									c: $CR,
									d: $CQ,
									e: $CP.force
								});
							}
						}
						let $CS = $KEY_31;
						let $CT = { a: [false, true] };
						$CT.b = $KEY_20();
						$CT.c = $CL;
						$CT.d = $CM.instanceOfUIEvent;
						$CT.e = $CM.markedAsTrusted;
						$CS.put($CT);
						$KEY_40 = true;
					} catch (e) {
						$KEY_11.error(e);
					}
				}"#;
const RETURNED_POINTER_BODY: &str = r#"function $THREAD_5_5005($E) {
let $M = {
instanceOfUIEvent: $E instanceof UIEvent,
markedAsTrusted: !!$E.isTrusted
};
let $R = {
a: $CLOCK(),
b: $E.pageX,
c: $E.pageY,
d: $M.instanceOfUIEvent,
e: $M.markedAsTrusted
};
return $R;
}"#;
const ALPHA_BODY: &str = r#"function $THREAD_9_4242($T, $E) {
let $X1, $X2, $X3, $X4;
if ($E.target != null) {
$X3 = $E.target.id;
$X2 = $E.target.name;
$X1 = $E.target.offsetHeight;
$X4 = $E.target.offsetWidth;
} else {
$X1 = null;
$X2 = null;
$X3 = null;
$X4 = null;
}
$X3 = {
id: $X3,
name: $X2,
height: $X1,
width: $X4
};
let $M = {
instanceOfUIEvent: $E instanceof UIEvent,
markedAsTrusted: !!$E.isTrusted
};
const $K = $T === 3 ? $H($E.key || "") : typeof ($E.which || $E.keyCode) !== "number" ? 0 : $E.which || $E.keyCode;
let $B1, $B2;
if ($T === 1) {
$B1 = [true];
$B2 = false;
} else {
$B2 = $T === 2 && true;
$B1 = [false];
}
$B1[1] = $B2;
let $R = $SINK;
let $P = $SINK.put;
$B1 = {
a: $B1,
b: $CLOCK()
};
let $MOD = [!!$E.ctrlKey, !!$E.metaKey, !!$E.altKey, !!$E.shiftKey];
$B1.c = $MOD;
$B1.d = $K;
$B1.e = $X3.id;
$B1.f = $X3.name;
$B1.g = $M.instanceOfUIEvent;
$B1.h = $M.markedAsTrusted;
$P.call($R, $B1);
$READY = true;
}"#;

const CONSUMER_BODY: &str = r#"function $THREAD_3_3003($E) {
try {
let $L = { a: clock() };
$L.b = $E.pageX;
$L.c = $E.pageY;
$RING.put($L);
} catch (e) {
$ERR.error(e);
}
}"#;

const TARGET_ONLY_BODY: &str = r#"function $THREAD_4_4004($E) {
let $T = {
id: $E.target.id,
name: $E.target.name,
height: $E.target.offsetHeight,
width: $E.target.offsetWidth
};
return $T;
}"#;

fn keyboard() -> String {
    [
        "function main(){",
        "let $KEY_7={put:function(v){return v;}};",
        "let $KEY_148=[];",
        "let $KEY_0=false;",
        "function $KEY_346($v){return $v.length;}",
        "function clock(){return 7;}",
        "let $KEY_16=clock;",
        KEYBOARD_BODY,
        "return $THREAD_7_14760;}main();",
    ]
    .concat()
}

fn geometry() -> String {
    [
        "function main(){",
        "let $KEY_11={put:function(v){return v;}};",
        "let $KEY_25={error:function(e){return e;}};",
        "let $KEY_17=false;",
        "function clock(){return 7;}",
        "let $KEY_1=clock;",
        GEOMETRY_BODY,
        "return $THREAD_0_101014;}main();",
    ]
    .concat()
}

fn geometry_local() -> String {
    [
        "function main(){",
        "let $KEY_23={put:function(v){return v;}};",
        "let $KEY_10={error:function(e){return e;}};",
        "let $KEY_19=false;",
        "function clock(){return 7;}",
        "let $KEY_29=clock;",
        GEOMETRY_LOCAL_BODY,
        "return $THREAD_1_110482;}main();",
    ]
    .concat()
}

fn pointer() -> String {
    [
        "function main(){",
        "let $KEY_16={put:function(v){return v;}};",
        "let $KEY_29={error:function(e){return e;}};",
        "let $KEY_1=false;",
        "function clock(){return 7;}",
        "let $KEY_19=clock;",
        "let $KEY_34=function(e){return e;};",
        POINTER_BODY,
        "return $THREAD_5_87976;}main();",
    ]
    .concat()
}

fn pointer_local() -> String {
    [
        "function main(){",
        "let $KEY_0={put:function(v){return v;}};",
        "let $KEY_25={error:function(e){return e;}};",
        "let $KEY_17=false;",
        "function clock(){return 7;}",
        "let $KEY_1=clock;",
        "let $KEY_28=function(e){return e;};",
        POINTER_LOCAL_BODY,
        "return $THREAD_6_112407;}main();",
    ]
    .concat()
}

fn touch() -> String {
    [
        "function main(){",
        "let $KEY_31={put:function(v){return v;}};",
        "let $KEY_11={error:function(e){return e;}};",
        "let $KEY_40=false;",
        "function clock(){return 7;}",
        "let $KEY_20=clock;",
        TOUCH_BODY,
        "return $THREAD_1_144804;}main();",
    ]
    .concat()
}

fn returned_pointer() -> String {
    [
        "function main(){",
        "function clock(){return 7;}",
        "let $CLOCK=clock;",
        RETURNED_POINTER_BODY,
        "return $THREAD_5_5005;}main();",
    ]
    .concat()
}

fn alpha() -> String {
    [
        "function main(){",
        "let $SINK={put:function(v){return v;}};",
        "let $READY=false;",
        "function clock(){return 7;}",
        "let $CLOCK=clock;",
        "let $H=function(v){return v.length;};",
        ALPHA_BODY,
        "return $THREAD_9_4242;}main();",
    ]
    .concat()
}

fn claims(source: &str, name: &str) -> usize {
    name_threads(source, true)
        .unwrap()
        .report
        .function_roles
        .iter()
        .filter(|role| {
            role.candidates.iter().any(|candidate| candidate == name)
                && role
                    .evaluators
                    .iter()
                    .any(|evaluator| evaluator == EVALUATOR)
        })
        .count()
}

fn applied(source: &str, name: &str) -> usize {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .filter(|outcome| {
            outcome.applied
                && outcome.proposed_name.as_deref() == Some(name)
                && outcome.evaluator.as_deref() == Some(EVALUATOR)
        })
        .count()
}

fn renamed_contains(source: &str, name: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .source
        .contains(&format!("function {name}("))
}

#[test]
fn full_keyboard_metadata_witness_is_claimed_and_renamed() {
    let source = keyboard();
    assert_eq!(claims(&source, KEYBOARD), 1);
    assert_eq!(applied(&source, KEYBOARD), 1);
    assert!(renamed_contains(&source, KEYBOARD));
    let renamed = name_threads(&source, true).unwrap();
    assert!(!renamed.source.contains("function $THREAD_7_14760("));
    // The readable captured key names are untouched.
    assert!(renamed.source.contains("$KEY_148"));
    assert!(renamed.source.contains("$KEY_346"));
    // Replaying the reported edits reproduces the transformed source.
    let mut replay = source.clone();
    let mut edits = renamed.report.edits.clone();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    for edit in edits {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, renamed.source);
}

#[test]
fn geometry_and_pointer_shapes_are_claimed() {
    let cases = [
        ("geometry", geometry(), MOUSE),
        ("geometry-local", geometry_local(), MOUSE),
        ("pointer", pointer(), POINTER),
        ("pointer-local", pointer_local(), POINTER),
        ("alpha-full", alpha(), KEYBOARD),
        ("returned-pointer", returned_pointer(), POINTER),
    ];
    for (label, source, name) in cases {
        assert_eq!(claims(&source, name), 1, "{label}");
        assert_eq!(applied(&source, name), 1, "{label}");
        assert!(renamed_contains(&source, name), "{label}");
    }
}

#[test]
fn touch_metadata_shape_keeps_the_existing_touch_sample_role() {
    let source = touch();
    assert_eq!(claims(&source, TOUCH), 1);
    assert_eq!(applied(&source, TOUCH), 1);
    assert!(renamed_contains(&source, TOUCH));
    assert!(!renamed_contains(&source, POINTER));
}

#[test]
fn alpha_spellings_and_put_call_form_are_equivalent() {
    let source = alpha();
    let renamed = name_threads(&source, true).unwrap();
    assert!(renamed
        .source
        .contains("function recordKeyboardEventSample("));
    assert!(renamed.source.contains("$P.call($R, $B1);"));
}

#[test]
fn a_wrong_event_receiver_is_refused() {
    let source = keyboard().replace(
        "instanceOfUIEvent: $AV instanceof UIEvent",
        "instanceOfUIEvent: $AU instanceof UIEvent",
    );
    assert_eq!(claims(&source, KEYBOARD), 0);
    assert_eq!(applied(&source, KEYBOARD), 0);
}

#[test]
fn a_shadowed_ui_event_global_is_refused() {
    let source = keyboard().replace(
        "function main(){",
        "function main(){let UIEvent=function(){};",
    );
    assert_eq!(claims(&source, KEYBOARD), 0);
}

#[test]
fn a_swapped_target_property_is_refused() {
    let source = keyboard().replace("height: $AW", "height: $AY");
    assert_eq!(claims(&source, KEYBOARD), 0);
}

#[test]
fn a_record_disconnected_from_the_target_snapshot_is_refused() {
    let source = geometry()
        .replace(
            "$KEY_11.put({",
            "let $CU2={id:1,name:\"x\",height:1,width:1};$KEY_11.put({",
        )
        .replace("$CU.height", "$CU2.height")
        .replace("$CU.id", "$CU2.id")
        .replace("$CU.name", "$CU2.name")
        .replace("$CU.width", "$CU2.width");
    assert_eq!(claims(&source, MOUSE), 0);
}

#[test]
fn a_consumer_record_without_the_metadata_pair_is_refused() {
    let source = [
        "function main(){",
        "let $RING={put:function(v){return v;}};",
        "let $ERR={error:function(e){return e;}};",
        "function clock(){return 7;}",
        CONSUMER_BODY,
        "return $THREAD_3_3003;}main();",
    ]
    .concat();
    assert_eq!(claims(&source, POINTER), 0);
    assert_eq!(claims(&source, MOUSE), 0);
    assert_eq!(claims(&source, TOUCH), 0);
    assert_eq!(claims(&source, KEYBOARD), 0);
}

#[test]
fn an_arbitrary_target_object_builder_is_refused() {
    let source = [
        "function main(){",
        TARGET_ONLY_BODY,
        "return $THREAD_4_4004;}main();",
    ]
    .concat();
    assert_eq!(claims(&source, KEYBOARD), 0);
    assert_eq!(claims(&source, MOUSE), 0);
    assert_eq!(claims(&source, POINTER), 0);
    assert_eq!(claims(&source, TOUCH), 0);
}

#[test]
fn a_container_borrows_no_nested_descriptor_evidence() {
    let source = [
        "function main(){",
        "let $KEY_11={put:function(v){return v;}};",
        "let $KEY_25={error:function(e){return e;}};",
        "let $KEY_17=false;",
        "function clock(){return 7;}",
        "let $KEY_1=clock;",
        "function $THREAD_7_7777($CT){",
        GEOMETRY_BODY,
        "}",
        "return $THREAD_7_7777;}main();",
    ]
    .concat();
    assert_eq!(claims(&source, MOUSE), 1);
    assert_eq!(applied(&source, MOUSE), 1);
    let report = name_threads(&source, true).unwrap();
    // The outer container never receives the descriptor role from its child.
    assert!(report.report.outcomes.iter().all(|outcome| {
        !(outcome.binding.name == "$THREAD_7_7777"
            && outcome.evaluator.as_deref() == Some(EVALUATOR))
    }));
    assert!(report.source.contains("function recordMouseEventSample("));
}

#[test]
fn an_existing_readable_recorder_name_is_not_replaced() {
    let source = geometry()
        .replace("function $THREAD_0_101014(", "function captureClickEvent(")
        .replace("return $THREAD_0_101014;", "return captureClickEvent;");
    let renamed = name_threads(&source, true).unwrap();
    assert_eq!(renamed.source, source);
    assert!(!renamed.source.contains("function recordMouseEventSample("));
    assert_eq!(applied(&source, MOUSE), 0);
}

#[test]
fn a_local_name_collision_is_not_applied() {
    let source = keyboard().replace(
        "return $THREAD_7_14760;}main();",
        "function recordKeyboardEventSample(){}\nreturn $THREAD_7_14760;}main();",
    );
    let report = name_threads(&source, true).unwrap();
    let outcome = report
        .report
        .outcomes
        .iter()
        .find(|outcome| {
            outcome.binding.name == "$THREAD_7_14760"
                && outcome.evaluator.as_deref() == Some(EVALUATOR)
        })
        .expect("the descriptor outcome is reported");
    assert_eq!(outcome.proposed_name.as_deref(), Some(KEYBOARD));
    assert!(!outcome.applied);
    assert_eq!(outcome.reason.as_deref(), Some("NameCollision"));
}

#[test]
fn two_descriptor_instances_in_one_scope_are_both_claimed() {
    let mut source = String::from("function main(){");
    source.push_str("let $KEY_7={put:function(v){return v;}};");
    source.push_str("let $KEY_148=[];");
    source.push_str("let $KEY_0=false;");
    source.push_str("function $KEY_346($v){return $v.length;}");
    source.push_str("function clock(){return 7;}");
    source.push_str("let $KEY_16=clock;");
    source.push_str(&KEYBOARD_BODY.replace("$THREAD_7_14760", "$THREAD_8_2001"));
    source.push_str(&KEYBOARD_BODY.replace("$THREAD_7_14760", "$THREAD_8_2002"));
    source.push_str("return $THREAD_8_2001;}main();");
    // Both instances produce the canonical role claim. Whether the separate
    // duplicate-name allocation family applies both (base + ordinal) or
    // refuses the batch is its policy, not this evaluator's: what this focused
    // test owns is that the descriptor proves both bodies and that any applied
    // rename can never collapse two bindings onto one spelling.
    assert_eq!(claims(&source, KEYBOARD), 2);
    let renamed = name_threads(&source, true).unwrap();
    let mut concrete = std::collections::BTreeSet::new();
    for outcome in renamed.report.outcomes.iter().filter(|outcome| {
        outcome.evaluator.as_deref() == Some(EVALUATOR)
            && outcome.binding.name.starts_with("$THREAD_8_")
    }) {
        if outcome.applied {
            let name = outcome
                .proposed_name
                .as_deref()
                .expect("an applied claim carries a concrete name");
            assert!(
                concrete.insert(name.to_string()),
                "two bindings collapsed onto {name}"
            );
            assert!(renamed.source.contains(&format!("function {name}(")));
        } else {
            // A refused claim must say why; never a silent drop.
            assert!(outcome.reason.is_some(), "unapplied claim lost its reason");
        }
    }
    // At most one binding keeps the unsuffixed canonical spelling.
    assert!(
        concrete
            .iter()
            .filter(|name| name.as_str() == KEYBOARD)
            .count()
            <= 1
    );
}

#[test]
fn an_inline_argument_put_still_needs_a_captured_sink() {
    // Geometry emits its record as the direct argument of `put({...})`. That
    // argument form must still prove a captured receiver: re-binding the sink
    // inside the descriptor body (an object literal with the same put method)
    // is a decoy and the record is not emitted.
    let source = geometry()
        .replace(
            "let $CW = $CT.target;",
            "let $LOCAL_SINK = { put: $KEY_11.put };\nlet $CW = $CT.target;",
        )
        .replace("$KEY_11.put({", "$LOCAL_SINK.put({");
    assert_eq!(claims(&source, MOUSE), 0);
    assert_eq!(applied(&source, MOUSE), 0);
}

// Exact event-metadata bodies. Source coordinates must never drive recognition.
const INLINE_TOUCH_BODIES: &[(&str, &str, &str)] = &[
    (
        "fn@282222..283394",
        "$THREAD_0_6176",
        r##"function $THREAD_0_6176($DE) {
								try {
									let $DF = {
										instanceOfUIEvent: $DE instanceof UIEvent,
										markedAsTrusted: !!$DE.isTrusted
									};
									let $DG = [];
									let $DH = $DE[$DE.touches && $DE.touches.length === 0 ? "changedTouches" : "touches"];
									if ($DH && $DH.length !== 0) {
										let $DI = $DH.length;
										let i = 0;
										for (; i < $DI; i++) {
											let $DJ = $DH[i];
											let $DK = $DJ.target;
											$DK = $DK instanceof Element ? $DK.getBoundingClientRect() : null;
											const $DL = $DK == null ? 0 : Math.round($DJ.clientX - $DK.left);
											$DK = $DK == null ? 0 : Math.round($DJ.clientY - $DK.top);
											$DG.push({
												a: Math.round($DJ.pageX),
												b: Math.round($DJ.pageY),
												c: $DL,
												d: $DK,
												e: $DJ.force
											});
										}
									}
									$KEY_6.put({
										a: [true, false],
										b: $KEY_5(),
										c: $DG,
										d: $DF.instanceOfUIEvent,
										e: $DF.markedAsTrusted
									});
									$KEY_12 = true;
								} catch (e) {
									$KEY_24.error(e);
								}
							}"##,
    ),
    (
        "fn@284523..285696",
        "$THREAD_3_22757",
        r##"function $THREAD_3_22757($DE) {
								try {
									let $DF = {
										instanceOfUIEvent: $DE instanceof UIEvent,
										markedAsTrusted: !!$DE.isTrusted
									};
									let $DG = [];
									let $DH = $DE[$DE.touches && $DE.touches.length === 0 ? "changedTouches" : "touches"];
									if ($DH && $DH.length !== 0) {
										let $DI = $DH.length;
										let i = 0;
										for (; i < $DI; i++) {
											let $DJ = $DH[i];
											let $DK = $DJ.target;
											$DK = $DK instanceof Element ? $DK.getBoundingClientRect() : null;
											const $DL = $DK == null ? 0 : Math.round($DJ.clientX - $DK.left);
											$DK = $DK == null ? 0 : Math.round($DJ.clientY - $DK.top);
											$DG.push({
												a: Math.round($DJ.pageX),
												b: Math.round($DJ.pageY),
												c: $DL,
												d: $DK,
												e: $DJ.force
											});
										}
									}
									$KEY_6.put({
										a: [false, true],
										b: $KEY_5(),
										c: $DG,
										d: $DF.instanceOfUIEvent,
										e: $DF.markedAsTrusted
									});
									$KEY_12 = true;
								} catch (e) {
									$KEY_24.error(e);
								}
							}"##,
    ),
    (
        "fn@286972..288147",
        "$THREAD_2_125005",
        r##"function $THREAD_2_125005($DE) {
								try {
									let $DF = {
										instanceOfUIEvent: $DE instanceof UIEvent,
										markedAsTrusted: !!$DE.isTrusted
									};
									let $DG = [];
									let $DH = $DE[$DE.touches && $DE.touches.length === 0 ? "changedTouches" : "touches"];
									if ($DH && $DH.length !== 0) {
										let $DI = $DH.length;
										let i = 0;
										for (; i < $DI; i++) {
											let $DJ = $DH[i];
											let $DK = $DJ.target;
											$DK = $DK instanceof Element ? $DK.getBoundingClientRect() : null;
											const $DL = $DK == null ? 0 : Math.round($DJ.clientX - $DK.left);
											$DK = $DK == null ? 0 : Math.round($DJ.clientY - $DK.top);
											$DG.push({
												a: Math.round($DJ.pageX),
												b: Math.round($DJ.pageY),
												c: $DL,
												d: $DK,
												e: $DJ.force
											});
										}
									}
									$KEY_6.put({
										a: [false, false],
										b: $KEY_5(),
										c: $DG,
										d: $DF.instanceOfUIEvent,
										e: $DF.markedAsTrusted
									});
									$KEY_12 = true;
								} catch (e) {
									$KEY_24.error(e);
								}
							}"##,
    ),
    (
        "fn@291425..292488",
        "$THREAD_0_6176_2",
        r##"function $THREAD_0_6176_2($DE) {
					try {
						let $DF = {
							instanceOfUIEvent: $DE instanceof UIEvent,
							markedAsTrusted: !!$DE.isTrusted
						};
						let $DG = [];
						let $DH = $DE[$DE.touches && $DE.touches.length === 0 ? "changedTouches" : "touches"];
						if ($DH && $DH.length !== 0) {
							let $DI = $DH.length;
							let i = 0;
							for (; i < $DI; i++) {
								let $DJ = $DH[i];
								let $DK = $DJ.target;
								$DK = $DK instanceof Element ? $DK.getBoundingClientRect() : null;
								const $DL = $DK == null ? 0 : Math.round($DJ.clientX - $DK.left);
								$DK = $DK == null ? 0 : Math.round($DJ.clientY - $DK.top);
								$DG.push({
									a: Math.round($DJ.pageX),
									b: Math.round($DJ.pageY),
									c: $DL,
									d: $DK,
									e: $DJ.force
								});
							}
						}
						$KEY_6.put({
							a: [true, false],
							b: $KEY_5(),
							c: $DG,
							d: $DF.instanceOfUIEvent,
							e: $DF.markedAsTrusted
						});
						$KEY_12 = true;
					} catch (e) {
						$KEY_24.error(e);
					}
				}"##,
    ),
    (
        "fn@293552..294616",
        "$THREAD_3_22757_2",
        r##"function $THREAD_3_22757_2($DE) {
					try {
						let $DF = {
							instanceOfUIEvent: $DE instanceof UIEvent,
							markedAsTrusted: !!$DE.isTrusted
						};
						let $DG = [];
						let $DH = $DE[$DE.touches && $DE.touches.length === 0 ? "changedTouches" : "touches"];
						if ($DH && $DH.length !== 0) {
							let $DI = $DH.length;
							let i = 0;
							for (; i < $DI; i++) {
								let $DJ = $DH[i];
								let $DK = $DJ.target;
								$DK = $DK instanceof Element ? $DK.getBoundingClientRect() : null;
								const $DL = $DK == null ? 0 : Math.round($DJ.clientX - $DK.left);
								$DK = $DK == null ? 0 : Math.round($DJ.clientY - $DK.top);
								$DG.push({
									a: Math.round($DJ.pageX),
									b: Math.round($DJ.pageY),
									c: $DL,
									d: $DK,
									e: $DJ.force
								});
							}
						}
						$KEY_6.put({
							a: [false, true],
							b: $KEY_5(),
							c: $DG,
							d: $DF.instanceOfUIEvent,
							e: $DF.markedAsTrusted
						});
						$KEY_12 = true;
					} catch (e) {
						$KEY_24.error(e);
					}
				}"##,
    ),
    (
        "fn@295780..296846",
        "$THREAD_2_125005_2",
        r##"function $THREAD_2_125005_2($DE) {
					try {
						let $DF = {
							instanceOfUIEvent: $DE instanceof UIEvent,
							markedAsTrusted: !!$DE.isTrusted
						};
						let $DG = [];
						let $DH = $DE[$DE.touches && $DE.touches.length === 0 ? "changedTouches" : "touches"];
						if ($DH && $DH.length !== 0) {
							let $DI = $DH.length;
							let i = 0;
							for (; i < $DI; i++) {
								let $DJ = $DH[i];
								let $DK = $DJ.target;
								$DK = $DK instanceof Element ? $DK.getBoundingClientRect() : null;
								const $DL = $DK == null ? 0 : Math.round($DJ.clientX - $DK.left);
								$DK = $DK == null ? 0 : Math.round($DJ.clientY - $DK.top);
								$DG.push({
									a: Math.round($DJ.pageX),
									b: Math.round($DJ.pageY),
									c: $DL,
									d: $DK,
									e: $DJ.force
								});
							}
						}
						$KEY_6.put({
							a: [false, false],
							b: $KEY_5(),
							c: $DG,
							d: $DF.instanceOfUIEvent,
							e: $DF.markedAsTrusted
						});
						$KEY_12 = true;
					} catch (e) {
						$KEY_24.error(e);
					}
				}"##,
    ),
];

fn inline_touch_fixture(bodies: &str) -> String {
    [
        "function main(){",
        "let $KEY_6={put:function(v){return v;}};",
        "let $KEY_24={error:function(e){return e;}};",
        // A bound zero-argument clock is still captured before every handler.
        // The older anchor accepts plain callable aliases and zero-argument
        // factories, so it intentionally declines this one-argument bind call.
        // This exercises the metadata fallback without changing the six bodies.
        "let $KEY_12=false;function clock(){return 7;}let $KEY_5=clock.bind(null);",
        bodies,
        "}main();",
    ]
    .concat()
}

fn touch_payload_as_local(body: &str) -> String {
    // Only the payload assembly changes; the exact per-touch scan stays owned.
    let start = body.find("$KEY_6.put({").unwrap();
    let end = start + body[start..].find("});").unwrap() + 3;
    let object = &body[start + "$KEY_6.put(".len()..end - 2];
    format!(
        "{}let payload={object};$KEY_6.put(payload);{}",
        &body[..start],
        &body[end..]
    )
}

#[test]
fn six_cached_inline_touch_bodies_independently_claim_and_rename() {
    for &(id, original, body) in INLINE_TOUCH_BODIES {
        for (variant_name, variant) in [
            ("direct-inline", body.to_string()),
            ("local", touch_payload_as_local(body)),
            (
                "put.call-inline",
                body.replace("$KEY_6.put({", "$KEY_6.put.call($KEY_6, {"),
            ),
        ] {
            let source = inline_touch_fixture(&variant);
            let result = name_threads(&source, true).unwrap();
            assert_eq!(
                claims(&source, TOUCH),
                1,
                "{id}: {variant_name}; outcomes: {:?}",
                result.report.outcomes
            );
            assert_eq!(applied(&source, TOUCH), 1, "{id}: {variant_name}");
            assert!(
                !result.source.contains(&format!("function {original}(")),
                "{id}: {variant_name}"
            );
            assert!(
                result.source.contains("function recordTouchSample("),
                "{id}: {variant_name}"
            );
            for key in ["$KEY_6", "$KEY_5", "$KEY_24"] {
                assert!(result.source.contains(key), "{id}: {variant_name}: {key}");
            }
        }
    }
}

#[test]
fn duplicate_touch_phases_do_not_select_an_arbitrary_phase_handler() {
    let bodies = INLINE_TOUCH_BODIES
        .iter()
        .map(|(_, _, body)| *body)
        .collect::<Vec<_>>()
        .join("\n");
    let mut registrations = String::new();
    for (i, &(_, original, _)) in INLINE_TOUCH_BODIES.iter().enumerate() {
        let event = ["touchstart", "touchmove", "touchend"][i % 3];
        registrations.push_str(&format!(
            "document.addEventListener(\"{event}\",{original},false);"
        ));
    }
    let source = inline_touch_fixture(&format!("{bodies}\n{registrations}"));
    assert_eq!(claims(&source, TOUCH), 6);
    let result = name_threads(&source, true).unwrap();
    for &(_, original, _) in INLINE_TOUCH_BODIES {
        assert!(!result.source.contains(&format!("function {original}(")));
    }
    for phase in ["onTouchStart", "onTouchMove", "recordTouchEndSample"] {
        assert!(
            !result
                .report
                .outcomes
                .iter()
                .any(|outcome| outcome.applied && outcome.proposed_name.as_deref() == Some(phase)),
            "{phase}"
        );
    }
}

#[test]
fn inline_touch_wrong_event_list_and_overwritten_list_are_refused() {
    let body = INLINE_TOUCH_BODIES[0].2;
    let wrong = body.replace("$DE[$DE.touches", "$DONOR[$DE.touches");
    let overwritten = body.replace("if ($DH &&", "$DH = $DONOR.touches; if ($DH &&");
    for variant in [wrong, overwritten] {
        assert_eq!(claims(&inline_touch_fixture(&variant), TOUCH), 0);
    }
}

#[test]
fn inline_touch_donor_force_bounds_and_swapped_axes_are_refused() {
    let body = INLINE_TOUCH_BODIES[0].2;
    for (before, after) in [
        ("e: $DJ.force", "e: $DONOR.force"),
        ("let $DK = $DJ.target", "let $DK = $DONOR.target"),
        ("$DJ.clientX - $DK.left", "$DJ.clientX - $DK.top"),
        ("Math.round($DJ.pageY)", "Math.round($DONOR.pageY)"),
    ] {
        assert!(body.contains(before));
        assert_eq!(
            claims(&inline_touch_fixture(&body.replace(before, after)), TOUCH),
            0,
            "{before}"
        );
    }
}

#[test]
fn inline_touch_wrong_clock_sink_and_disconnected_payload_are_refused() {
    let body = INLINE_TOUCH_BODIES[0].2;
    for (before, after) in [
        ("b: $KEY_5()", "b: $DE.timeStamp()"),
        ("b: $KEY_5()", "b: $KEY_5($DE)"),
        ("$KEY_6.put({", "$KEY_6.error({"),
        ("$KEY_6.put({", "let dead = ({"),
        ("$KEY_6.put({", "let localSink = {}; localSink.put({"),
    ] {
        assert!(body.contains(before));
        assert_eq!(
            claims(&inline_touch_fixture(&body.replace(before, after)), TOUCH),
            0,
            "{after}"
        );
    }
    let local = touch_payload_as_local(body).replace("$KEY_6.put(payload);", "");
    assert_eq!(claims(&inline_touch_fixture(&local), TOUCH), 0);
}

#[test]
fn valid_touch_neighbors_cannot_supply_a_broken_functions_sample_proof() {
    let mut bodies = String::new();
    for (i, &(_, _, body)) in INLINE_TOUCH_BODIES.iter().enumerate() {
        let body = if i == 0 {
            body.replace("e: $DJ.force", "e: $DONOR.force")
        } else {
            body.to_string()
        };
        bodies.push_str(&body);
        bodies.push('\n');
    }
    let source = inline_touch_fixture(&bodies);
    assert_eq!(claims(&source, TOUCH), 5);
    let result = name_threads(&source, true).unwrap();
    assert!(result.source.contains("function $THREAD_0_6176("));
    for &(_, original, _) in &INLINE_TOUCH_BODIES[1..] {
        assert!(!result.source.contains(&format!("function {original}(")));
    }
}

#[test]
fn inline_touch_alpha_names_and_put_call_preserve_meaningful_captures() {
    let source = inline_touch_fixture(INLINE_TOUCH_BODIES[0].2)
        .replace("$KEY_6.put({", "$KEY_6.put.call($KEY_6, {")
        .replace("$KEY_6", "$KEY_touchSamples")
        .replace("$KEY_5", "$KEY_sampleClock")
        .replace("$KEY_24", "$KEY_sampleErrors")
        .replace("$DE", "inputEvent")
        .replace("$DJ", "touchPoint");
    assert_eq!(claims(&source, TOUCH), 1);
    assert_eq!(applied(&source, TOUCH), 1);
    let result = name_threads(&source, true).unwrap();
    for key in ["$KEY_touchSamples", "$KEY_sampleClock", "$KEY_sampleErrors"] {
        assert!(result.source.contains(key));
    }
}

#[test]
fn inline_touch_donor_metadata_and_shadowed_element_are_refused() {
    let body = INLINE_TOUCH_BODIES[0].2;
    for (before, after) in [
        ("$DE instanceof UIEvent", "$DONOR instanceof UIEvent"),
        ("!!$DE.isTrusted", "!!$DONOR.isTrusted"),
        ("try {", "try { let Element = $DE.constructor;"),
    ] {
        assert_eq!(
            claims(&inline_touch_fixture(&body.replace(before, after)), TOUCH),
            0
        );
    }
}

#[test]
fn exact_inline_touch_fixtures_preserve_the_earlier_anchor_proof() {
    // The original fixture clock aliases a known function. The exact direct
    // inline bodies therefore satisfy PASS7 before the metadata fallback runs.
    // Counting only fallback claims in that context incorrectly reports zero
    // semantic recognitions even though the earlier rule proves the same role.
    let bodies = INLINE_TOUCH_BODIES
        .iter()
        .map(|(_, _, body)| *body)
        .collect::<Vec<_>>()
        .join("\n");
    let source =
        inline_touch_fixture(&bodies).replace("let $KEY_5=clock.bind(null);", "let $KEY_5=clock;");
    let result = name_threads(&source, true).unwrap();
    let anchor_claims = result
        .report
        .function_roles
        .iter()
        .filter(|role| {
            role.candidates.iter().any(|candidate| candidate == TOUCH)
                && role
                    .evaluators
                    .iter()
                    .any(|evaluator| evaluator == "events_touch_sample")
        })
        .count();
    assert_eq!(anchor_claims, 6, "outcomes: {:?}", result.report.outcomes);
    assert_eq!(
        result
            .report
            .outcomes
            .iter()
            .filter(|outcome| {
                outcome.applied
                    && outcome.evaluator.as_deref() == Some("events_touch_sample")
                    && outcome.role_name.as_deref() == Some(TOUCH)
            })
            .count(),
        6,
        "outcomes: {:?}",
        result.report.outcomes
    );
    for &(_, original, _) in INLINE_TOUCH_BODIES {
        assert!(!result.source.contains(&format!("function {original}(")));
    }
}
