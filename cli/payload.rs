//! `shape gen`: pin one traced seed into a layout artifact, then encode or
//! decode payloads and headers with that artifact or an explicit profile.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
    sync::atomic::{AtomicU64, Ordering},
};

use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};

use shape::signals::payload::typed_json::parse_typed_json;
use shape::signals::payload::superpack::{unpack_canonical, PackRequest, UnpackRequest};
use shape::signals::payload::{
    extract::{pin, session_from_initializer, SeedLayout, Session},
    report::ChannelReport,
    Generator, HeaderReport, Headers, Payload, PayloadCodec, PayloadProfile, Profile,
    MAX_CONTAINER_ENTRIES,
};

#[derive(Subcommand)]
enum PayloadCommand {
    /// Read a report (or four channels with --seeds) and emit custom-base64 + LF.
    Encode {
        #[command(flatten)]
        io: IoArgs,
        /// Payload seeds for a four-channel input document (W1,W2).
        #[arg(long, value_delimiter = ',')]
        seeds: Option<Vec<u32>>,
    },
    /// Read custom-base64; recover report values with external facts.
    Decode {
        #[command(flatten)]
        io: IoArgs,
        /// Overrides the saved layout's or full profile's positional count.
        #[arg(long, value_parser = clap::value_parser!(u32).range(0..=MAX_CONTAINER_ENTRIES as i64))]
        positional_count: Option<u32>,
        /// Ordered omitted keysets JSON; overrides layout/profile defaults (even []).
        #[arg(long)]
        keysets: Option<PathBuf>,
        /// Emit only errors/positionalSignals/signals/keyedSignals.
        #[arg(long)]
        channels: bool,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum HeaderInputFormat {
    Auto,
    Json,
    Text,
}
#[derive(Clone, Copy, ValueEnum)]
enum HeaderOutputFormat {
    Json,
    Text,
}
#[derive(Clone, Copy, ValueEnum)]
enum HeaderEncodeFormat {
    Report,
    Payload,
}

#[derive(Subcommand)]
enum HeadersCommand {
    /// Encode root report JSON (default) or wrap already-finalized payload text.
    Encode {
        #[command(flatten)]
        io: IoArgs,
        /// Payload seeds for a four-channel input document (W1,W2).
        #[arg(long, value_delimiter = ',')]
        seeds: Option<Vec<u32>>,
        #[arg(long, value_enum, default_value = "report")]
        input_format: HeaderEncodeFormat,
        #[arg(long, value_enum, default_value = "json")]
        output_format: HeaderOutputFormat,
    },
    /// Check JSON headers or a strict Name: value block and return a final report.
    Decode {
        #[command(flatten)]
        io: IoArgs,
        #[arg(long, value_enum, default_value = "auto")]
        input_format: HeaderInputFormat,
        /// Overrides the saved layout's or full profile's positional count.
        #[arg(long, value_parser = clap::value_parser!(u32).range(0..=MAX_CONTAINER_ENTRIES as i64))]
        positional_count: Option<u32>,
        /// Ordered omitted keysets JSON; overrides layout/profile defaults (even []).
        #[arg(long)]
        keysets: Option<PathBuf>,
        /// Emit only errors/positionalSignals/signals/keyedSignals.
        #[arg(long)]
        channels: bool,
    },
}

// Accept existing profiles as typed legacy metadata, but payload operations
// use their crypto fields and optional report facts, never header policy or context.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum PayloadProfileInput {
    Payload(PayloadProfile),
    Legacy(Profile),
}

#[derive(Args)]
#[command(group(
    ArgGroup::new("settings")
        .required(true)
        .multiple(false)
        .args(["profile", "seed", "program"])
))]
struct IoArgs {
    /// Explicit shape-byte-codec/1 JSON profile (file required).
    #[arg(long)]
    profile: Option<PathBuf>,
    /// shape-seed-layout/1 artifact from `shape pin` or `--save-seed`.
    #[arg(long)]
    seed: Option<PathBuf>,
    /// Traced program to pin on this run; pair with --init or --session.
    #[arg(long)]
    program: Option<PathBuf>,
    /// Save the pinned artifact here so later runs can use --seed.
    #[arg(long, requires = "program")]
    save_seed: Option<PathBuf>,
    /// Static detail.init(...) JavaScript for --program (file only).
    #[arg(long, requires = "program", conflicts_with = "session")]
    init: Option<PathBuf>,
    /// Normalized session JSON for --program (file only).
    #[arg(long, requires = "program", conflicts_with = "init")]
    session: Option<PathBuf>,
    #[command(flatten)]
    files: FileArgs,
}

