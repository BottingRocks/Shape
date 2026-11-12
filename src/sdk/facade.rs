//! Typed operation facade. Cache policies apply only to source compilation;
//! loaded inputs are reused in memory and outputs are persisted only by callers.
use super::{decode_error, failed, invalid, parsed, to_value, Result, COMMANDS, VERSION};
use serde::Serialize;
use serde_json::Value;
use std::{path::PathBuf, sync::Arc};

pub use super::{
    CaptureHeaders, CompileOptions, DecodeArtifact, DecodeOptions, DecodedPayload, FloatByteOrder,
    Session,
};
pub use crate::emitted_cleanup::ThreadCleanup;
pub use crate::naming::{
    collectors::FocusedCollectors, function_index::FunctionIndex,
    inputs_context::ProgramInputContextReport, labels::LabelReference,
    role_graph::ProgramRoleGraph, semantic_names::SemanticNamingPlan, NamedThreads,
};
pub use crate::signals::decode::CompactSeedSchema;
pub use crate::signals::payload::extract::SeedLayout;
pub use crate::signals::wire::NamedSignal;
pub use crate::signals::{
    analysis::SignalSchemaReport, extract::contracts::SignalContractReport,
    schema::ProgramProperties,
};
pub use crate::trace::corpus::KernelCachePolicy;
pub use crate::trace::corpus::KernelCachePolicy as CachePolicy;
pub use crate::trace::inputs::Snapshot;
pub use crate::{BranchOverrides, KernelArtifact};

/// Scoped version of the SDK boundary: borrows large immutable artifacts without
/// cloning their documents, while retaining the same stack and panic semantics.
fn run<T: Send>(f: impl FnOnce() -> Result<T> + Send) -> Result<T> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("shape".into())
            .stack_size(crate::LARGE_STACK_BYTES)
            .spawn_scoped(scope, move || {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(
                    |payload| {
                        Err(super::Error {
                            code: "panic",
                            message: payload
                                .downcast_ref::<&str>()
                                .map(|s| s.to_string())
                                .or_else(|| payload.downcast_ref::<String>().cloned())
                                .unwrap_or_else(|| "panic".into()),
                        })
                    },
                )
            })
            .expect("failed to spawn the large-stack worker thread")
            .join()
            .unwrap_or_else(|payload| std::panic::resume_unwind(payload))
    })
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct VersionInfo {
    pub version: &'static str,
    pub commands: &'static [&'static str],
}
pub fn version() -> VersionInfo {
    VersionInfo {
        version: VERSION,
        commands: COMMANDS,
    }
}
pub fn commands() -> &'static [&'static str] {
    COMMANDS
}

