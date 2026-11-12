use shape::naming::name_threads;
const MAP_OPERATOR_SOURCE: &str = include_str!("fixtures/program/observable-map-baseline.js");
const MAP_OPERATOR_ALTERNATE: &str = include_str!("fixtures/program/observable-map-alternate.js");
const EVALUATOR: &str = "shared_observable_map_operator";
fn names(source: &str) -> Vec<String> {
    let result = name_threads(source, true).expect("valid program model");
    result
        .report
        .outcomes
        .iter()
        .filter(|o| o.evaluator.as_deref() == Some(EVALUATOR) && o.applied)
        .map(|o| o.proposed_name.clone().unwrap())
        .collect()
}
fn expected(source: &str) {
    let got = names(source);
    for name in [
        "createMapOperator",
        "mapOperatorFunction",
        "mapNextHandler",
        "unsubscribeMapSource",
    ] {
        assert!(got.iter().any(|n| n == name), "missing {name}; got {got:?}");
    }
    assert_eq!(got.len(), 4);
}
#[test]
fn original_variants_and_alpha_renaming() {
    expected(MAP_OPERATOR_SOURCE);
    expected(MAP_OPERATOR_ALTERNATE);
    expected(
        &MAP_OPERATOR_ALTERNATE.replace("$KEY_362", "constructorCapture")
            .replace("$THREAD_7_53213", "$THREAD_7_999999")
            .replace("$KEY_433", "mapCapture")
            .replace("$AP", "valueFormal")
            .replace("$KEY_3", "sinkCapture"),
    );
}
fn changed(from: &str, to: &str) -> String {
    // Find the current factory by symbol, never by a stale absolute offset.
    let start = MAP_OPERATOR_ALTERNATE.find("function $THREAD_7_53213(").unwrap();
    let end = start + MAP_OPERATOR_ALTERNATE[start..].find("\n\t\t\t\tlet $KEY_93").unwrap();
    let own = &MAP_OPERATOR_ALTERNATE[start..end];
    assert!(own.contains(from), "{from}");
    let mut result = MAP_OPERATOR_ALTERNATE.to_string();
    result.replace_range(start..end, &own.replace(from, to));
    result
}
#[test]
fn changed_semantics_shadow_and_sibling_are_rejected() {
    for (from, to) in [
        ("$KEY_3.next($KEY_0($AP))", "$KEY_3.next($AP)"),
        ("$KEY_3.error(e)", "$KEY_3.error($AP)"),
        ("$KEY_3.complete()", "$KEY_3.next()"),
        ("$AO.error = $THREAD_9_95417", "$AO.error = other"),
        ("$KEY_1.subscribe($AO)", "other.subscribe($AO)"),
        ("$KEY_2.unsubscribe()", "other.unsubscribe()"),
        ("return $THREAD_6_49402;", "return other;"),
        ("$AN => {", "async $AN => {"),
        ("try {", "let $KEY_0 = other; try {"),
        (
            "let $KEY_1 = $AM;",
            "let $KEY_1 = $AM; const $KEY_362 = other;",
        ),
        (
            "$AO.complete = $THREAD_7_94967;",
            "$AO.complete = $THREAD_7_94967; leak($THREAD_7_94967);",
        ),
        (
            "return $THREAD_2_62429;",
            "function $THREAD_1_999999(x){ return x; } return $THREAD_2_62429;",
        ),
    ] {
        assert!(
            names(&changed(from, to)).is_empty(),
            "accepted mutation {from} => {to}"
        );
    }
}
#[test]
fn origin_availability_and_sibling_donor_are_rejected() {
    for source in [
        MAP_OPERATOR_ALTERNATE.replace(
            "$KEY_362 = $THREAD_2_81250();",
            "$KEY_433(x)(source); $KEY_362 = $THREAD_2_81250();",
        ),
        MAP_OPERATOR_ALTERNATE.replace("$KEY_362 = $THREAD_2_81250();", "$KEY_362 = other();"),
        MAP_OPERATOR_ALTERNATE.replace(
            "$KEY_362 = $THREAD_2_81250();",
            "$KEY_362 = $THREAD_2_81250(); $KEY_362 = other;",
        ),
        MAP_OPERATOR_ALTERNATE.replace(
            "$KEY_362 = $THREAD_2_81250();",
            "$THREAD_2_81250(); $KEY_362 = $THREAD_2_81250();",
        ),
    ] {
        assert!(names(&source).is_empty());
    }
}
#[test]
fn replay_and_scoped_reference_targets_are_preserved() {
    let result = name_threads(MAP_OPERATOR_ALTERNATE, true).unwrap();
    let mut replay = MAP_OPERATOR_ALTERNATE.to_string();
    for e in result.report.edits.iter().rev() {
        replay.replace_range(e.start as usize..e.end as usize, &e.replacement);
    }
    assert_eq!(replay, result.source);
    fn refs(source: &str) -> Vec<(u32, u32)> {
        use oxc_span::GetSpan;
        let allocator = oxc_allocator::Allocator::default();
        let parsed = shape::analysis::parse(&allocator, source);
        let built = oxc_semantic::SemanticBuilder::new()
            .with_check_syntax_error(true)
            .build(&parsed.program);
        assert!(built.errors.is_empty());
        let scope = built.semantic.scoping();
        let mut out = Vec::new();
        for s in scope.symbol_ids() {
            for r in scope.get_resolved_references(s) {
                out.push((
                    built.semantic.nodes().kind(r.node_id()).span().start,
                    scope.symbol_span(s).start,
                ));
            }
        }
        out.sort();
        out
    }
    let translated = |p: u32| -> u32 {
        let delta: i64 = result
            .report
            .edits
            .iter()
            .filter(|e| e.end <= p)
            .map(|e| e.replacement.len() as i64 - (e.end - e.start) as i64)
            .sum();
        (p as i64 + delta) as u32
    };
    let before: Vec<_> = refs(MAP_OPERATOR_ALTERNATE)
        .into_iter()
        .map(|(r, b)| (translated(r), translated(b)))
        .collect();
    assert_eq!(before, refs(&result.source));
    for e in &result.report.edits {
        assert!(
            e.replacement.is_empty()
                || MAP_OPERATOR_ALTERNATE[e.start as usize..e.end as usize].starts_with("$THREAD_")
        );
    }
}