impl IoArgs {
    fn settings_path(&self) -> Option<&Path> {
        self.profile
            .as_deref()
            .or(self.seed.as_deref())
            .or(self.program.as_deref())
    }
}

#[derive(Args)]
struct FileArgs {
    /// Input JSON or payload text file, or - for stdin.
    #[arg(long, default_value = "-")]
    input: PathBuf,
    /// Output JSON file, or - for stdout. Existing files need --force.
    #[arg(long, default_value = "-")]
    out: PathBuf,
    /// Atomically replace an existing output file. Input/profile aliases are refused.
    #[arg(long)]
    force: bool,
}

/// A settings file is either an explicit profile or a pinned seed layout. The
/// layout carries decode facts; a full profile may supply each fact independently.
enum Settings {
    Profile(Profile),
    /// Payload commands accept a crypto-only `PayloadProfile`; it has no
    /// header name or policy and therefore cannot format headers.
    Payload(PayloadProfile),
    Layout(SeedLayout),
}

impl Settings {
    fn payload_profile(&self) -> PayloadProfile {
        match self {
            Self::Profile(profile) => profile.clone().into(),
            Self::Payload(profile) => profile.clone(),
            Self::Layout(layout) => layout.profile().into(),
        }
    }

    fn generator(&self) -> Result<Generator, Box<dyn std::error::Error>> {
        Ok(match self {
            Self::Profile(profile) => Generator::new(profile.clone())?,
            Self::Payload(_) => {
                return Err(invalid(
                    "this command needs a full profile or --seed; a payload profile has no header policy",
                )
                .into())
            }
            Self::Layout(layout) => layout.generator()?,
        })
    }

    fn facts(&self) -> ReportFacts<'_> {
        match self {
            Self::Layout(layout) => ReportFacts {
                positional_count: Some(layout.report.positional_signals.len()),
                keysets: Some(&layout.keysets),
            },
            Self::Profile(profile) => ReportFacts {
                positional_count: profile.positional_count,
                keysets: profile.keysets.as_deref(),
            },
            Self::Payload(_) => ReportFacts::default(),
        }
    }
}

/// Missing facts differ from an explicit zero count or empty dictionary.
#[derive(Default)]
struct ReportFacts<'a> {
    positional_count: Option<usize>,
    keysets: Option<&'a [Vec<String>]>,
}

impl ReportFacts<'_> {
    fn resolve_count(&self, explicit: Option<u32>) -> io::Result<Option<usize>> {
        let count = explicit
            .map(|count| count as usize)
            .or(self.positional_count);
        if let Some(count) = count {
            if count > MAX_CONTAINER_ENTRIES {
                return Err(invalid(format!(
                    "positional count {count} exceeds container limit {MAX_CONTAINER_ENTRIES}; supply --positional-count or correct profile metadata"
                )));
            }
        }
        Ok(count)
    }
}

