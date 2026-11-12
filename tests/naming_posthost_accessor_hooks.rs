use shape::naming::name_threads;

// Fast-loop family: accessor descriptor interceptor (get/set instrumentation).
// The outer interceptor is the only claimed function; the owned getter/setter
// wrappers are re-proved but belong to the purpose-finder callback family.
const EVALUATOR: &str = "posthost_accessor_hooks";
const NAME: &str = "installAccessorInterceptor";

const WITNESS: &str = r##"function $THREAD_7_102041($BH, $BI) {
					let $KEY_2 = $KEY_100($BH, $BI);
					if ($KEY_2 == null) {
						return null;
					}
					let $BJ = $KEY_2.containingObj;
					$KEY_2 = $KEY_2.desc;
					let $KEY_5 = $KEY_2.configurable;
					let $KEY_14 = $KEY_2.get;
					let $KEY_0 = $KEY_2.set;
					let $BK = $KEY_197(Object.hasOwnProperty, $KEY_2, "value");
					let $BL = Object.create(null);
					if ($KEY_5 === false || $BK) {
						if ($KEY_14 != null) {
							$BL.get = $KEY_14;
						}
						if ($KEY_0 != null) {
							$BL.set = $KEY_0;
						}
						$BL = { originals: $BL };
						return $BL;
					} else {
						let $BM = $KEY_315($BJ, $BI);
						if ($BM != null) {
							if ($BM.result != null) {
								return $BM.result;
							}
							$KEY_14 = $BM.original.get;
							$KEY_0 = $BM.original.set;
						}
						let $BN = {
							onAfterGet: void 0,
							onBeforeGet: void 0,
							onAfterSet: void 0,
							onBeforeSet: void 0,
							originals: {}
						};
						if ($KEY_14 != null) {
							$BL.get = $KEY_14;
							let $KEY_1 = $KEY_186();
							let $KEY_4 = $KEY_186();
							function $THREAD_3_150630() {
								"use strict";
								let $BS = $KEY_1.notify({ thisObj: this });
								if (!$BS || $BS.bypassResult == null) {
									let $BT = {
										thisObj: this,
										result: null,
										threw: true
									};
									let $BU;
									let $BV = $BT;
									let $BW;
									try {
										$BU = $KEY_197($KEY_14, this);
										$BV = {
											thisObj: this,
											result: $BU,
											threw: false
										};
									} finally {
										let $BY = $KEY_4.notify($BV);
										if ($BY && $BY.bypassResult != null) {
											if ($BY.bypassResult.throw) {
												throw $BY.bypassResult.value;
											} else {
												return $BY.bypassResult.value;
											}
										} else {
											$BW = $BU;
										}
									}
									return $BW;
								}
								if ($BS.bypassResult.throw) {
									throw $BS.bypassResult.value;
								} else {
									return $BS.bypassResult.value;
								}
							}
							$KEY_2.get = $THREAD_3_150630;
							if (typeof $KEY_14 === "function") {
								try {
									let $BO = $KEY_49.toString.call($KEY_14);
									$KEY_93.push($BO);
									$KEY_252.push($KEY_2.get);
									if (Function.prototype.toString !== $KEY_49.toString) {
										Function.prototype.toString = $KEY_49.toString;
									}
								} catch (e) {}
							}
							try {
								if (!$KEY_197(Object.hasOwnProperty, $KEY_14, "prototype")) {
									$KEY_2.get.prototype = void 0;
								}
							} catch (e_2) {}
							$BN.onBeforeGet = $KEY_1;
							$BN.onAfterGet = $KEY_4;
						}
						if ($KEY_0 != null) {
							$BL.set = $KEY_0;
							let $KEY_11 = $KEY_186();
							let $KEY_8 = $KEY_186();
							function $THREAD_4_36154($BR) {
								"use strict";
								let $BT = $KEY_11.notify({
									param: $BR,
									thisObj: this
								});
								if ($BT) {
									if ($BT.bypassResult != null) {
										if ($BT.bypassResult.throw) {
											throw $BT.bypassResult.value;
										} else {
											return $BT.bypassResult.value;
										}
									} else {
										$BT = $KEY_197(Object.hasOwnProperty, $BT, "param") ? $BT.param : $BR;
									}
								} else {
									$BT = $BR;
								}
								let $BU = {
									param: $BR,
									thisObj: this,
									result: null,
									threw: true
								};
								let $BV;
								let $BW = $BU;
								let $BX;
								try {
									$BV = $KEY_197($KEY_0, this, $BT);
									$BW = {
										param: $BR,
										thisObj: this,
										result: $BV,
										threw: false
									};
								} finally {
									let $BZ = $KEY_8.notify($BW);
									if ($BZ && $BZ.bypassResult != null) {
										if ($BZ.bypassResult.throw) {
											throw $BZ.bypassResult.value;
										} else {
											return $BZ.bypassResult.value;
										}
									} else {
										$BX = $BV;
									}
								}
								return $BX;
							}
							$KEY_2.set = $THREAD_4_36154;
							if (typeof $KEY_0 === "function") {
								try {
									let $BP = $KEY_49.toString.call($KEY_0);
									$KEY_93.push($BP);
									$KEY_252.push($KEY_2.set);
									if (Function.prototype.toString !== $KEY_49.toString) {
										Function.prototype.toString = $KEY_49.toString;
									}
								} catch (e_3) {}
							}
							try {
								if (!$KEY_197(Object.hasOwnProperty, $KEY_0, "prototype")) {
									$KEY_2.set.prototype = void 0;
								}
							} catch (e_4) {}
							$BN.onBeforeSet = $KEY_11;
							$BN.onAfterSet = $KEY_8;
						}
						Object.defineProperty($BJ, $BI, $KEY_2);
						$BN.originals = $BL;
						$BJ = { object: $BJ };
						$BJ.result = $BN;
						$BJ.original = $BL;
						let $BQ = $KEY_362[$BI];
						if ($BQ == null) {
							$BQ = [];
							$KEY_362[$BI] = $BQ;
						}
						$KEY_197(Array.prototype.push, $BQ, $BJ);
						return $BN;
					}
				}"##;

