//! Focused, portable coverage for the hidden nested-frame bridge carrier.
//!
//! The claimed family is the parent carrier that builds the hidden iframe/style
//! pair and schedules the already-named `startNestedFrameBridge` child. Both
//! children are direct `FunctionDeclaration` children owned by the carrier and
//! are named by their own evaluators; this rule only claims the parent. The
//! portable positives are the combined-guard, split-guard and assigned-locals
//! shapes seen in the recovered seeds. Negatives change operations, receivers,
//! the null guard, capture shadowing, the two children, a nested/indirect child
//! donor and a page receiver disconnected from the carrier's document getter.
//! Existing readable names and the two already-named children stay untouched.
use shape::naming::{name_threads, NamedThreads, ThreadName};

const EVALUATOR: &str = "thread_posthost_frame_carrier";
const PARENT: &str = "createHiddenFrameBridgeCarrier";
const BRIDGE: &str = "startNestedFrameBridge";
const CLEANUP: &str = "removeInjectedGlobalAndNotifyCleanup";

const POSITIVE: &str = r##"function bootstrap(){
let applier, createNode, appendNode, nodeMethod, frameDescriptor, styleDescriptor, documentDescriptor, bridgeDescriptor;
let timerFlag, taskQueue, notifyConsumer;
function $THREAD_12_400($nonceOne, $nonceTwo, $nonceThree){
let localNonce = $nonceOne;
if (createNode == null || appendNode == null || nodeMethod == null || frameDescriptor == null || frameDescriptor.get == null || frameDescriptor.set == null || styleDescriptor == null || styleDescriptor.set == null || documentDescriptor == null || documentDescriptor.get == null || bridgeDescriptor == null || bridgeDescriptor.get == null) {
return false;
} else {
let iframe = applier(createNode, document, "iframe");
applier(frameDescriptor.set, iframe, "x" + Math.random().toString(36).slice(2));
if ($nonceThree != null) {
iframe.nonce = $nonceThree;
}
let sheet = applier(createNode, document, "style");
applier(styleDescriptor.set, sheet, "#" + applier(frameDescriptor.get, iframe) + "{display:none !important;}");
if ($nonceTwo != null) {
sheet.nonce = $nonceTwo;
}
let page = applier(documentDescriptor.get, window.document);
let key;
function $THREAD_11_300($cleanupInput){
delete window[key];
try {
notifyConsumer($cleanupInput);
} catch (e) {}
applier(nodeMethod, page, iframe);
applier(nodeMethod, page, sheet);
}
key = "x" + Math.random().toString(36).slice(2);
let release = $THREAD_11_300;
function $THREAD_10_200(){
applier(appendNode, page, sheet);
applier(appendNode, page, iframe);
let content = applier(bridgeDescriptor.get, iframe);
window[key] = release;
content.location.href = "javascript:\"<iframe src=\\\"javascript:(window.parent.parent['" + key + "'](window),'')\\\"></iframe>\"";
}
if (typeof setTimeout !== "function") {
$THREAD_10_200();
} else if (timerFlag) {
setTimeout($THREAD_10_200, 0);
} else if (taskQueue) {
taskQueue.push($THREAD_10_200);
}
return true;
}
}
$THREAD_12_400(null, null, null);
}
bootstrap();"##;

