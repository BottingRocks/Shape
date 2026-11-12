//! Source-derived properties of the local NUMBER, STRING, and byte-stream
//! writers.
//!
//! This pass is deliberately independent of the signal collector wiring.  It
//! parses one exact source, resolves identifier references in that parse, and
//! retains the source evidence used for every recovered fact.  Generated names
//! are labels only: helper identity is the resolved function span and the
//! relationship between its parameters, writes, calls, and returned closure.
//!
//! Helper parameters are argument *slots*: the Nth plain, never-written
//! formal and `arguments[N]` of an ordinary function are the same slot (see
//! [`super::bindings::ParameterSlots`]), so a helper is recognized alike in
//! named-formal captures and in `arguments[N]` traces. Member names are read
//! the same whether written `a.name` or `a["name"]`.
//!
//! The pass describes the encoder; it does not execute JavaScript or implement
//! the codec/PRNG.  Runtime token values, random words, and host-dependent
//! floating-point byte order therefore remain explicit unknowns.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ops::Deref;

use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UnaryOperator};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::bindings::{ParameterSlot, ParameterSlots};
use super::reaching::expression_value_at_use;
use crate::signals::extract::layout;
use crate::signals::schema::{ProgramPropertiesError, SourceIdentity, SourceSpan};

/// Version 2: a parameter is an argument slot, so its binding is optional
/// (an `arguments[N]` slot has none).
pub const ENCODING_PROPERTIES_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncodingPropertiesError {
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
}

impl std::fmt::Display for EncodingPropertiesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(message) => {
                write!(f, "invalid encoding-properties input: {message}")
            }
            Self::Parse(message) => write!(f, "encoding-properties parse failed: {message}"),
            Self::Limit { what, limit } => {
                write!(f, "encoding-properties limit exceeded for {what} ({limit})")
            }
            Self::Ambiguous { what, candidates } => write!(
                f,
                "encoding-properties ambiguity for {what}: {} candidates",
                candidates.len()
            ),
        }
    }
}

impl std::error::Error for EncodingPropertiesError {}

