//! Signal finders: the per-seed signal schema of one deobfuscated program.
//!
//! `shape signals <program.js>` runs the contract pass
//! ([`super::contracts`]), the package finders ([`super::finders`]) and then,
//! per slot (one descriptor of this capture with its producer, callbacks and
//! emissions), every registered signal recognizer. A slot that exactly one
//! recognizer claims is projected into the seed-schema signal object
//! ([`project`]) and its fields are named by that signal's binder. Anything
//! else is reported under `diagnostics`, never guessed:
//!
//! * `unidentified` — no recognizer claimed the slot;
//! * `ambiguous` — more than one recognizer claimed it;
//! * `refused` — the binder could not name every field (the user decision:
//!   no `field_1`/`field_k` names are ever emitted);
//! * `no_emission` — the contract pass reconstructed no emission (or proved
//!   the slot declared, not emitted: its producer completes without `next`).
//!
//! # Rules every recognizer and binder follows
//!
//! These extend the package-finder rules of [`super::finders`]:
//!
//! * Recognition reads structure only: the emission shapes, leaf kinds and
//!   nesting, provenance classes and transforms, host API paths of written
//!   values, stable host/API member and method names used in the producer and
//!   its callbacks, string arguments of host calls, and the proven finder
//!   roles on those spans ([`FinderCtx`]).
//! * Never read: the stream salt, descriptor index, slot address or position,
//!   routing name, `$KEY_`/`$THREAD_` or other generated spellings, randomized
//!   literals, numeric build constants or saved catalogs. [`SlotView`] keeps
//!   those private; `catalog_sources_stay_structural` fails a catalog, flag
//!   or shared-helper source that reaches for them.
//! * A binder names fields from the signal's approved card (`CARD` in its
//!   catalog file, [`crate::signals::catalog::card`]), by position or by
//!   provenance (host API path, object key). A binder that cannot bind every
//!   key returns a [`BindRefusal`].
//!
//! # Signal catalog
//!
//! Each signal owns one file in [`crate::signals::catalog`]
//! (`catalog/<signal_snake>.rs`) with its `recognize_*` and `bind_*` and the
//! helpers only it uses. The registry [`SIGNALS`] refers to those functions
//! by name, so a signal change touches only its own file. Packed flag bytes
//! are decoded in [`crate::signals::flags`].
//!
//! # Shared helpers, by technique
//!
//! Helpers used by two or more signals live in this module's children, each
//! with its own unit tests:
//!
//! * [`shape_text`] — salt-free emission shapes as text (compact form, items);
//! * [`leaf_checks`] — predicates over emission leaves and their provenance;
//! * [`layout_nodes`] — projected layout trees: leaf counts, kinds, equality;
//! * [`slot_closures`] — slot-local member/call evidence by def-use hops;
//! * [`render_probes`] — canvas/WebGL/audio/font probes by shape class;
//! * [`buffer_writes`] — writes into a slot's emitted buffer, with slices and
//!   flag-store contexts;
//! * [`native_checks`] — native-code census and tamper-probe decisions;
//! * [`flag_bytes`] — flag-byte counts and positional or hinted binding;
//! * [`property_census`] / [`census_probes`] — the slot-local property
//!   census and the decisions over it;
//! * [`field_solver`] — naming fields by forced uniqueness of rules;
//! * [`value_facts`] / [`described_fields`] — flow-aware value descriptions
//!   and the record/field binder built on them.
//!
//! Catalog-wide tests (registry, structural rules, vendor-seed corpus runs
//! for all signals or `SHAPE_SIGNAL=<Name>`) live in `catalog_tests`.
//!
//! # Helper API
//!
//! Recognizers: [`SlotView`] (`emissions`, `shapes`, `leaves`,
//! `value_reads_api`, `value_apis`, `uses_member(s)`, `reads_global`,
//! `calls_with`, `call_string_args`, `constructs`, `leaf_count`,
//! `has_repeat`, `is_generic_object`, `callback_spans`) and [`FinderCtx`]
//! (`slot_roles`, `has_role`, `roles_inside`, `roles_enclosing`, `parse` for an own AST
//! walk). Shape helpers: [`shape_is`], [`top_level_ops`].
//!
//! Binders: [`SignalLayout::top_keys`] / [`SignalLayout::keys`],
//! [`bind_in_order`], [`bind_messages_in_order`], [`bind_by_object_key`],
//! [`bind_by_api`], [`FieldBinding::name`].

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;
use std::rc::Rc;

