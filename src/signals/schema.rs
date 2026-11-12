//! Typed generator-input properties for a recovered program capture.
//!
//! # What this module is
//!
//! [`extract_program_properties`] parses the exact source with Oxc, resolves
//! lexical bindings with `oxc_semantic`, and returns a typed, serde-serializable
//! [`ProgramProperties`] report. The report answers the generator's first
//! questions:
//!
//! * which function is the broadest reachable main program scope,
//! * which of its own bindings carry the descriptor/wave/session/helper roles,
//! * what the descriptor registry actually contains, including the population
//!   assignments that override literal entries,
//! * how the wave registry, index rows, routing selector and direct-constructor
//!   slots cover the registry,
//! * how each signal's observer is wired to a wave slot or a direct descriptor,
//! * how the exported session initializer's public arguments map to session
//!   constructor arguments and context fields,
//! * which shared NUMBER/STRING/PRNG/hash/serializer helpers the emission chain
//!   depends on, with their exact original bodies retained.
//!
//! It is not a codec and not a generator. In particular it never invents the
//! per-emission XOR literal, a signal's wire layout, a runtime key, or a random
//! value: those either come from exact source expressions or remain in
//! [`ProgramProperties::unresolved`] with a required-missing-fact marker.
//!
//! # Identity without generated names
//!
//! Every [`BindingRef`] carries a resolved symbol ordinal, the source name as a
//! *label*, the exact declaration span, and the exact identifier span. A
//! transform that must rename a helper can address it by `declaration.start`
//! without any name or ordinal map. A role is assigned only after its structure
//! matches; [`BindingRole`] is a closed enum of canonical role names.
//!
//! # Fail closed
//!
//! Relative counts (how many descriptors, how many waves) are report data and
//! test expectations, never recognition gates. When two structurally plausible
//! registries/helpers remain indistinguishable, extraction returns
//! [`ProgramPropertiesError::Ambiguous`] instead of taking the first candidate.
//! Exceeding a [`ProgramPropertiesLimits`] budget returns
//! [`ProgramPropertiesError::Limit`].

use std::fmt;

use serde::{Deserialize, Serialize};

/// Version of the serialized [`ProgramProperties`] contract.
pub const PROGRAM_PROPERTIES_SCHEMA_VERSION: u32 = 1;

// ===========================================================================
// Errors and limits
// ===========================================================================

/// A bounded extraction budget. Defaults are generous enough for a recovered
/// capture and small enough to reject adversarial input deterministically.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgramPropertiesLimits {
    pub max_source_bytes: usize,
    pub max_ast_nodes: usize,
    pub max_functions: usize,
    pub max_bindings: usize,
    pub max_array_elements: usize,
    pub max_emissions: usize,
    pub max_retained_source_bytes: usize,
}

impl Default for ProgramPropertiesLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 64 * 1024 * 1024,
            max_ast_nodes: 4_000_000,
            max_functions: 262_144,
            max_bindings: 262_144,
            max_array_elements: 65_536,
            max_emissions: 65_536,
            max_retained_source_bytes: 4 * 1024 * 1024,
        }
    }
}

/// A typed extraction failure. Every variant is a refusal, not a partial guess.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProgramPropertiesError {
    /// The caller supplied nothing to read.
    InvalidInput(String),
    /// Oxc rejected the source.
    Parse(String),
    /// The source or an individual structure exceeded a budget.
    Limit { what: &'static str, limit: usize },
    /// More than one structure matched and the source does not decide.
    Ambiguous {
        what: &'static str,
        candidates: Vec<String>,
    },
    /// A structurally required fact is absent.
    Missing { what: &'static str },
}

impl fmt::Display for ProgramPropertiesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(value) => write!(f, "invalid program-properties input: {value}"),
            Self::Parse(value) => write!(f, "program-properties parse failed: {value}"),
            Self::Limit { what, limit } => {
                write!(f, "program-properties limit exceeded for {what} ({limit})")
            }
            Self::Ambiguous { what, candidates } => write!(
                f,
                "program-properties ambiguity for {what}: {} matching candidates",
                candidates.len()
            ),
            Self::Missing { what } => {
                write!(f, "program-properties required fact is missing: {what}")
            }
        }
    }
}

impl std::error::Error for ProgramPropertiesError {}

// ===========================================================================
// Spans, identity, confidence
// ===========================================================================

/// A UTF-8 byte span in the exact input source.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct SourceSpan {
    pub start: u32,
    pub end: u32,
}

impl SourceSpan {
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    pub const fn len(self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    pub const fn is_empty(self) -> bool {
        self.end <= self.start
    }
}

impl From<oxc_span::Span> for SourceSpan {
    fn from(value: oxc_span::Span) -> Self {
        Self {
            start: value.start,
            end: value.end,
        }
    }
}

/// Exact-source identity. The hash is SHA-256 of the supplied bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceIdentity {
    pub sha256: String,
    pub bytes: usize,
}

/// How strongly a fact is established. `High` means the structure and its
/// scope-resolved participants were both proven; report coverage is never
/// upgraded just because a count matched an observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    High,
    Medium,
    Low,
    Unresolved,
}

/// A missing generator-required fact, with the reason it is not established.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnresolvedFact {
    /// Stable machine label of the fact, e.g. `signal_wire_layout`.
    pub fact: String,
    pub reason: String,
    pub span: Option<SourceSpan>,
    pub required: bool,
}

/// A non-fatal diagnostic emitted while matching a structure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub span: Option<SourceSpan>,
}

// ===========================================================================
// Bindings and functions
// ===========================================================================

/// One source binding. The ordinal is `SymbolId::index` for the single parse
/// that produced this report; the declaration and identifier spans are the
/// stable public addresses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingRef {
    pub symbol: u32,
    pub name: String,
    /// Whole declaration node (`VariableDeclarator`, `Function`, parameter, ...).
    pub declaration: SourceSpan,
    /// The declaring identifier itself.
    pub identifier: SourceSpan,
    /// True when the binding is a direct local of the selected main scope.
    pub owned_by_main: bool,
}

/// A closed set of canonical, structure-derived binding roles.
///
/// The role names are intentionally semantic descriptions (`WriteNumber`,
/// `WriteString`, ...). They become available only after the matcher has proven
/// the corresponding structure; an ordinary same-spelled binding stays
/// [`BindingRole::Unknown`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingRole {
    Unknown,
    MainEntry,
    DescriptorRegistry,
    WaveRegistry,
    WaveIndexRows,
    WaveRoutingSelector,
    WaveCollector,
    DirectCollector,
    WriteNumber,
    WriteString,
    CreateSignalMask,
    HashTransactionToken,
    ObserverFactory,
    SnapshotGetter,
    EnvelopeBuilder,
    SerializerFactory,
    DefaultEncoder,
    SessionConstructor,
    ContextConfig,
    LatestValueStore,
    StatusStore,
    DebugNameStore,
    ErrorRingBuffer,
    ErrorRingConstructor,
    SessionInit,
    SessionTeardown,
    DefaultSignalFilter,
    SentinelObject,
    TagTable,
}

impl BindingRole {
    pub const fn is_unknown(self) -> bool {
        matches!(self, Self::Unknown)
    }
}

