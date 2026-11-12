//! `shape naming corpus`: run the whole naming chain for each seed, compare
//! the result with a reviewed baseline, and compare seeds with each other.
//!
//! A `*.raw.js` seed is traced into one linked program (`--trace` forces this
//! for any seed); any other seed is taken as an already-linked program. The
//! same program then goes through the naming plan, the rename, and — with
//! `--annotate-anonymous` — the annotation, so one fingerprint records every
//! layer. Nothing executes the recovered JavaScript: the command parses,
//! plans and rewrites source only.
//!
//! Three comparisons share that fingerprint:
//!
//! * **temporal**: [`compare_seed`] against a reviewed golden file
//!   (`--baseline` / `--update-baseline`); regressions exit 1;
//! * **intra-corpus**: [`reference_report`] against one chosen seed
//!   (`--reference`), for "is one capture overfit?";
//! * **inter-vendor**: [`vendor_report`] across `vendor` groups from a
//!   manifest (`--manifest`) or `--vendor NAME=PATH`.
//!
//! The comparison reports are reports: exit 0 unless `--fail-on` opts in.
//!
//! The fingerprint schema is versioned [`CORPUS_SCHEMA`]. It carries no
//! timestamp, absolute path, host or build id: a baseline is a human-reviewed
//! golden file that a review can diff across machines.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
    time::Instant,
};

use serde_json::{json, Map, Value};

use shape::naming::semantic_names::{NamingCandidate, SkipKind};
use shape::naming::semantic_names::SemanticNamingPlan;
use shape::naming::program::{
    apply_anonymous_names_mapped, rename_semantic_names_with_options, AnonymousNameAction,
    AnonymousNameOptions, AnonymousNameOutcome, AnonymousNamesResult, LocalNameOptions,
    SemanticRenameOptions, SemanticRenameResult,
};
use shape::trace::program::{link_program, LinkOptions};

pub(crate) const CORPUS_SCHEMA: &str = "program-naming-corpus/1";

const UNLABELLED: &str = "unlabelled";

pub(crate) const CORPUS_USAGE: &str = "usage: shape naming corpus <source>... [--out DIR] [--json] [--baseline FILE] [--update-baseline]
       [--manifest FILE] [--vendor NAME=PATH]... [--reference LABEL|PATH]
       [--fail-on missing-role|name-divergence]... [--format markdown]
       [--steps N] [--no-thread-cleanup] [--name-anonymous] [--annotate-anonymous]
       [--no-local-names] [--trace] [--force]

Run the whole naming chain for each seed and compare the fingerprints. A
`*.raw.js` seed is traced into a linked program (thread cleanup on; --steps is
the dispatch limit); any other seed is read as an already-linked program, and
--trace forces tracing for it too. Each linked/read program is then planned,
renamed, and annotated when asked.

  --out DIR           write <stem>.program.js (traced seeds only),
                      <stem>.renamed.js, <stem>.names.json (the naming plan)
                      and, with --annotate-anonymous, <stem>.annotated.js.
                      An existing output needs --force.
  --json              one structured JSON result on stdout
  --baseline FILE     compare with this reviewed baseline; regressions exit 1
  --update-baseline   write the fingerprints to --baseline instead, printing
                      the per-seed diff being accepted
  --manifest FILE     a program-naming-corpus/1 JSON manifest of seed paths and
                      their vendor/label/build/date metadata. Paths are
                      relative to the manifest; a seed with no vendor is
                      grouped as \"unlabelled\".
  --vendor NAME=PATH  add one ad-hoc seed under a vendor (repeatable)
  --reference LABEL|PATH
                      compare every other seed with this one: the role matrix,
                      shared roles with different proposed names, and coverage
                      deltas
  --fail-on WHAT      turn a comparison report into a failure (repeatable):
                      missing-role or name-divergence. Applies to --reference
                      and to the vendor matrix.
  --format markdown   print the comparison matrix as markdown on stdout
                      instead of a compact matrix on stderr
  --steps N           the most dispatch steps per function when tracing
                      (default 1000000)
  --no-thread-cleanup do not clean each emitted thread while tracing (cleanup
                      is on by default)
  --name-anonymous    give provably safe anonymous function expressions their
                      plan name in renamed.js
  --annotate-anonymous
                      also write annotated.js: renames, anonymous names and
                      one /* <name> */ comment per plan-named anonymous node
  --trace             trace this seed even when its name does not end in
                      .raw.js
  --force             replace existing outputs

Exit codes: 0 no baseline regression and no opted-in report failure, 1 a
regression / opted-in failure / runtime error, 2 usage error.

The temporal baseline is the durable replacement for the research target lists
and per-package coverage reports: review the first baseline once by hand, then
let CI run --baseline without --update-baseline.";

/// Where a seed's metadata came from: positional, manifest or `--vendor`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SeedSpec {
    path: PathBuf,
    vendor: Option<String>,
    label: Option<String>,
    build: Option<String>,
    date: Option<String>,
}

/// A seed with its stable baseline key.
#[derive(Clone, Debug)]
struct ResolvedSeed {
    spec: SeedSpec,
    label: String,
    file: String,
}

/// A comparison report is a report unless an opt-in says otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum FailOn {
    MissingRole,
    NameDivergence,
}

pub(crate) struct CorpusArgs {
    json: bool,
    seeds: Vec<SeedSpec>,
    manifest: Option<PathBuf>,
    vendors: Vec<(String, PathBuf)>,
    out: Option<PathBuf>,
    baseline: Option<PathBuf>,
    update: bool,
    steps: usize,
    thread_cleanup: bool,
    name_anonymous: bool,
    annotate_anonymous: bool,
    local_names: bool,
    force: bool,
    /// `--trace`: trace a seed even when its name does not end in `.raw.js`.
    force_trace: bool,
    markdown: bool,
    reference: Option<String>,
    fail_on: BTreeSet<FailOn>,
}

/// Run `shape naming corpus` from its arguments.
pub(crate) fn run_corpus(args: Vec<String>) -> ExitCode {
    let json = args.iter().any(|arg| arg == "--json");
    let args = match parse(args) {
        Ok(Some(args)) => args,
        Ok(None) => {
            println!("{CORPUS_USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            if !json {
                eprintln!("{CORPUS_USAGE}");
            }
            return failure(json, &error, 2);
        }
    };
    match corpus(&args) {
        Ok(code) => code,
        Err(error) => failure(json, &error, 1),
    }
}

fn failure(json: bool, error: &str, code: u8) -> ExitCode {
    eprintln!("shape naming corpus: {error}");
    if json {
        if let Err(write) = write_json("failure", json!({"error": error, "exit_code": code})) {
            eprintln!("shape naming corpus: {write}");
            return ExitCode::from(1);
        }
    }
    ExitCode::from(code)
}

fn write_json(status: &str, result: Value) -> Result<(), String> {
    let document = json!({
        "version": 1,
        "command": "naming corpus",
        "status": status,
        "result": result,
    });
    crate::cli::write_stdout_line(&document.to_string())
        .map(|_| ())
        .map_err(|error| format!("failed to write JSON to stdout: {error}"))
}

