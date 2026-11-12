use super::*;

impl TraceSession {
    /// Open `try { … } catch (e) { … }` for the `try` frame the running path
    /// just pushed, and return the body and catch paths. The catch path
    /// starts as `vmHandleException` leaves the VM: the frame popped, an
    /// error record with a pending exception, and that exception (the
    /// catch parameter) on the thrown-value stack. Each `widened` cell is
    /// one the body writes and the catch reads: it becomes a variable
    /// declared before the `try`, assigned on every write in the body.
    pub fn fork_try(
        &mut self,
        widened: &BTreeSet<TraceCell>,
    ) -> Result<(TraceFork, TracePath, TracePath), TraceJoinError> {
        let every = self.concrete_cells();
        self.fork_try_with(widened, &BTreeMap::new(), &BTreeMap::new(), &every, &BTreeSet::new())
    }

    /// [`Self::fork_try`], with the values the driver saw the body assign
    /// each `try` variable (`None`: not a known set). A variable whose entry
    /// value and every such value are concrete has that value set in the
    /// catch; see [`Self::take_dispatch`]. A variable in `entry_hidden` is
    /// one the body writes before anything it runs can throw (see
    /// [`Self::try_entry_exposed`]): its set is only what the body assigned.
    pub fn fork_try_with(
        &mut self,
        widened: &BTreeSet<TraceCell>,
        values: &BTreeMap<TraceCell, Option<Vec<SymbolicValue>>>,
        types: &BTreeMap<TraceCell, u16>,
        cells_written: &BTreeSet<u32>,
        entry_hidden: &BTreeSet<TraceCell>,
    ) -> Result<(TraceFork, TracePath, TracePath), TraceJoinError> {
        let provenance = self.executor.driver_provenance();
        // The body knows the values exactly: each variable equals its cell
        // there, since every write in the body assigns both.
        let entry = self.executor.context.clone();
        let mut variables = BTreeMap::new();
        let mut variable_types = Vec::new();
        for &cell in widened {
            let Some(stored) = self.executor.context.cell(cell) else {
                continue;
            };
            let entry_known = self.executor.candidates(&stored, None);
            let jump_set = match (&entry_known, values.get(&cell)) {
                (_, Some(Some(seen)))
                    if !seen.is_empty()
                        && seen.len() <= MAX_VALUE_SET
                        && seen.iter().all(|value| {
                            matches!(value, SymbolicValue::Number(number)
                                if *number >= 0.0 && number.fract() == 0.0)
                        }) =>
                {
                    Some(seen.clone())
                }
                _ => None,
            };
            let set = match (entry_known, values.get(&cell)) {
                // No throw the catch takes comes before the body's first
                // write: the catch never sees the entry value.
                (_, Some(Some(seen))) if entry_hidden.contains(&cell) && !seen.is_empty() => {
                    Some(seen.clone())
                }
                (Some(_), Some(None)) | (None, _) => None,
                (Some(mut entry), Some(Some(seen))) => {
                    for value in seen {
                        if !entry.iter().any(|known| same_fact(known, value)) {
                            entry.push(value.clone());
                        }
                    }
                    Some(entry)
                }
                (Some(entry), None) => Some(entry),
            };
            let executor = &mut self.executor;
            let node = executor.materialize(stored, &provenance)?;
            let name = format!("trace_value_{}", executor.binding_serial);
            executor.binding_serial += 1;
            executor.statement(
                TraceStatementKind::Let {
                    name: name.clone(),
                    value: node,
                },
                provenance.clone(),
            );
            let variable = executor
                .identifier(&name, &provenance)
                .map_err(|_| TraceJoinError("a try variable cannot be built".into()))?;
            executor.context.set_cell(
                cell,
                StoredValue {
                    value: SymbolicValue::Computed { node: variable },
                    fact: None,
                    node: Some(variable),
                },
            );
            if let Some(set) = set.filter(|set| !set.is_empty() && set.len() <= MAX_VALUE_SET) {
                Arc::make_mut(&mut executor.context.value_sets).insert(name.clone(), set.into());
            }
            if let Some(set) = jump_set {
                Arc::make_mut(&mut executor.context.jump_sets).insert(name.clone(), set.into());
            }
            // Its value is the entry's or one the body assigned (see
            // [`Self::try_assigned_types`]): one of their types, once a pass
            // learned what the body assigns.
            if let Some(&assigned) = types.get(&cell) {
                let entry = predicate::value_types(
                    &executor.arena,
                    &executor.aliases,
                    &executor.joins,
                    &executor.call_returns,
                    &executor.context.type_facts,
                    node,
                );
                let union = entry | assigned;
                if union & predicate::ANY_TYPES != predicate::ANY_TYPES {
                    let key = predicate::variable_key(&name);
                    Arc::make_mut(&mut executor.context.type_facts).insert(key.clone(), union);
                    variable_types.push((key, union));
                }
            }
            variables.insert(cell, name);
        }
        let executor = &mut self.executor;
        if executor.context.try_frames.is_empty() {
            return Err(TraceJoinError("no try frame to open".into()));
        }
        let declare = executor.statement(
            TraceStatementKind::Declare { names: Vec::new() },
            provenance.clone(),
        );
        let param = format!("trace_value_{}", executor.binding_serial);
        executor.binding_serial += 1;
        executor.catch_params.insert(param.clone());
        let statement = executor.statement(
            TraceStatementKind::Try {
                param: param.clone(),
                body: Vec::new(),
                handler: Vec::new(),
                catches: true,
                finalizer: None,
            },
            provenance.clone(),
        );
        // The catch parameter is only in scope inside the catch.
        let first_node = executor.arena.nodes.len();
        let exception = executor
            .identifier(&param, &provenance)
            .map_err(|_| TraceJoinError("the catch parameter cannot be built".into()))?;
        // The catch runs in the enclosing `try`; the body opens its own
        // scope, keeping the enclosing one for when it ends.
        let mut catch = executor.context.clone();
        // The throw comes after any of the body's code. What the path knows
        // of a cell no other function holds that the body does not write is
        // there when it throws, and what it concretely put in any cell the
        // body does not write; the walk traces the `try` again with any cell
        // the body writes found since (`cells_written`).
        let escaped = catch.escaped_cells.clone();
        // The body may have written an object's properties before it threw.
        catch.made_objects = Arc::default();
        let keep = |slot: &u32| !cells_written.contains(slot) && !escaped.contains(slot);
        if !catch.cell_facts.keys().all(keep) {
            Arc::make_mut(&mut catch.cell_facts).retain(|slot, _| keep(slot));
        }
        if !catch.cell_nodes.keys().all(keep) {
            Arc::make_mut(&mut catch.cell_nodes).retain(|slot, _| keep(slot));
        }
        if catch
            .cell_concrete
            .keys()
            .any(|slot| cells_written.contains(slot))
        {
            Arc::make_mut(&mut catch.cell_concrete).retain(|slot, _| !cells_written.contains(slot));
        }
        // A VM position the body does not write is there when it throws
        // (see `cell_positions`); the walk traces the `try` again with any
        // cell the body writes found since.
        if catch
            .cell_positions
            .keys()
            .any(|slot| widened.contains(&TraceCell::Memory(*slot)))
        {
            Arc::make_mut(&mut catch.cell_positions)
                .retain(|slot, _| !widened.contains(&TraceCell::Memory(*slot)));
        }
        if catch
            .cell_values
            .iter()
            .any(|slot| widened.contains(&TraceCell::Memory(*slot)))
        {
            Arc::make_mut(&mut catch.cell_values)
                .retain(|slot| !widened.contains(&TraceCell::Memory(*slot)));
        }
        let mut body = entry;
        // The variables' types hold in the body and after the `try` too.
        for (key, types) in variable_types {
            Arc::make_mut(&mut body.type_facts).insert(key, types);
        }
        let outer = TryScope {
            variables: std::mem::take(&mut body.try_variables),
            writes: std::mem::take(&mut body.try_writes),
            entry: body.try_entry.take(),
        };
        Arc::make_mut(&mut body.try_outer).push(outer);
        body.try_variables = Arc::new(variables);
        body.try_entry = Some(Arc::new(body.clone()));
        catch.try_frames.pop();
        catch.error_records.push(ErrorRecord {
            handled: true,
            saved: None,
            finally: None,
            thrown_in: Some(statement),
        });
        let memory = Arc::clone(&catch.memory);
        Arc::make_mut(&mut catch.catch_memory).insert(statement, memory);
        catch.thrown.push(StoredValue {
            value: SymbolicValue::Computed { node: exception },
            fact: None,
            node: Some(exception),
        });
        catch.from_catch = true;
        catch.catch_try = Some(statement);
        let path = |context, arm| TracePath {
            context,
            block: Some(BlockTarget { statement, arm }),
        };
        Ok((
            TraceFork {
                statement,
                declare,
                first_node,
                first_binding: executor.binding_serial,
                parent: executor.block,
                arms: (BlockArm::TryBody, BlockArm::Catch),
            },
            path(body, BlockArm::TryBody),
            path(catch, BlockArm::Catch),
        ))
    }

