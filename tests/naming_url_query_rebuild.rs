//! Portable whole-behavior proof tests. Every negative parses and runs through
//! name_threads (including Model construction); rejection cannot be a parse error.
use shape::naming::name_threads;
const SOURCE: &str = include_str!("fixtures/url_query_rebuild/primary.js");
fn accepted(source: &str) -> bool {
    let result = name_threads(source, true).expect("fixture must parse/build a valid Model");
    result.report.function_roles.iter().any(|r| {
        r.evaluators.iter().any(|e| e == "thread_url_query_rebuild")
            && r.candidates
                .iter()
                .any(|n| n == "rebuildUrlWithEncodedQuery")
    })
}
fn change(from: &str, to: &str) -> String {
    assert!(SOURCE.contains(from), "missing mutation {from}");
    SOURCE.replacen(from, to, 1)
}
#[test]
fn exact_owned_reconstruction_and_alpha_renaming() {
    assert!(accepted(SOURCE));
    assert!(accepted(
        &SOURCE
            .replace("$KEY_", "alias_")
            .replace("$THREAD_", "worker_")
            .replace("$AI", "urlInput")
            .replace("$AJ", "queryName")
            .replace("$AK", "queryValue")
    ));
}
#[test]
fn direct_split_and_inline_optional_port() {
    let direct = SOURCE
        .replace("$AO = [$AO, $AO.split];", "")
        .replace("$AO[1].call($AO[0], \"&\")", "$AO.split(\"&\")")
        .replace("$AY = [$AY, $AY.split];", "")
        .replace("$AY[1].call($AY[0], \"&\")", "$AY.split(\"&\")");
    assert!(accepted(&direct));
    let assignment = "$KEY_5 = typeof $AJ === \"undefined\" ? \"undefined\" : $AJ === null ? \"null\" : encodeURIComponent($AJ.toString()).replace(new RegExp(RegExp(\"%20\", \"g\")), \"+\");";
    let explicit = "if (typeof $AJ === \"undefined\") { $KEY_5 = \"undefined\"; } else if ($AJ === null) { $KEY_5 = \"null\"; } else { $KEY_5 = encodeURIComponent($AJ.toString()).replace(new RegExp(RegExp(\"%20\", \"g\")), \"+\"); }";
    let variant = SOURCE.replacen(assignment, explicit, 1);
    assert_ne!(variant, SOURCE);
    assert!(accepted(&variant));
    assert!(!accepted(&variant.replacen(
        "$KEY_5 = \"null\";",
        "$KEY_5 = \"undefined\";",
        1
    )));
    let start = SOURCE.rfind("let $AZ;").unwrap();
    let stop = SOURCE[start..].find("return $AV.protocol").unwrap() + start;
    let mut hash_variant = SOURCE[..start].to_owned();
    hash_variant.push_str("let suffix; if ($AV.hash.length > 0) { suffix = \"#\" + $AV.hash; } else { suffix = \"\"; }\n");
    hash_variant.push_str(
        &SOURCE[stop..]
            .replace(
                "+ $AZ +",
                "+ ($AV.port.length > 0 ? \":\" + $AV.port : \"\") +",
            )
            .replace("($AV.hash.length > 0 ? \"#\" + $AV.hash : \"\")", "suffix"),
    );
    assert!(accepted(&hash_variant));
}
#[test]
fn semantic_changes_and_scope_donors_are_rejected() {
    for (from, to) in [
        (
            "encodeURIComponent($AJ.toString())",
            "encodeURIComponent($AK.toString())",
        ),
        (
            "encodeURIComponent($AK.toString())",
            "decodeURIComponent($AK.toString())",
        ),
        ("RegExp(\"%20\", \"g\")", "RegExp(\"%20\", \"i\")"),
        ("$AN.href = $AI", "$AN.href = $AJ"),
        (
            "$AQ.protocol.slice(0, $AQ.protocol.length - 1)",
            "$AQ.protocol.slice(1, $AQ.protocol.length - 1)",
        ),
        ("$AQ.hostname,", "$AQ.host,"),
        (
            "$AP.anchorHrefSet($AQ, $KEY_9)",
            "$AP.anchorHrefSet($AQ, $AI)",
        ),
        ("return $AI.href;", "return $AI.host;"),
        ("$AI.href = $AJ;", "$AI.host = $AJ;"),
        (
            "Object.getOwnPropertyDescriptor(window.HTMLAnchorElement.prototype, \"href\")",
            "Object.getOwnPropertyDescriptor(window.HTMLAnchorElement.prototype, \"host\")",
        ),
        ("https: \"443\"", "https: \"444\""),
        ("$KEY_184 = Object.create(null)", "$KEY_184 = {}"),
        (
            "$BA => decodeURIComponent($BA)",
            "$BA => encodeURIComponent($BA)",
        ),
        (
            "$KEY_7.length > 0 ? \":\" + $KEY_7",
            "$KEY_7.length > 0 ? \";\" + $KEY_7",
        ),
        (
            "$KEY_10.length > 0 ? \"?\" + $KEY_10",
            "$KEY_10.length > 0 ? \"#\" + $KEY_10",
        ),
        (
            "let $KEY_6 = $KEY_184[$KEY_9].protocol;",
            "let $KEY_6 = $KEY_184[$AI].protocol;",
        ),
        ("let $KEY_10 = \"\";", "let $KEY_10 = \"x\";"),
        ("let $AP = $KEY_31;", "let $KEY_31 = {}; let $AP = $KEY_31;"),
        (
            "let $AP = $KEY_31;",
            "let $AP = {}; function unused(){return $KEY_31;}",
        ),
        ("let $AP = $KEY_31;", "$KEY_31 = {}; let $AP = $KEY_31;"),
        ("$KEY_5 = typeof $AJ", "$KEY_1 = typeof $AJ"),
        (
            "return $THREAD_7_118108;",
            "$KEY_58.http = \"81\"; return $THREAD_7_118108;",
        ),
        (
            "return $THREAD_7_118108;",
            "function unused(){ $KEY_58[\"https\"] = \"444\"; } return $THREAD_7_118108;",
        ),
        (
            "return $THREAD_7_118108;",
            "function unused(){ $KEY_31.anchorHrefSet = function(){}; } return $THREAD_7_118108;",
        ),
        (
            "return $THREAD_7_118108;",
            "function unused(){ $AA[\"anchorHrefSet\"] = function(){}; } return $THREAD_7_118108;",
        ),
        (
            "return $THREAD_7_118108;",
            "function unused(){ consume($KEY_58); } return $THREAD_7_118108;",
        ),
    ] {
        assert!(
            !accepted(&change(from, to)),
            "unsafe accepted mutation: {from} -> {to}"
        );
    }
    let shadowed = SOURCE.replacen(
        "function main(){",
        "function main(encodeURIComponent, RegExp, document){",
        1,
    );
    assert!(!accepted(&shadowed));
}

