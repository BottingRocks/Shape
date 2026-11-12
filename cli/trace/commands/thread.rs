//! `shape trace thread`: walk one VM thread of a JavaScript VM capture and
//! write the function it becomes.
//!
//! The walk is the prefix driver's: it forks at program-value branches, opens
//! loops and `try`s, and completes when every path returns, throws or
//! continues a loop. A completed walk is emitted with bindings simplified.

use std::{fmt::Write as _, fs, io, path::PathBuf, process::ExitCode, time::Instant};

use shape::{
    util::fnv1a64_hex, KernelArtifact, PrefixBranchKind, PrefixStepEffect, PrefixStopReason,
    PrefixTraceReport,
};

pub(crate) const THREAD_USAGE: &str = "usage: shape trace thread <seed.raw.js> --pc N --state N (--descriptor N | --initialslots SLOTS) [--report PATH] [--steps N] [--out PATH] [--log PATH] [--self-check] [--no-outline] [--reverse-sides] [--partial] [--fake-branches FILE] [--not-fake-branches FILE]
   or: shape trace thread --report ARTIFACT --pc N --state N (--descriptor N | --initialslots SLOTS) [--steps N] [--out PATH] [--log PATH] [--self-check] [--no-outline] [--reverse-sides] [--partial] [--fake-branches FILE] [--not-fake-branches FILE]

