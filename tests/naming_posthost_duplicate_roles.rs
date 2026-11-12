use shape::naming::labels::build_label_reference;
use shape::naming::{name_threads, name_threads_with_labels};

fn outcome<'a>(
    report: &'a shape::naming::ThreadNamingReport,
    binding: &str,
) -> &'a shape::naming::ThreadName {
    report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == binding)
        .unwrap_or_else(|| panic!("missing outcome for {binding}"))
}

/// Replay edits explicitly by descending byte offset. Never reverse an
/// unsorted edit list: overlapping coordinates would silently drift.
fn replay(
    source: &str,
    result: &str,
    edits: &[shape::naming::program::ProgramSourceEdit],
) -> String {
    let mut replay = source.to_owned();
    let mut edits = edits.to_vec();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    for edit in edits {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result);
    replay
}

const SOLO: &str =
    r#"function main(){function $THREAD_1_10(e){sink.error(e);}return $THREAD_1_10;}main();"#;

#[test]
fn solo_claim_keeps_the_meaningful_unsuffixed_name() {
    let result = name_threads(SOLO, true).unwrap();
    let solo = outcome(&result.report, "$THREAD_1_10");
    assert!(solo.applied);
    assert_eq!(solo.role_name.as_deref(), Some("forwardError"));
    assert_eq!(solo.proposed_name.as_deref(), Some("forwardError"));
    assert!(result
        .source
        .contains("function forwardError(e){sink.error(e);}"));
}

const DUPLICATES: &str = r#"function main(){function $THREAD_1_10(e){sink.error(e);}function $THREAD_2_20(e){sink.error(e);}return [$THREAD_1_10,$THREAD_2_20];}main();"#;

#[test]
fn same_scope_duplicates_get_distinct_ordinals_and_keep_the_canonical_role() {
    let result = name_threads(DUPLICATES, true).unwrap();
    let first = outcome(&result.report, "$THREAD_1_10");
    let second = outcome(&result.report, "$THREAD_2_20");
    assert!(first.applied && second.applied);
    assert_eq!(first.proposed_name.as_deref(), Some("forwardError"));
    assert_eq!(second.proposed_name.as_deref(), Some("forwardError2"));
    assert_eq!(first.role_name.as_deref(), Some("forwardError"));
    assert_eq!(second.role_name.as_deref(), Some("forwardError"));
    assert!(result
        .source
        .contains("function forwardError(e){sink.error(e);}"));
    assert!(result
        .source
        .contains("function forwardError2(e){sink.error(e);}"));
    // Canonical role metadata still names the role for both concrete copies.
    for original in ["$THREAD_1_10", "$THREAD_2_20"] {
        assert!(result.report.function_roles.iter().any(|role| {
            role.current_name.as_deref() == Some(original)
                && role.candidates == vec!["forwardError".to_string()]
        }));
    }
}

const SUFFIXED: &str = r#"function main(){function $THREAD_1_10(e){sink.error(e);}function $THREAD_2_20_2(e){sink.error(e);}return [$THREAD_1_10,$THREAD_2_20_2];}main();"#;

#[test]
fn suffixed_original_thread_gets_the_next_ordinal_without_renaming_the_base() {
    let result = name_threads(SUFFIXED, true).unwrap();
    let base = outcome(&result.report, "$THREAD_1_10");
    let clone = outcome(&result.report, "$THREAD_2_20_2");
    assert!(base.applied);
    assert!(clone.applied);
    assert_eq!(base.proposed_name.as_deref(), Some("forwardError"));
    assert_eq!(clone.proposed_name.as_deref(), Some("forwardError2"));
    assert_eq!(clone.role_name.as_deref(), Some("forwardError"));
    assert_eq!(clone.disambiguator, Some(2));
}

const SIBLINGS: &str = r#"function main(){function a(){function $THREAD_1_10(e){sink.error(e);}return $THREAD_1_10;}function b(){function $THREAD_2_20_2(e){sink.error(e);}return $THREAD_2_20_2;}return [a,b];}main();"#;

#[test]
fn sibling_scopes_keep_the_shared_base_name() {
    let result = name_threads(SIBLINGS, true).unwrap();
    assert!(result.report.outcomes.iter().all(|outcome| outcome.applied));
    assert_eq!(
        outcome(&result.report, "$THREAD_1_10")
            .proposed_name
            .as_deref(),
        Some("forwardError")
    );
    assert_eq!(
        outcome(&result.report, "$THREAD_2_20_2")
            .proposed_name
            .as_deref(),
        Some("forwardError")
    );
}

