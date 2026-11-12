use shape::naming::name_threads;
const PROXY: &str = include_str!("fixtures/program/reflection-protocols/proxy.js");
fn named(s: &str, role: &str) -> usize {
    name_threads(s, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .filter(|o| o.applied && o.role_name.as_deref() == Some(role))
        .count()
}
#[test]
fn proxy_variants() {
    assert_eq!(named(PROXY, "supportsProxyGetPrototypeOf"), 1);
    assert_eq!(named(PROXY, "markProxyPrototypeTrap"), 1);
    for s in [
        PROXY.replace("$KEY_2 = true", "$KEY_2 = false"),
        PROXY.replace("$AF === $AG", "$AF !== $AG"),
        PROXY.replace(
            "if ($KEY_2 !== null) {}",
            "if ($KEY_2 !== null) { sideEffect(); }",
        ),
        PROXY.replace("getPrototypeOf: $THREAD_8_21212", "getPrototypeOf: foreign"),
        PROXY.replace("let $AG = $KEY_1", "let $AG = foreign"),
        format!("function outer(Proxy){{{PROXY}}}"),
    ] {
        assert_eq!(named(&s, "supportsProxyGetPrototypeOf"), 0);
        assert_eq!(named(&s, "markProxyPrototypeTrap"), 0);
    }
    assert_eq!(
        named(
            &PROXY
                .replace("$KEY_1", "prototypeObject")
                .replace("$KEY_2", "seen"),
            "supportsProxyGetPrototypeOf"
        ),
        1
    );
}
const SHAPE: &str = include_str!("fixtures/program/reflection-protocols/function-shape.js");
#[test]
fn function_shape_whole_body() {
    let role = "collectCallabilitySourceAndPrototypeDeletionTraits";
    assert_eq!(named(SHAPE, role), 1);
    for s in [
        SHAPE.replace("delete $AM.prototype", "delete foreign.prototype"),
        SHAPE.replace("$KEY_246.test", "$KEY_246.exec"),
        SHAPE.replace("$KEY_4.c = !$KEY_1", "$KEY_4.c = $KEY_1"),
        SHAPE.replace("$KEY_24.call($KEY_1)", "$KEY_24.call(foreign)"),
        SHAPE.replace("$AO = true", "$AO = false"),
        SHAPE.replace("catch (e) {}", "catch (e) { extraEffect(); }"),
        SHAPE.replace("$KEY_198($AM, \"prototype\")", "$KEY_198($AM, \"other\")"),
        SHAPE.replace("return $KEY_4;", "extraEffect(); return $KEY_4;"),
    ] {
        assert_eq!(named(&s, role), 0);
    }
    assert_eq!(
        named(
            &SHAPE
                .replace("$KEY_1", "valueAlias")
                .replace("$KEY_24", "sourceReader")
                .replace("$AM", "candidate"),
            role
        ),
        1
    );
}
#[test]
fn url_record_ordered_split_fields() {
    let original = include_str!("fixtures/url_query_rebuild/primary.js");
    let removed = ",\n\t\t\t\t\t\tsearch: $AT,\n\t\t\t\t\t\thash: $AU";
    assert!(original.contains(removed));
    let source = original
        .replace(removed, "")
        .replace("$AV.href =", "$AV.search = $AT; $AV.hash = $AU; $AV.href =");
    let role = "rebuildUrlWithEncodedQuery";
    assert_eq!(named(&source, role), 1);
    for s in [
        source.replace("$AV.search = $AT", "$AV.search = $AU"),
        source.replace("$AV.hash = $AU", "$AV.hash = $AT"),
        source.replace("$AV.hash = $AU;", "$AV.hash = $AU; extraEffect();"),
        source.replace("$AV.search = $AT;", "$AV[foreign] = $AT;"),
    ] {
        assert_eq!(named(&s, role), 0);
    }
}
