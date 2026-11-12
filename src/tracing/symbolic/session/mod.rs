use super::*;

/// Incremental owner of one persistent symbolic arena and [`TraceContext`].
/// This is the reusable execution boundary for bounded dispatch drivers.
///
/// Sessions are owned per tracing worker. A session is `Send`, so ownership can
/// move between threads, but it is not `Sync`: queries update internal caches
/// through shared references. Share completed reports or debug status when
/// observing work from another thread.
pub struct TraceSession {
    pub(super) executor: Executor,
}

impl TraceSession {
    #[must_use]
    pub fn new(context: TraceContext, limits: TraceLimits) -> Self {
        let mut session = Self::with_return_semantics(context, limits, true);
        session.check_initial_context(limits);
        session
    }

    /// Create a session whose handler `return` values are non-terminal: the
    /// returned expression is still evaluated and recorded, but the session
    /// stays runnable and the emitted JavaScript must not return early. This is
    /// the bounded dispatch continuation contract.
    #[must_use]
    pub fn new_continuing(context: TraceContext, limits: TraceLimits) -> Self {
        let mut session = Self::with_return_semantics(context, limits, false);
        session.check_initial_context(limits);
        session
    }

    /// Attach an owned live debug writer to this session.
    #[must_use]
    /// The live debug output, taken back for a session that replaces this
    /// one; this one goes on silent.
    pub(crate) fn take_debug(&mut self) -> TraceDebugOptions {
        let sink = &mut self.executor.debug;
        TraceDebugOptions {
            writer: sink.writer.take(),
            status: sink.status.clone(),
        }
    }

    pub fn with_debug(mut self, options: TraceDebugOptions) -> Self {
        self.executor.debug = DebugSink::from_options(options);
        if let Some((code, message)) = self
            .executor
            .diagnostics
            .last()
            .map(|diagnostic| (diagnostic.code, diagnostic.message.clone()))
        {
            self.executor.debug_stop(code, &message);
        }
        self
    }

    /// Register an immutable constant string table, such as the source-proven
    /// `vmStringTable`. A read `name[index]` with a concrete in-range index
    /// then resolves to that entry as a string literal; any other read of the
    /// table stays an abstract property read.
    #[must_use]
    pub fn with_constant_table(mut self, name: impl Into<String>, entries: Arc<[String]>) -> Self {
        self.executor.constant_tables.insert(name.into(), entries);
        self
    }

    /// Register an immutable number table: a read of it at a known index is
    /// that entry, and a read at an unknown or missing index stops.
    #[must_use]
    pub fn with_number_table(mut self, name: impl Into<String>, entries: Arc<[f64]>) -> Self {
        self.executor.number_tables.insert(name.into(), entries);
        self
    }

    /// Register the proven `vmDecodeString`: a call of it whose encoded
    /// string and key index are known is the string it returns, a literal.
    #[must_use]
    pub fn with_string_decoder(mut self, decoder: Arc<crate::vm_string::VmStringDecoder>) -> Self {
        self.executor.string_decoder = Some(decoder);
        self
    }

    /// Trace an interpreter whose `try` depth below zero traps (see
    /// [`Executor::try_depth_traps`]).
    #[must_use]
    pub fn with_try_depth_traps(mut self) -> Self {
        self.executor.try_depth_traps = true;
        self
    }

    /// How many `try` frames and error records the running path holds.
    #[must_use]
    pub fn exception_depths(&self) -> (usize, usize) {
        let context = &self.executor.context;
        (context.try_frames.len(), context.error_records.len())
    }

    /// Tell the session where the VM stands for the next handler: the PC after
    /// its operands and the dispatch state `dispatch()` already selected.
    /// Required before a handler that pushes a call frame; consumed by the
    /// next handler execution.
    /// Branches the user decides (see [`BranchOverrides`]).
    pub fn set_branch_overrides(&mut self, overrides: Arc<BranchOverrides>) {
        self.executor.overrides = overrides;
    }

    /// The position (`pc`, `state`) of the instruction the next handler
    /// runs: what [`BranchOverrides`] are looked up by.
    pub fn set_instruction_at(&mut self, at: (usize, usize)) {
        self.executor.instruction_at = Some(at);
    }

