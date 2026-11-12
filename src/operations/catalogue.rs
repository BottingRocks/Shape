//! Opt-in, static operation-handler catalogue.
//!
//! This module is intentionally a catalogue, not a VM, tracer, decoder, or
//! renamer. It consumes the exact operation-table identity proven by the
//! deobfuscation report and applies bounded AST recognizers to each handler.
//! Unknown behavior remains visible with its source span and body.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    io::{self, Write},
};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::Span;
use serde::ser::Serializer;
use serde::Serialize;
use serde_json::ser::{CharEscape, Formatter};

use crate::analysis;
use crate::transforms::deob::pipeline::{PipelineError, PipelineReport, ProvenOperationTable};
use crate::util::fnv1a64_hex;

pub use super::table_scan::SpanRecord;
use super::table_scan::{scan_target_table, Handler, TableLiteral, TableScanMode, TableSink};

pub const SCHEMA_VERSION: &str = "shape-kernel-operation-catalogue/v1";
const CATALOGUE_VERSION: &str = "shape-oxc-deob/operation-catalogue-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OperationCatalogue {
    pub schema_version: String,
    pub catalogue_version: String,
    pub source_hash: String,
    pub table: OperationTable,
    pub inventory: InventoryCounts,
    pub handlers: Vec<OperationRecord>,
    pub fingerprint_groups: Vec<FingerprintGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OperationTable {
    pub declaration_ordinal: u32,
    pub original_name: String,
    pub canonical_name: String,
    pub shape: String,
    pub element_count: u32,
    pub function_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct InventoryCounts {
    pub exact: u32,
    pub partial: u32,
    pub unknown: u32,
    pub missing_sparse_entries: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FingerprintGroup {
    pub fingerprint: String,
    pub handler_indices: Vec<u32>,
    pub frequency_across_seeds: u32,
    pub canonical_normalized_body: String,
}

/// The closed classification assigned by the bounded recognizer.
///
/// Confidence is derived from this value rather than stored beside it, so a
/// record cannot claim an atomic classification with an unknown proof rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Classification {
    Unknown,
    Atomic,
    Composite,
}

impl Classification {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Atomic => "atomic",
            Self::Composite => "composite",
        }
    }

    pub const fn confidence_level(self) -> ConfidenceLevel {
        match self {
            Self::Unknown => ConfidenceLevel::Unknown,
            Self::Atomic => ConfidenceLevel::High,
            Self::Composite => ConfidenceLevel::Partial,
        }
    }

    pub const fn proof_rule_id(self) -> ProofRuleId {
        match self {
            Self::Unknown => ProofRuleId::OpUnknown001,
            Self::Atomic => ProofRuleId::OpAstAtomic001,
            Self::Composite => ProofRuleId::OpAstComposite001,
        }
    }

    pub const fn evidence(self) -> &'static [&'static str] {
        match self {
            Self::Unknown => &["No bounded recognizer matched; body retained verbatim"],
            Self::Atomic => {
                &["One exact AST recognizer matched and no unknown fragments were observed"]
            }
            Self::Composite => {
                &["Ordered exact recognizers retained; unsupported fragments remain explicit"]
            }
        }
    }

    pub fn confidence(self) -> Confidence {
        Confidence {
            level: self.confidence_level(),
            proof_rule_id: self.proof_rule_id(),
            evidence: self.evidence().iter().map(|item| (*item).into()).collect(),
        }
    }
}

impl fmt::Display for Classification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for Classification {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfidenceLevel {
    Unknown,
    High,
    Partial,
}

impl ConfidenceLevel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::High => "high",
            Self::Partial => "partial",
        }
    }
}

impl fmt::Display for ConfidenceLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for ConfidenceLevel {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProofRuleId {
    OpUnknown001,
    OpAstAtomic001,
    OpAstComposite001,
}

impl ProofRuleId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpUnknown001 => "OP-UNKNOWN-001",
            Self::OpAstAtomic001 => "OP-AST-ATOMIC-001",
            Self::OpAstComposite001 => "OP-AST-COMPOSITE-001",
        }
    }
}

impl fmt::Display for ProofRuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for ProofRuleId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// The closed set of action kinds emitted by the bounded recognizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MicroOperationKind {
    JsAdd,
    JsSubtract,
    JsXor,
    JsBinary,
    PropertyWrite,
    PropertyRead,
    StackRead,
    StackWrite,
    MemoryReadSlot,
    MemoryWriteSlot,
    MemoryInitializeSlot,
    Call,
    ThreadCapture,
    SharedStringDecode,
    ConstructorCall,
    Return,
    Throw,
    Branch,
    Loop,
}

