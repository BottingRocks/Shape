//! Source-only lane: the gate integrator owns compilation and execution.
use shape::naming::collectors::identify_collectors;
use shape::naming::name_threads;

const SOURCE: &str = include_str!("fixtures/program/wave-named-transport/parameter-helper.js");

fn valid(source: &str) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::unambiguous()).parse();
    assert!(!parsed.panicked && parsed.errors.is_empty(), "{:?}", parsed.errors);
    // This API must build a Model before any matcher or refusal is inspected.
    identify_collectors(source, false).expect("fixture must build a valid Model");
}

fn refused(source: &str) {
    valid(source);
    let result = identify_collectors(source, true).unwrap();
    assert_eq!(result.source, source, "{:?}", result.report.renames);
}

#[test]
fn stable_named_parameter_transport_preserves_all_four_origins_and_hooks() {
    valid(SOURCE);
    let result = name_threads(SOURCE, true).unwrap();
    let groups = &result.report.collectors.registered_groups;
    assert_eq!(groups.len(), 4);
    assert_eq!(groups.iter().map(|g| g.slot).collect::<Vec<_>>(), vec![0, 1, 2, 3]);
    assert!(groups.iter().all(|g| g.unresolved.is_empty() && g.function.is_some()), "{groups:?}");
    for group in groups {
        let function = group.function.as_ref().unwrap();
        assert_eq!(&SOURCE[function.span.start as usize..function.span.end as usize],
            match group.slot {
                0 => "function $THREAD_1_101(emitters) { unknown(emitters); }",
                1 => "(emitters) => { return this === arguments[0] ? emitters : null; }",
                2 => "(emitters) => { return emitters; }",
                3 => "function semanticPurpose(emitters) { return emitters.length; }",
                _ => unreachable!(),
            });
    }
    assert!(result.source.contains("function waveCollector1(emitters)"));
    assert!(result.source.contains("const waveCollector2 = (emitters) => { return this === arguments[0] ? emitters : null; };"));
    assert!(result.source.contains("const waveCollector3 = (emitters) => { return emitters; };"));
    assert!(result.source.contains("function waveCollector4(emitters)"));
    assert!(result.source.contains("function hookPurpose(context)"));
    assert!(result.source.contains("let hooks = [hookPurpose]"));
    assert!(result.report.aggregate_collector_claims.iter().any(|c|
        c.purpose_names.iter().any(|n| n == "semanticPurpose")));
    valid(&result.source);
    assert_eq!(name_threads(&result.source, true).unwrap().source, result.source);
}

#[test]
fn stable_callable_aliases_transport_the_same_parameter_bindings() {
    for helper in [
        "const transport = $B;",
        "let transport; transport = $B;",
        "const transport = (array, index, first, second) => { array[index] = first; return second; };",
        "const transport = function(array, index, first, second) { array[index] = first; return second; };",
    ] {
        let source = SOURCE.replace("let grouped =", &format!("{helper}\n  let grouped ="))
            .replace("= $B(grouped, 1,", "= transport(grouped, 1,");
        valid(&source);
        let result = identify_collectors(&source, true).unwrap();
        assert_eq!(result.report.registered_groups.len(), 4);
        assert!(result.report.registered_groups.iter().all(|g| g.unresolved.is_empty()));
        assert!(result.source.contains("const waveCollector2 = "));
        assert!(result.source.contains("const waveCollector3 = "));
    }
}

