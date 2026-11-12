//! Opt-in naming and annotation of the plan's anonymous functions.
//!
//! Cases build `AnonymousFunctionName` entries against exact function spans and
//! drive `apply_anonymous_names` directly, plus the rename path and the CLI.
//! The assertions cover the exact edit, every refusal rule, marker-only
//! annotation and the CLI map contract.


use oxc_allocator::Allocator;
use oxc_ast::ast::{ArrowFunctionExpression, Function};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use oxc_syntax::scope::ScopeFlags;
use shape::naming::semantic_names::AnonymousFunctionName;
use shape::signals::schema::{Confidence, SourceSpan};
use shape::naming::program::{
    apply_anonymous_names, AnonymousNameAction, AnonymousNameOptions, AnonymousNamesResult,
    NameSkip,
};


const NAME: AnonymousNameOptions = AnonymousNameOptions {
    name: true,
    annotate: false,
};
const BOTH: AnonymousNameOptions = AnonymousNameOptions {
    name: true,
    annotate: true,
};

/// Every function and arrow span in pre-order.
fn function_spans(source: &str) -> Vec<(u32, u32, bool)> {
    struct Collect(Vec<(u32, u32, bool)>);
    impl<'a> Visit<'a> for Collect {
        fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
            self.0.push((function.span.start, function.span.end, false));
            walk::walk_function(self, function, flags);
        }
        fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
            self.0.push((arrow.span.start, arrow.span.end, true));
            walk::walk_arrow_function_expression(self, arrow);
        }
    }
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::unambiguous()).parse();
    assert!(parsed.errors.is_empty(), "test input must parse: {source}");
    let mut collect = Collect(Vec::new());
    collect.visit_program(&parsed.program);
    collect.0
}

/// The entry for the first function node starting at or after `marker`.
fn entry(source: &str, marker: &str, name: &str) -> AnonymousFunctionName {
    let at = source.find(marker).unwrap_or_else(|| panic!("{marker}")) as u32;
    let (start, end, arrow) = function_spans(source)
        .into_iter()
        .filter(|(start, _, _)| *start >= at)
        .min_by_key(|(start, _, _)| *start)
        .expect("function after marker");
    AnonymousFunctionName {
        role: format!("test_{name}"),
        proposed_name: name.to_string(),
        function: SourceSpan::new(start, end),
        id: format!("fn@{start}..{end}"),
        arrow,
        confidence: Confidence::High,
        evidence: vec!["synthetic".to_string()],
    }
}

/// Parse with Oxc and require no parse or semantic early error.
fn assert_parses(source: &str) {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::unambiguous()).parse();
    assert!(
        parsed.errors.is_empty() && !parsed.panicked,
        "output must parse: {:?}\n{source}",
        parsed.errors
    );
    let semantic = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    assert!(
        semantic.errors.is_empty(),
        "output has early errors: {:?}\n{source}",
        semantic.errors
    );
}

fn apply(
    source: &str,
    entries: &[AnonymousFunctionName],
    options: AnonymousNameOptions,
) -> AnonymousNamesResult {
    let result = apply_anonymous_names(source, entries, options).expect("source parses");
    assert_parses(&result.transformed_source);
    result
}

fn name_skip(result: &AnonymousNamesResult, index: usize) -> Option<NameSkip> {
    result
        .of(AnonymousNameAction::Name)
        .nth(index)
        .expect("name outcome")
        .skip_reason
}

#[test]
fn a_plain_callback_function_expression_is_named() {
    let source = "el.addEventListener(\"click\", function (event) { return event.x; });\n";
    let result = apply(source, &[entry(source, "function", "handleClick")], NAME);
    assert_eq!(
        result.transformed_source,
        "el.addEventListener(\"click\", function handleClick(event) { return event.x; });\n"
    );
    assert_eq!((result.named, result.annotated, result.skipped), (1, 0, 0));
    let outcome = &result.outcomes[0];
    assert!(outcome.applied);
    assert_eq!(outcome.node_kind, Some("function_expression"));
}

#[test]
fn a_tight_function_keyword_and_a_generator_get_valid_spacing() {
    let source = "run(function(a){return a}); run(function*(){yield 1}); run(async function(){});\n";
    let entries = [
        entry(source, "function(a)", "first"),
        entry(source, "function*", "second"),
        entry(source, "async", "third"),
    ];
    let result = apply(source, &entries, NAME);
    assert_eq!(
        result.transformed_source,
        "run(function first(a){return a}); run(function*second(){yield 1}); run(async function third(){});\n"
    );
    assert_eq!(result.named, 3);
}

#[test]
fn a_name_the_body_references_is_refused() {
    // `handler` inside resolves to the outer binding; a self-name would
    // capture it.
    let source = "var handler = 1;\nrun(function () { return handler + 1; });\n";
    let result = apply(source, &[entry(source, "function", "handler")], NAME);
    assert_eq!(result.transformed_source, source);
    assert_eq!(name_skip(&result, 0), Some(NameSkip::NameReferencedInside));
}

