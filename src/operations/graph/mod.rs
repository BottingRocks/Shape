//! Canonical per-handler semantic graph: individual micro-op occurrences.
//!
//! This is the micro-op-first foundation.  It does not replace the operation
//! catalogue (`catalogue.rs`) and it does not add a second, stringly-typed
//! operation vocabulary: every occurrence reuses the existing
//! [`RecipeOperation`] / [`SemanticValue`] / [`PrimitiveOperator`] payloads,
//! but gives each *individual* nested value and operation occurrence its own
//! identity, explicit input/result edges, source provenance and an ordered
//! effect slot.
//!
//! Design rules (see `docs/operations/operations.md`; the original compositional-handler
//! design audit and micro-op normalization spec are retained in Git history):
//!
//! * Occurrence ids are dense and unique **within one exact
//!   (source hash, table key, handler index) identity**.  They are provenance
//!   metadata, never semantic names and never helper names.
//! * Semantic family names ([`Occurrence::semantic_name`]) are meaningful and
//!   stable; counter-derived names such as `runLoop_7` are deliberately not
//!   produced here.
//! * Effectful and may-throw occurrences are an ordered chain, not a multiset
//!   of counters.  Reads record the write they observe so an unsafe reorder is
//!   detectable.
//! * Assignment/temporary dependencies are SSA-like: a [`OccurrenceOp::Bind`]
//!   or [`OccurrenceOp::Assign`] defines a new version, and every
//!   [`OccurrenceOp::TemporaryRef`] points at the exact version it observed, so
//!   a "before"/"after" read is unambiguous.
//!
//! Unknown/unsupported regions (loops, `try`/`catch`, calls, unproved
//! properties) stay visible with their source text and are never promoted to
//! executable primitives merely because they received an id.
//!
//! # Terminology
//!
//! [`HandlerIR`] (also [`HandlerIRReport`]) is the preferred public name for
//! this recovered per-handler micro-op program. [`HandlerGraph`] and
//! [`HandlerGraphReport`] are the underlying concrete types; the preferred
//! names alias those same types. A `HandlerIR` is the static, recovered program for one
//! selected handler and is reused across instruction occurrences.  It is not a
//! decoded instruction: the selected-handler occurrence, operand values and
//! dispatch PC/provenance belong to the dispatch record, not to a fabricated
//! IR field.
//!
//! # Authority boundaries
//!
//! [`HandlerGraph::validate`] proves *structural consistency* and, when the
//! immutable standardized source is attached, *source-coordinate ownership*
//! (the hash/length plus UTF-8 span bounds and retained region text).  It does
//! not re-derive the graph from the AST, so it is not proof of authentic AST
//! correspondence against a caller that deliberately rewrites every public
//! field; the extractor's own builder is the source of that correspondence.
//! Exact source provenance is therefore not AST authenticity.
//! Validation is also not runtime admission: unsupported regions, operands,
//! context state and resource limits remain execution-time concerns.  The
//! legacy [`crate::operations::templates::HandlerTemplate`] projection is not claimed to be
//! one-to-one with graph nodes and is never executable without
//! [`crate::operations::templates::admit_recipe`].
//!
//! Source storage is shared: a [`HandlerGraphReport`] and every graph extracted
//! from the same report hold the same immutable [`Arc`]-backed source, and the
//! semantic source hash is computed once per extraction.  Public projection is
//! fallible and bounded; caller-supplied graphs must be validated first, while
//! the extractor uses a crate-private trusted projection over its own builder.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use oxc_syntax::number::ToJsString;
use sha2::{Digest, Sha256};

use crate::transforms::deob::pipeline::ProvenOperationTable;

use crate::operations::templates::{
    CallFrameField, LiteralValue, PrimitiveOperator, ReceiverField, RecipeOperation, RecipeStep,
    SemanticValue, SpanRecord, TemplateStatus, UnknownFragment,
};

/// Version of the graph schema.  Analytics/caches must key on this together
/// with [`PRIMITIVE_REGISTRY_VERSION`].
mod build;
mod types;

pub(crate) use build::*;
use build::{
    latest_write_for, locations_match, operation_signature, record_slot_write, record_stack_write,
    render_location,
};
pub use types::*;
use types::{take_projected_value, ProjectionMode};

impl HandlerGraph {
    /// Dense occurrence-slot lookup; panics only on a non-dense/out-of-range id,
    /// which [`Self::validate`] rejects first.
    pub fn occurrence(&self, id: OccurrenceId) -> &Occurrence {
        &self.occurrences[id as usize]
    }

    /// Fallibly reconstruct the legacy [`SemanticValue`] tree rooted at `id`.
    ///
    /// The build is iterative and bounded: shared operand edges are projected
    /// by cloning, and an external graph whose value DAG describes a deeper or
    /// larger tree than [`MAX_PROJECTED_VALUE_DEPTH`] /
    /// [`MAX_PROJECTED_VALUE_NODES`] fails closed instead of risking recursive
    /// drop/clone overflow.  [`Self::validate`] applies the same bounds.
    pub fn try_value(
        &self,
        id: OccurrenceId,
    ) -> Result<crate::operations::templates::SemanticValue, GraphProjectionError> {
        self.project_value(id, ProjectionMode::Bounded)
    }

    /// Trusted projection used only for graphs produced by the shared
    /// extractor walk.  It preserves legacy extraction for unusually deep but
    /// valid source rather than imposing an artificial source limit.
    pub(crate) fn try_value_trusted(
        &self,
        id: OccurrenceId,
    ) -> Result<crate::operations::templates::SemanticValue, GraphProjectionError> {
        self.project_value(id, ProjectionMode::Trusted)
    }