/// What kind of declaration introduced a [`BindingRecord`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingKind {
    Variable,
    Function,
    Parameter,
    Catch,
    /// The binding a class declaration introduces; its value is the class.
    Class,
    Other,
}

/// A summary of the static value shape of an initializer or expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueKind {
    Literal,
    Array,
    Object,
    Function,
    Identifier,
    Call,
    Other,
}

/// A literal value extracted from source. Numbers keep the source text in the
/// surrounding [`ExprRecord`]; this is only the decoded value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LiteralValue {
    Null,
    Undefined,
    Boolean(bool),
    Number(f64),
    String(String),
    /// An array literal whose every element is itself a source-closed literal.
    /// A hole stays [`LiteralValue::Hole`]; a spread or any dynamic element makes
    /// the whole expression non-literal.
    Array(Vec<LiteralValue>),
    /// An object literal whose every data property is itself source-closed.
    /// Computed keys, methods, getters/setters and spreads make it non-literal.
    Object(std::collections::BTreeMap<String, LiteralValue>),
    /// An elided array element (`[1, , 2]`).
    Hole,
}

/// An exact source expression plus its resolved binding/function participants.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExprRecord {
    pub span: SourceSpan,
    pub source: String,
    pub kind: ValueKind,
    pub literal: Option<LiteralValue>,
    pub binding: Option<BindingRef>,
    pub function: Option<FunctionRef>,
}

/// A typed source function. `binding` is present when the function had a
/// declaration or a variable it was assigned to; anonymous callbacks have none.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FunctionRef {
    pub binding: Option<BindingRef>,
    pub name_label: Option<String>,
    pub span: SourceSpan,
    pub body: Option<SourceSpan>,
    /// Exact original body source, retained for the roles a generator consumes.
    pub source: Option<String>,
}

/// One direct local binding of the selected main scope.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BindingRecord {
    pub id: u32,
    pub name: String,
    pub declaration: SourceSpan,
    pub identifier: SourceSpan,
    pub kind: BindingKind,
    pub role: BindingRole,
    pub references: usize,
    pub writes: usize,
    pub initializer: Option<ExprRecord>,
}

// ===========================================================================
// Helpers
// ===========================================================================

/// A recognized shared helper with its exact source body retained.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HelperRecord {
    pub role: BindingRole,
    pub function: FunctionRef,
    pub body_source: Option<String>,
    pub confidence: Confidence,
    pub notes: Vec<String>,
}

// ===========================================================================
// Descriptors
// ===========================================================================

/// Where a descriptor element came from: the base array literal or a later
/// indexed population assignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DescriptorPopulation {
    ArrayLiteral,
    IndexOverride,
}

/// One populated descriptor slot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DescriptorRecord {
    pub index: usize,
    pub population: DescriptorPopulation,
    pub span: SourceSpan,
    pub ctor: Option<ExprRecord>,
    pub priority: Option<LiteralValue>,
    pub name: Option<LiteralValue>,
    /// The whole object-element source when it is an object literal.
    pub source: String,
}

/// One write that populates/overrides a registry index.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PopulationAssignment {
    pub index: usize,
    pub assignment_span: SourceSpan,
    pub value_span: SourceSpan,
    pub value_source: String,
}

/// The descriptor registry recovered from source.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DescriptorRegistry {
    pub binding: BindingRef,
    pub array_literal_span: Option<SourceSpan>,
    pub length: usize,
    pub elements: Vec<DescriptorRecord>,
    pub population_assignments: Vec<PopulationAssignment>,
}

// ===========================================================================
// Waves and signals
// ===========================================================================

/// One wave: collector function, its descriptor-index row, and selector.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaveRecord {
    pub index: usize,
    pub function: FunctionRef,
    pub signal_indices: Vec<usize>,
    /// `slot -> descriptor index` rows used by the collector's `arguments[0]`.
    pub observer_slots: Vec<ObserverSlot>,
    pub routing_selector: bool,
    pub function_span: SourceSpan,
    pub row_span: SourceSpan,
    pub selector_span: SourceSpan,
}

/// A descriptor index reached through a non-null constructor rather than a wave
/// row. The constructor expression is preserved exactly; its collector body is
/// intentionally left to the per-signal layout pass.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DirectSlot {
    pub signal: usize,
    pub descriptor_index: usize,
    pub ctor: ExprRecord,
    pub ctor_call: Option<FunctionRef>,
    /// The task callback argument passed to the constructor wrapper. This is the
    /// function whose body owns the observer emissions, not the wrapper factory.
    pub callback: Option<FunctionRef>,
    /// True when the resolved task callback completes without ever reading or
    /// calling an observer `next` anywhere in its lexical subtree.
    #[serde(default)]
    pub declared_not_emitted: bool,
    pub unresolved: Vec<String>,
}

/// The wave/registry wiring recovered from source.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WaveRegistry {
    pub function_array: BindingRef,
    pub index_rows: BindingRef,
    pub routing_selector: BindingRef,
    pub waves: Vec<WaveRecord>,
    pub direct_slots: Vec<DirectSlot>,
}

/// Observer position within a collector.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObserverRoute {
    WaveSlot(u32),
    Direct,
}

/// `slot -> signal` mapping for a vector observer list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObserverSlot {
    pub slot: u32,
    pub signal: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmissionKind {
    Next,
    Error,
    Complete,
}

/// One observer emission proven from the collector's own source.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmissionRecord {
    pub kind: EmissionKind,
    pub span: SourceSpan,
    pub observer: ObserverRoute,
    /// The whole first argument expression of the sink call.
    pub payload: Option<ExprRecord>,
    /// For `[[], buffer]` payloads, the emitted byte-buffer expression.
    pub buffer: Option<ExprRecord>,
}

/// Explicit statement that this pass did not prove a signal's wire layout.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireLayoutStatus {
    NotAnalyzed,
    Unresolved(String),
    Partial(String),
    /// The signal is declared by a direct constructor and its task callback is
    /// installed, but that callback never mentions an observer `next` at all
    /// and only completes. The value does not exist in this capture, so this is
    /// not an extraction failure.
    DeclaredNotEmitted(String),
}

/// Registry/observer identity of one signal. The detailed ordered writes,
/// encoder, seed chain and mask literal belong to the per-signal layout pass.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignalRecord {
    pub signal: u32,
    pub descriptor_index: u32,
    pub direct: bool,
    pub wave: Option<u32>,
    pub wave_position: Option<u32>,
    pub collector: Option<FunctionRef>,
    pub observer: ObserverRoute,
    pub emissions: Vec<EmissionRecord>,
    /// Ordered per-emission wire reconstruction produced by [`signals`]. Empty
    /// only when the collector's writes could not be reconstructed;
    /// `wire_layout` then says why.
    pub emission_layouts: Vec<crate::signals::extract::layout::Emission>,
    pub wire_layout: WireLayoutStatus,
    pub unresolved: Vec<String>,
}

// ===========================================================================
// Session and shared inputs
// ===========================================================================

/// Mapping of one session-constructor call argument back to a public init
/// argument. Internal captures deliberately leave `public_init_argument` empty.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionArgumentMapping {
    pub session_argument: usize,
    pub public_init_argument: Option<usize>,
    pub expression: ExprRecord,
}