fn load_settings(io: &IoArgs) -> Result<Settings, Box<dyn std::error::Error>> {
    if let Some(program) = &io.program {
        let pinned = pin_program(io, program)?;
        for evidence in &pinned.evidence {
            eprintln!(
                "pinned {} at bytes {}..{} ({})",
                evidence.role, evidence.start, evidence.end, evidence.sha256
            );
        }
        if let Some(path) = &io.save_seed {
            save_layout(io, path, &pinned.layout)?;
        }
        return Ok(Settings::Layout(pinned.layout));
    }
    let path = io.settings_path().expect("clap settings");
    let bytes = read_file(path, 64 * 1024)?;
    if io.seed.is_some() {
        return Ok(Settings::Layout(SeedLayout::from_json(&bytes)?));
    }
    let parsed: PayloadProfileInput =
        serde_json::from_slice(&bytes).map_err(|e| invalid(format!("profile JSON: {e}")))?;
    Ok(match parsed {
        PayloadProfileInput::Payload(profile) => Settings::Payload(profile),
        PayloadProfileInput::Legacy(profile) => Settings::Profile(profile),
    })
}

/// Pin the program named by `--program` with its session literals.
fn pin_program(
    io: &IoArgs,
    program: &Path,
) -> Result<shape::signals::payload::extract::PinnedSeed, Box<dyn std::error::Error>> {
    let session: Session = match (io.init.as_deref(), io.session.as_deref()) {
        (Some(path), None) => {
            let source = read_file(path, 128 * 1024)?;
            session_from_initializer(std::str::from_utf8(&source)?)?
        }
        (None, Some(path)) => serde_json::from_slice(&read_file(path, 128 * 1024)?)
            .map_err(|error| invalid(format!("session JSON: {error}")))?,
        _ => return Err(invalid("--program needs exactly one of --init or --session").into()),
    };
    let source = read_file(program, 16 * 1024 * 1024)?;
    Ok(pin(std::str::from_utf8(&source)?, &session)?)
}

/// Persist the artifact built by `--program`. `--force` replaces an existing
/// file; the save path must not alias the program or the command output.
fn save_layout(
    io: &IoArgs,
    path: &Path,
    layout: &SeedLayout,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = planned_output(path)?;
    if let Some(program) = &io.program {
        reject_alias(
            &output,
            &fs::canonicalize(program)?,
            "save-seed and program paths alias",
        )?;
    }
    if !is_stdio(&io.files.out) {
        reject_alias(
            &output,
            &planned_output(&io.files.out)?,
            "save-seed and output paths alias",
        )?;
    }
    let mut bytes = serde_json::to_vec_pretty(layout)?;
    bytes.push(b'\n');
    write_atomic(path, &bytes, io.files.force)?;
    eprintln!("pinned layout saved to {}", path.display());
    Ok(())
}

/// One top-level command's clap shape and execution.
fn dispatch<T, F>(name: &str, args: impl IntoIterator<Item = String>, run: F) -> ExitCode
where
    T: Parser,
    F: FnOnce(T) -> Result<(), Box<dyn std::error::Error>>,
{
    match T::try_parse_from(std::iter::once(name.to_owned()).chain(args)) {
        Ok(options) => match run(options) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{name}: {error}");
                ExitCode::from(1)
            }
        },
        Err(error) => {
            let code = error.exit_code();
            let _ = error.print();
            ExitCode::from(code as u8)
        }
    }
}

#[derive(Parser)]
#[command(
    name = "shape pin",
    about = "Pin one traced program into a loadable shape-seed-layout/1 artifact."
)]
pub struct PinCli {
    #[command(flatten)]
    files: FileArgs,
    /// Static detail.init(...) JavaScript (file only).
    #[arg(long, conflicts_with = "session")]
    init: Option<PathBuf>,
    /// Normalized session JSON (file only).
    #[arg(long, conflicts_with = "init")]
    session: Option<PathBuf>,
}

#[derive(Parser)]
#[command(
    name = "shape headers",
    about = "Convert final reports or finalized payloads to/from session headers."
)]
struct HeadersCli {
    #[command(subcommand)]
    command: HeadersCommand,
}

#[derive(Parser)]
#[command(
    name = "shape payload",
    about = "Convert generic report JSON to/from a finalized payload, without headers."
)]
struct PayloadCli {
    #[command(subcommand)]
    command: PayloadCommand,
}

