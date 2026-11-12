use super::*;
const SOURCE: &str =
    include_str!("../../../../../tests/fixtures/program/observable-mixed-factories/source-class-parent.js");
const LAST: &str = "$THREAD_5_3651";
const SCAN: &str = "$THREAD_0_70818";
fn inspect(source: &str, name: &str, check: impl FnOnce(&Model<'_, '_>, &Function<'_>)) {
    let arena = Allocator::default();
    let program = crate::analysis::parse_program(&arena, source).unwrap();
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let model = Model::new(
        source,
        &program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .unwrap();
    let sites: Vec<_> = model
        .all_sites()
        .iter()
        .filter(|s| {
            s.binding_symbol
                .is_some_and(|s| scoping.symbol_name(s) == name)
        })
        .collect();
    assert_eq!(sites.len(), 1);
    let AstKind::Function(f) = nodes.kind(sites[0].node_id) else {
        panic!()
    };
    check(&model, f);
}
#[test]
fn actual_initial_guard() {
    for name in [LAST, SCAN] {
        inspect(SOURCE, name, |m, f| {
            let p = params(m, f, 2).unwrap();
            let ss = body(f).unwrap();
            formal_copies(m, &ss[0], &ss[1], &p).expect("formal capture correspondence");
            let inner = function(&ss[ss.len() - 2]).unwrap();
            let bs = body(inner).unwrap();
            let Expression::NewExpression(n) = ret(&bs[1]).unwrap().without_parentheses() else {
                panic!()
            };
            let constructor = id(m, &n.callee).unwrap();
            let (value, owner, end) = observables::observable_map_operator::origin(m, constructor)
                .expect("unique actual constructor origin");
            let maker = stable_function(m, &call(value).unwrap().callee, 0).unwrap();
            let native = observables::observable_factory::proven_native_class(m, maker).is_some();
            let exposures:Vec<_>=m.scoping.get_resolved_references(fs(f).unwrap()).map(|r|serde_json::json!({"span":format!("{:?}",m.nodes.kind(r.node_id()).span()),"parent":format!("{:?}",m.nodes.parent_kind(r.node_id())),"ready":callable_origin_precedes(m,owner,end,r.node_id(),None,0,true)})).collect();
            let old_origin =
                observables::observable_map_operator::native_origin(m, &n.callee, fs(f).unwrap(), 8)
                    .is_some();
            let full = if name == LAST { last(m, f) } else { scan(m, f) };
            println!(
                "{}",
                serde_json::json!({"factory":name,"constructor":format!("{:?}",constructor),"maker":format!("{:?}",maker.span),"class_body":native,"unique_constructor_origin":true,"constructor_end":end,"exposures":exposures,"old_native_origin":old_origin,"full_parent":full.is_some()})
            );
            assert!(native, "actual source class already has a whole proof");
            assert!(
                !old_origin,
                "measure current actual first native-origin refusal"
            );
            assert_eq!(
                full.as_ref().map(Vec::len),
                Some(2),
                "all parent roles must be proved"
            );
        });
    }
}

#[test]
fn actual_whole_parents_and_production() {
    for name in [LAST, SCAN] {
        inspect(SOURCE, name, |m, f| {
            let ss = body(f).unwrap();
            let inner = function(&ss[ss.len() - 2]).unwrap();
            let (input, exec) = operator(m, inner, ss.last().unwrap())
                .expect("constructor including assignment callable copies");
            operator_constructor_closed(m, inner)
                .expect("all constructor object uses remain closed");
            subscription(m, exec, input).expect("actual split locals, setup and cleanup");
            let roles = if name == LAST { last(m, f) } else { scan(m, f) }
                .expect("complete actual own parent, never origin-only");
            assert_eq!(roles.len(), 2);
            println!("actual_parent {} {:?}", name, roles);
        });
    }
    production(SOURCE, true);
}
fn production(source: &str, expected: bool) {
    let result = crate::naming::name_threads(source, true).unwrap();
    for (name, role, evaluator) in [
        (
            LAST,
            "createLastOperator",
            "shared_observable_last_operator",
        ),
        (
            "$THREAD_1_29141",
            "lastOperatorFunction",
            "shared_observable_last_operator",
        ),
        (
            SCAN,
            "createScanOperator",
            "shared_observable_scan_operator",
        ),
        (
            "$THREAD_6_113689",
            "scanOperatorFunction",
            "shared_observable_scan_operator",
        ),
    ] {
        let rows: Vec<_> = result
            .report
            .outcomes
            .iter()
            .filter(|o| o.binding.name == name)
            .collect();
        assert_eq!(rows.len(), 1);
        let o = rows[0];
        assert_eq!(o.applied, expected, "{o:?}");
        if expected {
            assert_eq!(o.role_name.as_deref(), Some(role));
            assert_eq!(o.evaluator.as_deref(), Some(evaluator));
            assert_eq!(o.reference_edits, 1);
        }
    }
}

fn changed(source: &str, name: &str, from: &str, to: &str) -> String {
    let mut out = None;
    inspect(source, name, |_, f| {
        let text = &source[f.span.start as usize..f.span.end as usize];
        assert!(
            text.contains(from),
            "nonvacuous own mutation: {name} {from}"
        );
        let mut s = source.to_string();
        s.replace_range(
            f.span.start as usize..f.span.end as usize,
            &text.replacen(from, to, 1),
        );
        out = Some(s);
    });
    out.unwrap()
}
fn global(from: &str, to: &str) -> String {
    assert_eq!(SOURCE.matches(from).count(), 1, "unique mutation {from}");
    SOURCE.replacen(from, to, 1)
}
fn proved(source: &str, name: &str, expected: bool) {
    inspect(source, name, |m, f| {
        assert_eq!(
            if name == LAST { last(m, f) } else { scan(m, f) }.is_some(),
            expected,
            "full parent {name}"
        );
    });
}
#[test]
fn alpha_and_equivalent_capture_alias_and_split_layouts() {
    let alpha = SOURCE
        .replace("$KEY_306", "ownStreamConstructor")
        .replace("$KEY_250", "lastIdentity")
        .replace("$KEY_178", "scanIdentity");
    for name in [LAST, SCAN] {
        proved(&alpha, name, true);
    }
    production(&alpha, true);
    let last = changed(
        SOURCE,
        LAST,
        "let $KEY_5;\n\t\t\t\t\t\t\tlet $KEY_3 = $KEY_250;",
        "let $KEY_3 = $KEY_250; let $KEY_5;",
    );
    proved(&last, LAST, true);
    let scan = changed(SOURCE, SCAN, "let $KEY_4, $KEY_5;", "let $KEY_5, $KEY_4;");
    proved(&scan, SCAN, true);
    for (from, to, name) in [
        (
            "$KEY_344 = $THREAD_5_3651;",
            "const functionCopy = $THREAD_5_3651; $KEY_344 = functionCopy;",
            LAST,
        ),
        (
            "$KEY_208 = $THREAD_0_70818;",
            "const functionCopy = $THREAD_0_70818; $KEY_208 = functionCopy;",
            SCAN,
        ),
    ] {
        proved(&global(from, to), name, true);
    }
}
#[test]
fn last_complete_operational_mutations_refuse() {
    for (from, to) in [
        ("$KEY_0($AH)", "$KEY_1($AH)"),
        ("$KEY_3 === $KEY_250", "$KEY_3 !== $KEY_250"),
        ("$KEY_1 === void 0", "$KEY_0 === void 0"),
        ("No elements in sequence", "No elements remain"),
        ("$KEY_4.next($KEY_1);", "$KEY_4.next($KEY_3);"),
        ("$KEY_3 = $AH;", "$KEY_3 = $KEY_1;"),
        ("$KEY_4.error(e);", "$KEY_4.error($AH);"),
        ("$KEY_4.error($AH);", "other.error($AH);"),
        (
            "$KEY_4.next($KEY_1);",
            "$KEY_4.complete(); $KEY_4.next($KEY_1);",
        ),
        (
            "return $KEY_5.unsubscribe();",
            "return other.unsubscribe();",
        ),
        (
            "return $KEY_5.unsubscribe();",
            "extraEffect(); return $KEY_5.unsubscribe();",
        ),
        ("$KEY_5 = $KEY_2.subscribe(", "$KEY_5 = other.subscribe("),
        ("let $KEY_3 = $KEY_250;", "let $KEY_3 = other;"),
    ] {
        proved(&changed(SOURCE, LAST, from, to), LAST, false);
    }
}
#[test]
fn scan_complete_operational_mutations_refuse() {
    for (from, to) in [
        ("$KEY_0 = $KEY_178;", "$KEY_0 = other;"),
        ("if ($AE === void 0)", "if ($AD === void 0)"),
        ("$KEY_4 === $KEY_178", "$KEY_4 !== $KEY_178"),
        ("$KEY_4 = $AH;", "$KEY_4 = $KEY_0;"),
        ("$KEY_1($KEY_4, $AH)", "$KEY_1($AH, $KEY_4)"),
        ("$KEY_3.next($KEY_4);", "other.next($KEY_4);"),
        ("$KEY_3.error(e);", "$KEY_3.error($AH);"),
        ("$AH => $KEY_3.error($AH)", "$AH => $KEY_3.error($KEY_0)"),
        ("() => $KEY_3.complete()", "() => other.complete()"),
        (
            "return $KEY_5.unsubscribe();",
            "return other.unsubscribe();",
        ),
        ("$KEY_4 = $KEY_0;", "$KEY_4 = $KEY_0; extraEffect();"),
        (
            "$KEY_4 = $KEY_0;",
            "$KEY_5 = $KEY_2.subscribe($AH => $KEY_3.next($AH)); $KEY_4 = $KEY_0;",
        ),
    ] {
        proved(&changed(SOURCE, SCAN, from, to), SCAN, false);
    }
}
#[test]
fn constructor_sentinel_and_alias_lifetimes_refuse() {
    const STORE: &str = "$KEY_306 = $THREAD_7_101596();";
    for source in [
       global(STORE,"$KEY_306 = foreign();"),
       global(STORE,"$KEY_306 = $THREAD_7_101596(); $KEY_306 = foreign();"),
       global(STORE,"if (route) { $KEY_306 = $THREAD_7_101596(); }"),
       global(STORE,"function initializeLater(){ $KEY_306 = $THREAD_7_101596(); } initializeLater();"),
       global(STORE,"const earlyConstructorCopy=$KEY_306; $KEY_306 = $THREAD_7_101596();"),
       global(STORE,"$KEY_306 = $THREAD_7_101596(); leak($KEY_306);"),
       global(STORE,"$KEY_306 = $THREAD_7_101596(); $KEY_306.from=foreign;"),
       global(STORE,"$KEY_306 = $THREAD_7_101596(); const alias=$KEY_306; alias.prototype.subscribe=foreign;"),
       global(STORE,"$THREAD_5_3651(); $THREAD_0_70818(); $KEY_306 = $THREAD_7_101596();"),
       global(STORE,"$KEY_306 = $THREAD_7_101596(); $THREAD_7_101596();"),
       changed(SOURCE,"$THREAD_7_101596","static empty()","static missingEmpty()"),
    ] {for name in [LAST,SCAN] {proved(&source,name,false);}}
    for (name, alias, sent) in [
        (LAST, "$KEY_344", "$KEY_250"),
        (SCAN, "$KEY_208", "$KEY_178"),
    ] {
        let store = format!("{alias} = {name};");
        for replacement in [
            format!("{alias}(); {store}"),
            format!("const earlyCopy={alias}; {store}"),
            format!("if (route) {{ {store} }}"),
            format!("function setLater() {{ {store} }} setLater();"),
            format!("{store} {alias}=foreign;"),
            format!("leak({name}); {store}"),
            format!("{store} {alias}();"),
        ] {
            proved(&global(&store, &replacement), name, false);
        }
        let s = format!("{sent} = {{}};");
        for replacement in [
            format!("if (route) {{ {s} }}"),
            format!("{sent}=other;"),
            format!("{sent}={{property:1}};"),
            format!("{s} {sent}=other;"),
            format!("{alias}(); {s}"),
        ] {
            proved(&global(&s, &replacement), name, false);
        }
    }
}
#[test]
fn actual_readiness_gate_and_production_refusal() {
    for name in [LAST, SCAN] {
        inspect(SOURCE, name, |m, f| {
            let ss = body(f).unwrap();
            let inner = function(&ss[ss.len() - 2]).unwrap();
            let bs = body(inner).unwrap();
            let Expression::NewExpression(n) = ret(&bs[1]).unwrap().without_parentheses() else {
                panic!()
            };
            readiness::constructor(m, &n.callee, fs(f).unwrap(), 8)
                .expect("actual production readiness adapter");
        });
    }
    production(
        &global("$KEY_306 = $THREAD_7_101596();", "$KEY_306 = foreign();"),
        false,
    );
}

#[test]
fn scan_last_preserves_exact_role_and_cleanup_metadata() {
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/scan-last-parent-outcomes/parent-outcomes.json"
    ))
    .unwrap();
    let result = crate::naming::name_threads(
        include_str!("../../../../../tests/fixtures/scan_last_capture_order/captured-order-parent.js"),
        true,
    )
    .unwrap();
    for old in expected.as_array().unwrap() {
        let id = old["function_id"].as_str().unwrap();
        let o = result
            .report
            .outcomes
            .iter()
            .find(|o| o.function_id == id)
            .unwrap();
        assert_eq!(
            serde_json::to_value(o).unwrap(),
            *old,
            "captured scan entire outcome metadata"
        );
    }
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/naming/subscription_operators/parent-outcomes.json"
    ))
    .unwrap();
    for (key, source) in [
        (
            "subscription-parent-alternate",
            include_str!("../../../../../tests/fixtures/naming/subscription_operators/subscription-parent-alternate.js"),
        ),
        (
            "subscription-parent-baseline",
            include_str!("../../../../../tests/fixtures/naming/subscription_operators/subscription-parent-baseline.js"),
        ),
    ] {
        let old = &expected[key];
        // The stored legacy fixture keys are validated instead of accepting a null entry.
        assert!(old.is_object(), "legacy snapshot key {key}");
        let result = crate::naming::name_threads(source, true).unwrap();
        let o = result
            .report
            .outcomes
            .iter()
            .find(|o| o.function_id == old["function_id"].as_str().unwrap())
            .unwrap();
        assert_eq!(serde_json::to_value(o).unwrap(), *old);
    }
}
