use shape::naming::name_threads;

fn names_persistence(source: &str) -> bool {
    name_threads(source, true)
        .expect("naming")
        .report
        .outcomes
        .iter()
        .any(|outcome| {
            outcome.applied
                && outcome.proposed_name.as_deref() == Some("loadAndPersistTelemetryState")
        })
}

/// Minimal full-context harness shared by every fixture.  Only the persistence
/// body changes; the telemetry records, storage handles, captured error sink
/// and the AES-GCM encryptor stay identical so a difference is always the
/// intended structural variant.
const HARNESS: &str = r#"
function main() {
    let telemetryState = { et: [], lit: 0, tlr: 0, top: 0, lvTop: 0 };
    let telemetryAggregate = { a: { c: 0, ci: 0, ts: 0 }, b: 0 };
    let globalObject = { localStorage: null };
    let otherObject = { localStorage: null };
    let capturedText = JSON.stringify(telemetryState);
    let parsedReports;
    function errorSink(error) { return error; }
    function encryptString(value) {
        let algorithm = { name: "AES-GCM" };
        crypto.generateKey(algorithm, true, ["encrypt"]);
        crypto.exportKey("raw", value);
        return crypto.encrypt(algorithm, value, value);
    }
    function $THREAD_1_100() {
__BODY__
    }
    return $THREAD_1_100;
}
main();
"#;

fn source(body: &str) -> String {
    HARNESS.replace("__BODY__", body)
}

/// The 27-family structural form: no post-read alias, the serialization
/// temporary is declared once and then self-reassigned through the captured
/// encryptor, and the upload stringifies the parsed-derived stored map after
/// the `Object.fromEntries(...slice(-N))` fold.
const NEW_SELF_REASSIGN: &str = r#"        let parsed = Object.create(null);
        let storedReports;
        let item;
        try {
            item = globalObject.localStorage.getItem("storage-key");
        } catch (error) {
            errorSink({ message: "read", stack: "" + error });
            return void 0;
        }
        if (item != null) {
            try {
                parsed = JSON.parse(item);
            } catch (parseError) {
                errorSink({ message: "parse", stack: parseError + "" });
                return void 0;
            }
            if (Object.keys(parsed).length > 0) {
                parsedReports = parsed;
                storedReports = parsed;
            }
        }
        if (Object.keys(telemetryState).length !== 0) {
            let text = JSON.stringify(telemetryState);
            text = encryptString(text);
            if (text != null) {
                text.then(value => {
                    parsed[0] = new Uint8Array(value).toString();
                    storedReports = parsed;
                    storedReports = Object.fromEntries(Object.entries(storedReports).slice(-5));
                    globalObject.localStorage.setItem("storage-key", JSON.stringify(storedReports));
                }).catch(error => {
                    return errorSink({ message: "upload", stack: "" + error });
                });
            }
        }
        return void 0;"#;

/// The loadAndPersistTelemetryState alias-reuse shape has a redundant
/// `let alias = read;` and reuses the same alias
/// as the promise result.
const OLD_ALIAS_REUSE: &str = r#"        let parsed = Object.create(null);
        let item;
        try {
            item = globalObject.localStorage.getItem("storage-key");
        } catch (error) {
            errorSink({ message: "read", stack: "" + error });
            return void 0;
        }
        let readAlias = item;
        if (readAlias != null) {
            try {
                parsed = JSON.parse(item);
            } catch (parseError) {
                errorSink({ message: "parse", stack: parseError + "" });
                return void 0;
            }
            if (Object.keys(parsed).length > 0) {
                parsedReports = parsed;
            }
        }
        if (Object.keys(telemetryState).length !== 0) {
            let text = JSON.stringify(telemetryState);
            readAlias = encryptString(text);
            if (readAlias != null) {
                readAlias.then(value => {
                    globalObject.localStorage.setItem("storage-key", JSON.stringify(parsed));
                }).catch(error => {
                    return errorSink({ message: "upload", stack: "" + error });
                });
            }
        }
        return void 0;"#;

/// In the declarator-result shape, the encryptor
/// result is bound by a declarator and the argument is a direct stringify.
const OLD_DECLARATOR_RESULT: &str = r#"        let parsed = Object.create(null);
        let item;
        try {
            item = globalObject.localStorage.getItem("storage-key");
        } catch (error) {
            errorSink({ message: "read", stack: "" + error });
            return void 0;
        }
        if (item != null) {
            try {
                parsed = JSON.parse(item);
            } catch (parseError) {
                errorSink({ message: "parse", stack: parseError + "" });
                return void 0;
            }
            if (Object.keys(parsed).length > 0) {
                parsedReports = parsed;
            }
        }
        if (Object.keys(telemetryState).length !== 0) {
            let encrypted = encryptString(JSON.stringify(telemetryState));
            if (encrypted != null) {
                encrypted.then(value => {
                    globalObject.localStorage.setItem("storage-key", JSON.stringify(parsed));
                }).catch(error => {
                    return errorSink({ message: "upload", stack: "" + error });
                });
            }
        }
        return void 0;"#;

fn mutate(base: &str, from: &str, to: &str) -> String {
    let changed = base.replace(from, to);
    assert_ne!(changed, base, "mutation did not apply: {from}");
    changed
}

