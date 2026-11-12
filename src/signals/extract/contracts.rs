//! Generator-ready signal contracts: the ordered grammar every producer emits
//! and the provenance of every value it writes.
//!
//! [`extract_signal_contracts`] composes the source-derived passes that
//! already exist (encoding helpers, collector wiring, signal layouts) into one
//! report per descriptor of the supplied capture:
//!
//! * **Producer** — whether the descriptor is served by a group body row or a
//!   direct constructor, with the body's exact span. Counts (descriptors,
//!   groups, row lengths, direct descriptors) are derived from this capture.
//! * **Grammar** — a typed, ordered [`GrammarNode`] tree per `observer.next`
//!   site in JavaScript evaluation order: the raw 4-byte prefix with its
//!   spelling, `NUMBER` / `STRING` writes, logical single `BYTE` writes with
//!   the mask split from the value, `SEQUENCE`, `CHOICE` (with its branch kind
//!   and condition), `REPEAT` (with loop kind and direction), buffers handed to
//!   unmodelled calls (`Opaque`), and explicit `Unresolved` nodes. A depth or
//!   budget limit produces a visible `Unresolved`, never a silent drop.
//! * **Lifecycle** — `error` / `complete` sites of the same emitter, kept apart
//!   from the emitted body.
//! * **Provenance** — for every written value, a bounded backward walk over
//!   the value expression and its definitions, classified as source literal,
//!   source table, per-message randomness, per-session/shared state, host
//!   observation, derived computation, loop iteration or opaque external, with
//!   the transforms applied on the way and the exact source spans.
//! * **Accounting** — every write to an emitted buffer in the producer is
//!   either in some emission's grammar or listed as excluded with a reason.
//! * **Status** — `complete`, `partial` or `unsupported`, with concrete
//!   reasons. A record existing is not completeness.
//!
//! # Identity rules
//!
//! Nothing here recognizes a signal by name, key, salt, row number, descriptor
//! index, atlas id, offset or literal value. The descriptor index is reported
//! because the capture's own registry defines it; it is a capture-local join,
//! not a recognition input. Helper identities come from the encoding pass,
//! routes from the wiring pass.
//!
//! # Source versus host
//!
//! The report describes what the source does. It never executes the program
//! and never reports a runtime value. A numeric literal is reported as a
//! literal with unproven origin: a tracer can fold a host measurement into a
//! literal, so a literal alone does not prove generator configuration (see
//! [`LiteralOrigin`]). [`pair_trace_folded_literals`] can upgrade a literal to
//! a trace-folded observation only when a second trace of the same build
//! computes the same field from a host source.

use std::collections::{BTreeSet, HashMap};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_parser::{ParseOptions, Parser};
use oxc_semantic::{AstNodes, NodeId, Scoping, SemanticBuilder, SymbolId};
use oxc_span::{GetSpan, SourceType, Span};
use oxc_syntax::operator::{AssignmentOperator, UnaryOperator};
use oxc_syntax::scope::ScopeFlags;
use serde::{Deserialize, Serialize};

use super::encoding::EncodingProperties;
use super::inputs::{extract_program_inputs, ProgramInputError, ProgramInputReport};
use super::reaching;
use crate::signals::extract::layout::{span_serde, BufferAccounting, EmissionVariant, ObserverRoute};
use super::wiring::{
    CallbackInvocationWiring, EmitterMethod, LifecycleRole, LifecycleWiring, WiringProperties,
};
use crate::signals::schema::{SourceIdentity, SourceSpan};

/// Version of the serialized [`SignalContractReport`].
pub const SIGNAL_CONTRACT_SCHEMA_VERSION: u32 = 1;

// ===========================================================================
// Grammar IR
// ===========================================================================

/// The control construct a [`GrammarNode::Choice`] was built from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChoiceKind {
    /// `if (test) A else B`; a missing `else` is an empty arm.
    If,
    /// `test ? A : B`.
    Conditional,
    /// `left && right`, `left || right`, `left ?? right`: the right operand
    /// runs only when the left does not decide.
    Logical,
    /// `try { A } catch { B }`; a `finally` block is appended to both arms.
    TryCatch,
    /// `switch` cases, one arm per case body.
    Switch,
}

/// Iteration order of a loop over its counter, derived from the loop's own
/// test and update. `Unknown` when no single counter is proven.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopDirection {
    Ascending,
    Descending,
    /// `for…in` / `for…of`: the iteration order of the iterated value.
    Iteration,
    #[default]
    Unknown,
}

/// How the four raw prefix bytes reach the buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefixSpelling {
    /// `buf = [w >>> 24, w >> 16 & 255, w >> 8 & 255, w & 255]`.
    ArrayLiteral,
    /// `buf[0] = …; buf[1] = …; buf[2] = …; buf[3] = …` (possibly into a
    /// reused buffer).
    IndexStores,
    /// The prefix binding was established outside the replayed path (an
    /// enclosing scope), so its position is the buffer's start.
    Inherited,
}

/// How one logical value is masked on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaskForm {
    /// XORed with the next output of the emission's PRNG stream.
    Stream,
    /// XORed with a static literal only.
    StaticXor,
    /// Written as is (prefix bytes, unmasked stores).
    Unmasked,
    /// A mask is applied but its stream was not resolved.
    Unresolved,
}

/// One value-carrying write.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WriteNode {
    /// The whole write (writer call or indexed store).
    #[serde(with = "span_serde")]
    pub span: Span,
    /// The logical value expression, without the mask.
    #[serde(with = "span_serde")]
    pub value_span: Span,
    pub value: String,
    /// The value after resolving a plain local read at this write to its latest
    /// assigned expression (`$KEY_40` -> `navigator.userAgent`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<String>,
    pub mask: MaskForm,
    /// The stream argument or mask callee text, when masked by a stream.
    pub mask_source: Option<String>,
    /// Factory-call identity of the captured stream (shared cursor scope).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_instance: Option<SourceSpan>,
    /// A source-proven draw retained in a scalar before its byte store.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_draw: Option<SourceSpan>,
    /// Source binding of the concrete stream factory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_factory: Option<SourceSpan>,
    /// Factory-call identity of the captured string writer/intern table.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub string_instance: Option<SourceSpan>,
    /// Binding of the concrete NUMBER or STRING factory helper.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub writer_factory: Option<SourceSpan>,
    /// A static XOR operand applied to the value, when present.
    #[serde(default, deserialize_with = "optional_json_f64")]
    pub xor_literal: Option<f64>,
    /// The position/index expression (`buf[i]` index or writer position).
    pub position: Option<String>,
    /// Exact caller index argument of a source-proven inline store helper.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_span: Option<SourceSpan>,
    /// The caller's helper-call span when this write happens inside a program
    /// helper (a helper whose body writes through a writer passed by the
    /// caller). The leaf's own `span` and `value_span` are inside the helper.
    #[serde(default)]
    pub via_helper: Option<SourceSpan>,
    /// For a helper write, the caller argument span for each helper formal, so
    /// provenance follows a formal reference back to what the caller passed.
    #[serde(default)]
    pub helper_arguments: Vec<SourceSpan>,
    /// Where the value comes from. Filled by [`extract_signal_contracts`].
    #[serde(default)]
    pub provenance: Option<Provenance>,
}

// Internally tagged GrammarNode buffers its fields through serde Content.
// With serde_json/arbitrary_precision, a float in that buffer is represented
// by the Number protocol map. Value understands both that protocol and native
// numeric visitors; deserializing f64 directly does not.
fn optional_json_f64<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<f64>, D::Error> {
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        None => Ok(None),
        Some(serde_json::Value::Number(number)) => number
            .as_f64()
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom("XOR operand is outside f64 range")),
        _ => Err(serde::de::Error::custom(
            "XOR operand must be numeric or null",
        )),
    }
}

/// The raw 4-byte seed prefix.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PrefixNode {
    #[serde(with = "span_serde")]
    pub span: Span,
    pub spelling: PrefixSpelling,
    /// The seed word source (`Math.random() * 4294967296 | 0`).
    pub seed_word: String,
    pub elements: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_binding: Option<SourceSpan>,
}

/// One arm of a [`GrammarNode::Choice`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChoiceArm {
    /// `then`, `else`, `try`, `catch`, `right`, `skip`, `case <test>`, `default`.
    pub label: String,
    pub body: GrammarNode,
}

/// The typed, ordered write grammar of one emission.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "op")]
pub enum GrammarNode {
    RawPrefix(PrefixNode),
    Number(WriteNode),
    String(WriteNode),
    /// One logical byte at one index.
    Byte(WriteNode),
    Sequence {
        items: Vec<GrammarNode>,
    },
    Choice {
        kind: ChoiceKind,
        #[serde(with = "span_serde")]
        span: Span,
        condition: Option<String>,
        arms: Vec<ChoiceArm>,
    },
    Repeat {
        loop_kind: crate::signals::extract::layout::LoopKind,
        direction: LoopDirection,
        #[serde(with = "span_serde")]
        span: Span,
        header: String,
        count: Option<u64>,
        body: Box<GrammarNode>,
    },
    /// The emitted buffer is passed to a call the extractor does not model;
    /// that call may write it. Never treated as complete.
    Opaque {
        #[serde(with = "span_serde")]
        span: Span,
        callee: String,
        reason: String,
    },
    /// A write path that could not be typed, or a limit that was hit.
    Unresolved {
        #[serde(with = "span_serde")]
        span: Span,
        text: String,
        reason: String,
    },
}

impl GrammarNode {
    pub fn empty() -> Self {
        GrammarNode::Sequence { items: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self, GrammarNode::Sequence { items } if items.is_empty())
    }

    /// Calls `visit` on every node in pre-order.
    pub fn walk<'n>(&'n self, visit: &mut impl FnMut(&'n GrammarNode)) {
        visit(self);
        match self {
            GrammarNode::Sequence { items } => items.iter().for_each(|item| item.walk(visit)),
            GrammarNode::Choice { arms, .. } => arms.iter().for_each(|arm| arm.body.walk(visit)),
            GrammarNode::Repeat { body, .. } => body.walk(visit),
            _ => {}
        }
    }

    pub fn walk_mut(&mut self, visit: &mut impl FnMut(&mut GrammarNode)) {
        visit(self);
        match self {
            GrammarNode::Sequence { items } => items.iter_mut().for_each(|item| item.walk_mut(visit)),
            GrammarNode::Choice { arms, .. } => {
                arms.iter_mut().for_each(|arm| arm.body.walk_mut(visit))
            }
            GrammarNode::Repeat { body, .. } => body.walk_mut(visit),
            _ => {}
        }
    }

    /// The writes in evaluation order.
    pub fn writes(&self) -> Vec<&WriteNode> {
        let mut out = Vec::new();
        self.walk(&mut |node| match node {
            GrammarNode::Number(write) | GrammarNode::String(write) | GrammarNode::Byte(write) => {
                out.push(write)
            }
            _ => {}
        });
        out
    }

    /// A compact one-line rendering: `RAW4 NUMBER CHOICE:if[t]{BYTE|} …`.
    pub fn render(&self) -> String {
        let mut out = String::new();
        render_into(self, &mut out);
        out.trim().to_string()
    }
}

fn render_into(node: &GrammarNode, out: &mut String) {
    let push = |out: &mut String, text: &str| {
        if !out.is_empty() && !out.ends_with(['{', '|', ' ']) {
            out.push(' ');
        }
        out.push_str(text);
    };
    match node {
        // The spelling (literal, index stores, inherited) is a representation
        // detail kept in the node; the rendering is the wire shape.
        GrammarNode::RawPrefix(_) => push(out, "RAW4"),
        GrammarNode::Number(_) => push(out, "NUMBER"),
        GrammarNode::String(_) => push(out, "STRING"),
        GrammarNode::Byte(write) => push(
            out,
            match write.mask {
                MaskForm::Unmasked => "BYTE!",
                _ => "BYTE",
            },
        ),
        GrammarNode::Sequence { items } => items.iter().for_each(|item| render_into(item, out)),
        GrammarNode::Choice { kind, arms, .. } => {
            let kind = match kind {
                ChoiceKind::If => "if",
                ChoiceKind::Conditional => "cond",
                ChoiceKind::Logical => "logical",
                ChoiceKind::TryCatch => "try",
                ChoiceKind::Switch => "switch",
            };
            push(out, &format!("CHOICE:{kind}{{"));
            for (index, arm) in arms.iter().enumerate() {
                if index > 0 {
                    out.push('|');
                }
                render_into(&arm.body, out);
            }
            out.push('}');
        }
        GrammarNode::Repeat {
            loop_kind,
            direction,
            body,
            ..
        } => {
            let direction = match direction {
                LoopDirection::Ascending => "asc",
                LoopDirection::Descending => "desc",
                LoopDirection::Iteration => "iter",
                LoopDirection::Unknown => "?",
            };
            push(out, &format!("REPEAT:{loop_kind:?}/{direction}{{"));
            render_into(body, out);
            out.push('}');
        }
        GrammarNode::Opaque { .. } => push(out, "OPAQUE"),
        GrammarNode::Unresolved { .. } => push(out, "UNRESOLVED"),
    }
}

// ===========================================================================
// Provenance
// ===========================================================================

/// The kind of origin one dataflow source has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceClass {
    /// A literal written in this capture. Its origin (generator configuration
    /// or a trace-folded measurement) is not proven by the literal itself.
    Literal,
    /// An array/object literal table or shape (probe lists, glyph rows,
    /// option bags) read by the producer.
    SourceTable,
    /// `Math.random()` / `crypto.getRandomValues` evaluated per emission.
    MessageRandomness,
    /// Randomness drawn once outside the producer and reused.
    SessionRandomness,
    /// A binding owned outside the producer body (main-level or enclosing
    /// closure state, the producer context object): shared/per-session state.
    SharedState,
    /// An unbound (global) identifier or member path on it: a host value read
    /// at run time (`navigator.…`, `document.…`, `performance.now()`, VM
    /// kernel globals the tracer exposes).
    HostObservation,
    /// A clock read (`Date.now()`, `new Date()`, `performance.now()`).
    HostClock,
    /// A loop counter or iterated element.
    LoopIteration,
    /// The caught exception object.
    CaughtError,
    /// A call into a program function whose result is not modelled further.
    ProgramCall,
    /// A value this walk could not follow (call result of an unknown callee,
    /// parameter of an unknown caller, depth limit).
    Opaque,
}

