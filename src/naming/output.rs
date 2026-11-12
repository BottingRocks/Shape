//! Source-derived properties of the outer output pipeline.
//!
//! This pass describes the boundary between the recovered program's local
//! writers and the host: the request-time snapshot boundary, the configurable
//! encode selection (`options.encode` vs. the runtime encoder), the optional
//! outer cipher, the custom-alphabet unpadded text emission, the public
//! envelope object, the header chunker and the carrier adapters (form, fetch,
//! XHR, CustomEvent, and any cookie/URL/beacon consumer) which carry the
//! encoded envelope to a host surface.
//!
//! It is comprehension, not a codec and not a generator.  It never executes the
//! recovered JavaScript and never implements the cipher, the HTTP request, or
//! the alphabet mapping.  Every source constant it finds (the default alphabet,
//! the header prefix, the chunk size, cipher constants, entropy sources) is
//! reported as **data**, never used as a recognition gate.  A source function
//! that is present is not proof that it ran; captured host functions are not
//! proof of pristine natives; runtime values (token, keys, nonces, storage
//! contents) remain explicit unknowns.
//!
//! # Separation from the local writers
//!
//! The local NUMBER/STRING/byte writers and their per-message keystream belong
//! to [`super::encoding`].  The four raw seed bytes written before a local
//! stream and the raw transaction-token hash that seeds it are signal-local and
//! stay there.  The optional outer cipher prepends a *different* eight-byte
//! prefix to the assembled bundle; the envelope content hash is computed over
//! the final text.  Those three boundaries are recorded separately here and are
//! never merged.
//!
//! # Identity without generated names
//!
//! Functions are addressed by their resolved symbol ordinal and exact source
//! spans; generated `$KEY_`/`$THREAD_` spellings are labels only.  Symbols are
//! never compared across parses: [`extract_output_properties_with_context`]
//! validates the exact source hash first and then resolves spans in its own
//! parse.

use std::collections::{BTreeSet, HashMap};
use std::fmt;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{BinaryOperator, LogicalOperator, UnaryOperator};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::signals::extract::bindings::ParameterSlots;
use crate::signals::extract::encoding::{EncodingProperties, EncodingPropertiesError};
use crate::signals::extract::reaching::expression_value_at_use;
use crate::signals::extract::wiring::WiringProperties;
use crate::signals::schema::{Confidence, SourceIdentity, SourceSpan};

pub const OUTPUT_PROPERTIES_SCHEMA_VERSION: u32 = 1;

// ===========================================================================
// Errors
// ===========================================================================

/// A typed output-properties failure.  Every variant is a refusal, not a guess.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputPropertiesError {
    InvalidInput(String),
    Parse(String),
    Limit {
        what: String,
        limit: usize,
    },
    Ambiguous {
        what: String,
        candidates: Vec<String>,
    },
    Missing {
        what: String,
    },
    SourceIdentity {
        stage: String,
    },
}

impl fmt::Display for OutputPropertiesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(f, "invalid output-properties input: {message}"),
            Self::Parse(message) => write!(f, "output-properties parse failed: {message}"),
            Self::Limit { what, limit } => {
                write!(f, "output-properties limit exceeded for {what} ({limit})")
            }
            Self::Ambiguous { what, candidates } => write!(
                f,
                "output-properties ambiguity for {what}: {} candidates",
                candidates.len()
            ),
            Self::Missing { what } => {
                write!(f, "output-properties required fact is missing: {what}")
            }
            Self::SourceIdentity { stage } => {
                write!(
                    f,
                    "output-properties {stage} context does not match the source"
                )
            }
        }
    }
}

impl std::error::Error for OutputPropertiesError {}

impl From<EncodingPropertiesError> for OutputPropertiesError {
    fn from(error: EncodingPropertiesError) -> Self {
        match error {
            EncodingPropertiesError::InvalidInput(message) => Self::InvalidInput(message),
            EncodingPropertiesError::Parse(message) => Self::Parse(message),
            EncodingPropertiesError::Limit { what, limit } => Self::Limit {
                what: what.to_string(),
                limit,
            },
            EncodingPropertiesError::Ambiguous { what, candidates } => Self::Ambiguous {
                what: what.to_string(),
                candidates,
            },
        }
    }
}

// ===========================================================================
// Evidence and identity
// ===========================================================================

/// An exact source anchor plus the source text it names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputEvidence {
    pub span: SourceSpan,
    pub source: String,
    pub detail: String,
}

/// One source binding.  `symbol` is the resolved ordinal in this parse only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputBinding {
    pub symbol: u32,
    pub name: String,
    pub declaration: SourceSpan,
    pub identifier: SourceSpan,
}

/// One formal parameter of a callable, with the role proven from its use.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputParameter {
    pub index: usize,
    pub binding: OutputBinding,
    pub role: String,
    pub evidence: Vec<OutputEvidence>,
}

/// A source function together with its formals and evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputFunction {
    pub binding: Option<OutputBinding>,
    pub name_label: Option<String>,
    pub span: SourceSpan,
    pub body: Option<SourceSpan>,
    pub source: String,
    pub parameters: Vec<OutputParameter>,
    pub confidence: Confidence,
}

/// A fact this pass declined to guess.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputUnresolved {
    pub fact: String,
    pub reason: String,
    pub span: Option<SourceSpan>,
    pub required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputDiagnostic {
    pub code: String,
    pub message: String,
    pub span: Option<SourceSpan>,
}

// ===========================================================================
// Pipeline, encoder, cipher, alphabet
// ===========================================================================

/// One ordered stage of the outer pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStage {
    SnapshotCollector,
    EncodeSelection,
    DefaultEncoder,
    OuterCipher,
    CustomAlphabet,
    TextEmission,
    EnvelopeFields,
    Chunking,
    Carrier,
}

/// The snapshot boundary: the collector call and the runtime encoder default.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotBoundary {
    pub collector: Option<OutputBinding>,
    pub collector_call: Option<OutputEvidence>,
    pub encoder_default: Option<OutputBinding>,
    pub invocation: Option<OutputEvidence>,
}

/// The custom-alphabet / unpadded-text configuration of the encoder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlphabetProperties {
    /// Index of the factory formal used as the alphabet, when proven.
    pub parameter: Option<usize>,
    pub binding: Option<OutputBinding>,
    /// The literal used as the `??` fallback (the default alphabet, as data).
    pub default_literal: Option<OutputEvidence>,
    pub char_at_sites: Vec<OutputEvidence>,
    pub remainder_two: Option<OutputEvidence>,
    pub remainder_one: Option<OutputEvidence>,
    /// The encoder never emits padding here; reported as a bounded fact.
    pub padding_emitted: bool,
    /// Index of the factory formal accepted but never read (`shuffleKey`).
    pub shuffle_parameter: Option<usize>,
    pub shuffle_read: bool,
    pub unresolved: Vec<OutputUnresolved>,
}

/// The optional outer cipher and its per-call configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CipherConfig {
    pub cipher_binding: Option<OutputBinding>,
    pub key_binding: Option<OutputBinding>,
    /// The exact `cipher != null && key != null`-shaped gate.
    pub gate: Option<OutputEvidence>,
    pub call: Option<OutputEvidence>,
    /// The eight-byte prefix `unshift` performed after encryption.
    pub prefix_unshift: Option<OutputEvidence>,
    pub prefix_byte_count: Option<usize>,
    pub key_parameter_index: Option<usize>,
    pub entropy_sources: Vec<OutputEvidence>,
    pub entropy_counter: Option<OutputBinding>,
    /// Masked literal constants inside the cipher state, as data.
    pub constants: Vec<OutputEvidence>,
    pub initial_counter: Option<OutputEvidence>,
    pub unresolved: Vec<OutputUnresolved>,
}

/// The flat bundle framing contract, recorded as ordered region labels.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FramingProperties {
    pub regions: Vec<String>,
    pub length_sites: usize,
    pub raw_body_copy: bool,
    pub unresolved: Vec<OutputUnresolved>,
}

/// The configured bundle encoder factory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncoderFactory {
    pub function: OutputFunction,
    pub returned_closure: Option<OutputFunction>,
    pub alphabet: Option<AlphabetProperties>,
    pub cipher: Option<CipherConfig>,
    pub framing: Option<FramingProperties>,
    pub depth_bound: Option<OutputEvidence>,
    pub config: Vec<OutputEvidence>,
    pub unresolved: Vec<OutputUnresolved>,
}

// ===========================================================================
// Envelope
// ===========================================================================

/// The encode-selection: `options.encode` override vs. the runtime encoder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncodeSelection {
    pub span: SourceSpan,
    pub source: String,
    pub override_source: Option<String>,
    pub type_check: Option<OutputEvidence>,
    pub fallback: Option<OutputBinding>,
    /// The override return value is not type-checked by source.
    pub return_unchecked: bool,
}

/// The `notifyLastChance` defaulting expression, preserved exactly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotifyLastChance {
    pub span: SourceSpan,
    pub source: String,
    pub has_or_operand: bool,
    pub has_nullish: bool,
    pub literal_true: bool,
}

/// One public envelope field with a role derived from its value flow.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvelopeField {
    /// The observed property spelling (a label, not an identity gate).
    pub key: Option<String>,
    /// `encoder_text` | `token` | `content_hash` | `fixed_literal` |
    /// `descriptor_default` | `unknown`.
    pub role: String,
    pub value: OutputEvidence,
    pub literal: Option<String>,
}

/// The public envelope factory (`getEncodedData`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvelopeProperties {
    pub function: OutputFunction,
    pub parameter: Option<OutputParameter>,
    pub options_binding: Option<OutputBinding>,
    pub fields: Vec<EnvelopeField>,
    pub encode_selection: Option<EncodeSelection>,
    pub notify_last_chance: Option<NotifyLastChance>,
    pub include_debug_names_forced: bool,
    pub snapshot: Option<SnapshotBoundary>,
    /// The hash function used for the content hash, when it is the shared one.
    pub content_hash_function: Option<OutputBinding>,
    pub callers: Vec<OutputEvidence>,
    pub consumers: Vec<OutputEvidence>,
    pub unresolved: Vec<OutputUnresolved>,
}

// ===========================================================================
// Chunking
// ===========================================================================

/// One call site of the header chunker.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChunkCall {
    pub span: SourceSpan,
    pub source: String,
    pub size_literal: Option<f64>,
}

/// The header chunker.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChunkingProperties {
    pub function: OutputFunction,
    pub name_parameter: Option<usize>,
    pub value_parameter: Option<usize>,
    pub size_parameter: Option<usize>,
    pub slice_calls: Vec<OutputEvidence>,
    pub continuation_keys: Vec<OutputEvidence>,
    pub callers: Vec<ChunkCall>,
    pub unresolved: Vec<OutputUnresolved>,
}

// ===========================================================================
// Carriers
// ===========================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CarrierKind {
    Fetch,
    Xhr,
    Form,
    CustomEvent,
    Cookie,
    Url,
    Beacon,
    Unknown,
}

/// A lifecycle (`init`/`teardown`) record returned by a carrier factory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CarrierLifecycle {
    pub returns_lifecycle_object: bool,
    /// A returned lifecycle whose init and teardown bodies are both empty.
    pub no_op_return: bool,
}

/// A consumer that carries the encoded envelope to a host surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CarrierProperties {
    pub kind: CarrierKind,
    pub function: OutputFunction,
    pub root_function: Option<OutputFunction>,
    pub anchors: Vec<OutputEvidence>,
    pub captured_originals: Vec<OutputBinding>,
    pub uses_envelope: bool,
    pub uses_chunker: bool,
    pub chunk_calls: Vec<ChunkCall>,
    pub lifecycle: Option<CarrierLifecycle>,
    pub config: Vec<OutputEvidence>,
    pub anomalies: Vec<OutputEvidence>,
    pub confidence: Confidence,
    pub unresolved: Vec<OutputUnresolved>,
}

