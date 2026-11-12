use shape::naming::name_threads;
const NAME: &str = "deleteConcatenatedCapturedKeysAndCallUnregisterMembers";
const EVALUATOR: &str = "shared_captured_key_list_unregister_operation";
const ACTUAL: &str = include_str!("fixtures/operation-captured-key-list-unregister/stateful-parent-expanded.js");
const STATEFUL_PARENT: &str =
    include_str!("fixtures/operation-captured-key-list-unregister/stateful-whole-parent.js");
const WORK_BATCH_PARENT: &str =
    include_str!("fixtures/operation-captured-key-list-unregister/canonical-whole-parent.js");

fn check(source: &str, target: &str, positive: bool) {
    let r = name_threads(source, true).expect("valid scoped source");
    let o = r
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == target)
        .expect("target exists");
    if positive {
        assert!(o.applied, "{o:?}");
        assert_eq!(o.role_name.as_deref(), Some(NAME));
        assert_eq!(o.evaluator.as_deref(), Some(EVALUATOR));
        for claim in [
            "opaque dynamic external",
            "getter behavior",
            "external call success",
            "native XHR purpose are unproved",
        ] {
            assert!(o.explanation.iter().any(|e| e.contains(claim)), "{o:?}");
        }
    } else {
        assert_ne!(o.role_name.as_deref(), Some(NAME), "{o:?}");
        assert_ne!(o.evaluator.as_deref(), Some(EVALUATOR), "{o:?}");
    }
    let mut replay = source.to_owned();
    let mut edits = r.report.edits.clone();
    edits.sort_by_key(|e| std::cmp::Reverse(e.start));
    for e in edits {
        replay.replace_range(e.start as usize..e.end as usize, &e.replacement);
    }
    assert_eq!(replay, r.source);
    fn members(source: &str) -> std::collections::BTreeMap<String, usize> {
        use oxc_ast_visit::{walk, Visit};
        struct Members(std::collections::BTreeMap<String, usize>);
        impl<'a> Visit<'a> for Members {
            fn visit_static_member_expression(
                &mut self,
                m: &oxc_ast::ast::StaticMemberExpression<'a>,
            ) {
                *self.0.entry(m.property.name.to_string()).or_default() += 1;
                walk::walk_static_member_expression(self, m);
            }
        }
        let allocator = oxc_allocator::Allocator::default();
        let parsed = shape::analysis::parse(&allocator, source);
        assert!(parsed.errors.is_empty());
        let mut m = Members(Default::default());
        m.visit_program(&parsed.program);
        m.0
    }
    assert_eq!(members(source), members(&r.source));
}
fn swap(s: &str, a: &str, b: &str) -> String {
    s.replace(a, "PORT_SWAP_TMP")
        .replace(b, a)
        .replace("PORT_SWAP_TMP", b)
}
#[test]
fn whole_original_and_alpha_sources() {
    check(STATEFUL_PARENT, "$THREAD_3_86302", true);
    let alpha = STATEFUL_PARENT
        .replace("$KEY_", "capturePortAlpha_")
        .replace("$DW", "localAliasAlpha")
        .replace("$DX", "localListAlpha");
    check(&alpha, "$THREAD_3_86302", true);
    // Existing stronger claims retain precedence. The natively named input also has its actual
    // owned body tested with opaque captures below.
    let r = name_threads(WORK_BATCH_PARENT, true).unwrap();
    let o = r
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.declaration.start == 516842)
        .unwrap();
    assert!(o.applied);
    assert_eq!(
        o.evaluator.as_deref(),
        Some("function_finder::teardown_xhr_instrumentation")
    );
}
#[test]
fn actual_bodies_and_all_global_free_port_permutations() {
    let rows: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/operation-captured-key-list-unregister/provenance.json"
    ))
    .unwrap();
    for row in rows.as_array().unwrap() {
        let source = std::fs::read_to_string(format!(
            "{}/tests/fixtures/operation-captured-key-list-unregister/{}",
            env!("CARGO_MANIFEST_DIR"),
            row["fixture"].as_str().unwrap()
        ))
        .unwrap();
        check(&source, row["name"].as_str().unwrap(), true);
    }
    for (a, b) in [
        ("$KEY_48", "$KEY_243"),
        ("$KEY_17", "$KEY_20"),
        ("$KEY_21", "$KEY_11"),
        ("$KEY_22", "$KEY_8"),
        ("$KEY_48", "$KEY_22"),
    ] {
        check(&swap(ACTUAL, a, b), "$THREAD_3_86302", true);
    }
    let reordered = ACTUAL.replace(
        "$KEY_21,$KEY_11,$KEY_48,$KEY_243,$KEY_22,$KEY_8,$KEY_17,$KEY_20",
        "$KEY_20,$KEY_17,$KEY_8,$KEY_22,$KEY_243,$KEY_48,$KEY_11,$KEY_21",
    );
    assert_ne!(reordered, ACTUAL);
    check(&reordered, "$THREAD_3_86302", true);
    let dynamic=ACTUAL.replace("return $THREAD_3_86302;", "$KEY_22={get onBeforeInvoke(){return changeExternal();}}; $KEY_48=null; function writer(){ $KEY_11=externalValue; $KEY_22=other; } return $THREAD_3_86302;");
    check(&dynamic, "$THREAD_3_86302", true);
    let alpha = ACTUAL
        .replace("$KEY_", "opaque_")
        .replace("$THREAD_3_86302", "$THREAD_7_888")
        .replace("$DW", "aliasAlpha");
    check(&alpha, "$THREAD_7_888", true);
}
#[test]
fn own_relationship_and_operation_refusal_matrix() {
    for (from, to) in [
        ("$KEY_21 == null", "$KEY_21 === null"),
        ("$KEY_21 == null", "$KEY_11 == null"),
        ("let $DW = $KEY_11;", "let $DW = $KEY_21;"),
        ("$KEY_48.concat($KEY_243)", "$KEY_48.concat($KEY_48)"),
        ("$KEY_48.concat($KEY_243)", "$KEY_243.concat($KEY_243)"),
        (
            "$KEY_48.concat($KEY_243)",
            "$KEY_48.concat($KEY_243, $KEY_17)",
        ),
        ("$KEY_48.concat($KEY_243)", "$KEY_48.concat(...$KEY_243)"),
        ("$KEY_48.concat($KEY_243)", "$KEY_48?.concat($KEY_243)"),
        ("$KEY_48.concat($KEY_243)", "$KEY_48.concat?.($KEY_243)"),
        ("$KEY_48.concat($KEY_243)", "$KEY_48.slice($KEY_243)"),
        ("$KEY_48.concat($KEY_243)", "[$KEY_48,$KEY_243]"),
        ("$KEY_48.concat($KEY_243)", "$DW.concat($KEY_243)"),
        ("let i = 0;", "let i = 1;"),
        ("i < $DX.length", "i <= $DX.length"),
        ("i < $DX.length", "i < $KEY_48.length"),
        ("i++)", "++i)"),
        ("delete $DW[$DX[i]];", "delete $KEY_11[$DX[i]];"),
        ("delete $DW[$DX[i]];", "delete $DW[i];"),
        ("delete $DW[$DX[i]];", "delete $DW[$DX[i]]; $DW = $KEY_11;"),
        ("$DY = $KEY_22 == null;", "$DY = $KEY_8 == null;"),
        (
            "$DZ = $KEY_22.onBeforeInvoke;",
            "$DZ = $KEY_8.onBeforeInvoke;",
        ),
        (
            "$DZ = $KEY_22.onBeforeInvoke;",
            "$DZ = $KEY_22?.onBeforeInvoke;",
        ),
        ("$DZ = void 0;", "$DZ = null;"),
        (
            "$DY = $DZ == null;",
            "$DY = $KEY_22.onBeforeInvoke == null;",
        ),
        ("if (!$DY)", "if ($DY)"),
        ("$DZ.unregister($KEY_17);", "$DZ.register($KEY_17);"),
        ("$DZ.unregister($KEY_17);", "$DZ.unregister($KEY_20);"),
        ("$DZ.unregister($KEY_17);", "$EB.unregister($KEY_17);"),
        (
            "$DZ.unregister($KEY_17);",
            "$KEY_22.onBeforeInvoke.unregister($KEY_17);",
        ),
        ("$DZ.unregister($KEY_17);", "$DZ.unregister?.($KEY_17);"),
        ("$DZ.unregister($KEY_17);", "$DZ.unregister();"),
        (
            "$DZ.unregister($KEY_17);",
            "$DZ.unregister($KEY_17,$KEY_20);",
        ),
        ("$EB.unregister($KEY_20);", "$EB.unregister($KEY_17);"),
        ("$EA = $EB == null;", "$DY = $EB == null;"),
        ("return void 0;", "return 0;"),
        ("return void 0;", "return;"),
        (
            "$DZ.unregister($KEY_17);",
            "$DZ.unregister($KEY_17); extra();",
        ),
        (
            "function $THREAD_3_86302()",
            "async function $THREAD_3_86302()",
        ),
        ("function $THREAD_3_86302()", "function* $THREAD_3_86302()"),
        (
            "function $THREAD_3_86302()",
            "function $THREAD_3_86302(arg)",
        ),
        (
            "function $THREAD_3_86302() {",
            "function $THREAD_3_86302() { 'use strict';",
        ),
        ("let $DY, $DZ;", "let $DY, $DZ; let extra;"),
    ] {
        assert!(ACTUAL.contains(from), "missing mutation {from}");
        check(&ACTUAL.replace(from, to), "$THREAD_3_86302", false);
    }
    // Partial G swap leaves the alias link broken; global swap above accepts.
    let partial = ACTUAL
        .replace("if ($KEY_21 == null)", "if (PARTIAL_TMP == null)")
        .replace("if ($KEY_11 == null)", "if ($KEY_21 == null)")
        .replace("PARTIAL_TMP", "$KEY_11");
    check(&partial, "$THREAD_3_86302", false);
    for (a, b) in [
        ("$KEY_21", "$KEY_11"),
        ("$KEY_48", "$KEY_243"),
        ("$KEY_22", "$KEY_8"),
        ("$KEY_17", "$KEY_20"),
        ("$KEY_48", "$KEY_17"),
    ] {
        // Merge only own uses; keep declarations legal and distinct.
        let (prefix, own) = ACTUAL.split_once("function $THREAD_3_86302").unwrap();
        check(
            &format!("{prefix}function $THREAD_3_86302{}", own.replace(a, b)),
            "$THREAD_3_86302",
            false,
        );
    }
}
#[test]
fn nonlexical_capture_origins_refuse() {
    let declaration = "let $KEY_21,$KEY_11,$KEY_48,$KEY_243,$KEY_22,$KEY_8,$KEY_17,$KEY_20;";
    assert!(ACTUAL.contains(declaration));
    check(&ACTUAL.replace(declaration, ""), "$THREAD_3_86302", false);
    let p = ACTUAL
        .replace(
            declaration,
            "let $KEY_11,$KEY_48,$KEY_243,$KEY_22,$KEY_8,$KEY_17,$KEY_20;",
        )
        .replace("function main()", "function main($KEY_21)");
    check(&p, "$THREAD_3_86302", false);
    let destructured = ACTUAL.replace(
        declaration,
        "let [$KEY_21]=external;let $KEY_11,$KEY_48,$KEY_243,$KEY_22,$KEY_8,$KEY_17,$KEY_20;",
    );
    check(&destructured, "$THREAD_3_86302", false);
    // The enumerated function expression is exercised non-vacuously by the module-local
    // exact-site matcher test.
}

