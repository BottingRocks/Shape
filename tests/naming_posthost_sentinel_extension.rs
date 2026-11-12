use shape::naming::name_threads;

// Focused post-host family: the sentinel extension whose serialiser is a plain
// member assignment after the extension object's literal. The registry and the
// encoding tag table below are the smallest shape that reaches the serializer
// package's extension proof; the extension members are the test's subject.

const HEADER: &str = r#"function host(){
let capturedSentinel = Object.create(null);
let encodingTags = {
    UINT14_BASE: 1,
    UINT16: 2,
    UINT24: 3,
    UINT32: 4,
    UINT64: 5,
    NINT4_BASE: 6,
    FLOAT32: 7,
    DOUBLE64: 8,
    STR5_BASE: 9,
    STRREF: 10,
    STRLUT: 11,
    ARRAY8: 12,
    BARRAY8: 13,
    MAP_: 14,
    BMAP_: 15,
    EXTENSION: 16
};
function createExtensionRegistryClass() {
    let extensionRegistry;
    function ExtensionRegistry() {
        "use strict";
        if (this instanceof extensionRegistry) {
            this.extensions = Object.create(null);
            return void 0;
        } else {
            throw new TypeError("Cannot call a class as a function");
        }
    }
    extensionRegistry = ExtensionRegistry;
    function registerExtension(key, detector, serialiser, deserialiser) {
        "use strict";
        this.extensions[key] = {
            detector: detector,
            serialiser: serialiser,
            deserialiser: deserialiser
        };
    }
    return extensionRegistry;
}
let registry = createExtensionRegistryClass();
"#;

const FOOTER: &str = r#"
}
host();"#;

fn program(body: &str) -> String {
    format!("{HEADER}{body}{FOOTER}")
}

fn named(source: &str, target: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result.report.outcomes.iter().any(|outcome| {
            outcome.applied
                && outcome.binding.name == target
                && outcome.proposed_name.as_deref() == Some(name)
        })
    })
}

/// A negative source must still parse and analyze; only the naming proof is
/// expected to fail. This keeps refusals from passing vacuously.
fn refused(source: &str, target: &str, name: &str) {
    let result = name_threads(source, true)
        .unwrap_or_else(|error| panic!("negative source must parse and analyze: {error}\n{source}"));
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == target)
        .unwrap_or_else(|| panic!("target binding {target} missing from outcomes:\n{source}"));
    assert!(
        !(outcome.applied && outcome.proposed_name.as_deref() == Some(name)),
        "accepted {target} as {name}:\n{source}"
    );
}

// The literal contains the detector; empty serializer and sentinel deserializer are
// assigned later on the same holder.
const POSTLITERAL_HOLDER: &str = r#"
function $THREAD_10_1($value) {
    return $value === capturedSentinel;
}
let $holder = { detector: $THREAD_10_1 };
function $THREAD_10_2() {
}
$holder.serialiser = $THREAD_10_2;
function $THREAD_10_3() {
    return capturedSentinel;
}
$holder.deserialiser = $THREAD_10_3;
"#;

// The literal-equivalent extension already worked; its three sentinel names
// must stay exactly as they are.
const LITERAL_HOLDER: &str = r#"
function $THREAD_11_1($value) {
    return $value === capturedSentinel;
}
function $THREAD_11_2() {
}
function $THREAD_11_3() {
    return capturedSentinel;
}
let $holder = {
    detector: $THREAD_11_1,
    serialiser: $THREAD_11_2,
    deserialiser: $THREAD_11_3
};
"#;

// Every member starts as a transpiled void 0 placeholder and is filled in later on the
// holder.
const PLACEHOLDER_HOLDER: &str = r#"
let $holder = {
    detector: void 0,
    serialiser: void 0,
    deserialiser: void 0
};
function $THREAD_12_1($value) {
    return $value === capturedSentinel;
}
$holder.detector = $THREAD_12_1;
function $THREAD_12_2() {
}
$holder.serialiser = $THREAD_12_2;
function $THREAD_12_3() {
    return capturedSentinel;
}
$holder.deserialiser = $THREAD_12_3;
"#;

#[test]
fn postliteral_sentinel_members_are_named() {
    let source = program(POSTLITERAL_HOLDER);
    assert!(
        named(&source, "$THREAD_10_1", "detectSentinelValue"),
        "did not name the postliteral detector"
    );
    assert!(
        named(&source, "$THREAD_10_2", "serialiseSentinelAsUndefined"),
        "did not name the postliteral serialiser"
    );
    // The deserialiser keeps the independently proven captured-state name.
    refused(&source, "$THREAD_10_3", "deserialiseSentinelValue");
}