/// One context/config field with its declaration and live assignment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContextField {
    pub name: String,
    pub declaration_span: SourceSpan,
    pub declared: Option<LiteralValue>,
    pub assigned: Option<ExprRecord>,
    pub session_argument: Option<usize>,
    pub public_init_argument: Option<usize>,
}

/// The descriptor-indexed stores installed by the session constructor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoreWiring {
    pub latest_values: Option<BindingRef>,
    pub statuses: Option<BindingRef>,
    pub debug_names: Option<BindingRef>,
    pub sentinel: Option<ExprRecord>,
    pub sentinel_binding: Option<BindingRef>,
    pub error_buffer: Option<BindingRef>,
}

/// The snapshot getter and the configured envelope/serializer boundary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnvelopeProperties {
    pub builder: FunctionRef,
    pub fields: std::collections::BTreeMap<String, ExprRecord>,
    /// Header name prefix read from the dispatch-detail literal object.
    pub header_prefix: Option<LiteralValue>,
    /// Header chunk size read from the same dispatch-detail literal object.
    pub header_chunk_size: Option<LiteralValue>,
}

/// The serializer factory and its default configuration as wired by the session.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SerializerProperties {
    pub default_encoder: Option<BindingRef>,
    pub factory: Option<FunctionRef>,
    pub factory_arguments: Vec<ExprRecord>,
    pub alphabet_fallback: Option<ExprRecord>,
    pub keysets: Option<ExprRecord>,
}

/// Backward chain from the mask PRNG state to the runtime transaction token.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HashChain {
    pub prng_state: BindingRef,
    pub hash_function: FunctionRef,
    pub hash_input: ExprRecord,
    pub transaction_token_field: Option<String>,
    pub session_argument: Option<usize>,
    pub public_init_argument: Option<usize>,
}

/// The session constructor/context/init contract recovered structurally.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionProperties {
    pub constructor: FunctionRef,
    pub context_binding: Option<BindingRef>,
    pub context_span: Option<SourceSpan>,
    pub context_fields: Vec<ContextField>,
    pub init: Option<FunctionRef>,
    pub teardown: Option<FunctionRef>,
    pub argument_mapping: Vec<SessionArgumentMapping>,
    pub stores: StoreWiring,
    pub default_filter: Option<BindingRef>,
    pub envelope: Option<EnvelopeProperties>,
    pub serializer: Option<SerializerProperties>,
    pub hash_chain: Option<HashChain>,
}

/// One external value the generator must receive at runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalInputKind {
    SessionArgument,
    ContextField,
    HostGlobal,
}

/// An external runtime input. No value is guessed and no random bytes are
/// embedded; the exact source expression is retained.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExternalInput {
    pub kind: ExternalInputKind,
    pub name: String,
    pub source: ExprRecord,
    pub public_init_argument: Option<usize>,
    pub session_argument: Option<usize>,
    pub required: bool,
}

/// A static source constant captured from an emission producer. `source` is
/// exact; changing it in the input changes this record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StaticCapture {
    pub kind: CaptureKind,
    pub span: SourceSpan,
    pub value: f64,
    pub source: String,
    pub collector: Option<FunctionRef>,
    pub role: BindingRole,
}

/// Why a static capture was retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureKind {
    EmissionXorLiteral,
}

/// A pure numeric object literal retained as a constant table (tag map or
/// similar). Entries keep exact decoded values.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConstantTable {
    pub binding: BindingRef,
    pub span: SourceSpan,
    pub entries: std::collections::BTreeMap<String, LiteralValue>,
}

// ===========================================================================
// Program scope and report
// ===========================================================================

/// The structurally discovered main program scope.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgramScope {
    pub main: FunctionRef,
    pub main_argument: Option<String>,
    pub owned_binding_count: usize,
    pub returns_init_object: bool,
    pub top_level_call: Option<SourceSpan>,
    pub host_event_bridge: Option<SourceSpan>,
}

/// Summary counters. These are observations about this input, never gates.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageSummary {
    pub binding_count: usize,
    pub recognized_binding_roles: usize,
    pub descriptor_count: usize,
    pub wave_count: usize,
    pub direct_slot_count: usize,
    pub signal_count: usize,
    pub emission_count: usize,
    pub unresolved_count: usize,
}

/// The complete generator-input property report for one exact capture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgramProperties {
    pub schema_version: u32,
    pub source: SourceIdentity,
    pub program_scope: ProgramScope,
    pub bindings: Vec<BindingRecord>,
    pub helpers: Vec<HelperRecord>,
    pub descriptors: Option<DescriptorRegistry>,
    pub waves: Option<WaveRegistry>,
    pub session: Option<SessionProperties>,
    pub signals: Vec<SignalRecord>,
    /// Observer routes whose collector performs a host side effect (a
    /// `CustomEvent` dispatch) and completes without a payload.
    #[serde(default)]
    pub host_probes: Vec<crate::signals::extract::host_probes::HostProbe>,
    pub constants: Vec<ConstantTable>,
    pub static_captures: Vec<StaticCapture>,
    pub external_inputs: Vec<ExternalInput>,
    pub unresolved: Vec<UnresolvedFact>,
    pub diagnostics: Vec<Diagnostic>,
    pub coverage: CoverageSummary,
}

impl ProgramProperties {
    pub fn helper(&self, role: BindingRole) -> Option<&HelperRecord> {
        self.helpers.iter().find(|helper| helper.role == role)
    }

    pub fn binding_with_role(&self, role: BindingRole) -> Option<&BindingRecord> {
        self.bindings.iter().find(|binding| binding.role == role)
    }

    pub fn signal(&self, signal: u32) -> Option<&SignalRecord> {
        self.signals.iter().find(|record| record.signal == signal)
    }

    /// Generator-required facts that this pass did not prove. The follow-up
    /// signal-layout pass is expected to close these; they must not be treated
    /// as confirmed values.
    pub fn required_missing_facts(&self) -> Vec<&UnresolvedFact> {
        self.unresolved
            .iter()
            .filter(|fact| fact.required)
            .collect()
    }
}

// ===========================================================================
// Per-seed signal schema projection
// ===========================================================================
//
// Projection of a slot's contract grammar into the per-seed signal schema.
//
// The projection is a pure function of the contract: it drops the raw 4-byte
// prefix, turns every value leaf into a typed field, every `REPEAT` into an
// `array`, every `CHOICE` into a `choice`, and a multi-node arm or loop body
// into a `record`. A byte whose value is a literal keeps it as `byte` (the
// 130+ null markers); a byte whose value is `(c ? A : B)` becomes a `choice`
// of two byte arms. A stream-masked leaf inherits the message mask unless its
// stream salt differs (a nested `RAW4` stream, reported as `mask: <salt>`); a
// static XOR operand is kept as `xor`. A named-object emission (generic
// codec) keeps its object keys and types every value as `value`.
//
// Every emission is projected; emissions with the same projected layout are
// one message. A slot with more than one distinct message layout is rendered
// with `messages`; a top-level `CHOICE` whose arms each start their own raw
// prefix is one message per arm (items after the choice belong to every
// arm's message).
//
// Field names are not chosen here. Every nameable field has a positional
// [`FieldKey`]; a binder maps keys to card names ([`FieldBinding`]) and
// [`render_signal`] refuses a layout with an unbound key.