#[test]
fn early_activation_before_capture_stores_is_rejected() {
    let early = change(
        "let $KEY_58,$KEY_31,$KEY_184;",
        "let $KEY_58,$KEY_31,$KEY_184; $THREAD_7_118108(\"https://example.test/\",\"x\",\"y\");",
    );
    assert!(
        !accepted(&early),
        "hoisted activation precedes all capture stores"
    );
}

const PREAMBLE: &str = "let $KEY_58,$KEY_31,$KEY_184;";
const RETURN_CANDIDATE: &str = "return $THREAD_7_118108;";

fn before_setup(code: &str) -> String {
    change(PREAMBLE, &format!("{PREAMBLE} {code}"))
}

#[test]
fn all_early_exposures_and_unproved_activation_chains_are_refused() {
    for code in [
        "consume($THREAD_7_118108);",
        "let alias = $THREAD_7_118108; consume(alias);",
        "let alias = $THREAD_7_118108; let next = alias; next('https://example.test/','x','y');",
        "consume(() => $THREAD_7_118108('https://example.test/','x','y'));",
        "(function(){ $THREAD_7_118108('https://example.test/','x','y'); })();",
        "function relay(arg){ $THREAD_7_118108('https://example.test/','x','y'); } relay(relay);",
        "function relay(){ $THREAD_7_118108('https://example.test/','x','y'); } consume(relay);",
        // The wrapper's declaration is after all stores, but its call is not.
        "lateDeclared();",
        // A late use cannot repair an early read of a var-undefined alias.
        "consume(alias); var alias = $THREAD_7_118108;",
    ] {
        let mut source = before_setup(code);
        if code == "lateDeclared();" {
            source = source.replace(
                RETURN_CANDIDATE,
                "function lateDeclared(){ $THREAD_7_118108('https://example.test/','x','y'); } return $THREAD_7_118108;",
            );
        }
        assert!(!accepted(&source), "unsafe activation/exposure: {code}");
    }
    let unused = before_setup(
        "function unused(){ return $THREAD_7_118108('https://example.test/','x','y'); }",
    )
    .replace(RETURN_CANDIDATE, "return 0;");
    assert!(!accepted(&unused), "unused wrapper is no activation proof");
    let cycle = before_setup(
        "function cyclic(){ cyclic(); return $THREAD_7_118108('https://example.test/','x','y'); }",
    )
    .replace(RETURN_CANDIDATE, "return cyclic;");
    assert!(!accepted(&cycle), "cyclic activation graph is not proved");
    let mutable = before_setup("let alias = $THREAD_7_118108;")
        .replace(RETURN_CANDIDATE, "alias = replacement; return alias;");
    assert!(!accepted(&mutable), "mutable early alias is not followed");
    let conditional = before_setup("let alias; if (flag) { alias = $THREAD_7_118108; }")
        .replace(RETURN_CANDIDATE, "return alias;");
    assert!(
        !accepted(&conditional),
        "conditional early alias is not proved"
    );
}

