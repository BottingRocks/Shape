//! `shape seeds`: fetch, parse and import async launchers and seed kernels.
//!
//! This is the retained clap parser for the acquisition subcommands only. The
//! root `shape` entry point keeps its existing handwritten routing and hands
//! the remaining arguments to [`run`], which stays in-process and never shells
//! out to another executable.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use clap::{Args, Parser, Subcommand};

use crate::cli::write_stdout_line;
use shape::seeds::collect::{self, Attempt, FetchOptions, Source};
use shape::seeds::http::{ClientConfig, ReqwestClient};
use shape::seeds::script::{self, Script};
use shape::seeds::seed::{self, SeedToken, DEFAULT_SEED_PARAM};
use shape::seeds::sites::{self, Protocol, SiteStatus, SITES};
use shape::seeds::store::{SeedStore, StoreOutcome, StoreStatus};
use shape::seeds::{Error, Result};

#[derive(Parser)]
#[command(
    name = "shape seeds",
    version,
    about = "Acquire async launchers and their seed kernels"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Fetch launchers and the kernels they load into <out>/<seed>.{async,raw}.js
    Fetch(FetchArgs),
    /// Classify saved files: launchers print seed + kernel URL, kernels a summary
    Parse(ParseArgs),
    /// Store an already downloaded raw kernel (and its launcher) without any network access
    Import(ImportArgs),
    /// List the site registry
    Sites,
}

#[derive(Args)]
struct FetchArgs {
    /// Launcher URLs to fetch
    #[arg(value_name = "URL", required_unless_present = "site")]
    urls: Vec<String>,
    /// Fetch a validated site from the registry by name (repeatable)
    #[arg(long, value_name = "NAME")]
    site: Vec<String>,
    /// Request each launcher this many times, to collect several seeds
    #[arg(long, short = 'n', default_value_t = 1, value_parser = clap::value_parser!(u32).range(1..=1000))]
    count: u32,
    /// Pause between launcher requests
    #[arg(long, value_name = "MS", default_value_t = 0)]
    delay_ms: u64,
    /// Directory for <seed>.async.js and <seed>.raw.js
    #[arg(long, short, value_name = "DIR", default_value = "seeds")]
    out: PathBuf,
    /// Extra request header, `name: value` (repeatable; overrides defaults)
    #[arg(long = "header", short = 'H', value_name = "HEADER")]
    headers: Vec<String>,
    /// Shorthand for -H 'user-agent: …' (defaults use a recent Chrome profile)
    #[arg(long, short = 'A', value_name = "UA")]
    user_agent: Option<String>,
    /// Shorthand for -H 'referer: …'
    #[arg(long, short = 'e', value_name = "URL")]
    referer: Option<String>,
    /// Base for a relative kernel URL (default: the launcher URL, or the site's origin)
    #[arg(long, value_name = "URL")]
    base_url: Option<String>,
    /// Query parameter of the kernel URL that carries the seed token
    #[arg(long, value_name = "NAME")]
    seed_param: Option<String>,
    /// Whole-request timeout in seconds
    #[arg(long, value_name = "SECS", default_value_t = 30)]
    timeout: u64,
    /// Largest response body accepted, in bytes
    #[arg(long, value_name = "BYTES", default_value_t = 32 * 1024 * 1024)]
    max_bytes: u64,
}

#[derive(Args)]
struct ParseArgs {
    /// Saved launcher (.async.js) and/or kernel (.raw.js) files
    #[arg(value_name = "FILE", required = true)]
    files: Vec<PathBuf>,
    /// Base for a relative kernel URL
    #[arg(long, value_name = "URL", conflicts_with = "site")]
    base_url: Option<String>,
    /// Take base URL and seed parameter from a registry site
    #[arg(long, value_name = "NAME")]
    site: Option<String>,
    /// Query parameter of the kernel URL that carries the seed token
    #[arg(long, value_name = "NAME")]
    seed_param: Option<String>,
}

