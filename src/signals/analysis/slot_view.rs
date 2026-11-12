//! [`SlotView`]: what a signal recognizer may read about one slot.
//!
//! A slot is one descriptor of the capture together with its producer, the
//! lifecycle callbacks the producer registers and the emissions the contract
//! pass reconstructed. The view exposes structure only:
//!
//! * spans of the producer body and registered callbacks (to join with
//!   proven finder roles through [`super::FinderCtx`]);
//! * per emission: phase, payload kind, the salt-free grammar rendering and
//!   the ordered value leaves with their mask form, nesting and provenance
//!   (source classes, transforms, host API paths);
//! * host facts of the producer and callback bodies: static member names,
//!   called method names with their string-literal arguments, constructed
//!   names and unbound global reads.
//!
//! The capture-local join facts (address, descriptor index, routing, salts,
//! value spellings) are kept in private fields and are read only by the
//! projection and report code of this module tree, never by recognizers
//! (enforced by `catalog_sources_stay_structural`).

use std::collections::BTreeSet;

use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::Scoping;
use oxc_span::{GetSpan, Span};

use crate::signals::extract::contracts::{
    GrammarNode, SignalContract, SourceClass, WriteNode, DECLARED_NOT_EMITTED,
};
use crate::signals::extract::layout::EmissionVariant;
use crate::signals::extract::wiring::LifecycleRole;
use crate::signals::schema::SourceSpan;

/// The kind of one written value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum LeafOp {
    Number,
    String,
    Byte,
}

/// One value leaf of an emission, in evaluation order.
#[derive(Clone, Debug)]
pub(crate) struct LeafView {
    pub op: LeafOp,
    /// Number of enclosing `REPEAT` nodes.
    pub repeat_depth: usize,
    /// Number of enclosing `CHOICE` nodes.
    pub choice_depth: usize,
    /// Source classes of the value's provenance.
    pub classes: Vec<SourceClass>,
    /// Transforms between the sources and the written value (`^`, `.length`,
    /// `charCodeAt`, `hash`, ...).
    pub transforms: Vec<String>,
    /// Host API paths of host observation / clock sources
    /// (`navigator.plugins.length`, `performance.now()`).
    pub apis: Vec<String>,
    /// For a named-object (generic) emission: the object key the value is
    /// written under. Keys are part of the wire format of that emission.
    pub generic_key: Option<String>,
    /// Where and in which order the value is written (spans only; resolve
    /// the value expression with [`super::FinderCtx::value_node`] /
    /// [`super::FinderCtx::caller_value_node`]).
    pub write: WriteSite,
}

/// The write site of one wire leaf, from the contract's write node.
///
/// Spans are offsets into the analysed program, never spellings: a binder
/// reads what the value is computed from through the parse.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct WriteSite {
    /// Execution order of the write inside its emission: the index of the
    /// leaf in [`EmissionView::leaves`] (contract evaluation order).
    pub order: usize,
    /// The whole write (writer call or indexed store). For a helper write
    /// this lies inside the shared helper.
    pub span: Span,
    /// The logical value expression, without the mask.
    pub value_span: Span,
    /// The caller's helper call, when the write happens inside a program
    /// helper that writes through a writer passed by the caller.
    pub helper_call: Option<SourceSpan>,
    /// For a helper write: the caller's argument span for each helper formal.
    pub helper_arguments: Vec<SourceSpan>,
    /// Spans of the enclosing `CHOICE` nodes (the `if` / conditional /
    /// logical construct), outermost first; resolve a test with
    /// [`super::FinderCtx::guard_test`].
    pub guards: Vec<Span>,
}

impl WriteSite {
    /// The slot's own (caller-side) site of the write: the helper call for a
    /// helper write, otherwise the write itself.
    pub(crate) fn site(&self) -> Span {
        self.helper_call
            .map_or(self.span, |call| Span::new(call.start, call.end))
    }
}

/// One `observer.next` site of the slot.
#[derive(Clone, Debug)]
pub(crate) struct EmissionView {
    pub phase: Option<LifecycleRole>,
    pub payload: EmissionVariant,
    /// Salt-free grammar rendering (`RAW4 NUMBER REPEAT:For/desc{NUMBER}`).
    pub shape: String,
    pub leaves: Vec<LeafView>,
}

/// One call made in the producer or a registered callback.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CallFact {
    /// The static method or global function name (`getContext`, `Object`).
    pub method: String,
    /// String-literal arguments, in order (`["2d"]`).
    pub string_args: Vec<String>,
    /// `new X(...)`.
    pub construct: bool,
}

