//! Bounded, static extraction of per-seed runtime configuration.
//!
//! This module is a data hand-off for a future ShapeVM kernel/tracer.  It is
//! intentionally separate from readable deobfuscated JavaScript: every fact
//! keeps the raw-source span, source identity, evidence, and an explicit
//! extracted/unknown/unsupported status.  No JavaScript or WebAssembly is
//! executed here.

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use crate::transforms::deob::pipeline::{self, PipelineError, PipelineReport};
use crate::vm_patterns::{recognize_nested_dispatch, NestedDispatch, NestedDispatchPolicy};
use crate::{
    analysis,
    kernel::decoder_proof::{
        prove_runtime_decoder, DecoderProof, STANDARD_ALPHABET, URL_SAFE_ALPHABET,
    },
    kernel::wasm_module::{self, Base64Policy, ModuleLimits, ValidationStatus},
    operations::catalogue,
    util::{fnv1a64_hex, json_string},
};
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::{GetSpan, Span};

pub const SCHEMA_VERSION: &str = "shape-runtime-config/v1";
pub const ANALYSIS_VERSION: &str = "shape-oxc-deob/runtime-config-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtractionLimits {
    pub max_source_bytes: usize,
    pub max_decoded_bytes: usize,
    pub max_literal_elements: usize,
    pub max_literal_depth: usize,
    pub max_artifacts: usize,
}

impl Default for ExtractionLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 16 * 1024 * 1024,
            max_decoded_bytes: 8 * 1024 * 1024,
            max_literal_elements: 100_000,
            max_literal_depth: 64,
            max_artifacts: 10_000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeConfigOptions {
    pub input_mode: InputMode,
    pub limits: ExtractionLimits,
}

impl Default for RuntimeConfigOptions {
    fn default() -> Self {
        Self {
            input_mode: InputMode::RawJavaScript,
            limits: ExtractionLimits::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    RawJavaScript,
    DeobfuscatedJavaScript,
    /// A saved pipeline report is being used without the original raw source.
    /// `source` is the report's declared capture identity; only
    /// `artifact_source` is freshly verified from `report.output`.
    SavedReport,
}

impl InputMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::RawJavaScript => "raw-javascript",
            Self::DeobfuscatedJavaScript => "deobfuscated-javascript",
            Self::SavedReport => "saved-report",
        }
    }
}

#[derive(Debug)]
pub enum RuntimeConfigError {
    InvalidInput(String),
    Parse(String),
    Pipeline(PipelineError),
    Limit { what: String, limit: usize },
}

