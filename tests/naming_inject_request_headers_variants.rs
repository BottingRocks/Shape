//! Role-local coverage for the guarded branch-assigned send carrier.
//! Full prepared inputs preserve bootstrap, factory, captures and consumers.
//! The capture transport proof preserves two separate instrumented reads.
use shape::naming::name_threads;

const INLINE: &str = include_str!("fixtures/inject_request_headers_variants/branch-assigned-inline-carrier.js");
const HELPER: &str = include_str!("fixtures/inject_request_headers_variants/single-assignment-helper-carrier.js");
const NAME: &str = "injectRequestHeadersBeforeSend";
const EVALUATOR: &str = "function_finder::inject_request_headers_before_send";

fn recognizes(source: &str, target: &str) -> bool {
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::unambiguous()).parse();
    assert!(
        !parsed.panicked && parsed.errors.is_empty(),
        "fixture must parse: {:?}",
        parsed.errors
    );
    // A parse error or invalid Model must never count as semantic refusal.
    let result = name_threads(source, true).expect("full fixture must build a valid Model and analyze");
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == target)
        .expect("original candidate must remain present");
    outcome.applied
        && outcome.proposed_name.as_deref() == Some(NAME)
        && outcome.evaluator.as_deref() == Some(EVALUATOR)
}

#[test]
fn branch_assigned_inline_carrier_has_proven_send_transport() {
    assert!(recognizes(INLINE, "$THREAD_9_31784"));
}

#[test]
fn single_assignment_helper_counterpart_preserves_existing_name() {
    assert!(recognizes(HELPER, "$THREAD_8_134961"));
}

// Mutate only the actual XHR factory, preserving every bootstrap/carrier and
// the complete owned callback. Each replacement must exist exactly once.
fn changed_factory(from: &str, to: &str) -> String {
    let begin = INLINE.find("function $THREAD_8_70141($BN)").expect("factory");
    let end = begin + INLINE[begin..].find("let $KEY_203 =").expect("factory end");
    let factory = &INLINE[begin..end];
    assert_eq!(factory.matches(from).count(), 1, "ambiguous mutation: {from}");
    let mut result = INLINE.to_owned();
    result.replace_range(begin..end, &factory.replacen(from, to, 1));
    result
}

#[test]
fn refuses_invalid_split_origins_guards_writes_and_phase() {
    for (from, to) in [
        ("if ($BN.instrumented.xhrSend != null)", "if ($BN.instrumented.xhrSend == null)"),
        ("if ($BN.instrumented.xhrSend != null)", "if ($BN.instrumented.xhrSend !== null)"),
        ("$KEY_10 = $BN.instrumented.xhrSend;", "$KEY_10 = $BN.instrumented.xhrOpen;"),
        ("$KEY_10 = $KEY_58($KEY_0, \"send\");", "$KEY_10 = $KEY_58($KEY_0, \"open\");"),
        ("$KEY_10 = $KEY_58($KEY_0, \"send\");", "$KEY_10 = $KEY_58($BN.global, \"send\");"),
        ("$KEY_10 = $KEY_58($KEY_0, \"send\");", "$KEY_10 = $KEY_58({}, \"send\");"),
        ("$KEY_10 = $KEY_58($KEY_0, \"send\");", "$KEY_10 = $KEY_58($KEY_0, \"send\"); return void 0;"),
        ("$KEY_10 = $BN.instrumented.xhrSend;", "void 0;"),
        ("$KEY_10 = $KEY_58($KEY_0, \"send\");", "void 0;"),
        ("let $BS;", "let $BS; $KEY_10 = $KEY_2;"),
        ("$KEY_10.onBeforeInvoke.register($THREAD_9_31784);", "if (false) { $KEY_10.onBeforeInvoke.register($THREAD_9_31784); }"),
        ("$KEY_10.onBeforeInvoke.register($THREAD_9_31784);", "$KEY_10.onBeforeSet.register($THREAD_9_31784);"),
        ("$KEY_0 = $KEY_5.prototype;", "$KEY_0 = $BN.global.other.prototype;"),
        ("$KEY_5 = $BN.global.XMLHttpRequest;", "$KEY_5 = $BN.global.OtherRequest;"),
        ("let $BS;", "let $BS; consume($KEY_10);"),
    ] {
        let source = changed_factory(from, to);
        assert!(!recognizes(&source, "$THREAD_9_31784"), "accepted {from} -> {to}");
    }
}

