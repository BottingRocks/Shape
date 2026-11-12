//! Opt-in, static inventory of the JavaScript/WASM boundary.
//!
//! This is an inventory and proof ledger, not a WebAssembly engine.  It consumes
//! the already-produced [`PipelineReport`], parses its standardized output once
//! with Oxc, and validates every extracted module with `wasmparser`.  It never
//! executes JavaScript or WebAssembly and it never changes the default pipeline
//! output.
//!
//! The dispatch adapter, interpreter lift, and low-level module inspector live
//! beside this boundary in the same folder: [`dispatch`] (the `wasm_dispatch`
//! path), [`lift`] (`wasm_lift`), and [`module`] (`wasm_module`).

pub mod dispatch;
pub(crate) mod lift;
pub(crate) mod module;

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::Span;
use wasmparser::{ElementItems, ElementKind, ExternalKind, Imports, Operator, Payload, TypeRef};

use crate::{
    analysis, decode_base64,
    kernel::wasm_module::{self, ModuleLimits, ValidationStatus},
    transforms::deob::pipeline::{
        ExtractionFact, ExtractionKind, ExtractionStatus, PipelineError, PipelineReport,
    },
    util::{fnv1a64_hex, json_string},
};

pub const SCHEMA_VERSION: &str = "shape-wasm-js-boundary/v1";
pub const ANALYSIS_VERSION: &str = "shape-oxc-deob/wasm-js-boundary-v1";

#[derive(Debug)]
pub enum WasmBoundaryError {
    Pipeline(PipelineError),
    Parse(String),
}

impl fmt::Display for WasmBoundaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pipeline(error) => write!(f, "pipeline failed: {error}"),
            Self::Parse(error) => write!(f, "boundary source parse failed: {error}"),
        }
    }
}

impl std::error::Error for WasmBoundaryError {}