    pub fn set_vm_cursor(&mut self, cursor: VmCursor) {
        self.executor.cursor = Some(cursor);
    }

    /// The PC/state assignments made by the last handler, cleared on read.
    pub fn take_control_transfer(&mut self) -> ControlTransfer {
        std::mem::take(&mut self.executor.control)
    }

    /// The current VM call stack, innermost frame last.
    #[must_use]
    pub fn call_frames(&self) -> &[CallFrame] {
        &self.executor.context.call_frames
    }

    /// Tell the session what each shared cell of this function (by slot)
    /// can hold, from every function that holds it: a read with no value of
    /// its own known holds one of those addresses or, when the cell holds
    /// JavaScript values too, one of those.
    #[must_use]
    pub fn with_cell_contents(mut self, contents: BTreeMap<u32, CellContent>) -> Self {
        self.executor.cell_sets = contents
            .iter()
            .filter_map(|(&slot, content)| match CellReadEffect::of(Some(content)) {
                CellReadEffect::Addresses(addresses, _) => {
                    let set: Vec<SymbolicValue> = addresses
                        .iter()
                        .map(|&address| SymbolicValue::Number(address as f64))
                        .collect();
                    Some((slot, Arc::from(set)))
                }
                _ => None,
            })
            .collect();
        self.executor.cell_contents = contents;
        self
    }

    /// What this function put in each shared cell: `None` for a cell given a
    /// VM value it does not know.
    #[must_use]
    pub fn cell_writes(&self) -> &BTreeMap<u32, Option<CellContent>> {
        &self.executor.cell_writes
    }

    /// Tell the session which slots each thread descriptor captures, so a
    /// created closure is handed its cells; see [`Self::open_cells`].
    #[must_use]
    pub fn with_captures(mut self, captures: Arc<[Vec<u32>]>) -> Self {
        self.executor.captures = Some(captures);
        self
    }

    /// The slots a created closure captured that were not cells, cleared
    /// on read: the driver traces the function again with them as cells.
    /// Only a closure the trace still makes counts: one a side the walk
    /// went back from made (a junk side, say) shares nothing.
    pub fn take_wanted_cells(&mut self) -> BTreeSet<u32> {
        let wanted = std::mem::take(&mut self.executor.wanted_cells);
        let (Some(captures), false) = (&self.executor.captures, wanted.is_empty()) else {
            return wanted;
        };
        let arena = &self.executor.arena;
        let live = arena.live_nodes();
        arena
            .nodes
            .iter()
            .filter(|node| live[node.id as usize])
            .filter_map(|node| match node.kind {
                TraceNodeKind::VmFunction { descriptor, .. } => captures.get(descriptor),
                _ => None,
            })
            .flatten()
            .copied()
            .filter(|slot| wanted.contains(slot))
            .collect()
    }

    /// Tell `created` each VM function the trace creates (see
    /// [`CreatedHook`]).
    #[must_use]
    pub fn with_created_hook(mut self, created: Option<CreatedHook>) -> Self {
        self.executor.created_hook = created;
        self
    }

    /// Ask `returns` what a call of a VM function made here returns.
    #[must_use]
    pub fn with_returns_hook(mut self, returns: Option<ReturnsHook>) -> Self {
        self.executor.returns_hook = returns;
        self
    }

    /// Ask `types` what the closures made here put in the cells they
    /// capture (see [`CellTypesHook`]).
    #[must_use]
    pub fn with_cell_types_hook(mut self, types: Option<CellTypesHook>) -> Self {
        self.executor.cell_types_hook = types;
        self
    }

    /// The JavaScript types (predicate type bits) this walk put in each
    /// shared cell, by slot, in any attempt: every type where it put a value
    /// of a type it does not know.
    #[must_use]
    pub fn cell_types(&self) -> &BTreeMap<u32, u16> {
        &self.executor.cell_types
    }