// ===========================================================================
// Cross-cutting report data
// ===========================================================================

/// A module/runtime-shared state binding (e.g. the cipher entropy counter).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateBinding {
    pub binding: OutputBinding,
    pub role: String,
    pub evidence: Vec<OutputEvidence>,
}

/// A callable binding with its role, confidence and adjacency.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallableBinding {
    pub role: String,
    pub binding: Option<OutputBinding>,
    pub function: OutputFunction,
    pub confidence: Confidence,
    pub callers: usize,
    pub consumers: usize,
}

/// A link between a context report and this report, keyed by role.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextLink {
    pub role: String,
    pub context_source: String,
    pub report_span: Option<SourceSpan>,
    pub matched: bool,
}

/// The complete outer-output-pipeline report for one exact capture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutputProperties {
    pub schema_version: u32,
    pub source: SourceIdentity,
    pub pipeline: Vec<PipelineStage>,
    pub encoder_factory: Option<EncoderFactory>,
    pub envelope: Option<EnvelopeProperties>,
    pub chunking: Option<ChunkingProperties>,
    pub carriers: Vec<CarrierProperties>,
    pub state: Vec<StateBinding>,
    pub callables: Vec<CallableBinding>,
    pub context_links: Vec<ContextLink>,
    pub unresolved: Vec<OutputUnresolved>,
    pub diagnostics: Vec<OutputDiagnostic>,
}

impl OutputProperties {
    pub fn carrier(&self, kind: CarrierKind) -> Option<&CarrierProperties> {
        self.carriers.iter().find(|carrier| carrier.kind == kind)
    }

    pub fn callable(&self, role: &str) -> Option<&CallableBinding> {
        self.callables.iter().find(|callable| callable.role == role)
    }
}

// ===========================================================================
// Internal model
// ===========================================================================

#[derive(Clone, Copy)]
struct Param {
    index: usize,
    identifier: Span,
    symbol: SymbolId,
}

#[derive(Clone)]
struct FunctionSite {
    node: oxc_semantic::NodeId,
    span: Span,
    body: Option<Span>,
    symbol: Option<SymbolId>,
    name_label: Option<String>,
    params: Vec<Param>,
    /// Argument slots the function accepts: its formals, or one past the
    /// highest `arguments[N]` it reads when that is higher.
    arity: usize,
}

struct FunctionCollector<'s, 'a> {
    slots: &'s ParameterSlots<'a, 'a>,
    functions: Vec<FunctionSite>,
    /// One past the highest `arguments[N]` read, per owning function.
    argument_arity: HashMap<oxc_semantic::NodeId, usize>,
}

impl FunctionCollector<'_, '_> {
    fn finish(mut self) -> Vec<FunctionSite> {
        for site in &mut self.functions {
            if let Some(arity) = self.argument_arity.get(&site.node) {
                site.arity = site.arity.max(*arity);
            }
        }
        self.functions
    }
}

type SpanKey = (u32, u32);

struct FunctionScopeIndex<'a> {
    functions: &'a [FunctionSite],
    owner_by_span: HashMap<SpanKey, Option<SpanKey>>,
}

impl<'a> FunctionScopeIndex<'a> {
    fn new(nodes: &oxc_semantic::AstNodes<'_>, functions: &'a [FunctionSite]) -> Self {
        let mut owner_by_span = HashMap::with_capacity(nodes.len());
        for (node_id, node) in nodes.iter_enumerated() {
            let owner = match node.kind() {
                AstKind::Function(function) => Some(function.span),
                AstKind::ArrowFunctionExpression(function) => Some(function.span),
                _ => nodes
                    .ancestors(node_id)
                    .find_map(|ancestor| match ancestor.kind() {
                        AstKind::Function(function) => Some(function.span),
                        AstKind::ArrowFunctionExpression(function) => Some(function.span),
                        _ => None,
                    }),
            };
            owner_by_span
                .entry((node.span().start, node.span().end))
                .or_insert_with(|| owner.map(|span| (span.start, span.end)));
        }
        Self {
            functions,
            owner_by_span,
        }
    }

    fn owner_of(&self, span: Span) -> Option<SpanKey> {
        self.owner_by_span
            .get(&(span.start, span.end))
            .copied()
            .flatten()
    }
}

impl<'a> Visit<'a> for FunctionCollector<'_, '_> {
    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        let params = collect_params(&function.params);
        self.functions.push(FunctionSite {
            node: function.node_id.get(),
            span: function.span,
            body: function.body.as_ref().map(|body| body.span),
            symbol: function.id.as_ref().and_then(|id| id.symbol_id.get()),
            name_label: function.id.as_ref().map(|id| id.name.to_string()),
            arity: params.len(),
            params,
        });
        walk::walk_function(self, function, flags);
    }

    fn visit_arrow_function_expression(&mut self, function: &ArrowFunctionExpression<'a>) {
        let params = collect_params(&function.params);
        self.functions.push(FunctionSite {
            node: function.node_id.get(),
            span: function.span,
            body: Some(function.body.span),
            symbol: None,
            name_label: None,
            arity: params.len(),
            params,
        });
        walk::walk_arrow_function_expression(self, function);
    }

    fn visit_expression(&mut self, expression: &Expression<'a>) {
        if matches!(expression, Expression::ComputedMemberExpression(_)) {
            if let Some(slot) = self.slots.parameter_slot(expression) {
                let arity = self.argument_arity.entry(slot.owner).or_default();
                *arity = (*arity).max(slot.index as usize + 1);
            }
        }
        walk::walk_expression(self, expression);
    }
}

fn collect_params(params: &FormalParameters<'_>) -> Vec<Param> {
    params
        .items
        .iter()
        .enumerate()
        .filter_map(|(index, parameter)| {
            let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
                return None;
            };
            Some(Param {
                index,
                identifier: identifier.span,
                symbol: identifier.symbol_id.get()?,
            })
        })
        .collect()
}

fn source_of(source: &str, span: Span) -> String {
    source
        .get(span.start as usize..span.end as usize)
        .unwrap_or_default()
        .to_string()
}

fn evidence(source: &str, span: Span, detail: impl Into<String>) -> OutputEvidence {
    OutputEvidence {
        span: span.into(),
        source: source_of(source, span),
        detail: detail.into(),
    }
}

fn binding_of(
    scoping: &Scoping,
    nodes: &oxc_semantic::AstNodes<'_>,
    symbol: SymbolId,
) -> OutputBinding {
    let declaration = nodes.get_node(scoping.symbol_declaration(symbol)).span();
    OutputBinding {
        symbol: symbol.index() as u32,
        name: scoping.symbol_name(symbol).to_string(),
        declaration: declaration.into(),
        identifier: scoping.symbol_span(symbol).into(),
    }
}

fn function_of(
    context: &Context<'_, '_>,
    site: &FunctionSite,
    roles: Option<&[String]>,
) -> OutputFunction {
    let parameters = site
        .params
        .iter()
        .map(|parameter| OutputParameter {
            index: parameter.index,
            binding: binding_of(context.scoping, context.nodes, parameter.symbol),
            role: roles
                .and_then(|roles| roles.get(parameter.index))
                .cloned()
                .unwrap_or_else(|| "unknown".to_string()),
            evidence: vec![evidence(
                context.source,
                parameter.identifier,
                "formal parameter",
            )],
        })
        .collect();
    OutputFunction {
        binding: site
            .symbol
            .map(|symbol| binding_of(context.scoping, context.nodes, symbol)),
        name_label: site.name_label.clone(),
        span: site.span.into(),
        body: site.body.map(Into::into),
        source: source_of(context.source, site.span),
        parameters,
        confidence: Confidence::High,
    }
}

fn within(outer: Span, inner: Span) -> bool {
    inner.start >= outer.start && inner.end <= outer.end
}

fn expression_symbol(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    match expression.without_parentheses() {
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

fn is_void_zero(expression: &Expression<'_>) -> bool {
    match expression.without_parentheses() {
        Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::Void => {
            numeric_literal(&unary.argument) == Some(0.0)
        }
        Expression::Identifier(identifier) => identifier.name == "undefined",
        _ => false,
    }
}

/// Build a conservative single-assignment identifier alias map.  Only symbols
/// whose every binding event is an identity alias (`let x = y`) are mapped.
fn build_aliases(program: &Program<'_>, scoping: &Scoping) -> HashMap<SymbolId, SymbolId> {
    struct Collector<'s> {
        scoping: &'s Scoping,
        events: HashMap<SymbolId, Vec<Option<SymbolId>>>,
    }
    impl<'s, 'a> Visit<'a> for Collector<'s> {
        fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
            // A declaration without an initializer stores no value: traces
            // declare every binding up front (`let a, b;`) and assign later.
            if let (BindingPattern::BindingIdentifier(identifier), Some(init)) =
                (&declarator.id, declarator.init.as_ref())
            {
                if let Some(symbol) = identifier.symbol_id.get() {
                    let target = expression_symbol(init, self.scoping);
                    self.events.entry(symbol).or_default().push(target);
                }
            }
            walk::walk_variable_declarator(self, declarator);
        }
        fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
            if let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left {
                if let Some(symbol) = crate::analysis::identifier_symbol(identifier, self.scoping) {
                    let target = expression_symbol(&assignment.right, self.scoping);
                    self.events.entry(symbol).or_default().push(target);
                }
            }
            walk::walk_assignment_expression(self, assignment);
        }
    }
    let mut collector = Collector {
        scoping,
        events: HashMap::new(),
    };
    collector.visit_program(program);
    collector
        .events
        .into_iter()
        .filter_map(|(symbol, events)| {
            let single = events.len() == 1;
            let target = events.into_iter().next().flatten();
            (single && target.is_some()).then(|| (symbol, target.unwrap()))
        })
        .collect()
}

fn resolve_alias(
    symbol: SymbolId,
    aliases: &HashMap<SymbolId, SymbolId>,
    seen: &mut BTreeSet<SymbolId>,
) -> SymbolId {
    let mut current = symbol;
    while seen.insert(current) {
        match aliases.get(&current) {
            Some(next) => current = *next,
            None => break,
        }
    }
    current
}

fn direct_node(site: &FunctionSite, span: Span, index: &FunctionScopeIndex<'_>) -> bool {
    if let Some(owner) = index.owner_of(span) {
        return owner == (site.span.start, site.span.end);
    }
    within(site.span, span)
}

fn enclosing<'a, 'b>(
    site: &FunctionSite,
    index: &'a FunctionScopeIndex<'b>,
) -> Option<&'a FunctionSite> {
    index
        .functions
        .iter()
        .filter(|candidate| candidate.span != site.span && within(candidate.span, site.span))
        .min_by_key(|candidate| candidate.span.end - candidate.span.start)
}

/// The member name of `object.name` or `object["name"]`; traces spell every
/// member the second way.
fn member_name<'e>(expression: &'e Expression<'_>) -> Option<&'e str> {
    match expression.without_parentheses() {
        Expression::StaticMemberExpression(member) => Some(member.property.name.as_str()),
        Expression::ComputedMemberExpression(member) => computed_member_name(member),
        _ => None,
    }
}

fn computed_member_name<'e>(member: &'e ComputedMemberExpression<'_>) -> Option<&'e str> {
    match member.expression.without_parentheses() {
        Expression::StringLiteral(name) => Some(name.value.as_str()),
        _ => None,
    }
}

