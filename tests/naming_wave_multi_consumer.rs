//! Source-only packet; the gate integrator owns execution and valid-Model checks.
use std::collections::BTreeSet;
use shape::naming::collectors::identify_collectors;
use shape::naming::name_threads;

const TEMPLATE: &str = include_str!("fixtures/program/wave-multi-consumer/finite-origins.js");

fn source(registrations: &str) -> String {
    TEMPLATE.replace("REGISTRATIONS", registrations)
}

fn valid(source: &str) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::unambiguous()).parse();
    assert!(!parsed.panicked && parsed.errors.is_empty(), "{:?}", parsed.errors);
    identify_collectors(source, false).expect("source must build a valid Model");
}

fn origins(source: &str) -> BTreeSet<(String, usize)> {
    valid(source);
    let report = identify_collectors(source, false).unwrap().report;
    report.registered_groups.into_iter().filter(|g| g.unresolved.is_empty())
        .map(|g| (g.function.unwrap().binding.unwrap().name, g.slot)).collect()
}

fn expected(names: &[(&str, usize)]) -> BTreeSet<(String, usize)> {
    names.iter().map(|(name, slot)| ((*name).into(), *slot)).collect()
}

fn refused(source: &str) {
    assert!(origins(source).is_empty());
    let result = identify_collectors(source, true).unwrap();
    assert_eq!(result.source, source, "{:?}", result.report.renames);
    valid(&result.source);
}

#[test]
fn each_call_uses_its_own_reaching_registry_and_exact_position() {
    let source = source("grouped = [$THREAD_1_101, meaningfulPurpose]; run();\n\
        grouped = [$THREAD_1_101_2, meaningfulPurpose]; run();");
    assert_eq!(origins(&source), expected(&[
        ("$THREAD_1_101", 0), ("$THREAD_1_101_2", 0), ("meaningfulPurpose", 1),
    ]));
    let result = name_threads(&source, true).unwrap();
    assert!(result.report.collectors.renames.iter().all(|r|
        r.proposed_name.as_deref().is_some_and(|n| n.starts_with("waveCollector"))));
    assert!(result.report.aggregate_collector_claims.iter().any(|c|
        c.purpose_names.iter().any(|n| n == "meaningfulPurpose")));
    valid(&result.source);
}

#[test]
fn overwritten_and_unconsumed_versions_do_not_donate_labels() {
    for registration in [
        "grouped = [$THREAD_1_101]; grouped = [$THREAD_1_101_2]; run();",
        "grouped = [$THREAD_1_101_2]; run(); grouped = [$THREAD_1_101];",
    ] {
        assert_eq!(origins(&source(registration)), expected(&[("$THREAD_1_101_2", 0)]));
    }
}

#[test]
fn distinct_if_else_origins_are_both_live_at_the_join() {
    let source = source("if (branchFlag) { grouped = [$THREAD_1_101]; }\n\
        else { grouped = [$THREAD_1_101_2]; } run();");
    assert_eq!(origins(&source), expected(&[("$THREAD_1_101", 0), ("$THREAD_1_101_2", 0)]));
}

#[test]
fn branch_local_consumption_does_not_borrow_its_counterpart() {
    let source = source("if (branchFlag) { grouped = [$THREAD_1_101]; run(); }\n\
        else { grouped = [$THREAD_1_101_2]; }");
    assert_eq!(origins(&source), expected(&[("$THREAD_1_101", 0)]));
}

#[test]
fn callable_and_registry_branches_remain_correlated() {
    let source = source("function dormant() { unknownDormant(); } let select;\n\
        if (branchFlag) { grouped = [$THREAD_1_101]; select = start; }\n\
        else { grouped = [$THREAD_1_101_2]; select = dormant; } select();");
    assert_eq!(origins(&source), expected(&[("$THREAD_1_101", 0)]));
}

#[test]
fn callable_snapshot_retains_identity_after_the_old_alias_is_replaced() {
    let source = source("function dormant() { unknownDormant(); }\n\
        let selected = start; const snapshot = selected; selected = dormant;\n\
        grouped = [$THREAD_1_101]; snapshot(); grouped = [$THREAD_1_101_2];");
    assert_eq!(origins(&source), expected(&[("$THREAD_1_101", 0)]));
}