#[test]
fn actual_oxc_symbol_ids_and_local_write_inventory() {
    use oxc_span::GetSpan;
    let start = 518627;
    let end = 519596;
    let allocator = oxc_allocator::Allocator::default();
    let parsed = shape::analysis::parse(&allocator, STATEFUL_PARENT);
    assert!(parsed.errors.is_empty());
    let built = oxc_semantic::SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    assert!(built.errors.is_empty());
    let scope = built.semantic.scoping();
    let mut roles = Vec::new();
    for (role, name, writes) in [
        ("G1", "$KEY_21", None),
        ("G2", "$KEY_11", None),
        ("K1", "$KEY_48", None),
        ("K2", "$KEY_243", None),
        ("H1", "$KEY_22", None),
        ("H2", "$KEY_8", None),
        ("C1", "$KEY_17", None),
        ("C2", "$KEY_20", None),
        ("B", "$DW", Some(0)),
        ("L", "$DX", Some(0)),
        ("C", "i", Some(1)),
        ("T1", "$DY", Some(2)),
        ("V1", "$DZ", Some(2)),
        ("T2", "$EA", Some(2)),
        ("V2", "$EB", Some(2)),
    ] {
        let ids: Vec<_> = scope
            .symbol_ids()
            .filter(|s| {
                scope.symbol_name(*s) == name
                    && scope.get_resolved_references(*s).any(|r| {
                        let span = built.semantic.nodes().kind(r.node_id()).span();
                        span.start >= start && span.end <= end
                    })
            })
            .collect();
        assert_eq!(ids.len(), 1, "{role}");
        let id = ids[0];
        let span = scope.symbol_span(id);
        let actual_writes = scope
            .get_resolved_references(id)
            .filter(|r| r.is_write())
            .count();
        if let Some(w) = writes {
            assert!(span.start >= start && span.end <= end);
            assert_eq!(actual_writes, w, "{role}");
        } else {
            assert!(span.start < start || span.end > end);
        }
        roles.push(serde_json::json!({"role":role,"symbol_id":format!("{id:?}"),"name":name,"declaration":{"start":span.start,"end":span.end},"writes":actual_writes}));
    }
    let distinct: std::collections::BTreeSet<_> =
        roles.iter().map(|v| v["symbol_id"].to_string()).collect();
    assert_eq!(distinct.len(), 15);
    println!(
        "ACTUAL_OXC_ROLE_INVENTORY {}",
        serde_json::to_string(&roles).unwrap()
    );
}
