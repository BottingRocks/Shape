use super::*;

const ACTUAL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/program/submit-cleanup/direct-submit-parent.js"
));

fn inspect(source: &str, inspect: impl FnOnce(&Model<'_, '_>)) {
    let allocator = oxc_allocator::Allocator::default();
    let program = crate::analysis::parse_program(&allocator, source).unwrap();
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let model = Model::new(
        source,
        &program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .unwrap();
    inspect(&model);
}

#[test]
fn actual_first_refusal_and_capture_epoch_gaps() {
    inspect(ACTUAL, |model| {
        // Original byte identity is a diagnostic locator only. No evaluator
        // gains a span/name/vendor shortcut from this test.
        let site = model
            .all_sites()
            .iter()
            .find(|site| site.span.start == 525911 && site.span.end == 526359)
            .copied()
            .unwrap();
        let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
            panic!("expected the saved declaration");
        };
        let body = function.body.as_ref().unwrap();
        assert!(directives(body));
        assert!(plain_params(model, &function.params));
        assert!(function.params.items.is_empty());
        let [Statement::TryStatement(t)] = body.statements.as_slice() else {
            panic!("expected one try");
        };
        assert!(t.finalizer.is_none());
        let handler = t.handler.as_ref().unwrap();
        let BindingPattern::BindingIdentifier(error) = &handler.param.as_ref().unwrap().pattern
        else {
            panic!("expected plain catch parameter");
        };
        assert!(handler.body.body.is_empty());
        assert!(unchanged(model, error.symbol_id.get().unwrap()));
        let [
            click,
            Statement::IfStatement(conditional),
            Statement::IfStatement(head),
        ] = t.block.body.as_slice()
        else {
            panic!("expected the exact teardown statements");
        };
        assert!(conditional.alternate.is_none() && head.alternate.is_none());
        let click = statement_call(click).unwrap();
        let args = arity(click, 2).unwrap();
        let receiver = data_capture(
            model,
            site,
            member(&click.callee, "removeEventListener").unwrap(),
        )
        .unwrap();
        assert!(string(args[0], "click"));
        assert!(captured_handler(model, site, args[1]));
        let flag = truth_test(model, &conditional.test).unwrap();
        assert!(capture_identity(
            model,
            site,
            flag,
            conditional.test.span().start
        ));
        let Statement::BlockStatement(cleanup) = &conditional.consequent else {
            panic!("expected cleanup block");
        };
        let (reset, clears) = cleanup.body.split_last().unwrap();
        assert_eq!(clears.len(), 4);
        let mut registries = BTreeSet::new();
        for statement in &clears[..3] {
            let call = statement_call(statement).unwrap();
            assert!(arity(call, 0).is_some());
            let registry =
                data_capture(model, site, member(&call.callee, "clear").unwrap()).unwrap();
            assert!(registries.insert(registry));
            // Identity succeeds, but the last textual assignment belongs to
            // another later setup epoch. It is not a reaching definition.
            assert_eq!(model.binding_writes(registry), 2);
            assert!(model.initializer(registry).is_none());
            assert!(model.symbol_value(registry).unwrap().span().start > site.span.end);
        }
        assert!(matches!(&clears[3], Statement::IfStatement(_)));
        assert!(statement_call(&clears[3]).is_none());
        assert!(listener_removal(model, site, &clears[3], "submit").is_none());
        let reset = assignment(reset).unwrap();
        assert_eq!(assign_symbol(model, reset), Some(flag));
        assert!(matches!(&reset.right, Expression::BooleanLiteral(b) if !b.value));
        assert!(teardown_submit_instrumentation(model, site, body).is_none());

        // Local receiver initialization is later than the hoisted declaration.
        // The one property publication is later still; no direct call edge
        // proves when the published teardown will run relative to setup/reset.
        assert!(model.initializer(receiver).is_none());
        assert_eq!(model.binding_writes(receiver), 1);
        let value = model.symbol_value(receiver).unwrap();
        assert!(value.span().start > site.span.end);
        let refs: Vec<_> = model
            .scoping
            .get_resolved_references(site.binding_symbol.unwrap())
            .collect();
        assert_eq!(refs.len(), 1);
        let AstKind::AssignmentExpression(publication) = model.nodes.parent_kind(refs[0].node_id())
        else {
            panic!("expected property publication");
        };
        assert!(publication.span.start > value.span().end);
        assert!(
            matches!(&publication.left, AssignmentTarget::StaticMemberExpression(m) if m.property.name == "teardown")
        );
        assert!(model.calls_to(site).is_empty());
        println!(
            "actual first refusal: cleanup[3] guarded submit is not statement_call; capture identities pass; three registries have two writes each; last textual values occur after this function; document initialized after declaration and before property publication; direct calls=0"
        );
    });
}

const IDENTITY_ONLY: &str = r#"function main(){
let doc,flag,reg1,reg2,clickHandler,submitHandler,styleNode;
function cleanup(){try{
doc.removeEventListener("click",clickHandler);
if(!!flag){reg1.clear();reg2.clear();
doc.removeEventListener("submit",submitHandler);flag=false;}
if(doc.head){doc.head.removeChild(styleNode);}
}catch(e){}}
return cleanup;
} main();"#;

#[test]
fn identity_only_is_not_initialization_or_publication_proof() {
    for source in [
        IDENTITY_ONLY.to_string(),
        IDENTITY_ONLY
            .replace("doc", "surface")
            .replace("flag", "enabled")
            .replace("cleanup", "dispose"),
        IDENTITY_ONLY.replace("return cleanup;", "cleanup(); return cleanup;"),
        IDENTITY_ONLY.replace(
            "return cleanup;",
            "publish(cleanup); reg1={}; return cleanup;",
        ),
    ] {
        inspect(&source, |model| {
            let site = model
                .all_sites()
                .iter()
                .find(|site| {
                    let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
                        return false;
                    };
                    f.body.as_ref().is_some_and(|b| {
                        matches!(b.statements.first(), Some(Statement::TryStatement(_)))
                    })
                })
                .copied()
                .unwrap();
            let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
                unreachable!()
            };
            // Existing acceptance is the proof-gap counterexample, never a new
            // claim that these captures are ready or native operations.
            assert!(
                teardown_submit_instrumentation(model, site, f.body.as_ref().unwrap()).is_some()
            );
        });
    }
}

