//! Single writes prove identity only after a supported runtime origin. Fixtures are
//! analyzed statically.
use oxc_allocator::Allocator;
use shape::analysis;
use shape::naming::{name_threads, NamedThreads};

const COLLECTOR_TARGET: &str = "$THREAD_3_8222";
const COLLECTOR_ROLE: &str = "collectFontMetrics";
const COLLECTOR_EVALUATOR: &str = "shared_font_metric_collector";
const ALIAS: &str = "let $KEY_261 = $THREAD_9_26141;";

fn analyze(source: &str, target: &str) -> NamedThreads {
    let allocator = Allocator::default();
    let parsed = analysis::parse(&allocator, source);
    assert!(parsed.errors.is_empty() && !parsed.panicked, "invalid fixture: {:?}", parsed.errors);
    let result = name_threads(source, true).expect("fixture must analyze");
    assert_eq!(result.report.outcomes.iter().filter(|o| o.binding.name == target).count(), 1,
        "expected exactly the original candidate {target}");
    result
}

fn assert_role(source: &str, target: &str, role: &str, evaluator: &str, expected: bool) {
    let result = analyze(source, target);
    let outcome = result.report.outcomes.iter().find(|o| o.binding.name == target).unwrap();
    if expected {
        assert!(outcome.applied, "role did not apply for {target}: {outcome:?}");
        assert_eq!(outcome.proposed_name.as_deref(), Some(role));
        assert_eq!(outcome.evaluator.as_deref(), Some(evaluator));
    } else {
        assert_ne!(outcome.evaluator.as_deref(), Some(evaluator), "unsupported evaluator: {outcome:?}");
        assert!(!(outcome.applied && (outcome.proposed_name.as_deref() == Some(role)
            || outcome.role_name.as_deref() == Some(role))), "unsupported applied role: {outcome:?}");
        let function = result.report.function_roles.iter()
            .find(|f| f.current_name.as_deref() == Some(target)).expect("candidate role record missing");
        assert!(!function.evaluators.iter().any(|e| e == evaluator), "unsupported candidate: {function:?}");
    }
}

fn collector(source: &str, expected: bool) {
    assert_role(source, COLLECTOR_TARGET, COLLECTOR_ROLE, COLLECTOR_EVALUATOR, expected);
    // The donor is still a real supported sampler. Refusal must isolate the
    // relationship, not disable the entire family or erase donor evidence.
    assert_role(source, "$THREAD_9_26141", "emitFontMetricsProbe", "shared_font_metric_canvas_sampler", true);
}

fn change(source: &str, from: &str, to: &str) -> String {
    assert_eq!(source.matches(from).count(), 1, "replacement must address exactly one origin");
    source.replacen(from, to, 1)
}

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

const EQUALITY: &str = r#"function main(){
let tags={UINT14_BASE:0,UINT16:1,UINT24:2,UINT32:3,UINT64:4,NINT4_BASE:5,FLOAT32:6,DOUBLE64:7,STR5_BASE:8,STRREF:9,STRLUT:10,ARRAY8:11,BARRAY8:12,MAP_:13,BMAP_:14,EXTENSION:15};
function $THREAD_1_10(a,b){let other=b;if(a.length===b.length){return a.every((item,index)=>other[index]===item);}else{return false;}}
let compare=$THREAD_1_10;let target=[];
function $THREAD_2_20(value){return compare(value,target);}
return [$THREAD_1_10,$THREAD_2_20];}main();"#;

#[test]
fn callable_origin_exact_reported_dead_branch_assignment_refuses() {
    collector(&change(COLLECTOR, ALIAS,
        "let $KEY_261; if (false) { $KEY_261 = $THREAD_9_26141; }"), false);
}

#[test]
fn callable_origin_nested_sibling_and_loop_donors_refuse() {
    for origin in [
        "let $KEY_261; function neverCalled(){ $KEY_261 = $THREAD_9_26141; }",
        "let $KEY_261; const neverCalled = () => { $KEY_261 = $THREAD_9_26141; };",
        "let $KEY_261; while (false) { $KEY_261 = $THREAD_9_26141; }",
        "let $KEY_261; for (let i = 0; i < 0; i++) { $KEY_261 = $THREAD_9_26141; }",
        "let $KEY_261; if (flag) { $KEY_261 = $THREAD_9_26141; }",
    ] {
        collector(&change(COLLECTOR, ALIAS, origin), false);
    }
    let sibling = change(COLLECTOR, ALIAS, "let $KEY_261;");
    let sibling = change(&sibling, "return $THREAD_3_8222;",
        "$KEY_261 = $THREAD_9_26141; return $THREAD_3_8222;");
    // The factory's write is in a different invocation from the bootstrap's
    // alias declaration; it is not a direct same-body origin.
    collector(&sibling, false);
}

