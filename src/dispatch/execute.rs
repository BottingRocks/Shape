//! Bounded dispatch execution: run the committed prefix once, threading a
//! single [`TraceSession`] so prior committed state/events are never replayed,
//! and map every halt to a typed stop reason that agrees with the tracer's
//! completion.

use std::collections::BTreeSet;

use crate::tracing::symbolic::VmCursor;

use crate::{
    CompletionValue, DispatchExecutionRecord, DispatchInvocation, DispatchPreparation,
    DispatchStopReason, DispatchTraceReport, TraceCompletion, TraceContext, TraceDebugOptions,
    TraceFinishError, TraceHandlerResult, TraceLimits, TraceProvenance, TraceSession,
    TraceStopCode,
};

use super::step::{self, StepFault};

impl DispatchPreparation {
    /// Run the bounded prefix without replaying prior handlers. One
    /// [`TraceSession`] owns the context and arena for the whole execution.
    #[must_use]
    pub fn execute(&self, context: TraceContext, trace_limits: TraceLimits) -> DispatchTraceReport {
        self.execute_with_debug(context, trace_limits, TraceDebugOptions::disabled())
    }

    /// Run the bounded prefix with live per-operation debug output.
    #[must_use]
    pub fn execute_with_debug(
        &self,
        context: TraceContext,
        trace_limits: TraceLimits,
        debug: TraceDebugOptions,
    ) -> DispatchTraceReport {
        self.with_entry(self.start)
            .expect("the prepared entry was validated during preparation")
            .execute_with_debug(context, trace_limits, debug)
    }
}

impl<'a> DispatchInvocation<'a> {
    /// Run from this validated entry with the caller-provided incoming VM
    /// context. The context is consumed into one persistent trace session and
    /// is never shared with later invocations.
    #[must_use]
    pub fn execute(&self, context: TraceContext, trace_limits: TraceLimits) -> DispatchTraceReport {
        self.execute_with_debug(context, trace_limits, TraceDebugOptions::disabled())
    }

