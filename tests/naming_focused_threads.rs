use shape::naming::collectors::identify_collectors;
use shape::naming::name_threads;
const ROUTES: &str = include_str!("fixtures/program/focused-threads.js");

#[test]
fn wave_direct_identity_and_payload_indices_are_distinct() {
    let result = identify_collectors(ROUTES, true).unwrap();
    let report = &result.report;
    assert_eq!(report.wiring.groups.len(), 2);
    assert_eq!(report.wiring.groups[0].row, vec![2, 0]);
    assert_eq!(report.wiring.direct_routes.len(), 1);
    assert_eq!(report.wiring.direct_routes[0].descriptor_index, 3);
    assert!(
        report.payload_routing.unresolved.is_empty(),
        "{:?}",
        report.payload_routing.unresolved
    );
    let slots = &report.payload_routing.slots;
    assert_eq!(
        slots.iter().map(|s| s.descriptor_index).collect::<Vec<_>>(),
        vec![2, 1, 3, 0, 4]
    );
    assert_eq!(
        slots.iter().map(|s| s.positional_index).collect::<Vec<_>>(),
        vec![Some(0), None, Some(1), Some(2), None]
    );
    assert_eq!(slots[1].route, "keyed");
    assert_eq!(slots[4].route, "named");
    let original = report
        .functions
        .iter()
        .find(|f| f.thread_id == Some(1))
        .unwrap();
    assert_eq!(original.pc, Some(101));
    assert!(original.enclosing_function.is_some());
    assert!(result.source.contains("function waveCollector1("));
    assert!(result
        .source
        .contains("let waves = [waveCollector1, waveCollector2]"));
    assert!(result
        .source
        .contains("function directCollectorDescriptor3("));
    // Inverting only the applied collector binding names reproduces every byte.
    let mut restored = result.source;
    for r in &report.renames {
        if r.applied {
            restored = restored.replace(
                r.proposed_name.as_ref().unwrap(),
                &r.binding.as_ref().unwrap().name,
            );
        }
    }
    assert_eq!(restored, ROUTES);
}
#[test]
fn shared_factory_is_mapped_per_route_but_not_forcibly_named() {
    let source = ROUTES.replace(
        "[$THREAD_1_101, $THREAD_2_202]",
        "[$THREAD_1_101, $THREAD_1_101]",
    );
    let result = identify_collectors(&source, true).unwrap();
    assert_eq!(result.report.wiring.groups.len(), 2);
    assert!(result.report.renames.iter().any(|r| r
        .reason
        .as_deref()
        .is_some_and(|s| s.contains("shared factory"))));
    assert!(result.source.contains("function $THREAD_1_101("));
}
#[test]
fn unsupported_partition_and_dynamic_descriptor_stay_unresolved() {
    let source = ROUTES.replace("i++", "i += 2");
    let report = identify_collectors(&source, false).unwrap().report;
    assert!(report.payload_routing.slots.is_empty());
    assert!(!report.payload_routing.unresolved.is_empty());
    let source = ROUTES.replace(
        "{ctor:null}, {ctor:null, name:17}",
        "{ctor:null, name:unknown}, {ctor:null, name:17}",
    );
    let report = identify_collectors(&source, false).unwrap().report;
    assert_eq!(report.payload_routing.slots[3].route, "unresolved");
}
#[test]
fn anonymous_direct_collector_is_reported_without_inventing_a_binding() {
    let source = ROUTES.replace(
        "wrap($THREAD_9_900)",
        "wrap(observer => { observer.next(1); })",
    );
    let result = identify_collectors(&source, true).unwrap();
    assert!(result.report.renames.iter().any(|r| r
        .reason
        .as_deref()
        .is_some_and(|s| s.contains("no editable binding"))));
    assert!(result
        .source
        .contains("wrap(observer => { observer.next(1); })"));
}
#[test]
fn function_evaluators_preserve_readable_keys_comments_strings_and_scopes() {
    let source = r#"function main(){
      let meaningfulKey = 3;
      function keepHumanName(){return true;}
      function $THREAD_1_10(x){return x;}
      function $THREAD_2_20(){return false;}
      function $THREAD_3_30(x){ sink.error(x); }
      function $THREAD_4_40(){ }
      function $THREAD_5_50(x){ return mystery(x); }
      const labels = "$THREAD_1_10 meaningfulKey"; // $THREAD_1_10
      let refs = {$THREAD_1_10};
      function child(){function $THREAD_1_10(x){return mystery(x);};return $THREAD_1_10(meaningfulKey);}
      return [$THREAD_1_10, $THREAD_2_20, $THREAD_3_30, $THREAD_4_40, $THREAD_5_50, keepHumanName,child,refs,labels];
    } main();"#;
    let result = name_threads(source, true).unwrap();
    assert!(result.source.contains("function identity(x){return x;}"));
    assert!(result
        .source
        .contains("function returnFalse(){return false;}"));
    assert!(result
        .source
        .contains("function forwardError(x){ sink.error(x); }"));
    assert!(result.source.contains("let meaningfulKey = 3;"));
    assert!(result
        .source
        .contains("function keepHumanName(){return true;}"));
    assert!(result
        .source
        .contains("const labels = \"$THREAD_1_10 meaningfulKey\"; // $THREAD_1_10"));
    assert!(result.source.contains("{$THREAD_1_10: identity}"));
    assert!(result.source.contains("function child(){function $THREAD_1_10(x){return mystery(x);};return $THREAD_1_10(meaningfulKey);}"));
    // A referenced, stable, empty zero-arity declaration is a proven no-op
    // (`referenced_ordinary_noop`); only unreferenced empty stubs are deferred.
    assert!(result.source.contains("function noOp(){ }"));
    assert!(result.report.outcomes.iter().any(|o| o.binding.name == "$THREAD_4_40"
        && o.applied
        && o.evaluator.as_deref() == Some("referenced_ordinary_noop")));
    assert!(result.source.contains("function $THREAD_5_50(x){ return mystery(x); }"));
    assert_eq!(result.report.applied_count, 4);
    assert!(result
        .report
        .outcomes
        .iter()
        .any(|o| o.shorthand_edits == 1));
}
#[test]
fn collisions_mutations_and_equal_scope_clones_are_named_without_deletion() {
    let source = r#"function main(){let identity=1;
    function $THREAD_1_10(x){return x;}
    function $THREAD_2_20(){return false;}
    function $THREAD_3_30(){return false;}
    function $THREAD_4_40(x){return x;}
    $THREAD_4_40 = other;
    return [$THREAD_1_10,$THREAD_2_20,$THREAD_3_30,$THREAD_4_40,identity];} main();"#;
    let result = name_threads(source, true).unwrap();
    // A readable existing binding still refuses its own spelling.
    assert!(result
        .report
        .outcomes
        .iter()
        .any(|o| o.reason.as_deref() == Some("NameCollision")));
    // Two positively identified equal-scope clones get distinct concrete names
    // instead of both being refused by the simultaneous-collision guard.
    let clones: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|o| o.binding.name == "$THREAD_2_20" || o.binding.name == "$THREAD_3_30")
        .collect();
    assert_eq!(clones.len(), 2);
    assert!(clones.iter().all(|o| o.applied));
    let mut concrete: Vec<_> = clones
        .iter()
        .filter_map(|o| o.proposed_name.clone())
        .collect();
    concrete.sort();
    assert_eq!(
        concrete,
        vec!["returnFalse".to_string(), "returnFalse2".to_string()]
    );
    assert!(result.source.contains("function returnFalse(){return false;}"));
    assert!(result
        .source
        .contains("function returnFalse2(){return false;}"));
    assert!(result
        .report
        .outcomes
        .iter()
        .any(|o| o.reason.as_deref() == Some("function binding is reassigned")));
}
#[test]
fn report_only_and_invalid_input_never_rewrite() {
    let result = name_threads(ROUTES, false).unwrap();
    assert_eq!(result.source, ROUTES);
    assert!(name_threads("function {", true).is_err());
    assert!(identify_collectors("", false).is_err());
}
#[test]
fn api_calls_are_scoped_and_nested_body_evidence_does_not_leak() {
    let source = r#"function main(){
    const Object={assign:{apply:other}};
    function $THREAD_1_1(){return Object.assign.apply(Object, arguments);}
    function $THREAD_2_2(){function child(){return Math.random();} return child;}
    return [$THREAD_1_1,$THREAD_2_2];} main();"#;
    assert_eq!(name_threads(source, true).unwrap().source, source);
}
#[test]
fn resolved_api_alias_loops_and_recursion_use_only_identifier_edits() {
    let source = r#"function main(){const marker="é";
    let own = Object.prototype.hasOwnProperty;
    function $THREAD_1_10(a,b){return own.call(a,b);}
    function $THREAD_2_20(){
      if(typeof Object.assign === "function")return Object.assign.apply(Object,arguments);
      if(!arguments[0])throw new TypeError("null");
      for(let i=1;i<arguments.length;i++){$THREAD_2_20(arguments[i]);}
      return arguments[0];
    }
    function $THREAD_3_30(name){let event=document.createEvent("CustomEvent");event.initCustomEvent(name,false,false,null);return event;}
    return [$THREAD_1_10,$THREAD_2_20,$THREAD_3_30,marker];}main();"#;
    let result = name_threads(source, true).unwrap();
    assert_eq!(result.report.applied_count, 3);
    assert!(result.source.contains("function hasOwnProperty(a,b)"));
    assert!(result
        .source
        .contains("assignObjectProperties(arguments[i])"));
    assert!(result
        .source
        .contains("function createCustomEventFallback(name)"));
    let mut replay = source.to_string();
    let mut edits = result.report.edits.clone();
    edits.sort_by_key(|e| std::cmp::Reverse(e.start));
    for e in edits {
        assert!(source.is_char_boundary(e.start as usize));
        replay.replace_range(e.start as usize..e.end as usize, &e.replacement);
    }
    assert_eq!(replay, result.source);
    assert!(result.source.contains("Object.prototype.hasOwnProperty"));
    assert!(result
        .report
        .edits
        .iter()
        .all(|e| source[e.start as usize..e.end as usize].starts_with("$THREAD_")));
}
#[test]
fn a_reassigned_api_alias_is_not_semantic_evidence() {
    let source="function main(){let own=Object.prototype.hasOwnProperty;own=other;function $THREAD_1_2(a,b){return own.call(a,b);}return $THREAD_1_2;}main();";
    assert_eq!(name_threads(source, true).unwrap().source, source);
}

