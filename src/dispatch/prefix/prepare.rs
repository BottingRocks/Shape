//! Proof-backed preparation of the bounded JavaScript VM prefix program.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::SymbolId;
use oxc_span::{GetSpan, Span};

use crate::{
    handler_templates_from_report, js_operand_separation_from_report_with_handoff_and_templates,
    runtime_config::{self, ArtifactKind, FactStatus},
    DispatchCell, OperandSpan, PipelineReport,
};

use super::super::prepare::{merge_lifted_templates, parse_dispatch_value, single_operation_table};
use super::super::proof::{self, DispatchReadProof};
use super::super::step::SelectorIndex;
use super::proof as prefix_proof;
use super::types::{PrefixLimits, PrefixPrepareError};
use super::PrefixPreparation;

/// Prepare a bounded JavaScript VM prefix from one standardized report.
pub fn prepare_prefix_from_report(
    source: &str,
    report: &PipelineReport,
    limits: PrefixLimits,
) -> Result<PrefixPreparation, PrefixPrepareError> {
    // A kernel whose dispatch loop is a WASM interpreter has no JavaScript
    // dispatch table: its own preparation reads the interpreter instead.
    if let Some(preparation) = super::wasm::prepare_wasm_prefix(source, report, limits)? {
        return Ok(preparation);
    }
    let config = runtime_config::from_report(source, report, Default::default())
        .map_err(|error| PrefixPrepareError::RuntimeConfig(error.to_string()))?;
    prepare_prefix(report, config, limits)
}

/// Prepare a bounded JavaScript VM prefix from a complete saved report.  The
/// report's declared raw-capture identity is carried through, while every
/// structural proof is rebuilt against its standardized artifact output.
///
/// This supports the JavaScript dispatch path only: unlike
/// [`prepare_prefix_from_report`], it does not try the WASM interpreter
/// preparation, because a WASM kernel has no JavaScript dispatch table. Prepare
/// a WASM kernel from raw source or from a [`crate::KernelArtifact`] compiled
/// from raw source.
pub fn prepare_prefix_from_saved_report(
    report: &PipelineReport,
    limits: PrefixLimits,
) -> Result<PrefixPreparation, PrefixPrepareError> {
    let config = runtime_config::from_saved_report(report, Default::default())
        .map_err(|error| PrefixPrepareError::RuntimeConfig(error.to_string()))?;
    prepare_prefix(report, config, limits)
}

fn prepare_prefix(
    report: &PipelineReport,
    config: runtime_config::RuntimeConfig,
    limits: PrefixLimits,
) -> Result<PrefixPreparation, PrefixPrepareError> {
    let dispatch_artifacts = config
        .artifacts
        .iter()
        .filter(|artifact| {
            artifact.kind == ArtifactKind::DispatchTable && artifact.status == FactStatus::Extracted
        })
        .collect::<Vec<_>>();
    if dispatch_artifacts.len() != 1 {
        return Err(PrefixPrepareError::InvalidFact(
            "prefix requires exactly one proven extracted dispatch-table fact".into(),
        ));
    }
    let dispatch = parse_dispatch_value(
        dispatch_artifacts[0]
            .value
            .as_ref()
            .ok_or(PrefixPrepareError::MissingFact("dispatch table value"))?,
    )?;
    validate_table(&dispatch, limits)?;

    let read = proof::prove_dispatch_read(&report.output)?;
    proof::require_binding_at_span(
        &report.output,
        &dispatch_artifacts[0].span,
        read.table,
        "dispatch table",
    )?;
    proof::prove_immutable_binding(&report.output, read.table, "dispatch table")?;
    if dispatch_artifacts[0].binding.as_deref() != Some(read.table_name.as_str()) {
        return Err(PrefixPrepareError::InvalidFact(
            "proven dispatch read table differs from the extracted dispatch-table binding".into(),
        ));
    }

    let (initial_image, bytecode_span) = extract_image(&report.output, &config, &read, limits)?;

    let operation_table = single_operation_table(report)?;
    proof::validate_operation_table_declaration(&report.output, &operation_table)?;

    let handoff = read.handoff();
    let mut templates = handler_templates_from_report(report)
        .map_err(|error| PrefixPrepareError::InvalidFact(error.to_string()))?;
    let operands = js_operand_separation_from_report_with_handoff_and_templates(
        report,
        Some(&handoff),
        &templates,
    )
    .map_err(|error| PrefixPrepareError::SourceProof(error.to_string()))?;
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
        return Err(PrefixPrepareError::InvalidFact(
            "template and operand reports disagree on the operation table identity".into(),
        ));
    }

    let handler_spans = templates
        .handlers
        .iter()
        .map(|handler| {
            (
                handler.handler_index,
                OperandSpan {
                    start: handler.source_span.start,
                    end: handler.source_span.end,
                },
            )
        })
        .collect::<Vec<_>>();
    let patch_handler = prefix_proof::prove_patch_handler(
        &report.output,
        read.bytecode,
        &read.pc_property,
        &handler_spans,
    )
    .map_err(PrefixPrepareError::SourceProof)?;

    let run_loop = prefix_proof::prove_run_loop(
        &report.output,
        &read.method_name,
        operation_table.declaration_ordinal,
        read.context_constructor,
        &handler_spans,
    )
    .map_err(PrefixPrepareError::SourceProof)?;

    let constant_tables = super::super::prepare::constant_string_tables(&config);
    Ok(PrefixPreparation {
        source_hash: config.source.digest.clone(),
        artifact_source_hash: config.artifact_source.digest.clone(),
        initial_image,
        table: dispatch,
        patch_handler,
        templates,
        operands,
        template_index,
        operand_index,
        limits,
        bytecode_span,
        run_loop,
        operation_table,
        constant_tables,
        number_tables: super::super::prepare::constant_number_tables(&config),
        string_decoder: super::super::prepare::string_decoder(report),
        wasm: None,
        junk_entries: Default::default(),
    })
}