    /// Run from this validated entry with live per-operation debug output.
    #[must_use]
    pub fn execute_with_debug(
        &self,
        context: TraceContext,
        trace_limits: TraceLimits,
        debug: TraceDebugOptions,
    ) -> DispatchTraceReport {
        let preparation = self.preparation;
        let limits = preparation.limits;
        let mut session = preparation.constant_tables.iter().fold(
            TraceSession::new_continuing(context, trace_limits).with_debug(debug),
            |session, (name, entries)| session.with_constant_table(name.clone(), entries.clone()),
        );
        let mut records = Vec::new();
        let mut visited = BTreeSet::new();
        let mut pc = self.start.pc;
        let mut state = self.start.state;
        let mut stop = DispatchStopReason::Budget;

        // Defense in depth: preparation already validated these bounds. A
        // violation here is a driver-level budget halt, never a silent success.
        let Some(table_cells) = preparation
            .table
            .iter()
            .try_fold(0usize, |total, row| total.checked_add(row.len()))
        else {
            session.stop_prefix(
                TraceStopCode::DispatchTableOverflow,
                "dispatch table cell count overflowed the address space",
                None,
            );
            return DispatchTraceReport {
                start: self.start,
                records,
                trace: session.report(),
                stop,
            };
        };
        if preparation.bytecode.len() > limits.max_image_bytes
            || table_cells > limits.max_table_cells
        {
            session.stop_prefix(
                TraceStopCode::DispatchPrepareBudget,
                "dispatch image or table exceeds the configured preparation bound",
                None,
            );
            return DispatchTraceReport {
                start: self.start,
                records,
                trace: session.report(),
                stop,
            };
        }

        for _ in 0..limits.max_dispatch_steps {
            let fetched = match step::fetch_step(
                &preparation.bytecode,
                &preparation.table,
                pc,
                state,
                session.call_frames(),
                &mut visited,
            ) {
                Ok(fetched) => fetched,
                Err(fault) => {
                    stop = halt(&mut session, &fault);
                    break;
                }
            };
            let opcode = fetched.opcode;
            let cell = fetched.cell;
            let next_state = fetched.next_state;
            session.debug_before_operation(Some(pc), Some(state), Some(opcode), cell.selector);

            if cell.selector == preparation.completion_selector() {
                let coordinates = match step::completion_coordinates(
                    pc,
                    preparation.bytecode.len(),
                    preparation.completion_span.clone(),
                ) {
                    Ok(coordinates) => coordinates,
                    Err(_) => {
                        stop = DispatchStopReason::TruncatedImage;
                        session.stop_prefix(
                            TraceStopCode::DispatchCompletionPcOverflow,
                            "completion PC increment overflowed the address space",
                            None,
                        );
                        break;
                    }
                };
                let provenance = preparation.completion_provenance(pc);
                let completion = preparation.completion.value();
                let finalized = match completion {
                    CompletionValue::Stack { index } => {
                        session.finalize_stack(index as usize, provenance.clone())
                    }
                    CompletionValue::Undefined => session.finalize_undefined(provenance.clone()),
                };
                match finalized {
                    Ok(_) => {
                        session.debug_after_operation(Some("completion"));
                        records.push(DispatchExecutionRecord {
                            dispatch_pc: pc,
                            operand_entry_pc: coordinates.operand_entry_pc,
                            state_before: state,
                            state_after: next_state,
                            opcode,
                            handler_key: cell.selector,
                            operands: Vec::new(),
                            source_span: coordinates.source_span.clone(),
                            recipe_identity: step::COMPLETION_RECIPE_IDENTITY.into(),
                            actual_next_pc: coordinates.actual_next_pc,
                            completion: Some(completion),
                        });
                        stop = DispatchStopReason::CompletedSelector;
                    }
                    Err(TraceFinishError::AlreadyStopped) => {
                        stop = existing_stop_reason(&session);
                    }
                    Err(error @ TraceFinishError::StackIndexOutOfRange { .. })
                    | Err(error @ TraceFinishError::ValueGraphUnavailable(_)) => {
                        stop = DispatchStopReason::Budget;
                        session.stop_prefix(
                            TraceStopCode::DispatchCompletionBudget,
                            error.to_string(),
                            Some(provenance),
                        );
                    }
                    Err(TraceFinishError::Unsupported { code, message }) => {
                        stop = DispatchStopReason::UnsupportedHandler;
                        session.stop_prefix(code, message, Some(provenance));
                    }
                }
                break;
            }

            let lifted = match step::find_operand_handler(
                &preparation.operands,
                &preparation.operand_index,
                cell.selector,
            ) {
                Ok(lifted) => lifted,
                Err(fault) => {
                    stop = halt(&mut session, &fault);
                    break;
                }
            };
            let decoded = match step::decode_step(&preparation.bytecode, pc, lifted) {
                Ok(decoded) => decoded,
                Err(fault) => {
                    stop = halt(&mut session, &fault);
                    break;
                }
            };
            let step::Decoded {
                operand_entry_pc,
                operands: operands_decoded,
                actual_next_pc,
                instruction,
            } = decoded;
            // Valid preparations always contain the template. Resolve it through
            // the prepared index; a malformed clone keeps the tracer's defensive
            // missing-handler path by passing `None`.
            let handler = step::find_template(
                &preparation.templates,
                &preparation.template_index,
                cell.selector,
            );
            session.set_vm_cursor(VmCursor {
                byte_offset: actual_next_pc,
                dispatch_state: next_state,
            });
            match session.execute_handler_at(handler, &instruction) {
                TraceHandlerResult::Continued | TraceHandlerResult::Returned(_) => {
                    // A jump, call or return continues at the handler's
                    // assigned PC/state; otherwise the VM falls through.
                    let control = session.take_control_transfer();
                    let actual_next_pc = control.byte_offset.unwrap_or(actual_next_pc);
                    let next_state = control.dispatch_state.unwrap_or(next_state);
                    records.push(DispatchExecutionRecord {
                        dispatch_pc: pc,
                        operand_entry_pc,
                        state_before: state,
                        state_after: next_state,
                        opcode,
                        handler_key: cell.selector,
                        operands: operands_decoded,
                        source_span: lifted.source_span.clone(),
                        recipe_identity: lifted.template.semantic_identity.to_string(),
                        actual_next_pc,
                        completion: None,
                    });
                    pc = actual_next_pc;
                    state = next_state;
                }
                TraceHandlerResult::Thrown(_) => {
                    // The throw is committed: its effects and terminal record
                    // are retained and reported with the failed coordinates.
                    records.push(DispatchExecutionRecord {
                        dispatch_pc: pc,
                        operand_entry_pc,
                        state_before: state,
                        state_after: next_state,
                        opcode,
                        handler_key: cell.selector,
                        operands: operands_decoded,
                        source_span: lifted.source_span.clone(),
                        recipe_identity: lifted.template.semantic_identity.to_string(),
                        actual_next_pc,
                        completion: None,
                    });
                    stop = DispatchStopReason::Thrown;
                    break;
                }
                TraceHandlerResult::Partial => {
                    // The tracer already recorded the unsupported diagnostic and
                    // rolled back the failed attempt: no record is committed and
                    // the preceding prefix is intact.
                    stop = DispatchStopReason::UnsupportedHandler;
                    break;
                }
                TraceHandlerResult::BudgetStopped => {
                    stop = DispatchStopReason::Budget;
                    break;
                }
            }
        }
        if stop == DispatchStopReason::Budget
            && !session.is_stopped()
            && visited.len() == limits.max_dispatch_steps
        {
            session.stop_prefix(
                TraceStopCode::DispatchStepBudget,
                "dispatch step budget exhausted before a completion selector",
                None,
            );
        }
        DispatchTraceReport {
            start: self.start,
            records,
            trace: session.report(),
            stop,
        }
    }
}