Walk the VM thread that starts at --pc / --state of a JavaScript VM capture and,
when every path returns, throws or continues a loop, write the JavaScript
function it becomes (bindings simplified) to --out, or to stdout.
Concrete, source-proven vmDecodeString calls are folded in the emitted artifact;
the stderr summary reports folded and declined calls.

  --pc N, --state N   the thread's entry (from `shape kernel show`, or the
                      byteOffset / dispatchState of a vmCreateThread call)
  --descriptor N      take the thread's slots from vmThreadDescriptors[N]:
                      its arguments, captured slots, `arguments` object,
                      own function and receiver are unknown inputs and
                      become the emitted function's parameters
  --initialslots S    memory slots initialized to undefined: a comma list of
                      slots and inclusive ranges, e.g. 2,0,3,1 or 0-430;
                      without --descriptor the thread takes no inputs
  --report PATH       raw mode: load the kernel artifact when it exists,
                      otherwise compile and write it. Report-only mode: omit
                      the seed and load this existing source-free artifact;
                      `-` reads bounded artifact JSON from stdin.
                      A raw seed with `--report -` is rejected. In report-only
                      mode, stdin is bounded by the 16 MiB trace transport limit.
                      Legacy shape-pipeline-report/1 JSON is rejected; rerun
                      `shape deob SOURCE --report ARTIFACT` from raw source.
  --steps N           the most dispatch steps to take, counting re-traced
                      work (default 1000000)
  --inputs FILE      use an explicit input snapshot (arguments, captures, stack, receiver)
  --json             write one structured JSON result to stdout; --out still saves JavaScript
  --out PATH          write the JavaScript here instead of stdout
  --log PATH          write one line per dispatch step, with each branch's
                      kind and test
  --self-check        also trace every outlined subroutine call inline and
                      compare (costly); differences are listed and exit 5
  --no-outline        trace VM subroutines inline at every call
  --reverse-sides     step the other side of every fork first (a correct
                      walk emits the same code either way)
  --partial           when the walk stops before every path ends, still
                      write what was traced, headed by an `// INCOMPLETE:`
                      comment naming the stop; for inspecting a stop only.
                      `trace program` and the corpus never emit partial code
  --fake-branches FILE
                      a JSON object of branches to decide fake, by the jump
                      instruction's position, each with the side it always
                      takes: {\"pc/state\": true} always jumps, false always
                      falls through
  --not-fake-branches FILE
                      a JSON array of branches that are real, by the jump
                      instruction's position: [\"pc/state\"]. Neither their
                      test nor what a side reaches decides them; a side that
                      reaches a state real code never reaches stops the walk
                      there

A summary and the time of each phase go to stderr.";

struct ThreadArgs {
    json: bool,
    inputs: Option<PathBuf>,
    seed: Option<PathBuf>,
    pc: usize,
    state: usize,
    descriptor: Option<usize>,
    report: Option<PathBuf>,
    initial_slots: Option<Vec<u32>>,
    steps: usize,
    out: Option<PathBuf>,
    log: Option<PathBuf>,
    self_check: bool,
    outline: bool,
    reverse_sides: bool,
    partial: bool,
    overrides: shape::BranchOverrides,
}

/// Run `shape trace thread` from its arguments.
pub(crate) fn run_thread(args: Vec<String>) -> ExitCode {
    let json = args.iter().any(|arg| arg == "--json");
    let args = match parse(args) {
        Ok(Some(args)) => args,
        Ok(None) => {
            println!("{THREAD_USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            if !json {
                eprintln!("{THREAD_USAGE}");
            }
            return super::failure("thread", json, &error, 2);
        }
    };
    match walk(&args) {
        Ok(code) => code,
        Err(error) => super::failure("thread", json, &error, 1),
    }
}

fn walk(args: &ThreadArgs) -> Result<ExitCode, String> {
    let total = Instant::now();
    let started = Instant::now();
    let (artifact, loaded) = match &args.seed {
        Some(seed) => {
            let source = fs::read_to_string(seed)
                .map_err(|error| format!("cannot read {}: {error}", seed.display()))?;
            if args
                .report
                .as_deref()
                .is_some_and(|path| path == PathBuf::from("-"))
            {
                return Err(
                    "raw seed mode cannot use --report -; stdin is report-only input".into(),
                );
            }
            let (artifact, loaded) = match &args.report {
                Some(path) if path.exists() => {
                    let artifact = crate::cli::trace::read_kernel_artifact(path)?;
                    if artifact.source_hash() != fnv1a64_hex(source.as_bytes()) {
                        return Err(format!(
                            "{} is the artifact of another seed",
                            path.display()
                        ));
                    }
                    (artifact, true)
                }
                _ => (shape::trace::thread::compile_kernel_artifact(&source)?, false),
            };
            if let (Some(path), false) = (&args.report, loaded) {
                write_artifact_if_absent(path, &artifact)
                    .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
            }
            (artifact, loaded)
        }
        None => {
            let path = args
                .report
                .as_ref()
                .ok_or("report-only mode requires --report REPORT")?;
            if path != &PathBuf::from("-") && !path.exists() {
                return Err(format!(
                    "cannot read {}: report-only input must already exist",
                    path.display()
                ));
            }
            (crate::cli::trace::read_kernel_artifact(path)?, true)
        }
    };
    let artifact_phase = started.elapsed();

    let walked = shape::trace::thread::walk_thread(
        &artifact,
        &shape::trace::thread::ThreadOptions {
            pc: args.pc,
            state: args.state,
            descriptor: args.descriptor,
            initial_slots: args.initial_slots.clone(),
            inputs: args
                .inputs
                .as_deref()
                .map(crate::cli::trace::inputs::Snapshot::read)
                .transpose()?,
            steps: args.steps,
            self_check: args.self_check,
            outline: args.outline,
            reverse_sides: args.reverse_sides,
            partial: args.partial,
            overrides: args.overrides.clone(),
        },
    )?;
    let (prepare, trace, emit) = (walked.prepare, walked.trace, walked.emit);
    let string_folds = walked.string_folds;
    let property_writes = walked.property_writes;
    let array_constructions = walked.array_constructions;
    let undeclared = walked.undeclared;
    let completed = walked.completed;
    let result = walked.report;
    let mut written = None;
    let mut javascript = serde_json::json!({"status": "not_emitted", "code": null});
    if let Some(code) = walked.code {
        javascript = super::javascript(&code, args.out.as_deref(), !completed);
        match &args.out {
            Some(path) => {
                fs::write(path, &code)
                    .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
                written = Some((path.clone(), code.len()));
            }
            None if !args.json => {
                crate::cli::write_stdout_line(&code)
                    .map_err(|error| format!("failed to write stdout: {error}"))?;
            }
            None => {}
        }
    }
    if let Some(path) = &args.log {
        fs::write(path, step_log(&result))
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    }

    let (mut real, mut by_test, mut by_context) = (0, 0, Vec::new());
    for record in &result.records {
        if let PrefixStepEffect::Handler {
            branch: Some(branch),
            ..
        } = &record.effect
        {
            match &branch.kind {
                PrefixBranchKind::Real => real += 1,
                PrefixBranchKind::FakeByTest { .. } => by_test += 1,
                PrefixBranchKind::FakeByContext { taken, reason } => by_context.push(format!(
                    "  pc {} state {}: followed only the {} side ({reason})",
                    record.dispatch_pc,
                    record.state_before,
                    if *taken { "jump" } else { "fall-through" },
                )),
            }
        }
    }
    eprintln!("stop: {}", result.stop);
    if result.stop != PrefixStopReason::Completed {
        if let Some(diagnostic) = result.trace.diagnostics.last() {
            eprintln!("detail: {}: {}", diagnostic.code, diagnostic.message);
        }
    }
    let retraces = &result.retraces;
    eprintln!(
        "retraces: {} (new loops {}, loop variables {}, loop values {}, try variables {}, meeting points {}, splits {}, inline calls {}, wrong breaks {}); {} value-set dispatches; {} unmergeable meetings",
        retraces.total(),
        retraces.new_loops,
        retraces.loop_variables,
        retraces.loop_values,
        retraces.try_variables,
        retraces.meeting_points,
        retraces.splits,
        retraces.inline_calls,
        retraces.wrong_breaks,
        retraces.dispatches,
        retraces.unmergeable
    );
    if !undeclared.is_empty() {
        eprintln!(
            "scope: the function reads {} generated names where none is declared: {}",
            undeclared.len(),
            undeclared.join(", ")
        );
    }
    eprintln!(
        "steps: {} kept, {} handlers run",
        result.records.len(),
        result.trace.handlers_traced
    );
    eprintln!(
        "branches: {real} real ifs, {by_test} fake by test, {} fake by context",
        by_context.len()
    );
    // Every side the walk dropped without a decided test, so each can be
    // checked by hand.
    for line in &by_context {
        eprintln!("{line}");
    }
    if let Some((path, bytes)) = written {
        eprintln!("javascript: {} ({bytes} bytes)", path.display());
    }
    if let Some(report) = &string_folds {
        eprintln!(
            "vm strings: {} folded, {} declined",
            report.folded, report.declined
        );
        for (reason, count) in &report.reasons {
            eprintln!("  {}: {count}", reason.as_str());
        }
        for diagnostic in report.diagnostics.iter().take(3) {
            eprintln!("  detail: {diagnostic}");
        }
    }
    if let Some((rewritten, retained)) = property_writes {
        eprintln!("property writes: {rewritten} simplified, {retained} retained");
    }
    if let Some((arrays, writes)) = array_constructions {
        eprintln!("array constructions: {arrays} compacted, {writes} writes removed");
    }
    eprintln!(
        "time: {} {artifact_phase:.2?}, prepare {prepare:.2?}, trace {trace:.2?}{}, total {:.2?}",
        if loaded {
            "load artifact"
        } else {
            "compile artifact"
        },
        emit.map(|emit| format!(", emit {emit:.2?}"))
            .unwrap_or_default(),
        total.elapsed()
    );
    for difference in &result.self_check {
        eprintln!("self-check: {difference}");
    }
    if args.json {
        let diagnostics = super::diagnostics(&result.trace);
        let decisions: Vec<_> = result
            .records
            .iter()
            .filter_map(|record| {
                let PrefixStepEffect::Handler {
                    branch: Some(branch),
                    ..
                } = &record.effect
                else {
                    return None;
                };
                let (kind, taken, reason) = match &branch.kind {
                    PrefixBranchKind::Real => ("real", None, None),
                    PrefixBranchKind::FakeByTest { taken } => ("fake_by_test", Some(*taken), None),
                    PrefixBranchKind::FakeByContext { taken, reason } => {
                        ("fake_by_context", Some(*taken), Some(reason))
                    }
                };
                Some(
                    serde_json::json!({"pc": record.dispatch_pc, "state": record.state_before,
                "kind": kind, "taken": taken, "reason": reason}),
                )
            })
            .collect();
        let status = if !completed {
            "partial"
        } else if !result.self_check.is_empty() || !undeclared.is_empty() {
            "failure"
        } else {
            "complete"
        };
        super::write_json(
            "thread",
            status,
            serde_json::json!({
                "entry": {"pc": args.pc, "state": args.state},
                "source_hash": artifact.source_hash(),
                "stop": result.stop.to_string(), "proven_complete": completed,
                "steps": result.records.len(), "handlers_traced": result.trace.handlers_traced,
                "diagnostics": diagnostics, "self_check": result.self_check, "undeclared": undeclared,
                "branches": {"real": real, "fake_by_test": by_test, "fake_by_context": by_context.len(), "decisions": decisions},
                "javascript": javascript,
            }),
        )?;
    }
    Ok(if result.stop != PrefixStopReason::Completed {
        ExitCode::from(3)
    } else if !result.self_check.is_empty() {
        ExitCode::from(5)
    } else if !undeclared.is_empty() {
        // The function would throw a ReferenceError: not a usable result.
        ExitCode::from(4)
    } else {
        ExitCode::SUCCESS
    })
}


/// One line per committed dispatch step, with each branch's kind and test.
fn step_log(result: &PrefixTraceReport) -> String {
    let describer = result.trace.arena.describer();
    let mut log = String::new();
    for record in &result.records {
        let _ = write!(
            log,
            "step={} pc={} state={}->{} selector={} next_pc={}",
            record.step,
            record.dispatch_pc,
            record.state_before,
            record.state_after,
            record.selector,
            record.actual_next_pc
        );
        if let PrefixStepEffect::Handler {
            branch: Some(branch),
            ..
        } = &record.effect
        {
            let kind = match &branch.kind {
                PrefixBranchKind::Real => "real_if".to_string(),
                PrefixBranchKind::FakeByTest { taken } => format!("fake_by_test(taken={taken})"),
                PrefixBranchKind::FakeByContext { taken, reason } => {
                    format!("fake_by_context(taken={taken}, {reason})")
                }
            };
            let _ = write!(
                log,
                " branch={kind} target={}/{} test=`{}`",
                branch.target.pc,
                branch.target.state,
                describer.describe(branch.test)
            );
        }
        log.push('\n');
    }
    log
}

fn parse(args: Vec<String>) -> Result<Option<ThreadArgs>, String> {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        return Ok(None);
    }
    let mut json = false;
    let mut inputs = None;
    let mut seed = None;
    let (mut pc, mut state, mut descriptor, mut initial_slots) = (None, None, None, None);
    let (mut steps, mut out, mut log, mut report) = (None, None, None, None);
    let (mut self_check, mut outline, mut reverse_sides) = (false, true, false);
    let mut partial = false;
    let (mut fake_branches, mut not_fake_branches) = (None, None);
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let mut value = |name: &str| {
            args.next()
                .ok_or_else(|| format!("missing value after {name}"))
        };
        match arg.as_str() {
            "--pc" => pc = Some(number(&value("--pc")?, "--pc")?),
            "--state" => state = Some(number(&value("--state")?, "--state")?),
            "--report" => {
                if report.is_some() {
                    return Err("--report may be provided only once".into());
                }
                report = Some(PathBuf::from(value("--report")?));
            }
            "--descriptor" => descriptor = Some(number(&value("--descriptor")?, "--descriptor")?),
            "--initialslots" => initial_slots = Some(slots(&value("--initialslots")?)?),
            "--steps" => steps = Some(number(&value("--steps")?, "--steps")?),
            "--inputs" => inputs = Some(PathBuf::from(value("--inputs")?)),
            "--json" => json = true,
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--log" => log = Some(PathBuf::from(value("--log")?)),
            "--self-check" => self_check = true,
            "--no-outline" => outline = false,
            "--reverse-sides" => reverse_sides = true,
            "--partial" => partial = true,
            "--fake-branches" => fake_branches = Some(PathBuf::from(value("--fake-branches")?)),
            "--not-fake-branches" => {
                not_fake_branches = Some(PathBuf::from(value("--not-fake-branches")?));
            }
            option if option.starts_with('-') => {
                return Err(format!("unrecognized option {option}"))
            }
            path if seed.is_none() => seed = Some(PathBuf::from(path)),
            extra => return Err(format!("unexpected argument {extra}")),
        }
    }
    let steps = steps.unwrap_or(1_000_000);
    if steps == 0 {
        return Err("--steps must be at least 1".into());
    }
    if descriptor.is_none() && initial_slots.is_none() {
        return Err("missing --descriptor or --initialslots".into());
    }
    if seed.is_none() && report.is_none() {
        return Err("report-only mode requires --report REPORT".into());
    }
    if seed.is_some()
        && report
            .as_deref()
            .is_some_and(|path| path == PathBuf::from("-"))
    {
        return Err("raw seed mode cannot use --report -; stdin is report-only input".into());
    }
    Ok(Some(ThreadArgs {
        json,
        inputs,
        seed,
        pc: pc.ok_or("missing --pc")?,
        state: state.ok_or("missing --state")?,
        descriptor,
        report,
        initial_slots,
        steps,
        out,
        log,
        self_check,
        outline,
        reverse_sides,
        partial,
        overrides: super::branch_overrides(fake_branches.as_deref(), not_fake_branches.as_deref())?,
    }))
}