// Fixture-only diagnostic locators: these names/spans never enter production
// matching. Enumerate both scoped bindings; finding only the first is vacuous.
fn guarded_candidates(source: &str, diagnostic_name: &str) -> Vec<(oxc_span::Span, bool, String)> {
    let mut result = Vec::new();
    inspect(source, |model| {
        let mut sites: Vec<_> = model
            .all_sites()
            .iter()
            .copied()
            .filter(|s| {
                s.binding_symbol
                    .is_some_and(|s| model.scoping.symbol_name(s) == diagnostic_name)
            })
            .collect();
        sites.sort_by_key(|s| s.span.start);
        assert_eq!(
            sites.len(),
            2,
            "nonempty exact two-candidate fixture selection"
        );
        for site in sites {
            result.push((
                site.span,
                guarded_cleanup::accepts(model, site),
                guarded_cleanup::diagnostic(model, site),
            ));
        }
    });
    result
}
fn guarded(source: &str) -> (bool, String) {
    let (_, accepted, why) = guarded_candidates(source, "$THREAD_4_6761").remove(0);
    (accepted, why)
}
fn parent_change(from: &str, to: &str) -> String {
    let parent = &ACTUAL[517820..526641];
    assert!(parent.contains(from), "mutation must hit {from}");
    format!(
        "{}{}{}",
        &ACTUAL[..517820],
        {
            let at = parent.rfind(from).unwrap();
            format!("{}{}{}", &parent[..at], to, &parent[at + from.len()..])
        },
        &ACTUAL[526641..]
    )
}
#[test]
fn guarded_production_real_witness_and_alpha_setup_forms() {
    for source in [
        ACTUAL.to_string(),
        parent_change(
            "let $KEY_9, $KEY_10, $KEY_11, $KEY_15, $KEY_0;",
            "var $KEY_9; let $KEY_10; var $KEY_11; let $KEY_15, $KEY_0;",
        ),
        ACTUAL
            .replace("$KEY_9", "$cleanupDocument")
            .replace("$KEY_402", "$cleanupEnabled")
            .replace("$KEY_176", "$cleanupRegistry"),
    ] {
        for (_, accepted, why) in guarded_candidates(&source, "$THREAD_4_6761") {
            assert!(accepted, "{why}");
        }
    }
    let alpha = ACTUAL
        .replace("$THREAD_4_6761", "$cleanupDisposer")
        .replace("$KEY_9", "$cleanupSurface");
    for (_, accepted, why) in guarded_candidates(&alpha, "$cleanupDisposer") {
        assert!(accepted, "alpha: {why}");
    }
    let named = crate::naming::name_threads(ACTUAL, true).unwrap();
    // Registry provenance is the resolved allocation, including when a bare
    // declaration is populated by a later assignment. Check the public source
    // independently of the golden report before comparing every report field.
    let descriptors = named.report.collectors.wiring.descriptors.as_ref().unwrap();
    let allocation_span = crate::signals::schema::SourceSpan::new(719698, 722707);
    assert_eq!(descriptors.array_literal_span, Some(allocation_span));
    assert_eq!(descriptors.length, 77);
    inspect(ACTUAL, |model| {
        let allocation = model
            .nodes
            .iter()
            .find(|node| {
                node.span().start == allocation_span.start
                    && node.span().end == allocation_span.end
                    && matches!(node.kind(), AstKind::ArrayExpression(_))
            })
            .expect("descriptor provenance must locate an actual array expression");
        let AstKind::ArrayExpression(array) = allocation.kind() else {
            unreachable!()
        };
        assert_eq!(array.elements.len(), 77);
        let AstKind::AssignmentExpression(assignment) = model.nodes.parent_kind(allocation.id())
        else {
            panic!("descriptor allocation must be the later registry assignment");
        };
        assert_eq!(assignment.right.span(), array.span);
        let registry = assign_symbol(model, assignment).unwrap();
        assert_eq!(model.scoping.symbol_name(registry), "$KEY_136");
        assert_eq!(registry.index() as u32, descriptors.binding.symbol);
        assert!(model.initializer(registry).is_none());
    });
    let mut expected: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/program/submit-cleanup/direct-submit-after.map.json"
    )))
    .unwrap();
    let mut actual = serde_json::to_value(&named.report).unwrap();
    fn remove_timings(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                map.remove("timings_ms");
                for value in map.values_mut() {
                    remove_timings(value);
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    remove_timings(value);
                }
            }
            _ => {}
        }
    }
    remove_timings(&mut expected);
    remove_timings(&mut actual);
    assert!(
        actual == expected,
        "every existing name, evaluator, role, purpose, explanation, evidence, refcount and edit must remain exact"
    );
    let private = guarded_candidates(ACTUAL, "$THREAD_4_6761");
    assert_eq!(
        private
            .iter()
            .map(|r| (r.0.start, r.0.end))
            .collect::<Vec<_>>(),
        vec![(525911, 526359), (782496, 782944)]
    );
    for id in ["fn@525911..526359", "fn@782496..782944"] {
        let rows: Vec<_> = named
            .report
            .outcomes
            .iter()
            .filter(|o| o.function_id == id)
            .collect();
        assert_eq!(rows.len(), 1, "exact candidate cardinality: {id}");
        let target = rows[0];
        assert!(target.applied, "{target:?}");
        assert_eq!(
            target.role_name.as_deref(),
            Some("teardownSubmitInstrumentation")
        );
        assert_eq!(
            target.evaluator.as_deref(),
            Some("browser_submit_instrumentation_teardown")
        );
    }
    assert_eq!(
        named
            .report
            .outcomes
            .iter()
            .filter(|o| o.applied
                && o.evaluator.as_deref() == Some("browser_submit_instrumentation_teardown"))
            .count(),
        2
    );
}
#[test]
fn guarded_refuses_contract_and_publication_mutations() {
    let mut cases = Vec::new();
    for (from, to) in [
        ("if ($KEY_352 != null)", "if ($KEY_2 != null)"),
        (
            "$KEY_182.removeEventListener(\"submit\", $KEY_352)",
            "$KEY_9.removeEventListener(\"submit\", $KEY_352)",
        ),
        (
            "$KEY_182.removeEventListener(\"submit\", $KEY_352)",
            "$KEY_182.removeEventListener(\"submit\", $KEY_2)",
        ),
        (
            "$KEY_182.removeEventListener(\"submit\", $KEY_352)",
            "$KEY_182.removeEventListener(\"click\", $KEY_352)",
        ),
        (
            "if ($KEY_352 != null) {",
            "if ($KEY_352 != null) { extra();",
        ),
        ("$KEY_37.clear();", "$KEY_176.clear();"),
        ("$KEY_402 = false;", "$KEY_402 = true;"),
        ("if ($KEY_9.head)", "if ($KEY_11.head)"),
        (
            "$KEY_9.head.removeChild($KEY_3)",
            "$KEY_11.head.removeChild($KEY_3)",
        ),
        ("catch (e) {}", "catch (e) { extra(); }"),
        (
            "$KEY_11 = $KEY_5.global;",
            "let early = $THREAD_4_6761; $KEY_11 = $KEY_5.global;",
        ),
        (
            "$KEY_11 = $KEY_5.global;",
            "let early = {teardown:$THREAD_4_6761}; $KEY_11 = $KEY_5.global;",
        ),
        (
            "$KEY_3 = $CC[1].call($CC[0], \"style\");",
            "$KEY_3 = $CC[1].call($CC[0], \"style\", $THREAD_4_6761());",
        ),
        (
            "$CD.teardown = $THREAD_4_6761;",
            "$KEY_176 = {}; $CD.teardown = $THREAD_4_6761;",
        ),
        (
            "$CD.teardown = $THREAD_4_6761;",
            "$KEY_9.removeEventListener = replacement; $CD.teardown = $THREAD_4_6761;",
        ),
        (
            "$CD.teardown = $THREAD_4_6761;",
            "$KEY_176.clear = replacement; $CD.teardown = $THREAD_4_6761;",
        ),
        (
            "$CD.teardown = $THREAD_4_6761;",
            "$KEY_352 = replacement; $CD.teardown = $THREAD_4_6761;",
        ),
    ] {
        cases.push((from.to_string(), parent_change(from, to)));
    }
    cases.push((
        "direct eval".into(),
        parent_change(
            "$KEY_11 = $KEY_5.global;",
            r#"eval("$KEY_176 = {}"); $KEY_11 = $KEY_5.global;"#,
        ),
    ));
    cases.push((
        "registry method alias".into(),
        parent_change(
            "$CD.teardown = $THREAD_4_6761;",
            "let borrowed = $KEY_176.register; $CD.teardown = $THREAD_4_6761;",
        ),
    ));
    let guard_start = ACTUAL[517820..526641]
        .find("if ($KEY_352 != null) {")
        .unwrap()
        + 517820;
    let guard_end = ACTUAL[guard_start..].find("$KEY_402 = false;").unwrap() + guard_start;
    let original = &ACTUAL[guard_start..guard_end];
    cases.push((
        "alternate effect".into(),
        ACTUAL.replacen(
            original,
            &format!("{} else {{ extra(); }}\n", original.trim_end()),
            1,
        ),
    ));
    let reordered = parent_change("$KEY_92.clear();", "");
    cases.push(("clear after listener removal".into(), {
        let at = reordered[..526641].rfind("$KEY_402 = false;").unwrap();
        format!("{}$KEY_92.clear(); {}", &reordered[..at], &reordered[at..])
    }));
    let early = parent_change(
        "$KEY_3 = $CC[1].call($CC[0], \"style\");",
        "let $CD = { init: $THREAD_6_4235 }; $CD.teardown = $THREAD_4_6761; $KEY_3 = $CC[1].call($CC[0], \"style\");",
    );
    cases.push(("publication before style".into(), {
        let decl = "let $CD = { init: $THREAD_6_4235 };";
        let at = early.rfind(decl).unwrap();
        format!("{}{}", &early[..at], &early[at + decl.len()..])
    }));
    let early_document = parent_change(
        "$KEY_11 = $KEY_5.global;",
        "let $CD = { init: $THREAD_6_4235 }; $CD.teardown = $THREAD_4_6761; $KEY_11 = $KEY_5.global;",
    );
    let decl = "let $CD = { init: $THREAD_6_4235 };";
    let at = early_document.rfind(decl).unwrap();
    cases.push((
        "publication before document".into(),
        format!(
            "{}{}",
            &early_document[..at],
            &early_document[at + decl.len()..]
        ),
    ));
    for (case, source) in cases {
        let (accepted, why) = guarded(&source);
        assert!(!accepted, "accepted {case}: {why}");
    }
}