#[test]
fn split_capture_is_independent_of_generated_binding_names() {
    let begin = INLINE.find("function $THREAD_8_70141($BN)").expect("factory");
    let end = begin + INLINE[begin..].find("let $KEY_203 =").expect("factory end");
    let renamed = INLINE[begin..end]
        .replace("$KEY_10", "$SEND_BUNDLE")
        .replace("$THREAD_9_31784", "$THREAD_932_81473");
    let mut source = INLINE.to_owned();
    source.replace_range(begin..end, &renamed);
    assert!(recognizes(&source, "$THREAD_932_81473"));
}

#[test]
fn refuses_changed_owned_header_receiver_and_result_return() {
    for (from, to) in [
        ("$KEY_11.call($BX.thisObj, k, $CM[k]);", "$KEY_11.call($BX, k, $CM[k]);"),
        ("$KEY_11.call($BX.thisObj, k, $CM[k]);", "$KEY_11.apply($BX.thisObj, k, $CM[k]);"),
    ] {
        let source = changed_factory(from, to);
        assert!(!recognizes(&source, "$THREAD_9_31784"), "accepted {from} -> {to}");
    }
    let begin = INLINE.find("function $THREAD_9_31784($BX, $BY)").expect("callback");
    let end = begin + INLINE[begin..].find("$KEY_14 = $THREAD_9_31784;").expect("callback end");
    let mut source = INLINE.to_owned();
    source.replace_range(begin..end, &INLINE[begin..end].replace("return $BY;", "return void 0;"));
    assert!(!recognizes(&source, "$THREAD_9_31784"));
}

// Full-input counterexample changes only the callback body. Factory and captured bindings
// remain intact.
const REVIEWED_DEAD_BODY: &str =
    include_str!("fixtures/inject_request_headers_variants/dead-inline-carrier-callback.js");

#[test]
fn refuses_reviewed_dead_callback_with_valid_model() {
    assert!(!recognizes(REVIEWED_DEAD_BODY, "$THREAD_9_31784"));
}

#[test]
fn refuses_dormant_or_reordered_owned_emission() {
    for (from, to) in [
        (
            "if ($CW !== null && $CW !== void 0)",
            "if (true)",
        ),
        (
            "$KEY_11.call($BX.thisObj, k, $CM[k]);",
            "if (false) { $KEY_11.call($BX.thisObj, k, $CM[k]); }",
        ),
        (
            "if ($CX !== null && $CX !== void 0)",
            "return $BY; if ($CX !== null && $CX !== void 0)",
        ),
        (
            "if ($CA[0] != null && $KEY_106($CA[1]))",
            "if (false && $CA[0] != null && $KEY_106($CA[1]))",
        ),
        (
            "if ($KEY_7 && $KEY_77.includes($KEY_3.toUpperCase()) && $BX.args.length > 0 && $BX.args[0] != null)",
            "if (false && $KEY_77.includes($KEY_3.toUpperCase()) && $BX.args.length > 0 && $BX.args[0] != null)",
        ),
        (
            "$KEY_11.call($BX.thisObj, k_2, $CX[k_2]);",
            "$KEY_11.call($BX.thisObj, k_2, $CM[k_2]);",
        ),
    ] {
        let source = changed_factory(from, to);
        assert!(!recognizes(&source, "$THREAD_9_31784"), "accepted {from} -> {to}");
    }
}

