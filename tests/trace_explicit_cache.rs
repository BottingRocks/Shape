//! Small public VM fixture only: no captures, private inputs, or seed generation.
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use shape::trace::{
    corpus::{load_artifact, load_artifact_with_policy, KernelCachePolicy},
    program::{link_source, link_source_with_artifact, link_source_with_cache, LinkOptions},
};

const SOURCE: &str = include_str!("fixtures/javascript-vm-prefix.raw.js");

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "shape-explicit-cache-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn on_stack(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

fn options(report_dir: Option<PathBuf>) -> LinkOptions {
    LinkOptions {
        seed: "seed.js".into(),
        report_dir,
        steps: 64,
        dump_stages: None,
        thread_cleanup: false,
        no_cleanup: true,
        check_reuse: false,
        overrides: Default::default(),
        jobs: Some(1),
        quiet: true,
    }
}

#[test]
fn explicit_directory_miss_hit_and_source_mismatch() {
    on_stack(|| {
        let scratch = Scratch::new();
        let directory = scratch.0.join("cache");
        let policy = KernelCachePolicy::Directory(directory.clone());
        let seed = Path::new("path/seed.js");
        let first = load_artifact_with_policy(seed, SOURCE, &policy).unwrap();
        let path = directory.join("seed.js.kernel.json");
        let saved = fs::read(&path).unwrap();
        let hit = load_artifact_with_policy(seed, SOURCE, &policy).unwrap();
        assert_eq!(first.to_json().unwrap(), hit.to_json().unwrap());
        assert_eq!(saved, fs::read(&path).unwrap());
        let changed = format!("{SOURCE}\n// different raw bytes");
        let error = load_artifact_with_policy(seed, &changed, &policy).unwrap_err();
        assert!(error.contains("another seed"), "{error}");
        assert_eq!(saved, fs::read(&path).unwrap());
        // The legacy explicit-directory wrapper still accepts the same entry.
        assert_eq!(
            load_artifact(seed, SOURCE, Some(&directory))
                .unwrap()
                .source_hash(),
            first.source_hash()
        );
        fs::write(&path, "invalid artifact").unwrap();
        assert!(load_artifact_with_policy(seed, SOURCE, &policy).is_err());
        fs::remove_file(&path).unwrap();
        fs::write(directory.join("seed.js.report.json"), "legacy").unwrap();
        assert!(load_artifact_with_policy(seed, SOURCE, &policy)
            .unwrap_err()
            .contains("legacy PipelineReport"));
    });
}

#[test]
fn disabled_ignores_invalid_legacy_configuration_without_environment_races() {
    const CHILD: &str = "SHAPE_TEST_EXPLICIT_CACHE_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let scratch = Scratch::new();
        let blocked = scratch.0.join("not-a-directory");
        fs::write(&blocked, "unchanged").unwrap();
        // Configure only a child process; no global set_var or racing test env.
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "disabled_ignores_invalid_legacy_configuration_without_environment_races",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .env("SHAPE_KERNEL_CACHE", &blocked)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read_to_string(blocked).unwrap(), "unchanged");
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
        return;
    }
    on_stack(|| {
        assert!(matches!(
            KernelCachePolicy::default(),
            KernelCachePolicy::Disabled
        ));
        let scratch = Scratch::new();
        fs::write(scratch.0.join("seed.js.kernel.json"), "invalid").unwrap();
        fs::write(scratch.0.join("seed.js.report.json"), "invalid").unwrap();
        let args = options(Some(scratch.0.clone()));
        assert!(link_source(SOURCE, &args).is_err());
        let artifact =
            load_artifact_with_policy(&args.seed, SOURCE, &KernelCachePolicy::Disabled).unwrap();
        artifact.verify_source_identity(SOURCE).unwrap();
        link_source_with_cache(SOURCE, &args, &KernelCachePolicy::Disabled).unwrap();
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 2);
        assert_eq!(
            fs::read_to_string(scratch.0.join("seed.js.kernel.json")).unwrap(),
            "invalid"
        );
    });
}

#[test]
fn loaded_artifact_matches_uncached_trace_and_refuses_other_source() {
    on_stack(|| {
        let args = options(None);
        let artifact =
            load_artifact_with_policy(&args.seed, SOURCE, &KernelCachePolicy::Disabled).unwrap();
        // Exercise reuse of a canonical artifact loaded once from JSON.
        let loaded = shape::KernelArtifact::from_json(&artifact.to_json().unwrap()).unwrap();
        let direct = link_source_with_cache(SOURCE, &args, &KernelCachePolicy::Disabled).unwrap();
        let reused = link_source_with_artifact(SOURCE, &args, &loaded).unwrap();
        assert_eq!(direct.code, reused.code);
        assert_eq!(direct.traced, reused.traced);
        assert_eq!(direct.stopped, reused.stopped);
        assert_eq!(direct.kernel_globals, reused.kernel_globals);
        assert!(direct.traced + direct.stopped.len() > 0);
        let wrong = link_source_with_artifact("different source", &args, &loaded)
            .err()
            .unwrap();
        assert!(wrong.contains("source identity mismatch"), "{wrong}");
    });
}

#[test]
fn independent_policies_coexist_per_call() {
    let scratch = Scratch::new();
    let directory = scratch.0.join("enabled");
    let explicit = directory.clone();
    let cached = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            load_artifact_with_policy(
                Path::new("seed.js"),
                SOURCE,
                &KernelCachePolicy::Directory(explicit),
            )
            .unwrap()
        })
        .unwrap();
    on_stack(move || {
        let artifact =
            load_artifact_with_policy(Path::new("seed.js"), SOURCE, &KernelCachePolicy::Disabled)
                .unwrap();
        assert_eq!(artifact.source_hash(), cached.join().unwrap().source_hash());
    });
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
    let legacy = load_artifact(Path::new("seed.js"), "not a kernel", None).unwrap_err();
    let environment = load_artifact_with_policy(
        Path::new("seed.js"),
        "not a kernel",
        &KernelCachePolicy::Environment,
    )
    .unwrap_err();
    assert_eq!(legacy, environment);
}

#[test]
fn concurrent_directory_publishers_leave_a_valid_entry() {
    let scratch = Scratch::new();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let writers: Vec<_> = (0..2)
        .map(|_| {
            let barrier = barrier.clone();
            let policy = KernelCachePolicy::Directory(scratch.0.clone());
            std::thread::Builder::new()
                .stack_size(64 * 1024 * 1024)
                .spawn(move || {
                    barrier.wait();
                    load_artifact_with_policy(Path::new("seed.js"), SOURCE, &policy)
                })
                .unwrap()
        })
        .collect();
    let mut successes = 0;
    for writer in writers {
        match writer.join().unwrap() {
            Ok(artifact) => {
                artifact.verify_source_identity(SOURCE).unwrap();
                successes += 1;
            }
            // Preserve legacy explicit-cache acceptance: losing publication
            // may fail rather than silently accepting the winner's artifact.
            Err(error) => assert!(error.contains("cannot publish"), "{error}"),
        }
    }
    assert!(successes > 0);
    let json = fs::read_to_string(scratch.0.join("seed.js.kernel.json")).unwrap();
    shape::KernelArtifact::from_json(&json)
        .unwrap()
        .verify_source_identity(SOURCE)
        .unwrap();
    assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
}
