//! The `shape` command line (binary only; not part of the library).
//!
//! One module per command family. This file owns the plumbing whose behavior
//! must be identical across commands: stdout handling (including a clean
//! closed pipe), help/usage output, and diagnostics that must stay on stderr.
//! Exact source reading and SHA-256 identity live in [`shape::source`].

pub mod decode;
pub mod deob;
pub mod kernel;
pub mod naming;
pub mod ops;
pub mod payload;
pub mod report;
pub mod seeds;
pub mod signals;
pub mod show;
pub mod trace;

use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use shape::PipelineReport;

pub use shape::source::{read_source, sha256_hex, SourceBytes};

/// Write bytes to stdout, treating a closed downstream pipe as a clean success.
///
/// `Ok(false)` means the consumer closed the pipe; callers return status 0.
pub fn write_stdout(bytes: &[u8]) -> io::Result<bool> {
    write_stdout_parts(&[bytes])
}

/// Write one line (text plus a trailing newline) for a machine-readable view.
pub fn write_stdout_line(text: &str) -> io::Result<bool> {
    write_stdout_parts(&[text.as_bytes(), b"\n"])
}

fn write_stdout_parts(parts: &[&[u8]]) -> io::Result<bool> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let result = (|| {
        for part in parts {
            stdout.write_all(part)?;
        }
        stdout.flush()
    })();
    match result {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(false),
        Err(error) => Err(error),
    }
}

/// Render the optional pipeline/extraction report on stderr.
pub fn write_verbose_report(report: &PipelineReport, output: Option<&Path>) -> io::Result<()> {
    let stderr = io::stderr();
    let mut stderr = stderr.lock();
    if let Some(output) = output {
        writeln!(stderr, "wrote {}", output.display())?;
    }
    for stage in &report.stages {
        writeln!(
            stderr,
            "{}: {} ({} changes)",
            stage.stage, stage.status, stage.changed_nodes
        )?;
    }
    for fact in &report.extraction {
        writeln!(
            stderr,
            "extract {}: {}{}",
            fact.kind,
            fact.status,
            fact.binding
                .as_deref()
                .map_or(String::new(), |name| format!(" ({name})"))
        )?;
    }
    for diagnostic in &report.diagnostics {
        writeln!(stderr, "{}", diagnostic.render())?;
    }
    Ok(())
}

/// Top-level `shape` usage.
pub const USAGE: &str = r#"usage: shape <command> [options]

commands:
  deob <source>                 deobfuscate one JavaScript source
  report <source>               compile a source-free kernel artifact
  kernel show <source>          show the tracer-input properties bundle
  ops list <source>             list proven operation handlers
  ops show <source> --op KEY    show one operation handler
  ops classify <source>...      label handlers as JavaScript ops, VM machinery,
                                fused or unrecognized, across captures
  show signals [SignalName] [--json]
                                logical cards/data; --artifact/--headers for capture size
  signals <program.js> [--json] [--layouts-only] [--no-finder-roles]
                                print the per-seed signal schema
  signals show|inputs|contracts <program.js> [options]
                                generator-input properties, composed inputs,
                                typed signal contracts
  naming threads|collectors|inventory|plan <program.js> [options]
                                name functions, identify collectors, inspect
  naming corpus <source>... [options]
                                run the naming chain per seed, compare a
                                reviewed baseline, and compare seeds/vendors
  seeds <command> [options]     acquire or inspect async launchers and seed
                                kernels
  pin <program.js> [--init <init.js>|--session <session.json>] [--out seed.json]
                                pin one traced program into a loadable layout
  headers <encode|decode>       report <-> session header bundle
  payload <encode|decode>       report <-> bare finalized payload
  decode --seed RAW --headers JSON --session JSON [--save-artifact JSON]
                                decode captured headers into named signals
  decode --artifact JSON --headers JSON [--session JSON]
                                reuse compiled recipes without seed analysis
  encode / decode --profile     opaque bytes <-> session headers
  pack / unpack                 generic superpack serialization
  gen <command>                 legacy alias for the generator commands above
  trace thread <source> [options]
  trace thread --report REPORT [options]
                                walk one VM thread from raw JavaScript
                                or a saved kernel artifact
  trace program <source> [options]
                                recover a linked program
  trace corpus <source>... [options]
                                compare reachable-function fingerprints
  trace cleanup-thread <thread.js> [--out FILE] [--json]
                                clean one emitted thread independently

