//! Portable synthetic header/report proofs, reusing the source-factory answers.
use serde::Deserialize;
use shape::signals::payload::{Generator, HeaderContext, HeaderReport, Headers, PayloadCodec, Profile};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
};

fn profile() -> Profile {
    let mut p: Profile =
        serde_json::from_str(include_str!("fixtures/sdk/generator/profile.json")).unwrap();
    p.transaction_token = Some("synthetic-header-report-token".into());
    p.chunk_size = Some(7);
    p
}
fn context() -> HeaderContext {
    HeaderContext {
        b: true,
        f: true,
        c: Some(String::new()),
        d: Some("opaque:descriptor==".into()),
        z: Some("synthetic".into()),
    }
}
fn report() -> HeaderReport {
    HeaderReport::from_json(include_bytes!("fixtures/sdk/generator/report.json")).unwrap()
}
fn deep_json<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> T {
    let mut d = serde_json::Deserializer::from_slice(bytes);
    d.disable_recursion_limit();
    let value = T::deserialize(&mut d).unwrap();
    d.end().unwrap();
    value
}
fn large_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn source_fixture_reports_headers_and_finalized_payloads_share_one_boundary() {
    large_stack(|| {
        #[derive(Deserialize)]
        struct Fixtures {
            profile: shape::signals::payload::PayloadProfile,
            answers: Vec<Answer>,
        }
        #[derive(Deserialize)]
        struct Answer {
            report: shape::signals::payload::report::Report,
        }
        let fixture: Fixtures = deep_json(include_bytes!(
            "fixtures/generator/report-known-answers.json"
        ));
        let payload_codec = PayloadCodec::new(fixture.profile.clone()).unwrap();
        let mut p = profile();
        p.format = fixture.profile.format;
        p.cipher = fixture.profile.cipher;
        p.alphabet = fixture.profile.alphabet;
        p.keys = fixture.profile.keys;
        p.chunk_size = Some(7900);
        let codec = Generator::new(p).unwrap();
        for answer in fixture.answers {
            let mut input: HeaderReport = answer.report.into();
            input.header_context = Some(context());
            let payload = payload_codec.encode_report(&input.report()).unwrap();
            let headers = codec.encode_report_headers(&input).unwrap();
            assert_eq!(
                headers,
                codec
                    .encode_finalized_headers(&payload, input.header_context.as_ref())
                    .unwrap()
            );
            let (joined, metadata) = codec.decode_finalized_headers(&headers).unwrap();
            assert_eq!(joined, payload);
            assert_eq!(metadata, context());
            for headers in [
                headers.clone(),
                Headers::from_text(&headers.to_text()).unwrap(),
                Headers::from_text(&headers.to_text().replace('\n', "\r\n")).unwrap(),
            ] {
                let decoded = codec
                    .decode_report_headers(
                        &headers,
                        input.positional_signals.len(),
                        input.keysets.clone(),
                    )
                    .unwrap();
                let parsed =
                    HeaderReport::from_json(&serde_json::to_vec(&decoded).unwrap()).unwrap();
                assert_eq!(codec.encode_report_headers(&parsed).unwrap(), headers);
                // Source normalizations (depth 100 etc.) are inherited from Report.
                assert_eq!(
                    parsed.report(),
                    payload_codec
                        .decode_report(
                            &payload,
                            input.positional_signals.len(),
                            input.keysets.clone()
                        )
                        .unwrap()
                );
            }
        }
    });
}

