use shape::naming::name_threads;
const FIRST: &str = include_str!("fixtures/closed-emission-protocols/tuple-reverse-and-storage.js");
const SECOND: &str = include_str!("fixtures/closed-emission-protocols/paste-callback-and-network.js");
fn check(source: &str, expected: &[(&str, &str)]) {
    let r = name_threads(source, true).unwrap();
    for (binding, role) in expected {
        let o = r
            .report
            .outcomes
            .iter()
            .find(|o| {
                o.binding.declaration.start as usize
                    == source
                        .rfind(&format!("function {binding}("))
                        .expect("source declaration")
            })
            .expect("binding exists");
        assert_eq!(
            o.role_name.as_deref(),
            Some(*role),
            "binding {binding}: {:?}",
            o.reason
        );
        assert!(o.applied, "binding {binding}");
    }
}
#[test]
fn assigned_closed_positive_protocols() {
    check(
        FIRST,
        &[
            ("$THREAD_8_75327", "emitSeededFiveFieldTuple"),
            ("$THREAD_3_47870", "emitSeededFiveFieldTuple"),
            ("$THREAD_0_94930", "emitReverseArrayEncoding"),
            ("$THREAD_0_94930_2", "emitReverseArrayEncoding"),
            ("$THREAD_5_128796", "emitSeededChannelResult"),
            ("$THREAD_9_118060", "emitStorageRemovalSignal"),
        ],
    );
    check(
        SECOND,
        &[
            ("$THREAD_5_86532", "countInputPasteAndMarkInactiveTarget"),
            ("$THREAD_5_86532_2", "countInputPasteAndMarkInactiveTarget"),
            ("$THREAD_3_70501", "foldReverseCallbackListOperation"),
            ("$THREAD_7_39214", "forwardCallbackToListOperation"),
            ("$THREAD_7_95034", "nullCallbackListSlotFromOperation"),
            ("$THREAD_3_88844", "replaceCallbackListWithPeerOperation"),
            ("$THREAD_0_13192", "collectDistinctCandidateIpMatches"),
            ("$THREAD_7_26784", "emitStorageRemovalSignal"),
        ],
    );
}
#[test]
fn tuple_reverse_and_storage_near_misses() {
    // Mutate only concrete complete-body edges; all original enclosing capture
    // implementations stay intact and cannot be donated from another fixture.
    let source = FIRST
        .replace("$CS[4], $KEY_3, $CU", "$CS[0], $KEY_3, $CU")
        .replace("$EH[$EK - 1]", "$EH[$EK]")
        .replace("$CG = $CG | $KEY_17 << 2", "$CG = $CG | $KEY_17 << 1");
    let r = name_threads(&source, true).unwrap();
    for binding in [
        "$THREAD_8_75327",
        "$THREAD_0_94930",
        "$THREAD_0_94930_2",
        "$THREAD_9_118060",
    ] {
        let o = r
            .report
            .outcomes
            .iter()
            .find(|o| {
                o.binding.declaration.start as usize
                    == source
                        .rfind(&format!("function {binding}("))
                        .expect("source declaration")
            })
            .unwrap();
        assert!(!o.applied, "near miss {binding}");
    }
}
#[test]
fn callback_and_candidate_near_misses() {
    let source = SECOND
        .replace(
            "this.__callbacks[$DG] = null",
            "this.__callbacks[$DG] = true",
        )
        .replace("$DG($DF, $DJ)", "$DG($DJ, $DF)")
        .replace("$DI = $DI($DF, $DJ)", "$DI = $DI($DJ, $DF)")
        .replace("$DJ.ip === $KEY_0", "$DJ.ip !== $KEY_0")
        .replace(
            "$KEY_325($KEY_355, this.__callbacks, $DF)",
            "$KEY_325($KEY_355, this.__callbacks, this)",
        );
    let r = name_threads(&source, true).unwrap();
    for binding in [
        "$THREAD_7_95034",
        "$THREAD_3_70501",
        "$THREAD_0_13192",
        "$THREAD_7_39214",
    ] {
        let o = r
            .report
            .outcomes
            .iter()
            .find(|o| {
                o.binding.declaration.start as usize
                    == source
                        .rfind(&format!("function {binding}("))
                        .expect("source declaration")
            })
            .unwrap();
        assert!(!o.applied, "near miss {binding}");
    }
}

#[test]
fn unrelated_capture_write_cannot_donate_writer_identity() {
    let source = FIRST.replace("$KEY_250 = $KEY_250;", "$KEY_250 = unrelatedWriter;");
    let r = name_threads(&source, true).unwrap();
    for binding in ["$THREAD_8_75327", "$THREAD_0_94930", "$THREAD_5_128796"] {
        let at = source.rfind(&format!("function {binding}(")).unwrap();
        let o = r
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.declaration.start as usize == at)
            .unwrap();
        assert!(!o.applied, "foreign writer: {binding}");
    }
}

#[test]
fn alpha_named_callable_slots_keep_scoped_protocols() {
 let source=FIRST.replace("$KEY_250", "numericPort").replace("$KEY_102", "streamPort");
 check(&source,&[("$THREAD_8_75327","emitSeededFiveFieldTuple"),("$THREAD_0_94930","emitReverseArrayEncoding"),("$THREAD_5_128796","emitSeededChannelResult")]);
}