impl DispatchPreparation {
    pub(super) fn completion_provenance(&self, pc: usize) -> TraceProvenance {
        TraceProvenance {
            handler_index: self.completion_selector(),
            table_key: self.completion_selector(),
            semantic_identity: step::COMPLETION_RECIPE_IDENTITY.into(),
            step_order: 0,
            trace_step: pc as u64,
            span: crate::operations::templates::SpanRecord {
                start: self.completion_span.start,
                end: self.completion_span.end,
            },
        }
    }
}

/// Record the diagnostic for a shared step fault and return the complete
/// driver's stop reason.
fn halt(session: &mut TraceSession, fault: &StepFault) -> DispatchStopReason {
    let (stop, code, message) = dispatch_fault(fault);
    session.stop_prefix(code, message, None);
    stop
}

/// Map a shared step fault to the complete driver's observable stop reason,
/// diagnostic code and message.
fn dispatch_fault(fault: &StepFault) -> (DispatchStopReason, TraceStopCode, String) {
    match fault {
        StepFault::Repeated { pc, state } => (
            DispatchStopReason::RepeatedPcState,
            TraceStopCode::DispatchRepeatedPcState,
            format!("dispatch revisited pc {pc} state {state} without progress"),
        ),
        StepFault::TruncatedImage { pc, image_len } => (
            DispatchStopReason::TruncatedImage,
            TraceStopCode::DispatchTruncatedImage,
            format!("dispatch pc {pc} is outside the {image_len} byte image"),
        ),
        StepFault::MissingState { state } => (
            DispatchStopReason::MissingState,
            TraceStopCode::DispatchMissingState,
            format!("dispatch state {state} has no proven table row"),
        ),
        StepFault::MissingOpcode { state, opcode } => (
            DispatchStopReason::MissingOpcode,
            TraceStopCode::DispatchMissingOpcode,
            format!("dispatch state {state} opcode {opcode} has no proven cell"),
        ),
        StepFault::InvalidNextState { next_state } => (
            DispatchStopReason::MissingState,
            TraceStopCode::DispatchInvalidNextState,
            format!("dispatch cell targets state {next_state} outside the proven table"),
        ),
        StepFault::PcOverflow { .. } => (
            DispatchStopReason::TruncatedImage,
            TraceStopCode::DispatchPcOverflow,
            "dispatch PC increment overflowed the address space".into(),
        ),
        StepFault::TruncatedOperands { .. } => (
            DispatchStopReason::TruncatedImage,
            TraceStopCode::DispatchTruncatedOperands,
            "operand image is truncated before the handler operands are complete".into(),
        ),
        StepFault::UnsupportedOperands { reason, .. } => (
            DispatchStopReason::UnsupportedHandler,
            TraceStopCode::DispatchUnsupportedOperands,
            reason.clone(),
        ),
        StepFault::NextPcOverflow { .. } => (
            DispatchStopReason::TruncatedImage,
            TraceStopCode::DispatchNextPcOverflow,
            "handler PC delta overflowed the address space".into(),
        ),
        StepFault::MissingOperandHandler { selector } => (
            DispatchStopReason::MissingHandler,
            TraceStopCode::DispatchMissingHandler,
            format!("selector {selector} has no operand-lifted handler"),
        ),
    }
}