/// A node that reads a named member, in either spelling: its object and name.
fn member_node<'a>(kind: AstKind<'a>) -> Option<(&'a Expression<'a>, &'a str)> {
    match kind {
        AstKind::StaticMemberExpression(member) => {
            Some((&member.object, member.property.name.as_str()))
        }
        AstKind::ComputedMemberExpression(member) => {
            computed_member_name(member).map(|name| (&member.object, name))
        }
        _ => None,
    }
}

/// The method name, receiver and arguments of `receiver.name(...)`, reading
/// `receiver.name.call(receiver, ...)` (the trace spelling) the same way when
/// the explicit receiver is the same binding or argument slot.
fn method_call<'c, 'a>(
    context: &Context<'_, '_>,
    call: &'c CallExpression<'a>,
) -> Option<(&'c str, &'c Expression<'a>, &'c [Argument<'a>])> {
    let name = member_name(&call.callee)?;
    let object = member_object(&call.callee)?;
    if name == "call" {
        if let (Some(method), Some(receiver)) = (member_name(object), member_object(object)) {
            let explicit = call
                .arguments
                .first()
                .and_then(|argument| argument.as_expression());
            if explicit.is_some_and(|explicit| context.same_reference(explicit, receiver)) {
                return Some((method, receiver, &call.arguments[1..]));
            }
        }
    }
    Some((name, object, &call.arguments))
}

fn member_property(expression: &Expression<'_>) -> Option<String> {
    member_name(expression).map(str::to_string)
}

fn member_property_root<'a>(expression: &'a Expression<'a>) -> Option<&'a str> {
    match expression.without_parentheses() {
        Expression::Identifier(identifier) => Some(identifier.name.as_str()),
        expression => member_name(expression),
    }
}

fn property_key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
        PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
        PropertyKey::NumericLiteral(literal) => Some(literal.value.to_string()),
        PropertyKey::Identifier(identifier) => Some(identifier.name.to_string()),
        _ => None,
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

// ===========================================================================
// Anchor scanning
// ===========================================================================

#[derive(Clone)]
struct Anchor {
    kind: &'static str,
    span: Span,
}

fn scan_anchors(nodes: &oxc_semantic::AstNodes<'_>, within: Span) -> Vec<Anchor> {
    let mut anchors = Vec::new();
    for node in nodes.iter() {
        let span = node.span();
        if !within_span(within, span) {
            continue;
        }
        match node.kind() {
            AstKind::CallExpression(call) => {
                if let (Some(name), Some(object)) =
                    (member_name(&call.callee), member_object(&call.callee))
                {
                    match name {
                        "setRequestHeader" => anchors.push(Anchor { kind: "xhr", span }),
                        "set" if member_property(object).as_deref() == Some("headers") => anchors
                            .push(Anchor {
                                kind: "fetch",
                                span,
                            }),
                        "createElement" | "setAttribute" | "querySelector" | "appendChild"
                        | "insertBefore" => anchors.push(Anchor { kind: "form", span }),
                        "dispatchEvent" | "initCustomEvent" => anchors.push(Anchor {
                            kind: "custom_event",
                            span,
                        }),
                        "sendBeacon" => anchors.push(Anchor {
                            kind: "beacon",
                            span,
                        }),
                        "createEvent" => {
                            let is_custom = call
                                .arguments
                                .first()
                                .and_then(|argument| argument.as_expression())
                                .is_some_and(|argument| {
                                    matches!(
                                        argument.without_parentheses(),
                                        Expression::StringLiteral(literal)
                                            if literal.value == "CustomEvent"
                                    )
                                });
                            if is_custom {
                                anchors.push(Anchor {
                                    kind: "custom_event",
                                    span,
                                });
                            }
                        }
                        _ => {}
                    }
                }
                if matches!(&call.callee, Expression::Identifier(identifier) if identifier.name == "fetch")
                {
                    anchors.push(Anchor {
                        kind: "fetch",
                        span,
                    });
                }
            }
            AstKind::NewExpression(new) => match member_property_root(&new.callee) {
                Some("Request") | Some("Headers") => anchors.push(Anchor {
                    kind: "fetch",
                    span,
                }),
                Some("CustomEvent") => anchors.push(Anchor {
                    kind: "custom_event",
                    span,
                }),
                Some("XMLHttpRequest") => anchors.push(Anchor { kind: "xhr", span }),
                Some("URL") | Some("URLSearchParams") => anchors.push(Anchor { kind: "url", span }),
                Some("FormData") => anchors.push(Anchor { kind: "form", span }),
                _ => {}
            },
            kind @ (AstKind::StaticMemberExpression(_) | AstKind::ComputedMemberExpression(_)) => {
                match member_node(kind).map(|(_, name)| name) {
                    Some("cookie") => anchors.push(Anchor {
                        kind: "cookie",
                        span,
                    }),
                    Some("searchParams") => anchors.push(Anchor { kind: "url", span }),
                    Some("XMLHttpRequest") => anchors.push(Anchor { kind: "xhr", span }),
                    _ => {}
                }
            }
            AstKind::IdentifierReference(identifier) => {
                if identifier.name == "XMLHttpRequest" {
                    anchors.push(Anchor { kind: "xhr", span });
                }
            }
            _ => {}
        }
    }
    anchors
}

fn classify(anchors: &[Anchor]) -> CarrierKind {
    let has = |kind: &str| anchors.iter().any(|anchor| anchor.kind == kind);
    if has("xhr") {
        CarrierKind::Xhr
    } else if has("custom_event") {
        CarrierKind::CustomEvent
    } else if has("fetch") {
        CarrierKind::Fetch
    } else if has("beacon") {
        CarrierKind::Beacon
    } else if has("cookie") {
        CarrierKind::Cookie
    } else if has("url") {
        CarrierKind::Url
    } else if has("form") {
        CarrierKind::Form
    } else {
        CarrierKind::Unknown
    }
}

// ===========================================================================
// Extraction context
// ===========================================================================

struct Context<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s oxc_semantic::AstNodes<'a>,
    index: FunctionScopeIndex<'s>,
    slots: &'s ParameterSlots<'a, 'a>,
    aliases: HashMap<SymbolId, SymbolId>,
    /// Every identifier reference grouped by its alias-resolved root symbol.
    reference_roots: HashMap<SymbolId, Vec<Span>>,
    encoding: Option<&'s EncodingProperties>,
    wiring: Option<&'s WiringProperties>,
}

impl<'s, 'a> Context<'s, 'a> {
    fn ev(&self, span: Span, detail: impl Into<String>) -> OutputEvidence {
        evidence(self.source, span, detail)
    }

    fn references(&self, symbol: SymbolId) -> Vec<Span> {
        self.scoping
            .get_resolved_reference_ids(symbol)
            .iter()
            .map(|reference| {
                self.nodes
                    .get_node(self.scoping.get_reference(*reference).node_id())
                    .span()
            })
            .collect()
    }

    /// Whether `site`'s subtree contains any reference whose alias-resolved
    /// root is `symbol` (this catches carriers that use the module-level alias
    /// of a discovered function rather than its declaration name).
    fn subtree_references(&self, site: &FunctionSite, symbol: SymbolId) -> bool {
        self.reference_roots
            .get(&symbol)
            .is_some_and(|spans| spans.iter().any(|span| within(site.span, *span)))
    }

    fn resolve(&self, symbol: SymbolId) -> SymbolId {
        let mut seen = BTreeSet::new();
        resolve_alias(symbol, &self.aliases, &mut seen)
    }

    /// Whether two side-effect-free references name the same binding or the
    /// same argument slot.
    fn same_reference(&self, left: &Expression<'_>, right: &Expression<'_>) -> bool {
        if let (Some(left), Some(right)) = (
            self.slots.parameter_slot(left),
            self.slots.parameter_slot(right),
        ) {
            return left == right;
        }
        match (left.without_parentheses(), right.without_parentheses()) {
            (Expression::Identifier(left), Expression::Identifier(right)) => {
                let symbol =
                    |identifier| crate::analysis::identifier_symbol(identifier, self.scoping);
                match (symbol(left), symbol(right)) {
                    (Some(left), Some(right)) => left == right,
                    _ => false,
                }
            }
            _ => false,
        }
    }

    /// The argument slot of `site` that `expression` denotes: `arguments[N]`
    /// of `site`, or a name whose alias root is an eligible formal of `site`.
    fn slot_index(&self, expression: &Expression<'_>, site: &FunctionSite) -> Option<usize> {
        let slot = match self.slots.parameter_slot(expression) {
            Some(slot) => slot,
            None => {
                let symbol = expression_symbol(expression, self.scoping)?;
                self.slots.formal_symbol_slot(self.resolve(symbol))?
            }
        };
        (slot.owner == site.node).then_some(slot.index as usize)
    }

    fn param_index(&self, symbol: SymbolId, site: &FunctionSite) -> Option<usize> {
        let resolved = self.resolve(symbol);
        site.params
            .iter()
            .find(|parameter| self.resolve(parameter.symbol) == resolved)
            .map(|parameter| parameter.index)
    }
}

fn source_identity(source: &str) -> SourceIdentity {
    let digest = Sha256::digest(source.as_bytes());
    SourceIdentity {
        sha256: digest.iter().map(|byte| format!("{byte:02x}")).collect(),
        bytes: source.len(),
    }
}

/// Extract the outer-output-pipeline properties from one exact capture,
/// reusing helper/collector identity from the encoding and wiring reports.
pub fn extract_output_properties_with_context(
    source: &str,
    encoding: &EncodingProperties,
    wiring: &WiringProperties,
) -> Result<OutputProperties, OutputPropertiesError> {
    let identity = source_identity(source);
    if encoding.source.bytes != identity.bytes || encoding.source.sha256 != identity.sha256 {
        return Err(OutputPropertiesError::SourceIdentity {
            stage: "encoding".to_string(),
        });
    }
    if wiring.source.bytes != identity.bytes || wiring.source.sha256 != identity.sha256 {
        return Err(OutputPropertiesError::SourceIdentity {
            stage: "wiring".to_string(),
        });
    }
    extract_impl(source, Some(encoding), Some(wiring))
}

fn extract_impl(
    source: &str,
    encoding: Option<&EncodingProperties>,
    wiring: Option<&WiringProperties>,
) -> Result<OutputProperties, OutputPropertiesError> {
    if source.trim().is_empty() {
        return Err(OutputPropertiesError::InvalidInput(
            "source is empty".to_string(),
        ));
    }
    let allocator = Allocator::default();
    let parsed = crate::analysis::parse(&allocator, source);
    if let Some(error) = parsed.errors.first() {
        return Err(OutputPropertiesError::Parse(error.clone()));
    }
    if parsed.panicked {
        return Err(OutputPropertiesError::Parse(
            "Oxc parser panicked on the source".to_string(),
        ));
    }
    let (scoping, nodes) = crate::analysis::analyze(&parsed.program);
    let slots = ParameterSlots::new(&parsed.program, &scoping, &nodes);
    let mut collector = FunctionCollector {
        slots: &slots,
        functions: Vec::new(),
        argument_arity: HashMap::new(),
    };
    collector.visit_program(&parsed.program);
    let functions = collector.finish();
    let index = FunctionScopeIndex::new(&nodes, &functions);
    let aliases = build_aliases(&parsed.program, &scoping);
    let mut reference_roots: HashMap<SymbolId, Vec<Span>> = HashMap::new();
    for node in nodes.iter() {
        let AstKind::IdentifierReference(identifier) = node.kind() else {
            continue;
        };
        let Some(symbol) = crate::analysis::identifier_symbol(identifier, &scoping) else {
            continue;
        };
        let mut seen = BTreeSet::new();
        let root = resolve_alias(symbol, &aliases, &mut seen);
        reference_roots.entry(root).or_default().push(node.span());
    }
    let context = Context {
        source,
        scoping: &scoping,
        nodes: &nodes,
        index,
        slots: &slots,
        aliases,
        reference_roots,
        encoding,
        wiring,
    };
    build_report(&context)
}