impl From<ProgramPropertiesError> for EncodingPropertiesError {
    fn from(error: ProgramPropertiesError) -> Self {
        match error {
            ProgramPropertiesError::InvalidInput(message) => Self::InvalidInput(message),
            ProgramPropertiesError::Parse(message) => Self::Parse(message),
            ProgramPropertiesError::Limit { what, limit } => Self::Limit { what, limit },
            ProgramPropertiesError::Ambiguous { what, candidates } => {
                Self::Ambiguous { what, candidates }
            }
            ProgramPropertiesError::Missing { what } => {
                Self::InvalidInput(format!("missing {what}"))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingEvidence {
    pub symbol: u32,
    pub name: String,
    pub declaration: SourceSpan,
    pub identifier: SourceSpan,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionEvidence {
    pub binding: Option<BindingEvidence>,
    pub span: SourceSpan,
    pub body: Option<SourceSpan>,
    pub name_label: Option<String>,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub span: SourceSpan,
    pub source: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterEvidence {
    pub index: usize,
    /// The named formal of this slot; `None` for a slot read only as
    /// `arguments[index]`.
    pub binding: Option<BindingEvidence>,
    pub role: String,
    pub evidence: Vec<Evidence>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NumberBranch {
    pub kind: String,
    pub evidence: Vec<Evidence>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NumberWriterProperties {
    pub function: FunctionEvidence,
    pub parameters: Vec<ParameterEvidence>,
    pub branches: Vec<NumberBranch>,
    pub buffer_write_count: usize,
    pub continuation_masks: Vec<Evidence>,
    pub shifts: Vec<Evidence>,
    pub float_scratch: Option<Evidence>,
    pub fallback: Option<Evidence>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StringFactoryProperties {
    pub factory: FunctionEvidence,
    pub writer: Option<FunctionEvidence>,
    pub parameters: Vec<ParameterEvidence>,
    pub cache: Option<BindingEvidence>,
    pub counter: Option<BindingEvidence>,
    pub miss_traversal: Vec<Evidence>,
    pub terminator: Option<Evidence>,
    pub terminator_is_literal_zero: bool,
    pub number_dependency: Option<FunctionEvidence>,
    pub unresolved: Vec<UnresolvedEncodingFact>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamStateProperties {
    pub binding: Option<BindingEvidence>,
    pub role: String,
    pub evidence: Vec<Evidence>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByteStreamProperties {
    pub factory: FunctionEvidence,
    pub cycle: Option<FunctionEvidence>,
    pub captured_state: Vec<StreamStateProperties>,
    pub warmup_calls: Option<usize>,
    pub cycle_modulus: Option<Evidence>,
    pub word_updates: Vec<Evidence>,
    pub extraction_shifts: Vec<Evidence>,
    pub seed_parameter: Option<ParameterEvidence>,
    pub local_seed: Option<Evidence>,
    pub per_caller_prefix: bool,
    pub shared_cursor_writeback: Option<Evidence>,
    pub unresolved: Vec<UnresolvedEncodingFact>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedLinkProperties {
    pub hash: Option<FunctionEvidence>,
    pub seed_reads: Vec<Evidence>,
    pub runtime_value: bool,
    pub unresolved: Vec<UnresolvedEncodingFact>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnresolvedEncodingFact {
    pub fact: String,
    pub reason: String,
    pub span: Option<SourceSpan>,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncodingProperties {
    pub schema_version: u32,
    pub source: SourceIdentity,
    pub number: Option<NumberWriterProperties>,
    pub string: Option<StringFactoryProperties>,
    pub byte_stream: Option<ByteStreamProperties>,
    /// Further copies of the NUMBER writer, identical in role to `number`.
    ///
    /// A capture that traces several threads repeats each encoder helper once
    /// per thread. `number` stays the canonical copy for the reported
    /// properties; every copy is listed here so a write site can be resolved to
    /// the binding it actually captured.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub number_equivalents: Vec<SourceSpan>,
    /// Further copies of the STRING writer factory.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub string_equivalents: Vec<SourceSpan>,
    /// Further copies of the PRNG/byte-stream factory.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stream_equivalents: Vec<SourceSpan>,
    pub seed_link: SeedLinkProperties,
    pub unresolved: Vec<UnresolvedEncodingFact>,
}

impl EncodingProperties {
    /// Every proven function span for one helper role: the canonical copy
    /// followed by its equivalents, deduplicated and sorted.
    fn role_spans(primary: Option<SourceSpan>, equivalents: &[SourceSpan]) -> Vec<Span> {
        let mut spans: Vec<Span> = primary.into_iter().chain(equivalents.iter().copied())
            .map(|span| Span::new(span.start, span.end))
            .collect();
        spans.sort_by_key(|span| (span.start, span.end));
        spans.dedup();
        spans
    }
}

impl EncodingProperties {
    /// Convert the helper identities to the existing signal-layout input
    /// contract. Spans belong to this exact source parse; callers must not
    /// compare the returned spans or symbols with another parse.
    pub fn helper_bindings(&self) -> layout::HelperBindings {
        layout::HelperBindings {
            number_writer: Self::role_spans(
                self.number.as_ref().map(|value| value.function.span),
                &self.number_equivalents,
            ),
            string_writer: Self::role_spans(
                self.string.as_ref().map(|value| value.factory.span),
                &self.string_equivalents,
            ),
            prng_factory: Self::role_spans(
                self.byte_stream.as_ref().map(|value| value.factory.span),
                &self.stream_equivalents,
            ),
            hash: self
                .seed_link
                .hash
                .as_ref()
                .map(|value| to_oxc_span(value.span)),
        }
    }

    pub fn as_signal_helpers(&self) -> layout::HelperBindings {
        self.helper_bindings()
    }
}

fn to_oxc_span(span: SourceSpan) -> Span {
    Span::new(span.start, span.end)
}

/// One argument slot of a [`FunctionSite`].
#[derive(Clone, Copy)]
struct Parameter {
    index: usize,
    /// The formal identifier, or the first `arguments[index]` read.
    identifier: Span,
    /// The named formal, if the slot has one.
    symbol: Option<SymbolId>,
    /// Whether the slot is proven equivalent to the call argument. A formal
    /// that is written, defaulted or otherwise refused by
    /// [`ParameterSlots::formal_symbol_slot`] stays listed but takes no role.
    slot: bool,
}

#[derive(Clone)]
struct FunctionSite {
    node: oxc_semantic::NodeId,
    span: Span,
    body: Option<Span>,
    symbol: Option<SymbolId>,
    name_label: Option<String>,
    params: Vec<Parameter>,
}

struct FunctionCollector<'s, 'a> {
    slots: &'s ParameterSlots<'a, 'a>,
    functions: Vec<FunctionSite>,
    /// First `arguments[index]` read per owning function and slot index.
    argument_reads: HashMap<oxc_semantic::NodeId, BTreeMap<u32, Span>>,
}

impl FunctionCollector<'_, '_> {
    /// Adds each slot read only as `arguments[index]` to its function.
    fn finish(mut self) -> Vec<FunctionSite> {
        for site in &mut self.functions {
            let Some(reads) = self.argument_reads.get(&site.node) else {
                continue;
            };
            for (index, span) in reads {
                let index = *index as usize;
                match site
                    .params
                    .iter_mut()
                    .find(|parameter| parameter.index == index)
                {
                    Some(parameter) => parameter.slot = true,
                    None => site.params.push(Parameter {
                        index,
                        identifier: *span,
                        symbol: None,
                        slot: true,
                    }),
                }
            }
            site.params.sort_by_key(|parameter| parameter.index);
        }
        self.functions
    }
}

type SpanKey = (u32, u32);

struct FunctionScopeIndex<'a> {
    functions: &'a [FunctionSite],
    owner_by_span: rustc_hash::FxHashMap<SpanKey, Option<SpanKey>>,
    /// Node ids (in node order) per owner: exactly the nodes for which
    /// [`direct_node`] holds, so a scan of one function's direct nodes need
    /// not visit the whole program.
    direct_by_owner: rustc_hash::FxHashMap<SpanKey, Vec<oxc_semantic::NodeId>>,
    slots: &'a ParameterSlots<'a, 'a>,
}

impl<'a> FunctionScopeIndex<'a> {
    fn new(
        nodes: &oxc_semantic::AstNodes<'_>,
        functions: &'a [FunctionSite],
        slots: &'a ParameterSlots<'a, 'a>,
    ) -> Self {
        let mut owner_by_span: rustc_hash::FxHashMap<SpanKey, Option<SpanKey>> =
            rustc_hash::FxHashMap::with_capacity_and_hasher(nodes.len(), Default::default());
        // Nearest enclosing function (the node itself when it is one), by
        // node id. A parent is created before its children, so one pass in
        // id order sees every parent first; otherwise walk the ancestors.
        let mut enclosing: Vec<Option<Span>> = Vec::with_capacity(nodes.len());
        for (node_id, node) in nodes.iter_enumerated() {
            let owner = match node.kind() {
                AstKind::Function(function) => Some(function.span),
                _ => {
                    let parent = nodes.parent_id(node_id);
                    if parent != node_id && parent.index() < node_id.index() {
                        enclosing[parent.index()]
                    } else {
                        nodes.ancestors(node_id).find_map(|ancestor| {
                            if let AstKind::Function(function) = ancestor.kind() {
                                Some(function.span)
                            } else {
                                None
                            }
                        })
                    }
                }
            };
            enclosing.push(owner);
            owner_by_span
                .entry((node.span().start, node.span().end))
                .or_insert_with(|| owner.map(|span| (span.start, span.end)));
        }
        let mut direct_by_owner: rustc_hash::FxHashMap<SpanKey, Vec<oxc_semantic::NodeId>> =
            rustc_hash::FxHashMap::default();
        for (node_id, node) in nodes.iter_enumerated() {
            if let Some(Some(owner)) = owner_by_span.get(&(node.span().start, node.span().end)) {
                direct_by_owner.entry(*owner).or_default().push(node_id);
            }
        }
        Self {
            functions,
            owner_by_span,
            direct_by_owner,
            slots,
        }
    }

    /// The nodes directly owned by `site` (see [`direct_node`]), in node order.
    fn direct<'n, 'b>(
        &'n self,
        site: &FunctionSite,
        nodes: &'n oxc_semantic::AstNodes<'b>,
    ) -> impl Iterator<Item = &'n oxc_semantic::AstNode<'b>> + 'n {
        self.direct_by_owner
            .get(&(site.span.start, site.span.end))
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .map(move |id| nodes.get_node(*id))
    }

    /// The slot index when `expression` denotes an argument slot of `site`
    /// itself, never of an inner or outer function.
    fn slot(&self, expression: &Expression<'_>, site: &FunctionSite) -> Option<usize> {
        self.slots
            .parameter_slot(expression)
            .filter(|slot| slot.owner == site.node)
            .map(|slot| slot.index as usize)
    }

    /// Whether `expression` reads slot `index` of `site` anywhere inside it.
    fn mentions_slot(
        &self,
        expression: &Expression<'_>,
        site: &FunctionSite,
        index: usize,
    ) -> bool {
        self.mentioned_slots(expression, site).contains(&index)
    }

    /// The slot of `site` a local read holds at its own position: its single
    /// reaching definition (see [`super::reaching`]) stores exactly that
    /// argument slot. A coalesced local (`v = arguments[1]` here, `v =
    /// -arguments[1]` elsewhere) is answered per read, never by the binding.
    fn slot_at_use(
        &self,
        scoping: &Scoping,
        nodes: &oxc_semantic::AstNodes<'_>,
        expression: &Expression<'_>,
        site: &FunctionSite,
    ) -> Option<usize> {
        let definition = expression_value_at_use(scoping, nodes, expression).ok()?;
        let slot = self.slots.parameter_slot(definition.value)?;
        (slot.owner == site.node).then_some(slot.index as usize)
    }

    /// Every slot of `site` that `expression` reads anywhere inside it.
    fn mentioned_slots(&self, expression: &Expression<'_>, site: &FunctionSite) -> BTreeSet<usize> {
        struct Find<'i, 'a> {
            index: &'i FunctionScopeIndex<'a>,
            owner: oxc_semantic::NodeId,
            found: BTreeSet<usize>,
        }
        impl<'b> Visit<'b> for Find<'_, '_> {
            fn visit_expression(&mut self, expression: &Expression<'b>) {
                if let Some(slot) = self.index.slots.parameter_slot(expression) {
                    if slot.owner == self.owner {
                        self.found.insert(slot.index as usize);
                    }
                }
                walk::walk_expression(self, expression);
            }
        }
        let mut find = Find {
            index: self,
            owner: site.node,
            found: BTreeSet::new(),
        };
        find.visit_expression(expression);
        find.found
    }

    /// The argument slot a call's callee holds at `at`: the callee itself, or
    /// the value its alias chain resolves to (`let m = mask; m()` and
    /// `m = arguments[0]; m()` alike).
    fn call_slot(
        &self,
        callee: &Expression<'_>,
        aliases: &AliasHistory,
        at: u32,
        scoping: &Scoping,
    ) -> Option<ParameterSlot> {
        if let Some(slot) = self.slots.parameter_slot(callee) {
            return Some(slot);
        }
        match resolve_alias(callee, aliases, at, scoping)? {
            AliasValue::Symbol(symbol) => self.slots.formal_symbol_slot(symbol),
            AliasValue::Slot(slot) => Some(slot),
        }
    }
}

impl Deref for FunctionScopeIndex<'_> {
    type Target = [FunctionSite];

    fn deref(&self) -> &Self::Target {
        self.functions
    }
}

impl<'a> Visit<'a> for FunctionCollector<'_, '_> {
    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        let params = function
            .params
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, parameter)| {
                let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
                    return None;
                };
                let symbol = identifier.symbol_id.get()?;
                Some(Parameter {
                    index,
                    identifier: identifier.span,
                    symbol: Some(symbol),
                    slot: self.slots.formal_symbol_slot(symbol).is_some(),
                })
            })
            .collect();
        self.functions.push(FunctionSite {
            node: function.node_id.get(),
            span: function.span,
            body: function.body.as_ref().map(|body| body.span),
            symbol: function.id.as_ref().and_then(|id| id.symbol_id.get()),
            name_label: function.id.as_ref().map(|id| id.name.to_string()),
            params,
        });
        walk::walk_function(self, function, flags);
    }

    fn visit_expression(&mut self, expression: &Expression<'a>) {
        if matches!(expression, Expression::ComputedMemberExpression(_)) {
            if let Some(slot) = self.slots.parameter_slot(expression) {
                self.argument_reads
                    .entry(slot.owner)
                    .or_default()
                    .entry(slot.index)
                    .or_insert(expression.span());
            }
        }
        walk::walk_expression(self, expression);
    }
}

fn source(source: &str, span: Span) -> String {
    source
        .get(span.start as usize..span.end as usize)
        .unwrap_or_default()
        .to_string()
}

fn evidence(source_text: &str, span: Span, detail: impl Into<String>) -> Evidence {
    Evidence {
        span: span.into(),
        source: source(source_text, span),
        detail: detail.into(),
    }
}

fn binding(
    scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
    symbol: SymbolId,
) -> BindingEvidence {
    let declaration = nodes.get_node(scoping.symbol_declaration(symbol)).span();
    BindingEvidence {
        symbol: symbol.index() as u32,
        name: scoping.symbol_name(symbol).to_string(),
        declaration: declaration.into(),
        identifier: scoping.symbol_span(symbol).into(),
    }
}

fn function_evidence(
    source_text: &str,
    scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
    site: FunctionSite,
) -> FunctionEvidence {
    FunctionEvidence {
        binding: site.symbol.map(|symbol| binding(scoping, nodes, symbol)),
        span: site.span.into(),
        body: site.body.map(Into::into),
        name_label: site.name_label,
        source: source(source_text, site.span),
    }
}

fn contains_symbol(expression: &Expression<'_>, symbol: SymbolId, scoping: &Scoping) -> bool {
    struct Find<'s> {
        symbol: SymbolId,
        scoping: &'s Scoping,
        found: bool,
    }
    impl<'s, 'a> Visit<'a> for Find<'s> {
        fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
            if crate::analysis::identifier_symbol(id, self.scoping) == Some(self.symbol) {
                self.found = true;
            }
        }
    }
    let mut find = Find {
        symbol,
        scoping,
        found: false,
    };
    find.visit_expression(expression);
    find.found
}

fn expression_symbol(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    match expression {
        Expression::Identifier(identifier) => {
            crate::analysis::identifier_symbol(identifier, scoping)
        }
        _ => None,
    }
}

fn numeric_literal(expression: &Expression<'_>) -> Option<f64> {
    match expression.without_parentheses() {
        Expression::NumericLiteral(number) => Some(number.value),
        Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::UnaryNegation => {
            numeric_literal(&unary.argument).map(|value| -value)
        }
        _ => None,
    }
}

fn comparison_operator(operator: BinaryOperator) -> bool {
    matches!(
        operator,
        BinaryOperator::LessThan
            | BinaryOperator::LessEqualThan
            | BinaryOperator::GreaterThan
            | BinaryOperator::GreaterEqualThan
            | BinaryOperator::StrictEquality
            | BinaryOperator::StrictInequality
            | BinaryOperator::Equality
            | BinaryOperator::Inequality
    )
}

/// The slot of `site` that a computed member assignment stores into.
fn computed_buffer_write(
    assignment: &AssignmentExpression<'_>,
    site: &FunctionSite,
    functions: &FunctionScopeIndex<'_>,
    scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
) -> Option<usize> {
    let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left else {
        return None;
    };
    identity_buffer_slot(&member.object, site, functions, scoping, nodes, &mut BTreeSet::new())
}

/// Resolve an output-buffer alias only when every definition preserves the
/// same parameter object. Conditional/sequence expressions may change other
/// locals, but neither selected value may replace the buffer.
fn identity_buffer_slot(
    expression: &Expression<'_>, site: &FunctionSite,
    functions: &FunctionScopeIndex<'_>, scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>, seen: &mut BTreeSet<SymbolId>,
) -> Option<usize> {
    if let Some(slot) = functions.slot(expression, site) { return Some(slot); }
    match expression.without_parentheses() {
        Expression::ConditionalExpression(value) => {
            let left = identity_buffer_slot(&value.consequent, site, functions, scoping, nodes, &mut seen.clone())?;
            let right = identity_buffer_slot(&value.alternate, site, functions, scoping, nodes, &mut seen.clone())?;
            (left == right).then_some(left)
        }
        Expression::SequenceExpression(value) => identity_buffer_slot(value.expressions.last()?, site, functions, scoping, nodes, seen),
        Expression::Identifier(id) => {
            let symbol = crate::analysis::identifier_symbol(id, scoping)?;
            if !seen.insert(symbol) { return None; }
            let mut values = Vec::new();
            if let AstKind::VariableDeclarator(declaration) = nodes.kind(scoping.symbol_declaration(symbol)) {
                if let Some(value) = &declaration.init { values.push(value); }
            } else { return None; }
            for reference in scoping.get_resolved_references(symbol).filter(|r| r.is_write()) {
                let AstKind::AssignmentExpression(assignment) = nodes.parent_kind(reference.node_id()) else { return None; };
                if assignment.operator != AssignmentOperator::Assign || !node_in(site, assignment.span) { return None; }
                values.push(&assignment.right);
            }
            let slots: BTreeSet<_> = values.into_iter().map(|value| identity_buffer_slot(value, site, functions, scoping, nodes, &mut seen.clone())).collect();
            if slots.len() == 1 { slots.into_iter().next().flatten() } else { None }
        }
        _ => None,
    }
}

/// The member name of `object.name` or `object["name"]`.
fn member_name<'e>(expression: &'e Expression<'_>) -> Option<&'e str> {
    match expression.without_parentheses() {
        Expression::StaticMemberExpression(member) => Some(member.property.name.as_str()),
        Expression::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(name) => Some(name.value.as_str()),
            _ => None,
        },
        _ => None,
    }
}

/// The object of `object.name` or `object["name"]`.
fn member_object<'e, 'a>(expression: &'e Expression<'a>) -> Option<&'e Expression<'a>> {
    match expression.without_parentheses() {
        Expression::StaticMemberExpression(member) => Some(&member.object),
        Expression::ComputedMemberExpression(member)
            if matches!(member.expression, Expression::StringLiteral(_)) =>
        {
            Some(&member.object)
        }
        _ => None,
    }
}

/// A node that reads member `name` of some object, in either spelling.
fn member_node<'a>(kind: AstKind<'a>, name: &str) -> Option<&'a Expression<'a>> {
    match kind {
        AstKind::StaticMemberExpression(member) if member.property.name == name => {
            Some(&member.object)
        }
        AstKind::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(key) if key.value == name => Some(&member.object),
            _ => None,
        },
        _ => None,
    }
}

/// The callee and arguments of a call, reading `f.call(void 0, ...rest)` as
/// `f(...rest)`: traces spell a receiverless call that way.
fn plain_call<'c, 'a>(call: &'c CallExpression<'a>) -> (&'c Expression<'a>, &'c [Argument<'a>]) {
    if member_name(&call.callee) == Some("call") {
        if let (Some(object), Some(receiver)) =
            (member_object(&call.callee), call.arguments.first())
        {
            if receiver.as_expression().is_some_and(is_void_literal) {
                return (object, &call.arguments[1..]);
            }
        }
    }
    (&call.callee, &call.arguments)
}

fn is_void_literal(expression: &Expression<'_>) -> bool {
    matches!(expression.without_parentheses(), Expression::UnaryExpression(unary)
        if unary.operator == UnaryOperator::Void
            && matches!(unary.argument.without_parentheses(), Expression::NumericLiteral(_)))
}

fn call_target(
    expression: &Expression<'_>,
    aliases: &AliasHistory,
    at: u32,
    scoping: &Scoping,
) -> Option<SymbolId> {
    match resolve_alias(expression, aliases, at, scoping)? {
        AliasValue::Symbol(symbol) => Some(symbol),
        AliasValue::Slot(_) => None,
    }
}

/// What an identifier holds at source position `at`, following plain
/// unconditional alias assignments. `None` when a link is conditional, not
/// a plain alias, or not uniquely ordered.
fn resolve_alias(
    expression: &Expression<'_>,
    aliases: &AliasHistory,
    at: u32,
    scoping: &Scoping,
) -> Option<AliasValue> {
    let mut symbol = expression_symbol(expression, scoping)?;
    let mut seen = BTreeSet::new();
    while let Some(events) = aliases.get(&symbol) {
        if !seen.insert(symbol) {
            return None;
        }
        let prior = events.iter().rev().find(|event| event.at < at);
        // A read before the binding's store (a hoisted helper alias called
        // from a function defined earlier) takes the one later store. Stores
        // inside a conditional arm (`if (!buf) w = w;`, junk the obfuscated
        // programs keep under opaque tests) do not count when exactly one
        // later store is unconditional.
        let later = || {
            let future: Vec<_> = events.iter().filter(|event| event.at >= at).collect();
            if future.len() == 1 {
                return Some(future[0]);
            }
            match future.iter().filter(|event| !event.conditional).collect::<Vec<_>>()[..] {
                [only] => Some(*only),
                _ => None,
            }
        };
        let event = match prior {
            Some(event) if event.value.is_some() || !event.declaration => event,
            Some(_event) => match later() {
                Some(event) if !event.declaration => event,
                _ => return None,
            },
            None => later()?,
        };
        if event.conditional {
            return None;
        }
        match event.value? {
            AliasValue::Symbol(target) => symbol = target,
            slot @ AliasValue::Slot(_) => return Some(slot),
        }
    }
    Some(AliasValue::Symbol(symbol))
}

/// The value an alias event stores: another binding, or an `arguments[N]`
/// slot read.
#[derive(Clone, Copy)]
enum AliasValue {
    Symbol(SymbolId),
    Slot(ParameterSlot),
}

#[derive(Clone, Copy)]
struct AliasEvent {
    at: u32,
    value: Option<AliasValue>,
    conditional: bool,
    declaration: bool,
}

type AliasHistory = HashMap<SymbolId, Vec<AliasEvent>>;

fn build_aliases(
    program: &Program<'_>,
    scoping: &Scoping,
    slots: &ParameterSlots<'_, '_>,
) -> AliasHistory {
    fn record_alias(
        values: &mut AliasHistory,
        symbol: SymbolId,
        value: Option<AliasValue>,
        at: u32,
        conditional: bool,
        declaration: bool,
    ) {
        values.entry(symbol).or_default().push(AliasEvent {
            at,
            value,
            conditional,
            declaration,
        });
    }
    struct Aliases<'s, 'p> {
        scoping: &'s Scoping,
        slots: &'s ParameterSlots<'p, 'p>,
        values: AliasHistory,
        conditional_depth: usize,
    }
    impl Aliases<'_, '_> {
        /// A named binding stays a symbol link (resolved to its own slot, if
        /// any, at the end of the chain); `arguments[N]` is a slot value.
        fn alias_value(&self, value: &Expression<'_>) -> Option<AliasValue> {
            match value {
                Expression::Identifier(right) => {
                    crate::analysis::identifier_symbol(right, self.scoping).map(AliasValue::Symbol)
                }
                Expression::ComputedMemberExpression(_) => {
                    self.slots.parameter_slot(value).map(AliasValue::Slot)
                }
                _ => None,
            }
        }
    }
    impl<'a> Visit<'a> for Aliases<'_, '_> {
        fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
            if let BindingPattern::BindingIdentifier(left) = &declarator.id {
                if let Some(symbol) = left.symbol_id.get() {
                    let target = declarator
                        .init
                        .as_ref()
                        .and_then(|init| self.alias_value(init));
                    record_alias(
                        &mut self.values,
                        symbol,
                        target,
                        declarator.span.start,
                        self.conditional_depth > 0,
                        true,
                    );
                }
            }
            walk::walk_variable_declarator(self, declarator);
        }
        fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
            if let AssignmentTarget::AssignmentTargetIdentifier(left) = &assignment.left {
                if let Some(symbol) = crate::analysis::identifier_symbol(left, self.scoping) {
                    let target = (assignment.operator == AssignmentOperator::Assign)
                        .then(|| self.alias_value(&assignment.right))
                        .flatten();
                    record_alias(
                        &mut self.values,
                        symbol,
                        target,
                        assignment.span.start,
                        self.conditional_depth > 0,
                        false,
                    );
                }
            }
            walk::walk_assignment_expression(self, assignment);
        }
        fn visit_update_expression(&mut self, update: &UpdateExpression<'a>) {
            if let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) = &update.argument
            {
                if let Some(symbol) = crate::analysis::identifier_symbol(identifier, self.scoping) {
                    record_alias(
                        &mut self.values,
                        symbol,
                        None,
                        update.span.start,
                        self.conditional_depth > 0,
                        false,
                    );
                }
            }
            walk::walk_update_expression(self, update);
        }
        fn visit_if_statement(&mut self, statement: &IfStatement<'a>) {
            self.conditional_depth += 1;
            walk::walk_if_statement(self, statement);
            self.conditional_depth -= 1;
        }
        fn visit_for_statement(&mut self, statement: &ForStatement<'a>) {
            self.conditional_depth += 1;
            walk::walk_for_statement(self, statement);
            self.conditional_depth -= 1;
        }
        fn visit_while_statement(&mut self, statement: &WhileStatement<'a>) {
            self.conditional_depth += 1;
            walk::walk_while_statement(self, statement);
            self.conditional_depth -= 1;
        }
        fn visit_do_while_statement(&mut self, statement: &DoWhileStatement<'a>) {
            self.conditional_depth += 1;
            walk::walk_do_while_statement(self, statement);
            self.conditional_depth -= 1;
        }
        fn visit_switch_statement(&mut self, statement: &SwitchStatement<'a>) {
            self.conditional_depth += 1;
            walk::walk_switch_statement(self, statement);
            self.conditional_depth -= 1;
        }
        fn visit_try_statement(&mut self, statement: &TryStatement<'a>) {
            self.conditional_depth += 1;
            walk::walk_try_statement(self, statement);
            self.conditional_depth -= 1;
        }
    }
    let mut aliases = Aliases {
        scoping,
        slots,
        values: HashMap::new(),
        conditional_depth: 0,
    };
    aliases.visit_program(program);
    aliases.values
}

