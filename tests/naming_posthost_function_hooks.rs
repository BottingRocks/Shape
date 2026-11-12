use shape::naming::{name_threads, NamedThreads};

// Fast-loop shared family: function-valued property descriptor interceptor.
// The installer owns every descriptor operation, the hook factories, the
// strict wrapper and the keyed registration. The wrapper is a different owned
// entity and is intentionally not claimed by this evaluator.
const EVALUATOR: &str = "posthost_function_hooks_interceptor";
const NAME: &str = "installMethodInterceptor";

const INTERCEPTOR: &str = r#"function main(){
let $KEY_362 = {};
function $THREAD_6_128322($BH, $BI, $BJ) {
					let $KEY_16 = $BJ;
					if ($BJ === void 0) {
						$KEY_16 = false;
					}
					let $BK = $KEY_100($BH, $BI);
					if ($BK == null) {
						return null;
					}
					let $BL = $BK.containingObj;
					$BK = $BK.desc;
					let $BM = $BK.configurable;
					let $BN = $BK.writable;
					let $KEY_2 = $BK.value;
					if (!$KEY_197(Object.hasOwnProperty, $BK, "value")) {
						return null;
					}
					let $BO = Object.create(null);
					$BO.value = $KEY_2;
					if ($BM === false && $BN === false || typeof $KEY_2 !== "function") {
						return { originals: $BO };
					} else {
						let $BP = $KEY_315($BL, $BI);
						if ($BP != null) {
							if ($BP.result != null) {
								return $BP.result;
							}
							$KEY_2 = $BP.original;
							$BO.value = $KEY_2;
						}
						let $KEY_6 = $KEY_186();
						let $KEY_7 = $KEY_186();
						function $THREAD_0_34164() {
							"use strict";
							let $KEY_10 = arguments;
							let $BW = $KEY_6.notify({
								args: $KEY_10,
								thisObj: this
							});
							let $BX;
							if ($BW) {
								if ($BW.bypassResult != null) {
									if ($BW.bypassResult.throw) {
										throw $BW.bypassResult.value;
									} else {
										return $BW.bypassResult.value;
									}
								} else {
									$BX = $BW.args != null ? $BW.args : $KEY_10;
								}
							} else {
								$BX = $KEY_10;
							}
							$KEY_10 = arguments;
							let $BY;
							let $BZ = {
								args: $KEY_10,
								thisObj: this,
								threw: true,
								result: null
							};
							let $CA;
							try {
								$BY = $KEY_16 && this instanceof $THREAD_0_34164 ? $KEY_59($KEY_2, $BX) : $KEY_9($KEY_2, this, $BX);
								$BZ = {
									args: arguments,
									thisObj: this,
									threw: false,
									result: $BY
								};
							} finally {
								let $CB = $KEY_7.notify($BZ);
								if ($CB && $CB.bypassResult != null) {
									if ($CB.bypassResult.throw) {
										throw $CB.bypassResult.value;
									} else {
										return $CB.bypassResult.value;
									}
								} else {
									$CA = $BY;
								}
							}
							return $CA;
						}
						$BK.value = $THREAD_0_34164;
						let $BQ = $BK.value;
						if (typeof $KEY_2 === "function") {
							try {
								let $BR = $KEY_49.toString.call($KEY_2);
								$KEY_93.push($BR);
								$KEY_252.push($BQ);
								if (Function.prototype.toString !== $KEY_49.toString) {
									Function.prototype.toString = $KEY_49.toString;
								}
							} catch (e) {}
						}
						let $BS = Object.getOwnPropertyNames($KEY_2);
						let i = 0;
						for (; i < $BS.length; i++) {
							if ($KEY_197(Array.prototype.indexOf, $KEY_103, $BS[i]) === -1) {
								let $BT = Object.getOwnPropertyDescriptor($BQ, $BS[i]);
								if ($BT == null || $BT.configurable === true || $BT.writable === true) {
									let $BU = Object.getOwnPropertyDescriptor($KEY_2, $BS[i]);
									if ($BU != null) {
										if ($BT == null || $BT.configurable !== false || $BU.configurable !== true) {
											Object.defineProperty($BQ, $BS[i], $BU);
										}
									}
								}
							}
						}
						try {
							if (!$KEY_197(Object.hasOwnProperty, $KEY_2, "prototype")) {
								$BQ.prototype = void 0;
							}
						} catch (e_2) {}
						Object.defineProperty($BL, $BI, $BK);
						$BO = {
							onBeforeInvoke: $KEY_6,
							onAfterInvoke: $KEY_7,
							originals: $BO
						};
						$BL = {
							object: $BL,
							result: $BO,
							original: $KEY_2
						};
						let $BV = $KEY_362[$BI];
						if ($BV == null) {
							$BV = [];
							$KEY_362[$BI] = $BV;
						}
						$KEY_197(Array.prototype.push, $BV, $BL);
						return $BO;
					}
				}
return $THREAD_6_128322;
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

fn our_claim(source: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result.report.function_roles.iter().any(|role| {
            role.evaluators
                .iter()
                .any(|evaluator| evaluator == EVALUATOR)
        })
    })
}

fn claims(source: &str) -> Vec<(Option<String>, Option<String>)> {
    name_threads(source, true)
        .map(|result| {
            result
                .report
                .function_roles
                .iter()
                .filter(|role| {
                    role.evaluators
                        .iter()
                        .any(|evaluator| evaluator == EVALUATOR)
                })
                .map(|role| (role.current_name.clone(), role.candidates.first().cloned()))
                .collect()
        })
        .unwrap_or_default()
}

fn parses(source: &str) -> bool {
    name_threads(source, true).is_ok()
}

fn denied(source: &str) {
    assert!(parses(source), "fixture did not parse");
    assert!(!named(source, NAME), "unexpected {NAME}");
    assert!(!our_claim(source), "unexpected posthost claim");
}

#[test]
fn interceptor_accepts_the_representative_source() {
    assert!(named(INTERCEPTOR, NAME), "representative source not named");
    assert!(our_claim(INTERCEPTOR));
}

#[test]
fn interceptor_claims_only_the_installer_not_the_wrapper() {
    let result: NamedThreads = name_threads(INTERCEPTOR, true).unwrap();
    let claimed: Vec<_> = result
        .report
        .function_roles
        .iter()
        .filter(|role| {
            role.evaluators
                .iter()
                .any(|evaluator| evaluator == EVALUATOR)
        })
        .collect();
    assert_eq!(claimed.len(), 1, "expected only the outer installer");
    assert_eq!(claimed[0].current_name.as_deref(), Some("$THREAD_6_128322"));
    let wrapper = result
        .report
        .function_roles
        .iter()
        .find(|role| role.current_name.as_deref() == Some("$THREAD_0_34164"))
        .expect("nested wrapper must exist");
    assert!(
        !wrapper
            .evaluators
            .iter()
            .any(|evaluator| evaluator == EVALUATOR),
        "the owned wrapper belongs to a different evaluator"
    );
}