// ===========================================================================
// Report construction
// ===========================================================================

fn build_report(context: &Context<'_, '_>) -> Result<OutputProperties, OutputPropertiesError> {
    let mut unresolved = Vec::new();
    let mut diagnostics = Vec::new();

    let encoder = encoder_factory(context, &mut unresolved);
    let envelope = envelope(context, &mut unresolved, &mut diagnostics);
    let chunking = chunking(context, &mut unresolved);

    let envelope_symbol = envelope
        .as_ref()
        .and_then(|envelope| envelope.function.binding.as_ref())
        .map(|binding| binding.symbol);
    let chunker_symbol = chunking
        .as_ref()
        .and_then(|chunking| chunking.function.binding.as_ref())
        .map(|binding| binding.symbol);

    let carriers = carriers(
        context,
        envelope_symbol,
        chunker_symbol,
        &chunking,
        &mut diagnostics,
    );

    let mut pipeline = Vec::new();
    if envelope
        .as_ref()
        .and_then(|envelope| envelope.snapshot.as_ref())
        .is_some()
    {
        pipeline.push(PipelineStage::SnapshotCollector);
    }
    if envelope
        .as_ref()
        .and_then(|envelope| envelope.encode_selection.as_ref())
        .is_some()
    {
        pipeline.push(PipelineStage::EncodeSelection);
    }
    if encoder.is_some() {
        pipeline.push(PipelineStage::DefaultEncoder);
    }
    if encoder
        .as_ref()
        .and_then(|encoder| encoder.cipher.as_ref())
        .is_some()
    {
        pipeline.push(PipelineStage::OuterCipher);
    }
    if encoder
        .as_ref()
        .and_then(|encoder| encoder.alphabet.as_ref())
        .is_some()
    {
        pipeline.push(PipelineStage::CustomAlphabet);
        pipeline.push(PipelineStage::TextEmission);
    }
    if envelope.is_some() {
        pipeline.push(PipelineStage::EnvelopeFields);
    }
    if chunking.is_some() {
        pipeline.push(PipelineStage::Chunking);
    }
    if !carriers.is_empty() {
        pipeline.push(PipelineStage::Carrier);
    }

    if encoder.is_none() {
        unresolved.push(fact(
            "default_encoder_factory",
            "no configured bundle encoder factory was proven",
            None,
            true,
        ));
    }
    if envelope.is_none() {
        unresolved.push(fact(
            "public_envelope",
            "no envelope factory with the notify-last-chance default was proven",
            None,
            true,
        ));
    }
    if chunking.is_none() {
        unresolved.push(fact(
            "header_chunker",
            "no header chunker with the slice/continuation shape was proven",
            None,
            true,
        ));
    }

    let mut state = Vec::new();
    let mut callables = Vec::new();
    if let Some(encoder) = &encoder {
        callables.push(callable(
            context,
            "encoder_factory",
            &encoder.function,
            symbol_of(&encoder.function, context),
            Confidence::High,
        ));
        if let Some(cipher) = &encoder.cipher {
            if let Some(counter) = &cipher.entropy_counter {
                state.push(StateBinding {
                    binding: counter.clone(),
                    role: "entropy_counter".to_string(),
                    evidence: cipher.constants.clone(),
                });
            }
        }
    }
    if let Some(envelope) = &envelope {
        callables.push(callable(
            context,
            "envelope_factory",
            &envelope.function,
            symbol_of(&envelope.function, context),
            Confidence::High,
        ));
    }
    if let Some(chunking) = &chunking {
        callables.push(callable(
            context,
            "header_chunker",
            &chunking.function,
            symbol_of(&chunking.function, context),
            Confidence::High,
        ));
    }

    let context_links = context_links(context, &encoder, &envelope);

    Ok(OutputProperties {
        schema_version: OUTPUT_PROPERTIES_SCHEMA_VERSION,
        source: source_identity(context.source),
        pipeline,
        encoder_factory: encoder,
        envelope,
        chunking,
        carriers,
        state,
        callables,
        context_links,
        unresolved,
        diagnostics,
    })
}

fn fact(
    name: &str,
    reason: impl Into<String>,
    span: Option<SourceSpan>,
    required: bool,
) -> OutputUnresolved {
    OutputUnresolved {
        fact: name.to_string(),
        reason: reason.into(),
        span,
        required,
    }
}

/// Recover the internal `SymbolId` for a report function from its ordinal by
/// matching the exact declaration span in this parse.
fn symbol_of(function: &OutputFunction, context: &Context<'_, '_>) -> Option<SymbolId> {
    let binding = function.binding.as_ref()?;
    let identifier = Span::new(binding.identifier.start, binding.identifier.end);
    context
        .nodes
        .iter()
        .find_map(|node| match node.kind() {
            AstKind::BindingIdentifier(identifier_node) => {
                (identifier_node.span == identifier).then(|| identifier_node.symbol_id.get())
            }
            _ => None,
        })
        .flatten()
}

fn callable(
    context: &Context<'_, '_>,
    role: &str,
    function: &OutputFunction,
    symbol: Option<SymbolId>,
    confidence: Confidence,
) -> CallableBinding {
    let (mut callers, mut consumers) = (0usize, 0usize);
    if let Some(symbol) = symbol {
        let own = (function.span.start, function.span.end);
        for span in context.references(symbol) {
            match context.index.owner_of(span) {
                Some(owner) if owner == own => {}
                Some(_) => callers += 1,
                None => consumers += 1,
            }
        }
    }
    CallableBinding {
        role: role.to_string(),
        binding: function.binding.clone(),
        function: function.clone(),
        confidence,
        callers,
        consumers,
    }
}

// ===========================================================================
// Encoder factory
// ===========================================================================

fn encoder_factory(
    context: &Context<'_, '_>,
    unresolved: &mut Vec<OutputUnresolved>,
) -> Option<EncoderFactory> {
    let closure = encoder_closure(context)?;
    let factory = enclosing(&closure, &context.index).cloned();
    let Some(factory) = factory else {
        unresolved.push(fact(
            "encoder_factory",
            "the text closure is not returned by an enclosing factory",
            Some(closure.span.into()),
            true,
        ));
        return None;
    };

    let cipher = cipher_config(context, &closure, &factory);
    let alphabet = alphabet(context, &closure, &factory);
    let framing = framing(context, &closure);
    let depth_bound = context
        .nodes
        .iter()
        .filter(|node| within(factory.span, node.span()))
        .find_map(|node| {
            let AstKind::ObjectExpression(object) = node.kind() else {
                return None;
            };
            object
                .properties
                .iter()
                .find_map(|property| match property {
                    ObjectPropertyKind::ObjectProperty(property) => {
                        (property_key_name(&property.key).as_deref() == Some("depthBound"))
                            .then(|| context.ev(property.value.span(), "serializer depth bound"))
                    }
                    _ => None,
                })
        });

    let config = context
        .nodes
        .iter()
        .filter(|node| within(factory.span, node.span()))
        .filter(|node| matches!(node.kind(), AstKind::ObjectExpression(_)))
        .map(|node| context.ev(node.span(), "encoder configuration object"))
        .collect();

    let mut local = Vec::new();
    if cipher.is_none() {
        local.push(fact(
            "outer_cipher",
            "no cipher call and eight-byte prefix were proven inside the encoder",
            Some(closure.span.into()),
            false,
        ));
    }
    if alphabet
        .as_ref()
        .and_then(|alphabet| alphabet.default_literal.as_ref())
        .is_none()
    {
        local.push(fact(
            "custom_alphabet_default",
            "no `??` alphabet literal was proven in the encoder",
            Some(closure.span.into()),
            true,
        ));
    }

    Some(EncoderFactory {
        function: function_of(context, &factory, None),
        returned_closure: Some(function_of(context, &closure, None)),
        alphabet,
        cipher,
        framing,
        depth_bound,
        config,
        unresolved: local,
    })
}

fn encoder_closure(context: &Context<'_, '_>) -> Option<FunctionSite> {
    let mut candidates: Vec<FunctionSite> = context
        .index
        .functions
        .iter()
        .filter(|site| {
            let mut char_at = 0usize;
            let mut remainder = 0usize;
            let mut unshift = false;
            for node in context.nodes.iter() {
                if !direct_node(site, node.span(), &context.index) {
                    continue;
                }
                match node.kind() {
                    AstKind::CallExpression(call) => {
                        match member_property(&call.callee).as_deref() {
                            Some("charAt") => char_at += 1,
                            Some("unshift") => unshift = true,
                            _ => {}
                        }
                    }
                    AstKind::BinaryExpression(binary)
                        if binary.operator == BinaryOperator::Remainder =>
                    {
                        remainder += 1;
                    }
                    _ => {}
                }
            }
            char_at >= 3 && remainder >= 2 && unshift
        })
        .cloned()
        .collect();
    candidates.sort_by_key(|site| site.span.end - site.span.start);
    candidates.into_iter().next()
}

fn alphabet(
    context: &Context<'_, '_>,
    closure: &FunctionSite,
    factory: &FunctionSite,
) -> Option<AlphabetProperties> {
    let mut default_literal = None;
    let mut parameter = None;
    let mut binding = None;
    for node in context.nodes.iter() {
        if !within(factory.span, node.span()) {
            continue;
        }
        let AstKind::LogicalExpression(logical) = node.kind() else {
            continue;
        };
        if logical.operator != LogicalOperator::Coalesce {
            continue;
        }
        let Expression::StringLiteral(literal) = logical.right.without_parentheses() else {
            continue;
        };
        default_literal = Some(context.ev(logical.right.span(), "default alphabet literal"));
        parameter = context.slot_index(&logical.left, factory);
        if let Some(symbol) = expression_symbol(&logical.left, context.scoping) {
            let resolved = context.resolve(symbol);
            binding = Some(binding_of(context.scoping, context.nodes, resolved));
        }
        let _ = literal;
        break;
    }

    let mut char_at_sites = Vec::new();
    let mut remainder_two = None;
    let mut remainder_one = None;
    for node in context.nodes.iter() {
        if !within(closure.span, node.span()) {
            continue;
        }
        match node.kind() {
            AstKind::CallExpression(call)
                if member_property(&call.callee).as_deref() == Some("charAt") =>
            {
                char_at_sites.push(context.ev(node.span(), "alphabet character lookup"))
            }
            AstKind::BinaryExpression(binary)
                if binary.operator == BinaryOperator::StrictEquality =>
            {
                let (left, right) = (&binary.left, &binary.right);
                if numeric_literal(right) == Some(2.0) || numeric_literal(left) == Some(2.0) {
                    remainder_two = Some(context.ev(node.span(), "two-byte text tail"));
                }
                if numeric_literal(right) == Some(1.0) || numeric_literal(left) == Some(1.0) {
                    remainder_one = Some(context.ev(node.span(), "one-byte text tail"));
                }
            }
            _ => {}
        }
    }

    // The shuffle formal is accepted by the factory but never read anywhere.
    let mut shuffle_parameter = None;
    let shuffle_read = false;
    let unread: Vec<usize> = factory
        .params
        .iter()
        .filter(|parameter_site| {
            let resolved = context.resolve(parameter_site.symbol);
            !context
                .references(resolved)
                .into_iter()
                .any(|span| within(factory.span, span))
        })
        .filter(|parameter_site| Some(parameter_site.index) != parameter)
        .map(|parameter_site| parameter_site.index)
        .collect();
    if let [index] = unread.as_slice() {
        shuffle_parameter = Some(*index);
    }

    let mut unresolved = Vec::new();
    if default_literal.is_none() {
        unresolved.push(fact(
            "custom_alphabet_default",
            "the default alphabet literal was not proven",
            Some(factory.span.into()),
            true,
        ));
    }

    Some(AlphabetProperties {
        parameter,
        binding,
        default_literal,
        char_at_sites,
        remainder_two,
        remainder_one,
        padding_emitted: false,
        shuffle_parameter,
        shuffle_read,
        unresolved,
    })
}

