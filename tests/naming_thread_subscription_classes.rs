use shape::naming::name_threads;
const SOURCE: &str = include_str!("fixtures/program/thread-owned-subscriptions.js");

#[test]
fn lifecycle_classes_edit_declarations_and_all_constructor_alias_references() {
    let result = name_threads(SOURCE, true).unwrap();
    assert_eq!(result.report.applied_class_count, 2, "{:?}", result.report.class_outcomes);
    for (original, canonical) in [("$THREAD_901_1", "Subscription"), ("$THREAD_901_2", "SubscriptionObserver")] {
        let c = result.report.class_outcomes.iter().find(|c| c.binding.name == original).unwrap();
        assert!(c.applied && c.reference_edits > 0, "{c:?}");
        assert_eq!(c.proposed_name.as_deref(), Some(canonical));
        assert!(!result.source.contains(original));
        assert!(result.report.outcomes.iter().all(|o| o.binding.name != original));
    }
    assert!(result.source.contains("unsubscribe()"));
    assert!(result.source.contains("next(input)"));
    let mut replay = SOURCE.to_string();
    let mut edits = result.report.edits;
    edits.sort_by_key(|e| e.start);
    for e in edits.iter().rev() { replay.replace_range(e.start as usize..e.end as usize, &e.replacement); }
    assert_eq!(replay, result.source);
    name_threads(&result.source, false).unwrap();
}

#[test]
fn lifecycle_dryrun_and_human_classes_preserve_bytes() {
    let report = name_threads(SOURCE, false).unwrap();
    assert_eq!(report.source, SOURCE);
    assert!(report.report.edits.is_empty());
    assert_eq!(report.report.applied_class_count, 0);
    assert!(report.report.class_outcomes.iter().all(|c| c.proposed_name.is_some()));
    let human = SOURCE.replace("$THREAD_901_1", "UserSubscription").replace("$THREAD_901_2", "UserObserver");
    assert_eq!(name_threads(&human, true).unwrap().source, human);
}

#[test]
fn lifecycle_collision_capture_and_dynamic_scopes_refuse_without_property_edits() {
    for source in [
        SOURCE.replace("function bootstrap() {", "function bootstrap() { let Subscription = 1; consume(Subscription);"),
        SOURCE.replace("function bootstrap() {", "function bootstrap() { consume(Subscription);"),
        format!("eval('');{SOURCE}"),
        format!("with (context) {{ {SOURCE} }}"),
    ] {
        let result = name_threads(&source, true).unwrap();
        let sub = result.report.class_outcomes.iter().find(|c| c.binding.name == "$THREAD_901_1").unwrap();
        assert!(!sub.applied, "{sub:?}");
        assert!(sub.reason.is_some());
        assert!(result.source.contains("unsubscribe()"));
    }
}

#[test]
fn lifecycle_wrong_receiver_mutation_and_shape_only_donors_refuse() {
    for (old, new) in [
        ("this._cleanup = void 0; this._observer = observer;", "other._cleanup = void 0; this._observer = observer;"),
        ("try { cleanup(); }", "try { unrelated(); }"),
        ("const subscriptionAlias", "$THREAD_901_1 = other; const subscriptionAlias"),
    ] {
        assert!(SOURCE.contains(old));
        let result = name_threads(&SOURCE.replace(old, new), true).unwrap();
        assert!(!result.report.class_outcomes.iter().find(|c| c.binding.name == "$THREAD_901_1").unwrap().applied);
    }
}

const INSTALLED: &str = include_str!("fixtures/program/thread-installed-subscriptions.js");

#[test]
fn installed_closed_descriptors_route_both_class_bindings_and_resolved_references() {
    let result = name_threads(INSTALLED, true).unwrap();
    assert_eq!(result.report.applied_class_count, 2, "{:?}", result.report.class_outcomes);
    for (binding, name) in [("$THREAD_8_57126", "Subscription"), ("$THREAD_0_94586", "SubscriptionObserver")] {
        let class = result.report.class_outcomes.iter().find(|c| c.binding.name == binding).unwrap();
        assert!(class.applied && class.reference_edits >= 3, "{class:?}");
        assert_eq!(class.proposed_name.as_deref(), Some(name));
        assert!(!result.source.contains(binding));
        assert!(result.source.contains(&format!("Object.defineProperty({name}.prototype,")));
        assert!(result.source.contains(&format!("Object.defineProperty({name}, \"prototype\", {{ writable: false }})")));
        assert!(result.report.outcomes.iter().all(|o| o.binding.name != binding));
    }
    assert!(result.source.contains("unsubscribe()"));
    assert!(result.source.contains("key: \"closed\""));
    let mut replay = INSTALLED.to_string();
    for edit in result.report.edits.iter().rev() {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
    name_threads(&result.source, false).unwrap();
    let dry = name_threads(INSTALLED, false).unwrap();
    assert_eq!(dry.source, INSTALLED);
    assert!(dry.report.edits.is_empty());
}

#[test]
fn installed_foreign_descriptor_owner_refuses_class_routing() {
    let source = INSTALLED.replace("Object.defineProperty($THREAD_8_57126.prototype,", "Object.defineProperty(foreign.prototype,");
    let result = name_threads(&source, true).unwrap();
    let class = result.report.class_outcomes.iter().find(|c| c.binding.name == "$THREAD_8_57126").unwrap();
    assert!(!class.applied, "{class:?}");
    assert!(result.source.contains("class $THREAD_8_57126"));
}
