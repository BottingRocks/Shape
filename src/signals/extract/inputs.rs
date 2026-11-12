//! The single source-only composition entry point for glyph and DOM inputs.
//!
//! This module is deliberately a thin adapter.  Helper and collector identity
//! are discovered by the encoding and wiring passes for the same source, then
//! passed as spans to the signal pass, whose result is passed to the glyph/DOM
//! projection.  No generated spelling, signal count, source offset, or report
//! fixture is used as a lookup key.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::encoding::{extract_encoding_properties, EncodingProperties, EncodingPropertiesError};
use super::generator_inputs::{
    GeneratorInputError, GeneratorInputReport,
};
use crate::signals::extract::layout::{
    extract_signal_layouts, SignalLayoutError, SignalLayoutLimits, SignalLayoutReport,
};
use super::wiring::{WiringError, WiringProperties};

/// Version of the composed program-input report.
pub const PROGRAM_INPUT_SCHEMA_VERSION: u32 = 1;

/// A source-backed failure at one composition stage.  The underlying message
/// is retained so ambiguity, unsupported shapes, and missing mandatory facts
/// cannot be mistaken for an empty or confirmed report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgramInputError {
    InvalidInput { stage: String, message: String },
    Parse { stage: String, message: String },
    Limit { stage: String, message: String },
    Ambiguous { stage: String, message: String },
    Missing { stage: String, message: String },
    Unsupported { stage: String, message: String },
    SourceIdentity { stage: String, message: String },
}

impl std::fmt::Display for ProgramInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (stage, message) = match self {
            Self::InvalidInput { stage, message }
            | Self::Parse { stage, message }
            | Self::Limit { stage, message }
            | Self::Ambiguous { stage, message }
            | Self::Missing { stage, message }
            | Self::Unsupported { stage, message }
            | Self::SourceIdentity { stage, message } => (stage, message),
        };
        write!(f, "program-input {stage}: {message}")
    }
}

impl std::error::Error for ProgramInputError {}

impl From<EncodingPropertiesError> for ProgramInputError {
    fn from(error: EncodingPropertiesError) -> Self {
        let stage = "encoding".to_string();
        match error {
            EncodingPropertiesError::InvalidInput(message) => Self::InvalidInput { stage, message },
            EncodingPropertiesError::Parse(message) => Self::Parse { stage, message },
            EncodingPropertiesError::Limit { what, limit } => Self::Limit {
                stage,
                message: format!("{what} ({limit})"),
            },
            EncodingPropertiesError::Ambiguous { what, candidates } => Self::Ambiguous {
                stage,
                message: format!("{what}: {} candidates", candidates.len()),
            },
        }
    }
}

impl From<WiringError> for ProgramInputError {
    fn from(error: WiringError) -> Self {
        let stage = "wiring".to_string();
        match error {
            WiringError::InvalidInput(message) => Self::InvalidInput { stage, message },
            WiringError::Parse(message) => Self::Parse { stage, message },
            WiringError::Limit { what, limit } => Self::Limit {
                stage,
                message: format!("{what} ({limit})"),
            },
            WiringError::Ambiguous { what, candidates } => Self::Ambiguous {
                stage,
                message: format!("{what}: {} candidates", candidates.len()),
            },
            WiringError::Missing { what } => Self::Missing {
                stage,
                message: what.to_string(),
            },
        }
    }
}

impl From<SignalLayoutError> for ProgramInputError {
    fn from(error: SignalLayoutError) -> Self {
        let stage = "signals".to_string();
        match error {
            SignalLayoutError::Parse(message) => Self::Parse { stage, message },
            SignalLayoutError::Limit { what, limit } => Self::Limit {
                stage,
                message: format!("{what} ({limit})"),
            },
            SignalLayoutError::TooLarge { bytes } => Self::Limit {
                stage,
                message: format!("source bytes ({bytes})"),
            },
        }
    }
}

impl From<GeneratorInputError> for ProgramInputError {
    fn from(error: GeneratorInputError) -> Self {
        let stage = "generator_inputs".to_string();
        match error {
            GeneratorInputError::Parse(message) => Self::Parse { stage, message },
            GeneratorInputError::TooLarge { bytes } => Self::Limit {
                stage,
                message: format!("source bytes ({bytes})"),
            },
            GeneratorInputError::Ambiguous { family, candidates } => Self::Ambiguous {
                stage,
                message: format!("{family:?}: {} candidates", candidates.len()),
            },
            GeneratorInputError::Missing { family, reason } => Self::Missing {
                stage,
                message: format!("{family:?}: {reason}"),
            },
            GeneratorInputError::Signal(message) => Self::Unsupported { stage, message },
        }
    }
}

