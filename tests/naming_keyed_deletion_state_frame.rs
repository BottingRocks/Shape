//! Full original scopes retain real capture, factory, and registration origins.
use shape::naming::name_threads;
const SOURCE: &str = include_str!("fixtures/program/keyed-deletion/keyed-deletion-alternate.js");
const NAME: &str = "emitKeyedDeletionStateFrame";
const TARGET: &str = "$THREAD_7_92674";
fn check(source: &str, target: &str, accepted: bool) {
    let result = name_threads(source, true).expect("full source must analyze");
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == target)
        .expect("actual binding");
    assert_eq!(
        outcome.applied && outcome.proposed_name.as_deref() == Some(NAME),
        accepted,
        "{:?}",
        outcome
    );
    if accepted {
        assert_eq!(
            outcome.evaluator.as_deref(),
            Some("thread_keyed_deletion_state_frame")
        );
        let text = outcome.explanation.join(" ");
        for evidence in [
            "explicitly unresolved",
            "before value serialization",
            "earlier previous state",
            "auxiliary at bit 2",
            "original coercions",
        ] {
            assert!(text.contains(evidence), "missing {evidence}");
        }
    }
}
fn mutate(from: &str, to: &str) -> String {
    let body = &SOURCE[381912..383062];
    assert!(body.contains(from), "missing {from}");
    format!(
        "{}{}{}",
        &SOURCE[..381912],
        body.replacen(from, to, 1),
        &SOURCE[383062..]
    )
}
#[test]
fn original_closed_schema() {
    check(SOURCE, TARGET, true);
}
#[test]
fn alpha_full_scope_and_distinct_salt() {
    // Bijective renaming across the entire input preserves same-text shadows
    // and every actual origin; no invented factory replaces the original.
    let mut source = SOURCE.to_owned();
    for (from, to) in [
        ("$KEY_24", "$renamedRemoval"),
        ("$KEY_20", "$renamedError"),
        ("$KEY_22", "$renamedAux"),
        ("$KEY_28", "$renamedKey"),
        ("$KEY_320", "$renamedSeedFactory"),
        ("$KEY_357", "$renamedWriterFactory"),
        ("$CR", "$eventAlpha"),
        ("$CS", "$seedAlpha"),
        (TARGET, "$THREAD_9_123456"),
    ] {
        source = source.replace(from, to);
    }
    source = source.replace("386536690", "123456789");
    check(&source, "$THREAD_9_123456", true);
}
#[test]
fn exact_schema_semantic_refusals() {
    for (from, to) in [
        ("$CR.newValue === null", "$CR.newValue !== null"),
        ("$CR.oldValue ?? null", "$CR.newValue ?? null"),
        ("$CS() ^ 130", "$CS() ^ 131"),
        ("$CU($CW, $CT, 3)", "$CU($CW, $CT, 4)"),
        ("$CS >>> 24", "$CS >> 24"),
        ("$CX ^ 386536690", "$CX ^ 386536691"),
        ("$KEY_7.next([[], $CY])", "$KEY_31.next([[], $CY])"),
        ("$KEY_7.next([[], $CY])", "$KEY_7.next([[], $CT])"),
        ("$KEY_7.next([[], $CT])", "$KEY_7.next([[], $CT, 1])"),
        (
            "$KEY_7.next([[], $CT])",
            "$KEY_7.next([[], $CT]); $KEY_7.complete()",
        ),
        (
            "let $DA = $KEY_24 | 0 | $KEY_20 << 1",
            "let $DA = $KEY_20 | 0 | $KEY_24 << 1",
        ),
        (
            "let $CV = $KEY_24 | 0 | $KEY_20 << 1;",
            "let $CV = $KEY_24 | 0 | $KEY_20 << 1; $KEY_24 = false;",
        ),
        (
            "let $CV = $KEY_24 | 0 | $KEY_20 << 1;",
            "let $CV = $KEY_24 | 0 | $KEY_20 << 1; let $KEY_28 = 'shadow';",
        ),
        ("$CY[$CZ + 1]", "$CY[$CZ + 2]"),
    ] {
        check(&mutate(from, to), TARGET, false);
    }
    let late = mutate("let $CV = $KEY_24 | 0 | $KEY_20 << 1;", "");
    let late = late.replacen(
        "$CT[$CU + 1] =",
        "let $CV = $KEY_24 | 0 | $KEY_20 << 1; $CT[$CU + 1] =",
        1,
    );
    check(&late, TARGET, false);
    let finalizer = mutate("\n\t\t\t}", " finally { cleanup(); }\n\t\t\t}");
    check(&finalizer, TARGET, false);
}
#[test]
fn scoped_origins_and_early_invocation_refusals() {
    for (from, to) in [
        (
            "$KEY_26 = $THREAD_7_92674;",
            "$KEY_26 = $THREAD_7_92674; $KEY_26({});",
        ),
        (
            "$KEY_26 = $THREAD_7_92674;",
            "$THREAD_7_92674({}); $KEY_26 = $THREAD_7_92674;",
        ),
        (
            "$KEY_26 = $THREAD_7_92674;",
            "expose($THREAD_7_92674); $KEY_26 = $THREAD_7_92674;",
        ),
        (
            "$KEY_28 = $KEY_19.persistentId.key;",
            "$KEY_28 = $KEY_19.persistentId.key; $KEY_28 = 'reset';",
        ),
        ("$KEY_7 = $BY[1];", "$KEY_7 = foreign;"),
        ("$KEY_24 = false;", "$KEY_24 = 17;"),
        ("$KEY_3 = null;", "$KEY_3 = 'unproved';"),
        (
            "$KEY_26 = $THREAD_7_92674;",
            "$KEY_320 = otherFactory; $KEY_26 = $THREAD_7_92674;",
        ),
    ] {
        // Anchors are restricted to the actual parent scope.
        let start = SOURCE[..381912].rfind("function $THREAD_8_30515").unwrap();
        let end = SOURCE[383062..]
            .find("\n\t\tfunction ")
            .map(|x| x + 383062)
            .unwrap_or(SOURCE.len());
        let parent = &SOURCE[start..end];
        assert!(parent.contains(from), "missing parent anchor {from}");
        let source = format!(
            "{}{}{}",
            &SOURCE[..start],
            parent.replacen(from, to, 1),
            &SOURCE[end..]
        );
        check(&source, TARGET, false);
    }
}
#[test]
fn unsupported_probe_topologies_remain_unclaimed() {
    check(
        include_str!("fixtures/program/keyed-deletion/keyed-deletion-heldout.js"),
        "$THREAD_6_35537",
        false,
    );
    check(
        include_str!("fixtures/program/keyed-deletion/keyed-deletion-baseline.js"),
        "$THREAD_8_162909",
        false,
    );
}