/// Host facts of a set of spans.
#[derive(Clone, Debug, Default)]
pub(crate) struct HostFacts {
    /// Static member property names (`a.b`, `a["b"]`), generated spellings
    /// (`$...`) excluded.
    pub members: BTreeSet<String>,
    pub calls: Vec<CallFact>,
    /// Unbound (global) identifier reads.
    pub globals: BTreeSet<String>,
}

/// Everything a recognizer or binder may read about one slot.
#[derive(Clone, Debug)]
pub(crate) struct SlotView {
    // ---- capture-local join facts: never recognition inputs ----
    address: String,
    descriptor: u32,
    contract: SignalContract,
    // ---- structure ----
    producer: Option<SourceSpan>,
    callbacks: Vec<(LifecycleRole, SourceSpan)>,
    emissions: Vec<EmissionView>,
    host: HostFacts,
    /// Offsets that belong to this slot alone: its emission and lifecycle
    /// sites, the caller-side write of each value, and the value's
    /// provenance sources and definitions inside the producer/callbacks.
    anchors: Vec<u32>,
}

impl SlotView {
    pub(super) fn new(address: String, contract: SignalContract, host: &HostIndex) -> Self {
        let producer = contract.producer.body;
        let callbacks: Vec<(LifecycleRole, SourceSpan)> = contract
            .producer
            .registrations
            .iter()
            .filter_map(|registration| registration.callback.map(|span| (registration.role, span)))
            .collect();
        let emissions = contract
            .emissions
            .iter()
            .map(|emission| {
                let generic = emission.payload == EmissionVariant::GenericObject;
                let mut leaves = Vec::new();
                collect_leaves(&emission.grammar, 0, 0, generic, &mut Vec::new(), &mut leaves);
                EmissionView {
                    phase: emission.phase,
                    payload: emission.payload.clone(),
                    shape: emission.rendered.clone(),
                    leaves,
                }
            })
            .collect();
        let mut spans: Vec<SourceSpan> = producer.into_iter().collect();
        spans.extend(callbacks.iter().map(|(_, span)| *span));
        let host = host.facts(&spans);
        let anchors = slot_anchors(&contract, &spans);
        Self {
            address,
            descriptor: contract.descriptor,
            contract,
            producer,
            callbacks,
            emissions,
            host,
            anchors,
        }
    }

    /// Offsets owned by this slot (see the field docs): a shared group
    /// producer serves several slots, so its whole span is not slot-specific.
    pub(crate) fn anchors(&self) -> &[u32] {
        &self.anchors
    }

    // ---- join facts (module-internal) ----

    pub(super) fn address(&self) -> &str {
        &self.address
    }

    pub(super) fn descriptor(&self) -> u32 {
        self.descriptor
    }

    pub(super) fn contract(&self) -> &SignalContract {
        &self.contract
    }

    // ---- structure (recognizer API) ----

    /// The producer function body span, when the route was proven.
    pub(crate) fn producer_span(&self) -> Option<SourceSpan> {
        self.producer
    }

    /// Registered lifecycle callbacks with their role.
    pub(crate) fn callback_spans(&self) -> &[(LifecycleRole, SourceSpan)] {
        &self.callbacks
    }

    /// Producer plus callback spans.
    pub(crate) fn spans(&self) -> Vec<SourceSpan> {
        let mut spans: Vec<SourceSpan> = self.producer.into_iter().collect();
        spans.extend(self.callbacks.iter().map(|(_, span)| *span));
        spans
    }

    pub(crate) fn emissions(&self) -> &[EmissionView] {
        &self.emissions
    }

    /// Every value leaf of every emission.
    pub(crate) fn leaves(&self) -> impl Iterator<Item = &LeafView> {
        self.emissions.iter().flat_map(|emission| emission.leaves.iter())
    }

    /// Whether some emission is a named object (generic codec) rather than a
    /// masked byte buffer.
    pub(crate) fn is_generic_object(&self) -> bool {
        self.emissions
            .iter()
            .any(|emission| emission.payload == EmissionVariant::GenericObject)
    }

    /// The salt-free shape of each emission.
    pub(crate) fn shapes(&self) -> Vec<&str> {
        self.emissions.iter().map(|emission| emission.shape.as_str()).collect()
    }

    /// Union of host API paths feeding written values.
    pub(crate) fn value_apis(&self) -> BTreeSet<&str> {
        self.leaves()
            .flat_map(|leaf| leaf.apis.iter().map(String::as_str))
            .collect()
    }

    /// Some written value reads a host API path ending in `suffix`
    /// (`"plugins.length"`, `"deviceMemory"`).
    pub(crate) fn value_reads_api(&self, suffix: &str) -> bool {
        self.leaves()
            .any(|leaf| leaf.apis.iter().any(|api| api_matches(api, suffix)))
    }

    /// Host facts of the producer and callback bodies.
    pub(crate) fn host(&self) -> &HostFacts {
        &self.host
    }