#[test]
fn numeric_chunks_metadata_presence_absence_and_empty_opaque_values() {
    let codec = Generator::new(profile()).unwrap();
    for ctx in [
        HeaderContext::default(),
        context(),
        HeaderContext {
            c: Some(String::new()),
            ..Default::default()
        },
    ] {
        let mut input = report();
        input.header_context = Some(ctx.clone());
        let headers = codec.encode_report_headers(&input).unwrap();
        assert!(headers.get("x-example-a10").is_some());
        let decoded = codec
            .decode_report_headers(
                &headers,
                input.positional_signals.len(),
                input.keysets.clone(),
            )
            .unwrap();
        assert_eq!(decoded.header_context, Some(ctx));
        assert_eq!(codec.encode_report_headers(&decoded).unwrap(), headers);
    }
    let mut p = profile();
    p.header_context = Some(context());
    let codec = Generator::new(p).unwrap();
    let plain = report();
    let headers = codec.encode_report_headers(&plain).unwrap();
    let mut bad = plain;
    bad.header_context = Some(HeaderContext::default());
    assert!(codec.encode_report_headers(&bad).is_err());
    assert!(codec
        .encode_finalized_headers("invalid", Some(&HeaderContext::default()))
        .is_err());
    assert_eq!(
        codec.decode_finalized_headers(&headers).unwrap().1,
        context()
    );
}

#[test]
fn raw_text_preserves_interior_bytes_and_refuses_malformed_blocks() {
    let parsed = Headers::from_text("X-example-c:\t a:b==c \t\r\nX-example-d: \t\r\n\r\n").unwrap();
    assert_eq!(parsed.get("x-EXAMPLE-c"), Some("a:b==c"));
    assert_eq!(parsed.get("x-example-d"), Some(""));
    for bad in [
        "",
        "GET / HTTP/1.1\n",
        "HTTP/1.1 200 OK\n",
        "x-a : v\n",
        " x-a: v\n",
        "x-a: v\n folded",
        "x-a: v\n\nbody",
        "x-a:v\n\n\n",
        "x-a:v\r",
        "x-a:v\r\nx-b:v\n",
        "x-a:v\nx-A:w",
        "x-a:v\0",
        "x-a:v\tinside",
        "x-a:v\n:bad",
        "x-a:v\n\nx-b:w",
    ] {
        assert!(Headers::from_text(bad).is_err(), "{bad:?}");
    }
    assert!(Headers::from_text(&format!(
        "x-a:{}",
        "x".repeat(shape::signals::payload::MAX_HEADER_BYTES + 1)
    ))
    .is_err());
}

#[test]
fn strict_header_checks_run_before_report_decoding() {
    let codec = Generator::new(profile()).unwrap();
    let mut input = report();
    input.header_context = Some(context());
    let good = codec.encode_report_headers(&input).unwrap();
    for (name, value) in [
        ("X-example-b", "bad"),
        ("X-example-f", "bad"),
        ("X-example-a00", "x"),
        ("X-example-e", "x"),
        ("x-other-a", "x"),
        ("Host", "example.invalid"),
        ("X-example-a0", "a a"),
    ] {
        let mut entries: Vec<_> = good
            .iter()
            .filter(|(n, _)| !n.eq_ignore_ascii_case(name))
            .map(|(n, v)| (n.to_owned(), v.to_owned()))
            .collect();
        entries.push((name.into(), value.into()));
        let bad = Headers::from_entries(entries).unwrap();
        assert!(codec.decode_report_headers(&bad, 0, vec![]).is_err());
    }
    let bad = Headers::from_entries(
        good.iter()
            .filter(|(n, _)| !n.eq_ignore_ascii_case("X-example-a2"))
            .map(|(n, v)| (n.into(), v.into())),
    )
    .unwrap();
    assert!(codec.decode_finalized_headers(&bad).is_err());
    for value in ["", "bad payload", "AAAAAAAAAAB", "AAAAAAAAAAA\n"] {
        assert!(codec.encode_finalized_headers(value, None).is_err());
    }
}