#[test]
fn callable_origin_initializer_under_unexecuted_control_flow_refuses() {
    for origin in [
        "if (false) { var $KEY_261 = $THREAD_9_26141; }",
        "for (; false;) { var $KEY_261 = $THREAD_9_26141; }",
    ] {
        collector(&change(COLLECTOR, ALIAS, origin), false);
    }
}

#[test]
fn callable_origin_after_exposure_assignment_and_initializer_refuse() {
    for origin in ["let $KEY_261;", ""] {
        let source = change(COLLECTOR, ALIAS, origin);
        let late = if origin.is_empty() { ALIAS } else { "$KEY_261 = $THREAD_9_26141;" };
        let source = change(&source, "return $THREAD_5_12882;",
            &format!("return $THREAD_5_12882; {late}"));
        collector(&source, false);
    }
}

#[test]
fn callable_origin_earlier_initializer_assignment_chain_and_hoisted_direct_callee_survive() {
    collector(COLLECTOR, true);
    for origin in [
        "let $KEY_261; $KEY_261 = $THREAD_9_26141;",
        "var $KEY_261; $KEY_261 = $THREAD_9_26141;",
        "const first = $THREAD_9_26141; let $KEY_261; $KEY_261 = first;",
    ] {
        collector(&change(COLLECTOR, ALIAS, origin), true);
    }
    let direct = change(COLLECTOR, "$KEY_261($KEY_0, $KEY_6, $BZ =>",
        "$THREAD_9_26141($KEY_0, $KEY_6, $BZ =>");
    collector(&direct, true);
}

#[test]
fn callable_origin_enclosing_alias_setup_after_collector_declaration_survives() {
    for setup in [ALIAS, "let $KEY_261; $KEY_261 = $THREAD_9_26141;"] {
        let source = change(COLLECTOR, ALIAS, "");
        let source = change(&source, "return $THREAD_5_12882;",
            &format!("{setup} return $THREAD_5_12882;"));
        collector(&source, true);
    }
}

#[test]
fn callable_origin_role_resolver_rejects_dead_nested_loop_and_late_aliases() {
    for origin in [
        "let compare; if (false) { compare=$THREAD_1_10; }",
        "let compare; function unused(){ compare=$THREAD_1_10; }",
        "let compare; while (false) { compare=$THREAD_1_10; }",
    ] {
        let source = change(EQUALITY, "let compare=$THREAD_1_10;", origin);
        assert_role(&source, "$THREAD_2_20", "matchesArrayContents", "shared_scoped_role", false);
    }
    let source = change(EQUALITY, "let compare=$THREAD_1_10;", "let compare;");
    let source = change(&source, "return [$THREAD_1_10,$THREAD_2_20];",
        "return [$THREAD_1_10,$THREAD_2_20]; compare=$THREAD_1_10;");
    assert_role(&source, "$THREAD_2_20", "matchesArrayContents", "shared_scoped_role", false);
}

#[test]
fn callable_origin_role_resolver_accepts_setup_after_hoisted_wrapper_declaration() {
    let source = change(EQUALITY, "let compare=$THREAD_1_10;", "");
    let source = change(&source, "return [$THREAD_1_10,$THREAD_2_20];",
        "let compare; compare=$THREAD_1_10; return [$THREAD_1_10,$THREAD_2_20];");
    assert_role(&source, "$THREAD_2_20", "matchesArrayContents", "shared_scoped_role", true);
}

const COUNT: &str = "function main(){let recur;function $THREAD_1_2(obj){if(obj==null){return 0;}else{return Object.getOwnPropertyNames(obj).length+recur(Object.getPrototypeOf(obj));}}recur=$THREAD_1_2;return recur;}main();";

#[test]
fn callable_origin_recursive_same_body_setup_survives_and_early_call_refuses() {
    assert_role(COUNT, "$THREAD_1_2", "countOwnPropertiesAlongPrototypeChain", "shared_scoped_role", true);
    let early = change(COUNT, "recur=$THREAD_1_2;", "$THREAD_1_2({});recur=$THREAD_1_2;");
    assert_role(&early, "$THREAD_1_2", "countOwnPropertiesAlongPrototypeChain", "shared_scoped_role", false);
}

#[test]
fn callable_origin_transitive_early_exposure_refuses() {
    let source = change(COLLECTOR, ALIAS, "");
    let source = change(&source, "return $THREAD_5_12882;",
        "function expose(){ return $THREAD_5_12882; } expose(); let $KEY_261 = $THREAD_9_26141; return expose;");
    // A later return of expose cannot hide its earlier invocation, even when
    // the actual collector/factory references sit inside hoisted declarations.
    collector(&source, false);
}

