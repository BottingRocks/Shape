use super::super::*;
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/program/observable-mixed-factories/source-class-parent.js"
));
const PARENT: &str = "$THREAD_7_101596";
const MEMBERS: [(&str, &str); 5] = [
    ("$THREAD_0_48952", "convertToObservable"),
    ("$THREAD_8_34488", "subscribeToObservable"),
    ("$THREAD_3_87088", "pipeObservable"),
    ("$THREAD_5_45807", "pipeObservableMapOperator"),
    ("$THREAD_6_85746", "pipeObservableFlatMapOperator"),
];
fn inspect(source: &str, check: impl FnOnce(&Model<'_, '_>, FunctionSite, &Function<'_>)) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = crate::analysis::parse(&allocator, source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let built = oxc_semantic::SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    assert!(built.errors.is_empty(), "{:?}", built.errors);
    let model = Model::new(
        source,
        &parsed.program,
        built.semantic.scoping(),
        built.semantic.nodes(),
        ProgramPropertiesLimits::default(),
    )
    .unwrap();
    let sites: Vec<_> = model.all_sites().iter().copied().filter(|s| matches!(model.nodes.kind(s.node_id), AstKind::Function(f) if f.id.as_ref().is_some_and(|id|id.name==PARENT))).collect();
    assert_eq!(sites.len(), 1);
    let site = sites[0];
    let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
        panic!()
    };
    check(&model, site, f);
}
fn own_change(from: &str, to: &str) -> String {
    let own = &SOURCE[148462..153948];
    assert!(own.contains(from), "missing mutation {from}");
    format!(
        "{}{}{}",
        &SOURCE[..148462],
        own.replace(from, to),
        &SOURCE[153948..]
    )
}
fn global_change(from: &str, to: &str) -> String {
    assert_eq!(
        SOURCE.matches(from).count(),
        1,
        "unique global mutation {from}"
    );
    SOURCE.replacen(from, to, 1)
}
fn private_roles(source: &str, accepted: bool) {
    inspect(source, |m, site, f| {
        assert_eq!(proven_native_class(m, site).is_some(), accepted);
        let proposals = evaluate(m, site, &Facts::default(), &BTreeMap::new());
        assert_eq!(proposals.len(), usize::from(accepted));
        if accepted {
            assert_eq!(proposals[0].name, "createObservableClass");
            assert_eq!(proposals[0].evaluator, "shared_fast_observable_factory");
        }
        for (binding, name) in MEMBERS {
            let sites:Vec<_>=m.all_sites().iter().copied().filter(|s| matches!(m.nodes.kind(s.node_id),AstKind::Function(f) if f.id.as_ref().is_some_and(|id|id.name==binding))).collect();
            assert_eq!(sites.len(), 1);
            let claims = evaluate(m, sites[0], &Facts::default(), &BTreeMap::new());
            assert_eq!(claims.len(), usize::from(accepted), "member {binding}");
            if accepted {
                assert_eq!(claims[0].name, name);
                assert_eq!(
                    claims[0].evaluator,
                    "shared_fast_observable_installed_member"
                );
            }
        }
        assert_eq!(
            installed_factory_variant(m, site, f, true, true).is_some(),
            accepted
        );
    });
}
fn production(source: &str, accepted: bool) {
    let result = crate::naming::name_threads(source, true).unwrap();
    for binding in std::iter::once(PARENT).chain(MEMBERS.map(|x| x.0)) {
        let outcomes: Vec<_> = result
            .report
            .outcomes
            .iter()
            .filter(|o| o.binding.name == binding)
            .collect();
        assert_eq!(outcomes.len(), 1, "binding {binding}");
        let o = outcomes[0];
        let ours = o.evaluator.as_deref().is_some_and(|e| {
            matches!(
                e,
                "shared_fast_observable_factory" | "shared_fast_observable_installed_member"
            )
        });
        assert_eq!(ours, accepted, "production {binding}: {:?}", o);
        if accepted {
            assert!(o.applied);
            assert!(o.reference_edits > 0);
        }
    }
    let classes: Vec<_> = result
        .report
        .class_outcomes
        .iter()
        .filter(|o| o.binding.name == "$THREAD_0_68664")
        .collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].applied, accepted);
}
#[test]
fn actual_first_guard_and_all_other_parent_obligations() {
    inspect(SOURCE, |m, site, f| {
        assert_eq!(site.span, Span::new(148462, 153948));
        let [Statement::ClassDeclaration(c), alias, Statement::FunctionDeclaration(from), _, Statement::FunctionDeclaration(subscribe), _, Statement::FunctionDeclaration(pipe), _, Statement::FunctionDeclaration(map), _, Statement::FunctionDeclaration(flat), _, _] =
            body_of(f).unwrap().statements.as_slice()
        else {
            panic!()
        };
        let class = c.id.as_ref().unwrap().symbol_id.get().unwrap();
        let alias = local(m, one_variable(alias).unwrap()).unwrap();
        for e in &c.body.body {
            let ClassElement::MethodDefinition(method) = e else {
                panic!()
            };
            match property_key_name(&method.key).unwrap().as_str() {
                "constructor" => assert!(constructor_method(m, &method.value).is_some()),
                "of" => assert!(of_method(m, &method.value, class, alias).is_some()),
                "empty" => assert!(empty_method(m, &method.value, class, alias).is_some()),
                _ => panic!(),
            }
        }
        assert!(from_method(m, from, class, alias, true, true).is_none());
        assert!(from_method_variant(m, from, class, alias, true, true, true).is_some());
        assert!(
            subscribe_method(m, site, subscribe, class, alias, true).is_some(),
            "tuple start/catch/cleanup must pass independently"
        );
        assert!(pipe_method(m, pipe).is_some());
        assert!(operator_method(m, site, map).is_some());
        assert!(operator_method(m, site, flat).is_some());
        let uses: Vec<_> = m
            .scoping
            .get_resolved_references(site.binding_symbol.unwrap())
            .map(|r| r.node_id())
            .collect();
        assert_eq!(uses.len(), 1);
        let mut origins = CapturedOrigins {
            model: m,
            site,
            uses,
            valid: true,
        };
        origins.visit_function_body(body_of(f).unwrap());
        assert!(origins.valid);
        assert!(installed_factory(m, site, f, false).is_none());
        assert!(installed_factory(m, site, f, true).is_none());
        assert!(installed_factory_variant(m, site, f, true, true).is_some());
        eprintln!("whole mixed factory first failed method: from; strict_array_executor unscheduled lowering rejects two temporary declarations and three setup stores; subscribe tuple start/catch/cleanup, pipe/operators and all actual capture exposures pass. Typed setup-array/thunk protocol closes complete installed parent.");
    });
    private_roles(SOURCE, true);
    production(SOURCE, true);
}
#[test]
fn alpha_and_neutral_declarations_keep_whole_parent_roles() {
    let alpha = SOURCE
        .replace("$PARAMA", "receiverFormal")
        .replace("$PARAMB", "methodFormal")
        .replace("$PARAMC", "arrayFormal")
        .replace("$PARAMD", "indexFormal")
        .replace("$AI", "retainedArray")
        .replace("42397", "12031");
    private_roles(&alpha, true);
    production(&alpha, true);
    let equivalent = own_change("let $AI, j;", "let j, $AI;");
    private_roles(&equivalent, true);
    production(&equivalent, true);
    let sentinels = own_change("let $AG, $AH;", "let $AH, $AG;");
    private_roles(&sentinels, true);
    production(&sentinels, true);
}
#[test]
fn complete_parent_and_production_refuse_effect_and_protocol_changes() {
    for (from, to) in [
        ("i < $AI.length", "i <= $AI.length"),
        ("i < $AI.length", "i < $KEY_2.length"),
        ("i = Number(i) + 1", "i = i + 1"),
        ("i = Number(i) + 1", "i = Number(i) + 2"),
        (
            "let i = 0;\n\t\t\t\t\t\t\t\t\t\t\tlet $AI, j;",
            "let i = 1; let $AI, j;",
        ),
        ("$AI = $KEY_2;", "$AI = other;"),
        ("$AH = 42397;", "$AH = sideEffect();"),
        (
            "$PARAMB.call($PARAMA, $PARAMC[$PARAMD]);",
            "$PARAMB.call($PARAMB, $PARAMC[$PARAMD]);",
        ),
        ("$PARAMC[$PARAMD]", "$PARAMD[$PARAMC]"),
        (
            "(j, (j.next), $KEY_2, i)",
            "(j, (j.next), $KEY_2, i, extra())",
        ),
        ("(j, (j.next), $KEY_2, i)", "(j, (other.next), $KEY_2, i)"),
        (
            "i = Number(i) + 1, true",
            "sideEffect(), i = Number(i) + 1, true",
        ),
        ("$KEY_5.error(e);", "$KEY_5.error(other);"),
        ("if ($AH === void 0)", "if ($AG === void 0)"),
        ("$AI.length &&", "$AI.length ||"),
        ("$KEY_6 >= $KEY_2.length", "$KEY_6 > $KEY_2.length"),
        (
            "$AI[1].call($AI[0], $AG, $AH)",
            "$AI[1].call($AI[0], $AH, $AG)",
        ),
        ("$AJ[1].call($AJ[0], e)", "$AJ[1].call($AJ[1], e)"),
        ("$AH._cleanup = void 0;", "$AH._cleanup = other;"),
        ("$KEY_1.prototype.pipe =", "$KEY_1.otherPrototype.pipe ="),
        (
            "$KEY_1.from = $THREAD_0_48952;",
            "$KEY_1.from = $THREAD_0_48952; expose($THREAD_0_48952);",
        ),
        (
            "return this.pipe($KEY_199($AD));",
            "return this.pipe($KEY_153($AD));",
        ),
        ("return $KEY_1;", "expose($KEY_1); return $KEY_1;"),
        ("this._subscriber = $AD;", "this._subscriber = other;"),
        ("static empty() {", "static empty() { sideEffect();"),
        (
            ".forEach($AE => $KEY_0.next($AE))",
            ".forEach($AE => $KEY_0.error($AE))",
        ),
        (
            "class $THREAD_0_68664 {",
            "class $THREAD_0_68664 { static eager = sideEffect();",
        ),
        ("\"use strict\";", ""),
    ] {
        let source = own_change(from, to);
        private_roles(&source, false);
        production(&source, false);
    }
}
#[test]
fn capture_origin_readiness_escape_and_reentry_refusals() {
    let init = "let $KEY_372 = $THREAD_2_86243;";
    for prefix in [
        "$THREAD_7_101596(); ",
        "register($THREAD_7_101596); ",
        "let early = $THREAD_7_101596; early(); ",
    ] {
        let source = global_change(init, &format!("{prefix}{init}"));
        private_roles(&source, false);
        production(&source, false);
    }
    for to in [
        "let $KEY_372 = other;",
        "let $KEY_372 = 1;",
        "let $KEY_372 = $THREAD_2_86243; $KEY_372 = other;",
    ] {
        let source = global_change(init, to);
        private_roles(&source, false);
        production(&source, false);
    }
    let source = global_change(
        init,
        "let $KEY_372 = lateProducer; let lateProducer = $THREAD_2_86243;",
    );
    private_roles(&source, false);
    production(&source, false);
}
#[test]
fn saved_ordinary_and_source_class_controls_remain_unchanged() {
    let ordinary = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/program/observable-setup-order/ordinary-parent.js"
    ));
    let constructor_epoch = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/program/observable-setup-order/constructor-epoch-parent.js"
    ));
    let whole = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/program/observable-mixed-factories/whole-class.js"
    ));
    for (source, binding, name, evaluator) in [
        (
            ordinary,
            "$THREAD_2_101607",
            "createSubscriberStreamConstructor",
            "shared_observable_ordinary_constructor_api",
        ),
        (
            constructor_epoch,
            "$THREAD_5_45035",
            "createObservableClass",
            "shared_fast_observable_factory",
        ),
        (
            whole,
            "$THREAD_5_1328",
            "createObservableClass",
            "shared_fast_observable_factory",
        ),
    ] {
        let result = crate::naming::name_threads(source, true).unwrap();
        let outcomes: Vec<_> = result
            .report
            .outcomes
            .iter()
            .filter(|o| o.binding.name == binding)
            .collect();
        assert_eq!(outcomes.len(), 1);
        assert!(outcomes[0].applied);
        assert!(outcomes[0]
            .proposed_name
            .as_deref()
            .unwrap()
            .starts_with(name));
        assert_eq!(outcomes[0].evaluator.as_deref(), Some(evaluator));
    }
}