    /// What the walk used of the cells' contents (see
    /// [`Self::with_cell_contents`]), in every attempt.
    #[must_use]
    pub fn cell_reads(&self) -> CellReads {
        CellReads {
            cells: self.executor.consulted.clone(),
            data: self.executor.data_consulted.borrow().clone(),
            sets: self.executor.set_consulted.borrow().clone(),
        }
    }

    /// Make `cells` shared cells, as `vmCaptureThread` shares
    /// `memory.slots[slot]` objects between a VM function and the closures
    /// it creates: `let $CELL_n;` for each, and for one that exists now
    /// `$CELL_n = captured` (a cell the caller passed, for a slot in
    /// `captured` still holding its incoming value) or `{ value }`. From then
    /// on the slot is read as `$CELL_n.value` and written as `$CELL_n.value
    /// = v`, and a new cell for it is a new `{ value }`. Must come before the
    /// first step.
    pub fn open_cells(&mut self, cells: &BTreeSet<u32>, captured: &BTreeMap<u32, SymbolicValue>) {
        let executor = &mut self.executor;
        executor.cells = cells.clone();
        executor.captured_cells = captured.keys().copied().collect();
        if cells.is_empty() {
            return;
        }
        let provenance = executor.driver_provenance();
        executor.statement(
            TraceStatementKind::Declare {
                names: cells
                    .iter()
                    .map(|slot| Executor::cell_variable(*slot))
                    .collect(),
            },
            provenance.clone(),
        );
        for &slot in cells {
            let Some(stored) = executor.context.cell(TraceCell::Memory(slot)) else {
                continue;
            };
            let passed = captured.get(&slot).is_some_and(|incoming| {
                matches!(incoming, SymbolicValue::External { .. }) && stored.value == *incoming
            });
            let value = stored.fact.clone();
            if !passed {
                // What the cell starts with, it holds too.
                let start = EvalValue {
                    value: stored.value.clone(),
                    fact: stored.fact.clone(),
                    node: stored.node.unwrap_or(u32::MAX),
                };
                // An argument is a JavaScript value its caller passed.
                let (content, types) = match &start.value {
                    SymbolicValue::External { .. } if stored.node.is_none() => (
                        Some(CellContent {
                            addresses: BTreeSet::new(),
                            data: true,
                        }),
                        predicate::ANY_TYPES,
                    ),
                    _ => (executor.cell_content_of(&start), executor.types_of(&start)),
                };
                executor.note_cell_types(slot, types);
                executor.note_cell_content(slot, content);
            }
            let cell = if passed {
                executor
                    .value_node(stored.value.clone(), &provenance)
                    .map(|value| value.node)
            } else {
                executor
                    .materialize(stored, &provenance)
                    .map_err(|_| ExecutionStop::Budget("node budget exhausted".into(), None))
                    .and_then(|value| executor.new_cell(value, &provenance))
            };
            // Out of nodes: the first step stops on the same budget.
            let Ok(cell) = cell else {
                return;
            };
            // Its own slot starts with the value it holds (`undefined`, say);
            // one its creator passed holds what the creator put there.
            let start = if passed { None } else { value.clone() };
            executor.assign_cell(slot, cell, start, &provenance);
            if passed {
                Arc::make_mut(&mut executor.context.escaped_cells).insert(slot);
            } else if value.is_some_and(|value| !matches!(value, SymbolicValue::Number(_))) {
                // Its own slot's value, `undefined` say: no VM position.
                Arc::make_mut(&mut executor.context.cell_values).insert(slot);
            }
            executor
                .context
                .memory_mut()
                .insert(slot, MemoryCell::Initialized);
        }
    }

    /// Let the driver walk VM threads. A program-value conditional jump then
    /// forks the trace instead of stopping it: the driver takes each
    /// [`PendingBranch`] and traces both successors with [`Self::fork`],
    /// [`Self::switch_path`] and [`Self::join`]. A VM function return
    /// (`vmPendingResult = value`) emits `return value` and the driver ends
    /// that path when [`Self::take_thread_return`] reports it.
    #[must_use]
    pub fn with_thread_control(mut self) -> Self {
        self.executor.thread_control = true;
        self
    }