#[test]
fn interceptor_accepts_local_spelling_variants() {
    let renamed = INTERCEPTOR
        .replace("$THREAD_6_128322", "$THREAD_90_777_2")
        .replace("$THREAD_0_34164", "$THREAD_42_31337")
        .replace("$BH", "$objectArgument")
        .replace("$BI", "$keyArgument")
        .replace("$BJ", "$optionalArgument")
        .replace("$BK", "$record")
        .replace("$BL", "$host")
        .replace("$BM", "$configurable")
        .replace("$BN", "$writable")
        .replace("$BO", "$originals")
        .replace("$BP", "$cached")
        .replace("$BQ", "$wrapped")
        .replace("$BS", "$names")
        .replace("$BV", "$bucket")
        .replace("$BW", "$beforeResult")
        .replace("$BX", "$forwardedArguments")
        .replace("$BY", "$callResult")
        .replace("$BZ", "$afterRecord")
        .replace("$CA", "$returned")
        .replace("$CB", "$afterResult")
        .replace("$KEY_6", "$onBefore")
        .replace("$KEY_7", "$onAfter");
    assert!(named(&renamed, NAME), "spelling variant not named");
    assert!(our_claim(&renamed));
}

#[test]
fn interceptor_accepts_for_and_while_copy_forms() {
    let for_let = INTERCEPTOR.replace(
        "let i = 0;\n\t\t\t\t\t\tfor (; i < $BS.length; i++) {",
        "for (let i = 0; i < $BS.length; i++) {",
    );
    assert!(named(&for_let, NAME), "for-let variant not named");

    let while_form = INTERCEPTOR.replace(
        "let i = 0;\n\t\t\t\t\t\tfor (; i < $BS.length; i++) {",
        "let i = 0;\n\t\t\t\t\t\twhile (i < $BS.length) {",
    );
    let while_form = while_form.replace(
        "\t\t\t\t\t\t\t}\n\t\t\t\t\t\t}\n\t\t\t\t\t\ttry {",
        "\t\t\t\t\t\t\t}\n\t\t\t\t\t\t\ti++;\n\t\t\t\t\t\t}\n\t\t\t\t\t\ttry {",
    );
    assert!(parses(&while_form), "while variant did not parse");
    assert!(named(&while_form, NAME), "while variant not named");
}

#[test]
fn interceptor_accepts_statement_order_variants() {
    // Move the descriptor re-installation after the hook record is built: the
    // same dataflow, a different node order.
    let reordered = INTERCEPTOR.replace(
        "\t\t\t\t\t\tObject.defineProperty($BL, $BI, $BK);\n\t\t\t\t\t\t$BO = {",
        "\t\t\t\t\t\t$BO = {",
    );
    let reordered = reordered.replace(
        "\t\t\t\t\t\t\toriginal: $KEY_2\n\t\t\t\t\t\t};",
        "\t\t\t\t\t\t\toriginal: $KEY_2\n\t\t\t\t\t\t};\n\t\t\t\t\t\tObject.defineProperty($BL, $BI, $BK);",
    );
    assert!(parses(&reordered), "reordered fixture did not parse");
    assert!(named(&reordered, NAME), "reordered variant not named");
}

#[test]
fn interceptor_accepts_bare_declaration_then_assignment_forms() {
    // Hoisted locals use bare `let a, b, c;` declarations followed by plain assignments. Symbol
    // identity must establish roles independently of declaration form.
    let assigned = INTERCEPTOR
        .replace(
            "let $KEY_2 = $BK.value;",
            "let $KEY_2;\n$KEY_2 = $BK.value;",
        )
        .replace("let $BQ = $BK.value;", "let $BQ;\n$BQ = $BK.value;")
        .replace(
            "let $BP = $KEY_315($BL, $BI);",
            "let $BP;\n$BP = $KEY_315($BL, $BI);",
        )
        .replace(
            "let $KEY_6 = $KEY_186();",
            "let $KEY_6;\n$KEY_6 = $KEY_186();",
        )
        .replace(
            "let $KEY_7 = $KEY_186();",
            "let $KEY_7;\n$KEY_7 = $KEY_186();",
        );
    assert!(parses(&assigned), "assignment-form fixture did not parse");
    assert!(named(&assigned, NAME), "assignment-form variant not named");
    assert!(our_claim(&assigned));
}

#[test]
fn interceptor_requires_the_descriptor_dataflow() {
    for source in [
        // A different descriptor field is not `writable`.
        INTERCEPTOR.replace("let $BN = $BK.writable;", "let $BN = $BL.writable;"),
        // A different receiver for the live value read.
        INTERCEPTOR.replace("let $KEY_2 = $BK.value;", "let $KEY_2 = $BL.value;"),
        // A different own-property key.
        INTERCEPTOR.replace(
            "Object.hasOwnProperty, $BK, \"value\"",
            "Object.hasOwnProperty, $BK, \"writable\"",
        ),
        // The null short-circuit disappears.
        INTERCEPTOR.replace(
            "if ($BK == null) {\n\t\t\t\t\t\treturn null;\n\t\t\t\t\t}",
            "if (false) {\n\t\t\t\t\t\treturn null;\n\t\t\t\t\t}",
        ),
        // The early predicate is no longer the proved one.
        INTERCEPTOR.replace(
            "typeof $KEY_2 !== \"function\"",
            "typeof $KEY_2 === \"function\"",
        ),
        // The cache shortcut stops rebinding the original.
        INTERCEPTOR.replace("$KEY_2 = $BP.original;", "$KEY_2 = $BP.result;"),
        // The null-prototype originals record is a normal object.
        INTERCEPTOR.replace("Object.create(null)", "Object.create({})"),
    ] {
        denied(&source);
    }
}