// Saved byte ranges are diagnostic fixture slicing only. The recognizer uses
// neither these numbers nor the fixture's THREAD names as evidence.
const PARENTS: [(usize, usize); 2] = [(517820, 526641), (774407, 783226)];
fn in_parent(arm: usize, change: impl FnOnce(&str) -> String) -> String {
    let (start, end) = PARENTS[arm];
    let parent = &ACTUAL[start..end];
    let modified = change(parent);
    assert_ne!(modified, parent, "mutation/equivalent must hit");
    format!("{}{}{}", &ACTUAL[..start], modified, &ACTUAL[end..])
}
fn setup_order(arm: usize, order: [usize; 4], direct: bool) -> String {
    in_parent(arm, |parent| {
        let start = parent.rfind("$KEY_11 = $KEY_5.global;").unwrap();
        let end = parent[start..].find("$KEY_0 = null;").unwrap() + start;
        let lines: Vec<_> = parent[start..end].trim().lines().map(str::trim).collect();
        assert_eq!(lines.len(), 4);
        let mut new = Vec::new();
        for index in order {
            if direct && index == 2 {
                continue;
            }
            new.push(if direct && index == 3 {
                "$KEY_3 = $KEY_9.createElement(\"style\");"
            } else {
                lines[index]
            });
        }
        format!(
            "{}{}\n\t\t\t\t\t\t{}",
            &parent[..start],
            new.join("\n\t\t\t\t\t\t"),
            &parent[end..]
        )
    })
}
fn production_refuses(source: &str, arm: usize) {
    let candidates = guarded_candidates(source, "$THREAD_4_6761");
    let span = candidates[arm].0;
    let id = format!("fn@{}..{}", span.start, span.end);
    let result = crate::naming::name_threads(source, true).unwrap();
    let rows: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|o| o.function_id == id)
        .collect();
    assert_eq!(rows.len(), 1, "nonvacuous production refusal {id}");
    assert!(
        !rows[0].applied,
        "unexpected production fallback: {:?}",
        rows[0]
    );
}
fn assert_shape_without_legacy_fallback(source: &str, arm: usize) {
    let span = guarded_candidates(source, "$THREAD_4_6761")[arm].0;
    inspect(source, |model| {
        let site = model
            .all_sites()
            .iter()
            .copied()
            .find(|s| s.span == span)
            .unwrap();
        assert!(
            guarded_cleanup::has_guarded_shape(model, site),
            "must exercise valid guarded grammar"
        );
        let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
            unreachable!()
        };
        assert!(
            teardown_submit_instrumentation(model, site, f.body.as_ref().unwrap()).is_none(),
            "guarded counterexample must not fall through legacy grammar"
        );
    });
}
#[test]
fn ordered_producers_both_arms_and_supported_forms() {
    use sha2::{Digest, Sha256};
    let exact = setup_order(0, [2, 3, 0, 1], false);
    assert_eq!(
        Sha256::digest(exact.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        "8c82a5195577a47f2ca2acb1817f7d0c4d5321457aa99946c424e3b251be24b8"
    );
    let mut negatives = vec![
        ("review exact tuple before host/document", exact, 0),
        (
            "second-arm tuple before producers",
            setup_order(1, [2, 3, 0, 1], false),
            1,
        ),
        (
            "direct call before document",
            setup_order(0, [3, 0, 1, 2], true),
            0,
        ),
        (
            "document before host",
            setup_order(0, [1, 0, 2, 3], false),
            0,
        ),
        (
            "tuple before document after host",
            setup_order(0, [0, 2, 3, 1], false),
            0,
        ),
        (
            "tuple call before tuple",
            setup_order(0, [0, 1, 3, 2], false),
            0,
        ),
    ];
    for (label, producer, alias) in [
        (
            "host alias before producer",
            "$KEY_11 = $KEY_5.global;",
            "let earlyHost=$KEY_11; ",
        ),
        (
            "document alias before producer",
            "$KEY_9 = $KEY_11.document;",
            "let earlyDocument=$KEY_9; ",
        ),
        (
            "tuple alias before producer",
            "let $CC = [$KEY_9, $KEY_9.createElement];",
            "let earlyTuple=$CC; ",
        ),
        (
            "style alias before producer",
            "$KEY_3 = $CC[1].call($CC[0], \"style\");",
            "let earlyStyle=$KEY_3; ",
        ),
    ] {
        negatives.push((
            label,
            in_parent(0, |p| {
                p.replacen(producer, &format!("{alias}{producer}"), 1)
            }),
            0,
        ));
    }
    for (label, source, arm) in &negatives {
        assert_shape_without_legacy_fallback(source, *arm);
        let rows = guarded_candidates(source, "$THREAD_4_6761");
        assert!(!rows[*arm].1, "accepted {label}");
        assert_eq!(
            rows[*arm].2, "factory publication",
            "{label}: must fail producer walk, not unrelated protocol"
        );
    }
    // Full production dispatch must refuse the counterexample and each
    // distinct immediate dependency boundary (including the alternate arm).
    for (_, source, arm) in &negatives[..6] {
        production_refuses(source, *arm);
    }
    for arm in 0..2 {
        let direct = setup_order(arm, [0, 1, 2, 3], true);
        for (_, accepted, why) in guarded_candidates(&direct, "$THREAD_4_6761") {
            assert!(accepted, "direct arm {arm}: {why}");
        }
    }
    // Independent null reset may move before the host/style producers. Bodies
    // of the hoisted callbacks stay deferred and do not count as early reads.
    let reordered = in_parent(0, |p| {
        let at = p.rfind("$KEY_0 = null;").unwrap();
        let without = format!("{}{}", &p[..at], &p[at + "$KEY_0 = null;".len()..]);
        without.replacen(
            "$KEY_11 = $KEY_5.global;",
            "$KEY_0 = null; $KEY_11 = $KEY_5.global;",
            1,
        )
    });
    for (_, accepted, why) in guarded_candidates(&reordered, "$THREAD_4_6761") {
        assert!(accepted, "legitimate setup order: {why}");
    }
}
#[test]
fn distinct_flag_reset_order_and_distinct_registry_order() {
    let flag = in_parent(0, |p| {
        let begin = p.find("function $THREAD_4_6761()").unwrap();
        let end = p.rfind("$KEY_11 = $KEY_5.global;").unwrap();
        format!(
            "{}let replacementFlag=null; {}{}",
            &p[..begin],
            p[begin..end].replace("$KEY_402", "replacementFlag"),
            &p[end..]
        )
    });
    let reset = ACTUAL.replacen("$KEY_402 = false;", "", 1).replacen(
        "$KEY_176 = new $BT();",
        "$KEY_176 = new $BT(); $KEY_402 = false;",
        1,
    );
    let swap = in_parent(0, |p| {
        p.replacen("$KEY_176.clear();", "SWAP_CLEAR", 1)
            .replacen("$KEY_37.clear();", "$KEY_176.clear();", 1)
            .replacen("SWAP_CLEAR", "$KEY_37.clear();", 1)
    });
    for (label, source) in [
        ("consistent distinct initialized flag", flag),
        ("reset after first registry construction", reset),
        ("swap two distinct registry clears", swap),
    ] {
        assert_shape_without_legacy_fallback(&source, 0);
        assert!(!guarded(&source).0, "accepted {label}");
        production_refuses(&source, 0);
        println!("B3 production refusal: {label}");
    }
}

const DUAL_CLEANUP: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/program/submit-cleanup/captured-submit-parent.js"
));