#[derive(Parser)]
#[command(
    name = "shape pack",
    about = "Serialize a typed PackRequest into bytes and the same serializer options."
)]
struct PackCli {
    #[command(flatten)]
    files: FileArgs,
}

#[derive(Parser)]
#[command(
    name = "shape unpack",
    about = "Decode canonical bytes into a directly re-encodable typed PackRequest."
)]
struct UnpackCli {
    #[command(flatten)]
    files: FileArgs,
}

#[derive(Parser)]
#[command(
    name = "shape encode",
    about = "Encode opaque bytes into configured a chunks and explicit sibling headers."
)]
struct EncodeCli {
    #[command(flatten)]
    io: IoArgs,
}

#[derive(Parser)]
#[command(
    name = "shape decode",
    about = "Join/check a header bundle and decode into reusable opaque Payload JSON."
)]
struct DecodeCli {
    #[command(flatten)]
    io: IoArgs,
    /// Container count; defaults to layout/profile metadata, else leaves bytes raw.
    #[arg(long, value_parser = clap::value_parser!(u32).range(0..=MAX_CONTAINER_ENTRIES as i64))]
    positional_count: Option<u32>,
}

pub fn run_pin(args: impl IntoIterator<Item = String>) -> ExitCode {
    dispatch::<PinCli, _>("shape pin", args, execute_pin)
}

pub fn run_headers(args: impl IntoIterator<Item = String>) -> ExitCode {
    dispatch::<HeadersCli, _>("shape headers", args, |cli| execute_headers(cli.command))
}

pub fn run_payload(args: impl IntoIterator<Item = String>) -> ExitCode {
    dispatch::<PayloadCli, _>("shape payload", args, |cli| execute_payload(cli.command))
}

pub fn run_pack(args: impl IntoIterator<Item = String>) -> ExitCode {
    dispatch::<PackCli, _>("shape pack", args, |cli| execute_pack(cli.files))
}

pub fn run_unpack(args: impl IntoIterator<Item = String>) -> ExitCode {
    dispatch::<UnpackCli, _>("shape unpack", args, |cli| execute_unpack(cli.files))
}

pub fn run_encode(args: impl IntoIterator<Item = String>) -> ExitCode {
    dispatch::<EncodeCli, _>("shape encode", args, |cli| execute_encode(cli.io))
}

pub fn run_decode(args: impl IntoIterator<Item = String>) -> ExitCode {
    dispatch::<DecodeCli, _>("shape decode", args, |cli| {
        execute_decode(cli.io, cli.positional_count)
    })
}

const GEN_USAGE: &str =
    "usage: shape <pin|headers|payload|pack|unpack|encode|decode> [options]\n\n\
`shape gen <command>` is a legacy alias for those top-level commands.\n\
Run `shape <command> --help` for options.";

/// `shape gen ...` legacy spelling: forward to the top-level command.
pub fn run(args: impl IntoIterator<Item = String>) -> ExitCode {
    let mut args: Vec<String> = args.into_iter().collect();
    if args.is_empty() {
        eprintln!("{GEN_USAGE}");
        return ExitCode::from(2);
    }
    let command = args.remove(0);
    let rest = args;
    match command.as_str() {
        "pin" => run_pin(rest),
        "headers" => run_headers(rest),
        "payload" => run_payload(rest),
        "pack" => run_pack(rest),
        "unpack" => run_unpack(rest),
        "encode" => run_encode(rest),
        "decode" => run_decode(rest),
        "-h" | "--help" | "help" => {
            println!("{GEN_USAGE}");
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("shape gen: unrecognized subcommand {other}\n{GEN_USAGE}");
            ExitCode::from(2)
        }
    }
}

fn read_input(files: &FileArgs) -> io::Result<Vec<u8>> {
    if is_stdio(&files.input) {
        read_bounded(io::stdin().lock(), 128 * 1024 * 1024)
    } else {
        read_file(&files.input, 128 * 1024 * 1024)
    }
}