#[test]
fn interceptor_requires_the_property_copy_and_prototype_guard() {
    for source in [
        // No own-property descriptor copy loop at all.
        INTERCEPTOR.replace(
            "let $BS = Object.getOwnPropertyNames($KEY_2);\n\t\t\t\t\t\tlet i = 0;\n\t\t\t\t\t\tfor (; i < $BS.length; i++) {\n\t\t\t\t\t\t\tif ($KEY_197(Array.prototype.indexOf, $KEY_103, $BS[i]) === -1) {\n\t\t\t\t\t\t\t\tlet $BT = Object.getOwnPropertyDescriptor($BQ, $BS[i]);\n\t\t\t\t\t\t\t\tif ($BT == null || $BT.configurable === true || $BT.writable === true) {\n\t\t\t\t\t\t\t\t\tlet $BU = Object.getOwnPropertyDescriptor($KEY_2, $BS[i]);\n\t\t\t\t\t\t\t\t\tif ($BU != null) {\n\t\t\t\t\t\t\t\t\t\tif ($BT == null || $BT.configurable !== false || $BU.configurable !== true) {\n\t\t\t\t\t\t\t\t\t\t\tObject.defineProperty($BQ, $BS[i], $BU);\n\t\t\t\t\t\t\t\t\t\t}\n\t\t\t\t\t\t\t\t\t}\n\t\t\t\t\t\t\t\t}\n\t\t\t\t\t\t\t}\n\t\t\t\t\t\t}",
            "",
        ),
        // A different source-of-own-names operation.
        INTERCEPTOR.replace(
            "Object.getOwnPropertyNames($KEY_2)",
            "Object.keys($KEY_2)",
        ),
        // A different target for the copied descriptor.
        INTERCEPTOR.replace(
            "Object.defineProperty($BQ, $BS[i], $BU);",
            "Object.defineProperty($BO, $BS[i], $BU);",
        ),
        // The prototype guard writes a different object.
        INTERCEPTOR.replace("$BQ.prototype = void 0;", "$BO.prototype = void 0;"),
        // The descriptor is installed on a different containing object.
        INTERCEPTOR.replace(
            "Object.defineProperty($BL, $BI, $BK);",
            "Object.defineProperty($BO, $BI, $BK);",
        ),
    ] {
        denied(&source);
    }
}

#[test]
fn interceptor_requires_the_owned_wrapper_and_hooks() {
    for source in [
        // One hook is an alias of the other, so only one zero-argument factory
        // call exists.
        INTERCEPTOR.replace("let $KEY_7 = $KEY_186();", "let $KEY_7 = $KEY_6;"),
        // The after-hook is never notified.
        INTERCEPTOR.replace("$KEY_7.notify($BZ)", "$KEY_7($BZ)"),
        // The wrapper stops handling a bypass result.
        INTERCEPTOR
            .replace("$CB.bypassResult", "$CB.result")
            .replace("$BW.bypassResult", "$BW.result"),
        // The wrapper stops invoking the captured original value.
        INTERCEPTOR.replace(
            "$KEY_59($KEY_2, $BX) : $KEY_9($KEY_2, this, $BX)",
            "$KEY_59($KEY_6, $BX) : $KEY_9($KEY_6, this, $BX)",
        ),
        // The wrapper stops being a strict function.
        INTERCEPTOR.replace("\"use strict\";", ""),
    ] {
        denied(&source);
    }
}

#[test]
fn interceptor_requires_unshadowed_global_builtins() {
    for source in [
        // `Object` is shadowed by a local binding.
        INTERCEPTOR.replace(
            "let $KEY_362 = {};",
            "let $KEY_362 = {}; let Object = { create: function () { return {}; } };",
        ),
        // `Array` is shadowed, so the indexOf guard is not the global.
        INTERCEPTOR.replace(
            "let $KEY_362 = {};",
            "let $KEY_362 = {}; let Array = { prototype: { indexOf: function () {} , push: function () {} } };",
        ),
    ] {
        denied(&source);
    }
}

#[test]
fn interceptor_refuses_a_nested_decoy_that_only_contains_the_installer() {
    // The outer function now only declares and returns the real installer:
    // none of the operations are owned by the outer body.
    let nested = INTERCEPTOR.replace(
        "function $THREAD_6_128322($BH, $BI, $BJ) {",
        "function $THREAD_6_128322($BH, $BI, $BJ) { return $THREAD_9_4242($BH, $BI, $BJ); }\nfunction $THREAD_9_4242($BH, $BI, $BJ) {",
    );
    assert!(parses(&nested), "nested fixture did not parse");
    let claimed = claims(&nested);
    assert!(
        !claimed
            .iter()
            .any(|(current, _)| current.as_deref() == Some("$THREAD_6_128322")),
        "the outer decoy must not be claimed: {claimed:?}"
    );
}

#[test]
fn interceptor_preserves_human_names_and_collision_guards() {
    let human = INTERCEPTOR.replace("$THREAD_6_128322", NAME);
    let result = name_threads(&human, true).unwrap();
    assert!(result.source.contains(&format!("function {NAME}(")));
    assert!(!result
        .report
        .outcomes
        .iter()
        .any(|outcome| outcome.binding.name == NAME && outcome.applied));

    for collision in [
        INTERCEPTOR.replace(
            "let $KEY_362 = {};",
            "let $KEY_362 = {}; let installMethodInterceptor = 17; useValue(installMethodInterceptor);",
        ),
        INTERCEPTOR.replace(
            "let $KEY_362 = {};",
            "let $KEY_362 = {}; function installMethodInterceptor() {} useValue(installMethodInterceptor);",
        ),
    ] {
        assert!(parses(&collision), "{collision}");
        assert!(!named(&collision, NAME), "{collision}");
    }
}