fn returned_nested(
    outer: &FunctionSite,
    index: &FunctionScopeIndex<'_>,
    nodes: &oxc_semantic::AstNodes<'_>,
    scoping: &Scoping,
) -> Vec<FunctionSite> {
    index
        .functions
        .iter()
        .filter(|inner| {
            inner.span != outer.span
                && inner.span.start >= outer.span.start
                && inner.span.end <= outer.span.end
                && outer_returns(outer, inner, nodes, scoping, index)
        })
        .cloned()
        .collect()
}

fn outer_returns(
    outer: &FunctionSite,
    inner: &FunctionSite,
    nodes: &oxc_semantic::AstNodes<'_>,
    scoping: &Scoping,
    index: &FunctionScopeIndex<'_>,
) -> bool {
    index.direct(outer, nodes).any(|node| {
        if !direct_node(outer, node.span(), index) {
            return false;
        }
        let AstKind::ReturnStatement(statement) = node.kind() else {
            return false;
        };
        statement
            .argument
            .as_ref()
            .and_then(|argument| expression_symbol(argument, scoping))
            == inner.symbol
    })
}

/// The object a STRING writer reads `.length` / `.charCodeAt` from.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ValueSource {
    Slot(usize),
    Symbol(SymbolId),
}

fn node_in(site: &FunctionSite, span: Span) -> bool {
    span.start >= site.span.start && span.end <= site.span.end
}

