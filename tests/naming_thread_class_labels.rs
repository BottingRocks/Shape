use shape::naming::{name_threads, ThreadClassName};
use shape::naming::program::{transform_program, ProgramBindingRename, ProgramTransformOptions};

// Complete existing source-backed observable witness, including the captured
// callable origins checked by the family proof. Never execute recovered JS.
const SOURCE: &str = include_str!("fixtures/program/thread-owned-observable.js");
const CLASS: &str = "$THREAD_6_110488";

fn class_result(source: &str) -> ThreadClassName {
    let result = name_threads(source, true).unwrap();
    assert_eq!(
        result.report.generated_thread_count,
        result.report.outcomes.len()
    );
    assert_eq!(
        result.report.generated_class_count,
        result.report.class_outcomes.len()
    );
    assert_eq!(
        result.report.applied_class_count + result.report.unresolved_class_count,
        result.report.generated_class_count
    );
    result
        .report
        .class_outcomes
        .into_iter()
        .find(|o| o.binding.name == CLASS)
        .unwrap()
}

#[test]
fn owned_class_and_self_references_are_separate_from_function_labels() {
    for source in [
        SOURCE.to_string(),
        SOURCE
            .replace("new $KEY_0(", &format!("new {CLASS}("))
            .replace("instanceof $KEY_0", &format!("instanceof {CLASS}")),
    ] {
        let result = name_threads(&source, true).unwrap();
        let class = class_result(&source);
        assert!(class.applied, "{class:?}");
        assert_eq!(class.proposed_name.as_deref(), Some("Observable"));
        assert_eq!(class.evaluator.as_deref(), Some("owned_observable_class"));
        assert!(class.reference_edits > 0);
        assert!(!result.source.contains(CLASS));
        assert!(result.source.contains("constructor("));
        assert!(result.source.contains("static from("));
        assert!(result
            .report
            .outcomes
            .iter()
            .all(|o| o.binding.name != CLASS));
        let mut replay = source;
        let mut edits = result.report.edits;
        edits.sort_by_key(|e| e.start);
        for e in edits.iter().rev() {
            replay.replace_range(e.start as usize..e.end as usize, &e.replacement);
        }
        assert_eq!(replay, result.source);
    }
}

#[test]
fn spelling_and_nested_scope_do_not_change_the_class_proof() {
    let alpha = SOURCE
        .replace(CLASS, "$THREAD_999_777_2")
        .replace("$KEY_0", "differentAlias")
        .replace("$BS", "inputSubscriber");
    for source in [alpha.clone(), format!("function enclosing() {{ {alpha} }}")] {
        let result = name_threads(&source, true).unwrap();
        let class = &result.report.class_outcomes[0];
        assert!(class.applied, "{class:?}");
        assert_eq!(class.proposed_name.as_deref(), Some("Observable"));
    }
    // Independent lexical scopes may each use the same semantic role.
    // Keep the existing MAIN selector unambiguous; its policy is unchanged.
    let second = alpha.replace("function bootstrap(){", "function bootstrap(){ let extra = 1;");
    let source = format!("function a() {{ {SOURCE} }} function b() {{ {second} }}");
    assert_eq!(
        name_threads(&source, true)
            .unwrap()
            .report
            .applied_class_count,
        2
    );
}

#[test]
fn report_only_and_readable_classes_are_preserved() {
    let result = name_threads(SOURCE, false).unwrap();
    assert_eq!(result.source, SOURCE);
    assert_eq!(result.report.applied_class_count, 0);
    assert_eq!(
        result.report.class_outcomes[0].proposed_name.as_deref(),
        Some("Observable")
    );
    assert!(result.report.edits.is_empty());
    let human = SOURCE.replace(CLASS, "UserObservable");
    let result = name_threads(&human, true).unwrap();
    assert_eq!(result.report.generated_class_count, 0);
    assert!(result.source.contains("class UserObservable"));
}