use std::collections::BTreeMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::Expression;
use oxc_parser::Parser;
use oxc_span::SourceType;
use serde_json::{json, Map, Value};

use crate::signals::extract::contracts::{GrammarNode, MaskForm, SignalContract, WriteNode};
use crate::signals::extract::layout::EmissionVariant;
use crate::signals::extract::wiring::LifecycleRole;

/// The positional address of a nameable field inside one message:
/// `f2` (top-level field 2), `f3.item.f1` (field 1 of the record item of
/// array field 3), `f4.then.f2` (field 2 of the record `then` arm of choice
/// field 4). One-based, salt- and spelling-free.
pub type FieldKey = String;

/// The scalar type of a leaf.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeafKind {
    Number,
    String,
    Byte,
    /// A value of a named-object emission, encoded by the generic codec with
    /// every runtime type branch retained.
    Value,
}

impl LeafKind {
    fn as_str(self) -> &'static str {
        match self {
            LeafKind::Number => "number",
            LeafKind::String => "string",
            LeafKind::Byte => "byte",
            LeafKind::Value => "value",
        }
    }
}

/// How a leaf is masked relative to its message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeafMask {
    /// The message's stream (`mask: null`).
    Inherit,
    /// A different stream salt: a nested raw-prefix stream.
    Nested(i64),
    /// Not stream-masked (static XOR only, generic codec value, unmasked
    /// store).
    None,
    /// A mask whose stream could not be resolved.
    Unresolved,
}

/// One value leaf.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutLeaf {
    pub kind: LeafKind,
    pub mask: LeafMask,
    /// The literal written byte (`130` null marker), when the value is one.
    pub byte: Option<i64>,
    /// Static XOR operand applied to the value.
    pub xor: Option<f64>,
    /// The object key of a named-object value.
    pub key: Option<String>,
    /// Host API paths of the value's sources (binder input).
    pub apis: Vec<String>,
    /// The leaf's mask stream salt, as written (comparison input).
    pub salt: Option<i64>,
}

