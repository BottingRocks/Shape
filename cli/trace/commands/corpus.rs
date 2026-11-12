//! `shape trace corpus`: walk every VM function of one or more seeds and
//! compare what each became with a saved baseline.
//!
//! The walk starts at each seed's entry thread and follows every
//! `vmCreateThread(descriptor, pc, state)` the traced functions create. Each
//! function is reduced to a fingerprint: how the walk ended, the bytecode
//! positions it covered, the VM functions it creates, the host properties it
//! reads and writes, its branch kinds (with every branch decided by context),
//! emitted names read out of scope, and self-check differences. A baseline
//! diff tells a regression (a function stops that completed, loses
//! positions or effects, gains a branch decided by context, reads a name out
//! of scope, or fails its self-check) from any other change.

use std::{collections::BTreeMap, fs, path::PathBuf, process::ExitCode, time::Instant};

use serde_json::{json, Value};

use crate::cli::trace::corpus::{
    compare, overstep, parallel_map, stop_kind, text, walk_seed, CorpusOptions,
};

pub(crate) const CORPUS_USAGE: &str = "usage: shape trace corpus <seed.raw.js>... [--report-dir DIR] [--baseline FILE] [--update-baseline] [--fast] [--cleanup] [--seed-by-seed] [--branches FILE] [--audit-fakes] [--steps N] [--limit N]

Walk every VM function of each seed: its entry thread, then every function a
traced function creates with vmCreateThread, with that descriptor's slots.
Each function becomes a fingerprint (how the walk ended, positions covered,
functions created, host properties read and written, branch kinds and every
branch decided by context, names read out of scope, self-check differences).

  --json             write fingerprints and comparison results as JSON to stdout
  --report-dir DIR    keep each seed's compiled kernel artifact here (written
                      once, loaded after); legacy report JSON is rejected
  --baseline FILE     compare with this baseline; regressions exit 1
  --update-baseline   write the fingerprints to --baseline instead
  --fast              skip the self-checks: by default each outlined call is
                      also traced inline, and each function is traced again
                      without outlining, and the results compared; the same
                      walk twice must emit the same code. Each function is
                      also walked stepping the other side of every fork
                      first: one whose code differs depends on the side order
                      (counted, not yet a regression)

Each seed's summary gives the overstep, VMInstructions run per kept one,
of the walks and of the walks without outlining; the ten functions with the
most overstep follow the seeds.
  --cleanup           also run the output cleanup passes (string folding,
                      property writes, arrays) and check their code too
                      (slow: about 0.1 s per function)
  --seed-by-seed      walk the seeds one at a time, each with its functions
                      on every core, to time how fast one seed can go (by
                      default every seed walks at once, each on one core,
                      which is faster for many seeds)
  --branches FILE     write every conditional jump's decision, one JSON line
                      each: seed, function, position, kind (real, fake by
                      test, fake by context), the side kept, why, the test
  --audit-fakes       also walk the side each branch decided fake by test
                      rules out (undone after): the branch list gives what
                      it reached
  --steps N           the most dispatch steps per function (default 1000000)
  --limit N           the most functions per seed (default: all)

A summary goes to stderr; the fingerprints go to --baseline with
--update-baseline.";


/// Run `shape trace corpus` from its arguments.
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
            return super::failure("corpus", json, &error, 2);
        }
    };
    match corpus(&args) {
        Ok(code) => code,
        Err(error) => super::failure("corpus", json, &error, 1),
    }
}

