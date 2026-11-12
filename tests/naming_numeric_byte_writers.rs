use shape::naming::name_threads;

const EQUALITY: &str = r#"function $THREAD_2_24809(left, right) {
    let copy = right;
    if (left.length === right.length) {
        return left.every((value, index) => copy[index] === value);
    } else { return false; }
}"#;
fn target(source: &str) -> shape::naming::ThreadName {
    let source = format!("function main() {{ {source} return $THREAD_2_24809; }} main();");
    name_threads(&source, true)
        .unwrap()
        .report
        .outcomes
        .into_iter()
        .find(|o| o.binding.name == "$THREAD_2_24809")
        .unwrap()
}
#[test]
fn scoped_equality_and_alpha_variant() {
    for source in [
        EQUALITY.to_string(),
        EQUALITY
            .replace("left", "alpha")
            .replace("right", "beta")
            .replace("copy", "holder")
            .replace("index", "position")
            .replace("value", "element"),
    ] {
        let outcome = target(&source);
        assert_eq!(outcome.proposed_name.as_deref(), Some("arraysEqualByIndex"));
        assert_eq!(
            outcome.evaluator.as_deref(),
            Some("shared_arrays_equal_by_index")
        );
    }
}
#[test]
fn changed_protocols_remain_unresolved() {
    for source in [
        EQUALITY.replace("copy[index] === value", "copy[index] == value"),
        EQUALITY.replace("copy[index]", "copy[value]"),
        EQUALITY.replace("let copy = right", "let copy = left"),
        EQUALITY.replace("return false", "return true"),
        EQUALITY.replace("return left.every", "sideEffect(); return left.every"),
        EQUALITY.replace("(value, index) =>", "async (value, index) =>"),
        EQUALITY.replace("let copy = right;", "let copy = right; copy = left;"),
        EQUALITY.replace("right.length", "foreign.length"),
        EQUALITY.replace("copy[index] === value", "(() => copy[index] === value)()"),
        EQUALITY.replace("left.every", "right.every"),
        EQUALITY.replace("left.every", "left?.every"),
        EQUALITY.replace("let copy = right;", "let copy = right; right = foreign;"),
    ] {
        let outcome = target(&source);
        assert!(
            !outcome.applied,
            "wrongly applied {} to {source}",
            outcome.proposed_name.unwrap_or_default()
        );
    }
}