/// One projected layout node.
#[derive(Clone, Debug, PartialEq)]
pub enum LayoutNode {
    Leaf(LayoutLeaf),
    /// `(c ? A : B)` written as one byte: a choice of two literal bytes.
    ByteSelect {
        leaf: LayoutLeaf,
        then_byte: i64,
        else_byte: i64,
    },
    Array {
        item: Box<LayoutNode>,
    },
    Record {
        fields: Vec<LayoutField>,
    },
    Choice {
        arms: Vec<LayoutArm>,
    },
    /// An arm or loop body that writes nothing.
    Empty,
    /// An opaque or unresolved grammar node.
    Unknown {
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutField {
    pub key: FieldKey,
    pub node: LayoutNode,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutArm {
    pub label: String,
    pub node: LayoutNode,
}

/// One distinct message shape.
#[derive(Clone, Debug, PartialEq)]
pub struct MessageLayout {
    /// When the message is sent: the lifecycle phases and arm labels of the
    /// emissions that share this layout.
    pub when: Vec<String>,
    /// The message stream salt (`createMaskStream(seed ^ C)`); `None` for a
    /// named-object emission.
    pub mask: Option<i64>,
    pub fields: Vec<LayoutField>,
}

/// The projection of one emission (before merging equal layouts).
#[derive(Clone, Debug, PartialEq)]
pub struct EmissionLayout {
    pub emission: usize,
    /// One part per message the emission can send: a single unlabeled part,
    /// or one part per arm of a top-level choice whose arms each start a raw
    /// prefix.
    pub parts: Vec<(Option<String>, MessageLayout)>,
    /// Per part, the indices of the emission's value leaves (in evaluation
    /// order, as in `SlotView` leaves) that its fields cover, in field
    /// order. Items after a split choice belong to every part, so parts are
    /// not always consecutive runs.
    pub part_leaves: Vec<Vec<usize>>,
}

impl EmissionLayout {
    /// The emission leaf indices of part `part`.
    pub fn part_leaves(&self, part: usize) -> &[usize] {
        self.part_leaves.get(part).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Every leaf of an emission with `leaves` value leaves belongs to some
    /// part, and each part's fields hold exactly its leaves.
    pub fn covers(&self, leaves: usize) -> bool {
        let sized = self.parts.iter().zip(&self.part_leaves).all(|((_, part), indices)| {
            part.fields.iter().map(|field| layout_leaves(&field.node)).sum::<usize>() == indices.len()
        });
        if !sized || self.parts.len() != self.part_leaves.len() {
            return false;
        }
        let mut seen = vec![false; leaves];
        for index in self.part_leaves.iter().flatten() {
            match seen.get_mut(*index) {
                Some(slot) => *slot = true,
                None => return false,
            }
        }
        seen.into_iter().all(|seen| seen)
    }
}

/// Value leaves under a projected node (a byte select is one leaf).
pub fn layout_leaves(node: &LayoutNode) -> usize {
    match node {
        LayoutNode::Leaf(_) | LayoutNode::ByteSelect { .. } => 1,
        LayoutNode::Array { item } => layout_leaves(item),
        LayoutNode::Record { fields } => fields.iter().map(|field| layout_leaves(&field.node)).sum(),
        LayoutNode::Choice { arms } => arms.iter().map(|arm| layout_leaves(&arm.node)).sum(),
        LayoutNode::Empty | LayoutNode::Unknown { .. } => 0,
    }
}

/// Value leaves (number / string / byte writes) under a grammar node.
fn grammar_leaves(node: &GrammarNode) -> usize {
    let mut count = 0;
    node.walk(&mut |child| {
        if matches!(child, GrammarNode::Number(_) | GrammarNode::String(_) | GrammarNode::Byte(_)) {
            count += 1;
        }
    });
    count
}

/// The projected layout of one slot.
#[derive(Clone, Debug, PartialEq)]
pub struct SignalLayout {
    /// Distinct message layouts, in first-emission order.
    pub messages: Vec<MessageLayout>,
    pub emissions: Vec<EmissionLayout>,
}

impl SignalLayout {
    /// Every nameable key of message `message`, in order.
    pub fn keys(&self, message: usize) -> Vec<FieldKey> {
        let mut out = Vec::new();
        if let Some(layout) = self.messages.get(message) {
            collect_keys(&layout.fields, &mut out);
        }
        out
    }

    /// Top-level field keys of message `message`.
    pub fn top_keys(&self, message: usize) -> Vec<FieldKey> {
        self.messages
            .get(message)
            .map(|layout| layout.fields.iter().map(|field| field.key.clone()).collect())
            .unwrap_or_default()
    }
}

fn collect_keys(fields: &[LayoutField], out: &mut Vec<FieldKey>) {
    for field in fields {
        out.push(field.key.clone());
        collect_node_keys(&field.node, out);
    }
}

fn collect_node_keys(node: &LayoutNode, out: &mut Vec<FieldKey>) {
    match node {
        LayoutNode::Record { fields } => collect_keys(fields, out),
        LayoutNode::Array { item } => collect_node_keys(item, out),
        LayoutNode::Choice { arms } => arms.iter().for_each(|arm| collect_node_keys(&arm.node, out)),
        _ => {}
    }
}

/// Parse the salt of `seed ^ C` (or a bare `C`).
pub(crate) fn parse_salt(text: &str) -> Option<i64> {
    let tail = text.rsplit('^').next()?.trim();
    let tail = tail.trim_end_matches(')').trim();
    if let Some(hex) = tail.strip_prefix("0x").or_else(|| tail.strip_prefix("0X")) {
        return i64::from_str_radix(hex, 16).ok();
    }
    tail.parse::<i64>()
        .ok()
        .or_else(|| tail.parse::<f64>().ok().filter(|v| v.fract() == 0.0).map(|v| v as i64))
}

/// The literal shape of a byte value: `130` or `c ? A : B`.
enum ByteLiteral {
    One(i64),
    Select(i64, i64),
}

fn byte_literal(value: &str) -> Option<ByteLiteral> {
    let allocator = Allocator::default();
    let expression = Parser::new(&allocator, value, SourceType::mjs())
        .parse_expression()
        .ok()?;
    let integer = |expression: &Expression<'_>| match expression.without_parentheses() {
        Expression::NumericLiteral(literal)
            if literal.value.fract() == 0.0 && literal.value >= 0.0 =>
        {
            Some(literal.value as i64)
        }
        _ => None,
    };
    match expression.without_parentheses() {
        Expression::ConditionalExpression(conditional) => Some(ByteLiteral::Select(
            integer(&conditional.consequent)?,
            integer(&conditional.alternate)?,
        )),
        other => integer(other).map(ByteLiteral::One),
    }
}

struct Projector {
    message_salt: Option<i64>,
    generic: bool,
}

impl Projector {
    fn leaf(&self, kind: LeafKind, write: &WriteNode) -> LayoutLeaf {
        let salt = write.mask_source.as_deref().and_then(parse_salt);
        let mask = if self.generic {
            LeafMask::None
        } else {
            match write.mask {
                MaskForm::Stream => match (salt, self.message_salt) {
                    (Some(leaf), Some(message)) if leaf != message => LeafMask::Nested(leaf),
                    (_, Some(_)) => LeafMask::Inherit,
                    (Some(leaf), None) => LeafMask::Nested(leaf),
                    (None, None) => LeafMask::Unresolved,
                },
                MaskForm::StaticXor | MaskForm::Unmasked => LeafMask::None,
                MaskForm::Unresolved => LeafMask::Unresolved,
            }
        };
        let apis = write
            .provenance
            .as_ref()
            .map(|provenance| {
                provenance
                    .sources
                    .iter()
                    .filter_map(|source| source.api.clone())
                    .collect()
            })
            .unwrap_or_default();
        LayoutLeaf {
            kind: if self.generic { LeafKind::Value } else { kind },
            mask,
            byte: None,
            xor: write.xor_literal,
            key: if self.generic { write.position.clone() } else { None },
            apis,
            salt,
        }
    }

    fn node(&self, node: &GrammarNode) -> LayoutNode {
        match node {
            GrammarNode::Number(write) => LayoutNode::Leaf(self.leaf(LeafKind::Number, write)),
            GrammarNode::String(write) => LayoutNode::Leaf(self.leaf(LeafKind::String, write)),
            GrammarNode::Byte(write) => {
                let mut leaf = self.leaf(LeafKind::Byte, write);
                if self.generic {
                    return LayoutNode::Leaf(leaf);
                }
                match byte_literal(&write.value) {
                    Some(ByteLiteral::One(byte)) => {
                        leaf.byte = Some(byte);
                        LayoutNode::Leaf(leaf)
                    }
                    Some(ByteLiteral::Select(then_byte, else_byte)) => LayoutNode::ByteSelect {
                        leaf,
                        then_byte,
                        else_byte,
                    },
                    None => LayoutNode::Leaf(leaf),
                }
            }
            GrammarNode::Sequence { items } => self.body(items),
            GrammarNode::Repeat { body, .. } => LayoutNode::Array {
                item: Box::new(self.node(body)),
            },
            GrammarNode::Choice { arms, .. } => LayoutNode::Choice {
                arms: arms
                    .iter()
                    .map(|arm| LayoutArm {
                        label: arm.label.clone(),
                        node: self.node(&arm.body),
                    })
                    .collect(),
            },
            GrammarNode::RawPrefix(_) => LayoutNode::Empty,
            GrammarNode::Opaque { reason, .. } => LayoutNode::Unknown {
                reason: format!("opaque: {reason}"),
            },
            GrammarNode::Unresolved { reason, .. } => LayoutNode::Unknown {
                reason: format!("unresolved: {reason}"),
            },
        }
    }

    /// A body: nothing -> empty, one node -> that node, more -> a record.
    fn body(&self, items: &[GrammarNode]) -> LayoutNode {
        let nodes: Vec<LayoutNode> = items
            .iter()
            .filter(|item| !matches!(item, GrammarNode::RawPrefix(_)))
            .map(|item| self.node(item))
            .filter(|node| !matches!(node, LayoutNode::Empty))
            .collect();
        match nodes.len() {
            0 => LayoutNode::Empty,
            1 => nodes.into_iter().next().unwrap_or(LayoutNode::Empty),
            _ => LayoutNode::Record {
                fields: nodes
                    .into_iter()
                    .map(|node| LayoutField {
                        key: String::new(),
                        node,
                    })
                    .collect(),
            },
        }
    }

    /// The top-level fields of one message body.
    fn fields(&self, items: &[GrammarNode]) -> Vec<LayoutField> {
        items
            .iter()
            .filter(|item| !matches!(item, GrammarNode::RawPrefix(_)))
            .map(|item| self.node(item))
            .filter(|node| !matches!(node, LayoutNode::Empty))
            .map(|node| LayoutField {
                key: String::new(),
                node,
            })
            .collect()
    }
}

/// Assign positional keys (`f1`, `f3.item.f2`, `f4.then.f1`).
fn assign_keys(fields: &mut [LayoutField], prefix: &str) {
    for (index, field) in fields.iter_mut().enumerate() {
        field.key = format!("{prefix}f{}", index + 1);
        let key = field.key.clone();
        assign_node_keys(&mut field.node, &key);
    }
}

fn assign_node_keys(node: &mut LayoutNode, key: &str) {
    match node {
        LayoutNode::Record { fields } => assign_keys(fields, &format!("{key}.")),
        LayoutNode::Array { item } => assign_node_keys(item, &format!("{key}.item")),
        LayoutNode::Choice { arms } => {
            for (index, arm) in arms.iter_mut().enumerate() {
                let label = arm_key(&arm.label, index);
                assign_node_keys(&mut arm.node, &format!("{key}.{label}"));
            }
        }
        _ => {}
    }
}

/// The key component of a choice arm: its label when it is a plain word
/// (`then`, `else`, `try`, `catch`, `right`, `skip`, `default`), otherwise
/// `arm<N>` (a `case <test>` label carries source text).
fn arm_key(label: &str, index: usize) -> String {
    if !label.is_empty() && label.chars().all(|c| c.is_ascii_alphabetic()) {
        label.to_string()
    } else {
        format!("arm{}", index + 1)
    }
}

fn top_items(grammar: &GrammarNode) -> Vec<GrammarNode> {
    match grammar {
        GrammarNode::Sequence { items } => items.clone(),
        other => vec![other.clone()],
    }
}

fn contains_prefix(node: &GrammarNode) -> bool {
    let mut found = false;
    node.walk(&mut |child| {
        if matches!(child, GrammarNode::RawPrefix(_)) {
            found = true;
        }
    });
    found
}

fn phase_label(phase: Option<LifecycleRole>) -> String {
    match phase {
        None => "body".to_string(),
        Some(role) => serde_json::to_value(role)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| format!("{role:?}")),
    }
}

/// Project every emission of `contract`.
pub fn project_contract(contract: &SignalContract) -> SignalLayout {
    let mut emissions = Vec::new();
    for (index, emission) in contract.emissions.iter().enumerate() {
        let generic = emission.payload == EmissionVariant::GenericObject;
        let message_salt = emission
            .stream
            .as_ref()
            .and_then(|stream| stream.salt.as_deref().and_then(parse_salt));
        let projector = Projector {
            message_salt,
            generic,
        };
        let phase = phase_label(emission.phase);
        let items = top_items(&emission.grammar);
        // A top-level choice whose every arm starts its own raw prefix sends
        // one message per arm; items after the choice (no raw prefix of
        // their own) are written into every arm's message.
        let split = match items.as_slice() {
            [GrammarNode::Choice { arms, .. }, rest @ ..]
                if !generic
                    && arms.iter().all(|arm| contains_prefix(&arm.body))
                    && !rest.iter().any(contains_prefix) =>
            {
                Some((arms.clone(), rest.to_vec()))
            }
            _ => None,
        };
        let (parts, part_leaves) = match split {
            Some((arms, rest)) => {
                let arm_counts: Vec<usize> = arms.iter().map(|arm| grammar_leaves(&arm.body)).collect();
                let arms_total: usize = arm_counts.iter().sum();
                let rest_count: usize = rest.iter().map(grammar_leaves).sum();
                let mut offset = 0;
                let mut parts = Vec::new();
                let mut part_leaves = Vec::new();
                for (arm, count) in arms.iter().zip(&arm_counts) {
                    let mut arm_items = top_items(&arm.body);
                    arm_items.extend(rest.iter().cloned());
                    let mut fields = projector.fields(&arm_items);
                    assign_keys(&mut fields, "");
                    parts.push((
                        Some(arm.label.clone()),
                        MessageLayout {
                            when: vec![format!("{phase}:{}", arm.label)],
                            mask: message_salt,
                            fields,
                        },
                    ));
                    part_leaves.push((offset..offset + count).chain(arms_total..arms_total + rest_count).collect());
                    offset += count;
                }
                (parts, part_leaves)
            }
            None => {
                let mut fields = projector.fields(&items);
                assign_keys(&mut fields, "");
                let count = items.iter().map(grammar_leaves).sum();
                (
                    vec![(
                        None,
                        MessageLayout {
                            when: vec![phase],
                            mask: message_salt,
                            fields,
                        },
                    )],
                    vec![(0..count).collect()],
                )
            }
        };
        emissions.push(EmissionLayout {
            emission: index,
            parts,
            part_leaves,
        });
    }
    let mut messages: Vec<MessageLayout> = Vec::new();
    for emission in &emissions {
        for (_, part) in &emission.parts {
            match messages
                .iter_mut()
                .find(|message| same_layout(message, part))
            {
                Some(message) => {
                    for when in &part.when {
                        if !message.when.contains(when) {
                            message.when.push(when.clone());
                        }
                    }
                }
                None => messages.push(part.clone()),
            }
        }
    }
    SignalLayout {
        messages,
        emissions,
    }
}

/// Equal wire layouts: same mask and the same rendered fields.
fn same_layout(left: &MessageLayout, right: &MessageLayout) -> bool {
    let names = Names {
        name: &|key| Some(key.to_string()),
        flags: &no_flags,
    };
    left.mask == right.mask && render_fields(&left.fields, &names) == render_fields(&right.fields, &names)
}

impl SignalLayout {
    /// The message index the part `part` of emission `emission` was merged
    /// into.
    pub fn part_message(&self, emission: usize, part: usize) -> Option<usize> {
        let (_, layout) = self.emissions.get(emission)?.parts.get(part)?;
        self.messages.iter().position(|message| same_layout(message, layout))
    }

    /// The node of `key` in message `message`.
    pub fn node_of(&self, message: usize, key: &str) -> Option<&LayoutNode> {
        find_key(&self.messages.get(message)?.fields, key).map(|(node, _)| node)
    }
}

/// The node of `key` among `fields` and the offset of its first value leaf
/// in the fields' leaf order (the order of [`EmissionLayout::part_leaves`]).
pub fn find_key<'l>(fields: &'l [LayoutField], key: &str) -> Option<(&'l LayoutNode, usize)> {
    let mut offset = 0;
    for field in fields {
        if field.key == key {
            return Some((&field.node, offset));
        }
        if let Some((node, inner)) = find_node_key(&field.node, key) {
            return Some((node, offset + inner));
        }
        offset += layout_leaves(&field.node);
    }
    None
}

fn find_node_key<'l>(node: &'l LayoutNode, key: &str) -> Option<(&'l LayoutNode, usize)> {
    match node {
        LayoutNode::Record { fields } => find_key(fields, key),
        LayoutNode::Array { item } => find_node_key(item, key),
        LayoutNode::Choice { arms } => {
            let mut offset = 0;
            for arm in arms {
                if let Some((found, inner)) = find_node_key(&arm.node, key) {
                    return Some((found, offset + inner));
                }
                offset += layout_leaves(&arm.node);
            }
            None
        }
        _ => None,
    }
}

