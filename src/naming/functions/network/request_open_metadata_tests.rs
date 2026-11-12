use super::*;
const ACTUAL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/program/wave-multi-consumer/reaching-registry-heldout.js"
));
const SPANS: [(u32, u32); 3] = [(283778, 284782), (495089, 496062), (748809, 749689)];
fn inspect(source: &str, check: impl FnOnce(&Model<'_, '_>)) {
    let a = oxc_allocator::Allocator::default();
    let parsed = crate::analysis::parse(&a, source);
    assert!(parsed.errors.is_empty(), "parse {:?}", parsed.errors);
    let built = oxc_semantic::SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    assert!(built.errors.is_empty(), "semantic {:?}", built.errors);
    let m = Model::new(
        source,
        &parsed.program,
        built.semantic.scoping(),
        built.semantic.nodes(),
        ProgramPropertiesLimits::default(),
    )
    .unwrap();
    check(&m)
}
fn sites(m: &Model<'_, '_>) -> Vec<FunctionSite> {
    m.all_sites().iter().copied().filter(|s|matches!(m.nodes.kind(s.node_id),AstKind::Function(f) if f.id.as_ref().is_some_and(|i|i.name=="$THREAD_9_65801"))).collect()
}
fn result(m: &Model<'_, '_>, site: FunctionSite) -> Result<(), &'static str> {
    let AstKind::Function(fun) = m.nodes.kind(site.node_id) else {
        panic!()
    };
    protocol(m, site, fun)
}
fn callback() -> String {
    ACTUAL[SPANS[0].0 as usize..SPANS[0].1 as usize].into()
}
fn fixture() -> String {
    format!("function root() {{ let $KEY_145=['opaque','alternate']; function log(e) {{ consume(e); }} let $KEY_11=log; function setup() {{ let $KEY_0=''; {} publish($THREAD_9_65801); }} setup(); }} root();",callback())
}
#[test]
fn actual_three_full_body_and_first_guard() {
    inspect(ACTUAL, |m| {
        let found = sites(m);
        assert_eq!(found.len(), 3);
        let mut scratch = Vec::new();
        for (site, (a, b)) in found.into_iter().zip(SPANS) {
            assert_eq!(site.span, Span::new(a, b));
            let AstKind::Function(fun) = m.nodes.kind(site.node_id) else {
                panic!()
            };
            let proof = body(m, site, fun).expect("actual complete body protocol");
            scratch.push(proof.scratch);
            assert_eq!(m.binding_writes(proof.key), 1);
            assert_eq!(
                m.binding_writes(proof.logger),
                4,
                "all scoped logger writers, never a last-writer guess"
            );
            assert!(
                capture_ready(m, site, proof.logger, false).is_none(),
                "001 first guard remains unchanged"
            );
            assert!(
                logger_with_boolean_dead_stores(m, site, proof.logger).is_some(),
                "every numeric writer has an impossible boolean/null guard"
            );
            if a == SPANS[2].0 {
                assert_eq!(result(m, site), Ok(()));
            } else {
                assert!(
                    result(m, site).is_err(),
                    "dormant setup copy has no initialized key/publication contract"
                );
            }
            assert!(
                crate::naming::functions::network::xhr_open_metadata::evaluate(m, site, &Facts::default())
                    .is_empty()
            );
            eprintln!(
                "actual {}..{}: {:?}; key_writers={} logger_writers={}",
                a,
                b,
                result(m, site),
                m.binding_writes(proof.key),
                m.binding_writes(proof.logger)
            );
        }
        assert_eq!(scratch.iter().collect::<BTreeSet<_>>().len(), 3);
    });
}
#[test]
fn positive_alpha_and_distinct_scopes() {
    let src = fixture();
    inspect(&src, |m| {
        let found = sites(m);
        assert_eq!(found.len(), 1);
        assert_eq!(result(m, found[0]), Ok(()));
    });
    let alpha = src
        .replace("$HE", "event")
        .replace("$HF", "previous")
        .replace("$HG", "methodScratch")
        .replace("$HH", "descriptorScratch")
        .replace("$HI", "errorRecord")
        .replace("$KEY_145", "metadataKeys")
        .replace("$KEY_11", "diagnostic");
    inspect(&alpha, |m| assert_eq!(result(m, sites(m)[0]), Ok(())));
    let three = format!(
        "function envelope() {{ let a,b,c,d,e,f,g,h; {} {} {} }} envelope();",
        src.replace("root", "rootA"),
        src.replace("root", "rootB"),
        src.replace("root", "rootC")
    );
    inspect(&three, |m| {
        let found = sites(m);
        assert_eq!(found.len(), 3);
        for site in found {
            assert_eq!(result(m, site), Ok(()));
        }
    });
    let named = crate::naming::name_threads(&three, true).unwrap();
    let claims: Vec<_> = named
        .report
        .outcomes
        .iter()
        .filter(|o| {
            o.evaluator
                .as_deref()
                .is_some_and(|e| e.contains("request_open_metadata"))
        })
        .collect();
    assert_eq!(claims.len(), 3);
    assert!(claims.iter().all(|c| c.applied));
    let separate = src
        .replace("$HG = {", "let record = {")
        .replace("$HH.value = $HG;", "$HH.value = record;");
    inspect(&separate, |m| assert_eq!(result(m, sites(m)[0]), Ok(())));
}
#[test]
fn complete_parseable_protocol_refusals() {
    let src = fixture();
    for (from, to) in [
        ("return $HF;", "return $HE;"),
        ("let $HG =", "const $HG ="),
        ("let $KEY_0='';", "const $KEY_0='';"),
        ("$HF != null", "$HF == null"),
        ("$HF.bypassResult != null", "$HF.bypassResult == null"),
        ("$HE.args == null", "$HE.args != null"),
        ("$HE.args.length < 2", "$HE.args.length < 1"),
        ("$HE.args[0] != null", "$HE.args[1] != null"),
        ("$HE.args[0].toLowerCase()", "$HE.args[1].toLowerCase()"),
        (".toLowerCase()", ".toUpperCase()"),
        (
            "\"\" + $HE.args[0].toLowerCase()",
            "$HE.args[0].toLowerCase()",
        ),
        ("$KEY_0 = $HE.args[1]", "$KEY_0 = $HE.args[0]"),
        ("writable: true", "writable: false"),
        ("configurable: true", "configurable: false"),
        ("enumerable: false", "enumerable: true"),
        ("method: $HG", "method: $KEY_0"),
        ("url: $KEY_0", "url: $HG"),
        ("$HH.value = $HG", "$HH.value = $KEY_0"),
        (
            "Object.defineProperty($HE.thisObj",
            "Object.defineProperty($HE.args",
        ),
        ("$KEY_145[0]", "$KEY_145[1]"),
        ("\"2.31:\" + e", "\"2.31:\" + $HE"),
        ("$KEY_11($HI)", "$KEY_11($HE)"),
        ("$KEY_11($HI)", "log($HI)"),
        ("let $HI = {}", "let $HI = { message: e }"),
        ("$HG = {", "extra(); $HG = {"),
        (
            "Object.defineProperty($HE.thisObj, $KEY_145[0], $HH)",
            "Object.defineProperty($HE.thisObj, $KEY_145[0], {...$HH})",
        ),
        (
            "$KEY_0 = $HE.args[1];",
            "let reordered = $KEY_0; $KEY_0 = $HE.args[1];",
        ),
        (
            "$HH.value = $HG;",
            "$HH.value = {method: $HG, url: $KEY_0};",
        ),
        ("$KEY_11($HI);", "$KEY_11($HI); throw e;"),
        ("$HH.value = $HG;", "extra(); $HH.value = $HG;"),
    ] {
        assert!(src.contains(from), "mutation {from}");
        let changed = src.replacen(from, to, 1);
        inspect(&changed, |m| {
            assert_eq!(sites(m).len(), 1);
            assert!(result(m, sites(m)[0]).is_err(), "accepted {from}");
        });
    }
}
#[test]
fn readiness_writers_and_escape_refusals() {
    let src = fixture();
    for changed in [
        src.replace(
            "let $KEY_145=['opaque','alternate'];",
            "let alias=setup; alias(); let $KEY_145=['opaque','alternate'];",
        ),
        src.replace(
            "setup(); } root();",
            "$KEY_145.length=0; setup(); } root();",
        ),
        src.replace("let $KEY_0='';", "publish($THREAD_9_65801); let $KEY_0='';"),
        src.replace("let $KEY_145=['opaque','alternate'];", "let $KEY_145;"),
        src.replace(
            "let $KEY_145=['opaque','alternate'];",
            "let $KEY_145; setup(); $KEY_145=['opaque'];",
        ),
        src.replace(
            "setup(); } root();",
            "setup(); $KEY_145=['late']; } root();",
        ),
        src.replace(
            "publish($THREAD_9_65801);",
            "escape($KEY_0); publish($THREAD_9_65801);",
        ),
        src.replace("setup(); } root();", "expose($KEY_145); setup(); } root();"),
        src.replace("let $KEY_11=log;", "let $KEY_11=log; $KEY_11=other;"),
        src.replace(
            "let $KEY_145=['opaque','alternate'];",
            "setup(); let $KEY_145=['opaque','alternate'];",
        ),
    ] {
        assert_ne!(changed, src, "nonvacuous mutation");
        inspect(&changed, |m| {
            assert_eq!(sites(m).len(), 1);
            assert!(result(m, sites(m)[0]).is_err(), "accepted {changed}");
        });
    }
}

