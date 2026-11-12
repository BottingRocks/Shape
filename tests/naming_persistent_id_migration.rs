use shape::naming::name_threads;

const SOURCE: &str = include_str!("fixtures/program/thread-persistent-id-migration.js");
const NAME: &str = "migrateAndInitializePersistentId";
fn claim(source: &str) -> bool {
    let result = name_threads(source, true).unwrap();
    result.report.outcomes.iter().any(|o| o.applied
        && o.proposed_name.as_deref() == Some(NAME)
        && o.evaluator.as_deref() == Some("persistent_id_migration"))
}

#[test]
fn cached_migration_and_alpha_variant_preserve_identifier_only_edits() {
    for source in [SOURCE.to_string(), SOURCE
        .replace("$KEY_2", "settings").replace("$KEY_300", "provider")
        .replace("$KEY_120", "available").replace("$AZ", "legacyKey")
        .replace("$BA", "oldValue").replace("$BC", "decoded")
        .replace("$BE", "currentKey").replace("$BF", "token")
        .replace("\t", " ")] {
        assert!(claim(&source));
        let result = name_threads(&source, true).unwrap();
        let mut replay = source.clone();
        for edit in result.report.edits.iter().rev() {
            replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
        }
        assert_eq!(replay, result.source);
        assert!(result.source.contains("migrateAndInitializePersistentId();"));
    }
}

#[test]
fn migration_rejects_wrong_origins_decode_and_effects() {
    for (from, to) in [
        ("getItem(\"f\")", "getItem(\"other\")"),
        ("removeItem(\"f\")", "removeItem(\"other\")"),
        ("0123456789-_=", "0123456789+/="),
        ("$BC[5] >= 90", "$BC[5] >= 89"),
        ("$BC[6] === 1", "$BC[6] === 2"),
        ("$BC[8] === 0", "$BB[8] === 0"),
        ("$BB[$BA[$BD + 3]] !== 64", "$BB[$BA[$BD + 3]] === 64"),
        ("$BB[$BA[$BD]] << 2", "$BB[$BA[$BD]] << 3"),
        ("$KEY_300.setItem($AZ, $BA)", "$KEY_300.setItem($AZ, $BF)"),
        ("$KEY_300.setItem($BE, $BF)", "$KEY_300.setItem($BE, $BA)"),
        ("$KEY_300.setItem($BE, $BF)", "$KEY_300.setItem($BF, $BE)"),
        ("$KEY_2.transactionToken", "$OTHER.transactionToken"),
        ("$KEY_2.persistentId.resetKey", "$OTHER.persistentId.resetKey"),
        ("$KEY_300.removeItem($KEY_2.persistentId.resetKey)", "$OTHER.removeItem($KEY_2.persistentId.resetKey)"),
        ("if (!$KEY_300.getItem($BE))", "if ($KEY_300.getItem($BE))"),
        ("throw new Error(", "throw new TypeError("),
    ] {
        assert!(SOURCE.contains(from), "bad mutation: {from}");
        assert!(!claim(&SOURCE.replace(from, to)), "accepted mutation: {from} -> {to}");
    }
}

#[test]
fn migration_rejects_local_providers_shadowed_error_and_nested_donors() {
    let marker = "function $THREAD_6_57781() {";
    for declaration in ["let $KEY_2 = {};", "let $KEY_300 = {};", "let $KEY_120 = false;", "let Error = CustomError;"] {
        assert!(!claim(&SOURCE.replace(marker, &format!("{marker}\n{declaration}"))));
    }
    // A caller/outer wrapper cannot borrow the independently owned inner proof.
    let nested = SOURCE.replace(marker, "function $THREAD_7_77() { function $THREAD_6_57781() {")
        .replace("$THREAD_6_57781();", "} $THREAD_7_77();");
    let result = name_threads(&nested, true).unwrap();
    assert!(!result.report.outcomes.iter().any(|o| o.binding.name == "$THREAD_7_77"
        && o.proposed_name.as_deref() == Some(NAME)));
    assert!(!claim(&SOURCE.replace("let $AZ =", "let $KEY_2 = null; let $AZ =")));
    // A local key must not be rewritten by a later callback.
    assert!(!claim(&SOURCE.replace("$KEY_61 =", "const later = () => { $AZ = 'other'; }; $KEY_61 =")));
}

#[test]
fn migration_preserves_human_names_and_collision_guards() {
    assert!(!claim(&SOURCE.replace("$THREAD_6_57781", "existingInitializer")));
    assert!(!claim(&SOURCE.replace("function main() {", "function main() { let migrateAndInitializePersistentId = 1; use(migrateAndInitializePersistentId);")));
}