/// What a literal is known to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiteralOrigin {
    /// Only the literal is known; it may be configuration or a folded value.
    Unproven,
    /// A paired trace of the same build computes this field from a host
    /// source and the field here is literal-only: the literal is a
    /// trace-folded observation.
    TraceFoldedObservation,
    /// The paired field has host sources this field lacks, but this field
    /// also has non-literal inputs, so which literal was folded is not proven.
    PossiblyTraceFolded,
}

/// One leaf of a value's backward dataflow.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProvenanceSource {
    pub class: SourceClass,
    #[serde(with = "span_serde")]
    pub span: Span,
    pub text: String,
    /// For host reads: the member path or call (`navigator.plugins.length`,
    /// `performance.now()`); for program calls: the callee text.
    pub api: Option<String>,
    /// For literals.
    pub literal_origin: Option<LiteralOrigin>,
    /// Definition hops followed from the written expression to this source.
    pub via: Vec<DefinitionHop>,
}

/// One followed definition of a binding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefinitionHop {
    /// Descriptive label only (the source name), never an identity key.
    pub binding: String,
    #[serde(with = "span_serde")]
    pub definition: Span,
    /// `single_reaching` (proven nearest definition), `all_definitions`
    /// (flow-insensitive union of every write), `parameter`.
    pub flow: String,
}

/// The provenance of one written value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    /// The summary class set of all sources, sorted.
    pub classes: Vec<SourceClass>,
    /// The operations applied between the sources and the written value, in
    /// first-seen order (`^`, `|0`, `>>>`, `.length`, `charCodeAt`, `hash`, …).
    pub transforms: Vec<String>,
    pub sources: Vec<ProvenanceSource>,
    /// True when the walk stopped early (depth/budget); the source list is
    /// then incomplete.
    pub truncated: bool,
    /// Literal / table sources beyond the per-class bound, counted but not
    /// listed. Never changes `classes`.
    #[serde(default)]
    pub elided: usize,
    /// The same field in a paired trace of the same raw build, when
    /// [`pair_trace_folded_literals`] aligned it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paired: Option<PairedField>,
}

/// The aligned field of a second trace of the same raw build.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PairedField {
    pub source_sha256: String,
    #[serde(with = "span_serde")]
    pub value_span: Span,
    pub value: String,
    pub classes: Vec<SourceClass>,
    pub apis: Vec<String>,
    /// Bare global reads the paired field makes and this one does not.
    #[serde(default)]
    pub missing_host_reads: Vec<String>,
}

impl Provenance {
    pub fn has(&self, class: SourceClass) -> bool {
        self.classes.contains(&class)
    }

    pub(crate) fn finish(mut self) -> Self {
        let classes: BTreeSet<SourceClass> = self.sources.iter().map(|source| source.class).collect();
        self.classes = classes.into_iter().collect();
        self
    }
}

// ===========================================================================
// Report
// ===========================================================================

/// How completely a signal's contract was extracted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractStatus {
    /// Every emission has a fully typed grammar, every write to its buffers
    /// is accounted for, and every mask stream is resolved.
    Complete,
    /// At least one concrete gap; see `reasons`.
    Partial,
    /// No emission grammar could be reconstructed.
    Unsupported,
}

/// [`ContractReason::code`] of a descriptor that is declared but never
/// emitted: its producer completes without ever calling `next`.
pub const DECLARED_NOT_EMITTED: &str = "declared_not_emitted";

/// One concrete reason a contract is not complete.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractReason {
    pub code: String,
    pub span: Option<SourceSpan>,
    pub detail: String,
}

/// How the capture serves one descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ProducerKind {
    /// Row `group` of the group table names this descriptor at `position`.
    Grouped {
        group: usize,
        position: usize,
        row_length: usize,
    },
    /// The descriptor's constructor wraps its own producer callback.
    Direct { wrapper: Option<SourceSpan> },
    /// No route was proven.
    Unrouted,
}

/// The producer body and the lifecycle callbacks it registers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Producer {
    #[serde(flatten)]
    pub kind: ProducerKind,
    /// The function whose body owns the emissions.
    pub body: Option<SourceSpan>,
    pub registrations: Vec<Registration>,
    /// The roles of the producer's positional arguments, from its invocation.
    pub arguments: Vec<LifecycleRole>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Registration {
    pub role: LifecycleRole,
    pub callback: Option<SourceSpan>,
    pub call: SourceSpan,
}

/// The stream an emission's masked writes draw from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StreamContract {
    /// Factory call argument, e.g. `seed ^ 3352346894`.
    pub argument: String,
    pub seed_word: Option<String>,
    /// The static salt text and span; a per-build literal, never an identity.
    pub salt: Option<String>,
    pub salt_span: Option<SourceSpan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<SourceSpan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_binding: Option<SourceSpan>,
}

/// One `observer.next(payload)` site.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmissionContract {
    pub site: SourceSpan,
    /// Main-thread observer sites that forward this worker-produced buffer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forwarding_sites: Vec<SourceSpan>,
    pub quote: String,
    pub route: ObserverRoute,
    /// The registered lifecycle callback the site runs in; `None` for the
    /// producer body itself or an unregistered callback.
    pub phase: Option<LifecycleRole>,
    pub payload: EmissionVariant,
    pub stream: Option<StreamContract>,
    /// Compact rendering of `grammar`.
    pub rendered: String,
    pub grammar: GrammarNode,
}

/// An `error` / `complete` call of the producer's emitter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LifecycleSite {
    pub method: EmitterMethod,
    pub site: SourceSpan,
    pub quote: String,
    pub phase: Option<LifecycleRole>,
}

/// The extracted contract of one descriptor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignalContract {
    /// The descriptor's index in this capture's registry: a capture-local join
    /// key derived from the source, never a recognition input.
    pub descriptor: u32,
    pub producer: Producer,
    pub status: ContractStatus,
    pub reasons: Vec<ContractReason>,
    pub emissions: Vec<EmissionContract>,
    pub lifecycle: Vec<LifecycleSite>,
    pub accounting: Vec<BufferAccounting>,
    /// Union of the source classes feeding this signal's written values.
    pub input_classes: Vec<SourceClass>,
}

/// The capture's producer structure, derived from its own tables.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProducerInventory {
    pub descriptor_count: usize,
    pub group_count: usize,
    pub row_lengths: Vec<usize>,
    pub grouped_descriptors: usize,
    pub direct_descriptors: Vec<u32>,
    /// Descriptors no group row or direct route serves.
    pub unrouted: Vec<u32>,
    /// Descriptors served more than once.
    pub multiply_routed: Vec<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractCoverage {
    pub descriptors: usize,
    pub complete: usize,
    pub partial: usize,
    pub unsupported: usize,
    pub emissions: usize,
    pub lifecycle_sites: usize,
    pub writes: usize,
    pub writes_in_grammar: usize,
    pub writes_excluded: usize,
    pub opaque_boundaries: usize,
    pub unresolved_nodes: usize,
    pub provenance_truncated: usize,
    /// Reason code -> number of signals carrying it.
    pub reasons: std::collections::BTreeMap<String, usize>,
}

/// The generator-ready contract report of one capture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignalContractReport {
    pub schema_version: u32,
    pub source: SourceIdentity,
    pub producers: ProducerInventory,
    pub coverage: ContractCoverage,
    pub signals: Vec<SignalContract>,
    /// What this report does not claim.
    pub limitations: Vec<String>,
    /// Set by [`pair_trace_folded_literals`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pairing: Option<PairingSummary>,
}

/// The outcome of aligning this report with a second trace of the same build.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingSummary {
    pub other_sha256: String,
    /// Descriptors whose emissions and write kinds align one to one.
    pub aligned: usize,
    /// Descriptors not aligned, with the reason.
    pub skipped: Vec<(u32, String)>,
    /// Fields without a host source whose paired field reads one, all other
    /// source classes being equal.
    pub trace_folded: usize,
    /// Aligned fields whose source classes differ otherwise: a provenance
    /// walk that one trace representation defeats, never a fold claim.
    pub provenance_mismatches: Vec<ProvenanceMismatch>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceMismatch {
    pub descriptor: u32,
    pub value_span: SourceSpan,
    pub value: String,
    pub paired_value: String,
}

impl SignalContractReport {
    pub fn signal(&self, descriptor: u32) -> Option<&SignalContract> {
        self.signals.iter().find(|signal| signal.descriptor == descriptor)
    }
}

const LIMITATIONS: &[&str] = &[
    "Static source facts only: no recovered JavaScript is executed and no runtime value is reported.",
    "Whether a branch runs, how often a loop iterates and when a callback fires are not observed; they are kept as CHOICE / REPEAT / phase.",
    "A literal's origin is unproven: a tracer can fold a host measurement into a literal. Only a paired trace of the same build can mark it trace-folded.",
    "Provenance follows definitions inside the capture; a value from a parameter of an unknown caller, an unmodelled program call or an external callee stays opaque.",
    "Descriptor indices, group numbers, salts, markers and spans are capture-local facts, never cross-build identities.",
];

/// Why [`extract_signal_contracts`] failed.
#[derive(Debug)]
pub enum ContractError {
    Inputs(ProgramInputError),
    Parse(String),
}

impl std::fmt::Display for ContractError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContractError::Inputs(error) => write!(formatter, "{error}"),
            ContractError::Parse(message) => write!(formatter, "parse: {message}"),
        }
    }
}

impl std::error::Error for ContractError {}

impl From<ProgramInputError> for ContractError {
    fn from(error: ProgramInputError) -> Self {
        ContractError::Inputs(error)
    }
}

/// Extract the generator-ready signal contracts of one capture.
pub fn extract_signal_contracts(source: &str) -> Result<SignalContractReport, ContractError> {
    let inputs = extract_program_inputs(source)?;
    signal_contracts_from_inputs(source, &inputs)
}

/// The same report from an already extracted [`ProgramInputReport`] of the
/// same source.
pub fn signal_contracts_from_inputs(
    source: &str,
    inputs: &ProgramInputReport,
) -> Result<SignalContractReport, ContractError> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::unambiguous())
        .with_options(ParseOptions {
            preserve_parens: false,
            ..ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        return Err(ContractError::Parse(
            parsed
                .errors
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; "),
        ));
    }
    let program = parsed.program;
    let semantic = SemanticBuilder::new()
        .with_check_syntax_error(false)
        .build(&program)
        .semantic;
    let (scoping, nodes) = semantic.into_scoping_and_nodes();
    let expressions = ExpressionIndex::build(&program);
    let helpers = HelperSpans::from_encoding(&inputs.encoding);
    let function_nodes = function_nodes(&nodes);

    let wiring = &inputs.wiring;
    let producers = producer_inventory(wiring);
    let mut signals = Vec::new();
    for descriptor in 0..producers.descriptor_count as u32 {
        let producer = producer_of(wiring, descriptor);
        let mut walker = ProvenanceWalker {
            source,
            scoping: &scoping,
            nodes: &nodes,
            expressions: &expressions,
            helpers: &helpers,
            producer: producer.body.map(|span| Span::new(span.start, span.end)),
            producer_arguments: producer.arguments.clone(),
            function_nodes: &function_nodes,
            helper_owner: None,
            helper_arguments: Vec::new(),
        };
        signals.push(build_contract(source, inputs, descriptor, producer, &mut walker));
    }
    let coverage = coverage_of(&signals);
    Ok(SignalContractReport {
        schema_version: SIGNAL_CONTRACT_SCHEMA_VERSION,
        source: inputs.source.clone(),
        producers,
        coverage,
        signals,
        limitations: LIMITATIONS.iter().map(|text| text.to_string()).collect(),
        pairing: None,
    })
}

fn producer_inventory(wiring: &WiringProperties) -> ProducerInventory {
    let descriptor_count = wiring
        .descriptors
        .as_ref()
        .map(|registry| registry.length)
        .unwrap_or_else(|| {
            // Some recovered programs carry the descriptor rows only in the
            // groups. Use the highest routed descriptor so the producer
            // inventory still covers them.
            wiring
                .groups
                .iter()
                .flat_map(|group| group.row.iter())
                .copied()
                .max()
                .map(|last| (last + 1) as usize)
                .unwrap_or(0)
        });
    let mut served: std::collections::BTreeMap<u32, usize> = std::collections::BTreeMap::new();
    for group in &wiring.groups {
        for descriptor in &group.row {
            *served.entry(*descriptor).or_default() += 1;
        }
    }
    let grouped_descriptors = served.len();
    for route in &wiring.direct_routes {
        *served.entry(route.descriptor_index).or_default() += 1;
    }
    let mut direct_descriptors: Vec<u32> = wiring
        .direct_routes
        .iter()
        .map(|route| route.descriptor_index)
        .collect();
    direct_descriptors.sort_unstable();
    ProducerInventory {
        descriptor_count,
        group_count: wiring.groups.len(),
        row_lengths: wiring.groups.iter().map(|group| group.row.len()).collect(),
        grouped_descriptors,
        direct_descriptors,
        unrouted: (0..descriptor_count as u32)
            .filter(|descriptor| !served.contains_key(descriptor))
            .collect(),
        multiply_routed: served
            .iter()
            .filter(|(_, count)| **count > 1)
            .map(|(descriptor, _)| *descriptor)
            .collect(),
    }
}

