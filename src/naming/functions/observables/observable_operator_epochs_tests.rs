use super::*;
const ACTUAL: &str =
    include_str!("../../../../tests/fixtures/joined_stream_state_and_cleanup/constructor-epoch-parent.js");
const MAP: &str = "$THREAD_2_113070";
const INIT: &str = "if (branch) { Constructor = firstMaker(); } else { Constructor = secondMaker(); } Constructor = thirdMaker(); publish($THREAD_2_113070);";

fn inspect(source: &str, check: impl FnOnce(&Model<'_, '_>, &Function<'_>)) {
    let arena = Allocator::default();
    let program = crate::analysis::parse_program(&arena, source).unwrap();
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let m = Model::new(
        source,
        &program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .unwrap();
    let sites: Vec<_> = m
        .all_sites()
        .iter()
        .filter(|s| {
            s.binding_symbol
                .is_some_and(|s| scoping.symbol_name(s) == MAP)
        })
        .collect();
    assert_eq!(sites.len(), 1);
    let AstKind::Function(f) = nodes.kind(sites[0].node_id) else {
        panic!("function")
    };
    check(&m, f);
}
fn callee<'a>(f: &'a Function<'a>) -> &'a Expression<'a> {
    let [_, inner, _] = body(f).unwrap() else {
        panic!("factory")
    };
    let [_, finish] = body(function(inner).unwrap()).unwrap() else {
        panic!("operator")
    };
    let Expression::NewExpression(n) = ret(finish).unwrap() else {
        panic!("new")
    };
    &n.callee
}
fn fixture(tail: &str) -> String {
    // Whole accepted own class/API with explicit opaque operational ports.
    let base =
        include_str!("../../../../tests/fixtures/program/thread-observable-installed-origin.js");
    let start = base.find("function $THREAD_2_81250()").unwrap();
    let end = base.find("return $THREAD_2_81250();").unwrap();
    let maker = &base[start..end];
    let mut s = base[..start].to_string();
    for name in ["firstMaker", "secondMaker", "thirdMaker"] {
        s.push_str(&maker.replacen("$THREAD_2_81250", name, 1));
    }
    s.push_str("let Constructor;\n");
    s.push_str(&ACTUAL[10968..11775].replace("$KEY_225", "Constructor"));
    s.push_str(&ACTUAL[40209..40894].replace("$KEY_225", "Constructor"));
    s.push_str(tail);
    s.push_str("}\nbootstrap();");
    s
}
fn accepted(source: &str, yes: bool) {
    inspect(source, |m, f| {
        assert!(
            native_origin_witness(m, callee(f), symbol(f).unwrap(), 8, false).is_none(),
            "strict guard must refuse multiple stores"
        );
        assert_eq!(prove(m, f, false).is_some(), yes, "full own map proof");
    });
}
#[test]
fn compact_whole_map_epochs_and_all_branch_stores() {
    accepted(&fixture(INIT), true);
    accepted(
        &fixture(INIT)
            .replace("Constructor", "epochConstructor")
            .replace("branch", "otherBranch"),
        true,
    );
    accepted(&fixture("if (branch) { Constructor = firstMaker(); publish($THREAD_2_113070); } else { Constructor = secondMaker(); publish($THREAD_2_113070); } Constructor = thirdMaker();"), true);
}
#[test]
fn compact_constructor_writes_reads_publications_and_makers_refuse() {
    for tail in [
        INIT.replace("Constructor = secondMaker();", "").replace(
            "Constructor = thirdMaker();",
            "publish($THREAD_2_113070); Constructor = thirdMaker();",
        ),
        INIT.replace(
            "Constructor = thirdMaker();",
            "Constructor = foreignMaker();",
        ),
        INIT.replace(
            "Constructor = thirdMaker();",
            "Constructor += thirdMaker();",
        ),
        INIT.replace(
            "Constructor = thirdMaker();",
            "while (looping) { Constructor = thirdMaker(); }",
        ),
        INIT.replace(
            "Constructor = thirdMaker();",
            "function deferred() { Constructor = thirdMaker(); } deferred();",
        ),
        format!("publish($THREAD_2_113070); {INIT}"),
        format!("$THREAD_2_113070(x); {INIT}"),
        format!("const alias = $THREAD_2_113070; publish(alias); {INIT}"),
        format!("let alias; publish(alias); alias = $THREAD_2_113070; {INIT}"),
        format!("const copy = Constructor; {INIT}"),
        format!("{INIT} mutate(Constructor);"),
        format!("{INIT} Constructor.from = other;"),
        format!("{INIT} Constructor.prototype.subscribe = other;"),
        format!("firstMaker(); {INIT}"),
    ] {
        accepted(&fixture(&tail), false);
    }
    // Capture the real operator in every maker's deferred map method.
    let deferred = fixture(INIT).replace("$KEY_433 = mapOperator", "$KEY_433 = $THREAD_2_113070");
    accepted(&deferred, true);
    let late = deferred.replace(
        "const $KEY_108 = teardown, $KEY_433 = $THREAD_2_113070, $KEY_437 = flatOperator;",
        "const $KEY_108 = teardown, $KEY_437 = flatOperator;",
    );
    assert_ne!(late, deferred);
    accepted(
        &late.replace(INIT, &format!("{INIT} const $KEY_433 = $THREAD_2_113070;")),
        false,
    );
    let good = fixture(INIT);

    for (from, to) in [
        (
            "function secondMaker() {",
            "function secondMaker() { publish($THREAD_2_113070);",
        ),
        ("this._subscriber = $AL;", "this._subscriber = other;"),
        ("$KEY_4.next($KEY_0($DJ));", "$KEY_4.next($DJ);"),
        ("$KEY_4.error(e);", "$KEY_4.error(other);"),
        ("$KEY_4.complete();", "$KEY_4.next();"),
        ("$KEY_1.subscribe($DI)", "foreign.subscribe($DI)"),
        ("$KEY_3.unsubscribe()", "foreign.unsubscribe()"),
        ("return $THREAD_0_37257;", "return other;"),
        ("$DI.error = $THREAD_2_50453;", "$DI.error = other;"),
    ] {
        assert!(good.contains(from), "mutation exists: {from}");
        accepted(&good.replace(from, to), false);
    }
}
#[test]
fn actual_guard_roles_and_publication_counterexample() {
    inspect(ACTUAL, |m, f| {
        assert_eq!(f.span, Span::new(10968, 11775));
        let c = callee(f);
        assert_eq!(m.binding_writes(id(m, c).unwrap()), 3);
        assert!(origin(m, id(m, c).unwrap()).is_none());
        assert!(native_origin_witness(m, c, symbol(f).unwrap(), 8, false).is_none());
        assert!(native_origin(m, c, symbol(f).unwrap(), 8).is_some());
        assert_eq!(prove(m, f, false).unwrap().0.len(), 6);
    });
    let result = crate::naming::name_threads(ACTUAL, true).unwrap();
    for (name, role, evaluator) in [
        (MAP, "createMapOperator", "shared_observable_map_operator"),
        (
            "$THREAD_5_41873",
            "mapOperatorFunction",
            "shared_observable_map_operator",
        ),
        (
            "$THREAD_1_134500",
            "mapNextHandler",
            "shared_observable_map_operator",
        ),
        (
            "$THREAD_2_127320",
            "createCallbackObservableAdapter",
            "shared_callback_subscription_adapter",
        ),
        (
            "$THREAD_6_80930",
            "callbackObservableInvoker",
            "shared_callback_subscription_adapter",
        ),
    ] {
        let o = result
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == name)
            .unwrap();
        assert!(o.applied, "{o:?}");
        assert_eq!(o.role_name.as_deref(), Some(role));
        assert_eq!(o.evaluator.as_deref(), Some(evaluator));
        println!("ACTUAL {o:?}");
    }
    let store = "$KEY_225 = $THREAD_5_45035();";
    assert_eq!(ACTUAL.matches(store).count(), 1);
    accepted(
        &ACTUAL.replacen(store, &format!("publish({MAP}); {store}"), 1),
        false,
    );
}

