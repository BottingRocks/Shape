use shape::naming::name_threads;

const EVALUATOR: &str = "network_url_pattern_matcher";
const ROLE: &str = "matchesRequestUrlPatterns";
const SOURCE: &str = include_str!("fixtures/naming/route_pattern_canonicalizer/url-pattern-matcher.js");

fn outcome(source: &str) -> (bool, Option<String>, Option<String>) {
    let result = name_threads(source, true).expect("valid scoped JavaScript");
    let entry = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_1_33315")
        .expect("fixture must expose the matcher binding");
    (
        entry.applied,
        entry.evaluator.clone(),
        entry.role_name.clone(),
    )
}

fn positive(source: &str) {
    let (applied, evaluator, role) = outcome(source);
    assert!(applied, "expected the matcher to be named");
    assert_eq!(evaluator.as_deref(), Some(EVALUATOR));
    assert_eq!(role.as_deref(), Some(ROLE));
}

fn negative(source: &str, why: &str) {
    let (applied, evaluator, role) = outcome(source);
    assert!(
        !(applied && evaluator.as_deref() == Some(EVALUATOR)),
        "must refuse {why}"
    );
    assert_ne!(role.as_deref(), Some(ROLE), "must refuse role for {why}");
}

fn replace_once(source: &str, from: &str, to: &str) -> String {
    assert!(source.contains(from), "fixture missing {from}");
    source.replacen(from, to, 1)
}

#[test]
fn whole_own_body_is_named_with_exact_protocol() {
    positive(SOURCE);
}

#[test]
fn alpha_renaming_of_locals_and_captures_is_irrelevant() {
    let alpha = SOURCE
        .replace("$BP", "request")
        .replace("$KEY_32", "cacheTable")
        .replace("$KEY_390", "anchorApi")
        .replace("$KEY_312", "defaultPorts")
        .replace("$KEY_316", "mergeRule");
    positive(&alpha);
}

#[test]
fn edits_replay_and_leave_native_spellings_alone() {
    let result = name_threads(SOURCE, true).expect("valid");
    let mut replay = SOURCE.to_owned();
    let mut edits = result.report.edits.clone();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.start));
    for edit in edits {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
    // Every live edit must replace generated THREAD text or delete a proven
    // empty stub; no KEY, native, property or captured spelling is editable.
    let mut checked = result.report.edits.clone();
    checked.sort_by_key(|edit| edit.start);
    for edit in &checked {
        let original = &SOURCE[edit.start as usize..edit.end as usize];
        assert!(
            original.contains("$THREAD_")
                || (edit.replacement.is_empty() && original.contains("function $THREAD_")),
            "edit touches non-generated text: {original:?}"
        );
    }
    assert_eq!(
        checked
            .iter()
            .filter(|edit| SOURCE[edit.start as usize..edit.end as usize].contains("$KEY_"))
            .count(),
        0,
        "no KEY binding may be edited"
    );
    for token in [
        "_subscriber",
        "_observer",
        "_cleanup",
        "createElement",
        "anchorHrefSet",
        "anchorHrefGet",
        "decodeURIComponent",
        "protocol",
        "hostname",
        "pathname",
        "search",
        "hash",
        "href",
        "unwrap",
    ] {
        assert_eq!(
            SOURCE.matches(token).count(),
            result.source.matches(token).count(),
            "property/native spelling {token} changed"
        );
    }
}

#[test]
fn rejects_changed_punycode_constants_and_regexes() {
    for (from, to) in [
        ("9007199254740991", "9007199254740990"),
        ("257348550135456.88", "257348550135456.5"),
        ("$KEY_20 = 34377;", "$KEY_20 = 34376;"),
        ("$CM > 253", "$CM >= 253"),
        ("$CB >= 253", "$CB > 253"),
        ("RegExp(\"(%(2[146-9a-f]|3[0-9abd]|4[0-9a-f]|5[0-9abdf]|6[1-9a-f]|7[0-9ae]))\", \"gi\")", "RegExp(\"(%(2[146-9a-f]|3[0-9abd]|4[0-9a-f]|5[0-9abdf]|6[1-9a-f]|7[0-9ae]))\", \"g\")"),
        ("/[xX][nN]--/.test($KEY_6.hostname)", "/[xX][nN]--/.test($KEY_6.hostname) === true"),
        ("$CO = 128", "$CO = 129"),
        ("$CP = 72", "$CP = 71"),
        ("$CZ > 455", "$CZ >= 455"),
        ("$CY += 36", "$CY += 35"),
    ] {
        negative(&replace_once(SOURCE, from, to), from);
    }
}

