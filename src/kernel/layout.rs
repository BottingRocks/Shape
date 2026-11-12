//! Opt-in static inventory of the post-rename Shape VM layout.
//!
//! This module is deliberately a locator, not a VM.  It parses source with Oxc,
//! consumes the existing pipeline hand-off and operation catalogue, and records
//! source provenance for things that can be proven without evaluating JavaScript
//! or WebAssembly.  Unknown relationships stay visible in the report.

use std::{collections::BTreeMap, fmt};

use crate::transforms::deob::pipeline::{
    deobfuscate_with_report, ExtractionFact, ExtractionKind, ExtractionStatus, PipelineError,
    PipelineReport,
};
use crate::{
    analysis, catalogue_from_report,
    kernel::properties::{extract_actual_properties, ActualProperties},
    kernel::wasm_module::{self, ModuleLimits, ValidationStatus},
    operations::catalogue::{
        Classification, MicroOperationKind, OperationCatalogue, OperationRecord, SemanticFamily,
    },
    util::{fnv1a64_hex, json_string},
};

pub const SCHEMA_VERSION: &str = "shape-kernel-layout";
pub const ANALYSIS_VERSION: &str = "shape-oxc-deob/kernel-layout";

#[derive(Debug)]
pub enum KernelLayoutError {
    Pipeline(PipelineError),
    Parse(String),
}

impl fmt::Display for KernelLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pipeline(error) => write!(f, "kernel layout pipeline failed: {error}"),
            Self::Parse(error) => write!(f, "kernel layout parse failed: {error}"),
        }
    }
}

impl std::error::Error for KernelLayoutError {}

