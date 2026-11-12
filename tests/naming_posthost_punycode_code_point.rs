//! Focused coverage for the punycode basic-code-point predicate if/else spelling.
//!
//! The inline donor returns two code ranges and the exact hyphen code. The branched variant
//! writes `if (ranges) { return true; } else { return code === 45; }`. Recognition requires
//! one owned IfStatement with one pure return per arm, reuses range, bound-character, and
//! exact-code proofs, and retains every()/route context.
//!
//! Positives cover the verbatim branched predicate, inline donor, and alpha spelling.
//! Negatives change results, exact code, inputs, bounds, effects, statements, nesting, or
//! route context.

use shape::naming::name_threads;

const EVALUATOR: &str = "function_finder::punycode_basic_code_point_predicate";
const ROUTE_EVALUATOR: &str = "function_finder::request_route_matcher";
const NAME: &str = "isPunycodeBasicCodePoint";

/// Verbatim if/else predicate body. Leading indentation is part of the source bytes.
const ACTUAL_PREDICATE: &str = "function $THREAD_0_29760($DD) {\n\t\t\t\t\t\t\t\t\t\t\tif ($DD >= $KEY_96 && $DD <= $KEY_299 || $DD >= $KEY_391 && $DD <= $KEY_166) {\n\t\t\t\t\t\t\t\t\t\t\t\treturn true;\n\t\t\t\t\t\t\t\t\t\t\t} else {\n\t\t\t\t\t\t\t\t\t\t\t\treturn $DD === 45;\n\t\t\t\t\t\t\t\t\t\t\t}\n\t\t\t\t\t\t\t\t\t\t}";

/// Intentional minimal spelling changes for the negative carriers. They reuse
/// the same bound declarations and route context.
const EXTRA_STATEMENT: &str = r#"function $THREAD_0_29760($DD) {
    if ($DD >= $KEY_96 && $DD <= $KEY_299 || $DD >= $KEY_391 && $DD <= $KEY_166) {
        return true;
    } else {
        return $DD === 45;
    }
    return false;
}"#;

const NESTED_BRANCH: &str = r#"function $THREAD_0_29760($DD) {
    if ($DD >= $KEY_96 && $DD <= $KEY_299 || $DD >= $KEY_391 && $DD <= $KEY_166) {
        return true;
    } else {
        if ($DD > 0) {
            return $DD === 45;
        }
        return false;
    }
}"#;

/// Minimal carrier that the existing p07 finder proves: a URL parser and a
/// route matcher with a static `some()` rule, a hostname regex and the
/// predicate handed to `.every()` inside the matcher. Nothing else supplies
/// the predicate's role.
const TEMPLATE: &str = r#"function main(){
var $KEY_DOC = document;
var $KEY_TABLE = [];
var $KEY_HOST = /[xX][nN]--/;
let $KEY_96, $KEY_299, $KEY_391, $KEY_166;
$KEY_391 = $KEY_CHAR("a");
$KEY_166 = $KEY_CHAR("z");
$KEY_96 = $KEY_CHAR("0");
$KEY_299 = $KEY_CHAR("9");
function $THREAD_URL($KEY_REQ) {
    let $KEY_ANCHOR = $KEY_DOC.createElement("a");
    $KEY_ANCHOR.href = $KEY_REQ;
    let $KEY_RESULT = {
        protocol: $KEY_ANCHOR.protocol,
        hostname: $KEY_ANCHOR.hostname,
        port: $KEY_ANCHOR.port,
        pathname: $KEY_ANCHOR.pathname,
        search: $KEY_ANCHOR.search,
        hash: $KEY_ANCHOR.hash,
        href: $KEY_ANCHOR.href
    };
    return $KEY_RESULT;
}
function $THREAD_MATCH($KEY_REQ) {
    let $KEY_URL = $THREAD_URL($KEY_REQ.url);
    let $KEY_METHOD = $KEY_REQ.method.toLowerCase();
    let $KEY_HIT = $KEY_TABLE.some(function ($KEY_RULE) {
        return $KEY_RULE.protocol.test($KEY_URL.protocol) && $KEY_RULE.hostname.test($KEY_URL.hostname) && $KEY_RULE.port.test($KEY_URL.port) && $KEY_RULE.pathname.test($KEY_URL.pathname) && $KEY_RULE.search.test($KEY_URL.search);
    });
    if ($KEY_HOST.test($KEY_URL.hostname)) {
        let $KEY_LABELS = $KEY_URL.hostname.split(".");
        #PREDICATE#
        if (!$KEY_LABELS.every($THREAD_0_29760)) {
            throw new Error("bad input");
        }
    }
    return $KEY_HIT;
}
return $THREAD_MATCH;
}
main();"#;

