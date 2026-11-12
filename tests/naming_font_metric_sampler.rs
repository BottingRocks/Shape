//! Focused static coverage for the standalone canvas font-metric sampler.
//!
//! Positives cover direct context/push calls and a holder-call context with a push alias.
//! The carrier provides a captured queue factory and unique broad parent. Refusals mutate
//! scoped identity, flow, returns, or effects while keeping valid JavaScript so parser
//! failures cannot make a negative pass.
use shape::naming::name_threads;

const EVALUATOR: &str = "shared_font_metric_canvas_sampler";
const NAME: &str = "emitFontMetricsProbe";
const TARGET: &str = "$THREAD_9_26141";

const COLLECTOR_EVALUATOR: &str = "shared_font_metric_collector";
const COLLECTOR_NAME: &str = "collectFontMetrics";
const COLLECTOR_TARGET: &str = "$THREAD_3_8222";
const COLLECTOR_FACTORY: &str = "$THREAD_5_12882";
const COLLECTOR_FACTORY_NAME: &str = "createCollectFontMetrics";

const POSITIVE: &str = r##"function bootstrap(){
let $KEY_220 = function($task, $items) { return { onEmpty: function($done){}, start: function(){}, empty: function(){} }; };
let $KEY_286 = function($seed) { return function() { return $seed; }; };
let $KEY_58 = function($keystream, $value, $header, $cursor) { return $cursor; };
function $THREAD_9_26141($BS, $BT, $BU) {
			let $KEY_4 = $BU;
			let $KEY_7, $KEY_9, $KEY_10;
			let $KEY_2 = [];
			let $BV = $BT.createElement("canvas");
			$KEY_7 = $BV && $BV.getContext && $BV.getContext("2d");
			let $KEY_0;
			if ($KEY_7 && typeof $KEY_7.measureText === "function") {
				$KEY_7.textBaseline = "top";
				$KEY_7.font = "50px  sans-serif";
				$KEY_10 = $KEY_7.measureText("0-_{w.").width;
				$KEY_7.font = "50px  serif";
				$KEY_9 = $KEY_7.measureText("0-_{w.").width;
				$KEY_0 = $KEY_220(($BW, $BX) => {
					let $BY = $KEY_2;
					$KEY_7.font = "50px " + ($BW == null ? "" : "'" + $BW + "', ") + " sans-serif";
					let $BZ;
					if ($KEY_7.measureText("0-_{w.").width !== $KEY_10) {
						$BZ = true;
					} else {
						$KEY_7.font = "50px " + ($BW == null ? "" : "'" + $BW + "', ") + " serif";
						$BZ = $KEY_7.measureText("0-_{w.").width !== $KEY_9;
					}
					$BY.push($BZ);
					$BX();
				}, $BS.fonts);
				$KEY_0.onEmpty(() => {
					$KEY_4($KEY_2);
				});
				$KEY_0.start();
			}
			function $THREAD_0_12819() {
				if ($KEY_0) {
					$KEY_0.empty();
				}
			}
			return $THREAD_0_12819;
		}
return $THREAD_9_26141;
}
bootstrap();"##;

const DONOR: &str = r##"function bootstrap(){
let $KEY_187 = function($task, $items) { return { onEmpty: function($done){}, start: function(){}, empty: function(){} }; };
let $KEY_286 = function($seed) { return function() { return $seed; }; };
let $KEY_58 = function($keystream, $value, $header, $cursor) { return $cursor; };
function $THREAD_3_55913($BV, $BW, $BX) {
			let $KEY_0 = $BX;
			let $KEY_1, $KEY_2, $KEY_5, $KEY_6, $KEY_7;
			$KEY_6 = [];
			let $BY = $BW.createElement("canvas");
			let $BZ = $BY && $BY.getContext;
			if ($BZ) {
				$BY = [$BY, $BY.getContext];
				$BZ = $BY[1].call($BY[0], "2d");
			}
			$KEY_2 = $BZ;
			if ($BZ && typeof $KEY_2.measureText === "function") {
				$KEY_2.textBaseline = "top";
				$KEY_2.font = "50px  sans-serif";
				$KEY_5 = $KEY_2.measureText("0-_{w.").width;
				$KEY_2.font = "50px  serif";
				$KEY_1 = $KEY_2.measureText("0-_{w.").width;
				$KEY_7 = $KEY_187(($CA, $CB) => {
					let $CC = $KEY_6;
					let $CD = $KEY_6.push;
					$KEY_2.font = "50px " + ($CA == null ? "" : "'" + $CA + "', ") + " sans-serif";
					let $CE;
					if ($KEY_2.measureText("0-_{w.").width !== $KEY_5) {
						$CE = true;
					} else {
						$KEY_2.font = "50px " + ($CA == null ? "" : "'" + $CA + "', ") + " serif";
						$CE = $KEY_2.measureText("0-_{w.").width !== $KEY_1;
					}
					$CD.call($CC, $CE);
					$CB();
				}, $BV.fonts);
				$KEY_7.onEmpty(() => {
					$KEY_0($KEY_6);
				});
				$KEY_7.start();
			}
			function $THREAD_8_61641() {
				if ($KEY_7) {
					$KEY_7.empty();
				}
			}
			return $THREAD_8_61641;
		}
return $THREAD_3_55913;
}
bootstrap();"##;

fn outcome(source: &str, target: &str) -> shape::naming::ThreadName {
    let result = name_threads(source, true).unwrap_or_else(|error| panic!("fixture did not analyze: {error}"));
    result.report.outcomes.into_iter().find(|outcome| outcome.binding.name == target).unwrap_or_else(|| panic!("target {target} missing"))
}

fn claimed(source: &str, target: &str) -> bool {
    let result = outcome(source, target);
    result.applied && result.proposed_name.as_deref() == Some(NAME) && result.evaluator.as_deref() == Some(EVALUATOR)
}

fn refused(source: &str, target: &str) {
    assert!(!claimed(source, target), "unexpected sampler claim for {target}");
}

fn replace(source: &str, from: &str, to: &str) -> String {
    assert!(source.contains(from), "missing fixture replacement: {from}");
    source.replace(from, to)
}

#[test]
fn exact_cached_body_is_named() {
    assert!(claimed(POSITIVE, TARGET));
}

