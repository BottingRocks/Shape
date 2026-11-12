//! Source-driven extraction of generator *input* facts from a recovered program.
//!
//! This module walks the emitted `program.js` for a capture and reconstructs, for
//! every signal, the ordered write sequence that builds its byte buffer, the raw
//! 4-byte prefix seed, the per-emission static XOR literal, and the runtime
//! dependencies the run needs (mask closure, session seed, host measurement).
//!
//! # What this is (and is not)
//!
//! * **Is**: a real Oxc + semantic-analysis extractor. It resolves identifier
//!   references to their lexical bindings (`SymbolId`), follows observer wiring
//!   through the wave / direct-ctor collector functions, and reproduces the
//!   *JavaScript evaluation order* of nested writer calls.
//! * **Is not**: a codec, generator, renamer, or classifier. It never executes
//!   the recovered JavaScript. Nothing here is keyed by a generated name: every
//!   fact is anchored to a resolved binding, a source span, and the structural
//!   role that produced it. String names appear only as descriptive labels in
//!   the evidence.
//!
//! # Recognition model
//!
//! The core program-properties extractor already identifies the generated
//! helpers by their structure (not their names). This module *consumes* those
//! identities as spans and does the source-level wire reconstruction:
//!
//! * `helpers` — the declaration/identifier spans of the NUMBER writer, the
//!   STRING writer factory, the per-signal PRNG factory, and (optionally) the
//!   transaction hash helper.
//! * `collectors` — the collector function spans plus, for wave collectors, the
//!   `slot -> signal` observer positions, or, for a direct-ctor collector, the
//!   single signal index. Both come from the registry/wave arrays, not from
//!   names.
//!
//! For each collector this module:
//!
//! 1. resolves the observer expression(s) from `arguments[0]` /
//!    `arguments[0][k]` and their in-function aliases (scope-aware, so a
//!    same-spelled local in another function never misleads),
//! 2. finds every `observer.next(payload)` emission, in its `next(...)`,
//!    `next.call(recv, ...)` and `next.apply(recv, [...])` forms,
//! 3. resolves the emitted payload to its byte buffer or its raw named object,
//! 4. replays the collector body in evaluation order, keeping the writes to that
//!    buffer as a `SEQUENCE` / `REPEAT` / `CHOICE` tree,
//! 5. links the buffer's prefix array back to its `Math.random()*2^32` seed word
//!    and the writes' first argument back to the `PRNG(seed ^ C)` factory call,
//!    extracting the static `C` literal for *that* emission.
//!
//! # Emission-scoped write attribution
//!
//! Writes are attributed to an emission from two coordinates at once:
//!
//! * **path** — the emission snapshots the ordered writes that ran *before* it
//!   on its own execution path. The snapshot is taken at the emission call, so
//!   it already contains the writes of every enclosing sequential statement and
//!   of the conditional arm being scanned; it is never a prefix of a temporary
//!   `if`/loop body log, so a common write followed by an arm write before a
//!   `next` keeps both. A buffer that is filled, emitted, then filled again
//!   reports the two schemas separately instead of the union, and each write
//!   keeps its own mask on its own leaf while the emission's [`MaskSeed`] is the
//!   mask in force at the emission (the last write's).
//! * **function scope** — writes are recorded per enclosing function body. A
//!   write inside a callback is never flattened into the declaration position of
//!   a sibling function body. When an emission's buffer is written only from a
//!   *different* function body, the order cannot be proven from source and the
//!   emission keeps a non-empty `Unknown`/`Unresolved` instead of a combined
//!   confirmed order. A nested ordinary function has its own `arguments`, so its
//!   `arguments[0]` never names the collector's observer; an arrow keeps the
//!   enclosing `arguments` and a bound observer captured from the collector is
//!   still followed.
//!
//! # Integration surface
//!
//! The integration layer fills one [`SignalLayoutRequest`] from the core
//! extractor's structural facts and calls [`extract_signal_layouts`]:
//!
//! * [`HelperBindings`] — identifier spans for the NUMBER writer, the STRING
//!   writer factory, and the PRNG factory, plus an optional hash helper. Pass
//!   the span of the declaration *or* any reference; the module resolves it
//!   through plain identifier aliases (`x = writeNumber`) to the binding itself.
//! * [`CollectorRequest`] — one per collector function, with [`ObserverWiring`]
//!   mapping wave slots to signal indices or naming a single direct signal.
//! * The result is a [`SignalLayoutReport`] of per-signal emissions
//!   ([`Emission`]) with the raw/generic [`EmissionVariant`], the [`PrefixSeed`],
//!   the [`MaskSeed`], the ordered [`WireOp`] tree, and [`Unresolved`] entries
//!   for anything the extractor declined to guess.
//!
//! Every value carried out is a source slice or a resolved binding, never an
//! executed result. The NUMBER writer, the PRNG factory, the hash helper, and
//! the closure returned by the STRING writer factory are all identified by
//! resolved [`SymbolId`], so a write keeps its type and mask across renaming and
//! through local aliases.
//!
//! # Integration API changes
//!
//! This revision changes the public surface in ways the integration worker must
//! absorb (the module previously compiled only through the `#[path]` test):
//!
//! * [`HelperBindings`] gained `hash: Option<Span>` — the transaction hash
//!   helper. A call to it is typed `NUMBER` in generic-object field inference.
//!   Pass the identifier span of the binding that is *called* (an `x = hash`
//!   alias is followed), or the hash function's own declaration span.
//! * `WireOp::Repeat` now carries a [`LoopControl`] instead of a bare
//!   `count: String`. [`LoopControl`] preserves the loop `kind`, `init`, `test`,
//!   `update`, and full header `source`, and exposes `count: Option<u64>` which
//!   is `Some` only when the loop is provably closed from the AST. A boolean
//!   test like `i < 3` is never a numeric count.
//! * The 4-byte prefix is emitted as four individual `BYTE` leaves (each
//!   `byte_width == 1`, each retaining its own `seed >> n & 255` expression) or,
//!   generally, one leaf per prefix-array element. No `BYTE` leaf is ever typed
//!   with a width greater than one.
//! * `next.apply(receiver, [payload])` is recognized only when the argument list
//!   is a literal array with a single element; any other shape (a multi-element
//!   or non-literal array) is kept as an explicit `Unknown`/`Unresolved`.
//! * The NUMBER writer, PRNG factory, and hash helper are matched through plain
//!   identifier aliases (`var nw = writeNumber; nw(...)`), and a binding that
//!   holds the closure returned by the STRING writer factory
//!   (`var sw = makeStringWriter(mask); sw(...)`) types its write `STRING`, by
//!   resolved [`SymbolId`] — so types and masks survive renaming.
//! * A nested ordinary function has its own `arguments` and its own formals and
//!   cannot name the collector's observer; only the collector's own scope, an
//!   arrow (which keeps the enclosing `arguments` and captures the collector's
//!   bindings), an explicitly forwarded scheduled callback, or an observer
//!   binding captured from the collector qualifies.
//! * Generic-object field kinds are inferred structurally (string literals,
//!   `String.prototype` `join`/`toString`/`slice`/... , location/DOM string
//!   properties, hash `NUMBER`). Unproven fields stay `Unknown`. A string-keyed
//!   field assignment also defines the field, so an object whose literal lives in
//!   an enclosing scope is still recovered from its assignments.
//!
//! # Collector request ABI
//!
//! A [`CollectorRequest::function`] span is matched by its exact start against
//! either a [`Function`] (declaration or expression) or an
//! [`ArrowFunctionExpression`] in the same parse. Both roots are scanned with
//! the same ordered engine, including an arrow's expression body.
//!
//! The *observer position* of a root is source-resolved, never name-based:
//!
//! * The **first plain formal parameter** of the root is its semantic observer
//!   root: the single observer for [`ObserverWiring::Direct`], the observer
//!   list for [`ObserverWiring::WaveSlots`] (`param[0]`, `param[1]`, ...).
//!   "Plain" means a bare `BindingIdentifier` with no initializer, rest,
//!   destructuring pattern, decorator or optional marker. A source alias
//!   (`const sink = observer`) and a capturing arrow keep the resolved binding.
//! * `arguments[0]` / `arguments[0][slot]` remain supported when the root is an
//!   **ordinary function**, because that function owns `arguments`. A *written*
//!   first formal is not seeded as the observer root (an explicit
//!   `observer_parameter_mutated` unresolved is recorded); `arguments[]` remains
//!   independently valid there.
//! * An **arrow root** does not own `arguments` — an arrow keeps the enclosing
//!   function's `arguments` — so `arguments[...]` inside an arrow root is never
//!   interpreted as the arrow's observer. Only its formals and captured
//!   observer bindings are used.
//! * A **nested ordinary function** owns a fresh `arguments` and fresh formals;
//!   its first formal is never the collector observer, and its `arguments[0]`
//!   is accepted only when the collector explicitly hands it to an
//!   `arguments[...]` task scheduler (the existing forwarding rule). An
//!   unforwarded nested function is left unresolved rather than guessed.
//! * Unknown/destructured/default/rest first parameters fail closed: no observer
//!   root is seeded for them and an `observer_parameter_unsupported` unresolved
//!   carries the exact span. Reassigning an observer alias stops it naming the
//!   observer, so an emission after such a write is never attributed.
//!
//! # Ownership boundary
//!
//! This file is a standalone module. It deliberately avoids `crate::` paths so
//! it compiles both as part of the library (as `signals::extract::layout`) and, until that integration lands, as a module included by
//! `tests/program_signal_layouts.rs` through `#[path]`. All input/output types
//! are owned by this module; the integration layer adapts the core extractor's
//! facts into [`SignalLayoutRequest`].
//!
//! # Known limits
//!
//! * A buffer written in one function body and emitted from another keeps a
//!   non-empty `Unknown` (`write_order_crosses_function_scope`): the write order
//!   is not provable from source, so it is not reported as a confirmed sequence.
//! * Only write paths reachable through the recognized NUMBER/STRING helpers and
//!   direct `array[i] = ...` stores are typed; other paths become explicit
//!   `Unknown`/`Unresolved` facts rather than silent drops.
//! * Generic named-object field types are inferred only from expressions whose
//!   result type is provable from source; otherwise the field stays `Unknown`
//!   and the encoder rule is reported as unapplied.
//! * A store helper's returned-parameter proof is attached to a binding only
//!   while that binding is single-origin and never written. A reassignment,
//!   update, captured nested write or duplicate/conflicting declaration of the
//!   helper binding leaves the payload unresolved: a `SymbolId` proves which
//!   binding is called, not which function it currently holds.
//! * The session/host runtime boundary is not traversed here; only the
//!   per-emission mask factory, its static XOR literal, and the seed word are
//!   surfaced as the runtime dependencies.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_parser::{ParseOptions, Parser};
use oxc_semantic::{AstNodes, Scoping, SemanticBuilder, SymbolId};
use oxc_span::{GetSpan, SourceType, Span};
use oxc_syntax::operator::{
    AssignmentOperator, BinaryOperator, LogicalOperator, UnaryOperator, UpdateOperator,
};
use oxc_syntax::scope::ScopeFlags;
use serde::{Deserialize, Serialize};

use super::contracts::{
    ChoiceArm, ChoiceKind, GrammarNode, LoopDirection, MaskForm, PrefixNode, PrefixSpelling,
    WriteNode,
};
use crate::signals::schema::SourceSpan;

/// Serialize an Oxc [`Span`] as an explicit `{start,end}` byte pair so the
/// signal report stays portable across parses and process boundaries.
pub mod span_serde {
    use oxc_span::Span;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(span: &Span, serializer: S) -> Result<S::Ok, S::Error> {
        (span.start, span.end).serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Span, D::Error> {
        let (start, end) = <(u32, u32)>::deserialize(deserializer)?;
        Ok(Span::new(start, end))
    }
}

// ===========================================================================
// Bounds
// ===========================================================================

/// Maximum call-argument chain depth [`CollectorScanner::value_symbol`] follows
/// before failing closed. The AST depth already bounds it; this makes the budget
/// explicit and small.
const VALUE_SYMBOL_MAX_DEPTH: usize = 16;

/// Deterministic extraction bounds. Extraction stops descending and records a
/// `truncated` flag plus an [`Unresolved`] entry rather than doing unbounded
/// work on adversarial input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignalLayoutLimits {
    pub max_source_bytes: usize,
    pub max_collectors: usize,
    pub max_emissions: usize,
    pub max_wire_depth: usize,
    pub max_wire_nodes: usize,
}

impl Default for SignalLayoutLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 64 * 1024 * 1024,
            max_collectors: 4096,
            max_emissions: 8192,
            max_wire_depth: 64,
            max_wire_nodes: 200_000,
        }
    }
}

// ===========================================================================
// Input surface
// ===========================================================================

/// Declaration/identifier spans of the generated wire helpers, resolved to
/// lexical bindings by this module. Any helper may be absent; a missing helper
/// simply means those writes stay untyped instead of being misclassified.
///
/// Each role lists **every** proven binding, not one. A capture that traces
/// both the launcher and the worker thread carries one copy of each encoder
/// helper per thread, and each collector closure captures the copy that was in
/// scope where it was created. Identity is resolved at the use site (the
/// callee's lexical binding must be one of the listed helpers), so duplicated
/// but independently aliased copies type their own writes without the
/// extractor having to pick a single "the" writer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HelperBindings {
    /// NUMBER writers: `writer(mask, value, buffer, offset)`.
    pub number_writer: Vec<Span>,
    /// STRING writer factories: `writer(mask)(value, buffer, offset)`.
    pub string_writer: Vec<Span>,
    /// Per-signal PRNG factories: `prng(seed ^ static_literal)`.
    pub prng_factory: Vec<Span>,
    /// Transaction hash helper: `hash(text)` -> integer. Optional; when absent a
    /// hash-shaped call stays untyped instead of being guessed.
    pub hash: Option<Span>,
}

/// `arguments[0][slot]` observer position for a wave collector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObserverSlot {
    pub slot: u32,
    pub signal: u32,
}

/// How a collector receives its observers. Both forms are resolved from the
/// collector root's first plain formal parameter (see the module's "Collector
/// request ABI" section); `arguments[0]` is the equivalent source form for an
/// ordinary-function root.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObserverWiring {
    /// Wave collector: the observer list root (`observer[i]` / `arguments[0][i]`)
    /// maps each list slot to the signal index it collects.
    WaveSlots(Vec<ObserverSlot>),
    /// Direct-ctor collector: the observer root (`observer` / `arguments[0]`) is
    /// a single observer.
    Direct { signal: u32 },
}

/// One collector to scan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CollectorRequest {
    /// The collector root start. Matched against either `Function::span.start`
    /// (an ordinary declaration/expression) or
    /// `ArrowFunctionExpression::span.start` (an arrow callback), so nested
    /// collectors and same-span helpers stay distinct. See the module's
    /// "Collector request ABI" section for which formal is the observer root.
    pub function: Span,
    pub observers: ObserverWiring,
    /// How this producer delivers its payload.
    pub site: EmissionSite,
    /// The reply parameter this producer forwards from a VM worker. Only an
    /// emission of exactly that identifier may be payload-less; its siblings
    /// keep their own gaps.
    pub forwarded_parameter: Option<String>,
}

/// How a collector hands its payload to the runtime.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EmissionSite {
    /// `observer.next(...)`, the ordinary collector route.
    #[default]
    ObserverNext,
    /// `postMessage(payload)` in the VM worker function a launcher posts to.
    /// The launcher half only forwards the reply, so its wire lives here.
    WorkerReply,
    /// The launcher half of a worker signal: it forwards a value it did not
    /// build, so its own emissions carry no wire and are not a gap.
    WorkerForward,
}

/// The full extraction request.
#[derive(Clone, Debug)]
pub struct SignalLayoutRequest<'s> {
    pub source: &'s str,
    pub helpers: HelperBindings,
    pub collectors: Vec<CollectorRequest>,
    pub limits: SignalLayoutLimits,
}

// ===========================================================================
// Output surface
// ===========================================================================

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignalLayoutError {
    Parse(String),
    Limit { what: &'static str, limit: usize },
    TooLarge { bytes: usize },
}

impl std::fmt::Display for SignalLayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(message) => write!(f, "signal-layout parse failed: {message}"),
            Self::Limit { what, limit } => {
                write!(f, "signal-layout limit exceeded for {what} ({limit})")
            }
            Self::TooLarge { bytes } => write!(f, "signal-layout source too large ({bytes} bytes)"),
        }
    }
}

impl std::error::Error for SignalLayoutError {}

/// A wire leaf. Only `STRING`, `NUMBER`, and `BYTE` leaves exist; containers
/// (`Sequence` / `Repeat` / `Choice`) organize them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeafKind {
    String,
    Number,
    Byte,
}

/// One wire leaf with the exact source anchor that produced it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Leaf {
    pub kind: LeafKind,
    #[serde(with = "span_serde")]
    pub span: Span,
    /// Descriptive label of the buffer binding (never an identity key).
    pub buffer: Option<String>,
    /// Descriptive field label for a raw named-object field.
    pub label: Option<String>,
    /// True when the byte is stored as `expr ^ prng()`.
    pub masked: bool,
    /// The value expression text (NUMBER / STRING value, or one prefix byte).
    pub value: Option<String>,
    /// Where this field's value came from, when it is an indexed read of the
    /// collected value (`$AC[3]` in a derived collector).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<FieldSource>,
    /// The byte index expression text, for direct `arr[i]` writes.
    pub index: Option<String>,
    /// Static XOR operand when the write is `expr ^ literal`.
    pub xor_literal: Option<f64>,
    /// Byte width when this leaf consumes a fixed number of bytes. A single
    /// `BYTE` leaf is always exactly one byte (`Some(1)` for prefix elements);
    /// a multi-byte group is represented as several leaves, never one leaf with
    /// `byte_width > 1`.
    pub byte_width: Option<usize>,
}

/// The kind of loop header a [`WireOp::Repeat`] was built from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopKind {
    While,
    DoWhile,
    For,
    ForIn,
    ForOf,
}

/// The latest expression assigned to a local, and the local it was copied from
/// when that assignment was a plain identifier.
struct ValueBinding {
    text: String,
    alias: Option<SymbolId>,
}

/// The control evidence of a loop header. `count` is `Some` only when the loop's
/// iteration count is provable from the AST; a boolean `test` is retained in
/// `test`/`source` and is never presented as a numeric count.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopControl {
    pub kind: LoopKind,
    /// The initializer clause source (`for (…; …; …)`), when present.
    pub init: Option<String>,
    /// The condition clause source, when present.
    pub test: Option<String>,
    /// The update clause source, when present.
    pub update: Option<String>,
    /// The proven iteration count, only for closed loops.
    pub count: Option<u64>,
    /// The exact loop header source (`while (…)`, `for (…)`, `for (… of …)`).
    pub source: String,
    /// Counter direction derived from the loop's own test and update (or the
    /// leading decrement-and-break of a `for (;;)` body).
    #[serde(default)]
    pub direction: LoopDirection,
    /// The whole loop statement.
    #[serde(default, with = "span_serde")]
    pub span: Span,
}

/// Ordered wire operations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireOp {
    Sequence(Vec<WireOp>),
    Repeat {
        control: LoopControl,
        body: Box<WireOp>,
    },
    Choice(Vec<WireOp>),
    Leaf(Leaf),
    /// A write path this extractor could not type. Kept explicitly so an
    /// incomplete signal is never reported as complete.
    Unknown {
        #[serde(with = "span_serde")]
        span: Span,
        text: String,
        reason: String,
    },
}

/// The raw 4-byte big-endian prefix and the seed word behind it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PrefixSeed {
    pub seed_word: String,
    pub prefix_binding: Option<String>,
    /// Number of prefix bytes. The wire carries them as individual BYTE leaves.
    pub byte_width: usize,
    /// Exact source expression of each prefix byte. Empty when the prefix could
    /// not be read as an element list; the width is then zero.
    #[serde(default)]
    pub elements: Vec<String>,
}

/// The per-emission mask: `PRNG(seed_word ^ xor_literal)`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MaskSeed {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<SourceSpan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed_binding: Option<SourceSpan>,
    pub factory: Option<String>,
    pub seed_word: Option<String>,
    pub xor_literal: Option<f64>,
    /// Exact source text of the static XOR literal, when present.
    #[serde(default)]
    pub xor_source: Option<String>,
    /// Exact span of the static XOR literal in the input source.
    #[serde(default, with = "span_serde")]
    pub xor_span: Span,
    pub argument: String,
}

/// How the emission was routed to its signal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObserverRoute {
    WaveSlot(u32),
    Direct,
}

/// Whether the emission is a masked byte buffer or a raw named object.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmissionVariant {
    RawBuffer,
    GenericObject,
}

/// A single `observer.next(payload)` emission.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Emission {
    pub signal: u32,
    #[serde(with = "span_serde")]
    pub origin: Span,
    pub route: ObserverRoute,
    pub variant: EmissionVariant,
    pub prefix: Option<PrefixSeed>,
    pub mask: Option<MaskSeed>,
    pub wire: WireOp,
    /// The render order of a derived signal: the element of the collected value
    /// each field was read from, in write order (`$AC[3]` -> `3`).
    ///
    /// Present only when every write of this emission reads the same collected
    /// binding and there are at least two such fields. The prefix seed is not a
    /// collected field, so it is excluded from the proof. A computed field or
    /// an unknown write leaves this absent rather than guessed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<u32>>,
    /// Resolved session token-state binding label this emission's mask derives
    /// from, when the session hash chain is known.
    #[serde(default)]
    pub session_state: Option<String>,
    /// The typed contract grammar of this emission: the same writes as
    /// `wire`, with branch kinds and conditions, loop directions, positioned
    /// raw prefixes, logical byte values split from their masks, opaque
    /// buffer hand-offs and visible limits.
    #[serde(default = "GrammarNode::empty")]
    pub grammar: GrammarNode,
}

/// All emissions observed for one signal index. Several entries mean the signal
/// is produced by more than one stream / branch / nested container.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignalLayout {
    pub signal: u32,
    pub emissions: Vec<Emission>,
}

/// A fact the extractor declined to guess.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Unresolved {
    #[serde(with = "span_serde")]
    pub span: Span,
    pub reason: String,
    pub detail: String,
}

/// Every write the replay saw to one emitted buffer of one signal, and
/// whether some emission's grammar holds it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BufferAccounting {
    pub signal: u32,
    /// Descriptive label of the buffer binding.
    pub buffer: String,
    #[serde(with = "span_serde")]
    pub declaration: Span,
    /// Writes, prefix stores and opaque hand-offs of the buffer in the
    /// collector (all scopes and branches).
    pub writes: usize,
    pub in_grammar: usize,
    pub excluded: Vec<ExcludedWrite>,
}

/// A write to an emitted buffer that no emission's grammar contains.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExcludedWrite {
    #[serde(with = "span_serde")]
    pub span: Span,
    pub text: String,
    /// `other_function_scope`: written from a body no emission of the buffer
    /// runs in, so its order relative to an emission is not provable.
    /// `not_on_emission_path`: on no path that reaches an emission (after the
    /// last emission, or in an arm that cannot precede one).
    pub reason: String,
}

/// The extraction result.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct SignalLayoutReport {
    pub signals: Vec<SignalLayout>,
    pub unresolved: Vec<Unresolved>,
    pub collector_count: usize,
    pub emission_count: usize,
    pub truncated: bool,
    #[serde(default)]
    pub accounting: Vec<BufferAccounting>,
}

impl SignalLayoutReport {
    pub fn signal(&self, index: u32) -> Option<&SignalLayout> {
        self.signals.iter().find(|signal| signal.signal == index)
    }

    /// The single emission of `signal`, when it has exactly one.
    pub fn sole_emission(&self, index: u32) -> Option<&Emission> {
        let signal = self.signal(index)?;
        (signal.emissions.len() == 1).then(|| &signal.emissions[0])
    }
}

// ===========================================================================
// Entry point
// ===========================================================================

/// Extract per-signal wire layouts from a recovered program source.
pub fn extract_signal_layouts(
    request: &SignalLayoutRequest<'_>,
) -> Result<SignalLayoutReport, SignalLayoutError> {
    if request.source.len() > request.limits.max_source_bytes {
        return Err(SignalLayoutError::TooLarge {
            bytes: request.source.len(),
        });
    }
    if request.collectors.len() > request.limits.max_collectors {
        return Err(SignalLayoutError::Limit {
            what: "collectors",
            limit: request.limits.max_collectors,
        });
    }

    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, request.source, SourceType::unambiguous())
        .with_options(ParseOptions {
            preserve_parens: false,
            ..ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.errors.is_empty() {
        let mut messages: Vec<String> = parsed.errors.iter().map(ToString::to_string).collect();
        if messages.is_empty() {
            messages.push("parser panicked".into());
        }
        return Err(SignalLayoutError::Parse(messages.join("; ")));
    }
    let program = parsed.program;
    let (scoping, nodes) = SemanticBuilder::new()
        .with_check_syntax_error(false)
        .build(&program)
        .semantic
        .into_scoping_and_nodes();

    let index = IdentifierIndex::build(&scoping, &program);
    let aliases = AliasCollector::build(&scoping, &program);
    let returned_parameters = ReturnedParameterCollector::build(&scoping, &program);
    let writer_helpers = WriterHelperCollector::build(&scoping, request.source, &program);
    let sealed = SealedTuples::build(&scoping, &program);
    // A caller may hand in the span of a binding that is a direct alias of the
    // real helper (`x = writeNumber`); resolve through the alias chain so the
    // helper identity is the underlying function, not the spelling.
    let helper = |span: Span| {
        index
            .symbol_at(span)
            .map(|symbol| follow_aliases(&aliases, symbol))
    };
    let helpers = |spans: &[Span]| spans.iter().filter_map(|span| helper(*span)).collect();
    let helper_symbols = HelperSymbols {
        number_writer: helpers(&request.helpers.number_writer),
        string_writer: helpers(&request.helpers.string_writer),
        prng_factory: helpers(&request.helpers.prng_factory),
        hash: request.helpers.hash.and_then(helper),
    };

    let mut request_by_start: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (position, collector) in request.collectors.iter().enumerate() {
        request_by_start
            .entry(collector.function.start)
            .or_default()
            .push(position);
    }

    let mut visitor = LayoutVisitor {
        source: request.source,
        scoping: &scoping,
        nodes: &nodes,
        aliases: &aliases,
        helpers: helper_symbols,
        returned_parameters: &returned_parameters,
        writer_helpers: &writer_helpers,
        sealed: &sealed,
        collectors: &request.collectors,
        request_by_start,
        limits: request.limits,
        signals: BTreeMap::new(),
        unresolved: Vec::new(),
        accounting: Vec::new(),
        collector_count: 0,
        emission_count: 0,
        truncated: false,
        _marker: std::marker::PhantomData,
    };
    visitor.visit_program(&program);

    let LayoutVisitor {
        signals,
        mut unresolved,
        accounting,
        collector_count,
        emission_count,
        truncated,
        ..
    } = visitor;

    if request.collectors.len() > collector_count {
        for collector in request.collectors.iter().skip(collector_count) {
            unresolved.push(Unresolved {
                span: collector.function,
                reason: "collector_not_found".into(),
                detail: "no function started at the requested span".into(),
            });
        }
    }

    let signals = signals
        .into_iter()
        .map(|(signal, emissions)| SignalLayout { signal, emissions })
        .collect();

    Ok(SignalLayoutReport {
        signals,
        unresolved,
        collector_count,
        emission_count,
        truncated,
        accounting,
    })
}

// ===========================================================================
// Identifier index / alias index
// ===========================================================================

/// Bindings read only as `t[<numeric literal>]` (besides plain rebinding
/// `t = …`): a register tuple whose elements nothing replaces or reaches
/// through another name, so an empty-literal element is a buffer of its own.
struct SealedTuples<'s> {
    scoping: &'s Scoping,
    indexed: BTreeSet<SymbolId>,
    escaped: BTreeSet<SymbolId>,
}

impl SealedTuples<'_> {
    fn build(scoping: &Scoping, program: &Program<'_>) -> BTreeSet<SymbolId> {
        let mut walk = SealedTuples { scoping, indexed: BTreeSet::new(), escaped: BTreeSet::new() };
        walk.visit_program(program);
        walk.indexed.difference(&walk.escaped).copied().collect()
    }

    fn indexed_object(member: &ComputedMemberExpression<'_>) -> bool {
        matches!(member.object.without_parentheses(), Expression::Identifier(_))
            && matches!(member.expression.without_parentheses(), Expression::NumericLiteral(_))
    }
}

impl<'a> Visit<'a> for SealedTuples<'_> {
    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        if Self::indexed_object(member) {
            if let Expression::Identifier(identifier) = member.object.without_parentheses() {
                if let Some(symbol) = resolve_reference(identifier, self.scoping) {
                    self.indexed.insert(symbol);
                }
            }
            return;
        }
        walk::walk_computed_member_expression(self, member);
    }

    fn visit_assignment_target(&mut self, target: &AssignmentTarget<'a>) {
        // `t[k] = v` replaces an element.
        if let AssignmentTarget::ComputedMemberExpression(member) = target {
            if Self::indexed_object(member) {
                if let Expression::Identifier(identifier) = member.object.without_parentheses() {
                    if let Some(symbol) = resolve_reference(identifier, self.scoping) {
                        self.escaped.insert(symbol);
                    }
                }
                return;
            }
        }
        walk::walk_assignment_target(self, target);
    }

    fn visit_simple_assignment_target(&mut self, target: &SimpleAssignmentTarget<'a>) {
        if let SimpleAssignmentTarget::ComputedMemberExpression(member) = target {
            if Self::indexed_object(member) {
                if let Expression::Identifier(identifier) = member.object.without_parentheses() {
                    if let Some(symbol) = resolve_reference(identifier, self.scoping) {
                        self.escaped.insert(symbol);
                    }
                }
                return;
            }
        }
        walk::walk_simple_assignment_target(self, target);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        let Some(reference) = identifier.reference_id.get() else {
            return;
        };
        let reference = self.scoping.get_reference(reference);
        let Some(symbol) = reference.symbol_id() else {
            return;
        };
        // A plain rebinding `t = …` does not expose the old tuple.
        if reference.is_write() && !reference.is_read() {
            return;
        }
        self.escaped.insert(symbol);
    }
}

/// Maps an identifier span start to its resolved symbol, and every symbol to a
/// descriptive name. Uses scoping so references resolve to their binding.
#[derive(Default)]
struct IdentifierIndex {
    by_start: BTreeMap<u32, SymbolId>,
}

impl IdentifierIndex {
    fn build(scoping: &Scoping, program: &Program<'_>) -> Self {
        let mut index = IdentifierIndex::default();
        IdentifierCollector {
            scoping,
            index: &mut index,
        }
        .visit_program(program);
        index
    }

    fn symbol_at(&self, span: Span) -> Option<SymbolId> {
        self.by_start.get(&span.start).copied()
    }
}

struct IdentifierCollector<'s, 'i> {
    scoping: &'s Scoping,
    index: &'i mut IdentifierIndex,
}

impl<'s, 'a> Visit<'a> for IdentifierCollector<'s, '_> {
    fn visit_binding_identifier(&mut self, identifier: &BindingIdentifier<'a>) {
        if let Some(symbol) = identifier.symbol_id.get() {
            self.index.by_start.insert(identifier.span.start, symbol);
        }
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if let Some(symbol) = resolve_reference(identifier, self.scoping) {
            self.index.by_start.insert(identifier.span.start, symbol);
        }
    }

    /// A caller may hand in a helper's function-declaration span (which starts
    /// at the `function` keyword) instead of its identifier; resolve that too so
    /// the helper identity is robust to which span the core extractor reports.
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some(id) = function.id.as_ref() {
            if let Some(symbol) = id.symbol_id.get() {
                self.index
                    .by_start
                    .entry(function.span.start)
                    .or_insert(symbol);
            }
        }
        walk::walk_function(self, function, flags);
    }
}

/// Program-wide map of plain `x = y` / `var x = y` identifier aliases, used to
/// follow a called helper reference back to the helper binding it points at.
struct AliasCollector<'s, 'i> {
    scoping: &'s Scoping,
    aliases: &'i mut BTreeMap<SymbolId, SymbolId>,
    /// Every store of every binding, for [`FunctionAliasStores`].
    stores: BTreeMap<SymbolId, FunctionAliasStores>,
    /// Per enclosing function: how many conditional arms (`if` branches,
    /// `?:` arms, `&&`/`||`/`??` right sides, `switch` cases) the visit is in.
    guards: Vec<usize>,
}

/// The stores of one binding, split by whether they run unconditionally in
/// their function.
#[derive(Default)]
struct FunctionAliasStores {
    /// Unconditional stores: the identifier stored (`x = f` / `let x = f`),
    /// `None` for any other value.
    unguarded: Vec<Option<SymbolId>>,
    /// Stores inside a conditional arm of their function.
    guarded: usize,
    /// An update (`x++`, `x += …`) or a store this map does not classify.
    other: bool,
}

impl AliasCollector<'static, '_> {
    fn build(scoping: &Scoping, program: &Program<'_>) -> BTreeMap<SymbolId, SymbolId> {
        let mut aliases = BTreeMap::new();
        let mut collector = AliasCollector {
            scoping,
            aliases: &mut aliases,
            stores: BTreeMap::new(),
            guards: vec![0],
        };
        collector.visit_program(program);
        let stores = std::mem::take(&mut collector.stores);
        restore_guarded_function_aliases(&mut aliases, &stores, scoping);
        aliases
    }
}

/// A helper alias (`w = writeNumber`, stored once unconditionally) whose
/// binding is also stored inside a conditional arm elsewhere (`if (!buf)
/// w = other;`, `if (k === void 0) w = w;`, `if (o.flag) w = o.flag | 0;`)
/// keeps the helper. The obfuscated programs carry such stores under opaque
/// tests the tracer keeps; the source-order last store would otherwise
/// replace the helper for every call program-wide. Only a binding with
/// exactly one unconditional store, an identifier that resolves to a
/// declared function, and no update is restored.
fn restore_guarded_function_aliases(
    aliases: &mut BTreeMap<SymbolId, SymbolId>,
    stores: &BTreeMap<SymbolId, FunctionAliasStores>,
    scoping: &Scoping,
) {
    let mut restored = Vec::new();
    for (left, stores) in stores {
        if stores.other || stores.guarded == 0 {
            continue;
        }
        let [Some(right)] = stores.unguarded[..] else { continue };
        if right == *left || aliases.get(left) == Some(&right) {
            continue;
        }
        let target = follow_aliases(aliases, right);
        if scoping.symbol_flags(target).is_function() {
            restored.push((*left, right));
        }
    }
    for (left, right) in restored {
        aliases.insert(left, right);
    }
}

impl AliasCollector<'_, '_> {
    fn guarded(&self) -> bool {
        self.guards.last().is_some_and(|depth| *depth > 0)
    }

    fn record_store(&mut self, left: SymbolId, right: Option<SymbolId>) {
        let guarded = self.guarded();
        let entry = self.stores.entry(left).or_default();
        if guarded {
            entry.guarded += 1;
        } else {
            entry.unguarded.push(right);
        }
    }

    fn in_arm(&mut self, visit: impl FnOnce(&mut Self)) {
        if let Some(depth) = self.guards.last_mut() {
            *depth += 1;
        }
        visit(self);
        if let Some(depth) = self.guards.last_mut() {
            *depth -= 1;
        }
    }
}

impl<'s, 'a> Visit<'a> for AliasCollector<'s, '_> {
    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        if let AssignmentTarget::AssignmentTargetIdentifier(left) = &assignment.left {
            if let Some(left) = resolve_reference(left, self.scoping) {
                self.aliases.remove(&left);
                if assignment.operator == AssignmentOperator::Assign {
                    let mut stored = None;
                    if let Expression::Identifier(right) = &assignment.right {
                        if let Some(right) = resolve_reference(right, self.scoping) {
                            self.aliases.insert(left, right);
                            stored = Some(right);
                        }
                    }
                    self.record_store(left, stored);
                } else {
                    self.stores.entry(left).or_default().other = true;
                }
            }
        }
        walk::walk_assignment_expression(self, assignment);
    }

    fn visit_update_expression(&mut self, update: &UpdateExpression<'a>) {
        if let Some(left) = simple_assignment_target_root(&update.argument, self.scoping) {
            self.aliases.remove(&left);
            if let SimpleAssignmentTarget::AssignmentTargetIdentifier(_) = &update.argument {
                self.stores.entry(left).or_default().other = true;
            }
        }
        walk::walk_update_expression(self, update);
    }

    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if let BindingPattern::BindingIdentifier(id) = &declarator.id {
            if let Some(left) = id.symbol_id.get() {
                match declarator.init.as_ref() {
                    Some(Expression::Identifier(right)) => {
                        let right = resolve_reference(right, self.scoping);
                        if let Some(right) = right {
                            self.aliases.insert(left, right);
                        }
                        self.record_store(left, right);
                    }
                    Some(_) => self.record_store(left, None),
                    None => {}
                }
            }
        }
        walk::walk_variable_declarator(self, declarator);
    }

    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        self.guards.push(0);
        walk::walk_function(self, function, flags);
        self.guards.pop();
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.guards.push(0);
        walk::walk_arrow_function_expression(self, arrow);
        self.guards.pop();
    }

    fn visit_if_statement(&mut self, statement: &IfStatement<'a>) {
        self.visit_expression(&statement.test);
        self.in_arm(|this| this.visit_statement(&statement.consequent));
        if let Some(alternate) = &statement.alternate {
            self.in_arm(|this| this.visit_statement(alternate));
        }
    }

    /// The statements after `if (t) { …; return; }` (no `else`) run only
    /// when `t` is false: they are a conditional arm of their function like
    /// an `else` branch. Junk the tracer keeps behind such an always-taken
    /// early return must not count as an unconditional store.
    fn visit_statements(&mut self, statements: &oxc_allocator::Vec<'a, Statement<'a>>) {
        let mut arms = 0;
        for statement in statements {
            self.visit_statement(statement);
            if let Statement::IfStatement(early) = statement {
                if early.alternate.is_none() && always_returns(&early.consequent) {
                    arms += 1;
                    if let Some(depth) = self.guards.last_mut() {
                        *depth += 1;
                    }
                }
            }
        }
        if let Some(depth) = self.guards.last_mut() {
            *depth -= arms;
        }
    }

    fn visit_conditional_expression(&mut self, expression: &ConditionalExpression<'a>) {
        self.visit_expression(&expression.test);
        self.in_arm(|this| this.visit_expression(&expression.consequent));
        self.in_arm(|this| this.visit_expression(&expression.alternate));
    }

    fn visit_logical_expression(&mut self, expression: &LogicalExpression<'a>) {
        self.visit_expression(&expression.left);
        self.in_arm(|this| this.visit_expression(&expression.right));
    }

    fn visit_switch_case(&mut self, case: &SwitchCase<'a>) {
        self.in_arm(|this| walk::walk_switch_case(this, case));
    }
}

/// True when `statement` always leaves its function: a `return` or `throw`,
/// or a block whose last statement does.
fn always_returns(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::ReturnStatement(_) | Statement::ThrowStatement(_) => true,
        Statement::BlockStatement(block) => block.body.last().is_some_and(always_returns),
        _ => false,
    }
}

/// Maps a function binding to the start of its declaration, so an identifier
/// passed to a task scheduler can be related to the nested function body.
struct FunctionSpanCollector {
    by_symbol: BTreeMap<SymbolId, u32>,
}

impl FunctionSpanCollector {
    fn build(function: &Function<'_>) -> Self {
        let mut collector = Self {
            by_symbol: BTreeMap::new(),
        };
        collector.visit_function(function, ScopeFlags::Function);
        collector
    }

    fn build_arrow(arrow: &ArrowFunctionExpression<'_>) -> Self {
        let mut collector = Self {
            by_symbol: BTreeMap::new(),
        };
        collector.visit_arrow_function_expression(arrow);
        collector
    }
}

impl<'a> Visit<'a> for FunctionSpanCollector {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some(id) = function.id.as_ref() {
            if let Some(symbol) = id.symbol_id.get() {
                self.by_symbol.entry(symbol).or_insert(function.span.start);
            }
        }
        walk::walk_function(self, function, flags);
    }
}

/// Program-wide map from a function binding to the emitted store-helper shape it
/// proves. The emitted program builds a two-element `[aux, buffer]` payload with
/// such a helper:
///
/// ```text
/// function store(a, b, c, d) { a[b] = c; return d; }
/// let payload = [];
/// payload[1] = store(payload, 0, [], buffer);
/// sink.next(payload);
/// ```
///
/// The call value is the argument at the returned parameter position, which is
/// the emitted buffer. Recognition is deliberately narrow: the body must be
/// exactly one plain indexed store followed by a `return` of a distinct plain
/// formal parameter, and a scope-resolved safety scan must find no other write,
/// call, dynamic-scope construct or mapped-`arguments` use. Broad "every return
/// is the same parameter" is not enough: `d = other; return d` returns the same
/// lexical symbol but not the original argument, and `d.push(1); return d`
/// mutates the very buffer whose byte layout we would report.
///
/// The proof is attached to a binding only while that binding still holds the
/// declared function. A [`SymbolId`] identifies the binding, not its value, so
/// [`binding_holds_single_function`] additionally requires one declaration
/// origin and no write reference anywhere in the program.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ReturnedParameterProof {
    /// Formal position whose argument is the provable call value.
    returned: usize,
    /// Formal position of the one array the helper may index-store into.
    written: usize,
    /// Formal positions of the stored index and value (`a[b] = c`).
    index: usize,
    value: usize,
}

/// Collects [`ReturnedParameterProof`]s for the store-helper bindings of one
/// program. The proof lives on the binding only while it is a single-origin,
/// never-written binding (see [`binding_holds_single_function`]).
struct ReturnedParameterCollector;

impl ReturnedParameterCollector {
    fn build(
        scoping: &Scoping,
        program: &Program<'_>,
    ) -> BTreeMap<SymbolId, ReturnedParameterProof> {
        let mut walker = ReturnedParameterWalk {
            scoping,
            returned: BTreeMap::new(),
            origins: BTreeMap::new(),
            conflicted: BTreeSet::new(),
        };
        walker.visit_program(program);
        let mut returned = walker.returned;
        returned.retain(|symbol, _| binding_holds_single_function(scoping, *symbol));
        returned
    }
}

/// Registration walk for returned-parameter proofs. Each candidate keeps the
/// declaration origin it came from, so a second declaration of the same binding
/// invalidates the proof instead of the walker silently keeping the first one.
struct ReturnedParameterWalk<'s> {
    scoping: &'s Scoping,
    returned: BTreeMap<SymbolId, ReturnedParameterProof>,
    /// The one declaration span a binding's proof came from.
    origins: BTreeMap<SymbolId, Span>,
    /// Bindings claimed by more than one declaration origin.
    conflicted: BTreeSet<SymbolId>,
}

impl ReturnedParameterWalk<'_> {
    /// Record one declaration of `symbol`. The first origin seen wins the slot;
    /// a *different* origin for the same binding makes the reaching definition
    /// unprovable, so the binding is dropped entirely rather than picking the
    /// first declaration's proof.
    fn record(&mut self, symbol: SymbolId, origin: Span, proof: Option<ReturnedParameterProof>) {
        if self.conflicted.contains(&symbol) {
            return;
        }
        if let Some(previous) = self.origins.insert(symbol, origin) {
            if previous != origin {
                self.conflicted.insert(symbol);
                self.returned.remove(&symbol);
                return;
            }
        }
        if let Some(proof) = proof {
            self.returned.entry(symbol).or_insert(proof);
        }
    }
}

impl<'a> Visit<'a> for ReturnedParameterWalk<'_> {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) {
            let proof = returned_parameter_proof(function, self.scoping);
            self.record(symbol, function.span, proof);
        }
        walk::walk_function(self, function, flags);
    }

    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if let BindingPattern::BindingIdentifier(identifier) = &declarator.id {
            if let Some(symbol) = identifier.symbol_id.get() {
                // An initializer that is not one of the recognized function
                // forms still declares the binding; recording it as a proof-less
                // origin keeps a conflicting declaration from being ignored.
                let proof =
                    declarator
                        .init
                        .as_ref()
                        .and_then(|value| match value.without_parentheses() {
                            Expression::FunctionExpression(function) => {
                                returned_parameter_proof(function, self.scoping)
                            }
                            Expression::ArrowFunctionExpression(arrow) => {
                                returned_arrow_parameter_proof(arrow, self.scoping)
                            }
                            _ => None,
                        });
                self.record(symbol, declarator.span, proof);
            }
        }
        walk::walk_variable_declarator(self, declarator);
    }
}

/// A [`SymbolId`] is the identity of a *binding*, not evidence that the binding
/// still holds the function its declaration assigned:
///
/// ```text
/// function store(a, b, c, d) { a[b] = c; return d; }
/// store = other;                       // the binding no longer names the helper
/// payload[1] = store(payload, 0, [], buffer);
/// ```
///
/// The same holds for `let store = function (...) {...}; store =
/// unknownFactory();`, an update, a destructuring/`for-in` target, or a write
/// captured by a nested closure. Every such write is a resolved reference on the
/// binding, so no write reference is admitted. A redeclared binding has more
/// than one declaration origin and the definition that reaches a call is not
/// provable from source, so it is rejected as a whole. Both conditions are
/// source-level facts; nothing is executed.
fn binding_holds_single_function(scoping: &Scoping, symbol: SymbolId) -> bool {
    scoping.symbol_declarations(symbol).count() == 1
        && !scoping
            .get_resolved_references(symbol)
            .any(|reference| reference.is_write())
}

/// The formal parameter a function returns unchanged, proven from its exact
/// store-helper shape plus a scope-resolved unchanged-parameter scan.
fn returned_parameter_proof(
    function: &Function<'_>,
    scoping: &Scoping,
) -> Option<ReturnedParameterProof> {
    let parameters = plain_parameter_symbols(&function.params)?;
    let body = function.body.as_ref()?;
    returned_statement_proof(&body.statements, &parameters, scoping)
}

fn returned_arrow_parameter_proof(
    arrow: &ArrowFunctionExpression<'_>,
    scoping: &Scoping,
) -> Option<ReturnedParameterProof> {
    let parameters = plain_parameter_symbols(&arrow.params)?;
    returned_statement_proof(&arrow.body.statements, &parameters, scoping)
}

/// Only plain formal identifiers with no rest, default initializer, decorator
/// or duplicate binding are admitted. A default expression executes before the
/// body and can mutate another parameter; a rest/pattern parameter has no
/// single argument identity. Both are hazards this extractor declines to model.
fn plain_parameter_symbols(params: &FormalParameters<'_>) -> Option<Vec<SymbolId>> {
    if params.rest.is_some() {
        return None;
    }
    let mut symbols = Vec::with_capacity(params.items.len());
    let mut seen = BTreeSet::new();
    for parameter in &params.items {
        if !parameter.decorators.is_empty() || parameter.initializer.is_some() || parameter.optional
        {
            return None;
        }
        let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
            return None;
        };
        let symbol = identifier.symbol_id.get()?;
        if !seen.insert(symbol) {
            return None;
        }
        symbols.push(symbol);
    }
    (!symbols.is_empty()).then_some(symbols)
}

fn returned_statement_proof(
    statements: &[Statement<'_>],
    parameters: &[SymbolId],
    scoping: &Scoping,
) -> Option<ReturnedParameterProof> {
    // Every reachable `return` must yield the same formal parameter, and the
    // body must not fall through. This is necessary but not sufficient: the
    // parameter can still have been reassigned, so the checks below prove the
    // value is unchanged and the helper cannot mutate it opaquely.
    let returned = returned_statement_index(statements, parameters, scoping)?;
    let (written, store_returned, index, value) = store_helper_shape(statements, parameters, scoping)?;
    if returned != store_returned {
        return None;
    }
    if !parameter_is_unchanged(statements, parameters[returned], scoping) {
        return None;
    }
    Some(ReturnedParameterProof { returned, written, index, value })
}

/// The exact recognized helper grammar: one `array[index] = value;` store over
/// distinct plain formal parameters, immediately followed by `return returned;`.
/// The returned parameter is never the store's object, index or value, so the
/// helper's only modeled side effect is the indexed write into `written`.
fn store_helper_shape(
    statements: &[Statement<'_>],
    parameters: &[SymbolId],
    scoping: &Scoping,
) -> Option<(usize, usize, usize, usize)> {
    let [Statement::ExpressionStatement(expression), Statement::ReturnStatement(returned)] =
        statements
    else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = expression.expression.without_parentheses()
    else {
        return None;
    };
    if assignment.operator != AssignmentOperator::Assign {
        return None;
    }
    let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left else {
        return None;
    };
    let written = expression_parameter_index(&member.object, parameters, scoping)?;
    let index = expression_parameter_index(&member.expression, parameters, scoping)?;
    let value = expression_parameter_index(&assignment.right, parameters, scoping)?;
    let returned = expression_parameter_index(returned.argument.as_ref()?, parameters, scoping)?;
    if returned == written || returned == index || returned == value {
        return None;
    }
    Some((written, returned, index, value))
}

/// An inline store helper whose stored value is an expression over its own
/// formals (a tracer spelling of one raw-prefix byte):
/// `((a, b, c, d, e) => { a[b] = c >>> d; return e; })(buf, 0, s, 24, s)`.
struct InlineStore<'n, 'a> {
    /// Formal positions of the stored-into array, the index and the returned
    /// value.
    written: usize,
    index: usize,
    returned: usize,
    /// The stored expression (formals, literals and pure operators only).
    value: &'n Expression<'a>,
    formals: Vec<SymbolId>,
}

fn inline_store_helper<'n, 'a>(callee: &'n Expression<'a>, scoping: &Scoping) -> Option<InlineStore<'n, 'a>> {
    let Expression::ArrowFunctionExpression(arrow) = callee.without_parentheses() else {
        return None;
    };
    let formals = plain_parameter_symbols(&arrow.params)?;
    let [Statement::ExpressionStatement(statement), Statement::ReturnStatement(returned)] = arrow.body.statements.as_slice()
    else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = statement.expression.without_parentheses() else {
        return None;
    };
    if assignment.operator != AssignmentOperator::Assign {
        return None;
    }
    let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left else {
        return None;
    };
    let written = expression_parameter_index(&member.object, &formals, scoping)?;
    let index = expression_parameter_index(&member.expression, &formals, scoping)?;
    let returned = expression_parameter_index(returned.argument.as_ref()?, &formals, scoping)?;
    if returned == written || !pure_formal_expression(&assignment.right, &formals, scoping) {
        return None;
    }
    Some(InlineStore { written, index, returned, value: &assignment.right, formals })
}

/// The returned argument of an inline store helper call
/// (`((a, b, c, d) => { a[b] = c; return d; })(pair, 0, [], buf)` → `buf`).
pub(crate) fn inline_store_returned<'n, 'a>(call: &'n CallExpression<'a>, scoping: &Scoping) -> Option<&'n Expression<'a>> {
    let store = inline_store_helper(&call.callee, scoping)?;
    if call.arguments.len() != store.formals.len() {
        return None;
    }
    call.arguments.get(store.returned)?.as_expression()
}

/// Only formals, numeric literals and side-effect-free operators.
fn pure_formal_expression(expression: &Expression<'_>, formals: &[SymbolId], scoping: &Scoping) -> bool {
    match expression.without_parentheses() {
        Expression::Identifier(_) => expression_parameter_index(expression, formals, scoping).is_some(),
        Expression::NumericLiteral(_) => true,
        Expression::BinaryExpression(binary) => {
            pure_formal_expression(&binary.left, formals, scoping) && pure_formal_expression(&binary.right, formals, scoping)
        }
        Expression::UnaryExpression(unary) => {
            unary.operator != UnaryOperator::Delete && pure_formal_expression(&unary.argument, formals, scoping)
        }
        _ => false,
    }
}

fn expression_parameter_index(
    expression: &Expression<'_>,
    parameters: &[SymbolId],
    scoping: &Scoping,
) -> Option<usize> {
    let symbol = expression_symbol(expression, scoping)?;
    parameters.iter().position(|parameter| *parameter == symbol)
}

fn returned_statement_index(
    statements: &[Statement<'_>],
    parameters: &[SymbolId],
    scoping: &Scoping,
) -> Option<usize> {
    // Require the body to end in a `return`: a function that can fall through
    // does not always yield the parameter, so its call value is not provable.
    if !matches!(statements.last(), Some(Statement::ReturnStatement(_))) {
        return None;
    }
    let mut scan = ReturnScan {
        scoping,
        parameters,
        indices: Vec::new(),
        complete: true,
    };
    for statement in statements {
        scan.visit_statement(statement);
    }
    scan.finish()
}

/// Collects the formal-parameter index returned by every reachable `return`
/// statement of one function body. A nested function is a different scope and
/// its returns do not decide this function's value.
struct ReturnScan<'a, 's> {
    scoping: &'s Scoping,
    parameters: &'a [SymbolId],
    indices: Vec<usize>,
    complete: bool,
}

impl<'a, 's> ReturnScan<'a, 's> {
    fn finish(self) -> Option<usize> {
        if !self.complete || self.indices.is_empty() {
            return None;
        }
        let first = *self.indices.first()?;
        self.indices
            .iter()
            .all(|index| *index == first)
            .then_some(first)
    }
}

impl<'s, 'a> Visit<'a> for ReturnScan<'a, 's> {
    fn visit_return_statement(&mut self, statement: &ReturnStatement<'a>) {
        match statement
            .argument
            .as_ref()
            .map(|argument| argument.without_parentheses())
        {
            Some(Expression::Identifier(identifier)) => {
                match resolve_reference(identifier, self.scoping).and_then(|symbol| {
                    self.parameters
                        .iter()
                        .position(|parameter| *parameter == symbol)
                }) {
                    Some(index) => self.indices.push(index),
                    None => self.complete = false,
                }
            }
            _ => self.complete = false,
        }
    }

    fn visit_function(&mut self, _function: &Function<'a>, _flags: ScopeFlags) {}

    fn visit_arrow_function_expression(&mut self, _arrow: &ArrowFunctionExpression<'a>) {}
}

/// Scope-resolved proof that a helper body never changes the returned formal
/// parameter, never mutates an object rooted at it, never hands it to an opaque
/// call, and never introduces another way to write the binding. Nested scopes
/// are traversed too, so a captured closure write is rejected. Dynamic `eval`
/// and `with`, and any `arguments` use (a sloppy-mode mapped `arguments[3]`
/// assignment writes back to parameter 3), fail closed.
fn parameter_is_unchanged(
    statements: &[Statement<'_>],
    returned: SymbolId,
    scoping: &Scoping,
) -> bool {
    let mut scan = UnchangedParameterScan {
        scoping,
        returned,
        safe: true,
    };
    for statement in statements {
        scan.visit_statement(statement);
    }
    scan.safe
}

struct UnchangedParameterScan<'s> {
    scoping: &'s Scoping,
    returned: SymbolId,
    safe: bool,
}

impl<'s> UnchangedParameterScan<'s> {
    fn mentions_returned(&mut self, symbol: Option<SymbolId>) {
        if symbol == Some(self.returned) {
            self.safe = false;
        }
    }
}

impl<'s, 'a> Visit<'a> for UnchangedParameterScan<'s> {
    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        if !self.safe {
            return;
        }
        self.mentions_returned(assignment_target_root(&assignment.left, self.scoping));
        walk::walk_assignment_expression(self, assignment);
    }

    fn visit_update_expression(&mut self, update: &UpdateExpression<'a>) {
        if !self.safe {
            return;
        }
        self.mentions_returned(simple_assignment_target_root(
            &update.argument,
            self.scoping,
        ));
        walk::walk_update_expression(self, update);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if !self.safe {
            return;
        }
        if is_direct_eval(&call.callee) {
            self.safe = false;
            return;
        }
        self.mentions_returned(expression_root_symbol(&call.callee, self.scoping));
        for argument in &call.arguments {
            if let Some(expression) = argument.as_expression() {
                self.mentions_returned(expression_root_symbol(expression, self.scoping));
            }
        }
        if self.safe {
            walk::walk_call_expression(self, call);
        }
    }

    fn visit_new_expression(&mut self, new: &NewExpression<'a>) {
        if !self.safe {
            return;
        }
        for argument in &new.arguments {
            if let Some(expression) = argument.as_expression() {
                self.mentions_returned(expression_root_symbol(expression, self.scoping));
            }
        }
        if self.safe {
            walk::walk_new_expression(self, new);
        }
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        // A mapped `arguments` binding can write any parameter in sloppy mode,
        // including through an alias we do not track. Decline the whole helper.
        if identifier.name == "arguments" {
            self.safe = false;
        }
    }

    fn visit_with_statement(&mut self, _with: &WithStatement<'a>) {
        // `with` makes identifier resolution dynamic; no source proof survives.
        self.safe = false;
    }
}

fn assignment_target_root(target: &AssignmentTarget<'_>, scoping: &Scoping) -> Option<SymbolId> {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            resolve_reference(identifier, scoping)
        }
        AssignmentTarget::ComputedMemberExpression(member) => {
            expression_root_symbol(&member.object, scoping)
        }
        AssignmentTarget::StaticMemberExpression(member) => {
            expression_root_symbol(&member.object, scoping)
        }
        AssignmentTarget::PrivateFieldExpression(member) => {
            expression_root_symbol(&member.object, scoping)
        }
        _ => None,
    }
}

fn simple_assignment_target_root(
    target: &SimpleAssignmentTarget<'_>,
    scoping: &Scoping,
) -> Option<SymbolId> {
    match target {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            resolve_reference(identifier, scoping)
        }
        SimpleAssignmentTarget::ComputedMemberExpression(member) => {
            expression_root_symbol(&member.object, scoping)
        }
        SimpleAssignmentTarget::StaticMemberExpression(member) => {
            expression_root_symbol(&member.object, scoping)
        }
        SimpleAssignmentTarget::PrivateFieldExpression(member) => {
            expression_root_symbol(&member.object, scoping)
        }
        _ => None,
    }
}

/// The lexical binding at the root of an expression, following member access so
/// `buffer[0]`, `buffer.owner` and `buffer.push` all report `buffer`.
fn expression_root_symbol(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    match expression.without_parentheses() {
        Expression::Identifier(identifier) => resolve_reference(identifier, scoping),
        Expression::ComputedMemberExpression(member) => {
            expression_root_symbol(&member.object, scoping)
        }
        Expression::StaticMemberExpression(member) => {
            expression_root_symbol(&member.object, scoping)
        }
        Expression::PrivateFieldExpression(member) => {
            expression_root_symbol(&member.object, scoping)
        }
        _ => None,
    }
}

fn is_direct_eval(callee: &Expression<'_>) -> bool {
    matches!(callee.without_parentheses(), Expression::Identifier(identifier)
        if identifier.name == "eval")
}

// ===========================================================================
// Writer-helper proofs
// ===========================================================================

/// A writer passed through a program helper is still a typed write. The emitted
/// program folds two writes into one call:
///
/// ```text
/// function h(write, stream, a, b, c, position, buffer) {
///   return write(stream, c + (a ^ b), buffer, position);
/// }
/// buffer[k] = h(writeNumber, mask, x << 8, y & 255, x, writeNumber(mask, v, buffer, k), buffer);
/// ```
///
/// Recognition is structural and stays narrow: the helper must be a single
/// function whose plain formals receive the writer, its body is a bounded
/// straight-line sequence, and *every* call in that body goes through one of its
/// own formals. A loop, a nested function, a reassigned formal or a call to any
/// other callee makes the hand-off unprovable, so the buffer keeps the existing
/// [`IOp::Opaque`] node instead of a guessed grammar.
/// The most helper levels one write is expanded through. Deeper nesting is kept
/// `Opaque` rather than flattened without proof.
const MAX_HELPER_DEPTH: usize = 2;

/// The longest straight-line helper body admitted.
const MAX_HELPER_STATEMENTS: usize = 8;

/// One call inside a writer-helper body whose callee is one of the helper's own
/// plain formals.
#[derive(Clone, Debug)]
struct HelperWriteCall {
    /// Formal position of the writer the call reaches.
    writer: usize,
    /// The call's arguments, mapped back to formals where they are plain
    /// references.
    arguments: Vec<HelperArgument>,
    /// The whole call in the helper body.
    span: Span,
}

/// One argument of a [`HelperWriteCall`].
#[derive(Clone, Debug)]
enum HelperArgument {
    /// A plain reference to the helper formal at this position.
    Formal(usize),
    /// An expression written in the helper body, kept as its exact source.
    Expression { span: Span, text: String },
    /// An absent or non-expression argument.
    Other,
}

/// The proof that a binding holds a writer helper: a bounded straight-line body
/// whose only calls go through the function's own formals.
#[derive(Clone, Debug)]
struct WriterHelperProof {
    /// Every call to a formal, in JavaScript evaluation order.
    writes: Vec<HelperWriteCall>,
    /// Number of plain formals, so caller arguments can be mapped back.
    formals: usize,
}

/// Program-wide map from a binding to its writer-helper proof. Like the
/// returned-parameter proofs, a proof is attached only while the binding still
/// holds the declared function (single declaration origin, never written).
struct WriterHelperCollector;

impl WriterHelperCollector {
    fn build(
        scoping: &Scoping,
        source: &str,
        program: &Program<'_>,
    ) -> BTreeMap<SymbolId, WriterHelperProof> {
        let mut walker = WriterHelperWalk {
            scoping,
            source,
            proofs: BTreeMap::new(),
            origins: BTreeMap::new(),
            conflicted: BTreeSet::new(),
        };
        walker.visit_program(program);
        let mut proofs = walker.proofs;
        proofs.retain(|symbol, _| binding_holds_single_function(scoping, *symbol));
        proofs
    }
}

/// Registration walk for writer-helper proofs. A second declaration of the same
/// binding invalidates the proof instead of keeping the first declaration's.
struct WriterHelperWalk<'s> {
    scoping: &'s Scoping,
    source: &'s str,
    proofs: BTreeMap<SymbolId, WriterHelperProof>,
    origins: BTreeMap<SymbolId, Span>,
    conflicted: BTreeSet<SymbolId>,
}

impl WriterHelperWalk<'_> {
    fn record(&mut self, symbol: SymbolId, origin: Span, proof: Option<WriterHelperProof>) {
        if self.conflicted.contains(&symbol) {
            return;
        }
        if let Some(previous) = self.origins.insert(symbol, origin) {
            if previous != origin {
                self.conflicted.insert(symbol);
                self.proofs.remove(&symbol);
                return;
            }
        }
        if let Some(proof) = proof {
            self.proofs.entry(symbol).or_insert(proof);
        }
    }
}

impl<'a> Visit<'a> for WriterHelperWalk<'_> {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some(symbol) = function.id.as_ref().and_then(|id| id.symbol_id.get()) {
            let proof = function
                .body
                .as_ref()
                .and_then(|body| writer_helper_proof(self.scoping, self.source, &function.params, &body.statements));
            self.record(symbol, function.span, proof);
        }
        walk::walk_function(self, function, flags);
    }

    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if let BindingPattern::BindingIdentifier(identifier) = &declarator.id {
            if let Some(symbol) = identifier.symbol_id.get() {
                let proof = declarator.init.as_ref().and_then(|value| match value.without_parentheses() {
                    Expression::FunctionExpression(function) => function.body.as_ref().and_then(|body| {
                        writer_helper_proof(self.scoping, self.source, &function.params, &body.statements)
                    }),
                    Expression::ArrowFunctionExpression(arrow) => writer_helper_proof(
                        self.scoping,
                        self.source,
                        &arrow.params,
                        &arrow.body.statements,
                    ),
                    _ => None,
                });
                self.record(symbol, declarator.span, proof);
            }
        }
        walk::walk_variable_declarator(self, declarator);
    }
}

/// The writer-helper proof of one function body, or `None` when any part of the
/// shape is unproven.
fn writer_helper_proof(
    scoping: &Scoping,
    source: &str,
    params: &FormalParameters<'_>,
    statements: &[Statement<'_>],
) -> Option<WriterHelperProof> {
    let formals = plain_parameter_symbols(params)?;
    if statements.len() > MAX_HELPER_STATEMENTS {
        return None;
    }
    // A straight-line body: only expression statements, returns and variable
    // initializers are admitted. A loop, branch, block, `try` or nested function
    // declaration makes the write set unprovable.
    let mut roots: Vec<&Expression<'_>> = Vec::new();
    for statement in statements {
        match statement {
            Statement::ExpressionStatement(statement) => roots.push(&statement.expression),
            Statement::ReturnStatement(statement) => {
                if let Some(argument) = statement.argument.as_ref() {
                    roots.push(argument);
                }
            }
            Statement::VariableDeclaration(declaration) => {
                for declarator in &declaration.declarations {
                    if let Some(init) = declarator.init.as_ref() {
                        roots.push(init);
                    }
                }
            }
            Statement::EmptyStatement(_) => {}
            _ => return None,
        }
    }
    let mut scan = WriterHelperBodyScan {
        scoping,
        source,
        formals: &formals,
        writes: Vec::new(),
        unproven: false,
    };
    for root in roots {
        scan.visit_expression(root);
    }
    if scan.unproven || scan.writes.is_empty() {
        return None;
    }
    Some(WriterHelperProof {
        writes: scan.writes,
        formals: formals.len(),
    })
}

/// Collects the formal-callee calls of one helper body in evaluation order.
/// Nested calls in arguments are recorded before the call that contains them,
/// matching how the caller's arguments evaluate. Anything that could hide a
/// write (a nested function, a reassigned formal, `arguments`, `with`) fails the
/// proof instead of being skipped.
struct WriterHelperBodyScan<'s, 'f> {
    scoping: &'s Scoping,
    source: &'s str,
    formals: &'f [SymbolId],
    writes: Vec<HelperWriteCall>,
    unproven: bool,
}

impl<'s, 'f> WriterHelperBodyScan<'s, 'f> {
    fn formal_position(&self, expression: &Expression<'_>) -> Option<usize> {
        let symbol = expression_symbol(expression, self.scoping)?;
        self.formals.iter().position(|formal| *formal == symbol)
    }
}

impl<'a> Visit<'a> for WriterHelperBodyScan<'_, '_> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if self.unproven {
            return;
        }
        // The callee and every argument evaluate before the call itself.
        walk::walk_call_expression(self, call);
        match self.formal_position(&call.callee) {
            Some(writer) => {
                let arguments = call
                    .arguments
                    .iter()
                    .map(|argument| match argument.as_expression() {
                        Some(expression) => match self.formal_position(expression) {
                            Some(position) => HelperArgument::Formal(position),
                            None => HelperArgument::Expression {
                                span: expression.span(),
                                text: self
                                    .source
                                    .get(expression.span().start as usize..expression.span().end as usize)
                                    .unwrap_or_default()
                                    .trim()
                                    .to_string(),
                            },
                        },
                        None => HelperArgument::Other,
                    })
                    .collect();
                self.writes.push(HelperWriteCall {
                    writer,
                    arguments,
                    span: call.span,
                });
            }
            // A call through anything but a plain formal can write any argument
            // it is handed; the body is not a proven writer sequence.
            None => self.unproven = true,
        }
    }

    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        if assignment_target_root(&assignment.left, self.scoping)
            .is_some_and(|symbol| self.formals.contains(&symbol))
        {
            self.unproven = true;
            return;
        }
        walk::walk_assignment_expression(self, assignment);
    }

    fn visit_update_expression(&mut self, update: &UpdateExpression<'a>) {
        if let Some(symbol) = simple_assignment_target_root(&update.argument, self.scoping) {
            if self.formals.contains(&symbol) {
                self.unproven = true;
                return;
            }
        }
        walk::walk_update_expression(self, update);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        // A mapped `arguments` object can write any formal in sloppy mode.
        if identifier.name == "arguments" {
            self.unproven = true;
        }
    }

    fn visit_with_statement(&mut self, _with: &WithStatement<'a>) {
        self.unproven = true;
    }

    fn visit_function(&mut self, _function: &Function<'a>, _flags: ScopeFlags) {
        self.unproven = true;
    }

    fn visit_arrow_function_expression(&mut self, _arrow: &ArrowFunctionExpression<'a>) {
        self.unproven = true;
    }

    fn visit_class(&mut self, _class: &Class<'a>) {
        self.unproven = true;
    }
}

/// Finds nested functions handed to a task scheduler: `arguments[...]` of an
/// ordinary collector root, or one of the root's own unwritten plain formals
/// (the same slot in the named-formal representation). That hand-off is the
/// source proof that the nested function receives the observer as its own
/// first argument (`arguments[0]`, or its first plain formal); an uncalled
/// nested function has no such proof and must not inherit the enclosing route.
struct ForwardedFunctionCollector<'s, 'i> {
    scoping: &'s Scoping,
    function_spans: &'i BTreeMap<SymbolId, u32>,
    /// The root owns `arguments`, so `arguments[k]` is one of its slots.
    owns_arguments: bool,
    /// The root's unwritten plain formals.
    formal_schedulers: &'i BTreeSet<SymbolId>,
    forwarded: &'i mut BTreeSet<u32>,
    /// First plain formals of forwarded inline functions and arrows.
    forwarded_formals: &'i mut Vec<SymbolId>,
}

impl ForwardedFunctionCollector<'_, '_> {
    fn is_scheduler(&self, callee: &Expression<'_>) -> bool {
        if self.owns_arguments && scheduler_target(callee, self.scoping) {
            return true;
        }
        let callee = callee.without_parentheses();
        let target = match callee {
            Expression::StaticMemberExpression(member)
                if matches!(member.property.name.as_str(), "call" | "apply") =>
            {
                &member.object
            }
            _ => callee,
        };
        matches!(target.without_parentheses(), Expression::Identifier(identifier)
            if resolve_reference(identifier, self.scoping)
                .is_some_and(|symbol| self.formal_schedulers.contains(&symbol)))
    }

    fn forward_formal(&mut self, params: &FormalParameters<'_>) {
        if let ObserverParameter::Supported { symbol, .. } = observer_parameter(params) {
            if !observer_parameter_is_written(self.scoping, symbol) {
                self.forwarded_formals.push(symbol);
            }
        }
    }
}

impl<'s, 'a> Visit<'a> for ForwardedFunctionCollector<'s, '_> {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if self.is_scheduler(&call.callee) {
            for argument in &call.arguments {
                let Some(expression) = argument.as_expression() else {
                    continue;
                };
                match expression.without_parentheses() {
                    Expression::FunctionExpression(function) => {
                        self.forwarded.insert(function.span.start);
                        self.forward_formal(&function.params);
                    }
                    Expression::ArrowFunctionExpression(arrow) => {
                        self.forwarded.insert(arrow.span.start);
                        self.forward_formal(&arrow.params);
                    }
                    Expression::Identifier(identifier) => {
                        if let Some(symbol) = resolve_reference(identifier, self.scoping) {
                            if let Some(start) = self.function_spans.get(&symbol) {
                                self.forwarded.insert(*start);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        walk::walk_call_expression(self, call);
    }
}

/// `arguments[k]` or `arguments[k].call/apply(...)`, i.e. a task scheduler
/// reached through the collector's own arguments list.
fn scheduler_target(callee: &Expression<'_>, scoping: &Scoping) -> bool {
    if arguments_member(callee, scoping) {
        return true;
    }
    let Some(object) = member_object(callee) else {
        return false;
    };
    arguments_member(object, scoping)
}

fn arguments_member(expression: &Expression<'_>, scoping: &Scoping) -> bool {
    matches!(expression.without_parentheses(), Expression::ComputedMemberExpression(member)
        if is_arguments(&member.object, scoping))
}

struct HelperSymbols {
    number_writer: Vec<SymbolId>,
    string_writer: Vec<SymbolId>,
    prng_factory: Vec<SymbolId>,
    hash: Option<SymbolId>,
}

// ===========================================================================
// Program-lifetime visitor
// ===========================================================================

struct LayoutVisitor<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    aliases: &'s BTreeMap<SymbolId, SymbolId>,
    helpers: HelperSymbols,
    returned_parameters: &'s BTreeMap<SymbolId, ReturnedParameterProof>,
    /// Writer helpers a producer hands a writer to, proven from their body
    /// shape and single-origin binding (see [`WriterHelperProof`]).
    writer_helpers: &'s BTreeMap<SymbolId, WriterHelperProof>,
    /// See [`SealedTuples`].
    sealed: &'s BTreeSet<SymbolId>,
    collectors: &'s [CollectorRequest],
    request_by_start: BTreeMap<u32, Vec<usize>>,
    limits: SignalLayoutLimits,
    signals: BTreeMap<u32, Vec<Emission>>,
    unresolved: Vec<Unresolved>,
    accounting: Vec<BufferAccounting>,
    collector_count: usize,
    emission_count: usize,
    truncated: bool,
    _marker: std::marker::PhantomData<&'a ()>,
}

/// The requested collector root: an ordinary function or an arrow callback.
/// Both go through the same ordered scanner; only `arguments` ownership differs.
///
/// `'n` is the borrow of the visited AST node; the node's own contents live for
/// the allocator lifetime `'a`.
#[derive(Clone, Copy)]
enum CollectorRoot<'n, 'a> {
    Function(&'n Function<'a>),
    Arrow(&'n ArrowFunctionExpression<'a>),
}

impl<'n, 'a> CollectorRoot<'n, 'a> {
    fn span_start(self) -> u32 {
        match self {
            Self::Function(function) => function.span.start,
            Self::Arrow(arrow) => arrow.span.start,
        }
    }

    fn body(self) -> Option<&'n FunctionBody<'a>> {
        match self {
            Self::Function(function) => function.body.as_deref(),
            Self::Arrow(arrow) => Some(arrow.body.as_ref()),
        }
    }

    fn params(self) -> &'n FormalParameters<'a> {
        match self {
            Self::Function(function) => function.params.as_ref(),
            Self::Arrow(arrow) => arrow.params.as_ref(),
        }
    }

    /// An ordinary function owns its `arguments`; an arrow keeps the enclosing
    /// function's `arguments`, so it must not expose an observer route there.
    fn owns_arguments(self) -> bool {
        matches!(self, Self::Function(_))
    }
}

impl<'s, 'a> Visit<'a> for LayoutVisitor<'s, 'a> {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        self.analyze_requested(CollectorRoot::Function(function));
        walk::walk_function(self, function, flags);
    }

    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.analyze_requested(CollectorRoot::Arrow(arrow));
        walk::walk_arrow_function_expression(self, arrow);
    }
}

impl<'s, 'a> LayoutVisitor<'s, 'a> {
    fn analyze_requested(&mut self, root: CollectorRoot<'_, 'a>) {
        let Some(positions) = self.request_by_start.get(&root.span_start()).cloned() else {
            return;
        };
        // Clone the requests so the `&mut self` analysis does not hold a borrow
        // of `self.collectors`.
        let requests: Vec<CollectorRequest> = positions
            .into_iter()
            .map(|position| self.collectors[position].clone())
            .collect();
        for request in &requests {
            self.analyze_collector(root, request);
        }
    }

    fn analyze_collector(&mut self, root: CollectorRoot<'_, 'a>, collector: &CollectorRequest) {
        self.collector_count += 1;
        if self.collector_count > self.limits.max_collectors {
            self.truncated = true;
            return;
        }
        let Some(body) = root.body() else {
            return;
        };
        let function_spans = match root {
            CollectorRoot::Function(function) => FunctionSpanCollector::build(function),
            CollectorRoot::Arrow(arrow) => FunctionSpanCollector::build_arrow(arrow),
        };
        let mut forwarded = BTreeSet::new();
        let mut forwarded_formals = Vec::new();
        // Forwarding is proven from the collector's own hand-off through one
        // of its argument slots: `arguments[k]` for an ordinary root (an arrow
        // does not own `arguments`), or an unwritten plain formal of either
        // root kind.
        let formal_schedulers: BTreeSet<SymbolId> = root
            .params()
            .items
            .iter()
            .filter_map(|param| match &param.pattern {
                BindingPattern::BindingIdentifier(identifier) if param.initializer.is_none() => {
                    identifier.symbol_id.get()
                }
                _ => None,
            })
            .filter(|symbol| !observer_parameter_is_written(self.scoping, *symbol))
            .collect();
        let mut forwarding = ForwardedFunctionCollector {
            scoping: self.scoping,
            function_spans: &function_spans.by_symbol,
            owns_arguments: root.owns_arguments(),
            formal_schedulers: &formal_schedulers,
            forwarded: &mut forwarded,
            forwarded_formals: &mut forwarded_formals,
        };
        match root {
            CollectorRoot::Function(function) => {
                forwarding.visit_function(function, ScopeFlags::Function)
            }
            CollectorRoot::Arrow(arrow) => forwarding.visit_arrow_function_expression(arrow),
        }
        // Decide the observer root before constructing the scanner, so the
        // decision only borrows `self` immutably and returns owned evidence.
        let (observer_symbol, observer_unresolved) =
            self.observer_seed(root.params(), root.body(), root.owns_arguments());
        let mut scanner = CollectorScanner::new(
            self.source,
            self.scoping,
            self.nodes,
            self.aliases,
            &self.helpers,
            self.returned_parameters,
            self.writer_helpers,
            &collector.observers,
            forwarded,
            root.owns_arguments(),
            collector.site,
            collector.forwarded_parameter.as_deref(),
            self.limits,
        );
        scanner.sealed = Some(self.sealed);
        if let Some(symbol) = observer_symbol {
            scanner.seed_observer_root(symbol);
        }
        scanner.seed_stable_observer_aliases(body.statements.as_slice());
        // A direct task forwarded through a scheduler slot receives the
        // observer as its first formal, as a forwarded ordinary function
        // receives it as `arguments[0]`.
        if matches!(collector.observers, ObserverWiring::Direct { .. }) {
            for symbol in forwarded_formals {
                scanner.seed_observer_root(symbol);
            }
        }
        if let Some(unresolved) = observer_unresolved {
            self.unresolved.push(unresolved);
        }
        scanner.scan_statements(body.statements.as_slice());
        let found = scanner.finish();
        if found.truncated {
            self.truncated = true;
        }
        for unresolved in found.unresolved {
            self.unresolved.push(unresolved);
        }
        self.accounting.extend(found.accounting);
        for emission in found.emissions {
            if self.emission_count >= self.limits.max_emissions {
                self.truncated = true;
                break;
            }
            self.emission_count += 1;
            self.signals
                .entry(emission.signal)
                .or_default()
                .push(emission);
        }
    }

    /// Resolves the collector root's first plain formal as the semantic observer
    /// root, or returns why it cannot. `arguments[]` stays independently valid
    /// for a written parameter because it names the call argument, not the
    /// binding's later value.
    fn observer_seed(
        &self,
        params: &FormalParameters<'a>,
        body: Option<&FunctionBody<'a>>,
        owns_arguments: bool,
    ) -> (Option<SymbolId>, Option<Unresolved>) {
        match observer_parameter(params) {
            ObserverParameter::Absent => (None, None),
            ObserverParameter::Supported { symbol, span } => {
                if observer_parameter_is_written(self.scoping, symbol)
                    || (owns_arguments
                        && body.is_some_and(|body| {
                            mapped_arguments_observer_is_written(body, self.scoping)
                        }))
                {
                    (
                        None,
                        Some(Unresolved {
                            span,
                            reason: "observer_parameter_mutated".into(),
                            detail: "the first formal parameter is written, so it no longer proves the observer root".into(),
                        }),
                    )
                } else {
                    (Some(symbol), None)
                }
            }
            ObserverParameter::Unsupported { span, detail } => (
                None,
                Some(Unresolved {
                    span,
                    reason: "observer_parameter_unsupported".into(),
                    detail: detail.into(),
                }),
            ),
        }
    }
}

/// How the collector root's formal list names the observer root.
enum ObserverParameter {
    /// No formal parameter is declared; only the `arguments[]` ABI applies.
    Absent,
    /// A plain first formal (`BindingIdentifier`, no initializer/rest/pattern).
    Supported { symbol: SymbolId, span: Span },
    /// A first formal whose value cannot be proven to be the observer argument.
    Unsupported { span: Span, detail: &'static str },
}

/// Resolves the observer-position ABI's first plain formal. A default
/// initializer, rest element, destructuring pattern, decorator or optional
/// marker is not guessed at; the caller records the reason and leaves the
/// `arguments[]` route to prove the actual call argument.
fn observer_parameter(params: &FormalParameters<'_>) -> ObserverParameter {
    let Some(first) = params.items.first() else {
        return match params.rest.as_ref() {
            Some(rest) => ObserverParameter::Unsupported {
                span: rest.span,
                detail: "a rest-only formal list has no plain first parameter",
            },
            None => ObserverParameter::Absent,
        };
    };
    if !first.decorators.is_empty() {
        return ObserverParameter::Unsupported {
            span: first.span,
            detail: "the first formal parameter is decorated",
        };
    }
    if first.initializer.is_some() {
        return ObserverParameter::Unsupported {
            span: first.span,
            detail: "the first formal parameter has a default initializer",
        };
    }
    if first.optional {
        return ObserverParameter::Unsupported {
            span: first.span,
            detail: "the first formal parameter is optional",
        };
    }
    match &first.pattern {
        BindingPattern::BindingIdentifier(identifier) => match identifier.symbol_id.get() {
            Some(symbol) => ObserverParameter::Supported {
                symbol,
                span: identifier.span,
            },
            None => ObserverParameter::Unsupported {
                span: first.span,
                detail: "the first formal parameter has no resolved binding",
            },
        },
        _ => ObserverParameter::Unsupported {
            span: first.span,
            detail: "the first formal parameter is a destructuring pattern",
        },
    }
}

/// A written formal no longer proves the value passed by the caller. This is
/// checked across the whole collector subtree, so a captured nested write also
/// fails closed.
fn observer_parameter_is_written(scoping: &Scoping, symbol: SymbolId) -> bool {
    scoping
        .get_resolved_references(symbol)
        .any(|reference| reference.is_write())
}

struct MappedArgumentsObserverWrite<'s> {
    scoping: &'s Scoping,
    written: bool,
    ordinary_function_depth: usize,
}

impl<'a> Visit<'a> for MappedArgumentsObserverWrite<'_> {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        self.ordinary_function_depth += 1;
        walk::walk_function(self, function, flags);
        self.ordinary_function_depth -= 1;
    }

    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        let mapped_arguments_write = match &assignment.left {
            AssignmentTarget::ComputedMemberExpression(member) => {
                matches!(&member.object, Expression::Identifier(identifier)
                    if identifier.name == "arguments"
                        && resolve_reference(identifier, self.scoping).is_none())
                    && numeric_index(&member.expression) == Some(0)
            }
            _ => false,
        };
        if self.ordinary_function_depth == 0 && mapped_arguments_write {
            self.written = true;
        }
        walk::walk_assignment_expression(self, assignment);
    }

    fn visit_update_expression(&mut self, update: &UpdateExpression<'a>) {
        if self.ordinary_function_depth == 0
            && matches!(&update.argument, SimpleAssignmentTarget::ComputedMemberExpression(member)
                if matches!(&member.object, Expression::Identifier(identifier)
                    if identifier.name == "arguments"
                        && resolve_reference(identifier, self.scoping).is_none())
                    && numeric_index(&member.expression) == Some(0))
        {
            self.written = true;
        }
        walk::walk_update_expression(self, update);
    }
}

fn mapped_arguments_observer_is_written(body: &FunctionBody<'_>, scoping: &Scoping) -> bool {
    let mut visitor = MappedArgumentsObserverWrite {
        scoping,
        written: false,
        ordinary_function_depth: 0,
    };
    for statement in &body.statements {
        visitor.visit_statement(statement);
    }
    visitor.written
}

fn numeric_index(expression: &Expression<'_>) -> Option<u32> {
    match expression {
        Expression::NumericLiteral(literal)
            if literal.value.fract() == 0.0
                && literal.value >= 0.0
                && literal.value <= u32::MAX as f64 =>
        {
            Some(literal.value as u32)
        }
        _ => None,
    }
}

// ===========================================================================
// Collector scanner
// ===========================================================================

/// The receiver of an `.next(...)` call, before wiring resolves it to a signal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ObsTarget {
    /// `arguments[0]` — the observer itself (direct collector).
    ArgumentsList,
    /// `arguments[0][k]` — one observer in the list (wave collector).
    Slot(u32),
}

/// A write collected during the ordered replay.
#[derive(Clone)]
struct ILeaf {
    writer_factory: Option<Span>,
    string_instance: Option<Span>,
    kind: LeafKind,
    span: Span,
    buffer: Option<SymbolId>,
    /// The mask evidence captured when this write was replayed. Keeping the
    /// whole `MaskInfo` (not just a `SymbolId`) preserves an inline factory
    /// argument and the variable's value at write time even if the original
    /// binding is reassigned later.
    mask: Option<MaskInfo>,
    masked: bool,
    value: Option<String>,
    index: Option<String>,
    index_span: Option<Span>,
    xor_literal: Option<f64>,
    xor_source: Option<String>,
    xor_span: Span,
    byte_width: Option<usize>,
    /// The logical value without its mask (`v` in `v ^ mask()`), and where it
    /// sits in the source.
    logical_value: String,
    logical_span: Span,
    /// The logical value after resolving a plain local read at this write to
    /// the expression last assigned to it.
    resolved_value: Option<String>,
    mask_form: MaskForm,
    /// The stream argument or the mask callee text.
    mask_source: Option<String>,
    /// `Some(k)` for a store of raw prefix byte `k` (0..=3) of a seed word into
    /// a buffer that already had a prefix; grouped into one `RawPrefix` node.
    prefix_element: Option<u32>,
    /// For a write made through a program helper: the caller's helper-call
    /// span. The leaf's own `span` is the writer call inside the helper body.
    helper_call: Option<Span>,
    /// For a write made through a program helper: the caller argument span for
    /// each helper formal, so the provenance of a formal reference is what the
    /// caller passed (see [`WriteNode::helper_arguments`]).
    helper_arguments: Vec<Span>,
}

/// The construct one `IOp::Choice` was built from.
#[derive(Clone)]
struct ChoiceSite {
    buffer_aliases: Vec<BTreeMap<SymbolId, SymbolId>>,
    kind: ChoiceKind,
    span: Span,
    condition: Option<String>,
    /// One label per arm.
    labels: Vec<String>,
}

#[derive(Clone, Copy)]
enum ControlExit {
    Return,
    Throw,
    Break,
    Continue,
}

#[derive(Clone)]
enum IOp {
    Exit(ControlExit),
    Seq(Vec<IOp>),
    Repeat {
        control: LoopControl,
        body: Box<IOp>,
    },
    Choice(ChoiceSite, Vec<IOp>),
    Leaf(ILeaf),
    Unknown {
        span: Span,
        text: String,
        reason: String,
        buffer: Option<SymbolId>,
    },
    /// The buffer was (re)initialized with a raw prefix array literal here.
    Prefix {
        buffer: SymbolId,
        node: PrefixNode,
    },
    /// The buffer is passed to a call that is neither a recognized writer nor
    /// a verified store helper; the callee may write it.
    Opaque {
        span: Span,
        callee: String,
        buffer: SymbolId,
        /// The binding's write version at this call site. A collector reuses one
        /// local for several values (a host object, an `Error`, the payload
        /// array), so passing the binding here only concerns the emitted buffer
        /// when the version matches the emission's.
        version: u32,
    },
    /// A bounded control/data-flow boundary whose exact relation to specific
    /// emitted buffers is not proven.  This is deliberately distinct from a
    /// dropped operation: filtering it for an affected buffer produces an
    /// explicit unresolved grammar node.  `buffers` is the source-proven set of
    /// bindings the construct can write; it keeps an unrelated construct (for
    /// example a `try` around a read-only call) from degrading every grammar.
    Unsupported {
        span: Span,
        text: String,
        reason: String,
        buffers: BTreeSet<SymbolId>,
    },
}

struct RawEmission {
    origin: Span,
    target: ObsTarget,
    payload: PayloadResolution,
    /// The function-body scope the emission executes in.
    scope: u32,
    /// The exact ordered writes on the execution path that reached this
    /// emission, snapshotted at the emission call. The path already includes the
    /// writes of every enclosing sequential statement and of the conditional arm
    /// currently being scanned, so it never depends on the length of a temporary
    /// branch log.
    path: Vec<IOp>,
    /// Write version of the emitted buffer at this emission, when the payload
    /// resolves to a buffer binding.
    buffer_version: Option<u32>,
    /// Prefix state at the call, not whichever value a reused binding holds
    /// after the rest of the collector has been scanned.
    prefix: Option<PrefixInfo>,
}

/// One inferred field of a raw named object.
#[derive(Clone, Debug)]
struct ObjectField {
    key: String,
    value: Option<String>,
    kind: Option<LeafKind>,
    span: Span,
}

enum PayloadResolution {
    Buffer(SymbolId),
    Object(Vec<ObjectField>),
    /// A bare (non-object, non-buffer) value handed to the generic codec,
    /// whose every conditional arm has a source-proven type.
    Scalar(ScalarTree),
    Unresolved(String),
}

/// The typed arms of a bare generic value: one typed leaf, or a `test ? a : b`
/// whose arms are typed differently.
#[derive(Clone, Debug, PartialEq)]
enum ScalarTree {
    Leaf { kind: LeafKind, value: String, span: Span },
    Choice {
        kind: ChoiceKind,
        span: Span,
        condition: String,
        then: Box<ScalarTree>,
        other: Box<ScalarTree>,
    },
}

impl ScalarTree {
    /// `test ? then : other` (or the if/else join of a binding): one leaf when
    /// both arms have the same type, else a two-arm choice.
    fn join(kind: ChoiceKind, span: Span, condition: String, value: String, then: ScalarTree, other: ScalarTree) -> ScalarTree {
        if let (ScalarTree::Leaf { kind: a, .. }, ScalarTree::Leaf { kind: b, .. }) = (&then, &other) {
            if a == b {
                return ScalarTree::Leaf { kind: *a, value, span };
            }
        }
        ScalarTree::Choice { kind, span, condition, then: Box::new(then), other: Box::new(other) }
    }
}

struct PrefixParts {
    first_store: u32,
    seed: Option<SymbolId>,
    elements: BTreeMap<u32, (String, Span)>,
}

#[derive(Clone)]
struct PrefixInfo {
    seed_binding: Option<SourceSpan>,
    /// Full allocation/store range, including the array delimiters. Element
    /// spans alone are not an allocation identity when a prefix is inherited.
    span: Span,
    seed_word: String,
    binding: Option<String>,
    /// Each prefix-array element: its exact source text and its span. One BYTE
    /// leaf with `byte_width == 1` is produced per element.
    elements: Vec<(String, Span)>,
}

#[derive(Clone, Debug)]
struct MaskInfo {
    predrawn: Option<Span>,
    seed_binding: Option<SourceSpan>,
    instance: Span,
    factory_binding: Span,
    factory: Option<String>,
    seed_word: Option<String>,
    xor_literal: Option<f64>,
    xor_source: Option<String>,
    xor_span: Span,
    argument: String,
}

struct CollectorResult {
    emissions: Vec<Emission>,
    unresolved: Vec<Unresolved>,
    truncated: bool,
    accounting: Vec<BufferAccounting>,
}

struct CollectorScanner<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    aliases: &'s BTreeMap<SymbolId, SymbolId>,
    helpers: &'s HelperSymbols,
    /// Functions whose call value is one of their arguments unchanged; used to
    /// resolve a store-helper-built payload array to its emitted buffer.
    returned_parameters: &'s BTreeMap<SymbolId, ReturnedParameterProof>,
    /// Program functions a producer hands a writer to, with the writer calls
    /// their bounded straight-line bodies make (see [`WriterHelperProof`]).
    writer_helpers: &'s BTreeMap<SymbolId, WriterHelperProof>,
    observers: &'s ObserverWiring,
    limits: SignalLayoutLimits,
    aliases_out: BTreeMap<SymbolId, ObsTarget>,
    /// Binding -> seed-word source text (`Math.random()*2^32[|0]`).
    seeds: BTreeMap<SymbolId, String>,
    /// Arrays of bare seed-word reads (`p = [w, w]`, a tracer spelling):
    /// `p[k]` reads the seed word.
    seed_copies: BTreeMap<SymbolId, SymbolId>,
    /// Plain copies of a seed word (`r = seed`), until `r` is written again:
    /// `r >>> 24` reads the seed.
    seed_aliases: BTreeMap<SymbolId, SymbolId>,
    prefixes: BTreeMap<SymbolId, PrefixInfo>,
    /// Prefix arrays built by `buf[k] = seed >> ...` stores instead of an
    /// array literal. They are assembled as the four ordered byte writes are
    /// seen; only a complete 0..=3 set becomes a `PrefixInfo`.
    prefix_parts: BTreeMap<SymbolId, PrefixParts>,
    masks: BTreeMap<SymbolId, MaskInfo>,
    payload_buffers: BTreeMap<SymbolId, SymbolId>,
    payload_index: BTreeMap<(SymbolId, i64), SymbolId>,
    payload_objects: BTreeMap<SymbolId, Vec<ObjectField>>,
    /// Bindings whose latest value is a bare, fully typed scalar (see
    /// [`ScalarTree`]); only consulted for an identifier payload that is
    /// neither a buffer nor a named object.
    payload_scalars: BTreeMap<SymbolId, ScalarTree>,
    /// `let alias = obj` of a raw named object: dotted writes through the
    /// alias (`alias.b = v`) update `obj`'s fields.
    object_aliases: BTreeMap<SymbolId, SymbolId>,
    /// The write tree of the function body currently being scanned; the
    /// `Choice`/`Repeat` structure of an emission's wire is built from it.
    ops: Vec<IOp>,
    /// Per-binding write counter: bumped on every binding write, so an operation
    /// recorded before a rebinding cannot be attributed to the value the
    /// binding holds after it (collectors reuse one local for a host object, an
    /// `Error`, a scratch count and then the payload array).
    versions: BTreeMap<SymbolId, u32>,
    /// Latest value assigned to each local during the ordered replay. Used to
    /// resolve a write's plain local read to the expression it holds at that
    /// point (`$KEY_40` -> `navigator.userAgent`).
    values: BTreeMap<SymbolId, ValueBinding>,
    /// Locals whose latest value is a `String.prototype.split(<string>)`
    /// result: an indexed read of one is a string element.
    split_results: BTreeSet<SymbolId>,
    /// The ordered writes on the current execution path, extended as statements
    /// are scanned. Inside a conditional arm it continues from the enclosing
    /// path, so an emission snapshots the writes that actually ran before it.
    path: Vec<IOp>,
    /// Completed logs of nested function bodies, keyed by scope id.
    scope_logs: BTreeMap<u32, Vec<IOp>>,
    scope_id: u32,
    scope_counter: u32,
    /// Nesting depth of ordinary (non-arrow) functions currently being scanned.
    /// `arguments` is the collector's own only while this is zero; an arrow
    /// retains the enclosing function's `arguments` and does not increase it.
    fn_depth: usize,
    /// True when the collector root is an ordinary function and therefore owns
    /// the `arguments` object named in its own body. An arrow root keeps an
    /// enclosing function's `arguments`, so it is not the collector's observer.
    root_owns_arguments: bool,
    /// Span start of the ordinary function whose body is currently scanned
    /// (`None` at the collector root).
    current_function: Option<u32>,
    /// Nested ordinary functions that are handed to a task scheduler on
    /// `arguments[...]` and therefore do receive the observer as their own
    /// `arguments[0]`. A direct collector may resolve that hand-off; a wave
    /// collector never forwards observers to nested ordinary functions.
    forwarded_functions: BTreeSet<u32>,
    /// Bindings whose value is the STRING-writer closure returned by the STRING
    /// writer factory, mapped to the mask binding passed at the factory call.
    string_writers: BTreeMap<SymbolId, Option<MaskInfo>>,
    string_instances: BTreeMap<SymbolId, Span>,
    string_factories: BTreeMap<SymbolId, Span>,
    /// Bindings holding an observer's own `next` method (`m = observer.next`),
    /// later invoked as `m.call(observer, payload)`.
    next_methods: BTreeMap<SymbolId, ObsTarget>,
    /// Locals that hold the NUMBER writer at the current point of the replay
    /// (`let w = writeNumber; … w = w(…)` reuses one holder for the writer and
    /// then for the returned position). Updated in evaluation order.
    number_holders: BTreeSet<SymbolId>,
    /// The element expressions of each tuple in [`Self::tuples`]: a writer
    /// value read back as `t[k]` is that element (`t = [v, buf]; w(t[0], t[1])`).
    tuple_values: BTreeMap<SymbolId, Vec<Option<Span>>>,
    /// Locals holding a tuple literal `[buffer, index, …]` whose elements are
    /// read back as `t[0][t[1]] = …`.
    tuples: BTreeMap<SymbolId, Vec<Option<(Option<SymbolId>, String)>>>,
    /// Per tuple element: the stream an element drew from (`t = [buf, i, ks()]`).
    tuple_draws: BTreeMap<SymbolId, Vec<Option<MaskInfo>>>,
    /// Locals holding one stream draw (`let m = ks(); …; buf[i] = m ^ 144`)
    /// while no other write has drawn since.
    register_draws: BTreeMap<SymbolId, MaskInfo>,
    /// Stable, source-proven aliases of buffer bindings.  This is intentionally
    /// narrower than general value flow: a plain `holder = buffer` is safe to
    /// coalesce, while a reassignment between two known buffers is an
    /// unresolved alias-flow boundary.
    buffer_aliases: BTreeMap<SymbolId, SymbolId>,
    known_buffers: BTreeSet<SymbolId>,
    /// See [`SealedTuples`].
    sealed: Option<&'s BTreeSet<SymbolId>>,
    /// Empty array literals held in a sealed register tuple
    /// (`t = [[], seed]`), by literal start: each is a buffer without a
    /// binding, named by an id past the program's symbols.
    virtual_buffers: BTreeMap<u32, SymbolId>,
    /// Label (`t[k]`) and literal span of each virtual buffer.
    virtual_names: BTreeMap<SymbolId, (String, Span)>,
    /// Per tuple element: the seed word the element held when the tuple was
    /// made (`t = [[], seed]; t[0][0] = t[1] >>> 24`).
    tuple_seeds: BTreeMap<SymbolId, Vec<Option<SymbolId>>>,
    /// A register tuple holding a seed draw as an element
    /// (`t = [[], Math.random() * 4294967296 | 0]`): the draw's text. Its
    /// [`Self::tuple_seeds`] element is the tuple itself.
    tuple_seed_words: BTreeMap<SymbolId, String>,
    /// How this producer emits; only `WorkerReply` looks for `postMessage`.
    site: EmissionSite,
    /// The VM worker reply parameter this launcher half forwards, when any.
    forwarded_parameter: Option<&'s str>,
    /// Bindings that hold the worker's `postMessage` global (`p = postMessage`).
    worker_post: BTreeSet<SymbolId>,
    /// Bindings that hold a worker reply carrying `[[], buffer]`, mapped to that
    /// buffer (`reply = { a, b: [[], buf] }`).
    worker_replies: BTreeMap<SymbolId, SymbolId>,
    /// Nesting depth of writer-helper expansions currently on the stack; deeper
    /// than [`MAX_HELPER_DEPTH`] stays `Opaque`.
    helper_depth: usize,
    /// The label of the statement being scanned when it is a labeled
    /// statement's body (`L: for (;;) …`): its span and label name.
    statement_label: Option<(Span, String)>,
    emissions: Vec<RawEmission>,
    unresolved: Vec<Unresolved>,
    node_budget: usize,
    depth_truncated: bool,
    emission_truncated: bool,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'s, 'a> CollectorScanner<'s, 'a> {
    fn new(
        source: &'s str,
        scoping: &'s Scoping,
        nodes: &'s AstNodes<'a>,
        aliases: &'s BTreeMap<SymbolId, SymbolId>,
        helpers: &'s HelperSymbols,
        returned_parameters: &'s BTreeMap<SymbolId, ReturnedParameterProof>,
        writers: &'s BTreeMap<SymbolId, WriterHelperProof>,
        observers: &'s ObserverWiring,
        forwarded_functions: BTreeSet<u32>,
        root_owns_arguments: bool,
        site: EmissionSite,
        forwarded_parameter: Option<&'s str>,
        limits: SignalLayoutLimits,
    ) -> Self {
        Self {
            source,
            scoping,
            nodes,
            aliases,
            helpers,
            returned_parameters,
            writer_helpers: writers,
            observers,
            limits,
            aliases_out: BTreeMap::new(),
            seeds: BTreeMap::new(),
            seed_copies: BTreeMap::new(),
            seed_aliases: BTreeMap::new(),
            prefixes: BTreeMap::new(),
            prefix_parts: BTreeMap::new(),
            masks: BTreeMap::new(),
            payload_buffers: BTreeMap::new(),
            payload_index: BTreeMap::new(),
            payload_objects: BTreeMap::new(),
            payload_scalars: BTreeMap::new(),
            object_aliases: BTreeMap::new(),
            split_results: BTreeSet::new(),
            ops: Vec::new(),
            path: Vec::new(),
            scope_logs: BTreeMap::new(),
            scope_id: 0,
            scope_counter: 0,
            fn_depth: 0,
            root_owns_arguments,
            current_function: None,
            forwarded_functions,
            string_writers: BTreeMap::new(),
            string_instances: BTreeMap::new(),
            string_factories: BTreeMap::new(),
            next_methods: BTreeMap::new(),
            number_holders: BTreeSet::new(),
            tuples: BTreeMap::new(),
            tuple_draws: BTreeMap::new(),
            register_draws: BTreeMap::new(),
            tuple_values: BTreeMap::new(),
            buffer_aliases: BTreeMap::new(),
            known_buffers: BTreeSet::new(),
            sealed: None,
            virtual_buffers: BTreeMap::new(),
            virtual_names: BTreeMap::new(),
            tuple_seeds: BTreeMap::new(),
            tuple_seed_words: BTreeMap::new(),
            site,
            forwarded_parameter,
            worker_post: BTreeSet::new(),
            worker_replies: BTreeMap::new(),
            versions: BTreeMap::new(),
            values: BTreeMap::new(),
            helper_depth: 0,
            statement_label: None,
            emissions: Vec::new(),
            unresolved: Vec::new(),
            node_budget: limits.max_wire_nodes,
            depth_truncated: false,
            emission_truncated: false,
            _marker: std::marker::PhantomData,
        }
    }

    /// Seeds the collector root's first plain formal as the observer root. It
    /// uses the same [`ObsTarget::ArgumentsList`] target as `arguments[0]`, so a
    /// direct observer and a wave list both reuse the existing slot/direct
    /// wiring resolution. Aliases and capturing arrows then inherit it through
    /// their resolved [`SymbolId`].
    fn seed_observer_root(&mut self, symbol: SymbolId) {
        self.aliases_out.insert(symbol, ObsTarget::ArgumentsList);
    }

    fn finish(&mut self) -> CollectorResult {
        // The top-level collector body is scope 0 and is still the live log.
        self.scope_logs.insert(0, std::mem::take(&mut self.ops));
        let mut emissions = Vec::new();
        let mut unresolved = std::mem::take(&mut self.unresolved);
        let mut emitted: BTreeMap<(u32, SymbolId), (BTreeSet<u32>, Vec<GrammarNode>)> =
            BTreeMap::new();
        for raw in &self.emissions {
            let (signal, route) = match (raw.target, self.observers) {
                (ObsTarget::Slot(slot), ObserverWiring::WaveSlots(slots)) => {
                    match slots.iter().find(|entry| entry.slot == slot) {
                        Some(entry) => (entry.signal, ObserverRoute::WaveSlot(slot)),
                        None => {
                            unresolved.push(Unresolved {
                                span: raw.origin,
                                reason: "observer_slot_unmapped".into(),
                                detail: format!("wave slot {slot} has no signal position"),
                            });
                            continue;
                        }
                    }
                }
                (ObsTarget::ArgumentsList, ObserverWiring::Direct { signal }) => {
                    (*signal, ObserverRoute::Direct)
                }
                _ => continue,
            };
            let emission = self.build_emission(signal, route, raw, &mut unresolved);
            if let PayloadResolution::Buffer(buffer) = &raw.payload {
                let mut roots = BTreeSet::from([*buffer]);
                collect_branch_buffer_roots(&raw.path, *buffer, &mut roots);
                for root in roots {
                    let entry = emitted
                        .entry((signal, root))
                        .or_insert_with(|| (BTreeSet::new(), Vec::new()));
                    entry.0.insert(raw.scope);
                    entry.1.push(emission.grammar.clone());
                }
            }
            emissions.push(emission);
        }
        let accounting = emitted
            .into_iter()
            .map(|((signal, buffer), (scopes, grammars))| {
                self.buffer_accounting(signal, buffer, &scopes, &grammars)
            })
            .collect();
        CollectorResult {
            emissions,
            unresolved,
            truncated: self.depth_truncated || self.emission_truncated,
            accounting,
        }
    }

    fn buffer_accounting(
        &self,
        signal: u32,
        buffer: SymbolId,
        emission_scopes: &BTreeSet<u32>,
        grammars: &[GrammarNode],
    ) -> BufferAccounting {
        let mut held: BTreeSet<(u32, u32)> = BTreeSet::new();
        let mut prefix_ranges: Vec<Span> = Vec::new();
        for grammar in grammars {
            grammar.walk(&mut |node| match node {
                GrammarNode::Number(write) | GrammarNode::String(write) | GrammarNode::Byte(write) => {
                    // A helper write's accounting site is the caller's helper
                    // call, matching `collect_buffer_writes`; its own span is
                    // inside the helper body and repeats per call site.
                    match write.via_helper {
                        Some(via) => held.insert((via.start, via.end)),
                        None => held.insert((write.span.start, write.span.end)),
                    };
                }
                GrammarNode::Opaque { span, .. } | GrammarNode::Unresolved { span, .. } => {
                    held.insert((span.start, span.end));
                }
                GrammarNode::RawPrefix(prefix) => prefix_ranges.push(prefix.span),
                _ => {}
            });
        }
        let mut all: Vec<(u32, Span, String)> = Vec::new();
        for (scope, log) in &self.scope_logs {
            collect_buffer_writes(log, buffer, *scope, &mut all);
        }
        all.sort_by_key(|(_, span, _)| (span.start, span.end));
        all.dedup_by_key(|(_, span, _)| (span.start, span.end));
        let mut excluded = Vec::new();
        for (scope, span, text) in &all {
            let in_prefix = prefix_ranges
                .iter()
                .any(|range| range.start <= span.start && span.end <= range.end);
            if held.contains(&(span.start, span.end)) || in_prefix {
                continue;
            }
            excluded.push(ExcludedWrite {
                span: *span,
                text: text.clone(),
                reason: if emission_scopes.contains(scope) {
                    "not_on_emission_path".into()
                } else {
                    "other_function_scope".into()
                },
            });
        }
        let declaration = self.buffer_span(buffer);
        BufferAccounting {
            signal,
            buffer: self.name(buffer).unwrap_or_default(),
            declaration,
            writes: all.len(),
            in_grammar: all.len() - excluded.len(),
            excluded,
        }
    }

    fn build_emission(
        &self,
        signal: u32,
        route: ObserverRoute,
        raw: &RawEmission,
        unresolved: &mut Vec<Unresolved>,
    ) -> Emission {
        match &raw.payload {
            PayloadResolution::Buffer(buffer) => {
                let reachable =
                    reachable_ops(&raw.path, Some(Vec::new()), None).unwrap_or_default();
                let scope_ops: &[IOp] = &reachable;
                let version = raw
                    .buffer_version
                    .unwrap_or_else(|| self.version_of(*buffer));
                let prefix_info = raw.prefix.as_ref();
                let prefix = prefix_info.map(|prefix| PrefixSeed {
                    seed_word: prefix.seed_word.clone(),
                    prefix_binding: prefix.binding.clone(),
                    byte_width: prefix.elements.len(),
                    elements: prefix
                        .elements
                        .iter()
                        .map(|(text, _)| text.clone())
                        .collect(),
                });
                let grammar = self.emission_grammar(
                    scope_ops,
                    *buffer,
                    version,
                    raw.origin,
                    raw.scope,
                    prefix_info,
                );
                // The message stream is the one feeding this emission's own
                // writes: the lifetime-filtered grammar, with each choice arm's
                // buffer alias resolved. A reused payload register's earlier
                // allocation (another emission's writes) never supplies it.
                let mask = self.mask_for(scope_ops, &grammar_write_sites(&grammar));
                // Project the same lifetime-filtered grammar into the legacy
                // wire API. Replaying the unfiltered path here would reintroduce
                // writes from earlier allocations and flatten branch prefixes.
                let order = wire_order(&self.grammar_wire(&grammar, scope_ops, *buffer, false));
                let wire = self.grammar_wire(&grammar, scope_ops, *buffer, true);
                grammar.walk(&mut |node| {
                    if let GrammarNode::Unresolved { span, text, reason } = node {
                        if matches!(
                            reason.as_str(),
                            "buffer_alias_flow_unresolved"
                                | "try_catch_control_flow_unresolved"
                                | "switch_fallthrough_control_flow_unresolved"
                        ) {
                            unresolved.push(Unresolved {
                                span: *span,
                                reason: reason.clone(),
                                detail: text.clone(),
                            });
                        }
                    }
                });
                if prefix.is_none() {
                    unresolved.push(Unresolved {
                        span: raw.origin,
                        reason: "prefix_seed_unresolved".into(),
                        detail: format!(
                            "buffer {} has no resolved 4-byte prefix seed",
                            self.name(*buffer).unwrap_or_default()
                        ),
                    });
                }
                Emission {
                    signal,
                    origin: raw.origin,
                    route,
                    variant: EmissionVariant::RawBuffer,
                    prefix,
                    mask,
                    wire,
                    order,
                    session_state: None,
                    grammar,
                }
            }
            PayloadResolution::Object(fields) => {
                let grammar = GrammarNode::Sequence {
                    items: fields
                        .iter()
                        .map(|field| {
                            let write = WriteNode {
                                stream_instance: None,
                                stream_draw: None,
                                position_span: None,
                                stream_factory: None,
                                string_instance: None,
                                writer_factory: None,
                                span: field.span,
                                value_span: field.span,
                                value: field.value.clone().unwrap_or_default(),
                                resolved: None,
                                mask: MaskForm::Unmasked,
                                mask_source: None,
                                xor_literal: None,
                                position: Some(field.key.clone()),
                                via_helper: None,
                                helper_arguments: Vec::new(),
                                provenance: None,
                            };
                            match field.kind {
                                Some(LeafKind::Number) => GrammarNode::Number(write),
                                Some(LeafKind::String) => GrammarNode::String(write),
                                Some(LeafKind::Byte) => GrammarNode::Byte(write),
                                None => GrammarNode::Unresolved {
                                    span: field.span,
                                    text: field.key.clone(),
                                    reason: "generic_field_type_unprovable".into(),
                                },
                            }
                        })
                        .collect(),
                };
                let wire = WireOp::Sequence(
                    fields
                        .iter()
                        .map(|field| match field.kind {
                            Some(kind) => WireOp::Leaf(Leaf {
                                source: field.value.as_deref().and_then(field_source),
                                kind,
                                span: field.span,
                                buffer: None,
                                label: Some(field.key.clone()),
                                masked: false,
                                value: field.value.clone(),
                                index: None,
                                xor_literal: None,
                                byte_width: None,
                            }),
                            None => WireOp::Unknown {
                                span: field.span,
                                text: field.key.clone(),
                                reason: "generic_field_type_unprovable".into(),
                            },
                        })
                        .collect(),
                );
                // The generic envelope's byte encoder is not modelled, but that
                // is not a missing field: when every field's type is proven the
                // ordered logical writes are complete and the signal is partial.
                // Only an untyped field keeps the emission unresolved.
                if fields.iter().any(|field| field.kind.is_none()) {
                    unresolved.push(Unresolved {
                        span: raw.origin,
                        reason: "generic_encoder_rule_unapplied".into(),
                        detail: "raw named object has an untyped field; the generic token encoder is not modelled".into(),
                    });
                }
                let order = wire_order(&wire);
                Emission {
                    signal,
                    origin: raw.origin,
                    route,
                    variant: EmissionVariant::GenericObject,
                    prefix: None,
                    mask: None,
                    wire,
                    order,
                    session_state: None,
                    grammar,
                }
            }
            PayloadResolution::Scalar(tree) => {
                // A bare value through the generic codec: one keyless value
                // whose runtime type is one of the typed arms.
                let grammar = scalar_grammar(tree);
                let wire = scalar_wire(tree);
                let order = wire_order(&wire);
                Emission {
                    signal,
                    origin: raw.origin,
                    route,
                    variant: EmissionVariant::GenericObject,
                    prefix: None,
                    mask: None,
                    wire,
                    order,
                    session_state: None,
                    grammar,
                }
            }
            PayloadResolution::Unresolved(text) => {
                unresolved.push(Unresolved {
                    span: raw.origin,
                    reason: "unresolved_payload".into(),
                    detail: text.clone(),
                });
                Emission {
                    signal,
                    origin: raw.origin,
                    route,
                    variant: EmissionVariant::RawBuffer,
                    prefix: None,
                    mask: None,
                    wire: WireOp::Unknown {
                        span: raw.origin,
                        text: text.clone(),
                        reason: "unresolved_payload".into(),
                    },
                    order: None,
                    session_state: None,
                    grammar: GrammarNode::Unresolved {
                        span: raw.origin,
                        text: text.clone(),
                        reason: "unresolved_payload".into(),
                    },
                }
            }
        }
    }

    fn buffer_written_elsewhere(&self, buffer: SymbolId, exclude_scope: u32) -> bool {
        self.scope_logs
            .iter()
            .any(|(scope, log)| *scope != exclude_scope && ops_mention_buffer(log, buffer))
    }

    fn mask_for(&self, ops: &[IOp], writes: &BTreeSet<WriteSite>) -> Option<MaskSeed> {
        let mut mask: Option<&MaskInfo> = None;
        let mut xor_literal: Option<f64> = None;
        let mut xor_source: Option<String> = None;
        let mut xor_span = Span::default();
        collect_masks(
            ops,
            writes,
            &mut mask,
            &mut xor_literal,
            &mut xor_source,
            &mut xor_span,
        );
        // The last snapshotted write mask is the one in force at the emission.
        if let Some(mask) = mask {
            return Some(MaskSeed {
                instance: Some(mask.instance.into()),
                seed_binding: mask.seed_binding,
                factory: mask.factory.clone(),
                seed_word: mask.seed_word.clone(),
                xor_literal: mask.xor_literal.or(xor_literal),
                xor_source: mask.xor_source.clone().or(xor_source),
                xor_span: if mask.xor_span != Span::default() {
                    mask.xor_span
                } else {
                    xor_span
                },
                argument: mask.argument.clone(),
            });
        }
        // A bare `expr ^ literal` write has no PRNG factory binding but still
        // carries its exact XOR evidence; the caller decides whether that is
        // enough to claim a mask.
        if xor_literal.is_some() || xor_source.is_some() {
            return Some(MaskSeed {
                instance: None,
                seed_binding: None,
                factory: None,
                seed_word: None,
                xor_literal,
                xor_source,
                xor_span,
                argument: String::new(),
            });
        }
        None
    }

    /// The contract grammar of one emission of `buffer` from its snapshotted
    /// path. A prefix established on the path stays at its position; one
    /// established outside the replayed path (another scope) leads the buffer.
    fn emission_grammar(
        &self,
        path: &[IOp],
        buffer: SymbolId,
        version: u32,
        origin: Span,
        scope: u32,
        prefix: Option<&PrefixInfo>,
    ) -> GrammarNode {
        let mut items = hoist_common_suffixes(self.grammar_items(path, buffer, version, 0));
        let positioned = items.iter().any(|item| {
            let mut found = false;
            item.walk(&mut |node| found |= matches!(node, GrammarNode::RawPrefix(_)));
            found
        });
        if !positioned {
            if let Some(prefix) = prefix {
                items.insert(
                    0,
                    GrammarNode::RawPrefix(PrefixNode {
                        seed_binding: prefix.seed_binding,
                        span: prefix.span,
                        spelling: PrefixSpelling::Inherited,
                        seed_word: prefix.seed_word.clone(),
                        elements: prefix.elements.iter().map(|(text, _)| text.clone()).collect(),
                    }),
                );
            }
        }
        if items.is_empty() {
            return GrammarNode::Unresolved {
                span: origin,
                text: self.name(buffer).unwrap_or_default(),
                reason: if self.buffer_written_elsewhere(buffer, scope) {
                    "write_order_crosses_function_scope".into()
                } else {
                    "no_writes_for_buffer".into()
                },
            };
        }
        sequence(items)
    }

    fn grammar_items(
        &self,
        ops: &[IOp],
        buffer: SymbolId,
        version: u32,
        depth: usize,
    ) -> Vec<GrammarNode> {
        let mut items = Vec::new();
        self.append_grammar_items(ops, buffer, version, depth, &mut items, &OpHistory::Root);
        group_prefix_stores(items)
    }

    fn append_grammar_items(
        &self,
        ops: &[IOp],
        buffer: SymbolId,
        version: u32,
        depth: usize,
        items: &mut Vec<GrammarNode>,
        prior: &OpHistory<'_>,
    ) {
        if depth > self.limits.max_wire_depth {
            if ops_mention_buffer(ops, buffer) {
                items.push(GrammarNode::Unresolved {
                    span: Span::default(),
                    text: String::new(),
                    reason: "grammar_depth_limit".into(),
                });
            }
            return;
        }
        // Names that, after a choice, hold this buffer's array on every arm
        // (`if (g) { a = b; } else { b = a; } a[6] = v; emit(b)`): their
        // writes after the choice are this buffer's writes.
        let mut co_names: Vec<SymbolId> = Vec::new();
        for (op_index, op) in ops.iter().enumerate() {
            match op {
                IOp::Seq(children) => {
                    let history = OpHistory::Then {
                        prior,
                        ops: &ops[..op_index],
                    };
                    self.append_grammar_items(
                        children,
                        buffer,
                        version,
                        depth + 1,
                        items,
                        &history,
                    );
                    continue;
                }
                IOp::Prefix {
                    buffer: target,
                    node,
                } if *target == buffer && node.spelling == PrefixSpelling::ArrayLiteral => {
                    // This allocation replaces the array. Earlier writes to a
                    // same-spelled binding cannot belong to the new payload.
                    co_names.clear();
                    items.clear();
                    items.push(GrammarNode::RawPrefix(node.clone()));
                    continue;
                }
                IOp::Choice(site, branches) if depth <= self.limits.max_wire_depth => {
                    co_names = choice_co_names(site, branches.len(), buffer);
                    let before = items.clone();
                    let history = OpHistory::Then {
                        prior,
                        ops: &ops[..op_index],
                    };
                    // The flat history is only needed for an aliased branch.
                    let flat: std::cell::OnceCell<Vec<IOp>> = std::cell::OnceCell::new();
                    let states: Vec<Vec<GrammarNode>> = branches
                        .iter()
                        .enumerate()
                        .map(|(index, branch)| {
                            let target = site
                                .buffer_aliases
                                .get(index)
                                .map_or(buffer, |aliases| resolve_alias_map(aliases, buffer));
                            let mut state = if target == buffer {
                                before.clone()
                            } else {
                                self.grammar_items(
                                    flat.get_or_init(|| history.to_vec()),
                                    target,
                                    version,
                                    depth + 1,
                                )
                            };
                            self.append_grammar_items(
                                std::slice::from_ref(branch),
                                target,
                                version,
                                depth + 1,
                                &mut state,
                                &history,
                            );
                            // An arm that allocates a short prefix literal
                            // (`b = [w >>> 24]; b[1] = …; …`) completes it
                            // with its own index stores.
                            group_prefix_stores(state)
                        })
                        .collect();
                    if states.is_empty() || states.iter().all(|state| *state == before) {
                        continue;
                    }
                    if states.iter().all(|state| *state == states[0]) {
                        *items = states[0].clone();
                        continue;
                    }
                    // Factor only a genuinely common prefix. A branch that
                    // allocates a fresh buffer keeps its own RAW4 and drops
                    // the previous buffer's writes on that branch alone.
                    let common = (0..states.iter().map(Vec::len).min().unwrap_or(0))
                        .take_while(|&i| states.iter().all(|state| state[i] == states[0][i]))
                        .count();
                    *items = states[0][..common].to_vec();
                    let arms = states
                        .into_iter()
                        .enumerate()
                        .map(|(i, state)| ChoiceArm {
                            label: site.labels.get(i).cloned().unwrap_or_default(),
                            body: sequence(state[common..].to_vec()),
                        })
                        .collect();
                    items.push(GrammarNode::Choice {
                        kind: site.kind,
                        span: site.span,
                        condition: site.condition.clone(),
                        arms,
                    });
                    continue;
                }
                _ => {}
            }
            let node = self.grammar_one(op, buffer, version, depth + 1).or_else(|| {
                co_names
                    .iter()
                    .find_map(|name| matches!(op, IOp::Leaf(_)).then(|| self.grammar_one(op, *name, version, depth + 1)).flatten())
            });
            match node {
                Some(GrammarNode::Sequence { items: inner }) => items.extend(inner),
                Some(node) => items.push(node),
                None => {}
            }
        }
    }

    fn grammar_one(
        &self,
        op: &IOp,
        buffer: SymbolId,
        version: u32,
        depth: usize,
    ) -> Option<GrammarNode> {
        if depth > self.limits.max_wire_depth {
            return ops_mention_buffer(std::slice::from_ref(op), buffer).then(|| {
                GrammarNode::Unresolved {
                    span: Span::default(),
                    text: String::new(),
                    reason: "grammar_depth_limit".into(),
                }
            });
        }
        match op {
            IOp::Exit(_) => None,
            IOp::Seq(children) => {
                let items = self.grammar_items(children, buffer, version, depth);
                (!items.is_empty()).then(|| sequence(items))
            }
            IOp::Choice(site, branches) => {
                let arms: Vec<GrammarNode> = branches
                    .iter()
                    .map(|branch| {
                        self.grammar_one(branch, buffer, version, depth + 1)
                            .unwrap_or_else(GrammarNode::empty)
                    })
                    .collect();
                if arms.iter().all(GrammarNode::is_empty) {
                    return None;
                }
                Some(GrammarNode::Choice {
                    kind: site.kind,
                    span: site.span,
                    condition: site.condition.clone(),
                    arms: arms
                        .into_iter()
                        .enumerate()
                        .map(|(index, body)| ChoiceArm {
                            label: site.labels.get(index).cloned().unwrap_or_default(),
                            body,
                        })
                        .collect(),
                })
            }
            IOp::Repeat { control, body } => {
                let body = self.grammar_one(body, buffer, version, depth + 1)?;
                if body.is_empty() {
                    return None;
                }
                Some(GrammarNode::Repeat {
                    loop_kind: control.kind,
                    direction: control.direction,
                    span: control.span,
                    header: control.source.clone(),
                    count: control.count,
                    body: Box::new(body),
                })
            }
            IOp::Leaf(leaf) => (leaf.buffer == Some(buffer)).then(|| {
                let write = WriteNode {
                    stream_instance: leaf.mask.as_ref().map(|m| m.instance.into()),
                    stream_draw: leaf.mask.as_ref().and_then(|m| m.predrawn.map(Into::into)),
                    stream_factory: leaf.mask.as_ref().map(|m| m.factory_binding.into()),
                    string_instance: leaf.string_instance.map(Into::into),
                    writer_factory: leaf.writer_factory.map(Into::into),
                    span: leaf.span,
                    value_span: leaf.logical_span,
                    value: leaf.logical_value.clone(),
                    resolved: leaf.resolved_value.clone(),
                    mask: leaf.mask_form,
                    mask_source: leaf.mask_source.clone(),
                    xor_literal: match leaf.mask_form {
                        MaskForm::Stream | MaskForm::Unresolved => None,
                        _ => leaf.xor_literal,
                    },
                    position_span: leaf.index_span.map(Into::into),
                    position: leaf
                        .prefix_element
                        .map(|element| element.to_string())
                        .or_else(|| leaf.index.clone()),
                    via_helper: leaf.helper_call.map(SourceSpan::from),
                    helper_arguments: leaf
                        .helper_arguments
                        .iter()
                        .copied()
                        .map(SourceSpan::from)
                        .collect(),
                    provenance: None,
                };
                match (leaf.kind, leaf.prefix_element) {
                    (LeafKind::Number, _) => GrammarNode::Number(write),
                    (LeafKind::String, _) => GrammarNode::String(write),
                    (LeafKind::Byte, Some(_)) => GrammarNode::Unresolved {
                        span: leaf.span,
                        text: leaf.value.clone().unwrap_or_default(),
                        reason: format!("prefix_store:{}", write.position.unwrap_or_default()),
                    },
                    (LeafKind::Byte, None) => GrammarNode::Byte(write),
                }
            }),
            IOp::Unknown {
                span,
                text,
                reason,
                buffer: unknown_buffer,
            } => (*unknown_buffer == Some(buffer)).then(|| GrammarNode::Unresolved {
                span: *span,
                text: text.clone(),
                reason: reason.clone(),
            }),
            IOp::Prefix {
                buffer: prefixed,
                node,
            } => (*prefixed == buffer).then(|| GrammarNode::RawPrefix(node.clone())),
            IOp::Opaque {
                span,
                callee,
                buffer: passed,
                version: passed_version,
            } => (*passed == buffer && *passed_version == version).then(|| GrammarNode::Opaque {
                span: *span,
                callee: callee.clone(),
                reason: "buffer_passed_to_unmodelled_call".into(),
            }),
            IOp::Unsupported {
                span,
                text,
                reason,
                buffers,
            } => buffers.contains(&buffer).then(|| GrammarNode::Unresolved {
                span: *span,
                text: text.clone(),
                reason: reason.clone(),
            }),
        }
    }

    fn grammar_wire(
        &self,
        grammar: &GrammarNode,
        ops: &[IOp],
        buffer: SymbolId,
        prefixes: bool,
    ) -> WireOp {
        fn leaf_at(ops: &[IOp], span: Span, buffer: SymbolId) -> Option<&ILeaf> {
            ops.iter().find_map(|op| match op {
                IOp::Seq(children) | IOp::Choice(_, children) => leaf_at(children, span, buffer),
                IOp::Repeat { body, .. } => {
                    leaf_at(std::slice::from_ref(body.as_ref()), span, buffer)
                }
                IOp::Leaf(leaf) if leaf.span == span && leaf.buffer == Some(buffer) => Some(leaf),
                _ => None,
            })
        }
        fn control_at(ops: &[IOp], span: Span) -> Option<LoopControl> {
            ops.iter().find_map(|op| match op {
                IOp::Repeat { control, .. } if control.span == span => Some(control.clone()),
                IOp::Repeat { body, .. } => control_at(std::slice::from_ref(body.as_ref()), span),
                IOp::Seq(children) | IOp::Choice(_, children) => control_at(children, span),
                _ => None,
            })
        }
        match grammar {
            GrammarNode::Sequence { items } => WireOp::Sequence(
                items
                    .iter()
                    .map(|g| self.grammar_wire(g, ops, buffer, prefixes))
                    .collect(),
            ),
            GrammarNode::Choice { arms, .. } => WireOp::Choice(
                arms.iter()
                    .map(|a| self.grammar_wire(&a.body, ops, buffer, prefixes))
                    .collect(),
            ),
            GrammarNode::Repeat {
                span,
                body,
                loop_kind,
                direction,
                header,
                count,
            } => WireOp::Repeat {
                control: control_at(ops, *span).unwrap_or(LoopControl {
                    kind: *loop_kind,
                    init: None,
                    test: None,
                    update: None,
                    count: *count,
                    source: header.clone(),
                    direction: *direction,
                    span: *span,
                }),
                body: Box::new(self.grammar_wire(body, ops, buffer, prefixes)),
            },
            GrammarNode::RawPrefix(prefix) => {
                if !prefixes {
                    return WireOp::Sequence(Vec::new());
                }
                let mut cursor = prefix.span.start as usize;
                WireOp::Sequence(
                    prefix
                        .elements
                        .iter()
                        .map(|text| {
                            let end = prefix.span.end as usize;
                            let span = self
                                .source
                                .get(cursor..end)
                                .and_then(|part| part.find(text))
                                .map(|at| {
                                    let start = cursor + at;
                                    cursor = start + text.len();
                                    Span::new(start as u32, cursor as u32)
                                })
                                .unwrap_or(prefix.span);
                            WireOp::Leaf(Leaf {
                                source: field_source(text),
                                kind: LeafKind::Byte,
                                span,
                                buffer: self.name(buffer),
                                label: None,
                                masked: false,
                                value: Some(text.clone()),
                                index: None,
                                xor_literal: None,
                                byte_width: Some(1),
                            })
                        })
                        .collect(),
                )
            }
            GrammarNode::Number(write) | GrammarNode::String(write) | GrammarNode::Byte(write) => {
                let leaf = leaf_at(ops, write.span, buffer);
                WireOp::Leaf(Leaf {
                    source: leaf.and_then(|l| l.value.as_deref()).and_then(field_source),
                    kind: match grammar {
                        GrammarNode::Number(_) => LeafKind::Number,
                        GrammarNode::String(_) => LeafKind::String,
                        _ => LeafKind::Byte,
                    },
                    span: write.span,
                    buffer: self.name(buffer),
                    label: None,
                    masked: leaf.map_or(write.mask != MaskForm::Unmasked, |l| l.masked),
                    value: leaf
                        .and_then(|l| l.value.clone())
                        .or_else(|| Some(write.value.clone())),
                    index: leaf
                        .and_then(|l| l.index.clone())
                        .or_else(|| write.position.clone()),
                    xor_literal: write.xor_literal,
                    byte_width: leaf.and_then(|l| l.byte_width),
                })
            }
            GrammarNode::Opaque {
                span,
                callee,
                reason,
            } => WireOp::Unknown {
                span: *span,
                text: callee.clone(),
                reason: reason.clone(),
            },
            GrammarNode::Unresolved { span, text, reason } => WireOp::Unknown {
                span: *span,
                text: text.clone(),
                reason: reason.clone(),
            },
        }
    }

    // ----- scanning ----------------------------------------------------------

    fn scan_statements(&mut self, statements: &[Statement<'a>]) {
        for statement in statements {
            if self.node_budget == 0 {
                self.depth_truncated = true;
                return;
            }
            self.scan_statement(statement, 0);
        }
    }

    /// Appends a structural op to both the current scope's tree and the current
    /// execution path, keeping them in lockstep.
    fn push_op(&mut self, op: IOp) {
        // Every write draws from the stream: a register drawn before it no
        // longer holds the next draw.
        if matches!(op, IOp::Leaf(_)) {
            self.register_draws.clear();
        }
        self.path.push(op.clone());
        self.ops.push(op);
    }

    fn scan_branch_statements(&mut self, statements: &[Statement<'a>], depth: usize) -> Vec<IOp> {
        let saved_ops = std::mem::take(&mut self.ops);
        let saved_path = self.path.clone();
        self.scan_statements_at(statements, depth + 1);
        let branch = std::mem::replace(&mut self.ops, saved_ops);
        self.path = saved_path;
        branch
    }

    fn scan_statements_at(&mut self, statements: &[Statement<'a>], depth: usize) {
        for statement in statements {
            if self.node_budget == 0 {
                self.depth_truncated = true;
                return;
            }
            self.scan_statement(statement, depth);
        }
    }

    /// A hoisted event callback can precede the collector's initialization
    /// of its captured observer handle. Admit only a plain top-level copy
    /// whose binding has exactly one write; a changing handle stays refused.
    fn seed_stable_observer_aliases(&mut self, statements: &[Statement<'a>]) {
        for statement in statements {
            let Statement::ExpressionStatement(statement) = statement else { continue };
            let Expression::AssignmentExpression(assignment) = &statement.expression else { continue };
            if assignment.operator != AssignmentOperator::Assign { continue; }
            let AssignmentTarget::AssignmentTargetIdentifier(left) = &assignment.left else { continue };
            let Some(symbol) = resolve_reference(left, self.scoping) else { continue };
            if self.scoping.get_resolved_references(symbol).filter(|r| r.is_write()).count() != 1 { continue; }
            if let Some(target) = self.observer_target(&assignment.right) {
                self.aliases_out.insert(symbol, target);
            }
        }
    }

    /// An endless loop whose body starts with its own exit test is the loop
    /// `while (test) { body }` followed by the exit path, which runs once when
    /// the test fails. Both spellings the traces use (`if (!t) { exit } body`
    /// and `if (t) { body } else { exit }`) normalize to that shape.
    /// The label naming the loop statement at `span`, if any.
    fn own_label(&self, span: Span) -> Option<&str> {
        self.statement_label
            .as_ref()
            .filter(|(labeled, _)| *labeled == span)
            .map(|(_, label)| label.as_str())
    }

    fn scan_guarded_loop(
        &mut self,
        mut control: LoopControl,
        update: Option<&Expression<'a>>,
        guarded: GuardedLoop<'_, 'a>,
        depth: usize,
    ) {
        let test = self.slice(guarded.test.span());
        control.test = Some(if guarded.negated {
            format!("!({test})")
        } else {
            test
        });
        let lowered_for_in = is_lowered_for_in(guarded.test, &guarded.body, self.scoping);
        control.kind = if lowered_for_in { LoopKind::ForIn } else { control.kind };
        control.direction = if lowered_for_in {
            LoopDirection::Iteration
        } else {
            loop_direction(Some(guarded.test), update, guarded.body.iter().copied(), self.scoping)
        };
        let entry = self.path.len();
        let first_emission = self.emissions.len();
        let scope = self.scope_id;
        let mut body_ops = reachable_ops(
            &self.scan_guarded_body(&guarded.body, depth),
            Some(Vec::new()),
            Some(Vec::new()),
        )
        .unwrap_or_default();
        if lowered_for_in {
            for op in &mut body_ops {
                collapse_lowered_wrapper(op);
            }
        }
        let repeat = IOp::Repeat {
            control,
            body: Box::new(IOp::Seq(body_ops)),
        };
        for emission in &mut self.emissions[first_emission..] {
            if emission.scope == scope && emission.path.len() >= entry {
                emission.path.insert(entry, repeat.clone());
            }
        }
        self.push_op(repeat);
        // The exit path runs once, after the iterations, on the enclosing path:
        // its writes stay in the scope log and on the path of what follows.
        for statement in guarded.exit.iter().copied() {
            if self.node_budget == 0 {
                self.depth_truncated = true;
                break;
            }
            // The normalizer has already exited this loop. Its local break
            // must not terminate the enclosing collector's emission path.
            if !matches!(statement, Statement::BreakStatement(_)) {
                self.scan_statement(statement, depth);
            }
        }
    }

    fn scan_guarded_body(&mut self, statements: &[&Statement<'a>], depth: usize) -> Vec<IOp> {
        let saved_ops = std::mem::take(&mut self.ops);
        let saved_path = self.path.clone();
        for statement in statements {
            if self.node_budget == 0 {
                self.depth_truncated = true;
                break;
            }
            self.scan_statement(statement, depth + 1);
        }
        let branch = std::mem::replace(&mut self.ops, saved_ops);
        self.path = saved_path;
        branch
    }

    /// Scans a loop body. An emission inside the body can follow any number
    /// of earlier complete iterations, so its path gets the whole body as a
    /// `Repeat` at the loop's position, before the writes of the iteration it
    /// runs in.
    fn scan_loop(
        &mut self,
        control: LoopControl,
        body: &Statement<'a>,
        tail: &[&Expression<'a>],
        depth: usize,
    ) {
        let entry = self.path.len();
        let first_emission = self.emissions.len();
        let scope = self.scope_id;
        let saved_ops = std::mem::take(&mut self.ops);
        let saved_path = self.path.clone();
        self.scan_statement(body, depth + 1);
        let body_len = self.ops.len();
        // Updates and do/while tests execute in the iteration. In particular
        // `do { --n; } while (n >= 0 && (p = write(...), true))` writes in the
        // right operand of its test; scanning only the body silently loses it.
        // A do/while test's `&&` skip route is the loop exit, so its right
        // operand is the rest of every repeated iteration, exactly like the
        // guarded body of the equivalent `for (;;) { …; if (t) { …; continue; }
        // else { exit } }` (scan_guarded_loop), not a per-item choice.
        let do_while = control.kind == LoopKind::DoWhile;
        for expression in tail {
            if do_while {
                self.walk_loop_exit_test(expression, depth + 1);
            } else {
                self.walk_expression(expression, depth + 1);
            }
        }
        let mut body_ops = std::mem::replace(&mut self.ops, saved_ops);
        let tail_ops = body_ops.split_off(body_len);
        let body_ops =
            reachable_ops(&body_ops, Some(tail_ops.clone()), Some(tail_ops)).unwrap_or_default();
        self.path = saved_path;
        let repeat = IOp::Repeat {
            control,
            body: Box::new(IOp::Seq(body_ops)),
        };
        for emission in &mut self.emissions[first_emission..] {
            if emission.scope == scope && emission.path.len() >= entry {
                emission.path.insert(entry, repeat.clone());
            }
        }
        self.push_op(repeat);
    }

    /// Walks a loop test whose failing route leaves the loop: the operands of
    /// a top-level `&&` chain run in order on every iteration that repeats.
    fn walk_loop_exit_test(&mut self, expression: &Expression<'a>, depth: usize) {
        match expression.without_parentheses() {
            Expression::LogicalExpression(logical) if logical.operator == LogicalOperator::And => {
                self.walk_loop_exit_test(&logical.left, depth);
                self.walk_expression(&logical.right, depth);
            }
            _ => self.walk_expression(expression, depth),
        }
    }

    fn scan_branch(&mut self, statement: &Statement<'a>, depth: usize) -> Vec<IOp> {
        let saved_ops = std::mem::take(&mut self.ops);
        let saved_path = self.path.clone();
        self.scan_statement(statement, depth + 1);
        let branch = std::mem::replace(&mut self.ops, saved_ops);
        self.path = saved_path;
        branch
    }

    /// Scans a nested function body in its own write scope. Writes recorded here
    /// belong to this body's execution, never to the declaration position of the
    /// enclosing collector. `own_arguments` is true for an ordinary function or
    /// function expression and false for an arrow, which keeps the enclosing
    /// function's `arguments`.
    fn scan_nested_function(
        &mut self,
        function_start: u32,
        statements: &[Statement<'a>],
        depth: usize,
        own_arguments: bool,
    ) {
        let saved_ops = std::mem::take(&mut self.ops);
        let saved_path = std::mem::take(&mut self.path);
        let saved_scope = self.scope_id;
        let saved_function = self.current_function;
        self.scope_counter += 1;
        let scope = self.scope_counter;
        self.scope_id = scope;
        if own_arguments {
            self.fn_depth += 1;
            self.current_function = Some(function_start);
        }
        self.scan_statements_at(statements, depth);
        if own_arguments {
            self.fn_depth -= 1;
            self.current_function = saved_function;
        }
        let log = std::mem::replace(&mut self.ops, saved_ops);
        self.scope_logs.insert(scope, log);
        self.path = saved_path;
        self.scope_id = saved_scope;
    }

    fn scan_statement(&mut self, statement: &Statement<'a>, depth: usize) {
        if depth > self.limits.max_wire_depth {
            self.depth_truncated = true;
            return;
        }
        if let Statement::ForStatement(loop_) = statement {
            if let Some(init) = &loop_.init {
                match init {
                    ForStatementInit::VariableDeclaration(declaration) => {
                        for declarator in &declaration.declarations {
                            self.record_binding(declarator);
                            if let Some(value) = &declarator.init {
                                self.walk_expression(value, depth);
                            }
                        }
                    }
                    _ => {
                        if let Some(value) = init.as_expression() {
                            self.walk_expression(value, depth);
                        }
                    }
                }
            }
            if let Some(test) = &loop_.test {
                self.walk_expression(test, depth);
            }
        }
        match statement {
            Statement::ExpressionStatement(statement) => {
                self.walk_expression(&statement.expression, depth);
            }
            Statement::VariableDeclaration(declaration) => {
                for declarator in &declaration.declarations {
                    self.record_binding(declarator);
                    if let Some(init) = declarator.init.as_ref() {
                        self.walk_expression(init, depth);
                    }
                }
            }
            Statement::BlockStatement(block) => self.scan_statements_at(&block.body, depth),
            Statement::IfStatement(if_statement) => {
                // The test always runs first; its writes are unconditional and
                // belong on the enclosing path before either arm is scanned.
                self.walk_expression(&if_statement.test, depth + 1);
                let initial_aliases = self.buffer_aliases.clone();
                let initial_payloads = self.payload_buffers.clone();
                let initial_masks = self.masks.clone();
                let initial_scalars = self.payload_scalars.clone();
                let mut branches = vec![self.scan_branch(&if_statement.consequent, depth)];
                let then_scalars = std::mem::replace(&mut self.payload_scalars, initial_scalars);
                let mut then_aliases = self.buffer_aliases.clone();
                let then_payloads = self.payload_buffers.clone();
                let then_masks = std::mem::replace(&mut self.masks, initial_masks);
                self.buffer_aliases = initial_aliases.clone();
                self.payload_buffers = initial_payloads;
                match if_statement.alternate.as_ref() {
                    Some(alternate) => branches.push(self.scan_branch(alternate, depth)),
                    None => branches.push(Vec::new()),
                }
                let mut else_aliases = self.buffer_aliases.clone();
                let else_payloads = self.payload_buffers.clone();
                // A bare scalar binding survives the join when both paths leave
                // it typed: unchanged on both, or an if/else choice of the two.
                let else_scalars = std::mem::take(&mut self.payload_scalars);
                let condition = self.slice(if_statement.test.span());
                self.payload_scalars = then_scalars
                    .into_iter()
                    .filter_map(|(symbol, then)| {
                        let other = else_scalars.get(&symbol)?.clone();
                        if then == other {
                            return Some((symbol, then));
                        }
                        let tree = ScalarTree::join(
                            ChoiceKind::If,
                            if_statement.span,
                            condition.clone(),
                            format!("if ({condition}) …"),
                            then,
                            other,
                        );
                        Some((symbol, tree))
                    })
                    .collect();
                // A mask binding survives the join only when both paths leave
                // it naming the same stream kind (factory and salt). Entries
                // both arms inherited unchanged are equal and survive.
                self.masks.retain(|symbol, else_mask| {
                    then_masks
                        .get(symbol)
                        .is_some_and(|then_mask| same_mask_stream(then_mask, else_mask))
                });
                // A payload holder can select different [aux, bytes] arrays
                // on the two paths. Keep the holder as a symbolic join and
                // resolve its byte buffer separately in each grammar arm.
                self.payload_buffers = then_payloads.iter().filter_map(|(&holder, &left)| {
                    let &right = else_payloads.get(&holder)?;
                    if left == right { Some((holder, left)) } else {
                        then_aliases.insert(holder, left);
                        else_aliases.insert(holder, right);
                        Some((holder, holder))
                    }
                }).collect();
                // Only aliases common to both paths survive the join. The
                // branch snapshots retain each concrete buffer for the grammar.
                self.buffer_aliases = then_aliases
                    .iter()
                    .filter_map(|(&key, &value)| {
                        (else_aliases.get(&key) == Some(&value)).then_some((key, value))
                    })
                    .collect();
                self.join_swapped_aliases(&then_aliases, &else_aliases);
                let site = ChoiceSite {
                    buffer_aliases: vec![then_aliases, else_aliases],
                    kind: ChoiceKind::If,
                    span: if_statement.span,
                    condition: Some(self.slice(if_statement.test.span())),
                    labels: vec!["then".into(), "else".into()],
                };
                self.push_op(IOp::Choice(site, branches.into_iter().map(IOp::Seq).collect()));
            }
            Statement::WhileStatement(while_statement)
                if matches!(
                    while_statement.test.without_parentheses(),
                    Expression::BooleanLiteral(literal) if literal.value
                ) && guarded_loop(&while_statement.body, self.own_label(while_statement.span)).is_some() =>
            {
                let control = LoopControl {
                    kind: LoopKind::While,
                    init: None,
                    test: None,
                    update: None,
                    count: None,
                    source: self.loop_header(while_statement.span.start, &while_statement.body),
                    direction: LoopDirection::Unknown,
                    span: while_statement.span,
                };
                if let Some(guarded) = guarded_loop(&while_statement.body, self.own_label(while_statement.span)) {
                    self.scan_guarded_loop(control, None, guarded, depth);
                }
            }
            Statement::WhileStatement(while_statement) => {
                self.walk_expression(&while_statement.test, depth + 1);
                let control = LoopControl {
                    kind: LoopKind::While,
                    init: None,
                    test: Some(self.slice(while_statement.test.span())),
                    update: None,
                    count: None,
                    source: self.loop_header(while_statement.span.start, &while_statement.body),
                    direction: loop_direction(
                        Some(&while_statement.test),
                        None,
                        statement_list(&while_statement.body).iter(),
                        self.scoping,
                    ),
                    span: while_statement.span,
                };
                self.scan_loop(
                    control,
                    &while_statement.body,
                    &[&while_statement.test],
                    depth,
                );
            }
            Statement::DoWhileStatement(do_while) => {
                let control = LoopControl {
                    kind: LoopKind::DoWhile,
                    init: None,
                    test: Some(self.slice(do_while.test.span())),
                    update: None,
                    count: None,
                    source: self.loop_header(do_while.span.start, &do_while.body),
                    direction: loop_direction(
                        Some(&do_while.test),
                        None,
                        statement_list(&do_while.body).iter(),
                        self.scoping,
                    ),
                    span: do_while.span,
                };
                self.scan_loop(control, &do_while.body, &[&do_while.test], depth);
            }
            Statement::ForStatement(for_statement) if for_statement.test.is_none() => {
                let control = self.for_control(for_statement);
                match guarded_loop(&for_statement.body, self.own_label(for_statement.span)) {
                    Some(guarded) => self.scan_guarded_loop(
                        control,
                        for_statement.update.as_ref(),
                        guarded,
                        depth,
                    ),
                    None => self.scan_loop(
                        control,
                        &for_statement.body,
                        &for_statement.update.iter().collect::<Vec<_>>(),
                        depth,
                    ),
                }
            }
            Statement::ForStatement(for_statement) => {
                let control = self.for_control(for_statement);
                let tail = for_statement
                    .update
                    .iter()
                    .chain(for_statement.test.iter())
                    .collect::<Vec<_>>();
                self.scan_loop(control, &for_statement.body, &tail, depth);
            }
            Statement::ForInStatement(for_in) => {
                // The iterated object is evaluated once before the first pass.
                self.walk_expression(&for_in.right, depth + 1);
                let control = LoopControl {
                    kind: LoopKind::ForIn,
                    init: Some(self.slice(for_in.left.span())),
                    test: None,
                    update: None,
                    count: None,
                    source: self.loop_header(for_in.span.start, &for_in.body),
                    direction: LoopDirection::Iteration,
                    span: for_in.span,
                };
                self.scan_loop(control, &for_in.body, &[], depth);
            }
            Statement::ForOfStatement(for_of) => {
                self.walk_expression(&for_of.right, depth + 1);
                let control = LoopControl {
                    kind: LoopKind::ForOf,
                    init: Some(self.slice(for_of.left.span())),
                    test: None,
                    update: None,
                    count: None,
                    source: self.loop_header(for_of.span.start, &for_of.body),
                    direction: LoopDirection::Iteration,
                    span: for_of.span,
                };
                self.scan_loop(control, &for_of.body, &[], depth);
            }
            Statement::FunctionDeclaration(function) => {
                if let Some(body) = function.body.as_ref() {
                    self.scan_nested_function(function.span.start, &body.statements, depth, true);
                }
            }
            Statement::ReturnStatement(return_statement) => {
                if let Some(argument) = return_statement.argument.as_ref() {
                    self.walk_expression(argument, depth);
                }
                self.push_op(IOp::Exit(ControlExit::Return));
            }
            Statement::ThrowStatement(throw_statement) => {
                self.walk_expression(&throw_statement.argument, depth);
                self.push_op(IOp::Exit(ControlExit::Throw));
            }
            Statement::BreakStatement(_) => self.push_op(IOp::Exit(ControlExit::Break)),
            Statement::ContinueStatement(_) => self.push_op(IOp::Exit(ControlExit::Continue)),
            Statement::LabeledStatement(labeled) => {
                let saved = self.statement_label.replace((labeled.body.span(), labeled.label.name.to_string()));
                self.scan_statement(&labeled.body, depth);
                self.statement_label = saved;
            }
            Statement::TryStatement(try_statement) => {
                let try_ops = self.scan_branch_statements(&try_statement.block.body, depth);
                let catch_ops = try_statement
                    .handler
                    .as_ref()
                    .map(|handler| self.scan_branch_statements(&handler.body.body, depth));
                let finally_ops = try_statement
                    .finalizer
                    .as_ref()
                    .map(|finalizer| self.scan_branch_statements(&finalizer.body, depth));

                // The two arms are not exclusive: a throw after a write leaves
                // that write applied while the catch body also runs. That is a
                // real grammar gap only when both arms can write the same
                // buffer; a write in one arm alone is already represented by
                // the choice. `finally` is appended to both arms and therefore
                // excluded from the intersection. The bound is placed after the
                // choice so that it qualifies only subsequent emissions; an
                // emission inside the try body cannot observe a later catch.
                if let Some(catch_ops) = &catch_ops {
                    let mut try_buffers = BTreeSet::new();
                    collect_op_buffers(&try_ops, &mut try_buffers);
                    let mut catch_buffers = BTreeSet::new();
                    collect_op_buffers(catch_ops, &mut catch_buffers);
                    try_buffers.retain(|buffer| catch_buffers.contains(buffer));
                    let combined = try_buffers;
                    // `finally` runs after whichever of `try`/`catch` executed, so it
                    // is appended to both alternatives, never offered as a third one.
                    let append_finally = |mut ops: Vec<IOp>| -> Vec<IOp> {
                        if let Some(finally) = &finally_ops {
                            ops.extend(finally.iter().cloned());
                        }
                        ops
                    };
                    let arms = vec![append_finally(try_ops), append_finally(catch_ops.clone())];
                    let site = ChoiceSite {
                        buffer_aliases: Vec::new(),
                        kind: ChoiceKind::TryCatch,
                        span: try_statement.span,
                        condition: None,
                        labels: vec!["try".into(), "catch".into()],
                    };
                    self.push_op(IOp::Choice(site, arms.into_iter().map(IOp::Seq).collect()));
                    if !combined.is_empty() {
                        self.push_op(IOp::Unsupported {
                            span: try_statement.span,
                            text: self.slice(try_statement.span),
                            reason: "try_catch_control_flow_unresolved".into(),
                            buffers: combined,
                        });
                    }
                } else {
                    // No handler: `try` then `finally` is an unconditional
                    // sequence, not a choice.
                    let append_finally = |mut ops: Vec<IOp>| -> Vec<IOp> {
                        if let Some(finally) = &finally_ops {
                            ops.extend(finally.iter().cloned());
                        }
                        ops
                    };
                    for op in append_finally(try_ops) {
                        self.push_op(op);
                    }
                }
            }
            Statement::SwitchStatement(switch_statement) => {
                self.walk_expression(&switch_statement.discriminant, depth + 1);
                let mut branches = Vec::new();
                let mut labels = Vec::new();
                for case in &switch_statement.cases {
                    branches.push(self.scan_branch_statements(&case.consequent, depth));
                    labels.push(match case.test.as_ref() {
                        Some(test) => format!("case {}", self.slice(test.span())),
                        None => "default".into(),
                    });
                }
                let site = ChoiceSite {
                    buffer_aliases: Vec::new(),
                    kind: ChoiceKind::Switch,
                    span: switch_statement.span,
                    condition: Some(self.slice(switch_statement.discriminant.span())),
                    labels,
                };
                // A case body that writes the buffer may also fall through into
                // a later case, and a switch without a default has an unmatched
                // empty route. Both make the case-shaped choice a bounded
                // approximation, so the affected buffers are marked explicitly.
                let mut written = BTreeSet::new();
                for branch in &branches {
                    collect_op_buffers(branch, &mut written);
                }
                self.push_op(IOp::Choice(site, branches.into_iter().map(IOp::Seq).collect()));
                if !written.is_empty() {
                    self.push_op(IOp::Unsupported {
                        span: switch_statement.span,
                        text: self.slice(switch_statement.span),
                        reason: "switch_fallthrough_control_flow_unresolved".into(),
                        buffers: written,
                    });
                }
            }
            _ => {}
        }
    }

    fn scan_expression(&mut self, expression: &Expression<'a>, depth: usize) -> Vec<IOp> {
        let saved_ops = std::mem::take(&mut self.ops);
        let saved_path = self.path.clone();
        self.walk_expression(expression, depth);
        let ops = std::mem::replace(&mut self.ops, saved_ops);
        self.path = saved_path;
        ops
    }

    fn walk_expression(&mut self, expression: &Expression<'a>, depth: usize) {
        if self.node_budget == 0 {
            self.depth_truncated = true;
            return;
        }
        self.node_budget -= 1;
        if depth > self.limits.max_wire_depth {
            self.depth_truncated = true;
            return;
        }
        match expression {
            Expression::CallExpression(call) => self.walk_call(call, depth),
            Expression::AssignmentExpression(assignment) => self.walk_assignment(assignment, depth),
            Expression::SequenceExpression(sequence) => {
                for expression in &sequence.expressions {
                    self.walk_expression(expression, depth);
                }
            }
            Expression::ConditionalExpression(conditional) => {
                self.walk_expression(&conditional.test, depth);
                let consequent = self.scan_expression(&conditional.consequent, depth + 1);
                let alternate = self.scan_expression(&conditional.alternate, depth + 1);
                let site = ChoiceSite {
                    buffer_aliases: Vec::new(),
                    kind: ChoiceKind::Conditional,
                    span: conditional.span,
                    condition: Some(self.slice(conditional.test.span())),
                    labels: vec!["then".into(), "else".into()],
                };
                self.push_op(IOp::Choice(site, vec![IOp::Seq(consequent), IOp::Seq(alternate)]));
            }
            Expression::LogicalExpression(logical) => {
                // `&&`, `||` and `??` always evaluate the left operand; the right
                // operand runs only when the left does not decide. It is a
                // conditional path, never an unconditional sequence.
                self.walk_expression(&logical.left, depth);
                let right = self.scan_expression(&logical.right, depth + 1);
                let site = ChoiceSite {
                    buffer_aliases: Vec::new(),
                    kind: ChoiceKind::Logical,
                    span: logical.span,
                    condition: Some(format!(
                        "{} {}",
                        self.slice(logical.left.span()),
                        logical.operator.as_str()
                    )),
                    labels: vec!["right".into(), "skip".into()],
                };
                self.push_op(IOp::Choice(site, vec![IOp::Seq(right), IOp::Seq(Vec::new())]));
            }
            Expression::NewExpression(new) => {
                // `new C(args)`: the callee, then the arguments. Callbacks
                // passed to constructors (`new MutationObserver(() => …)`)
                // are scanned as nested function bodies like any callback.
                self.walk_expression(&new.callee, depth);
                for argument in &new.arguments {
                    if let Some(expression) = argument.as_expression() {
                        self.walk_expression(expression, depth + 1);
                    }
                }
            }
            Expression::AwaitExpression(await_expression) => {
                self.walk_expression(&await_expression.argument, depth);
            }
            Expression::UpdateExpression(_) | Expression::Identifier(_) => {}
            Expression::BinaryExpression(binary) => {
                self.walk_expression(&binary.left, depth);
                self.walk_expression(&binary.right, depth);
            }
            Expression::UnaryExpression(unary) => self.walk_expression(&unary.argument, depth),
            Expression::ParenthesizedExpression(paren) => {
                self.walk_expression(&paren.expression, depth);
            }
            Expression::ArrayExpression(array) => {
                for element in &array.elements {
                    if let Some(expression) = element.as_expression() {
                        self.walk_expression(expression, depth);
                    }
                }
            }
            Expression::ObjectExpression(object) => {
                for property in &object.properties {
                    if let ObjectPropertyKind::ObjectProperty(property) = property {
                        self.walk_expression(&property.value, depth);
                    }
                }
            }
            Expression::FunctionExpression(function) => {
                if let Some(body) = function.body.as_ref() {
                    self.scan_nested_function(function.span.start, &body.statements, depth, true);
                }
            }
            Expression::ArrowFunctionExpression(arrow) => {
                // An arrow keeps the enclosing function's `arguments`, so it is
                // not a new `arguments` owner.
                self.scan_nested_function(arrow.span.start, &arrow.body.statements, depth, false);
            }
            Expression::StaticMemberExpression(member) => {
                // A member callee's object evaluates before its property
                // arguments. Traversing it keeps nested writes/emissions in
                // source evaluation order instead of silently skipping them.
                self.walk_expression(&member.object, depth);
            }
            Expression::ComputedMemberExpression(member) => {
                self.walk_expression(&member.object, depth);
                self.walk_expression(&member.expression, depth);
            }
            Expression::TemplateLiteral(template) => {
                for expression in &template.expressions {
                    self.walk_expression(expression, depth);
                }
            }
            _ => {}
        }
    }

    fn walk_call(&mut self, call: &CallExpression<'a>, depth: usize) {
        if let Some((target, payload)) = self.extracted_next_call(call) {
            let payload = self.resolve_payload(payload);
            self.push_emission(call, target, payload);
        }
        if self.site == EmissionSite::WorkerForward && self.emission_is_forwarded(call) {
            return;
        }
        if let Some(emission_call) = next_emission(call) {
            let (receiver, payload) = match emission_call {
                EmissionCall::Static { receiver, payload } => {
                    (receiver, self.resolve_payload(payload))
                }
                EmissionCall::Dynamic { receiver, array } => (
                    receiver,
                    PayloadResolution::Unresolved(format!(
                        "dynamic apply argument array: {}",
                        self.slice(array.span())
                    )),
                ),
            };
            if let Some(target) = self.observer_target(receiver) {
                self.push_emission(call, target, payload);
            }
        }
        if self.site == EmissionSite::WorkerReply {
            if let Some(buffer) = self.worker_reply_buffer(call) {
                self.push_emission(
                    call,
                    ObsTarget::ArgumentsList,
                    PayloadResolution::Buffer(buffer),
                );
            }
        }
        // The three-argument call shape with the emitted buffer in second
        // position is stronger evidence than the program-wide alias table: a
        // binding reused as the STRING closure here and the NUMBER writer later
        // (`w = makeString(mask); w("", buf, 3); …; w = writeNumber`) otherwise
        // resolves to its last assignment and the string write is typed as a
        // number write, losing the buffer argument entirely.
        // `writer.call(void 0, value, buffer, position)` (a tracer spelling):
        // the same write without its receiver.
        if let Some((writer, kind)) = self.receiverless_writer_call(call) {
            for argument in &call.arguments {
                if let Some(expression) = argument.as_expression() {
                    self.walk_expression(expression, depth + 1);
                }
            }
            self.record_write_parts(kind, writer, &call.arguments[1..], call.span);
            return;
        }
        let kind = self
            .chained_string_write(call)
            .or_else(|| self.write_kind(&call.callee));
        if let Some(kind) = kind {
            // Arguments evaluate before the writer call: replay them first, then
            // append this write, so nested writer chains record inner-first.
            for argument in &call.arguments {
                if let Some(expression) = argument.as_expression() {
                    self.walk_expression(expression, depth + 1);
                }
            }
            self.record_write(kind, call);
            return;
        }
        self.walk_expression(&call.callee, depth);
        for argument in &call.arguments {
            if let Some(expression) = argument.as_expression() {
                self.walk_expression(expression, depth + 1);
            }
        }
        // An inline helper storing one raw-prefix byte of a seed.
        if self.record_inline_prefix_store(call) {
            return;
        }
        // `((a, b, c, d) => { a[b] = c; return d; })(buf, i, v ^ k(), r)`:
        // the byte store `buf[i] = v ^ k()`, after the arguments evaluated.
        if self.record_inline_byte_store(call) {
            return;
        }
        // A program helper that receives this buffer *and* a writer writes it
        // through that writer; replay its writer calls here, at the call site,
        // after the call's own arguments evaluated. Any unproven part leaves the
        // buffer to `record_opaque_arguments` instead of dropping writes.
        if self.expand_helper_call(call) {
            return;
        }
        self.record_opaque_arguments(call);
    }

    /// An inline store helper storing its own formal (`a[b] = c`) into a
    /// payload buffer at a payload index (not a raw-prefix byte): one byte
    /// store of the caller's argument. Returns whether the call was taken.
    fn record_inline_byte_store(&mut self, call: &CallExpression<'a>) -> bool {
        let Some(store) = inline_store_helper(&call.callee, self.scoping) else {
            return false;
        };
        if call.arguments.len() != store.formals.len() {
            return false;
        }
        let argument = |position: usize| call.arguments.get(position).and_then(Argument::as_expression);
        let formal = |expression: &Expression<'a>| expression_parameter_index(expression, &store.formals, self.scoping);
        // `a[b] = c` stores an argument; `a[b] = c ^ d` an argument masked by
        // another (`(…)(buf, 11, bits, ks(), r)`).
        let (value, mask) = match store.value.without_parentheses() {
            Expression::BinaryExpression(binary) if binary.operator == BinaryOperator::BitwiseXOR => {
                match (formal(&binary.left), formal(&binary.right)) {
                    (Some(left), Some(right)) => (left, Some(right)),
                    _ => return false,
                }
            }
            other => match formal(other) {
                Some(value) => (value, None),
                None => return false,
            },
        };
        let (Some(array), Some(index), Some(value)) = (argument(store.written), argument(store.index), argument(value)) else {
            return false;
        };
        let Some(buffer) = self.buffer_symbol(array) else {
            return false;
        };
        let index_text = self.slice(index.span());
        let index_number = parse_index(&index_text).filter(|value| (0..=3).contains(value));
        if index_number.is_some() && self.seed_symbol_in(value).is_some() {
            return false;
        }
        let Some(mask) = mask else {
            self.record_byte_store(
                buffer,
                index_text,
                index_number,
                value,
                call.span,
                Some(index.span()),
            );
            return true;
        };
        let Some(other) = argument(mask) else {
            return false;
        };
        // One operand is a draw from a known stream; the other is the value.
        let draw = |expression: &Expression<'a>| -> Option<MaskInfo> {
            let Expression::CallExpression(draw) = expression.without_parentheses() else { return None };
            if !draw.arguments.is_empty() {
                return None;
            }
            expression_symbol(&draw.callee, self.scoping)
                .and_then(|stream| self.masks.get(&stream).cloned())
                .or_else(|| self.mask_of(&draw.callee))
        };
        let (value, drawn, mask_span) = match (draw(other), draw(value)) {
            (Some(drawn), _) => (value, drawn, other.span()),
            (None, Some(drawn)) => (other, drawn, value.span()),
            (None, None) => return false,
        };
        let xor_literal = numeric_value(value);
        let mask_source = Some(drawn.argument.clone()).filter(|argument| !argument.is_empty()).unwrap_or_else(|| self.slice(mask_span));
        self.push_op(IOp::Leaf(ILeaf {
            index_span: Some(index.span()),
            writer_factory: None,
            string_instance: None,
            kind: LeafKind::Byte,
            span: call.span,
            buffer: Some(buffer),
            mask: Some(drawn),
            masked: xor_literal.is_some(),
            value: Some(format!("{} ^ {}", self.slice(value.span()), self.slice(mask_span))),
            index: Some(index_text),
            xor_literal,
            xor_source: xor_literal.map(|_| self.slice(value.span())),
            xor_span: if xor_literal.is_some() { value.span() } else { Span::default() },
            byte_width: None,
            logical_value: self.slice(value.span()),
            logical_span: value.span(),
            resolved_value: self.resolve_value(value, 0),
            mask_form: MaskForm::Stream,
            mask_source: Some(mask_source),
            prefix_element: None,
            helper_call: None,
            helper_arguments: Vec::new(),
        }));
        true
    }

    /// `w.call(void 0, …)` / `t[0].call(undefined, …)` of a known writer `w`.
    fn receiverless_writer_call<'c>(&self, call: &'c CallExpression<'a>) -> Option<(&'c Expression<'a>, LeafKind)> {
        if member_property(&call.callee).as_deref() != Some("call") {
            return None;
        }
        let receiver = call.arguments.first()?.as_expression()?.without_parentheses();
        let receiverless = match receiver {
            Expression::UnaryExpression(unary) => {
                unary.operator == UnaryOperator::Void && matches!(unary.argument, Expression::NumericLiteral(_))
            }
            Expression::Identifier(identifier) => identifier.name == "undefined" && resolve_reference(identifier, self.scoping).is_none(),
            Expression::NullLiteral(_) => true,
            _ => false,
        };
        if !receiverless {
            return None;
        }
        let Expression::StaticMemberExpression(member) = call.callee.without_parentheses() else {
            return None;
        };
        let writer = &member.object;
        Some((writer, self.write_kind(writer)?))
    }

    fn push_emission(&mut self, call: &CallExpression<'a>, target: ObsTarget, payload: PayloadResolution) {
        if self.emissions.len() >= self.limits.max_emissions {
            self.emission_truncated = true;
        } else {
            let buffer_version = match &payload {
                PayloadResolution::Buffer(buffer) => Some(self.version_of(*buffer)),
                _ => None,
            };
            let prefix = match &payload {
                PayloadResolution::Buffer(buffer) => self.prefixes.get(buffer).cloned(),
                _ => None,
            };
            self.emissions.push(RawEmission {
                origin: call.span,
                target,
                payload,
                scope: self.scope_id,
                path: self.path.clone(),
                buffer_version,
                prefix,
            });
        }
    }

    /// `v ^ factory(seed ^ C)()`: a draw from a stream created inline.
    fn inline_stream_draw(&self, expression: &Expression<'a>) -> Option<MaskInfo> {
        let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
            return None;
        };
        if binary.operator != BinaryOperator::BitwiseXOR {
            return None;
        }
        [&binary.left, &binary.right].into_iter().find_map(|operand| {
            let Expression::CallExpression(draw) = operand.without_parentheses() else {
                return None;
            };
            let callee = receiverless_draw_callee(draw)?;
            // `t = [i, v, mask]; buf[t[0]] = t[1] ^ t[2]()`: the tracer's
            // argument tuple holds the mask binding.
            self.mask_of(callee).or_else(|| {
                let (symbol, _) = self.tuple_element(callee)?;
                self.masks.get(&(*symbol)?).cloned()
            })
        })
    }

    /// `t[k] ^ v` (either order) where `t[k]` holds a draw from a stream:
    /// that stream and the span of `v`.
    fn predrawn_tuple_mask(&self, expression: &Expression<'a>) -> Option<(MaskInfo, Span)> {
        let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
            return None;
        };
        if binary.operator != BinaryOperator::BitwiseXOR {
            return None;
        }
        let drawn = |side: &Expression<'a>| -> Option<MaskInfo> {
            let Expression::ComputedMemberExpression(member) = side.without_parentheses() else {
                return None;
            };
            let tuple = expression_symbol(&member.object, self.scoping)?;
            let index = numeric_value(&member.expression)?;
            if index < 0.0 || index.fract() != 0.0 || !self.tuples.contains_key(&tuple) {
                return None;
            }
            self.tuple_draws.get(&tuple)?.get(index as usize)?.clone()
        };
        drawn(&binary.left)
            .map(|mask| (mask, binary.right.span()))
            .or_else(|| drawn(&binary.right).map(|mask| (mask, binary.left.span())))
    }

    /// `m ^ v` (either order) where the local `m` holds the latest stream
    /// draw (`let m = ks();` with no write since): that stream and `v`.
    fn predrawn_register_mask(&self, expression: &Expression<'a>) -> Option<(MaskInfo, Span)> {
        let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
            return None;
        };
        if binary.operator != BinaryOperator::BitwiseXOR {
            return None;
        }
        let drawn = |side: &Expression<'a>| -> Option<MaskInfo> {
            let Expression::Identifier(_) = side.without_parentheses() else {
                return None;
            };
            let symbol = expression_symbol(side, self.scoping)?;
            let mask = self.register_draws.get(&symbol)?;
            let draw = mask.predrawn?;
            let statement = |span: Span| {
                let mut id = self.nodes.iter().find(|node| node.span() == span)?.id();
                loop {
                    if matches!(
                        self.nodes.kind(id),
                        AstKind::ExpressionStatement(_) | AstKind::VariableDeclaration(_)
                    ) {
                        return Some(id);
                    }
                    let parent = self.nodes.parent_id(id);
                    if parent == id {
                        return None;
                    }
                    id = parent;
                }
            };
            let from = statement(draw)?;
            // A conditional subexpression is not an unconditional retained draw,
            // even when its enclosing statement immediately precedes the store.
            let direct = match self.nodes.kind(from) {
                AstKind::ExpressionStatement(statement) => matches!(
                    statement.expression.without_parentheses(),
                    Expression::AssignmentExpression(assignment)
                        if assignment.operator == AssignmentOperator::Assign
                            && identifier_target(&assignment.left, self.scoping) == Some(symbol)
                            && assignment.right.without_parentheses().span() == draw
                ),
                AstKind::VariableDeclaration(declaration) => {
                    declaration.declarations.len() == 1
                        && declaration.declarations[0]
                            .init
                            .as_ref()
                            .is_some_and(|init| init.without_parentheses().span() == draw)
                }
                _ => false,
            };
            if !direct {
                return None;
            }
            // A callback can replace the scalar while evaluating the store's
            // value. Refuse captured writes rather than assuming a local draw
            // survives a call into another function.
            let function = |mut id| loop {
                if matches!(
                    self.nodes.kind(id),
                    AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
                ) {
                    return Some(id);
                }
                let parent = self.nodes.parent_id(id);
                if parent == id {
                    return None;
                }
                id = parent;
            };
            let owner = function(from)?;
            if self
                .scoping
                .get_resolved_references(symbol)
                .any(|reference| {
                    reference.is_write() && function(reference.node_id()) != Some(owner)
                })
            {
                return None;
            }
            let to = statement(expression.span())?;
            let parent = self.nodes.parent_id(from);
            if self.nodes.parent_id(to) != parent {
                return None;
            }
            let statements = match self.nodes.kind(parent) {
                AstKind::BlockStatement(block) => block.body.as_slice(),
                AstKind::FunctionBody(body) => body.statements.as_slice(),
                _ => return None,
            };
            let index = statements
                .iter()
                .position(|s| s.span() == self.nodes.get_node(from).span())?;
            let to_span = self.nodes.get_node(to).span();
            let end = index
                + 1
                + statements[index + 1..]
                    .iter()
                    .position(|s| s.span() == to_span)?;
            // Statements between the draw and the store may only rebind other
            // locals from locals, literals and operators (`m = ks(); f = f | 0;
            // buf[i] = m ^ 144`): no call, getter, setter or member write can
            // draw from the stream, write the buffer, or replace `m` there.
            if !statements[index + 1..end]
                .iter()
                .all(|between| inert_statement(between, symbol, self.scoping))
            {
                return None;
            }
            Some(mask.clone())
        };
        drawn(&binary.left)
            .map(|mask| (mask, binary.right.span()))
            .or_else(|| drawn(&binary.right).map(|mask| (mask, binary.left.span())))
    }

    /// The tuple element `t[k]` names, when `t` holds a tuple literal.
    fn tuple_element(&self, expression: &Expression<'a>) -> Option<&(Option<SymbolId>, String)> {
        let Expression::ComputedMemberExpression(member) = expression.without_parentheses() else {
            return None;
        };
        let tuple = expression_symbol(&member.object, self.scoping)?;
        let index = numeric_value(&member.expression)?;
        if index < 0.0 || index.fract() != 0.0 {
            return None;
        }
        self.tuples.get(&tuple)?.get(index as usize)?.as_ref()
    }

    /// The element expression `t[k]` reads, when `t` holds a tuple literal.
    fn tuple_value(&self, expression: &Expression<'a>) -> Option<Span> {
        let Expression::ComputedMemberExpression(member) = expression.without_parentheses() else {
            return None;
        };
        let tuple = expression_symbol(&member.object, self.scoping)?;
        let index = numeric_value(&member.expression)?;
        if index < 0.0 || index.fract() != 0.0 {
            return None;
        }
        *self.tuple_values.get(&tuple)?.get(index as usize)?
    }

    fn tuple_store(&self, object: &Expression<'a>, index: &Expression<'a>) -> Option<(SymbolId, String)> {
        let (buffer, _) = self.tuple_element(object)?;
        let index = match self.tuple_element(index) {
            Some((_, text)) => text.clone(),
            None => self.slice(index.span()),
        };
        Some(((*buffer)?, index))
    }

    /// `observer.next` read into a binding (`m = observer.next`,
    /// `m = observer["next"]`).
    fn next_method_target(&self, value: &Expression<'a>) -> Option<ObsTarget> {
        let value = value.without_parentheses();
        if member_property(value).as_deref() != Some("next") {
            return None;
        }
        self.observer_target(member_object(value)?)
    }

    /// `m.call(receiver, payload)` where `m` holds an observer's `next` and
    /// `receiver` names the same observer.
    fn extracted_next_call(
        &self,
        call: &'a CallExpression<'a>,
    ) -> Option<(ObsTarget, &'a Expression<'a>)> {
        if member_property(&call.callee).as_deref() != Some("call") {
            return None;
        }
        let method = expression_symbol(member_object(&call.callee)?, self.scoping)?;
        let target = *self.next_methods.get(&method)?;
        let receiver = call.arguments.first()?.as_expression()?;
        if self.observer_target(receiver) != Some(target) {
            return None;
        }
        Some((target, call.arguments.get(1)?.as_expression()?))
    }

    /// A plain binding passed to a call that is neither a writer nor a
    /// verified store helper may be written by the callee. The op is kept per
    /// argument binding and only surfaces in a grammar when that binding is
    /// the emitted buffer.
    fn record_opaque_arguments(&mut self, call: &CallExpression<'a>) {
        if next_emission(call).is_some() {
            return;
        }
        // A standard pure builtin reads its arguments; passing the emitted
        // buffer to `Number(buf)` is not an opaque write and must not shadow
        // the buffer's real wire (this is how a try/catch that only converts a
        // loop counter in both arms used to look like a write to the buffer).
        if is_pure_builtin_call(&call.callee, self.scoping) {
            return;
        }
        if member_property(&call.callee).as_deref() == Some("call")
            && member_object(&call.callee)
                .and_then(|method| expression_symbol(method, self.scoping))
                .is_some_and(|method| self.next_methods.contains_key(&method))
        {
            return;
        }
        let callee_symbol = expression_symbol(&call.callee, self.scoping);
        // An inline store helper (a tracer spelling) carries its own proof.
        let inline_proof = match call.callee.without_parentheses() {
            Expression::ArrowFunctionExpression(arrow) => returned_arrow_parameter_proof(arrow, self.scoping),
            _ => None,
        };
        let store_proof = inline_proof
            .as_ref()
            .or_else(|| callee_symbol.and_then(|symbol| self.returned_parameters.get(&symbol)));
        for (position, argument) in call.arguments.iter().enumerate() {
            if store_proof.is_some_and(|proof| proof.returned == position) {
                continue;
            }
            let Some(symbol) = argument
                .as_expression()
                .and_then(|expression| expression_symbol(expression, self.scoping))
            else {
                continue;
            };
            let version = self.version_of(symbol);
            self.push_op(IOp::Opaque {
                span: call.span,
                callee: self.slice(call.callee.span()),
                buffer: symbol,
                version,
            });
        }
    }

    /// Replays a program helper's proven writer calls at this call site.
    ///
    /// A helper is expanded only when its callee resolves to a proven
    /// [`WriterHelperProof`], every writer it reaches is a writer the caller
    /// passed, and each write targets a caller binding. Anything else keeps the
    /// existing [`IOp::Opaque`] accounting, so a possible write is never dropped
    /// without a visible reason.
    fn expand_helper_call(&mut self, call: &CallExpression<'a>) -> bool {
        if self.helper_depth >= MAX_HELPER_DEPTH {
            return false;
        }
        // An inline helper (`((w, k, o, f, p, b) => w(k, o[f], b, p))(…)`, a
        // tracer spelling of a folded write) carries its own proof: the same
        // straight-line formal-callee body a named helper must have.
        let inline = match call.callee.without_parentheses() {
            Expression::ArrowFunctionExpression(arrow) => {
                writer_helper_proof(self.scoping, self.source, &arrow.params, &arrow.body.statements)
            }
            Expression::FunctionExpression(function) => function
                .body
                .as_ref()
                .and_then(|body| writer_helper_proof(self.scoping, self.source, &function.params, &body.statements)),
            _ => None,
        };
        let proof = match inline {
            Some(proof) => proof,
            None => {
                let Some(symbol) = expression_symbol(&call.callee, self.scoping) else {
                    return false;
                };
                let Some(proof) = self.writer_helpers.get(&symbol).cloned() else {
                    return false;
                };
                proof
            }
        };
        self.helper_depth += 1;
        let leaves = self.helper_leaves(call, &proof);
        self.helper_depth -= 1;
        let Some(leaves) = leaves else {
            return false;
        };
        for leaf in leaves {
            self.push_op(IOp::Leaf(leaf));
        }
        true
    }

    /// The leaves one proven helper call writes, in the helper body's order.
    /// Every writer and buffer must be a caller value; a helper expression used
    /// as the value keeps its body span so provenance can follow its formals
    /// back to the caller's arguments.
    fn helper_leaves(
        &self,
        call: &CallExpression<'a>,
        proof: &WriterHelperProof,
    ) -> Option<Vec<ILeaf>> {
        let helper_arguments: Vec<Span> = call
            .arguments
            .iter()
            .map(|argument| argument.as_expression().map(GetSpan::span).unwrap_or_default())
            .collect();
        let mut leaves = Vec::with_capacity(proof.writes.len());
        for write in &proof.writes {
            let writer = call.arguments.get(write.writer)?.as_expression()?;
            let kind = self.write_kind(writer)?;
            let (value_argument, buffer_argument, index_argument, mask_argument) = match kind {
                LeafKind::String => (
                    write.arguments.first()?,
                    write.arguments.get(1)?,
                    write.arguments.get(2),
                    None,
                ),
                _ => (
                    write.arguments.get(1)?,
                    write.arguments.get(2)?,
                    write.arguments.get(3),
                    write.arguments.first(),
                ),
            };
            let buffer = self.helper_buffer(buffer_argument, call)?;
            let (logical_span, logical_value) = self.helper_argument(value_argument, call)?;
            let index = index_argument
                .and_then(|argument| self.helper_argument(argument, call))
                .map(|(_, text)| text);
            let (mask, mask_source) = match kind {
                LeafKind::String => self.helper_string_mask(writer),
                _ => match mask_argument {
                    Some(argument) => self.helper_number_mask(argument, call),
                    None => (None, None),
                },
            };
            let xor_literal = mask.as_ref().and_then(|mask| mask.xor_literal);
            let xor_source = mask.as_ref().and_then(|mask| mask.xor_source.clone());
            let xor_span = mask.as_ref().map(|mask| mask.xor_span).unwrap_or_default();
            let mask_form = match &mask {
                Some(_) => MaskForm::Stream,
                None => MaskForm::Unresolved,
            };
            leaves.push(ILeaf {
                index_span: None,
                writer_factory: self.writer_factory(writer, kind),
                string_instance: if kind == LeafKind::String {
                    self.string_instance(writer)
                } else {
                    None
                },
                kind,
                span: write.span,
                buffer: Some(buffer),
                mask,
                masked: true,
                value: Some(logical_value.clone()),
                index,
                xor_literal,
                xor_source,
                xor_span,
                byte_width: None,
                logical_value,
                logical_span,
                resolved_value: None,
                mask_form,
                mask_source,
                prefix_element: None,
                helper_call: Some(call.span),
                helper_arguments: helper_arguments.clone(),
            });
        }
        Some(leaves)
    }

    /// The caller expression bound to a helper formal.
    fn helper_caller_expression<'e>(
        &self,
        argument: &HelperArgument,
        call: &'e CallExpression<'a>,
    ) -> Option<&'e Expression<'a>> {
        match argument {
            HelperArgument::Formal(position) => call.arguments.get(*position)?.as_expression(),
            _ => None,
        }
    }

    /// The caller value of a helper argument: a formal maps to the caller's
    /// expression, a helper-body expression keeps its own span and text.
    fn helper_argument(
        &self,
        argument: &HelperArgument,
        call: &CallExpression<'a>,
    ) -> Option<(Span, String)> {
        match argument {
            HelperArgument::Formal(_) => {
                let expression = self.helper_caller_expression(argument, call)?;
                let span = expression.span();
                Some((span, self.slice(span)))
            }
            HelperArgument::Expression { span, text } => Some((*span, text.clone())),
            HelperArgument::Other => None,
        }
    }

    /// The caller binding a helper writes through.
    fn helper_buffer(&self, argument: &HelperArgument, call: &CallExpression<'a>) -> Option<SymbolId> {
        expression_symbol(self.helper_caller_expression(argument, call)?, self.scoping)
    }

    /// The concrete helper binding of the caller's snapshotted writer value.
    fn writer_factory(&self, writer: &Expression<'a>, kind: LeafKind) -> Option<Span> {
        let (symbol, helpers) = if kind == LeafKind::String {
            match writer.without_parentheses() {
                Expression::CallExpression(call) => (
                    expression_symbol(&call.callee, self.scoping)?,
                    &self.helpers.string_writer,
                ),
                _ => {
                    return self
                        .writer_symbol(writer)
                        .and_then(|s| self.string_factories.get(&s).copied())
                }
            }
        } else {
            (self.writer_symbol(writer)?, &self.helpers.number_writer)
        };
        helpers
            .iter()
            .find(|h| self.resolves_to_any_helper(symbol, std::slice::from_ref(h)))
            .map(|s| self.scoping.symbol_span(*s))
    }

    fn string_instance(&self, writer: &Expression<'a>) -> Option<Span> {
        match writer.without_parentheses() {
            Expression::CallExpression(call) => Some(call.span),
            _ => self
                .writer_symbol(writer)
                .and_then(|s| self.string_instances.get(&s).copied()),
        }
    }

    /// A STRING writer's captured mask, read from the caller's writer value.
    fn helper_string_mask(
        &self,
        writer: &Expression<'a>,
    ) -> (Option<MaskInfo>, Option<String>) {
        let mask = match writer.without_parentheses() {
            Expression::CallExpression(inner) => inner
                .arguments
                .first()
                .and_then(Argument::as_expression)
                .and_then(|expression| self.mask_info_for(expression)),
            _ => expression_symbol(writer, self.scoping)
                .and_then(|symbol| self.string_writers.get(&symbol).cloned().flatten()),
        };
        match &mask {
            Some(mask) => (Some(mask.clone()), Some(mask.argument.clone())),
            None => (None, Some(self.slice(writer.span()))),
        }
    }

    /// A NUMBER writer's mask, read from the caller argument the helper passes
    /// as the writer's mask.
    fn helper_number_mask(
        &self,
        argument: &HelperArgument,
        call: &CallExpression<'a>,
    ) -> (Option<MaskInfo>, Option<String>) {
        let mask = self
            .helper_caller_expression(argument, call)
            .and_then(|expression| self.mask_info_for(expression));
        match &mask {
            Some(mask) => (Some(mask.clone()), Some(mask.argument.clone())),
            None => (None, self.helper_argument(argument, call).map(|(_, text)| text)),
        }
    }

    fn record_write(&mut self, kind: LeafKind, call: &CallExpression<'a>) {
        self.record_write_parts(kind, &call.callee, &call.arguments, call.span);
    }

    /// [`Self::record_write`] of a writer `callee` invoked with `arguments`
    /// (also `writer.call(void 0, …)`, whose receiver is dropped).
    fn record_write_parts(
        &mut self,
        kind: LeafKind,
        callee: &Expression<'a>,
        arguments: &[Argument<'a>],
        call_span: Span,
    ) {
        let (mask, value_expr, buffer_expr, index_expr) = match kind {
            LeafKind::String => {
                // `factory(mask)("value", buffer, offset)` reads the mask from the
                // factory call; `writer("value", buffer, offset)` where `writer`
                // is a binding that already holds the returned STRING-writer
                // closure reads the mask captured when that closure was created.
                let mask = match callee.without_parentheses() {
                    Expression::CallExpression(inner) => inner
                        .arguments
                        .first()
                        .and_then(Argument::as_expression)
                        .and_then(|expression| self.mask_info_for(expression)),
                    _ => self.writer_symbol(callee)
                        .and_then(|symbol| self.string_writers.get(&symbol).cloned().flatten()),
                };
                (
                    mask,
                    arguments.first().and_then(Argument::as_expression),
                    arguments.get(1).and_then(Argument::as_expression),
                    arguments.get(2).and_then(Argument::as_expression),
                )
            }
            _ => {
                let mask_expr = arguments.first().and_then(Argument::as_expression);
                (
                    mask_expr.and_then(|expression| self.mask_info_for(expression)),
                    arguments.get(1).and_then(Argument::as_expression),
                    arguments.get(2).and_then(Argument::as_expression),
                    arguments.get(3).and_then(Argument::as_expression),
                )
            }
        };
        let xor_literal = mask.as_ref().and_then(|mask| mask.xor_literal);
        let xor_source = mask.as_ref().and_then(|mask| mask.xor_source.clone());
        let xor_span = mask.as_ref().map(|mask| mask.xor_span).unwrap_or_default();
        let buffer = buffer_expr.and_then(|expression| self.buffer_symbol(expression));
        if let Some(buffer) = buffer {
            self.known_buffers.insert(buffer);
        }
        // A writer always masks with the stream it is handed; an argument that
        // is not a resolved stream keeps the write but marks the mask unresolved.
        let (mask_form, mask_source) = match &mask {
            Some(mask) => (MaskForm::Stream, Some(mask.argument.clone())),
            None => (
                MaskForm::Unresolved,
                match kind {
                    LeafKind::String => Some(self.slice(callee.span())),
                    _ => arguments
                        .first()
                        .and_then(Argument::as_expression)
                        .map(|expression| self.slice(expression.span())),
                },
            ),
        };
        // `t = [v, buf]; w(t[0], t[1], i)` writes the tuple's element `v`.
        let tuple_value = value_expr.and_then(|expression| self.tuple_value(expression));
        let logical_span = tuple_value.or(value_expr.map(GetSpan::span)).unwrap_or(call_span);
        let leaf = ILeaf {
            index_span: None,
            writer_factory: self.writer_factory(callee, kind),
            string_instance: if kind == LeafKind::String {
                self.string_instance(callee)
            } else {
                None
            },
            kind,
            span: call_span,
            buffer,
            mask,
            masked: true,
            value: tuple_value
                .map(|span| self.slice(span))
                .or_else(|| value_expr.map(|expression| self.slice(expression.span()))),
            index: index_expr.map(|expression| self.slice(expression.span())),
            xor_literal,
            xor_source,
            xor_span,
            byte_width: None,
            logical_value: self.slice(logical_span),
            logical_span,
            resolved_value: value_expr.and_then(|expression| self.resolve_value(expression, 0)),
            mask_form,
            mask_source,
            prefix_element: None,
            helper_call: None,
            helper_arguments: Vec::new(),
        };
        if let Some(Expression::ConditionalExpression(conditional)) = buffer_expr {
            if let (Some(yes), Some(no)) = (self.buffer_symbol(&conditional.consequent),
                self.buffer_symbol(&conditional.alternate)) {
                let mut yes_leaf = leaf.clone(); yes_leaf.buffer = Some(yes);
                let mut no_leaf = leaf; no_leaf.buffer = Some(no);
                self.known_buffers.extend([yes, no]);
                self.push_op(IOp::Choice(ChoiceSite {
                    buffer_aliases: Vec::new(), kind: ChoiceKind::Conditional,
                    span: conditional.span, condition: Some(self.slice(conditional.test.span())),
                    labels: vec!["then".into(), "else".into()],
                }, vec![IOp::Leaf(yes_leaf), IOp::Leaf(no_leaf)]));
                return;
            }
        }
        if buffer.is_none() {
            self.unresolved.push(Unresolved {
                span: call_span,
                reason: "writer_buffer_unresolved".into(),
                detail: format!(
                    "{:?} write has no resolvable buffer binding",
                    match kind {
                        LeafKind::Number => "NUMBER",
                        LeafKind::String => "STRING",
                        LeafKind::Byte => "BYTE",
                    }
                ),
            });
            self.push_op(IOp::Unknown {
                span: call_span,
                text: self.slice(call_span),
                reason: "writer_buffer_unresolved".into(),
                buffer: None,
            });
        } else {
            self.push_op(IOp::Leaf(leaf));
        }
    }

    /// Updates or defines one field of a raw named object (`obj.a = value` or
    /// `obj["a"] = value`); the field's type comes from the assigned
    /// expression.
    fn record_object_field(&mut self, owner: SymbolId, field: String, value: &Expression<'a>) {
        let value_text = self.slice(value.span());
        let value_span = value.span();
        // `obj.c = x` with `x` a local whose latest value is one typed scalar
        // (including an if/else join of same-typed arms).
        let kind = self.infer_kind(value).or_else(|| match self.resolve_scalar(value)? {
            ScalarTree::Leaf { kind, .. } => Some(kind),
            ScalarTree::Choice { .. } => None,
        });
        let fields = self.payload_objects.entry(owner).or_default();
        match fields.iter_mut().find(|entry| entry.key == field) {
            Some(entry) => {
                entry.value = Some(value_text);
                entry.kind = kind;
                entry.span = value_span;
            }
            None => fields.push(ObjectField {
                key: field,
                value: Some(value_text),
                kind,
                span: value_span,
            }),
        }
    }

    fn walk_assignment(&mut self, assignment: &AssignmentExpression<'a>, depth: usize) {
        // Evaluate the left-hand reference (object, then computed key) before
        // the right-hand side, matching JavaScript evaluation order.
        let mut member_buffer: Option<SymbolId> = None;
        let mut index_text: Option<String> = None;
        let mut target_symbol: Option<SymbolId> = None;
        let mut field_owner: Option<SymbolId> = None;
        let mut worker_owner: Option<SymbolId> = None;
        match &assignment.left {
            AssignmentTarget::ComputedMemberExpression(member) => {
                self.walk_expression(&member.object, depth);
                self.walk_expression(&member.expression, depth);
                member_buffer = self.buffer_symbol(&member.object);
                index_text = Some(self.slice(member.expression.span()));
                // `t[0][t[1]] = v` with `t = [buffer, index, …]`: a store into
                // the tuple's buffer at the tuple's index.
                if member_buffer.is_none() {
                    if let Some((buffer, index)) =
                        self.tuple_store(&member.object, &member.expression)
                    {
                        member_buffer = Some(buffer);
                        index_text = Some(index);
                    }
                }
            }
            AssignmentTarget::StaticMemberExpression(member) => {
                self.walk_expression(&member.object, depth);
                field_owner = self.buffer_symbol(&member.object);
                if self.site == EmissionSite::WorkerReply {
                    if let Expression::Identifier(identifier) = member.object.without_parentheses() {
                        worker_owner = resolve_reference(identifier, self.scoping);
                    }
                }
            }
            AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                target_symbol = resolve_reference(identifier, self.scoping);
            }
            _ => {}
        }
        self.walk_expression(&assignment.right, depth);

        // A reply object built property by property (`r = {a: t}; r.b =
        // [[], buffer]` or `r.b = payload`) carries the payload's buffer like
        // a reply literal with that property.
        if let Some(owner) = worker_owner {
            let buffer = match assignment.right.without_parentheses() {
                Expression::Identifier(identifier) => resolve_reference(identifier, self.scoping)
                    .and_then(|symbol| self.worker_replies.get(&symbol).copied()),
                value => self.array_payload_buffer(value),
            };
            if let Some(buffer) = buffer {
                self.worker_replies.insert(owner, buffer);
            }
        }

        // `obj.a = value` updates a raw named object like `obj["a"] = value`
        // below, but only for an object whose literal the pass has already
        // seen: a dotted write to any other binding is not a payload field.
        if let (Some(owner), Some(field)) = (field_owner, member_field_name(&assignment.left)) {
            let owner = match self.object_aliases.get(&owner) {
                Some(&source) if !self.payload_objects.contains_key(&owner) => source,
                _ => owner,
            };
            if self.payload_objects.contains_key(&owner) {
                self.record_object_field(owner, field, &assignment.right);
            }
        }

        if let (Some(buffer), Some(index)) = (member_buffer, index_text.clone()) {
            // `payload[1] = buf` records the emitted buffer for a list-built
            // payload; other indexed writes are byte writes.
            if let Some(index) = parse_index(&index) {
                self.payload_index.remove(&(buffer, index));
                if let Some(value_buffer) = self.value_symbol(&assignment.right)
                    .map(|buffer| self.resolve_buffer_alias(buffer)) {
                    self.payload_index.insert((buffer, index), value_buffer);
                }
            }
            // A raw named-object field update or definition, e.g. `obj["a"] =
            // value`. The literal may live in an enclosing scope, so a
            // string-keyed write also *creates* the field the first time it is
            // seen; the field's type comes from the assigned expression.
            if let Some(field) = member_field_name(&assignment.left) {
                self.record_object_field(buffer, field, &assignment.right);
            }
            let index_number = parse_index(&index).filter(|value| (0..=3).contains(value));
            if let (Some(buffer), Some(index_number)) = (member_buffer, index_number) {
                if !self.prefixes.contains_key(&buffer) {
                    if let Some(seed) = self.seed_symbol_in(&assignment.right) {
                        self.record_prefix_part(
                            buffer,
                            index_number as u32,
                            seed,
                            self.slice(assignment.right.span()),
                            assignment.right.span(),
                            assignment.span,
                        );
                        // These four stores are the prefix bytes, not ordered
                        // payload writes; the prefix is emitted as four BYTE
                        // leaves at the emission instead of being duplicated.
                        self.record_payload_index_assignment(buffer, &index, &assignment.right);
                        return;
                    }
                }
            }
            self.record_byte_store(
                buffer,
                index,
                index_number,
                &assignment.right,
                assignment.span,
                None,
            );
        }

        if let Some(symbol) = target_symbol {
            if assignment.operator == AssignmentOperator::Assign {
                let payload = self.array_payload_buffer(&assignment.right);
                self.record_buffer_assignment(symbol, &assignment.right, assignment.span);
                self.record_value_binding(symbol, &assignment.right, payload);
            } else {
                // `x op= value` / `x++` writes the binding without proving it
                // still names the observer; drop the route instead of keeping a
                // stale alias.
                self.bump_version(symbol);
                self.seed_aliases.remove(&symbol);
                self.aliases_out.remove(&symbol);
                self.next_methods.remove(&symbol);
                self.number_holders.remove(&symbol);
                self.tuples.remove(&symbol);
                self.tuple_values.remove(&symbol);
                self.tuple_seeds.remove(&symbol);
                self.tuple_seed_words.remove(&symbol);
                self.buffer_aliases.remove(&symbol);
            }
        }
    }

    /// One logical byte store `buffer[index] = right` (`span` is the whole
    /// store): its mask split, xor evidence and resolved value.
    fn record_byte_store(
        &mut self,
        buffer: SymbolId,
        index: String,
        index_number: Option<i64>,
        right: &Expression<'a>,
        span: Span,
        index_span: Option<Span>,
    ) {
        let (xor_value, xor_source, xor_span) = xor_evidence(&right, self.source);
        let masked = xor_value.is_some();
        let mask_symbol = byte_write_mask(&right, self.scoping);
        let mask = mask_symbol
            .and_then(|symbol| self.masks.get(&symbol).cloned())
            .or_else(|| self.inline_stream_draw(&right));
        let mut split = split_byte_mask(&right, &self.masks, self.scoping);
        // `t = [buf, i, ks()]; t[0][t[1]] = t[2] ^ 136`: the tuple holds a
        // draw already made from the stream; the other operand is the value.
        let predrawn = self.predrawn_tuple_mask(&right);
        let predrawn = predrawn.or_else(|| self.predrawn_register_mask(right));
        let mask = match predrawn {
            Some((drawn, value)) => {
                split = ByteMask::Stream {
                    value,
                    mask: right.span(),
                };
                Some(drawn)
            }
            None => mask,
        };
        let (mask_form, mask_source, logical_span) = match split {
            ByteMask::Stream {
                value,
                mask: mask_call,
            } => (
                if mask.is_some() {
                    MaskForm::Stream
                } else {
                    MaskForm::Unresolved
                },
                Some(
                    mask.as_ref()
                        .map(|mask| mask.argument.clone())
                        .filter(|argument| !argument.is_empty())
                        .unwrap_or_else(|| self.slice(mask_call)),
                ),
                value,
            ),
            ByteMask::StaticXor { value } => (MaskForm::StaticXor, None, value),
            ByteMask::None => (MaskForm::Unmasked, None, right.span()),
        };
        // A raw prefix byte stored again into a buffer that already has a
        // prefix (a reused buffer re-seeded in one arm).
        let prefix_element = index_number
            .filter(|_| mask_form == MaskForm::Unmasked)
            .filter(|_| self.seed_symbol_in(&right).is_some())
            .map(|value| value as u32);
        self.push_op(IOp::Leaf(ILeaf {
            index_span,
            writer_factory: None,
            string_instance: None,
            kind: LeafKind::Byte,
            span,
            buffer: Some(buffer),
            mask,
            masked,
            value: Some(self.slice(right.span())),
            index: Some(index),
            xor_literal: xor_value,
            xor_source,
            xor_span,
            byte_width: None,
            logical_value: self.slice(logical_span),
            logical_span,
            resolved_value: self.resolve_value(&right, 0),
            mask_form,
            mask_source,
            prefix_element,
            helper_call: None,
            helper_arguments: Vec::new(),
        }));
    }

    /// Preserve the `[aux, buffer]` index mapping for a prefix-byte store
    /// without recording it as an ordered payload write.
    /// One raw-prefix byte store `buffer[index] = <seed byte>` (`store` is the
    /// whole store); the fourth byte of one seed completes the prefix.
    fn record_prefix_part(&mut self, buffer: SymbolId, index: u32, seed: SymbolId, text: String, span: Span, store: Span) {
        let complete = {
            let parts =
                self.prefix_parts
                    .entry(buffer)
                    .or_insert_with(|| PrefixParts {
                        first_store: store.start,
                        seed: Some(seed),
                        elements: BTreeMap::new(),
                    });
            if parts.seed != Some(seed) {
                parts.seed = None;
            }
            parts.elements.insert(index, (text, span));
            parts.seed.is_some()
                && (0..=3).all(|slot| parts.elements.contains_key(&slot))
        };
        if complete {
            let (seed, elements) = {
                let parts = &self.prefix_parts[&buffer];
                (
                    parts.seed,
                    parts
                        .elements
                        .values()
                        .cloned()
                        .collect::<Vec<(String, Span)>>(),
                )
            };
            if let Some(seed) = seed {
                let seed_word = self
                    .seeds
                    .get(&seed)
                    .or_else(|| self.tuple_seed_words.get(&seed))
                    .cloned()
                    .or_else(|| self.name(seed))
                    .unwrap_or_default();
                self.push_op(IOp::Prefix {
                    buffer,
                    node: PrefixNode {
                        seed_binding: Some(self.scoping.symbol_span(seed).into()),
                        span: Span::new(
                            self.prefix_parts[&buffer].first_store,
                            store.end,
                        ),
                        spelling: PrefixSpelling::IndexStores,
                        seed_word: seed_word.clone(),
                        elements: elements.iter().map(|(text, _)| text.clone()).collect(),
                    },
                });
                self.prefixes.insert(
                    buffer,
                    PrefixInfo {
                        seed_binding: Some(self.scoping.symbol_span(seed).into()),
                        span: Span::new(
                            self.prefix_parts[&buffer].first_store,
                            store.end,
                        ),
                        seed_word,
                        binding: self.name(seed),
                        elements,
                    },
                );
            }
        }
    }

    fn record_payload_index_assignment(
        &mut self,
        buffer: SymbolId,
        index: &str,
        value: &Expression<'a>,
    ) {
        if let (Some(value_buffer), Some(index)) = (
            self.value_symbol(value).map(|buffer| self.resolve_buffer_alias(buffer)),
            parse_index(index),
        ) {
            self.payload_index.insert((buffer, index), value_buffer);
        }
    }

    fn record_binding(&mut self, declarator: &VariableDeclarator<'a>) {
        let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
            return;
        };
        let Some(symbol) = identifier.symbol_id.get() else {
            return;
        };
        if let Some(init) = declarator.init.as_ref() {
            let payload = self.array_payload_buffer(init);
            self.record_buffer_assignment(symbol, init, declarator.span);
            self.record_value_binding(symbol, init, payload);
        }
    }

    /// Resolves only the narrow buffer alias form that is safe to coalesce:
    /// an identifier currently known to hold a buffer, or a stable plain alias
    /// of one.  The general program-wide alias table is intentionally not used
    /// here because it has no path/definition-join proof for mutable buffers.
    fn buffer_symbol(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        // `t = [value, buffer, buffer]; write(t[0], t[1], i)`: the tracer's
        // argument tuple names the buffer binding it holds.
        let symbol = expression_symbol(expression, self.scoping)
            .or_else(|| self.tuple_element(expression).and_then(|(symbol, _)| *symbol))
            .or_else(|| literal_tuple_element(expression).and_then(|element| expression_symbol(element, self.scoping)))?;
        Some(self.resolve_buffer_alias(symbol))
    }

    /// After an `if`, names that hold one array on each path hold the same
    /// array: `if (g) { a = [..]; b = a; } else { b = [..]; a = b; }
    /// b[6] = v; emit(a)` writes the emitted array after the join. Every
    /// name whose two arm roots match another name's (and that is not yet a
    /// common alias) joins the group's then-arm root, or else-arm root;
    /// the choice keeps each arm's concrete buffer for the grammar.
    fn join_swapped_aliases(
        &mut self,
        then_aliases: &BTreeMap<SymbolId, SymbolId>,
        else_aliases: &BTreeMap<SymbolId, SymbolId>,
    ) {
        let names: BTreeSet<SymbolId> = then_aliases
            .iter()
            .chain(else_aliases.iter())
            .flat_map(|(&key, &value)| [key, value])
            .collect();
        let mut groups: BTreeMap<(SymbolId, SymbolId), Vec<SymbolId>> = BTreeMap::new();
        for name in names {
            let roots = (resolve_alias_map(then_aliases, name), resolve_alias_map(else_aliases, name));
            if roots.0 == roots.1 || !self.known_buffers.contains(&roots.0) || !self.known_buffers.contains(&roots.1) {
                continue;
            }
            groups.entry(roots).or_default().push(name);
        }
        for ((then_root, else_root), group) in groups {
            if group.len() < 2 {
                continue;
            }
            let representative = if group.contains(&then_root) {
                then_root
            } else if group.contains(&else_root) {
                else_root
            } else {
                continue;
            };
            for name in group {
                if name != representative && !self.buffer_aliases.contains_key(&name) {
                    self.buffer_aliases.insert(name, representative);
                }
            }
        }
    }

    fn resolve_buffer_alias(&self, mut symbol: SymbolId) -> SymbolId {
        for _ in 0..16 {
            match self.buffer_aliases.get(&symbol) {
                Some(&next) if next != symbol => symbol = next,
                _ => break,
            }
        }
        symbol
    }

    /// Records a stable `holder = buffer` alias, and makes a reassignment
    /// between two known buffers visibly unresolved.  That assignment may
    /// change which backing array a later writer mutates (including through a
    /// branch), so a symbol-only grammar is not a complete proof.
    fn record_buffer_assignment(&mut self, target: SymbolId, value: &Expression<'a>, span: Span) {
        let source = self
            .buffer_symbol(value)
            .filter(|symbol| self.known_buffers.contains(symbol));
        let target_root = self.resolve_buffer_alias(target);
        let target_is_buffer = self.known_buffers.contains(&target_root);

        self.buffer_aliases.remove(&target);
        match (target_is_buffer, source) {
            (true, Some(source)) if source != target_root => {
                // Both names hold the same array, so coalesce them. The old rule
                // recorded an `Unsupported` boundary here, which demoted every
                // collector that re-seeds its payload buffer inside a branch
                // (`else { buf = other; …; write(…, buf, …) }`). Writes recorded
                // before the alias kept their own key, so only uses after the
                // rebinding follow the new name.
                let _ = span;
                self.buffer_aliases.insert(target, source);
            }
            (false, Some(source)) => {
                // A plain, first-origin alias is stable until a later write
                // clears it.  This keeps the known holder-coalescing case.
                self.buffer_aliases.insert(target, source);
            }
            // `a = b; …; a = b`: a repeated copy keeps the alias it repeats.
            (true, Some(source)) if source != target => {
                self.buffer_aliases.insert(target, source);
            }
            _ => {}
        }
    }

    /// Records the structural role of a binding's value: observer alias, seed
    /// word, prefix array, mask closure, payload buffer, payload object, or a
    /// STRING-writer closure returned by the STRING writer factory.
    /// `payload` is the `[aux, buffer]` buffer of `value`, resolved by the
    /// caller before this write rebinds `symbol`'s buffer alias:
    /// `w = buf; …; w = [[], w]` delivers `buf`.
    fn record_value_binding(&mut self, symbol: SymbolId, value: &Expression<'a>, payload: Option<SymbolId>) {
        // The value is read against the bindings' state before this write
        // (`r = [r >>> 24, ..]` reads the old `r`); only then does `symbol`
        // take its new seed alias, if any.
        let copied_seed = match value.without_parentheses() {
            Expression::Identifier(identifier) => resolve_reference(identifier, self.scoping)
                .filter(|alias| *alias != symbol)
                .and_then(|alias| {
                    if self.seeds.contains_key(&alias) {
                        Some(alias)
                    } else {
                        self.seed_aliases.get(&alias).copied()
                    }
                }),
            _ => None,
        };
        let held_draw = match value.without_parentheses() {
            Expression::CallExpression(draw) if draw.arguments.is_empty() => {
                expression_symbol(&draw.callee, self.scoping)
                    .and_then(|callee| self.masks.get(&callee).cloned())
                    .map(|mut mask| {
                        mask.predrawn = Some(draw.span);
                        mask
                    })
            }
            _ => None,
        };
        self.record_value_binding_role(symbol, value, payload);
        // The draw span lets `predrawn_register_mask` require the draw and
        // consuming write to be adjacent in the same block. That also rejects
        // branch-local draws without excluding valid reassigned registers.
        if let Some(mask) = held_draw {
            self.register_draws.insert(symbol, mask);
        }
        match copied_seed {
            Some(seed) => {
                self.seed_aliases.insert(symbol, seed);
            }
            None => {
                self.seed_aliases.remove(&symbol);
            }
        }
    }

    fn record_value_binding_role(&mut self, symbol: SymbolId, value: &Expression<'a>, payload: Option<SymbolId>) {
        // A bare scalar is only the binding's latest value; any rebinding
        // replaces it (`x = a || b; x = x != null ? x.checked : ""`).
        self.payload_scalars.remove(&symbol);
        if let Some(tree) = self.scalar_tree(value, 0) {
            self.payload_scalars.insert(symbol, tree);
        }
        self.object_aliases.remove(&symbol);
        if let Some(source) = expression_symbol(value, self.scoping) {
            if source != symbol && self.payload_objects.contains_key(&source) {
                self.object_aliases.insert(symbol, source);
            }
        }
        // A reused scratch name can hold a complete payload, then a new
        // one-element list populated with `list[1] = buffer`. Its earlier
        // cached second element must not outrank the new indexed assignment.
        if expression_symbol(value, self.scoping) != Some(symbol) {
            self.payload_buffers.remove(&symbol);
            self.payload_objects.remove(&symbol);
            self.payload_index.retain(|(owner, _), _| *owner != symbol);
        }
        self.bump_version(symbol);
        if is_string_split_call(value) {
            self.split_results.insert(symbol);
        } else {
            self.split_results.remove(&symbol);
        }
        self.values.insert(
            symbol,
            ValueBinding {
                // Resolve at assignment time so a later read of the binding does
                // not lose the host expression inside it (a stored text like
                // `Object.getOwnPropertyNames($CTX.Math)` is already `…(Math)`).
                text: self
                    .resolve_value(value, 0)
                    .unwrap_or_else(|| self.slice(value.span())),
                alias: expression_symbol(value, self.scoping),
            },
        );
        if self.site == EmissionSite::WorkerReply {
            self.record_worker_value(symbol, value);
        }
        // A binding names the observer from its latest write only. Resolve the
        // new value against the previous state, then drop any stale alias: an
        // assignment away from the observer must not keep an earlier route.
        let observer = self.observer_target(value);
        // `m = m.next` reads the observer through the *old* value of `m`, so the
        // next-method target must be resolved before this write drops the alias.
        // The same holds for `m = observer.next` where `m` already aliases the
        // observer it reads.
        let next_method = self.next_method_target(value);
        // A copy of a stream binding (`s = stream`) names that stream. (A
        // stream binding keeps its entry when rebound to a value drawn from
        // it, such as `stream = writerFactory(stream)`.)
        if let Expression::Identifier(identifier) = value.without_parentheses() {
            if let Some(mask) = resolve_reference(identifier, self.scoping)
                .and_then(|alias| self.masks.get(&alias).cloned())
            {
                self.masks.insert(symbol, mask);
            }
        }
        self.aliases_out.remove(&symbol);
        self.next_methods.remove(&symbol);
        self.tuples.remove(&symbol);
        self.tuple_values.remove(&symbol);
        self.tuple_seeds.remove(&symbol);
        self.tuple_seed_words.remove(&symbol);
        let holds_number_writer = matches!(value.without_parentheses(), Expression::Identifier(_))
            && self.write_kind(value) == Some(LeafKind::Number);
        if holds_number_writer {
            self.number_holders.insert(symbol);
            return;
        }
        self.number_holders.remove(&symbol);
        if let Expression::ArrayExpression(array) = value.without_parentheses() {
            if array.elements.len() >= 2 {
                let sealed = self.sealed.is_some_and(|sealed| sealed.contains(&symbol));
                let seeds: Vec<Option<SymbolId>> = array
                    .elements
                    .iter()
                    .map(|element| {
                        let expression = element.as_expression()?;
                        if let Some(seed) = self.tuple_seed(expression) {
                            return Some(seed);
                        }
                        // `t = [[], Math.random() * 4294967296 | 0]`: the
                        // element is the draw itself; the tuple stands for
                        // its seed ([`Self::tuple_seed_words`]).
                        if is_seed_word(expression, self.scoping) {
                            return Some(symbol);
                        }
                        // `t = [b, s | 0]`: the seed word truncated to 32
                        // bits is the seed word.
                        let expression = match expression.without_parentheses() {
                            Expression::BinaryExpression(binary)
                                if binary.operator == BinaryOperator::BitwiseOR && numeric_value(&binary.right) == Some(0.0) =>
                            {
                                &binary.left
                            }
                            expression => expression,
                        };
                        let symbol = expression_symbol(expression, self.scoping)?;
                        if self.seeds.contains_key(&symbol) {
                            Some(symbol)
                        } else {
                            self.seed_aliases.get(&symbol).copied()
                        }
                    })
                    .collect();
                let elements = array
                    .elements
                    .iter()
                    .enumerate()
                    .map(|(position, element)| {
                        element.as_expression().map(|expression| {
                            // An element read from another tuple (`d = [c[0],
                            // ..]` with `c = [buffer, ..]`) holds what that
                            // tuple element holds.
                            if let Some(inner) = self.tuple_element(expression) {
                                return inner.clone();
                            }
                            // `t = [[], …]` in a sealed tuple: the literal is
                            // a buffer of its own.
                            if let Expression::ArrayExpression(literal) = expression.without_parentheses() {
                                if sealed && literal.elements.is_empty() {
                                    let buffer = self.virtual_buffer(symbol, position, literal.span);
                                    return (Some(buffer), self.slice(expression.span()));
                                }
                            }
                            (
                                expression_symbol(expression, self.scoping),
                                self.slice(expression.span()),
                            )
                        })
                    })
                    .collect();
                if let Some(draw) = array
                    .elements
                    .iter()
                    .filter_map(|element| element.as_expression())
                    .find(|expression| is_seed_word(expression, self.scoping))
                {
                    self.tuple_seed_words.insert(symbol, self.slice(draw.span()));
                }
                if seeds.iter().any(Option::is_some) {
                    self.tuple_seeds.insert(symbol, seeds);
                }
                self.tuples.insert(symbol, elements);
                let draws = array
                    .elements
                    .iter()
                    .map(|element| match element.as_expression().map(Expression::without_parentheses) {
                        Some(Expression::CallExpression(draw)) if draw.arguments.is_empty() => expression_symbol(&draw.callee, self.scoping)
                            .and_then(|stream| self.masks.get(&stream).cloned())
                            .or_else(|| self.mask_of(&draw.callee)),
                        _ => None,
                    })
                    .collect();
                self.tuple_draws.insert(symbol, draws);
                self.tuple_values.insert(
                    symbol,
                    array.elements.iter().map(|element| element.as_expression().map(GetSpan::span)).collect(),
                );
            }
        }
        if let Expression::CallExpression(call) = value.without_parentheses() {
            if let Some(callee_symbol) = expression_symbol(&call.callee, self.scoping) {
                if self.resolves_to_any_helper(callee_symbol, &self.helpers.string_writer) {
                    // Capture the whole mask value at closure creation. The
                    // factory argument may be an inline `prng(seed ^ C)` call
                    // or a mask binding; either way the writer keeps that value
                    // even if the source binding is reassigned later.
                    let mask = call
                        .arguments
                        .first()
                        .and_then(Argument::as_expression)
                        .and_then(|argument| self.mask_info_for(argument));
                    self.string_writers.insert(symbol, mask);
                    self.string_instances.insert(symbol, call.span);
                    if let Some(factory) = self.helpers.string_writer.iter().find(|h| {
                        self.resolves_to_any_helper(callee_symbol, std::slice::from_ref(h))
                    }) {
                        self.string_factories
                            .insert(symbol, self.scoping.symbol_span(*factory));
                    }
                    return;
                }
            }
        }
        if let Expression::Identifier(identifier) = value.without_parentheses() {
            if let Some(alias) = resolve_reference(identifier, self.scoping) {
                if let Some(mask) = self.string_writers.get(&alias).cloned() {
                    self.string_writers.insert(symbol, mask);
                    if let Some(instance) = self.string_instances.get(&alias).copied() {
                        self.string_instances.insert(symbol, instance);
                    }
                    if let Some(factory) = self.string_factories.get(&alias).copied() {
                        self.string_factories.insert(symbol, factory);
                    }
                    return;
                }
            }
        }
        if let Some(target) = observer {
            // A binding that becomes the observer is no longer the writer
            // closure an earlier write made it: a copy of it (`t = local`) must
            // alias the observer, not inherit the stale writer role and hide the
            // `.next` call that follows.
            self.string_writers.remove(&symbol);
            self.string_instances.remove(&symbol);
            self.string_factories.remove(&symbol);
            self.aliases_out.insert(symbol, target);
            return;
        }
        if is_seed_word(value, self.scoping) {
            self.seeds.insert(symbol, self.slice(value.span()));
            return;
        }
        if let Some(seed) = self.seed_copy_array(value) {
            self.seed_copies.insert(symbol, seed);
            return;
        }
        if let Some(prefix) = self.detect_prefix(value) {
            self.known_buffers.insert(symbol);
            self.push_op(IOp::Prefix {
                buffer: symbol,
                node: PrefixNode {
                    seed_binding: prefix.seed_binding,
                    span: value.span(),
                    spelling: PrefixSpelling::ArrayLiteral,
                    seed_word: prefix.seed_word.clone(),
                    elements: prefix.elements.iter().map(|(text, _)| text.clone()).collect(),
                },
            });
            let tail = prefix.elements.len();
            self.prefixes.insert(symbol, prefix);
            self.literal_tail_bytes(symbol, value, tail);
            return;
        }
        if let Some(target) = next_method {
            self.next_methods.insert(symbol, target);
            return;
        }
        if let Some(mask) = self.mask_of(value) {
            self.masks.insert(symbol, mask);
            return;
        }
        if let Some(buffer) = payload {
            self.payload_buffers.insert(symbol, buffer);
            return;
        }
        if let Some(fields) = self.object_fields(value) {
            self.payload_objects.insert(symbol, fields);
        }
    }

    fn detect_prefix(&self, expression: &Expression<'a>) -> Option<PrefixInfo> {
        let Expression::ArrayExpression(array) = expression.without_parentheses() else {
            return None;
        };
        if array.elements.is_empty() {
            return None;
        }
        let mut seed_symbol: Option<SymbolId> = None;
        let mut elements = Vec::with_capacity(array.elements.len());
        for (position, element) in array.elements.iter().enumerate() {
            let expression = element.as_expression()?;
            // `[s >>> 24, .., s & 255, v ^ mask()]`: the tracer folds the
            // first payload byte stores into the prefix literal; the prefix
            // is the four seed bytes and the rest are byte writes
            // ([`Self::literal_tail_bytes`]).
            if position >= 4 && elements.len() == 4 && self.seed_symbol_in(expression).is_none() {
                break;
            }
            let symbol = self.seed_symbol_in(expression)?;
            match seed_symbol {
                None => seed_symbol = Some(symbol),
                Some(existing) if existing == symbol => {}
                _ => return None,
            }
            elements.push((self.slice(expression.span()), expression.span()));
        }
        let seed_symbol = seed_symbol?;
        let seed_word = self
            .seeds
            .get(&seed_symbol)
            .cloned()
            .or_else(|| self.name(seed_symbol))
            .unwrap_or_default();
        Some(PrefixInfo {
            seed_binding: Some(self.scoping.symbol_span(seed_symbol).into()),
            span: expression.span(),
            seed_word,
            binding: self.name(seed_symbol),
            elements,
        })
    }

    /// The elements after a prefix literal's seed bytes (`[.., s & 255, v ^
    /// mask()]`) are byte writes at their positions, as `buf[4] = v ^
    /// mask()` would be.
    fn literal_tail_bytes(&mut self, buffer: SymbolId, value: &Expression<'a>, from: usize) {
        let Expression::ArrayExpression(array) = value.without_parentheses() else {
            return;
        };
        for (position, element) in array.elements.iter().enumerate().skip(from) {
            let Some(right) = element.as_expression() else {
                continue;
            };
            let (xor_value, xor_source, xor_span) = xor_evidence(right, self.source);
            let masked = xor_value.is_some();
            let mask = byte_write_mask(right, self.scoping)
                .and_then(|symbol| self.masks.get(&symbol).cloned())
                .or_else(|| self.inline_stream_draw(right));
            let (mask_form, mask_source, logical_span) = match split_byte_mask(right, &self.masks, self.scoping) {
                ByteMask::Stream { value, mask: mask_call } => (
                    if mask.is_some() { MaskForm::Stream } else { MaskForm::Unresolved },
                    Some(
                        mask.as_ref()
                            .map(|mask| mask.argument.clone())
                            .filter(|argument| !argument.is_empty())
                            .unwrap_or_else(|| self.slice(mask_call)),
                    ),
                    value,
                ),
                ByteMask::StaticXor { value } => (MaskForm::StaticXor, None, value),
                ByteMask::None => (MaskForm::Unmasked, None, right.span()),
            };
            self.push_op(IOp::Leaf(ILeaf {
                index_span: None,
                writer_factory: None,
                string_instance: None,
                kind: LeafKind::Byte,
                span: right.span(),
                buffer: Some(buffer),
                mask,
                masked,
                value: Some(self.slice(right.span())),
                index: Some(position.to_string()),
                xor_literal: xor_value,
                xor_source,
                xor_span,
                byte_width: None,
                logical_value: self.slice(logical_span),
                logical_span,
                resolved_value: self.resolve_value(right, 0),
                mask_form,
                mask_source,
                prefix_element: None,
                helper_call: None,
                helper_arguments: Vec::new(),
            }));
        }
    }

    /// `[w, w]`: an array whose elements are all bare reads of one seed word.
    fn seed_copy_array(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        let Expression::ArrayExpression(array) = expression.without_parentheses() else {
            return None;
        };
        let mut seed = None;
        for element in &array.elements {
            // `[w, w]` or `[w | 0, w | 0]` (a float seed word truncated).
            let element = match element.as_expression()?.without_parentheses() {
                Expression::BinaryExpression(binary)
                    if binary.operator == oxc_syntax::operator::BinaryOperator::BitwiseOR
                        && matches!(binary.right.without_parentheses(), Expression::NumericLiteral(zero) if zero.value == 0.0) =>
                {
                    &binary.left
                }
                other => other,
            };
            let Expression::Identifier(identifier) = element.without_parentheses() else {
                return None;
            };
            let symbol = resolve_reference(identifier, self.scoping)?;
            if !self.seeds.contains_key(&symbol) || seed.is_some_and(|seed| seed != symbol) {
                return None;
            }
            seed = Some(symbol);
        }
        seed
    }

    fn seed_symbol_in(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        let mut found = None;
        let mut stack = vec![expression];
        while let Some(current) = stack.pop() {
            match current {
                Expression::ComputedMemberExpression(member) => {
                    if let Expression::Identifier(identifier) = member.object.without_parentheses() {
                        if let Some(copy) = resolve_reference(identifier, self.scoping) {
                            if let Some(seed) = self.seed_copies.get(&copy) {
                                found = Some(*seed);
                            }
                        }
                    }
                    if let Some(seed) = self.tuple_seed(current) {
                        found = Some(seed);
                    }
                }
                Expression::Identifier(identifier) => {
                    if let Some(symbol) = resolve_reference(identifier, self.scoping) {
                        if self.seeds.contains_key(&symbol) {
                            found = Some(symbol);
                        } else if let Some(seed) = self.seed_aliases.get(&symbol) {
                            found = Some(*seed);
                        }
                    }
                }
                Expression::BinaryExpression(binary) => {
                    stack.push(&binary.left);
                    stack.push(&binary.right);
                }
                Expression::UnaryExpression(unary) => stack.push(&unary.argument),
                Expression::ParenthesizedExpression(paren) => stack.push(&paren.expression),
                // An inline store helper's call value is its returned argument.
                Expression::CallExpression(call) => {
                    if let Some(store) = inline_store_helper(&call.callee, self.scoping) {
                        if let Some(argument) = call.arguments.get(store.returned).and_then(Argument::as_expression) {
                            stack.push(argument);
                        }
                    } else if let Some(proof) = self.named_store_helper(call) {
                        if let Some(argument) = call.arguments.get(proof.returned).and_then(Argument::as_expression) {
                            stack.push(argument);
                        }
                    }
                }
                _ => {}
            }
        }
        found
    }

    /// `((a, b, c, d, e) => { a[b] = c >>> d; return e; })(buf, k, s, 24, s)`
    /// with `k` in 0..=3 and the stored value read from a seed: the raw-prefix
    /// store `buf[k] = s >>> 24`. Returns whether the call was taken.
    /// The proof of a call to a named plain store helper
    /// (`function h(a, b, c, d) { a[b] = c; return d; }`).
    fn named_store_helper(&self, call: &CallExpression<'a>) -> Option<&'s ReturnedParameterProof> {
        let callee = expression_symbol(&call.callee, self.scoping)?;
        self.returned_parameters.get(&callee)
    }

    fn record_inline_prefix_store(&mut self, call: &CallExpression<'a>) -> bool {
        let Some(store) = inline_store_helper(&call.callee, self.scoping) else {
            return self.record_named_prefix_store(call);
        };
        let argument = |position: usize| call.arguments.get(position).and_then(Argument::as_expression);
        let (Some(array), Some(index)) = (argument(store.written), argument(store.index)) else {
            return false;
        };
        let Some(buffer) = self.buffer_symbol(array) else {
            return false;
        };
        let Some(index) = numeric_value(index).filter(|value| value.fract() == 0.0 && (0.0..=3.0).contains(value)) else {
            return false;
        };
        // A buffer that already has a (short-literal) prefix gets the byte as
        // a prefix-element store, grouped with the literal later.
        let prefixed = self.prefixes.contains_key(&buffer);
        // The stored value with each formal read as the caller's argument.
        let mut references: Vec<(Span, usize)> = Vec::new();
        let mut stack = vec![store.value];
        while let Some(current) = stack.pop() {
            match current.without_parentheses() {
                Expression::Identifier(identifier) => {
                    if let Some(position) = expression_parameter_index(current, &store.formals, self.scoping) {
                        references.push((identifier.span, position));
                    }
                }
                Expression::BinaryExpression(binary) => {
                    stack.push(&binary.left);
                    stack.push(&binary.right);
                }
                Expression::UnaryExpression(unary) => stack.push(&unary.argument),
                _ => {}
            }
        }
        let mut seed = None;
        for (_, position) in &references {
            match argument(*position).and_then(|expression| self.seed_symbol_in(expression)) {
                Some(found) if seed.is_none_or(|seed| seed == found) => seed = Some(found),
                Some(_) => return false,
                None => {}
            }
        }
        let Some(seed) = seed else {
            return false;
        };
        let value_span = store.value.span();
        let mut text = self.slice(value_span);
        references.sort_by_key(|(span, _)| std::cmp::Reverse(span.start));
        for (span, position) in references {
            let Some(caller) = argument(position) else { return false };
            let (start, end) = ((span.start - value_span.start) as usize, (span.end - value_span.start) as usize);
            if end > text.len() || !text.is_char_boundary(start) || !text.is_char_boundary(end) {
                return false;
            }
            text.replace_range(start..end, &self.slice(caller.span()));
        }
        if prefixed {
            self.push_op(IOp::Leaf(ILeaf {
                index_span: None,
                writer_factory: None,
                string_instance: None,
                kind: LeafKind::Byte,
                span: call.span,
                buffer: Some(buffer),
                mask: None,
                masked: false,
                value: Some(text.clone()),
                index: Some(format!("{index}")),
                xor_literal: None,
                xor_source: None,
                xor_span: Span::default(),
                byte_width: None,
                logical_value: text,
                logical_span: call.span,
                resolved_value: None,
                mask_form: MaskForm::Unmasked,
                mask_source: None,
                prefix_element: Some(index as u32),
                helper_call: None,
                helper_arguments: Vec::new(),
            }));
            return true;
        }
        self.record_prefix_part(buffer, index as u32, seed, text, call.span, call.span);
        true
    }

    /// `h(buf, k, s >>> 24, r)` through a named plain store helper, `k` in
    /// 0..=3 and the stored argument read from a seed: the raw-prefix store
    /// `buf[k] = s >>> 24`.
    fn record_named_prefix_store(&mut self, call: &CallExpression<'a>) -> bool {
        let Some(proof) = self.named_store_helper(call) else {
            return false;
        };
        let argument = |position: usize| call.arguments.get(position).and_then(Argument::as_expression);
        let (Some(array), Some(index), Some(value)) = (argument(proof.written), argument(proof.index), argument(proof.value)) else {
            return false;
        };
        let Some(buffer) = self.buffer_symbol(array) else {
            return false;
        };
        let Some(index) = numeric_value(index).filter(|value| value.fract() == 0.0 && (0.0..=3.0).contains(value)) else {
            return false;
        };
        let Some(seed) = self.seed_symbol_in(value) else {
            return false;
        };
        let text = self.slice(value.span());
        if self.prefixes.contains_key(&buffer) {
            self.push_op(IOp::Leaf(ILeaf {
                index_span: None,
                writer_factory: None,
                string_instance: None,
                kind: LeafKind::Byte,
                span: call.span,
                buffer: Some(buffer),
                mask: None,
                masked: false,
                value: Some(text.clone()),
                index: Some(format!("{index}")),
                xor_literal: None,
                xor_source: None,
                xor_span: Span::default(),
                byte_width: None,
                logical_value: text,
                logical_span: value.span(),
                resolved_value: None,
                mask_form: MaskForm::Unmasked,
                mask_source: None,
                prefix_element: Some(index as u32),
                helper_call: None,
                helper_arguments: Vec::new(),
            }));
            return true;
        }
        self.record_prefix_part(buffer, index as u32, seed, text, value.span(), call.span);
        true
    }

    fn observer_target(&self, expression: &Expression<'a>) -> Option<ObsTarget> {
        match expression.without_parentheses() {
            Expression::Identifier(identifier) => {
                let symbol = resolve_reference(identifier, self.scoping)?;
                self.aliases_out.get(&symbol).copied()
            }
            Expression::ComputedMemberExpression(member) => {
                let index = observer_index(&member.expression)?;
                if is_arguments(&member.object, self.scoping) {
                    // `arguments` is the collector's own only for an ordinary
                    // root at depth zero. An arrow root keeps the enclosing
                    // function's `arguments`, and a nested ordinary function
                    // owns a fresh one. A nested ordinary function is a valid
                    // direct task only when the collector hands it to an
                    // `arguments[...]` task scheduler; a wave collector never
                    // forwards observers, and an uncalled nested function must
                    // not inherit the route.
                    let owns_root_arguments = self.root_owns_arguments && self.fn_depth == 0;
                    if !owns_root_arguments {
                        let forwarded = matches!(self.observers, ObserverWiring::Direct { .. })
                            && self
                                .current_function
                                .is_some_and(|start| self.forwarded_functions.contains(&start));
                        if !forwarded {
                            return None;
                        }
                    }
                    return (index == 0).then_some(ObsTarget::ArgumentsList);
                }
                match self.observer_target(&member.object)? {
                    ObsTarget::ArgumentsList => Some(ObsTarget::Slot(index)),
                    ObsTarget::Slot(_) => None,
                }
            }
            _ => None,
        }
    }

    /// The binding a writer callee names: a plain binding or an element of
    /// the tracer's argument tuple (`t = [w, v, buf]; t[0](…)`).
    fn writer_symbol(&self, callee: &Expression<'a>) -> Option<SymbolId> {
        expression_symbol(callee, self.scoping).or_else(|| self.tuple_element(callee).and_then(|(symbol, _)| *symbol))
    }

    fn write_kind(&self, callee: &Expression<'a>) -> Option<LeafKind> {
        if let Some(symbol) = self.writer_symbol(callee) {
            if self.number_holders.contains(&symbol)
                || self.resolves_to_any_helper(symbol, &self.helpers.number_writer)
            {
                return Some(LeafKind::Number);
            }
            if self.string_writers.contains_key(&symbol) {
                return Some(LeafKind::String);
            }
        }
        if let Expression::CallExpression(inner) = callee.without_parentheses() {
            if let Some(inner_symbol) = expression_symbol(&inner.callee, self.scoping) {
                if self.resolves_to_any_helper(inner_symbol, &self.helpers.string_writer) {
                    return Some(LeafKind::String);
                }
            }
        }
        None
    }

    /// `writer(value, buffer, position)`: the STRING-writer closure calling
    /// convention, whether the closure was produced in place
    /// (`factory(mask)(value, buffer, pos)`) or is held in a binding
    /// (`w = factory(mask); w(value, buffer, pos)`).
    ///
    /// Prefer the source-time closure over a later global NUMBER alias. A
    /// three-argument host call alone is never evidence of a STRING writer.
    fn chained_string_write(&self, call: &CallExpression<'a>) -> Option<LeafKind> {
        if call.arguments.len() != 3 {
            return None;
        }
        let proven = match call.callee.without_parentheses() {
            Expression::Identifier(id) => resolve_reference(id, self.scoping)
                .is_some_and(|symbol| self.string_writers.contains_key(&symbol)),
            Expression::CallExpression(inner) => expression_symbol(&inner.callee, self.scoping)
                .is_some_and(|symbol| self.resolves_to_any_helper(symbol, &self.helpers.string_writer)),
            _ => false,
        };
        proven.then_some(LeafKind::String)
    }

    fn mask_symbol(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        if let Some(symbol) = expression_symbol(expression, self.scoping) {
            if self.masks.contains_key(&symbol) {
                return Some(symbol);
            }
        }
        None
    }

    /// The binding that holds an expression's value when it is provable from
    /// source without executing anything: a plain identifier, or the argument a
    /// verified store helper returns unchanged (`store(array, index, value,
    /// returned)`). The emitted program builds many `[aux, buffer]` payloads
    /// this way instead of an array literal.
    fn value_symbol(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        self.value_symbol_with_depth(expression, 0)
    }

    /// Recursion descends into one call argument at a time, so the finite AST
    /// already bounds it; the explicit budget keeps an adversarial nesting chain
    /// from becoming unbounded work and fails closed instead of guessing.
    fn value_symbol_with_depth(
        &self,
        expression: &Expression<'a>,
        depth: usize,
    ) -> Option<SymbolId> {
        if depth > VALUE_SYMBOL_MAX_DEPTH {
            return None;
        }
        if let Some(symbol) = expression_symbol(expression, self.scoping) {
            return Some(symbol);
        }
        // `t[k]` of a register tuple holding a buffer binding (or a sealed
        // tuple's own empty literal).
        if let Some((Some(symbol), _)) = self.tuple_element(expression) {
            return Some(*symbol);
        }
        let Expression::CallExpression(call) = expression.without_parentheses() else {
            return None;
        };
        // An inline store helper (`((a, b, c, d) => { a[b] = c; return d; })(…)`,
        // a tracer spelling) carries its own proof.
        let inline;
        let proof = match call.callee.without_parentheses() {
            Expression::ArrowFunctionExpression(arrow) => {
                inline = returned_arrow_parameter_proof(arrow, self.scoping)?;
                &inline
            }
            _ => {
                let callee = expression_symbol(&call.callee, self.scoping)?;
                self.returned_parameters.get(&callee)?
            }
        };
        let argument = call.arguments.get(proof.returned)?.as_expression()?;
        let returned = self.value_symbol_with_depth(argument, depth + 1)?;
        // The helper writes only `arguments[written][...]`. If that argument is
        // not a plain binding, the returned binding could be aliased inside the
        // write and mutated in a way this extractor does not model. Reject that
        // call shape rather than attach a buffer whose bytes we may be missing.
        let written = call.arguments.get(proof.written)?.as_expression()?;
        expression_symbol(written, self.scoping)?;
        // Likewise, any other call argument that resolves to the returned
        // binding (directly or through a recorded identifier alias) makes the
        // store able to target the returned buffer itself.
        let returned_alias = follow_aliases(self.aliases, returned);
        for (position, argument) in call.arguments.iter().enumerate() {
            if position == proof.returned {
                continue;
            }
            let Some(expression) = argument.as_expression() else {
                continue;
            };
            let Some(symbol) = expression_symbol(expression, self.scoping) else {
                continue;
            };
            if follow_aliases(self.aliases, symbol) == returned_alias {
                return None;
            }
        }
        Some(returned)
    }

    fn mask_of(&self, expression: &Expression<'a>) -> Option<MaskInfo> {
        let Expression::CallExpression(call) = expression.without_parentheses() else {
            return None;
        };
        let factory_symbol = expression_symbol(&call.callee, self.scoping)?;
        if !self.resolves_to_any_helper(factory_symbol, &self.helpers.prng_factory) {
            return None;
        }
        let argument = call.arguments.first()?.as_expression()?;
        let (xor_literal, xor_source, xor_span) = xor_evidence(argument, self.source);
        Some(MaskInfo {
            predrawn: None,
            seed_binding: match argument.without_parentheses() {
                Expression::BinaryExpression(b) if b.operator == BinaryOperator::BitwiseXOR => {
                    expression_symbol(&b.left, self.scoping)
                        .map(|s| self.scoping.symbol_span(s).into())
                }
                _ => None,
            },
            instance: call.span,
            factory_binding: self.scoping.symbol_span(
                *self.helpers.prng_factory.iter().find(|h| {
                    self.resolves_to_any_helper(factory_symbol, std::slice::from_ref(h))
                })?,
            ),
            factory: self.name(factory_symbol),
            seed_word: xor_left_text(argument, self.source),
            xor_literal,
            xor_source,
            xor_span,
            argument: self.slice(argument.span()),
        })
    }

    /// Full mask evidence for a writer's mask argument: an inline PRNG factory
    /// call keeps its expression, while an identifier resolves to the mask
    /// binding's own recorded factory call.
    fn mask_info_for(&self, expression: &Expression<'a>) -> Option<MaskInfo> {
        if let Some(mask) = self.mask_of(expression) {
            return Some(mask);
        }
        let symbol = expression_symbol(expression, self.scoping)?;
        self.masks.get(&symbol).cloned()
    }

    fn xor_of(&self, expression: &Expression<'a>) -> Option<f64> {
        if let Some(mask) = self.mask_info_for(expression) {
            return mask.xor_literal;
        }
        xor_literal(expression)
    }

    fn resolve_payload(&self, payload: &Expression<'a>) -> PayloadResolution {
        if let Some(buffer) = self.resolve_buffer(payload) {
            return PayloadResolution::Buffer(buffer);
        }
        if let Some(fields) = self.resolve_object(payload) {
            return PayloadResolution::Object(fields);
        }
        if let Some(tree) = self.resolve_scalar(payload) {
            return PayloadResolution::Scalar(tree);
        }
        PayloadResolution::Unresolved(self.slice(payload.span()))
    }

    fn resolve_buffer(&self, payload: &Expression<'a>) -> Option<SymbolId> {
        match payload.without_parentheses() {
            Expression::ArrayExpression(_) => self.array_payload_buffer(payload),
            Expression::Identifier(identifier) => {
                let symbol = resolve_reference(identifier, self.scoping)?;
                if let Some(buffer) = self.payload_buffers.get(&symbol) {
                    return Some(*buffer);
                }
                self.payload_index
                    .get(&(symbol, 1))
                    .copied()
                    .map(|buffer| self.resolve_buffer_alias(buffer))
            }
            _ => None,
        }
    }

    /// The emitted buffer of an `[aux, buffer]` payload expression. The first
    /// element is an auxiliary metadata list and need not be empty; the source
    /// buffer is always the second element.
    fn array_payload_buffer(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        let Expression::ArrayExpression(array) = expression.without_parentheses() else {
            return None;
        };
        if array.elements.len() != 2 {
            return None;
        }
        let second = array.elements[1].as_expression()?;
        self.value_symbol(second)
            .map(|buffer| self.resolve_buffer_alias(buffer))
    }

    fn resolve_object(&self, payload: &Expression<'a>) -> Option<Vec<ObjectField>> {
        match payload.without_parentheses() {
            // `observer.next({ a: …, b: … })`: a raw named object delivered
            // inline. Its fields are typed where the value proves one; a member
            // read (`screen.innerWidth`) stays an untyped logical field rather
            // than sinking the whole payload.
            Expression::ObjectExpression(_) => self.object_fields(payload),
            Expression::Identifier(identifier) => {
                let symbol = resolve_reference(identifier, self.scoping)?;
                self.payload_objects.get(&symbol).cloned()
            }
            _ => None,
        }
    }

    /// `observer.next(x)` with `x` a local whose latest value is a bare typed
    /// scalar (no buffer, no named object).
    fn resolve_scalar(&self, payload: &Expression<'a>) -> Option<ScalarTree> {
        let Expression::Identifier(identifier) = payload.without_parentheses() else {
            return None;
        };
        let symbol = resolve_reference(identifier, self.scoping)?;
        self.payload_scalars.get(&symbol).cloned()
    }

    /// The typed arms of a bare value. A leaf is typed by [`Self::infer_kind`],
    /// or as a BYTE when it is boolean by language semantics (a comparison,
    /// `!x`) or the boolean `checked` state of a form control. A conditional
    /// whose arms agree collapses to one leaf. Any untyped arm gives `None`.
    fn scalar_tree(&self, expression: &Expression<'a>, depth: usize) -> Option<ScalarTree> {
        if depth > 8 {
            return None;
        }
        let expression = expression.without_parentheses();
        if let Expression::ConditionalExpression(conditional) = expression {
            let then = self.scalar_tree(&conditional.consequent, depth + 1)?;
            let other = self.scalar_tree(&conditional.alternate, depth + 1)?;
            return Some(ScalarTree::join(
                ChoiceKind::Conditional,
                expression.span(),
                self.slice(conditional.test.span()),
                self.slice(expression.span()),
                then,
                other,
            ));
        }
        let boolean = match expression {
            Expression::BinaryExpression(binary) => matches!(
                binary.operator,
                BinaryOperator::Equality
                    | BinaryOperator::Inequality
                    | BinaryOperator::StrictEquality
                    | BinaryOperator::StrictInequality
                    | BinaryOperator::LessThan
                    | BinaryOperator::LessEqualThan
                    | BinaryOperator::GreaterThan
                    | BinaryOperator::GreaterEqualThan
                    | BinaryOperator::In
                    | BinaryOperator::Instanceof
            ),
            Expression::UnaryExpression(unary) => unary.operator == UnaryOperator::LogicalNot,
            Expression::StaticMemberExpression(member) => member.property.name == "checked",
            _ => false,
        };
        let kind = if boolean { Some(LeafKind::Byte) } else { self.infer_kind(expression) }?;
        Some(ScalarTree::Leaf {
            kind,
            value: self.slice(expression.span()),
            span: expression.span(),
        })
    }

    /// The structural fields of an object literal, with each field's value text
    /// and its source-proven type (when there is one).
    fn object_fields(&self, expression: &Expression<'a>) -> Option<Vec<ObjectField>> {
        let Expression::ObjectExpression(object) = expression.without_parentheses() else {
            return None;
        };
        let mut fields = Vec::new();
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                return None;
            };
            let key = property_key_name(&property.key)?;
            let kind = self
                .infer_kind(&property.value)
                .or_else(|| protocol_field_kind(&key));
            fields.push(ObjectField {
                key,
                value: Some(self.slice(property.value.span())),
                kind,
                span: property.value.span(),
            });
        }
        Some(fields)
    }

    /// The result type of an expression, when it is provable from source with
    /// standard-library semantics. Anything unprovable is `None`; a caller must
    /// not substitute a guessed type.
    fn infer_kind(&self, expression: &Expression<'a>) -> Option<LeafKind> {
        match expression.without_parentheses() {
            Expression::StringLiteral(_) | Expression::TemplateLiteral(_) => Some(LeafKind::String),
            Expression::NumericLiteral(_) => Some(LeafKind::Number),
            // A boolean flag is one encoded byte, not a varint number.
            Expression::BooleanLiteral(_) => Some(LeafKind::Byte),
            Expression::UnaryExpression(unary) => match unary.operator {
                UnaryOperator::UnaryNegation | UnaryOperator::UnaryPlus => {
                    self.infer_kind(&unary.argument)
                }
                _ => None,
            },
            Expression::BinaryExpression(binary) if binary.operator == BinaryOperator::Addition => {
                let left = self.infer_kind(&binary.left);
                let right = self.infer_kind(&binary.right);
                if left == Some(LeafKind::String) || right == Some(LeafKind::String) {
                    Some(LeafKind::String)
                } else if left == Some(LeafKind::Number) && right == Some(LeafKind::Number) {
                    Some(LeafKind::Number)
                } else {
                    None
                }
            }
            Expression::ConditionalExpression(conditional) => {
                let consequent = self.infer_kind(&conditional.consequent);
                let alternate = self.infer_kind(&conditional.alternate);
                if consequent.is_some() && consequent == alternate {
                    consequent
                } else {
                    None
                }
            }
            Expression::CallExpression(call) => self.infer_call_kind(call),
            // `parts[k]` of a `text.split("sep")` result: a string element.
            Expression::ComputedMemberExpression(member)
                if matches!(member.expression.without_parentheses(), Expression::NumericLiteral(_))
                    && expression_symbol(&member.object, self.scoping)
                        .is_some_and(|symbol| self.split_results.contains(&symbol)) =>
            {
                Some(LeafKind::String)
            }
            Expression::StaticMemberExpression(_) | Expression::ComputedMemberExpression(_) => {
                let property = member_property(expression)?;
                string_valued_property(&property).or_else(|| number_valued_property(&property))
            }
            _ => None,
        }
    }

    fn infer_call_kind(&self, call: &CallExpression<'a>) -> Option<LeafKind> {
        if self.is_hash_callee(&call.callee) {
            return Some(LeafKind::Number);
        }
        let property = member_property(&call.callee)?;
        string_returning_call(&property)
    }

    fn is_hash_callee(&self, callee: &Expression<'a>) -> bool {
        let Some(symbol) = expression_symbol(callee, self.scoping) else {
            return false;
        };
        self.resolves_to_helper(symbol, self.helpers.hash)
    }

    /// Follows a chain of plain identifier aliases (`x = y`) program-wide.
    /// True when this call emits the exact parameter the launcher forwards from
    /// a VM worker, with no locally built payload. Its siblings in the same
    /// collector are unaffected.
    fn emission_is_forwarded(&self, call: &CallExpression<'a>) -> bool {
        let Some(parameter) = self.forwarded_parameter else {
            return false;
        };
        let payload = if let Some((_, payload)) = self.extracted_next_call(call) {
            Some(payload)
        } else if let Some(EmissionCall::Static { payload, .. }) = next_emission(call) {
            Some(payload)
        } else {
            None
        };
        // The value is the worker's reply, so its own emission carries no local
        // wire whether or not the alias table can name a buffer for it.
        payload.is_some_and(|payload| {
            let Expression::Identifier(identifier) = payload.without_parentheses() else {
                return false;
            };
            identifier.name == parameter
        })
    }

    /// Records the two values a worker function needs for its reply: the
    /// `postMessage` global (called directly or through an alias) and a reply
    /// object whose `[[], buffer]` property names the emitted buffer.
    fn record_worker_value(&mut self, symbol: SymbolId, value: &Expression<'a>) {
        match value.without_parentheses() {
            Expression::Identifier(identifier) if identifier.name == "postMessage" => {
                self.worker_post.insert(symbol);
            }
            Expression::ArrayExpression(_) => {
                if let Some(buffer) = self.array_payload_buffer(value) {
                    self.worker_replies.insert(symbol, buffer);
                }
            }
            Expression::ObjectExpression(_) => {
                if let Some(buffer) = self.object_reply_buffer(value) {
                    self.worker_replies.insert(symbol, buffer);
                }
            }
            _ => {}
        }
    }

    /// The buffer a `postMessage(...)` call delivers: either `[[], buffer]`
    /// directly, a reply object with such a property, or a binding holding one.
    fn worker_reply_buffer(&self, call: &CallExpression<'a>) -> Option<SymbolId> {
        let callee = call.callee.without_parentheses();
        let is_post = match callee {
            Expression::Identifier(identifier) => {
                identifier.name == "postMessage"
                    || resolve_reference(identifier, self.scoping)
                        .is_some_and(|symbol| self.worker_post.contains(&symbol))
            }
            _ => false,
        };
        if !is_post {
            return None;
        }
        let argument = call.arguments.first()?.as_expression()?.without_parentheses();
        match argument {
            Expression::ArrayExpression(_) => self.array_payload_buffer(argument),
            Expression::ObjectExpression(_) => self.object_reply_buffer(argument),
            Expression::Identifier(identifier) => resolve_reference(identifier, self.scoping)
                .and_then(|symbol| self.worker_replies.get(&symbol).copied())
                .map(|buffer| self.resolve_buffer_alias(buffer)),
            _ => None,
        }
    }

    fn object_reply_buffer(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        let Expression::ObjectExpression(object) = expression.without_parentheses() else {
            return None;
        };
        object.properties.iter().find_map(|property| {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                return None;
            };
            // `{a: t, b: [[], buffer]}`, or `{a: t, b: p}` with `p = [[],
            // buffer]` bound earlier (a reply literal built after its payload).
            match property.value.without_parentheses() {
                Expression::Identifier(identifier) => resolve_reference(identifier, self.scoping)
                    .and_then(|symbol| self.worker_replies.get(&symbol).copied())
                    .map(|buffer| self.resolve_buffer_alias(buffer)),
                value => self.array_payload_buffer(value),
            }
        })
    }

    /// The binding's current write version; `0` before its first recorded write.
    fn version_of(&self, symbol: SymbolId) -> u32 {
        self.versions.get(&symbol).copied().unwrap_or(0)
    }

    fn bump_version(&mut self, symbol: SymbolId) {
        self.register_draws.remove(&symbol);
        *self.versions.entry(symbol).or_insert(0) += 1;
    }

    /// The expression a plain local read holds at this point, following plain
    /// identifier copies a bounded number of hops. A non-identifier expression
    /// is returned as its own source text.
    fn resolve_value(&self, expression: &Expression<'a>, depth: usize) -> Option<String> {
        if depth > 6 {
            return None;
        }
        let expression = expression.without_parentheses();
        if let Expression::Identifier(_) = expression {
            if let Some(symbol) = expression_symbol(expression, self.scoping) {
                // A binding that is one of the proven encoder helpers is named
                // by its role, never by its generated spelling: `$KEY_21(...)`
                // reads `hash(...)`.
                if let Some(role) = self.helper_role(symbol) {
                    return Some(role.to_string());
                }
                if let Some(value) = self.resolve_symbol(symbol, depth) {
                    return Some(value);
                }
            }
            return Some(self.slice(expression.span()));
        }
        if let Expression::StaticMemberExpression(member) = expression {
            // A collector context parameter proxies the host: `<ctx>.global`,
            // `<ctx>.Math`, `<ctx>.document` are the host objects. When the
            // object resolves to an unresolved local/parameter and the property
            // is a host root, the property is the value.
            let object = self
                .resolve_value(&member.object, depth + 1)
                .unwrap_or_else(|| self.slice(member.object.span()));
            let property = member.property.name.as_str();
            if is_host_root_name(property) && object.starts_with('$') {
                return Some(property.to_string());
            }
            return Some(format!("{object}.{property}"));
        }
        if let Expression::ComputedMemberExpression(member) = expression {
            let object = self
                .resolve_value(&member.object, depth + 1)
                .unwrap_or_else(|| self.slice(member.object.span()));
            let index = self
                .resolve_value(&member.expression, depth + 1)
                .unwrap_or_else(|| self.slice(member.expression.span()));
            return Some(format!("{object}[{index}]"));
        }
        if let Expression::CallExpression(call) = expression {
            let callee = self
                .resolve_value(&call.callee, depth + 1)
                .unwrap_or_else(|| self.slice(call.callee.span()));
            let mut arguments = Vec::with_capacity(call.arguments.len());
            for argument in &call.arguments {
                let argument = argument.as_expression()?;
                arguments.push(
                    self.resolve_value(argument, depth + 1)
                        .unwrap_or_else(|| self.slice(argument.span())),
                );
            }
            return Some(format!("{callee}({})", arguments.join(", ")));
        }
        Some(self.slice(expression.span()))
    }


    /// The proven encoder role of a binding, if it is (or aliases) one of the
    /// helper roles the extractor already identified.
    fn helper_role(&self, symbol: SymbolId) -> Option<&'static str> {
        if self
            .helpers
            .hash
            .is_some_and(|hash| self.resolves_to_any_helper(symbol, &[hash]))
        {
            Some("hash")
        } else if self.resolves_to_any_helper(symbol, &self.helpers.number_writer) {
            Some("writeNumber")
        } else if self.resolves_to_any_helper(symbol, &self.helpers.string_writer) {
            Some("writeString")
        } else if self.resolves_to_any_helper(symbol, &self.helpers.prng_factory) {
            Some("prng")
        } else {
            None
        }
    }

    fn resolve_symbol(&self, symbol: SymbolId, depth: usize) -> Option<String> {
        if depth > 4 {
            return None;
        }
        let binding = self.values.get(&symbol)?;
        match binding.alias {
            Some(alias) => self.resolve_symbol(alias, depth + 1),
            None => Some(binding.text.clone()),
        }
    }

    fn resolves_to(&self, symbol: SymbolId) -> SymbolId {
        follow_aliases(self.aliases, symbol)
    }

    /// True when `symbol`, after following direct value aliases, names the same
    /// binding as `helper` (also alias-followed). Typed writes and masks survive
    /// renaming because identity is the resolved `SymbolId`, never the spelling.
    fn resolves_to_helper(&self, symbol: SymbolId, helper: Option<SymbolId>) -> bool {
        helper.is_some_and(|helper| self.resolves_to(symbol) == self.resolves_to(helper))
    }

    /// True when `symbol`, after following direct value aliases, names **any**
    /// of the helper bindings proven for one role. A capture that traces two
    /// threads carries one copy of each encoder helper per thread; each copy is
    /// its own binding, so a use site matches whichever copy was in its scope.
    fn resolves_to_any_helper(&self, symbol: SymbolId, helpers: &[SymbolId]) -> bool {
        let target = self.resolves_to(symbol);
        helpers
            .iter()
            .any(|helper| self.resolves_to(*helper) == target)
    }

    fn loop_header(&self, start: u32, body: &Statement<'a>) -> String {
        self.slice(Span::new(start, body.span().start))
    }

    fn for_control(&self, for_statement: &ForStatement<'a>) -> LoopControl {
        LoopControl {
            kind: LoopKind::For,
            init: for_statement
                .init
                .as_ref()
                .map(|init| self.slice(init.span())),
            test: for_statement
                .test
                .as_ref()
                .map(|test| self.slice(test.span())),
            update: for_statement
                .update
                .as_ref()
                .map(|update| self.slice(update.span())),
            count: self.closed_for_count(for_statement),
            source: self.loop_header(for_statement.span.start, &for_statement.body),
            direction: loop_direction(
                for_statement.test.as_ref(),
                for_statement.update.as_ref(),
                statement_list(&for_statement.body).iter(),
                self.scoping,
            ),
            span: for_statement.span,
        }
    }

    /// The iteration count of a `for` loop, only when its AST proves a plain
    /// counted loop with literal bounds and an unmodified unit-step counter.
    fn closed_for_count(&self, for_statement: &ForStatement<'a>) -> Option<u64> {
        let (counter, start) = self.for_counter_symbol(for_statement.init.as_ref()?)?;
        let test = for_statement.test.as_ref()?;
        let (bound, inclusive, direction) = self.counter_bound(test, counter)?;
        let update = for_statement.update.as_ref()?;
        if !self.counter_steps_by_one(update, counter, direction) {
            return None;
        }
        // A write from the body or a captured function invalidates the bound.
        if self
            .scoping
            .get_resolved_references(counter)
            .any(|reference| {
                let span = self.nodes.get_node(reference.node_id()).span();
                reference.is_write()
                    && !(update.span().start <= span.start && span.end <= update.span().end)
            })
        {
            return None;
        }
        struct Abrupt(bool);
        impl<'a> Visit<'a> for Abrupt {
            fn visit_break_statement(&mut self, _: &BreakStatement<'a>) {
                self.0 = true;
            }
            fn visit_continue_statement(&mut self, _: &ContinueStatement<'a>) {
                self.0 = true;
            }
            fn visit_return_statement(&mut self, _: &ReturnStatement<'a>) {
                self.0 = true;
            }
            fn visit_throw_statement(&mut self, _: &ThrowStatement<'a>) {
                self.0 = true;
            }
            fn visit_with_statement(&mut self, _: &WithStatement<'a>) {
                self.0 = true;
            }
            fn visit_call_expression(&mut self, c: &CallExpression<'a>) {
                if matches!(c.callee.without_parentheses(),Expression::Identifier(i) if i.name=="eval")
                {
                    self.0 = true;
                }
                walk::walk_call_expression(self, c);
            }
            fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
            fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
        }
        let mut abrupt = Abrupt(false);
        abrupt.visit_statement(&for_statement.body);
        if abrupt.0 {
            return None;
        }
        let distance = if direction == LoopDirection::Ascending {
            i128::from(bound) - i128::from(start)
        } else {
            i128::from(start) - i128::from(bound)
        };
        u64::try_from((distance + i128::from(inclusive)).max(0)).ok()
    }

    fn for_counter_symbol(&self, init: &ForStatementInit<'a>) -> Option<(SymbolId, i64)> {
        let ForStatementInit::VariableDeclaration(declaration) = init else {
            return None;
        };
        if declaration.kind == VariableDeclarationKind::Const {
            return None;
        }
        if declaration.declarations.len() != 1 {
            return None;
        }
        let declarator = &declaration.declarations[0];
        let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
            return None;
        };
        let symbol = identifier.symbol_id.get()?;
        let value = numeric_value(declarator.init.as_ref()?)?;
        (value.is_finite() && value.fract() == 0.0 && value.abs() <= 9_007_199_254_740_990.0)
            .then_some((symbol, value as i64))
    }

    fn counter_bound(
        &self,
        test: &Expression<'a>,
        counter: SymbolId,
    ) -> Option<(i64, bool, LoopDirection)> {
        let Expression::BinaryExpression(binary) = test.without_parentheses() else {
            return None;
        };
        let (inclusive, direction) = match binary.operator {
            BinaryOperator::LessThan => (false, LoopDirection::Ascending),
            BinaryOperator::LessEqualThan => (true, LoopDirection::Ascending),
            BinaryOperator::GreaterThan => (false, LoopDirection::Descending),
            BinaryOperator::GreaterEqualThan => (true, LoopDirection::Descending),
            _ => return None,
        };
        if expression_symbol(&binary.left, self.scoping)? != counter {
            return None;
        }
        let bound = numeric_value(&binary.right)?;
        if !bound.is_finite() || bound.fract() != 0.0 || bound.abs() > 9_007_199_254_740_990.0 {
            return None;
        }
        Some((bound as i64, inclusive, direction))
    }

    fn counter_steps_by_one(
        &self,
        update: &Expression<'a>,
        counter: SymbolId,
        direction: LoopDirection,
    ) -> bool {
        let ascending = direction == LoopDirection::Ascending;
        match update.without_parentheses() {
            Expression::UpdateExpression(update) => {
                update.operator
                    == if ascending {
                        UpdateOperator::Increment
                    } else {
                        UpdateOperator::Decrement
                    }
                    && matches!(&update.argument,
                        SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier)
                            if resolve_reference(identifier, self.scoping) == Some(counter))
            }
            Expression::AssignmentExpression(assignment) => match assignment.operator {
                AssignmentOperator::Addition | AssignmentOperator::Subtraction => {
                    let target = identifier_target(&assignment.left, self.scoping);
                    target == Some(counter)
                        && numeric_value(&assignment.right) == Some(1.0)
                        && (assignment.operator == AssignmentOperator::Addition) == ascending
                }
                AssignmentOperator::Assign => {
                    if identifier_target(&assignment.left, self.scoping) != Some(counter) {
                        return false;
                    }
                    let Expression::BinaryExpression(binary) =
                        assignment.right.without_parentheses()
                    else {
                        return false;
                    };
                    binary.operator
                        == if ascending {
                            BinaryOperator::Addition
                        } else {
                            BinaryOperator::Subtraction
                        }
                        && expression_symbol(&binary.left, self.scoping) == Some(counter)
                        && numeric_value(&binary.right) == Some(1.0)
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn slice(&self, span: Span) -> String {
        self.source
            .get(span.start as usize..span.end as usize)
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    fn name(&self, symbol: SymbolId) -> Option<String> {
        if let Some((name, _)) = self.virtual_names.get(&symbol) {
            return Some(name.clone());
        }
        Some(self.scoping.symbol_name(symbol).to_string())
    }

    /// The declaration span of a buffer: its binding's, or a virtual
    /// buffer's literal.
    fn buffer_span(&self, symbol: SymbolId) -> Span {
        match self.virtual_names.get(&symbol) {
            Some((_, span)) => *span,
            None => self.scoping.symbol_span(symbol),
        }
    }

    /// The virtual buffer of the empty literal at `literal` held as element
    /// `position` of the sealed tuple `tuple`.
    fn virtual_buffer(&mut self, tuple: SymbolId, position: usize, literal: Span) -> SymbolId {
        if let Some(buffer) = self.virtual_buffers.get(&literal.start) {
            return *buffer;
        }
        let buffer = SymbolId::from_usize(self.scoping.symbols_len() + self.virtual_buffers.len());
        self.virtual_buffers.insert(literal.start, buffer);
        let label = format!("{}[{position}]", self.scoping.symbol_name(tuple));
        self.virtual_names.insert(buffer, (label, literal));
        buffer
    }

    /// The seed word `t[k]` held when the tuple `t` was made.
    fn tuple_seed(&self, expression: &Expression<'a>) -> Option<SymbolId> {
        let Expression::ComputedMemberExpression(member) = expression.without_parentheses() else {
            return None;
        };
        let tuple = expression_symbol(&member.object, self.scoping)?;
        let index = numeric_value(&member.expression)?;
        if index < 0.0 || index.fract() != 0.0 {
            return None;
        }
        *self.tuple_seeds.get(&tuple)?.get(index as usize)?
    }
}

/// The render order proven by per-leaf provenance.
///
/// A derived collector rebuilds a signal by reading the value it was handed at a
/// fixed permutation of elements (`$AC[3]`, `$AC[0]`, ...). [`Leaf::source`]
/// records each such read; this walks the reconstructed writes and returns the
/// elements in write order, but only when *every* write reads the same collected
/// binding and at least two fields were written.
///
/// A computed field, an unknown write or a choice between mutually exclusive
/// arms leaves the order unproven: an arm's writes are alternatives, not one
/// linear permutation, so a choice is never flattened into a tuple. The prefix
/// seed is excluded by the caller (it is the collector's own fresh seed), not by
/// this walk.
fn wire_order(wire: &WireOp) -> Option<Vec<u32>> {
    fn walk(wire: &WireOp, binding: &mut Option<String>, order: &mut Vec<u32>) -> bool {
        match wire {
            WireOp::Sequence(items) => items.iter().all(|item| walk(item, binding, order)),
            WireOp::Choice(_) => false,
            WireOp::Repeat { body, .. } => walk(body, binding, order),
            WireOp::Leaf(leaf) => {
                let Some(source) = leaf.source.as_ref() else {
                    return false;
                };
                match binding {
                    Some(name) if name != &source.binding => false,
                    Some(_) => {
                        order.push(source.element);
                        true
                    }
                    None => {
                        *binding = Some(source.binding.clone());
                        order.push(source.element);
                        true
                    }
                }
            }
            WireOp::Unknown { .. } => false,
        }
    }

    let mut binding = None;
    let mut order = Vec::new();
    if !walk(wire, &mut binding, &mut order) {
        return None;
    }
    (order.len() >= 2).then_some(order)
}

/// Keep only paths reaching a continuation. For loop histories, `continue`
/// reaches the update/test tail; return/throw/break never reach the back edge.
fn reachable_ops(
    ops: &[IOp],
    mut next: Option<Vec<IOp>>,
    continue_tail: Option<Vec<IOp>>,
) -> Option<Vec<IOp>> {
    fn has_exit(ops: &[IOp]) -> bool {
        ops.iter().any(|op| match op {
            IOp::Exit(_) => true,
            IOp::Seq(children) | IOp::Choice(_, children) => has_exit(children),
            _ => false,
        })
    }
    if !has_exit(ops) {
        return next.map(|suffix| [ops.to_vec(), suffix].concat());
    }
    for op in ops.iter().rev() {
        match op {
            IOp::Exit(ControlExit::Continue) => next = continue_tail.clone(),
            IOp::Exit(_) => next = None,
            IOp::Seq(children) => next = reachable_ops(children, next, continue_tail.clone()),
            IOp::Choice(_, branches) if !has_exit(branches) => {
                if let Some(suffix) = &mut next {
                    suffix.insert(0, op.clone());
                }
            }
            IOp::Choice(site, branches) => {
                let mut kept = Vec::new();
                let mut kept_site = site.clone();
                kept_site.labels.clear();
                kept_site.buffer_aliases.clear();
                for (i, branch) in branches.iter().enumerate() {
                    if let Some(ops) = reachable_ops(
                        std::slice::from_ref(branch),
                        next.clone(),
                        continue_tail.clone(),
                    ) {
                        kept.push(IOp::Seq(ops));
                        kept_site
                            .labels
                            .push(site.labels.get(i).cloned().unwrap_or_default());
                        if let Some(aliases) = site.buffer_aliases.get(i) {
                            kept_site.buffer_aliases.push(aliases.clone());
                        }
                    }
                }
                next = (!kept.is_empty()).then(|| vec![IOp::Choice(kept_site, kept)]);
            }
            _ => {
                if let Some(suffix) = &mut next {
                    suffix.insert(0, op.clone());
                }
            }
        }
    }
    next
}

/// The other names that hold `buffer`'s array at the end of every arm of a
/// choice whose arms each recorded their alias map.
fn choice_co_names(site: &ChoiceSite, arms: usize, buffer: SymbolId) -> Vec<SymbolId> {
    if arms < 2 || site.buffer_aliases.len() != arms {
        return Vec::new();
    }
    let mut names: BTreeSet<SymbolId> = BTreeSet::new();
    for aliases in &site.buffer_aliases {
        for (key, value) in aliases {
            names.insert(*key);
            names.insert(*value);
        }
    }
    names
        .into_iter()
        .filter(|name| *name != buffer)
        .filter(|name| {
            site.buffer_aliases
                .iter()
                .all(|aliases| resolve_alias_map(aliases, *name) == resolve_alias_map(aliases, buffer))
        })
        .collect()
}

fn resolve_alias_map(aliases: &BTreeMap<SymbolId, SymbolId>, mut symbol: SymbolId) -> SymbolId {
    for _ in 0..16 {
        match aliases.get(&symbol) {
            Some(&next) if next != symbol => symbol = next,
            _ => break,
        }
    }
    symbol
}

fn collect_branch_buffer_roots(ops: &[IOp], buffer: SymbolId, roots: &mut BTreeSet<SymbolId>) {
    for op in ops {
        match op {
            IOp::Seq(children) => collect_branch_buffer_roots(children, buffer, roots),
            IOp::Choice(site, branches) => {
                for (index, branch) in branches.iter().enumerate() {
                    let target = site
                        .buffer_aliases
                        .get(index)
                        .map_or(buffer, |aliases| resolve_alias_map(aliases, buffer));
                    roots.insert(target);
                    collect_branch_buffer_roots(std::slice::from_ref(branch), target, roots);
                }
            }
            IOp::Repeat { body, .. } => {
                collect_branch_buffer_roots(std::slice::from_ref(body.as_ref()), buffer, roots)
            }
            _ => {}
        }
    }
}

fn sequence(mut items: Vec<GrammarNode>) -> GrammarNode {
    if items.len() == 1 {
        return items.pop().unwrap_or_else(GrammarNode::empty);
    }
    GrammarNode::Sequence { items }
}

/// The same wire item written by two copies of one statement: equal kind,
/// value text, mask and nested structure; spans and positions (the cursor
/// register of each copy) are ignored. Prefixes, opaque and unresolved nodes
/// never match.
fn same_wire_item(left: &GrammarNode, right: &GrammarNode) -> bool {
    let write = |a: &WriteNode, b: &WriteNode| {
        a.value == b.value
            && a.mask == b.mask
            && a.mask_source == b.mask_source
            && a.xor_literal == b.xor_literal
            && a.via_helper.is_some() == b.via_helper.is_some()
    };
    match (left, right) {
        (GrammarNode::Number(a), GrammarNode::Number(b))
        | (GrammarNode::String(a), GrammarNode::String(b))
        | (GrammarNode::Byte(a), GrammarNode::Byte(b)) => write(a, b),
        (GrammarNode::Sequence { items: a }, GrammarNode::Sequence { items: b }) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same_wire_item(a, b))
        }
        (
            GrammarNode::Choice { kind: ka, condition: ca, arms: a, .. },
            GrammarNode::Choice { kind: kb, condition: cb, arms: b, .. },
        ) => {
            ka == kb
                && ca == cb
                && a.len() == b.len()
                && a.iter().zip(b).all(|(a, b)| a.label == b.label && same_wire_item(&a.body, &b.body))
        }
        (
            GrammarNode::Repeat { loop_kind: la, direction: da, header: ha, count: ca, body: a, .. },
            GrammarNode::Repeat { loop_kind: lb, direction: db, header: hb, count: cb, body: b, .. },
        ) => la == lb && da == db && ha == hb && ca == cb && same_wire_item(a, b),
        _ => false,
    }
}

/// The emission's top-level items with every top-level choice's common arm
/// suffix ([`split_common_suffix`]) written after it; a hoisted suffix is
/// itself top-level and is processed the same way.
fn hoist_common_suffixes(items: Vec<GrammarNode>) -> Vec<GrammarNode> {
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        match item {
            GrammarNode::Choice { kind, span, condition, mut arms } => {
                let suffix = split_common_suffix(&mut arms);
                if suffix.is_empty() || !arms.iter().all(|arm| arm.body.is_empty()) {
                    out.push(GrammarNode::Choice { kind, span, condition, arms });
                }
                out.extend(hoist_common_suffixes(suffix));
            }
            other => out.push(other),
        }
    }
    out
}

/// `if (c) { A; X } else { B; X }`: the trailing items every arm ends with
/// (a continuation the printer copied into each arm) are written after the
/// choice, once. Returns them in order and leaves the arms without them.
fn split_common_suffix(arms: &mut [ChoiceArm]) -> Vec<GrammarNode> {
    let items = |body: &GrammarNode| -> Vec<GrammarNode> {
        match body {
            GrammarNode::Sequence { items } => items.clone(),
            other => vec![other.clone()],
        }
    };
    if arms.len() < 2 {
        return Vec::new();
    }
    let mut lists: Vec<Vec<GrammarNode>> = arms.iter().map(|arm| items(&arm.body)).collect();
    let mut suffix = Vec::new();
    loop {
        let Some(last) = lists[0].last() else { break };
        if !lists.iter().all(|list| list.last().is_some_and(|item| same_wire_item(item, last))) {
            break;
        }
        suffix.push(last.clone());
        for list in &mut lists {
            list.pop();
        }
    }
    if suffix.is_empty() {
        return suffix;
    }
    suffix.reverse();
    for (arm, list) in arms.iter_mut().zip(lists) {
        arm.body = sequence(list);
    }
    suffix
}

/// Four consecutive stores of raw prefix bytes 0..=3 of one seed word become
/// one `RawPrefix` with the `IndexStores` spelling. Anything short of the full
/// ordered set stays visible as unresolved prefix stores.
fn group_prefix_stores(items: Vec<GrammarNode>) -> Vec<GrammarNode> {
    let element = |node: &GrammarNode| match node {
        GrammarNode::Unresolved { reason, .. } => reason
            .strip_prefix("prefix_store:")
            .and_then(|index| index.parse::<u32>().ok()),
        _ => None,
    };
    let mut out: Vec<GrammarNode> = Vec::with_capacity(items.len());
    let mut index = 0;
    while index < items.len() {
        // A short prefix literal (`[w >>> 24]`) completed by index stores of
        // the remaining prefix bytes is one prefix.
        if let Some(GrammarNode::RawPrefix(prefix)) = out.last_mut() {
            let have = prefix.elements.len() as u32;
            if have < 4 && element(&items[index]) == Some(have) {
                if let GrammarNode::Unresolved { span, text, .. } = &items[index] {
                    prefix.elements.push(text.clone());
                    prefix.span = Span::new(prefix.span.start.min(span.start), span.end);
                    prefix.spelling = PrefixSpelling::IndexStores;
                }
                index += 1;
                continue;
            }
        }
        let run = items[index..]
            .iter()
            .take(4)
            .map(element)
            .collect::<Vec<_>>();
        if run == [Some(0), Some(1), Some(2), Some(3)] {
            let span_of = |node: &GrammarNode| match node {
                GrammarNode::Unresolved { span, .. } => *span,
                _ => Span::default(),
            };
            let text_of = |node: &GrammarNode| match node {
                GrammarNode::Unresolved { text, .. } => text.clone(),
                _ => String::new(),
            };
            out.push(GrammarNode::RawPrefix(PrefixNode {
                seed_binding: None,
                span: Span::new(span_of(&items[index]).start, span_of(&items[index + 3]).end),
                spelling: PrefixSpelling::IndexStores,
                seed_word: String::new(),
                elements: items[index..index + 4].iter().map(text_of).collect(),
            }));
            index += 4;
        } else {
            out.push(items[index].clone());
            index += 1;
        }
    }
    out
}

fn collect_buffer_writes(ops: &[IOp], buffer: SymbolId, scope: u32, out: &mut Vec<(u32, Span, String)>) {
    for op in ops {
        match op {
            IOp::Seq(children) | IOp::Choice(_, children) => {
                collect_buffer_writes(children, buffer, scope, out)
            }
            IOp::Repeat { body, .. } => {
                collect_buffer_writes(std::slice::from_ref(body), buffer, scope, out)
            }
            IOp::Leaf(leaf) if leaf.buffer == Some(buffer) => out.push((
                scope,
                leaf.helper_call.unwrap_or(leaf.span),
                leaf.value.clone().unwrap_or_default(),
            )),
            IOp::Unknown {
                span,
                text,
                buffer: Some(written),
                ..
            } if *written == buffer => out.push((scope, *span, text.clone())),
            IOp::Opaque {
                span,
                callee,
                buffer: passed,
                ..
            } if *passed == buffer => out.push((scope, *span, callee.clone())),
            IOp::Unsupported { .. } => {}
            _ => {}
        }
    }
}

fn ops_mention_buffer(ops: &[IOp], buffer: SymbolId) -> bool {
    ops.iter().any(|op| match op {
        IOp::Exit(_) => false,
        IOp::Seq(children) => ops_mention_buffer(children, buffer),
        IOp::Choice(_, branches) => branches
            .iter()
            .any(|branch| ops_mention_buffer(std::slice::from_ref(branch), buffer)),
        IOp::Repeat { body, .. } => ops_mention_buffer(std::slice::from_ref(body), buffer),
        IOp::Leaf(leaf) => leaf.buffer == Some(buffer),
        IOp::Prefix { .. } => false,
        IOp::Opaque { buffer: passed, .. } => *passed == buffer,
        IOp::Unknown {
            buffer: unknown, ..
        } => *unknown == Some(buffer),
        IOp::Unsupported { buffers, .. } => buffers.contains(&buffer),
    })
}

/// Every buffer binding an operation list can write or hand to an unmodelled
/// call. Used to bound an `Unsupported` boundary to the buffers it can really
/// affect, instead of degrading every grammar on the same path.
fn collect_op_buffers(ops: &[IOp], out: &mut BTreeSet<SymbolId>) {
    for op in ops {
        match op {
            IOp::Exit(_) => {}
            IOp::Seq(children) => collect_op_buffers(children, out),
            IOp::Choice(_, branches) => {
                for branch in branches {
                    collect_op_buffers(std::slice::from_ref(branch), out);
                }
            }
            IOp::Repeat { body, .. } => collect_op_buffers(std::slice::from_ref(body), out),
            IOp::Leaf(leaf) => {
                if let Some(buffer) = leaf.buffer {
                    out.insert(buffer);
                }
            }
            IOp::Prefix { buffer, .. } => {
                out.insert(*buffer);
            }
            IOp::Opaque { buffer, .. } => {
                out.insert(*buffer);
            }
            IOp::Unknown {
                buffer: Some(buffer),
                ..
            } => {
                out.insert(*buffer);
            }
            IOp::Unknown { buffer: None, .. } => {}
            IOp::Unsupported { buffers, .. } => out.extend(buffers.iter().copied()),
        }
    }
}

/// One write's identity in a grammar: its own span and, for a write made
/// inside a program helper, the caller's helper-call span (the helper body's
/// writer span repeats per call site).
type WriteSite = ((u32, u32), Option<(u32, u32)>);

fn write_site(span: Span, via_helper: Option<(u32, u32)>) -> WriteSite {
    ((span.start, span.end), via_helper)
}

/// The value writes an emission grammar holds.
fn grammar_write_sites(grammar: &GrammarNode) -> BTreeSet<WriteSite> {
    let mut sites = BTreeSet::new();
    grammar.walk(&mut |node| {
        if let GrammarNode::Number(write) | GrammarNode::String(write) | GrammarNode::Byte(write) =
            node
        {
            sites.insert(write_site(
                write.span,
                write.via_helper.map(|via| (via.start, via.end)),
            ));
        }
    });
    sites
}

/// Two mask bindings name the same stream kind: the same factory over a seed
/// word XORed with the same salt. Each arm may draw its own seed word (its own
/// `Math.random()` binding) and make its own factory call; the salt is what
/// the message mask claims.
fn same_mask_stream(left: &MaskInfo, right: &MaskInfo) -> bool {
    let identical = left.factory == right.factory
        && left.seed_word == right.seed_word
        && left.xor_literal == right.xor_literal
        && left.xor_source == right.xor_source
        && left.xor_span == right.xor_span
        && left.argument == right.argument;
    identical
        || (left.factory == right.factory
            && left.xor_source.is_some()
            && left.xor_literal == right.xor_literal
            && left.xor_source == right.xor_source)
}

fn collect_masks<'a>(
    ops: &'a [IOp],
    writes: &BTreeSet<WriteSite>,
    mask: &mut Option<&'a MaskInfo>,
    xor_literal: &mut Option<f64>,
    xor_source: &mut Option<String>,
    xor_span: &mut Span,
) {
    for op in ops {
        match op {
            IOp::Exit(_) => {}
            IOp::Seq(children) => {
                collect_masks(children, writes, mask, xor_literal, xor_source, xor_span)
            }
            IOp::Choice(_, branches) => {
                for branch in branches {
                    collect_masks(
                        std::slice::from_ref(branch),
                        writes,
                        mask,
                        xor_literal,
                        xor_source,
                        xor_span,
                    );
                }
            }
            IOp::Repeat { body, .. } => collect_masks(
                std::slice::from_ref(body),
                writes,
                mask,
                xor_literal,
                xor_source,
                xor_span,
            ),
            IOp::Leaf(leaf) => {
                if writes.contains(&write_site(
                    leaf.span,
                    leaf.helper_call.map(|via| (via.start, via.end)),
                )) {
                    // The mask in force at the emission is the one on the last
                    // write that preceded it on the path; every write keeps its
                    // own per-write mask/xor on its own leaf.
                    if let Some(candidate) = leaf.mask.as_ref() {
                        *mask = Some(candidate);
                    }
                    if leaf.xor_literal.is_some() {
                        *xor_literal = leaf.xor_literal;
                    }
                    if leaf.xor_source.is_some() {
                        *xor_source = leaf.xor_source.clone();
                        *xor_span = leaf.xor_span;
                    }
                }
            }
            IOp::Unknown { .. }
            | IOp::Prefix { .. }
            | IOp::Opaque { .. }
            | IOp::Unsupported { .. } => {}
        }
    }
}

// ===========================================================================
// Structural helpers
// ===========================================================================

fn resolve_reference(identifier: &IdentifierReference<'_>, scoping: &Scoping) -> Option<SymbolId> {
    identifier
        .reference_id
        .get()
        .and_then(|reference| scoping.get_reference(reference).symbol_id())
}

/// Follows a chain of direct value aliases (`x = y`, `var x = y`) a bounded
/// number of hops, stopping at a repeated symbol.
fn follow_aliases(aliases: &BTreeMap<SymbolId, SymbolId>, symbol: SymbolId) -> SymbolId {
    let mut current = symbol;
    for _ in 0..16 {
        match aliases.get(&current) {
            Some(&next) if next != current => current = next,
            _ => break,
        }
    }
    current
}

fn expression_symbol(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    let Expression::Identifier(identifier) = expression.without_parentheses() else {
        return None;
    };
    resolve_reference(identifier, scoping)
}

/// A statement that runs no code besides operators on locals and literals and
/// writes only resolved locals other than `register`: `let a = b | 0;`,
/// `a = a | c << 5;`. Member accesses (getters), calls, unresolved globals
/// (global-object accessors), `in`/`instanceof` (traps, `Symbol.hasInstance`)
/// and `delete` are refused; as elsewhere in the scanner, operator coercion
/// of a local is treated as side-effect free.
fn inert_statement(statement: &Statement<'_>, register: SymbolId, scoping: &Scoping) -> bool {
    match statement {
        Statement::ExpressionStatement(statement) => match statement.expression.without_parentheses() {
            Expression::AssignmentExpression(assignment) => {
                identifier_target(&assignment.left, scoping).is_some_and(|target| target != register)
                    && inert_expression(&assignment.right, scoping)
            }
            _ => false,
        },
        Statement::VariableDeclaration(declaration) => declaration.declarations.iter().all(|declarator| {
            matches!(declarator.id, BindingPattern::BindingIdentifier(_))
                && declarator
                    .init
                    .as_ref()
                    .is_none_or(|init| inert_expression(init, scoping))
        }),
        _ => false,
    }
}

fn inert_expression(expression: &Expression<'_>, scoping: &Scoping) -> bool {
    match expression.without_parentheses() {
        Expression::Identifier(_) => expression_symbol(expression, scoping).is_some(),
        Expression::NumericLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::StringLiteral(_) => true,
        Expression::UnaryExpression(unary) => {
            unary.operator != UnaryOperator::Delete && inert_expression(&unary.argument, scoping)
        }
        Expression::BinaryExpression(binary) => {
            !matches!(binary.operator, BinaryOperator::In | BinaryOperator::Instanceof)
                && inert_expression(&binary.left, scoping)
                && inert_expression(&binary.right, scoping)
        }
        Expression::LogicalExpression(logical) => {
            inert_expression(&logical.left, scoping) && inert_expression(&logical.right, scoping)
        }
        Expression::ConditionalExpression(conditional) => {
            inert_expression(&conditional.test, scoping)
                && inert_expression(&conditional.consequent, scoping)
                && inert_expression(&conditional.alternate, scoping)
        }
        _ => false,
    }
}

fn identifier_target(target: &AssignmentTarget<'_>, scoping: &Scoping) -> Option<SymbolId> {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
            resolve_reference(identifier, scoping)
        }
        _ => None,
    }
}

fn is_arguments(expression: &Expression<'_>, scoping: &Scoping) -> bool {
    matches!(expression.without_parentheses(), Expression::Identifier(identifier)
        if identifier.name == "arguments" && resolve_reference(identifier, scoping).is_none())
}

fn numeric_value(expression: &Expression<'_>) -> Option<f64> {
    match expression.without_parentheses() {
        Expression::NumericLiteral(literal) => Some(literal.value),
        Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::UnaryNegation => {
            numeric_value(&unary.argument).map(|value| -value)
        }
        _ => None,
    }
}

/// A non-negative integer observer slot index, rejecting fractional, negative,
/// or out-of-range values instead of silently truncating them.
fn observer_index(expression: &Expression<'_>) -> Option<u32> {
    let value = numeric_value(expression)?;
    if value < 0.0 || value.fract() != 0.0 || value > u32::MAX as f64 {
        return None;
    }
    Some(value as u32)
}

fn parse_index(text: &str) -> Option<i64> {
    text.trim().parse::<i64>().ok()
}

/// `String.prototype`-shaped methods and standard calls whose return value is a
/// string. Number-returning members (`charCodeAt`, `indexOf`, `length`, ...) are
/// deliberately absent.
fn string_returning_call(property: &str) -> Option<LeafKind> {
    const STRING_METHODS: &[&str] = &[
        "toString",
        "toLocaleString",
        "join",
        "slice",
        "substr",
        "substring",
        "toUpperCase",
        "toLocaleUpperCase",
        "toLowerCase",
        "toLocaleLowerCase",
        "trim",
        "trimStart",
        "trimEnd",
        "concat",
        "replace",
        "replaceAll",
        "padStart",
        "padEnd",
        "repeat",
        "normalize",
        "charAt",
        "getAttribute",
        "getAttributeNS",
    ];
    STRING_METHODS
        .contains(&property)
        .then_some(LeafKind::String)
}

/// Standard member properties whose value is a string: the `location` fields and
/// the DOM text/data properties.
fn string_valued_property(property: &str) -> Option<LeafKind> {
    const STRING_PROPERTIES: &[&str] = &[
        "href",
        "pathname",
        "hostname",
        "host",
        "origin",
        "protocol",
        "search",
        "hash",
        "port",
        "outerText",
        "innerText",
        "textContent",
        "nodeValue",
        "data",
        "className",
        "tagName",
        "localName",
        "namespaceURI",
        "outerHTML",
        "innerHTML",
        "baseURI",
    ];
    STRING_PROPERTIES
        .contains(&property)
        .then_some(LeafKind::String)
}

/// Standard member properties whose value is a number: the DOM viewport and
/// screen measurements (WebIDL `double`/`long`).
fn number_valued_property(property: &str) -> Option<LeafKind> {
    const NUMBER_PROPERTIES: &[&str] = &[
        "innerWidth",
        "innerHeight",
        "outerWidth",
        "outerHeight",
        "availWidth",
        "availHeight",
        "availLeft",
        "availTop",
        "screenX",
        "screenY",
        "scrollX",
        "scrollY",
        "devicePixelRatio",
    ];
    NUMBER_PROPERTIES
        .contains(&property)
        .then_some(LeafKind::Number)
}

/// The field types of the two raw-object protocols this extractor recognizes
/// by their fixed source keys, never by a capture-local binding:
///
/// * the `_selectMeta` accessor probe (`threads::runtime::select_meta_setter`)
///   hands four boolean flags;
/// * the viewport probe reads DOM measurements, which are numbers.
///
/// A key outside these protocols stays untyped.
fn protocol_field_kind(key: &str) -> Option<LeafKind> {
    const SELECT_META_FLAGS: &[&str] = &[
        "selectMeta",
        "animationStart",
        "measurementDiv",
        "animationStyle",
    ];
    const VIEWPORT_MEASUREMENTS: &[&str] = &[
        "innerWidth",
        "innerHeight",
        "outerHeight",
        "outerWidth",
        "screenWidth",
        "screenHeight",
        "screenAvailWidth",
        "screenAvailHeight",
        "visualViewportWidth",
        "visualViewportHeight",
        "visualViewportScale",
    ];
    if SELECT_META_FLAGS.contains(&key) {
        Some(LeafKind::Byte)
    } else if VIEWPORT_MEASUREMENTS.contains(&key) {
        Some(LeafKind::Number)
    } else {
        None
    }
}

fn is_math_random_call(expression: &Expression<'_>, scoping: &Scoping) -> bool {
    let Expression::CallExpression(call) = expression.without_parentheses() else {
        return false;
    };
    if !call.arguments.is_empty() {
        return false;
    }
    match call.callee.without_parentheses() {
        Expression::StaticMemberExpression(member) => {
            member.property.name == "random" && is_global_named(&member.object, "Math", scoping)
        }
        Expression::ComputedMemberExpression(member) => {
            matches!(&member.expression, Expression::StringLiteral(value) if value.value == "random")
                && is_global_named(&member.object, "Math", scoping)
        }
        _ => false,
    }
}

/// Standard-library calls that only read their arguments.
fn is_pure_builtin_call(callee: &Expression<'_>, scoping: &Scoping) -> bool {
    match callee.without_parentheses() {
        Expression::Identifier(identifier) => {
            resolve_reference(identifier, scoping).is_none()
                && matches!(
                    identifier.name.as_str(),
                    "Number"
                        | "String"
                        | "Boolean"
                        | "parseInt"
                        | "parseFloat"
                        | "isNaN"
                        | "isFinite"
                        | "encodeURI"
                        | "decodeURI"
                        | "encodeURIComponent"
                        | "decodeURIComponent"
                )
        }
        Expression::StaticMemberExpression(member) => {
            let Expression::Identifier(object) = member.object.without_parentheses() else {
                return false;
            };
            if resolve_reference(object, scoping).is_some() {
                return false;
            }
            match object.name.as_str() {
                "Math" => true,
                "JSON" => matches!(member.property.name.as_str(), "stringify" | "parse"),
                "Array" => member.property.name == "isArray",
                "Object" => matches!(
                    member.property.name.as_str(),
                    "keys"
                        | "values"
                        | "entries"
                        | "getOwnPropertyNames"
                        | "getOwnPropertyDescriptor"
                        | "getOwnPropertyDescriptors"
                        | "getPrototypeOf"
                ),
                _ => false,
            }
        }
        _ => false,
    }
}


/// Host roots a collector context proxies (`ctx.global`, `ctx.Math`, ...).
fn is_host_root_name(name: &str) -> bool {
    matches!(
        name,
        "global"
            | "window"
            | "self"
            | "document"
            | "navigator"
            | "location"
            | "history"
            | "screen"
            | "Math"
            | "Date"
            | "Intl"
            | "JSON"
            | "Object"
            | "Array"
            | "String"
            | "Number"
            | "RegExp"
            | "performance"
            | "visualViewport"
            | "CustomEvent"
            | "Function"
            | "Promise"
            | "Error"
    )
}

fn is_global_named(expression: &Expression<'_>, name: &str, scoping: &Scoping) -> bool {
    matches!(expression.without_parentheses(), Expression::Identifier(identifier)
        if identifier.name == name && resolve_reference(identifier, scoping).is_none())
}

fn is_seed_word(expression: &Expression<'_>, scoping: &Scoping) -> bool {
    match expression.without_parentheses() {
        Expression::BinaryExpression(binary) => match binary.operator {
            BinaryOperator::Multiplication => {
                is_math_random_call(&binary.left, scoping)
                    && numeric_value(&binary.right) == Some(4_294_967_296.0)
            }
            BinaryOperator::BitwiseOR => is_seed_word(&binary.left, scoping),
            _ => false,
        },
        _ => false,
    }
}

/// The static XOR operand of `left ^ literal` / `literal ^ right`, with the
/// operand's exact source text and span. Returns all-`None`/default when the
/// expression is not a XOR against a source literal.
fn xor_evidence(expression: &Expression<'_>, source: &str) -> (Option<f64>, Option<String>, Span) {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return (None, None, Span::default());
    };
    if binary.operator != BinaryOperator::BitwiseXOR {
        return (None, None, Span::default());
    }
    for operand in [&binary.right, &binary.left] {
        if let Some(value) = numeric_value(operand) {
            let span = operand.span();
            return (
                Some(value),
                Some(source[span.start as usize..span.end as usize].to_string()),
                span,
            );
        }
    }
    (None, None, Span::default())
}

fn xor_literal(expression: &Expression<'_>) -> Option<f64> {
    match expression.without_parentheses() {
        Expression::BinaryExpression(binary) if binary.operator == BinaryOperator::BitwiseXOR => {
            numeric_value(&binary.right)
        }
        Expression::ParenthesizedExpression(paren) => xor_literal(&paren.expression),
        _ => None,
    }
}

fn xor_left_text(expression: &Expression<'_>, source: &str) -> Option<String> {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return None;
    };
    if binary.operator != BinaryOperator::BitwiseXOR {
        return None;
    }
    source
        .get(binary.left.span().start as usize..binary.left.span().end as usize)
        .map(|text| text.trim().to_string())
}

/// The mask closure symbol of a `prng() ^ literal` byte write.
fn byte_write_mask(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return None;
    };
    if binary.operator != BinaryOperator::BitwiseXOR {
        return None;
    }
    for operand in [&binary.left, &binary.right] {
        if let Some(symbol) = mask_call_symbol(operand, scoping) {
            return Some(symbol);
        }
    }
    for operand in [&binary.left, &binary.right] {
        if let Expression::CallExpression(call) = operand.without_parentheses() {
            if let Some(symbol) = expression_symbol(&call.callee, scoping) {
                return Some(symbol);
            }
        }
    }
    None
}

/// The binding called by a stream draw: `m()` or `m.call(void 0)` with no
/// other argument.
fn mask_call_symbol(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    let Expression::CallExpression(call) = expression.without_parentheses() else {
        return None;
    };
    if call.arguments.is_empty() {
        return expression_symbol(&call.callee, scoping);
    }
    if member_property(&call.callee).as_deref() != Some("call") || call.arguments.len() != 1 {
        return None;
    }
    let receiver = call.arguments[0].as_expression()?;
    let undefined_receiver = matches!(receiver.without_parentheses(),
        Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::Void);
    if !undefined_receiver {
        return None;
    }
    expression_symbol(member_object(&call.callee)?, scoping)
}

/// How one indexed byte store combines its value with a mask.
enum ByteMask {
    /// `value ^ m()` (either operand order): `m` is a stream draw.
    Stream { value: Span, mask: Span },
    /// `value ^ literal` with no stream draw.
    StaticXor { value: Span },
    None,
}

/// The drawn function of a zero-argument draw: `f()`, or `f.call(void 0)`
/// (the receiver-less spelling of the same call).
fn receiverless_draw_callee<'n, 'a>(draw: &'n CallExpression<'a>) -> Option<&'n Expression<'a>> {
    if draw.arguments.is_empty() {
        return Some(&draw.callee);
    }
    match draw.callee.without_parentheses() {
        Expression::StaticMemberExpression(call)
            if call.property.name == "call"
                && matches!(draw.arguments.as_slice(), [Argument::UnaryExpression(unary)]
                    if unary.operator == UnaryOperator::Void && numeric_value(&unary.argument).is_some()) =>
        {
            Some(&call.object)
        }
        _ => None,
    }
}

fn split_byte_mask(
    expression: &Expression<'_>,
    masks: &BTreeMap<SymbolId, MaskInfo>,
    scoping: &Scoping,
) -> ByteMask {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return ByteMask::None;
    };
    if binary.operator != BinaryOperator::BitwiseXOR {
        return ByteMask::None;
    }
    let pairs = [(&binary.right, &binary.left), (&binary.left, &binary.right)];
    // A draw from a known stream binding wins; any other zero-argument draw is
    // still a stream mask, with its stream unresolved.
    for known in [true, false] {
        for (mask, value) in pairs {
            if let Some(symbol) = mask_call_symbol(mask, scoping) {
                if !known || masks.contains_key(&symbol) {
                    return ByteMask::Stream {
                        value: value.span(),
                        mask: mask.span(),
                    };
                }
            }
        }
    }
    for (mask, value) in pairs {
        if let Expression::CallExpression(draw) = mask.without_parentheses() {
            // `t[k].call(void 0)` is the receiver-less draw `t[k]()`.
            let (callee, arguments_empty) = match receiverless_draw_callee(draw) {
                Some(callee) => (callee, true),
                None => (&draw.callee, false),
            };
            // `prng(seed)()` inline, or `t[k]()` with `t` the tracer's
            // argument tuple holding the mask binding.
            let tuple_draw = matches!(
                callee.without_parentheses(),
                Expression::ComputedMemberExpression(member)
                    if numeric_value(&member.expression).is_some()
                        && matches!(member.object.without_parentheses(), Expression::Identifier(_))
            );
            if arguments_empty
                && (tuple_draw
                    || matches!(draw.callee.without_parentheses(), Expression::CallExpression(_)))
            {
                return ByteMask::Stream {
                    value: value.span(),
                    mask: mask.span(),
                };
            }
        }
    }
    for (literal, value) in pairs {
        if numeric_value(literal).is_some() {
            return ByteMask::StaticXor {
                value: value.span(),
            };
        }
    }
    ByteMask::None
}

/// The counter direction of a loop: the single counter named by the loop test
/// (or by a leading `if (…) break;` guard of a `for (;;)` body) and updated by
/// the loop update or a top-level statement of the body. A coercion such as
/// `i = Number(i)` is not an update. More than one candidate direction, or
/// none, is `Unknown`.
fn loop_direction<'a, 's, I>(
    test: Option<&Expression<'a>>,
    update: Option<&Expression<'a>>,
    statements: I,
    scoping: &Scoping,
) -> LoopDirection
where
    'a: 's,
    I: IntoIterator<Item = &'s Statement<'a>>,
{
    let statements: Vec<&Statement<'_>> = statements.into_iter().collect();
    let aliases = collect_counter_aliases(statements.iter().copied(), scoping);
    let mut counters = BTreeSet::new();
    if let Some(test) = test {
        let mut symbols = BTreeSet::new();
        collect_symbols(test, scoping, &mut symbols);
        counters.extend(symbols.into_iter().map(|symbol| canonical_counter(symbol, &aliases)));
    }
    for statement in &statements {
        if let Statement::IfStatement(guard) = statement {
            let breaks = match &guard.consequent {
                Statement::BreakStatement(_) => true,
                Statement::BlockStatement(block) => {
                    matches!(block.body.as_slice(), [Statement::BreakStatement(_)])
                }
                _ => false,
            };
            if breaks && guard.alternate.is_none() {
                let mut symbols = BTreeSet::new();
                collect_symbols(&guard.test, scoping, &mut symbols);
                counters.extend(symbols.into_iter().map(|symbol| canonical_counter(symbol, &aliases)));
            }
        }
    }
    let mut found = BTreeSet::new();
    if let Some(update) = update {
        collect_counter_updates(update, scoping, &aliases, &mut found);
    }
    if let Some(test) = test {
        collect_counter_updates(test, scoping, &aliases, &mut found);
    }
    for statement in &statements {
        collect_body_counter_updates(statement, scoping, &aliases, &mut found);
    }
    let directions: BTreeSet<LoopDirection> = found
        .into_iter()
        .filter_map(|(symbol, direction)| {
            let symbol = canonical_counter(symbol, &aliases);
            counters.contains(&symbol).then_some((symbol, direction))
        })
        .map(|(_, direction)| direction)
        .collect();
    match directions.len() {
        1 => directions.into_iter().next().unwrap_or_default(),
        _ => LoopDirection::Unknown,
    }
}

fn statement_list<'n, 'a>(body: &'n Statement<'a>) -> &'n [Statement<'a>] {
    match body {
        Statement::BlockStatement(block) => &block.body,
        other => std::slice::from_ref(other),
    }
}

/// The parts of `for (;;) { if (t) { exit } body… }` or
/// `for (;;) { if (t) { body } else { exit } }`.
struct GuardedLoop<'n, 'a> {
    test: &'n Expression<'a>,
    /// True when the loop continues while `test` is false.
    negated: bool,
    body: Vec<&'n Statement<'a>>,
    exit: Vec<&'n Statement<'a>>,
}

/// A statement list that leaves the loop unconditionally at its end.
fn ends_loop(statements: &[Statement<'_>]) -> bool {
    match statements.last() {
        Some(Statement::BreakStatement(statement)) => statement.label.is_none(),
        // A throwing validation before the counter guard is not the normal
        // loop exit: moving it after the repeat made the throw unconditional
        // and erased all fields that reach the emission on the valid path.
        Some(Statement::ReturnStatement(_)) => true,
        _ => false,
    }
}

/// A statement list ending in `continue L` for a label `L` other than the
/// loop's own: it leaves the loop.
fn ends_outer_continue(statements: &[Statement<'_>], own_label: Option<&str>) -> bool {
    match statements.last() {
        Some(Statement::ContinueStatement(statement)) => statement
            .label
            .as_ref()
            .is_some_and(|label| Some(label.name.as_str()) != own_label),
        Some(Statement::BlockStatement(block)) => ends_outer_continue(&block.body, own_label),
        _ => false,
    }
}

fn ends_continue(statements: &[Statement<'_>]) -> bool {
    match statements.last() {
        Some(Statement::ContinueStatement(_)) => true,
        Some(Statement::BlockStatement(block)) => ends_continue(&block.body),
        _ => false,
    }
}

fn without_terminal_continue<'n, 'a>(statements: &'n [Statement<'a>]) -> Vec<&'n Statement<'a>> {
    let end = if ends_continue(statements) { statements.len().saturating_sub(1) } else { statements.len() };
    statements[..end].iter().collect()
}

fn is_lowered_for_in(test: &Expression<'_>, body: &[&Statement<'_>], scoping: &Scoping) -> bool {
    let test = match test.without_parentheses() {
        Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::LogicalNot => {
            unary.argument.without_parentheses()
        }
        other => other,
    };
    let Expression::BinaryExpression(binary) = test else { return false };
    if !matches!(binary.operator, BinaryOperator::LessThan | BinaryOperator::LessEqualThan)
        || !matches!(binary.left.without_parentheses(), Expression::Identifier(_))
        || !matches!(
            binary.right.without_parentheses(),
            Expression::StaticMemberExpression(_) | Expression::ComputedMemberExpression(_)
        )
    {
        return false;
    }
    let aliases = collect_counter_aliases(body.iter().copied(), scoping);
    !aliases.is_empty()
        && loop_direction(Some(test), None, body.iter().copied(), scoping)
            == LoopDirection::Ascending
}

fn collapse_lowered_wrapper(op: &mut IOp) {
    if let IOp::Choice(site, arms) = op {
        if site.kind == ChoiceKind::If && arms.len() == 2 {
            let [then_arm, else_arm] = arms.as_mut_slice() else { return };
            if matches!(else_arm, IOp::Seq(parts) if parts.is_empty()) {
                let replacement = std::mem::replace(then_arm, IOp::Seq(Vec::new()));
                *op = replacement;
                return;
            }
        }
    }
    match op {
        IOp::Exit(_) => {}
        IOp::Seq(parts) => {
            for part in parts {
                collapse_lowered_wrapper(part);
            }
        }
        IOp::Repeat { body, .. } => collapse_lowered_wrapper(body),
        IOp::Choice(_, arms) => {
            for arm in arms {
                collapse_lowered_wrapper(arm);
            }
        }
        IOp::Leaf(_)
        | IOp::Unknown { .. }
        | IOp::Prefix { .. }
        | IOp::Opaque { .. }
        | IOp::Unsupported { .. } => {}
    }
}

fn guarded_loop<'n, 'a>(body: &'n Statement<'a>, own_label: Option<&str>) -> Option<GuardedLoop<'n, 'a>> {
    let statements = statement_list(body);
    for (index, statement) in statements.iter().enumerate() {
        let Statement::IfStatement(guard) = statement else { continue };
        let consequent = statement_list(&guard.consequent);
        let prefix = statements[..index].iter();
        let suffix = &statements[index + 1..];

        // `for (;;) { if (!(c)) { TAIL; continue outer; } … }`: continuing an
        // enclosing loop leaves this one, so TAIL (with its `continue`) runs
        // once after the iterations, on the enclosing loop's path.
        if guard.alternate.is_none() && (ends_loop(consequent) || ends_outer_continue(consequent, own_label)) {
            return Some(GuardedLoop {
                test: &guard.test,
                negated: true,
                body: prefix.chain(suffix.iter()).collect(),
                exit: consequent.iter().collect(),
            });
        }

        if let Some(alternate) = guard.alternate.as_ref().map(statement_list) {
            // `continue <outer label>` leaves this loop: an exit, not a
            // continue of this loop.
            let consequent_outer = ends_outer_continue(consequent, own_label);
            let alternate_outer = ends_outer_continue(alternate, own_label);
            let consequent_continues = ends_continue(consequent) && !consequent_outer;
            let alternate_continues = ends_continue(alternate) && !alternate_outer;
            let consequent_exits = ends_loop(consequent) || consequent_outer;
            let alternate_exits = ends_loop(alternate) || alternate_outer;
            if (consequent_continues && !alternate_continues)
                || (consequent_exits && !alternate_exits)
            {
                let mut loop_body: Vec<_> = prefix.collect();
                loop_body.extend(if consequent_continues {
                    without_terminal_continue(consequent)
                } else {
                    consequent.iter().collect()
                });
                return Some(GuardedLoop {
                    test: &guard.test,
                    negated: false,
                    body: loop_body,
                    exit: alternate.iter().collect(),
                });
            }
            if (alternate_continues && !consequent_continues)
                || (alternate_exits && !consequent_exits)
            {
                let mut loop_body: Vec<_> = prefix.collect();
                loop_body.extend(if alternate_continues {
                    without_terminal_continue(alternate)
                } else {
                    alternate.iter().collect()
                });
                return Some(GuardedLoop {
                    test: &guard.test,
                    negated: true,
                    body: loop_body,
                    exit: consequent.iter().collect(),
                });
            }
        }

        if guard.alternate.is_none() && ends_continue(consequent) && !suffix.is_empty() {
            let mut loop_body: Vec<_> = prefix.collect();
            loop_body.extend(without_terminal_continue(consequent));
            return Some(GuardedLoop {
                test: &guard.test,
                negated: false,
                body: loop_body,
                exit: suffix.iter().collect(),
            });
        }
    }
    None
}

fn collect_symbols(expression: &Expression<'_>, scoping: &Scoping, out: &mut BTreeSet<SymbolId>) {
    match expression.without_parentheses() {
        Expression::Identifier(identifier) => {
            if let Some(symbol) = resolve_reference(identifier, scoping) {
                out.insert(symbol);
            }
        }
        Expression::BinaryExpression(binary) => {
            collect_symbols(&binary.left, scoping, out);
            collect_symbols(&binary.right, scoping, out);
        }
        Expression::LogicalExpression(logical) => {
            collect_symbols(&logical.left, scoping, out);
            collect_symbols(&logical.right, scoping, out);
        }
        Expression::UnaryExpression(unary) => collect_symbols(&unary.argument, scoping, out),
        Expression::UpdateExpression(update) => {
            if let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) = &update.argument {
                if let Some(symbol) = resolve_reference(identifier, scoping) {
                    out.insert(symbol);
                }
            }
        }
        _ => {}
    }
}

fn collect_counter_aliases<'a, 's>(
    statements: impl IntoIterator<Item = &'s Statement<'a>>,
    scoping: &Scoping,
) -> BTreeMap<SymbolId, SymbolId>
where
    'a: 's,
{
    let mut aliases = BTreeMap::new();
    for statement in statements {
        let Statement::VariableDeclaration(declaration) = statement else { continue };
        for declarator in &declaration.declarations {
            let BindingPattern::BindingIdentifier(identifier) = &declarator.id else { continue };
            let Some(left) = identifier.symbol_id.get() else { continue };
            let Some(Expression::Identifier(right)) = declarator.init.as_ref() else { continue };
            if let Some(right) = resolve_reference(right, scoping) {
                aliases.insert(left, right);
            }
        }
    }
    aliases
}

fn canonical_counter(mut symbol: SymbolId, aliases: &BTreeMap<SymbolId, SymbolId>) -> SymbolId {
    let mut seen = BTreeSet::new();
    while let Some(&next) = aliases.get(&symbol) {
        if !seen.insert(symbol) {
            break;
        }
        symbol = next;
    }
    symbol
}

fn collect_body_counter_updates(
    statement: &Statement<'_>,
    scoping: &Scoping,
    aliases: &BTreeMap<SymbolId, SymbolId>,
    out: &mut BTreeSet<(SymbolId, LoopDirection)>,
) {
    match statement {
        Statement::ExpressionStatement(statement) => {
            collect_counter_updates(&statement.expression, scoping, aliases, out);
        }
        Statement::VariableDeclaration(declaration) => {
            for declarator in &declaration.declarations {
                let BindingPattern::BindingIdentifier(identifier) = &declarator.id else { continue };
                let Some(symbol) = identifier.symbol_id.get() else { continue };
                let Some(Expression::BinaryExpression(binary)) = declarator
                    .init
                    .as_ref()
                    .map(|expression| expression.without_parentheses())
                else {
                    continue;
                };
                if numeric_value(&binary.right).filter(|step| *step > 0.0).is_none() {
                    continue;
                }
                let Some(base) = expression_symbol(&binary.left, scoping) else { continue };
                let direction = match binary.operator {
                    BinaryOperator::Addition => LoopDirection::Ascending,
                    BinaryOperator::Subtraction => LoopDirection::Descending,
                    _ => continue,
                };
                out.insert((canonical_counter(symbol, aliases), direction));
                out.insert((canonical_counter(base, aliases), direction));
            }
        }
        Statement::BlockStatement(block) => {
            for statement in &block.body {
                collect_body_counter_updates(statement, scoping, aliases, out);
            }
        }
        Statement::IfStatement(if_statement) => {
            collect_body_counter_updates(&if_statement.consequent, scoping, aliases, out);
            if let Some(alternate) = &if_statement.alternate {
                collect_body_counter_updates(alternate, scoping, aliases, out);
            }
        }
        Statement::LabeledStatement(labeled) => {
            collect_body_counter_updates(&labeled.body, scoping, aliases, out);
        }
        // An outer loop can advance its counter from an inner loop's exit
        // arm. Binding filtering in loop_direction excludes the inner loop's
        // own counters; function bodies remain outside this walk.
        Statement::ForStatement(loop_) => {
            collect_body_counter_updates(&loop_.body, scoping, aliases, out);
            if let Some(update) = &loop_.update {
                collect_counter_updates(update, scoping, aliases, out);
            }
        }
        Statement::WhileStatement(loop_) => {
            collect_body_counter_updates(&loop_.body, scoping, aliases, out);
        }
        Statement::DoWhileStatement(loop_) => {
            collect_body_counter_updates(&loop_.body, scoping, aliases, out);
        }
        _ => {}
    }
}

fn collect_counter_updates(
    expression: &Expression<'_>,
    scoping: &Scoping,
    aliases: &BTreeMap<SymbolId, SymbolId>,
    out: &mut BTreeSet<(SymbolId, LoopDirection)>,
) {
    match expression.without_parentheses() {
        Expression::SequenceExpression(sequence) => {
            for expression in &sequence.expressions {
                collect_counter_updates(expression, scoping, aliases, out);
            }
        }
        Expression::BinaryExpression(binary) => {
            collect_counter_updates(&binary.left, scoping, aliases, out);
            collect_counter_updates(&binary.right, scoping, aliases, out);
        }
        Expression::LogicalExpression(logical) => {
            collect_counter_updates(&logical.left, scoping, aliases, out);
            collect_counter_updates(&logical.right, scoping, aliases, out);
        }
        Expression::UnaryExpression(unary) => {
            collect_counter_updates(&unary.argument, scoping, aliases, out);
        }
        Expression::UpdateExpression(update) => {
            if let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) = &update.argument {
                if let Some(symbol) = resolve_reference(identifier, scoping) {
                    out.insert((
                        canonical_counter(symbol, aliases),
                        match update.operator {
                            UpdateOperator::Increment => LoopDirection::Ascending,
                            UpdateOperator::Decrement => LoopDirection::Descending,
                        },
                    ));
                }
            }
        }
        Expression::AssignmentExpression(assignment) => {
            let Some(symbol) = identifier_target(&assignment.left, scoping) else {
                return;
            };
            let step = |value: &Expression<'_>| numeric_value(value).filter(|step| *step > 0.0);
            let direction = match assignment.operator {
                AssignmentOperator::Addition => step(&assignment.right).map(|_| LoopDirection::Ascending),
                AssignmentOperator::Subtraction => {
                    step(&assignment.right).map(|_| LoopDirection::Descending)
                }
                AssignmentOperator::Assign => match assignment.right.without_parentheses() {
                    Expression::BinaryExpression(binary) if expression_symbol(&binary.left, scoping).is_some() && step(&binary.right).is_some() => {
                        match binary.operator {
                            BinaryOperator::Addition => Some(LoopDirection::Ascending),
                            BinaryOperator::Subtraction => Some(LoopDirection::Descending),
                            _ => None,
                        }
                    }
                    _ => None,
                },
                _ => None,
            };
            if let Some(direction) = direction {
                out.insert((canonical_counter(symbol, aliases), direction));
            }
        }
        _ => {}
    }
}

/// The field name of a computed/static member assignment target (`obj["a"]` or
/// `obj.a`).
fn member_field_name(target: &AssignmentTarget<'_>) -> Option<String> {
    match target {
        AssignmentTarget::StaticMemberExpression(member) => Some(member.property.name.to_string()),
        AssignmentTarget::ComputedMemberExpression(member) => {
            match member.expression.without_parentheses() {
                Expression::StringLiteral(value) => Some(value.value.to_string()),
                _ => None,
            }
        }
        _ => None,
    }
}

/// A recognized `observer.next(payload)` call shape.
enum EmissionCall<'a> {
    /// `next(p)` or `next.call(recv, p)`.
    Static {
        receiver: &'a Expression<'a>,
        payload: &'a Expression<'a>,
    },
    /// `next.apply(recv, args)` whose `args` is not a single static literal
    /// element, so the payload cannot be read from source.
    Dynamic {
        receiver: &'a Expression<'a>,
        array: &'a Expression<'a>,
    },
}

fn next_emission<'a>(call: &'a CallExpression<'a>) -> Option<EmissionCall<'a>> {
    let callee = call.callee.without_parentheses();
    match member_property(callee).as_deref()? {
        "next" => {
            let payload = call.arguments.first()?.as_expression()?;
            Some(EmissionCall::Static {
                receiver: member_object(callee)?,
                payload,
            })
        }
        "call" => {
            let inner = member_object(callee)?;
            if member_property(inner).as_deref() != Some("next") {
                return None;
            }
            let payload = call.arguments.get(1)?.as_expression()?;
            Some(EmissionCall::Static {
                receiver: member_object(inner)?,
                payload,
            })
        }
        "apply" => {
            let inner = member_object(callee)?;
            if member_property(inner).as_deref() != Some("next") {
                return None;
            }
            let receiver = member_object(inner)?;
            let array = call.arguments.get(1)?.as_expression()?;
            match array.without_parentheses() {
                Expression::ArrayExpression(elements) if elements.elements.len() == 1 => {
                    match elements.elements[0].as_expression() {
                        Some(payload) => Some(EmissionCall::Static { receiver, payload }),
                        None => Some(EmissionCall::Dynamic { receiver, array }),
                    }
                }
                _ => Some(EmissionCall::Dynamic { receiver, array }),
            }
        }
        _ => None,
    }
}

/// `receiver.split("separator")`: a `String.prototype.split` with a string
/// separator (standard-library semantics, as for the other string methods).
fn is_string_split_call(expression: &Expression<'_>) -> bool {
    let Expression::CallExpression(call) = expression.without_parentheses() else {
        return false;
    };
    member_property(&call.callee).as_deref() == Some("split")
        && matches!(
            call.arguments.first().and_then(Argument::as_expression),
            Some(Expression::StringLiteral(_))
        )
}

fn member_property(expression: &Expression<'_>) -> Option<String> {
    match expression.without_parentheses() {
        Expression::StaticMemberExpression(member) => Some(member.property.name.to_string()),
        Expression::ComputedMemberExpression(member) => {
            match member.expression.without_parentheses() {
                Expression::StringLiteral(value) => Some(value.value.to_string()),
                _ => None,
            }
        }
        _ => None,
    }
}

fn member_object<'a>(expression: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    match expression.without_parentheses() {
        Expression::StaticMemberExpression(member) => Some(&member.object),
        Expression::ComputedMemberExpression(member) => Some(&member.object),
        _ => None,
    }
}

fn property_key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
        PropertyKey::StringLiteral(value) => Some(value.value.to_string()),
        PropertyKey::NumericLiteral(value) => Some(value.value.to_string()),
        _ => None,
    }
}

/// The collected-value field a derived signal's field was read from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldSource {
    pub binding: String,
    pub element: u32,
}

/// `$AC[3]` -> (`$AC`, 3). Only a plain identifier indexed by a decimal literal
/// counts; anything else is not a provenance claim.
fn scalar_grammar(tree: &ScalarTree) -> GrammarNode {
    match tree {
        ScalarTree::Leaf { kind, value, span } => {
            let write = WriteNode {
                stream_instance: None,
                stream_draw: None,
                position_span: None,
                stream_factory: None,
                string_instance: None,
                writer_factory: None,
                span: *span,
                value_span: *span,
                value: value.clone(),
                resolved: None,
                mask: MaskForm::Unmasked,
                mask_source: None,
                xor_literal: None,
                position: None,
                via_helper: None,
                helper_arguments: Vec::new(),
                provenance: None,
            };
            match kind {
                LeafKind::Number => GrammarNode::Number(write),
                LeafKind::String => GrammarNode::String(write),
                LeafKind::Byte => GrammarNode::Byte(write),
            }
        }
        ScalarTree::Choice { kind, span, condition, then, other } => GrammarNode::Choice {
            kind: *kind,
            span: *span,
            condition: Some(condition.clone()),
            arms: vec![
                ChoiceArm { label: "then".into(), body: scalar_grammar(then) },
                ChoiceArm { label: "else".into(), body: scalar_grammar(other) },
            ],
        },
    }
}

fn scalar_wire(tree: &ScalarTree) -> WireOp {
    match tree {
        ScalarTree::Leaf { kind, value, span } => WireOp::Leaf(Leaf {
            source: None,
            kind: *kind,
            span: *span,
            buffer: None,
            label: None,
            masked: false,
            value: Some(value.clone()),
            index: None,
            xor_literal: None,
            byte_width: None,
        }),
        ScalarTree::Choice { then, other, .. } => WireOp::Choice(vec![scalar_wire(then), scalar_wire(other)]),
    }
}

fn field_source(value: &str) -> Option<FieldSource> {
    let (binding, index) = value.trim().split_once('[')?;
    let element = index.strip_suffix(']')?.parse::<u32>().ok()?;
    let binding = binding.trim();
    if binding.is_empty()
        || !binding
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
    {
        return None;
    }
    Some(FieldSource {
        binding: binding.to_string(),
        element,
    })
}

/// The ops that ran before the current grammar walk position, as a chain of
/// borrowed slices; [`OpHistory::to_vec`] is the flat list the walk used to
/// copy at every nested sequence and choice.
enum OpHistory<'h> {
    Root,
    Then {
        prior: &'h OpHistory<'h>,
        ops: &'h [IOp],
    },
}

impl OpHistory<'_> {
    fn to_vec(&self) -> Vec<IOp> {
        let mut parts = Vec::new();
        let mut current = self;
        while let OpHistory::Then { prior, ops } = current {
            parts.push(*ops);
            current = prior;
        }
        parts.into_iter().rev().flat_map(|ops| ops.iter().cloned()).collect()
    }
}

/// `[k, buffer][1]`: the tracer's inline tuple literal read at a literal
/// index (`[3, b][1][4] = v` stores into `b`) is that element.
fn literal_tuple_element<'e, 'a>(expression: &'e Expression<'a>) -> Option<&'e Expression<'a>> {
    let Expression::ComputedMemberExpression(member) = expression.without_parentheses() else {
        return None;
    };
    let Expression::ArrayExpression(array) = member.object.without_parentheses() else {
        return None;
    };
    let Expression::NumericLiteral(index) = member.expression.without_parentheses() else {
        return None;
    };
    if index.value.fract() != 0.0 || index.value < 0.0 {
        return None;
    }
    if array.elements.iter().any(|element| element.is_spread() || element.is_elision()) {
        return None;
    }
    array.elements.get(index.value as usize)?.as_expression()
}

#[cfg(test)]
mod alias_tests {
    use super::*;

    /// The binding `name` resolves to after following the program's aliases.
    fn resolved(source: &str, name: &str) -> String {
        let allocator = Allocator::default();
        let program = Parser::new(&allocator, source, SourceType::mjs()).parse().program;
        let scoping = SemanticBuilder::new().build(&program).semantic.into_scoping();
        let aliases = AliasCollector::build(&scoping, &program);
        let symbol = scoping.find_binding(scoping.root_scope_id(), name.into()).expect("binding");
        scoping.symbol_name(follow_aliases(&aliases, symbol)).to_string()
    }

    #[test]
    fn stores_after_an_early_return_do_not_replace_a_helper_alias() {
        let source = "function writer(m) {}\n\
            let w = writer;\n\
            function collect() {\n\
              let list = [];\n\
              if (!!list) { w(1); return void 0; }\n\
              for (;;) { w = list; w = list[0]; }\n\
            }\n\
            function emit() { w(2); }\n";
        assert_eq!(resolved(source, "w"), "writer");
    }

    #[test]
    fn a_second_unconditional_store_still_replaces_the_alias() {
        let source = "function writer(m) {}\n\
            let w = writer;\n\
            function collect() { let list = []; w = list; }\n";
        assert_ne!(resolved(source, "w"), "writer");
    }
}