#[test]
fn collision_capture_eval_and_with_are_refused() {
    let collision = SOURCE.replace("$KEY_0", "Observable");
    let outcome = class_result(&collision);
    assert!(!outcome.applied);
    assert_eq!(outcome.reason.as_deref(), Some("NameCollision"));
    // Free uses inside a descendant scope would be captured by the class name.
    let capture = SOURCE.replace("$KEY_378", "Observable");
    let outcome = class_result(&capture);
    assert!(!outcome.applied);
    assert_eq!(outcome.reason.as_deref(), Some("NameCollision"));
    for source in [
        format!("eval('');{SOURCE}"),
        format!("with (context) {{ {SOURCE} }}"),
    ] {
        let class = class_result(&source);
        assert!(!class.applied);
        assert_eq!(class.reason.as_deref(), Some("DynamicScope"));
    }
}

#[test]
fn donor_mutation_and_partial_property_bags_do_not_authorize_a_class() {
    for (from, to) in [
        ("let $KEY_0 = $THREAD_6_110488;", "let $KEY_0 = other;"),
        ("return $KEY_0;", "return unrelated;"),
        (
            "let $KEY_0 = $THREAD_6_110488;",
            "let $KEY_0 = $THREAD_6_110488; $KEY_0 = other;",
        ),
        (
            "return $KEY_0;",
            " $THREAD_6_110488 = other; return $KEY_0;",
        ),
        ("pipe() {", "static pipe() {"),
        ("static of() {", "static of() {} static of() {"),
        ("this._subscriber = $BS", "other._subscriber = $BS"),
        (
            "class $THREAD_6_110488 {",
            "class $THREAD_6_110488 extends Base {",
        ),
    ] {
        assert!(SOURCE.contains(from));
        let class = class_result(&SOURCE.replace(from, to));
        assert!(!class.applied, "{from}: {class:?}");
        assert!(class.reason.is_some());
    }
    let source = format!(
        "function donor() {{ {SOURCE} }} class $THREAD_3_4 {{ subscribe() {{}} pipe() {{}} }}"
    );
    let result = name_threads(&source, true).unwrap();
    let donor = result
        .report
        .class_outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_3_4")
        .unwrap();
    assert!(!donor.applied);
    assert!(donor
        .reason
        .as_deref()
        .unwrap()
        .contains("pending family proof"));
}

#[test]
fn class_expressions_and_private_self_are_explicitly_refused() {
    let source = "function bootstrap() { let $THREAD_1_2 = class $THREAD_3_4 { #self() { return $THREAD_3_4; } }; let $THREAD_5_6 = class {}; let $THREAD_7_8 = class HumanSelf {}; } bootstrap();";
    let result = name_threads(source, true).unwrap();
    assert_eq!(result.source, source);
    assert_eq!(result.report.generated_class_count, 3);
    assert!(result
        .report
        .class_outcomes
        .iter()
        .all(|o| !o.applied && o.reason.as_deref().unwrap().contains("class expression")));
}

#[test]
fn checked_class_renamer_preserves_new_extends_alias_shorthand_and_shadow_identity() {
    let source = "class $THREAD_1_2 { self() { return $THREAD_1_2; } } const alias = $THREAD_1_2; const bag = {$THREAD_1_2}; class Child extends $THREAD_1_2 {} new $THREAD_1_2(); function shadow($THREAD_1_2) { return $THREAD_1_2; }";
    let offset = source.find("$THREAD_1_2").unwrap() as u32;
    let result = transform_program(
        source,
        &[ProgramBindingRename::new(offset, "Observable")],
        &[],
        &ProgramTransformOptions::default(),
    )
    .unwrap();
    assert!(result.renames[0].applied, "{:?}", result.renames);
    assert_eq!(result.renames[0].reference_edits, 5);
    assert_eq!(result.renames[0].shorthand_edits, 1);
    assert!(result.transformed_source.contains("return Observable"));
    assert!(result
        .transformed_source
        .contains("{$THREAD_1_2: Observable}"));
    assert!(result.transformed_source.contains("extends Observable"));
    assert!(result.transformed_source.contains("new Observable()"));
    assert!(result
        .transformed_source
        .contains("function shadow($THREAD_1_2) { return $THREAD_1_2; }"));
}
