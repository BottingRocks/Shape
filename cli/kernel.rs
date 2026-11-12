//! `shape kernel show`: the tracer-input properties bundle.

use std::{path::Path, process::ExitCode};

use shape::{deobfuscate, extract_actual_properties};

use super::{print_help, usage_error, write_formatted_output, InputKind, OutputFormat};
use crate::cli;

const KERNEL_USAGE: &str = "usage: shape kernel show <source> [options]";

const KERNEL_SHOW_USAGE: &str = "usage: shape kernel show <source> [--input-kind raw|deobfuscated] [--format json|markdown] [--pretty] [--op KEY]...\n\
JSON/markdown default to the flat tracer-input bundle: every\n\
operation-table handler as a direct key-to-source map, the string and thread descriptor\n\
arrays under their actual names, merged VM context/memory/stack field records, filtered\n\
scalar/array constants, and the bytecode/dispatch/WASM payloads with their actual names.\n\
Repeatable --op KEY restricts that operation map to those local table keys (no keys\n\
selects all handlers; filtering adds no metadata).";

/// Run `shape kernel`.
pub fn run(args: Vec<String>) -> ExitCode {
    let mut args = args.into_iter();
    let Some(subcommand) = args.next() else {
        eprintln!("shape: missing kernel subcommand\n{KERNEL_USAGE}");
        return ExitCode::from(2);
    };
    match subcommand.as_str() {
        "-h" | "--help" => print_help(KERNEL_USAGE),
        "show" => run_kernel_show(args.collect()),
        other => {
            eprintln!("shape: unknown kernel subcommand {other}\n{KERNEL_USAGE}");
            ExitCode::from(2)
        }
    }
}

struct KernelShowArgs {
    input: String,
    input_kind: InputKind,
    format: OutputFormat,
    pretty: bool,
    operations: Vec<String>,
}

fn run_kernel_show(args: Vec<String>) -> ExitCode {
    let parsed = match parse_kernel_show(args) {
        Ok(Some(parsed)) => parsed,
        Ok(None) => return print_help(KERNEL_SHOW_USAGE),
        Err(error) => return usage_error("shape kernel show", &error, KERNEL_SHOW_USAGE),
    };

    let input = Path::new(&parsed.input);
    let source = match cli::read_source(input) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("shape kernel show: {error}");
            return ExitCode::from(1);
        }
    };
    let standardized = match parsed.input_kind {
        InputKind::Deobfuscated => source.text,
        InputKind::Raw => match deobfuscate(&source.text) {
            Ok(output) => output,
            Err(error) => {
                eprintln!("shape kernel show: raw deobfuscation failed: {error}");
                return ExitCode::from(1);
            }
        },
    };
    let properties = match extract_actual_properties(&standardized) {
        Ok(properties) => properties,
        Err(error) => {
            eprintln!("shape kernel show: {error}");
            return ExitCode::from(1);
        }
    };
    if let Err(error) = properties.validate_selected_operations(&parsed.operations) {
        eprintln!("shape kernel show: {error}");
        return ExitCode::from(2);
    }
    let output = match parsed.format {
        OutputFormat::Markdown => properties.tracer_markdown(&parsed.operations),
        OutputFormat::Json => properties.tracer_json(&parsed.operations),
    };
    write_formatted_output("shape kernel show", output, parsed.pretty)
}

fn parse_kernel_show(args: Vec<String>) -> Result<Option<KernelShowArgs>, String> {
    let mut args = args.into_iter();
    let mut input = None;
    let mut input_kind = None;
    let mut format = None;
    let mut pretty = false;
    let mut operations = Vec::new();

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
            "--format" => {
                if format.is_some() {
                    return Err("format option provided more than once".into());
                }
                let Some(value) = args.next() else {
                    return Err("--format requires json or markdown".into());
                };
                format =
                    Some(OutputFormat::parse(&value).ok_or_else(|| {
                        format!("format must be json or markdown, got `{value}`")
                    })?);
            }
            "--pretty" => {
                if pretty {
                    return Err("pretty option provided more than once".into());
                }
                pretty = true;
            }
            "--op" => {
                let Some(key) = args.next() else {
                    return Err("--op requires a table key".into());
                };
                operations.push(key);
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

    let format = format.unwrap_or(OutputFormat::Json);
    if pretty && format == OutputFormat::Markdown {
        return Err("--pretty is only supported with JSON output".into());
    }
    Ok(Some(KernelShowArgs {
        input: input.ok_or_else(|| "missing input path".to_string())?,
        input_kind: input_kind.unwrap_or(InputKind::Raw),
        format,
        pretty,
        operations,
    }))
}
