use shape::naming::collectors::identify_collectors;
use shape::naming::name_threads;

const TEMPLATE: &str = r#"function main(){
  const registryContext = {kind: "test"};
  function $THREAD_1_101(emitters){ unknown(emitters); }
  function semanticPurpose(emitters){ return emitters.length; }
  function keepMeaningfulName(value){ return value + 17; }
  function coordinate(emitters, factory, context, init, collect, flag){
    factory(emitters, context, init, collect, function(){}, function(){});
  }
  REGISTRATION
  const publicGroups = grouped;
  function start(){
    publicGroups.forEach((factory, index)=>{
      coordinate([], factory, {}, function(){}, function(){}, false);
    });
  }
  start();
  return {keepMeaningfulName, start, registryContext};
}main();"#;

const TRANSPORT: &str = r#"let grouped = [$THREAD_1_101];
  grouped[2] = ((array, index, first, second)=>{
    array[index] = first;
    return second;
  })(grouped, 1, (emitters)=>{ return this === arguments[0] ? emitters : null; },
     (emitters)=>{ return emitters; });
  grouped[3] = semanticPurpose;"#;

fn named_pair() -> String {
    TEMPLATE.replace(
        "REGISTRATION",
        "let grouped = [$THREAD_1_101, semanticPurpose];",
    )
}

fn no_wave(source: &str) {
    let result = identify_collectors(source, true).unwrap();
    assert!(
        !result.report.renames.iter().any(|r| r.applied
            && r.proposed_name
                .as_ref()
                .is_some_and(|n| n.starts_with("waveCollector"))),
        "{:?}",
        result.report.renames
    );
    assert_eq!(result.source, source);
}

#[test]
fn registry_positions_name_missing_emitters_and_migrate_semantics() {
    let source = named_pair();
    let result = name_threads(&source, true).unwrap();
    assert!(result.report.collectors.wiring.groups.is_empty());
    assert_eq!(result.report.collectors.registered_groups.len(), 2);
    assert!(result
        .source
        .contains("let grouped = [waveCollector1, waveCollector2]"));
    assert!(result.source.contains("function keepMeaningfulName("));
    assert!(result
        .report
        .aggregate_collector_claims
        .iter()
        .any(|c| c.purpose_names.iter().any(|n| n == "semanticPurpose")));
    assert!(result
        .report
        .outcomes
        .iter()
        .filter(|r| r.applied && r.evaluator.as_deref() == Some("collector_registry"))
        .all(|r| r.reference_edits > 0));
    assert_eq!(
        name_threads(&result.source, true).unwrap().source,
        result.source
    );
}

#[test]
fn source_order_alpha_and_transposed_slots_follow_registry_position() {
    let source = named_pair();
    let changed = source
        .replace(
            "[$THREAD_1_101, semanticPurpose]",
            "[semanticPurpose, $THREAD_1_101]",
        )
        .replace("$THREAD_1_101", "$THREAD_900_77_2")
        .replace("grouped", "renamedArray")
        .replace("coordinate", "renamedCoordinator")
        .replace("publicGroups", "renamedAlias");
    let result = identify_collectors(&changed, true).unwrap();
    assert!(result
        .source
        .contains("function waveCollector2(emitters){ unknown(emitters); }"));
    assert!(result
        .source
        .contains("function waveCollector1(emitters){ return emitters.length; }"));
    assert!(result
        .source
        .contains("let renamedArray = [waveCollector1, waveCollector2]"));
    let reordered = source
        .replace("function $THREAD_1_101(emitters){ unknown(emitters); }", "")
        .replace(
            "function semanticPurpose",
            "function $THREAD_1_101(emitters){ unknown(emitters); }\n function semanticPurpose",
        );
    assert!(identify_collectors(&reordered, true)
        .unwrap()
        .source
        .contains("let grouped = [waveCollector1, waveCollector2]"));
}

#[test]
fn anonymous_transport_origins_are_named_without_replacing_arrows() {
    let source = TEMPLATE.replace("REGISTRATION", TRANSPORT);
    let result = name_threads(&source, true).unwrap();
    assert_eq!(result.report.collectors.registered_groups.len(), 4);
    assert_eq!(
        result
            .report
            .collectors
            .renames
            .iter()
            .filter(|r| r.applied && r.binding.is_none())
            .count(),
        2
    );
    assert!(result.source.contains(
        "const waveCollector2 = (emitters)=>{ return this === arguments[0] ? emitters : null; };"
    ));
    assert!(result
        .source
        .contains("const waveCollector3 = (emitters)=>{ return emitters; };"));
    assert_eq!(result.report.collectors.anonymous_naming_edits.len(), 4);
    assert_eq!(
        name_threads(&result.source, true).unwrap().source,
        result.source
    );
    let once = identify_collectors(&source, true).unwrap();
    assert_eq!(
        identify_collectors(&once.source, true).unwrap().source,
        once.source
    );
    // Original bytes inside each arrow survive, including lexical arguments.
    assert_eq!(
        result
            .source
            .matches("return this === arguments[0] ? emitters : null;")
            .count(),
        1
    );
}

