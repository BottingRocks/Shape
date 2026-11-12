use shape::naming::name_threads;
const A: &str = include_str!("fixtures/observer_batch_lifecycle/observer-batch-baseline.js");
const B: &str = include_str!("fixtures/observer_batch_lifecycle/observer-batch-alternate.js");
const CANVAS: &str = include_str!("fixtures/observer_batch_lifecycle/reverse-canvas.js");
fn lifecycle(source: &str, expected: bool) {
    let result = name_threads(source, true).expect("valid scoped source");
    let claims: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|o| o.evaluator.as_deref() == Some("shared_observer_batch_lifecycle") && o.applied)
        .collect();
    assert_eq!(claims.len(), if expected { 9 } else { 0 });
    if expected {
        assert_eq!(
            claims
                .iter()
                .filter(|o| o.proposed_name.as_deref() == Some("runObserverBatchLifecycle"))
                .count(),
            1
        );
        assert!(claims
            .iter()
            .all(|o| !o.explanation.is_empty() && !o.evidence.is_empty()));
    }
}
#[test]
fn lifecycle_variants_alpha_and_near_misses() {
    for s in [A, B] {
        lifecycle(s, true);
    }
    lifecycle(
        &A.replace("$KEY_", "renamed_state_")
            .replace("$AQ", "formal_value"),
        true,
    );
    for (from, to) in [
        ("$KEY_0.push($AQ)", "$KEY_0.push(void 0)"),
        ("$AR.error($KEY_2)", "$AR.error($AQ)"),
        ("$KEY_4 = $AR", "$KEY_4 = true"),
        ("$KEY_12 = $KEY_171", "$KEY_12 = void 0"),
        ("$KEY_1();", "$KEY_1(1);"),
        ("$AM(() =>", "$AL(() =>"),
        ("$KEY_0 = [];", "$KEY_0 = []; expose($THREAD_6_118253);"),
        ("$KEY_10 = $AQ;", "$KEY_10 = $AQ; $KEY_12 = $AQ;"),
    ] {
        assert!(A.contains(from));
        lifecycle(&A.replacen(from, to, 1), false);
    }
}
#[test]
fn lifecycle_declaration_availability_and_capture_origins() {
    lifecycle(
        &A.replacen("let $KEY_0;", "", 1)
            .replacen("let $KEY_1;", "let $KEY_0; let $KEY_1;", 1),
        false,
    );
    lifecycle(
        &A.replacen(
            "let $KEY_176=opaqueConstructor",
            "$THREAD_6_11724(); let $KEY_176=opaqueConstructor",
            1,
        ),
        false,
    );
    lifecycle(
        &A.replacen("$KEY_176=opaqueConstructor", "$KEY_176", 1),
        false,
    );
    lifecycle(
        &A.replacen("$KEY_171=opaqueResetValue", "$KEY_171", 1),
        false,
    );
    lifecycle(
        &A.replacen(
            "$KEY_176=opaqueConstructor",
            "$KEY_176=opaqueConstructor",
            1,
        )
        .replacen("let $KEY_0;", "$KEY_176=other; let $KEY_0;", 1),
        false,
    );
}
fn canvas(source: &str, expected: bool) {
    let result = name_threads(source, true).expect("valid naming source");
    let matches: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|o| o.binding.name == "$THREAD_5_51829")
        .collect();
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].applied
            && matches[0].evaluator.as_deref() == Some("shared_reverse_canvas_text_sampling_setup"),
        expected
    );
}
#[test]
fn reverse_rows_samples_alpha_and_near_misses() {
    canvas(CANVAS, true);
    canvas(
        &CANVAS.replace("$KEY_", "state_").replace("$AN", "glyph"),
        true,
    );
    for (from, to) in [
        ("--$AZ;", "++$AZ;"),
        ("$AZ = Number($AZ)", "$AZ = Number($AW)"),
        ("$AY[$AZ]", "$AY[$AW]"),
        ("$KEY_4.length, $AV, 3", "$AW, $AV, 3"),
        ("height = 96", "height = 95"),
        ("$AQ.push($AT[0] << 24", "$AQ.push($AT[0] << 16"),
    ] {
        assert!(CANVAS.contains(from));
        canvas(&CANVAS.replacen(from, to, 1), false);
    }
}
mod retained_canvas {
    include!("naming_canvas_text_sampling_setup.rs");
}

#[test]
fn reverse_canvas_late_captures_require_every_exposure_and_alias_hop_ready() {
    let init = "let $KEY_95=queueFactory, $KEY_19=hashFactory, $KEY_74=numericWriter;";
    let late = CANVAS
        .replacen(init, "let $KEY_95, $KEY_19, $KEY_74;", 1)
        .replacen(
        "return $THREAD_5_51829;",
        "$KEY_95=queueFactory; $KEY_19=hashFactory; $KEY_74=numericWriter; return $THREAD_5_51829;",
        1,
    );
    assert!(CANVAS.contains(init));
    canvas(&late, true);
    canvas(
        &late.replacen(
            "$KEY_95=queueFactory;",
            "$THREAD_5_51829(); $KEY_95=queueFactory;",
            1,
        ),
        false,
    );
    canvas(
        &late.replacen(
            "$KEY_95=queueFactory;",
            "expose($THREAD_5_51829); $KEY_95=queueFactory;",
            1,
        ),
        false,
    );
    canvas(
        &late.replacen(
            "$KEY_95=queueFactory;",
            "if (flag) $KEY_95=queueFactory;",
            1,
        ),
        false,
    );
    canvas(
        &late.replacen(
            "$KEY_95=queueFactory;",
            "defer(()=>$KEY_95=queueFactory);",
            1,
        ),
        false,
    );
    canvas(
        &late.replacen(
            "$KEY_95=queueFactory;",
            "$KEY_95=next; let next=queueFactory;",
            1,
        ),
        false,
    );
    let alias = late.replacen(
        "let $KEY_95, $KEY_19, $KEY_74;",
        "let $KEY_95, $KEY_19, $KEY_74; let readyAlias=$THREAD_5_51829;",
        1,
    );
    canvas(&alias, true);
    canvas(
        &alias.replacen(
            "$KEY_95=queueFactory;",
            "readyAlias(); $KEY_95=queueFactory;",
            1,
        ),
        false,
    );
}
