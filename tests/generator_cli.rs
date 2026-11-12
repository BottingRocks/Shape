//! Hermetic end-to-end tests of the byte generator. No JS, Python, or browser.
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

use serde_json::{json, Value};

const CLI: &str = env!("CARGO_BIN_EXE_shape");
const VECTORS: &str = include_str!("fixtures/generator/known-answers.json");

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "shape-generator-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let dir = Self(path);
        let fixture = fixture(0);
        dir.write(
            "profile.json",
            &serde_json::to_string(&fixture["profile"]).unwrap(),
        );
        dir.write(
            "data.json",
            &serde_json::to_string(&payload(&fixture)).unwrap(),
        );
        dir
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
    fn write(&self, name: &str, data: &str) {
        fs::write(self.path(name), data).unwrap();
    }
    fn run(&self, args: &[&str], stdin: Option<&[u8]>) -> Output {
        let mut child = Command::new(CLI)
            .current_dir(&self.0)
            .args(args)
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        if let Some(input) = stdin {
            child.stdin.take().unwrap().write_all(input).unwrap();
        }
        child.wait_with_output().unwrap()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn fixture(index: usize) -> Value {
    serde_json::from_str::<Value>(VECTORS).unwrap()[index].clone()
}
fn payload(fixture: &Value) -> Value {
    let hex = fixture["plaintext_hex"].as_str().unwrap();
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    json!({"seeds":fixture["seeds"],"data":{"kind":"raw","bytes":bytes}})
}
fn success(output: &Output) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn failure(output: &Output, message: &str) {
    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "unexpected stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(message),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn help_lists_working_generator_and_only_supported_commands() {
    let dir = Directory::new();
    let help = dir.run(&["--help"], None);
    success(&help);
    assert!(String::from_utf8_lossy(&help.stdout).contains("headers <encode|decode>"));
    let help = dir.run(&["gen", "--help"], None);
    success(&help);
    assert!(String::from_utf8_lossy(&help.stdout).contains("encode"));
    let help = dir.run(&["gen", "decode", "--help"], None);
    success(&help);
    assert!(String::from_utf8_lossy(&help.stdout).contains("--positional-count"));
    failure(&dir.run(&["gen", "build"], None), "unrecognized subcommand");
}

#[test]
fn stdin_stdout_known_answers_and_decode_output_reused_as_encode_input() {
    let dir = Directory::new();
    for i in 0..3 {
        let fixture = fixture(i);
        dir.write(
            "profile.json",
            &serde_json::to_string(&fixture["profile"]).unwrap(),
        );
        let input = serde_json::to_vec(&payload(&fixture)).unwrap();
        let output = dir.run(
            &["gen", "encode", "--profile", "profile.json"],
            Some(&input),
        );
        success(&output);
        let headers: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(headers, json!({"X-fixture-a":fixture["value"]}));
        let lower_headers = serde_json::to_vec(&json!({"x-fixture-A":fixture["value"]})).unwrap();
        let decoded = dir.run(
            &[
                "gen",
                "decode",
                "--profile",
                "profile.json",
                "--input",
                "-",
                "--out",
                "-",
            ],
            Some(&lower_headers),
        );
        success(&decoded);
        assert_eq!(
            serde_json::from_slice::<Value>(&decoded.stdout).unwrap(),
            payload(&fixture)
        );
        let encoded = dir.run(
            &["gen", "encode", "--profile", "profile.json"],
            Some(&decoded.stdout),
        );
        success(&encoded);
        assert_eq!(encoded.stdout, output.stdout);
    }
}

#[test]
fn container_cli_and_checked_in_fixtures_roundtrip() {
    let dir = Directory::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    fs::copy(
        root.join("tests/fixtures/sdk/generator/profile.json"),
        dir.path("profile.json"),
    )
    .unwrap();
    for name in ["raw.json", "container.json"] {
        let input = fs::read(root.join("tests/fixtures/sdk/generator").join(name)).unwrap();
        let encoded = dir.run(
            &["gen", "encode", "--profile", "profile.json"],
            Some(&input),
        );
        success(&encoded);
        let mut args = vec!["gen", "decode", "--profile", "profile.json"];
        if name == "container.json" {
            args.extend(["--positional-count", "2"]);
        }
        let decoded = dir.run(&args, Some(&encoded.stdout));
        success(&decoded);
        assert_eq!(
            serde_json::from_slice::<Value>(&decoded.stdout).unwrap(),
            serde_json::from_slice::<Value>(&input).unwrap()
        );
        let reencoded = dir.run(
            &["gen", "encode", "--profile", "profile.json"],
            Some(&decoded.stdout),
        );
        success(&reencoded);
        assert_eq!(encoded.stdout, reencoded.stdout);
    }
}

#[test]
fn file_output_no_clobber_force_and_failed_requests_do_not_change_output() {
    let dir = Directory::new();
    let args = [
        "gen",
        "encode",
        "--profile",
        "profile.json",
        "--input",
        "data.json",
        "--out",
        "headers.json",
    ];
    let out = dir.run(&args, None);
    success(&out);
    assert!(out.stdout.is_empty());
    let expected = fs::read(dir.path("headers.json")).unwrap();
    failure(&dir.run(&args, None), "output already exists");
    assert_eq!(fs::read(dir.path("headers.json")).unwrap(), expected);
    dir.write("headers.json", "preserve me");
    failure(&dir.run(&args, None), "output already exists");
    assert_eq!(
        fs::read_to_string(dir.path("headers.json")).unwrap(),
        "preserve me"
    );
    let mut force = args.to_vec();
    force.push("--force");
    success(&dir.run(&force, None));
    assert_eq!(fs::read(dir.path("headers.json")).unwrap(), expected);
    dir.write("data.json", "{bad JSON}");
    failure(&dir.run(&force, None), "payload JSON");
    assert_eq!(fs::read(dir.path("headers.json")).unwrap(), expected);
    assert!(!fs::read_dir(&dir.0).unwrap().any(|e| e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with(".shape-gen-")));
}

#[test]
fn direct_lexical_and_input_profile_path_aliases_refused_even_with_force() {
    let dir = Directory::new();
    let original_input = fs::read(dir.path("data.json")).unwrap();
    let original_profile = fs::read(dir.path("profile.json")).unwrap();
    for out in ["data.json", "./data.json", "profile.json", "./profile.json"] {
        failure(
            &dir.run(
                &[
                    "gen",
                    "encode",
                    "--profile",
                    "profile.json",
                    "--input",
                    "data.json",
                    "--out",
                    out,
                    "--force",
                ],
                None,
            ),
            "paths alias",
        );
    }
    failure(
        &dir.run(
            &[
                "gen",
                "encode",
                "--profile",
                "profile.json",
                "--input",
                "./profile.json",
                "--out",
                "new.json",
            ],
            None,
        ),
        "paths alias",
    );
    assert_eq!(fs::read(dir.path("data.json")).unwrap(), original_input);
    assert_eq!(
        fs::read(dir.path("profile.json")).unwrap(),
        original_profile
    );
    assert!(!dir.path("new.json").exists());
}

#[cfg(unix)]
#[test]
fn symlink_hardlink_and_directory_aliases_refused_before_any_write() {
    use std::os::unix::fs::symlink;
    let dir = Directory::new();
    let original_input = fs::read(dir.path("data.json")).unwrap();
    let original_profile = fs::read(dir.path("profile.json")).unwrap();
    symlink(dir.path("data.json"), dir.path("data-link.json")).unwrap();
    fs::hard_link(dir.path("data.json"), dir.path("data-hard.json")).unwrap();
    fs::hard_link(dir.path("profile.json"), dir.path("profile-hard.json")).unwrap();
    symlink(&dir.0, dir.path("linked-dir")).unwrap();
    for out in [
        "data-link.json",
        "data-hard.json",
        "profile-hard.json",
        "linked-dir/data.json",
        "linked-dir/./profile.json",
    ] {
        failure(
            &dir.run(
                &[
                    "gen",
                    "encode",
                    "--profile",
                    "profile.json",
                    "--input",
                    "data.json",
                    "--out",
                    out,
                    "--force",
                ],
                None,
            ),
            "paths alias",
        );
    }
    failure(
        &dir.run(
            &[
                "gen",
                "encode",
                "--profile",
                "profile.json",
                "--input",
                "profile-hard.json",
                "--out",
                "new.json",
            ],
            None,
        ),
        "paths alias",
    );
    symlink("does-not-exist.json", dir.path("dangling.json")).unwrap();
    failure(
        &dir.run(
            &[
                "gen",
                "encode",
                "--profile",
                "profile.json",
                "--input",
                "data.json",
                "--out",
                "dangling.json",
                "--force",
            ],
            None,
        ),
        "symlinks",
    );
    assert_eq!(fs::read(dir.path("data.json")).unwrap(), original_input);
    assert_eq!(
        fs::read(dir.path("profile.json")).unwrap(),
        original_profile
    );
}

#[test]
fn malformed_json_and_profile_have_stderr_only_and_leave_no_output() {
    let dir = Directory::new();
    for input in [
        "{}",
        "null",
        "[]",
        "not json",
        r#"{"seeds":[1,2],"data":{"kind":"raw","bytes":[],"ignored":1}}"#,
        r#"{"seeds":[1,2,3],"data":{"kind":"raw","bytes":[]}}"#,
        r#"{"seeds":[1,2],"data":{"kind":"raw","bytes":[256]}}"#,
    ] {
        failure(
            &dir.run(
                &[
                    "gen",
                    "encode",
                    "--profile",
                    "profile.json",
                    "--out",
                    "new.json",
                ],
                Some(input.as_bytes()),
            ),
            "payload JSON",
        );
        assert!(!dir.path("new.json").exists());
    }
    dir.write("profile.json", "{}");
    failure(
        &dir.run(
            &[
                "gen",
                "encode",
                "--profile",
                "profile.json",
                "--input",
                "data.json",
                "--out",
                "new.json",
            ],
            None,
        ),
        "profile JSON",
    );
    assert!(!dir.path("new.json").exists());
    dir.write(
        "profile.json",
        &serde_json::to_string(&fixture(0)["profile"]).unwrap(),
    );
    let mut p = fixture(0)["profile"].clone();
    p["cipher"]["double_rounds"] = json!(0);
    dir.write("profile.json", &p.to_string());
    failure(
        &dir.run(
            &[
                "gen",
                "encode",
                "--profile",
                "profile.json",
                "--input",
                "data.json",
            ],
            None,
        ),
        "double_rounds",
    );
    failure(
        &dir.run(&["gen", "encode", "--profile", "-"], None),
        "requires a file",
    );
    failure(&dir.run(&["gen", "encode"], None), "--profile");
}

#[test]
fn decode_rejects_duplicate_unknown_unsafe_and_noncanonical_headers() {
    let dir = Directory::new();
    for (input, message) in [
        (
            r#"{"X-fixture-a":"x","X-fixture-a":"y"}"#,
            "duplicate header",
        ),
        (
            r#"{"X-fixture-a":"x","x-fixture-A":"y"}"#,
            "duplicate header",
        ),
        (r#"{"X-fixture-a":"x","X-other-b":"y"}"#, "unsupported"),
        (r#"{"X-other-b":"abcd"}"#, "unsupported header name"),
        (r#"{"X-fixture-a":"DD"}"#, "8-byte seed prefix"),
        (r#"{"X-fixture-a":"D1\r\n"}"#, "visible ASCII"),
        (r#"{"X-fixture-a":"D1q"}"#, "canonical"),
    ] {
        failure(
            &dir.run(
                &["gen", "decode", "--profile", "profile.json"],
                Some(input.as_bytes()),
            ),
            message,
        );
    }
    let headers = json!({"X-fixture-a":fixture(0)["value"]}).to_string();
    failure(
        &dir.run(
            &[
                "gen",
                "decode",
                "--profile",
                "profile.json",
                "--positional-count",
                "0",
            ],
            Some(headers.as_bytes()),
        ),
        "container",
    );
    failure(
        &dir.run(
            &[
                "gen",
                "decode",
                "--profile",
                "profile.json",
                "--positional-count",
                "4294967295",
            ],
            None,
        ),
        "invalid value",
    );
}

#[test]
fn session_bundle_cli_roundtrip_and_new_byte_input_with_explicit_context() {
    let dir = Directory::new();
    let profile: Value =
        serde_json::from_str(include_str!("fixtures/sdk/generator/session-profile.json")).unwrap();
    dir.write("profile.json", &profile.to_string());
    let input = json!({"seeds":[123,456],"data":{"kind":"raw","bytes":(0..256).map(|i|i as u8).collect::<Vec<_>>()}}).to_string();
    let encoded = dir.run(
        &["gen", "encode", "--profile", "profile.json"],
        Some(input.as_bytes()),
    );
    success(&encoded);
    let bundle: Value = serde_json::from_slice(&encoded.stdout).unwrap();
    assert!(bundle.get("X-example-a10").is_some());
    assert_eq!(bundle["X-example-f"], profile["transaction_token"]);
    assert_eq!(bundle["X-example-d"], profile["header_context"]["d"]);
    dir.write(
        "headers.json",
        std::str::from_utf8(&encoded.stdout).unwrap(),
    );
    let decoded = dir.run(
        &[
            "gen",
            "decode",
            "--profile",
            "profile.json",
            "--input",
            "headers.json",
            "--out",
            "decoded.json",
        ],
        None,
    );
    success(&decoded);
    let plain: Value =
        serde_json::from_slice(&fs::read(dir.path("decoded.json")).unwrap()).unwrap();
    assert_eq!(
        plain["data"],
        serde_json::from_str::<Value>(&input).unwrap()["data"]
    );
    assert_eq!(plain["header_context"], profile["header_context"]);
    let back = dir.run(
        &[
            "gen",
            "encode",
            "--profile",
            "profile.json",
            "--input",
            "decoded.json",
        ],
        None,
    );
    success(&back);
    assert_eq!(
        serde_json::from_slice::<Value>(&back.stdout).unwrap(),
        bundle
    );
    for key in ["X-example-b", "X-example-f"] {
        let mut bad = bundle.clone();
        bad[key] = json!("wrong");
        failure(
            &dir.run(
                &["gen", "decode", "--profile", "profile.json"],
                Some(bad.to_string().as_bytes()),
            ),
            if key.ends_with('b') {
                "checksum mismatch"
            } else {
                "does not match"
            },
        );
    }
    // Context may be carried exclusively by decoded input for the next encode.
    let mut runtime = profile.clone();
    runtime.as_object_mut().unwrap().remove("header_context");
    dir.write("runtime.json", &runtime.to_string());
    let back = dir.run(
        &[
            "gen",
            "encode",
            "--profile",
            "runtime.json",
            "--input",
            "decoded.json",
        ],
        None,
    );
    success(&back);
    assert_eq!(
        serde_json::from_slice::<Value>(&back.stdout).unwrap(),
        bundle
    );
    let mut changed = plain;
    changed["data"]["bytes"] = json!([255, 0]);
    changed["seeds"] = json!([1, 2]);
    let next = dir.run(
        &["gen", "encode", "--profile", "runtime.json"],
        Some(changed.to_string().as_bytes()),
    );
    success(&next);
    let next: Value = serde_json::from_slice(&next.stdout).unwrap();
    assert_ne!(next["X-example-b"], bundle["X-example-b"]);
    assert_eq!(next["X-example-c"], bundle["X-example-c"]);
}

#[test]
fn opaque_decode_profile_count_is_optional_overridable_and_bounded() {
    let dir = Directory::new();
    let mut profile: Value =
        serde_json::from_str(include_str!("fixtures/sdk/generator/profile.json")).unwrap();
    dir.write("profile.json", &profile.to_string());
    let input = include_bytes!("fixtures/sdk/generator/container.json");
    let encoded = dir.run(&["gen", "encode", "--profile", "profile.json"], Some(input));
    success(&encoded);
    let decode = ["gen", "decode", "--profile", "profile.json"];
    let raw = dir.run(&decode, Some(&encoded.stdout));
    success(&raw);
    assert_eq!(
        serde_json::from_slice::<Value>(&raw.stdout).unwrap()["data"]["kind"],
        "raw"
    );
    profile["positional_count"] = json!(2);
    dir.write("profile.json", &profile.to_string());
    let decoded = dir.run(&decode, Some(&encoded.stdout));
    success(&decoded);
    assert_eq!(
        serde_json::from_slice::<Value>(&decoded.stdout).unwrap(),
        serde_json::from_slice::<Value>(input).unwrap()
    );
    // Profile counts must fit usize before count validation or CLI overrides run.
    for count in std::iter::once(shape::signals::payload::MAX_CONTAINER_ENTRIES + 1).chain(
        [u64::from(u32::MAX) + 1, u64::from(u32::MAX) + 3]
            .into_iter()
            .filter_map(|count| usize::try_from(count).ok()),
    ) {
        profile["positional_count"] = json!(count);
        dir.write("profile.json", &profile.to_string());
        failure(&dir.run(&decode, Some(&encoded.stdout)), "positional count");
        // The effective CLI override wins even over out-of-range saved metadata.
        let mut args = decode.to_vec();
        args.extend(["--positional-count", "2"]);
        let decoded = dir.run(&args, Some(&encoded.stdout));
        success(&decoded);
        assert_eq!(
            serde_json::from_slice::<Value>(&decoded.stdout).unwrap(),
            serde_json::from_slice::<Value>(input).unwrap()
        );
    }
    // Zero must request container parsing rather than falling back to raw bytes.
    profile["positional_count"] = json!(0);
    dir.write("profile.json", &profile.to_string());
    let empty = br#"{"seeds":[1,2],"data":{"kind":"container","container":{"errors":[],"positional":[],"signals":[],"keyed":[]}}}"#;
    let encoded = dir.run(&["gen", "encode", "--profile", "profile.json"], Some(empty));
    success(&encoded);
    let decoded = dir.run(&decode, Some(&encoded.stdout));
    success(&decoded);
    assert_eq!(
        serde_json::from_slice::<Value>(&decoded.stdout).unwrap(),
        serde_json::from_slice::<Value>(empty).unwrap()
    );
}