/// Offsets (relative to the node's first leaf) of the non-literal byte
/// leaves under `node` that packed flags apply to: through arrays and choice
/// arms, not into records.
pub fn flag_leaf_offsets(node: &LayoutNode) -> Vec<usize> {
    fn walk(node: &LayoutNode, offset: usize, out: &mut Vec<usize>) {
        match node {
            LayoutNode::Leaf(leaf) if leaf.kind == LeafKind::Byte && leaf.byte.is_none() => out.push(offset),
            LayoutNode::Array { item } => walk(item, offset, out),
            LayoutNode::Choice { arms } => {
                let mut offset = offset;
                for arm in arms {
                    walk(&arm.node, offset, out);
                    offset += layout_leaves(&arm.node);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    walk(node, 0, &mut out);
    out
}

// ===========================================================================
// Binding and rendering
// ===========================================================================

/// Card field names for the keys of each message.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FieldBinding {
    names: BTreeMap<(usize, FieldKey), String>,
    /// Packed flag bytes: card flag name -> bit position (0 = least
    /// significant) for the byte leaves of a bound key (see
    /// [`super::bits`]). A key listed here renders as `type: flags`.
    flags: BTreeMap<(usize, FieldKey), BTreeMap<String, u32>>,
}

impl FieldBinding {
    pub fn new() -> Self {
        Self::default()
    }

    /// Name `key` of message `message`.
    pub fn name(mut self, message: usize, key: impl Into<FieldKey>, name: impl Into<String>) -> Self {
        self.names.insert((message, key.into()), name.into());
        self
    }

    pub fn insert(&mut self, message: usize, key: impl Into<FieldKey>, name: impl Into<String>) {
        self.names.insert((message, key.into()), name.into());
    }

    pub fn get(&self, message: usize, key: &str) -> Option<&str> {
        self.names
            .get(&(message, key.to_string()))
            .map(String::as_str)
    }

    /// Every bound name, for card validation.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.names.values().map(String::as_str)
    }

    /// Every bound `(message, key, name)`.
    pub fn entries(&self) -> impl Iterator<Item = (usize, &str, &str)> {
        self.names
            .iter()
            .map(|((message, key), name)| (*message, key.as_str(), name.as_str()))
    }

    /// Render the byte leaves of `key` of message `message` as packed flags.
    pub fn set_flags(&mut self, message: usize, key: impl Into<FieldKey>, bits: BTreeMap<String, u32>) {
        self.flags.insert((message, key.into()), bits);
    }

    /// The packed flags of `key`, when set.
    pub fn flags(&self, message: usize, key: &str) -> Option<&BTreeMap<String, u32>> {
        self.flags.get(&(message, key.to_string()))
    }
}

/// Why a binder did not name a slot's fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindRefusal {
    pub reason: String,
}

impl BindRefusal {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    pub fn not_implemented(signal: &str) -> Self {
        Self::new(format!("no field binder for {signal} yet"))
    }
}

/// Display names and packed flags of the keys of one message.
pub(crate) struct Names<'n> {
    /// Key -> display name; `None` means the key is unbound and the render
    /// fails.
    pub name: &'n dyn Fn(&str) -> Option<String>,
    /// Key -> packed flag bits (`{"<flag>": <bit>}`), for a key whose byte
    /// leaves render as `type: flags`.
    pub flags: &'n dyn Fn(&str) -> Option<BTreeMap<String, u32>>,
}

