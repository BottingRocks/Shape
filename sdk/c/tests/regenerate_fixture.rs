//! Regenerate the public C ABI fixtures, with no Python or private capture:
//! cargo test --release -p shape-c --test regenerate_fixture -- --ignored
//! Uses the same synthetic writer setup as tests/decode_workflow.rs.
use engine::signals::{
    codec::{encode_signal, hash_string_token},
    decode::{CaptureHeaders, DecodeArtifact},
    payload::{superpack::Value as Raw, Headers, Report},
};
use serde_json::{json, Value};
use std::{fs, path::Path};

#[test]
#[ignore = "explicitly regenerate committed C ABI fixtures"]
fn regenerate_fixture() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let reference = root.join("src/signals/wire/reference");
    let source = fs::read_to_string(root.join("tests/fixtures/signal-wire-roundtrip/program.js"))
        .unwrap()
        .replace(
            "__HASH__",
            &fs::read_to_string(reference.join("hash.js"))
                .unwrap()
                .replace("local0(", "tokenHash("),
        )
        .replace(
            "__STREAM__",
            &fs::read_to_string(reference.join("stream.js"))
                .unwrap()
                .replace("local0(", "streamFactory(")
                .replace("local9", "tokenState"),
        );
    let writer = fs::read_to_string(reference.join("number.js"))
        .unwrap()
        .replace("local0(", "integerWriter(");
    // The third signal is a constant STRING. It needs the reviewed modified-UTF-8
    // writer, its integer dependency and the certified character accessor.
    let string_writer = fs::read_to_string(reference.join("string.js"))
        .unwrap()
        .replace("local0(", "stringFactory(")
        .replace("local10(", "integerWriter(")
        .replace("local15(", "codeUnit(");
    let helpers = format!(
        "{writer}\n{string_writer}\nlet codeUnit=Array.call.bind(String.prototype.charCodeAt);"
    );
    let source = source.replacen("function main() {", &format!("function main() {{\n{helpers}"), 1)
        .replace("function Session(token) {", "function Session(token) {\nlet sharedNumber; sharedNumber=integerWriter; const navigator={deviceMemory:32};")
        .replace("b[4]=9^mask();", "let callable; callable=sharedNumber; callable=callable(mask,navigator.deviceMemory,b,3); callable=0;")
        .replace(
            "let registry=[{ctor:task,priority:false},{ctor:literal,priority:false}];",
            "let text=wrap(function(o) {\n    let seed=Math.random()*4294967296|0;\n    let mask=streamFactory(seed^17);\n    let b=[seed>>>24,seed>>16&255,seed>>8&255,seed&255];\n    let write=stringFactory(mask);\n    write(\"\",b,3);\n    o.next([[],b]);\n  });\n  let registry=[{ctor:task,priority:false},{ctor:literal,priority:false},{ctor:text,priority:false}];",
        )
        .replace("let filter=[0,1];", "let filter=[0,1,2];");
    let session: Value = serde_json::from_str(
        &fs::read_to_string(root.join("tests/fixtures/sdk/decode/session.json")).unwrap(),
    )
    .unwrap();
    let artifact = engine::sdk::call(
        "artifact.compile",
        json!({"source": source, "session": session, "site_id": "example"}),
    )
    .unwrap();
    // Fresh named encoding intentionally chooses random signal prefixes. Use
    // fixed prefixes and outer seeds here so regeneration is byte-for-byte
    // deterministic and the saved capture is independent of the named encoder.
    let typed: DecodeArtifact = serde_json::from_value(artifact.clone()).unwrap();
    let token = hash_string_token(&typed.layout.session.transaction_token);
    let report = Report {
        errors: vec![Raw::Null],
        positional_signals: vec![
            Some((vec![], encode_signal(token, 0x01020304, 17, |w| w.byte(1)))),
            Some((
                vec![],
                encode_signal(token, 0x01020304, 17, |w| w.number(32.0)),
            )),
            Some((vec![], encode_signal(token, 0x01020304, 17, |w| w.string("")))),
        ],
        signals: vec![Raw::Bool(true)],
        keyed_signals: vec![(17, None)],
        seeds: [19, 23],
        keysets: vec![],
    };
    let headers = typed
        .layout
        .encode_headers(&report.channels_owned(), report.seeds)
        .unwrap();
    let capture = serde_json::to_value(CaptureHeaders {
        site_id: typed.site_id,
        headers: Headers::from_entries(headers.iter().map(|(name, value)| {
            (
                name.strip_prefix(&typed.layout.header.prefix)
                    .unwrap()
                    .to_owned(),
                value.to_owned(),
            )
        }))
        .unwrap(),
    })
    .unwrap();
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for (name, value) in [("artifact", artifact), ("capture", capture)] {
        let path = fixtures.join(format!("{name}.json"));
        fs::write(
            &path,
            format!("{}\n", serde_json::to_string(&value).unwrap()),
        )
        .unwrap();
        println!("{}", path.display());
    }
}
