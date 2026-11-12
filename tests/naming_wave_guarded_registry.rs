//! Guarded origin proof: one owned write, same-branch ordered calls/escapes.
//! Gate integrator owns execution; captured JavaScript is never evaluated.
use shape::naming::collectors::identify_collectors;
use shape::naming::name_threads;

const SOURCE: &str = include_str!("fixtures/program/wave-named-transport/guarded-origin.js");

fn valid(source: &str) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::unambiguous()).parse();
    assert!(!parsed.panicked && parsed.errors.is_empty(), "{:?}", parsed.errors);
    identify_collectors(source, false).expect("fixture must build a valid Model");
}

fn refused(source: &str) {
    valid(source);
    let result = identify_collectors(source, true).unwrap();
    assert_eq!(result.source, source, "{:?}", result.report.renames);
}

#[test]
fn owned_guarded_write_precedes_deferred_initialization_escape() {
    valid(SOURCE);
    let result = name_threads(SOURCE, true).unwrap();
    assert_eq!(result.report.collectors.registered_groups.len(), 2);
    assert!(result.report.collectors.registered_groups.iter().all(|g| g.unresolved.is_empty()));
    assert!(result.source.contains("grouped = [waveCollector1, waveCollector2]"));
    valid(&result.source);
    assert_eq!(name_threads(&result.source, true).unwrap().source, result.source);
}

#[test]
fn guarded_write_cannot_donate_to_early_outside_sibling_or_dormant_consumption() {
    for source in [
        SOURCE.replace("grouped = [", "startAlias(); grouped = ["),
        SOURCE.replace("return {};", "startAlias(); return {};"),
        SOURCE.replace("return {};", "return {outside: startAlias};"),
        SOURCE.replace("return {initialize};", "return {};"),
        SOURCE.replace("grouped = [producer, secondProducer];", "function unusedDonor() { grouped = [producer, secondProducer]; }"),
        SOURCE.replace("return {};", "if (other) { startAlias(); } return {};"),
        SOURCE.replace("  return {};", "  else { startAlias(); } return {};"),
    ] {
        refused(&source);
    }
}

#[test]
fn guarded_write_requires_unique_own_binding_and_direct_single_arm() {
    for source in [
        SOURCE.replace("grouped = [producer, secondProducer];", "let grouped = [producer, secondProducer];"),
        SOURCE.replace("grouped = [producer, secondProducer];", "const unrelated = [producer, secondProducer];"),
        SOURCE.replace("grouped = [producer, secondProducer];", "grouped = [producer, secondProducer]; startAlias(); grouped = [secondProducer, producer];"),
        SOURCE.replace("if (flag) {", "while (flag) {"),
        SOURCE.replace("grouped = [producer, secondProducer];", "if (other) { grouped = [producer, secondProducer]; }"),
        SOURCE.replace("factory(emitters, context, init, collect, function(){}, function(){});", "factory(emitters, context, init, collect);"),
    ] {
        refused(&source);
    }
}

#[test]
fn guarded_write_keeps_slot_order_and_scope_specific_function_origin() {
    let source = SOURCE.replace("[producer, secondProducer]", "[secondProducer, producer]")
        .replace("grouped = [", "function producer(emitters) { branchOnly(emitters); } grouped = [");
    valid(&source);
    let result = identify_collectors(&source, true).unwrap();
    assert!(result.source.contains("function producer(emitters) { unknown(emitters); }"));
    assert!(result.source.contains("function waveCollector2(emitters) { branchOnly(emitters); }"));
    assert!(result.source.contains("grouped = [waveCollector1, waveCollector2]"));
}