fn validate_table(
    table: &[Vec<DispatchCell>],
    limits: PrefixLimits,
) -> Result<(), PrefixPrepareError> {
    if table.is_empty() {
        return Err(PrefixPrepareError::InvalidFact(
            "dispatch table has no proven states".into(),
        ));
    }
    if table.iter().any(Vec::is_empty) {
        return Err(PrefixPrepareError::InvalidFact(
            "dispatch table has an empty state row".into(),
        ));
    }
    let cells = table
        .iter()
        .try_fold(0usize, |total, row| total.checked_add(row.len()))
        .ok_or_else(|| PrefixPrepareError::InvalidFact("dispatch table size overflow".into()))?;
    if cells > limits.max_table_cells {
        return Err(PrefixPrepareError::Limit(format!(
            "dispatch table has {cells} cells, exceeding {}",
            limits.max_table_cells
        )));
    }
    let states = table.len();
    for (state, row) in table.iter().enumerate() {
        for (opcode, cell) in row.iter().enumerate() {
            if cell.next_state as usize >= states {
                return Err(PrefixPrepareError::InvalidFact(format!(
                    "dispatch cell [{state}][{opcode}] targets state {} outside the {states} proven states",
                    cell.next_state
                )));
            }
            if cell.next_state > limits.max_integer || cell.selector > limits.max_integer {
                return Err(PrefixPrepareError::Limit(format!(
                    "dispatch cell [{state}][{opcode}] exceeds the configured integer bound {}",
                    limits.max_integer
                )));
            }
        }
    }
    Ok(())
}

/// Resolve the proven bytecode binding's initializer and return the immutable
/// initial image snapshot. Either the runtime-config artifact carries bytes
/// from the proven base64 decoder call whose span joins this exact declarator,
/// or the initializer is a bounded numeric-literal array.
fn extract_image(
    source: &str,
    config: &runtime_config::RuntimeConfig,
    read: &DispatchReadProof,
    limits: PrefixLimits,
) -> Result<(Vec<u8>, OperandSpan), PrefixPrepareError> {
    let allocator = Allocator::default();
    let program =
        crate::analysis::parse_program(&allocator, source).map_err(PrefixPrepareError::Parse)?;
    let (scoping, _nodes) = crate::analysis::analyze(&program);
    let mut finder = BytecodeDeclaratorFinder {
        target: read.bytecode,
        init_span: None,
        literal: None,
        declarations: 0,
    };
    finder.visit_program(&program);
    if finder.declarations != 1 {
        return Err(PrefixPrepareError::SourceProof(
            "bytecode image binding is not uniquely declared".into(),
        ));
    }
    let bytecode_span = OperandSpan {
        start: scoping.symbol_span(read.bytecode).start,
        end: scoping.symbol_span(read.bytecode).end,
    };
    let init_span = finder.init_span.ok_or(PrefixPrepareError::MissingFact(
        "bytecode image initializer",
    ))?;

    let mut decoder_artifacts = config
        .artifacts
        .iter()
        .filter(|artifact| {
            artifact.kind == ArtifactKind::JsBytecodeImage
                && artifact.bytes.is_some()
                && artifact.span.start == init_span.start
                && artifact.span.end == init_span.end
        })
        .collect::<Vec<_>>();
    if decoder_artifacts.len() > 1 {
        return Err(PrefixPrepareError::InvalidFact(
            "bytecode image decoder artifact is ambiguous".into(),
        ));
    }
    let bytes = match decoder_artifacts.pop() {
        Some(artifact) => {
            proof::require_decoder_call_binding(
                source,
                &artifact.span,
                read.bytecode,
                "bytecode image",
            )?;
            artifact.bytes.clone().expect("filtered on bytes")
        }
        None => finder.literal.ok_or(PrefixPrepareError::InvalidFact(
            "bytecode image is neither a proven decoder literal nor a bounded numeric array".into(),
        ))?,
    };
    if bytes.is_empty() {
        return Err(PrefixPrepareError::InvalidFact(
            "bytecode image is empty".into(),
        ));
    }
    if bytes.len() > limits.max_image_bytes {
        return Err(PrefixPrepareError::Limit(format!(
            "bytecode image has {} bytes, exceeding {}",
            bytes.len(),
            limits.max_image_bytes
        )));
    }
    Ok((bytes, bytecode_span))
}

struct BytecodeDeclaratorFinder {
    target: SymbolId,
    init_span: Option<Span>,
    literal: Option<Vec<u8>>,
    declarations: u32,
}

impl<'a> Visit<'a> for BytecodeDeclaratorFinder {
    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if let (BindingPattern::BindingIdentifier(identifier), Some(init)) =
            (&declarator.id, declarator.init.as_ref())
        {
            if identifier.symbol_id.get() == Some(self.target) {
                self.declarations += 1;
                self.init_span = Some(init.span());
                if let Expression::ArrayExpression(array) = init {
                    let mut bytes = Vec::with_capacity(array.elements.len());
                    let mut valid = true;
                    for element in &array.elements {
                        let Some(Expression::NumericLiteral(number)) = element.as_expression()
                        else {
                            valid = false;
                            break;
                        };
                        if !number.value.is_finite()
                            || number.value.fract() != 0.0
                            || number.value < 0.0
                            || number.value > 255.0
                        {
                            valid = false;
                            break;
                        }
                        bytes.push(number.value as u8);
                    }
                    if valid {
                        self.literal = Some(bytes);
                    }
                }
            }
        }
        walk::walk_variable_declarator(self, declarator);
    }
}