/// The complete source-derived input contract.  The individual reports are
/// retained so callers can inspect evidence and unresolved facts instead of
/// receiving a flattened set of guessed generator constants.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgramInputReport {
    pub schema_version: u32,
    pub source: crate::signals::schema::SourceIdentity,
    pub encoding: EncodingProperties,
    pub wiring: WiringProperties,
    pub signals: SignalLayoutReport,
    pub generator_inputs: GeneratorInputReport,
    /// Descriptors the core scanner proves are declared but never emitted:
    /// the direct task callback completes without ever mentioning an observer
    /// `next` ([`crate::signals::schema::WireLayoutStatus::DeclaredNotEmitted`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub declared_not_emitted: Vec<u32>,
}

impl ProgramInputReport {
    pub fn family(
        &self,
        kind: super::generator_inputs::FamilyKind,
    ) -> Option<&super::generator_inputs::GeneratorInputFamily> {
        self.generator_inputs.family(kind)
    }
}

/// Extract all source-derived inputs needed by the glyph and DOM contracts.
///
/// Every stage reparses or consumes facts from the exact `source` argument;
/// helper symbols are never compared across parses.  A missing mandatory
/// helper, ambiguous wiring candidate, unsupported call graph, or absent input
/// family is returned as an explicit typed error.
pub fn extract_program_inputs(source: &str) -> Result<ProgramInputReport, ProgramInputError> {
    if source.trim().is_empty() {
        return Err(ProgramInputError::InvalidInput {
            stage: "composition".into(),
            message: "source is empty".into(),
        });
    }

    // Encoding and wiring each parse the source on their own; run them side
    // by side. The encoding error still takes precedence.
    let (encoding, wiring) = std::thread::scope(|scope| {
        let wiring = std::thread::Builder::new()
            .stack_size(crate::LARGE_STACK_BYTES)
            .spawn_scoped(scope, || {
                let _t = crate::signals::timing::span("  wiring");
                super::wiring::extract_program_wiring_and_properties(source)
            })
            .expect("spawn wiring thread");
        let t = crate::signals::timing::span("  encoding");
        let encoding = extract_encoding_properties(source);
        drop(t);
        let wiring = match wiring.join() {
            Ok(wiring) => wiring,
            Err(panic) => std::panic::resume_unwind(panic),
        };
        (encoding, wiring)
    });
    let encoding = encoding?;
    let (wiring, core_properties) = wiring?;
    let expected = source_identity(source);
    verify_identity("encoding", &expected, &encoding.source)?;
    verify_identity("wiring", &expected, &wiring.source)?;

    // A recovered program can legitimately lack one of the helper families
    // (for example a build with no string writes). Keep the partial encoding:
    // the contract provenance still resolves the writes that are present.
    let helpers = encoding.helper_bindings();
    let mut collectors = wiring.signal_collector_requests();
    let t = crate::signals::timing::span("  worker collectors");
    add_worker_collectors(source, &core_properties, &mut collectors)?;
    let declared_not_emitted: Vec<u32> = core_properties
        .signals
        .iter()
        .filter(|record| {
            matches!(
                record.wire_layout,
                crate::signals::schema::WireLayoutStatus::DeclaredNotEmitted(_)
            )
        })
        .map(|record| record.signal)
        .collect();
    drop(core_properties);
    drop(t);
    if collectors.is_empty() {
        return Err(ProgramInputError::Missing {
            stage: "wiring".into(),
            message: "no source-derived signal collector requests were proven".into(),
        });
    }

    let request = crate::signals::extract::layout::SignalLayoutRequest {
        source,
        helpers,
        collectors,
        limits: SignalLayoutLimits::default(),
    };
    let t = crate::signals::timing::span("  signal layouts");
    let signals = extract_signal_layouts(&request)?;
    drop(t);
    let t = crate::signals::timing::span("  generator inputs");
    if signals.signals.is_empty() {
        return Err(ProgramInputError::Missing {
            stage: "signals".into(),
            message: "collector requests produced no signal emissions".into(),
        });
    }
    let generator_inputs = match super::generator_inputs::extract_generator_inputs_with_layout(&request, &signals) {
        Ok(report) => report,
        Err(GeneratorInputError::Missing { .. }) => GeneratorInputReport {
            schema_version: super::generator_inputs::GENERATOR_INPUT_SCHEMA_VERSION,
            source: super::generator_inputs::SourceIdentity {
                sha256: expected.sha256.clone(),
                bytes: expected.bytes,
            },
            families: Vec::new(),
            unresolved: Vec::new(),
        },
        Err(error) => return Err(error.into()),
    };
    drop(t);
    if expected.sha256 != generator_inputs.source.sha256
        || expected.bytes != generator_inputs.source.bytes
    {
        return Err(ProgramInputError::SourceIdentity {
            stage: "generator_inputs".into(),
            message: format!(
                "stage parsed different source (expected {} bytes {}, got {} bytes {})",
                expected.bytes,
                expected.sha256,
                generator_inputs.source.bytes,
                generator_inputs.source.sha256
            ),
        });
    }

    Ok(ProgramInputReport {
        schema_version: PROGRAM_INPUT_SCHEMA_VERSION,
        source: expected,
        encoding,
        wiring,
        signals,
        generator_inputs,
        declared_not_emitted,
    })
}

