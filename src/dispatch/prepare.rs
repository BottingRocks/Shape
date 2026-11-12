//! Proof-backed dispatch preparation. Every fact is re-derived from the
//! standardized report; nothing is manufactured to make a fixture pass.

use std::collections::BTreeSet;

use crate::{
    handler_templates_from_report, js_operand_separation_from_report_with_handoff_and_templates,
    runtime_config::{self, ArtifactKind, FactStatus, RuntimeValue},
    HandlerTemplateReport, JsOperandSeparationReport, OperandHandlerStatus, PipelineReport,
    ProvenOperationTable,
};

use super::proof;
use super::step::SelectorIndex;
use super::types::{
    CompletionContract, CompletionValue, DispatchCell, DispatchLimits, DispatchPreparation,
    DispatchPrepareError, DispatchStart,
};

/// Prepare from one source-verified pipeline report and an explicit entry and
/// completion contract. Preparation enforces `limits` before any allocation or
/// lowering work; the resulting value is read-only.
pub fn prepare_dispatch_from_report(
    source: &str,
    report: &PipelineReport,
    start: DispatchStart,
    completion: CompletionContract,
    limits: DispatchLimits,
) -> Result<DispatchPreparation, DispatchPrepareError> {
    let config = runtime_config::from_report(source, report, Default::default())
        .map_err(|error| DispatchPrepareError::RuntimeConfig(error.to_string()))?;

    // The dispatch image must be exactly one proven extracted fact. There is no
    // spelling fallback and no synthesized table.
    let dispatch_artifacts = config
        .artifacts
        .iter()
        .filter(|artifact| {
            artifact.kind == ArtifactKind::DispatchTable && artifact.status == FactStatus::Extracted
        })
        .collect::<Vec<_>>();
    if dispatch_artifacts.len() != 1 {
        return Err(DispatchPrepareError::InvalidFact(
            "dispatch requires exactly one proven extracted dispatch-table fact".into(),
        ));
    }
    let dispatch = parse_dispatch_value(
        dispatch_artifacts[0]
            .value
            .as_ref()
            .ok_or(DispatchPrepareError::MissingFact("dispatch table value"))?,
    )?;
    let table_cells = dispatch
        .iter()
        .try_fold(0usize, |total, row| total.checked_add(row.len()))
        .ok_or_else(|| DispatchPrepareError::InvalidFact("dispatch table size overflow".into()))?;
    if dispatch.is_empty() {
        return Err(DispatchPrepareError::InvalidFact(
            "dispatch table has no proven states".into(),
        ));
    }
    if dispatch.iter().any(Vec::is_empty) {
        return Err(DispatchPrepareError::InvalidFact(
            "dispatch table has an empty state row".into(),
        ));
    }
    if table_cells > limits.max_table_cells {
        return Err(DispatchPrepareError::Limit(format!(
            "dispatch table has {table_cells} cells, exceeding {}",
            limits.max_table_cells
        )));
    }
    let states = dispatch.len();
    for (state, row) in dispatch.iter().enumerate() {
        for (opcode, cell) in row.iter().enumerate() {
            if cell.next_state as usize >= states {
                return Err(DispatchPrepareError::InvalidFact(format!(
                    "dispatch cell [{state}][{opcode}] targets state {} outside the {states} proven states",
                    cell.next_state
                )));
            }
            if cell.next_state > limits.max_integer || cell.selector > limits.max_integer {
                return Err(DispatchPrepareError::Limit(format!(
                    "dispatch cell [{state}][{opcode}] exceeds the configured integer bound {}",
                    limits.max_integer
                )));
            }
        }
    }
    if start.state >= states {
        return Err(DispatchPrepareError::InvalidFact(format!(
            "entry state {} is outside the {states} proven states",
            start.state
        )));
    }

    // Resolve the one nested dispatch read and the method that owns it against
    // Oxc SymbolIds in the standardized artifact.
    let read = proof::prove_dispatch_read(&report.output)?;
    proof::require_binding_at_span(
        &report.output,
        &dispatch_artifacts[0].span,
        read.table,
        "dispatch table",
    )?;
    proof::prove_immutable_binding(&report.output, read.table, "dispatch table")?;
    if dispatch_artifacts[0].binding.as_deref() != Some(read.table_name.as_str()) {
        return Err(DispatchPrepareError::TableIdentity(
            "proven dispatch read table differs from the extracted dispatch-table binding".into(),
        ));
    }
    let handoff = read.handoff();

    let bytecode = match extracted_bytecode(&config)? {
        Some((bytes, binding)) => {
            let artifact = config
                .artifacts
                .iter()
                .find(|artifact| {
                    artifact.kind == ArtifactKind::JsBytecodeImage
                        && artifact.status == FactStatus::Extracted
                })
                .ok_or(DispatchPrepareError::MissingFact("JS bytecode image"))?;
            proof::require_binding_at_span(
                &report.output,
                &artifact.span,
                read.bytecode,
                "bytecode image",
            )?;
            if binding.as_deref() != Some(read.bytecode_name.as_str()) {
                return Err(DispatchPrepareError::TableIdentity(
                    "extracted bytecode image binding differs from the proven dispatch read".into(),
                ));
            }
            bytes
        }
        None => proof::prove_direct_bytecode(&report.output, &read)?,
    };
    if bytecode.is_empty() {
        return Err(DispatchPrepareError::InvalidFact(
            "bytecode image is empty".into(),
        ));
    }
    if bytecode.len() > limits.max_image_bytes {
        return Err(DispatchPrepareError::Limit(format!(
            "bytecode image has {} bytes, exceeding {}",
            bytecode.len(),
            limits.max_image_bytes
        )));
    }
    if start.pc >= bytecode.len() {
        return Err(DispatchPrepareError::InvalidFact(format!(
            "entry PC {} is outside the {}-byte image",
            start.pc,
            bytecode.len()
        )));
    }

    // Exactly one proven operation table; it is never synthesized.
    let operation_table = single_operation_table(report)?;
    proof::validate_operation_table_declaration(&report.output, &operation_table)?;

    let mut templates = handler_templates_from_report(report)
        .map_err(|error| DispatchPrepareError::TableIdentity(error.to_string()))?;
    let operands = js_operand_separation_from_report_with_handoff_and_templates(
        report,
        Some(&handoff),
        &templates,
    )
    .map_err(|error| DispatchPrepareError::SourceJoin(error.to_string()))?;
    if config.source.digest != report.input_hash
        || templates.source_hash != report.input_hash
        || operands.source_hash != report.input_hash
    {
        return Err(DispatchPrepareError::SourceJoin(
            "runtime, template, operand, and pipeline source identities disagree".into(),
        ));
    }
    merge_lifted_templates(&mut templates, &operands);
    let template_index = SelectorIndex::from_handler_indices(
        templates
            .handlers
            .iter()
            .map(|handler| handler.handler_index),
    );
    let operand_index = SelectorIndex::from_handler_indices(
        operands
            .handlers
            .iter()
            .map(|handler| handler.handler_index),
    );

    if templates.table != operation_table || operands.table != operation_table {
        return Err(DispatchPrepareError::TableIdentity(
            "runtime, operand, template, and pipeline operation identities disagree".into(),
        ));
    }

    validate_completion(&dispatch, completion, &templates)?;
    require_lifted_handlers(&dispatch, completion, &operands)?;

    // Prove the selector join and take the consumer source span for the
    // terminal completion record. This is a real source coordinate, not a
    // fabricated origin.
    let completion_span =
        proof::prove_selector_join(&report.output, operation_table.declaration_ordinal, &read)?;

    let constant_tables = constant_string_tables(&config);
    Ok(DispatchPreparation {
        constant_tables,
        source_hash: config.source.digest,
        artifact_source_hash: config.artifact_source.digest,
        bytecode,
        table: dispatch,
        templates,
        operands,
        template_index,
        operand_index,
        start,
        completion,
        operation_table,
        limits,
        completion_span,
    })
}

