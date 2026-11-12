use shape::naming::name_threads;

fn check(fixture: &str, expected: Option<&str>) {
    let source = format!("function main() {{ {fixture} return $THREAD_1_100; }} main();");
    let source = source.as_str();
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::unambiguous()).parse();
    assert!(
        !parsed.panicked && parsed.errors.is_empty(),
        "{:?}",
        parsed.errors
    );
    let result = name_threads(source, true).expect("analyze fixture");
    let target = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_1_100")
        .expect("target exists");
    if let Some(expected) = expected {
        assert_eq!(target.proposed_name.as_deref(), Some(expected), "{source}");
        assert!(target.applied);
        assert_eq!(
            target.evaluator.as_deref(),
            Some("runtime_owned_lexical_variants")
        );
    } else {
        assert_ne!(
            target.evaluator.as_deref(),
            Some("runtime_owned_lexical_variants"),
            "{source}"
        );
        assert!(
            !result
                .report
                .function_roles
                .iter()
                .any(|r| r.current_name.as_deref() == Some("$THREAD_1_100")
                    && r.evaluators
                        .iter()
                        .any(|e| e == "runtime_owned_lexical_variants")),
            "{source}"
        );
    }
}

#[test]
fn boolean_variants_and_near_misses() {
    for body in [
        "return typeof value === 'boolean';",
        "return 'boolean' === typeof (value);",
    ] {
        check(
            &format!("function $THREAD_1_100(value) {{ {body} }}"),
            Some("isBooleanElement"),
        );
    }
    check(
        "let values; function $THREAD_1_100(index) { return typeof values[index] === 'boolean'; }",
        Some("isBooleanEntryValue"),
    );
    for body in [
        "return typeof other === 'boolean';",
        "return typeof value === 'string';",
        "return typeof value == 'boolean';",
        "value = other; return typeof value === 'boolean';",
        "function donor() { return typeof value === 'boolean'; } return true;",
        "return typeof values[other] === 'boolean';",
        "sideEffect(); return typeof value === 'boolean';",
        "return typeof getValues()[value] === 'boolean';",
        "return typeof missing[value] === 'boolean';",
    ] {
        check(
            &format!("let other, values; function $THREAD_1_100(value) {{ {body} }}"),
            None,
        );
    }
    check(
        "function $THREAD_1_100({value}) { return typeof value === 'boolean'; }",
        None,
    );
    check(
        "async function $THREAD_1_100(value) { return typeof value === 'boolean'; }",
        None,
    );
}

#[test]
fn indexed_encoder_variants_and_near_misses() {
    for (index, collection, context, receiver) in [
        ("index", "values", "state", "codec"),
        ("$CS", "$KEY_8", "$KEY_5", "$KEY_1"),
    ] {
        check(&format!("let {collection}, {context}, {receiver}; function $THREAD_1_100({index}) {{ return {receiver}.encodeValue({collection}[{index}], {context}); }}"), Some("encodeEntryValue"));
    }
    for body in [
        "return codec.encodeValue(values[other], state);",
        "return codec.encodeValue(values[index], index);",
        "return codec.encodeValue(values[index], state, 1);",
        "return codec.encodeOther(values[index], state);",
        "return codec.encodeValue(values[index], codec);",
        "return codec.encodeValue(values[index], values);",
        "return missing.encodeValue(values[index], state);",
        "return codec.encodeValue(missing[index], state);",
        "function donor() { return codec.encodeValue(values[index], state); } return null;",
        "index = other; return codec.encodeValue(values[index], state);",
    ] {
        check(
            &format!("let values, state, codec, other; function $THREAD_1_100(index) {{ {body} }}"),
            None,
        );
    }
}

const EVENT: &str = "let selectors, count, errors; function $THREAD_1_100(event) { let target; try { target = event.target; if (target) { if (target.closest(selectors.join(',')) == null) { ++count; } } target = 7; } catch (e) { errors.error(e); } }";

#[test]
fn selector_alias_variants_and_near_misses() {
    check(EVENT, Some("countEventsOutsideSelector"));
    check(
        &EVENT
            .replace("target = 7", "target = 88404")
            .replace("target", "element")
            .replace("event.element", "event.target"),
        Some("countEventsOutsideSelector"),
    );
    for (from, to) in [
        ("target = event.target", "target = selectors.target"),
        ("if (target)", "if (event)"),
        ("target.closest", "event.closest"),
        ("selectors.join(',')", "selectors.join(';')"),
        ("== null", "!= null"),
        ("++count", "--count"),
        ("errors.error(e)", "errors.error(event)"),
        ("errors.error(e)", "errors.error(e); count++"),
        ("target = 7", "target = selectors"),
        ("if (target)", "target = selectors; if (target)"),
        ("++count;", "function donor() { ++count; }"),
        ("errors.error(e)", "function donor() { errors.error(e); }"),
        (
            "target.closest(selectors.join(','))",
            "target.closest(missing.join(','))",
        ),
        ("++count", "++selectors"),
    ] {
        assert_eq!(EVENT.matches(from).count(), 1);
        check(&EVENT.replace(from, to), None);
    }
}
