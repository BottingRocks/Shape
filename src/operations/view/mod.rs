//! Unversioned inspection projections for `shape ops list` and
//! `shape ops show`, plus the instruction-occurrence projection.
//!
//! These views are derived from `shape-kernel-operation-catalogue/v1`. They do
//! not add a schema version, do not alter the catalogue serializer, and keep
//! the catalogue FNV `source_hash` explicitly labeled as FNV. Capture identity is
//! SHA-256 of the exact raw input bytes and is never synthesized from
//! deobfuscated output.
//!
//! `ops show` consumes one [`PipelineReport`] through [`Analyses`]: the
//! catalogue, handler templates, and operand separation are separate analyses,
//! joined only by the proven `table_key`. Their statuses stay distinct
//! (inventory/classification, recipe extraction, recipe admission, and operand
//! lifting are four different facts).
//!
//! The instruction-occurrence projection consumes an existing
//! [`DispatchExecutionRecord`]. It revalidates the dispatch selector join and
//! never invents operands for a completion record. Its decode classifier
//! distinguishes missing context, truncation, unsupported descriptors, a
//! handler with no certified operands, and a completed decode.

use std::collections::BTreeMap;

use serde::ser::{SerializeSeq, SerializeStruct, Serializer};
use serde::Serialize;

use crate::dispatch::{DispatchExecutionRecord, DispatchPreparation};
use crate::operations::catalogue::{
    to_json, InventoryCounts, MicroOperation, OperationCatalogue, OperationRecord, OperationTable,
};
use crate::operations::operands::{
    bounded_decode_operands, DecodeOperandsError, DecodedOperand, JsOperandSeparationReport,
    LiftedHandler, OperandCoverage, OperandEndian, OperandEntryCoordinate, OperandHandlerStatus,
};
use crate::operations::templates::{
    admit_recipe, AdmissionError, HandlerTemplate, HandlerTemplateReport, RecipeBounds,
    RecipeOperation, SemanticValue, TemplateCoverage, TemplateStatus,
};
use crate::transforms::deob::pipeline::{PipelineReport, ProvenOperationTable};

/// Identity of the exact bytes supplied to this invocation.
#[derive(Debug, Clone, Copy)]
pub struct Capture<'a> {
    pub input_kind: &'a str,
    pub digest: &'a str,
    pub byte_length: u64,
    /// `verified` only when the hash is over raw input bytes; `unavailable`
    /// when only a deobfuscated supplied source exists.
    pub raw_identity: &'a str,
}

/// Coordinate space used by handler spans in this projection.
#[derive(Debug, Clone, Copy)]
pub struct SpanSource<'a> {
    pub digest: &'a str,
    pub byte_length: usize,
}

/// Availability of one optional analysis. `Unavailable` means the analysis was
/// requested but failed; it is distinct from an omitted analysis.
#[derive(Debug, Clone)]
pub enum AnalysisData<T> {
    Included(T),
    Unavailable { reason: String },
}

impl<T> AnalysisData<T> {
    pub fn included(&self) -> Option<&T> {
        match self {
            Self::Included(value) => Some(value),
            Self::Unavailable { .. } => None,
        }
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Included(_) => None,
            Self::Unavailable { reason } => Some(reason),
        }
    }

    pub fn status(&self) -> &'static str {
        match self {
            Self::Included(_) => "included",
            Self::Unavailable { .. } => "unavailable",
        }
    }
}

/// The handler-template and operand-separation analyses for one report.
///
/// Every analysis keeps its own scanning policy and is joined later by the
/// proven table key. A failure in one analysis never fabricates a result for
/// another.
#[derive(Debug, Clone)]
pub struct Analyses {
    /// Pipeline-report input identity (FNV over the caller-supplied bytes).
    pub source_hash: String,
    /// The single proven operation table the analyses were given.
    pub operation_table: Option<ProvenOperationTable>,
    pub templates: AnalysisData<HandlerTemplateReport>,
    pub operands: AnalysisData<JsOperandSeparationReport>,
}

