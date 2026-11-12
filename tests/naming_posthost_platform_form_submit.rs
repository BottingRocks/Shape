//! Focused coverage for the platform form-submit factory style capture.
//!
//! The base gate recognizes document.createElement("style"). The tuple variant lowers that
//! native call to `[document, document.createElement][1].call([document,
//! document.createElement][0], "style")` with stable reaching definitions. Tests retain the
//! direct form, prove the tuple and exact factory body, and refuse identity, order, and
//! mutation near misses. Evidence uses runtime role evaluator strings
//! (`function_finder::<role>`); the package label is not an evaluator.

use shape::naming::name_threads;

const FACTORY_EVALUATOR: &str = "function_finder::create_platform_form_submit_hook";
const INIT_EVALUATOR: &str = "function_finder::init_submit_signal_hooks";
const TEARDOWN_EVALUATOR: &str = "function_finder::teardown_submit_instrumentation";
const FACTORY: &str = "createPlatformFormSubmitHook";
const INIT: &str = "initSubmitSignalHooks";
const TEARDOWN: &str = "teardownSubmitInstrumentation";

const TUPLE: &str = r#"function main(){
  function $THREAD_1_100($BR){
    let $KEY_2=$BR;
    let $KEY_6=$KEY_2.global;
    let $KEY_4=$KEY_6.document;
    let $BS=[$KEY_4,$KEY_4.createElement];
    let $KEY_8=$BS[1].call($BS[0],"style");
    function $THREAD_2_200(){use("formaction","formenctype","formmethod");}
    function $THREAD_3_300(){$KEY_4.head.removeChild($KEY_8);}
    return {init:$THREAD_2_200,teardown:$THREAD_3_300};
  }
  return $THREAD_1_100;
}main();"#;

const DIRECT: &str = r#"function main(){
  function $THREAD_1_100($BR){
    let $KEY_2=$BR;
    let $KEY_6=$KEY_2.global;
    let $KEY_4=$KEY_6.document;
    let $KEY_8=$KEY_4.createElement("style");
    function $THREAD_2_200(){use("formaction","formenctype","formmethod");}
    function $THREAD_3_300(){$KEY_4.head.removeChild($KEY_8);}
    return {init:$THREAD_2_200,teardown:$THREAD_3_300};
  }
  return $THREAD_1_100;
}main();"#;

