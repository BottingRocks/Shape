//! `shape signals`: the per-seed signal schema and the generator-input views.
//!
//! `shape signals <program.js>` prints the signal schema of one recovered
//! `program.js`; `show`, `inputs` and `contracts` print the structural
//! generator-input properties, the composed inputs and the typed signal
//! contracts. No command executes recovered JavaScript.

use std::path::Path;
use std::process::ExitCode;

use shape::signals::extract::scanner::extract_program_properties;

use super::{parse_show, read_source, usage_error};

const SIGNALS_USAGE: &str = "\
usage: shape signals <program.js> [--json] [--layouts-only] [--no-finder-roles]
       shape signals <show|inputs|contracts> <program.js> ... [options]

  <program.js> [--json] [--layouts-only] [--no-finder-roles]
      print the per-seed signal schema (docs/signals/schema.md): every
      slot a signal recognizer identifies, keyed by canonical name, with its
      routing, mask salt, field order and typed fields named from the
      approved field cards. Unidentified, ambiguous and refused slots (a
      field without a card name is never emitted) are listed under
      `diagnostics`. --layouts-only adds each unidentified/refused slot's
      projected layout with positional field keys. The output is JSON;
      --json is accepted for symmetry.

  show <program.js> [--json]
      print the recovered generator-input properties: descriptor registry,
      wave/direct routing, per-signal ordered emission layouts, session
      contract and external runtime inputs. --json prints the typed report.

  inputs <program.js> [--json]
      compose the source-derived encoding, collector wiring, signal layouts,
      and glyph/DOM input families. --json prints the complete typed report.

  contracts <program.js> [--json] [--signal <descriptor>] [--pair <program.js>]
      print the generator-ready signal contracts: per descriptor its
      producer, ordered typed grammar per emission (raw prefix, NUMBER,
      STRING, logical BYTE with its mask, SEQUENCE/CHOICE/REPEAT with branch
      kind and loop direction, opaque helper boundaries, unresolved nodes),
      error/complete sites, the provenance class of every written value,
      write accounting and a complete/partial/unsupported status with
      reasons. Spans are UTF-8 byte offsets of the input. --signal limits the
      output to one descriptor of this capture. --pair takes a second trace
      of the same raw build and marks a literal as a trace-folded observation
      where that trace computes the same field from a host source.

No command executes the recovered JavaScript.";

const SHOW_USAGE: &str = "usage: shape signals show <program.js> [--json]";
const INPUTS_USAGE: &str = "usage: shape signals inputs <program.js> [--json]";
const CONTRACTS_USAGE: &str =
    "usage: shape signals contracts <program.js> [--json] [--signal <descriptor>] [--pair <program.js>]";

/// Run `shape signals` from the root command's remaining arguments.
pub fn run(mut args: impl Iterator<Item = String>) -> ExitCode {
    let Some(first) = args.next() else {
        eprintln!("shape signals: missing <program.js>\n{SIGNALS_USAGE}");
        return ExitCode::from(2);
    };
    match first.as_str() {
        "-h" | "--help" => {
            println!("{SIGNALS_USAGE}");
            ExitCode::SUCCESS
        }
        "show" => run_show(args.collect()),
        "inputs" => run_inputs(args.collect()),
        "contracts" => run_contracts(args.collect()),
        // Hidden: regenerate docs/signals/*.md from the cards.
        "docs" => run_docs(args.collect()),
        _ => run_signals(std::iter::once(first).chain(args).collect()),
    }
}

/// `shape signals docs <out-dir>`: write `<signal>.md` for every signal and
/// the index `README.md`, rendered from the cards (the `.js` samples and
/// `schema.md` are left alone).
fn run_docs(args: Vec<String>) -> ExitCode {
    let [dir] = args.as_slice() else {
        eprintln!("usage: shape signals docs <out-dir>");
        return ExitCode::from(2);
    };
    let dir = Path::new(dir);
    if let Err(error) = std::fs::create_dir_all(dir) {
        eprintln!("shape signals docs: {}: {error}", dir.display());
        return ExitCode::from(1);
    }
    let pages = shape::signals::docs::render_pages(|stem| dir.join(format!("{stem}.js")).exists());
    for (name, text) in &pages {
        if let Err(error) = std::fs::write(dir.join(name), text) {
            eprintln!("shape signals docs: {name}: {error}");
            return ExitCode::from(1);
        }
    }
    eprintln!("wrote {} pages to {}", pages.len(), dir.display());
    ExitCode::SUCCESS
}