// Actual split-guard shape: createElement/appendChild are hoisted into a
// boolean prelude and the locals are pre-declared and assigned in the body.
const SPLIT: &str = r##"function bootstrap(){
let applier, createNode, appendNode, nodeMethod, frameDescriptor, styleDescriptor, documentDescriptor, bridgeDescriptor;
let timerFlag, taskQueue, notifyConsumer;
function $THREAD_22_400($nonceOne, $nonceTwo, $nonceThree){
let localNonce = $nonceOne;
let splitGuard;
let iframe, sheet, page, key, release;
if (createNode == null) {
splitGuard = true;
} else {
splitGuard = appendNode == null;
}
if (splitGuard || nodeMethod == null || frameDescriptor == null || frameDescriptor.get == null || frameDescriptor.set == null || styleDescriptor == null || styleDescriptor.set == null || documentDescriptor == null || documentDescriptor.get == null || bridgeDescriptor == null || bridgeDescriptor.get == null) {
return false;
} else {
iframe = applier(createNode, document, "iframe");
applier(frameDescriptor.set, iframe, "x" + Math.random().toString(36).slice(2));
if ($nonceThree != null) {
iframe.nonce = $nonceThree;
}
sheet = applier(createNode, document, "style");
applier(styleDescriptor.set, sheet, "#" + applier(frameDescriptor.get, iframe) + "{display:none !important;}");
if ($nonceTwo != null) {
sheet.nonce = $nonceTwo;
}
page = applier(documentDescriptor.get, window.document);
function $THREAD_21_300($cleanupInput){
delete window[key];
try {
notifyConsumer($cleanupInput);
} catch (e) {}
applier(nodeMethod, page, iframe);
applier(nodeMethod, page, sheet);
}
key = "x" + Math.random().toString(36).slice(2);
release = $THREAD_21_300;
function $THREAD_20_200(){
applier(appendNode, page, sheet);
applier(appendNode, page, iframe);
let content = applier(bridgeDescriptor.get, iframe);
window[key] = release;
content.location.href = "javascript:\"<iframe src=\\\"javascript:(window.parent.parent['" + key + "'](window),'')\\\"></iframe>\"";
}
if (typeof setTimeout !== "function") {
$THREAD_20_200();
} else if (timerFlag) {
setTimeout($THREAD_20_200, 0);
} else if (taskQueue) {
taskQueue.push($THREAD_20_200);
}
return true;
}
}
$THREAD_22_400(null, null, null);
}
bootstrap();"##;

// Actual assigned-locals shape: the cleanup callback is declared before the
// bridge child and the random key / cleanup assignments are reordered.
const ASSIGNED: &str = r##"function bootstrap(){
let applier, createNode, appendNode, nodeMethod, frameDescriptor, styleDescriptor, documentDescriptor, bridgeDescriptor;
let timerFlag, taskQueue, notifyConsumer;
function $THREAD_32_400($nonceOne, $nonceTwo, $nonceThree){
let localNonce = $nonceOne;
let iframe, sheet, page, key, release;
if (createNode == null || appendNode == null || nodeMethod == null || frameDescriptor == null || frameDescriptor.get == null || frameDescriptor.set == null || styleDescriptor == null || styleDescriptor.set == null || documentDescriptor == null || documentDescriptor.get == null || bridgeDescriptor == null || bridgeDescriptor.get == null) {
return false;
} else {
iframe = applier(createNode, document, "iframe");
applier(frameDescriptor.set, iframe, "x" + Math.random().toString(36).slice(2));
if ($nonceThree != null) {
iframe.nonce = $nonceThree;
}
sheet = applier(createNode, document, "style");
applier(styleDescriptor.set, sheet, "#" + applier(frameDescriptor.get, iframe) + "{display:none !important;}");
if ($nonceTwo != null) {
sheet.nonce = $nonceTwo;
}
page = applier(documentDescriptor.get, window.document);
function $THREAD_31_300($cleanupInput){
delete window[key];
try {
notifyConsumer($cleanupInput);
} catch (e) {}
applier(nodeMethod, page, iframe);
applier(nodeMethod, page, sheet);
}
key = "x" + Math.random().toString(36).slice(2);
release = $THREAD_31_300;
function $THREAD_30_200(){
applier(appendNode, page, sheet);
applier(appendNode, page, iframe);
let content = applier(bridgeDescriptor.get, iframe);
window[key] = release;
content.location.href = "javascript:\"<iframe src=\\\"javascript:(window.parent.parent['" + key + "'](window),'')\\\"></iframe>\"";
}
if (typeof setTimeout !== "function") {
$THREAD_30_200();
} else if (timerFlag) {
setTimeout($THREAD_30_200, 0);
} else if (taskQueue) {
taskQueue.push($THREAD_30_200);
}
return true;
}
}
$THREAD_32_400(null, null, null);
}
bootstrap();"##;