#[test]
fn refuses_arbitrary_native_capture_helpers_and_escaping_aliases() {
    // Hoisted named decoy retains the exact one-write branch topology and
    // resolves as a stable callable. Its body must fail the native proof.
    let source = changed_factory(
        "$KEY_10 = $KEY_58($KEY_0, \"send\");",
        "$KEY_10 = $HEADER_NATIVE_DECOY($KEY_0, \"send\");",
    );
    let source = source.replacen(
        "function $THREAD_8_70141($BN) {",
        "function $THREAD_8_70141($BN) { function $HEADER_NATIVE_DECOY(object,key,optional) { return {}; }",
        1,
    );
    assert!(!recognizes(&source, "$THREAD_9_31784"));

    for (from, to) in [
        (
            "$KEY_10 = $KEY_58($KEY_0, \"send\");",
            "$KEY_10 = (function(object, name) { return {}; })($KEY_0, \"send\");",
        ),
        ("let $BS;", "let $BS; consume($KEY_58);"),
        ("let $BS;", "let $BS; $KEY_58 = function() { return {}; };"),
        (
            "$KEY_95($KEY_0, \"setRequestHeader\")",
            "(function(object, name) { return {containingObj:object, desc:{value:function(){}}}; })($KEY_0, \"setRequestHeader\")",
        ),
        (
            "$KEY_95($KEY_0, \"setRequestHeader\")",
            "$KEY_95($KEY_0, \"send\")",
        ),
        (
            "$KEY_95($KEY_0, \"setRequestHeader\")",
            "$KEY_95($BN.global, \"setRequestHeader\")",
        ),
    ] {
        let source = changed_factory(from, to);
        assert!(!recognizes(&source, "$THREAD_9_31784"), "accepted {from} -> {to}");
    }
}

#[test]
fn refuses_wrong_header_original_and_alias_protocol() {
    for (from, to) in [
        ("$KEY_11 = $BN.instrumented.setRequestHeader?.originals?.value;", "$KEY_11 = $BN.instrumented.xhrSend?.originals?.value;"),
        ("$KEY_16 = $KEY_16.original;", "$KEY_16 = $KEY_16.result;"),
        ("$KEY_16 = $KEY_3.desc.value;", "$KEY_16 = $KEY_3.desc.get;"),
        ("$KEY_11 = $KEY_16;", "$KEY_16 = function() {}; $KEY_11 = $KEY_16;"),
        ("$KEY_11 = $KEY_16;", "consume($KEY_16); $KEY_11 = $KEY_16;"),
        ("let $BS;", "let $BS; consume($KEY_11);"),
        (
            "$KEY_143($KEY_3.containingObj, \"setRequestHeader\")",
            "(function(object, name){return {original:function(){}};})($KEY_3.containingObj, \"setRequestHeader\")",
        ),
    ] {
        let source = changed_factory(from, to);
        assert!(!recognizes(&source, "$THREAD_9_31784"), "accepted {from} -> {to}");
    }
}

#[test]
fn refuses_wrong_options_chunk_and_writer_dataflow() {
    for (from, to) in [
        ("let $CQ = $KEY_268(void 0);", "let $CQ = (function(){ return {a:'x'}; })(void 0);"),
        ("let $CW = $KEY_268(void 0);", "let $CW = (function(){ return {a:'x'}; })(void 0);"),
        (
            "$KEY_353(\"EWcll5PZpa-\" + i, $CQ[i], 7900)",
            "(function(name,value,size){return {}; })(\"EWcll5PZpa-\" + i, $CQ[i], 7900)",
        ),
        (
            "$KEY_353(\"EWcll5PZpa-\" + i, $CQ[i], 7900)",
            "$KEY_353(\"EWcll5PZpa-\" + i, $CM[i], 7900)",
        ),
        ("$CM[j] = $CS[j];", "$CM[j] = $CQ[j];"),
        ("$CX[$CZ] = $DA[$CZ];", "$CX[$CZ] = $CW[$CZ];"),
        ("$CZ = j_2;", "$CZ = i_2;"),
        ("let $CX = Object.create(null);", "let $CX = {};"),
    ] {
        let source = changed_factory(from, to);
        assert!(!recognizes(&source, "$THREAD_9_31784"), "accepted {from} -> {to}");
    }
}

