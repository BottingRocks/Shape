use shape::naming::name_threads;

const ROLE: &str = "createGlobalCollectionApi";
const DIRECT_RETURN: &str =
    include_str!("fixtures/program/collection-carrier-layout/encoded-capture-reuse.js");
const ASSIGNED_RETURN: &str =
    include_str!("fixtures/program/collection-carrier-layout/immutable-alias-carrier.js");
const HOLDER_RETURN: &str =
    include_str!("fixtures/program/collection-carrier-layout/immutable-alias-carrier-alternate.js");
const ALIASED_RETURN: &str =
    include_str!("fixtures/program/collection-carrier-layout/grouped-alias-carrier.js");

fn source(body: &str) -> String {
    let captures = (0..500)
        .map(|i| format!("$KEY_{i}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("(function(){{let {captures}; {body} }})();")
}

fn named(body: &str, target: &str) -> bool {
    name_threads(&source(body), true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|o| o.binding.name == target && o.applied && o.proposed_name.as_deref() == Some(ROLE))
}

fn changed(body: &str, from: &str, to: &str) -> String {
    assert!(body.contains(from), "missing mutation anchor {from}");
    body.replacen(from, to, 1)
}

#[test]
fn carrier_layout_selected_positive_variants() {
    for (body, target) in [
        (ASSIGNED_RETURN, "$THREAD_0_25672"),
        (HOLDER_RETURN, "$THREAD_8_71882"),
        (ALIASED_RETURN, "$THREAD_1_44111"),
    ] {
        assert!(named(body, target), "{target}");
    }
}

#[test]
fn carrier_layout_unproven_child_still_refuses() {
    // Its capture reuses an encoded local; this package does not widen that proof.
    assert!(!named(DIRECT_RETURN, "$THREAD_0_44765"));
}

#[test]
fn carrier_layout_immutable_alias_permutation_and_grouping() {
    let body = changed(ASSIGNED_RETURN,
        "let $KEY_0 = $AM;\n\t\t\tlet $KEY_1;\n\t\t\tlet $KEY_2 = $AN;\n\t\t\tlet $KEY_3 = $AO;\n\t\t\tlet $KEY_4, $KEY_6;",
        "let $KEY_4, $KEY_3 = $AO, $KEY_6, $KEY_2 = $AN, $KEY_1, $KEY_0 = $AM;");
    assert!(named(&body, "$THREAD_0_25672"));
}

#[test]
fn carrier_layout_alias_and_state_near_misses() {
    for (from, to) in [
        ("let $KEY_0 = $AM;", "let $KEY_0 = $AN;"),
        ("let $KEY_0 = $AM;", "let $KEY_0 = sideEffect($AM);"),
        ("let $KEY_1;", "let $KEY_1 = void 0;"),
        ("let $KEY_1;", "let $KEY_1; let extra;"),
        ("$KEY_1 = $AL.global;", "$KEY_1 = $AM.global;"),
        ("$KEY_1 = $AL.global;", "$KEY_1 = $AL.global; $KEY_0 = $AN;"),
        (
            "$KEY_6 = $KEY_1.ns_collect;",
            "$KEY_6 = $KEY_499.ns_collect;",
        ),
    ] {
        assert!(
            !named(&changed(ASSIGNED_RETURN, from, to), "$THREAD_0_25672"),
            "{from} -> {to}"
        );
    }
}

#[test]
fn carrier_layout_lifecycle_binding_near_misses() {
    for (from, to) in [
        (
            "$AP.teardown = $THREAD_3_9833;",
            "$AP.teardown = $THREAD_7_143361;",
        ),
        ("init: $THREAD_7_143361", "init: $THREAD_3_9833"),
        ("$KEY_1.ns_collect = $KEY_6;", "$KEY_1.ns_collect = $KEY_4;"),
        ("$KEY_1[$KEY_341] = $KEY_4;", "$KEY_499[$KEY_341] = $KEY_4;"),
        (
            "function $THREAD_3_9833()",
            "async function $THREAD_3_9833()",
        ),
    ] {
        assert!(
            !named(&changed(ASSIGNED_RETURN, from, to), "$THREAD_0_25672"),
            "{from} -> {to}"
        );
    }
}

#[test]
fn carrier_layout_direct_return_near_misses() {
    for (from, to) in [
        ("teardown: $THREAD_1_42949", "teardown: $THREAD_3_476"),
        (
            "teardown: $THREAD_1_42949",
            "teardown: $THREAD_1_42949, extra: true",
        ),
        (
            "init: $THREAD_3_476",
            "init: $THREAD_3_476, init: $THREAD_3_476",
        ),
    ] {
        assert!(!named(&changed(HOLDER_RETURN, from, to), "$THREAD_8_71882"));
    }
}

#[test]
fn carrier_layout_nested_donor_cannot_supply_installed_collector() {
    let body = changed(
        ASSIGNED_RETURN,
        "$KEY_1.ns_collect = $THREAD_6_76040;",
        "function donor() { $KEY_1.ns_collect = $THREAD_6_76040; }",
    );
    assert!(!named(&body, "$THREAD_0_25672"));
}

#[test]
fn carrier_layout_valid_sibling_cannot_supply_lifecycle_proof() {
    let invalid = changed(ASSIGNED_RETURN, "init: $THREAD_7_143361", "init: $THREAD_3_9833");
    let invalid = changed(
        &invalid,
        "function $THREAD_0_25672(",
        "function $THREAD_9_99999(",
    );
    let pair = format!("{ASSIGNED_RETURN}\n{invalid}");
    assert!(named(&pair, "$THREAD_0_25672"));
    assert!(!named(&pair, "$THREAD_9_99999"));
}
