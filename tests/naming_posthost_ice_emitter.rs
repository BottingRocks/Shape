//! Focused, portable coverage for WebRTC-candidate seeded signal-frame emission.
//!
//! Positives cover forward counters, reversed ICE fields, and descending counters. Carriers
//! retain captured observers, including formal-element initialization, and independently
//! scoped reseed, writer, and pack callables. The guarded body completes on that same
//! observer outside the null guard.
//!
//! Negatives change nonce, header, reseed, writer, field flow, observer, shadowing, or
//! nesting. Unrelated array-length, audio, and canvas forms prevent broad fallback.
//!
//! The checked renamer and duplicate-role allocator own concrete spelling. Positives prove
//! candidate and evaluator identity, plus exact transformed source when applied. A later
//! application-policy refusal must retain the proposal identity and typed refusal.
use shape::naming::{name_threads, ThreadName};

const EVALUATOR: &str = "thread_posthost_ice_emitter";
const NAME: &str = "emitSeededHashEncoding";

const FORWARD: &str = r#"function $THREAD_8_24420($BV) {
								if ($BV != null) {
									let $BW = Math.random() * 4294967296;
									let $BX = [($BW | 0) >>> 24, ($BW | 0) >> 16 & 255, ($BW | 0) >> 8 & 255, ($BW | 0) & 255];
									let $KEY_0 = $KEY_355(($BW | 0) ^ 3339881226);
									let $BY = $KEY_205($KEY_0);
									let i = 0;
									$KEY_0 = $KEY_173($KEY_0, $BV.length, $BX, 3);
									for (; i < $BV.length; i++) {
										$KEY_0 = $BY($BV[i].ip, $BX, $KEY_0);
										$KEY_0 = $BY($BV[i].candidate, $BX, $KEY_0);
									}
									$KEY_4.next([[], $BX]);
								}
								$KEY_4.complete();
							}"#;

const FORWARD_REVERSED: &str = r#"function $THREAD_7_34667($CE) {
											if ($CE != null) {
												let $CF = Math.random() * 4294967296;
												let $CG = [($CF | 0) >>> 24, ($CF | 0) >> 16 & 255, ($CF | 0) >> 8 & 255, ($CF | 0) & 255];
												let $KEY_7 = $KEY_73(($CF | 0) ^ 3505639934);
												let $CH = $KEY_362($KEY_7);
												let i = 0;
												$KEY_7 = $KEY_304($KEY_7, $CE.length, $CG, 3);
												for (; i < $CE.length; i++) {
													$KEY_7 = $CH($CE[i].candidate, $CG, $KEY_7);
													$KEY_7 = $CH($CE[i].ip, $CG, $KEY_7);
												}
												$KEY_4.next([[], $CG]);
											}
											$KEY_4.complete();
										}"#;

const REVERSE: &str = r#"function $THREAD_6_119024($CL) {
								if ($CL != null) {
									let $KEY_4 = Math.random() * 4294967296 | 0;
									let $CM = [$KEY_4 >>> 24, $KEY_4 >> 16 & 255, $KEY_4 >> 8 & 255, $KEY_4 & 255];
									$KEY_4 = $KEY_43($KEY_4 ^ 2750064904);
									let $CN = $KEY_433($KEY_4);
									let $CO = $CL.length;
									$KEY_4 = $KEY_275($KEY_4, $CO, $CM, 3);
									for (;;) {
										--$CO;
										if (!($CO >= 0)) break;
										$KEY_4 = $CN($CL[$CO].ip, $CM, $KEY_4);
										$KEY_4 = $CN($CL[$CO].candidate, $CM, $KEY_4);
										$CO = Number($CO);
									}
									$KEY_3.next([[], $CM]);
								}
								$KEY_3.complete();
							}"#;