    /// The `try` variables of the body path `path`, by cell.
    #[must_use]
    pub fn try_variable_names(path: &TracePath) -> BTreeMap<TraceCell, String> {
        (*path.context.try_variables).clone()
    }

    /// The values the body of the `try` `fork` assigns each of `names`:
    /// `None` for one assigned a value that is not concrete or in a known
    /// value set.
    #[must_use]
    pub fn try_assigned_values(
        &self,
        fork: &TraceFork,
        names: &BTreeMap<TraceCell, String>,
    ) -> BTreeMap<TraceCell, Option<Vec<SymbolicValue>>> {
        let executor = &self.executor;
        let cells: BTreeMap<&str, TraceCell> = names
            .iter()
            .map(|(cell, name)| (name.as_str(), *cell))
            .collect();
        let mut values: BTreeMap<TraceCell, Option<Vec<SymbolicValue>>> =
            names.keys().map(|cell| (*cell, Some(Vec::new()))).collect();
        let statements = &executor.arena.statements;
        let mut blocks: Vec<&[TraceStatementId]> =
            match &statements[fork.statement as usize].kind {
                TraceStatementKind::Try { body, .. } => vec![body.as_slice()],
                _ => Vec::new(),
            };
        let mut assigns = Vec::new();
        while let Some(block) = blocks.pop() {
            for (index, &id) in block.iter().enumerate() {
                let kind = &statements[id as usize].kind;
                match kind {
                    TraceStatementKind::If {
                        taken,
                        fall_through,
                        ..
                    } => blocks.extend([taken.as_slice(), fall_through.as_slice()]),
                    TraceStatementKind::Loop { body, .. }
                    | TraceStatementKind::Function { body, .. } => blocks.push(body),
                    TraceStatementKind::Try {
                        body,
                        handler,
                        finalizer,
                        ..
                    } => {
                        blocks.extend([body.as_slice(), handler.as_slice()]);
                        blocks.extend(finalizer.as_deref());
                    }
                    _ => {}
                }
                let TraceStatementKind::Assign { name, value } = kind else {
                    continue;
                };
                // A value the next assignment of the variable replaces
                // before anything that can throw: no catch sees it.
                let replaced = block[index + 1..]
                    .iter()
                    .map(|&next| &statements[next as usize].kind)
                    .take_while(|next| match next {
                        TraceStatementKind::Assign { value, .. }
                        | TraceStatementKind::Let { value, .. } => {
                            is_pure_value(&executor.arena, *value)
                        }
                        _ => false,
                    })
                    .any(|next| {
                        matches!(next, TraceStatementKind::Assign { name: other, .. } if other == name)
                    });
                if !replaced {
                    assigns.push((name, *value));
                }
            }
        }
        for (name, value) in assigns {
            let Some(cell) = cells.get(name.as_str()) else {
                continue;
            };
            let assigned = executor.node_values(value);
            let entry = values.get_mut(cell).expect("every cell has an entry");
            match (entry.as_mut(), assigned) {
                (Some(set), Some(assigned)) => {
                    for value in assigned {
                        if !set.iter().any(|known| same_fact(known, &value)) {
                            set.push(value);
                        }
                    }
                }
                _ => *entry = None,
            }
        }
        values
    }