#[test]
fn named_transports_cannot_borrow_globals_factories_or_swapped_provenance() {
    for source in [
        SOURCE.replace("$PARAMA[$PARAMB] = $PARAMC;", "grouped[$PARAMB] = $PARAMC;"),
        SOURCE.replace("$PARAMA[$PARAMB] = $PARAMC;", "$PARAMB[$PARAMA] = $PARAMC;"),
        SOURCE.replace("return $PARAMD;", "return semanticPurpose;"),
        SOURCE.replace("return $PARAMD;", "return $PARAMD();"),
        SOURCE.replace("$PARAMA[$PARAMB] = $PARAMC;", "if (flag) $PARAMA[$PARAMB] = $PARAMC;"),
        SOURCE.replace("function $B(", "async function $B("),
        SOURCE.replace("function $B(", "function* $B("),
        SOURCE.replace("$PARAMC, $PARAMD)", "$PARAMC, $PARAMD = semanticPurpose)"),
        SOURCE.replace("grouped[2] = $B", "$B = external; grouped[2] = $B"),
        SOURCE.replace("grouped[2] = $B(grouped, 1,", "grouped[2] = $B?.(grouped, 1,"),
        SOURCE.replace("let grouped =", "const transport = recursive();\n  let grouped =")
            .replace("= $B(grouped, 1,", "= transport(grouped, 1,"),
    ] {
        refused(&source);
    }
}

#[test]
fn dormant_or_early_helper_calls_cannot_populate_the_active_registry() {
    let dormant = SOURCE.replace("grouped[2] = $B", "function unusedDonor() { grouped[2] = $B")
        .replace("grouped[3] = semanticPurpose;", "} grouped[3] = semanticPurpose;");
    refused(&dormant);
    refused(&SOURCE.replace("grouped[2] = $B", "start(); grouped[2] = $B"));
    refused(&SOURCE.replace("const publicGroups = grouped;", "const publicGroups = grouped; external(grouped);"));
}

#[test]
fn four_argument_hook_consumption_cannot_establish_a_wave_registry() {
    refused(&SOURCE.replace(
        "factory(emitters, context, init, collect, function(){}, function(){});",
        "factory(emitters, context, init, collect);",
    ));
}

#[test]
fn affected_named_helpers_recover_positions_from_the_exact_cached_sources() {
    for (key, source, count) in [
        ("named-transport-baseline", include_str!("fixtures/program/wave-named-transport/named-transport-baseline.js"), 13),
        ("named-transport-guarded", include_str!("fixtures/program/wave-named-transport/named-transport-guarded.js"), 13),
        ("named-transport-parameter-layout", include_str!("fixtures/program/wave-named-transport/named-transport-parameter-layout.js"), 14),
    ] {
        valid(source);
        let result = name_threads(source, true).unwrap();
        let groups = &result.report.collectors.registered_groups;
        assert_eq!(groups.len(), count, "{key}: {groups:?}");
        for (slot, group) in groups.iter().enumerate() {
            assert_eq!(group.slot, slot, "{key}");
            assert!(group.unresolved.is_empty(), "{key}: {group:?}");
            let function = group.function.as_ref().expect("stable source origin");
            let id = format!("fn@{}..{}", function.span.start, function.span.end);
            let name = format!("waveCollector{}", slot + 1);
            assert!(result.report.collectors.renames.iter().any(|r|
                r.function_id == id && r.proposed_name.as_deref() == Some(name.as_str())
                    && (r.applied || r.reason.as_deref() == Some("report only; rename disabled"))),
                "{key}: missing positional claim {id} {name}");
        }
        valid(&result.source);
    }
}

#[test]
fn affected_multiple_writers_keep_both_actual_consumption_paths() {
    let source = include_str!("fixtures/program/wave-named-transport/named-transport-alternate.js");
    valid(source);
    let result = name_threads(source, true).unwrap();
    // Two returned init paths activate the consumer after their respective
    // array replacements. Both exact function identities must be retained.
    assert_eq!(result.report.collectors.registered_groups.len(),26);
    assert!(result.report.outcomes.iter().any(|o|
        o.binding.name == "$THREAD_8_31842_2" && o.applied
            && o.proposed_name.as_deref() == Some("waveCollector1")));
    assert!(result.report.outcomes.iter().any(|o|
        o.binding.name == "$THREAD_8_31842" && o.applied
            && o.proposed_name.as_deref() == Some("waveCollector1")));
    valid(&result.source);
}
