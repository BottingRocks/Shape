//! Complete owned query/read/emission protocol with an opaque receiver. Real factory bodies
//! retain one trailing newline. Sibling setup and hook activation provide no evidence for
//! this rule.
use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use sha2::{Digest, Sha256};
use shape::naming::name_threads;

const PRIMARY: &str = include_str!("fixtures/program/remember-checkbox-baseline.js");
const ALTERNATE: &str = include_str!("fixtures/program/remember-checkbox-alternate.js");
const ROLE: &str = "queryRememberCheckboxAndEmitState";
const EVALUATOR: &str = "events_remember_checkbox_protocol";

fn check(source: &str, sampler: &str, accepted: bool) {
    let source = format!("function main(){{\n{source}\nreturn 0;\n}}main();");
    // The API parses, scopes and constructs an actual Model. A malformed
    // negative is an error, never evidence of safe matcher refusal.
    let result = name_threads(&source, true).expect("fixture must build a valid Model");
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == sampler)
        .expect("sampler is indexed");
    if accepted {
        assert!(outcome.applied, "{outcome:?}");
        assert_eq!(outcome.role_name.as_deref(), Some(ROLE));
        assert_eq!(outcome.evaluator.as_deref(), Some(EVALUATOR));
        assert!(outcome
            .explanation
            .iter()
            .any(|s| s.contains("native DOM identity is unproved")));
        assert!(outcome
            .explanation
            .iter()
            .any(|s| s.contains("hook activation order is opaque")));
    } else {
        assert_ne!(outcome.role_name.as_deref(), Some(ROLE), "{outcome:?}");
        assert_ne!(outcome.evaluator.as_deref(), Some(EVALUATOR), "{outcome:?}");
    }
    // Replay the exact scoped edit ledger, including existing setup names.
    let mut replay = source.clone();
    let mut edits = result.report.edits.clone();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    for edit in edits {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
    for key in [
        "getElementById",
        "getElementsByName",
        "checked",
        "className",
        "indexOf",
        "next",
        "setup",
        "lastChance",
    ] {
        assert_eq!(
            source.matches(key).count(),
            result.source.matches(key).count(),
            "{key}"
        );
    }
}

fn own_body() -> &'static str {
    // The primary callback remains inside its original parent.
    &PRIMARY[34..386]
}

#[test]
fn real_primary_and_different_capture_variant_are_protocol_positives() {
    check(PRIMARY, "$THREAD_8_18384", true);
    check(ALTERNATE, "$THREAD_5_105024", true);
}

#[test]
fn own_protocol_needs_no_sibling_setup_or_native_origin() {
    let source = format!("let $KEY_0; {}", own_body());
    check(&source, "$THREAD_8_18384", true);
    let source = format!("{} let $KEY_0;", own_body());
    check(&source, "$THREAD_8_18384", true);
    let alpha = source
        .replace("$THREAD_8_18384", "$THREAD_33_700")
        .replace("$KEY_0", "queryPort")
        .replace("$KEY_4", "selected")
        .replace("$AJ", "sink")
        .replace("$AK", "matches");
    check(&alpha, "$THREAD_33_700", true);
}

#[test]
fn receiver_origin_is_not_a_native_or_hook_claim() {
    // Even an explicitly non-native supplier does not alter the owned protocol.
    let changed = PRIMARY.replace(".global.document", ".global.navigator");
    check(&changed, "$THREAD_8_18384", true);
    for calls in [
        "hooks.setup(context); hooks.lastChance(observer);",
        "hooks.lastChance(observer); hooks.setup(context);",
    ] {
        let source = format!("{PRIMARY}\nconst hooks = $THREAD_4_11611();\n{calls}\n");
        check(&source, "$THREAD_8_18384", true);
    }
}

#[test]
fn exact_value_graph_and_selectors_are_required() {
    for (before, after) in [
        (".checked", ".disabled"),
        (".next($KEY_4)", ".next($AK)"),
        (".next($KEY_4)", ".next($KEY_4.checked)"),
        (".next($KEY_4)", ".error($KEY_4)"),
        ("$AJ.next", "other.next"),
        ("indexOf(\"checked\") > -1", "indexOf(\"checked\") === -1"),
        ("indexOf(\"checked\")", "indexOf(\"selected\")"),
        ("\"rememberMe\"", "\"subscribeToNewsletter\""),
        ("\"TravelerLoginForm_rememberMe\"", "\"otherCheckbox\""),
        ("\"shouldRememberUser\"", "\"otherName\""),
        (": \"\";", ": false;"),
        ("$AK.length > 0", "$AK.length >= 0"),
        ("$AK[0]", "$AK[1]"),
        ("$AK[0]", "other[0]"),
        ("$KEY_4 != null", "$KEY_4 == null"),
        ("$KEY_4.checked", "other.checked"),
        ("|| $KEY_0.getElementById", "&& $KEY_0.getElementById"),
        ("$KEY_0.getElementsByName", "other.getElementsByName"),
        (
            "$KEY_0.getElementById(\"TravelerLoginForm_rememberMe\")",
            "other.getElementById(\"TravelerLoginForm_rememberMe\")",
        ),
        ("$KEY_4 = $KEY_4 != null", "$AK = $KEY_4 != null"),
        (".next($KEY_4)", ".next(...$KEY_4)"),
        (".next($KEY_4)", ".next($KEY_4, $AK)"),
        ("$AJ.next($KEY_4)", "$AJ.next?.($KEY_4)"),
    ] {
        assert_eq!(
            PRIMARY.matches(before).count(),
            1,
            "mutation must hit actual evidence: {before}"
        );
        let changed = format!("let other; {}", PRIMARY.replacen(before, after, 1));
        check(&changed, "$THREAD_8_18384", false);
    }
}