#[derive(Args)]
#[command(group(
    clap::ArgGroup::new("identity")
        .required(true)
        .multiple(false)
        .args(["launcher", "seed"])
))]
struct ImportArgs {
    /// Saved raw kernel
    #[arg(value_name = "KERNEL")]
    kernel: PathBuf,
    /// The saved launcher that loaded this kernel; the seed token is read from it
    #[arg(long, short, value_name = "FILE")]
    launcher: Option<PathBuf>,
    /// Seed token, when no launcher was saved
    #[arg(long, value_name = "TOKEN")]
    seed: Option<String>,
    /// Directory for <seed>.async.js and <seed>.raw.js
    #[arg(long, short, value_name = "DIR", default_value = "seeds")]
    out: PathBuf,
    /// Query parameter of the kernel URL that carries the seed token
    #[arg(long, value_name = "NAME", default_value = DEFAULT_SEED_PARAM)]
    seed_param: String,
}

/// Run one `shape seeds ...` invocation. The argument list excludes the
/// top-level `shape seeds` words already consumed by the root router.
pub fn run(args: impl IntoIterator<Item = String>) -> ExitCode {
    let mut argv = vec!["shape seeds".to_owned()];
    argv.extend(args);
    let command = match Cli::try_parse_from(argv) {
        Ok(cli) => cli.command,
        Err(error) => return exit_for_clap_error(&error),
    };
    let mut output = Output::stdout();
    let outcome = match command {
        Command::Fetch(args) => fetch(&args, &mut output),
        Command::Parse(args) => parse(&args, &mut output),
        Command::Import(args) => import(&args, &mut output),
        Command::Sites => {
            list_sites(&mut output);
            Ok(true)
        }
    };
    if let Some(error) = output.take_error() {
        eprintln!("shape seeds: failed to write stdout: {error}");
        return ExitCode::FAILURE;
    }
    match outcome {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            print_error(None, &error);
            ExitCode::FAILURE
        }
    }
}

/// Write seed command output without panicking on a closed stdout pipe.
///
/// `Ok(false)` from the shared helper means the reader closed the pipe; that is
/// a clean exit for otherwise successful work, so later lines are skipped. A
/// real write error is retained for the caller to report as status 1.
type LineWriter = fn(&str) -> io::Result<bool>;

struct Output {
    write_line: LineWriter,
    closed: bool,
    error: Option<io::Error>,
}

impl Output {
    fn stdout() -> Self {
        Self::with_writer(write_stdout_line)
    }

    fn with_writer(write_line: LineWriter) -> Self {
        Self {
            write_line,
            closed: false,
            error: None,
        }
    }

    fn line(&mut self, args: fmt::Arguments<'_>) {
        let mut text = String::new();
        fmt::write(&mut text, args).expect("formatting into a String cannot fail");
        self.emit(&text);
    }

    fn blank(&mut self) {
        self.emit("");
    }

    fn emit(&mut self, text: &str) {
        if self.closed || self.error.is_some() {
            return;
        }
        match (self.write_line)(text) {
            Ok(true) => {}
            Ok(false) => self.closed = true,
            Err(error) => self.error = Some(error),
        }
    }

    fn take_error(&mut self) -> Option<io::Error> {
        self.error.take()
    }
}

/// Print clap's help/version or usage diagnostic through the normal streams
/// and map it onto the CLI status contract (0 help, 2 usage).
fn exit_for_clap_error(error: &clap::Error) -> ExitCode {
    use clap::error::ErrorKind;
    let display = matches!(
        error.kind(),
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
    );
    match error.print() {
        Ok(()) if display => ExitCode::SUCCESS,
        Ok(()) => ExitCode::from(2),
        Err(write_error) if display && write_error.kind() == io::ErrorKind::BrokenPipe => {
            ExitCode::SUCCESS
        }
        Err(write_error) => {
            eprintln!("shape seeds: failed to write diagnostic: {write_error}");
            if display {
                ExitCode::FAILURE
            } else {
                ExitCode::from(2)
            }
        }
    }
}