use oxc_allocator::Allocator;
use oxc_parser::{ParseOptions, Parser};
use oxc_semantic::SemanticBuilder;
use oxc_span::{SourceType, Span};
use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::signals::extract::contracts::{signal_contracts_from_inputs, ContractStatus, ProducerKind, SignalContract};
use roles::FinderClaim;
pub(crate) use crate::signals::catalog::SIGNALS;
use crate::signals::extract::inputs::extract_program_inputs;
use crate::signals::schema::{Confidence, LiteralValue, SourceIdentity, SourceSpan};

pub(crate) mod slot_view;
pub(crate) mod write_sites;

// Helpers shared by two or more signals, one module per technique.
pub(crate) mod buffer_writes;
pub(crate) mod census_probes;
pub(crate) mod described_fields;
pub(crate) mod field_solver;
pub(crate) mod flag_bytes;
pub(crate) mod layout_nodes;
pub(crate) mod leaf_checks;
pub(crate) mod native_checks;
pub(crate) mod property_census;
pub(crate) mod render_probes;
pub(crate) mod shape_text;
pub(crate) mod slot_closures;
pub(crate) mod value_facts;
pub(crate) use write_sites::ValueSlice;

pub mod roles;

pub use crate::signals::schema::{
    render_signal, BindRefusal, EmissionLayout, FieldBinding, FieldKey, FlatLeaf, LayoutArm,
    LayoutField, LayoutLeaf, LayoutNode, LeafKind, LeafMask, MessageLayout, Routing, SignalLayout,
};
#[allow(unused_imports)]
pub(crate) use slot_view::{api_matches, CallFact, EmissionView, HostFacts, LeafOp, LeafView, SlotView, WriteSite};

// ===========================================================================
// Registry
// ===========================================================================

/// A recognizer's positive identification of one slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Claim {
    pub confidence: Confidence,
    /// Human-readable structural evidence (host names, shapes, roles).
    pub evidence: Vec<String>,
}

impl Claim {
    pub(crate) fn new(evidence: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            confidence: Confidence::High,
            evidence: evidence.into_iter().map(Into::into).collect(),
        }
    }
}

pub(crate) type Recognizer = fn(&SlotView, &FinderCtx) -> Option<Claim>;
/// Binders get the same [`FinderCtx`] as recognizers (parse access, roles,
/// and the per-analysis [`FinderCtx::memo`] where a recognizer leaves facts).
pub(crate) type Binder = fn(&SlotView, &SignalLayout, &FinderCtx) -> Result<FieldBinding, BindRefusal>;

// ===========================================================================
// Recognizer context
// ===========================================================================

/// What a recognizer or binder may read besides its [`SlotView`]: the parse
/// (for an own AST walk), the proven package-finder roles and a memo table
/// that lives for one analysis (one program).
pub(crate) struct FinderCtx<'c, 's, 'a> {
    pub parse: &'c roles::FinderCtx<'s, 'a>,
    pub(crate) claims: &'c [RoleSpan],
    memo: RefCell<HashMap<TypeId, Box<dyn Any>>>,
}

#[derive(Clone, Debug)]
pub(crate) struct RoleSpan {
    pub(crate) role: &'static str,
    pub(crate) span: Span,
    pub(crate) function: bool,
}

fn claim_span(claim: &FinderClaim) -> Span {
    match claim {
        FinderClaim::Function { function_span, .. } => *function_span,
        FinderClaim::MainBinding {
            identifier_span, ..
        } => *identifier_span,
        FinderClaim::ClassBinding { class_span, .. } => *class_span,
    }
}

fn within(inner: Span, outer: SourceSpan) -> bool {
    inner.start >= outer.start && inner.end <= outer.end
}

impl<'c, 's, 'a> FinderCtx<'c, 's, 'a> {
    pub(crate) fn new(parse: &'c roles::FinderCtx<'s, 'a>, claims: &'c [RoleSpan]) -> Self {
        Self {
            parse,
            claims,
            memo: RefCell::new(HashMap::new()),
        }
    }