    /// The producer or a callback reads member `name` (`.getImageData`).
    pub(crate) fn uses_member(&self, name: &str) -> bool {
        self.host.members.contains(name)
    }

    /// The producer or a callback reads every member in `names`.
    pub(crate) fn uses_members(&self, names: &[&str]) -> bool {
        names.iter().all(|name| self.uses_member(name))
    }

    /// The producer or a callback reads global `name` (`navigator`, `Proxy`).
    pub(crate) fn reads_global(&self, name: &str) -> bool {
        self.host.globals.contains(name)
    }

    /// String-literal arguments of every call of `method`
    /// (`createElement` -> `[["a"], ["canvas"]]`).
    pub(crate) fn call_string_args(&self, method: &str) -> Vec<&[String]> {
        self.host
            .calls
            .iter()
            .filter(|call| call.method == method)
            .map(|call| call.string_args.as_slice())
            .collect()
    }

    /// Some call of `method` passes `argument` as a string literal.
    pub(crate) fn calls_with(&self, method: &str, argument: &str) -> bool {
        self.host
            .calls
            .iter()
            .any(|call| call.method == method && call.string_args.iter().any(|a| a == argument))
    }

    /// The producer or a callback evaluates `new name(...)`.
    pub(crate) fn constructs(&self, name: &str) -> bool {
        self.host
            .calls
            .iter()
            .any(|call| call.construct && call.method == name)
    }

    /// Leaves of `op` across all emissions.
    pub(crate) fn leaf_count(&self, op: LeafOp) -> usize {
        self.leaves().filter(|leaf| leaf.op == op).count()
    }

    /// Some emission has a `REPEAT`.
    pub(crate) fn has_repeat(&self) -> bool {
        self.leaves().any(|leaf| leaf.repeat_depth > 0)
    }

    /// Declared, not emitted: the producer's task callback completes without
    /// ever mentioning an observer `next`, so the slot never carries a value
    /// (contract reason [`DECLARED_NOT_EMITTED`]).
    pub(crate) fn declared_not_emitted(&self) -> bool {
        self.emissions.is_empty()
            && self
                .contract
                .reasons
                .iter()
                .any(|reason| reason.code == DECLARED_NOT_EMITTED)
    }
}

fn slot_anchors(contract: &SignalContract, spans: &[SourceSpan]) -> Vec<u32> {
    let inside = |offset: u32| spans.iter().any(|span| offset >= span.start && offset < span.end);
    let mut anchors = Vec::new();
    for emission in &contract.emissions {
        anchors.push(emission.site.start);
        anchors.extend(emission.forwarding_sites.iter().map(|site| site.start));
        for write in emission.grammar.writes() {
            // A helper write sits in a shared encoder; the caller's helper
            // call is the slot's own site.
            anchors.push(write.via_helper.map_or(write.span.start, |call| call.start));
            if let Some(provenance) = &write.provenance {
                for source in &provenance.sources {
                    if inside(source.span.start) {
                        anchors.push(source.span.start);
                    }
                    for hop in &source.via {
                        if inside(hop.definition.start) {
                            anchors.push(hop.definition.start);
                        }
                    }
                }
            }
        }
    }
    anchors.extend(contract.lifecycle.iter().map(|site| site.site.start));
    anchors.sort_unstable();
    anchors.dedup();
    anchors
}

/// `api` ends with `suffix` at a path-component boundary.
pub(crate) fn api_matches(api: &str, suffix: &str) -> bool {
    api == suffix
        || api
            .strip_suffix(suffix)
            .is_some_and(|head| head.ends_with('.'))
}

fn collect_leaves(
    node: &GrammarNode,
    repeat_depth: usize,
    choice_depth: usize,
    generic: bool,
    guards: &mut Vec<Span>,
    out: &mut Vec<LeafView>,
) {
    let mut push = |op: LeafOp, write: &WriteNode| {
        let (classes, transforms, apis) = match &write.provenance {
            Some(provenance) => (
                provenance.classes.clone(),
                provenance.transforms.clone(),
                provenance
                    .sources
                    .iter()
                    .filter(|source| {
                        matches!(source.class, SourceClass::HostObservation | SourceClass::HostClock)
                    })
                    .filter_map(|source| source.api.clone())
                    .collect(),
            ),
            None => Default::default(),
        };
        let write_site = WriteSite {
            order: out.len(),
            span: write.span,
            value_span: write.value_span,
            helper_call: write.via_helper,
            helper_arguments: write.helper_arguments.clone(),
            guards: guards.clone(),
        };
        out.push(LeafView {
            write: write_site,
            op,
            repeat_depth,
            choice_depth,
            classes,
            transforms,
            apis,
            generic_key: if generic { write.position.clone() } else { None },
        });
    };
    match node {
        GrammarNode::Number(write) => push(LeafOp::Number, write),
        GrammarNode::String(write) => push(LeafOp::String, write),
        GrammarNode::Byte(write) => push(LeafOp::Byte, write),
        GrammarNode::Sequence { items } => items
            .iter()
            .for_each(|item| collect_leaves(item, repeat_depth, choice_depth, generic, guards, out)),
        GrammarNode::Choice { arms, span, .. } => {
            guards.push(*span);
            arms.iter().for_each(|arm| {
                collect_leaves(&arm.body, repeat_depth, choice_depth + 1, generic, guards, out)
            });
            guards.pop();
        }
        GrammarNode::Repeat { body, .. } => {
            collect_leaves(body, repeat_depth + 1, choice_depth, generic, guards, out)
        }
        GrammarNode::RawPrefix(_) | GrammarNode::Opaque { .. } | GrammarNode::Unresolved { .. } => {
        }
    }
}

