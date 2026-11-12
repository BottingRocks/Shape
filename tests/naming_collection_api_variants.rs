use shape::naming::name_threads;

const MAPPER: &str =
    include_str!("fixtures/program/collection-mapper-variants/immutable-alias-carrier-mapper.js");
const ROLE: &str = "buildBeaconDescriptorPayloads";

fn source(body: &str) -> String {
    // Captured opaque protocol inputs; no fixture is executed.
    let captures = (0..500)
        .map(|i| format!("$KEY_{i}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("(function(){{let {captures}; {body} }})();")
}

fn named(body: &str, target: &str, role: &str) -> bool {
    name_threads(&source(body), true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|o| o.binding.name == target && o.applied && o.proposed_name.as_deref() == Some(role))
}

#[test]
fn mapper_placeholder_positive_variants() {
    for (body, target) in [
        (MAPPER, "$THREAD_7_97278"),
        (
            include_str!(
                "fixtures/program/collection-mapper-variants/immutable-alias-carrier-alternate-mapper.js"
            ),
            "$THREAD_4_49099",
        ),
        (
            include_str!(
                "fixtures/program/collection-mapper-variants/grouped-alias-carrier-mapper.js"
            ),
            "$THREAD_9_79372",
        ),
    ] {
        assert!(named(body, target, ROLE), "{target}");
    }
}

#[test]
fn mapper_placeholder_near_misses() {
    for replacement in [
        "payload: void sideEffect()",
        "payload: 1",
        "payload: undefined",
        "payload: void 0, payload: void 0",
    ] {
        let body = MAPPER.replace("payload: void 0", replacement);
        assert_ne!(body, MAPPER);
        assert!(!named(&body, "$THREAD_7_97278", ROLE), "{replacement}");
    }
    let body = MAPPER.replace("$AT.payload = $KEY_57($AU);", "");
    assert_ne!(body, MAPPER);
    assert!(!named(&body, "$THREAD_7_97278", ROLE));
}

#[test]
fn mapper_nested_donor_cannot_supply_payload_write() {
    let body = MAPPER.replace(
        "$AT.payload = $KEY_57($AU);",
        "function donor() { $AT.payload = $KEY_57($AU); }",
    );
    assert_ne!(body, MAPPER);
    assert!(!named(&body, "$THREAD_7_97278", ROLE));
}

const COLLECTOR: &str =
    include_str!("fixtures/program/collection-mapper-variants/immutable-alias-carrier-collector.js");
const HEADER_ROLE: &str = "collectEncodedHeaders";

#[test]
fn header_void_key_positive_variants() {
    for (body, target) in [
        (COLLECTOR, "$THREAD_6_76040"),
        (
            include_str!(
                "fixtures/program/collection-mapper-variants/grouped-alias-carrier-collector.js"
            ),
            "$THREAD_1_8190",
        ),
    ] {
        assert!(named(body, target, HEADER_ROLE), "{target}");
    }
    assert!(named(
        &COLLECTOR.replace("let $AT = void 0;", "let $AT;"),
        "$THREAD_6_76040",
        HEADER_ROLE
    ));
}

#[test]
fn header_void_key_near_misses() {
    for replacement in ["void sideEffect()", "null", "undefined", "void $AQ"] {
        let body = COLLECTOR.replace("let $AT = void 0;", &format!("let $AT = {replacement};"));
        assert_ne!(body, COLLECTOR);
        assert!(
            !named(&body, "$THREAD_6_76040", HEADER_ROLE),
            "{replacement}"
        );
    }
    // The copy must still read the key assigned by this loop.
    let body = COLLECTOR.replace("$AT = j;", "$AT = $KEY_499;");
    assert_ne!(body, COLLECTOR);
    assert!(!named(&body, "$THREAD_6_76040", HEADER_ROLE));
}

#[test]
fn header_nested_donor_cannot_supply_key_assignment() {
    let body = COLLECTOR.replace("$AT = j;", "function donor() { $AT = j; }");
    assert_ne!(body, COLLECTOR);
    assert!(!named(&body, "$THREAD_6_76040", HEADER_ROLE));
}