fn corpus(args: &CorpusOptions) -> Result<ExitCode, String> {
    let started = Instant::now();
    // Seeds share nothing: each walks on its own thread, and the results
    // are reported in the order given. Seed by seed, one walks at a time,
    // its functions on every core.
    type Walked = (Result<BTreeMap<String, Value>, String>, std::time::Duration);
    let walk = |seed: &PathBuf| {
        let started = Instant::now();
        let mut walked = walk_seed(seed, args);
        // Kept until every seed is walked: only what the report reads.
        if args.branches.is_none() {
            for print in walked
                .iter_mut()
                .flat_map(|functions| functions.values_mut())
            {
                if let Some(print) = print.as_object_mut() {
                    print.remove("branch_list");
                }
            }
        }
        (walked, started.elapsed())
    };
    let walked: Vec<Option<Walked>> = if args.jobs > 1 {
        args.seeds
            .iter()
            .flat_map(|seed| parallel_map(std::slice::from_ref(seed), 1, |seed| walk(seed)))
            .collect()
    } else {
        // One seed per core: more at once only hold more walks in memory.
        let cores = std::thread::available_parallelism().map_or(1, usize::from);
        parallel_map(&args.seeds, cores * 2, |seed| walk(seed))
    };
    let walked = walked.into_iter().map(|walked| {
        walked.unwrap_or_else(|| (Err("a seed's walk panicked".into()), Default::default()))
    });
    let mut prints = BTreeMap::new();
    for (seed, (functions, elapsed)) in args.seeds.iter().zip(walked) {
        let name = seed
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| seed.display().to_string());
        let functions = functions?;
        let complete = functions
            .values()
            .filter(|print| print["complete"] == json!(true))
            .count();
        let total = |field: &str| -> u64 {
            functions
                .values()
                .map(|print| print[field].as_u64().unwrap_or(0))
                .sum()
        };
        let (kept, run) = (total("steps"), total("handlers"));
        let (inline_kept, inline_run) = (total("inline_steps"), total("inline_handlers"));
        let dependent = functions
            .values()
            .filter(|print| {
                print["schedule"]
                    .as_array()
                    .is_some_and(|list| !list.is_empty())
            })
            .count();
        let worst = functions
            .iter()
            .filter_map(|(key, print)| Some((overstep(print)?, key)))
            .max_by(|a, b| a.0.total_cmp(&b.0));
        eprintln!(
            "{name}: {} functions, {complete} complete, {:.1}s, overstep {:.1}x{}{}",
            functions.len(),
            elapsed.as_secs_f64(),
            run as f64 / kept.max(1) as f64,
            worst.map_or(String::new(), |(ratio, key)| format!(
                " (worst {key} {ratio:.0}x)"
            )),
            if args.self_check {
                format!(
                    ", inline overstep {:.1}x, {dependent} depend on the side order",
                    inline_run as f64 / inline_kept.max(1) as f64
                )
            } else {
                String::new()
            }
        );
        let mut stops: BTreeMap<String, usize> = BTreeMap::new();
        for print in functions.values() {
            if print["complete"] != json!(true) {
                let stop = print["stop"].as_str().unwrap_or("").to_string();
                *stops.entry(stop_kind(&stop)).or_default() += 1;
            }
        }
        for (stop, count) in stops {
            eprintln!("  {count:>4} stopped: {stop}");
        }
        prints.insert(name, Value::Object(functions.into_iter().collect()));
    }
    let mut current = Value::Object(prints.into_iter().collect());
    // Every decision to the branch list, never into a baseline: written
    // only when asked for.
    let mut lines = String::new();
    let listing = args.branches.is_some();
    // The steps every walk threw away, by what threw them away: not part of
    // a baseline either.
    let mut wasted: BTreeMap<String, u64> = BTreeMap::new();
    let (mut kept, mut run) = (0, 0);
    for (seed, functions) in current.as_object_mut().into_iter().flatten() {
        for (key, print) in functions.as_object_mut().into_iter().flatten() {
            kept += print["steps"].as_u64().unwrap_or(0);
            run += print["handlers"].as_u64().unwrap_or(0);
            let mut cells_wasted = 0;
            let mut by_cause = Value::Null;
            if let Some(Value::Object(causes)) = print
                .as_object_mut()
                .and_then(|print| print.remove("wasted"))
            {
                by_cause = Value::Object(causes.clone());
                cells_wasted = causes
                    .get("walk: shared cells")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                for (cause, steps) in causes {
                    *wasted.entry(cause).or_default() += steps.as_u64().unwrap_or(0);
                }
            }
            if let Some(digest) = print
                .as_object_mut()
                .and_then(|print| print.remove("digest"))
                .filter(|_| listing)
            {
                lines.push_str(&json!({ "seed": seed, "function": key, "digest": digest, "kept": print["steps"], "run": print["handlers"], "undeclared": print["undeclared"], "cell_restarts": print["cell_restarts"], "cells_wasted": cells_wasted, "wasted": by_cause }).to_string());
                lines.push('\n');
            }
            let Some(list) = print
                .as_object_mut()
                .and_then(|print| print.remove("branch_list"))
                .filter(|_| listing)
            else {
                continue;
            };
            for mut decision in list.as_array().cloned().unwrap_or_default() {
                if let Some(decision) = decision.as_object_mut() {
                    decision.insert("seed".into(), json!(seed));
                    decision.insert("function".into(), json!(key));
                }
                lines.push_str(&decision.to_string());
                lines.push('\n');
            }
        }
    }
    if let Some(path) = &args.branches {
        fs::write(path, lines)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        eprintln!("branches: wrote {}", path.display());
    }
    if run > 0 {
        let thrown: u64 = wasted.values().sum();
        eprintln!(
            "steps: {run} run, {kept} kept, {thrown} thrown away ({:.0}% of those run)",
            100.0 * thrown as f64 / run as f64
        );
        let mut causes: Vec<_> = wasted.into_iter().collect();
        causes.sort_by(|a, b| b.1.cmp(&a.1));
        for (cause, steps) in causes {
            eprintln!(
                "  {steps:>12} {:>5.1}%  {cause}",
                100.0 * steps as f64 / run as f64
            );
        }
    }
    // The functions whose walks ran the most VMInstructions per kept one.
    let mut worst: Vec<(f64, String)> = current
        .as_object()
        .into_iter()
        .flatten()
        .flat_map(|(seed, functions)| {
            functions
                .as_object()
                .into_iter()
                .flatten()
                .filter_map(move |(key, print)| Some((overstep(print)?, format!("{seed} {key}"))))
        })
        .collect();
    worst.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (ratio, function) in worst.iter().take(10) {
        eprintln!("overstep {ratio:>6.0}x {function}");
    }
    eprintln!("time: {:.2?}", started.elapsed());

    let mut regressions = Vec::new();
    // Problems a run has without any baseline.
    for (seed, functions) in current.as_object().into_iter().flatten() {
        for (key, print) in functions.as_object().into_iter().flatten() {
            for field in ["undeclared", "emission", "self_check"] {
                for item in print[field].as_array().into_iter().flatten() {
                    regressions.push(format!("{seed} {key}: {field}: {}", text(item)));
                }
            }
        }
    }
    let mut baseline_changes = Vec::new();
    match (&args.baseline, args.update) {
        (Some(path), true) => {
            let json = serde_json::to_string_pretty(&current).expect("fingerprints serialize");
            fs::write(path, json + "\n")
                .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
            eprintln!("baseline: wrote {}", path.display());
        }
        (Some(path), false) => {
            let json = fs::read_to_string(path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            let baseline: Value = serde_json::from_str(&json)
                .map_err(|error| format!("{} is not a baseline: {error}", path.display()))?;
            let (found, changes) = compare(&baseline, &current);
            regressions.extend(found);
            for change in &changes {
                eprintln!("changed: {change}");
            }
            eprintln!(
                "baseline: {} regressions, {} other changes",
                regressions.len(),
                changes.len()
            );
            baseline_changes = changes;
        }
        (None, _) => {}
    }
    for regression in &regressions {
        eprintln!("regression: {regression}");
    }
    if args.json {
        let stopped = current
            .as_object()
            .into_iter()
            .flat_map(|seeds| seeds.values())
            .flat_map(|functions| {
                functions
                    .as_object()
                    .into_iter()
                    .flat_map(|map| map.values())
            })
            .any(|function| function["complete"] != json!(true));
        let status = if !regressions.is_empty() {
            "failure"
        } else if stopped {
            "partial"
        } else {
            "complete"
        };
        super::write_json(
            "corpus",
            status,
            json!({
                "captures": current, "regressions": regressions, "changes": baseline_changes,
                "baseline_updated": args.update,
            }),
        )?;
    }
    Ok(if regressions.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

/// `f` of each item, in order, on up to `threads` threads (each with the
/// room deeply nested code needs to recurse); `None` where `f` panicked.
fn parse(args: Vec<String>) -> Result<Option<CorpusOptions>, String> {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        return Ok(None);
    }
    let mut parsed = CorpusOptions {
        json: false,
        seeds: Vec::new(),
        report_dir: None,
        baseline: None,
        update: false,
        self_check: true,
        cleanup: false,
        jobs: 1,
        branches: None,
        audit: false,
        steps: 1_000_000,
        limit: None,
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        let mut value = |name: &str| {
            args.next()
                .ok_or_else(|| format!("missing value after {name}"))
        };
        let number = |text: String, name: &str| {
            text.parse::<usize>()
                .map_err(|_| format!("{name} expects a non-negative integer, got {text:?}"))
        };
        match arg.as_str() {
            "--report-dir" => parsed.report_dir = Some(PathBuf::from(value("--report-dir")?)),
            "--baseline" => parsed.baseline = Some(PathBuf::from(value("--baseline")?)),
            "--update-baseline" => parsed.update = true,
            "--json" => parsed.json = true,
            "--fast" => parsed.self_check = false,
            "--cleanup" => parsed.cleanup = true,
            "--branches" => parsed.branches = Some(PathBuf::from(value("--branches")?)),
            "--audit-fakes" => parsed.audit = true,
            "--seed-by-seed" => {
                parsed.jobs = std::thread::available_parallelism().map_or(1, usize::from);
            }
            "--self-check" => parsed.self_check = true,
            "--steps" => parsed.steps = number(value("--steps")?, "--steps")?,
            "--limit" => parsed.limit = Some(number(value("--limit")?, "--limit")?),
            option if option.starts_with('-') => {
                return Err(format!("unrecognized option {option}"))
            }
            seed => parsed.seeds.push(PathBuf::from(seed)),
        }
    }
    if parsed.seeds.is_empty() {
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
    use crate::cli::trace::corpus::*;
    use serde_json::json;

    #[test]
    fn emitted_threads_are_read_with_or_without_their_cells() {
        let code = r#"vmCreateThread(1, 98e3, 2); f(vmCreateThread(3, 4, 5, { "7": $CELL_7 }));"#;
        let threads: Vec<(usize, usize)> = emitted_threads(code)
            .into_iter()
            .map(|function| (function.descriptor.unwrap_or(usize::MAX), function.pc))
            .collect();
        assert_eq!(threads, [(1, 98_000), (3, 4)]);
    }

    #[test]
    fn functions_posted_to_the_worker_are_created_too() {
        // (state, pc, argument, descriptor, callback), with an argument that
        // holds commas of its own; the helper's own definition names none.
        let code = r#"function vmPostToWorker(first, second, third, operand, callback) {}
            let x = vmPostToWorker(4, 108770, f(a, [b, "c,)"]), 46, $THREAD_452);"#;
        let threads: Vec<(usize, usize, usize)> = emitted_threads(code)
            .into_iter()
            .map(|function| {
                (
                    function.descriptor.unwrap_or(usize::MAX),
                    function.pc,
                    function.state,
                )
            })
            .collect();
        assert_eq!(threads, [(46, 108_770, 4)]);
    }

    #[test]
    fn a_function_created_past_the_image_stops_alone() {
        // An operand a patch rewrote on one path can name an entry past the
        // image: that function cannot start, which is its stop, not the seed's.
        let source = include_str!("../../../tests/fixtures/javascript-vm-prefix.raw.js");
        let report = shape::deobfuscate_with_report(source).expect("fixture pipeline");
        let prepared =
            shape::prepare_prefix_from_report(source, &report, shape::PrefixLimits::default())
                .expect("fixture preparation");
        let function = |pc| Function {
            descriptor: Some(0),
            pc,
            state: 0,
        };
        assert_eq!(startable(&prepared, function(0)), Ok(()));
        let error = startable(&prepared, function(2_242_792)).unwrap_err();
        assert!(
            error.starts_with("d0@2242792/0: thread startup rejected: thread entry rejected:"),
            "{error}"
        );
    }

    #[test]
    fn a_function_that_stops_or_loses_positions_is_a_regression() {
        let baseline = json!({ "s": { "f": {
            "complete": true, "stop": "done", "positions": ["1/0", "2/0"],
            "creates": [], "host": ["read a"], "branches": {"real": 1, "fake_by_test": 0},
            "fake_by_context": []
        }}});
        let mut current = baseline.clone();
        current["s"]["f"]["positions"] = json!(["1/0"]);
        current["s"]["f"]["fake_by_context"] = json!(["1/0 kept jump: x"]);
        let (regressions, _) = compare(&baseline, &current);
        assert_eq!(regressions.len(), 2, "{regressions:?}");
        current["s"]["f"]["complete"] = json!(false);
        current["s"]["f"]["stop"] = json!("budget");
        let (regressions, _) = compare(&baseline, &current);
        assert!(
            regressions.iter().any(|r| r.contains("now stops")),
            "{regressions:?}"
        );
        let (regressions, changes) = compare(&baseline, &baseline);
        assert!(regressions.is_empty() && changes.is_empty());
    }

    #[test]
    fn a_function_announced_early_stands_only_if_a_finished_walk_creates_it() {
        let function = |pc| Function {
            descriptor: Some(pc),
            pc,
            state: 0,
        };
        let (entry, kept, dropped, failed) = (function(0), function(1), function(2), function(3));
        for jobs in [1, 4] {
            let walked = walk_reachable(&[entry], jobs, None, |at, announce| {
                if at == entry {
                    // An attempt created all three; the finished walk only one.
                    announce(&[kept, dropped, failed]);
                    return Ok(((), vec![kept]));
                }
                if at == failed {
                    return Err("a walk nothing creates".into());
                }
                Ok(((), Vec::new()))
            })
            .expect("only the unreached walk failed");
            assert_eq!(
                walked.keys().copied().collect::<Vec<_>>(),
                vec![entry, kept]
            );
        }
        // A failure of a function that stands fails the walk.
        let walked = walk_reachable(&[entry], 2, None, |at, _| {
            if at == entry {
                return Ok(((), vec![failed]));
            }
            Err("a created walk failed".into())
        });
        assert!(walked.is_err());
    }
}