    fn project_value(
        &self,
        id: OccurrenceId,
        mode: ProjectionMode,
    ) -> Result<crate::operations::templates::SemanticValue, GraphProjectionError> {
        use crate::operations::templates::SemanticValue;
        let Some(root) = self.occurrences.get(id as usize) else {
            return Err(GraphProjectionError::InvalidId(id));
        };
        if !root.op.is_value() {
            return Err(GraphProjectionError::NotValue(id));
        }

        // Reachable value occurrences.  Temporary references are leaves even
        // though their version edge names the defining operation.
        let mut needed = std::collections::BTreeSet::new();
        let mut pending = vec![id];
        while let Some(current) = pending.pop() {
            if !needed.insert(current) {
                continue;
            }
            let Some(occurrence) = self.occurrences.get(current as usize) else {
                return Err(GraphProjectionError::InvalidId(current));
            };
            if !occurrence.op.is_value() {
                return Err(GraphProjectionError::NotValue(current));
            }
            pending.extend(occurrence.op.value_operands());
        }

        // Compute projected depth, node count and per-operand use count before
        // materializing anything.  A malformed DAG therefore fails closed
        // before a large tree exists to drop recursively.
        let mut depth = BTreeMap::<OccurrenceId, u32>::new();
        let mut nodes = BTreeMap::<OccurrenceId, u64>::new();
        let mut remaining = BTreeMap::<OccurrenceId, u32>::new();
        for &current in &needed {
            let Some(occurrence) = self.occurrences.get(current as usize) else {
                return Err(GraphProjectionError::InvalidId(current));
            };
            let mut value_depth = 1u32;
            let mut value_nodes = 1u64;
            for operand in occurrence.op.value_operands() {
                if operand >= current {
                    return Err(GraphProjectionError::Cycle(operand));
                }
                let (Some(&operand_depth), Some(&operand_nodes)) =
                    (depth.get(&operand), nodes.get(&operand))
                else {
                    return Err(GraphProjectionError::InvalidGraph(
                        GraphValidationError::InvalidGraphForProjection,
                    ));
                };
                value_depth = value_depth.max(operand_depth.saturating_add(1));
                value_nodes = value_nodes.saturating_add(operand_nodes);
                *remaining.entry(operand).or_insert(0) += 1;
            }
            if mode == ProjectionMode::Bounded {
                if value_depth > MAX_PROJECTED_VALUE_DEPTH {
                    return Err(GraphProjectionError::ValueTooDeep {
                        occurrence: current,
                        depth: value_depth,
                    });
                }
                if value_nodes > MAX_PROJECTED_VALUE_NODES {
                    return Err(GraphProjectionError::ValueTooComplex {
                        occurrence: current,
                        nodes: value_nodes,
                    });
                }
            }
            depth.insert(current, value_depth);
            nodes.insert(current, value_nodes);
        }

        let mut values = BTreeMap::<OccurrenceId, SemanticValue>::new();
        for current in needed {
            let Some(occurrence) = self.occurrences.get(current as usize) else {
                return Err(GraphProjectionError::InvalidId(current));
            };
            if !occurrence.op.is_value() {
                return Err(GraphProjectionError::NotValue(current));
            }
            let mut take = |operand| take_projected_value(&mut values, &mut remaining, operand);
            let value = match &occurrence.op {
                OccurrenceOp::Context => SemanticValue::Context,
                OccurrenceOp::Operand { position } => SemanticValue::Operand {
                    position: *position,
                },
                OccurrenceOp::Literal(value) => SemanticValue::Literal(value.clone()),
                OccurrenceOp::StackLength => SemanticValue::StackLength,
                OccurrenceOp::StackRead { index } => SemanticValue::StackRead {
                    index: Box::new(take(*index)?),
                },
                OccurrenceOp::MemoryRead { slot } => SemanticValue::MemoryRead {
                    slot: Box::new(take(*slot)?),
                },
                OccurrenceOp::TemporaryRef { temporary, .. } => {
                    SemanticValue::Temporary { id: *temporary }
                }
                OccurrenceOp::Binary {
                    operator,
                    left,
                    right,
                } => SemanticValue::Binary {
                    operator: *operator,
                    left: Box::new(take(*left)?),
                    right: Box::new(take(*right)?),
                },
                OccurrenceOp::Unary { operator, value } => SemanticValue::Unary {
                    operator: *operator,
                    value: Box::new(take(*value)?),
                },
                OccurrenceOp::UnknownValue { reason } => SemanticValue::Unknown {
                    span: occurrence.span.clone(),
                    reason: reason.clone(),
                },
                OccurrenceOp::Binding { name } => SemanticValue::Binding { name: name.clone() },
                OccurrenceOp::PropertyRead { object, key } => SemanticValue::PropertyRead {
                    object: Box::new(take(*object)?),
                    key: Box::new(take(*key)?),
                },
                OccurrenceOp::ObjectLiteral { keys, values } => SemanticValue::ObjectLiteral {
                    properties: keys
                        .iter()
                        .zip(values)
                        .map(|(key, value)| Ok((key.clone(), take(*value)?)))
                        .collect::<Result<_, GraphProjectionError>>()?,
                },
                OccurrenceOp::PoppedCallFrame { field } => {
                    SemanticValue::PoppedCallFrame { field: *field }
                }
                OccurrenceOp::Receiver { field } => SemanticValue::Receiver { field: *field },
                OccurrenceOp::ExceptionRead { value } => value.clone(),
                OccurrenceOp::FunctionLiteral { source } => SemanticValue::FunctionLiteral {
                    source: source.clone(),
                },
                OccurrenceOp::Construct { callee, arguments } => SemanticValue::Construct {
                    callee: Box::new(take(*callee)?),
                    arguments: Box::new(take(*arguments)?),
                },
                OccurrenceOp::Delete { object, key } => SemanticValue::Delete {
                    object: Box::new(take(*object)?),
                    key: Box::new(take(*key)?),
                },
                OccurrenceOp::StackSlice { start } => SemanticValue::StackSlice {
                    start: Box::new(take(*start)?),
                },
                OccurrenceOp::Call {
                    callee,
                    this_value,
                    arguments,
                } => SemanticValue::Call {
                    callee: Box::new(take(*callee)?),
                    this_value: Box::new(take(*this_value)?),
                    arguments: Box::new(take(*arguments)?),
                },
                OccurrenceOp::CreateThread {
                    descriptor,
                    byte_offset,
                    dispatch_state,
                } => SemanticValue::CreateThread {
                    descriptor: Box::new(take(*descriptor)?),
                    byte_offset: Box::new(take(*byte_offset)?),
                    dispatch_state: Box::new(take(*dispatch_state)?),
                },
                OccurrenceOp::ArrayLiteral { elements } => SemanticValue::ArrayLiteral {
                    elements: elements
                        .iter()
                        .map(|element| take(*element))
                        .collect::<Result<_, _>>()?,
                },
                _ => return Err(GraphProjectionError::NotValue(current)),
            };
            values.insert(current, value);
        }
        values
            .remove(&id)
            .ok_or(GraphProjectionError::InvalidId(id))
    }