    /// Record that the conditional jump on `test` is a fake branch whose
    /// other side reaches a state real code never reaches; only the `taken`
    /// side is traced.
    pub fn record_fake_branch(&mut self, test: TraceNodeId, taken: bool) {
        let executor = &mut self.executor;
        if let Some((key, negated)) = predicate::test_key(&executor.arena, &executor.aliases, test)
        {
            Arc::make_mut(&mut executor.context.branch_facts).insert(key, taken ^ negated);
        }
        predicate::learn_type(
            &executor.arena,
            &executor.aliases,
            &executor.call_returns,
            &executor.concrete_reads,
            &mut executor.context.type_facts,
            test,
            taken,
        );
        let mut context = executor.context.clone();
        Self::learn_from_join(executor, &mut context, test, taken);
        executor.context = context;
        let provenance = self.executor.driver_provenance();
        self.executor.event(
            TraceEventKind::BranchDecided {
                test,
                taken,
                by: BranchEvidence::Context,
            },
            provenance,
        );
    }

    /// The last diagnostic: why the session stopped, once it has.
    #[must_use]
    pub fn last_diagnostic(&self) -> Option<&TraceDiagnostic> {
        self.executor.diagnostics.last()
    }

    /// The value the last handler made the VM function return, cleared on
    /// read.
    pub fn take_thread_return(&mut self) -> Option<TraceValue> {
        self.executor.thread_return.take()
    }

    /// The program-value conditional jump of the last handler, cleared on
    /// read.
    pub fn take_branch(&mut self) -> Option<PendingBranch> {
        self.executor.branch.take()
    }

    /// Open `if (test)` in the current block and return the two successors,
    /// each with a copy of the current VM state. The session keeps running
    /// the current path until the driver switches to one of them.
    pub fn fork(&mut self, test: TraceNodeId) -> (TraceFork, TracePath, TracePath) {
        let executor = &mut self.executor;
        let provenance = executor.arena.nodes[test as usize].provenance.clone();
        let declare = executor.statement(
            TraceStatementKind::Declare { names: Vec::new() },
            provenance.clone(),
        );
        let statement = executor.statement(
            TraceStatementKind::If {
                test,
                taken: Vec::new(),
                fall_through: Vec::new(),
            },
            provenance,
        );
        // Each side knows how the test came out.
        let key = predicate::test_key(&executor.arena, &executor.aliases, test);
        let path = |arm, truthy: bool| {
            let mut context = executor.context.clone();
            if let Some((key, negated)) = &key {
                Arc::make_mut(&mut context.branch_facts).insert(key.clone(), truthy ^ negated);
            }
            predicate::learn_type(
                &executor.arena,
                &executor.aliases,
                &executor.call_returns,
                &executor.concrete_reads,
                &mut context.type_facts,
                test,
                truthy,
            );
            Self::learn_from_join(executor, &mut context, test, truthy);
            TracePath {
                context,
                block: Some(BlockTarget { statement, arm }),
            }
        };
        let taken = path(BlockArm::Taken, true);
        let fall_through = path(BlockArm::FallThrough, false);
        (
            TraceFork {
                statement,
                declare,
                first_node: executor.arena.nodes.len(),
                first_binding: executor.binding_serial,
                parent: executor.block,
                arms: (BlockArm::Taken, BlockArm::FallThrough),
            },
            taken,
            fall_through,
        )
    }

    /// The `try` the last handler entered, cleared on read. The driver then
    /// opens it with [`Self::fork_try`].
    pub fn take_try_entry(&mut self) -> Option<TryFrame> {
        self.executor.try_entered.take()
    }

    /// The entries (byte offset, dispatch state) of the VM functions the
    /// last handler created, cleared on read.
    pub fn take_created_now(&mut self) -> Vec<(usize, usize, usize)> {
        std::mem::take(&mut self.executor.created_now)
    }

    /// The number of active `try` frames on the running path.
    #[must_use]
    pub fn try_depth(&self) -> usize {
        self.executor.context.try_frames.len()
    }
}

mod branches;
mod convergence;
mod lifecycle;
mod loops;
mod subroutines;
mod try_blocks;

pub use branches::StackPadding;
pub use lifecycle::*;
