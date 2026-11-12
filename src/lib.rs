//! Shape-style VM deobfuscation, implemented entirely in Rust with Oxc.
//!
//! The pipeline is `CLEAN -> RENAME -> FIND -> EXTRACT`. No Babel, Node, npm, or
//! JavaScript parser subprocess is used anywhere; every AST is parsed, analyzed,
//! transformed and printed by the Oxc crates.

pub mod kernel;
pub mod operations;
pub mod naming;
pub(crate) mod transforms;

pub mod dispatch;
pub(crate) mod js_number;
pub mod oxc;
pub mod profile;
pub mod seeds;
pub mod signals;
pub mod sdk;
pub mod source;
pub mod trace;

/// The loaded decode artifact every SDK binding uses: see [`sdk::Generator`].
pub use sdk::Generator;
pub mod tracing;
#[doc(hidden)]
pub mod util;
pub mod vm_patterns;
pub mod vm_string;

// Keep the established public module paths as aliases of the canonical
// tracing modules. The symbolic and text event models remain distinct.
pub use tracing::{symbolic as tracer, text as trace_text};

// Keep the established public `shape::analysis` path as an alias of the
// canonical Oxc helper module. The implementation lives in `src/oxc.rs`.
pub use oxc as analysis;

// Keep the established public `shape::diagnostics` and `shape::emitted_cleanup`
// paths while their implementations live under `transforms`. The source-pass
// and post-emission pipelines stay separate inside `transforms`.
pub use transforms::diagnostics;
pub use transforms::emitted_js as emitted_cleanup;

// Keep the established public module paths as aliases of the canonical
// operation modules. Reexporting the modules preserves exact type identity.
pub use operations::{
    catalogue as operation_catalogue, executor as handler_executor, operands as js_operands,
    templates as handler_templates, view as ops_view,
};

// Keep the established public module paths as aliases of the canonical kernel
// modules. These are reexports, so their types remain identical.
pub use kernel::{
    bundle as tracer_bundle, config as runtime_config, decoder_proof as runtime_decoder,
    layout as kernel_layout, properties as actual_properties, roles, wasm as wasm_boundary,
    wasm_dispatch,
};

// Keep the established public module paths while grouping implementation files
// by responsibility under `transforms`.
pub use transforms::deob::{native_cleanup, string_decode, wasm_bindings};