// The short-circuit branch rewrites the null-prototype originals holder into the result
// record and returns that same local. This differs from the representative body above.
const REWRITE_RETURN_INTERCEPTOR: &str = r#"function main(){
let $KEY_140 = {};
				function $THREAD_8_121349($BQ, $BR, $BS) {
					let $KEY_9 = $BS;
					let $KEY_11;
					if ($BS === void 0) {
						$KEY_9 = false;
					}
					let $BT = $KEY_411($BQ, $BR);
					if ($BT == null) {
						return null;
					}
					let $BU = $BT.containingObj;
					$BT = $BT.desc;
					let $BV = $BT.configurable;
					let $BW = $BT.writable;
					let $KEY_2 = $BT.value;
					if (!$KEY_136(Object.hasOwnProperty, $BT, "value")) {
						return null;
					}
					let $BX = Object.create(null);
					$BX.value = $KEY_2;
					if ($BV === false && $BW === false || typeof $KEY_2 !== "function") {
						$BX = { originals: $BX };
						return $BX;
					} else {
						let $BY = $KEY_135($BU, $BR);
						if ($BY != null) {
							if ($BY.result != null) {
								return $BY.result;
							}
							$KEY_2 = $BY.original;
							$BX.value = $KEY_2;
						}
						let $KEY_7 = $KEY_161();
						$KEY_11 = $KEY_161();
						function $THREAD_5_127666() {
							"use strict";
							let $KEY_1 = arguments;
							let $CF = $KEY_7.notify({
								args: $KEY_1,
								thisObj: this
							});
							let $CG;
							if ($CF) {
								if ($CF.bypassResult != null) {
									if ($CF.bypassResult.throw) {
										throw $CF.bypassResult.value;
									} else {
										return $CF.bypassResult.value;
									}
								} else {
									$CG = $CF.args != null ? $CF.args : $KEY_1;
								}
							} else {
								$CG = $KEY_1;
							}
							let $CH = {
								args: void 0,
								thisObj: void 0
							};
							$KEY_1 = arguments;
							$CH.args = $KEY_1;
							$CH.thisObj = this;
							$CH.threw = true;
							$CH.result = null;
							let $CI;
							let $CJ = $CH;
							let $CK;
							try {
								$CI = $KEY_9 && this instanceof $THREAD_5_127666 ? $KEY_454($KEY_2, $CG) : $KEY_75($KEY_2, this, $CG);
								$CJ = {
									args: arguments,
									thisObj: this,
									threw: false,
									result: $CI
								};
							} finally {
								let $CM = $KEY_11.notify($CJ);
								if ($CM && $CM.bypassResult != null) {
									if ($CM.bypassResult.throw) {
										throw $CM.bypassResult.value;
									} else {
										return $CM.bypassResult.value;
									}
								} else {
									$CK = $CI;
								}
							}
							return $CK;
						}
						$BT.value = $THREAD_5_127666;
						let $BZ = $BT.value;
						if (typeof $KEY_2 === "function") {
							try {
								let $CA = $KEY_445.toString.call($KEY_2);
								$KEY_457.push($CA);
								$KEY_208.push($BZ);
								if (Function.prototype.toString !== $KEY_445.toString) {
									Function.prototype.toString = $KEY_445.toString;
								}
							} catch (e) {}
						}
						let $CB = Object.getOwnPropertyNames($KEY_2);
						let i = 0;
						for (; i < $CB.length; i++) {
							if ($KEY_136(Array.prototype.indexOf, $KEY_408, $CB[i]) === -1) {
								let $CC = Object.getOwnPropertyDescriptor($BZ, $CB[i]);
								if ($CC == null || $CC.configurable === true || $CC.writable === true) {
									let $CD = Object.getOwnPropertyDescriptor($KEY_2, $CB[i]);
									if ($CD != null) {
										if ($CC == null || $CC.configurable !== false || $CD.configurable !== true) {
											Object.defineProperty($BZ, $CB[i], $CD);
										}
									}
								}
							}
						}
						try {
							if (!$KEY_136(Object.hasOwnProperty, $KEY_2, "prototype")) {
								$BZ.prototype = void 0;
							}
						} catch (e_2) {}
						Object.defineProperty($BU, $BR, $BT);
						$BX = {
							onBeforeInvoke: $KEY_7,
							onAfterInvoke: $KEY_11,
							originals: $BX
						};
						$BU = {
							object: $BU,
							result: $BX,
							original: $KEY_2
						};
						let $CE = $KEY_140[$BR];
						if ($CE == null) {
							$CE = [];
							$KEY_140[$BR] = $CE;
						}
						$KEY_136(Array.prototype.push, $CE, $BU);
						return $BX;
					}
				}
return $THREAD_8_121349;
}
main();"#;

// The rewrite-return shape also supports descriptor fields, live values, and both hook
// factories initialized by plain assignments to bare let locals.
const ASSIGNED_REWRITE_RETURN_INTERCEPTOR: &str = r#"function main(){
let $KEY_140 = {};
		function $THREAD_1_75932($BR, $BS, $BT) {
			let $KEY_0 = $BT;
			let $KEY_3, $KEY_4, $KEY_12;
			if ($BT === void 0) {
				$KEY_0 = false;
			}
			let $BU = $KEY_9($BR, $BS);
			if ($BU == null) {
				return null;
			}
			let $BV = $BU.containingObj;
			$BU = $BU.desc;
			let $BW = $BU.configurable;
			let $BX = $BU.writable;
			$KEY_12 = $BU.value;
			if (!$KEY_406(Object.hasOwnProperty, $BU, "value")) {
				return null;
			}
			let $BY = Object.create(null);
			$BY.value = $KEY_12;
			if ($BW === false && $BX === false || typeof $KEY_12 !== "function") {
				$BY = { originals: $BY };
				return $BY;
			} else {
				let $BZ = $KEY_142($BV, $BS);
				if ($BZ != null) {
					if ($BZ.result != null) {
						return $BZ.result;
					}
					$KEY_12 = $BZ.original;
					$BY.value = $KEY_12;
				}
				$KEY_3 = $KEY_360();
				$KEY_4 = $KEY_360();
				function $THREAD_8_34667() {
					"use strict";
					let $KEY_5 = arguments;
					let $CG = $KEY_3.notify({
						args: $KEY_5,
						thisObj: this
					});
					let $CH;
					if ($CG) {
						if ($CG.bypassResult != null) {
							if ($CG.bypassResult.throw) {
								throw $CG.bypassResult.value;
							} else {
								return $CG.bypassResult.value;
							}
						} else {
							$CH = $CG.args != null ? $CG.args : $KEY_5;
						}
					} else {
						$CH = $KEY_5;
					}
					$KEY_5 = arguments;
					let $CI = {
						args: $KEY_5,
						thisObj: this,
						threw: true,
						result: null
					};
					let $CJ, $CK;
					try {
						$CJ = $KEY_0 && this instanceof $THREAD_8_34667 ? $KEY_263($KEY_12, $CH) : $KEY_451($KEY_12, this, $CH);
						$CI = {
							args: arguments,
							thisObj: this,
							threw: false,
							result: $CJ
						};
					} finally {
						let $CL = $KEY_4.notify($CI);
						if ($CL && $CL.bypassResult != null) {
							if ($CL.bypassResult.throw) {
								throw $CL.bypassResult.value;
							} else {
								return $CL.bypassResult.value;
							}
						} else {
							$CK = $CJ;
						}
					}
					return $CK;
				}
				$BU.value = $THREAD_8_34667;
				let $CA = $BU.value;
				if (typeof $KEY_12 === "function") {
					try {
						let $CB = $KEY_213.toString.call($KEY_12);
						$KEY_73.push($CB);
						$KEY_266.push($CA);
						if (Function.prototype.toString !== $KEY_213.toString) {
							Function.prototype.toString = $KEY_213.toString;
						}
					} catch (e) {}
				}
				let $CC = Object.getOwnPropertyNames($KEY_12);
				let i = 0;
				for (; i < $CC.length; i++) {
					if ($KEY_406(Array.prototype.indexOf, $KEY_206, $CC[i]) === -1) {
						let $CD = Object.getOwnPropertyDescriptor($CA, $CC[i]);
						if ($CD == null || $CD.configurable === true || $CD.writable === true) {
							let $CE = Object.getOwnPropertyDescriptor($KEY_12, $CC[i]);
							if ($CE != null) {
								if ($CD == null || $CD.configurable !== false || $CE.configurable !== true) {
									Object.defineProperty($CA, $CC[i], $CE);
								}
							}
						}
					}
				}
				try {
					if (!$KEY_406(Object.hasOwnProperty, $KEY_12, "prototype")) {
						$CA.prototype = void 0;
					}
				} catch (e_2) {}
				Object.defineProperty($BV, $BS, $BU);
				$BY = {
					onBeforeInvoke: $KEY_3,
					onAfterInvoke: $KEY_4,
					originals: $BY
				};
				$BV = {
					object: $BV,
					result: $BY,
					original: $KEY_12
				};
				let $CF = $KEY_140[$BS];
				if ($CF == null) {
					$CF = [];
					$KEY_140[$BS] = $CF;
				}
				$KEY_406(Array.prototype.push, $CF, $BV);
				return $BY;
			}
		}
