use super::*;

// Run the unchanged meaningful integration cases in this focused lib-test
// executable too, avoiding any need to rebuild the application binary.
mod retained {
    use crate as shape;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/naming_operation_captured_key_list_unregister.rs"
    ));
}

#[test]
fn enumerated_function_expression_is_refused() {
    const ORIGINAL: &str = include_str!(
        "../../../tests/fixtures/operation-captured-key-list-unregister/stateful-parent-expanded.js"
    );
    let source = ORIGINAL.replace(
        "function $THREAD_3_86302()",
        "let $THREAD_3_86302 = function privateSelf()",
    );
    assert_ne!(source, ORIGINAL);
    let allocator = Allocator::default();
    let parsed = crate::analysis::parse(&allocator, &source);
    assert!(!parsed.panicked && parsed.errors.is_empty(), "must parse");
    let built = oxc_semantic::SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    assert!(
        built.errors.is_empty(),
        "must be semantically valid: {:?}",
        built.errors
    );
    let model = Model::new(
        &source,
        &parsed.program,
        built.semantic.scoping(),
        built.semantic.nodes(),
        ProgramPropertiesLimits::default(),
    )
    .expect("actual parsed semantic model");
    let sites: Vec<_> = model
        .all_sites()
        .iter()
        .copied()
        .filter(|site| {
            matches!(model.nodes.kind(site.node_id), AstKind::Function(f)
            if f.id.as_ref().is_some_and(|id| id.name == "privateSelf"))
        })
        .collect();
    assert_eq!(sites.len(), 1, "the exact target must be enumerated");
    let site = sites[0];
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        panic!("enumerated target must be an actual Function node");
    };
    assert_eq!(function.r#type, FunctionType::FunctionExpression);
    assert_eq!(site.span, function.span);
    assert_eq!(site.body_span, function.body.as_ref().map(|b| b.span));
    assert!(source[site.span.start as usize..site.span.end as usize]
        .starts_with("function privateSelf()"));
    let private_symbol = function
        .id
        .as_ref()
        .unwrap()
        .symbol_id
        .get()
        .expect("private self symbol");
    assert_eq!(site.binding_symbol, Some(private_symbol));
    assert_eq!(model.scoping.symbol_name(private_symbol), "privateSelf");
    let AstKind::VariableDeclarator(declarator) = model.nodes.parent_kind(site.node_id) else {
        panic!("target must initialize the intended outer variable");
    };
    let BindingPattern::BindingIdentifier(outer) = &declarator.id else {
        panic!("outer candidate must have a plain binding");
    };
    assert_eq!(outer.name, "$THREAD_3_86302");
    let outer_symbol = outer.symbol_id.get().expect("outer candidate symbol");
    assert_ne!(
        outer_symbol, private_symbol,
        "private self is not the external candidate binding"
    );
    assert!(
        matches!(declarator.init.as_ref(), Some(Expression::FunctionExpression(f)) if f.span == site.span)
    );
    assert_eq!(
        captured_symbol(&model, site, private_symbol),
        None,
        "private self never proves an external capture"
    );

    let invocations = std::cell::Cell::new(0usize);
    let invoke_exact_target = || {
        invocations.set(invocations.get() + 1);
        matches_operation(&model, site)
    };
    let matched = invoke_exact_target();
    assert_eq!(invocations.get(), 1);
    assert!(
        matched.is_none(),
        "production matcher must explicitly refuse this actual target"
    );
    assert!(
        !matched.is_some(),
        "no accepted reviewed operation/role/evaluator"
    );
    println!(
        "R10_EXACT_TARGET_NEGATIVE {}",
        serde_json::json!({
            "node_id":format!("{:?}",site.node_id),
            "span":{"start":site.span.start,"end":site.span.end},
            "node_kind":"Function", "function_type":"FunctionExpression",
            "target_exists":true, "enumerated_sites_matching_target":sites.len(),
            "outer_binding":outer.name.as_str(), "outer_symbol":format!("{outer_symbol:?}"),
            "site_binding":"privateSelf", "private_symbol":format!("{private_symbol:?}"),
            "private_self_external_capture":false,
            "parse_errors":0, "semantic_errors":0, "production_matcher_invocations":invocations.get(),
            "accepted":false, "accepted_role":null, "accepted_evaluator":null
        })
    );
}
