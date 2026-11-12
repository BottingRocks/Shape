//! Public data types for the bounded JavaScript VM prefix contract.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{
    DecodedOperand, DispatchEntryError, DispatchStart, OperandSpan, TraceNodeId, TraceReport,
};

/// Why the committed prefix ended. Every non-`Completed` reason is an explicit
/// partial boundary; none is reported as a full run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrefixStopReason {
    /// The requested dispatch-step budget was reached with no boundary. This
    /// slice models no terminal completion register, so a clean budget halt is
    /// the only non-boundary stop shape.
    Budget,
    TruncatedImage {
        pc: usize,
        image_len: usize,
    },
    TruncatedOperand {
        operand_entry_pc: usize,
        needed: usize,
        available: usize,
    },
    MissingState {
        state: usize,
    },
    MissingOpcode {
        state: usize,
        opcode: u8,
    },
    AddressOutOfBounds {
        address: u64,
        image_len: usize,
    },
    UnsupportedHandler {
        selector: u32,
        reason: String,
    },
    RepeatedPcState {
        pc: usize,
        state: usize,
    },
    /// Every path ended, by returning (`vmPendingResult = value`) or by
    /// coming back to a loop header. This is the one complete outcome: the
    /// trace emits as a function.
    Completed,
    /// The two branches of a conditional jump reached the same position with
    /// VM state that cannot be merged.
    UnjoinableBranches {
        pc: usize,
        state: usize,
        reason: String,
    },
}

impl fmt::Display for PrefixStopReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Budget => f.write_str("dispatch step budget reached"),
            Self::TruncatedImage { pc, image_len } => {
                write!(f, "dispatch PC {pc} is outside the {image_len}-byte image")
            }
            Self::TruncatedOperand {
                operand_entry_pc,
                needed,
                available,
            } => write!(
                f,
                "handler operands at {operand_entry_pc} need {needed} bytes but only {available} remain"
            ),
            Self::MissingState { state } => write!(f, "state {state} has no proven table row"),
            Self::MissingOpcode { state, opcode } => {
                write!(f, "state {state} opcode {opcode} has no proven dispatch cell")
            }
            Self::AddressOutOfBounds { address, image_len } => write!(
                f,
                "bytecode write address {address} is outside the {image_len}-byte image"
            ),
            Self::UnsupportedHandler { selector, reason } => {
                write!(f, "selector {selector} is unsupported: {reason}")
            }
            Self::RepeatedPcState { pc, state } => {
                write!(f, "dispatch revisited pc {pc} state {state} without progress")
            }
            Self::Completed => {
                f.write_str("every path returned or continued a loop: the function is complete")
            }
            Self::UnjoinableBranches { pc, state, reason } => {
                write!(f, "branches cannot join at pc {pc} state {state}: {reason}")
            }
        }
    }
}

/// Bounds for prefix preparation and execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefixLimits {
    pub max_dispatch_steps: usize,
    pub max_image_bytes: usize,
    pub max_table_cells: usize,
    pub max_integer: u32,
    /// Trace each VM subroutine once as a function (on by default); off,
    /// every call is traced inline.
    pub outline_subroutines: bool,
    /// Check the walk's shortcuts against tracing without them, and report
    /// every difference in [`PrefixTraceReport::self_check`]. Costly.
    pub self_check: bool,
    /// Step a fork's sides in the other order wherever the walk chooses
    /// which goes first. A walk whose result depends on that order is
    /// wrong; the corpus self-check walks both ways and compares.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reverse_sides: bool,
    /// Walk the side every branch decided fake by test rules out, alone,
    /// undone after, and report in [`PrefixTraceReport::fake_audits`]
    /// whether it reaches a state real code never reaches.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub audit_fakes: bool,
}

impl Default for PrefixLimits {
    fn default() -> Self {
        Self {
            max_dispatch_steps: 10_000,
            max_image_bytes: 8 * 1024 * 1024,
            max_table_cells: 100_000,
            max_integer: 1_000_000,
            outline_subroutines: true,
            self_check: false,
            reverse_sides: false,
            audit_fakes: false,
        }
    }
}

/// One committed image-byte write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImagePatchRecord {
    pub address: u32,
    pub previous: u8,
    pub value: u8,
}