fn write_output(files: &FileArgs, output: &[u8]) -> io::Result<()> {
    if is_stdio(&files.out) {
        crate::cli::write_stdout(output).map(|_| ())
    } else {
        write_atomic(&files.out, output, files.force)
    }
}

fn execute_pack(files: FileArgs) -> Result<(), Box<dyn std::error::Error>> {
    check_paths(&files, None)?;
    let input = read_input(&files)?;
    let request: PackRequest =
        parse_typed_json(&input).map_err(|error| invalid(format!("pack JSON: {error}")))?;
    let mut output = serde_json::to_vec(&UnpackRequest {
        bytes: request.pack()?,
        options: request.options,
    })?;
    output.push(b'\n');
    write_output(&files, &output)?;
    Ok(())
}

fn execute_unpack(files: FileArgs) -> Result<(), Box<dyn std::error::Error>> {
    check_paths(&files, None)?;
    let input = read_input(&files)?;
    let request: UnpackRequest =
        serde_json::from_slice(&input).map_err(|error| invalid(format!("unpack JSON: {error}")))?;
    let value = unpack_canonical(&request.bytes, &request.options).map_err(|error| {
        invalid(format!("{error}; for original byte provenance use the raw/library API generator::superpack::unpack"))
    })?;
    let mut output = serde_json::to_vec(&PackRequest {
        value,
        options: request.options,
    })?;
    output.push(b'\n');
    write_output(&files, &output)?;
    Ok(())
}

fn execute_encode(io: IoArgs) -> Result<(), Box<dyn std::error::Error>> {
    check_paths(&io.files, io.settings_path())?;
    let settings = load_settings(&io)?;
    let codec = settings.generator()?;
    let input = read_input(&io.files)?;
    let payload: Payload = serde_json::from_slice(&input)
        .map_err(|error| invalid(format!("payload JSON: {error}")))?;
    let mut output = serde_json::to_vec(&codec.encode_headers(&payload)?)?;
    output.push(b'\n');
    write_output(&io.files, &output)?;
    Ok(())
}

fn execute_decode(
    io: IoArgs,
    positional_count: Option<u32>,
) -> Result<(), Box<dyn std::error::Error>> {
    check_paths(&io.files, io.settings_path())?;
    let settings = load_settings(&io)?;
    let codec = settings.generator()?;
    let positional_count = settings.facts().resolve_count(positional_count)?;
    let input = read_input(&io.files)?;
    let headers: Headers =
        serde_json::from_slice(&input).map_err(|error| invalid(format!("header JSON: {error}")))?;
    let mut payload = codec.decode_headers(&headers)?;
    if let Some(count) = positional_count {
        payload = payload.parse_container(count)?;
    }
    let mut output = serde_json::to_vec(&payload)?;
    output.push(b'\n');
    write_output(&io.files, &output)?;
    Ok(())
}

fn execute_pin(cli: PinCli) -> Result<(), Box<dyn std::error::Error>> {
    let PinCli {
        files,
        init,
        session,
    } = cli;
    // The traced program is input, not a settings file: never let --out alias it.
    if !is_stdio(&files.out) && !is_stdio(&files.input) {
        reject_alias(
            &planned_output(&files.out)?,
            &fs::canonicalize(&files.input)?,
            "input and output paths alias",
        )?;
    }
    check_paths(&files, None)?;
    let source = if is_stdio(&files.input) {
        read_bounded(io::stdin().lock(), 16 * 1024 * 1024)?
    } else {
        read_file(&files.input, 16 * 1024 * 1024)?
    };
    let session: Session = match (init.as_deref(), session.as_deref()) {
        (Some(path), None) => {
            let initializer = read_file(path, 128 * 1024)?;
            session_from_initializer(std::str::from_utf8(&initializer)?)?
        }
        (None, Some(path)) => serde_json::from_slice(&read_file(path, 128 * 1024)?)
            .map_err(|e| invalid(format!("session JSON: {e}")))?,
        _ => {
            return Err(invalid("pin needs exactly one of --init or --session").into());
        }
    };
    let pinned = pin(std::str::from_utf8(&source)?, &session)?;
    for evidence in &pinned.evidence {
        eprintln!(
            "pinned {} at bytes {}..{} ({})",
            evidence.role, evidence.start, evidence.end, evidence.sha256
        );
    }
    eprintln!(
        "program {} bytes sha256 {}",
        pinned.layout.source.bytes, pinned.layout.source.sha256
    );
    let output = pinned.to_json_pretty();
    if is_stdio(&files.out) {
        crate::cli::write_stdout(&output)?;
    } else {
        write_atomic(&files.out, &output, files.force)?;
    }
    Ok(())
}