const SHARED: &str = include_str!("fixtures/program/thread-persistent-id-migration-shared.js");
const HELDOUT: &str = include_str!("fixtures/program/thread-persistent-id-migration-heldout.js");
const PROTOCOL: &str = "runPersistentIdMigrationProtocol";
const PROTOCOL_EVALUATOR: &str = "owned_persistent_id_migration_protocol_definition";

fn assert_protocol(source: &str, binding: &str, span: Option<(u32, u32)>) {
    let result = name_threads(source, true).unwrap();
    let outcomes: Vec<_> = result.report.outcomes.iter().filter(|o| o.binding.name == binding).collect();
    assert_eq!(outcomes.len(), 1, "exact target {binding}");
    let outcome = outcomes[0];
    assert!(outcome.applied, "{outcome:?}");
    assert_eq!(outcome.proposed_name.as_deref(), Some(PROTOCOL));
    assert_eq!(outcome.role_name.as_deref(), Some(PROTOCOL));
    assert_eq!(outcome.evaluator.as_deref(), Some(PROTOCOL_EVALUATOR));
    if let Some((start, end)) = span {
        assert_eq!(outcome.binding.declaration.start, start);
        assert_eq!(outcome.binding.declaration.end, end);
    }
    assert!(outcome.explanation.iter().any(|e| e.contains("opaque and effectful")
        && e.contains("no entry readiness") && e.contains("persistence success")));
    assert!(!outcome.explanation.iter().any(|e| e.contains("closed initialized") || e.contains("fully owned adapter")));
    let mut replay = source.to_owned();
    for edit in result.report.edits.iter().rev() {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source, "scoped identifier edits must replay exactly");
    assert!(result.source.contains(PROTOCOL));
}

fn refuses_protocol(source: &str, binding: &str) {
    let result = name_threads(source, true).unwrap();
    let outcome = result.report.outcomes.iter().find(|o| o.binding.name == binding).unwrap();
    assert_ne!(outcome.evaluator.as_deref(), Some(PROTOCOL_EVALUATOR), "{outcome:?}");
    assert_ne!(outcome.role_name.as_deref(), Some(PROTOCOL), "{outcome:?}");
}

#[test]
fn operational_protocol_and_independent_port_layout_have_exact_target_claims() {
    assert_protocol(SHARED, "$THREAD_1_1234", None);
    assert_protocol(HELDOUT, "$THREAD_2_991", None);
    assert_protocol(&SHARED.replace("++cursor;", "cursor++;"), "$THREAD_1_1234", None);
    assert_protocol(&SHARED.replace("cursor", "scan").replace("legacy", "oldText")
        .replace("store", "port").replace("settings", "optionsPort"), "$THREAD_1_1234", None);
    // The for-loop form retains its existing claim and explanation.
    let old_for = SHARED.replace(
        "while (cursor < \"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=\".length) {\n                            alphabet[\"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=\"[cursor]] = cursor;\n                            ++cursor;\n                        }",
        "for (; cursor < \"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=\".length; cursor++) { alphabet[\"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_=\"[cursor]] = cursor; }");
    assert_ne!(old_for, SHARED);
    assert!(claim(&old_for));
    refuses_protocol(&old_for, "$THREAD_1_1234");
}

