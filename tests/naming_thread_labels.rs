use shape::naming::labels::build_label_reference;
use shape::naming::{name_threads, name_threads_with_labels};
const ORIGINAL: &str = r#"function main(){
function $THREAD_1_10(value){return value.length;}
function $THREAD_2_20(value){return value.slice(0,2);}
function $THREAD_3_30(value){return value.slice(0,3);}
return [$THREAD_1_10,$THREAD_2_20,$THREAD_3_30];}main();"#;
fn labeled() -> String {
    ORIGINAL
        .replace("$THREAD_1_10", "readLength")
        .replace("$THREAD_2_20", "takeFirstTwo")
        .replace("$THREAD_3_30", "takeFirstThree")
}
#[test]
fn independent_ids_names_and_declaration_order_match_roles() {
    let reference = build_label_reference(ORIGINAL, &labeled()).unwrap();
    let target = r#"function otherMain(){
function $THREAD_8_944(input){return input.slice(0,3);}
function $THREAD_7_345(input){return input.length;}
function $THREAD_9_109(input){return input.slice(0,2);}
return [$THREAD_7_345,$THREAD_9_109,$THREAD_8_944];}otherMain();"#;
    let result = name_threads_with_labels(target, true, Some(&reference)).unwrap();
    assert_eq!(result.report.applied_count, 3);
    assert!(result.source.contains("function readLength(input)"));
    assert!(result.source.contains("function takeFirstTwo(input)"));
    assert!(result.source.contains("function takeFirstThree(input)"));
    assert!(result
        .report
        .outcomes
        .iter()
        .all(|o| o.evaluator.as_deref() == Some("inherited_structural_label")));
    assert_eq!(
        name_threads_with_labels(target, false, Some(&reference))
            .unwrap()
            .source,
        target
    );
}
#[test]
fn conflicting_structural_labels_and_literal_api_differences_are_refused() {
    let mut reference = build_label_reference(ORIGINAL, &labeled()).unwrap();
    let mut duplicate = reference
        .labels
        .iter()
        .find(|l| l.name == "readLength")
        .unwrap()
        .clone();
    duplicate.name = "differentPurpose".into();
    reference.labels.push(duplicate);
    let result = name_threads_with_labels(ORIGINAL, true, Some(&reference)).unwrap();
    assert!(result.source.contains("function $THREAD_1_10("));
    assert!(result
        .report
        .reference_matches
        .iter()
        .any(|m| m.status == "ambiguous"));
    let reference = build_label_reference(ORIGINAL, &labeled()).unwrap();
    let changed = ORIGINAL
        .replace("value.length", "value.byteLength")
        .replace("slice(0,2)", "slice(0,4)");
    let result = name_threads_with_labels(&changed, true, Some(&reference)).unwrap();
    assert!(result.source.contains("function $THREAD_1_10("));
    assert!(result.source.contains("function $THREAD_2_20("));
    assert!(build_label_reference(ORIGINAL, &changed).is_err());
}
#[test]
fn capture_values_remain_significant_and_readable_labels_are_kept() {
    let original="function main(){let offset=2;function $THREAD_1_2(x){return x+offset;}return $THREAD_1_2;}main();";
    let labeled = original.replace("$THREAD_1_2", "addOffset");
    let reference = build_label_reference(original, &labeled).unwrap();
    let changed = original
        .replace("offset=2", "offset=3")
        .replace("$THREAD_1_2", "$THREAD_9_99");
    assert_eq!(
        name_threads_with_labels(&changed, true, Some(&reference))
            .unwrap()
            .source,
        changed
    );
    let human = original.replace("$THREAD_1_2", "existingReadableName");
    assert_eq!(
        name_threads_with_labels(&human, true, Some(&reference))
            .unwrap()
            .source,
        human
    );
}
#[test]
fn only_unreferenced_empty_scoped_declarations_are_removed_and_replay_exactly() {
    let source="function publicEmpty(){} function main(){function $THREAD_1_2(){}function retained(){}function liveThrow(){throw Error('live');}function child(){function retained(){}return 1;}return [retained,liveThrow,child];}main();";
    let result = name_threads(source, true).unwrap();
    assert_eq!(
        result.report.removals.iter().filter(|r| r.applied).count(),
        2
    );
    assert!(!result.source.contains("$THREAD_1_2"));
    assert!(result.source.contains("function publicEmpty(){}"));
    assert!(result.source.contains("function retained(){}"));
    assert!(result.source.contains("throw Error('live')"));
    let mut replay = source.to_string();
    let mut edits = result.report.edits.clone();
    edits.sort_by_key(|e| std::cmp::Reverse(e.start));
    for e in edits {
        replay.replace_range(e.start as usize..e.end as usize, &e.replacement);
    }
    assert_eq!(replay, result.source);
    assert_eq!(name_threads(source, false).unwrap().source, source);
    for dynamic in ["eval('x')", "with (host) {}"] {
        let source = format!("function main(){{function unused(){{}} {dynamic};return 1;}}main();");
        assert_eq!(name_threads(&source, true).unwrap().source, source);
    }
}
#[test]
fn native_class_methods_have_role_metadata_without_being_renamed() {
    let source = r#"function main(){function $THREAD_1_2(Base){class Encoder extends Base{
 constructor(depth,extension){super();this.keysets=[];this.stringHist={};this.remainingDepth=depth;this.depthBoundExtensionPoint=extension;}
 encode(value){this.encodeValue(value,[]);this.stringHist;this.keysets;return tags.STRLUT;}
 }return Encoder;}return $THREAD_1_2;}main();"#;
    let result = name_threads(source, true).unwrap();
    assert!(result
        .source
        .contains("function createValueEncoderClass(Base)"));
    assert!(result.source.contains("constructor(depth,extension)"));
    assert!(result
        .report
        .function_roles
        .iter()
        .any(|r| r.current_name.as_deref() == Some("constructor")
            && r.candidates.contains(&"initializeValueEncoder".to_string())
            && r.status == "metadata_only"));
}
#[test]
fn empty_callback_bodies_do_not_transfer_semantic_labels() {
    let original = "function main(){function $THREAD_1_2(){}return $THREAD_1_2;}main();";
    let reference = build_label_reference(
        original,
        &original.replace("$THREAD_1_2", "unsubscribeHook"),
    )
    .unwrap();
    assert!(!reference.labels.iter().any(|l| l.name == "unsubscribeHook"));
    let target = original.replace("$THREAD_1_2", "$THREAD_9_8");
    // The empty referenced declaration is still named by its own body
    // (`referenced_ordinary_noop`), never by the reference label.
    let with_labels = name_threads_with_labels(&target, true, Some(&reference)).unwrap();
    let without_labels = name_threads(&target, true).unwrap();
    assert!(!with_labels.source.contains("unsubscribeHook"));
    assert_eq!(with_labels.source, without_labels.source);
    assert_eq!(
        with_labels.source,
        "function main(){function noOp(){}return noOp;}main();"
    );
    let outcome = with_labels
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_9_8")
        .unwrap();
    assert_eq!(outcome.evaluator.as_deref(), Some("referenced_ordinary_noop"));
}
#[test]
fn capture_alias_relationships_survive_alpha_renaming() {
    let original="function main(){let a=[],b=[];function $THREAD_1_2(x){a.push(x);return b;}return $THREAD_1_2;}main();";
    let reference = build_label_reference(
        original,
        &original.replace("$THREAD_1_2", "appendAndReturnOther"),
    )
    .unwrap();
    let alpha = original
        .replace("a.push(x);return b", "left.push(x);return right")
        .replace("let a=[],b=[]", "let right=[],left=[]")
        .replace("$THREAD_1_2", "$THREAD_9_9");
    let positive = name_threads_with_labels(&alpha, true, Some(&reference)).unwrap();
    assert!(positive.source.contains("function appendAndReturnOther(x)"));
    let aliased = original
        .replace("return b", "return a")
        .replace("$THREAD_1_2", "$THREAD_8_8");
    let negative = name_threads_with_labels(&aliased, true, Some(&reference)).unwrap();
    assert!(!negative.source.contains("appendAndReturnOther"));
}
#[test]
fn reassigned_function_captures_cannot_inherit_old_body_signatures() {
    let original="function main(){function helper(x){return x.length;}function $THREAD_1_2(x){return helper(x);}return $THREAD_1_2;}main();";
    let reference =
        build_label_reference(original, &original.replace("$THREAD_1_2", "readViaHelper")).unwrap();
    let mutated = original.replace("return $THREAD_1_2;", "helper=foreign;return $THREAD_1_2;");
    assert!(!name_threads_with_labels(&mutated, true, Some(&reference))
        .unwrap()
        .source
        .contains("readViaHelper"));
}
#[test]
fn pruning_removes_only_whitespace_on_its_own_lines_and_refuses_uncertainty() {
    let source="function main(){\n\t function unused() {\n\t }\n\tfunction kept(){\"use strict\";}\n\tfunction dup(){} var dup;\n\tif(flag){function annex(){}}\n return [kept,typeof annex];\n}main();";
    let result = name_threads(source, true).unwrap();
    assert_eq!(result.report.removals.len(), 1);
    assert_eq!(
        result.source,
        source.replace("\t function unused() {\n\t }\n", "")
    );
    assert!(result.source.contains("function kept(){\"use strict\";}"));
    assert!(result.source.contains("function dup(){} var dup;"));
    assert!(result.source.contains("function annex(){}"));
}
#[test]
fn aliases_through_capture_initializers_keep_shared_origin_edges() {
    let original="function main(){let a=[],b=a;function $THREAD_1_2(x){a.push(x);return b;}return $THREAD_1_2;}main();";
    let reference = build_label_reference(
        original,
        &original.replace("$THREAD_1_2", "appendAndReturnAlias"),
    )
    .unwrap();
    let different = original
        .replace("let a=[],b=a", "let a=[],other=[],b=other")
        .replace("$THREAD_1_2", "$THREAD_9_8");
    assert!(
        !name_threads_with_labels(&different, true, Some(&reference))
            .unwrap()
            .source
            .contains("appendAndReturnAlias")
    );
}
#[test]
fn standalone_function_packages_name_raw_helpers_without_reference_or_key_renaming() {
    let source = r#"function main(){let $KEY_0={UINT14_BASE:1,UINT16:2,UINT24:3,UINT32:4,UINT64:5,NINT4_BASE:6,FLOAT32:7,DOUBLE64:8,STR5_BASE:9,STRREF:10,STRLUT:11,ARRAY8:12,BARRAY8:13,MAP_:14,BMAP_:15,EXTENSION:16};
 function $THREAD_7_77(value,items){if(value<1)items.push(value);else if(value<2)items.push($KEY_0.UINT16,value);else if(value<3)items.push($KEY_0.UINT24,value);else if(value<4)items.push($KEY_0.UINT32,value);else items.push($KEY_0.UINT64,value);}
 return $THREAD_7_77;}main();"#;
    let result = name_threads(source, true).unwrap();
    assert!(result
        .source
        .contains("function writeUnsignedIntBytes(value,items)"));
    assert!(result.source.contains("let $KEY_0={UINT14_BASE:1"));
    assert!(result.report.outcomes.iter().any(|o| o.applied
        && o.evaluator
            .as_deref()
            .is_some_and(|e| e.starts_with("function_finder::"))));
    assert!(!result
        .report
        .timings_ms
        .contains_key("finder_p13_graphics_canvas"));
    assert!(result
        .report
        .timings_ms
        .contains_key("skipped_p13_graphics_canvas"));
}