#[test]
fn early_hoisted_helpers_cannot_donate_late_initialization() {
    // The read occurs textually after setup, but the hoisted helper runs early.
    // Restrict this second mutation to the parent: earlier scopes can use the
    // same generated spelling, and must not supply evidence for this callback.
    let parent = SOURCE[..381912].rfind("function $THREAD_8_30515").unwrap();
    let prefix = &SOURCE[..parent];
    let body = SOURCE[parent..]
        .replacen(
            "$KEY_26 = $THREAD_7_92674;",
            "$KEY_26 = $THREAD_7_92674; earlyProbe();",
            1,
        )
        .replacen(
            "$KEY_3 = null;",
            "$KEY_3 = null; function earlyProbe() { $KEY_26({}); }",
            1,
        );
    check(&format!("{prefix}{body}"), TARGET, false);
    let before_factories = SOURCE.replacen(
        "let $KEY_320 = $THREAD_0_140423;",
        "$THREAD_8_30515([], {}, null, null, null, null); let $KEY_320 = $THREAD_0_140423;",
        1,
    );
    check(&before_factories, TARGET, false);
}

#[test]
fn original_live_factory_contracts_apply_to_generic_route() {
    for (from, to) in [
        (
            "$KEY_0 = $BY;\n\t\t\t$KEY_5 = -1;",
            "$KEY_0 = 0; $BY;\n\t\t\t$KEY_5 = -1;",
        ),
        ("return $THREAD_3_81589;", "return $THREAD_7_104621;"),
        ("return $THREAD_7_104621;", "return $THREAD_3_81589;"),
        (
            "let $KEY_1 = $BY;\n\t\t\tlet $KEY_2;",
            "let $KEY_1 = $BY; $KEY_1 = () => 0;\n\t\t\tlet $KEY_2;",
        ),
    ] {
        assert_eq!(SOURCE.matches(from).count(), 1, "factory anchor");
        check(&SOURCE.replacen(from, to, 1), TARGET, false);
    }
    check(&mutate("$CW == null", "$CW === null"), TARGET, false);
}
