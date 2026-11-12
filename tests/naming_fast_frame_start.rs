use shape::naming::name_threads;

// The owner witness shape, reduced to an inert source with the same scoped
// identities. The nested `javascript:` URL is never executed; it is only text.
const POSITIVE: &str = r#"function bootstrap(){
let applier, append, head, iframe, style, key, callback, descriptor;
function $THREAD_71_8123(){
applier(append, head, style);
applier(append, head, iframe);
let frame = applier(descriptor.get, iframe);
window[key] = callback;
frame.location.href = "javascript:\"<iframe src=\\\"javascript:(window.parent.parent['" + key + "'](window),'')\\\"></iframe>\"";
}
key = "k" + Math.random().toString(36).slice(2);
callback = function releaseBridge(){ delete window[key]; };
return $THREAD_71_8123;
}
bootstrap();"#;

// Alpha renaming: generated thread identity and every scoped capture/formal
// spelling changes. Nothing about the rule may depend on the seed spellings.
const ALPHA: &str = r#"function bootstrap(){
let $KEY_90, $KEY_91, $KEY_92, $KEY_93, $KEY_94, $KEY_95, $KEY_96, $KEY_97;
function $THREAD_900_777_2(){
$KEY_90($KEY_91, $KEY_92, $KEY_94);
$KEY_90($KEY_91, $KEY_92, $KEY_93);
let $KEY_98 = $KEY_90($KEY_95.get, $KEY_93);
window[$KEY_96] = $KEY_97;
$KEY_98.location.href = "javascript:\"<iframe src=\\\"javascript:(window.parent.parent['" + $KEY_96 + "'](window),'')\\\"></iframe>\"";
}
$KEY_96 = "k" + Math.random().toString(36).slice(2);
$KEY_97 = function $KEY_99(){ delete window[$KEY_96]; };
return $THREAD_900_777_2;
}
bootstrap();"#;

// Readable spellings prove the rule is not matching $KEY_/$THREAD_ lexemes.
const HUMAN: &str = r#"function bootstrap(){
let applyProvided, appendChild, documentHead, frameElement, styleElement, bridgeKey, bridgeCallback, descriptor;
function $THREAD_71_8123(){
applyProvided(appendChild, documentHead, styleElement);
applyProvided(appendChild, documentHead, frameElement);
let contentWindow = applyProvided(descriptor.get, frameElement);
window[bridgeKey] = bridgeCallback;
contentWindow.location.href = "javascript:\"<iframe src=\\\"javascript:(window.parent.parent['" + bridgeKey + "'](window),'')\\\"></iframe>\"";
}
bridgeKey = "k" + Math.random().toString(36).slice(2);
bridgeCallback = function releaseBridge(){ delete window[bridgeKey]; };
return $THREAD_71_8123;
}
bootstrap();"#;

// The provided applier is consumer input. Binding it to the recovered native
// call protocol must not change the rule and must not be claimed as identity.
const NATIVE_PROTOCOL: &str = r#"function bootstrap(){
let applier, append, head, iframe, style, key, callback, descriptor;
applier = Object.call.bind(Object.bind, Object.call)(Object.call);
function $THREAD_71_8123(){
applier(append, head, style);
applier(append, head, iframe);
let frame = applier(descriptor.get, iframe);
window[key] = callback;
frame.location.href = "javascript:\"<iframe src=\\\"javascript:(window.parent.parent['" + key + "'](window),'')\\\"></iframe>\"";
}
key = "k" + Math.random().toString(36).slice(2);
callback = function releaseBridge(){ delete window[key]; };
return $THREAD_71_8123;
}
bootstrap();"#;

fn named(source: &str, target: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report.outcomes.iter().any(|o| {
            o.applied && o.binding.name == target && o.proposed_name.as_deref() == Some(name)
        })
    })
}

/// A negative source must still parse and analyze; only the naming proof is
/// expected to fail. This keeps refusal tests from passing vacuously.
fn refused(source: &str, target: &str, name: &str) {
    let result = name_threads(source, true)
        .unwrap_or_else(|error| panic!("negative source must parse and analyze: {error}\n{source}"));
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == target)
        .unwrap_or_else(|| panic!("target binding {target} missing from outcomes:\n{source}"));
    assert!(
        !(outcome.applied && outcome.proposed_name.as_deref() == Some(name)),
        "accepted {target} as {name}:\n{source}"
    );
}