fn print_error(subject: Option<&str>, error: &Error) {
    match subject {
        Some(subject) => eprintln!("error[{}]: {subject}: {error}", error.kind()),
        None => eprintln!("error[{}]: {error}", error.kind()),
    }
}

fn fetch(args: &FetchArgs, output: &mut Output) -> Result<bool> {
    let mut options = FetchOptions::default();
    if let Some(user_agent) = &args.user_agent {
        options
            .headers
            .set_line(&format!("user-agent: {user_agent}"))?;
    }
    if let Some(referer) = &args.referer {
        options.headers.set_line(&format!("referer: {referer}"))?;
    }
    for line in &args.headers {
        options.headers.set_line(line)?;
    }
    if let Some(base_url) = &args.base_url {
        options.base_url = Some(seed::parse_http_url(base_url)?);
    }
    if let Some(seed_param) = &args.seed_param {
        options.seed_param.clone_from(seed_param);
    }

    let mut sources = Vec::new();
    for name in &args.site {
        let site = sites::validated(name)?;
        let mut options = options.clone();
        for (header, value) in site.headers {
            if options.headers.get(header).is_none() {
                options.headers.set(header, value);
            }
        }
        if options.base_url.is_none() {
            options.base_url = site.base_url.map(seed::parse_http_url).transpose()?;
        }
        if args.seed_param.is_none() {
            options.seed_param = site.seed_param.to_owned();
        }
        sources.push(Source {
            launcher_url: seed::parse_http_url(site.launcher_url)?,
            options,
            protocol: site.protocol,
        });
    }
    for url in &args.urls {
        sources.push(Source {
            launcher_url: seed::parse_http_url(url)?,
            options: options.clone(),
            protocol: Protocol::AsyncLauncher,
        });
    }

    let client = ReqwestClient::new(&ClientConfig {
        timeout: Duration::from_secs(args.timeout),
        max_body_bytes: args.max_bytes,
        ..ClientConfig::default()
    })?;
    let store = SeedStore::new(&args.out);
    let attempts = collect::collect(
        &client,
        &store,
        &sources,
        args.count,
        Duration::from_millis(args.delay_ms),
        |attempt| print_attempt(attempt, output),
    );

    let tally = |status| {
        attempts
            .iter()
            .filter(|attempt| matches!(&attempt.result, Ok(c) if c.outcome.status == status))
            .count()
    };
    let failed = attempts
        .iter()
        .filter(|attempt| attempt.result.is_err())
        .count();
    if attempts.len() > 1 {
        output.line(format_args!(
            "{} requests: {} stored, {} duplicate, {} variant, {} failed",
            attempts.len(),
            tally(StoreStatus::Stored),
            tally(StoreStatus::Duplicate),
            tally(StoreStatus::Variant),
            failed
        ));
    }
    Ok(failed == 0)
}

fn print_attempt(attempt: &Attempt, output: &mut Output) {
    match &attempt.result {
        Ok(collected) => {
            print_outcome(&collected.seed, &collected.outcome, output);
            output.line(format_args!("  kernel url  {}", collected.kernel_url));
        }
        Err(error) => print_error(Some(attempt.launcher_url.as_str()), error),
    }
}

fn print_outcome(seed: &SeedToken, outcome: &StoreOutcome, output: &mut Output) {
    output.line(format_args!("{:<9} {seed}", outcome.status.as_str()));
    if let Some(launcher_path) = &outcome.launcher_path {
        let note = if outcome.launcher_differs {
            "  (kept; the new launcher differs and was not saved)"
        } else {
            ""
        };
        output.line(format_args!(
            "  launcher    {}{note}",
            launcher_path.display()
        ));
    }
    output.line(format_args!(
        "  kernel      {}",
        outcome.kernel_path.display()
    ));
}