fn direct_node(site: &FunctionSite, span: Span, index: &FunctionScopeIndex<'_>) -> bool {
    let key = (span.start, span.end);
    if let Some(owner) = index.owner_by_span.get(&key) {
        return *owner == Some((site.span.start, site.span.end));
    }
    node_in(site, span)
        && !index.functions.iter().any(|child| {
            child.span != site.span
                && child.span.start >= site.span.start
                && child.span.end <= site.span.end
                && child.span.start <= span.start
                && child.span.end >= span.end
        })
}

fn number_candidates(
    scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
    functions: &FunctionScopeIndex<'_>,
) -> Vec<FunctionSite> {
    let mut candidates = Vec::new();
    for site in functions.iter() {
        let writes: Vec<&AssignmentExpression<'_>> = functions.direct(site, nodes)
            .filter_map(|node| {
                if !direct_node(site, node.span(), functions) {
                    return None;
                }
                let AstKind::AssignmentExpression(assignment) = node.kind() else {
                    return None;
                };
                computed_buffer_write(assignment, site, functions, scoping, nodes).map(|_| assignment)
            })
            .collect();
        if writes.len() < 2 {
            continue;
        }
        let integer_tests = functions.direct(site, nodes)
            .filter_map(|node| {
                if !direct_node(site, node.span(), functions) {
                    return None;
                }
                let AstKind::BinaryExpression(binary) = node.kind() else {
                    return None;
                };
                if binary.operator != BinaryOperator::StrictEquality
                    && binary.operator != BinaryOperator::Equality
                {
                    return None;
                }
                (!functions.mentioned_slots(&binary.left, site).is_empty()
                    || !functions.mentioned_slots(&binary.right, site).is_empty())
                .then_some(binary)
            })
            .count();
        let sign_tests = functions.direct(site, nodes)
            .filter(|node| {
                if !direct_node(site, node.span(), functions) {
                    return false;
                }
                let AstKind::BinaryExpression(binary) = node.kind() else {
                    return false;
                };
                comparison_operator(binary.operator)
                    && (!functions.mentioned_slots(&binary.left, site).is_empty()
                        || !functions.mentioned_slots(&binary.right, site).is_empty())
            })
            .count();
        let calls = functions.direct(site, nodes)
            .filter(|node| {
                let AstKind::CallExpression(call) = node.kind() else {
                    return false;
                };
                direct_node(site, node.span(), functions) && call.arguments.is_empty()
            })
            .count();
        if integer_tests == 0 || sign_tests < 2 {
            continue;
        }
        let _ = calls;
        candidates.push(site.clone());
    }
    candidates
}

/// Every admissible NUMBER writer, canonical order.
///
/// A writer that a proven STRING factory calls is stronger evidence than a
/// bare shape match, so linked candidates win as a group. When a capture
/// carries several thread-local copies, each copy is linked through the STRING
/// factory of its own thread, so the whole group is admitted: writes are typed
/// per resolved callee binding, never by picking one copy for the whole file.
fn select_number_candidates(
    candidates: &[FunctionSite],
    linked: &[FunctionSite],
) -> Vec<FunctionSite> {
    if !linked.is_empty() {
        return dedup_sites(linked);
    }
    if candidates.len() == 1 {
        return vec![candidates[0].clone()];
    }
    Vec::new()
}

/// The copies of a role group after the canonical (first, source-ordered) one.
fn equivalent_spans(sites: &[FunctionSite]) -> Vec<SourceSpan> {
    sites
        .iter()
        .skip(1)
        .map(|site| SourceSpan::new(site.span.start, site.span.end))
        .collect()
}

/// Remove repeated spans while keeping source order.
fn dedup_sites(sites: &[FunctionSite]) -> Vec<FunctionSite> {
    let mut seen = std::collections::BTreeSet::new();
    let mut unique = Vec::new();
    for site in sites {
        if seen.insert((site.span.start, site.span.end)) {
            unique.push(site.clone());
        }
    }
    unique
}