#[test]
fn holder_and_push_alias_donor_is_named() {
    assert!(claimed(DONOR, "$THREAD_3_55913"));
}

#[test]
fn holder_context_and_push_alias_require_exact_identity_and_arity() {
    refused(
        &replace(DONOR, "$BY[1].call($BY[0], \"2d\")", "$BY[1].call($BY[0], \"2d\", \"extra\")"),
        "$THREAD_3_55913",
    );
    refused(
        &replace(DONOR, "$BY[1].call($BY[0], \"2d\")", "$BY[1].call($BY[0], \"webgl\")"),
        "$THREAD_3_55913",
    );
    refused(
        &replace(DONOR, "$CD.call($CC, $CE);", "$CD.call($CC, true);"),
        "$THREAD_3_55913",
    );
    for mutation in [
        "let $CD; if ($CA) { $CD = $KEY_6.push; }",
        "let $CD = $KEY_6.push; $CD = other;",
        "let $CD = $KEY_6.push; const invoke = $CD; $CD = other;",
        "let $CD = $KEY_6.push; const invoke = $CA ? $CD : other;",
    ] {
        let source = replace(DONOR, "let $CD = $KEY_6.push;", mutation);
        let source = if mutation.contains("const invoke") { source.replace("$CD.call", "invoke.call") } else { source };
        refused(&source, "$THREAD_3_55913");
    }
    refused(&replace(DONOR, "$CD.call($CC, $CE);", "$CD.call([], $CE);"), "$THREAD_3_55913");
    refused(&replace(DONOR, "$CD.call($CC, $CE);", "$CD.call($CC, $CE, true);"), "$THREAD_3_55913");
    refused(&replace(DONOR, "$CE = $KEY_2.measureText(\"0-_{w.\").width !== $KEY_1;", "$CE = $CA;"), "$THREAD_3_55913");
    refused(&replace(DONOR, "$CD.call($CC, $CE);", "$CE = false; $CD.call($CC, $CE);"), "$THREAD_3_55913");
    refused(&replace(DONOR, "$CE = $KEY_2.measureText(\"0-_{w.\").width !== $KEY_1;", "$CE = $KEY_2.measureText(\"0-_{w.\").width !== $KEY_5;"), "$THREAD_3_55913");
    refused(&replace(DONOR, "$CA == null ? \"\"", "$CA == null ? \"wrong\""), "$THREAD_3_55913");
}

#[test]
fn alpha_renaming_is_not_identity_evidence() {
    let alpha = POSITIVE
        .replace("$THREAD_9_26141", "$THREAD_901_888_3")
        .replace("$THREAD_0_12819", "$THREAD_902_888_4")
        .replace("$KEY_220", "$A_queue")
        .replace("$KEY_2", "$A_values")
        .replace("$KEY_7", "$A_context")
        .replace("$BS", "$A_config")
        .replace("$BT", "$A_document")
        .replace("$BU", "$A_sink");
    assert!(claimed(&alpha, "$THREAD_901_888_3"));
}

#[test]
fn identity_flow_return_and_effect_negatives_are_refused() {
    refused(&replace(POSITIVE, "getContext(\"2d\")", "getContext(\"webgl\")"), TARGET);
    refused(&replace(POSITIVE, "$BS.fonts", "$BS.otherFonts"), TARGET);
    refused(&replace(POSITIVE, "$KEY_0.empty();", "$KEY_2.empty();"), TARGET);
    refused(&replace(POSITIVE, "$BX();", "$BX($BW);"), TARGET);
    refused(&replace(POSITIVE, "$KEY_0.start();", "$KEY_2.start();"), TARGET);
}

#[test]
fn forwarding_wrapper_is_not_annex_borrowed() {
    let wrapped = POSITIVE.replace(
        "function $THREAD_9_26141(",
        "function $THREAD_4_777(){ return $THREAD_9_26141; }\nfunction $THREAD_9_26141(",
    );
    refused(&wrapped, "$THREAD_4_777");
    assert!(claimed(&wrapped, TARGET));
}

#[test]
fn existing_specific_label_is_preserved() {
    let readable = POSITIVE.replace("$THREAD_9_26141", "emitFontMetricsProbe");
    let result = name_threads(&readable, true).unwrap();
    assert!(result.source.contains("function emitFontMetricsProbe("));
    assert!(!result.source.contains("function emitFontMetricsProbe2("));
}

/// The proven sampler `$THREAD_9_26141` is reached through `$KEY_261`. The factory wrapper
/// `$THREAD_5_12882` contains the six-formal collector `$THREAD_3_8222`. Only the collector
/// is independently named; the factory inherits through the existing returned-function
/// rule.
const COLLECTOR: &str = r##"function bootstrap(){
let $KEY_220 = function($task, $items) { return { onEmpty: function($done){}, start: function(){}, empty: function(){} }; };
let $KEY_286 = function($seed) { return function() { return $seed; }; };
let $KEY_58 = function($keystream, $value, $header, $cursor) { return $cursor; };
function $THREAD_9_26141($BS, $BT, $BU) {
			let $KEY_4 = $BU;
			let $KEY_7, $KEY_9, $KEY_10;
			let $KEY_2 = [];
			let $BV = $BT.createElement("canvas");
			$KEY_7 = $BV && $BV.getContext && $BV.getContext("2d");
			let $KEY_0;
			if ($KEY_7 && typeof $KEY_7.measureText === "function") {
				$KEY_7.textBaseline = "top";
				$KEY_7.font = "50px  sans-serif";
				$KEY_10 = $KEY_7.measureText("0-_{w.").width;
				$KEY_7.font = "50px  serif";
				$KEY_9 = $KEY_7.measureText("0-_{w.").width;
				$KEY_0 = $KEY_220(($BW, $BX) => {
					let $BY = $KEY_2;
					$KEY_7.font = "50px " + ($BW == null ? "" : "'" + $BW + "', ") + " sans-serif";
					let $BZ;
					if ($KEY_7.measureText("0-_{w.").width !== $KEY_10) {
						$BZ = true;
					} else {
						$KEY_7.font = "50px " + ($BW == null ? "" : "'" + $BW + "', ") + " serif";
						$BZ = $KEY_7.measureText("0-_{w.").width !== $KEY_9;
					}
					$BY.push($BZ);
					$BX();
				}, $BS.fonts);
				$KEY_0.onEmpty(() => {
					$KEY_4($KEY_2);
				});
				$KEY_0.start();
			}
			function $THREAD_0_12819() {
				if ($KEY_0) {
					$KEY_0.empty();
				}
			}
			return $THREAD_0_12819;
		}