#[test]
fn escaped_consumer_can_observe_later_versions_without_losing_the_earlier_one() {
    let source = source("grouped = [$THREAD_1_101]; external(run);\n\
        grouped = [$THREAD_1_101_2];");
    assert_eq!(origins(&source), expected(&[("$THREAD_1_101", 0), ("$THREAD_1_101_2", 0)]));
}

#[test]
fn uninitialized_branch_early_consumption_and_write_escape_refuse() {
    for registration in [
        "run(); grouped = [$THREAD_1_101]; grouped = [$THREAD_1_101_2];",
        "grouped = [$THREAD_1_101]; run(); grouped = externalValue;",
        "if (branchFlag) { grouped = [$THREAD_1_101]; grouped = [$THREAD_1_101_2]; } run();",
        "grouped = [$THREAD_1_101]; external(grouped); grouped = [$THREAD_1_101_2]; run();",
        "grouped = [$THREAD_1_101]; grouped[0] = $THREAD_1_101_2; grouped = [$THREAD_1_101_2]; run();",
        "grouped = [$THREAD_1_101]; grouped = [$THREAD_1_101_2]; function donor() { grouped = [$THREAD_1_101]; } run();",
    ] { refused(&source(registration)); }
}

#[test]
fn dormant_nested_donors_and_wrong_abi_cannot_establish_origins() {
    refused(&source("function donor() { grouped = [$THREAD_1_101]; grouped = [$THREAD_1_101_2]; } run();"));
    let wrong = source("grouped = [$THREAD_1_101]; run(); grouped = [$THREAD_1_101_2]; run();")
        .replace("factory(emitters, context, init, collect, function(){}, function(){});",
            "factory(emitters, context, init, collect);");
    refused(&wrong);
    let dormant = source("grouped = [$THREAD_1_101]; grouped = [$THREAD_1_101_2];");
    refused(&dormant);
}

#[test]
fn finite_origins_do_not_invent_a_unique_position_for_a_shared_factory() {
    let source = source("grouped = [$THREAD_1_101, meaningfulPurpose]; run();\n\
        grouped = [meaningfulPurpose, $THREAD_1_101]; run();");
    valid(&source);
    let result = identify_collectors(&source, true).unwrap();
    assert_eq!(result.source, source);
    assert!(result.report.renames.iter().all(|r| !r.applied));
}

#[test]
fn existing_wave_name_or_capture_is_not_overwritten() {
    let source = source("const waveCollector1 = 7;\n\
        grouped = [$THREAD_1_101]; run(); grouped = [$THREAD_1_101_2]; run();\n\
        external(waveCollector1);");
    valid(&source);
    let result = name_threads(&source, true).unwrap();
    assert!(result.source.contains("const waveCollector1 = 7;"));
    assert!(result.source.contains("external(waveCollector1)"));
    valid(&result.source);
}

#[test]
fn compound_declarator_order_refuses_instead_of_losing_the_early_origin() {
    // Markers must not run before selected is initialized.
    refused(&source("grouped = [$THREAD_1_101];\n\
        let selected = start, ignored = selected();\n\
        grouped = [$THREAD_1_101_2]; run();"));
}

#[test]
fn escaped_alias_latches_the_value_and_rebinding_cannot_revoke_it() {
    // The external closure retains start rather than the selected cell.
    let source = source("function dormant() {}\n\
        grouped = [$THREAD_1_101]; let selected = start; external(selected);\n\
        selected = dormant; grouped = [$THREAD_1_101_2];");
    assert_eq!(origins(&source), expected(&[("$THREAD_1_101", 0), ("$THREAD_1_101_2", 0)]));
}

#[test]
fn a_failed_escape_does_not_become_live_when_the_alias_changes_later() {
    let source = source("function dormant() {} let selected = dormant;\n\
        grouped = [$THREAD_1_101]; external(selected);\n\
        selected = start; grouped = [$THREAD_1_101_2]; run();");
    assert_eq!(origins(&source), expected(&[("$THREAD_1_101_2", 0)]));
}