#[test]
fn rejects_changed_effects_order_and_branches() {
    for (from, to) in [
        ("$DE.href = ", "$DE.hrefText = "),
        ("$KEY_32[$KEY_1] = $DE;", ""),
        (
            "if ($KEY_28.host === \"\") {",
            "if ($KEY_28.host !== \"\") {",
        ),
        (
            "$DB.anchorHrefSet($KEY_28, $DB.anchorHrefGet($KEY_28));",
            "$DB.anchorHrefSet($KEY_28, $KEY_1);",
        ),
        (
            "throw new Error(\"Unknown Input\");",
            "throw new Error(\"Unknown\");",
        ),
        (
            "$KEY_23 = $BP.method.toLowerCase();",
            "$KEY_23 = $BP.method;",
        ),
        ("return $EL.some($EM => {", "return $EL.every($EM => {"),
        ("if (!$DG) {", "if ($DG) {"),
    ] {
        negative(&replace_once(SOURCE, from, to), from);
    }
}

#[test]
fn nested_punycode_predicate_keeps_its_own_canonical_rule() {
    let result = name_threads(SOURCE, true).expect("valid");
    let nested = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_6_139329")
        .expect("nested predicate");
    assert_ne!(nested.evaluator.as_deref(), Some(EVALUATOR));
}

#[test]
fn ordinary_unrelated_functions_are_not_claimed() {
    let source = r#"function bootstrap(){
let $KEY_32 = {}, $KEY_390 = {}, $KEY_312 = 0, $KEY_316 = {}, $KEY_401 = 0, $KEY_382 = 0, $KEY_259 = 0, $KEY_379 = 0, $KEY_210 = 0, $KEY_116 = 0;
function $THREAD_1_33315($BP, $BQ, $BR, $BS) {
let $KEY_1 = 1;
return $KEY_1;
}
return $THREAD_1_33315;
}
bootstrap();"#;
    let result = name_threads(source, true).expect("valid");
    let entry = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == "$THREAD_1_33315")
        .unwrap();
    assert_ne!(entry.evaluator.as_deref(), Some(EVALUATOR));
}

const LATE_STORE: &str =
    include_str!("fixtures/naming/route_pattern_canonicalizer/url-pattern-matcher-late-store.js");
const STORE_AFTER_EXPOSURE: &str =
    include_str!("fixtures/naming/route_pattern_canonicalizer/url-pattern-matcher-store-after-exposure.js");

#[test]
fn predeclared_captures_stored_before_exposure_are_positive() {
    positive(LATE_STORE);
}

#[test]
fn capture_store_after_matcher_invocation_is_refused() {
    negative(
        STORE_AFTER_EXPOSURE,
        "capture store after matcher invocation",
    );
}

#[test]
fn globals_keep_exact_names_and_cannot_be_shadowed() {
    negative(
        &replace_once(SOURCE, "window.location", "Promise.location"),
        "wrong allowed global",
    );
    for global in ["window", "RegExp", "Array", "Error", "decodeURIComponent"] {
        let shadow = replace_once(
            SOURCE,
            "function bootstrap() {",
            &format!("function bootstrap() {{\nvar {global} = {{}};"),
        );
        negative(&shadow, "shadowed host global");
    }
}

#[test]
fn initialized_captures_have_no_later_or_deferred_writers() {
    for writer in [
        "$KEY_390 = other;",
        "function deferredWriter() { $KEY_390 = other; }",
        "if (other) { $KEY_390 = other; }",
    ] {
        negative(
            &replace_once(
                SOURCE,
                "return $THREAD_1_33315;",
                &format!("{writer}\nreturn $THREAD_1_33315;"),
            ),
            "initialized capture writer",
        );
    }
}