fn fixture(body: &str) -> String {
    format!("function main(){{\n{body}\nreturn $THREAD_7_102041;\n}}\nmain();")
}

fn fixture_named(body: &str, name: &str) -> String {
    format!("function main(){{\n{body}\nreturn {name};\n}}\nmain();")
}

// Inline lookup snippets are spliced into the accepted base skeleton. They cover registry
// and descriptor-walk variants; only opaque local spellings are portable.
const INLINE_LOOKUP: &str = r#"let $X_CURSOR = $BH;let $KEY_2;for (;;) {$KEY_2 = void 0;if ($X_CURSOR == null) {$KEY_2 = null;break;}let $X_DESC = Object.getOwnPropertyDescriptor($X_CURSOR, $BI);if ($X_DESC == null) {$X_CURSOR = Object.getPrototypeOf($X_CURSOR);continue;}$KEY_2 = { containingObj: $X_CURSOR, desc: $X_DESC };break;}"#;

const INLINE_LOOKUP_CURSOR: &str = r#"let $X_CURSOR = $BH;let $X_RECORD, $KEY_2;for (;;) {$X_RECORD = void 0;$KEY_2 = void 0;if ($X_CURSOR == null) {$X_RECORD = null;$KEY_2 = null;break;}let $X_DESC = Object.getOwnPropertyDescriptor($X_CURSOR, $BI);if ($X_DESC == null) {$X_CURSOR = Object.getPrototypeOf($X_CURSOR);continue;}$X_CURSOR = { containingObj: $X_CURSOR, desc: $X_DESC };$X_RECORD = $X_CURSOR;$KEY_2 = $X_CURSOR;break;}"#;

const INLINE_REGISTRY_TWO: &str = r#"let $X_CACHED;let $BM;if ($KEY_362[$BI] == null) {$BM = null;$X_CACHED = null;}else {let $X_INDEX = 0;for (;;) {$BM = void 0;if ($X_INDEX >= $KEY_362[$BI].length) {$BM = null;break;}$BM = $KEY_362[$BI][$X_INDEX];if ($BJ !== $BM.object) {++$X_INDEX;continue;}break;}$X_CACHED = $BM;}"#;

const INLINE_REGISTRY_ONE: &str = r#"let $BM;if ($KEY_362[$BI] == null) {$BM = null;}else {let $X_INDEX = 0;for (;;) {if ($X_INDEX >= $KEY_362[$BI].length) {$BM = null;break;}$BM = $KEY_362[$BI][$X_INDEX];if ($BJ !== $BM.object) {++$X_INDEX;continue;}break;}}"#;

const INLINE_REGISTRY_TRUTH: &str = r#"let $X_RECORD, $X_FOUND;if ($KEY_362[$BI] == null) {$X_FOUND = false;$X_RECORD = null;}else {let $X_INDEX = 0;for (;;) {$X_RECORD = void 0;if ($X_INDEX >= $KEY_362[$BI].length) {$X_RECORD = null;break;}$X_RECORD = $KEY_362[$BI][$X_INDEX];if ($BJ !== $X_RECORD.object) {++$X_INDEX;continue;}break;}$X_FOUND = $X_RECORD != null;}"#;

fn inline_descriptor(base: &str) -> String {
    base.replace("let $KEY_2 = $KEY_100($BH, $BI);", INLINE_LOOKUP)
}

fn inline_descriptor_cursor(base: &str) -> String {
    base.replace("let $KEY_2 = $KEY_100($BH, $BI);", INLINE_LOOKUP_CURSOR)
        .replace("if ($KEY_2 == null) {", "if ($X_RECORD == null) {")
}

fn inline_registry_two(base: &str) -> String {
    base.replace("$BM.result", "$X_CACHED.result")
        .replace("$BM.original", "$X_CACHED.original")
        .replace("let $BM = $KEY_315($BJ, $BI);", INLINE_REGISTRY_TWO)
}

fn inline_registry_one(base: &str) -> String {
    base.replace("let $BM = $KEY_315($BJ, $BI);", INLINE_REGISTRY_ONE)
}