fn no_flags(_: &str) -> Option<BTreeMap<String, u32>> {
    None
}

/// Render one field node. `bits` are the packed flags of the enclosing
/// field: they apply to its non-literal byte leaves (through arrays and
/// choice arms, not into records, whose fields have their own keys).
fn render_node(node: &LayoutNode, names: &Names, key: &str, bits: Option<&BTreeMap<String, u32>>) -> Result<Value, String> {
    Ok(match node {
        LayoutNode::Leaf(leaf) => match bits {
            Some(bits) if leaf.kind == LeafKind::Byte && leaf.byte.is_none() => flags_value(leaf, bits),
            _ => leaf_value(leaf),
        },
        LayoutNode::ByteSelect {
            leaf,
            then_byte,
            else_byte,
        } => {
            let mut value = Map::new();
            value.insert("type".into(), json!("choice"));
            value.insert("mask".into(), mask_value(leaf.mask));
            value.insert("maskMode".into(), mask_mode(leaf.mask));
            value.insert(
                "arms".into(),
                json!([
                    {"label": "then", "type": "byte", "byte": then_byte},
                    {"label": "else", "type": "byte", "byte": else_byte},
                ]),
            );
            if let Some(xor) = leaf.xor {
                value.insert("xor".into(), json!(xor));
            }
            Value::Object(value)
        }
        LayoutNode::Array { item } => {
            let mut item_value = render_node(item, names, &format!("{key}.item"), bits)?;
            if let (Value::Object(map), false) = (&mut item_value, matches!(**item, LayoutNode::Record { .. })) {
                if let Some(field_name) = (names.name)(key) {
                    map.insert("name".into(), json!(format!("{field_name}_item")));
                }
            }
            json!({"type": "array", "item": item_value})
        }
        LayoutNode::Record { fields } => {
            let (order, fields) = render_fields(fields, names)?;
            json!({"type": "record", "order": order, "fields": fields})
        }
        LayoutNode::Choice { arms } => {
            let mut rendered = Vec::new();
            for (index, arm) in arms.iter().enumerate() {
                let label = arm_key(&arm.label, index);
                let mut value = render_node(&arm.node, names, &format!("{key}.{label}"), bits)?;
                if let Value::Object(map) = &mut value {
                    let mut with_label = Map::new();
                    with_label.insert("label".into(), json!(arm.label));
                    with_label.extend(std::mem::take(map));
                    value = Value::Object(with_label);
                }
                rendered.push(value);
            }
            json!({"type": "choice", "mask": Value::Null, "arms": rendered})
        }
        LayoutNode::Empty => json!({"type": "empty"}),
        LayoutNode::Unknown { reason } => json!({"type": "unknown", "reason": reason}),
    })
}

fn mask_value(mask: LeafMask) -> Value {
    match mask {
        LeafMask::Inherit | LeafMask::None => Value::Null,
        LeafMask::Nested(salt) => json!(salt),
        LeafMask::Unresolved => json!("unresolved"),
    }
}

fn mask_mode(mask: LeafMask) -> Value {
    json!(match mask {
        LeafMask::Inherit => "inherit",
        LeafMask::None => "none",
        LeafMask::Nested(_) => "nested",
        LeafMask::Unresolved => "unresolved",
    })
}

/// A packed flag byte: `{"type": "flags", "mask", "bits": {"<flag>": <bit>}}`.
fn flags_value(leaf: &LayoutLeaf, bits: &BTreeMap<String, u32>) -> Value {
    let mut value = Map::new();
    value.insert("type".into(), json!("flags"));
    value.insert("mask".into(), mask_value(leaf.mask));
    value.insert("maskMode".into(), mask_mode(leaf.mask));
    value.insert(
        "bits".into(),
        Value::Object(bits.iter().map(|(name, bit)| (name.clone(), json!(bit))).collect()),
    );
    if let Some(xor) = leaf.xor {
        value.insert("xor".into(), json!(xor));
    }
    Value::Object(value)
}

fn leaf_value(leaf: &LayoutLeaf) -> Value {
    let mut value = Map::new();
    value.insert("type".into(), json!(leaf.kind.as_str()));
    value.insert("mask".into(), mask_value(leaf.mask));
    value.insert("maskMode".into(), mask_mode(leaf.mask));
    if let Some(byte) = leaf.byte {
        value.insert("byte".into(), json!(byte));
    }
    if let Some(xor) = leaf.xor {
        value.insert("xor".into(), json!(xor));
    }
    if let Some(key) = &leaf.key {
        value.insert("key".into(), json!(key));
    }
    Value::Object(value)
}

fn render_fields(fields: &[LayoutField], names: &Names) -> Result<(Vec<String>, Map<String, Value>), String> {
    let mut order = Vec::new();
    let mut rendered = Map::new();
    for field in fields {
        let display = (names.name)(&field.key).ok_or_else(|| field.key.clone())?;
        if rendered.contains_key(&display) {
            return Err(format!("{} (duplicate name {display})", field.key));
        }
        let bits = (names.flags)(&field.key);
        let value = render_node(&field.node, names, &field.key, bits.as_ref())?;
        order.push(display.clone());
        rendered.insert(display, value);
    }
    Ok((order, rendered))
}

/// How a slot is routed to the output object.
#[derive(Clone, Debug, PartialEq)]
pub enum Routing {
    /// `positionalSignals[index]`.
    Positional(usize),
    /// `signals` (string name) or `keyedSignals` (numeric name).
    Name(Value),
}