#[test]
fn shared_transport_declaration_is_read_like_its_iife() {
    let registration = r#"function store(array, index, first, second){
    array[index] = first;
    return second;
  }
  let grouped = [$THREAD_1_101];
  grouped[2] = store(grouped, 1, (emitters)=>{ return this === arguments[0] ? emitters : null; },
     (emitters)=>{ return emitters; });
  grouped[3] = semanticPurpose;"#;
    let source = TEMPLATE.replace("REGISTRATION", registration);
    let result = name_threads(&source, true).unwrap();
    assert_eq!(result.report.collectors.registered_groups.len(), 4);
    assert!(result.report.collectors.unresolved.is_empty());
    assert!(result.source.contains("function waveCollector1(emitters)"));
    assert!(result.source.contains("const waveCollector2 ="));
    assert!(result.source.contains("const waveCollector3 ="));
    assert_eq!(
        name_threads(&result.source, true).unwrap().source,
        result.source
    );

    // A rebound helper proves nothing about what the call writes.
    let rebound = source.replace(
        "let grouped = [",
        "store = function(){};\n  let grouped = [",
    );
    let result = name_threads(&rebound, true).unwrap();
    assert!(result.source.contains("$THREAD_1_101"));
    assert!(!result.source.contains("waveCollector"));
}

#[test]
fn nested_and_sibling_donors_cannot_prove_consumer_or_coordinator() {
    let source = named_pair();
    no_wave(&source.replace(
        "coordinate([], factory, {}, function(){}, function(){}, false);",
        "function unusedDonor(){coordinate([], factory, {}, function(){}, function(){}, false);}",
    ));
    no_wave(&source.replace(
        "factory(emitters, context, init, collect, function(){}, function(){});",
        "function unusedDonor(){factory(emitters, context, init, collect, function(){}, function(){});}",
    ));
    no_wave(&source.replace(
        "factory(emitters, context, init, collect, function(){}, function(){});",
        "other(factory); function sibling(factory){factory(emitters, context, init, collect, function(){}, function(){});}",
    ));
    no_wave(&source.replace(
        "coordinate([], factory, {}, function(){}, function(){}, false);",
        "function sibling(factory){coordinate([], factory, {}, function(){}, function(){}, false);}",
    ));
}

#[test]
fn shared_binding_mutation_escapes_and_shadowing_are_refused() {
    let source = named_pair();
    let shared = source.replace(
        "[$THREAD_1_101, semanticPurpose]",
        "[$THREAD_1_101, $THREAD_1_101]",
    );
    let report = identify_collectors(&shared, true).unwrap();
    assert!(report.report.renames.iter().any(|r| r
        .reason
        .as_deref()
        .is_some_and(|r| r.contains("shared factory"))));
    assert_eq!(report.source, shared);
    for mutation in [
        "grouped = [];",
        "grouped[0] = semanticPurpose;",
        "grouped[dynamic] = semanticPurpose;",
        "grouped[0]++;",
        "delete grouped[0];",
        "grouped.push(semanticPurpose);",
        "external(grouped);",
        "let escaped = grouped; external(escaped);",
        "let escaped = grouped; escaped = [];",
        "function mutate(){grouped[0] = semanticPurpose;}",
    ] {
        no_wave(&source.replace(
            "const publicGroups",
            &format!("{mutation}\n const publicGroups"),
        ));
    }
    no_wave(&source.replace(
        "(factory, index)=>{",
        "(factory, index)=>{ factory = other;",
    ));
    no_wave(&source.replace("factory(emitters, context, init, collect, function(){}, function(){});",
        "arguments[1] = other; arguments[1](emitters, context, init, collect, function(){}, function(){});"));
    no_wave(&source.replace("function start(){", "function start(publicGroups){"));
}