/// Join a tracer-linked worker to the one observer that forwards its reply.
/// The core scanner already proves that callback/observer association; only
/// its routing facts are reused here. The worker is then replayed normally so
/// its fields, masks and write accounting come from the same extractor.
fn add_worker_collectors(
    source: &str,
    properties: &crate::signals::schema::ProgramProperties,
    collectors: &mut Vec<crate::signals::extract::layout::CollectorRequest>,
) -> Result<(), ProgramInputError> {
    use crate::signals::extract::layout::{CollectorRequest, EmissionSite, ObserverWiring};
    use oxc_ast::ast::Function;
    use oxc_ast_visit::{walk, Visit};
    use oxc_span::Span;
    use oxc_syntax::scope::ScopeFlags;

    if !source.contains("vmWorkerCallbacks.push") {
        return Ok(());
    }
    // The same report as `extract_program_properties(source)`, already built
    // by the wiring pass.
    verify_identity(
        "worker_wiring",
        &source_identity(source),
        &properties.source,
    )?;
    let mut links = Vec::new();
    for signal in &properties.signals {
        // A direct slot's collector request scans its task callback, not the
        // constructor wrapper its record names: read the same function.
        let owner = if signal.direct {
            collectors
                .iter()
                .find(|request| {
                    request.site != EmissionSite::WorkerReply
                        && matches!(request.observers, ObserverWiring::Direct { signal: s } if s == signal.signal)
                })
                .map(|request| request.function)
        } else {
            signal.collector.as_ref().map(|owner| Span::new(owner.span.start, owner.span.end))
        };
        let Some(owner) = owner else {
            continue;
        };
        let Some(text) = source.get(owner.start as usize..owner.end as usize) else {
            continue;
        };
        let Some(parameter) = super::scanner::forwarded_parameter(text, owner.start, signal) else {
            continue;
        };
        let Some((thread, pc)) = super::scanner::posted_worker_target(text) else {
            continue;
        };
        links.push((
            signal.signal,
            owner.start,
            parameter,
            format!("$THREAD_{thread}_{pc}"),
        ));
    }
    struct Finder {
        names: std::collections::BTreeMap<String, Vec<Span>>,
    }
    impl<'a> Visit<'a> for Finder {
        fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
            if let Some(id) = &f.id {
                if let Some(found) = self.names.get_mut(id.name.as_str()) {
                    found.push(f.span);
                }
            }
            walk::walk_function(self, f, flags);
        }
    }
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::unambiguous()).parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Err(ProgramInputError::Parse {
            stage: "worker_wiring".into(),
            message: "worker source did not parse".into(),
        });
    }
    let mut finder = Finder {
        names: links
            .iter()
            .map(|link| (link.3.clone(), Vec::new()))
            .collect(),
    };
    finder.visit_program(&parsed.program);
    for (signal, owner, parameter, name) in links {
        let Some(spans) = finder.names.get(&name) else {
            continue;
        };
        if spans.len() != 1 {
            continue;
        }
        let Some(launcher) = collectors
            .iter_mut()
            .find(|request| request.function.start == owner)
        else {
            continue;
        };
        launcher.site = EmissionSite::WorkerForward;
        launcher.forwarded_parameter = Some(parameter);
        collectors.push(CollectorRequest {
            function: spans[0],
            observers: ObserverWiring::Direct { signal },
            site: EmissionSite::WorkerReply,
            forwarded_parameter: None,
        });
    }
    Ok(())
}

pub(crate) fn source_identity(source: &str) -> crate::signals::schema::SourceIdentity {
    let digest = Sha256::digest(source.as_bytes());
    crate::signals::schema::SourceIdentity {
        sha256: digest.iter().map(|byte| format!("{byte:02x}")).collect(),
        bytes: source.len(),
    }
}

pub(crate) fn verify_identity(
    stage: &str,
    expected: &crate::signals::schema::SourceIdentity,
    actual: &crate::signals::schema::SourceIdentity,
) -> Result<(), ProgramInputError> {
    if expected != actual {
        return Err(ProgramInputError::SourceIdentity {
            stage: stage.into(),
            message: format!(
                "stage parsed different source (expected {} bytes {}, got {} bytes {})",
                expected.bytes, expected.sha256, actual.bytes, actual.sha256
            ),
        });
    }
    Ok(())
}