    fn operation_step(
        &self,
        occurrence: &Occurrence,
        mode: ProjectionMode,
    ) -> Result<RecipeStep, GraphProjectionError> {
        let value = |id| match mode {
            ProjectionMode::Bounded => self.try_value(id),
            ProjectionMode::Trusted => self.try_value_trusted(id),
        };
        let operation = match &occurrence.op {
            OccurrenceOp::Bind {
                temporary,
                value: id,
            } => RecipeOperation::Let {
                temporary: *temporary,
                value: value(*id)?,
            },
            OccurrenceOp::Assign {
                temporary,
                value: id,
            } => RecipeOperation::Assign {
                temporary: *temporary,
                value: value(*id)?,
            },
            OccurrenceOp::StackWrite { index, value: id } => RecipeOperation::StackWrite {
                index: value(*index)?,
                value: value(*id)?,
            },
            OccurrenceOp::StackLengthSet { value: id } => {
                RecipeOperation::StackLengthSet { value: value(*id)? }
            }
            OccurrenceOp::MemoryWrite { slot, value: id } => RecipeOperation::MemoryWrite {
                slot: value(*slot)?,
                value: value(*id)?,
            },
            OccurrenceOp::MemoryInitialize { slot } => RecipeOperation::MemoryInitialize {
                slot: value(*slot)?,
            },
            OccurrenceOp::PropertyWrite {
                object,
                key,
                value: id,
            } => RecipeOperation::PropertyWrite {
                object: value(*object)?,
                key: value(*key)?,
                value: value(*id)?,
            },
            OccurrenceOp::PushTryFrame {
                byte_offset,
                dispatch_state,
            } => RecipeOperation::PushTryFrame {
                byte_offset: value(*byte_offset)?,
                dispatch_state: value(*dispatch_state)?,
            },
            OccurrenceOp::ExceptionStep { operation } => operation.clone(),
            OccurrenceOp::PushCallFrame => RecipeOperation::PushCallFrame,
            OccurrenceOp::PopCallFrame => RecipeOperation::PopCallFrame,
            OccurrenceOp::SetProgramCounter { value: id } => {
                RecipeOperation::SetProgramCounter { value: value(*id)? }
            }
            OccurrenceOp::SetDispatchState { value: id } => {
                RecipeOperation::SetDispatchState { value: value(*id)? }
            }
            OccurrenceOp::CompleteThread { value: id } => {
                RecipeOperation::CompleteThread { value: value(*id)? }
            }
            OccurrenceOp::JumpIf {
                condition,
                byte_offset,
                dispatch_state,
            } => RecipeOperation::JumpIf {
                condition: value(*condition)?,
                byte_offset: value(*byte_offset)?,
                dispatch_state: value(*dispatch_state)?,
            },
            OccurrenceOp::Evaluate { value: id } => {
                RecipeOperation::Evaluate { value: value(*id)? }
            }
            OccurrenceOp::Return { value: id } => RecipeOperation::Return { value: value(*id)? },
            OccurrenceOp::Throw { value: id } => RecipeOperation::Throw { value: value(*id)? },
            OccurrenceOp::Unknown { reason } => RecipeOperation::Unknown {
                reason: reason.clone(),
                source: occurrence.source.clone().unwrap_or_default(),
            },
            _ => unreachable!("occurrence {} is not an operation", occurrence.id),
        };
        Ok(RecipeStep {
            order: occurrence.step_order.unwrap_or(occurrence.id),
            span: occurrence.span.clone(),
            operation,
        })
    }

    fn project_steps_with(
        &self,
        mode: ProjectionMode,
    ) -> Result<Vec<RecipeStep>, GraphProjectionError> {
        self.occurrences
            .iter()
            .filter(|occurrence| occurrence.op.is_operation())
            .map(|occurrence| self.operation_step(occurrence, mode))
            .collect()
    }

