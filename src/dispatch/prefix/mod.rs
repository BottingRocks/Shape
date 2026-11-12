//! A bounded, proof-backed *prefix* execution contract for a JavaScript VM.
//!
//! The complete-dispatch API in this module's siblings requires a
//! source-proven terminal selector, a fully lifted handler table, and an
//! immutable image. The inspected capture has none of those: it completes
//! through a data register, most handlers are partial, and one proven bytecode
//! handler rewrites the image at run time. This module therefore adds an explicit
//! *prefix* contract:
//!
//! * preparation proves the JavaScript VM dispatch read, the exact run-loop topology, the
//!   initial image snapshot (a proven base64 decoder call or a bounded literal
//!   array), and exactly one bounded bytecode-write handler, all through Oxc
//!   `SymbolId`s and genuine source spans;
//! * execution clones the initial image per invocation, dispatches through the
//!   proven table, applies the recognized image write to the clone
//!   transactionally, reuses the ordinary [`TraceSession`] recipes for
//!   supported handlers, and stops honestly at the first boundary.
//!
//! It never fabricates a completed [`crate::DispatchPreparation`]
//! and never reports this prefix as a complete JavaScript program.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::{
    DispatchCell, DispatchEntryError, DispatchStart, HandlerTemplateReport,
    JsOperandSeparationReport, OperandSpan, ProvenOperationTable, SymbolicValue, ThreadDescriptor,
    ThreadInputs, ThreadStartupError, TraceContext, TraceDebugOptions, TraceLimits,
};

use self::proof::{PatchHandlerProof, RunLoopProof};
use super::thread::{
    build_captured_memory, build_context_from_spec, collect_invocation_arguments,
    validate_descriptor, ThreadContextSpec,
};

mod artifact;
mod execute;
mod fresh_stack;
mod prepare;
mod proof;
mod types;
pub(crate) mod wasm;

pub use artifact::{
    DescriptorRecord, EntryFacts, KernelArtifact, KernelArtifactCompileError, KernelArtifactError,
    ARTIFACT_FORMAT, COMPILER_PROVENANCE, MAX_ARTIFACT_BYTES,
};
pub use prepare::{prepare_prefix_from_report, prepare_prefix_from_saved_report};
pub use types::{
    ImagePatchRecord, PrefixBranch, PrefixBranchKind, PrefixExecutionRecord, PrefixLimits,
    PrefixPrepareError, PrefixRetraces, PrefixStepEffect, PrefixStopReason, PrefixTraceReport,
};

/// Proven image-patch operand layout: three little-endian address bytes followed
/// by one value byte. The proof and the decoder share these constants, so a
/// layout change cannot silently leave one side decoding fixed offsets.
pub(super) const PATCH_ADDRESS_BYTES: usize = 3;
pub(super) const PATCH_VALUE_BYTES: usize = 1;
pub(super) const PATCH_OPERAND_WIDTH: usize = PATCH_ADDRESS_BYTES + PATCH_VALUE_BYTES;

/// A validated, read-only prefix program. Every field is proved from the
/// standardized source and the runtime-config facts; callers bind an entry and
/// a caller-supplied thread descriptor at invocation time.
#[derive(Debug, Clone)]
pub struct PrefixPreparation {
    source_hash: String,
    artifact_source_hash: String,
    initial_image: Vec<u8>,
    table: Vec<Vec<DispatchCell>>,
    patch_handler: PatchHandlerProof,
    templates: HandlerTemplateReport,
    operands: JsOperandSeparationReport,
    /// Selector -> first position in `templates.handlers`, prepared once.
    template_index: super::step::SelectorIndex,
    /// Selector -> first position in `operands.handlers`, prepared once.
    operand_index: super::step::SelectorIndex,
    limits: PrefixLimits,
    bytecode_span: OperandSpan,
    run_loop: RunLoopProof,
    operation_table: ProvenOperationTable,
    /// Extracted constant string tables registered on every trace session.
    constant_tables: Vec<(String, std::sync::Arc<[String]>)>,
    /// Extracted constant number tables (`vmNumberTable`), registered the
    /// same way: a read at a known index is that number.
    number_tables: Vec<(String, std::sync::Arc<[f64]>)>,
    /// The proven `vmDecodeString`: a call of it with known operands is the
    /// string it returns.
    string_decoder: Option<std::sync::Arc<crate::vm_string::VmStringDecoder>>,
    /// Set for a kernel whose dispatch loop is a WASM interpreter.
    wasm: Option<wasm::WasmMachine>,
    /// What [`execute::Walk::junk_entry`] found for each VM function made,
    /// by descriptor, entry pc and state, and whether the walk knew the
    /// descriptors' starting slots: it depends on nothing else, so every
    /// walk of the seed shares it.
    pub(super) junk_entries: JunkEntries,
}