impl MicroOperationKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::JsAdd => "js_add",
            Self::JsSubtract => "js_subtract",
            Self::JsXor => "js_xor",
            Self::JsBinary => "js_binary",
            Self::PropertyWrite => "property_write",
            Self::PropertyRead => "property_read",
            Self::StackRead => "stack_read",
            Self::StackWrite => "stack_write",
            Self::MemoryReadSlot => "memory_read_slot",
            Self::MemoryWriteSlot => "memory_write_slot",
            Self::MemoryInitializeSlot => "memory_initialize_slot",
            Self::Call => "call",
            Self::ThreadCapture => "thread_capture",
            Self::SharedStringDecode => "shared_string_decode",
            Self::ConstructorCall => "constructor_call",
            Self::Return => "return",
            Self::Throw => "throw",
            Self::Branch => "branch",
            Self::Loop => "loop",
        }
    }
}

impl fmt::Display for MicroOperationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for MicroOperationKind {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// The handler family. It is either a classification sentinel or the single
/// recognizer kind that matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticFamily {
    Unknown,
    Composite,
    Recognizer(MicroOperationKind),
}

impl SemanticFamily {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Composite => "composite",
            Self::Recognizer(kind) => kind.as_str(),
        }
    }
}

impl fmt::Display for SemanticFamily {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for SemanticFamily {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationRecord {
    /// Complete versioned catalogue identity: `CATALOGUE_VERSION`, a colon,
    /// and the resolved source hash (FNV by default, or supplied by the caller).
    pub kernel_identity: String,
    /// Handler's numeric index retained for the versioned record and dispatch
    /// metadata; it is separate from `table_key` because sparse table keys and
    /// handler indices are distinct concepts even when the current extractor
    /// assigns them the same value.
    pub handler_index: u32,
    /// Numeric key used to select the handler in this capture's operation
    /// table; it is not a semantic operation name and is kept separately from
    /// `handler_index` for the versioned key/index contract.
    pub table_key: u32,
    pub source_span: SpanRecord,
    pub source_snippet: String,
    pub structural_fingerprint: String,
    pub canonical_normalized_body: String,
    pub semantic_family: SemanticFamily,
    pub classification: Classification,
    pub ordered_micro_operations: Vec<MicroOperation>,
    pub effects: Effects,
    pub unknown_fragments: Vec<UnknownFragment>,
}

impl OperationRecord {
    /// Confidence is always derived from the record's classification.
    pub fn confidence(&self) -> Confidence {
        self.classification.confidence()
    }
}

/// The record's JSON object is hand-ordered: it inserts the derived
/// `confidence` object between `effects` and `unknown_fragments`, which is not
/// the struct's field order, so the versioned key order is preserved explicitly.
impl Serialize for OperationRecord {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        let mut record = serializer.serialize_struct("OperationRecord", 13)?;
        record.serialize_field("kernel_identity", &self.kernel_identity)?;
        record.serialize_field("handler_index", &self.handler_index)?;
        record.serialize_field("table_key", &self.table_key)?;
        record.serialize_field("source_span", &self.source_span)?;
        record.serialize_field("source_snippet", &self.source_snippet)?;
        record.serialize_field("structural_fingerprint", &self.structural_fingerprint)?;
        record.serialize_field("canonical_normalized_body", &self.canonical_normalized_body)?;
        record.serialize_field("semantic_family", &self.semantic_family)?;
        record.serialize_field("classification", &self.classification)?;
        record.serialize_field("ordered_micro_operations", &self.ordered_micro_operations)?;
        record.serialize_field("effects", &self.effects)?;
        record.serialize_field("confidence", &self.confidence())?;
        record.serialize_field("unknown_fragments", &self.unknown_fragments)?;
        record.end()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MicroOperation {
    pub kind: MicroOperationKind,
    pub operator: Option<String>,
    pub operand_roles: Vec<String>,
    pub span: SpanRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct Effects {
    pub stack_reads: u32,
    pub stack_writes: u32,
    pub stack_delta: i32,
    pub memory_reads: u32,
    pub memory_writes: u32,
    pub memory_slots: Vec<String>,
    pub control_flow: Vec<String>,
    pub exception_effects: Vec<String>,
    pub calls_unknown_possible_throw: bool,
    pub constructor_calls: u32,
    pub thread_captures: u32,
}

/// Confidence derived from a [`Classification`]. Fields are private so callers
/// cannot pair a level or rule with an unrelated classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Confidence {
    level: ConfidenceLevel,
    proof_rule_id: ProofRuleId,
    evidence: Vec<String>,
}

impl Confidence {
    pub const fn level(&self) -> ConfidenceLevel {
        self.level
    }

