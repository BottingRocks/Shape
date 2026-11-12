//! Source-derived program wiring for a recovered capture.
//!
//! [`extract_program_wiring`] is the lane-C wiring pass. It reuses the Oxc parse,
//! semantic scope table and structural registry recognition in
//! [`super::scanner`], then resolves the *call graph that actually connects a
//! descriptor/row table to a producer callback*:
//!
//! * the descriptor registry entries and the population writes that extend or
//!   replace them,
//! * the group rows, routing selectors and the group-body functions,
//! * the direct-constructor routes, including the producer callback passed to a
//!   constructor wrapper (never the wrapper factory itself),
//! * the six-argument ABI used for group bodies and the four registration
//!   callbacks each body receives, with the coordinator's stored-init, routing,
//!   last-chance and cleanup behaviour proven from its own body,
//! * the local emitter aliases that map `emitters[k]` to the global signal in
//!   row position `k`, with distinct `next` / `error` / `complete` sites,
//! * the observer factory's latest-value cell, status cell, notifier and
//!   completion scratch roles, and
//! * the public-init -> session-constructor -> transaction-token/state chain
//!   when the source decides it.
//!
//! Every public span is a byte pair in the exact input source. Names are kept
//! only as evidence labels; a route is never recognized by a generated name, a
//! slot number, a group count or a source offset. Ambiguous structures fail
//! closed via [`WiringError::Ambiguous`], and facts the source does not decide
//! remain in [`WiringProperties::unresolved`] instead of being invented.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_semantic::SymbolId;
use oxc_span::{GetSpan, Span};
use serde::{Deserialize, Serialize};

use super::bindings::{
    binding_pattern_symbol, call_arguments, formal_parameter_slot, function_formal_symbols,
    new_arguments, FunctionSite, Model,
};
use super::scanner::resolve_descriptor_ctor;
use crate::signals::extract::layout::{CollectorRequest, ObserverSlot, ObserverWiring};
use crate::signals::schema::{
    BindingRef, BindingRole, ExprRecord, FunctionRef,
    LiteralValue, ProgramProperties, ProgramPropertiesError, ProgramPropertiesLimits,
    SourceIdentity, SourceSpan, ValueKind,
};

/// Version of the serialized [`WiringProperties`] contract.
pub const PROGRAM_WIRING_SCHEMA_VERSION: u32 = 1;

/// A refusal, never a partial guess.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WiringError {
    InvalidInput(String),
    Parse(String),
    Limit {
        what: &'static str,
        limit: usize,
    },
    Ambiguous {
        what: &'static str,
        candidates: Vec<String>,
    },
    Missing {
        what: &'static str,
    },
}

impl std::fmt::Display for WiringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(value) => write!(f, "invalid program-wiring input: {value}"),
            Self::Parse(value) => write!(f, "program-wiring parse failed: {value}"),
            Self::Limit { what, limit } => {
                write!(f, "program-wiring limit exceeded for {what} ({limit})")
            }
            Self::Ambiguous { what, candidates } => write!(
                f,
                "program-wiring ambiguity for {what}: {} matching candidates",
                candidates.len()
            ),
            Self::Missing { what } => {
                write!(f, "program-wiring required fact is missing: {what}")
            }
        }
    }
}

impl std::error::Error for WiringError {}

impl From<ProgramPropertiesError> for WiringError {
    fn from(value: ProgramPropertiesError) -> Self {
        match value {
            ProgramPropertiesError::InvalidInput(message) => Self::InvalidInput(message),
            ProgramPropertiesError::Parse(message) => Self::Parse(message),
            ProgramPropertiesError::Limit { what, limit } => Self::Limit { what, limit },
            ProgramPropertiesError::Ambiguous { what, candidates } => {
                Self::Ambiguous { what, candidates }
            }
            ProgramPropertiesError::Missing { what } => Self::Missing { what },
        }
    }
}

// ===========================================================================
// Output surface
// ===========================================================================

/// An exact source anchor plus the source text it names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WiringEvidence {
    pub span: SourceSpan,
    pub source: String,
    pub detail: String,
}

/// A fact this pass declined to guess.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WiringUnresolved {
    pub fact: String,
    pub reason: String,
    pub span: Option<SourceSpan>,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WiringDiagnostic {
    pub code: String,
    pub message: String,
    pub span: Option<SourceSpan>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DescriptorPopulationKind {
    ArrayLiteral,
    IndexOverride,
}

/// The role of one positional argument in a producer callback ABI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleRole {
    Emitters,
    Context,
    Observer,
    Init,
    Collection,
    LastChance,
    Cleanup,
    Unknown,
}

/// One positional argument of the producer callback invocation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CallbackArgumentWiring {
    pub index: usize,
    pub role: LifecycleRole,
    pub span: SourceSpan,
    pub source: String,
    /// The callback's own formal parameter at this position, when it declares one.
    pub parameter: Option<BindingRef>,
    /// False when the caller passes an argument the body does not name.
    pub bound: bool,
}

/// The call that invokes a producer callback (group body or direct probe arrow).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CallbackInvocationWiring {
    pub span: SourceSpan,
    pub callee: String,
    pub argument_count: usize,
    pub arguments: Vec<CallbackArgumentWiring>,
}

/// A callback registration made by a producer body through one of its formal
/// registrar parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RegisteredCallbackWiring {
    pub role: LifecycleRole,
    pub registrar_index: usize,
    pub registrar_parameter: Option<BindingRef>,
    pub call_span: SourceSpan,
    pub callback: Option<FunctionRef>,
    pub callback_source: Option<String>,
    pub argument_count: usize,
    pub evidence: WiringEvidence,
}

/// The lifecycle registrations proven from a producer body.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LifecycleWiring {
    pub registrations: Vec<RegisteredCallbackWiring>,
    pub body_emissions: Vec<EmitterCallWiring>,
    pub evidence: Vec<WiringEvidence>,
}

/// A method call on a producer emitter/observer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmitterMethod {
    Next,
    Error,
    Complete,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EmitterCallWiring {
    pub method: EmitterMethod,
    pub span: SourceSpan,
    pub receiver_source: String,
    pub payload_source: Option<String>,
    pub nested_callback: bool,
}

/// One local emitter position and the global slot/route it resolves to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalEmitterWiring {
    pub local_position: u32,
    pub global_signal: Option<u32>,
    pub aliases: Vec<BindingRef>,
    pub calls: Vec<EmitterCallWiring>,
    pub evidence: Vec<WiringEvidence>,
}

/// A descriptor entry with the constructor expression and, for a direct route,
/// the resolved wrapper/callback pair.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConstructorWiring {
    /// Exact source expression stored under the descriptor's constructor member.
    pub expression: ExprRecord,
    /// The factory/wrapper actually called in that expression, when it is a call
    /// with a producer callback argument.
    pub wrapper: Option<FunctionRef>,
    /// The producer callback passed to the wrapper. This is the function whose
    /// body owns the observer emissions, not the wrapper itself.
    pub callback: Option<FunctionRef>,
    pub call_span: Option<SourceSpan>,
    /// Lifecycle-spec hooks the producer callback hands an observer to; their
    /// bodies own the emissions (see `scanner::resolve_spec_adapter`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hooks: Vec<SpecHookWiring>,
}

/// A lifecycle-spec hook and the producer callback's call that hands it an
/// observer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpecHookWiring {
    pub function: FunctionRef,
    pub call: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DescriptorEntryWiring {
    pub index: usize,
    pub population: DescriptorPopulationKind,
    pub span: SourceSpan,
    pub source: String,
    pub ctor: Option<ConstructorWiring>,
    pub name: Option<LiteralValue>,
    pub priority: Option<LiteralValue>,
    pub unresolved: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DescriptorPopulationWiring {
    pub index: usize,
    pub assignment_span: SourceSpan,
    pub value_span: SourceSpan,
    pub value_source: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DescriptorRegistryWiring {
    pub binding: BindingRef,
    pub array_literal_span: Option<SourceSpan>,
    pub length: usize,
    pub entries: Vec<DescriptorEntryWiring>,
    pub population_writes: Vec<DescriptorPopulationWiring>,
}

/// One group body route.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupRouteWiring {
    pub group_index: usize,
    pub function: FunctionRef,
    pub row: Vec<u32>,
    pub routing_selector: bool,
    pub row_span: SourceSpan,
    pub selector_span: SourceSpan,
    pub invocation: Option<CallbackInvocationWiring>,
    pub lifecycle: LifecycleWiring,
    pub local_emitters: Vec<LocalEmitterWiring>,
    pub evidence: Vec<WiringEvidence>,
}

/// One direct-constructor route.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DirectRouteWiring {
    pub signal: u32,
    pub descriptor_index: u32,
    pub ctor: ConstructorWiring,
    pub invocation: Option<CallbackInvocationWiring>,
    pub lifecycle: LifecycleWiring,
    pub local_emitters: Vec<LocalEmitterWiring>,
    pub evidence: Vec<WiringEvidence>,
    pub unresolved: Vec<String>,
}

/// Proven callback-first, slot-second notifier registration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NotifierWiring {
    pub registrar: FunctionRef,
    pub table: Option<BindingRef>,
    pub callback_first: bool,
    pub slot_second: bool,
    pub evidence: Vec<WiringEvidence>,
}

/// Coordinator lifecycle facts proven from the coordinator body itself.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoordinatorWiring {
    pub function: FunctionRef,
    pub body_parameter: Option<BindingRef>,
    pub invocation: Option<CallbackInvocationWiring>,
    pub init_stored: Option<BindingRef>,
    pub emit_stored: Option<BindingRef>,
    pub lastchance_stored: Option<BindingRef>,
    pub cleanup_stored: Option<BindingRef>,
    pub init_invoked_after_body: bool,
    pub routing_after_init: bool,
    pub lastchance_registered_after_body: bool,
    pub cleanup_invoked_on_unsubscribe: bool,
    pub cleanup_resets_init_and_lastchance: bool,
    pub noop: Option<BindingRef>,
    pub evidence: Vec<WiringEvidence>,
    pub unresolved: Vec<String>,
}