#[test]
fn deferred_caller_keeps_an_inner_alias_dynamic_until_invocation() {
    let source = source("function dormant() {} let selected = start;\n\
        function later() { selected(); }\n\
        grouped = [$THREAD_1_101]; external(later);\n\
        selected = dormant; grouped = [$THREAD_1_101_2];");
    assert_eq!(origins(&source), expected(&[("$THREAD_1_101", 0)]));
}

#[test]
fn false_and_returned_six_argument_donors_cannot_override_a_live_hook_abi() {
    for dead in [
        "if (false) factory(emitters, context, init, collect, function(){}, function(){});",
        "return; factory(emitters, context, init, collect, function(){}, function(){});",
        "function dormantAbi() { factory(emitters, context, init, collect, function(){}, function(){}); }",
    ] {
        let source = source("grouped = [$THREAD_1_101]; run(); grouped = [$THREAD_1_101_2]; run();")
            .replace("factory(emitters, context, init, collect, function(){}, function(){});",
                &format!("factory(emitters, context, init, collect); {dead}"));
        refused(&source);
    }
}

#[test]
fn consumption_after_a_nested_return_or_in_a_false_branch_is_dormant() {
    for body in ["function later() { return; run(); } later();",
        "function later() { if (false) run(); } later();"] {
        refused(&source(&format!("grouped = [$THREAD_1_101]; grouped = [$THREAD_1_101_2]; {body}")));
    }
}

#[test]
fn short_circuit_and_unsupported_identity_control_fail_closed() {
    for registration in [
        "grouped = [$THREAD_1_101]; branchFlag && run(); grouped = [$THREAD_1_101_2]; run();",
        "grouped = [$THREAD_1_101]; while (branchFlag) { run(); } grouped = [$THREAD_1_101_2];",
        "switch (branchFlag) { default: grouped = [$THREAD_1_101]; } grouped = [$THREAD_1_101_2]; run();",
        "try { grouped = [$THREAD_1_101]; } finally { grouped = [$THREAD_1_101_2]; } run();",
    ] { refused(&source(registration)); }
}

#[test]
fn catch_paths_keep_whole_registry_and_callable_states() {
    let source = source("let selected;\n\
        try { mayThrow(); grouped = [$THREAD_1_101]; selected = start; }\n\
        catch (error) { grouped = [$THREAD_1_101_2]; selected = start; } selected();");
    assert_eq!(origins(&source), expected(&[("$THREAD_1_101", 0), ("$THREAD_1_101_2", 0)]));
    refused(&source.replace("grouped = [$THREAD_1_101_2]; selected = start;", "selected = start;"));
}

#[test]
fn a_pure_try_block_cannot_borrow_a_dormant_catch_donor() {
    for registration in [
        "try { 1; } catch (error) { run(); } grouped = [$THREAD_1_101]; grouped = [$THREAD_1_101_2];",
        "grouped = [$THREAD_1_101]; grouped = [$THREAD_1_101_2]; try { 1; } catch (error) { run(); }",
    ] { refused(&source(registration)); }
}