fn execute_payload(command: PayloadCommand) -> Result<(), Box<dyn std::error::Error>> {
    let (io, keysets_path) = match &command {
        PayloadCommand::Encode { io, .. } => (io, None),
        PayloadCommand::Decode { io, keysets, .. } => (io, keysets.as_deref()),
    };
    let check = || -> io::Result<()> {
        check_paths(&io.files, io.settings_path())?;
        if let Some(keysets) = keysets_path {
            // Apply the same stdin/alias/no-clobber policy to the additional
            // settings file, including hard links and symlinked parents.
            check_paths(&io.files, Some(keysets))?;
            reject_alias(
                &fs::canonicalize(keysets)?,
                &fs::canonicalize(io.settings_path().expect("clap settings"))?,
                "keysets and settings paths alias",
            )?;
        }
        Ok(())
    };
    check()?;
    let settings = load_settings(io)?;
    let codec = PayloadCodec::new(settings.payload_profile())?;
    let input = if is_stdio(&io.files.input) {
        read_bounded(io::stdin().lock(), 128 * 1024 * 1024)?
    } else {
        read_file(&io.files.input, 128 * 1024 * 1024)?
    };
    let mut output = match &command {
        PayloadCommand::Encode { seeds, .. } => {
            let report = if let Some(seeds) = seed_pair(seeds)? {
                ChannelReport::from_json(&input)
                    .map_err(|e| invalid(format!("four-channel report JSON: {e}")))?
                    .report(seeds, encode_keysets(&settings))
            } else {
                parse_typed_json(&input).map_err(|e| invalid(format!("report JSON: {e}")))?
            };
            codec.encode_report(&report)?.into_bytes()
        }
        PayloadCommand::Decode {
            positional_count,
            keysets,
            channels,
            ..
        } => {
            let (count, keysets) = decode_facts(&settings, *positional_count, keysets.as_deref())?;
            let text = std::str::from_utf8(&input)?;
            // Exactly one optional LF or CRLF, as emitted by this CLI. Spaces,
            // embedded line breaks, lone CR and repeated final lines are errors.
            let text = text
                .strip_suffix("\r\n")
                .or_else(|| text.strip_suffix('\n'))
                .unwrap_or(text);
            let report = codec.decode_report(text, count, keysets)?;
            if *channels {
                serde_json::to_vec(&report.channels())?
            } else {
                serde_json::to_vec(&report)?
            }
        }
    };
    output.push(b'\n');
    if is_stdio(&io.files.out) {
        crate::cli::write_stdout(&output)?;
    } else {
        check()?;
        write_atomic(&io.files.out, &output, io.files.force)?;
    }
    Ok(())
}

