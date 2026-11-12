use shape::naming::name_threads;
const FIRST: &str = include_str!("fixtures/naming/joined_observables/joined-state-excerpt-baseline.js");
const SECOND: &str = include_str!("fixtures/naming/joined_observables/joined-state-excerpt-alternate.js");
const EVAL: &str = "shared_observable_merge_map_operator";
fn roles(s: &str) -> Vec<String> {
    name_threads(s, true)
        .expect("valid whole scope")
        .report
        .outcomes
        .into_iter()
        .filter(|o| o.applied && o.evaluator.as_deref() == Some(EVAL))
        .map(|o| o.role_name.unwrap())
        .collect()
}
fn expect(s: &str) {
    let got = roles(s);
    for name in [
        "createMergeMapOperator",
        "mergeMapOperatorFunction",
        "completeMergeMapSource",
    ] {
        assert!(got.iter().any(|r| r == name), "missing {name}: {got:?}");
    }
    assert_eq!(got.len(), 3);
}
#[test]
fn whole_context_and_alpha_variants() {
    expect(FIRST);
    expect(SECOND);
    expect(
        &SECOND
            .replace("$KEY_38", "observableConstructorCapture")
            .replace("$THREAD_1_89127", "$THREAD_1_999991")
            .replace("$KEY_8", "nextSlot")
            .replace("$AL", "itemFormal"),
    );
}
fn mutation(from: &str, to: &str) -> String {
    let start = SECOND.find("function $THREAD_1_89127(").unwrap();
    let end = start
        + SECOND[start..].find("return $THREAD_1_43437;").unwrap()
        + "return $THREAD_1_43437;".len();
    let own = &SECOND[start..end];
    assert!(own.contains(from), "missing mutation {from}");
    let mut out = SECOND.to_owned();
    out.replace_range(start..end, &own.replace(from, to));
    out
}
#[test]
fn altered_protocols_and_capture_writers_refuse_same_target() {
    for (from, to) in [
        ("++$KEY_2;", "--$KEY_2;"),
        ("$KEY_2 <= 0", "$KEY_2 < 0"),
        ("$KEY_2 = 1;", "$KEY_2 = 2;"),
        ("return $KEY_3.next($AL);", "return $KEY_3.error($AL);"),
        ("return $KEY_3.error($AL);", "return $KEY_3.error(0);"),
        ("$KEY_3.complete();", "$KEY_3.complete(); sideEffect();"),
        ("$KEY_0($AL).subscribe", "$KEY_0(0).subscribe"),
        ("$KEY_4.push($KEY_3);", "$KEY_4.push($KEY_3); $KEY_2 = 7;"),
        (
            "$KEY_5 = $THREAD_1_115352;",
            "if (ready) { $KEY_5 = $THREAD_1_115352; }",
        ),
        (
            "return $KEY_4.forEach($AL => $AL.unsubscribe());",
            "return $KEY_4.forEach($AL => other.unsubscribe());",
        ),
        ("let $KEY_3 = $AK;", "let $KEY_3 = other;"),
    ] {
        assert!(
            roles(&mutation(from, to)).is_empty(),
            "accepted {from} => {to}"
        );
    }
}

