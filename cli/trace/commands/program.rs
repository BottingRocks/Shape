//! `shape trace program`: flags, output and summary. Tracing and linking
//! live in [`shape::trace::program`].

use std::{fs, path::PathBuf, process::ExitCode, time::Instant};

use shape::trace::program::{link_program, LinkOptions};

pub(crate) const PROGRAM_USAGE: &str =
    "usage: shape trace program <seed.raw.js> [--out FILE] [--report-dir DIR] [--steps N] [--dump-stages DIR] [--thread-cleanup] [--no-cleanup] [--check-reuse] [--fake-branches FILE] [--not-fake-branches FILE]

Trace every VM function of one JavaScript VM capture (its entry thread, then
every function a traced function creates with vmCreateThread) and build them
into one program: each function as `function $THREAD_<state>_<pc>` nested
where the function that creates it does, sharing its cells through scope, and
a call of the entry thread. A function that does not complete becomes one
that throws. The kernel globals the program still reads (string tables,
`host`, …) are listed on stderr.

  --json             write one structured JSON result to stdout; --out still saves JavaScript
  --out FILE      write the program here (default: stdout)
  --report-dir DIR
                  keep the compiled kernel artifact here (written once,
                  loaded after)
  --steps N       the most dispatch steps per function (default 1000000)
  --dump-stages DIR
                  save intermediate JavaScript: DIR/00-linked.js and the
                  named whole-program stages (12-…, 13c…-13e-…); for each
                  function, DIR/functions/<function>/ holds its emission
                  (00-…04-), the cleaned thread (06-thread-pipeline.js, after
                  05-thread-cleaned.js with --thread-cleanup) and, under
                  pipeline/, each per-function cleanup stage (01-…13b-)
  --thread-cleanup
                  clean each emitted thread on its tracing worker before
                  linking; normalize final JavaScript parentheses/formatting
  --no-cleanup    write the linked program without the
                  whole-program Oxc cleanup passes (much faster: for runs
                  that check the tracing)
  --check-reuse   when the program is walked again with cell contents, also
                  walk every function afresh and fail if any function's code
                  differs from the walk kept from before (slow: for runs
                  that check the tracing)
  --fake-branches FILE
                  a JSON object of branches to decide fake, by the jump
                  instruction's position, each with the side it always takes:
                  {\"pc/state\": true} always jumps, false always falls through
  --not-fake-branches FILE
                  a JSON array of branches that are real, by the jump
                  instruction's position: [\"pc/state\"]. Neither their test
                  nor what a side reaches decides them; a side that reaches a
                  state real code never reaches stops that function's walk";

struct ProgramArgs {
    json: bool,
    seed: PathBuf,
    out: Option<PathBuf>,
    report_dir: Option<PathBuf>,
    steps: usize,
    dump_stages: Option<PathBuf>,
    no_cleanup: bool,
    check_reuse: bool,
    overrides: shape::BranchOverrides,
    thread_cleanup: bool,
}

/// Run `shape trace program` from its arguments.
pub(crate) fn run_program(args: Vec<String>) -> ExitCode {
    let json = args.iter().any(|arg| arg == "--json");
    let args = match parse(args) {
        Ok(Some(args)) => args,
        Ok(None) => {
            println!("{PROGRAM_USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            if !json {
                eprintln!("{PROGRAM_USAGE}");
            }
            return super::failure("program", json, &error, 2);
        }
    };
    match program(&args) {
        Ok(code) => code,
        Err(error) => super::failure("program", json, &error, 1),
    }
}

/// Run `shape trace program`: link, then write and summarize the program.
fn program(args: &ProgramArgs) -> Result<ExitCode, String> {
    let started = Instant::now();
    let linked = link_program(&LinkOptions {
        seed: args.seed.clone(),
        report_dir: args.report_dir.clone(),
        steps: args.steps,
        dump_stages: args.dump_stages.clone(),
        thread_cleanup: args.thread_cleanup,
        no_cleanup: args.no_cleanup,
        check_reuse: args.check_reuse,
        overrides: args.overrides.clone(),
        jobs: None,
        quiet: false,
    })?;
    let code = &linked.code;
    match &args.out {
        Some(path) => fs::write(path, code)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?,
        None if !args.json => {
            crate::cli::write_stdout(code.as_bytes())
                .map_err(|error| format!("failed to write stdout: {error}"))?;
        }
        None => {}
    }
    eprintln!(
        "functions: {} traced, {} stopped",
        linked.traced,
        linked.stopped.len()
    );
    if args.thread_cleanup {
        let stats = linked.stats.thread;
        eprintln!(
            "per-thread vm_sub cleanup: {} calls inlined, {} declarations removed, {} remaining",
            stats.calls, stats.removed, stats.remaining
        );
        eprintln!("per-thread expression cleanup: {} property writes simplified, {} arrays folded, {} method calls restored", stats.properties, stats.arrays, stats.methods);
        eprintln!(
            "per-thread loop cleanup: {} loops reconstructed, {} blocks flattened",
            stats.loops, stats.blocks
        );
    }
    for (key, reason) in &linked.stopped {
        eprintln!("  stopped {key}: {reason}");
    }
    eprintln!(
        "kernel globals read: {}",
        linked.kernel_globals.join(", ")
    );
    let destination = args
        .out
        .as_ref()
        .map_or_else(|| "stdout".to_string(), |path| path.display().to_string());
    eprintln!("program: {destination} ({} bytes)", code.len());
    eprintln!("time: {:.2?}", started.elapsed());
    shape::profile::report();
    if args.json {
        let stopped_functions: Vec<_> = linked
            .stopped
            .iter()
            .map(|(function, reason)| serde_json::json!({"function": function, "reason": reason}))
            .collect();
        super::write_json(
            "program",
            if linked.stopped.is_empty() {
                "complete"
            } else {
                "partial"
            },
            serde_json::json!({
                "functions": {"traced": linked.traced, "stopped": stopped_functions},
                "kernel_globals": linked.kernel_globals,
                "javascript": super::javascript(code, args.out.as_deref(), !linked.stopped.is_empty()),
            }),
        )?;
    }
    Ok(if linked.stopped.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(3)
    })
}

