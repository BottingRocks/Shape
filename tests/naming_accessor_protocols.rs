use shape::naming::name_threads;
const SOURCE: &str = include_str!("fixtures/program/reflection-protocols/accessor.js");
fn accepted(s: &str) -> bool {
    name_threads(s, true)
        .unwrap()
        .report
        .outcomes
        .iter()
        .any(|o| o.applied && o.proposed_name.as_deref() == Some("installAccessorInterceptor"))
}
#[test]
fn accessor_correlated_registry_alias() {
    assert!(accepted(SOURCE));
    let bounds = "$AO = null;\n\t\t\t\t\t\t\t\t\t$AP = null;";
    assert!(SOURCE.contains(bounds));
    for s in [
        SOURCE.replace(bounds, "$AO = null; $AP = false;"),
        SOURCE.replace("$AP = $AO;", "$AP = $AK;"),
        SOURCE.replace("$AP = $AO;", "sideEffect(); $AP = $AO;"),
        SOURCE.replace("if ($AK !== $AO.object)", "if ($AK === $AO.object)"),
        SOURCE.replace("$AP.original.get", "$AO.original.set"),
        SOURCE.replace("$KEY_194($KEY_13, this)", "$KEY_194($KEY_7, this)"),
    ] {
        assert_ne!(s, SOURCE);
        assert!(!accepted(&s));
    }
    assert!(accepted(
        &SOURCE
            .replace("$AO", "selected")
            .replace("$AP", "cachedRecord")
            .replace("$AQ", "cursor")
    ));
}