impl Analyses {
    /// Run both optional analyses from one report. Failures are retained as
    /// unavailable analyses rather than aborting the whole inspection.
    pub fn from_report(report: &PipelineReport) -> Self {
        let templates = match crate::operations::templates::handler_templates_from_report(report) {
            Ok(report) => AnalysisData::Included(report),
            Err(error) => AnalysisData::Unavailable {
                reason: error.to_string(),
            },
        };
        let operands = match crate::operations::operands::js_operand_separation_from_report(report)
        {
            Ok(report) => AnalysisData::Included(report),
            Err(error) => AnalysisData::Unavailable {
                reason: error.to_string(),
            },
        };
        Self {
            source_hash: report.input_hash.clone(),
            operation_table: report.proven_operation_tables.first().cloned(),
            templates,
            operands,
        }
    }

    /// Construct an analysis set from already-computed reports. Tests use this
    /// to mutate one row and prove the join fails closed.
    pub fn new(
        source_hash: String,
        operation_table: Option<ProvenOperationTable>,
        templates: AnalysisData<HandlerTemplateReport>,
        operands: AnalysisData<JsOperandSeparationReport>,
    ) -> Self {
        Self {
            source_hash,
            operation_table,
            templates,
            operands,
        }
    }

    pub fn templates(&self) -> Option<&HandlerTemplateReport> {
        self.templates.included()
    }

    pub fn operands(&self) -> Option<&JsOperandSeparationReport> {
        self.operands.included()
    }
}

pub struct OpsView<'a> {
    pub catalogue: &'a OperationCatalogue,
    pub capture: Capture<'a>,
    pub span_source: Option<SpanSource<'a>>,
    /// Present only for `ops show`; `ops list` deliberately does not join the
    /// per-handler analyses and keeps its historical projection unchanged.
    pub analyses: Option<&'a Analyses>,
    /// Display-only analyst note. It never participates in admission or
    /// execution and is serialized with explicit non-authority metadata.
    pub annotation: Option<&'a str>,
}

pub fn handler_by_key(catalogue: &OperationCatalogue, key: u32) -> Option<&OperationRecord> {
    catalogue
        .handlers
        .iter()
        .find(|handler| handler.table_key == key)
}

pub fn available_keys(catalogue: &OperationCatalogue) -> Vec<u32> {
    let mut keys = catalogue
        .handlers
        .iter()
        .map(|handler| handler.table_key)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys.dedup();
    keys
}

pub fn ops_list_json(view: &OpsView<'_>) -> String {
    let catalogue = view.catalogue;
    to_json(&OpsListJson {
        capture: capture_json(&view.capture),
        derived_from: derived_from_json(catalogue),
        table: &catalogue.table,
        inventory: &catalogue.inventory,
        handlers: ListHandlers(&catalogue.handlers),
    })
}

pub fn ops_show_json(view: &OpsView<'_>, handler: &OperationRecord) -> String {
    let catalogue = view.catalogue;
    let span_source = view.span_source.unwrap_or(SpanSource {
        digest: "",
        byte_length: 0,
    });
    let context = show_context(catalogue, view.analyses, handler);
    to_json(&OpsShowJson {
        capture: capture_json(&view.capture),
        derived_from: derived_from_json(catalogue),
        table: &catalogue.table,
        handler: ShowHandlerJson(handler),
        span_source: span_source_json(&span_source),
        analyses: analyses_json(view.analyses, &context),
        analyst_annotation: view.annotation.map(|text| AnalystAnnotationJson {
            text,
            authority: "analyst_note",
            execution_authority: false,
        }),
    })
}