fn deferred_equality_expression(before_setup: &str) -> String {
    let source = change(EQUALITY, "let compare=$THREAD_1_10;", "");
    change(&source, "function $THREAD_2_20(value){return compare(value,target);}",
        &format!("const $THREAD_2_20 = function(value){{return compare(value,target);}}; {before_setup} let compare=$THREAD_1_10;"))
}

#[test]
fn deferred_expression_exact_deferred_function_expression_retains_role() {
    // Closure creation precedes compare,
    // but its only exposure (the returned functions) follows setup.
    assert_role(&deferred_equality_expression(""), "$THREAD_2_20",
        "matchesArrayContents", "shared_scoped_role", true);
}

#[test]
fn deferred_expression_expression_early_call_and_transitive_escape_refuse() {
    for before_setup in [
        "$THREAD_2_20([]);",
        "publish($THREAD_2_20);",
        "function expose(){return $THREAD_2_20;} publish(expose());",
        "const expose = function(){return $THREAD_2_20;}; publish(expose());",
        "const invoke = function(){return $THREAD_2_20([]);}; invoke();",
    ] {
        assert_role(&deferred_equality_expression(before_setup), "$THREAD_2_20",
            "matchesArrayContents", "shared_scoped_role", false);
    }
    let deferred = deferred_equality_expression("");
    let transitive = change(&deferred, "return [$THREAD_1_10,$THREAD_2_20];",
        "const expose = function(){return $THREAD_2_20;}; return [$THREAD_1_10,expose];");
    assert_role(&transitive, "$THREAD_2_20", "matchesArrayContents", "shared_scoped_role", true);
}

#[test]
fn deferred_expression_iife_before_setup_refuses() {
    // Keep the exact wrapper target; an IIFE calls it before compare exists.
    assert_role(&deferred_equality_expression("(function(){return $THREAD_2_20([]);})();"),
        "$THREAD_2_20", "matchesArrayContents", "shared_scoped_role", false);
}

#[test]
fn deferred_expression_recursive_expression_setup_survives_and_early_exposure_refuses() {
    let source = change(COUNT,
        "function $THREAD_1_2(obj){if(obj==null){return 0;}else{return Object.getOwnPropertyNames(obj).length+recur(Object.getPrototypeOf(obj));}}",
        "const $THREAD_1_2 = function(obj){if(obj==null){return 0;}else{return Object.getOwnPropertyNames(obj).length+recur(Object.getPrototypeOf(obj));}};");
    assert_role(&source, "$THREAD_1_2", "countOwnPropertiesAlongPrototypeChain", "shared_scoped_role", true);
    for early in ["$THREAD_1_2({});", "publish($THREAD_1_2);"] {
        let source = change(&source, "recur=$THREAD_1_2;", &format!("{early} recur=$THREAD_1_2;"));
        assert_role(&source, "$THREAD_1_2", "countOwnPropertiesAlongPrototypeChain", "shared_scoped_role", false);
    }
}

// Original cached wrapper and hasher bodies; minimal enclosing setup preserves
// their scoped identities and the early value-capture / late helper timing.

const ASYNC_DISPATCH_BS: &str = r##"function bootstrap(){
let $D = (function(sourceString) {
			return sourceString.charCodeAt();
		});
let $KEY_222 = $D;
let $KEY_299;
let $KEY_354 = function(){ return function(){ return Date.now(); }; };
let $KEY_24, $KEY_432;
let $KEY_265;
function $THREAD_1_15592($BS, $BT, $BU, $BV) {
			if (typeof $BS === "string") {
				$KEY_24($BS, $BT, $BU, $BV);
			} else {
				let $BW = $KEY_265;
				if ($BW) {
					$BW = ArrayBuffer.isView($BS);
				}
				if ($BW) {
					$KEY_432($BS, $BT, $BU, $BV);
				} else {
					$BV(new TypeError());
				}
			}
		}
let $KEY_186 = $THREAD_1_15592;
function $THREAD_5_80456($BS, $BT, $BU, $BV) {
			let $KEY_1 = $BU;
			let $KEY_3 = $BV;
			let $KEY_4;
			let $KEY_5 = $BS;
			let $KEY_6;
			let $KEY_0 = $BS.length >> 1 << 1;
			$KEY_6 = 0;
			$KEY_4 = 0;
			new $KEY_299($BT).schedule(() => {
				try {
					let $BW = $KEY_6 + 400 < $KEY_0 ? $KEY_6 + 400 : $KEY_0;
					for (; $KEY_6 < $BW; $KEY_6 = $KEY_6 + 2) {
						$KEY_4 = ($KEY_4 << 5) - $KEY_4 + ($KEY_222($KEY_5[$KEY_6]) << 16 | $KEY_222($KEY_5[$KEY_6 + 1])) | 0;
					}
					if ($KEY_6 < $KEY_0) {
						return true;
					} else {
						if ($KEY_0 !== $KEY_5.length) {
							$KEY_4 = ($KEY_4 << 5) - $KEY_4 + ($KEY_222($KEY_5[$KEY_0]) << 16) | 0;
						}
						$KEY_1($KEY_4);
						return false;
					}
				} catch (e) {
					$KEY_3(e);
					return false;
				}
			});
		}