    /// Per-analysis memo: the value stored under `key` (a helper-private key
    /// type), built by `build` on first use. Replaces per-thread caches keyed
    /// by program text: the table is dropped with the analysis. `build` may
    /// itself call `memo` (no borrow is held while it runs).
    pub(crate) fn memo<K, V>(&self, key: K, build: impl FnOnce() -> V) -> Rc<V>
    where
        K: Hash + Eq + 'static,
        V: 'static,
    {
        let type_id = TypeId::of::<(K, V)>();
        if let Some(found) = self
            .memo
            .borrow()
            .get(&type_id)
            .and_then(|table| table.downcast_ref::<HashMap<K, Rc<V>>>())
            .and_then(|table| table.get(&key))
        {
            return found.clone();
        }
        let value = Rc::new(build());
        let mut memo = self.memo.borrow_mut();
        let table = memo
            .entry(type_id)
            .or_insert_with(|| Box::new(HashMap::<K, Rc<V>>::new()))
            .downcast_mut::<HashMap<K, Rc<V>>>()
            .expect("memo table type");
        table.entry(key).or_insert(value).clone()
    }

    /// Roles of proven functions/bindings inside the producer or a callback
    /// (including the producer function itself).
    pub(crate) fn roles_inside(&self, view: &SlotView) -> Vec<&'static str> {
        let spans = view.spans();
        let mut roles: Vec<&'static str> = self
            .claims
            .iter()
            .filter(|claim| spans.iter().any(|span| within(claim.span, *span)))
            .map(|claim| claim.role)
            .collect();
        roles.sort_unstable();
        roles.dedup();
        roles
    }

    /// Roles of proven functions strictly enclosing the producer (a group
    /// body shared by a whole row, an outer probe closure).
    pub(crate) fn roles_enclosing(&self, view: &SlotView) -> Vec<&'static str> {
        let Some(producer) = view.producer_span() else {
            return Vec::new();
        };
        let mut roles: Vec<&'static str> = self
            .claims
            .iter()
            .filter(|claim| {
                claim.span.start <= producer.start
                    && claim.span.end >= producer.end
                    && (claim.span.start, claim.span.end) != (producer.start, producer.end)
            })
            .map(|claim| claim.role)
            .collect();
        roles.sort_unstable();
        roles.dedup();
        roles
    }

    /// Roles specific to this slot: for every slot anchor
    /// ([`SlotView::anchors`]) the innermost proven function containing it,
    /// when that function lies inside the producer or a callback. Unlike
    /// [`Self::roles_inside`], a group body shared by a row does not make
    /// every slot of the row carry every role of the row.
    pub(crate) fn slot_roles(&self, view: &SlotView) -> Vec<&'static str> {
        let spans = view.spans();
        let mut roles = Vec::new();
        for anchor in view.anchors() {
            let innermost = self
                .claims
                .iter()
                .filter(|claim| claim.function)
                .filter(|claim| claim.span.start <= *anchor && *anchor < claim.span.end)
                .filter(|claim| spans.iter().any(|span| within(claim.span, *span)))
                .min_by_key(|claim| claim.span.end - claim.span.start);
            if let Some(claim) = innermost {
                roles.push(claim.role);
            }
        }
        roles.sort_unstable();
        roles.dedup();
        roles
    }

    /// `role` is a slot role ([`Self::slot_roles`]).
    pub(crate) fn has_role(&self, view: &SlotView, role: &str) -> bool {
        self.slot_roles(view).contains(&role)
    }
}

// ===========================================================================
// Recognizer shape helpers
// ===========================================================================

/// Every emission's salt-free shape equals `shape` (`"RAW4 NUMBER"`).
pub(crate) fn shape_is(view: &SlotView, shape: &str) -> bool {
    !view.emissions().is_empty() && view.shapes().iter().all(|s| *s == shape)
}

/// The leaf kinds of the first emission's top-level (unnested) leaves.
pub(crate) fn top_level_ops(view: &SlotView) -> Vec<LeafOp> {
    view.emissions()
        .first()
        .map(|emission| {
            emission
                .leaves
                .iter()
                .filter(|leaf| leaf.repeat_depth == 0 && leaf.choice_depth == 0)
                .map(|leaf| leaf.op)
                .collect()
        })
        .unwrap_or_default()
}

// ===========================================================================
// Binder helpers
// ===========================================================================

