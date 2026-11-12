use super::*;
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/program/observable-setup-order/constructor-epoch-parent.js"
));
const FACTORIES: [&str; 3] = ["$THREAD_5_45035", "$THREAD_5_45035_2", "$THREAD_5_45035_3"];
const SPANS: [(u32, u32, u32, u32); 3] = [
    (149641, 154847, 149678, 154777),
    (582097, 586963, 582134, 586899),
    (993607, 998302, 993643, 998241),
];
fn inspect(source: &str, check: impl FnOnce(&Model<'_, '_>, [FunctionSite; 3])) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = crate::analysis::parse(&allocator, source);
    assert!(
        parsed.errors.is_empty(),
        "parse errors: {:?}",
        parsed.errors
    );
    let built = oxc_semantic::SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    assert!(
        built.errors.is_empty(),
        "scoping errors: {:?}",
        built.errors
    );
    let model = Model::new(
        source,
        &parsed.program,
        built.semantic.scoping(),
        built.semantic.nodes(),
        ProgramPropertiesLimits::default(),
    )
    .unwrap();
    let sites=FACTORIES.map(|name| {
        let candidates:Vec<_>=model.all_sites().iter().copied().filter(|site|matches!(model.nodes.kind(site.node_id),AstKind::Function(f) if f.id.as_ref().is_some_and(|id|id.name==name))).collect();
        assert_eq!(candidates.len(),1,"nonvacuous unique factory selection {name}");candidates[0]
    });
    assert!(distinct(&sites.map(|s| s.binding_symbol.unwrap())));
    check(&model, sites);
}
#[test]
fn actual_three_source_classes_close_the_first_subscribe_capture_guard() {
    inspect(SOURCE, |m, sites| {
        let mut symbols = Vec::new();
        for (site, (start, end, cs, ce)) in sites.into_iter().zip(SPANS) {
            assert_eq!(site.span, Span::new(start, end));
            let AstKind::Function(f) = m.nodes.kind(site.node_id) else {
                panic!()
            };
            let [Statement::ClassDeclaration(class), alias, _] =
                body_of(f).unwrap().statements.as_slice()
            else {
                panic!()
            };
            assert_eq!(class.span, Span::new(cs, ce));
            let api = class_api(class).unwrap();
            let class_symbol = class.id.as_ref().unwrap().symbol_id.get().unwrap();
            let alias = local(m, one_variable(alias).unwrap()).unwrap();
            symbols.push(class_symbol);
            assert!(constructor_method(m, api.constructor).is_some());
            assert!(of_method(m, api.of, class_symbol, alias).is_some());
            assert!(empty_method(m, api.empty, class_symbol, alias).is_some());
            assert!(from_method(m, api.from, class_symbol, alias, true, true).is_some());
            assert!(pipe_method(m, api.pipe).is_some());
            let inputs =
                subscribe_inputs(m, site, api.subscribe).expect("own subscribe input identities");
            for input in &inputs[..2] {
                assert_eq!(m.binding_writes(*input), 3);
                assert!(!immutable(m, *input));
                let AstKind::VariableDeclarator(v) =
                    m.nodes.kind(m.scoping.symbol_declaration(*input))
                else {
                    panic!()
                };
                assert!(v.init.is_none());
            }
            assert!(
                subscribe_method(m, site, api.subscribe, class_symbol, alias, true).is_none(),
                "legacy immutable captured_callable must still refuse"
            );
            assert!(
                source_subscribe_method(m, site, api.subscribe, class_symbol, alias, true)
                    .is_some()
            );
            let witness = body_proof(m, site)
                .expect("complete own class and every captured operator/teardown body");
            assert_eq!(witness.class_span, Span::new(cs, ce));
            assert_eq!(
                proven_native_class(m, site),
                Some((Span::new(cs, ce), class_symbol))
            );
            eprintln!("CLOSED factory {}..{} class {}..{}: legacy subscribe rejects the three-store constructor capture; full class/API, input producer/copy/writes, result group and all maker publications now pass",start,end,cs,ce);
        }
        assert!(
            distinct(&symbols),
            "same class spelling has three distinct scoped identities"
        );
    });
}
fn own_change(index: usize, from: &str, to: &str) -> String {
    let (start, end, _, _) = SPANS[index];
    let start = start as usize;
    let end = end as usize;
    let body = &SOURCE[start..end];
    assert!(body.contains(from), "missing own mutation {from}");
    format!(
        "{}{}{}",
        &SOURCE[..start],
        body.replace(from, to),
        &SOURCE[end..]
    )
}
fn denied(source: &str, label: &str) {
    denied_at(source, label, 0);
}
fn denied_at(source: &str, label: &str, index: usize) {
    inspect(source, |m, sites| {
        assert!(
            proven_native_class(m, sites[index]).is_none(),
            "accepted {label} at {:?}",
            sites[index].span
        );
    });
}
#[test]
fn whole_class_alpha_and_legitimate_method_order() {
    let alpha = SOURCE
        .replace("$KEY_420", "projectOperatorCapture")
        .replace("$KEY_295", "flattenOperatorCapture")
        .replace("$DF", "sourceArgument")
        .replace("$DK", "methodTuple")
        .replace("$THREAD_6_90030", "LexicalStreamClass");
    inspect(&alpha, |m, sites| {
        for site in sites {
            assert!(proven_native_class(m, site).is_some());
        }
    });
    // These class members are independent declarations; no execution order is
    // inferred from property names or textual proximity of outer stores.
    let mut ranges = Vec::new();
    inspect(SOURCE, |m, sites| {
        for site in sites {
            let AstKind::Function(f) = m.nodes.kind(site.node_id) else {
                panic!()
            };
            let Statement::ClassDeclaration(c) = &body_of(f).unwrap().statements[0] else {
                panic!()
            };
            let mut members = c.body.body.iter().map(|e| e.span()).collect::<Vec<_>>();
            members.reverse();
            ranges.push((
                c.body.span.start as usize + 1,
                c.body.span.end as usize - 1,
                members,
            ));
        }
    });
    let mut reordered = SOURCE.to_string();
    for (start, end, members) in ranges.into_iter().rev() {
        let replacement = members
            .into_iter()
            .map(|s| &SOURCE[s.start as usize..s.end as usize])
            .collect::<Vec<_>>()
            .join("\n");
        reordered.replace_range(start..end, &replacement);
    }
    inspect(&reordered, |m, sites| {
        for site in sites {
            assert!(proven_native_class(m, site).is_some());
        }
    });
}
#[test]
fn class_tuple_api_and_closure_refusals() {
    for (from, to) in [
        (
            "$DK[1].call($DK[0], $DI, $DJ)",
            "$DK[1].call($DK[0], $DJ, $DI)",
        ),
        (
            "$DK[1].call($DK[0], $DI, $DJ)",
            "$DK[1].call($DK[1], $DI, $DJ)",
        ),
        ("$DI.start.call", "$DI.error.call"),
        ("$DK[1].call", "$DK[0].call"),
        ("$DL[1].call($DL[0], e)", "$DL[1].call($DL[0], $DF)"),
        ("$DL = [$DL, $DL.error]", "$DL = [$DL, $DL.next]"),
        ("$DL[1].call($DL[0], e)", "$DL[1].call($DL[1], e)"),
        ("this._subscriber = $DF;", "this._subscriber = $DG;"),
        ("$KEY_0.complete();", "$KEY_0.next();"),
        ("return $DJ;", "sideEffect(); return $DJ;"),
        ("pipe() {", "unknownMethod() {"),
        ("static of() {", "@replaceMethod static of() {"),
        (
            "class $THREAD_6_90030 {",
            "class $THREAD_6_90030 extends UnknownSuperclass {",
        ),
        ("this.pipe($KEY_420($DF))", "this.pipe($KEY_295($DF))"),
        ("$DM();", "$DM;"),
        ("$DJ._cleanup = void 0;", "$DJ._cleanup = $DM;"),
    ] {
        for index in 0..3 {
            denied_at(&own_change(index, from, to), from, index);
        }
    }
    // Swap both operator captures rather than merely duplicating a slot.
    let swapped = own_change(0, "$KEY_420", "swapSlot")
        .replace("this.pipe($KEY_295($DF))", "this.pipe($KEY_420($DF))")
        .replace("swapSlot", "$KEY_295");
    denied(&swapped, "swapped map/flat-map captures");
}
#[test]
fn all_input_writers_copies_and_class_publications_are_checked() {
    let cases = [
        (
            "$KEY_226 = $THREAD_6_19967;",
            "if (flag) { $KEY_226 = $THREAD_6_19967; }",
        ),
        ("$KEY_226 = $THREAD_6_19967;", "$KEY_226 = other;"),
        ("$KEY_202 = $THREAD_0_137272;", "$KEY_202 = $KEY_226;"),
        (
            "let $KEY_420 = $THREAD_2_113070;",
            "let $KEY_420 = laterProjection;",
        ),
        (
            "let $KEY_295 = $THREAD_7_6252;",
            "let $KEY_295 = $THREAD_7_6252; $KEY_295 = other;",
        ),
        (
            "$KEY_225 = $THREAD_5_45035();",
            "$KEY_225 = $THREAD_5_45035().of(1);",
        ),
        (
            "$KEY_225 = $THREAD_5_45035();",
            "publish($THREAD_5_45035());",
        ),
        (
            "$KEY_225 = $THREAD_5_45035();",
            "$THREAD_5_45035(); $KEY_225 = $THREAD_5_45035();",
        ),
        (
            "$KEY_225 = $THREAD_5_45035();",
            "$KEY_225 = $THREAD_5_45035(); $KEY_226 = other;",
        ),
        ("$KEY_225 = $THREAD_5_45035_3();", "$KEY_225 = other();"),
        (
            "$KEY_225 = $THREAD_5_45035_3();",
            "$KEY_225 = $THREAD_5_45035_3(); mutate($KEY_225);",
        ),
        (
            "$KEY_225 = $THREAD_5_45035_3();",
            "$KEY_225 = $THREAD_5_45035_3(); $KEY_225.prototype.map = other;",
        ),
    ];
    for (from, to) in cases {
        assert_eq!(
            SOURCE.matches(from).count(),
            1,
            "nonvacuous global mutation {from}"
        );
        denied(&SOURCE.replacen(from, to, 1), from);
    }
    let early = SOURCE.replacen(
        "$KEY_226 = $THREAD_6_19967;",
        "$THREAD_5_45035(); $KEY_226 = $THREAD_6_19967;",
        1,
    );
    inspect(&early, |m, sites| {
        assert_eq!(
            m.binding_references(sites[0].binding_symbol.unwrap()),
            2,
            "the added early call must resolve to the actual same factory"
        );
        assert!(proven_native_class(m, sites[0]).is_none());
    });
    let mut late = SOURCE.to_string();
    inspect(SOURCE, |m, sites| {
        let capture = body_proof(m, sites[0]).unwrap().inputs[3];
        let declaration = m.scoping.symbol_declaration(capture);
        let statement = m.nodes.kind(m.nodes.parent_id(declaration)).span();
        let owner = body_owner(m, declaration).unwrap();
        assert!(matches!(
            m.nodes.kind(owner),
            AstKind::FunctionBody(_) | AstKind::BlockStatement(_)
        ));
        let end = m.nodes.kind(owner).span().end as usize - 1;
        // The declaration stays in exactly its original lexical scope. Its
        // binding is now uninitialized at each earlier class publication.
        late.insert_str(
            end,
            &format!(
                "\n{}\n",
                &SOURCE[statement.start as usize..statement.end as usize]
            ),
        );
        late.replace_range(statement.start as usize..statement.end as usize, "");
    });
    inspect(&late, |m, sites| {
        let capture = body_proof(m, sites[0]).unwrap().inputs[3];
        let origin = input_origin(m, capture, 12).unwrap();
        for site in sites {
            let publication = m
                .scoping
                .get_resolved_references(site.binding_symbol.unwrap())
                .next()
                .unwrap()
                .node_id();
            assert!(
                !available(m, &origin, publication),
                "actual late capture readiness must refuse"
            );
            assert!(proven_native_class(m, site).is_none());
        }
    });
    // Moving a complete independent producer earlier is legitimate. The new
    // store still executes in its original activation and lexical scope.
    let (from, to) = (
        "let $KEY_420 = $THREAD_2_113070;\n\t\t\t\tlet $KEY_295 = $THREAD_7_6252;",
        "let $KEY_295 = $THREAD_7_6252;\n\t\t\t\tlet $KEY_420 = $THREAD_2_113070;",
    );
    assert_eq!(SOURCE.matches(from).count(), 1);
    inspect(&SOURCE.replacen(from, to, 1), |m, sites| {
        for site in sites {
            assert!(proven_native_class(m, site).is_some());
        }
    });
}