impl From<PipelineError> for KernelLayoutError {
    fn from(value: PipelineError) -> Self {
        Self::Pipeline(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpanRecord {
    pub start: u32,
    pub end: u32,
    pub line_start: u32,
    pub line_end: u32,
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLink {
    pub artifact: String,
    pub span: SpanRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelComponent {
    pub id: String,
    pub display_name: Option<String>,
    pub kind: String,
    pub status: String,
    pub confidence: String,
    pub count: Option<u64>,
    pub symbol_identity: Option<String>,
    pub raw_source: Option<SourceLink>,
    pub deob_source: Option<SourceLink>,
    pub pass_lineage: Vec<String>,
    pub evidence: Vec<String>,
    pub unknown_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutRelationship {
    pub from: String,
    pub relation: String,
    pub to: String,
    pub status: String,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmModuleLayout {
    pub source_binding: Option<String>,
    pub byte_len: Option<u64>,
    pub digest: Option<String>,
    pub imports: Vec<String>,
    pub exports: Vec<String>,
    pub validated: bool,
    pub source: Option<SourceLink>,
    pub unknown_reason: Option<String>,
}

/// Options for the static layout report. Per-handler operation records are
/// diagnostics only and are deliberately not materialized by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KernelLayoutOptions {
    pub include_operation_diagnostics: bool,
}

/// Compact facts about the operation table and the bounded recognizer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationInventory {
    pub table_identity: String,
    pub handler_count: u32,
    pub atomic_handler_count: u32,
    pub composite_handler_count: u32,
    pub unknown_handler_count: u32,
    pub sparse_entry_count: u32,
    pub dispatch_branch_count: Option<u32>,
    /// This remains a fact/unknown statement, not a guessed selector mapping.
    pub dispatch_link: String,
}

/// Optional per-handler inspection records from the same catalogue pass as the
/// compact operation inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationDiagnostics {
    pub records: Vec<OperationRecordSummary>,
}

/// A bounded recognizer summary. Its family and classification describe only
/// the catalogue recognizers, not HandlerIR completeness or execution admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationRecordSummary {
    pub handler_index: u32,
    pub source: SourceLink,
    pub classification: Classification,
    pub semantic_family: SemanticFamily,
    pub recognized_actions: Vec<MicroOperationKind>,
    pub unknown_fragments: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KernelLayout {
    pub schema_version: String,
    pub analysis_version: String,
    pub raw_source_hash: String,
    pub deob_source_hash: String,
    pub deob_output_unchanged: bool,
    pub components: Vec<KernelComponent>,
    pub relationships: Vec<LayoutRelationship>,
    pub wasm_modules: Vec<WasmModuleLayout>,
    pub operations: Option<OperationInventory>,
    pub operation_diagnostics: Option<OperationDiagnostics>,
    pub readiness: String,
    pub pending: Vec<String>,
    /// Concise actual values from the deobfuscated source. The static locator
    /// fields above remain available to callers that need the verbose report.
    pub properties: ActualProperties,
}

/// Build the explicit, static kernel-layout report.
pub fn static_kernel_layout(source: &str) -> Result<KernelLayout, KernelLayoutError> {
    static_kernel_layout_with_options(source, KernelLayoutOptions::default())
}

/// Build the static layout report, optionally retaining bounded per-handler
/// diagnostics for inspection.
pub fn static_kernel_layout_with_options(
    source: &str,
    options: KernelLayoutOptions,
) -> Result<KernelLayout, KernelLayoutError> {
    if source.is_empty() {
        return Err(KernelLayoutError::Pipeline(PipelineError::InvalidInput(
            "source is empty".into(),
        )));
    }
    let report = deobfuscate_with_report(source)?;
    build_layout(source, &report, options)
}

/// Alias used by callers that prefer a report-shaped API.
pub fn report(source: &str) -> Result<KernelLayout, KernelLayoutError> {
    static_kernel_layout(source)
}

pub fn report_with_options(
    source: &str,
    options: KernelLayoutOptions,
) -> Result<KernelLayout, KernelLayoutError> {
    static_kernel_layout_with_options(source, options)
}

/// Alias used by the opt-in binary and downstream analysis tools.
pub fn analyze(source: &str) -> Result<KernelLayout, KernelLayoutError> {
    static_kernel_layout(source)
}

pub fn analyze_with_options(
    source: &str,
    options: KernelLayoutOptions,
) -> Result<KernelLayout, KernelLayoutError> {
    static_kernel_layout_with_options(source, options)
}

fn build_layout(
    raw: &str,
    pipeline: &PipelineReport,
    options: KernelLayoutOptions,
) -> Result<KernelLayout, KernelLayoutError> {
    let output = &pipeline.output;
    let properties = extract_actual_properties(output)
        .map_err(|error| KernelLayoutError::Parse(error.to_string()))?;
    let output_symbols = output_symbol_index(output)?;
    let mut components = Vec::new();
    let mut role_components = BTreeMap::new();

    for mapping in &pipeline.binding_mappings {
        let role = mapping.role.clone().unwrap_or_else(|| "binding".into());
        let id = format!("role:{role}");
        if role_components.contains_key(&id) {
            continue;
        }
        let deob = find_identifier_link(output, &mapping.canonical_name, &mapping.canonical_name);
        let raw_link = raw_link_if_proven(raw, mapping.span, &mapping.original_name);
        let mut evidence = vec![format!(
            "RENAME mapping `{}` -> `{}`; canonical spelling is display metadata only",
            mapping.original_name, mapping.canonical_name
        )];
        let unknown_reason = if raw_link.is_none() {
            evidence.push("raw span was not reused because CLEAN/output offsets differ".into());
            Some("raw source correlation was not proven at this byte span".into())
        } else {
            None
        };
        let symbol_identity =
            resolve_output_symbol_identity(&output_symbols, &mapping.canonical_name);
        let component = KernelComponent {
            id: id.clone(),
            display_name: Some(mapping.canonical_name.clone()),
            kind: role_kind(&role),
            status: "located".into(),
            confidence: if symbol_identity.is_some() {
                "high"
            } else {
                "partial"
            }
            .into(),
            count: None,
            symbol_identity,
            raw_source: raw_link,
            deob_source: deob,
            pass_lineage: vec!["CLEAN".into(), "RENAME".into(), "FIND".into()],
            evidence,
            unknown_reason,
        };
        role_components.insert(id, components.len());
        components.push(component);
    }

    let mut property_ids = std::collections::BTreeSet::new();
    for mapping in &pipeline.property_mappings {
        let id = format!("property:{}:{}", mapping.owner, mapping.semantic_name);
        if !property_ids.insert(id.clone()) {
            continue;
        }
        let raw_link = raw_link_if_proven(raw, mapping.span, &mapping.original_name);
        components.push(KernelComponent {
            id,
            display_name: Some(mapping.semantic_name.clone()),
            kind: format!("{} layout property", mapping.owner),
            status: "located".into(),
            confidence: "high".into(),
            count: None,
            symbol_identity: None,
            raw_source: raw_link,
            deob_source: find_identifier_link(
                output,
                &mapping.semantic_name,
                &mapping.semantic_name,
            ),
            pass_lineage: vec!["CLEAN".into(), "RENAME".into()],
            evidence: vec![format!(
                "typed receiver property mapping `{}` -> `{}`; owner `{}` is structural evidence",
                mapping.original_name, mapping.semantic_name, mapping.owner
            )],
            unknown_reason: None,
        });
    }

    for fact in &pipeline.extraction {
        let component = extraction_component(raw, output, fact, pipeline, &output_symbols);
        components.push(component);
    }

    let known_ids: std::collections::BTreeSet<String> =
        components.iter().map(|c| c.id.clone()).collect();
    for (id, kind, reason) in required_unknown_components(&known_ids) {
        components.push(KernelComponent {
            id: id.into(),
            display_name: None,
            kind: kind.into(),
            status: "unknown".into(),
            confidence: "unknown".into(),
            count: None,
            symbol_identity: None,
            raw_source: None,
            deob_source: None,
            pass_lineage: vec!["static locator".into()],
            evidence: Vec::new(),
            unknown_reason: Some(reason.into()),
        });
    }

    let catalogue = catalogue_from_report(pipeline).ok();
    let operations = catalogue.as_ref().map(operation_inventory);
    let operation_diagnostics = options
        .include_operation_diagnostics
        .then(|| {
            catalogue
                .as_ref()
                .map(|value| operation_diagnostics(output, value))
        })
        .flatten();
    if let Some(value) = &operations {
        components.push(KernelComponent {
            id: "operations:numeric-js-handlers".into(),
            display_name: Some(value.table_identity.clone()),
            kind: "original numeric JS operations".into(),
            status: "located".into(),
            confidence: "partial".into(),
            count: Some(value.handler_count as u64),
            symbol_identity: Some(value.table_identity.clone()),
            raw_source: None,
            deob_source: None,
            pass_lineage: vec![
                "CLEAN".into(),
                "RENAME".into(),
                "EXTRACT".into(),
                "operation catalogue".into(),
            ],
            evidence: vec![
                "operation table re-resolved by declaration ordinal and initializer shape".into(),
                "ordered micro-actions are reused from the existing operation catalogue".into(),
                "partial recognitions do not certify whole handlers".into(),
            ],
            unknown_reason: Some(
                "classifier coverage is partial; unsupported fragments remain visible".into(),
            ),
        });
    }

    let wasm_modules = pipeline
        .extraction
        .iter()
        .filter(|fact| fact.kind == ExtractionKind::WasmModule)
        .map(|fact| wasm_layout(raw, output, fact))
        .collect::<Vec<_>>();
    let relationships = relationships(&components, operations.as_ref());
    let mut pending = vec![
        "Static layout inventory does not establish executable VM readiness".into(),
        "dynamic JS coercion, calls, throws, and unsupported fragments remain explicit".into(),
        "loop backedges and loop-carried state need tracer events; exception regions must complete normal, return, throw, break, and continue paths".into(),
        "WASM operand widths and dispatch-index-to-handler mapping are not inferred".into(),
    ];
    if wasm_modules.is_empty() {
        pending.push("no statically proven WebAssembly module was found in this input".into());
    }
    let readiness = if operations.is_some() || !components.is_empty() {
        "static-readiness-only".into()
    } else {
        "unproven".into()
    };
    Ok(KernelLayout {
        schema_version: SCHEMA_VERSION.into(),
        analysis_version: ANALYSIS_VERSION.into(),
        raw_source_hash: fnv1a64_hex(raw.as_bytes()),
        deob_source_hash: fnv1a64_hex(output.as_bytes()),
        deob_output_unchanged: raw == output,
        components,
        relationships,
        wasm_modules,
        operations,
        operation_diagnostics,
        readiness,
        pending,
        properties,
    })
}

fn extraction_component(
    raw: &str,
    output: &str,
    fact: &ExtractionFact,
    report: &PipelineReport,
    output_symbols: &BTreeMap<String, Vec<String>>,
) -> KernelComponent {
    let id = format!(
        "extraction:{}:{}",
        fact.kind,
        fact.binding.as_deref().unwrap_or("anonymous")
    );
    let display_name = fact.binding.clone();
    let deob = display_name
        .as_deref()
        .and_then(|name| find_identifier_link(output, name, name));
    let raw_link = fact.binding.as_deref().and_then(|name| {
        let original = report
            .binding_mappings
            .iter()
            .find(|mapping| mapping.canonical_name == name)
            .map(|mapping| mapping.original_name.as_str())
            .unwrap_or(name);
        raw_link_if_proven(raw, (fact.span.start, fact.span.end), original)
    });
    let status = fact.status.to_string();
    let unknown_reason = if status == "extracted" && (deob.is_none() || raw_link.is_none()) {
        Some(
            "fact was proven in a pipeline pass, but the exact cross-pass byte span was not proven"
                .into(),
        )
    } else if status != "extracted" {
        Some(fact.evidence.clone())
    } else {
        None
    };
    KernelComponent {
        id,
        display_name,
        kind: fact.kind.to_string(),
        status,
        confidence: if fact.status == ExtractionStatus::Extracted {
            "high"
        } else {
            "unknown"
        }
        .into(),
        count: fact.length.map(|value| value as u64),
        symbol_identity: fact
            .binding
            .as_deref()
            .and_then(|name| resolve_output_symbol_identity(output_symbols, name)),
        raw_source: raw_link,
        deob_source: deob,
        pass_lineage: vec!["CLEAN".into(), "RENAME".into(), "EXTRACT".into()],
        evidence: vec![fact.evidence.clone()],
        unknown_reason,
    }
}

fn operation_inventory(catalogue: &OperationCatalogue) -> OperationInventory {
    OperationInventory {
        table_identity: format!(
            "ordinal:{}:{}:{}",
            catalogue.table.declaration_ordinal,
            catalogue.table.element_count,
            catalogue.table.function_count
        ),
        handler_count: catalogue.table.function_count,
        atomic_handler_count: catalogue.inventory.exact,
        composite_handler_count: catalogue.inventory.partial,
        unknown_handler_count: catalogue.inventory.unknown,
        sparse_entry_count: catalogue.inventory.missing_sparse_entries,
        dispatch_branch_count: None,
        dispatch_link:
            "unproven; decoder branch-to-handler relationship and selector linkage were not established".into(),
    }
}

fn operation_diagnostics(output: &str, catalogue: &OperationCatalogue) -> OperationDiagnostics {
    OperationDiagnostics {
        records: catalogue
            .handlers
            .iter()
            .map(|record| operation_record(output, record))
            .collect(),
    }
}

fn operation_record(output: &str, record: &OperationRecord) -> OperationRecordSummary {
    let source = SourceLink {
        artifact: "deobfuscated-output".into(),
        span: source_span(output, record.source_span.start, record.source_span.end),
    };
    OperationRecordSummary {
        handler_index: record.handler_index,
        source,
        classification: record.classification,
        semantic_family: record.semantic_family,
        recognized_actions: record
            .ordered_micro_operations
            .iter()
            .map(|operation| operation.kind)
            .collect(),
        unknown_fragments: record
            .unknown_fragments
            .iter()
            .map(|fragment| fragment.reason.clone())
            .collect(),
    }
}

fn relationships(
    components: &[KernelComponent],
    operations: Option<&OperationInventory>,
) -> Vec<LayoutRelationship> {
    let ids: std::collections::BTreeSet<&str> = components
        .iter()
        .map(|component| component.id.as_str())
        .collect();
    let mut result = Vec::new();
    add_relationship(
        &mut result,
        &ids,
        "role:vmEntry",
        "calls",
        "role:run",
        "entry-to-run call is role evidence",
    );
    add_relationship(
        &mut result,
        &ids,
        "role:vmContext",
        "reads/writes",
        "role:vmMemory",
        "context carries memory and dispatch fields",
    );
    add_relationship(
        &mut result,
        &ids,
        "role:vmContext",
        "reads/writes",
        "role:vmStack",
        "context owns operand and error stacks",
    );
    add_relationship(
        &mut result,
        &ids,
        "role:vmWorker",
        "captures/calls",
        "role:vmWorkerCallbacks",
        "worker callback array is a separate role",
    );
    if operations.is_some() {
        add_relationship(
            &mut result,
            &ids,
            "role:vmOperations",
            "dispatches",
            "operations:numeric-js-handlers",
            "operation table identity was handed off by ordinal",
        );
    }
    result.push(LayoutRelationship {
        from: "dispatch:decoder".into(),
        relation: "dispatches".into(),
        to: "operations:numeric-js-handlers".into(),
        status: "unknown".into(),
        evidence: "decoder branch count, operand widths, and branch-to-handler mapping are intentionally not invented".into(),
    });
    result
}

fn add_relationship(
    result: &mut Vec<LayoutRelationship>,
    ids: &std::collections::BTreeSet<&str>,
    from: &str,
    relation: &str,
    to: &str,
    evidence: &str,
) {
    if ids.contains(from) && ids.contains(to) {
        result.push(LayoutRelationship {
            from: from.into(),
            relation: relation.into(),
            to: to.into(),
            status: "proven-structural".into(),
            evidence: evidence.into(),
        });
    }
}

fn required_unknown_components<'a>(
    known: &std::collections::BTreeSet<String>,
) -> Vec<(&'a str, &'a str, &'a str)> {
    [
        (
            "dispatch:decoder",
            "dispatch decoder",
            "decoder function/table relation is not independently proven by the current hand-off",
        ),
        (
            "dispatch:state-transition",
            "dispatch state transition",
            "state transition edges and operand widths are not statically mapped",
        ),
        (
            "vm:exception-error-stack",
            "exception/error stack",
            "handler stack and catch/finally completion require runtime/tracer evidence",
        ),
        (
            "vm:guarded-call",
            "guarded call",
            "throw boundary is not a whole-handler proof",
        ),
        (
            "vm:run-dispatch-loop",
            "run/dispatch loop",
            "loop backedges and loop-carried state are not emitted by the locator",
        ),
    ]
    .into_iter()
    .filter(|(id, _, _)| !known.contains(*id))
    .collect()
}

fn wasm_layout(raw: &str, output: &str, fact: &ExtractionFact) -> WasmModuleLayout {
    let Some(bytes) = fact.decoded_bytes.as_deref() else {
        return WasmModuleLayout {
            source_binding: fact.binding.clone(),
            byte_len: fact.length.map(|v| v as u64),
            digest: None,
            imports: Vec::new(),
            exports: Vec::new(),
            validated: false,
            source: fact
                .binding
                .as_deref()
                .and_then(|name| find_identifier_link(output, name, name)),
            unknown_reason: Some(fact.evidence.clone()),
        };
    };
    let (imports, exports, validated, error) = parse_wasm_abi(bytes);
    WasmModuleLayout {
        source_binding: fact.binding.clone(),
        byte_len: Some(bytes.len() as u64),
        digest: Some(fnv1a64_hex(bytes)),
        imports,
        exports,
        validated,
        source: fact
            .binding
            .as_deref()
            .and_then(|name| find_identifier_link(output, name, name)),
        unknown_reason: error.or_else(|| {
            if raw.is_empty() {
                Some("raw source is empty".into())
            } else {
                None
            }
        }),
    }
}

fn parse_wasm_abi(bytes: &[u8]) -> (Vec<String>, Vec<String>, bool, Option<String>) {
    let Ok(summary) = wasm_module::inspect_module(bytes, ModuleLimits::for_input(bytes)) else {
        return (
            Vec::new(),
            Vec::new(),
            false,
            Some("WASM byte limit exceeded".into()),
        );
    };
    if let ValidationStatus::Invalid(error) = &summary.validation {
        return (Vec::new(), Vec::new(), false, Some(error.clone()));
    }
    let abi = &summary.abi;
    let project_export = |export: &wasm_module::AbiExport| {
        export.kind.is_layout_export().then(|| export.name.clone())
    };
    if let Some(error) = &abi.first_error {
        // The old layout reader returned the vectors it had already built and
        // a fixed string per error class.  Preserve that, including the prefix
        // of readable imports/exports before the first failure.
        let imports = abi.imports[..error.imports]
            .iter()
            .map(|import| format!("{}::{}", import.namespace, import.name))
            .collect();
        let exports = abi.exports[..error.exports]
            .iter()
            .filter_map(project_export)
            .collect();
        let reason = match error.kind {
            wasm_module::AbiErrorKind::Payload => "WASM payload parse failed",
            wasm_module::AbiErrorKind::Import => "WASM import parse failed",
            wasm_module::AbiErrorKind::Export => "WASM export parse failed",
        };
        return (imports, exports, false, Some(reason.into()));
    }
    let imports = abi
        .imports
        .iter()
        .map(|import| format!("{}::{}", import.namespace, import.name))
        .collect();
    let exports = abi.exports.iter().filter_map(project_export).collect();
    (imports, exports, true, None)
}

fn output_symbol_index(output: &str) -> Result<BTreeMap<String, Vec<String>>, KernelLayoutError> {
    let allocator = oxc_allocator::Allocator::default();
    let program = analysis::parse_program(&allocator, output).map_err(KernelLayoutError::Parse)?;
    let (scoping, _) = analysis::analyze(&program);
    let mut index = BTreeMap::<String, Vec<String>>::new();
    for symbol in scoping
        .iter_bindings()
        .flat_map(|(_, bindings)| bindings.iter().map(|(_, symbol)| *symbol))
    {
        index
            .entry(scoping.symbol_name(symbol).to_owned())
            .or_default()
            .push(format!(
                "symbol:{}@{}..{}",
                symbol.index(),
                scoping.symbol_span(symbol).start,
                scoping.symbol_span(symbol).end
            ));
    }
    Ok(index)
}

fn resolve_output_symbol_identity(
    index: &BTreeMap<String, Vec<String>>,
    name: &str,
) -> Option<String> {
    let matches = index.get(name)?;
    (matches.len() == 1).then(|| matches[0].clone())
}

fn role_kind(role: &str) -> String {
    match role {
        "vmEntry" => "vm entry".into(),
        "vmContext" => "context".into(),
        "vmMemory" | "vmLinearMemory" => "memory layout".into(),
        "vmStack" | "vmCreateStack" => "stack".into(),
        "vmWorker" | "vmWorkerCallbacks" | "initialize_worker" => {
            "worker bootstrap/callbacks".into()
        }
        "vmCaptureThread" | "vmBuildThreadInvocation" | "vmInvokeThread" | "vmExecuteThread" => {
            "thread capture/build/invocation".into()
        }
        "vmSlotDescriptor" => "slot descriptor".into(),
        _ => role.into(),
    }
}

fn raw_link_if_proven(source: &str, span: (u32, u32), expected: &str) -> Option<SourceLink> {
    let start = span.0 as usize;
    let end = span.1 as usize;
    if start > end || end > source.len() || source.get(start..end) != Some(expected) {
        return None;
    }
    Some(SourceLink {
        artifact: "raw-source".into(),
        span: source_span(source, span.0, span.1),
    })
}

fn find_identifier_link(source: &str, name: &str, _identity_hint: &str) -> Option<SourceLink> {
    if name.is_empty() {
        return None;
    }
    let bytes = source.as_bytes();
    let needle = name.as_bytes();
    let mut offset = 0;
    while let Some(relative) = source[offset..].find(name) {
        let start = offset + relative;
        let end = start + needle.len();
        let left = start == 0 || !is_identifier_byte(bytes[start - 1]);
        let right = end == bytes.len() || !is_identifier_byte(bytes[end]);
        if left && right {
            return Some(SourceLink {
                artifact: "deobfuscated-output".into(),
                span: source_span(source, start as u32, end as u32),
            });
        }
        offset = end;
    }
    None
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

fn source_span(source: &str, start: u32, end: u32) -> SpanRecord {
    let start_usize = start.min(source.len() as u32) as usize;
    let end_usize = end.min(source.len() as u32).max(start) as usize;
    let line_start = source[..start_usize]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count() as u32
        + 1;
    let line_end = source[..end_usize]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count() as u32
        + 1;
    let line_begin = source[..start_usize]
        .rfind('\n')
        .map_or(0, |index| index + 1);
    let line_finish = source[end_usize..]
        .find('\n')
        .map_or(source.len(), |index| end_usize + index);
    SpanRecord {
        start,
        end,
        line_start,
        line_end,
        snippet: source[line_begin..line_finish].trim().to_owned(),
    }
}

/// Serialize the concise actual-properties object used by the default CLI.
pub fn layout_json(layout: &KernelLayout) -> String {
    layout.properties.json()
}

/// Serialize the pre-properties locator report for consumers that still need
/// spans, evidence, confidence, hashes, and relationship inventory.
pub fn layout_verbose_json(layout: &KernelLayout) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    json_field(
        &mut out,
        1,
        "schema_version",
        &json_string(&layout.schema_version),
        true,
    );
    json_field(
        &mut out,
        1,
        "analysis_version",
        &json_string(&layout.analysis_version),
        true,
    );
    json_field(
        &mut out,
        1,
        "raw_source_hash",
        &json_string(&layout.raw_source_hash),
        true,
    );
    json_field(
        &mut out,
        1,
        "deob_source_hash",
        &json_string(&layout.deob_source_hash),
        true,
    );
    json_field(
        &mut out,
        1,
        "deob_output_unchanged",
        if layout.deob_output_unchanged {
            "true"
        } else {
            "false"
        },
        true,
    );
    json_field(
        &mut out,
        1,
        "readiness",
        &json_string(&layout.readiness),
        true,
    );
    json_field(
        &mut out,
        1,
        "components",
        &components_json(&layout.components),
        true,
    );
    json_field(
        &mut out,
        1,
        "relationships",
        &relationships_json(&layout.relationships),
        true,
    );
    json_field(
        &mut out,
        1,
        "wasm_modules",
        &wasm_json(&layout.wasm_modules),
        true,
    );
    json_field(
        &mut out,
        1,
        "operations",
        &operations_json(layout.operations.as_ref()),
        true,
    );
    if let Some(diagnostics) = &layout.operation_diagnostics {
        json_field(
            &mut out,
            1,
            "operation_diagnostics",
            &operation_diagnostics_json(diagnostics),
            true,
        );
    }
    json_field(
        &mut out,
        1,
        "pending",
        &string_array(&layout.pending),
        false,
    );
    out.push_str("\n}");
    out
}

/// Human-readable rendering of the same concise property object.
pub fn layout_markdown(layout: &KernelLayout) -> String {
    layout.properties.markdown()
}

/// Render the static locator report as markdown.
pub fn layout_verbose_markdown(layout: &KernelLayout) -> String {
    let mut out = format!("# Kernel layout\n\n- Schema: `{}`\n- Raw hash: `{}`\n- Deobfuscated hash: `{}`\n- Readiness: `{}`\n- Default `shape deob` output changed: `{}`\n\n", layout.schema_version, layout.raw_source_hash, layout.deob_source_hash, layout.readiness, layout.deob_output_unchanged);
    out.push_str("## Components\n\n| ID | Kind | Status | Confidence | Count | Source | Unknown |\n|---|---|---|---|---:|---|---|\n");
    for component in &layout.components {
        let source = component.deob_source.as_ref().map_or("—".into(), |link| {
            format!(
                "{}..{} (line {})",
                link.span.start, link.span.end, link.span.line_start
            )
        });
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | `{}` | {} |\n",
            component.id,
            component.kind,
            component.status,
            component.confidence,
            component.count.map_or("—".into(), |v| v.to_string()),
            source,
            component
                .unknown_reason
                .as_deref()
                .unwrap_or("—")
                .replace('|', "\\|")
        ));
    }
    out.push_str("\n## Relationships\n\n| From | Relation | To | Status | Evidence |\n|---|---|---|---|---|\n");
    for relation in &layout.relationships {
        out.push_str(&format!(
            "| `{}` | {} | `{}` | {} | {} |\n",
            relation.from, relation.relation, relation.to, relation.status, relation.evidence
        ));
    }
    if let Some(operations) = &layout.operations {
        out.push_str(&format!(
            "\n## Numeric operation catalogue\n\n- Table identity: `{}`\n- Handlers: {}\n- Atomic recognizer classifications: {}\n- Composite recognizer classifications: {}\n- Unknown recognizer classifications: {}\n- Sparse entries: {}\n- Dispatch link: {}\n",
            operations.table_identity,
            operations.handler_count,
            operations.atomic_handler_count,
            operations.composite_handler_count,
            operations.unknown_handler_count,
            operations.sparse_entry_count,
            operations.dispatch_link
        ));
    }
    if let Some(diagnostics) = &layout.operation_diagnostics {
        out.push_str(
            "\n## Operation diagnostics\n\n| Handler | Family | Classification | Recognized actions | Unknown fragments | Output span |\n|---:|---|---|---|---|---|\n",
        );
        for record in &diagnostics.records {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | `{}` {}..{} |\n",
                record.handler_index,
                record.semantic_family,
                record.classification,
                record
                    .recognized_actions
                    .iter()
                    .map(|action| action.as_str())
                    .collect::<Vec<_>>()
                    .join(" → "),
                if record.unknown_fragments.is_empty() {
                    "—".into()
                } else {
                    record.unknown_fragments.join("; ")
                },
                record.source.artifact,
                record.source.span.start,
                record.source.span.end
            ));
        }
    }
    out.push_str("\n## Analysis boundary\n\n");
    for pending in &layout.pending {
        out.push_str(&format!("- {pending}\n"));
    }
    out
}