#[test]
fn completed_stores_reach_every_late_activation_and_exposure() {
    for late in [
        "$THREAD_7_118108('https://example.test/','x','y'); return $THREAD_7_118108;",
        "consume($THREAD_7_118108); return $THREAD_7_118108;",
        "let alias = $THREAD_7_118108; consume(alias); return alias;",
        "let alias = $THREAD_7_118108; let next = alias; return next;",
    ] {
        assert!(
            accepted(&change(RETURN_CANDIDATE, late)),
            "late exposure: {late}"
        );
    }
    let inert = before_setup("let alias = $THREAD_7_118108; let next = alias;")
        .replace(RETURN_CANDIDATE, "return next;");
    assert!(
        accepted(&inert),
        "inert early aliases have only late exposures"
    );
    let hoisted = before_setup(
        "function hoisted(){ return $THREAD_7_118108('https://example.test/','x','y'); }",
    )
    .replace(RETURN_CANDIDATE, "return { init: hoisted };");
    assert!(
        accepted(&hoisted),
        "hoisted wrapper is exposed only after setup"
    );
    let arrow =
        before_setup("const callback = () => $THREAD_7_118108('https://example.test/','x','y');")
            .replace(RETURN_CANDIDATE, "consume(callback); return callback;");
    assert!(accepted(&arrow), "stable callback has only late consumers");
    let expression = before_setup(
        "const callback = function(){ return $THREAD_7_118108('https://example.test/','x','y'); };",
    )
    .replace(RETURN_CANDIDATE, "return callback;");
    assert!(
        accepted(&expression),
        "stable function expression is exposed late"
    );
    let same_callee_argument = before_setup(
        "function relay(arg){ return $THREAD_7_118108('https://example.test/','x','y'); }",
    )
    .replace(RETURN_CANDIDATE, "relay(relay); return relay;");
    assert!(
        accepted(&same_callee_argument),
        "both relay(relay) reads are late"
    );
}

#[test]
fn sole_recovered_alias_store_reaches_all_late_exposures() {
    let call = "$THREAD_7_118108('https://example.test/','x','y');";
    for prefix in [
        "let alias; alias = $THREAD_7_118108;",
        "var alias; alias = $THREAD_7_118108;",
        "let alias, next; alias = $THREAD_7_118108; next = alias;",
    ] {
        let name = if prefix.contains("next") {
            "next"
        } else {
            "alias"
        };
        let source = before_setup(prefix).replace(RETURN_CANDIDATE, &format!("return {name};"));
        assert!(accepted(&source), "sole recovered assignment: {prefix}");
    }
    let hoisted = before_setup(
        "let alias; function wrapper(){ return alias('https://example.test/','x','y'); } alias = $THREAD_7_118108;",
    ).replace(RETURN_CANDIDATE, "return {init:wrapper};");
    assert!(
        accepted(&hoisted),
        "use inside earlier hoisted wrapper activates late"
    );
    for prefix in [
        "let alias; alias = $THREAD_7_118108; consume(alias);",
        "var alias; consume(alias); alias = $THREAD_7_118108;",
        "let alias; if(flag){ alias = $THREAD_7_118108; }",
        "let alias; function initialize(){ alias = $THREAD_7_118108; }",
        "let alias; alias = $THREAD_7_118108; alias = replacement;",
        "let alias = replacement; alias = $THREAD_7_118108;",
        "let alias; alias = $THREAD_7_118108($THREAD_7_118108);",
        "let alias; function wrapper(){ return alias('https://example.test/','x','y'); } wrapper(); alias = $THREAD_7_118108;",
        "let alias; alias = $THREAD_7_118108; function recursive(){ recursive(); return alias(); }",
    ] {
        let source = before_setup(prefix).replace(RETURN_CANDIDATE, "return alias;");
        assert!(!accepted(&source), "unproved recovered alias: {prefix}");
    }
    assert!(!accepted(&before_setup(&format!(
        "let alias; alias = $THREAD_7_118108; {call}"
    ))));
}