fn source(predicate: &str) -> String {
    TEMPLATE.replace("#PREDICATE#", predicate)
}

fn parses(source: &str) -> bool {
    name_threads(source, true).is_ok()
}

fn named(source: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result
            .report
            .outcomes
            .iter()
            .any(|outcome| outcome.applied && outcome.proposed_name.as_deref() == Some(NAME))
    })
}

fn claimed(source: &str, evaluator: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result
            .report
            .function_roles
            .iter()
            .any(|role| role.evaluators.iter().any(|found| found == evaluator))
    })
}

fn accepts(predicate: &str) {
    let source = source(predicate);
    assert!(parses(&source), "fixture did not parse: {source}");
    assert!(claimed(&source, ROUTE_EVALUATOR), "route matcher was not proven: {source}");
    assert!(claimed(&source, EVALUATOR), "predicate was not claimed: {source}");
    assert!(named(&source), "predicate was not named: {source}");
}

/// The route context stays proven, but only this predicate role must be absent.
fn denied_predicate(predicate: &str, cause: &str) {
    let source = source(predicate);
    assert!(parses(&source), "{cause}: fixture did not parse");
    assert!(claimed(&source, ROUTE_EVALUATOR), "{cause}: route matcher was not proven");
    assert!(!claimed(&source, EVALUATOR), "{cause}: predicate was claimed");
    assert!(!named(&source), "{cause}: predicate was named");
}

fn denied_source(source: &str, cause: &str) {
    assert!(parses(source), "{cause}: fixture did not parse");
    assert!(!claimed(source, EVALUATOR), "{cause}: predicate was claimed");
    assert!(!named(source), "{cause}: predicate was named");
}

/// The verbatim if/else body and inline donor must name the same role.
#[test]
fn accepts_actual_if_else_and_old_inline_base() {
    assert_eq!(ACTUAL_PREDICATE.len(), 222);
    accepts(ACTUAL_PREDICATE);

    let old_inline = r#"function $THREAD_0_29760($DD) {
    return $DD >= $KEY_96 && $DD <= $KEY_299 || $DD >= $KEY_391 && $DD <= $KEY_166 || $DD === 45;
}"#;
    accepts(old_inline);
}

/// Recognition reads the structure, not the generated spellings.
#[test]
fn accepts_renamed_spellings() {
    // Keep the literal `$THREAD_` grammar the namer keys on and rename complete
    // identifiers, not the shared prefix: the predicate's id/PC change together
    // at its declaration and its `.every()` reference, and the two helper
    // threads get new spellings. Truncating the prefix (`$HT0_29760`) would
    // erase the generated identity and is deliberately not done.
    let renamed = TEMPLATE
        .replace("#PREDICATE#", ACTUAL_PREDICATE)
        .replace("$KEY_", "$HK")
        .replace("$THREAD_0_29760", "$THREAD_7_48121")
        .replace("$THREAD_URL", "$THREAD_PARSE")
        .replace("$THREAD_MATCH", "$THREAD_ROUTE");
    assert!(parses(&renamed));
    assert!(claimed(&renamed, ROUTE_EVALUATOR));
    assert!(claimed(&renamed, EVALUATOR));
    assert!(named(&renamed));
}