    pub const fn proof_rule_id(&self) -> ProofRuleId {
        self.proof_rule_id
    }

    pub fn evidence(&self) -> &[String] {
        &self.evidence
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnknownFragment {
    pub span: SpanRecord,
    pub reason: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CatalogueOptions {
    /// Names are accepted only as an explicit hand-off from a proven report.
    /// `catalogue_from_report` populates these from binding mappings.
    pub proven_helper_bindings: BTreeSet<String>,
}

#[derive(Debug)]
pub enum CatalogueError {
    Parse(String),
    NoProvenTable,
    TableIdentityMismatch(String),
}

impl fmt::Display for CatalogueError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "catalogue parse failed: {e}"),
            Self::NoProvenTable => {
                f.write_str("pipeline report contains no proven operation table")
            }
            Self::TableIdentityMismatch(e) => write!(f, "proven operation table mismatch: {e}"),
        }
    }
}
impl std::error::Error for CatalogueError {}

struct CatalogueTableSink<'a> {
    source: &'a str,
    scoping: &'a Scoping,
    options: &'a CatalogueOptions,
    metadata: Option<(&'static str, u32, u32)>,
    handlers: Vec<OperationRecord>,
}

impl TableSink for CatalogueTableSink<'_> {
    fn table(&mut self, table: TableLiteral<'_>) {
        self.metadata = Some((table.shape, table.element_count(), table.function_count));
        for entry in table.entries {
            let Some(handler) = entry.handler else {
                continue;
            };
            self.handlers.push(classify_handler(
                self.source,
                self.scoping,
                entry.key,
                handler,
                self.options,
            ));
        }
    }
}

/// Catalogue the exact proven table in a pipeline report. The report output is
/// used because it contains the same source identity after presentation; no
/// name-only search is performed.
pub fn catalogue_from_report(
    report: &PipelineReport,
) -> Result<OperationCatalogue, CatalogueError> {
    let table = report
        .proven_operation_tables
        .first()
        .ok_or(CatalogueError::NoProvenTable)?;
    let mut options = CatalogueOptions::default();
    for mapping in &report.binding_mappings {
        if mapping.canonical_name.starts_with("vm") {
            options
                .proven_helper_bindings
                .insert(mapping.canonical_name.clone());
        }
    }
    catalogue_source(
        &report.output,
        table,
        &options,
        Some(report.input_hash.clone()),
    )
}

/// Catalogue an explicitly handed-off proven operation table. Callers must
/// provide metadata obtained from a proof/report; this API does not infer a
/// table from `vmOperations`, a spelling, or arbitrary function arrays.
pub fn catalogue_source(
    source: &str,
    identity: &ProvenOperationTable,
    options: &CatalogueOptions,
    source_hash: Option<String>,
) -> Result<OperationCatalogue, CatalogueError> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source).map_err(CatalogueError::Parse)?;
    let (scoping, _) = analysis::analyze(&program);
    let mut sink = CatalogueTableSink {
        source,
        scoping: &scoping,
        options,
        metadata: None,
        handlers: Vec::new(),
    };
    scan_target_table(
        &program,
        identity.declaration_ordinal,
        TableScanMode::Strict,
        &mut sink,
    );
    let (shape, element_count, function_count) = sink.metadata.ok_or_else(|| {
        CatalogueError::TableIdentityMismatch(format!(
            "declaration ordinal {} was not found",
            identity.declaration_ordinal
        ))
    })?;
    if element_count != identity.initializer_elements
        || function_count != identity.initializer_functions
    {
        return Err(CatalogueError::TableIdentityMismatch(format!(
            "expected {} elements/{} functions, found {}/{}",
            identity.initializer_elements,
            identity.initializer_functions,
            element_count,
            function_count
        )));
    }

    let resolved_source_hash = source_hash.unwrap_or_else(|| fnv1a64_hex(source.as_bytes()));
    let kernel_identity = format!("{CATALOGUE_VERSION}:{resolved_source_hash}");
    let mut handlers = sink.handlers;
    for handler in &mut handlers {
        handler.kernel_identity = kernel_identity.clone();
    }
    let mut inventory = InventoryCounts {
        missing_sparse_entries: element_count.saturating_sub(function_count),
        ..InventoryCounts::default()
    };
    for record in &handlers {
        match record.classification {
            Classification::Atomic => inventory.exact += 1,
            Classification::Composite => inventory.partial += 1,
            Classification::Unknown => inventory.unknown += 1,
        }
    }
    let mut groups: BTreeMap<(String, String), Vec<u32>> = BTreeMap::new();
    for record in &handlers {
        groups
            .entry((
                record.structural_fingerprint.clone(),
                record.canonical_normalized_body.clone(),
            ))
            .or_default()
            .push(record.handler_index);
    }
    let fingerprint_groups = groups
        .into_iter()
        .map(|((fingerprint, body), handler_indices)| FingerprintGroup {
            fingerprint,
            frequency_across_seeds: handler_indices.len() as u32,
            handler_indices,
            canonical_normalized_body: body,
        })
        .collect();
    Ok(OperationCatalogue {
        schema_version: SCHEMA_VERSION.into(),
        catalogue_version: CATALOGUE_VERSION.into(),
        source_hash: resolved_source_hash,
        table: OperationTable {
            declaration_ordinal: identity.declaration_ordinal,
            original_name: identity.original_name.clone(),
            canonical_name: identity.canonical_name.clone(),
            shape: shape.into(),
            element_count,
            function_count,
        },
        inventory,
        handlers,
        fingerprint_groups,
    })
}

