//! Capture decoding orchestration; all signal and payload logic lives in the library.
use clap::{ArgGroup, Parser, ValueEnum};
use shape::signals::{
    decode::{CaptureHeaders, CompileOptions, DecodeArtifact, DecodeOptions, ValidatedArtifact},
    payload::extract::Session,
    wire::FloatByteOrder,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
    time::Instant,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum InputKind {
    Raw,
    Program,
}
#[derive(Clone, Copy, ValueEnum)]
enum ByteOrder {
    Little,
    Big,
}
#[derive(Parser)]
#[command(name="shape decode", about="Decode captured headers into named signal fields using a raw seed or a reusable artifact", group(ArgGroup::new("source").required(true).args(["seed", "artifact"])))]
struct Args {
    /// Raw local JavaScript seed (traced source with --direct or --input-kind program).
    #[arg(long)]
    seed: Option<PathBuf>,
    /// Load compiled recipes; no seed analysis or source files required.
    #[arg(long)]
    artifact: Option<PathBuf>,
    /// Opt into source-format decoding without lifecycle certification; saved in the artifact.
    #[arg(long, requires = "seed", conflicts_with_all = ["artifact", "schema_only"])]
    direct: bool,
    /// JSON {"siteId":"example","headers":{"a":"...","b":"..."}}.
    #[arg(long, required_unless_present = "schema_only")]
    headers: Option<PathBuf>,
    /// Parsed async Session JSON. Required with --seed; replaces artifact defaults together.
    #[arg(long, required_unless_present_any = ["artifact", "schema_only"])]
    session: Option<PathBuf>,
    /// Export only static schema from a traced program; no session or capture needed.
    #[arg(long, requires_all = ["seed", "schema_out"], conflicts_with_all = ["artifact", "headers", "session", "save_artifact"])]
    schema_only: bool,
    /// Save a reusable artifact after a successful decode.
    #[arg(long, requires = "seed")]
    save_artifact: Option<PathBuf>,
    /// Export shape-seed-schema/1: source encoding facts, routes and logical contracts.
    #[arg(long)]
    schema_out: Option<PathBuf>,
    /// Seed format: raw by default, program with --direct. Use raw to trace a direct seed.
    #[arg(long, value_enum, requires = "seed")]
    input_kind: Option<InputKind>,
    /// Browser native float byte order; saved in the artifact. Defaults to little with --direct.
    #[arg(long, value_enum)]
    float_byte_order: Option<ByteOrder>,
    /// Named payload JSON output, or - for stdout.
    #[arg(long, default_value = "-")]
    out: PathBuf,
    /// Replace existing output files; input paths are always protected.
    #[arg(long)]
    force: bool,
    /// Re-encode every signal and the header bundle and require identical bytes.
    /// Always on with --save-artifact or --schema-out.
    #[arg(long)]
    verify: bool,
    /// Print input loading, artifact loading/compilation, decoding and output timings to stderr.
    #[arg(long)]
    timings: bool,
}

impl Args {
    fn effective_input_kind(&self) -> InputKind {
        self.input_kind.unwrap_or(if self.direct {
            InputKind::Program
        } else {
            InputKind::Raw
        })
    }
}

pub fn run(args: Vec<String>) -> ExitCode {
    // Explicit-profile byte decoding remains available on its established surface.
    if args
        .iter()
        .any(|a| a == "--profile" || a.starts_with("--profile="))
    {
        return super::payload::run_decode(args.into_iter());
    }
    let args = match Args::try_parse_from(std::iter::once("shape decode".to_string()).chain(args)) {
        Ok(args) => args,
        Err(error) => {
            let code = if error.use_stderr() { 2 } else { 0 };
            let _ = error.print();
            return ExitCode::from(code);
        }
    };
    match execute(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("shape decode: {error}");
            ExitCode::from(1)
        }
    }
}
fn same(a: &Path, b: &Path) -> bool {
    a == b || matches!((fs::canonicalize(a),fs::canonicalize(b)), (Ok(x),Ok(y)) if x==y)
}
fn execute(args: &Args) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let inputs: Vec<&Path> = [
        args.headers.as_deref(),
        args.seed.as_deref(),
        args.artifact.as_deref(),
        args.session.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect();
    let outputs: Vec<&Path> = [
        Some(args.out.as_path()),
        args.save_artifact.as_deref(),
        args.schema_out.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect();
    for output in &outputs {
        if **output == *Path::new("-") {
            continue;
        }
        if inputs.iter().any(|input| same(input, output)) {
            return Err("output path aliases an input".into());
        }
        if output.exists() && !args.force {
            return Err("output already exists (use --force)".into());
        }
        if !output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .is_dir()
        {
            return Err("output parent directory does not exist".into());
        }
    }
    if let Some(save) = &args.save_artifact {
        if save == Path::new("-") || same(save, &args.out) {
            return Err("artifact output must be a distinct file".into());
        }
    }
    if let Some(schema) = &args.schema_out {
        if schema == Path::new("-")
            || same(schema, &args.out)
            || args
                .save_artifact
                .as_ref()
                .is_some_and(|save| same(schema, save))
        {
            return Err("schema output must be a distinct file".into());
        }
    }
    if args.schema_only {
        if !matches!(args.input_kind, Some(InputKind::Program)) {
            return Err(
                "--schema-only requires --input-kind program (a traced local source)".into(),
            );
        }
        let source = fs::read_to_string(args.seed.as_ref().ok_or("seed required")?)?;
        let schema = shape::signals::decode::CompactSeedSchema::from_source(&source)?;
        let mut bytes = serde_json::to_vec_pretty(&schema)?;
        bytes.push(b'\n');
        super::atomic_write(
            args.schema_out.as_ref().ok_or("schema output required")?,
            &bytes,
            args.force,
        )?;
        return Ok(());
    }
    let capture: CaptureHeaders = serde_json::from_slice(&fs::read(
        args.headers
            .as_ref()
            .ok_or("--headers required for decoding")?,
    )?)?;
    capture.validate()?;
    let session: Option<Session> = args
        .session
        .as_ref()
        .map(|p| -> Result<_, Box<dyn std::error::Error>> {
            Ok(serde_json::from_slice(&fs::read(p)?)?)
        })
        .transpose()?;
    let order = args.float_byte_order.map(|o| match o {
        ByteOrder::Little => FloatByteOrder::Little,
        ByteOrder::Big => FloatByteOrder::Big,
    });
    let inputs_done = Instant::now();
    let mut artifact = if let Some(path) = &args.artifact {
        DecodeArtifact::from_json(&fs::read(path)?)?
    } else {
        let seed = args.seed.as_ref().ok_or("seed required")?;
        let source = match args.effective_input_kind() {
            InputKind::Program => fs::read_to_string(seed)?,
            InputKind::Raw => {
                use shape::trace::program::{link_program, LinkOptions};
                let linked = link_program(&LinkOptions {
                    seed: seed.clone(),
                    report_dir: None,
                    steps: 1_000_000,
                    dump_stages: None,
                    thread_cleanup: true,
                    no_cleanup: false,
                    check_reuse: false,
                    overrides: Default::default(),
                    jobs: None,
                    quiet: true,
                })?;
                if !linked.stopped.is_empty() {
                    return Err(
                        "seed tracing stopped; inspect with shape trace program before decoding"
                            .into(),
                    );
                }
                linked.code
            }
        };
        DecodeArtifact::compile_with_options(
            &source,
            session.as_ref().ok_or("session required")?,
            capture.site_id.clone(),
            order.unwrap_or(if args.direct {
                FloatByteOrder::Little
            } else {
                FloatByteOrder::default()
            }),
            CompileOptions {
                direct: args.direct,
            },
        )?
    };
    if let Some(order) = order {
        artifact.float_byte_order = order;
    }
    let artifact_done = Instant::now();
    // Published artifacts and schemas always come from a verified roundtrip.
    let verify = args.verify || args.save_artifact.is_some() || args.schema_out.is_some();
    let validated = ValidatedArtifact::new(artifact)?;
    let options = DecodeOptions {
        verify,
        metadata: true,
    };
    let decoded = validated.decode(&capture, session.as_ref(), options)?;
    let artifact = validated.into_inner();
    let decode_done = Instant::now();
    let mut output = serde_json::to_vec_pretty(&decoded)?;
    output.push(b'\n');
    let schema_output = args
        .schema_out
        .as_ref()
        .map(|_| -> Result<_, Box<dyn std::error::Error>> {
            let mut bytes = serde_json::to_vec_pretty(&artifact.compact_schema()?)?;
            bytes.push(b'\n');
            Ok(bytes)
        })
        .transpose()?;
    let serialization_done = Instant::now();
    // Publish only after every present signal and the full header bundle round-trips.
    if let Some(path) = &args.save_artifact {
        // Artifacts are machine inputs: avoid reparsing megabytes of indentation on reuse.
        let mut bytes = serde_json::to_vec(&artifact)?;
        bytes.push(b'\n');
        super::atomic_write(path, &bytes, args.force)?;
    }
    if let (Some(path), Some(bytes)) = (&args.schema_out, schema_output) {
        super::atomic_write(path, &bytes, args.force)?;
    }
    if args.out == Path::new("-") {
        super::write_stdout(&output)?;
    } else {
        super::atomic_write(&args.out, &output, args.force)?;
    }
    if args.timings {
        let ms = |a: Instant, b: Instant| b.duration_since(a).as_secs_f64() * 1000.;
        eprintln!("shape decode timings (ms): inputs={:.3} artifact={:.3} decode={:.3} serialize={:.3} write={:.3} total={:.3}",
            ms(start, inputs_done), ms(inputs_done, artifact_done), ms(artifact_done, decode_done),
            ms(decode_done, serialization_done), ms(serialization_done, Instant::now()), ms(start, Instant::now()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed_args(extra: &[&str]) -> Result<Args, clap::Error> {
        Args::try_parse_from(
            [
                "shape decode",
                "--seed",
                "program.js",
                "--session",
                "session.json",
                "--headers",
                "headers.json",
            ]
            .into_iter()
            .chain(extra.iter().copied()),
        )
    }

    #[test]
    fn direct_is_opt_in_and_accepts_traced_seed_without_input_kind() {
        let strict = seed_args(&[]).unwrap();
        assert!(!strict.direct);
        assert_eq!(strict.effective_input_kind(), InputKind::Raw);
        let direct = seed_args(&["--direct", "--verify"]).unwrap();
        assert!(direct.direct && direct.verify);
        assert_eq!(direct.effective_input_kind(), InputKind::Program);
        for direct in [false, true] {
            for (kind, expected) in [("raw", InputKind::Raw), ("program", InputKind::Program)] {
                let mut flags = vec!["--input-kind", kind];
                if direct {
                    flags.push("--direct");
                }
                assert_eq!(seed_args(&flags).unwrap().effective_input_kind(), expected);
            }
        }
    }

    #[test]
    fn direct_requires_seed_and_conflicts_with_loaded_artifact_or_schema_only() {
        assert!(seed_args(&["--direct", "--artifact", "artifact.json"]).is_err());
        assert!(Args::try_parse_from([
            "shape decode",
            "--artifact",
            "artifact.json",
            "--headers",
            "headers.json",
            "--direct"
        ])
        .is_err());
        assert!(Args::try_parse_from([
            "shape decode",
            "--seed",
            "program.js",
            "--schema-only",
            "--schema-out",
            "schema.json",
            "--input-kind",
            "program",
            "--direct"
        ])
        .is_err());
        let loaded = Args::try_parse_from([
            "shape decode",
            "--artifact",
            "artifact.json",
            "--headers",
            "headers.json",
            "--verify",
        ])
        .unwrap();
        assert!(!loaded.direct && loaded.verify);
    }

    #[test]
    fn help_describes_direct_boundary() {
        use clap::CommandFactory;
        let help = Args::command().term_width(0).render_long_help().to_string();
        assert!(help.contains("--direct"));
        assert!(help.contains("without lifecycle certification"));
        assert!(help.contains("saved in the artifact"));
        assert!(help.contains("program with --direct"));
        assert!(help.contains("require identical bytes"));
    }
}