fn run_signals(args: Vec<String>) -> ExitCode {
    use shape::signals::analysis::{
        analyze_program_signals, SignalFinderOptions,
    };
    let mut input = None;
    let mut options = SignalFinderOptions::default();
    for argument in args {
        match argument.as_str() {
            "-h" | "--help" => {
                println!("{SIGNALS_USAGE}");
                return ExitCode::SUCCESS;
            }
            "--json" => {}
            "--layouts-only" => options.layouts_only = true,
            "--no-finder-roles" => options.finder_roles = false,
            other if other.starts_with('-') => {
                return usage_error(
                    "shape signals",
                    &format!("unknown option {other}"),
                    SIGNALS_USAGE,
                )
            }
            other => {
                if input.replace(other.to_string()).is_some() {
                    return usage_error(
                        "shape signals",
                        "more than one input file",
                        SIGNALS_USAGE,
                    );
                }
            }
        }
    }
    let Some(input) = input else {
        return usage_error("shape signals", "missing <program.js>", SIGNALS_USAGE);
    };
    let source = match read_source(Path::new(&input)) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("shape signals: {error}");
            return ExitCode::from(1);
        }
    };
    let mut analysis = match analyze_program_signals(&source.text, &options) {
        Ok(analysis) => analysis,
        Err(error) => {
            eprintln!("shape signals: {error}");
            return ExitCode::from(1);
        }
    };
    // The seed ID is the program file stem (`<seed>.js`), from the path.
    analysis.report.seed = Path::new(&input)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_string);
    let rendered = match serde_json::to_string_pretty(&analysis.report) {
        Ok(json) => json,
        Err(error) => {
            eprintln!("shape signals: cannot serialize report: {error}");
            return ExitCode::from(1);
        }
    };
    match super::write_stdout_line(&rendered) {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("shape signals: failed to write stdout: {error}");
            ExitCode::from(1)
        }
    }
}

fn run_show(args: Vec<String>) -> ExitCode {
    let parsed = match parse_show(args) {
        Ok(Some(parsed)) => parsed,
        Ok(None) => {
            println!("{SHOW_USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(error) => return usage_error("shape signals show", &error, SHOW_USAGE),
    };
    let source = match read_source(Path::new(&parsed.input)) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("shape signals show: {error}");
            return ExitCode::from(1);
        }
    };
    let properties = match extract_program_properties(&source.text) {
        Ok(properties) => properties,
        Err(error) => {
            eprintln!("shape signals show: {error}");
            return ExitCode::from(1);
        }
    };
    // The report is the fact source; the human view is a summary of it.
    let rendered = if parsed.json {
        match serde_json::to_string_pretty(&properties) {
            Ok(json) => json,
            Err(error) => {
                eprintln!("shape signals show: cannot serialize report: {error}");
                return ExitCode::from(1);
            }
        }
    } else {
        human_summary(&properties)
    };
    match super::write_stdout_line(&rendered) {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("shape signals show: failed to write stdout: {error}");
            ExitCode::from(1)
        }
    }
}

fn run_inputs(args: Vec<String>) -> ExitCode {
    let parsed = match parse_show(args) {
        Ok(Some(parsed)) => parsed,
        Ok(None) => {
            println!("{INPUTS_USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(error) => return usage_error("shape signals inputs", &error, INPUTS_USAGE),
    };
    let source = match read_source(Path::new(&parsed.input)) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("shape signals inputs: {error}");
            return ExitCode::from(1);
        }
    };
    let report = match shape::naming::inputs_context::extract_program_inputs_with_context(&source.text) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("shape signals inputs: {error}");
            return ExitCode::from(1);
        }
    };
    let rendered = if parsed.json {
        match serde_json::to_string_pretty(&report) {
            Ok(json) => json,
            Err(error) => {
                eprintln!("shape signals inputs: cannot serialize report: {error}");
                return ExitCode::from(1);
            }
        }
    } else {
        inputs_summary(&report)
    };
    match super::write_stdout_line(&rendered) {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("shape signals inputs: failed to write stdout: {error}");
            ExitCode::from(1)
        }
    }
}

struct ContractsArgs {
    input: String,
    json: bool,
    signal: Option<u32>,
    pair: Option<String>,
}