fn execute_headers(command: HeadersCommand) -> Result<(), Box<dyn std::error::Error>> {
    let (io, keysets_path) = match &command {
        HeadersCommand::Encode { io, .. } => (io, None),
        HeadersCommand::Decode { io, keysets, .. } => (io, keysets.as_deref()),
    };
    let check = || -> io::Result<()> {
        check_paths(&io.files, io.settings_path())?;
        if let Some(keysets) = keysets_path {
            check_paths(&io.files, Some(keysets))?;
            reject_alias(
                &fs::canonicalize(keysets)?,
                &fs::canonicalize(io.settings_path().expect("clap settings"))?,
                "keysets and settings paths alias",
            )?;
        }
        Ok(())
    };
    check()?;
    let settings = load_settings(io)?;
    let codec = settings.generator()?;
    let input = if is_stdio(&io.files.input) {
        read_bounded(io::stdin().lock(), 128 * 1024 * 1024)?
    } else {
        read_file(&io.files.input, 128 * 1024 * 1024)?
    };
    let mut output = match &command {
        HeadersCommand::Encode {
            input_format,
            output_format,
            seeds,
            ..
        } => {
            let headers = match input_format {
                HeaderEncodeFormat::Report => {
                    let document = if let Some(seeds) = seed_pair(seeds)? {
                        let channels = ChannelReport::from_json(&input)
                            .map_err(|e| invalid(format!("four-channel report JSON: {e}")))?;
                        HeaderReport::from_channels(
                            &channels,
                            seeds,
                            encode_keysets(&settings),
                            None,
                        )
                    } else {
                        HeaderReport::from_json(&input)?
                    };
                    codec.encode_report_headers(&document)?
                }
                HeaderEncodeFormat::Payload => {
                    let text = std::str::from_utf8(&input)?;
                    let text = text
                        .strip_suffix("\r\n")
                        .or_else(|| text.strip_suffix('\n'))
                        .unwrap_or(text);
                    codec.encode_finalized_headers(text, None)?
                }
            };
            match output_format {
                HeaderOutputFormat::Json => serde_json::to_vec(&headers)?,
                HeaderOutputFormat::Text => {
                    headers.to_text().trim_end_matches('\n').as_bytes().to_vec()
                }
            }
        }
        HeadersCommand::Decode {
            input_format,
            positional_count,
            keysets,
            channels,
            ..
        } => {
            let json = match input_format {
                HeaderInputFormat::Json => true,
                HeaderInputFormat::Text => false,
                HeaderInputFormat::Auto => {
                    input.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{')
                }
            };
            let headers: Headers = if json {
                serde_json::from_slice(&input).map_err(|e| invalid(format!("header JSON: {e}")))?
            } else {
                Headers::from_text(std::str::from_utf8(&input)?)?
            };
            let (count, keysets) = decode_facts(&settings, *positional_count, keysets.as_deref())?;
            let report = codec.decode_report_headers(&headers, count, keysets)?;
            if *channels {
                serde_json::to_vec(&report.channels())?
            } else {
                serde_json::to_vec(&report)?
            }
        }
    };
    output.push(b'\n');
    if is_stdio(&io.files.out) {
        crate::cli::write_stdout(&output)?;
    } else {
        check()?;
        write_atomic(&io.files.out, &output, io.files.force)?;
    }
    Ok(())
}

/// Parse `--seeds W1,W2` into a pair, refusing any other arity.
fn seed_pair(seeds: &Option<Vec<u32>>) -> Result<Option<[u32; 2]>, Box<dyn std::error::Error>> {
    let Some(seeds) = seeds else {
        return Ok(None);
    };
    match seeds.as_slice() {
        [first, second] => Ok(Some([*first, *second])),
        _ => Err(invalid("--seeds needs exactly two u32 words: W1,W2").into()),
    }
}

/// Four-channel encode keeps the legacy empty table when no default is supplied.
/// Full reports already carry their own keysets and do not call this helper.
fn encode_keysets(settings: &Settings) -> Vec<Vec<String>> {
    settings.facts().keysets.unwrap_or_default().to_vec()
}

fn decode_facts(
    settings: &Settings,
    count: Option<u32>,
    keysets: Option<&Path>,
) -> Result<(usize, Vec<Vec<String>>), Box<dyn std::error::Error>> {
    let facts = settings.facts();
    let keysets = match keysets {
        Some(path) => serde_json::from_slice(&read_file(path, 64 * 1024)?)
            .map_err(|e| invalid(format!("keysets JSON: {e}")))?,
        None => facts.keysets.ok_or_else(|| {
            invalid("--keysets is required when layout/profile keysets metadata is absent (use [] for none)")
        })?.to_vec(),
    };
    let count = facts.resolve_count(count)?.ok_or_else(|| {
        invalid("--positional-count is required when layout/profile positional_count metadata is absent")
    })?;
    Ok((count, keysets))
}

/// Canonical path a not-yet-existing `--out` will occupy.
fn planned_output(path: &Path) -> io::Result<PathBuf> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    Ok(fs::canonicalize(parent)?.join(
        path.file_name()
            .ok_or_else(|| invalid("output needs a file name"))?,
    ))
}