#[test]
fn later_origins_and_writes_after_consumption_do_not_donate_values() {
    let source = named_pair();
    no_wave(&source.replace(
        "let grouped = [$THREAD_1_101, semanticPurpose];",
        "let later; let grouped = [later]; later = $THREAD_1_101;",
    ));
    no_wave(&source.replace(
        "const publicGroups = grouped;",
        "const publicGroups = grouped; start(); grouped[2] = semanticPurpose;",
    ));
    no_wave(&source.replace(
        "const publicGroups = grouped;",
        "let publicGroups; start(); publicGroups = grouped;",
    ));
    no_wave(
        &source
            .replace(
                "const publicGroups = grouped;",
                "let publicGroups; const stale = publicGroups; publicGroups = grouped;",
            )
            .replace("publicGroups.forEach", "stale.forEach"),
    );
    no_wave(&source.replace("const publicGroups = grouped;", "const publicGroups = grouped; function early(){start();} early(); grouped[2] = semanticPurpose;"));
}

#[test]
fn transport_requires_plain_synchronous_nonoptional_unconditional_syntax() {
    let source = TEMPLATE.replace("REGISTRATION", TRANSPORT);
    for changed in [
        source.replace(
            "((array, index, first, second)=>",
            "(async (array, index, first, second)=>",
        ),
        source.replace(
            "((array, index, first, second)=>",
            "((array, index, first, second = other)=>",
        ),
        source.replace(
            "array[index] = first;",
            "\"use strict\"; array[index] = first;",
        ),
        source.replace("})(grouped, 1", "})?.(grouped, 1"),
        source.replace("array[index] = first;", "if (flag) array[index] = first;"),
        source.replace(
            "((array, index, first, second)=>{",
            "(function* (array, index, first, second){",
        ),
    ] {
        no_wave(&changed);
    }
    let donor = TEMPLATE.replace("REGISTRATION", &format!(
        "let grouped = [$THREAD_1_101]; function unused(){{ {} }}",
        TRANSPORT.replace("let grouped = [$THREAD_1_101];", "")
    ));
    no_wave(&donor);
}

#[test]
fn anonymous_capture_collision_and_dynamic_scope_are_reported_exactly() {
    let source = TEMPLATE.replace("REGISTRATION", TRANSPORT).replace(
        "return this === arguments[0] ? emitters : null;",
        "return waveCollector2(emitters);",
    );
    let result = identify_collectors(&source, true).unwrap();
    assert!(result
        .report
        .renames
        .iter()
        .any(|r| r.proposed_name.as_deref() == Some("waveCollector2")
            && !r.applied
            && r.reason.as_deref().is_some_and(|s| s.contains("capture"))));
    assert!(!result.source.contains("const waveCollector2"));
    assert!(result.source.contains("return waveCollector2(emitters);"));
    let parameter = TEMPLATE.replace("REGISTRATION", TRANSPORT)
        .replace("(emitters)=>{ return this", "(emitters = waveCollector2)=>{ return this");
    assert!(identify_collectors(&parameter, true).unwrap().report.renames.iter().any(|r|
        r.proposed_name.as_deref() == Some("waveCollector2") && !r.applied
            && r.reason.as_deref().is_some_and(|r| r.contains("capture"))));
    let dynamic = TEMPLATE
        .replace("REGISTRATION", TRANSPORT)
        .replace("start();", "eval(code);start();");
    assert!(identify_collectors(&dynamic, true)
        .unwrap()
        .report
        .renames
        .iter()
        .filter(|r| r.binding.is_none())
        .all(|r| !r.applied));
}

#[test]
fn same_shape_different_bindings_keep_their_distinct_slots() {
    let source = named_pair().replace("return emitters.length;", "unknown(emitters);");
    let result = name_threads(&source, true).unwrap();
    assert!(result
        .source
        .contains("function waveCollector1(emitters){ unknown(emitters); }"));
    assert!(result
        .source
        .contains("function waveCollector2(emitters){ unknown(emitters); }"));
    assert!(result.source.contains("[waveCollector1, waveCollector2]"));
}

#[test]
fn anonymous_holder_cannot_capture_a_simultaneous_registry_rename() {
    let source = named_pair().replace("let grouped = [$THREAD_1_101, semanticPurpose];",
        "let grouped = [(e)=>$THREAD_1_101(e)]; let another = [$THREAD_1_101]; another.forEach((f,i)=>{coordinate([],f,{},function(){},function(){},false);});");
    let result = identify_collectors(&source, true).unwrap();
    assert!(result.report.renames.iter().any(|r| r.binding.is_none()
        && !r.applied
        && r.reason
            .as_deref()
            .is_some_and(|r| r.contains("simultaneous"))));
    assert!(!result.source.contains("const waveCollector1"));
}