#[test]
#[ignore = "integrator: run after focused synthetic checks and first pinned witness"]
fn all_seven_pinned_sources_keep_each_live_origin_at_its_own_exact_position() {
    use sha2::{Digest, Sha256};
    let expectations: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/program/wave-multi-consumer/pinned-origins.json")).unwrap();
    let fixtures = [
        include_str!("fixtures/program/wave-multi-consumer/reaching-registry-baseline.js"),
        include_str!("fixtures/program/wave-multi-consumer/reaching-registry-alternate.js"),
        include_str!("fixtures/program/wave-multi-consumer/registry-callable-aliases.js"),
        include_str!("fixtures/program/wave-multi-consumer/branched-registry.js"),
        include_str!("fixtures/program/wave-multi-consumer/registry-consumer-heldout.js"),
        include_str!("fixtures/program/wave-multi-consumer/single-guarded-registry.js"),
        include_str!("fixtures/program/wave-multi-consumer/reaching-registry-heldout.js"),
    ];
    for (source, expectation) in fixtures.into_iter().zip(expectations.as_array().unwrap()) {
        let key = expectation["key"].as_str().unwrap();
        let digest: String = Sha256::digest(source.as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(digest, expectation["input_sha256"].as_str().unwrap(), "{key}");
        valid(source);
        let report = identify_collectors(source, false).unwrap().report;
        assert!(report.registered_groups.iter().all(|g| g.unresolved.is_empty()), "{key}");
        let actual: BTreeSet<_> = report.registered_groups.iter().map(|g| {
            let f = g.function.as_ref().unwrap(); (f.span.start, f.span.end, g.slot)
        }).collect();
        let mut expected = BTreeSet::new();
        for origin in expectation["origins"].as_array().unwrap() {
            for slot in origin["slots"].as_array().unwrap() {
                expected.insert((slot["function_span"]["start"].as_u64().unwrap() as u32,
                    slot["function_span"]["end"].as_u64().unwrap() as u32,
                    slot["slot"].as_u64().unwrap() as usize));
            }
        }
        assert_eq!(actual, expected, "{key}: exact own-origin positional proof");
    }
}

#[test]
fn exact_independent_review_reproductions_are_not_vacuous_refusals() {
    refused(include_str!("fixtures/program/wave-multi-consumer/review-multi-declarator.js"));
    let escaped = include_str!("fixtures/program/wave-multi-consumer/review-escaped-callable.js");
    assert_eq!(origins(escaped), expected(&[("$THREAD_1_101", 0), ("$THREAD_1_101_2", 0)]));
    // Keep the independent bytes intact; add a marker only to establish the
    // same reachable-main Model contract used by the synthetic family tests.
    let abi = include_str!("fixtures/program/wave-multi-consumer/review-dormant-abi.js")
        .replace("function main(flag) {", "function main(flag) { const registryContext = {kind: 'review'};");
    refused(&abi);
}

#[test]
fn no_exit_loop_and_do_return_cannot_donate_a_later_consumer() {
    for control in ["for (;;) {}", "while (true) {}", "do { return; } while (branchFlag);"] {
        refused(&source(&format!("grouped = [$THREAD_1_101]; grouped = [$THREAD_1_101_2]; {control} run();")));
    }
}

#[test]
fn opaque_loop_with_a_reachable_owned_break_can_precede_consumption() {
    let source = source("grouped = [$THREAD_1_101]; for (;;) { if (branchFlag) break; } run();\n\
        grouped = [$THREAD_1_101_2]; run();");
    assert_eq!(origins(&source), expected(&[("$THREAD_1_101", 0), ("$THREAD_1_101_2", 0)]));
}

#[test]
fn review_self_callee_argument_escape_retains_both_origins() {
    let source = include_str!("fixtures/program/wave-multi-consumer/review-self-callee-argument.js");
    assert_eq!(origins(source), expected(&[("$THREAD_1_101", 0), ("$THREAD_1_101_2", 0)]));
}

#[test]
fn branched_registry_has_both_branch_origins() {
    let source = include_str!("fixtures/program/wave-multi-consumer/branched-registry.js");
    let got = origins(source);
    assert!(got.contains(&("$THREAD_4_157555_2".into(), 0)), "first branch missing: {got:?}");
    assert!(got.contains(&("$THREAD_4_157555".into(), 0)), "second branch missing: {got:?}");
    // Seven tail factories are shared by both versions, so the identity set
    // contains 25 entries while the two arrays have 32 registration records.
    assert_eq!(got.len(), 25);
    assert_eq!(identify_collectors(source, false).unwrap().report.registered_groups.len(), 32);
}

#[test]
fn coordinator_alias_initializers_cannot_borrow_later_or_guarded_values() {
    let activation = "function maker(){function boot(){run();}return {init:boot};}\n\
        let ctor;function entry(){let instance=new ctor();instance.init();}\n\
        grouped=[$THREAD_1_101];ctor=maker;external(entry);\n\
        grouped=[$THREAD_1_101_2];ctor=maker;";
    for setup in [
        "let helper2; const helper1=helper2; helper2=coordinate;",
        "let helper1; if(branchFlag){helper1=coordinate;}",
    ] {
        let source = source(&format!("{setup}\n{activation}"))
            .replace("coordinate([], factory,", "helper1([], factory,");
        refused(&source);
    }
}
