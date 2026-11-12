use super::*;

impl TraceSession {
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        !matches!(self.executor.completion, TraceCompletion::Completed)
    }

    pub(crate) fn completion(&self) -> &TraceCompletion {
        &self.executor.completion
    }

    #[must_use]
    pub fn recipe_steps(&self) -> usize {
        self.executor.steps
    }

    /// Halt the committed prefix for a driver-level reason without discarding
    /// prior committed nodes, statements, events, or diagnostics. The session
    /// becomes stopped and its completion no longer reports `Completed`.
    pub fn stop_prefix(
        &mut self,
        code: TraceStopCode,
        message: impl Into<String>,
        provenance: Option<TraceProvenance>,
    ) {
        self.executor.completion = TraceCompletion::PrefixStopped(code);
        let message = message.into();
        self.executor.debug_stop(code, &message);
        self.executor.diagnostics.push(TraceDiagnostic {
            code,
            message,
            provenance,
        });
    }

    /// Record the typed terminal completion produced by the caller-supplied
    /// completion convention: read the VM stack at `index` (absent cells read
    /// as `undefined`), emit the read and a terminal `return`, and complete
    /// with that value.
    pub fn finalize_stack(
        &mut self,
        index: usize,
        provenance: TraceProvenance,
    ) -> Result<TraceValue, TraceFinishError> {
        if self.is_stopped() {
            return Err(TraceFinishError::AlreadyStopped);
        }
        if index >= self.executor.limits.max_stack_length {
            return Err(TraceFinishError::StackIndexOutOfRange {
                index,
                limit: self.executor.limits.max_stack_length,
            });
        }
        let checkpoint = self.executor.checkpoint();
        let index_node = self
            .executor
            .add_node(
                TraceNodeKind::Number(number_text(index as f64)),
                provenance.clone(),
            )
            .map_err(|stop| {
                self.executor.rollback(checkpoint.clone());
                finish_error(stop)
            })?;
        let value = self
            .executor
            .resolve_stack(index, &provenance)
            .map_err(|stop| {
                self.executor.rollback(checkpoint.clone());
                finish_error(stop)
            })?;
        self.executor.event(
            TraceEventKind::StackRead {
                index: index_node,
                value: value.node,
            },
            provenance.clone(),
        );
        let trace_value = TraceValue {
            value: value.value,
            node: value.node,
        };
        self.executor
            .statement(TraceStatementKind::Return { value: value.node }, provenance);
        self.executor.completion = TraceCompletion::Returned(trace_value.clone());
        Ok(trace_value)
    }

    /// Complete with the constant `undefined` under the caller-supplied
    /// convention.
    pub fn finalize_undefined(
        &mut self,
        provenance: TraceProvenance,
    ) -> Result<TraceValue, TraceFinishError> {
        if self.is_stopped() {
            return Err(TraceFinishError::AlreadyStopped);
        }
        let checkpoint = self.executor.checkpoint();
        let node = self
            .executor
            .add_node(TraceNodeKind::Undefined, provenance.clone())
            .map_err(|stop| {
                self.executor.rollback(checkpoint);
                finish_error(stop)
            })?;
        let value = TraceValue {
            value: SymbolicValue::Undefined,
            node,
        };
        self.executor
            .statement(TraceStatementKind::Return { value: node }, provenance);
        self.executor.completion = TraceCompletion::Returned(value.clone());
        Ok(value)
    }

    /// Execute one already-decoded handler transactionally. A returned recipe
    /// value is reported to the caller while the session remains runnable.
    pub fn execute_handler(
        &mut self,
        templates: &HandlerTemplateReport,
        instruction: &DecodedInstruction,
    ) -> TraceHandlerResult {
        self.debug_before_operation(None, None, None, instruction.handler_index);
        let handler = templates
            .handlers
            .iter()
            .find(|handler| handler.handler_index == instruction.handler_index);
        self.execute_handler_at(handler, instruction)
    }

    /// Execute one handler that the caller already resolved from a prepared
    /// selector index. The stop checks, missing-handler diagnostic and recipe
    /// execution are shared with [`Self::execute_handler`].
    pub(crate) fn execute_handler_at(
        &mut self,
        handler: Option<&HandlerTemplate>,
        instruction: &DecodedInstruction,
    ) -> TraceHandlerResult {
        // The cursor describes exactly one dispatch; a later handler must not
        // push a stale return address.
        let cursor = self.executor.cursor.take();
        if self.is_stopped() {
            return match self.executor.completion {
                TraceCompletion::BudgetStopped => TraceHandlerResult::BudgetStopped,
                _ => TraceHandlerResult::Partial,
            };
        }
        if self.executor.steps >= self.executor.limits.max_steps {
            self.executor.budget(
                TraceStopCode::StepBudget,
                "trace step budget exhausted",
                None,
            );
            return TraceHandlerResult::BudgetStopped;
        }
        let Some(handler) = handler else {
            self.executor.unsupported(
                TraceStopCode::MissingHandler,
                format!(
                    "handler {} is absent from the template report",
                    instruction.handler_index
                ),
                None,
            );
            return TraceHandlerResult::Partial;
        };
        if let Some((operand, node)) =
            instruction
                .operands
                .iter()
                .enumerate()
                .find_map(|(index, value)| match value {
                    SymbolicValue::Computed { node } => Some((index, *node)),
                    _ => None,
                })
        {
            self.executor.unsupported(
                TraceStopCode::ComputedInput,
                format!(
                    "decoded operand {operand} supplies Computed node {node}; computed values must come from this trace arena"
                ),
                Some(self.executor.provenance(handler, None)),
            );
            return TraceHandlerResult::Partial;
        }
        // A handler's recipe is checked once: the answer is the template's.
        let key = (
            handler as *const HandlerTemplate as usize,
            handler.handler_index,
            handler.steps.as_ptr() as usize,
            handler.steps.len(),
        );
        let reason = match self.executor.preflights.get(&key) {
            Some(reason) => reason.clone(),
            None => {
                let reason = preflight(handler);
                self.executor.preflights.insert(key, reason.clone());
                reason
            }
        };
        if let Some(reason) = reason {
            self.executor.unsupported(
                TraceStopCode::UnsupportedHandler,
                reason,
                Some(self.executor.provenance(handler, None)),
            );
            return TraceHandlerResult::Partial;
        }
        let checkpoint = self.executor.checkpoint();
        self.executor.cursor = cursor;
        let outcome = self.executor.run_handler(handler, instruction);
        self.executor.cursor = None;
        // What a dispatch told of the instruction's cell reads was for it.
        if outcome.is_ok() && !self.executor.context.cell_assumed.is_empty() {
            self.executor.context.cell_assumed = Arc::default();
        }
        match outcome {
            Ok(HandlerOutcome::Continue) => {
                self.executor.handlers_traced += 1;
                self.executor.debug_after_operation(None);
                TraceHandlerResult::Continued
            }
            Ok(HandlerOutcome::Return(value)) => {
                self.executor.handlers_traced += 1;
                self.executor.debug_after_operation(None);
                TraceHandlerResult::Returned(value)
            }
            Ok(HandlerOutcome::Throw(value)) => {
                self.executor.handlers_traced += 1;
                // A walking driver ends only this path at a throw: the
                // emitted `throw` stands where the VM threw.
                if !self.executor.thread_control {
                    self.executor.completion = TraceCompletion::Thrown(value.clone());
                }
                self.executor.debug_after_operation(None);
                TraceHandlerResult::Thrown(value)
            }
            Err(ExecutionStop::Budget(message, provenance)) => {
                self.executor.rollback(checkpoint);
                self.executor
                    .budget(TraceStopCode::Budget, &message, provenance);
                TraceHandlerResult::BudgetStopped
            }
            Err(ExecutionStop::Unsupported(code, message, provenance)) => {
                self.executor.rollback(checkpoint);
                self.executor.unsupported(code, message, provenance);
                TraceHandlerResult::Partial
            }
            Err(ExecutionStop::Throws(_)) => {
                unreachable!("run_handler turns a throw into its outcome")
            }
        }
    }

    /// Drop each VM subroutine's function no code calls: one a fork's side
    /// traced before it was cut back, whose calls went with it.
    pub fn drop_unused_functions(&mut self) {
        let arena = &mut self.executor.arena;
        let mut owned = vec![false; arena.statements.len()];
        for statement in &arena.statements {
            for child in block_children(&statement.kind) {
                if let Some(owned) = owned.get_mut(child as usize) {
                    *owned = true;
                }
            }
        }
        let functions: BTreeMap<String, TraceStatementId> = arena
            .statements
            .iter()
            .filter(|statement| !owned[statement.id as usize])
            .filter_map(|statement| match &statement.kind {
                TraceStatementKind::Function { name, .. } => Some((name.clone(), statement.id)),
                _ => None,
            })
            .collect();
        if functions.is_empty() {
            return;
        }
        // The functions the code under `roots` reads.
        let names_read = |arena: &TraceArena, roots: Vec<TraceStatementId>| {
            let mut names = BTreeSet::new();
            let mut statements = roots;
            let mut nodes = Vec::new();
            while let Some(id) = statements.pop() {
                let kind = &arena.statements[id as usize].kind;
                statements.extend(block_children(kind));
                match kind {
                    TraceStatementKind::Let { value, .. }
                    | TraceStatementKind::Assign { value, .. }
                    | TraceStatementKind::Evaluate { value }
                    | TraceStatementKind::Return { value }
                    | TraceStatementKind::Throw { value }
                    | TraceStatementKind::If { test: value, .. } => nodes.push(*value),
                    TraceStatementKind::PropertyWrite { object, key, value } => {
                        nodes.extend([*object, *key, *value]);
                    }
                    _ => {}
                }
            }
            let mut seen = vec![false; arena.nodes.len()];
            while let Some(node) = nodes.pop() {
                match seen.get_mut(node as usize) {
                    Some(seen) if !*seen => *seen = true,
                    _ => continue,
                }
                let node = &arena.nodes[node as usize];
                if let TraceNodeKind::Identifier { name } = &node.kind {
                    if functions.contains_key(name) {
                        names.insert(name.clone());
                    }
                }
                nodes.extend(node_children(&node.kind));
            }
            names
        };
        let roots: Vec<TraceStatementId> = arena
            .statements
            .iter()
            .filter(|statement| {
                !owned[statement.id as usize]
                    && !matches!(statement.kind, TraceStatementKind::Function { .. })
            })
            .map(|statement| statement.id)
            .collect();
        let mut used: BTreeSet<String> = BTreeSet::new();
        let mut pending = names_read(arena, roots);
        while !pending.is_empty() {
            let reached: Vec<TraceStatementId> = pending
                .iter()
                .filter(|name| !used.contains(*name))
                .filter_map(|name| functions.get(name).copied())
                .collect();
            used.extend(
                pending
                    .into_iter()
                    .filter(|name| functions.contains_key(name)),
            );
            pending = names_read(arena, reached);
            pending.retain(|name| !used.contains(name));
        }
        for (name, id) in functions {
            if used.contains(&name) {
                continue;
            }
            let mut statements = vec![id];
            while let Some(statement) = statements.pop() {
                let kind = std::mem::replace(
                    &mut arena.statements[statement as usize].kind,
                    TraceStatementKind::Declare { names: Vec::new() },
                );
                statements.extend(block_children(&kind));
            }
        }
    }

    #[must_use]
    pub fn report(self) -> TraceReport {
        let mut debug = self.executor.debug;
        debug.finish(&self.executor.completion);
        TraceReport {
            arena: self.executor.arena,
            completion: self.executor.completion,
            diagnostics: self.executor.diagnostics,
            handlers_traced: self.executor.handlers_traced,
            external_inputs: self.executor.external_inputs.iter().cloned().collect(),
            final_state: TraceStateSnapshot {
                stack_length: self.executor.context.stack.len(),
                memory_slots: self.executor.context.memory.len(),
            },
        }
    }

    pub(crate) fn debug_before_operation(
        &mut self,
        pc: Option<usize>,
        state: Option<usize>,
        opcode: Option<u8>,
        handler_key: u32,
    ) {
        self.executor
            .debug_before_operation(pc, state, opcode, handler_key);
    }

    pub(crate) fn debug_after_operation(&mut self, effect: Option<&str>) {
        self.executor.debug_after_operation(effect);
    }

    pub(crate) fn debug_enabled(&self) -> bool {
        self.executor.debug.enabled()
    }
}