fn inline_registry_truth(base: &str) -> String {
    base.replace("if ($BM != null) {", "if ($X_FOUND) {")
        .replace("$BM.result", "$X_RECORD.result")
        .replace("$BM.original", "$X_RECORD.original")
        .replace("let $BM = $KEY_315($BJ, $BI);", INLINE_REGISTRY_TRUTH)
}

/// The full inline variant as one carrier function. Callers must splice it into
/// exactly one `fixture_named` wrapper; adding a second wrapper would create two
/// equally broad `main` scopes and the properties model would refuse it.
fn inline_full() -> String {
    inline_registry_two(&inline_descriptor(WITNESS))
}

fn named(source: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result
            .report
            .outcomes
            .iter()
            .any(|outcome| outcome.applied && outcome.proposed_name.as_deref() == Some(NAME))
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

fn parses(source: &str) -> bool {
    name_threads(source, true).is_ok()
}

fn denied(source: &str) {
    assert!(parses(source), "fixture did not parse: {source}");
    assert!(!named(source), "unexpected {NAME} for: {source}");
    assert!(
        !our_claim(source),
        "unexpected posthost claim for: {source}"
    );
}

fn base() -> String {
    fixture(WITNESS)
}

/// Structural variants cover spelling, declaration grouping, aliased or direct event
/// records, and inline, complete-literal, or stepped registration records.
#[test]
fn accepts_portable_family_shapes() {
    let base = base();
    // Respell the generated names too: they stay valid rename candidates, and
    // only symbol identity may decide the family.
    let renamed = base
        .replace("$KEY_", "$HK")
        .replace("$THREAD_7_102041", "$THREAD_90_777_2")
        .replace("$THREAD_3_150630", "$THREAD_42_31337")
        .replace("$THREAD_4_36154", "$THREAD_43_31337")
        .replace("$BH", "target")
        .replace("$BI", "propertyName");
    let variable = base.replace("let ", "var ");
    let grouped = base
        .replace(
            "let $KEY_5 = $KEY_2.configurable;",
            "let $KEY_5 = $KEY_2.configurable, $KEY_14 = $KEY_2.get;",
        )
        .replace("let $KEY_14 = $KEY_2.get;", "")
        .replace(
            "let $KEY_1 = $KEY_186();",
            "let $KEY_1 = $KEY_186(), $KEY_4 = $KEY_186();",
        )
        .replace("let $KEY_4 = $KEY_186();", "");
    let no_alias = base
        .replace("let $BV = $BT;", "")
        .replace("$BV = {", "$BT = {")
        .replace("$KEY_4.notify($BV)", "$KEY_4.notify($BT)")
        .replace("let $BW = $BU;", "")
        .replace("$BW = {", "$BU = {")
        .replace("$KEY_8.notify($BW)", "$KEY_8.notify($BU)");
    let inline = base
        .replace("$BJ = { object: $BJ };", "")
        .replace("$BJ.result = $BN;", "")
        .replace("$BJ.original = $BL;", "")
        .replace(
            "$KEY_197(Array.prototype.push, $BQ, $BJ);",
            "$KEY_197(Array.prototype.push, $BQ, { object: $BJ, result: $BN, original: $BL });",
        );
    let full_literal = base
        .replace(
            "$BJ = { object: $BJ };",
            "$BJ = { object: $BJ, result: $BN, original: $BL };",
        )
        .replace("$BJ.result = $BN;", "")
        .replace("$BJ.original = $BL;", "");

    for (label, source) in [
        ("base", base),
        ("renamed", renamed),
        ("variable", variable),
        ("grouped", grouped),
        ("no_alias", no_alias),
        ("inline", inline),
        ("full_literal", full_literal),
    ] {
        assert!(parses(&source), "{label} did not parse");
        assert!(named(&source), "{label} was not named");
        assert!(our_claim(&source), "{label} was not our claim");
    }
}

#[test]
fn claims_only_the_outer_interceptor() {
    let result = name_threads(&base(), true).unwrap();
    let claimed: Vec<_> = result
        .report
        .function_roles
        .iter()
        .filter(|role| role.evaluators.iter().any(|e| e == EVALUATOR))
        .collect();
    assert_eq!(claimed.len(), 1, "expected exactly the outer interceptor");
    assert_eq!(claimed[0].current_name.as_deref(), Some("$THREAD_7_102041"));
    for callback in ["$THREAD_3_150630", "$THREAD_4_36154"] {
        let role = result
            .report
            .function_roles
            .iter()
            .find(|role| role.current_name.as_deref() == Some(callback))
            .expect("the owned wrapper must exist");
        assert!(
            !role.evaluators.iter().any(|e| e == EVALUATOR),
            "{callback} belongs to the callback family"
        );
    }
}

#[test]
fn denies_wrong_operations_receivers_and_shadowing() {
    let base = base();
    let cases = [
        // Wrong descriptor installed on the owner.
        base.replace(
            "Object.defineProperty($BJ, $BI, $KEY_2)",
            "Object.defineProperty($BJ, $BI, $BL)",
        ),
        // A getter wrapper installed on the setter slot.
        base.replace(
            "$KEY_2.get = $THREAD_3_150630;",
            "$KEY_2.set = $THREAD_3_150630;",
        ),
        // The after-observer replaced by the before-observer.
        base.replace("$KEY_4.notify($BV)", "$KEY_1.notify($BV)"),
        // A different own-property predicate.
        base.replace("Object.hasOwnProperty", "Object.propertyIsEnumerable"),
        // The getter wrapper invokes the setter accessor.
        base.replace("$KEY_197($KEY_14, this)", "$KEY_197($KEY_0, this)"),
        // A non-null original descriptor.
        base.replace("Object.create(null)", "Object.create({})"),
        // A different array operation.
        base.replace("Array.prototype.push", "Array.prototype.unshift"),
        // The getter block's null guard is inverted.
        base.replace("if ($KEY_14 != null) {", "if ($KEY_14 == null) {"),
        // An extra nested function breaks the owned body.
        base.replace(
            "let $BL = Object.create(null);",
            "let $BL = Object.create(null);function $THREAD_99_1(){}",
        ),
        // A method-interceptor hook key in the accessor record.
        base.replace("onBeforeGet: void 0", "onBeforeInvoke: void 0"),
        // The hook wiring swaps the before/after observers.
        base.replace("$BN.onBeforeGet = $KEY_1;", "$BN.onBeforeGet = $KEY_4;")
            .replace("$BN.onAfterGet = $KEY_4;", "$BN.onAfterGet = $KEY_1;"),
        // The outer interceptor loses its property formal.
        base.replace(
            "function $THREAD_7_102041($BH, $BI)",
            "function $THREAD_7_102041($BH)",
        ),
        // A different descriptor field is read.
        base.replace(
            "let $BJ = $KEY_2.containingObj;",
            "let $BJ = $KEY_2.object;",
        ),
        base.replace("$KEY_2 = $KEY_2.desc;", "$KEY_2 = $KEY_2.descriptor;"),
        // The registration record points at the original buffer instead of the
        // hook record.
        base.replace("$BJ.result = $BN;", "$BJ.result = $BL;"),
        // The bypass field is misspelled inside the getter wrapper.
        base.replace(
            "if ($BY.bypassResult.throw)",
            "if ($BY.bypassResult.failed)",
        ),
        // The same wrapper is installed for both accessors.
        base.replace(
            "$KEY_2.set = $THREAD_4_36154;",
            "$KEY_2.set = $THREAD_3_150630;",
        ),
    ];
    for source in cases {
        denied(&source);
    }

    // The global `Object` is shadowed in the owning scope.
    denied(&base.replace(
        "function main(){",
        "function main(){let Object={create:function(){return {};},hasOwnProperty:function(){return true;}};",
    ));

    // A decoy that writes the four hook keys but is not the interceptor.
    denied(
        "function main(){function $THREAD_9_1(){let hooks={};hooks.onBeforeGet=1;hooks.onAfterGet=2;hooks.onBeforeSet=3;hooks.onAfterSet=4;return hooks;}return $THREAD_9_1;}main();",
    );
}

#[test]
fn preserves_human_names_and_collision_guards() {
    let human = base().replace("$THREAD_7_102041", NAME);
    let result = name_threads(&human, true).unwrap();
    assert!(result.source.contains(&format!("function {NAME}(")));
    assert!(!result
        .report
        .outcomes
        .iter()
        .any(|outcome| outcome.binding.name == NAME && outcome.applied));

    for collision in [
        base().replace(
            "function main(){",
            &format!("function main(){{let {NAME}=17;useValue({NAME});"),
        ),
        base().replace(
            "function main(){",
            &format!("function main(){{function {NAME}(){{}}useValue({NAME});"),
        ),
    ] {
        assert!(parses(&collision), "{collision}");
        assert!(!named(&collision), "{collision}");
    }
}

#[test]
fn edits_are_identifier_only_replacements() {
    // Replay against the integrated source so the assertion stays scoped to the
    // outer interceptor's identifier edit.
    let source = base().replace("$THREAD_4_36154", "instrumentedPropertySetter");
    let result = name_threads(&source, true).unwrap();
    let applied: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|outcome| outcome.applied && outcome.proposed_name.as_deref() == Some(NAME))
        .collect();
    assert_eq!(applied.len(), 1, "expected exactly one applied target");
    // Sort the edits explicitly by descending original offset before replay.
    // Never iterate an unsorted edit list in reverse: the harness does not
    // guarantee emission order, and a blind reverse can splice a later edit
    // before an earlier one and corrupt the offsets.
    let mut replay = source;
    let mut edits: Vec<_> = result.report.edits.iter().collect();
    edits.sort_by(|a, b| b.start.cmp(&a.start).then(b.end.cmp(&a.end)));
    for edit in edits {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
}

/// Inline lookup variants select the registry record and derived result into one or two
/// locals, or retain a cursor and helper cache. The descriptor walk can carry one or two
/// record locals.
#[test]
fn accepts_inline_lookup_variants() {
    // `WITNESS` is the bare interceptor; every variant is wrapped exactly once
    // below so each fixture stays a one-carrier program.
    let source = WITNESS;
    let full = inline_registry_two(&inline_descriptor(source));
    let single = inline_registry_one(&inline_descriptor(source));
    let truth = inline_registry_truth(&inline_descriptor(source));
    let cursor = inline_descriptor_cursor(source);
    let descriptor_only = inline_descriptor(source);
    for (label, body) in [
        ("target_two_local", full.clone()),
        ("single_local", single),
        ("truth_derived", truth),
        ("cursor_helper_cache", cursor),
        ("descriptor_inline_helper_cache", descriptor_only),
    ] {
        let source = fixture_named(&body, "$THREAD_7_102041");
        assert!(parses(&source), "{label} did not parse");
        assert!(named(&source), "{label} was not named");
        assert!(our_claim(&source), "{label} was not our claim");
    }

    // Opaque spelling: generated names, the owner formals and the record
    // locals carry no role evidence.
    let renamed = full
        .replace("$KEY_", "$HK")
        .replace("$THREAD_7_102041", "$THREAD_90_777_2")
        .replace("$THREAD_3_150630", "$THREAD_42_31337")
        .replace("$THREAD_4_36154", "$THREAD_43_31337")
        .replace("$BH", "target")
        .replace("$BI", "propertyName");
    let renamed_source = fixture_named(&renamed, "$THREAD_90_777_2");
    assert!(parses(&renamed_source), "renamed inline did not parse");
    assert!(named(&renamed_source), "renamed inline was not named");
    assert!(
        our_claim(&renamed_source),
        "renamed inline was not our claim"
    );

    // Node-shape: `while (true)` is the same unbounded walk as `for (;;)`.
    let while_body = full.replace("for (;;) {", "while (true) {");
    let while_source = fixture_named(&while_body, "$THREAD_7_102041");
    assert!(parses(&while_source), "while inline did not parse");
    assert!(named(&while_source), "while inline was not named");
    assert!(our_claim(&while_source), "while inline was not our claim");

    // Declaration grouping does not decide the proof.
    let grouped = full.replace(
        "let $X_CURSOR = $BH;let $KEY_2;",
        "let $X_CURSOR = $BH, $KEY_2;",
    );
    let grouped_source = fixture_named(&grouped, "$THREAD_7_102041");
    assert!(parses(&grouped_source), "grouped inline did not parse");
    assert!(named(&grouped_source), "grouped inline was not named");
    assert!(
        our_claim(&grouped_source),
        "grouped inline was not our claim"
    );
}

#[test]
fn inline_claims_only_the_outer_interceptor() {
    let source = fixture_named(&inline_full(), "$THREAD_7_102041");
    let result = name_threads(&source, true).unwrap();
    let claimed: Vec<_> = result
        .report
        .function_roles
        .iter()
        .filter(|role| role.evaluators.iter().any(|e| e == EVALUATOR))
        .collect();
    assert_eq!(claimed.len(), 1, "expected exactly the outer interceptor");
    assert_eq!(claimed[0].current_name.as_deref(), Some("$THREAD_7_102041"));
    for callback in ["$THREAD_3_150630", "$THREAD_4_36154"] {
        let role = result
            .report
            .function_roles
            .iter()
            .find(|role| role.current_name.as_deref() == Some(callback))
            .expect("the owned wrapper must exist");
        assert!(
            !role.evaluators.iter().any(|e| e == EVALUATOR),
            "{callback} belongs to the callback family"
        );
    }
}

#[test]
fn denies_inline_wrong_receiver_key_advance_and_disconnects() {
    let full = inline_full();
    let cases = [
        // The prototype advance leaves the bound receiver.
        full.replace(
            "Object.getPrototypeOf($X_CURSOR)",
            "Object.getPrototypeOf($BH)",
        ),
        // The descriptor read is taken from the wrong receiver.
        full.replace(
            "Object.getOwnPropertyDescriptor($X_CURSOR, $BI)",
            "Object.getOwnPropertyDescriptor($BH, $BI)",
        ),
        // The descriptor read uses the wrong key.
        full.replace(
            "Object.getOwnPropertyDescriptor($X_CURSOR, $BI)",
            "Object.getOwnPropertyDescriptor($X_CURSOR, $BH)",
        ),
        // The packed record points at the input instead of the containing owner.
        full.replace(
            "$KEY_2 = { containingObj: $X_CURSOR, desc: $X_DESC };",
            "$KEY_2 = { containingObj: $BH, desc: $X_DESC };",
        ),
        // The null guard is inverted.
        full.replace("if ($KEY_2 == null) {", "if ($KEY_2 != null) {"),
        // The null guard no longer stores a literal null.
        full.replace("$KEY_2 = null;break;", "$KEY_2 = void 0;break;"),
        // The registry bucket is keyed by the wrong property.
        full.replace(
            "if ($KEY_362[$BI] == null) {",
            "if ($KEY_362[$BH] == null) {",
        ),
        // The scan compares against the input instead of the containing owner.
        full.replace("if ($BJ !== $BM.object) {", "if ($BH !== $BM.object) {"),
        // The bounds check is disconnected from the scanned bucket.
        full.replace(
            "$X_INDEX >= $KEY_362[$BI].length",
            "$X_INDEX >= $KEY_14.length",
        ),
        // The registration tail uses a different registry than the scan.
        full.replace("let $BQ = $KEY_362[$BI];", "let $BQ = $KEY_999[$BI];"),
        // The selected element is the bucket, not one element of it.
        full.replace("$BM = $KEY_362[$BI][$X_INDEX];", "$BM = $KEY_362[$BI];"),
        // A method-interceptor hook key replaces an accessor hook key.
        full.replace("onBeforeGet: void 0", "onBeforeInvoke: void 0"),
        // The getter wrapper invokes the setter accessor.
        full.replace("$KEY_197($KEY_14, this)", "$KEY_197($KEY_0, this)"),
    ];
    for body in cases {
        denied(&fixture_named(&body, "$THREAD_7_102041"));
    }

    // The global `Object` is shadowed in the owning scope.
    denied(
        &fixture_named(&full, "$THREAD_7_102041").replace(
            "function main(){",
            "function main(){let Object={create:function(){return {};},hasOwnProperty:function(){return true;}};",
        ),
    );
}

const INLINE_DECOY: &str = r##"function main(){function $THREAD_9_1($A,$B){function $THREAD_9_2($C,$D){let $E=$C;let $F;for(;;){$F=void 0;if($E==null){$F=null;break;}let $G=Object.getOwnPropertyDescriptor($E,$D);if($G==null){$E=Object.getPrototypeOf($E);continue;}$F={containingObj:$E,desc:$G};break;}if($F==null){return null;}return $F;}return $THREAD_9_2;}return $THREAD_9_1;}main();"##;

#[test]
fn denies_inline_nested_and_hook_decoys() {
    // A nested inline prototype walk never donates a descriptor lookup to the
    // enclosing function that merely returns it.
    denied(INLINE_DECOY);

    // A full lookup whose accessor record is not the interceptor.
    let no_registration = inline_full().replace("$BJ.result = $BN;", "$BJ.result = $BL;");
    denied(&fixture_named(&no_registration, "$THREAD_7_102041"));
}

const SPLIT_NAME: &str = "installAccessorHookProtocol";

// A body-only harness with invented captures. It tests the local protocol,
// never claims source-backed external origins; actual saved-source receipts
// provide the independent end-to-end evidence.
fn split_fixture() -> String {
    let body = inline_full().replace(
        "if (!$BS || $BS.bypassResult == null) {",
        "let bypass = $BS; if (bypass) { bypass = $BS.bypassResult != null; } if (!bypass) {",
    );
    fixture_named(&body, "$THREAD_7_102041")
}

fn split_named(source: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|o| o.applied && o.proposed_name.as_deref() == Some(SPLIT_NAME))
}