const WHOLE_WRITER_SCOPE: &str = include_str!("fixtures/naming/numeric_byte_writers/numeric-writers-whole-scope.js");
const WHOLE_WRITER_SCOPE_ALTERNATE: &str = include_str!("fixtures/naming/numeric_byte_writers/numeric-writers-whole-scope-alternate.js");
#[test]
fn original_whole_scopes_and_alpha_variants_numeric_protocols() {
    for seed in [WHOLE_WRITER_SCOPE, WHOLE_WRITER_SCOPE_ALTERNATE] {
        for source in [
            seed.to_string(),
            seed.replace("$CC", "$alphaInput")
                .replace("$CD", "$alphaOutput")
                .replace("$AX", "$alphaValue")
                .replace("$AY", "$alphaDestination"),
        ] {
            let report = name_threads(&source, true).unwrap().report;
            for (evaluator, count) in [
                ("shared_tagged_unsigned_integer_writer", 1),
                ("shared_typed_array_float_writer", 2),
                ("shared_extension_registry_factory", 2),
                ("shared_arrays_equal_wrapper", 2),
            ] {
                let outcomes: Vec<_> = report
                    .outcomes
                    .iter()
                    .filter(|o| o.evaluator.as_deref() == Some(evaluator) && o.applied)
                    .collect();
                assert_eq!(
                    outcomes.len(),
                    count,
                    "missing {evaluator}: {:?}",
                    report
                        .outcomes
                        .iter()
                        .filter(|o| !o.applied && o.binding.name.contains("5759"))
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}
fn refused(source: &str, name: &str) {
    let report = name_threads(source, true).unwrap().report;
    let target = report
        .outcomes
        .iter()
        .find(|o| o.binding.name == name)
        .unwrap();
    assert!(
        !target.applied && target.proposed_name.is_none() && target.evaluator.is_none(),
        "actual target {:?} must remain unresolved",
        target
    );
}
#[test]
fn numeric_actual_target_protocol_and_capture_negatives() {
    // Edits are made in the actual original whole scope, never fabricated
    // initialized carriers; the unresolved target outcome must still exist.
    for source in [
        WHOLE_WRITER_SCOPE.replacen("if ($CC <= 63)", "if ($CC <= 64)", 1),
        WHOLE_WRITER_SCOPE.replacen(
            "$KEY_200.UINT14_BASE | $CC >> 8",
            "$KEY_200.UINT14_BASE | $CC >> 7",
            1,
        ),
        WHOLE_WRITER_SCOPE.replacen("$CD.push($KEY_200.UINT32)", "$CD.push($KEY_200.UINT24)", 1),
        WHOLE_WRITER_SCOPE.replacen("$KEY_200.UINT16, $CC >> 8", "foreign.UINT16, $CC >> 8", 1),
        WHOLE_WRITER_SCOPE.replacen("UINT14_BASE: 64", "UINT14_BASE: 65", 1),
        WHOLE_WRITER_SCOPE.replacen(
            "let $KEY_261 = $THREAD_7_10239;",
            "let $KEY_261 = $THREAD_7_10239; $KEY_261(70, []);",
            1,
        ),
        WHOLE_WRITER_SCOPE.replacen(
            "let $KEY_261 = $THREAD_7_10239;",
            "let $KEY_261 = $THREAD_7_10239; invokeNow(() => $KEY_261(70, []));",
            1,
        ),
        WHOLE_WRITER_SCOPE.replacen(
            "let $KEY_261 = $THREAD_7_10239;",
            "let $KEY_261 = $THREAD_7_10239; defer(() => { $KEY_200 = {}; });",
            1,
        ),
    ] {
        refused(&source, "$THREAD_7_10239");
    }
    for source in [
        WHOLE_WRITER_SCOPE.replacen("if ($KEY_0[0] !== $CC)", "if ($KEY_0[0] === $CC)", 1),
        WHOLE_WRITER_SCOPE.replacen(
            "new Uint8Array($KEY_0.buffer)",
            "new Uint8Array($CF.buffer)",
            1,
        ),
        WHOLE_WRITER_SCOPE.replacen("$CE = $KEY_200.DOUBLE64", "$CE = foreign.DOUBLE64", 1),
        WHOLE_WRITER_SCOPE.replacen(
            "let $CG = new Uint8Array($KEY_0.buffer);",
            "sideEffect(); let $CG = new Uint8Array($KEY_0.buffer);",
            1,
        ),
        WHOLE_WRITER_SCOPE.replacen("$CD.push($CG[$CH - 1]);", "$CD.push($CG[$CH]);", 1),
        WHOLE_WRITER_SCOPE.replacen("FLOAT32: 236", "FLOAT32: 238", 1),
    ] {
        refused(&source, "$THREAD_7_5759");
    }
}

#[test]
fn numeric_initialization_epochs_are_executed_stores() {
    let start = WHOLE_WRITER_SCOPE.find("$KEY_200 = {").unwrap();
    let end = start + WHOLE_WRITER_SCOPE[start..].find("};").unwrap() + 2;
    let store = &WHOLE_WRITER_SCOPE[start..end];
    for replacement in [
        format!("if (condition) {{ {store} }}"),
        format!("defer(() => {{ {store} }});"),
        format!("{store} $KEY_200.FLOAT32 = 238;"),
        format!("{store} expose($KEY_200);"),
        format!("{store} ({{x: $KEY_200.FLOAT32}} = foreign);"),
    ] {
        let source = WHOLE_WRITER_SCOPE.replacen(store, &replacement, 1);
        refused(&source, "$THREAD_7_5759");
        refused(&source, "$THREAD_7_10239");
    }
    // The typed declaration is in its own nested block. A call injected at the
    // outer table store does not resolve to that binding. Move the first store
    // after a real call in the target block and prove the identity independently.
    let without_store = WHOLE_WRITER_SCOPE.replacen(store, ";", 1);
    let source = without_store.replacen(
        "$KEY_213 = $THREAD_7_5759;",
        &format!("$THREAD_7_5759(1, []); $KEY_213 = $THREAD_7_5759; {store}"),
        1,
    );
    actual_call_identity(&source, "$THREAD_7_5759");
    refused(&source, "$THREAD_7_5759");
}

fn actual_call_identity(source: &str, name: &str) {
    use oxc_ast::{ast::Expression, AstKind};
    let allocator = oxc_allocator::Allocator::default();
    let parsed = shape::analysis::parse(&allocator, source);
    assert!(parsed.errors.is_empty(), "actual negative must parse");
    let (scoping, nodes) = shape::analysis::analyze(&parsed.program);
    let target = nodes
        .iter()
        .find_map(|n| match n.kind() {
            AstKind::Function(f) if f.id.as_ref().is_some_and(|id| id.name == name) => {
                f.id.as_ref().and_then(|id| id.symbol_id.get())
            }
            _ => None,
        })
        .unwrap();
    let calls: Vec<_> = nodes
        .iter()
        .filter_map(|n| match n.kind() {
            AstKind::CallExpression(c) => match &c.callee {
                Expression::Identifier(id) if id.name == name => Some(
                    id.reference_id
                        .get()
                        .and_then(|r| scoping.get_reference(r).symbol_id()),
                ),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(
        calls,
        vec![Some(target)],
        "negative must invoke actual target binding, not an unresolved global or sibling"
    );
}

fn mutate_after(source: &str, anchor: &str, from: &str, to: &str) -> String {
    let start = source.find(anchor).unwrap();
    let offset = start + source[start..].find(from).unwrap();
    let mut result = source.to_string();
    result.replace_range(offset..offset + from.len(), to);
    result
}
#[test]
fn registry_factory_actual_protocol_and_epoch_negatives() {
    let anchor = "function $THREAD_7_11417()";
    for source in [
        mutate_after(WHOLE_WRITER_SCOPE, anchor, "key: \"extend\"", "key: \"remove\""),
        mutate_after(WHOLE_WRITER_SCOPE, anchor, "Object.create(null)", "Object.create({})"),
        mutate_after(
            WHOLE_WRITER_SCOPE,
            anchor,
            "this.extensions[$CD]",
            "this.extensions[$CE]",
        ),
        mutate_after(
            WHOLE_WRITER_SCOPE,
            anchor,
            "$KEY_398($KEY_0, [$CC]);",
            "$KEY_398($KEY_0, []);",
        ),
        mutate_after(
            WHOLE_WRITER_SCOPE,
            anchor,
            "$CC.value = $THREAD_7_60850;",
            "$CC.value = foreign;",
        ),
        WHOLE_WRITER_SCOPE.replacen("$KEY_398 = $THREAD_4_16589();", "$KEY_398 = foreign();", 1),
        WHOLE_WRITER_SCOPE.replacen(
            "$KEY_398 = $THREAD_4_16589();",
            "$KEY_398 = $THREAD_4_16589(); defer(() => { $KEY_398 = foreign(); });",
            1,
        ),
    ] {
        refused(&source, "$THREAD_7_11417");
    }
    let store = "$KEY_398 = $THREAD_4_16589();";
    let source = WHOLE_WRITER_SCOPE.replacen(store, ";", 1).replacen(
        "let $BN = $THREAD_7_11417();",
        &format!("let $BN = $THREAD_7_11417(); {store}"),
        1,
    );
    actual_call_identity(&source, "$THREAD_7_11417");
    refused(&source, "$THREAD_7_11417");
}
/// The equality-wrapper role is refused. A structural captured-callee forwarder can still
/// name the body: forwarding the sole formal and one captured value does not establish
/// array comparison or invocation timing.
fn equality_role_refused(source: &str, name: &str) {
    let report = name_threads(source, true).unwrap().report;
    let target = report
        .outcomes
        .iter()
        .find(|o| o.binding.name == name)
        .unwrap();
    let generic_forwarder = target.applied
        && target.evaluator.as_deref() == Some("runtime_captured_callee_wrappers")
        && target.proposed_name.as_deref() == Some("forwardFormalAndCapturedToCapturedCallable");
    assert!(
        generic_forwarder
            || (!target.applied && target.proposed_name.is_none() && target.evaluator.is_none()),
        "actual target {:?} must not keep the equality-wrapper role",
        target
    );
}

#[test]
fn equality_wrappers_prove_callee_capture_and_own_body() {
    for source in [
        WHOLE_WRITER_SCOPE.replacen(
            "return $KEY_131($CP, $KEY_1);",
            "return foreign($CP, $KEY_1);",
            1,
        ),
        WHOLE_WRITER_SCOPE.replacen(
            "return $KEY_131($CP, $KEY_1);",
            "return $KEY_131($KEY_1, $CP);",
            1,
        ),
        WHOLE_WRITER_SCOPE.replacen(
            "return $KEY_131($CP, $KEY_1);",
            "effect(); return $KEY_131($CP, $KEY_1);",
            1,
        ),
        WHOLE_WRITER_SCOPE.replacen(
            "return $KEY_131($CP, $KEY_1);",
            "return $KEY_131($CP, foreign);",
            1,
        ),
        WHOLE_WRITER_SCOPE.replacen(
            "return $KEY_131($CP, $KEY_1);",
            "$KEY_1 = foreign; return $KEY_131($CP, $KEY_1);",
            1,
        ),
    ] {
        refused(&source, "$THREAD_3_44967");
    }
    // The wrapper body is unchanged; only the captured callee stops being the
    // array comparison.
    equality_role_refused(
        &WHOLE_WRITER_SCOPE.replacen("$KEY_0[$CF] === $CE", "$KEY_0[$CF] == $CE", 1),
        "$THREAD_3_44967",
    );
}

#[test]
fn equality_wrapper_actual_preinit_call_is_refused() {
    let wrapper = WHOLE_WRITER_SCOPE.find("function $THREAD_3_44967(").unwrap();
    let alias = WHOLE_WRITER_SCOPE[..wrapper].rfind("let $KEY_1 = $CL;").unwrap();
    let mut source = WHOLE_WRITER_SCOPE.to_string();
    source.insert_str(alias, "$THREAD_3_44967([]); ");
    use oxc_ast::{ast::Expression, AstKind};
    let allocator = oxc_allocator::Allocator::default();
    let parsed = shape::analysis::parse(&allocator, &source);
    assert!(parsed.errors.is_empty());
    let (scoping, nodes) = shape::analysis::analyze(&parsed.program);
    let target = nodes
        .iter()
        .find_map(|n| match n.kind() {
            AstKind::Function(f)
                if f.id.as_ref().is_some_and(|id| id.name == "$THREAD_3_44967") =>
            {
                f.id.as_ref().and_then(|id| id.symbol_id.get())
            }
            _ => None,
        })
        .unwrap();
    let actual = nodes
        .iter()
        .find_map(|n| match n.kind() {
            AstKind::CallExpression(c) if c.span.start == alias as u32 => match &c.callee {
                Expression::Identifier(id) => Some(
                    id.reference_id
                        .get()
                        .and_then(|r| scoping.get_reference(r).symbol_id()),
                ),
                _ => None,
            },
            _ => None,
        })
        .unwrap();
    assert_eq!(
        actual,
        Some(target),
        "actual wrapper call must resolve to actual target declaration"
    );
    equality_role_refused(&source, "$THREAD_3_44967");
}