    /// For each `try` variable in `names`: whether a throw the catch of
    /// the `try` `fork` takes can come while the variable still holds the
    /// value it had at the `try`'s entry (`true`), or the body writes it
    /// first on every path that can throw there (`false`). A throw inside
    /// a nested `try` goes to that `try`'s handler, which then starts with
    /// the entry value only if its own body could throw before the write.
    #[must_use]
    pub fn try_entry_exposed(
        &self,
        fork: &TraceFork,
        names: &BTreeMap<TraceCell, String>,
    ) -> BTreeMap<TraceCell, bool> {
        let arena = &self.executor.arena;
        let TraceStatementKind::Try { body, .. } = &arena.statements[fork.statement as usize].kind else {
            return names.keys().map(|cell| (*cell, true)).collect();
        };
        names
            .iter()
            .map(|(cell, name)| {
                let mut walk = EntryWalk { arena, name, exposed: false };
                walk.block(body, true);
                (*cell, walk.exposed)
            })
            .collect()
    }

    /// After a `try` that needs no other pass: each `try` variable in
    /// `names` holds its entry value or one the body assigned it, so on the
    /// path after the `try` it has one of their types (see
    /// [`predicate::variable_key`]). A test after the `try` on one (an opaque
    /// predicate on a probe's outcome, say) is decided on them; the variable
    /// is assigned nowhere else.
    pub fn type_try_variables(&mut self, names: &BTreeMap<TraceCell, String>) {
        let executor = &mut self.executor;
        for name in names.values() {
            let key = predicate::variable_key(name);
            if executor.context.type_facts.contains_key(&key) {
                continue;
            }
            let Some(definitions) = executor.definitions.get(name) else {
                continue;
            };
            let union = definitions.iter().fold(0, |union, &(_, value)| {
                union
                    | predicate::value_types(
                        &executor.arena,
                        &executor.aliases,
                        &executor.joins,
                        &executor.call_returns,
                        &executor.context.type_facts,
                        value,
                    )
            });
            if union != 0 && union & predicate::ANY_TYPES != predicate::ANY_TYPES {
                Arc::make_mut(&mut executor.context.type_facts).insert(key, union);
            }
        }
    }