impl From<PipelineError> for WasmBoundaryError {
    fn from(value: PipelineError) -> Self {
        Self::Pipeline(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLink {
    pub artifact: String,
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedFinding {
    pub area: String,
    pub reason: String,
    pub source: Option<SourceLink>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProofCounts {
    pub imports: u32,
    pub imports_linked: u32,
    pub functions: u32,
    pub table_entries: u32,
    pub table_entries_proven: u32,
    pub call_sites: u32,
    pub handler_mappings: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmImport {
    pub index: u32,
    pub function_index: Option<u32>,
    pub namespace: String,
    pub name: String,
    pub kind: String,
    pub signature: String,
    pub js_binding: Option<String>,
    pub js_property: Option<String>,
    pub operation_table: Option<OperationTableLink>,
    pub source_instance_id: Option<u32>,
    pub source: Option<SourceLink>,
    pub status: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationTableLink {
    pub declaration_ordinal: u32,
    pub property: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmExport {
    pub name: String,
    pub kind: String,
    pub index: u32,
    pub function_index: Option<u32>,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmFunction {
    pub function_index: u32,
    pub index_space: String,
    pub signature: String,
    pub import: Option<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableEntry {
    pub table_index: u32,
    pub element_index: u32,
    pub function_index: Option<u32>,
    pub status: String,
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementSegment {
    pub segment_index: u32,
    pub table_index: Option<u32>,
    pub offset: Option<u32>,
    pub entries: Vec<TableEntry>,
    pub status: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmTable {
    pub table_index: u32,
    pub imported: bool,
    pub element_type: String,
    pub initial: u64,
    pub maximum: Option<u64>,
    pub entries: Vec<TableEntry>,
    pub exported_as: Vec<String>,
    pub mutable: bool,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallSite {
    pub caller_function_index: u32,
    pub byte_offset: u64,
    pub kind: String,
    pub target_function_index: Option<u32>,
    pub table_index: Option<u32>,
    pub type_index: Option<u32>,
    pub signature: String,
    pub operand_order: String,
    pub argument_provenance: Option<String>,
    pub source: Option<SourceLink>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandlerMapping {
    pub table_index: u32,
    pub element_index: u32,
    pub function_index: u32,
    pub import_name: Option<String>,
    pub host_wrapper: Option<String>,
    pub original_handler_key: Option<u32>,
    pub status: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmModuleBoundary {
    pub source_binding: Option<String>,
    pub extraction_source: Option<SourceLink>,
    pub deob_source: Option<SourceLink>,
    pub module_hash: String,
    pub byte_len: u64,
    pub instance_id: Option<u32>,
    pub validated: bool,
    pub imports: Vec<WasmImport>,
    pub exports: Vec<WasmExport>,
    pub functions: Vec<WasmFunction>,
    pub tables: Vec<WasmTable>,
    pub element_segments: Vec<ElementSegment>,
    pub call_sites: Vec<CallSite>,
    pub handler_mappings: Vec<HandlerMapping>,
    pub status: String,
    pub unsupported: Vec<UnsupportedFinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmBoundaryReport {
    pub schema_version: String,
    pub analysis_version: String,
    pub pipeline_input_hash: String,
    pub deob_output_hash: String,
    pub wasm_present: bool,
    pub js_only: bool,
    pub modules: Vec<WasmModuleBoundary>,
    pub proof_counts: ProofCounts,
    pub unsupported: Vec<UnsupportedFinding>,
}

/// Analyze a standardized, already-deobfuscated pipeline report.  This is the
/// preferred API for callers that already own a `PipelineReport`: no pipeline
/// pass is rerun and the report's raw bytes remain immutable.
pub fn analyze_report(report: &PipelineReport) -> WasmBoundaryReport {
    let allocator = Allocator::default();
    let program = match analysis::parse_program(&allocator, &report.output) {
        Ok(program) => program,
        Err(reason) => {
            return empty_report(
                report,
                vec![UnsupportedFinding {
                    area: "deob_output".into(),
                    reason,
                    source: None,
                }],
            );
        }
    };
    let (scoping, nodes) = analysis::analyze(&program);
    let declarations = declarations(&nodes);
    let proven_operation_bindings =
        proven_operation_bindings(&nodes, &scoping, &report.proven_operation_tables);
    let context = BoundaryContext {
        proven_operation_bindings: &proven_operation_bindings,
    };
    let mut modules = Vec::new();
    let mut top_level_unsupported = Vec::new();
    for fact in report
        .extraction
        .iter()
        .filter(|fact| fact.kind == ExtractionKind::WasmModule)
    {
        if fact.status != ExtractionStatus::Extracted {
            top_level_unsupported.push(UnsupportedFinding {
                area: "module_extraction".into(),
                reason: fact.evidence.clone(),
                source: Some(extraction_link(fact)),
            });
            continue;
        }
        let Some(bytes) = fact.decoded_bytes.as_deref() else {
            top_level_unsupported.push(UnsupportedFinding {
                area: "module_extraction".into(),
                reason: "extraction is marked extracted but has no immutable decoded bytes".into(),
                source: Some(extraction_link(fact)),
            });
            continue;
        };
        let js = locate_js_instance(&nodes, &scoping, &declarations, fact, bytes);
        let mut module = analyze_module(
            report,
            fact,
            bytes,
            js.as_ref(),
            &report.proven_operation_tables,
            &context,
        );
        module.instance_id = js
            .as_ref()
            .map(|instance| instance.instance_symbol.index() as u32);
        modules.push(module);
    }
    link_cross_module_tables(&mut modules);
    let wasm_present = !modules.is_empty()
        || report
            .extraction
            .iter()
            .any(|fact| fact.kind == ExtractionKind::WasmModule);
    let js_only = !wasm_present;
    if js_only {
        top_level_unsupported.push(UnsupportedFinding {
            area: "wasm_boundary".into(),
            reason:
                "no statically extracted WebAssembly module; interpreter variant is not classified by this boundary report"
                    .into(),
            source: None,
        });
    }
    let mut proof_counts = ProofCounts::default();
    for module in &modules {
        proof_counts.imports += module.imports.len() as u32;
        proof_counts.imports_linked += module
            .imports
            .iter()
            .filter(|item| item.status == "proven")
            .count() as u32;
        proof_counts.functions += module.functions.len() as u32;
        proof_counts.table_entries += module
            .tables
            .iter()
            .filter(|table| !table.imported)
            .map(|table| table.entries.len() as u32)
            .sum::<u32>();
        proof_counts.table_entries_proven += module
            .tables
            .iter()
            .filter(|table| !table.imported)
            .flat_map(|table| table.entries.iter())
            .filter(|entry| entry.status == "proven")
            .count() as u32;
        proof_counts.call_sites += module.call_sites.len() as u32;
        proof_counts.handler_mappings += module
            .handler_mappings
            .iter()
            .filter(|mapping| mapping.status == "proven")
            .count() as u32;
    }
    WasmBoundaryReport {
        schema_version: SCHEMA_VERSION.into(),
        analysis_version: ANALYSIS_VERSION.into(),
        pipeline_input_hash: report.input_hash.clone(),
        deob_output_hash: fnv1a64_hex(report.output.as_bytes()),
        wasm_present,
        js_only,
        modules,
        proof_counts,
        unsupported: top_level_unsupported,
    }
}

/// Run the normal pipeline once and analyze its standardized report.
pub fn analyze_source(source: &str) -> Result<WasmBoundaryReport, WasmBoundaryError> {
    Ok(analyze_report(&crate::deobfuscate_with_report(source)?))
}

fn empty_report(
    report: &PipelineReport,
    unsupported: Vec<UnsupportedFinding>,
) -> WasmBoundaryReport {
    WasmBoundaryReport {
        schema_version: SCHEMA_VERSION.into(),
        analysis_version: ANALYSIS_VERSION.into(),
        pipeline_input_hash: report.input_hash.clone(),
        deob_output_hash: fnv1a64_hex(report.output.as_bytes()),
        wasm_present: false,
        js_only: false,
        modules: Vec::new(),
        proof_counts: ProofCounts::default(),
        unsupported,
    }
}

#[derive(Clone)]
struct Declaration<'a> {
    span: Span,
    init: Option<&'a Expression<'a>>,
}

struct ProvenOperationBinding<'a> {
    symbol: SymbolId,
    declaration_ordinal: u32,
    properties: Vec<JsProperty<'a>>,
    mutated: bool,
}

struct BoundaryContext<'a> {
    proven_operation_bindings: &'a [ProvenOperationBinding<'a>],
}

fn declarations<'a>(nodes: &oxc_semantic::AstNodes<'a>) -> BTreeMap<SymbolId, Declaration<'a>> {
    nodes
        .iter()
        .filter_map(|node| {
            let AstKind::VariableDeclarator(decl) = node.kind() else {
                return None;
            };
            let BindingPattern::BindingIdentifier(id) = &decl.id else {
                return None;
            };
            let symbol = id.symbol_id.get()?;
            Some((
                symbol,
                Declaration {
                    span: decl.span,
                    init: decl.init.as_ref(),
                },
            ))
        })
        .collect()
}

fn symbol_of_binding(binding: &BindingPattern<'_>) -> Option<SymbolId> {
    match binding {
        BindingPattern::BindingIdentifier(id) => id.symbol_id.get(),
        _ => None,
    }
}

#[derive(Clone)]
struct JsProperty<'a> {
    key: String,
    value: &'a Expression<'a>,
    span: Span,
    binding: Option<String>,
}

fn object_properties<'a>(expression: &'a Expression<'a>) -> Option<Vec<JsProperty<'a>>> {
    let Expression::ObjectExpression(object) = expression else {
        return None;
    };
    let mut result = Vec::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        if property.computed || property.kind != PropertyKind::Init || property.shorthand {
            return None;
        }
        let key = match &property.key {
            PropertyKey::StaticIdentifier(id) => id.name.to_string(),
            PropertyKey::StringLiteral(value) => value.value.to_string(),
            PropertyKey::NumericLiteral(value) => value.value.to_string(),
            _ => return None,
        };
        result.push(JsProperty {
            key,
            value: &property.value,
            span: property.span,
            binding: None,
        });
    }
    Some(result)
}

/// Resolve an object literal whose keys are all statically known.  Unlike
/// [`object_properties`] this accepts `{ ["literal"]: value }` keys, but it
/// still refuses every computed key that depends on a runtime value.
fn literal_object_properties<'a>(expression: &'a Expression<'a>) -> Option<Vec<JsProperty<'a>>> {
    let Expression::ObjectExpression(object) = expression else {
        return None;
    };
    let mut result = Vec::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        if property.kind != PropertyKind::Init || property.shorthand {
            return None;
        }
        let key = property.key.static_name()?.into_owned();
        result.push(JsProperty {
            key,
            value: &property.value,
            span: property.span,
            binding: binding_name(&property.value),
        });
    }
    Some(result)
}

fn binding_name(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::Identifier(id) => Some(id.name.to_string()),
        Expression::StaticMemberExpression(member) => {
            let object = binding_name(&member.object)?;
            Some(format!("{object}.{}", member.property.name))
        }
        Expression::ComputedMemberExpression(member) => {
            let object = binding_name(&member.object)?;
            let key = match &member.expression {
                Expression::StringLiteral(value) => value.value.to_string(),
                Expression::TemplateLiteral(template) if template.expressions.is_empty() => {
                    template.single_quasi()?.to_string()
                }
                _ => return None,
            };
            Some(format!("{object}[{}]", json_string(&key)))
        }
        _ => None,
    }
}

fn resolve_string_expression<'a>(
    expression: &'a Expression<'a>,
    declarations: &BTreeMap<SymbolId, Declaration<'a>>,
    scoping: &Scoping,
    seen: &mut BTreeSet<SymbolId>,
) -> Option<String> {
    match expression {
        Expression::StringLiteral(value) => Some(value.value.to_string()),
        Expression::TemplateLiteral(template) if template.expressions.is_empty() => {
            template.single_quasi().map(|value| value.to_string())
        }
        Expression::Identifier(id) => {
            let symbol = analysis::identifier_symbol(id, scoping)?;
            if !seen.insert(symbol) {
                return None;
            }
            resolve_string_expression(
                declarations.get(&symbol)?.init?,
                declarations,
                scoping,
                seen,
            )
        }
        Expression::StaticMemberExpression(member) => resolve_static_member_string(
            &member.object,
            member.property.name.as_str(),
            declarations,
            scoping,
            seen,
        ),
        Expression::ComputedMemberExpression(member) => {
            let key = resolve_string_expression(&member.expression, declarations, scoping, seen)?;
            resolve_static_member_string(&member.object, &key, declarations, scoping, seen)
        }
        _ => None,
    }
}

fn resolve_static_member_string<'a>(
    object: &'a Expression<'a>,
    key: &str,
    declarations: &BTreeMap<SymbolId, Declaration<'a>>,
    scoping: &Scoping,
    seen: &mut BTreeSet<SymbolId>,
) -> Option<String> {
    let properties = resolve_literal_object(object, declarations, scoping, seen)?;
    let property = properties.iter().find(|property| property.key == key)?;
    resolve_string_expression(property.value, declarations, scoping, seen)
}

fn resolve_literal_object<'a>(
    expression: &'a Expression<'a>,
    declarations: &BTreeMap<SymbolId, Declaration<'a>>,
    scoping: &Scoping,
    seen: &mut BTreeSet<SymbolId>,
) -> Option<Vec<JsProperty<'a>>> {
    if let Some(properties) = literal_object_properties(expression) {
        return Some(properties);
    }
    match expression {
        Expression::Identifier(id) => {
            let symbol = analysis::identifier_symbol(id, scoping)?;
            if !seen.insert(symbol) {
                return None;
            }
            resolve_literal_object(
                declarations.get(&symbol)?.init?,
                declarations,
                scoping,
                seen,
            )
        }
        Expression::StaticMemberExpression(member) => {
            let properties = resolve_literal_object(&member.object, declarations, scoping, seen)?;
            let property = properties
                .iter()
                .find(|property| property.key == member.property.name.as_str())?;
            resolve_literal_object(property.value, declarations, scoping, seen)
        }
        Expression::ComputedMemberExpression(member) => {
            let properties = resolve_literal_object(&member.object, declarations, scoping, seen)?;
            let key = resolve_string_expression(&member.expression, declarations, scoping, seen)?;
            let property = properties.iter().find(|property| property.key == key)?;
            resolve_literal_object(property.value, declarations, scoping, seen)
        }
        _ => None,
    }
}

fn adapter_property_key<'a>(
    key: &PropertyKey<'a>,
    declarations: &BTreeMap<SymbolId, Declaration<'a>>,
    scoping: &Scoping,
) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(id) => Some(id.name.to_string()),
        PropertyKey::StringLiteral(value) => Some(value.value.to_string()),
        PropertyKey::NumericLiteral(value) => Some(value.value.to_string()),
        PropertyKey::Identifier(id) => {
            let symbol = analysis::identifier_symbol(id, scoping)?;
            resolve_string_expression(
                declarations.get(&symbol)?.init?,
                declarations,
                scoping,
                &mut BTreeSet::new(),
            )
        }
        PropertyKey::StaticMemberExpression(member) => resolve_static_member_string(
            &member.object,
            member.property.name.as_str(),
            declarations,
            scoping,
            &mut BTreeSet::new(),
        ),
        PropertyKey::ComputedMemberExpression(member) => {
            let object = resolve_literal_object(
                &member.object,
                declarations,
                scoping,
                &mut BTreeSet::new(),
            )?;
            let key = resolve_string_expression(
                &member.expression,
                declarations,
                scoping,
                &mut BTreeSet::new(),
            )?;
            let property = object.into_iter().find(|property| property.key == key)?;
            resolve_string_expression(property.value, declarations, scoping, &mut BTreeSet::new())
        }
        _ => None,
    }
}