// Both original bodies and their full parent/activation context are retained.
// Names and byte spans here are diagnostic fixture locators only.
const DUAL_CLEANUP_PARENTS: [(usize, usize); 2] = [(443775, 450603), (638298, 644956)];
fn dual_cleanup_candidates(source: &str) -> Vec<(oxc_span::Span, bool, String)> {
    guarded_candidates(source, "$THREAD_4_28197")
}
fn dual_cleanup_change(arm: usize, from: &str, to: &str) -> String {
    let (start, end) = DUAL_CLEANUP_PARENTS[arm];
    let parent = &DUAL_CLEANUP[start..end];
    let at = parent.rfind(from).expect("mutation must hit own parent");
    format!(
        "{}{}{}{}{}",
        &DUAL_CLEANUP[..start],
        &parent[..at],
        to,
        &parent[at + from.len()..],
        &DUAL_CLEANUP[end..]
    )
}
fn dual_cleanup_body_change(arm: usize, from: &str, to: &str) -> String {
    let (start, end) = [(443889, 444336), (638411, 638842)][arm];
    let body = &DUAL_CLEANUP[start..end];
    assert!(body.contains(from), "body mutation must hit {from}");
    format!(
        "{}{}{}",
        &DUAL_CLEANUP[..start],
        body.replacen(from, to, 1),
        &DUAL_CLEANUP[end..]
    )
}
#[test]
fn dual_cleanup_first_exact_protocol_refusal() {
    inspect(DUAL_CLEANUP, |model| {
        for (start, end) in [(443889, 444336), (638411, 638842)] {
            let site = model
                .all_sites()
                .iter()
                .copied()
                .find(|s| s.span.start == start && s.span.end == end)
                .unwrap();
            assert!(guarded_cleanup::has_guarded_shape(model, site));
            let factory = model
                .enclosing_function(model.nodes.parent_id(site.node_id))
                .unwrap();
            let container = model.nodes.parent_id(factory.node_id);
            // The epoch guard requires this parent to be a BlockStatement immediately inside an
            // IfStatement. Neither factory has that shape.
            assert!(!matches!(
                model.nodes.parent_kind(container),
                AstKind::IfStatement(_)
            ));
            assert!(matches!(
                model.nodes.kind(container),
                AstKind::BlockStatement(_) | AstKind::FunctionBody(_)
            ));
            assert!(
                guarded_cleanup::accepts(model, site),
                "{}",
                guarded_cleanup::diagnostic(model, site)
            );
            println!(
                "cleanup candidate {start}..{end}: returning catch/normal continuation and own function/prototype registry prove exclusive epoch"
            );
        }
    });
}
#[test]
fn dual_cleanup_actual_both_model_production_roles_and_equivalents() {
    for (_, accepted, why) in dual_cleanup_candidates(DUAL_CLEANUP) {
        assert!(accepted, "{why}");
    }
    let named = crate::naming::name_threads(DUAL_CLEANUP, true).unwrap();
    for id in ["fn@443889..444336", "fn@638411..638842"] {
        let rows: Vec<_> = named
            .report
            .outcomes
            .iter()
            .filter(|o| o.function_id == id)
            .collect();
        assert_eq!(rows.len(), 1);
        let row = rows[0];
        assert!(row.applied, "{row:?}");
        assert_eq!(
            row.role_name.as_deref(),
            Some("teardownSubmitInstrumentation")
        );
        assert_eq!(
            row.evaluator.as_deref(),
            Some("browser_submit_instrumentation_teardown")
        );
        assert_eq!(row.reference_edits, 1);
        assert_eq!(
            named
                .report
                .function_roles
                .iter()
                .filter(|r| format!("fn@{}..{}", r.function_span.start, r.function_span.end) == id)
                .count(),
            1
        );
    }
    assert_eq!(
        named
            .report
            .outcomes
            .iter()
            .filter(|o| o.applied
                && o.evaluator.as_deref() == Some("browser_submit_instrumentation_teardown"))
            .count(),
        2
    );
    // Alpha changes are full-source lexical fixture changes, never rule keys.
    let alpha = DUAL_CLEANUP
        .replace("$KEY_43", "$cleanupEnabled")
        .replace("$KEY_110", "$cleanupFirstRegistry")
        .replace("$KEY_101", "$cleanupInstallSurface")
        .replace("$KEY_48", "$cleanupRetainedHandler");
    for (_, accepted, why) in dual_cleanup_candidates(&alpha) {
        assert!(accepted, "alpha: {why}");
    }
    for arm in 0..2 {
        // An independent null store can precede host creation. Its callbacks
        // remain deferred. No effectful listener/registry operation is moved.
        let moved = dual_cleanup_change(arm, "$KEY_9 = null;", "");
        let (start, _) = DUAL_CLEANUP_PARENTS[arm];
        let at = moved[start..].find("$KEY_5 = $KEY_8.global;").unwrap() + start;
        let moved = format!("{}$KEY_9 = null; {}", &moved[..at], &moved[at..]);
        for (_, accepted, why) in dual_cleanup_candidates(&moved) {
            assert!(accepted, "neutral order: {why}");
        }
        let split = dual_cleanup_change(
            arm,
            "let $KEY_9, $KEY_10, $KEY_13, $KEY_1, $KEY_2;",
            "var $KEY_9; let $KEY_10, $KEY_13; var $KEY_1; let $KEY_2;",
        );
        for (_, accepted, why) in dual_cleanup_candidates(&split) {
            assert!(accepted, "split declarations: {why}");
        }
        let tuple = dual_cleanup_change(
            arm,
            "$KEY_10 = $KEY_2.createElement(\"style\");",
            "let styleOperands = [$KEY_2, $KEY_2.createElement]; $KEY_10 = styleOperands[1].call(styleOperands[0], \"style\");",
        );
        for (_, accepted, why) in dual_cleanup_candidates(&tuple) {
            assert!(accepted, "equivalent style operands: {why}");
        }
        let alias = dual_cleanup_change(
            arm,
            "$KEY_1 = $THREAD_4_10212;",
            "let retainedClick = $THREAD_4_10212; $KEY_1 = retainedClick;",
        );
        // Callable alias chains are deliberately outside the retained proof;
        // neither equivalent-looking aliases nor source similarity earn credit.
        assert!(!dual_cleanup_candidates(&alias)[arm].1);
    }
}
#[test]
fn dual_cleanup_refuses_full_contract_and_lifecycle_mutations() {
    let mut cases: Vec<(&str, String, usize)> = Vec::new();
    for arm in 0..2 {
        for (label, from, to) in [
            (
                "click receiver",
                "$KEY_2.removeEventListener(\"click\", $KEY_1)",
                "$KEY_5.removeEventListener(\"click\", $KEY_1)",
            ),
            (
                "click listener",
                "$KEY_2.removeEventListener(\"click\", $KEY_1)",
                "$KEY_2.removeEventListener(\"click\", $KEY_3)",
            ),
            (
                "click options",
                "$KEY_2.removeEventListener(\"click\", $KEY_1)",
                "$KEY_2.removeEventListener(\"click\", $KEY_1, true)",
            ),
            (
                "guard handler",
                "if ($KEY_48 != null)",
                "if ($KEY_9 != null)",
            ),
            (
                "submit receiver",
                "$KEY_101.removeEventListener(\"submit\", $KEY_48)",
                "$KEY_2.removeEventListener(\"submit\", $KEY_48)",
            ),
            (
                "submit listener",
                "$KEY_101.removeEventListener(\"submit\", $KEY_48)",
                "$KEY_101.removeEventListener(\"submit\", $KEY_1)",
            ),
            (
                "submit options",
                "$KEY_101.removeEventListener(\"submit\", $KEY_48)",
                "$KEY_101.removeEventListener(\"submit\", $KEY_48, true)",
            ),
            ("flag", "$KEY_43 = false;", "$KEY_43 = true;"),
            ("head receiver", "if ($KEY_2.head)", "if ($KEY_5.head)"),
            (
                "head child",
                "$KEY_2.head.removeChild($KEY_10)",
                "$KEY_2.head.removeChild($KEY_9)",
            ),
            (
                "extra effect",
                "$KEY_336.clear();",
                "$KEY_336.clear(); extra();",
            ),
            ("missing clear", "$KEY_336.clear();", ""),
            ("clear order", "$KEY_336.clear();", "$KEY_47.clear();"),
            ("nonempty catch", "catch (e) {}", "catch (e) { extra(); }"),
            (
                "early publication",
                "$KEY_5 = $KEY_8.global;",
                "let escape = $THREAD_4_28197; $KEY_5 = $KEY_8.global;",
            ),
            (
                "copy before init",
                "$KEY_5 = $KEY_8.global;",
                "let unready = $KEY_2; $KEY_5 = $KEY_8.global;",
            ),
            (
                "host read before init",
                "$KEY_5 = $KEY_8.global;",
                "$KEY_2 = $KEY_5.document; $KEY_5 = $KEY_8.global;",
            ),
            (
                "branch producer",
                "$KEY_2 = $KEY_5.document;",
                "if (condition) { $KEY_2 = $KEY_5.document; }",
            ),
            (
                "deferred producer",
                "$KEY_2 = $KEY_5.document;",
                "function later(){ $KEY_2 = $KEY_5.document; }",
            ),
            (
                "later writer",
                "$AX.teardown = $THREAD_4_28197;",
                "$AX.teardown = $THREAD_4_28197; $KEY_2 = replacement;",
            ),
            (
                "reentry",
                "$KEY_10 = $KEY_2.createElement(\"style\");",
                "$THREAD_4_28197(); $KEY_10 = $KEY_2.createElement(\"style\");",
            ),
            (
                "init export before document",
                "$KEY_5 = $KEY_8.global;",
                "let escapeInit = $THREAD_8_121882; $KEY_5 = $KEY_8.global;",
            ),
        ] {
            cases.push((
                label,
                if from.contains("removeEventListener")
                    || [
                        "guard handler",
                        "flag",
                        "head receiver",
                        "head child",
                        "extra effect",
                        "missing clear",
                        "clear order",
                        "nonempty catch",
                    ]
                    .contains(&label)
                {
                    dual_cleanup_body_change(arm, from, to)
                } else {
                    dual_cleanup_change(arm, from, to)
                },
                arm,
            ));
        }
    }
    for (label, from, to) in [
        (
            "clear body wrong collection",
            "this.callbacks = [];\n\t\t\t\t\t\treturn this;",
            "this.other = [];\n\t\t\t\t\t\treturn this;",
        ),
        (
            "clear deferred reset",
            "this.callbacks = [];\n\t\t\t\t\t\treturn this;",
            "return this;",
        ),
        (
            "constructor reentry",
            "function $THREAD_8_122616() {",
            "function $THREAD_8_122616() { $KEY_157();",
        ),
        (
            "prototype later write",
            "$THREAD_8_122616.prototype.clear = $THREAD_1_60582;",
            "$THREAD_8_122616.prototype.clear = $THREAD_1_60582; $THREAD_8_122616.prototype.clear = replacement;",
        ),
        (
            "prototype alias",
            "$THREAD_8_122616.prototype.clear = $THREAD_1_60582;",
            "let escapedPrototype = $THREAD_8_122616.prototype; $THREAD_8_122616.prototype.clear = $THREAD_1_60582;",
        ),
        (
            "clear external helper",
            "this.callbacks = [];\n\t\t\t\t\t\treturn this;",
            "externalClear(this); return this;",
        ),
        (
            "registry reentry before init",
            "$KEY_110 = new $AJ();",
            "$KEY_157(); $KEY_110 = new $AJ();",
        ),
        (
            "setter installation options",
            "$KEY_101.addEventListener(\"submit\", $THREAD_6_73862, true)",
            "$KEY_101.addEventListener(\"submit\", $THREAD_6_73862, false)",
        ),
    ] {
        assert!(DUAL_CLEANUP.contains(from), "mutation must hit {label}");
        cases.push((label, DUAL_CLEANUP.replacen(from, to, 1), 0));
    }
    for (label, from, to) in [
        (
            "catch may fall through",
            "return { init: $THREAD_8_65381 };",
            "void 0;",
        ),
        (
            "normal registry before init",
            "$KEY_110 = new $AU();",
            "$KEY_110.clear(); $KEY_110 = new $AU();",
        ),
        (
            "catch registry before init",
            "$KEY_110 = new $AJ();",
            "$KEY_110.notify(null); $KEY_110 = new $AJ();",
        ),
    ] {
        assert!(DUAL_CLEANUP.contains(from));
        // The caught and normal setup are one protocol: corrupting either
        // initialization path must refuse both candidate lifecycles.
        let source = DUAL_CLEANUP.replacen(from, to, 1);
        cases.push((label, source.clone(), 0));
        cases.push((label, source, 1));
    }
    for (index, (label, source, arm)) in cases.iter().enumerate() {
        let candidates = dual_cleanup_candidates(source);
        assert!(
            !candidates[*arm].1,
            "accepted {label} arm {arm}: {}",
            candidates[*arm].2
        );
        // Full production refuses each lifecycle boundary and a sample of body
        // refusals in both arms, including shape-preserving capture mutations.
        if index % 4 == 0 || index >= 44 {
            let span = candidates[*arm].0;
            let id = format!("fn@{}..{}", span.start, span.end);
            let named = crate::naming::name_threads(source, true).unwrap();
            let rows: Vec<_> = named
                .report
                .outcomes
                .iter()
                .filter(|o| o.function_id == id)
                .collect();
            assert_eq!(rows.len(), 1, "nonvacuous production {label}");
            assert!(
                !rows[0].applied,
                "production accepted {label}: {:?}",
                rows[0]
            );
        }
        println!("cleanup negative {label} arm {arm}: {}", candidates[*arm].2);
    }
}

