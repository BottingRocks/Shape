use super::*;

impl TraceSession {
    /// The running path just entered a VM subroutine: emit
    /// `function name() { … }` at the top level and trace the subroutine
    /// into it. Every caller stack cell and memory slot is read as a
    /// parameter of the function; what only the caller knows (its branch
    /// facts, `try` variables and receiver) is not visible inside.
    pub fn open_subroutine(&mut self, name: String) -> Result<SubroutineFrame, TraceJoinError> {
        let executor = &mut self.executor;
        let provenance = executor.driver_provenance();
        let caller = executor.context.clone();
        let caller_block = executor.block;
        executor.block = None;
        let statement = executor.statement(
            TraceStatementKind::Function {
                name: name.clone(),
                params: Vec::new(),
                body: Vec::new(),
            },
            provenance.clone(),
        );
        executor.block = Some(BlockTarget {
            statement,
            arm: BlockArm::FunctionBody,
        });
        let entry_stack = executor.context.stack.len();
        // Every caller cell is a parameter, named when the body reads it.
        let context = &mut executor.context;
        for cell in context.stack.iter_mut() {
            *cell = StackCell::Param;
        }
        for (_, cell) in Arc::make_mut(&mut context.memory).0.iter_mut() {
            *cell = MemoryCell::Param;
        }
        context.subroutine = Some(Arc::new(OpenSubroutine {
            name: name.clone(),
            entry_stack,
            provenance,
        }));
        context.placeholders = Arc::default();
        context.receiver = None;
        context.branch_facts = Arc::default();
        context.type_facts = Arc::default();
        context.merged_concrete = Arc::default();
        context.loop_variables = Arc::default();
        context.value_sets = Arc::default();
        context.jump_sets = Arc::default();
        context.assumed = Arc::default();
        context.try_variables = Arc::default();
        context.try_writes = Arc::default();
        context.try_entry = None;
        context.try_outer = Arc::default();
        context.param_writes = Arc::default();
        Ok(SubroutineFrame {
            name,
            statement,
            caller,
            caller_block,
            entry_stack,
        })
    }

