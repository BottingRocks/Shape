//! Payload-only report boundary. Permanent proofs use public synthetic fixtures.
use serde::Deserialize;
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};
use shape::signals::payload::{
    report::Report,
    superpack::{Number, Value},
    PayloadCodec, PayloadProfile,
};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
};

fn sha(bytes: impl AsRef<[u8]>) -> String {
    Sha256::digest(bytes.as_ref())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn trusted_json<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> T {
    // Only trusted fixture/CLI outputs, not a production parser. Deep outputs
    // must never be read with serde_json's accidental default recursion cap.
    let mut d = serde_json::Deserializer::from_slice(bytes);
    d.disable_recursion_limit();
    let value = T::deserialize(&mut d).unwrap();
    d.end().unwrap();
    value
}
#[derive(Deserialize)]
struct Fixtures {
    profile: PayloadProfile,
    answers: Vec<Answer>,
}
#[derive(Deserialize)]
struct Answer {
    name: String,
    report: Report,
    payload_sha256: String,
    plaintext_sha256: String,
    base64: Option<String>,
}
fn fixtures() -> Fixtures {
    trusted_json(include_bytes!(
        "fixtures/generator/report-known-answers.json"
    ))
}
fn empty() -> Report {
    Report {
        errors: vec![],
        positional_signals: vec![],
        signals: vec![],
        keyed_signals: vec![],
        seeds: [1200151530, 1200151531],
        keysets: vec![],
    }
}
fn codec() -> PayloadCodec {
    PayloadCodec::new(fixtures().profile).unwrap()
}

struct Dir(PathBuf);
impl Dir {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "shape-report-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let dir = Self(path);
        dir.write(
            "profile.json",
            &serde_json::to_vec(&fixtures().profile).unwrap(),
        );
        dir.write("keysets.json", b"[]");
        dir
    }
    fn write(&self, name: &str, bytes: &[u8]) {
        fs::write(self.0.join(name), bytes).unwrap();
    }
    fn run(&self, args: &[&str], input: &[u8]) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_shape"))
            .current_dir(&self.0)
            .args(["gen", "payload"])
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        child.wait_with_output().unwrap()
    }
    fn encode(&self, report: &[u8]) -> Output {
        self.run(&["encode", "--profile", "profile.json"], report)
    }
    fn decode(&self, text: &[u8], count: &str) -> Output {
        self.run(
            &[
                "decode",
                "--profile",
                "profile.json",
                "--keysets",
                "keysets.json",
                "--positional-count",
                count,
            ],
            text,
        )
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn ok(output: Output) -> Vec<u8> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    output.stdout
}
fn fails(output: Output, diagnostic: &str) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(diagnostic),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn source_factory_fixed_answers_always_run_and_roundtrip() {
    shape::run_with_large_stack(|| {
        let fixture = fixtures();
        assert_eq!(fixture.answers.len(), 53);
        let codec = PayloadCodec::new(fixture.profile).unwrap();
        for a in fixture.answers {
            if let Some((site, length)) = a.name.rsplit_once("-length-") {
                let expected: usize = length.parse().unwrap();
                let shape::signals::payload::Data::Container { container } =
                    a.report.to_payload().unwrap().data
                else {
                    unreachable!()
                };
                let actual = match site {
                    "errors" => container.errors.len(),
                    "signals" => container.signals.len(),
                    "positional" => container.positional[0].as_ref().unwrap().encoded.len(),
                    "keyed" => container.keyed[0].encoded.len(),
                    "positional-extra" => container.positional[0].as_ref().unwrap().extra.len(),
                    "keyed-extra" => container.keyed[0].extra.len(),
                    _ => panic!("unknown boundary site"),
                };
                assert_eq!(actual, expected, "{} exact ULEB boundary", a.name);
            }
            let payload = codec
                .encode_report(&a.report)
                .unwrap_or_else(|e| panic!("{}: {e}", a.name));
            assert_eq!(
                sha(&payload),
                a.payload_sha256,
                "{} finalized payload",
                a.name
            );
            if let Some(literal) = a.base64 {
                assert_eq!(payload, literal, "{} literal", a.name);
            }
            assert_eq!(
                sha(codec.decode(&payload).unwrap().plaintext().unwrap()),
                a.plaintext_sha256,
                "{} plaintext",
                a.name
            );
            let recovered = codec
                .decode_report(
                    &payload,
                    a.report.positional_signals.len(),
                    a.report.keysets.clone(),
                )
                .unwrap_or_else(|e| panic!("{}: {e}", a.name));
            assert_eq!(recovered.seeds, a.report.seeds);
            assert_eq!(recovered.keysets, a.report.keysets);
            assert_eq!(
                codec.encode_report(&recovered).unwrap(),
                payload,
                "{} report roundtrip",
                a.name
            );
            assert_eq!(
                codec
                    .decode_report(
                        &codec.encode_report(&recovered).unwrap(),
                        recovered.positional_signals.len(),
                        recovered.keysets.clone()
                    )
                    .unwrap(),
                recovered
            );
        }
    });
}