/// Verbatim createPlatformFormSubmitHook factory body in a callable carrier. Coverage is
/// bounded to this function and does not read the full input.
const ACTUAL_STYLE_TUPLE_BODY: &str = r#"function $THREAD_6_59714($BR) {
			let $KEY_2 = $BR;
			let $KEY_4, $KEY_6, $KEY_8, $KEY_12, $KEY_14, $KEY_15;
			function $THREAD_4_58250($BT) {
				$KEY_14 = $BT.target;
				$KEY_6.setTimeout(() => {
					$KEY_14 = null;
				}, 0);
			}
			let $KEY_1 = $THREAD_4_58250;
			function $THREAD_4_80184($BT) {
				let $KEY_0 = $BT;
				if ($KEY_12 != null) {
					Object.keys($KEY_12).forEach($BU => {
						let $BV;
						if ($KEY_0.elements.length !== 0) {
							let $BW = [$KEY_0, $KEY_0.compareDocumentPosition];
							$BV = $BW[1].call($BW[0], $KEY_0.elements[0]) & Node.DOCUMENT_POSITION_CONTAINED_BY;
						} else {
							$BV = false;
						}
						let $BX = ($BV ? $KEY_0 : $KEY_4.body).querySelector;
						$BV = $BX.call($BV ? $KEY_0 : $KEY_4.body, "input[type=hidden][name=\"" + ("EWcll5PZpa-" + $BU) + "\"]");
						if ($BV && $BV.parentNode != null) {
							$BV.parentNode.removeChild($BV);
						}
					});
				}
			}
			$KEY_15 = $THREAD_4_80184;
			function $THREAD_9_123157($BT) {
				$KEY_200({ message: $BT });
			}
			let $KEY_0 = $THREAD_9_123157;
			function $THREAD_2_123461() {
				$KEY_368.register($BT => {
					let $BU = $BT.event?.submitter;
					$BU = $BU == null && $KEY_14 != null ? $KEY_14 : $BU;
					let $KEY_0 = $BU == null ? null : $KEY_6.HTMLFormElement.prototype.getAttribute.call($BU, "formaction");
					$KEY_0 = $KEY_0 == null || $KEY_0 === "" ? $BT.form == null ? null : $KEY_6.HTMLFormElement.prototype.getAttribute.call($BT.form, "action") : $KEY_0;
					let $KEY_1 = $BU == null ? null : $KEY_6.HTMLFormElement.prototype.getAttribute.call($BU, "formenctype");
					let $BV;
					if ($KEY_1 != null) {
						$BV = $KEY_1 !== "";
					} else {
						$BV = $KEY_1 != null;
					}
					$BV = $BV ? $BU?.formEnctype ?? "" : $KEY_1;
					$BV = $BV == null || $BV === "" ? $BT.form.enctype : $BV;
					$BU = $BU == null ? null : $KEY_6.HTMLFormElement.prototype.getAttribute.call($BU, "formmethod");
					$BU = $BU == null || $BU === "" ? $BT.form == null ? null : $KEY_6.HTMLFormElement.prototype.getAttribute.call($BT.form, "method") : $BU;
					if ($BV === "application/x-www-form-urlencoded" || $BV === "multipart/form-data") {
						$KEY_0 = {
							url: $KEY_0,
							method: void 0
						};
						let $BW;
						if ($BU == null) {
							$BW = "get";
						} else {
							let $BX = $BU.toLowerCase();
							$BW = $BX === "get" || $BX === "post" || $BX === "dialog" ? $BX : "get";
						}
						$KEY_0.method = $BW;
						if ($KEY_0.method !== "get" && $KEY_178($KEY_0)) {
							if ($KEY_12 == null) {
								let $BY = $KEY_415();
								if ($KEY_19 != null) {
									$BY[$KEY_19] = $KEY_107($BY);
								}
								$KEY_12 = $BY;
								if ($BY != null) {
									$KEY_6.setTimeout(() => {
										$KEY_12 = null;
									}, 0);
									let $BZ = "x" + Math.random().toString(36).slice(2);
									$KEY_8.type = "text/css";
									$KEY_8.nonce = $KEY_2.csp.styleNonce;
									$KEY_8.textContent = "." + $BZ + "{display:none !important;}";
									if ($KEY_4.head) {
										$KEY_4.head.appendChild($KEY_8);
									}
									if ($KEY_12 !== null && $KEY_12 !== void 0) {
										for (const i in $KEY_12) {
											if (!!Object.prototype.hasOwnProperty.call($KEY_12, i)) {
												let $CB = [$BT.form, $BT.form.querySelector];
												let $CC = $CB[1].call($CB[0], "input[type=hidden][name=\"" + ("EWcll5PZpa-" + i) + "\"]");
												if ($CC == null) {
													let $CD = $KEY_4.createElement("input");
													$CD.setAttribute("type", "hidden");
													$CD.setAttribute("hidden", "true");
													$CD.setAttribute("name", "EWcll5PZpa-" + i);
													$CD.setAttribute("value", $KEY_12[i]);
													$CD.className = $BZ;
													let $CE;
													if ($BT.form.elements.length !== 0) {
														let $CF = [$BT.form, $BT.form.compareDocumentPosition];
														$CE = $CF[1].call($CF[0], $BT.form.elements[0]) & Node.DOCUMENT_POSITION_CONTAINED_BY;
													} else {
														$CE = false;
													}
													if ($CE) {
														$BT.form.insertBefore($CD, $BT.form.children[0]);
													} else {
														let $CG = $BT.form.id;
														if (!$CG) {
															$CG = "x" + Math.random().toString(36).slice(2);
															$BT.form.id = $CG;
														}
														$CD.setAttribute("form", $CG);
														$KEY_4.body.insertBefore($CD, $KEY_4.body.firstChild);
													}
												} else {
													$CC.setAttribute("value", $KEY_12[i]);
												}
											}
										}
									}
								}
								return void 0;
							}
							let $CH = "x" + Math.random().toString(36).slice(2);
							$KEY_8.type = "text/css";
							$KEY_8.nonce = $KEY_2.csp.styleNonce;
							$KEY_8.textContent = "." + $CH + "{display:none !important;}";
							if ($KEY_4.head) {
								$KEY_4.head.appendChild($KEY_8);
							}
							if ($KEY_12 !== null && $KEY_12 !== void 0) {
								for (const j in $KEY_12) {
									if (!!Object.prototype.hasOwnProperty.call($KEY_12, j)) {
										let $CJ = [$BT.form, $BT.form.querySelector];
										let $CK = $CJ[1].call($CJ[0], "input[type=hidden][name=\"" + ("EWcll5PZpa-" + j) + "\"]");
										if ($CK == null) {
											let $CL = $KEY_4.createElement("input");
											$CL.setAttribute("type", "hidden");
											$CL.setAttribute("hidden", "true");
											$CL.setAttribute("name", "EWcll5PZpa-" + j);
											$CL.setAttribute("value", $KEY_12[j]);
											$CL.className = $CH;
											let $CM;
											if ($BT.form.elements.length !== 0) {
												let $CN = [$BT.form, $BT.form.compareDocumentPosition];
												$CM = $CN[1].call($CN[0], $BT.form.elements[0]) & Node.DOCUMENT_POSITION_CONTAINED_BY;
											} else {
												$CM = false;
											}
											if ($CM) {
												$BT.form.insertBefore($CL, $BT.form.children[0]);
											} else {
												let $CO = $BT.form.id;
												if (!$CO) {
													$CO = "x" + Math.random().toString(36).slice(2);
													$BT.form.id = $CO;
												}
												$CL.setAttribute("form", $CO);
												$KEY_4.body.insertBefore($CL, $KEY_4.body.firstChild);
											}
										} else {
											$CK.setAttribute("value", $KEY_12[j]);
										}
									}
								}
							}
						}
					}
				});
				$KEY_70.register($KEY_15);
				$KEY_412.register($KEY_0);
				let $KEY_9 = typeof SubmitEvent !== "undefined" && RegExp("Safari").test($KEY_6.navigator.userAgent) && !CSS.supports("appearance", "auto");
				if (typeof SubmitEvent === "undefined" || $KEY_9) {
					$KEY_4.addEventListener("click", $KEY_1);
				}
			}
			function $THREAD_0_87198() {
				try {
					$KEY_4.removeEventListener("click", $KEY_1);
					if (!!$KEY_351) {
						$KEY_368.clear();
						$KEY_70.clear();
						$KEY_198.clear();
						$KEY_412.clear();
						window.document.removeEventListener("submit", $KEY_405);
						if (window.navigation != null) {
							window.navigation.removeEventListener("navigate", $KEY_47);
						}
						$KEY_351 = false;
					}
					if ($KEY_4.head) {
						$KEY_4.head.removeChild($KEY_8);
					}
				} catch (e) {}
			}
			$KEY_6 = $KEY_2.global;
			$KEY_4 = $KEY_6.document;
			let $BS = [$KEY_4, $KEY_4.createElement];
			$KEY_8 = $BS[1].call($BS[0], "style");
			$KEY_14 = null;
			return {
				init: $THREAD_2_123461,
				teardown: $THREAD_0_87198
			};
		}"#;