#[test]
fn refuses_dormant_options_producer_and_wrong_chunk_progress() {
    // Keep every donor node, but insert an unconditional return before its
    // execution. Full-source parsing and valid Model remain mandatory.
    let source = INLINE.replacen(
        "function $THREAD_3_127658($BN) {",
        "function $THREAD_3_127658($BN) { return {};",
        1,
    );
    assert_ne!(source, INLINE);
    assert!(!recognizes(&source, "$THREAD_9_31784"));
    let begin = INLINE.find("function $THREAD_0_133053($BN, $BO, $BP)").expect("chunk helper");
    let end = begin + INLINE[begin..].find("let $KEY_353 =").expect("chunk alias");
    let helper = &INLINE[begin..end];
    assert_eq!(helper.matches("$BS = $BS + $BP;").count(), 1);
    let mut source = INLINE.to_owned();
    source.replace_range(begin..end, &helper.replace("$BS = $BS + $BP;", "$BS = $BS - $BP;"));
    assert_ne!(source, INLINE);
    assert!(!recognizes(&source, "$THREAD_9_31784"));
}

#[test]
fn published_chunker_identity_does_not_depend_on_export_key_names() {
    // This API publication is a copied function value. No export-key name
    // supplies the source identity of the separately proved direct chunk call.
    let source = INLINE
        .replacen("c: $KEY_353,", "compactChunkAlias: $KEY_353,", 1)
        .replacen("chunk: $KEY_353,", "verboseChunkAlias: $KEY_353,", 1);
    assert_ne!(source, INLINE);
    assert!(recognizes(&source, "$THREAD_9_31784"));
}

#[test]
fn refuses_forged_mutated_or_foreign_chunker_publications() {
    // Full source retains bootstrap, callback, captured helpers, and consumers. Every negative
    // must parse and build a valid Model before recognizes can test refusal.
    for (from, to) in [
        ("chunk: $KEY_353,", "chunk: $KEY_103,"),
        ("chunk: $KEY_353,", "chunk: $KEY_353, chunk: $KEY_103,"),
        ("chunk: $KEY_353,", "chunk: ($KEY_353 = function(name,value,size){return {}; }),"),
        ("chunk: $KEY_353,", "get chunk(){return $KEY_353;},"),
        ("chunk: $KEY_353,", "[\"chunk\"]: $KEY_353,"),
        ("chunk: $KEY_353,", "chunk: $KEY_353, extraAlias: $KEY_353,"),
        // Shadowing at the exporter creates a foreign binding, despite the
        // same textual spelling. It cannot donate origin for the direct call.
        ("function $THREAD_8_40362() {", "function $THREAD_8_40362() { let $KEY_353 = function(name,value,size){return {}; };"),
    ] {
        assert_eq!(INLINE.matches(from).count(), 1, "unique actual export mutation: {from}");
        let source = INLINE.replacen(from, to, 1);
        assert!(!recognizes(&source, "$THREAD_9_31784"), "accepted {from} -> {to}");
    }
    for (from, to) in [
        ("let $CQ = $KEY_268(void 0);", "let $CQ = $KEY_268(void 0); $KEY_353 = function(name,value,size){return {}; };"),
        ("let $CQ = $KEY_268(void 0);", "let $CQ = $KEY_268(void 0); $KEY_353.call = function(){};"),
        (
            "$KEY_353(\"EWcll5PZpa-\" + i, $CQ[i], 7900)",
            "localChunk(\"EWcll5PZpa-\" + i, $CQ[i], 7900)",
        ),
    ] {
        let mut source = changed_factory(from, to);
        if to.starts_with("localChunk(") {
            source = source.replacen("let $CQ = $KEY_268(void 0);",
                "let $CQ = $KEY_268(void 0); let localChunk = $KEY_353; localChunk = function(){return {};};", 1);
        }
        assert!(!recognizes(&source, "$THREAD_9_31784"), "accepted {from} -> {to}");
    }
}