$KEY_24 = $THREAD_5_80456;
function $THREAD_0_92523($BS, $BT, $BU, $BV) {
			let $KEY_0 = $BV;
			let $KEY_1, $KEY_2;
			let $KEY_3 = $BU;
			let $KEY_7, $KEY_8, $BW;
			if ($BS.byteLength % 4 === 0) {
				$BW = $BS.buffer;
			} else {
				$BW = new ArrayBuffer($BS.byteLength + (4 - $BS.byteLength % 4));
				new Int8Array($BW).set(new Int8Array($BS.buffer));
			}
			$KEY_2 = new Int32Array($BW);
			$KEY_1 = $KEY_2.length;
			$KEY_7 = 0;
			$KEY_8 = 0;
			new $KEY_299($BT).schedule(() => {
				try {
					let $BX = $KEY_7 + 400 < $KEY_1 ? $KEY_7 + 400 : $KEY_1;
					while ($KEY_7 < $BX) {
						$KEY_8 = ($KEY_8 << 5) - $KEY_8 + $KEY_2[$KEY_7] | 0;
						++$KEY_7;
					}
					if ($KEY_7 < $KEY_1) {
						return true;
					} else {
						$KEY_3($KEY_8);
						return false;
					}
				} catch (e) {
					$KEY_0(e);
					return false;
				}
			});
		}
$KEY_432 = $THREAD_0_92523;
class $THREAD_5_33946 {
			constructor($BS) {
				this.targetBatchTimeMs = $BS;
				this._internalNow = $KEY_354(typeof global !== "undefined" && global != null && global.global === global ? global : typeof self !== "undefined" && self != null && self.self === self ? self : typeof window !== "undefined" && window != null && window.window === window ? window : void 0);
			}
			now() {
				return this._internalNow();
			}
			schedule($BS) {
				setTimeout(this._process.bind(this, $BS), 0);
			}
			_process($BS) {
				let $BT = this.targetBatchTimeMs;
				let $BU = this.now();
				let $BV = 0;
				let i = 0;
				let $BW, $BX;
				for (;; i = $BX) {
					let $BY = this.now();
					$BW = $BS();
					let $BZ = this.now();
					$BX = (i * $BV + ($BZ - $BY)) / ($BV + 1);
					++$BV;
					if (!($BZ + $BX < $BU + $BT ? $BW : $BZ + $BX < $BU + $BT)) break;
				}
				if ($BW) {
					setTimeout(this._process.bind(this, $BS), 0);
				}
			}
		}
$KEY_299 = $THREAD_5_33946;
$KEY_265 = typeof ArrayBuffer !== "undefined" && ArrayBuffer != null && typeof ArrayBuffer.isView === "function" && typeof Int32Array !== "undefined";
return $KEY_186;
}
bootstrap();"##;

#[test]
fn async_dispatch_preserves_bs_parameter_captures() {
    assert_role(ASYNC_DISPATCH_BS, "$THREAD_1_15592",
        "hashStringOrArrayBufferViewAsync", "shared_scoped_role", true);
}

const ASYNC_DISPATCH_BO: &str = r##"function bootstrap(){
let $D = (function(sourceString) {
			return sourceString.charCodeAt();
		});
let $KEY_108 = $D;
let $KEY_18;
let $KEY_248 = function(){ return function(){ return Date.now(); }; };
let $KEY_384, $KEY_251;
let $KEY_371;
function $THREAD_3_15987($BO, $BP, $BQ, $BR) {
			if (typeof $BO === "string") {
				$KEY_384($BO, $BP, $BQ, $BR);
			} else {
				let $BS = $KEY_371;
				if ($BS) {
					$BS = ArrayBuffer.isView($BO);
				}
				if ($BS) {
					$KEY_251($BO, $BP, $BQ, $BR);
				} else {
					$BR(new TypeError());
				}
			}
		}
let $KEY_72 = $THREAD_3_15987;
function $THREAD_7_38168($BO, $BP, $BQ, $BR) {
			let $KEY_0 = $BO;
			let $KEY_2 = $BR;
			let $KEY_5, $KEY_6;
			let $KEY_7 = $BQ;
			let $KEY_8;
			$KEY_5 = $BO.length >> 1 << 1;
			$KEY_8 = 0;
			$KEY_6 = 0;
			new $KEY_18($BP).schedule(() => {
				try {
					let $BS = $KEY_8 + 400 < $KEY_5 ? $KEY_8 + 400 : $KEY_5;
					for (; $KEY_8 < $BS; $KEY_8 = $KEY_8 + 2) {
						$KEY_6 = ($KEY_6 << 5) - $KEY_6 + ($KEY_108($KEY_0[$KEY_8]) << 16 | $KEY_108($KEY_0[$KEY_8 + 1])) | 0;
					}
					if ($KEY_8 < $KEY_5) {
						return true;
					} else {
						if ($KEY_5 !== $KEY_0.length) {
							$KEY_6 = ($KEY_6 << 5) - $KEY_6 + ($KEY_108($KEY_0[$KEY_5]) << 16) | 0;
						}
						$KEY_7($KEY_6);
						return false;
					}
				} catch (e) {
					$KEY_2(e);
					return false;
				}
			});
		}