Add --json to any trace command for structured output.
Run `shape <command> --help` for command options."#;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputKind {
    Raw,
    Deobfuscated,
}

impl InputKind {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "raw" => Some(Self::Raw),
            "deobfuscated" => Some(Self::Deobfuscated),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutputFormat {
    Json,
    Markdown,
}

impl OutputFormat {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "json" => Some(Self::Json),
            "markdown" | "md" => Some(Self::Markdown),
            _ => None,
        }
    }
}

pub(crate) fn write_source_atomically(
    input: &Path,
    output: &Path,
    source: &str,
    replace: bool,
) -> std::io::Result<()> {
    if output.exists() && fs::canonicalize(input).ok() == fs::canonicalize(output).ok() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "output path is the input path",
        ));
    }
    if !replace && output.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("output already exists: {}", output.display()),
        ));
    }
    atomic_write(output, source.as_bytes(), replace)
}

pub(crate) fn atomic_write(path: &Path, bytes: &[u8], replace: bool) -> std::io::Result<()> {
    let temporary = temporary_path(path);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    if let Err(error) = file.write_all(bytes) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    drop(file);
    let result = if replace {
        fs::rename(&temporary, path)
    } else {
        fs::hard_link(&temporary, path)
    };
    let _ = fs::remove_file(&temporary);
    result
}

fn temporary_path(path: &Path) -> PathBuf {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("tmp");
    path.with_extension(format!("{extension}.tmp-{}", std::process::id()))
}

pub(crate) fn write_formatted_output(label: &str, output: String, pretty: bool) -> ExitCode {
    let output = match pretty_json(&output, pretty) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("{label}: failed to format JSON: {error}");
            return ExitCode::from(1);
        }
    };
    match write_stdout_line(&output) {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{label}: failed to write stdout: {error}");
            ExitCode::from(1)
        }
    }
}

fn pretty_json(output: &str, pretty: bool) -> Result<String, serde_json::Error> {
    if !pretty {
        return Ok(output.to_owned());
    }
    let value: serde_json::Value = serde_json::from_str(output)?;
    serde_json::to_string_pretty(&value)
}

pub(crate) fn print_help(text: &str) -> ExitCode {
    match write_stdout_line(text) {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("shape: failed to write stdout: {error}");
            ExitCode::from(1)
        }
    }
}

/// `<program.js> [--json]`, shared by the single-input inspection views.
pub(crate) struct ShowArgs {
    pub(crate) input: String,
    pub(crate) json: bool,
}

pub(crate) fn parse_show(args: Vec<String>) -> Result<Option<ShowArgs>, String> {
    let mut input = None;
    let mut json = false;
    for argument in args {
        match argument.as_str() {
            "-h" | "--help" => return Ok(None),
            "--json" => json = true,
            other if other.starts_with('-') => return Err(format!("unknown option {other}")),
            other => {
                if input.replace(other.to_string()).is_some() {
                    return Err("more than one input file".to_string());
                }
            }
        }
    }
    let input = input.ok_or_else(|| "missing <program.js>".to_string())?;
    Ok(Some(ShowArgs { input, json }))
}

pub(crate) fn usage_error(label: &str, message: &str, usage: &str) -> ExitCode {
    eprintln!("{label}: {message}\n{usage}");
    ExitCode::from(2)
}

#[cfg(test)]
mod tests {
    use super::pretty_json;

    #[test]
    fn pretty_json_preserves_large_numeric_lexemes() {
        let output = pretty_json(r#"{"value":90071992547409931234567890}"#, true).unwrap();
        assert!(output.contains("90071992547409931234567890"));
    }
}