/// Observer/store roles proven from source.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StoreRolesWiring {
    pub observer_factory: Option<FunctionRef>,
    pub latest_cells: Option<BindingRef>,
    pub status_cells: Option<BindingRef>,
    pub notifier: Option<NotifierWiring>,
    pub snapshot: Option<FunctionRef>,
    pub prefix_scratch: Option<BindingRef>,
    pub evidence: Vec<WiringEvidence>,
    pub unresolved: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicArgumentProvenance {
    /// The constructor argument is a formal parameter of the public init function.
    PublicFormal,
    /// The constructor argument is an exchange/bridge value captured by the
    /// public init closure, not one of its formal parameters.
    CapturedBinding,
    /// A source value the pass can reproduce but cannot map to a public formal.
    SourceExpression,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PublicInitArgumentWiring {
    pub constructor_argument: usize,
    pub public_init_argument: Option<usize>,
    pub provenance: PublicArgumentProvenance,
    pub span: SourceSpan,
    pub source: String,
    pub binding: Option<BindingRef>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TokenProvenanceWiring {
    pub transaction_token_field: Option<String>,
    pub state_binding: Option<BindingRef>,
    pub hash_function: Option<FunctionRef>,
    pub input_source: String,
    pub constructor_argument: Option<usize>,
    pub public_init_argument: Option<usize>,
    pub evidence: Vec<WiringEvidence>,
    pub unresolved: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PublicInitWiring {
    pub function: FunctionRef,
    pub constructor_call_span: SourceSpan,
    pub arguments: Vec<PublicInitArgumentWiring>,
    pub token: Option<TokenProvenanceWiring>,
}

/// A serde-friendly collector request. Convert it to the signal-layout request
/// with [`WiringProperties::signal_collector_requests`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CollectorRequestWiring {
    pub function: SourceSpan,
    pub function_source: String,
    pub observers: CollectorObserversWiring,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CollectorObserversWiring {
    WaveSlots(Vec<CollectorSlotWiring>),
    Direct { signal: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectorSlotWiring {
    pub slot: u32,
    pub signal: u32,
}

/// The complete lane-C wiring report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WiringProperties {
    pub schema_version: u32,
    pub source: SourceIdentity,
    pub main: FunctionRef,
    pub main_argument: Option<String>,
    pub descriptors: Option<DescriptorRegistryWiring>,
    pub groups: Vec<GroupRouteWiring>,
    pub direct_routes: Vec<DirectRouteWiring>,
    pub coordinator: Option<CoordinatorWiring>,
    pub stores: StoreRolesWiring,
    pub public_init: Option<PublicInitWiring>,
    pub collector_requests: Vec<CollectorRequestWiring>,
    pub unresolved: Vec<WiringUnresolved>,
    pub diagnostics: Vec<WiringDiagnostic>,
}

impl WiringProperties {
    /// Build the requests consumed by [`super::signals::extract_signal_layouts`].
    pub fn signal_collector_requests(&self) -> Vec<CollectorRequest> {
        self.collector_requests
            .iter()
            .map(|request| CollectorRequest {
                function: Span::new(request.function.start, request.function.end),
                site: crate::signals::extract::layout::EmissionSite::ObserverNext,
                forwarded_parameter: None,
                observers: match &request.observers {
                    CollectorObserversWiring::WaveSlots(slots) => ObserverWiring::WaveSlots(
                        slots
                            .iter()
                            .map(|slot| ObserverSlot {
                                slot: slot.slot,
                                signal: slot.signal,
                            })
                            .collect(),
                    ),
                    CollectorObserversWiring::Direct { signal } => {
                        ObserverWiring::Direct { signal: *signal }
                    }
                },
            })
            .collect()
    }

    pub fn group(&self, index: usize) -> Option<&GroupRouteWiring> {
        self.groups.iter().find(|group| group.group_index == index)
    }

    pub fn direct(&self, signal: u32) -> Option<&DirectRouteWiring> {
        self.direct_routes
            .iter()
            .find(|route| route.signal == signal)
    }
}

// ===========================================================================
// Entry point
// ===========================================================================

/// Extract the source-derived wiring graph from an exact capture.
pub fn extract_program_wiring(source: &str) -> Result<WiringProperties, WiringError> {
    extract_program_wiring_with_limits(source, ProgramPropertiesLimits::default())
}

/// Extract the wiring graph with explicit deterministic budgets.
fn extract_program_wiring_with_limits(
    source: &str,
    limits: ProgramPropertiesLimits,
) -> Result<WiringProperties, WiringError> {
    wiring_and_properties(source, limits).map(|(wiring, _)| wiring)
}

/// [`extract_program_wiring`] plus the core property report it was built
/// from (the same report [`super::extract_program_properties`] returns), so a
/// caller that needs both parses and scans the source once.
pub(crate) fn extract_program_wiring_and_properties(
    source: &str,
) -> Result<(WiringProperties, ProgramProperties), WiringError> {
    wiring_and_properties(source, ProgramPropertiesLimits::default())
}

fn wiring_and_properties(
    source: &str,
    limits: ProgramPropertiesLimits,
) -> Result<(WiringProperties, ProgramProperties), WiringError> {
    if source.trim().is_empty() {
        return Err(WiringError::InvalidInput("source is empty".to_string()));
    }
    if source.len() > limits.max_source_bytes {
        return Err(WiringError::Limit {
            what: "source bytes",
            limit: limits.max_source_bytes,
        });
    }
    // One parse and model serve the core property scan and the wiring pass
    // (the model is immutable); errors keep the property scan's precedence.
    let allocator = Allocator::default();
    let parsed = crate::analysis::parse(&allocator, source);
    if let Some(error) = parsed.errors.first() {
        return Err(ProgramPropertiesError::Parse(error.clone()).into());
    }
    if parsed.panicked {
        return Err(ProgramPropertiesError::Parse(
            "Oxc parser panicked on the source".to_string(),
        )
        .into());
    }
    let (scoping, nodes) = crate::analysis::analyze(&parsed.program);
    let t = crate::signals::timing::span("   model");
    let model = Model::new(source, &parsed.program, &scoping, &nodes, limits)?;
    drop(t);
    let t = crate::signals::timing::span("   scanner");
    let properties = super::scanner::properties_from_model(&model, limits)?;
    drop(t);
    let _t = crate::signals::timing::span("   wiring extract");
    if properties.source.bytes != source.len() {
        return Err(WiringError::InvalidInput(
            "core property report source identity does not match the supplied source".to_string(),
        ));
    }
    let kept = properties.clone();
    let wiring = WiringExtractor {
        calls: model
            .nodes()
            .iter()
            .filter_map(|n| match n.kind() {
                AstKind::CallExpression(c) => Some(c),
                _ => None,
            })
            .collect(),
        model,
        properties,
        unresolved: Vec::new(),
        diagnostics: Vec::new(),
    }
    .extract()?;
    Ok((wiring, kept))
}

/// Collector-only wiring using an existing scoped parse and registry report.
/// No store/session, codec, signal grammar or semantic naming pass runs here.
pub(crate) fn collector_wiring<'s, 'a>(
    model: Model<'s, 'a>,
    properties: ProgramProperties,
    timings: &mut BTreeMap<String, f64>,
) -> Result<WiringProperties, WiringError> {
    let extractor = WiringExtractor {
        calls: model
            .nodes()
            .iter()
            .filter_map(|n| match n.kind() {
                AstKind::CallExpression(c) => Some(c),
                _ => None,
            })
            .collect(),
        model,
        properties,
        unresolved: Vec::new(),
        diagnostics: Vec::new(),
    };
    extractor.extract_collectors(timings)
}

struct WiringExtractor<'s, 'a> {
    calls: Vec<&'a CallExpression<'a>>,
    model: Model<'s, 'a>,
    properties: ProgramProperties,
    unresolved: Vec<WiringUnresolved>,
    diagnostics: Vec<WiringDiagnostic>,
}

impl<'s, 'a> WiringExtractor<'s, 'a> {
    fn extract_collectors(
        mut self,
        timings: &mut BTreeMap<String, f64>,
    ) -> Result<WiringProperties, WiringError> {
        let start = std::time::Instant::now();
        let descriptors = self.descriptor_registry();
        let covered = self.covered_signals();
        let coordinator = self.coordinator()?;
        timings.insert(
            "wiring_coordinator".into(),
            start.elapsed().as_secs_f64() * 1000.0,
        );
        let start = std::time::Instant::now();
        let groups = self.groups(&coordinator)?;
        timings.insert(
            "wiring_groups".into(),
            start.elapsed().as_secs_f64() * 1000.0,
        );
        let start = std::time::Instant::now();
        let direct_routes = self.direct_routes(descriptors.as_ref(), &covered)?;
        timings.insert(
            "wiring_direct".into(),
            start.elapsed().as_secs_f64() * 1000.0,
        );
        let start = std::time::Instant::now();
        let collector_requests = self.collector_requests(&groups, &direct_routes);
        timings.insert(
            "wiring_requests".into(),
            start.elapsed().as_secs_f64() * 1000.0,
        );
        Ok(WiringProperties {
            schema_version: PROGRAM_WIRING_SCHEMA_VERSION,
            source: self.properties.source.clone(),
            main: self.properties.program_scope.main.clone(),
            main_argument: None,
            descriptors,
            groups,
            direct_routes,
            coordinator,
            stores: StoreRolesWiring {
                observer_factory: None,
                latest_cells: None,
                status_cells: None,
                notifier: None,
                snapshot: None,
                prefix_scratch: None,
                evidence: Vec::new(),
                unresolved: Vec::new(),
            },
            public_init: None,
            collector_requests,
            unresolved: self.unresolved,
            diagnostics: self.diagnostics,
        })
    }
    fn extract(mut self) -> Result<WiringProperties, WiringError> {
        let descriptors = self.descriptor_registry();
        let covered = self.covered_signals();
        let coordinator = self.coordinator()?;
        let groups = self.groups(&coordinator)?;
        let direct_routes = self.direct_routes(descriptors.as_ref(), &covered)?;
        let stores = self.stores();
        let public_init = self.public_init();
        let collector_requests = self.collector_requests(&groups, &direct_routes);
        let main = self.properties.program_scope.main.clone();

        for fact in self.properties.required_missing_facts() {
            // Keep the core registry/session gaps that decide a route. The
            // per-signal wire/encoding gaps belong to the layout/encoding
            // passes, not to this wiring report; carrying them here would make a
            // complete route set look unresolved for an unrelated reason.
            if fact.fact.starts_with("signal_")
                || fact.fact.starts_with("helper_")
                || fact.fact.starts_with("latest_value")
                || fact.fact.starts_with("status_store")
                || fact.fact.starts_with("default_encoder")
            {
                continue;
            }
            self.unresolved.push(WiringUnresolved {
                fact: fact.fact.clone(),
                reason: fact.reason.clone(),
                span: fact.span,
                required: true,
            });
        }

        Ok(WiringProperties {
            schema_version: PROGRAM_WIRING_SCHEMA_VERSION,
            source: self.properties.source.clone(),
            main,
            main_argument: self.properties.program_scope.main_argument.clone(),
            descriptors,
            groups,
            direct_routes,
            coordinator,
            stores,
            public_init,
            collector_requests,
            unresolved: self.unresolved,
            diagnostics: self.diagnostics,
        })
    }

    fn covered_signals(&self) -> BTreeSet<usize> {
        let mut covered = BTreeSet::new();
        if let Some(waves) = &self.properties.waves {
            for wave in &waves.waves {
                covered.extend(wave.signal_indices.iter().copied());
            }
        }
        covered
    }

    fn evidence(&self, span: Span, detail: impl Into<String>) -> WiringEvidence {
        WiringEvidence {
            span: SourceSpan::from(span),
            source: self.model.exact(span).to_string(),
            detail: detail.into(),
        }
    }

    fn push_unresolved(
        &mut self,
        fact: impl Into<String>,
        reason: impl Into<String>,
        span: Option<Span>,
    ) {
        self.unresolved.push(WiringUnresolved {
            fact: fact.into(),
            reason: reason.into(),
            span: span.map(SourceSpan::from),
            required: true,
        });
    }
}

// ===========================================================================
// Descriptors
// ===========================================================================

impl<'s, 'a> WiringExtractor<'s, 'a> {
    fn descriptor_registry(&mut self) -> Option<DescriptorRegistryWiring> {
        let Some(registry) = self.properties.descriptors.as_ref() else {
            self.push_unresolved(
                "descriptor_registry",
                "the source did not expose a descriptor registry with a constructor member",
                None,
            );
            return None;
        };
        let mut entries = Vec::with_capacity(registry.elements.len());
        for element in &registry.elements {
            let ctor = element.ctor.as_ref().map(|expression| {
                let mut unresolved = Vec::new();
                // An inline `wrapper(task)` call has neither a binding nor a
                // function of its own; the shared resolver below decides it.
                if self.constructor_is_nullish(expression) {
                    unresolved.push("null constructor (not a route)".to_string());
                }
                let (wired, refusals) = self.constructor_wiring(expression);
                if !self.constructor_is_nullish(expression) {
                    unresolved.extend(refusals);
                }
                if wired.call_span.is_some() && wired.callback.is_none() {
                    unresolved.push(
                        "constructor wrapper call has no source-resolved producer callback"
                            .to_string(),
                    );
                }
                if wired.wrapper.is_none() && wired.callback.is_none() {
                    unresolved.push(
                        "constructor expression does not resolve to a wrapper or function"
                            .to_string(),
                    );
                }
                (wired, unresolved)
            });
            let (ctor, mut unresolved) = match ctor {
                Some((wired, unresolved)) => (Some(wired), unresolved),
                None => (None, Vec::new()),
            };
            entries.push(DescriptorEntryWiring {
                index: element.index,
                population: match element.population {
                    crate::signals::schema::DescriptorPopulation::ArrayLiteral => {
                        DescriptorPopulationKind::ArrayLiteral
                    }
                    crate::signals::schema::DescriptorPopulation::IndexOverride => {
                        DescriptorPopulationKind::IndexOverride
                    }
                },
                span: element.span,
                source: element.source.clone(),
                ctor,
                name: element.name.clone(),
                priority: element.priority.clone(),
                unresolved: std::mem::take(&mut unresolved),
            });
        }
        Some(DescriptorRegistryWiring {
            binding: registry.binding.clone(),
            array_literal_span: registry.array_literal_span,
            length: registry.length,
            entries,
            population_writes: registry
                .population_assignments
                .iter()
                .map(|write| DescriptorPopulationWiring {
                    index: write.index,
                    assignment_span: write.assignment_span,
                    value_span: write.value_span,
                    value_source: write.value_source.clone(),
                })
                .collect(),
        })
    }

    fn constructor_is_nullish(&self, expression: &ExprRecord) -> bool {
        matches!(
            expression.literal,
            Some(LiteralValue::Null) | Some(LiteralValue::Undefined)
        ) || (expression.kind == ValueKind::Literal
            && matches!(expression.source.trim(), "null" | "undefined" | "void 0"))
    }

    fn constructor_wiring(&self, expression: &ExprRecord) -> (ConstructorWiring, Vec<String>) {
        let (wrapper, callback, call_span, hooks, refusals) = self.resolve_constructor(expression);
        (
            ConstructorWiring {
                expression: expression.clone(),
                wrapper,
                callback,
                call_span,
                hooks,
            },
            refusals,
        )
    }

    /// Resolve a descriptor constructor member through the scanner's shared
    /// direct-collector identity: `ctor` expression -> constructor wrapper ->
    /// last function argument (the task callback). Both the inline
    /// `{ctor: wrapper(task)}` form and a binding that holds the wrapper call
    /// resolve here; every refusal is kept as an unresolved reason.
    fn resolve_constructor(
        &self,
        expression: &ExprRecord,
    ) -> (
        Option<FunctionRef>,
        Option<FunctionRef>,
        Option<SourceSpan>,
        Vec<SpecHookWiring>,
        Vec<String>,
    ) {
        let resolution = resolve_descriptor_ctor(&self.model, expression);
        let wrapper = resolution
            .wrapper
            .map(|site| self.model.function_ref(site))
            .or_else(|| {
                resolution
                    .call_span
                    .is_none()
                    .then(|| expression.function.clone())
                    .flatten()
            });
        (
            wrapper,
            resolution
                .callback
                .map(|site| self.model.function_ref(site)),
            resolution.call_span.map(SourceSpan::from),
            resolution
                .hooks
                .iter()
                .map(|(site, call)| SpecHookWiring {
                    function: self.model.function_ref(*site),
                    call: SourceSpan::from(*call),
                })
                .collect(),
            resolution.refusals,
        )
    }
}

// ===========================================================================
// Callback ABI analysis
// ===========================================================================

#[derive(Clone, Debug)]
struct InvocationTemplate {
    call_span: Span,
    callee_source: String,
    arguments: Vec<InvocationArgument>,
    /// Index of the callback parameter in the function that owns the callee.
    callback_parameter: usize,
    /// The function/arrow node whose formal parameter names the callback.
    callback_owner_span: Span,
}

#[derive(Clone, Debug)]
struct InvocationArgument {
    index: usize,
    role: LifecycleRole,
    span: Span,
    source: String,
    /// The function the argument denotes: an inline function/arrow or a
    /// binding of a function declaration.
    function: Option<Span>,
}

impl<'s, 'a> WiringExtractor<'s, 'a> {
    fn formal_symbols(&self, site: FunctionSite) -> Vec<Option<SymbolId>> {
        function_formal_symbols(self.model.nodes(), site.node_id)
    }

    /// Resolve a producer-body expression to an argument position of `site`
    /// through the shared parameter-slot resolver ([`Model::parameter_slot`]):
    /// a named formal or the literal `arguments[k]` form. An `arguments[k]`
    /// inside a nested ordinary function belongs to that function's own
    /// `arguments` and is rejected; an arrow, which inherits its enclosing
    /// ordinary function's `arguments`, is accepted. A local alias of a formal
    /// is followed by [`Self::formal_origin`].
    fn expression_argument_slot(
        &self,
        expression: &Expression<'a>,
        site: FunctionSite,
    ) -> Option<usize> {
        if let Some(slot) = self.model.site_parameter_slot(expression, site) {
            return Some(slot as usize);
        }
        let Expression::Identifier(identifier) = expression else {
            return None;
        };
        let symbol = crate::analysis::identifier_symbol(identifier, self.model.scoping())?;
        let (owner, index) = self.formal_origin(symbol, 0)?;
        (owner.span.start == site.span.start).then_some(index)
    }

    /// Like [`Self::expression_argument_slot`], but also accepts a local of
    /// `site` that only ever holds that slot (`$k = arguments[2]`).
    fn expression_slot_value(
        &self,
        expression: &Expression<'a>,
        site: FunctionSite,
    ) -> Option<usize> {
        self.expression_argument_slot(expression, site).or_else(|| {
            self.model
                .site_slot_value(expression, site)
                .map(|slot| slot as usize)
        })
    }

    /// The function and argument position an expression denotes, wherever
    /// the owner is: a formal (or an alias of one), `arguments[k]`, or a local
    /// that only ever holds one slot of the function that declares it.
    fn slot_origin(&self, expression: &Expression<'a>) -> Option<(FunctionSite, usize)> {
        if let Some(slot) = self.model.parameter_slot(expression) {
            let span = self.model.nodes().get_node(slot.owner).span();
            return self
                .model
                .site_by_span(span)
                .map(|site| (site, slot.index as usize));
        }
        let Expression::Identifier(identifier) = expression.without_parentheses() else {
            return None;
        };
        let symbol = crate::analysis::identifier_symbol(identifier, self.model.scoping())?;
        if let Some(found) = self.formal_origin(symbol, 0) {
            return Some(found);
        }
        let owner = self
            .model
            .owning_site(self.model.scoping().symbol_declaration(symbol))?;
        let slot = self.model.site_slot_value(expression, owner)?;
        Some((owner, slot as usize))
    }

    /// The function declaration an identifier names, when the binding is the
    /// declaration itself and nothing writes it.
    fn declared_function(&self, expression: &Expression<'a>) -> Option<FunctionSite> {
        let Expression::Identifier(identifier) = expression.without_parentheses() else {
            return None;
        };
        let scoping = self.model.scoping();
        let symbol = crate::analysis::identifier_symbol(identifier, scoping)?;
        if !scoping.symbol_redeclarations(symbol).is_empty()
            || scoping
                .get_resolved_references(symbol)
                .any(|reference| reference.is_write())
        {
            return None;
        }
        match self.model.nodes().kind(scoping.symbol_declaration(symbol)) {
            AstKind::Function(function) => self.model.site_by_span(function.span),
            _ => None,
        }
    }

    fn formal_origin(&self, symbol: SymbolId, depth: usize) -> Option<(FunctionSite, usize)> {
        if depth > 8 {
            return None;
        }
        // A formal that the shared resolver refuses (written, defaulted,
        // destructured, or exposed through `arguments`) proves no position.
        if formal_parameter_slot(self.model.nodes(), self.model.scoping(), symbol).is_some() {
            let slot = self.model.formal_symbol_slot(symbol)?;
            let span = self.model.nodes().get_node(slot.owner).span();
            return self
                .model
                .site_by_span(span)
                .map(|site| (site, slot.index as usize));
        }
        for value in [
            self.model.initializer(symbol),
            self.model.symbol_assigned_value(symbol),
        ]
        .into_iter()
        .flatten()
        {
            let Expression::Identifier(identifier) = value else {
                continue;
            };
            let alias = crate::analysis::identifier_symbol(identifier, self.model.scoping())?;
            if alias == symbol {
                continue;
            }
            if let Some(found) = self.formal_origin(alias, depth + 1) {
                return Some(found);
            }
        }
        None
    }

    fn invocation_template(&self, site: FunctionSite) -> Option<InvocationTemplate> {
        let body = site.body_span?;
        let mut matches = Vec::new();
        for call in &self.calls {
            let span = call.span;
            if span.start < body.start || span.end > body.end {
                continue;
            }
            let (callee, arguments) = invoked_callee(call, self.model.scoping());
            let Some((owner, callback_parameter)) = self.slot_origin(callee) else {
                continue;
            };
            if arguments.len() < 5 {
                continue;
            }
            let roles = callback_roles(arguments.len());
            let trailing = arguments.len().saturating_sub(4);
            let mut resolved = Vec::with_capacity(arguments.len());
            let mut all_trailing_are_functions = true;
            for (index, argument) in arguments.iter().enumerate() {
                if index >= trailing
                    && self.model.function_from_expression(argument).is_none()
                    && !matches!(
                        argument,
                        Expression::Identifier(identifier)
                            if crate::analysis::identifier_symbol(
                                identifier,
                                self.model.scoping()
                            )
                            .and_then(|symbol| self.model.function_from_symbol(symbol))
                            .is_some()
                    )
                {
                    all_trailing_are_functions = false;
                    break;
                }
                resolved.push(InvocationArgument {
                    index,
                    role: roles.get(index).copied().unwrap_or(LifecycleRole::Unknown),
                    span: argument.span(),
                    source: self.model.exact(argument.span()).to_string(),
                    function: self
                        .model
                        .function_from_expression(argument)
                        .map(|site| site.span),
                });
            }
            if !all_trailing_are_functions {
                continue;
            }
            matches.push(InvocationTemplate {
                call_span: call.span,
                callee_source: self.model.exact(callee.span()).to_string(),
                arguments: resolved,
                callback_parameter,
                callback_owner_span: self.model.nodes().get_node(owner.node_id).span(),
            });
        }
        matches.sort_by_key(|template| {
            (
                std::cmp::Reverse(template.arguments.len()),
                template.call_span.start,
            )
        });
        if matches.len() > 1 {
            // More than one callback invocation shape in the same function is
            // not decided by the source order; keep the widest one only when the
            // others are strictly smaller and clearly registrar calls.
            let widest = matches[0].arguments.len();
            let competing = matches
                .iter()
                .filter(|template| template.arguments.len() == widest)
                .count();
            if competing > 1 {
                return None;
            }
        }
        matches.into_iter().next()
    }

    fn invocation_wiring(
        &self,
        template: &InvocationTemplate,
        callback_formals: &[Option<SymbolId>],
    ) -> CallbackInvocationWiring {
        CallbackInvocationWiring {
            span: SourceSpan::from(template.call_span),
            callee: template.callee_source.clone(),
            argument_count: template.arguments.len(),
            arguments: template
                .arguments
                .iter()
                .map(|argument| {
                    let parameter = callback_formals
                        .get(argument.index)
                        .copied()
                        .flatten()
                        .map(|symbol| self.model.binding_ref(symbol));
                    CallbackArgumentWiring {
                        index: argument.index,
                        role: argument.role,
                        span: SourceSpan::from(argument.span),
                        source: argument.source.clone(),
                        bound: parameter.is_some(),
                        parameter,
                    }
                })
                .collect(),
        }
    }

    fn coordinator(&mut self) -> Result<Option<CoordinatorWiring>, WiringError> {
        let mut candidates = Vec::new();
        for site in self.model.all_sites() {
            let Some(template) = self.invocation_template(*site) else {
                continue;
            };
            if template.callback_owner_span.start != site.span.start {
                continue;
            }
            // The observer-handle list is argument slot 0, read as a named
            // formal or as `arguments[0]`.
            if !self.loops_over_slot(0, *site) || !self.returns_function(*site) {
                continue;
            }
            candidates.push((*site, template));
        }
        if candidates.len() > 1 {
            return Err(WiringError::Ambiguous {
                what: "group coordinator (callback invoker with an observer loop and unsubscribe return)",
                candidates: candidates
                    .iter()
                    .map(|(site, _)| format!("{}..{}", site.span.start, site.span.end))
                    .collect(),
            });
        }
        let Some((site, template)) = candidates.into_iter().next() else {
            self.push_unresolved(
                "group_coordinator",
                "no function body invokes a producer callback with a four-registrar ABI after an observer-handle loop",
                None,
            );
            return Ok(None);
        };
        Ok(Some(self.analyze_coordinator(site, &template)))
    }

    /// True when a loop of `site` reads a member of `site`'s argument `slot`
    /// (`list.length`, `list[i]`, `arguments[0]["length"]`).
    fn loops_over_slot(&self, slot: usize, site: FunctionSite) -> bool {
        self.model.nodes_within(site).into_iter().any(|node| {
            if !self.model.node_within(node.id(), site) {
                return false;
            }
            let is_loop = matches!(
                node.kind(),
                AstKind::ForStatement(_)
                    | AstKind::ForInStatement(_)
                    | AstKind::ForOfStatement(_)
                    | AstKind::WhileStatement(_)
                    | AstKind::DoWhileStatement(_)
            );
            if !is_loop {
                return false;
            }
            let span = node.span();
            self.model.nodes().iter().any(|inner| {
                let object = match inner.kind() {
                    AstKind::StaticMemberExpression(member) => &member.object,
                    AstKind::ComputedMemberExpression(member) => &member.object,
                    _ => return false,
                };
                let inner_span = inner.span();
                inner_span.start >= span.start
                    && inner_span.end <= span.end
                    && self.expression_slot_value(object, site) == Some(slot)
            })
        })
    }

    fn returns_function(&self, site: FunctionSite) -> bool {
        self.model.nodes_within(site).into_iter().any(|node| {
            if !self.model.node_within(node.id(), site) {
                return false;
            }
            let AstKind::ReturnStatement(ret) = node.kind() else {
                return false;
            };
            if self
                .model
                .enclosing_function(node.id())
                .map(|owner| owner.span.start)
                != Some(site.span.start)
            {
                return false;
            }
            ret.argument
                .as_ref()
                .map(|argument| self.model.function_from_expression(argument).is_some())
                .unwrap_or(false)
        })
    }

    fn analyze_coordinator(
        &mut self,
        site: FunctionSite,
        template: &InvocationTemplate,
    ) -> CoordinatorWiring {
        let formals = self.formal_symbols(site);
        let body_parameter = formals
            .get(template.callback_parameter)
            .copied()
            .flatten()
            .map(|symbol| self.model.binding_ref(symbol));
        let argument = |role: LifecycleRole| {
            template
                .arguments
                .iter()
                .find(|argument| argument.role == role)
        };
        // Stored callbacks are read from the registrar function bodies. The
        // registrar function is the actual argument expression at the body call,
        // so the alias table below only names the callback the coordinator later
        // invokes, not a generated letter.
        let stored = |role: LifecycleRole| -> Option<BindingRef> {
            let argument = argument(role)?;
            self.stored_callback_binding(argument.function?)
        };
        let init_stored = stored(LifecycleRole::Init);
        let emit_stored = stored(LifecycleRole::Collection);
        let lastchance_stored = stored(LifecycleRole::LastChance);
        let cleanup_stored = stored(LifecycleRole::Cleanup);

        let init_closure = init_stored
            .as_ref()
            .map(|binding| self.alias_closure(site, binding.symbol))
            .unwrap_or_default();
        let emit_closure = emit_stored
            .as_ref()
            .map(|binding| self.alias_closure(site, binding.symbol))
            .unwrap_or_default();
        let lastchance_closure = lastchance_stored
            .as_ref()
            .map(|binding| self.alias_closure(site, binding.symbol))
            .unwrap_or_default();
        let init_invocation =
            self.call_after_body_referencing(site, template.call_span.end, &init_closure, false);
        let routing =
            self.call_after_body_referencing(site, template.call_span.end, &emit_closure, true);
        let lastchance_registration = self.call_after_body_referencing(
            site,
            template.call_span.end,
            &lastchance_closure,
            true,
        );
        let cleanup_invocation = cleanup_stored
            .as_ref()
            .and_then(|binding| self.call_inside_returned_function(site, binding.symbol));
        let noop = self.cleanup_noop(site, &[&init_stored, &lastchance_stored]);
        let init_invoked_after_body = init_invocation.is_some();
        let routing_after_init = match (&init_invocation, &routing) {
            (Some(init), Some(routing)) => routing.start >= init.start,
            _ => false,
        };
        let lastchance_registered_after_body = lastchance_registration.is_some();
        let cleanup_invoked_on_unsubscribe = cleanup_invocation.is_some();
        let cleanup_resets_init_and_lastchance = noop.is_some();

        let mut evidence = vec![
            self.evidence(
                template.call_span,
                "coordinator invokes the stored group body with the proven argument ABI",
            ),
            self.evidence(
                site.span,
                "coordinator body owns the callback registrars and unsubscribe function",
            ),
        ];
        if let Some(span) = init_invocation {
            evidence.push(self.evidence(span, "stored init callback invoked after body returns"));
        }
        if let Some(span) = routing {
            evidence.push(self.evidence(span, "collection callback routed after the init call"));
        }
        if let Some(span) = lastchance_registration {
            evidence.push(self.evidence(
                span,
                "last-chance callback registered through the caller registrar",
            ));
        }
        if let Some(span) = cleanup_invocation {
            evidence.push(self.evidence(
                span,
                "cleanup callback invoked from the returned unsubscribe",
            ));
        }

        let mut unresolved = Vec::new();
        if init_stored.is_none() {
            unresolved.push("coordinator init registrar does not store a callback".to_string());
        }
        if emit_stored.is_none() {
            unresolved.push("coordinator emit registrar does not store a callback".to_string());
        }
        if lastchance_stored.is_none() {
            unresolved
                .push("coordinator last-chance registrar does not store a callback".to_string());
        }
        if cleanup_stored.is_none() {
            unresolved.push("coordinator cleanup registrar does not store a callback".to_string());
        }
        if !init_invoked_after_body {
            unresolved.push(
                "stored init callback invocation was not proven after the body call".to_string(),
            );
        }
        if !routing_after_init {
            unresolved
                .push("collection routing was not proven after the init invocation".to_string());
        }
        if !lastchance_registered_after_body {
            unresolved
                .push("last-chance registration was not proven after the body call".to_string());
        }
        if !cleanup_invoked_on_unsubscribe {
            unresolved.push(
                "cleanup callback invocation was not proven in the returned unsubscribe"
                    .to_string(),
            );
        }
        if !cleanup_resets_init_and_lastchance {
            unresolved.push(
                "unsubscribe no-op substitution was not proven for both init and last-chance"
                    .to_string(),
            );
        }

        CoordinatorWiring {
            function: self.model.function_ref(site),
            body_parameter,
            invocation: Some(self.invocation_wiring(template, &[])),
            init_stored,
            emit_stored,
            lastchance_stored,
            cleanup_stored,
            init_invoked_after_body,
            routing_after_init,
            lastchance_registered_after_body,
            cleanup_invoked_on_unsubscribe,
            cleanup_resets_init_and_lastchance,
            noop,
            evidence,
            unresolved,
        }
    }

    fn alias_closure(&self, site: FunctionSite, symbol: u32) -> BTreeSet<u32> {
        let mut closure = BTreeSet::new();
        closure.insert(symbol);
        for _ in 0..6 {
            let mut changed = false;
            for node in self.model.nodes_within(site).into_iter() {
                if !self.model.node_within(node.id(), site) {
                    continue;
                }
                let (target, value) = match node.kind() {
                    AstKind::VariableDeclarator(declarator) => (
                        binding_pattern_symbol(&declarator.id),
                        declarator.init.as_ref(),
                    ),
                    AstKind::AssignmentExpression(assignment) => {
                        let target = match &assignment.left {
                            AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                                crate::analysis::identifier_symbol(identifier, self.model.scoping())
                            }
                            _ => None,
                        };
                        (target, Some(&assignment.right))
                    }
                    _ => continue,
                };
                let (Some(target), Some(value)) = (target, value) else {
                    continue;
                };
                let Expression::Identifier(identifier) = value else {
                    continue;
                };
                let Some(alias) =
                    crate::analysis::identifier_symbol(identifier, self.model.scoping())
                else {
                    continue;
                };
                if closure.contains(&(alias.index() as u32))
                    && closure.insert(target.index() as u32)
                {
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        closure
    }

    fn span_references_any(&self, span: Span, symbols: &BTreeSet<u32>) -> bool {
        symbols.iter().any(|symbol| {
            let Some(symbol) = self.model.symbol_by_index(*symbol) else {
                return false;
            };
            self.model
                .scoping()
                .get_resolved_reference_ids(symbol)
                .iter()
                .any(|reference| {
                    let node_id = self.model.scoping().get_reference(*reference).node_id();
                    let reference_span = self.model.nodes().get_node(node_id).span();
                    reference_span.start >= span.start && reference_span.end <= span.end
                })
        })
    }

    fn call_after_body_referencing(
        &self,
        site: FunctionSite,
        after: u32,
        symbols: &BTreeSet<u32>,
        require_function_argument: bool,
    ) -> Option<Span> {
        if symbols.is_empty() {
            return None;
        }
        let mut found = Vec::new();
        for node in self.model.nodes_within(site).into_iter() {
            if node.span().start < after || !self.model.node_within(node.id(), site) {
                continue;
            }
            let AstKind::CallExpression(call) = node.kind() else {
                continue;
            };
            if self.span_references_any(call.callee.span(), symbols) {
                found.push(call.span);
                continue;
            }
            for argument in invoked_callee(call, self.model.scoping()).1 {
                let function = self.model.function_from_expression(argument);
                if require_function_argument && function.is_none() {
                    continue;
                }
                // A function declaration passed by name is read through its own
                // body; any other argument is read where it is written.
                let span = self
                    .declared_function(argument)
                    .map_or(argument.span(), |site| site.span);
                if self.span_references_any(span, symbols) {
                    found.push(call.span);
                    break;
                }
            }
        }
        found.sort_by_key(|span| span.start);
        found.into_iter().next()
    }

    /// The binding a registrar function stores its first argument into
    /// (`cb => { stored = cb; }` or `function () { stored = arguments[0]; }`).
    fn stored_callback_binding(&self, registrar_span: Span) -> Option<BindingRef> {
        let registrar = self.model.site_by_span(registrar_span)?;
        let body = registrar.body_span?;
        for node in self.model.nodes().iter() {
            if node.span().start < body.start || node.span().end > body.end {
                continue;
            }
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                continue;
            };
            let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                continue;
            };
            if self.expression_argument_slot(&assignment.right, registrar) != Some(0) {
                continue;
            }
            let target_symbol = crate::analysis::identifier_symbol(target, self.model.scoping())?;
            return Some(self.model.binding_ref(target_symbol));
        }
        None
    }

    fn call_inside_returned_function(&self, site: FunctionSite, symbol: u32) -> Option<Span> {
        self.model.nodes_within(site).into_iter().find_map(|node| {
            if !self.model.node_within(node.id(), site) {
                return None;
            }
            let AstKind::Function(function) = node.kind() else {
                return None;
            };
            if self
                .model
                .enclosing_function(node.id())
                .map(|owner| owner.span.start)
                != Some(site.span.start)
            {
                return None;
            }
            let body = function.body.as_ref()?.span;
            self.model.nodes().iter().find_map(|inner| {
                if inner.span().start < body.start || inner.span().end > body.end {
                    return None;
                }
                let AstKind::CallExpression(call) = inner.kind() else {
                    return None;
                };
                let callee_is_symbol = match &call.callee {
                    Expression::Identifier(identifier) => {
                        crate::analysis::identifier_symbol(identifier, self.model.scoping())
                            .map(|found| found.index() as u32)
                            == Some(symbol)
                    }
                    _ => false,
                };
                (callee_is_symbol
                    || call.arguments.iter().any(|argument| {
                        argument
                            .as_expression()
                            .and_then(|expression| match expression {
                                Expression::Identifier(identifier) => {
                                    crate::analysis::identifier_symbol(
                                        identifier,
                                        self.model.scoping(),
                                    )
                                    .map(|found| found.index() as u32)
                                }
                                _ => None,
                            })
                            == Some(symbol)
                    }))
                .then_some(call.span)
            })
        })
    }

    fn cleanup_noop(
        &self,
        site: FunctionSite,
        stored: &[&Option<BindingRef>],
    ) -> Option<BindingRef> {
        let mut assignments: Vec<(u32, SymbolId, Span)> = Vec::new();
        for node in self.model.nodes_within(site).into_iter() {
            if !self.model.node_within(node.id(), site) {
                continue;
            }
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                continue;
            };
            let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                continue;
            };
            let Some(target_symbol) =
                crate::analysis::identifier_symbol(target, self.model.scoping())
            else {
                continue;
            };
            if !stored.iter().any(|binding| {
                binding.as_ref().map(|binding| binding.symbol) == Some(target_symbol.index() as u32)
            }) {
                continue;
            }
            let Expression::Identifier(value) = &assignment.right else {
                continue;
            };
            let Some(value_symbol) =
                crate::analysis::identifier_symbol(value, self.model.scoping())
            else {
                continue;
            };
            assignments.push((target_symbol.index() as u32, value_symbol, assignment.span));
        }
        let mut by_value: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
        for (target, value, _) in assignments {
            by_value
                .entry(value.index() as u32)
                .or_default()
                .insert(target);
        }
        for (value, targets) in by_value {
            let matched = stored
                .iter()
                .filter_map(|binding| binding.as_ref().map(|binding| binding.symbol))
                .filter(|symbol| targets.contains(symbol))
                .count();
            if matched >= 2 {
                let symbol = self.model.symbol_by_index(value)?;
                return Some(self.model.binding_ref(symbol));
            }
        }
        None
    }
}

/// The callee a call invokes and the arguments it receives. `f.call(undefined,
/// a, b)` (or `void 0`) invokes `f` with `(a, b)` and the same `this` as a
/// plain call; any other `.call` receiver is a different call and is left as
/// written.
fn invoked_callee<'a>(
    call: &'a CallExpression<'a>,
    scoping: &oxc_semantic::Scoping,
) -> (&'a Expression<'a>, Vec<&'a Expression<'a>>) {
    let arguments = call_arguments(call);
    let callee = call.callee.without_parentheses();
    let object = match callee {
        Expression::StaticMemberExpression(member) if member.property.name == "call" => {
            Some(&member.object)
        }
        Expression::ComputedMemberExpression(member) if matches!(&member.expression, Expression::StringLiteral(name) if name.value == "call") => {
            Some(&member.object)
        }
        _ => None,
    };
    let undefined_receiver =
        arguments
            .first()
            .is_some_and(|receiver| match receiver.without_parentheses() {
                Expression::UnaryExpression(unary) => {
                    unary.operator == oxc_syntax::operator::UnaryOperator::Void
                        && matches!(unary.argument, Expression::NumericLiteral(_))
                }
                Expression::Identifier(identifier) => {
                    identifier.name == "undefined"
                        && crate::analysis::identifier_symbol(identifier, scoping).is_none()
                }
                _ => false,
            });
    match object {
        Some(object) if undefined_receiver => (object, arguments[1..].to_vec()),
        _ => (callee, arguments),
    }
}

fn callback_roles(argument_count: usize) -> Vec<LifecycleRole> {
    group_role_map(argument_count)
        .into_iter()
        .map(|(_, role)| role)
        .collect()
}

// ===========================================================================
// Groups, lifecycle registrations and local emitters
// ===========================================================================

impl<'s, 'a> WiringExtractor<'s, 'a> {
    fn groups(
        &mut self,
        coordinator: &Option<CoordinatorWiring>,
    ) -> Result<Vec<GroupRouteWiring>, WiringError> {
        let Some(waves) = self.properties.waves.clone() else {
            self.push_unresolved(
                "wave_registry",
                "the source did not expose a group/function/row/selector registry",
                None,
            );
            return Ok(Vec::new());
        };
        let template = coordinator.as_ref().and_then(|coordinator| {
            coordinator
                .function
                .binding
                .as_ref()
                .and_then(|binding| self.model.symbol_by_index(binding.symbol))
                .and_then(|symbol| self.model.function_from_symbol(symbol))
                .and_then(|site| self.invocation_template(site))
        });
        let mut groups = Vec::with_capacity(waves.waves.len());
        for wave in &waves.waves {
            let site = self
                .model
                .site_by_span(Span::new(wave.function_span.start, wave.function_span.end));
            let Some(site) = site else {
                self.push_unresolved(
                    format!("group_{}_body", wave.index),
                    "the row references a function span that is not a parsed function body",
                    Some(Span::new(wave.function_span.start, wave.function_span.end)),
                );
                continue;
            };
            let formals = self.formal_symbols(site);
            let invocation = template
                .as_ref()
                .map(|template| self.invocation_wiring(template, &formals));
            let row: Vec<u32> = wave
                .signal_indices
                .iter()
                .map(|signal| *signal as u32)
                .collect();
            let roles = group_role_map(
                invocation
                    .as_ref()
                    .map_or(0, |invocation| invocation.argument_count),
            );
            let lifecycle = self.analyze_lifecycle(site, &formals, &roles);
            // The emitter array is argument slot 0, read as a named formal or
            // as `arguments[0]`.
            let local_emitters = self.analyze_emitters(site, 0, Some(&row), None);
            if local_emitters
                .iter()
                .all(|emitter| emitter.aliases.is_empty() && emitter.calls.is_empty())
            {
                self.push_unresolved(
                    format!("group_{}_emitter_parameter", wave.index),
                    "the group body never reads an emitter from its emitter-array argument slot 0",
                    Some(site.span),
                );
            }
            if invocation.is_none() {
                self.push_unresolved(
                    format!("group_{}_invocation", wave.index),
                    "the coordinator body did not prove a six-argument invocation for this group",
                    Some(site.span),
                );
            }
            groups.push(GroupRouteWiring {
                group_index: wave.index,
                function: self.model.function_ref(site),
                row,
                routing_selector: wave.routing_selector,
                row_span: wave.row_span,
                selector_span: wave.selector_span,
                invocation,
                lifecycle,
                local_emitters,
                evidence: vec![
                    self.evidence(
                        Span::new(wave.row_span.start, wave.row_span.end),
                        "group row is the ordered global-slot map for the local emitter positions",
                    ),
                    self.evidence(
                        Span::new(wave.selector_span.start, wave.selector_span.end),
                        "routing selector array entry consumed by the coordinator",
                    ),
                    self.evidence(site.span, "group body function"),
                ],
            });
        }
        Ok(groups)
    }

    fn analyze_lifecycle(
        &self,
        site: FunctionSite,
        formals: &[Option<SymbolId>],
        roles: &BTreeMap<usize, LifecycleRole>,
    ) -> LifecycleWiring {
        let mut registrations = Vec::new();
        let mut evidence = Vec::new();
        for node in self.model.nodes_within(site).into_iter() {
            if !self.model.node_within(node.id(), site) {
                continue;
            }
            let AstKind::CallExpression(call) = node.kind() else {
                continue;
            };
            let (callee, arguments) = invoked_callee(call, self.model.scoping());
            let Some(index) = self.expression_slot_value(callee, site) else {
                continue;
            };
            let Some(role) = roles.get(&index).copied() else {
                continue;
            };
            if !matches!(
                role,
                LifecycleRole::Init
                    | LifecycleRole::Collection
                    | LifecycleRole::LastChance
                    | LifecycleRole::Cleanup
            ) {
                continue;
            }
            let parameter = formals
                .get(index)
                .copied()
                .flatten()
                .map(|symbol| self.model.binding_ref(symbol));
            let Some(callback) = arguments
                .iter()
                .find_map(|argument| self.model.function_from_expression(argument))
            else {
                continue;
            };
            if registrations
                .iter()
                .any(|registered: &RegisteredCallbackWiring| {
                    registered.call_span == SourceSpan::from(call.span)
                })
            {
                continue;
            }
            registrations.push(RegisteredCallbackWiring {
                role,
                registrar_index: index,
                registrar_parameter: parameter,
                call_span: SourceSpan::from(call.span),
                callback_source: Some(self.model.exact(callback.span).to_string()),
                callback: Some(self.model.function_ref(callback)),
                argument_count: arguments.len(),
                evidence: self.evidence(
                    call.span,
                    format!("body registers a {role:?} callback through formal/argument position {index}"),
                ),
            });
        }
        if !registrations.is_empty() {
            evidence.push(self.evidence(
                site.span,
                "registration calls are resolved through the body's own formal parameters or arguments, not source order",
            ));
        }
        LifecycleWiring {
            registrations,
            body_emissions: Vec::new(),
            evidence,
        }
    }

    /// Local emitter records for the emitters `site` receives in argument
    /// `slot`: a wave group's emitter array, or a direct callback's observer.
    /// The slot is matched through the shared parameter-slot resolver, so a
    /// named formal, `arguments[slot]` and a local that only ever holds that
    /// slot are the same root.
    fn analyze_emitters(
        &self,
        site: FunctionSite,
        slot: u32,
        row: Option<&[u32]>,
        direct_signal: Option<u32>,
    ) -> Vec<LocalEmitterWiring> {
        let mut aliases: BTreeMap<SymbolId, u32> = BTreeMap::new();
        for _ in 0..4 {
            for node in self.model.nodes_within(site).into_iter() {
                if !self.model.node_within(node.id(), site) {
                    continue;
                }
                let (target, value) = match node.kind() {
                    AstKind::VariableDeclarator(declarator) => (
                        binding_pattern_symbol(&declarator.id),
                        declarator.init.as_ref(),
                    ),
                    AstKind::AssignmentExpression(assignment) => {
                        let target = match &assignment.left {
                            AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                                crate::analysis::identifier_symbol(identifier, self.model.scoping())
                            }
                            _ => None,
                        };
                        (target, Some(&assignment.right))
                    }
                    _ => continue,
                };
                let (Some(target), Some(value)) = (target, value) else {
                    continue;
                };
                if aliases.contains_key(&target) {
                    continue;
                }
                if let Some(position) = self.emitter_position(value, site, slot, &aliases) {
                    aliases.insert(target, position);
                }
            }
        }

        let mut by_position: BTreeMap<u32, (Vec<BindingRef>, Vec<EmitterCallWiring>)> =
            BTreeMap::new();
        for (symbol, position) in &aliases {
            by_position
                .entry(*position)
                .or_default()
                .0
                .push(self.model.binding_ref(*symbol));
        }
        for node in self.model.nodes_within(site).into_iter() {
            if !self.model.node_within(node.id(), site) {
                continue;
            }
            let AstKind::CallExpression(call) = node.kind() else {
                continue;
            };
            let Some((method, receiver, payload)) = sink_call(call) else {
                continue;
            };
            let Some(position) = self.emitter_position(receiver, site, slot, &aliases) else {
                continue;
            };
            let nested_callback = self
                .model
                .enclosing_function(node.id())
                .map(|owner| owner.span.start != site.span.start)
                .unwrap_or(false);
            by_position
                .entry(position)
                .or_default()
                .1
                .push(EmitterCallWiring {
                    method,
                    span: SourceSpan::from(call.span),
                    receiver_source: self.model.exact(receiver.span()).to_string(),
                    payload_source: payload.map(|value| self.model.exact(value.span()).to_string()),
                    nested_callback,
                });
        }

        let positions: BTreeSet<u32> = match (row, direct_signal) {
            (Some(row), _) => (0..row.len() as u32).collect(),
            (None, Some(_)) => [0].into_iter().collect(),
            (None, None) => by_position.keys().copied().collect(),
        };
        positions
            .into_iter()
            .map(|position| {
                let (aliases, calls) = by_position.remove(&position).unwrap_or_default();
                let global_signal = match (row, direct_signal) {
                    (Some(row), _) => row.get(position as usize).copied(),
                    (None, Some(signal)) => (position == 0).then_some(signal),
                    (None, None) => None,
                };
                let mut evidence = Vec::new();
                if let Some(global) = global_signal {
                    if row.is_some() {
                        evidence.push(self.evidence(
                            site.span,
                            format!(
                                "local emitter position {position} maps through the row to global signal {global}"
                            ),
                        ));
                    }
                }
                LocalEmitterWiring {
                    local_position: position,
                    global_signal,
                    aliases,
                    calls,
                    evidence,
                }
            })
            .collect()
    }

    /// True when `expression` is the emitter root: argument `slot` of `site`.
    fn is_emitter_root(&self, expression: &Expression<'a>, site: FunctionSite, slot: u32) -> bool {
        self.expression_argument_slot(expression, site) == Some(slot as usize)
            || self.model.site_slot_value(expression, site) == Some(slot)
    }

    fn emitter_position(
        &self,
        expression: &Expression<'a>,
        site: FunctionSite,
        slot: u32,
        aliases: &BTreeMap<SymbolId, u32>,
    ) -> Option<u32> {
        // `arguments[slot]` is itself a computed member: the root is tested first.
        if self.is_emitter_root(expression, site, slot) {
            return Some(0);
        }
        match expression {
            Expression::ComputedMemberExpression(member) => {
                if !self.is_emitter_root(&member.object, site, slot) {
                    return None;
                }
                match &member.expression {
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
            Expression::Identifier(identifier) => {
                let symbol = crate::analysis::identifier_symbol(identifier, self.model.scoping())?;
                aliases.get(&symbol).copied()
            }
            _ => None,
        }
    }
}

fn group_role_map(argument_count: usize) -> BTreeMap<usize, LifecycleRole> {
    if argument_count != 6 {
        return (0..argument_count)
            .map(|index| (index, LifecycleRole::Unknown))
            .collect();
    }
    [
        (0, LifecycleRole::Emitters),
        (1, LifecycleRole::Context),
        (2, LifecycleRole::Init),
        (3, LifecycleRole::Collection),
        (4, LifecycleRole::LastChance),
        (5, LifecycleRole::Cleanup),
    ]
    .into_iter()
    .collect()
}

/// Resolve the receiver of a `next` / `error` / `complete` sink call, including
/// the `next.call(recv, payload)` and `next.apply(recv, [payload])` forms.
fn sink_call<'a>(
    call: &'a CallExpression<'a>,
) -> Option<(
    EmitterMethod,
    &'a Expression<'a>,
    Option<&'a Expression<'a>>,
)> {
    let method = |name: &str| match name {
        "next" => Some(EmitterMethod::Next),
        "error" => Some(EmitterMethod::Error),
        "complete" => Some(EmitterMethod::Complete),
        _ => None,
    };
    // `o.next(...)` and `o["next"](...)` are the same member call.
    let (name, object) = named_member(&call.callee)?;
    if let Some(method) = method(name) {
        let payload = call_arguments(call).first().copied();
        return Some((method, object, payload));
    }
    if !matches!(name, "call" | "apply") {
        return None;
    }
    let (inner, _) = named_member(object)?;
    let method = method(inner)?;
    let arguments = call_arguments(call);
    let receiver = arguments.first().copied()?;
    let payload = match name {
        "call" => arguments.get(1).copied(),
        "apply" => arguments.get(1).copied().and_then(|array| {
            let Expression::ArrayExpression(array) = array else {
                return None;
            };
            let mut elements = array.elements.iter();
            let first = elements.next()?.as_expression()?;
            elements.next().is_none().then_some(first)
        }),
        _ => None,
    };
    Some((method, receiver, payload))
}

/// The property name and object of a static member or a computed member with
/// a string-literal key.
fn named_member<'a>(expression: &'a Expression<'a>) -> Option<(&'a str, &'a Expression<'a>)> {
    match expression.without_parentheses() {
        Expression::StaticMemberExpression(member) => {
            Some((member.property.name.as_str(), &member.object))
        }
        Expression::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(name) => Some((name.value.as_str(), &member.object)),
            _ => None,
        },
        _ => None,
    }
}