pub fn ops_list_markdown(view: &OpsView<'_>) -> String {
    let catalogue = view.catalogue;
    let mut out = String::from("# Operations\n\n");
    out.push_str(&format!(
        "- Capture: `sha256:{}` ({}, {} bytes)\n",
        view.capture.digest, view.capture.input_kind, view.capture.byte_length
    ));
    out.push_str(&format!("- Analysis: `{}`\n", catalogue.catalogue_version));
    out.push_str(&format!(
        "- Legacy source hash: `fnv1a64:{}`\n",
        catalogue.source_hash
    ));
    out.push_str(&format!(
        "- Table: `{}` (declaration ordinal {}; {} elements, {} handlers)\n",
        catalogue.table.canonical_name,
        catalogue.table.declaration_ordinal,
        catalogue.table.element_count,
        catalogue.table.function_count
    ));
    out.push_str(&format!(
        "- Inventory: exact {}, partial {}, unknown {}, sparse {}\n",
        catalogue.inventory.exact,
        catalogue.inventory.partial,
        catalogue.inventory.unknown,
        catalogue.inventory.missing_sparse_entries
    ));
    out.push_str(
        "\nHandler order is table order. Keys are per-capture table keys and may differ from handler indices.\n\n",
    );
    out.push_str(
        "| Key | Index | Family | Class | Micro-ops | Confidence | Unknowns |\n|---:|---:|---|---|---|---|---:|\n",
    );
    for handler in &catalogue.handlers {
        let ops = handler
            .ordered_micro_operations
            .iter()
            .map(|operation| operation.kind.as_str())
            .collect::<Vec<_>>()
            .join(" -> ");
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            handler.table_key,
            handler.handler_index,
            handler.semantic_family.as_str(),
            handler.classification.as_str(),
            ops,
            handler.confidence().level().as_str(),
            handler.unknown_fragments.len()
        ));
    }
    out
}

