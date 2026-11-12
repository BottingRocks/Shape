use shape::naming::name_threads;
const EVALUATOR: &str = "shared_canvas_text_sampling_setup";
const HARNESS: &str =
    include_str!("fixtures/canvas_text_sampling_setup/invented-capture-harness.js");
fn claimed(source: &str, target: &str) -> bool {
    let result = name_threads(source, true).expect("valid naming input");
    result
        .report
        .outcomes
        .iter()
        .any(|o| o.binding.name == target && o.evaluator.as_deref() == Some(EVALUATOR) && o.applied)
}
#[test]
fn source_backed_three_closed_variants() {
    for (source, target) in [
        (
            include_str!("fixtures/canvas_text_sampling_setup/closed-queue-setup-alternate.js"),
            "$THREAD_7_68924",
        ),
        (
            include_str!("fixtures/canvas_text_sampling_setup/closed-queue-setup-baseline.js"),
            "$THREAD_8_12",
        ),
        (
            include_str!("fixtures/canvas_text_sampling_setup/closed-queue-setup-event-bootstrap.js"),
            "$THREAD_8_15900",
        ),
    ] {
        assert!(claimed(source, target), "missing {target}");
    }
}
#[test]
fn source_backed_late_capture_is_refused() {
    assert!(!claimed(
        include_str!("fixtures/canvas_text_sampling_setup/late-queue-capture.js"),
        "$THREAD_0_112977"
    ));
}
#[test]
fn invented_capture_harness_positive_and_alpha() {
    assert!(claimed(HARNESS, "$THREAD_7_68924"));
    let alpha = HARNESS
        .replace("$KEY_", "renamedLocal")
        .replace("$BY", "inputGlyph")
        .replace("$CJ", "recordRow");
    assert!(claimed(&alpha, "$THREAD_7_68924"));
}
#[test]
fn closed_protocol_semantic_negatives() {
    // These are invented-capture harness mutations, not source-backed positives.
    for (label, from, to) in [
        (
            "catch parameter effect",
            "catch (e) {",
            "catch ({x = unrelated()}) {",
        ),
        (
            "conditional capture",
            "$KEY_229 = numericWriter;",
            "if (false) $KEY_229 = numericWriter;",
        ),
        (
            "early outer call",
            "let $KEY_321 = queueFactory;",
            "$THREAD_7_68924(null,null,null,null); let $KEY_321 = queueFactory;",
        ),
        ("size", "height = 96", "height = 95"),
        ("budget", "}, 45, $BW)", "}, 44, $BW)"),
        ("budget slot", "}, 45, $BW)", "}, 45, $BV)"),
        (
            "callback replacement",
            "$KEY_321($BY =>",
            "$KEY_321(($BY, extra) =>",
        ),
        ("queue receiver", "$KEY_3.start()", "$KEY_1.start()"),
        ("start argument", "$KEY_3.start()", "$KEY_3.start(1)"),
        ("onEmpty receiver", "$KEY_3.onEmpty(", "$KEY_1.onEmpty("),
        ("onEmpty result", "$KEY_0($KEY_5)", "$KEY_0($CB)"),
        ("cancel instance", "$KEY_3.empty()", "$KEY_1.empty()"),
        ("cancel arguments", "$KEY_3.empty()", "$KEY_3.empty(1)"),
        (
            "early capture",
            "let $KEY_321 = queueFactory;",
            "let $KEY_321 = laterFactory; let laterFactory = queueFactory;",
        ),
        (
            "shadow capture",
            "let $KEY_6 = $BU;",
            "let $KEY_6 = $BU; let $KEY_321 = queueFactory;",
        ),
        (
            "unrelated effect",
            "$KEY_3.start();",
            "$KEY_3.start(); unrelated();",
        ),
        (
            "callback effect",
            "$KEY_7.width = 96;",
            "$KEY_7.width = 96; unrelated();",
        ),
        (
            "extra success effect",
            "$CC.fillStyle = \"#000\";",
            "$CC.fillStyle = \"#000\"; unrelated();",
        ),
        (
            "extra catch effect",
            "catch (e) {",
            "catch (e) { unrelated();",
        ),
        ("return", "return $THREAD_7_137861;", "return $KEY_3;"),
        ("throw", "return void 0;", "throw 0;"),
        ("lane", "$CD[1] << 16", "$CD[1] << 8"),
        (
            "point order",
            "$BY[1][i][0], $BY[1][i][1]",
            "$BY[1][i][1], $BY[1][i][0]",
        ),
        ("wire order", "j >= 0; j--", "j >= 0; j++"),
        (
            "catch accumulator",
            "let $CM = $KEY_5.length",
            "let $CM = $CB.length",
        ),
        (
            "pixel mutation",
            "$CA.call($BZ, $CB);",
            "$CB.length = 0; $CA.call($BZ, $CB);",
        ),
        (
            "state helper identity",
            "$CK = $KEY_267(",
            "$CK = $KEY_229(",
        ),
        (
            "capture overwritten",
            "let $KEY_267 = hashFactory;",
            "let $KEY_267 = hashFactory; $KEY_267 = queueFactory;",
        ),
        (
            "observer overwritten",
            "let $KEY_6 = $BU;",
            "let $KEY_6 = $BU; $BU = other;",
        ),
    ] {
        assert!(HARNESS.contains(from), "invalid mutation {label}");
        assert!(
            !claimed(&HARNESS.replace(from, to), "$THREAD_7_68924"),
            "accepted {label}"
        );
    }
}
#[test]
fn sibling_and_nested_donor_are_not_owners() {
    let source = HARNESS.replace(
        "return $THREAD_7_68924;",
        "function $THREAD_99_1(a,b,c,d){return $THREAD_7_68924(a,b,c,d);} return $THREAD_99_1;",
    );
    assert!(!claimed(&source, "$THREAD_99_1"));
    let source = HARNESS.replace("function $THREAD_7_68924", "function donor").replace("return $THREAD_7_68924;", "function $THREAD_99_1(a,b,c,d){ function nested(){ return donor(a,b,c,d); } return nested; } return $THREAD_99_1;");
    assert!(!claimed(&source, "$THREAD_99_1"));
}
// Preserve the existing focused semantic controls without modifying their source.
#[path = "naming_canvas_text_pixel.rs"]
mod prior_canvas_controls;
