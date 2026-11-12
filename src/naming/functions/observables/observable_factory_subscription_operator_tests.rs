use super::*;
const SEPARATE_CLEANUP: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/naming/subscription_operators/subscription-parent-alternate.js"
));
const CAPTURED_CLEANUP: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/naming/subscription_operators/subscription-parent-baseline.js"
));

// Parse the entire original enclosing program, retaining every captured writer
// and invocation. Select the own class factory, never a nested/sibling donor.
fn proof(source: &str) -> (Span, bool) {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = crate::analysis::parse(&allocator, source);
    assert!(parsed.errors.is_empty());
    let built = oxc_semantic::SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    assert!(built.errors.is_empty());
    let model = Model::new(
        source,
        &parsed.program,
        built.semantic.scoping(),
        built.semantic.nodes(),
        ProgramPropertiesLimits::default(),
    )
    .unwrap();
    let site = model.all_sites().iter().copied().find(|s| {
        let AstKind::Function(f) = model.nodes.kind(s.node_id) else { return false; };
        f.body.as_ref().is_some_and(|b| matches!(b.statements.first(), Some(Statement::ClassDeclaration(c)) if c.body.body.iter().any(|e| matches!(e,ClassElement::MethodDefinition(m) if m.r#static && property_key_name(&m.key).as_deref()==Some("of")))))
    }).expect("actual target class factory");
    (site.span, proven_native_class(&model, site).is_some())
}
fn proven(source: &str) -> bool {
    proof(source).1
}
fn changed(source: &str, from: &str, to: &str) -> String {
    if source.matches(from).count() == 1 {
        return source.replacen(from, to, 1);
    }
    let (span, _) = proof(source);
    let own = &source[span.start as usize..span.end as usize];
    assert_eq!(
        own.matches(from).count(),
        1,
        "unique OWN target mutation: {from}"
    );
    let mut result = source.to_string();
    result.replace_range(
        span.start as usize..span.end as usize,
        &own.replacen(from, to, 1),
    );
    result
}
#[test]
fn whole_factory_protocol_and_alpha_variants() {
    for source in [
        SEPARATE_CLEANUP.to_string(),
        CAPTURED_CLEANUP.to_string(),
        SEPARATE_CLEANUP
            .replace("$BQ", "promiseGuardValue")
            .replace("$KEY_4", "classAliasCapture"),
        CAPTURED_CLEANUP
            .replace("$AU", "sentinelSecondDeclared")
            .replace("$AV", "sentinelFirstStored"),
        changed(CAPTURED_CLEANUP, "let $AU, $AV;", "let $AV, $AU;"),
    ] {
        assert!(proven(&source));
    }
}
#[test]
fn factory_guard_and_sentinel_protocol_negatives() {
    for (source, from, to) in [
        (SEPARATE_CLEANUP, "$BQ = false;", "$BQ = true;"),
        (
            SEPARATE_CLEANUP,
            "$BQ = $BO instanceof Promise;",
            "$BQ = $BP instanceof Promise;",
        ),
        (SEPARATE_CLEANUP, "if ($BQ) {", "if (!$BQ) {"),
        (CAPTURED_CLEANUP, "$AV = 9658;", "$AV = void 0;"),
        (CAPTURED_CLEANUP, "$AU = 2;", "$AV = 2;"),
        (CAPTURED_CLEANUP, "$AU = 2;", "$AU = 2; extraEffect();"),
        (CAPTURED_CLEANUP, "if ($AV === void 0)", "if ($AU === void 0)"),
        (
            CAPTURED_CLEANUP,
            "Cannot read properties of undefined (reading '0')",
            "Cannot read properties of undefined (reading '1')",
        ),
        (
            CAPTURED_CLEANUP,
            "$PARAMB.call($PARAMA, $PARAMC[$PARAMD]);",
            "$PARAMB.call($PARAMC, $PARAMA[$PARAMD]);",
        ),
        (CAPTURED_CLEANUP, "i = Number(i) + 1, true", "i = Number(i) + 2, true"),
        (
            CAPTURED_CLEANUP,
            "})(j, (j.next), $KEY_4, i)",
            "})(j, (j.error), $KEY_4, i)",
        ),
        (
            CAPTURED_CLEANUP,
            "$KEY_2.error(e);\n\t\t\t\t\t\t\t\t\tif ($AV",
            "$KEY_2.error($AU);\n\t\t\t\t\t\t\t\t\tif ($AV",
        ),
    ] {
        assert!(
            !proven(&changed(source, from, to)),
            "accepted altered own protocol {from} => {to}"
        );
    }
}
#[test]
fn factory_captured_origins_and_reentrant_exposures() {
    for replacement in [
        "let $KEY_401; if (flag) { $KEY_401 = $THREAD_7_29383; }",
        "let $KEY_401 = $THREAD_7_29383; $KEY_401 = other;",
        "$THREAD_9_123048(); let $KEY_401 = $THREAD_7_29383;",
        "register($THREAD_9_123048); let $KEY_401 = $THREAD_7_29383;",
    ] {
        assert!(
            !proven(&changed(
                SEPARATE_CLEANUP,
                "let $KEY_401 = $THREAD_7_29383;",
                replacement
            )),
            "accepted unavailable capture {replacement}"
        );
    }
    let preinit = changed(
        SEPARATE_CLEANUP,
        "let $KEY_401 = $THREAD_7_29383;",
        "let $KEY_401 = laterAlias;",
    );
    let preinit = changed(
        &preinit,
        "$KEY_315 = $THREAD_9_123048();",
        "let laterAlias = $THREAD_7_29383; $KEY_315 = $THREAD_9_123048();",
    );
    assert!(!proven(&preinit));
}
