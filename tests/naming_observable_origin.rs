use shape::naming::name_threads;
const SOURCE: &str = include_str!("fixtures/program/thread-observable-installed-origin.js");
fn check(source: &str, expected: bool) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = shape::analysis::parse(&allocator, source);
    assert!(
        !parsed.panicked && parsed.errors.is_empty(),
        "{:?}",
        parsed.errors
    );
    shape::naming::collectors::identify_collectors(source, false)
        .expect("valid Model");
    let result = name_threads(source, true).unwrap();
    let factory = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_2_81250")
        .unwrap();
    assert_eq!(
        factory.applied && factory.evaluator.as_deref() == Some("shared_fast_observable_factory"),
        expected
    );
    let class = result
        .report
        .class_outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_2_127309")
        .unwrap();
    assert_eq!(
        class.applied && class.proposed_name.as_deref() == Some("Observable"),
        expected
    );
    if !expected {
        assert!(
            !result
                .report
                .outcomes
                .iter()
                .any(|o| o.evaluator.as_deref() == Some("shared_fast_observable_installed_member")),
            "an incomplete factory must not certify any installed declaration"
        );
    }
}
#[test]
fn installed_observable_proves_complete_same_origin_protocol() {
    check(SOURCE, true);
    check(
        &SOURCE
            .replace("$KEY_0", "constructorAlias")
            .replace("$PARAMA", "receiverFormal"),
        true,
    );
}
#[test]
fn installed_observable_refuses_changed_installations_and_protocol() {
    for (from, to) in [
        ("$KEY_0.from =", "other.from ="),
        ("$KEY_0.prototype.subscribe =", "$KEY_0.subscribe ="),
        ("$KEY_0.prototype.pipe = $THREAD_6_23756;", ""),
        (
            "$KEY_0.prototype.map = $THREAD_0_111722;",
            "$KEY_0.prototype.map = $THREAD_7_146447;",
        ),
        ("return $KEY_0;", "return other;"),
        ("let $KEY_0 = $THREAD_2_127309;", "let $KEY_0 = other;"),
        ("this._subscriber = $AL;", "this._subscriber = other;"),
        ("$KEY_0.complete();", "$KEY_0.next();"),
        ("$KEY_5.next();", "$KEY_5.error();"),
        ("!$AO.done", "$AO.done"),
        ("i = $KEY_1", "i = other"),
        ("(i.next)", "(i.error)"),
        ("$AO, \"value\"", "$AO, \"done\""),
        (
            "$PARAMB.call($PARAMA, $PARAMC[$PARAMD])",
            "$PARAMB.call(other, $PARAMC[$PARAMD])",
        ),
        (
            "$PARAMB.call($PARAMA, $PARAMC[$PARAMD])",
            "$PARAMB.call($PARAMA, $PARAMC[0])",
        ),
        (
            "$PARAMB.call($PARAMA, $PARAMC[$PARAMD])",
            "return $PARAMB.call($PARAMA, $PARAMC[$PARAMD])",
        ),
        (
            "$KEY_0.from = $THREAD_9_146693;",
            "leak($THREAD_9_146693); $KEY_0.from = $THREAD_9_146693;",
        ),
        (
            "$KEY_0.from = $THREAD_9_146693;",
            "$KEY_0.from = $THREAD_9_146693; $KEY_0 = other;",
        ),
        ("$AL => {", "async $AL => {"),
    ] {
        assert!(SOURCE.contains(from), "missing mutation {from}");
        check(&SOURCE.replace(from, to), false);
    }
}

#[test]
fn installed_observable_requires_available_immutable_captures() {
    let aliases = "const $KEY_351 = Observer, $KEY_168 = Subscription;";
    let late = SOURCE.replace(aliases, "").replace(
        "return $THREAD_2_81250();",
        &format!("const result = $THREAD_2_81250(); {aliases} return result;"),
    );
    check(&late, false);
    let mutable = SOURCE.replace("class Observer {}", "class Observer {} Observer = other;");
    check(&mutable, false);
    let early = SOURCE.replace(aliases, &format!("$THREAD_2_81250(); {aliases}"));
    check(&early, false);
}

#[test]
fn installed_observable_replays_and_preserves_scoped_references() {
    replay_preserves(SOURCE);
    replay_preserves(&invoked_array_source());
}