fn refuses(template: &str, target: &str, name: &str, changes: &[(&str, &str)]) {
    for &(from, to) in changes {
        assert!(template.contains(from), "missing replacement {from}");
        refused(&template.replace(from, to), target, name);
    }
}

#[test]
fn nested_frame_bridge_whole_body_is_named() {
    for source in [POSITIVE, ALPHA, HUMAN, NATIVE_PROTOCOL] {
        assert!(
            named(source, "$THREAD_71_8123", "startNestedFrameBridge")
                || named(source, "$THREAD_900_777_2", "startNestedFrameBridge"),
            "did not name nested frame bridge fixture:\n{source}"
        );
    }
}

#[test]
fn nested_frame_bridge_requires_both_applier_calls_and_distinct_values() {
    refuses(
        POSITIVE,
        "$THREAD_71_8123",
        "startNestedFrameBridge",
        &[
            (
                "applier(append, head, style);\napplier(append, head, iframe);",
                "applier(append, head, style);",
            ),
            (
                "applier(append, head, style);",
                "applier(append, head, style, extra);",
            ),
            (
                "applier(append, head, style);",
                "other(append, head, style);",
            ),
            (
                "applier(append, head, style);",
                "applier(other, head, style);",
            ),
            (
                "applier(append, head, style);",
                "applier(append, other, style);",
            ),
            (
                "applier(append, head, iframe);",
                "applier(append, head, style);",
            ),
        ],
    );
}

#[test]
fn nested_frame_bridge_requires_descriptor_get_and_captured_frame() {
    refuses(
        POSITIVE,
        "$THREAD_71_8123",
        "startNestedFrameBridge",
        &[
            (
                "applier(descriptor.get, iframe)",
                "applier(descriptor.set, iframe)",
            ),
            (
                "applier(descriptor.get, iframe)",
                "applier(descriptor.value, iframe)",
            ),
            (
                "applier(descriptor.get, iframe)",
                "applier(descriptor.get, style)",
            ),
            (
                "applier(descriptor.get, iframe)",
                "applier(descriptor, iframe)",
            ),
            (
                "applier(descriptor.get, iframe)",
                "applier(descriptor.get, iframe, extra)",
            ),
        ],
    );
}

#[test]
fn nested_frame_bridge_requires_global_window_key_and_function_callback() {
    refuses(
        POSITIVE,
        "$THREAD_71_8123",
        "startNestedFrameBridge",
        &[
            ("window[key] = callback;", "window.key = callback;"),
            ("window[key] = callback;", "window[style] = callback;"),
            ("window[key] = callback;", "window[key] = descriptor;"),
            ("window[key] = callback;", "window[key] = key;"),
            (
                "let applier, append, head, iframe, style, key, callback, descriptor;",
                "let applier, append, head, iframe, style, key, callback, descriptor, window;",
            ),
        ],
    );
}

#[test]
fn nested_frame_bridge_requires_frame_location_and_semantic_js_url() {
    refuses(
        POSITIVE,
        "$THREAD_71_8123",
        "startNestedFrameBridge",
        &[
            ("frame.location.href", "other.location.href"),
            ("frame.location.href", "frame.location.src"),
            ("frame.location.href", "frame.href"),
            (
                "frame.location.href = \"javascript:",
                "frame.location.href = \"https:",
            ),
            (
                "window.parent.parent['",
                "window.parent['",
            ),
            ("](window)", "](other)"),
            ("</iframe>", "</div>"),
            (
                "\" + key + \"",
                "\" + \"fixed\" + \"",
            ),
        ],
    );
}

#[test]
fn nested_frame_bridge_requires_exact_statement_order_and_arity() {
    refuses(
        POSITIVE,
        "$THREAD_71_8123",
        "startNestedFrameBridge",
        &[
            (
                "function $THREAD_71_8123(){",
                "function $THREAD_71_8123(extra){",
            ),
            (
                "function $THREAD_71_8123(){",
                "function $THREAD_71_8123(){ let local = applier;",
            ),
            (
                "window[key] = callback;\nframe.location.href",
                "window[key] = callback; return;\nframe.location.href",
            ),
        ],
    );
}

#[test]
fn nested_frame_bridge_does_not_rename_readable_bindings() {
    let human = HUMAN.replace("$THREAD_71_8123", "startBridge");
    assert_eq!(name_threads(&human, true).unwrap().source, human);
}
