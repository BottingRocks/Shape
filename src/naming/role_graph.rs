//! Whole-program role/inventory graph for a recovered Shape-style capture.
//!
//! # What this module is
//!
//! [`extract_program_role_graph`] answers the first whole-program naming
//! question — *what is each callable and each MAIN-owned binding, and what
//! source-derived evidence supports that?* — without inventing a name, a
//! purpose, or a runtime fact. It joins the already-committed property passes:
//!
//! * [`super::function_index::FunctionIndex`] accounts for **every** callable
//!   (declarations, expressions, arrows, object/class methods, getters, setters
//!   and constructors) exactly once, and for every direct binding owned by the
//!   selected MAIN scope, with lexical call edges, unresolved calls, held
//!   values/alternatives and aliases;
//! * [`super::encoding::EncodingProperties`] supplies source-derived
//!   NUMBER/STRING/byte-stream/hash helper identities and state bindings;
//! * [`super::wiring::WiringProperties`] supplies the source-derived group,
//!   direct-route, coordinator, store, notifier and public-init identities.
//!
//! The graph keeps **scope/source ownership**, **inferred purpose roles** and
//! **runtime execution** as three separate fields. Lexical containment is
//! ownership only; it is never promoted to a semantic role. A callable with no
//! source-derived role evidence keeps an explicit [`UnresolvedFact`] instead of
//! being labelled a generic "utility". Conflicting role evidence, ambiguous
//! aliases and mutable/conditional definitions are retained as conflict or
//! unknown evidence rather than resolved first-wins.
//!
//! # Identity and joins
//!
//! The only cross-report join key is the exact source identity (SHA-256 plus
//! byte count) followed by an exact source span in that same source. Symbol ids
//! are never compared across parses: two same-spelled locals in different
//! parses are different bindings, so the graph uses the source-derived
//! [`super::function_index::FunctionRecord::id`] and
//! [`super::function_index::MainVariableRecord::id`] plus spans only.
//!
//! `with_context` refuses a context whose source identity differs from the
//! supplied source, and validates that every identity span it consumes is
//! inside the source. No generated `$KEY_n`/`$THREAD_n` spelling, slot number,
//! offset, count or build constant is a recognition gate.
//!
//! # Value identity before role uniqueness
//!
//! A binding is not a value. Each [`RoleGraphNode`] keeps its own binding
//! identity, and also records a canonical [`RoleGraphNode::value_identity`]:
//! a MAIN binding whose single unconditional definition holds a function or
//! class takes that function/class id, and an immutable alias takes its
//! target's identity through proven single-definition `alias_of` links (the
//! target must itself be proven static and initialized before the alias is
//! written). Unique-role checks compare these value identities, so a holder
//! and its own immutable alias are one value with one role, while two
//! different values claiming one unique role are still reported. Holder and
//! alias stay separate nodes so later naming can name each binding.
//!
//! # Edge coverage
//!
//! [`RoleGraphEdgeKind::ReadsMainBinding`] and
//! [`RoleGraphEdgeKind::WritesMainBinding`] come from the function index's
//! per-function MAIN captures, which exclude the MAIN function's own body. They
//! record accesses from nested functions only and are **not** an assignment
//! inventory: zero write edges does not mean a binding is never initialized.
//! For total-assignment questions use
//! [`super::function_index::MainVariableRecord::writes`] and
//! [`super::function_index::MainVariableRecord::assignments`].
//!
//! # Coverage is accounting, not semantic closure
//!
//! [`RoleGraphCoverage`] reports counts by node kind, ownership, held shape,
//! role and edge kind, plus the unresolved set. Those counts are an inventory
//! of *this input*, not a proof that the whole program is semantically
//! understood. [`SemanticClosure`] is always [`SemanticClosure::NotClaimed`] and
//! [`RuntimeExecution`] is always [`RuntimeExecution::NotObserved`]: neither
//! recovered JavaScript nor any runtime probe is executed here.
//!
//! # Extending the graph
//!
//! A future typed property pass (for example the parallel seed/payload/output
//! workers) can add facts with [`ProgramRoleGraph::apply_typed_role_facts`].
//! The extension point accepts only a closed [`RoleKind`] plus an exact span
//! that must already identify a node; there is deliberately no string role
//! hint, JSON role-map loader, generated-name matcher or count gate.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::signals::extract::encoding::{
    BindingEvidence, EncodingProperties, FunctionEvidence, ENCODING_PROPERTIES_SCHEMA_VERSION,
};
use super::function_index::{
    BindingDeclarationKind, FunctionIndex, FunctionKind, HeldAlternative, MainVariableClass,
    MainVariableRecord, FUNCTION_INDEX_SCHEMA_VERSION,
};
use crate::signals::extract::wiring::{EmitterMethod, WiringProperties, PROGRAM_WIRING_SCHEMA_VERSION};
use crate::signals::schema::{BindingRef, Confidence, Diagnostic, SourceIdentity, SourceSpan, UnresolvedFact};
use crate::naming::function_index::CallResolution;

/// Version of the serialized [`ProgramRoleGraph`] contract.
///
/// Version 2 adds class nodes, the `holds_class`/`class_member` edges, the
/// `class` held shape, and per-node `class_id`/`value_identity`. Every
/// version-1 field is kept.
pub const PROGRAM_ROLE_GRAPH_SCHEMA_VERSION: u32 = 2;

// ===========================================================================
// Errors
// ===========================================================================

/// A typed role-graph failure. Every variant refuses instead of guessing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoleGraphError {
    /// No source or an unusable context was supplied.
    InvalidInput(String),
    /// A supplied property context was extracted from a different source.
    IdentityMismatch {
        context: &'static str,
        expected: SourceIdentity,
        actual: SourceIdentity,
    },
    /// A context was internally inconsistent (duplicate ids, out-of-bounds
    /// span, wrong schema).
    InvalidContext {
        context: &'static str,
        message: String,
    },
    /// One of the joined property passes refused the input.
    Extraction {
        context: &'static str,
        message: String,
    },
    /// A graph invariant would be violated by the assembled report.
    Invariant(String),
}

impl std::fmt::Display for RoleGraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(f, "invalid role-graph input: {message}"),
            Self::IdentityMismatch {
                context,
                expected,
                actual,
            } => write!(
                f,
                "role-graph {context} identity mismatch: expected {} bytes / {}, got {} bytes / {}",
                expected.bytes, expected.sha256, actual.bytes, actual.sha256
            ),
            Self::InvalidContext { context, message } => {
                write!(f, "invalid role-graph {context} context: {message}")
            }
            Self::Extraction { context, message } => {
                write!(f, "role-graph {context} extraction failed: {message}")
            }
            Self::Invariant(message) => write!(f, "role-graph invariant violated: {message}"),
        }
    }
}

impl std::error::Error for RoleGraphError {}

// ===========================================================================
// Report vocabulary
// ===========================================================================

/// What a node inventories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleGraphNodeKind {
    /// One entry from [`FunctionIndex::functions`].
    Function,
    /// One entry from [`FunctionIndex::main_variables`].
    MainVariable,
    /// A call the inventory could not resolve to a function identity. These
    /// nodes exist only so a caller -> unresolved-target edge has a real target;
    /// they are not functions.
    UnresolvedCallTarget,
    /// One entry from [`FunctionIndex::classes`]. A class is a value, not a
    /// callable; its constructor and methods stay function nodes.
    Class,
}

impl RoleGraphNodeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::MainVariable => "main_variable",
            Self::UnresolvedCallTarget => "unresolved_call_target",
            Self::Class => "class",
        }
    }
}

/// Where a node sits in the source ownership hierarchy. This is deliberately
/// separate from [`RoleAssignment`] (inferred purpose) and
/// [`RuntimeExecution`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceOwnership {
    /// The selected MAIN function.
    MainFunction,
    /// A direct binding of the selected MAIN scope.
    MainOwnedBinding,
    /// A callable lexically contained in the selected MAIN function.
    MainNestedFunction,
    /// A callable declared at program top level that is not MAIN.
    TopLevelFunction,
    /// A callable lexically nested under a non-MAIN callable.
    NestedFunction,
    /// A call site the inventory could not resolve to a function identity.
    UnresolvedCallSite,
    /// A class lexically contained in the selected MAIN function.
    MainNestedClass,
    /// A class declared at program top level.
    TopLevelClass,
    /// A class lexically nested under a non-MAIN callable.
    NestedClass,
}

impl SourceOwnership {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MainFunction => "main_function",
            Self::MainOwnedBinding => "main_owned_binding",
            Self::MainNestedFunction => "main_nested_function",
            Self::TopLevelFunction => "top_level_function",
            Self::NestedFunction => "nested_function",
            Self::UnresolvedCallSite => "unresolved_call_site",
            Self::MainNestedClass => "main_nested_class",
            Self::TopLevelClass => "top_level_class",
            Self::NestedClass => "nested_class",
        }
    }
}

/// The static shape a MAIN binding holds, copied from the function index. It is
/// evidence about the value, *not* a semantic purpose role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeldClass {
    Literal,
    ArrayLiteral,
    ObjectLiteral,
    Function,
    ConfiguredResult,
    HostReference,
    Alias,
    RuntimeState,
    Unknown,
    Class,
}

impl HeldClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Literal => "literal",
            Self::ArrayLiteral => "array_literal",
            Self::ObjectLiteral => "object_literal",
            Self::Function => "function",
            Self::ConfiguredResult => "configured_result",
            Self::HostReference => "host_reference",
            Self::Alias => "alias",
            Self::RuntimeState => "runtime_state",
            Self::Unknown => "unknown",
            Self::Class => "class",
        }
    }
}

impl From<MainVariableClass> for HeldClass {
    fn from(value: MainVariableClass) -> Self {
        match value {
            MainVariableClass::Literal => Self::Literal,
            MainVariableClass::ArrayLiteral => Self::ArrayLiteral,
            MainVariableClass::ObjectLiteral => Self::ObjectLiteral,
            MainVariableClass::Function => Self::Function,
            MainVariableClass::ConfiguredResult => Self::ConfiguredResult,
            MainVariableClass::HostReference => Self::HostReference,
            MainVariableClass::Alias => Self::Alias,
            MainVariableClass::RuntimeState => Self::RuntimeState,
            MainVariableClass::Unknown => Self::Unknown,
            MainVariableClass::Class => Self::Class,
        }
    }
}

/// Runtime execution state. This pass never executes recovered JavaScript and
/// never treats source presence as a runtime observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeExecution {
    NotObserved,
}

/// Explicit statement that an inventory count is not whole-program semantic
/// closure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticClosure {
    NotClaimed,
}

/// A closed set of source-derived purpose roles. Each variant is available only
/// because an existing property/call/consumer surface proved the corresponding
/// structure; the graph never assigns a generic catch-all role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleKind {
    // -- program scope -----------------------------------------------------
    MainEntry,
    TopLevelEntry,
    // -- registries / routing ---------------------------------------------
    DescriptorRegistry,
    WaveRegistry,
    WaveIndexRows,
    WaveRoutingSelector,
    WaveCollector,
    DirectCollector,
    DirectSignalCallback,
    SignalCollector,
    // -- encoding ----------------------------------------------------------
    NumberWriter,
    StringFactory,
    StringWriter,
    CreateSignalMask,
    HashTransactionToken,
    ByteStreamCycle,
    StringCache,
    StringCounter,
    ByteStreamState,
    // -- observer / service functions -------------------------------------
    ObserverFactory,
    SnapshotGetter,
    EnvelopeBuilder,
    SerializerFactory,
    DefaultEncoder,
    DefaultSignalFilter,
    SessionConstructor,
    SessionInit,
    SessionTeardown,
    ContextConfig,
    Coordinator,
    Notifier,
    PublicInit,
    ErrorRingConstructor,
    // -- stores / state bindings ------------------------------------------
    LatestValueStore,
    StatusStore,
    DebugNameStore,
    ErrorRingBuffer,
    SentinelObject,
    TagTable,
    PrefixScratch,
    NotifierTable,
    CoordinatorState,
    SharedNoop,
    SeedState,
    // -- structural consumer positions ------------------------------------
    CallbackArgument,
    ConstructorArgument,
    ReturnedFunction,
}