/// Name the top-level fields of a single-message layout in order. Refuses
/// when the layout has more than one message or the field count differs.
/// Nested record keys are not bound here (see [`FieldBinding::name`]).
pub(crate) fn bind_in_order(
    layout: &SignalLayout,
    names: &[&str],
) -> Result<FieldBinding, BindRefusal> {
    if layout.messages.len() != 1 {
        return Err(BindRefusal::new(format!(
            "expected one message layout, found {}",
            layout.messages.len()
        )));
    }
    bind_messages_in_order(layout, &[names])
}

/// Name the top-level fields of each message in order: `names[m]` for
/// message `m`.
pub(crate) fn bind_messages_in_order(
    layout: &SignalLayout,
    names: &[&[&str]],
) -> Result<FieldBinding, BindRefusal> {
    if layout.messages.len() != names.len() {
        return Err(BindRefusal::new(format!(
            "expected {} message layouts, found {}",
            names.len(),
            layout.messages.len()
        )));
    }
    let mut binding = FieldBinding::new();
    for (message, names) in names.iter().enumerate() {
        let keys = layout.top_keys(message);
        if keys.len() != names.len() {
            return Err(BindRefusal::new(format!(
                "message {message}: expected {} fields, found {}",
                names.len(),
                keys.len()
            )));
        }
        for (key, name) in keys.into_iter().zip(names.iter()) {
            binding.insert(message, key, *name);
        }
    }
    Ok(binding)
}

/// Name the fields of a named-object (generic) emission by object key:
/// `(wire key, card name)`. Every top-level field must be matched.
pub(crate) fn bind_by_object_key(
    layout: &SignalLayout,
    names: &[(&str, &str)],
) -> Result<FieldBinding, BindRefusal> {
    let mut binding = FieldBinding::new();
    for (message, layout_message) in layout.messages.iter().enumerate() {
        for field in &layout_message.fields {
            let key = match &field.node {
                LayoutNode::Leaf(leaf) => leaf.key.as_deref(),
                _ => None,
            };
            let name = key.and_then(|key| names.iter().find(|(wire, _)| *wire == key));
            match name {
                Some((_, name)) => binding.insert(message, field.key.clone(), *name),
                None => {
                    return Err(BindRefusal::new(format!(
                        "message {message}: field {} has no card name for key {key:?}",
                        field.key
                    )))
                }
            }
        }
    }
    Ok(binding)
}

// ===========================================================================
// Report
// ===========================================================================

/// Options of [`analyze_program_signals`].
#[derive(Clone, Debug)]
pub struct SignalFinderOptions {
    /// Also emit the positional-key layout of every unidentified or refused
    /// slot under its diagnostic (`--layouts-only`).
    pub layouts_only: bool,
    /// Run the package finders for [`FinderCtx`] roles.
    pub finder_roles: bool,
}

impl Default for SignalFinderOptions {
    fn default() -> Self {
        Self {
            layouts_only: false,
            finder_roles: true,
        }
    }
}