// ===========================================================================
// Direct constructor routes
// ===========================================================================

impl<'s, 'a> WiringExtractor<'s, 'a> {
    fn direct_routes(
        &mut self,
        descriptors: Option<&DescriptorRegistryWiring>,
        covered: &BTreeSet<usize>,
    ) -> Result<Vec<DirectRouteWiring>, WiringError> {
        let Some(descriptors) = descriptors else {
            return Ok(Vec::new());
        };
        let mut routes = Vec::new();
        for entry in &descriptors.entries {
            if covered.contains(&entry.index) {
                continue;
            }
            let Some(ctor) = entry.ctor.as_ref() else {
                continue;
            };
            if self.constructor_is_nullish(&ctor.expression) {
                continue;
            }
            let callback = ctor.callback.clone().or_else(|| {
                (ctor.call_span.is_none())
                    .then(|| ctor.wrapper.clone())
                    .flatten()
            });
            let mut unresolved = Vec::new();
            if ctor.wrapper.is_none() {
                unresolved
                    .push("direct constructor does not resolve to a wrapper function".to_string());
            }
            if callback.is_none() {
                unresolved.push(
                    "direct constructor has no source-resolved producer callback".to_string(),
                );
            }
            let callback_site = callback.as_ref().and_then(|callback| {
                self.model
                    .site_by_span(Span::new(callback.span.start, callback.span.end))
            });
            let wrapper_site = ctor.wrapper.as_ref().and_then(|wrapper| {
                self.model
                    .site_by_span(Span::new(wrapper.span.start, wrapper.span.end))
            });
            // The wrapper factory never stands in for a missing task callback:
            // its body does not own the emissions.
            let Some(callback_site) = callback_site else {
                self.push_unresolved(
                    format!("direct_{}_callback_body", entry.index),
                    "the direct route's producer callback span is not a parsed function body",
                    Some(Span::new(
                        ctor.expression.span.start,
                        ctor.expression.span.end,
                    )),
                );
                routes.push(DirectRouteWiring {
                    signal: entry.index as u32,
                    descriptor_index: entry.index as u32,
                    ctor: ctor.clone(),
                    invocation: None,
                    lifecycle: LifecycleWiring {
                        registrations: Vec::new(),
                        body_emissions: Vec::new(),
                        evidence: Vec::new(),
                    },
                    local_emitters: Vec::new(),
                    evidence: vec![self.evidence(
                        Span::new(ctor.expression.span.start, ctor.expression.span.end),
                        "direct descriptor constructor expression",
                    )],
                    unresolved,
                });
                continue;
            };
            let callback_formals = self.formal_symbols(callback_site);
            let template = wrapper_site
                .or(Some(callback_site))
                .and_then(|site| self.invocation_template(site));
            let template = template.map(|mut template| {
                let roles = direct_callback_roles(template.arguments.len());
                for argument in &mut template.arguments {
                    argument.role = roles
                        .get(argument.index)
                        .copied()
                        .unwrap_or(LifecycleRole::Unknown);
                }
                template
            });
            let invocation = template
                .as_ref()
                .map(|template| self.invocation_wiring(template, &callback_formals));
            if invocation.is_none() {
                unresolved.push(
                    "no wrapper body invocation of the producer callback with a four-registrar ABI"
                        .to_string(),
                );
            }
            let roles =
                direct_callback_role_map(template.as_ref().map_or(0, |t| t.arguments.len()));
            let mut lifecycle = self.analyze_lifecycle(callback_site, &callback_formals, &roles);
            lifecycle.body_emissions = self.local_calls(&lifecycle);

            let mut local_emitters = Vec::new();
            if let Some((index, _role)) = roles
                .iter()
                .find(|(_, role)| matches!(role, LifecycleRole::Observer | LifecycleRole::Emitters))
            {
                local_emitters = self.analyze_emitters(
                    callback_site,
                    *index as u32,
                    None,
                    Some(entry.index as u32),
                );
            } else if let Some(registration) = lifecycle
                .registrations
                .iter()
                .find(|registration| registration.role == LifecycleRole::Collection)
            {
                if let Some(registration_callback) = registration.callback.as_ref() {
                    if let Some(site) = self.model.site_by_span(Span::new(
                        registration_callback.span.start,
                        registration_callback.span.end,
                    )) {
                        local_emitters =
                            self.analyze_emitters(site, 0, None, Some(entry.index as u32));
                    }
                }
            }
            for hook in &ctor.hooks {
                if let Some(site) = self
                    .model
                    .site_by_span(Span::new(hook.function.span.start, hook.function.span.end))
                {
                    local_emitters.extend(self.analyze_emitters(
                        site,
                        0,
                        None,
                        Some(entry.index as u32),
                    ));
                }
            }
            if local_emitters
                .iter()
                .all(|emitter| emitter.calls.is_empty())
            {
                unresolved.push(
                    "no distinct next/error/complete call was proven on the direct observer path"
                        .to_string(),
                );
            }
            routes.push(DirectRouteWiring {
                signal: entry.index as u32,
                descriptor_index: entry.index as u32,
                ctor: ctor.clone(),
                invocation,
                lifecycle,
                local_emitters,
                evidence: vec![
                    self.evidence(
                        Span::new(ctor.expression.span.start, ctor.expression.span.end),
                        "direct descriptor constructor expression",
                    ),
                    self.evidence(
                        callback_site.span,
                        "producer callback body routed through the constructor wrapper",
                    ),
                ],
                unresolved,
            });
        }
        Ok(routes)
    }