let $KEY_261 = $THREAD_9_26141;
		function $THREAD_5_12882($BS) {
			let $KEY_0 = $BS;
			function $THREAD_3_8222($BT, $BU, $BV, $BW, $BX, $BY) {
				let $KEY_8 = $BT;
				let $KEY_6 = $BU.global.document;
				let $KEY_4 = null;
				$BW(() => {
					$KEY_4 = $KEY_261($KEY_0, $KEY_6, $BZ => {
						let $KEY_5 = Math.random() * 4294967296 | 0;
						let $CA = [$KEY_5 >>> 24, $KEY_5 >> 16 & 255, $KEY_5 >> 8 & 255, $KEY_5 & 255];
						let $KEY_6 = $KEY_286($KEY_5 ^ 3367138309);
						let i = 0;
						let $CB = 0;
						let $CC = 0;
						let $CD = $KEY_58($KEY_6, $BZ.length, $CA, $KEY_58($KEY_6, $KEY_0.version, $CA, 3));
						for (; i < $BZ.length; i++) {
							let $CE = $BZ[i] << $CB;
							++$CB;
							if ($CB === 8) {
								$CA[++$CD] = ($CC | $CE) ^ $KEY_6();
								$CB = 0;
								$CC = 0;
							} else {
								$CC = $CC | $CE;
							}
						}
						if ($CB !== 0) {
							$CA[$CD + 1] = $CC ^ $KEY_6();
						}
						$KEY_8.next([[], $CA]);
						$KEY_8.complete();
					});
				}, true);
				$BY(() => {
					if (typeof $KEY_4 === "function") {
						$KEY_4();
					}
				});
			}
			return $THREAD_3_8222;
		}
return $THREAD_5_12882;
}
bootstrap();"##;

fn claimed_as(source: &str, target: &str, name: &str, evaluator: &str) -> bool {
    let result = outcome(source, target);
    result.applied && result.proposed_name.as_deref() == Some(name) && result.evaluator.as_deref() == Some(evaluator)
}

fn collector_claimed(source: &str, target: &str) -> bool {
    claimed_as(source, target, COLLECTOR_NAME, COLLECTOR_EVALUATOR)
}

fn collector_refused(source: &str, target: &str) {
    assert!(!collector_claimed(source, target), "unexpected collector claim for {target}");
}

#[test]
fn collector_body_is_named() {
    assert!(collector_claimed(COLLECTOR, COLLECTOR_TARGET));
}

#[test]
fn collector_factory_inherits_the_create_name() {
    assert!(collector_claimed(COLLECTOR, COLLECTOR_TARGET));
    assert!(claimed_as(COLLECTOR, COLLECTOR_FACTORY, COLLECTOR_FACTORY_NAME, "returned_function_factory"));
    let result = name_threads(COLLECTOR, true).unwrap();
    assert!(result.source.contains("function collectFontMetrics("));
    assert!(result.source.contains("function createCollectFontMetrics("));
}

#[test]
fn collector_alpha_renaming_is_not_identity_evidence() {
    let alpha = COLLECTOR
        .replace("$THREAD_3_8222", "$THREAD_903_777_5")
        .replace("$THREAD_5_12882", "$THREAD_904_777_6")
        .replace("$KEY_261", "$A_probe")
        .replace("$KEY_286", "$A_prng")
        .replace("$KEY_58", "$A_writer")
        .replace("$KEY_8", "$A_observer")
        .replace("$KEY_6", "$A_reader")
        .replace("$KEY_4", "$A_cleanup")
        .replace("$KEY_5", "$A_seed")
        .replace("$CA", "$A_header")
        .replace("$CB", "$A_bit")
        .replace("$CC", "$A_accumulator")
        .replace("$CD", "$A_byte")
        .replace("$CE", "$A_chunk")
        .replace("$BZ", "$A_result");
    assert!(collector_claimed(&alpha, "$THREAD_903_777_5"));
    assert!(claimed_as(&alpha, "$THREAD_904_777_6", COLLECTOR_FACTORY_NAME, "returned_function_factory"));
}

#[test]
fn collector_donor_helper_declarations_and_alias_chain_are_named() {
    let donor = COLLECTOR
        .replace(
            "let $KEY_286 = function($seed) { return function() { return $seed; }; };",
            "function $KEY_286($seed) { return function() { return $seed; }; }",
        )
        .replace(
            "let $KEY_58 = function($keystream, $value, $header, $cursor) { return $cursor; };",
            "function $KEY_58($keystream, $value, $header, $cursor) { return $cursor; }",
        )
        .replace(
            "let $KEY_261 = $THREAD_9_26141;",
            "let $KEY_270 = $THREAD_9_26141;\nlet $KEY_261 = $KEY_270;",
        );
    assert!(collector_claimed(&donor, COLLECTOR_TARGET));
}

#[test]
fn collector_wrong_config_callee_version_state_length_cursor_flush_observer_start_cleanup_extra_refuse() {
    collector_refused(
        &replace(COLLECTOR, "$KEY_261($KEY_0, $KEY_6, $BZ =>", "$KEY_220($KEY_0, $KEY_6, $BZ =>"),
        COLLECTOR_TARGET,
    );
    collector_refused(
        &replace(COLLECTOR, "$KEY_261($KEY_0, $KEY_6, $BZ =>", "$KEY_261($KEY_8, $KEY_6, $BZ =>"),
        COLLECTOR_TARGET,
    );
    collector_refused(&replace(COLLECTOR, "$KEY_0.version", "$KEY_0.size"), COLLECTOR_TARGET);
    collector_refused(
        &replace(COLLECTOR, "Math.random() * 4294967296 | 0", "Date.now() * 4294967296 | 0"),
        COLLECTOR_TARGET,
    );
    collector_refused(&replace(COLLECTOR, "$BZ.length", "$BZ.count"), COLLECTOR_TARGET);
    collector_refused(&replace(COLLECTOR, "$CA[++$CD]", "$CA[$CD]"), COLLECTOR_TARGET);
    collector_refused(&replace(COLLECTOR, "if ($CB === 8)", "if ($CB === 7)"), COLLECTOR_TARGET);
    collector_refused(
        &replace(COLLECTOR, "$KEY_8.next([[], $CA])", "$KEY_8.push([[], $CA])"),
        COLLECTOR_TARGET,
    );
    collector_refused(&replace(COLLECTOR, "$BW(() => {", "$BV(() => {"), COLLECTOR_TARGET);
    collector_refused(&replace(COLLECTOR, "$KEY_4();", "$KEY_0();"), COLLECTOR_TARGET);
    collector_refused(&replace(COLLECTOR, "$BY(() => {", "$BX.sync(); $BY(() => {"), COLLECTOR_TARGET);
    // The wrong-callee and wrong-start near misses must still leave the sampler
    // claim intact, so the refusals above are not parser failures.
    assert!(claimed(COLLECTOR, TARGET));
}

