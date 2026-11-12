use shape::naming::name_threads;

const COLLECTOR: &str = include_str!("fixtures/program/collection-staged-guard/collector.js");
const FACTORY: &str = include_str!("fixtures/program/collection-staged-guard/factory.js");
const HEADER: &str = "collectEncodedHeaders";

fn source(body: &str) -> String {
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

fn changed(body: &str, from: &str, to: &str) -> String {
    assert!(body.contains(from), "missing mutation anchor {from}");
    body.replacen(from, to, 1)
}

#[test]
fn staged_header_positive_variants() {
    assert!(named(COLLECTOR, "$THREAD_5_92214", HEADER));
    // Scoped identity must survive arbitrary local and formal spellings.
    let renamed = COLLECTOR
        .replace("$AW", "acceptedRequest")
        .replace("$AV", "request");
    assert!(named(&renamed, "$THREAD_5_92214", HEADER));
}

#[test]
fn staged_header_lifecycle_positive() {
    for (target, role) in [
        ("$THREAD_5_92214", HEADER),
        ("$THREAD_7_140685", "initialiseGlobalCollectionApi"),
        ("$THREAD_5_140787", "createGlobalCollectionApi"),
    ] {
        assert!(named(FACTORY, target, role), "{target}: {role}");
    }
}

#[test]
fn staged_header_guard_near_misses() {
    for (from, to) in [
        ("$AV == null", "$AV != null"),
        ("$AV == null", "$AV === null"),
        ("$AW = true;", "$AW = false;"),
        ("$AV.method != null", "$AV.url != null"),
        ("$AV.url != null", "$AV.url !== null"),
        ("$AV.url != null &&", "$AV.url != null ||"),
        ("$KEY_322($AV)", "$KEY_322($KEY_499)"),
        ("$KEY_322($AV)", "$KEY_322($AV, sideEffect())"),
        ("$KEY_322($AV)", "$KEY_322?.($AV)"),
        ("if ($AW) {", "if (!$AW) {"),
        ("let $AW;", "let $AW = true;"),
        ("let $AW;", "let $AW; let extra;"),
        ("let $AW;", "let $AW; $AV = null;"),
        ("$AW = $KEY_322($AV);", "$AW = $KEY_322($AV); $AW = true;"),
        ("return null;", "return {};"),
    ] {
        assert!(
            !named(&changed(COLLECTOR, from, to), "$THREAD_5_92214", HEADER),
            "{from} -> {to}"
        );
    }
}

#[test]
fn staged_header_rejects_nested_donor_and_shadow() {
    for replacement in [
        "function donor() { $AW = $KEY_322($AV); }",
        "let $AW; $AW = $KEY_322($AV);",
        "let $AV = other; $AW = $KEY_322($AV);",
    ] {
        assert!(!named(
            &changed(COLLECTOR, "$AW = $KEY_322($AV);", replacement),
            "$THREAD_5_92214",
            HEADER
        ));
    }
    let mutable = format!("{COLLECTOR}\n$KEY_322 = otherPolicy;");
    assert!(!named(&mutable, "$THREAD_5_92214", HEADER));
}

#[test]
fn staged_header_rejects_wrong_policy_branch() {
    let body = changed(COLLECTOR, "if ($AW) {", "if (true) {");
    assert!(!named(&body, "$THREAD_5_92214", HEADER));
    let body = changed(
        COLLECTOR,
        "$AW = $KEY_322($AV);",
        "$AW = $KEY_322($AV); } else { $AW = true;",
    );
    assert!(!named(&body, "$THREAD_5_92214", HEADER));
}

#[test]
fn staged_header_invalid_child_blocks_lifecycle() {
    let body = changed(FACTORY, "$AW = true;", "$AW = false;");
    for (target, role) in [
        ("$THREAD_5_92214", HEADER),
        ("$THREAD_7_140685", "initialiseGlobalCollectionApi"),
        ("$THREAD_5_140787", "createGlobalCollectionApi"),
    ] {
        assert!(!named(&body, target, role), "{target}: {role}");
    }
}