pub fn catalogue_json(catalogue: &OperationCatalogue) -> String {
    to_json(catalogue)
}

pub fn catalogue_markdown(catalogue: &OperationCatalogue) -> String {
    let mut out = format!("# Operation catalogue\n\n- Schema: `{}`\n- Source: `{}`\n- Proven table: `{}` (ordinal {}, {} entries, {} handlers)\n- Inventory: exact {}, partial {}, unknown {}, sparse {}\n\n", catalogue.schema_version, catalogue.source_hash, catalogue.table.canonical_name, catalogue.table.declaration_ordinal, catalogue.table.element_count, catalogue.table.function_count, catalogue.inventory.exact, catalogue.inventory.partial, catalogue.inventory.unknown, catalogue.inventory.missing_sparse_entries);
    out.push_str(
        "| Handler | Family | Class | Fingerprint | Ordered micro-ops |\n|---:|---|---|---|---|\n",
    );
    for h in &catalogue.handlers {
        let ops = h
            .ordered_micro_operations
            .iter()
            .map(|op| op.kind.as_str())
            .collect::<Vec<_>>()
            .join(" → ");
        out.push_str(&format!(
            "| {} | {} | {} | `{}` | {} |\n",
            h.handler_index, h.semantic_family, h.classification, h.structural_fingerprint, ops
        ));
    }
    out
}

fn classify_handler(
    source: &str,
    scoping: &Scoping,
    key: u32,
    handler: Handler<'_>,
    options: &CatalogueOptions,
) -> OperationRecord {
    let span = match handler {
        Handler::Function(f) => f.span,
        Handler::Arrow(f) => f.span,
    };
    let canonical_normalized_body = normalize_handler(source, scoping, &handler);
    let structural_fingerprint = fnv1a64_hex(canonical_normalized_body.as_bytes());
    let mut recognizer = Recognizer {
        source,
        options,
        actions: Vec::new(),
        unknown: Vec::new(),
        effects: Effects::default(),
        in_assignment_target: false,
    };
    match handler {
        Handler::Function(f) => {
            if let Some(body) = &f.body {
                recognizer.visit_function_body(body);
            }
        }
        Handler::Arrow(f) => {
            recognizer.visit_function_body(&f.body);
        }
    }
    let recognized = recognizer.actions.len();
    let classification = if recognized == 0 || (!recognizer.unknown.is_empty() && recognized == 1) {
        Classification::Unknown
    } else if recognizer.unknown.is_empty() && recognized == 1 {
        Classification::Atomic
    } else {
        Classification::Composite
    };
    let semantic_family = if recognized == 0 {
        SemanticFamily::Unknown
    } else if recognized == 1 {
        SemanticFamily::Recognizer(recognizer.actions[0].kind)
    } else {
        SemanticFamily::Composite
    };
    let source_snippet = source.get(span.start as usize..span.end as usize);
    debug_assert!(
        source_snippet.is_some(),
        "parsed handler span must be a valid range in the parsed source"
    );
    OperationRecord {
        kernel_identity: CATALOGUE_VERSION.into(),
        handler_index: key,
        table_key: key,
        source_span: SpanRecord {
            start: span.start,
            end: span.end,
        },
        source_snippet: source_snippet.unwrap_or_default().into(),
        structural_fingerprint,
        canonical_normalized_body,
        semantic_family,
        classification,
        ordered_micro_operations: recognizer.actions,
        effects: recognizer.effects,
        unknown_fragments: recognizer.unknown,
    }
}