#[test]
fn push_alias_split_initialization_and_immutable_chains_are_named() {
    for origin in [
        "let $CD; $CD = $KEY_6.push;",
        "const push = $KEY_6.push; const $CD = push;",
        "let push; push = $KEY_6.push; const $CD = push;",
    ] {
        assert!(claimed(&replace(DONOR, "let $CD = $KEY_6.push;", origin), "$THREAD_3_55913"));
    }
    let late = replace(DONOR, "let $CD = $KEY_6.push;", "let $CD;");
    refused(&replace(&late, "$CD.call($CC, $CE);", "$CD.call($CC, $CE); $CD = $KEY_6.push;"), "$THREAD_3_55913");
}

// Exact cached sampler and collector bodies, with inert captured helper bindings.
const DIRECT_CONTEXT: &str = r##"function bootstrap(){
let $KEY_231 = function(){}; let $KEY_274 = function(){}; let $KEY_6 = function(){}; let $KEY_76;
function $THREAD_0_105187($BO, $BP, $BQ) {
			let $KEY_8 = $BQ;
			let $KEY_5 = [];
			let $BR = $BP.createElement("canvas");
			let $BS = $BR && $BR.getContext;
			if ($BS) {
				$BR = [$BR, $BR.getContext];
				$BS = $BR[1].call($BR[0], "2d");
			}
			let $KEY_6 = $BS;
			let $KEY_3;
			if ($BS && typeof $KEY_6.measureText === "function") {
				$KEY_6.textBaseline = "top";
				$KEY_6.font = "50px  sans-serif";
				let $KEY_4 = $KEY_6.measureText("0-_{w.").width;
				$KEY_6.font = "50px  serif";
				let $KEY_2 = $KEY_6.measureText("0-_{w.").width;
				$KEY_3 = $KEY_231(($BT, $BU) => {
					let $BV = $KEY_5;
					$KEY_6.font = "50px " + ($BT == null ? "" : "'" + $BT + "', ") + " sans-serif";
					let $BW;
					if ($KEY_6.measureText("0-_{w.").width !== $KEY_4) {
						$BW = true;
					} else {
						$KEY_6.font = "50px " + ($BT == null ? "" : "'" + $BT + "', ") + " serif";
						$BW = $KEY_6.measureText("0-_{w.").width !== $KEY_2;
					}
					$BV.push($BW);
					$BU();
				}, $BO.fonts);
				$KEY_3.onEmpty(() => {
					$KEY_8($KEY_5);
				});
				$KEY_3.start();
			}
			function $THREAD_6_91002() {
				if ($KEY_3) {
					$KEY_3.empty();
				}
			}
			return $THREAD_6_91002;
		}

$KEY_76 = $THREAD_0_105187;
function $THREAD_9_52043($BO) {
			let $KEY_0 = $BO;
			function $THREAD_3_46400($BP, $BQ, $BR, $BS, $BT, $BU) {
				let $KEY_9 = $BP;
				let $KEY_2 = $BQ.global.document;
				let $KEY_3 = null;
				$BS(() => {
					$KEY_3 = $KEY_76($KEY_0, $KEY_2, $BV => {
						let $BW = Math.random() * 4294967296;
						let $BX = [($BW | 0) >>> 24, ($BW | 0) >> 16 & 255, ($BW | 0) >> 8 & 255, ($BW | 0) & 255];
						let $KEY_1 = $KEY_274(($BW | 0) ^ 3940662672);
						let i = 0;
						let $BY = $KEY_6($KEY_1, $BV.length, $BX, 3);
						let $BZ = 0;
						let $CA = 0;
						for (; i < $BV.length; i++) {
							let $CB = $BV[i] << $CA;
							++$CA;
							if ($CA === 8) {
								$BX[++$BY] = ($BZ | $CB) ^ $KEY_1();
								$BZ = 0;
								$CA = 0;
							} else {
								$BZ = $BZ | $CB;
							}
						}
						if ($CA !== 0) {
							$BX[++$BY] = $BZ ^ $KEY_1();
						}
						$KEY_6($KEY_1, $KEY_0.version, $BX, $BY);
						$KEY_9.next([[], $BX]);
						$KEY_9.complete();
					});
				}, true);
				$BU(() => {
					if (typeof $KEY_3 === "function") {
						$KEY_3();
					}
				});
			}
			return $THREAD_3_46400;
		}

return $THREAD_9_52043;
}
bootstrap();"##;

#[test]
fn direct_context_sampler_collector_and_factory_are_named() {
    assert!(claimed(DIRECT_CONTEXT, "$THREAD_0_105187"));
    assert!(collector_claimed(DIRECT_CONTEXT, "$THREAD_3_46400"));
    assert!(claimed_as(DIRECT_CONTEXT, "$THREAD_9_52043", COLLECTOR_FACTORY_NAME, "returned_function_factory"));
}