fn replay_preserves(source: &str) {
    let result = name_threads(source, true).unwrap();
    let mut replay = source.to_string();
    for edit in result.report.edits.iter().rev() {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
    let translated = |position: u32| -> u32 {
        let delta: i64 = result
            .report
            .edits
            .iter()
            .filter(|e| e.end <= position)
            .map(|e| e.replacement.len() as i64 - (e.end - e.start) as i64)
            .sum();
        (position as i64 + delta) as u32
    };
    fn refs(source: &str) -> Vec<(u32, u32)> {
        use oxc_span::GetSpan;
        let allocator = oxc_allocator::Allocator::default();
        let parsed = shape::analysis::parse(&allocator, source);
        let built = oxc_semantic::SemanticBuilder::new()
            .with_check_syntax_error(true)
            .build(&parsed.program);
        assert!(built.errors.is_empty(), "{:?}", built.errors);
        let scoping = built.semantic.scoping();
        let mut refs = Vec::new();
        for symbol in scoping.symbol_ids() {
            for reference in scoping.get_resolved_references(symbol) {
                refs.push((
                    built
                        .semantic
                        .nodes()
                        .kind(reference.node_id())
                        .span()
                        .start,
                    scoping.symbol_span(symbol).start,
                ));
            }
        }
        refs.sort();
        refs
    }
    let original: Vec<_> = refs(source)
        .into_iter()
        .map(|(reference, binding)| (translated(reference), translated(binding)))
        .collect();
    assert_eq!(original, refs(&result.source));
    // Only generated declaration/reference identifiers can change. All native
    // and property keys and captured carrier spellings survive byte-for-byte.
    for edit in &result.report.edits {
        assert!(source[edit.start as usize..edit.end as usize].starts_with("$THREAD_"));
    }
}

#[test]
fn installed_observable_names_only_fully_proven_installed_declarations() {
    installed_names(SOURCE);
    installed_names(
        &SOURCE
            .replace("$KEY_0", "constructorAlias")
            .replace("$PARAMA", "receiverFormal"),
    );
}

fn installed_names(source: &str) {
    let result = name_threads(source, true).unwrap();
    for (binding, name) in [
        ("$THREAD_9_146693", "convertToObservable"),
        ("$THREAD_9_152576", "subscribeToObservable"),
        ("$THREAD_6_23756", "pipeObservable"),
        ("$THREAD_0_111722", "pipeObservableMapOperator"),
        ("$THREAD_7_146447", "pipeObservableFlatMapOperator"),
    ] {
        let outcome = result
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == binding)
            .unwrap();
        assert_eq!(outcome.proposed_name.as_deref(), Some(name), "{binding}");
        assert!(outcome.applied);
        assert_eq!(
            outcome.evaluator.as_deref(),
            Some("shared_fast_observable_installed_member")
        );
    }
}

#[test]
fn installed_observable_rejects_sibling_and_shadowed_installation_evidence() {
    let marker = "$KEY_0.prototype.pipe = $THREAD_6_23756;";
    for replacement in [
        "{ let $KEY_0 = other; $KEY_0.prototype.pipe = $THREAD_6_23756; }",
        "(() => { $KEY_0.prototype.pipe = $THREAD_6_23756; });",
        "function sibling(){ $KEY_0.prototype.pipe = $THREAD_6_23756; }",
        "$KEY_0.prototype.pipe = other;",
    ] {
        check(&SOURCE.replace(marker, replacement), false);
    }
    let donor = SOURCE.replace(
        "function $THREAD_2_81250() {",
        "function $THREAD_8_77777() { return arguments[0]; } function $THREAD_2_81250() {",
    );
    installed_names(&donor);
    let result = name_threads(&donor, true).unwrap();
    let sibling = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_8_77777")
        .unwrap();
    assert_ne!(
        sibling.evaluator.as_deref(),
        Some("shared_fast_observable_installed_member")
    );
}

// Exact array-loop lowering placed into the complete installed factory. The iterator branch
// and every other member stay intact.
fn invoked_array_source() -> String {
    let source = SOURCE;
    let start = source[..source.find("let $AO = 0;").unwrap()]
        .rfind("try {")
        .unwrap();
    let error = start + source[start..].find("$KEY_3.error(e);").unwrap();
    let end = error + source[error..].find('}').unwrap() + 1;
    let replacement = r#"let sentinelA, sentinelB;
    try {
        let i = 0;
        let j;
        do {
            sentinelA = 72388;
            sentinelB = 2;
        } while (i < $KEY_1.length && (j = $KEY_3, (($PARAMA, $PARAMB, $PARAMC, $PARAMD) => {
            $PARAMB.call($PARAMA, $PARAMC[$PARAMD]);
        })(j, (j.next), $KEY_1, i), i = Number(i) + 1, true));
        $KEY_3.complete();
    } catch (e) {
        $KEY_3.error(e);
        if (sentinelA === void 0) {
            throw new TypeError("Cannot read properties of undefined (reading '0')");
        }
        if (sentinelB === void 0) {
            throw new TypeError("Cannot read properties of undefined (reading 'undefined')");
        }
    }"#;
    let mut source = source.to_owned();
    source.replace_range(start..end, replacement);
    source.push_str("\nfunction main(){ return bootstrap(); } main();\n");
    source
}