fn registrations(lifecycle: &LifecycleWiring) -> Vec<Registration> {
    lifecycle
        .registrations
        .iter()
        .map(|registration| Registration {
            role: registration.role,
            callback: registration.callback.as_ref().map(|function| function.span),
            call: registration.call_span,
        })
        .collect()
}

fn producer_of(wiring: &WiringProperties, descriptor: u32) -> Producer {
    for group in &wiring.groups {
        if let Some(position) = group.row.iter().position(|entry| *entry == descriptor) {
            return Producer {
                kind: ProducerKind::Grouped {
                    group: group.group_index,
                    position,
                    row_length: group.row.len(),
                },
                body: Some(group.function.span),
                registrations: registrations(&group.lifecycle),
                arguments: argument_roles(group.invocation.as_ref()),
            };
        }
    }
    if let Some(route) = wiring
        .direct_routes
        .iter()
        .find(|route| route.descriptor_index == descriptor)
    {
        // A lifecycle-spec hook runs in the phase of the registered callback
        // that calls it.
        let mut registered = registrations(&route.lifecycle);
        let hooks: Vec<Registration> = route
            .ctor
            .hooks
            .iter()
            .filter_map(|hook| {
                Some(Registration {
                    role: phase_at(&registered, hook.call)?,
                    callback: Some(hook.function.span),
                    call: hook.call,
                })
            })
            .collect();
        registered.extend(hooks);
        return Producer {
            kind: ProducerKind::Direct {
                wrapper: route.ctor.wrapper.as_ref().map(|function| function.span),
            },
            body: route.ctor.callback.as_ref().map(|function| function.span),
            registrations: registered,
            arguments: argument_roles(route.invocation.as_ref()),
        };
    }
    Producer {
        kind: ProducerKind::Unrouted,
        body: None,
        registrations: Vec::new(),
        arguments: Vec::new(),
    }
}

fn argument_roles(invocation: Option<&CallbackInvocationWiring>) -> Vec<LifecycleRole> {
    let Some(invocation) = invocation else {
        return Vec::new();
    };
    let mut roles = vec![LifecycleRole::Unknown; invocation.argument_count];
    for argument in &invocation.arguments {
        if let Some(role) = roles.get_mut(argument.index) {
            *role = argument.role;
        }
    }
    roles
}

/// The innermost registered callback containing `span`.
fn phase_at(registrations: &[Registration], span: SourceSpan) -> Option<LifecycleRole> {
    registrations
        .iter()
        .filter_map(|registration| registration.callback.map(|callback| (registration.role, callback)))
        .filter(|(_, callback)| callback.start <= span.start && span.end <= callback.end)
        .min_by_key(|(_, callback)| callback.end - callback.start)
        .map(|(role, _)| role)
}

fn quote(source: &str, span: SourceSpan) -> String {
    let text = source
        .get(span.start as usize..span.end as usize)
        .unwrap_or_default();
    if text.len() <= 160 {
        return text.to_string();
    }
    let mut end = 157;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &text[..end])
}

fn lifecycle_sites(
    source: &str,
    wiring: &WiringProperties,
    descriptor: u32,
    producer: &Producer,
) -> (Vec<LifecycleSite>, Vec<SourceSpan>) {
    let emitters = wiring
        .groups
        .iter()
        .flat_map(|group| group.local_emitters.iter())
        .chain(wiring.direct_routes.iter().flat_map(|route| route.local_emitters.iter()))
        .filter(|emitter| emitter.global_signal == Some(descriptor));
    let mut sites = Vec::new();
    let mut next_sites = Vec::new();
    for emitter in emitters {
        for call in &emitter.calls {
            match call.method {
                EmitterMethod::Next => next_sites.push(call.span),
                method => sites.push(LifecycleSite {
                    method,
                    site: call.span,
                    quote: quote(source, call.span),
                    phase: phase_at(&producer.registrations, call.span),
                }),
            }
        }
    }
    sites.sort_by_key(|site| site.site.start);
    (sites, next_sites)
}

fn build_contract(
    source: &str,
    inputs: &ProgramInputReport,
    descriptor: u32,
    producer: Producer,
    walker: &mut ProvenanceWalker<'_, '_>,
) -> SignalContract {
    let mut reasons = Vec::new();
    let (lifecycle, wiring_next_sites) =
        lifecycle_sites(source, &inputs.wiring, descriptor, &producer);
    if matches!(producer.kind, ProducerKind::Unrouted) {
        reasons.push(ContractReason {
            code: "producer_unrouted".into(),
            span: None,
            detail: "no group row or direct route serves this descriptor".into(),
        });
    }
    let layout = inputs.signals.signal(descriptor);
    let mut emissions = Vec::new();
    let mut classes = BTreeSet::new();
    for emission in layout.map(|layout| layout.emissions.as_slice()).unwrap_or_default() {
        let site = SourceSpan::new(emission.origin.start, emission.origin.end);
        let mut grammar = emission.grammar.clone();
        grammar.walk_mut(&mut |node| {
            if let GrammarNode::Number(write) | GrammarNode::String(write) | GrammarNode::Byte(write) =
                node
            {
                let provenance = walker.provenance_with(
                    write.value_span,
                    write.via_helper,
                    &write.helper_arguments,
                );
                classes.extend(provenance.classes.iter().copied());
                write.provenance = Some(provenance);
            }
        });
        grammar.walk(&mut |node| match node {
            GrammarNode::Opaque { span, callee, .. } => reasons.push(ContractReason {
                code: "buffer_passed_to_unmodelled_call".into(),
                span: Some(SourceSpan::new(span.start, span.end)),
                detail: format!("the emitted buffer is passed to `{callee}`, which may write it"),
            }),
            GrammarNode::Unresolved { span, text, reason } => reasons.push(ContractReason {
                code: reason.clone(),
                span: Some(SourceSpan::new(span.start, span.end)),
                detail: text.clone(),
            }),
            GrammarNode::Number(write) | GrammarNode::String(write) | GrammarNode::Byte(write)
                if write.mask == MaskForm::Unresolved =>
            {
                reasons.push(ContractReason {
                    code: "mask_stream_unresolved".into(),
                    span: Some(SourceSpan::new(write.span.start, write.span.end)),
                    detail: write.mask_source.clone().unwrap_or_default(),
                })
            }
            _ => {}
        });
        if emission.variant == EmissionVariant::GenericObject {
            reasons.push(ContractReason {
                code: "generic_encoder_rule_unapplied".into(),
                span: Some(site),
                detail: "a named object goes through the generic token encoder; its field encoding is not part of this grammar".into(),
            });
        }
        let stream = emission.mask.as_ref().map(|mask| StreamContract {
            instance: mask.instance,
            seed_binding: mask.seed_binding,
            argument: mask.argument.clone(),
            seed_word: mask.seed_word.clone(),
            salt: mask.xor_source.clone(),
            salt_span: mask
                .xor_source
                .as_ref()
                .map(|_| SourceSpan::new(mask.xor_span.start, mask.xor_span.end)),
        });
        emissions.push(EmissionContract {
            site,
            forwarding_sites: if producer
                .body
                .is_some_and(|body| site.start < body.start || site.end > body.end)
            {
                inputs
                    .wiring
                    .groups
                    .iter()
                    .flat_map(|g| g.local_emitters.iter())
                    .chain(
                        inputs
                            .wiring
                            .direct_routes
                            .iter()
                            .flat_map(|r| r.local_emitters.iter()),
                    )
                    .filter(|e| e.global_signal == Some(descriptor))
                    .flat_map(|e| e.calls.iter())
                    .filter(|c| {
                        c.method == EmitterMethod::Next
                            && c.payload_source.as_ref().is_some_and(|p| {
                                !p.is_empty()
                                    && p.chars().all(|ch| {
                                        ch.is_ascii_alphanumeric() || ch == '_' || ch == '$'
                                    })
                            })
                    })
                    .map(|c| c.span)
                    .collect()
            } else {
                Vec::new()
            },
            quote: quote(source, site),
            route: emission.route.clone(),
            phase: phase_at(&producer.registrations, site),
            payload: emission.variant.clone(),
            stream,
            rendered: grammar.render(),
            grammar,
        });
    }
    emissions.sort_by_key(|emission| emission.site.start);
    let accounting: Vec<BufferAccounting> = inputs
        .signals
        .accounting
        .iter()
        .filter(|entry| entry.signal == descriptor)
        .cloned()
        .collect();
    for entry in &accounting {
        for excluded in &entry.excluded {
            if excluded.reason == "other_function_scope" {
                reasons.push(ContractReason {
                    code: "write_in_other_function_scope".into(),
                    span: Some(SourceSpan::new(excluded.span.start, excluded.span.end)),
                    detail: format!(
                        "buffer {} is written from a body no emission runs in",
                        entry.buffer
                    ),
                });
            }
        }
    }
    let covered_next_sites: BTreeSet<(u32, u32)> = emissions.iter()
        .flat_map(|emission| std::iter::once(&emission.site).chain(&emission.forwarding_sites))
        .map(|site| (site.start, site.end)).collect();
    let missing_next_sites: Vec<_> = wiring_next_sites.iter()
        .filter(|site| !covered_next_sites.contains(&(site.start, site.end))).collect();
    if !missing_next_sites.is_empty() {
        reasons.push(ContractReason {
            code: "emitter_next_site_not_reconstructed".into(),
            span: None,
            detail: format!(
                "wiring proves {} next sites; {} have no reconstructed payload or forwarding route: {:?}",
                wiring_next_sites.len(), missing_next_sites.len(), missing_next_sites
            ),
        });
    }
    let status = if emissions.is_empty()
        || emissions
            .iter()
            .all(|emission| matches!(emission.grammar, GrammarNode::Unresolved { .. }))
    {
        if emissions.is_empty()
            && wiring_next_sites.is_empty()
            && inputs.declared_not_emitted.contains(&descriptor)
        {
            // Not an extraction gap: the scanner proved the task callback
            // completes without ever mentioning an observer `next`.
            reasons.push(ContractReason {
                code: DECLARED_NOT_EMITTED.into(),
                span: producer.body,
                detail: "the producer's task callback completes without ever mentioning an observer next; this descriptor never collects a value".into(),
            });
        } else if emissions.is_empty() {
            reasons.push(ContractReason {
                code: "no_emission_reconstructed".into(),
                span: None,
                detail: "no observer.next site of this descriptor was replayed".into(),
            });
        }
        ContractStatus::Unsupported
    } else if reasons.is_empty() {
        ContractStatus::Complete
    } else {
        ContractStatus::Partial
    };
    reasons.sort_by(|a, b| (a.code.as_str(), a.span.map(|s| s.start)).cmp(&(b.code.as_str(), b.span.map(|s| s.start))));
    reasons.dedup();
    SignalContract {
        descriptor,
        producer,
        status,
        reasons,
        emissions,
        lifecycle,
        accounting,
        input_classes: classes.into_iter().collect(),
    }
}

fn coverage_of(signals: &[SignalContract]) -> ContractCoverage {
    let mut coverage = ContractCoverage {
        descriptors: signals.len(),
        ..ContractCoverage::default()
    };
    for signal in signals {
        match signal.status {
            ContractStatus::Complete => coverage.complete += 1,
            ContractStatus::Partial => coverage.partial += 1,
            ContractStatus::Unsupported => coverage.unsupported += 1,
        }
        coverage.emissions += signal.emissions.len();
        coverage.lifecycle_sites += signal.lifecycle.len();
        for entry in &signal.accounting {
            coverage.writes += entry.writes;
            coverage.writes_in_grammar += entry.in_grammar;
            coverage.writes_excluded += entry.excluded.len();
        }
        for emission in &signal.emissions {
            emission.grammar.walk(&mut |node| match node {
                GrammarNode::Opaque { .. } => coverage.opaque_boundaries += 1,
                GrammarNode::Unresolved { .. } => coverage.unresolved_nodes += 1,
                GrammarNode::Number(write) | GrammarNode::String(write) | GrammarNode::Byte(write) => {
                    if write.provenance.as_ref().is_some_and(|provenance| provenance.truncated) {
                        coverage.provenance_truncated += 1;
                    }
                }
                _ => {}
            });
        }
        let codes: BTreeSet<&str> = signal.reasons.iter().map(|reason| reason.code.as_str()).collect();
        for code in codes {
            *coverage.reasons.entry(code.to_string()).or_default() += 1;
        }
    }
    coverage
}

// ===========================================================================
// Provenance walk
// ===========================================================================

/// Every expression of the parse by exact span (outermost first).
struct ExpressionIndex<'a> {
    by_span: HashMap<(u32, u32), &'a Expression<'a>>,
    /// Spans of ordinary (non-arrow) functions: the owners of `arguments`.
    functions: Vec<Span>,
    /// Every value stored under a static property name anywhere in the
    /// program: object-literal properties and `x.p = v` / `x["p"] = v`.
    fields: HashMap<String, Vec<&'a Expression<'a>>>,
}