// Exact cached sampler and collector bodies, with inert captured helper bindings.
const ALTERNATE_CONTEXT: &str = r##"function bootstrap(){
let $KEY_187 = function(){}; let $KEY_67 = function(){}; let $KEY_291 = function(){};
function $THREAD_3_55913($BV, $BW, $BX) {
			let $KEY_0 = $BX;
			let $KEY_1, $KEY_2, $KEY_5, $KEY_6, $KEY_7;
			$KEY_6 = [];
			let $BY = $BW.createElement("canvas");
			let $BZ = $BY && $BY.getContext;
			if ($BZ) {
				$BY = [$BY, $BY.getContext];
				$BZ = $BY[1].call($BY[0], "2d");
			}
			$KEY_2 = $BZ;
			if ($BZ && typeof $KEY_2.measureText === "function") {
				$KEY_2.textBaseline = "top";
				$KEY_2.font = "50px  sans-serif";
				$KEY_5 = $KEY_2.measureText("0-_{w.").width;
				$KEY_2.font = "50px  serif";
				$KEY_1 = $KEY_2.measureText("0-_{w.").width;
				$KEY_7 = $KEY_187(($CA, $CB) => {
					let $CC = $KEY_6;
					let $CD = $KEY_6.push;
					$KEY_2.font = "50px " + ($CA == null ? "" : "'" + $CA + "', ") + " sans-serif";
					let $CE;
					if ($KEY_2.measureText("0-_{w.").width !== $KEY_5) {
						$CE = true;
					} else {
						$KEY_2.font = "50px " + ($CA == null ? "" : "'" + $CA + "', ") + " serif";
						$CE = $KEY_2.measureText("0-_{w.").width !== $KEY_1;
					}
					$CD.call($CC, $CE);
					$CB();
				}, $BV.fonts);
				$KEY_7.onEmpty(() => {
					$KEY_0($KEY_6);
				});
				$KEY_7.start();
			}
			function $THREAD_8_61641() {
				if ($KEY_7) {
					$KEY_7.empty();
				}
			}
			return $THREAD_8_61641;
		}

let $KEY_51 = $THREAD_3_55913;
function $THREAD_2_118987($BV) {
			let $KEY_0 = $BV;
			function $THREAD_2_66725($BW, $BX, $BY, $BZ, $CA, $CB) {
				let $KEY_6 = $BW;
				let $KEY_2 = $BX.global.document;
				let $KEY_3 = null;
				$BZ(() => {
					$KEY_3 = $KEY_51($KEY_0, $KEY_2, $CC => {
						let $CD = Math.random() * 4294967296;
						let $CE = [($CD | 0) >>> 24, ($CD | 0) >> 16 & 255, ($CD | 0) >> 8 & 255, ($CD | 0) & 255];
						let $KEY_1 = $KEY_67(($CD | 0) ^ 3002247363);
						let i = 0;
						let $CF = 0;
						let $CG = $KEY_291($KEY_1, $CC.length, $CE, $KEY_291($KEY_1, $KEY_0.version, $CE, 3));
						let $CH = 0;
						for (; i < $CC.length; i++) {
							$CF = $CF | $CC[i] << $CH;
							++$CH;
							if ($CH === 8) {
								$CE[++$CG] = $CF ^ $KEY_1();
								$CF = 0;
								$CH = 0;
							}
						}
						if ($CH !== 0) {
							$CE[$CG + 1] = $CF ^ $KEY_1();
						}
						$KEY_6.next([[], $CE]);
						$KEY_6.complete();
					});
				}, true);
				$CB(() => {
					if (typeof $KEY_3 === "function") {
						$KEY_3();
					}
				});
			}
			return $THREAD_2_66725;
		}

return $THREAD_2_118987;
}
bootstrap();"##;

#[test]
fn alternate_context_sampler_collector_and_factory_are_named() {
    assert!(claimed(ALTERNATE_CONTEXT, "$THREAD_3_55913"));
    assert!(collector_claimed(ALTERNATE_CONTEXT, "$THREAD_2_66725"));
    assert!(claimed_as(ALTERNATE_CONTEXT, "$THREAD_2_118987", COLLECTOR_FACTORY_NAME, "returned_function_factory"));
}

#[test]
fn cached_collector_variants_refuse_wrong_packing_and_version() {
    for (source, target, from, to) in [
        (ALTERNATE_CONTEXT, "$THREAD_2_66725", "$CF = $CF | $CC[i] << $CH", "$CF = $CF ^ $CC[i] << $CH"),
        (ALTERNATE_CONTEXT, "$THREAD_2_66725", "$CF ^ $KEY_1()", "$CH ^ $KEY_1()"),
        (ALTERNATE_CONTEXT, "$THREAD_2_66725", "($CD | 0) ^ 3002247363", "($CD | 1) ^ 3002247363"),
        (DIRECT_CONTEXT, "$THREAD_3_46400", "$KEY_0.version", "$KEY_0.size"),
        (DIRECT_CONTEXT, "$THREAD_3_46400", "$BX[++$BY] = $BZ", "$BX[$BY] = $BZ"),
        (DIRECT_CONTEXT, "$THREAD_3_46400", "$KEY_6($KEY_1, $KEY_0.version, $BX, $BY)", "$KEY_6($KEY_1, $KEY_0.version, $BX, 3)"),
    ] { collector_refused(&replace(source, from, to), target); }
}

/// Every isolation refusal first proves grammar and target identity. Neither a
/// parser failure nor an unrelated candidate/evaluator may satisfy the test.
fn iso_refused(source: &str, target: &str, name: &str, evaluator: &str) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::unambiguous()).parse();
    assert!(parsed.errors.is_empty(), "invalid counterexample: {:?}", parsed.errors);
    let result = outcome(source, target);
    assert_ne!(result.evaluator.as_deref(), Some(evaluator), "family evaluator accepted {target}");
    assert!(!(result.applied && result.proposed_name.as_deref() == Some(name)), "role applied to {target}");
}

fn iso_sampler_refused(source: &str, target: &str) {
    iso_refused(source, target, NAME, EVALUATOR);
}

#[test]
fn protocol_arity_reported_completion_spread_is_refused() {
    iso_sampler_refused(&replace(POSITIVE, "$BX();", "$BX(...evil());"), TARGET);
}

#[test]
fn protocol_arity_reported_create_element_spread_is_refused() {
    iso_sampler_refused(&replace(POSITIVE, "$BT.createElement(\"canvas\")", "$BT.createElement(...[\"div\"], \"canvas\")"), TARGET);
}