pub fn ops_show_markdown(view: &OpsView<'_>, handler: &OperationRecord) -> String {
    let catalogue = view.catalogue;
    let span_source = view.span_source.unwrap_or(SpanSource {
        digest: "",
        byte_length: 0,
    });
    let context = show_context(catalogue, view.analyses, handler);
    let mut out = String::from("# Operation\n\n");
    out.push_str(&format!(
        "- Capture: `sha256:{}` ({}, {} bytes; raw identity {})\n",
        view.capture.digest,
        view.capture.input_kind,
        view.capture.byte_length,
        view.capture.raw_identity
    ));
    out.push_str(&format!("- Analysis: `{}`\n", catalogue.catalogue_version));
    out.push_str(&format!(
        "- Table: `{}` (declaration ordinal {}; {} elements, {} handlers)\n",
        catalogue.table.canonical_name,
        catalogue.table.declaration_ordinal,
        catalogue.table.element_count,
        catalogue.table.function_count
    ));
    out.push_str(&format!(
        "- Handler: key {}, index {}\n",
        handler.table_key, handler.handler_index
    ));
    out.push_str(&format!(
        "- Kernel identity: `{}`\n",
        handler.kernel_identity
    ));
    out.push_str(&format!(
        "- Family / class: {} / {}\n",
        handler.semantic_family.as_str(),
        handler.classification.as_str()
    ));
    out.push_str(&format!(
        "- Fingerprint: `{}`\n",
        handler.structural_fingerprint
    ));
    out.push_str(&format!(
        "- Confidence: {} (`{}`)\n",
        handler.confidence().level().as_str(),
        handler.confidence().proof_rule_id().as_str()
    ));
    out.push_str(&format!(
        "- Span source: `sha256:{}` over the deobfuscated pipeline output ({} bytes); handler spans below use those offsets, not raw-source offsets\n",
        span_source.digest, span_source.byte_length
    ));
    out.push_str(&format!(
        "- Legacy source hash: `fnv1a64:{}`\n",
        catalogue.source_hash
    ));

    out.push_str("\n## Description\n\n");
    out.push_str(&describe_handler(&context, handler));
    out.push('\n');

    out.push_str("\n## Handler definition\n\n");
    out.push_str("| Fact | Status | Detail |\n|---|---|---|\n");
    out.push_str(&format!(
        "| Inventory / classification | {} | family {}; confidence {} (`{}`) |\n",
        handler.classification.as_str(),
        handler.semantic_family.as_str(),
        handler.confidence().level().as_str(),
        handler.confidence().proof_rule_id().as_str()
    ));
    out.push_str(&format!(
        "| Extracted recipe | {} | {} |\n",
        template_status_cell(&context),
        template_detail_cell(&context)
    ));
    out.push_str(&format!(
        "| Recipe admission | {} | {} |\n",
        context.admission.status,
        admission_detail_cell(&context.admission)
    ));
    out.push_str(&format!(
        "| Operand lifting | {} | {} |\n",
        operand_status_cell(&context),
        operand_detail_cell(&context)
    ));

    out.push_str("\n## Ordered micro-operations\n\n");
    out.push_str("| Kind | Operator | Operand roles | Span |\n|---|---|---|---|\n");
    for operation in &handler.ordered_micro_operations {
        out.push_str(&format!(
            "| {} | {} | {} | {}..{} |\n",
            operation.kind.as_str(),
            operation.operator.as_deref().unwrap_or("-"),
            operation.operand_roles.join(", "),
            operation.span.start,
            operation.span.end
        ));
    }

    out.push_str("\n## Recipe\n\n");
    if let Some(template) = context.template {
        out.push_str(&format!(
            "Joined by `table_key` {} (row `handler_index` {}); status `{}`; {} steps.\n\n",
            template.table_key,
            template.handler_index,
            template.status,
            template.steps.len()
        ));
        if context
            .lifted
            .is_some_and(|lifted| lifted.status == OperandHandlerStatus::Lifted)
        {
            if let Some(lifted) = context.lifted {
                if lifted.template.steps.len() != template.steps.len() {
                    out.push_str(&format!(
                        "The operand-lifted recipe used for admission has {} steps; the extracted recipe above keeps every proven and unknown step.\n\n",
                        lifted.template.steps.len()
                    ));
                } else if lifted.template.status != template.status {
                    out.push_str(&format!(
                        "The operand-lifted recipe used for admission is `{}`.\n\n",
                        lifted.template.status
                    ));
                }
            }
        }
        out.push_str("| Step | Operation | Source span |\n|---:|---|---|\n");
        for step in &template.steps {
            out.push_str(&format!(
                "| {} | {} | {}..{} |\n",
                step.order,
                recipe_operation_name(&step.operation),
                step.span.start,
                step.span.end
            ));
        }
    } else {
        out.push_str(&format!(
            "No handler template is available ({}){}\n",
            context.template_join.status.as_str(),
            join_reason_suffix(&context.template_join)
        ));
    }

    out.push_str("\n## Operands\n\n");
    if let Some(lifted) = context.lifted {
        let input_binding = context
            .operands_report
            .and_then(|report| report.input_identity.as_ref())
            .map(|identity| {
                format!(
                    "input binding `{}` (display name only; declaration span {}..{})",
                    identity.binding_name,
                    identity.declaration_span.start,
                    identity.declaration_span.end
                )
            })
            .unwrap_or_else(|| "input binding unavailable".into());
        let entry_coordinate = context
            .operands_report
            .and_then(|report| report.entry_coordinate)
            .map(entry_coordinate_str)
            .unwrap_or("unavailable");
        out.push_str(&format!(
            "Status `{}`; entry coordinate `{}`; {}; PC delta {}; {}.\n",
            operand_status_str(&lifted.status),
            entry_coordinate,
            input_binding,
            lifted.pc_delta,
            join_detail(
                &context.operand_join,
                lifted.table_key,
                lifted.handler_index
            )
        ));
        if !lifted.unresolved.is_empty() {
            out.push_str(&format!(
                "\nUnresolved: {}.\n",
                lifted.unresolved.join("; ")
            ));
        }
        out.push('\n');
        out.push_str("| Position | Byte offset | Width | Byte order | Signedness | Mask | PC delta | Proven role |\n|---:|---:|---:|---|---|---|---:|---|\n");
        let roles = collect_operand_roles(lifted);
        for descriptor in &lifted.operands {
            let role = roles
                .get(&descriptor.position)
                .map(|uses| format_role_uses(uses))
                .filter(|role| !role.is_empty())
                .unwrap_or_else(|| "role unresolved".into());
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
                descriptor.position,
                descriptor.byte_offset,
                descriptor.width,
                endian_str(descriptor.endian),
                if descriptor.signed {
                    "signed"
                } else {
                    "unsigned"
                },
                format_mask(descriptor.mask),
                descriptor.pc_delta,
                role
            ));
        }
        if lifted.operands.is_empty() {
            out.push_str("\nNo certified inline operands.\n");
        }
    } else {
        out.push_str(&format!(
            "No operand-separation row is available ({}){}\n",
            context.operand_join.status.as_str(),
            join_reason_suffix(&context.operand_join)
        ));
    }

    out.push_str("\n## Source\n\n````js\n");
    out.push_str(&handler.source_snippet);
    out.push_str("\n````\n\n## Effects\n\n");
    out.push_str(&format!(
        "- Stack: {} reads, {} writes, delta {}\n",
        handler.effects.stack_reads, handler.effects.stack_writes, handler.effects.stack_delta
    ));
    out.push_str(&format!(
        "- Memory: {} reads, {} writes; slots {}\n",
        handler.effects.memory_reads,
        handler.effects.memory_writes,
        if handler.effects.memory_slots.is_empty() {
            "none".into()
        } else {
            handler.effects.memory_slots.join(", ")
        }
    ));
    out.push_str(&format!(
        "- Control flow: {}\n",
        if handler.effects.control_flow.is_empty() {
            "none".into()
        } else {
            handler.effects.control_flow.join(", ")
        }
    ));
    out.push_str(&format!(
        "- Exceptions: {}; unknown calls may throw: {}; constructor calls: {}; thread captures: {}\n",
        if handler.effects.exception_effects.is_empty() {
            "none".into()
        } else {
            handler.effects.exception_effects.join(", ")
        },
        handler.effects.calls_unknown_possible_throw,
        handler.effects.constructor_calls,
        handler.effects.thread_captures
    ));
    out.push_str("\n## Evidence\n\n");
    for evidence in handler.confidence().evidence() {
        out.push_str(&format!("- {evidence}\n"));
    }
    out.push_str("\n## Unknown fragments\n\n");
    if handler.unknown_fragments.is_empty() {
        out.push_str("None.\n");
    } else {
        for fragment in &handler.unknown_fragments {
            out.push_str(&format!(
                "- {}..{}: {} ({})\n",
                fragment.span.start,
                fragment.span.end,
                fragment.reason,
                fragment.body.replace('\n', " ")
            ));
        }
    }

    out.push_str("\n## Recipe unknown fragments\n\n");
    if let Some(template) = context.template {
        if template.unsupported.is_empty() {
            out.push_str("None.\n");
        } else {
            out.push_str(&format!(
                "Joined template status {}; {}.\n\n",
                template.status,
                count_noun(template.unsupported.len(), "unsupported fragment")
            ));
            for fragment in &template.unsupported {
                out.push_str(&format!(
                    "- {}..{}: {} ({})\n",
                    fragment.span.start,
                    fragment.span.end,
                    fragment.reason,
                    fragment.source.replace('\n', " ")
                ));
            }
        }
    } else {
        out.push_str("No handler template is available.\n");
    }

    out.push_str("\n## Unresolved\n\n");
    let unresolved = unresolved_lines(&context);
    if unresolved.is_empty() {
        out.push_str("None.\n");
    } else {
        for line in unresolved {
            out.push_str(&format!("- {line}\n"));
        }
    }

    out.push_str("\n## Analyst annotation\n\n");
    match view.annotation {
        Some(text) if !text.is_empty() => {
            for line in text.lines() {
                out.push_str(&format!("> {line}\n"));
            }
            out.push_str("\nDisplay-only analyst note; it carries no execution authority.\n");
        }
        Some(_) => out.push_str("An empty analyst note was supplied.\n"),
        None => out.push_str("None supplied.\n"),
    }

    out
}

// ---------------------------------------------------------------------------
// Handler join and status separation
// ---------------------------------------------------------------------------

mod json;
mod occurrence;
mod show;

pub use occurrence::*;

use json::*;
use show::*;