fn components_json(values: &[KernelComponent]) -> String {
    values
        .iter()
        .map(component_json)
        .collect::<Vec<_>>()
        .join(",")
        .pipe_array()
}
fn component_json(value: &KernelComponent) -> String {
    format!("{{\"id\":{},\"display_name\":{},\"kind\":{},\"status\":{},\"confidence\":{},\"count\":{},\"symbol_identity\":{},\"raw_source\":{},\"deob_source\":{},\"pass_lineage\":{},\"evidence\":{},\"unknown_reason\":{}}}", json_string(&value.id), option_string(value.display_name.as_deref()), json_string(&value.kind), json_string(&value.status), json_string(&value.confidence), value.count.map_or("null".into(), |v| v.to_string()), option_string(value.symbol_identity.as_deref()), source_json(value.raw_source.as_ref()), source_json(value.deob_source.as_ref()), string_array(&value.pass_lineage), string_array(&value.evidence), option_string(value.unknown_reason.as_deref()))
}
fn relationships_json(values: &[LayoutRelationship]) -> String {
    values
        .iter()
        .map(|value| {
            format!(
                "{{\"from\":{},\"relation\":{},\"to\":{},\"status\":{},\"evidence\":{}}}",
                json_string(&value.from),
                json_string(&value.relation),
                json_string(&value.to),
                json_string(&value.status),
                json_string(&value.evidence)
            )
        })
        .collect::<Vec<_>>()
        .join(",")
        .pipe_array()
}
fn wasm_json(values: &[WasmModuleLayout]) -> String {
    values.iter().map(|value| format!("{{\"source_binding\":{},\"byte_len\":{},\"digest\":{},\"imports\":{},\"exports\":{},\"validated\":{},\"source\":{},\"unknown_reason\":{}}}", option_string(value.source_binding.as_deref()), value.byte_len.map_or("null".into(), |v| v.to_string()), option_string(value.digest.as_deref()), string_array(&value.imports), string_array(&value.exports), value.validated, source_json(value.source.as_ref()), option_string(value.unknown_reason.as_deref()))).collect::<Vec<_>>().join(",").pipe_array()
}
fn operations_json(value: Option<&OperationInventory>) -> String {
    let Some(value) = value else {
        return "null".into();
    };
    format!(
        "{{\"table_identity\":{},\"handler_count\":{},\"atomic_handler_count\":{},\"composite_handler_count\":{},\"unknown_handler_count\":{},\"sparse_entry_count\":{},\"dispatch_branch_count\":{},\"dispatch_link\":{}}}",
        json_string(&value.table_identity),
        value.handler_count,
        value.atomic_handler_count,
        value.composite_handler_count,
        value.unknown_handler_count,
        value.sparse_entry_count,
        value
            .dispatch_branch_count
            .map_or("null".into(), |count| count.to_string()),
        json_string(&value.dispatch_link),
    )
}
fn operation_diagnostics_json(value: &OperationDiagnostics) -> String {
    let records = value
        .records
        .iter()
        .map(|record| {
            format!(
                "{{\"handler_index\":{},\"source\":{},\"classification\":{},\"semantic_family\":{},\"recognized_actions\":{},\"unknown_fragments\":{}}}",
                record.handler_index,
                source_json(Some(&record.source)),
                json_string(record.classification.as_str()),
                json_string(record.semantic_family.as_str()),
                record
                    .recognized_actions
                    .iter()
                    .map(|action| json_string(action.as_str()))
                    .collect::<Vec<_>>()
                    .join(",")
                    .pipe_array(),
                string_array(&record.unknown_fragments),
            )
        })
        .collect::<Vec<_>>()
        .join(",")
        .pipe_array();
    format!("{{\"records\":{records}}}")
}
fn source_json(value: Option<&SourceLink>) -> String {
    value.map_or("null".into(), |value| {
        format!(
            "{{\"artifact\":{},\"span\":{}}}",
            json_string(&value.artifact),
            span_json(&value.span)
        )
    })
}
fn span_json(value: &SpanRecord) -> String {
    format!(
        "{{\"start\":{},\"end\":{},\"line_start\":{},\"line_end\":{},\"snippet\":{}}}",
        value.start,
        value.end,
        value.line_start,
        value.line_end,
        json_string(&value.snippet)
    )
}
fn string_array(values: &[String]) -> String {
    values
        .iter()
        .map(|value| json_string(value))
        .collect::<Vec<_>>()
        .join(",")
        .pipe_array()
}
fn option_string(value: Option<&str>) -> String {
    value.map_or("null".into(), json_string)
}
fn json_field(out: &mut String, indent: usize, key: &str, value: &str, comma: bool) {
    out.push_str(&format!(
        "{}{}: {}{}\n",
        "  ".repeat(indent),
        json_string(key),
        value,
        if comma { "," } else { "" }
    ));
}