#[test]
fn default_filter_duplicates_keep_occurrences_and_ambiguous_objects_do_not_get_positions() {
    let source = ROUTES.replace("[2,1,3,0,4]", "[2,1,2,3,0,4]");
    let report = identify_collectors(&source, false).unwrap().report;
    assert_eq!(
        report
            .payload_routing
            .slots
            .iter()
            .map(|s| s.positional_index)
            .collect::<Vec<_>>(),
        vec![Some(0), None, Some(1), Some(2), Some(3), None]
    );
    let route = report
        .signal_routes
        .iter()
        .find(|r| r.descriptor_index == 2)
        .unwrap();
    assert_eq!(route.default_payload_occurrences.len(), 2);
    for object in ["{ctor:null,name:1,...unknown}", "{ctor:null,name:1,name:2}"] {
        let source = ROUTES.replacen("{ctor:null}", object, 1);
        let report = identify_collectors(&source, false).unwrap().report;
        assert!(report
            .payload_routing
            .slots
            .iter()
            .find(|s| s.descriptor_index == 0)
            .unwrap()
            .reason
            .is_some());
    }
    let source = ROUTES.replace(
        "let filterDefault = [2,1,3,0,4];",
        "let filterDefault = [2,1,3,0,4]; filterDefault = [0,1,2,3,4];",
    );
    assert!(!identify_collectors(&source, false)
        .unwrap()
        .report
        .payload_routing
        .unresolved
        .is_empty());
}