#[test]
fn a_name_referenced_in_a_parameter_default_is_refused() {
    let source = "var tick = 0;\nrun(function (a = tick) { return a; });\n";
    let result = apply(source, &[entry(source, "function", "tick")], NAME);
    assert_eq!(result.transformed_source, source);
    assert_eq!(name_skip(&result, 0), Some(NameSkip::NameReferencedInside));
}

#[test]
fn a_name_bound_inside_the_function_is_refused() {
    let param = "run(function (step) { return step; });\n";
    let local = "run(function () { var step = 2; return 1; });\n";
    let nested = "run(function () { return [1].map(function step(x) { return x; }); });\n";
    for source in [param, local, nested] {
        let result = apply(source, &[entry(source, "function", "step")], NAME);
        assert_eq!(result.transformed_source, source);
        assert_eq!(name_skip(&result, 0), Some(NameSkip::NameBoundInside), "{source}");
    }
}

#[test]
fn a_same_spelling_member_name_or_string_is_not_a_conflict() {
    let source = "run(function (o) { return o.onTick + \"onTick\"; });\n";
    let result = apply(source, &[entry(source, "function", "onTick")], NAME);
    assert!(result.transformed_source.contains("function onTick(o)"));
}

#[test]
fn dynamic_scope_and_callee_reads_are_refused() {
    let cases = [
        ("run(function () { return eval(\"1\"); });\n", NameSkip::DirectEval),
        ("run(function (o) { with (o) { return x; } });\n", NameSkip::WithStatement),
        ("run(function () { return arguments.callee; });\n", NameSkip::ArgumentsCallee),
        ("run(function (k) { return arguments[k]; });\n", NameSkip::ArgumentsCallee),
    ];
    for (source, reason) in cases {
        let result = apply(source, &[entry(source, "function", "safeName")], NAME);
        assert_eq!(result.transformed_source, source);
        assert_eq!(name_skip(&result, 0), Some(reason), "{source}");
    }
    // A numeric `arguments` read is not `callee`.
    let source = "run(function () { return arguments[0]; });\n";
    let result = apply(source, &[entry(source, "function", "safeName")], NAME);
    assert_eq!(result.named, 1);
}

#[test]
fn an_inferred_name_initializer_is_refused_unless_it_matches() {
    let cases = [
        "var onLoad = function () {};\n",
        "onLoad = function () {};\n",
        "onLoad ||= function () {};\n",
        "var o = { onLoad: function () {} };\n",
        "class C { onLoad = function () {}; }\n",
        "function f(onLoad = function () {}) {}\n",
        "var { onLoad = function () {} } = {};\n",
        "({ onLoad = function () {} } = {});\n",
    ];
    for source in cases {
        let marker = if source.starts_with("function f") {
            "= function"
        } else {
            "function ("
        };
        let refused = apply(source, &[entry(source, marker, "differentName")], NAME);
        assert_eq!(refused.transformed_source, source);
        assert_eq!(
            name_skip(&refused, 0),
            Some(NameSkip::InferredNameDiffers),
            "{source}"
        );
        assert_eq!(
            refused.outcomes[0].inferred_name.as_deref(),
            Some("onLoad"),
            "{source}"
        );
        let matching = apply(source, &[entry(source, marker, "onLoad")], NAME);
        assert_eq!(matching.named, 1, "{source}");
        assert!(
            matching.transformed_source.contains("function onLoad("),
            "{}",
            matching.transformed_source
        );
    }
    // A computed key's name is only known at runtime.
    let source = "var o = { [key]: function () {} };\n";
    let result = apply(source, &[entry(source, "function", "anything")], NAME);
    assert_eq!(name_skip(&result, 0), Some(NameSkip::InferredNameUnknown));
    // A member assignment infers nothing; the rename applies.
    let source = "o.onLoad = function () {};\n";
    let result = apply(source, &[entry(source, "function", "handleLoad")], NAME);
    assert_eq!(result.named, 1);
}

#[test]
fn arrows_are_only_annotated() {
    let source = "list.map((item) => item.id).filter(async x => x);\n";
    let entries = [
        entry(source, "(item)", "readItemId"),
        entry(source, "async", "keepTruthy"),
    ];
    let result = apply(source, &entries, BOTH);
    assert_eq!(
        result.transformed_source,
        "list.map(/* readItemId */ (item) => item.id).filter(/* keepTruthy */ async x => x);\n"
    );
    assert_eq!((result.named, result.annotated), (0, 2));
    for outcome in result.of(AnonymousNameAction::Name) {
        assert_eq!(outcome.skip_reason, Some(NameSkip::ArrowFunction));
        assert_eq!(outcome.node_kind, Some("arrow"));
    }
}