fn cipher_config(
    context: &Context<'_, '_>,
    closure: &FunctionSite,
    factory: &FunctionSite,
) -> Option<CipherConfig> {
    // Locate the gate `cipher != null && key != null`.
    let mut gate = None;
    let mut cipher_symbol = None;
    let mut key_parameter_index = None;
    for node in context.nodes.iter() {
        if !within(closure.span, node.span()) {
            continue;
        }
        let AstKind::LogicalExpression(logical) = node.kind() else {
            continue;
        };
        if logical.operator != LogicalOperator::And {
            continue;
        }
        let mut operands = [&logical.left, &logical.right];
        let mut seen = Vec::new();
        for operand in operands.iter_mut() {
            if let Expression::BinaryExpression(binary) = operand.without_parentheses() {
                if matches!(
                    binary.operator,
                    BinaryOperator::Inequality | BinaryOperator::Equality
                ) {
                    if let Some(symbol) = nullish_operand_symbol(binary, context) {
                        seen.push(symbol);
                    }
                }
            }
        }
        if seen.len() == 2 {
            gate = Some(context.ev(node.span(), "cipher gate"));
            cipher_symbol = Some(seen[0]);
            key_parameter_index = context.param_index(seen[1], factory);
            break;
        }
    }

    // Locate the cipher call and the eight-byte prefix.
    let mut call_evidence = None;
    let mut prefix_unshift = None;
    let mut prefix_byte_count = None;
    let mut entropy_counter = None;
    let mut entropy_sources = Vec::new();
    let mut constants = Vec::new();
    let mut initial_counter = None;
    let gate_span = gate
        .as_ref()
        .map(|evidence| Span::new(evidence.span.start, evidence.span.end));

    for node in context.nodes.iter() {
        if !within(closure.span, node.span()) {
            continue;
        }
        match node.kind() {
            AstKind::CallExpression(call) => {
                if let Some(target) = expression_symbol(&call.callee, context.scoping) {
                    if Some(context.resolve(target)) == cipher_symbol.map(|s| context.resolve(s)) {
                        call_evidence = Some(context.ev(node.span(), "outer cipher call"));
                    }
                }
                match member_property(&call.callee).as_deref() {
                    Some("unshift") => {
                        prefix_unshift = Some(context.ev(node.span(), "cipher prefix prepend"));
                        prefix_byte_count = Some(call.arguments.len());
                    }
                    Some("now") => entropy_sources.push(context.ev(node.span(), "performance.now")),
                    Some("getRandomValues") => {
                        entropy_sources.push(context.ev(node.span(), "crypto.getRandomValues"))
                    }
                    Some("random") => {
                        entropy_sources.push(context.ev(node.span(), "Math.random entropy"))
                    }
                    _ => {}
                }
            }
            AstKind::UpdateExpression(update) => {
                if let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) =
                    &update.argument
                {
                    if let Some(symbol) =
                        crate::analysis::identifier_symbol(identifier, context.scoping)
                    {
                        if gate_span.is_some_and(|gate| within(gate, node.span()))
                            || within(closure.span, node.span())
                        {
                            entropy_counter = Some(binding_of(
                                context.scoping,
                                context.nodes,
                                context.resolve(symbol),
                            ));
                        }
                    }
                }
            }
            AstKind::BinaryExpression(binary)
                if binary.operator == BinaryOperator::BitwiseAnd
                    && (numeric_literal(&binary.left).is_some()
                        || numeric_literal(&binary.right).is_some()) =>
            {
                if constants.len() < 8 {
                    constants.push(context.ev(node.span(), "cipher masked constant"));
                }
            }
            AstKind::NumericLiteral(literal) if literal.value > 1_000_000_000_000.0 => {
                if initial_counter.is_none() {
                    initial_counter = Some(context.ev(node.span(), "cipher initial block counter"));
                }
            }
            _ => {}
        }
    }

    let cipher_binding = cipher_symbol
        .map(|symbol| binding_of(context.scoping, context.nodes, context.resolve(symbol)));
    let key_binding = key_parameter_index
        .and_then(|index| {
            factory
                .params
                .iter()
                .find(|parameter| parameter.index == index)
        })
        .map(|parameter| {
            binding_of(
                context.scoping,
                context.nodes,
                context.resolve(parameter.symbol),
            )
        });

    if call_evidence.is_none() && gate.is_none() {
        return None;
    }

    Some(CipherConfig {
        cipher_binding,
        key_binding,
        gate,
        call: call_evidence,
        prefix_unshift,
        prefix_byte_count,
        key_parameter_index,
        entropy_sources,
        entropy_counter,
        constants,
        initial_counter,
        unresolved: Vec::new(),
    })
}

fn nullish_operand_symbol(
    binary: &BinaryExpression<'_>,
    context: &Context<'_, '_>,
) -> Option<SymbolId> {
    let left = binary.left.without_parentheses();
    let right = binary.right.without_parentheses();
    if matches!(left, Expression::NullLiteral(_)) {
        return expression_symbol(right, context.scoping);
    }
    if matches!(right, Expression::NullLiteral(_)) {
        return expression_symbol(left, context.scoping);
    }
    None
}

fn framing(context: &Context<'_, '_>, closure: &FunctionSite) -> Option<FramingProperties> {
    // The snapshot is the closure's first argument slot.
    if closure.arity == 0 {
        return None;
    }
    let mut regions = Vec::new();
    let mut length_sites = 0usize;
    let mut raw_body_copy = false;
    for node in context.nodes.iter() {
        if !within(closure.span, node.span()) {
            continue;
        }
        match node.kind() {
            kind @ (AstKind::StaticMemberExpression(_) | AstKind::ComputedMemberExpression(_)) => {
                let Some((object, name)) = member_node(kind) else {
                    continue;
                };
                if context.slot_index(object, closure) == Some(0) {
                    let name = name.to_string();
                    if !regions.contains(&name) {
                        regions.push(name);
                    }
                }
            }
            AstKind::CallExpression(call) => {
                if member_property(&call.callee).as_deref() == Some("push") {
                    length_sites += 1;
                }
            }
            AstKind::ForStatement(_) | AstKind::WhileStatement(_) | AstKind::ForInStatement(_) => {
                raw_body_copy = true;
            }
            _ => {}
        }
    }
    Some(FramingProperties {
        regions,
        length_sites,
        raw_body_copy,
        unresolved: Vec::new(),
    })
}

// ===========================================================================
// Envelope
// ===========================================================================

