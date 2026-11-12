use super::*;

impl TraceSession {
    /// Open `label: for (;;) { … }` at a loop header. Each `widened` cell
    /// changes between iterations: it becomes a variable declared before the
    /// loop with its current value, and the loop body sees the variable.
    pub fn open_loop(
        &mut self,
        widened: &BTreeSet<TraceCell>,
    ) -> Result<TraceLoop, TraceJoinError> {
        self.open_loop_with(
            widened,
            &BTreeMap::new(),
            &BTreeMap::new(),
            None,
            None,
            None,
        )
    }

    /// [`Self::open_loop`], with what the driver saw each loop variable hold
    /// at a `continue` (`None`: values that are not a known set). A loop
    /// variable whose entry value and every such value are concrete gets
    /// that value set; see [`Self::take_dispatch`].
    pub fn open_loop_with(
        &mut self,
        widened: &BTreeSet<TraceCell>,
        values: &BTreeMap<TraceCell, Option<Vec<SymbolicValue>>>,
        types: &BTreeMap<TraceCell, u16>,
        rewritten: Option<&BTreeMap<u32, CellMark>>,
        written: Option<&BTreeSet<u32>>,
        objects: Option<&BTreeSet<TraceNodeId>>,
    ) -> Result<TraceLoop, TraceJoinError> {
        let provenance = self.executor.driver_provenance();
        // A later iteration reaches the header after code that may have
        // written the cells. What the function itself last wrote there, a
        // VM position or not, holds unless an iteration writes it again
        // (`rewritten`, see `loop_cell_changes`; `None`: every cell).
        self.executor.context.cell_facts = Arc::default();
        self.executor.context.cell_nodes = Arc::default();
        // A made object an iteration writes or hands on holds other values
        // in a later one (`objects`, see `loop_object_writes`; `None`: any).
        match objects {
            None => self.executor.context.made_objects = Arc::default(),
            Some(objects) => {
                if self
                    .executor
                    .context
                    .made_objects
                    .keys()
                    .any(|object| objects.contains(object))
                {
                    Arc::make_mut(&mut self.executor.context.made_objects)
                        .retain(|object, _| !objects.contains(object));
                }
            }
        }
        // What the path put in a cell before the loop is not what a later
        // iteration finds there if any iteration writes it (`written`, see
        // `loop_cell_writes`; `None`: every cell).
        match written {
            None => self.executor.context.cell_concrete = Arc::default(),
            Some(written) => {
                if self
                    .executor
                    .context
                    .cell_concrete
                    .keys()
                    .any(|slot| written.contains(slot))
                {
                    Arc::make_mut(&mut self.executor.context.cell_concrete)
                        .retain(|slot, _| !written.contains(slot));
                }
            }
        }
        let context = &mut self.executor.context;
        match rewritten {
            None => {
                context.cell_positions = Arc::default();
                context.cell_values = Arc::default();
            }
            Some(rewritten) => {
                for (&slot, &mark) in rewritten {
                    let joined = CellMark::of(context, slot).join(mark);
                    joined.set(context, slot);
                }
            }
        }
        let mut variables = BTreeMap::new();
        let mut starts = BTreeMap::new();
        for &cell in widened {
            // Widened from another entry of the header, one with a longer
            // stack: not a cell of this one.
            let Some(stored) = self.executor.context.cell(cell) else {
                continue;
            };
            let set = match (self.executor.candidates(&stored, None), values.get(&cell)) {
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
            starts.insert(cell, node);
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
                .map_err(|_| TraceJoinError("a loop variable cannot be built".into()))?;
            executor.context.set_cell(
                cell,
                StoredValue {
                    value: SymbolicValue::Computed { node: variable },
                    fact: None,
                    node: Some(variable),
                },
            );
            Arc::make_mut(&mut executor.context.loop_variables).insert(name.clone(), cell);
            if let Some(set) = set.filter(|set| !set.is_empty() && set.len() <= MAX_VALUE_SET) {
                Arc::make_mut(&mut executor.context.value_sets).insert(name.clone(), set.into());
            }
            // Its value is the entry's or one an iteration carried (see
            // [`Self::loop_types`]): one of their types.
            if let Some(&seen) = types.get(&cell) {
                let entry = predicate::value_types(
                    &executor.arena,
                    &executor.aliases,
                    &executor.joins,
                    &executor.call_returns,
                    &executor.context.type_facts,
                    node,
                );
                Arc::make_mut(&mut executor.context.type_facts)
                    .insert(predicate::variable_key(&name), entry | seen);
            }
            variables.insert(cell, name);
        }
        let executor = &mut self.executor;
        let label = format!("vm_loop_{}", executor.loop_serial);
        executor.loop_serial += 1;
        let declare = executor.statement(
            TraceStatementKind::Declare { names: Vec::new() },
            provenance.clone(),
        );
        let statement = executor.statement(
            TraceStatementKind::Loop {
                label: label.clone(),
                body: Vec::new(),
            },
            provenance,
        );
        let first_node = executor.arena.nodes.len();
        let parent = executor.block;
        executor.block = Some(BlockTarget {
            statement,
            arm: BlockArm::LoopBody,
        });
        Ok(TraceLoop {
            label,
            parent,
            entry: executor.context.clone(),
            variables,
            starts,
            declare,
            first_node,
        })
    }

    /// The shared cells whose concrete value the header of `trace_loop` kept
    /// (see [`TraceContext::cell_concrete`]) that the running path wrote
    /// since: a later iteration finds what this one put there.
    #[must_use]
    pub fn loop_cell_writes(&self, trace_loop: &TraceLoop) -> BTreeSet<u32> {
        let (entry, now) = (&trace_loop.entry, &self.executor.context);
        // A cell a closure made in the iteration holds may change behind it.
        entry
            .cell_concrete
            .keys()
            .copied()
            .filter(|slot| {
                entry.cell_writes.get(slot) != now.cell_writes.get(slot)
                    || (now.escaped_cells.contains(slot) && !entry.escaped_cells.contains(slot))
            })
            .collect()
    }

    /// The made objects the header kept that the running path wrote or
    /// handed on since: a later iteration finds other values in them.
    #[must_use]
    pub fn loop_object_writes(&self, trace_loop: &TraceLoop) -> BTreeSet<TraceNodeId> {
        let (entry, now) = (&trace_loop.entry, &self.executor.context);
        if Arc::ptr_eq(&entry.made_objects, &now.made_objects) {
            return BTreeSet::new();
        }
        entry
            .made_objects
            .iter()
            .filter(|(object, properties)| now.made_objects.get(object) != Some(properties))
            .map(|(object, _)| *object)
            .collect()
    }

    /// The shared cells whose VM position, or whether they hold one, the
    /// running path changed since the header of `trace_loop`, with what the
    /// header then holds: the loop is traced again from that.
    #[must_use]
    pub fn loop_cell_changes(&self, trace_loop: &TraceLoop) -> BTreeMap<u32, CellMark> {
        let (entry, now) = (&trace_loop.entry, &self.executor.context);
        entry
            .cell_positions
            .keys()
            .chain(entry.cell_values.iter())
            .copied()
            .filter_map(|slot| {
                let at_entry = CellMark::of(entry, slot);
                let joined = at_entry.join(CellMark::of(now, slot));
                (joined != at_entry).then_some((slot, joined))
            })
            .collect()
    }

    /// The running path is back at the header of `trace_loop`: assign the
    /// loop variables and emit `continue label`. Returns the cells that
    /// changed but are not loop variables yet; the caller re-traces the loop
    /// with them widened.
    pub fn continue_loop(
        &mut self,
        trace_loop: &TraceLoop,
    ) -> Result<BTreeSet<TraceCell>, TraceJoinError> {
        let (entry, current) = (&trace_loop.entry, &self.executor.context);
        if entry.call_frames != current.call_frames || entry.receiver != current.receiver {
            return Err(TraceJoinError(
                "an iteration reaches the loop header with a different VM call stack".into(),
            ));
        }
        if entry.stack.len() != current.stack.len() {
            return Err(TraceJoinError(format!(
                "an iteration changes the stack length from {} to {}",
                entry.stack.len(),
                current.stack.len()
            )));
        }
        if !entry.memory.keys().eq(current.memory.keys()) {
            return Err(TraceJoinError(
                "an iteration changes the set of memory slots".into(),
            ));
        }
        let cells = (0..entry.stack.len())
            .map(TraceCell::Stack)
            .chain(entry.memory.keys().map(|slot| TraceCell::Memory(*slot)));
        let mut widen = BTreeSet::new();
        let mut assignments = Vec::new();
        for cell in cells {
            let (Some(before), Some(after)) = (entry.cell(cell), current.cell(cell)) else {
                continue;
            };
            if same_cell(&before, &after) {
                continue;
            }
            match trace_loop.variables.get(&cell) {
                Some(name) => assignments.push((name.clone(), after)),
                None => {
                    widen.insert(cell);
                }
            }
        }
        let provenance = self.executor.driver_provenance();
        let loop_variables: BTreeSet<&String> = trace_loop.variables.values().collect();
        let mut values = Vec::with_capacity(assignments.len());
        for (name, stored) in assignments {
            let executor = &mut self.executor;
            let node = executor.materialize(stored, &provenance)?;
            // Assignments are sequential: a value that reads another loop
            // variable is copied before any of them changes.
            let reads_variable = matches!(
                &executor.arena.nodes[node as usize].kind,
                TraceNodeKind::Identifier { name } if loop_variables.contains(name)
            );
            let node = if reads_variable {
                executor
                    .bind_value(computed(node), &provenance)
                    .map_err(|_| TraceJoinError("a loop value cannot be copied".into()))?
                    .1
                    .node
            } else {
                node
            };
            values.push((name, node));
        }
        for (name, value) in values {
            self.executor.statement(
                TraceStatementKind::Assign { name, value },
                provenance.clone(),
            );
        }
        self.executor.statement(
            TraceStatementKind::Continue {
                label: trace_loop.label.clone(),
            },
            provenance,
        );
        Ok(widen)
    }
}