/// See [`PrefixPreparation::junk_entries`].
pub(super) type JunkEntries =
    std::sync::Arc<std::sync::Mutex<std::collections::HashMap<(usize, usize, usize, bool), bool>>>;

impl PrefixPreparation {
    /// Compile the source-proven preparation into a source-free runtime
    /// artifact. This is the one-time boundary where source proof is allowed.
    pub fn export_artifact(&self) -> Result<KernelArtifact, KernelArtifactError> {
        KernelArtifact::from_preparation(self)
    }

    /// Import a validated artifact without parsing or analyzing source.
    pub fn import_artifact(artifact: &KernelArtifact) -> Result<Self, KernelArtifactError> {
        artifact.prepare()
    }

    /// The source identity carried by the input contract. Raw preparation
    /// verifies this against the supplied seed; saved-report preparation
    /// preserves the report's declared capture identity because the seed is
    /// intentionally absent.
    #[must_use]
    pub fn source_hash(&self) -> &str {
        &self.source_hash
    }

    /// The freshly computed identity of the standardized `PipelineReport`
    /// artifact that was structurally and natively reproved during
    /// preparation. This is distinct from [`Self::source_hash`], especially
    /// for report-only inputs.
    #[must_use]
    pub fn artifact_source_hash(&self) -> &str {
        &self.artifact_source_hash
    }

    /// The immutable initial image snapshot. Every invocation clones it.
    #[must_use]
    pub fn initial_image(&self) -> &[u8] {
        &self.initial_image
    }

    #[must_use]
    pub fn table(&self) -> &[Vec<DispatchCell>] {
        &self.table
    }

    /// The recognized bytecode-write handler selector.
    #[must_use]
    pub fn patch_selector(&self) -> u32 {
        self.patch_handler.selector
    }

    #[must_use]
    pub fn patch_handler_span(&self) -> OperandSpan {
        self.patch_handler.span.clone()
    }

    #[must_use]
    pub fn templates(&self) -> &HandlerTemplateReport {
        &self.templates
    }

    #[must_use]
    pub fn operands(&self) -> &JsOperandSeparationReport {
        &self.operands
    }

    #[must_use]
    pub fn limits(&self) -> PrefixLimits {
        self.limits
    }

    pub(crate) fn set_limits(&mut self, limits: PrefixLimits) {
        self.limits = limits;
    }

    /// Genuine span of the proven bytecode-image binding.
    #[must_use]
    pub fn bytecode_span(&self) -> OperandSpan {
        self.bytecode_span.clone()
    }

    #[must_use]
    pub fn operation_table(&self) -> &ProvenOperationTable {
        &self.operation_table
    }

    /// Genuine span of the proven JavaScript VM run loop.
    #[must_use]
    pub fn run_loop_span(&self) -> OperandSpan {
        self.run_loop.span.clone()
    }

    #[must_use]
    pub fn completion_register(&self) -> &str {
        &self.run_loop.completion_register
    }

    #[must_use]
    pub fn exception_helper_span(&self) -> Option<OperandSpan> {
        self.run_loop.exception_helper.clone()
    }

    pub fn validate_entry(&self, start: DispatchStart) -> Result<(), DispatchEntryError> {
        if start.pc >= self.initial_image.len() {
            return Err(DispatchEntryError::PcOutOfBounds {
                pc: start.pc,
                image_len: self.initial_image.len(),
            });
        }
        if start.state >= self.table.len() {
            return Err(DispatchEntryError::StateOutOfBounds {
                state: start.state,
                state_count: self.table.len(),
            });
        }
        Ok(())
    }