/// `return true` is not `return false`, and the exact code must be 45.
#[test]
fn denies_false_consequent_and_wrong_exact_code() {
    denied_predicate(
        &ACTUAL_PREDICATE.replace("return true;", "return false;"),
        "consequent false",
    );
    denied_predicate(
        &ACTUAL_PREDICATE.replace("$DD === 45", "$DD === 44"),
        "exact code 44",
    );
    denied_predicate(
        &ACTUAL_PREDICATE.replace("$DD === 45", "$DD === 46"),
        "exact code 46",
    );
}

/// All comparisons must read the same input, and the bound characters must be
/// the digit and lowercase-letter pairs.
#[test]
fn denies_foreign_input_and_changed_bounds() {
    let foreign = ACTUAL_PREDICATE
        .replace("$THREAD_0_29760($DD)", "$THREAD_0_29760($DD, $EE)")
        .replace("$DD === 45", "$EE === 45");
    denied_predicate(&foreign, "foreign second parameter");

    let changed_bound = source(ACTUAL_PREDICATE)
        .replace("$KEY_299 = $KEY_CHAR(\"9\");", "$KEY_299 = $KEY_CHAR(\"8\");");
    denied_source(&changed_bound, "changed digit high bound");
    assert!(claimed(&changed_bound, ROUTE_EVALUATOR));

    let changed_letter = source(ACTUAL_PREDICATE)
        .replace("$KEY_166 = $KEY_CHAR(\"z\");", "$KEY_166 = $KEY_CHAR(\"y\");");
    denied_source(&changed_letter, "changed letter high bound");
    assert!(claimed(&changed_letter, ROUTE_EVALUATOR));

    let extra_disjunct = ACTUAL_PREDICATE.replace(
        "$DD >= $KEY_391 && $DD <= $KEY_166)",
        "$DD >= $KEY_391 && $DD <= $KEY_166 || $DD === 45)",
    );
    denied_predicate(&extra_disjunct, "extra disjunct");
}

/// An effectful arm expression, an extra owned statement and a nested branch
/// are all outside the one-If/one-return view.
#[test]
fn denies_side_effects_extra_statements_and_nested_branch() {
    denied_predicate(
        &ACTUAL_PREDICATE.replace("return $DD === 45;", "return ($DD = 45);"),
        "assignment in alternate arm",
    );
    denied_predicate(
        &ACTUAL_PREDICATE.replace(
            "return $DD === 45;",
            "return $DD === 45 && $KEY_SIDE();",
        ),
        "call in alternate arm",
    );
    denied_predicate(EXTRA_STATEMENT, "extra owned statement");
    denied_predicate(NESTED_BRANCH, "nested branch");
}

/// The predicate is role-bearing only as the `.every()` callback of the proven
/// route matcher; another method, a direct call or a missing static rule does
/// not name it.
#[test]
fn denies_missing_every_and_route_context() {
    let no_every = source(ACTUAL_PREDICATE).replace(
        ".every($THREAD_0_29760)",
        ".map($THREAD_0_29760)",
    );
    denied_source(&no_every, "map instead of every");
    assert!(claimed(&no_every, ROUTE_EVALUATOR));

    let direct_call = source(ACTUAL_PREDICATE)
        .replace("$KEY_LABELS.every($THREAD_0_29760)", "$THREAD_0_29760($KEY_LABELS[0])");
    denied_source(&direct_call, "direct call");
    assert!(claimed(&direct_call, ROUTE_EVALUATOR));

    let no_static_rule = source(ACTUAL_PREDICATE).replace(".some(function", ".filter(function");
    denied_source(&no_static_rule, "missing static rule");
    assert!(!claimed(&no_static_rule, ROUTE_EVALUATOR));
}