fn corpus(args: &CorpusArgs) -> Result<ExitCode, String> {
    let started = Instant::now();
    let seeds = resolve_seeds(args)?;
    output_plan(args, &seeds)?;
    // Seeds share nothing: process them on up to one core each. A traced
    // seed still walks its functions in parallel, so leave it roughly its
    // share of the machine.
    let cores = std::thread::available_parallelism().map_or(1, usize::from);
    let seed_threads = cores.min(seeds.len()).max(1);
    let jobs = (cores / seed_threads).max(1);
    let processed = crate::cli::trace::corpus::parallel_map(&seeds, seed_threads, |seed| {
        process_seed(seed, args, jobs)
    });
    let mut results = Vec::with_capacity(seeds.len());
    for (seed, outcome) in seeds.iter().zip(processed) {
        let result = match outcome {
            Some(result) => result?,
            None => return Err(format!("{}: the seed run panicked", seed.file)),
        };
        results.push(result);
    }
    for result in &results {
        for (path, contents) in &result.files {
            write_file(path, contents)?;
        }
    }
    let current: BTreeMap<String, Value> = results
        .iter()
        .map(|result| (result.label.clone(), result.fingerprint.clone()))
        .collect();

    for (label, fingerprint) in &current {
        print_seed_summary(label, fingerprint);
    }
    let aggregate = aggregate(&current);
    print_aggregate(&aggregate);

    // Temporal comparison: a reviewed baseline.
    let mut diff_by_seed: BTreeMap<String, SeedDiff> = BTreeMap::new();
    let mut regressions = Vec::new();
    let mut improvements = Vec::new();
    let mut changes = Vec::new();
    let mut input_changes = Vec::new();
    let mut baseline_updated = false;
    match (&args.baseline, args.update) {
        (Some(path), true) => {
            let previous = read_baseline_if_present(path)?;
            print_update_review(path, previous.as_ref(), &current);
            write_baseline(path, &current)?;
            baseline_updated = true;
        }
        (Some(path), false) => {
            let previous = read_baseline(path)?;
            for (label, fingerprint) in &current {
                let diff = match previous.get(label) {
                    Some(old) => compare_seed(old, fingerprint),
                    None => SeedDiff {
                        improvements: vec!["new seed (no baseline fingerprint)".into()],
                        ..SeedDiff::default()
                    },
                };
                print_seed_diff(label, &diff);
                for regression in &diff.regressions {
                    regressions.push(format!("{label}: {regression}"));
                }
                for improvement in &diff.improvements {
                    improvements.push(format!("{label}: {improvement}"));
                }
                for change in &diff.changes {
                    changes.push(format!("{label}: {change}"));
                }
                for input_change in &diff.input_changes {
                    input_changes.push(format!("{label}: {input_change}"));
                }
                diff_by_seed.insert(label.clone(), diff);
            }
            for label in previous
                .keys()
                .filter(|label| !current.contains_key(*label))
            {
                let change = "not in this run (baseline seed dropped)".to_string();
                eprintln!("seed {label}: changed: {change}");
                changes.push(format!("{label}: {change}"));
            }
            eprintln!(
                "baseline: {} regressions, {} improvements, {} other changes, {} input changes",
                regressions.len(),
                improvements.len(),
                changes.len(),
                input_changes.len()
            );
        }
        (None, _) => {}
    }
    if !regressions.is_empty() {
        for regression in &regressions {
            eprintln!("regression: {regression}");
        }
    }

    // Intra-corpus and inter-vendor reports never fail unless asked to.
    let reference = match &args.reference {
        Some(reference) => {
            let report = reference_report(reference, &seeds, &current)?;
            emit_report(&report.render(args.markdown), args.markdown, args.json);
            Some(report)
        }
        None => None,
    };
    let vendors = vendor_report(&current);
    if vendors.groups.len() > 1 || args.manifest.is_some() || !args.vendors.is_empty() {
        emit_report(&vendors.render(args.markdown), args.markdown, args.json);
    }
    let mut report_failures = Vec::new();
    if !args.fail_on.is_empty() {
        if reference.is_none() && vendors.groups.len() <= 1 {
            return Err("--fail-on needs --reference or at least two vendor groups".into());
        }
        if args.fail_on.contains(&FailOn::MissingRole) {
            if let Some(report) = &reference {
                for (role, label) in &report.missing {
                    report_failures.push(format!(
                        "missing role: {role} (reference {}, missing from {label})",
                        report.reference
                    ));
                }
            }
            for (role, present) in &vendors.missing {
                report_failures.push(format!(
                    "missing role: {role} (only in {})",
                    present.join(", ")
                ));
            }
        }
        if args.fail_on.contains(&FailOn::NameDivergence) {
            if let Some(report) = &reference {
                for (role, reference_names, label, names) in &report.divergences {
                    report_failures.push(format!(
                        "name divergence: {role}: reference {reference_names:?}, {label} {names:?}"
                    ));
                }
            }
            for (role, by_vendor) in &vendors.divergences {
                report_failures.push(format!(
                    "name divergence: {role}: {}",
                    by_vendor
                        .iter()
                        .map(|(vendor, names)| format!("{vendor} {names:?}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        for failure in &report_failures {
            eprintln!("failure: {failure}");
        }
    }

    let status = if regressions.is_empty() && report_failures.is_empty() {
        if current.values().any(|fingerprint| {
            fingerprint["trace"]["stopped"]
                .as_array()
                .is_some_and(|stopped| !stopped.is_empty())
        }) {
            "partial"
        } else {
            "complete"
        }
    } else {
        "failure"
    };
    if args.json {
        let aggregate_json = json!({
            "roles_resolved": aggregate.roles_resolved,
            "roles_varying": aggregate.roles_varying,
        });
        let diff_json: BTreeMap<&String, Value> = diff_by_seed
            .iter()
            .map(|(label, diff)| (label, diff.to_json()))
            .collect();
        write_json(
            status,
            json!({
                "schema": CORPUS_SCHEMA,
                "seeds": current,
                "diff": diff_json,
                "regressions": regressions,
                "improvements": improvements,
                "changes": changes,
                "input_changes": input_changes,
                "baseline_updated": baseline_updated,
                "report_failures": report_failures,
                "aggregate": aggregate_json,
                "reference": reference.as_ref().map(ReferenceReport::to_json),
                "vendors": vendors.to_json(),
            }),
        )?;
    }
    eprintln!("time: {:.2?}", started.elapsed());
    if !regressions.is_empty() || !report_failures.is_empty() {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

// ---------------------------------------------------------------------------
// Seeds, outputs and one seed's chain
// ---------------------------------------------------------------------------

fn resolve_seeds(args: &CorpusArgs) -> Result<Vec<ResolvedSeed>, String> {
    let mut specs = args.seeds.clone();
    if let Some(manifest) = &args.manifest {
        specs.extend(read_manifest(manifest)?);
    }
    for (vendor, path) in &args.vendors {
        specs.push(SeedSpec {
            path: path.clone(),
            vendor: Some(vendor.clone()),
            label: None,
            build: None,
            date: None,
        });
    }
    if specs.is_empty() {
        return Err("missing a seed path".into());
    }
    let mut labels = BTreeSet::new();
    let mut resolved = Vec::with_capacity(specs.len());
    for spec in specs {
        let file = spec
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(|| format!("seed path {} has no file name", spec.path.display()))?;
        let label = spec.label.clone().unwrap_or_else(|| file.clone());
        if label.is_empty() {
            return Err(format!("seed {} has an empty label", spec.path.display()));
        }
        if !labels.insert(label.clone()) {
            return Err(format!(
                "two seeds share the label {label:?}; give one an explicit label"
            ));
        }
        resolved.push(ResolvedSeed { spec, label, file });
    }
    Ok(resolved)
}

/// Read a `program-naming-corpus/1` manifest: vendor groups and/or plain
/// seeds, every path relative to the manifest.
fn read_manifest(path: &Path) -> Result<Vec<SeedSpec>, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("cannot read manifest {}: {error}", path.display()))?;
    let document: Value = serde_json::from_str(&text)
        .map_err(|error| format!("{} is not a JSON manifest: {error}", path.display()))?;
    if document["version"] != json!(CORPUS_SCHEMA) {
        return Err(format!(
            "{} is not a {CORPUS_SCHEMA} manifest (found {})",
            path.display(),
            document["version"]
        ));
    }
    let base = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let resolve = |seed: &str| base.join(seed);
    let mut specs = Vec::new();
    let vendors = document["vendors"].as_object().cloned().unwrap_or_default();
    for (vendor, group) in vendors {
        let (group_build, group_date, seeds) = match &group {
            Value::Array(seeds) => (None, None, seeds),
            Value::Object(fields) => (
                fields.get("build").and_then(Value::as_str),
                fields.get("date").and_then(Value::as_str),
                fields
                    .get("seeds")
                    .and_then(Value::as_array)
                    .ok_or_else(|| {
                        format!("{}: vendor {vendor:?} has no seeds array", path.display())
                    })?,
            ),
            _ => {
                return Err(format!(
                    "{}: vendor {vendor:?} must be an array or an object with seeds",
                    path.display()
                ))
            }
        };
        for seed in seeds {
            let mut spec = manifest_seed(seed, &resolve, path)?;
            spec.vendor = Some(vendor.clone());
            spec.build = spec.build.or_else(|| group_build.map(str::to_string));
            spec.date = spec.date.or_else(|| group_date.map(str::to_string));
            specs.push(spec);
        }
    }
    if let Some(seeds) = document["seeds"].as_array() {
        for seed in seeds {
            specs.push(manifest_seed(seed, &resolve, path)?);
        }
    }
    Ok(specs)
}

fn manifest_seed(
    seed: &Value,
    resolve: &impl Fn(&str) -> PathBuf,
    manifest: &Path,
) -> Result<SeedSpec, String> {
    match seed {
        Value::String(seed) => Ok(SeedSpec {
            path: resolve(seed),
            vendor: None,
            label: None,
            build: None,
            date: None,
        }),
        Value::Object(fields) => {
            let path = fields.get("path").and_then(Value::as_str).ok_or_else(|| {
                format!("{}: a seed entry has no path string", manifest.display())
            })?;
            let string = |name: &str| fields.get(name).and_then(Value::as_str).map(str::to_string);
            Ok(SeedSpec {
                path: resolve(path),
                vendor: string("vendor"),
                label: string("label"),
                build: string("build"),
                date: string("date"),
            })
        }
        _ => Err(format!(
            "{}: a seed entry must be a path string or an object",
            manifest.display()
        )),
    }
}

/// Output files per seed, checked before any work: nothing is overwritten
/// without `--force`, and no output may be a seed.
fn output_plan(args: &CorpusArgs, seeds: &[ResolvedSeed]) -> Result<Vec<SeedOutputs>, String> {
    let Some(directory) = &args.out else {
        return Ok(seeds.iter().map(|_| SeedOutputs::default()).collect());
    };
    let mut plan = Vec::with_capacity(seeds.len());
    let mut seen = BTreeSet::new();
    for seed in seeds {
        let stem = output_stem(&seed.file);
        let traced = args.force_trace || is_raw_js(&seed.file);
        let outputs = SeedOutputs {
            program: traced.then(|| directory.join(format!("{stem}.program.js"))),
            renamed: directory.join(format!("{stem}.renamed.js")),
            names: directory.join(format!("{stem}.names.json")),
            annotated: args
                .annotate_anonymous
                .then(|| directory.join(format!("{stem}.annotated.js"))),
        };
        for path in outputs.paths() {
            if !seen.insert(path.clone()) {
                return Err(format!(
                    "two seeds share the output {}; give one a distinct label",
                    path.display()
                ));
            }
            if path.as_path() == seed.spec.path.as_path()
                || same_path(path.as_path(), seed.spec.path.as_path())
            {
                return Err(format!(
                    "refusing to overwrite the seed {} with its own output",
                    seed.spec.path.display()
                ));
            }
            if !args.force && path.exists() {
                return Err(format!(
                    "output {} already exists; pass --force to replace it",
                    path.display()
                ));
            }
        }
        plan.push(outputs);
    }
    Ok(plan)
}

#[derive(Default)]
struct SeedOutputs {
    program: Option<PathBuf>,
    renamed: PathBuf,
    names: PathBuf,
    annotated: Option<PathBuf>,
}

impl SeedOutputs {
    fn paths(&self) -> Vec<PathBuf> {
        let mut paths = vec![self.renamed.clone(), self.names.clone()];
        paths.extend(self.program.clone());
        paths.extend(self.annotated.clone());
        paths
    }
}

/// The stem of the output names: `kernel.raw.js` -> `kernel`, `program.js` ->
/// `program`.
fn output_stem(file: &str) -> String {
    if let Some(stem) = file.strip_suffix(".raw.js") {
        return stem.to_string();
    }
    Path::new(file)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| file.to_string())
}

fn is_raw_js(file: &str) -> bool {
    file.ends_with(".raw.js")
}

struct SeedResult {
    label: String,
    fingerprint: Value,
    files: Vec<(PathBuf, String)>,
}

fn process_seed(seed: &ResolvedSeed, args: &CorpusArgs, jobs: usize) -> Result<SeedResult, String> {
    let input = crate::cli::read_source(&seed.spec.path)
        .map_err(|error| format!("cannot read {}: {error}", seed.spec.path.display()))?;
    let traced = args.force_trace || is_raw_js(&seed.file);
    let linked = if traced {
        Some(
            link_program(&LinkOptions {
                seed: seed.spec.path.clone(),
                report_dir: None,
                steps: args.steps,
                dump_stages: None,
                // The product default: clean each thread while tracing.
                thread_cleanup: args.thread_cleanup,
                no_cleanup: false,
                check_reuse: false,
                overrides: shape::BranchOverrides::default(),
                jobs: Some(jobs),
                quiet: true,
            })
            .map_err(|error| format!("trace {}: {error}", seed.spec.path.display()))?,
        )
    } else {
        None
    };
    let program_source = linked
        .as_ref()
        .map(|linked| linked.code.as_str())
        .unwrap_or(&input.text);
    let rename = rename_semantic_names_with_options(
        program_source,
        SemanticRenameOptions {
            anonymous: AnonymousNameOptions {
                name: args.name_anonymous,
                annotate: false,
            },
            local_names: if args.local_names {
                LocalNameOptions::enabled()
            } else {
                LocalNameOptions::disabled()
            },
        },
    )
    .map_err(|error| format!("plan/rename {}: {error}", seed.spec.path.display()))?;
    let annotated = if args.annotate_anonymous {
        Some(
            apply_anonymous_names_mapped(
                program_source,
                &rename.transformed_source,
                &rename.plan.anonymous_functions,
                AnonymousNameOptions {
                    name: false,
                    annotate: true,
                },
            )
            .map_err(|error| format!("annotate {}: {error}", seed.spec.path.display()))?,
        )
    } else {
        None
    };
    let parses = parses(&rename.transformed_source);
    let fingerprint = fingerprint(
        seed,
        &input,
        traced,
        linked.as_ref(),
        &rename,
        annotated.as_ref(),
        parses,
    );
    let mut files = Vec::new();
    if args.out.is_some() {
        let outputs = output_plan_for(args, seed);
        if let Some(path) = &outputs.program {
            if let Some(linked) = &linked {
                files.push((path.clone(), linked.code.clone()));
            }
        }
        files.push((outputs.renamed.clone(), rename.transformed_source.clone()));
        let names = serde_json::to_string_pretty(&rename.plan)
            .map_err(|error| format!("cannot serialize the naming plan: {error}"))?;
        files.push((outputs.names.clone(), names + "\n"));
        if let (Some(path), Some(annotated)) = (&outputs.annotated, &annotated) {
            files.push((path.clone(), annotated.transformed_source.clone()));
        }
    }
    Ok(SeedResult {
        label: seed.label.clone(),
        fingerprint,
        files,
    })
}

/// The output paths for one seed, computed again for the worker. Validation
/// (duplicates, overwrite) happened once in [`output_plan`].
fn output_plan_for(args: &CorpusArgs, seed: &ResolvedSeed) -> SeedOutputs {
    let Some(directory) = &args.out else {
        return SeedOutputs::default();
    };
    let stem = output_stem(&seed.file);
    SeedOutputs {
        program: (args.force_trace || is_raw_js(&seed.file))
            .then(|| directory.join(format!("{stem}.program.js"))),
        renamed: directory.join(format!("{stem}.renamed.js")),
        names: directory.join(format!("{stem}.names.json")),
        annotated: args
            .annotate_anonymous
            .then(|| directory.join(format!("{stem}.annotated.js"))),
    }
}

fn write_file(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    fs::write(path, contents).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

fn parses(source: &str) -> bool {
    let allocator = oxc_allocator::Allocator::default();
    shape::analysis::parse_program(&allocator, source).is_ok()
}

fn same_path(left: &Path, right: &Path) -> bool {
    normalize_path(left) == normalize_path(right)
}

/// Canonicalize what exists; for an output that does not exist yet, resolve
/// its parent and keep the file name.
fn normalize_path(path: &Path) -> PathBuf {
    if let Ok(canonical) = fs::canonicalize(path) {
        return canonical;
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    match (fs::canonicalize(parent), path.file_name()) {
        (Ok(parent), Some(name)) => parent.join(name),
        _ => path.to_path_buf(),
    }
}

// ---------------------------------------------------------------------------
// Fingerprint
// ---------------------------------------------------------------------------

fn fingerprint(
    seed: &ResolvedSeed,
    input: &crate::cli::SourceBytes,
    traced: bool,
    linked: Option<&shape::trace::program::LinkedProgram>,
    rename: &SemanticRenameResult,
    annotated: Option<&AnonymousNamesResult>,
    parses: bool,
) -> Value {
    let plan = &rename.plan;
    let roles = plan_roles(plan);
    let anonymous_entries = plan_anonymous_entries(plan);
    let ambiguous = plan_ambiguous(plan);
    let skipped = plan_skipped(plan);
    let mut fingerprint = json!({
        "seed": {
            "path": seed.file,
            "sha256": crate::cli::sha256_hex(&input.bytes),
            "bytes": input.bytes.len(),
            "mode": if traced { "traced" } else { "linked" },
            "label": seed.label,
            "vendor": seed.spec.vendor,
            "build": seed.spec.build,
            "date": seed.spec.date,
        },
        "plan": {
            "candidates": plan.candidates.len(),
            "anonymous": plan.anonymous_functions.len(),
            "ambiguous": ambiguous,
            "skipped": skipped,
            "roles": roles,
            "anonymous_entries": anonymous_entries,
        },
        "rename": {
            "applied": rename.applied,
            "skipped": rename.skipped,
            "output": {
                "sha256": rename.transformed_sha256,
                "bytes": rename.transformed_source.len(),
            },
            "parses": parses,
        },
        "anonymous": anonymous_fingerprint(rename, annotated),
    });
    if let Some(linked) = linked {
        fingerprint["trace"] = json!({
            "traced": linked.traced,
            "stopped": linked
                .stopped
                .iter()
                .map(|(function, reason)| json!({"function": function, "reason": reason}))
                .collect::<Vec<_>>(),
            "program": {
                "sha256": crate::cli::sha256_hex(linked.code.as_bytes()),
                "bytes": linked.code.len(),
            },
            "kernel_globals": linked.kernel_globals,
        });
    }
    fingerprint
}

fn plan_roles(plan: &SemanticNamingPlan) -> Value {
    let mut by_role: BTreeMap<&str, Vec<&NamingCandidate>> = BTreeMap::new();
    for candidate in &plan.candidates {
        by_role
            .entry(candidate.role.as_str())
            .or_default()
            .push(candidate);
    }
    let mut roles = Map::new();
    for (role, mut candidates) in by_role {
        candidates.sort_by(|left, right| {
            left.entity
                .id
                .cmp(&right.entity.id)
                .then_with(|| left.proposed_name.cmp(&right.proposed_name))
        });
        let entries: Vec<Value> = candidates
            .iter()
            .map(|candidate| {
                json!({
                    "entity_id": candidate.entity.id,
                    "proposed_name": candidate.proposed_name,
                    "binding_kind": candidate.binding_kind,
                })
            })
            .collect();
        roles.insert(role.to_string(), Value::Array(entries));
    }
    Value::Object(roles)
}

fn plan_anonymous_entries(plan: &SemanticNamingPlan) -> Value {
    let mut entries = plan.anonymous_functions.clone();
    entries.sort_by(|left, right| left.id.cmp(&right.id));
    let mut map = Map::new();
    for entry in entries {
        map.insert(
            entry.id,
            json!({
                "role": entry.role,
                "proposed_name": entry.proposed_name,
                "arrow": entry.arrow,
            }),
        );
    }
    Value::Object(map)
}

fn plan_ambiguous(plan: &SemanticNamingPlan) -> Value {
    let mut entries: Vec<(String, String)> = plan
        .ambiguous
        .iter()
        .map(|ambiguity| (ambiguity.role.clone(), ambiguity.reason.clone()))
        .collect();
    entries.sort();
    Value::Array(
        entries
            .into_iter()
            .map(|(role, reason)| json!({"role": role, "reason": reason}))
            .collect(),
    )
}

fn plan_skipped(plan: &SemanticNamingPlan) -> Value {
    let mut entries: Vec<(String, &'static str, String)> = plan
        .skipped
        .iter()
        .map(|skip| (skip.role.clone(), skip_kind(skip.kind), skip.reason.clone()))
        .collect();
    entries.sort();
    Value::Array(
        entries
            .into_iter()
            .map(|(role, kind, reason)| json!({"role": role, "kind": kind, "reason": reason}))
            .collect(),
    )
}

fn skip_kind(kind: SkipKind) -> &'static str {
    match kind {
        SkipKind::Refused => "refused",
        SkipKind::RecognizedNoEditableBinding => "recognized_no_editable_binding",
        SkipKind::Unproven => "unproven",
    }
}

fn anonymous_fingerprint(
    rename: &SemanticRenameResult,
    annotated: Option<&AnonymousNamesResult>,
) -> Value {
    let name_outcomes = if rename.anonymous.options.name {
        anonymous_outcomes(rename.anonymous.of(AnonymousNameAction::Name))
    } else {
        Vec::new()
    };
    let annotate_outcomes = annotated
        .map(|annotated| anonymous_outcomes(annotated.of(AnonymousNameAction::Annotate)))
        .unwrap_or_default();
    json!({
        "name_anonymous": rename.anonymous.options.name,
        "annotate_anonymous": annotated.is_some(),
        "named": rename.anonymous.named,
        "annotated": annotated.map_or(0, |annotated| annotated.annotated),
        "skipped": rename.anonymous.skipped + annotated.map_or(0, |annotated| annotated.skipped),
        "outcomes": {
            "name": name_outcomes,
            "annotate": annotate_outcomes,
        },
    })
}

fn anonymous_outcomes<'a>(outcomes: impl Iterator<Item = &'a AnonymousNameOutcome>) -> Vec<Value> {
    let mut outcomes: Vec<&AnonymousNameOutcome> = outcomes.collect();
    outcomes.sort_by(|left, right| left.entry.id.cmp(&right.entry.id));
    outcomes
        .into_iter()
        .map(|outcome| {
            json!({
                "id": outcome.entry.id,
                "role": outcome.entry.role,
                "proposed_name": outcome.entry.proposed_name,
                "applied": outcome.applied,
                "skip_reason": outcome.skip_reason,
                "node_kind": outcome.node_kind,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Reading a fingerprint's semantic layers
// ---------------------------------------------------------------------------

/// Every resolved role and its proposed names, merging rename candidates and
/// anonymous-function names. This is the semantic layer the reports compare;
/// byte offsets (`entity_id`) are deliberately not part of it.
fn role_names(fingerprint: &Value) -> BTreeMap<String, BTreeSet<String>> {
    let mut roles: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (role, entries) in fingerprint["plan"]["roles"]
        .as_object()
        .into_iter()
        .flatten()
    {
        let names = roles.entry(role.clone()).or_default();
        for entry in entries.as_array().into_iter().flatten() {
            if let Some(name) = entry["proposed_name"].as_str() {
                names.insert(name.to_string());
            }
        }
    }
    for entry in fingerprint["plan"]["anonymous_entries"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(_, entry)| entry)
    {
        if let (Some(role), Some(name)) = (entry["role"].as_str(), entry["proposed_name"].as_str())
        {
            roles
                .entry(role.to_string())
                .or_default()
                .insert(name.to_string());
        }
    }
    roles.retain(|_, names| !names.is_empty());
    roles
}

/// A binding-kind change for the same role and name is reported but is not a
/// regression: the name still resolves.
fn role_kinds(fingerprint: &Value) -> BTreeMap<(String, String), String> {
    let mut kinds = BTreeMap::new();
    for (role, entries) in fingerprint["plan"]["roles"]
        .as_object()
        .into_iter()
        .flatten()
    {
        for entry in entries.as_array().into_iter().flatten() {
            if let (Some(name), Some(kind)) = (
                entry["proposed_name"].as_str(),
                entry["binding_kind"].as_str(),
            ) {
                kinds.insert((role.clone(), name.to_string()), kind.to_string());
            }
        }
    }
    for entry in fingerprint["plan"]["anonymous_entries"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(_, entry)| entry)
    {
        if let (Some(role), Some(name)) = (entry["role"].as_str(), entry["proposed_name"].as_str())
        {
            kinds.insert(
                (role.to_string(), name.to_string()),
                "anonymous".to_string(),
            );
        }
    }
    kinds
}

fn refusals(fingerprint: &Value) -> BTreeSet<(String, String)> {
    fingerprint["plan"]["skipped"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|skip| skip["kind"] == json!("refused"))
        .filter_map(|skip| {
            Some((
                skip["role"].as_str()?.to_string(),
                skip["reason"].as_str()?.to_string(),
            ))
        })
        .collect()
}

fn non_refusal_skips(fingerprint: &Value) -> BTreeSet<(String, String)> {
    fingerprint["plan"]["skipped"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|skip| skip["kind"] != json!("refused"))
        .filter_map(|skip| {
            Some((
                skip["role"].as_str()?.to_string(),
                skip["kind"].as_str()?.to_string(),
            ))
        })
        .collect()
}

fn ambiguities(fingerprint: &Value) -> BTreeSet<(String, String)> {
    fingerprint["plan"]["ambiguous"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|ambiguity| {
            Some((
                ambiguity["role"].as_str()?.to_string(),
                ambiguity["reason"].as_str()?.to_string(),
            ))
        })
        .collect()
}

struct TraceFacts {
    traced: u64,
    stopped: BTreeMap<String, String>,
    program_sha256: String,
    program_bytes: u64,
    kernel_globals: BTreeSet<String>,
}

fn trace_facts(fingerprint: &Value) -> Option<TraceFacts> {
    let trace = fingerprint.get("trace")?;
    let stopped = trace["stopped"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|stopped| {
            Some((
                stopped["function"].as_str()?.to_string(),
                stopped["reason"].as_str()?.to_string(),
            ))
        })
        .collect();
    Some(TraceFacts {
        traced: trace["traced"].as_u64().unwrap_or(0),
        stopped,
        program_sha256: trace["program"]["sha256"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        program_bytes: trace["program"]["bytes"].as_u64().unwrap_or(0),
        kernel_globals: trace["kernel_globals"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
    })
}

// ---------------------------------------------------------------------------
// Temporal baseline
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct SeedDiff {
    regressions: Vec<String>,
    improvements: Vec<String>,
    changes: Vec<String>,
    input_changes: Vec<String>,
}

impl SeedDiff {
    fn counts(&self) -> (usize, usize, usize, usize) {
        (
            self.regressions.len(),
            self.improvements.len(),
            self.changes.len(),
            self.input_changes.len(),
        )
    }

    fn to_json(&self) -> Value {
        json!({
            "regressions": self.regressions,
            "improvements": self.improvements,
            "changes": self.changes,
            "input_changes": self.input_changes,
        })
    }
}

/// What moved from `old` to `new`. Only the regressions fail a command; an
/// added role or name, fewer stops and a changed byte are reported.
fn compare_seed(old: &Value, new: &Value) -> SeedDiff {
    let mut diff = SeedDiff::default();
    let old_seed = &old["seed"];
    let new_seed = &new["seed"];
    if old_seed["sha256"] != new_seed["sha256"] {
        diff.input_changes.push(format!(
            "input changed: sha256 {} -> {} ({} -> {} bytes)",
            short(old_seed["sha256"].as_str().unwrap_or_default()),
            short(new_seed["sha256"].as_str().unwrap_or_default()),
            old_seed["bytes"].as_u64().unwrap_or(0),
            new_seed["bytes"].as_u64().unwrap_or(0),
        ));
    }
    if old_seed["mode"] != new_seed["mode"] {
        diff.changes.push(format!(
            "mode: {} -> {}",
            old_seed["mode"].as_str().unwrap_or("?"),
            new_seed["mode"].as_str().unwrap_or("?")
        ));
    }
    if old_seed["vendor"] != new_seed["vendor"] {
        diff.changes.push(format!(
            "vendor: {} -> {}",
            text(&old_seed["vendor"]),
            text(&new_seed["vendor"])
        ));
    }
    compare_trace(old, new, &mut diff);
    compare_plan(old, new, &mut diff);
    compare_rename(old, new, &mut diff);
    compare_anonymous(old, new, &mut diff);
    diff
}

fn compare_trace(old: &Value, new: &Value, diff: &mut SeedDiff) {
    match (trace_facts(old), trace_facts(new)) {
        (Some(old), Some(new)) => {
            if new.traced < old.traced {
                diff.regressions.push(format!(
                    "trace: traced count fell {} -> {}",
                    old.traced, new.traced
                ));
            } else if new.traced > old.traced {
                diff.improvements.push(format!(
                    "trace: traced count rose {} -> {}",
                    old.traced, new.traced
                ));
            }
            if new.stopped.len() > old.stopped.len() {
                diff.regressions.push(format!(
                    "trace: stopped count rose {} -> {}",
                    old.stopped.len(),
                    new.stopped.len()
                ));
            } else if new.stopped.len() < old.stopped.len() {
                diff.improvements.push(format!(
                    "trace: stopped count fell {} -> {}",
                    old.stopped.len(),
                    new.stopped.len()
                ));
            }
            for (function, reason) in &new.stopped {
                match old.stopped.get(function) {
                    None => diff
                        .regressions
                        .push(format!("function now stops: {function}: {reason}")),
                    Some(old_reason) if old_reason != reason => diff.changes.push(format!(
                        "function stops differently: {function}: {old_reason} -> {reason}"
                    )),
                    _ => {}
                }
            }
            for function in old
                .stopped
                .keys()
                .filter(|function| !new.stopped.contains_key(*function))
            {
                diff.improvements
                    .push(format!("function no longer stops: {function}"));
            }
            if old.program_sha256 != new.program_sha256 {
                diff.changes.push(format!(
                    "program changed: sha256 {} -> {} ({} -> {} bytes)",
                    short(&old.program_sha256),
                    short(&new.program_sha256),
                    old.program_bytes,
                    new.program_bytes
                ));
            }
            for lost in old.kernel_globals.difference(&new.kernel_globals) {
                diff.changes
                    .push(format!("kernel global no longer read: {lost}"));
            }
            for gained in new.kernel_globals.difference(&old.kernel_globals) {
                diff.changes.push(format!("kernel global read: {gained}"));
            }
        }
        (Some(_), None) => diff
            .changes
            .push("mode: trace facts no longer compared (seed is linked)".into()),
        (None, Some(_)) => diff.changes.push("mode: seed is now traced".into()),
        (None, None) => {}
    }
}

fn compare_plan(old: &Value, new: &Value, diff: &mut SeedDiff) {
    let old_roles = role_names(old);
    let new_roles = role_names(new);
    for (role, old_names) in &old_roles {
        match new_roles.get(role) {
            None => diff.regressions.push(format!(
                "role lost: {role} (was {})",
                joined_names(old_names)
            )),
            Some(new_names) => {
                for name in old_names.difference(new_names) {
                    diff.regressions.push(format!(
                        "name changed: {role}: {name} -> {}",
                        joined_names(new_names)
                    ));
                }
                for name in new_names.difference(old_names) {
                    diff.improvements
                        .push(format!("name added: {role}: {name}"));
                }
            }
        }
    }
    for (role, new_names) in &new_roles {
        if !old_roles.contains_key(role) {
            diff.improvements
                .push(format!("role added: {role}: {}", joined_names(new_names)));
        }
    }
    let old_kinds = role_kinds(old);
    let new_kinds = role_kinds(new);
    for (key, old_kind) in &old_kinds {
        if let Some(new_kind) = new_kinds.get(key) {
            if old_kind != new_kind {
                diff.changes.push(format!(
                    "binding kind changed: {} {}: {old_kind} -> {new_kind}",
                    key.0, key.1
                ));
            }
        }
    }

    let old_refusals = refusals(old);
    let new_refusals = refusals(new);
    for (role, reason) in new_refusals.difference(&old_refusals) {
        diff.regressions
            .push(format!("new refusal: {role}: {reason}"));
    }
    for (role, reason) in old_refusals.difference(&new_refusals) {
        diff.improvements
            .push(format!("refusal resolved: {role}: {reason}"));
    }
    let old_ambiguities = ambiguities(old);
    let new_ambiguities = ambiguities(new);
    for (role, reason) in new_ambiguities.difference(&old_ambiguities) {
        diff.regressions
            .push(format!("new ambiguity: {role}: {reason}"));
    }
    for (role, reason) in old_ambiguities.difference(&new_ambiguities) {
        diff.improvements
            .push(format!("ambiguity resolved: {role}: {reason}"));
    }
    let old_skips = non_refusal_skips(old);
    let new_skips = non_refusal_skips(new);
    for (role, kind) in new_skips.difference(&old_skips) {
        diff.changes.push(format!("new plan skip: {role} ({kind})"));
    }
    for (role, kind) in old_skips.difference(&new_skips) {
        diff.changes
            .push(format!("plan skip gone: {role} ({kind})"));
    }
}

fn compare_rename(old: &Value, new: &Value, diff: &mut SeedDiff) {
    let old_rename = &old["rename"];
    let new_rename = &new["rename"];
    let old_parses = old_rename["parses"].as_bool().unwrap_or(true);
    let new_parses = new_rename["parses"].as_bool().unwrap_or(true);
    if old_parses && !new_parses {
        diff.regressions
            .push("renamed output no longer parses".into());
    } else if !old_parses && new_parses {
        diff.improvements.push("renamed output parses again".into());
    }
    let old_output = &old_rename["output"];
    let new_output = &new_rename["output"];
    if old_output["sha256"] != new_output["sha256"] {
        diff.changes.push(format!(
            "renamed output changed: sha256 {} -> {} ({} -> {} bytes)",
            short(old_output["sha256"].as_str().unwrap_or_default()),
            short(new_output["sha256"].as_str().unwrap_or_default()),
            old_output["bytes"].as_u64().unwrap_or(0),
            new_output["bytes"].as_u64().unwrap_or(0)
        ));
    }
    if old_rename["applied"] != new_rename["applied"] {
        diff.changes.push(format!(
            "rename: applied {} -> {}",
            old_rename["applied"].as_u64().unwrap_or(0),
            new_rename["applied"].as_u64().unwrap_or(0)
        ));
    }
    if old_rename["skipped"] != new_rename["skipped"] {
        diff.changes.push(format!(
            "rename: skipped {} -> {}",
            old_rename["skipped"].as_u64().unwrap_or(0),
            new_rename["skipped"].as_u64().unwrap_or(0)
        ));
    }
}

fn compare_anonymous(old: &Value, new: &Value, diff: &mut SeedDiff) {
    let old = &old["anonymous"];
    let new = &new["anonymous"];
    for (field, label) in [
        ("named", "named"),
        ("annotated", "annotated"),
        ("skipped", "skipped"),
    ] {
        if old[field] != new[field] {
            diff.changes.push(format!(
                "anonymous {label}: {} -> {}",
                old[field].as_u64().unwrap_or(0),
                new[field].as_u64().unwrap_or(0)
            ));
        }
    }
}

fn joined_names(names: &BTreeSet<String>) -> String {
    names.iter().cloned().collect::<Vec<_>>().join(", ")
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(12)]
}

fn text(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_string)
}

fn read_baseline_if_present(path: &Path) -> Result<Option<Map<String, Value>>, String> {
    if !path.exists() {
        return Ok(None);
    }
    read_baseline(path).map(Some)
}

fn read_baseline(path: &Path) -> Result<Map<String, Value>, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let document: Value = serde_json::from_str(&text)
        .map_err(|error| format!("{} is not a baseline: {error}", path.display()))?;
    if document["version"] != json!(CORPUS_SCHEMA) {
        return Err(format!(
            "{} is not a {CORPUS_SCHEMA} baseline (found version {})",
            path.display(),
            document["version"]
        ));
    }
    document["seeds"]
        .as_object()
        .cloned()
        .ok_or_else(|| format!("{} has no seeds object", path.display()))
}

fn write_baseline(path: &Path, seeds: &BTreeMap<String, Value>) -> Result<(), String> {
    let document = json!({"version": CORPUS_SCHEMA, "seeds": seeds});
    let json = serde_json::to_string_pretty(&document)
        .map_err(|error| format!("cannot serialize the baseline: {error}"))?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    fs::write(path, json + "\n")
        .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    eprintln!("baseline: wrote {}", path.display());
    Ok(())
}

/// A baseline is a reviewable act: print every change (or, for a new seed,
/// its full role -> name map) before accepting it.
fn print_update_review(
    path: &Path,
    previous: Option<&Map<String, Value>>,
    current: &BTreeMap<String, Value>,
) {
    eprintln!(
        "baseline update: accepting {} seed fingerprint(s) into {}",
        current.len(),
        path.display()
    );
    for (label, fingerprint) in current {
        print_seed_identity(label, fingerprint);
        match previous.and_then(|previous| previous.get(label)) {
            None => {
                eprintln!("  new baseline: no previous fingerprint");
                print_role_map(fingerprint);
            }
            Some(old) => {
                let diff = compare_seed(old, fingerprint);
                let (regressions, improvements, changes, input_changes) = diff.counts();
                eprintln!(
                    "  accepting {regressions} regression(s), {improvements} improvement(s), {changes} change(s), {input_changes} input change(s)"
                );
                for regression in &diff.regressions {
                    eprintln!("    accepted regression: {regression}");
                }
                for improvement in &diff.improvements {
                    eprintln!("    accepted improvement: {improvement}");
                }
                for change in &diff.changes {
                    eprintln!("    accepted change: {change}");
                }
                for input_change in &diff.input_changes {
                    eprintln!("    accepted input change: {input_change}");
                }
                if diff.counts() == (0, 0, 0, 0) {
                    eprintln!("    no change");
                }
            }
        }
    }
    if let Some(previous) = previous {
        for label in previous
            .keys()
            .filter(|label| !current.contains_key(*label))
        {
            eprintln!("  seed {label}: dropped from the baseline (not in this run)");
        }
    }
}

fn print_seed_identity(label: &str, fingerprint: &Value) {
    let seed = &fingerprint["seed"];
    eprintln!(
        "seed {label}: {} {} (vendor {}, sha256 {}, {} bytes)",
        seed["mode"].as_str().unwrap_or("?"),
        seed["path"].as_str().unwrap_or("?"),
        seed["vendor"].as_str().unwrap_or(UNLABELLED),
        short(seed["sha256"].as_str().unwrap_or_default()),
        seed["bytes"].as_u64().unwrap_or(0)
    );
}

fn print_role_map(fingerprint: &Value) {
    for (role, names) in role_names(fingerprint) {
        eprintln!("    {role} -> {}", joined_names(&names));
    }
}

fn print_seed_summary(label: &str, fingerprint: &Value) {
    let plan = &fingerprint["plan"];
    let rename = &fingerprint["rename"];
    let roles = role_names(fingerprint).len();
    let mut summary = format!(
        "seed {label}: {} mode, {} candidate(s), {} anonymous, {} role(s) resolved, {} refused, {} ambiguous, rename {}/{} applied/skipped, parses {}",
        fingerprint["seed"]["mode"].as_str().unwrap_or("?"),
        plan["candidates"].as_u64().unwrap_or(0),
        plan["anonymous"].as_u64().unwrap_or(0),
        roles,
        plan["skipped"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|skip| skip["kind"] == json!("refused"))
            .count(),
        plan["ambiguous"].as_array().map_or(0, Vec::len),
        rename["applied"].as_u64().unwrap_or(0),
        rename["skipped"].as_u64().unwrap_or(0),
        rename["parses"].as_bool().unwrap_or(false),
    );
    if let Some(trace) = trace_facts(fingerprint) {
        summary.push_str(&format!(
            "; trace {} traced, {} stopped, program {} bytes",
            trace.traced,
            trace.stopped.len(),
            trace.program_bytes
        ));
    }
    eprintln!("{summary}");
}

fn print_seed_diff(label: &str, diff: &SeedDiff) {
    let (regressions, improvements, changes, input_changes) = diff.counts();
    eprintln!(
        "seed {label}: {regressions} regression(s), {improvements} improvement(s), {changes} change(s), {input_changes} input change(s)"
    );
    for regression in &diff.regressions {
        eprintln!("  regression: {regression}");
    }
    for improvement in &diff.improvements {
        eprintln!("  improvement: {improvement}");
    }
    for change in &diff.changes {
        eprintln!("  changed: {change}");
    }
    for input_change in &diff.input_changes {
        eprintln!("  input: {input_change}");
    }
}

struct Aggregate {
    roles_resolved: BTreeMap<String, usize>,
    roles_varying: Vec<String>,
}

fn aggregate(seeds: &BTreeMap<String, Value>) -> Aggregate {
    let roles: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> = seeds
        .iter()
        .map(|(label, fingerprint)| (label.clone(), role_names(fingerprint)))
        .collect();
    let roles_resolved = roles
        .iter()
        .map(|(label, roles)| (label.clone(), roles.len()))
        .collect();
    let mut all_roles = BTreeSet::new();
    for names in roles.values() {
        all_roles.extend(names.keys().cloned());
    }
    let empty = BTreeSet::new();
    let mut roles_varying = Vec::new();
    for role in all_roles {
        let mut sets = roles
            .values()
            .map(|names| names.get(&role).unwrap_or(&empty));
        let Some(first) = sets.next() else {
            continue;
        };
        if sets.any(|names| names != first) {
            roles_varying.push(role);
        }
    }
    Aggregate {
        roles_resolved,
        roles_varying,
    }
}

fn print_aggregate(aggregate: &Aggregate) {
    let per_seed: Vec<String> = aggregate
        .roles_resolved
        .iter()
        .map(|(label, count)| format!("{label}={count}"))
        .collect();
    eprintln!("roles resolved: {}", per_seed.join(", "));
    if aggregate.roles_varying.is_empty() {
        eprintln!("roles varying across seeds: none");
    } else {
        eprintln!(
            "roles varying across seeds: {}",
            aggregate.roles_varying.join(", ")
        );
    }
}

// ---------------------------------------------------------------------------
// Intra-corpus: one reference seed
// ---------------------------------------------------------------------------

struct ReferenceReport {
    reference: String,
    reference_vendor: String,
    labels: Vec<String>,
    rows: Vec<ReferenceRow>,
    coverage: BTreeMap<String, usize>,
    /// A role the reference resolves and a compared seed does not.
    missing: Vec<(String, String)>,
    /// A role resolved in both with different proposed names.
    divergences: Vec<(String, String, String, String)>,
}

struct ReferenceRow {
    role: String,
    reference_names: BTreeSet<String>,
    cells: Vec<(String, BTreeSet<String>, &'static str)>,
}

fn reference_report(
    reference: &str,
    seeds: &[ResolvedSeed],
    fingerprints: &BTreeMap<String, Value>,
) -> Result<ReferenceReport, String> {
    let label = resolve_reference(reference, seeds)?;
    let reference_fingerprint = &fingerprints[&label];
    let reference_roles = role_names(reference_fingerprint);
    let reference_meta = &reference_fingerprint["seed"];
    let mut compared: Vec<&ResolvedSeed> =
        seeds.iter().filter(|seed| seed.label != label).collect();
    let vendor_of = |seed: &ResolvedSeed| {
        fingerprints[&seed.label]["seed"]["vendor"]
            .as_str()
            .unwrap_or(UNLABELLED)
            .to_string()
    };
    compared.sort_by(|left, right| {
        vendor_of(left)
            .cmp(&vendor_of(right))
            .then_with(|| left.label.cmp(&right.label))
    });
    let mut labels = vec![label.clone()];
    labels.extend(compared.iter().map(|seed| seed.label.clone()));
    let coverage = seeds
        .iter()
        .map(|seed| {
            (
                seed.label.clone(),
                role_names(&fingerprints[&seed.label]).len(),
            )
        })
        .collect();

    let mut roles = reference_roles.keys().cloned().collect::<BTreeSet<_>>();
    for seed in &compared {
        roles.extend(role_names(&fingerprints[&seed.label]).keys().cloned());
    }
    let mut rows = Vec::new();
    let mut missing = Vec::new();
    let mut divergences = Vec::new();
    for role in roles {
        let reference_names = reference_roles.get(&role).cloned().unwrap_or_default();
        let mut cells = Vec::new();
        for seed in &compared {
            let seed_roles = role_names(&fingerprints[&seed.label]);
            let names = seed_roles.get(&role).cloned().unwrap_or_default();
            let state = match (reference_names.is_empty(), names.is_empty()) {
                (false, false) => "both",
                (false, true) => "reference-only",
                (true, false) => "seed-only",
                (true, true) => continue,
            };
            if state == "reference-only" {
                missing.push((role.clone(), seed.label.clone()));
            }
            if state == "both" && names != reference_names {
                divergences.push((
                    role.clone(),
                    joined_names(&reference_names),
                    seed.label.clone(),
                    joined_names(&names),
                ));
            }
            cells.push((seed.label.clone(), names, state));
        }
        rows.push(ReferenceRow {
            role,
            reference_names,
            cells,
        });
    }
    let reference_vendor = reference_meta["vendor"]
        .as_str()
        .unwrap_or(UNLABELLED)
        .to_string();
    Ok(ReferenceReport {
        reference: label,
        reference_vendor,
        labels,
        rows,
        coverage,
        missing,
        divergences,
    })
}

fn resolve_reference(reference: &str, seeds: &[ResolvedSeed]) -> Result<String, String> {
    if let Some(seed) = seeds.iter().find(|seed| seed.label == reference) {
        return Ok(seed.label.clone());
    }
    let canonical = fs::canonicalize(reference).ok();
    if let Some(seed) = seeds.iter().find(|seed| {
        seed.file == reference
            || seed.spec.path == Path::new(reference)
            || canonical.as_ref().is_some_and(|canonical| {
                fs::canonicalize(&seed.spec.path).ok().as_ref() == Some(canonical)
            })
    }) {
        return Ok(seed.label.clone());
    }
    Err(format!(
        "unknown --reference {reference:?}; seeds are {}",
        seeds
            .iter()
            .map(|seed| seed.label.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

impl ReferenceReport {
    fn render(&self, markdown: bool) -> String {
        if markdown {
            let mut out = format!(
                "### Reference: `{}` (vendor `{}`)\n\n",
                self.reference, self.reference_vendor
            );
            out.push_str("| role | reference |");
            for label in self.labels.iter().filter(|label| **label != self.reference) {
                out.push_str(&format!(" {label} |"));
            }
            out.push('\n');
            out.push_str("| --- | --- |");
            for _ in self.labels.iter().filter(|label| **label != self.reference) {
                out.push_str(" --- |");
            }
            out.push('\n');
            for row in &self.rows {
                out.push_str(&format!(
                    "| `{}` | {} |",
                    row.role,
                    matrix_names(&row.reference_names)
                ));
                for (_, names, state) in &row.cells {
                    out.push_str(&format!(" {} ({state}) |", matrix_names(names)));
                }
                out.push('\n');
            }
            out.push_str(&format!(
                "\n- coverage (roles resolved): {}\n",
                self.labels
                    .iter()
                    .filter_map(|label| Some(format!("{label}={}", self.coverage.get(label)?)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            out.push_str(&format!(
                "- missing from a compared seed: {}\n",
                if self.missing.is_empty() {
                    "none".to_string()
                } else {
                    self.missing
                        .iter()
                        .map(|(role, label)| format!("{role} ({label})"))
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ));
            out.push_str(&format!(
                "- name divergences: {}\n",
                if self.divergences.is_empty() {
                    "none".to_string()
                } else {
                    self.divergences
                        .iter()
                        .map(|(role, reference, label, names)| {
                            format!("{role}: reference {reference}, {label} {names}")
                        })
                        .collect::<Vec<_>>()
                        .join("; ")
                }
            ));
            return out;
        }
        let mut out = format!(
            "reference {} (vendor {}): {} comparison seed(s)\n",
            self.reference,
            self.reference_vendor,
            self.labels.len().saturating_sub(1)
        );
        let width = self
            .rows
            .iter()
            .map(|row| row.role.len())
            .chain(std::iter::once(4))
            .max()
            .unwrap_or(4);
        let reference_width = self
            .rows
            .iter()
            .map(|row| matrix_names(&row.reference_names).len())
            .chain(std::iter::once("reference".len()))
            .max()
            .unwrap_or(9);
        let cell_width = self
            .rows
            .iter()
            .flat_map(|row| {
                row.cells
                    .iter()
                    .map(|(_, names, state)| matrix_cell(names, state).len())
            })
            .chain(std::iter::once("seed".len()))
            .max()
            .unwrap_or(4);
        out.push_str(&format!(
            "{:width$}  {:>reference_width$}",
            "role",
            "reference",
            width = width,
            reference_width = reference_width
        ));
        for label in self.labels.iter().filter(|label| **label != self.reference) {
            out.push_str(&format!("  {label:>cell_width$}"));
        }
        out.push('\n');
        for row in &self.rows {
            out.push_str(&format!(
                "{:width$}  {:>reference_width$}",
                row.role,
                matrix_names(&row.reference_names),
                width = width,
                reference_width = reference_width
            ));
            for (_, names, state) in &row.cells {
                out.push_str(&format!("  {:>cell_width$}", matrix_cell(names, state)));
            }
            out.push('\n');
        }
        out.push_str(&format!(
            "coverage (roles resolved): {}\n",
            self.labels
                .iter()
                .filter_map(|label| Some(format!("{label}={}", self.coverage.get(label)?)))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        out.push_str(&format!(
            "roles missing from a compared seed: {}\n",
            if self.missing.is_empty() {
                "none".to_string()
            } else {
                self.missing
                    .iter()
                    .map(|(role, label)| format!("{role} ({label})"))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        ));
        out.push_str(&format!(
            "name divergences: {}\n",
            if self.divergences.is_empty() {
                "none".to_string()
            } else {
                self.divergences
                    .iter()
                    .map(|(role, reference, label, names)| {
                        format!("{role}: reference {reference}, {label} {names}")
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            }
        ));
        out
    }

    fn to_json(&self) -> Value {
        json!({
            "reference": self.reference,
            "vendor": self.reference_vendor,
            "labels": self.labels,
            "coverage": self.coverage,
            "missing_roles": self.missing,
            "name_divergences": self.divergences,
            "rows": self.rows.iter().map(|row| json!({
                "role": row.role,
                "reference": row.reference_names,
                "cells": row.cells.iter().map(|(label, names, state)| json!({
                    "label": label, "names": names, "state": state,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })
    }
}

fn matrix_cell(names: &BTreeSet<String>, state: &str) -> String {
    if state == "both" {
        matrix_names(names)
    } else {
        format!("{} ({state})", matrix_names(names))
    }
}

fn matrix_names(names: &BTreeSet<String>) -> String {
    if names.is_empty() {
        "-".to_string()
    } else {
        names.iter().cloned().collect::<Vec<_>>().join(",")
    }
}

// ---------------------------------------------------------------------------
// Inter-vendor matrix
// ---------------------------------------------------------------------------

struct VendorReport {
    groups: BTreeMap<String, Vec<String>>,
    vendor_roles: BTreeMap<String, BTreeSet<String>>,
    rows: Vec<VendorRow>,
    /// A role resolved in some vendor group but not in every group.
    missing: Vec<(String, Vec<String>)>,
    /// A role resolved in more than one group with different names.
    divergences: Vec<(String, Vec<(String, String)>)>,
}

struct VendorRow {
    role: String,
    cells: Vec<(String, BTreeSet<String>)>,
}

fn vendor_report(fingerprints: &BTreeMap<String, Value>) -> VendorReport {
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (label, fingerprint) in fingerprints {
        let vendor = fingerprint["seed"]["vendor"]
            .as_str()
            .unwrap_or(UNLABELLED)
            .to_string();
        groups.entry(vendor).or_default().push(label.clone());
    }
    for labels in groups.values_mut() {
        labels.sort();
    }
    let mut vendor_roles: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut vendor_names: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();
    for (vendor, labels) in &groups {
        let roles = vendor_roles.entry(vendor.clone()).or_default();
        let names = vendor_names.entry(vendor.clone()).or_default();
        for label in labels {
            for (role, role_names) in role_names(&fingerprints[label]) {
                roles.insert(role.clone());
                names.entry(role).or_default().extend(role_names);
            }
        }
    }
    let mut all_roles = BTreeSet::new();
    for roles in vendor_roles.values() {
        all_roles.extend(roles.iter().cloned());
    }
    let mut missing = Vec::new();
    let mut divergences = Vec::new();
    let mut rows = Vec::new();
    for role in all_roles {
        let mut cells = Vec::new();
        for vendor in groups.keys() {
            let names = vendor_names
                .get(vendor)
                .and_then(|names| names.get(&role))
                .cloned()
                .unwrap_or_default();
            cells.push((vendor.clone(), names));
        }
        let present: Vec<String> = cells
            .iter()
            .filter(|(_, names)| !names.is_empty())
            .map(|(vendor, _)| vendor.clone())
            .collect();
        if present.len() < groups.len() && !groups.is_empty() {
            missing.push((role.clone(), present));
        }
        let mut by_vendor: Vec<(String, String)> = cells
            .iter()
            .filter(|(_, names)| !names.is_empty())
            .map(|(vendor, names)| (vendor.clone(), joined_names(names)))
            .collect();
        let distinct: BTreeSet<&String> = by_vendor.iter().map(|(_, names)| names).collect();
        if distinct.len() > 1 {
            by_vendor.sort();
            divergences.push((role.clone(), by_vendor));
        }
        rows.push(VendorRow { role, cells });
    }
    VendorReport {
        groups,
        vendor_roles,
        rows,
        missing,
        divergences,
    }
}

impl VendorReport {
    fn render(&self, markdown: bool) -> String {
        let summary = self
            .groups
            .iter()
            .map(|(vendor, labels)| format!("{vendor}={}", labels.len()))
            .collect::<Vec<_>>()
            .join(", ");
        let all_vendors: Vec<String> = self.groups.keys().cloned().collect();
        let in_all: Vec<String> = self
            .rows
            .iter()
            .filter(|row| row.cells.iter().all(|(_, names)| !names.is_empty()))
            .map(|row| row.role.clone())
            .collect();
        let mut vendor_only: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (role, present) in &self.missing {
            if present.len() == 1 {
                vendor_only
                    .entry(present[0].clone())
                    .or_default()
                    .push(role.clone());
            }
        }
        if markdown {
            let mut out = format!("### Vendors: {summary}\n\n");
            out.push_str("| role |");
            for vendor in &all_vendors {
                out.push_str(&format!(" {vendor} |"));
            }
            out.push('\n');
            out.push_str("| --- |");
            for _ in &all_vendors {
                out.push_str(" --- |");
            }
            out.push('\n');
            for row in &self.rows {
                out.push_str(&format!("| `{}` |", row.role));
                for (_, names) in &row.cells {
                    out.push_str(&format!(" {} |", matrix_names(names)));
                }
                out.push('\n');
            }
            out.push_str(&format!(
                "\n- roles in all vendors: {}\n",
                if in_all.is_empty() {
                    "none".to_string()
                } else {
                    in_all.join(", ")
                }
            ));
            out.push_str(&format!(
                "- vendor-only roles: {}\n",
                if vendor_only.is_empty() {
                    "none".to_string()
                } else {
                    vendor_only
                        .iter()
                        .map(|(vendor, roles)| format!("{vendor}: {}", roles.join(", ")))
                        .collect::<Vec<_>>()
                        .join("; ")
                }
            ));
            out.push_str(&format!(
                "- name divergences: {}\n",
                if self.divergences.is_empty() {
                    "none".to_string()
                } else {
                    self.divergences
                        .iter()
                        .map(|(role, by_vendor)| {
                            format!(
                                "{role}: {}",
                                by_vendor
                                    .iter()
                                    .map(|(vendor, names)| format!("{vendor}={names}"))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("; ")
                }
            ));
            return out;
        }
        let mut out = format!("vendors: {summary}\n");
        let width = self
            .rows
            .iter()
            .map(|row| row.role.len())
            .chain(std::iter::once(4))
            .max()
            .unwrap_or(4);
        let cell_width = self
            .rows
            .iter()
            .flat_map(|row| row.cells.iter().map(|(_, names)| matrix_names(names).len()))
            .chain(std::iter::once("vendor".len()))
            .max()
            .unwrap_or(4);
        out.push_str(&format!("{:width$}", "role", width = width));
        for vendor in &all_vendors {
            out.push_str(&format!("  {vendor:>cell_width$}"));
        }
        out.push('\n');
        for row in &self.rows {
            out.push_str(&format!("{:width$}", row.role, width = width));
            for (_, names) in &row.cells {
                out.push_str(&format!("  {:>cell_width$}", matrix_names(names)));
            }
            out.push('\n');
        }
        out.push_str(&format!(
            "roles in all vendors: {}\n",
            if in_all.is_empty() {
                "none".to_string()
            } else {
                in_all.join(", ")
            }
        ));
        out.push_str(&format!(
            "vendor-only roles: {}\n",
            if vendor_only.is_empty() {
                "none".to_string()
            } else {
                vendor_only
                    .iter()
                    .map(|(vendor, roles)| format!("{vendor}: {}", roles.join(", ")))
                    .collect::<Vec<_>>()
                    .join("; ")
            }
        ));
        out.push_str(&format!(
            "name divergences: {}\n",
            if self.divergences.is_empty() {
                "none".to_string()
            } else {
                self.divergences
                    .iter()
                    .map(|(role, by_vendor)| {
                        format!(
                            "{role}: {}",
                            by_vendor
                                .iter()
                                .map(|(vendor, names)| format!("{vendor}={names}"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ")
            }
        ));
        out
    }

    fn to_json(&self) -> Value {
        json!({
            "groups": self.groups,
            "vendor_roles": self.vendor_roles,
            "missing_roles": self.missing,
            "name_divergences": self.divergences,
            "rows": self.rows.iter().map(|row| json!({
                "role": row.role,
                "cells": row.cells.iter().map(|(vendor, names)| json!({
                    "vendor": vendor, "names": names,
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })
    }
}

fn emit_report(report: &str, markdown: bool, json: bool) {
    if json {
        return;
    }
    if markdown {
        let _ = crate::cli::write_stdout_line(report.trim_end_matches('\n'));
    } else {
        for line in report.lines() {
            eprintln!("{line}");
        }
    }
}

// ---------------------------------------------------------------------------
// Arguments
// ---------------------------------------------------------------------------

fn parse(args: Vec<String>) -> Result<Option<CorpusArgs>, String> {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        return Ok(None);
    }
    let mut parsed = CorpusArgs {
        json: false,
        seeds: Vec::new(),
        manifest: None,
        vendors: Vec::new(),
        out: None,
        baseline: None,
        update: false,
        steps: 1_000_000,
        thread_cleanup: true,
        name_anonymous: false,
        annotate_anonymous: false,
        local_names: true,
        force: false,
        force_trace: false,
        markdown: false,
        reference: None,
        fail_on: BTreeSet::new(),
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let mut value = |name: &str| {
            args.next()
                .ok_or_else(|| format!("missing value after {name}"))
        };
        match arg.as_str() {
            "--json" => parsed.json = true,
            "--out" => parsed.out = Some(PathBuf::from(value("--out")?)),
            "--baseline" => parsed.baseline = Some(PathBuf::from(value("--baseline")?)),
            "--update-baseline" => parsed.update = true,
            "--manifest" => {
                if parsed
                    .manifest
                    .replace(PathBuf::from(value("--manifest")?))
                    .is_some()
                {
                    return Err("more than one --manifest".into());
                }
            }
            "--vendor" => {
                let spec = value("--vendor")?;
                let (vendor, path) = spec
                    .split_once('=')
                    .ok_or_else(|| format!("--vendor expects NAME=PATH, got {spec:?}"))?;
                if vendor.is_empty() || path.is_empty() {
                    return Err(format!("--vendor expects NAME=PATH, got {spec:?}"));
                }
                parsed
                    .vendors
                    .push((vendor.to_string(), PathBuf::from(path)));
            }
            "--reference" => parsed.reference = Some(value("--reference")?),
            "--fail-on" => {
                let what = value("--fail-on")?;
                parsed.fail_on.insert(match what.as_str() {
                    "missing-role" => FailOn::MissingRole,
                    "name-divergence" => FailOn::NameDivergence,
                    other => {
                        return Err(format!(
                            "--fail-on expects missing-role or name-divergence, got {other:?}"
                        ))
                    }
                });
            }
            "--format" => {
                let format = value("--format")?;
                match format.as_str() {
                    "markdown" => parsed.markdown = true,
                    "text" | "plain" => parsed.markdown = false,
                    other => {
                        return Err(format!("--format expects markdown or text, got {other:?}"))
                    }
                }
            }
            "--steps" => {
                let steps = value("--steps")?;
                parsed.steps = steps.parse::<usize>().map_err(|_| {
                    format!("--steps expects a non-negative integer, got {steps:?}")
                })?;
            }
            "--no-thread-cleanup" => parsed.thread_cleanup = false,
            "--name-anonymous" => parsed.name_anonymous = true,
            "--annotate-anonymous" => parsed.annotate_anonymous = true,
            "--no-local-names" => parsed.local_names = false,
            "--force" => parsed.force = true,
            "--trace" => parsed.force_trace = true,
            option if option.starts_with('-') => {
                return Err(format!("unrecognized option {option}"))
            }
            source => parsed.seeds.push(SeedSpec {
                path: PathBuf::from(source),
                vendor: None,
                label: None,
                build: None,
                date: None,
            }),
        }
    }
    if parsed.seeds.is_empty() && parsed.manifest.is_none() && parsed.vendors.is_empty() {
        return Err("missing a seed path".into());
    }
    if parsed.update && parsed.baseline.is_none() {
        return Err("--update-baseline needs --baseline".into());
    }
    if parsed.steps == 0 {
        return Err("--steps must be at least 1".into());
    }
    Ok(Some(parsed))
}

#[cfg(test)]
mod tests {
    //! The name-divergence classification cannot be produced by two current
    //! plans (one role always maps to one vocabulary name), so the report
    //! logic is exercised with synthesized fingerprints here.
    use super::*;
    use serde_json::json;

    fn seed(label: &str, vendor: Option<&str>, roles: &[(&str, &str)]) -> (ResolvedSeed, Value) {
        let roles = roles
            .iter()
            .map(|(role, name)| {
                (
                    role.to_string(),
                    json!([{
                        "entity_id": format!("fn@0..{}", name.len()),
                        "proposed_name": name,
                        "binding_kind": "function",
                    }]),
                )
            })
            .collect::<Map<String, Value>>();
        let fingerprint = json!({
            "seed": {
                "path": label, "sha256": "0".repeat(64), "bytes": 1,
                "mode": "linked", "label": label,
                "vendor": vendor, "build": null, "date": null,
            },
            "plan": {
                "candidates": roles.len(), "anonymous": 0,
                "ambiguous": [], "skipped": [],
                "roles": roles, "anonymous_entries": {},
            },
            "rename": {"applied": roles.len(), "skipped": 0,
                "output": {"sha256": "0".repeat(64), "bytes": 1}, "parses": true},
            "anonymous": {"name_anonymous": false, "annotate_anonymous": false,
                "named": 0, "annotated": 0, "skipped": 0,
                "outcomes": {"name": [], "annotate": []}},
        });
        (
            ResolvedSeed {
                spec: SeedSpec {
                    path: PathBuf::from(label),
                    vendor: vendor.map(str::to_string),
                    label: None,
                    build: None,
                    date: None,
                },
                label: label.to_string(),
                file: label.to_string(),
            },
            fingerprint,
        )
    }

    fn traced_fingerprint(program_sha: &str, traced: u64, stopped: &[(&str, &str)]) -> Value {
        let (seed, mut fingerprint) = seed(
            "seed.raw.js",
            None,
            &[("agent_module", "createAgentModule")],
        );
        let _ = seed;
        let stopped: Vec<Value> = stopped
            .iter()
            .map(|(function, reason)| json!({"function": function, "reason": reason}))
            .collect();
        fingerprint["seed"]["mode"] = json!("traced");
        fingerprint["trace"] = json!({
            "traced": traced,
            "stopped": stopped,
            "program": {"sha256": program_sha, "bytes": 100},
            "kernel_globals": ["host"],
        });
        fingerprint
    }

    #[test]
    fn a_program_byte_change_with_the_same_names_is_only_a_change() {
        let old = traced_fingerprint(&"a".repeat(64), 2, &[]);
        let new = traced_fingerprint(&"b".repeat(64), 2, &[]);
        let diff = compare_seed(&old, &new);
        assert_eq!(diff.counts(), (0, 0, 1, 0), "{diff:?}");
        assert!(diff.changes[0].contains("program changed"));
    }

    #[test]
    fn a_new_stop_a_lower_traced_count_and_a_new_refusal_are_regressions() {
        let old = traced_fingerprint(&"a".repeat(64), 2, &[]);
        let mut stopped = traced_fingerprint(&"a".repeat(64), 1, &[("d1@1/0", "budget")]);
        // The same role and name, so only the trace layer moved.
        let diff = compare_seed(&old, &stopped);
        assert!(diff.counts().0 >= 3, "{diff:?}");
        assert!(diff
            .regressions
            .iter()
            .any(|line| line.contains("function now stops")));
        assert!(diff
            .regressions
            .iter()
            .any(|line| line.contains("traced count fell")));
        assert!(diff
            .regressions
            .iter()
            .any(|line| line.contains("stopped count rose")));

        stopped["plan"]["skipped"] = json!([{
            "role": "observable_class", "kind": "refused",
            "reason": "the binding is mutable",
        }]);
        let diff = compare_seed(&old, &stopped);
        assert!(diff
            .regressions
            .iter()
            .any(|line| line.contains("new refusal: observable_class")));
    }

    #[test]
    fn same_role_with_different_names_diverges() {
        let (left, left_fingerprint) = seed(
            "left.js",
            Some("case"),
            &[("agent_module", "createAgentModule")],
        );
        let (right, right_fingerprint) = seed(
            "right.js",
            Some("module-only"),
            &[("agent_module", "makeAgentModule")],
        );
        let seeds = vec![left, right];
        let fingerprints = [
            ("left.js".to_string(), left_fingerprint),
            ("right.js".to_string(), right_fingerprint),
        ]
        .into_iter()
        .collect();
        let report = reference_report("left.js", &seeds, &fingerprints).expect("reference report");
        assert_eq!(report.divergences.len(), 1);
        assert_eq!(report.divergences[0].0, "agent_module");
        assert!(report.divergences[0].1.contains("createAgentModule"));
        let vendors = vendor_report(&fingerprints);
        assert_eq!(vendors.divergences.len(), 1);
        assert_eq!(vendors.divergences[0].0, "agent_module");
    }

    #[test]
    fn a_role_missing_from_a_vendor_is_reported_as_missing() {
        let (left, left_fingerprint) = seed(
            "left.js",
            Some("case"),
            &[
                ("agent_module", "createAgentModule"),
                ("session_bootstrap", "bootstrapSessionRuntime"),
            ],
        );
        let (right, right_fingerprint) = seed(
            "right.js",
            Some("module-only"),
            &[("agent_module", "createAgentModule")],
        );
        let seeds = vec![left, right];
        let fingerprints = [
            ("left.js".to_string(), left_fingerprint),
            ("right.js".to_string(), right_fingerprint),
        ]
        .into_iter()
        .collect();
        let report = reference_report("left.js", &seeds, &fingerprints).expect("reference report");
        assert_eq!(
            report.missing,
            vec![("session_bootstrap".to_string(), "right.js".to_string())]
        );
        let vendors = vendor_report(&fingerprints);
        assert_eq!(
            vendors.missing,
            vec![("session_bootstrap".to_string(), vec!["case".to_string()])]
        );
    }
}