fn number_properties(
    source_text: &str,
    scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
    site: FunctionSite,
    functions: &FunctionScopeIndex<'_>,
) -> NumberWriterProperties {
    let mut buffer_slots = BTreeSet::new();
    let mut writes = Vec::new();
    let mut shifts = Vec::new();
    let mut continuation_masks = Vec::new();
    let mut branches = Vec::new();
    for node in functions.direct(&site, nodes) {
        if !direct_node(&site, node.span(), functions) {
            continue;
        }
        match node.kind() {
            AstKind::AssignmentExpression(assignment) => {
                if let Some(slot) = computed_buffer_write(assignment, &site, functions, scoping, nodes) {
                    buffer_slots.insert(slot);
                    writes.push(assignment);
                }
            }
            AstKind::BinaryExpression(binary) => {
                if matches!(
                    binary.operator,
                    BinaryOperator::ShiftLeft
                        | BinaryOperator::ShiftRight
                        | BinaryOperator::ShiftRightZeroFill
                ) {
                    shifts.push(evidence(source_text, binary.span, "NUMBER shift"));
                }
                if matches!(
                    binary.operator,
                    BinaryOperator::BitwiseOR | BinaryOperator::BitwiseAnd
                ) && (numeric_literal(&binary.left).is_some()
                    || numeric_literal(&binary.right).is_some())
                {
                    continuation_masks.push(evidence(
                        source_text,
                        binary.span,
                        "NUMBER continuation/sign mask",
                    ));
                }
                if comparison_operator(binary.operator)
                    && (numeric_literal(&binary.left).is_some()
                        || numeric_literal(&binary.right).is_some())
                {
                    branches.push(NumberBranch {
                        kind: "threshold_or_sign_test".to_string(),
                        evidence: vec![evidence(source_text, binary.span, "source comparison")],
                    });
                }
            }
            _ => {}
        }
    }
    let equality_slots: BTreeSet<usize> = functions.direct(&site, nodes)
        .filter_map(|node| {
            let AstKind::BinaryExpression(binary) = node.kind() else {
                return None;
            };
            (direct_node(&site, node.span(), functions)
                && (binary.operator == BinaryOperator::StrictEquality
                    || binary.operator == BinaryOperator::Equality))
                .then(|| functions.mentioned_slots(&binary.left, &site))
        })
        .flatten()
        .collect();
    let value_parameter = equality_slots.iter().next().copied();
    let buffer_parameter = buffer_slots.iter().next().copied();
    let mask_parameters: BTreeSet<usize> = functions.direct(&site, nodes)
        .filter_map(|node| {
            let AstKind::CallExpression(call) = node.kind() else {
                return None;
            };
            if !direct_node(&site, node.span(), functions) {
                return None;
            }
            functions.slot(plain_call(call).0, &site)
        })
        .collect();
    let offset_parameter = functions.direct(&site, nodes)
        .filter_map(|node| {
            let AstKind::AssignmentExpression(assignment) = node.kind() else {
                return None;
            };
            let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left else {
                return None;
            };
            direct_node(&site, node.span(), functions)
                .then(|| functions.mentioned_slots(&member.expression, &site))
        })
        .flatten()
        .min();
    let mut parameters = Vec::new();
    for parameter in &site.params {
        let slot = Some(parameter.index).filter(|_| parameter.slot);
        let (role, detail) = if slot.is_none() {
            ("unknown", "formal is not equivalent to its argument slot")
        } else if slot == value_parameter {
            ("value", "integer/sign/threshold dataflow")
        } else if slot == buffer_parameter {
            ("buffer", "computed byte-store target")
        } else if mask_parameters.contains(&parameter.index) {
            ("mask", "called to produce masked byte")
        } else if slot == offset_parameter {
            ("offset", "computed byte index")
        } else {
            ("unknown", "formal parameter role not proven")
        };
        parameters.push(ParameterEvidence {
            index: parameter.index,
            binding: parameter
                .symbol
                .map(|symbol| binding(scoping, nodes, symbol)),
            role: role.to_string(),
            evidence: vec![evidence(source_text, parameter.identifier, detail)],
        });
    }
    let float_scratch = functions.direct(&site, nodes).find_map(|node| {
        if !direct_node(&site, node.span(), functions) {
            return None;
        }
        let text = source(source_text, node.span());
        if text.contains("ArrayBuffer")
            || text.contains("Math.pow")
            || text.contains("Math[\"pow\"]")
        {
            Some(evidence(
                source_text,
                node.span(),
                "float scratch or exponent/mantissa path",
            ))
        } else {
            None
        }
    });
    let fallback = functions.direct(&site, nodes).find_map(|node| {
        if !direct_node(&site, node.span(), functions) {
            return None;
        }
        let AstKind::BinaryExpression(binary) = node.kind() else {
            return None;
        };
        let value_parameter = value_parameter?;
        if binary.operator == BinaryOperator::StrictInequality
            && (functions.mentions_slot(&binary.left, &site, value_parameter)
                || functions.slot_at_use(scoping, nodes, &binary.left, &site)
                    == Some(value_parameter))
        {
            Some(evidence(
                source_text,
                binary.span,
                "NaN/fallback comparison",
            ))
        } else {
            None
        }
    });
    branches.push(NumberBranch {
        kind: "integer_path".to_string(),
        evidence: vec![],
    });
    branches.push(NumberBranch {
        kind: "float_path".to_string(),
        evidence: vec![float_scratch
            .clone()
            .unwrap_or_else(|| evidence(source_text, site.span, "float path not proven"))],
    });
    NumberWriterProperties {
        function: function_evidence(source_text, scoping, nodes, site),
        parameters,
        branches,
        buffer_write_count: writes.len(),
        continuation_masks,
        shifts,
        float_scratch,
        fallback,
    }
}

fn object_create_null(call: &CallExpression<'_>) -> bool {
    member_name(&call.callee) == Some("create")
        && matches!(member_object(&call.callee), Some(Expression::Identifier(identifier)) if identifier.name == "Object")
        && call
            .arguments
            .first()
            .and_then(|argument| argument.as_expression())
            .is_some_and(|argument| matches!(argument, Expression::NullLiteral(_)))
}

fn string_candidates(
    scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
    functions: &FunctionScopeIndex<'_>,
    numbers: &[FunctionSite],
    aliases: &AliasHistory,
) -> Vec<FunctionSite> {
    let mut candidates = Vec::new();
    for outer in functions.iter() {
        let nested = returned_nested(outer, functions, nodes, scoping);
        if nested.len() != 1 {
            continue;
        }
        let writer = &nested[0];
        let has_cache = functions.direct(outer, nodes).any(|node| {
            let AstKind::CallExpression(call) = node.kind() else {
                return false;
            };
            direct_node(outer, node.span(), functions) && object_create_null(call)
        });
        if !has_cache {
            continue;
        }
        let has_length = functions.direct(writer, nodes).any(|node| {
            direct_node(writer, node.span(), functions)
                && member_node(node.kind(), "length").is_some()
        });
        let has_cache_test = functions.direct(writer, nodes).any(|node| {
            let AstKind::BinaryExpression(binary) = node.kind() else {
                return false;
            };
            direct_node(writer, node.span(), functions)
                && matches!(
                    binary.operator,
                    BinaryOperator::Equality
                        | BinaryOperator::StrictEquality
                        | BinaryOperator::Inequality
                        | BinaryOperator::StrictInequality
                )
                && (contains_member_expression(&binary.left)
                    || contains_member_expression(&binary.right))
        });
        if has_length && has_cache_test {
            let linked_numbers = numbers
                .iter()
                .filter(|number| {
                    let Some(target) = number.symbol else {
                        return false;
                    };
                    functions.direct(writer, nodes).any(|node| {
                        let AstKind::CallExpression(call) = node.kind() else {
                            return false;
                        };
                        direct_node(writer, node.span(), functions)
                            && call_target(&call.callee, aliases, node.span().start, scoping)
                                == Some(target)
                    })
                })
                .count();
            candidates.push((outer.clone(), linked_numbers));
        }
    }
    if candidates.len() == 1 {
        return vec![candidates[0].0.clone()];
    }
    // A factory that calls exactly one NUMBER writer is a proven writer; when a
    // capture repeats it per thread, every copy has that proof of its own.
    let linked: Vec<FunctionSite> = candidates
        .iter()
        .filter(|(_, linked_numbers)| *linked_numbers == 1)
        .map(|(site, _)| site.clone())
        .collect();
    if !linked.is_empty() {
        return dedup_sites(&linked);
    }
    candidates.into_iter().map(|(site, _)| site).collect()
}

fn linked_number_candidates(
    factory: &FunctionSite,
    numbers: &[FunctionSite],
    scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
    aliases: &AliasHistory,
    functions: &FunctionScopeIndex<'_>,
) -> Vec<FunctionSite> {
    let nested = returned_nested(factory, functions, nodes, scoping);
    let Some(writer) = (nested.len() == 1).then(|| nested[0].clone()) else {
        return Vec::new();
    };
    numbers
        .iter()
        .filter(|number| {
            let Some(target) = number.symbol else {
                return false;
            };
            functions.direct(&writer, nodes).any(|node| {
                let AstKind::CallExpression(call) = node.kind() else {
                    return false;
                };
                direct_node(&writer, node.span(), functions)
                    && call_target(&call.callee, aliases, node.span().start, scoping)
                        == Some(target)
            })
        })
        .cloned()
        .collect()
}