#[test]
fn uninitialized_capture_store_must_execute_in_its_declaring_body() {
    for writer in [
        "function deferredWriter() { $KEY_390 = {}; }",
        "if (other) { $KEY_390 = {}; }",
        "other && ($KEY_390 = {});",
    ] {
        negative(
            &replace_once(LATE_STORE, "$KEY_390 = {};", writer),
            "non-direct or deferred store",
        );
    }
}

#[test]
fn preinitialization_calls_aliases_and_registration_are_refused() {
    let declaration = "function $THREAD_1_33315($BP, $BQ, $BR, $BS) {";
    for exposure in [
        "$THREAD_1_33315({});",
        "const alias = $THREAD_1_33315; alias({});",
        "register($THREAD_1_33315);",
        "function register(callback) { callback({}); } register($THREAD_1_33315);",
        "function invoke() { $THREAD_1_33315({}); } invoke();",
        "const early = later; const later = $THREAD_1_33315; early({});",
    ] {
        negative(
            &replace_once(
                LATE_STORE,
                declaration,
                &format!("{exposure}\n{declaration}"),
            ),
            "actual invocation/exposure before capture store",
        );
    }
}

#[test]
fn immutable_alias_before_setup_can_be_invoked_after_setup() {
    let source = replace_once(
        LATE_STORE,
        "function $THREAD_1_33315(",
        "const alias = $THREAD_1_33315;\nfunction $THREAD_1_33315(",
    );
    positive(&replace_once(
        &source,
        "return $THREAD_1_33315;",
        "alias({}); return alias;",
    ));
}

#[test]
fn private_function_expression_name_is_not_an_external_binding() {
    for exposure in [
        "$THREAD_1_33315({});",
        "const alias = $THREAD_1_33315; alias({});",
        "register($THREAD_1_33315);",
        "function register(callback) { callback({}); } register($THREAD_1_33315);",
    ] {
        let source = replace_once(
            LATE_STORE,
            "function $THREAD_1_33315(",
            "const $THREAD_2_33316 = function $THREAD_1_33315(",
        );
        let source = replace_once(
            &source,
            "$KEY_32 = {};",
            &format!(
                "; {}\n$KEY_32 = {{}};",
                exposure.replace("$THREAD_1_33315", "$THREAD_2_33316")
            ),
        );
        let source = replace_once(
            &source,
            "return $THREAD_1_33315;",
            "return $THREAD_2_33316;",
        );
        let allocator = oxc_allocator::Allocator::default();
        let parsed =
            oxc_parser::Parser::new(&allocator, &source, oxc_span::SourceType::default()).parse();
        assert!(parsed.errors.is_empty() && !parsed.panicked);
        let semantic = oxc_semantic::SemanticBuilder::new()
            .build(&parsed.program)
            .semantic;
        let scope = semantic.scoping();
        let external = scope
            .symbol_ids()
            .find(|s| scope.symbol_name(*s) == "$THREAD_2_33316")
            .expect("actual external binding");
        let private = scope
            .symbol_ids()
            .find(|s| scope.symbol_name(*s) == "$THREAD_1_33315")
            .expect("private expression binding");
        assert_ne!(external, private);
        assert!(
            scope.get_resolved_references(external).count() >= 2,
            "external callable is exposed and returned"
        );
        assert_eq!(scope.get_resolved_references(private).count(), 0);
        // The reporting layer indexes the private generated id, while calls
        // resolve to the distinct external symbol verified above.
        negative(
            &source,
            "named expression exposed through its external binding before setup",
        );
    }
}

#[test]
fn exact_operators_regex_flags_and_receiver_order_are_consumed() {
    for (from, to) in [
        ("/[xX][nN]--/", "/[xX][nN]--/i"),
        ("i++", "++i"),
        ("$BP.url || window.location", "$BP.url && window.location"),
        (".call($KEY_16[0], \".\")", ".call(\".\", $KEY_16[0])"),
        ("$BP.method.toLowerCase()", "(0, $BP.method.toLowerCase)()"),
    ] {
        negative(&replace_once(SOURCE, from, to), "whole protocol mutation");
    }
}