#[test]
fn installed_observable_invoked_array_positive() {
    let source = invoked_array_source();
    check(&source, true);
    installed_names(&source);
    let alpha = source
        .replace("$PARAMA", "receiverFormal")
        .replace("$PARAMB", "methodFormal")
        .replace("$PARAMC", "arrayFormal")
        .replace("$PARAMD", "indexFormal")
        .replace("sentinelA", "firstMark")
        .replace("sentinelB", "secondMark")
        .replace("let j;", "let receiverTemp;")
        .replace("j = $KEY_3", "receiverTemp = $KEY_3")
        .replace("})(j, (j.next)", "})(receiverTemp, (receiverTemp.next)");
    check(&alpha, true);
    installed_names(&alpha);
}

#[test]
fn installed_observable_invoked_array_rejects_changed_semantics() {
    let source = invoked_array_source();
    for (from, to) in [
        ("let i = 0;\n        let j;", "let i = 1;\n        let j;"),
        ("sentinelA = 72388;", "sentinelB = 72388;"),
        ("sentinelB = 2;", "sentinelB = void 0;"),
        ("i < $KEY_1.length &&", "i <= $KEY_1.length &&"),
        ("j = $KEY_3,", "j = other,"),
        ("})(j, (j.next), $KEY_1, i)", "})(j, (j.error), $KEY_1, i)"),
        (
            "})(j, (j.next), $KEY_1, i)",
            "})(other, (j.next), $KEY_1, i)",
        ),
        ("})(j, (j.next), $KEY_1, i)", "})(j, (j.next), other, i)"),
        ("})(j, (j.next), $KEY_1, i)", "})(j, (j.next), $KEY_1, 0)"),
        (
            "})(j, (j.next), $KEY_1, i)",
            "})(j, (j.next), $KEY_1, i, extra)",
        ),
        (
            "$PARAMB.call($PARAMA, $PARAMC[$PARAMD]);",
            "$PARAMB.call($PARAMC, $PARAMC[$PARAMD]);",
        ),
        ("$PARAMC[$PARAMD]", "$PARAMC[0]"),
        (
            "$PARAMB.call($PARAMA, $PARAMC[$PARAMD]);",
            "return $PARAMB.call($PARAMA, $PARAMC[$PARAMD]);",
        ),
        (
            "$PARAMB.call($PARAMA, $PARAMC[$PARAMD]);",
            "$PARAMB.call($PARAMA, $PARAMC[$PARAMD]); extra();",
        ),
        ("i = Number(i) + 1, true", "i = Number(i) + 2, true"),
        ("i = Number(i) + 1, true", "i = Number(i) + 1, false"),
        (
            "$KEY_3.complete();\n    } catch",
            "$KEY_3.next();\n    } catch",
        ),
        (
            "$KEY_3.error(e);\n        if",
            "$KEY_3.error(other);\n        if",
        ),
        ("if (sentinelA === void 0)", "if (sentinelB === void 0)"),
        ("let j;", "let j = other;"),
        (
            "(($PARAMA, $PARAMB, $PARAMC, $PARAMD) =>",
            "(async ($PARAMA, $PARAMB, $PARAMC, $PARAMD) =>",
        ),
    ] {
        assert!(source.contains(from), "missing mutation {from}");
        check(&source.replace(from, to), false);
    }
    check(
        &source.replace("let j;", "let j; const Number = other;"),
        false,
    );
    check(
        &source.replace("$PARAMC[$PARAMD]);", "$PARAMC[$PARAMD]); $PARAMD = 0;"),
        false,
    );
    check(
        &source
            .replace(
                "$PARAMB.call($PARAMA,",
                "(($PARAMA) => $PARAMB.call($PARAMA,",
            )
            .replace("$PARAMC[$PARAMD]);", "$PARAMC[$PARAMD]))(other);"),
        false,
    );
}

#[test]
fn installed_observable_invoked_array_requires_own_installations_and_early_origins() {
    let source = invoked_array_source();
    for replacement in [
        "{ let $KEY_0 = other; $KEY_0.from = $THREAD_9_146693; }",
        "function sibling(){ $KEY_0.from = $THREAD_9_146693; }",
        "$KEY_0.from = other;",
    ] {
        check(
            &source.replace("$KEY_0.from = $THREAD_9_146693;", replacement),
            false,
        );
    }
    let aliases = "const $KEY_351 = Observer, $KEY_168 = Subscription;";
    check(
        &source.replace(aliases, "").replace(
            "return $THREAD_2_81250();",
            &format!("const result = $THREAD_2_81250(); {aliases} return result;"),
        ),
        false,
    );
    let donor = source.replace(
        "function $THREAD_2_81250() {",
        "function $THREAD_8_77777(){ return arguments[0]; } function $THREAD_2_81250() {",
    );
    installed_names(&donor);
    let result = name_threads(&donor, true).unwrap();
    let sibling = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_8_77777")
        .unwrap();
    assert_ne!(
        sibling.evaluator.as_deref(),
        Some("shared_fast_observable_installed_member")
    );
}
