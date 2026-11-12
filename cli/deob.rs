//! `shape deob`: deobfuscate one JavaScript source.

use std::{path::Path, process::ExitCode};

use shape::{deobfuscate, deobfuscate_with_report, KernelArtifact, PrefixLimits};

use super::{atomic_write, print_help, usage_error, write_source_atomically};
use crate::cli;

const DEOB_USAGE: &str =
    "usage: shape deob <source> [-o|--output <output>] [-v|--verbose] [--force] [--report <path>]\n\n\
--report <path> also saves the compiled source-free kernel artifact.\n\
`shape trace thread --report <path> ...` traces it without the raw source.\n\
Legacy shape-pipeline-report/1 JSON is not accepted; regenerate it from the\noriginal raw source with this command.";

/// Run `shape deob`.
pub fn run(args: Vec<String>) -> ExitCode {
    let parsed = match parse_deob(args) {
        Ok(Some(parsed)) => parsed,
        Ok(None) => return print_help(DEOB_USAGE),
        Err(error) => return usage_error("shape deob", &error, DEOB_USAGE),
    };

    let input = Path::new(&parsed.input);
    let source = match cli::read_source(input) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("shape deob: {error}");
            return ExitCode::from(1);
        }
    };
    if parsed.verbose || parsed.report.is_some() || parsed.output.is_some() {
        let report = match deobfuscate_with_report(&source.text) {
            Ok(report) => report,
            Err(error) => {
                eprintln!("shape deob: {error}");
                return ExitCode::from(1);
            }
        };
        let artifact = if parsed.report.is_some() {
            match KernelArtifact::compile_from_raw(&source.text, &report, PrefixLimits::default()) {
                Ok(artifact) => Some(artifact),
                Err(error) => {
                    eprintln!("shape deob: cannot produce kernel artifact: {error}");
                    return ExitCode::from(1);
                }
            }
        } else {
            None
        };
        if let Some(output) = &parsed.output {
            if let Err(error) =
                write_source_atomically(input, Path::new(output), &report.output, parsed.force)
            {
                eprintln!("shape deob: {error}");
                return ExitCode::from(1);
            }
        }
        if let (Some(path), Some(artifact)) = (parsed.report.as_deref(), artifact.as_ref()) {
            if let Err(error) = write_artifact_atomically(Path::new(path), artifact) {
                eprintln!("shape deob: cannot write kernel artifact to {path}: {error}");
                return ExitCode::from(1);
            }
        }
        if parsed.output.is_some() {
            if parsed.verbose {
                if let Err(error) =
                    cli::write_verbose_report(&report, parsed.output.as_deref().map(Path::new))
                {
                    eprintln!("shape deob: failed to write verbose report: {error}");
                    return ExitCode::from(1);
                }
            }
            return ExitCode::SUCCESS;
        }
        return write_deob_stdout(&report.output, parsed.verbose.then_some(&report));
    }
    let output = match deobfuscate(&source.text) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("shape deob: {error}");
            return ExitCode::from(1);
        }
    };
    write_deob_stdout(&output, None)
}

fn write_artifact_atomically(path: &Path, artifact: &KernelArtifact) -> std::io::Result<()> {
    let json = artifact
        .to_json()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string()))?;
    atomic_write(path, json.as_bytes(), true)
}

fn write_deob_stdout(output: &str, report: Option<&shape::PipelineReport>) -> ExitCode {
    match cli::write_stdout(output.as_bytes()) {
        Ok(false) => return ExitCode::SUCCESS,
        Ok(true) => {}
        Err(error) => {
            eprintln!("shape deob: failed to write stdout: {error}");
            return ExitCode::from(1);
        }
    }
    if let Some(report) = report {
        if let Err(error) = cli::write_verbose_report(report, None) {
            eprintln!("shape deob: failed to write verbose report: {error}");
            return ExitCode::from(1);
        }
    }
    ExitCode::SUCCESS
}

struct DeobArgs {
    input: String,
    output: Option<String>,
    verbose: bool,
    force: bool,
    report: Option<String>,
}

/// `Ok(None)` means help was requested.
fn parse_deob(args: Vec<String>) -> Result<Option<DeobArgs>, String> {
    let mut args = args.into_iter();
    let mut input = None;
    let mut output = None;
    let mut verbose = false;
    let mut force = false;
    let mut report = None;

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "-h" | "--help" => return Ok(None),
            "-o" | "--output" => {
                if output.is_some() {
                    return Err("output option provided more than once".into());
                }
                let Some(path) = args.next() else {
                    return Err(format!("missing path after {argument}"));
                };
                if path.starts_with('-') {
                    return Err(format!("missing path after {argument}"));
                }
                output = Some(path);
            }
            "-v" | "--verbose" => {
                if verbose {
                    return Err("verbose option provided more than once".into());
                }
                verbose = true;
            }
            "--report" => {
                if report.is_some() {
                    return Err("report option provided more than once".into());
                }
                match args.next() {
                    Some(path) if !path.starts_with('-') => report = Some(path),
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

    let input = input.ok_or_else(|| "missing input path".to_string())?;
    if force && output.is_none() {
        return Err("--force requires --output".into());
    }
    Ok(Some(DeobArgs {
        input,
        output,
        verbose,
        force,
        report,
    }))
}