#[derive(Clone, Copy, Debug)]
pub enum KernelSource<'a> {
    Raw(&'a str),
    Deobfuscated(&'a str),
}
#[derive(Clone, Copy, Debug)]
pub enum KernelInput<'a> {
    Source(&'a str),
    Artifact(&'a KernelArtifact),
}
#[derive(Clone, Copy, Debug)]
pub enum ProgramInput<'a> {
    Program(&'a str),
    Raw(&'a str),
}
#[derive(Clone, Copy)]
pub enum SessionInput<'a> {
    Session(&'a Session),
    Initializer(&'a str),
}

#[derive(Clone, Copy, Debug, Default)]
pub struct DeobOptions {
    pub artifact: bool,
}
pub struct DeobResult {
    pub code: String,
    pub artifact: Option<KernelArtifact>,
}
pub fn deob(source: &str, options: DeobOptions) -> Result<DeobResult> {
    run(|| {
        if !options.artifact {
            return Ok(DeobResult {
                code: crate::deobfuscate(source).map_err(failed)?,
                artifact: None,
            });
        }
        let report = crate::deobfuscate_with_report(source).map_err(failed)?;
        let artifact =
            KernelArtifact::compile_from_raw(source, &report, crate::PrefixLimits::default())
                .map_err(failed)?;
        Ok(DeobResult {
            code: report.output,
            artifact: Some(artifact),
        })
    })
}
pub fn report(source: KernelSource<'_>) -> Result<KernelArtifact> {
    run(|| {
        match source {
            KernelSource::Raw(source) => {
                KernelArtifact::compile_source(source, crate::PrefixLimits::default())
            }
            KernelSource::Deobfuscated(source) => {
                KernelArtifact::compile_from_deobfuscated(source, crate::PrefixLimits::default())
            }
        }
        .map_err(failed)
    })
}

#[derive(Clone, Debug, Default)]
pub struct KernelShowOptions {
    pub operations: Vec<String>,
}
pub mod kernel {
    use super::*;
    pub fn show(source: KernelSource<'_>, options: KernelShowOptions) -> Result<Value> {
        run(|| {
            let standardized = match source {
                KernelSource::Raw(s) => crate::deobfuscate(s).map_err(failed)?,
                KernelSource::Deobfuscated(s) => s.to_owned(),
            };
            let properties = crate::extract_actual_properties(&standardized).map_err(failed)?;
            properties
                .validate_selected_operations(&options.operations)
                .map_err(invalid)?;
            parsed(properties.tracer_json(&options.operations))
        })
    }
    pub fn load(json: &str) -> Result<KernelArtifact> {
        run(|| KernelArtifact::from_json(json).map_err(failed))
    }
    pub fn to_json(artifact: &KernelArtifact) -> Result<String> {
        run(|| artifact.to_json().map_err(failed))
    }
}

pub mod ops {
    pub use super::super::extensions::ops::classify;
    use super::*;
    pub fn list(raw_source: &str) -> Result<Value> {
        run(|| project(raw_source, None))
    }
    pub fn show(raw_source: &str, op: u32) -> Result<Value> {
        run(|| project(raw_source, Some(op)))
    }
    fn project(source: &str, key: Option<u32>) -> Result<Value> {
        use crate::operations::view::{
            available_keys, handler_by_key, ops_list_json, ops_show_json, Capture, OpsView,
        };
        let digest = crate::source::sha256_hex(source.as_bytes());
        let report = crate::deobfuscate_with_report(source).map_err(failed)?;
        let catalogue = crate::catalogue_from_report(&report).map_err(failed)?;
        let view = OpsView {
            catalogue: &catalogue,
            capture: Capture {
                input_kind: "raw",
                digest: &digest,
                byte_length: source.len() as u64,
                raw_identity: "verified",
            },
            span_source: None,
            analyses: None,
            annotation: None,
        };
        match key {
            None => parsed(ops_list_json(&view)),
            Some(key) => {
                let handler = handler_by_key(&catalogue, key).ok_or_else(|| {
                    invalid(format!(
                        "unknown operation key {key}; available: {:?}",
                        available_keys(&catalogue)
                    ))
                })?;
                parsed(ops_show_json(&view, handler))
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProgramOptions {
    pub name: PathBuf,
    /// Explicit policy for kernel compilation; never a program/AST cache.
    pub cache: CachePolicy,
    pub steps: usize,
    /// Clean emitted threads in parallel (default true).
    pub thread_cleanup: bool,
    /// Skip the full mechanical cleanup pipeline for diagnostic output (default false).
    /// Semantic naming operations are separate and optional.
    pub no_cleanup: bool,
    pub check_reuse: bool,
    pub overrides: BranchOverrides,
    pub jobs: Option<usize>,
    pub quiet: bool,
}
impl Default for ProgramOptions {
    fn default() -> Self {
        Self {
            name: "seed.js".into(),
            cache: CachePolicy::Disabled,
            steps: 1_000_000,
            thread_cleanup: true,
            no_cleanup: false,
            check_reuse: false,
            overrides: Default::default(),
            jobs: None,
            quiet: true,
        }
    }
}
impl ProgramOptions {
    fn link_options(&self) -> crate::trace::program::LinkOptions {
        crate::trace::program::LinkOptions {
            seed: self.name.clone(),
            report_dir: None,
            steps: self.steps,
            dump_stages: None,
            thread_cleanup: self.thread_cleanup,
            no_cleanup: self.no_cleanup,
            check_reuse: self.check_reuse,
            overrides: self.overrides.clone(),
            jobs: self.jobs,
            quiet: self.quiet,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct TraceStop {
    pub function: String,
    pub reason: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct ProgramTrace {
    pub code: String,
    pub complete: bool,
    pub traced: usize,
    pub stopped: Vec<TraceStop>,
    pub kernel_globals: Vec<String>,
}
impl From<crate::trace::program::LinkedProgram> for ProgramTrace {
    fn from(linked: crate::trace::program::LinkedProgram) -> Self {
        Self {
            code: linked.code,
            complete: linked.stopped.is_empty(),
            traced: linked.traced,
            stopped: linked
                .stopped
                .into_iter()
                .map(|(function, reason)| TraceStop { function, reason })
                .collect(),
            kernel_globals: linked.kernel_globals,
        }
    }
}
#[derive(Clone, Debug)]
pub struct ThreadOptions {
    pub pc: usize,
    pub state: usize,
    pub descriptor: Option<usize>,
    pub initial_slots: Option<Vec<u32>>,
    pub inputs: Option<Snapshot>,
    pub steps: usize,
    pub self_check: bool,
    pub outline: bool,
    pub reverse_sides: bool,
    pub partial: bool,
    pub overrides: BranchOverrides,
}
impl ThreadOptions {
    pub fn new(pc: usize, state: usize) -> Self {
        Self {
            pc,
            state,
            descriptor: None,
            initial_slots: None,
            inputs: None,
            steps: 1_000_000,
            self_check: false,
            outline: true,
            reverse_sides: false,
            partial: false,
            overrides: Default::default(),
        }
    }
    fn into_core(self) -> crate::trace::thread::ThreadOptions {
        crate::trace::thread::ThreadOptions {
            pc: self.pc,
            state: self.state,
            descriptor: self.descriptor,
            initial_slots: self.initial_slots,
            inputs: self.inputs,
            steps: self.steps,
            self_check: self.self_check,
            outline: self.outline,
            reverse_sides: self.reverse_sides,
            partial: self.partial,
            overrides: self.overrides,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct ThreadTrace {
    pub code: Option<String>,
    pub complete: bool,
    pub stop: String,
    pub steps: usize,
    pub handlers_traced: usize,
    pub undeclared: Vec<String>,
    pub self_check: Vec<String>,
    pub source_hash: String,
}
pub mod trace {
    pub use super::super::extensions::trace::corpus;
    use super::*;
    pub fn program(raw: &str, options: ProgramOptions) -> Result<ProgramTrace> {
        run(|| {
            crate::trace::program::link_source_with_cache(
                raw,
                &options.link_options(),
                &options.cache,
            )
            .map(Into::into)
            .map_err(failed)
        })
    }
    /// Trace with a supplied kernel. `options.cache` is unused: no disk cache is consulted.
    pub fn program_with_artifact(
        raw: &str,
        artifact: &KernelArtifact,
        options: ProgramOptions,
    ) -> Result<ProgramTrace> {
        run(|| {
            crate::trace::program::link_source_with_artifact(raw, &options.link_options(), artifact)
                .map(Into::into)
                .map_err(failed)
        })
    }
    pub fn thread(input: KernelInput<'_>, options: ThreadOptions) -> Result<ThreadTrace> {
        run(|| {
            let compiled;
            let artifact = match input {
                KernelInput::Source(source) => {
                    compiled =
                        crate::trace::thread::compile_kernel_artifact(source).map_err(failed)?;
                    &compiled
                }
                KernelInput::Artifact(artifact) => artifact,
            };
            let walked = crate::trace::thread::walk_thread(artifact, &options.into_core())
                .map_err(failed)?;
            Ok(ThreadTrace {
                code: walked.code,
                complete: walked.completed,
                stop: walked.report.stop.to_string(),
                steps: walked.report.records.len(),
                handlers_traced: walked.report.trace.handlers_traced,
                undeclared: walked.undeclared,
                self_check: walked.report.self_check,
                source_hash: artifact.source_hash().to_owned(),
            })
        })
    }
}

pub mod cleanup {
    use super::*;
    pub fn thread(source: &str) -> Result<ThreadCleanup> {
        run(|| crate::emitted_cleanup::cleanup_thread(source).map_err(failed))
    }
}
#[derive(Clone, Default)]
pub struct NamingThreadsOptions {
    pub rename: bool,
    pub labels: Option<LabelReference>,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct NamingCollectorsOptions {
    pub rename: bool,
}
#[derive(Serialize)]
pub struct NamingInventory {
    pub function_index: FunctionIndex,
    pub role_graph: ProgramRoleGraph,
}
pub mod naming {
    pub use super::super::extensions::naming::corpus;
    use super::*;
    pub fn threads(source: &str, options: NamingThreadsOptions) -> Result<NamedThreads> {
        run(|| {
            crate::naming::name_threads_with_labels(source, options.rename, options.labels.as_ref())
                .map_err(failed)
        })
    }
    pub fn collectors(source: &str, options: NamingCollectorsOptions) -> Result<FocusedCollectors> {
        run(|| {
            crate::naming::collectors::identify_collectors(source, options.rename).map_err(failed)
        })
    }
    pub fn plan(source: &str) -> Result<SemanticNamingPlan> {
        run(|| crate::naming::semantic_names::plan_semantic_names(source).map_err(failed))
    }
    pub fn inventory(source: &str) -> Result<NamingInventory> {
        run(|| {
            Ok(NamingInventory {
                function_index: crate::naming::function_index::extract_function_index(source)
                    .map_err(failed)?,
                role_graph: crate::naming::role_graph::extract_program_role_graph(source)
                    .map_err(failed)?,
            })
        })
    }
}
#[derive(Clone, Debug)]
pub struct SchemaOptions {
    pub name: Option<PathBuf>,
    pub layouts_only: bool,
    pub finder_roles: bool,
}
impl Default for SchemaOptions {
    fn default() -> Self {
        Self {
            name: None,
            layouts_only: false,
            finder_roles: true,
        }
    }
}
pub mod signals {
    use super::*;
    pub fn schema(program: &str, options: SchemaOptions) -> Result<SignalSchemaReport> {
        run(|| {
            let mut analysis = crate::signals::analysis::analyze_program_signals(
                program,
                &crate::signals::analysis::SignalFinderOptions {
                    layouts_only: options.layouts_only,
                    finder_roles: options.finder_roles,
                },
            )
            .map_err(failed)?;
            analysis.report.seed = options.name.as_ref().map(|p| {
                p.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_else(|| p.to_str().unwrap_or("seed.js"))
                    .to_owned()
            });
            Ok(analysis.report)
        })
    }
    pub fn show(program: &str) -> Result<ProgramProperties> {
        run(|| {
            crate::signals::extract::scanner::extract_program_properties(program).map_err(failed)
        })
    }
    pub fn inputs(program: &str) -> Result<ProgramInputContextReport> {
        run(|| {
            crate::naming::inputs_context::extract_program_inputs_with_context(program)
                .map_err(failed)
        })
    }
    pub fn contracts(program: &str) -> Result<SignalContractReport> {
        run(|| {
            crate::signals::extract::contracts::extract_signal_contracts(program).map_err(failed)
        })
    }
    pub fn compact_schema(program: &str, options: CompileOptions) -> Result<CompactSeedSchema> {
        run(|| CompactSeedSchema::from_source_with_options(program, options).map_err(failed))
    }
    /// Static descriptions and examples, without capture observations or measured sizes.
    pub fn catalog() -> Result<Vec<Value>> {
        run(|| {
            let mut cards: Vec<_> = crate::signals::catalog::cards().collect();
            cards.sort_by_key(|card| card.signal);
            Ok(cards
                .into_iter()
                .map(|card| crate::signals::catalog::view::catalogue_json(card, None, false))
                .collect())
        })
    }
    pub fn describe(name: &str) -> Result<Value> {
        run(|| {
            let card = crate::signals::catalog::card(name)
                .ok_or_else(|| invalid(format!("unknown signal {name:?}")))?;
            Ok(crate::signals::catalog::view::catalogue_json(
                card, None, false,
            ))
        })
    }
}
pub mod session {
    use super::*;
    pub fn from_initializer(init: &str) -> Result<Session> {
        run(|| crate::signals::payload::extract::session_from_initializer(init).map_err(failed))
    }
}
pub fn pin(program: &str, session: SessionInput<'_>) -> Result<SeedLayout> {
    run(|| {
        let parsed;
        let session = match session {
            SessionInput::Session(session) => session,
            SessionInput::Initializer(init) => {
                parsed = crate::signals::payload::extract::session_from_initializer(init)
                    .map_err(failed)?;
                &parsed
            }
        };
        Ok(crate::signals::payload::extract::pin(program, session)
            .map_err(failed)?
            .layout)
    })
}

/// An immutable executable artifact, validated once and cheaply shared across calls.
#[derive(Clone)]
pub struct LoadedArtifact {
    artifact: Arc<crate::signals::decode::ValidatedArtifact>,
}
/// Move the already validated state into the compatible native handle owner.
impl From<LoadedArtifact> for super::Generator {
    fn from(loaded: LoadedArtifact) -> Self {
        Self {
            artifact: loaded.artifact,
        }
    }
}
impl LoadedArtifact {
    pub fn artifact(&self) -> &DecodeArtifact {
        self.artifact.artifact()
    }
    pub fn to_json(&self) -> Result<Vec<u8>> {
        run(|| serde_json::to_vec(self.artifact()).map_err(failed))
    }
}
#[derive(Clone, Debug)]
pub struct ArtifactOptions {
    pub name: PathBuf,
    pub cache: CachePolicy,
    pub float_byte_order: FloatByteOrder,
    pub direct: bool,
}
impl Default for ArtifactOptions {
    fn default() -> Self {
        Self {
            name: "seed.js".into(),
            cache: CachePolicy::Disabled,
            float_byte_order: FloatByteOrder::Little,
            direct: false,
        }
    }
}
pub mod artifact {
    use super::*;
    pub fn load(json: &[u8]) -> Result<LoadedArtifact> {
        run(|| {
            Ok(LoadedArtifact {
                artifact: Arc::new(
                    crate::signals::decode::ValidatedArtifact::from_json(json).map_err(failed)?,
                ),
            })
        })
    }
    pub fn from_artifact(artifact: DecodeArtifact) -> Result<LoadedArtifact> {
        run(|| validated(artifact))
    }
    fn validated(artifact: DecodeArtifact) -> Result<LoadedArtifact> {
        Ok(LoadedArtifact {
            artifact: Arc::new(
                crate::signals::decode::ValidatedArtifact::new(artifact).map_err(failed)?,
            ),
        })
    }
    pub fn compile(
        input: ProgramInput<'_>,
        session: &Session,
        site_id: &str,
        options: ArtifactOptions,
    ) -> Result<LoadedArtifact> {
        run(|| {
            let traced;
            let program = match input {
                ProgramInput::Program(program) => program,
                ProgramInput::Raw(raw) => {
                    let trace_options = ProgramOptions {
                        name: options.name,
                        cache: options.cache,
                        thread_cleanup: true,
                        ..Default::default()
                    };
                    let linked = crate::trace::program::link_source_with_cache(
                        raw,
                        &trace_options.link_options(),
                        &trace_options.cache,
                    )
                    .map_err(failed)?;
                    if !linked.stopped.is_empty() {
                        return Err(failed(
                            "seed tracing stopped; inspect with trace.program before compiling",
                        ));
                    }
                    traced = linked.code;
                    &traced
                }
            };
            validated(
                DecodeArtifact::compile_with_options(
                    program,
                    session,
                    site_id.to_owned(),
                    options.float_byte_order,
                    CompileOptions {
                        direct: options.direct,
                    },
                )
                .map_err(failed)?,
            )
        })
    }
    pub fn compact_schema(artifact: &DecodeArtifact) -> Result<CompactSeedSchema> {
        run(|| {
            artifact.validate().map_err(failed)?;
            artifact.compact_schema().map_err(failed)
        })
    }
}
pub type NamedSignals = std::collections::BTreeMap<String, serde_json::Map<String, Value>>;
pub fn decode(
    artifact: &LoadedArtifact,
    capture: &CaptureHeaders,
    session: Option<&Session>,
    options: DecodeOptions,
) -> Result<DecodedPayload> {
    run(|| {
        artifact
            .artifact
            .decode(capture, session, options)
            .map_err(decode_error)
    })
}
pub fn explain(
    artifact: &LoadedArtifact,
    decoded: &DecodedPayload,
    signal: &str,
    session: Option<&Session>,
    verify: bool,
) -> Result<NamedSignal> {
    run(|| {
        if decoded.site_id != artifact.artifact().site_id {
            return Err(invalid("payload/artifact siteId mismatch"));
        }
        artifact
            .artifact
            .explain(decoded, signal, session, verify)
            .map_err(decode_error)
    })
}
pub fn encode(
    artifact: &LoadedArtifact,
    decoded: &DecodedPayload,
    session: Option<&Session>,
) -> Result<CaptureHeaders> {
    // The shared encoder consumes the domain payload as a Value. This conversion
    // preserves its original-report metadata; no dispatcher request is constructed.
    run(|| {
        artifact
            .artifact
            .encode_named(&to_value(decoded)?, session)
            .map_err(decode_error)
    })
}
pub fn encode_signals(
    artifact: &LoadedArtifact,
    signals: &NamedSignals,
    session: Option<&Session>,
) -> Result<CaptureHeaders> {
    run(|| {
        artifact
            .artifact
            .encode_named(&serde_json::json!({"positionalSignals": signals}), session)
            .map_err(decode_error)
    })
}
pub fn schema(artifact: &LoadedArtifact) -> Result<CompactSeedSchema> {
    run(|| artifact.artifact().compact_schema().map_err(failed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_boundary_borrows_inputs_and_contains_panics() {
        let borrowed = String::from("borrowed without a document clone");
        assert_eq!(
            run(|| {
                assert_eq!(std::thread::current().name(), Some("shape"));
                Ok(borrowed.as_str())
            })
            .unwrap(),
            borrowed
        );
        let error = run::<()>(|| panic!("facade panic probe")).unwrap_err();
        assert_eq!(error.code, "panic");
        assert_eq!(error.message, "facade panic probe");
        assert_eq!(run(|| Ok(7)).unwrap(), 7);
    }
}