impl fmt::Display for RuntimeConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(s) => write!(f, "invalid runtime-config input: {s}"),
            Self::Parse(s) => write!(f, "runtime-config parse failed: {s}"),
            Self::Pipeline(e) => write!(f, "runtime-config pipeline failed: {e}"),
            Self::Limit { what, limit } => {
                write!(f, "runtime-config limit exceeded for {what} ({limit})")
            }
        }
    }
}
impl std::error::Error for RuntimeConfigError {}
impl From<PipelineError> for RuntimeConfigError {
    fn from(value: PipelineError) -> Self {
        Self::Pipeline(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSpan {
    pub start: u32,
    pub end: u32,
}
impl From<Span> for SourceSpan {
    fn from(value: Span) -> Self {
        Self {
            start: value.start,
            end: value.end,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    pub rule: String,
    pub message: String,
    pub span: Option<SourceSpan>,
    pub span_source: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FactStatus {
    Extracted,
    Unknown,
    Unsupported,
}
impl FactStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Extracted => "extracted",
            Self::Unknown => "unknown",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArtifactKind {
    WasmModule,
    WasmMemoryImage,
    JsBytecodeImage,
    DispatchTable,
    OperandFormat,
    StringTable,
    /// An immutable array of number literals, such as `vmNumberTable`.
    NumberTable,
    DecoderParameters,
    ThreadDescriptor,
    InitialMemorySlots,
    OperationTable,
    OperationRecipeReference,
}
impl ArtifactKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::WasmModule => "wasm-module",
            Self::WasmMemoryImage => "wasm-memory-image",
            Self::JsBytecodeImage => "js-bytecode-image",
            Self::DispatchTable => "dispatch-table",
            Self::OperandFormat => "operand-format",
            Self::StringTable => "string-table",
            Self::NumberTable => "number-table",
            Self::DecoderParameters => "decoder-parameters",
            Self::ThreadDescriptor => "thread-descriptor",
            Self::InitialMemorySlots => "initial-memory-slots",
            Self::OperationTable => "operation-table",
            Self::OperationRecipeReference => "operation-recipe-reference",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeValue {
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
    Array(Vec<RuntimeValue>),
    Object(BTreeMap<String, RuntimeValue>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeArtifact {
    pub id: String,
    pub kind: ArtifactKind,
    pub status: FactStatus,
    pub binding: Option<String>,
    pub consumer: Option<String>,
    pub span: SourceSpan,
    /// `input` for explicit deobfuscated input, `artifact` for spans in a
    /// standardized report output. This is deliberately separate from the
    /// input identity below.
    pub span_source: String,
    pub byte_length: Option<usize>,
    pub bytes: Option<Vec<u8>>,
    pub value: Option<RuntimeValue>,
    pub metadata: BTreeMap<String, String>,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceIdentity {
    pub algorithm: String,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeConfig {
    pub schema_version: String,
    pub analysis_version: String,
    pub input_mode: InputMode,
    pub coordinate_system: String,
    /// Identity of the bytes supplied by the caller (the report input).
    pub source: SourceIdentity,
    /// Identity of the standardized source that was parsed for facts.
    pub artifact_source: SourceIdentity,
    pub artifacts: Vec<RuntimeArtifact>,
    pub diagnostics: Vec<String>,
}

impl RuntimeConfig {
    pub fn json(&self) -> String {
        to_json(self)
    }
    pub fn stable_json(&self) -> String {
        self.json()
    }
}

/// Extract a raw seed by first obtaining the existing standardized pipeline report.
pub fn extract(source: &str) -> Result<RuntimeConfig, RuntimeConfigError> {
    let limits = ExtractionLimits::default();
    if source.len() > limits.max_source_bytes {
        return Err(RuntimeConfigError::Limit {
            what: "source bytes".into(),
            limit: limits.max_source_bytes,
        });
    }
    preparse_nesting_guard(source, limits.max_literal_depth)?;
    let report = pipeline::deobfuscate_with_report(source)?;
    from_report(source, &report, RuntimeConfigOptions::default())
}

/// Extract from an already deobfuscated source.  This mode is explicit because
/// source spans then refer to the deobfuscated coordinate system.
pub fn extract_deobfuscated(
    source: &str,
    limits: ExtractionLimits,
) -> Result<RuntimeConfig, RuntimeConfigError> {
    if source.is_empty() {
        return Err(RuntimeConfigError::InvalidInput("source is empty".into()));
    }
    scan(
        source,
        None,
        SourceIdentity {
            algorithm: "fnv1a64".into(),
            digest: fnv1a64_hex(source.as_bytes()),
        },
        RuntimeConfigOptions {
            input_mode: InputMode::DeobfuscatedJavaScript,
            limits,
        },
    )
}

/// Primary integration seam for callers that already own the standardized report.
pub fn from_report(
    source: &str,
    report: &PipelineReport,
    options: RuntimeConfigOptions,
) -> Result<RuntimeConfig, RuntimeConfigError> {
    if source.is_empty() {
        return Err(RuntimeConfigError::InvalidInput("source is empty".into()));
    }
    if source.len() > options.limits.max_source_bytes {
        return Err(RuntimeConfigError::Limit {
            what: "source bytes".into(),
            limit: options.limits.max_source_bytes,
        });
    }
    preparse_nesting_guard(source, options.limits.max_literal_depth)?;
    if report.input_hash != fnv1a64_hex(source.as_bytes()) {
        return Err(RuntimeConfigError::InvalidInput(
            "pipeline report input_hash does not match supplied source".into(),
        ));
    }
    if report.output.len() > options.limits.max_source_bytes {
        return Err(RuntimeConfigError::Limit {
            what: "standardized artifact source bytes".into(),
            limit: options.limits.max_source_bytes,
        });
    }
    scan(
        &report.output,
        Some(report),
        SourceIdentity {
            algorithm: "fnv1a64".into(),
            digest: fnv1a64_hex(source.as_bytes()),
        },
        options,
    )
}

/// Extract runtime facts from a complete saved pipeline report without
/// requiring the original raw capture.  The report's `input_hash` remains a
/// declared capture identity; the standardized `output` is parsed and
/// reproved now, and receives its own freshly computed artifact identity.
pub fn from_saved_report(
    report: &PipelineReport,
    options: RuntimeConfigOptions,
) -> Result<RuntimeConfig, RuntimeConfigError> {
    validate_saved_report(report, options.limits)?;
    scan(
        &report.output,
        Some(report),
        SourceIdentity {
            algorithm: "fnv1a64".into(),
            digest: report.input_hash.clone(),
        },
        RuntimeConfigOptions {
            input_mode: InputMode::SavedReport,
            ..options
        },
    )
}

/// Validate the untrusted envelope and coordinate metadata of a saved report
/// before any report-only consumer uses its claimed facts.  The preparation
/// APIs still re-prove the role/table/span relationships against `output`.
pub(crate) fn validate_saved_report(
    report: &PipelineReport,
    limits: ExtractionLimits,
) -> Result<(), RuntimeConfigError> {
    if report.output.is_empty() {
        return Err(RuntimeConfigError::InvalidInput(
            "saved report output is empty".into(),
        ));
    }
    if report.input_hash.len() != 16
        || !report
            .input_hash
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(RuntimeConfigError::InvalidInput(
            "saved report input_hash is not a 16-digit FNV-1a identity".into(),
        ));
    }
    if report.output.len() > limits.max_source_bytes {
        return Err(RuntimeConfigError::Limit {
            what: "standardized artifact source bytes".into(),
            limit: limits.max_source_bytes,
        });
    }
    preparse_nesting_guard(&report.output, limits.max_literal_depth)?;
    let valid_span = |start: u32, end: u32, label: &str| {
        if start > end {
            Err(RuntimeConfigError::InvalidInput(format!(
                "saved report {label} span {start}..{end} is reversed"
            )))
        } else {
            Ok(())
        }
    };
    for fact in &report.extraction {
        valid_span(fact.span.start, fact.span.end, "extraction")?;
        if let (Some(length), Some(bytes)) = (fact.length, fact.decoded_bytes.as_ref()) {
            if length != bytes.len() {
                return Err(RuntimeConfigError::InvalidInput(format!(
                    "saved report extraction length {length} disagrees with decoded bytes {}",
                    bytes.len()
                )));
            }
        }
    }
    for mapping in &report.binding_mappings {
        valid_span(mapping.span.0, mapping.span.1, "binding mapping")?;
    }
    for mapping in &report.property_mappings {
        valid_span(mapping.span.0, mapping.span.1, "property mapping")?;
    }
    for table in &report.proven_operation_tables {
        if table.initializer_elements == 0 || table.initializer_functions == 0 {
            return Err(RuntimeConfigError::InvalidInput(
                "saved report operation-table shape metadata is empty".into(),
            ));
        }
    }
    Ok(())
}

struct Binding {
    name: String,
    span: Span,
    value: Option<RuntimeValue>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum DecoderCallStatus {
    Candidate,
    Finalized,
}
struct DecoderCall {
    span: Span,
    symbol: SymbolId,
    encoded: Option<String>,
    decoded: Result<Vec<u8>, String>,
    destination: Option<MemorySink>,
    has_destination_argument: bool,
    status: DecoderCallStatus,
}
#[derive(Clone)]
struct MemorySink {
    memory: SymbolId,
    offset: usize,
    length: Option<usize>,
}
struct NewUse {
    span: Span,
    module: bool,
    argument_span: Option<Span>,
    argument_symbol: Option<SymbolId>,
    direct_decoder_span: Option<Span>,
}
struct DispatchUse {
    table: SymbolId,
    bytecode: SymbolId,
    record: SymbolId,
    state_field: String,
    span: Span,
    proven_consumer: bool,
}

struct Scanner<'a> {
    scoping: &'a Scoping,
    limits: ExtractionLimits,
    bindings: Vec<Binding>,
    symbols: BTreeMap<SymbolId, usize>,
    decoder_symbols: BTreeSet<SymbolId>,
    decoder_spans: BTreeMap<SymbolId, Span>,
    decoder_proofs: BTreeMap<SymbolId, DecoderProof>,
    candidate_decoder_symbols: BTreeSet<SymbolId>,
    candidate_decoder_spans: BTreeMap<SymbolId, Span>,
    calls: Vec<DecoderCall>,
    decoded_bindings: BTreeMap<SymbolId, Span>,
    new_uses: Vec<NewUse>,
    dispatch_uses: Vec<DispatchUse>,
    assignments: BTreeSet<SymbolId>,
    memory_allocations: BTreeMap<SymbolId, usize>,
    property_mutations: BTreeSet<SymbolId>,
    dynamic_scope: bool,
    memory_limit_exceeded: bool,
}

impl<'a> Scanner<'a> {
    fn new(scoping: &'a Scoping, limits: ExtractionLimits) -> Self {
        Self {
            scoping,
            limits,
            bindings: Vec::new(),
            symbols: BTreeMap::new(),
            decoder_symbols: BTreeSet::new(),
            decoder_spans: BTreeMap::new(),
            decoder_proofs: BTreeMap::new(),
            candidate_decoder_symbols: BTreeSet::new(),
            candidate_decoder_spans: BTreeMap::new(),
            calls: Vec::new(),
            decoded_bindings: BTreeMap::new(),
            new_uses: Vec::new(),
            dispatch_uses: Vec::new(),
            assignments: BTreeSet::new(),
            memory_allocations: BTreeMap::new(),
            property_mutations: BTreeSet::new(),
            dynamic_scope: false,
            memory_limit_exceeded: false,
        }
    }
    fn binding(&self, symbol: SymbolId) -> Option<&Binding> {
        self.symbols
            .get(&symbol)
            .and_then(|i| self.bindings.get(*i))
    }
    fn symbol_of(&self, expression: &Expression<'_>) -> Option<SymbolId> {
        let Expression::Identifier(id) = expression else {
            return None;
        };
        analysis::identifier_symbol(id, self.scoping)
    }
    fn is_global(&self, expression: &Expression<'_>, name: &str) -> bool {
        matches!(expression, Expression::Identifier(id) if id.name == name && id.reference_id.get().is_some() && analysis::identifier_symbol(id, self.scoping).is_none())
    }
}

impl<'a> Visit<'a> for Scanner<'a> {
    fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
        if let (Some(id), Some(init), Some(symbol)) = (
            simple_binding(&it.id),
            it.init.as_ref(),
            simple_binding(&it.id).and_then(|b| b.symbol_id.get()),
        ) {
            let index = self.bindings.len();
            self.symbols.insert(symbol, index);
            self.bindings.push(Binding {
                name: id.name.to_string(),
                span: it.span,
                value: static_value(init, &self.limits).ok(),
            });
            if matches!(
                init,
                Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
            ) && decoder_prefilter(init, self.scoping).is_some()
            {
                self.candidate_decoder_symbols.insert(symbol);
                self.candidate_decoder_spans.insert(symbol, init.span());
            }
            if let Some(capacity) = wasm_memory_capacity(init, self) {
                if capacity <= self.limits.max_decoded_bytes {
                    self.memory_allocations.insert(symbol, capacity);
                } else {
                    self.memory_limit_exceeded = true;
                }
            }
            if let Expression::CallExpression(call) = init {
                if self.symbol_of(&call.callee).is_some() {
                    self.decoded_bindings.insert(symbol, call.span);
                }
            }
        }
        walk::walk_variable_declarator(self, it);
    }
    fn visit_function(&mut self, it: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        if let Some(id) = &it.id {
            if let Some(symbol) = id.symbol_id.get() {
                if decoder_prefilter_function(it, self.scoping).is_some() {
                    self.candidate_decoder_symbols.insert(symbol);
                    self.candidate_decoder_spans.insert(symbol, it.span);
                }
            }
        }
        walk::walk_function(self, it, flags);
    }
    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        let direct_eval = matches!(&it.callee, Expression::Identifier(id) if id.name == "eval" && analysis::identifier_symbol(id, self.scoping).is_none());
        if direct_eval {
            self.dynamic_scope = true;
        }
        if let Expression::Identifier(id) = &it.callee {
            if let Some(symbol) = analysis::identifier_symbol(id, self.scoping) {
                let decoded = it
                    .arguments
                    .first()
                    .and_then(Argument::as_expression)
                    .and_then(string_literal)
                    .map_or_else(
                        || Err("decoder consumer argument is not a string literal".into()),
                        |s| decode_standard_base64(&s, self.limits.max_decoded_bytes),
                    );
                let destination = it
                    .arguments
                    .get(1)
                    .and_then(Argument::as_expression)
                    .and_then(|e| byte_array_destination(e, self));
                if it
                    .arguments
                    .first()
                    .and_then(Argument::as_expression)
                    .is_some()
                    || destination.is_some()
                {
                    self.calls.push(DecoderCall {
                        span: it.span,
                        symbol,
                        encoded: it
                            .arguments
                            .first()
                            .and_then(Argument::as_expression)
                            .and_then(string_literal),
                        decoded,
                        destination,
                        has_destination_argument: it.arguments.get(1).is_some(),
                        status: DecoderCallStatus::Candidate,
                    });
                }
            }
        }
        walk::walk_call_expression(self, it);
    }
    fn visit_with_statement(&mut self, _it: &WithStatement<'a>) {
        self.dynamic_scope = true;
        walk::walk_with_statement(self, _it);
    }
    fn visit_new_expression(&mut self, it: &NewExpression<'a>) {
        let module = is_wasm_module_new(it, self);
        let argument = it.arguments.first().and_then(Argument::as_expression);
        let argument_span = argument.map(GetSpan::span);
        let argument_symbol = argument.and_then(|e| self.symbol_of(e));
        let direct_decoder_span = argument.and_then(|e| match e {
            Expression::CallExpression(c) => Some(c.span),
            _ => None,
        });
        self.new_uses.push(NewUse {
            span: it.span,
            module,
            argument_span,
            argument_symbol,
            direct_decoder_span,
        });
        walk::walk_new_expression(self, it);
    }
    fn visit_assignment_expression(&mut self, it: &AssignmentExpression<'a>) {
        if let AssignmentTarget::AssignmentTargetIdentifier(id) = &it.left {
            if let Some(symbol) = analysis::identifier_symbol(id, self.scoping) {
                self.assignments.insert(symbol);
            }
        } else {
            let object = match &it.left {
                AssignmentTarget::StaticMemberExpression(member) => Some(&member.object),
                AssignmentTarget::ComputedMemberExpression(member) => Some(&member.object),
                _ => None,
            };
            if let Some(symbol) = object.and_then(|expression| self.symbol_of(expression)) {
                self.property_mutations.insert(symbol);
            }
        }
        walk::walk_assignment_expression(self, it);
    }
}

fn simple_binding<'a>(pattern: &'a BindingPattern<'a>) -> Option<&'a BindingIdentifier<'a>> {
    match pattern {
        BindingPattern::BindingIdentifier(id) => Some(id),
        _ => None,
    }
}

fn is_symbol(expression: &Expression<'_>, symbol: SymbolId, scoping: &Scoping) -> bool {
    analysis::expression_symbol(expression, scoping) == Some(symbol)
}

fn string_literal(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::StringLiteral(v) => Some(v.value.to_string()),
        _ => None,
    }
}
#[allow(dead_code)]
#[derive(Debug, Clone)]
/// Discovery-only prefilter. It never certifies a decoder; the exact
/// `runtime_decoder::prove_runtime_decoder` matcher is the certification gate.
struct DecoderPrefilter {
    rule: &'static str,
    alphabet: &'static str,
    destination_parameter: bool,
}

struct DecoderProbe<'a> {
    scoping: &'a Scoping,
    loops: Vec<Span>,
    writes: Vec<(SymbolId, Span)>,
    returns: Vec<(Option<SymbolId>, Span)>,
    bitwise_arithmetic: bool,
    shift_left_values: BTreeSet<i64>,
    shift_right_values: BTreeSet<i64>,
    alphabet: Option<&'static str>,
    parameter_symbols: BTreeSet<SymbolId>,
    parameter_reference: bool,
}

impl<'a> DecoderProbe<'a> {
    fn new(scoping: &'a Scoping) -> Self {
        Self {
            scoping,
            loops: Vec::new(),
            writes: Vec::new(),
            returns: Vec::new(),
            bitwise_arithmetic: false,
            shift_left_values: BTreeSet::new(),
            shift_right_values: BTreeSet::new(),
            alphabet: None,
            parameter_symbols: BTreeSet::new(),
            parameter_reference: false,
        }
    }
    fn finish(self, span: Span) -> Option<DecoderPrefilter> {
        let alphabet = self.alphabet?;
        if span.size() > 32_768
            || self.loops.is_empty()
            || !self.bitwise_arithmetic
            || ![2, 4, 6]
                .iter()
                .all(|value| self.shift_left_values.contains(value))
            || ![2, 4]
                .iter()
                .all(|value| self.shift_right_values.contains(value))
            || self.writes.is_empty()
        {
            return None;
        }
        let output_symbols: BTreeSet<_> = self.writes.iter().map(|(symbol, _)| *symbol).collect();
        let has_returned_output = self.returns.iter().any(|(symbol, return_span)| {
            symbol.is_some_and(|symbol| output_symbols.contains(&symbol))
                && self
                    .loops
                    .iter()
                    .any(|loop_span| loop_span.end <= return_span.start)
                && self.writes.iter().any(|(write_symbol, write_span)| {
                    output_symbols.contains(write_symbol)
                        && self.loops.iter().any(|loop_span| {
                            loop_span.start <= write_span.start && write_span.end <= loop_span.end
                        })
                })
        });
        has_returned_output.then_some(DecoderPrefilter {
            rule: "base64-output-loop",
            alphabet,
            destination_parameter: self.parameter_reference,
        })
    }
}

impl<'a> Visit<'a> for DecoderProbe<'a> {
    fn visit_function(&mut self, _it: &Function<'a>, _flags: oxc_syntax::scope::ScopeFlags) {
        // Do not mix a nested function's dead decoder-looking code into the
        // proof for its parent.
    }
    fn visit_arrow_function_expression(&mut self, _it: &ArrowFunctionExpression<'a>) {}
    fn visit_string_literal(&mut self, it: &StringLiteral<'a>) {
        if it.value == STANDARD_ALPHABET {
            self.alphabet = Some(STANDARD_ALPHABET);
        } else if it.value == URL_SAFE_ALPHABET {
            self.alphabet.get_or_insert(URL_SAFE_ALPHABET);
        }
    }
    fn visit_logical_expression(&mut self, it: &LogicalExpression<'a>) {
        self.parameter_reference |= analysis::expression_symbol(&it.left, self.scoping)
            .is_some_and(|symbol| self.parameter_symbols.contains(&symbol))
            || analysis::expression_symbol(&it.right, self.scoping)
                .is_some_and(|symbol| self.parameter_symbols.contains(&symbol));
        walk::walk_logical_expression(self, it);
    }
    fn visit_for_statement(&mut self, it: &ForStatement<'a>) {
        self.loops.push(it.span);
        walk::walk_for_statement(self, it);
    }
    fn visit_while_statement(&mut self, it: &WhileStatement<'a>) {
        self.loops.push(it.span);
        walk::walk_while_statement(self, it);
    }
    fn visit_do_while_statement(&mut self, it: &DoWhileStatement<'a>) {
        self.loops.push(it.span);
        walk::walk_do_while_statement(self, it);
    }
    fn visit_binary_expression(&mut self, it: &BinaryExpression<'a>) {
        self.bitwise_arithmetic |= matches!(
            it.operator,
            BinaryOperator::ShiftLeft
                | BinaryOperator::ShiftRight
                | BinaryOperator::ShiftRightZeroFill
                | BinaryOperator::BitwiseOR
                | BinaryOperator::BitwiseAnd
        );
        if let Expression::NumericLiteral(value) = &it.right {
            match it.operator {
                BinaryOperator::ShiftLeft => {
                    self.shift_left_values.insert(value.value as i64);
                }
                BinaryOperator::ShiftRight | BinaryOperator::ShiftRightZeroFill => {
                    self.shift_right_values.insert(value.value as i64);
                }
                _ => {}
            }
        }
        walk::walk_binary_expression(self, it);
    }
    fn visit_assignment_expression(&mut self, it: &AssignmentExpression<'a>) {
        if let AssignmentTarget::ComputedMemberExpression(member) = &it.left {
            if let Some(symbol) = analysis::expression_symbol(&member.object, self.scoping) {
                self.writes.push((symbol, it.span));
            }
        }
        walk::walk_assignment_expression(self, it);
    }
    fn visit_return_statement(&mut self, it: &ReturnStatement<'a>) {
        self.returns.push((
            it.argument
                .as_ref()
                .and_then(|argument| analysis::expression_symbol(argument, self.scoping)),
            it.span,
        ));
        walk::walk_return_statement(self, it);
    }
}

fn decoder_prefilter_function(
    function: &Function<'_>,
    scoping: &Scoping,
) -> Option<DecoderPrefilter> {
    let mut probe = DecoderProbe::new(scoping);
    for parameter in &function.params.items {
        if let Some(identifier) = simple_binding(&parameter.pattern) {
            if let Some(symbol) = identifier.symbol_id.get() {
                probe.parameter_symbols.insert(symbol);
            }
        }
    }
    function.body.as_deref().map(|body| {
        probe.visit_function_body(body);
        probe.finish(function.span)
    })?
}

fn decoder_prefilter(expression: &Expression<'_>, scoping: &Scoping) -> Option<DecoderPrefilter> {
    match expression {
        Expression::FunctionExpression(function) => decoder_prefilter_function(function, scoping),
        Expression::ArrowFunctionExpression(function) => {
            let mut probe = DecoderProbe::new(scoping);
            for parameter in &function.params.items {
                if let Some(identifier) = simple_binding(&parameter.pattern) {
                    if let Some(symbol) = identifier.symbol_id.get() {
                        probe.parameter_symbols.insert(symbol);
                    }
                }
            }
            probe.visit_function_body(&function.body);
            probe.finish(function.span)
        }
        _ => None,
    }
}

fn decode_standard_base64(input: &str, limit: usize) -> Result<Vec<u8>, String> {
    wasm_module::decode_base64(input, Base64Policy::Standard { limit })
}

/// Decode a literal whose decoder proof matched the URL-safe alphabet.  The
/// standard `+/` characters are not data for that template.
fn decode_url_safe_base64(input: &str, limit: usize) -> Result<Vec<u8>, String> {
    wasm_module::decode_url_safe_strict(input, limit)
}

/// Name the base64 alphabet carried by a [`DecoderProof`].
fn alphabet_name(alphabet: &str) -> &'static str {
    match alphabet {
        STANDARD_ALPHABET => "standard",
        URL_SAFE_ALPHABET => "URL-safe",
        _ => "unknown",
    }
}

struct LiteralBudget {
    remaining: usize,
    depth: usize,
}

fn static_value(
    expression: &Expression<'_>,
    limits: &ExtractionLimits,
) -> Result<RuntimeValue, String> {
    let mut budget = LiteralBudget {
        remaining: limits.max_literal_elements,
        depth: limits.max_literal_depth,
    };
    static_value_inner(expression, &mut budget)
}

fn static_value_inner(
    expression: &Expression<'_>,
    budget: &mut LiteralBudget,
) -> Result<RuntimeValue, String> {
    if budget.depth == 0 {
        return Err("literal nesting depth limit exceeded".into());
    }
    match expression {
        Expression::StringLiteral(v) => Ok(RuntimeValue::String(v.value.to_string())),
        Expression::NumericLiteral(v) if v.value.is_finite() => Ok(RuntimeValue::Number(v.value)),
        Expression::NumericLiteral(_) => Err("non-finite numeric literal is unsupported".into()),
        Expression::BooleanLiteral(v) => Ok(RuntimeValue::Boolean(v.value)),
        Expression::NullLiteral(_) => Ok(RuntimeValue::Null),
        Expression::UnaryExpression(v)
            if matches!(
                v.operator,
                UnaryOperator::UnaryPlus | UnaryOperator::UnaryNegation
            ) =>
        {
            static_number(expression)
                .map(RuntimeValue::Number)
                .ok_or_else(|| "unary literal is not a finite number".into())
        }
        Expression::ArrayExpression(a) => {
            if a.elements.len() > budget.remaining {
                return Err("literal element limit exceeded".into());
            }
            budget.remaining -= a.elements.len();
            budget.depth -= 1;
            let mut values = Vec::with_capacity(a.elements.len());
            for element in &a.elements {
                let expression = match element {
                    ArrayExpressionElement::Elision(_) => {
                        return Err("array hole is unsupported".into())
                    }
                    ArrayExpressionElement::SpreadElement(_) => {
                        return Err("array spread is unsupported".into())
                    }
                    other => other.to_expression(),
                };
                values.push(static_value_inner(expression, budget)?);
            }
            budget.depth += 1;
            Ok(RuntimeValue::Array(values))
        }
        Expression::ObjectExpression(o) => {
            if o.properties.len() > budget.remaining {
                return Err("literal element limit exceeded".into());
            }
            budget.remaining -= o.properties.len();
            budget.depth -= 1;
            let mut map = BTreeMap::new();
            for p in &o.properties {
                let ObjectPropertyKind::ObjectProperty(p) = p else {
                    return Err("object method/accessor is unsupported".into());
                };
                if p.computed || p.kind != PropertyKind::Init {
                    return Err("computed, method, or accessor property is unsupported".into());
                };
                let key = match &p.key {
                    PropertyKey::StaticIdentifier(i) => i.name.to_string(),
                    PropertyKey::StringLiteral(s) => s.value.to_string(),
                    PropertyKey::NumericLiteral(n) if n.value.is_finite() => n.value.to_string(),
                    PropertyKey::NumericLiteral(_) => {
                        return Err("non-finite object key is unsupported".into())
                    }
                    _ => return Err("dynamic object key is unsupported".into()),
                };
                if key == "__proto__" || map.contains_key(&key) {
                    return Err("special or duplicate object key is unsupported".into());
                }
                map.insert(key, static_value_inner(&p.value, budget)?);
            }
            budget.depth += 1;
            Ok(RuntimeValue::Object(map))
        }
        _ => Err("expression is not a bounded literal".into()),
    }
}
fn static_number(expression: &Expression<'_>) -> Option<f64> {
    match expression {
        Expression::NumericLiteral(v) if v.value.is_finite() => Some(v.value),
        Expression::UnaryExpression(v)
            if matches!(
                v.operator,
                UnaryOperator::UnaryPlus | UnaryOperator::UnaryNegation
            ) =>
        {
            static_number(&v.argument).and_then(|n| {
                let value = if v.operator == UnaryOperator::UnaryNegation {
                    -n
                } else {
                    n
                };
                value.is_finite().then_some(value)
            })
        }
        _ => None,
    }
}
fn nested_dispatch(expression: &Expression<'_>, scoping: &Scoping) -> Option<NestedDispatch> {
    recognize_nested_dispatch(expression, scoping, NestedDispatchPolicy::StrictThisPc)
}

fn is_record_component(
    expression: &Expression<'_>,
    record: SymbolId,
    index: i32,
    scoping: &Scoping,
) -> bool {
    let Expression::ComputedMemberExpression(member) = expression else {
        return false;
    };
    is_symbol(&member.object, record, scoping)
        && matches!(&member.expression, Expression::NumericLiteral(value) if value.value == f64::from(index))
}

fn prove_dispatch_consumer(
    function: &Function<'_>,
    initializer_span: Span,
    table: SymbolId,
    bytecode: SymbolId,
    record: SymbolId,
    state_field: &str,
    scoping: &Scoping,
) -> bool {
    if function.r#async || function.generator {
        return false;
    }
    let Some(body) = function.body.as_deref() else {
        return false;
    };
    if !body.directives.is_empty() {
        return false;
    }
    // The JavaScript VM helper has one transparent block wrapper around its body.
    // Only unwrap that non-executable wrapper; the executable statement list
    // must still be exactly the complete three-statement template below.
    let statements = match body.statements.as_slice() {
        [Statement::BlockStatement(block)] => block.body.as_slice(),
        statements => statements,
    };
    let [declaration, assignment, returned] = statements else {
        return false;
    };
    let Statement::VariableDeclaration(declaration) = declaration else {
        return false;
    };
    if declaration.kind != VariableDeclarationKind::Var
        || declaration.declare
        || declaration.declarations.len() != 1
    {
        return false;
    }
    let declarator = &declaration.declarations[0];
    let Some(binding) = simple_binding(&declarator.id) else {
        return false;
    };
    if binding.symbol_id.get() != Some(record) {
        return false;
    }
    let Some(initializer) = declarator.init.as_ref() else {
        return false;
    };
    if initializer.span() != initializer_span {
        return false;
    }
    let Some(dispatch) = nested_dispatch(initializer, scoping) else {
        return false;
    };
    if dispatch.table != table
        || dispatch.bytecode != Some(bytecode)
        || dispatch.state_property.as_deref() != Some(state_field)
    {
        return false;
    }
    let Statement::ExpressionStatement(assignment) = assignment else {
        return false;
    };
    let Expression::AssignmentExpression(assignment) = &assignment.expression else {
        return false;
    };
    let AssignmentTarget::StaticMemberExpression(target) = &assignment.left else {
        return false;
    };
    if assignment.operator != AssignmentOperator::Assign
        || !matches!(&target.object, Expression::ThisExpression(_))
        || target.property.name != state_field
        || !is_record_component(&assignment.right, record, 0, scoping)
    {
        return false;
    }
    let Statement::ReturnStatement(returned) = returned else {
        return false;
    };
    returned
        .argument
        .as_ref()
        .is_some_and(|argument| is_record_component(argument, record, 1, scoping))
}

fn is_dispatch_literal(value: &RuntimeValue) -> bool {
    let RuntimeValue::Array(rows) = value else {
        return false;
    };
    !rows.is_empty()
        && rows.iter().all(|row| {
            let RuntimeValue::Array(records) = row else {
                return false;
            };
            records.iter().all(|record| {
                let RuntimeValue::Array(pair) = record else {
                    return false;
                };
                pair.len() == 2
                    && pair
                        .iter()
                        .all(|item| matches!(item, RuntimeValue::Number(number) if number.is_finite() && number.fract() == 0.0 && *number >= 0.0))
            })
        })
}
fn is_wasm_module_new(expression: &NewExpression<'_>, scanner: &Scanner<'_>) -> bool {
    matches!(&expression.callee, Expression::StaticMemberExpression(m) if m.property.name == "Module" && scanner.is_global(&m.object, "WebAssembly"))
}
fn wasm_memory_capacity(expression: &Expression<'_>, scanner: &Scanner<'_>) -> Option<usize> {
    let Expression::NewExpression(new) = expression else {
        return None;
    };
    if !matches!(&new.callee, Expression::StaticMemberExpression(member)
        if member.property.name == "Memory" && scanner.is_global(&member.object, "WebAssembly"))
    {
        return None;
    }
    let options = new.arguments.first().and_then(Argument::as_expression)?;
    let RuntimeValue::Object(fields) = static_value(options, &scanner.limits).ok()? else {
        return None;
    };
    let initial = match fields.get("initial") {
        Some(RuntimeValue::Number(value)) if value.is_finite() && *value >= 0.0 => *value as usize,
        _ => return None,
    };
    initial.checked_mul(65_536)
}

fn byte_array_destination(
    expression: &Expression<'_>,
    scanner: &Scanner<'_>,
) -> Option<MemorySink> {
    let Expression::NewExpression(n) = expression else {
        return None;
    };
    let Expression::Identifier(_) = &n.callee else {
        return None;
    };
    if !scanner.is_global(&n.callee, "Uint8Array") {
        return None;
    }
    let Expression::StaticMemberExpression(member) =
        n.arguments.first().and_then(Argument::as_expression)?
    else {
        return None;
    };
    if member.property.name != "buffer" {
        return None;
    }
    let memory = scanner.symbol_of(&member.object)?;
    let offset = n
        .arguments
        .get(1)
        .and_then(Argument::as_expression)
        .and_then(static_number)
        .filter(|value| value.is_finite() && *value >= 0.0 && value.fract() == 0.0)
        .map(|value| value as usize)
        .unwrap_or(0);
    let length = n
        .arguments
        .get(2)
        .and_then(Argument::as_expression)
        .and_then(static_number)
        .filter(|value| value.is_finite() && *value >= 0.0 && value.fract() == 0.0)
        .map(|value| value as usize);
    Some(MemorySink {
        memory,
        offset,
        length,
    })
}

fn finalize_decoder_proofs(source: &str, scanner: &mut Scanner<'_>) {
    let mut cache: BTreeMap<(u32, u32), Option<DecoderProof>> = BTreeMap::new();
    for (symbol, span) in scanner.candidate_decoder_spans.clone() {
        // A proof belongs to this exact lexical declaration. A reassigned
        // binding or duplicate declaration cannot inherit a proof found for
        // another runtime value with the same spelling.
        if scanner.assignments.contains(&symbol)
            || scanner.scoping.symbol_declarations(symbol).count() != 1
        {
            continue;
        }
        let key = (span.start, span.end);
        let proof = cache
            .entry(key)
            .or_insert_with(|| prove_runtime_decoder(source, span))
            .to_owned();
        if let Some(proof) = proof {
            if !scanner.assignments.contains(&symbol) {
                scanner.decoder_symbols.insert(symbol);
                scanner.decoder_spans.insert(symbol, span);
                scanner.decoder_proofs.insert(symbol, proof);
            }
        }
    }
    let candidate_symbols = scanner.candidate_decoder_symbols.clone();
    let mut consumer_call_spans = BTreeSet::new();
    for use_ in &scanner.new_uses {
        if let Some(span) = use_.direct_decoder_span {
            consumer_call_spans.insert((span.start, span.end));
        }
        if let Some(symbol) = use_.argument_symbol {
            if let Some(span) = scanner.decoded_bindings.get(&symbol) {
                consumer_call_spans.insert((span.start, span.end));
            }
        }
    }
    scanner.calls = scanner
        .calls
        .drain(..)
        .filter(|candidate| {
            candidate.status == DecoderCallStatus::Candidate
                && (candidate_symbols.contains(&candidate.symbol)
                    || candidate.destination.is_some()
                    || candidate.encoded.is_some()
                    || consumer_call_spans.contains(&(candidate.span.start, candidate.span.end)))
        })
        .map(|candidate| {
            let proof = scanner.decoder_proofs.get(&candidate.symbol);
            let proven_destination =
                proof.and_then(|proof| proof.destination_parameter_index) == Some(1);
            let DecoderCall {
                span,
                symbol,
                encoded,
                decoded: candidate_decoded,
                destination,
                has_destination_argument,
                ..
            } = candidate;
            let decoded = match (proof, encoded.as_deref()) {
                (None, _) => Err(
                    "exact decoder proof unavailable; encoded literal retained as unknown".into(),
                ),
                (Some(proof), Some(encoded)) if proof.alphabet == URL_SAFE_ALPHABET => {
                    // The scan-time decode used the standard alphabet, which
                    // rejects URL-safe literals.  Decode again with the
                    // alphabet the proof accepted; `decode_url_safe_strict`
                    // already refuses padding and the standard `+/` pair, so
                    // `proof.accepts_padding` cannot silently apply here.
                    decode_url_safe_base64(encoded, scanner.limits.max_decoded_bytes)
                }
                (Some(proof), Some(encoded)) if encoded.contains('=') && !proof.accepts_padding => {
                    match &candidate_decoded {
                        Err(error) => Err(error.clone()),
                        Ok(_) => Err(format!(
                            "{} decoder proof rejects padded consumer input",
                            alphabet_name(proof.alphabet)
                        )),
                    }
                }
                (Some(_), _) => candidate_decoded,
            };
            DecoderCall {
                span,
                symbol,
                encoded,
                decoded,
                destination: proven_destination.then_some(destination).flatten(),
                has_destination_argument,
                status: DecoderCallStatus::Finalized,
            }
        })
        .collect();
}

fn wasm_info(bytes: &[u8]) -> (bool, Vec<String>, Vec<String>, bool) {
    let Ok(summary) = wasm_module::inspect_module(bytes, ModuleLimits::for_input(bytes)) else {
        return (false, Vec::new(), Vec::new(), false);
    };
    let ValidationStatus::Valid = summary.validation else {
        return (false, Vec::new(), Vec::new(), false);
    };
    // Config historically flattened every import/export iterator error and
    // still reported the readable ABI.  The neutral summary preserves that;
    // only the validation gate above can reject the module.
    let imports = summary
        .abi
        .imports
        .iter()
        .map(|import| format!("{}::{}", import.namespace, import.name))
        .collect();
    let exports = summary
        .abi
        .exports
        .iter()
        .filter(|export| export.kind.is_config_export())
        .map(|export| export.name.clone())
        .collect();
    let memory = summary.abi.single_imported_memory || summary.abi.defined_memory;
    (true, imports, exports, memory)
}

fn scan(
    source: &str,
    report: Option<&PipelineReport>,
    source_identity: SourceIdentity,
    options: RuntimeConfigOptions,
) -> Result<RuntimeConfig, RuntimeConfigError> {
    if source.len() > options.limits.max_source_bytes {
        return Err(RuntimeConfigError::Limit {
            what: "source bytes".into(),
            limit: options.limits.max_source_bytes,
        });
    }
    preparse_nesting_guard(source, options.limits.max_literal_depth)?;
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source).map_err(RuntimeConfigError::Parse)?;
    let (scoping, _) = analysis::analyze(&program);
    let mut scanner = Scanner::new(&scoping, options.limits);
    scanner.visit_program(&program);
    finalize_decoder_proofs(source, &mut scanner);
    let mut artifacts = Vec::new();
    let mut diagnostics = Vec::new();
    if scanner.dynamic_scope {
        diagnostics.push("dynamic scope from direct eval or with makes lexical runtime relationships unknown; decoder and consumer certification declined".into());
        scanner.decoder_symbols.clear();
        scanner.decoder_spans.clear();
        scanner.decoder_proofs.clear();
        scanner.calls.clear();
    }
    if scanner.memory_limit_exceeded {
        diagnostics.push(format!(
            "a WebAssembly.Memory initial capacity exceeded max_decoded_bytes {}; image certification declined",
            options.limits.max_decoded_bytes
        ));
    }
    let mut ids = 0usize;
    let present_kinds = RefCell::new(BTreeSet::new());
    let mut push = |mut artifact: RuntimeArtifact| {
        if ids < options.limits.max_artifacts {
            artifact.id = format!("artifact-{ids:05}");
            present_kinds.borrow_mut().insert(artifact.kind);
            artifacts.push(artifact);
            ids += 1;
        } else {
            diagnostics.push(format!(
                "artifact limit {} reached",
                options.limits.max_artifacts
            ));
        }
    };
    for (symbol, span) in &scanner.decoder_spans {
        let proof = scanner.decoder_proofs.get(symbol);
        let alphabet = proof.map_or(STANDARD_ALPHABET, |proof| proof.alphabet);
        let mut metadata = BTreeMap::new();
        metadata.insert("alphabet".into(), alphabet.into());
        metadata.insert("padding".into(), "rejected-by-proof".into());
        metadata.insert(
            "consumer_validation".into(),
            format!("strict {} alphabet", alphabet_name(alphabet)),
        );
        if let Some(proof) = proof {
            metadata.insert("proof_rule".into(), proof.rule.into());
            metadata.insert("proof_alphabet".into(), proof.alphabet.into());
            metadata.insert(
                "declaration_identity".into(),
                format!("symbol-{:?}", symbol),
            );
            metadata.insert(
                "destination_parameter_index".into(),
                proof
                    .destination_parameter_index
                    .map_or("none".into(), |index| index.to_string()),
            );
            let consumers = scanner
                .calls
                .iter()
                .filter(|call| call.symbol == *symbol)
                .map(|call| format!("{}..{}", call.span.start, call.span.end))
                .collect::<Vec<_>>();
            metadata.insert("consumer_references".into(), consumers.join(","));
        }
        let decoder_status = if scanner.assignments.contains(symbol) {
            FactStatus::Unknown
        } else {
            FactStatus::Extracted
        };
        push(artifact(
            ArtifactKind::DecoderParameters,
            decoder_status,
            scanner.binding(*symbol).map(|b| b.name.clone()),
            *span,
            metadata,
            None,
            None,
            vec![evidence(
                "decoder-alphabet",
                if decoder_status != FactStatus::Extracted {
                    "decoder body is proven, but its binding is reassigned; runtime relationship is unknown"
                } else if alphabet == URL_SAFE_ALPHABET {
                    "URL-safe -_ alphabet and bounded output loop proven in decoder body"
                } else {
                    "standard +/ alphabet and bounded output loop proven in decoder body"
                },
                Some(*span),
            )],
        ));
    }
    for use_ in &scanner.new_uses {
        if use_.module {
            if let Some(span) = use_.argument_span {
                let symbol = use_.argument_symbol;
                let binding = symbol.and_then(|s| scanner.binding(s));
                let direct = use_
                    .direct_decoder_span
                    .and_then(|span| scanner.calls.iter().find(|x| x.span == span));
                let binding_stable = symbol.is_none_or(|s| {
                    !scanner.assignments.contains(&s) && !scanner.property_mutations.contains(&s)
                });
                let bound = binding_stable
                    .then(|| symbol.and_then(|s| scanner.decoded_bindings.get(&s)))
                    .flatten()
                    .and_then(|span| scanner.calls.iter().find(|call| call.span == *span));
                let linked_call = direct.or(bound);
                let encoded_literal = linked_call.and_then(|call| call.encoded.clone());
                let item = linked_call
                    .filter(|call| !call.has_destination_argument)
                    .map(|call| (call.span, call.decoded.clone()));
                let (span, result, binding_name) = item.map_or_else(
                    || {
                        linked_call.map_or(
                            (
                                span,
                                Err("module input is not a tracked decoder literal".into()),
                                binding.map(|b| b.name.clone()),
                            ),
                            |call| {
                                (
                                    call.span,
                                    Err(if call.has_destination_argument {
                                        "decoder destination argument returns a destination-backed view; full returned view is not modeled".into()
                                    } else {
                                        "module input is not a tracked decoder literal".into()
                                    }),
                                    binding.map(|b| b.name.clone()),
                                )
                            },
                        )
                    },
                    |(s, r)| (s, r, binding.map(|b| b.name.clone())),
                );
                let mut metadata = BTreeMap::new();
                if let Some(encoded) = encoded_literal {
                    metadata.insert("encoded_literal".into(), encoded);
                }
                let (status, bytes, evidence_text) = match result {
                    Ok(bytes) => {
                        let (valid, imports, exports, memory) = wasm_info(&bytes);
                        metadata.insert("validated".into(), valid.to_string());
                        metadata.insert("imports".into(), imports.join(","));
                        metadata.insert("exports".into(), exports.join(","));
                        metadata.insert("memory".into(), memory.to_string());
                        (
                            if valid {
                                FactStatus::Extracted
                            } else {
                                FactStatus::Unsupported
                            },
                            valid.then_some(bytes.clone()),
                            if valid {
                                "WebAssembly.Module consumer and validated module bytes".to_string()
                            } else {
                                "decoded bytes are not a validated WebAssembly module".to_string()
                            },
                        )
                    }
                    Err(e) => (FactStatus::Unknown, None, e),
                };
                push(artifact(
                    ArtifactKind::WasmModule,
                    status,
                    binding_name,
                    span,
                    metadata,
                    bytes,
                    None,
                    vec![evidence(
                        "wasm-module-consumer",
                        evidence_text,
                        Some(use_.span),
                    )],
                ));
            }
        }
    }
    for call in &scanner.calls {
        if !call.has_destination_argument {
            continue;
        }
        let mut metadata = BTreeMap::new();
        let (status, bytes, text) = match (&call.destination, &call.decoded) {
            (Some(sink), Ok(bytes)) => {
                let capacity = scanner.memory_allocations.get(&sink.memory).copied();
                let view_length = sink
                    .length
                    .or_else(|| capacity.and_then(|v| v.checked_sub(sink.offset)));
                let memory_mutated = scanner.assignments.contains(&sink.memory)
                    || scanner.property_mutations.contains(&sink.memory);
                metadata.insert(
                    "sink_identity".into(),
                    format!("memory-symbol-{:?}", sink.memory),
                );
                metadata.insert("view_offset_bytes".into(), sink.offset.to_string());
                metadata.insert(
                    "view_length_bytes".into(),
                    view_length.map_or("unknown".into(), |v| v.to_string()),
                );
                metadata.insert(
                    "initial_capacity_bytes".into(),
                    capacity.map_or("unknown".into(), |v| v.to_string()),
                );
                if memory_mutated {
                    (FactStatus::Unknown, None, "WebAssembly.Memory binding or property was mutated; exact initial image is retained only as an unproven candidate".into())
                } else if let Some(length) = view_length.filter(|_| capacity.is_some()) {
                    if sink
                        .offset
                        .checked_add(length)
                        .is_some_and(|end| end <= capacity.unwrap_or(0))
                        && bytes.len() <= length
                    {
                        (FactStatus::Extracted, Some(bytes.clone()), "decoder call writes a bounded image into proven WebAssembly.Memory.buffer; image is distinct from WASM modules".to_string())
                    } else {
                        (
                            FactStatus::Unknown,
                            None,
                            "decoded bytes do not fit the proven WebAssembly.Memory view"
                                .to_string(),
                        )
                    }
                } else {
                    (
                        FactStatus::Unknown,
                        None,
                        "typed-array sink is not linked to a bounded WebAssembly.Memory allocation"
                            .to_string(),
                    )
                }
            }
            (Some(_), Err(error)) => (FactStatus::Unknown, None, error.clone()),
            (None, _) => (
                FactStatus::Unknown,
                None,
                "decoder destination is not a proven WebAssembly.Memory.buffer Uint8Array view"
                    .into(),
            ),
        };
        if let Some(encoded) = &call.encoded {
            metadata.insert("encoded_literal".into(), encoded.clone());
        }
        push(artifact(
            ArtifactKind::WasmMemoryImage,
            status,
            scanner.binding(call.symbol).map(|b| b.name.clone()),
            call.span,
            metadata,
            bytes,
            None,
            vec![evidence("decoder-destination", text, Some(call.span))],
        ));
    }
    let mut table_symbols = BTreeSet::new();
    let mut bytecode_symbols = BTreeSet::new();
    struct DispatchVisitor<'a> {
        scoping: &'a Scoping,
        uses: Vec<DispatchUse>,
    }
    impl<'a> Visit<'a> for DispatchVisitor<'a> {
        fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
            if let Some(init) = &it.init {
                if let Some(dispatch) = nested_dispatch(init, self.scoping) {
                    let Some(bytecode) = dispatch.bytecode else {
                        return;
                    };
                    let Some(state_field) = dispatch.state_property else {
                        return;
                    };
                    if let Some(record) = simple_binding(&it.id).and_then(|id| id.symbol_id.get()) {
                        self.uses.push(DispatchUse {
                            table: dispatch.table,
                            bytecode,
                            record,
                            state_field,
                            span: init.span(),
                            proven_consumer: false,
                        });
                    }
                }
            }
            walk::walk_variable_declarator(self, it);
        }
    }
    let mut dv = DispatchVisitor {
        scoping: &scoping,
        uses: Vec::new(),
    };
    dv.visit_program(&program);
    scanner.dispatch_uses = dv.uses;
    struct DispatchConsumerVisitor<'a> {
        scoping: &'a Scoping,
        uses: &'a mut [DispatchUse],
    }
    impl<'a> Visit<'a> for DispatchConsumerVisitor<'a> {
        fn visit_function(&mut self, it: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
            for use_ in self
                .uses
                .iter_mut()
                .filter(|use_| use_.span.start >= it.span.start && use_.span.end <= it.span.end)
            {
                use_.proven_consumer = prove_dispatch_consumer(
                    it,
                    use_.span,
                    use_.table,
                    use_.bytecode,
                    use_.record,
                    &use_.state_field,
                    self.scoping,
                );
            }
            walk::walk_function(self, it, flags);
        }
    }
    let mut consumer_visitor = DispatchConsumerVisitor {
        scoping: &scoping,
        uses: &mut scanner.dispatch_uses,
    };
    consumer_visitor.visit_program(&program);
    for use_ in &scanner.dispatch_uses {
        table_symbols.insert(use_.table);
        bytecode_symbols.insert(use_.bytecode);
        let binding = scanner.binding(use_.table);
        let value = binding.and_then(|b| b.value.clone());
        if let Some(value) = value {
            let mut md = BTreeMap::new();
            let exact_shape = is_dispatch_literal(&value);
            let mutated = scanner.assignments.contains(&use_.table)
                || scanner.property_mutations.contains(&use_.table);
            let proven = use_.proven_consumer && exact_shape && !mutated;
            md.insert(
                "semantics".into(),
                if proven {
                    "state -> bytecode byte -> [next_state, selector]"
                } else {
                    "unknown-literal-candidate"
                }
                .into(),
            );
            md.insert("state_component_index".into(), "0".into());
            md.insert("selector_component_index".into(), "1".into());
            md.insert("handler_mapping".into(), "unknown-unlinked-selector".into());
            md.insert("runtime_mutability".into(), "unknown".into());
            push(artifact(
                ArtifactKind::DispatchTable,
                if proven {
                    FactStatus::Extracted
                } else {
                    FactStatus::Unknown
                },
                binding.map(|b| b.name.clone()),
                binding.map_or(use_.span, |b| b.span),
                md,
                None,
                Some(value),
                vec![evidence(
                    "nested-dispatch-read",
                    if proven {
                        "bounded table shape plus same-state assignment and selector return are proven; selector is not yet linked to a JavaScript handler"
                    } else {
                        "nested computed read and literal table are retained, but pair semantics are not certified without exact consumer dataflow"
                    },
                    Some(use_.span),
                )],
            ));
        } else {
            push(artifact(
                ArtifactKind::DispatchTable,
                FactStatus::Unknown,
                binding.map(|b| b.name.clone()),
                binding.map_or(use_.span, |b| b.span),
                BTreeMap::new(),
                None,
                None,
                vec![evidence(
                    "nested-dispatch-read",
                    "dispatch read was found but its table is not a bounded static literal",
                    Some(use_.span),
                )],
            ));
        }
    }
    for symbol in bytecode_symbols {
        if let Some(binding) = scanner.binding(symbol) {
            let decoded = scanner.decoded_bindings.get(&symbol).and_then(|span| {
                scanner
                    .calls
                    .iter()
                    .find(|call| call.span == *span)
                    .map(|call| {
                        (
                            call.span,
                            call.decoded.clone(),
                            call.has_destination_argument,
                        )
                    })
            });
            match decoded {
                Some((span, Ok(bytes), destination_backed)) => {
                    let mut md = BTreeMap::new();
                    md.insert(
                        "consumer".into(),
                        "nested dispatch byte read with post-increment".into(),
                    );
                    if destination_backed {
                        md.insert(
                            "destination_buffer_result".into(),
                            "full returned view not modeled; decoder prefix retained separately"
                                .into(),
                        );
                    }
                    md.insert(
                        "literal_mutability".into(),
                        if scanner.assignments.contains(&symbol) {
                            "assigned-after-initialization"
                        } else {
                            "unknown"
                        }
                        .into(),
                    );
                    let mutated = scanner.assignments.contains(&symbol)
                        || scanner.property_mutations.contains(&symbol);
                    push(artifact(
                        ArtifactKind::JsBytecodeImage,
                        if mutated || destination_backed {
                            FactStatus::Unknown
                        } else {
                            FactStatus::Extracted
                        },
                        Some(binding.name.clone()),
                        span,
                        md,
                        (!destination_backed).then_some(bytes.clone()),
                        None,
                        vec![evidence(
                            "dispatch-byte-read",
                            if destination_backed {
                                "decoder returned a destination-backed view; full bytecode image is not modeled and the known prefix is retained as a separate sink artifact"
                            } else if mutated {
                                "initial bytecode snapshot is retained, but its binding was mutated"
                            } else {
                                "bytecode is consumed by r[this.P++]-shaped nested dispatch read"
                            },
                            Some(span),
                        )],
                    ));
                    let mut operand = BTreeMap::new();
                    operand.insert("width_bytes".into(), "1".into());
                    operand.insert("pc_advance".into(), "1".into());
                    operand.insert("signedness".into(), "unknown".into());
                    operand.insert("endianness".into(), "not-applicable".into());
                    operand.insert("completeness".into(), "byte-fetch-observation-only".into());
                    push(artifact(
                        ArtifactKind::OperandFormat,
                        if mutated || destination_backed {
                            FactStatus::Unknown
                        } else {
                            FactStatus::Extracted
                        },
                        Some(binding.name.clone()),
                        span,
                        operand,
                        None,
                        None,
                        vec![evidence(
                            "simple-byte-read",
                            "opcode byte fetch width 1 and PC advance 1 are proven; this is not a complete or branch-specific operand schema",
                            Some(span),
                        )],
                    ));
                }
                Some((span, Err(e), _)) => {
                    let mut metadata = BTreeMap::new();
                    if let Some(encoded) = scanner
                        .calls
                        .iter()
                        .find(|call| call.span == span)
                        .and_then(|call| call.encoded.clone())
                    {
                        metadata.insert("encoded_literal".into(), encoded);
                    }
                    push(artifact(
                        ArtifactKind::JsBytecodeImage,
                        FactStatus::Unknown,
                        Some(binding.name.clone()),
                        span,
                        metadata,
                        None,
                        None,
                        vec![evidence("decoder-rejection", e.clone(), Some(span))],
                    ));
                }
                None => push(artifact(
                    ArtifactKind::JsBytecodeImage,
                    FactStatus::Unknown,
                    Some(binding.name.clone()),
                    binding.span,
                    BTreeMap::new(),
                    None,
                    None,
                    vec![evidence(
                        "dispatch-byte-read",
                        "dispatch consumes a binding without a proven decoder literal",
                        Some(binding.span),
                    )],
                )),
            }
        }
    }
    if !table_symbols.is_empty() && report.is_some() { /* report identity is linked below; no name-based role inference */
    }
    for binding in &scanner.bindings {
        if let Some(value) = binding.value.clone() {
            if let RuntimeValue::Array(v) = &value {
                if v.len() >= 4 && v.iter().all(|x| matches!(x, RuntimeValue::Number(_))) {
                    let binding_symbol = scanner.symbols.iter().find_map(|(symbol, index)| {
                        scanner
                            .bindings
                            .get(*index)
                            .is_some_and(|candidate| candidate.name == binding.name)
                            .then_some(*symbol)
                    });
                    let mutated = binding_symbol.is_some_and(|symbol| {
                        scanner.assignments.contains(&symbol)
                            || scanner.property_mutations.contains(&symbol)
                    });
                    push(artifact(
                        ArtifactKind::NumberTable,
                        if mutated {
                            FactStatus::Unknown
                        } else {
                            FactStatus::Extracted
                        },
                        Some(binding.name.clone()),
                        binding.span,
                        BTreeMap::new(),
                        None,
                        Some(value.clone()),
                        vec![evidence(
                            "literal-number-table",
                            "bounded array consists only of number literals",
                            Some(binding.span),
                        )],
                    ));
                }
                if v.len() > 32 && v.iter().all(|x| matches!(x, RuntimeValue::String(_))) {
                    let binding_symbol = scanner.symbols.iter().find_map(|(symbol, index)| {
                        scanner
                            .bindings
                            .get(*index)
                            .is_some_and(|candidate| candidate.name == binding.name)
                            .then_some(*symbol)
                    });
                    let mutated = binding_symbol.is_some_and(|symbol| {
                        scanner.assignments.contains(&symbol)
                            || scanner.property_mutations.contains(&symbol)
                    });
                    push(artifact(
                        ArtifactKind::StringTable,
                        if mutated {
                            FactStatus::Unknown
                        } else {
                            FactStatus::Extracted
                        },
                        Some(binding.name.clone()),
                        binding.span,
                        BTreeMap::from([(
                            "runtime_relationship".into(),
                            if mutated {
                                "initial-literal-snapshot; binding or property mutation observed"
                            } else {
                                "initial-literal-snapshot; runtime immutability unknown"
                            }
                            .into(),
                        )]),
                        None,
                        Some(value),
                        vec![evidence(
                            "literal-string-table",
                            "bounded array consists only of string literals",
                            Some(binding.span),
                        )],
                    ));
                }
            }
        }
    }
    if let Some(call) = pipeline::entry_call(&program) {
        if let Some(argument) = call.arguments.get(3).and_then(Argument::as_expression) {
            let value = static_value(argument, &options.limits).ok();
            push(artifact(
                ArtifactKind::InitialMemorySlots,
                FactStatus::Unknown,
                None,
                argument.span(),
                BTreeMap::from([(
                    "meaning".into(),
                    "candidate fourth entry literal; initial memory-slot role not proven and not operand stack".into(),
                )]),
                None,
                value,
                vec![evidence(
                    "entry-argument",
                    "fourth argument of the proven entry call is retained as an exact candidate; position alone does not prove its runtime role",
                    Some(argument.span()),
                )],
            ));
        }
    }
    add_report_facts(&mut push, report, &scanner, source);
    let has_wasm_image = scanner
        .calls
        .iter()
        .any(|call| call.destination.is_some() && call.decoded.is_ok());
    if has_wasm_image {
        let mut md = BTreeMap::new();
        md.insert("geometry".into(), "unknown".into());
        md.insert("handler_mapping".into(), "unproven".into());
        push(artifact(ArtifactKind::DispatchTable, FactStatus::Unknown, None, SourceSpan { start: 0, end: 0 }, md, None, None, vec![evidence("wasm-geometry-boundary", "WASM dispatch geometry and branch-to-handler mapping require operator/dataflow proof; no image-tail or constant heuristic is used", None)]));
        let mut md = BTreeMap::new();
        md.insert("schemas".into(), "unknown-per-branch".into());
        push(artifact(ArtifactKind::OperandFormat, FactStatus::Unknown, None, SourceSpan { start: 0, end: 0 }, md, None, None, vec![evidence("wasm-operand-boundary", "WASM operand widths and signedness are not certified by this configuration extractor", None)]));
    }
    if let Some(report) = report {
        if let Ok(catalogue) = catalogue::catalogue_from_report(report) {
            let mut md = BTreeMap::new();
            md.insert(
                "handler_count".into(),
                catalogue.table.element_count.to_string(),
            );
            md.insert("identity".into(), catalogue.table.canonical_name);
            let table_span =
                catalogue
                    .handlers
                    .first()
                    .map_or(SourceSpan { start: 0, end: 0 }, |handler| SourceSpan {
                        start: handler.source_span.start,
                        end: handler.source_span.end,
                    });
            push(artifact(
                ArtifactKind::OperationTable,
                FactStatus::Extracted,
                Some(catalogue.table.original_name),
                table_span,
                md,
                None,
                None,
                vec![evidence(
                    "proven-report-operation-table",
                    "operation table identity comes from the standardized pipeline report",
                    None,
                )],
            ));
            for h in catalogue.handlers {
                let mut md = BTreeMap::new();
                md.insert("handler_index".into(), h.handler_index.to_string());
                md.insert("recipe_identity".into(), h.structural_fingerprint);
                md.insert(
                    "deob_source_span".into(),
                    format!("{}..{}", h.source_span.start, h.source_span.end),
                );
                push(artifact(ArtifactKind::OperationRecipeReference, FactStatus::Extracted, None, SourceSpan { start: h.source_span.start, end: h.source_span.end }, md, None, None, vec![evidence("operation-catalogue", "handler recipe is referenced by catalogue identity in standardized artifact coordinates", None)]));
            }
        }
    }
    for kind in [
        ArtifactKind::WasmModule,
        ArtifactKind::WasmMemoryImage,
        ArtifactKind::JsBytecodeImage,
        ArtifactKind::DispatchTable,
        ArtifactKind::OperandFormat,
        ArtifactKind::StringTable,
        ArtifactKind::DecoderParameters,
        ArtifactKind::ThreadDescriptor,
        ArtifactKind::InitialMemorySlots,
        ArtifactKind::OperationTable,
        ArtifactKind::OperationRecipeReference,
    ] {
        if !present_kinds.borrow().contains(&kind) {
            push(artifact(
                kind,
                FactStatus::Unknown,
                None,
                SourceSpan { start: 0, end: 0 },
                BTreeMap::from([(
                    String::from("completeness"),
                    String::from("no bounded proof found"),
                )]),
                None,
                None,
                vec![evidence(
                    "category-not-proven",
                    format!(
                        "no bounded {} fact was proven; this is not an empty success",
                        kind.as_str()
                    ),
                    None,
                )],
            ));
        }
    }
    if artifacts.is_empty() {
        diagnostics.push("no bounded runtime configuration facts were proven".into());
    }
    let span_source = if report.is_some() {
        "artifact"
    } else {
        "input"
    };
    for item in &mut artifacts {
        item.span_source = span_source.into();
        for item_evidence in &mut item.evidence {
            item_evidence.span_source = span_source.into();
        }
    }
    Ok(RuntimeConfig {
        schema_version: SCHEMA_VERSION.into(),
        analysis_version: ANALYSIS_VERSION.into(),
        input_mode: options.input_mode,
        coordinate_system: if matches!(options.input_mode, InputMode::SavedReport) {
            "UTF-8 byte offsets into standardized report.output artifact source; original capture identity is declared by report.input_hash".into()
        } else if report.is_some() {
            "UTF-8 byte offsets into standardized report.output artifact source".into()
        } else {
            "UTF-8 byte offsets into supplied deobfuscated source".into()
        },
        source: source_identity,
        artifact_source: SourceIdentity {
            algorithm: "fnv1a64".into(),
            digest: fnv1a64_hex(source.as_bytes()),
        },
        artifacts,
        diagnostics,
    })
}

