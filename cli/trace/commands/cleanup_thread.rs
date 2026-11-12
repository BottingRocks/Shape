//! Standalone cleanup of one emitted thread, without tracing or linking.

use std::{fs, path::PathBuf, process::ExitCode, time::Instant};

const USAGE: &str = "usage: shape trace cleanup-thread <thread.js> [--out FILE] [--json]\n\n\
Clean one emitted JavaScript function. This reads emitted thread source, not a\n\
raw capture. Writes cleaned JavaScript to stdout or --out; counts go to stderr.\n\
No tracing, linking, or whole-program cleanup runs.";

pub(super) fn run(args: Vec<String>) -> ExitCode {
    let json = args.iter().any(|arg| arg == "--json");
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return super::print_help("shape trace cleanup-thread", USAGE);
    }
    let (input, output) = match parse(args) {
        Ok(paths) => paths,
        Err(error) => {
            if !json {
                eprintln!("{USAGE}");
            }
            return super::failure("cleanup-thread", json, &error, 2);
        }
    };
    let started = Instant::now();
    let work = || -> Result<(), String> {
        let source = fs::read_to_string(&input).map_err(|e| format!("{}: {e}", input.display()))?;
        let cleaned = shape::emitted_cleanup::cleanup_thread(&source)?;
        match &output {
            Some(path) => {
                fs::write(path, &cleaned.code).map_err(|e| format!("{}: {e}", path.display()))?
            }
            None if !json => {
                crate::cli::write_stdout(cleaned.code.as_bytes()).map_err(|e| e.to_string())?;
            }
            None => {}
        }
        if json {
            super::write_json(
                "cleanup-thread",
                "complete",
                serde_json::json!({
                    "javascript": super::javascript(&cleaned.code, output.as_deref(), false),
                    "cleanup": {"inlined_bindings": cleaned.inlined_bindings, "inlined_helpers": cleaned.inlined_helper_calls,
                        "removed_helpers": cleaned.removed_helpers, "remaining_helpers": cleaned.remaining_helpers,
                        "property_writes": cleaned.rewritten_property_writes, "arrays": cleaned.folded_arrays,
                        "methods": cleaned.simplified_method_calls, "loops": cleaned.reconstructed_loops,
                        "flattened_blocks": cleaned.flattened_blocks}
                }),
            )?;
        }
        eprintln!("thread cleanup: {} bindings inlined ({} computed snapshots), {} rounds, {} -> {} bytes, {:.2?}",
            cleaned.inlined_bindings, cleaned.inlined_snapshots, cleaned.rounds,
            source.len(), cleaned.code.len(), started.elapsed());
        eprintln!(
            "vm_sub cleanup: {} calls inlined, {} declarations removed, {} remaining",
            cleaned.inlined_helper_calls, cleaned.removed_helpers, cleaned.remaining_helpers
        );
        eprintln!("expression cleanup: {} property writes simplified, {} arrays folded, {} method calls restored", cleaned.rewritten_property_writes, cleaned.folded_arrays, cleaned.simplified_method_calls);
        eprintln!(
            "loop cleanup: {} loops reconstructed, {} blocks flattened",
            cleaned.reconstructed_loops, cleaned.flattened_blocks
        );
        Ok(())
    };
    match work() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => super::failure("cleanup-thread", json, &error, 1),
    }
}

fn parse(args: Vec<String>) -> Result<(PathBuf, Option<PathBuf>), String> {
    let (mut input, mut output) = (None, None);
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => {}
            "--out" if output.is_none() => {
                output = Some(PathBuf::from(args.next().ok_or("--out needs one file")?))
            }
            option if option.starts_with('-') => return Err(format!("unknown option {option}")),
            path if input.is_none() => input = Some(PathBuf::from(path)),
            _ => return Err("expected one thread file".into()),
        }
    }
    Ok((input.ok_or("missing thread file")?, output))
}
