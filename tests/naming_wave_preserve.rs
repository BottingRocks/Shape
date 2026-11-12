use shape::naming::collectors::identify_collectors;
use shape::naming::name_threads;

const ROUTES: &str = include_str!("fixtures/program/focused-threads.js");

#[test]
fn corroborating_registry_and_wiring_positions_are_one_claim() {
    let result = identify_collectors(ROUTES, true).unwrap();
    assert_eq!(result.report.wiring.groups.len(), 2);
    assert_eq!(result.report.registered_groups.len(), 2);
    for slot in 1..=2 {
        let name = format!("waveCollector{slot}");
        assert_eq!(result.report.renames.iter().filter(|r|
            r.applied && r.proposed_name.as_deref() == Some(name.as_str())).count(), 1);
    }
}

#[test]
fn conflicting_independent_position_cannot_override_wiring() {
    let source = ROUTES.replace("function connect() {", r#"
      let another = [$THREAD_2_202, $THREAD_1_101, $THREAD_2_202];
      another.forEach((factory, i) => {
        coordinate([], factory, {}, function(){}, function(){}, false);
      });
      function connect() {"#);
    let result = identify_collectors(&source, true).unwrap();
    assert_eq!(result.report.wiring.groups.len(), 2);
    assert!(result.report.registered_groups.len() >= 2);
    for name in ["$THREAD_1_101", "$THREAD_2_202"] {
        let rename = result.report.renames.iter().find(|r|
            r.binding.as_ref().is_some_and(|b| b.name == name)).unwrap();
        assert!(!rename.applied);
        assert!(rename.proposed_name.is_none());
        assert!(rename.reason.as_deref().unwrap().contains("shared factory"));
    }
}

#[test]
fn four_actual_wiring_only_registries_preserve_all_52_prior_labels() {
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/program/wave-preserve/expected.json")).unwrap();
    let mut count = 0;
    for (key, source) in [
        ("wave-preservation-heldout", include_str!("fixtures/program/wave-preserve/wave-preservation-heldout.js")),
        ("wave-preservation-alternate", include_str!("fixtures/program/wave-preserve/wave-preservation-alternate.js")),
        ("residual-origin-baseline", include_str!("fixtures/program/wave-preserve/wave-preservation-reordered.js")),
        ("native-forward", include_str!("fixtures/program/wave-preserve/wave-preservation-baseline.js")),
    ] {
        let seed = expected.as_array().unwrap().iter().find(|r| r["key"] == key).unwrap();
        let roles = seed["expected_preserved_wave_names"].as_array().unwrap();
        let result = name_threads(source, true).unwrap();
        // These captures were wiring-only when the labels were pinned. A later
        // registry proof may also register groups here; the property under test
        // is that the 52 prior labels survive either way, so the route the
        // names come from is not asserted.
        assert_eq!(result.report.collectors.wiring.groups.len(), roles.len(), "{key}");
        for role in roles {
            let id = role["function_id"].as_str().unwrap();
            let name = role["name"].as_str().unwrap();
            assert!(result.report.outcomes.iter().any(|r|
                r.function_id == id && r.applied && r.proposed_name.as_deref() == Some(name)), "{key} {id} {name}");
            count += 1;
        }
    }
    assert_eq!(count, 52);
}