/// Object literals produced by the post-rename WASM adapter use computed keys
/// such as `[vmWasmAbi.imports.hostCallback0]`. Resolve those keys for the
/// import-object inventory, but do not treat the resulting object as a copy of
/// any operation table: [`PipelineReport`] carries no such provenance.
fn adapter_object_properties<'a>(
    expression: &'a Expression<'a>,
    declarations: &BTreeMap<SymbolId, Declaration<'a>>,
    scoping: &Scoping,
) -> Option<Vec<JsProperty<'a>>> {
    let Expression::ObjectExpression(object) = expression else {
        return None;
    };
    let mut result = Vec::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        if property.kind != PropertyKind::Init || property.shorthand {
            return None;
        }
        let key = if property.computed {
            adapter_property_key(&property.key, declarations, scoping)?
        } else {
            property.key.static_name()?.into_owned()
        };
        result.push(JsProperty {
            key,
            value: &property.value,
            span: property.span,
            binding: binding_name(&property.value),
        });
    }
    Some(result)
}

fn resolved_object<'a>(
    expression: &'a Expression<'a>,
    declarations: &BTreeMap<SymbolId, Declaration<'a>>,
    scoping: &Scoping,
    seen: &mut BTreeSet<SymbolId>,
) -> Option<Vec<JsProperty<'a>>> {
    if let Some(properties) = adapter_object_properties(expression, declarations, scoping) {
        return Some(properties);
    }
    let Expression::Identifier(id) = expression else {
        return None;
    };
    let symbol = analysis::identifier_symbol(id, scoping)?;
    if !seen.insert(symbol) {
        return None;
    }
    let init = declarations.get(&symbol)?.init?;
    resolved_object(init, declarations, scoping, seen)
}

fn root_binding_symbol<'a>(
    expression: &'a Expression<'a>,
    declarations: &BTreeMap<SymbolId, Declaration<'a>>,
    scoping: &Scoping,
    seen: &mut BTreeSet<SymbolId>,
) -> Option<SymbolId> {
    let Expression::Identifier(id) = expression else {
        return None;
    };
    let symbol = analysis::identifier_symbol(id, scoping)?;
    root_binding_symbol_id(symbol, declarations, scoping, seen)
}

fn root_binding_symbol_id<'a>(
    symbol: SymbolId,
    declarations: &BTreeMap<SymbolId, Declaration<'a>>,
    scoping: &Scoping,
    seen: &mut BTreeSet<SymbolId>,
) -> Option<SymbolId> {
    if !seen.insert(symbol) {
        return None;
    }
    match declarations.get(&symbol).and_then(|decl| decl.init) {
        Some(Expression::Identifier(alias)) => root_binding_symbol_id(
            analysis::identifier_symbol(alias, scoping)?,
            declarations,
            scoping,
            seen,
        ),
        _ => Some(symbol),
    }
}

fn assignment_target_root(target: &AssignmentTarget<'_>, scoping: &Scoping) -> Option<SymbolId> {
    match target {
        AssignmentTarget::AssignmentTargetIdentifier(id) => {
            analysis::identifier_symbol(id, scoping)
        }
        AssignmentTarget::StaticMemberExpression(member) => {
            identifier_expression_symbol(&member.object, scoping)
        }
        AssignmentTarget::ComputedMemberExpression(member) => {
            identifier_expression_symbol(&member.object, scoping)
        }
        _ => None,
    }
}

fn update_target_root(target: &SimpleAssignmentTarget<'_>, scoping: &Scoping) -> Option<SymbolId> {
    match target {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => {
            analysis::identifier_symbol(id, scoping)
        }
        SimpleAssignmentTarget::StaticMemberExpression(member) => {
            identifier_expression_symbol(&member.object, scoping)
        }
        SimpleAssignmentTarget::ComputedMemberExpression(member) => {
            identifier_expression_symbol(&member.object, scoping)
        }
        _ => None,
    }
}

fn identifier_expression_symbol(
    expression: &Expression<'_>,
    scoping: &Scoping,
) -> Option<SymbolId> {
    let Expression::Identifier(id) = expression else {
        return None;
    };
    analysis::identifier_symbol(id, scoping)
}

fn symbol_is_mutated(
    nodes: &oxc_semantic::AstNodes<'_>,
    scoping: &Scoping,
    symbol: SymbolId,
) -> bool {
    nodes.iter().any(|node| match node.kind() {
        AstKind::AssignmentExpression(assignment) => {
            assignment_target_root(&assignment.left, scoping) == Some(symbol)
        }
        AstKind::UpdateExpression(update) => {
            update_target_root(&update.argument, scoping) == Some(symbol)
        }
        _ => false,
    })
}