return $THREAD_1_75932;
}
main();"#;

#[test]
fn interceptor_accepts_initialized_and_assigned_rewrite_return_shapes() {
    assert!(
        parses(REWRITE_RETURN_INTERCEPTOR),
        "rewrite-return fixture did not parse"
    );
    assert!(
        named(REWRITE_RETURN_INTERCEPTOR, NAME),
        "rewrite-return variant not named"
    );
    assert!(our_claim(REWRITE_RETURN_INTERCEPTOR));

    assert!(
        parses(ASSIGNED_REWRITE_RETURN_INTERCEPTOR),
        "assigned rewrite-return fixture did not parse"
    );
    assert!(
        named(ASSIGNED_REWRITE_RETURN_INTERCEPTOR, NAME),
        "assigned rewrite-return variant not named"
    );
    assert!(our_claim(ASSIGNED_REWRITE_RETURN_INTERCEPTOR));
}

#[test]
fn interceptor_accepts_rewrite_return_local_spellings() {
    let renamed = REWRITE_RETURN_INTERCEPTOR
        .replace("$THREAD_8_121349", "$THREAD_90_777_2")
        .replace("$THREAD_5_127666", "$THREAD_42_31337")
        .replace("$BQ", "$objectArgument")
        .replace("$BR", "$keyArgument")
        .replace("$BS", "$optionalArgument")
        .replace("$BT", "$record")
        .replace("$BU", "$host")
        .replace("$BV", "$configurable")
        .replace("$BW", "$writable")
        .replace("$BX", "$savedRecord")
        .replace("$BY", "$cached")
        .replace("$BZ", "$wrapped")
        .replace("$CB", "$ownNames")
        .replace("$CC", "$ownDescriptor")
        .replace("$CD", "$originalDescriptor")
        .replace("$CE", "$bucket")
        .replace("$CA", "$originalSource")
        .replace("$CF", "$beforeResult")
        .replace("$CG", "$forwardedArguments")
        .replace("$CH", "$afterRecord")
        .replace("$CI", "$callResult")
        .replace("$CJ", "$afterInput")
        .replace("$CK", "$afterResult")
        .replace("$CM", "$afterBypass");
    assert!(parses(&renamed), "spelling fixture did not parse");
    assert!(named(&renamed, NAME), "spelling variant not named");
    assert!(our_claim(&renamed));
}

#[test]
fn interceptor_refuses_rewrite_return_decoys() {
    for source in [
        // The returned record keeps the wrong originals value.
        REWRITE_RETURN_INTERCEPTOR
            .replace("$BX = { originals: $BX };", "$BX = { originals: $KEY_2 };"),
        // The returned record carries extra properties.
        REWRITE_RETURN_INTERCEPTOR.replace(
            "$BX = { originals: $BX };",
            "$BX = { originals: $BX, value: $KEY_2 };",
        ),
        // A preceding rewrite of the returned local to a non-record value
        // means the record's `originals` field would capture the wrong value.
        REWRITE_RETURN_INTERCEPTOR.replace(
            "$BX = { originals: $BX };",
            "$BX = useValue; $BX = { originals: $BX };",
        ),
        // A nested decoy builds the record, but the returned local is still
        // the untouched null-prototype holder.
        REWRITE_RETURN_INTERCEPTOR.replace(
            "$BX = { originals: $BX };",
            "if ($BR) { $BX = { originals: $BX }; }",
        ),
        // A different receiver for the live value read.
        REWRITE_RETURN_INTERCEPTOR.replace("let $KEY_2 = $BT.value;", "let $KEY_2 = $BU.value;"),
        // A different source-of-own-names operation.
        REWRITE_RETURN_INTERCEPTOR
            .replace("Object.getOwnPropertyNames($KEY_2)", "Object.keys($KEY_2)"),
        // The descriptor is re-installed on a different receiver.
        REWRITE_RETURN_INTERCEPTOR.replace(
            "Object.defineProperty($BU, $BR, $BT);",
            "Object.defineProperty($KEY_2, $BR, $BT);",
        ),
        // `Object` is shadowed inside the owner.
        REWRITE_RETURN_INTERCEPTOR.replace(
            "let $KEY_140 = {};",
            "let $KEY_140 = {}; let Object = { create: function () { return {}; } };",
        ),
    ] {
        denied(&source);
    }
}