fn roles(source: &str) -> Vec<String> {
    name_threads(source, true)
        .unwrap()
        .report
        .function_roles
        .iter()
        .filter(|role| {
            role.evaluators.iter().any(|evaluator| {
                evaluator.as_str() == FACTORY_EVALUATOR
                    || evaluator.as_str() == INIT_EVALUATOR
                    || evaluator.as_str() == TEARDOWN_EVALUATOR
            })
        })
        .flat_map(|role| role.candidates.iter().cloned())
        .collect()
}

fn assert_family(source: &str) {
    let found = roles(source);
    for expected in [FACTORY, INIT, TEARDOWN] {
        assert_eq!(
            found
                .iter()
                .filter(|name| name.as_str() == expected)
                .count(),
            1,
            "{expected}: {found:?}"
        );
    }
}

/// Apply a required mutation and fail loudly if the fixture text no longer
/// matches, so a near miss can never silently degrade into the positive.
fn mutate(source: &str, from: &str, to: &str) -> String {
    assert!(source.contains(from), "fixture fragment missing: {from}");
    let mutated = source.replace(from, to);
    assert_ne!(mutated, source, "mutation was a no-op: {from}");
    mutated
}

fn actual_style_tuple() -> String {
    format!("function main(){{{ACTUAL_STYLE_TUPLE_BODY}return $THREAD_6_59714;}}main();")
}

#[test]
fn direct_base_captured_tuple_and_alpha_are_claimed() {
    assert_family(DIRECT);
    assert_family(TUPLE);

    let alpha = mutate(TUPLE, "let $KEY_2=$BR;", "let $KEY_2=arguments[0];");
    assert!(alpha.contains("arguments[0]"));
    assert_family(&alpha);
}

#[test]
fn verbatim_style_tuple_factory_body_is_claimed() {
    let actual = actual_style_tuple();
    // Pin the actual straight-line tail: initialized declarators would prove
    // only the normalized shape, while the witness has uninitialized
    // declarators plus later assignments before the tuple.
    assert!(actual.contains("let $KEY_4, $KEY_6, $KEY_8, $KEY_12, $KEY_14, $KEY_15;"));
    assert!(actual.contains("$KEY_6 = $KEY_2.global;"));
    assert!(actual.contains("$KEY_4 = $KEY_6.document;"));
    assert!(actual.contains("let $BS = [$KEY_4, $KEY_4.createElement];"));
    assert!(actual.contains("$KEY_8 = $BS[1].call($BS[0], \"style\");"));
    assert_family(&actual);
}