    /// The types of the values the body of `fork` assigns each `try`
    /// variable in `names`, by cell (see [`predicate::value_types`]).
    #[must_use]
    pub fn try_assigned_types(
        &self,
        fork: &TraceFork,
        names: &BTreeMap<TraceCell, String>,
    ) -> BTreeMap<TraceCell, u16> {
        let executor = &self.executor;
        let cells: BTreeMap<&str, TraceCell> = names
            .iter()
            .map(|(cell, name)| (name.as_str(), *cell))
            .collect();
        let mut types: BTreeMap<TraceCell, u16> = names.keys().map(|cell| (*cell, 0)).collect();
        let mut pending = match &executor.arena.statements[fork.statement as usize].kind {
            TraceStatementKind::Try { body, .. } => body.clone(),
            _ => Vec::new(),
        };
        while let Some(id) = pending.pop() {
            let statement = &executor.arena.statements[id as usize];
            pending.extend(block_children(&statement.kind));
            let TraceStatementKind::Assign { name, value } = &statement.kind else {
                continue;
            };
            let Some(cell) = cells.get(name.as_str()) else {
                continue;
            };
            *types.get_mut(cell).expect("every cell has an entry") |= predicate::value_types(
                &executor.arena,
                &executor.aliases,
                &executor.joins,
                &executor.call_returns,
                &executor.context.type_facts,
                *value,
            );
        }
        types
    }

