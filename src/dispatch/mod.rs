//! Proof-backed bounded JavaScript dispatch execution.
//!
//! The driver consumes the existing runtime-config, operand-separation and
//! handler-template reports. Canonical names and report strings are never
//! execution authority: the dispatch/operation join is re-proved against Oxc
//! `SymbolId` references and the proven operation-table declaration identity in
//! the standardized artifact. Preparation is a validated, read-only value; the
//! caller supplies only the execution entry and the completion convention.

mod execute;
pub(crate) mod prefix;
mod prepare;
mod proof;
mod step;
mod thread;
mod types;

pub use prefix::{
    prepare_prefix_from_report, prepare_prefix_from_saved_report, DescriptorRecord, EntryFacts,
    ImagePatchRecord, KernelArtifact, KernelArtifactCompileError, KernelArtifactError,
    PrefixBranch, PrefixBranchKind, PrefixExecutionRecord, PrefixLimits, PrefixPreparation,
    PrefixPrepareError, PrefixRetraces, PrefixStepEffect, PrefixStopReason, PrefixThreadInvocation,
    PrefixTraceReport, ARTIFACT_FORMAT, COMPILER_PROVENANCE, MAX_ARTIFACT_BYTES,
};
pub use prepare::prepare_dispatch_from_report;
pub use thread::{
    CapturePolicy, ThreadDescriptor, ThreadInputs, ThreadInvocation, ThreadStartupError,
};
pub use types::{
    CompletionContract, CompletionValue, DispatchCell, DispatchEntryError, DispatchExecutionRecord,
    DispatchInvocation, DispatchLimits, DispatchPreparation, DispatchPrepareError, DispatchStart,
    DispatchStopReason, DispatchTraceReport,
};
