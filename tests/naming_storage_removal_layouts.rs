//! Exact assigned-input bodies with their own resolved seed/writer factories.
//! Mutations are checked for presence and must analyze before refusal is tested.
use shape::naming::name_threads;

const FIXED: &str = include_str!("fixtures/program/storage-removal-layouts/fixed-result-callback.js");
const MIXED: &str = include_str!("fixtures/program/storage-removal-layouts/mixed-carrier-callback.js");
const ALIASED: &str = include_str!("fixtures/program/storage-removal-layouts/aliased-factory-callback.js");
const ROLE: &str = "emitStorageRemovalSignal";
// Complete current UTF-8 input keeps real bootstrap, factory aliases,
// constructors and registration/consumers. Parsed and analyzed, never executed.
const HIGH_REMOVAL_LANE_CURRENT: &str =
    include_str!("fixtures/program/storage-removal-layouts/current-storage-callback-alternate.js");
const LOW_REMOVAL_LANE_CURRENT: &str =
    include_str!("fixtures/program/storage-removal-layouts/current-storage-callback.js");

fn check(source: &str, target: &str, accepted: bool) {
    let result = name_threads(source, true).expect("fixture must parse and analyze");
    let named = result.report.outcomes.iter().any(|outcome| {
        outcome.applied
            && outcome.binding.name == target
            && outcome.proposed_name.as_deref() == Some(ROLE)
    });
    assert_eq!(named, accepted, "{target}: expected accepted={accepted}");
}

fn changed(source: &str, from: &str, to: &str) -> String {
    assert!(source.contains(from), "missing mutation anchor: {from}");
    source.replacen(from, to, 1)
}

// Current maps use original UTF-8 byte spans. Restrict mutations to that
// callback: other real consumers can legitimately contain the same spelling.
fn changed_in_span(source: &str, span: std::ops::Range<usize>, from: &str, to: &str) -> String {
    let callback = source.get(span.clone()).expect("current UTF-8 callback span");
    assert_eq!(
        callback.matches(from).count(),
        1,
        "ambiguous mutation: {from}"
    );
    format!(
        "{}{}{}",
        &source[..span.start],
        changed(callback, from, to),
        &source[span.end..]
    )
}

#[test]
fn current_high_removal_lane_high_lane_requires_independent_wire_order_evidence() {
    // Equal success/catch frames do not prove that this observer expects the
    // reversed removal/error order. Keep it unknown until that origin is proved.
    check(HIGH_REMOVAL_LANE_CURRENT, "$THREAD_0_127439", false);
}

#[test]
fn current_high_removal_lane_direct_frames_require_exact_origins_and_order() {
    for (from, to) in [
        ("$CC.newValue === null", "$CC.oldValue === null"),
        ("$CF = $CF($CG, $CE, 3)", "$CF = $CF($CG, $CG, 3)"),
        ("$CF = $CF($CG, $CE, 3)", "$CF = $CF($CG, $CE, 4)"),
        ("$CI[$CJ + 1]", "$CI[$CJ + 2]"),
        (
            "$CE[$CF + 1] = ($KEY_17 | 0 | $KEY_27 << 1 | $KEY_9 << 2)",
            "$CE[$CF + 1] = ($KEY_17 | 0 | $KEY_9 << 1 | $KEY_27 << 2)",
        ),
        ("$KEY_10.next([[], $CI])", "$KEY_17.next([[], $CI])"),
        ("let $CK = $KEY_20;", "let $CK = $KEY_20; $CK = null;"),
        ("$CJ = $CJ($CK, $CI, 3)", "$CJ = $CJ($CK, $CK, 3)"),
        (
            "$CH = $KEY_450(($CH | 0) ^ 227760424)",
            "$CH = $KEY_450(($CH | 0) ^ 227760425)",
        ),
    ] {
        check(
            &changed_in_span(HIGH_REMOVAL_LANE_CURRENT, 346437..347601, from, to),
            "$THREAD_0_127439",
            false,
        );
    }
}

#[test]
fn current_low_removal_lane_aliased_direct_removal_low_lane_is_named() {
    check(LOW_REMOVAL_LANE_CURRENT, "$THREAD_2_96244", true);
}

