//! Generic typed serialization through the public CLI; synthetic inputs only.
use serde_json::{json, Value};
use shape::signals::payload::superpack::{Number, PackRequest, SerializerOptions, Value as Typed};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
};

struct Dir(PathBuf);
impl Dir {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "shape-pack-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn run(&self, args: &[&str], input: Option<&[u8]>) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_shape"))
            .current_dir(&self.0)
            .arg("gen")
            .args(args)
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        if let Some(input) = input {
            child.stdin.take().unwrap().write_all(input).unwrap();
        }
        child.wait_with_output().unwrap()
    }
    fn write(&self, name: &str, bytes: &[u8]) {
        fs::write(self.0.join(name), bytes).unwrap();
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
fn fails(output: Output, message: &str) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(message),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
fn request(value: Typed, options: SerializerOptions) -> Vec<u8> {
    serde_json::to_vec(&PackRequest { value, options }).unwrap()
}
fn roundtrip(dir: &Dir, value: Typed, options: SerializerOptions) {
    let input = request(value.clone(), options.clone());
    let packed = ok(dir.run(&["pack"], Some(&input)));
    let decoded = ok(dir.run(&["unpack", "--input", "-", "--out", "-"], Some(&packed)));
    let result: PackRequest = serde_json::from_slice(&decoded).unwrap();
    assert_eq!(result.value, value);
    assert_eq!(result.options, options);
    assert_eq!(ok(dir.run(&["pack"], Some(&decoded))), packed);
}

#[test]
fn typed_values_and_explicit_options_roundtrip_without_profile() {
    let dir = Dir::new();
    let values = vec![
        Typed::Undefined,
        Typed::Null,
        Typed::Bool(false),
        Typed::Bool(true),
        Typed::Number(Number(0)),
        Typed::Number(Number(0x8000000000000000)),
        Typed::Number(Number(0x7ff8000000000000)),
        Typed::Number(Number(0x7ff0000000000000)),
        Typed::Number(Number(0xfff0000000000000)),
        Typed::from(1.5),
        Typed::String("text😀\0".into()),
        Typed::Binary(vec![0, 128, 255]),
        Typed::Date(123456789),
        Typed::Array(vec![]),
        Typed::Object(vec![]),
        Typed::Object(vec![
            ("a".into(), Typed::Null),
            ("b".into(), Typed::Undefined),
        ]),
    ];
    roundtrip(&dir, Typed::Array(values), SerializerOptions::default());
    roundtrip(
        &dir,
        Typed::MissingSentinel,
        SerializerOptions::outer_channel(vec![vec!["x".into()]]),
    );
    roundtrip(
        &dir,
        Typed::Extension {
            id: 0xdeadbeef,
            payload: Box::new(Typed::Null),
        },
        SerializerOptions::outer_channel(vec![]),
    );
    roundtrip(
        &dir,
        Typed::Extension {
            id: 42,
            payload: Box::new(Typed::Null),
        },
        SerializerOptions {
            opaque_extensions: true,
            ..Default::default()
        },
    );
    let extension_zero = request(
        Typed::Extension {
            id: 0,
            payload: Box::new(Typed::Undefined),
        },
        SerializerOptions::outer_channel(vec![]),
    );
    let packed = ok(dir.run(&["pack"], Some(&extension_zero)));
    let decoded = ok(dir.run(&["unpack"], Some(&packed)));
    assert_eq!(
        serde_json::from_slice::<PackRequest>(&decoded)
            .unwrap()
            .value,
        Typed::MissingSentinel
    );
    assert_eq!(ok(dir.run(&["pack"], Some(&decoded))), packed);
    for example in [
        include_bytes!("fixtures/sdk/generator/superpack-values.json").as_slice(),
        include_bytes!("fixtures/sdk/generator/superpack-extensions.json").as_slice(),
    ] {
        let packed = ok(dir.run(&["pack"], Some(example)));
        let decoded = ok(dir.run(&["unpack"], Some(&packed)));
        assert_eq!(ok(dir.run(&["pack"], Some(&decoded))), packed);
    }
}

#[test]
fn malformed_noncanonical_trailing_and_truncated_inputs_do_not_write() {
    let dir = Dir::new();
    dir.write("out.json", b"keep");
    // An overwide unsigned integer is structurally legal, but not canonical.
    for (input, diagnostic) in [
        (json!({"bytes":[228,0,1]}), "noncanonical input"),
        (json!({"bytes":[1,2]}), "trailing"),
        (json!({"bytes":[228]}), "truncated"),
        (json!({"bytes":[]}), "truncated"),
        (json!({"bytes":[256]}), "unpack JSON"),
        (json!({"bytes":[1],"unknown":true}), "unknown field"),
    ] {
        let output = dir.run(
            &["unpack", "--out", "out.json", "--force"],
            Some(&serde_json::to_vec(&input).unwrap()),
        );
        if diagnostic == "noncanonical input" {
            assert!(String::from_utf8_lossy(&output.stderr).contains("raw/library API"));
        }
        fails(output, diagnostic);
        assert_eq!(fs::read(dir.0.join("out.json")).unwrap(), b"keep");
    }
    for input in [
        r#"{"value":{"kind":"null"},"extra":1}"#,
        r#"{"value":{"kind":"number","value":"NaN"}}"#,
        r#"{"value":{"kind":"number","value":0}}"#,
        r#"{"value":{"kind":"null","extra":1}}"#,
        r#"{"value":{"kind":"null"},"options":{"extra":1}}"#,
        r#"{"value":{"kind":"null"}} garbage"#,
    ] {
        fails(
            dir.run(&["pack", "--out", "new.json"], Some(input.as_bytes())),
            "pack JSON",
        );
        assert!(!dir.0.join("new.json").exists());
    }
    let limited =
        json!({"value":{"kind":"binary","value":[1,2]},"options":{"limits":{"max_bytes":1}}});
    fails(
        dir.run(&["pack"], Some(&serde_json::to_vec(&limited).unwrap())),
        "limit",
    );
}

#[test]
fn files_no_clobber_force_and_aliases() {
    let dir = Dir::new();
    let input = request(Typed::Undefined, SerializerOptions::default());
    dir.write("value.json", &input);
    ok(dir.run(
        &["pack", "--input", "value.json", "--out", "packed.json"],
        None,
    ));
    let packed = fs::read(dir.0.join("packed.json")).unwrap();
    fails(
        dir.run(
            &["pack", "--input", "value.json", "--out", "packed.json"],
            None,
        ),
        "already exists",
    );
    assert_eq!(fs::read(dir.0.join("packed.json")).unwrap(), packed);
    ok(dir.run(
        &[
            "pack",
            "--input",
            "value.json",
            "--out",
            "packed.json",
            "--force",
        ],
        None,
    ));
    ok(dir.run(
        &["unpack", "--input", "packed.json", "--out", "decoded.json"],
        None,
    ));
    ok(dir.run(
        &["pack", "--input", "decoded.json", "--out", "again.json"],
        None,
    ));
    assert_eq!(fs::read(dir.0.join("again.json")).unwrap(), packed);
    for command in ["pack", "unpack"] {
        fails(
            dir.run(
                &[
                    command,
                    "--input",
                    "value.json",
                    "--out",
                    "value.json",
                    "--force",
                ],
                None,
            ),
            "alias",
        );
        fs::hard_link(dir.0.join("value.json"), dir.0.join("hard.json")).unwrap();
        fails(
            dir.run(
                &[
                    command,
                    "--input",
                    "value.json",
                    "--out",
                    "hard.json",
                    "--force",
                ],
                None,
            ),
            "alias",
        );
        fs::remove_file(dir.0.join("hard.json")).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("value.json", dir.0.join("link.json")).unwrap();
            fails(
                dir.run(
                    &[
                        command,
                        "--input",
                        "link.json",
                        "--out",
                        "value.json",
                        "--force",
                    ],
                    None,
                ),
                "alias",
            );
            fails(
                dir.run(
                    &[
                        command,
                        "--input",
                        "value.json",
                        "--out",
                        "link.json",
                        "--force",
                    ],
                    None,
                ),
                "alias",
            );
            fs::remove_file(dir.0.join("link.json")).unwrap();
        }
        assert_eq!(fs::read(dir.0.join("value.json")).unwrap(), input);
    }
    assert!(!fs::read_dir(&dir.0).unwrap().any(|e| e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .ends_with(".tmp")));
}

