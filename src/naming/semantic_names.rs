//! Descriptive, structure-proven naming of a recovered program capture.
//!
//! This module turns structural evidence into a *source-stamped naming plan*:
//! a list of proposed renames, each addressed by the exact [`SourceSpan`] of the
//! declaring identifier, with the original name, the resolved [`BindingKind`], a
//! role label, a [`Confidence`], and the structural clues that established it.
//!
//! # What it is
//!
//! The core [`crate::signals`] extractor already proves the
//! generator-input *roles* (descriptor registry, wave wiring, session
//! constructor, stores, and the number/string/PRNG/hash/observer helpers) and
//! records each as a resolved lexical binding plus exact spans. This module
//! reads those proofs, adds fresh structural recognizers for roles the core
//! matcher does not model, and produces descriptive names for the *function*
//! that performs an action and the *main-owned alias* that holds it.
//!
//! # What it deliberately is not
//!
//! * It never keys a fact on a generated, `$`-prefixed spelling, a numeric
//!   signal id, a per-capture literal, a header token, a seed word or a table
//!   count. Those are navigation only.
//! * It never invents a numbered fallback name. A role with more than one
//!   equally plausible binding stays unnamed and is reported as ambiguous.
//!   Matching ancestors are pruned only by lexical containment; byte length,
//!   identifier spelling and source order are never tie-breakers.
//! * It never keeps a silent first winner when one binding matches two
//!   different roles: the binding is reported as a `role_conflict` ambiguity
//!   and keeps its original spelling.
//! * It never renames an ordinary local, a parameter, a `catch` binding or a
//!   temporary. Only a proven function identity and an important main-owned
//!   alias are candidates.
//! * It reads no report JSON at runtime; the analysis batch findings are
//!   hypotheses that the structural predicates here must re-establish from the
//!   source alone.
//!
//! # Fail-closed binding origin
//!
//! [`ProgramProperties`]' `symbol_value`/`function_from_symbol` return an
//! initializer or the last source assignment. That is a *candidate value*, not
//! an immutable identity. Before a property-derived or structural role becomes a
//! rename proof this module re-derives the binding's declaration origin from a
//! fresh Oxc scope and refuses a certain role when the binding has conflicting
//! declarations (every declaration origin of a symbol is counted, so sloppy
//! duplicate function declarations and repeated `var` initializers are
//! conflicts), reassignments, a compound/update write, or a mutated alias.
//! Function identities must be function-valued; non-function candidates must be
//! ordinary variables owned by the top main scope with a single source origin.
//!
//! # Identity across parses
//!
//! The properties report comes from the scanner's own parse. Its symbol
//! ordinals are never compared with this module's parse. A report binding is
//! joined by exact source identity instead: the report's SHA-256 must equal the
//! current source, and each identifier span must be in bounds, on UTF-8 char
//! boundaries, spell the recorded name, and be the declaring identifier of
//! exactly one symbol in the current parse. A binding that fails any check is
//! refused and recorded; valid byte spans are authoritative and never repaired.
//!
//! # Ownership of structural facts
//!
//! A role predicate reads the function's *owned implementation*: its own
//! statements plus the inline callbacks it hands to calls. A nested function
//! that escapes (it is returned, stored in a binding or a property) is a
//! separate entity and does not lend its facts to the enclosing function, so a
//! large function never matches a role just because some descendant does.
//! Class-shaped roles read only the class (or constructor function) that the
//! function itself declares and returns.
//!
//! # What a name means
//!
//! Every candidate's name asserts one structurally proven *role*. It is
//! naming-ready, not a claim that the function's whole behavior is understood;
//! the serialized plan says so in [`NAMING_CONTRACT`] and in each candidate's
//! [`BehaviorCoverage`].

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, NodeId, ScopeFlags, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use serde::Serialize;

use crate::analysis;

use crate::signals::extract::bindings::{object_members, property_key_name, ParameterSlots};
use crate::signals::extract::scanner::extract_program_properties;
use crate::signals::schema::{BindingKind, BindingRef, Confidence, ProgramProperties, ProgramPropertiesError, SourceIdentity, SourceSpan};

/// What every proposed name asserts, and what it does not.
pub const NAMING_CONTRACT: &str = "a proposed name asserts one structurally proven role \
(single origin, function identity, no ambiguity or role conflict); it does not claim that \
the entity's complete behavior is understood";

/// Where a role's proof comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleSource {
    /// A binding the properties report proved, re-proven here.
    Properties,
    /// A structural predicate of this module.
    Structural,
}

/// The one naming vocabulary: every role this module can name and the single
/// name it receives. `interception_record` is the only role named from its
/// proven target (`<target>InterceptionRecord`), and the single main alias of a
/// data container gets `<role>_alias` / `<name>Alias`. A claim that proposes a
/// different name for a listed role is refused.
pub const NAMING_VOCABULARY: &[(&str, &str, RoleSource)] = &[
    ("agent_module", "createAgentModule", RoleSource::Properties),
    (
        "session_runtime",
        "createSessionRuntime",
        RoleSource::Properties,
    ),
    (
        "session_runtime_constructor",
        "sessionRuntimeConstructor",
        RoleSource::Properties,
    ),
    ("session_context", "sessionContext", RoleSource::Properties),
    (
        "session_runtime_init",
        "initializeSessionRuntime",
        RoleSource::Properties,
    ),
    (
        "session_bootstrap",
        "bootstrapSessionRuntime",
        RoleSource::Properties,
    ),
    (
        "transaction_token_hash_seed",
        "transactionTokenHashSeed",
        RoleSource::Properties,
    ),
    ("write_number", "writeNumber", RoleSource::Properties),
    (
        "number_field_writer",
        "numberFieldWriter",
        RoleSource::Properties,
    ),
    (
        "string_intern_writer",
        "createStringInternWriter",
        RoleSource::Properties,
    ),
    (
        "string_intern_writer_factory",
        "stringInternWriterFactory",
        RoleSource::Properties,
    ),
    (
        "prng_byte_source",
        "createPrngByteSource",
        RoleSource::Properties,
    ),
    (
        "prng_byte_source_factory",
        "prngByteSourceFactory",
        RoleSource::Properties,
    ),
    (
        "string_buffer_hash",
        "hashStringOrBuffer",
        RoleSource::Properties,
    ),
    (
        "string_buffer_hash_alias",
        "stringBufferHash",
        RoleSource::Properties,
    ),
    (
        "signal_observer",
        "createSignalObserver",
        RoleSource::Properties,
    ),
    (
        "signal_observer_factory",
        "signalObserverFactory",
        RoleSource::Properties,
    ),
    (
        "signal_report_builder",
        "buildSignalReport",
        RoleSource::Properties,
    ),
    (
        "signal_report_builder_alias",
        "signalReportBuilder",
        RoleSource::Properties,
    ),
    (
        "signal_descriptor_registry",
        "signalDescriptorRegistry",
        RoleSource::Properties,
    ),
    (
        "wave_collector_factories",
        "waveCollectorFactories",
        RoleSource::Properties,
    ),
    (
        "wave_signal_position_map",
        "waveSignalPositionMap",
        RoleSource::Properties,
    ),
    (
        "wave_routing_flags",
        "waveRoutingFlags",
        RoleSource::Properties,
    ),
    (
        "signal_value_slots",
        "signalValueSlots",
        RoleSource::Properties,
    ),
    (
        "signal_status_slots",
        "signalStatusSlots",
        RoleSource::Properties,
    ),
    (
        "signal_debug_names",
        "signalDebugNames",
        RoleSource::Properties,
    ),
    (
        "error_ring_buffer",
        "errorRingBuffer",
        RoleSource::Properties,
    ),
    (
        "empty_signal_sentinel",
        "emptySignalSentinel",
        RoleSource::Properties,
    ),
    (
        "encrypt_string_aes_gcm",
        "encryptStringAesGcm",
        RoleSource::Structural,
    ),
    (
        "aes_gcm_string_encryptor",
        "aesGcmStringEncryptor",
        RoleSource::Structural,
    ),
    (
        "decrypt_packed_aes_gcm_blob",
        "decryptPackedAesGcmBlob",
        RoleSource::Structural,
    ),
    (
        "packed_aes_gcm_blob_decryptor",
        "packedAesGcmBlobDecryptor",
        RoleSource::Structural,
    ),
    (
        "dispatch_hash_by_type",
        "dispatchHashByType",
        RoleSource::Structural,
    ),
    ("hash_dispatch", "hashDispatch", RoleSource::Structural),
    (
        "stream_cipher_factory",
        "createChaChaLikeStreamCipher",
        RoleSource::Structural,
    ),
    ("stream_cipher", "streamCipher", RoleSource::Structural),
    (
        "observable_class",
        "createObservableClass",
        RoleSource::Structural,
    ),
    ("observable", "Observable", RoleSource::Structural),
    (
        "install_method_interceptor",
        "installMethodInterceptor",
        RoleSource::Structural,
    ),
    (
        "method_interceptor_installer",
        "methodInterceptorInstaller",
        RoleSource::Structural,
    ),
    (
        "install_accessor_interceptor",
        "installAccessorInterceptor",
        RoleSource::Structural,
    ),
    (
        "accessor_interceptor_installer",
        "accessorInterceptorInstaller",
        RoleSource::Structural,
    ),
    (
        "dispatch_signal_status",
        "dispatchSignalStatus",
        RoleSource::Structural,
    ),
    (
        "signal_status_dispatcher",
        "signalStatusDispatcher",
        RoleSource::Structural,
    ),
    (
        "bootstrap_signal_engine",
        "bootstrapSignalEngine",
        RoleSource::Structural,
    ),
    (
        "signal_engine_bootstrapper",
        "signalEngineBootstrapper",
        RoleSource::Structural,
    ),
    (
        "interception_record",
        "<target>InterceptionRecord",
        RoleSource::Structural,
    ),
    (
        "interceptor_specs",
        "interceptorSpecs",
        RoleSource::Structural,
    ),
    ("crypto_subtle", "cryptoSubtle", RoleSource::Structural),
    (
        "resolved_global_object",
        "resolvedGlobalObject",
        RoleSource::Structural,
    ),
    ("host_window", "hostWindow", RoleSource::Structural),
    (
        "instrumentation_window",
        "instrumentationWindow",
        RoleSource::Structural,
    ),
    ("document_ref", "documentRef", RoleSource::Structural),
    (
        "dom_field_collector",
        "collectDomFields",
        RoleSource::Structural,
    ),
    (
        "dom_signal_source_factory",
        "createDomSignalSource",
        RoleSource::Structural,
    ),
    (
        "first_child_text_reader",
        "readFirstChildText",
        RoleSource::Structural,
    ),
];

/// The vocabulary name of `role`, when the role is listed with a fixed name.
///
/// The purpose finders' roles ([`crate::signals::analysis::roles::purposes`]) are part of the same
/// vocabulary: `<role>` names the function and `<role>_alias` its single holder
/// alias.
pub fn vocabulary_name(role: &str) -> Option<&'static str> {
    NAMING_VOCABULARY
        .iter()
        .find(|(listed, _, _)| *listed == role)
        .map(|(_, name, _)| *name)
        .filter(|name| !name.starts_with('<'))
        .or_else(|| crate::signals::analysis::roles::finder_role(role).map(|entry| entry.name))
        .or_else(|| {
            crate::signals::analysis::roles::purposes::purpose_vocabulary().find_map(|(listed, action, alias)| {
                if listed == role {
                    Some(action)
                } else if role.strip_suffix(ALIAS_ROLE_SUFFIX) == Some(listed) {
                    alias
                } else {
                    None
                }
            })
        })
}

/// One raw package-finder claim, before the plan's proofs (for tests).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct FinderAuditEntry {
    pub package: String,
    pub role: String,
    /// Whether the role is listed in the package vocabulary.
    pub listed: bool,
}

/// Run every package finder on `source` and list each raw claim.
pub fn finder_audit(source: &str) -> Result<Vec<FinderAuditEntry>, SemanticNamesError> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source).map_err(SemanticNamesError::Parse)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let ctx = crate::signals::analysis::roles::FinderCtx {
        source,
        program: &program,
        scoping: &scoping,
        nodes: &nodes,
    };
    Ok(crate::signals::analysis::roles::audit(&ctx)
        .into_iter()
        .map(|(package, role, listed)| FinderAuditEntry {
            package: package.to_string(),
            role: role.to_string(),
            listed,
        })
        .collect())
}

/// Suffix of the alias role of a purpose-finder role.
const ALIAS_ROLE_SUFFIX: &str = "_alias";

/// A failure that prevented a naming plan from being produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SemanticNamesError {
    /// Structural extraction refused the input.
    Properties(ProgramPropertiesError),
    /// Oxc refused the source.
    Parse(String),
    /// The properties report was derived from a different source.
    SourceMismatch { report: String, source: String },
}

impl std::fmt::Display for SemanticNamesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Properties(error) => write!(f, "semantic naming failed: {error}"),
            Self::Parse(message) => write!(f, "semantic naming parse failed: {message}"),
            Self::SourceMismatch { report, source } => write!(
                f,
                "semantic naming refused: properties report sha256 {report} does not match source sha256 {source}"
            ),
        }
    }
}

impl std::error::Error for SemanticNamesError {}

impl From<ProgramPropertiesError> for SemanticNamesError {
    fn from(value: ProgramPropertiesError) -> Self {
        Self::Properties(value)
    }
}

/// What kind of program entity a name is proposed for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NamingEntityKind {
    /// A function node; the editable binding is its declaration, its single
    /// holder, or its own name.
    Function,
    /// A class declaration or expression. Class identities are refused today
    /// (their binding is not an ordinary variable or function); the kind is
    /// reserved so a proposal never mislabels one as a function.
    Class,
    /// A top-main-owned ordinary variable with a single source origin.
    MainData,
    /// A main-owned binding whose single origin is another proven function.
    AliasHolder,
}

/// The program entity a proposal names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NamingEntity {
    pub kind: NamingEntityKind,
    /// The function node (functions), or the declaring node (data, holders).
    pub span: SourceSpan,
    /// Stable source-derived id: `fn@start..end` for a function node and
    /// `mv@start..end` (declaration span) for a main binding, matching the
    /// function index.
    pub id: String,
    /// For an alias holder: the `fn@` id of the function it holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

/// How much of an entity's behavior a name claims.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BehaviorCoverage {
    /// The name asserts one structurally proven role only; the entity's
    /// complete behavior is not claimed to be understood.
    RoleOnly,
}

/// One proposed descriptive rename, addressed by its declaring identifier.
///
/// `original_name`, `binding_kind`, `identifier` and `declaration` together
/// are the *editable binding*; `entity` is the program entity it names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NamingCandidate {
    /// Stable machine label of the proven role, e.g. `signal_descriptor_registry`.
    pub role: String,
    /// The program entity the name is for.
    pub entity: NamingEntity,
    /// The generated name as it appears in the source (a label, not identity).
    pub original_name: String,
    /// The descriptive replacement name.
    pub proposed_name: String,
    /// The resolved symbol ordinal in *this plan's* parse. It is a same-parse
    /// handle only and is never serialized or compared across parses.
    #[serde(skip)]
    pub symbol: u32,
    /// How the binding was declared.
    pub binding_kind: BindingKind,
    /// The declared identifier: the rewrite engine's address.
    pub identifier: SourceSpan,
    /// The whole declaring node.
    pub declaration: SourceSpan,
    /// How strongly the role is established.
    pub confidence: Confidence,
    /// The structural clues that established the role.
    pub evidence: Vec<String>,
    /// Naming readiness is not behavior understanding.
    pub behavior: BehaviorCoverage,
}

/// A binding or entity involved in an ambiguity or refusal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InvolvedSpan {
    /// The source spelling, when one applies (a label, not identity).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// A declaring identifier, a function node or a declaration span.
    pub span: SourceSpan,
    /// `fn@`/`mv@` id where one applies.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// A role an equal-evidence tie left unnamed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NamingAmbiguity {
    pub role: String,
    pub reason: String,
    /// Human labels of the tied bindings (name and identifier span).
    pub candidates: Vec<String>,
    /// The tied bindings or entities, by span.
    pub involved: Vec<InvolvedSpan>,
}

/// Why a role has no candidate, in machine-readable form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipKind {
    /// A fail-closed refusal: the role's binding or one of its proofs did not
    /// validate (mutable or duplicated binding, refused span, name mismatch).
    Refused,
    /// The role *is* recognized, on a function entity that has no editable
    /// binding: an anonymous inline callback, a method, or a function
    /// expression nothing holds. The entry is role metadata only; its
    /// `involved` span is the function node (`fn@` id). Nothing is renamed.
    RecognizedNoEditableBinding,
    /// Nothing in the capture proved the role (entries of `unproven`).
    Unproven,
}

/// The reason text of a [`SkipKind::RecognizedNoEditableBinding`] entry.
pub const NO_EDITABLE_BINDING_REASON: &str = "recognized role, no editable binding: the function \
is anonymous (an inline callback, a method or an unheld expression); role metadata only";

/// Purpose-finder roles retired from the naming plan.
///
/// The purpose finders still prove and report these roles, but another package
/// names every function they match under a more specific role. Claiming the
/// generic role too would tie two structurally equal functions that receive
/// different names from their consumers, so the plan retires it instead of
/// reporting a false same-name ambiguity.
const RETIRED_PURPOSE_ROLES: &[&str] = &["once_callback_factory"];

/// A role that was recognized structurally but deliberately not named: a
/// fail-closed refusal, or a recognized function without an editable binding
/// ([`SkipKind`] tells them apart).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NamingSkip {
    pub role: String,
    /// Machine-readable class of the entry.
    pub kind: SkipKind,
    pub reason: String,
    /// The refused binding or entity, when known.
    pub involved: Vec<InvolvedSpan>,
}

/// The complete naming plan for one exact capture.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SemanticNamingPlan {
    /// Exact-source identity of the input the plan was derived from.
    pub source: SourceIdentity,
    /// What a proposed name asserts ([`NAMING_CONTRACT`]).
    pub contract: String,
    /// Proposed renames, sorted by identifier offset.
    pub candidates: Vec<NamingCandidate>,
    /// Roles left unnamed because two bindings matched equally well.
    pub ambiguous: Vec<NamingAmbiguity>,
    /// Roles recognized but intentionally not renamed: refusals and
    /// recognized functions without an editable binding (see [`SkipKind`]).
    pub skipped: Vec<NamingSkip>,
    /// Vocabulary roles with no candidate, ambiguity or refusal: nothing in
    /// this capture proved them. Every vocabulary role is accounted for in
    /// exactly one of `candidates`, `ambiguous`, `skipped` or `unproven`.
    pub unproven: Vec<NamingSkip>,
    /// Role labels for which a descriptive name was produced.
    pub supported_roles: Vec<String>,
    /// Free-form notes about surfaces that remain unknown.
    pub notes: Vec<String>,
    /// Package-finder names for functions that have no editable binding
    /// (anonymous callbacks and arrows). They are identified and named here but
    /// never renamed by the identifier rename; see `docs/naming/naming.md`.
    pub anonymous_functions: Vec<AnonymousFunctionName>,
}