const REVERSE_ALIASED_OBSERVER: &str = r#"function $THREAD_6_111611($CW) {
								if ($CW != null) {
									let $KEY_3 = Math.random() * 4294967296 | 0;
									let $CX = [$KEY_3 >>> 24, $KEY_3 >> 16 & 255, $KEY_3 >> 8 & 255, $KEY_3 & 255];
									$KEY_3 = $KEY_140($KEY_3 ^ 3370210187);
									let $CY = $KEY_144($KEY_3);
									let $CZ = $CW.length;
									$KEY_3 = $KEY_111($KEY_3, $CZ, $CX, 3);
									for (;;) {
										--$CZ;
										if (!($CZ >= 0)) break;
										$KEY_3 = $CY($CW[$CZ].ip, $CX, $KEY_3);
										$KEY_3 = $CY($CW[$CZ].candidate, $CX, $KEY_3);
										$CZ = Number($CZ);
									}
									$KEY_32.next([[], $CX]);
								}
								$KEY_32.complete();
							}"#;

const ARRAY_LENGTH: &str = r#"function $THREAD_8_82281($DR) {
							let $KEY_5 = Math.random() * 4294967296 | 0;
							let $DS = [$KEY_5 >>> 24, $KEY_5 >> 16 & 255, $KEY_5 >> 8 & 255, $KEY_5 & 255];
							let $DT = $DR.length;
							let $KEY_6 = $KEY_388($KEY_5 ^ 448289090);
							let i = 0;
							let $DU = $KEY_249($KEY_6, $DT, $DS, 3);
							for (; i < $DT; i++) {
								$DU = $KEY_249($KEY_6, $DR[i], $DS, $DU);
							}
							$KEY_14.next([[], $DS]);
							$KEY_14.complete();
						}"#;

fn wrap(body: &str, helper: &str, observer: &str, thread: &str) -> String {
    let mut source = String::from("function main(){");
    source.push_str(helper);
    source.push_str("let ");
    source.push_str(observer);
    source.push_str("={next:function(){},complete:function(){}};");
    source.push_str("let $KEY_9={next:function(){},complete:function(){}};");
    source.push_str(body);
    source.push_str("return ");
    source.push_str(thread);
    source.push_str(";}main();");
    source
}

fn forward() -> String {
    wrap(
        FORWARD,
        "function $KEY_355(v){return v;}function $KEY_205(v){return function(a,b,c){return c;};}function $KEY_173(a,b,c,d){return d;}",
        "$KEY_4",
        "$THREAD_8_24420",
    )
}

fn forward_reversed() -> String {
    wrap(
        FORWARD_REVERSED,
        "function $KEY_73(v){return v;}function $KEY_362(v){return function(a,b,c){return c;};}function $KEY_304(a,b,c,d){return d;}",
        "$KEY_4",
        "$THREAD_7_34667",
    )
}

fn reverse() -> String {
    wrap(
        REVERSE,
        "function $KEY_43(v){return v;}function $KEY_433(v){return function(a,b,c){return c;};}function $KEY_275(a,b,c,d){return d;}",
        "$KEY_3",
        "$THREAD_6_119024",
    )
}

fn reverse_aliased_observer() -> String {
    wrap(
        REVERSE_ALIASED_OBSERVER,
        "function $KEY_140(v){return v;}function $KEY_144(v){return function(a,b,c){return c;};}function $KEY_111(a,b,c,d){return d;}",
        "$KEY_32",
        "$THREAD_6_111611",
    )
}

fn array_length() -> String {
    wrap(
        ARRAY_LENGTH,
        "function $KEY_388(v){return v;}function $KEY_249(a,b,c,d){return d;}",
        "$KEY_14",
        "$THREAD_8_82281",
    )
}

/// The observer is a plain captured binding initialized from a formal element, preserving
/// the witness carrier dataflow.
fn forward_with_formal_element_observer() -> String {
    format!(
        "function main($AO){{let $KEY_4=$AO[0];function $KEY_355(v){{return v;}}function $KEY_205(v){{return function(a,b,c){{return c;}};}}function $KEY_173(a,b,c,d){{return d;}}{FORWARD}return $THREAD_8_24420;}}main([{{next:function(){{}},complete:function(){{}}}}]);"
    )
}