impl<'a> ExpressionIndex<'a> {
    fn build(program: &'a Program<'a>) -> Self {
        struct Collect<'a> {
            by_span: HashMap<(u32, u32), &'a Expression<'a>>,
            functions: Vec<Span>,
            fields: HashMap<String, Vec<&'a Expression<'a>>>,
        }
        impl<'a> Visit<'a> for Collect<'a> {
            fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
                let key = match &property.key {
                    PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
                    PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
                    _ => None,
                };
                if let Some(key) = key {
                    // SAFETY of lifetime: owned by the program borrowed for 'a.
                    let value: &'a Expression<'a> = unsafe { &*(&property.value as *const _) };
                    self.fields.entry(key).or_default().push(value);
                }
                walk::walk_object_property(self, property);
            }
            fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
                let key = match &assignment.left {
                    AssignmentTarget::StaticMemberExpression(member) => Some(member.property.name.to_string()),
                    AssignmentTarget::ComputedMemberExpression(member) => match member.expression.without_parentheses() {
                        Expression::StringLiteral(literal) => Some(literal.value.to_string()),
                        _ => None,
                    },
                    _ => None,
                };
                if let (Some(key), AssignmentOperator::Assign) = (key, assignment.operator) {
                    // SAFETY of lifetime: owned by the program borrowed for 'a.
                    let value: &'a Expression<'a> = unsafe { &*(&assignment.right as *const _) };
                    self.fields.entry(key).or_default().push(value);
                }
                walk::walk_assignment_expression(self, assignment);
            }
            fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
                self.functions.push(function.span);
                walk::walk_function(self, function, flags);
            }

            fn visit_expression(&mut self, expression: &Expression<'a>) {
                let span = expression.span();
                // SAFETY of lifetime: every expression visited is owned by the
                // arena-allocated program borrowed for 'a.
                let expression: &'a Expression<'a> = unsafe { &*(expression as *const _) };
                self.by_span.entry((span.start, span.end)).or_insert(expression);
                walk::walk_expression(self, expression);
            }
        }
        let mut collect = Collect {
            by_span: HashMap::new(),
            functions: Vec::new(),
            fields: HashMap::new(),
        };
        collect.visit_program(program);
        Self {
            by_span: collect.by_span,
            functions: collect.functions,
            fields: collect.fields,
        }
    }

    /// The innermost ordinary function containing `span`.
    fn arguments_owner(&self, span: Span) -> Option<Span> {
        self.functions
            .iter()
            .filter(|function| function.start <= span.start && span.end <= function.end)
            .min_by_key(|function| function.end - function.start)
            .copied()
    }

    fn get(&self, span: Span) -> Option<&'a Expression<'a>> {
        self.by_span.get(&(span.start, span.end)).copied()
    }
}

/// Function spans of the helpers the encoding pass proved.
struct HelperSpans {
    labels: Vec<(SourceSpan, &'static str)>,
}

impl HelperSpans {
    fn from_encoding(encoding: &EncodingProperties) -> Self {
        let bindings = encoding.helper_bindings();
        let mut labels = Vec::new();
        // Every copy of a role is labelled: a two-thread capture repeats each
        // encoder helper, and a contract may be produced by either copy.
        for (spans, label) in [
            (bindings.number_writer, "number_writer"),
            (bindings.string_writer, "string_writer_factory"),
            (bindings.prng_factory, "stream_factory"),
        ] {
            for span in spans {
                labels.push((SourceSpan::new(span.start, span.end), label));
            }
        }
        if let Some(span) = bindings.hash {
            labels.push((SourceSpan::new(span.start, span.end), "hash"));
        }
        Self { labels }
    }

    fn label(&self, function: Span) -> Option<&'static str> {
        self.labels
            .iter()
            .find(|(span, _)| span.start == function.start && span.end == function.end)
            .map(|(_, label)| *label)
    }
}

const PROVENANCE_MAX_DEPTH: usize = 48;
const PROVENANCE_MAX_SOURCES: usize = 256;
const PROVENANCE_MAX_LITERALS: usize = 12;
const PROVENANCE_MAX_DEFINITIONS: usize = 8;

/// Standard host globals whose member paths are host observations. These are
/// Web platform API names, not program-generated names.
const HOST_ROOTS: &[&str] = &[
    "navigator", "document", "window", "screen", "location", "history", "performance",
    "crypto", "localStorage", "sessionStorage", "indexedDB", "chrome", "external",
    "devicePixelRatio", "visualViewport", "Intl", "WebAssembly", "Notification",
    "matchMedia", "speechSynthesis", "globalThis", "self", "top", "parent", "frames",
];

/// Other standard ECMAScript / Web globals. A bare read of one of these is a
/// platform read, never evidence of a folded program-environment value.
const PLATFORM_GLOBALS: &[&str] = &[
    "global", "Date", "Symbol", "Promise", "Proxy", "Reflect", "Error", "TypeError", "Function",
    "Uint8Array", "Int8Array", "Uint16Array", "Int16Array", "Uint32Array", "Int32Array",
    "Float32Array", "Float64Array", "ArrayBuffer", "DataView", "Map", "Set", "WeakMap", "WeakSet",
    "TextEncoder", "TextDecoder", "Blob", "URL", "Image", "Audio", "Worker", "XMLHttpRequest",
    "fetch", "setTimeout", "clearTimeout", "setInterval", "clearInterval", "requestAnimationFrame",
    "queueMicrotask", "MutationObserver", "Event", "CustomEvent", "eval", "BigInt", "undefined",
];

/// Pure built-ins: a call or member read of these transforms its operands.
const PURE_BUILTINS: &[&str] = &[
    "Math", "Number", "String", "Boolean", "parseInt", "parseFloat", "isNaN", "isFinite",
    "Object", "Array", "JSON", "encodeURIComponent", "decodeURIComponent", "escape", "unescape",
    "btoa", "atob", "RegExp",
];

/// Function and arrow nodes by their span start.
fn function_nodes(nodes: &AstNodes<'_>) -> HashMap<u32, NodeId> {
    nodes
        .iter()
        .filter(|node| matches!(node.kind(), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)))
        .map(|node| (node.kind().span().start, node.id()))
        .collect()
}

struct ProvenanceWalker<'w, 'a> {
    source: &'w str,
    scoping: &'w Scoping,
    nodes: &'w AstNodes<'a>,
    expressions: &'w ExpressionIndex<'a>,
    helpers: &'w HelperSpans,
    /// The producer body; bindings declared outside it are shared state.
    producer: Option<Span>,
    producer_arguments: Vec<LifecycleRole>,
    function_nodes: &'w HashMap<u32, NodeId>,
    /// The function a helper write's value expression lives in, and the caller
    /// argument span per formal while walking that expression.
    helper_owner: Option<Span>,
    helper_arguments: Vec<SourceSpan>,
}

struct WalkState {
    provenance: Provenance,
    visited: BTreeSet<SymbolId>,
}

impl<'w, 'a> ProvenanceWalker<'w, 'a> {
    /// Provenance of a value expression. A write replayed through a program
    /// helper also carries the helper's function span and the caller argument
    /// per formal, so a formal reference resolves to what the caller passed.
    fn provenance_with(
        &mut self,
        value: Span,
        via_helper: Option<SourceSpan>,
        helper_arguments: &[SourceSpan],
    ) -> Provenance {
        let previous_owner = self.helper_owner;
        let previous_arguments = std::mem::take(&mut self.helper_arguments);
        self.helper_owner = via_helper
            .and_then(|call| self.helper_owner(Span::new(call.start, call.end)));
        self.helper_arguments = helper_arguments.to_vec();
        let provenance = self.provenance(value);
        self.helper_owner = previous_owner;
        self.helper_arguments = previous_arguments;
        provenance
    }

    /// The string literal the caller of the helper being replayed passes for
    /// the formal `key` names, if any.
    fn helper_formal_string(&self, key: &Expression<'a>) -> Option<String> {
        let Expression::Identifier(identifier) = key else {
            return None;
        };
        let owner = self.helper_owner?;
        let symbol = self.scoping.get_reference(identifier.reference_id.get()?).symbol_id()?;
        let declaration = self.scoping.symbol_declaration(symbol);
        if !matches!(self.nodes.kind(declaration), AstKind::FormalParameter(_) | AstKind::FormalParameters(_)) {
            return None;
        }
        let function = enclosing_function(self.nodes, declaration)?;
        if self.nodes.get_node(function).span() != owner {
            return None;
        }
        let declaration_span = self.nodes.get_node(declaration).span();
        let AstKind::FormalParameters(parameters) = self.nodes.kind(self.nodes.parent_id(declaration)) else {
            return None;
        };
        let position = parameters.items.iter().position(|parameter| parameter.span == declaration_span)?;
        let argument = self.helper_arguments.get(position).filter(|argument| !argument.is_empty())?;
        match self.expressions.get(Span::new(argument.start, argument.end))?.without_parentheses() {
            Expression::StringLiteral(literal) => Some(literal.value.to_string()),
            _ => None,
        }
    }

    /// The function declared by the callee of a helper call.
    fn helper_owner(&self, call: Span) -> Option<Span> {
        let expression = self.expressions.get(call)?;
        let Expression::CallExpression(call) = expression.without_parentheses() else {
            return None;
        };
        let symbol = match call.callee.without_parentheses() {
            Expression::Identifier(identifier) => identifier
                .reference_id
                .get()
                .and_then(|reference| self.scoping.get_reference(reference).symbol_id()),
            // An inline helper (`((w, k, o, f, p, b) => w(k, o[f], b, p))(…)`)
            // is its own owner.
            Expression::ArrowFunctionExpression(arrow) => return Some(arrow.span),
            Expression::FunctionExpression(function) => return Some(function.span),
            _ => None,
        }?;
        let declaration = self.scoping.symbol_declaration(symbol);
        match self.nodes.kind(declaration) {
            AstKind::Function(function) => Some(function.span),
            _ => enclosing_function(self.nodes, declaration)
                .map(|owner| self.nodes.get_node(owner).span()),
        }
    }

    fn provenance(&mut self, value: Span) -> Provenance {
        let mut state = WalkState {
            provenance: Provenance {
                classes: Vec::new(),
                transforms: Vec::new(),
                sources: Vec::new(),
                truncated: false,
                elided: 0,
                paired: None,
            },
            visited: BTreeSet::new(),
        };
        match self.expressions.get(value) {
            Some(expression) => {
                self.walk(expression, 0, &[], &mut state);
                if state.provenance.sources.is_empty() {
                    state.provenance.sources.push(ProvenanceSource {
                        class: SourceClass::Opaque,
                        span: value,
                        text: self.text(value),
                        api: Some("no source reached".into()),
                        literal_origin: None,
                        via: Vec::new(),
                    });
                }
            }
            None => state.provenance.sources.push(ProvenanceSource {
                class: SourceClass::Opaque,
                span: value,
                text: self.text(value),
                api: Some("value expression not found".into()),
                literal_origin: None,
                via: Vec::new(),
            }),
        }
        state.provenance.finish()
    }