fn envelope(
    context: &Context<'_, '_>,
    unresolved: &mut Vec<OutputUnresolved>,
    diagnostics: &mut Vec<OutputDiagnostic>,
) -> Option<EnvelopeProperties> {
    let candidates: Vec<FunctionSite> = context
        .index
        .functions
        .iter()
        .filter(|site| {
            context.nodes.iter().any(|node| {
                if !direct_node(site, node.span(), &context.index) {
                    return false;
                }
                let AstKind::LogicalExpression(logical) = node.kind() else {
                    return false;
                };
                logical.operator == LogicalOperator::Coalesce
                    && matches!(
                        logical.right.without_parentheses(),
                        Expression::BooleanLiteral(literal) if literal.value
                    )
            })
        })
        .cloned()
        .collect();
    if candidates.is_empty() {
        return None;
    }
    if candidates.len() > 1 {
        diagnostics.push(OutputDiagnostic {
            code: "envelope_ambiguous".to_string(),
            message: format!(
                "{} functions carry the notify-last-chance default",
                candidates.len()
            ),
            span: None,
        });
        unresolved.push(fact(
            "public_envelope",
            "more than one notify-last-chance default was proven; refusing to pick",
            None,
            true,
        ));
        return None;
    }
    let site = candidates.into_iter().next().unwrap();

    // The options object: `P != null ? P : Object.create(null)`.
    let mut options_binding = None;
    let mut parameter = None;
    for node in context.nodes.iter() {
        if !direct_node(&site, node.span(), &context.index) {
            continue;
        }
        match node.kind() {
            AstKind::VariableDeclarator(declarator) => {
                if let BindingPattern::BindingIdentifier(_) = &declarator.id {
                    if let Some(init) = declarator.init.as_ref() {
                        if let Expression::ConditionalExpression(conditional) =
                            init.without_parentheses()
                        {
                            if let Some(symbol) =
                                expression_symbol(&conditional.consequent, context.scoping)
                            {
                                options_binding = Some(binding_of(
                                    context.scoping,
                                    context.nodes,
                                    context.resolve(symbol),
                                ));
                                parameter = context.param_index(symbol, &site);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let parameter = parameter
        .and_then(|index| site.params.iter().find(|p| p.index == index))
        .map(|parameter| OutputParameter {
            index: parameter.index,
            binding: binding_of(context.scoping, context.nodes, parameter.symbol),
            role: "request_options".to_string(),
            evidence: vec![context.ev(parameter.identifier, "options parameter")],
        });

    // The returned envelope object symbol.
    let object_symbol = context.nodes.iter().find_map(|node| {
        if !direct_node(&site, node.span(), &context.index) {
            return None;
        }
        let AstKind::ReturnStatement(statement) = node.kind() else {
            return None;
        };
        statement
            .argument
            .as_ref()
            .and_then(|argument| expression_symbol(argument, context.scoping))
    });

    // Encode selection: `typeof (options.encode ...) === "function" ? ... : default`.
    // Each candidate also keeps the selection expression itself, so a later
    // read can be matched by the value it holds rather than by the binding.
    let encode = context.nodes.iter().find_map(|node| {
        if !direct_node(&site, node.span(), &context.index) {
            return None;
        }
        match node.kind() {
            AstKind::VariableDeclarator(declarator) => {
                let init = declarator.init.as_ref()?;
                let Expression::ConditionalExpression(conditional) = init.without_parentheses()
                else {
                    return None;
                };
                contains_typeof_function(&conditional.test).then(|| {
                    let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                        return None;
                    };
                    identifier
                        .symbol_id
                        .get()
                        .map(|symbol| (symbol, conditional.span))
                })?
            }
            AstKind::AssignmentExpression(assignment) => {
                let Expression::ConditionalExpression(conditional) =
                    assignment.right.without_parentheses()
                else {
                    return None;
                };
                contains_typeof_function(&conditional.test).then(|| {
                    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left
                    else {
                        return None;
                    };
                    crate::analysis::identifier_symbol(identifier, context.scoping)
                        .map(|symbol| (symbol, conditional.span))
                })?
            }
            _ => None,
        }
    });
    let encode_symbol = encode.map(|(symbol, _)| symbol);
    let encode_value = encode.map(|(_, span)| span);

    let encode_selection = build_encode_selection(context, &site, encode_symbol);

    // The notify-last-chance expression.
    let notify_last_chance = context.nodes.iter().find_map(|node| {
        if !direct_node(&site, node.span(), &context.index) {
            return None;
        }
        let AstKind::LogicalExpression(logical) = node.kind() else {
            return None;
        };
        if logical.operator != LogicalOperator::Coalesce {
            return None;
        }
        if !matches!(
            logical.right.without_parentheses(),
            Expression::BooleanLiteral(literal) if literal.value
        ) {
            return None;
        }
        Some(NotifyLastChance {
            span: node.span().into(),
            source: source_of(context.source, node.span()),
            has_or_operand: matches!(
                logical.left.without_parentheses(),
                Expression::LogicalExpression(inner) if inner.operator == LogicalOperator::Or
            ),
            has_nullish: true,
            literal_true: true,
        })
    });

    // The snapshot boundary: `encode = encode(collector(filter))`.
    let snapshot = build_snapshot(context, &site, encode_symbol);

    // Content hash function symbol, from the toString-bearing field.
    let content_hash_function = context.nodes.iter().find_map(|node| {
        if !direct_node(&site, node.span(), &context.index) {
            return None;
        }
        let AstKind::CallExpression(call) = node.kind() else {
            return None;
        };
        if member_property(&call.callee).as_deref() != Some("toString") {
            return None;
        }
        let Expression::CallExpression(inner) = member_object(&call.callee)?.without_parentheses()
        else {
            return None;
        };
        let symbol = expression_symbol(&inner.callee, context.scoping)?;
        Some(binding_of(
            context.scoping,
            context.nodes,
            context.resolve(symbol),
        ))
    });

    let (fields, token_symbol) = build_fields(context, &site, object_symbol, encode_value);

    let include_debug_names_forced = context.nodes.iter().any(|node| {
        direct_node(&site, node.span(), &context.index)
            && matches!(
                node.kind(),
                AstKind::BooleanLiteral(literal) if !literal.value
            )
    });

    let callers = symbol_span_references(context, &site);
    let mut consumers = Vec::new();
    if let Some(WiringProperties {
        public_init: Some(init),
        ..
    }) = context.wiring
    {
        if within(
            site.span,
            Span::new(init.function.span.start, init.function.span.end),
        ) {
            consumers.push(context.ev(
                Span::new(init.function.span.start, init.function.span.end),
                "public init owns the envelope factory",
            ));
        }
    }
    let _ = token_symbol;
    let _ = content_hash_function;

    let mut local = Vec::new();
    if fields.is_empty() {
        local.push(fact(
            "envelope_fields",
            "no populated envelope fields were proven",
            Some(site.span.into()),
            true,
        ));
    }

    Some(EnvelopeProperties {
        function: function_of(context, &site, None),
        parameter,
        options_binding,
        fields,
        encode_selection,
        notify_last_chance,
        include_debug_names_forced,
        snapshot,
        content_hash_function,
        callers,
        consumers,
        unresolved: local,
    })
}

fn contains_typeof_function(expression: &Expression<'_>) -> bool {
    struct Find {
        typeof_seen: bool,
        function_literal: bool,
    }
    impl<'a> Visit<'a> for Find {
        fn visit_unary_expression(&mut self, unary: &UnaryExpression<'a>) {
            if unary.operator == UnaryOperator::Typeof {
                self.typeof_seen = true;
            }
            walk::walk_unary_expression(self, unary);
        }
        fn visit_string_literal(&mut self, literal: &StringLiteral<'a>) {
            if literal.value == "function" {
                self.function_literal = true;
            }
        }
    }
    let mut find = Find {
        typeof_seen: false,
        function_literal: false,
    };
    find.visit_expression(expression);
    find.typeof_seen && find.function_literal
}

fn build_encode_selection(
    context: &Context<'_, '_>,
    site: &FunctionSite,
    encode_symbol: Option<SymbolId>,
) -> Option<EncodeSelection> {
    for node in context.nodes.iter() {
        if !direct_node(site, node.span(), &context.index) {
            continue;
        }
        let conditional = match node.kind() {
            AstKind::VariableDeclarator(declarator) => declarator
                .init
                .as_ref()
                .map(Expression::without_parentheses)
                .and_then(as_conditional),
            AstKind::AssignmentExpression(assignment) => {
                as_conditional(assignment.right.without_parentheses())
            }
            _ => None,
        };
        let Some(conditional) = conditional else {
            continue;
        };
        if !contains_typeof_function(&conditional.test) {
            continue;
        }
        let fallback = expression_symbol(&conditional.alternate, context.scoping)
            .map(|symbol| binding_of(context.scoping, context.nodes, context.resolve(symbol)));
        let override_source = expression_symbol(&conditional.consequent, context.scoping)
            .map(|_| source_of(context.source, conditional.consequent.span()));
        let _ = encode_symbol;
        return Some(EncodeSelection {
            span: node.span().into(),
            source: source_of(context.source, node.span()),
            override_source,
            type_check: Some(context.ev(conditional.test.span(), "typeof-function guard")),
            fallback,
            return_unchecked: true,
        });
    }
    None
}

fn as_conditional<'a>(expression: &'a Expression<'a>) -> Option<&'a ConditionalExpression<'a>> {
    match expression.without_parentheses() {
        Expression::ConditionalExpression(conditional) => Some(conditional),
        _ => None,
    }
}

fn build_snapshot(
    context: &Context<'_, '_>,
    site: &FunctionSite,
    encode_symbol: Option<SymbolId>,
) -> Option<SnapshotBoundary> {
    let encode_symbol = encode_symbol?;
    for node in context.nodes.iter() {
        if !direct_node(site, node.span(), &context.index) {
            continue;
        }
        let call = match node.kind() {
            AstKind::VariableDeclarator(declarator) => declarator.init.as_ref().and_then(as_call),
            AstKind::AssignmentExpression(assignment) => as_call(&assignment.right),
            _ => None,
        };
        let Some(call) = call else {
            continue;
        };
        let target = expression_symbol(&call.callee, context.scoping);
        if target.map(|symbol| context.resolve(symbol)) != Some(context.resolve(encode_symbol)) {
            continue;
        }
        let inner = call
            .arguments
            .first()
            .and_then(|argument| argument.as_expression())
            .and_then(as_call);
        let collector = inner.and_then(|inner_call| {
            expression_symbol(&inner_call.callee, context.scoping)
                .map(|symbol| binding_of(context.scoping, context.nodes, context.resolve(symbol)))
        });
        return Some(SnapshotBoundary {
            collector,
            collector_call: inner
                .map(|inner_call| context.ev(inner_call.span, "snapshot collector call")),
            encoder_default: None,
            invocation: Some(context.ev(call.span, "encoder invocation on the snapshot")),
        });
    }
    None
}

fn as_call<'a>(expression: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    match expression.without_parentheses() {
        Expression::CallExpression(call) => Some(call),
        _ => None,
    }
}

fn build_fields(
    context: &Context<'_, '_>,
    site: &FunctionSite,
    object_symbol: Option<SymbolId>,
    encode_value: Option<Span>,
) -> (Vec<EnvelopeField>, Option<SymbolId>) {
    let Some(object_symbol) = object_symbol else {
        return (Vec::new(), None);
    };
    let object = context.resolve(object_symbol);

    // Discover the token operand from the hash concat, before classifying.
    let mut token_symbol = None;
    for node in context.nodes.iter() {
        if !direct_node(site, node.span(), &context.index) {
            continue;
        }
        let AstKind::CallExpression(call) = node.kind() else {
            continue;
        };
        if member_property(&call.callee).as_deref() != Some("toString") {
            continue;
        }
        let Some(Expression::CallExpression(inner)) =
            member_object(&call.callee).map(Expression::without_parentheses)
        else {
            continue;
        };
        let Some(argument) = inner.arguments.first().and_then(|arg| arg.as_expression()) else {
            continue;
        };
        if let Expression::BinaryExpression(binary) = argument.without_parentheses() {
            if binary.operator == BinaryOperator::Addition {
                let symbol = |operand: &Expression<'_>| {
                    expression_symbol(operand, context.scoping).map(|s| context.resolve(s))
                };
                token_symbol = if is_encoder_text(context, &binary.left, encode_value) {
                    symbol(&binary.right)
                } else if is_encoder_text(context, &binary.right, encode_value) {
                    symbol(&binary.left)
                } else {
                    None
                };
            }
        }
    }

    let mut fields = Vec::new();
    for node in context.nodes.iter() {
        if !direct_node(site, node.span(), &context.index) {
            continue;
        }
        let AstKind::AssignmentExpression(assignment) = node.kind() else {
            continue;
        };
        let (key, value) = match &assignment.left {
            AssignmentTarget::StaticMemberExpression(member) => {
                if expression_symbol(&member.object, context.scoping)
                    .map(|symbol| context.resolve(symbol))
                    != Some(object)
                {
                    continue;
                }
                (Some(member.property.name.to_string()), &assignment.right)
            }
            AssignmentTarget::ComputedMemberExpression(member) => {
                if expression_symbol(&member.object, context.scoping)
                    .map(|symbol| context.resolve(symbol))
                    != Some(object)
                {
                    continue;
                }
                let key = match member.expression.without_parentheses() {
                    Expression::StringLiteral(literal) => Some(literal.value.to_string()),
                    _ => None,
                };
                (key, &assignment.right)
            }
            _ => continue,
        };
        let role = classify_field(context, value, encode_value, token_symbol);
        let literal = match value.without_parentheses() {
            Expression::StringLiteral(literal) => Some(literal.value.to_string()),
            _ => None,
        };
        fields.push(EnvelopeField {
            key,
            role,
            value: context.ev(value.span(), "envelope field value"),
            literal,
        });
    }
    (fields, token_symbol)
}

fn classify_field(
    context: &Context<'_, '_>,
    value: &Expression<'_>,
    encode_value: Option<Span>,
    token_symbol: Option<SymbolId>,
) -> String {
    let value = value.without_parentheses();
    if let Expression::StringLiteral(_) = value {
        return "fixed_literal".to_string();
    }
    if let Expression::CallExpression(call) = value {
        if member_property(&call.callee).as_deref() == Some("toString") {
            return "content_hash".to_string();
        }
    }
    if let Expression::ConditionalExpression(_) = value {
        return "descriptor_default".to_string();
    }
    if is_encoder_text(context, value, encode_value) {
        return "encoder_text".to_string();
    }
    if let Some(symbol) = expression_symbol(value, context.scoping) {
        if token_symbol == Some(context.resolve(symbol)) {
            return "token".to_string();
        }
    }
    "unknown".to_string()
}

/// Whether `value` holds, at its own position, the result of calling the
/// selected encoder: a local whose single reaching definition is a call
/// whose callee, at the call, holds the encode selection `selection`.
///
/// Recovered programs coalesce locals (`v = encode(v(filter))` reuses the
/// snapshot function's local for the text, and `encode = encode(...)` reuses
/// the encoder's), so the binding alone does not say which value a read sees.
/// Anything the reaching definition cannot prove stays unclassified.
fn is_encoder_text(
    context: &Context<'_, '_>,
    value: &Expression<'_>,
    selection: Option<Span>,
) -> bool {
    let Some(selection) = selection else {
        return false;
    };
    let mut current = value;
    for _ in 0..8 {
        let Ok(definition) = expression_value_at_use(context.scoping, context.nodes, current)
        else {
            return false;
        };
        match definition.value.without_parentheses() {
            Expression::CallExpression(call) => {
                return expression_value_at_use(context.scoping, context.nodes, &call.callee)
                    .is_ok_and(|callee| callee.value.without_parentheses().span() == selection);
            }
            // `let text = encoded;` copies the value the alias read sees.
            Expression::Identifier(_) => current = definition.value,
            _ => return false,
        }
    }
    false
}

fn symbol_span_references(context: &Context<'_, '_>, site: &FunctionSite) -> Vec<OutputEvidence> {
    let Some(symbol) = site.symbol else {
        return Vec::new();
    };
    context
        .references(symbol)
        .into_iter()
        .map(|span| context.ev(span, "reference to the envelope factory"))
        .collect()
}

// ===========================================================================
// Chunking
// ===========================================================================

fn chunking(
    context: &Context<'_, '_>,
    unresolved: &mut Vec<OutputUnresolved>,
) -> Option<ChunkingProperties> {
    let mut candidates: Vec<FunctionSite> = context
        .index
        .functions
        .iter()
        .filter(|site| {
            if site.arity != 3 {
                return false;
            }
            let mut has_create_null = false;
            let mut has_while = false;
            let mut slices = 0usize;
            let mut computed_writes = 0usize;
            let mut char_at = 0usize;
            for node in context.nodes.iter() {
                if !direct_node(site, node.span(), &context.index) {
                    continue;
                }
                match node.kind() {
                    AstKind::CallExpression(call) => {
                        if object_create_null(call) {
                            has_create_null = true;
                        }
                        match method_call(context, call).map(|(name, _, _)| name) {
                            Some("slice") => slices += 1,
                            Some("charAt") => char_at += 1,
                            _ => {}
                        }
                    }
                    // Traces emit the continuation loop as `for (;;)` with a
                    // negated-test `break`.
                    AstKind::WhileStatement(_) | AstKind::ForStatement(_) => has_while = true,
                    AstKind::AssignmentExpression(assignment) => {
                        if matches!(
                            assignment.left,
                            AssignmentTarget::ComputedMemberExpression(_)
                        ) {
                            computed_writes += 1;
                        }
                    }
                    _ => {}
                }
            }
            has_create_null && has_while && slices >= 2 && computed_writes >= 1 && char_at == 0
        })
        .cloned()
        .collect();
    candidates.sort_by_key(|site| site.span.end - site.span.start);
    let site = match candidates.len() {
        0 => return None,
        1 => candidates.into_iter().next().unwrap(),
        _ => {
            unresolved.push(fact(
                "header_chunker",
                "more than one slice/continuation function matched; refusing to pick",
                None,
                true,
            ));
            return None;
        }
    };

    let mut name_parameter = None;
    let mut value_parameter = None;
    let mut size_parameter = None;
    let mut slice_calls = Vec::new();
    let mut continuation_keys = Vec::new();
    for node in context.nodes.iter() {
        if !direct_node(&site, node.span(), &context.index) {
            continue;
        }
        match node.kind() {
            AstKind::CallExpression(call)
                if method_call(context, call).is_some_and(|(name, _, _)| name == "slice") =>
            {
                slice_calls.push(context.ev(node.span(), "value slice"))
            }
            AstKind::AssignmentExpression(assignment) => {
                if let AssignmentTarget::ComputedMemberExpression(member) = &assignment.left {
                    if let Some(index) = context.slot_index(&member.object, &site) {
                        name_parameter = Some(index);
                    }
                    if let Some(index) = context.slot_index(&member.expression, &site) {
                        name_parameter = name_parameter.or(Some(index));
                    }
                    if contains_addition(&member.expression) {
                        continuation_keys.push(context.ev(node.span(), "indexed continuation key"));
                    }
                }
            }
            _ => {}
        }
    }
    // Assign roles from actual use: the sliced expression is the value formal.
    for node in context.nodes.iter() {
        if !direct_node(&site, node.span(), &context.index) {
            continue;
        }
        if let AstKind::CallExpression(call) = node.kind() {
            if let Some(("slice", object, arguments)) = method_call(context, call) {
                value_parameter = context.slot_index(object, &site).or(value_parameter);
                if let Some(argument) = arguments.get(1).and_then(|arg| arg.as_expression()) {
                    size_parameter = context.slot_index(argument, &site).or(size_parameter);
                }
            }
        }
    }

    let callers = chunk_calls(context, &site);

    Some(ChunkingProperties {
        function: function_of(context, &site, None),
        name_parameter,
        value_parameter,
        size_parameter,
        slice_calls,
        continuation_keys,
        callers,
        unresolved: Vec::new(),
    })
}

fn member_object<'e, 'a>(expression: &'e Expression<'a>) -> Option<&'e Expression<'a>> {
    match expression.without_parentheses() {
        Expression::StaticMemberExpression(member) => Some(&member.object),
        Expression::ComputedMemberExpression(member) => {
            computed_member_name(member).map(|_| &member.object)
        }
        _ => None,
    }
}

fn contains_addition(expression: &Expression<'_>) -> bool {
    struct Find {
        found: bool,
    }
    impl<'a> Visit<'a> for Find {
        fn visit_binary_expression(&mut self, binary: &BinaryExpression<'a>) {
            if binary.operator == BinaryOperator::Addition {
                self.found = true;
            }
            walk::walk_binary_expression(self, binary);
        }
    }
    let mut find = Find { found: false };
    find.visit_expression(expression);
    find.found
}

fn chunk_calls(context: &Context<'_, '_>, site: &FunctionSite) -> Vec<ChunkCall> {
    let Some(symbol) = site.symbol else {
        return Vec::new();
    };
    let resolved = context.resolve(symbol);
    context
        .nodes
        .iter()
        .filter_map(|node| {
            let AstKind::CallExpression(call) = node.kind() else {
                return None;
            };
            let target = expression_symbol(&call.callee, context.scoping)?;
            (context.resolve(target) == resolved).then(|| ChunkCall {
                span: node.span().into(),
                source: source_of(context.source, node.span()),
                size_literal: call
                    .arguments
                    .get(2)
                    .and_then(|argument| argument.as_expression())
                    .and_then(numeric_literal),
            })
        })
        .collect()
}

// ===========================================================================
// Carriers
// ===========================================================================

fn carriers(
    context: &Context<'_, '_>,
    envelope_symbol: Option<u32>,
    chunker_symbol: Option<u32>,
    chunking: &Option<ChunkingProperties>,
    diagnostics: &mut Vec<OutputDiagnostic>,
) -> Vec<CarrierProperties> {
    let envelope = envelope_symbol.and_then(|ordinal| symbol_by_ordinal(context, ordinal));
    let chunker = chunker_symbol.and_then(|ordinal| symbol_by_ordinal(context, ordinal));
    if envelope.is_none() && chunker.is_none() {
        return Vec::new();
    }

    let mut candidates: Vec<FunctionSite> = context
        .index
        .functions
        .iter()
        .filter(|site| {
            envelope.is_some_and(|symbol| context.subtree_references(site, symbol))
                || chunker.is_some_and(|symbol| context.subtree_references(site, symbol))
        })
        .cloned()
        .collect();
    candidates.sort_by_key(|site| site.span.end - site.span.start);

    // Keep only innermost candidates.
    let mut selected: Vec<FunctionSite> = Vec::new();
    for candidate in candidates {
        if selected
            .iter()
            .any(|chosen| chosen.span != candidate.span && within(candidate.span, chosen.span))
        {
            continue;
        }
        selected.push(candidate);
    }

    let mut carriers = Vec::new();
    for site in &selected {
        let root = carrier_root(context, site, &selected);
        let mut anchors = scan_anchors(context.nodes, site.span);
        if let Some(root) = &root {
            anchors.extend(scan_anchors(context.nodes, root.span));
        }
        let kind = classify(&anchors);
        if kind == CarrierKind::Unknown {
            diagnostics.push(OutputDiagnostic {
                code: "carrier_unknown".to_string(),
                message: format!(
                    "a pipeline consumer at {}..{} has no recognized host anchor",
                    site.span.start, site.span.end
                ),
                span: Some(site.span.into()),
            });
        }

        let anchor_span = root.as_ref().map_or(site.span, |root| root.span);
        let anchor_evidence: Vec<OutputEvidence> = anchors
            .iter()
            .map(|anchor| context.ev(anchor.span, format!("host anchor: {}", anchor.kind)))
            .collect();
        let captured_originals = captured_originals(context, anchor_span);
        let chunk_calls = if envelope.is_some() || chunker.is_some() {
            chunking
                .as_ref()
                .map(|_| chunk_calls_in(context, anchor_span, chunker))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let lifecycle = lifecycle(context, anchor_span);
        let config = config_objects(context, anchor_span);
        let anomalies = anomalies(context, anchor_span, root.as_ref());
        let uses_envelope = envelope.is_some_and(|symbol| context.subtree_references(site, symbol));
        let uses_chunker = chunker.is_some_and(|symbol| context.subtree_references(site, symbol));

        let confidence = if kind != CarrierKind::Unknown {
            Confidence::High
        } else {
            Confidence::Medium
        };

        let unresolved = Vec::new();
        carriers.push(CarrierProperties {
            kind,
            function: function_of(context, site, None),
            root_function: root.as_ref().map(|root| function_of(context, root, None)),
            anchors: anchor_evidence,
            captured_originals,
            uses_envelope,
            uses_chunker,
            chunk_calls,
            lifecycle,
            config,
            anomalies,
            confidence,
            unresolved,
        });
    }
    carriers
}

fn symbol_by_ordinal(context: &Context<'_, '_>, ordinal: u32) -> Option<SymbolId> {
    context
        .index
        .functions
        .iter()
        .filter_map(|site| site.symbol)
        .find(|symbol| symbol.index() as u32 == ordinal)
}

fn carrier_root(
    context: &Context<'_, '_>,
    site: &FunctionSite,
    selected: &[FunctionSite],
) -> Option<FunctionSite> {
    let mut ancestors: Vec<FunctionSite> = context
        .index
        .functions
        .iter()
        .filter(|candidate| candidate.span != site.span && within(candidate.span, site.span))
        .cloned()
        .collect();
    ancestors.sort_by_key(|candidate| std::cmp::Reverse(candidate.span.end - candidate.span.start));
    ancestors.into_iter().find(|candidate| {
        !selected.iter().any(|other| {
            other.span != site.span
                && other.span != candidate.span
                && within(candidate.span, other.span)
        })
    })
}

fn captured_originals(context: &Context<'_, '_>, within: Span) -> Vec<OutputBinding> {
    let mut bindings = Vec::new();
    for node in context.nodes.iter() {
        if !within_span(within, node.span()) {
            continue;
        }
        let (symbol, init) = match node.kind() {
            AstKind::VariableDeclarator(declarator) => {
                let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                    continue;
                };
                (identifier.symbol_id.get(), declarator.init.as_ref())
            }
            AstKind::AssignmentExpression(assignment) => {
                let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left
                else {
                    continue;
                };
                (
                    crate::analysis::identifier_symbol(identifier, context.scoping),
                    Some(&assignment.right),
                )
            }
            _ => continue,
        };
        let Some(symbol) = symbol else {
            continue;
        };
        let Some(init) = init else {
            continue;
        };
        if member_chain_has(init, "originals") {
            bindings.push(binding_of(context.scoping, context.nodes, symbol));
        }
    }
    bindings.sort_by_key(|binding| binding.symbol);
    bindings.dedup_by_key(|binding| binding.symbol);
    bindings
}

fn member_chain_has(expression: &Expression<'_>, name: &str) -> bool {
    struct Find<'n> {
        name: &'n str,
        found: bool,
    }
    impl<'a> Visit<'a> for Find<'_> {
        fn visit_static_member_expression(&mut self, member: &StaticMemberExpression<'a>) {
            if member.property.name == self.name {
                self.found = true;
            }
            walk::walk_static_member_expression(self, member);
        }
        fn visit_computed_member_expression(&mut self, member: &ComputedMemberExpression<'a>) {
            if computed_member_name(member) == Some(self.name) {
                self.found = true;
            }
            walk::walk_computed_member_expression(self, member);
        }
    }
    let mut find = Find { name, found: false };
    find.visit_expression(expression);
    find.found
}