struct Recognizer<'a> {
    source: &'a str,
    options: &'a CatalogueOptions,
    actions: Vec<MicroOperation>,
    unknown: Vec<UnknownFragment>,
    effects: Effects,
    in_assignment_target: bool,
}
impl<'a> Recognizer<'a> {
    fn action(
        &mut self,
        kind: MicroOperationKind,
        operator: Option<String>,
        roles: &[&str],
        span: Span,
    ) {
        self.actions.push(MicroOperation {
            kind,
            operator,
            operand_roles: roles.iter().map(|r| (*r).into()).collect(),
            span: SpanRecord {
                start: span.start,
                end: span.end,
            },
        });
    }
    fn unknown(&mut self, span: Span, reason: &str) {
        self.unknown.push(UnknownFragment {
            span: SpanRecord {
                start: span.start,
                end: span.end,
            },
            reason: reason.into(),
            body: self
                .source
                .get(span.start as usize..span.end as usize)
                .unwrap_or_default()
                .into(),
        });
    }
    fn callee_member<'b>(callee: &'b Expression<'b>) -> Option<(&'b str, &'b str)> {
        let Expression::StaticMemberExpression(member) = callee else {
            return None;
        };
        let Expression::Identifier(object) = &member.object else {
            return None;
        };
        Some((object.name.as_str(), member.property.name.as_str()))
    }
    fn direct_name<'b>(callee: &'b Expression<'b>) -> Option<&'b str> {
        if let Expression::Identifier(id) = callee {
            Some(id.name.as_str())
        } else {
            None
        }
    }
}
impl<'a> Visit<'a> for Recognizer<'a> {
    fn visit_function(&mut self, it: &Function<'a>, _flags: oxc_syntax::scope::ScopeFlags) {
        self.unknown(
            it.span,
            "nested function body not flattened into handler sequence",
        );
    }
    fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'a>) {
        self.unknown(
            it.span,
            "nested arrow body not flattened into handler sequence",
        );
    }
    fn visit_binary_expression(&mut self, it: &BinaryExpression<'a>) {
        let op = it.operator.as_str();
        let kind = match op {
            "+" => MicroOperationKind::JsAdd,
            "-" => MicroOperationKind::JsSubtract,
            "^" => MicroOperationKind::JsXor,
            _ => MicroOperationKind::JsBinary,
        };
        self.action(kind, Some(op.into()), &["left", "right"], it.span);
        walk::walk_binary_expression(self, it);
    }
    fn visit_assignment_expression(&mut self, it: &AssignmentExpression<'a>) {
        self.visit_assignment_target(&it.left);
        self.visit_expression(&it.right);
    }
    fn visit_static_member_expression(&mut self, it: &StaticMemberExpression<'a>) {
        let kind = if self.in_assignment_target {
            MicroOperationKind::PropertyWrite
        } else {
            MicroOperationKind::PropertyRead
        };
        self.action(
            kind,
            Some(it.property.name.to_string()),
            &["object", "property"],
            it.span,
        );
        walk::walk_static_member_expression(self, it);
    }
    fn visit_computed_member_expression(&mut self, it: &ComputedMemberExpression<'a>) {
        let kind = if self.in_assignment_target {
            MicroOperationKind::PropertyWrite
        } else {
            MicroOperationKind::PropertyRead
        };
        self.action(kind, None, &["object", "computed_property"], it.span);
        walk::walk_computed_member_expression(self, it);
    }
    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        let mut recognized_helper = false;
        if let Some((object, property)) = Self::callee_member(&it.callee) {
            match (object, property) {
                ("vmStack", "pop") | ("vmStack", "read") => {
                    recognized_helper = true;
                    self.action(MicroOperationKind::StackRead, None, &["top"], it.span);
                    self.effects.stack_reads += 1;
                    self.effects.stack_delta -= 1;
                }
                ("vmStack", "push") | ("vmStack", "write") => {
                    recognized_helper = true;
                    self.action(MicroOperationKind::StackWrite, None, &["value"], it.span);
                    self.effects.stack_writes += 1;
                    self.effects.stack_delta += 1;
                }
                ("vmMemory", "readSlot") => {
                    recognized_helper = true;
                    self.action(MicroOperationKind::MemoryReadSlot, None, &["slot"], it.span);
                    self.effects.memory_reads += 1;
                    self.effects.memory_slots.push("slot_argument".into());
                }
                ("vmMemory", "writeSlot") => {
                    recognized_helper = true;
                    self.action(
                        MicroOperationKind::MemoryWriteSlot,
                        None,
                        &["slot", "value"],
                        it.span,
                    );
                    self.effects.memory_writes += 1;
                    self.effects.memory_slots.push("slot_argument".into());
                }
                ("vmMemory", "initializeSlot") => {
                    recognized_helper = true;
                    self.action(
                        MicroOperationKind::MemoryInitializeSlot,
                        None,
                        &["slot", "value"],
                        it.span,
                    );
                    self.effects.memory_writes += 1;
                    self.effects.memory_slots.push("slot_argument".into());
                }
                _ => {
                    self.action(
                        MicroOperationKind::Call,
                        None,
                        &["callee", "arguments"],
                        it.span,
                    );
                    self.unknown(it.span, "call target is not a proven helper binding");
                    self.effects.calls_unknown_possible_throw = true;
                }
            }
        } else if let Some(name) = Self::direct_name(&it.callee) {
            if (name == "vmCaptureThread" || name == "vmThreadCapture")
                && self.options.proven_helper_bindings.contains(name)
            {
                recognized_helper = true;
                self.action(
                    MicroOperationKind::ThreadCapture,
                    None,
                    &["context", "configuration"],
                    it.span,
                );
                self.effects.thread_captures += 1;
            } else if name == "vmDecodeString" && self.options.proven_helper_bindings.contains(name)
            {
                recognized_helper = true;
                self.action(
                    MicroOperationKind::SharedStringDecode,
                    None,
                    &["encoded", "key", "table", "cache"],
                    it.span,
                );
            } else {
                self.action(
                    MicroOperationKind::Call,
                    None,
                    &["callee", "arguments"],
                    it.span,
                );
                self.unknown(it.span, "call target is not a proven helper binding");
                self.effects.calls_unknown_possible_throw = true;
            }
        } else {
            self.action(
                MicroOperationKind::Call,
                None,
                &["callee", "arguments"],
                it.span,
            );
            self.unknown(it.span, "computed call target is unknown");
            self.effects.calls_unknown_possible_throw = true;
        }
        if recognized_helper {
            for argument in &it.arguments {
                self.visit_argument(argument);
            }
        } else {
            walk::walk_call_expression(self, it);
        }
    }
    fn visit_new_expression(&mut self, it: &NewExpression<'a>) {
        self.action(
            MicroOperationKind::ConstructorCall,
            None,
            &["callee", "arguments"],
            it.span,
        );
        self.effects.constructor_calls += 1;
        self.effects.calls_unknown_possible_throw = true;
        walk::walk_new_expression(self, it);
    }
    fn visit_return_statement(&mut self, it: &ReturnStatement<'a>) {
        self.action(MicroOperationKind::Return, None, &["value"], it.span);
        self.effects.control_flow.push("return".into());
        walk::walk_return_statement(self, it);
    }
    fn visit_throw_statement(&mut self, it: &ThrowStatement<'a>) {
        self.action(MicroOperationKind::Throw, None, &["value"], it.span);
        self.effects.exception_effects.push("throw".into());
        walk::walk_throw_statement(self, it);
    }
    fn visit_if_statement(&mut self, it: &IfStatement<'a>) {
        self.action(
            MicroOperationKind::Branch,
            None,
            &["test", "then", "else"],
            it.span,
        );
        self.effects.control_flow.push("if".into());
        walk::walk_if_statement(self, it);
    }
    fn visit_while_statement(&mut self, it: &WhileStatement<'a>) {
        self.action(MicroOperationKind::Loop, None, &["test", "body"], it.span);
        self.effects.control_flow.push("while".into());
        walk::walk_while_statement(self, it);
    }
    fn visit_for_statement(&mut self, it: &ForStatement<'a>) {
        self.action(
            MicroOperationKind::Loop,
            None,
            &["init", "test", "update", "body"],
            it.span,
        );
        self.effects.control_flow.push("for".into());
        walk::walk_for_statement(self, it);
    }
    fn visit_do_while_statement(&mut self, it: &DoWhileStatement<'a>) {
        self.action(MicroOperationKind::Loop, None, &["body", "test"], it.span);
        self.effects.control_flow.push("do_while".into());
        walk::walk_do_while_statement(self, it);
    }
    fn visit_simple_assignment_target(&mut self, it: &SimpleAssignmentTarget<'a>) {
        let previous = self.in_assignment_target;
        self.in_assignment_target = true;
        walk::walk_simple_assignment_target(self, it);
        self.in_assignment_target = previous;
    }
}