    fn local_calls(&self, lifecycle: &LifecycleWiring) -> Vec<EmitterCallWiring> {
        lifecycle
            .registrations
            .iter()
            .filter_map(|registration| registration.callback.as_ref())
            .flat_map(|callback| {
                self.model
                    .site_by_span(Span::new(callback.span.start, callback.span.end))
                    .map(|site| {
                        self.model
                            .nodes()
                            .iter()
                            .filter_map(|node| {
                                if !self.model.node_within(node.id(), site) {
                                    return None;
                                }
                                let AstKind::CallExpression(call) = node.kind() else {
                                    return None;
                                };
                                let (method, receiver, payload) = sink_call(call)?;
                                Some(EmitterCallWiring {
                                    method,
                                    span: SourceSpan::from(call.span),
                                    receiver_source: self.model.exact(receiver.span()).to_string(),
                                    payload_source: payload
                                        .map(|value| self.model.exact(value.span()).to_string()),
                                    nested_callback: false,
                                })
                            })
                            .collect::<Vec<EmitterCallWiring>>()
                    })
                    .unwrap_or_default()
            })
            .collect()
    }
}

fn direct_callback_roles(argument_count: usize) -> Vec<LifecycleRole> {
    match argument_count {
        0 => Vec::new(),
        1 => vec![LifecycleRole::Context],
        2 => vec![LifecycleRole::Observer, LifecycleRole::Context],
        3 => vec![
            LifecycleRole::Observer,
            LifecycleRole::Context,
            LifecycleRole::Init,
        ],
        4 => vec![
            LifecycleRole::Observer,
            LifecycleRole::Context,
            LifecycleRole::Init,
            LifecycleRole::Collection,
        ],
        5 => vec![
            LifecycleRole::Context,
            LifecycleRole::Init,
            LifecycleRole::Collection,
            LifecycleRole::LastChance,
            LifecycleRole::Cleanup,
        ],
        6 => vec![
            LifecycleRole::Observer,
            LifecycleRole::Context,
            LifecycleRole::Init,
            LifecycleRole::Collection,
            LifecycleRole::LastChance,
            LifecycleRole::Cleanup,
        ],
        _ => vec![LifecycleRole::Unknown; argument_count],
    }
}