fn target_outcome<'a>(result: &'a NamedThreads, target: &str) -> &'a ThreadName {
    result
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == target)
        .unwrap_or_else(|| panic!("target binding {target} missing from outcomes"))
}

fn our_claim(source: &str, target: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        target_outcome(&result, target).evaluator.as_deref() == Some(EVALUATOR)
    })
}

fn named(source: &str, target: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        let outcome = target_outcome(&result, target);
        outcome.applied && outcome.proposed_name.as_deref() == Some(name)
    })
}

fn parses(source: &str) -> bool {
    match name_threads(source, true) {
        Ok(_) => true,
        Err(error) => {
            eprintln!("name_threads error: {error}");
            false
        }
    }
}

fn denied(source: &str, target: &str) {
    assert!(parses(source), "fixture did not parse:\n{source}");
    assert!(!our_claim(source, target), "unexpected frame carrier for:\n{source}");
    assert!(!named(source, target, PARENT), "unexpected {PARENT} for:\n{source}");
}

fn refuses(template: &str, target: &str, changes: &[(&str, &str)]) {
    for &(from, to) in changes {
        assert!(template.contains(from), "missing replacement {from}");
        denied(&template.replace(from, to), target);
    }
}

fn claims(source: &str, target: &str, name: &str) {
    assert!(parses(source), "fixture did not parse:\n{source}");
    assert!(our_claim(source, target), "not our claim for {target}:\n{source}");
    assert!(named(source, target, name), "not named {name} for {target}:\n{source}");
}

fn named_only(source: &str, target: &str, name: &str) {
    assert!(parses(source), "fixture did not parse:\n{source}");
    assert!(named(source, target, name), "not named {name} for {target}:\n{source}");
}

#[test]
fn witness_parent_and_named_children_are_claimed() {
    claims(POSITIVE, "$THREAD_12_400", PARENT);
    named_only(POSITIVE, "$THREAD_10_200", BRIDGE);
    named_only(POSITIVE, "$THREAD_11_300", CLEANUP);
}

#[test]
fn portable_operation_shape_variants_are_claimed() {
    for (source, parent, bridge, cleanup) in [
        (SPLIT, "$THREAD_22_400", "$THREAD_20_200", "$THREAD_21_300"),
        (ASSIGNED, "$THREAD_32_400", "$THREAD_30_200", "$THREAD_31_300"),
    ] {
        claims(source, parent, PARENT);
        named_only(source, bridge, BRIDGE);
        named_only(source, cleanup, CLEANUP);
    }
}

#[test]
fn generated_and_capture_spellings_are_not_identity() {
    let renamed = POSITIVE
        .replace("$THREAD_12_400", "$THREAD_72_4400")
        .replace("$THREAD_11_300", "$THREAD_71_3300")
        .replace("$THREAD_10_200", "$THREAD_70_2200")
        .replace("applier", "$KEY_900")
        .replace("createNode", "$KEY_901")
        .replace("appendNode", "$KEY_902")
        .replace("nodeMethod", "$KEY_903")
        .replace("frameDescriptor", "$KEY_904")
        .replace("styleDescriptor", "$KEY_905")
        .replace("documentDescriptor", "$KEY_906")
        .replace("bridgeDescriptor", "$KEY_907")
        .replace("timerFlag", "$KEY_908")
        .replace("taskQueue", "$KEY_909")
        .replace("notifyConsumer", "$KEY_910")
        .replace("release", "$KEY_914")
        .replace("key", "$KEY_915");
    claims(&renamed, "$THREAD_72_4400", PARENT);
    named_only(&renamed, "$THREAD_70_2200", BRIDGE);
    named_only(&renamed, "$THREAD_71_3300", CLEANUP);
}