trait PipeArray {
    fn pipe_array(self) -> String;
}
impl PipeArray for String {
    fn pipe_array(self) -> String {
        format!("[{self}]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::wasm_module::test_support::*;

    fn validator_error(bytes: &[u8]) -> String {
        match wasmparser::Validator::new().validate_all(bytes) {
            Ok(_) => panic!("fixture must be validator-invalid"),
            Err(error) => error.to_string(),
        }
    }

    #[test]
    fn parse_wasm_abi_preserves_abi_for_valid_gc_types() {
        // Old layout validated, then walked only import/export sections.  The
        // detailed GC rejection must not change its ABI projection.
        assert_eq!(
            parse_wasm_abi(&gc_memory_export_module()),
            (vec![], vec!["m".into()], true, None)
        );
        assert_eq!(
            parse_wasm_abi(&gc_import_export_module()),
            (vec!["env::f".into()], vec!["run".into()], true, None)
        );
    }

    #[test]
    fn parse_wasm_abi_preserves_validator_error_precedence() {
        for bytes in [
            invalid_export_module(),
            malformed_type_memory_export_module(),
            malformed_body_module(),
        ] {
            let expected = validator_error(&bytes);
            assert_eq!(
                parse_wasm_abi(&bytes),
                (vec![], vec![], false, Some(expected))
            );
        }
    }

    #[test]
    fn parse_wasm_abi_keeps_compact_import_and_export_kinds() {
        assert_eq!(
            parse_wasm_abi(&compact2_memory_import_module()),
            (vec!["env::m".into()], vec!["m".into()], true, None)
        );
        assert_eq!(
            parse_wasm_abi(&compact1_imports_module()),
            (vec!["env::a".into(), "env::b".into()], vec![], true, None)
        );
    }

    #[test]
    fn parse_wasm_abi_accepts_large_legacy_pipeline_input() {
        let bytes = large_custom_section_module();
        assert!(bytes.len() > 8 * 1024 * 1024);
        assert_eq!(
            parse_wasm_abi(&bytes),
            (vec![], vec!["m".into()], true, None)
        );
    }
}