    /// The subroutine of `frame` returned to its caller: finish its function
    /// and go back to the caller's state at the call. Fails when the
    /// subroutine did more than a function can: changed the call, `try` or
    /// exception stacks, or read a caller variable.
    pub fn close_subroutine(
        &mut self,
        frame: SubroutineFrame,
    ) -> Result<SubroutineSummary, TraceJoinError> {
        let executor = &mut self.executor;
        let after = executor.context.clone();
        let caller = &frame.caller;
        let callers = &caller.call_frames[..caller.call_frames.len().saturating_sub(1)];
        if after.call_frames != callers {
            return Err(TraceJoinError(
                "the subroutine does not return to its caller".into(),
            ));
        }
        if after.try_frames != caller.try_frames
            || after.try_underflow != caller.try_underflow
            || after.error_records != caller.error_records
            || after.thrown.len() != caller.thrown.len()
        {
            return Err(TraceJoinError(
                "the subroutine changes the try or exception state".into(),
            ));
        }
        let owner: BTreeMap<&str, TraceCell> = after
            .placeholders
            .iter()
            .map(|(cell, name)| (name.as_str(), *cell))
            .collect();
        let name_of = |node: Option<TraceNodeId>| {
            node.and_then(|node| executor.arena.nodes.get(node as usize))
                .and_then(|node| match &node.kind {
                    TraceNodeKind::Identifier { name } => Some(name.as_str()),
                    _ => None,
                })
        };
        let own = |cell: TraceCell, stored: &StoredValue| {
            stored.fact.is_none()
                && name_of(stored.node).and_then(|name| owner.get(name)) == Some(&cell)
        };
        let relative = |cell: TraceCell| match cell {
            TraceCell::Stack(index) => CallCell::Stack(index as isize - frame.entry_stack as isize),
            TraceCell::Memory(slot) => CallCell::Memory(slot),
        };
        let mut changed = Vec::new();
        for (index, cell) in after.stack.iter().enumerate() {
            // Never read: the caller's value, which the call leaves.
            if matches!(cell, StackCell::Param) {
                continue;
            }
            let stored = stack_cell_value(cell);
            if index < frame.entry_stack && own(TraceCell::Stack(index), &stored) {
                continue;
            }
            changed.push((relative(TraceCell::Stack(index)), stored));
        }
        for (slot, cell) in after.memory.iter() {
            if matches!(cell, MemoryCell::Param) {
                continue;
            }
            let stored = memory_cell_value(cell);
            if own(TraceCell::Memory(*slot), &stored) {
                continue;
            }
            changed.push((relative(TraceCell::Memory(*slot)), stored));
        }
        let provenance = executor.driver_provenance();
        let mut outputs = Vec::with_capacity(changed.len());
        let mut returned = Vec::new();
        for (cell, stored) in changed {
            match &stored.fact {
                Some(fact) => outputs.push((cell, CallOutput::Constant(fact.clone()))),
                None => {
                    let node = executor.materialize(stored, &provenance)?;
                    outputs.push((cell, CallOutput::Returned(returned.len())));
                    returned.push(node);
                }
            }
        }
        // The parameters are the caller cells the function reads; it may
        // read nothing else from outside.
        let mut pending = returned.clone();
        let mut declared = BTreeSet::new();
        let mut blocks = vec![frame.statement];
        while let Some(id) = blocks.pop() {
            let statement = &executor.arena.statements[id as usize];
            blocks.extend(block_children(&statement.kind));
            match &statement.kind {
                TraceStatementKind::Let { name, value }
                | TraceStatementKind::Assign { name, value } => {
                    declared.insert(name.clone());
                    pending.push(*value);
                }
                TraceStatementKind::Declare { names } => declared.extend(names.iter().cloned()),
                TraceStatementKind::Try { param, .. } => {
                    declared.insert(param.clone());
                }
                TraceStatementKind::Evaluate { value }
                | TraceStatementKind::Return { value }
                | TraceStatementKind::Throw { value }
                | TraceStatementKind::If { test: value, .. } => pending.push(*value),
                TraceStatementKind::PropertyWrite { object, key, value } => {
                    pending.extend([*object, *key, *value]);
                }
                _ => {}
            }
        }
        let mut read = BTreeSet::new();
        let mut seen = BTreeSet::new();
        while let Some(node) = pending.pop() {
            if !seen.insert(node) {
                continue;
            }
            let Some(kind) = executor
                .arena
                .nodes
                .get(node as usize)
                .map(|node| &node.kind)
            else {
                continue;
            };
            if let TraceNodeKind::Identifier { name } = kind {
                match owner.get(name.as_str()) {
                    Some(cell) => {
                        read.insert(*cell);
                    }
                    None if declared.contains(name) => {}
                    None => {
                        return Err(TraceJoinError(format!(
                            "the subroutine reads {name}, a variable of its caller"
                        )))
                    }
                }
            }
            pending.extend(node_children(kind));
        }
        // Each parameter the body hands on or writes a property of, or
        // returns (the caller would hold it twice). Apart, the parameters
        // handed on other than by a plain `Object.defineProperty` on them
        // (`escaped_otherwise`), with those definitions (`defines`).
        let mut escaped = BTreeSet::new();
        let mut escaped_otherwise = BTreeSet::new();
        let mut defines: BTreeMap<String, Vec<(TraceNodeId, Option<SymbolicValue>)>> =
            BTreeMap::new();
        let mut blocks = vec![frame.statement];
        let param_name = |executor: &Executor, node: TraceNodeId| match executor
            .arena
            .nodes
            .get(node as usize)
            .map(|node| &node.kind)
        {
            Some(TraceNodeKind::Identifier { name }) if owner.contains_key(name.as_str()) => {
                Some(name.clone())
            }
            _ => None,
        };
        while let Some(id) = blocks.pop() {
            let kind = executor.arena.statements[id as usize].kind.clone();
            blocks.extend(block_children(&kind));
            let handed = executor.handed_on(&kind, true, |handle| {
                param_name(executor, handle).is_some()
            });
            for &handle in &handed {
                if let Some(name) = param_name(executor, handle) {
                    escaped.insert(name);
                }
            }
            let definition = match &kind {
                TraceStatementKind::Evaluate { value } | TraceStatementKind::Let { value, .. } => {
                    executor.plain_definition(*value).and_then(|(object, key, descriptor, value)| {
                        let name = param_name(executor, executor.object_handle(object))?;
                        Some((name, key, descriptor, value))
                    })
                }
                _ => None,
            };
            match definition {
                Some((name, key, descriptor, value)) => {
                    // What else the statement hands on: its key and
                    // descriptor.
                    for part in [key, descriptor] {
                        for handle in executor.handed_on(
                            &TraceStatementKind::Return { value: part },
                            true,
                            |handle| param_name(executor, handle).is_some(),
                        ) {
                            if let Some(name) = param_name(executor, handle) {
                                escaped_otherwise.insert(name);
                            }
                        }
                    }
                    defines.entry(name).or_default().push((key, value));
                }
                None => {
                    for handle in handed {
                        if let Some(name) = param_name(executor, handle) {
                            escaped_otherwise.insert(name);
                        }
                    }
                }
            }
        }
        // A parameter returned as the one value is the caller's object
        // again: that alone does not hand it to other code.
        let mut returned_param = None;
        for &node in &returned {
            if let Some(name) = param_name(executor, executor.object_handle(node)) {
                escaped.insert(name.clone());
                if returned.len() == 1 {
                    returned_param = Some(name);
                } else {
                    escaped_otherwise.insert(name);
                }
            }
        }
        let mut params = read.into_iter().collect::<Vec<_>>();
        params.sort_by_key(|cell| relative(*cell));
        let names = params
            .iter()
            .map(|cell| after.placeholders[cell].clone())
            .collect::<Vec<_>>();
        // An object the body made that only it held up to the return: what
        // it knew of it then. One returned in two places is two values to
        // the caller, so neither is known.
        let handles: Vec<TraceNodeId> = returned
            .iter()
            .map(|&node| executor.object_handle(node))
            .collect();
        let returned_objects = handles
            .iter()
            .map(|handle| {
                (handles.iter().filter(|other| *other == handle).count() == 1)
                    .then(|| after.made_objects.get(handle).cloned())
                    .flatten()
            })
            .collect();
        if !returned.is_empty() {
            let value = if returned.len() == 1 {
                returned[0]
            } else {
                executor
                    .add_node(
                        TraceNodeKind::ArrayLiteral {
                            elements: returned.clone(),
                        },
                        provenance.clone(),
                    )
                    .map_err(|_| TraceJoinError("the returned values cannot be built".into()))?
            };
            executor.statement(TraceStatementKind::Return { value }, provenance);
        }
        if let TraceStatementKind::Function { params, .. } =
            &mut executor.arena.statements[frame.statement as usize].kind
        {
            *params = names.clone();
        }
        let stack_delta = after.stack.len() as isize - frame.entry_stack as isize;
        executor.block = frame.caller_block;
        executor.context = frame.caller;
        let cell_writes = after
            .param_writes
            .iter()
            .filter_map(|(slot, name)| Some((*slot, relative(*owner.get(name.as_str())?))))
            .collect();
        Ok(SubroutineSummary {
            name: frame.name,
            statement: frame.statement,
            params: params.into_iter().map(relative).collect(),
            outputs,
            returned: returned.len(),
            stack_delta,
            cell_writes,
            returns: returned,
            returned_objects,
            param_escapes: names
                .iter()
                .map(|name| escaped.contains(name))
                .collect::<Vec<_>>()
                .into(),
            param_defines: names
                .iter()
                .map(|name| {
                    if escaped_otherwise.contains(name) {
                        return None;
                    }
                    defines
                        .get(name)
                        .cloned()
                        .or_else(|| (returned_param.as_ref() == Some(name)).then(Vec::new))
                })
                .collect::<Vec<_>>()
                .into(),
            returns_param: returned_param
                .filter(|name| !escaped_otherwise.contains(name))
                .and_then(|name| names.iter().position(|other| *other == name)),
            param_names: names,
        })
    }