pub use dispatch::{
    prepare_dispatch_from_report, prepare_prefix_from_report, prepare_prefix_from_saved_report,
    CapturePolicy, CompletionContract, CompletionValue, DescriptorRecord, DispatchCell,
    DispatchEntryError, DispatchExecutionRecord, DispatchInvocation, DispatchLimits,
    DispatchPreparation, DispatchPrepareError, DispatchStart, DispatchStopReason,
    DispatchTraceReport, EntryFacts, ImagePatchRecord, KernelArtifact, KernelArtifactCompileError,
    KernelArtifactError, PrefixBranch, PrefixBranchKind, PrefixExecutionRecord, PrefixLimits,
    PrefixPreparation, PrefixPrepareError, PrefixRetraces, PrefixStepEffect, PrefixStopReason,
    PrefixThreadInvocation, PrefixTraceReport, ThreadDescriptor, ThreadInputs, ThreadInvocation,
    ThreadStartupError, ARTIFACT_FORMAT, COMPILER_PROVENANCE, MAX_ARTIFACT_BYTES,
};
pub use kernel::bundle::{tracer_json, tracer_markdown};
pub use kernel::config::{
    extract as extract_runtime_config,
    extract_deobfuscated as extract_runtime_config_from_deobfuscated,
    from_report as runtime_config_from_report,
    from_saved_report as runtime_config_from_saved_report, ArtifactKind as RuntimeArtifactKind,
    ExtractionLimits as RuntimeExtractionLimits, FactStatus as RuntimeFactStatus,
    InputMode as RuntimeInputMode, RuntimeArtifact, RuntimeConfig, RuntimeConfigError,
    RuntimeConfigOptions,
};
pub use kernel::decoder_proof::{
    prove_runtime_decoder, prove_standard_decoder, DecoderProof, DecoderProofEvidence,
};
pub use kernel::layout::{
    analyze_with_options, layout_json, layout_markdown, layout_verbose_json,
    layout_verbose_markdown, report_with_options, static_kernel_layout_with_options,
    KernelLayoutOptions,
};
pub use kernel::properties::{
    extract_actual_properties, extract_actual_properties_with_limits, ActualProperties,
    ActualPropertiesError, EntryProperties, NamedValue, OperationSet, PropertyValue,
    SlotAssignment, SlotProperties,
};
pub use kernel::wasm::{
    analyze_report as analyze_wasm_boundary_report, analyze_source as analyze_wasm_boundary,
    boundary_json, boundary_markdown, WasmBoundaryError, WasmBoundaryReport,
};
pub use operations::catalogue::{catalogue_from_report, catalogue_json, catalogue_markdown};
pub use operations::classify::{
    classification_json, classification_markdown, classify_report, classify_seeds,
    classify_template, ClassifiedOp, KindSummary, NamedSeed, OpNature, OpScope, SeedClassification,
    VmVariant,
};
pub use operations::executor::{
    execute_handler, HandlerCompletion, HandlerContext, HandlerExecution, HandlerExecutionError,
    HandlerExecutionLimits, HandlerExecutor, HandlerObservation, JsValue, SlotChange, SlotState,
    StackChange, DEFAULT_MAX_SLOTS, DEFAULT_MAX_STACK_LENGTH, DEFAULT_MAX_STEPS,
};
pub use operations::graph::{
    Effect, EffectKind, GraphProjectionError, GraphValidationError, HandlerGraph,
    HandlerGraphReport, HandlerIR, HandlerIRReport, LocationKey, Occurrence, OccurrenceId,
    OccurrenceOp, Provenance, SpanGranularity, UnknownRegion, UnknownRegionKind,
    GRAPH_SCHEMA_VERSION, MAX_PROJECTED_VALUE_DEPTH, MAX_PROJECTED_VALUE_NODES,
    PRIMITIVE_REGISTRY_VERSION,
};
pub use operations::operands::{
    bounded_decode_operands, js_operand_separation_from_report,
    js_operand_separation_from_report_with_handoff,
    js_operand_separation_from_report_with_handoff_and_templates, CapturedDependency,
    CapturedDependencyRole, DecodeOperandsError, DecodedOperand, JsOperandBindingHandoff,
    JsOperandSeparationError, JsOperandSeparationReport, LiftedHandler, OperandCoverage,
    OperandDescriptor, OperandEndian, OperandEntryCoordinate, OperandHandlerStatus,
    OperandInputIdentity, OperandSpan, SCHEMA_VERSION as JS_OPERAND_SEPARATION_SCHEMA_VERSION,
};
pub use operations::templates::{
    admit_recipe, handler_graphs_from_report, handler_ir_from_report,
    handler_templates_and_graphs_from_report, handler_templates_from_report, AdmissionError,
    ClosureExpression, ClosurePlan, ClosureStatement, HandlerTemplate, HandlerTemplateError,
    HandlerTemplateReport, LiteralValue, PrimitiveOperator, RecipeBounds, RecipeOperation,
    RecipeStep, SemanticValue, SpanRecord, TemplateCoverage, TemplateStatus, UnknownFragment,
    DEFAULT_MAX_RECIPE_EXPRESSION_DEPTH, DEFAULT_MAX_RECIPE_EXPRESSION_WORK,
    DEFAULT_MAX_RECIPE_STEPS,
};
pub use operations::templates::{CallFrameField, ErrorRecordField, ReceiverField, TryFrameField};
pub use tracing::symbolic::{
    emit_javascript, trace_handler, trace_handler_with_debug, trace_handlers,
    trace_handlers_with_debug, BranchEvidence, BranchOverrides, DecodedInstruction, EmitError,
    EmitOptions, EmittedJavaScript, EmittedStatementMap, ErrorRecord, PendingBranch,
    SubroutineFrame, SubroutineSummary, SymbolicValue, TraceArena, TraceCell, TraceCheckpoint,
    TraceCompletion, TraceContext, TraceContextError, TraceDebugOptions, TraceDebugStatus,
    TraceDiagnostic, TraceEvent, TraceEventKind, TraceFinishError, TraceFork, TraceHandlerResult,
    TraceJoinError, TraceLimits, TraceLoop, TraceNode, TraceNodeId, TraceNodeKind, TracePath,
    TraceProvenance, TraceReport, TraceSession, TraceStateSnapshot, TraceStatement,
    TraceStatementId, TraceStatementKind, TraceStopCode, TraceValue, TryFrame, VMInstruction,
};
pub use tracing::symbolic::{
    emit_program_function, emit_program_function_in_namespace, ProgramFacts, ProgramThread,
    PROGRAM_SELF, PROGRAM_THIS,
};
pub use tracing::symbolic::{
    CallFrame, CellContent, CellRead, CellReads, ControlTransfer, StopMarks, ValueDispatch,
    VmCursor,
};
pub use transforms::deob::native_cleanup::{
    NativeCleanupPolicy, NativeCleanupReport, NativeCleanupResult,
};
pub use transforms::deob::pipeline::{
    decode_base64, decode_xor, deobfuscate, deobfuscate_with_report, write_deobfuscated_file,
    write_deobfuscated_file_to, write_deobfuscated_file_to_force, write_deobfuscated_source_to,
    write_deobfuscated_source_to_force, BindingMapping, ExtractionFact, ExtractionKind,
    ExtractionStatus, PipelineError, PipelineOptions, PipelineReport, PropertyMapping,
    ProvenOperationTable, StageReport, StaticValue,
};
pub use transforms::deob::string_decode::{DecodeOutcome, DecodeReport};
pub use vm_string::{fold_emitted_vm_strings_from_plan, fold_emitted_vm_strings_from_report};
pub use vm_string::{VmStringFoldPlan, VM_STRING_FOLD_PLAN_VERSION};

/// Stack size for [`run_with_large_stack`]. Oxc's parser and this crate's
/// visitors recurse over the AST, so deep but accepted input needs more than
/// the default main-thread stack. The memory is reserved, not committed.
pub const LARGE_STACK_BYTES: usize = 256 * 1024 * 1024;

/// Run `f` on a worker thread with a [`LARGE_STACK_BYTES`] stack and return its
/// result. A panic in `f` is resumed on the calling thread.
pub fn run_with_large_stack<T, F>(f: F) -> T
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    std::thread::Builder::new()
        .name("shape".into())
        .stack_size(LARGE_STACK_BYTES)
        .spawn(f)
        .expect("failed to spawn the large-stack worker thread")
        .join()
        .unwrap_or_else(|payload| std::panic::resume_unwind(payload))
}