#[test]
fn protocol_rejects_changed_owned_decode_order_receivers_and_effects() {
    for (from, to) in [
        ("0123456789-_=", "0123456789+/="),
        ("++cursor;", "cursor += 2;"),
        ("++cursor;", "++cursor; notify();"),
        ("alphabet = {};", "alphabet = external.table;"),
        ("++cursor;", "{ let cursor = 0; ++cursor; }"),
        ("bytes[5] <= 190", "bytes[5] <= 191"),
        ("bytes[6] === 1", "bytes[6] === 2"),
        ("alphabet[legacy[offset + 3]] !== 64", "alphabet[legacy[offset + 3]] === 64"),
        ("legacy[offset]] << 2", "legacy[offset]] << 3"),
        ("store.removeItem(\"f\");", "notify(); store.removeItem(\"f\");"),
        ("store.removeItem(\"f\");", "store.setItem(key, legacy); store.removeItem(\"f\");"),
        ("store.setItem(key, legacy)", "store.setItem(legacy, key)"),
        ("store.setItem(key, legacy)", "store.setItem(key, token)"),
        ("store.setItem(key, legacy)", "settings.setItem(key, legacy)"),
        ("ready = false;", "ready = true;"),
        ("store.removeItem(settings.persistentId.resetKey)", "store.removeItem(settings.transactionToken)"),
        ("if (!store.getItem(currentKey)) { store.setItem(currentKey, token); }", "if (!store.getItem(currentKey)) { store.setItem(currentKey, token); } store.removeItem(settings.persistentId.resetKey);"),
        ("leaseValue = \"\" + Date.now() + entropy();", "leaseValue = entropy() + \"\" + Date.now();"),
        ("notify();", "notify(settings);"),
        ("consumeToken(settings.transactionToken)", "consumeToken.call(settings, settings.transactionToken)"),
        ("captured.next(value)", "observer.next(value)"),
        ("pending.push(item.afterReady)", "pending.push(item.afterReady())"),
        ("report(error)", "report(moduleError)"),
        ("consumeToken(settings.transactionToken)", "entropy(settings.transactionToken)"),
        ("report(error)", "notify(error)"),
        ("$THREAD_1_1234() {", "$THREAD_1_1234() { let store = external.store;"),
        ("$THREAD_1_1234() {", "$THREAD_1_1234() { let Date = external.Date;"),
        ("++cursor;", "++cursor; let tuple = [alphabet, legacy, offset]; tuple[0] = external.table;"),
        ("notify();", "notify(); return;"),
        ("tokenState = consumeToken(settings.transactionToken);\n        notify();", "notify();\n        tokenState = consumeToken(settings.transactionToken);"),
        ("} catch (moduleError) {", "} catch (moduleError) { notify();"),
    ] {
        assert!(SHARED.contains(from), "invalid mutation {from}");
        refuses_protocol(&SHARED.replace(from, to), "$THREAD_1_1234");
    }
    refuses_protocol(&SHARED.replace("notify();", ""), "$THREAD_1_1234");
    refuses_protocol(&HELDOUT.replace("destination.next(value)", "destination.next()"), "$THREAD_2_991");
}

#[test]
fn opaque_port_changes_are_definition_claims_never_ready_or_algorithm_facts() {
    // These alter runtime behavior, but not the complete OWNED definition.
    // They must not be rejected or promoted to the stronger legacy role.
    for source in [
        SHARED.replace("let store = {", "let store; const initialStore = {"),
        SHARED.replace("let settings =", "let settings; const laterSettings ="),
        SHARED.replace("let settings =", "external.publish($THREAD_1_1234); let settings ="),
        SHARED.replace("$THREAD_1_1234();", "store = external.store; $THREAD_1_1234();"),
        SHARED.replace("let entropy = () =>", "let entropy = () =>")
            .replace("logs.push([\"entropy\"]); return \"salt-A\";", "store = external.store; return 'changed';"),
        HELDOUT.replace("trace.push([\"lookup-get\"]);", "backend = external.backend; trace.push(['lookup-get']);"),
    ] {
        let binding = if source.contains("function $THREAD_2_991") { "$THREAD_2_991" } else { "$THREAD_1_1234" };
        assert_protocol(&source, binding, None);
        let result = name_threads(&source, true).unwrap();
        let outcome = result.report.outcomes.iter().find(|o| o.binding.name == binding).unwrap();
        assert_ne!(outcome.role_name.as_deref(), Some(NAME));
        assert_ne!(outcome.evaluator.as_deref(), Some("persistent_id_migration"));
    }
}

#[test]
fn protocol_preserves_binding_collisions_and_human_names() {
    let collision = SHARED.replace("function main(options = {}) {", "function main(options = {}) { let runPersistentIdMigrationProtocol = 1; use(runPersistentIdMigrationProtocol);");
    let result = name_threads(&collision, true).unwrap();
    let target = result.report.outcomes.iter().find(|o| o.binding.name == "$THREAD_1_1234").unwrap();
    assert!(!target.applied);
    assert!(result.source.contains("function $THREAD_1_1234()"));
    let human = SHARED.replace("$THREAD_1_1234", "userProtocol");
    let result = name_threads(&human, true).unwrap();
    assert!(!result.report.outcomes.iter().any(|o| o.binding.name == "userProtocol" && o.applied));
    assert_eq!(result.source, human);
}

#[test]
fn complete_migration_definitions_are_protocol_witnesses() {
    assert_protocol(include_str!("fixtures/program/migration-actual/storage-migration-baseline.js"),
        "$THREAD_8_113071", Some((440458, 443489)));
    assert_protocol(include_str!("fixtures/program/migration-actual/storage-migration-alternate.js"),
        "$THREAD_4_66559", Some((413684, 416714)));
}
