use super::*;
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/program/observable-mixed-factories/mixed-factory-parent.js"
));
const BEFORE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/program/observable-mixed-factories/mixed-factory-before.map.json"
));
const PARENT: &str = "$THREAD_2_33189";
const MEMBERS: [(&str, &str); 5] = [
    ("$THREAD_4_24972", "convertToObservable"),
    ("$THREAD_8_86291", "subscribeToObservable"),
    ("$THREAD_8_97257", "pipeObservable"),
    ("$THREAD_7_11076", "pipeObservableMapOperator"),
    ("$THREAD_2_77280", "pipeObservableFlatMapOperator"),
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
#[test]
fn actual_guard_closure_and_all_parent_obligations() {
    inspect(SOURCE, |m, site, f| {
        assert_eq!(site.span, Span::new(118549, 123659));
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
        assert!(subscribe_method(m, site, subscribe, class, alias, true).is_some());
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
        assert!(installed_factory_variant(m, site, f, true, true).is_some());
        assert!(proven_native_class(m, site).is_some());
        // Select the own array executor by the complete guarded from protocol,
        // then its exact alternative, never a similarly named donor function.
        let body = body_of(from).unwrap();
        let Statement::IfStatement(input) = &body.statements[2] else {
            panic!()
        };
        let Statement::IfStatement(array_branch) = &block(&input.consequent).unwrap()[2] else {
            panic!()
        };
        let [_, returned] = block(&array_branch.consequent).unwrap() else {
            panic!()
        };
        let (_, executor) = new_instance(m, return_value(returned).unwrap(), class, alias).unwrap();
        let Statement::IfStatement(schedule) = &executor.body.statements[2] else {
            panic!()
        };
        let alternative = schedule.alternate.as_ref().unwrap();
        let [_, Statement::TryStatement(loop_try)] = block(alternative).unwrap() else {
            panic!()
        };
        let [_, _, Statement::DoWhileStatement(loop_), _] = loop_try.block.body.as_slice() else {
            panic!()
        };
        let [array_store, _, _] = block(&loop_.body).unwrap() else {
            panic!()
        };
        let Expression::AssignmentExpression(first) = expression_statement(array_store).unwrap()
        else {
            panic!()
        };
        assert!(
            identifier(m, &first.right).is_some(),
            "first setup store is array identity, not numeric sentinel"
        );
        assert!(mixed_from::array_loop(
            m,
            alternative,
            local(m, one_variable(&executor.body.statements[0]).unwrap()).unwrap(),
            local(
                m,
                one_variable(&block(&array_branch.consequent).unwrap()[0]).unwrap()
            )
            .unwrap()
        )
        .is_none());
        eprintln!("setup-order proof: class/alias/constructor/of split-array allocation/empty PASS; array refresh executes before numeric sentinels, while the strict executor requires sentinel/sentinel/array. Subscribe split complete write, tuple start/catch, cleanup, pipe/operators, captured origins and every actual exposure PASS. Whole native class API proof succeeds.");
    });
    let old: serde_json::Value = serde_json::from_str(BEFORE).unwrap();
    assert_eq!(
        old["outcomes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| o["binding"]["name"] == PARENT)
            .count(),
        1
    );
    assert_eq!(MEMBERS.len(), 5);
}

fn own_change(from: &str, to: &str) -> String {
    let own = &SOURCE[118549..123659];
    assert!(own.contains(from), "missing own mutation {from}");
    format!(
        "{}{}{}",
        &SOURCE[..118549],
        own.replace(from, to),
        &SOURCE[123659..]
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
        assert_eq!(
            installed_factory_variant(m, site, f, true, true).is_some(),
            accepted
        );
        let claims = evaluate(m, site, &Facts::default(), &BTreeMap::new());
        assert_eq!(claims.len(), usize::from(accepted));
        if accepted {
            assert_eq!(claims[0].name, "createObservableClass");
            assert_eq!(claims[0].evaluator, "shared_fast_observable_factory");
        }
        for (binding, name) in MEMBERS {
            let sites:Vec<_>=m.all_sites().iter().copied().filter(|s| matches!(m.nodes.kind(s.node_id),AstKind::Function(f) if f.id.as_ref().is_some_and(|id|id.name==binding))).collect();
            assert_eq!(sites.len(), 1, "own method {binding}");
            let claims = evaluate(m, sites[0], &Facts::default(), &BTreeMap::new());
            assert_eq!(claims.len(), usize::from(accepted), "method {binding}");
            if accepted {
                assert_eq!(claims[0].name, name);
                assert_eq!(
                    claims[0].evaluator,
                    "shared_fast_observable_installed_member"
                );
            }
        }
    });
}
fn production(source: &str, accepted: bool) -> serde_json::Value {
    let result = crate::naming::name_threads(source, true).unwrap();
    for (binding, name) in std::iter::once((PARENT, "createObservableClass")).chain(MEMBERS) {
        let outcomes: Vec<_> = result
            .report
            .outcomes
            .iter()
            .filter(|o| o.binding.name == binding)
            .collect();
        assert_eq!(outcomes.len(), 1);
        let o = outcomes[0];
        let ours = o.evaluator.as_deref().is_some_and(|e| {
            matches!(
                e,
                "shared_fast_observable_factory" | "shared_fast_observable_installed_member"
            )
        });
        assert_eq!(ours, accepted, "binding {binding}: {o:?}");
        if accepted {
            assert!(o.applied);
            assert_eq!(o.proposed_name.as_deref(), Some(name));
            assert!(o.reference_edits > 0);
            assert!(!o.evidence.is_empty());
            assert_eq!(o.role_name.as_deref(), Some(name));
        }
    }
    let classes: Vec<_> = result
        .report
        .class_outcomes
        .iter()
        .filter(|o| o.binding.name == "$THREAD_2_9489")
        .collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].applied, accepted);
    if accepted {
        assert_eq!(classes[0].proposed_name.as_deref(), Some("Observable"));
        assert_eq!(
            classes[0].evaluator.as_deref(),
            Some("owned_observable_class")
        );
        assert!(classes[0].reference_edits > 0);
    }
    serde_json::to_value(&result.report).unwrap()
}
#[test]
fn actual_and_alpha_roles_cardinality_class_and_prior_metadata() {
    private_roles(SOURCE, true);
    let after = production(SOURCE, true);
    let before: serde_json::Value = serde_json::from_str(BEFORE).unwrap();
    for field in ["outcomes", "class_outcomes"] {
        let id = if field == "outcomes" {
            "function_id"
        } else {
            "class_id"
        };
        for old in before[field]
            .as_array()
            .unwrap()
            .iter()
            .filter(|x| x["applied"] == true)
        {
            let next: Vec<_> = after[field]
                .as_array()
                .unwrap()
                .iter()
                .filter(|x| x[id] == old[id])
                .collect();
            assert_eq!(next.len(), 1);
            assert_eq!(old, next[0], "all old known metadata {id} {}", old[id]);
        }
    }
    let alpha = SOURCE
        .replace("$KEY_107", "observerCtorInput")
        .replace("$KEY_74", "wrapperCtorInput")
        .replace("$BU", "retainedArrayOrTuple")
        .replace("$PARAMA", "receiverFormal")
        .replace("$PARAMB", "methodFormal")
        .replace("$PARAMC", "sourceFormal")
        .replace("$PARAMD", "indexFormal")
        .replace("29392", "15023");
    private_roles(&alpha, true);
    production(&alpha, true);
}
#[test]
fn equivalent_split_allocation_and_neutral_declaration_orders() {
    for source in [
        own_change("let $BU, j;","let j, $BU;"),
        own_change("let $BS, $BT;","let $BT, $BS;"),
        own_change("let $KEY_4;\n\t\t\t\t\tlet $KEY_3 = arguments.length;\n\t\t\t\t\t$KEY_4 = new Array($KEY_3);","let $KEY_3 = arguments.length; let $KEY_4 = new Array($KEY_3);"),
    ] {private_roles(&source,true);production(&source,true);}
}
#[test]
fn exact_loop_call_catch_cleanup_and_api_refusals() {
    for (from, to) in [
        ("i < $BU.length", "i <= $BU.length"),
        ("i < $BU.length", "i < $KEY_0.length"),
        ("i = Number(i) + 1", "i = i + 1"),
        ("i = Number(i) + 1", "i = Number(i) + 2"),
        (
            "let i = 0;\n\t\t\t\t\t\t\t\t\tlet $BU, j;",
            "let i = 1; let $BU, j;",
        ),
        ("$BU = $KEY_0;", "$BU = other;"),
        ("$BS = 29392;", "$BS = sideEffect();"),
        (
            "$BU = $KEY_0;\n\t\t\t\t\t\t\t\t\t\t$BS = 29392;",
            "$BS = 29392; $BU = $KEY_0;",
        ),
        (
            "$PARAMB.call($PARAMA, $PARAMC[$PARAMD]);",
            "$PARAMB.call($PARAMB, $PARAMC[$PARAMD]);",
        ),
        ("$PARAMC[$PARAMD]", "$PARAMD[$PARAMC]"),
        ("(j, (j.next), $KEY_0, i)", "(j, (other.next), $KEY_0, i)"),
        (
            "(j, (j.next), $KEY_0, i)",
            "(j, (j.next), $KEY_0, i, sideEffect())",
        ),
        (
            "i = Number(i) + 1, true",
            "sideEffect(), i = Number(i) + 1, true",
        ),
        ("$BU.length &&", "$BU.length ||"),
        ("$KEY_5 >= $KEY_0.length", "$KEY_5 > $KEY_0.length"),
        ("$KEY_3.error(e);", "$KEY_3.error(other);"),
        ("if ($BS === void 0)", "if ($BT === void 0)"),
        ("return void 0;", "return null;"),
        ("this._subscriber = $BP;", "this._subscriber = other;"),
        ("$KEY_4[i] = arguments[i]", "$KEY_4[i] = arguments[0]"),
        (
            "$BU[1].call($BU[0], $BS, $BT)",
            "$BU[1].call($BU[0], $BT, $BS)",
        ),
        (
            "$BU[1].call($BU[0], $BS, $BT)",
            "$BU[1].call($BU[1], $BS, $BT)",
        ),
        ("$BV[1].call($BV[0], e)", "$BV[1].call($BV[1], e)"),
        ("$BT._cleanup = void 0;", "$BT._cleanup = sideEffect();"),
        ("complete: void 0", "complete: sideEffect()"),
        ("$KEY_1.prototype.pipe =", "$KEY_1.otherPrototype.pipe ="),
        (
            "$KEY_1.from = $THREAD_4_24972;",
            "$KEY_1.from = $THREAD_4_24972; expose($THREAD_4_24972);",
        ),
        (
            "return this.pipe($KEY_198($BP));",
            "return this.pipe($KEY_115($BP));",
        ),
        ("return $KEY_1;", "expose($KEY_1); return $KEY_1;"),
        (
            "class $THREAD_2_9489 {",
            "class $THREAD_2_9489 { static eager = sideEffect();",
        ),
        ("\"use strict\";", ""),
    ] {
        let source = own_change(from, to);
        private_roles(&source, false);
        production(&source, false);
    }
}
#[test]
fn capture_copy_publication_later_writes_and_reentry_refusals() {
    let init = "let $KEY_107 = $THREAD_7_92642;";
    for prefix in [
        "$THREAD_2_33189(); ",
        "register($THREAD_2_33189); ",
        "let early = $THREAD_2_33189; early(); ",
    ] {
        let source = global_change(init, &format!("{prefix}{init}"));
        private_roles(&source, false);
        production(&source, false);
    }
    for to in [
        "let $KEY_107 = other;",
        "let $KEY_107 = 1;",
        "let $KEY_107 = $THREAD_7_92642; $KEY_107 = other;",
        "let $KEY_107 = laterCopy; let laterCopy = $THREAD_7_92642;",
    ] {
        let source = global_change(init, to);
        private_roles(&source, false);
        production(&source, false);
    }
    let source = global_change(init, "let $KEY_107 = createInput($THREAD_2_33189);");
    private_roles(&source, false);
    production(&source, false);
}
#[test]
fn every_method_stub_is_refused_by_complete_parent_and_production() {
    let mut ranges = Vec::new();
    inspect(SOURCE, |m, _, f| {
        let Statement::ClassDeclaration(c) = &body_of(f).unwrap().statements[0] else {
            panic!()
        };
        for element in &c.body.body {
            let ClassElement::MethodDefinition(method) = element else {
                panic!()
            };
            ranges.push(method.value.body.as_ref().unwrap().span);
        }
        for (binding, _) in MEMBERS {
            let own = m
                .all_sites()
                .iter()
                .filter_map(|s| match m.nodes.kind(s.node_id) {
                    AstKind::Function(f) if f.id.as_ref().is_some_and(|id| id.name == binding) => {
                        Some(f.body.as_ref().unwrap().span)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(own.len(), 1);
            ranges.push(own[0]);
        }
    });
    assert_eq!(ranges.len(), 8);
    for span in ranges {
        let mut source = SOURCE.to_string();
        source.replace_range(
            span.start as usize + 1..span.end as usize - 1,
            "\"use strict\"; return void 0;",
        );
        private_roles(&source, false);
        production(&source, false);
    }
}
#[test]
fn whole_class_ordinary_and_guarded_setup_controls() {
    let source_class = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/program/observable-mixed-factories/source-class-parent.js"
    ));
    let whole = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/program/observable-mixed-factories/whole-class.js"
    ));
    let ordinary = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/program/observable-setup-order/ordinary-parent.js"
    ));
    let constructor_epoch = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/program/observable-setup-order/constructor-epoch-parent.js"
    ));
    for (source, binding, name, evaluator) in [
        (
            source_class,
            "$THREAD_7_101596",
            "createObservableClass",
            "shared_fast_observable_factory",
        ),
        (
            whole,
            "$THREAD_5_1328",
            "createObservableClass",
            "shared_fast_observable_factory",
        ),
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