const CLOSED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/program/submit-cleanup/closed-lifecycle.js"
));
const CALLED_UNINITIALIZED_READER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/program/submit-cleanup/deferred-uninitialized-reader.js"
));

fn both_lifecycles(source: &str, expected: bool, case: &str) {
    let private = dual_cleanup_candidates(source);
    assert_eq!(private.len(), 2);
    for (_, accepted, why) in &private {
        assert_eq!(*accepted, expected, "{case}: {why}");
    }
    let named = crate::naming::name_threads(source, true).unwrap();
    for (span, _, _) in private {
        let id = format!("fn@{}..{}", span.start, span.end);
        let rows: Vec<_> = named
            .report
            .outcomes
            .iter()
            .filter(|o| o.function_id == id)
            .collect();
        assert_eq!(rows.len(), 1, "nonvacuous {case}: {id}");
        assert_eq!(
            rows[0].applied, expected,
            "production {case}: {:?}",
            rows[0]
        );
        if expected {
            assert_eq!(
                rows[0].role_name.as_deref(),
                Some("teardownSubmitInstrumentation")
            );
            assert_eq!(
                rows[0].evaluator.as_deref(),
                Some("browser_submit_instrumentation_teardown")
            );
            assert_eq!(rows[0].reference_edits, 1);
        }
    }
}
#[test]
fn called_uninitialized_reader_refuses_both_roles() {
    use sha2::{Digest, Sha256};
    assert_eq!(
        Sha256::digest(CALLED_UNINITIALIZED_READER.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        "a68078968af3ac1f4b85aef0fd6ecb562a55ae8f65aa70873bffce47fc44fa5d"
    );
    let actual = DUAL_CLEANUP.replacen("$KEY_110 = new $AJ();",
        "function reviewReadUnready(){ $KEY_110.clear(); } reviewReadUnready(); $KEY_110 = new $AJ();",1);
    assert_eq!(
        actual, CALLED_UNINITIALIZED_READER,
        "exact reviewer mutation, not a changed expectation fixture"
    );
    let rows = dual_cleanup_candidates(CALLED_UNINITIALIZED_READER);
    assert_eq!(
        rows.iter()
            .map(|x| (x.0.start, x.0.end))
            .collect::<Vec<_>>(),
        vec![(443960, 444407), (638482, 638913)]
    );
    inspect(CALLED_UNINITIALIZED_READER, |model| {
        for (span, _, _) in &rows {
            let site = model
                .all_sites()
                .iter()
                .copied()
                .find(|s| s.span == *span)
                .unwrap();
            assert!(guarded_cleanup::has_guarded_shape(model, site));
            assert_eq!(
                guarded_cleanup::diagnostic(model, site),
                "known local reader readiness"
            );
        }
    });
    both_lifecycles(
        CALLED_UNINITIALIZED_READER,
        false,
        "immediately invoked uninitialized local reader",
    );
    println!(
        "both cleanup callbacks refuse known local reader readiness in private Model and production; two-candidate positive controls preserve counts"
    );
}
#[test]
fn reader_entries_closed_protocol_positive_aliases_and_inert_declarations() {
    both_lifecycles(CLOSED, true, "complete closed protocol");
    for (before, after) in [
        ("CATCH_BEFORE", "CATCH_AFTER"),
        ("NORMAL_BEFORE", "NORMAL_AFTER"),
    ] {
        for (label, declaration, call) in [
            (
                "inert returning reader",
                "function reader(){return first.clear();}",
                "",
            ),
            (
                "inert unused alias",
                "function reader(){first.clear();} let unused=reader;",
                "",
            ),
            ("inert reader", "function reader(){first.clear();}", ""),
            (
                "late direct entry",
                "function reader(){first.clear();}",
                "reader();",
            ),
            (
                "late nested entry",
                "function reader(){first.clear();} function outer(){reader();}",
                "outer();",
            ),
            (
                "late callable aliases",
                "function reader(){first.clear();} let one=reader; let two=one;",
                "two();",
            ),
            (
                "late callback alias",
                "function reader(){first.clear();} let callback=reader; function consume(fn){fn();}",
                "consume(callback);",
            ),
            (
                "nested alias ready at entry",
                "function reader(){first.clear();} function outer(){alias();} let alias=reader;",
                "outer();",
            ),
            (
                "recursive inert readers",
                "function left(){right();} function right(){first.clear();left();}",
                "",
            ),
            (
                "unrelated early consumer",
                "function unrelated(){external.clear();} unrelated();",
                "",
            ),
        ] {
            let source = CLOSED
                .replace(&format!("// {before}"), declaration)
                .replace(&format!("// {after}"), call);
            both_lifecycles(&source, true, label);
        }
    }
    for marker in ["CATCH_BEFORE", "NORMAL_BEFORE"] {
        for (label, body) in [
            ("flag already produced before registries", "return enabled;"),
            (
                "document already produced before registries",
                "return installDocument.head;",
            ),
        ] {
            let source = CLOSED.replace(
                &format!("// {marker}"),
                &format!("function reader(){{{body}}} reader();"),
            );
            both_lifecycles(&source, true, label);
        }
    }
    for constructor in ["Constructor", "Constructor2"] {
        let from = format!("first=new {constructor}();second=new {constructor}();");
        let prefix = format!(
            "first=new {constructor}();function reader(){{first.clear();}} reader();second=new {constructor}();"
        );
        assert!(CLOSED.contains(&from));
        both_lifecycles(
            &CLOSED.replace(&from, &prefix),
            true,
            "own first registry produced, unrelated second not yet produced",
        );
    }
    let late_callback = CLOSED.replace("// NORMAL_AFTER", "consume(()=>first.clear());");
    both_lifecycles(&late_callback, true, "late inline callback publication");
}
#[test]
fn reader_entries_closed_protocol_relevant_early_calls_copies_and_retention_refuse() {
    for (before, after) in [
        ("CATCH_BEFORE", "CATCH_AFTER"),
        ("NORMAL_BEFORE", "NORMAL_AFTER"),
    ] {
        for (label, early, late) in [
            (
                "direct known helper",
                "function reader(){first.clear();} reader();",
                "",
            ),
            (
                "transitive known helper",
                "function reader(){first.clear();} function outer(){reader();} outer();",
                "",
            ),
            (
                "helper alias",
                "function reader(){first.clear();} let copy=reader; copy();",
                "",
            ),
            (
                "callback alias",
                "function reader(){first.clear();} let callback=reader; function consume(fn){fn();} consume(callback);",
                "",
            ),
            ("inline callback", "consume(()=>first.clear());", ""),
            (
                "returned capability",
                "function reader(){first.clear();} function get(){return reader;} let copy=get(); copy();",
                "",
            ),
            (
                "unknown early publication",
                "function reader(){first.clear();} publish(reader);",
                "",
            ),
            (
                "object publication",
                "function reader(){first.clear();} external.callback=reader;",
                "",
            ),
            (
                "copy before executed alias store",
                "function reader(){first.clear();} var alias; let copy=alias; alias=reader;",
                "copy();",
            ),
            (
                "nested copy before producer",
                "function reader(){first.clear();} var alias; function copy(){let held=alias;} copy(); alias=reader;",
                "",
            ),
            (
                "recursive reachable reader",
                "function left(){right();} function right(){first.clear();left();} left();",
                "",
            ),
            (
                "wrong path reflection",
                "function reader(){first.clear();} reader.call({});",
                "",
            ),
            (
                "notify also reads captured registry",
                "function reader(){first.notify(null);} reader();",
                "",
            ),
            (
                "later known method mutation",
                "",
                "function mutate(){first.clear=replacement;} mutate();",
            ),
            (
                "later alias writer",
                "function reader(){first.clear();} var alias=reader;",
                "alias=replacement; alias();",
            ),
            (
                "clear result publishes registry",
                "",
                "let leaked=first.clear(); leaked.clear=replacement;",
            ),
            (
                "returning registry aliases",
                "",
                "function leak(){return first.clear();} publish(leak);",
            ),
            (
                "late reflection context unsupported",
                "function reader(){first.clear();}",
                "reader.call({});",
            ),
            (
                "deferred reader alias not yet produced at actual entry",
                "function reader(){first.clear();} function outer(){copy();} var copy;",
                "outer();copy=reader;",
            ),
        ] {
            let source = CLOSED
                .replace(&format!("// {before}"), early)
                .replace(&format!("// {after}"), late);
            both_lifecycles(&source, false, label);
        }
    }
    for (label, body) in [
        ("flag before its producer", "return enabled;"),
        (
            "document before its producer",
            "return installDocument.head;",
        ),
    ] {
        let source = CLOSED.replace(
            "// BEFORE_BOTH",
            &format!("function reader(){{{body}}} reader();"),
        );
        both_lifecycles(&source, false, label);
    }
    for constructor in ["Constructor", "Constructor2"] {
        let from = format!("first=new {constructor}();second=new {constructor}();");
        let prefix = format!(
            "first=new {constructor}();function reader(){{second.clear();}} reader();second=new {constructor}();"
        );
        both_lifecycles(
            &CLOSED.replace(&from, &prefix),
            false,
            "same actual entry now reads uninitialized second registry",
        );
    }
    let early_handler = CLOSED.replace(
        "// NORMAL_BEFORE",
        "function reader(){return retained;} reader();",
    );
    both_lifecycles(
        &early_handler,
        false,
        "known handler slot before actual producer",
    );
    // Extra teardown reference is a real call via a helper, not a body hash or
    // a declaration-position rejection. The original factory proof must refuse.
    let early_teardown = CLOSED.replace(
        "click=onClick;host=port.global;",
        "function invoke(){ $THREAD_4_28197(); } invoke(); click=onClick;host=port.global;",
    );
    both_lifecycles(
        &early_teardown,
        false,
        "direct early teardown through known helper",
    );
}

#[test]
fn old_canonical_minimal_teardown_controls_remain_positive() {
    const CANONICAL: &str = r#"function main(){let doc={head:null},flag=true,reg1={},reg2={},reg3={},clickHandler=()=>{},submitHandler=()=>{},styleNode={};
function $THREAD_6_6(){try{doc.removeEventListener("click",clickHandler);
if(!!flag){reg1.clear();reg2.clear();reg3.clear();
window.document.removeEventListener("submit",submitHandler);flag=false;}
if(doc.head){doc.head.removeChild(styleNode);}}catch(e){}}
return $THREAD_6_6;}main();"#;
    for source in [
        CANONICAL.to_string(),
        CANONICAL
            .replace("doc={", "surface={")
            .replace("doc.remove", "surface.remove")
            .replace("doc.head", "surface.head")
            .replace("flag", "enabled"),
        CANONICAL.replace(
            "flag=false;",
            "window.navigation.removeEventListener(\"navigate\",submitHandler);flag=false;",
        ),
        CANONICAL
            .replace("reg3={},", "reg3={},reg4={},")
            .replace("reg3.clear();", "reg3.clear();reg4.clear();"),
    ] {
        inspect(&source, |model| {
            let site = model
                .all_sites()
                .iter()
                .copied()
                .find(|s| {
                    s.binding_symbol
                        .is_some_and(|id| model.scoping.symbol_name(id) == "$THREAD_6_6")
                })
                .unwrap();
            let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
                unreachable!()
            };
            assert!(
                teardown_submit_instrumentation(model, site, f.body.as_ref().unwrap()).is_some()
            );
        });
        let named = crate::naming::name_threads(&source, true).unwrap();
        let rows: Vec<_> = named
            .report
            .outcomes
            .iter()
            .filter(|o| o.binding.name == "$THREAD_6_6")
            .collect();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].applied);
        assert_eq!(
            rows[0].role_name.as_deref(),
            Some("teardownSubmitInstrumentation")
        );
    }
}