#[test]
fn single_guarded_registry_names_exactly_sixteen_origins() {
    let source = include_str!("fixtures/program/wave-named-transport/named-transport-heldout.js");
    valid(source);
    let result = name_threads(source, true).unwrap();
    let groups = &result.report.collectors.registered_groups;
    let spans = [(440489,455300),(436018,440209),(408811,435982),(335235,390435),
        (402981,407626),(390441,402975),(324434,334682),(300588,324428),
        (294321,300582),(278674,291004),(247626,278572),(218038,242487),
        (199761,218033),(192041,199197),(167284,190846),(157546,165960)];
    assert_eq!(groups.len(), spans.len(), "{groups:?}");
    for (slot, (start, end)) in spans.into_iter().enumerate() {
        let group = groups.iter().find(|g| g.slot == slot).unwrap();
        assert!(group.unresolved.is_empty(), "{group:?}");
        let function = group.function.as_ref().unwrap();
        assert_eq!((function.span.start, function.span.end), (start, end));
        let id = format!("fn@{start}..{end}");
        let name = format!("waveCollector{}", slot + 1);
        assert!(result.report.outcomes.iter().any(|o| o.function_id == id
            && o.applied && o.proposed_name.as_deref() == Some(name.as_str())), "{id}: {name}");
    }
    assert!(result.report.aggregate_collector_claims.iter().any(|c|
        c.function_span.start == 440489
            && c.purpose_names.iter().any(|n| n == "emitEncodedEnvironmentSignal")));
    // The three noncollector hook origins retain their accepted labels.
    for (start, name) in [(490831, "createPlatformFormSubmitHook"),
        (498367, "createFetchInstrumentation"), (506769, "createXhrInstrumentationModule")] {
        assert!(result.report.outcomes.iter().any(|o| o.binding.declaration.start == start
            && o.applied && o.proposed_name.as_deref() == Some(name)));
    }
    valid(&result.source);
}

#[test]
fn registry_branches_keep_their_own_proven_origins() {
    let source = include_str!("fixtures/program/wave-named-transport/branched-registry.js");
    valid(source);
    let result = name_threads(source, true).unwrap();
    // The finite replacement proof certifies both actual init escapes. Each
    // branch keeps its own factory; neither textual origin substitutes for
    // its counterpart. The seven shared tail factories stay shared.
    assert_eq!(result.report.collectors.registered_groups.len(),32);
    assert!(result.report.outcomes.iter().any(|o| o.binding.name == "$THREAD_4_157555"
        && o.applied && o.proposed_name.as_deref() == Some("waveCollector1")));
    assert!(result.report.outcomes.iter().any(|o| o.binding.name == "$THREAD_4_157555_2"
        && o.applied && o.proposed_name.as_deref() == Some("waveCollector1")));
    valid(&result.source);
}

#[test]
fn an_unconsumed_guarded_version_does_not_conflict_with_the_live_slot_order() {
    let source=SOURCE.replace("grouped = [producer, secondProducer];", "grouped = [producer, secondProducer]; grouped = [secondProducer, producer];");
    valid(&source);
    let result=identify_collectors(&source,true).unwrap();
    assert_eq!(result.report.registered_groups.len(),2);
    assert!(result.report.renames.iter().any(|r|r.binding.as_ref().is_some_and(|b|b.name=="secondProducer")&&r.applied&&r.proposed_name.as_deref()==Some("waveCollector1")));
    assert!(result.report.renames.iter().any(|r|r.binding.as_ref().is_some_and(|b|b.name=="producer")&&r.applied&&r.proposed_name.as_deref()==Some("waveCollector2")));
    valid(&result.source);
}

#[test]
fn unwritten_dormant_registry_is_not_invented() {
    let source = include_str!("fixtures/program/wave-named-transport/dormant-registry.js");
    valid(source);
    let result = name_threads(source, true).unwrap();
    assert!(result.report.collectors.registered_groups.is_empty());
    assert!(!result.report.outcomes.iter().any(|o|
        o.proposed_name.as_deref().is_some_and(|n| n.starts_with("waveCollector"))));
    assert!(result.source.contains("$KEY_161.forEach"));
}