#[test]
fn api_mentions_without_the_required_dataflow_are_not_roles() {
    let source = r#"function main(){
 function $THREAD_1_1(value){let index=invoke(Array.prototype.indexOf,this.__callbacks,value);if(index!==-1)log(index);}
 function $THREAD_2_2(value){this.__callbacks=value;log(Array.prototype.concat);}
 function $THREAD_3_3(a,b){let setter=Object.setPrototypeOf;return unrelated(a,b);}
 function $THREAD_4_4(name){let event=document.createEvent("CustomEvent");other.initCustomEvent(name);return event;}
 return [$THREAD_1_1,$THREAD_2_2,$THREAD_3_3,$THREAD_4_4];}main();"#;
    assert_eq!(name_threads(source, true).unwrap().source, source);
}

#[test]
fn collector_mode_migrates_registered_readable_bindings_and_keeps_direct_names() {
    let source = ROUTES
        .replace("$THREAD_1_101", "existingWaveRoutine")
        .replace("$THREAD_2_202", "otherWaveRoutine")
        .replace("$THREAD_9_900", "existingDirectRoutine");
    let result = identify_collectors(&source, true).unwrap();
    assert!(result.source.contains("function waveCollector1("));
    assert!(result.source.contains("function waveCollector2("));
    assert!(result.source.contains("function existingDirectRoutine("));
    assert_eq!(result.report.renames.iter().filter(|r| r.applied).count(), 2);
    assert!(result.report.renames.iter().any(|r|
        r.binding.as_ref().is_some_and(|b| b.name == "existingDirectRoutine") && !r.applied));
}