fn parse(args: Vec<String>) -> Result<Option<ProgramArgs>, String> {
    let mut json = false;
    let mut seed = None;
    let mut out = None;
    let mut report_dir = None;
    let mut steps = 1_000_000;
    let mut dump_stages = None;
    let mut no_cleanup = false;
    let mut check_reuse = false;
    let (mut fake_branches, mut not_fake_branches) = (None, None);
    let mut thread_cleanup = false;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let mut value = |flag: &str| args.next().ok_or_else(|| format!("{flag} needs a value"));
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--json" => json = true,
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--report-dir" => report_dir = Some(PathBuf::from(value("--report-dir")?)),
            "--dump-stages" => dump_stages = Some(PathBuf::from(value("--dump-stages")?)),
            "--no-cleanup" => no_cleanup = true,
            "--check-reuse" => check_reuse = true,
            "--fake-branches" => fake_branches = Some(PathBuf::from(value("--fake-branches")?)),
            "--not-fake-branches" => {
                not_fake_branches = Some(PathBuf::from(value("--not-fake-branches")?));
            }
            "--thread-cleanup" => thread_cleanup = true,
            "--steps" => {
                steps = value("--steps")?
                    .parse()
                    .map_err(|_| "--steps needs a number".to_string())?;
            }
            flag if flag.starts_with('-') => return Err(format!("unrecognized option {flag}")),
            path => {
                if seed.replace(PathBuf::from(path)).is_some() {
                    return Err("only one seed".into());
                }
            }
        }
    }
    Ok(Some(ProgramArgs {
        json,
        seed: seed.ok_or("missing seed")?,
        out,
        report_dir,
        steps,
        dump_stages,
        no_cleanup,
        check_reuse,
        overrides: super::branch_overrides(fake_branches.as_deref(), not_fake_branches.as_deref())?,
        thread_cleanup,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(extra: &[&str]) -> Vec<String> {
        ["seed.raw.js", "--out", "program.js"]
            .into_iter()
            .chain(extra.iter().copied())
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn out_is_optional_and_defaults_to_stdout() {
        let parsed = parse(vec!["seed.raw.js".to_string()]).unwrap().unwrap();
        assert!(parsed.out.is_none());
        assert_eq!(parsed.seed, PathBuf::from("seed.raw.js"));
    }

    #[test]
    fn thread_cleanup_combines_with_trace_reuse_and_raw_linked_output() {
        let parsed = parse(args(&["--thread-cleanup", "--check-reuse", "--no-cleanup"]))
            .unwrap()
            .unwrap();
        assert!(parsed.thread_cleanup);
        assert!(parsed.check_reuse);
        assert!(parsed.no_cleanup);
    }

    #[test]
    fn program_stage_dump_is_optional() {
        assert!(parse(args(&[])).unwrap().unwrap().dump_stages.is_none());
        assert_eq!(
            parse(args(&["--dump-stages", "stages"]))
                .unwrap()
                .unwrap()
                .dump_stages,
            Some("stages".into())
        );
        assert!(parse(args(&["--dump-stages"])).is_err());
    }

    #[test]
    fn program_cleanup_runs_unless_turned_off() {
        assert!(!parse(args(&[])).unwrap().unwrap().no_cleanup);
        assert!(parse(args(&["--no-cleanup"])).unwrap().unwrap().no_cleanup);
    }

    #[test]
    fn thread_cleanup_is_independent_of_linked_cleanup() {
        assert!(!parse(args(&[])).unwrap().unwrap().thread_cleanup);
        let parsed = parse(args(&["--thread-cleanup", "--no-cleanup"]))
            .unwrap()
            .unwrap();
        assert!(parsed.thread_cleanup && parsed.no_cleanup);
    }
}