#[test]
fn shadowing_and_borrowed_neighbor_literals_cannot_complete_a_protocol() {
    // The formal now shadows the outer receiver, so its query calls are not
    // on a lexical capture even though the source spelling still matches.
    let shadowed = own_body().replace("$AJ", "$KEY_0");
    check(&format!("let $KEY_0; {shadowed}"), "$THREAD_8_18384", false);
    // Same outer spelling in a nested block resolves to a different symbol.
    let shadowed = own_body().replacen("($AJ) {", "($AJ) { let $KEY_0 = foreign;", 1);
    check(
        &format!("let $KEY_0, foreign; {shadowed}"),
        "$THREAD_8_18384",
        false,
    );
    let changed = PRIMARY.replace("\"shouldRememberUser\"", "\"otherName\"");
    let decoy = format!("{changed}\nfunction neighbor(){{return \"shouldRememberUser\";}}");
    check(&decoy, "$THREAD_8_18384", false);
}

#[test]
fn incomplete_body_and_reordered_operations_fail() {
    let incomplete = "let receiver; function $THREAD_8_18384(sink){receiver.getElementById(\"rememberMe\"); sink.next(false);}";
    check(incomplete, "$THREAD_8_18384", false);
    let changed = PRIMARY.replace("$AJ.next($KEY_4);", "$AJ.next($KEY_4); return $KEY_4;");
    check(&changed, "$THREAD_8_18384", false);
    let changed = PRIMARY.replace("$AJ.next($KEY_4);", "").replace(
        "$KEY_4 = $KEY_4 != null",
        "$AJ.next($KEY_4); $KEY_4 = $KEY_4 != null",
    );
    check(&changed, "$THREAD_8_18384", false);
}

#[test]
fn real_captured_references_resolve_to_one_opaque_binding() {
    for (source, capture, sampler, setup) in [
        (PRIMARY, "$KEY_0", "$THREAD_8_18384", "$THREAD_4_49952"),
        (ALTERNATE, "$KEY_1", "$THREAD_5_105024", "$THREAD_8_131385"),
    ] {
        let allocator = Allocator::default();
        let parsed = shape::analysis::parse(&allocator, source);
        assert!(
            !parsed.panicked && parsed.errors.is_empty(),
            "{:?}",
            parsed.errors
        );
        let (scoping, nodes) = shape::analysis::analyze(&parsed.program);
        let declarations: Vec<_> = nodes
            .iter()
            .filter_map(|n| {
                let AstKind::VariableDeclarator(v) = n.kind() else {
                    return None;
                };
                let oxc_ast::ast::BindingPattern::BindingIdentifier(id) = &v.id else {
                    return None;
                };
                (id.name == capture).then_some((v, id.symbol_id.get().unwrap()))
            })
            .collect();
        assert_eq!(declarations.len(), 1);
        let (declaration, symbol) = declarations[0];
        assert!(declaration.init.is_none(), "native origin remains unproved");
        assert!(scoping.symbol_redeclarations(symbol).is_empty());
        let references: Vec<_> = scoping.get_resolved_references(symbol).collect();
        assert_eq!(references.len(), 4);
        assert_eq!(references.iter().filter(|r| r.is_write()).count(), 1);
        for reference in references {
            let mut parent = nodes.parent_id(reference.node_id());
            loop {
                if let AstKind::Function(function) = nodes.kind(parent) {
                    assert_eq!(
                        function.id.as_ref().unwrap().name.as_str(),
                        if reference.is_write() { setup } else { sampler }
                    );
                    break;
                }
                parent = nodes.parent_id(parent);
            }
        }
    }
}

#[test]
fn fixtures_are_pinned_exact_source_slices() {
    for (source, expected) in [
        (
            PRIMARY,
            "e408b56cb5c55330b033b1db66c2ed544ba8de377355c5cb96c84c9512518ea2",
        ),
        (
            ALTERNATE,
            "23962ce5d21e458ebdd73f155e618d3162659d6e2a3d5cca5ffaabc2e6c87395",
        ),
    ] {
        let digest: String = Sha256::digest(source.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(digest, expected);
    }
}