#[test]
fn old_valid_metadata_label_and_opaque_diagnostic_variation() {
    let src = fixture();
    let old = src.replace("['opaque','alternate']", "['opaque']");
    let result = crate::naming::name_threads(&old, true).unwrap();
    let outcomes: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|o| o.evaluator.as_deref() == Some("shared_lowercase_method_url_metadata"))
        .collect();
    assert_eq!(outcomes.len(), 1);
    assert!(outcomes[0].applied);
    assert_eq!(
        outcomes[0].role_name.as_deref(),
        Some("storeLowercaseMethodAndUrlMetadata")
    );
    let changed = src.replace("2.31:", "own-diagnostic:");
    inspect(&changed, |m| {
        assert_eq!(super::tests::result(m, sites(m)[0]), Ok(()))
    });
}
#[test]
fn actual_full_pipeline_cardinality_names_only_initialized_third_callback() {
    let named = crate::naming::name_threads(ACTUAL, true).unwrap();
    assert_eq!(named.report.generated_thread_count, 364);
    for (a, b) in SPANS {
        let id = format!("fn@{a}..{b}");
        let found: Vec<_> = named
            .report
            .outcomes
            .iter()
            .filter(|o| o.function_id == id)
            .collect();
        assert_eq!(found.len(), 1);
        if a == SPANS[2].0 {
            assert!(found[0].applied);
            assert_eq!(
                found[0].proposed_name.as_deref(),
                Some("captureRequestOpenMetadata")
            );
            assert_eq!(
                found[0].evaluator.as_deref(),
                Some("network_request_open_metadata")
            );
        } else {
            assert!(!found[0].applied);
            assert!(found[0].proposed_name.is_none());
        }
    }
}