$KEY_384 = $THREAD_7_38168;
function $THREAD_7_83987($BO, $BP, $BQ, $BR) {
			let $KEY_1 = $BR;
			let $KEY_2;
			let $KEY_3 = $BQ;
			let $KEY_4, $KEY_6, $BS;
			if ($BO.byteLength % 4 === 0) {
				$BS = $BO.buffer;
			} else {
				$BS = new ArrayBuffer($BO.byteLength + (4 - $BO.byteLength % 4));
				new Int8Array($BS).set(new Int8Array($BO.buffer));
			}
			$KEY_4 = new Int32Array($BS);
			$KEY_6 = $KEY_4.length;
			$KEY_2 = 0;
			let $KEY_0 = 0;
			new $KEY_18($BP).schedule(() => {
				try {
					const $BT = $KEY_2 + 400 < $KEY_6 ? $KEY_2 + 400 : $KEY_6;
					while ($KEY_2 < $BT) {
						$KEY_0 = ($KEY_0 << 5) - $KEY_0 + $KEY_4[$KEY_2] | 0;
						++$KEY_2;
					}
					if ($KEY_2 < $KEY_6) {
						return true;
					} else {
						$KEY_3($KEY_0);
						return false;
					}
				} catch (e) {
					$KEY_1(e);
					return false;
				}
			});
		}
$KEY_251 = $THREAD_7_83987;
class $THREAD_0_125272 {
			constructor($BO) {
				this.targetBatchTimeMs = $BO;
				this._internalNow = $KEY_248(typeof global !== "undefined" && global != null && global.global === global ? global : typeof self !== "undefined" && self != null && self.self === self ? self : typeof window !== "undefined" && window != null && window.window === window ? window : void 0);
			}
			now() {
				return this._internalNow();
			}
			schedule($BO) {
				setTimeout(this._process.bind(this, $BO), 0);
			}
			_process($BO) {
				let $BP = this.targetBatchTimeMs;
				let $BQ = this.now();
				let $BR = 0;
				let i = 0;
				let $BS, $BT;
				for (;; i = $BT) {
					let $BU = this.now();
					$BS = $BO();
					let $BV = this.now();
					$BT = (i * $BR + ($BV - $BU)) / ($BR + 1);
					++$BR;
					if (!($BV + $BT < $BQ + $BP ? $BS : $BV + $BT < $BQ + $BP)) break;
				}
				if ($BS) {
					setTimeout(this._process.bind(this, $BO), 0);
				}
			}
		}
$KEY_18 = $THREAD_0_125272;
$KEY_371 = typeof ArrayBuffer !== "undefined" && ArrayBuffer != null && typeof ArrayBuffer.isView === "function" && typeof Int32Array !== "undefined";
return $KEY_72;
}
bootstrap();"##;

#[test]
fn async_dispatch_preserves_bo_parameter_captures() {
    assert_role(ASYNC_DISPATCH_BO, "$THREAD_3_15987",
        "hashStringOrArrayBufferViewAsync", "shared_scoped_role", true);
}

const ASYNC_DISPATCH_BV: &str = r##"function bootstrap(){
let $E = (function(sourceString) {
			return sourceString.charCodeAt();
		});
let $KEY_329 = $E;
let $KEY_196;
let $KEY_98 = function(){ return function(){ return Date.now(); }; };
let $KEY_416, $KEY_34;
let $KEY_344;
function $THREAD_5_70176($BV, $BW, $BX, $BY) {
			if (typeof $BV === "string") {
				$KEY_416($BV, $BW, $BX, $BY);
			} else {
				let $BZ = $KEY_344;
				if ($BZ) {
					$BZ = ArrayBuffer.isView($BV);
				}
				if ($BZ) {
					$KEY_34($BV, $BW, $BX, $BY);
				} else {
					$BY(new TypeError());
				}
			}
		}
