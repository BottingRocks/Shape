use shape::naming::name_threads;
fn bundle(source: &str) -> bool {
    let result = name_threads(source, true).unwrap();
    ["createSeededWorkerEnvironmentBundle", "emitSeededWorkerEnvironmentBundle"].iter().all(|role|
        result.report.outcomes.iter().any(|o| o.applied && o.role_name.as_deref() == Some(role)))
}
#[test]
fn stored_bundle_variants() {
    let fixtures: serde_json::Value = serde_json::from_str(include_str!("fixtures/seeded-worker-environment-bundles.json")).unwrap();
    let mut rejected = vec![];
    for f in fixtures.as_array().unwrap() {
        let source = f["source"].as_str().unwrap();
        if !bundle(source) { rejected.push(f["key"].as_str().unwrap()); }
        for (from, to) in [("Math.pow(2, 52)", "Math.pow(2, 51)"), ("^ 129", "^ 128")] {
            assert!(source.contains(from));
            assert!(!bundle(&source.replacen(from, to, 1)), "{} accepted {from}", f["key"]);
        }
    }
    assert!(rejected.is_empty(), "rejected: {rejected:?}");
}
#[test]
fn bundle_closed_dependencies_reject_mutations() {
    let fixtures: serde_json::Value = serde_json::from_str(include_str!("fixtures/seeded-worker-environment-bundles.json")).unwrap();
    let source = fixtures[0]["source"].as_str().unwrap();
    for (from,to) in [
        ("Math.floor", "Math.ceil"),
        ("Math.abs", "Math.sin"),
        ("Math.pow(2, 52)", "Math.pow(2, 51)"),
        ("248, 127", "247, 127"),
        ("Object.create(null)", "Object.create({})"),
        ("^ 129", "^ 128"),
        ("^ 130", "^ 129"),
        ("location.origin", "location.hostname"),
        ("postMessage;", "otherPostMessage;"),
        ("Array.call.bind(String.prototype.charCodeAt)", "Array.call.bind(String.prototype.charAt)"),
    ] {
        assert!(source.contains(from), "missing mutation {from}");
        assert!(!bundle(&source.replacen(from,to,1)), "accepted mutation {from} => {to}");
    }
    // Alpha-renaming preserves resolved identities and still passes.
    assert!(bundle(&source.replace("$KEY_", "$CAPTURE_").replace("$THREAD_", "$THREAD_9")));
}
fn prng(source:&str)->bool {
    name_threads(&format!("function main(){{ {source} return $THREAD_9_13908; }}main();"),true).unwrap().report.outcomes.iter().any(|o|
        o.applied && o.role_name.as_deref()==Some("appendSineDerivedSamples"))
}
#[test]
fn existing_prng_role_and_effect_order() {
    let source=include_str!("fixtures/sine-derived-sample-effects.js");
    assert!(prng(source));
    for (from,to) in [("++$AO;\n\t\t\t\t\t\t\t\tcontinue;", "continue;"),
        ("$AP < 50", "$AP < 51"), ("$AP - $AO", "$AO - $AP"),
        ("$AN.push($AQ)", "$AN.push($AO)"), ("Math.fround", "Math.floor"),
        ("continue;", "break;"), ("Number($AN[1]) + 1", "Number($AN[1]) + 2")]
    {
        assert!(source.contains(from), "missing mutation {from}");
        assert!(!prng(&source.replacen(from,to,1)), "accepted {from} => {to}");
    }
}
#[test]
fn literal_hardware_read_requires_its_null_guard() {
    let fixtures: serde_json::Value = serde_json::from_str(include_str!("fixtures/seeded-worker-environment-bundles.json")).unwrap();
    let source=fixtures.as_array().unwrap().iter().find(|f|f["key"]=="worker-bundle-push-output-3-capture-slots-24-guards-15-choices-4-argument-helper-baseline").unwrap()["source"].as_str().unwrap();
    let guarded="if (navigator.hardwareConcurrency == null) {\n\t\t\t$KEY_7[++$E] = $KEY_8() ^ 130;\n\t\t} else {\n\t\t\t$E = $KEY_9($KEY_8, navigator.hardwareConcurrency, $KEY_7, $E);\n\t\t}";
    assert!(source.contains(guarded));
    assert!(!bundle(&source.replace(guarded,"$E = $KEY_9($KEY_8, navigator.hardwareConcurrency, $KEY_7, $E);")));
}
