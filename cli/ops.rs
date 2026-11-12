//! `shape ops`: list, show and classify proven operation handlers.

use std::{path::Path, process::ExitCode};

use shape::{
    catalogue_from_report, classification_json, classification_markdown, classify_report,
    deobfuscate_with_report,
    operations::view::{
        available_keys, handler_by_key, ops_list_json, ops_list_markdown, ops_show_json,
        ops_show_markdown, Analyses, Capture, OpsView, SpanSource,
    },
    NamedSeed,
};

use super::{print_help, usage_error, write_formatted_output, OutputFormat};
use crate::cli;

const OPS_USAGE: &str = "usage: shape ops <list|show|classify> <source> [options]";

const OPS_CLASSIFY_USAGE: &str =
    "usage: shape ops classify <source>... [--format json|markdown] [--pretty] [--ops]\n\n\
Classifies every handler of each capture from its HandlerIR. A kind seen in\n\
both the JavaScript VM and the WebAssembly VM is common; otherwise it is\n\
specific to one VM. --ops lists every handler in the markdown output.";

const OPS_LIST_USAGE: &str = "usage: shape ops list <source> [--format json|markdown] [--pretty]";

const OPS_SHOW_USAGE: &str = "usage: shape ops show <source> --op KEY [--format json|markdown] [--pretty] [--annotation TEXT]\n\
--annotation is a display-only analyst note. It is never treated as proof or\n\
execution authority.";

/// Run `shape ops`.
pub fn run(args: Vec<String>) -> ExitCode {
    let mut args = args.into_iter();
    let Some(subcommand) = args.next() else {
        eprintln!("shape: missing ops subcommand\n{OPS_USAGE}");
        return ExitCode::from(2);
    };
    match subcommand.as_str() {
        "-h" | "--help" => print_help(OPS_USAGE),
        "list" => run_ops_list(args.collect()),
        "show" => run_ops_show(args.collect()),
        "classify" => run_ops_classify(args.collect()),
        other => {
            eprintln!("shape: unknown ops subcommand {other}\n{OPS_USAGE}");
            ExitCode::from(2)
        }
    }
}

struct OpsListArgs {
    input: String,
    format: OutputFormat,
    pretty: bool,
}

fn run_ops_list(args: Vec<String>) -> ExitCode {
    let parsed = match parse_ops_list(args) {
        Ok(Some(parsed)) => parsed,
        Ok(None) => return print_help(OPS_LIST_USAGE),
        Err(error) => return usage_error("shape ops list", &error, OPS_LIST_USAGE),
    };

    let prepared = match prepare_catalogue(&parsed.input, "shape ops list") {
        Ok(prepared) => prepared,
        Err(code) => return code,
    };
    let catalogue = match catalogue_from_report(&prepared.report) {
        Ok(catalogue) => catalogue,
        Err(error) => {
            eprintln!("shape ops list: {error}");
            return ExitCode::from(1);
        }
    };
    let view = OpsView {
        catalogue: &catalogue,
        capture: prepared.capture(),
        span_source: None,
        analyses: None,
        annotation: None,
    };
    let output = match parsed.format {
        OutputFormat::Json => ops_list_json(&view),
        OutputFormat::Markdown => ops_list_markdown(&view),
    };
    write_formatted_output("shape ops list", output, parsed.pretty)
}

fn parse_ops_list(args: Vec<String>) -> Result<Option<OpsListArgs>, String> {
    let mut args = args.into_iter();
    let mut input = None;
    let mut format = None;
    let mut pretty = false;

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "-h" | "--help" => return Ok(None),
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
    Ok(Some(OpsListArgs {
        input: input.ok_or_else(|| "missing input path".to_string())?,
        format,
        pretty,
    }))
}

fn run_ops_classify(args: Vec<String>) -> ExitCode {
    const LABEL: &str = "shape ops classify";
    let mut inputs = Vec::new();
    let mut format = None;
    let mut pretty = false;
    let mut include_ops = false;
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "-h" | "--help" => return print_help(OPS_CLASSIFY_USAGE),
            "--ops" => include_ops = true,
            "--format" => {
                let parsed = args.next().as_deref().and_then(OutputFormat::parse);
                if format.is_some() || parsed.is_none() {
                    return usage_error(
                        LABEL,
                        "--format takes json or markdown once",
                        OPS_CLASSIFY_USAGE,
                    );
                }
                format = parsed;
            }
            "--pretty" => {
                if pretty {
                    return usage_error(
                        LABEL,
                        "pretty option provided more than once",
                        OPS_CLASSIFY_USAGE,
                    );
                }
                pretty = true;
            }
            option if option.starts_with('-') => {
                return usage_error(
                    LABEL,
                    &format!("unrecognized option {option}"),
                    OPS_CLASSIFY_USAGE,
                );
            }
            input => inputs.push(input.to_owned()),
        }
    }
    if inputs.is_empty() {
        return usage_error(LABEL, "missing input path", OPS_CLASSIFY_USAGE);
    }
    let format = format.unwrap_or(OutputFormat::Markdown);
    if pretty && format == OutputFormat::Markdown {
        return usage_error(
            LABEL,
            "--pretty is only supported with JSON output",
            OPS_CLASSIFY_USAGE,
        );
    }
    let mut classified = Vec::with_capacity(inputs.len());
    for input in &inputs {
        let prepared = match prepare_catalogue(input, LABEL) {
            Ok(prepared) => prepared,
            Err(code) => return code,
        };
        match classify_report(&prepared.report) {
            Ok(classification) => classified.push(classification),
            Err(error) => {
                eprintln!("{LABEL}: {input}: {error}");
                return ExitCode::from(1);
            }
        }
    }
    let names: Vec<String> = inputs
        .iter()
        .map(|input| {
            Path::new(input)
                .file_name()
                .map_or_else(|| input.clone(), |name| name.to_string_lossy().into_owned())
        })
        .collect();
    let seeds: Vec<NamedSeed<'_>> = names
        .iter()
        .zip(&classified)
        .map(|(name, classification)| NamedSeed {
            name,
            classification,
        })
        .collect();
    let output = match format {
        OutputFormat::Json => classification_json(&seeds),
        OutputFormat::Markdown => classification_markdown(&seeds, include_ops),
    };
    write_formatted_output(LABEL, output, pretty)
}

