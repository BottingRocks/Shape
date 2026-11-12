//! End-to-end tests for `shape naming corpus`: the temporal baseline, the
//! intra-corpus reference matrix and the inter-vendor matrix.
//!
//! Every case is hermetic: tiny linked programs and the tracked JavaScript
//! VM fixture, written to a temporary directory. The traced case uses a small
//! dispatch budget (two calls, about 0.3 s).

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use serde_json::Value;

const CLI: &str = env!("CARGO_BIN_EXE_shape");
const KERNEL: &str = include_str!("fixtures/kernels/javascript/kernel.raw.js");

/// Two proven roles: `agent_module` and `session_bootstrap`.
const LINKED_TWO_ROLES: &str = r#"(function main() {
  var registry = [1, 2, 3];
  function entry() { return registry[0]; }
  return { init: entry };
})();
"#;

/// The same capture with the `init` entrypoint removed: `session_bootstrap`
/// is no longer resolved.
const LINKED_ONE_ROLE: &str = r#"(function main() {
  var registry = [1, 2, 3];
  return registry[0];
})();
"#;

struct TemporaryDirectory {
    path: PathBuf,
}

impl TemporaryDirectory {
    fn new(label: &str) -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "shape-program-corpus-{label}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("temporary directory");
        Self { path }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }

    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.path(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("output directory");
        }
        fs::write(&path, contents).expect("temporary seed");
        path
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run(directory: &TemporaryDirectory, args: Vec<String>) -> Output {
    Command::new(CLI)
        .current_dir(&directory.path)
        .args(args)
        .output()
        .expect("shape naming corpus run")
}

fn run_args(directory: &TemporaryDirectory, args: &[&str]) -> Output {
    run(directory, args.iter().map(|arg| arg.to_string()).collect())
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("UTF-8 diagnostics")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("UTF-8 stdout")
}

fn json(output: &Output) -> Value {
    serde_json::from_str(&stdout(output)).expect("JSON result")
}

fn baseline_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).expect("baseline JSON"))
        .expect("valid baseline JSON")
}

fn path_string(path: &Path) -> String {
    path.to_str().expect("UTF-8 path").to_string()
}

/// A JSON result from a failing run still parses and reports the failure.
fn failure_json(output: &Output) -> Value {
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit 1, got {:?}\n{}",
        output.status.code(),
        stderr(output)
    );
    let document = json(output);
    assert_eq!(document["status"], "failure");
    document
}