#[test]
fn protocol_arity_all_sampler_protocol_arities_refuse_spread() {
    for (from, to) in [
        ("$BT.createElement(\"canvas\")", "$BT.createElement(\"canvas\", ...[])"),
        ("$BV.getContext(\"2d\")", "$BV.getContext(...[\"webgl\"], \"2d\")"),
        ("$KEY_7.measureText(\"0-_{w.\")", "$KEY_7.measureText(...[\"wrong\"], \"0-_{w.\")"),
        ("$BY.push($BZ)", "$BY.push(...[false], $BZ)"),
        ("$BX();", "$BX(...[]);"),
        ("}, $BS.fonts);", "}, ...[], $BS.fonts);"),
        ("$KEY_4($KEY_2)", "$KEY_4(...[], $KEY_2)"),
        ("$KEY_0.onEmpty(() =>", "$KEY_0.onEmpty(...[], () =>"),
        ("$KEY_0.start()", "$KEY_0.start(...[])"),
        ("$KEY_0.empty()", "$KEY_0.empty(...[])"),
    ] {
        iso_sampler_refused(&replace(POSITIVE, from, to), TARGET);
    }
    for (from, to) in [
        ("$BY[1].call($BY[0], \"2d\")", "$BY[1].call(...[], $BY[0], \"2d\")"),
        ("$CD.call($CC, $CE)", "$CD.call($CC, ...[], $CE)"),
    ] {
        iso_sampler_refused(&replace(DONOR, from, to), "$THREAD_3_55913");
    }
}

#[test]
fn protocol_arity_collector_arities_refuse_spread() {
    for (from, to) in [
        ("$BW(() =>", "$BW(...[], () =>"),
        ("$KEY_261($KEY_0, $KEY_6,", "$KEY_261(...[], $KEY_0, $KEY_6,"),
        ("$KEY_286($KEY_5 ^ 3367138309)", "$KEY_286(...[], $KEY_5 ^ 3367138309)"),
        ("$KEY_58($KEY_6, $BZ.length", "$KEY_58(...[], $KEY_6, $BZ.length"),
        ("$KEY_58($KEY_6, $KEY_0.version", "$KEY_58(...[], $KEY_6, $KEY_0.version"),
        ("$KEY_8.next([[], $CA])", "$KEY_8.next(...[], [[], $CA])"),
        ("$KEY_8.complete()", "$KEY_8.complete(...[])"),
        ("$BY(() =>", "$BY(...[], () =>"),
        ("$KEY_4();", "$KEY_4(...[]);"),
    ] {
        iso_refused(&replace(COLLECTOR, from, to), COLLECTOR_TARGET, COLLECTOR_NAME, COLLECTOR_EVALUATOR);
    }
    let source = replace(COLLECTOR, "$BX();", "$BX(...evil());");
    iso_sampler_refused(&source, TARGET);
    iso_refused(&source, COLLECTOR_TARGET, COLLECTOR_NAME, COLLECTOR_EVALUATOR);
    let result = outcome(&source, COLLECTOR_FACTORY);
    assert!(!(result.applied && result.proposed_name.as_deref() == Some(COLLECTOR_FACTORY_NAME)));
}

#[test]
fn baseline_flow_reported_baseline_overwrite_is_refused() {
    iso_sampler_refused(&replace(POSITIVE,
        "$KEY_10 = $KEY_7.measureText(\"0-_{w.\").width;",
        "$KEY_10 = $KEY_7.measureText(\"0-_{w.\").width; $KEY_10 = 0;"), TARGET);
}

#[test]
fn baseline_flow_reported_dead_font_write_is_refused() {
    iso_sampler_refused(&replace(POSITIVE,
        "$KEY_7.font = \"50px  sans-serif\";",
        "if (false) { $KEY_7.font = \"50px  sans-serif\"; }"), TARGET);
}

#[test]
fn baseline_flow_baseline_origins_and_invalidating_writes_are_refused() {
    for (from, to) in [
        ("$KEY_9 = $KEY_7.measureText(\"0-_{w.\").width;", "$KEY_9 = $KEY_7.measureText(\"0-_{w.\").width; $KEY_9 = 0;"),
        ("$KEY_10 = $KEY_7.measureText(\"0-_{w.\").width;", "if (false) { $KEY_10 = $KEY_7.measureText(\"0-_{w.\").width; }"),
        ("$KEY_7.font = \"50px  sans-serif\";", "$KEY_7.font = \"50px  sans-serif\"; $KEY_7.font = \"wrong\";"),
        ("$KEY_7.font = \"50px  sans-serif\";", "$KEY_7.font = \"50px  sans-serif\"; $KEY_7[\"font\"] = \"wrong\";"),
        ("$KEY_7.font = \"50px  sans-serif\";", "$KEY_7.font = \"50px  sans-serif\"; ++$KEY_7.font;"),
        ("$KEY_7.font = \"50px  sans-serif\";", "$KEY_7.font = \"50px  sans-serif\"; delete $KEY_7.font;"),
        ("$KEY_7.font = \"50px  sans-serif\";", "$KEY_7.font = \"50px  sans-serif\"; const alias = $KEY_7; alias.font = \"wrong\";"),

        ("$KEY_7.font = \"50px  sans-serif\";", "$KEY_7.font = \"50px  sans-serif\"; if (false) { $KEY_7.font = \"50px  serif\"; }"),
        ("$KEY_10 = $KEY_7.measureText(\"0-_{w.\").width;", "$KEY_10 = $KEY_7.measureText(\"0-_{w.\").width; if ($BS) { $KEY_10 = 0; }"),
        ("$KEY_10 = $KEY_7.measureText(\"0-_{w.\").width;", "$KEY_10 = $KEY_7.measureText(\"0-_{w.\").width; function invalidate(){ $KEY_10 = 0; }"),
    ] {
        iso_sampler_refused(&replace(POSITIVE, from, to), TARGET);
    }
}