let $KEY_199 = $THREAD_5_70176;
function $THREAD_2_102569($BV, $BW, $BX, $BY) {
			let $KEY_0 = $BY;
			let $KEY_1, $KEY_2;
			let $KEY_3 = $BV;
			let $KEY_4 = $BX;
			let $KEY_5;
			$KEY_2 = $BV.length >> 1 << 1;
			$KEY_5 = 0;
			$KEY_1 = 0;
			new $KEY_196($BW).schedule(() => {
				try {
					let $BZ = $KEY_5 + 400 < $KEY_2 ? $KEY_5 + 400 : $KEY_2;
					for (; $KEY_5 < $BZ; $KEY_5 = $KEY_5 + 2) {
						$KEY_1 = ($KEY_1 << 5) - $KEY_1 + ($KEY_329($KEY_3[$KEY_5]) << 16 | $KEY_329($KEY_3[$KEY_5 + 1])) | 0;
					}
					if ($KEY_5 < $KEY_2) {
						return true;
					} else {
						if ($KEY_2 !== $KEY_3.length) {
							$KEY_1 = ($KEY_1 << 5) - $KEY_1 + ($KEY_329($KEY_3[$KEY_2]) << 16) | 0;
						}
						$KEY_4($KEY_1);
						return false;
					}
				} catch (e) {
					$KEY_0(e);
					return false;
				}
			});
		}
$KEY_416 = $THREAD_2_102569;
function $THREAD_8_32616($BV, $BW, $BX, $BY) {
			let $KEY_0 = $BY;
			let $KEY_3, $KEY_5;
			let $KEY_6 = $BX;
			let $KEY_7, $KEY_8, $BZ;
			if ($BV.byteLength % 4 === 0) {
				$BZ = $BV.buffer;
			} else {
				$BZ = new ArrayBuffer($BV.byteLength + (4 - $BV.byteLength % 4));
				new Int8Array($BZ).set(new Int8Array($BV.buffer));
			}
			$KEY_7 = new Int32Array($BZ);
			$KEY_8 = $KEY_7.length;
			$KEY_3 = 0;
			$KEY_5 = 0;
			new $KEY_196($BW).schedule(() => {
				try {
					let $CA = $KEY_3 + 400 < $KEY_8 ? $KEY_3 + 400 : $KEY_8;
					while ($KEY_3 < $CA) {
						$KEY_5 = ($KEY_5 << 5) - $KEY_5 + $KEY_7[$KEY_3] | 0;
						++$KEY_3;
					}
					if ($KEY_3 < $KEY_8) {
						return true;
					} else {
						$KEY_6($KEY_5);
						return false;
					}
				} catch (e) {
					$KEY_0(e);
					return false;
				}
			});
		}
$KEY_34 = $THREAD_8_32616;
class $THREAD_2_20825 {
			constructor($BV) {
				this.targetBatchTimeMs = $BV;
				this._internalNow = $KEY_98(typeof global !== "undefined" && global != null && global.global === global ? global : typeof self !== "undefined" && self != null && self.self === self ? self : typeof window !== "undefined" && window != null && window.window === window ? window : void 0);
			}
			now() {
				return this._internalNow();
			}
			schedule($BV) {
				setTimeout(this._process.bind(this, $BV), 0);
			}
			_process($BV) {
				let $BW = this.targetBatchTimeMs;
				let $BX = this.now();
				let $BY = 0;
				let i = 0;
				let $BZ, $CA;
				for (;; i = $CA) {
					let $CB = this.now();
					$BZ = $BV();
					let $CC = this.now();
					$BY = ($BY * i + ($CC - $CB)) / (i + 1);
					$CA = Number(i) + 1;
					if (!($CC + $BY < $BX + $BW ? $BZ : $CC + $BY < $BX + $BW)) break;
				}
				if ($BZ) {
					setTimeout(this._process.bind(this, $BV), 0);
				}
			}
		}
$KEY_196 = $THREAD_2_20825;
$KEY_344 = typeof ArrayBuffer !== "undefined" && ArrayBuffer != null && typeof ArrayBuffer.isView === "function" && typeof Int32Array !== "undefined";
return $KEY_199;
}
bootstrap();"##;

#[test]
fn async_dispatch_preserves_bv_parameter_captures() {
    assert_role(ASYNC_DISPATCH_BV, "$THREAD_5_70176",
        "hashStringOrArrayBufferViewAsync", "shared_scoped_role", true);
}