#[test]
fn synthetic_baseline_update_clean_rerun_and_regressions() {
    let directory = TemporaryDirectory::new("baseline");
    let a = "a.js";
    let b = "b.js";
    directory.write(a, LINKED_TWO_ROLES);
    directory.write(b, LINKED_TWO_ROLES);
    let baseline = directory.path("baseline.json");
    let baseline_arg = path_string(&baseline);

    // 1. The first baseline is written and the accepted role map is printed.
    let update = run_args(
        &directory,
        &[
            "naming",
            "corpus",
            a,
            b,
            "--baseline",
            &baseline_arg,
            "--update-baseline",
        ],
    );
    assert!(update.status.success(), "{}", stderr(&update));
    let update_stderr = stderr(&update);
    assert!(update_stderr.contains("baseline: wrote"), "{update_stderr}");
    assert!(
        update_stderr.contains("new baseline"),
        "the accepted baseline is reviewable: {update_stderr}"
    );
    assert!(
        update_stderr.contains("agent_module -> createAgentModule"),
        "{update_stderr}"
    );

    let baseline_value = baseline_json(&baseline);
    assert_eq!(baseline_value["version"], "program-naming-corpus/1");
    let seeds = baseline_value["seeds"]
        .as_object()
        .expect("seed fingerprints");
    assert_eq!(seeds.len(), 2);
    let fingerprint = &seeds[a];
    assert_eq!(fingerprint["seed"]["mode"], "linked");
    assert_eq!(fingerprint["seed"]["label"], a);
    assert!(fingerprint["seed"]["vendor"].is_null());
    assert_eq!(
        fingerprint["plan"]["roles"]["agent_module"][0]["proposed_name"],
        "createAgentModule"
    );
    assert_eq!(
        fingerprint["plan"]["roles"]["session_bootstrap"][0]["proposed_name"],
        "bootstrapSessionRuntime"
    );
    assert_eq!(fingerprint["rename"]["parses"], true);
    assert!(fingerprint.get("trace").is_none());

    // 2. A second identical run is clean, and a second update is byte-identical.
    let clean = run_args(
        &directory,
        &["naming", "corpus", a, b, "--baseline", &baseline_arg],
    );
    assert!(clean.status.success(), "{}", stderr(&clean));
    assert!(
        stderr(&clean)
            .contains("baseline: 0 regressions, 0 improvements, 0 other changes, 0 input changes"),
        "{}",
        stderr(&clean)
    );

    let first_bytes = fs::read(&baseline).expect("baseline");
    let update_again = run_args(
        &directory,
        &[
            "naming",
            "corpus",
            a,
            b,
            "--baseline",
            &baseline_arg,
            "--update-baseline",
        ],
    );
    assert!(update_again.status.success(), "{}", stderr(&update_again));
    assert_eq!(
        fs::read(&baseline).expect("baseline"),
        first_bytes,
        "the baseline is byte-identical across runs"
    );
    assert!(
        stderr(&update_again).contains("no change"),
        "{}",
        stderr(&update_again)
    );
    let second = directory.path("baseline-2.json");
    let second_arg = path_string(&second);
    let fresh = run_args(
        &directory,
        &[
            "naming",
            "corpus",
            a,
            b,
            "--baseline",
            &second_arg,
            "--update-baseline",
        ],
    );
    assert!(fresh.status.success(), "{}", stderr(&fresh));
    assert_eq!(
        fs::read(&baseline).expect("baseline"),
        fs::read(&second).expect("second baseline"),
        "a fresh baseline is byte-identical"
    );

    // 3. The `--json` document carries the same schema and facts.
    let json_run = run_args(
        &directory,
        &[
            "naming",
            "corpus",
            a,
            b,
            "--baseline",
            &baseline_arg,
            "--json",
        ],
    );
    assert!(json_run.status.success(), "{}", stderr(&json_run));
    let document = json(&json_run);
    assert_eq!(document["version"], 1);
    assert_eq!(document["command"], "naming corpus");
    assert_eq!(document["status"], "complete");
    let result = &document["result"];
    assert_eq!(result["schema"], "program-naming-corpus/1");
    assert_eq!(result["seeds"][a], baseline_value["seeds"][a]);
    assert_eq!(result["baseline_updated"], false);
    assert!(result["aggregate"]["roles_resolved"][a].as_u64().unwrap() >= 2);
    assert!(result["regressions"].as_array().unwrap().is_empty());
    assert!(result["diff"][a]["regressions"]
        .as_array()
        .unwrap()
        .is_empty());

    let first_json = stdout(&json_run);
    let repeated = run_args(
        &directory,
        &[
            "naming",
            "corpus",
            a,
            b,
            "--baseline",
            &baseline_arg,
            "--json",
        ],
    );
    assert_eq!(
        first_json,
        stdout(&repeated),
        "the JSON result is byte-identical across runs"
    );

    // 4. A changed proposed name in the reviewed baseline is a regression.
    let changed = directory.path("baseline-name.json");
    let mut changed_value = baseline_json(&baseline);
    changed_value["seeds"][b]["plan"]["roles"]["agent_module"][0]["proposed_name"] =
        Value::String("makeAgentModule".into());
    fs::write(
        &changed,
        serde_json::to_string_pretty(&changed_value).expect("changed baseline") + "\n",
    )
    .expect("write changed baseline");
    let changed_arg = path_string(&changed);
    let name_change = run_args(
        &directory,
        &["naming", "corpus", b, "--baseline", &changed_arg],
    );
    assert_eq!(
        name_change.status.code(),
        Some(1),
        "{}",
        stderr(&name_change)
    );
    assert!(
        stderr(&name_change).contains("name changed: agent_module"),
        "{}",
        stderr(&name_change)
    );

    // 5. A source change that drops a resolved role is a regression.
    directory.write(a, LINKED_ONE_ROLE);
    let role_loss = run_args(
        &directory,
        &["naming", "corpus", a, b, "--baseline", &baseline_arg],
    );
    assert_eq!(role_loss.status.code(), Some(1), "{}", stderr(&role_loss));
    let role_loss_stderr = stderr(&role_loss);
    assert!(
        role_loss_stderr.contains("role lost: session_bootstrap"),
        "{role_loss_stderr}"
    );
    assert!(
        role_loss_stderr.contains("input changed: sha256"),
        "the changed input is reported separately: {role_loss_stderr}"
    );
    let role_loss_json = run_args(
        &directory,
        &[
            "naming",
            "corpus",
            a,
            b,
            "--baseline",
            &baseline_arg,
            "--json",
        ],
    );
    let failed = failure_json(&role_loss_json);
    assert!(!failed["result"]["regressions"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(!failed["result"]["input_changes"]
        .as_array()
        .unwrap()
        .is_empty());

    // 6. Accepting that change with --update-baseline prints the diff it accepts.
    let accepted = run_args(
        &directory,
        &[
            "naming",
            "corpus",
            a,
            b,
            "--baseline",
            &baseline_arg,
            "--update-baseline",
        ],
    );
    assert!(accepted.status.success(), "{}", stderr(&accepted));
    let accepted_stderr = stderr(&accepted);
    assert!(
        accepted_stderr.contains("baseline update: accepting"),
        "{accepted_stderr}"
    );
    assert!(
        accepted_stderr.contains("accepted regression: role lost: session_bootstrap"),
        "the accepted regression is reviewable: {accepted_stderr}"
    );
}

#[test]
fn a_byte_only_change_is_reported_but_is_not_a_regression() {
    let directory = TemporaryDirectory::new("bytes");
    let seed = "program.js";
    directory.write(seed, LINKED_TWO_ROLES);
    let baseline = directory.path("baseline.json");
    let baseline_arg = path_string(&baseline);
    let update = run_args(
        &directory,
        &[
            "naming",
            "corpus",
            seed,
            "--baseline",
            &baseline_arg,
            "--update-baseline",
        ],
    );
    assert!(update.status.success(), "{}", stderr(&update));

    // A comment changes every output byte but no role or name.
    directory.write(
        seed,
        &LINKED_TWO_ROLES.replacen(
            "(function main() {",
            "(function main() {\n  // harmless comment",
            1,
        ),
    );
    let compared = run_args(
        &directory,
        &["naming", "corpus", seed, "--baseline", &baseline_arg],
    );
    assert!(
        compared.status.success(),
        "bytes alone must not fail: {}",
        stderr(&compared)
    );
    let compared_stderr = stderr(&compared);
    assert!(
        compared_stderr.contains("renamed output changed: sha256"),
        "{compared_stderr}"
    );
    assert!(
        compared_stderr.contains("input changed: sha256"),
        "{compared_stderr}"
    );
    assert!(
        !compared_stderr.contains("regression:"),
        "{compared_stderr}"
    );
    let document = json(&run_args(
        &directory,
        &[
            "naming",
            "corpus",
            seed,
            "--baseline",
            &baseline_arg,
            "--json",
        ],
    ));
    assert_eq!(document["status"], "complete");
    assert!(document["result"]["seeds"][seed].get("plan").is_some());
}

#[test]
fn traced_seed_runs_the_chain_and_compares_a_clean_baseline() {
    let directory = TemporaryDirectory::new("traced");
    let seed = "kernel.raw.js";
    directory.write(seed, KERNEL);
    let out = directory.path("out");
    let out_arg = path_string(&out);
    let baseline = directory.path("baseline.json");
    let baseline_arg = path_string(&baseline);

    let first = run_args(
        &directory,
        &[
            "naming",
            "corpus",
            seed,
            "--steps",
            "20",
            "--out",
            &out_arg,
            "--baseline",
            &baseline_arg,
            "--update-baseline",
        ],
    );
    assert!(first.status.success(), "{}", stderr(&first));
    let first_stderr = stderr(&first);
    assert!(first_stderr.contains("traced mode"), "{first_stderr}");
    assert!(first_stderr.contains("baseline: wrote"), "{first_stderr}");

    for output in [
        "kernel.program.js",
        "kernel.renamed.js",
        "kernel.names.json",
    ] {
        assert!(out.join(output).is_file(), "missing {output}");
    }
    assert!(
        !out.join("kernel.annotated.js").exists(),
        "annotation is opt-in"
    );
    let renamed = fs::read_to_string(out.join("kernel.renamed.js")).expect("renamed source");
    assert!(
        renamed.contains("createAgentModule"),
        "the rename reached the linked program: {renamed}"
    );

    let baseline_value = baseline_json(&baseline);
    let fingerprint = &baseline_value["seeds"][seed];
    assert_eq!(fingerprint["seed"]["mode"], "traced");
    assert_eq!(fingerprint["seed"]["path"], seed);
    assert!(
        fingerprint["trace"]["traced"].as_u64().unwrap() > 0,
        "the traced program has traced functions: {fingerprint}"
    );
    assert_eq!(
        fingerprint["trace"]["program"]["sha256"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    assert_eq!(
        fingerprint["trace"]["program"]["bytes"],
        fs::metadata(out.join("kernel.program.js"))
            .expect("program metadata")
            .len()
    );

    let clean = run_args(
        &directory,
        &[
            "naming",
            "corpus",
            seed,
            "--steps",
            "20",
            "--baseline",
            &baseline_arg,
        ],
    );
    assert!(clean.status.success(), "{}", stderr(&clean));
    assert!(
        stderr(&clean).contains("baseline: 0 regressions"),
        "{}",
        stderr(&clean)
    );

    // Annotation is a separate output and a separate fingerprint layer.
    let annotated_out = directory.path("annotated");
    let annotated_out_arg = path_string(&annotated_out);
    let annotated_baseline = directory.path("annotated-baseline.json");
    let annotated_baseline_arg = path_string(&annotated_baseline);
    let annotated = run_args(
        &directory,
        &[
            "naming",
            "corpus",
            seed,
            "--steps",
            "20",
            "--out",
            &annotated_out_arg,
            "--annotate-anonymous",
            "--baseline",
            &annotated_baseline_arg,
            "--update-baseline",
        ],
    );
    assert!(annotated.status.success(), "{}", stderr(&annotated));
    assert!(annotated_out.join("kernel.annotated.js").is_file());
    let annotated_value = baseline_json(&annotated_baseline);
    assert_eq!(
        annotated_value["seeds"][seed]["anonymous"]["annotate_anonymous"],
        true
    );
}

#[test]
fn outputs_need_force_and_never_overwrite_a_seed() {
    let directory = TemporaryDirectory::new("force");
    let seed = "program.js";
    directory.write(seed, LINKED_TWO_ROLES);
    let out = directory.path("out");
    let out_arg = path_string(&out);

    let first = run_args(&directory, &["naming", "corpus", seed, "--out", &out_arg]);
    assert!(first.status.success(), "{}", stderr(&first));
    let renamed = out.join("program.renamed.js");
    assert!(renamed.is_file());
    let before = fs::read(&renamed).expect("renamed output");

    let second = run_args(&directory, &["naming", "corpus", seed, "--out", &out_arg]);
    assert_eq!(second.status.code(), Some(1), "{}", stderr(&second));
    assert!(
        stderr(&second).contains("already exists; pass --force"),
        "{}",
        stderr(&second)
    );
    assert_eq!(
        fs::read(&renamed).expect("unchanged output"),
        before,
        "a refused run writes nothing"
    );

    let forced = run_args(
        &directory,
        &["naming", "corpus", seed, "--out", &out_arg, "--force"],
    );
    assert!(forced.status.success(), "{}", stderr(&forced));
}

#[test]
fn manifest_vendor_metadata_and_vendor_matrix_are_advisory() {
    let directory = TemporaryDirectory::new("vendor");
    directory.write("case.js", LINKED_TWO_ROLES);
    directory.write("module-only.js", LINKED_ONE_ROLE);
    directory.write("plain.js", LINKED_ONE_ROLE);
    directory.write("peer.js", LINKED_TWO_ROLES);
    directory.write(
        "manifest.json",
        r#"{
  "version": "program-naming-corpus/1",
  "vendors": {
    "case": {"build": "build-7", "date": "2026-01-02", "seeds": ["case.js"]},
    "module-only": {"seeds": [{"path": "module-only.js", "label": "module-only-main"}]}
  },
  "seeds": ["plain.js"]
}"#,
    );
    let manifest = path_string(&directory.path("manifest.json"));

    let report = run_args(&directory, &["naming", "corpus", "--manifest", &manifest]);
    assert!(report.status.success(), "{}", stderr(&report));
    let report_stderr = stderr(&report);
    assert!(
        report_stderr.contains("vendors: case=1, module-only=1, unlabelled=1"),
        "{report_stderr}"
    );
    assert!(
        report_stderr.contains("roles in all vendors: agent_module"),
        "{report_stderr}"
    );
    assert!(
        report_stderr.contains("vendor-only roles: case: session_bootstrap"),
        "{report_stderr}"
    );

    let document = json(&run_args(
        &directory,
        &["naming", "corpus", "--manifest", &manifest, "--json"],
    ));
    assert_eq!(document["status"], "complete");
    let result = &document["result"];
    assert_eq!(result["seeds"]["case.js"]["seed"]["vendor"], "case");
    assert_eq!(result["seeds"]["case.js"]["seed"]["build"], "build-7");
    assert_eq!(result["seeds"]["case.js"]["seed"]["date"], "2026-01-02");
    assert_eq!(result["seeds"]["module-only-main"]["seed"]["vendor"], "module-only");
    assert_eq!(result["seeds"]["plain.js"]["seed"]["vendor"], Value::Null);
    assert_eq!(result["vendors"]["groups"]["case"][0], "case.js");
    assert!(result["vendors"]["missing_roles"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry[0] == "session_bootstrap"));

    // Reports are reports by default; the opt-in makes the same gap fail.
    let failure = run_args(
        &directory,
        &[
            "naming",
            "corpus",
            "--manifest",
            &manifest,
            "--fail-on",
            "missing-role",
        ],
    );
    assert_eq!(failure.status.code(), Some(1), "{}", stderr(&failure));
    assert!(
        stderr(&failure).contains("missing role: session_bootstrap"),
        "{}",
        stderr(&failure)
    );

    // Ad-hoc --vendor seeds use the same groups; with identical coverage the
    // opt-ins hold.
    let ad_hoc = run_args(
        &directory,
        &[
            "naming",
            "corpus",
            "--vendor",
            "one=case.js",
            "--vendor",
            "two=peer.js",
            "--fail-on",
            "missing-role",
            "--fail-on",
            "name-divergence",
        ],
    );
    assert!(ad_hoc.status.success(), "{}", stderr(&ad_hoc));
    assert!(
        stderr(&ad_hoc).contains("name divergences: none"),
        "{}",
        stderr(&ad_hoc)
    );
}

#[test]
fn reference_matrix_reports_coverage_deltas_and_can_fail_on_missing_roles() {
    let directory = TemporaryDirectory::new("reference");
    let reference = directory.path("reference.js");
    let narrower = directory.path("narrower.js");
    fs::write(&reference, LINKED_TWO_ROLES).expect("reference seed");
    fs::write(&narrower, LINKED_ONE_ROLE).expect("narrower seed");
    let reference_arg = path_string(&reference);
    let narrower_arg = path_string(&narrower);

    let report = run_args(
        &directory,
        &[
            "naming",
            "corpus",
            &reference_arg,
            &narrower_arg,
            "--reference",
            &reference_arg,
        ],
    );
    assert!(report.status.success(), "{}", stderr(&report));
    let report_stderr = stderr(&report);
    assert!(
        report_stderr.contains("reference reference.js"),
        "{report_stderr}"
    );
    assert!(report_stderr.contains("reference-only"), "{report_stderr}");
    assert!(
        report_stderr.contains("session_bootstrap"),
        "{report_stderr}"
    );
    assert!(
        report_stderr.contains("coverage (roles resolved)"),
        "{report_stderr}"
    );
    assert!(
        report_stderr
            .contains("roles missing from a compared seed: session_bootstrap (narrower.js)"),
        "{report_stderr}"
    );

    let failure = run_args(
        &directory,
        &[
            "naming",
            "corpus",
            &reference_arg,
            &narrower_arg,
            "--reference",
            "reference.js",
            "--fail-on",
            "missing-role",
        ],
    );
    assert_eq!(failure.status.code(), Some(1), "{}", stderr(&failure));
    assert!(
        stderr(&failure).contains("missing role: session_bootstrap"),
        "{}",
        stderr(&failure)
    );

    let markdown = run_args(
        &directory,
        &[
            "naming",
            "corpus",
            &reference_arg,
            &narrower_arg,
            "--reference",
            "reference.js",
            "--format",
            "markdown",
        ],
    );
    assert!(markdown.status.success(), "{}", stderr(&markdown));
    assert!(
        stdout(&markdown).contains("### Reference: `reference.js`"),
        "{}",
        stdout(&markdown)
    );
    assert!(
        stdout(&markdown).contains("| role | reference |"),
        "{}",
        stdout(&markdown)
    );

    let document = json(&run_args(
        &directory,
        &[
            "naming",
            "corpus",
            &reference_arg,
            &narrower_arg,
            "--reference",
            "reference.js",
            "--json",
        ],
    ));
    assert_eq!(document["status"], "complete");
    assert_eq!(document["result"]["reference"]["reference"], "reference.js");
    assert!(document["result"]["reference"]["missing_roles"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry[0] == "session_bootstrap"));
}

#[test]
fn a_malformed_or_versionless_baseline_is_rejected() {
    let directory = TemporaryDirectory::new("baseline-version");
    directory.write("seed.js", LINKED_TWO_ROLES);
    let baseline = directory.write("baseline.json", "{\"seeds\": {}}");
    let baseline_arg = path_string(&baseline);
    let output = run_args(
        &directory,
        &["naming", "corpus", "seed.js", "--baseline", &baseline_arg],
    );
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(
        stderr(&output).contains("is not a program-naming-corpus/1 baseline"),
        "{}",
        stderr(&output)
    );
}