#[test]
fn compact_callback_full_protocol_and_refusals() {
    let source = fixture(&format!("{INIT} publish($THREAD_2_127320);"));
    let check = |s: &str, expected: bool| {
        let report = crate::naming::name_threads(s, true).unwrap().report;
        for (name, role) in [
            ("$THREAD_2_127320", "createCallbackObservableAdapter"),
            ("$THREAD_6_80930", "callbackObservableInvoker"),
        ] {
            let o = report
                .outcomes
                .iter()
                .find(|o| o.binding.name == name)
                .unwrap();
            assert_eq!(
                o.applied && o.evaluator.as_deref() == Some("shared_callback_subscription_adapter"),
                expected,
                "{o:?}"
            );
            if expected {
                assert_eq!(o.role_name.as_deref(), Some(role));
            }
        }
    };
    check(&source, true);
    for (from, to) in [
        (
            "typeof $KEY_0 !== \"function\"",
            "typeof $KEY_0 === \"function\"",
        ),
        (
            "$KEY_2.error(new TypeError());",
            "$KEY_2.error(new Error());",
        ),
        ("$KEY_0.apply(null,", "$KEY_0.apply(other,"),
        ("$KEY_2.next($DH);", "$KEY_2.next(other);"),
        ("$KEY_2.complete();", "$KEY_2.next();"),
        ("$KEY_2.error($DH);", "$KEY_2.error(other);"),
        ("$KEY_1[i] = arguments[i];", "$KEY_1[i] = arguments[0];"),
        ("return $THREAD_6_80930;", "return other;"),
    ] {
        assert!(source.contains(from), "callback mutation exists {from}");
        check(&source.replace(from, to), false);
    }
    check(
        &fixture(&format!("publish($THREAD_2_127320); {INIT}")),
        false,
    );
}

