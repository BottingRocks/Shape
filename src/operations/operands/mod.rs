//! Opt-in, fail-closed separation of inline JavaScript handler operands.
//!
//! This module consumes one standardized [`PipelineReport`](crate::PipelineReport)
//! and produces decoder evidence plus the existing handler-template semantics.
//! It never changes the report output and never executes JavaScript or WASM.
//! Names such as `context`, `P`, and `r` are only display hints: certification
//! requires an Oxc `SymbolId` relation proven by the dispatch method, or the
//! narrow [`JsOperandBindingHandoff`] supplied by a later runtime-config pass.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, UpdateOperator};
use serde::{Deserialize, Serialize};

use crate::transforms::deob::pipeline::{PipelineReport, ProvenOperationTable};
use crate::{
    analysis,
    operations::templates::{
        HandlerTemplate, HandlerTemplateReport, RecipeOperation, SemanticValue, TemplateStatus,
    },
};

use super::table_scan::{scan_target_table, Handler, TableLiteral, TableScanMode, TableSink};
use lift::js_int32;
use resolve::{resolve_proof, unresolved_report, OperandTableSink};

pub const SCHEMA_VERSION: &str = "shape-js-operand-separation/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperandEntryCoordinate {
    /// The JavaScript dispatch method consumed the opcode before calling the
    /// handler.  Offset zero is therefore the first operand byte.
    AfterOpcode,
    /// The supplied binding handoff says the handler starts at the opcode.
    AtOpcode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperandEndian {
    Little,
    Big,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperandSpan {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperandInputIdentity {
    /// The resolved binding name is display metadata. `declaration_span` is
    /// the stable identity used with the standardized output.
    pub binding_name: String,
    pub declaration_span: OperandSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperandDescriptor {
    pub position: u32,
    pub byte_offset: u32,
    pub width: u8,
    pub endian: OperandEndian,
    /// True when the resulting number is JavaScript Int32-signed.  Four-byte
    /// bitwise compositions use this because `|`, `<<`, and `>>` coerce to
    /// Int32; `>>> 0` is represented as unsigned.
    pub signed: bool,
    pub mask: Option<u32>,
    pub pc_delta: u32,
    pub source_spans: Vec<OperandSpan>,
    pub input_identity: OperandInputIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapturedDependencyRole {
    Bytecode,
    Helper,
    External,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedDependency {
    pub name: String,
    pub role: CapturedDependencyRole,
    pub declaration_span: OperandSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperandHandlerStatus {
    Lifted,
    Partial,
    Unresolved,
    /// No certified inline bytecode read was found. The original template is
    /// retained unchanged; this is not an extraction failure.
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiftedHandler {
    pub handler_index: u32,
    pub table_key: u32,
    pub source_span: OperandSpan,
    pub entry_coordinate: OperandEntryCoordinate,
    pub operands: Vec<OperandDescriptor>,
    pub pc_delta: u32,
    pub captured_dependencies: Vec<CapturedDependency>,
    pub template: HandlerTemplate,
    pub status: OperandHandlerStatus,
    pub unresolved: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OperandCoverage {
    pub lifted: u32,
    pub partial: u32,
    pub unresolved: u32,
    pub not_applicable: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsOperandSeparationReport {
    pub schema_version: String,
    pub source_hash: String,
    pub table: ProvenOperationTable,
    pub input_identity: Option<OperandInputIdentity>,
    pub entry_coordinate: Option<OperandEntryCoordinate>,
    pub handlers: Vec<LiftedHandler>,
    pub coverage: OperandCoverage,
    pub unresolved: Vec<String>,
}

/// A narrow handoff for a later runtime-config extractor.  The module resolves
/// `declaration_span` back to an actual Oxc `SymbolId`; it never trusts the
/// spelling alone.  `pc_property` is the proven context field (for example a
/// minified `P`), and `entry_coordinate` records whether dispatch consumed the
/// opcode first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsOperandBindingHandoff {
    pub bytecode_declaration_span: OperandSpan,
    pub pc_property: String,
    pub entry_coordinate: OperandEntryCoordinate,
}

#[derive(Debug)]
pub enum JsOperandSeparationError {
    Parse(String),
    NoProvenTable,
    TableIdentityMismatch(String),
    InvalidHandoff(String),
}

impl fmt::Display for JsOperandSeparationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "operand separation parse failed: {error}"),
            Self::NoProvenTable => {
                f.write_str("pipeline report contains no proven operation table")
            }
            Self::TableIdentityMismatch(error) => write!(f, "operation table mismatch: {error}"),
            Self::InvalidHandoff(error) => write!(f, "invalid operand binding handoff: {error}"),
        }
    }
}
impl std::error::Error for JsOperandSeparationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedOperand {
    pub position: u32,
    pub value: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeOperandsError {
    Unsupported(String),
    Truncated {
        position: u32,
        required_end: usize,
        image_len: usize,
    },
}

impl fmt::Display for DecodeOperandsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(reason) => write!(f, "unsupported operand recipe: {reason}"),
            Self::Truncated {
                position,
                required_end,
                image_len,
            } => write!(
                f,
                "operand {position} needs byte {required_end}, but image has {image_len} bytes"
            ),
        }
    }
}
impl std::error::Error for DecodeOperandsError {}

/// Decode one lifted handler without mutating the image.  This is deliberately
/// not a VM dispatcher: it only applies the bounded descriptors recorded for
/// one handler entry.
pub fn bounded_decode_operands(
    image: &[u8],
    handler_entry_pc: usize,
    handler: &LiftedHandler,
) -> Result<Vec<DecodedOperand>, DecodeOperandsError> {
    let mut decoded = Vec::with_capacity(handler.operands.len());
    for descriptor in &handler.operands {
        let start = handler_entry_pc
            .checked_add(descriptor.byte_offset as usize)
            .ok_or_else(|| DecodeOperandsError::Unsupported("byte offset overflow".into()))?;
        let end = start
            .checked_add(descriptor.width as usize)
            .ok_or_else(|| DecodeOperandsError::Unsupported("operand width overflow".into()))?;
        let bytes = image
            .get(start..end)
            .ok_or(DecodeOperandsError::Truncated {
                position: descriptor.position,
                required_end: end,
                image_len: image.len(),
            })?;
        let mut value = match descriptor.endian {
            OperandEndian::Little => bytes
                .iter()
                .enumerate()
                .fold(0_u64, |value, (index, byte)| {
                    value | (u64::from(*byte) << (index * 8))
                }),
            OperandEndian::Big => bytes
                .iter()
                .fold(0_u64, |value, byte| (value << 8) | u64::from(*byte)),
        };
        if let Some(mask) = descriptor.mask {
            value &= u64::from(mask);
        }
        let value = if descriptor.signed {
            js_int32(value) as i64
        } else {
            value as i64
        };
        decoded.push(DecodedOperand {
            position: descriptor.position,
            value,
        });
    }
    Ok(decoded)
}

/// Analyze the standardized report using the proven JS dispatch relationship.
pub fn js_operand_separation_from_report(
    report: &PipelineReport,
) -> Result<JsOperandSeparationReport, JsOperandSeparationError> {
    js_operand_separation_from_report_with_handoff(report, None)
}

/// Analyze a report with a narrow, already-resolved runtime-config handoff.
pub fn js_operand_separation_from_report_with_handoff(
    report: &PipelineReport,
    handoff: Option<&JsOperandBindingHandoff>,
) -> Result<JsOperandSeparationReport, JsOperandSeparationError> {
    let templates = crate::operations::templates::handler_templates_from_report(report)
        .map_err(|error| JsOperandSeparationError::Parse(error.to_string()))?;
    js_operand_separation_from_report_with_handoff_and_templates(report, handoff, &templates)
}

/// Same analysis as [`js_operand_separation_from_report_with_handoff`], but
/// accepts handler templates already extracted from the same report.  Callers
/// that need both surfaces can avoid parsing and walking the standardized
/// source a second time.
pub fn js_operand_separation_from_report_with_handoff_and_templates(
    report: &PipelineReport,
    handoff: Option<&JsOperandBindingHandoff>,
    templates: &HandlerTemplateReport,
) -> Result<JsOperandSeparationReport, JsOperandSeparationError> {
    let table = report
        .proven_operation_tables
        .first()
        .cloned()
        .ok_or(JsOperandSeparationError::NoProvenTable)?;
    if templates.table != table {
        return Err(JsOperandSeparationError::TableIdentityMismatch(
            "handler-template table identity differs from the pipeline report".into(),
        ));
    }
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, &report.output)
        .map_err(JsOperandSeparationError::Parse)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let proof = match resolve_proof(report, &program, &scoping, &table, handoff) {
        Ok(proof) => proof,
        Err(error) if handoff.is_none() => {
            return Ok(unresolved_report(
                report,
                table,
                &templates.handlers,
                error.to_string(),
            ));
        }
        Err(error) => return Err(error),
    };
    let mut sink = OperandTableSink {
        source: &report.output,
        scoping: &scoping,
        nodes: &nodes,
        proof,
        templates: &templates.handlers,
        handlers: Vec::new(),
    };
    scan_target_table(
        &program,
        table.declaration_ordinal,
        TableScanMode::Lenient,
        &mut sink,
    );
    if sink.handlers.is_empty() {
        return Err(JsOperandSeparationError::TableIdentityMismatch(format!(
            "declaration ordinal {} was not found",
            table.declaration_ordinal
        )));
    }
    let mut coverage = OperandCoverage::default();
    for handler in &sink.handlers {
        match handler.status {
            OperandHandlerStatus::Lifted => coverage.lifted += 1,
            OperandHandlerStatus::Partial => coverage.partial += 1,
            OperandHandlerStatus::Unresolved => coverage.unresolved += 1,
            OperandHandlerStatus::NotApplicable => coverage.not_applicable += 1,
        }
    }
    let input_identity = Some(sink.proof.input_identity.clone());
    let entry_coordinate = Some(sink.proof.entry_coordinate);
    Ok(JsOperandSeparationReport {
        schema_version: SCHEMA_VERSION.into(),
        source_hash: report.input_hash.clone(),
        table,
        input_identity,
        entry_coordinate,
        handlers: sink.handlers,
        coverage,
        unresolved: sink.proof.unresolved,
    })
}

mod lift;
mod resolve;