fn contains_member_expression(expression: &Expression<'_>) -> bool {
    struct Find {
        found: bool,
    }
    impl<'a> Visit<'a> for Find {
        fn visit_computed_member_expression(&mut self, _member: &ComputedMemberExpression<'a>) {
            self.found = true;
        }
        fn visit_static_member_expression(&mut self, _member: &StaticMemberExpression<'a>) {
            self.found = true;
        }
    }
    let mut find = Find { found: false };
    find.visit_expression(expression);
    find.found
}

fn first_object_create_binding(
    scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
    site: FunctionSite,
    writer: FunctionSite,
    functions: &FunctionScopeIndex<'_>,
) -> Option<BindingEvidence> {
    let created: BTreeSet<SymbolId> = functions.direct(&site, nodes)
        .filter_map(|node| {
            if !direct_node(&site, node.span(), functions) {
                return None;
            }
            match node.kind() {
                AstKind::VariableDeclarator(declarator) => {
                    let Some(Expression::CallExpression(call)) = declarator.init.as_ref() else {
                        return None;
                    };
                    if !object_create_null(call) {
                        return None;
                    }
                    let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                        return None;
                    };
                    Some(identifier.symbol_id.get()?)
                }
                AstKind::AssignmentExpression(assignment) => {
                    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left
                    else {
                        return None;
                    };
                    let Expression::CallExpression(call) = assignment.right.without_parentheses()
                    else {
                        return None;
                    };
                    if !object_create_null(call) {
                        return None;
                    }
                    Some(crate::analysis::identifier_symbol(identifier, scoping)?)
                }
                _ => None,
            }
        })
        .collect();
    let used: BTreeSet<SymbolId> = functions.direct(&writer, nodes)
        .filter_map(|node| {
            if !direct_node(&writer, node.span(), functions) {
                return None;
            }
            let AstKind::ComputedMemberExpression(member) = node.kind() else {
                return None;
            };
            expression_symbol(&member.object, scoping)
        })
        .collect();
    let matches: Vec<_> = created.intersection(&used).copied().collect();
    (matches.len() == 1).then(|| binding(scoping, nodes, matches[0]))
}

fn first_zero_binding(
    scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
    site: FunctionSite,
    writer: FunctionSite,
    functions: &FunctionScopeIndex<'_>,
) -> Option<BindingEvidence> {
    let zeros: BTreeSet<SymbolId> = functions.direct(&site, nodes)
        .filter_map(|node| {
            if !direct_node(&site, node.span(), functions) {
                return None;
            }
            match node.kind() {
                AstKind::VariableDeclarator(declarator) => {
                    let init = declarator.init.as_ref()?;
                    if numeric_literal(init) != Some(0.0) {
                        return None;
                    }
                    let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                        return None;
                    };
                    Some(identifier.symbol_id.get()?)
                }
                AstKind::AssignmentExpression(assignment) => {
                    if numeric_literal(assignment.right.without_parentheses()) != Some(0.0) {
                        return None;
                    }
                    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left
                    else {
                        return None;
                    };
                    Some(crate::analysis::identifier_symbol(identifier, scoping)?)
                }
                _ => None,
            }
        })
        .collect();
    let used: Vec<_> = zeros
        .into_iter()
        .filter(|symbol| {
            scoping
                .get_resolved_reference_ids(*symbol)
                .iter()
                .any(|reference| {
                    let span = nodes
                        .get_node(scoping.get_reference(*reference).node_id())
                        .span();
                    direct_node(&writer, span, functions)
                })
        })
        .collect();
    (used.len() == 1).then(|| binding(scoping, nodes, used[0]))
}

fn string_properties(
    source_text: &str,
    scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
    site: FunctionSite,
    number: Option<FunctionSite>,
    aliases: &AliasHistory,
    functions: &FunctionScopeIndex<'_>,
) -> StringFactoryProperties {
    let nested = returned_nested(&site, functions, nodes, scoping);
    let writer = (nested.len() == 1).then(|| nested[0].clone());
    let cache = writer.as_ref().and_then(|writer| {
        first_object_create_binding(scoping, nodes, site.clone(), writer.clone(), functions)
    });
    let counter = writer.as_ref().and_then(|writer| {
        first_zero_binding(scoping, nodes, site.clone(), writer.clone(), functions)
    });
    let mut miss_traversal = Vec::new();
    let mut terminator = None;
    let mut terminator_is_literal_zero = false;
    let mut number_dependency = None;
    let writer_buffer = writer.as_ref().and_then(|writer| {
        let buffers: BTreeSet<_> = functions.direct(&writer, nodes)
            .filter_map(|node| {
                if !direct_node(&writer, node.span(), functions) {
                    return None;
                }
                let AstKind::AssignmentExpression(assignment) = node.kind() else {
                    return None;
                };
                computed_buffer_write(assignment, writer, functions, scoping, nodes)
            })
            .collect();
        (buffers.len() == 1)
            .then(|| buffers.iter().next().copied())
            .flatten()
    });
    let writer_value = writer.as_ref().and_then(|writer| {
        let values: BTreeSet<_> = functions.direct(&writer, nodes)
            .filter_map(|node| {
                if !direct_node(&writer, node.span(), functions) {
                    return None;
                }
                let object = match node.kind() {
                    AstKind::CallExpression(call)
                        if member_name(&call.callee) == Some("charCodeAt") =>
                    {
                        member_object(&call.callee)?
                    }
                    kind => member_node(kind, "length")?,
                };
                // Any other read object keeps the value role unproven.
                match functions.slot(object, writer) {
                    Some(slot) => Some(ValueSource::Slot(slot)),
                    None => expression_symbol(object, scoping).map(ValueSource::Symbol),
                }
            })
            .collect();
        (values.len() == 1)
            .then(|| values.iter().next().copied())
            .flatten()
    });
    let writer_offset = writer.as_ref().and_then(|writer| {
        let offsets: BTreeSet<_> = functions.direct(&writer, nodes)
            .filter_map(|node| {
                if !direct_node(&writer, node.span(), functions) {
                    return None;
                }
                let AstKind::AssignmentExpression(assignment) = node.kind() else {
                    return None;
                };
                let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left else {
                    return None;
                };
                functions
                    .mentioned_slots(&member.expression, writer)
                    .into_iter()
                    .next()
            })
            .collect();
        (offsets.len() == 1)
            .then(|| offsets.iter().next().copied())
            .flatten()
    });
    let mask_symbols: BTreeSet<_> = writer
        .as_ref()
        .map(|writer| {
            functions.direct(&writer, nodes)
                .filter_map(|node| {
                    if !direct_node(&writer, node.span(), functions) {
                        return None;
                    }
                    let AstKind::CallExpression(call) = node.kind() else {
                        return None;
                    };
                    let (callee, arguments) = plain_call(call);
                    if !arguments.is_empty() {
                        return None;
                    }
                    functions
                        .call_slot(callee, aliases, node.span().start, scoping)
                        .filter(|slot| slot.owner == site.node)
                        .map(|slot| slot.index)
                })
                .collect()
        })
        .unwrap_or_default();
    for node in writer
        .as_ref()
        .into_iter()
        .flat_map(|writer_site| functions.direct(writer_site, nodes))
    {
        let Some(writer_site) = writer.as_ref() else {
            continue;
        };
        if !direct_node(writer_site, node.span(), functions) {
            continue;
        }
        match node.kind() {
            AstKind::ForStatement(_)
            | AstKind::WhileStatement(_)
            | AstKind::DoWhileStatement(_)
            | AstKind::ForInStatement(_)
            | AstKind::ForOfStatement(_) => miss_traversal.push(evidence(
                source_text,
                node.span(),
                "STRING cache miss traversal",
            )),
            AstKind::CallExpression(call) => {
                if let Some(target_site) = number.as_ref() {
                    if let Some(number) = target_site.symbol {
                        if call_target(&call.callee, aliases, node.span().start, scoping)
                            == Some(number)
                        {
                            number_dependency = Some(function_evidence(
                                source_text,
                                scoping,
                                nodes,
                                target_site.clone(),
                            ));
                        }
                    }
                }
            }
            AstKind::AssignmentExpression(assignment) => {
                let Some(buffer) = writer_buffer else {
                    continue;
                };
                let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left else {
                    continue;
                };
                if functions.slot(&member.object, writer_site) != Some(buffer) {
                    continue;
                }
                let Expression::CallExpression(call) = assignment.right.without_parentheses()
                else {
                    continue;
                };
                let (callee, arguments) = plain_call(call);
                let Some(mask) = functions
                    .call_slot(callee, aliases, node.span().start, scoping)
                    .filter(|slot| slot.owner == site.node)
                else {
                    continue;
                };
                if !arguments.is_empty() || !mask_symbols.contains(&mask.index) {
                    continue;
                }
                if terminator.is_none() {
                    terminator = Some(evidence(
                        source_text,
                        assignment.right.span(),
                        "STRING terminator writes a stream byte",
                    ));
                    terminator_is_literal_zero =
                        numeric_literal(&assignment.right).is_some_and(|value| value == 0.0);
                }
            }
            _ => {}
        }
    }
    let params = writer
        .as_ref()
        .map(|writer| {
            writer
                .params
                .iter()
                .map(|parameter| ParameterEvidence {
                    index: parameter.index,
                    binding: parameter
                        .symbol
                        .map(|symbol| binding(scoping, nodes, symbol)),
                    role: if !parameter.slot {
                        "unknown"
                    } else if Some(ValueSource::Slot(parameter.index)) == writer_value {
                        "value"
                    } else if Some(parameter.index) == writer_buffer {
                        "buffer"
                    } else if Some(parameter.index) == writer_offset {
                        "offset"
                    } else {
                        "unknown"
                    }
                    .to_string(),
                    evidence: vec![evidence(
                        source_text,
                        parameter.identifier,
                        "STRING writer formal",
                    )],
                })
                .collect()
        })
        .unwrap_or_default();
    let mut unresolved = Vec::new();
    if cache.is_none() {
        unresolved.push(UnresolvedEncodingFact {
            fact: "string_cache".to_string(),
            reason: "cache object was not structurally located".to_string(),
            span: Some(site.span.into()),
            required: true,
        });
    }
    if terminator.is_none() {
        unresolved.push(UnresolvedEncodingFact {
            fact: "string_terminator".to_string(),
            reason: "stream-byte terminator write was not proven".to_string(),
            span: writer.as_ref().map(|value| value.span.into()),
            required: true,
        });
    }
    StringFactoryProperties {
        factory: function_evidence(source_text, scoping, nodes, site),
        writer: writer.map(|value| function_evidence(source_text, scoping, nodes, value)),
        parameters: params,
        cache,
        counter,
        miss_traversal,
        terminator,
        terminator_is_literal_zero,
        number_dependency,
        unresolved,
    }
}

