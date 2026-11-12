//! Trace command routing and shared JSON result output.
use serde_json::{json, Value as JsonValue};
use std::process::ExitCode;

const TRACE_USAGE: &str = "usage: shape trace <thread|cleanup-thread|corpus|program> ...

  thread          Recover one function from an explicit entry.
  program         Recover the reachable functions as linked JavaScript.
  corpus          Compare reachable-function fingerprints across captures.
  cleanup-thread  Clean an emitted function.

Add --json to any command for one structured result on stdout.
Use the command's --help for its flags.";

/// Run the trace command from the root command's remaining arguments.
pub fn run(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(command) = args.next() else {
        return usage_error("shape trace", "missing trace subcommand", TRACE_USAGE);
    };
    match command.as_str() {
        "-h" | "--help" => print_help("shape trace", TRACE_USAGE),
        "thread" => thread::run_thread(args.collect()),
        "program" => program::run_program(args.collect()),
        "corpus" => corpus::run_corpus(args.collect()),
        "cleanup-thread" => cleanup_thread::run(args.collect()),
        other => usage_error(
            "shape trace",
            &format!("unknown trace subcommand {other}"),
            TRACE_USAGE,
        ),
    }
}

/// Emit exactly one JSON document. A broken output stream is a command failure.
fn write_json(command: &str, status: &str, result: JsonValue) -> Result<(), String> {
    let document = json!({"version": 1, "command": command, "status": status, "result": result});
    crate::cli::write_stdout_line(&document.to_string())
        .and_then(|written| {
            if written {
                Ok(())
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "stdout closed",
                ))
            }
        })
        .map_err(|error| format!("failed to write JSON to stdout: {error}"))
}

fn failure(command: &str, json: bool, error: &str, code: u8) -> ExitCode {
    eprintln!("shape trace {command}: {error}");
    if json {
        if let Err(error) = write_json(
            command,
            "failure",
            json!({"error": error, "exit_code": code}),
        ) {
            eprintln!("shape trace {command}: {error}");
            return ExitCode::from(1);
        }
    }
    ExitCode::from(code)
}

fn diagnostics(trace: &shape::TraceReport) -> Vec<JsonValue> {
    trace
        .diagnostics
        .iter()
        .map(|item| {
            json!({
                "code": item.code.to_string(), "message": item.message,
                "provenance": item.provenance.as_ref().map(|p| json!({
                    "handler_index": p.handler_index, "table_key": p.table_key,
                    "semantic_identity": p.semantic_identity.to_string(),
                    "step_order": p.step_order, "trace_step": p.trace_step,
                    "source_start": p.span.start, "source_end": p.span.end,
                })),
            })
        })
        .collect()
}

fn javascript(code: &str, path: Option<&std::path::Path>, incomplete: bool) -> JsonValue {
    json!({
        "status": if path.is_some() { "written" } else { "emitted" },
        "code": if path.is_none() { Some(code) } else { None },
        "path": path,
        "bytes": code.len(),
        "incomplete": incomplete,
    })
}

fn print_help(label: &str, text: &str) -> ExitCode {
    match crate::cli::write_stdout_line(text) {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{label}: failed to write stdout: {error}");
            ExitCode::from(1)
        }
    }
}
fn usage_error(label: &str, message: &str, usage: &str) -> ExitCode {
    eprintln!("{label}: {message}\n{usage}");
    ExitCode::from(2)
}
mod cleanup_thread;
pub(crate) mod corpus;
pub(crate) mod program;
pub(crate) mod thread;

/// The branches `--fake-branches FILE` and `--not-fake-branches FILE` decide
/// (see [`shape::BranchOverrides`]): an object of `"pc/state"` to the side
/// taken (`true`, the jump), and an array of `"pc/state"`, as `shape/`'s
/// trace read them.
pub(crate) fn branch_overrides(
    fake: Option<&std::path::Path>,
    real: Option<&std::path::Path>,
) -> Result<shape::BranchOverrides, String> {
    let read = |path: &std::path::Path| -> Result<JsonValue, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
    };
    let mut overrides = shape::BranchOverrides::default();
    if let Some(path) = fake {
        let JsonValue::Object(entries) = read(path)? else {
            return Err(format!(
                "{}: --fake-branches takes an object of \"pc/state\": true or false",
                path.display()
            ));
        };
        for (position, taken) in entries {
            let taken = taken.as_bool().ok_or_else(|| {
                format!(
                    "{}: the side of {position} is not true or false",
                    path.display()
                )
            })?;
            overrides
                .fake
                .insert(shape::BranchOverrides::position(&position)?, taken);
        }
    }
    if let Some(path) = real {
        let JsonValue::Array(entries) = read(path)? else {
            return Err(format!(
                "{}: --not-fake-branches takes an array of \"pc/state\"",
                path.display()
            ));
        };
        for position in entries {
            let position = position.as_str().ok_or_else(|| {
                format!(
                    "{}: {position} is not a \"pc/state\" string",
                    path.display()
                )
            })?;
            let at = shape::BranchOverrides::position(position)?;
            if overrides.fake.contains_key(&at) {
                return Err(format!(
                    "{position} is in both --fake-branches and --not-fake-branches"
                ));
            }
            overrides.real.insert(at);
        }
    }
    Ok(overrides)
}