/// One slot reported outside `signals`.
#[derive(Clone, Debug, Serialize)]
pub struct SlotDiagnostic {
    pub address: String,
    /// The canonical signal the slot was identified as (refused slots: the
    /// one recognizer that claimed it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
    #[serde(flatten)]
    pub routing: Map<String, Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<String>,
    pub reason: String,
    pub contract_status: ContractStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<Value>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SignalDiagnostics {
    pub unidentified: Vec<SlotDiagnostic>,
    pub ambiguous: Vec<SlotDiagnostic>,
    pub refused: Vec<SlotDiagnostic>,
    pub no_emission: Vec<SlotDiagnostic>,
    /// Packed flag bytes of emitted signals rendered as plain `byte`
    /// because a bit could not be derived or named.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flags_fallback: Vec<FlagsFallback>,
}

/// One packed flags field kept as `byte`.
#[derive(Clone, Debug, Serialize)]
pub struct FlagsFallback {
    pub address: String,
    pub signal: String,
    pub field: String,
    pub reason: String,
}

/// The `shape signals` report: the per-seed signal schema.
#[derive(Clone, Debug, Serialize)]
pub struct SignalSchemaReport {
    /// The seed ID of the program: the program file stem (`<seed>.js`), set
    /// by the caller that knows the path (never read from the program).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<String>,
    pub source: SourceIdentity,
    pub signals: Map<String, Value>,
    pub diagnostics: SignalDiagnostics,
    /// Lossless, versioned source contract; descriptive fields above are unchanged.
    pub wire: crate::signals::wire::WireSchema,
}

/// Per-slot facts for coverage and verification (not part of the schema).
#[derive(Clone, Debug)]
pub struct SlotSummary {
    /// `wave:<group>:<position>` or `direct:<descriptor>`.
    pub address: String,
    pub descriptor: u32,
    pub routing: Routing,
    pub contract_status: ContractStatus,
    pub producer: Option<SourceSpan>,
    pub callbacks: Vec<SourceSpan>,
    /// Innermost proven functions around this slot's own sites.
    pub slot_roles: Vec<&'static str>,
    /// Every proven role inside the producer or its callbacks (shared by all
    /// slots of a group row).
    pub roles_inside: Vec<&'static str>,
    pub roles_enclosing: Vec<&'static str>,
    pub layout: SignalLayout,
    pub candidates: Vec<&'static str>,
    /// The schema key this slot was emitted under (with `_2` suffix).
    pub emitted_as: Option<String>,
    /// Post-flags source bindings, retained for the versioned named projection.
    pub binding: Option<crate::signals::schema::FieldBinding>,
}

/// The report plus per-slot facts.
#[derive(Clone, Debug)]
pub struct SignalAnalysis {
    pub report: SignalSchemaReport,
    pub slots: Vec<SlotSummary>,
}

mod completion;

/// Why [`analyze_program_signals`] failed.
#[derive(Debug)]
pub struct SignalFinderError(pub String);

impl std::fmt::Display for SignalFinderError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for SignalFinderError {}

fn address_of(contract: &SignalContract) -> String {
    match &contract.producer.kind {
        ProducerKind::Grouped {
            group, position, ..
        } => format!("wave:{group}:{position}"),
        ProducerKind::Direct { .. } => format!("direct:{}", contract.descriptor),
        ProducerKind::Unrouted => format!("unrouted:{}", contract.descriptor),
    }
}

fn routing_map(routing: &Routing) -> Map<String, Value> {
    let mut map = Map::new();
    match routing {
        Routing::Positional(index) => {
            map.insert("type".into(), json!("positional"));
            map.insert("positional".into(), json!(index));
        }
        Routing::Name(name) => {
            map.insert("type".into(), json!("name"));
            map.insert("name".into(), name.clone());
        }
    }
    map
}

/// The routing name of each descriptor, from the capture's own registry.
fn descriptor_names(inputs: &crate::signals::extract::inputs::ProgramInputReport) -> BTreeMap<u32, Value> {
    let mut names = BTreeMap::new();
    if let Some(registry) = &inputs.wiring.descriptors {
        for entry in &registry.entries {
            let name = match &entry.name {
                Some(LiteralValue::String(name)) => json!(name),
                Some(LiteralValue::Number(number)) if number.fract() == 0.0 => {
                    json!(*number as i64)
                }
                Some(LiteralValue::Number(number)) => json!(number),
                _ => continue,
            };
            names.insert(entry.index as u32, name);
        }
    }
    names
}

/// Run the signal finders on one program.
/// The package finders' proven role claims as spans, from an own parse of
/// `source` (the parse the analysis makes later is the same source, so the
/// spans are shared). A parse that panics claims nothing; the analysis then
/// refuses the program on its own parse.
fn finder_role_spans(source: &str) -> Vec<RoleSpan> {
    let _t = super::timing::span("finder roles");
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::unambiguous())
        .with_options(ParseOptions {
            preserve_parens: false,
            ..ParseOptions::default()
        })
        .parse();
    if parsed.panicked {
        return Vec::new();
    }
    let program = parsed.program;
    let semantic = SemanticBuilder::new()
        .with_check_syntax_error(false)
        .build(&program)
        .semantic;
    let (scoping, nodes) = semantic.into_scoping_and_nodes();
    let parse = roles::FinderCtx {
        source,
        program: &program,
        scoping: &scoping,
        nodes: &nodes,
    };
    roles::run_all(&parse)
        .iter()
        .map(|claim| RoleSpan {
            role: claim.role(),
            span: claim_span(claim),
            function: matches!(claim, FinderClaim::Function { .. }),
        })
        .collect()
}