// ===========================================================================
// Host index: one walk of the program, queried per slot by span.
// ===========================================================================

#[derive(Clone, Debug)]
enum HostFact {
    Member(String),
    Call(CallFact),
    Global(String),
}

/// Host facts of the whole program, sorted by source offset.
pub(super) struct HostIndex {
    facts: Vec<(u32, HostFact)>,
}

impl HostIndex {
    pub(super) fn build(program: &Program<'_>, scoping: &Scoping) -> Self {
        let mut collector = HostCollector {
            scoping,
            facts: Vec::new(),
        };
        collector.visit_program(program);
        let mut facts = collector.facts;
        facts.sort_by_key(|(start, _)| *start);
        Self { facts }
    }

    fn facts(&self, spans: &[SourceSpan]) -> HostFacts {
        let mut out = HostFacts::default();
        let mut seen_calls = BTreeSet::new();
        for span in spans {
            let first = self.facts.partition_point(|(start, _)| *start < span.start);
            for (start, fact) in &self.facts[first..] {
                if *start >= span.end {
                    break;
                }
                match fact {
                    HostFact::Member(name) => {
                        out.members.insert(name.clone());
                    }
                    HostFact::Global(name) => {
                        out.globals.insert(name.clone());
                    }
                    HostFact::Call(call) => {
                        if seen_calls.insert(call.clone()) {
                            out.calls.push(call.clone());
                        }
                    }
                }
            }
        }
        out
    }
}

struct HostCollector<'s> {
    scoping: &'s Scoping,
    facts: Vec<(u32, HostFact)>,
}

/// A generated spelling (`$A`, `$KEY_…`): never a recognition fact.
pub(crate) fn generated(name: &str) -> bool {
    name.starts_with('$')
}

fn callee_name(callee: &Expression<'_>) -> Option<String> {
    match callee.without_parentheses() {
        Expression::Identifier(identifier) if !generated(&identifier.name) => {
            Some(identifier.name.to_string())
        }
        Expression::StaticMemberExpression(member) => Some(member.property.name.to_string()),
        Expression::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(literal) => Some(literal.value.to_string()),
            _ => None,
        },
        _ => None,
    }
}

fn string_args(arguments: &[Argument<'_>]) -> Vec<String> {
    arguments
        .iter()
        .filter_map(|argument| match argument {
            Argument::StringLiteral(literal) => Some(literal.value.to_string()),
            _ => None,
        })
        .collect()
}

impl<'a> Visit<'a> for HostCollector<'_> {
    fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
        if !generated(&member.property.name) {
            self.facts.push((
                member.span.start,
                HostFact::Member(member.property.name.to_string()),
            ));
        }
        walk::walk_static_member_expression(self, member);
    }

    fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
        if let Expression::StringLiteral(literal) = &member.expression {
            self.facts
                .push((member.span.start, HostFact::Member(literal.value.to_string())));
        }
        walk::walk_computed_member_expression(self, member);
    }

    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let Some(method) = callee_name(&call.callee) {
            self.facts.push((
                call.span.start,
                HostFact::Call(CallFact {
                    method,
                    string_args: string_args(&call.arguments),
                    construct: false,
                }),
            ));
        }
        walk::walk_call_expression(self, call);
    }

    fn visit_new_expression(&mut self, new: &NewExpression<'a>) {
        if let Some(method) = callee_name(&new.callee) {
            self.facts.push((
                new.span.start,
                HostFact::Call(CallFact {
                    method,
                    string_args: string_args(&new.arguments),
                    construct: true,
                }),
            ));
        }
        walk::walk_new_expression(self, new);
    }

    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if !generated(&identifier.name)
            && crate::analysis::identifier_symbol(identifier, self.scoping).is_none()
        {
            self.facts.push((
                identifier.span().start,
                HostFact::Global(identifier.name.to_string()),
            ));
        }
    }
}