const OPERATIONAL_CLASS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/program/submit-cleanup/class-positive.js"
));
const NON_OPERATIONAL_CLASS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/program/submit-cleanup/class-register-overwrites-clear.js"
));

#[test]
fn class_operational_contract_pair() {
    use sha2::{Digest, Sha256};
    for (source, hash, ids, accepted) in [
        (
            OPERATIONAL_CLASS,
            "f26076b901e9ffc1645c507b36cae7455e62e11c055a73458580a3e5f81fce12",
            [(1087, 1396), (2500, 2802)],
            true,
        ),
        (
            NON_OPERATIONAL_CLASS,
            "2973c964ed28ecf69f6cbe50e1cdce14a70a07c0d592b62c1e3ed3dfd6f6d5f7",
            [(1111, 1420), (2548, 2850)],
            false,
        ),
    ] {
        assert_eq!(
            Sha256::digest(source.as_bytes())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
            hash
        );
        let candidates = dual_cleanup_candidates(source);
        assert_eq!(
            candidates
                .iter()
                .map(|(s, _, _)| (s.start, s.end))
                .collect::<Vec<_>>(),
            ids
        );
        both_lifecycles(source, accepted, "exact guarded cleanup paired class contract");
        println!("guarded cleanup paired accepted={accepted}: {candidates:?}");
    }
    assert_eq!(
        OPERATIONAL_CLASS.replace(
            "this.callbacks.push(value);return this;",
            "this.clear=function(){};this.callbacks.push(value);return this;"
        ),
        NON_OPERATIONAL_CLASS
    );
}