    /// Join the closed body path of `fork` with its catch path where both
    /// enter a `finally`: the body with a record of where it goes on, the
    /// catch, before any statement, with the exception pending. The VM runs
    /// the same code for both: traced once on the merged path, it is the
    /// `try`'s `finally` block, and the handler is no `catch`. The merged
    /// path holds the body's record, tagged so that popping it ends the
    /// `finally` (see `Executor::close_finally`).
    ///
    /// `Ok(None)` when the paths are not such an entry; an error when they
    /// are but do not join: the caller rolls back and traces them apart.
    pub fn join_finally(
        &mut self,
        fork: &TraceFork,
        body: &TracePath,
        catch: &TracePath,
    ) -> Result<Option<TracePath>, TraceJoinError> {
        let (Some(&entered), Some(thrown)) = (
            body.context.error_records.last(),
            catch.context.error_records.last(),
        ) else {
            return Ok(None);
        };
        let catch_arm = Some(BlockTarget {
            statement: fork.statement,
            arm: BlockArm::Catch,
        });
        if entered.handled
            || entered.saved.is_none()
            || entered.finally.is_some()
            || !thrown.handled
            || catch.context.thrown.len() != body.context.thrown.len() + 1
            || catch.block != catch_arm
            || !self.executor.arm(catch_arm).is_some_and(Vec::is_empty)
        {
            return Ok(None);
        }
        // The catch path as the body enters the `finally`: no exception
        // pending, the same record.
        let mut entering = catch.clone();
        entering.context.thrown.pop();
        entering.context.error_records.pop();
        entering.context.error_records.push(entered);
        // A `try` variable both paths hold: the types the catch knows it to
        // have are those of every value the body gives it, the last one
        // too, which the body no longer knows once it assigned one.
        let mut body = body.clone();
        let body_concrete = body.context.cell_concrete.clone();
        let context = &body.context;
        let cells: Vec<TraceCell> = (0..context.stack.len())
            .map(TraceCell::Stack)
            .chain(context.memory.keys().map(|slot| TraceCell::Memory(*slot)))
            .collect();
        let arena = &self.executor.arena;
        let held = |context: &TraceContext, cell: TraceCell| {
            let node = context.cell(cell)?.node?;
            match &arena.nodes.get(node as usize)?.kind {
                TraceNodeKind::Identifier { name } => Some(name.clone()),
                _ => None,
            }
        };
        let mut known = Vec::new();
        for cell in cells {
            let Some(name) = held(&body.context, cell) else {
                continue;
            };
            if held(&entering.context, cell).as_ref() != Some(&name) {
                continue;
            }
            let key = predicate::variable_key(&name);
            if body.context.type_facts.contains_key(&key) {
                continue;
            }
            if let Some(&types) = entering.context.type_facts.get(&key) {
                known.push((key, types));
            }
        }
        if !known.is_empty() {
            Arc::make_mut(&mut body.context.type_facts).extend(known);
        }
        // A join that fails may have assigned variables in the arms and
        // declared them: put the two statements back as they were.
        let statements = &self.executor.arena.statements;
        let saved = (
            statements[fork.statement as usize].kind.clone(),
            statements[fork.declare as usize].kind.clone(),
        );
        let joined = self.join(*fork, body, entering).and_then(|merged| {
            if self.executor.arm(catch_arm).is_some_and(Vec::is_empty) {
                Ok(merged)
            } else {
                Err(TraceJoinError(
                    "a finally's paths join with values only the throw sets".into(),
                ))
            }
        });
        let mut merged = match joined {
            Ok(merged) => merged,
            Err(error) => {
                let statements = &mut self.executor.arena.statements;
                statements[fork.statement as usize].kind = saved.0;
                statements[fork.declare as usize].kind = saved.1;
                return Err(error);
            }
        };
        if let TraceStatementKind::Try {
            catches, finalizer, ..
        } = &mut self.executor.arena.statements[fork.statement as usize].kind
        {
            *catches = false;
            *finalizer = Some(Vec::new());
        }
        if let Some(record) = merged.context.error_records.last_mut() {
            record.finally = Some(fork.statement);
        }
        // What each entry put in a shared cell where they put different
        // values, for opaque predicates in the `finally`.
        let (from_body, from_throw) = (&body_concrete, &catch.context.cell_concrete);
        let alternatives: BTreeMap<u32, (TraceNodeId, TraceNodeId)> = from_body
            .iter()
            .filter_map(|(slot, &node)| {
                let thrown = *from_throw.get(slot)?;
                (thrown != node && !merged.context.cell_concrete.contains_key(slot))
                    .then_some((*slot, (node, thrown)))
            })
            .collect();
        merged.context.finally_concrete = Arc::new(alternatives);
        let serial = self.executor.finally_joins;
        self.executor.finally_joins += 1;
        self.executor
            .finally_forks
            .insert(fork.statement, (*fork, serial));
        merged.block = Some(BlockTarget {
            statement: fork.statement,
            arm: BlockArm::Finally,
        });
        Ok(Some(merged))
    }