struct OpsShowArgs {
    input: String,
    key: u32,
    format: OutputFormat,
    pretty: bool,
    annotation: Option<String>,
}

fn run_ops_show(args: Vec<String>) -> ExitCode {
    let parsed = match parse_ops_show(args) {
        Ok(Some(parsed)) => parsed,
        Ok(None) => return print_help(OPS_SHOW_USAGE),
        Err(error) => return usage_error("shape ops show", &error, OPS_SHOW_USAGE),
    };

    let prepared = match prepare_catalogue(&parsed.input, "shape ops show") {
        Ok(prepared) => prepared,
        Err(code) => return code,
    };
    let catalogue = match catalogue_from_report(&prepared.report) {
        Ok(catalogue) => catalogue,
        Err(error) => {
            eprintln!("shape ops show: {error}");
            return ExitCode::from(1);
        }
    };
    let Some(handler) = handler_by_key(&catalogue, parsed.key) else {
        let keys = available_keys(&catalogue);
        let available = if keys.is_empty() {
            "none".to_string()
        } else {
            keys.iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        };
        eprintln!(
            "shape ops show: unknown operation key {}; available operation keys: {available}",
            parsed.key
        );
        return ExitCode::from(2);
    };
    let span_digest = cli::sha256_hex(prepared.report.output.as_bytes());
    let analyses = Analyses::from_report(&prepared.report);
    let view = OpsView {
        catalogue: &catalogue,
        capture: prepared.capture(),
        span_source: Some(SpanSource {
            digest: &span_digest,
            byte_length: prepared.report.output.len(),
        }),
        analyses: Some(&analyses),
        annotation: parsed.annotation.as_deref(),
    };
    let output = match parsed.format {
        OutputFormat::Json => ops_show_json(&view, handler),
        OutputFormat::Markdown => ops_show_markdown(&view, handler),
    };
    write_formatted_output("shape ops show", output, parsed.pretty)
}

fn parse_ops_show(args: Vec<String>) -> Result<Option<OpsShowArgs>, String> {
    let mut args = args.into_iter();
    let mut input = None;
    let mut key = None;
    let mut format = None;
    let mut pretty = false;
    let mut annotation = None;

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "-h" | "--help" => return Ok(None),
            "--op" => {
                if key.is_some() {
                    return Err("--op may be provided only once".into());
                }
                let Some(value) = args.next() else {
                    return Err("--op requires a table key".into());
                };
                key = Some(value.parse::<u32>().map_err(|_| {
                    format!("operation key must be a non-negative integer, got `{value}`")
                })?);
            }
            "--annotation" => {
                if annotation.is_some() {
                    return Err("annotation option provided more than once".into());
                }
                let Some(value) = args.next() else {
                    return Err("--annotation requires display-only text".into());
                };
                annotation = Some(value);
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
    Ok(Some(OpsShowArgs {
        input: input.ok_or_else(|| "missing input path".to_string())?,
        key: key.ok_or_else(|| "missing --op KEY".to_string())?,
        format,
        pretty,
        annotation,
    }))
}

struct PreparedCatalogue {
    report: shape::PipelineReport,
    digest: String,
    byte_length: u64,
}

impl PreparedCatalogue {
    fn capture(&self) -> Capture<'_> {
        Capture {
            input_kind: "raw",
            digest: &self.digest,
            byte_length: self.byte_length,
            raw_identity: "verified",
        }
    }
}

/// Read the exact raw bytes and run the single deobfuscation report used by an
/// `ops` invocation. The SHA-256 is computed before UTF-8 decoding.
fn prepare_catalogue(input: &str, label: &str) -> Result<PreparedCatalogue, ExitCode> {
    let path = Path::new(input);
    let source = match cli::read_source(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("{label}: {error}");
            return Err(ExitCode::from(1));
        }
    };
    let digest = cli::sha256_hex(&source.bytes);
    let byte_length = source.bytes.len() as u64;
    let report = match deobfuscate_with_report(&source.text) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("{label}: {error}");
            return Err(ExitCode::from(1));
        }
    };
    Ok(PreparedCatalogue {
        report,
        digest,
        byte_length,
    })
}