/// Classify the tracer completion that prevented `finalize_*` from committing.
///
/// Caller audit: this function is reachable only from
/// [`TraceFinishError::AlreadyStopped`] at the completion selector. Every
/// in-loop `stop_prefix` call halts the loop immediately, and the handler
/// results that stop the session (`Partial` and `BudgetStopped`) also break.
/// The only remaining producer of a pre-stopped session is
/// [`TraceSession::new_continuing`]'s initial-context check, which sets exactly
/// `PartialUnsupported` (computed input) or `BudgetStopped` (stack or memory
/// bound). Driver `PrefixStopped` codes cannot reach here, so matching their
/// substrings was both unnecessary and wrong for codes such as
/// `"dispatch-invalid-next-state"`.
fn existing_stop_reason(session: &TraceSession) -> DispatchStopReason {
    match session.completion() {
        TraceCompletion::PartialUnsupported => DispatchStopReason::UnsupportedHandler,
        TraceCompletion::BudgetStopped => DispatchStopReason::Budget,
        other => {
            debug_assert!(
                false,
                "pre-existing trace completion {other:?} at the completion selector is unreachable"
            );
            DispatchStopReason::Budget
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispatch::step::SelectorIndex;
    use crate::{
        deobfuscate_with_report, prepare_dispatch_from_report, CompletionContract, DispatchStart,
        OperandHandlerStatus, TemplateStatus,
    };

    fn prepared_fixture() -> DispatchPreparation {
        let source = include_str!("../../tests/fixtures/dispatch-trace.raw.js");
        let report = deobfuscate_with_report(source).expect("fixture pipeline");
        prepare_dispatch_from_report(
            source,
            &report,
            DispatchStart { pc: 0, state: 0 },
            CompletionContract::SelectorSentinel {
                selector: 255,
                value: CompletionValue::Stack { index: 0 },
            },
            crate::DispatchLimits::default(),
        )
        .expect("proof-backed preparation")
    }

    fn execute_at(prepared: &DispatchPreparation, start: DispatchStart) -> DispatchTraceReport {
        DispatchInvocation {
            preparation: prepared,
            start,
        }
        .execute(TraceContext::new(), TraceLimits::default())
    }

    fn last_diagnostic(report: &DispatchTraceReport) -> (&str, &str) {
        let diagnostic = report
            .trace
            .diagnostics
            .last()
            .expect("a halted step records a diagnostic");
        (diagnostic.code.as_str(), diagnostic.message.as_str())
    }

    #[test]
    fn fetch_and_decode_faults_keep_their_stops_codes_and_messages() {
        let prepared = prepared_fixture();

        let mut truncated_image = prepared.clone();
        truncated_image
            .operands
            .handlers
            .iter_mut()
            .find(|handler| handler.handler_index == 0)
            .expect("handler 0")
            .pc_delta = 1_000;
        let report = execute_at(&truncated_image, DispatchStart { pc: 0, state: 0 });
        assert_eq!(report.stop, DispatchStopReason::TruncatedImage);
        assert_eq!(
            last_diagnostic(&report),
            (
                "dispatch-truncated-image",
                "dispatch pc 1001 is outside the 6 byte image"
            )
        );

        let mut missing_state = prepared.clone();
        let report = execute_at(&missing_state, DispatchStart { pc: 0, state: 9 });
        assert_eq!(report.stop, DispatchStopReason::MissingState);
        assert_eq!(
            last_diagnostic(&report),
            (
                "dispatch-missing-state",
                "dispatch state 9 has no proven table row"
            )
        );

        missing_state.bytecode[0] = 200;
        let report = execute_at(&missing_state, DispatchStart { pc: 0, state: 0 });
        assert_eq!(report.stop, DispatchStopReason::MissingOpcode);
        assert_eq!(
            last_diagnostic(&report),
            (
                "dispatch-missing-opcode",
                "dispatch state 0 opcode 200 has no proven cell"
            )
        );

        let mut invalid_next_state = prepared.clone();
        invalid_next_state.table[0][0].next_state = 9;
        let report = execute_at(&invalid_next_state, DispatchStart { pc: 0, state: 0 });
        assert_eq!(report.stop, DispatchStopReason::MissingState);
        assert_eq!(
            last_diagnostic(&report),
            (
                "dispatch-invalid-next-state",
                "dispatch cell targets state 9 outside the proven table"
            )
        );

        let mut repeated = prepared.clone();
        repeated.table[0][0].next_state = 0;
        let handler = repeated
            .operands
            .handlers
            .iter_mut()
            .find(|handler| handler.handler_index == 0)
            .expect("handler 0");
        handler.pc_delta = 0;
        handler.entry_coordinate = crate::OperandEntryCoordinate::AtOpcode;
        let report = execute_at(&repeated, DispatchStart { pc: 0, state: 0 });
        assert_eq!(report.stop, DispatchStopReason::RepeatedPcState);
        assert_eq!(
            last_diagnostic(&report),
            (
                "dispatch-repeated-pc-state",
                "dispatch revisited pc 0 state 0 without progress"
            )
        );

        let mut truncated_operands = prepared.clone();
        truncated_operands.bytecode.truncate(1);
        let report = execute_at(&truncated_operands, DispatchStart { pc: 0, state: 0 });
        assert_eq!(report.stop, DispatchStopReason::TruncatedImage);
        assert_eq!(
            last_diagnostic(&report),
            (
                "dispatch-truncated-operands",
                "operand image is truncated before the handler operands are complete"
            )
        );

        let mut missing_handler = prepared.clone();
        missing_handler.operands.handlers.clear();
        missing_handler.operand_index =
            SelectorIndex::from_handler_indices(std::iter::empty::<u32>());
        let report = execute_at(&missing_handler, DispatchStart { pc: 0, state: 0 });
        assert_eq!(report.stop, DispatchStopReason::MissingHandler);
        assert_eq!(
            last_diagnostic(&report),
            (
                "dispatch-missing-handler",
                "selector 0 has no operand-lifted handler"
            )
        );
    }

    #[test]
    fn completion_selector_precedes_operand_handler_lookup() {
        let mut prepared = prepared_fixture();
        prepared.operands.handlers.clear();
        prepared.operand_index = SelectorIndex::from_handler_indices(std::iter::empty::<u32>());
        prepared.completion = CompletionContract::SelectorSentinel {
            selector: 255,
            value: CompletionValue::Undefined,
        };
        let report = execute_at(&prepared, DispatchStart { pc: 5, state: 0 });
        assert_eq!(report.stop, DispatchStopReason::CompletedSelector);
        assert_eq!(report.records.len(), 1);
        assert_eq!(
            report.records[0].completion,
            Some(CompletionValue::Undefined)
        );
    }

    /// A valid preparation always contains the template and an executable
    /// merged template. The run loop does not re-check either condition; a
    /// malformed clone falls back to the tracer's defensive diagnostics and
    /// commits no record. The indexes are rebuilt after the malformed list edit
    /// so they stay consistent with the lists they describe.
    #[test]
    fn malformed_template_state_falls_back_to_the_tracer() {
        let mut missing_template = prepared_fixture();
        missing_template
            .templates
            .handlers
            .retain(|handler| handler.handler_index != 0);
        missing_template.template_index = SelectorIndex::from_handler_indices(
            missing_template
                .templates
                .handlers
                .iter()
                .map(|handler| handler.handler_index),
        );
        let report = execute_at(&missing_template, DispatchStart { pc: 0, state: 0 });
        assert_eq!(report.stop, DispatchStopReason::UnsupportedHandler);
        assert!(report.records.is_empty());
        assert_eq!(report.trace.completion, TraceCompletion::PartialUnsupported);
        assert_eq!(report.trace.handlers_traced, 0);
        assert_eq!(
            last_diagnostic(&report),
            (
                "missing-handler",
                "handler 0 is absent from the template report"
            )
        );

        let mut non_executable = prepared_fixture();
        non_executable
            .templates
            .handlers
            .iter_mut()
            .find(|handler| handler.handler_index == 0)
            .expect("handler 0")
            .status = TemplateStatus::Partial;
        let report = execute_at(&non_executable, DispatchStart { pc: 0, state: 0 });
        assert_eq!(report.stop, DispatchStopReason::UnsupportedHandler);
        assert!(report.records.is_empty());
        assert_eq!(report.trace.completion, TraceCompletion::PartialUnsupported);
        assert_eq!(report.trace.handlers_traced, 0);
        assert_eq!(
            last_diagnostic(&report),
            ("unsupported-handler", "handler status is partial")
        );
    }

    /// Operand status is preparation-owned. A public preparation can never be
    /// `Partial`, so the run loop does not re-check it; a malformed clone keeps
    /// the already-merged executable template and still traces to completion.
    /// This pins why the preparation check must not be removed.
    #[test]
    fn runtime_does_not_recheck_operand_handler_status() {
        let mut prepared = prepared_fixture();
        prepared
            .operands
            .handlers
            .iter_mut()
            .find(|handler| handler.handler_index == 0)
            .expect("handler 0")
            .status = OperandHandlerStatus::Partial;
        let report = execute_at(&prepared, DispatchStart { pc: 0, state: 0 });
        assert_eq!(report.stop, DispatchStopReason::CompletedSelector);
        assert_eq!(report.records.len(), 3);
    }

    /// Replace handler 0's merged template with `operations`, keeping its
    /// provenance, and complete with `undefined` so no stack value is needed.
    fn with_handler_zero(operations: Vec<crate::RecipeOperation>) -> DispatchPreparation {
        let mut prepared = prepared_fixture();
        prepared.completion = CompletionContract::SelectorSentinel {
            selector: 255,
            value: CompletionValue::Undefined,
        };
        let template = prepared
            .templates
            .handlers
            .iter_mut()
            .find(|handler| handler.handler_index == 0)
            .expect("handler 0");
        let span = template.source_span.clone();
        template.steps = operations
            .into_iter()
            .enumerate()
            .map(|(order, operation)| crate::RecipeStep {
                order: order as u32,
                span: span.clone(),
                operation,
            })
            .collect();
        prepared
    }

    fn number(text: &str) -> crate::SemanticValue {
        crate::SemanticValue::Literal(crate::LiteralValue::Number(text.into()))
    }

    /// The fixture image is `[0, 5, 1, 1, 7, 2]`: pc 0 in state 0 selects
    /// handler 0 and moves to state 1, and opcode 2 at pc 5 in state 1 is the
    /// completion selector. A jump from handler 0 straight to pc 5 must skip
    /// the bytes in between.
    #[test]
    fn the_driver_follows_a_handler_jump() {
        let prepared = with_handler_zero(vec![crate::RecipeOperation::SetProgramCounter {
            value: number("5"),
        }]);
        let report = execute_at(&prepared, DispatchStart { pc: 0, state: 0 });
        assert_eq!(report.stop, DispatchStopReason::CompletedSelector);
        assert_eq!(report.records.len(), 2, "{:?}", report.records);
        assert_eq!(report.records[0].dispatch_pc, 0);
        assert_eq!(report.records[0].actual_next_pc, 5);
        assert_eq!(report.records[0].state_after, 1);
        assert_eq!(report.records[1].dispatch_pc, 5);
        assert_eq!(report.records[1].state_before, 1);
    }

    #[test]
    fn the_driver_follows_a_call_and_its_dispatch_state() {
        let prepared = with_handler_zero(vec![
            crate::RecipeOperation::PushCallFrame,
            crate::RecipeOperation::SetProgramCounter { value: number("5") },
            crate::RecipeOperation::SetDispatchState { value: number("1") },
        ]);
        let report = execute_at(&prepared, DispatchStart { pc: 0, state: 0 });
        assert_eq!(report.stop, DispatchStopReason::CompletedSelector);
        assert_eq!(report.records.len(), 2, "{:?}", report.records);
        assert_eq!(report.records[0].actual_next_pc, 5);
        assert_eq!(report.records[0].state_after, 1);
    }

    /// A jump back to the same position with an unchanged call stack is a
    /// loop the driver must stop, not follow forever.
    #[test]
    fn a_jump_back_to_the_same_position_is_a_repeat() {
        let prepared = with_handler_zero(vec![
            crate::RecipeOperation::SetProgramCounter { value: number("0") },
            crate::RecipeOperation::SetDispatchState { value: number("0") },
        ]);
        let report = execute_at(&prepared, DispatchStart { pc: 0, state: 0 });
        assert_eq!(report.stop, DispatchStopReason::RepeatedPcState);
        assert_eq!(report.records.len(), 1);
        assert_eq!(
            last_diagnostic(&report),
            (
                "dispatch-repeated-pc-state",
                "dispatch revisited pc 0 state 0 without progress"
            )
        );
    }

    /// A jump target outside the image stops at the next fetch, after the
    /// jumping handler's record is committed.
    #[test]
    fn a_jump_outside_the_image_stops_at_the_next_fetch() {
        let prepared = with_handler_zero(vec![crate::RecipeOperation::SetProgramCounter {
            value: number("99"),
        }]);
        let report = execute_at(&prepared, DispatchStart { pc: 0, state: 0 });
        assert_eq!(report.stop, DispatchStopReason::TruncatedImage);
        assert_eq!(report.records.len(), 1);
        assert_eq!(report.records[0].actual_next_pc, 99);
        assert_eq!(
            last_diagnostic(&report),
            (
                "dispatch-truncated-image",
                "dispatch pc 99 is outside the 6 byte image"
            )
        );
    }

    /// A dispatch state assigned by a handler is checked like a table cell's
    /// next state: outside the table it is a missing state, not a panic.
    #[test]
    fn a_jump_to_a_missing_dispatch_state_stops() {
        let prepared = with_handler_zero(vec![crate::RecipeOperation::SetDispatchState {
            value: number("7"),
        }]);
        let report = execute_at(&prepared, DispatchStart { pc: 0, state: 0 });
        assert_eq!(report.stop, DispatchStopReason::MissingState);
        assert_eq!(report.records.len(), 1);
        assert_eq!(report.records[0].state_after, 7);
    }

    #[test]
    fn step_faults_keep_the_dispatch_codes_and_messages() {
        let cases = [
            (
                StepFault::Repeated { pc: 3, state: 4 },
                DispatchStopReason::RepeatedPcState,
                "dispatch-repeated-pc-state",
                "dispatch revisited pc 3 state 4 without progress",
            ),
            (
                StepFault::TruncatedImage {
                    pc: 5,
                    image_len: 7,
                },
                DispatchStopReason::TruncatedImage,
                "dispatch-truncated-image",
                "dispatch pc 5 is outside the 7 byte image",
            ),
            (
                StepFault::MissingState { state: 2 },
                DispatchStopReason::MissingState,
                "dispatch-missing-state",
                "dispatch state 2 has no proven table row",
            ),
            (
                StepFault::MissingOpcode {
                    state: 2,
                    opcode: 9,
                },
                DispatchStopReason::MissingOpcode,
                "dispatch-missing-opcode",
                "dispatch state 2 opcode 9 has no proven cell",
            ),
            (
                StepFault::InvalidNextState { next_state: 6 },
                DispatchStopReason::MissingState,
                "dispatch-invalid-next-state",
                "dispatch cell targets state 6 outside the proven table",
            ),
            (
                StepFault::PcOverflow {
                    pc: 8,
                    image_len: 9,
                },
                DispatchStopReason::TruncatedImage,
                "dispatch-pc-overflow",
                "dispatch PC increment overflowed the address space",
            ),
            (
                StepFault::TruncatedOperands {
                    position: 2,
                    operand_entry_pc: 1,
                    required_end: 5,
                    image_len: 4,
                },
                DispatchStopReason::TruncatedImage,
                "dispatch-truncated-operands",
                "operand image is truncated before the handler operands are complete",
            ),
            (
                StepFault::UnsupportedOperands {
                    selector: 7,
                    reason: "unsupported operand recipe: reason".into(),
                },
                DispatchStopReason::UnsupportedHandler,
                "dispatch-unsupported-operands",
                "unsupported operand recipe: reason",
            ),
            (
                StepFault::NextPcOverflow {
                    pc: 10,
                    image_len: 11,
                },
                DispatchStopReason::TruncatedImage,
                "dispatch-next-pc-overflow",
                "handler PC delta overflowed the address space",
            ),
            (
                StepFault::MissingOperandHandler { selector: 7 },
                DispatchStopReason::MissingHandler,
                "dispatch-missing-handler",
                "selector 7 has no operand-lifted handler",
            ),
        ];
        for (fault, expected_stop, expected_code, expected_message) in cases {
            let (stop, code, message) = dispatch_fault(&fault);
            assert_eq!(stop, expected_stop, "stop for {fault:?}");
            assert_eq!(code.as_str(), expected_code, "code for {fault:?}");
            assert_eq!(message, expected_message, "message for {fault:?}");
        }
    }
}