#[test]
fn baseline_flow_context_reaching_values_and_guards_are_required() {
    for (from, to) in [
        ("$KEY_7 = $BV && $BV.getContext && $BV.getContext(\"2d\");", "$KEY_7 = ($BV && $BV.getContext && $BV.getContext(\"2d\"), other);"),
        ("$KEY_7 = $BV && $BV.getContext && $BV.getContext(\"2d\");", "$KEY_7 = $BV && $BV.getContext && $BV.getContext(\"2d\"); $KEY_7 = other;"),
        ("$KEY_7 = $BV && $BV.getContext && $BV.getContext(\"2d\");", "if (false) { $KEY_7 = $BV && $BV.getContext && $BV.getContext(\"2d\"); }"),
        ("if ($KEY_7 && typeof $KEY_7.measureText", "if (false && typeof $KEY_7.measureText"),
        ("$KEY_7.font = \"50px  sans-serif\";", "let other = $BV.getContext(\"2d\"); other.font = \"50px  sans-serif\";"),
    ] {
        iso_sampler_refused(&replace(POSITIVE, from, to), TARGET);
    }
    let wrong = replace(POSITIVE, "$KEY_7.textBaseline = \"top\";", "let other = $BV.getContext(\"2d\"); $KEY_7.textBaseline = \"top\";");
    let wrong = replace(&wrong, "$KEY_9 = $KEY_7.measureText", "$KEY_9 = other.measureText");
    iso_sampler_refused(&wrong, TARGET);
    for (from, to) in [
        ("if ($BZ) {", "if (false) {"),
        ("$KEY_2 = $BZ;", "$BZ = other; $KEY_2 = $BZ;"),
        ("$KEY_2 = $BZ;", "$KEY_2 = ($BZ, other);"),
        ("$KEY_2 = $BZ;", "$KEY_2 = $BZ; $KEY_2 = other;"),
        ("$BZ = $BY[1].call($BY[0], \"2d\");", "$BZ = ($BY[1].call($BY[0], \"2d\"), other);"),
    ] {
        iso_sampler_refused(&replace(DONOR, from, to), "$THREAD_3_55913");
    }
}

#[test]
fn baseline_flow_nested_and_sibling_donors_cannot_supply_baselines() {
    for replacement in [
        "function donor(){ $KEY_7.font = \"50px  sans-serif\"; }",
        "(() => { $KEY_7.font = \"50px  sans-serif\"; });",
    ] {
        iso_sampler_refused(&replace(POSITIVE, "$KEY_7.font = \"50px  sans-serif\";", replacement), TARGET);
    }
    let source = replace(POSITIVE, "$KEY_7.font = \"50px  sans-serif\";", "");
    let source = replace(&source, "return $THREAD_9_26141;", "function donor(){ $KEY_7.font = \"50px  sans-serif\"; } return $THREAD_9_26141;");
    iso_sampler_refused(&source, TARGET);
    let source = replace(POSITIVE, "$KEY_10 = $KEY_7.measureText(\"0-_{w.\").width;", "function donor(){ $KEY_10 = $KEY_7.measureText(\"0-_{w.\").width; }");
    iso_sampler_refused(&source, TARGET);
}

#[test]
fn baseline_flow_supported_context_and_baseline_aliases_remain_named() {
    let source = replace(POSITIVE, "$KEY_7 = $BV && $BV.getContext && $BV.getContext(\"2d\");",
        "let context = $BV && $BV.getContext && $BV.getContext(\"2d\"); $KEY_7 = context;");
    assert!(claimed(&source, TARGET));
    let source = replace(POSITIVE, "$KEY_10 = $KEY_7.measureText(\"0-_{w.\").width;",
        "$KEY_10 = $KEY_7.measureText(\"0-_{w.\").width; const sans = $KEY_10;");
    let source = replace(&source, ".width !== $KEY_10", ".width !== sans");
    assert!(claimed(&source, TARGET));
}

const HOLDER_PUSH_ALIAS: &str =
    include_str!("fixtures/program/font-metrics-holder-push-alias.js");
const DIRECT_CONTEXT_SOURCE: &str = include_str!("fixtures/program/font-metrics-cached-baseline.js");
const ALTERNATE_CONTEXT_SOURCE: &str = include_str!("fixtures/program/font-metrics-cached-alternate.js");

#[test]
fn lowered_real_font_installations_preserve_distinct_roles() {
    for (source, sampler, collector, factory) in [
        (
            HOLDER_PUSH_ALIAS,
            "$THREAD_9_43168",
            "$THREAD_2_31406",
            "$THREAD_2_40476",
        ),
        (
            DIRECT_CONTEXT_SOURCE,
            "$THREAD_5_30885",
            "$THREAD_4_7170",
            "$THREAD_1_119865",
        ),
        (
            ALTERNATE_CONTEXT_SOURCE,
            "$THREAD_7_111058",
            "$THREAD_7_139974",
            "$THREAD_9_22617",
        ),
    ] {
        assert!(claimed(source, sampler), "sampler {sampler}");
        assert!(
            collector_claimed(source, collector),
            "collector {collector}"
        );
        assert!(
            claimed_as(
                source,
                factory,
                COLLECTOR_FACTORY_NAME,
                "returned_function_factory"
            ),
            "factory {factory}"
        );
    }
}

#[test]
fn lowered_split_cleanup_origins_and_callback_contracts_refuse_near_misses() {
    for (from, to) in [
        (
            "$KEY_5 = $BW.global.document",
            "$KEY_5 = $BX.global.document",
        ),
        ("$KEY_7 = null;", "$KEY_7 = null; $KEY_7 = other;"),
        (
            "$KEY_5 = $BW.global.document;",
            "$KEY_5 = $BW.global.document; $KEY_5 = other;",
        ),
        ("$KEY_7 = null;", ""),
        ("$KEY_336($KEY_0, $KEY_5,", "$KEY_336($KEY_1, $KEY_5,"),
        ("$KEY_336($KEY_0, $KEY_5,", "$KEY_390($KEY_0, $KEY_5,"),
        ("$KEY_1.next([[], $CD])", "$BW.next([[], $CD])"),
        ("$KEY_7();", "$KEY_0();"),
        ("$CA(() =>", "$BZ(() =>"),
        ("$KEY_11();", "$CC();"),
        (
            "$KEY_390($KEY_11, $CB.length",
            "$KEY_344($KEY_11, $CB.length",
        ),
    ] {
        iso_refused(
            &replace(DIRECT_CONTEXT_SOURCE, from, to),
            "$THREAD_4_7170",
            COLLECTOR_NAME,
            COLLECTOR_EVALUATOR,
        );
    }
}

