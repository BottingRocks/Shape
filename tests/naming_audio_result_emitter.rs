//! Full saved cleaned source, including original lexical captures and helper
//! definitions. These are source-backed tests; no invented capture harness.
use shape::naming::name_threads;
const SOURCE: &str = include_str!("fixtures/program/audio-result/audio-result-alternate.js");
const BINDING: &str = "$THREAD_7_27797";
const START: usize = 128704;
const END: usize = 129304;

fn check(source: &str, binding: &str, expected: bool) {
    let a = oxc_allocator::Allocator::default();
    let p = oxc_parser::Parser::new(&a, source, oxc_span::SourceType::unambiguous()).parse();
    assert!(p.errors.is_empty(), "invalid mutation: {:?}", p.errors);
    let result = name_threads(source, true).unwrap();
    let o = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == binding)
        .unwrap();
    assert_eq!(
        o.applied && o.proposed_name.as_deref() == Some("emitSeededChannelResult"),
        expected,
        "{o:?}"
    );
    if expected {
        assert_eq!(
            o.evaluator.as_deref(),
            Some("encoding_audio_result_emitter")
        );
    }
}
fn changed(from: &str, to: &str) -> String {
    let body = &SOURCE[START..END];
    assert!(body.contains(from), "missing mutation {from}");
    format!(
        "{}{}{}",
        &SOURCE[..START],
        body.replace(from, to),
        &SOURCE[END..]
    )
}
#[test]
fn audio_result_positive_and_alpha() {
    check(SOURCE, BINDING, true);
    check(
        &SOURCE
            .replace(BINDING, "$THREAD_7_99001")
            .replace("$CH", "channelFrameAlpha")
            .replace("$CI", "channelCursorAlpha"),
        "$THREAD_7_99001",
        true,
    );
}
#[test]
fn audio_result_semantic_negatives() {
    for (a, b) in [
        ("getChannelData(0)", "getChannelData(1)"),
        ("$KEY_11[0]", "$KEY_11[1]"),
        ("Array.from($KEY_7)", "Array.from([])"),
        ("$KEY_5 ^ 4191055287", "$KEY_5 + 4191055287"),
        ("i >= 0", "i > 0"),
        ("i--", "i++"),
        ("$KEY_7[i]", "$KEY_7[0]"),
        ("$KEY_7.length - 1", "$KEY_7.length"),
        ("$CH, $CI)", "$CH, 3)"),
        ("$KEY_3.complete()", "$KEY_11.complete()"),
        ("$KEY_3.next([[], $CH])", "$KEY_3.next([[], $KEY_7])"),
        ("$KEY_7 = Array.from", "unexpected(); $KEY_7 = Array.from"),
        ("let $KEY_7 =", "let Math = other; let $KEY_7 ="),
        ("let $KEY_7 =", "let Array = other; let $KEY_7 ="),
        ("let $KEY_7 =", "let $KEY_11 = []; let $KEY_7 ="),
    ] {
        check(&changed(a, b), BINDING, false);
    }
}
#[test]
fn audio_result_origin_return_and_early_capture() {
    // Exact real helper's seed initializer: removing live seed flow must fail.
    let from = "$KEY_1 = $BM;\n\t\t\t$KEY_3 = -1;";
    assert!(SOURCE.contains(from));
    check(
        &SOURCE.replace(from, "$KEY_1 = 0;\n\t\t\t$KEY_3 = -1;"),
        BINDING,
        false,
    );
    let writer = &SOURCE[15385..18801];
    assert!(writer.contains("return $KEY_2;"));
    let wrong = format!(
        "{}{}{}",
        &SOURCE[..15385],
        writer.replace("return $KEY_2;", "return 0;"),
        &SOURCE[18801..]
    );
    check(&wrong, BINDING, false);
    // Hoisted direct use before capture initialization/registration is refused.
    check(
        &format!("{}{}({{}});{}", &SOURCE[..START], BINDING, &SOURCE[START..]),
        BINDING,
        false,
    );
}
#[test]
fn audio_result_other_saved_variants() {
    check(
        include_str!("fixtures/program/audio-result/audio-result-heldout.js"),
        "$THREAD_8_34114",
        true,
    );
    let source = include_str!("fixtures/program/audio-result/audio-result-baseline.js");
    check(source, "$THREAD_1_133417", true);
    for (a, b) in [
        ("$KEY_19[0]", "$KEY_19[1]"),
        ("$DH >= 0", "$DH > 0"),
        ("--$DH", "++$DH"),
        ("$KEY_1[$DH]", "$KEY_1[0]"),
    ] {
        // Mutate only the exact saved declaration.
        let start = 199942;
        let end = 200549;
        assert!(source[start..end].contains(a));
        let changed = format!(
            "{}{}{}",
            &source[..start],
            source[start..end].replace(a, b),
            &source[end..]
        );
        check(&changed, "$THREAD_1_133417", false);
    }
}