    /// Bind a validated entry and a caller-supplied descriptor/capture
    /// snapshot to this prefix, without re-proving or rebuilding anything.
    pub fn thread(
        &self,
        entry: DispatchStart,
        descriptor: ThreadDescriptor,
        inputs: ThreadInputs,
    ) -> Result<PrefixThreadInvocation<'_>, ThreadStartupError> {
        self.validate_entry(entry)?;
        validate_descriptor(&descriptor, &inputs)?;
        let captured_memory = build_captured_memory(&descriptor, &inputs)?;
        let max_invocation_arguments = inputs
            .max_invocation_arguments
            .unwrap_or(inputs.limits.max_stack_length);
        Ok(PrefixThreadInvocation {
            preparation: self,
            entry,
            descriptor,
            captured_memory,
            arguments_object: inputs.arguments_object,
            receiver: inputs.receiver.unwrap_or(SymbolicValue::Undefined),
            incoming_stack: inputs.incoming_stack,
            trace_limits: inputs.limits,
            max_invocation_arguments,
            captures: None,
            starts: None,
            cell_contents: BTreeMap::new(),
            cells: std::collections::BTreeSet::new(),
            created: Created::default(),
            returns: Returns::default(),
            overrides: Arc::default(),
        })
    }
}

/// A validated, reusable prefix thread invocation. Each invocation clones the
/// preparation's initial image, so mutations never leak between invocations.
#[derive(Debug, Clone)]
pub struct PrefixThreadInvocation<'a> {
    preparation: &'a PrefixPreparation,
    entry: DispatchStart,
    descriptor: ThreadDescriptor,
    captured_memory: BTreeMap<u32, SymbolicValue>,
    arguments_object: Option<SymbolicValue>,
    receiver: SymbolicValue,
    incoming_stack: Vec<Option<SymbolicValue>>,
    trace_limits: TraceLimits,
    max_invocation_arguments: usize,
    captures: Option<Arc<[Vec<u32>]>>,
    starts: Option<Arc<[Vec<(u32, bool)>]>>,
    cell_contents: BTreeMap<u32, crate::CellContent>,
    cells: std::collections::BTreeSet<u32>,
    created: Created,
    returns: Returns,
    overrides: Arc<crate::BranchOverrides>,
}

/// [`PrefixThreadInvocation::with_returns_hook`]'s hook.
#[derive(Clone, Default)]
struct Returns(
    Option<crate::tracing::symbolic::ReturnsHook>,
    Option<crate::tracing::symbolic::CellTypesHook>,
);

impl std::fmt::Debug for Returns {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_some() {
            "Returns(hook)"
        } else {
            "Returns(none)"
        })
    }
}

/// [`PrefixThreadInvocation::on_created`]'s hook.
#[derive(Clone, Default)]
struct Created(Option<crate::tracing::symbolic::CreatedHook>);

impl std::fmt::Debug for Created {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_some() {
            "Created(hook)"
        } else {
            "Created(none)"
        })
    }
}

impl<'a> PrefixThreadInvocation<'a> {
    /// The slots each thread descriptor (`vmThreadDescriptors[i]`)
    /// captures. With them a closure this thread creates is handed the
    /// shared cells of those slots, and this thread reads and writes them
    /// as cells; without them only the slots this thread captured are.
    #[must_use]
    pub fn with_thread_captures(mut self, captures: Arc<[Vec<u32>]>) -> Self {
        self.captures = Some(captures);
        self
    }

    /// The slots each thread descriptor initializes to `undefined`: a VM
    /// function this thread creates starts with them (see the check of
    /// what it does first, `Walk::junk_entry`).
    #[must_use]
    pub fn with_thread_starts(mut self, starts: Arc<[Vec<(u32, bool)>]>) -> Self {
        self.starts = Some(starts);
        self
    }

    /// What each shared cell of this thread (by slot) can hold, gathered
    /// from every function that holds the cell. A read of the cell with no
    /// value of its own known holds one of those; a VM jump through it
    /// dispatches on the addresses, and one that matches none went to a
    /// JavaScript value. Only complete contents may be given.
    #[must_use]
    pub fn with_cell_contents(mut self, contents: BTreeMap<u32, crate::CellContent>) -> Self {
        self.cell_contents = contents;
        self
    }

    /// Branches the user decides, by the jump instruction's position (see
    /// [`crate::BranchOverrides`]).
    #[must_use]
    pub fn with_branch_overrides(mut self, overrides: Arc<crate::BranchOverrides>) -> Self {
        self.overrides = overrides;
        self
    }