    /// Lossless projection back to the legacy recipe steps.  This is the
    /// compatibility view; the occurrences remain the semantic foundation.
    pub fn try_project_steps(&self) -> Result<Vec<RecipeStep>, GraphProjectionError> {
        self.validate()
            .map_err(GraphProjectionError::InvalidGraph)?;
        self.project_steps_with(ProjectionMode::Bounded)
    }

    /// Trusted builder projection used by the extractor for the legacy
    /// template surface; the graph has already been produced by this walk.
    pub(crate) fn project_steps_trusted(&self) -> Result<Vec<RecipeStep>, GraphProjectionError> {
        self.project_steps_with(ProjectionMode::Trusted)
    }

    pub fn project_steps(&self) -> Vec<RecipeStep> {
        self.try_project_steps().unwrap_or_default()
    }

    /// Lossless projection of the retained unsupported fragments.
    pub fn project_unsupported(&self) -> Vec<UnknownFragment> {
        self.unknown_regions
            .iter()
            .map(|region| UnknownFragment {
                span: region.span.clone(),
                reason: region.reason.clone(),
                source: region.source.clone(),
            })
            .collect()
    }

    /// Structural admission status.  This deliberately does not materialize
    /// legacy trees: it is total for any occurrence list, so a graph that the
    /// builder could not have produced cannot turn an orphan `UnknownValue`
    /// into an executable claim.
    pub(crate) fn derived_status(&self) -> TemplateStatus {
        let mut supported_values = BTreeMap::<OccurrenceId, bool>::new();
        for occurrence in &self.occurrences {
            if occurrence.op.is_value() {
                let supported = match &occurrence.op {
                    OccurrenceOp::UnknownValue { .. } => false,
                    OccurrenceOp::TemporaryRef { .. } => true,
                    _ => occurrence
                        .op
                        .value_operands()
                        .into_iter()
                        .all(|input| supported_values.get(&input).copied().unwrap_or(false)),
                };
                supported_values.insert(occurrence.id, supported);
            }
        }
        let has_unknown_occurrence = self.occurrences.iter().any(|occurrence| {
            matches!(
                occurrence.op,
                OccurrenceOp::Unknown { .. } | OccurrenceOp::UnknownValue { .. }
            )
        });
        let operation_supported = |occurrence: &Occurrence| match &occurrence.op {
            OccurrenceOp::Unknown { .. } => false,
            _ => occurrence
                .op
                .operands()
                .into_iter()
                .all(|input| supported_values.get(&input).copied().unwrap_or(false)),
        };
        if self.unknown_regions.is_empty() && !has_unknown_occurrence {
            TemplateStatus::Executable
        } else if self
            .occurrences
            .iter()
            .filter(|occurrence| occurrence.op.is_operation())
            .any(operation_supported)
        {
            TemplateStatus::Partial
        } else {
            TemplateStatus::Unsupported
        }
    }

    pub fn is_executable(&self) -> bool {
        self.validate().is_ok() && self.derived_status() == TemplateStatus::Executable
    }

    pub fn project_status(&self) -> TemplateStatus {
        if self.validate().is_err() {
            TemplateStatus::Unsupported
        } else {
            self.derived_status()
        }
    }

    /// Validate all structural invariants.  Any error means this graph is not a
    /// trustworthy description and must not be treated as execution authority.
    ///
    /// This proves structural consistency and, when the standardized source is
    /// attached, source-coordinate ownership.  It is not a re-derivation from
    /// the AST and it is not runtime admission.
    pub fn validate(&self) -> Result<(), GraphValidationError> {
        self.validate_inner(false)
    }

