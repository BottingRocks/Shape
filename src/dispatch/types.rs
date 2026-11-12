//! Public dispatch types. [`DispatchPreparation`] keeps its invariants private:
//! callers may only read validated state through getters, so a proof cannot be
//! silently swapped for a fabricated program after preparation.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    DecodedOperand, HandlerTemplateReport, JsOperandSeparationReport, OperandSpan,
    ProvenOperationTable, TraceReport,
};

/// A caller-supplied execution entry. Entry inference is deliberately not
/// performed from an outer kernel argument or an unrelated constructor call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchStart {
    pub pc: usize,
    pub state: usize,
}

/// Why a requested execution entry was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchEntryError {
    PcOutOfBounds { pc: usize, image_len: usize },
    StateOutOfBounds { state: usize, state_count: usize },
}

impl fmt::Display for DispatchEntryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PcOutOfBounds { pc, image_len } => {
                write!(f, "entry PC {pc} is outside the {image_len}-byte image")
            }
            Self::StateOutOfBounds { state, state_count } => write!(
                f,
                "entry state {state} is outside the {state_count} proven states"
            ),
        }
    }
}

impl std::error::Error for DispatchEntryError {}

/// The value a terminal dispatch completion produces from the shared VM state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionValue {
    /// Completion returns the value of the VM stack at `index`.
    Stack { index: u32 },
    /// Completion yields `undefined` (an explicit caller choice, never a
    /// silent default).
    Undefined,
}

/// The supported VM completion convention. Ordinary handler returns continue
/// the loop; only this explicit selector terminates a run, and it names its own
/// completion value policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionContract {
    SelectorSentinel {
        selector: u32,
        value: CompletionValue,
    },
}

impl CompletionContract {
    #[must_use]
    pub fn selector(self) -> u32 {
        match self {
            Self::SelectorSentinel { selector, .. } => selector,
        }
    }

    #[must_use]
    pub fn value(self) -> CompletionValue {
        match self {
            Self::SelectorSentinel { value, .. } => value,
        }
    }
}

/// Bounds for dispatch image/table traversal, preparation and execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchLimits {
    pub max_dispatch_steps: usize,
    pub max_image_bytes: usize,
    pub max_table_cells: usize,
    pub max_integer: u32,
}

impl Default for DispatchLimits {
    fn default() -> Self {
        Self {
            max_dispatch_steps: 10_000,
            max_image_bytes: 8 * 1024 * 1024,
            max_table_cells: 100_000,
            max_integer: 1_000_000,
        }
    }
}

/// One decoded dispatch-table cell: `[next_state, selector]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchCell {
    pub next_state: u32,
    pub selector: u32,
}

/// Why dispatch preparation was rejected.
#[derive(Debug)]
pub enum DispatchPrepareError {
    RuntimeConfig(String),
    MissingFact(&'static str),
    InvalidFact(String),
    Parse(String),
    SourceJoin(String),
    TableIdentity(String),
    /// A configured preparation bound was exceeded before any allocation.
    Limit(String),
}

impl fmt::Display for DispatchPrepareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeConfig(e) => write!(f, "runtime proof unavailable: {e}"),
            Self::MissingFact(kind) => write!(f, "missing extracted runtime fact: {kind}"),
            Self::InvalidFact(e) => write!(f, "invalid runtime fact: {e}"),
            Self::Parse(e) => write!(f, "dispatch proof parse failed: {e}"),
            Self::SourceJoin(e) => write!(f, "dispatch-to-handler join rejected: {e}"),
            Self::TableIdentity(e) => write!(f, "dispatch table identity rejected: {e}"),
            Self::Limit(e) => write!(f, "dispatch preparation limit: {e}"),
        }
    }
}

impl std::error::Error for DispatchPrepareError {}

/// A validated, proof-backed dispatch program. Fields are private and have no
/// mutating accessor: every boundary and size was checked in
/// [`prepare_dispatch_from_report`] before this value existed.
#[derive(Debug, Clone)]
pub struct DispatchPreparation {
    pub(super) source_hash: String,
    pub(super) artifact_source_hash: String,
    pub(super) bytecode: Vec<u8>,
    pub(super) table: Vec<Vec<DispatchCell>>,
    pub(super) templates: HandlerTemplateReport,
    pub(super) operands: JsOperandSeparationReport,
    /// Selector -> first position in `templates.handlers`, prepared once.
    pub(super) template_index: super::step::SelectorIndex,
    /// Selector -> first position in `operands.handlers`, prepared once.
    pub(super) operand_index: super::step::SelectorIndex,
    pub(super) start: DispatchStart,
    pub(super) completion: CompletionContract,
    pub(super) operation_table: ProvenOperationTable,
    pub(super) limits: DispatchLimits,
    /// Real source span of the proven dispatch consumer, used as the completion
    /// record's coordinate. Never a fabricated origin.
    pub(super) completion_span: OperandSpan,
    /// Extracted constant string tables registered on every trace session.
    pub(super) constant_tables: Vec<(String, std::sync::Arc<[String]>)>,
}