/// The concrete effect of one committed dispatch step.
#[derive(Debug, Clone, PartialEq)]
pub enum PrefixStepEffect {
    /// A recognized bounded bytecode-image write applied to the invocation's
    /// private image clone. This is an internal VM effect, not emitted
    /// JavaScript.
    ImagePatch(ImagePatchRecord),
    /// A supported ordinary handler executed through the shared recipe
    /// [`TraceSession`].
    Handler {
        recipe_identity: std::sync::Arc<str>,
        operands: Vec<DecodedOperand>,
        /// What the walk made of a conditional jump on a program value.
        branch: Option<PrefixBranch>,
    },
}

/// A conditional jump whose test is a program value, and what it turned out
/// to be.
#[derive(Debug, Clone, PartialEq)]
pub struct PrefixBranch {
    /// Where the jump goes when its test holds; the fall-through is the
    /// record's `actual_next_pc`.
    pub target: DispatchStart,
    /// The test's trace node.
    pub test: TraceNodeId,
    pub kind: PrefixBranchKind,
}

/// Whether a conditional jump is a real `if` or a fake branch, and why.
#[derive(Debug, Clone, PartialEq)]
pub enum PrefixBranchKind {
    /// A real `if`: both successors are valid code, traced as its arms.
    Real,
    /// A fake branch decided from what is known about the test's operands.
    /// Only the `taken` side is traced.
    FakeByTest { taken: bool },
    /// A fake branch whose other side reaches a state real code never
    /// reaches (`reason`). Only the `taken` side is traced.
    FakeByContext { taken: bool, reason: String },
}

/// One committed dispatch step.
#[derive(Debug, Clone, PartialEq)]
pub struct PrefixExecutionRecord {
    pub step: usize,
    pub dispatch_pc: usize,
    pub operand_entry_pc: usize,
    pub state_before: usize,
    pub state_after: usize,
    pub opcode: u8,
    pub selector: u32,
    pub actual_next_pc: usize,
    pub source_span: OperandSpan,
    pub effect: PrefixStepEffect,
}

/// The committed prefix, the per-invocation image result, and the stop reason.
#[derive(Debug, Clone, PartialEq)]
pub struct PrefixTraceReport {
    pub start: DispatchStart,
    pub records: Vec<PrefixExecutionRecord>,
    pub patches: Vec<ImagePatchRecord>,
    pub trace: TraceReport,
    pub stop: PrefixStopReason,
    pub image_len: usize,
    /// FNV-1a 64 digest of the invocation's final (possibly mutated) image.
    /// Where `image_divergence` is not empty, of one history's image.
    pub image_digest: String,
    /// Bytecode addresses the merged histories may still disagree on at the
    /// end: a fork's sides patched them differently and no later patch
    /// every history makes rewrote them.
    pub image_divergence: Vec<u32>,
    /// What this function put in each shared memory cell (by slot): the VM
    /// addresses it wrote and whether it wrote JavaScript values; `None`
    /// for a cell it gave a VM value it does not know. A driver that gathers
    /// these from every function holding a cell knows what the cell holds.
    pub cell_writes: std::collections::BTreeMap<u32, Option<crate::CellContent>>,
    /// The JavaScript types (predicate type bits) this function put in each
    /// shared cell, by slot: every type where it put a value of a type it
    /// does not know (see [`crate::tracing::symbolic::CellTypesHook`]).
    pub cell_types: std::collections::BTreeMap<u32, u16>,
    /// The slots the walk traced as shared cells: those it was given and
    /// those its closures capture.
    pub cells: std::collections::BTreeSet<u32>,
    /// What the walk used of the contents it was told the cells hold: a
    /// walk told other contents the same of these traces the same.
    pub cell_reads: crate::CellReads,
    /// The recognized bytecode-write handler's selector.
    pub patch_selector: u32,
    /// How often the walk traced work again, and why.
    pub retraces: PrefixRetraces,
    /// With [`PrefixLimits::self_check`], every place a shortcut gave a
    /// different result than tracing without it. Empty means it held.
    pub self_check: Vec<String>,
    /// With [`PrefixLimits::audit_fakes`], for each position of a branch
    /// decided fake by test, what walking the side it rules out found.
    pub fake_audits: std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
    /// The dispatch steps the walk ran and then threw away, by what threw
    /// them away.
    pub wasted_steps: std::collections::BTreeMap<&'static str, usize>,
    /// Where each `try` the walk entered goes when it catches.
    pub try_handlers: Vec<DispatchStart>,
}