    fn validate_inner(&self, source_verified: bool) -> Result<(), GraphValidationError> {
        if self.handler_span.start > self.handler_span.end {
            return Err(GraphValidationError::HandlerSpanInvalid);
        }
        if !source_verified {
            if let Some(source) = &self.source {
                if source.len() != self.source_length {
                    return Err(GraphValidationError::SourceLengthMismatch);
                }
                if semantic_source_hash(source.as_bytes()) != self.source_hash {
                    return Err(GraphValidationError::SourceHashMismatch);
                }
            }
        } else if let Some(source) = &self.source {
            if source.len() != self.source_length {
                return Err(GraphValidationError::SourceLengthMismatch);
            }
        }
        let source_limit = self.source_length;
        if self.handler_span.end as usize > source_limit {
            return Err(GraphValidationError::HandlerSpanOutOfBounds);
        }
        if let Some(source) = &self.source {
            if source
                .get(self.handler_span.start as usize..self.handler_span.end as usize)
                .is_none()
            {
                return Err(GraphValidationError::InvalidUtf8Boundary {
                    occurrence: None,
                    span: self.handler_span.clone(),
                });
            }
        }
        let mut latest_versions = BTreeMap::<u32, OccurrenceId>::new();
        let mut expected_step = 0u32;
        let mut unknown_occurrences = BTreeMap::<OccurrenceId, usize>::new();
        for (index, occurrence) in self.occurrences.iter().enumerate() {
            if occurrence.id as usize != index {
                return Err(GraphValidationError::NonDenseId {
                    index,
                    id: occurrence.id,
                });
            }
            if occurrence.span.start > occurrence.span.end {
                return Err(GraphValidationError::InvertedSpan {
                    occurrence: occurrence.id,
                    span: occurrence.span.clone(),
                });
            }
            if occurrence.span.start < self.handler_span.start
                || occurrence.span.end > self.handler_span.end
            {
                return Err(GraphValidationError::SpanOutOfBounds {
                    occurrence: Some(occurrence.id),
                    span: occurrence.span.clone(),
                });
            }
            if occurrence.span.end as usize > source_limit {
                return Err(GraphValidationError::SpanOutOfBounds {
                    occurrence: Some(occurrence.id),
                    span: occurrence.span.clone(),
                });
            }
            if let Some(source) = &self.source {
                if source
                    .get(occurrence.span.start as usize..occurrence.span.end as usize)
                    .is_none()
                {
                    return Err(GraphValidationError::InvalidUtf8Boundary {
                        occurrence: Some(occurrence.id),
                        span: occurrence.span.clone(),
                    });
                }
            }
            if occurrence.provenance.table_key != self.table_key
                || occurrence.provenance.handler_index != self.handler_index
                || occurrence.provenance.source_hash != self.source_hash
                || occurrence.provenance.raw_input_hash != self.raw_input_hash
                || occurrence.provenance.source_length != self.source_length
                || occurrence.provenance.table_declaration_ordinal != self.table_declaration_ordinal
                || occurrence.provenance.table_element_count != self.table_element_count
                || occurrence.provenance.table_function_count != self.table_function_count
                || occurrence.provenance.span != occurrence.span
            {
                return Err(GraphValidationError::ForeignProvenance {
                    occurrence: occurrence.id,
                });
            }
            for input in occurrence.op.operands() {
                if input as usize >= self.occurrences.len() {
                    return Err(GraphValidationError::DanglingInput {
                        occurrence: occurrence.id,
                        input,
                    });
                }
                if input >= occurrence.id {
                    return Err(GraphValidationError::UseBeforeDefinition {
                        occurrence: occurrence.id,
                        input,
                    });
                }
                let is_definition_edge = matches!(occurrence.op, OccurrenceOp::TemporaryRef { version, .. } if version == input);
                if !is_definition_edge && !self.occurrences[input as usize].op.is_value() {
                    return Err(GraphValidationError::WrongOperandKind {
                        occurrence: occurrence.id,
                        input,
                    });
                }
            }
            if let OccurrenceOp::TemporaryRef { temporary, version } = &occurrence.op {
                let Some(version_occurrence) = self.occurrences.get(*version as usize) else {
                    return Err(GraphValidationError::InvalidTemporaryVersion {
                        occurrence: occurrence.id,
                        temporary: *temporary,
                        version: *version,
                    });
                };
                match version_occurrence.op.defines() {
                    Some(defined) if defined == *temporary => {}
                    _ => {
                        return Err(GraphValidationError::InvalidTemporaryVersion {
                            occurrence: occurrence.id,
                            temporary: *temporary,
                            version: *version,
                        })
                    }
                }
                if latest_versions.get(temporary).copied() != Some(*version) {
                    return Err(GraphValidationError::InvalidTemporaryVersion {
                        occurrence: occurrence.id,
                        temporary: *temporary,
                        version: *version,
                    });
                }
            }
            if occurrence.op.is_operation() {
                if occurrence.step_order != Some(expected_step) {
                    return Err(GraphValidationError::StepOrderMismatch {
                        occurrence: occurrence.id,
                        expected: expected_step,
                        actual: occurrence.step_order,
                    });
                }
                expected_step += 1;
            } else if occurrence.step_order.is_some() {
                return Err(GraphValidationError::StepOrderMismatch {
                    occurrence: occurrence.id,
                    expected: expected_step,
                    actual: occurrence.step_order,
                });
            }
            if let Some((kind, may_throw, location)) = self.expected_effect(occurrence) {
                let Some(effect) = occurrence.effect else {
                    return Err(GraphValidationError::MissingEffect {
                        occurrence: occurrence.id,
                    });
                };
                if let Some(entry) = self.effects.get(effect as usize) {
                    if entry.kind != kind
                        || entry.may_throw != may_throw
                        || entry.location != location
                    {
                        return Err(GraphValidationError::IncorrectEffect { effect });
                    }
                }
            } else if let Some(effect) = occurrence.effect {
                return Err(GraphValidationError::UnexpectedEffect {
                    occurrence: occurrence.id,
                    effect,
                });
            }
            if matches!(occurrence.op, OccurrenceOp::Unknown { .. }) {
                unknown_occurrences.insert(occurrence.id, 0);
            }
            if let Some(effect) = occurrence.effect {
                match self.effects.get(effect as usize) {
                    Some(entry) if entry.occurrence == occurrence.id => {}
                    _ => {
                        return Err(GraphValidationError::EffectLinkMismatch {
                            occurrence: occurrence.id,
                            effect,
                        })
                    }
                }
            }
            if let Some(temporary) = occurrence.op.defines() {
                latest_versions.insert(temporary, occurrence.id);
            }
        }

        let mut previous: Option<OccurrenceId> = None;
        for (index, effect) in self.effects.iter().enumerate() {
            if effect.id as usize != index {
                return Err(GraphValidationError::NonDenseEffectId {
                    index,
                    id: effect.id,
                });
            }
            if effect.occurrence as usize >= self.occurrences.len() {
                return Err(GraphValidationError::DanglingEffect {
                    effect: effect.id,
                    occurrence: effect.occurrence,
                });
            }
            if self.occurrences[effect.occurrence as usize].effect != Some(effect.id) {
                return Err(GraphValidationError::EffectLinkMismatch {
                    occurrence: effect.occurrence,
                    effect: effect.id,
                });
            }
            let occurrence = &self.occurrences[effect.occurrence as usize];
            let Some((kind, may_throw, location)) = self.expected_effect(occurrence) else {
                return Err(GraphValidationError::IncorrectEffect { effect: effect.id });
            };
            if effect.kind != kind || effect.may_throw != may_throw || effect.location != location {
                return Err(GraphValidationError::IncorrectEffect { effect: effect.id });
            }
            if let Some(previous) = previous {
                if effect.occurrence <= previous {
                    return Err(GraphValidationError::NonMonotonicEffect {
                        effect: effect.id,
                        previous,
                        occurrence: effect.occurrence,
                    });
                }
            }
            if let Some(observed) = effect.observed_write {
                let write = self.occurrences.get(observed as usize);
                let is_write = write.is_some_and(|write| {
                    matches!(
                        write.op,
                        OccurrenceOp::StackWrite { .. }
                            | OccurrenceOp::StackLengthSet { .. }
                            | OccurrenceOp::MemoryWrite { .. }
                            | OccurrenceOp::MemoryInitialize { .. }
                    )
                });
                if observed >= effect.occurrence || !is_write {
                    return Err(GraphValidationError::UnsafeEffectReorder {
                        effect: effect.id,
                        read: effect.occurrence,
                        observed_write: observed,
                    });
                }
                let valid_observation = match effect.kind {
                    EffectKind::StackLengthRead => matches!(
                        write.map(|entry| &entry.op),
                        Some(OccurrenceOp::StackWrite { .. })
                            | Some(OccurrenceOp::StackLengthSet { .. })
                    ),
                    EffectKind::StackRead => {
                        matches!(
                            write.map(|entry| &entry.op),
                            Some(OccurrenceOp::StackWrite { .. })
                        ) && locations_match(
                            write.and_then(|entry| self.location_for(entry)),
                            effect.location.clone(),
                        )
                    }
                    EffectKind::MemoryRead => {
                        matches!(
                            write.map(|entry| &entry.op),
                            Some(OccurrenceOp::MemoryWrite { .. })
                                | Some(OccurrenceOp::MemoryInitialize { .. })
                        ) && locations_match(
                            write.and_then(|entry| self.location_for(entry)),
                            effect.location.clone(),
                        )
                    }
                    _ => false,
                };
                if !valid_observation {
                    return Err(GraphValidationError::IncorrectEffect { effect: effect.id });
                }
            } else if matches!(
                effect.kind,
                EffectKind::StackLengthRead | EffectKind::StackRead | EffectKind::MemoryRead
            ) {
                // None is valid for a read when no statically provable write
                // exists. It is deliberately not inferred for dynamic aliases.
            } else if effect.observed_write.is_some() {
                return Err(GraphValidationError::IncorrectEffect { effect: effect.id });
            }
            previous = Some(effect.occurrence);
        }

        // A read-from witness must be the *latest* write the builder's
        // conservative alias model would retain, not merely some earlier
        // same-location write.  Replaying the walk rejects a witness that an
        // intervening potentially-aliasing write or unknown region invalidated.
        self.validate_read_witnesses()?;

        for (region, entry) in self.unknown_regions.iter().enumerate() {
            if entry.occurrence as usize >= self.occurrences.len() {
                return Err(GraphValidationError::DanglingUnknownRegion {
                    region,
                    occurrence: entry.occurrence,
                });
            }
            let occurrence = &self.occurrences[entry.occurrence as usize];
            let OccurrenceOp::Unknown { reason } = &occurrence.op else {
                return Err(GraphValidationError::UnknownRegionMismatch { region });
            };
            if occurrence.span != entry.span
                || occurrence.source.as_deref() != Some(entry.source.as_str())
                || reason != &entry.reason
            {
                return Err(GraphValidationError::UnknownRegionMismatch { region });
            }
            if let Some(slot) = unknown_occurrences.get_mut(&entry.occurrence) {
                if *slot != 0 {
                    return Err(GraphValidationError::UnknownRegionMismatch { region });
                }
                *slot = region + 1;
            } else {
                return Err(GraphValidationError::UnknownRegionMismatch { region });
            }
            if entry.span.start < self.handler_span.start
                || entry.span.end > self.handler_span.end
                || entry.span.end as usize > source_limit
            {
                return Err(GraphValidationError::SpanOutOfBounds {
                    occurrence: Some(entry.occurrence),
                    span: entry.span.clone(),
                });
            }
            if let Some(source) = &self.source {
                if source
                    .get(entry.span.start as usize..entry.span.end as usize)
                    .is_none()
                    || source.get(entry.span.start as usize..entry.span.end as usize)
                        != Some(entry.source.as_str())
                {
                    return Err(GraphValidationError::InvalidUtf8Boundary {
                        occurrence: Some(entry.occurrence),
                        span: entry.span.clone(),
                    });
                }
            }
        }
        if unknown_occurrences.values().any(|slot| *slot == 0) {
            let occurrence = *unknown_occurrences
                .iter()
                .find_map(|(id, slot)| (*slot == 0).then_some(id))
                .expect("checked non-empty unknown occurrence map");
            return Err(GraphValidationError::UnknownRegionMissing { occurrence });
        }
        // An `UnknownValue` is meaningful only with the retained unknown region
        // that the extractor emits beside it.  If that region/occurrence is
        // removed, the remaining value must not silently support a complete or
        // executable claim.
        for occurrence in &self.occurrences {
            if matches!(occurrence.op, OccurrenceOp::UnknownValue { .. })
                && !self
                    .unknown_regions
                    .iter()
                    .any(|region| region.span == occurrence.span)
            {
                return Err(GraphValidationError::OrphanUnknownValue {
                    occurrence: occurrence.id,
                });
            }
        }
        self.validate_value_projection_bounds()?;
        let expected_status = self.derived_status();
        if self.status != expected_status {
            return Err(GraphValidationError::StatusMismatch {
                expected: expected_status,
                actual: self.status,
            });
        }
        Ok(())
    }