#[test]
fn option_errors_and_missing_request_fields() {
    let dir = Dir::new();
    for command in ["pack", "unpack"] {
        for args in [
            vec![command, "--input"],
            vec![command, "--out"],
            vec![command, "--input", "-", "--input", "-"],
            vec![command, "--out", "-", "--out", "-"],
            vec![command, "--force", "--force"],
            vec![command, "--profile", "none"],
            vec![command, "--unknown"],
            vec![command, "--positional-count", "0"],
        ] {
            fails(dir.run(&args, None), "error:");
        }
        fails(dir.run(&[command], Some(b"{}")), "missing field");
    }
    let packed = ok(dir.run(&["pack"], Some(br#"{"value":{"kind":"null"}}"#)));
    let document: Value = serde_json::from_slice(&packed).unwrap();
    assert!(document.get("options").is_some());
}

#[test]
fn oversized_json_is_bounded_before_parsing_or_output() {
    let dir = Dir::new();
    // A sparse file checks the shared reader ceiling without a huge fixture.
    fs::File::create(dir.0.join("large.json"))
        .unwrap()
        .set_len(128 * 1024 * 1024 + 1)
        .unwrap();
    for command in ["pack", "unpack"] {
        fails(
            dir.run(
                &[command, "--input", "large.json", "--out", "out.json"],
                None,
            ),
            "JSON input exceeds",
        );
        assert!(!dir.0.join("out.json").exists());
    }
}

fn nest(mut value: Typed, depth: usize, objects: bool) -> Typed {
    for _ in 0..depth {
        value = if objects {
            Typed::Object(vec![("x".into(), value)])
        } else {
            Typed::Array(vec![value])
        };
    }
    value
}

#[test]
fn source_depth_100_cli_roundtrips_and_truncates_only_containers() {
    shape::run_with_large_stack(|| {
        use shape::signals::payload::superpack::{unpack_canonical, UnpackRequest, DEPTH_EXTENSION};
        let dir = Dir::new();
        let options = SerializerOptions::outer_channel(vec![]);
        let specials = Typed::Array(vec![
            Typed::Undefined,
            Typed::Null,
            Typed::Number(Number(0)),
            Typed::Number(Number(0x8000000000000000)),
            Typed::Number(Number(0x7ff8000000000000)),
            Typed::Number(Number(0x7ff0000000000000)),
            Typed::Number(Number(0xfff0000000000000)),
        ]);
        for objects in [false, true] {
            for depth in [99, 100, 101, 256] {
                let value = nest(specials.clone(), depth, objects);
                let input = request(value.clone(), options.clone());
                // Reproduce the accidental old CLI ceiling on INPUT only.
                assert!(serde_json::from_slice::<PackRequest>(&input).is_err());
                let packed = ok(dir.run(&["pack"], Some(&input)));
                let wire: UnpackRequest = serde_json::from_slice(&packed).unwrap();
                assert_eq!(
                    wire.bytes,
                    shape::signals::payload::superpack::pack(&value, &options).unwrap()
                );
                let expected = if depth == 99 {
                    value
                } else {
                    nest(
                        Typed::Extension {
                            id: DEPTH_EXTENSION,
                            payload: Box::new(Typed::Null),
                        },
                        100,
                        objects,
                    )
                };
                assert_eq!(unpack_canonical(&wire.bytes, &options).unwrap(), expected);
                if depth >= 100 {
                    assert!(wire
                        .bytes
                        .ends_with(&[0xff, 0xe6, 0xde, 0xad, 0xbe, 0xef, 0xe2]));
                }
                let decoded = ok(dir.run(&["unpack"], Some(&packed)));
                // No default-depth serde parser touches deep CLI output.
                assert_eq!(
                    decoded,
                    [request(expected, options.clone()), vec![b'\n']].concat()
                );
                assert_eq!(ok(dir.run(&["pack"], Some(&decoded))), packed);
            }
        }
        // Fixed source byte answers: the primitive at semantic depth 100 is
        // preserved; only an array/object entered at depth 100 is truncated.
        for (leaf, tail) in [
            (Typed::Null, vec![0xe2]),
            (Typed::Undefined, vec![0xe3]),
            (
                Typed::Number(Number(0x8000000000000000)),
                vec![0xec, 0x80, 0, 0, 0],
            ),
            (
                Typed::Array(vec![]),
                vec![0xff, 0xe6, 0xde, 0xad, 0xbe, 0xef, 0xe2],
            ),
        ] {
            let packed = ok(dir.run(
                &["pack"],
                Some(&request(nest(leaf, 100, false), options.clone())),
            ));
            let wire: UnpackRequest = serde_json::from_slice(&packed).unwrap();
            assert_eq!(wire.bytes, [vec![0xa1; 100], tail].concat());
        }
    });
}

#[test]
fn full_core_depth_remains_available_and_resource_limits_still_apply() {
    shape::run_with_large_stack(|| {
        let dir = Dir::new();
        let mut options = SerializerOptions::default();
        options.limits.max_depth = 256;
        // Object layers are the worst structural JSON overhead (three each).
        let input = request(nest(Typed::Null, 256, true), options.clone());
        let packed = ok(dir.run(&["pack"], Some(&input)));
        let decoded = ok(dir.run(&["unpack"], Some(&packed)));
        assert_eq!(decoded, [input, vec![b'\n']].concat());
        assert_eq!(ok(dir.run(&["pack"], Some(&decoded))), packed);
        fails(
            dir.run(
                &["pack"],
                Some(&request(nest(Typed::Null, 257, true), options.clone())),
            ),
            "depth limit",
        );
        options.limits.max_depth = 128;
        fails(
            dir.run(
                &["pack"],
                Some(&request(nest(Typed::Null, 129, false), options.clone())),
            ),
            "depth limit",
        );
        options.limits.max_entries = 2;
        fails(
            dir.run(
                &["pack"],
                Some(&request(nest(Typed::Null, 3, false), options)),
            ),
            "limit",
        );
        let mut options = SerializerOptions::outer_channel(vec![]);
        options.limits.max_depth = 99;
        fails(
            dir.run(
                &["pack"],
                Some(&request(nest(Typed::Null, 100, false), options)),
            ),
            "depth limit",
        );
    });
}

#[test]
fn deep_cli_bytes_match_both_shipped_original_js_encoders_when_node_available() {
    match Command::new("node").arg("--version").output() {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("Node unavailable; native and fixed deep byte assertions still run");
            return;
        }
        Err(error) => panic!("Node: {error}"),
        Ok(output) => assert!(output.status.success()),
    }
    let script = r#"
const path = require('node:path');
const answers = [];
for (const file of ['superpack-reference.cjs','superpack-reference-alternate.cjs']) {
  const api = require(path.resolve('tests/fixtures/generator', file));
  const cases = [];
  for (const objects of [false,true]) for (const depth of [99,100,101,256]) {
    let value = [undefined,null,0,-0,NaN,Infinity,-Infinity];
    for(let i=0;i<depth;i++) value = objects ? {x:value} : [value];
    cases.push(Array.from(api.packValue(value, api.signalOptions([],100))));
  }
  answers.push(cases);
}

console.log(JSON.stringify(answers));
"#;
    let output = Command::new("node")
        .args(["-e", script])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let answers: Vec<Vec<Vec<u8>>> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(answers.len(), 2);
    assert_eq!(answers[0], answers[1]);
    shape::run_with_large_stack(move || {
        let dir = Dir::new();
        let mut index = 0;
        for objects in [false, true] {
            for depth in [99, 100, 101, 256] {
                let specials = Typed::Array(vec![
                    Typed::Undefined,
                    Typed::Null,
                    Typed::from(0.0),
                    Typed::from(-0.0),
                    Typed::from(f64::NAN),
                    Typed::from(f64::INFINITY),
                    Typed::from(f64::NEG_INFINITY),
                ]);
                let packed = ok(dir.run(
                    &["pack"],
                    Some(&request(
                        nest(specials, depth, objects),
                        SerializerOptions::outer_channel(vec![]),
                    )),
                ));
                let wire: shape::signals::payload::superpack::UnpackRequest =
                    serde_json::from_slice(&packed).unwrap();
                assert_eq!(
                    wire.bytes, answers[0][index],
                    "objects={objects}, depth={depth}"
                );
                index += 1;
            }
        }
        assert_eq!(index, answers[0].len());
    });
}

#[test]
fn deep_generic_pack_cipher_decode_unpack_cli_pipeline_preserves_bytes() {
    shape::run_with_large_stack(|| {
        use shape::signals::payload::{superpack::UnpackRequest, Payload};
        let dir = Dir::new();
        dir.write(
            "profile.json",
            include_bytes!("fixtures/sdk/generator/profile.json"),
        );
        let options = SerializerOptions::outer_channel(vec![]);
        let input = request(nest(Typed::Null, 101, true), options.clone());
        let packed = ok(dir.run(&["pack"], Some(&input)));
        let wire: UnpackRequest = serde_json::from_slice(&packed).unwrap();
        let payload = Payload::raw([0x11223344, 0x55667788], wire.bytes.clone());
        let headers = ok(dir.run(
            &["encode", "--profile", "profile.json"],
            Some(&serde_json::to_vec(&payload).unwrap()),
        ));
        let decoded = ok(dir.run(&["decode", "--profile", "profile.json"], Some(&headers)));
        let recovered: Payload = serde_json::from_slice(&decoded).unwrap();
        assert_eq!(recovered.seeds, payload.seeds);
        assert_eq!(recovered.plaintext().unwrap(), wire.bytes);
        let unpack_input = serde_json::to_vec(&UnpackRequest {
            bytes: recovered.plaintext().unwrap(),
            options,
        })
        .unwrap();
        let value = ok(dir.run(&["unpack"], Some(&unpack_input)));
        assert_eq!(ok(dir.run(&["pack"], Some(&value))), packed);
    });
}

#[test]
fn structural_cap_quotes_escapes_and_malformed_json_are_bounded() {
    shape::run_with_large_stack(structural_cap_checks);
}

fn structural_cap_checks() {
    let dir = Dir::new();
    let options = SerializerOptions::outer_channel(vec![]);
    // Request + 511 typed arrays + null = 1 + 2*511 + 1 = 1024.
    let exact = request(nest(Typed::Null, 511, false), options.clone());
    let packed = ok(dir.run(&["pack"], Some(&exact)));
    // Content preceding its tag exercises serde's recursive buffering path.
    let mut reordered = r#"{"kind":"null"}"#.to_owned();
    for _ in 0..511 {
        reordered = format!(r#"{{"value":[{reordered}],"kind":"array"}}"#);
    }
    let reordered = format!(
        r#"{{"value":{reordered},"options":{}}}"#,
        serde_json::to_string(&options).unwrap()
    );
    assert_eq!(ok(dir.run(&["pack"], Some(reordered.as_bytes()))), packed);
    let decoded = ok(dir.run(&["unpack"], Some(&packed)));
    assert_eq!(ok(dir.run(&["pack"], Some(&decoded))), packed);
    // Replacing one array by an object tuple adds exactly one level.
    let over = request(nest(nest(Typed::Null, 1, true), 510, false), options);
    fails(
        dir.run(&["pack", "--out", "new.json"], Some(&over)),
        "JSON nesting exceeds 1024",
    );
    assert!(!dir.0.join("new.json").exists());
    for depth in [1024, 1025, 100_000] {
        let input = format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
        dir.write("hostile.json", input.as_bytes());
        dir.write("keep.json", b"keep");
        let output = dir.run(
            &[
                "pack",
                "--input",
                "hostile.json",
                "--out",
                "keep.json",
                "--force",
            ],
            None,
        );
        fails(
            output,
            if depth == 1024 {
                "invalid type"
            } else {
                "JSON nesting exceeds 1024"
            },
        );
        assert_eq!(fs::read(dir.0.join("keep.json")).unwrap(), b"keep");
        assert!(!dir.0.join("new.json").exists());
    }
    let text = format!("{}\\\"{}\\", "[{".repeat(2048), "}]".repeat(2048));
    roundtrip(&dir, Typed::String(text), SerializerOptions::default());
    for input in [
        r#"{"value":{"kind":"null"},"value":{"kind":"null"}}"#,
        r#"{"value":{"kind":"null","kind":"null"}}"#,
        r#"{"value":{"kind":"string","value":"\q"}}"#,
        r#"{"value":{"kind":"string","value":"unterminated\"}}"#,
        r#"{"value":{"kind":"array","value":[}}"#,
        r#"{"value":{"kind":"null"}} []"#,
        r#"{"value":{"kind":"null"}} {"#,
        r#"{"value":{"kind":"null"}} ]"#,
        r#"{"value":{"kind":"null"}"#,
    ] {
        fails(
            dir.run(&["pack", "--out", "new.json"], Some(input.as_bytes())),
            "pack JSON",
        );
        assert!(!dir.0.join("new.json").exists());
    }
}

#[cfg(unix)]
#[test]
fn symlinked_parent_aliases_and_unsafe_outputs_are_refused() {
    let dir = Dir::new();
    let input = request(Typed::Null, SerializerOptions::default());
    dir.write("value.json", &input);
    std::os::unix::fs::symlink(&dir.0, dir.0.join("parent")).unwrap();
    std::os::unix::fs::symlink("missing.json", dir.0.join("dangling.json")).unwrap();
    fs::create_dir(dir.0.join("directory")).unwrap();
    for command in ["pack", "unpack"] {
        fails(
            dir.run(
                &[
                    command,
                    "--input",
                    "value.json",
                    "--out",
                    "parent/value.json",
                    "--force",
                ],
                None,
            ),
            "alias",
        );
        for out in ["dangling.json", "directory"] {
            fails(
                dir.run(
                    &[command, "--input", "value.json", "--out", out, "--force"],
                    None,
                ),
                "regular file",
            );
        }
    }
    assert_eq!(fs::read(dir.0.join("value.json")).unwrap(), input);
    assert!(!dir.0.join("missing.json").exists());
}