fn normalize_handler(source: &str, scoping: &Scoping, handler: &Handler<'_>) -> String {
    let span = match handler {
        Handler::Function(f) => f.span,
        Handler::Arrow(f) => f.span,
    };
    let mut ids = SymbolCollector {
        scoping,
        symbols: BTreeSet::new(),
        replacements: Vec::new(),
    };
    match handler {
        Handler::Function(f) => {
            ids.visit_formal_parameters(&f.params);
            if let Some(body) = &f.body {
                ids.visit_function_body(body);
            }
        }
        Handler::Arrow(f) => {
            ids.visit_formal_parameters(&f.params);
            ids.visit_function_body(&f.body);
        }
    }
    let mut symbols: Vec<_> = ids.symbols.into_iter().collect();
    symbols.sort_by_key(|symbol| {
        let s = scoping.symbol_span(*symbol);
        (s.start, s.end)
    });
    let names: BTreeMap<SymbolId, String> = symbols
        .into_iter()
        .enumerate()
        .map(|(i, s)| (s, format!("b{i}")))
        .collect();
    let mut replacements = ids.replacements;
    replacements.sort_by_key(|(span, _)| (span.start, span.end));
    let mut out = String::new();
    let mut cursor = span.start as usize;
    for (replacement_span, symbol) in replacements {
        if replacement_span.start < span.start
            || replacement_span.end > span.end
            || replacement_span.start < cursor as u32
        {
            continue;
        }
        out.push_str(&source[cursor..replacement_span.start as usize]);
        out.push_str(
            names
                .get(&symbol)
                .map_or("free".into(), Clone::clone)
                .as_str(),
        );
        cursor = replacement_span.end as usize;
    }
    out.push_str(&source[cursor..span.end as usize]);
    let mut compact = String::with_capacity(out.len());
    let mut whitespace = false;
    for ch in out.chars() {
        if ch.is_whitespace() {
            whitespace = true;
        } else {
            if whitespace && !compact.is_empty() {
                compact.push(' ');
            }
            whitespace = false;
            compact.push(ch);
        }
    }
    compact
}
struct SymbolCollector<'a> {
    scoping: &'a Scoping,
    symbols: BTreeSet<SymbolId>,
    replacements: Vec<(Span, SymbolId)>,
}
impl<'a> Visit<'a> for SymbolCollector<'a> {
    fn visit_binding_identifier(&mut self, it: &BindingIdentifier<'a>) {
        if let Some(symbol) = it.symbol_id.get() {
            self.symbols.insert(symbol);
            self.replacements.push((it.span, symbol));
        }
    }
    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
        if let Some(symbol) = analysis::identifier_symbol(it, self.scoping) {
            self.symbols.insert(symbol);
            self.replacements.push((it.span, symbol));
        }
    }
    fn visit_function(&mut self, it: &Function<'a>, _flags: oxc_syntax::scope::ScopeFlags) {
        walk::walk_function(self, it, _flags);
    }
}