fn is_stdio(path: &Path) -> bool {
    path == Path::new("-")
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn read_bounded(reader: impl Read, limit: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(invalid(format!("JSON input exceeds {limit} bytes")));
    }
    Ok(bytes)
}

fn read_file(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    read_bounded(File::open(path)?, limit)
        .map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", path.display())))
}

fn check_paths(args: &FileArgs, profile: Option<&Path>) -> io::Result<()> {
    let profile = profile
        .map(|path| {
            if is_stdio(path) {
                return Err(invalid("--profile requires a file, not stdin"));
            }
            fs::canonicalize(path)
        })
        .transpose()?;
    let input = if is_stdio(&args.input) {
        None
    } else {
        Some(fs::canonicalize(&args.input)?)
    };
    if let (Some(input), Some(profile)) = (&input, &profile) {
        reject_alias(input, profile, "input and profile paths alias")?;
    }
    if is_stdio(&args.out) {
        return Ok(());
    }
    // Resolve the parent even for a not-yet-existing output. This catches paths
    // through symlinked directories and lexical ./../ aliases.
    let parent = args
        .out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let output = fs::canonicalize(parent)?.join(
        args.out
            .file_name()
            .ok_or_else(|| invalid("output needs a file name"))?,
    );
    if let Some(profile) = &profile {
        reject_alias(&output, profile, "output and profile paths alias")?;
    }
    if let Some(input) = &input {
        reject_alias(&output, input, "output and input paths alias")?;
    }
    match fs::symlink_metadata(&output) {
        Ok(metadata) => {
            if !metadata.is_file() {
                return Err(invalid(
                    "output must be a regular file; symlinks and directories are refused",
                ));
            }
            if !args.force {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "output already exists; use --force",
                ));
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    Ok(())
}

fn reject_alias(left: &Path, right: &Path, message: &str) -> io::Result<()> {
    if left == right {
        return Err(invalid(message));
    }
    // Either path may be a not-yet-created output; only compare what resolves.
    if let (Ok(left), Ok(right)) = (fs::canonicalize(left), fs::canonicalize(right)) {
        if left == right {
            return Err(invalid(message));
        }
    }
    // Canonical paths alone do not detect hard links.
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let (Ok(left), Ok(right)) = (fs::metadata(left), fs::metadata(right)) {
            if left.dev() == right.dev() && left.ino() == right.ino() {
                return Err(invalid(message));
            }
        }
    }
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8], force: bool) -> io::Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let (temporary, mut file) = (0..128)
        .find_map(|_| {
            let temporary = parent.join(format!(
                ".shape-gen-{}-{}.tmp",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
            {
                Ok(file) => Some(Ok((temporary, file))),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => None,
                Err(error) => Some(Err(error)),
            }
        })
        .unwrap_or_else(|| {
            Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "cannot reserve temporary output",
            ))
        })?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        if force {
            fs::rename(&temporary, path)
        } else {
            fs::hard_link(&temporary, path)
        }
    })();
    let _ = fs::remove_file(&temporary);
    result
}