impl RoleKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MainEntry => "main_entry",
            Self::TopLevelEntry => "top_level_entry",
            Self::DescriptorRegistry => "descriptor_registry",
            Self::WaveRegistry => "wave_registry",
            Self::WaveIndexRows => "wave_index_rows",
            Self::WaveRoutingSelector => "wave_routing_selector",
            Self::WaveCollector => "wave_collector",
            Self::DirectCollector => "direct_collector",
            Self::DirectSignalCallback => "direct_signal_callback",
            Self::SignalCollector => "signal_collector",
            Self::NumberWriter => "number_writer",
            Self::StringFactory => "string_factory",
            Self::StringWriter => "string_writer",
            Self::CreateSignalMask => "create_signal_mask",
            Self::HashTransactionToken => "hash_transaction_token",
            Self::ByteStreamCycle => "byte_stream_cycle",
            Self::StringCache => "string_cache",
            Self::StringCounter => "string_counter",
            Self::ByteStreamState => "byte_stream_state",
            Self::ObserverFactory => "observer_factory",
            Self::SnapshotGetter => "snapshot_getter",
            Self::EnvelopeBuilder => "envelope_builder",
            Self::SerializerFactory => "serializer_factory",
            Self::DefaultEncoder => "default_encoder",
            Self::DefaultSignalFilter => "default_signal_filter",
            Self::SessionConstructor => "session_constructor",
            Self::SessionInit => "session_init",
            Self::SessionTeardown => "session_teardown",
            Self::ContextConfig => "context_config",
            Self::Coordinator => "coordinator",
            Self::Notifier => "notifier",
            Self::PublicInit => "public_init",
            Self::ErrorRingConstructor => "error_ring_constructor",
            Self::LatestValueStore => "latest_value_store",
            Self::StatusStore => "status_store",
            Self::DebugNameStore => "debug_name_store",
            Self::ErrorRingBuffer => "error_ring_buffer",
            Self::SentinelObject => "sentinel_object",
            Self::TagTable => "tag_table",
            Self::PrefixScratch => "prefix_scratch",
            Self::NotifierTable => "notifier_table",
            Self::CoordinatorState => "coordinator_state",
            Self::SharedNoop => "shared_noop",
            Self::SeedState => "seed_state",
            Self::CallbackArgument => "callback_argument",
            Self::ConstructorArgument => "constructor_argument",
            Self::ReturnedFunction => "returned_function",
        }
    }

    /// Mutually-exclusive role slot on one node. Two different roles in the same
    /// slot are retained as conflict evidence, never first-wins.
    pub const fn slot(self) -> Option<&'static str> {
        match self {
            Self::MainEntry => Some("main_entry"),
            Self::DescriptorRegistry => Some("descriptor_registry"),
            Self::WaveRegistry => Some("wave_registry"),
            Self::WaveIndexRows => Some("wave_index_rows"),
            Self::WaveRoutingSelector => Some("wave_routing_selector"),
            Self::WaveCollector | Self::SignalCollector => Some("wave_collector"),
            Self::DirectCollector => Some("direct_collector"),
            Self::DirectSignalCallback => Some("direct_signal_callback"),
            Self::NumberWriter => Some("number_writer"),
            Self::StringFactory => Some("string_factory"),
            Self::StringWriter => Some("string_writer"),
            Self::CreateSignalMask => Some("create_signal_mask"),
            Self::HashTransactionToken => Some("hash_transaction_token"),
            Self::ByteStreamCycle => Some("byte_stream_cycle"),
            Self::StringCache => Some("string_cache"),
            Self::StringCounter => Some("string_counter"),
            Self::ByteStreamState => Some("byte_stream_state"),
            Self::ObserverFactory => Some("observer_factory"),
            Self::SnapshotGetter => Some("snapshot_getter"),
            Self::EnvelopeBuilder => Some("envelope_builder"),
            Self::SerializerFactory => Some("serializer_factory"),
            Self::DefaultEncoder => Some("default_encoder"),
            Self::DefaultSignalFilter => Some("default_signal_filter"),
            Self::SessionConstructor => Some("session_constructor"),
            Self::SessionInit => Some("session_init"),
            Self::SessionTeardown => Some("session_teardown"),
            Self::ContextConfig => Some("context_config"),
            Self::Coordinator => Some("coordinator"),
            Self::Notifier => Some("notifier"),
            Self::PublicInit => Some("public_init"),
            Self::ErrorRingConstructor => Some("error_ring_constructor"),
            Self::LatestValueStore => Some("latest_value_store"),
            Self::StatusStore => Some("status_store"),
            Self::DebugNameStore => Some("debug_name_store"),
            Self::ErrorRingBuffer => Some("error_ring_buffer"),
            Self::SentinelObject => Some("sentinel_object"),
            Self::TagTable => Some("tag_table"),
            Self::PrefixScratch => Some("prefix_scratch"),
            Self::NotifierTable => Some("notifier_table"),
            Self::SharedNoop => Some("shared_noop"),
            Self::SeedState => Some("seed_state"),
            Self::TopLevelEntry
            | Self::CoordinatorState
            | Self::CallbackArgument
            | Self::ConstructorArgument
            | Self::ReturnedFunction => None,
        }
    }

    /// True when at most one node of each kind may carry this role. Used for
    /// explicit identity-conflict evidence; it never selects a winner.
    pub const fn unique_identity(self) -> bool {
        !matches!(
            self,
            Self::WaveCollector
                | Self::SignalCollector
                | Self::DirectCollector
                | Self::DirectSignalCallback
                | Self::TopLevelEntry
                | Self::CoordinatorState
                | Self::ByteStreamState
                | Self::CallbackArgument
                | Self::ConstructorArgument
                | Self::ReturnedFunction
        )
    }

    /// WaveCollector and SignalCollector are the same collector concept seen by
    /// two property passes; they may coexist on one node without a conflict.
    pub const fn compatible_with(self, other: Self) -> bool {
        matches!(
            (self, other),
            (Self::WaveCollector, Self::SignalCollector)
                | (Self::SignalCollector, Self::WaveCollector)
        )
    }
}

/// Typed provenance of one role evidence item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleEvidenceSource {
    FunctionIndexRole,
    FunctionIndexMainVariableRole,
    FunctionIndexMainSelection,
    FunctionIndexTopLevelEntry,
    FunctionIndexDeclarationContext,
    EncodingHelper,
    EncodingBinding,
    WiringMain,
    WiringDescriptorRegistry,
    WiringGroup,
    WiringDirectRoute,
    WiringCoordinator,
    WiringStore,
    WiringPublicInit,
    WiringNotifier,
    ExternalTypedAdapter,
}

impl RoleEvidenceSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FunctionIndexRole => "function_index_role",
            Self::FunctionIndexMainVariableRole => "function_index_main_variable_role",
            Self::FunctionIndexMainSelection => "function_index_main_selection",
            Self::FunctionIndexTopLevelEntry => "function_index_top_level_entry",
            Self::FunctionIndexDeclarationContext => "function_index_declaration_context",
            Self::EncodingHelper => "encoding_helper",
            Self::EncodingBinding => "encoding_binding",
            Self::WiringMain => "wiring_main",
            Self::WiringDescriptorRegistry => "wiring_descriptor_registry",
            Self::WiringGroup => "wiring_group",
            Self::WiringDirectRoute => "wiring_direct_route",
            Self::WiringCoordinator => "wiring_coordinator",
            Self::WiringStore => "wiring_store",
            Self::WiringPublicInit => "wiring_public_init",
            Self::WiringNotifier => "wiring_notifier",
            Self::ExternalTypedAdapter => "external_typed_adapter",
        }
    }
}

/// One exact-source evidence item supporting a role.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleEvidence {
    pub source: RoleEvidenceSource,
    pub detail: String,
    pub span: Option<SourceSpan>,
}

/// A role with all the source-derived evidence that supports it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleAssignment {
    pub role: RoleKind,
    pub confidence: Confidence,
    pub evidence: Vec<RoleEvidence>,
}

/// One graph node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoleGraphNode {
    /// Source-derived identity from the function index (or `uc@start..end` for
    /// an unresolved call target node).
    pub id: String,
    pub kind: RoleGraphNodeKind,
    /// Existing source label, never a generated or proposed name.
    pub label: Option<String>,
    /// Whole declaration span (function node or MAIN binding declarator).
    pub span: SourceSpan,
    /// Exact declaring identifier span when the source has one.
    pub identifier_span: Option<SourceSpan>,
    /// Lexical/source ownership, separate from purpose and runtime.
    pub ownership: SourceOwnership,
    /// Lexical parent identity. Ownership only, not semantic proof.
    pub lexical_parent: Option<String>,
    /// Source-derived purpose roles. Empty only when `unresolved` explains why.
    pub roles: Vec<RoleAssignment>,
    /// Static value shape for MAIN bindings (function-index evidence).
    pub held: Option<HeldClass>,
    pub held_proven_static: bool,
    /// All competing definitions when the value is not a single unconditional
    /// definition. Kept as evidence, never collapsed to the first write.
    pub held_alternatives: Vec<HeldAlternative>,
    pub alias_of: Option<String>,
    /// Source-derived signal membership. Memberships are not sorted by any
    /// assumed family or display order; the vector is deduplicated for output.
    pub signal_indices: Vec<u32>,
    pub wave_indices: Vec<u32>,
    /// Emitter evidence proven by the wiring pass for collector containers.
    pub emitters: Vec<RoleGraphEmitterEvidence>,
    pub function_kind: Option<FunctionKind>,
    /// For a class node, its own id; for a MAIN binding, the class it is
    /// proven to hold; for a function node, the class it is a member of.
    #[serde(default)]
    pub class_id: Option<String>,
    /// Canonical proven value identity used for unique-role checks: a function
    /// or class id when the node is (or provably holds) one, the alias
    /// target's identity for an immutable alias, otherwise the node's own id.
    /// It never replaces `id`: holder and alias stay separate bindings.
    #[serde(default)]
    pub value_identity: String,
    /// MAIN binding ids followed from this node to reach `value_identity`, in
    /// order. Empty when the node is its own value.
    #[serde(default)]
    pub value_identity_path: Vec<String>,
    pub is_top_level_entry: bool,
    /// Lexically resolved caller count. Zero is **not** deadness: a callback,
    /// property call or external invocation may still reach it.
    pub lexical_caller_count: usize,
    pub lexical_callee_count: usize,
    pub unresolved_call_count: usize,
    /// Explicit unknown/conflict evidence. A node with no roles always has at
    /// least one entry here.
    pub unresolved: Vec<UnresolvedFact>,
    pub runtime: RuntimeExecution,
}