/// A proven name for a function with no editable binding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AnonymousFunctionName {
    pub role: String,
    pub proposed_name: String,
    /// The function node span (`fn@start..end`).
    pub function: SourceSpan,
    pub id: String,
    /// Whether the node is an arrow function (arrows cannot take a name
    /// without moving or wrapping them).
    pub arrow: bool,
    pub confidence: Confidence,
    pub evidence: Vec<String>,
}

impl SemanticNamingPlan {
    /// Number of proposed renames.
    pub fn proposed(&self) -> usize {
        self.candidates.len()
    }

    /// Number of roles left ambiguous.
    pub fn ambiguous_count(&self) -> usize {
        self.ambiguous.len()
    }

    /// True when the plan proposes no rename.
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    /// Entries for a fail-closed refusal: a role whose binding or one of its
    /// proofs did not validate. A [`SkipKind::RecognizedNoEditableBinding`]
    /// entry is *not* a refusal.
    pub fn refusals(&self) -> impl Iterator<Item = &NamingSkip> {
        self.skipped
            .iter()
            .filter(|skip| skip.kind == SkipKind::Refused)
    }

    /// Entries for a recognized role on a function without an editable
    /// binding: role metadata, never a rename.
    pub fn metadata_only(&self) -> impl Iterator<Item = &NamingSkip> {
        self.skipped
            .iter()
            .filter(|skip| skip.kind == SkipKind::RecognizedNoEditableBinding)
    }

    /// Every role this capture recognizes: the named candidates' roles, the
    /// metadata-only roles, and the anonymous functions' proposed roles.
    pub fn recognized_roles(&self) -> BTreeSet<String> {
        self.candidates
            .iter()
            .map(|candidate| candidate.role.clone())
            .chain(self.metadata_only().map(|skip| skip.role.clone()))
            .chain(self.anonymous_functions.iter().map(|entry| entry.role.clone()))
            .collect()
    }
}

/// Build the naming plan for exact program source.
///
/// When the properties extractor refuses the capture as ambiguous (for example
/// two equal session constructors), the plan degrades per role instead of
/// failing: the refused structure is recorded as an ambiguity, MAIN and its
/// bindings come from the independent function index, every role that needs
/// the report is listed as unproven, and the structural roles are kept.
pub fn plan_semantic_names(source: &str) -> Result<SemanticNamingPlan, SemanticNamesError> {
    match extract_program_properties(source) {
        Ok(properties) => plan_semantic_names_with_properties(source, &properties),
        Err(ProgramPropertiesError::Ambiguous { what, candidates }) => {
            plan_without_properties(source, what, &candidates)
        }
        Err(error) => Err(error.into()),
    }
}

/// The degraded plan for a capture whose properties report was refused.
fn plan_without_properties(
    source: &str,
    what: &'static str,
    candidates: &[String],
) -> Result<SemanticNamingPlan, SemanticNamesError> {
    let index = crate::naming::function_index::extract_function_index(source)
        .map_err(|error| SemanticNamesError::Parse(error.to_string()))?;
    let properties = degraded_properties(source, &index);
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source).map_err(SemanticNamesError::Parse)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut recognizer = Recognizer::new(source, &properties, &program, &scoping, &nodes);
    let role = match what {
        "main program scope (equal broadest reachable binding set)" => "agent_module",
        "descriptor registry" => "signal_descriptor_registry",
        "session constructor" => "session_runtime",
        _ if what.starts_with("wave registry") => "wave_collector_factories",
        _ => "properties_report",
    };
    let involved = candidates
        .iter()
        .flat_map(|candidate| spans_in(candidate))
        .map(|span| InvolvedSpan {
            name: None,
            span,
            id: None,
        })
        .collect();
    recognizer.record_ambiguity(
        role,
        &format!(
            "the properties report refused the capture: more than one {what} matched; roles that need the report are unproven"
        ),
        involved,
    );
    recognizer.degraded = Some(what);
    recognizer.run();
    recognizer.claim_purposes(&program);
    recognizer.claim_finders(&program);
    Ok(recognizer.finish())
}

/// A properties shell carrying only what the function index proves without
/// the report: the source identity, MAIN and MAIN's direct bindings.
fn degraded_properties(source: &str, index: &crate::naming::function_index::FunctionIndex) -> ProgramProperties {
    let main = index.main.as_ref();
    let binding = main.and_then(|main| {
        Some(BindingRef {
            symbol: 0,
            name: main.label.clone()?,
            declaration: main.full_span,
            identifier: main.identifier_span?,
            owned_by_main: false,
        })
    });
    let bindings = index
        .main_variables
        .iter()
        .map(|variable| crate::signals::schema::BindingRecord {
            id: 0,
            name: variable.name.clone(),
            declaration: variable.declaration,
            identifier: variable.identifier,
            kind: BindingKind::Other,
            role: crate::signals::schema::BindingRole::Unknown,
            references: variable.references,
            writes: variable.writes,
            initializer: None,
        })
        .collect::<Vec<_>>();
    ProgramProperties {
        schema_version: crate::signals::schema::PROGRAM_PROPERTIES_SCHEMA_VERSION,
        source: SourceIdentity {
            sha256: crate::source::sha256_hex(source.as_bytes()),
            bytes: source.len(),
        },
        program_scope: crate::signals::schema::ProgramScope {
            main: crate::signals::schema::FunctionRef {
                name_label: binding.as_ref().map(|binding| binding.name.clone()),
                binding,
                span: main.map(|main| main.full_span).unwrap_or_default(),
                body: main.and_then(|main| main.body_span),
                source: None,
            },
            main_argument: None,
            owned_binding_count: bindings.len(),
            returns_init_object: false,
            top_level_call: None,
            host_event_bridge: None,
        },
        bindings,
        helpers: Vec::new(),
        descriptors: None,
        waves: None,
        session: None,
        signals: Vec::new(),
        host_probes: Vec::new(),
        constants: Vec::new(),
        static_captures: Vec::new(),
        external_inputs: Vec::new(),
        unresolved: Vec::new(),
        diagnostics: Vec::new(),
        coverage: Default::default(),
    }
}

/// Every `start..end` byte range written in an extractor candidate label.
fn spans_in(text: &str) -> Vec<SourceSpan> {
    let mut out = Vec::new();
    for token in text.split(|c: char| !(c.is_ascii_digit() || c == '.')) {
        if let Some((start, end)) = token.split_once("..") {
            if let (Ok(start), Ok(end)) = (start.parse(), end.parse()) {
                out.push(SourceSpan::new(start, end));
            }
        }
    }
    out
}

/// Build the naming plan from an already extracted properties report.
///
/// The report must describe exactly `source` (equal SHA-256 and length), or the
/// plan is refused. Report bindings are joined to this parse by validated
/// identifier spans, never by symbol ordinal.
pub fn plan_semantic_names_with_properties(
    source: &str,
    properties: &ProgramProperties,
) -> Result<SemanticNamingPlan, SemanticNamesError> {
    plan_semantic_names_with_claims(source, properties, &[])
}

impl<'p, 'a> Recognizer<'p, 'a> {
    fn finish(mut self) -> SemanticNamingPlan {
        // Built-in recognizers report an anonymous function as a metadata-only
        // skip. A role in the one vocabulary is a *named* anonymous function
        // instead, exactly as the package finders already do: record it in
        // `anonymous_functions` and drop the skip, so it is never both.
        self.name_anonymous_builtins();
        let mut candidates = self.candidates;
        candidates.sort_by_key(|candidate| candidate.identifier.start);
        candidates.dedup_by_key(|candidate| candidate.symbol);

        let mut supported_roles: Vec<String> = Vec::new();
        for candidate in &candidates {
            if !supported_roles.contains(&candidate.role) {
                supported_roles.push(candidate.role.clone());
            }
        }
        supported_roles.sort();

        let accounted = |role: &str| {
            candidates.iter().any(|candidate| candidate.role == role)
                || self.skipped.iter().any(|skip| skip.role == role)
                || self.ambiguous.iter().any(|ambiguity| {
                    ambiguity.role == role
                        || ambiguity
                            .role
                            .strip_prefix("role_conflict:")
                            .is_some_and(|pair| pair.split('|').any(|listed| listed == role))
                })
        };
        let purpose_roles = crate::signals::analysis::roles::purposes::purpose_vocabulary()
            .filter(|(role, _, _)| !RETIRED_PURPOSE_ROLES.contains(role))
            .map(|(role, _, _)| (role, RoleSource::Structural))
            .collect::<Vec<_>>();
        let finder_roles = crate::signals::analysis::roles::vocabulary()
            .filter(|(_, entry)| entry.kind != crate::signals::analysis::roles::FinderRoleKind::FunctionAlias)
            .map(|(_, entry)| (entry.role, RoleSource::Structural))
            .collect::<Vec<_>>();
        let accounted = |role: &str| {
            accounted(role) || self.anonymous.iter().any(|named| named.role == role)
        };
        let unproven = NAMING_VOCABULARY
            .iter()
            .map(|(role, _, source)| (*role, *source))
            .chain(purpose_roles)
            .chain(finder_roles)
            .filter(|(role, _)| !accounted(role))
            .map(|(role, source)| NamingSkip {
                role: role.to_string(),
                kind: SkipKind::Unproven,
                reason: match (source, self.degraded) {
                    (RoleSource::Properties, Some(what)) => format!(
                        "needs the properties report, which refused the capture (more than one {what})"
                    ),
                    (RoleSource::Properties, None) => "the properties report proves no unique binding for this role (absent, or refused by the extractor as duplicated)".to_string(),
                    (RoleSource::Structural, _) => "no function or binding proved this role's structure (or its anchors are unproven)".to_string(),
                },
                involved: Vec::new(),
            })
            .collect();

        SemanticNamingPlan {
            source: self.properties.source.clone(),
            contract: NAMING_CONTRACT.to_string(),
            candidates,
            ambiguous: self.ambiguous,
            skipped: self.skipped,
            unproven,
            supported_roles,
            notes: self.notes,
            anonymous_functions: self.anonymous,
        }
    }

    /// Turn every remaining built-in `RecognizedNoEditableBinding` skip whose
    /// role has a vocabulary name into an [`AnonymousFunctionName`], and remove
    /// the skip. The package-finder path already does this for its own claims;
    /// this gives the built-in recognizers (core roles, purpose finders and the
    /// DOM family) the same treatment instead of recording their functions as
    /// role metadata only.
    ///
    /// The skip must carry exactly one involved span and that span must be a
    /// known function node in this parse; anything else stays a skip.
    fn name_anonymous_builtins(&mut self) {
        let mut kept = Vec::with_capacity(self.skipped.len());
        for skip in std::mem::take(&mut self.skipped) {
            if skip.kind != SkipKind::RecognizedNoEditableBinding {
                kept.push(skip);
                continue;
            }
            let Some(proposed_name) = vocabulary_name(&skip.role) else {
                kept.push(skip);
                continue;
            };
            let span = match skip.involved.as_slice() {
                [involved] => involved.span,
                _ => {
                    kept.push(skip);
                    continue;
                }
            };
            let Some(arrow) = self
                .fns
                .iter()
                .find(|facts| facts.span.start == span.start && facts.span.end == span.end)
                .map(|facts| facts.is_arrow)
            else {
                kept.push(skip);
                continue;
            };
            let already = self.anonymous.iter().any(|named| {
                named.role == skip.role
                    && named.function.start == span.start
                    && named.function.end == span.end
            });
            if !already {
                self.anonymous.push(AnonymousFunctionName {
                    role: skip.role.clone(),
                    proposed_name: proposed_name.to_string(),
                    function: span,
                    id: fn_id(span),
                    arrow,
                    confidence: Confidence::Medium,
                    evidence: vec![
                        "recognized by the built-in roles; no editable binding".to_string()
                    ],
                });
            }
        }
        self.skipped = kept;
    }
}

// ===========================================================================
// Structural index
// ===========================================================================

/// Per-function structural facts.
///
/// "Direct" attributes are the function's own statements. "Owned" attributes
/// (filled by [`Recognizer::new`]) add the inline callbacks the function hands
/// to calls, recursively; a nested function that escapes is a separate entity
/// and lends nothing. Role predicates read direct or owned facts only.
#[derive(Clone, Debug)]
struct FnFacts {
    span: Span,
    /// The `Function` / `ArrowFunctionExpression` node: the owner a
    /// [`ParameterSlots`] slot names.
    node: NodeId,
    symbol: Option<SymbolId>,
    enclosing: Option<usize>,
    nested: Vec<usize>,
    is_arrow: bool,
    /// No formal parameters and no rest parameter.
    nullary: bool,
    /// An ordinary function whose own `arguments` object is referenced
    /// (directly or from a nested arrow): it reads inputs without formals.
    uses_arguments: bool,
    /// A `function f() {}` declaration (its own name is its binding).
    is_declaration: bool,
    /// A function or arrow expression written directly as a call argument.
    inline_callback: bool,
    /// Set by [`Recognizer::new`]: this function is an owned callback of its
    /// enclosing function (inline, or a declaration that is only handed to
    /// calls and never escapes).
    callback: bool,
    direct_members: BTreeSet<String>,
    direct_strings: BTreeSet<String>,
    /// Property names *written* at this function's own level: object-literal
    /// keys and member assignment targets.
    direct_written_keys: BTreeSet<String>,
    /// Member names actually *called* at this function's own level.
    direct_calls: BTreeSet<String>,
    /// Identifier symbols passed as direct call arguments (a deferred-step
    /// factory hands off the nested function this way).
    direct_call_args: BTreeSet<u32>,
    /// Identifier symbols passed as call arguments anywhere inside.
    all_call_args: BTreeSet<u32>,
    /// Symbols that escape anywhere inside: returned, stored in a binding, or
    /// stored as a property value.
    escaping: BTreeSet<u32>,
    /// Identifier symbols returned at this function's own level.
    direct_returned: BTreeSet<u32>,
    /// Direct `object[key] = <query result>` writes (query -> field flow).
    direct_query_field_writes: usize,
    direct_refs: BTreeSet<u32>,
    owned_members: BTreeSet<String>,
    owned_strings: BTreeSet<String>,
    owned_written_keys: BTreeSet<String>,
    owned_calls: BTreeSet<String>,
    owned_refs: BTreeSet<u32>,
    direct_xors: usize,
    all_xors: usize,
    all_while_loops: usize,
    /// Nested functions that prove real ARX state/mixer flow: they XOR a wide
    /// state inside a loop.
    nested_mixers: usize,
    /// A direct-level `return <function-valued sibling>`.
    returns_nested_function: bool,
    /// Own parameter slots tested with `typeof <slot> === "string"`, as the
    /// shared [`ParameterSlots`] resolver proves them (a named formal and
    /// `arguments[N]` are one slot).
    typeof_string_slots: BTreeSet<u32>,
    /// Own parameter slots passed to `isView(...)`.
    is_view_slots: BTreeSet<u32>,
    /// A direct-level `new TypeError()`.
    new_type_error: bool,
}

impl FnFacts {
    fn new(
        span: Span,
        node: NodeId,
        symbol: Option<SymbolId>,
        enclosing: Option<usize>,
        nullary: bool,
    ) -> Self {
        Self {
            span,
            node,
            symbol,
            enclosing,
            nested: Vec::new(),
            is_arrow: false,
            nullary,
            uses_arguments: false,
            is_declaration: false,
            inline_callback: false,
            callback: false,
            direct_members: BTreeSet::new(),
            direct_strings: BTreeSet::new(),
            direct_written_keys: BTreeSet::new(),
            direct_calls: BTreeSet::new(),
            direct_call_args: BTreeSet::new(),
            all_call_args: BTreeSet::new(),
            escaping: BTreeSet::new(),
            direct_returned: BTreeSet::new(),
            direct_query_field_writes: 0,
            direct_refs: BTreeSet::new(),
            owned_members: BTreeSet::new(),
            owned_strings: BTreeSet::new(),
            owned_written_keys: BTreeSet::new(),
            owned_calls: BTreeSet::new(),
            owned_refs: BTreeSet::new(),
            direct_xors: 0,
            all_xors: 0,
            all_while_loops: 0,
            nested_mixers: 0,
            returns_nested_function: false,
            typeof_string_slots: BTreeSet::new(),
            is_view_slots: BTreeSet::new(),
            new_type_error: false,
        }
    }

    /// A factory that takes no inputs in either capture representation: no
    /// formals and no use of its own `arguments` object.
    fn takes_no_inputs(&self) -> bool {
        self.nullary && !self.uses_arguments
    }

    /// Seed the owned facts with the function's own statements.
    fn seed_owned(&mut self) {
        self.owned_members = self.direct_members.clone();
        self.owned_strings = self.direct_strings.clone();
        self.owned_written_keys = self.direct_written_keys.clone();
        self.owned_calls = self.direct_calls.clone();
        self.owned_refs = self.direct_refs.clone();
    }

    fn absorb_owned(&mut self, callback: &FnFacts) {
        self.owned_members
            .extend(callback.owned_members.iter().cloned());
        self.owned_strings
            .extend(callback.owned_strings.iter().cloned());
        self.owned_written_keys
            .extend(callback.owned_written_keys.iter().cloned());
        self.owned_calls
            .extend(callback.owned_calls.iter().cloned());
        self.owned_refs.extend(callback.owned_refs.iter().copied());
    }
}

/// A class declaration or expression and the members it declares itself.
#[derive(Clone, Debug)]
struct ClassFacts {
    span: Span,
    symbol: Option<SymbolId>,
    /// The function whose own body declares the class.
    enclosing: Option<usize>,
    /// Method, accessor and field names of the class body.
    members: BTreeSet<String>,
    /// The constructor method's function node.
    constructor: Option<Span>,
}

/// `S.key = ...` or `S.prototype.key = ...` written at one function's level.
#[derive(Clone, Debug)]
struct KeyWrite {
    owner: Option<usize>,
    target: u32,
    key: String,
}