#[test]
fn native_capture_local_alias_is_immutable_direct_and_not_published() {
    let declaration = "let $CE = $KEY_58;";
    let invocation = "$CE($CH, $CD.property, !!$CD.isConstructor)";
    assert_eq!(INLINE.matches(declaration).count(), 1);
    assert_eq!(INLINE.matches(invocation).count(), 1);
    let renamed = INLINE.replacen(declaration, "let nativeCaptureAlias = $KEY_58;", 1)
        .replacen(invocation, "nativeCaptureAlias($CH, $CD.property, !!$CD.isConstructor)", 1);
    assert!(recognizes(&renamed, "$THREAD_9_31784"));
    for replacement in [
        "let $CE = $KEY_58; $CE = function(){return {};};",
        "let $CE = $KEY_58; consume($CE);",
        "let $CE = $KEY_58; let escaped = {installer:$CE};",
        "let $CE = $KEY_58; $CE.call = function(){};",
        "let $CE = $KEY_58; let chained = $CE; chained = function(){};",
    ] {
        let source = INLINE.replacen(declaration, replacement, 1);
        assert!(!recognizes(&source, "$THREAD_9_31784"), "accepted native alias mutation {replacement}");
    }
}

#[test]
fn native_capture_nested_uses_require_a_reaching_same_container_definition() {
    let declaration = "let $CE = $KEY_58;";
    let invocation = "$CE($CH, $CD.property, !!$CD.isConstructor)";
    assert_eq!(INLINE.matches(declaration).count(), 1);
    assert_eq!(INLINE.matches(invocation).count(), 1);
    // Preserve the direct native call while adding one nested consumer scope.
    let nested = INLINE.replacen(invocation, &format!("(() => {invocation})()"), 1);
    assert!(recognizes(&nested, "$THREAD_9_31784"));
    for replacement in [
        "$CE({}, \"early\"); let $CE = $KEY_58;",
        "(() => $CE({}, \"early\"))(); let $CE = $KEY_58;",
        "let $CE; if (true) { $CE = $KEY_58; }",
    ] {
        let source = INLINE.replacen(declaration, replacement, 1);
        assert!(!recognizes(&source, "$THREAD_9_31784"), "accepted non-reaching alias: {replacement}");
    }
}

#[test]
fn original_header_optional_reads_require_the_exact_value_path_without_calls() {
    let guarded = "$BN.instrumented.setRequestHeader?.originals?.value";
    assert_eq!(INLINE.matches(guarded).count(), 2);
    // Both forms preserve their own reads; recognition does not rewrite them.
    let ordinary = INLINE.replace(guarded, "$BN.instrumented.setRequestHeader.originals.value");
    assert!(recognizes(&ordinary, "$THREAD_9_31784"));
    for (from, to) in [
        (
            "if ($BN.instrumented.setRequestHeader?.originals?.value != null)",
            "if ($BN.instrumented.setRequestHeader?.originals?.other != null)",
        ),
        (
            "$KEY_11 = $BN.instrumented.setRequestHeader?.originals?.value;",
            "$KEY_11 = $BN.instrumented.xhrSend?.originals?.value;",
        ),
        (
            "$KEY_11 = $BN.instrumented.setRequestHeader?.originals?.value;",
            "$KEY_11 = $BN.instrumented.setRequestHeader?.originals?.value?.();",
        ),
    ] {
        assert!(!recognizes(&changed_factory(from, to), "$THREAD_9_31784"));
    }
}