impl RoleGraphNode {
    fn add_role(&mut self, role: RoleKind, confidence: Confidence, evidence: RoleEvidence) {
        if let Some(existing) = self.roles.iter_mut().find(|item| item.role == role) {
            if !existing.evidence.iter().any(|item| item == &evidence) {
                existing.evidence.push(evidence);
            }
            existing.confidence = max_confidence(existing.confidence, confidence);
            return;
        }
        let conflicts: Vec<RoleKind> = self
            .roles
            .iter()
            .filter(|item| {
                item.role.slot() == role.slot()
                    && item.role != role
                    && !item.role.compatible_with(role)
            })
            .map(|item| item.role)
            .collect();
        self.roles.push(RoleAssignment {
            role,
            confidence,
            evidence: vec![evidence],
        });
        for other in conflicts {
            let reason = format!(
                "conflicting role evidence on one node: {} vs {}",
                other.as_str(),
                role.as_str()
            );
            if !self
                .unresolved
                .iter()
                .any(|fact| fact.fact == "role_conflict" && fact.reason == reason)
            {
                self.unresolved.push(UnresolvedFact {
                    fact: "role_conflict".to_string(),
                    reason,
                    span: Some(self.span),
                    required: true,
                });
            }
        }
    }
}

/// Emitter evidence linked to a collector container. A mixed collector may hold
/// several unrelated signals; each call stays individual evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleGraphEmitterEvidence {
    pub local_position: u32,
    pub signal: Option<u32>,
    pub method: EmitterMethod,
    pub span: SourceSpan,
    pub receiver_source: String,
    pub payload_source: Option<String>,
}

/// An edge in the whole-program graph. Every `from`/`to` names a node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleGraphEdge {
    pub id: String,
    pub kind: RoleGraphEdgeKind,
    pub from: String,
    pub to: String,
    pub span: SourceSpan,
    pub detail: String,
    /// Populated only for resolved call edges.
    pub call_resolution: Option<CallResolution>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleGraphEdgeKind {
    /// Lexical containment: parent callable owns a nested callable.
    LexicallyContains,
    /// MAIN function owns a direct binding.
    OwnsBinding,
    /// A MAIN binding holds a function value (only when the value is a proven
    /// single unconditional definition).
    HoldsFunction,
    /// A MAIN binding is a single-definition alias of another MAIN binding.
    AliasOf,
    /// A MAIN binding holds a class value (a class declaration, or a proven
    /// single unconditional class definition).
    HoldsClass,
    /// A class owns a constructor, method, getter, setter or field function.
    ClassMember,
    /// A lexically resolved call.
    Calls,
    /// A call whose callee the inventory could not resolve to one identity.
    UnresolvedCall,
    /// A nested callable reads a MAIN-owned binding at an exact site. Reads in
    /// the MAIN function's own body are not edges.
    ReadsMainBinding,
    /// A nested callable writes a MAIN-owned binding at an exact site. Writes
    /// in the MAIN function's own body are not edges, so these edges are not
    /// an assignment inventory; use `MainVariableRecord::writes` and
    /// `MainVariableRecord::assignments` for that.
    WritesMainBinding,
}

impl RoleGraphEdgeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LexicallyContains => "lexically_contains",
            Self::OwnsBinding => "owns_binding",
            Self::HoldsFunction => "holds_function",
            Self::AliasOf => "alias_of",
            Self::HoldsClass => "holds_class",
            Self::ClassMember => "class_member",
            Self::Calls => "calls",
            Self::UnresolvedCall => "unresolved_call",
            Self::ReadsMainBinding => "reads_main_binding",
            Self::WritesMainBinding => "writes_main_binding",
        }
    }
}

/// Inventory counts plus the explicit unresolved set. These are observations
/// about this input, never recognition gates or a semantic-closure claim.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleGraphCoverage {
    pub function_node_count: usize,
    pub main_variable_node_count: usize,
    pub unresolved_call_target_count: usize,
    #[serde(default)]
    pub class_node_count: usize,
    pub total_node_count: usize,
    pub total_edge_count: usize,
    pub functions_with_roles: usize,
    pub functions_without_roles: usize,
    pub main_variables_with_roles: usize,
    pub main_variables_without_roles: usize,
    pub nodes_with_explicit_unknowns: usize,
    pub nodes_and_context_unknowns: usize,
    /// Must be zero. A non-zero value would mean a function/MAIN record had
    /// neither a role nor an explicit unknown, which the extractor refuses.
    pub nodes_missing_role_and_unknown: usize,
    pub function_role_counts: BTreeMap<String, usize>,
    pub main_variable_role_counts: BTreeMap<String, usize>,
    #[serde(default)]
    pub class_role_counts: BTreeMap<String, usize>,
    pub held_class_counts: BTreeMap<String, usize>,
    pub ownership_counts: BTreeMap<String, usize>,
    pub edge_kind_counts: BTreeMap<String, usize>,
    pub evidence_source_counts: BTreeMap<String, usize>,
    pub unresolved_count: usize,
}

/// A typed role fact supplied by a future property adapter. The span must
/// already identify an existing node of `node_kind`; no name/count hint is
/// accepted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleFact {
    pub node_span: SourceSpan,
    pub node_kind: RoleGraphNodeKind,
    pub role: RoleKind,
    pub confidence: Confidence,
    pub evidence: RoleEvidence,
}

/// The complete whole-program role/inventory graph for one exact capture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgramRoleGraph {
    pub schema_version: u32,
    pub source: SourceIdentity,
    /// The MAIN function node id, when the inventory selected one.
    pub main: Option<String>,
    pub nodes: Vec<RoleGraphNode>,
    pub edges: Vec<RoleGraphEdge>,
    pub coverage: RoleGraphCoverage,
    /// Context-level unresolved facts (encoding/wiring gaps, top-level
    /// unresolved calls, identity conflicts). Node-level unknowns live on
    /// [`RoleGraphNode::unresolved`].
    pub unresolved: Vec<UnresolvedFact>,
    pub diagnostics: Vec<Diagnostic>,
    pub runtime: RuntimeExecution,
    pub closure: SemanticClosure,
}

impl ProgramRoleGraph {
    /// Add facts from a future typed adapter. Each fact must match an existing
    /// node by exact `(kind, span)`; unknown spans are rejected instead of
    /// creating a speculative node. Coverage is recomputed afterwards.
    pub fn apply_typed_role_facts(&mut self, facts: &[RoleFact]) -> Result<(), RoleGraphError> {
        for fact in facts {
            let index = self
                .nodes
                .iter()
                .position(|node| node.kind == fact.node_kind && node.span == fact.node_span)
                .ok_or_else(|| RoleGraphError::InvalidContext {
                    context: "typed adapter",
                    message: format!(
                        "role fact span {}..{} does not identify a {} node",
                        fact.node_span.start,
                        fact.node_span.end,
                        fact.node_kind.as_str()
                    ),
                })?;
            self.nodes[index].add_role(fact.role, fact.confidence, fact.evidence.clone());
        }
        // New roles can create or settle unique-role conflicts; refresh them.
        self.unresolved
            .retain(|fact| !fact.fact.starts_with(ROLE_IDENTITY_CONFLICT_FACT));
        self.unresolved.extend(role_identity_conflicts(&self.nodes));
        self.coverage = compute_coverage(&self.nodes, &self.edges, &self.unresolved);
        Ok(())
    }

    /// Look up a node by its exact source-derived id.
    pub fn node(&self, id: &str) -> Option<&RoleGraphNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    /// Nodes of one kind, in inventory order.
    pub fn nodes_of_kind(&self, kind: RoleGraphNodeKind) -> impl Iterator<Item = &RoleGraphNode> {
        self.nodes.iter().filter(move |node| node.kind == kind)
    }
}

// ===========================================================================
// Public API
// ===========================================================================

/// Extract the whole-program role graph, joining all existing property passes
/// from the same exact source.
pub fn extract_program_role_graph(source: &str) -> Result<ProgramRoleGraph, RoleGraphError> {
    if source.trim().is_empty() {
        return Err(RoleGraphError::InvalidInput("source is empty".to_string()));
    }
    let index =
        crate::naming::function_index::extract_function_index(source).map_err(|error| RoleGraphError::Extraction {
            context: "function_index",
            message: error.to_string(),
        })?;
    let encoding = crate::signals::extract::encoding::extract_encoding_properties(source).map_err(|error| {
        RoleGraphError::Extraction {
            context: "encoding_properties",
            message: error.to_string(),
        }
    })?;
    let wiring = crate::signals::extract::wiring::extract_program_wiring(source).map_err(|error| {
        RoleGraphError::Extraction {
            context: "wiring_properties",
            message: error.to_string(),
        }
    })?;
    extract_program_role_graph_with_context(source, &index, &encoding, &wiring)
}

/// Extract the whole-program role graph from already-extracted contexts.
///
/// Every context must carry the identity of the supplied `source`; otherwise
/// the call is refused. Joins use only exact spans/ids in that one source.
pub fn extract_program_role_graph_with_context(
    source: &str,
    index: &FunctionIndex,
    encoding: &EncodingProperties,
    wiring: &WiringProperties,
) -> Result<ProgramRoleGraph, RoleGraphError> {
    if source.trim().is_empty() {
        return Err(RoleGraphError::InvalidInput("source is empty".to_string()));
    }
    let identity = role_graph_identity(source);
    validate_identity("function_index", &identity, &index.source)?;
    validate_identity("encoding_properties", &identity, &encoding.source)?;
    validate_identity("wiring_properties", &identity, &wiring.source)?;
    Builder::new(source, identity, index, encoding, wiring)?.finish()
}

// ===========================================================================
// Builder
// ===========================================================================