/// A validated invocation of an immutable [`DispatchPreparation`] at one VM
/// coordinate. The completion contract remains the one source-validated by
/// the preparation; callers can supply a different incoming [`TraceContext`]
/// without reparsing or rebuilding any proof-backed artifacts.
#[derive(Debug, Clone, Copy)]
pub struct DispatchInvocation<'a> {
    pub(super) preparation: &'a DispatchPreparation,
    pub(super) start: DispatchStart,
}

impl<'a> DispatchInvocation<'a> {
    /// The validated entry used by this invocation.
    #[must_use]
    pub fn start(&self) -> DispatchStart {
        self.start
    }

    /// The immutable preparation and its source-validated completion policy.
    #[must_use]
    pub fn preparation(&self) -> &'a DispatchPreparation {
        self.preparation
    }

    /// The completion policy inherited from the preparation.
    #[must_use]
    pub fn completion(&self) -> CompletionContract {
        self.preparation.completion
    }
}

impl DispatchPreparation {
    /// Identity of the caller-supplied source bytes (the report input).
    #[must_use]
    pub fn source_hash(&self) -> &str {
        &self.source_hash
    }

    /// Identity of the standardized source the facts were parsed from.
    #[must_use]
    pub fn artifact_source_hash(&self) -> &str {
        &self.artifact_source_hash
    }

    #[must_use]
    pub fn bytecode(&self) -> &[u8] {
        &self.bytecode
    }

    /// The proven dispatch image as `state -> opcode -> cell`.
    #[must_use]
    pub fn table(&self) -> &[Vec<DispatchCell>] {
        &self.table
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
    pub fn start(&self) -> DispatchStart {
        self.start
    }

    /// Validate and bind a new VM entry to this preparation.
    ///
    /// The returned invocation reuses the prepared source proof, dispatch
    /// image, table, operand descriptors, and handler recipes. Its context is
    /// supplied at execution time and must represent the state entering that
    /// coordinate; this is a continuation entry, not replay from the original
    /// start and not a way to infer missing mid-block state.
    pub fn with_entry(
        &self,
        start: DispatchStart,
    ) -> Result<DispatchInvocation<'_>, DispatchEntryError> {
        self.validate_entry(start)?;
        Ok(DispatchInvocation {
            preparation: self,
            start,
        })
    }

    pub(crate) fn validate_entry(&self, start: DispatchStart) -> Result<(), DispatchEntryError> {
        if start.pc >= self.bytecode.len() {
            return Err(DispatchEntryError::PcOutOfBounds {
                pc: start.pc,
                image_len: self.bytecode.len(),
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

    #[must_use]
    pub fn completion(&self) -> CompletionContract {
        self.completion
    }

    #[must_use]
    pub fn completion_selector(&self) -> u32 {
        self.completion.selector()
    }

    #[must_use]
    pub fn operation_table(&self) -> &ProvenOperationTable {
        &self.operation_table
    }

    #[must_use]
    pub fn limits(&self) -> DispatchLimits {
        self.limits
    }

    #[must_use]
    pub fn completion_span(&self) -> OperandSpan {
        self.completion_span.clone()
    }
}

/// One committed execution coordinate. Coordinates are committed only after the
/// handler succeeds (or throws terminally); a rolled-back failed attempt has no
/// record and leaves the preceding records/context intact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchExecutionRecord {
    pub dispatch_pc: usize,
    pub operand_entry_pc: usize,
    pub state_before: usize,
    pub state_after: usize,
    pub opcode: u8,
    pub handler_key: u32,
    pub operands: Vec<DecodedOperand>,
    pub source_span: OperandSpan,
    pub recipe_identity: String,
    pub actual_next_pc: usize,
    /// `Some` on the single terminal completion record; `None` for a handler
    /// occurrence. The completion record has no operands and never invents a
    /// stack read: its value policy lives in [`CompletionValue`].
    pub completion: Option<CompletionValue>,
}

/// Why the committed prefix ended. Only [`DispatchStopReason::CompletedSelector`]
/// is a full run; every other reason is an explicit partial/budget halt and is
/// mirrored by a non-`Completed` tracer completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchStopReason {
    CompletedSelector,
    TruncatedImage,
    MissingState,
    MissingOpcode,
    MissingHandler,
    UnsupportedHandler,
    Budget,
    RepeatedPcState,
    Thrown,
}

/// The committed prefix, its tracer report, and the stop reason.
#[derive(Debug, Clone, PartialEq)]
pub struct DispatchTraceReport {
    /// The validated entry used to produce this report.
    pub start: DispatchStart,
    pub records: Vec<DispatchExecutionRecord>,
    pub trace: TraceReport,
    pub stop: DispatchStopReason,
}

impl DispatchTraceReport {
    /// True only for a full, selector-terminated run whose completion value was
    /// committed. A truncated/budget/unsupported halt is never reported here.
    #[must_use]
    pub fn is_complete_completion(&self) -> bool {
        self.stop == DispatchStopReason::CompletedSelector
            && matches!(self.trace.completion, crate::TraceCompletion::Returned(_))
    }
}