    fn validate_read_witnesses(&self) -> Result<(), GraphValidationError> {
        let mut stack_writes = BTreeMap::<LocationKey, OccurrenceId>::new();
        let mut slot_writes = BTreeMap::<LocationKey, OccurrenceId>::new();
        for occurrence in &self.occurrences {
            match &occurrence.op {
                OccurrenceOp::StackWrite { .. } => {
                    record_stack_write(
                        &mut stack_writes,
                        self.location_for(occurrence),
                        occurrence.id,
                    );
                }
                OccurrenceOp::StackLengthSet { .. } => stack_writes.clear(),
                OccurrenceOp::MemoryWrite { .. } | OccurrenceOp::MemoryInitialize { .. } => {
                    record_slot_write(
                        &mut slot_writes,
                        self.location_for(occurrence),
                        occurrence.id,
                    );
                }
                OccurrenceOp::Unknown { .. } => {
                    stack_writes.clear();
                    slot_writes.clear();
                }
                OccurrenceOp::StackLength
                | OccurrenceOp::StackRead { .. }
                | OccurrenceOp::MemoryRead { .. } => {
                    let expected = match occurrence.op {
                        OccurrenceOp::StackLength => None,
                        OccurrenceOp::StackRead { .. } => self
                            .location_for(occurrence)
                            .as_ref()
                            .and_then(|location| latest_write_for(&stack_writes, location)),
                        OccurrenceOp::MemoryRead { .. } => self
                            .location_for(occurrence)
                            .as_ref()
                            .and_then(|location| latest_write_for(&slot_writes, location)),
                        _ => None,
                    };
                    let Some(effect) = occurrence
                        .effect
                        .and_then(|id| self.effects.get(id as usize))
                    else {
                        continue;
                    };
                    if effect.observed_write != expected {
                        return Err(GraphValidationError::StaleEffectWitness {
                            effect: effect.id,
                            expected,
                            actual: effect.observed_write,
                        });
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// The public projection and [`Self::validate`] share the same bounded
    /// cost model so a graph that validates can be projected without a
    /// silently-different result.
    fn validate_value_projection_bounds(&self) -> Result<(), GraphValidationError> {
        let mut depth = BTreeMap::<OccurrenceId, u32>::new();
        let mut nodes = BTreeMap::<OccurrenceId, u64>::new();
        for occurrence in &self.occurrences {
            if !occurrence.op.is_value() {
                continue;
            }
            let mut value_depth = 1u32;
            let mut value_nodes = 1u64;
            for operand in occurrence.op.value_operands() {
                let (Some(&operand_depth), Some(&operand_nodes)) =
                    (depth.get(&operand), nodes.get(&operand))
                else {
                    return Err(GraphValidationError::InvalidGraphForProjection);
                };
                value_depth = value_depth.max(operand_depth.saturating_add(1));
                value_nodes = value_nodes.saturating_add(operand_nodes);
            }
            if value_depth > MAX_PROJECTED_VALUE_DEPTH {
                return Err(GraphValidationError::ValueProjectionTooDeep {
                    occurrence: occurrence.id,
                    depth: value_depth,
                });
            }
            if value_nodes > MAX_PROJECTED_VALUE_NODES {
                return Err(GraphValidationError::ValueProjectionTooComplex {
                    occurrence: occurrence.id,
                    nodes: value_nodes,
                });
            }
            depth.insert(occurrence.id, value_depth);
            nodes.insert(occurrence.id, value_nodes);
        }
        Ok(())
    }

    fn location_for(&self, occurrence: &Occurrence) -> Option<LocationKey> {
        let id = match occurrence.op {
            OccurrenceOp::StackRead { index }
            | OccurrenceOp::MemoryRead { slot: index }
            | OccurrenceOp::StackWrite { index, .. }
            | OccurrenceOp::MemoryWrite { slot: index, .. } => index,
            OccurrenceOp::MemoryInitialize { slot } => slot,
            _ => return None,
        };
        match self.occurrences.get(id as usize).map(|entry| &entry.op) {
            Some(OccurrenceOp::Literal(LiteralValue::DecodedNumber { bits })) => {
                Some(LocationKey::NumberBits(*bits))
            }
            Some(OccurrenceOp::Literal(LiteralValue::String(value))) => {
                Some(LocationKey::String(value.clone()))
            }
            Some(OccurrenceOp::Literal(LiteralValue::Boolean(value))) => {
                Some(LocationKey::Boolean(*value))
            }
            Some(OccurrenceOp::Literal(LiteralValue::Null)) => Some(LocationKey::Null),
            Some(OccurrenceOp::Literal(LiteralValue::Undefined)) => Some(LocationKey::Undefined),
            _ => None,
        }
    }

    fn expected_effect(
        &self,
        occurrence: &Occurrence,
    ) -> Option<(EffectKind, bool, Option<LocationKey>)> {
        match occurrence.op {
            OccurrenceOp::StackLength => Some((
                EffectKind::StackLengthRead,
                false,
                Some(LocationKey::StackLength),
            )),
            OccurrenceOp::StackRead { .. } => {
                Some((EffectKind::StackRead, false, self.location_for(occurrence)))
            }
            OccurrenceOp::MemoryRead { .. } => {
                Some((EffectKind::MemoryRead, true, self.location_for(occurrence)))
            }
            OccurrenceOp::StackWrite { .. } => {
                Some((EffectKind::StackWrite, false, self.location_for(occurrence)))
            }
            OccurrenceOp::StackLengthSet { .. } => Some((
                EffectKind::StackLengthWrite,
                false,
                Some(LocationKey::StackLength),
            )),
            OccurrenceOp::MemoryWrite { .. } => Some((
                EffectKind::MemoryWrite,
                false,
                self.location_for(occurrence),
            )),
            OccurrenceOp::MemoryInitialize { .. } => Some((
                EffectKind::MemoryInitialize,
                false,
                self.location_for(occurrence),
            )),
            OccurrenceOp::PropertyWrite { .. } => Some((EffectKind::PropertyWrite, false, None)),
            OccurrenceOp::PushTryFrame { .. } | OccurrenceOp::ExceptionRead { .. } => {
                Some((EffectKind::ControlTransfer, false, None))
            }
            OccurrenceOp::ExceptionStep {
                operation: RecipeOperation::RethrowIfHandled,
            } => Some((EffectKind::Throw, false, None)),
            OccurrenceOp::ExceptionStep { .. } => Some((EffectKind::ControlTransfer, false, None)),
            OccurrenceOp::PushCallFrame => Some((EffectKind::CallFramePush, false, None)),
            OccurrenceOp::PopCallFrame => Some((EffectKind::CallFramePop, false, None)),
            OccurrenceOp::SetProgramCounter { .. }
            | OccurrenceOp::SetDispatchState { .. }
            | OccurrenceOp::JumpIf { .. } => Some((EffectKind::ControlTransfer, false, None)),
            OccurrenceOp::StackSlice { .. } => Some((EffectKind::StackRangeRead, false, None)),
            OccurrenceOp::Call { .. }
            | OccurrenceOp::Construct { .. }
            | OccurrenceOp::Delete { .. } => Some((EffectKind::ProgramCall, true, None)),
            OccurrenceOp::CreateThread { .. } => Some((EffectKind::ThreadCapture, false, None)),
            OccurrenceOp::Evaluate { .. } => Some((EffectKind::Evaluate, false, None)),
            OccurrenceOp::Return { .. } | OccurrenceOp::CompleteThread { .. } => {
                Some((EffectKind::Return, false, None))
            }
            OccurrenceOp::Throw { .. } => Some((EffectKind::Throw, false, None)),
            OccurrenceOp::Unknown { .. } => Some((EffectKind::Unknown, false, None)),
            _ => None,
        }
    }

    /// Alpha-renaming-invariant signature for *comparison and diagnostics only*.
    ///
    /// It is never identity, never admission, and never execution authority.
    /// This is deliberately conservative: every definition/use edge remains in
    /// the signature. In particular, captured reads are never inlined away and
    /// temporary aliases are assigned at definitions, not first use.
    pub fn canonical_signature(&self) -> String {
        if self.validate().is_err() {
            return "invalid-graph".into();
        }
        let mut temporary_names = BTreeMap::<u32, u32>::new();
        let mut next_temporary = 0u32;
        let mut tokens = Vec::with_capacity(self.occurrences.len() + self.effects.len());
        for occurrence in &self.occurrences {
            let op = operation_signature(&occurrence.op, &mut temporary_names, &mut next_temporary);
            let inputs = occurrence
                .op
                .operands()
                .into_iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let effect = occurrence
                .effect
                .map(|id| format!(" effect#{id}"))
                .unwrap_or_default();
            let source = occurrence
                .source
                .as_deref()
                .map_or_else(String::new, |source| format!(" source={source:?}"));
            tokens.push(format!("{}[{}]{}{}", op, inputs, effect, source));
        }
        for effect in &self.effects {
            tokens.push(format!(
                "{}:{}:{}:{}:{}",
                effect.id,
                effect.kind.semantic_name(),
                effect.may_throw,
                effect
                    .observed_write
                    .map_or_else(|| "-".into(), |id| id.to_string()),
                effect
                    .location
                    .as_ref()
                    .map_or_else(|| "-".into(), render_location),
            ));
        }
        tokens.join(" ")
    }

    /// Human-readable node/dependency listing for inspection and the example.
    pub fn explain(&self) -> String {
        let mut out = format!(
            "handler#{} table#{} status={} span={}..{} occurrences={} effects={} unknown={}\n",
            self.handler_index,
            self.table_key,
            self.status,
            self.handler_span.start,
            self.handler_span.end,
            self.occurrences.len(),
            self.effects.len(),
            self.unknown_regions.len(),
        );
        for occurrence in &self.occurrences {
            let operands = occurrence
                .op
                .operands()
                .into_iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let mut line = format!(
                "  #{:<3} {:<26} span={}..{} {} inputs=[{}]",
                occurrence.id,
                occurrence.semantic_name(),
                occurrence.span.start,
                occurrence.span.end,
                match occurrence.granularity {
                    SpanGranularity::Expression => "expr",
                    SpanGranularity::Statement => "stmt",
                },
                operands,
            );
            if let Some(temporary) = occurrence.op.defines() {
                line.push_str(&format!(" defines=temporary#{temporary}"));
            }
            if let Some(effect) = occurrence.effect {
                let entry = &self.effects[effect as usize];
                line.push_str(&format!(
                    " effect={} may_throw={}",
                    entry.kind.semantic_name(),
                    entry.may_throw
                ));
                if let Some(observed) = entry.observed_write {
                    line.push_str(&format!(" observes=#{observed}"));
                }
            }
            if let Some(source) = &occurrence.source {
                line.push_str(&format!(" source={source:?}"));
            }
            line.push('\n');
            out.push_str(&line);
        }
        for region in &self.unknown_regions {
            out.push_str(&format!(
                "  ~unsupported {} span={}..{} reason={:?} source={:?}\n",
                region.kind.semantic_name(),
                region.span.start,
                region.span.end,
                region.reason,
                region.source
            ));
        }
        out
    }
}