#[test]
fn literal_sentinel_members_keep_all_three_names() {
    let source = program(LITERAL_HOLDER);
    for (target, name) in [
        ("$THREAD_11_1", "detectSentinelValue"),
        ("$THREAD_11_2", "serialiseSentinelAsUndefined"),
        ("$THREAD_11_3", "deserialiseSentinelValue"),
    ] {
        assert!(named(&source, target, name), "literal extension lost {name}");
    }
}

#[test]
fn placeholder_literal_members_are_filled_by_later_writes() {
    let source = program(PLACEHOLDER_HOLDER);
    assert!(
        named(&source, "$THREAD_12_1", "detectSentinelValue"),
        "did not name the placeholder detector"
    );
    assert!(
        named(&source, "$THREAD_12_2", "serialiseSentinelAsUndefined"),
        "did not name the placeholder serialiser"
    );
}

#[test]
fn agreeing_literal_and_postliteral_writes_stay_accepted() {
    let body = r#"
function $THREAD_13_1($value) {
    return $value === capturedSentinel;
}
function $THREAD_13_2() {
}
function $THREAD_13_3() {
    return capturedSentinel;
}
let $holder = {
    detector: $THREAD_13_1,
    serialiser: $THREAD_13_2
};
$holder.serialiser = $THREAD_13_2;
$holder.deserialiser = $THREAD_13_3;
"#;
    let source = program(body);
    assert!(named(&source, "$THREAD_13_1", "detectSentinelValue"));
    assert!(named(&source, "$THREAD_13_2", "serialiseSentinelAsUndefined"));
    assert!(named(&source, "$THREAD_13_3", "deserialiseSentinelValue"));
}

#[test]
fn spelling_variant_of_the_postliteral_holder_is_named() {
    let source = program(POSTLITERAL_HOLDER)
        .replace("$THREAD_10_1", "$THREAD_40_41")
        .replace("$THREAD_10_2", "$THREAD_40_42")
        .replace("$THREAD_10_3", "$THREAD_40_43")
        .replace("$holder", "$KEY_holder")
        .replace("capturedSentinel", "$KEY_sentinel")
        .replace("registry", "$KEY_registry");
    assert!(named(&source, "$THREAD_40_41", "detectSentinelValue"));
    assert!(named(&source, "$THREAD_40_42", "serialiseSentinelAsUndefined"));
}

#[test]
fn wrong_receiver_does_not_donate_the_serialiser() {
    let source = program(
        &POSTLITERAL_HOLDER.replace(
            "$holder.serialiser = $THREAD_10_2;",
            "let $other = {};\n$other.serialiser = $THREAD_10_2;",
        ),
    );
    refused(&source, "$THREAD_10_1", "detectSentinelValue");
    refused(&source, "$THREAD_10_2", "serialiseSentinelAsUndefined");
}

#[test]
fn ambiguous_reassignment_does_not_donate_the_serialiser() {
    let source = program(
        &POSTLITERAL_HOLDER.replace(
            "$holder.serialiser = $THREAD_10_2;",
            "function $THREAD_10_9() {\n}\n$holder.serialiser = $THREAD_10_2;\n$holder.serialiser = $THREAD_10_9;",
        ),
    );
    refused(&source, "$THREAD_10_1", "detectSentinelValue");
    refused(&source, "$THREAD_10_2", "serialiseSentinelAsUndefined");
}

#[test]
fn nested_donor_does_not_donate_the_serialiser() {
    let source = program(
        &POSTLITERAL_HOLDER.replace(
            "function $THREAD_10_2() {\n}\n$holder.serialiser = $THREAD_10_2;",
            "function $THREAD_10_9() {\n    function $THREAD_10_8() {\n    }\n    $holder.serialiser = $THREAD_10_8;\n}\n$THREAD_10_9();",
        ),
    );
    refused(&source, "$THREAD_10_1", "detectSentinelValue");
    refused(&source, "$THREAD_10_8", "serialiseSentinelAsUndefined");
}

#[test]
fn wrong_detector_sentinel_is_refused() {
    let source = program(&POSTLITERAL_HOLDER.replace(
        "return $value === capturedSentinel;",
        "return $value === $otherSentinel;",
    ))
    .replace(
        "let capturedSentinel = Object.create(null);",
        "let capturedSentinel = Object.create(null);\nlet $otherSentinel = Object.create(null);",
    );
    refused(&source, "$THREAD_10_1", "detectSentinelValue");
    refused(&source, "$THREAD_10_2", "serialiseSentinelAsUndefined");
}

#[test]
fn effectful_serialiser_is_refused() {
    let source = program(
        &POSTLITERAL_HOLDER.replace(
            "function $THREAD_10_2() {\n}",
            "function $THREAD_10_2() {\n    capturedSentinel.marker = true;\n}",
        ),
    );
    refused(&source, "$THREAD_10_2", "serialiseSentinelAsUndefined");
}