#[test]
fn accepts_closed_split_guard_and_alpha_variant() {
    let source = split_fixture();
    assert!(split_named(&source));
    let alpha = source
        .replace("$KEY_", "$CAP_")
        .replace("$BH", "target")
        .replace("$BI", "key")
        .replace("let bypass", "let testResult")
        .replace("(bypass)", "(testResult)")
        .replace("(!bypass)", "(!testResult)")
        .replace("bypass =", "testResult =");
    assert!(split_named(&alpha));
    let result = name_threads(&source, true).unwrap();
    let applied: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|o| o.applied && o.proposed_name.as_deref() == Some(SPLIT_NAME))
        .collect();
    assert_eq!(applied.len(), 1);
    assert!(applied[0]
        .explanation
        .iter()
        .any(|e| e.contains("explicitly unproved")));
}

#[test]
fn refuses_split_guard_identity_effect_and_publication_negatives() {
    let source = split_fixture();
    for (old, new) in [
        ("let bypass = $BS;", "let bypass = $KEY_14;"),
        ("if (bypass) {", "if ($BS) {"),
        (
            "bypass = $BS.bypassResult != null;",
            "bypass = $BS.bypassResult == null;",
        ),
        ("if (!bypass) {", "if (bypass) {"),
        (
            "bypass = $BS.bypassResult != null;",
            "let bypass = $BS.bypassResult != null;",
        ),
        ("let bypass = $BS;", "let bypass = $BS; extraEffect();"),
        (
            "let bypass = $BS;",
            "let bypass = lateEvent; let lateEvent = $BS;",
        ),
        ("$KEY_197($KEY_14, this)", "$KEY_197($KEY_14, $BH)"),
        ("$KEY_4.notify($BV)", "$KEY_1.notify($BV)"),
        ("$BN.onBeforeGet = $KEY_1;", "$BN.onBeforeGet = $KEY_4;"),
        ("if ($KEY_5 === false || $BK)", "if (!$KEY_5 || $BK)"),
        (
            "$KEY_2.get = $THREAD_3_150630;",
            "$KEY_2.set = $THREAD_3_150630;",
        ),
        (
            "Object.defineProperty($BJ, $BI, $KEY_2)",
            "Object.defineProperty($BH, $BI, $KEY_2)",
        ),
        (
            "Object.getPrototypeOf($X_CURSOR)",
            "Object.getPrototypeOf($BH)",
        ),
        ("if ($BJ !== $BM.object)", "if ($BH !== $BM.object)"),
        ("let $BQ = $KEY_362[$BI];", "let $BQ = $KEY_999[$BI];"),
        ("$BJ.result = $BN;", "$BJ.result = $BL;"),
        ("$BJ.original = $BL;", "$BJ.original = $BN;"),
        (
            "$KEY_2.get.prototype = void 0;",
            "$KEY_2.set.prototype = void 0;",
        ),
        ("typeof $KEY_14 ===", "typeof $KEY_14 !=="),
        (
            "$KEY_49.toString.call($KEY_14)",
            "$KEY_999.toString.call($KEY_14)",
        ),
        ("$KEY_93.push($BO)", "$KEY_252.push($BO)"),
        ("function main(){", "function main(){let Object={};"),
    ] {
        assert!(source.contains(old), "invalid mutation: {old}");
        assert!(
            !split_named(&source.replace(old, new)),
            "accepted mutation: {old} => {new}"
        );
    }
}