pub fn analyze_program_signals(
    source: &str,
    options: &SignalFinderOptions,
) -> Result<SignalAnalysis, SignalFinderError> {
    let _total = super::timing::span("signals total");
    // The package role finders read only their own parse of the source, so
    // they run beside the input/contract passes; their claims are plain spans.
    let (inputs_and_contracts, role_spans) = std::thread::scope(|scope| {
        let finder_roles = options.finder_roles.then(|| {
            std::thread::Builder::new()
                .stack_size(crate::LARGE_STACK_BYTES)
                .spawn_scoped(scope, || finder_role_spans(source))
                .expect("spawn finder-role thread")
        });
        let t = super::timing::span("inputs");
        let inputs = extract_program_inputs(source).map_err(|error| SignalFinderError(error.to_string()));
        drop(t);
        let t = super::timing::span("contracts");
        let both = inputs.and_then(|inputs| {
            signal_contracts_from_inputs(source, &inputs)
                .map(|contracts| (inputs, contracts))
                .map_err(|error| SignalFinderError(error.to_string()))
        });
        drop(t);
        let roles = finder_roles.map(|handle| handle.join());
        (both, roles)
    });
    let (inputs, contracts) = inputs_and_contracts?;
    let role_spans: Vec<RoleSpan> = match role_spans {
        Some(Ok(spans)) => spans,
        Some(Err(panic)) => std::panic::resume_unwind(panic),
        None => Vec::new(),
    };
    let t = super::timing::span("parse+semantic");
    let names = descriptor_names(&inputs);

    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::unambiguous())
        .with_options(ParseOptions {
            preserve_parens: false,
            ..ParseOptions::default()
        })
        .parse();
    if parsed.panicked {
        return Err(SignalFinderError("parse failed".into()));
    }
    let program = parsed.program;
    let semantic = SemanticBuilder::new()
        .with_check_syntax_error(false)
        .build(&program)
        .semantic;
    let (scoping, nodes) = semantic.into_scoping_and_nodes();
    let parse = roles::FinderCtx {
        source,
        program: &program,
        scoping: &scoping,
        nodes: &nodes,
    };
    drop(t);
    let t = super::timing::span("host+positional");
    let ctx = FinderCtx::new(&parse, &role_spans);
    let host = slot_view::HostIndex::build(&program, &scoping);

    let recovered = positional_registrations(&contracts.signals, &ctx);
    drop(t);
    let t = super::timing::span("recognize");
    let mut positional = 0usize;
    let mut slots = Vec::new();
    let mut views = Vec::new();
    for contract in &contracts.signals {
        let contract = &with_registrations(contract, &recovered);
        let routing = match names.get(&contract.descriptor) {
            Some(name) => Routing::Name(name.clone()),
            None => {
                positional += 1;
                Routing::Positional(positional - 1)
            }
        };
        let view = SlotView::new(address_of(contract), contract.clone(), &host);
        let layout = crate::signals::schema::project_contract(view.contract());
        // Debug aid: `SHAPE_SIGNAL_DUMP=<address>[,<address>]` prints the view.
        if std::env::var("SHAPE_SIGNAL_DUMP")
            .is_ok_and(|list| list.split(',').any(|address| address == view.address()))
        {
            eprintln!("DUMP {} producer={:?} host={:?}", view.address(), view.producer_span(), view.host());
            for emission in view.emissions() {
                eprintln!("  emission {:?} {}", emission.phase, emission.shape);
                for leaf in &emission.leaves {
                    eprintln!(
                        "    {:?} r{} c{} classes={:?} tf={:?} apis={:?} write={:?}",
                        leaf.op, leaf.repeat_depth, leaf.choice_depth, leaf.classes, leaf.transforms, leaf.apis, leaf.write.span
                    );
                }
            }
        }
        let candidates: Vec<&'static str> = SIGNALS
            .iter()
            .filter(|entry| (entry.recognize)(&view, &ctx).is_some())
            .map(|entry| entry.canonical)
            .collect();
        slots.push(SlotSummary {
            address: view.address().to_string(),
            descriptor: view.descriptor(),
            routing,
            contract_status: contract.status,
            producer: view.producer_span(),
            callbacks: view.callback_spans().iter().map(|(_, span)| *span).collect(),
            slot_roles: ctx.slot_roles(&view),
            roles_inside: ctx.roles_inside(&view),
            roles_enclosing: ctx.roles_enclosing(&view),
            layout,
            candidates,
            emitted_as: None,
            binding: None,
        });
        views.push(view);
    }

    drop(t);
    let _t = super::timing::span("bind");
    let mut signals = Map::new();
    let mut diagnostics = SignalDiagnostics::default();
    let reserved: std::collections::BTreeSet<_> = slots
        .iter()
        .flat_map(|slot| slot.candidates.iter().copied())
        .collect();
    let mut seen: BTreeMap<&'static str, usize> = BTreeMap::new();
    for (slot, view) in slots.iter_mut().zip(&views) {
        let diagnostic = |reason: String, layout: bool| SlotDiagnostic {
            address: slot.address.clone(),
            signal: match slot.candidates.as_slice() {
                [one] => Some(one.to_string()),
                _ => None,
            },
            routing: routing_map(&slot.routing),
            candidates: slot.candidates.iter().map(|name| name.to_string()).collect(),
            reason,
            contract_status: slot.contract_status,
            layout: if layout && options.layouts_only {
                render_signal(&slot.layout, &slot.routing, None).ok()
            } else {
                None
            },
        };
        if slot.layout.emissions.is_empty() {
            let declared_not_emitted = view.declared_not_emitted();
            // A declared-not-emitted slot that exactly one no-data signal
            // claims is a signal with no data: listed under `signals` with
            // `type: "no_data"`, no fields and its page-visible `effect`.
            if declared_not_emitted && completion::prove(view.contract(), &nodes, &scoping) {
                if let [one] = slot.candidates.as_slice() {
                    if let Some(effect) = crate::signals::catalog::no_data_effect(one, view, &ctx) {
                        // The routing stays (`name` / `positional`); only
                        // `type` says there is no data.
                        let mut map = routing_map(&slot.routing);
                        map.insert("type".into(), json!("no_data"));
                        map.insert("fields".into(), json!({}));
                        map.insert("effect".into(), Value::Object(effect));
                        map.insert("behavior".into(), json!("completion_only"));
                        map.insert("payload".into(), json!("none"));
                        map.insert("address".into(), json!(slot.address));
                        let value = Value::Object(map);
                        let count = seen.entry(one).or_default();
                        let key = crate::signals::wire::emitted_key(one, count, &reserved, &signals);
                        slot.emitted_as = Some(key.clone());
                        signals.insert(key, value);
                        continue;
                    }
                }
            }
            let reason = if declared_not_emitted {
                "declared, not emitted: the producer completes without ever calling next, so this slot never carries a collected value"
            } else {
                "the contract pass reconstructed no emission"
            };
            diagnostics.no_emission.push(diagnostic(reason.into(), false));
            continue;
        }
        let entry = match slot.candidates.as_slice() {
            [] => {
                diagnostics
                    .unidentified
                    .push(diagnostic("no recognizer claimed this slot".into(), true));
                continue;
            }
            [one] => SIGNALS
                .iter()
                .find(|entry| entry.canonical == *one)
                .expect("candidate comes from SIGNALS"),
            _ => {
                diagnostics
                    .ambiguous
                    .push(diagnostic("more than one recognizer claimed this slot".into(), true));
                continue;
            }
        };
        let rendered = (entry.bind)(view, &slot.layout, &ctx)
            .map_err(|refusal| refusal.reason)
            .map(|mut binding| {
                // Packed flag bytes: bit positions from the write sites, bit
                // names from the card (shared; signal binders need no edit).
                // A field whose bits cannot all be named stays a `byte`.
                for (field, reason) in crate::signals::flags::apply_flags_or_fall_back(entry.canonical, view, &slot.layout, &ctx, &mut binding) {
                    diagnostics.flags_fallback.push(FlagsFallback {
                        address: slot.address.clone(),
                        signal: entry.canonical.to_string(),
                        field,
                        reason,
                    });
                }
                binding
            })
            .and_then(|binding| {
                let rendered = render_signal(&slot.layout, &slot.routing, Some(&binding));
                if rendered.is_ok() {
                    slot.binding = Some(binding);
                }
                rendered
            });
        match rendered {
            Ok(mut value) => {
                if let Value::Object(map) = &mut value {
                    map.insert("address".into(), json!(slot.address));
                }
                let count = seen.entry(entry.canonical).or_default();
                let key =
                    crate::signals::wire::emitted_key(entry.canonical, count, &reserved, &signals);
                slot.emitted_as = Some(key.clone());
                signals.insert(key, value);
            }
            Err(reason) => diagnostics.refused.push(diagnostic(reason, true)),
        }
    }
    let wire = crate::signals::wire::WireSchema::from_analysis(source, &inputs, &contracts, &slots)
        .map_err(|error| SignalFinderError(error.to_string()))?;
    crate::signals::wire::semantic::describe(&mut signals, &wire);
    Ok(SignalAnalysis {
        report: SignalSchemaReport {
            seed: None,
            source: contracts.source.clone(),
            signals,
            diagnostics,
            wire,
        },
        slots,
    })
}