fn parse_contracts(args: Vec<String>) -> Result<Option<ContractsArgs>, String> {
    let mut input = None;
    let mut json = false;
    let mut signal = None;
    let mut pair = None;
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "-h" | "--help" => return Ok(None),
            "--json" => json = true,
            "--signal" => {
                let value = args.next().ok_or("--signal needs a descriptor index")?;
                signal = Some(
                    value
                        .parse::<u32>()
                        .map_err(|_| format!("--signal: not an index: {value}"))?,
                );
            }
            "--pair" => pair = Some(args.next().ok_or("--pair needs a program.js")?),
            other if other.starts_with('-') => return Err(format!("unknown option {other}")),
            other => {
                if input.replace(other.to_string()).is_some() {
                    return Err("more than one input file".to_string());
                }
            }
        }
    }
    let input = input.ok_or_else(|| "missing <program.js>".to_string())?;
    Ok(Some(ContractsArgs {
        input,
        json,
        signal,
        pair,
    }))
}

fn run_contracts(args: Vec<String>) -> ExitCode {
    use shape::signals::extract::contracts;
    let parsed = match parse_contracts(args) {
        Ok(Some(parsed)) => parsed,
        Ok(None) => {
            println!("{CONTRACTS_USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(error) => return usage_error("shape signals contracts", &error, CONTRACTS_USAGE),
    };
    let extract = |path: &str| -> Result<(String, contracts::SignalContractReport), String> {
        let source = read_source(Path::new(path)).map_err(|error| error.to_string())?;
        let report =
            contracts::extract_signal_contracts(&source.text).map_err(|error| error.to_string())?;
        Ok((source.text, report))
    };
    let mut report = match extract(&parsed.input) {
        Ok((_, report)) => report,
        Err(error) => {
            eprintln!("shape signals contracts: {error}");
            return ExitCode::from(1);
        }
    };
    if let Some(pair) = &parsed.pair {
        match extract(pair) {
            Ok((_, paired)) => contracts::pair_trace_folded_literals(&mut report, &paired),
            Err(error) => {
                eprintln!("shape signals contracts: --pair {pair}: {error}");
                return ExitCode::from(1);
            }
        }
    }
    if let Some(signal) = parsed.signal {
        if report.signal(signal).is_none() {
            eprintln!(
                "shape signals contracts: this capture has no descriptor {signal} ({} descriptors)",
                report.producers.descriptor_count
            );
            return ExitCode::from(1);
        }
        report
            .signals
            .retain(|contract| contract.descriptor == signal);
    }
    let rendered = if parsed.json {
        match serde_json::to_string_pretty(&report) {
            Ok(json) => json,
            Err(error) => {
                eprintln!("shape signals contracts: cannot serialize report: {error}");
                return ExitCode::from(1);
            }
        }
    } else {
        contracts::render_summary(&report)
    };
    match super::write_stdout_line(&rendered) {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("shape signals contracts: failed to write stdout: {error}");
            ExitCode::from(1)
        }
    }
}

fn inputs_summary(report: &shape::naming::inputs_context::ProgramInputContextReport) -> String {
    let inputs = &report.inputs;
    let mut out = format!(
        "source sha256={} bytes={}\n",
        inputs.source.sha256, inputs.source.bytes
    );
    out.push_str(&format!(
        "helpers: number={} string={} byte_stream={} hash={}\n",
        inputs.encoding.number.is_some(),
        inputs.encoding.string.is_some(),
        inputs.encoding.byte_stream.is_some(),
        inputs.encoding.seed_link.hash.is_some(),
    ));
    out.push_str(&format!(
        "wiring: collectors={} groups={} direct={} unresolved={}\n",
        inputs.wiring.collector_requests.len(),
        inputs.wiring.groups.len(),
        inputs.wiring.direct_routes.len(),
        inputs.wiring.unresolved.len(),
    ));
    out.push_str(&format!(
        "signals: {} emissions={} unresolved={}\n",
        inputs.signals.signals.len(),
        inputs
            .signals
            .signals
            .iter()
            .map(|signal| signal.emissions.len())
            .sum::<usize>(),
        inputs.signals.unresolved.len(),
    ));
    for family in &inputs.generator_inputs.families {
        let detail = match family.kind {
            shape::signals::extract::generator_inputs::FamilyKind::GlyphCanvasPixels => family
                .glyph
                .as_ref()
                .map(|glyph| format!(" table_entries={}", glyph.entries.len()))
                .unwrap_or_default(),
            shape::signals::extract::generator_inputs::FamilyKind::DomCensus => String::new(),
        };
        out.push_str(&format!(
            "family: {:?} emissions={} inputs={} unresolved={}{}\n",
            family.kind,
            family.emissions.len(),
            family.inputs.len(),
            family.unresolved.len(),
            detail,
        ));
    }
    out.push_str(&format!(
        "whole-program: nodes={} functions={} main_bindings={} unresolved={} joins={} output_carriers={}\n",
        report.role_graph.nodes.len(),
        report.role_graph.coverage.function_node_count,
        report.role_graph.coverage.main_variable_node_count,
        report.role_graph.unresolved.len(),
        report.joins.len(),
        report.output.carriers.len(),
    ));
    out
}

/// A compact, fact-first human summary of the typed report.
fn human_summary(properties: &shape::signals::schema::ProgramProperties) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "source sha256={} bytes={}\n",
        properties.source.sha256, properties.source.bytes
    ));
    out.push_str(&format!(
        "program: main={} argument={:?} returns_init={} bindings={}\n",
        properties
            .program_scope
            .main
            .name_label
            .as_deref()
            .unwrap_or("<anonymous>"),
        properties.program_scope.main_argument,
        properties.program_scope.returns_init_object,
        properties.program_scope.owned_binding_count,
    ));
    if let Some(descriptors) = &properties.descriptors {
        out.push_str(&format!(
            "registry: {} descriptors ({} literal, {} indexed population)\n",
            descriptors.length,
            descriptors
                .elements
                .iter()
                .filter(|element| element.population
                    == shape::signals::schema::DescriptorPopulation::ArrayLiteral)
                .count(),
            descriptors.population_assignments.len(),
        ));
    }
    if let Some(waves) = &properties.waves {
        let covered: usize = waves
            .waves
            .iter()
            .map(|wave| wave.signal_indices.len())
            .sum();
        out.push_str(&format!(
            "waves: {} covering {} slot(s); direct: {}; union={}\n",
            waves.waves.len(),
            covered,
            waves.direct_slots.len(),
            covered + waves.direct_slots.len(),
        ));
    }
    out.push_str(&format!(
        "signals: {} ({} with ordered emission layouts, {} emission record(s))\n",
        properties.signals.len(),
        properties
            .signals
            .iter()
            .filter(|signal| !signal.emission_layouts.is_empty())
            .count(),
        properties
            .signals
            .iter()
            .map(|signal| signal.emissions.len())
            .sum::<usize>(),
    ));
    out.push_str(&format!(
        "helpers: {}\n",
        properties
            .helpers
            .iter()
            .map(|helper| format!(
                "{:?}:{}",
                helper.role,
                helper
                    .function
                    .name_label
                    .as_deref()
                    .unwrap_or("<anonymous>")
            ))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    if let Some(session) = &properties.session {
        out.push_str(&format!(
            "session: arguments={} context_fields={} filter={} hash={} serializer={} envelope={}\n",
            session.argument_mapping.len(),
            session.context_fields.len(),
            session.default_filter.is_some(),
            session.hash_chain.is_some(),
            session.serializer.is_some(),
            session.envelope.is_some(),
        ));
        if let Some(envelope) = &session.envelope {
            if let (Some(prefix), Some(chunk)) =
                (&envelope.header_prefix, &envelope.header_chunk_size)
            {
                out.push_str(&format!(
                    "header: prefix={:?} chunk_size={:?}\n",
                    prefix, chunk
                ));
            }
        }
        if let Some(buffer) = &session.stores.error_buffer {
            out.push_str(&format!("error ring: {}\n", buffer.name));
        }
    }
    out.push_str(&format!(
        "static captures: {}; tag tables: {}; external inputs: {}; unresolved: {}; required missing: {}\n",
        properties.static_captures.len(),
        properties.constants.len(),
        properties.external_inputs.len(),
        properties.unresolved.len(),
        properties.required_missing_facts().len(),
    ));
    out.push_str("signal layouts:\n");
    for signal in &properties.signals {
        let route = if signal.direct {
            "direct".to_string()
        } else {
            format!(
                "wave {} slot {}",
                signal.wave.unwrap_or(0),
                signal.wave_position.unwrap_or(0)
            )
        };
        let masks: Vec<String> = signal
            .emission_layouts
            .iter()
            .filter_map(|emission| emission.mask.as_ref())
            .filter_map(|mask| mask.xor_literal)
            .map(|value| format!("{value}"))
            .collect();
        out.push_str(&format!(
            "  signal {} [{}] x{} emission(s){}",
            signal.signal,
            route,
            signal.emission_layouts.len(),
            if masks.is_empty() {
                String::new()
            } else {
                format!(" mask_xor=[{}]", masks.join(","))
            }
        ));
        out.push('\n');
    }
    out.push_str(&format!(
        "input sha256-verified by the extractor; report schema v{}\n",
        properties.schema_version
    ));
    out
}