#[test]
fn ordered_effects_catch_return_and_callback_reentry_controls() {
    let src = fixture();
    for changed in [
        src.replace("$KEY_0 = $HE.args[1];", "").replace("$HH.value = $HG;", "$KEY_0 = $HE.args[1]; $HH.value = $HG;"),
        src.replace("url: $KEY_0", "url: reenter($KEY_0)"),
        src.replace("Object.defineProperty($HE.thisObj, $KEY_145[0], $HH)", "Object.defineProperty($HE.thisObj, $KEY_145[0], ($HH.value = {method: $HG, url: $KEY_0}, $HH))"),
        src.replace("$KEY_11($HI);", "$KEY_11($HI); return $HE;"),
        src.replace("$KEY_11($HI);", "$KEY_11($HI); e = $HE;"),
        src.replace("$HI.message = \"2.31:\" + e;", "$HI.message = e + \"2.31:\";"),
        src.replace("setup(); } root();", "for ($KEY_145[0] in external) {} setup(); } root();"),
    ] {
        assert_ne!(changed,src);
        inspect(&changed,|m|{assert_eq!(sites(m).len(),1);assert!(result(m,sites(m)[0]).is_err());});
    }
}

#[test]
fn request_factory_uniqueness_is_the_first_gate() {
    inspect(ACTUAL, |m| {
        // Mirror only the legacy factory gate for diagnostics. This does not
        // supply any evidence to the new evaluator.
        let mut candidates = Vec::new();
        for site in m.all_sites() {
            let has = |field: &str| {
                m.nodes.iter_enumerated().any(|(_,n)|site.span.contains_inclusive(n.kind().span()) && matches!(n.kind(),AstKind::StaticMemberExpression(v) if v.property.name==field))
            };
            if !["XMLHttpRequest", "onBeforeInvoke", "setRequestHeader"]
                .iter()
                .all(|k| has(k))
            {
                continue;
            }
            let mut pairs = BTreeSet::new();
            for (_, n) in m.nodes.iter_enumerated() {
                let AstKind::ObjectExpression(o) = n.kind() else {
                    continue;
                };
                if m.enclosing_function(n.id()).map(|s| s.span) != Some(site.span) {
                    continue;
                }
                let members = crate::signals::extract::bindings::object_members_of(o);
                let Some(init) = members
                    .iter()
                    .find(|e| e.name == "init")
                    .and_then(|e| m.function_from_expression(e.value))
                else {
                    continue;
                };
                let mut teardown = members
                    .iter()
                    .find(|e| e.name == "teardown")
                    .and_then(|e| m.function_from_expression(e.value));
                if teardown.is_none() {
                    let holder = m.nodes.iter_enumerated().find_map(|(_, n)| match n.kind() {
                        AstKind::VariableDeclarator(v)
                            if v.init.as_ref().is_some_and(|e| e.span() == o.span) =>
                        {
                            q::plain_binding(m, v)
                        }
                        _ => None,
                    });
                    if let Some(holder) = holder {
                        for (id, n) in m.nodes.iter_enumerated() {
                            if m.enclosing_function(id).map(|s| s.span) != Some(site.span) {
                                continue;
                            }
                            let AstKind::AssignmentExpression(a) = n.kind() else {
                                continue;
                            };
                            if matches!(&a.left,AssignmentTarget::StaticMemberExpression(v) if v.property.name=="teardown" && i::symbol(m,&v.object)==Some(holder))
                            {
                                teardown = m.function_from_expression(&a.right);
                            }
                        }
                    }
                }
                if let Some(t) = teardown {
                    pairs.insert((init.span.start, t.span.start));
                }
            }
            if pairs.len() >= 2 {
                candidates.push(site.span);
            }
        }
        assert!(
            candidates.len() > 1,
            "xhr_factory must refuse its nonunique candidates"
        );
        eprintln!("XHR uniqueness guard refuses {} actual factories: {:?}",candidates.len(),candidates);
    });
}

