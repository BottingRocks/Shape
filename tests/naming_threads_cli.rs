use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "shape-threads-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn run(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_shape"))
        .args(["naming", "threads"])
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn cli_report_rename_invalid_inputs_and_no_clobber() {
    let temp = Temp::new();
    let input = temp.0.join("in.js");
    let output = temp.0.join("out.js");
    let map = temp.0.join("map.json");
    fs::write(&input,"function main(){let meaningfulKey=1;function $THREAD_1_2(x){return x;}return [$THREAD_1_2,meaningfulKey];}main();").unwrap();
    let result = run(&[
        input.as_os_str(),
        "--rename".as_ref(),
        "--out".as_ref(),
        output.as_os_str(),
        "--map-out".as_ref(),
        map.as_os_str(),
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(fs::read_to_string(&output)
        .unwrap()
        .contains("function identity(x)"));
    let json: serde_json::Value = serde_json::from_slice(&fs::read(&map).unwrap()).unwrap();
    assert_eq!(json["applied_count"], 1);
    assert!(!json["edits"].as_array().unwrap().is_empty());
    let before = fs::read(&output).unwrap();
    assert!(!run(&[
        input.as_os_str(),
        "--rename".as_ref(),
        "--out".as_ref(),
        output.as_os_str()
    ])
    .status
    .success());
    assert_eq!(fs::read(&output).unwrap(), before);
    assert!(!run(&[
        input.as_os_str(),
        "--rename".as_ref(),
        "--out".as_ref(),
        input.as_os_str()
    ])
    .status
    .success());
    let collision = temp.0.join("same");
    assert!(!run(&[
        input.as_os_str(),
        "--rename".as_ref(),
        "--out".as_ref(),
        collision.as_os_str(),
        "--map-out".as_ref(),
        collision.as_os_str()
    ])
    .status
    .success());
    assert!(!collision.exists());
    let report = run(&[input.as_os_str()]);
    assert!(report.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&report.stdout).unwrap()["applied_count"],
        0
    );
    for args in [
        vec!["--out", "x"],
        vec!["--rename"],
        vec!["--name-anonymous"],
        vec!["--unknown"],
        vec!["--map-out"],
    ] {
        let mut full = vec![input.as_os_str()];
        full.extend(args.iter().map(|s| std::ffi::OsStr::new(s)));
        assert!(!run(&full).status.success());
    }
    let invalid = temp.0.join("bad.js");
    fs::write(&invalid, "function {").unwrap();
    let untouched = temp.0.join("bad-output.js");
    assert!(!run(&[
        invalid.as_os_str(),
        "--rename".as_ref(),
        "--out".as_ref(),
        untouched.as_os_str()
    ])
    .status
    .success());
    assert!(!untouched.exists());
    let traversal = temp.0.join("new/../escape.js");
    assert!(!run(&[
        input.as_os_str(),
        "--rename".as_ref(),
        "--out".as_ref(),
        traversal.as_os_str()
    ])
    .status
    .success());
}
#[cfg(unix)]
#[test]
fn aliases_and_symlinks_cannot_overwrite_the_source() {
    let temp = Temp::new();
    let input = temp.0.join("in.js");
    fs::write(&input, "function main(){return 1;}main();").unwrap();
    let hard = temp.0.join("hard.js");
    fs::hard_link(&input, &hard).unwrap();
    let soft = temp.0.join("soft.js");
    std::os::unix::fs::symlink(&input, &soft).unwrap();
    for path in [&hard, &soft] {
        assert!(!run(&[
            input.as_os_str(),
            "--rename".as_ref(),
            "--out".as_ref(),
            path.as_os_str()
        ])
        .status
        .success());
    }
    assert_eq!(
        fs::read_to_string(&input).unwrap(),
        "function main(){return 1;}main();"
    );
}

#[test]
fn removed_commands_reject_before_input_read_and_preserve_outputs() {
    let temp = Temp::new();
    let out = temp.0.join("existing.js");
    fs::write(&out, "sentinel").unwrap();
    for command in ["annotate", "rename"] {
        for flags in [
            vec!["--help"],
            vec![
                "missing-input.js",
                "--rename",
                "--name-anonymous",
                "--out",
                out.to_str().unwrap(),
                "--force",
            ],
        ] {
            let output = Command::new(env!("CARGO_BIN_EXE_shape"))
                .args(["naming", command])
                .args(flags)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(2));
            let error = String::from_utf8_lossy(&output.stderr);
            assert!(error.contains("removed"));
            assert!(error.contains("naming threads"));
            assert!(!error.contains("No such file"));
        }
    }
    assert_eq!(fs::read_to_string(out).unwrap(), "sentinel");
}

#[test]
fn reusable_reference_cli_roundtrip_no_clobber_and_unequal_build_refusal() {
    let temp = Temp::new();
    let original = temp.0.join("original.js");
    let labeled = temp.0.join("labeled.js");
    let labels = temp.0.join("labels.json");
    let output = temp.0.join("out.js");
    let source =
        "function main(){function $THREAD_1_2(x){return x.length;}return $THREAD_1_2;}main();";
    fs::write(&original, source).unwrap();
    fs::write(&labeled, source.replace("$THREAD_1_2", "readLength")).unwrap();
    let args = [
        labeled.as_os_str(),
        "--original".as_ref(),
        original.as_os_str(),
        "--save-labels".as_ref(),
        labels.as_os_str(),
    ];
    let saved = run(&args);
    assert!(
        saved.status.success(),
        "{}",
        String::from_utf8_lossy(&saved.stderr)
    );
    let before = fs::read(&labels).unwrap();
    assert!(!run(&args).status.success());
    assert_eq!(fs::read(&labels).unwrap(), before);
    let named = run(&[
        original.as_os_str(),
        "--labels".as_ref(),
        labels.as_os_str(),
        "--rename".as_ref(),
        "--out".as_ref(),
        output.as_os_str(),
    ]);
    assert!(
        named.status.success(),
        "{}",
        String::from_utf8_lossy(&named.stderr)
    );
    assert!(fs::read_to_string(output)
        .unwrap()
        .contains("function readLength(x)"));
    fs::write(&labeled, "function main(){return 0;}main();").unwrap();
    let bad = temp.0.join("bad-reference.json");
    assert!(!run(&[
        labeled.as_os_str(),
        "--original".as_ref(),
        original.as_os_str(),
        "--save-labels".as_ref(),
        bad.as_os_str()
    ])
    .status
    .success());
    assert!(!bad.exists());
}