#[test]
fn original_header_cache_receiver_is_proven_before_later_record_reuse() {
    let begin = INLINE.find("function $THREAD_2_100270(").expect("native installer");
    let end = begin + INLINE[begin..].find("let $KEY_58 =").expect("native alias");
    let native = &INLINE[begin..end];
    let mut renamed = INLINE.to_owned();
    renamed.replace_range(begin..end, &native.replace("$BQ", "nativeObjectOrEntry"));
    assert!(recognizes(&renamed, "$THREAD_9_31784"));
    for (from, to) in [
        ("let $BQ = $KEY_8.containingObj;", "let $BQ = $BN;"),
        ("let $BU = $KEY_143($BQ, $BO);", "$BQ = $BN; let $BU = $KEY_143($BQ, $BO);"),
        ("let $BQ = $KEY_8.containingObj;", "$KEY_8.containingObj = $BN; let $BQ = $KEY_8.containingObj;"),
        ("let $BQ = $KEY_8.containingObj;", "let $BQ = $KEY_8.containingObj; mutateReceiver(); function mutateReceiver() { $BQ = $BN; }"),
    ] {
        assert_eq!(native.matches(from).count(), 1);
        let mut source = INLINE.to_owned();
        source.replace_range(begin..end, &native.replacen(from, to, 1));
        assert!(!recognizes(&source, "$THREAD_9_31784"), "accepted receiver mutation: {from} -> {to}");
    }
}

#[test]
fn reviewed_native_alias_self_argument_escape_is_refused() {
    let source = include_str!("fixtures/inject_request_headers_variants/self-argument-escape.js");
    assert!(!recognizes(source, "$THREAD_9_31784"), "H008-1: a callee's argument reference is an escape, not another callee read");
    let chained = INLINE.replacen("let $CE = $KEY_58;",
        "let $CE = $KEY_58; let nextAlias = $CE; nextAlias(nextAlias, \"send\");", 1);
    assert!(!recognizes(&chained, "$THREAD_9_31784"), "recursive alias arguments also escape");
}

#[test]
fn reviewed_native_alias_hoisted_early_call_is_refused() {
    let source = include_str!("fixtures/inject_request_headers_variants/early-alias-call.js");
    assert!(!recognizes(source, "$THREAD_9_31784"), "H008-2: actual hoisted activation precedes initialization");
    let declaration = "let $CE = $KEY_58;";
    assert_eq!(INLINE.matches(declaration).count(), 1);
    // Declaration order may vary; every actual activation is after the alias.
    for replacement in [
        "function useNativeAlias() { $CE({}, \"send\"); } let $CE = $KEY_58; useNativeAlias();",
        "let $CE = $KEY_58; useNativeAlias(); function useNativeAlias() { $CE({}, \"send\"); }",
        "function activateAlias() { useNativeAlias(); } function useNativeAlias() { $CE({}, \"send\"); } let $CE = $KEY_58; activateAlias();",
    ] {
        assert!(recognizes(&INLINE.replacen(declaration, replacement, 1), "$THREAD_9_31784"),
            "late activation must not depend on hoisted declaration position: {replacement}");
    }
    for replacement in [
        "activateAlias(); let $CE = $KEY_58; function activateAlias() { useNativeAlias(); } function useNativeAlias() { $CE({}, \"send\"); }",
        "consume(useNativeAlias); let $CE = $KEY_58; function useNativeAlias() { $CE({}, \"send\"); }",
        "let exposed = {run: useNativeAlias}; let $CE = $KEY_58; function useNativeAlias() { $CE({}, \"send\"); }",
        "(() => useNativeAlias())(); let $CE = $KEY_58; function useNativeAlias() { $CE({}, \"send\"); }",
        "useNativeAlias(); let $CE = $KEY_58; useNativeAlias(); function useNativeAlias() { $CE({}, \"send\"); }",
        "let $CE = $KEY_58; function useNativeAlias() { $CE({}, \"send\"); } useNativeAlias = function() {}; useNativeAlias();",
    ] {
        assert!(!recognizes(&INLINE.replacen(declaration, replacement, 1), "$THREAD_9_31784"),
            "early activation/publication or mutable callable must refuse: {replacement}");
    }
}