fn census_binding(m: &Model<'_, '_>, id: S) -> serde_json::Value {
    let declaration = m.scoping.symbol_declaration(id);
    let span = m.nodes.kind(declaration).span();
    let references=m.scoping.get_resolved_references(id).map(|r|{
        let node=r.node_id();let p=m.nodes.parent_id(node);let span=m.nodes.kind(node).span();let ps=m.nodes.kind(p).span();
        serde_json::json!({"span":[span.start,span.end],"write":r.is_write(),"parent_span":[ps.start,ps.end],"parent_source":m.exact(ps),"owner":m.enclosing_function(node).map(|s|[s.span.start,s.span.end])})
    }).collect::<Vec<_>>();
    serde_json::json!({"symbol_id":id.index(),"name":m.scoping.symbol_name(id),"declaration_span":[span.start,span.end],"declaration_source":m.exact(span),"initializer":m.initializer(id).map(|e|m.exact(e.span())),"writes":m.binding_writes(id),"references":references})
}
#[test]
fn actual_capture_census_proves_return_and_catch_resume_obstructions() {
    inspect(ACTUAL, |m| {
        let mut rows = Vec::new();
        let mut scratches = BTreeSet::new();
        let mut keys = BTreeSet::new();
        let mut loggers = BTreeSet::new();
        for site in sites(m) {
            let AstKind::Function(f) = m.nodes.kind(site.node_id) else {
                panic!()
            };
            let proof = body(m, site, f).unwrap();
            scratches.insert(proof.scratch);
            keys.insert(proof.key);
            loggers.insert(proof.logger);
            assert_eq!(m.binding_writes(proof.key), 1);
            assert_eq!(m.binding_writes(proof.logger), 4);
            assert!(m.initializer(proof.key).is_none());
            assert!(m.initializer(proof.logger).is_none());
            let setup = m.enclosing_function(site.node_id).unwrap();
            let factory = m.enclosing_function(setup.node_id).unwrap();
            let root = m.enclosing_function(factory.node_id).unwrap();
            assert_eq!(root.span, Span::new(325, 766233));
            let mut writer_spans = Vec::new();
            let mut catch_paths = Vec::new();
            for r in m
                .scoping
                .get_resolved_references(proof.logger)
                .filter(|r| r.is_write())
            {
                let assignment = m.nodes.parent_id(r.node_id());
                let AstKind::AssignmentExpression(a) = m.nodes.kind(assignment) else {
                    panic!()
                };
                writer_spans.push(a.span.start);
                if a.span.start == 44310 {
                    continue;
                }
                let (flag,snapshot,guard)=boolean_null_arm(m,assignment).expect("boolean producer and every writer/copy prove strict null arm impossible, not just its return");
                assert_eq!(guard, Span::new(65331, 515847));
                assert_eq!(m.binding_writes(flag), 1);
                assert_eq!(m.binding_writes(snapshot), 0);
                assert!(
                    matches!(m.initializer(flag),Some(Expression::BooleanLiteral(v)) if !v.value)
                );
                let mut node = assignment;
                let mut found = None;
                for _ in 0..32 {
                    let parent = m.nodes.parent_id(node);
                    if let AstKind::TryStatement(t) = m.nodes.kind(parent) {
                        if t.span == Span::new(64914, 515875) {
                            let catch = t.handler.as_ref().unwrap();
                            assert!(catch.body.body.is_empty());
                            assert!(t.finalizer.is_none());
                            found = Some(
                                serde_json::json!({"write":[a.span.start,a.span.end],"try":[t.span.start,t.span.end],"empty_catch":[catch.span.start,catch.span.end],"caught_abrupt_completion_could_resume_if_guard_were_feasible":true}),
                            );
                            break;
                        }
                    }
                    assert_ne!(parent, node);
                    node = parent;
                }
                catch_paths.push(
                    found.expect("every numeric store shares the exact resuming empty catch"),
                );
            }
            writer_spans.sort();
            assert_eq!(writer_spans, vec![44310, 69190, 69339, 72455]);
            assert_eq!(catch_paths.len(), 3);
            let function_reads = m
                .scoping
                .get_resolved_references(factory.binding_symbol.unwrap())
                .collect::<Vec<_>>();
            assert_eq!(function_reads.len(), 1);
            let publication = function_reads[0].node_id();
            let array = m.nodes.parent_id(publication);
            assert!(
                matches!(m.nodes.kind(array),AstKind::ArrayExpression(a) if a.elements.len()==1)
            );
            let assignment = m.nodes.parent_id(array);
            let AstKind::AssignmentExpression(a) = m.nodes.kind(assignment) else {
                panic!()
            };
            let registry = q::target_symbol(m, a).unwrap();
            let retained = m
                .scoping
                .get_resolved_references(site.binding_symbol.unwrap())
                .find_map(|r| match m.nodes.parent_kind(r.node_id()) {
                    AstKind::AssignmentExpression(a)
                        if i::symbol(m, &a.right) == site.binding_symbol =>
                    {
                        q::target_symbol(m, a)
                    }
                    _ => None,
                })
                .unwrap();
            let numeric_reference = m
                .scoping
                .get_resolved_references(proof.logger)
                .find(|r| r.is_write() && m.nodes.kind(r.node_id()).span().start == 72455)
                .unwrap();
            let (flag, snapshot, guard) =
                boolean_null_arm(m, m.nodes.parent_id(numeric_reference.node_id())).unwrap();
            let producer = serde_json::json!({"flag":census_binding(m,flag),"snapshot":census_binding(m,snapshot),"strict_null_guard":[guard.start,guard.end],"arm_feasible":false});
            rows.push(serde_json::json!({"callback":[site.span.start,site.span.end],"setup":[setup.span.start,setup.span.end],"factory":[factory.span.start,factory.span.end],"root":[root.span.start,root.span.end],"key":census_binding(m,proof.key),"logger":census_binding(m,proof.logger),"url_scratch":census_binding(m,proof.scratch),"callback_binding":census_binding(m,site.binding_symbol.unwrap()),"setup_binding":census_binding(m,setup.binding_symbol.unwrap()),"factory_binding":census_binding(m,factory.binding_symbol.unwrap()),"factory_registry":census_binding(m,registry),"retained_callback":census_binding(m,retained),"catch_resume_paths":catch_paths,"boolean_producer":producer,"result":format!("{:?}",result(m,site))}));
        }
        assert_eq!(scratches.len(), 3);
        assert_eq!(keys.len(), 1);
        assert_eq!(loggers.len(), 1);
    });
}
fn boolean_epoch_fixture() -> String {
    fixture().replace("let $KEY_11=log;", "let $KEY_11=log; try { let flag=false; function exchange(input) { flag=true; } opaquePort({exchange}); let snapshot=flag; if(snapshot === null) { $KEY_11=7; runtimePort(); return alternateResult; } } catch (caught) {}")
}
#[test]
fn boolean_copy_epochs_close_with_opaque_reentrant_or_retained_ports() {
    let src = boolean_epoch_fixture();
    for changed in [
        src.clone(),
        src.replace("flag=false", "flag=true"),
        src.replace("flag=true;", "flag=false;"),
        src.replace("let snapshot=flag", "const snapshot=flag"),
        src.replace("flag", "readyFlag")
            .replace("snapshot", "copiedFlag"),
    ] {
        inspect(&changed, |m| {
            let found = sites(m);
            assert_eq!(found.len(), 1);
            let AstKind::Function(f) = m.nodes.kind(found[0].node_id) else {
                panic!()
            };
            let proof = body(m, found[0], f).unwrap();
            assert!(capture_ready(m, found[0], proof.logger, false).is_none());
            assert!(logger_with_boolean_dead_stores(m, found[0], proof.logger).is_some());
            assert_eq!(result(m, found[0]), Ok(()));
        });
    }
}
#[test]
fn feasible_guards_wrong_copy_late_writes_and_dynamic_scope_refuse() {
    let src = boolean_epoch_fixture();
    for changed in [
        src.replace("flag=false", "flag=null"),
        src.replace("flag=true;", "flag=0;"),
        src.replace("let snapshot=flag;", "let snapshot=flag; snapshot=null;"),
        src.replace("let snapshot=flag;", "let snapshot=otherFlag;"),
        src.replace("snapshot === null", "snapshot == null"),
        src.replace(
            "let snapshot=flag;",
            "eval('flag=null'); let snapshot=flag;",
        ),
        src.replace("setup(); } root();", "setup(); $KEY_11=8; } root();"),
        src.replace(
            "setup(); } root();",
            "setup(); function later() { $KEY_11=8; } retain(later); } root();",
        ),
        src.replace(
            "let $KEY_145=['opaque','alternate'];",
            "let $KEY_145; retain(setup); $KEY_145=['opaque','alternate'];",
        ),
    ] {
        assert_ne!(changed, src);
        inspect(&changed, |m| {
            let found = sites(m);
            assert_eq!(found.len(), 1);
            let AstKind::Function(f) = m.nodes.kind(found[0].node_id) else {
                panic!()
            };
            assert!(body(m, found[0], f).is_some());
            assert!(result(m, found[0]).is_err(), "accepted {changed}");
        });
    }
}
#[test]
fn actual_context_alpha_preserves_exactly_one_initialized_callback() {
    let source = ACTUAL
        .replace("$KEY_145", "opaqueKeys")
        .replace("$KEY_11", "diagnosticPort")
        .replace("$KEY_63", "producerFlag");
    assert_ne!(source, ACTUAL);
    inspect(&source, |m| {
        let found = sites(m);
        assert_eq!(found.len(), 3);
        for (index, site) in found.into_iter().enumerate() {
            let AstKind::Function(f) = m.nodes.kind(site.node_id) else {
                panic!()
            };
            assert!(body(m, site, f).is_some());
            assert_eq!(result(m, site).is_ok(), index == 2);
        }
    });
}