fn names_for(s: &str, evaluator: &str) -> Vec<String> {
    name_threads(s, true)
        .unwrap()
        .report
        .outcomes
        .into_iter()
        .filter(|o| o.applied && o.evaluator.as_deref() == Some(evaluator))
        .map(|o| o.role_name.unwrap())
        .collect()
}
#[test]
fn join_whole_context_and_reentrant_initialization() {
    for s in [
        FIRST.to_owned(),
        SECOND.to_owned(),
        SECOND
            .replace("$KEY_38", "joinedConstructor")
            .replace("$KEY_7", "joinedIndex")
            .replace("$THREAD_5_131738", "$THREAD_5_999992"),
    ] {
        let got = names_for(&s, "shared_observable_fork_join");
        assert_eq!(got.len(), 5, "{got:?}");
        assert!(got.iter().any(|n| n == "unsubscribeJoinedSources"));
        assert!(got.iter().any(|n| n == "joinObservableLastValues"));
    }
}
#[test]
fn join_changed_own_protocol_refuses() {
    let start = SECOND.find("function $THREAD_5_131738(").unwrap();
    let end = start
        + SECOND[start..].find("return $THREAD_6_109885;").unwrap()
        + "return $THREAD_6_109885;".len();
    let own = &SECOND[start..end];
    for (from, to) in [
        ("$KEY_3[$KEY_7] = $AN;", "$KEY_3[0] = $AN;"),
        ("--$KEY_4;", "++$KEY_4;"),
        ("$KEY_4 <= 0", "$KEY_4 < 0"),
        ("$KEY_1.error($AN);", "$KEY_1.error($AN); sideEffect();"),
        ("$KEY_5 = [];", "if (ready) { $KEY_5 = []; }"),
        ("$KEY_5 = [];", "$KEY_5 = []; expose($THREAD_6_109885);"),
        (
            "$AM.complete = $THREAD_1_82922;",
            "$AM.complete = $THREAD_2_6022;",
        ),
        ("$KEY_1.next($KEY_3);", "$KEY_1.next($KEY_5);"),
        ("$AO => $AO.unsubscribe()", "$AO => $AO.unsubscribe?.()"),
    ] {
        assert!(own.contains(from), "{from}");
        let mut s = SECOND.to_owned();
        s.replace_range(start..end, &own.replace(from, to));
        assert!(
            names_for(&s, "shared_observable_fork_join").is_empty(),
            "accepted {from} => {to}"
        );
    }
}
#[test]
fn proxy_explicit_branch_whole_context() {
    let got = names_for(SECOND, "shared_browser_proxy_probe");
    assert!(got.contains(&"supportsProxyGetPrototypeOf".to_owned()));
    assert!(got.contains(&"markProxyPrototypeTrap".to_owned()));
    for (from, to) in [
        ("return $KEY_1;", "return {};"),
        ("$KEY_3 = true;", "$KEY_3 = false;"),
        ("getPrototypeOf: $THREAD_5_81473", "getPrototypeOf: other"),
        ("return $KEY_3;", "sideEffect(); return $KEY_3;"),
    ] {
        let start = SECOND.find("function $THREAD_1_122988(").unwrap();
        let end = start + SECOND[start..].find("\n\t\t\t\t}").unwrap() + "\n\t\t\t\t}".len();
        let own = &SECOND[start..end];
        assert!(own.contains(from));
        let mut s = SECOND.to_owned();
        s.replace_range(start..end, &own.replace(from, to));
        assert!(names_for(&s, "shared_browser_proxy_probe").is_empty());
    }
}

#[test]
fn no_counter_entropy_whole_context_and_own_protocol_mutations() {
    let ev = "encoding_entropy_seed_no_counter";
    assert_eq!(names_for(FIRST, ev), vec!["deriveSignalCodecSeed"]);
    let alpha = FIRST
        .replace("$THREAD_3_29056", "$THREAD_3_999993")
        .replace("$KEY_6", "clockWord");
    assert_eq!(names_for(&alpha, ev), vec!["deriveSignalCodecSeed"]);
    let start = FIRST.find("function $THREAD_3_29056(").unwrap();
    let end = start
        + FIRST[start..]
            .find("return ($KEY_5 ^ $KEY_0) >>> 0;")
            .unwrap()
        + "return ($KEY_5 ^ $KEY_0) >>> 0;".len();
    let own = &FIRST[start..end];
    for (from, to) in [
        ("1103515245", "1103515246"),
        ("4294967296", "4294967295"),
        ("$KEY_4 === $KEY_4", "$KEY_4 !== $KEY_4"),
        ("catch (e) {}", "catch (e) { sideEffect(); }"),
        ("$KEY_6 = $KEY_6;", "$KEY_6 = other;"),
        ("$KEY_5 ^ $KEY_0", "$KEY_5 + $KEY_0"),
        ("let $KEY_4 =", "let performance = other; let $KEY_4 ="),
    ] {
        assert!(own.contains(from));
        let mut s = FIRST.to_owned();
        s.replace_range(start..end, &own.replace(from, to));
        assert!(names_for(&s, ev).is_empty(), "accepted {from} => {to}");
    }
}
#[test]
fn native_capture_preinit_branch_reentry_and_later_writer_refuse() {
    for replacement in [
        "$KEY_145(x)(source); $KEY_38 = $THREAD_1_28126();",
        "register(() => $KEY_145(x)(source)); $KEY_38 = $THREAD_1_28126();",
        "if (ready) { $KEY_38 = $THREAD_1_28126(); }",
        "$KEY_38 = $THREAD_1_28126(); $KEY_38 = other;",
    ] {
        let s = SECOND.replacen("$KEY_38 = $THREAD_1_28126();", replacement, 1);
        assert_ne!(s, SECOND);
        assert!(roles(&s).is_empty(), "accepted {replacement}");
    }
}