#[test]
fn original_factory_integrity_and_optional_live_differential() {
    let provenance: Json =
        serde_json::from_slice(include_bytes!("fixtures/generator/report-provenance.json"))
            .unwrap();
    for (name, bytes) in [
        (
            "payload-factory.js",
            include_bytes!("fixtures/generator/payload-factory.js").as_slice(),
        ),
        (
            "payload-cipher.js",
            include_bytes!("fixtures/generator/payload-cipher.js").as_slice(),
        ),
        (
            "superpack-reference.cjs",
            include_bytes!("fixtures/generator/superpack-reference.cjs").as_slice(),
        ),
    ] {
        assert_eq!(sha(bytes), provenance["files"][name]);
    }
    match Command::new("node").arg("--version").output() {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("Node absent; fixed source answers still run");
            return;
        }
        Err(e) => panic!("Node: {e}"),
        Ok(output) => assert!(output.status.success()),
    }
    let output = Command::new("node")
        .args(["tests/fixtures/generator/report-vectors.cjs", "--check"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn report_cli_emits_bare_payload_and_four_source_fields_with_metadata() {
    shape::run_with_large_stack(|| {
        let dir = Dir::new();
        for a in fixtures().answers.into_iter().filter(|a| {
            a.name == "empty"
                || a.name == "four-fields-specials-raw"
                || a.name == "ordered-omitted-keysets"
                || a.name.starts_with("depth-100")
        }) {
            dir.write(
                "keysets.json",
                &serde_json::to_vec(&a.report.keysets).unwrap(),
            );
            let packed = ok(dir.encode(&serde_json::to_vec(&a.report).unwrap()));
            assert_eq!(sha(&packed[..packed.len() - 1]), a.payload_sha256);
            assert!(!packed.starts_with(b"\""));
            assert!(!packed.starts_with(b"{"));
            assert!(packed.ends_with(b"\n"));
            let decoded = ok(dir.decode(&packed, &a.report.positional_signals.len().to_string()));
            let document: Json = trusted_json(&decoded);
            let fields: Vec<_> = document
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect();
            assert_eq!(
                fields,
                vec![
                    "errors",
                    "keyedSignals",
                    "keysets",
                    "positionalSignals",
                    "seeds",
                    "signals"
                ]
            );
            assert!(document.get("data").is_none());
            assert_eq!(ok(dir.encode(&decoded)), packed);
            assert_eq!(
                ok(dir.decode(
                    &ok(dir.encode(&decoded)),
                    &a.report.positional_signals.len().to_string()
                )),
                decoded
            );
            let crlf = [packed[..packed.len() - 1].to_vec(), b"\r\n".to_vec()].concat();
            assert_eq!(
                ok(dir.decode(&crlf, &a.report.positional_signals.len().to_string())),
                decoded
            );
            assert_eq!(
                ok(dir.decode(
                    &packed[..packed.len() - 1],
                    &a.report.positional_signals.len().to_string()
                )),
                decoded
            );
        }
    });
}

#[test]
fn report_order_sentinels_raw_extras_and_special_values_are_preserved() {
    let mut report = empty();
    let special = vec![
        Value::Undefined,
        Value::Null,
        Value::from(0.0),
        Value::from(-0.0),
        Value::Number(Number(0x7ff8000000000000)),
        Value::from(f64::INFINITY),
        Value::from(f64::NEG_INFINITY),
    ];
    report.errors = special.clone();
    report.signals = special.clone();
    report.positional_signals = vec![None, Some((special.clone(), vec![255, 0, 128]))];
    report.keyed_signals = vec![
        (u32::MAX, None),
        (7, Some((special, vec![128, 0]))),
        (7, None),
        (0, None),
    ];
    let codec = codec();
    let text = codec.encode_report(&report).unwrap();
    assert_eq!(codec.decode_report(&text, 2, vec![]).unwrap(), report);
    let mut empty_entry = empty();
    empty_entry.positional_signals.push(Some((vec![], vec![])));
    assert!(codec
        .encode_report(&empty_entry)
        .unwrap_err()
        .to_string()
        .contains("ambiguous"));
    empty_entry.positional_signals.clear();
    empty_entry.keyed_signals.push((7, Some((vec![], vec![]))));
    assert!(codec
        .encode_report(&empty_entry)
        .unwrap_err()
        .to_string()
        .contains("ambiguous"));
}

#[test]
fn wrong_external_facts_and_noncanonical_channels_do_not_pretend_to_decode() {
    let codec = codec();
    let report = fixtures()
        .answers
        .into_iter()
        .find(|a| a.name == "ordered-omitted-keysets")
        .unwrap()
        .report;
    let text = codec.encode_report(&report).unwrap();
    assert!(codec
        .decode_report(&text, 0, vec![])
        .unwrap_err()
        .to_string()
        .contains("exact ordered keysets"));
    let mut report = empty();
    report.positional_signals.push(None);
    let text = codec.encode_report(&report).unwrap();
    assert!(codec.decode_report(&text, 0, vec![]).is_err());
    assert!(codec.decode_report(&text, 2, vec![]).is_err());
    assert!(codec.decode_report(&text, 65537, vec![]).is_err());
    for (raw, diagnostic) in [
        (vec![128, 0, 0], "noncanonical container"),
        (vec![4, 0xa1, 0xe4, 0, 1, 0], "noncanonical input"),
        (vec![1, 0xa0, 0], "noncanonical report channel"),
        (vec![1, 0xe2, 0], "must be arrays"),
        (vec![1, 0xa1, 0], "remaining input"),
        (vec![2, 0xa0, 0xe2, 0], "trailing"),
    ] {
        let text = codec.encode_bytes(&raw, [1, 2]).unwrap();
        let error = codec
            .decode_report(&text, 0, vec![])
            .unwrap_err()
            .to_string();
        assert!(error.contains(diagnostic), "expected {diagnostic}: {error}");
        assert_eq!(codec.decode(&text).unwrap().plaintext().unwrap(), raw);
    }
}

#[test]
fn aggregate_report_decode_budget_bounds_individually_valid_channels() {
    use shape::signals::payload::{
        superpack::{pack, SerializerOptions},
        Container, Data, Payload,
    };
    let codec = codec();
    let values = Value::Array(vec![Value::Null; 40_000]);
    let errors = pack(&values, &SerializerOptions::default()).unwrap();
    let signals = pack(&values, &SerializerOptions::outer_channel(vec![])).unwrap();
    let payload = Payload {
        seeds: [1, 2],
        data: Data::Container {
            container: Container {
                errors,
                signals,
                ..Default::default()
            },
        },
        header_context: None,
    };
    let text = codec
        .encode_bytes(&payload.plaintext().unwrap(), payload.seeds)
        .unwrap();
    assert!(codec
        .decode_report(&text, 0, vec![])
        .unwrap_err()
        .to_string()
        .contains("report generic values exceed"));
}

#[test]
fn payload_cli_strict_inputs_counts_keysets_whitespace_and_prefix() {
    let dir = Dir::new();
    let report = serde_json::to_vec(&empty()).unwrap();
    let packed = ok(dir.encode(&report));
    let alphabet = fixtures().profile.alphabet;
    let mut tail = packed[..packed.len() - 1].to_vec();
    let last = tail.len() - 1;
    let index = alphabet
        .as_bytes()
        .iter()
        .position(|b| *b == tail[last])
        .unwrap();
    tail[last] = alphabet.as_bytes()[index ^ 1];
    fails(dir.decode(&tail, "0"), "canonical");
    for text in [b"".as_slice(), b"AAA", b"!invalid", b" ", b"AA AA", b"A\nA"] {
        fails(dir.decode(text, "0"), "shape payload:");
    }
    for suffix in [b"\n\n".as_slice(), b"\r", b" ", b"\t"] {
        let text = [packed[..packed.len() - 1].to_vec(), suffix.to_vec()].concat();
        fails(dir.decode(&text, "0"), "shape payload:");
    }
    fails(
        dir.run(
            &[
                "decode",
                "--profile",
                "profile.json",
                "--keysets",
                "keysets.json",
            ],
            &packed,
        ),
        "--positional-count",
    );
    fails(
        dir.run(
            &[
                "decode",
                "--profile",
                "profile.json",
                "--positional-count",
                "0",
            ],
            &packed,
        ),
        "--keysets",
    );
    for name in [
        "errors",
        "positionalSignals",
        "signals",
        "keyedSignals",
        "seeds",
        "keysets",
    ] {
        let mut value: Json = serde_json::from_slice(&report).unwrap();
        value.as_object_mut().unwrap().remove(name);
        fails(
            dir.encode(&serde_json::to_vec(&value).unwrap()),
            "missing field",
        );
    }
    let base = String::from_utf8(report.clone()).unwrap();
    let duplicate = base.replacen("\"errors\":[]", "\"errors\":[],\"errors\":[]", 1);
    for bad in [
        duplicate,
        format!("{base} garbage"),
        base[..base.len() - 1].into(),
        base.replacen("\"errors\":[]", "\"errors\":{}", 1),
        base.replacen("\"errors\":[]", "\"errors\":[],\"unknown\":1", 1),
    ] {
        fails(dir.encode(bad.as_bytes()), "report JSON");
    }
    for name in [-1i64, 4294967296] {
        let mut value: Json = serde_json::from_slice(&report).unwrap();
        value["keyedSignals"] = json!([[name, null]]);
        fails(
            dir.encode(&serde_json::to_vec(&value).unwrap()),
            "report JSON",
        );
    }
    let mut value: Json = serde_json::from_slice(&report).unwrap();
    value["positionalSignals"] = json!([[[], [256]]]);
    fails(
        dir.encode(&serde_json::to_vec(&value).unwrap()),
        "report JSON",
    );
    dir.write("keysets.json", b"[[\"x\",\"x\"]]");
    fails(dir.decode(&packed, "0"), "duplicate preloaded key");
    dir.write("keysets.json", b"{} junk");
    fails(dir.decode(&packed, "0"), "keysets JSON");
}

#[test]
fn payload_cli_structural_depth_and_resource_limits_are_finite() {
    shape::run_with_large_stack(|| {
        let dir = Dir::new();
        let mut report = empty();
        let mut value = Value::Null;
        for _ in 0..100 {
            value = Value::Array(vec![value]);
        }
        report.signals.push(value);
        let packed = ok(dir.encode(&serde_json::to_vec(&report).unwrap()));
        assert_eq!(ok(dir.encode(&ok(dir.decode(&packed, "0")))), packed);
        // Report + signals array + 509 arrays + one object + null = 1024.
        let mut exact = Value::Object(vec![("x".into(), Value::Null)]);
        for _ in 0..509 {
            exact = Value::Array(vec![exact]);
        }
        report.signals = vec![exact];
        let packed = ok(dir.encode(&serde_json::to_vec(&report).unwrap()));
        assert_eq!(ok(dir.encode(&ok(dir.decode(&packed, "0")))), packed);
        // Report + signals array + 511 arrays + null = 1025, one past.
        let mut over = Value::Null;
        for _ in 0..511 {
            over = Value::Array(vec![over]);
        }
        report.signals = vec![over];
        fails(
            dir.encode(&serde_json::to_vec(&report).unwrap()),
            "JSON nesting exceeds 1024",
        );
        let hostile = format!("{}0{}", "[".repeat(100_000), "]".repeat(100_000));
        fails(
            dir.run(
                &["encode", "--profile", "profile.json", "--out", "new.json"],
                hostile.as_bytes(),
            ),
            "JSON nesting exceeds 1024",
        );
        assert!(!dir.0.join("new.json").exists());
        let codec = codec();
        let mut report = empty();
        report.positional_signals = vec![None; 65537];
        assert!(codec
            .encode_report(&report)
            .unwrap_err()
            .to_string()
            .contains("entry limit"));
        report.positional_signals.clear();
        report.errors = vec![Value::Null; 65537];
        assert!(codec
            .encode_report(&report)
            .unwrap_err()
            .to_string()
            .contains("entry limit"));
        report.errors = vec![Value::String("x".repeat(16 * 1024 * 1024 + 1))];
        assert!(codec
            .encode_report(&report)
            .unwrap_err()
            .to_string()
            .contains("16 MiB"));
        report.errors = vec![];
        let mut value = Value::Null;
        for _ in 0..129 {
            value = Value::Array(vec![value]);
        }
        report.errors.push(value);
        assert!(codec
            .encode_report(&report)
            .unwrap_err()
            .to_string()
            .contains("depth limit"));
        fs::File::create(dir.0.join("huge.json"))
            .unwrap()
            .set_len(128 * 1024 * 1024 + 1)
            .unwrap();
        fails(
            dir.run(
                &[
                    "encode",
                    "--profile",
                    "profile.json",
                    "--input",
                    "huge.json",
                    "--out",
                    "new.json",
                ],
                b"",
            ),
            "JSON input exceeds",
        );
        fs::File::create(dir.0.join("huge-profile.json"))
            .unwrap()
            .set_len(64 * 1024 + 1)
            .unwrap();
        fails(
            dir.run(&["encode", "--profile", "huge-profile.json"], b"{}"),
            "JSON input exceeds",
        );
        assert!(!dir.0.join("new.json").exists());
    });
}

#[test]
fn payload_cli_files_aliases_no_clobber_and_legacy_crypto_profile() {
    let dir = Dir::new();
    let report = serde_json::to_vec(&empty()).unwrap();
    dir.write("report.json", &report);
    dir.write("out.txt", b"keep");
    fails(
        dir.run(
            &[
                "encode",
                "--profile",
                "profile.json",
                "--input",
                "report.json",
                "--out",
                "out.txt",
            ],
            b"",
        ),
        "already exists",
    );
    ok(dir.run(
        &[
            "encode",
            "--profile",
            "profile.json",
            "--input",
            "report.json",
            "--out",
            "out.txt",
            "--force",
        ],
        b"",
    ));
    let packed = fs::read(dir.0.join("out.txt")).unwrap();
    for out in ["report.json", "profile.json"] {
        fails(
            dir.run(
                &[
                    "encode",
                    "--profile",
                    "profile.json",
                    "--input",
                    "report.json",
                    "--out",
                    out,
                    "--force",
                ],
                b"",
            ),
            "alias",
        );
    }
    fails(
        dir.run(
            &[
                "decode",
                "--profile",
                "profile.json",
                "--input",
                "out.txt",
                "--keysets",
                "keysets.json",
                "--positional-count",
                "0",
                "--out",
                "keysets.json",
                "--force",
            ],
            b"",
        ),
        "alias",
    );
    dir.write("bad.txt", b"not a payload");
    fails(
        dir.run(
            &[
                "decode",
                "--profile",
                "profile.json",
                "--input",
                "bad.txt",
                "--keysets",
                "keysets.json",
                "--positional-count",
                "0",
                "--out",
                "out.txt",
                "--force",
            ],
            b"",
        ),
        "shape payload:",
    );
    assert_eq!(fs::read(dir.0.join("out.txt")).unwrap(), packed);
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("report.json", dir.0.join("sym.json")).unwrap();
        fs::hard_link(dir.0.join("profile.json"), dir.0.join("hard.json")).unwrap();
        for out in ["sym.json", "hard.json"] {
            fails(
                dir.run(
                    &[
                        "encode",
                        "--profile",
                        "profile.json",
                        "--input",
                        "report.json",
                        "--out",
                        out,
                        "--force",
                    ],
                    b"",
                ),
                "alias",
            );
        }
    }
    let mut profile: Json =
        serde_json::from_slice(&fs::read(dir.0.join("profile.json")).unwrap()).unwrap();
    profile["header_name"] = json!("legacy metadata, not validated for payloads");
    profile["header_context"] = json!({"b":true,"f":true});
    dir.write("profile.json", &serde_json::to_vec(&profile).unwrap());
    assert_eq!(ok(dir.encode(&report)), packed);
}

fn metadata_profile(report: &Report) -> shape::signals::payload::Profile {
    let crypto = fixtures().profile;
    let mut p: shape::signals::payload::Profile =
        serde_json::from_str(include_str!("fixtures/sdk/generator/profile.json")).unwrap();
    p.format = crypto.format;
    p.cipher = crypto.cipher;
    p.alphabet = crypto.alphabet;
    p.keys = crypto.keys;
    p.positional_count = Some(report.positional_signals.len());
    p.keysets = Some(report.keysets.clone());
    p
}

#[test]
fn cli_profile_decode_metadata_resolves_each_field_and_explicit_empty_values() {
    let dir = Dir::new();
    for name in [
        "ordered-omitted-keysets",
        "empty",
        "four-fields-specials-raw",
    ] {
        let answer = fixtures()
            .answers
            .into_iter()
            .find(|a| a.name == name)
            .unwrap();
        let packed = answer.base64.as_ref().unwrap().as_bytes();
        let expected = ok(dir.encode(&serde_json::to_vec(&answer.report).unwrap()));
        assert_eq!(&expected[..expected.len() - 1], packed);
        dir.write(
            "keysets.json",
            &serde_json::to_vec(&answer.report.keysets).unwrap(),
        );
        let count = answer.report.positional_signals.len().to_string();
        // Defaults alone, each partial default plus its opposite flag, and
        // overrides of both conflicting defaults (including zero and []).
        for case in 0..6 {
            let mut p = metadata_profile(&answer.report);
            let mut args = vec!["decode", "--profile", "profile.json"];
            match case {
                0 => {}
                1 => {
                    p.keysets = None;
                    args.extend(["--keysets", "keysets.json"]);
                }
                2 => {
                    p.positional_count = None;
                    args.extend(["--positional-count", &count]);
                }
                3 => {
                    p.positional_count = Some(999);
                    args.extend(["--positional-count", &count]);
                }
                4 => {
                    p.keysets = Some(vec![vec!["conflicting".into()]]);
                    args.extend(["--keysets", "keysets.json"]);
                }
                _ => {
                    p.positional_count = Some(999);
                    p.keysets = Some(vec![vec!["conflicting".into()]]);
                    args.extend(["--positional-count", &count, "--keysets", "keysets.json"]);
                }
            }
            dir.write("profile.json", &serde_json::to_vec(&p).unwrap());
            let decoded: Report = trusted_json(&ok(dir.run(&args, packed)));
            assert_eq!(decoded.keysets, answer.report.keysets);
            assert_eq!(
                decoded.positional_signals.len(),
                answer.report.positional_signals.len()
            );
            assert_eq!(codec().encode_report(&decoded).unwrap().as_bytes(), packed);
        }
    }
}

#[test]
fn cli_profile_four_channels_use_ordered_keysets_and_full_report_stays_authoritative() {
    let dir = Dir::new();
    let answer = fixtures()
        .answers
        .into_iter()
        .find(|a| a.name == "ordered-omitted-keysets")
        .unwrap();
    let packed = format!("{}\n", answer.base64.as_ref().unwrap()).into_bytes();
    let channels = serde_json::to_vec(&answer.report.channels()).unwrap();
    let seeds = format!("{},{}", answer.report.seeds[0], answer.report.seeds[1]);
    let mut p = metadata_profile(&answer.report);
    dir.write("profile.json", &serde_json::to_vec(&p).unwrap());
    assert_eq!(
        ok(dir.run(
            &["encode", "--profile", "profile.json", "--seeds", &seeds],
            &channels
        )),
        packed
    );
    let decoded: Report = trusted_json(&ok(
        dir.run(&["decode", "--profile", "profile.json"], &packed)
    ));
    assert_eq!(decoded.keysets, p.keysets.clone().unwrap());
    // Repeated preloads occupy rows, so new keyset indices shift. Preserve that
    // behavior and the exact table, rather than expecting the old golden bytes.
    let mut repeated = answer.report.clone();
    repeated.keysets.push(repeated.keysets[0].clone());
    p.keysets = Some(repeated.keysets.clone());
    dir.write("profile.json", &serde_json::to_vec(&p).unwrap());
    let repeated_packed = ok(dir.run(
        &["encode", "--profile", "profile.json", "--seeds", &seeds],
        &channels,
    ));
    assert_ne!(repeated_packed, packed);
    assert_eq!(
        repeated_packed,
        ok(dir.encode(&serde_json::to_vec(&repeated).unwrap()))
    );
    let decoded: Report = trusted_json(&ok(dir.run(
        &["decode", "--profile", "profile.json"],
        &repeated_packed,
    )));
    assert_eq!(decoded.keysets, repeated.keysets);
    p.keysets = Some(vec![vec!["conflicting".into()]]);
    p.positional_count = Some(999);
    dir.write("profile.json", &serde_json::to_vec(&p).unwrap());
    assert_eq!(
        ok(dir.encode(&serde_json::to_vec(&answer.report).unwrap())),
        packed
    );
}

#[test]
fn cli_absent_encode_keysets_keep_legacy_empty_default_but_decode_requires_facts() {
    let dir = Dir::new();
    let answer = fixtures()
        .answers
        .into_iter()
        .find(|a| a.name == "empty")
        .unwrap();
    let channels = serde_json::to_vec(&answer.report.channels()).unwrap();
    let seeds = format!("{},{}", answer.report.seeds[0], answer.report.seeds[1]);
    for full in [false, true] {
        if full {
            let mut p = metadata_profile(&answer.report);
            p.keysets = None;
            p.positional_count = None;
            dir.write("profile.json", &serde_json::to_vec(&p).unwrap());
        }
        let packed = ok(dir.run(
            &["encode", "--profile", "profile.json", "--seeds", &seeds],
            &channels,
        ));
        assert_eq!(
            packed,
            format!("{}\n", answer.base64.as_ref().unwrap()).as_bytes()
        );
        fails(
            dir.run(
                &[
                    "decode",
                    "--profile",
                    "profile.json",
                    "--positional-count",
                    "0",
                ],
                &packed,
            ),
            "--keysets",
        );
        fails(
            dir.run(
                &[
                    "decode",
                    "--profile",
                    "profile.json",
                    "--keysets",
                    "keysets.json",
                ],
                &packed,
            ),
            "--positional-count",
        );
        ok(dir.decode(&packed, "0"));
    }
}

#[test]
fn cli_profile_metadata_keeps_count_limits_and_keyset_validation() {
    let dir = Dir::new();
    let answer = fixtures()
        .answers
        .into_iter()
        .find(|a| a.name == "empty")
        .unwrap();
    let packed = answer.base64.unwrap();
    let mut p = metadata_profile(&answer.report);
    // Profile counts must fit usize before count validation or CLI overrides run.
    for count in std::iter::once(shape::signals::payload::MAX_CONTAINER_ENTRIES + 1)
        .chain(usize::try_from(u64::from(u32::MAX) + 1).ok())
    {
        p.positional_count = Some(count);
        dir.write("profile.json", &serde_json::to_vec(&p).unwrap());
        fails(
            dir.run(&["decode", "--profile", "profile.json"], packed.as_bytes()),
            "positional count",
        );
        ok(dir.run(
            &[
                "decode",
                "--profile",
                "profile.json",
                "--positional-count",
                "0",
            ],
            packed.as_bytes(),
        ));
    }
    p.positional_count = Some(0);
    p.keysets = Some(vec![vec!["duplicate".into(), "duplicate".into()]]);
    dir.write("profile.json", &serde_json::to_vec(&p).unwrap());
    fails(
        dir.run(&["decode", "--profile", "profile.json"], packed.as_bytes()),
        "duplicate",
    );
    let channels = serde_json::to_vec(&answer.report.channels()).unwrap();
    fails(
        dir.run(
            &[
                "encode",
                "--profile",
                "profile.json",
                "--seeds",
                "1200151530,1200151531",
            ],
            &channels,
        ),
        "duplicate",
    );
}