/// Serialize a value into the versioned operations JSON layout.
pub(crate) fn to_json<T: Serialize + ?Sized>(value: &T) -> String {
    let mut buffer = Vec::new();
    let mut serializer = serde_json::Serializer::with_formatter(&mut buffer, JsonLayout::default());
    value
        .serialize(&mut serializer)
        .expect("operations JSON serialization is infallible");
    String::from_utf8(buffer).expect("serde_json always emits valid UTF-8")
}

/// Reproduces the historical operations JSON bytes: the top-level object is
/// indented two spaces with one field per line, while every nested value is
/// emitted compactly. String escaping matches [`crate::util::json_string`], including
/// `\u00XX` for DEL and the C1 controls and `\u0008`/`\u000c` for backspace and
/// form feed, which `serde_json`'s defaults omit or shorten.
#[derive(Default)]
struct JsonLayout {
    depth: usize,
}

impl Formatter for JsonLayout {
    fn begin_object<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        if self.depth == 0 {
            writer.write_all(b"{\n")?;
        } else {
            writer.write_all(b"{")?;
        }
        self.depth += 1;
        Ok(())
    }

    fn end_object<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        self.depth -= 1;
        if self.depth == 0 {
            // The historical layout gives every field a trailing newline and
            // then closes on its own line, so the top level ends in a blank
            // line followed by `}`.
            writer.write_all(b"\n\n}")
        } else {
            writer.write_all(b"}")
        }
    }

    fn begin_array<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        self.depth += 1;
        writer.write_all(b"[")
    }

    fn end_array<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        self.depth -= 1;
        writer.write_all(b"]")
    }

    fn begin_array_value<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        if first {
            Ok(())
        } else {
            writer.write_all(b",")
        }
    }

    fn begin_object_key<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        if self.depth == 1 {
            if first {
                writer.write_all(b"  ")
            } else {
                writer.write_all(b",\n  ")
            }
        } else if first {
            Ok(())
        } else {
            writer.write_all(b",")
        }
    }

    fn begin_object_value<W>(&mut self, writer: &mut W) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        if self.depth == 1 {
            writer.write_all(b": ")
        } else {
            writer.write_all(b":")
        }
    }

    fn write_string_fragment<W>(&mut self, writer: &mut W, fragment: &str) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        let bytes = fragment.as_bytes();
        let mut start = 0;
        for (index, character) in fragment.char_indices() {
            if character.is_control() {
                writer.write_all(&bytes[start..index])?;
                write!(writer, "\\u{:04x}", character as u32)?;
                start = index + character.len_utf8();
            }
        }
        writer.write_all(&bytes[start..])
    }

    fn write_char_escape<W>(&mut self, writer: &mut W, escape: CharEscape) -> io::Result<()>
    where
        W: ?Sized + Write,
    {
        let bytes: &[u8] = match escape {
            CharEscape::Quote => b"\\\"",
            CharEscape::ReverseSolidus => b"\\\\",
            CharEscape::Solidus => b"\\/",
            CharEscape::Backspace => b"\\u0008",
            CharEscape::FormFeed => b"\\u000c",
            CharEscape::LineFeed => b"\\n",
            CharEscape::CarriageReturn => b"\\r",
            CharEscape::Tab => b"\\t",
            CharEscape::AsciiControl(byte) => return write!(writer, "\\u{:04x}", byte),
        };
        writer.write_all(bytes)
    }
}