/// Render a slot's schema object. `binding: None` renders positional keys
/// (`--layouts-only`); with a binding every key must be bound or the render
/// fails with the first unbound key.
pub fn render_signal(
    layout: &SignalLayout,
    routing: &Routing,
    binding: Option<&FieldBinding>,
) -> Result<Value, String> {
    let mut out = Map::new();
    match routing {
        Routing::Positional(index) => {
            out.insert("type".into(), json!("positional"));
            out.insert("positional".into(), json!(index));
        }
        Routing::Name(name) => {
            out.insert("type".into(), json!("name"));
            out.insert("name".into(), name.clone());
        }
    }
    let masks: Vec<Option<i64>> = layout.messages.iter().map(|message| message.mask).collect();
    let common = masks.first().copied().flatten().filter(|first| masks.iter().all(|m| *m == Some(*first)));
    out.insert("mask".into(), common.map(|salt| json!(salt)).unwrap_or(Value::Null));
    let render_message = |index: usize, message: &MessageLayout| -> Result<(Vec<String>, Map<String, Value>), String> {
        let name = |key: &str| -> Option<String> {
            match binding {
                None => Some(local_key(key)),
                Some(binding) => binding.get(index, key).map(str::to_string),
            }
        };
        let flags = |key: &str| binding.and_then(|binding| binding.flags(index, key).cloned());
        render_fields(&message.fields, &Names { name: &name, flags: &flags })
            .map_err(|key| format!("message {index}: unbound field {key}"))
    };
    match layout.messages.as_slice() {
        [] => {
            out.insert("order".into(), json!([]));
            out.insert("fields".into(), json!({}));
        }
        [message] => {
            let (order, fields) = render_message(0, message)?;
            out.insert("order".into(), json!(order));
            out.insert("fields".into(), Value::Object(fields));
        }
        messages => {
            let mut rendered = Vec::new();
            for (index, message) in messages.iter().enumerate() {
                let (order, fields) = render_message(index, message)?;
                rendered.push(json!({
                    "when": message.when.join(" | "),
                    "mask": message.mask.map(|salt| json!(salt)).unwrap_or(Value::Null),
                    "order": order,
                    "fields": fields,
                }));
            }
            out.insert("messages".into(), Value::Array(rendered));
        }
    }
    Ok(Value::Object(out))
}

/// The display form of a positional key inside its own record: the last
/// `fN` component.
fn local_key(key: &str) -> String {
    key.rsplit('.').next().unwrap_or(key).to_string()
}

// ===========================================================================
// Flat leaves (proof comparison)
// ===========================================================================

/// One leaf of one emission, in evaluation order, with the chain of
/// containers it sits in (`repeat`, `arm:<label>`). This is the projection's
/// view of a proof `field_set` entry.
#[derive(Clone, Debug, PartialEq)]
pub struct FlatLeaf {
    pub emission: usize,
    /// `number`, `string`, `byte`, `generic_value`.
    pub op: &'static str,
    pub chain: Vec<String>,
    /// Effective stream salt (`None` when not stream-masked).
    pub salt: Option<i64>,
    pub xor: Option<f64>,
    /// Literal byte values (`[130]`, `[A, B]` for a byte select).
    pub bytes: Option<Vec<i64>>,
    /// Object key of a named-object value.
    pub key: Option<String>,
}

impl SignalLayout {
    /// Every leaf of every emission, flattened back from the projected nodes.
    pub fn flat_leaves(&self) -> Vec<FlatLeaf> {
        let mut out = Vec::new();
        for emission in &self.emissions {
            // Items after a split choice are in every part's fields; flatten
            // them once, outside the arms.
            let mut uses: BTreeMap<usize, usize> = BTreeMap::new();
            for index in emission.part_leaves.iter().flatten() {
                *uses.entry(*index).or_default() += 1;
            }
            let mut shared_done = false;
            for (part_index, (arm, part)) in emission.parts.iter().enumerate() {
                let indices = emission.part_leaves(part_index);
                let mut cursor = 0;
                for field in &part.fields {
                    let count = layout_leaves(&field.node);
                    let shared = count > 0
                        && indices
                            .get(cursor..cursor + count)
                            .is_some_and(|run| run.iter().all(|index| uses.get(index).copied().unwrap_or(0) > 1));
                    cursor += count;
                    let mut chain = Vec::new();
                    if shared {
                        if shared_done {
                            continue;
                        }
                    } else if let Some(arm) = arm {
                        chain.push(format!("arm:{arm}"));
                    }
                    flatten(&field.node, emission.emission, part.mask, &mut chain, &mut out);
                }
                shared_done |= emission.parts.len() > 1;
            }
        }
        out
    }
}

fn flat_leaf(leaf: &LayoutLeaf, emission: usize, message: Option<i64>, chain: &[String], bytes: Option<Vec<i64>>) -> FlatLeaf {
    FlatLeaf {
        emission,
        op: match leaf.kind {
            LeafKind::Number => "number",
            LeafKind::String => "string",
            LeafKind::Byte => "byte",
            LeafKind::Value => "generic_value",
        },
        chain: chain.to_vec(),
        salt: match leaf.mask {
            LeafMask::Inherit => message,
            LeafMask::Nested(salt) => Some(salt),
            LeafMask::None | LeafMask::Unresolved => leaf.salt,
        },
        xor: leaf.xor,
        bytes,
        key: leaf.key.clone(),
    }
}

fn flatten(node: &LayoutNode, emission: usize, message: Option<i64>, chain: &mut Vec<String>, out: &mut Vec<FlatLeaf>) {
    match node {
        LayoutNode::Leaf(leaf) => out.push(flat_leaf(leaf, emission, message, chain, leaf.byte.map(|b| vec![b]))),
        LayoutNode::ByteSelect {
            leaf,
            then_byte,
            else_byte,
        } => out.push(flat_leaf(leaf, emission, message, chain, Some(vec![*then_byte, *else_byte]))),
        LayoutNode::Array { item } => {
            chain.push("repeat".into());
            flatten(item, emission, message, chain, out);
            chain.pop();
        }
        LayoutNode::Record { fields } => fields
            .iter()
            .for_each(|field| flatten(&field.node, emission, message, chain, out)),
        LayoutNode::Choice { arms } => {
            for arm in arms {
                chain.push(format!("arm:{}", arm.label));
                flatten(&arm.node, emission, message, chain, out);
                chain.pop();
            }
        }
        LayoutNode::Empty | LayoutNode::Unknown { .. } => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn salts_and_byte_literals() {
        assert_eq!(parse_salt("$DH ^ 128625474"), Some(128625474));
        assert_eq!(parse_salt("2624265287"), Some(2624265287));
        assert!(matches!(byte_literal("130"), Some(ByteLiteral::One(130))));
        assert!(matches!(
            byte_literal("$CN.a ? 96 : 120"),
            Some(ByteLiteral::Select(96, 120))
        ));
        assert!(byte_literal("$DA").is_none());
        assert!(byte_literal("x ? y : 3").is_none());
    }

    #[test]
    fn local_keys() {
        assert_eq!(local_key("f3.item.f2"), "f2");
        assert_eq!(local_key("f1"), "f1");
    }
}