#[test]
fn current_low_removal_lane_direct_frame_metadata_describes_its_own_lane_order() {
    let result = name_threads(LOW_REMOVAL_LANE_CURRENT, true).expect("fixture must parse and build Model");
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == "$THREAD_2_96244")
        .expect("current fixed-slot binding");
    assert!(outcome.applied);
    assert_eq!(outcome.proposed_name.as_deref(), Some(ROLE));
    assert_eq!(outcome.explanation.len(), 1);
    assert!(outcome.explanation[0].contains("removal at bit 0, error at bit 1"));
    assert!(outcome.explanation[0].contains("before flags"));
}

#[test]
fn current_low_removal_lane_alias_frames_require_exact_origins_and_order() {
    for (from, to) in [
        ("$BQ.newValue === null", "$BQ.oldValue === null"),
        ("let $BU = $KEY_25;", "let $BU = $KEY_19;"),
        ("let $BZ = $KEY_25;", "let $BZ = $KEY_25; $BZ = false;"),
        ("let $CA = $KEY_35;", "let $CA = $KEY_19;"),
        ("$BT = $BT($BV, $BS, 3)", "$BT = $BT($BV, $BV, 3)"),
        ("$BT = $BT($BV, $BS, 3)", "$BT = $BT($BV, $BS, 4)"),
        ("$BY = $BY($CA, $BX, 3)", "$BY = $BY($BZ, $BX, 3)"),
        ("$BS[$BT + 1]", "$BS[$BT + 2]"),
        ("$BX[4] = $BW() ^ 130", "$BX[4] = $BW() ^ 131"),
        (
            "$BU | 0 | $KEY_33 << 1 | $KEY_19 << 2",
            "$BU | 0 | $KEY_19 << 1 | $KEY_33 << 2",
        ),
        ("$KEY_36.next([[], $BX])", "$KEY_19.next([[], $BX])"),
        ("$KEY_36.next([[], $BX])", "$KEY_36.next([$BX, []])"),
        ("let $BY = $KEY_152($BW)", "let $BY = $KEY_382($BW)"),
        (
            "let $KEY_152 = $THREAD_3_12;",
            "let $KEY_152 = $THREAD_4_47142;",
        ),
        (
            "function $THREAD_3_12($BK) {\n\t\t\t\t\tlet $KEY_0 = $BK;",
            "function $THREAD_3_12($BK) {\n\t\t\t\t\tlet $KEY_0 = () => 0;",
        ),
    ] {
        // Factory mutations sit outside the callback and carry unique anchors.
        let source = if from.starts_with("function ") || from.starts_with("let $KEY_152 =") {
            assert_eq!(LOW_REMOVAL_LANE_CURRENT.matches(from).count(), 1);
            changed(LOW_REMOVAL_LANE_CURRENT, from, to)
        } else {
            changed_in_span(LOW_REMOVAL_LANE_CURRENT, 233261..234542, from, to)
        };
        check(&source, "$THREAD_2_96244", false);
    }
}

#[test]
fn assigned_fixed_and_mixed_storage_layouts_are_named() {
    check(FIXED, "$THREAD_9_91527", true);
    check(MIXED, "$THREAD_3_98944", true);
    check(ALIASED, "$THREAD_6_11610", true);
}

#[test]
fn alpha_spelling_and_literal_variants_keep_the_contract() {
    let source = FIXED
        .replace("$KEY_", "slot_")
        // Change numeric thread IDs while retaining generated-name eligibility.
        .replace("$THREAD_", "$THREAD_7")
        .replace("2554166702", "1001")
        .replace("^ 130", "^ 131");
    check(&source, "$THREAD_79_91527", true);
    let source = changed(
        MIXED,
        "$BW[0].call(void 0, $BW[1], $BW[2], 3)",
        "$BW[0]($BW[1], $BW[2], 3)",
    );
    check(&source, "$THREAD_3_98944", true);
}

#[test]
fn readable_callback_name_is_preserved() {
    let source = FIXED.replace("$THREAD_9_91527", "helper_9_91527");
    let result = name_threads(&source, true).expect("fixture must parse and analyze");
    assert!(result.source.contains("function helper_9_91527("));
    assert!(!result.report.outcomes.iter().any(|outcome| {
        outcome.applied && outcome.binding.name == "helper_9_91527"
    }));
}

#[test]
fn fixed_holder_must_prove_tuple_receiver_cursor_and_tag() {
    for (from, to) in [
        ("$CA = [$CA, $CB, $BZ]", "$CA = [$CA, $BZ, $CB]"),
        ("$CA[0].call(void 0", "$CA[0].call(sideEffect()"),
        ("$CA[0].call(void 0", "$CA[1].call(void 0"),
        ("$CA[2], 4)", "$CA[2], 3)"),
        ("$BZ[5] =", "$BZ[6] ="),
        ("$BZ[4] =", "$BZ[3] ="),
        ("$CE[0]($CE[1], $CE[2], 4)", "$CE[0]($CE[1], $CE[2], 3)"),
    ] {
        check(&changed(FIXED, from, to), "$THREAD_9_91527", false);
    }
}