fn write_artifact_if_absent(path: &PathBuf, artifact: &KernelArtifact) -> io::Result<()> {
    let json = artifact
        .to_json()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    let temporary = path.with_extension(format!(
        "{}.tmp-{}",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("artifact"),
        std::process::id()
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    if let Err(error) = std::io::Write::write_all(&mut file, json.as_bytes()) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    drop(file);
    let result = fs::hard_link(&temporary, path);
    let _ = fs::remove_file(&temporary);
    result
}

fn number(text: &str, name: &str) -> Result<usize, String> {
    text.parse()
        .map_err(|_| format!("{name} expects a non-negative integer, got {text:?}"))
}

/// `2,0,3,1` or `0-430`, or a mix.
fn slots(text: &str) -> Result<Vec<u32>, String> {
    let mut slots = Vec::new();
    for part in text.split(',').filter(|part| !part.is_empty()) {
        let bad = || format!("--initialslots expects slots or ranges like 0-430, got {part:?}");
        match part.split_once('-') {
            Some((first, last)) => {
                let first: u32 = first.parse().map_err(|_| bad())?;
                let last: u32 = last.parse().map_err(|_| bad())?;
                if last < first || last - first > 1_000_000 {
                    return Err(bad());
                }
                slots.extend(first..=last);
            }
            None => slots.push(part.parse().map_err(|_| bad())?),
        }
    }
    Ok(slots)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_only_arguments_have_no_positional_source() {
        let parsed = parse(vec![
            "--report".into(),
            "capture.report.json".into(),
            "--pc".into(),
            "115869".into(),
            "--state".into(),
            "7".into(),
            "--initialslots".into(),
            "0-430".into(),
        ])
        .unwrap()
        .unwrap();
        assert!(parsed.seed.is_none());
        assert_eq!(parsed.report, Some(PathBuf::from("capture.report.json")));
    }

    #[test]
    fn raw_seed_cannot_send_report_to_stdin() {
        let error = parse(vec![
            "seed.raw.js".into(),
            "--report".into(),
            "-".into(),
            "--pc".into(),
            "1".into(),
            "--state".into(),
            "0".into(),
            "--initialslots".into(),
            "0".into(),
        ])
        .err()
        .expect("raw stdin report must be rejected");
        assert!(error.contains("report-only input"));
    }

    #[test]
    fn duplicate_report_options_are_rejected() {
        let error = parse(vec![
            "--report".into(),
            "first.json".into(),
            "--report".into(),
            "second.json".into(),
        ])
        .err()
        .expect("duplicate report option must be rejected");
        assert!(error.contains("--report may be provided only once"));
    }

    #[test]
    fn slots_take_lists_and_inclusive_ranges() {
        assert_eq!(slots("2,0,3,1").unwrap(), [2, 0, 3, 1]);
        assert_eq!(slots("0-3").unwrap(), [0, 1, 2, 3]);
        assert_eq!(slots("5,0-1").unwrap(), [5, 0, 1]);
        assert!(slots("3-1").is_err());
        assert!(slots("a").is_err());
    }

    #[test]
    fn the_seed_and_entry_are_required() {
        let args = |list: &[&str]| list.iter().map(|arg| arg.to_string()).collect();
        assert!(parse(args(&["--pc", "0", "--state", "0", "--initialslots", "0"])).is_err());
        assert!(parse(args(&["seed.js", "--pc", "0", "--state", "0"])).is_err());
        let parsed = parse(args(&[
            "seed.js",
            "--pc",
            "0",
            "--state",
            "0",
            "--descriptor",
            "31",
        ]))
        .unwrap()
        .unwrap();
        assert_eq!((parsed.descriptor, parsed.initial_slots), (Some(31), None));
        let parsed = parse(args(&[
            "seed.js",
            "--pc",
            "1",
            "--state",
            "2",
            "--initialslots",
            "0-1",
        ]))
        .unwrap()
        .unwrap();
        assert_eq!((parsed.pc, parsed.state, parsed.steps), (1, 2, 1_000_000));
        assert_eq!(parsed.initial_slots.as_deref(), Some(&[0, 1][..]));
        assert_eq!(parsed.descriptor, None);
        assert!(parse(args(&["--help"])).unwrap().is_none());
    }
}