fn native_fixture(source: &str, name: &str, expected: bool) {
    let arena = Allocator::default();
    let program = crate::analysis::parse_program(&arena, source).unwrap();
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let m = Model::new(
        source,
        &program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .unwrap();
    let sites: Vec<_> = m
        .all_sites()
        .iter()
        .copied()
        .filter(|s| {
            s.binding_symbol
                .is_some_and(|s| scoping.symbol_name(s) == name)
        })
        .collect();
    assert_eq!(sites.len(), 1, "unique own maker");
    let maker = sites[0];
    assert_eq!(
        observable_factory::proven_native_class(&m, maker).is_some(),
        expected,
        "complete native API: {name}"
    );
    assert_eq!(
        observable_factory::proven_canonical_native_class(&m, maker).is_some(),
        expected,
        "canonical API: {name}"
    );
    assert_eq!(
        observable_factory::proven_deferred_api_bodies(&m, maker).is_some(),
        expected
    );
    let result = crate::naming::name_threads(source, true).unwrap();
    assert_eq!(
        result.report.outcomes.iter().any(|o| o.applied
            && o.binding.name == name
            && o.evaluator.as_deref() == Some("shared_fast_observable_factory")),
        expected
    );
    assert_eq!(
        result
            .report
            .class_outcomes
            .iter()
            .any(|o| o.applied && o.evaluator.as_deref() == Some("owned_observable_class")),
        expected
    );
}

#[test]
fn eager_decorator_refuses_native_and_all_five_roles() {
    let control =
        include_str!("../../../../tests/fixtures/observable-operator-epochs/control.js");
    let bad = include_str!(
        "../../../../tests/fixtures/observable-operator-epochs/eager-decorator.js"
    );
    assert_eq!(
        control.replacen(
            "static of()",
            "@($THREAD_2_113070(x)(source))\nstatic of()",
            1
        ),
        bad
    );
    accepted(control, true);
    accepted(bad, false);
    for (source, expected) in [(control, true), (bad, false)] {
        inspect(source, |m, _| {
            let maker = m
                .all_sites()
                .iter()
                .copied()
                .find(|s| {
                    s.binding_symbol
                        .is_some_and(|s| m.scoping.symbol_name(s) == "firstMaker")
                })
                .unwrap();
            assert_eq!(
                observable_factory::proven_native_class(m, maker).is_some(),
                expected
            );
            assert_eq!(
                observable_factory::proven_deferred_api_bodies(m, maker).is_some(),
                expected
            );
        });
        let result = crate::naming::name_threads(source, true).unwrap();
        for (name, evaluator) in [
            (MAP, "shared_observable_map_operator"),
            ("$THREAD_5_41873", "shared_observable_map_operator"),
            ("$THREAD_1_134500", "shared_observable_map_operator"),
            ("$THREAD_2_127320", "shared_callback_subscription_adapter"),
            ("$THREAD_6_80930", "shared_callback_subscription_adapter"),
        ] {
            let o = result
                .report
                .outcomes
                .iter()
                .find(|o| o.binding.name == name)
                .unwrap();
            assert_eq!(
                o.applied && o.evaluator.as_deref() == Some(evaluator),
                expected,
                "{o:?}"
            );
        }
        let first = result.report.class_outcomes.first().unwrap();
        assert_eq!(
            first.applied && first.evaluator.as_deref() == Some("owned_observable_class"),
            expected
        );
    }
    println!("exact eager decorator refused by private full native proof, map proof and all five production roles; undecorated control retained");
}

#[test]
fn all_native_api_definition_surfaces_are_consumed() {
    let installed =
        include_str!("../../../../tests/fixtures/program/thread-observable-installed-origin.js");
    let owned = include_str!("../../../../tests/fixtures/program/thread-owned-observable.js");
    for (base, maker, class, formal) in [
        (installed, "$THREAD_2_81250", "$THREAD_2_127309", "$AL"),
        (owned, "$THREAD_5_1328", "$THREAD_6_110488", "$BS"),
    ] {
        native_fixture(base, maker, true);
        for replacement in [
            "@replaceAPI\nstatic of()",
            "static get of()",
            "static [(eager(), 'of')]()",
            "static ['of']()",
        ] {
            assert!(base.contains("static of()"));
            native_fixture(&base.replacen("static of()", replacement, 1), maker, false);
        }
        for head in [
            format!("@replaceAPI\nclass {class} {{"),
            format!("class {class} extends Other {{"),
            format!("class {class} {{ static {{ eager(); }}"),
            format!("class {class} {{ static field = eager();"),
            format!("class {class} {{ field = eager();"),
        ] {
            native_fixture(
                &base.replacen(&format!("class {class} {{"), &head, 1),
                maker,
                false,
            );
        }
        native_fixture(
            &base.replacen(
                &format!("constructor({formal})"),
                &format!("constructor({formal} = eager())"),
                1,
            ),
            maker,
            false,
        );
        native_fixture(
            &base.replacen(
                &format!("function {maker}() {{"),
                &format!("function {maker}() {{ eager();"),
                1,
            ),
            maker,
            false,
        );
        // Noncomputed quoted literal API keys are already supported. Preserve
        // the independently modeled nested `get closed` consumer descriptor.
        assert!(owned.contains("get closed()"));
        native_fixture(
            &base
                .replacen("static of()", "static 'of'()", 1)
                .replace("static empty()", "static \"empty\"()"),
            maker,
            true,
        );
        let decorated = base.replacen(
            &format!("constructor({formal})"),
            &format!("constructor(@replaceAPI {formal})"),
            1,
        );
        let arena = Allocator::default();
        let parsed = crate::analysis::parse(&arena, &decorated);
        if parsed.errors.is_empty() {
            native_fixture(&decorated, maker, false);
        } else {
            println!(
                "parameter decorator parser rejects {maker}: {:?}",
                parsed.errors
            );
        }
    }
}

#[test]
fn deferred_api_witness_contains_only_owned_function_bodies() {
    let source = fixture(INIT);
    inspect(&source, |m, f| {
        let maker = m
            .all_sites()
            .iter()
            .copied()
            .find(|s| {
                s.binding_symbol
                    .is_some_and(|s| m.scoping.symbol_name(s) == "firstMaker")
            })
            .unwrap();
        let spans = observable_factory::proven_deferred_api_bodies(m, maker).unwrap();
        assert_eq!(
            spans.len(),
            8,
            "three class bodies and five exact installed API declarations"
        );
        assert!(!spans.iter().any(|s| s.contains_inclusive(maker.span)));
        for span in spans {
            assert!(m
                .nodes
                .iter()
                .any(|n| matches!(n.kind(), AstKind::FunctionBody(b) if b.span == span)));
        }
        assert!(native_origin(m, callee(f), symbol(f).unwrap(), 8).is_some());
    });
    accepted(
        &fixture(INIT).replacen(
            "function firstMaker() {",
            "function firstMaker() { publish($THREAD_2_113070);",
            1,
        ),
        false,
    );
}