// A branch-shaped rewrite evaluates the own-property test once, reads param
// only on success, and uses the original setter argument on both fallbacks.
#[test]
fn accepts_branch_param_rewrite_and_alpha() {
    let body = WITNESS.replace(
        "else {\n\t\t\t\t\t\t\t\t\t\t$BT = $KEY_197(Object.hasOwnProperty, $BT, \"param\") ? $BT.param : $BR;\n\t\t\t\t\t\t\t\t\t}",
        "else if ($KEY_197(Object.hasOwnProperty, $BT, \"param\")) { $BT = $BT.param; } else { $BT = $BR; }",
    );
    assert_ne!(body, WITNESS);
    assert!(named(&fixture(&body)));
    assert!(named(&fixture(
        &body
            .replace("$KEY_", "$CAP_")
            .replace("$BT", "event")
            .replace("$BR", "argument")
    )));
    for (old, new) in [
        ("else { $BT = $BR; }", "else { $BT = sideEffect(); }"),
        ("{ $BT = $BT.param; }", "{ $BT = $BR; }"),
        ("else { $BT = $BR; }", "else { $BT = $BT.param; }"),
        (
            "Object.hasOwnProperty, $BT, \"param\"",
            "Object.hasOwnProperty, $BR, \"param\"",
        ),
        ("{ $BT = $BT.param; }", "{ sideEffect(); $BT = $BT.param; }"),
    ] {
        let variant = body.replace(old, new);
        assert_ne!(variant, body, "missing mutation {old}");
        denied(&fixture(&variant));
    }
}