struct Builder<'a> {
    identity: SourceIdentity,
    index: &'a FunctionIndex,
    encoding: &'a EncodingProperties,
    wiring: &'a WiringProperties,
    nodes: Vec<RoleGraphNode>,
    node_by_id: HashMap<String, usize>,
    function_by_span: HashMap<(u32, u32), usize>,
    main_var_by_identifier: HashMap<(u32, u32), usize>,
    main_var_by_declaration: HashMap<(u32, u32), usize>,
    edges: Vec<RoleGraphEdge>,
    edge_ids: BTreeSet<String>,
    global_unresolved: Vec<UnresolvedFact>,
    global_unresolved_keys: BTreeSet<(String, String, u32, u32)>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Builder<'a> {
    fn new(
        source: &'a str,
        identity: SourceIdentity,
        index: &'a FunctionIndex,
        encoding: &'a EncodingProperties,
        wiring: &'a WiringProperties,
    ) -> Result<Self, RoleGraphError> {
        validate_context(source, index, encoding, wiring)?;
        Ok(Self {
            identity,
            index,
            encoding,
            wiring,
            nodes: Vec::new(),
            node_by_id: HashMap::new(),
            function_by_span: HashMap::new(),
            main_var_by_identifier: HashMap::new(),
            main_var_by_declaration: HashMap::new(),
            edges: Vec::new(),
            edge_ids: BTreeSet::new(),
            global_unresolved: Vec::new(),
            global_unresolved_keys: BTreeSet::new(),
            diagnostics: Vec::new(),
        })
    }

    fn finish(mut self) -> Result<ProgramRoleGraph, RoleGraphError> {
        self.build_function_nodes()?;
        self.build_main_variable_nodes()?;
        self.build_class_nodes()?;
        self.apply_function_index_roles();
        self.apply_encoding_roles();
        self.apply_wiring_roles();
        self.finalize_missing_roles();
        self.retain_non_static_evidence();
        self.build_edges()?;
        self.add_context_unresolved();
        self.assign_value_identities();
        self.detect_role_identity_conflicts();
        self.finalize_signals_and_emitters();
        self.validate_node_unresolved_invariant()?;
        let coverage = compute_coverage(&self.nodes, &self.edges, &self.global_unresolved);
        let main = self
            .index
            .main
            .as_ref()
            .map(|main| main.function_id.clone());
        Ok(ProgramRoleGraph {
            schema_version: PROGRAM_ROLE_GRAPH_SCHEMA_VERSION,
            source: self.identity,
            main,
            nodes: self.nodes,
            edges: self.edges,
            coverage,
            unresolved: self.global_unresolved,
            diagnostics: self.diagnostics,
            runtime: RuntimeExecution::NotObserved,
            closure: SemanticClosure::NotClaimed,
        })
    }

    // -- node construction --------------------------------------------------

    fn build_function_nodes(&mut self) -> Result<(), RoleGraphError> {
        let index = self.index.clone();
        for function in &index.functions {
            if self.node_by_id.contains_key(&function.id) {
                return Err(RoleGraphError::InvalidContext {
                    context: "function_index",
                    message: format!("duplicate function node id {}", function.id),
                });
            }
            let ownership = self.function_ownership(function);
            let node = RoleGraphNode {
                id: function.id.clone(),
                kind: RoleGraphNodeKind::Function,
                label: function
                    .label
                    .clone()
                    .or_else(|| function.binding_label.clone()),
                span: function.full_span,
                identifier_span: function.identifier_span,
                ownership,
                lexical_parent: function.parent_function.clone(),
                roles: Vec::new(),
                held: None,
                held_proven_static: false,
                held_alternatives: Vec::new(),
                alias_of: None,
                signal_indices: function.signal_indices.clone(),
                wave_indices: function.wave_index.into_iter().collect(),
                emitters: Vec::new(),
                function_kind: Some(function.kind),
                class_id: function.class_id.clone(),
                value_identity: function.id.clone(),
                value_identity_path: Vec::new(),
                is_top_level_entry: function.is_top_level_entry,
                lexical_caller_count: function.caller_count,
                lexical_callee_count: function.callee_count,
                unresolved_call_count: function.unresolved_call_count,
                unresolved: Vec::new(),
                runtime: RuntimeExecution::NotObserved,
            };
            let position = self.nodes.len();
            self.nodes.push(node);
            self.node_by_id.insert(function.id.clone(), position);
            self.function_by_span
                .insert((function.full_span.start, function.full_span.end), position);
        }
        Ok(())
    }

    fn build_main_variable_nodes(&mut self) -> Result<(), RoleGraphError> {
        let index = self.index.clone();
        for variable in &index.main_variables {
            if self.node_by_id.contains_key(&variable.id) {
                return Err(RoleGraphError::InvalidContext {
                    context: "function_index",
                    message: format!("duplicate MAIN variable node id {}", variable.id),
                });
            }
            let node = RoleGraphNode {
                id: variable.id.clone(),
                kind: RoleGraphNodeKind::MainVariable,
                label: Some(variable.name.clone()),
                span: variable.declaration,
                identifier_span: Some(variable.identifier),
                ownership: SourceOwnership::MainOwnedBinding,
                lexical_parent: self
                    .index
                    .main
                    .as_ref()
                    .map(|main| main.function_id.clone()),
                roles: Vec::new(),
                held: Some(HeldClass::from(variable.classification)),
                held_proven_static: variable.held.proven_static,
                held_alternatives: variable.held.alternatives.clone(),
                alias_of: variable.alias_of.clone(),
                signal_indices: variable.signal_indices.clone(),
                wave_indices: variable.wave_indices.clone(),
                emitters: Vec::new(),
                function_kind: None,
                class_id: variable
                    .held
                    .proven_static
                    .then(|| variable.held.class_id.clone())
                    .flatten(),
                // Canonicalized in `assign_value_identities` once every node exists.
                value_identity: variable.id.clone(),
                value_identity_path: Vec::new(),
                is_top_level_entry: false,
                lexical_caller_count: 0,
                lexical_callee_count: 0,
                unresolved_call_count: 0,
                unresolved: Vec::new(),
                runtime: RuntimeExecution::NotObserved,
            };
            let position = self.nodes.len();
            self.nodes.push(node);
            self.node_by_id.insert(variable.id.clone(), position);
            self.main_var_by_identifier.insert(
                (variable.identifier.start, variable.identifier.end),
                position,
            );
            self.main_var_by_declaration.insert(
                (variable.declaration.start, variable.declaration.end),
                position,
            );
        }
        Ok(())
    }

    /// Class nodes follow the MAIN-variable nodes, so the function and MAIN
    /// node positions stay `0..functions` and `functions..functions+bindings`.
    fn build_class_nodes(&mut self) -> Result<(), RoleGraphError> {
        let index = self.index.clone();
        for class in &index.classes {
            if self.node_by_id.contains_key(&class.id) {
                return Err(RoleGraphError::InvalidContext {
                    context: "function_index",
                    message: format!("duplicate class node id {}", class.id),
                });
            }
            let ownership = if self.under_main(class.parent_function.as_deref()) {
                SourceOwnership::MainNestedClass
            } else if class.parent_function.is_none() {
                SourceOwnership::TopLevelClass
            } else {
                SourceOwnership::NestedClass
            };
            // An anonymous class expression keeps the label of the binding that
            // directly holds it, as function nodes keep `binding_label`.
            let label = class.label.clone().or_else(|| {
                class
                    .holders
                    .iter()
                    .find(|holder| holder.relation != super::function_index::HolderRelation::Alias)
                    .map(|holder| holder.binding.name.clone())
            });
            let node = RoleGraphNode {
                id: class.id.clone(),
                kind: RoleGraphNodeKind::Class,
                label,
                span: class.full_span,
                identifier_span: class.identifier_span,
                ownership,
                lexical_parent: class.parent_function.clone(),
                roles: Vec::new(),
                held: None,
                held_proven_static: false,
                held_alternatives: Vec::new(),
                alias_of: None,
                signal_indices: Vec::new(),
                wave_indices: Vec::new(),
                emitters: Vec::new(),
                function_kind: None,
                class_id: Some(class.id.clone()),
                value_identity: class.id.clone(),
                value_identity_path: Vec::new(),
                is_top_level_entry: false,
                lexical_caller_count: 0,
                lexical_callee_count: 0,
                unresolved_call_count: 0,
                unresolved: Vec::new(),
                runtime: RuntimeExecution::NotObserved,
            };
            let position = self.nodes.len();
            self.nodes.push(node);
            self.node_by_id.insert(class.id.clone(), position);
        }
        Ok(())
    }

    /// True when `parent` is the MAIN function or lexically nested in it.
    fn under_main(&self, parent: Option<&str>) -> bool {
        let main_id = self
            .index
            .main
            .as_ref()
            .map(|main| main.function_id.as_str());
        let mut current = parent.map(str::to_string);
        let mut guard = 0usize;
        while let Some(parent) = current {
            if Some(parent.as_str()) == main_id {
                return true;
            }
            current = self
                .index
                .functions
                .iter()
                .find(|candidate| candidate.id == parent)
                .and_then(|candidate| candidate.parent_function.clone());
            guard += 1;
            if guard > 4_096 {
                break;
            }
        }
        false
    }

    fn function_ownership(
        &self,
        function: &super::function_index::FunctionRecord,
    ) -> SourceOwnership {
        if function.is_main {
            return SourceOwnership::MainFunction;
        }
        if self.under_main(function.parent_function.as_deref()) {
            SourceOwnership::MainNestedFunction
        } else if function.parent_function.is_none() {
            SourceOwnership::TopLevelFunction
        } else {
            SourceOwnership::NestedFunction
        }
    }

    // -- role application ---------------------------------------------------

    fn apply_function_index_roles(&mut self) {
        let functions: Vec<(usize, Option<String>, Option<String>, Option<String>, bool)> = self
            .index
            .functions
            .iter()
            .enumerate()
            .map(|(position, function)| {
                (
                    position,
                    function.role.clone(),
                    function.role_basis.clone(),
                    function.declaration_context.clone(),
                    function.is_top_level_entry,
                )
            })
            .collect();
        for (position, role, basis, context, top_level) in functions {
            if self
                .index
                .main
                .as_ref()
                .map(|main| main.function_id.as_str())
                == Some(self.nodes[position].id.as_str())
            {
                self.add_role(
                    position,
                    RoleKind::MainEntry,
                    RoleEvidenceSource::FunctionIndexMainSelection,
                    "broadest reachable MAIN scope selected by the function index".to_string(),
                    Some(self.nodes[position].span),
                    Confidence::High,
                );
            }
            if top_level {
                self.add_role(
                    position,
                    RoleKind::TopLevelEntry,
                    RoleEvidenceSource::FunctionIndexTopLevelEntry,
                    "invoked directly at the program top level".to_string(),
                    Some(self.nodes[position].span),
                    Confidence::High,
                );
            }
            if let Some(role) = role {
                match function_role_kind(&role) {
                    Some(kind) => self.add_role(
                        position,
                        kind,
                        RoleEvidenceSource::FunctionIndexRole,
                        format!(
                            "function index role {role} ({})",
                            basis.unwrap_or_else(|| "no basis recorded".to_string())
                        ),
                        Some(self.nodes[position].span),
                        Confidence::High,
                    ),
                    None => self.push_diagnostic(
                        "unrecognized_function_role",
                        format!("function index role label {role} is not in the closed role set"),
                        Some(self.nodes[position].span),
                    ),
                }
            }
            if let Some(context) = context {
                let kind = declaration_context_role(&context);
                if let Some(kind) = kind {
                    self.add_role(
                        position,
                        kind,
                        RoleEvidenceSource::FunctionIndexDeclarationContext,
                        format!("source declaration context {context}"),
                        Some(self.nodes[position].span),
                        Confidence::Medium,
                    );
                }
            }
        }

        let variables: Vec<(usize, Option<String>)> = self
            .index
            .main_variables
            .iter()
            .enumerate()
            .map(|(offset, variable)| (self.index.functions.len() + offset, variable.role.clone()))
            .collect();
        for (position, role) in variables {
            if let Some(role) = role {
                match main_variable_role_kind(&role) {
                    Some(kind) => self.add_role(
                        position,
                        kind,
                        RoleEvidenceSource::FunctionIndexMainVariableRole,
                        format!("function index MAIN binding role {role}"),
                        Some(self.nodes[position].span),
                        Confidence::High,
                    ),
                    None => self.push_diagnostic(
                        "unrecognized_binding_role",
                        format!(
                            "function index MAIN binding role label {role} is not in the closed role set"
                        ),
                        Some(self.nodes[position].span),
                    ),
                }
            }
        }
    }

    fn apply_encoding_roles(&mut self) {
        let encoding = self.encoding.clone();
        if let Some(number) = &encoding.number {
            self.apply_function_evidence_role(
                &number.function,
                RoleKind::NumberWriter,
                "encoding.number_writer",
            );
        }
        if let Some(string) = &encoding.string {
            self.apply_function_evidence_role(
                &string.factory,
                RoleKind::StringFactory,
                "encoding.string_factory",
            );
            if let Some(writer) = &string.writer {
                self.apply_function_evidence_role(
                    writer,
                    RoleKind::StringWriter,
                    "encoding.string_writer",
                );
            }
            if let Some(cache) = &string.cache {
                self.apply_binding_evidence_role(
                    cache,
                    RoleKind::StringCache,
                    "encoding.string_cache",
                );
            }
            if let Some(counter) = &string.counter {
                self.apply_binding_evidence_role(
                    counter,
                    RoleKind::StringCounter,
                    "encoding.string_counter",
                );
            }
        }
        if let Some(stream) = &encoding.byte_stream {
            self.apply_function_evidence_role(
                &stream.factory,
                RoleKind::CreateSignalMask,
                "encoding.byte_stream_factory",
            );
            if let Some(cycle) = &stream.cycle {
                self.apply_function_evidence_role(
                    cycle,
                    RoleKind::ByteStreamCycle,
                    "encoding.byte_stream_cycle",
                );
            }
            for state in &stream.captured_state {
                if let Some(binding) = &state.binding {
                    self.apply_binding_evidence_role(
                        binding,
                        RoleKind::ByteStreamState,
                        "encoding.byte_stream_state",
                    );
                }
            }
        }
        if let Some(hash) = &encoding.seed_link.hash {
            self.apply_function_evidence_role(
                hash,
                RoleKind::HashTransactionToken,
                "encoding.seed_hash",
            );
        }
    }

    fn apply_function_evidence_role(
        &mut self,
        function: &FunctionEvidence,
        role: RoleKind,
        detail: &str,
    ) {
        match self.function_position(function.span) {
            Some(position) => self.add_role(
                position,
                role,
                RoleEvidenceSource::EncodingHelper,
                detail.to_string(),
                Some(function.span),
                Confidence::High,
            ),
            None => {
                self.push_global_unresolved(
                    format!("encoding.unmapped_function.{detail}"),
                    format!(
                        "encoding evidence span {}..{} does not match an indexed callable",
                        function.span.start, function.span.end
                    ),
                    Some(function.span),
                    true,
                );
            }
        }
    }

    fn apply_binding_evidence_role(
        &mut self,
        binding: &BindingEvidence,
        role: RoleKind,
        detail: &str,
    ) {
        match self.variable_position(binding.declaration, binding.identifier) {
            Some(position) => self.add_role(
                position,
                role,
                RoleEvidenceSource::EncodingBinding,
                detail.to_string(),
                Some(binding.identifier),
                Confidence::High,
            ),
            None => self.push_diagnostic(
                "context_binding_not_main_owned",
                format!(
                    "encoding {detail} binding {} is not a direct MAIN-owned binding",
                    binding.name
                ),
                Some(binding.identifier),
            ),
        }
    }

    fn apply_wiring_roles(&mut self) {
        let wiring = self.wiring.clone();
        if let Some(position) = self.function_position(wiring.main.span) {
            self.add_role(
                position,
                RoleKind::MainEntry,
                RoleEvidenceSource::WiringMain,
                "wiring pass MAIN function".to_string(),
                Some(wiring.main.span),
                Confidence::High,
            );
        }
        if let Some(descriptors) = &wiring.descriptors {
            self.apply_binding_ref_role(
                &descriptors.binding,
                RoleKind::DescriptorRegistry,
                RoleEvidenceSource::WiringDescriptorRegistry,
                "wiring descriptor registry binding",
            );
        }
        for group in &wiring.groups {
            let Some(position) = self.function_position(group.function.span) else {
                self.push_global_unresolved(
                    "wiring.unmapped_group_function",
                    format!(
                        "wiring group {} function span {}..{} does not match an indexed callable",
                        group.group_index, group.function.span.start, group.function.span.end
                    ),
                    Some(group.function.span),
                    true,
                );
                continue;
            };
            self.add_role(
                position,
                RoleKind::WaveCollector,
                RoleEvidenceSource::WiringGroup,
                format!(
                    "wiring group {} with source-derived row membership",
                    group.group_index
                ),
                Some(group.function.span),
                Confidence::High,
            );
            self.nodes[position]
                .signal_indices
                .extend(group.row.iter().copied());
            for emitter in &group.local_emitters {
                for call in &emitter.calls {
                    self.nodes[position]
                        .emitters
                        .push(RoleGraphEmitterEvidence {
                            local_position: emitter.local_position,
                            signal: emitter.global_signal,
                            method: call.method,
                            span: call.span,
                            receiver_source: call.receiver_source.clone(),
                            payload_source: call.payload_source.clone(),
                        });
                }
            }
        }
        for route in &wiring.direct_routes {
            if let Some(wrapper) = &route.ctor.wrapper {
                if let Some(position) = self.function_position(wrapper.span) {
                    self.add_role(
                        position,
                        RoleKind::DirectCollector,
                        RoleEvidenceSource::WiringDirectRoute,
                        format!("wiring direct route wrapper for signal {}", route.signal),
                        Some(wrapper.span),
                        Confidence::High,
                    );
                }
            }
            let Some(callback) = &route.ctor.callback else {
                self.push_global_unresolved(
                    "wiring.direct_route_without_callback",
                    format!(
                        "wiring direct route for signal {} has no resolved producer callback",
                        route.signal
                    ),
                    Some(route.ctor.expression.span),
                    true,
                );
                continue;
            };
            let Some(position) = self.function_position(callback.span) else {
                self.push_global_unresolved(
                    "wiring.unmapped_direct_callback",
                    format!(
                        "wiring direct route callback span {}..{} does not match an indexed callable",
                        callback.span.start, callback.span.end
                    ),
                    Some(callback.span),
                    true,
                );
                continue;
            };
            self.add_role(
                position,
                RoleKind::DirectSignalCallback,
                RoleEvidenceSource::WiringDirectRoute,
                format!("wiring direct route producer for signal {}", route.signal),
                Some(callback.span),
                Confidence::High,
            );
            self.nodes[position].signal_indices.push(route.signal);
            for emitter in &route.local_emitters {
                for call in &emitter.calls {
                    self.nodes[position]
                        .emitters
                        .push(RoleGraphEmitterEvidence {
                            local_position: emitter.local_position,
                            signal: emitter.global_signal.or(Some(route.signal)),
                            method: call.method,
                            span: call.span,
                            receiver_source: call.receiver_source.clone(),
                            payload_source: call.payload_source.clone(),
                        });
                }
            }
        }
        if let Some(coordinator) = &wiring.coordinator {
            if let Some(position) = self.function_position(coordinator.function.span) {
                self.add_role(
                    position,
                    RoleKind::Coordinator,
                    RoleEvidenceSource::WiringCoordinator,
                    "wiring coordinator function".to_string(),
                    Some(coordinator.function.span),
                    Confidence::High,
                );
            }
            for (binding, detail) in [
                (&coordinator.init_stored, "coordinator stored init"),
                (&coordinator.emit_stored, "coordinator stored emitter"),
                (
                    &coordinator.lastchance_stored,
                    "coordinator stored last-chance",
                ),
                (&coordinator.cleanup_stored, "coordinator stored cleanup"),
            ] {
                if let Some(binding) = binding {
                    self.apply_binding_ref_role(
                        binding,
                        RoleKind::CoordinatorState,
                        RoleEvidenceSource::WiringCoordinator,
                        detail,
                    );
                }
            }
            if let Some(noop) = &coordinator.noop {
                self.apply_binding_ref_role(
                    noop,
                    RoleKind::SharedNoop,
                    RoleEvidenceSource::WiringCoordinator,
                    "coordinator shared no-op",
                );
            }
        }
        let stores = &wiring.stores;
        if let Some(factory) = &stores.observer_factory {
            if let Some(position) = self.function_position(factory.span) {
                self.add_role(
                    position,
                    RoleKind::ObserverFactory,
                    RoleEvidenceSource::WiringStore,
                    "wiring observer factory".to_string(),
                    Some(factory.span),
                    Confidence::High,
                );
            }
        }
        if let Some(snapshot) = &stores.snapshot {
            if let Some(position) = self.function_position(snapshot.span) {
                self.add_role(
                    position,
                    RoleKind::SnapshotGetter,
                    RoleEvidenceSource::WiringStore,
                    "wiring snapshot getter".to_string(),
                    Some(snapshot.span),
                    Confidence::High,
                );
            }
        }
        if let Some(binding) = &stores.latest_cells {
            self.apply_binding_ref_role(
                binding,
                RoleKind::LatestValueStore,
                RoleEvidenceSource::WiringStore,
                "wiring latest-value store",
            );
        }
        if let Some(binding) = &stores.status_cells {
            self.apply_binding_ref_role(
                binding,
                RoleKind::StatusStore,
                RoleEvidenceSource::WiringStore,
                "wiring status store",
            );
        }
        if let Some(binding) = &stores.prefix_scratch {
            self.apply_binding_ref_role(
                binding,
                RoleKind::PrefixScratch,
                RoleEvidenceSource::WiringStore,
                "wiring prefix scratch binding",
            );
        }
        if let Some(notifier) = &stores.notifier {
            if let Some(position) = self.function_position(notifier.registrar.span) {
                self.add_role(
                    position,
                    RoleKind::Notifier,
                    RoleEvidenceSource::WiringNotifier,
                    "wiring notifier registrar".to_string(),
                    Some(notifier.registrar.span),
                    Confidence::High,
                );
            }
            if let Some(table) = &notifier.table {
                self.apply_binding_ref_role(
                    table,
                    RoleKind::NotifierTable,
                    RoleEvidenceSource::WiringNotifier,
                    "wiring notifier table",
                );
            }
        }
        if let Some(public_init) = &wiring.public_init {
            if let Some(position) = self.function_position(public_init.function.span) {
                self.add_role(
                    position,
                    RoleKind::PublicInit,
                    RoleEvidenceSource::WiringPublicInit,
                    "wiring public init function".to_string(),
                    Some(public_init.function.span),
                    Confidence::High,
                );
            }
            if let Some(token) = &public_init.token {
                if let Some(hash) = &token.hash_function {
                    if let Some(position) = self.function_position(hash.span) {
                        self.add_role(
                            position,
                            RoleKind::HashTransactionToken,
                            RoleEvidenceSource::WiringPublicInit,
                            "wiring public-init token hash".to_string(),
                            Some(hash.span),
                            Confidence::High,
                        );
                    }
                }
                if let Some(state) = &token.state_binding {
                    self.apply_binding_ref_role(
                        state,
                        RoleKind::SeedState,
                        RoleEvidenceSource::WiringPublicInit,
                        "wiring token state binding",
                    );
                }
            }
        }
    }

    fn apply_binding_ref_role(
        &mut self,
        binding: &BindingRef,
        role: RoleKind,
        source: RoleEvidenceSource,
        detail: &str,
    ) {
        match self.variable_position(binding.declaration, binding.identifier) {
            Some(position) => self.add_role(
                position,
                role,
                source,
                detail.to_string(),
                Some(binding.identifier),
                Confidence::High,
            ),
            None => self.push_diagnostic(
                "context_binding_not_main_owned",
                format!(
                    "{detail} binding {} is not a direct MAIN-owned binding",
                    binding.name
                ),
                Some(binding.identifier),
            ),
        }
    }

    // -- unknowns -----------------------------------------------------------

    fn finalize_missing_roles(&mut self) {
        for position in 0..self.nodes.len() {
            if self.nodes[position].kind == RoleGraphNodeKind::UnresolvedCallTarget {
                continue;
            }
            if !self.nodes[position].roles.is_empty() {
                continue;
            }
            let (fact, reason) = match self.nodes[position].kind {
                RoleGraphNodeKind::Function => {
                    let function = self
                        .index
                        .functions
                        .iter()
                        .find(|function| function.id == self.nodes[position].id);
                    let caller_count = function.map_or(0, |function| function.caller_count);
                    (
                        "role_not_proven".to_string(),
                        format!(
                            "no existing source-derived property or call/consumer role covers this callable; lexical_caller_count={caller_count} (surface caller absence is not deadness)"
                        ),
                    )
                }
                RoleGraphNodeKind::MainVariable => {
                    let held = self.nodes[position]
                        .held
                        .map(HeldClass::as_str)
                        .unwrap_or("unknown");
                    let alternatives = self.nodes[position].held_alternatives.len();
                    (
                        "role_not_proven".to_string(),
                        format!(
                            "no proven semantic purpose for this MAIN-owned binding; held shape is {held} with {alternatives} retained definitions (shape alone is not a role)"
                        ),
                    )
                }
                RoleGraphNodeKind::Class => (
                    "role_not_proven".to_string(),
                    "no existing source-derived property role covers this class value".to_string(),
                ),
                RoleGraphNodeKind::UnresolvedCallTarget => continue,
            };
            self.push_node_unresolved(position, fact, reason, Some(self.nodes[position].span));
        }
    }

    fn retain_non_static_evidence(&mut self) {
        for position in 0..self.nodes.len() {
            if self.nodes[position].kind != RoleGraphNodeKind::MainVariable {
                continue;
            }
            if self.nodes[position].held_proven_static {
                continue;
            }
            let held = self.nodes[position].held;
            if !matches!(held, Some(HeldClass::RuntimeState)) {
                continue;
            }
            let alternatives = self.nodes[position].held_alternatives.len();
            let classes: BTreeSet<String> = self.nodes[position]
                .held_alternatives
                .iter()
                .map(|alternative| format!("{:?}", alternative.class))
                .collect();
            self.push_node_unresolved(
                position,
                "value_not_static",
                format!(
                    "mutable/conditional definition is not a proven current value: {alternatives} definitions across [{}]",
                    classes.into_iter().collect::<Vec<_>>().join(", ")
                ),
                self.nodes[position]
                    .held_alternatives
                    .first()
                    .map(|alternative| alternative.span),
            );
        }
    }

    fn add_context_unresolved(&mut self) {
        if self.index.main.is_none() {
            self.push_global_unresolved(
                "function_index.main_scope",
                "no MAIN function was selected by the function index",
                None,
                true,
            );
        }
        for fact in &self.encoding.unresolved {
            self.push_global_unresolved(
                format!("encoding.{}", fact.fact),
                fact.reason.clone(),
                fact.span,
                fact.required,
            );
        }
        for fact in &self.wiring.unresolved {
            self.push_global_unresolved(
                format!("wiring.{}", fact.fact),
                fact.reason.clone(),
                fact.span,
                fact.required,
            );
        }
        for diagnostic in &self.index.diagnostics {
            self.diagnostics.push(diagnostic.clone());
        }
        for diagnostic in &self.wiring.diagnostics {
            self.diagnostics.push(Diagnostic {
                code: diagnostic.code.clone(),
                message: diagnostic.message.clone(),
                span: diagnostic.span,
            });
        }
    }

    // -- edges --------------------------------------------------------------

    fn build_edges(&mut self) -> Result<(), RoleGraphError> {
        self.build_lexical_edges();
        self.build_ownership_and_held_edges()?;
        self.build_class_member_edges();
        self.build_call_edges()?;
        self.build_main_reference_edges();
        Ok(())
    }

    fn build_lexical_edges(&mut self) {
        for position in 0..self.index.functions.len() {
            let (id, parent, span) = {
                let function = &self.index.functions[position];
                (
                    function.id.clone(),
                    function.parent_function.clone(),
                    function.full_span,
                )
            };
            if let Some(parent) = parent {
                if self.node_by_id.contains_key(&parent) {
                    self.push_edge(
                        RoleGraphEdgeKind::LexicallyContains,
                        parent,
                        id,
                        span,
                        "lexical containment (ownership only, not semantic proof)".to_string(),
                        None,
                    );
                }
            }
        }
    }

    fn build_ownership_and_held_edges(&mut self) -> Result<(), RoleGraphError> {
        let main_id = self
            .index
            .main
            .as_ref()
            .map(|main| main.function_id.clone());
        for position in 0..self.index.main_variables.len() {
            let (id, declaration, alias_of, held_function, held_class, static_value) = {
                let variable = &self.index.main_variables[position];
                (
                    variable.id.clone(),
                    variable.declaration,
                    variable.alias_of.clone(),
                    variable.held.function_id.clone(),
                    variable.held.class_id.clone(),
                    variable.held.proven_static,
                )
            };
            if let Some(main_id) = &main_id {
                if self.node_by_id.contains_key(main_id) {
                    self.push_edge(
                        RoleGraphEdgeKind::OwnsBinding,
                        main_id.clone(),
                        id.clone(),
                        declaration,
                        "MAIN function directly owns this binding".to_string(),
                        None,
                    );
                }
            }
            if let Some(alias_of) = alias_of {
                if self.node_by_id.contains_key(&alias_of) {
                    self.push_edge(
                        RoleGraphEdgeKind::AliasOf,
                        id.clone(),
                        alias_of,
                        declaration,
                        "single-definition source alias".to_string(),
                        None,
                    );
                } else {
                    self.push_global_unresolved(
                        "role_graph.unmapped_alias_target",
                        format!("alias target {alias_of} is not a MAIN variable node"),
                        Some(declaration),
                        true,
                    );
                }
            }
            if static_value {
                if let Some(class_id) = held_class {
                    if self.node_by_id.contains_key(&class_id) {
                        self.push_edge(
                            RoleGraphEdgeKind::HoldsClass,
                            id.clone(),
                            class_id,
                            declaration,
                            "class declaration or single unconditional definition holds this class"
                                .to_string(),
                            None,
                        );
                    } else {
                        self.push_global_unresolved(
                            "role_graph.unmapped_held_class",
                            format!("held class id {class_id} is not a class node"),
                            Some(declaration),
                            true,
                        );
                    }
                }
                if let Some(function_id) = held_function {
                    if self.node_by_id.contains_key(&function_id) {
                        self.push_edge(
                            RoleGraphEdgeKind::HoldsFunction,
                            id,
                            function_id,
                            declaration,
                            "single unconditional definition holds this function".to_string(),
                            None,
                        );
                    } else {
                        self.push_global_unresolved(
                            "role_graph.unmapped_held_function",
                            format!("held function id {function_id} is not a function node"),
                            Some(declaration),
                            true,
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn build_class_member_edges(&mut self) {
        for position in 0..self.index.classes.len() {
            let class = &self.index.classes[position];
            let class_id = class.id.clone();
            let members: Vec<(String, &'static str)> = class
                .constructor_id
                .iter()
                .map(|id| (id.clone(), "constructor"))
                .chain(class.method_ids.iter().map(|id| (id.clone(), "method")))
                .chain(class.getter_ids.iter().map(|id| (id.clone(), "getter")))
                .chain(class.setter_ids.iter().map(|id| (id.clone(), "setter")))
                .chain(
                    class
                        .field_function_ids
                        .iter()
                        .map(|id| (id.clone(), "field function")),
                )
                .collect();
            for (function_id, member) in members {
                let Some(&function_position) = self.node_by_id.get(&function_id) else {
                    let span = self.index.classes[position].full_span;
                    self.push_global_unresolved(
                        "role_graph.unmapped_class_member",
                        format!("class member {function_id} is not a function node"),
                        Some(span),
                        true,
                    );
                    continue;
                };
                let span = self.nodes[function_position].span;
                self.push_edge(
                    RoleGraphEdgeKind::ClassMember,
                    class_id.clone(),
                    function_id,
                    span,
                    format!("class {member}"),
                    None,
                );
            }
        }
    }

    fn build_call_edges(&mut self) -> Result<(), RoleGraphError> {
        let edge_count = self.index.call_edges.len();
        for position in 0..edge_count {
            let (caller, callee, span, detail, resolution) = {
                let edge = &self.index.call_edges[position];
                (
                    edge.caller.clone(),
                    edge.callee.clone(),
                    edge.span,
                    format!(
                        "{} call to {} (line {})",
                        match edge.kind {
                            super::function_index::CallKind::Call => "call",
                            super::function_index::CallKind::New => "new",
                        },
                        edge.callee_source,
                        edge.line
                    ),
                    Some(edge.resolution),
                )
            };
            let Some(caller) = caller else {
                // A program-top-level call has no function caller. It is kept in
                // the function index; the role graph only links function ->
                // function/target.
                continue;
            };
            if !self.node_by_id.contains_key(&caller) {
                self.push_global_unresolved(
                    "role_graph.unmapped_call_caller",
                    format!("call caller {caller} is not a graph node"),
                    Some(span),
                    true,
                );
                continue;
            }
            if !self.node_by_id.contains_key(&callee) {
                self.push_global_unresolved(
                    "role_graph.unmapped_call_callee",
                    format!("resolved call callee {callee} is not a graph node"),
                    Some(span),
                    true,
                );
                continue;
            }
            self.push_edge(
                RoleGraphEdgeKind::Calls,
                caller,
                callee,
                span,
                detail,
                resolution,
            );
        }

        let unresolved_count = self.index.unresolved_calls.len();
        for position in 0..unresolved_count {
            let (caller, span, reason, callee_source) = {
                let call = &self.index.unresolved_calls[position];
                (
                    call.caller.clone(),
                    call.span,
                    call.reason.clone(),
                    call.callee_source.clone(),
                )
            };
            let Some(caller) = caller else {
                self.push_global_unresolved(
                    "role_graph.top_level_unresolved_call",
                    format!(
                        "top-level unresolved call to {callee_source} ({reason}) has no function caller"
                    ),
                    Some(span),
                    true,
                );
                continue;
            };
            if !self.node_by_id.contains_key(&caller) {
                self.push_global_unresolved(
                    "role_graph.unmapped_unresolved_caller",
                    format!("unresolved-call caller {caller} is not a graph node"),
                    Some(span),
                    true,
                );
                continue;
            }
            let target_id = format!("uc@{}..{}", span.start, span.end);
            if !self.node_by_id.contains_key(&target_id) {
                let target = RoleGraphNode {
                    id: target_id.clone(),
                    kind: RoleGraphNodeKind::UnresolvedCallTarget,
                    label: Some(callee_source.clone()),
                    span,
                    identifier_span: None,
                    ownership: SourceOwnership::UnresolvedCallSite,
                    lexical_parent: None,
                    roles: Vec::new(),
                    held: None,
                    held_proven_static: false,
                    held_alternatives: Vec::new(),
                    alias_of: None,
                    signal_indices: Vec::new(),
                    wave_indices: Vec::new(),
                    emitters: Vec::new(),
                    function_kind: None,
                    class_id: None,
                    value_identity: target_id.clone(),
                    value_identity_path: Vec::new(),
                    is_top_level_entry: false,
                    lexical_caller_count: 0,
                    lexical_callee_count: 0,
                    unresolved_call_count: 0,
                    unresolved: vec![UnresolvedFact {
                        fact: "call_target".to_string(),
                        reason: format!(
                            "callee {callee_source} did not resolve to one function identity: {reason}"
                        ),
                        span: Some(span),
                        required: true,
                    }],
                    runtime: RuntimeExecution::NotObserved,
                };
                let node_position = self.nodes.len();
                self.nodes.push(target);
                self.node_by_id.insert(target_id.clone(), node_position);
            }
            self.push_edge(
                RoleGraphEdgeKind::UnresolvedCall,
                caller,
                target_id,
                span,
                format!("unresolved call to {callee_source} ({reason})"),
                None,
            );
        }
        Ok(())
    }

    fn build_main_reference_edges(&mut self) {
        for position in 0..self.index.functions.len() {
            let function_id = self.index.functions[position].id.clone();
            let reads = self.index.functions[position].main_reads.clone();
            for record in reads {
                if self.node_by_id.contains_key(&record.main_variable_id) {
                    self.push_edge(
                        RoleGraphEdgeKind::ReadsMainBinding,
                        function_id.clone(),
                        record.main_variable_id,
                        record.span,
                        format!("read of MAIN binding at line {}", record.line),
                        None,
                    );
                }
            }
            let writes = self.index.functions[position].main_writes.clone();
            for record in writes {
                if self.node_by_id.contains_key(&record.main_variable_id) {
                    self.push_edge(
                        RoleGraphEdgeKind::WritesMainBinding,
                        function_id.clone(),
                        record.main_variable_id,
                        record.span,
                        format!("write of MAIN binding at line {}", record.line),
                        None,
                    );
                }
            }
        }
    }

    // -- final validation ---------------------------------------------------

    /// Resolve every MAIN binding to its canonical proven value identity. A
    /// binding that holds a function or class takes that id. An alias follows
    /// `alias_of` only while each hop is a proven single unconditional
    /// definition whose target is initialized before the alias is written
    /// (a function declaration is initialized on entry). Anything else is its
    /// own value: a mutable or unproven binding never merges with another.
    fn assign_value_identities(&mut self) {
        let by_id: HashMap<&str, &MainVariableRecord> = self
            .index
            .main_variables
            .iter()
            .map(|variable| (variable.id.as_str(), variable))
            .collect();
        for position in 0..self.nodes.len() {
            if self.nodes[position].kind != RoleGraphNodeKind::MainVariable {
                continue;
            }
            let Some(start) = by_id.get(self.nodes[position].id.as_str()).copied() else {
                continue;
            };
            let mut current = start;
            let mut path: Vec<String> = Vec::new();
            let mut visited: BTreeSet<&str> = BTreeSet::new();
            let value = loop {
                if !visited.insert(current.id.as_str()) || !current.held.proven_static {
                    break current.id.clone();
                }
                let held_value = current
                    .held
                    .function_id
                    .as_ref()
                    .or(current.held.class_id.as_ref())
                    .filter(|id| self.node_by_id.contains_key(id.as_str()));
                if let Some(value) = held_value {
                    break value.clone();
                }
                let Some(target) = current
                    .alias_of
                    .as_deref()
                    .and_then(|target| by_id.get(target).copied())
                else {
                    break current.id.clone();
                };
                if !target.held.proven_static || !initialized_before(target, current) {
                    break current.id.clone();
                }
                path.push(target.id.clone());
                current = target;
            };
            // Only the hops that were actually followed belong in the path.
            if value == start.id {
                path.clear();
            }
            self.nodes[position].value_identity = value;
            self.nodes[position].value_identity_path = path;
        }
    }

    fn detect_role_identity_conflicts(&mut self) {
        for fact in role_identity_conflicts(&self.nodes) {
            self.push_global_unresolved(fact.fact, fact.reason, fact.span, fact.required);
        }
        // Holder/alias groups that canonicalized to one value stay visible as
        // diagnostics, so the merge is auditable.
        for (slot, kind, value, members) in canonicalized_role_groups(&self.nodes) {
            self.push_diagnostic(
                "role_identity_canonicalized",
                format!(
                    "unique role slot {slot} on {} nodes is one value {value} held by {}",
                    kind.as_str(),
                    members.join(", ")
                ),
                None,
            );
        }
    }

    fn finalize_signals_and_emitters(&mut self) {
        for node in &mut self.nodes {
            node.signal_indices.sort_unstable();
            node.signal_indices.dedup();
            node.wave_indices.sort_unstable();
            node.wave_indices.dedup();
            node.emitters
                .sort_by_key(|emitter| (emitter.span.start, emitter.span.end));
            node.emitters.dedup();
            node.roles.sort_by_key(|assignment| assignment.role);
            for assignment in &mut node.roles {
                assignment
                    .evidence
                    .sort_by_key(|evidence| (evidence.source, evidence.span));
                assignment.evidence.dedup();
            }
        }
    }

    fn validate_node_unresolved_invariant(&mut self) -> Result<(), RoleGraphError> {
        for node in &self.nodes {
            if node.kind == RoleGraphNodeKind::UnresolvedCallTarget {
                continue;
            }
            if node.roles.is_empty() && node.unresolved.is_empty() {
                return Err(RoleGraphError::Invariant(format!(
                    "{} node {} has neither a role assignment nor an explicit unresolved fact",
                    node.kind.as_str(),
                    node.id
                )));
            }
        }
        for edge in &self.edges {
            if !self.node_by_id.contains_key(&edge.from) {
                return Err(RoleGraphError::Invariant(format!(
                    "edge {} has dangling from {}",
                    edge.id, edge.from
                )));
            }
            if !self.node_by_id.contains_key(&edge.to) {
                return Err(RoleGraphError::Invariant(format!(
                    "edge {} has dangling to {}",
                    edge.id, edge.to
                )));
            }
        }
        if self.edge_ids.len() != self.edges.len() {
            return Err(RoleGraphError::Invariant(
                "edge ids are not unique".to_string(),
            ));
        }
        Ok(())
    }

    // -- low-level helpers --------------------------------------------------

    fn add_role(
        &mut self,
        position: usize,
        role: RoleKind,
        source: RoleEvidenceSource,
        detail: String,
        span: Option<SourceSpan>,
        confidence: Confidence,
    ) {
        self.nodes[position].add_role(
            role,
            confidence,
            RoleEvidence {
                source,
                detail,
                span,
            },
        );
    }

    fn push_node_unresolved(
        &mut self,
        position: usize,
        fact: impl Into<String>,
        reason: impl Into<String>,
        span: Option<SourceSpan>,
    ) {
        let fact = fact.into();
        let reason = reason.into();
        if self.nodes[position]
            .unresolved
            .iter()
            .any(|existing| existing.fact == fact && existing.reason == reason)
        {
            return;
        }
        self.nodes[position].unresolved.push(UnresolvedFact {
            fact,
            reason,
            span,
            required: true,
        });
    }

    fn push_global_unresolved(
        &mut self,
        fact: impl Into<String>,
        reason: impl Into<String>,
        span: Option<SourceSpan>,
        required: bool,
    ) {
        let fact = fact.into();
        let reason = reason.into();
        let (start, end) = span.map_or((0, 0), |span| (span.start, span.end));
        if !self
            .global_unresolved_keys
            .insert((fact.clone(), reason.clone(), start, end))
        {
            return;
        }
        self.global_unresolved.push(UnresolvedFact {
            fact,
            reason,
            span,
            required,
        });
    }

    fn push_diagnostic(
        &mut self,
        code: impl Into<String>,
        message: impl Into<String>,
        span: Option<SourceSpan>,
    ) {
        let code = code.into();
        let message = message.into();
        if self
            .diagnostics
            .iter()
            .any(|existing| existing.code == code && existing.message == message)
        {
            return;
        }
        self.diagnostics.push(Diagnostic {
            code,
            message,
            span,
        });
    }

    fn push_edge(
        &mut self,
        kind: RoleGraphEdgeKind,
        from: String,
        to: String,
        span: SourceSpan,
        detail: String,
        call_resolution: Option<CallResolution>,
    ) {
        let id = format!(
            "edge@{}@{}->{}@{}..{}",
            kind.as_str(),
            from,
            to,
            span.start,
            span.end
        );
        if !self.edge_ids.insert(id.clone()) {
            return;
        }
        self.edges.push(RoleGraphEdge {
            id,
            kind,
            from,
            to,
            span,
            detail,
            call_resolution,
        });
    }

    fn function_position(&self, span: SourceSpan) -> Option<usize> {
        self.function_by_span.get(&(span.start, span.end)).copied()
    }

    fn variable_position(&self, declaration: SourceSpan, identifier: SourceSpan) -> Option<usize> {
        self.main_var_by_identifier
            .get(&(identifier.start, identifier.end))
            .copied()
            .or_else(|| {
                self.main_var_by_declaration
                    .get(&(declaration.start, declaration.end))
                    .copied()
            })
    }
}

// ===========================================================================
// Validation
// ===========================================================================

// ===========================================================================
// Value identity and unique roles
// ===========================================================================

/// Prefix of the context-level fact raised when one unique role slot covers
/// more than one value identity.
const ROLE_IDENTITY_CONFLICT_FACT: &str = "role_graph.role_identity_conflict.";

/// The node's canonical value identity, falling back to its own id for a
/// report deserialized without one.
fn node_value_identity(node: &RoleGraphNode) -> &str {
    if node.value_identity.is_empty() {
        &node.id
    } else {
        &node.value_identity
    }
}

/// Nodes carrying each unique role, grouped by `(slot, node kind)` and then by
/// canonical value identity.
fn unique_role_groups(
    nodes: &[RoleGraphNode],
) -> BTreeMap<(&'static str, RoleGraphNodeKind), BTreeMap<String, BTreeSet<(String, SourceSpan)>>> {
    let mut groups: BTreeMap<
        (&'static str, RoleGraphNodeKind),
        BTreeMap<String, BTreeSet<(String, SourceSpan)>>,
    > = BTreeMap::new();
    for node in nodes {
        for assignment in &node.roles {
            if !assignment.role.unique_identity() {
                continue;
            }
            let Some(slot) = assignment.role.slot() else {
                continue;
            };
            groups
                .entry((slot, node.kind))
                .or_default()
                .entry(node_value_identity(node).to_string())
                .or_default()
                .insert((node.id.clone(), node.span));
        }
    }
    groups
}

/// A unique role slot is a conflict only when its nodes resolve to two or more
/// distinct value identities. A holder and its own immutable alias are one value.
fn role_identity_conflicts(nodes: &[RoleGraphNode]) -> Vec<UnresolvedFact> {
    let mut conflicts = Vec::new();
    for ((slot, kind), values) in unique_role_groups(nodes) {
        if values.len() < 2 {
            continue;
        }
        let detail = values
            .iter()
            .map(|(value, members)| {
                let members = members
                    .iter()
                    .map(|(id, span)| format!("{id}@{}..{}", span.start, span.end))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{value} [{members}]")
            })
            .collect::<Vec<_>>()
            .join("; ");
        let span = values
            .values()
            .flat_map(|members| members.iter().map(|(_, span)| *span))
            .min();
        conflicts.push(UnresolvedFact {
            fact: format!("{ROLE_IDENTITY_CONFLICT_FACT}{slot}.{}", kind.as_str()),
            reason: format!(
                "unique role slot {slot} on {} nodes resolves to {} distinct value identities: {detail}",
                kind.as_str(),
                values.len()
            ),
            span,
            required: true,
        });
    }
    conflicts
}

/// Unique role slots held by several nodes that are all one canonical value.
fn canonicalized_role_groups(
    nodes: &[RoleGraphNode],
) -> Vec<(&'static str, RoleGraphNodeKind, String, Vec<String>)> {
    let mut groups = Vec::new();
    for ((slot, kind), values) in unique_role_groups(nodes) {
        if values.len() != 1 {
            continue;
        }
        let Some((value, members)) = values.into_iter().next() else {
            continue;
        };
        if members.len() < 2 {
            continue;
        }
        groups.push((
            slot,
            kind,
            value,
            members.into_iter().map(|(id, _)| id).collect(),
        ));
    }
    groups
}

/// True when `target`'s value exists before `alias` is written: a function
/// declaration is initialized on entry to its scope; any other target's single
/// definition must end before the alias's definition starts.
fn initialized_before(target: &MainVariableRecord, alias: &MainVariableRecord) -> bool {
    if target.declaration_kind == BindingDeclarationKind::Function {
        return true;
    }
    match (target.held.source, alias.held.source) {
        (Some(target), Some(alias)) => target.end <= alias.start,
        _ => false,
    }
}

fn role_graph_identity(source: &str) -> SourceIdentity {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    let digest = hasher.finalize();
    let mut sha256 = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(&mut sha256, "{byte:02x}");
    }
    SourceIdentity {
        sha256,
        bytes: source.len(),
    }
}

fn validate_identity(
    context: &'static str,
    expected: &SourceIdentity,
    actual: &SourceIdentity,
) -> Result<(), RoleGraphError> {
    if expected.bytes != actual.bytes || expected.sha256 != actual.sha256 {
        return Err(RoleGraphError::IdentityMismatch {
            context,
            expected: expected.clone(),
            actual: actual.clone(),
        });
    }
    Ok(())
}

fn validate_context(
    source: &str,
    index: &FunctionIndex,
    encoding: &EncodingProperties,
    wiring: &WiringProperties,
) -> Result<(), RoleGraphError> {
    if index.schema_version != FUNCTION_INDEX_SCHEMA_VERSION {
        return Err(RoleGraphError::InvalidContext {
            context: "function_index",
            message: format!(
                "unsupported schema version {} (expected {})",
                index.schema_version, FUNCTION_INDEX_SCHEMA_VERSION
            ),
        });
    }
    if encoding.schema_version != ENCODING_PROPERTIES_SCHEMA_VERSION {
        return Err(RoleGraphError::InvalidContext {
            context: "encoding_properties",
            message: format!(
                "unsupported schema version {} (expected {})",
                encoding.schema_version, ENCODING_PROPERTIES_SCHEMA_VERSION
            ),
        });
    }
    if wiring.schema_version != PROGRAM_WIRING_SCHEMA_VERSION {
        return Err(RoleGraphError::InvalidContext {
            context: "wiring_properties",
            message: format!(
                "unsupported schema version {} (expected {})",
                wiring.schema_version, PROGRAM_WIRING_SCHEMA_VERSION
            ),
        });
    }
    let len = source.len();
    for function in &index.functions {
        validate_span("function_index.function", Some(function.full_span), len)?;
        validate_span("function_index.function.body", function.body_span, len)?;
        validate_span(
            "function_index.function.identifier",
            function.identifier_span,
            len,
        )?;
    }
    for variable in &index.main_variables {
        validate_span(
            "function_index.main_variable",
            Some(variable.declaration),
            len,
        )?;
        validate_span(
            "function_index.main_variable.identifier",
            Some(variable.identifier),
            len,
        )?;
    }
    for class in &index.classes {
        validate_span("function_index.class", Some(class.full_span), len)?;
        validate_span(
            "function_index.class.identifier",
            class.identifier_span,
            len,
        )?;
    }
    for edge in &index.call_edges {
        validate_span("function_index.call_edge", Some(edge.span), len)?;
    }
    for call in &index.unresolved_calls {
        validate_span("function_index.unresolved_call", Some(call.span), len)?;
    }
    validate_span("wiring.main", Some(wiring.main.span), len)?;
    validate_span(
        "encoding.number",
        encoding.number.as_ref().map(|value| value.function.span),
        len,
    )?;
    validate_span(
        "encoding.string",
        encoding.string.as_ref().map(|value| value.factory.span),
        len,
    )?;
    validate_span(
        "encoding.byte_stream",
        encoding
            .byte_stream
            .as_ref()
            .map(|value| value.factory.span),
        len,
    )?;
    validate_span(
        "encoding.hash",
        encoding.seed_link.hash.as_ref().map(|value| value.span),
        len,
    )?;
    Ok(())
}

fn validate_span(
    context: &'static str,
    span: Option<SourceSpan>,
    source_len: usize,
) -> Result<(), RoleGraphError> {
    let Some(span) = span else {
        return Ok(());
    };
    if span.start >= span.end || span.end as usize > source_len {
        return Err(RoleGraphError::InvalidContext {
            context,
            message: format!(
                "span {}..{} is empty or outside the {} byte source",
                span.start, span.end, source_len
            ),
        });
    }
    Ok(())
}

// ===========================================================================
// Mapping from existing role labels to the closed graph role set
// ===========================================================================

fn max_confidence(left: Confidence, right: Confidence) -> Confidence {
    if confidence_rank(right) > confidence_rank(left) {
        right
    } else {
        left
    }
}

fn confidence_rank(value: Confidence) -> u8 {
    match value {
        Confidence::High => 3,
        Confidence::Medium => 2,
        Confidence::Low => 1,
        Confidence::Unresolved => 0,
    }
}

fn function_role_kind(label: &str) -> Option<RoleKind> {
    let kind = match label {
        "MainEntry" => RoleKind::MainEntry,
        "DescriptorRegistry" => RoleKind::DescriptorRegistry,
        "WaveRegistry" => RoleKind::WaveRegistry,
        "WaveIndexRows" => RoleKind::WaveIndexRows,
        "WaveRoutingSelector" => RoleKind::WaveRoutingSelector,
        "WaveCollector" => RoleKind::WaveCollector,
        "DirectCollector" => RoleKind::DirectCollector,
        "DirectSignalCallback" => RoleKind::DirectSignalCallback,
        "SignalCollector" => RoleKind::SignalCollector,
        "WriteNumber" | "NumberWriter" => RoleKind::NumberWriter,
        "WriteString" | "StringFactory" => RoleKind::StringFactory,
        "StringWriter" => RoleKind::StringWriter,
        "CreateSignalMask" => RoleKind::CreateSignalMask,
        "HashTransactionToken" => RoleKind::HashTransactionToken,
        "ObserverFactory" => RoleKind::ObserverFactory,
        "SnapshotGetter" => RoleKind::SnapshotGetter,
        "EnvelopeBuilder" => RoleKind::EnvelopeBuilder,
        "SerializerFactory" => RoleKind::SerializerFactory,
        "DefaultEncoder" => RoleKind::DefaultEncoder,
        "SessionConstructor" => RoleKind::SessionConstructor,
        "SessionInit" => RoleKind::SessionInit,
        "SessionTeardown" => RoleKind::SessionTeardown,
        "ContextConfig" => RoleKind::ContextConfig,
        "DefaultSignalFilter" => RoleKind::DefaultSignalFilter,
        "ErrorRingConstructor" => RoleKind::ErrorRingConstructor,
        _ => return None,
    };
    Some(kind)
}

fn main_variable_role_kind(label: &str) -> Option<RoleKind> {
    let kind = match label {
        "MainEntry" => RoleKind::MainEntry,
        "DescriptorRegistry" => RoleKind::DescriptorRegistry,
        "WaveRegistry" => RoleKind::WaveRegistry,
        "WaveIndexRows" => RoleKind::WaveIndexRows,
        "WaveRoutingSelector" => RoleKind::WaveRoutingSelector,
        "WaveCollector" | "SignalCollector" => RoleKind::WaveCollector,
        "DirectCollector" => RoleKind::DirectCollector,
        "WriteNumber" | "NumberWriter" => RoleKind::NumberWriter,
        "WriteString" | "StringFactory" => RoleKind::StringFactory,
        "CreateSignalMask" => RoleKind::CreateSignalMask,
        "HashTransactionToken" => RoleKind::HashTransactionToken,
        "ObserverFactory" => RoleKind::ObserverFactory,
        "SnapshotGetter" => RoleKind::SnapshotGetter,
        "EnvelopeBuilder" => RoleKind::EnvelopeBuilder,
        "SerializerFactory" => RoleKind::SerializerFactory,
        "DefaultEncoder" => RoleKind::DefaultEncoder,
        "SessionConstructor" => RoleKind::SessionConstructor,
        "SessionInit" => RoleKind::SessionInit,
        "SessionTeardown" => RoleKind::SessionTeardown,
        "ContextConfig" => RoleKind::ContextConfig,
        "LatestValueStore" => RoleKind::LatestValueStore,
        "StatusStore" => RoleKind::StatusStore,
        "DebugNameStore" => RoleKind::DebugNameStore,
        "ErrorRingBuffer" => RoleKind::ErrorRingBuffer,
        "ErrorRingConstructor" => RoleKind::ErrorRingConstructor,
        "DefaultSignalFilter" => RoleKind::DefaultSignalFilter,
        "SentinelObject" => RoleKind::SentinelObject,
        "TagTable" => RoleKind::TagTable,
        _ => return None,
    };
    Some(kind)
}

fn declaration_context_role(context: &str) -> Option<RoleKind> {
    match context {
        "call_argument" => Some(RoleKind::CallbackArgument),
        "new_argument" => Some(RoleKind::ConstructorArgument),
        "returned_function" => Some(RoleKind::ReturnedFunction),
        _ => None,
    }
}

// ===========================================================================
// Coverage
// ===========================================================================

fn compute_coverage(
    nodes: &[RoleGraphNode],
    edges: &[RoleGraphEdge],
    global_unresolved: &[UnresolvedFact],
) -> RoleGraphCoverage {
    let mut coverage = RoleGraphCoverage {
        total_node_count: nodes.len(),
        total_edge_count: edges.len(),
        ..RoleGraphCoverage::default()
    };
    for node in nodes {
        match node.kind {
            RoleGraphNodeKind::Function => {
                coverage.function_node_count += 1;
                if node.roles.is_empty() {
                    coverage.functions_without_roles += 1;
                } else {
                    coverage.functions_with_roles += 1;
                }
            }
            RoleGraphNodeKind::MainVariable => {
                coverage.main_variable_node_count += 1;
                if node.roles.is_empty() {
                    coverage.main_variables_without_roles += 1;
                } else {
                    coverage.main_variables_with_roles += 1;
                }
            }
            RoleGraphNodeKind::UnresolvedCallTarget => {
                coverage.unresolved_call_target_count += 1;
            }
            RoleGraphNodeKind::Class => {
                coverage.class_node_count += 1;
            }
        }
        if !node.unresolved.is_empty() {
            coverage.nodes_with_explicit_unknowns += 1;
        }
        if node.roles.is_empty()
            && node.unresolved.is_empty()
            && node.kind != RoleGraphNodeKind::UnresolvedCallTarget
        {
            coverage.nodes_missing_role_and_unknown += 1;
        }
        *coverage
            .ownership_counts
            .entry(node.ownership.as_str().to_string())
            .or_default() += 1;
        if let Some(held) = node.held {
            *coverage
                .held_class_counts
                .entry(held.as_str().to_string())
                .or_default() += 1;
        }
        for assignment in &node.roles {
            let bucket = match node.kind {
                RoleGraphNodeKind::Function => &mut coverage.function_role_counts,
                RoleGraphNodeKind::MainVariable => &mut coverage.main_variable_role_counts,
                RoleGraphNodeKind::Class => &mut coverage.class_role_counts,
                RoleGraphNodeKind::UnresolvedCallTarget => continue,
            };
            *bucket
                .entry(assignment.role.as_str().to_string())
                .or_default() += 1;
            for evidence in &assignment.evidence {
                *coverage
                    .evidence_source_counts
                    .entry(evidence.source.as_str().to_string())
                    .or_default() += 1;
            }
        }
    }
    for edge in edges {
        *coverage
            .edge_kind_counts
            .entry(edge.kind.as_str().to_string())
            .or_default() += 1;
    }
    coverage.unresolved_count = nodes
        .iter()
        .map(|node| node.unresolved.len())
        .sum::<usize>()
        + global_unresolved.len();
    coverage.nodes_and_context_unknowns =
        coverage.nodes_with_explicit_unknowns + global_unresolved.len();
    coverage
}