/// Execute a sequence of supplied decoded instructions. Each handler is
/// preflighted before it can mutate state; failed handlers are transactional.
#[must_use]
pub fn trace_handlers(
    templates: &HandlerTemplateReport,
    instructions: &[DecodedInstruction],
    context: TraceContext,
    limits: TraceLimits,
) -> TraceReport {
    trace_handlers_with_debug(
        templates,
        instructions,
        context,
        limits,
        TraceDebugOptions::disabled(),
    )
}

/// Execute supplied decoded instructions with live per-operation debug output.
///
/// The output is emitted while each instruction executes. Direct symbolic
/// tracing has no proven dispatch PC/state, so its preamble uses
/// `NEXT:unavailable`; dispatch drivers supply the legacy `NEXT:state_pc`.
#[must_use]
pub fn trace_handlers_with_debug(
    templates: &HandlerTemplateReport,
    instructions: &[DecodedInstruction],
    context: TraceContext,
    limits: TraceLimits,
    debug: TraceDebugOptions,
) -> TraceReport {
    let mut session = TraceSession::new(context, limits).with_debug(debug);
    for instruction in instructions {
        match session.execute_handler(templates, instruction) {
            TraceHandlerResult::Continued => {}
            TraceHandlerResult::Returned(value) => {
                session.executor.completion = TraceCompletion::Returned(value);
                break;
            }
            TraceHandlerResult::Thrown(_) => {
                break;
            }
            TraceHandlerResult::Partial | TraceHandlerResult::BudgetStopped => {
                break;
            }
        }
    }
    session.report()
}

/// Convenience API for one recipe and one explicit operand list.
#[must_use]
pub fn trace_handler(
    templates: &HandlerTemplateReport,
    handler_index: u32,
    operands: Vec<SymbolicValue>,
    context: TraceContext,
    limits: TraceLimits,
) -> TraceReport {
    trace_handler_with_debug(
        templates,
        handler_index,
        operands,
        context,
        limits,
        TraceDebugOptions::disabled(),
    )
}

/// Convenience API for one recipe with live per-operation debug output.
#[must_use]
pub fn trace_handler_with_debug(
    templates: &HandlerTemplateReport,
    handler_index: u32,
    operands: Vec<SymbolicValue>,
    context: TraceContext,
    limits: TraceLimits,
    debug: TraceDebugOptions,
) -> TraceReport {
    trace_handlers_with_debug(
        templates,
        &[DecodedInstruction::new(handler_index, operands)],
        context,
        limits,
        debug,
    )
}