#[test]
fn operation_and_receiver_negatives_are_refused() {
    refuses(
        POSITIVE,
        "$THREAD_12_400",
        &[
            (
                "applier(createNode, document, \"iframe\")",
                "applier(createNode, document, \"div\")",
            ),
            (
                "applier(frameDescriptor.set, iframe,",
                "other(frameDescriptor.set, iframe,",
            ),
            (
                "applier(createNode, document, \"style\")",
                "other(createNode, document, \"style\")",
            ),
            (
                "applier(documentDescriptor.get, window.document)",
                "applier(documentDescriptor.get, window.body)",
            ),
            (
                "applier(documentDescriptor.get, window.document)",
                "applier(documentDescriptor.get, document.document)",
            ),
            (
                "\"#\" + applier(frameDescriptor.get, iframe) + \"{display:none !important;}\"",
                "\".\" + applier(frameDescriptor.get, iframe) + \"{display:none !important;}\"",
            ),
            (
                "\"#\" + applier(frameDescriptor.get, iframe) + \"{display:none !important;}\"",
                "\"#\" + applier(frameDescriptor.get, sheet) + \"{display:none !important;}\"",
            ),
            (
                "\"#\" + applier(frameDescriptor.get, iframe) + \"{display:none !important;}\"",
                "\"#\" + applier(styleDescriptor.get, iframe) + \"{display:none !important;}\"",
            ),
            (
                "\"#\" + applier(frameDescriptor.get, iframe) + \"{display:none !important;}\"",
                "\"#\" + applier(frameDescriptor.get, iframe) + \"{display:block !important;}\"",
            ),
            (
                "applier(styleDescriptor.set, sheet,",
                "applier(frameDescriptor.set, sheet,",
            ),
            (
                "applier(frameDescriptor.set, iframe, \"x\" + Math.random().toString(36).slice(2))",
                "applier(frameDescriptor.set, iframe, \"fixed\")",
            ),
            (
                "key = \"x\" + Math.random().toString(36).slice(2);",
                "key = \"fixed\";",
            ),
        ],
    );
}

#[test]
fn guard_and_shadow_negatives_are_refused() {
    refuses(
        POSITIVE,
        "$THREAD_12_400",
        &[
            (" || frameDescriptor.set == null", ""),
            (" || frameDescriptor.get == null", ""),
            (" || documentDescriptor.get == null", ""),
            (" || styleDescriptor.set == null", ""),
            (" || appendNode == null", ""),
            ("createNode == null", "createNode != null"),
            (
                "let applier, createNode,",
                "let document, applier, createNode,",
            ),
            (
                "let applier, createNode,",
                "let window, applier, createNode,",
            ),
            (
                "let applier, createNode,",
                "let setTimeout, applier, createNode,",
            ),
        ],
    );
}

#[test]
fn child_decoy_negatives_are_refused() {
    refuses(
        POSITIVE,
        "$THREAD_12_400",
        &[
            (
                "delete window[key];",
                "delete window[iframe];",
            ),
            (
                "window[key] = release;",
                "window[content] = release;",
            ),
            (
                "$THREAD_10_200();",
                "$THREAD_11_300();",
            ),
            (
                "taskQueue.push($THREAD_10_200);",
                "taskQueue.push($THREAD_11_300);",
            ),
            (
                "setTimeout($THREAD_10_200, 0);",
                "setTimeout($THREAD_11_300, 0);",
            ),
            (
                "if (typeof setTimeout !== \"function\") {",
                "if (typeof setTimeout === \"function\") {",
            ),
            (
                "return true;",
                "return false;",
            ),
        ],
    );
    // An extra nested declaration owned by the carrier is not a decoy slot.
    denied(
        &POSITIVE.replace(
            "return true;\n}\n}",
            "function $THREAD_13_500(){return 1;}\nreturn true;\n}\n}",
        ),
        "$THREAD_12_400",
    );
    // The random key written by the carrier must be the exact key the bridge
    // installs and the cleanup deletes. Moving the random write to a distinct
    // local keeps both child proofs but breaks the parent link.
    denied(
        &POSITIVE
            .replace("let key;", "let key, otherKey;")
            .replace(
                "key = \"x\" + Math.random().toString(36).slice(2);",
                "otherKey = \"x\" + Math.random().toString(36).slice(2);",
            ),
        "$THREAD_12_400",
    );
}