    /// Trace these slots as shared cells from the start: the cells an
    /// earlier walk of this function found (see
    /// [`PrefixTraceReport::cells`]), so this walk need not find them again.
    #[must_use]
    pub fn with_known_cells(mut self, cells: std::collections::BTreeSet<u32>) -> Self {
        self.cells = cells;
        self
    }

    /// Tell `created` each VM function the walk creates, the moment it
    /// does (see [`crate::tracing::symbolic::CreatedHook`]).
    #[must_use]
    pub fn on_created(mut self, created: crate::tracing::symbolic::CreatedHook) -> Self {
        self.created = Created(Some(created));
        self
    }

    /// Ask `returns` what a call of a VM function the walk makes returns
    /// (see [`crate::tracing::symbolic::ReturnsHook`]).
    #[must_use]
    pub fn with_returns_hook(
        mut self,
        returns: Option<crate::tracing::symbolic::ReturnsHook>,
    ) -> Self {
        self.returns.0 = returns;
        self
    }

    /// Ask `types` what the closures the walk makes put in the cells they
    /// capture (see [`crate::tracing::symbolic::CellTypesHook`]).
    #[must_use]
    pub fn with_cell_types_hook(
        mut self,
        types: Option<crate::tracing::symbolic::CellTypesHook>,
    ) -> Self {
        self.returns.1 = types;
        self
    }

    #[must_use]
    pub fn entry(&self) -> DispatchStart {
        self.entry
    }

    #[must_use]
    pub fn descriptor(&self) -> &ThreadDescriptor {
        &self.descriptor
    }

    #[must_use]
    pub fn preparation(&self) -> &'a PrefixPreparation {
        self.preparation
    }

    #[must_use]
    pub fn trace_limits(&self) -> TraceLimits {
        self.trace_limits
    }

    /// Build the canonical incoming context in the shared thread order.
    pub fn build_context<I, V>(&self, args: I) -> Result<TraceContext, ThreadStartupError>
    where
        I: IntoIterator<Item = V>,
        V: std::borrow::Borrow<SymbolicValue>,
    {
        let provided = collect_invocation_arguments(args, self.max_invocation_arguments)?;
        build_context_from_spec(
            &ThreadContextSpec {
                descriptor: &self.descriptor,
                captured_memory: &self.captured_memory,
                arguments_object: self.arguments_object.as_ref(),
                receiver: &self.receiver,
                incoming_stack: &self.incoming_stack,
                trace_limits: self.trace_limits,
            },
            &provided,
        )
    }

    /// Execute this prefix invocation. Rejected inputs/entries return `Err`
    /// before any step commits; dispatch boundaries are reported in the
    /// [`PrefixTraceReport`].
    pub fn invoke<I, V>(&self, args: I) -> Result<PrefixTraceReport, ThreadStartupError>
    where
        I: IntoIterator<Item = V>,
        V: std::borrow::Borrow<SymbolicValue>,
    {
        self.invoke_with_debug(args, TraceDebugOptions::disabled())
    }

    /// Execute this prefix invocation with live per-operation debug output.
    pub fn invoke_with_debug<I, V>(
        &self,
        args: I,
        debug: TraceDebugOptions,
    ) -> Result<PrefixTraceReport, ThreadStartupError>
    where
        I: IntoIterator<Item = V>,
        V: std::borrow::Borrow<SymbolicValue>,
    {
        let context = self.build_context(args)?;
        // A slot that is also initialized gets a new cell: not shared.
        let captured = self
            .captured_memory
            .iter()
            .filter(|(slot, _)| !self.descriptor.initial_slots.contains(slot))
            .map(|(slot, value)| (*slot, value.clone()))
            .collect();
        let sharing = execute::CellSharing {
            captures: self.captures.clone(),
            starts: self.starts.clone(),
            captured,
            contents: self.cell_contents.clone(),
            cells: self.cells.clone(),
            created: self.created.0.clone(),
            returns: self.returns.0.clone(),
            cell_types: self.returns.1.clone(),
            probe: false,
            overrides: self.overrides.clone(),
        };
        Ok(execute::run_prefix(
            self.preparation,
            self.entry,
            context,
            self.trace_limits,
            debug,
            &sharing,
        ))
    }
}