struct Builder<'a> {
    scoping: &'a Scoping,
    slots: ParameterSlots<'a, 'a>,
    fns: Vec<FnFacts>,
    classes: Vec<ClassFacts>,
    key_writes: Vec<KeyWrite>,
    stack: Vec<usize>,
    fn_of_symbol: BTreeMap<u32, usize>,
    /// Function/arrow expressions written directly as call arguments.
    callback_spans: BTreeSet<(u32, u32)>,
    /// Property names read directly on a symbol (`S["x"]` / `S.x`).
    member_refs: BTreeMap<u32, BTreeSet<String>>,
}

impl<'a> Builder<'a> {
    fn new(program: &Program<'a>, scoping: &'a Scoping, nodes: &'a AstNodes<'a>) -> Self {
        Self {
            scoping,
            slots: ParameterSlots::new(program, scoping, nodes),
            fns: Vec::new(),
            classes: Vec::new(),
            key_writes: Vec::new(),
            stack: Vec::new(),
            fn_of_symbol: BTreeMap::new(),
            callback_spans: BTreeSet::new(),
            member_refs: BTreeMap::new(),
        }
    }

    fn for_each(&mut self, attribute: impl Fn(&mut FnFacts)) {
        for index in &self.stack {
            attribute(&mut self.fns[*index]);
        }
    }

    fn innermost(&mut self, attribute: impl Fn(&mut FnFacts)) {
        if let Some(index) = self.stack.last().copied() {
            attribute(&mut self.fns[index]);
        }
    }

    /// Count a just-finished nested function as an ARX mixer when it XORs a
    /// wide state inside a loop. The enclosing factory inherits the count, so a
    /// cipher role is never inferred from a bare member/string bag.
    fn finish_function(&mut self, index: usize, enclosing: Option<usize>) {
        let facts = &self.fns[index];
        let is_mixer = facts.direct_xors >= 4 && facts.all_xors >= 6 && facts.all_while_loops >= 1;
        if is_mixer {
            if let Some(enclosing) = enclosing {
                self.fns[enclosing].nested_mixers += 1;
            }
        }
    }

    /// The function on the stack whose own parameter slot `expression` reads,
    /// with the slot index. The read must be in that function's own code or
    /// in inline callbacks it hands to calls; a read from an escaping nested
    /// function lends nothing.
    fn own_slot(&self, expression: &Expression<'a>) -> Option<(usize, u32)> {
        let slot = self.slots.incoming_slot(expression)?;
        for &index in self.stack.iter().rev() {
            if self.fns[index].node == slot.owner {
                return Some((index, slot.index));
            }
            if !self.fns[index].inline_callback {
                return None;
            }
        }
        None
    }

    fn record_call(&mut self, name: String) {
        self.innermost(|facts| {
            facts.direct_calls.insert(name.clone());
        });
    }

    fn record_callback_arguments(&mut self, arguments: &[Argument<'a>]) {
        for argument in arguments {
            if let Some(
                Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_),
            ) = argument.as_expression()
            {
                let span = argument.span();
                self.callback_spans.insert((span.start, span.end));
            }
        }
    }

    /// Mark an identifier value as escaping: it is stored or returned, so the
    /// function it names is a separate entity rather than an owned callback.
    fn record_escape(&mut self, expression: &Expression<'a>) {
        if let Expression::Identifier(identifier) = expression {
            if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
                let index = symbol.index() as u32;
                self.for_each(|facts| {
                    facts.escaping.insert(index);
                });
            }
        }
    }

    fn is_member_target(&self, target: &AssignmentTarget<'a>) -> bool {
        matches!(
            target,
            AssignmentTarget::StaticMemberExpression(_)
                | AssignmentTarget::ComputedMemberExpression(_)
        )
    }

    fn record_written_key(&mut self, name: String) {
        self.innermost(|facts| {
            facts.direct_written_keys.insert(name.clone());
        });
    }

    /// Record `S.key = ...` / `S.prototype.key = ...` against `S`.
    fn record_key_write(&mut self, object: &Expression<'a>, key: &str) {
        let base = match object {
            Expression::Identifier(identifier) => Some(identifier),
            _ => match member_access(object) {
                Some((Expression::Identifier(identifier), property)) if property == "prototype" => {
                    Some(identifier)
                }
                _ => None,
            },
        };
        let Some(base) = base else {
            return;
        };
        if let Some(symbol) = analysis::identifier_symbol(base, self.scoping) {
            self.key_writes.push(KeyWrite {
                owner: self.stack.last().copied(),
                target: symbol.index() as u32,
                key: key.to_string(),
            });
        }
    }

    fn record_member(&mut self, object: &Expression<'a>, name: &str) {
        let owned = name.to_string();
        self.innermost(|facts| {
            facts.direct_members.insert(owned.clone());
        });
        if let Expression::Identifier(identifier) = object {
            if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
                self.member_refs
                    .entry(symbol.index() as u32)
                    .or_default()
                    .insert(name.to_string());
            }
        }
    }
}

impl<'a> Visit<'a> for Builder<'a> {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        let enclosing = self.stack.last().copied();
        let symbol = function.id.as_ref().and_then(|id| id.symbol_id.get());
        let index = self.fns.len();
        let mut facts = FnFacts::new(
            function.span,
            function.node_id.get(),
            symbol,
            enclosing,
            function.params.items.is_empty() && function.params.rest.is_none(),
        );
        facts.is_declaration = function.is_declaration();
        facts.inline_callback = self
            .callback_spans
            .contains(&(function.span.start, function.span.end));
        self.fns.push(facts);
        if let Some(enclosing) = enclosing {
            self.fns[enclosing].nested.push(index);
        }
        if let Some(symbol) = symbol {
            self.fn_of_symbol.insert(symbol.index() as u32, index);
        }
        self.stack.push(index);
        walk::walk_function(self, function, flags);
        self.finish_function(index, enclosing);
        self.stack.pop();
    }

    fn visit_arrow_function_expression(&mut self, function: &ArrowFunctionExpression<'a>) {
        let enclosing = self.stack.last().copied();
        let index = self.fns.len();
        let mut facts = FnFacts::new(
            function.span,
            function.node_id.get(),
            None,
            enclosing,
            function.params.items.is_empty() && function.params.rest.is_none(),
        );
        facts.is_arrow = true;
        facts.inline_callback = self
            .callback_spans
            .contains(&(function.span.start, function.span.end));
        self.fns.push(facts);
        if let Some(enclosing) = enclosing {
            self.fns[enclosing].nested.push(index);
        }
        self.stack.push(index);
        walk::walk_arrow_function_expression(self, function);
        self.finish_function(index, enclosing);
        self.stack.pop();
    }

    fn visit_class(&mut self, class: &Class<'a>) {
        let mut members = BTreeSet::new();
        let mut constructor = None;
        for element in &class.body.body {
            match element {
                ClassElement::MethodDefinition(method) => {
                    if method.kind == MethodDefinitionKind::Constructor {
                        constructor = Some(method.value.span);
                    } else if let Some(name) = property_key_name(&method.key) {
                        members.insert(name);
                    }
                }
                ClassElement::PropertyDefinition(property) => {
                    if let Some(name) = property_key_name(&property.key) {
                        members.insert(name);
                    }
                }
                _ => {}
            }
        }
        self.classes.push(ClassFacts {
            span: class.span,
            symbol: class.id.as_ref().and_then(|id| id.symbol_id.get()),
            enclosing: self.stack.last().copied(),
            members,
            constructor,
        });
        walk::walk_class(self, class);
    }

    fn visit_string_literal(&mut self, literal: &StringLiteral<'a>) {
        let value = literal.value.to_string();
        self.innermost(|facts| {
            facts.direct_strings.insert(value.clone());
        });
    }

    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        let name = member.property.name.to_string();
        self.record_member(&member.object, &name);
        walk::walk_static_member_expression(self, member);
    }

    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        if let Expression::StringLiteral(literal) = &member.expression {
            let name = literal.value.to_string();
            self.record_member(&member.object, &name);
        }
        walk::walk_computed_member_expression(self, member);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
            let index = symbol.index() as u32;
            self.innermost(|facts| {
                facts.direct_refs.insert(index);
            });
        } else if identifier.name == "arguments" {
            // An arrow has no `arguments` of its own.
            let owner = self
                .stack
                .iter()
                .rev()
                .copied()
                .find(|index| !self.fns[*index].is_arrow);
            if let Some(owner) = owner {
                self.fns[owner].uses_arguments = true;
            }
        }
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let Some((object, name)) = member_access(&call.callee) {
            if name == "isView" {
                let tested = call
                    .arguments
                    .first()
                    .and_then(Argument::as_expression)
                    .and_then(|argument| self.own_slot(argument));
                if let Some((owner, slot)) = tested {
                    self.fns[owner].is_view_slots.insert(slot);
                }
            }
            self.record_call(name.clone());
            // `fn["generateKey"].call(fn, ...)` still calls generateKey.
            if matches!(name.as_str(), "call" | "apply") {
                if let Some((_target, inner)) = member_access(object) {
                    self.record_call(inner);
                }
            }
        }
        self.record_callback_arguments(&call.arguments);
        let mut symbols = BTreeSet::new();
        for argument in &call.arguments {
            if let Some(expression) = argument.as_expression() {
                collect_identifier_symbols(expression, self.scoping, &mut symbols);
            }
        }
        if !symbols.is_empty() {
            self.innermost(|facts| facts.direct_call_args.extend(symbols.iter().copied()));
            self.for_each(|facts| facts.all_call_args.extend(symbols.iter().copied()));
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        if property.kind == PropertyKind::Init {
            if let Some(name) = property_key_name(&property.key) {
                self.record_written_key(name);
            }
            self.record_escape(&property.value);
        }
        walk::walk_object_property(self, property);
    }

    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if let Some(init) = declarator.init.as_ref() {
            self.record_escape(init);
        }
        walk::walk_variable_declarator(self, declarator);
    }

    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        if assignment.operator == oxc_syntax::operator::AssignmentOperator::Assign {
            let target = match &assignment.left {
                AssignmentTarget::AssignmentTargetIdentifier(_) => None,
                AssignmentTarget::StaticMemberExpression(member) => {
                    Some((&member.object, member.property.name.to_string()))
                }
                AssignmentTarget::ComputedMemberExpression(member) => match &member.expression {
                    Expression::StringLiteral(literal) => {
                        Some((&member.object, literal.value.to_string()))
                    }
                    _ => None,
                },
                _ => None,
            };
            if let Some((object, name)) = target {
                self.record_key_write(object, &name);
                self.record_written_key(name);
            }
            if self.is_member_target(&assignment.left) && query_field_expression(&assignment.right)
            {
                self.innermost(|facts| facts.direct_query_field_writes += 1);
            }
            self.record_escape(&assignment.right);
        }
        walk::walk_assignment_expression(self, assignment);
    }

    fn visit_new_expression(&mut self, expression: &NewExpression<'a>) {
        if matches!(&expression.callee, Expression::Identifier(id) if id.name == "TypeError") {
            self.innermost(|facts| facts.new_type_error = true);
        }
        self.record_callback_arguments(&expression.arguments);
        walk::walk_new_expression(self, expression);
    }

    fn visit_binary_expression(&mut self, binary: &BinaryExpression<'a>) {
        use oxc_syntax::operator::BinaryOperator;
        match binary.operator {
            BinaryOperator::BitwiseXOR => {
                self.innermost(|facts| facts.direct_xors += 1);
                self.for_each(|facts| facts.all_xors += 1);
            }
            BinaryOperator::StrictEquality => {
                if let Some(tested) = typeof_string_operand(binary) {
                    if let Some((owner, slot)) = self.own_slot(tested) {
                        self.fns[owner].typeof_string_slots.insert(slot);
                    }
                }
            }
            _ => {}
        }
        walk::walk_binary_expression(self, binary);
    }

    fn visit_while_statement(&mut self, statement: &WhileStatement<'a>) {
        self.for_each(|facts| facts.all_while_loops += 1);
        walk::walk_while_statement(self, statement);
    }

    fn visit_do_while_statement(&mut self, statement: &DoWhileStatement<'a>) {
        self.for_each(|facts| facts.all_while_loops += 1);
        walk::walk_do_while_statement(self, statement);
    }

    fn visit_for_statement(&mut self, statement: &ForStatement<'a>) {
        self.for_each(|facts| facts.all_while_loops += 1);
        walk::walk_for_statement(self, statement);
    }

    fn visit_return_statement(&mut self, statement: &ReturnStatement<'a>) {
        if let Some(argument) = statement.argument.as_ref() {
            let is_function = match argument {
                Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => true,
                Expression::Identifier(identifier) => {
                    analysis::identifier_symbol(identifier, self.scoping)
                        .map(|symbol| self.fn_of_symbol.contains_key(&(symbol.index() as u32)))
                        .unwrap_or(false)
                }
                _ => false,
            };
            if is_function {
                self.innermost(|facts| facts.returns_nested_function = true);
            }
            if let Expression::Identifier(identifier) = argument {
                if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
                    let index = symbol.index() as u32;
                    self.innermost(|facts| {
                        facts.direct_returned.insert(index);
                    });
                }
            }
            self.record_escape(argument);
        }
        walk::walk_return_statement(self, statement);
    }
}

/// The operand of a `typeof <operand> === "string"` test.
fn typeof_string_operand<'e, 'a>(binary: &'e BinaryExpression<'a>) -> Option<&'e Expression<'a>> {
    let Expression::UnaryExpression(unary) = &binary.left else {
        return None;
    };
    if unary.operator != oxc_syntax::operator::UnaryOperator::Typeof {
        return None;
    }
    matches!(&binary.right, Expression::StringLiteral(literal) if literal.value == "string")
        .then_some(&unary.argument)
}

/// Property names and identifier names reachable in one expression.
#[derive(Default)]
struct ExprScan {
    members: BTreeSet<String>,
    identifiers: BTreeSet<String>,
}

impl<'a> Visit<'a> for ExprScan {
    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        self.members.insert(member.property.name.to_string());
        walk::walk_static_member_expression(self, member);
    }

    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        if let Expression::StringLiteral(literal) = &member.expression {
            self.members.insert(literal.value.to_string());
        }
        walk::walk_computed_member_expression(self, member);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.identifiers.insert(identifier.name.to_string());
    }
}

// ===========================================================================
// Recognizer
// ===========================================================================

enum Pick {
    None,
    One(usize),
    Ambiguous(Vec<InvolvedSpan>),
}

/// A role claim addressed by a function node span rather than a binding.
///
/// This is the entry point for span-keyed recognizers (for example the purpose
/// finders): the span is resolved in the current parse to the function's
/// editable binding — its declaration, its single-origin holder, or its own
/// name — and every fail-closed proof of the built-in recognizers applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionSpanClaim {
    /// Stable machine label of the role.
    pub role: String,
    /// The function node span (`fn@start..end`).
    pub function_span: SourceSpan,
    /// The descriptive action name for the function's binding.
    pub proposed_name: String,
    /// An optional `(alias_name, alias_role)` for the single main-owned alias.
    pub alias: Option<(String, String)>,
    pub confidence: Confidence,
    pub evidence: Vec<String>,
}

/// Build the naming plan and then apply extra span-keyed claims.
///
/// The claims go through the same proofs as the built-in roles: an unresolved
/// span, a mutable or duplicated binding, an ambiguous holder, a second role on
/// one binding and a non-unique alias are all refused and recorded.
pub fn plan_semantic_names_with_claims(
    source: &str,
    properties: &ProgramProperties,
    claims: &[FunctionSpanClaim],
) -> Result<SemanticNamingPlan, SemanticNamesError> {
    let current = crate::source::sha256_hex(source.as_bytes());
    if properties.source.sha256 != current || properties.source.bytes != source.len() {
        return Err(SemanticNamesError::SourceMismatch {
            report: properties.source.sha256.clone(),
            source: current,
        });
    }
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source).map_err(SemanticNamesError::Parse)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut recognizer = Recognizer::new(source, properties, &program, &scoping, &nodes);
    recognizer.run();
    recognizer.claim_purposes(&program);
    recognizer.claim_finders(&program);
    for claim in claims {
        let alias = claim
            .alias
            .as_ref()
            .map(|(name, role)| (name.as_str(), role.as_str()));
        recognizer.claim_function_span(
            &claim.role,
            claim.function_span,
            &claim.proposed_name,
            alias,
            claim.confidence,
            claim.evidence.clone(),
        );
    }
    Ok(recognizer.finish())
}

struct Recognizer<'p, 'a> {
    source: &'p str,
    properties: &'p ProgramProperties,
    scoping: &'a Scoping,
    nodes: &'a AstNodes<'a>,
    fns: Vec<FnFacts>,
    classes: Vec<ClassFacts>,
    key_writes: Vec<KeyWrite>,
    assignments: BTreeMap<u32, Vec<&'a Expression<'a>>>,
    member_refs: BTreeMap<u32, BTreeSet<String>>,
    symbols: BTreeMap<u32, SymbolId>,
    /// Declaring identifier span -> symbols declared there, in this parse.
    identifier_symbols: BTreeMap<(u32, u32), Vec<SymbolId>>,
    main_ids: BTreeSet<u32>,
    candidates: Vec<NamingCandidate>,
    ambiguous: Vec<NamingAmbiguity>,
    skipped: Vec<NamingSkip>,
    notes: Vec<String>,
    claimed: BTreeMap<u32, String>,
    /// Symbols whose binding matched two incompatible roles; never renamed.
    conflicted: BTreeSet<u32>,
    /// The structure the properties extractor refused, in a degraded plan.
    degraded: Option<&'static str>,
    /// MAIN bindings a package finder claims explicitly; the automatic
    /// `<name>Alias` naming of a data container leaves them alone.
    explicit_main: BTreeSet<u32>,
    anonymous: Vec<AnonymousFunctionName>,
}

