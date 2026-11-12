use super::*;

const DIRECT: &str = "return x >= a && x <= b || x >= c && x <= d || x === 45;";
const SPLIT: &str = "let flag; if (x >= a) flag = x <= b; else flag = false; return flag || x >= c && x <= d || x === 45;";
const STORES: &str = "a = scalar('j'); b = scalar('q'); c = scalar('v'); d = scalar('z');";
fn context(body: &str, lifetime: usize) -> String {
    let setup = match lifetime {
        0 => "owner([]);".to_string(),
        1 => format!("let scalar = function(s) {{ return s.charCodeAt(); }}; {STORES} function entry() {{ owner([]); }} entry();"),
        2 => format!("let scalar = function(s) {{ return s.charCodeAt(); }}; if (choice) {{ {STORES} function firstEntry() {{ owner([]); }} firstEntry(); }} else {{ {STORES} function secondEntry() {{ owner([]); }} secondEntry(); }}"),
        _ => unreachable!(),
    };
    format!("(function() {{ 'use strict'; let a,b,c,d; function owner(values) {{ function $THREAD_0_700001(x) {{ {body} }} return values.every($THREAD_0_700001); }} {setup} }})();")
}
fn check(source: &str, body_ok: bool, lifetime_ok: bool) {
    let allocator = Allocator::default();
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
    let site = *model.all_sites().iter().find(|s| matches!(model.nodes.kind(s.node_id), AstKind::Function(f) if f.id.as_ref().is_some_and(|id| id.name == "$THREAD_0_700001"))).unwrap();
    let bounds = flow_parts(&model, site);
    assert_eq!(bounds.is_some(), body_ok, "{source}");
    let accepted = bounds.is_some_and(|bounds| {
        uninitialized_bounds(&model, site, bounds).is_some()
            || initialized_bounds(&model, site, bounds).is_some()
            || conditional_initialized_bounds(&model, site, bounds).is_some()
    });
    assert_eq!(accepted, lifetime_ok, "{source}");
}
#[test]
fn self_authored_multivariant_grammar_and_three_distinct_lifetimes() {
    for body in [DIRECT, SPLIT,
        "if (x >= a && x <= b || x >= c && x <= d) return true; else return x === 45;",
        "let first = x >= a && x <= b; if (first) return first; let second = x >= c && x <= d; return second || x === 45;",
        "let result; if (x >= a && x <= b) result = true; else if (x >= c) result = x <= d; else result = false; if (result) return result; return x === 45;",
    ] {
        for lifetime in 0..3 { check(&context(body, lifetime), true, true); }
    }
}
#[test]
fn heldout_nested_blocks_and_multiple_boolean_copies() {
    let body = "let first; { if (x >= a) { first = x <= b; } else { first = false; } } let saved = first; if (saved) { return true; } if (x >= c && x <= d) { return true; } return x === 45;";
    check(&context(body, 0), true, true);
    check(&context(body, 1).replace("values", "receiver"), true, true);
}
#[test]
fn coercion_order_repetition_early_evaluation_and_non_boolean_negatives() {
    for body in [
        "return x <= b && x >= a || x >= c && x <= d || x === 45;",
        "return a <= x && x <= b || x >= c && x <= d || x === 45;",
        "let r; if (x >= a && x <= b) r = true; else if (x >= c) r = x <= d; else r = x >= c; return r || x === 45;",
        "let r = x >= c && x <= d; return x >= a && x <= b || r || x === 45;",
        "let r; if (x >= a) r = x <= b; return r || x >= c && x <= d || x === 45;",
        "let r = x >= a && x <= b; return r || x >= c && x <= d || 45;",
        "let r = x >= a && x <= b; effect(r); return r || x >= c && x <= d || x === 45;",
        "let r = x >= a && x <= b; function leak() { return r; } return r || x >= c && x <= d || x === 45;",
        "let r = x >= a && x <= b; return r || x >= c && x <= d || x == 45;",
        "let a = true; return x >= a && x <= b || x >= c && x <= d || x === 45;",
    ] { check(&context(body, 0), false, false); }
}
#[test]
fn bad_capture_initialization_helper_scope_receiver_and_publication() {
    for lifetime in 0..3 {
        let source = context(SPLIT, lifetime);
        for changed in [
            source.replace("let a,b,c,d;", "let a,b,c,d; later(() => a = 1);"),
            source.replace("let a,b,c,d;", "let a,b,c,d; publish(owner);"),
            source.replace(
                "values.every($THREAD_0_700001)",
                "values.every($THREAD_0_700001, extra)",
            ),
            source.replace("values.every($THREAD_0_700001)", "expose($THREAD_0_700001)"),
            source.replace(
                "values.every($THREAD_0_700001)",
                "values.every?.($THREAD_0_700001)",
            ),
        ] {
            check(&changed, true, false);
        }
    }
    let source = context(SPLIT, 1);
    for changed in [
        source.replace("a = scalar('j');", "if (choice) a = scalar('j');"),
        source.replace("a = scalar('j');", ""),
        source.replace("a = scalar('j');", "owner([]); a = scalar('j');"),
        source.replace("d = scalar('z');", "d = foreign('z');"),
        source.replace("return s.charCodeAt();", "effect(); return s.charCodeAt();"),
        source.replace(
            "a = scalar('j');",
            "{ let alias = scalar; } a = alias('j');",
        ),
    ] {
        check(&changed, true, false);
    }
    // An out-of-scope alias supplies no accepted producer origin.
    check(
        &source.replace("a = scalar('j');", "a = alias('j');"),
        true,
        false,
    );
}
#[test]
fn old_direct_proof_and_proposal_explanations_keep_priority() {
    let source = context(DIRECT, 0);
    let result = name_threads(&source, true).unwrap();
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_0_700001")
        .unwrap();
    assert!(outcome.applied);
    assert_eq!(
        outcome.evaluator.as_deref(),
        Some("signals_captured_range_predicate")
    );
    assert!(outcome
        .explanation
        .iter()
        .any(|s| s.contains("entire one-formal return")));
    let source = context(SPLIT, 0);
    let result = name_threads(&source, true).unwrap();
    let matches: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|o| o.applied && o.evaluator.as_deref() == Some("signals_captured_range_predicate"))
        .collect();
    assert_eq!(matches.len(), 1, "never label an enclosing function");
    assert_eq!(matches[0].binding.name, "$THREAD_0_700001");
    let mut replay = source.clone();
    for edit in result.report.edits.iter().rev() {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
}