fn direct_callback_role_map(argument_count: usize) -> BTreeMap<usize, LifecycleRole> {
    direct_callback_roles(argument_count)
        .into_iter()
        .enumerate()
        .filter(|(_, role)| {
            matches!(
                role,
                LifecycleRole::Observer
                    | LifecycleRole::Emitters
                    | LifecycleRole::Context
                    | LifecycleRole::Init
                    | LifecycleRole::Collection
                    | LifecycleRole::LastChance
                    | LifecycleRole::Cleanup
            )
        })
        .collect()
}

// ===========================================================================
// Observer/store roles
// ===========================================================================

impl<'s, 'a> WiringExtractor<'s, 'a> {
    fn helper_site(&self, role: BindingRole) -> Option<FunctionSite> {
        self.properties
            .helpers
            .iter()
            .find(|helper| helper.role == role)
            .and_then(|helper| {
                self.model.site_by_span(Span::new(
                    helper.function.span.start,
                    helper.function.span.end,
                ))
            })
    }

    fn stores(&mut self) -> StoreRolesWiring {
        let observer_factory = self.helper_site(BindingRole::ObserverFactory);
        let snapshot = self.helper_site(BindingRole::SnapshotGetter);
        let notifier_candidates = self.notifier_candidates();
        let notifier = (notifier_candidates.len() == 1).then(|| notifier_candidates[0].clone());
        let mut evidence = Vec::new();
        let mut unresolved = Vec::new();
        if notifier_candidates.len() > 1 {
            unresolved.push(format!(
                "{} structurally plausible last-chance notifier registrars were found; none is selected",
                notifier_candidates.len()
            ));
        }
        let mut latest_cells = None;
        let mut status_cells = None;
        let mut prefix_scratch = None;

        if let Some(site) = observer_factory {
            evidence.push(self.evidence(
                site.span,
                "observer factory whose next/error/complete closures write the shared stores",
            ));
            for inner in self.model.all_sites() {
                if inner.span == site.span
                    || self
                        .model
                        .enclosing_function(inner.node_id)
                        .map(|owner| owner.span.start != site.span.start)
                        .unwrap_or(true)
                {
                    continue;
                }
                let mut has_status = false;
                for node in self.model.nodes_within(*inner).into_iter() {
                    if !self.model.node_within(node.id(), *inner) {
                        continue;
                    }
                    let AstKind::AssignmentExpression(assignment) = node.kind() else {
                        continue;
                    };
                    // `cell.hasCollected` and `cell["hasCollected"]` are the
                    // same named member; only a non-literal key is a per-slot
                    // index.
                    let object = match &assignment.left {
                        AssignmentTarget::StaticMemberExpression(member) => &member.object,
                        AssignmentTarget::ComputedMemberExpression(member) => &member.object,
                        _ => continue,
                    };
                    match assignment_property_name(&assignment.left).as_deref() {
                        Some("hasCollected") => {
                            if let Some(symbol) = root_identifier_symbol(object, &self.model) {
                                status_cells.get_or_insert_with(|| self.model.binding_ref(symbol));
                                has_status = true;
                                evidence.push(self.evidence(
                                    assignment.span,
                                    "next writes the per-slot status cell",
                                ));
                            }
                        }
                        Some(_) => {}
                        None => {
                            if matches!(
                                assignment.left,
                                AssignmentTarget::ComputedMemberExpression(_)
                            ) {
                                if let Some(symbol) = expression_symbol(object, &self.model) {
                                    latest_cells
                                        .get_or_insert_with(|| self.model.binding_ref(symbol));
                                    evidence.push(self.evidence(
                                        assignment.span,
                                        "next stores the latest value in the per-slot cell",
                                    ));
                                }
                            }
                        }
                    }
                }
                if has_status && latest_cells.is_some() {
                    break;
                }
            }
            for node in self.model.nodes_within(site).into_iter() {
                if !self.model.node_within(node.id(), site) {
                    continue;
                }
                let (name, object, span) = match node.kind() {
                    AstKind::StaticMemberExpression(member) => {
                        (member.property.name.as_str(), &member.object, member.span)
                    }
                    AstKind::ComputedMemberExpression(member) => match &member.expression {
                        Expression::StringLiteral(name) => {
                            (name.value.as_str(), &member.object, member.span)
                        }
                        _ => continue,
                    },
                    _ => continue,
                };
                if name != "scrBytes" {
                    continue;
                }
                if let Some(symbol) = expression_symbol(object, &self.model) {
                    prefix_scratch.get_or_insert_with(|| self.model.binding_ref(symbol));
                    evidence.push(self.evidence(
                        span,
                        "completion hook references the truncated prefix scratch object",
                    ));
                }
            }
        } else {
            unresolved.push(
                "observer factory was not recognized; latest/status/notifier roles stay unproven"
                    .to_string(),
            );
        }
        if latest_cells.is_none() {
            unresolved.push("latest-value cell array was not proven".to_string());
        }
        if status_cells.is_none() {
            unresolved.push("status cell array was not proven".to_string());
        }
        if prefix_scratch.is_none() {
            unresolved.push("completion prefix scratch was not proven".to_string());
        }
        if snapshot.is_none() {
            unresolved.push("snapshot/partition function was not recognized".to_string());
        }
        if notifier.is_none() {
            unresolved.push("last-chance notifier registrar was not proven".to_string());
        }

        StoreRolesWiring {
            observer_factory: observer_factory.map(|site| self.model.function_ref(site)),
            latest_cells,
            status_cells,
            notifier,
            snapshot: snapshot.map(|site| self.model.function_ref(site)),
            prefix_scratch,
            evidence,
            unresolved,
        }
    }