/// Lifecycle registrations recovered from the producer callback ABI
/// `(emitters, context, init, collection, last_chance, cleanup)` for
/// producers whose coordinator invocation was not proven (no argument roles,
/// no registrations): a function passed to a call of formal 2..=5 is
/// registered in that position's role. Grouped producers are all invoked by
/// the one group coordinator, so they share the ABI: the table's producers
/// together must call registrars at two or more positions. A direct
/// producer must do so itself. Keyed by producer body span.
fn positional_registrations(
    signals: &[SignalContract],
    ctx: &FinderCtx,
) -> HashMap<SourceSpan, Vec<crate::signals::extract::contracts::Registration>> {
    use crate::signals::extract::wiring::LifecycleRole;
    let role = |position: usize| match position {
        2 => Some(LifecycleRole::Init),
        3 => Some(LifecycleRole::Collection),
        4 => Some(LifecycleRole::LastChance),
        5 => Some(LifecycleRole::Cleanup),
        _ => None,
    };
    let mut found: Vec<(SourceSpan, bool, Vec<write_sites::RegistrarCall>)> = Vec::new();
    for contract in signals {
        let producer = &contract.producer;
        let Some(body) = producer.body else {
            continue;
        };
        if !producer.registrations.is_empty()
            || !producer.arguments.is_empty()
            || found.iter().any(|(seen, _, _)| *seen == body)
        {
            continue;
        }
        let Some(calls) = ctx.registrar_calls(body) else {
            continue;
        };
        let calls: Vec<_> = calls.into_iter().filter(|(position, _, _)| role(*position).is_some()).collect();
        found.push((body, matches!(producer.kind, ProducerKind::Grouped { .. }), calls));
    }
    let positions = |calls: &[write_sites::RegistrarCall]| -> std::collections::BTreeSet<usize> {
        calls.iter().map(|(position, _, _)| *position).collect()
    };
    let grouped_positions: std::collections::BTreeSet<usize> = found
        .iter()
        .filter(|(_, grouped, _)| *grouped)
        .flat_map(|(_, _, calls)| positions(calls))
        .collect();
    let mut out = HashMap::new();
    for (body, grouped, calls) in found {
        let evidence = if grouped { grouped_positions.len() } else { positions(&calls).len() };
        if evidence < 2 || calls.is_empty() {
            continue;
        }
        let registrations = calls
            .into_iter()
            .filter_map(|(position, callback, call)| {
                Some(crate::signals::extract::contracts::Registration {
                    role: role(position)?,
                    callback: Some(callback),
                    call,
                })
            })
            .collect();
        out.insert(body, registrations);
    }
    out
}

/// `contract` with recovered registrations (see [`positional_registrations`])
/// and the phases they give its unphased emission and lifecycle sites: the
/// innermost registered callback containing the site.
fn with_registrations(
    contract: &SignalContract,
    recovered: &HashMap<SourceSpan, Vec<crate::signals::extract::contracts::Registration>>,
) -> SignalContract {
    let mut contract = contract.clone();
    let Some(registrations) = contract.producer.body.and_then(|body| recovered.get(&body)) else {
        return contract;
    };
    let phase_at = |site: SourceSpan| {
        registrations
            .iter()
            .filter_map(|registration| registration.callback.map(|callback| (registration.role, callback)))
            .filter(|(_, callback)| callback.start <= site.start && site.end <= callback.end)
            .min_by_key(|(_, callback)| callback.end - callback.start)
            .map(|(role, _)| role)
    };
    for emission in &mut contract.emissions {
        if emission.phase.is_none() {
            emission.phase = phase_at(emission.site);
        }
    }
    for site in &mut contract.lifecycle {
        if site.phase.is_none() {
            site.phase = phase_at(site.site);
        }
    }
    contract.producer.registrations = registrations.clone();
    contract
}

#[cfg(test)]
pub(crate) mod catalog_tests;