#[test]
fn lowered_reverse_traversal_and_pair_holder_require_exact_origins() {
    for (from, to) in [
        ("while ($BX - 1 >= 0)", "while ($BX >= 0)"),
        ("$BT[$BX - 1]", "$BT[$BX]"),
        ("--$BX;", "++$BX;"),
        ("let $BX = $BT.length", "let $BX = other.length"),
        ("let $BV = [];", "let $BV = []; $BV.push(false);"),
        ("let $BV = [];", "let $BV = []; $BV = other;"),
        ("$KEY_6.next([$BV, $BW])", "$KEY_6.next([other, $BW])"),
        ("$BW[$BY + 1]", "$BW[$BY]"),
        ("$KEY_5();", "$BU();"),
    ] {
        iso_refused(
            &replace(ALTERNATE_CONTEXT_SOURCE, from, to),
            "$THREAD_7_139974",
            COLLECTOR_NAME,
            COLLECTOR_EVALUATOR,
        );
    }
}

#[test]
fn lowered_trailing_version_requires_advanced_partial_cursor() {
    for (from, to) in [
        ("$BV[++$BY] = $BX ^", "$BV[$BY + 1] = $BX ^"),
        (
            "$KEY_90($KEY_5, $KEY_0.version, $BV, $BY)",
            "$KEY_90($KEY_5, $KEY_0.version, $BV, 3)",
        ),
        ("$KEY_0.version", "$KEY_0.fonts"),
    ] {
        iso_refused(
            &replace(HOLDER_PUSH_ALIAS, from, to),
            "$THREAD_2_31406",
            COLLECTOR_NAME,
            COLLECTOR_EVALUATOR,
        );
    }
}

#[test]
fn lowered_single_seed_to_prng_transition_is_branch_complete() {
    let source = replace(
        DIRECT_CONTEXT_SOURCE,
        "let $KEY_11 = $KEY_344(($CC | 0) ^ 75749485);",
        "$CC = $KEY_344(($CC | 0) ^ 75749485);",
    );
    let source = replace(&source, "$KEY_11", "$CC");
    assert!(collector_claimed(&source, "$THREAD_4_7170"));
    for (from, to) in [
        (
            "$CC = $KEY_344(($CC | 0) ^ 75749485);",
            "if (other) { $CC = $KEY_344(($CC | 0) ^ 75749485); }",
        ),
        (
            "$CC = $KEY_344(($CC | 0) ^ 75749485);",
            "$CC = $KEY_344(($CC | 0) ^ 75749485); $CC = other;",
        ),
        (
            "$CC = $KEY_344(($CC | 0) ^ 75749485);",
            "$CC = $KEY_344((other | 0) ^ 75749485);",
        ),
    ] {
        iso_refused(
            &replace(&source, from, to),
            "$THREAD_4_7170",
            COLLECTOR_NAME,
            COLLECTOR_EVALUATOR,
        );
    }
}

#[test]
fn lowered_real_sampler_contracts_cannot_borrow_donor_evidence() {
    for (from, to) in [
        ("$CB.call($CA, $CC)", "$CB.call(other, $CC)"),
        ("let $CB = $KEY_4.push", "let $CB = other.push"),
        (
            "$KEY_2.textBaseline = \"top\"",
            "$KEY_2.textBaseline = \"bottom\"",
        ),
        (
            "$KEY_2.measureText(\"0-_{w.\").width !== $KEY_0",
            "other.measureText(\"0-_{w.\").width !== $KEY_0",
        ),
        ("}, $BU.fonts)", "}, other.fonts)"),
        ("$KEY_5($KEY_4)", "$KEY_5(other)"),
    ] {
        let source = replace(DIRECT_CONTEXT_SOURCE, from, to);
        iso_sampler_refused(&source, "$THREAD_5_30885");
        iso_refused(
            &source,
            "$THREAD_4_7170",
            COLLECTOR_NAME,
            COLLECTOR_EVALUATOR,
        );
    }
}

#[test]
fn lowered_extension_preserves_original_nine_font_names() {
    for (source, sampler, collector, factory) in [
        (COLLECTOR, TARGET, COLLECTOR_TARGET, COLLECTOR_FACTORY),
        (DIRECT_CONTEXT, "$THREAD_0_105187", "$THREAD_3_46400", "$THREAD_9_52043"),
        (ALTERNATE_CONTEXT, "$THREAD_3_55913", "$THREAD_2_66725", "$THREAD_2_118987"),
    ] {
        assert!(claimed(source, sampler));
        assert!(collector_claimed(source, collector));
        assert!(claimed_as(source, factory, COLLECTOR_FACTORY_NAME, "returned_function_factory"));
    }
}

#[test]
fn text_alignment_text_baseline_requires_owned_reaching_top_alignment() {
    for replacement in [
        "$KEY_2.textBaseline = \"bottom\";",
        "",
        "other.textBaseline = \"top\";",
        "if (false) { $KEY_2.textBaseline = \"top\"; }",
        "function donor(){ $KEY_2.textBaseline = \"top\"; }",
        "$KEY_2.textBaseline = \"top\"; $KEY_2.textBaseline = \"bottom\";",
        "$KEY_2.textBaseline = \"top\"; $KEY_2[\"textBaseline\"] = \"bottom\";",
        "$KEY_2.textBaseline = \"top\"; const alias = $KEY_2; alias.textBaseline = \"bottom\";",
        "$KEY_2.textBaseline = \"top\"; ++$KEY_2.textBaseline;",
        "$KEY_2.textBaseline = \"top\"; delete $KEY_2.textBaseline;",
    ] {
        let source = replace(DIRECT_CONTEXT_SOURCE, "$KEY_2.textBaseline = \"top\";", replacement);
        eprintln!("alignment replacement: {replacement}");
        iso_sampler_refused(&source, "$THREAD_5_30885");
        iso_refused(&source, "$THREAD_4_7170", COLLECTOR_NAME, COLLECTOR_EVALUATOR);
    }
    let source = replace(DIRECT_CONTEXT_SOURCE, "$KEY_2.textBaseline = \"top\";",
        "$KEY_2.textBaseline = \"bottom\"; $KEY_2.textBaseline = \"top\";");
    assert!(claimed(&source, "$THREAD_5_30885"));
}