    /// The `try` frame the last handler popped, if it pushed an error record
    /// with where to go on as it did: it entered that frame's `finally`
    /// from its body, a `return` in it, say.
    #[must_use]
    pub fn finally_entered(&self) -> Option<(TryFrame, CallFrame)> {
        let executor = &self.executor;
        let frame = executor.popped_try?;
        let record = executor.context.error_records.last()?;
        let saved = record
            .saved
            .filter(|_| executor.pushed_finally && !record.handled)?;
        Some((frame, saved))
    }

    /// The memory slots the handler of a `try` wrote, when the last handler
    /// ended it rethrowing the exception (a `finally` traced after a throw,
    /// say): each slot whose value differs from the one the catch path
    /// started with.
    #[must_use]
    pub fn finally_writes(&self) -> Option<(TraceStatementId, BTreeSet<u32>)> {
        let executor = &self.executor;
        let record = executor.popped_error.filter(|record| record.handled)?;
        let statement = record.thrown_in?;
        let context = &executor.context;
        let entry = context.catch_memory.get(&statement)?;
        let same = |left: Option<&MemoryCell>, right: Option<&MemoryCell>| match (left, right) {
            (Some(MemoryCell::Value(left)), Some(MemoryCell::Value(right))) => {
                left.node == right.node
                    && match (&left.fact, &right.fact) {
                        (Some(left), Some(right)) => same_fact(left, right),
                        (None, None) => true,
                        _ => false,
                    }
            }
            (Some(MemoryCell::Initialized), Some(MemoryCell::Initialized))
            | (Some(MemoryCell::Param), Some(MemoryCell::Param))
            | (None, None) => true,
            _ => false,
        };
        let written = context
            .memory
            .keys()
            .chain(entry.keys())
            .copied()
            .filter(|slot| !same(entry.get(slot), context.memory.get(slot)))
            .collect();
        Some((statement, written))
    }

    /// Go on past the `finally` just entered, to a `return` of the value
    /// the body left: the emitted `return` runs the `finally` itself, the
    /// `try`'s handler traced after a throw (see `mark_finally`). The error
    /// record the `finally` pops at its end is dropped.
    pub fn skip_finally(&mut self) {
        self.executor.context.error_records.pop();
    }

    /// The `try` `statement`'s handler is its `finally`, traced after a
    /// throw: no `catch` (see [`TraceStatementKind::Try`]).
    pub fn mark_finally(&mut self, statement: TraceStatementId) {
        if let Some(TraceStatementKind::Try {
            catches, finalizer, ..
        }) = self
            .executor
            .arena
            .statements
            .get_mut(statement as usize)
            .map(|statement| &mut statement.kind)
        {
            if finalizer.is_none() {
                *catches = false;
            }
        }
    }