#[test]
fn mixed_frames_must_match_old_value_flags_factories_and_observer() {
    for (from, to) in [
        ("$BT.newValue === null", "$BT.oldValue === null"),
        (
            "$KEY_9 = $BT.oldValue ?? null",
            "$KEY_9 = $BT.newValue ?? null",
        ),
        ("$BW[2], 3)", "$BW[2], 4)"),
        ("$BY = 4", "$BY = 5"),
        ("$CB = $CB($CC, $CA, 3)", "$CB = $CB($CC, $BV, 3)"),
        ("$KEY_20 | 0 | $KEY_16 << 1", "$KEY_16 | 0 | $KEY_20 << 1"),
        ("$KEY_5.next([[], $CA])", "$KEY_13.next([[], $CA])"),
        (
            "$BZ = $KEY_241(($BZ | 0) ^ 721228179)",
            "$BZ = $KEY_241(($BZ | 0) ^ 721228180)",
        ),
        ("let $CB = $KEY_122($BZ)", "let $CB = $KEY_241($BZ)"),
        ("$CA[4] = $BZ() ^ 130", "$CA[4] = $BZ() ^ 131"),
    ] {
        check(&changed(MIXED, from, to), "$THREAD_3_98944", false);
    }
}

#[test]
fn immutable_alias_and_owned_effects_are_required() {
    for (from, to) in [
        ("let $CA = $KEY_41;", "let $CA = $KEY_4;"),
        ("let $CA = $KEY_41;", "let $CA = $KEY_41; $CA = $KEY_4;"),
        ("$BY[$BZ + 1]", "$BY[$BZ + 2]"),
        (
            "$KEY_24.next([[], $CD]);",
            "$KEY_24.next([[], $CD]); $KEY_24.complete();",
        ),
        ("let $CB = $KEY_45;", "let $CB = $KEY_45; sideEffect();"),
        ("if ($CB == null)", "if ($KEY_45 == null)"),
    ] {
        check(&changed(ALIASED, from, to), "$THREAD_6_11610", false);
    }
}

#[test]
fn nested_or_sibling_factory_evidence_cannot_donate_a_live_seed() {
    // The invoked factory must return its own child with its own reaching seed.
    // Keep a real live sibling in scope so name-based/global evidence is present.
    let dead = "function deadSeed(seed) { function child(){return 1 & 255;} return child; }\n";
    let source = changed(
        FIXED,
        "let $KEY_342 = $THREAD_1_115090;",
        "let $KEY_342 = deadSeed;",
    );
    check(&format!("{dead}{source}"), "$THREAD_9_91527", false);
    let dead = "function deadWriter(seed) { function donor(){return seed();} function child(a,b,c){return c;} return child; }\n";
    let source = changed(
        FIXED,
        "let $KEY_426 = $THREAD_9_71496;",
        "let $KEY_426 = deadWriter;",
    );
    check(&format!("{dead}{source}"), "$THREAD_9_91527", false);
    let borrowed = "function foreign(){return 1 & 255;} function borrowed(seed){ function donor(){return seed & 255;} return foreign;}\n";
    let source = changed(
        FIXED,
        "let $KEY_342 = $THREAD_1_115090;",
        "let $KEY_342 = borrowed;",
    );
    check(&format!("{borrowed}{source}"), "$THREAD_9_91527", false);
}

#[test]
fn shadowed_globals_clobbers_and_wrong_branches_are_refused() {
    check(
        &changed(FIXED, "function main(){", "function main(){ let Math = {};"),
        "$THREAD_9_91527",
        false,
    );
    check(
        &changed(MIXED, "if ($BX == null)", "if ($BX != null)"),
        "$THREAD_3_98944",
        false,
    );
    check(
        &changed(MIXED, "let $BY;", "let $BY; $BW = foreign;"),
        "$THREAD_3_98944",
        false,
    );
    check(
        &changed(
            FIXED,
            "$KEY_35.next([[], $BZ]);",
            "function donor(){ $KEY_35.next([[], $BZ]); }",
        ),
        "$THREAD_9_91527",
        false,
    );
}