    /// Whether the function of `summary` is still traced: a rollback past
    /// it removes it.
    #[must_use]
    pub fn has_subroutine(&self, summary: &SubroutineSummary) -> bool {
        matches!(
            self.executor.arena.statements.get(summary.statement as usize).map(|s| &s.kind),
            Some(TraceStatementKind::Function { name, .. }) if *name == summary.name
        )
    }

    /// The running path just entered the subroutine `summary` describes:
    /// call its function with the caller's cells, write what it leaves,
    /// and return from the VM call. Fails, changing nothing, when the
    /// caller lacks a cell the function reads or writes.
    pub fn call_subroutine(
        &mut self,
        summary: &SubroutineSummary,
        tag: usize,
    ) -> Result<(), TraceJoinError> {
        let executor = &mut self.executor;
        let length = executor.context.stack.len() as isize;
        let cell = |relative: CallCell| match relative {
            CallCell::Stack(offset) => {
                let index = length + offset;
                (index >= 0).then_some(TraceCell::Stack(index as usize))
            }
            CallCell::Memory(slot) => Some(TraceCell::Memory(slot)),
        };
        let missing = || TraceJoinError("the caller lacks a cell the subroutine uses".into());
        let new_length = length + summary.stack_delta;
        if new_length < 0 {
            return Err(missing());
        }
        // A call that reaches the holes a join put into the stack is traced
        // inline, where the step that reaches them undoes that join.
        let reaches =
            |index: isize| index < 0 || executor.context.padded_below(index as usize).is_some();
        if reaches(new_length)
            || summary
                .params
                .iter()
                .chain(summary.outputs.iter().map(|(output, _)| output))
                .chain(summary.cell_writes.iter().map(|(_, written)| written))
                .any(|cell| matches!(cell, CallCell::Stack(offset) if reaches(length + offset)))
        {
            return Err(missing());
        }
        // A caller that is a subroutine itself names the parameters read.
        let unbuilt = |_| TraceJoinError("a subroutine parameter cannot be built".into());
        for (slot, written) in &summary.cell_writes {
            let stored = match cell(*written) {
                Some(cell) => executor.cell_value(cell).map_err(unbuilt)?,
                None => None,
            };
            let types = stored
                .as_ref()
                .map_or(predicate::ANY_TYPES, |stored| executor.stored_types(stored));
            executor.note_cell_types(*slot, types);
            let content = stored.and_then(|stored| executor.stored_content(&stored));
            executor.note_cell_content(*slot, content);
        }
        let mut arguments = Vec::with_capacity(summary.params.len());
        for param in &summary.params {
            let cell = cell(*param).ok_or_else(missing)?;
            let stored = executor
                .cell_value(cell)
                .map_err(unbuilt)?
                .ok_or_else(missing)?;
            arguments.push(stored);
        }
        for (output, _) in &summary.outputs {
            match cell(*output) {
                Some(TraceCell::Stack(index)) if (index as isize) < new_length => {}
                Some(TraceCell::Memory(slot)) if executor.context.memory.contains_key(&slot) => {}
                _ => return Err(missing()),
            }
        }
        let provenance = executor.driver_provenance();
        let build = |_: ExecutionStop| TraceJoinError("the call cannot be built".into());
        let mut nodes = Vec::with_capacity(arguments.len());
        // To decisions on what the call returns, an argument known to be a
        // constant is that constant.
        let mut nodes_for_returns = Vec::with_capacity(arguments.len());
        for stored in arguments {
            let fact = stored.fact.clone();
            let node = executor.materialize(stored, &provenance)?;
            nodes.push(node);
            nodes_for_returns.push(match fact {
                Some(fact) => executor.value_node(fact, &provenance).map_err(build)?.node,
                None => node,
            });
        }
        executor
            .subroutine_escapes
            .insert(summary.name.clone(), summary.param_escapes.clone());
        let argument_nodes = nodes.clone();
        let callee = executor
            .identifier(&summary.name, &provenance)
            .map_err(build)?;
        let this_value = executor
            .add_node(TraceNodeKind::Undefined, provenance.clone())
            .map_err(build)?;
        let call = executor
            .add_node(
                TraceNodeKind::Call {
                    callee,
                    this_value,
                    arguments: nodes,
                },
                provenance.clone(),
            )
            .map_err(build)?;
        let bindings: BTreeMap<&str, TraceNodeId> = summary
            .param_names
            .iter()
            .map(String::as_str)
            .zip(nodes_for_returns.iter().copied())
            .collect();
        // A made object passed to a parameter the body neither writes nor
        // hands on (see `SubroutineSummary::param_escapes`) holds, where the
        // body reads it, what the caller wrote there: to decisions on what
        // the call returns, such a read is that value.
        let handles: Vec<TraceNodeId> = nodes_for_returns
            .iter()
            .map(|&node| executor.object_handle(node))
            .collect();
        let objects: BTreeMap<TraceNodeId, BTreeMap<String, SymbolicValue>> = handles
            .iter()
            .zip(summary.param_escapes.iter())
            .filter(|(handle, &escapes)| {
                !escapes
                    && !handles
                        .iter()
                        .zip(summary.param_escapes.iter())
                        .any(|(other, &escapes)| escapes && other == *handle)
            })
            .filter_map(|(&handle, _)| {
                let properties = executor.context.made_objects.get(&handle)?;
                (!properties.is_empty()).then(|| (handle, properties.clone()))
            })
            .collect();
        let mut copied = BTreeMap::new();
        let returns: Option<Vec<TraceNodeId>> = summary
            .returns
            .iter()
            .map(|&node| executor.substitute(node, &bindings, &objects, &mut copied, 0))
            .collect();
        match returns {
            Some(returns) => {
                executor.call_returns.insert(call, returns);
                executor.substituted.insert(tag);
            }
            None => {
                executor.substituted.remove(&tag);
            }
        }
        // A made object passed only where the body defines plain data
        // properties on it (see `SubroutineSummary::param_defines`): what
        // the path knew of it, with those properties, before the call hands
        // it on.
        let mut definitions: Vec<(TraceNodeId, BTreeMap<String, SymbolicValue>, Vec<(Option<SymbolicValue>, Option<SymbolicValue>)>)> =
            Vec::new();
        for (index, defines) in summary.param_defines.iter().enumerate() {
            let Some(defines) = defines else { continue };
            let handle = handles[index];
            let Some(properties) = executor.context.made_objects.get(&handle).cloned() else {
                continue;
            };
            let elsewhere = handles.iter().enumerate().any(|(other, &other_handle)| {
                other_handle == handle
                    && summary.param_escapes[other]
                    && summary.param_defines[other].is_none()
            });
            if elsewhere || definitions.iter().any(|(done, ..)| *done == handle) {
                continue;
            }
            let applied = defines
                .iter()
                .map(|(key, value)| {
                    let key = executor
                        .folded_literal(*key, &bindings, 0)
                        .filter(|key| matches!(key, SymbolicValue::String(_)));
                    (key, value.clone())
                })
                .collect();
            definitions.push((handle, properties, applied));
        }
        let result = if summary.returned == 0 {
            executor.statement(
                TraceStatementKind::Evaluate { value: call },
                provenance.clone(),
            );
            None
        } else {
            let (name, bound) = executor
                .bind_value(computed(call), &provenance)
                .map_err(build)?;
            Some((name, bound.node))
        };
        // The call's result is the object passed to the parameter it returns.
        if let Some(&argument) = summary.returns_param.and_then(|index| argument_nodes.get(index)) {
            executor.returned_arguments.insert(call, argument);
        }
        for (handle, properties, applied) in definitions {
            Arc::make_mut(&mut executor.context.made_objects).insert(handle, properties);
            for (key, value) in applied {
                executor.note_property(handle, key.as_ref(), value);
            }
        }
        let context = &mut executor.context;
        context.call_frames.pop();
        let new_length = new_length as usize;
        if context.stack.len() > new_length {
            context.stack.truncate(new_length);
        } else {
            context.stack.resize(new_length, StackCell::Hole);
        }
        for (output, value) in &summary.outputs {
            let target = cell(*output).expect("checked above");
            let evaluated = match value {
                CallOutput::Constant(fact) => executor
                    .value_node(fact.clone(), &provenance)
                    .map_err(build)?,
                CallOutput::Returned(index) => {
                    let (result_name, result) = result.clone().expect("a returned value");
                    let (name, value) = if summary.returned == 1 {
                        executor.note_made_return(result, summary.returned_objects.first());
                        (result_name, computed(result))
                    } else {
                        let key = executor
                            .value_node(SymbolicValue::Number(*index as f64), &provenance)
                            .map_err(build)?
                            .node;
                        let read = executor
                            .add_node(
                                TraceNodeKind::PropertyRead {
                                    object: result,
                                    key,
                                },
                                provenance.clone(),
                            )
                            .map_err(build)?;
                        executor.note_made_return(read, summary.returned_objects.get(*index));
                        executor
                            .bind_value(computed(read), &provenance)
                            .map_err(build)?
                    };
                    // Traced inline, the value might be concrete or of a known
                    // type: a test or VM value that reads it sends the call
                    // inline, so it is decided as without functions.
                    executor.marked.insert(name.clone());
                    executor.outlined.insert(name, tag);
                    value
                }
            };
            let evaluated = executor.note_try_write(target, evaluated, &provenance);
            executor.context.set_cell(
                target,
                StoredValue {
                    value: evaluated.value,
                    fact: evaluated.fact,
                    node: Some(evaluated.node),
                },
            );
        }
        Ok(())
    }