fn claims(source: &str) -> usize {
    name_threads(source, true)
        .unwrap()
        .report
        .function_roles
        .iter()
        .filter(|role| {
            role.candidates.iter().any(|candidate| candidate == NAME)
                && role
                    .evaluators
                    .iter()
                    .any(|evaluator| evaluator == EVALUATOR)
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

fn evaluator_claimed(source: &str) -> bool {
    name_threads(source, true)
        .unwrap()
        .report
        .function_roles
        .iter()
        .any(|role| {
            role.evaluators
                .iter()
                .any(|evaluator| evaluator == EVALUATOR)
        })
}

/// The canonical role identity, independent of the concrete name allocator:
/// exactly one candidate (the governed role) plus this evaluator, on one binding.
fn canonical_roles(source: &str) -> usize {
    name_threads(source, true)
        .unwrap()
        .report
        .function_roles
        .iter()
        .filter(|role| {
            role.candidates.len() == 1
                && role.candidates[0].as_str() == NAME
                && role
                    .evaluators
                    .iter()
                    .any(|evaluator| evaluator == EVALUATOR)
        })
        .count()
}

fn outcome(source: &str, thread: &str) -> ThreadName {
    name_threads(source, true)
        .unwrap()
        .report
        .outcomes
        .into_iter()
        .find(|outcome| outcome.binding.name == thread)
        .expect("the target binding is reported")
}

fn assert_canonical_proof(source: &str, thread: &str) {
    assert_eq!(claims(source), 1, "{source}");
    assert_eq!(canonical_roles(source), 1, "{source}");
    let outcome = outcome(source, thread);
    assert_eq!(outcome.evaluator.as_deref(), Some(EVALUATOR), "{source}");
    assert_eq!(outcome.proposed_name.as_deref(), Some(NAME), "{source}");
}

#[test]
fn actual_ice_candidate_shapes_are_claimed_and_renamed() {
    for (source, thread) in [
        (forward(), "$THREAD_8_24420"),
        (forward_reversed(), "$THREAD_7_34667"),
        (reverse(), "$THREAD_6_119024"),
        (reverse_aliased_observer(), "$THREAD_6_111611"),
    ] {
        // The proof is over the exact verified body: the ICE evaluator proposes
        // the one canonical role `emitSeededHashEncoding` on the exact binding.
        assert_canonical_proof(&source, thread);
        let renamed = name_threads(&source, true).unwrap();
        // Concrete spelling is owned by the checked renamer. When it applies,
        // the transformed source must be the exact renamed witness with replay
        // in descending byte order. When shared policy refuses, the refusal is
        // typed and the proposal identity above already proved the role.
        if applied(&source) == 1 {
            assert!(
                renamed.source.contains("function emitSeededHashEncoding("),
                "{source}"
            );
            assert!(
                !renamed.source.contains(&format!("function {thread}(")),
                "{source}"
            );
            let mut replay = source.clone();
            let mut edits = renamed.report.edits.clone();
            edits.sort_by(|left, right| right.start.cmp(&left.start));
            for edit in edits {
                replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
            }
            assert_eq!(replay, renamed.source);
        }
    }
}

#[test]
fn actual_unbraced_descending_break_is_claimed_and_labeled_break_is_refused() {
    // Descending scans can end in an unbraced or braced single-statement break. Both must
    // claim. Labeled scans, labeled breaks, and other exits fall outside the verified form.
    let unbraced = reverse();
    assert!(unbraced.contains("if (!($CO >= 0)) break;"), "{unbraced}");
    assert_canonical_proof(&unbraced, "$THREAD_6_119024");
    let braced = unbraced.replace(
        "if (!($CO >= 0)) break;",
        "if (!($CO >= 0)) { break; }",
    );
    assert_canonical_proof(&braced, "$THREAD_6_119024");
    let reverse_observer = reverse_aliased_observer();
    assert!(reverse_observer.contains("if (!($CZ >= 0)) break;"), "{reverse_observer}");
    assert_canonical_proof(&reverse_observer, "$THREAD_6_111611");
    let labeled = unbraced
        .replace("for (;;) {", "scan: for (;;) {")
        .replace("if (!($CO >= 0)) break;", "if (!($CO >= 0)) break scan;");
    assert_eq!(claims(&labeled), 0, "{labeled}");
    let continued = unbraced.replace(
        "if (!($CO >= 0)) break;",
        "if (!($CO >= 0)) continue;",
    );
    assert_eq!(claims(&continued), 0, "{continued}");
}

#[test]
fn witness_capture_shape_from_a_formal_element_is_proven() {
    let source = forward_with_formal_element_observer();
    assert_canonical_proof(&source, "$THREAD_8_24420");
    assert_eq!(applied(&source), 1, "{source}");
    assert!(name_threads(&source, true)
        .unwrap()
        .source
        .contains("function emitSeededHashEncoding("));
}

#[test]
fn renamed_spelling_variant_keeps_the_proof() {
    let source = forward()
        .replace("$THREAD_8_24420", "$THREAD_71_887766_2")
        .replace("$BV", "$CANDIDATES")
        .replace("$BW", "$SEED")
        .replace("$BX", "$FRAME")
        .replace("$KEY_0", "$STREAM")
        .replace("$BY", "$WRITER")
        .replace("let i = 0", "let cursor = 0")
        .replace("$CANDIDATES[i]", "$CANDIDATES[cursor]")
        .replace("for (; i <", "for (; cursor <")
        .replace("i++)", "cursor++)");
    assert_canonical_proof(&source, "$THREAD_71_887766_2");
    assert_eq!(applied(&source), 1, "{source}");
    assert!(name_threads(&source, true)
        .unwrap()
        .source
        .contains("function emitSeededHashEncoding("));
}

#[test]
fn wrong_nonce_header_and_reseed_are_refused() {
    let base = forward();
    for source in [
        // nonce is not Math.random() * 2^32
        base.replace("Math.random() * 4294967296", "Math.random() * 4294967295"),
        base.replace("Math.random()", "custom.random()"),
        // header byte shifts and masks are wrong
        base.replace(">>> 24", ">>> 16"),
        base.replace(">> 8 & 255", ">> 7 & 255"),
        base.replace("& 255];", "& 256];"),
        // the stream is not reseeded through seed XOR a numeric literal
        base.replace("^ 3339881226", "+ 3339881226"),
        base.replace("$KEY_355(($BW | 0) ^ 3339881226)", "$KEY_355(($BW | 0))"),
    ] {
        assert_eq!(claims(&source), 0, "accepted negative:\n{source}");
    }
}

#[test]
fn wrong_writer_field_flow_and_observer_are_refused() {
    let base = forward();
    for source in [
        // writer factory does not derive from the established stream
        base.replace("let $BY = $KEY_205($KEY_0);", "let $BY = $KEY_205(0);"),
        // writer is called with a different header
        base.replace(
            "$BY($BV[i].ip, $BX, $KEY_0)",
            "$BY($BV[i].ip, $KEY_0, $KEY_0)",
        ),
        // the running cursor is not forwarded
        base.replace(
            "$BY($BV[i].candidate, $BX, $KEY_0)",
            "$BY($BV[i].candidate, $BX, $BW)",
        ),
        // unrelated media fields stand in for the ICE pair
        base.replace(".ip", ".frequency")
            .replace(".candidate", ".sample"),
        // the length mix uses a different tag byte
        base.replace("$BV.length, $BX, 3", "$BV.length, $BX, 4"),
        // next forwards a different frame
        base.replace("$KEY_4.next([[], $BX]);", "$KEY_4.next([[], $BW]);"),
        // the empty payload sentinel is missing
        base.replace("$KEY_4.next([[], $BX]);", "$KEY_4.next([$BX]);"),
        // complete uses a different captured observer
        base.replace("$KEY_4.complete();", "$KEY_9.complete();"),
    ] {
        assert_eq!(claims(&source), 0, "accepted negative:\n{source}");
    }
}

#[test]
fn nested_arrow_shadowed_math_and_unrelated_shapes_are_refused() {
    // The owned seeded emitter moved inside an arrow: the arrow is not an owned
    // function site and the outer factory body is not the emitter.
    let inner = FORWARD.trim_end().strip_suffix('}').unwrap();
    let mut nested = String::from("function main(){");
    nested.push_str(
        "function $KEY_355(v){return v;}function $KEY_205(v){return function(a,b,c){return c;};}function $KEY_173(a,b,c,d){return d;}",
    );
    nested.push_str("let $KEY_4={next:function(){},complete:function(){}};");
    nested.push_str(&inner.replace(
        "function $THREAD_8_24420($BV) {",
        "function $THREAD_8_24420($BV){let $inner=()=>{",
    ));
    nested.push_str("};return $inner;}return $THREAD_8_24420;}main();");
    assert!(!evaluator_claimed(&nested), "{nested}");

    // A shadowing Math binding is not the host Math namespace.
    let shadowed = forward().replace(
        "function main(){",
        "function main(){let Math={random:function(){return 0;}};",
    );
    assert_eq!(claims(&shadowed), 0, "{shadowed}");

    // The length-loop emitter is a different discovered subvariant, not ICE.
    assert!(!evaluator_claimed(&array_length()));

    // An audio-style field flow with the same nonce/header/observer skeleton is
    // not the ICE candidate emitter.
    let audio = forward()
        .replace("$BV[i].ip", "$BV[i].sampleRate")
        .replace("$BV[i].candidate", "$BV[i].channelCount");
    assert_eq!(claims(&audio), 0, "{audio}");
}

#[test]
fn two_instances_are_both_claimed() {
    // Verbatim duplicate copies of the same verified body.
    let second = FORWARD.replace("$THREAD_8_24420", "$THREAD_9_424242");
    let mut source = String::from("function main(){");
    source.push_str(
        "function $KEY_355(v){return v;}function $KEY_205(v){return function(a,b,c){return c;};}function $KEY_173(a,b,c,d){return d;}",
    );
    source.push_str("let $KEY_4={next:function(){},complete:function(){}};");
    source.push_str(FORWARD);
    source.push_str(&second);
    source.push_str("return [$THREAD_8_24420,$THREAD_9_424242];}main();");
    // Both copies carry the same canonical role proof. Concrete spelling is
    // owned by the separately reviewed duplicate-role allocator: the base
    // checked renamer refuses two simultaneous identical plans, while that
    // allocator may apply `role` and `role2`. Either way the proposal identity
    // must be exactly the proven role on both bindings, never a different name.
    assert_eq!(claims(&source), 2, "{source}");
    assert_eq!(canonical_roles(&source), 2, "{source}");
    let report = name_threads(&source, true).unwrap();
    let applied_outcomes: Vec<_> = report
        .report
        .outcomes
        .iter()
        .filter(|outcome| outcome.applied)
        .collect();
    for outcome in &applied_outcomes {
        let proposed = outcome.proposed_name.as_deref().unwrap_or_default();
        assert!(
            proposed == NAME
                || proposed.strip_prefix(NAME).is_some_and(|suffix| {
                    !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit())
                }),
            "allocated name must stay the canonical role stem: {proposed}"
        );
    }
    assert!(
        applied_outcomes.is_empty() || applied_outcomes.len() == 2,
        "duplicate spelling policy must apply both copies or refuse the batch"
    );
}

#[test]
fn prior_readable_name_is_retained() {
    let prior = forward().replace("$THREAD_8_24420", "emitSeededHashEncoding");
    let report = name_threads(&prior, true).unwrap();
    assert_eq!(report.source, prior);
    assert!(report.report.function_roles.iter().any(|role| {
        role.current_name.as_deref() == Some("emitSeededHashEncoding")
            && role
                .evaluators
                .iter()
                .any(|evaluator| evaluator == EVALUATOR)
    }));
}