#[test]
fn persistence_base_and_new_family_variants_share_the_same_role() {
    let base = source(NEW_SELF_REASSIGN);
    // BASE positive MUST pass before every near-miss below; if it does not the
    // negative loop is vacuous.
    assert!(names_persistence(&base), "new-family BASE must be named");

    // The 8-member placeholder-object + `stack` field-write variant.
    let placeholder = mutate(
        &base,
        "            errorSink({ message: \"read\", stack: \"\" + error });",
        "            let readFailure = { message: \"read\" };\n            readFailure.stack = \"\" + error;\n            errorSink(readFailure);",
    );
    let placeholder = mutate(
        &placeholder,
        "                errorSink({ message: \"parse\", stack: parseError + \"\" });",
        "                let parseFailure = { message: \"parse\", stack: void 0 };\n                parseFailure.stack = parseError + \"\";\n                errorSink(parseFailure);",
    );
    assert!(names_persistence(&placeholder), "placeholder-write variant must be named");

    // The proper-declarator result binding (`let result = encryptor(temp)`).
    let declared = mutate(
        &base,
        "            let text = JSON.stringify(telemetryState);\n            text = encryptString(text);\n            if (text != null) {",
        "            let text = JSON.stringify(telemetryState);\n            let encryptedResult = encryptString(text);\n            if (encryptedResult != null) {",
    );
    let declared = mutate(
        &declared,
        "                text.then(value => {",
        "                encryptedResult.then(value => {",
    );
    assert!(names_persistence(&declared), "declarator result binding must be named");

    // Every applied-base shape must keep its name.
    assert!(
        names_persistence(&source(OLD_ALIAS_REUSE)),
        "77-family post-read-alias reuse base must stay named"
    );
    assert!(
        names_persistence(&source(OLD_DECLARATOR_RESULT)),
        "6-family declarator-result base must stay named"
    );
}

#[test]
fn persistence_variants_refuse_broken_state_flow() {
    let base = source(NEW_SELF_REASSIGN);
    assert!(names_persistence(&base), "new-family BASE must be named");

    let mutations: Vec<(&str, String)> = vec![
        (
            "upload key changed",
            mutate(
                &base,
                "setItem(\"storage-key\"",
                "setItem(\"other-key\"",
            ),
        ),
        (
            "parsed Object.keys bound weakened",
            mutate(
                &base,
                "Object.keys(parsed).length > 0",
                "Object.keys(parsed).length >= 0",
            ),
        ),
        (
            "promise catch replaced by then",
            mutate(&base, "}).catch(error => {", "}).then(error => {"),
        ),
        (
            "parse error record loses its stack field",
            mutate(
                &base,
                "errorSink({ message: \"parse\", stack: parseError + \"\" });",
                "errorSink({ message: \"parse\" });",
            ),
        ),
        (
            "wrong overwrite before the actual encryptor use",
            mutate(
                &base,
                "let text = JSON.stringify(telemetryState);\n            text = encryptString(text);",
                "let text = JSON.stringify(telemetryState);\n            text = \"wrong\";\n            text = encryptString(text);",
            ),
        ),
        (
            "later-only stringify donor",
            mutate(
                &base,
                "let text = JSON.stringify(telemetryState);\n            text = encryptString(text);",
                "let text;\n            text = encryptString(text);\n            text = JSON.stringify(telemetryState);",
            ),
        ),
        (
            "branch-only stringify donor",
            mutate(
                &base,
                "let text = JSON.stringify(telemetryState);\n            text = encryptString(text);",
                "let text;\n            if (telemetryState) {\n                text = JSON.stringify(telemetryState);\n            }\n            text = encryptString(text);",
            ),
        ),
        (
            "captured/foreign serializer temporary",
            {
                let captured = mutate(
                    &base,
                    "let text = JSON.stringify(telemetryState);\n            text = encryptString(text);\n            if (text != null) {",
                    "let encryptedResult = encryptString(capturedText);\n            if (encryptedResult != null) {",
                );
                mutate(
                    &captured,
                    "                text.then(value => {",
                    "                encryptedResult.then(value => {",
                )
            },
        ),
        (
            "third non-proven write",
            mutate(
                &base,
                "text = encryptString(text);",
                "text = encryptString(text);\n            text = \"third\";",
            ),
        ),
        (
            "different upload storage receiver",
            mutate(
                &base,
                "globalObject.localStorage.setItem(\"storage-key\"",
                "otherObject.localStorage.setItem(\"storage-key\"",
            ),
        ),
        (
            "upload stringifies a value not tied to the parsed map",
            mutate(
                &base,
                "JSON.stringify(storedReports)",
                "JSON.stringify(capturedText)",
            ),
        ),
        (
            "error sink declared inside the persistence function",
            mutate(
                &base,
                "    function $THREAD_1_100() {",
                "    function $THREAD_1_100() {\n        function errorSink(error) { return error; }",
            ),
        ),
    ];

    for (label, changed) in mutations {
        assert!(
            !names_persistence(&changed),
            "accepted broken persistence flow: {label}"
        );
    }
}