const CAPTURED_DESCRIPTOR: &str = "function lookup(object, key) { let cursor = object; while (cursor != null) { let desc = Object.getOwnPropertyDescriptor(cursor, key); if (desc != null) { cursor = {containingObj: cursor, desc: desc}; return cursor; } else {cursor = Object.getPrototypeOf(cursor); continue;} } return null; } let $KEY_100 = lookup;";
const CAPTURED_CACHE: &str = "function cached(object, key) { if ($KEY_362[key] == null) {return null;} let index = 0; while (index < $KEY_362[key].length) {if (object === $KEY_362[key][index].object) {return $KEY_362[key][index];} else {++index;continue;}}return null;} let $KEY_315 = cached;";

fn captured_fixture(descriptor: bool) -> String {
    let mut body = inline_full()
        .replace(INLINE_REGISTRY_TWO, "let $BM = $KEY_315($BJ, $BI);")
        .replace("$X_CACHED", "$BM");
    if descriptor {
        body = body.replace(INLINE_LOOKUP, "let $KEY_2 = $KEY_100($BH, $BI);");
    }
    body = body.replace(
        "if (!$BS || $BS.bypassResult == null) {",
        "let bypass = $BS; if (bypass) { bypass = $BS.bypassResult != null; } if (!bypass) {",
    );
    // Include the early-call negatives: they also make the interceptor itself
    // reachable. Carrier padding is solely for deterministic fixture selection.
    let padding = (0..32)
        .map(|i| format!("fixture{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let carrier = format!("function main(){{ let {padding};");
    fixture(&format!(
        "{} {CAPTURED_CACHE} {body}",
        if descriptor { CAPTURED_DESCRIPTOR } else { "" }
    ))
    .replace("function main(){", &carrier)
}

#[test]
fn accepts_resolved_accessor_lookup_captures() {
    for descriptor in [true, false] {
        let source = captured_fixture(descriptor);
        assert!(split_named(&source), "descriptor={descriptor}");
        assert!(split_named(
            &source
                .replace("lookup", "walk")
                .replace("cached", "search")
                .replace("$KEY_", "$CAP_")
        ));
        let assigned = source.replace("let $KEY_315 = cached;", "let $KEY_315; $KEY_315 = cached;");
        assert!(split_named(&assigned));
    }
}

#[test]
fn refuses_accessor_capture_origin_and_cache_decoys() {
    let source = captured_fixture(true);
    for (old, new) in [
        (
            "let $KEY_315 = cached;",
            "let $KEY_315 = cached; $KEY_315 = other;",
        ),
        ("let $KEY_315 = cached;", "let $KEY_315 = other;"),
        (
            "function main(){",
            "function main(){ $THREAD_7_102041({}, 'x');",
        ),
        (
            "function main(){",
            "function main(){ early(); function early(){ return $THREAD_7_102041; }",
        ),
        (
            "let $KEY_315 = cached;",
            "let $KEY_315 = late; let late = cached;",
        ),
        (
            "let $KEY_100 = lookup;",
            "let $KEY_100 = lookup; lookup = other;",
        ),
        ("while (cursor != null)", "while (object != null)"),
        (
            "Object.getPrototypeOf(cursor)",
            "Object.getPrototypeOf(object)",
        ),
        (
            "getOwnPropertyDescriptor(cursor, key)",
            "getOwnPropertyDescriptor(object, key)",
        ),
        (
            "if (object === $KEY_362[key][index].object)",
            "if (key === $KEY_362[key][index].object)",
        ),
        (
            "return $KEY_362[key][index];",
            "return $KEY_999[key][index];",
        ),
        (
            "let $KEY_315 = cached;",
            "let $KEY_315; if (condition) { $KEY_315 = cached; }",
        ),
        (
            "return null;} let $KEY_315",
            "return sideEffect();} let $KEY_315",
        ),
        (
            "let $KEY_100 = lookup;",
            "let $KEY_100; if (condition) { $KEY_100 = lookup; }",
        ),
    ] {
        let variant = source.replace(old, new);
        assert_ne!(variant, source, "missing mutation {old}");
        assert!(!split_named(&variant), "accepted {old} -> {new}");
    }
}

const INLINE_REGISTRY_LOOP_TRUTH: &str = "let $X_FOUND, $X_RECORD; if ($KEY_362[$BI] == null) { $X_FOUND = false; $X_RECORD = null; } else { let $X_INDEX = 0; for (;;) { if ($X_INDEX >= $KEY_362[$BI].length) { $X_FOUND = false; $X_RECORD = null; break; } $X_RECORD = $KEY_362[$BI][$X_INDEX]; if ($BJ !== $X_RECORD.object) { ++$X_INDEX; continue; } $X_FOUND = $X_RECORD != null; break; } }";

fn loop_truth_fixture() -> String {
    fixture(
        &inline_descriptor(WITNESS)
            .replace("if ($BM != null) {", "if ($X_FOUND) {")
            .replace("$BM.result", "$X_RECORD.result")
            .replace("$BM.original", "$X_RECORD.original")
            .replace("let $BM = $KEY_315($BJ, $BI);", INLINE_REGISTRY_LOOP_TRUTH),
    )
}

#[test]
fn accepts_cache_truth_derived_inside_scan_and_alpha() {
    let source = loop_truth_fixture();
    assert!(named(&source));
    assert!(named(
        &source
            .replace("$KEY_", "$CAP_")
            .replace("$X_FOUND", "found")
            .replace("$X_RECORD", "entry")
    ));
    assert!(named(&source.replace(
        "$X_FOUND = false; $X_RECORD = null;",
        "$X_RECORD = null; $X_FOUND = false;"
    )));
}

#[test]
fn refuses_cache_truth_exit_and_record_disconnects() {
    let source = loop_truth_fixture();
    for (old, new) in [
        (
            "$X_FOUND = false; $X_RECORD = null;",
            "$X_FOUND = true; $X_RECORD = null;",
        ),
        ("$X_FOUND = false; $X_RECORD = null;", "$X_RECORD = null;"),
        (
            "$X_FOUND = $X_RECORD != null;",
            "$X_FOUND = $X_RECORD == null;",
        ),
        ("$X_FOUND = $X_RECORD != null;", "$X_FOUND = $BJ != null;"),
        ("$X_FOUND = $X_RECORD != null;", "$X_FOUND = effect();"),
        (
            "$X_FOUND = $X_RECORD != null;",
            "$X_RECORD = $X_RECORD != null;",
        ),
        (
            "$X_RECORD = $KEY_362[$BI][$X_INDEX];",
            "$X_RECORD = $KEY_999[$BI][$X_INDEX];",
        ),
        ("if ($X_FOUND)", "if (!$X_FOUND)"),
        ("++$X_INDEX; continue;", "++$X_INDEX; break;"),
        (
            "$X_FOUND = $X_RECORD != null; break;",
            "$X_FOUND = $X_RECORD != null; extra(); break;",
        ),
    ] {
        let variant = source.replace(old, new);
        assert_ne!(variant, source, "missing mutation {old}");
        denied(&variant);
    }
}

#[test]
fn refuses_descriptor_packet_exposure_outside_local_frontier() {
    let source =
        fixture(&inline_full()).replace("function main(){", "function main(){let foreign;");
    for replacement in [
        "$KEY_2 = { containingObj: $X_CURSOR, desc: $X_DESC }; foreign = $KEY_2;",
        "foreign = { containingObj: $X_CURSOR, desc: $X_DESC }; $KEY_2 = foreign;",
    ] {
        let variant = source.replace(
            "$KEY_2 = { containingObj: $X_CURSOR, desc: $X_DESC };",
            replacement,
        );
        assert_ne!(variant, source);
        denied(&variant);
    }
}