    /// Whether `fork`'s handler, emitted as its `finally` (no `catch` and no
    /// finalizer), reads the caught exception other than to rethrow it at
    /// its end: a `finally` has no exception to read, so the handler is a
    /// `catch`.
    #[must_use]
    pub fn finally_reads_exception(&self, fork: &TraceFork) -> bool {
        let arena = &self.executor.arena;
        let Some(TraceStatementKind::Try {
            param,
            handler,
            catches: false,
            finalizer: None,
            ..
        }) = arena
            .statements
            .get(fork.statement as usize)
            .map(|statement| &statement.kind)
        else {
            return false;
        };
        let rethrows = finally_rethrows(arena);
        let mut statements = handler.clone();
        let mut pending = Vec::new();
        while let Some(id) = statements.pop() {
            if rethrows.contains(&id) {
                continue;
            }
            let kind = &arena.statements[id as usize].kind;
            statements.extend(block_children(kind));
            match kind {
                TraceStatementKind::Let { value, .. }
                | TraceStatementKind::Assign { value, .. }
                | TraceStatementKind::Evaluate { value }
                | TraceStatementKind::Return { value }
                | TraceStatementKind::Throw { value }
                | TraceStatementKind::If { test: value, .. } => pending.push(*value),
                TraceStatementKind::PropertyWrite { object, key, value } => {
                    pending.extend([*object, *key, *value]);
                }
                _ => {}
            }
        }
        let mut seen = BTreeSet::new();
        while let Some(node) = pending.pop() {
            if !seen.insert(node) {
                continue;
            }
            let Some(kind) = arena.nodes.get(node as usize).map(|node| &node.kind) else {
                continue;
            };
            if matches!(kind, TraceNodeKind::Identifier { name } if name == param) {
                return true;
            }
            pending.extend(node_children(kind));
        }
        false
    }

    /// Whether `fork`'s `try` is emitted with a `finally` (see
    /// [`TraceStatementKind::Try`]).
    #[must_use]
    pub fn is_finally(&self, fork: &TraceFork) -> bool {
        matches!(
            self.executor
                .arena
                .statements
                .get(fork.statement as usize)
                .map(|statement| &statement.kind),
            Some(TraceStatementKind::Try { catches: false, .. })
        )
    }

    /// Whether the paths of `fork` joined in its `finally` (see
    /// [`Self::join_finally`]).
    #[must_use]
    pub fn joined_finally(&self, fork: &TraceFork) -> bool {
        self.executor.finally_forks.contains_key(&fork.statement)
    }

    /// The body path of `fork` popped its `try` frame: its later statements
    /// go after the `try`. Returns the cells the body wrote.
    pub fn close_try_body(
        &mut self,
        fork: &TraceFork,
        body: &mut TracePath,
    ) -> Result<BTreeSet<TraceCell>, TraceJoinError> {
        if body.block
            != Some(BlockTarget {
                statement: fork.statement,
                arm: BlockArm::TryBody,
            })
        {
            return Err(TraceJoinError(
                "a try block ends inside a nested block".into(),
            ));
        }
        self.executor
            .hoist(&mut body.context, body.block, fork.declare, fork.first_node)?;
        body.block = fork.parent;
        // Each cell the catch reads as a variable equals it here: read it
        // as the variable, keeping its known value, so a join with the
        // catch sees the same variable on both sides.
        let provenance = self.executor.driver_provenance();
        let variables = std::mem::take(&mut body.context.try_variables);
        for (cell, name) in Arc::unwrap_or_clone(variables) {
            let Some(stored) = body.context.cell(cell) else {
                continue;
            };
            let node = self
                .executor
                .identifier(&name, &provenance)
                .map_err(|_| TraceJoinError("a try variable cannot be read".into()))?;
            let value = match &stored.fact {
                Some(fact) => fact.clone(),
                None => SymbolicValue::Computed { node },
            };
            body.context.set_cell(
                cell,
                StoredValue {
                    value,
                    fact: stored.fact,
                    node: Some(node),
                },
            );
        }
        body.context.left_try = Some(BlockTarget {
            statement: fork.statement,
            arm: BlockArm::TryBody,
        });
        let context = &mut body.context;
        let changed = std::mem::take(&mut context.try_writes);
        let outer = Arc::make_mut(&mut context.try_outer)
            .pop()
            .unwrap_or_default();
        context.try_variables = outer.variables;
        context.try_entry = outer.entry;
        // What the body changed, the enclosing `try` body changed too.
        context.try_writes = outer.writes;
        if !changed.is_subset(&context.try_writes) {
            Arc::make_mut(&mut context.try_writes).extend(changed.iter().copied());
        }
        Ok(Arc::unwrap_or_clone(changed))
    }
}