fn proven_operation_bindings<'a>(
    nodes: &oxc_semantic::AstNodes<'a>,
    scoping: &Scoping,
    proven_tables: &[crate::transforms::deob::pipeline::ProvenOperationTable],
) -> Vec<ProvenOperationBinding<'a>> {
    let mut declarators = nodes
        .iter()
        .filter_map(|node| match node.kind() {
            AstKind::VariableDeclarator(decl) => Some((
                decl.span.start,
                symbol_of_binding(&decl.id),
                decl.init.as_ref(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    declarators.sort_by_key(|(start, _, _)| *start);
    proven_tables
        .iter()
        .filter_map(|table| {
            let (_, symbol, init) = declarators
                .get(table.declaration_ordinal as usize)
                .copied()?;
            let symbol = symbol?;
            let init = init?;
            let properties = object_properties(init)?;
            let function_count = properties
                .iter()
                .filter(|property| {
                    matches!(
                        property.value,
                        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
                    )
                })
                .count() as u32;
            if properties.len() as u32 != table.initializer_elements
                || function_count != table.initializer_functions
            {
                return None;
            }
            Some(ProvenOperationBinding {
                symbol,
                declaration_ordinal: table.declaration_ordinal,
                properties,
                mutated: symbol_is_mutated(nodes, scoping, symbol),
            })
        })
        .collect()
}

fn source_link(artifact: &str, span: Span) -> SourceLink {
    SourceLink {
        artifact: artifact.into(),
        start: span.start,
        end: span.end,
    }
}

fn extraction_link(fact: &ExtractionFact) -> SourceLink {
    source_link("pipeline-extraction", fact.span)
}

struct JsInstance<'a> {
    module_source: SourceLink,
    instance_symbol: SymbolId,
    namespaces: BTreeMap<String, Vec<JsProperty<'a>>>,
    namespace_bindings: BTreeMap<String, String>,
    namespace_symbols: BTreeMap<String, SymbolId>,
    namespace_exports: BTreeMap<String, SymbolId>,
}

fn locate_js_instance<'a>(
    nodes: &oxc_semantic::AstNodes<'a>,
    scoping: &Scoping,
    declarations: &BTreeMap<SymbolId, Declaration<'a>>,
    fact: &ExtractionFact,
    bytes: &[u8],
) -> Option<JsInstance<'a>> {
    let byte_symbols = nodes
        .iter()
        .filter_map(|node| {
            let AstKind::VariableDeclarator(decl) = node.kind() else {
                return None;
            };
            let BindingPattern::BindingIdentifier(id) = &decl.id else {
                return None;
            };
            if id.name != fact.binding.as_deref()?
                || symbol_is_mutated(nodes, scoping, id.symbol_id.get()?)
            {
                return None;
            }
            decoded_module_expression(Some(decl.init.as_ref()?))
                .filter(|candidate| candidate == bytes)
                .map(|_| id.symbol_id.get().unwrap())
        })
        .collect::<BTreeSet<_>>();
    if byte_symbols.is_empty() {
        return None;
    }
    let mut module_symbols = BTreeSet::new();
    for node in nodes.iter() {
        let AstKind::VariableDeclarator(decl) = node.kind() else {
            continue;
        };
        let Some(Expression::NewExpression(expression)) = decl.init.as_ref() else {
            continue;
        };
        if !is_webassembly_member(&expression.callee, "Module", scoping) {
            continue;
        }
        let Some(argument) = expression
            .arguments
            .first()
            .and_then(Argument::as_expression)
        else {
            continue;
        };
        let matches = match argument {
            Expression::Identifier(id) => analysis::identifier_symbol(id, scoping)
                .is_some_and(|symbol| byte_symbols.contains(&symbol)),
            _ => decoded_module_expression(Some(argument)).is_some_and(|value| value == bytes),
        };
        if !matches {
            continue;
        }
        if let Some(symbol) = symbol_of_binding(&decl.id) {
            if !symbol_is_mutated(nodes, scoping, symbol) {
                module_symbols.insert(symbol);
            }
        }
    }
    for node in nodes.iter() {
        let AstKind::VariableDeclarator(decl) = node.kind() else {
            continue;
        };
        let Some(Expression::NewExpression(expression)) = decl.init.as_ref() else {
            continue;
        };
        if !is_webassembly_member(&expression.callee, "Instance", scoping) {
            continue;
        }
        let Some(Argument::Identifier(module_id)) = expression.arguments.first() else {
            continue;
        };
        let Some(module_symbol) = analysis::identifier_symbol(module_id, scoping) else {
            continue;
        };
        if !module_symbols.contains(&module_symbol) {
            continue;
        }
        let Some(imports_expression) = expression
            .arguments
            .get(1)
            .and_then(Argument::as_expression)
        else {
            continue;
        };
        let Some(properties) = resolved_object(
            imports_expression,
            declarations,
            scoping,
            &mut BTreeSet::new(),
        ) else {
            continue;
        };
        let Some(instance_symbol) = symbol_of_binding(&decl.id) else {
            continue;
        };
        if symbol_is_mutated(nodes, scoping, instance_symbol)
            || symbol_is_mutated(nodes, scoping, module_symbol)
        {
            continue;
        }
        let mut namespace_bindings = BTreeMap::new();
        let mut namespace_symbols = BTreeMap::new();
        let mut namespace_exports = BTreeMap::new();
        let mut namespaces = BTreeMap::new();
        for property in properties {
            if let Expression::Identifier(id) = property.value {
                namespace_bindings.insert(property.key.clone(), id.name.to_string());
                if let Some(symbol) =
                    root_binding_symbol(property.value, declarations, scoping, &mut BTreeSet::new())
                {
                    namespace_symbols.insert(property.key.clone(), symbol);
                }
            }
            if let Some(export_instance) = instance_exports_symbol(property.value, scoping) {
                if let Expression::StaticMemberExpression(member) = property.value {
                    if let Expression::Identifier(id) = &member.object {
                        namespace_bindings.insert(property.key.clone(), id.name.to_string());
                    }
                }
                namespace_exports.insert(property.key.clone(), export_instance);
                continue;
            }
            let root =
                root_binding_symbol(property.value, declarations, scoping, &mut BTreeSet::new());
            if root.is_some_and(|symbol| symbol_is_mutated(nodes, scoping, symbol)) {
                continue;
            }
            let Some(object) =
                resolved_object(property.value, declarations, scoping, &mut BTreeSet::new())
            else {
                continue;
            };
            namespaces.insert(property.key, object);
        }
        let module_source = declarations
            .get(&module_symbol)
            .map(|decl| source_link("deob-output", decl.span))
            .unwrap_or_else(|| source_link("deob-output", expression.span));
        return Some(JsInstance {
            module_source,
            instance_symbol,
            namespaces,
            namespace_bindings,
            namespace_symbols,
            namespace_exports,
        });
    }
    None
}

fn decoded_module_expression(expression: Option<&Expression<'_>>) -> Option<Vec<u8>> {
    let expression = expression?;
    match expression {
        Expression::StringLiteral(value) => decode_base64(value.value.as_str()).ok(),
        Expression::CallExpression(call) if call.arguments.len() == 1 => {
            let Expression::Identifier(callee) = &call.callee else {
                return None;
            };
            if !callee.name.to_ascii_lowercase().contains("base64") {
                return None;
            }
            let value = call.arguments.first()?.as_expression()?;
            let Expression::StringLiteral(value) = value else {
                return None;
            };
            decode_base64(value.value.as_str()).ok()
        }
        _ => None,
    }
}

fn instance_exports_symbol(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    let Expression::StaticMemberExpression(member) = expression else {
        return None;
    };
    if member.property.name != "exports" {
        return None;
    }
    identifier_expression_symbol(&member.object, scoping)
}

fn is_webassembly_member(expression: &Expression<'_>, property: &str, scoping: &Scoping) -> bool {
    let Expression::StaticMemberExpression(member) = expression else {
        return false;
    };
    let Expression::Identifier(object) = &member.object else {
        return false;
    };
    object.name == "WebAssembly"
        && member.property.name == property
        && analysis::identifier_symbol(object, scoping).is_none()
}

fn analyze_module(
    report: &PipelineReport,
    fact: &ExtractionFact,
    bytes: &[u8],
    js: Option<&JsInstance<'_>>,
    proven_tables: &[crate::transforms::deob::pipeline::ProvenOperationTable],
    context: &BoundaryContext<'_>,
) -> WasmModuleBoundary {
    let source = Some(extraction_link(fact));
    let mut parsed = match parse_detailed(bytes) {
        Ok(parsed) => parsed,
        Err(error) => {
            return WasmModuleBoundary {
                source_binding: fact.binding.clone(),
                extraction_source: source,
                deob_source: js.map(|value| value.module_source.clone()),
                module_hash: fnv1a64_hex(bytes),
                byte_len: bytes.len() as u64,
                instance_id: js.map(|value| value.instance_symbol.index() as u32),
                validated: false,
                imports: Vec::new(),
                exports: Vec::new(),
                functions: Vec::new(),
                tables: Vec::new(),
                element_segments: Vec::new(),
                call_sites: Vec::new(),
                handler_mappings: Vec::new(),
                status: "unsupported".into(),
                unsupported: vec![UnsupportedFinding {
                    area: "wasm_validation".into(),
                    reason: error,
                    source: Some(extraction_link(fact)),
                }],
            }
        }
    };
    let mut imports = parsed.imports;
    for call in &mut parsed.call_sites {
        call.source = source.clone();
    }
    let mut unsupported = parsed.unsupported;
    if let Some(js) = js {
        link_imports(&mut imports, js, report, proven_tables, context);
    } else {
        for import in &mut imports {
            import.status = "unknown".into();
            import.evidence.push(
                "validated WASM ABI has no proven JS WebAssembly.Instance import object".into(),
            );
        }
        if !imports.is_empty() {
            unsupported.push(UnsupportedFinding {
                area: "js_import_object".into(),
                reason: "instance/import-object relation was not proven from Oxc dataflow".into(),
                source: Some(source_link("deob-output", Span::new(0, 0))),
            });
        }
    }
    let mut handler_mappings = Vec::new();
    for table in &parsed.tables {
        for entry in &table.entries {
            if let Some(function_index) = entry.function_index {
                let import = parsed
                    .functions
                    .iter()
                    .find(|function| function.function_index == function_index)
                    .and_then(|function| function.import.clone());
                let import_record = imports
                    .iter()
                    .find(|item| item.function_index == Some(function_index));
                let mut mapping = HandlerMapping {
                    table_index: entry.table_index,
                    element_index: entry.element_index,
                    function_index,
                    import_name: import.as_ref().map(|(_, name)| name.clone()),
                    host_wrapper: None,
                    original_handler_key: None,
                    status: "unknown".into(),
                    evidence: Vec::new(),
                };
                if let Some(import_record) = import_record {
                    if import_record.operation_table.is_some()
                        && import_record.name.parse::<u32>().ok().is_some()
                        && import_record.status == "proven"
                    {
                        mapping.original_handler_key = import_record.name.parse().ok();
                        mapping.status = "proven".into();
                        mapping.evidence.push("element segment resolves to an imported numeric property of the selected proven operation table; table index is kept separate".into());
                    } else if import_record.status == "proven" {
                        mapping.host_wrapper = import_record.js_binding.clone();
                        mapping.evidence.push("element segment resolves to a direct host import; it is not assumed to be a vm operation".into());
                    }
                }
                handler_mappings.push(mapping);
            }
        }
    }
    for segment in &parsed.element_segments {
        if segment.entries.iter().any(|entry| entry.status != "proven")
            && segment.status != "proven"
        {
            unsupported.push(UnsupportedFinding {
                area: "element_segment".into(),
                reason:
                    "element segment contains null, dynamic, passive, or unresolved table entries"
                        .into(),
                source: source.clone(),
            });
        }
    }
    for table in &parsed.tables {
        if table.mutable {
            unsupported.push(UnsupportedFinding { area: format!("table[{}]", table.table_index), reason: "table writes/growth make handler contents mutable; no runtime value was guessed".into(), source: source.clone() });
        }
    }
    let status = if unsupported.is_empty() && imports.iter().all(|item| item.status == "proven") {
        "proven"
    } else {
        "partial"
    };
    WasmModuleBoundary {
        source_binding: fact.binding.clone(),
        extraction_source: source,
        deob_source: js.map(|value| value.module_source.clone()),
        module_hash: fnv1a64_hex(bytes),
        byte_len: bytes.len() as u64,
        instance_id: js.map(|value| value.instance_symbol.index() as u32),
        validated: true,
        imports,
        exports: parsed.exports,
        functions: parsed.functions,
        tables: parsed.tables,
        element_segments: parsed.element_segments,
        call_sites: parsed.call_sites,
        handler_mappings,
        status: status.into(),
        unsupported,
    }
}

fn link_imports(
    imports: &mut [WasmImport],
    js: &JsInstance<'_>,
    _report: &PipelineReport,
    _proven_tables: &[crate::transforms::deob::pipeline::ProvenOperationTable],
    context: &BoundaryContext<'_>,
) {
    for import in imports {
        if import.kind == "table" {
            if let Some(instance_symbol) = js.namespace_exports.get(&import.namespace) {
                import.source_instance_id = Some(instance_symbol.index() as u32);
                import.status = "unknown".into();
                import.evidence.push(format!(
                    "imported table is supplied by the validated WebAssembly.Instance `{}` exports",
                    js.namespace_bindings
                        .get(&import.namespace)
                        .map(String::as_str)
                        .unwrap_or("unknown")
                ));
            } else {
                import
                    .evidence
                    .push("imported table has no validated instance-export source".into());
            }
            continue;
        }
        let Some(namespace) = js.namespaces.get(&import.namespace) else {
            import
                .evidence
                .push("namespace is not present in the proven instance import object".into());
            continue;
        };
        let Some(property) = namespace
            .iter()
            .find(|property| property.key == import.name)
        else {
            import
                .evidence
                .push("import field is not present in the proven namespace object".into());
            continue;
        };
        import.js_property = Some(import.name.clone());
        import.source = Some(source_link("deob-output", property.span));
        import.js_binding = property.binding.clone();
        if import.js_binding.is_none() {
            if let Expression::Identifier(id) = property.value {
                import.js_binding = Some(id.name.to_string());
            }
        }
        if import.js_binding.is_none() {
            import.js_binding = js.namespace_bindings.get(&import.namespace).cloned();
        }
        import.status = "proven".into();
        import.evidence.push(format!(
            "validated namespace/name binding {}::{}",
            import.namespace, import.name
        ));
        let mut operation_linked = false;
        if let Some(namespace_symbol) = js.namespace_symbols.get(&import.namespace) {
            if let Some(table) = context.proven_operation_bindings.iter().find(|table| {
                !table.mutated
                    && table.symbol == *namespace_symbol
                    && table.properties.iter().any(|candidate| {
                        candidate.key == import.name
                            && matches!(
                                candidate.value,
                                Expression::FunctionExpression(_)
                                    | Expression::ArrowFunctionExpression(_)
                            )
                    })
            }) {
                import.operation_table = Some(OperationTableLink {
                    declaration_ordinal: table.declaration_ordinal,
                    property: import.name.clone(),
                });
                import.evidence.push(
                    "numeric field is linked to a property of the selected proven operation-table binding".into(),
                );
                operation_linked = true;
            }
        }
        if !operation_linked && !js.namespace_symbols.contains_key(&import.namespace) {
            import.evidence.push(
                "namespace value is not the exact proven operation-table binding; copied adapter handler provenance is unavailable".into(),
            );
        }
    }
}

fn link_cross_module_tables(modules: &mut [WasmModuleBoundary]) {
    for consumer_index in 0..modules.len() {
        let imports = modules[consumer_index]
            .imports
            .iter()
            .enumerate()
            .filter_map(|(index, import)| {
                (import.kind == "table").then_some((
                    index,
                    import.name.clone(),
                    import.source_instance_id,
                ))
            })
            .collect::<Vec<_>>();
        for (import_index, export_name, source_instance_id) in imports {
            let Some(source_instance_id) = source_instance_id else {
                continue;
            };
            let Some(producer_index) = modules.iter().enumerate().position(|(index, module)| {
                index != consumer_index && module.instance_id == Some(source_instance_id)
            }) else {
                modules[consumer_index].imports[import_index].evidence.push(
                    "instance export source did not resolve to a validated producer module".into(),
                );
                continue;
            };
            let Some(producer_table_index) = modules[producer_index]
                .exports
                .iter()
                .find(|export| export.kind == "table" && export.name == export_name)
                .map(|export| export.index)
            else {
                modules[consumer_index].imports[import_index]
                    .evidence
                    .push("instance export name is not an exported producer table".into());
                continue;
            };
            let Some(producer_table) = modules[producer_index]
                .tables
                .iter()
                .find(|table| table.table_index == producer_table_index && !table.mutable)
                .cloned()
            else {
                modules[consumer_index].imports[import_index].evidence.push(
                    "producer table is missing or mutable; imported contents remain unknown".into(),
                );
                continue;
            };
            if producer_table.status == "unsupported"
                || producer_table
                    .entries
                    .iter()
                    .any(|entry| entry.status != "proven")
            {
                modules[consumer_index].imports[import_index]
                    .evidence
                    .push("producer table contains unknown or dynamic entries; imported contents remain unknown".into());
                continue;
            }
            let imported_table_index = modules[consumer_index].imports[import_index].index;
            let Some(consumer_table) = modules[consumer_index]
                .tables
                .iter_mut()
                .find(|table| table.table_index == imported_table_index)
            else {
                continue;
            };
            consumer_table.entries = producer_table
                .entries
                .iter()
                .map(|entry| TableEntry {
                    table_index: consumer_table.table_index,
                    element_index: entry.element_index,
                    function_index: entry.function_index,
                    status: "proven".into(),
                    evidence: format!(
                        "resolved from producer instance export {} table {} active element; producer function index {}",
                        export_name,
                        producer_table_index,
                        entry
                            .function_index
                            .map_or_else(|| "null".into(), |value| value.to_string())
                    ),
                })
                .collect();
            consumer_table.status = "proven".into();
            modules[consumer_index].imports[import_index].status = "proven".into();
            modules[consumer_index].imports[import_index]
                .evidence
                .push(format!(
                    "resolved by instance symbol {} and exported table {} from module {}",
                    source_instance_id, export_name, producer_index
                ));
            modules[consumer_index].unsupported.retain(|finding| {
                !(finding.area == "imported_table"
                    && finding.reason.contains(&format!("::{}", export_name)))
            });
        }
        let module = &mut modules[consumer_index];
        if module.unsupported.is_empty()
            && module.imports.iter().all(|item| item.status == "proven")
        {
            module.status = "proven".into();
        } else {
            module.status = "partial".into();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedWasm {
    pub(crate) imports: Vec<WasmImport>,
    pub(crate) exports: Vec<WasmExport>,
    pub(crate) functions: Vec<WasmFunction>,
    pub(crate) tables: Vec<WasmTable>,
    pub(crate) element_segments: Vec<ElementSegment>,
    pub(crate) call_sites: Vec<CallSite>,
    pub(crate) unsupported: Vec<UnsupportedFinding>,
}

struct FunctionImport {
    function_index: u32,
    type_index: u32,
}

type ImportDef = (String, String, String, Option<u32>, u32, String);

struct ImportContext<'a> {
    imports: &'a mut Vec<ImportDef>,
    function_imports: &'a mut Vec<FunctionImport>,
    imported_functions: &'a mut u32,
    external_indices: &'a mut [u32; 5],
    tables: &'a mut Vec<WasmTable>,
}

impl ImportContext<'_> {
    fn add(&mut self, namespace: &str, name: &str, ty: TypeRef) {
        let signature_hint = format!("{ty:?}");
        let (kind, function_index, type_index) = match ty {
            TypeRef::Func(index) | TypeRef::FuncExact(index) => {
                let function_index = *self.imported_functions;
                *self.imported_functions += 1;
                self.function_imports.push(FunctionImport {
                    function_index,
                    type_index: index,
                });
                ("function", Some(function_index), index)
            }
            TypeRef::Table(table) => {
                let table_index = self.external_indices[1];
                self.tables.push(WasmTable {
                    table_index,
                    imported: true,
                    element_type: table.element_type.to_string(),
                    initial: table.initial,
                    maximum: table.maximum,
                    entries: Vec::new(),
                    exported_as: Vec::new(),
                    mutable: false,
                    status: "unknown".into(),
                });
                ("table", None, 0)
            }
            TypeRef::Memory(_) => ("memory", None, 0),
            TypeRef::Global(_) => ("global", None, 0),
            TypeRef::Tag(tag) => ("tag", None, tag.func_type_idx),
        };
        let kind_index = external_kind_index(kind);
        self.external_indices[kind_index] += 1;
        self.imports.push((
            namespace.into(),
            name.into(),
            kind.into(),
            function_index,
            type_index,
            signature_hint,
        ));
    }
}

const TABLE_ENTRY_MATERIALIZATION_LIMIT: usize = 100_000;

/// Detailed boundary projection built from the one shared payload stream in
/// `wasm_module::inspect_module_detailed`.  A failure here is reported as the
/// boundary's historical unsupported reason and never affects simple callers.
#[derive(Default)]
struct RichWasmParser<'a> {
    types: Vec<wasmparser::FuncType>,
    import_defs: Vec<ImportDef>,
    function_imports: Vec<FunctionImport>,
    defined_types: Vec<u32>,
    tables: Vec<WasmTable>,
    elements: Vec<ElementSegment>,
    exports: Vec<WasmExport>,
    call_sites: Vec<CallSite>,
    bodies: Vec<wasmparser::FunctionBody<'a>>,
    imported_functions: u32,
    external_indices: [u32; 5],
}

impl<'a> RichWasmParser<'a> {
    fn observe(&mut self, payload: &Payload<'a>) -> Result<(), String> {
        match payload {
            Payload::TypeSection(reader) => {
                self.types.extend(
                    reader
                        .clone()
                        .into_iter_err_on_gc_types()
                        .map(|item| item.map_err(|error| error.to_string()))
                        .collect::<Result<Vec<_>, _>>()?,
                );
            }
            Payload::ImportSection(reader) => {
                let mut context = ImportContext {
                    imports: &mut self.import_defs,
                    function_imports: &mut self.function_imports,
                    imported_functions: &mut self.imported_functions,
                    external_indices: &mut self.external_indices,
                    tables: &mut self.tables,
                };
                for group in reader.clone() {
                    match group.map_err(|error| error.to_string())? {
                        Imports::Single(_, import) => {
                            context.add(import.module, import.name, import.ty)
                        }
                        Imports::Compact1 { module, items } => {
                            for item in items {
                                let item = item.map_err(|error| error.to_string())?;
                                context.add(module, item.name, item.ty);
                            }
                        }
                        Imports::Compact2 { module, ty, names } => {
                            for name in names {
                                context.add(module, name.map_err(|error| error.to_string())?, ty);
                            }
                        }
                    }
                }
            }
            Payload::FunctionSection(reader) => {
                for item in reader.clone() {
                    self.defined_types
                        .push(item.map_err(|error| error.to_string())?);
                }
            }
            Payload::TableSection(reader) => {
                for item in reader.clone() {
                    let item = item.map_err(|error| error.to_string())?;
                    let index = self.tables.len() as u32;
                    self.tables.push(WasmTable {
                        table_index: index,
                        imported: false,
                        element_type: item.ty.element_type.to_string(),
                        initial: item.ty.initial,
                        maximum: item.ty.maximum,
                        entries: Vec::new(),
                        exported_as: Vec::new(),
                        mutable: false,
                        status: "proven".into(),
                    });
                }
            }
            Payload::ExportSection(reader) => {
                for item in reader.clone() {
                    let item = item.map_err(|error| error.to_string())?;
                    let function_index =
                        matches!(item.kind, ExternalKind::Func).then_some(item.index);
                    let signature = function_index.and_then(|index| {
                        function_signature(
                            index,
                            &self.function_imports,
                            &self.defined_types,
                            &self.types,
                        )
                    });
                    self.exports.push(WasmExport {
                        name: item.name.to_owned(),
                        kind: external_kind_name(item.kind).into(),
                        index: item.index,
                        function_index,
                        signature,
                    });
                    if item.kind == ExternalKind::Table {
                        if let Some(table) = self
                            .tables
                            .iter_mut()
                            .find(|table| table.table_index == item.index)
                        {
                            table.exported_as.push(item.name.to_owned());
                        }
                    }
                }
            }
            Payload::ElementSection(reader) => {
                for (segment_index, item) in reader.clone().into_iter().enumerate() {
                    let item = item.map_err(|error| error.to_string())?;
                    let (table_index, offset) = match item.kind {
                        ElementKind::Active {
                            table_index,
                            offset_expr,
                        } => (table_index.or(Some(0)), const_i32(&offset_expr)),
                        ElementKind::Passive | ElementKind::Declared => (None, None),
                    };
                    let mut entries = Vec::new();
                    match item.items {
                        ElementItems::Functions(reader) => {
                            for (element_index, function) in reader.into_iter().enumerate() {
                                let function_index = function.map_err(|error| error.to_string())?;
                                entries.push(TableEntry {
                                    table_index: table_index.unwrap_or(0),
                                    element_index: element_index as u32,
                                    function_index: Some(function_index),
                                    status: "proven".into(),
                                    evidence: "validated ref.func/function-index element".into(),
                                });
                            }
                        }
                        ElementItems::Expressions(_, reader) => {
                            for (element_index, expression) in reader.into_iter().enumerate() {
                                let expression = expression.map_err(|error| error.to_string())?;
                                let (function_index, status, evidence) =
                                    const_ref_func(&expression);
                                entries.push(TableEntry {
                                    table_index: table_index.unwrap_or(0),
                                    element_index: element_index as u32,
                                    function_index,
                                    status,
                                    evidence,
                                });
                            }
                        }
                    };
                    let status = if table_index.is_some() && offset.is_some() {
                        "proven"
                    } else {
                        "unknown"
                    };
                    self.elements.push(ElementSegment {
                        segment_index: segment_index as u32,
                        table_index,
                        offset,
                        entries,
                        status: status.into(),
                        evidence: vec![
                            "element segment parsed without executing initialization".into()
                        ],
                    });
                }
            }
            Payload::CodeSectionEntry(body) => self.bodies.push(body.clone()),
            _ => {}
        }
        Ok(())
    }

    fn finish(self, max_entries: usize) -> Result<ParsedWasm, String> {
        let RichWasmParser {
            types,
            import_defs,
            function_imports: _,
            defined_types,
            mut tables,
            elements,
            exports,
            mut call_sites,
            bodies,
            imported_functions,
            external_indices: _,
        } = self;
        let mut functions = Vec::new();
        for (namespace, name, kind, function_index, type_index, _) in &import_defs {
            let function_index = if kind == "function" {
                function_index.expect("function import metadata")
            } else {
                continue;
            };
            functions.push(WasmFunction {
                function_index,
                index_space: "imported".into(),
                signature: signature_for(*type_index, &types),
                import: Some((namespace.clone(), name.clone())),
            });
        }
        for (ordinal, type_index) in defined_types.iter().enumerate() {
            let function_index = imported_functions + ordinal as u32;
            functions.push(WasmFunction {
                function_index,
                index_space: "defined".into(),
                signature: signature_for(*type_index, &types),
                import: None,
            });
        }
        for (index, table) in tables.iter_mut().enumerate() {
            let _ = index;
            if table.imported {
                continue;
            }
            if table.initial <= max_entries as u64 {
                table.entries = (0..table.initial as u32)
                    .map(|element_index| TableEntry {
                        table_index: table.table_index,
                        element_index,
                        function_index: None,
                        status: "proven".into(),
                        evidence: "defined table default-initialized to ref.null".into(),
                    })
                    .collect();
            } else {
                table.status = "unsupported".into();
            }
        }
        for segment in &elements {
            if let (Some(table_index), Some(offset)) = (segment.table_index, segment.offset) {
                if let Some(table) = tables
                    .iter_mut()
                    .find(|table| table.table_index == table_index)
                {
                    for entry in &segment.entries {
                        let slot = offset.saturating_add(entry.element_index);
                        if let Some(existing) = table
                            .entries
                            .iter_mut()
                            .find(|existing| existing.element_index == slot)
                        {
                            *existing = TableEntry {
                                table_index,
                                element_index: slot,
                                function_index: entry.function_index,
                                status: entry.status.clone(),
                                evidence: entry.evidence.clone(),
                            };
                        } else {
                            table.status = "partial".into();
                        }
                    }
                }
            }
        }
        for (body_ordinal, body) in bodies.iter().enumerate() {
            let caller = imported_functions + body_ordinal as u32;
            let reader = body
                .get_operators_reader()
                .map_err(|error| error.to_string())?;
            let mut previous_const = None;
            for item in reader.into_iter_with_offsets() {
                let (operator, offset) = item.map_err(|error| error.to_string())?;
                match operator {
                    Operator::Call { function_index } => {
                        call_sites.push(CallSite {
                            caller_function_index: caller,
                            byte_offset: offset,
                            kind: "call".into(),
                            target_function_index: Some(function_index),
                            table_index: None,
                            type_index: None,
                            signature: functions
                                .iter()
                                .find(|function| function.function_index == function_index)
                                .map(|function| function.signature.clone())
                                .unwrap_or_else(|| "unknown".into()),
                            operand_order:
                                "callee function index; arguments follow the validated target signature"
                                    .into(),
                            argument_provenance: None,
                            source: None,
                        });
                        previous_const = None;
                    }
                    Operator::ReturnCall { function_index } => {
                        call_sites.push(CallSite {
                            caller_function_index: caller,
                            byte_offset: offset,
                            kind: "return_call".into(),
                            target_function_index: Some(function_index),
                            table_index: None,
                            type_index: None,
                            signature: functions
                                .iter()
                                .find(|function| function.function_index == function_index)
                                .map(|function| function.signature.clone())
                                .unwrap_or_else(|| "unknown".into()),
                            operand_order:
                                "callee function index; arguments follow the validated target signature"
                                    .into(),
                            argument_provenance: None,
                            source: None,
                        });
                        previous_const = None;
                    }
                    Operator::CallIndirect {
                        type_index,
                        table_index,
                    } => {
                        call_sites.push(CallSite {
                            caller_function_index: caller,
                            byte_offset: offset,
                            kind: "call_indirect".into(),
                            target_function_index: None,
                            table_index: Some(table_index),
                            type_index: Some(type_index),
                            signature: signature_for(type_index, &types),
                            operand_order:
                                "call arguments then table index, in WebAssembly stack order".into(),
                            argument_provenance: previous_const.map(|value| {
                                format!("bounded immediate table-index candidate: i32.const {value}")
                            }),
                            source: None,
                        });
                        previous_const = None;
                    }
                    Operator::ReturnCallIndirect {
                        type_index,
                        table_index,
                    } => {
                        call_sites.push(CallSite {
                            caller_function_index: caller,
                            byte_offset: offset,
                            kind: "return_call_indirect".into(),
                            target_function_index: None,
                            table_index: Some(table_index),
                            type_index: Some(type_index),
                            signature: signature_for(type_index, &types),
                            operand_order:
                                "call arguments then table index, in WebAssembly stack order".into(),
                            argument_provenance: previous_const.map(|value| {
                                format!("bounded immediate table-index candidate: i32.const {value}")
                            }),
                            source: None,
                        });
                        previous_const = None;
                    }
                    Operator::I32Const { value } => previous_const = Some(value),
                    Operator::TableSet { table }
                    | Operator::TableFill { table }
                    | Operator::TableGrow { table } => {
                        if let Some(found) =
                            tables.iter_mut().find(|item| item.table_index == table)
                        {
                            found.mutable = true;
                        }
                        previous_const = None;
                    }
                    Operator::TableCopy {
                        dst_table,
                        src_table,
                    } => {
                        for table_index in [dst_table, src_table] {
                            if let Some(found) = tables
                                .iter_mut()
                                .find(|item| item.table_index == table_index)
                            {
                                found.mutable = true;
                            }
                        }
                        previous_const = None;
                    }
                    _ => previous_const = None,
                }
            }
        }
        let mut unsupported = Vec::new();
        for table in &tables {
            if table.status == "unsupported" {
                unsupported.push(UnsupportedFinding {
                    area: format!("table[{}]", table.table_index),
                    reason: "table is too large to materialize null entries within the bounded static inventory".into(),
                    source: None,
                });
            }
        }
        for import in &import_defs {
            if import.2 == "table" {
                unsupported.push(UnsupportedFinding {
                    area: "imported_table".into(),
                    reason: format!(
                        "imported table {}::{} contents are supplied by the host and were not guessed",
                        import.0, import.1
                    ),
                    source: None,
                });
            }
        }
        let mut kind_indices = [0u32; 5];
        let imports = import_defs
            .into_iter()
            .map(
                |(namespace, name, kind, function_index, type_index, signature_hint)| {
                    let kind_slot = external_kind_index(&kind);
                    let index = kind_indices[kind_slot];
                    kind_indices[kind_slot] += 1;
                    let signature = if kind == "function" {
                        signature_for(type_index, &types)
                    } else {
                        signature_hint
                    };
                    WasmImport {
                        index,
                        function_index,
                        namespace,
                        name,
                        kind,
                        signature,
                        js_binding: None,
                        js_property: None,
                        operation_table: None,
                        source_instance_id: None,
                        source: None,
                        status: "unknown".into(),
                        evidence: Vec::new(),
                    }
                },
            )
            .collect();
        Ok(ParsedWasm {
            imports,
            exports,
            functions,
            tables,
            element_segments: elements,
            call_sites,
            unsupported,
        })
    }
}

fn parse_detailed(bytes: &[u8]) -> Result<ParsedWasm, String> {
    // The old boundary parser materialized at most 100,000 entries per
    // defined table.  That historical threshold is deliberately not a global
    // module entry budget and does not reject any module.
    let limits = ModuleLimits {
        max_entries: TABLE_ENTRY_MATERIALIZATION_LIMIT,
        ..ModuleLimits::for_input(bytes)
    };
    let mut detailed = RichWasmParser::default();
    let summary =
        wasm_module::inspect_module_detailed(bytes, limits, |payload| detailed.observe(payload))
            .map_err(|error| error.to_string())?;
    if let ValidationStatus::Invalid(error) = summary.validation {
        return Err(error);
    }
    if let Some(error) = summary.detailed_error {
        return Err(error);
    }
    detailed.finish(limits.max_entries)
}

fn external_kind_index(kind: &str) -> usize {
    match kind {
        "function" => 0,
        "table" => 1,
        "memory" => 2,
        "global" => 3,
        _ => 4,
    }
}

fn function_signature(
    index: u32,
    imports: &[FunctionImport],
    defined: &[u32],
    types: &[wasmparser::FuncType],
) -> Option<String> {
    imports
        .iter()
        .find(|item| item.function_index == index)
        .map(|item| signature_for(item.type_index, types))
        .or_else(|| {
            defined
                .get(index.checked_sub(imports.len() as u32)? as usize)
                .map(|type_index| signature_for(*type_index, types))
        })
}
fn signature_for(index: u32, types: &[wasmparser::FuncType]) -> String {
    types
        .get(index as usize)
        .map(ToString::to_string)
        .unwrap_or_else(|| format!("type[{index}]"))
}
fn external_kind_name(kind: ExternalKind) -> &'static str {
    match kind {
        ExternalKind::Func => "function",
        ExternalKind::Table => "table",
        ExternalKind::Memory => "memory",
        ExternalKind::Global => "global",
        ExternalKind::Tag => "tag",
        ExternalKind::FuncExact => "function_exact",
    }
}