fn within_span(outer: Span, inner: Span) -> bool {
    inner.start >= outer.start && inner.end <= outer.end
}

fn chunk_calls_in(
    context: &Context<'_, '_>,
    within: Span,
    chunker: Option<SymbolId>,
) -> Vec<ChunkCall> {
    let Some(chunker) = chunker else {
        return Vec::new();
    };
    let resolved = context.resolve(chunker);
    context
        .nodes
        .iter()
        .filter_map(|node| {
            if !within_span(within, node.span()) {
                return None;
            }
            let AstKind::CallExpression(call) = node.kind() else {
                return None;
            };
            let target = expression_symbol(&call.callee, context.scoping)?;
            (context.resolve(target) == resolved).then(|| ChunkCall {
                span: node.span().into(),
                source: source_of(context.source, node.span()),
                size_literal: call
                    .arguments
                    .get(2)
                    .and_then(|argument| argument.as_expression())
                    .and_then(numeric_literal),
            })
        })
        .collect()
}

fn lifecycle(context: &Context<'_, '_>, within: Span) -> Option<CarrierLifecycle> {
    let mut returns_lifecycle_object = false;
    let mut no_op_return = false;
    for node in context.nodes.iter() {
        if !within_span(within, node.span()) {
            continue;
        }
        let AstKind::ReturnStatement(statement) = node.kind() else {
            continue;
        };
        let Some(argument) = statement.argument.as_ref() else {
            continue;
        };
        let values = object_property_values(context, within, argument);
        if values.len() < 2 {
            continue;
        }
        if !values
            .iter()
            .all(|value| resolve_function(context, value).is_some() || is_inline_function(value))
        {
            continue;
        }
        returns_lifecycle_object = true;
        if values
            .iter()
            .all(|value| function_value_is_empty(context, value))
        {
            no_op_return = true;
        }
    }
    returns_lifecycle_object.then_some(CarrierLifecycle {
        returns_lifecycle_object,
        no_op_return,
    })
}