    fn notifier_candidates(&self) -> Vec<NotifierWiring> {
        let mut candidates = Vec::new();
        for site in self.model.all_sites() {
            // Callback first, slot second: two named formals, or a body that
            // reads them as `arguments[0]` / `arguments[1]`.
            let formals = self.formal_symbols(*site);
            if !formals.is_empty() && formals.len() != 2 {
                continue;
            }
            let mut table = None;
            for node in self.model.nodes_within(*site).into_iter() {
                if !self.model.node_within(node.id(), *site) {
                    continue;
                }
                let AstKind::AssignmentExpression(assignment) = node.kind() else {
                    continue;
                };
                let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left else {
                    continue;
                };
                if self.expression_slot_value(&member.expression, *site) != Some(1)
                    || self.expression_slot_value(&assignment.right, *site) != Some(0)
                {
                    continue;
                }
                if let Some(symbol) = expression_symbol(&member.object, &self.model) {
                    table = Some(self.model.binding_ref(symbol));
                }
            }
            if let Some(table) = table {
                candidates.push(NotifierWiring {
                    registrar: self.model.function_ref(*site),
                    table: Some(table),
                    callback_first: true,
                    slot_second: true,
                    evidence: vec![self.evidence(
                        site.span,
                        "last-chance registrar stores table[slot] = callback with callback first and slot second",
                    )],
                });
            }
        }
        candidates
    }
}