impl From<PipelineError> for CatalogueError {
    fn from(error: PipelineError) -> Self {
        Self::Parse(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use crate::util::json_string;

    use super::to_json;

    fn reference_json_string(value: &str) -> String {
        let mut out = String::from("\"");
        for ch in value.chars() {
            match ch {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }

    /// The serde formatter must reproduce the frozen escaper byte for byte,
    /// including DEL/C1 (which serde leaves raw) and backspace/form feed (which
    /// serde shortens to `\b`/`\f`). Raw line terminators cannot appear inside a
    /// parsed JS handler snippet, so the full code-point range is pinned here.
    #[test]
    fn json_layout_escaping_matches_reference_across_control_code_points() {
        let mut corpus = String::new();
        for code in 0x00u32..=0x1f {
            corpus.push(char::from_u32(code).unwrap());
        }
        corpus.push('"');
        corpus.push('/');
        corpus.push('\\');
        for code in 0x7f..=0x9f {
            corpus.push(char::from_u32(code).unwrap());
        }
        corpus.push_str("café 日本語 😀 \u{2028}\u{2029}");

        assert_eq!(to_json(&corpus), reference_json_string(&corpus));
        assert_eq!(json_string(&corpus), reference_json_string(&corpus));

        assert_eq!(json_string("\0"), "\"\\u0000\"");
        assert_eq!(json_string("\u{8}"), "\"\\u0008\"");
        assert_eq!(json_string("\u{c}"), "\"\\u000c\"");
        assert_eq!(json_string("\u{7f}"), "\"\\u007f\"");
        assert_eq!(json_string("\u{85}"), "\"\\u0085\"");
        assert_eq!(json_string("\u{9f}"), "\"\\u009f\"");
        assert_eq!(json_string("\""), "\"\\\"\"");
        assert_eq!(json_string("\\"), "\"\\\\\"");
        assert_eq!(json_string("/"), "\"/\"");
        assert_eq!(json_string("café"), "\"café\"");
    }
}