#[test]
fn branch_and_later_tuple_definitions_do_not_dominate() {
    let branch = mutate(
        TUPLE,
        "let $BS=[$KEY_4,$KEY_4.createElement];\n    let $KEY_8=",
        "let $BS;\n    if ($KEY_4.head) { $BS=[$KEY_4,$KEY_4.createElement]; }\n    let $KEY_8=",
    );
    assert!(branch.contains("if ($KEY_4.head) { $BS=[$KEY_4,$KEY_4.createElement]; }"));
    assert!(roles(&branch).is_empty(), "{branch}");

    let later = mutate(
        TUPLE,
        "let $BS=[$KEY_4,$KEY_4.createElement];\n    let $KEY_8=$BS[1].call($BS[0],\"style\");",
        "let $BS;\n    let $KEY_8=$BS[1].call($BS[0],\"style\");\n    $BS=[$KEY_4,$KEY_4.createElement];",
    );
    assert!(later.contains("$BS=[$KEY_4,$KEY_4.createElement];"));
    assert!(roles(&later).is_empty(), "{later}");
}

#[test]
fn foreign_receiver_overwrites_and_shadowed_native_are_refused() {
    let foreign_receiver = mutate(
        TUPLE,
        "let $BS=[$KEY_4,$KEY_4.createElement];",
        "let $BS=[$KEY_2,$KEY_4.createElement];",
    );
    assert!(roles(&foreign_receiver).is_empty(), "{foreign_receiver}");

    let overwritten_method = mutate(
        TUPLE,
        "let $KEY_8=$BS[1].call($BS[0],\"style\");",
        "$BS[1]=other; let $KEY_8=$BS[1].call($BS[0],\"style\");",
    );
    assert!(roles(&overwritten_method).is_empty(), "{overwritten_method}");

    let alias_overwrite = mutate(
        TUPLE,
        "let $KEY_8=$BS[1].call($BS[0],\"style\");",
        "let $ALIAS=$BS; $ALIAS[1]=other; let $KEY_8=$BS[1].call($BS[0],\"style\");",
    );
    assert!(alias_overwrite.contains("$ALIAS[1]=other;"));
    assert!(roles(&alias_overwrite).is_empty(), "{alias_overwrite}");

    let shadowed_native = mutate(
        TUPLE,
        "let $BS=[$KEY_4,$KEY_4.createElement];",
        "$KEY_4.createElement=other; let $BS=[$KEY_4,$KEY_4.createElement];",
    );
    assert!(roles(&shadowed_native).is_empty(), "{shadowed_native}");
}

#[test]
fn unrelated_nested_tuple_is_refused() {
    let unrelated_nested = mutate(
        TUPLE,
        "let $BS=[$KEY_4,$KEY_4.createElement];\n    let $KEY_8=$BS[1].call($BS[0],\"style\");",
        "let $KEY_8=null; function $THREAD_4_400(){let $OTHER=[$KEY_4,$KEY_4.createElement]; return $OTHER[1].call($OTHER[0],\"style\");}",
    );
    assert!(roles(&unrelated_nested).is_empty(), "{unrelated_nested}");
}

#[test]
fn noop_lifecycle_is_refused_without_removing_form_option_evidence() {
    // Move the three required form-option literals into the surviving
    // teardown, then make only the init body a no-op. The style gate and the
    // form-string gate still pass, so the refusal comes from the lifecycle
    // non-noop requirement rather than from a deleted literal.
    let with_teardown_evidence = mutate(
        TUPLE,
        "function $THREAD_3_300(){$KEY_4.head.removeChild($KEY_8);}",
        "function $THREAD_3_300(){use(\"formaction\",\"formenctype\",\"formmethod\");$KEY_4.head.removeChild($KEY_8);}",
    );
    let no_op = mutate(
        &with_teardown_evidence,
        "function $THREAD_2_200(){use(\"formaction\",\"formenctype\",\"formmethod\");}",
        "function $THREAD_2_200(){return void 0;}",
    );
    assert!(no_op.contains("function $THREAD_2_200(){return void 0;}"));
    assert!(no_op.contains("use(\"formaction\",\"formenctype\",\"formmethod\");$KEY_4.head.removeChild"));
    assert!(roles(&no_op).is_empty(), "{no_op}");
}