/// Values of the object returned by a lifecycle return: either the properties
/// of an object literal, or the properties of a bound object (its literal
/// initializer plus any member assignments) when the return is an identifier.
fn object_property_values<'a>(
    context: &'a Context<'_, '_>,
    within: Span,
    expression: &'a Expression<'a>,
) -> Vec<&'a Expression<'a>> {
    let expression = expression.without_parentheses();
    match expression {
        Expression::ObjectExpression(object) => object
            .properties
            .iter()
            .filter_map(|property| match property {
                ObjectPropertyKind::ObjectProperty(property) => Some(&property.value),
                _ => None,
            })
            .collect(),
        Expression::Identifier(_) => {
            let Some(symbol) = expression_symbol(expression, context.scoping) else {
                return Vec::new();
            };
            let object = context.resolve(symbol);
            let mut values = Vec::new();
            for node in context.nodes.iter() {
                if !within_span(within, node.span()) {
                    continue;
                }
                match node.kind() {
                    AstKind::VariableDeclarator(declarator) => {
                        let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
                            continue;
                        };
                        if identifier.symbol_id.get().map(|s| context.resolve(s)) != Some(object) {
                            continue;
                        }
                        if let Some(Expression::ObjectExpression(literal)) = declarator
                            .init
                            .as_ref()
                            .map(Expression::without_parentheses)
                        {
                            values.extend(literal.properties.iter().filter_map(|property| {
                                match property {
                                    ObjectPropertyKind::ObjectProperty(property) => {
                                        Some(&property.value)
                                    }
                                    _ => None,
                                }
                            }));
                        }
                    }
                    AstKind::AssignmentExpression(assignment) => {
                        let (target, value) = match &assignment.left {
                            AssignmentTarget::StaticMemberExpression(member) => (
                                expression_symbol(&member.object, context.scoping),
                                Some(&assignment.right),
                            ),
                            AssignmentTarget::ComputedMemberExpression(member) => (
                                expression_symbol(&member.object, context.scoping),
                                Some(&assignment.right),
                            ),
                            _ => (None, None),
                        };
                        if target.map(|s| context.resolve(s)) == Some(object) {
                            if let Some(value) = value {
                                values.push(value);
                            }
                        }
                    }
                    _ => {}
                }
            }
            values
        }
        _ => Vec::new(),
    }
}

fn is_inline_function(expression: &Expression<'_>) -> bool {
    matches!(
        expression.without_parentheses(),
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
    )
}

fn resolve_function<'a>(
    context: &'a Context<'_, '_>,
    expression: &Expression<'_>,
) -> Option<&'a FunctionSite> {
    let symbol = expression_symbol(expression, context.scoping)?;
    let resolved = context.resolve(symbol);
    context
        .index
        .functions
        .iter()
        .find(|site| site.symbol.map(|s| context.resolve(s)) == Some(resolved))
}

fn function_value_is_empty(context: &Context<'_, '_>, expression: &Expression<'_>) -> bool {
    if let Some(site) = resolve_function(context, expression) {
        return function_body_empty(context, site);
    }
    match expression.without_parentheses() {
        Expression::FunctionExpression(function) => function
            .body
            .as_ref()
            .is_some_and(|body| is_no_op_body(body)),
        Expression::ArrowFunctionExpression(function) => is_no_op_body(&function.body),
        _ => false,
    }
}

fn function_body_empty(context: &Context<'_, '_>, site: &FunctionSite) -> bool {
    context.nodes.iter().any(|node| {
        node.span() == site.span
            && match node.kind() {
                AstKind::Function(function) => function
                    .body
                    .as_ref()
                    .is_some_and(|body| is_no_op_body(body)),
                AstKind::ArrowFunctionExpression(function) => is_no_op_body(&function.body),
                _ => false,
            }
    })
}

/// An empty body, or one that only returns `undefined` (traces end every
/// function with an explicit `return void 0;`).
fn is_no_op_body(body: &FunctionBody<'_>) -> bool {
    match body.statements.as_slice() {
        [] => true,
        [Statement::ReturnStatement(statement)] => statement
            .argument
            .as_ref()
            .is_none_or(|argument| is_void_zero(argument)),
        _ => false,
    }
}

fn config_objects(context: &Context<'_, '_>, within: Span) -> Vec<OutputEvidence> {
    let mut config = Vec::new();
    for node in context.nodes.iter() {
        if !within_span(within, node.span()) {
            continue;
        }
        let AstKind::ObjectExpression(object) = node.kind() else {
            continue;
        };
        let literals = object
            .properties
            .iter()
            .filter(|property| match property {
                ObjectPropertyKind::ObjectProperty(property) => matches!(
                    property.value.without_parentheses(),
                    Expression::NumericLiteral(_) | Expression::StringLiteral(_)
                ),
                _ => false,
            })
            .count();
        if literals >= 2 && config.len() < 8 {
            config.push(context.ev(node.span(), "carrier configuration literals"));
        }
    }
    config
}

fn anomalies(
    context: &Context<'_, '_>,
    within: Span,
    root: Option<&FunctionSite>,
) -> Vec<OutputEvidence> {
    let mut anomalies = Vec::new();

    // The as-cleaned fetch no-op: a returned lifecycle whose init and teardown
    // bodies are both empty.
    if lifecycle(context, within).is_some_and(|lifecycle| lifecycle.no_op_return) {
        anomalies.push(context.ev(
            root.map_or(within, |root| root.span),
            "carrier returns a no-op init/teardown lifecycle",
        ));
    }

    for node in context.nodes.iter() {
        if !within_span(within, node.span()) {
            continue;
        }
        match node.kind() {
            AstKind::AssignmentExpression(assignment) => {
                // `X = void 0` where X is later used as a call argument.
                if is_void_zero(&assignment.right) {
                    if let AssignmentTarget::AssignmentTargetIdentifier(identifier) =
                        &assignment.left
                    {
                        if let Some(symbol) =
                            crate::analysis::identifier_symbol(identifier, context.scoping)
                        {
                            if symbol_used_as_argument(context, symbol, within) {
                                anomalies.push(context.ev(
                                    node.span(),
                                    "resolved target replaced with undefined before use",
                                ));
                            }
                        }
                    }
                }
                // `<test> = true; if (<test>)` marker overwrite.
                if matches!(
                    assignment.right.without_parentheses(),
                    Expression::BooleanLiteral(literal) if literal.value
                ) {
                    if let AssignmentTarget::AssignmentTargetIdentifier(identifier) =
                        &assignment.left
                    {
                        if let Some(symbol) =
                            crate::analysis::identifier_symbol(identifier, context.scoping)
                        {
                            if symbol_used_as_if_test(context, symbol, within) {
                                anomalies.push(
                                    context.ev(
                                        node.span(),
                                        "marker test overwritten with literal true",
                                    ),
                                );
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    anomalies
}

fn symbol_used_as_argument(context: &Context<'_, '_>, symbol: SymbolId, within: Span) -> bool {
    let resolved = context.resolve(symbol);
    context.nodes.iter().any(|node| {
        if !within_span(within, node.span()) {
            return false;
        }
        let AstKind::CallExpression(call) = node.kind() else {
            return false;
        };
        call.arguments.iter().any(|argument| {
            argument
                .as_expression()
                .and_then(|expression| expression_symbol(expression, context.scoping))
                .map(|s| context.resolve(s))
                == Some(resolved)
        })
    })
}

fn symbol_used_as_if_test(context: &Context<'_, '_>, symbol: SymbolId, within: Span) -> bool {
    let resolved = context.resolve(symbol);
    context.nodes.iter().any(|node| {
        if !within_span(within, node.span()) {
            return false;
        }
        let AstKind::IfStatement(statement) = node.kind() else {
            return false;
        };
        expression_symbol(&statement.test, context.scoping).map(|s| context.resolve(s))
            == Some(resolved)
    })
}

// ===========================================================================
// Context links
// ===========================================================================

fn context_links(
    context: &Context<'_, '_>,
    encoder: &Option<EncoderFactory>,
    envelope: &Option<EnvelopeProperties>,
) -> Vec<ContextLink> {
    let mut links = Vec::new();

    // Main scope.
    if let Some(wiring) = context.wiring {
        links.push(ContextLink {
            role: "main".to_string(),
            context_source: "wiring".to_string(),
            report_span: Some(wiring.main.span.clone()),
            matched: true,
        });
        // Snapshot collector: wiring names it; confirm the envelope calls it.
        if let Some(snapshot) = wiring.stores.snapshot.as_ref() {
            let snapshot_span = snapshot.span.clone();
            let matched = envelope
                .as_ref()
                .and_then(|envelope| envelope.snapshot.as_ref())
                .and_then(|snapshot| snapshot.collector.as_ref())
                .is_some_and(|collector| collector.declaration == snapshot_span);
            links.push(ContextLink {
                role: "snapshot_collector".to_string(),
                context_source: "wiring".to_string(),
                report_span: Some(snapshot_span),
                matched,
            });
        }
    }

    // Source hash: encoding names it; confirm the envelope hashes with it.
    if let Some(encoding) = context.encoding {
        if let Some(hash) = encoding.seed_link.hash.as_ref() {
            let hash_span = hash.span.clone();
            let matched = envelope
                .as_ref()
                .and_then(|envelope| envelope.content_hash_function.as_ref())
                .is_some_and(|function| function.declaration == hash_span);
            links.push(ContextLink {
                role: "content_hash".to_string(),
                context_source: "encoding".to_string(),
                report_span: Some(hash_span),
                matched,
            });
        }
    }

    let _ = encoder;
    links
}