// Keep the existing owned map's whole original/alpha and protocol controls in
// this single focused target, along with the factory identity regression.
#[path = "naming_observable_map_operator.rs"]
mod owned_map;

fn named_expression(source: &str, external: &str) -> String {
    use oxc_span::GetSpan;
    let allocator = oxc_allocator::Allocator::default();
    let parsed = shape::analysis::parse(&allocator, source);
    let built = oxc_semantic::SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    assert!(built.errors.is_empty());
    let span = built
        .semantic
        .nodes()
        .iter()
        .find_map(|node| {
            if let oxc_ast::AstKind::Function(f) = node.kind() {
                if f.id.as_ref().is_some_and(|id| id.name == external) {
                    return Some(f.span());
                }
            }
            None
        })
        .expect("actual factory declaration");
    let own = &source[span.start as usize..span.end as usize];
    let expression = own.replacen(
        &format!("function {external}("),
        &format!("let {external} = function $THREAD_9_999994("),
        1,
    ) + ";";
    assert_ne!(expression, own);
    let mut result = source.to_owned();
    result.replace_range(span.start as usize..span.end as usize, &expression);
    result
}

#[test]
fn named_expression_external_binding_preinit_exposure_refuses() {
    let map = include_str!("fixtures/program/observable-map-alternate.js");
    for (source, external, initialization, invocation, evaluator) in [
        (
            SECOND,
            "$THREAD_1_89127",
            "$KEY_38 = $THREAD_1_28126();",
            "$THREAD_1_89127(v => v)({})",
            EVAL,
        ),
        (
            SECOND,
            "$THREAD_5_131738",
            "$KEY_38 = $THREAD_1_28126();",
            "$THREAD_5_131738([])",
            "shared_observable_fork_join",
        ),
        (
            map,
            "$THREAD_7_53213",
            "$KEY_362 = $THREAD_2_81250();",
            "$THREAD_7_53213(v => v)({})",
            "shared_observable_map_operator",
        ),
    ] {
        let expression = named_expression(source, external);
        for exposure in [
            String::new(),
            format!("{invocation};"),
            format!(
                "let earlyAlias = {external}; {};",
                invocation.replace(external, "earlyAlias")
            ),
            format!("register({external});"),
            format!("register(() => {{ {invocation}; }});"),
            format!("(function(callback) {{ callback(); }})(() => {{ {invocation}; }});"),
        ] {
            assert!(expression.contains(initialization));
            let s = expression.replacen(initialization, &format!("{exposure} {initialization}"), 1);
            let allocator = oxc_allocator::Allocator::default();
            let parsed = shape::analysis::parse(&allocator, &s);
            let built = oxc_semantic::SemanticBuilder::new()
                .with_check_syntax_error(true)
                .build(&parsed.program);
            assert!(built.errors.is_empty(), "valid whole-scope control");
            let scope = built.semantic.scoping();
            let external_symbol = scope
                .symbol_ids()
                .find(|&symbol| scope.symbol_name(symbol) == external)
                .expect("actual external callable binding");
            assert!(scope.get_resolved_reference_ids(external_symbol).len() > 0);
            let private_symbol = scope
                .symbol_ids()
                .find(|&symbol| scope.symbol_name(symbol) == "$THREAD_9_999994")
                .expect("private self binding");
            assert_ne!(external_symbol, private_symbol);
            assert!(scope.get_resolved_reference_ids(private_symbol).is_empty());
            let private_start = s.find("function $THREAD_9_999994(").unwrap() as u32;
            let result = name_threads(&s, true).expect("valid whole-scope expression control");
            let target = result
                .report
                .outcomes
                .iter()
                .find(|o| {
                    o.binding.declaration.start == private_start
                        && o.binding.name == "$THREAD_9_999994"
                })
                .expect("exact named-expression factory target must be reported");
            assert!(!target.applied, "accepted {external}: {exposure}");
            assert!(
                result
                    .report
                    .outcomes
                    .iter()
                    .all(|o| !o.applied || o.evaluator.as_deref() != Some(evaluator)),
                "accepted private factory protocol {external}: {exposure}"
            );
        }
    }
}

#[path = "ownership/joined_observables_teardown.rs"]
mod join_teardown;