fn expression_symbol(expression: &Expression<'_>, model: &Model<'_, '_>) -> Option<SymbolId> {
    match expression {
        Expression::Identifier(identifier) => {
            crate::analysis::identifier_symbol(identifier, model.scoping())
        }
        _ => None,
    }
}

// ===========================================================================
// Public init and collector requests
// ===========================================================================

impl<'s, 'a> WiringExtractor<'s, 'a> {
    fn public_init(&mut self) -> Option<PublicInitWiring> {
        let Some(session) = self.properties.session.as_ref() else {
            self.push_unresolved(
                "public_init",
                "the source did not expose a session constructor/init contract",
                None,
            );
            return None;
        };
        let constructor = session.constructor.clone();
        let constructor_site = self
            .model
            .site_by_span(Span::new(constructor.span.start, constructor.span.end))?;
        let mut calls = Vec::new();
        for node in self.model.kind_nodes(oxc_ast::AstType::NewExpression) {
            let AstKind::NewExpression(new) = node.kind() else {
                continue;
            };
            if self
                .model
                .callee_function(&new.callee)
                .map(|site| site.span.start)
                != Some(constructor_site.span.start)
            {
                continue;
            }
            calls.push((node.id(), new.span, new_arguments(new)));
        }
        // `new` is the recovered shape; a call to a constructor function is
        // accepted only when no new-expression exists, so the two forms never
        // silently compete.
        if calls.is_empty() {
            for node in self.model.kind_nodes(oxc_ast::AstType::CallExpression) {
                let AstKind::CallExpression(call) = node.kind() else {
                    continue;
                };
                if self
                    .model
                    .callee_function(&call.callee)
                    .map(|site| site.span.start)
                    == Some(constructor_site.span.start)
                {
                    calls.push((node.id(), call.span, call_arguments(call)));
                }
            }
        }
        if calls.len() > 1 {
            self.diagnostics.push(WiringDiagnostic {
                code: "public_init_multiple_constructor_calls".to_string(),
                message: format!(
                    "the session constructor is called from {} source sites; the public-init mapping is reported for none",
                    calls.len()
                ),
                span: Some(constructor.span),
            });
            self.push_unresolved(
                "public_init_argument_mapping",
                "more than one session-constructor call site was found; the public argument mapping is not decided",
                Some(Span::new(constructor.span.start, constructor.span.end)),
            );
            return None;
        }
        let Some((node_id, call_span, arguments)) = calls.into_iter().next() else {
            self.push_unresolved(
                "public_init_call",
                "no source call to the session constructor was found",
                Some(Span::new(constructor.span.start, constructor.span.end)),
            );
            return None;
        };
        let Some(public_site) = self.model.enclosing_function(node_id) else {
            self.push_unresolved(
                "public_init_function",
                "the session constructor call is not inside a parsed function body",
                Some(call_span),
            );
            return None;
        };
        let mut mapped = Vec::with_capacity(arguments.len());
        for (index, argument) in arguments.iter().enumerate() {
            let binding = match argument {
                Expression::Identifier(identifier) => {
                    crate::analysis::identifier_symbol(identifier, self.model.scoping())
                        .map(|symbol| self.model.binding_ref(symbol))
                }
                _ => None,
            };
            // A public formal, `arguments[k]` or a local that only holds one
            // public slot all name the same public-init argument.
            let public_init_argument = self.expression_slot_value(argument, public_site);
            let provenance = if public_init_argument.is_some() {
                PublicArgumentProvenance::PublicFormal
            } else if matches!(argument, Expression::Identifier(_)) {
                PublicArgumentProvenance::CapturedBinding
            } else {
                PublicArgumentProvenance::SourceExpression
            };
            mapped.push(PublicInitArgumentWiring {
                constructor_argument: index,
                public_init_argument,
                provenance,
                span: SourceSpan::from(argument.span()),
                source: self.model.exact(argument.span()).to_string(),
                binding,
            });
        }
        let token = self.token_provenance(constructor_site, &mapped);
        Some(PublicInitWiring {
            function: self.model.function_ref(public_site),
            constructor_call_span: SourceSpan::from(call_span),
            arguments: mapped,
            token,
        })
    }