/// What made the walk trace work again: each count is one region (a fork,
/// loop, `try` or the whole walk) traced again, except `dispatches`, which
/// counts jumps that branched on a variable's value set.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PrefixRetraces {
    /// A path came back to a position it passed: a loop header.
    pub new_loops: usize,
    /// A loop iteration changed a cell that was not a loop variable yet.
    pub loop_variables: usize,
    /// A loop variable gained a value after a jump took one from its set.
    pub loop_values: usize,
    /// A `try` body changed a cell its catch reads, or gave a `try`
    /// variable a value after the catch took one from its set.
    pub try_variables: usize,
    /// A fork's sides met where neither was expected to.
    pub meeting_points: usize,
    /// A join merged values a later jump needed apart.
    pub splits: usize,
    /// A test or jump used what an outlined call returned.
    pub inline_calls: usize,
    /// A `break` turned out to be a path still inside its loop.
    pub wrong_breaks: usize,
    /// Jumps that branched on a variable's value set.
    pub dispatches: usize,
    /// Positions where a fork's sides met in VM states that cannot merge
    /// and stepped on apart.
    pub unmergeable: usize,
    /// A created closure captured slots that were not shared cells yet.
    pub cells: usize,
}

impl PrefixRetraces {
    /// Count `other`'s retraces too.
    pub fn add(&mut self, other: &Self) {
        let Self {
            new_loops,
            loop_variables,
            loop_values,
            try_variables,
            meeting_points,
            splits,
            inline_calls,
            wrong_breaks,
            dispatches,
            unmergeable,
            cells,
        } = other;
        self.new_loops += new_loops;
        self.loop_variables += loop_variables;
        self.loop_values += loop_values;
        self.try_variables += try_variables;
        self.meeting_points += meeting_points;
        self.splits += splits;
        self.inline_calls += inline_calls;
        self.wrong_breaks += wrong_breaks;
        self.dispatches += dispatches;
        self.unmergeable += unmergeable;
        self.cells += cells;
    }

    /// The regions traced again, all reasons together.
    #[must_use]
    pub fn total(&self) -> usize {
        self.new_loops
            + self.loop_variables
            + self.loop_values
            + self.try_variables
            + self.meeting_points
            + self.splits
            + self.inline_calls
            + self.wrong_breaks
            + self.cells
    }
}

impl PrefixTraceReport {
    /// The last diagnostic message recorded for the stop, if any. Diagnostics
    /// are recorded in order, so this is the message that explains why the
    /// walk stopped.
    #[must_use]
    pub fn stop_detail(&self) -> Option<&str> {
        self.trace
            .diagnostics
            .last()
            .map(|diagnostic| diagnostic.message.as_str())
    }
}

/// Why prefix preparation was rejected.
#[derive(Debug)]
pub enum PrefixPrepareError {
    RuntimeConfig(String),
    MissingFact(&'static str),
    InvalidFact(String),
    Parse(String),
    SourceProof(String),
    Limit(String),
    Thread(DispatchEntryError),
}

impl fmt::Display for PrefixPrepareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeConfig(error) => write!(f, "runtime proof unavailable: {error}"),
            Self::MissingFact(kind) => write!(f, "missing extracted runtime fact: {kind}"),
            Self::InvalidFact(error) => write!(f, "invalid runtime fact: {error}"),
            Self::Parse(error) => write!(f, "prefix proof parse failed: {error}"),
            Self::SourceProof(error) => write!(f, "prefix source proof rejected: {error}"),
            Self::Limit(error) => write!(f, "prefix preparation limit: {error}"),
            Self::Thread(error) => write!(f, "prefix entry rejected: {error}"),
        }
    }
}

impl std::error::Error for PrefixPrepareError {}

impl From<DispatchEntryError> for PrefixPrepareError {
    fn from(error: DispatchEntryError) -> Self {
        Self::Thread(error)
    }
}

impl From<super::super::types::DispatchPrepareError> for PrefixPrepareError {
    fn from(error: super::super::types::DispatchPrepareError) -> Self {
        Self::SourceProof(error.to_string())
    }
}