#[test]
fn multi_signal_collector_purpose_labels_stay_metadata() {
    use shape::naming::labels::build_label_reference;
    use shape::naming::name_threads_with_labels;
    let reference = build_label_reference(
        ROUTES,
        &ROUTES.replace("$THREAD_1_101", "singleSignalGuess"),
    )
    .unwrap();
    let result = name_threads_with_labels(ROUTES, true, Some(&reference)).unwrap();
    assert!(result.source.contains("function waveCollector1("));
    assert!(!result.source.contains("singleSignalGuess"));
    assert!(result
        .report
        .aggregate_collector_claims
        .iter()
        .any(|c| c.purpose_names.contains(&"singleSignalGuess".to_string())));
}

#[test]
fn exact_hardcoded_serialization_hook_is_named_without_changing_live_body() {
    let source = r#"(function(){function $THREAD_71_9201(){throw new Error("serialisation is hard-coded into the encoder");}const fixedSerializationExtension={serialiser:$THREAD_71_9201}; use(fixedSerializationExtension);})();"#;
    let result = name_threads(source, true).unwrap();
    assert_eq!(result.report.applied_count, 1);
    let outcome = &result.report.outcomes[0];
    assert_eq!(
        outcome.evaluator.as_deref(),
        Some("hardcoded_serialization_error")
    );
    assert!(outcome.applied);
    assert!(result.report.removals.is_empty());
    assert_eq!(
        result.source,
        source.replace("$THREAD_71_9201", "throwHardcodedSerializationError")
    );
    assert_eq!(name_threads(source, false).unwrap().source, source);
    let readable = source.replace("$THREAD_71_9201", "existingHumanHook");
    assert_eq!(name_threads(&readable, true).unwrap().source, readable);
    for refused in [
        source.replace(
            "function $THREAD_",
            "const Error = CustomError; function $THREAD_",
        ),
        source.replace("hard-coded into", "configured by"),
        source.replace("throw new", "sideEffect();throw new"),
        source
            .replace("Error(\"serialisation", "Error(\"serialisation")
            .replace("encoder\");", "encoder\", sideEffect());"),
    ] {
        let result = name_threads(&refused, true).unwrap();
        assert!(!result
            .report
            .outcomes
            .iter()
            .any(|o| o.applied
                && o.proposed_name.as_deref() == Some("throwHardcodedSerializationError")));
        assert_eq!(result.source, refused);
    }
}