/// The walk of [`TraceSession::try_entry_exposed`] for one variable.
struct EntryWalk<'a> {
    arena: &'a TraceArena,
    name: &'a str,
    /// Whether code that can throw ran while the variable could still hold
    /// its entry value.
    exposed: bool,
}

impl EntryWalk<'_> {
    /// Walks `block` with `entry` (whether the variable may still hold its
    /// entry value); returns whether it may after the block, on a path that
    /// goes on past it.
    fn block(&mut self, block: &[TraceStatementId], mut entry: bool) -> bool {
        for &id in block {
            let Some(after) = self.statement(id, entry) else {
                // This path ends here.
                return false;
            };
            entry = after;
        }
        entry
    }

    /// One statement: `None` when no path goes on after it.
    fn statement(&mut self, id: TraceStatementId, entry: bool) -> Option<bool> {
        let arena = self.arena;
        let pure = |node| is_pure_value(arena, node);
        match &arena.statements[id as usize].kind {
            TraceStatementKind::Let { value, .. } => {
                self.exposed |= entry && !pure(*value);
                Some(entry)
            }
            TraceStatementKind::Assign { name, value } => {
                self.exposed |= entry && !pure(*value);
                Some(entry && name != self.name)
            }
            TraceStatementKind::Declare { .. } | TraceStatementKind::Function { .. } => Some(entry),
            TraceStatementKind::Evaluate { .. } | TraceStatementKind::PropertyWrite { .. } => {
                self.exposed |= entry;
                Some(entry)
            }
            TraceStatementKind::Throw { .. } => {
                self.exposed |= entry;
                None
            }
            TraceStatementKind::Return { value } => {
                self.exposed |= entry && !pure(*value);
                None
            }
            TraceStatementKind::Continue { .. } | TraceStatementKind::Break { .. } => None,
            TraceStatementKind::If { test, taken, fall_through } => {
                self.exposed |= entry && !pure(*test);
                let taken = self.block(taken, entry);
                let fall_through = self.block(fall_through, entry);
                Some(taken || fall_through)
            }
            // The entry value never comes back once replaced: one walk of
            // the body covers every iteration; the loop may also not run.
            TraceStatementKind::Loop { body, .. } => {
                self.block(body, entry);
                Some(entry)
            }
            TraceStatementKind::Try { body, handler, catches, finalizer, .. } => {
                // The body's throws go to its own handler.
                let mut inner = EntryWalk { arena, name: self.name, exposed: false };
                let after_body = inner.block(body, entry);
                let handler_entry = inner.exposed;
                let after_handler = self.block(handler, handler_entry);
                let mut after = after_body;
                if *catches {
                    after |= after_handler;
                } else {
                    // A `finally` handler throws again when it ends.
                    self.exposed |= after_handler;
                }
                if let Some(finalizer) = finalizer {
                    after = self.block(finalizer, after || handler_entry);
                }
                Some(after)
            }
        }
    }
}