const REGISTRY_WALK_INLINE_INTERCEPTOR: &str = r#"function main(){
let $KEY_129 = {};
function $THREAD_4_134856($AM, $AN, $AO) {
			let $KEY_17 = $AO;
			if ($AO === void 0) {
				$KEY_17 = false;
			}
			let $AP = $AM;
			let $AQ;
			for (;;) {
				$AQ = void 0;
				if ($AP == null) {
					$AQ = null;
					break;
				}
				let $AR = Object.getOwnPropertyDescriptor($AP, $AN);
				if ($AR == null) {
					$AP = Object.getPrototypeOf($AP);
					continue;
				}
				$AQ = {
					containingObj: $AP,
					desc: $AR
				};
				break;
			}
			if ($AQ == null) {
				return null;
			}
			let $AS = $AQ.containingObj;
			$AQ = $AQ.desc;
			let $AT = $AQ.configurable;
			let $AU = $AQ.writable;
			let $KEY_14 = $AQ.value;
			if (!$KEY_115(Object.hasOwnProperty, $AQ, "value")) {
				return null;
			}
			let $AV = Object.create(null);
			$AV.value = $KEY_14;
			if ($AT === false && $AU === false || typeof $KEY_14 !== "function") {
				$AV = { originals: $AV };
				return $AV;
			} else {
				let $AW;
				if ($KEY_129[$AN] == null) {
					$AW = null;
				} else {
					let $AX = 0;
					for (;;) {
						if ($AX >= $KEY_129[$AN].length) {
							$AW = null;
							break;
						}
						$AW = $KEY_129[$AN][$AX];
						if ($AS !== $AW.object) {
							++$AX;
							continue;
						}
						break;
					}
				}
				if ($AW != null) {
					if ($AW.result != null) {
						return $AW.result;
					}
					$KEY_14 = $AW.original;
					$AV.value = $KEY_14;
				}
				let $KEY_16 = $KEY_381();
				let $KEY_2 = $KEY_381();
				function $THREAD_6_6695() {
					"use strict";
					let $KEY_5 = arguments;
					let $BE = $KEY_16.notify({
						args: $KEY_5,
						thisObj: this
					});
					let $BF;
					if ($BE) {
						if ($BE.bypassResult != null) {
							if ($BE.bypassResult.throw) {
								throw $BE.bypassResult.value;
							} else {
								return $BE.bypassResult.value;
							}
						} else {
							$BF = $BE.args != null ? $BE.args : $KEY_5;
						}
					} else {
						$BF = $KEY_5;
					}
					$KEY_5 = arguments;
					let $BG = {
						args: $KEY_5,
						thisObj: this,
						threw: true,
						result: null
					};
					let $BH, $BI;
					try {
						$BH = $KEY_17 && this instanceof $THREAD_6_6695 ? $KEY_134($KEY_14, $BF) : $KEY_103($KEY_14, this, $BF);
						$BG = {
							args: arguments,
							thisObj: this,
							threw: false,
							result: $BH
						};
					} finally {
						let $BJ = $KEY_2.notify($BG);
						if ($BJ && $BJ.bypassResult != null) {
							if ($BJ.bypassResult.throw) {
								throw $BJ.bypassResult.value;
							} else {
								return $BJ.bypassResult.value;
							}
						} else {
							$BI = $BH;
						}
					}
					return $BI;
				}
				$AQ.value = $THREAD_6_6695;
				let $AY = $AQ.value;
				if (typeof $KEY_14 === "function") {
					try {
						let $AZ = $KEY_255.toString.call($KEY_14);
						$KEY_156.push($AZ);
						$KEY_416.push($AY);
						if (Function.prototype.toString !== $KEY_255.toString) {
							Function.prototype.toString = $KEY_255.toString;
						}
					} catch (e) {}
				}
				let $BA = Object.getOwnPropertyNames($KEY_14);
				let i = 0;
				for (; i < $BA.length; i++) {
					if ($KEY_115(Array.prototype.indexOf, $KEY_303, $BA[i]) === -1) {
						let $BB = Object.getOwnPropertyDescriptor($AY, $BA[i]);
						if ($BB == null || $BB.configurable === true || $BB.writable === true) {
							let $BC = Object.getOwnPropertyDescriptor($KEY_14, $BA[i]);
							if ($BC != null) {
								if ($BB == null || $BB.configurable !== false || $BC.configurable !== true) {
									Object.defineProperty($AY, $BA[i], $BC);
								}
							}
						}
					}
				}
				try {
					if (!$KEY_115(Object.hasOwnProperty, $KEY_14, "prototype")) {
						$AY.prototype = void 0;
					}
				} catch (e_2) {}
				Object.defineProperty($AS, $AN, $AQ);
				$AV = {
					onBeforeInvoke: $KEY_16,
					onAfterInvoke: $KEY_2,
					originals: $AV
				};
				$AS = {
					object: $AS,
					result: $AV,
					original: $KEY_14
				};
				let $BD = $KEY_129[$AN];
				if ($BD == null) {
					$BD = [];
					$KEY_129[$AN] = $BD;
				}
				$KEY_115(Array.prototype.push, $BD, $AS);
				return $AV;
			}
		}
return $THREAD_4_134856;
}
main();"#;

const TARGET_INLINE_INTERCEPTOR: &str = r#"function main(){
let $KEY_303 = {};
function $THREAD_1_121919($AP, $AQ, $AR) {
			let $KEY_1 = $AR;
			let $KEY_7, $KEY_10, $KEY_16;
			if ($AR === void 0) {
				$KEY_1 = false;
			}
			let $AS = $AP;
			let $AT;
			for (;;) {
				$AT = void 0;
				if ($AS == null) {
					$AT = null;
					break;
				}
				let $AU = Object.getOwnPropertyDescriptor($AS, $AQ);
				if ($AU == null) {
					$AS = Object.getPrototypeOf($AS);
					continue;
				}
				$AT = {
					containingObj: $AS,
					desc: $AU
				};
				break;
			}
			if ($AT == null) {
				return null;
			}
			let $AV = $AT.containingObj;
			$AT = $AT.desc;
			let $AW = $AT.configurable;
			let $AX = $AT.writable;
			$KEY_10 = $AT.value;
			if (!$KEY_128(Object.hasOwnProperty, $AT, "value")) {
				return null;
			}
			let $AY = Object.create(null);
			$AY.value = $KEY_10;
			if ($AW === false && $AX === false || typeof $KEY_10 !== "function") {
				$AY = { originals: $AY };
				return $AY;
			} else {
				let $AZ;
				if ($KEY_303[$AQ] == null) {
					$AZ = null;
				} else {
					let $BA = 0;
					for (;;) {
						if ($BA >= $KEY_303[$AQ].length) {
							$AZ = null;
							break;
						}
						$AZ = $KEY_303[$AQ][$BA];
						if ($AV !== $AZ.object) {
							++$BA;
							continue;
						}
						break;
					}
				}
				if ($AZ != null) {
					if ($AZ.result != null) {
						return $AZ.result;
					}
					$KEY_10 = $AZ.original;
					$AY.value = $KEY_10;
				}
				$KEY_7 = $KEY_107();
				$KEY_16 = $KEY_107();
				function $THREAD_1_59499() {
					"use strict";
					let $KEY_2 = arguments;
					let $BH = $KEY_7.notify({
						args: $KEY_2,
						thisObj: this
					});
					let $BI;
					if ($BH) {
						if ($BH.bypassResult != null) {
							if ($BH.bypassResult.throw) {
								throw $BH.bypassResult.value;
							} else {
								return $BH.bypassResult.value;
							}
						} else {
							$BI = $BH.args != null ? $BH.args : $KEY_2;
						}
					} else {
						$BI = $KEY_2;
					}
					let $BJ = {
						args: void 0,
						thisObj: void 0
					};
					$KEY_2 = arguments;
					$BJ.args = $KEY_2;
					$BJ.thisObj = this;
					$BJ.threw = true;
					$BJ.result = null;
					let $BK = $BJ;
					let $BL, $BM;
					try {
						$BL = $KEY_1 && this instanceof $THREAD_1_59499 ? $KEY_174($KEY_10, $BI) : $KEY_145($KEY_10, this, $BI);
						$BK = {
							args: arguments,
							thisObj: this,
							threw: false,
							result: $BL
						};
					} finally {
						let $BO = $KEY_16.notify($BK);
						if ($BO && $BO.bypassResult != null) {
							if ($BO.bypassResult.throw) {
								throw $BO.bypassResult.value;
							} else {
								return $BO.bypassResult.value;
							}
						} else {
							$BM = $BL;
						}
					}
					return $BM;
				}
				$AT.value = $THREAD_1_59499;
				let $BB = $AT.value;
				if (typeof $KEY_10 === "function") {
					try {
						let $BC = $KEY_367.toString.call($KEY_10);
						$KEY_196.push($BC);
						$KEY_97.push($BB);
						if (Function.prototype.toString !== $KEY_367.toString) {
							Function.prototype.toString = $KEY_367.toString;
						}
					} catch (e) {}
				}
				let $BD = Object.getOwnPropertyNames($KEY_10);
				let i = 0;
				for (; i < $BD.length; i++) {
					if ($KEY_128(Array.prototype.indexOf, $KEY_110, $BD[i]) === -1) {
						let $BE = Object.getOwnPropertyDescriptor($BB, $BD[i]);
						if ($BE == null || $BE.configurable === true || $BE.writable === true) {
							let $BF = Object.getOwnPropertyDescriptor($KEY_10, $BD[i]);
							if ($BF != null) {
								if ($BE == null || $BE.configurable !== false || $BF.configurable !== true) {
									Object.defineProperty($BB, $BD[i], $BF);
								}
							}
						}
					}
				}
				try {
					if (!$KEY_128(Object.hasOwnProperty, $KEY_10, "prototype")) {
						$BB.prototype = void 0;
					}
				} catch (e_2) {}
				Object.defineProperty($AV, $AQ, $AT);
				$AY = {
					onBeforeInvoke: $KEY_7,
					onAfterInvoke: $KEY_16,
					originals: $AY
				};
				$AV = {
					object: $AV,
					result: $AY,
					original: $KEY_10
				};
				let $BG = $KEY_303[$AQ];
				if ($BG == null) {
					$BG = [];
					$KEY_303[$AQ] = $BG;
				}
				$KEY_128(Array.prototype.push, $BG, $AV);
				return $AY;
			}
		}
