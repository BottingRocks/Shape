use shape::naming::name_threads;

const DIRECT: &str = include_str!("fixtures/program/url-query-reader/captured-query-baseline.js");
const LOWERED: &str = include_str!("fixtures/program/url-query-reader/captured-query-alternate.js");
const ROLE: &str = "readUrlSearchParameterValues";

fn claims(source: &str) -> Vec<String> {
    name_threads(source, true)
        .expect("fixture must parse")
        .report
        .outcomes
        .iter()
        .filter(|o| {
            o.applied
                && o.evaluator.as_deref() == Some("thread_url_query_reader")
                && o.role_name.as_deref() == Some(ROLE)
        })
        .map(|o| o.binding.name.to_string())
        .collect()
}
fn changed(source: &str, from: &str, to: &str) -> String {
    assert!(source.contains(from), "mutation absent: {from}");
    source.replace(from, to)
}

#[test]
fn owned_query_reader_positive_variants() {
    for source in [
        DIRECT.to_string(),
        LOWERED.to_string(),
        DIRECT
            .replace("$THREAD_9_67964", "$THREAD_91_409_2")
            .replace("$KEY_1", "requestedKey")
            .replace("$BW", "urlInput"),
        changed(DIRECT, "$BZ = [$BZ, $BZ.split];", "")
            .replace("$BZ[1].call($BZ[0], \"&\")", "$BZ.split(\"&\")"),
        changed(LOWERED, "$BW = [$BW, $BW.split];", "")
            .replace("$BW[1].call($BW[0], \"&\")", "$BW.split(\"&\")"),
        changed(DIRECT, "let $KEY_1 = $BX;", "const $KEY_1 = $BX;"),
    ] {
        assert_eq!(claims(&source).len(), 1, "{source}");
    }
}

#[test]
fn owned_query_reader_rejects_wrong_dataflow_and_effects() {
    for source in [
        changed(DIRECT, "$CA[0] === $KEY_1", "$CA[1] === $KEY_1"),
        changed(DIRECT, "$CA[0] === $KEY_1", "$CA[0] !== $KEY_1"),
        changed(DIRECT, ".map($CA => $CA[1])", ".map($CA => $CA[0])"),
        changed(DIRECT, "$KEY_1 = $BX", "$KEY_1 = $BW"),
        changed(DIRECT, "$KEY_3.href = $KEY_0", "$KEY_3.href = $KEY_1"),
        changed(DIRECT, ".search.slice(1)", ".search.slice(0)"),
        changed(
            DIRECT,
            "$BZ[1].call($BZ[0], \"&\")",
            "$BZ[1].call($KEY_1, \"&\")",
        ),
        changed(DIRECT, "$CA.split(\"=\")", "$CA.split(\":\")"),
        changed(DIRECT, "decodeURIComponent", "encodeURIComponent"),
        changed(DIRECT, ".toString()", ".toString(1)"),
        changed(DIRECT, "$KEY_0 = [$KEY_0, $CB]", "$KEY_0 = [$CB, $KEY_0]"),
        changed(DIRECT, "return $KEY_0;", "return $KEY_1;"),
        changed(
            DIRECT,
            "let $KEY_0 = $BW.trim();",
            "let $KEY_0 = $BX.trim();",
        ),
        changed(DIRECT, "return [];", "return null;"),
        changed(
            DIRECT,
            "function main(){",
            "function main(){let document = fakeDocument;",
        ),
        changed(
            DIRECT,
            "function main(){",
            "function main(){let decodeURIComponent = fakeDecode;",
        ),
        changed(
            DIRECT,
            "function main(){",
            "function main(){let RegExp = fakeRegex;",
        ),
        changed(DIRECT, "return $KEY_0;", "sideEffect(); return $KEY_0;"),
        changed(LOWERED, "$BS = true;", "$BS = false;"),
        changed(
            LOWERED,
            "$BS = $BQ.trim() === \"\";",
            "$BS = $BR.trim() === \"\";",
        ),
        changed(
            DIRECT,
            "function $THREAD_9_67964(",
            "async function $THREAD_9_67964(",
        ),
        changed(
            DIRECT,
            ".filter($CA => $CA[0] === $KEY_1)",
            ".filter($CA => { $KEY_1 = \"other\"; return $CA[0] === $KEY_1; })",
        ),
    ] {
        assert!(claims(&source).is_empty(), "{source}");
    }
}

#[test]
fn owned_query_reader_rejects_cross_callable_donors() {
    // An exact reader in a nested function cannot lend its algorithm to an
    // unrelated generated wrapper; only the child itself may be named.
    let child = DIRECT
        .replace(
            "function main(){",
            "function main(){function $THREAD_8_700(){",
        )
        .replace("}main();", "} return $THREAD_8_700;}main();");
    assert_eq!(claims(&child), vec!["$THREAD_9_67964"]);
    let hoisted = changed(
        DIRECT,
        ".filter($CA => $CA[0] === $KEY_1)",
        ".filter(otherPredicate)",
    );
    let hoisted = changed(
        &hoisted,
        "function main(){",
        "function main(){let otherPredicate = pair => pair[0] === externalKey;",
    );
    assert!(claims(&hoisted).is_empty());
    let wrong_capture = changed(DIRECT, "$CA[0] === $KEY_1", "$CA[0] === outsideKey");
    let wrong_capture = changed(
        &wrong_capture,
        "function main(){",
        "function main(){let outsideKey = \"x\";",
    );
    assert!(claims(&wrong_capture).is_empty());
}
