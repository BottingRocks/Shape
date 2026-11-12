//! Rename-safety regressions for the program rewrite engine.
//!
//! Identity-rename safety probes with their expected
//! outcome, plus the same-scope collision forms the engine must refuse: an
//! unused binding of the replacement name that shares a declaration scope
//! (after `var` hoisting) with the renamed binding. Distinct symbols must
//! never merge because they received the same name, and sibling scopes that
//! cannot see each other may still share one.
//!
//! Every case uses a synthetic source.


use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use shape::naming::program::{
    transform_program, ProgramBindingRename, ProgramTransformOptions, ProgramTransformResult,
    RenameSkipReason,
};

fn options() -> ProgramTransformOptions {
    ProgramTransformOptions {
        rename_bindings: true,
        annotate_signals: false,
    }
}

/// Oxc parse plus semantic early errors (redeclarations included).
fn semantic_errors(source: &str) -> Vec<String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::unambiguous()).parse();
    let mut errors: Vec<String> = parsed.errors.iter().map(ToString::to_string).collect();
    errors.extend(
        SemanticBuilder::new()
            .with_check_syntax_error(true)
            .build(&parsed.program)
            .errors
            .iter()
            .map(ToString::to_string),
    );
    errors
}

fn transform(source: &str, plans: &[(&str, &str)]) -> ProgramTransformResult {
    assert!(
        semantic_errors(source).is_empty(),
        "test input is invalid: {source}"
    );
    let renames: Vec<_> = plans
        .iter()
        .map(|(target, name)| {
            let start = source.find(target).unwrap_or_else(|| panic!("{target}")) as u32;
            ProgramBindingRename::new(start, *name)
        })
        .collect();
    let result = transform_program(source, &renames, &[], &options()).expect("transform");
    // Whatever the outcome, the engine never returns invalid output.
    assert!(
        semantic_errors(&result.transformed_source).is_empty(),
        "invalid output: {}",
        result.transformed_source
    );
    result
}

/// Rename the binding whose identifier starts at the first `target` and
/// assert it is applied with exactly `expected` as output.
fn applies(source: &str, target: &str, name: &str, expected: &str) {
    let result = transform(source, &[(target, name)]);
    assert!(
        result.renames[0].applied,
        "{source}: {:?} {:?}",
        result.renames[0].skip_reason, result.diagnostics
    );
    assert_eq!(result.transformed_source, expected);
}

/// Rename and assert it is skipped for `reason`, leaving the source unchanged.
fn skips(source: &str, target: &str, name: &str, reason: RenameSkipReason) {
    let result = transform(source, &[(target, name)]);
    assert!(
        !result.renames[0].applied,
        "{source} was renamed to: {}",
        result.transformed_source
    );
    assert_eq!(result.renames[0].skip_reason, Some(reason), "{source}");
    assert_eq!(result.transformed_source, source);
}

fn collides(source: &str, target: &str, name: &str) {
    skips(source, target, name, RenameSkipReason::NameCollision);
}

// ----- Rename safety probes -----

#[test]
fn probe_unreferenced_same_scope_var_is_refused() {
    // Applied before: `var foo = 5; var foo = function…` merged two bindings.
    collides(
        "function m(){ var foo = 5; var x = function(){ return 1 }; return x(); } m();",
        "x =",
        "foo",
    );
}

#[test]
fn probe_unreferenced_same_scope_let_is_refused() {
    // Applied before: `let foo = 5; let foo = 1;` is a SyntaxError.
    collides(
        "function m(){ let foo = 5; let x = 1; return x; } m();",
        "x =",
        "foo",
    );
}

#[test]
fn probe_referenced_same_scope_is_refused() {
    collides(
        "function m(){ let foo = 5; let x = 1; return x + foo; } m();",
        "x =",
        "foo",
    );
}

#[test]
fn probe_eval_and_arguments_names_are_refused() {
    skips(
        "function m(){ let x = function(s){ return s }; return x('1'); } m();",
        "x =",
        "eval",
        RenameSkipReason::InvalidCanonicalName,
    );
    skips(
        "function m(){ let x = 1; return x; } m();",
        "x =",
        "arguments",
        RenameSkipReason::InvalidCanonicalName,
    );
}

#[test]
fn probe_undefined_used_as_a_free_global_is_refused() {
    collides(
        "function m(){ let x = 1; return [x, undefined]; } m();",
        "x =",
        "undefined",
    );
}

#[test]
fn probe_undefined_without_a_global_use_is_a_valid_local_shadow() {
    // Inside a function no free `undefined` is captured, so this is valid.
    applies(
        "function m(){ let x = 1; return x; } m();",
        "x =",
        "undefined",
        "function m(){ let undefined = 1; return undefined; } m();",
    );
}

#[test]
fn non_configurable_globals_are_refused_at_script_top_level() {
    // A top-level `var undefined = 1` is silently ignored and a top-level
    // `let NaN` throws: both change behavior.
    for name in ["undefined", "NaN", "Infinity"] {
        collides("var x = 1; x;", "x =", name);
        collides("let x = 1; x;", "x =", name);
    }
}

#[test]
fn probe_shorthand_is_expanded() {
    applies(
        "function m(){ let x = 1; return { x }; } m();",
        "x =",
        "count",
        "function m(){ let count = 1; return { x: count }; } m();",
    );
}

#[test]
fn probe_recursive_declaration() {
    applies(
        "function m(){ function f(n){ return n ? f(n - 1) : 0 } return f(3); } m();",
        "f(n)",
        "countDown",
        "function m(){ function countDown(n){ return n ? countDown(n - 1) : 0 } return countDown(3); } m();",
    );
}

#[test]
fn probe_named_function_expression_outer_and_inner() {
    let source = "function m(){ let a = function b(){ return b; }; return a(); } m();";
    applies(
        source,
        "a =",
        "outer",
        "function m(){ let outer = function b(){ return b; }; return outer(); } m();",
    );
    applies(
        source,
        "b()",
        "inner",
        "function m(){ let a = function inner(){ return inner; }; return a(); } m();",
    );
}