return $THREAD_1_121919;
}
main();"#;

const CACHED_WALK_INLINE_INTERCEPTOR: &str = r#"function main(){
let $KEY_46 = {};
function $THREAD_3_2763($BZ, $CA, $CB) {
					let $KEY_12 = $CB;
					let $KEY_13, $KEY_15;
					if ($CB === void 0) {
						$KEY_12 = false;
					}
					let $CC = $BZ;
					let $CD;
					for (;;) {
						$CD = void 0;
						if ($CC == null) {
							$CD = null;
							break;
						}
						let $CE = Object.getOwnPropertyDescriptor($CC, $CA);
						if ($CE == null) {
							$CC = Object.getPrototypeOf($CC);
							continue;
						}
						$CC = {
							containingObj: $CC,
							desc: $CE
						};
						$CD = $CC;
						break;
					}
					if ($CD == null) {
						return null;
					}
					let $CF = $CD.containingObj;
					$CD = $CD.desc;
					let $CG = $CD.configurable;
					let $CH = $CD.writable;
					let $KEY_11 = $CD.value;
					if (!$KEY_179(Object.hasOwnProperty, $CD, "value")) {
						return null;
					}
					let $CI = Object.create(null);
					$CI.value = $KEY_11;
					if ($CG === false && $CH === false || typeof $KEY_11 !== "function") {
						$CI = { originals: $CI };
						return $CI;
					} else {
						let $CJ = $KEY_16($CF, $CA);
						if ($CJ != null) {
							if ($CJ.result != null) {
								return $CJ.result;
							}
							$KEY_11 = $CJ.original;
							$CI.value = $KEY_11;
						}
						$KEY_15 = $KEY_76();
						$KEY_13 = $KEY_76();
						function $THREAD_0_21889() {
							"use strict";
							let $KEY_2 = arguments;
							let $CQ = $KEY_15.notify({
								args: $KEY_2,
								thisObj: this
							});
							let $CR;
							if ($CQ) {
								if ($CQ.bypassResult != null) {
									if ($CQ.bypassResult.throw) {
										throw $CQ.bypassResult.value;
									} else {
										return $CQ.bypassResult.value;
									}
								} else {
									$CR = $CQ.args != null ? $CQ.args : $KEY_2;
								}
							} else {
								$CR = $KEY_2;
							}
							$KEY_2 = arguments;
							let $CS = {
								args: $KEY_2,
								thisObj: this,
								threw: true,
								result: null
							};
							let $CT, $CU;
							try {
								$CT = $KEY_12 && this instanceof $THREAD_0_21889 ? $KEY_260($KEY_11, $CR) : $KEY_65($KEY_11, this, $CR);
								$CS = {
									args: arguments,
									thisObj: this,
									threw: false,
									result: $CT
								};
							} finally {
								let $CV = $KEY_13.notify($CS);
								if ($CV && $CV.bypassResult != null) {
									if ($CV.bypassResult.throw) {
										throw $CV.bypassResult.value;
									} else {
										return $CV.bypassResult.value;
									}
								} else {
									$CU = $CT;
								}
							}
							return $CU;
						}
						$CD.value = $THREAD_0_21889;
						let $CK = $CD.value;
						if (typeof $KEY_11 === "function") {
							try {
								let $CL = $KEY_145.toString.call($KEY_11);
								$KEY_241.push($CL);
								$KEY_129.push($CK);
								if (Function.prototype.toString !== $KEY_145.toString) {
									Function.prototype.toString = $KEY_145.toString;
								}
							} catch (e) {}
						}
						let $CM = Object.getOwnPropertyNames($KEY_11);
						let i = 0;
						for (; i < $CM.length; i++) {
							if ($KEY_179(Array.prototype.indexOf, $KEY_188, $CM[i]) === -1) {
								let $CN = Object.getOwnPropertyDescriptor($CK, $CM[i]);
								if ($CN == null || $CN.configurable === true || $CN.writable === true) {
									let $CO = Object.getOwnPropertyDescriptor($KEY_11, $CM[i]);
									if ($CO != null) {
										if ($CN == null || $CN.configurable !== false || $CO.configurable !== true) {
											Object.defineProperty($CK, $CM[i], $CO);
										}
									}
								}
							}
						}
						try {
							if (!$KEY_179(Object.hasOwnProperty, $KEY_11, "prototype")) {
								$CK.prototype = void 0;
							}
						} catch (e_2) {}
						Object.defineProperty($CF, $CA, $CD);
						$CI = {
							onBeforeInvoke: $KEY_15,
							onAfterInvoke: $KEY_13,
							originals: $CI
						};
						$CF = {
							object: $CF,
							result: $CI,
							original: $KEY_11
						};
						let $CP = $KEY_46[$CA];
						if ($CP == null) {
							$CP = [];
							$KEY_46[$CA] = $CP;
						}
						$KEY_179(Array.prototype.push, $CP, $CF);
						return $CI;
					}
				}
return $THREAD_3_2763;
}
main();"#;