#[test]
fn class_registry_reachable_method_writers_and_publications_refuse() {
    for (case, port, replacement) in [
        (
            "unregister replaces clear",
            "unregister(value){return this;}",
            "unregister(value){this.clear=function(){};return this;}",
        ),
        (
            "notify replaces clear",
            "notify(value){return null;}",
            "notify(value){this.clear=function(){};return null;}",
        ),
        (
            "receiver alias writer",
            "notify(value){return null;}",
            "notify(value){const receiver=this;receiver.clear=function(){};return null;}",
        ),
        (
            "helper captured writer",
            "notify(value){return null;}",
            "notify(value){const receiver=this;function change(){receiver.clear=function(){};}change();return null;}",
        ),
        (
            "helper argument writer",
            "unregister(value){return this;}",
            "unregister(value){function change(receiver){receiver.callbacks=[];}change(this);return this;}",
        ),
        (
            "arrow lexical this writer",
            "notify(value){return null;}",
            "notify(value){const change=()=>{this.clear=function(){};};change();return null;}",
        ),
        (
            "ordinary this wrong context",
            "notify(value){return null;}",
            "notify(value){function clearLocal(){this.clear();}clearLocal();return null;}",
        ),
        (
            "receiver publication",
            "notify(value){return null;}",
            "notify(value){value(this);return null;}",
        ),
        (
            "collection publication",
            "notify(value){return null;}",
            "notify(value){return this.callbacks;}",
        ),
        (
            "collection alias publication",
            "notify(value){return null;}",
            "notify(value){const list=this.callbacks;value(list);return null;}",
        ),
        (
            "collection alias method replacement",
            "notify(value){return null;}",
            "notify(value){const list=this.callbacks;list.push=function(){};return null;}",
        ),
        (
            "unexpected collection reset",
            "unregister(value){return this;}",
            "unregister(value){this.callbacks=[];return this;}",
        ),
        (
            "prototype reflection",
            "notify(value){return null;}",
            "notify(value){Object.setPrototypeOf(this,{});return null;}",
        ),
        (
            "unknown call receiver",
            "notify(value){return null;}",
            "notify(value){Reflect.apply(value,this,[]);return null;}",
        ),
        (
            "closure publication",
            "notify(value){return null;}",
            "notify(value){const change=()=>{this.clear();};value(change);return null;}",
        ),
        (
            "uninitialized alias read",
            "notify(value){return null;}",
            "notify(value){function read(){receiver.clear();}read();const receiver=this;return null;}",
        ),
        (
            "callable copy before producer",
            "notify(value){return null;}",
            "notify(value){const copy=change;const change=()=>{this.clear();};copy();return null;}",
        ),
        (
            "later helper alias mutation",
            "notify(value){return null;}",
            "notify(value){let receiver=this;function change(){receiver=value;}change();return null;}",
        ),
        (
            "callback receives registry",
            "notify(value){return null;}",
            "notify(value){this.callbacks[0].call(void 0,this);return null;}",
        ),
        (
            "callback gets collection receiver",
            "notify(value){return null;}",
            "notify(value){this.callbacks[0]();return null;}",
        ),
        (
            "callback mutates method via string index",
            "unregister(value){return this;}",
            "unregister(value){let index='push';this.callbacks[index]=void 0;return this;}",
        ),
        (
            "extra eager static block",
            "class Base{",
            "class Base{static{sideEffect();}",
        ),
        (
            "extra eager method decorator",
            "notify(value){return null;}",
            "@sideEffect notify(value){return null;}",
        ),
        (
            "extra class method",
            "notify(value){return null;}",
            "notify(value){return null;}replace(){this.clear=function(){};}",
        ),
    ] {
        let source = OPERATIONAL_CLASS.replace(port, replacement);
        assert_ne!(source, OPERATIONAL_CLASS, "mutation exists: {case}");
        both_lifecycles(&source, false, case);
    }
}