fn parse(args: &ParseArgs, output: &mut Output) -> Result<bool> {
    let site = args.site.as_deref().map(sites::find).transpose()?;
    let base_url = match (&args.base_url, site) {
        (Some(base_url), _) => Some(seed::parse_http_url(base_url)?),
        (None, Some(site)) => site.base_url.map(seed::parse_http_url).transpose()?,
        (None, None) => None,
    };
    let seed_param = args
        .seed_param
        .as_deref()
        .or(site.map(|site| site.seed_param))
        .unwrap_or(DEFAULT_SEED_PARAM);

    let mut all_ok = true;
    for path in &args.files {
        let described = read(path).and_then(|bytes| {
            Ok(match script::classify_with_seed_param(&bytes, seed_param)? {
                Script::Launcher(launcher) => {
                    let kernel_url = seed::resolve_kernel_url(&launcher.kernel_ref, base_url.as_ref())?;
                    let seed = seed::seed_from_url(&kernel_url, seed_param)?;
                    format!("async launcher\n  seed        {seed}\n  kernel url  {kernel_url}")
                }
                Script::Kernel(kernel) => format!(
                    "raw kernel\n  bytes       {}\n  sha256      {}\n  statements  {}\n  functions   {}",
                    kernel.bytes, kernel.sha256, kernel.statements, kernel.functions
                ),
            })
        });
        match described {
            Ok(described) => output.line(format_args!("{}: {described}", path.display())),
            Err(error) => {
                all_ok = false;
                print_error(Some(&path.display().to_string()), &error);
            }
        }
    }
    Ok(all_ok)
}

fn import(args: &ImportArgs, output: &mut Output) -> Result<bool> {
    let kernel = read(&args.kernel)?;
    script::parse_kernel(&kernel)?;
    let (seed, launcher) = match (&args.launcher, &args.seed) {
        (Some(path), _) => {
            let launcher = read(path)?;
            let parsed = script::parse_launcher_with_seed_param(&launcher, &args.seed_param)?;
            let seed = seed::seed_from_reference(&parsed.kernel_ref, &args.seed_param)?;
            (seed, Some(launcher))
        }
        (None, Some(token)) => (SeedToken::new(token.as_str())?, None),
        (None, None) => unreachable!("clap requires --launcher or --seed"),
    };
    let outcome = SeedStore::new(&args.out).save(&seed, launcher.as_deref(), &kernel)?;
    print_outcome(&seed, &outcome, output);
    Ok(true)
}

fn read(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|source| Error::Io {
        path: path.to_owned(),
        source,
    })
}

fn list_sites(output: &mut Output) {
    output.line(format_args!(
        "{:<11} {:<12} {:<15} launcher url",
        "site", "status", "protocol"
    ));
    for site in SITES {
        let status = match site.status {
            SiteStatus::Validated => "validated",
            SiteStatus::Reference => "unvalidated",
        };
        let protocol = match site.protocol {
            Protocol::AsyncLauncher => "async-launcher",
            Protocol::SingleFile => "single-file",
            Protocol::DirectKernel => "direct-kernel",
        };
        output.line(format_args!(
            "{:<11} {status:<12} {protocol:<15} {}",
            site.name, site.launcher_url
        ));
    }
    output.blank();
    output.line(format_args!(
        "Only validated sites are fetched by `fetch --site NAME`. Unvalidated entries are"
    ));
    output.line(format_args!(
        "carried over from the legacy tool for reference and have not been tested here;"
    ));
    output.line(format_args!("single-file sites are not implemented."));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failed(_: &str) -> io::Result<bool> {
        Err(io::Error::other("disk full"))
    }

    fn broken(_: &str) -> io::Result<bool> {
        Ok(false)
    }

    #[test]
    fn real_stdout_errors_are_retained() {
        let mut output = Output::with_writer(failed);
        output.line(format_args!("first"));
        output.blank();
        let error = output.take_error().expect("real write error");
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert_eq!(error.to_string(), "disk full");
        assert!(output.take_error().is_none());
    }

    #[test]
    fn closed_stdout_is_not_an_error() {
        let mut output = Output::with_writer(broken);
        output.line(format_args!("first"));
        output.blank();
        assert!(output.take_error().is_none());
    }
}