fn stream_candidates(
    scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
    functions: &FunctionScopeIndex<'_>,
    aliases: &AliasHistory,
) -> Vec<FunctionSite> {
    let mut candidates = Vec::new();
    for outer in functions.iter() {
        let nested = returned_nested(outer, functions, nodes, scoping);
        if nested.len() != 1 {
            continue;
        }
        let inner = &nested[0];
        let has_cycle = functions.direct(inner, nodes).any(|node| direct_node(inner, node.span(), functions) && matches!(node.kind(), AstKind::BinaryExpression(binary) if binary.operator == BinaryOperator::Remainder && (numeric_literal(&binary.left).is_some() || numeric_literal(&binary.right).is_some())));
        let has_shift = functions.direct(inner, nodes).any(|node| direct_node(inner, node.span(), functions) && matches!(node.kind(), AstKind::BinaryExpression(binary) if matches!(binary.operator, BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight | BinaryOperator::ShiftRightZeroFill)));
        let warmup = functions.direct(outer, nodes)
            .filter(|node| {
                let AstKind::CallExpression(call) = node.kind() else {
                    return false;
                };
                direct_node(outer, node.span(), functions)
                    && expression_symbol(&call.callee, scoping) == inner.symbol
            })
            .count();
        if has_cycle && has_shift && warmup > 0 {
            candidates.push(outer.clone());
        } else {
            continue;
        }
    }
    if candidates.len() == 1 {
        return candidates;
    }
    // A factory that a mask call outside its own body consumes is proven; each
    // thread-local copy carries that proof for its own consumers.
    let consumed: Vec<FunctionSite> = candidates
        .iter()
        .filter(|candidate| {
            let Some(symbol) = candidate.symbol else {
                return false;
            };
            nodes.iter().any(|node| {
                let AstKind::CallExpression(call) = node.kind() else {
                    return false;
                };
                !node_in(candidate, node.span())
                    && call_target(&call.callee, aliases, node.span().start, scoping)
                        == Some(symbol)
            })
        })
        .cloned()
        .collect();
    if !consumed.is_empty() {
        return dedup_sites(&consumed);
    }
    candidates
}

fn stream_properties(
    source_text: &str,
    scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
    functions: &FunctionScopeIndex<'_>,
    site: FunctionSite,
) -> ByteStreamProperties {
    let cycle = returned_nested(&site, functions, nodes, scoping).into_iter().find(|inner| functions.direct(inner, nodes).any(|node| direct_node(inner, node.span(), functions) && matches!(node.kind(), AstKind::BinaryExpression(binary) if binary.operator == BinaryOperator::Remainder)));
    let mut cycle_modulus = None;
    let mut word_updates = Vec::new();
    let mut extraction_shifts = Vec::new();
    if let Some(inner) = cycle.as_ref() {
        for node in functions.direct(inner, nodes) {
            if !direct_node(inner, node.span(), functions) {
                continue;
            }
            match node.kind() {
                AstKind::BinaryExpression(binary)
                    if binary.operator == BinaryOperator::Remainder
                        && (numeric_literal(&binary.left).is_some()
                            || numeric_literal(&binary.right).is_some()) =>
                {
                    cycle_modulus = Some(evidence(source_text, binary.span, "byte cycle modulus"))
                }
                AstKind::AssignmentExpression(assignment)
                    if matches!(
                        assignment.right.without_parentheses(),
                        Expression::BinaryExpression(_)
                    ) =>
                {
                    word_updates.push(evidence(
                        source_text,
                        assignment.span,
                        "old/new word recurrence",
                    ))
                }
                AstKind::BinaryExpression(binary)
                    if matches!(
                        binary.operator,
                        BinaryOperator::ShiftLeft
                            | BinaryOperator::ShiftRight
                            | BinaryOperator::ShiftRightZeroFill
                    ) =>
                {
                    extraction_shifts.push(evidence(
                        source_text,
                        binary.span,
                        "stream byte extraction shift",
                    ))
                }
                _ => {}
            }
        }
    }
    let warmup_calls = cycle.as_ref().map(|inner| {
        functions.direct(&site, nodes)
            .filter(|node| {
                let AstKind::CallExpression(call) = node.kind() else {
                    return false;
                };
                direct_node(&site, node.span(), functions)
                    && expression_symbol(&call.callee, scoping) == inner.symbol
            })
            .count()
    });
    let captured_symbols: BTreeSet<SymbolId> = cycle
        .as_ref()
        .map(|inner| {
            functions.direct(&site, nodes)
                .filter_map(|node| {
                    let AstKind::VariableDeclarator(declarator) = node.kind() else {
                        return None;
                    };
                    if !direct_node(&site, node.span(), functions)
                        || node.span().start >= inner.span.start
                    {
                        return None;
                    }
                    let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                        return None;
                    };
                    let symbol = identifier.symbol_id.get()?;
                    let captured =
                        scoping
                            .get_resolved_reference_ids(symbol)
                            .iter()
                            .any(|reference| {
                                let span = nodes
                                    .get_node(scoping.get_reference(*reference).node_id())
                                    .span();
                                direct_node(inner, span, functions)
                            });
                    captured.then_some(symbol)
                })
                .collect()
        })
        .unwrap_or_default();
    let seed_initialization = site
        .params
        .iter()
        .filter(|parameter| parameter.slot)
        .find_map(|parameter| {
            functions.direct(&site, nodes)
                .find_map(|node| {
                    if !direct_node(&site, node.span(), functions) {
                        return None;
                    }
                    match node.kind() {
                        AstKind::AssignmentExpression(assignment) => {
                            let target = match &assignment.left {
                                AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                                    crate::analysis::identifier_symbol(identifier, scoping)
                                }
                                _ => None,
                            }?;
                            (captured_symbols.contains(&target)
                                && functions.mentions_slot(
                                    &assignment.right,
                                    &site,
                                    parameter.index,
                                ))
                            .then(|| {
                                evidence(
                                    source_text,
                                    assignment.span,
                                    "local state initialized from seed",
                                )
                            })
                        }
                        AstKind::VariableDeclarator(declarator) => {
                            let BindingPattern::BindingIdentifier(identifier) = &declarator.id
                            else {
                                return None;
                            };
                            let target = identifier.symbol_id.get()?;
                            let initializer = declarator.init.as_ref()?;
                            (captured_symbols.contains(&target)
                                && functions.mentions_slot(initializer, &site, parameter.index))
                            .then(|| {
                                evidence(
                                    source_text,
                                    node.span(),
                                    "local state initialized from seed",
                                )
                            })
                        }
                        _ => None,
                    }
                })
                .map(|local_seed| (parameter, local_seed))
        });
    let seed_parameter =
        seed_initialization
            .as_ref()
            .map(|(parameter, local_seed)| ParameterEvidence {
                index: parameter.index,
                binding: parameter
                    .symbol
                    .map(|symbol| binding(scoping, nodes, symbol)),
                role: "seed".to_string(),
                evidence: vec![
                    evidence(source_text, parameter.identifier, "captured seed input"),
                    local_seed.clone(),
                ],
            });
    let mut captured_state = Vec::new();
    if let Some(inner) = cycle.as_ref() {
        for node in functions.direct(&site, nodes) {
            let AstKind::VariableDeclarator(declarator) = node.kind() else {
                continue;
            };
            if !direct_node(&site, node.span(), functions) || node.span().start >= inner.span.start
            {
                continue;
            }
            let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                continue;
            };
            let Some(symbol) = identifier.symbol_id.get() else {
                continue;
            };
            let captured = scoping
                .get_resolved_reference_ids(symbol)
                .iter()
                .any(|reference| {
                    let span = nodes
                        .get_node(scoping.get_reference(*reference).node_id())
                        .span();
                    direct_node(inner, span, functions)
                });
            if !captured {
                continue;
            }
            let role = if functions.direct(inner, nodes).any(|node| direct_node(inner, node.span(), functions) && matches!(node.kind(), AstKind::BinaryExpression(binary) if binary.operator == BinaryOperator::Remainder && contains_symbol(&binary.left, symbol, scoping))) { "cycle_index" } else { "captured_word_state" };
            captured_state.push(StreamStateProperties {
                binding: Some(binding(scoping, nodes, symbol)),
                role: role.to_string(),
                evidence: vec![evidence(
                    source_text,
                    node.span(),
                    "factory-local captured state",
                )],
            });
        }
    }
    let shared_cursor_writeback = functions.direct(&site, nodes).find_map(|node| {
        if !direct_node(&site, node.span(), functions) {
            return None;
        }
        let AstKind::AssignmentExpression(assignment) = node.kind() else {
            return None;
        };
        let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
            return None;
        };
        let symbol = crate::analysis::identifier_symbol(identifier, scoping)?;
        site.params
            .iter()
            .any(|parameter| parameter.symbol == Some(symbol))
            .then(|| {
                evidence(
                    source_text,
                    assignment.span,
                    "factory parameter is reassigned",
                )
            })
    });
    let mut unresolved = Vec::new();
    if cycle.is_none() {
        unresolved.push(UnresolvedEncodingFact {
            fact: "stream_cycle".to_string(),
            reason: "returned closure with state cycle was not proven".to_string(),
            span: Some(site.span.into()),
            required: true,
        });
    }
    if warmup_calls == Some(0) {
        unresolved.push(UnresolvedEncodingFact {
            fact: "stream_warmup".to_string(),
            reason: "no factory-local warm-up calls were located".to_string(),
            span: Some(site.span.into()),
            required: false,
        });
    }
    let local_seed = seed_initialization.map(|(_, local_seed)| local_seed);
    ByteStreamProperties {
        factory: function_evidence(source_text, scoping, nodes, site.clone()),
        cycle: cycle.map(|value| function_evidence(source_text, scoping, nodes, value)),
        captured_state,
        warmup_calls,
        cycle_modulus,
        word_updates,
        extraction_shifts,
        seed_parameter,
        local_seed,
        per_caller_prefix: shared_cursor_writeback.is_none(),
        shared_cursor_writeback,
        unresolved,
    }
}