    fn token_provenance(
        &self,
        constructor_site: FunctionSite,
        mapping: &[PublicInitArgumentWiring],
    ) -> Option<TokenProvenanceWiring> {
        let chain = self.properties.session.as_ref()?.hash_chain.as_ref()?;
        let mut evidence = vec![self.evidence(
            Span::new(chain.hash_input.span.start, chain.hash_input.span.end),
            "session state is derived from the transaction-token field",
        )];
        let mut unresolved = Vec::new();
        let (property, root_symbol) = self.member_property_root(&chain.hash_input);
        let mut constructor_argument = None;
        let mut public_init_argument = None;
        if let Some(property) = property.as_deref() {
            let mut candidates = Vec::new();
            for node in self.model.nodes_within(constructor_site).into_iter() {
                if !self.model.node_within(node.id(), constructor_site) {
                    continue;
                }
                let AstKind::AssignmentExpression(assignment) = node.kind() else {
                    continue;
                };
                let left_property = assignment_property_name(&assignment.left);
                if left_property.as_deref() != Some(property) {
                    continue;
                }
                let Some((owner, index)) = self.member_root_slot(&assignment.right) else {
                    continue;
                };
                if owner.span.start != constructor_site.span.start {
                    continue;
                }
                candidates.push((index, assignment.span));
            }
            if candidates.len() == 1 {
                let (index, span) = candidates[0];
                constructor_argument = Some(index);
                public_init_argument = mapping
                    .get(index)
                    .and_then(|argument| argument.public_init_argument);
                evidence.push(self.evidence(
                    span,
                    "context/token field assigned from a session-constructor formal parameter",
                ));
            } else if candidates.len() > 1 {
                unresolved.push(
                    "the transaction-token field has more than one constructor assignment; public provenance is not decided"
                        .to_string(),
                );
            } else {
                unresolved.push(
                    "the transaction-token field was not mapped to a session-constructor formal parameter"
                        .to_string(),
                );
            }
        } else {
            unresolved.push(
                "the hashed token expression is not a direct member access; public provenance is not decided"
                    .to_string(),
            );
        }
        if constructor_argument.is_none() {
            let _ = root_symbol;
        }
        Some(TokenProvenanceWiring {
            transaction_token_field: property,
            state_binding: Some(chain.prng_state.clone()),
            hash_function: Some(chain.hash_function.clone()),
            input_source: chain.hash_input.source.clone(),
            constructor_argument,
            public_init_argument,
            evidence,
            unresolved,
        })
    }

    /// The argument slot at the root of a member chain (`ctx.token`,
    /// `arguments[0]["token"]`, `local.token` for a local that holds a slot).
    fn member_root_slot(&self, expression: &Expression<'a>) -> Option<(FunctionSite, usize)> {
        let mut current = expression.without_parentheses();
        for _ in 0..32 {
            if let Some(found) = self.slot_origin(current) {
                return Some(found);
            }
            current = match current {
                Expression::StaticMemberExpression(member) => member.object.without_parentheses(),
                Expression::ComputedMemberExpression(member) => member.object.without_parentheses(),
                _ => return None,
            };
        }
        None
    }

    fn member_property_root(&self, expression: &ExprRecord) -> (Option<String>, Option<SymbolId>) {
        let Some(node) = self
            .model
            .nodes()
            .iter()
            .find(|node| node.span() == Span::new(expression.span.start, expression.span.end))
        else {
            return (None, None);
        };
        match node.kind() {
            AstKind::StaticMemberExpression(member) => {
                let property = member.property.name.to_string();
                (
                    Some(property),
                    root_identifier_symbol(&member.object, &self.model),
                )
            }
            AstKind::ComputedMemberExpression(member) => {
                let property = match &member.expression {
                    Expression::StringLiteral(literal) => literal.value.to_string(),
                    _ => return (None, None),
                };
                (
                    Some(property),
                    root_identifier_symbol(&member.object, &self.model),
                )
            }
            _ => (None, None),
        }
    }

    fn collector_requests(
        &self,
        groups: &[GroupRouteWiring],
        direct_routes: &[DirectRouteWiring],
    ) -> Vec<CollectorRequestWiring> {
        let mut requests = Vec::with_capacity(groups.len() + direct_routes.len());
        for group in groups {
            requests.push(CollectorRequestWiring {
                function: group.function.span,
                function_source: self
                    .model
                    .exact(Span::new(
                        group.function.span.start,
                        group.function.span.end,
                    ))
                    .to_string(),
                observers: CollectorObserversWiring::WaveSlots(
                    group
                        .row
                        .iter()
                        .enumerate()
                        .map(|(slot, signal)| CollectorSlotWiring {
                            slot: slot as u32,
                            signal: *signal,
                        })
                        .collect(),
                ),
            });
        }
        self.auxiliary_emitter_helpers(groups, &mut requests);
        for route in direct_routes {
            let Some(callback) = route.ctor.callback.as_ref() else {
                continue;
            };
            requests.push(CollectorRequestWiring {
                function: callback.span,
                function_source: self
                    .model
                    .exact(Span::new(callback.span.start, callback.span.end))
                    .to_string(),
                observers: CollectorObserversWiring::Direct {
                    signal: route.signal,
                },
            });
            for hook in &route.ctor.hooks {
                requests.push(CollectorRequestWiring {
                    function: hook.function.span,
                    function_source: self
                        .model
                        .exact(Span::new(hook.function.span.start, hook.function.span.end))
                        .to_string(),
                    observers: CollectorObserversWiring::Direct {
                        signal: route.signal,
                    },
                });
            }
        }
        requests
    }

    /// Add source-proven helper bodies that a group invokes with one of its
    /// resolved local emitters.  Browser probes are commonly factored into a
    /// top-level helper and called through an assignment alias; scanning only
    /// the group body would retain the caller's `next` but lose the helper's
    /// ordered NUMBER/BYTE writes.  The helper is accepted only when its first
    /// argument is the exact local emitter binding and its own body contains a
    /// `next` call, so unrelated function calls cannot acquire a signal route.
    fn auxiliary_emitter_helpers(
        &self,
        groups: &[GroupRouteWiring],
        requests: &mut Vec<CollectorRequestWiring>,
    ) {
        let mut seen = requests
            .iter()
            .map(|request| (request.function.start, request.function.end, 0_u32))
            .collect::<BTreeSet<_>>();
        for group in groups {
            let Some(group_site) = self.model.site_by_span(Span::new(
                group.function.span.start,
                group.function.span.end,
            )) else {
                continue;
            };
            for node in self.model.nodes_within(group_site).into_iter() {
                if !self.model.node_within(node.id(), group_site) {
                    continue;
                }
                let AstKind::CallExpression(call) = node.kind() else {
                    continue;
                };
                let Some(receiver) = call_arguments(call).first().copied() else {
                    continue;
                };
                let Some(receiver_symbol) = (match receiver.without_parentheses() {
                    Expression::Identifier(identifier) => {
                        crate::analysis::identifier_symbol(identifier, self.model.scoping())
                    }
                    _ => None,
                }) else {
                    continue;
                };
                let Some(emitter) = group.local_emitters.iter().find(|emitter| {
                    emitter.aliases.iter().any(|alias| {
                        self.model
                            .symbol_by_index(alias.symbol)
                            .is_some_and(|symbol| symbol == receiver_symbol)
                    })
                }) else {
                    continue;
                };
                let Some(signal) = emitter.global_signal else {
                    continue;
                };
                // Resolve/inspect a helper body only after proving that its
                // first argument is one of this group's emitter bindings.
                let Some(target) = self.model.callee_function(&call.callee) else {
                    continue;
                };
                if target.span.start == group.function.span.start || !self.function_has_next(target)
                {
                    continue;
                }
                let key = (target.span.start, target.span.end, signal);
                if !seen.insert(key) {
                    continue;
                }
                requests.push(CollectorRequestWiring {
                    function: SourceSpan::from(target.span),
                    function_source: self.model.exact(target.span).to_string(),
                    observers: CollectorObserversWiring::Direct { signal },
                });
            }
        }
    }

    fn function_has_next(&self, site: FunctionSite) -> bool {
        self.model.nodes_within(site).into_iter().any(|node| {
            if !self.model.node_within(node.id(), site) {
                return false;
            }
            let AstKind::CallExpression(call) = node.kind() else {
                return false;
            };
            matches!(sink_call(call), Some((EmitterMethod::Next, _, _)))
        })
    }
}

fn assignment_property_name(target: &AssignmentTarget<'_>) -> Option<String> {
    match target {
        AssignmentTarget::StaticMemberExpression(member) => Some(member.property.name.to_string()),
        AssignmentTarget::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some(literal.value.to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn root_identifier_symbol(expression: &Expression<'_>, model: &Model<'_, '_>) -> Option<SymbolId> {
    match expression {
        Expression::Identifier(identifier) => {
            crate::analysis::identifier_symbol(identifier, model.scoping())
        }
        Expression::StaticMemberExpression(member) => root_identifier_symbol(&member.object, model),
        Expression::ComputedMemberExpression(member) => {
            root_identifier_symbol(&member.object, model)
        }
        _ => None,
    }
}