    /// After `trace_loop`, each loop variable holds the value it started with
    /// or one an iteration carried back to the header: one of its start's
    /// types or `carried`'s (see [`Self::loop_types`]). The path keeps what
    /// it already knew of it too.
    pub fn type_loop_variables(
        &mut self,
        trace_loop: &TraceLoop,
        carried: &BTreeMap<TraceCell, u16>,
    ) {
        let executor = &mut self.executor;
        for (cell, name) in &trace_loop.variables {
            let (Some(&seen), Some(&start)) = (carried.get(cell), trace_loop.starts.get(cell))
            else {
                continue;
            };
            let entry = predicate::value_types(
                &executor.arena,
                &executor.aliases,
                &executor.joins,
                &executor.call_returns,
                &executor.context.type_facts,
                start,
            );
            let key = predicate::variable_key(name);
            let known = executor
                .context
                .type_facts
                .get(&key)
                .copied()
                .unwrap_or(predicate::ANY_TYPES);
            let types = (entry | seen) & known;
            if types != predicate::ANY_TYPES {
                Arc::make_mut(&mut executor.context.type_facts).insert(key, types);
            }
        }
    }

    /// The running path leaves `trace_loop` where code after it continues:
    /// emit `break label`; its later statements go after the loop.
    pub fn break_loop(&mut self, trace_loop: &TraceLoop) -> Result<(), TraceJoinError> {
        let executor = &mut self.executor;
        let mut context = std::mem::take(&mut executor.context);
        let block = executor.block;
        let hoisted = executor.hoist(
            &mut context,
            block,
            trace_loop.declare,
            trace_loop.first_node,
        );
        executor.context = context;
        hoisted?;
        let provenance = self.executor.driver_provenance();
        self.executor.statement(
            TraceStatementKind::Break {
                label: trace_loop.label.clone(),
            },
            provenance,
        );
        self.executor.block = trace_loop.parent;
        Ok(())
    }

    /// Leave the body of `trace_loop`: new statements go to the block the
    /// loop was opened in.
    pub fn close_loop(&mut self, trace_loop: &TraceLoop) {
        self.executor.block = trace_loop.parent;
    }
}