type ExtractedBytecode = (Vec<u8>, Option<String>);

/// Every extracted, unmutated string table, keyed by its standardized binding
/// name. The tracer resolves `table[index]` with a concrete index to the
/// entry as a string literal.
pub(super) fn constant_string_tables(
    config: &runtime_config::RuntimeConfig,
) -> Vec<(String, std::sync::Arc<[String]>)> {
    config
        .artifacts
        .iter()
        .filter(|artifact| {
            artifact.kind == ArtifactKind::StringTable && artifact.status == FactStatus::Extracted
        })
        .filter_map(|artifact| {
            let RuntimeValue::Array(values) = artifact.value.as_ref()? else {
                return None;
            };
            let entries = values
                .iter()
                .map(|value| match value {
                    RuntimeValue::String(entry) => Some(entry.clone()),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            Some((artifact.binding.clone()?, entries.into()))
        })
        .collect()
}

/// The extracted immutable `vmNumberTable`, the handlers' number operands:
/// other number arrays are not read at operand indexes only.
pub(super) fn constant_number_tables(
    config: &runtime_config::RuntimeConfig,
) -> Vec<(String, std::sync::Arc<[f64]>)> {
    let name = crate::kernel::roles::VmRole::NumberTable.canonical();
    config
        .artifacts
        .iter()
        .filter(|artifact| {
            artifact.kind == ArtifactKind::NumberTable
                && artifact.status == FactStatus::Extracted
                && artifact.binding.as_deref() == Some(name)
        })
        .filter_map(|artifact| {
            let RuntimeValue::Array(values) = artifact.value.as_ref()? else {
                return None;
            };
            let entries = values
                .iter()
                .map(|value| match value {
                    RuntimeValue::Number(entry) => Some(*entry),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            Some((artifact.binding.clone()?, entries.into()))
        })
        .collect()
}

/// The source-proven `vmDecodeString` of `report`, if it is one.
pub(super) fn string_decoder(
    report: &PipelineReport,
) -> Option<std::sync::Arc<crate::vm_string::VmStringDecoder>> {
    crate::vm_string::VmStringDecoder::from_plan(&crate::vm_string::VmStringFoldPlan::compile(
        report,
    ))
    .map(std::sync::Arc::new)
}

fn extracted_bytecode(
    config: &runtime_config::RuntimeConfig,
) -> Result<Option<ExtractedBytecode>, DispatchPrepareError> {
    let artifacts = config
        .artifacts
        .iter()
        .filter(|artifact| {
            artifact.kind == ArtifactKind::JsBytecodeImage
                && artifact.status == FactStatus::Extracted
        })
        .collect::<Vec<_>>();
    if artifacts.len() > 1 {
        return Err(DispatchPrepareError::InvalidFact(
            "bytecode image identity is ambiguous".into(),
        ));
    }
    let Some(artifact) = artifacts.first() else {
        return Ok(None);
    };
    let bytes = artifact
        .bytes
        .clone()
        .ok_or(DispatchPrepareError::MissingFact("JS bytecode image"))?;
    Ok(Some((bytes, artifact.binding.clone())))
}

pub(super) fn single_operation_table(
    report: &PipelineReport,
) -> Result<ProvenOperationTable, DispatchPrepareError> {
    if report.proven_operation_tables.len() != 1 {
        return Err(DispatchPrepareError::TableIdentity(format!(
            "expected exactly one proven operation table, found {}",
            report.proven_operation_tables.len()
        )));
    }
    Ok(report.proven_operation_tables[0].clone())
}

pub(super) fn merge_lifted_templates(
    templates: &mut HandlerTemplateReport,
    operands: &JsOperandSeparationReport,
) {
    for lifted in &operands.handlers {
        if lifted.status == OperandHandlerStatus::Lifted {
            if let Some(template) = templates
                .handlers
                .iter_mut()
                .find(|template| template.handler_index == lifted.handler_index)
            {
                *template = lifted.template.clone();
            }
        }
    }
}

fn require_lifted_handlers(
    table: &[Vec<DispatchCell>],
    completion: CompletionContract,
    operands: &JsOperandSeparationReport,
) -> Result<(), DispatchPrepareError> {
    let sentinel = completion.selector();
    let mut required = BTreeSet::new();
    for cell in table.iter().flatten() {
        if cell.selector != sentinel {
            required.insert(cell.selector);
        }
    }
    for selector in required {
        let lifted = operands
            .handlers
            .iter()
            .find(|handler| handler.handler_index == selector)
            .ok_or_else(|| {
                DispatchPrepareError::TableIdentity(format!(
                    "selector {selector} has no operand-separation handler"
                ))
            })?;
        if lifted.status != OperandHandlerStatus::Lifted {
            return Err(DispatchPrepareError::SourceJoin(format!(
                "selector {selector} handler operands are not fully proven (status {:?})",
                lifted.status
            )));
        }
    }
    Ok(())
}

pub(super) fn integer(value: &RuntimeValue) -> Result<u32, DispatchPrepareError> {
    let RuntimeValue::Number(value) = value else {
        return Err(DispatchPrepareError::InvalidFact(
            "dispatch components must be numbers".into(),
        ));
    };
    if !value.is_finite() || value.fract() != 0.0 || *value < 0.0 || *value > u32::MAX as f64 {
        return Err(DispatchPrepareError::InvalidFact(
            "dispatch number is not a bounded u32".into(),
        ));
    }
    Ok(*value as u32)
}

pub(super) fn parse_dispatch_value(
    value: &RuntimeValue,
) -> Result<Vec<Vec<DispatchCell>>, DispatchPrepareError> {
    let RuntimeValue::Array(rows) = value else {
        return Err(DispatchPrepareError::InvalidFact(
            "dispatch table is not an array".into(),
        ));
    };
    if rows.is_empty() {
        return Err(DispatchPrepareError::InvalidFact(
            "dispatch table is empty".into(),
        ));
    }
    let mut cells = 0usize;
    rows.iter()
        .map(|row| {
            let RuntimeValue::Array(records) = row else {
                return Err(DispatchPrepareError::InvalidFact(
                    "dispatch row is not an array".into(),
                ));
            };
            records
                .iter()
                .map(|record| {
                    let RuntimeValue::Array(pair) = record else {
                        return Err(DispatchPrepareError::InvalidFact(
                            "dispatch cell is not a pair".into(),
                        ));
                    };
                    if pair.len() != 2 {
                        return Err(DispatchPrepareError::InvalidFact(
                            "dispatch cell is not exactly [state, selector]".into(),
                        ));
                    }
                    cells = cells.checked_add(1).ok_or_else(|| {
                        DispatchPrepareError::InvalidFact("dispatch cell count overflow".into())
                    })?;
                    Ok(DispatchCell {
                        next_state: integer(&pair[0])?,
                        selector: integer(&pair[1])?,
                    })
                })
                .collect()
        })
        .collect::<Result<Vec<_>, _>>()
}

fn validate_completion(
    table: &[Vec<DispatchCell>],
    completion: CompletionContract,
    templates: &HandlerTemplateReport,
) -> Result<(), DispatchPrepareError> {
    let selector = completion.selector();
    if templates
        .handlers
        .iter()
        .any(|handler| handler.handler_index == selector)
    {
        return Err(DispatchPrepareError::InvalidFact(
            "completion selector collides with an operation-table handler".into(),
        ));
    }
    if !table.iter().flatten().any(|cell| cell.selector == selector) {
        return Err(DispatchPrepareError::InvalidFact(
            "completion selector is not present in the proven dispatch image".into(),
        ));
    }
    let handler_keys = templates
        .handlers
        .iter()
        .map(|handler| handler.handler_index)
        .collect::<BTreeSet<_>>();
    if handler_keys.len() != templates.handlers.len() {
        return Err(DispatchPrepareError::InvalidFact(
            "operation-table handler identities are not unique".into(),
        ));
    }
    if table
        .iter()
        .flatten()
        .any(|cell| cell.selector != selector && !handler_keys.contains(&cell.selector))
    {
        return Err(DispatchPrepareError::InvalidFact(
            "dispatch selector has no unique operation-table handler".into(),
        ));
    }
    if let CompletionValue::Stack { index } = completion.value() {
        let _ = index;
    }
    Ok(())
}