#[test]
fn class_registry_closed_local_helpers_aliases_and_real_data_ports_remain_positive() {
    for (case, port, replacement) in [
        (
            "known clear API",
            "unregister(value){return this;}",
            "unregister(value){this.clear();return this;}",
        ),
        (
            "receiver aliases",
            "unregister(value){return this;}",
            "unregister(value){const receiver=this;const copy=receiver;copy.clear();return this;}",
        ),
        (
            "captured local helper",
            "notify(value){return null;}",
            "notify(value){const receiver=this;function wipe(){receiver.clear();}wipe();return null;}",
        ),
        (
            "explicit helper receiver argument",
            "notify(value){return null;}",
            "notify(value){function wipe(receiver){receiver.clear();}wipe(this);return null;}",
        ),
        (
            "arrow lexical receiver and callable copy",
            "notify(value){return null;}",
            "notify(value){const wipe=()=>{this.clear();};const copy=wipe;copy();return null;}",
        ),
        (
            "late local producer entry",
            "notify(value){return null;}",
            "notify(value){function wipe(){receiver.clear();}const receiver=this;wipe();return null;}",
        ),
        (
            "inert declaration no executed mutation",
            "notify(value){return null;}",
            "notify(value){function inert(){this.clear=function(){};}return null;}",
        ),
        (
            "ordinary function no implicit receiver",
            "notify(value){return null;}",
            "notify(value){function ordinary(){return null;}ordinary();return null;}",
        ),
        (
            "collection numeric read",
            "notify(value){return null;}",
            "notify(value){const list=this.callbacks;let count=list.length;return count;}",
        ),
        (
            "callback no protected arguments",
            "notify(value){return null;}",
            "notify(value){return this.callbacks[0].call(void 0,value,null);}",
        ),
        (
            "source local numeric unregister loop",
            "unregister(value){return this;}",
            "unregister(value){let index=0;for(;index<this.callbacks.length;index++){if(this.callbacks[index]===value){this.callbacks[index]=void 0;}}return this;}",
        ),
        (
            "source local notify loop",
            "notify(value){return null;}",
            "notify(value){let index=0;let result=null;for(;index<this.callbacks.length;index++){if(this.callbacks[index]!=null){try{result=this.callbacks[index].call(void 0,value,result);}catch(error){}}}return result;}",
        ),
    ] {
        let source = OPERATIONAL_CLASS.replace(port, replacement);
        assert_ne!(source, OPERATIONAL_CLASS, "control exists: {case}");
        both_lifecycles(&source, true, case);
    }
    both_lifecycles(CLOSED, true, "original prototype registry");
}

#[test]
fn prototype_registry_shares_closed_data_method_inventory() {
    for (replacement, expected) in [
        (
            "function notify(value){this.clear=function(){};return null;}",
            false,
        ),
        (
            "function notify(value){const receiver=this;function change(){receiver.clear=function(){};}change();return null;}",
            false,
        ),
        (
            "function notify(value){const list=this.callbacks;return list;}",
            false,
        ),
        (
            "function notify(value){const receiver=this;function wipe(){receiver.clear();}wipe();return null;}",
            true,
        ),
        (
            "function notify(value){const wipe=()=>{this.clear();};wipe();return null;}",
            true,
        ),
    ] {
        let source = CLOSED
            .replace("function notify(value){return null;}", replacement)
            .replace(
                "function notify2(value){return null;}",
                &replacement.replace("notify(", "notify2("),
            );
        assert_ne!(source, CLOSED);
        both_lifecycles(&source, expected, replacement);
    }
}

#[test]
fn registry_borrowed_array_copy_port_is_bounded_and_not_a_registry_escape() {
    let port = "notify(value){return null;}";
    for (method, accepted) in [
        (
            "notify(value){const list=Array.prototype.slice.call(this.callbacks);let count=list.length;let result=null;while(count-1>=0){if(list[count-1]!=null){try{result=list[count-1].call(void 0,value,result);}catch(error){}}--count;}return result;}",
            true,
        ),
        (
            "notify(value){const list=Array.prototype.slice.call(this);return null;}",
            false,
        ),
        (
            "notify(value){const list=Array.prototype.slice.call(this.callbacks,this);return null;}",
            false,
        ),
        (
            "notify(value){return Array.prototype.slice.call(this.callbacks);}",
            false,
        ),
        (
            "notify(value){let Array=value;const list=Array.prototype.slice.call(this.callbacks);return null;}",
            false,
        ),
        (
            "notify(value){const list=Array.prototype.slice.call(this.callbacks);list.clear=function(){};return null;}",
            false,
        ),
        (
            "notify(value){const list=Array.prototype.slice.call(this.callbacks);value(list);return null;}",
            false,
        ),
        (
            "notify(value){const list=value(this.callbacks);return null;}",
            false,
        ),
    ] {
        both_lifecycles(&OPERATIONAL_CLASS.replace(port, method), accepted, method);
    }
}