    fn text(&self, span: Span) -> String {
        let text = self
            .source
            .get(span.start as usize..span.end as usize)
            .unwrap_or_default();
        if text.len() <= 120 {
            return text.to_string();
        }
        let mut end = 117;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &text[..end])
    }

    fn transform(&self, state: &mut WalkState, name: impl Into<String>) {
        let name = name.into();
        if !state.provenance.transforms.contains(&name) {
            state.provenance.transforms.push(name);
        }
    }

    fn push(
        &self,
        state: &mut WalkState,
        class: SourceClass,
        span: Span,
        api: Option<String>,
        via: &[DefinitionHop],
    ) {
        // One source per (class, api) for named sources, and a bounded number
        // of literal/table sources: the class set never depends on the order
        // the walk reached them in.
        if state.provenance.sources.iter().any(|source| {
            source.class == class && (source.span == span || (source.api.is_some() && source.api == api))
        }) {
            return;
        }
        if matches!(class, SourceClass::Literal | SourceClass::SourceTable) {
            let same = state
                .provenance
                .sources
                .iter()
                .filter(|source| source.class == class)
                .count();
            if same >= PROVENANCE_MAX_LITERALS {
                state.provenance.elided += 1;
                return;
            }
        }
        if state.provenance.sources.len() >= PROVENANCE_MAX_SOURCES {
            state.provenance.truncated = true;
            return;
        }
        let text = self.text(span);
        state.provenance.sources.push(ProvenanceSource {
            class,
            span,
            text,
            api,
            literal_origin: (class == SourceClass::Literal).then_some(LiteralOrigin::Unproven),
            via: via.to_vec(),
        });
    }

    fn push_producer_argument(
        &self,
        state: &mut WalkState,
        span: Span,
        position: usize,
        via: &[DefinitionHop],
    ) {
        let role = self
            .producer_arguments
            .get(position)
            .copied()
            .unwrap_or(LifecycleRole::Unknown);
        self.push(
            state,
            SourceClass::SharedState,
            span,
            Some(format!("producer argument {position} ({role:?})")),
            via,
        );
    }

    /// `arguments[k]` read in the function that owns that `arguments`.
    fn arguments_read(&self, expression: &Expression<'_>) -> Option<(Span, Option<usize>)> {
        let Expression::ComputedMemberExpression(member) = expression.without_parentheses() else {
            return None;
        };
        let Expression::Identifier(identifier) = member.object.without_parentheses() else {
            return None;
        };
        let bound = identifier
            .reference_id
            .get()
            .and_then(|reference| self.scoping.get_reference(reference).symbol_id())
            .is_some();
        if bound || identifier.name != "arguments" {
            return None;
        }
        let owner = self.expressions.arguments_owner(member.span)?;
        let index = match member.expression.without_parentheses() {
            Expression::NumericLiteral(literal) if literal.value.fract() == 0.0 => {
                Some(literal.value as usize)
            }
            _ => None,
        };
        Some((owner, index))
    }

    /// True when `expression` is the producer's context argument, read as its
    /// formal, as `arguments[k]`, or through a local with that single value.
    fn is_context_value(&self, expression: &Expression<'_>, depth: usize) -> bool {
        if depth > 4 {
            return false;
        }
        let role_at = |position: usize| self.producer_arguments.get(position) == Some(&LifecycleRole::Context);
        if let Some((owner, Some(index))) = self.arguments_read(expression) {
            return Some(owner) == self.producer && role_at(index);
        }
        let Expression::Identifier(identifier) = expression.without_parentheses() else {
            return false;
        };
        let Some(symbol) = identifier
            .reference_id
            .get()
            .and_then(|reference| self.scoping.get_reference(reference).symbol_id())
        else {
            return false;
        };
        let declaration = self.scoping.symbol_declaration(symbol);
        let declaration_span = self.nodes.get_node(declaration).span();
        if let AstKind::FormalParameter(_) = self.nodes.kind(declaration) {
            let owner = enclosing_function(self.nodes, declaration).map(|owner| self.nodes.get_node(owner).span());
            let position = match self.nodes.kind(self.nodes.parent_id(declaration)) {
                AstKind::FormalParameters(parameters) => parameters
                    .items
                    .iter()
                    .position(|parameter| parameter.span == declaration_span),
                _ => None,
            };
            return owner == self.producer && position.is_some_and(role_at);
        }
        let definitions = self.definitions(symbol);
        matches!(definitions.as_slice(), [definition] if definition.value.is_some_and(|value| self.is_context_value(value, depth + 1)))
    }

    /// A callback's parameter: an element of the collection an iteration
    /// method walks, or otherwise the data the receiving call is handed.
    #[allow(clippy::too_many_arguments)]
    fn walk_callback_source(
        &mut self,
        call: &'a CallExpression<'a>,
        function_span: Span,
        span: Span,
        api: String,
        depth: usize,
        via: &[DefinitionHop],
        state: &mut WalkState,
    ) {
        let iterated = matches!(
            member_name(&call.callee).as_deref(),
            Some(
                "map" | "forEach" | "filter" | "reduce" | "reduceRight" | "some" | "every" | "find"
                    | "findIndex" | "flatMap" | "sort"
            )
        );
        let hop = DefinitionHop {
            binding: api.clone(),
            definition: call.span,
            flow: "callback_parameter".into(),
        };
        let via = [via, std::slice::from_ref(&hop)].concat();
        self.push(state, SourceClass::LoopIteration, span, Some(api), &via);
        if iterated {
            self.transform(state, "element");
            if let Some(object) = member_object_of(&call.callee) {
                self.walk(object, depth + 1, &via, state);
            }
            return;
        }
        self.transform(state, "callback");
        for argument in &call.arguments {
            if argument.span() == function_span {
                continue;
            }
            if let Some(argument) = argument.as_expression() {
                self.walk(argument, depth + 1, &via, state);
            }
        }
    }

    /// True when a member chain bottoms out at the host: an unbound global,
    /// the producer context object, or a local whose single value is one.
    fn host_rooted(&self, expression: &Expression<'_>, depth: usize) -> bool {
        if depth > 6 {
            return false;
        }
        if self.is_context_value(expression, 0) {
            return true;
        }
        match expression.without_parentheses() {
            Expression::StaticMemberExpression(member) => self.host_rooted(&member.object, depth + 1),
            Expression::ComputedMemberExpression(member) => self.host_rooted(&member.object, depth + 1),
            Expression::ConditionalExpression(conditional) => {
                self.host_rooted_or_void(&conditional.consequent, depth + 1)
                    && self.host_rooted_or_void(&conditional.alternate, depth + 1)
            }
            Expression::LogicalExpression(logical) => {
                self.host_rooted_or_void(&logical.left, depth + 1)
                    && self.host_rooted_or_void(&logical.right, depth + 1)
            }
            Expression::Identifier(identifier) => {
                match identifier
                    .reference_id
                    .get()
                    .and_then(|reference| self.scoping.get_reference(reference).symbol_id())
                {
                    None => identifier.name != "arguments",
                    Some(symbol) => {
                        let definitions = self.definitions(symbol);
                        !definitions.is_empty()
                            && definitions.len() <= 4
                            && definitions.iter().all(|definition| {
                                definition.value.is_some_and(|value| self.host_rooted(value, depth + 1))
                            })
                    }
                }
            }
            _ => false,
        }
    }

    fn host_rooted_or_void(&self, expression: &Expression<'_>, depth: usize) -> bool {
        matches!(expression.without_parentheses(),
            Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::Void)
            || matches!(expression.without_parentheses(), Expression::NullLiteral(_))
            || self.host_rooted(expression, depth)
    }

    /// The call a callback function is passed to, when it is a direct argument.
    fn callback_call(&self, function: NodeId) -> Option<&'a CallExpression<'a>> {
        let span = self.nodes.get_node(function).span();
        // A hoisted declaration passed by name: the one call that receives a
        // reference to its binding as an argument.
        if let AstKind::Function(declared) = self.nodes.kind(function) {
            if let Some(symbol) = declared.id.as_ref().and_then(|id| id.symbol_id.get()) {
                let mut calls = Vec::new();
                for reference in self.scoping.get_resolved_references(symbol) {
                    let reference_span = self.nodes.get_node(reference.node_id()).span();
                    let parent = self.nodes.parent_id(reference.node_id());
                    if let AstKind::CallExpression(call) = self.nodes.kind(parent) {
                        if call.arguments.iter().any(|argument| argument.span() == reference_span) {
                            calls.push(call);
                        }
                    }
                }
                if let [call] = calls.as_slice() {
                    return Some(call);
                }
                return None;
            }
        }
        let mut current = function;
        for _ in 0..2 {
            let parent = self.nodes.parent_id(current);
            if parent == current {
                return None;
            }
            current = parent;
            if let AstKind::CallExpression(call) = self.nodes.kind(current) {
                return call.arguments.iter().any(|argument| argument.span() == span).then_some(call);
            }
        }
        None
    }

    fn inside_producer(&self, span: Span) -> bool {
        self.producer
            .is_some_and(|producer| producer.start <= span.start && span.end <= producer.end)
    }

    fn walk(&mut self, expression: &'a Expression<'a>, depth: usize, via: &[DefinitionHop], state: &mut WalkState) {
        if depth > PROVENANCE_MAX_DEPTH {
            state.provenance.truncated = true;
            self.push(state, SourceClass::Opaque, expression.span(), Some("depth limit".into()), via);
            return;
        }
        let next = depth + 1;
        match expression.without_parentheses() {
            Expression::NumericLiteral(_)
            | Expression::StringLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::BigIntLiteral(_)
            | Expression::RegExpLiteral(_) => {
                self.push(state, SourceClass::Literal, expression.span(), None, via)
            }
            Expression::TemplateLiteral(template) => {
                if template.expressions.is_empty() {
                    self.push(state, SourceClass::Literal, expression.span(), None, via);
                } else {
                    self.transform(state, "template");
                    for inner in &template.expressions {
                        self.walk(inner, next, via, state);
                    }
                }
            }
            Expression::ArrayExpression(array) => {
                let dynamic: Vec<&'a Expression<'a>> = array
                    .elements
                    .iter()
                    .filter_map(|element| element.as_expression())
                    .filter(|element| !is_static_literal(element))
                    .collect();
                if dynamic.is_empty() {
                    self.push(state, SourceClass::SourceTable, expression.span(), None, via);
                } else {
                    self.transform(state, "array");
                    for element in dynamic {
                        self.walk(element, next, via, state);
                    }
                }
            }
            Expression::ObjectExpression(_) => {
                self.push(state, SourceClass::SourceTable, expression.span(), None, via)
            }
            Expression::Identifier(identifier) => self.walk_identifier(identifier, expression, depth, via, state),
            Expression::StaticMemberExpression(_) | Expression::ComputedMemberExpression(_) => {
                self.walk_member(expression, depth, via, state)
            }
            Expression::CallExpression(call) => self.walk_call(call, expression.span(), depth, via, state),
            Expression::NewExpression(new) => {
                let callee = root_global_name(&new.callee, self.scoping);
                let callee = callee.filter(|_| !self.is_context_value(&new.callee, 0));
                match callee.as_deref() {
                    Some("Date") if new.arguments.is_empty() => {
                        self.push(state, SourceClass::HostClock, expression.span(), Some("new Date()".into()), via)
                    }
                    Some(name) => {
                        self.transform(state, format!("new {name}"));
                        if new.arguments.is_empty() || !PURE_BUILTINS.contains(&name) {
                            self.push(
                                state,
                                SourceClass::HostObservation,
                                expression.span(),
                                Some(format!("new {}", self.text(new.callee.span()))),
                                via,
                            );
                        }
                        for argument in &new.arguments {
                            if let Some(argument) = argument.as_expression() {
                                self.walk(argument, next, via, state);
                            }
                        }
                    }
                    None if self.host_rooted(&new.callee, 0) => {
                        let path = member_path(&new.callee).unwrap_or_else(|| self.text(new.callee.span()));
                        let class = if path == "Date" || path.ends_with(".Date") {
                            SourceClass::HostClock
                        } else {
                            SourceClass::HostObservation
                        };
                        self.push(state, class, expression.span(), Some(format!("new {path}")), via);
                        for argument in &new.arguments {
                            if let Some(argument) = argument.as_expression() {
                                self.walk(argument, next, via, state);
                            }
                        }
                    }
                    None => self.push(
                        state,
                        SourceClass::Opaque,
                        expression.span(),
                        Some(format!("new {}", self.text(new.callee.span()))),
                        via,
                    ),
                }
            }
            Expression::BinaryExpression(binary) => {
                self.transform(state, binary.operator.as_str());
                self.walk(&binary.left, next, via, state);
                self.walk(&binary.right, next, via, state);
            }
            Expression::LogicalExpression(logical) => {
                self.transform(state, logical.operator.as_str());
                self.walk(&logical.left, next, via, state);
                self.walk(&logical.right, next, via, state);
            }
            Expression::UnaryExpression(unary) => {
                if unary.operator == UnaryOperator::Void {
                    self.push(state, SourceClass::Literal, expression.span(), None, via);
                } else {
                    self.transform(state, unary.operator.as_str());
                    self.walk(&unary.argument, next, via, state);
                }
            }
            Expression::ConditionalExpression(conditional) => {
                self.transform(state, "select");
                self.walk(&conditional.test, next, via, state);
                self.walk(&conditional.consequent, next, via, state);
                self.walk(&conditional.alternate, next, via, state);
            }
            Expression::SequenceExpression(sequence) => {
                if let Some(last) = sequence.expressions.last() {
                    self.walk(last, next, via, state);
                }
            }
            Expression::AssignmentExpression(assignment) => {
                if assignment.operator != AssignmentOperator::Assign {
                    self.transform(state, assignment.operator.as_str());
                }
                self.walk(&assignment.right, next, via, state);
            }
            Expression::AwaitExpression(await_expression) => {
                self.transform(state, "await");
                self.walk(&await_expression.argument, next, via, state);
            }
            Expression::UpdateExpression(_) => {
                self.transform(state, "update");
                self.push(state, SourceClass::LoopIteration, expression.span(), None, via);
            }
            _ => self.push(state, SourceClass::Opaque, expression.span(), None, via),
        }
    }

    fn walk_identifier(
        &mut self,
        identifier: &'a IdentifierReference<'a>,
        expression: &'a Expression<'a>,
        depth: usize,
        via: &[DefinitionHop],
        state: &mut WalkState,
    ) {
        let span = identifier.span;
        let symbol = identifier
            .reference_id
            .get()
            .and_then(|reference| self.scoping.get_reference(reference).symbol_id());
        let Some(symbol) = symbol else {
            match identifier.name.as_str() {
                "undefined" | "NaN" | "Infinity" => {
                    self.push(state, SourceClass::Literal, span, None, via)
                }
                name => self.push(state, SourceClass::HostObservation, span, Some(name.to_string()), via),
            }
            return;
        };
        if !state.visited.insert(symbol) {
            return;
        }
        let name = self.scoping.symbol_name(symbol).to_string();
        let declaration = self.scoping.symbol_declaration(symbol);
        let declaration_span = self.nodes.get_node(declaration).span();
        match self.nodes.kind(declaration) {
            AstKind::CatchParameter(_) => {
                self.push(state, SourceClass::CaughtError, span, Some(name), via);
                return;
            }
            AstKind::FormalParameter(_) | AstKind::FormalParameters(_) => {
                let owner = enclosing_function(self.nodes, declaration)
                    .map(|owner| self.nodes.get_node(owner).span());
                let position = match self.nodes.kind(self.nodes.parent_id(declaration)) {
                    AstKind::FormalParameters(parameters) => parameters
                        .items
                        .iter()
                        .position(|parameter| parameter.span == declaration_span),
                    _ => None,
                };
                match (owner, position) {
                    (Some(owner), Some(position))
                        if Some(owner) == self.helper_owner
                            && self
                                .helper_arguments
                                .get(position)
                                .is_some_and(|argument| !argument.is_empty()) =>
                    {
                        let argument = self.helper_arguments[position];
                        let hop = DefinitionHop {
                            binding: name.clone(),
                            definition: declaration_span,
                            flow: "helper_argument".into(),
                        };
                        let via = [via, std::slice::from_ref(&hop)].concat();
                        match self.expressions.get(Span::new(argument.start, argument.end)) {
                            Some(expression) => self.walk(expression, depth + 1, &via, state),
                            None => self.push(
                                state,
                                SourceClass::Opaque,
                                span,
                                Some(format!("helper argument for {name}")),
                                &via,
                            ),
                        }
                    }
                    (Some(owner), Some(position)) if Some(owner) == self.producer => {
                        self.push_producer_argument(state, span, position, via)
                    }
                    _ if !self.inside_producer(declaration_span) => self.push(
                        state,
                        SourceClass::SharedState,
                        span,
                        Some(format!("parameter {name} of an enclosing function")),
                        via,
                    ),
                    _ => {
                        let call = enclosing_function(self.nodes, declaration)
                            .and_then(|function| self.callback_call(function).map(|call| (function, call)));
                        match call {
                            Some((function, call)) => {
                                let function_span = self.nodes.get_node(function).span();
                                self.walk_callback_source(
                                    call,
                                    function_span,
                                    span,
                                    format!("callback parameter {name}"),
                                    depth,
                                    via,
                                    state,
                                );
                            }
                            None => self.push(
                                state,
                                SourceClass::Opaque,
                                span,
                                Some(format!("parameter {name} of a nested callback")),
                                via,
                            ),
                        }
                    }
                }
                return;
            }
            AstKind::Function(_) | AstKind::Class(_) => {
                self.push(state, SourceClass::ProgramCall, span, Some(format!("function {name}")), via);
                return;
            }
            _ => {}
        }
        if self.is_loop_binding(declaration) {
            self.push(state, SourceClass::LoopIteration, span, Some(name.clone()), via);
            if let Some(iterated) = self.iterated_value(declaration) {
                self.transform(state, "iterate");
                let hop = DefinitionHop {
                    binding: name,
                    definition: declaration_span,
                    flow: "loop_binding".into(),
                };
                let via = [via, std::slice::from_ref(&hop)].concat();
                self.walk(iterated, depth + 1, &via, state);
            }
            return;
        }
        let shared = !self.inside_producer(declaration_span);
        // A single proven reaching definition first.
        if let Ok(definition) = reaching::expression_value_at_use(self.scoping, self.nodes, expression) {
            let hop = DefinitionHop {
                binding: name,
                definition: definition.value.span(),
                flow: "single_reaching".into(),
            };
            let via = [via, std::slice::from_ref(&hop)].concat();
            self.walk_definition(definition.value, shared, depth, &via, state);
            return;
        }
        let definitions = self.definitions(symbol);
        // A reused local (`v = list.length; …; if (t) { v = a; } else { v = b; } f(v)`):
        // the arms' writes that reach the read, not every value the binding
        // ever held. A local whose every definition reaches keeps the
        // all-definitions walk below (with its branch-join test).
        if let Ok(reaching) = reaching::expression_definitions_at_use(self.scoping, self.nodes, expression) {
            if reaching.len() < definitions.len() {
                for definition in reaching {
                    let hop = DefinitionHop {
                        binding: name.clone(),
                        definition: definition.value.span(),
                        flow: "joined_reaching".into(),
                    };
                    let via = [via, std::slice::from_ref(&hop)].concat();
                    self.walk_definition(definition.value, shared, depth, &via, state);
                }
                return;
            }
        }
        if definitions.is_empty() {
            self.push(state, SourceClass::Opaque, span, Some(format!("{name}: no definition")), via);
            return;
        }
        if shared && definitions.len() > 1 {
            // Mutable state owned outside the producer: the value at emission
            // time depends on whatever ran before.
            self.push(state, SourceClass::SharedState, span, Some(name.clone()), via);
        }
        if definitions.len() > PROVENANCE_MAX_DEFINITIONS {
            state.provenance.truncated = true;
        }
        let owner = enclosing_function(self.nodes, declaration);
        let containers = definitions
            .iter()
            .any(|definition| definition.value.is_some_and(|value| self.is_container_value(value, 0)));
        if containers {
            self.walk_container_contents(symbol, &name, depth, via, state);
        }
        // `let v; if (t) { v = a; } else { v = b; }`: the tracer's spelling
        // of `v = t ? a : b`; the test selects the value.
        if let Some(test) = self.branch_join_test(declaration, &definitions) {
            self.transform(state, "select");
            self.walk(test, depth + 1, via, state);
        }
        for definition in definitions.into_iter().take(PROVENANCE_MAX_DEFINITIONS) {
            let hop = DefinitionHop {
                binding: name.clone(),
                definition: definition.span,
                flow: "all_definitions".into(),
            };
            let via = [via, std::slice::from_ref(&hop)].concat();
            match definition.value {
                Some(value) => {
                    if definition.compound {
                        self.transform(state, "accumulate");
                    }
                    self.walk_definition(value, shared, depth, &via, state);
                }
                None => {
                    self.transform(state, "count");
                    self.push(state, SourceClass::LoopIteration, definition.span, Some(name.clone()), &via);
                }
            }
            // An accumulating write takes its effect from the conditions that
            // guard it and from the data a callback around it is handed.
            if definition.compound || definition.value.is_none() {
                self.walk_write_context(definition.node, owner, depth, &via, state);
            }
        }
    }

    /// The test of the one `if` whose two arms hold every definition of a
    /// binding declared without an initializer: each definition is a plain
    /// value write, the nearest enclosing `if` of each (below the
    /// declaration's scope) is the same statement, and both arms write.
    fn branch_join_test(&self, declaration: NodeId, definitions: &[Definition<'a>]) -> Option<&'a Expression<'a>> {
        let AstKind::VariableDeclarator(declarator) = self.nodes.kind(declaration) else {
            return None;
        };
        if declarator.init.is_some() || definitions.len() < 2 {
            return None;
        }
        let declared = declarator.span;
        let mut join: Option<(NodeId, &'a oxc_ast::ast::IfStatement<'a>)> = None;
        let mut arms = [false, false];
        for definition in definitions {
            if definition.compound || definition.value.is_none() {
                return None;
            }
            let mut child = definition.node;
            let found = loop {
                let parent = self.nodes.parent_id(child);
                if parent == child || contains(self.nodes.get_node(parent).span(), declared) {
                    break None;
                }
                match self.nodes.kind(parent) {
                    AstKind::IfStatement(statement) => {
                        let child_span = self.nodes.get_node(child).span();
                        if contains(statement.consequent.span(), child_span) {
                            break Some((parent, statement, 0));
                        }
                        if statement.alternate.as_ref().is_some_and(|alternate| contains(alternate.span(), child_span)) {
                            break Some((parent, statement, 1));
                        }
                        return None;
                    }
                    AstKind::Function(_)
                    | AstKind::ArrowFunctionExpression(_)
                    | AstKind::ForStatement(_)
                    | AstKind::WhileStatement(_)
                    | AstKind::DoWhileStatement(_)
                    | AstKind::ForInStatement(_)
                    | AstKind::ForOfStatement(_)
                    | AstKind::ConditionalExpression(_)
                    | AstKind::LogicalExpression(_) => return None,
                    _ => {}
                }
                child = parent;
            };
            let (node, statement, arm) = found?;
            match join {
                Some((seen, _)) if seen != node => return None,
                _ => join = Some((node, statement)),
            }
            arms[arm] = true;
        }
        let (_, statement) = join?;
        (arms[0] && arms[1]).then_some(&statement.test)
    }

    /// Values stored into a container binding after it was created:
    /// `c[k] = v`, `c.p = v`, `c.push(v)` / `c.unshift(v)`.
    fn walk_container_contents(
        &mut self,
        symbol: SymbolId,
        name: &str,
        depth: usize,
        via: &[DefinitionHop],
        state: &mut WalkState,
    ) {
        let mut stored = 0;
        for reference in self.scoping.get_resolved_references(symbol) {
            if stored >= PROVENANCE_MAX_DEFINITIONS {
                state.provenance.truncated = true;
                return;
            }
            let member = self.nodes.parent_id(reference.node_id());
            let member_span = self.nodes.get_node(member).span();
            let is_member = matches!(
                self.nodes.kind(member),
                AstKind::StaticMemberExpression(_) | AstKind::ComputedMemberExpression(_)
            );
            if !is_member {
                continue;
            }
            let mut current = member;
            for _ in 0..3 {
                let parent = self.nodes.parent_id(current);
                if parent == current {
                    break;
                }
                current = parent;
                match self.nodes.kind(current) {
                    AstKind::AssignmentExpression(assignment)
                        if contains(assignment.left.span(), member_span) =>
                    {
                        let hop = DefinitionHop {
                            binding: name.to_string(),
                            definition: assignment.right.span(),
                            flow: "container_store".into(),
                        };
                        let via = [via, std::slice::from_ref(&hop)].concat();
                        self.transform(state, "store");
                        self.walk(&assignment.right, depth + 1, &via, state);
                        stored += 1;
                        break;
                    }
                    AstKind::CallExpression(call) if call.callee.span() == member_span => {
                        if matches!(member_name(&call.callee).as_deref(), Some("push" | "unshift" | "splice" | "set" | "add")) {
                            self.transform(state, "store");
                            for argument in &call.arguments {
                                if let Some(argument) = argument.as_expression() {
                                    let hop = DefinitionHop {
                                        binding: name.to_string(),
                                        definition: argument.span(),
                                        flow: "container_store".into(),
                                    };
                                    let via = [via, std::slice::from_ref(&hop)].concat();
                                    self.walk(argument, depth + 1, &via, state);
                                }
                            }
                            stored += 1;
                        }
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    /// The guards between a write and its binding's owner (`if` tests,
    /// conditional tests, the left side of a short circuit) and, for a write
    /// inside a callback, the other arguments of the call it is passed to.
    fn walk_write_context(
        &mut self,
        write: NodeId,
        owner: Option<NodeId>,
        depth: usize,
        via: &[DefinitionHop],
        state: &mut WalkState,
    ) {
        let mut child = write;
        let mut guards = 0;
        for _ in 0..64 {
            let parent = self.nodes.parent_id(child);
            if parent == child || Some(parent) == owner || guards >= 6 {
                return;
            }
            let child_span = self.nodes.get_node(child).span();
            match self.nodes.kind(parent) {
                AstKind::IfStatement(statement) if !contains(statement.test.span(), child_span) => {
                    self.transform(state, "guard");
                    self.walk(&statement.test, depth + 1, via, state);
                    guards += 1;
                }
                AstKind::ConditionalExpression(conditional)
                    if !contains(conditional.test.span(), child_span) =>
                {
                    self.transform(state, "guard");
                    self.walk(&conditional.test, depth + 1, via, state);
                    guards += 1;
                }
                AstKind::LogicalExpression(logical) if contains(logical.right.span(), child_span) => {
                    self.transform(state, "guard");
                    self.walk(&logical.left, depth + 1, via, state);
                    guards += 1;
                }
                AstKind::CallExpression(call)
                    if call
                        .arguments
                        .iter()
                        .any(|argument| argument.span() == child_span) =>
                {
                    // `child` is a callback argument of this call.
                    self.transform(state, "callback");
                    for argument in &call.arguments {
                        if argument.span() == child_span {
                            continue;
                        }
                        if let Some(argument) = argument.as_expression() {
                            self.walk(argument, depth + 1, via, state);
                        }
                    }
                    guards += 1;
                }
                _ => {}
            }
            child = parent;
        }
    }

    fn walk_definition(
        &mut self,
        value: &'a Expression<'a>,
        shared: bool,
        depth: usize,
        via: &[DefinitionHop],
        state: &mut WalkState,
    ) {
        let before = state.provenance.sources.len();
        self.walk(value, depth + 1, via, state);
        if shared {
            for source in &mut state.provenance.sources[before..] {
                if source.class == SourceClass::MessageRandomness {
                    source.class = SourceClass::SessionRandomness;
                }
            }
        }
    }

    /// An array or object literal, or a local that only ever holds one
    /// (`let list = held;` where `held` was created as `[]`): stores into
    /// the alias fill the same container.
    fn is_container_value(&self, value: &Expression<'_>, depth: usize) -> bool {
        match value.without_parentheses() {
            Expression::ArrayExpression(_) | Expression::ObjectExpression(_) => true,
            Expression::Identifier(identifier) if depth < 2 => {
                let Some(symbol) = identifier
                    .reference_id
                    .get()
                    .and_then(|reference| self.scoping.get_reference(reference).symbol_id())
                else {
                    return false;
                };
                let definitions = self.definitions(symbol);
                !definitions.is_empty()
                    && definitions
                        .iter()
                        .all(|definition| definition.value.is_some_and(|value| self.is_container_value(value, depth + 1)))
            }
            _ => false,
        }
    }

    /// Every write of `symbol`: declarator initializers, plain and compound
    /// assignments, and update expressions (no value of their own). A
    /// binding destructured from an array literal (`let [a, b] = [x, y]`)
    /// is defined by its element.
    fn definitions(&self, symbol: SymbolId) -> Vec<Definition<'a>> {
        let mut out = Vec::new();
        let declaration = self.scoping.symbol_declaration(symbol);
        if let AstKind::VariableDeclarator(declarator) = self.nodes.kind(declaration) {
            if let Some(init) = declarator.init.as_ref() {
                let value = match (&declarator.id, init.without_parentheses()) {
                    (BindingPattern::ArrayPattern(pattern), Expression::ArrayExpression(array)) => {
                        let position = pattern.elements.iter().position(|element| {
                            matches!(element, Some(BindingPattern::BindingIdentifier(binding)) if binding.symbol_id.get() == Some(symbol))
                        });
                        position
                            .and_then(|position| array.elements.get(position))
                            .and_then(|element| element.as_expression())
                            .filter(|_| array.elements.iter().all(|element| element.as_expression().is_some()))
                    }
                    _ => Some(init),
                };
                if let Some(value) = value {
                    out.push(Definition {
                        value: Some(value),
                        compound: false,
                        span: value.span(),
                        node: declaration,
                    });
                }
            }
        }
        for reference in self.scoping.get_resolved_references(symbol) {
            if !reference.is_write() {
                continue;
            }
            let mut current = reference.node_id();
            for _ in 0..4 {
                let parent = self.nodes.parent_id(current);
                if parent == current {
                    break;
                }
                current = parent;
                match self.nodes.kind(current) {
                    AstKind::AssignmentExpression(assignment) => {
                        out.push(Definition {
                            value: Some(&assignment.right),
                            compound: assignment.operator != AssignmentOperator::Assign,
                            span: assignment.right.span(),
                            node: current,
                        });
                        break;
                    }
                    AstKind::UpdateExpression(update) => {
                        out.push(Definition {
                            value: None,
                            compound: true,
                            span: update.span,
                            node: current,
                        });
                        break;
                    }
                    _ => {}
                }
            }
        }
        out
    }

    fn is_loop_binding(&self, declaration: NodeId) -> bool {
        let mut current = declaration;
        for _ in 0..3 {
            let parent = self.nodes.parent_id(current);
            if parent == current {
                return false;
            }
            current = parent;
            match self.nodes.kind(current) {
                AstKind::ForInStatement(_) | AstKind::ForOfStatement(_) => return true,
                AstKind::ForStatement(statement) => {
                    let declaration_span = self.nodes.get_node(declaration).span();
                    return statement
                        .init
                        .as_ref()
                        .is_some_and(|init| init.span().start <= declaration_span.start && declaration_span.end <= init.span().end);
                }
                _ => {}
            }
        }
        false
    }

    fn iterated_value(&self, declaration: NodeId) -> Option<&'a Expression<'a>> {
        let mut current = declaration;
        for _ in 0..3 {
            let parent = self.nodes.parent_id(current);
            if parent == current {
                return None;
            }
            current = parent;
            match self.nodes.kind(current) {
                AstKind::ForInStatement(statement) => return Some(&statement.right),
                AstKind::ForOfStatement(statement) => return Some(&statement.right),
                AstKind::ForStatement(_) => return None,
                _ => {}
            }
        }
        None
    }

    /// `t[k]` of the tracer's argument tuple (`t = [v, buf, buf]`): the
    /// element `t[k]` holds when `t`'s single reaching definition is that
    /// array literal and every use of `t` is a constant-index read.
    fn tuple_element(&self, expression: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
        let Expression::ComputedMemberExpression(member) = expression.without_parentheses() else {
            return None;
        };
        let Expression::NumericLiteral(index) = member.expression.without_parentheses() else {
            return None;
        };
        let Expression::Identifier(identifier) = member.object.without_parentheses() else {
            return None;
        };
        let symbol = identifier
            .reference_id
            .get()
            .and_then(|reference| self.scoping.get_reference(reference).symbol_id())?;
        let pair = reaching::expression_value_at_use(self.scoping, self.nodes, &member.object).ok()?;
        let Expression::ArrayExpression(array) = pair.value.without_parentheses() else {
            return None;
        };
        let literal = array.span;
        for reference in self.scoping.get_resolved_references(symbol) {
            if reference.is_write() {
                continue;
            }
            let read = self.nodes.get_node(reference.node_id()).span();
            // The literal's own reads (`t = [w, t, buf]`) read the old value.
            if literal.start <= read.start && read.end <= literal.end {
                continue;
            }
            let parent = self.nodes.parent_id(reference.node_id());
            let indexed = match self.nodes.kind(parent) {
                AstKind::ComputedMemberExpression(use_) => {
                    use_.object.span() == read && matches!(use_.expression.without_parentheses(), Expression::NumericLiteral(_))
                }
                // A test (`t == null`, `!t`, `typeof t`) cannot change it.
                AstKind::BinaryExpression(_) | AstKind::UnaryExpression(_) => continue,
                _ => false,
            };
            if !indexed {
                return None;
            }
            // A store through `t[k] = …` changes the element.
            let member_span = self.nodes.get_node(parent).span();
            let mut node = parent;
            for _ in 0..3 {
                node = self.nodes.parent_id(node);
                let stored = match self.nodes.kind(node) {
                    AstKind::AssignmentExpression(assignment) => {
                        let left = assignment.left.span();
                        left.start <= member_span.start && member_span.end <= left.end
                    }
                    AstKind::UpdateExpression(update) => update.argument.span() == member_span,
                    _ => false,
                };
                if stored {
                    return None;
                }
            }
        }
        if index.value.fract() != 0.0 || index.value < 0.0 {
            return None;
        }
        array.elements.get(index.value as usize)?.as_expression()
    }

    fn walk_member(&mut self, expression: &'a Expression<'a>, depth: usize, via: &[DefinitionHop], state: &mut WalkState) {
        let (object, property, key) = match expression.without_parentheses() {
            Expression::StaticMemberExpression(member) => {
                (&member.object, Some(member.property.name.to_string()), None)
            }
            Expression::ComputedMemberExpression(member) => match member.expression.without_parentheses() {
                Expression::StringLiteral(literal) => (&member.object, Some(literal.value.to_string()), None),
                key => match self.helper_formal_string(key) {
                    // `o[f]` in a helper whose caller passes a string for `f`.
                    Some(name) => (&member.object, Some(name), None),
                    None => (&member.object, None, Some(&member.expression)),
                },
            },
            _ => return,
        };
        if let Some(element) = self.tuple_element(expression) {
            self.walk(element, depth + 1, via, state);
            return;
        }
        if let Some(property) = property.as_ref().filter(|_| self.is_context_value(object, 0)) {
            // A field of the producer context object: follow the values the
            // program stores under that field name (object literals and
            // member stores), a flow-insensitive field approximation.
            let candidates = self.expressions.fields.get(property).cloned().unwrap_or_default();
            if candidates.is_empty() || candidates.len() > 4 {
                self.push(
                    state,
                    SourceClass::SharedState,
                    expression.span(),
                    Some(format!("context field {property}")),
                    via,
                );
            } else {
                self.transform(state, format!("context.{property}"));
                for candidate in candidates {
                    let hop = DefinitionHop {
                        binding: format!("context.{property}"),
                        definition: candidate.span(),
                        flow: "context_field".into(),
                    };
                    let via = [via, std::slice::from_ref(&hop)].concat();
                    self.walk(candidate, depth + 1, &via, state);
                }
            }
            return;
        }
        if let Some((owner, index)) = self.arguments_read(expression) {
            match index {
                Some(index) if Some(owner) == self.producer => {
                    self.push_producer_argument(state, expression.span(), index, via)
                }
                _ => {
                    let call = self
                        .function_nodes
                        .get(&owner.start)
                        .and_then(|function| self.callback_call(*function).map(|call| (owner, call)));
                    match call {
                        Some((function_span, call)) => self.walk_callback_source(
                            call,
                            function_span,
                            expression.span(),
                            "callback argument".into(),
                            depth,
                            via,
                            state,
                        ),
                        None => self.push(
                            state,
                            SourceClass::Opaque,
                            expression.span(),
                            Some("argument of a nested callback".into()),
                            via,
                        ),
                    }
                }
            }
            return;
        }
        if let Some(root) = root_global_name(object, self.scoping) {
            if HOST_ROOTS.contains(&root.as_str()) || !PURE_BUILTINS.contains(&root.as_str()) {
                self.push(
                    state,
                    SourceClass::HostObservation,
                    expression.span(),
                    Some(member_path(expression).unwrap_or_else(|| self.text(expression.span()))),
                    via,
                );
                if let Some(key) = key {
                    self.walk(key, depth + 1, via, state);
                }
                return;
            }
        }
        if let Some(path) = member_path(expression) {
            if path.split('.').any(|segment| HOST_ROOTS.contains(&segment)) {
                self.push(state, SourceClass::HostObservation, expression.span(), Some(path), via);
                if let Some(key) = key {
                    self.walk(key, depth + 1, via, state);
                }
                return;
            }
        }
        self.transform(
            state,
            match &property {
                Some(property) => format!(".{property}"),
                None => "[index]".into(),
            },
        );
        self.walk(object, depth + 1, via, state);
        if let Some(key) = key {
            self.walk(key, depth + 1, via, state);
        }
    }

    /// The tracer's method extraction `x = [o, o.m]; x[1].call(x[0], …)`:
    /// the member `o.m` the call invokes (the array element, so its receiver
    /// resolves to `o`'s value before the extraction) and the call's real
    /// arguments.
    fn extracted_method_call(
        &self,
        call: &'a CallExpression<'a>,
    ) -> Option<(&'a Expression<'a>, &'a [Argument<'a>])> {
        let Expression::StaticMemberExpression(invoke) = call.callee.without_parentheses() else {
            return None;
        };
        if invoke.property.name != "call" {
            return None;
        }
        let pair_read = |expression: &'a Expression<'a>, index: f64| -> Option<&'a IdentifierReference<'a>> {
            let Expression::ComputedMemberExpression(member) = expression.without_parentheses() else {
                return None;
            };
            let Expression::NumericLiteral(literal) = &member.expression else {
                return None;
            };
            match member.object.without_parentheses() {
                Expression::Identifier(identifier) if literal.value == index => Some(identifier),
                _ => None,
            }
        };
        let method = pair_read(&invoke.object, 1.0)?;
        let receiver = pair_read(call.arguments.first()?.as_expression()?, 0.0)?;
        let symbol_of = |identifier: &IdentifierReference<'_>| {
            identifier
                .reference_id
                .get()
                .and_then(|reference| self.scoping.get_reference(reference).symbol_id())
        };
        let symbol = symbol_of(method)?;
        if symbol_of(receiver) != Some(symbol) {
            return None;
        }
        let Expression::ComputedMemberExpression(method_read) = invoke.object.without_parentheses() else {
            return None;
        };
        let pair = reaching::expression_value_at_use(self.scoping, self.nodes, &method_read.object).ok()?;
        let Expression::ArrayExpression(array) = pair.value.without_parentheses() else {
            return None;
        };
        let [object, member] = array.elements.as_slice() else {
            return None;
        };
        let (Some(object), Some(member)) = (object.as_expression(), member.as_expression()) else {
            return None;
        };
        let Expression::StaticMemberExpression(bound) = member.without_parentheses() else {
            return None;
        };
        if self.text(bound.object.span()) != self.text(object.span()) {
            return None;
        }
        Some((member, &call.arguments[1..]))
    }

    fn walk_call(
        &mut self,
        call: &'a CallExpression<'a>,
        span: Span,
        depth: usize,
        via: &[DefinitionHop],
        state: &mut WalkState,
    ) {
        let (callee, call_arguments) = match self.extracted_method_call(call) {
            Some((member, arguments)) => (member, arguments),
            None => (&call.callee, call.arguments.as_slice()),
        };
        let next = depth + 1;
        let callee = callee.without_parentheses();
        let arguments = |walker: &mut Self, state: &mut WalkState| {
            for argument in call_arguments {
                if let Some(argument) = argument.as_expression() {
                    walker.walk(argument, next, via, state);
                }
            }
        };
        // Global roots: randomness, clocks, host APIs and pure built-ins.
        if let Some(root) = root_global_name(callee, self.scoping) {
            let path = member_path(callee).unwrap_or_else(|| root.clone());
            let inside = self.inside_producer(span);
            match path.as_str() {
                "Math.random" | "crypto.getRandomValues" => {
                    let class = if inside {
                        SourceClass::MessageRandomness
                    } else {
                        SourceClass::SessionRandomness
                    };
                    self.push(state, class, span, Some(format!("{path}()")), via);
                    return;
                }
                "Date.now" | "performance.now" => {
                    self.push(state, SourceClass::HostClock, span, Some(format!("{path}()")), via);
                    return;
                }
                _ => {}
            }
            if PURE_BUILTINS.contains(&root.as_str()) {
                self.transform(state, path);
                arguments(self, state);
                return;
            }
            self.push(state, SourceClass::HostObservation, span, Some(format!("{path}()")), via);
            arguments(self, state);
            return;
        }
        if matches!(callee, Expression::StaticMemberExpression(_) | Expression::ComputedMemberExpression(_))
            && !self.is_context_value(callee, 0)
            && member_object_of(callee).is_some_and(|object| !matches!(object.without_parentheses(), Expression::Identifier(_)) || self.host_rooted(object, 0))
            && self.host_rooted(callee, 0)
        {
            let path = member_path(callee).unwrap_or_else(|| self.text(callee.span()));
            let class = if path.ends_with(".now") && (path.contains("Date") || path.contains("performance")) {
                SourceClass::HostClock
            } else {
                SourceClass::HostObservation
            };
            self.push(state, class, span, Some(format!("{path}()")), via);
            arguments(self, state);
            return;
        }
        // A method of a computed value: transform of the receiver.
        if let Some(method) = member_name(callee) {
            if method == "call" || method == "apply" {
                if let Some(target) = member_object_of(callee) {
                    if let Some(label) = self.helper_label(target) {
                        self.transform(state, label);
                        arguments(self, state);
                        return;
                    }
                }
            }
            if let Some(object) = member_object_of(callee) {
                self.transform(state, format!(".{method}()"));
                self.walk(object, next, via, state);
                arguments(self, state);
                return;
            }
        }
        if let Some(label) = self.helper_label(callee) {
            self.transform(state, label);
            arguments(self, state);
            return;
        }
        match callee {
            Expression::Identifier(identifier) => {
                let symbol = identifier
                    .reference_id
                    .get()
                    .and_then(|reference| self.scoping.get_reference(reference).symbol_id());
                if let Some(symbol) = symbol {
                    self.transform(state, format!("call {}", identifier.name));
                    if !self.walk_callee_definition(symbol, depth, via, state) {
                        self.push(
                            state,
                            SourceClass::ProgramCall,
                            span,
                            Some(self.text(callee.span())),
                            via,
                        );
                    }
                    arguments(self, state);
                } else {
                    self.push(state, SourceClass::HostObservation, span, Some(format!("{}()", identifier.name)), via);
                    arguments(self, state);
                }
            }
            _ => {
                self.push(state, SourceClass::Opaque, span, Some(self.text(callee.span())), via);
                arguments(self, state);
            }
        }
    }

    /// A call through a local whose single definition is a host function
    /// reference (`now = ctx.global.Date.now`, `D = performance.now`) or a
    /// one-statement function returning an expression: walk that instead of
    /// stopping at an opaque program call. Clock APIs become `HostClock`.
    fn walk_callee_definition(
        &mut self,
        symbol: SymbolId,
        depth: usize,
        via: &[DefinitionHop],
        state: &mut WalkState,
    ) -> bool {
        if !state.visited.insert(symbol) {
            return true;
        }
        let declaration = self.scoping.symbol_declaration(symbol);
        let returned: Option<&'a Expression<'a>> = match self.nodes.kind(declaration) {
            AstKind::Function(function) => function.body.as_ref().and_then(|body| single_return(&body.statements)),
            AstKind::VariableDeclarator(_) => {
                let definitions = self.definitions(symbol);
                match definitions.as_slice() {
                    [definition] => match definition.value.map(Expression::without_parentheses) {
                        Some(Expression::ArrowFunctionExpression(arrow)) => {
                            single_return(&arrow.body.statements).or_else(|| {
                                if arrow.expression {
                                    match arrow.body.statements.first() {
                                        Some(Statement::ExpressionStatement(statement)) => Some(&statement.expression),
                                        _ => None,
                                    }
                                } else {
                                    None
                                }
                            })
                        }
                        Some(Expression::FunctionExpression(function)) => {
                            function.body.as_ref().and_then(|body| single_return(&body.statements))
                        }
                        Some(value @ (Expression::StaticMemberExpression(_) | Expression::ComputedMemberExpression(_))) => {
                            // A host function held in a local: the call reads it.
                            Some(value)
                        }
                        _ => None,
                    },
                    _ => None,
                }
            }
            _ => None,
        };
        let Some(returned) = returned else {
            return false;
        };
        let before = state.provenance.sources.len();
        self.walk(returned, depth + 1, via, state);
        for source in &mut state.provenance.sources[before..] {
            let clock = source.api.as_deref().is_some_and(|api| {
                api.ends_with(".now") || api.ends_with(".now()") || api == "Date" || api.ends_with(".Date")
            });
            if clock && source.class == SourceClass::HostObservation {
                source.class = SourceClass::HostClock;
            }
        }
        true
    }

    /// The encoding helper a callee resolves to, through its declaration.
    fn helper_label(&self, callee: &Expression<'_>) -> Option<&'static str> {
        let Expression::Identifier(identifier) = callee.without_parentheses() else {
            return None;
        };
        let symbol = self
            .scoping
            .get_reference(identifier.reference_id.get()?)
            .symbol_id()?;
        let declaration = self.scoping.symbol_declaration(symbol);
        let function_span = match self.nodes.kind(declaration) {
            AstKind::Function(function) => function.span,
            AstKind::VariableDeclarator(declarator) => declarator.init.as_ref()?.span(),
            _ => return None,
        };
        self.helpers.label(function_span)
    }
}

/// The returned expression of a body that is exactly `return expr;`.
fn single_return<'a>(statements: &'a [Statement<'a>]) -> Option<&'a Expression<'a>> {
    match statements {
        [Statement::ReturnStatement(statement)] => statement.argument.as_ref(),
        _ => None,
    }
}

/// One write of a binding.
struct Definition<'a> {
    /// The stored value; `None` for `x++` / `--x`.
    value: Option<&'a Expression<'a>>,
    compound: bool,
    span: Span,
    node: NodeId,
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

fn enclosing_function(nodes: &AstNodes<'_>, node: NodeId) -> Option<NodeId> {
    let mut current = node;
    loop {
        let parent = nodes.parent_id(current);
        if parent == current {
            return None;
        }
        current = parent;
        if matches!(
            nodes.kind(current),
            AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
        ) {
            return Some(current);
        }
    }
}

fn is_static_literal(expression: &Expression<'_>) -> bool {
    match expression.without_parentheses() {
        Expression::NumericLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_) => true,
        Expression::UnaryExpression(unary) => is_static_literal(&unary.argument),
        Expression::ArrayExpression(array) => array
            .elements
            .iter()
            .all(|element| element.as_expression().is_some_and(is_static_literal)),
        Expression::ObjectExpression(_) => true,
        _ => false,
    }
}

/// The unbound global at the root of a member chain or call callee.
fn root_global_name(expression: &Expression<'_>, scoping: &Scoping) -> Option<String> {
    match expression.without_parentheses() {
        Expression::Identifier(identifier) => {
            let bound = identifier
                .reference_id
                .get()
                .and_then(|reference| scoping.get_reference(reference).symbol_id())
                .is_some();
            // `arguments` is the enclosing function's own argument list, not
            // a global.
            (!bound && identifier.name != "arguments").then(|| identifier.name.to_string())
        }
        Expression::StaticMemberExpression(member) => root_global_name(&member.object, scoping),
        Expression::ComputedMemberExpression(member) => root_global_name(&member.object, scoping),
        _ => None,
    }
}

/// `a.b["c"].d` as `a.b.c.d` when every step is a static name.
fn member_path(expression: &Expression<'_>) -> Option<String> {
    match expression.without_parentheses() {
        Expression::Identifier(identifier) => Some(identifier.name.to_string()),
        Expression::StaticMemberExpression(member) => {
            Some(format!("{}.{}", member_path(&member.object)?, member.property.name))
        }
        Expression::ComputedMemberExpression(member) => match member.expression.without_parentheses() {
            Expression::StringLiteral(literal) => {
                Some(format!("{}.{}", member_path(&member.object)?, literal.value))
            }
            _ => Some(format!("{}[]", member_path(&member.object)?)),
        },
        _ => None,
    }
}

fn member_name(expression: &Expression<'_>) -> Option<String> {
    match expression.without_parentheses() {
        Expression::StaticMemberExpression(member) => Some(member.property.name.to_string()),
        Expression::ComputedMemberExpression(member) => match member.expression.without_parentheses() {
            Expression::StringLiteral(literal) => Some(literal.value.to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn member_object_of<'a>(expression: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    match expression.without_parentheses() {
        Expression::StaticMemberExpression(member) => Some(&member.object),
        Expression::ComputedMemberExpression(member) => Some(&member.object),
        _ => None,
    }
}

// ===========================================================================
// Paired traces
// ===========================================================================

/// Align `report` with `other`, a second trace of the same raw build (the
/// caller asserts the pairing; it is an analysis join, not a recognition
/// rule). A field with no host source here whose aligned field reads a host
/// source there has its literals marked: `TraceFoldedObservation` when the
/// field is literal-only, `PossiblyTraceFolded` when it has other inputs too.
///
/// Alignment is refused unless the two producer inventories are equal and,
/// per descriptor, the emissions carry the same ordered write kinds. The
/// literal is never replaced, and its exact derivation is not asserted: the
/// paired field's own classes and APIs are attached as evidence.
pub fn pair_trace_folded_literals(report: &mut SignalContractReport, other: &SignalContractReport) {
    let mut summary = PairingSummary {
        other_sha256: other.source.sha256.clone(),
        aligned: 0,
        skipped: Vec::new(),
        trace_folded: 0,
        provenance_mismatches: Vec::new(),
    };
    if report.producers != other.producers {
        summary.skipped.push((u32::MAX, "producer inventories differ".into()));
        report.pairing = Some(summary);
        return;
    }
    for contract in &mut report.signals {
        let Some(paired) = other.signal(contract.descriptor) else {
            summary.skipped.push((contract.descriptor, "missing in the paired trace".into()));
            continue;
        };
        let kinds = |emissions: &[EmissionContract]| -> Vec<Vec<&'static str>> {
            emissions
                .iter()
                .map(|emission| write_kinds(&emission.grammar))
                .collect()
        };
        if contract.emissions.len() != paired.emissions.len()
            || kinds(&contract.emissions) != kinds(&paired.emissions)
        {
            summary
                .skipped
                .push((contract.descriptor, "emission write kinds differ".into()));
            continue;
        }
        summary.aligned += 1;
        let mut folded_here = false;
        let mut mismatches: Vec<(Span, String, String)> = Vec::new();
        for (emission, paired_emission) in contract.emissions.iter_mut().zip(&paired.emissions) {
            let paired_writes = paired_emission.grammar.writes();
            let mut index = 0;
            emission.grammar.walk_mut(&mut |node| {
                let (GrammarNode::Number(write) | GrammarNode::String(write) | GrammarNode::Byte(write)) =
                    node
                else {
                    return;
                };
                let Some(other_write) = paired_writes.get(index) else {
                    return;
                };
                index += 1;
                let (Some(provenance), Some(other_provenance)) =
                    (write.provenance.as_mut(), other_write.provenance.as_ref())
                else {
                    return;
                };
                let is_host =
                    |class: &SourceClass| matches!(class, SourceClass::HostObservation | SourceClass::HostClock);
                // Bare global reads (an unbound identifier, e.g. a VM kernel
                // global) the paired field makes and this field does not.
                let bare_reads = |provenance: &Provenance| -> BTreeSet<String> {
                    provenance
                        .sources
                        .iter()
                        .filter(|source| is_host(&source.class))
                        .filter_map(|source| source.api.clone())
                        .filter(|api| !api.contains(['.', '(', '[', ' ']))
                        // Platform globals and built-ins are reachable in
                        // either representation; a fold hides a read of a
                        // global the platform does not define (the VM
                        // kernel's own environment).
                        .filter(|api| {
                            !HOST_ROOTS.contains(&api.as_str())
                                && !PURE_BUILTINS.contains(&api.as_str())
                                && !PLATFORM_GLOBALS.contains(&api.as_str())
                        })
                        .collect()
                };
                let missing: Vec<String> = bare_reads(other_provenance)
                    .difference(&bare_reads(provenance))
                    .cloned()
                    .collect();
                let has_literal = provenance.has(SourceClass::Literal);
                if missing.is_empty() || !has_literal {
                    if provenance.classes != other_provenance.classes {
                        mismatches.push((write.value_span, write.value.clone(), other_write.value.clone()));
                    }
                    return;
                }
                let origin = if provenance.classes == [SourceClass::Literal] {
                    LiteralOrigin::TraceFoldedObservation
                } else {
                    LiteralOrigin::PossiblyTraceFolded
                };
                for source in &mut provenance.sources {
                    if source.class == SourceClass::Literal {
                        source.literal_origin = Some(origin);
                    }
                }
                provenance.paired = Some(PairedField {
                    source_sha256: other.source.sha256.clone(),
                    value_span: other_write.value_span,
                    value: other_write.value.clone(),
                    classes: other_provenance.classes.clone(),
                    apis: other_provenance
                        .sources
                        .iter()
                        .filter_map(|source| source.api.clone())
                        .collect(),
                    missing_host_reads: missing,
                });
                summary.trace_folded += 1;
                folded_here = true;
            });
        }
        for (span, here, there) in mismatches {
            summary.provenance_mismatches.push(ProvenanceMismatch {
                descriptor: contract.descriptor,
                value_span: SourceSpan::new(span.start, span.end),
                value: here,
                paired_value: there,
            });
        }
        if folded_here && !contract.input_classes.contains(&SourceClass::HostObservation) {
            contract.input_classes.push(SourceClass::HostObservation);
            contract.input_classes.sort();
        }
    }
    report.pairing = Some(summary);
}

fn write_kinds(grammar: &GrammarNode) -> Vec<&'static str> {
    let mut kinds = Vec::new();
    grammar.walk(&mut |node| match node {
        GrammarNode::Number(_) => kinds.push("number"),
        GrammarNode::String(_) => kinds.push("string"),
        GrammarNode::Byte(_) => kinds.push("byte"),
        _ => {}
    });
    kinds
}

// ===========================================================================
// Text rendering
// ===========================================================================

/// A human-readable summary: inventory, coverage, then one block per
/// descriptor with each emission's grammar and the input classes.
pub fn render_summary(report: &SignalContractReport) -> String {
    let mut out = String::new();
    let producers = &report.producers;
    out.push_str(&format!(
        "source sha256={} bytes={}\n",
        report.source.sha256, report.source.bytes
    ));
    out.push_str(&format!(
        "producers: descriptors={} groups={} rows={:?} grouped={} direct={:?} unrouted={:?} multiply_routed={:?}\n",
        producers.descriptor_count,
        producers.group_count,
        producers.row_lengths,
        producers.grouped_descriptors,
        producers.direct_descriptors,
        producers.unrouted,
        producers.multiply_routed,
    ));
    let coverage = &report.coverage;
    out.push_str(&format!(
        "coverage: complete={} partial={} unsupported={} emissions={} lifecycle={} writes={} in_grammar={} excluded={} opaque={} unresolved={}\n",
        coverage.complete,
        coverage.partial,
        coverage.unsupported,
        coverage.emissions,
        coverage.lifecycle_sites,
        coverage.writes,
        coverage.writes_in_grammar,
        coverage.writes_excluded,
        coverage.opaque_boundaries,
        coverage.unresolved_nodes,
    ));
    for (code, count) in &coverage.reasons {
        out.push_str(&format!("  reason {code}: {count} signals\n"));
    }
    if let Some(pairing) = &report.pairing {
        out.push_str(&format!(
            "pairing: other={} aligned={} skipped={} trace_folded={} provenance_mismatches={}\n",
            pairing.other_sha256,
            pairing.aligned,
            pairing.skipped.len(),
            pairing.trace_folded,
            pairing.provenance_mismatches.len()
        ));
    }
    for contract in &report.signals {
        let producer = match &contract.producer.kind {
            ProducerKind::Grouped {
                group, position, ..
            } => format!("group {group}[{position}]"),
            ProducerKind::Direct { .. } => "direct".to_string(),
            ProducerKind::Unrouted => "unrouted".to_string(),
        };
        let errors = contract
            .lifecycle
            .iter()
            .filter(|site| site.method == EmitterMethod::Error)
            .count();
        out.push_str(&format!(
            "\nD{} {:?} {} emissions={} error={} complete={} inputs={:?}\n",
            contract.descriptor,
            contract.status,
            producer,
            contract.emissions.len(),
            errors,
            contract.lifecycle.len() - errors,
            contract.input_classes,
        ));
        for emission in &contract.emissions {
            out.push_str(&format!(
                "  @{} {:?}: {}\n",
                emission.site.start,
                emission.phase,
                emission.rendered
            ));
        }
        for reason in &contract.reasons {
            out.push_str(&format!(
                "  ! {}{} {}\n",
                reason.code,
                reason
                    .span
                    .map(|span| format!(" @{}", span.start))
                    .unwrap_or_default(),
                reason.detail
            ));
        }
    }
    out
}