#[test]
fn iso_preserve_async_capture_early_calls_escapes_and_stale_aliases_refuse() {
    for early in [
        "$THREAD_1_15592('',45,done,fail);",
        "publish($THREAD_1_15592);",
        "$KEY_186('',45,done,fail);",
        "publish($KEY_186);",
        "const again=$KEY_186; publish(again);",
        "function expose(){return $KEY_186;} publish(expose());",
        "const expose=function(){return $KEY_186;}; publish(expose());",
    ] {
        let source = change(ASYNC_DISPATCH_BS, "$KEY_24 = $THREAD_5_80456;",
            &format!("{early} $KEY_24 = $THREAD_5_80456;"));
        assert_role(&source, "$THREAD_1_15592", "hashStringOrArrayBufferViewAsync",
            "shared_scoped_role", false);
    }
    // A var read before alias initialization captures undefined, even though
    // the captured value escapes only after both helpers have been assigned.
    let stale = change(ASYNC_DISPATCH_BS, "let $KEY_186 = $THREAD_1_15592;",
        "var stale=$KEY_186; var $KEY_186 = $THREAD_1_15592;");
    let stale = change(&stale, "return $KEY_186;", "return stale;");
    assert_role(&stale, "$THREAD_1_15592", "hashStringOrArrayBufferViewAsync",
        "shared_scoped_role", false);
    for setup in [
        "let $KEY_186; if(false){$KEY_186=$THREAD_1_15592;}",
        "let $KEY_186=$THREAD_1_15592; $KEY_186=other;",
    ] {
        let source = change(ASYNC_DISPATCH_BS, "let $KEY_186 = $THREAD_1_15592;", setup);
        assert_role(&source, "$THREAD_1_15592", "hashStringOrArrayBufferViewAsync",
            "shared_scoped_role", false);
    }
}

fn async_uncalled_outbound_capture() -> String {
    change(ASYNC_DISPATCH_BS, "let $KEY_186 = $THREAD_1_15592;",
        "let $KEY_186; function unused(){$KEY_186=$THREAD_1_15592;}")
}

#[test]
fn iso_adjudicate_uncalled_outbound_capture_retains_independent_wrapper() {
    // KEY186 is an outbound capture, not either consumed helper callee.
    // The wrapper still dispatches through directly initialized KEY24/KEY432.
    // As in collector(), refuse an unsupported relationship without erasing
    // the independently proven donor's body role.
    assert_role(&async_uncalled_outbound_capture(), "$THREAD_1_15592",
        "hashStringOrArrayBufferViewAsync", "shared_scoped_role", true);
}

#[test]
fn iso_adjudicate_uncalled_outbound_capture_refuses_returned_factory() {
    let factory_role = "createHashStringOrArrayBufferViewAsync";
    // bootstrap is readable, so it has no generated-name outcome. Inspect
    // public role metadata too: absence of an applied rename is insufficient.
    // The initialized fixture is a positive control for this exact edge.
    for (source, expected) in [
        (ASYNC_DISPATCH_BS.to_string(), true),
        (async_uncalled_outbound_capture(), false),
    ] {
        let result = analyze(&source, "$THREAD_1_15592");
        let factories: Vec<_> = result.report.function_roles.iter()
            .filter(|f| f.current_name.as_deref() == Some("bootstrap")).collect();
        assert_eq!(factories.len(), 1, "expected exactly the returning bootstrap");
        let factory = factories[0];
        if expected {
            assert!(factory.candidates.iter().any(|r| r == factory_role),
                "initialized return edge lost its role: {factory:?}");
            assert!(factory.evaluators.iter().any(|e| e == "returned_function_factory"),
                "initialized return edge lost its evaluator: {factory:?}");
        } else {
            // unused() never runs: KEY186 is undefined when bootstrap returns.
            // No caller/return relationship to the named wrapper is justified.
            assert!(!factory.candidates.iter().any(|r|
                r == factory_role || r == "hashStringOrArrayBufferViewAsync"),
                "dead outbound alias supplied a factory role: {factory:?}");
            assert!(!factory.evaluators.iter().any(|e| e == "returned_function_factory"),
                "dead outbound alias supplied a return edge: {factory:?}");
        }
    }
}

#[test]
fn iso_adjudicate_stale_consumed_helper_alias_refuses() {
    // Unlike the outbound capture above, this is the actual consumed callee:
    // var captures undefined before KEY24 receives its independently valid donor.
    let source = change(ASYNC_DISPATCH_BS, "$KEY_24 = $THREAD_5_80456;",
        "var staleHelper=$KEY_24; $KEY_24 = $THREAD_5_80456;");
    let source = change(&source, "$KEY_24($BS, $BT, $BU, $BV);",
        "staleHelper($BS, $BT, $BU, $BV);");
    assert_role(&source, "$THREAD_1_15592", "hashStringOrArrayBufferViewAsync",
        "shared_scoped_role", false);
    assert_role(&source, "$THREAD_5_80456", "hashStringAsync",
        "purpose_finder::string_hash_async", true);
}