fn hash_candidate(
    source_text: &str,
    _scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
    functions: &FunctionScopeIndex<'_>,
) -> Option<FunctionSite> {
    let candidates: Vec<_> = functions
        .iter()
        .filter(|site| {
            let mut array_buffer = false;
            let mut int32 = false;
            let mut shift = false;
            let mut string_branch = false;
            for node in functions.direct(site, nodes) {
                if !direct_node(site, node.span(), functions) {
                    continue;
                }
                let text = source(source_text, node.span());
                array_buffer |= text.contains("ArrayBuffer");
                int32 |= text.contains("Int32Array");
                string_branch |= text.contains("typeof") && text.contains("string");
                if let AstKind::BinaryExpression(binary) = node.kind() {
                    shift |= binary.operator == BinaryOperator::ShiftLeft
                        && (numeric_literal(&binary.left) == Some(5.0)
                            || numeric_literal(&binary.right) == Some(5.0));
                }
            }
            array_buffer && int32 && shift && string_branch
        })
        .cloned()
        .collect();
    let mut candidates = candidates;
    candidates.sort_by_key(|candidate| candidate.span.end - candidate.span.start);
    if candidates.get(1).is_some_and(|other| {
        other.span.end - other.span.start == candidates[0].span.end - candidates[0].span.start
    }) {
        return None;
    }
    candidates.into_iter().next()
}

/// Extract source-backed local encoding helper properties from one exact JS
/// input. Ambiguous helper candidates are refused instead of selecting the
/// first similar function.
pub fn extract_encoding_properties(
    source_text: &str,
) -> Result<EncodingProperties, EncodingPropertiesError> {
    if source_text.trim().is_empty() {
        return Err(EncodingPropertiesError::InvalidInput(
            "source is empty".to_string(),
        ));
    }
    let allocator = oxc_allocator::Allocator::default();
    let parsed = crate::analysis::parse(&allocator, source_text);
    if let Some(error) = parsed.errors.first() {
        return Err(EncodingPropertiesError::Parse(error.clone()));
    }
    if parsed.panicked {
        return Err(EncodingPropertiesError::Parse(
            "Oxc parser panicked".to_string(),
        ));
    }
    let (scoping, nodes) = crate::analysis::analyze(&parsed.program);
    let slots = ParameterSlots::new(&parsed.program, &scoping, &nodes);
    let mut collector = FunctionCollector {
        slots: &slots,
        functions: Vec::new(),
        argument_reads: HashMap::new(),
    };
    collector.visit_program(&parsed.program);
    let functions = collector.finish();
    let function_index = FunctionScopeIndex::new(&nodes, &functions, &slots);
    let aliases = build_aliases(&parsed.program, &scoping, &slots);
    let number_candidates = number_candidates(&scoping, &nodes, &function_index);
    let string_sites = string_candidates(
        &scoping,
        &nodes,
        &function_index,
        &number_candidates,
        &aliases,
    );
    let linked_numbers = dedup_sites(
        &string_sites
            .iter()
            .flat_map(|factory| {
                linked_number_candidates(
                    factory,
                    &number_candidates,
                    &scoping,
                    &nodes,
                    &aliases,
                    &function_index,
                )
            })
            .collect::<Vec<_>>(),
    );
    let number_sites = select_number_candidates(&number_candidates, &linked_numbers);
    let stream_sites = stream_candidates(&scoping, &nodes, &function_index, &aliases);
    let number_site = number_sites.first().cloned();
    let string_site = string_sites.first().cloned();
    let stream_site = stream_sites.first().cloned();
    let number_equivalents = equivalent_spans(&number_sites);
    let string_equivalents = equivalent_spans(&string_sites);
    let stream_equivalents = equivalent_spans(&stream_sites);
    let hash_site = hash_candidate(source_text, &scoping, &nodes, &function_index);
    let mut unresolved = Vec::new();
    if number_site.is_none() {
        unresolved.push(UnresolvedEncodingFact {
            fact: "number_writer".to_string(),
            reason: if number_candidates.len() > 1 {
                "multiple admissible NUMBER writers lacked one proven STRING dependency"
            } else {
                "no integer/float byte writer was proven"
            }
            .to_string(),
            span: None,
            required: true,
        });
    }
    if string_site.is_none() {
        unresolved.push(UnresolvedEncodingFact {
            fact: "string_factory".to_string(),
            reason: "no unique cache-backed string factory with one returned writer was proven"
                .to_string(),
            span: None,
            required: true,
        });
    }
    if stream_site.is_none() {
        unresolved.push(UnresolvedEncodingFact {
            fact: "byte_stream_factory".to_string(),
            reason: "no unique returned stateful byte closure with distinguishing consumer proof was proven".to_string(),
            span: None,
            required: true,
        });
    }
    let seed_reads = hash_site
        .as_ref()
        .and_then(|hash| hash.symbol)
        .map(|hash_symbol| {
            nodes
                .iter()
                .filter_map(|node| {
                    let AstKind::CallExpression(call) = node.kind() else {
                        return None;
                    };
                    (call_target(&call.callee, &aliases, node.span().start, &scoping) == Some(hash_symbol)).then(
                        || {
                            evidence(
                                source_text,
                                call.span,
                                "runtime hash input; transaction-token/session proof is owned by integration",
                            )
                        },
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let seed_link = SeedLinkProperties {
        hash: hash_site.map(|value| function_evidence(source_text, &scoping, &nodes, value)),
        seed_reads,
        runtime_value: true,
        unresolved: vec![UnresolvedEncodingFact {
            fact: "runtime_seed_value".to_string(),
            reason: "token/hash/random runtime values are not source constants".to_string(),
            span: None,
            required: false,
        }],
    };
    let source_hash = Sha256::digest(source_text.as_bytes());
    let sha256 = source_hash
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(EncodingProperties {
        schema_version: ENCODING_PROPERTIES_SCHEMA_VERSION,
        source: SourceIdentity {
            sha256,
            bytes: source_text.len(),
        },
        number: number_site
            .clone()
            .map(|value| number_properties(source_text, &scoping, &nodes, value, &function_index)),
        string: string_site.map(|value| {
            string_properties(
                source_text,
                &scoping,
                &nodes,
                value,
                number_site.clone(),
                &aliases,
                &function_index,
            )
        }),
        byte_stream: stream_site
            .map(|value| stream_properties(source_text, &scoping, &nodes, &function_index, value)),
        number_equivalents,
        string_equivalents,
        stream_equivalents,
        seed_link,
        unresolved,
    })
}