struct Dir(PathBuf);
impl Dir {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = Self(std::env::temp_dir().join(format!(
            "shape-header-report-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        )));
        fs::create_dir(&dir.0).unwrap();
        dir.write("profile.json", &serde_json::to_vec(&profile()).unwrap());
        dir.write("keysets.json", b"[]");
        dir
    }
    fn write(&self, name: &str, data: &[u8]) {
        fs::write(self.0.join(name), data).unwrap();
    }
    fn run(&self, args: &[&str], input: &[u8]) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_shape"))
            .current_dir(&self.0)
            .args(["gen", "headers"])
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        child.wait_with_output().unwrap()
    }
    fn decode(&self, data: &[u8]) -> Output {
        self.run(
            &[
                "decode",
                "--profile",
                "profile.json",
                "--keysets",
                "keysets.json",
                "--positional-count",
                "3",
            ],
            data,
        )
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn ok(o: Output) -> Vec<u8> {
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    o.stdout
}

#[test]
fn cli_report_json_lf_crlf_payload_and_direct_reencode() {
    let dir = Dir::new();
    let mut p = profile();
    p.header_context = Some(context());
    dir.write("profile.json", &serde_json::to_vec(&p).unwrap());
    let encode = ["encode", "--profile", "profile.json"];
    let headers = ok(dir.run(&encode, include_bytes!("fixtures/sdk/generator/report.json")));
    let decoded = ok(dir.decode(&headers));
    let document: serde_json::Value = serde_json::from_slice(&decoded).unwrap();
    for key in [
        "errors",
        "positionalSignals",
        "signals",
        "keyedSignals",
        "seeds",
        "keysets",
        "header_context",
    ] {
        assert!(document.get(key).is_some());
    }
    assert_eq!(ok(dir.run(&encode, &decoded)), headers);
    let raw = ok(dir.run(
        &[
            "encode",
            "--profile",
            "profile.json",
            "--output-format",
            "text",
        ],
        &decoded,
    ));
    assert_eq!(ok(dir.decode(&raw)), decoded);
    assert_eq!(
        ok(dir.decode(
            String::from_utf8(raw)
                .unwrap()
                .replace('\n', "\r\n")
                .as_bytes()
        )),
        decoded
    );
    let payload = PayloadCodec::new(p.into())
        .unwrap()
        .encode_report(&report().report())
        .unwrap();
    assert_eq!(
        ok(dir.run(
            &[
                "encode",
                "--profile",
                "profile.json",
                "--input-format",
                "payload"
            ],
            payload.as_bytes()
        )),
        headers
    );
    // Metadata absent from the profile still survives decoded document -> encode.
    dir.write("profile.json", &serde_json::to_vec(&profile()).unwrap());
    assert_eq!(ok(dir.decode(&headers)), decoded);
    assert_eq!(ok(dir.run(&encode, &decoded)), headers);
}

#[test]
fn cli_rejects_unknown_duplicate_trailing_json_and_conflicting_context() {
    let dir = Dir::new();
    let input = serde_json::to_string(&report()).unwrap();
    for bad in [
        format!("{input} {{}}"),
        input.replacen('{', "{\"extra\":0,", 1),
        input.replacen('{', "{\"errors\":[],", 1),
        input.replacen('{', "{\"header_context\":{\"b\":false,\"b\":true},", 1),
    ] {
        assert!(!dir
            .run(&["encode", "--profile", "profile.json"], bad.as_bytes())
            .status
            .success());
    }
    for bad in [
        b"{\"x-a\":\"x\",\"X-A\":\"x\"}".as_slice(),
        b"{} {}",
        b"x-a:x\nx-A:x",
    ] {
        assert!(!dir.decode(bad).status.success());
    }
    let mut p = profile();
    p.header_context = Some(context());
    dir.write("profile.json", &serde_json::to_vec(&p).unwrap());
    let mut doc = report();
    doc.header_context = Some(HeaderContext::default());
    assert!(!dir
        .run(
            &["encode", "--profile", "profile.json"],
            &serde_json::to_vec(&doc).unwrap()
        )
        .status
        .success());
}

#[test]
fn cli_deep_report_parser_and_structural_limit() {
    large_stack(|| {
        let dir = Dir::new();
        let mut input = report();
        let mut deep = shape::signals::payload::superpack::Value::Null;
        for _ in 0..103 {
            deep = shape::signals::payload::superpack::Value::Array(vec![deep]);
        }
        input.signals = vec![deep];
        let encoded = ok(dir.run(
            &["encode", "--profile", "profile.json"],
            &serde_json::to_vec(&input).unwrap(),
        ));
        let decoded = ok(dir.decode(&encoded));
        assert_eq!(
            ok(dir.run(&["encode", "--profile", "profile.json"], &decoded)),
            encoded
        );
        let bad = format!("{}0{}", "[".repeat(1025), "]".repeat(1025));
        let output = dir.run(&["encode", "--profile", "profile.json"], bad.as_bytes());
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("nesting exceeds"));
    });
}

#[test]
fn cli_atomic_no_clobber_and_settings_alias_checks() {
    let dir = Dir::new();
    dir.write(
        "in.json",
        include_bytes!("fixtures/sdk/generator/report.json"),
    );
    let args = [
        "encode",
        "--profile",
        "profile.json",
        "--input",
        "in.json",
        "--out",
        "out.json",
    ];
    ok(dir.run(&args, b""));
    let original = fs::read(dir.0.join("out.json")).unwrap();
    assert!(!dir.run(&args, b"").status.success());
    let mut force = args.to_vec();
    force.push("--force");
    ok(dir.run(&force, b""));
    assert_eq!(fs::read(dir.0.join("out.json")).unwrap(), original);
    for path in ["in.json", "profile.json"] {
        assert!(!dir
            .run(
                &[
                    "encode",
                    "--profile",
                    "profile.json",
                    "--input",
                    "in.json",
                    "--out",
                    path,
                    "--force"
                ],
                b""
            )
            .status
            .success());
    }
    dir.write("headers.json", &original);
    for source in ["headers.json", "profile.json", "out.json"] {
        let link = dir.0.join("settings-link.json");
        fs::hard_link(dir.0.join(source), &link).unwrap();
        let o = dir.run(
            &[
                "decode",
                "--profile",
                "profile.json",
                "--input",
                "headers.json",
                "--keysets",
                "settings-link.json",
                "--positional-count",
                "3",
                "--out",
                "out.json",
                "--force",
            ],
            b"",
        );
        assert!(!o.status.success());
        assert!(String::from_utf8_lossy(&o.stderr).contains("alias"));
        fs::remove_file(link).unwrap();
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            dir.0.join("headers.json"),
            dir.0.join("settings-symlink.json"),
        )
        .unwrap();
        let o = dir.run(
            &[
                "decode",
                "--profile",
                "profile.json",
                "--input",
                "headers.json",
                "--keysets",
                "settings-symlink.json",
                "--positional-count",
                "3",
            ],
            b"",
        );
        assert!(!o.status.success());
        assert!(String::from_utf8_lossy(&o.stderr).contains("alias"));
    }
    assert!(!dir
        .run(
            &[
                "decode",
                "--profile",
                "profile.json",
                "--keysets",
                "-",
                "--positional-count",
                "3"
            ],
            &original
        )
        .status
        .success());
    assert_eq!(fs::read(dir.0.join("out.json")).unwrap(), original);
}

#[test]
fn cli_profile_metadata_defaults_overrides_and_four_channel_golden() {
    #[derive(Deserialize)]
    struct Fixture {
        profile: shape::signals::payload::PayloadProfile,
        answers: Vec<Answer>,
    }
    #[derive(Deserialize)]
    struct Answer {
        name: String,
        report: shape::signals::payload::report::Report,
        base64: Option<String>,
    }
    let fixture: Fixture = deep_json(include_bytes!(
        "fixtures/generator/report-known-answers.json"
    ));
    let dir = Dir::new();
    let mut p = profile();
    p.format = fixture.profile.format;
    p.cipher = fixture.profile.cipher;
    p.alphabet = fixture.profile.alphabet;
    p.keys = fixture.profile.keys;
    for answer in fixture.answers.into_iter().filter(|a| {
        [
            "ordered-omitted-keysets",
            "empty",
            "four-fields-specials-raw",
        ]
        .contains(&a.name.as_str())
    }) {
        let golden = answer.base64.unwrap();
        p.positional_count = Some(answer.report.positional_signals.len());
        p.keysets = Some(answer.report.keysets.clone());
        dir.write("profile.json", &serde_json::to_vec(&p).unwrap());
        // Wrap actual checked-in payload bytes, not a production encode oracle.
        let headers = ok(dir.run(
            &[
                "encode",
                "--profile",
                "profile.json",
                "--input-format",
                "payload",
            ],
            golden.as_bytes(),
        ));
        let full = serde_json::to_vec(&answer.report).unwrap();
        assert_eq!(
            ok(dir.run(&["encode", "--profile", "profile.json"], &full)),
            headers
        );
        let channels = serde_json::to_vec(&answer.report.channels()).unwrap();
        let seeds = format!("{},{}", answer.report.seeds[0], answer.report.seeds[1]);
        assert_eq!(
            ok(dir.run(
                &["encode", "--profile", "profile.json", "--seeds", &seeds],
                &channels
            )),
            headers
        );
        dir.write(
            "keysets.json",
            &serde_json::to_vec(&answer.report.keysets).unwrap(),
        );
        let count = answer.report.positional_signals.len().to_string();
        for case in 0..6 {
            let mut defaults = p.clone();
            let mut args = vec!["decode", "--profile", "profile.json"];
            match case {
                0 => {}
                1 => {
                    defaults.keysets = None;
                    args.extend(["--keysets", "keysets.json"]);
                }
                2 => {
                    defaults.positional_count = None;
                    args.extend(["--positional-count", &count]);
                }
                3 => {
                    defaults.positional_count = Some(999);
                    args.extend(["--positional-count", &count]);
                }
                4 => {
                    defaults.keysets = Some(vec![vec!["conflicting".into()]]);
                    args.extend(["--keysets", "keysets.json"]);
                }
                _ => {
                    defaults.positional_count = Some(999);
                    defaults.keysets = Some(vec![vec!["conflicting".into()]]);
                    args.extend(["--positional-count", &count, "--keysets", "keysets.json"]);
                }
            }
            dir.write("profile.json", &serde_json::to_vec(&defaults).unwrap());
            // Full report input carries its own metadata regardless of defaults.
            assert_eq!(
                ok(dir.run(&["encode", "--profile", "profile.json"], &full)),
                headers
            );
            let decoded: HeaderReport = deep_json(&ok(dir.run(&args, &headers)));
            assert_eq!(decoded.keysets, answer.report.keysets);
            assert_eq!(
                decoded.positional_signals.len(),
                answer.report.positional_signals.len()
            );
            assert_eq!(
                PayloadCodec::new(p.clone().into())
                    .unwrap()
                    .encode_report(&decoded.report())
                    .unwrap(),
                golden
            );
        }
        p.keysets = None;
        p.positional_count = None;
        dir.write("profile.json", &serde_json::to_vec(&p).unwrap());
        for (extra, diagnostic) in [
            (["--positional-count", count.as_str()], "--keysets"),
            (["--keysets", "keysets.json"], "--positional-count"),
        ] {
            let mut args = vec!["decode", "--profile", "profile.json"];
            args.extend(extra);
            let output = dir.run(&args, &headers);
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains(diagnostic));
        }
        if answer.report.keysets.is_empty() {
            assert_eq!(
                ok(dir.run(
                    &["encode", "--profile", "profile.json", "--seeds", &seeds],
                    &channels
                )),
                headers
            );
        }
    }
}