fn const_i32(expression: &wasmparser::ConstExpr<'_>) -> Option<u32> {
    let reader = expression.get_operators_reader();
    let mut value = None;
    for item in reader {
        match item.ok()? {
            Operator::I32Const { value: number } => value = u32::try_from(number).ok(),
            Operator::End => break,
            _ => return None,
        }
    }
    value
}
fn const_ref_func(expression: &wasmparser::ConstExpr<'_>) -> (Option<u32>, String, String) {
    let reader = expression.get_operators_reader();
    for item in reader {
        match item {
            Ok(Operator::RefFunc { function_index }) => {
                return (
                    Some(function_index),
                    "proven".into(),
                    "validated ref.func element expression".into(),
                )
            }
            Ok(Operator::RefNull { .. }) => {
                return (
                    None,
                    "proven".into(),
                    "validated ref.null element expression".into(),
                )
            }
            _ => {}
        }
    }
    (
        None,
        "unknown".into(),
        "element expression is not a bounded ref.func/ref.null".into(),
    )
}

fn opt_json(value: &Option<String>) -> String {
    value
        .as_deref()
        .map(json_string)
        .unwrap_or_else(|| "null".into())
}
fn source_json(source: &Option<SourceLink>) -> String {
    source
        .as_ref()
        .map(|source| {
            format!(
                "{{\"artifact\":{},\"start\":{},\"end\":{}}}",
                json_string(&source.artifact),
                source.start,
                source.end
            )
        })
        .unwrap_or_else(|| "null".into())
}