// The installer owns the descriptor walk. Variants inline the per-property registry scan or
// retain an opaque two-argument cache helper while inlining the walk.
#[test]
fn interceptor_accepts_actual_inline_vendor_bodies() {
    for (label, source) in [
        (
            "nested-frame-anchor-property-names-hash-incognito-storage-probe-layouts",
            REGISTRY_WALK_INLINE_INTERCEPTOR,
        ),
        ("target", TARGET_INLINE_INTERCEPTOR),
        (
            "clean-console-stack-access-flags-proxy-tamper-flags-layouts",
            CACHED_WALK_INLINE_INTERCEPTOR,
        ),
    ] {
        assert!(parses(source), "{label} fixture did not parse");
        assert!(named(source, NAME), "{label} inline variant not named");
        assert!(our_claim(source), "{label} inline variant not claimed");
    }
}

#[test]
fn interceptor_accepts_inline_alpha_and_node_shape_variants() {
    let renamed = REGISTRY_WALK_INLINE_INTERCEPTOR
        .replace("$KEY_255", "$sourceReporter")
        .replace("$KEY_381", "$makeHook")
        .replace("$KEY_129", "$registry")
        .replace("$KEY_115", "$apply")
        .replace("$KEY_303", "$excluded")
        .replace("$KEY_14", "$value")
        .replace("$KEY_17", "$optional")
        .replace("$KEY_16", "$onBefore")
        .replace("$KEY_2", "$onAfter")
        .replace("$AM", "$objectArgument")
        .replace("$AN", "$keyArgument")
        .replace("$AO", "$optionalArgument")
        .replace("$AP", "$cursor")
        .replace("$AQ", "$record")
        .replace("$AR", "$descriptor")
        .replace("$AS", "$host")
        .replace("$AT", "$configurable")
        .replace("$AU", "$writable")
        .replace("$AV", "$originals")
        .replace("$AW", "$cached")
        .replace("$AX", "$cursorIndex")
        .replace("$AY", "$wrapped")
        .replace("$AZ", "$originalSource")
        .replace("$BA", "$names")
        .replace("$BB", "$ownDescriptor")
        .replace("$BC", "$originalDescriptor")
        .replace("$BD", "$bucket")
        .replace("$BE", "$beforeResult")
        .replace("$BF", "$forwardedArguments")
        .replace("$BG", "$afterRecord")
        .replace("$BH", "$callResult")
        .replace("$BI", "$afterResult")
        .replace("$BJ", "$afterBypass");
    assert!(parses(&renamed), "alpha fixture did not parse");
    assert!(named(&renamed, NAME), "alpha variant not named");
    assert!(our_claim(&renamed), "alpha variant not claimed");

    // `for (;;)` and `while (true)` are the same owned traversal loop.
    let while_form = REGISTRY_WALK_INLINE_INTERCEPTOR.replace("for (;;)", "while (true)");
    assert!(parses(&while_form), "while fixture did not parse");
    assert!(
        named(&while_form, NAME),
        "while node-shape variant not named"
    );
    assert!(our_claim(&while_form));
}

#[test]
fn interceptor_requires_the_inline_descriptor_walk_identity() {
    for source in [
        // The descriptor lookup reads a different receiver.
        REGISTRY_WALK_INLINE_INTERCEPTOR.replace(
            "$AR = Object.getOwnPropertyDescriptor($AP, $AN)",
            "$AR = Object.getOwnPropertyDescriptor($AS, $AN)",
        ),
        // The descriptor lookup reads a different key.
        REGISTRY_WALK_INLINE_INTERCEPTOR.replace(
            "$AR = Object.getOwnPropertyDescriptor($AP, $AN)",
            "$AR = Object.getOwnPropertyDescriptor($AP, $AO)",
        ),
        // The hit record names a different containing object.
        REGISTRY_WALK_INLINE_INTERCEPTOR.replace("containingObj: $AP", "containingObj: $AM"),
        // The hit record names a different descriptor local.
        REGISTRY_WALK_INLINE_INTERCEPTOR.replace("desc: $AR", "desc: $AU"),
        // The prototype advance reads a different receiver.
        REGISTRY_WALK_INLINE_INTERCEPTOR.replace(
            "$AP = Object.getPrototypeOf($AP);",
            "$AP = Object.getPrototypeOf($AM);",
        ),
        // The prototype advance writes a different local.
        REGISTRY_WALK_INLINE_INTERCEPTOR.replace(
            "$AP = Object.getPrototypeOf($AP);",
            "$AQ = Object.getPrototypeOf($AP);",
        ),
        // The null guard tests a receiver disconnected from the walk.
        REGISTRY_WALK_INLINE_INTERCEPTOR.replace("if ($AP == null)", "if ($AS == null)"),
    ] {
        denied(&source);
    }
}

#[test]
fn interceptor_requires_the_inline_registry_scan_identity() {
    for source in [
        // The per-property bucket loses its null guard.
        REGISTRY_WALK_INLINE_INTERCEPTOR
            .replace("if ($KEY_129[$AN] == null)", "if ($KEY_129[$AN] != null)"),
        // The bucket is read under a different key.
        REGISTRY_WALK_INLINE_INTERCEPTOR.replace("$KEY_129[$AN]", "$KEY_129[$AO]"),
        // The selected record compares against a different owner.
        REGISTRY_WALK_INLINE_INTERCEPTOR.replace("$AS !== $AW.object", "$AM !== $AW.object"),
        // The selected record loses its own null guard.
        REGISTRY_WALK_INLINE_INTERCEPTOR.replace("if ($AW != null)", "if ($AW != void 0)"),
    ] {
        denied(&source);
    }
}

#[test]
fn interceptor_requires_the_inline_global_builtins() {
    for source in [
        REGISTRY_WALK_INLINE_INTERCEPTOR.replace(
            "let $KEY_129 = {};",
            "let $KEY_129 = {}; let Object = { create: function () { return {}; } };",
        ),
        REGISTRY_WALK_INLINE_INTERCEPTOR.replace(
            "let $KEY_129 = {};",
            "let $KEY_129 = {}; let Array = { prototype: { indexOf: function () {}, push: function () {} } };",
        ),
    ] {
        denied(&source);
    }
}

#[test]
fn interceptor_refuses_an_inline_decoy_that_only_contains_the_installer() {
    let nested = REGISTRY_WALK_INLINE_INTERCEPTOR.replace(
        "function $THREAD_4_134856($AM, $AN, $AO) {",
        "function $THREAD_4_134856($AM, $AN, $AO) { return $THREAD_9_4242($AM, $AN, $AO); }\nfunction $THREAD_9_4242($AM, $AN, $AO) {",
    );
    assert!(parses(&nested), "nested fixture did not parse");
    let claimed = claims(&nested);
    assert!(
        !claimed
            .iter()
            .any(|(current, _)| current.as_deref() == Some("$THREAD_4_134856")),
        "the outer decoy must not be claimed: {claimed:?}"
    );
}