/// One finder role in one declaring scope: the unit in which two renames can
/// give two different bindings the same name.
type FinderTieKey = (&'static str, usize);

/// The same-scope ties among package-finder claims.
struct FinderTies {
    /// Tied Function groups: the tied function-node spans, sorted.
    functions: BTreeMap<FinderTieKey, Vec<Span>>,
    /// Tied MainBinding groups: the tied identifier spans, sorted.
    main_bindings: BTreeMap<FinderTieKey, Vec<Span>>,
    /// `(role, declaring scope)` of every claim that resolves to a bound
    /// entity. `None` for an unbound/anonymous function or an unresolvable
    /// binding: those claims resolve independently.
    keys: Vec<Option<FinderTieKey>>,
}

impl<'p, 'a> Recognizer<'p, 'a> {
    fn new(
        source: &'p str,
        properties: &'p ProgramProperties,
        program: &Program<'a>,
        scoping: &'a Scoping,
        nodes: &'a AstNodes<'a>,
    ) -> Self {
        let mut builder = Builder::new(program, scoping, nodes);
        builder.visit_program(program);

        // Owned facts: a nested function lends its facts to its parent only
        // when it is the parent's callback. Children have larger indices than
        // their parents, so one reverse pass folds whole callback chains.
        let mut fns = builder.fns;
        for facts in &mut fns {
            facts.seed_owned();
        }
        for index in (0..fns.len()).rev() {
            let Some(parent) = fns[index].enclosing else {
                continue;
            };
            let handed_off = fns[index].is_declaration
                && fns[index].symbol.is_some_and(|symbol| {
                    let symbol = symbol.index() as u32;
                    fns[parent].all_call_args.contains(&symbol)
                        && !fns[parent].escaping.contains(&symbol)
                });
            if fns[index].inline_callback || handed_off {
                fns[index].callback = true;
                let (left, right) = fns.split_at_mut(index);
                left[parent].absorb_owned(&right[0]);
            }
        }

        let mut symbols: BTreeMap<u32, SymbolId> = BTreeMap::new();
        let mut identifier_symbols: BTreeMap<(u32, u32), Vec<SymbolId>> = BTreeMap::new();
        for symbol in scoping.symbol_ids() {
            symbols.insert(symbol.index() as u32, symbol);
            let span = scoping.symbol_span(symbol);
            identifier_symbols
                .entry((span.start, span.end))
                .or_default()
                .push(symbol);
        }

        let mut recognizer = Self {
            source,
            properties,
            scoping,
            nodes,
            fns,
            classes: builder.classes,
            key_writes: builder.key_writes,
            assignments: BTreeMap::new(),
            member_refs: builder.member_refs,
            symbols,
            identifier_symbols,
            main_ids: BTreeSet::new(),
            candidates: Vec::new(),
            ambiguous: Vec::new(),
            skipped: Vec::new(),
            notes: Vec::new(),
            claimed: BTreeMap::new(),
            conflicted: BTreeSet::new(),
            degraded: None,
            explicit_main: BTreeSet::new(),
            anonymous: Vec::new(),
        };
        recognizer.collect_assignments();
        for binding in &properties.bindings {
            match recognizer.resolve_identifier(&binding.name, binding.identifier) {
                Ok(symbol) => {
                    recognizer.main_ids.insert(symbol);
                }
                Err(reason) => recognizer.refuse_span(
                    "main_binding",
                    &binding.name,
                    binding.identifier,
                    &reason,
                ),
            }
        }
        recognizer
    }

    fn run(&mut self) {
        self.core_roles();
        self.extra_functions();
        self.extra_main_bindings();
        self.dom_family();
        self.mark_unknown_surfaces();
    }

    /// Claim the purpose finders' findings through the same fail-closed proofs
    /// as the built-in roles. A found function is resolved to its editable
    /// binding by its node span; a tie is recorded as an ambiguity; a role
    /// nothing matched is left for the unproven list.
    /// Resolve every package-finder claim ([`super::finders`]) through the
    /// same fail-closed proofs as the built-in roles.
    fn claim_finders(&mut self, program: &Program<'a>) {
        let ctx = crate::signals::analysis::roles::FinderCtx {
            source: self.source,
            program,
            scoping: self.scoping,
            nodes: self.nodes,
        };
        let claims = crate::signals::analysis::roles::run_all(&ctx);
        let mut main_claims = Vec::new();
        let mut class_claims = Vec::new();
        for claim in &claims {
            if let crate::signals::analysis::roles::FinderClaim::MainBinding { identifier_span, .. } = claim {
                if let Some(symbol) = self.symbol_at_identifier(*identifier_span) {
                    self.explicit_main.insert(symbol);
                }
            }
        }
        // Group before resolving, by `(role, declaring scope)`. Only two
        // distinct *bound* entities claimed for one role in the same scope are
        // an equal-evidence tie; different scopes do not collide, and an
        // unbound/anonymous function has no binding name to collide, so both
        // keep resolving exactly as before.
        let ties = self.finder_ties(&claims);
        let mut reported_functions: BTreeSet<FinderTieKey> = BTreeSet::new();
        let mut reported_main_bindings: BTreeSet<FinderTieKey> = BTreeSet::new();
        for (index, claim) in claims.into_iter().enumerate() {
            match claim {
                crate::signals::analysis::roles::FinderClaim::Function {
                    role,
                    function_span,
                    alias_role,
                    confidence,
                    evidence,
                } => {
                    let Some(entry) = crate::signals::analysis::roles::finder_role(role) else {
                        continue;
                    };
                    if let Some(key) = ties.keys[index] {
                        if let Some(spans) = ties.functions.get(&key) {
                            if reported_functions.insert(key) {
                                self.record_finder_function_ambiguity(role, entry.name, spans);
                            }
                            continue;
                        }
                    }
                    let alias = alias_role.and_then(|alias_role| {
                        crate::signals::analysis::roles::finder_role(alias_role).map(|alias| (alias.name, alias_role))
                    });
                    let skipped_before = self.skipped.len();
                    let span = SourceSpan::from(function_span);
                    let named = self.claim_function_span(
                        role,
                        span,
                        entry.name,
                        alias,
                        confidence,
                        evidence.clone(),
                    );
                    let anonymous = named.is_none()
                        && self.skipped[skipped_before..].iter().any(|skip| {
                            skip.role == role && skip.kind == SkipKind::RecognizedNoEditableBinding
                        });
                    if anonymous {
                        // The finder proposed a name through `anonymous_functions`,
                        // so this is not a metadata-only role. Drop the duplicate
                        // skip: every skip must be a refusal or metadata only.
                        self.skipped.truncate(skipped_before);
                        // An unbound claim can never collide with a binding
                        // rename, so equal claims of one role stay anonymous
                        // instead of becoming an ambiguity. One entry per span.
                        let already = self.anonymous.iter().any(|named| {
                            named.role == role
                                && named.function.start == span.start
                                && named.function.end == span.end
                        });
                        if !already {
                            let arrow = self
                                .fns
                                .iter()
                                .find(|facts| {
                                    facts.span.start == span.start && facts.span.end == span.end
                                })
                                .is_some_and(|facts| facts.is_arrow);
                            self.anonymous.push(AnonymousFunctionName {
                                role: role.to_string(),
                                proposed_name: entry.name.to_string(),
                                function: span,
                                id: fn_id(span),
                                arrow,
                                confidence,
                                evidence,
                            });
                        }
                    }
                }
                crate::signals::analysis::roles::FinderClaim::MainBinding {
                    role,
                    identifier_span,
                    confidence,
                    evidence,
                } => {
                    let Some(entry) = crate::signals::analysis::roles::finder_role(role) else {
                        continue;
                    };
                    if let Some(key) = ties.keys[index] {
                        if let Some(spans) = ties.main_bindings.get(&key) {
                            if reported_main_bindings.insert(key) {
                                self.record_finder_main_ambiguity(role, entry.name, spans);
                            }
                            continue;
                        }
                    }
                    main_claims.push((role, identifier_span, entry.name, confidence, evidence));
                }
                crate::signals::analysis::roles::FinderClaim::ClassBinding {
                    role,
                    class_span,
                    confidence,
                    evidence,
                } => {
                    let Some(entry) = crate::signals::analysis::roles::finder_role(role) else {
                        continue;
                    };
                    if let Some(key) = ties.keys[index] {
                        if let Some(spans) = ties.main_bindings.get(&key) {
                            if reported_main_bindings.insert(key) {
                                self.record_finder_main_ambiguity(role, entry.name, spans);
                            }
                            continue;
                        }
                    }
                    class_claims.push((role, class_span, entry.name, confidence, evidence));
                }
            }
        }
        for (role, identifier_span, name, confidence, evidence) in main_claims {
            self.claim_main_binding(role, identifier_span, name, confidence, &evidence.join("; "));
        }
        for (role, class_span, name, confidence, evidence) in class_claims {
            self.claim_class_span(role, class_span, name, confidence, &evidence.join("; "));
        }
    }

    /// Group finder claims by `(role, declaring scope)` and keep the groups
    /// with two or more distinct bound entities. That is the only case where
    /// two renames would give two different bindings the same name in one
    /// scope. An unbound/anonymous function claim and a claim whose binding
    /// scope cannot be resolved are never grouped: they resolve independently,
    /// as they did before the finder had a tie check.
    #[allow(clippy::type_complexity)]
    fn finder_ties(&self, claims: &[crate::signals::analysis::roles::FinderClaim]) -> FinderTies {
        let mut keys: Vec<Option<FinderTieKey>> = Vec::with_capacity(claims.len());
        let mut function_entities: BTreeMap<FinderTieKey, (BTreeSet<u32>, BTreeSet<(u32, u32)>)> =
            BTreeMap::new();
        let mut main_entities: BTreeMap<FinderTieKey, (BTreeSet<u32>, BTreeSet<(u32, u32)>)> =
            BTreeMap::new();
        for claim in claims {
            // A claim whose role its package does not list is dropped by the
            // resolution pass, so it cannot take part in a tie either.
            if crate::signals::analysis::roles::finder_role(claim.role()).is_none() {
                keys.push(None);
                continue;
            }
            let binding = match claim {
                crate::signals::analysis::roles::FinderClaim::Function { function_span, .. } => self
                    .function_binding_symbol(*function_span)
                    .and_then(|symbol| self.binding_scope(symbol).map(|scope| (symbol, scope))),
                crate::signals::analysis::roles::FinderClaim::MainBinding { identifier_span, .. } => self
                    .symbol_at_identifier(*identifier_span)
                    .and_then(|symbol| self.binding_scope(symbol).map(|scope| (symbol, scope))),
                crate::signals::analysis::roles::FinderClaim::ClassBinding { class_span, .. } => self
                    .class_binding_symbol(*class_span)
                    .and_then(|symbol| self.binding_scope(symbol).map(|scope| (symbol, scope))),
            };
            let Some((symbol, scope)) = binding else {
                keys.push(None);
                continue;
            };
            let key = (claim.role(), scope);
            match claim {
                crate::signals::analysis::roles::FinderClaim::Function { function_span, .. } => {
                    let entry = function_entities.entry(key).or_default();
                    entry.0.insert(symbol);
                    entry.1.insert((function_span.start, function_span.end));
                }
                crate::signals::analysis::roles::FinderClaim::MainBinding { identifier_span, .. } => {
                    let entry = main_entities.entry(key).or_default();
                    entry.0.insert(symbol);
                    entry.1.insert((identifier_span.start, identifier_span.end));
                }
                crate::signals::analysis::roles::FinderClaim::ClassBinding { class_span, .. } => {
                    let entry = main_entities.entry(key).or_default();
                    entry.0.insert(symbol);
                    entry.1.insert((class_span.start, class_span.end));
                }
            }
            keys.push(Some(key));
        }
        let ties = |groups: BTreeMap<FinderTieKey, (BTreeSet<u32>, BTreeSet<(u32, u32)>)>| {
            groups
                .into_iter()
                .filter(|(_, (entities, _))| entities.len() > 1)
                .map(|(key, (_, spans))| {
                    (
                        key,
                        spans
                            .into_iter()
                            .map(|(start, end)| Span::new(start, end))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<BTreeMap<_, Vec<Span>>>()
        };
        FinderTies {
            functions: ties(function_entities),
            main_bindings: ties(main_entities),
            keys,
        }
    }

    /// The editable binding symbol of a claimed function, when it has one.
    /// Mirrors [`Self::editable_function_binding`] without recording refusals:
    /// a declaration's own name, else its single-origin holder, else its own
    /// name. `None` when the function is unbound/anonymous or its binding is
    /// ambiguous or unresolved; such a claim is never tied with another.
    fn function_binding_symbol(&self, span: Span) -> Option<u32> {
        let facts = self
            .fns
            .iter()
            .find(|facts| facts.span.start == span.start && facts.span.end == span.end)?;
        if facts.is_declaration {
            if let Some(symbol) = facts.symbol {
                return Some(symbol.index() as u32);
            }
        }
        match self.single_origin_holders(facts.span).as_slice() {
            [holder] => Some(*holder),
            [] => facts.symbol.map(|symbol| symbol.index() as u32),
            _ => None,
        }
    }

    /// The declaring scope of a bound symbol, as a stable map key.
    fn binding_scope(&self, symbol: u32) -> Option<usize> {
        let symbol_id = self.symbols.get(&symbol).copied()?;
        Some(self.scoping.symbol_scope_id(symbol_id).index())
    }

    /// The single symbol declared at exactly this identifier span.
    fn symbol_at_identifier(&self, span: Span) -> Option<u32> {
        match self.identifier_symbols.get(&(span.start, span.end))?.as_slice() {
            [symbol] => Some(symbol.index() as u32),
            _ => None,
        }
    }

    /// The binding symbol of the class whose node span is exactly `span`.
    fn class_binding_symbol(&self, span: Span) -> Option<u32> {
        self.classes
            .iter()
            .find(|class| class.span.start == span.start && class.span.end == span.end)
            .and_then(|class| class.symbol)
            .map(|symbol| symbol.index() as u32)
    }

    /// True when another binding in `symbol`'s scope already has `name`.
    /// A class rename that would duplicate a lexical binding is an early
    /// error, so the plan refuses it instead of emitting it.
    fn scope_has_binding(&self, symbol: SymbolId, name: &str) -> bool {
        let scope = self.scoping.symbol_scope_id(symbol);
        self.scoping.symbol_ids().any(|other| {
            other != symbol
                && self.scoping.symbol_scope_id(other) == scope
                && self.scoping.symbol_name(other) == name
        })
    }

    /// Name a class declaration's binding. The class must be a known class
    /// node with one immutable name binding owned by the top main scope. A
    /// class name that would duplicate another lexical binding is refused.
    fn claim_class_span(
        &mut self,
        role: &str,
        class_span: Span,
        name: &str,
        confidence: Confidence,
        evidence: &str,
    ) -> bool {
        let involved = || {
            vec![InvolvedSpan {
                name: None,
                span: SourceSpan::from(class_span),
                id: Some(mv_id(SourceSpan::from(class_span))),
            }]
        };
        let Some(class) = self
            .classes
            .iter()
            .find(|class| class.span.start == class_span.start && class.span.end == class_span.end)
        else {
            self.skipped.push(NamingSkip {
                role: role.to_string(),
                kind: SkipKind::Refused,
                reason: "no class node has this span in the current parse".to_string(),
                involved: involved(),
            });
            return false;
        };
        let Some(symbol_id) = class.symbol else {
            self.skipped.push(NamingSkip {
                role: role.to_string(),
                kind: SkipKind::Refused,
                reason: "the class has no editable name binding".to_string(),
                involved: involved(),
            });
            return false;
        };
        let symbol = symbol_id.index() as u32;
        if self.conflicted.contains(&symbol) {
            return false;
        }
        if !self.main_ids.contains(&symbol) {
            self.skip_binding(
                role,
                "class binding is not owned by the top main scope",
                symbol,
            );
            return false;
        }
        if self.scoping.symbol_declarations(symbol_id).count() != 1 {
            self.skip_binding(role, "class binding is declared more than once", symbol);
            return false;
        }
        if self.assignments.contains_key(&symbol) {
            self.skip_binding(role, "class binding is reassigned", symbol);
            return false;
        }
        if self.scope_has_binding(symbol_id, name) {
            self.skip_binding(
                role,
                &format!("renaming the class to `{name}` would duplicate a binding in its scope"),
                symbol,
            );
            return false;
        }
        let declaration = SourceSpan::from(class.span);
        let entity = NamingEntity {
            kind: NamingEntityKind::Class,
            span: declaration,
            id: mv_id(declaration),
            target: None,
        };
        self.finish_claim(
            role,
            symbol,
            symbol_id,
            name,
            confidence,
            vec![evidence.to_string()],
            entity,
        )
    }

    /// Name one MAIN binding addressed by its declaring identifier: a data
    /// container (and its unclaimed single alias), an explicit alias of
    /// another MAIN binding, or mutable runtime state.
    fn claim_main_binding(
        &mut self,
        role: &str,
        identifier_span: Span,
        name: &str,
        confidence: Confidence,
        evidence: &str,
    ) -> bool {
        let Some(symbol) = self.symbol_at_identifier(identifier_span) else {
            self.skipped.push(NamingSkip {
                role: role.to_string(),
                kind: SkipKind::Refused,
                reason: "no single symbol is declared at this identifier span in the current parse".to_string(),
                involved: vec![InvolvedSpan {
                    name: None,
                    span: SourceSpan::from(identifier_span),
                    id: None,
                }],
            });
            return false;
        };
        let Some(symbol_id) = self.symbols.get(&symbol).copied() else {
            return false;
        };
        if self.single_origin(symbol_id).is_none() {
            // An alias role names the binding's single value. A binding that
            // was written again is no longer that alias, so it must not fall
            // back to mutable-state naming.
            if role.ends_with("_alias") {
                self.skip_binding(
                    role,
                    "alias role requires a single-origin binding",
                    symbol,
                );
                return false;
            }
            return self.claim_main_state(role, symbol, name, confidence, evidence);
        }
        match self.alias_target(symbol) {
            None => self.claim_main_data(role, symbol, name, confidence, evidence),
            Some(target) => {
                let Some(target_id) = self.symbols.get(&target).copied() else {
                    return false;
                };
                if !self.main_ids.contains(&symbol) || self.conflicted.contains(&symbol) {
                    self.skip_binding(role, "binding is not owned by the top main scope", symbol);
                    return false;
                }
                let declaration = SourceSpan::from(self.declaration_span(symbol_id));
                let target_declaration = SourceSpan::from(self.declaration_span(target_id));
                let entity = NamingEntity {
                    kind: NamingEntityKind::AliasHolder,
                    span: declaration,
                    id: mv_id(declaration),
                    target: Some(mv_id(target_declaration)),
                };
                self.finish_claim(
                    role,
                    symbol,
                    symbol_id,
                    name,
                    confidence,
                    vec![evidence.to_string()],
                    entity,
                )
            }
        }
    }

    /// Name MAIN-owned mutable state: one symbol, an ordinary main variable,
    /// not conflicted. Its writes are not a single origin, so the evidence
    /// must come from how the binding is written and read.
    fn claim_main_state(
        &mut self,
        role: &str,
        symbol: u32,
        name: &str,
        confidence: Confidence,
        evidence: &str,
    ) -> bool {
        let Some(symbol_id) = self.symbols.get(&symbol).copied() else {
            return false;
        };
        if self.conflicted.contains(&symbol) {
            return false;
        }
        if !self.main_ids.contains(&symbol) {
            self.skip_binding(role, "binding is not owned by the top main scope", symbol);
            return false;
        }
        if !matches!(
            binding_kind(self.scoping, self.nodes, symbol_id),
            BindingKind::Variable
        ) {
            self.skip_binding(role, "main binding is not an ordinary variable", symbol);
            return false;
        }
        let declaration = self.declaration_span(symbol_id);
        let entity = NamingEntity {
            kind: NamingEntityKind::MainData,
            span: SourceSpan::from(declaration),
            id: mv_id(SourceSpan::from(declaration)),
            target: None,
        };
        self.finish_claim(
            role,
            symbol,
            symbol_id,
            name,
            confidence,
            vec![format!("mutable main state: {evidence}")],
            entity,
        )
    }

    fn claim_purposes(&mut self, program: &Program<'a>) {
        for outcome in
            crate::signals::analysis::roles::purposes::find_purposes(self.source, program, self.scoping, self.nodes)
        {
            match outcome {
                crate::signals::analysis::roles::purposes::PurposeOutcome::Found(finding) => {
                    let alias_role = format!("{}{ALIAS_ROLE_SUFFIX}", finding.role);
                    let alias = finding
                        .holder_alias_name
                        .map(|name| (name, alias_role.as_str()));
                    self.claim_function_span(
                        finding.role,
                        SourceSpan::from(finding.function_span),
                        finding.action_name,
                        alias,
                        finding.confidence,
                        finding.evidence,
                    );
                }
                crate::signals::analysis::roles::purposes::PurposeOutcome::Ambiguous { role, .. }
                    if RETIRED_PURPOSE_ROLES.contains(&role) => {}
                crate::signals::analysis::roles::purposes::PurposeOutcome::Ambiguous {
                    role,
                    candidates,
                    reason,
                } => {
                    let involved = candidates
                        .into_iter()
                        .map(|span| {
                            let span = SourceSpan::from(span);
                            InvolvedSpan {
                                name: None,
                                span,
                                id: Some(fn_id(span)),
                            }
                        })
                        .collect();
                    self.record_ambiguity(role, &reason, involved);
                }
                crate::signals::analysis::roles::purposes::PurposeOutcome::NotFound { .. } => {}
            }
        }
    }

    // -----------------------------------------------------------------------
    // Cross-parse identity: exact source plus validated identifier spans
    // -----------------------------------------------------------------------

    /// Resolve a report binding to this parse's symbol by its declaring
    /// identifier span. The span must lie in the source, on UTF-8 char
    /// boundaries, spell `name`, and declare exactly one symbol.
    fn resolve_identifier(&self, name: &str, identifier: SourceSpan) -> Result<u32, String> {
        let (start, end) = (identifier.start as usize, identifier.end as usize);
        if start >= end || end > self.source.len() {
            return Err(format!(
                "identifier span {start}..{end} is empty or outside the {}-byte source",
                self.source.len()
            ));
        }
        if !self.source.is_char_boundary(start) || !self.source.is_char_boundary(end) {
            return Err(format!(
                "identifier span {start}..{end} is not on UTF-8 char boundaries"
            ));
        }
        if &self.source[start..end] != name {
            return Err(format!(
                "identifier span {start}..{end} does not spell the recorded name `{name}`"
            ));
        }
        match self
            .identifier_symbols
            .get(&(identifier.start, identifier.end))
            .map(Vec::as_slice)
        {
            Some([symbol]) => Ok(symbol.index() as u32),
            Some(_) => Err(format!(
                "more than one symbol is declared at identifier span {start}..{end}"
            )),
            None => Err(format!(
                "no symbol is declared at identifier span {start}..{end} in the current parse"
            )),
        }
    }

    /// Resolve a report [`BindingRef`] for `role`, recording a refusal when the
    /// span does not validate.
    fn ref_symbol(&mut self, role: &str, binding: &BindingRef) -> Option<u32> {
        match self.resolve_identifier(&binding.name, binding.identifier) {
            Ok(symbol) => Some(symbol),
            Err(reason) => {
                self.refuse_span(role, &binding.name, binding.identifier, &reason);
                None
            }
        }
    }

    /// Resolve without recording (for exclusion sets and anchors).
    fn ref_symbol_quiet(&self, binding: &BindingRef) -> Option<u32> {
        self.resolve_identifier(&binding.name, binding.identifier)
            .ok()
    }

    fn refuse_span(&mut self, role: &str, name: &str, span: SourceSpan, reason: &str) {
        self.skipped.push(NamingSkip {
            role: role.to_string(),
            kind: SkipKind::Refused,
            reason: format!("binding span refused: {reason}"),
            involved: vec![InvolvedSpan {
                name: Some(name.to_string()),
                span,
                id: None,
            }],
        });
    }

    /// Every plain `Symbol = expression` write in the program, by symbol.
    fn collect_assignments(&mut self) {
        let mut map: BTreeMap<u32, Vec<&'a Expression<'a>>> = BTreeMap::new();
        for node in self.nodes.iter() {
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                continue;
            };
            if assignment.operator != oxc_syntax::operator::AssignmentOperator::Assign {
                continue;
            }
            let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
                continue;
            };
            let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) else {
                continue;
            };
            map.entry(symbol.index() as u32)
                .or_default()
                .push(&assignment.right);
        }
        self.assignments = map;
    }

    // -----------------------------------------------------------------------
    // Candidate bookkeeping
    // -----------------------------------------------------------------------

    /// Claim a function identity. A function is eligible at any scope, but only
    /// when its binding is one immutable declaration whose value is
    /// function-valued (or an alias chain to one). Ordinary data is not.
    fn claim_function(
        &mut self,
        role: &str,
        symbol: u32,
        name: &str,
        confidence: Confidence,
        evidence: Vec<String>,
    ) -> bool {
        let Some(symbol_id) = self.symbols.get(&symbol).copied() else {
            return false;
        };
        if self.conflicted.contains(&symbol) {
            return false;
        }
        if !self.function_identity(symbol_id) {
            self.skip_binding(
                role,
                "function identity is not a single immutable function-valued declaration",
                symbol,
            );
            return false;
        }
        let entity = self.function_entity(symbol_id);
        self.finish_claim(role, symbol, symbol_id, name, confidence, evidence, entity)
    }

    /// Claim the function whose node has exactly `function_span`, addressed
    /// through its editable binding in the current parse: a declaration's own
    /// name, else the single binding whose single origin is the function, else
    /// a named expression's own name. Every function-identity, conflict and
    /// alias proof of [`Self::claim_function`] / [`Self::claim_alias`] applies.
    /// Returns the claimed binding.
    fn claim_function_span(
        &mut self,
        role: &str,
        function_span: SourceSpan,
        name: &str,
        alias: Option<(&str, &str)>,
        confidence: Confidence,
        evidence: Vec<String>,
    ) -> Option<u32> {
        let index = self.fns.iter().position(|facts| {
            facts.span.start == function_span.start && facts.span.end == function_span.end
        });
        let Some(index) = index else {
            self.skipped.push(NamingSkip {
                role: role.to_string(),
                kind: SkipKind::Refused,
                reason: "no function node has this span in the current parse".to_string(),
                involved: vec![InvolvedSpan {
                    name: None,
                    span: function_span,
                    id: Some(fn_id(function_span)),
                }],
            });
            return None;
        };
        let symbol = self.editable_function_binding(role, index)?;
        let alias_evidence = evidence
            .first()
            .cloned()
            .unwrap_or_else(|| "direct main-scope alias of the recognized function".to_string());
        if !self.claim_function(role, symbol, name, confidence, evidence) {
            return None;
        }
        if let Some((alias_name, alias_role)) = alias {
            if let Some(target) = self.symbols.get(&symbol).copied() {
                self.claim_alias(
                    alias_role,
                    target,
                    alias_name,
                    confidence,
                    &format!("direct main-scope alias: {alias_evidence}"),
                );
            }
        }
        Some(symbol)
    }

    /// The editable binding of the function at `index`, or a recorded refusal.
    fn editable_function_binding(&mut self, role: &str, index: usize) -> Option<u32> {
        let facts = &self.fns[index];
        if facts.is_declaration {
            if let Some(symbol) = facts.symbol {
                return Some(symbol.index() as u32);
            }
        }
        let span = facts.span;
        let own_name = facts.symbol;
        let written = self.holder_candidates(span);
        let holders = self.single_origin_holders(span);
        match holders.as_slice() {
            [holder] => Some(*holder),
            [] => match own_name {
                Some(symbol) => Some(symbol.index() as u32),
                None if written.is_empty() => {
                    // Recognized, but nothing editable names the function.
                    self.skipped.push(NamingSkip {
                        role: role.to_string(),
                        kind: SkipKind::RecognizedNoEditableBinding,
                        reason: NO_EDITABLE_BINDING_REASON.to_string(),
                        involved: vec![self.fn_involved(index)],
                    });
                    None
                }
                None => {
                    let mut involved = vec![self.fn_involved(index)];
                    involved.extend(written.iter().map(|holder| self.involved(*holder)));
                    self.skipped.push(NamingSkip {
                        role: role.to_string(),
                        kind: SkipKind::Refused,
                        reason:
                            "the binding holding the anonymous function is redeclared or reassigned"
                                .to_string(),
                        involved,
                    });
                    None
                }
            },
            _ => {
                let involved = holders
                    .iter()
                    .map(|holder| self.involved(*holder))
                    .collect();
                self.record_ambiguity(
                    role,
                    "more than one binding holds the function as its single origin",
                    involved,
                );
                None
            }
        }
    }

    /// Bindings whose single source origin is exactly the function node `span`.
    fn single_origin_holders(&self, span: Span) -> Vec<u32> {
        self.holder_candidates(span)
            .into_iter()
            .filter(|symbol| {
                self.symbols
                    .get(symbol)
                    .and_then(|symbol| self.single_origin(*symbol))
                    .is_some_and(|value| value.span() == span)
            })
            .collect()
    }

    /// Bindings initialized or plainly assigned with the function node
    /// `span`, whatever else is written to them.
    fn holder_candidates(&self, span: Span) -> BTreeSet<u32> {
        let mut candidates: BTreeSet<u32> = BTreeSet::new();
        for node in self.nodes.iter() {
            if let AstKind::VariableDeclarator(declarator) = node.kind() {
                if declarator.init.as_ref().map(GetSpan::span) == Some(span) {
                    if let BindingPattern::BindingIdentifier(identifier) = &declarator.id {
                        if let Some(symbol) = identifier.symbol_id.get() {
                            candidates.insert(symbol.index() as u32);
                        }
                    }
                }
            }
        }
        for (symbol, values) in &self.assignments {
            if values.iter().any(|value| value.span() == span) {
                candidates.insert(*symbol);
            }
        }
        candidates
    }

    /// Claim a top-MAIN-owned ordinary variable with a single source origin.
    /// Nested locals, parameters, `catch` bindings and function bindings are
    /// never eligible here.
    ///
    /// Holder and alias are named by one structural rule, whichever of the
    /// two the evidence pointed at: the *value holder* (the end of a chain of
    /// single-origin `alias = holder` bindings) receives the role, and its
    /// single main alias receives `<role>_alias` / `<name>Alias`. Several
    /// aliases leave the alias role ambiguous.
    fn claim_main_data(
        &mut self,
        role: &str,
        symbol: u32,
        name: &str,
        confidence: Confidence,
        evidence: &str,
    ) -> bool {
        let holder = self.value_holder(symbol);
        if !self.claim_main_value(role, holder, name, confidence, evidence) {
            return false;
        }
        let Some(holder_id) = self.symbols.get(&holder).copied() else {
            return true;
        };
        let aliases: Vec<u32> = self
            .aliases_of(holder_id)
            .into_iter()
            .filter(|alias| {
                self.symbols.get(alias).is_some_and(|alias| {
                    matches!(
                        binding_kind(self.scoping, self.nodes, *alias),
                        BindingKind::Variable
                    )
                })
            })
            .collect();
        let aliases: Vec<u32> = aliases
            .into_iter()
            .filter(|alias| !self.explicit_main.contains(alias))
            .collect();
        let alias_role = format!("{role}_alias");
        match aliases.as_slice() {
            [] => {}
            [alias] => {
                let alias = *alias;
                if let Some(alias_id) = self.symbols.get(&alias).copied() {
                    if !self.conflicted.contains(&alias) {
                        let declaration = SourceSpan::from(self.declaration_span(alias_id));
                        let holder_declaration = SourceSpan::from(self.declaration_span(holder_id));
                        let entity = NamingEntity {
                            kind: NamingEntityKind::AliasHolder,
                            span: declaration,
                            id: mv_id(declaration),
                            target: Some(mv_id(holder_declaration)),
                        };
                        self.finish_claim(
                            &alias_role,
                            alias,
                            alias_id,
                            &format!("{name}Alias"),
                            confidence,
                            vec![format!("direct main-scope alias of the proven {role}")],
                            entity,
                        );
                    }
                }
            }
            _ => {
                let involved = aliases.iter().map(|alias| self.involved(*alias)).collect();
                self.record_ambiguity(
                    &alias_role,
                    "more than one main alias holds the proven value",
                    involved,
                );
            }
        }
        true
    }

    /// Follow single-origin `alias = holder` links between main-owned ordinary
    /// variables to the binding that holds the value itself.
    fn value_holder(&self, mut symbol: u32) -> u32 {
        for _ in 0..4 {
            let Some(next) = self.alias_target(symbol) else {
                break;
            };
            symbol = next;
        }
        symbol
    }

    fn alias_target(&self, symbol: u32) -> Option<u32> {
        let is_main_variable = |symbol: u32| {
            self.main_ids.contains(&symbol)
                && self.symbols.get(&symbol).is_some_and(|id| {
                    matches!(
                        binding_kind(self.scoping, self.nodes, *id),
                        BindingKind::Variable
                    ) && self.single_origin(*id).is_some()
                })
        };
        if !is_main_variable(symbol) {
            return None;
        }
        let Expression::Identifier(identifier) = self.single_origin(*self.symbols.get(&symbol)?)?
        else {
            return None;
        };
        let target = analysis::identifier_symbol(identifier, self.scoping)?.index() as u32;
        (target != symbol && is_main_variable(target)).then_some(target)
    }

    fn claim_main_value(
        &mut self,
        role: &str,
        symbol: u32,
        name: &str,
        confidence: Confidence,
        evidence: &str,
    ) -> bool {
        let Some(symbol_id) = self.symbols.get(&symbol).copied() else {
            return false;
        };
        if self.conflicted.contains(&symbol) {
            return false;
        }
        if !self.main_ids.contains(&symbol) {
            self.skip_binding(role, "binding is not owned by the top main scope", symbol);
            return false;
        }
        if !matches!(
            binding_kind(self.scoping, self.nodes, symbol_id),
            BindingKind::Variable
        ) {
            self.skip_binding(role, "main binding is not an ordinary variable", symbol);
            return false;
        }
        if self.single_origin(symbol_id).is_none() {
            self.skip_binding(
                role,
                "binding has conflicting declarations or reassignments",
                symbol,
            );
            return false;
        }
        let declaration = self.declaration_span(symbol_id);
        let entity = NamingEntity {
            kind: NamingEntityKind::MainData,
            span: SourceSpan::from(declaration),
            id: mv_id(SourceSpan::from(declaration)),
            target: None,
        };
        self.finish_claim(
            role,
            symbol,
            symbol_id,
            name,
            confidence,
            vec![evidence.to_string()],
            entity,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn finish_claim(
        &mut self,
        role: &str,
        symbol: u32,
        symbol_id: SymbolId,
        name: &str,
        confidence: Confidence,
        evidence: Vec<String>,
        entity: NamingEntity,
    ) -> bool {
        if let Some(listed) = vocabulary_name(role) {
            if listed != name {
                self.skip_binding(
                    role,
                    &format!("proposed name `{name}` differs from the vocabulary name `{listed}`"),
                    symbol,
                );
                return false;
            }
        }
        if let Some(existing) = self.claimed.get(&symbol) {
            if existing != role {
                // Two incompatible roles on one binding: no silent first
                // winner. Remove the candidate and leave the binding unnamed.
                let existing = existing.clone();
                self.candidates
                    .retain(|candidate| candidate.symbol != symbol);
                self.claimed.remove(&symbol);
                self.conflicted.insert(symbol);
                self.record_ambiguity(
                    &format!("role_conflict:{existing}|{role}"),
                    &format!(
                        "one binding matched the incompatible roles `{existing}` and `{role}`; no rename is emitted"
                    ),
                    vec![self.involved(symbol)],
                );
            }
            return false;
        }
        let declaration = self.declaration_span(symbol_id);
        let identifier = self.scoping.symbol_span(symbol_id);
        self.claimed.insert(symbol, role.to_string());
        self.candidates.push(NamingCandidate {
            role: role.to_string(),
            entity,
            original_name: self.scoping.symbol_name(symbol_id).to_string(),
            proposed_name: name.to_string(),
            symbol,
            binding_kind: binding_kind(self.scoping, self.nodes, symbol_id),
            identifier: SourceSpan::from(identifier),
            declaration: SourceSpan::from(declaration),
            confidence,
            evidence,
            behavior: BehaviorCoverage::RoleOnly,
        });
        true
    }

    fn declaration_span(&self, symbol: SymbolId) -> Span {
        self.nodes
            .get_node(self.scoping.symbol_declaration(symbol))
            .span()
    }

    /// The entity a function claim names: the function node itself, or an
    /// alias holder whose single origin is another proven function.
    fn function_entity(&self, symbol: SymbolId) -> NamingEntity {
        let declaration = SourceSpan::from(self.declaration_span(symbol));
        let direct = match self
            .nodes
            .get_node(self.scoping.symbol_declaration(symbol))
            .kind()
        {
            AstKind::Function(function) => Some(function.span),
            AstKind::VariableDeclarator(_) => match self.single_origin(symbol) {
                Some(Expression::FunctionExpression(function)) => Some(function.span),
                Some(Expression::ArrowFunctionExpression(function)) => Some(function.span),
                _ => None,
            },
            _ => None,
        };
        if let Some(span) = direct {
            let span = SourceSpan::from(span);
            return NamingEntity {
                kind: NamingEntityKind::Function,
                span,
                id: fn_id(span),
                target: None,
            };
        }
        NamingEntity {
            kind: NamingEntityKind::AliasHolder,
            span: declaration,
            id: mv_id(declaration),
            target: self.function_node_span(symbol, 4).map(fn_id),
        }
    }

    /// The function node an immutable function identity resolves to.
    fn function_node_span(&self, symbol: SymbolId, depth: u8) -> Option<SourceSpan> {
        if depth == 0 {
            return None;
        }
        match self
            .nodes
            .get_node(self.scoping.symbol_declaration(symbol))
            .kind()
        {
            AstKind::Function(function) => Some(SourceSpan::from(function.span)),
            AstKind::VariableDeclarator(_) => match self.single_origin(symbol)? {
                Expression::FunctionExpression(function) => Some(SourceSpan::from(function.span)),
                Expression::ArrowFunctionExpression(function) => {
                    Some(SourceSpan::from(function.span))
                }
                Expression::Identifier(identifier) => {
                    let target = analysis::identifier_symbol(identifier, self.scoping)?;
                    self.function_node_span(target, depth - 1)
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn skip_binding(&mut self, role: &str, reason: &str, symbol: u32) {
        let involved = vec![self.involved(symbol)];
        self.skipped.push(NamingSkip {
            role: role.to_string(),
            kind: SkipKind::Refused,
            reason: reason.to_string(),
            involved,
        });
    }

    fn record_ambiguity(&mut self, role: &str, reason: &str, involved: Vec<InvolvedSpan>) {
        self.ambiguous.push(NamingAmbiguity {
            role: role.to_string(),
            reason: reason.to_string(),
            candidates: involved.iter().map(involved_label).collect(),
            involved,
        });
    }

    /// Record the one equal-evidence tie for a finder function role. The shape
    /// mirrors the `once_callback_factory` ambiguity: one entry per tied role,
    /// named by the proposed name, with every tied function in `involved`.
    fn record_finder_function_ambiguity(&mut self, role: &str, name: &str, spans: &[Span]) {
        let involved: Vec<InvolvedSpan> =
            spans
                .iter()
                .map(|span| {
                    let span = SourceSpan::from(*span);
                    match self.fns.iter().position(|facts| {
                        facts.span.start == span.start && facts.span.end == span.end
                    }) {
                        Some(index) => self.fn_involved(index),
                        None => InvolvedSpan {
                            name: None,
                            span,
                            id: Some(fn_id(span)),
                        },
                    }
                })
                .collect();
        let labels = spans
            .iter()
            .map(|span| format!("function {}..{}", span.start, span.end))
            .collect::<Vec<_>>()
            .join(", ");
        self.record_ambiguity(
            role,
            &format!(
                "{} proven functions would receive `{name}` in one scope: {labels}",
                spans.len()
            ),
            involved,
        );
    }

    /// Record the one equal-evidence tie for a finder main-binding role. No
    /// binding is resolved; every tied binding is named in the reason.
    fn record_finder_main_ambiguity(&mut self, role: &str, name: &str, spans: &[Span]) {
        let involved: Vec<InvolvedSpan> = spans
            .iter()
            .map(|span| match self.symbol_at_identifier(*span) {
                Some(symbol) => {
                    let mut involved = self.involved(symbol);
                    if let Some(symbol_id) = self.symbols.get(&symbol).copied() {
                        involved.id =
                            Some(mv_id(SourceSpan::from(self.declaration_span(symbol_id))));
                    }
                    involved
                }
                None => InvolvedSpan {
                    name: None,
                    span: SourceSpan::from(*span),
                    id: None,
                },
            })
            .collect();
        let labels = involved
            .iter()
            .map(involved_label)
            .collect::<Vec<_>>()
            .join(", ");
        self.record_ambiguity(
            role,
            &format!(
                "{} proven main bindings would receive `{name}` in one scope: {labels}",
                spans.len()
            ),
            involved,
        );
    }

    /// A binding by its declaring identifier in this parse.
    fn involved(&self, symbol_index: u32) -> InvolvedSpan {
        match self.symbols.get(&symbol_index) {
            Some(symbol) => InvolvedSpan {
                name: Some(self.scoping.symbol_name(*symbol).to_string()),
                span: SourceSpan::from(self.scoping.symbol_span(*symbol)),
                id: None,
            },
            None => InvolvedSpan {
                name: None,
                span: SourceSpan::new(0, 0),
                id: None,
            },
        }
    }

    /// A function node, labelled by its own name when it has one.
    fn fn_involved(&self, index: usize) -> InvolvedSpan {
        let span = SourceSpan::from(self.fns[index].span);
        match self.fns[index].symbol {
            Some(symbol) => InvolvedSpan {
                name: Some(self.scoping.symbol_name(symbol).to_string()),
                span: SourceSpan::from(self.scoping.symbol_span(symbol)),
                id: Some(fn_id(span)),
            },
            None => InvolvedSpan {
                name: None,
                span,
                id: Some(fn_id(span)),
            },
        }
    }

    // -----------------------------------------------------------------------
    // Binding-origin proofs (fail closed)
    // -----------------------------------------------------------------------

    fn write_count(&self, symbol: SymbolId) -> usize {
        self.scoping
            .get_resolved_references(symbol)
            .filter(|reference| reference.is_write())
            .count()
    }

    fn init_value(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        match self
            .nodes
            .get_node(self.scoping.symbol_declaration(symbol))
            .kind()
        {
            AstKind::VariableDeclarator(declarator) => declarator.init.as_ref(),
            _ => None,
        }
    }

    fn plain_assignments(&self, symbol: SymbolId) -> &[&'a Expression<'a>] {
        self.assignments
            .get(&(symbol.index() as u32))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// The single, unambiguous source value of a data binding. Every
    /// declaration origin and every write is counted: a repeated `var` with two
    /// initializers, an initializer plus an assignment, a compound/update write
    /// and any write the plain-assignment inventory does not model all return
    /// `None`.
    fn single_origin(&self, symbol: SymbolId) -> Option<&'a Expression<'a>> {
        if self.scoping.symbol_declarations(symbol).count() != 1 {
            return None;
        }
        let assignments = self.plain_assignments(symbol);
        if self.write_count(symbol) != assignments.len() {
            return None;
        }
        match (self.init_value(symbol), assignments.len()) {
            (Some(value), 0) => Some(value),
            (None, 1) => assignments.first().copied(),
            _ => None,
        }
    }

    /// A function identity: exactly one declaration origin, no write beyond the
    /// single plain assignment that established it (if any), and a
    /// function-valued origin.
    fn function_identity(&self, symbol: SymbolId) -> bool {
        self.function_identity_depth(symbol, 4)
    }

    fn function_identity_depth(&self, symbol: SymbolId, depth: u8) -> bool {
        if depth == 0 {
            return false;
        }
        if self.scoping.symbol_declarations(symbol).count() != 1 {
            return false;
        }
        match self
            .nodes
            .get_node(self.scoping.symbol_declaration(symbol))
            .kind()
        {
            AstKind::Function(function) => {
                function.body.is_some()
                    && self.fns.iter().any(|facts| facts.symbol == Some(symbol))
                    && self.write_count(symbol) == 0
            }
            AstKind::VariableDeclarator(_) => match self.single_origin(symbol) {
                Some(value) => self.function_value(value, symbol, depth - 1),
                None => false,
            },
            _ => false,
        }
    }

    fn function_value(&self, value: &'a Expression<'a>, owner: SymbolId, depth: u8) -> bool {
        match value {
            Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => true,
            Expression::Identifier(identifier) => {
                match analysis::identifier_symbol(identifier, self.scoping) {
                    Some(target) if target != owner => self.function_identity_depth(target, depth),
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn aliases_of(&self, target: SymbolId) -> Vec<u32> {
        let mut out = Vec::new();
        for &candidate in &self.main_ids {
            let Some(symbol) = self.symbols.get(&candidate).copied() else {
                continue;
            };
            let Some(value) = self.single_origin(symbol) else {
                continue;
            };
            if let Expression::Identifier(identifier) = value {
                if analysis::identifier_symbol(identifier, self.scoping) == Some(target) {
                    out.push(candidate);
                }
            }
        }
        out.sort_unstable();
        out
    }

    fn call_results_of(&self, factory: SymbolId) -> Vec<u32> {
        let mut out = Vec::new();
        for &candidate in &self.main_ids {
            let Some(symbol) = self.symbols.get(&candidate).copied() else {
                continue;
            };
            let Some(value) = self.single_origin(symbol) else {
                continue;
            };
            if let Expression::CallExpression(call) = value {
                if analysis::expression_symbol(&call.callee, self.scoping) == Some(factory) {
                    out.push(candidate);
                }
            }
        }
        out.sort_unstable();
        out
    }

    /// Claim the single main-owned alias of a proven function. The target must
    /// itself be an immutable function identity; several aliases leave the role
    /// ambiguous instead of selecting one by source order.
    fn claim_alias(
        &mut self,
        role: &str,
        target: SymbolId,
        name: &str,
        confidence: Confidence,
        evidence: &str,
    ) -> bool {
        if !self.function_identity(target) {
            return false;
        }
        let aliases = self.aliases_of(target);
        match aliases.len() {
            0 => false,
            1 => self.claim_function(
                role,
                aliases[0],
                name,
                confidence,
                vec![evidence.to_string()],
            ),
            _ => {
                let labels = aliases
                    .iter()
                    .map(|index| self.involved(*index))
                    .collect::<Vec<_>>();
                self.record_ambiguity(
                    role,
                    "more than one main alias resolves to one function",
                    labels,
                );
                false
            }
        }
    }

    // -----------------------------------------------------------------------
    // Core roles from the structural report
    // -----------------------------------------------------------------------

    /// [`Self::claim_function`] for a report binding joined by span.
    fn claim_function_ref(
        &mut self,
        role: &str,
        binding: &BindingRef,
        name: &str,
        confidence: Confidence,
        evidence: Vec<String>,
    ) -> Option<u32> {
        let symbol = self.ref_symbol(role, binding)?;
        self.claim_function(role, symbol, name, confidence, evidence)
            .then_some(symbol)
    }

    /// [`Self::claim_main_data`] for a report binding joined by span.
    fn claim_main_data_ref(
        &mut self,
        role: &str,
        binding: &BindingRef,
        name: &str,
        confidence: Confidence,
        evidence: &str,
    ) -> bool {
        match self.ref_symbol(role, binding) {
            Some(symbol) => self.claim_main_data(role, symbol, name, confidence, evidence),
            None => false,
        }
    }

    fn core_roles(&mut self) {
        let properties = self.properties;
        if let Some(binding) = properties.program_scope.main.binding.clone() {
            self.claim_function_ref(
                "agent_module",
                &binding,
                "createAgentModule",
                Confidence::High,
                vec!["the broadest reachable main program scope".to_string()],
            );
        }

        if let Some(session) = &properties.session {
            if let Some(binding) = session.constructor.binding.clone() {
                self.claim_function_ref(
                    "session_runtime",
                    &binding,
                    "createSessionRuntime",
                    Confidence::High,
                    vec!["returns { init, teardown } and builds the session context".to_string()],
                );
                if let Some(target) = self
                    .ref_symbol_quiet(&binding)
                    .and_then(|symbol| self.symbols.get(&symbol).copied())
                {
                    self.claim_alias(
                        "session_runtime_constructor",
                        target,
                        "sessionRuntimeConstructor",
                        Confidence::High,
                        "direct main-scope alias of the session runtime constructor",
                    );
                }
            }
            if let Some(binding) = session.context_binding.clone() {
                self.claim_main_data_ref(
                    "session_context",
                    &binding,
                    "sessionContext",
                    Confidence::Medium,
                    "context object carrying the session fields",
                );
            }
            if let Some(init) = session.init.clone() {
                if let Some(binding) = init.binding {
                    self.claim_function_ref(
                        "session_runtime_init",
                        &binding,
                        "initializeSessionRuntime",
                        Confidence::High,
                        vec!["the session constructor's returned init".to_string()],
                    );
                }
            }
            if let Some(chain) = &session.hash_chain {
                self.claim_main_data_ref(
                    "transaction_token_hash_seed",
                    &chain.prng_state,
                    "transactionTokenHashSeed",
                    Confidence::Medium,
                    "the PRNG state is the hash of the session transaction token",
                );
            }
        }

        // The main function's own returned `init` (the exported entrypoint).
        let main_span = properties.program_scope.main.span;
        let returned =
            self.returned_function_property(Span::new(main_span.start, main_span.end), "init");
        match returned.len() {
            0 => {}
            1 => {
                self.claim_function(
                    "session_bootstrap",
                    returned[0],
                    "bootstrapSessionRuntime",
                    Confidence::High,
                    vec!["the function main returns as its `init` entrypoint".to_string()],
                );
            }
            _ => {
                let labels = returned.iter().map(|index| self.involved(*index)).collect();
                self.record_ambiguity("session_bootstrap", "more than one returned `init`", labels);
            }
        }

        // Helper action names and their distinct main-owned aliases.
        let helpers = properties.helpers.clone();
        for helper in &helpers {
            let Some(binding) = helper.function.binding.clone() else {
                continue;
            };
            let (role, action, alias_role, alias) = match helper.role {
                crate::signals::schema::BindingRole::WriteNumber => (
                    "write_number",
                    "writeNumber",
                    "number_field_writer",
                    "numberFieldWriter",
                ),
                crate::signals::schema::BindingRole::WriteString => (
                    "string_intern_writer",
                    "createStringInternWriter",
                    "string_intern_writer_factory",
                    "stringInternWriterFactory",
                ),
                crate::signals::schema::BindingRole::CreateSignalMask => (
                    "prng_byte_source",
                    "createPrngByteSource",
                    "prng_byte_source_factory",
                    "prngByteSourceFactory",
                ),
                crate::signals::schema::BindingRole::HashTransactionToken => (
                    "string_buffer_hash",
                    "hashStringOrBuffer",
                    "string_buffer_hash_alias",
                    "stringBufferHash",
                ),
                crate::signals::schema::BindingRole::ObserverFactory => (
                    "signal_observer",
                    "createSignalObserver",
                    "signal_observer_factory",
                    "signalObserverFactory",
                ),
                crate::signals::schema::BindingRole::SnapshotGetter => (
                    "signal_report_builder",
                    "buildSignalReport",
                    "signal_report_builder_alias",
                    "signalReportBuilder",
                ),
                _ => continue,
            };
            let evidence = if helper.notes.is_empty() {
                vec![format!("helper role {:?}", helper.role)]
            } else {
                helper.notes.clone()
            };
            self.claim_function_ref(role, &binding, action, helper.confidence, evidence);
            if let Some(target) = self
                .ref_symbol_quiet(&binding)
                .and_then(|symbol| self.symbols.get(&symbol).copied())
            {
                self.claim_alias(
                    alias_role,
                    target,
                    alias,
                    Confidence::High,
                    "direct main-scope alias of the recognized helper",
                );
            }
        }

        // Data containers and stores proved by the report.
        if let Some(descriptors) = &properties.descriptors {
            self.claim_main_data_ref(
                "signal_descriptor_registry",
                &descriptors.binding,
                "signalDescriptorRegistry",
                Confidence::Medium,
                "descriptor array with ctor/priority/name elements and index population writes",
            );
        }
        if let Some(waves) = &properties.waves {
            self.claim_main_data_ref(
                "wave_collector_factories",
                &waves.function_array,
                "waveCollectorFactories",
                Confidence::Medium,
                "the per-wave collector function array",
            );
            self.claim_main_data_ref(
                "wave_signal_position_map",
                &waves.index_rows,
                "waveSignalPositionMap",
                Confidence::Medium,
                "the per-wave signal-position index rows",
            );
            self.claim_main_data_ref(
                "wave_routing_flags",
                &waves.routing_selector,
                "waveRoutingFlags",
                Confidence::Medium,
                "the per-wave routing selector",
            );
        }
        if let Some(session) = &properties.session {
            if let Some(binding) = session.stores.latest_values.clone() {
                self.claim_main_data_ref(
                    "signal_value_slots",
                    &binding,
                    "signalValueSlots",
                    Confidence::Medium,
                    "the per-signal value slot array",
                );
            }
            if let Some(binding) = session.stores.statuses.clone() {
                self.claim_main_data_ref(
                    "signal_status_slots",
                    &binding,
                    "signalStatusSlots",
                    Confidence::Medium,
                    "the per-signal status slot array",
                );
            }
            if let Some(binding) = session.stores.debug_names.clone() {
                self.claim_main_data_ref(
                    "signal_debug_names",
                    &binding,
                    "signalDebugNames",
                    Confidence::Low,
                    "the per-signal debug-name array",
                );
            }
            if let Some(binding) = session.stores.error_buffer.clone() {
                self.claim_main_data_ref(
                    "error_ring_buffer",
                    &binding,
                    "errorRingBuffer",
                    Confidence::Medium,
                    "the capacity-normalising error ring buffer",
                );
            }
            if let Some(binding) = session.stores.sentinel_binding.clone() {
                self.claim_main_data_ref(
                    "empty_signal_sentinel",
                    &binding,
                    "emptySignalSentinel",
                    Confidence::Medium,
                    "the unique null-prototype object marking `no value collected`",
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Extra functions
    // -----------------------------------------------------------------------

    /// Pick the innermost function that satisfies `predicate`.
    ///
    /// Matching ancestors that lexically contain another match are discarded:
    /// the contained match is the more precise owner of the shape. If more than
    /// one unrelated minimal match remains the role is ambiguous. Byte length,
    /// source order and name spelling are never tie-breakers.
    fn pick_fn(&self, predicate: impl Fn(&FnFacts) -> bool) -> Pick {
        let matches: Vec<usize> = self
            .fns
            .iter()
            .enumerate()
            .filter(|(_, facts)| predicate(facts))
            .map(|(index, _)| index)
            .collect();
        self.pick_minimal(matches)
    }

    fn pick_minimal(&self, matches: Vec<usize>) -> Pick {
        if matches.is_empty() {
            return Pick::None;
        }
        let minimal: Vec<usize> = matches
            .iter()
            .copied()
            .filter(|&index| {
                !matches
                    .iter()
                    .any(|&other| other != index && self.fn_contains(index, other))
            })
            .collect();
        match minimal.as_slice() {
            [] => Pick::None,
            [index] => Pick::One(*index),
            _ => Pick::Ambiguous(
                minimal
                    .iter()
                    .map(|index| self.fn_involved(*index))
                    .collect(),
            ),
        }
    }

    /// True when `outer`'s source span strictly contains `inner`'s.
    fn fn_contains(&self, outer: usize, inner: usize) -> bool {
        let outer_span = self.fns[outer].span;
        let inner_span = self.fns[inner].span;
        outer_span != inner_span
            && outer_span.start <= inner_span.start
            && outer_span.end >= inner_span.end
    }

    fn fn_span(&self, index: usize) -> SourceSpan {
        SourceSpan::from(self.fns[index].span)
    }

    /// The function that owns the callback at `index`: climb while the
    /// function is an owned callback of its parent.
    fn callback_owner(&self, mut index: usize) -> usize {
        while self.fns[index].callback {
            match self.fns[index].enclosing {
                Some(parent) => index = parent,
                None => break,
            }
        }
        index
    }

    // Structural predicates. Each requires the actual call/dataflow that owns
    // the role, not a bag of string or member spellings, and reads only the
    // function's own statements or owned callbacks (never arbitrary
    // descendant code).

    fn is_aes_encryptor(facts: &FnFacts) -> bool {
        facts.owned_strings.contains("AES-GCM")
            && facts.owned_calls.contains("generateKey")
            && facts.owned_calls.contains("exportKey")
            && facts.owned_calls.contains("encrypt")
            && (facts.owned_calls.contains("encode")
                || facts.owned_members.contains("Uint8Array")
                || facts.owned_members.contains("byteLength"))
    }

    fn is_aes_decryptor(facts: &FnFacts) -> bool {
        facts.owned_strings.contains("AES-GCM")
            && facts.owned_calls.contains("importKey")
            && facts.owned_calls.contains("decrypt")
            && (facts.owned_calls.contains("decode")
                || facts.owned_members.contains("TextDecoder")
                || facts.owned_calls.contains("from"))
    }

    /// The dispatcher tests one of its own input slots — a named formal or
    /// `arguments[N]`, one slot under the shared resolver — both with
    /// `typeof … === "string"` and with `isView(…)`, and ends in a
    /// `TypeError`. A test of a local or a captured value is not dispatch.
    fn is_hash_dispatcher(facts: &FnFacts) -> bool {
        facts.new_type_error
            && facts
                .typeof_string_slots
                .iter()
                .any(|slot| facts.is_view_slots.contains(slot))
    }

    /// A cipher factory is only a factory: it must return a nested function
    /// and one of its immediate nested functions must prove the wide-state XOR
    /// mixer loop (mixers are counted on the direct parent only).
    fn is_arx_cipher_factory(facts: &FnFacts) -> bool {
        facts.takes_no_inputs()
            && facts.direct_members.contains("pow")
            && facts.returns_nested_function
            && facts.nested_mixers >= 1
    }

    /// An observable class factory declares and returns its own class (or
    /// constructor function): the constructor stores `_subscriber` and the
    /// class itself provides `subscribe` and `pipe`. Code elsewhere inside the
    /// factory, or a function that merely uses an observable, does not count.
    fn is_observable_factory(&self, index: usize) -> bool {
        if !self.fns[index].takes_no_inputs() {
            return false;
        }
        let Some((members, constructor_writes)) = self.returned_implementation(index) else {
            return false;
        };
        constructor_writes.contains("_subscriber")
            && members.contains("subscribe")
            && members.contains("pipe")
    }

    /// The class or constructor function that the function at `index` itself
    /// declares and returns, through single-origin local aliases: its member
    /// names (class body plus `C.key =` / `C.prototype.key =` writes at the
    /// function's own level) and the keys its constructor writes.
    fn returned_implementation(
        &self,
        index: usize,
    ) -> Option<(BTreeSet<String>, BTreeSet<String>)> {
        let returned: Vec<u32> = self.fns[index].direct_returned.iter().copied().collect();
        let [returned] = returned.as_slice() else {
            return None;
        };
        let mut chain: Vec<u32> = vec![*returned];
        let mut current = self.symbols.get(returned).copied()?;
        // Each alias hop is a single-origin proof; the chain is short.
        let (class, constructor) = loop {
            if chain.len() > 4 {
                return None;
            }
            if let Some(class) = self
                .classes
                .iter()
                .find(|class| class.symbol == Some(current) && class.enclosing == Some(index))
            {
                break (Some(class), class.constructor);
            }
            if let Some(function) = self.fns.iter().find(|facts| {
                facts.symbol == Some(current)
                    && facts.is_declaration
                    && facts.enclosing == Some(index)
            }) {
                break (None, Some(function.span));
            }
            match self.single_origin(current)? {
                Expression::Identifier(identifier) => {
                    current = analysis::identifier_symbol(identifier, self.scoping)?;
                    chain.push(current.index() as u32);
                }
                Expression::ClassExpression(expression) => {
                    let class = self.classes.iter().find(|class| {
                        class.span == expression.span && class.enclosing == Some(index)
                    })?;
                    break (Some(class), class.constructor);
                }
                Expression::FunctionExpression(function) => {
                    let owned = self
                        .fns
                        .iter()
                        .any(|facts| facts.span == function.span && facts.enclosing == Some(index));
                    if !owned {
                        return None;
                    }
                    break (None, Some(function.span));
                }
                _ => return None,
            }
        };
        if let Some(symbol) = class.and_then(|class| class.symbol) {
            chain.push(symbol.index() as u32);
        }
        if class.is_none() {
            if let Some(function) = constructor
                .and_then(|span| self.fns.iter().find(|facts| facts.span == span))
                .and_then(|facts| facts.symbol)
            {
                chain.push(function.index() as u32);
            }
        }
        let mut members: BTreeSet<String> =
            class.map(|class| class.members.clone()).unwrap_or_default();
        for write in &self.key_writes {
            if write.owner == Some(index) && chain.contains(&write.target) {
                members.insert(write.key.clone());
            }
        }
        let constructor_writes = constructor
            .and_then(|span| self.fns.iter().find(|facts| facts.span == span))
            .map(|facts| facts.direct_written_keys.clone())
            .unwrap_or_default();
        Some((members, constructor_writes))
    }

    fn is_method_interceptor(facts: &FnFacts) -> bool {
        facts.direct_written_keys.contains("onBeforeInvoke")
            && facts.direct_written_keys.contains("onAfterInvoke")
    }

    fn is_accessor_interceptor(facts: &FnFacts) -> bool {
        facts.direct_written_keys.contains("onBeforeSet")
            && facts.direct_written_keys.contains("onBeforeGet")
    }

    fn is_status_fanout(facts: &FnFacts) -> bool {
        facts.owned_written_keys.contains("collectedSignals")
            && facts.owned_written_keys.contains("erroredSignals")
    }

    /// A DOM field collector actually queries, reads values, hashes and emits
    /// an object. A member/string mention without that flow proves nothing.
    fn is_dom_collector(facts: &FnFacts, hash: u32) -> bool {
        (facts.direct_calls.contains("querySelectorAll")
            || facts.direct_calls.contains("querySelector"))
            && facts.direct_query_field_writes >= 2
            && facts.owned_refs.contains(&hash)
            && !facts.owned_written_keys.is_empty()
    }

    fn extra_functions(&mut self) {
        // AES-GCM string encryptor and its packed-blob decryptor.
        let encryptor = self.pick_fn(Self::is_aes_encryptor);
        self.claim_picked_action(
            encryptor,
            "encrypt_string_aes_gcm",
            "encryptStringAesGcm",
            "aes_gcm_string_encryptor",
            "aesGcmStringEncryptor",
            "calls generateKey/exportKey and encrypts encoded string bytes",
        );
        let decryptor = self.pick_fn(Self::is_aes_decryptor);
        self.claim_picked_action(
            decryptor,
            "decrypt_packed_aes_gcm_blob",
            "decryptPackedAesGcmBlob",
            "packed_aes_gcm_blob_decryptor",
            "packedAesGcmBlobDecryptor",
            "imports the packed blob key and decrypts its bytes",
        );

        // Async hash dispatcher: string vs ArrayBuffer view, else TypeError.
        let hash_helpers: BTreeSet<u32> = self
            .properties
            .helpers
            .iter()
            .filter(|helper| helper.role == crate::signals::schema::BindingRole::HashTransactionToken)
            .filter_map(|helper| helper.function.binding.as_ref())
            .filter_map(|binding| self.ref_symbol_quiet(binding))
            .collect();
        let dispatch = self.pick_fn(|facts| {
            Self::is_hash_dispatcher(facts)
                && facts.symbol.map_or(true, |symbol| {
                    !hash_helpers.contains(&(symbol.index() as u32))
                })
        });
        self.claim_picked_action(
            dispatch,
            "dispatch_hash_by_type",
            "dispatchHashByType",
            "hash_dispatch",
            "hashDispatch",
            "dispatches a string or an ArrayBuffer view to a hash implementation",
        );

        // ARX keystream factory: a factory taking no inputs (no formals, no
        // `arguments`) returning a nested function whose body XORs a wide
        // state.
        let cipher = self.pick_fn(Self::is_arx_cipher_factory);
        self.claim_factory(
            cipher,
            "stream_cipher_factory",
            "createChaChaLikeStreamCipher",
            "nullary factory returning a nested function that XORs a wide state in a mixer loop",
            "more than one ARX keystream factory matched the structural shape",
            ("stream_cipher", "streamCipher"),
            "the keystream function returned by the cipher factory",
        );

        // Observable class factory: only the class it declares and returns.
        let observable_matches: Vec<usize> = (0..self.fns.len())
            .filter(|index| self.is_observable_factory(*index))
            .collect();
        let observable = self.pick_minimal(observable_matches);
        self.claim_factory(
            observable,
            "observable_class",
            "createObservableClass",
            "nullary factory declaring and returning a subscriber-backed class with subscribe/pipe",
            "more than one observable class factory matched the structural shape",
            ("observable", "Observable"),
            "the observable constructor returned by the factory",
        );

        // Interception producers (callbacks may rewrite args / replace values).
        let method = self.pick_fn(Self::is_method_interceptor);
        self.claim_picked_action(
            method,
            "install_method_interceptor",
            "installMethodInterceptor",
            "method_interceptor_installer",
            "methodInterceptorInstaller",
            "swaps a function property and builds onBeforeInvoke/onAfterInvoke hooks",
        );
        let accessor = self.pick_fn(Self::is_accessor_interceptor);
        self.claim_picked_action(
            accessor,
            "install_accessor_interceptor",
            "installAccessorInterceptor",
            "accessor_interceptor_installer",
            "accessorInterceptorInstaller",
            "swaps an accessor pair and builds onBefore/onAfter get/set hooks",
        );

        // Status fan-out and engine bootstrap, anchored on the core registries.
        // The fan-out writes its enriched status object in a callback, so the
        // role belongs to the function that owns that callback.
        let writer = self.pick_fn(Self::is_status_fanout);
        let writer = match writer {
            Pick::One(inner) => Pick::One(self.callback_owner(inner)),
            other => other,
        };
        self.claim_picked_action(
            writer,
            "dispatch_signal_status",
            "dispatchSignalStatus",
            "signal_status_dispatcher",
            "signalStatusDispatcher",
            "fans observer status out to the registered callbacks with aggregate counts",
        );

        let (registry, wave_registry, index_rows) = self.registry_symbols();
        if let (Some(registry), Some(wave_registry), Some(index_rows)) =
            (registry, wave_registry, index_rows)
        {
            let bootstrap = self.pick_fn(|facts| {
                facts.owned_refs.contains(&registry)
                    && facts.owned_refs.contains(&wave_registry)
                    && facts.owned_refs.contains(&index_rows)
            });
            self.claim_picked_action(
                bootstrap,
                "bootstrap_signal_engine",
                "bootstrapSignalEngine",
                "signal_engine_bootstrapper",
                "signalEngineBootstrapper",
                "subscribes every registry slot and wave group and returns the queue's onEmpty",
            );
        }
    }

    /// Claim a picked factory, then the single main binding holding its call
    /// result.
    #[allow(clippy::too_many_arguments)]
    fn claim_factory(
        &mut self,
        pick: Pick,
        role: &str,
        action: &str,
        evidence: &str,
        ambiguity: &str,
        (result_role, result_name): (&str, &str),
        result_evidence: &str,
    ) {
        let index = match pick {
            Pick::None => return,
            Pick::One(index) => index,
            Pick::Ambiguous(involved) => {
                self.record_ambiguity(role, ambiguity, involved);
                return;
            }
        };
        let span = self.fn_span(index);
        let Some(symbol) = self.claim_function_span(
            role,
            span,
            action,
            None,
            Confidence::Medium,
            vec![evidence.to_string()],
        ) else {
            return;
        };
        let Some(factory) = self.symbols.get(&symbol).copied() else {
            return;
        };
        let results = self.call_results_of(factory);
        match results.as_slice() {
            [] => {}
            [result] => {
                self.claim_main_data(
                    result_role,
                    *result,
                    result_name,
                    Confidence::Medium,
                    result_evidence,
                );
            }
            _ => {
                let involved = results.iter().map(|index| self.involved(*index)).collect();
                self.record_ambiguity(
                    result_role,
                    "factory result bindings are not unique",
                    involved,
                );
            }
        }
    }

    fn claim_picked_action(
        &mut self,
        pick: Pick,
        role: &str,
        action: &str,
        alias_role: &str,
        alias: &str,
        evidence: &str,
    ) -> Option<u32> {
        let index = match pick {
            Pick::None => return None,
            Pick::One(index) => index,
            Pick::Ambiguous(involved) => {
                self.record_ambiguity(
                    role,
                    "more than one function matched the structural shape",
                    involved,
                );
                return None;
            }
        };
        let span = self.fn_span(index);
        self.claim_function_span(
            role,
            span,
            action,
            Some((alias, alias_role)),
            Confidence::Medium,
            vec![evidence.to_string()],
        )
    }

    fn registry_symbols(&self) -> (Option<u32>, Option<u32>, Option<u32>) {
        let registry = self
            .properties
            .descriptors
            .as_ref()
            .and_then(|descriptors| self.ref_symbol_quiet(&descriptors.binding));
        let (wave, rows) = self
            .properties
            .waves
            .as_ref()
            .map(|waves| {
                (
                    self.ref_symbol_quiet(&waves.function_array),
                    self.ref_symbol_quiet(&waves.index_rows),
                )
            })
            .unwrap_or((None, None));
        (registry, wave, rows)
    }

    // -----------------------------------------------------------------------
    // Extra main bindings
    // -----------------------------------------------------------------------

    fn extra_main_bindings(&mut self) {
        let entries: Vec<(u32, SymbolId)> = self
            .main_ids
            .iter()
            .filter_map(|index| self.symbols.get(index).map(|symbol| (*index, *symbol)))
            .collect();

        // Interception records and the interceptor specs manifest. A record is
        // the result of calling a recognized installer, directly or through a
        // proven single-origin alias chain that ends at the installer's
        // function node; a mutable alias proves nothing.
        let installers: Vec<SourceSpan> = self
            .candidates
            .iter()
            .filter(|candidate| {
                matches!(
                    candidate.role.as_str(),
                    "install_method_interceptor" | "install_accessor_interceptor"
                ) && candidate.entity.kind == NamingEntityKind::Function
            })
            .map(|candidate| candidate.entity.span)
            .collect();
        for (index, symbol) in &entries {
            let Some(value) = self.single_origin(*symbol) else {
                continue;
            };
            let Expression::CallExpression(call) = value else {
                continue;
            };
            let Some(callee) = analysis::expression_symbol(&call.callee, self.scoping) else {
                continue;
            };
            let calls_installer = self.function_identity(callee)
                && self
                    .function_node_span(callee, 4)
                    .is_some_and(|span| installers.contains(&span));
            if calls_installer {
                let (Some(target), Some(property)) =
                    (call.arguments.first(), call.arguments.get(1))
                else {
                    continue;
                };
                let Some(property) = string_literal(property) else {
                    continue;
                };
                let Some(target_expression) = target.as_expression() else {
                    continue;
                };
                let target_members = expr_members(target_expression);
                let base = interception_base(&property, &target_members);
                let name = format!("{base}InterceptionRecord");
                self.claim_main_data(
                    "interception_record",
                    *index,
                    &name,
                    Confidence::High,
                    &format!(
                        "installed on the proven {property} target with the recognized interceptor"
                    ),
                );
                continue;
            }
            let _ = index;
        }

        // Interceptor specs manifest: an array of instrumentation descriptors.
        for (index, symbol) in &entries {
            let Some(value) = self.single_origin(*symbol) else {
                continue;
            };
            if !is_interceptor_specs(value) {
                continue;
            }
            self.claim_main_data(
                "interceptor_specs",
                *index,
                "interceptorSpecs",
                Confidence::Medium,
                "array of { name, property, type, isConstructor, location } descriptors",
            );
        }

        // Cryptographic references and the resolved global object.
        for (index, symbol) in &entries {
            let Some(value) = self.single_origin(*symbol) else {
                continue;
            };
            let scan = ExprScan::of(value);
            if scan.members.contains("crypto") && scan.members.contains("subtle") {
                self.claim_main_data(
                    "crypto_subtle",
                    *index,
                    "cryptoSubtle",
                    Confidence::High,
                    "assigned the resolved global's `crypto.subtle` capability",
                );
            } else if matches!(value, Expression::ConditionalExpression(_))
                && scan.members.contains("global")
                && scan.identifiers.contains("window")
            {
                self.claim_main_data(
                    "resolved_global_object",
                    *index,
                    "resolvedGlobalObject",
                    Confidence::High,
                    "resolves global/self/window to the host global object",
                );
            }
        }

        // Two window-valued slots, disambiguated by their proven consumers.
        let mut windows: Vec<(u32, SymbolId)> = Vec::new();
        for (index, symbol) in &entries {
            let Some(value) = self.single_origin(*symbol) else {
                continue;
            };
            if is_unresolved_identifier(value, "window", self.scoping) {
                windows.push((*index, *symbol));
            }
        }
        let mut host: Option<u32> = None;
        let mut instrumentation: Option<u32> = None;
        let mut host_ambiguous = Vec::new();
        let mut instrumentation_ambiguous = Vec::new();
        for (index, _symbol) in &windows {
            let members = self.member_refs.get(index).cloned().unwrap_or_default();
            let is_host = members.contains("location");
            let is_instrumentation = members.contains("document")
                || members.contains("HTMLFormElement")
                || members.contains("Event");
            match (is_host, is_instrumentation) {
                (true, false) => {
                    if host.is_some() {
                        host_ambiguous.push(*index);
                    } else {
                        host = Some(*index);
                    }
                }
                (false, true) => {
                    if instrumentation.is_some() {
                        instrumentation_ambiguous.push(*index);
                    } else {
                        instrumentation = Some(*index);
                    }
                }
                (true, true) => {
                    host_ambiguous.push(*index);
                    instrumentation_ambiguous.push(*index);
                }
                (false, false) => {}
            }
        }
        let window_symbols: Vec<u32> = windows.iter().map(|(index, _)| *index).collect();
        if host_ambiguous.is_empty() {
            if let Some(index) = host {
                self.claim_main_data(
                    "host_window",
                    index,
                    "hostWindow",
                    Confidence::Medium,
                    "window-valued slot consumed through `location` / anchor-href probes",
                );
            }
        } else {
            let labels = host_ambiguous
                .iter()
                .map(|index| self.involved(*index))
                .collect();
            self.record_ambiguity(
                "host_window",
                "several window slots consumed through `location`",
                labels,
            );
        }
        if instrumentation_ambiguous.is_empty() {
            if let Some(index) = instrumentation {
                self.claim_main_data(
                    "instrumentation_window",
                    index,
                    "instrumentationWindow",
                    Confidence::Medium,
                    "window-valued slot used for native prototype interception",
                );
            }
        } else {
            let labels = instrumentation_ambiguous
                .iter()
                .map(|index| self.involved(*index))
                .collect();
            self.record_ambiguity(
                "instrumentation_window",
                "several window slots used for prototype interception",
                labels,
            );
        }

        // The document derived from a proven window slot.
        for (index, symbol) in &entries {
            let Some(value) = self.single_origin(*symbol) else {
                continue;
            };
            let Some((object, property)) = member_access(value) else {
                continue;
            };
            if property != "document" {
                continue;
            }
            let Some(object) = analysis::expression_symbol(object, self.scoping) else {
                continue;
            };
            if window_symbols.contains(&(object.index() as u32)) {
                self.claim_main_data(
                    "document_ref",
                    *index,
                    "documentRef",
                    Confidence::Medium,
                    "the `document` of a proven window slot",
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // DOM field collector family
    // -----------------------------------------------------------------------

    fn dom_family(&mut self) {
        let hash_symbol = match self.string_buffer_hash_symbol() {
            Ok(Some(hash)) => hash,
            Ok(None) => {
                self.notes
                    .push("DOM field family not named: no proven string/byte hash".to_string());
                return;
            }
            Err(aliases) => {
                let labels = aliases.iter().map(|index| self.involved(*index)).collect();
                self.record_ambiguity(
                    "dom_field_collector",
                    "the proven string/byte hash has more than one main alias",
                    labels,
                );
                return;
            }
        };
        let collector = self.pick_fn(|facts| Self::is_dom_collector(facts, hash_symbol));
        let index = match collector {
            Pick::None => {
                self.notes
                    .push("DOM field family not named: no DOM collector matched".to_string());
                return;
            }
            Pick::One(index) => index,
            Pick::Ambiguous(labels) => {
                self.record_ambiguity(
                    "dom_field_collector",
                    "more than one DOM collector queried fields and hashed them",
                    labels,
                );
                return;
            }
        };
        // Each member of the family is recognized by its own structure. A
        // member without an editable binding (an anonymous arrow) is recorded
        // as role metadata by `claim_function_span` and never stops the
        // others from being recognized.
        let collector_span = self.fn_span(index);
        let collector_symbol = self.claim_function_span(
            "dom_field_collector",
            collector_span,
            "collectDomFields",
            None,
            Confidence::Medium,
            vec![
                "queries DOM fields, reads their values, hashes them and emits the field object"
                    .to_string(),
            ],
        );

        // The enclosing deferred step may be named only when it is an actual
        // factory: it returns a nested function or hands the collector to a
        // call (by a binding, or as an inline callback argument written at the
        // factory's own level). An arbitrary parent is not a source-factory
        // proof, and MAIN is never one.
        let main_span = self.properties.program_scope.main.span;
        let main_binding = self
            .properties
            .program_scope
            .main
            .binding
            .as_ref()
            .and_then(|binding| self.ref_symbol_quiet(binding));
        if let Some(enclosing) = self.fns[index].enclosing {
            let collector_names: BTreeSet<u32> = collector_symbol
                .into_iter()
                .chain(self.fns[index].symbol.map(|symbol| symbol.index() as u32))
                .chain(self.holder_candidates(self.fns[index].span))
                .collect();
            let parent = &self.fns[enclosing];
            let hands_off = parent.returns_nested_function
                || self.fns[index].inline_callback
                || collector_names
                    .iter()
                    .any(|symbol| parent.direct_call_args.contains(symbol));
            let is_main = SourceSpan::from(parent.span) == main_span
                || parent
                    .symbol
                    .is_some_and(|symbol| Some(symbol.index() as u32) == main_binding);
            if hands_off && !is_main {
                let span = self.fn_span(enclosing);
                let holder = self.single_origin_holders(parent.span);
                let is_main_holder = holder.iter().any(|symbol| Some(*symbol) == main_binding);
                if !is_main_holder {
                    self.claim_function_span(
                        "dom_signal_source_factory",
                        span,
                        "createDomSignalSource",
                        None,
                        Confidence::Low,
                        vec!["deferred-step factory that hands off the nested DOM step".to_string()],
                    );
                }
            } else if !hands_off {
                self.notes.push(
                    "DOM collector's parent is not a proven factory; left unnamed".to_string(),
                );
            }
        }

        // The first-child text adapter, when it is a real nested function.
        let nested: Vec<usize> = self.fns[index].nested.clone();
        for nested_index in nested {
            let is_reader = {
                let facts = &self.fns[nested_index];
                facts.direct_members.contains("firstChild") && facts.direct_members.contains("data")
            };
            if is_reader {
                let span = self.fn_span(nested_index);
                self.claim_function_span(
                    "first_child_text_reader",
                    span,
                    "readFirstChildText",
                    None,
                    Confidence::Medium,
                    vec!["maps an element to its first child's text data".to_string()],
                );
            }
        }
    }

    /// The unique main binding that holds the proven string/byte hash. Several
    /// aliases leave the consumer unproven: the caller reports ambiguity rather
    /// than choosing the first one. A hash binding whose span does not join to
    /// this parse is not proven.
    fn string_buffer_hash_symbol(&self) -> Result<Option<u32>, Vec<u32>> {
        for helper in &self.properties.helpers {
            if helper.role == crate::signals::schema::BindingRole::HashTransactionToken {
                if let Some(binding) = helper.function.binding.as_ref() {
                    let Some(hash) = self.ref_symbol_quiet(binding) else {
                        return Ok(None);
                    };
                    if let Some(symbol) = self.symbols.get(&hash).copied() {
                        match self.aliases_of(symbol).as_slice() {
                            [] => return Ok(Some(hash)),
                            [alias] => return Ok(Some(*alias)),
                            aliases => return Err(aliases.to_vec()),
                        }
                    }
                    return Ok(Some(hash));
                }
            }
        }
        Ok(None)
    }

    fn mark_unknown_surfaces(&mut self) {
        if self
            .properties
            .session
            .as_ref()
            .and_then(|session| session.serializer.as_ref())
            .is_some()
        {
            self.notes
                .push("payload encoder/serializer left unnamed: its role is a return value, not a binding".to_string());
        }
        self.notes.push(
            "same-producer sibling bindings (for example two hash-observable wrappers) are left unnamed rather than given numeric suffixes"
                .to_string(),
        );
    }

    // -----------------------------------------------------------------------
    // Small readers
    // -----------------------------------------------------------------------

    fn returned_function_property(&self, owner: Span, property: &str) -> Vec<u32> {
        let mut out = Vec::new();
        for node in self.nodes.iter() {
            let AstKind::ReturnStatement(ret) = node.kind() else {
                continue;
            };
            if self.enclosing_function_start(node.id()) != Some(owner.start) {
                continue;
            }
            let Some(argument) = ret.argument.as_ref() else {
                continue;
            };
            let Some(members) = object_members(argument) else {
                continue;
            };
            for member in members {
                if member.name != property {
                    continue;
                }
                if let Expression::Identifier(identifier) = member.value {
                    if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
                        out.push(symbol.index() as u32);
                    }
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    fn enclosing_function_start(&self, node: oxc_semantic::NodeId) -> Option<u32> {
        let mut current = node;
        for _ in 0..2048 {
            let parent = self.nodes.parent_id(current);
            if parent == current {
                return None;
            }
            current = parent;
            if let AstKind::Function(function) = self.nodes.kind(current) {
                return Some(function.span.start);
            }
        }
        None
    }
}

impl<'a> ExprScan {
    fn of(expression: &'a Expression<'a>) -> Self {
        let mut scan = Self::default();
        scan.visit_expression(expression);
        scan
    }
}

/// `fn@start..end`: the function index id of a function node.
fn fn_id(span: SourceSpan) -> String {
    format!("fn@{}..{}", span.start, span.end)
}

/// `mv@start..end`: the function index id of a main binding's declaration.
fn mv_id(span: SourceSpan) -> String {
    format!("mv@{}..{}", span.start, span.end)
}

/// The human label of an involved span, `name (start..end)` or
/// `<anonymous start..end>`.
fn involved_label(involved: &InvolvedSpan) -> String {
    match &involved.name {
        Some(name) => format!("{name} ({}..{})", involved.span.start, involved.span.end),
        None => format!("<anonymous {}..{}>", involved.span.start, involved.span.end),
    }
}

fn binding_kind(scoping: &Scoping, nodes: &AstNodes<'_>, symbol: SymbolId) -> BindingKind {
    match nodes.get_node(scoping.symbol_declaration(symbol)).kind() {
        AstKind::VariableDeclarator(_) => BindingKind::Variable,
        AstKind::Function(_) => BindingKind::Function,
        AstKind::FormalParameter(_) => BindingKind::Parameter,
        AstKind::CatchParameter(_) => BindingKind::Catch,
        AstKind::Class(_) => BindingKind::Class,
        _ => BindingKind::Other,
    }
}

fn string_literal(argument: &Argument<'_>) -> Option<String> {
    match argument {
        Argument::StringLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    }
}

fn expr_members(expression: &Expression<'_>) -> BTreeSet<String> {
    let mut scan = ExprScan::default();
    scan.visit_expression(expression);
    scan.members
}

fn interception_base(property: &str, target_members: &BTreeSet<String>) -> String {
    if property == "submit" && target_members.contains("HTMLFormElement") {
        return "formSubmit".to_string();
    }
    property.to_string()
}

fn is_interceptor_specs(expression: &Expression<'_>) -> bool {
    let Expression::ArrayExpression(array) = expression else {
        return false;
    };
    array.elements.iter().any(|element| {
        let Some(expression) = element.as_expression() else {
            return false;
        };
        let Expression::ObjectExpression(object) = expression else {
            return false;
        };
        let keys: BTreeSet<String> = object
            .properties
            .iter()
            .filter_map(|property| match property {
                ObjectPropertyKind::ObjectProperty(property) => property_key_name(&property.key),
                _ => None,
            })
            .collect();
        ["name", "property", "type", "isConstructor"]
            .iter()
            .all(|key| keys.contains(*key))
    })
}

fn member_access<'e>(expression: &'e Expression<'e>) -> Option<(&'e Expression<'e>, String)> {
    match expression {
        Expression::StaticMemberExpression(member) => {
            Some((&member.object, member.property.name.to_string()))
        }
        Expression::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some((&member.object, literal.value.to_string())),
            _ => None,
        },
        _ => None,
    }
}

/// True when an assignment's right-hand side is a DOM query/read, i.e. the
/// assignment is a query-to-field flow.
fn query_field_expression(expression: &Expression<'_>) -> bool {
    let scan = ExprScan::of(expression);
    [
        "querySelector",
        "querySelectorAll",
        "outerText",
        "textContent",
        "value",
        "getAttribute",
        "firstChild",
    ]
    .iter()
    .any(|name| scan.members.contains(*name))
}

struct SymbolCollector<'a> {
    scoping: &'a Scoping,
    symbols: BTreeSet<u32>,
}

impl<'a> Visit<'a> for SymbolCollector<'a> {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if let Some(symbol) = analysis::identifier_symbol(identifier, self.scoping) {
            self.symbols.insert(symbol.index() as u32);
        }
        walk::walk_identifier_reference(self, identifier);
    }
}

fn collect_identifier_symbols(
    expression: &Expression<'_>,
    scoping: &Scoping,
    out: &mut BTreeSet<u32>,
) {
    let mut collector = SymbolCollector {
        scoping,
        symbols: BTreeSet::new(),
    };
    collector.visit_expression(expression);
    out.extend(collector.symbols);
}

fn is_unresolved_identifier(expression: &Expression<'_>, name: &str, scoping: &Scoping) -> bool {
    match expression {
        Expression::Identifier(identifier) => {
            identifier.name == name && analysis::identifier_symbol(identifier, scoping).is_none()
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"(function main() {
  var mainValue = 1;
  function namedFn() { return 1; }
  function mutableFn() { return 1; }
  function dupFn() { return 1; }
  function dupFn() { return 2; }
  mutableFn = 2;
  var localFn = function () { return 1; };
  function nested() {
    var nestedValue = 3;
    return nestedValue;
  }
  return nested() + mainValue + localFn() + namedFn() + mutableFn + dupFn();
})();"#;

    fn symbol_named(scoping: &Scoping, name: &str) -> SymbolId {
        scoping
            .symbol_ids()
            .find(|symbol| scoping.symbol_name(*symbol) == name)
            .unwrap_or_else(|| panic!("symbol {name}"))
    }

    fn with_recognizer(check: impl FnOnce(&mut Recognizer<'_, '_>, &Scoping)) {
        with_recognizer_for(SOURCE, check);
    }

    fn with_recognizer_for(source: &str, check: impl FnOnce(&mut Recognizer<'_, '_>, &Scoping)) {
        let properties = extract_program_properties(source).expect("properties");
        let allocator = Allocator::default();
        let program = analysis::parse_program(&allocator, source).expect("parse");
        let (scoping, nodes) = analysis::analyze(&program);
        let mut recognizer = Recognizer::new(source, &properties, &program, &scoping, &nodes);
        check(&mut recognizer, &scoping);
    }

    #[test]
    fn eligibility_is_function_identity_or_top_level_main_data() {
        with_recognizer(|recognizer, scoping| {
            let main = symbol_named(scoping, "mainValue");
            assert!(recognizer.claim_main_data(
                "main_data",
                main.index() as u32,
                "mainData",
                Confidence::Medium,
                "test",
            ));

            let nested = symbol_named(scoping, "nestedValue");
            assert!(!recognizer.claim_main_data(
                "nested_local",
                nested.index() as u32,
                "nestedLocal",
                Confidence::Medium,
                "test",
            ));

            let function = symbol_named(scoping, "namedFn");
            assert!(recognizer.claim_function(
                "fn_role",
                function.index() as u32,
                "fnRole",
                Confidence::Medium,
                vec!["test".to_string()],
            ));
            assert!(!recognizer.claim_main_data(
                "fn_as_data",
                function.index() as u32,
                "fnAsData",
                Confidence::Medium,
                "test",
            ));

            let local_function = symbol_named(scoping, "localFn");
            assert!(recognizer.claim_function(
                "local_fn_role",
                local_function.index() as u32,
                "localFnRole",
                Confidence::Medium,
                vec!["test".to_string()],
            ));

            let mutable = symbol_named(scoping, "mutableFn");
            assert!(!recognizer.claim_function(
                "mutable_role",
                mutable.index() as u32,
                "mutableRole",
                Confidence::Medium,
                vec!["test".to_string()],
            ));

            let duplicate = symbol_named(scoping, "dupFn");
            assert!(!recognizer.claim_function(
                "dup_role",
                duplicate.index() as u32,
                "dupRole",
                Confidence::Medium,
                vec!["test".to_string()],
            ));

            assert!(!recognizer.claim_function(
                "nested_fn_role",
                nested.index() as u32,
                "nestedFnRole",
                Confidence::Medium,
                vec!["test".to_string()],
            ));
            assert!(recognizer
                .skipped
                .iter()
                .any(|skip| skip.role == "nested_fn_role"));
        });
    }

    #[test]
    fn dataflow_predicates_reject_string_and_member_bags() {
        let mut facts = FnFacts::new(Span::new(0, 1), NodeId::DUMMY, None, None, true);
        facts.owned_strings.insert("AES-GCM".to_string());
        facts.owned_members.insert("generateKey".to_string());
        facts.owned_members.insert("exportKey".to_string());
        assert!(!Recognizer::is_aes_encryptor(&facts));
        facts.owned_calls.insert("generateKey".to_string());
        facts.owned_calls.insert("exportKey".to_string());
        facts.owned_calls.insert("encrypt".to_string());
        facts.owned_calls.insert("encode".to_string());
        assert!(Recognizer::is_aes_encryptor(&facts));

        let mut dom = FnFacts::new(Span::new(0, 1), NodeId::DUMMY, None, None, true);
        dom.owned_members.insert("querySelectorAll".to_string());
        dom.owned_refs.insert(7);
        assert!(!Recognizer::is_dom_collector(&dom, 7));
        dom.direct_calls.insert("querySelectorAll".to_string());
        dom.direct_query_field_writes = 1;
        assert!(!Recognizer::is_dom_collector(&dom, 7));
        dom.direct_query_field_writes = 2;
        assert!(!Recognizer::is_dom_collector(&dom, 7));
        dom.owned_written_keys.insert("field".to_string());
        assert!(Recognizer::is_dom_collector(&dom, 7));

        let mut factory = FnFacts::new(Span::new(0, 10), NodeId::DUMMY, None, None, true);
        factory.direct_members.insert("pow".to_string());
        factory.returns_nested_function = true;
        assert!(!Recognizer::is_arx_cipher_factory(&factory));
        factory.nested_mixers = 1;
        assert!(Recognizer::is_arx_cipher_factory(&factory));
        // Reading `arguments` is taking an input, whatever the formal count.
        factory.uses_arguments = true;
        assert!(!Recognizer::is_arx_cipher_factory(&factory));
    }

    #[test]
    fn a_data_claim_on_an_alias_names_the_value_holder() {
        let source = "(function main() {\n  var holder = [1, 2];\n  var alias = holder;\n  var other = 3;\n  function entry() { return alias[0] + other; }\n  return { init: entry };\n})();";
        with_recognizer_for(source, |recognizer, scoping| {
            let alias = symbol_named(scoping, "alias");
            assert!(recognizer.claim_main_data(
                "data_role",
                alias.index() as u32,
                "dataRole",
                Confidence::Medium,
                "test",
            ));
            let by_role: BTreeMap<&str, &str> = recognizer
                .candidates
                .iter()
                .map(|candidate| (candidate.role.as_str(), candidate.original_name.as_str()))
                .collect();
            assert_eq!(by_role.get("data_role"), Some(&"holder"));
            assert_eq!(by_role.get("data_role_alias"), Some(&"alias"));
        });
    }
}