#[test]
fn iso_preserve_async_transitive_capture_and_deferred_expression_survive() {
    let source = change(ASYNC_DISPATCH_BS, "let $KEY_186 = $THREAD_1_15592;",
        "let $KEY_186 = $THREAD_1_15592; const again=$KEY_186; const finalAlias=again;");
    let source = change(&source, "return $KEY_186;", "return finalAlias;");
    assert_role(&source, "$THREAD_1_15592", "hashStringOrArrayBufferViewAsync",
        "shared_scoped_role", true);
    // The same timing proof applies when closure creation precedes setup.
    let source = change(ASYNC_DISPATCH_BS,
        "function $THREAD_1_15592($BS, $BT, $BU, $BV) {",
        "const $THREAD_1_15592 = function($BS, $BT, $BU, $BV) {");
    let source = change(&source, "let $KEY_186 = $THREAD_1_15592;",
        "; let $KEY_186 = $THREAD_1_15592;");
    assert_role(&source, "$THREAD_1_15592", "hashStringOrArrayBufferViewAsync",
        "shared_scoped_role", true);
}

#[test]
fn iso_preserve_async_stale_expression_and_unexecuted_helpers_refuse() {
    let source = change(ASYNC_DISPATCH_BS,
        "function $THREAD_1_15592($BS, $BT, $BU, $BV) {",
        "var stale=$THREAD_1_15592; var $THREAD_1_15592 = function($BS, $BT, $BU, $BV) {");
    let source = change(&source, "let $KEY_186 = $THREAD_1_15592;",
        "; let $KEY_186 = $THREAD_1_15592;");
    let source = change(&source, "return $KEY_186;", "return [$KEY_186,stale];");
    assert_role(&source, "$THREAD_1_15592", "hashStringOrArrayBufferViewAsync",
        "shared_scoped_role", false);
    for setup in [
        "if(false){$KEY_24 = $THREAD_5_80456;}",
        "function unused(){$KEY_24 = $THREAD_5_80456;}",
        "while(false){$KEY_24 = $THREAD_5_80456;}",
    ] {
        let source = change(ASYNC_DISPATCH_BS, "$KEY_24 = $THREAD_5_80456;", setup);
        assert_role(&source, "$THREAD_1_15592", "hashStringOrArrayBufferViewAsync",
            "shared_scoped_role", false);
    }
}

fn collector_factory(source: &str, expected: bool) {
    const TARGET: &str = "$THREAD_5_12882";
    const ROLE: &str = "createCollectFontMetrics";
    assert_role(source, TARGET, ROLE, "returned_function_factory", expected);
    let result = analyze(source, TARGET);
    let roles: Vec<_> = result.report.function_roles.iter()
        .filter(|f| f.current_name.as_deref() == Some(TARGET)).collect();
    assert_eq!(roles.len(), 1, "expected exactly the original factory role record");
    let function = roles[0];
    assert_eq!(function.candidates.iter().any(|r| r == ROLE), expected,
        "factory metadata disagrees with return-edge proof: {function:?}");
    assert_eq!(function.evaluators.iter().any(|e| e == "returned_function_factory"), expected,
        "factory evaluator disagrees with return-edge proof: {function:?}");
    // Only the return edge changes. Both the independently proven collector
    // and its real sampler must retain their public names and evaluators.
    collector(source, true);
}

#[test]
fn iso_factory_direct_initialized_and_split_return_aliases_survive() {
    collector_factory(COLLECTOR, true);
    for returned in [
        "let returned = $THREAD_3_8222; return returned;",
        "let returned; returned = $THREAD_3_8222; return returned;",
        "const first = $THREAD_3_8222; let returned; returned = first; return returned;",
    ] {
        let source = change(COLLECTOR, "return $THREAD_3_8222;", returned);
        collector_factory(&source, true);
    }
}

#[test]
fn iso_factory_uncalled_return_alias_refuses() {
    // Positive control proves that this exact factory/child edge is eligible.
    collector_factory(COLLECTOR, true);
    let source = change(COLLECTOR, "return $THREAD_3_8222;",
        "let returned; function unused(){returned=$THREAD_3_8222;} return returned;");
    collector_factory(&source, false);
}

#[test]
fn iso_factory_branch_and_nested_return_aliases_refuse() {
    collector_factory(COLLECTOR, true);
    for returned in [
        "let returned; if(false){returned=$THREAD_3_8222;} return returned;",
        "let returned; if(flag){returned=$THREAD_3_8222;} return returned;",
        "let returned; const unused=()=>{returned=$THREAD_3_8222;}; return returned;",
        "let returned; {returned=$THREAD_3_8222;} return returned;",
    ] {
        let source = change(COLLECTOR, "return $THREAD_3_8222;", returned);
        collector_factory(&source, false);
    }
}

#[test]
fn iso_factory_late_and_reassigned_return_aliases_refuse() {
    collector_factory(COLLECTOR, true);
    for returned in [
        "let returned; return returned; returned=$THREAD_3_8222;",
        "let returned=other; function unused(){returned=$THREAD_3_8222;} return returned;",
        "let returned; returned=other; if(false){returned=$THREAD_3_8222;} return returned;",
    ] {
        let source = change(COLLECTOR, "return $THREAD_3_8222;", returned);
        collector_factory(&source, false);
    }
}