pub fn boundary_json(report: &WasmBoundaryReport) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"schema_version\":{},\n  \"analysis_version\":{},\n  \"pipeline_input_hash\":{},\n  \"deob_output_hash\":{},\n  \"wasm_present\":{},\n  \"js_only\":{},\n", json_string(&report.schema_version), json_string(&report.analysis_version), json_string(&report.pipeline_input_hash), json_string(&report.deob_output_hash), report.wasm_present, report.js_only));
    out.push_str(&format!("  \"proof_counts\":{{\"imports\":{},\"imports_linked\":{},\"functions\":{},\"table_entries\":{},\"table_entries_proven\":{},\"call_sites\":{},\"handler_mappings\":{}}},\n", report.proof_counts.imports, report.proof_counts.imports_linked, report.proof_counts.functions, report.proof_counts.table_entries, report.proof_counts.table_entries_proven, report.proof_counts.call_sites, report.proof_counts.handler_mappings));
    out.push_str("  \"modules\":[");
    for (index, module) in report.modules.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&module_json(module));
    }
    out.push_str("],\n  \"unsupported\":[");
    for (index, finding) in report.unsupported.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&unsupported_json(finding));
    }
    out.push_str("]\n}\n");
    out
}

fn module_json(module: &WasmModuleBoundary) -> String {
    let imports = module
        .imports
        .iter()
        .map(import_json)
        .collect::<Vec<_>>()
        .join(",");
    let exports = module
        .exports
        .iter()
        .map(|item| {
            format!(
                "{{\"name\":{},\"kind\":{},\"index\":{},\"function_index\":{},\"signature\":{}}}",
                json_string(&item.name),
                json_string(&item.kind),
                item.index,
                item.function_index
                    .map_or_else(|| "null".into(), |value| value.to_string()),
                opt_json(&item.signature)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let tables = module
        .tables
        .iter()
        .map(|table| {
            let entries = table
                .entries
                .iter()
                .map(|entry| {
                    format!(
                        "{{\"table_index\":{},\"element_index\":{},\"function_index\":{},\"status\":{},\"evidence\":{}}}",
                        entry.table_index,
                        entry.element_index,
                        entry
                            .function_index
                            .map_or_else(|| "null".into(), |value| value.to_string()),
                        json_string(&entry.status),
                        json_string(&entry.evidence)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"table_index\":{},\"imported\":{},\"element_type\":{},\"initial\":{},\"maximum\":{},\"mutable\":{},\"status\":{},\"exported_as\":[{}],\"entries\":[{}]}}",
                table.table_index,
                table.imported,
                json_string(&table.element_type),
                table.initial,
                table.maximum
                    .map_or_else(|| "null".into(), |value| value.to_string()),
                table.mutable,
                json_string(&table.status),
                table
                    .exported_as
                    .iter()
                    .map(|value| json_string(value))
                    .collect::<Vec<_>>()
                    .join(","),
                entries
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let calls = module.call_sites.iter().map(|item| format!("{{\"caller_function_index\":{},\"byte_offset\":{},\"kind\":{},\"target_function_index\":{},\"table_index\":{},\"type_index\":{},\"signature\":{},\"operand_order\":{},\"argument_provenance\":{},\"source\":{}}}", item.caller_function_index, item.byte_offset, json_string(&item.kind), item.target_function_index.map_or_else(|| "null".into(), |value| value.to_string()), item.table_index.map_or_else(|| "null".into(), |value| value.to_string()), item.type_index.map_or_else(|| "null".into(), |value| value.to_string()), json_string(&item.signature), json_string(&item.operand_order), opt_json(&item.argument_provenance), source_json(&item.source))).collect::<Vec<_>>().join(",");
    let mappings = module.handler_mappings.iter().map(|item| format!("{{\"table_index\":{},\"element_index\":{},\"function_index\":{},\"import_name\":{},\"host_wrapper\":{},\"original_handler_key\":{},\"status\":{},\"evidence\":[{}]}}", item.table_index, item.element_index, item.function_index, opt_json(&item.import_name), opt_json(&item.host_wrapper), item.original_handler_key.map_or_else(|| "null".into(), |value| value.to_string()), json_string(&item.status), item.evidence.iter().map(|value| json_string(value)).collect::<Vec<_>>().join(","))).collect::<Vec<_>>().join(",");
    format!("{{\"source_binding\":{},\"extraction_source\":{},\"deob_source\":{},\"module_hash\":{},\"byte_len\":{},\"instance_id\":{},\"validated\":{},\"status\":{},\"imports\":[{}],\"exports\":[{}],\"tables\":[{}],\"calls\":[{}],\"handler_mappings\":[{}],\"unsupported\":[{}]}}", opt_json(&module.source_binding), source_json(&module.extraction_source), source_json(&module.deob_source), json_string(&module.module_hash), module.byte_len, module.instance_id.map_or_else(|| "null".into(), |value| value.to_string()), module.validated, json_string(&module.status), imports, exports, tables, calls, mappings, module.unsupported.iter().map(unsupported_json).collect::<Vec<_>>().join(","))
}
fn import_json(item: &WasmImport) -> String {
    let operation_table = item.operation_table.as_ref().map_or_else(
        || "null".into(),
        |link| {
            format!(
                "{{\"declaration_ordinal\":{},\"property\":{}}}",
                link.declaration_ordinal,
                json_string(&link.property)
            )
        },
    );
    format!("{{\"index\":{},\"function_index\":{},\"namespace\":{},\"name\":{},\"kind\":{},\"signature\":{},\"js_binding\":{},\"js_property\":{},\"operation_table\":{},\"source_instance_id\":{},\"source\":{},\"status\":{},\"evidence\":[{}]}}", item.index, item.function_index.map_or_else(|| "null".into(), |value| value.to_string()), json_string(&item.namespace), json_string(&item.name), json_string(&item.kind), json_string(&item.signature), opt_json(&item.js_binding), opt_json(&item.js_property), operation_table, item.source_instance_id.map_or_else(|| "null".into(), |value| value.to_string()), source_json(&item.source), json_string(&item.status), item.evidence.iter().map(|value| json_string(value)).collect::<Vec<_>>().join(","))
}
fn unsupported_json(item: &UnsupportedFinding) -> String {
    format!(
        "{{\"area\":{},\"reason\":{},\"source\":{}}}",
        json_string(&item.area),
        json_string(&item.reason),
        source_json(&item.source)
    )
}

pub fn boundary_markdown(report: &WasmBoundaryReport) -> String {
    let mut out = format!("# WASM/JS boundary\n\n- Schema: `{}`\n- WebAssembly module extracted: `{}`\n- No WebAssembly module extracted: `{}`\n- Proof counts: imports `{}/{}`, table entries `{}/{}`, handler mappings `{}`\n\n", report.schema_version, report.wasm_present, report.js_only, report.proof_counts.imports_linked, report.proof_counts.imports, report.proof_counts.table_entries_proven, report.proof_counts.table_entries, report.proof_counts.handler_mappings);
    for (index, module) in report.modules.iter().enumerate() {
        out.push_str(&format!("## Module {}\n\n- Binding: `{}`\n- Hash: `{}`\n- Bytes: {}\n- Status: `{}`\n\n| Import | Kind | Function index | JS link | Status |\n|---|---|---:|---|---|\n", index, module.source_binding.as_deref().unwrap_or("unknown"), module.module_hash, module.byte_len, module.status));
        for import in &module.imports {
            out.push_str(&format!(
                "| `{}::{}` | {} | {} | `{}` | {} |\n",
                import.namespace,
                import.name,
                import.kind,
                import
                    .function_index
                    .map_or_else(|| "-".into(), |value| value.to_string()),
                import.js_binding.as_deref().unwrap_or("unknown"),
                import.status
            ));
        }
        out.push('\n');
        for table in &module.tables {
            let proven = table
                .entries
                .iter()
                .filter(|entry| entry.status == "proven")
                .count();
            out.push_str(&format!(
                "- Table `{}`: {} `{}/{} entries proven`{}\n",
                table.table_index,
                if table.imported {
                    "imported"
                } else {
                    "defined"
                },
                proven,
                table.entries.len(),
                table
                    .exported_as
                    .first()
                    .map_or_else(String::new, |name| format!("; export `{name}`"))
            ));
        }
        out.push('\n');
        for finding in &module.unsupported {
            out.push_str(&format!(
                "- Unsupported: **{}** — {}\n",
                finding.area, finding.reason
            ));
        }
        out.push('\n');
    }
    if report.modules.is_empty() {
        out.push_str("No statically extracted WebAssembly module was found. Interpreter placement is not classified by this boundary analysis.\n\n");
    }
    for finding in &report.unsupported {
        out.push_str(&format!(
            "- Unsupported: **{}** — {}\n",
            finding.area, finding.reason
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::wasm_module::test_support::*;

    #[test]
    fn detailed_parse_accepts_valid_module() {
        let parsed = parse_detailed(&memory_export_module()).expect("valid module");
        assert_eq!(parsed.exports.len(), 1);
        assert_eq!(parsed.exports[0].name, "m");
    }

    #[test]
    fn detailed_parse_reports_old_gc_unsupported_reason() {
        // The rich boundary already rejected GC types before the refactor.
        // This pins that the rejection survives the shared payload stream and
        // remains separate from the config/layout/properties ABI projection.
        let error = parse_detailed(&gc_memory_export_module()).expect_err("GC unsupported");
        assert!(
            error.starts_with("gc proposal not supported"),
            "unexpected detailed error: {error}"
        );
    }

    #[test]
    fn detailed_parse_preserves_validation_error_precedence() {
        let bytes = invalid_export_module();
        let expected = match wasmparser::Validator::new().validate_all(&bytes) {
            Ok(_) => panic!("fixture must be validator-invalid"),
            Err(error) => error.to_string(),
        };
        assert_eq!(parse_detailed(&bytes), Err(expected));
    }

    #[test]
    fn detailed_parse_accepts_large_legacy_module() {
        let bytes = large_custom_section_module();
        assert!(bytes.len() > 8 * 1024 * 1024);
        let parsed = parse_detailed(&bytes).expect("old boundary had no byte cap");
        assert_eq!(parsed.exports[0].name, "m");
    }
}