const CAPTURE: &str = r#"function main(){function $THREAD_1_10(e){sink.error(e);}function wrap(){function $THREAD_2_20(e){sink.error(e);}return $THREAD_1_10;}return [$THREAD_1_10,wrap];}main();"#;

#[test]
fn ancestor_reference_capture_stays_on_the_outer_binding() {
    let result = name_threads(CAPTURE, true).unwrap();
    let outer = outcome(&result.report, "$THREAD_1_10");
    let inner = outcome(&result.report, "$THREAD_2_20");
    assert!(outer.applied && inner.applied);
    assert_eq!(outer.proposed_name.as_deref(), Some("forwardError"));
    assert_eq!(inner.proposed_name.as_deref(), Some("forwardError2"));
    assert!(result
        .source
        .contains("function forwardError(e){sink.error(e);}"));
    assert!(result
        .source
        .contains("function forwardError2(e){sink.error(e);}"));
    // The reference inside `wrap` resolves to the outer binding, so it must
    // move to the outer concrete name and never be captured by the inner copy.
    assert!(result.source.contains("return forwardError;"));
    assert!(!result.source.contains("return forwardError2;"));
}

const HUMAN: &str = r#"function main(){let forwardError=1;consume(forwardError);function $THREAD_1_10(e){sink.error(e);}function $THREAD_2_20(e){sink.error(e);}return [$THREAD_1_10,$THREAD_2_20];}main();"#;

const UNRESOLVED: &str = r#"function main(){consume(forwardError);function $THREAD_1_10(e){sink.error(e);}function $THREAD_2_20(e){sink.error(e);}return [$THREAD_1_10,$THREAD_2_20];}main();"#;

#[test]
fn human_and_unresolved_names_are_reserved_and_left_untouched() {
    for source in [HUMAN, UNRESOLVED] {
        let result = name_threads(source, true).unwrap();
        let mut concrete: Vec<_> = result
            .report
            .outcomes
            .iter()
            .filter(|outcome| outcome.binding.name.starts_with("$THREAD_"))
            .map(|outcome| {
                assert!(outcome.applied);
                outcome
                    .proposed_name
                    .clone()
                    .expect("generated duplicate must get a concrete name")
            })
            .collect();
        concrete.sort();
        // The reserved spelling is never reused for a generated duplicate.
        assert_eq!(
            concrete,
            vec!["forwardError2".to_string(), "forwardError3".to_string()]
        );
        assert!(result.source.contains("consume(forwardError)"));
    }
}

const LABEL_ORIGINAL: &str = r#"function main(){
function $THREAD_1_10(value){return value.length;}
function $THREAD_2_20(value){return value.slice(0,2);}
return [$THREAD_1_10,$THREAD_2_20];}main();"#;

fn labeled() -> String {
    LABEL_ORIGINAL
        .replace("$THREAD_1_10", "readLength")
        .replace("$THREAD_2_20", "takeFirstTwo")
}

const LABEL_DUPLICATES: &str = r#"function main(){
function $THREAD_8_1(value){return value.length;}
function $THREAD_8_2(value){return value.length;}
return [$THREAD_8_1,$THREAD_8_2];}main();"#;

#[test]
fn inherited_labels_keep_canonical_roles_while_concrete_names_diverge() {
    let reference = build_label_reference(LABEL_ORIGINAL, &labeled()).unwrap();
    let result = name_threads_with_labels(LABEL_DUPLICATES, true, Some(&reference)).unwrap();
    let first = outcome(&result.report, "$THREAD_8_1");
    let second = outcome(&result.report, "$THREAD_8_2");
    assert!(first.applied && second.applied);
    assert_eq!(first.role_name.as_deref(), Some("readLength"));
    assert_eq!(second.role_name.as_deref(), Some("readLength"));
    assert_eq!(first.proposed_name.as_deref(), Some("readLength"));
    assert_eq!(second.proposed_name.as_deref(), Some("readLength2"));
    assert!(result
        .source
        .contains("function readLength(value){return value.length;}"));
    assert!(result
        .source
        .contains("function readLength2(value){return value.length;}"));
}

#[test]
fn edit_replay_sorts_explicitly_by_descending_byte_offset() {
    let result = name_threads(DUPLICATES, true).unwrap();
    assert!(result.report.edits.len() >= 2);
    replay(DUPLICATES, &result.source, &result.report.edits);
}
