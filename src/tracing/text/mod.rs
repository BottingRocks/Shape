//! Deterministic plain-text rendering for producer-supplied trace observations.
//!
//! This module deliberately does not execute JavaScript, evaluate expressions, build a CFG,
//! or infer missing facts. `Trace` is a small typed observation boundary: producers retain
//! control, call, value, and provenance facts, and this module validates their structure before
//! presenting them as a readable trail plus a line-accurate details map.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
};

pub type EventId = u32;
pub type ScopeId = u32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceHeader {
    pub sample: String,
    pub mode: Mode,
    pub scope: String,
    pub capture: String,
    pub illustrative: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    RuntimeObserved,
    StaticDerived,
    Symbolic,
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::RuntimeObserved => "runtime observed",
            Self::StaticDerived => "static derived",
            Self::Symbolic => "symbolic",
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope {
    pub id: ScopeId,
    pub thread: u32,
    pub call: u32,
    pub parent: Option<ScopeId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TraceEvent {
    pub id: EventId,
    pub scope: ScopeId,
    pub evidence: Evidence,
    pub provenance: Provenance,
    pub kind: EventKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Evidence {
    Observed,
    StaticDerived,
    Assumed { branch: EventId, label: String },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Provenance {
    pub handler_key: Option<String>,
    pub micro_op_ordinal: Option<u32>,
    pub source: Option<SourceSpan>,
    pub ir_revision: Option<String>,
    pub ir_op: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceSpan {
    pub artifact: String,
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EventKind {
    Input {
        label: String,
        value: Value,
    },
    Read {
        expression: String,
        value: Value,
    },
    Compute {
        expression: String,
        result: Value,
    },
    Test {
        expression: String,
        result: TestResult,
    },
    BranchStart {
        label: String,
        status: BranchStatus,
    },
    BranchEnd {
        exit: BranchExit,
    },
    CallStart {
        callee: String,
        invocation: u32,
        child_scope: ScopeId,
        capture: CaptureClaim,
    },
    CallEnd {
        completion: CallCompletion,
    },
    Resume {
        caller_scope: ScopeId,
        result: Value,
    },
    Change {
        target: String,
        before: Value,
        after: Value,
    },
    Return {
        value: Value,
        disposition: ReturnDisposition,
    },
    Throw {
        error: String,
        destination: String,
    },
    CatchStart {
        label: String,
    },
    CatchEnd,
    FinallyStart {
        pending: PendingCompletion,
    },
    FinallyEnd {
        completion: FinallyCompletion,
    },
    Capture {
        thread: String,
        cell: String,
        identity_evidence: String,
    },
    OpaqueStop {
        reason: String,
    },
    LoopStart {
        label: String,
    },
    IterationStart {
        ordinal: u32,
    },
    IterationEnd,
    LoopExit {
        expression: String,
        result: bool,
        reason: String,
    },
    LoopEnd {
        exit: String,
    },
    NoUnconditionalResult {
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TestResult {
    Known { value: bool, reason: String },
    Unknown { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BranchStatus {
    Observed {
        outcome: bool,
        alternate: String,
    },
    Assumed {
        outcome: bool,
        assumption: String,
    },
    ProvenDead {
        dead_edge: String,
        proof: String,
        evidence_ref: String,
    },
    Suspected {
        outcome: bool,
        target: String,
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BranchExit {
    ContinueAfterProvenJoin,
    Returned,
    Stopped(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum CaptureClaim {
    NoCapture,
    CapturedCell {
        cell: String,
        identity_evidence: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum CallCompletion {
    Returned(Value),
    Threw(String),
    Stopped { reason: String },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ReturnDisposition {
    Complete,
    PendingFinally,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PendingCompletion {
    Return(Value),
    Throw(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum FinallyCompletion {
    Completed(PendingCompletion),
    ReplacedByReturn(Value),
    ReplacedByThrow(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Undefined,
    Null,
    Uninitialized,
    Unavailable(String),
    Bool(bool),
    I32(i32),
    I64(i64),
    Number(f64),
    String(String),
    Reference(String),
    Preview {
        display: String,
        full_ref: Option<String>,
        size_bytes: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_events: usize,
    pub max_scopes: usize,
    pub max_nesting: usize,
    pub max_field_bytes: usize,
    pub max_value_preview_bytes: usize,
    pub max_output_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_events: 4096,
            max_scopes: 1024,
            max_nesting: 64,
            max_field_bytes: 16 * 1024,
            max_value_preview_bytes: 256,
            max_output_bytes: 512 * 1024,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineDetail {
    pub line_start: usize,
    pub line_end: usize,
    pub event_ids: Vec<EventId>,
    pub provenance: Provenance,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedTrace {
    pub text: String,
    pub details: String,
    pub line_map: Vec<LineDetail>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TraceError {
    LimitExceeded { what: &'static str, limit: usize },
    OutputLimitExceeded { limit: usize },
    FieldTooLong { field: &'static str, limit: usize },
    DuplicateEventId { id: EventId },
    DuplicateScopeId { id: ScopeId },
    UnknownScope { event: EventId, scope: ScopeId },
    InvalidScopeParent { scope: ScopeId, parent: ScopeId },
    ScopeCycle { scope: ScopeId },
    UnknownEventReference { event: EventId, reference: EventId },
    InvalidBranchStructure { event: EventId, reason: String },
    InvalidCallStructure { event: EventId, reason: String },
    InvalidFinallyStructure { event: EventId, reason: String },
    InvalidLoopStructure { event: EventId, reason: String },
    EventAfterOpaqueStop { event: EventId, scope: ScopeId },
    MissingProofEvidence { event: EventId },
    UnsupportedEvidence { event: EventId, reason: String },
}

impl fmt::Display for TraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitExceeded { what, limit } => write!(f, "{what} exceeds limit {limit}"),
            Self::OutputLimitExceeded { limit } => {
                write!(f, "rendered output exceeds limit {limit} bytes")
            }
            Self::FieldTooLong { field, limit } => write!(f, "{field} exceeds limit {limit} bytes"),
            Self::DuplicateEventId { id } => write!(f, "duplicate event id {id}"),
            Self::DuplicateScopeId { id } => write!(f, "duplicate scope id {id}"),
            Self::UnknownScope { event, scope } => {
                write!(f, "event {event} references unknown scope {scope}")
            }
            Self::InvalidScopeParent { scope, parent } => {
                write!(f, "scope {scope} references unknown parent {parent}")
            }
            Self::ScopeCycle { scope } => write!(f, "scope {scope} has a parent cycle"),
            Self::UnknownEventReference { event, reference } => {
                write!(f, "event {event} references unknown event {reference}")
            }
            Self::InvalidBranchStructure { event, reason } => {
                write!(f, "invalid branch at event {event}: {reason}")
            }
            Self::InvalidCallStructure { event, reason } => {
                write!(f, "invalid call at event {event}: {reason}")
            }
            Self::InvalidFinallyStructure { event, reason } => {
                write!(f, "invalid finally at event {event}: {reason}")
            }
            Self::InvalidLoopStructure { event, reason } => {
                write!(f, "invalid loop at event {event}: {reason}")
            }
            Self::EventAfterOpaqueStop { event, scope } => {
                write!(f, "event {event} follows an opaque stop in scope {scope}")
            }
            Self::MissingProofEvidence { event } => write!(
                f,
                "proven-dead branch event {event} has no retained proof and evidence reference"
            ),
            Self::UnsupportedEvidence { event, reason } => {
                write!(f, "unsupported evidence at event {event}: {reason}")
            }
        }
    }
}

impl std::error::Error for TraceError {}

#[derive(Clone, Debug)]
pub struct Trace {
    pub header: TraceHeader,
    pub scopes: Vec<Scope>,
    pub events: Vec<TraceEvent>,
    pub limits: Limits,
}

impl Trace {
    pub fn new(header: TraceHeader, scopes: Vec<Scope>, events: Vec<TraceEvent>) -> Self {
        Self {
            header,
            scopes,
            events,
            limits: Limits::default(),
        }
    }

    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    pub fn validate(&self) -> Result<(), TraceError> {
        validate_text(&self.header.sample, "sample", &self.limits)?;
        validate_text(&self.header.scope, "scope", &self.limits)?;
        validate_text(&self.header.capture, "capture", &self.limits)?;
        if self.scopes.len() > self.limits.max_scopes {
            return Err(TraceError::LimitExceeded {
                what: "scope count",
                limit: self.limits.max_scopes,
            });
        }
        if self.events.len() > self.limits.max_events {
            return Err(TraceError::LimitExceeded {
                what: "event count",
                limit: self.limits.max_events,
            });
        }

        let mut scopes = HashMap::new();
        for scope in &self.scopes {
            if scopes.insert(scope.id, scope).is_some() {
                return Err(TraceError::DuplicateScopeId { id: scope.id });
            }
        }
        if !scopes.contains_key(&0) {
            return Err(TraceError::UnknownScope { event: 0, scope: 0 });
        }
        validate_scope_graph(&scopes)?;

        let mut ids = HashMap::new();
        for event in &self.events {
            if ids.insert(event.id, event).is_some() {
                return Err(TraceError::DuplicateEventId { id: event.id });
            }
            if !scopes.contains_key(&event.scope) {
                return Err(TraceError::UnknownScope {
                    event: event.id,
                    scope: event.scope,
                });
            }
            validate_event_fields(event, &self.limits)?;
            validate_provenance(event.id, &event.provenance, &self.limits)?;
        }

        validate_references(&ids)?;
        validate_structure(&self.events, &scopes, &self.limits, &ids)?;
        Ok(())
    }

    pub fn render(&self) -> Result<RenderedTrace, TraceError> {
        self.validate()?;
        Renderer::new(self).render()
    }
}

mod render;
mod validate;

use render::Renderer;
use validate::*;