/// Reject pathological syntax nesting before handing source to the recursive
/// parser or literal walker.  This is deliberately lexical and conservative:
/// it does not certify balanced JavaScript, it only prevents an attacker from
/// using tens of thousands of delimiters to exhaust the parser stack.
fn preparse_nesting_guard(source: &str, literal_depth: usize) -> Result<(), RuntimeConfigError> {
    let limit = literal_depth.saturating_mul(16).max(256);
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut index = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let mut line_comment = false;
    let mut block_comment = false;

    while index < bytes.len() {
        let byte = bytes[index];
        if line_comment {
            line_comment = byte != b'\n' && byte != b'\r';
            index += 1;
            continue;
        }
        if block_comment {
            if byte == b'*' && bytes.get(index + 1) == Some(&b'/') {
                block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if let Some(delimiter) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == delimiter {
                quote = None;
            }
            index += 1;
            continue;
        }
        if byte == b'/' && matches!(bytes.get(index + 1), Some(b'/') | Some(b'*')) {
            if bytes[index + 1] == b'/' {
                line_comment = true;
            } else {
                block_comment = true;
            }
            index += 2;
            continue;
        }
        if matches!(byte, b'\'' | b'"' | b'`') {
            quote = Some(byte);
            index += 1;
            continue;
        }
        if matches!(byte, b'(' | b'[' | b'{') {
            depth += 1;
            if depth > limit {
                return Err(RuntimeConfigError::Limit {
                    what: "syntactic nesting depth".into(),
                    limit,
                });
            }
        } else if matches!(byte, b')' | b']' | b'}') {
            depth = depth.saturating_sub(1);
        }
        index += 1;
    }
    Ok(())
}

fn add_report_facts(
    push: &mut impl FnMut(RuntimeArtifact),
    report: Option<&PipelineReport>,
    scanner: &Scanner<'_>,
    _source: &str,
) {
    let Some(report) = report else { return };
    for fact in &report.extraction {
        let kind = match fact.kind {
            pipeline::ExtractionKind::ThreadDescriptors => ArtifactKind::ThreadDescriptor,
            _ => continue,
        };
        let binding = fact.binding.as_deref().and_then(|name| {
            scanner.bindings.iter().find(|binding| {
                binding.name == name
                    && binding.span.start == fact.span.start
                    && binding.span.end == fact.span.end
            })
        });
        let mut md = BTreeMap::new();
        md.insert("pipeline_kind".into(), fact.kind.to_string());
        md.insert(
            "binding_match".into(),
            if binding.is_some() {
                "exact-span-and-name"
            } else {
                "none"
            }
            .into(),
        );
        if let Some(length) = fact.length {
            md.insert("record_count".into(), length.to_string());
        }
        md.insert(
            "literal_mutability".into(),
            "unknown; exact initial snapshot only".into(),
        );
        let mut status = match fact.status {
            pipeline::ExtractionStatus::Extracted => FactStatus::Extracted,
            pipeline::ExtractionStatus::Unknown => FactStatus::Unknown,
            pipeline::ExtractionStatus::Unsupported => FactStatus::Unsupported,
        };
        if binding.is_none()
            || (status == FactStatus::Extracted
                && binding.and_then(|item| item.value.as_ref()).is_none())
        {
            status = FactStatus::Unknown;
        }
        push(artifact(
            kind,
            status,
            binding.map(|item| item.name.clone()),
            SourceSpan::from(fact.span),
            md,
            None,
            binding.and_then(|binding| binding.value.clone()),
            vec![evidence("pipeline-report", &fact.evidence, Some(fact.span))],
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn artifact(
    kind: ArtifactKind,
    status: FactStatus,
    binding: Option<String>,
    span: impl Into<SourceSpan>,
    metadata: BTreeMap<String, String>,
    bytes: Option<Vec<u8>>,
    value: Option<RuntimeValue>,
    evidence: Vec<Evidence>,
) -> RuntimeArtifact {
    RuntimeArtifact {
        id: String::new(),
        kind,
        status,
        binding,
        consumer: None,
        span: span.into(),
        span_source: "input".into(),
        byte_length: bytes.as_ref().map(Vec::len),
        bytes,
        value,
        metadata,
        evidence,
    }
}
fn evidence(rule: &str, message: impl Into<String>, span: Option<Span>) -> Evidence {
    Evidence {
        rule: rule.into(),
        message: message.into(),
        span: span.map(Into::into),
        span_source: "input".into(),
    }
}
fn value_json(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::String(v) => json_string(v),
        RuntimeValue::Number(v) if v.is_finite() && *v == 0.0 && v.is_sign_negative() => {
            "-0".into()
        }
        RuntimeValue::Number(v) if v.is_finite() => v.to_string(),
        RuntimeValue::Number(_) => "\"unsupported-nonfinite-number\"".into(),
        RuntimeValue::Boolean(v) => v.to_string(),
        RuntimeValue::Null => "null".into(),
        RuntimeValue::Array(v) => format!(
            "[{}]",
            v.iter().map(value_json).collect::<Vec<_>>().join(",")
        ),
        RuntimeValue::Object(v) => format!(
            "{{{}}}",
            v.iter()
                .map(|(k, v)| format!("{}:{}", json_string(k), value_json(v)))
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}
fn bytes_json(bytes: &[u8]) -> String {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let a = bytes[i];
        let b = bytes.get(i + 1).copied();
        let c = bytes.get(i + 2).copied();
        out.push(alphabet[(a >> 2) as usize] as char);
        out.push(alphabet[((a & 3) << 4 | b.map_or(0, |v| v >> 4)) as usize] as char);
        out.push(b.map_or('=', |v| {
            alphabet[((v & 15) << 2 | c.map_or(0, |x| x >> 6)) as usize] as char
        }));
        out.push(c.map_or('=', |v| alphabet[(v & 63) as usize] as char));
        i += 3;
    }
    json_string(&out)
}
fn to_json(config: &RuntimeConfig) -> String {
    let mut out = String::from("{\"schema_version\":");
    out.push_str(&json_string(&config.schema_version));
    out.push_str(",\"analysis_version\":");
    out.push_str(&json_string(&config.analysis_version));
    out.push_str(",\"input_mode\":");
    out.push_str(&json_string(config.input_mode.as_str()));
    out.push_str(",\"coordinate_system\":");
    out.push_str(&json_string(&config.coordinate_system));
    out.push_str(",\"source\":{\"algorithm\":");
    out.push_str(&json_string(&config.source.algorithm));
    out.push_str(",\"digest\":");
    out.push_str(&json_string(&config.source.digest));
    out.push_str("},\"artifact_source\":{\"algorithm\":");
    out.push_str(&json_string(&config.artifact_source.algorithm));
    out.push_str(",\"digest\":");
    out.push_str(&json_string(&config.artifact_source.digest));
    out.push_str("},\"artifacts\":[");
    for (i, a) in config.artifacts.iter().enumerate() {
        if i > 0 {
            out.push(',')
        }
        out.push_str("{\"id\":");
        out.push_str(&json_string(&a.id));
        out.push_str(",\"kind\":");
        out.push_str(&json_string(a.kind.as_str()));
        out.push_str(",\"status\":");
        out.push_str(&json_string(a.status.as_str()));
        out.push_str(",\"binding\":");
        out.push_str(
            a.binding
                .as_deref()
                .map_or("null".into(), json_string)
                .as_str(),
        );
        out.push_str(",\"consumer\":");
        out.push_str(
            a.consumer
                .as_deref()
                .map_or("null".into(), json_string)
                .as_str(),
        );
        out.push_str(",\"span\":{");
        out.push_str(&format!(
            "\"start\":{},\"end\":{}",
            a.span.start, a.span.end
        ));
        out.push_str("},\"span_source\":");
        out.push_str(&json_string(&a.span_source));
        out.push_str(",\"byte_length\":");
        out.push_str(&a.byte_length.map_or("null".into(), |v| v.to_string()));
        out.push_str(",\"bytes_base64\":");
        out.push_str(
            a.bytes
                .as_deref()
                .map_or("null".into(), bytes_json)
                .as_str(),
        );
        out.push_str(",\"value\":");
        out.push_str(&a.value.as_ref().map_or("null".into(), value_json));
        out.push_str(",\"metadata\":{");
        out.push_str(
            &a.metadata
                .iter()
                .map(|(k, v)| format!("{}:{}", json_string(k), json_string(v)))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push_str("},\"evidence\":[");
        out.push_str(
            &a.evidence
                .iter()
                .map(|e| {
                    let span = e.span.as_ref().map_or("null".into(), |s| {
                        format!("{{\"start\":{},\"end\":{}}}", s.start, s.end)
                    });
                    format!(
                        "{{\"rule\":{},\"message\":{},\"span\":{},\"span_source\":{}}}",
                        json_string(&e.rule),
                        json_string(&e.message),
                        span,
                        json_string(&e.span_source)
                    )
                })
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push_str("]}");
    }
    out.push_str("],\"diagnostics\":[");
    out.push_str(
        &config
            .diagnostics
            .iter()
            .map(|v| json_string(v))
            .collect::<Vec<_>>()
            .join(","),
    );
    out.push_str("]}");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::wasm_module::test_support::*;

    const URL_SAFE_DECODER: &str = r#"function decode(input, destination) {
  var alpha = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
  var length = input.length;
  destination = destination || new (typeof Uint8Array === "function" ? Uint8Array : Array)(Math.floor(length * 3 / 4));
  var a, b, c, d, e, f, g;
  for (var inputCursor = 0, outputCursor = 0; inputCursor < length; inputCursor += 4, outputCursor += 3) {
    a = alpha.indexOf(input.charAt(inputCursor));
    b = alpha.indexOf(input.charAt(inputCursor + 1));
    c = alpha.indexOf(input.charAt(inputCursor + 2));
    d = alpha.indexOf(input.charAt(inputCursor + 3));
    e = a << 2 | b >> 4;
    f = (b & 15) << 4 | c >> 2;
    g = (c & 3) << 6 | d;
    destination[outputCursor] = e;
    if (inputCursor + 2 < length) { destination[outputCursor + 1] = f; }
    if (inputCursor + 3 < length) { destination[outputCursor + 2] = g; }
  }
  return destination;
}"#;

    fn finalized_calls(source: &str) -> Vec<DecoderCall> {
        let allocator = Allocator::default();
        let program = analysis::parse_program(&allocator, source).expect("parse");
        let (scoping, _) = analysis::analyze(&program);
        let mut scanner = Scanner::new(&scoping, ExtractionLimits::default());
        scanner.visit_program(&program);
        finalize_decoder_proofs(source, &mut scanner);
        scanner.calls
    }

    #[test]
    fn strict_consumer_rejects_url_safe_alphabet() {
        assert!(decode_standard_base64("-_", 16).is_err());
    }
    #[test]
    fn strict_consumer_decodes_standard() {
        assert_eq!(
            decode_standard_base64("AGFzbQ==", 16).expect("valid"),
            vec![0, 97, 115, 109]
        );
    }
    #[test]
    fn url_safe_decoder_reports_proven_alphabet() {
        let source = format!("{URL_SAFE_DECODER}\nvar image = decode(\"-_8\");");
        let config = scan(
            &source,
            None,
            SourceIdentity {
                algorithm: "fnv1a64".into(),
                digest: fnv1a64_hex(source.as_bytes()),
            },
            RuntimeConfigOptions::default(),
        )
        .expect("scan");
        let artifact = config
            .artifacts
            .iter()
            .find(|artifact| artifact.kind == ArtifactKind::DecoderParameters)
            .expect("decoder artifact");
        assert_eq!(artifact.metadata["alphabet"], URL_SAFE_ALPHABET);
        assert_eq!(artifact.metadata["proof_alphabet"], URL_SAFE_ALPHABET);
        assert_eq!(
            artifact.metadata["proof_rule"],
            "url-safe-base64-decoder-v1"
        );
        assert_eq!(
            artifact.metadata["consumer_validation"],
            "strict URL-safe alphabet"
        );
    }
    #[test]
    fn url_safe_decoder_decodes_with_url_safe_policy() {
        let source = format!("{URL_SAFE_DECODER}\nvar image = decode(\"-_8\");");
        let calls = finalized_calls(&source);
        let call = calls
            .iter()
            .find(|call| call.encoded.as_deref() == Some("-_8"))
            .expect("decoder call");
        assert_eq!(call.decoded, Ok(vec![251u8, 255]));
    }
    #[test]
    fn url_safe_decoder_rejects_standard_and_invalid_characters() {
        for encoded in ["+/8", "YQ==", "-_8!"] {
            let source = format!("{URL_SAFE_DECODER}\nvar image = decode(\"{encoded}\");");
            let calls = finalized_calls(&source);
            let call = calls
                .iter()
                .find(|call| call.encoded.as_deref() == Some(encoded))
                .expect("decoder call");
            assert!(call.decoded.is_err(), "{encoded} was decoded");
        }
    }
    #[test]
    fn deterministic_json() {
        let a = extract("var x = [1, 2];").expect("scan");
        let b = extract("var x = [1, 2];").expect("scan");
        assert_eq!(a.json(), b.json());
    }
    #[test]
    fn wasm_info_preserves_abi_for_valid_gc_types() {
        // Old config only walked import/memory/export sections after
        // validation.  The detailed parser's GC rejection must not erase the
        // memory export or the validation success.
        assert_eq!(
            wasm_info(&gc_memory_export_module()),
            (true, vec![], vec!["m".into()], true)
        );
        assert_eq!(
            wasm_info(&gc_import_export_module()),
            (true, vec!["env::f".into()], vec!["run".into()], false)
        );
    }
    #[test]
    fn wasm_info_keeps_compact_import_names_but_not_memory_flag() {
        // Compact-2 imports expand to names; config's historical memory flag
        // was set only by a single memory import or a defined memory section.
        assert_eq!(
            wasm_info(&compact2_memory_import_module()),
            (true, vec!["env::m".into()], vec!["m".into()], false)
        );
        assert_eq!(
            wasm_info(&compact1_imports_module()),
            (true, vec!["env::a".into(), "env::b".into()], vec![], false)
        );
    }
    #[test]
    fn wasm_info_still_rejects_validator_invalid_modules() {
        assert_eq!(
            wasm_info(&invalid_export_module()),
            (false, vec![], vec![], false)
        );
        assert_eq!(
            wasm_info(&malformed_type_memory_export_module()),
            (false, vec![], vec![], false)
        );
    }
    #[test]
    fn wasm_info_accepts_large_module_without_new_default_cap() {
        let bytes = large_custom_section_module();
        assert!(bytes.len() > 8 * 1024 * 1024);
        assert_eq!(wasm_info(&bytes), (true, vec![], vec!["m".into()], true));
    }
}
