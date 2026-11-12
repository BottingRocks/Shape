use super::*;
const CONDITIONAL_SOURCE: &str =
    include_str!("../../../../tests/fixtures/captured_range_predicate/conditional-bounds.js");
const TARGETS: [&str; 2] = ["$THREAD_1_40999", "$THREAD_1_40999_2"];
const SPANS: [(u32, u32); 2] = [(58359, 58591), (66928, 67150)];
const ROLE: &str = "signals_captured_range_predicate";

fn models(source: &str, expected: [bool; 2]) {
    let allocator = Allocator::default();
    let program = crate::analysis::parse_program(&allocator, source).expect("whole valid context");
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let model = Model::new(
        source,
        &program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .unwrap();
    for (index, name) in TARGETS.iter().enumerate() {
        let sites:Vec<_>=model.all_sites().iter().filter(|site|
            matches!(model.nodes.kind(site.node_id),AstKind::Function(f) if f.id.as_ref().is_some_and(|id|id.name==*name))).collect();
        assert_eq!(
            sites.len(),
            1,
            "whole indexed target, never a partial guard"
        );
        let proof = conditional_initialized(&model, *sites[0]);
        assert_eq!(proof.is_some(), expected[index], "target {name}: {proof:?}");
        if let Some(proof) = proof {
            assert!(
                proof.ready > sites[0].span.end,
                "hoisted definition is NOT initialization evidence"
            );
            assert_eq!(proof.aliases.len(), 2);
            assert_eq!(proof.owner_entries, 3);
            assert_eq!(proof.entry_roots.len(), 3);
        }
    }
}
fn production(source: &str, expected: [bool; 2], original: bool) {
    let result = name_threads(source, true).unwrap();
    let mut found = Vec::new();
    for (index, name) in TARGETS.iter().enumerate() {
        let outcomes: Vec<_> = result
            .report
            .outcomes
            .iter()
            .filter(|o| o.binding.name == *name)
            .collect();
        assert_eq!(outcomes.len(), 1);
        let outcome = outcomes[0];
        let matched = outcome.applied && outcome.evaluator.as_deref() == Some(ROLE);
        assert_eq!(matched, expected[index], "{outcome:?}");
        if matched {
            assert_eq!(
                outcome.role_name.as_deref(),
                Some("capturedTwoRangeOr45Predicate")
            );
            found.push(outcome);
        }
        if original {
            assert_eq!(
                outcome.function_id,
                format!("fn@{}..{}", SPANS[index].0, SPANS[index].1)
            );
            assert_eq!(
                (
                    outcome.binding.declaration.start,
                    outcome.binding.declaration.end
                ),
                SPANS[index]
            );
        }
    }
    let all = result
        .report
        .outcomes
        .iter()
        .filter(|o| o.applied && o.evaluator.as_deref() == Some(ROLE))
        .count();
    assert_eq!(
        all,
        expected.iter().filter(|v| **v).count(),
        "exact production family cardinality"
    );
    let mut replay = source.to_string();
    for edit in result.report.edits.iter().rev() {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
}
fn mutate_body(index: usize, from: &str, to: &str) -> String {
    let allocator = Allocator::default();
    let program = crate::analysis::parse_program(&allocator, CONDITIONAL_SOURCE).unwrap();
    let (scoping, nodes) = crate::analysis::analyze(&program);
    let model = Model::new(
        CONDITIONAL_SOURCE,
        &program,
        &scoping,
        &nodes,
        ProgramPropertiesLimits::default(),
    )
    .unwrap();
    let site = model
        .all_sites()
        .iter()
        .find(|site| {
            matches!(model.nodes.kind(site.node_id),AstKind::Function(f)
        if f.id.as_ref().is_some_and(|id|id.name==TARGETS[index]))
        })
        .unwrap();
    let body = &CONDITIONAL_SOURCE[site.span.start as usize..site.span.end as usize];
    assert_eq!(body.matches(from).count(), 1);
    let mut source = CONDITIONAL_SOURCE.to_string();
    source.replace_range(
        site.span.start as usize..site.span.end as usize,
        &body.replacen(from, to, 1),
    );
    source
}
fn changed(from: &str, to: &str) -> String {
    assert_eq!(
        CONDITIONAL_SOURCE.matches(from).count(),
        1,
        "precise source mutation {from}"
    );
    CONDITIONAL_SOURCE.replacen(from, to, 1)
}
#[test]
fn conditional_actual_both_full_models_and_production_identity() {
    models(CONDITIONAL_SOURCE, [true, true]);
    production(CONDITIONAL_SOURCE, [true, true], true);
}
#[test]
fn conditional_alpha_and_exact_equivalent_block_layouts() {
    let alpha = CONDITIONAL_SOURCE
        .replace("$DB", "datum")
        .replace("$KEY_47", "opaqueLowerOne")
        .replace("$KEY_97", "opaqueUpperOne")
        .replace("$KEY_185", "opaqueLowerTwo")
        .replace("$KEY_40", "opaqueUpperTwo");
    models(&alpha, [true, true]);
    production(&alpha, [true, true], false);
    // Rebuild each complete predicate body from the same scoped contract; no
    // source lookup positions are used by production matching.
    let compact=mutate_body(0,
        &CONDITIONAL_SOURCE[SPANS[0].0 as usize..SPANS[0].1 as usize],
        "function $THREAD_1_40999($DB) { if (($DB >= $KEY_47 && $DB <= $KEY_97) || ($DB >= $KEY_185 && $DB <= $KEY_40)) return true; else return ($DB === 45); }");
    models(&compact, [true, true]);
    production(&compact, [true, true], false);
    let wrapped=mutate_body(1,
        &CONDITIONAL_SOURCE[SPANS[1].0 as usize..SPANS[1].1 as usize],
        "function $THREAD_1_40999_2($DB) { { if ($DB >= $KEY_47 && $DB <= $KEY_97 || $DB >= $KEY_185 && $DB <= $KEY_40) {{ return (true); }} else {{ return $DB === 45; }} } }");
    models(&wrapped, [true, true]);
    production(&wrapped, [true, true], false);
}
#[test]
fn conditional_whole_body_operator_formal_and_branch_negatives() {
    for index in 0..2 {
        for (from, to) in [
            ("$DB >= $KEY_47", "$KEY_47 <= $DB"),
            ("$DB >= $KEY_47", "$DB > $KEY_47"),
            ("$DB <= $KEY_97", "$DB <= $KEY_114"),
            ("$DB <= $KEY_97", "otherFormal <= $KEY_97"),
            (
                "$DB >= $KEY_47 && $DB <= $KEY_97",
                "$DB <= $KEY_97 && $DB >= $KEY_47",
            ),
            ("return true;", "return 1;"),
            ("return true;", "return \"truthy\";"),
            ("return true;", "effect(); return true;"),
            ("return $DB === 45;", "return $DB;"),
            ("return $DB === 45;", "return $DB == 45;"),
            ("return $DB === 45;", "return $DB === 46;"),
            ("return $DB === 45;", "effect(); return $DB === 45;"),
        ] {
            let source = mutate_body(index, from, to);
            let mut expected = [true, true];
            expected[index] = false;
            models(&source, expected);
        }
        let source = mutate_body(index, "return true;", "return 1;");
        let mut expected = [true, true];
        expected[index] = false;
        production(&source, expected, false);
    }
}
#[test]
fn conditional_uninitialized_partial_conditional_and_deferred_stores_refused() {
    for (from, to) in [
        ("$KEY_47 = $KEY_44(\"0\");", ""),
        (
            "$KEY_47 = $KEY_44(\"0\");",
            "if (flag) $KEY_47 = $KEY_44(\"0\");",
        ),
        (
            "$KEY_47 = $KEY_44(\"0\");",
            "defer(() => $KEY_47 = $KEY_44(\"0\"));",
        ),
        (
            "$KEY_47 = $KEY_44(\"0\");",
            "while(flag) $KEY_47 = $KEY_44(\"0\");",
        ),
        ("$KEY_47 = $KEY_44(\"0\");", "$KEY_47 += $KEY_44(\"0\");"),
        (
            "$KEY_47 = $KEY_44(\"0\");",
            "$KEY_47 = $KEY_44(\"0\"); $KEY_40 = unknown;",
        ),
        ("$KEY_97 = $KEY_44(\"9\");", "$KEY_97 = foreign(\"9\");"),
    ] {
        let source = changed(from, to);
        models(&source, [false, false]);
    }
    let source = changed("$KEY_47 = $KEY_44(\"0\");", "");
    production(&source, [false, false], false);
}
#[test]
fn conditional_early_publication_later_mutation_alias_and_source_negatives() {
    for (from, to) in [
        (
            "$KEY_185 = $KEY_44(\"a\");",
            "publish($THREAD_0_340); $KEY_185 = $KEY_44(\"a\");",
        ),
        (
            "$KEY_185 = $KEY_44(\"a\");",
            "$KEY_249({}); $KEY_185 = $KEY_44(\"a\");",
        ),
        (
            "$KEY_185 = $KEY_44(\"a\");",
            "leak($KEY_249); $KEY_185 = $KEY_44(\"a\");",
        ),
        (
            "let $KEY_249 = $THREAD_6_90164;",
            "let $KEY_249 = laterAlias; let laterAlias = $THREAD_6_90164;",
        ),
        (
            "let $KEY_249 = $THREAD_6_90164;",
            "let $KEY_249 = $THREAD_6_90164; later(() => $KEY_47 = unknown);",
        ),
        (
            "let $KEY_249 = $THREAD_6_90164;",
            "let $KEY_249 = $THREAD_6_90164; let leaked = $KEY_47;",
        ),
        (
            "return sourceString.charCodeAt();",
            "effect(); return sourceString.charCodeAt();",
        ),
    ] {
        let source = changed(from, to);
        models(&source, [false, false]);
    }
    let source = changed(
        "$KEY_185 = $KEY_44(\"a\");",
        "publish($THREAD_0_340); $KEY_185 = $KEY_44(\"a\");",
    );
    production(&source, [false, false], false);
}
