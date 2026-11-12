//! `shape report`: compile a source-free kernel artifact.

use std::{path::Path, process::ExitCode};

use shape::{KernelArtifact, PrefixLimits};

use super::{print_help, usage_error, write_source_atomically, InputKind};
use crate::cli;

const REPORT_USAGE: &str =
    "usage: shape report <source> [--input-kind raw|deobfuscated] [-o|--output <artifact>] [--force]\n\n\
Compile the source-free kernel artifact without writing deobfuscated JavaScript.\n\
--input-kind deobfuscated accepts standardized output. That artifact records\n\
a declared-only source basis; it is not a raw-source-observed compile.\n\
Without --output the artifact JSON is written to stdout. --force allows an\n\
existing --output file to be replaced.";

/// Run `shape report`.
pub fn run(args: Vec<String>) -> ExitCode {
    let parsed = match parse_report(args) {
        Ok(Some(parsed)) => parsed,
        Ok(None) => return print_help(REPORT_USAGE),
        Err(error) => return usage_error("shape report", &error, REPORT_USAGE),
    };

    let input = Path::new(&parsed.input);
    let source = match cli::read_source(input) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("shape report: {error}");
            return ExitCode::from(1);
        }
    };
    let limits = PrefixLimits::default();
    let artifact = match parsed.input_kind {
        InputKind::Raw => KernelArtifact::compile_source(&source.text, limits),
        InputKind::Deobfuscated => KernelArtifact::compile_from_deobfuscated(&source.text, limits),
    };
    let artifact = match artifact {
        Ok(artifact) => artifact,
        Err(error) => {
            eprintln!("shape report: cannot compile kernel artifact: {error}");
            return ExitCode::from(1);
        }
    };
    let json = match artifact.to_json() {
        Ok(json) => json,
        Err(error) => {
            eprintln!("shape report: cannot serialize kernel artifact: {error}");
            return ExitCode::from(1);
        }
    };
    match parsed.output.as_deref() {
        Some(output) => {
            match write_source_atomically(input, Path::new(output), &json, parsed.force) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("shape report: cannot write kernel artifact to {output}: {error}");
                    ExitCode::from(1)
                }
            }
        }
        None => match cli::write_stdout_line(&json) {
            Ok(_) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("shape report: failed to write stdout: {error}");
                ExitCode::from(1)
            }
        },
    }
}

struct ReportArgs {
    input: String,
    input_kind: InputKind,
    output: Option<String>,
    force: bool,
}

/// `Ok(None)` means help was requested.
fn parse_report(args: Vec<String>) -> Result<Option<ReportArgs>, String> {
    let mut args = args.into_iter();
    let mut input = None;
    let mut input_kind = None;
    let mut output = None;
    let mut force = false;

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "-h" | "--help" => return Ok(None),
            "--input-kind" => {
                if input_kind.is_some() {
                    return Err("input kind option provided more than once".into());
                }
                let Some(value) = args.next() else {
                    return Err("--input-kind requires raw or deobfuscated".into());
                };
                input_kind = Some(InputKind::parse(&value).ok_or_else(|| {
                    format!("input kind must be raw or deobfuscated, got `{value}`")
                })?);
            }
            "-o" | "--output" => {
                if output.is_some() {
                    return Err("output option provided more than once".into());
                }
                match args.next() {
                    Some(path) if !path.starts_with('-') => output = Some(path),
                    _ => return Err(format!("missing path after {argument}")),
                }
            }
            "--force" => {
                if force {
                    return Err("force option provided more than once".into());
                }
                force = true;
            }
            option if option.starts_with('-') => {
                return Err(format!("unrecognized option {option}"));
            }
            value => {
                if input.is_some() {
                    return Err(format!("unexpected extra argument {value}"));
                }
                input = Some(value.to_owned());
            }
        }
    }

    if force && output.is_none() {
        return Err("--force requires --output".into());
    }
    Ok(Some(ReportArgs {
        input: input.ok_or_else(|| "missing input path".to_string())?,
        input_kind: input_kind.unwrap_or(InputKind::Raw),
        output,
        force,
    }))
}