#[test]
fn nested_donor_and_wrong_document_receiver_are_refused() {
    // A cleanup-shaped function expression declared in the carrier body is not a
    // direct `FunctionDeclaration` child: its span lies inside the carrier, but
    // its parent site is the declarator, so it must not lend cleanup evidence to
    // the parent.
    let nested_cleanup = POSITIVE
        .replace(
            "let key;\nfunction $THREAD_11_300($cleanupInput){",
            "let key;\nlet $THREAD_11_300 = function($cleanupInput){",
        )
        .replace(
            "applier(nodeMethod, page, sheet);\n}\nkey =",
            "applier(nodeMethod, page, sheet);\n};\nkey =",
        )
        .replace("let release = $THREAD_11_300;\n", "")
        .replace("window[key] = release;", "window[key] = $THREAD_11_300;");
    denied(&nested_cleanup, "$THREAD_12_400");

    // The bridge appends through exactly the page the carrier read with its
    // captured document getter. A different captured page keeps the bridge
    // proof intact but disconnects the parent.
    let wrong_page = POSITIVE
        .replace(
            "let applier, createNode, appendNode, nodeMethod, frameDescriptor, styleDescriptor, documentDescriptor, bridgeDescriptor;",
            "let applier, createNode, appendNode, nodeMethod, frameDescriptor, styleDescriptor, documentDescriptor, bridgeDescriptor, wrongPage;",
        )
        .replace(
            "applier(appendNode, page, sheet);",
            "applier(appendNode, wrongPage, sheet);",
        )
        .replace(
            "applier(appendNode, page, iframe);",
            "applier(appendNode, wrongPage, iframe);",
        );
    denied(&wrong_page, "$THREAD_12_400");
}

#[test]
fn human_names_and_named_children_are_preserved() {
    let human = POSITIVE.replace("$THREAD_12_400", "humanFrameCarrier");
    let result = name_threads(&human, true).unwrap();
    assert!(result.source.contains("function humanFrameCarrier("));
    assert!(!result
        .report
        .outcomes
        .iter()
        .any(|outcome| outcome.binding.name == "humanFrameCarrier" && outcome.applied));
    assert!(named(&human, "$THREAD_10_200", BRIDGE));
    assert!(named(&human, "$THREAD_11_300", CLEANUP));

    let collision = POSITIVE.replace(
        "function bootstrap(){",
        "function bootstrap(){let createHiddenFrameBridgeCarrier=17;useValue(createHiddenFrameBridgeCarrier);",
    );
    assert!(parses(&collision));
    let result = name_threads(&collision, true).unwrap();
    let parent = target_outcome(&result, "$THREAD_12_400");
    assert!(!parent.applied);
    assert_eq!(parent.reason.as_deref(), Some("NameCollision"));
    assert!(result.source.contains("let createHiddenFrameBridgeCarrier=17"));
    assert!(result.source.contains("$THREAD_12_400"));
}

#[test]
fn edits_are_exact_identifier_only_replacements() {
    let result = name_threads(POSITIVE, true).unwrap();
    let applied: Vec<_> = result
        .report
        .outcomes
        .iter()
        .filter(|outcome| {
            outcome.applied
                && matches!(
                    outcome.proposed_name.as_deref(),
                    Some(PARENT) | Some(BRIDGE) | Some(CLEANUP)
                )
        })
        .collect();
    assert_eq!(applied.len(), 3, "expected the parent and both children");
    // The report carries original-coordinate edits in plan order, not source
    // order; sort descending by byte offset and replay exactly.
    let mut replay = POSITIVE.to_string();
    let mut edits = result.report.edits.clone();
    edits.sort_by(|a, b| b.start.cmp(&a.start));
    for edit in edits {
        let original = &POSITIVE[edit.start as usize..edit.end as usize];
        assert!(
            original.starts_with("$THREAD_"),
            "edit targets a non-identifier: {original:?}"
        );
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
}