#[test]
fn four_argument_hooks_wrong_consumers_and_direct_callbacks_are_not_grouped() {
    let source = named_pair();
    no_wave(&source.replace(
        "factory(emitters, context, init, collect, function(){}, function(){});",
        "factory(emitters, context, init, collect);",
    ));
    no_wave(&source.replace(
        "coordinate([], factory, {}, function(){}, function(){}, false);",
        "factory([], {}, function(){}, function(){});",
    ));
    no_wave(&source.replace(
        "coordinate([], factory, {}, function(){}, function(){}, false);",
        "unrelated([], factory, {}, function(){}, function(){}, false);",
    ));
    no_wave(&source.replace(
        "let grouped = [$THREAD_1_101, semanticPurpose];",
        "let grouped = [{ctor:$THREAD_1_101}, {ctor:semanticPurpose}];",
    ));
    no_wave(&source.replace("publicGroups.forEach", "publicGroups.map"));
}

#[test]
fn scoped_reference_edits_keep_strings_comments_and_shadowed_functions() {
    let source = named_pair().replace("REGISTRATION", "")
        .replace("const publicGroups", "// $THREAD_1_101\n const text = '$THREAD_1_101';\n function shadow(){function $THREAD_1_101(){return 23;} return $THREAD_1_101;}\n const publicGroups");
    let result = identify_collectors(&source, true).unwrap();
    assert!(result.source.contains("// $THREAD_1_101"));
    assert!(result.source.contains("'$THREAD_1_101'"));
    assert!(result
        .source
        .contains("function shadow(){function $THREAD_1_101(){return 23;} return $THREAD_1_101;}"));
    assert!(result
        .source
        .contains("let grouped = [waveCollector1, waveCollector2]"));
    let mut restored = result.source.clone();
    for rename in result.report.renames.iter().filter(|r| r.applied) {
        restored = restored.replace(
            rename.proposed_name.as_ref().unwrap(),
            &rename.binding.as_ref().unwrap().name,
        );
    }
    assert_eq!(restored, source);
}

#[test]
fn independent_origins_use_all_registry_positions() {
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/program/wave-position/expected.json")).unwrap();
    for (key, source) in [
        (
            "wave-position-heldout",
            include_str!("fixtures/program/wave-position/wave-position-heldout.js"),
        ),
        (
            "captured-query-baseline",
            include_str!("fixtures/program/wave-position/wave-position-baseline.js"),
        ),
        (
            "wave-position-alternate",
            include_str!("fixtures/program/wave-position/wave-position-alternate.js"),
        ),
    ] {
        let seed = expected
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["key"] == key)
            .unwrap();
        let roles = seed["roles"].as_array().unwrap();
        let result = name_threads(source, true).unwrap();
        assert_eq!(
            result.report.collectors.registered_groups.len(),
            roles.len(),
            "{key}"
        );
        for expected in roles {
            let id = expected["function_id"].as_str().unwrap();
            let role = expected["role"].as_str().unwrap();
            let group = result
                .report
                .collectors
                .registered_groups
                .iter()
                .find(|g| {
                    g.function
                        .as_ref()
                        .is_some_and(|f| format!("fn@{}..{}", f.span.start, f.span.end) == id)
                })
                .unwrap_or_else(|| panic!("{key} {id}: registry origin missing"));
            assert!(!result.report.removals.iter().any(|r| {
                let span = group.function.as_ref().unwrap().span;
                r.span.start <= span.start && span.end <= r.span.end
            }), "{key} {id}: registry function must be retained");
            assert!(
                group.unresolved.is_empty(),
                "{key} {id}: {:?}",
                group.unresolved
            );
            assert_eq!(group.slot as u64, expected["slot"].as_u64().unwrap());
            if expected["anonymous"] == true {
                assert!(
                    result
                        .report
                        .collectors
                        .renames
                        .iter()
                        .any(|r| r.function_id == id
                            && r.applied
                            && r.proposed_name.as_deref() == Some(role)),
                    "{key} {id} {role}"
                );
                assert!(result.source.contains(&format!("const {role} = ")));
            } else {
                assert!(
                    result.report.outcomes.iter().any(|r| r.function_id == id
                        && r.applied
                        && r.proposed_name.as_deref() == Some(role)),
                    "{key} {id} {role}"
                );
            }
            if let Some(old) = expected["prior_semantic_label"].as_str() {
                assert!(
                    result.report.aggregate_collector_claims.iter().any(|c| {
                        c.function_span == group.function.as_ref().unwrap().span
                            && c.purpose_names.iter().any(|p| p == old)
                    }),
                    "{key} {id}: missing prior semantic metadata {old}"
                );
            }
        }
    }
}
