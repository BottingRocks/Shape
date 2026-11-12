use super::*;
#[derive(Debug, Clone)]
pub(super) struct EvalValue {
    pub(super) value: SymbolicValue,
    pub(super) fact: Option<SymbolicValue>,
    pub(super) node: TraceNodeId,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct EvalIndex {
    pub(super) value: usize,
    pub(super) node: TraceNodeId,
}

/// Facts a rollback keeps, saved by [`TraceSession::learned_facts`].
pub(crate) struct TraceFacts(pub(super) Box<Facts>);

pub(super) struct Facts {
    pub(super) marked: BTreeSet<String>,
    pub(super) used_sets: Vec<String>,
    pub(super) cells: BTreeSet<u32>,
    pub(super) wanted_cells: BTreeSet<u32>,
    pub(super) try_body_writes: BTreeMap<usize, BTreeSet<TraceCell>>,
    pub(super) try_body_cell_writes: BTreeMap<usize, BTreeSet<u32>>,
    pub(super) cell_writes: BTreeMap<u32, Option<CellContent>>,
    pub(super) cell_types: BTreeMap<u32, u16>,
    pub(super) cell_holders: BTreeMap<u32, BTreeSet<(usize, usize, usize)>>,
    pub(super) cell_contents: BTreeMap<u32, CellContent>,
    pub(super) instruction_reads: BTreeMap<u32, u32>,
    pub(super) constant_tables: BTreeMap<String, Arc<[String]>>,
    pub(super) steps: usize,
    pub(super) handlers_traced: usize,
}

#[derive(Debug, Clone)]
pub(super) struct Checkpoint {
    pub(super) nodes: usize,
    pub(super) statements: usize,
    pub(super) events: usize,
    pub(super) context: TraceContext,
    pub(super) diagnostics: usize,
    pub(super) external_inputs: Arc<BTreeSet<String>>,
    pub(super) binding_serial: usize,
    pub(super) completion: TraceCompletion,
    /// The current block and its length.
    pub(super) block: Option<BlockTarget>,
    pub(super) block_length: usize,
    pub(super) loop_serial: usize,
    pub(super) finally_joins: usize,
}

pub(super) enum HandlerOutcome {
    Continue,
    Return(TraceValue),
    Throw(TraceValue),
}
pub(super) enum ExecutionStop {
    Budget(String, Option<TraceProvenance>),
    Unsupported(TraceStopCode, String, Option<TraceProvenance>),
    /// The handler throws this value, as the VM does where a handler
    /// touches a memory slot the thread has none of.
    Throws(EvalValue),
}

pub(super) fn finish_error(stop: ExecutionStop) -> TraceFinishError {
    match stop {
        ExecutionStop::Budget(message, _) => TraceFinishError::ValueGraphUnavailable(message),
        ExecutionStop::Unsupported(code, message, _) => {
            TraceFinishError::Unsupported { code, message }
        }
        ExecutionStop::Throws(_) => TraceFinishError::Unsupported {
            code: TraceStopCode::UnsupportedHandler,
            message: "the handler throws".into(),
        },
    }
}

pub(super) struct Executor {
    /// Each handler's recipe check (see `preflight`), by template.
    pub(super) preflights: HashMap<(usize, u32, usize, usize), Option<String>>,
    pub(super) context: TraceContext,
    pub(super) limits: TraceLimits,
    pub(super) arena: TraceArena,
    pub(super) diagnostics: Vec<TraceDiagnostic>,
    pub(super) completion: TraceCompletion,
    pub(super) steps: usize,
    pub(super) handlers_traced: usize,
    /// Shared with checkpoints until written.
    pub(super) external_inputs: Arc<BTreeSet<String>>,
    pub(super) temps: BTreeSet<u32>,
    pub(super) temp_values: BTreeMap<u32, EvalValue>,
    pub(super) binding_serial: usize,
    pub(super) current: Option<TraceProvenance>,
    /// When true a recipe `return` emits a terminal `return` statement and is
    /// reported as a terminal outcome (legacy [`trace_handlers`] semantics).
    /// When false the returned expression is still evaluated and retained as an
    /// `Evaluate` statement, but it does not terminate the emitted JavaScript
    /// and the session remains runnable (dispatch continuation semantics).
    pub(super) handler_return_terminal: bool,
    /// Immutable constant string tables keyed by binding name.
    pub(super) constant_tables: BTreeMap<String, Arc<[String]>>,
    /// Immutable constant number tables keyed by binding name: a read at a
    /// known index is that number, and any other read stops.
    pub(super) number_tables: BTreeMap<String, Arc<[f64]>>,
    /// Whether each parameter of each outlined function, by name, is handed
    /// on or written by its body (see `SubroutineSummary::param_escapes`).
    pub(super) subroutine_escapes: BTreeMap<String, Arc<[bool]>>,
    /// The proven `vmDecodeString` (see [`TraceSession::with_string_decoder`]).
    pub(super) string_decoder: Option<Arc<crate::vm_string::VmStringDecoder>>,
    /// Whether the interpreter traps on a `try` entered or a throw after
    /// more `try` exits than entries, as the WASM one does (see
    /// [`TraceContext::try_underflow`]). A JavaScript VM's
    /// `tryFrameStack.pop()` of an empty stack does nothing.
    pub(super) try_depth_traps: bool,
    /// Shared cells this function wrote a VM Worker's message data to: a
    /// read of one it does not know is that data (see
    /// [`Self::from_message`]).
    pub(super) message_cells: BTreeSet<u32>,
    pub(super) debug: DebugSink,
    /// The VM position for the handler about to run, set by the driver.
    pub(super) cursor: Option<VmCursor>,
    /// PC/state assignments made by the current handler.
    pub(super) control: ControlTransfer,
    /// The call frame popped by the current handler.
    pub(super) popped_frame: Option<CallFrame>,
    /// Whether the driver walks VM threads: it traces both successors of a
    /// program-value conditional jump and ends a path at a VM function
    /// return. Without it either stops the trace.
    pub(super) thread_control: bool,
    /// The program-value conditional jump of the current handler.
    pub(super) branch: Option<PendingBranch>,
    /// The value the current handler makes the VM function return.
    pub(super) thread_return: Option<TraceValue>,
    /// The block new statements go to; `None` is the function body.
    pub(super) block: Option<BlockTarget>,
    /// Loops opened so far, for unique labels.
    pub(super) loop_serial: usize,
    /// Each `let`-bound identifier node and the value node it names, for
    /// deciding branch tests.
    pub(super) aliases: NodeMap<TraceNodeId>,
    /// Each read of a shared cell and what the path concretely last put
    /// there (see [`TraceContext::cell_concrete`]), for opaque-predicate
    /// tests only.
    pub(super) concrete_reads: BTreeMap<TraceNodeId, TraceNodeId>,
    /// Each read in a `finally` of a cell its two entries put different
    /// values in (see `TraceContext::finally_concrete`), with both: an
    /// opaque predicate is decided only where both decide it alike.
    pub(super) finally_reads: Vec<(TraceNodeId, TraceNodeId, TraceNodeId)>,
    /// Counts shared-cell writes (see [`TraceContext::cell_writes`]).
    pub(super) write_serial: u64,
    /// What each side of a join knew, by the join's variable node.
    pub(super) join_arms: BTreeMap<TraceNodeId, Vec<JoinArm>>,
    /// Each read of a shared cell that holds a VM position this path wrote
    /// there (see [`TraceContext::cell_positions`]), by read node.
    pub(super) position_reads: BTreeMap<TraceNodeId, usize>,
    /// What each call of an outlined function returns, by call node: its
    /// return values with the call's arguments for its parameters, for
    /// tests to be decided on without tracing the call inline.
    pub(super) call_returns: BTreeMap<TraceNodeId, Vec<TraceNodeId>>,
    /// Each call of an outlined function that returns one of its
    /// parameters (see `SubroutineSummary::returns_param`), by call node,
    /// with the argument passed there: the call's result is that object.
    pub(super) returned_arguments: BTreeMap<TraceNodeId, TraceNodeId>,
    /// The call sites, by tag, whose returns are in `call_returns`.
    pub(super) substituted: BTreeSet<usize>,
    /// Results of outlined subroutine calls, by binding, with the caller's
    /// tag for the call: traced inline, a result might be concrete or of a
    /// known type. Bindings are unique, so one map serves every path.
    pub(super) outlined: BTreeMap<String, usize>,
    /// Each variable a join made (its identifier node) and the value node
    /// each arm assigned it, for deciding branch tests on it.
    pub(super) joins: BTreeMap<TraceNodeId, Vec<TraceNodeId>>,
    /// The names a test may look for among what it is computed from (see
    /// [`Self::depends_on`]): those bound to an outlined call's result or to
    /// a join of concrete values.
    pub(super) marked: BTreeSet<String>,
    /// For each node, whether it is computed from a marked name, once
    /// worked out (what a node is computed from is older than it, so this
    /// never changes): the search for one skips the nodes that are not.
    pub(super) reaches_marked: std::cell::RefCell<Vec<Option<bool>>>,
    /// Each variable carried out of a block a path left early, and that
    /// block.
    pub(super) carried_from: BTreeMap<TraceNodeId, BlockTarget>,
    /// Set when a VM jump went straight to a program value carried out of a
    /// block: the block the value was carried out of first. See
    /// [`TraceSession::take_junk_origin`].
    pub(super) junk_origin: Option<BlockTarget>,
    /// Set when a VM value had to be concrete but was a merge of concrete
    /// values; see [`TraceSession::take_split`].
    pub(super) split: Option<MergedConcrete>,
    /// Set when a VM value had to be concrete but came from an outlined
    /// call; see [`TraceSession::take_inline`].
    pub(super) inline: Option<usize>,
    /// Set when a VM jump target or state was a variable with a known value
    /// set; see [`TraceSession::take_dispatch`].
    pub(super) dispatch: Option<ValueDispatch>,
    /// Variables whose value set a VM value was taken from since the last
    /// [`TraceSession::take_used_sets`].
    pub(super) used_sets: Vec<String>,
    /// The try frame and error record popped by the current handler.
    pub(super) popped_try: Option<TryFrame>,
    /// The last handler pushed an error record for a normally entered
    /// `finally` (see `TraceSession::finally_entered`).
    pub(super) pushed_finally: bool,
    pub(super) popped_error: Option<ErrorRecord>,
    /// The `try` the current handler entered, for the driver.
    pub(super) try_entered: Option<TryFrame>,
    /// Slots whose cell another VM function may hold (see
    /// [`TraceSession::open_cells`]): read and written through `$CELL_n`.
    pub(super) cells: BTreeSet<u32>,
    /// The slots each thread descriptor captures, when the driver knows.
    pub(super) captures: Option<Arc<[Vec<u32>]>>,
    /// See [`CreatedHook`].
    pub(super) created_hook: Option<CreatedHook>,
    /// See [`ReturnsHook`].
    pub(super) returns_hook: Option<ReturnsHook>,
    /// See [`CellTypesHook`].
    pub(super) cell_types_hook: Option<CellTypesHook>,
    /// The entries (byte offset, dispatch state) of the VM functions the
    /// running handler created.
    pub(super) created_now: Vec<(usize, usize, usize)>,
    /// Slots a created closure captures that are not cells yet; see
    /// [`TraceSession::take_wanted_cells`].
    pub(super) wanted_cells: BTreeSet<u32>,
    /// Every shared cell the walk read, in any attempt: what it was told of
    /// their contents is all it used of them. Kept across rollbacks.
    pub(super) consulted: BTreeSet<u32>,
    /// The shared cells whose reads were asked whether they are program
    /// data (see `data_reads`), in any attempt, kept across rollbacks; and
    /// the cell each read node with no value of its own known read, which a
    /// rollback forgets for the nodes it frees.
    pub(super) data_consulted: std::cell::RefCell<BTreeSet<u32>>,
    pub(super) read_slots: BTreeMap<TraceNodeId, u32>,
    /// The same for value sets: the shared cells whose reads were asked
    /// their value set, kept across rollbacks; and the cell each variable
    /// read with no value of its own known, which a rollback that numbers
    /// variables again forgets for the names it frees.
    pub(super) set_consulted: std::cell::RefCell<BTreeSet<u32>>,
    pub(super) read_names: BTreeMap<String, u32>,
    /// Set while the executor emits a cell's own read or write, which runs
    /// no other code.
    pub(super) cell_access: bool,
    /// Branches the user decides, and the position (`pc`, `state`) of the
    /// instruction running, which they are looked up by.
    pub(super) overrides: Arc<BranchOverrides>,
    pub(super) instruction_at: Option<(usize, usize)>,
    /// Set while a VM value is taken from a variable's complete value set.
    pub(super) from_value_set: bool,
    /// The jump target just read is an address only the `try` body wrote
    /// (see `jump_sets`): the state the same jump reads is the body's too.
    pub(super) body_address: bool,
    /// The jump target just read is the entry value of a `try` variable
    /// whose body writes VM addresses, one the body never wrote: the state
    /// the same jump reads is the entry's too.
    pub(super) entry_address: bool,
    /// The join whose padded stack a step found counted from the bottom
    /// (see `TraceSession::take_pad_conflict`).
    pub(super) pad_conflict: Option<(usize, bool)>,
    /// The running handler only cuts the stack to a fixed length (see
    /// `StackAccess::Reset`).
    pub(super) stack_reset: bool,
    /// Every cell written while each `try` depth was active, on any path,
    /// even one that ended since: a throw after the write reaches that
    /// `try`'s catch with it. See [`TraceSession::take_try_body_writes`].
    pub(super) try_body_writes: BTreeMap<usize, BTreeSet<TraceCell>>,
    /// The shared cells written while each `try` depth was active, the same
    /// way. See [`TraceSession::take_try_body_cell_writes`].
    pub(super) try_body_cell_writes: BTreeMap<usize, BTreeSet<u32>>,
    /// What this function put in each shared cell, by slot: `None` once it
    /// put a value that is neither a constant nor a JavaScript value (a VM
    /// value it does not know). See [`TraceSession::cell_writes`].
    pub(super) cell_writes: BTreeMap<u32, Option<CellContent>>,
    /// The JavaScript types (predicate type bits) this function put in
    /// each shared cell, by slot, in any attempt; every type for a value
    /// whose type is not known. See [`TraceSession::cell_types`].
    pub(super) cell_types: BTreeMap<u32, u16>,
    /// The closures (descriptor, byte offset, dispatch state) seen holding
    /// each shared cell a closed-cell test was decided on, in any attempt:
    /// a rollback drops the nodes of the closures an attempt made, not
    /// this. See [`Self::closed_cell_null_test`].
    pub(super) cell_holders: BTreeMap<u32, BTreeSet<(usize, usize, usize)>>,
    /// The slots whose cells the caller passed (captured by the function's
    /// descriptor): other functions made before this one may hold them.
    pub(super) captured_cells: BTreeSet<u32>,
    /// What each shared cell of this function can hold, from every function
    /// holding it, when the driver knows; see [`TraceSession::with_cell_contents`].
    pub(super) cell_contents: BTreeMap<u32, CellContent>,
    /// Reads of a cell that holds only JavaScript values: a VM jump to one
    /// went to a program value.
    pub(super) data_reads: BTreeSet<TraceNodeId>,
    /// Variables read from a cell that may hold a JavaScript value besides
    /// the addresses in their value set: a jump through one tests every
    /// address, and matching none is a jump to a program value.
    /// The parameters of the catches traced: what a VM `catch` binds is a
    /// thrown JavaScript value.
    /// Each generated variable's definitions: the values its `let` and
    /// every assignment give it, with their statements.
    pub(super) definitions: Definitions,
    pub(super) catch_params: BTreeSet<String>,
    /// The `try` of each traced `finally`, by statement (see
    /// [`ErrorRecord::finally`]), with the serial of the join of its paths:
    /// a rollback to before it undoes the join.
    pub(super) finally_forks: BTreeMap<TraceStatementId, (TraceFork, usize)>,
    /// How many `finally` joins and ends there were (see `finally_forks`
    /// and `finally_exits`).
    pub(super) finally_joins: usize,
    /// Each block a path went on in when a `finally` ended, with its length
    /// then and the serial of that end: a rollback to before it takes back
    /// what the block got since, which the rollback's own block does not
    /// hold.
    pub(super) finally_exits: Vec<(usize, Option<BlockTarget>, usize)>,
    pub(super) open_sets: BTreeSet<String>,
    /// Each variable a cell read with no value of its own known is bound
    /// to, the read: it holds one of the cell's addresses. The flag tells
    /// whether no code that may run other code came before the read in
    /// its instruction, so a test on the cell before the instruction tests
    /// what the read gets.
    pub(super) cell_reads: BTreeMap<String, (CellRead, bool)>,
    /// How many times the running instruction read each shared cell.
    pub(super) instruction_reads: BTreeMap<u32, u32>,
    /// Whether the running instruction has run code that may run other
    /// code.
    pub(super) instruction_ran_code: bool,
    /// The addresses each cell can hold, as a value set, by slot.
    pub(super) cell_sets: BTreeMap<u32, Arc<[SymbolicValue]>>,
}

impl Executor {
    /// Whether `node` is read out of an argument the thread was called with
    /// and is not a known value: a VM Worker's message data, which its
    /// message handler is given. Another unknown value (a captured cell, the
    /// receiver) is none.
    pub(super) fn from_message(&self, node: TraceNodeId) -> bool {
        let mut node = self.resolve_copies(node);
        loop {
            match self.arena.nodes.get(node as usize).map(|node| &node.kind) {
                Some(TraceNodeKind::External { name }) => return is_argument_input(name),
                // The cell it kept message data in.
                Some(TraceNodeKind::PropertyRead { object, .. })
                    if matches!(self.arena.nodes.get(*object as usize).map(|node| &node.kind),
                        Some(TraceNodeKind::Identifier { name })
                            if name.strip_prefix("$CELL_").and_then(|slot| slot.parse().ok())
                                .is_some_and(|slot: u32| self.message_cells.contains(&slot))) =>
                {
                    return true
                }
                Some(TraceNodeKind::PropertyRead { object, .. }) => {
                    node = self.resolve_copies(*object)
                }
                _ => return false,
            }
        }
    }

    /// Whether the statement only reads a property of a VM Worker's message:
    /// `event.data` is the platform's, and data that crossed a Worker is
    /// structured-cloned, with no getters to run.
    pub(super) fn reads_message(&self, kind: &TraceStatementKind) -> bool {
        let (TraceStatementKind::Let { value, .. } | TraceStatementKind::Assign { value, .. }) =
            kind
        else {
            return false;
        };
        matches!(self.arena.nodes.get(*value as usize).map(|node| &node.kind),
            Some(TraceNodeKind::PropertyRead { object, .. }) if self.from_message(*object))
    }

    /// A post to the VM Worker names the function it runs there: descriptor
    /// `arguments[3]` at pc `arguments[1]`, state `arguments[0]`. It is a
    /// function the program has, so it is created here; one that shares
    /// cells cannot share them across the Worker, and one the trace cannot
    /// name is not a function it has.
    pub(super) fn posted_to_worker(
        &mut self,
        arguments: &[EvalValue],
        p: &TraceProvenance,
    ) -> Result<(), ExecutionStop> {
        let number = |value: Option<&EvalValue>| -> Option<usize> {
            match value?.fact.as_ref()? {
                SymbolicValue::Number(value) if *value >= 0.0 && value.fract() == 0.0 => {
                    Some(*value as usize)
                }
                _ => None,
            }
        };
        let (Some(state), Some(pc), Some(descriptor)) = (
            number(arguments.first()),
            number(arguments.get(1)),
            number(arguments.get(3)),
        ) else {
            return Err(self.unsupported_stop(
                TraceStopCode::DynamicIndex,
                "a post to the VM Worker names a function the trace does not know".into(),
                p.clone(),
            ));
        };
        let shares = self
            .captures
            .as_ref()
            .and_then(|captures| captures.get(descriptor))
            .is_some_and(|slots| !slots.is_empty());
        if shares {
            return Err(self.unsupported_stop(
                TraceStopCode::DynamicIndex,
                format!("a post to the VM Worker runs descriptor {descriptor}, which shares cells"),
                p.clone(),
            ));
        }
        if let Some(created) = &self.created_hook {
            created(descriptor, pc, state);
        }
        Ok(())
    }

    pub(super) fn constant_entry(&self, object: &SemanticValue, key: &EvalValue) -> Option<String> {
        let SemanticValue::Binding { name } = object else {
            return None;
        };
        let table = self.constant_tables.get(name)?;
        let Some(SymbolicValue::Number(index)) = key.fact.as_ref() else {
            return None;
        };
        if !index.is_finite() || *index < 0.0 || index.fract() != 0.0 {
            return None;
        }
        table.get(*index as usize).cloned()
    }

    /// A concrete index past the end of the constant table `object`: the
    /// bytecode's operands index the table, so no code the VM runs reads it.
    fn past_constant_table(&self, object: &SemanticValue, key: &EvalValue) -> Option<String> {
        let SemanticValue::Binding { name } = object else {
            return None;
        };
        let table = self.constant_tables.get(name)?;
        match key.fact.as_ref() {
            Some(SymbolicValue::Number(index))
                if index.fract() == 0.0 && *index >= table.len() as f64 =>
            {
                Some(format!("{name}[{index}] is past its {} entries", table.len()))
            }
            _ => None,
        }
    }

    /// A path leaves the block `inner` for the block around a statement
    /// whose first node inside is `first_node`: values bound inside are not
    /// in scope there. A known value becomes itself again; another value
    /// made inside becomes a variable named in `declare` (before the
    /// statement) and assigned at the end of `inner`.
    pub(super) fn hoist(
        &mut self,
        context: &mut TraceContext,
        inner: Option<BlockTarget>,
        declare: TraceStatementId,
        first_node: usize,
    ) -> Result<(), TraceJoinError> {
        let mut cells: Vec<(TraceCell, StoredValue)> = context
            .stack
            .iter()
            .enumerate()
            .map(|(index, cell)| (TraceCell::Stack(index), stack_cell_value(cell)))
            .chain(context.memory.iter().filter_map(|(slot, cell)| match cell {
                MemoryCell::Value(_) | MemoryCell::Param => {
                    Some((TraceCell::Memory(*slot), memory_cell_value(cell)))
                }
                MemoryCell::Initialized => None,
            }))
            .collect();
        let thrown = context.thrown.clone();
        let provenance = self.driver_provenance();
        let saved_block = self.block;
        self.block = inner;
        let mut hoisted: BTreeMap<TraceNodeId, (TraceNodeId, String)> = BTreeMap::new();
        let mut lift = |executor: &mut Self,
                        stored: &StoredValue|
         -> Result<Option<StoredValue>, TraceJoinError> {
            if let Some(fact) = &stored.fact {
                return Ok(stored.node.map(|_| stored_value(fact.clone())));
            }
            let node = match (stored.node, &stored.value) {
                (Some(node), _) => node,
                (None, SymbolicValue::Computed { node }) => *node,
                _ => return Ok(None),
            };
            if (node as usize) < first_node {
                return Ok(None);
            }
            let variable = match hoisted.get(&node) {
                Some((variable, _)) => *variable,
                None => {
                    let name = format!("trace_value_{}", executor.binding_serial);
                    executor.binding_serial += 1;
                    executor.statement(
                        TraceStatementKind::Assign {
                            name: name.clone(),
                            value: node,
                        },
                        provenance.clone(),
                    );
                    if let TraceStatementKind::Declare { names } =
                        &mut executor.arena.statements[declare as usize].kind
                    {
                        names.push(name.clone());
                    }
                    let variable = executor
                        .identifier(&name, &provenance)
                        .map_err(|_| TraceJoinError("a carried value cannot be built".into()))?;
                    // Assigned once, where the path leaves the block.
                    executor.joins.insert(variable, vec![node]);
                    if let Some(inner) = inner {
                        executor.carried_from.insert(variable, inner);
                    }
                    hoisted.insert(node, (variable, name));
                    variable
                }
            };
            Ok(Some(StoredValue {
                value: SymbolicValue::Computed { node: variable },
                fact: None,
                node: Some(variable),
            }))
        };
        let mut lifted = Vec::new();
        let mut result = Ok(());
        for (cell, stored) in cells.drain(..) {
            match lift(self, &stored) {
                Ok(Some(value)) => lifted.push((Some(cell), 0, value)),
                Ok(None) => {}
                Err(error) => result = Err(error),
            }
        }
        for (index, stored) in thrown.iter().enumerate() {
            match lift(self, stored) {
                Ok(Some(value)) => lifted.push((None, index, value)),
                Ok(None) => {}
                Err(error) => result = Err(error),
            }
        }
        self.block = saved_block;
        result?;
        for (cell, index, value) in lifted {
            match cell {
                Some(cell) => context.set_cell(cell, value),
                None => context.thrown[index] = value,
            }
        }
        // What was known about a carried variable holds for its new name.
        for (node, (_, name)) in hoisted {
            let Some(TraceNodeKind::Identifier { name: old }) =
                self.arena.nodes.get(node as usize).map(|node| &node.kind)
            else {
                continue;
            };
            if let Some(set) = context.value_sets.get(old).cloned() {
                Arc::make_mut(&mut context.value_sets).insert(name.clone(), set);
            }
            if let Some(set) = context.jump_sets.get(old).cloned() {
                Arc::make_mut(&mut context.jump_sets).insert(name.clone(), set);
            }
            if let Some(read) = self.cell_reads.get(old).copied() {
                self.cell_reads.insert(name.clone(), read);
            }
            if let Some(slot) = self.read_names.get(old).copied() {
                self.read_names.insert(name.clone(), slot);
            }
            if self.open_sets.contains(old) {
                self.open_sets.insert(name.clone());
            }
            if let Some(merged) = context.merged_concrete.get(old).copied() {
                Arc::make_mut(&mut context.merged_concrete).insert(name.clone(), merged);
            }
            if let Some(call) = self.outlined.get(old).copied() {
                self.outlined.insert(name, call);
            }
        }
        Ok(())
    }

    /// Where each statement sits (`None`: the function body) and the
    /// statement declaring each name, for [`Self::in_scope_at`].
    pub(super) fn scopes(&self) -> Scopes {
        let mut parent = vec![None; self.arena.statements.len()];
        let mut declared = Declared::default();
        for statement in &self.arena.statements {
            let arms: Vec<(BlockArm, &Vec<TraceStatementId>)> = match &statement.kind {
                TraceStatementKind::If {
                    taken,
                    fall_through,
                    ..
                } => vec![
                    (BlockArm::Taken, taken),
                    (BlockArm::FallThrough, fall_through),
                ],
                TraceStatementKind::Loop { body, .. } => vec![(BlockArm::LoopBody, body)],
                TraceStatementKind::Try {
                    body,
                    handler,
                    finalizer,
                    ..
                } => [(BlockArm::TryBody, body), (BlockArm::Catch, handler)]
                    .into_iter()
                    .chain(
                        finalizer
                            .iter()
                            .map(|finalizer| (BlockArm::Finally, finalizer)),
                    )
                    .collect(),
                TraceStatementKind::Function { body, .. } => vec![(BlockArm::FunctionBody, body)],
                _ => Vec::new(),
            };
            for (arm, children) in arms {
                for child in children {
                    parent[*child as usize] = Some(BlockTarget {
                        statement: statement.id,
                        arm,
                    });
                }
            }
            match &statement.kind {
                TraceStatementKind::Let { name, .. } => {
                    declared.insert(name, statement.id);
                }
                TraceStatementKind::Declare { names } => {
                    for name in names {
                        declared.insert(name, statement.id);
                    }
                }
                TraceStatementKind::Try { param, .. } => {
                    declared.insert(param, statement.id);
                }
                _ => {}
            }
        }
        Scopes { parent, declared }
    }

    /// An opaque predicate's shape (see `predicate::opaque_shape`): decided
    /// on what the path concretely put in the cells `test` reads. In a
    /// `finally` traced once, `thrown` picks what the throw (`true`) or the
    /// end of the body (`false`) put in the cells they put different values
    /// in.
    fn opaque_decision(&mut self, test: TraceNodeId, thrown: Option<bool>) -> Option<bool> {
        let picked;
        let concrete = match thrown {
            None => &self.concrete_reads,
            Some(thrown) => {
                let mut concrete = self.concrete_reads.clone();
                for &(read, body, throw) in &self.finally_reads {
                    concrete.insert(read, if thrown { throw } else { body });
                }
                picked = concrete;
                &picked
            }
        };
        let shaped = predicate::opaque_shape(&self.arena, &self.aliases, &self.call_returns, test)
            || predicate::made_object_null_test(
                &self.arena,
                &self.aliases,
                &self.call_returns,
                concrete,
                test,
            );
        let closed = match shaped {
            true => None,
            false => Some(self.closed_cell_null_test(test, concrete)?),
        };
        let (decided, used) = predicate::decide_opaque(
            &self.arena,
            &self.aliases,
            &self.joins,
            &self.call_returns,
            &self.context.type_facts,
            concrete,
            &self.context.value_sets,
            test,
        );
        // A set that grows later was incomplete: the loop is traced again
        // (see `take_used_sets`).
        self.used_sets.extend(used);
        if let Some((slot, holders)) = closed {
            // A closure made later in a loop's body may run before this
            // test in the next iteration: the loop is traced again if one
            // holding the cell turns up (see `Self::cell_holders_grew`).
            self.cell_holders.entry(slot).or_default().extend(holders);
            self.used_sets.push(format!("holders {slot}"));
        }
        decided
    }

    /// Whether `test` is a `null` or truthiness test (see
    /// [`predicate::null_test`]) of a read of a shared cell this function
    /// owns that holds none of the tested values: what the path concretely
    /// put there is of none of their types, and neither is anything a
    /// closure holding the cell puts there (see [`CellTypesHook`]). The
    /// cell's value is one of those: no other function holds a cell the
    /// function made itself, and a closure made after the read cannot have
    /// written it before the read, except one made later in a loop's body:
    /// the closures counted are those made so far in this attempt and those
    /// seen before ([`Self::cell_holders`]), and the walk traces the loop
    /// again when one turns up after the test ([`Self::cell_holders_grew`]).
    /// The test is then decided on what the path put there, like an opaque
    /// predicate's: the closures that hold the cell rule out the lazy
    /// initialisation a `null` test of a cell usually is (see
    /// [`predicate::opaque_shape`]). Gives the slot and the closures this
    /// attempt made that hold it.
    fn closed_cell_null_test(
        &self,
        test: TraceNodeId,
        concrete: &BTreeMap<TraceNodeId, TraceNodeId>,
    ) -> Option<(u32, BTreeSet<(usize, usize, usize)>)> {
        let hook = self.cell_types_hook.as_ref()?;
        let (value, tested) =
            predicate::null_test(&self.arena, &self.aliases, &self.call_returns, test)?;
        let slot = self.cell_read_slot(value)?;
        if self.captured_cells.contains(&slot) || !self.cells.contains(&slot) {
            return None;
        }
        let held = predicate::concrete_types(
            &self.arena,
            &self.aliases,
            &self.joins,
            &self.call_returns,
            &self.context.type_facts,
            concrete,
            value,
        );
        if held & tested != 0 {
            return None;
        }
        // Every closure made so far that holds the cell, in this attempt
        // (more than the path made, never fewer) and in any attempt before
        // it: one made after this test in a loop's body is made before it
        // in the next iteration.
        let made = self.arena_cell_holders(slot);
        made.iter()
            .chain(self.cell_holders.get(&slot).into_iter().flatten())
            .all(|&(descriptor, pc, state)| {
                hook(descriptor, pc, state, slot).is_some_and(|types| types & tested == 0)
            })
            .then_some((slot, made))
    }

    /// The closures (descriptor, byte offset, dispatch state) this attempt
    /// made that hold the shared cell `slot`.
    fn arena_cell_holders(&self, slot: u32) -> BTreeSet<(usize, usize, usize)> {
        let key = slot.to_string();
        let mut holders = BTreeSet::new();
        for node in &self.arena.nodes {
            let TraceNodeKind::VmFunction {
                descriptor,
                byte_offset,
                dispatch_state,
                memory: Some(memory),
            } = &node.kind
            else {
                continue;
            };
            if let Some(TraceNodeKind::ObjectLiteral { keys, .. }) =
                self.arena.nodes.get(*memory as usize).map(|node| &node.kind)
            {
                if keys.contains(&key) {
                    holders.insert((*descriptor, *byte_offset, *dispatch_state));
                }
            }
        }
        holders
    }

    /// Note the closures this attempt made that hold the cells `slots`
    /// (see [`Self::cell_holders`]): whether one is new, so a closed-cell
    /// test decided before it was made was decided on too few.
    pub(super) fn cell_holders_grew(&mut self, slots: &BTreeSet<u32>) -> bool {
        let mut grew = false;
        for &slot in slots {
            let holders = self.arena_cell_holders(slot);
            let known = self.cell_holders.entry(slot).or_default();
            for holder in holders {
                grew |= known.insert(holder);
            }
        }
        grew
    }

    /// The shared cell whose read `node` is, following `let` bindings.
    fn cell_read_slot(&self, mut node: TraceNodeId) -> Option<u32> {
        for _ in 0..64 {
            if let Some(&slot) = self.read_slots.get(&node) {
                return Some(slot);
            }
            if self.is_cell_read(node) {
                let TraceNodeKind::PropertyRead { object, .. } =
                    &self.arena.nodes.get(node as usize)?.kind
                else {
                    return None;
                };
                let TraceNodeKind::Identifier { name } =
                    &self.arena.nodes.get(*object as usize)?.kind
                else {
                    return None;
                };
                return name.strip_prefix("$CELL_")?.parse().ok();
            }
            node = *self.aliases.get(&node)?;
        }
        None
    }

    /// The `finally` of the `try` `statement` ends: the path goes on after
    /// the `try`. Ended inside a fork the `finally` opened whose other side
    /// ended, it leaves that fork's arm too: what follows is no part of the
    /// `finally`.
    fn close_finally(
        &mut self,
        statement: TraceStatementId,
        p: &TraceProvenance,
    ) -> Result<(), ExecutionStop> {
        let Some((fork, _)) = self.finally_forks.get(&statement).copied() else {
            return Ok(());
        };
        let finally = Some(BlockTarget {
            statement,
            arm: BlockArm::Finally,
        });
        let mut left = Vec::new();
        if self.block != finally {
            let scopes = self.scopes();
            let mut block = self.block;
            while let Some(inner) = block.filter(|_| block != finally) {
                left.push(inner.statement);
                block = scopes.parent[inner.statement as usize];
            }
            if block != finally {
                return Err(self.unsupported_stop(
                    TraceStopCode::UnhandledTry,
                    "a finally ends outside its block".into(),
                    p.clone(),
                ));
            }
        }
        let mut context = std::mem::take(&mut self.context);
        let hoisted = self.hoist(&mut context, self.block, fork.declare, fork.first_node);
        self.context = context;
        if let Err(error) = hoisted {
            return Err(self.unsupported_stop(TraceStopCode::UnhandledTry, error.0, p.clone()));
        }
        if !left.is_empty() {
            Arc::make_mut(&mut self.context.finally_left).extend(left);
        }
        self.block = fork.parent;
        self.context.finally_concrete = Arc::default();
        let length = self.block_statements().map_or(0, Vec::len);
        self.finally_exits
            .push((self.finally_joins, fork.parent, length));
        self.finally_joins += 1;
        Ok(())
    }

    /// `node`, or what it copies through `let` bindings, as long as every
    /// variable it reads is declared in `block` or a block around it: a
    /// value an assignment at the end of `block` may use.
    pub(super) fn in_scope_at(
        &self,
        mut node: TraceNodeId,
        block: BlockTarget,
        scopes: &Scopes,
    ) -> Option<TraceNodeId> {
        // The blocks around `block`, innermost first.
        let mut around = vec![Some(block)];
        let mut next = scopes.parent[block.statement as usize];
        while let Some(outer) = next {
            around.push(Some(outer));
            next = scopes.parent[outer.statement as usize];
        }
        around.push(None);
        let in_scope = |name: &str| match scopes.declared.get(name) {
            // A catch parameter is only in scope in its catch.
            Some(statement) => match &self.arena.statements[statement as usize].kind {
                TraceStatementKind::Try { .. } => around.contains(&Some(BlockTarget {
                    statement,
                    arm: BlockArm::Catch,
                })),
                // Declared in `block`, or before the statement that holds
                // `block` in a block around it.
                _ => {
                    let container = scopes.parent[statement as usize];
                    container == Some(block)
                        || (statement < block.statement && around.contains(&container))
                }
            },
            // Parameters and names declared nowhere in the trace.
            None => true,
        };
        let late_name =
            |node: TraceNodeId| match self.arena.nodes.get(node as usize).map(|n| &n.kind) {
                Some(TraceNodeKind::Identifier { name }) => !in_scope(name),
                _ => false,
            };
        for _ in 0..64 {
            if !late_name(node) {
                break;
            }
            node = *self.aliases.get(&node)?;
        }
        let mut pending = vec![node];
        let mut seen = BTreeSet::new();
        while let Some(next) = pending.pop() {
            if !seen.insert(next) {
                continue;
            }
            if late_name(next) {
                return None;
            }
            if let Some(kind) = self.arena.nodes.get(next as usize).map(|node| &node.kind) {
                pending.extend(node_children(kind));
            }
        }
        Some(node)
    }

    /// Whether `callee` is a VM function made here, or a variable every
    /// value given it is one, whose every return is a VM function made (see
    /// [`ReturnsHook`]). A value given the variable that is never a function
    /// (a literal, a test's or arithmetic's result) is not what a call that
    /// returns called: calling it throws.
    pub(super) fn makes_functions(&self, callee: TraceNodeId) -> bool {
        let Some(hook) = &self.returns_hook else {
            return false;
        };
        let mut pending = vec![callee];
        let mut seen = BTreeSet::new();
        let mut made = false;
        while let Some(node) = pending.pop() {
            let node = self.resolve_copies(node);
            if !seen.insert(node) || seen.len() > 64 {
                if seen.len() > 64 {
                    return false;
                }
                continue;
            }
            match self.arena.nodes.get(node as usize).map(|node| &node.kind) {
                Some(&TraceNodeKind::VmFunction {
                    descriptor,
                    byte_offset,
                    dispatch_state,
                    ..
                }) => {
                    if !hook(descriptor, byte_offset, dispatch_state) {
                        return false;
                    }
                    made = true;
                }
                Some(TraceNodeKind::Identifier { name }) if self.definitions.contains_key(name) => {
                    pending.extend(self.definitions[name].iter().map(|&(_, value)| value));
                }
                Some(kind) if never_a_function(kind) => {}
                _ => return false,
            }
        }
        made
    }

    /// The concrete values `node` may be, following `let` copies: a
    /// literal's value, or a variable's value set.
    pub(super) fn node_values(&self, mut node: TraceNodeId) -> Option<Vec<SymbolicValue>> {
        for _ in 0..64 {
            match &self.arena.nodes.get(node as usize)?.kind {
                TraceNodeKind::Number(text) => {
                    return Some(vec![SymbolicValue::Number(parse_number_text(text)?)])
                }
                TraceNodeKind::String(text) => {
                    return Some(vec![SymbolicValue::String(text.clone())])
                }
                TraceNodeKind::Boolean(value) => return Some(vec![SymbolicValue::Boolean(*value)]),
                TraceNodeKind::Undefined => return Some(vec![SymbolicValue::Undefined]),
                TraceNodeKind::Null => return Some(vec![SymbolicValue::Null]),
                TraceNodeKind::Identifier { name } => {
                    if let Some(set) = self.value_set(name) {
                        return Some(set.to_vec());
                    }
                    node = *self.aliases.get(&node)?;
                }
                _ => return None,
            }
        }
        None
    }

    /// The `TypeError` the VM throws where a handler reads (`write` false)
    /// or writes a memory slot the thread has no cell for: `vmMemory` reads
    /// and writes `this.slots[slot].value`.
    pub(super) fn missing_slot(&mut self, write: bool, p: &TraceProvenance) -> ExecutionStop {
        let message = if write {
            "Cannot set properties of undefined (setting 'value')"
        } else {
            "Cannot read properties of undefined (reading 'value')"
        };
        self.type_error(message, p)
    }

    /// A stop that throws `new TypeError(message)`.
    pub(super) fn type_error(&mut self, message: &str, p: &TraceProvenance) -> ExecutionStop {
        let built = (|| {
            let callee = self.add_node(
                TraceNodeKind::Global {
                    name: "TypeError".into(),
                },
                p.clone(),
            )?;
            let message = self
                .value_node(SymbolicValue::String(message.into()), p)?
                .node;
            let node = self.add_node(
                TraceNodeKind::New {
                    callee,
                    arguments: vec![message],
                },
                p.clone(),
            )?;
            Ok(computed(node))
        })();
        match built {
            Ok(error) => ExecutionStop::Throws(error),
            Err(stop) => stop,
        }
    }

    /// The block the value `node` copies was first carried out of, if it
    /// was: following copies back to where the value was made.
    pub(super) fn carried_origin(&self, mut node: TraceNodeId) -> Option<BlockTarget> {
        let mut origin = None;
        for _ in 0..64 {
            if let Some(&block) = self.carried_from.get(&node) {
                origin = Some(block);
            }
            match (
                self.aliases.get(&node),
                self.joins.get(&node).map(Vec::as_slice),
            ) {
                (Some(&value), _) | (None, Some(&[value])) => node = value,
                _ => break,
            }
        }
        origin
    }

    /// Whether `node` is `$CELL_n["value"]`, a read of a shared cell.
    /// Whether `node` is a shared cell's read of program data. The walk
    /// notes the cell it asked about: its contents decided the answer.
    pub(super) fn is_data_read(&self, node: TraceNodeId) -> bool {
        if let Some(slot) = self.read_slots.get(&node) {
            self.data_consulted.borrow_mut().insert(*slot);
        }
        self.data_reads.contains(&node)
    }

    /// The generated variable `node` reads, following `let` bindings.
    pub(super) fn identifier_of(&self, mut node: TraceNodeId) -> Option<String> {
        for _ in 0..64 {
            if let Some(TraceNodeKind::Identifier { name }) =
                self.arena.nodes.get(node as usize).map(|node| &node.kind)
            {
                if self.definitions.contains_key(name) {
                    return Some(name.clone());
                }
            }
            node = *self.aliases.get(&node)?;
        }
        None
    }

    /// The bytecode constants (non-negative integers) the variable `name`
    /// was given, following the variables it was given in turn, when every
    /// other value it was given is a program value: a property read of no
    /// shared cell, a call, or a literal no position is. `None` when one may
    /// be a position the walk does not know (a parameter, a shared cell's
    /// read, arithmetic) or `undefined`, which a dispatch jumps to to throw.
    pub(super) fn position_definitions(&self, name: &str) -> Option<Vec<SymbolicValue>> {
        let mut constants: Vec<SymbolicValue> = Vec::new();
        let mut pending: Vec<TraceNodeId> = self
            .definitions
            .get(name)?
            .iter()
            .map(|&(_, value)| value)
            .collect();
        let mut seen: BTreeSet<String> = BTreeSet::from([name.to_string()]);
        let mut visited = 0;
        while let Some(node) = pending.pop() {
            visited += 1;
            if visited > 256 {
                return None;
            }
            let node = self.resolve_copies(node);
            if let Some(arms) = self.joins.get(&node) {
                pending.extend(arms.iter().copied());
                continue;
            }
            match self.arena.nodes.get(node as usize).map(|node| &node.kind)? {
                TraceNodeKind::Number(text) => {
                    let number = parse_number_text(text)?;
                    if number >= 0.0 && number.fract() == 0.0 {
                        let value = SymbolicValue::Number(number);
                        if !constants.iter().any(|known| same_fact(known, &value)) {
                            constants.push(value);
                        }
                    }
                }
                TraceNodeKind::Identifier { name } => {
                    if !seen.insert(name.clone()) {
                        continue;
                    }
                    pending.extend(self.definitions.get(name)?.iter().map(|&(_, value)| value));
                }
                TraceNodeKind::External { name } if name.starts_with("arg") => {}
                TraceNodeKind::PropertyRead { .. }
                    if self.is_cell_read(node) && !self.is_data_read(node) =>
                {
                    return None
                }
                TraceNodeKind::PropertyRead { .. }
                | TraceNodeKind::Call { .. }
                | TraceNodeKind::New { .. }
                | TraceNodeKind::String(_)
                | TraceNodeKind::Boolean(_)
                | TraceNodeKind::Null
                | TraceNodeKind::ObjectLiteral { .. }
                | TraceNodeKind::ArrayLiteral { .. }
                | TraceNodeKind::VmFunction { .. }
                | TraceNodeKind::Function { .. } => {}
                _ if self.program_data(node, 0) => {}
                _ => return None,
            }
        }
        Some(constants)
    }

    /// Whether `node` is program data no VM position is: an argument its
    /// caller passed, a property read of no shared cell, a call, or
    /// arithmetic on one of those (`Number(x) + 1`). Arithmetic on constants
    /// alone may compute a position.
    pub(super) fn program_data(&self, node: TraceNodeId, depth: usize) -> bool {
        if depth > 16 {
            return false;
        }
        let node = self.resolve_copies(node);
        match self.arena.nodes.get(node as usize).map(|node| &node.kind) {
            Some(TraceNodeKind::External { name }) => name.starts_with("arg"),
            Some(TraceNodeKind::PropertyRead { .. }) => {
                !self.is_cell_read(node) || self.is_data_read(node)
            }
            Some(TraceNodeKind::Call { .. } | TraceNodeKind::New { .. }) => true,
            Some(TraceNodeKind::Binary { left, right, .. }) => {
                self.program_data(*left, depth + 1) || self.program_data(*right, depth + 1)
            }
            Some(TraceNodeKind::Unary { value, .. }) => self.program_data(*value, depth + 1),
            _ => false,
        }
    }

    pub(super) fn is_cell_read(&self, node: TraceNodeId) -> bool {
        let kind = |node: TraceNodeId| self.arena.nodes.get(node as usize).map(|node| &node.kind);
        let Some(TraceNodeKind::PropertyRead { object, key }) = kind(node) else {
            return false;
        };
        matches!(kind(*object), Some(TraceNodeKind::Identifier { name }) if name.starts_with("$CELL_"))
            && matches!(kind(*key), Some(TraceNodeKind::String(key)) if key == "value")
    }

    /// Whether `node` is arithmetic on numbers: on number literals, such
    /// arithmetic, or bindings of either (the VM's index bookkeeping, `i + 1`
    /// with `i` bound to `1`). It runs no program code.
    pub(super) fn number_arithmetic(&self, node: TraceNodeId) -> bool {
        let mut pending = vec![node];
        let mut seen = 0;
        while let Some(node) = pending.pop() {
            seen += 1;
            if seen > 64 {
                return false;
            }
            match self.arena.nodes.get(node as usize).map(|node| &node.kind) {
                Some(TraceNodeKind::Binary { left, right, .. }) => {
                    for operand in [*left, *right] {
                        let operand = self.resolve_copies(operand);
                        match self
                            .arena
                            .nodes
                            .get(operand as usize)
                            .map(|node| &node.kind)
                        {
                            Some(TraceNodeKind::Number(_)) => {}
                            Some(TraceNodeKind::Binary { .. }) => pending.push(operand),
                            _ => return false,
                        }
                    }
                }
                _ => return false,
            }
        }
        true
    }

    /// The node `node` is a copy of, following `let` bindings and variables
    /// carried out of a block (each assigned once, where the path left it).
    /// The node that holds the object `node` is: through `let` copies,
    /// through `Object.defineProperty`, which returns the object it defines
    /// on, and through a call of an outlined function that returns the
    /// object passed to it. A made object's properties are kept by this
    /// node.
    pub(super) fn object_handle(&self, mut node: TraceNodeId) -> TraceNodeId {
        for _ in 0..64 {
            if let Some(next) = self.aliases.get(&node) {
                node = *next;
            } else if let Some(object) = self.defined_object(node) {
                node = object;
            } else if let Some(&argument) = self.returned_arguments.get(&node) {
                node = argument;
            } else {
                break;
            }
        }
        node
    }

    /// The object of `Object.defineProperty(object, key, descriptor)`, with
    /// the call's arguments.
    fn defined_object(&self, call: TraceNodeId) -> Option<TraceNodeId> {
        self.define_property(call).map(|(object, _, _)| object)
    }

    /// `call`'s object, key and descriptor if it is `Object.defineProperty`.
    fn define_property(
        &self,
        call: TraceNodeId,
    ) -> Option<(TraceNodeId, TraceNodeId, TraceNodeId)> {
        let kind = |node: TraceNodeId| self.arena.nodes.get(node as usize).map(|node| &node.kind);
        let Some(TraceNodeKind::Call {
            callee, arguments, ..
        }) = kind(call)
        else {
            return None;
        };
        let mut callee = *callee;
        while let Some(next) = self.aliases.get(&callee) {
            callee = *next;
        }
        let Some(TraceNodeKind::PropertyRead { object, key }) = kind(callee) else {
            return None;
        };
        let defines = matches!(kind(*object), Some(TraceNodeKind::Global { name }) if name == "Object")
            && matches!(kind(*key), Some(TraceNodeKind::String(key)) if key == "defineProperty");
        match arguments.as_slice() {
            [object, key, descriptor] if defines => Some((*object, *key, *descriptor)),
            _ => None,
        }
    }

    /// `key` of the made object `handle` is now `value` (unknown: `None`);
    /// an unknown key may be any of them.
    pub(super) fn note_property(
        &mut self,
        handle: TraceNodeId,
        key: Option<&SymbolicValue>,
        value: Option<SymbolicValue>,
    ) {
        if !self.context.made_objects.contains_key(&handle) {
            return;
        }
        let objects = Arc::make_mut(&mut self.context.made_objects);
        let properties = objects.get_mut(&handle).expect("checked");
        match (key, value.filter(is_primitive_fact)) {
            (Some(SymbolicValue::String(key)), Some(value)) => {
                properties.insert(key.clone(), value);
            }
            (Some(SymbolicValue::String(key)), None) => {
                properties.remove(key);
            }
            _ => properties.clear(),
        }
    }

    /// `Object.defineProperty(object, key, descriptor)` with a writable,
    /// configurable data descriptor: its object, key and descriptor nodes
    /// and the value when known.
    pub(super) fn plain_definition(
        &self,
        call: TraceNodeId,
    ) -> Option<(TraceNodeId, TraceNodeId, TraceNodeId, Option<SymbolicValue>)> {
        let (object, key, descriptor) = self.define_property(call)?;
        let value = self.definition_data(descriptor)?;
        Some((object, key, descriptor, value))
    }

    /// A literal `node` is, following `let` copies and, through `bindings`,
    /// a subroutine's parameters to a caller's arguments: a string, number
    /// or boolean, or what the proven string decoder makes of literals.
    pub(super) fn folded_literal(
        &self,
        node: TraceNodeId,
        bindings: &BTreeMap<&str, TraceNodeId>,
        depth: usize,
    ) -> Option<SymbolicValue> {
        if depth > 16 {
            return None;
        }
        match &self.arena.nodes.get(node as usize)?.kind {
            TraceNodeKind::String(text) => Some(SymbolicValue::String(text.clone())),
            TraceNodeKind::Number(text) => parse_number_text(text).map(SymbolicValue::Number),
            TraceNodeKind::Boolean(value) => Some(SymbolicValue::Boolean(*value)),
            TraceNodeKind::Identifier { name } => match bindings.get(name.as_str()) {
                Some(&argument) => self.folded_literal(argument, &BTreeMap::new(), depth + 1),
                None => self.folded_literal(*self.aliases.get(&node)?, bindings, depth + 1),
            },
            TraceNodeKind::Call {
                callee, arguments, ..
            } => {
                let decoder = self.string_decoder.as_ref()?;
                let callee = self.resolve_copies(*callee);
                if !matches!(&self.arena.nodes.get(callee as usize)?.kind,
                    TraceNodeKind::Global { name } if *name == decoder.helper)
                {
                    return None;
                }
                let [encoded, key, ..] = arguments.as_slice() else {
                    return None;
                };
                let (Some(SymbolicValue::String(encoded)), Some(SymbolicValue::Number(key))) = (
                    self.folded_literal(*encoded, bindings, depth + 1),
                    self.folded_literal(*key, bindings, depth + 1),
                ) else {
                    return None;
                };
                (key.fract() == 0.0 && key >= 0.0)
                    .then(|| decoder.decode(&encoded, key as usize))
                    .flatten()
                    .map(SymbolicValue::String)
            }
            _ => None,
        }
    }

    /// A data descriptor `descriptor` that leaves the property writable and
    /// configurable: its value when known (`Some(None)`: unknown).
    fn definition_data(&self, descriptor: TraceNodeId) -> Option<Option<SymbolicValue>> {
        let kind = |node: TraceNodeId| self.arena.nodes.get(node as usize).map(|node| &node.kind);
        let single = |node: TraceNodeId| match self.node_values(node).as_deref() {
            Some([value]) => Some(value.clone()),
            _ => None,
        };
        match kind(self.object_handle(descriptor)) {
            Some(TraceNodeKind::ObjectLiteral { keys, values }) => {
                let field = |name: &str| {
                    keys.iter()
                        .position(|key| key == name)
                        .map(|index| values[index])
                };
                let plain = keys.iter().all(|key| {
                    matches!(
                        key.as_str(),
                        "value" | "writable" | "configurable" | "enumerable"
                    )
                }) && ["writable", "configurable"]
                    .iter()
                    .all(|name| field(name).and_then(single) == Some(SymbolicValue::Boolean(true)));
                plain.then(|| field("value").and_then(single))
            }
            _ => None,
        }
    }

    /// `Object.defineProperty` on a made object: a writable, configurable
    /// data property with a known value is that value. Any other descriptor
    /// (an accessor, a read-only property) stops what the path knows of it.
    fn note_defined_property(&mut self, call: TraceNodeId) {
        if self.context.made_objects.is_empty() {
            return;
        }
        let Some((object, key, descriptor)) = self.define_property(call) else {
            return;
        };
        let handle = self.object_handle(object);
        if !self.context.made_objects.contains_key(&handle) {
            return;
        }
        let single = |node: TraceNodeId| match self.node_values(node).as_deref() {
            Some([value]) => Some(value.clone()),
            _ => None,
        };
        let data = self.definition_data(descriptor);
        let key = single(key);
        match data {
            Some(value) => self.note_property(handle, key.as_ref(), value),
            None => {
                Arc::make_mut(&mut self.context.made_objects).remove(&handle);
            }
        }
    }

    /// `value` is what a call of an outlined function returned: a new object
    /// with the properties `known` says its body left it with, held by
    /// `value`'s node alone (see [`TraceContext::made_objects`]).
    pub(super) fn note_made_return(
        &mut self,
        value: TraceNodeId,
        known: Option<&Option<BTreeMap<String, SymbolicValue>>>,
    ) {
        if let Some(Some(properties)) = known {
            let handle = self.object_handle(value);
            Arc::make_mut(&mut self.context.made_objects).insert(handle, properties.clone());
        }
    }

    /// The made objects `kind` hands to other code: an argument, a stored or
    /// assigned value, a return or throw. Reading a property of one, testing
    /// it, or defining a property on it does not.
    fn escaping_objects(&self, kind: &TraceStatementKind) -> Vec<TraceNodeId> {
        self.handed_on(kind, false, |handle| {
            self.context.made_objects.contains_key(&handle)
        })
    }

    /// The nodes `tracked` holds that `kind` hands to other code (see
    /// [`Self::escaping_objects`]); with `writes`, also those it writes a
    /// property of. A call of an outlined function hands on only the
    /// arguments its body hands on or writes (see `subroutine_escapes`).
    pub(super) fn handed_on(
        &self,
        kind: &TraceStatementKind,
        writes: bool,
        tracked: impl Fn(TraceNodeId) -> bool,
    ) -> Vec<TraceNodeId> {
        let node_kind =
            |node: TraceNodeId| self.arena.nodes.get(node as usize).map(|node| &node.kind);
        let mut pending: Vec<(TraceNodeId, bool)> = match kind {
            TraceStatementKind::Let { value, .. } | TraceStatementKind::Evaluate { value } => {
                vec![(*value, false)]
            }
            TraceStatementKind::Assign { value, .. }
            | TraceStatementKind::Return { value }
            | TraceStatementKind::Throw { value } => vec![(*value, true)],
            TraceStatementKind::PropertyWrite { object, key, value } => {
                vec![(*object, writes), (*key, true), (*value, true)]
            }
            _ => return Vec::new(),
        };
        let mut found = Vec::new();
        let mut seen = BTreeSet::new();
        while let Some((node, escapes)) = pending.pop() {
            if escapes {
                let handle = self.object_handle(node);
                if tracked(handle) {
                    found.push(handle);
                }
            }
            if !seen.insert((node, escapes)) {
                continue;
            }
            if let Some((object, key, descriptor)) = self.define_property(node) {
                // Its result is the object: handed on where the call is.
                pending.extend([(object, escapes || writes), (key, true), (descriptor, true)]);
                continue;
            }
            if let Some(TraceNodeKind::Call {
                callee,
                this_value,
                arguments,
            }) = node_kind(node)
            {
                let flags = match node_kind(*callee) {
                    Some(TraceNodeKind::Identifier { name }) => self.subroutine_escapes.get(name),
                    _ => None,
                };
                if let Some(flags) = flags.filter(|flags| flags.len() == arguments.len()) {
                    pending.extend([(*callee, true), (*this_value, true)]);
                    pending.extend(
                        arguments
                            .iter()
                            .zip(flags.iter())
                            .map(|(&argument, &flag)| (argument, flag)),
                    );
                    continue;
                }
            }
            match node_kind(node) {
                // A copy was looked at where its `let` was made.
                Some(TraceNodeKind::Identifier { .. }) | None => {}
                Some(TraceNodeKind::PropertyRead { object, key }) => {
                    pending.extend([(*object, false), (*key, true)]);
                }
                Some(TraceNodeKind::Unary {
                    operator:
                        PrimitiveOperator::Typeof
                        | PrimitiveOperator::LogicalNot
                        | PrimitiveOperator::Void,
                    value,
                }) => pending.push((*value, false)),
                Some(TraceNodeKind::Binary {
                    operator:
                        PrimitiveOperator::StrictEqual
                        | PrimitiveOperator::Equal
                        | PrimitiveOperator::StrictNotEqual
                        | PrimitiveOperator::NotEqual,
                    left,
                    right,
                }) => pending.extend([(*left, false), (*right, false)]),
                Some(kind) => {
                    pending.extend(node_children(kind).into_iter().map(|child| (child, true)))
                }
            }
        }
        found
    }

    pub(super) fn resolve_copies(&self, mut node: TraceNodeId) -> TraceNodeId {
        for _ in 0..64 {
            match (
                self.aliases.get(&node),
                self.joins.get(&node).map(Vec::as_slice),
            ) {
                (Some(&value), _) | (None, Some(&[value])) => node = value,
                _ => break,
            }
        }
        node
    }

    /// The variable `node` reads, following `let` copies.
    pub(super) fn variable_of(&self, mut node: TraceNodeId) -> Option<String> {
        for _ in 0..64 {
            if let Some(TraceNodeKind::Identifier { name }) =
                self.arena.nodes.get(node as usize).map(|node| &node.kind)
            {
                // A variable with a jump set (see `jump_sets`) too: a VM
                // position it holds is one of those addresses.
                if self.value_set(name).is_some()
                    || self.context.assumed.contains_key(name)
                    || self.context.jump_sets.contains_key(name)
                {
                    return Some(name.clone());
                }
            }
            node = *self.aliases.get(&node)?;
        }
        None
    }

    /// The concrete values `stored` may be: its own, or those of the value
    /// set its variable has. `own` is a variable whose read means "the
    /// values it already holds" (no new ones).
    pub(super) fn candidates(
        &self,
        stored: &StoredValue,
        own: Option<&String>,
    ) -> Option<Vec<SymbolicValue>> {
        if let Some(fact) = &stored.fact {
            return Some(vec![fact.clone()]);
        }
        let name = self.variable_of(stored.node?).or_else(|| {
            match self
                .arena
                .nodes
                .get(stored.node? as usize)
                .map(|node| &node.kind)
            {
                Some(TraceNodeKind::Identifier { name }) if Some(name) == own => Some(name.clone()),
                _ => None,
            }
        })?;
        if Some(&name) == own {
            return Some(Vec::new());
        }
        self.value_set(&name).map(|set| set.to_vec())
    }

    /// The value set the variable `name` has: one a loop, `try` or join
    /// gave it on this path, or that of the cell it read.
    /// Whether `value` of the variable `name` can only be the value it held
    /// at its `try`'s entry: it has a value set, the body writes it only VM
    /// addresses (a jump set), and `value` is none of them.
    fn entry_value_only(&self, name: &str, value: &SymbolicValue) -> bool {
        let Some(written) = self.context.jump_sets.get(name) else {
            return false;
        };
        self.context.value_sets.contains_key(name)
            && !written.iter().any(|known| same_fact(known, value))
    }

    pub(super) fn value_set(&self, name: &str) -> Option<Arc<[SymbolicValue]>> {
        // The cell's contents decide the answer: see `CellReads::same`.
        // A set of its own answers before the contents do.
        if let Some(slot) = self
            .read_names
            .get(name)
            .filter(|_| !self.context.value_sets.contains_key(name))
        {
            self.set_consulted.borrow_mut().insert(*slot);
        }
        self.context.value_sets.get(name).cloned().or_else(|| {
            let (read, _) = self.cell_reads.get(name)?;
            self.cell_sets.get(&read.slot).cloned()
        })
    }

    /// The merge of concrete values `node` is computed from, if any,
    /// following `let` bindings.
    pub(super) fn merged_concrete_in(&self, node: TraceNodeId) -> Option<MergedConcrete> {
        let merged = &self.context.merged_concrete;
        if merged.is_empty() {
            return None;
        }
        self.depends_on(node, |name| merged.get(name).copied())
    }

    /// The tag of the outlined call that `node` is computed from, if any.
    pub(super) fn outlined_in(&self, node: TraceNodeId) -> Option<usize> {
        let outlined = &self.outlined;
        if outlined.is_empty() {
            return None;
        }
        self.depends_on(node, |name| outlined.get(name).copied())
    }

    /// What `node` is computed from directly: the value a `let` binding
    /// holds, the arms of a join, the operands.
    pub(super) fn inputs(&self, node: TraceNodeId) -> Vec<TraceNodeId> {
        let mut inputs: Vec<TraceNodeId> = self.aliases.get(&node).copied().into_iter().collect();
        inputs.extend(self.joins.get(&node).into_iter().flatten().copied());
        if let Some(node) = self.arena.nodes.get(node as usize) {
            inputs.extend(node_children(&node.kind));
        }
        inputs
    }

    /// Whether `node` is computed from a marked name ([`Self::marked`]).
    pub(super) fn reaches_marked(&self, node: TraceNodeId) -> bool {
        let mut memo = self.reaches_marked.borrow_mut();
        if memo.len() < self.arena.nodes.len() {
            memo.resize(self.arena.nodes.len(), None);
        }
        let known = |memo: &Vec<Option<bool>>, node: TraceNodeId| {
            memo.get(node as usize).copied().unwrap_or(Some(false))
        };
        let mut pending = vec![(node, false)];
        let mut open = BTreeSet::new();
        while let Some((node, inputs_done)) = pending.pop() {
            if known(&memo, node).is_some() {
                continue;
            }
            let marked = matches!(
                self.arena.nodes.get(node as usize).map(|node| &node.kind),
                Some(TraceNodeKind::Identifier { name }) if self.marked.contains(name)
            );
            let inputs = self.inputs(node);
            if !marked && !inputs_done {
                open.insert(node);
                pending.push((node, true));
                pending.extend(
                    inputs
                        .into_iter()
                        .filter(|&input| known(&memo, input).is_none() && !open.contains(&input))
                        .map(|input| (input, false)),
                );
                continue;
            }
            open.remove(&node);
            let reaches = marked
                || inputs
                    .iter()
                    .any(|&input| known(&memo, input) == Some(true));
            memo[node as usize] = Some(reaches);
        }
        known(&memo, node) == Some(true)
    }

    /// The first variable `node` is computed from that `found` knows,
    /// following `let` bindings. Only marked names ([`Self::marked`]) are
    /// looked for, so a part of the graph that reaches none is skipped.
    pub(super) fn depends_on<T>(
        &self,
        node: TraceNodeId,
        found: impl Fn(&str) -> Option<T>,
    ) -> Option<T> {
        let mut pending = vec![node];
        let mut seen = BTreeSet::new();
        while let Some(node) = pending.pop() {
            if !seen.insert(node) || !self.reaches_marked(node) {
                continue;
            }
            if let Some(&value) = self.aliases.get(&node) {
                pending.push(value);
            }
            // A join variable is computed from what its arms assigned.
            if let Some(arms) = self.joins.get(&node) {
                pending.extend(arms.iter().copied());
            }
            let Some(kind) = self.arena.nodes.get(node as usize).map(|node| &node.kind) else {
                continue;
            };
            if let TraceNodeKind::Identifier { name } = kind {
                if let Some(found) = found(name) {
                    return Some(found);
                }
            }
            pending.extend(node_children(kind));
        }
        None
    }

    /// `node` with each parameter in `bindings` replaced by its argument,
    /// following `let` bindings: what a call of an outlined function
    /// returns, for tests only. Nodes whose value does not depend on their
    /// operands' values are shared; `None` past the depth bound.
    pub(super) fn substitute(
        &mut self,
        node: TraceNodeId,
        bindings: &BTreeMap<&str, TraceNodeId>,
        objects: &BTreeMap<TraceNodeId, BTreeMap<String, SymbolicValue>>,
        copied: &mut BTreeMap<TraceNodeId, TraceNodeId>,
        depth: usize,
    ) -> Option<TraceNodeId> {
        if depth > 64 {
            return None;
        }
        if let Some(&done) = copied.get(&node) {
            return Some(done);
        }
        let kind = self.arena.nodes.get(node as usize)?.kind.clone();
        let provenance = self.arena.nodes[node as usize].provenance.clone();
        let made = match kind {
            TraceNodeKind::Identifier { name } => match bindings.get(name.as_str()) {
                Some(&argument) => argument,
                // A binding of the function: its value, if that reads a
                // parameter; the binding itself otherwise.
                None => match self.aliases.get(&node).copied() {
                    Some(value) => {
                        let made = self.substitute(value, bindings, objects, copied, depth + 1)?;
                        if made == value {
                            node
                        } else {
                            made
                        }
                    }
                    None => node,
                },
            },
            // Copied only when an operand changed: a part that reads no
            // parameter is the function's own node.
            TraceNodeKind::Binary {
                operator,
                left,
                right,
            } => {
                let (new_left, new_right) = (
                    self.substitute(left, bindings, objects, copied, depth + 1)?,
                    self.substitute(right, bindings, objects, copied, depth + 1)?,
                );
                if (new_left, new_right) == (left, right) {
                    node
                } else {
                    self.add_node(
                        TraceNodeKind::Binary {
                            operator,
                            left: new_left,
                            right: new_right,
                        },
                        provenance,
                    )
                    .ok()?
                }
            }
            TraceNodeKind::Unary { operator, value } => {
                let new_value = self.substitute(value, bindings, objects, copied, depth + 1)?;
                if new_value == value {
                    node
                } else {
                    self.add_node(
                        TraceNodeKind::Unary {
                            operator,
                            value: new_value,
                        },
                        provenance,
                    )
                    .ok()?
                }
            }
            TraceNodeKind::PropertyRead { object, key } => {
                let (new_object, new_key) = (
                    self.substitute(object, bindings, objects, copied, depth + 1)?,
                    self.substitute(key, bindings, objects, copied, depth + 1)?,
                );
                // A property of a made object the caller passed and the
                // body neither writes nor hands on (`objects`): what the
                // caller last wrote there, as the read in the body saw it.
                let known = objects
                    .get(&self.object_handle(new_object))
                    .zip(
                        match self.arena.nodes.get(self.resolve_copies(new_key) as usize) {
                            Some(TraceNode {
                                kind: TraceNodeKind::String(name),
                                ..
                            }) => Some(name.clone()),
                            _ => None,
                        },
                    )
                    .and_then(|(properties, name)| properties.get(&name).cloned());
                if let Some(value) = known {
                    self.value_node(value, &provenance).ok()?.node
                } else if (new_object, new_key) == (object, key) {
                    node
                } else {
                    self.add_node(
                        TraceNodeKind::PropertyRead {
                            object: new_object,
                            key: new_key,
                        },
                        provenance,
                    )
                    .ok()?
                }
            }
            _ => node,
        };
        copied.insert(node, made);
        Some(made)
    }
}

impl Executor {
    pub(super) fn eval_index(
        &mut self,
        value: &SemanticValue,
        instruction: &DecodedInstruction,
        p: &TraceProvenance,
    ) -> Result<EvalIndex, ExecutionStop> {
        let evaluated = self.eval(value, instruction, p)?;
        let value = self.concrete_index(&evaluated, "index", p)?;
        Ok(EvalIndex {
            value,
            node: evaluated.node,
        })
    }

    /// The parameter a subroutine traced as a function reads the caller's
    /// `cell` as: named now, at its first read, as when every caller cell
    /// was named on entry.
    pub(super) fn param_value(&mut self, cell: TraceCell) -> Result<StoredValue, ExecutionStop> {
        let Some(open) = self.context.subroutine.clone() else {
            return Ok(unnamed_param());
        };
        let param = match cell {
            TraceCell::Stack(index) => format!("{}_s{}", open.name, open.entry_stack - index),
            TraceCell::Memory(slot) => format!("{}_m{slot}", open.name),
        };
        let node = self.identifier(&param, &open.provenance)?;
        let stored = StoredValue {
            value: SymbolicValue::Computed { node },
            fact: None,
            node: Some(node),
        };
        self.context.set_cell(cell, stored.clone());
        Arc::make_mut(&mut self.context.params).insert(param.clone());
        Arc::make_mut(&mut self.context.placeholders).insert(cell, param);
        Ok(stored)
    }

    /// The value of `cell`, a parameter named if it is first read now.
    pub(super) fn cell_value(
        &mut self,
        cell: TraceCell,
    ) -> Result<Option<StoredValue>, ExecutionStop> {
        let param = match cell {
            TraceCell::Stack(index) => {
                matches!(self.context.stack.get(index), Some(StackCell::Param))
            }
            TraceCell::Memory(slot) => {
                matches!(self.context.memory.get(&slot), Some(MemoryCell::Param))
            }
        };
        if param {
            return self.param_value(cell).map(Some);
        }
        Ok(self.context.cell(cell))
    }

    /// Stop: a step reached the holes join `join` put into the stack, or
    /// counted stack positions from the bottom (see `TraceContext::stack_pad`).
    /// `reached`: for a step counting from the top, the stack position it
    /// reads or writes, or the length it cuts the stack to. Without the
    /// holes the shorter side's stack is that many shorter, so a position
    /// below their count is below its bottom: the shorter side would read
    /// under an empty stack, which code that runs never does (see
    /// `TraceSession::take_pad_conflict`). Above it, the shorter side reads
    /// a value of its own (`v || x` with `x` pushed first), and the step
    /// only undoes the join.
    pub(super) fn padded_stop(
        &mut self,
        join: usize,
        reached: Option<usize>,
        p: &TraceProvenance,
    ) -> ExecutionStop {
        let one_join = self.context.stack_pad_joins == 1;
        let underflow = reached.is_some_and(|index| index < self.context.stack_pad);
        let evidence = underflow && one_join && self.context.stack_pad_deficit;
        self.pad_conflict = Some((join, evidence));
        self.unsupported_stop(
            TraceStopCode::PaddedStack,
            "a step reaches the stack values a join lined up from the top".into(),
            p.clone(),
        )
    }

    pub(super) fn resolve_stack(
        &mut self,
        index: usize,
        p: &TraceProvenance,
    ) -> Result<EvalValue, ExecutionStop> {
        if let Some(join) = self.context.padded_below(index) {
            return Err(self.padded_stop(join, Some(index), p));
        }
        let mut stored = match self.context.stack.get(index) {
            Some(StackCell::Value(value)) => value.clone(),
            Some(StackCell::Param) => self.param_value(TraceCell::Stack(index))?,
            Some(StackCell::Hole) | None => stored_value(SymbolicValue::Undefined),
        };
        let node = match stored.node {
            Some(node) => node,
            None => {
                let value = self.value_node(stored.value.clone(), p)?;
                stored.fact = value.fact.clone();
                stored.node = Some(value.node);
                value.node
            }
        };
        if let Some(StackCell::Value(value)) = self.context.stack.get_mut(index) {
            *value = stored.clone();
        }
        Ok(EvalValue {
            value: stored.value,
            fact: stored.fact,
            node,
        })
    }

    pub(super) fn resolve_memory(
        &mut self,
        slot: usize,
        mut stored: StoredValue,
        p: &TraceProvenance,
    ) -> Result<EvalValue, ExecutionStop> {
        let node = match stored.node {
            Some(node) => node,
            None => {
                let node = self.value_node(stored.value.clone(), p)?.node;
                stored.node = Some(node);
                node
            }
        };
        if let Some(MemoryCell::Value(value)) = self.context.memory_mut().get_mut(&(slot as u32)) {
            *value = stored.clone();
        } else if matches!(
            self.context.memory.get(&(slot as u32)),
            Some(MemoryCell::Initialized)
        ) {
            self.context
                .memory_mut()
                .insert(slot as u32, MemoryCell::Value(stored.clone()));
        }
        Ok(EvalValue {
            value: stored.value,
            fact: stored.fact,
            node,
        })
    }

    /// The VM position `node` reads from a shared cell this path wrote it
    /// to (see [`TraceContext::cell_positions`]).
    pub(super) fn vm_position(&self, mut node: TraceNodeId) -> Option<usize> {
        for _ in 0..MAX_VALUE_SET {
            if let Some(&position) = self.position_reads.get(&node) {
                return Some(position);
            }
            node = *self.aliases.get(&node)?;
        }
        None
    }

    pub(super) fn concrete_index(
        &mut self,
        value: &EvalValue,
        what: &str,
        p: &TraceProvenance,
    ) -> Result<usize, ExecutionStop> {
        let body_state =
            what == "dispatch state" && !self.from_value_set && std::mem::take(&mut self.body_address);
        let entry_state = what == "dispatch state"
            && !self.from_value_set
            && std::mem::take(&mut self.entry_address);
        if what == "jump target" && !self.from_value_set {
            self.body_address = false;
            self.entry_address = false;
        }
        if value.fact.is_none() && is_vm_position(what) {
            if let Some(position) = self.vm_position(value.node) {
                return Ok(position);
            }
        }
        let Some(SymbolicValue::Number(number)) = value.fact.as_ref() else {
            // A jump to `undefined`: the VM's next `context.dispatch()` reads
            // `vmDispatchTable[state][vmBytecode[NaN]]` (or the row of an
            // `undefined` state) as `undefined` and throws reading it.
            // Dispatch runs outside `vmInvokeOperation`, so no VM `try`
            // catches it: with none open that is a plain `throw`. Past a
            // catch only for a value the path took from a complete value
            // set (a `try` variable's), not from the catch's entry state.
            if matches!(value.fact, Some(SymbolicValue::Undefined))
                && matches!(what, "jump target" | "dispatch state")
                && self.context.try_frames.is_empty()
                && (!self.context.from_catch || self.from_value_set)
            {
                let key = if what == "jump target" {
                    "0"
                } else {
                    "undefined"
                };
                return Err(self.type_error(
                    &format!("Cannot read properties of undefined (reading '{key}')"),
                    p,
                ));
            }
            if let Some(merged) = self.merged_concrete_in(value.node) {
                self.split = Some(merged);
            }
            if let Some(call) = self.outlined_in(value.node) {
                self.inline = Some(call);
            }
            // A variable known to hold one of a few concrete values (the
            // targets a state machine's states set up): the value this path
            // assumes, or one dispatch over the values it may still hold.
            // Described only for a stop that stands: describing walks every
            // statement, and a dispatch over the values replaces the stop.
            let mut message = None;
            if let Some(name) = self.variable_of(value.node) {
                if what == "jump target" {
                    self.body_address = self.value_set(&name).is_none()
                        && self.context.jump_sets.contains_key(&name);
                }
                let assumed = self.context.assumed.get(&name).cloned();
                if let Some(Assumption::Is(value)) = &assumed {
                    if what == "jump target" {
                        self.entry_address = self.entry_value_only(&name, value);
                    }
                    let value = EvalValue {
                        value: value.clone(),
                        fact: Some(value.clone()),
                        node: 0,
                    };
                    self.from_value_set = true;
                    let index = self.concrete_index(&value, what, p);
                    self.from_value_set = false;
                    return index;
                }
                // A cell that may hold a JavaScript value says nothing of
                // one used as data: its addresses only matter for a VM
                // address.
                let mut open = self.open_sets.contains(&name);
                let mut set = self
                    .value_set(&name)
                    .filter(|_| !open || !matches!(what, "index" | "stack length"));
                if set.is_none() && is_vm_position(what) {
                    // Its other value is a program value: see `jump_sets`.
                    set = self.context.jump_sets.get(&name).cloned();
                    open = set.is_some();
                } else if body_state {
                    set = self.context.jump_sets.get(&name).cloned().or(set);
                } else if entry_state {
                    // An address and a state are one value in two slots:
                    // a jump to the address the entry held goes in the
                    // state the entry held, never in one the body wrote.
                    if let (Some(values), Some(written)) =
                        (set.as_ref(), self.context.jump_sets.get(&name))
                    {
                        let entry: Vec<SymbolicValue> = values
                            .iter()
                            .filter(|value| !written.iter().any(|known| same_fact(known, value)))
                            .cloned()
                            .collect();
                        if !entry.is_empty() {
                            set = Some(entry.into());
                        }
                    }
                }
                if let Some(set) = set {
                    self.used_sets.push(name.clone());
                    let mut candidates: Vec<SymbolicValue> = set
                        .iter()
                        .filter(|value| match &assumed {
                            Some(Assumption::Not(excluded)) => {
                                !excluded.iter().any(|other| same_fact(other, value))
                            }
                            _ => true,
                        })
                        .cloned()
                        .collect();
                    // Past a catch a variable holds any value the `try` body
                    // gave it, the throw having come anywhere in the body; a
                    // VM jump there went to one of its VM addresses, as one
                    // to any other value leaves the code the VM runs. A jump
                    // to `undefined` with no `try` open is the `TypeError`
                    // dispatch throws (see above), kept.
                    if self.context.from_catch && is_vm_position(what) {
                        let no_try = self.context.try_frames.is_empty();
                        let addresses: Vec<SymbolicValue> = candidates
                            .iter()
                            .filter(|value| match value {
                                SymbolicValue::Number(number) => {
                                    *number >= 0.0 && number.fract() == 0.0
                                }
                                SymbolicValue::Undefined => no_try,
                                _ => false,
                            })
                            .cloned()
                            .collect();
                        if !addresses.is_empty() {
                            candidates = addresses;
                        }
                    }
                    if open && candidates.is_empty() {
                        // Not one of the addresses the cell's holders wrote:
                        // a JavaScript value.
                        return Err(self.unsupported_stop(
                            TraceStopCode::ProgramValueJump,
                            format!("{what} is none of the VM addresses {name} can hold"),
                            p.clone(),
                        ));
                    }
                    // One address, or a JavaScript value: a VM jump to the
                    // value leaves the code the VM runs, so the address is
                    // the one it goes to.
                    let only_address = is_vm_position(what)
                        && matches!(candidates.as_slice(), [SymbolicValue::Number(_)]);
                    if let ([only], false) = (candidates.as_slice(), open && !only_address) {
                        if what == "jump target" {
                            self.entry_address = self.entry_value_only(&name, only);
                        }
                        let value = EvalValue {
                            value: only.clone(),
                            fact: Some(only.clone()),
                            node: 0,
                        };
                        self.from_value_set = true;
                        let index = self.concrete_index(&value, what, p);
                        self.from_value_set = false;
                        return index;
                    }
                    if candidates.len() > MAX_VALUE_SET {
                        message = Some(format!(
                            "{what} is the variable {name}, which may hold {} values, more than {MAX_VALUE_SET}",
                            candidates.len()
                        ));
                    } else if !candidates.is_empty() {
                        match self.cell_reads.get(&name).copied() {
                            // After code that may write the cell, a test
                            // before the instruction tests another value.
                            Some((_, false)) => {
                                message = Some(format!(
                                    "{what} is the variable {name}, read from a shared cell after code that may write it"
                                ));
                            }
                            cell => {
                                self.dispatch = Some(ValueDispatch {
                                    name,
                                    candidates,
                                    cell: cell.map(|(read, _)| read),
                                });
                            }
                        }
                    }
                }
            }
            // A VM position is one of the bytecode constants the variable
            // was given: a jump to any other of its values leaves the code
            // the VM runs.
            if self.dispatch.is_none()
                && self.split.is_none()
                && self.inline.is_none()
                && is_vm_position(what)
            {
                // A value the path knows is a function, an object, a string
                // or a boolean is no VM position.
                if predicate::never_a_position(
                    &self.arena,
                    &self.aliases,
                    &self.joins,
                    &self.call_returns,
                    &self.context.type_facts,
                    &self.concrete_reads,
                    value.node,
                ) {
                    return Err(self.unsupported_stop(
                        TraceStopCode::ProgramValueJump,
                        format!(
                            "{what} is {}, which this path knows is no number",
                            self.describe(value.node)
                        ),
                        p.clone(),
                    ));
                }
                if let Some(name) = self.identifier_of(value.node) {
                    // What a dispatch's side learned of the variable: the
                    // value it assumes, or the values it ruled out. Without
                    // it the side that ruled one out dispatches on the same
                    // values again, a fork inside a fork without end.
                    let constants = self.position_definitions(&name).map(|constants| {
                        match self.context.assumed.get(&name) {
                            Some(Assumption::Is(value)) => vec![value.clone()],
                            Some(Assumption::Not(excluded)) => constants
                                .into_iter()
                                .filter(|value| !excluded.iter().any(|other| same_fact(other, value)))
                                .collect(),
                            None => constants,
                        }
                    });
                    match constants {
                        Some(constants) if constants.is_empty() => {
                            return Err(self.unsupported_stop(
                                TraceStopCode::ProgramValueJump,
                                format!(
                                    "{what} is {name}, every value of which is a program value"
                                ),
                                p.clone(),
                            ));
                        }
                        Some(mut constants) if constants.len() == 1 => {
                            let only = constants.remove(0);
                            let value = EvalValue {
                                value: only.clone(),
                                fact: Some(only),
                                node: 0,
                            };
                            self.from_value_set = true;
                            let index = self.concrete_index(&value, what, p);
                            self.from_value_set = false;
                            return index;
                        }
                        Some(constants) if constants.len() <= MAX_VALUE_SET => {
                            self.dispatch = Some(ValueDispatch {
                                name,
                                candidates: constants,
                                cell: None,
                            });
                        }
                        _ => {}
                    }
                }
            }
            // A VM jump straight to a program value, not one a variable may
            // have merged from elsewhere: no code the VM runs does that.
            // A shared cell's read is no program value: it may be a VM
            // constant some function that holds the cell wrote, its value
            // only forgotten because another function could change it. One
            // whose value the path knows (a cell no other function holds,
            // made `{ value: void 0 }` and never written) is that value.
            let program_value = is_vm_position(what)
                && self.dispatch.is_none()
                && self.split.is_none()
                && self.inline.is_none()
                && (value.fact.is_some() || {
                    let root = self.resolve_copies(value.node);
                    (self.is_data_read(root) || !self.is_cell_read(root))
                        && !matches!(
                            self.arena.nodes.get(root as usize).map(|node| &node.kind),
                            Some(TraceNodeKind::Identifier { .. }) | None
                        )
                });
            if program_value {
                self.junk_origin = self.carried_origin(value.node);
            }
            let code = if program_value {
                TraceStopCode::ProgramValueJump
            } else {
                TraceStopCode::DynamicIndex
            };
            let message = match message {
                Some(message) => message,
                None if self.dispatch.is_some() => {
                    format!("{what} is dispatched over the values its variable may hold")
                }
                None => format!(
                    "{what} is not a concrete non-negative integer: {}",
                    self.describe(value.node)
                ),
            };
            return Err(self.unsupported_stop(code, message, p.clone()));
        };
        if !number.is_finite()
            || *number < 0.0
            || number.fract() != 0.0
            || *number > usize::MAX as f64
        {
            return Err(self.unsupported_stop(
                TraceStopCode::InvalidIndex,
                format!("{what} {number} is outside the supported index range"),
                p.clone(),
            ));
        }
        Ok(*number as usize)
    }

    /// The text [`TraceArena::describe`] gives `node`, looking up only the
    /// variables it reads (in `definitions`) instead of building the
    /// bindings of every statement traced so far.
    pub(super) fn describe(&self, node: TraceNodeId) -> String {
        let mut bindings: BTreeMap<&str, TraceNodeId> = BTreeMap::new();
        let mut pending = vec![(node, 0usize)];
        let mut seen = BTreeSet::new();
        while let Some((at, depth)) = pending.pop() {
            if depth > 12 || !seen.insert((at, depth)) {
                continue;
            }
            let Some(kind) = self.arena.nodes.get(at as usize).map(|node| &node.kind) else {
                continue;
            };
            let TraceNodeKind::Identifier { name } = kind else {
                pending.extend(
                    super::emit::node_children(kind)
                        .into_iter()
                        .map(|child| (child, depth + 1)),
                );
                continue;
            };
            // Only generated names are in `definitions`.
            if generated_binding_serial(name).is_none() {
                return self.arena.describe(node);
            }
            // A reassigned name has no single value; else its last `let`.
            let mut value = None;
            let mut reassigned = false;
            for &(statement, defined) in self.definitions.get(name).into_iter().flatten() {
                match self
                    .arena
                    .statements
                    .get(statement as usize)
                    .map(|s| &s.kind)
                {
                    Some(TraceStatementKind::Assign { .. }) => reassigned = true,
                    Some(TraceStatementKind::Let { .. }) => value = Some(defined),
                    _ => {}
                }
            }
            if let Some(value) = value.filter(|_| !reassigned) {
                bindings.insert(name.as_str(), value);
                pending.push((value, depth + 1));
            }
        }
        let mut text = String::new();
        self.arena.describe_into(node, &bindings, 0, &mut text);
        text
    }

    pub(super) fn memory_slot(
        &self,
        index: &EvalIndex,
        p: &TraceProvenance,
    ) -> Result<u32, ExecutionStop> {
        u32::try_from(index.value).map_err(|_| {
            self.unsupported_stop(
                TraceStopCode::InvalidIndex,
                format!(
                    "memory slot {} is outside the supported slot range",
                    index.value
                ),
                p.clone(),
            )
        })
    }

    pub(super) fn ensure_stack_capacity(&mut self, index: usize) -> Result<(), ExecutionStop> {
        if index >= self.limits.max_stack_length {
            return Err(ExecutionStop::Budget(
                format!(
                    "stack index {index} exceeds limit {}",
                    self.limits.max_stack_length
                ),
                self.current.clone(),
            ));
        }
        if self.context.stack.len() <= index {
            self.context.stack.resize(index + 1, StackCell::Hole);
        }
        Ok(())
    }

    /// `$CELL_<slot>`, the variable holding a shared cell.
    pub(super) fn cell_variable(slot: u32) -> String {
        format!("$CELL_{slot}")
    }

    /// `cell.value` of the shared cell `slot`, read now. The value this path
    /// last wrote there, while still known, decides; the code reads the cell.
    pub(super) fn read_cell(
        &mut self,
        slot: u32,
        p: &TraceProvenance,
    ) -> Result<EvalValue, ExecutionStop> {
        self.consulted.insert(slot);
        let fact = self.context.cell_facts.get(&slot).cloned();
        let cell = self.identifier(&Self::cell_variable(slot), p)?;
        let key = self.add_node(TraceNodeKind::String("value".into()), p.clone())?;
        let read = self.add_node(TraceNodeKind::PropertyRead { object: cell, key }, p.clone())?;
        // A read with a value of its own known is never program data
        // from the contents: asking about it depends on nothing there.
        if fact.is_none() {
            self.read_slots.insert(read, slot);
        }
        self.cell_access = true;
        let bound = self.bind_value(computed(read), p);
        self.cell_access = false;
        let (name, mut value) = bound?;
        // What the path concretely put there, to opaque predicates: also
        // when the value is known, since a call the read is passed to sees
        // only the read.
        match self.context.cell_concrete.get(&slot) {
            Some(&written) if written != value.node => {
                self.concrete_reads.insert(value.node, written);
                if written != read {
                    self.concrete_reads.insert(read, written);
                }
            }
            Some(_) => {}
            None => {
                Arc::make_mut(&mut self.context.cell_concrete).insert(slot, value.node);
            }
        }
        if let Some(&(body, thrown)) = self.context.finally_concrete.get(&slot) {
            self.finally_reads.push((read, body, thrown));
            if value.node != read {
                self.finally_reads.push((value.node, body, thrown));
            }
        }
        // What this path last wrote there, to decisions.
        if fact.is_none() {
            match self.context.cell_nodes.get(&slot) {
                Some(&written) if written != value.node => {
                    self.aliases.insert(value.node, written);
                }
                Some(_) => {}
                // Not written on this path: until it is, or code runs that
                // may write it, every read is the value this one read.
                None => {
                    Arc::make_mut(&mut self.context.cell_nodes).insert(slot, value.node);
                }
            }
            if let Some(&position) = self.context.cell_positions.get(&slot) {
                self.position_reads.insert(value.node, position);
            } else if self.context.cell_values.contains(&slot) {
                self.data_reads.insert(read);
            }
        }
        // A read with a value of its own known takes nothing from the
        // contents: its answers do not depend on them.
        if fact.is_none() {
            self.read_names.insert(name.clone(), slot);
        }
        let count = self.instruction_reads.entry(slot).or_default();
        let this = CellRead { slot, read: *count };
        *count += 1;
        // With no value of its own known, it holds what the functions that
        // hold the cell put there.
        if fact.is_none() {
            if let Some(assumption) = self.context.cell_assumed.get(&this).cloned() {
                Arc::make_mut(&mut self.context.assumed).insert(name.clone(), assumption);
            }
            // All the read takes from the contents: see `CellReads::same`.
            match CellReadEffect::of(self.cell_contents.get(&slot)) {
                CellReadEffect::Nothing => {}
                CellReadEffect::Data => {
                    self.data_reads.insert(read);
                }
                CellReadEffect::Addresses(_, data) => {
                    self.cell_reads
                        .insert(name.clone(), (this, !self.instruction_ran_code));
                    if data {
                        self.open_sets.insert(name);
                    }
                }
            }
        }
        value.fact = fact;
        Ok(value)
    }

    /// What writing `value` puts in a shared cell, `None` for a VM value
    /// this function does not know.
    pub(super) fn cell_content_of(&self, value: &EvalValue) -> Option<CellContent> {
        self.stored_content(&StoredValue {
            value: value.value.clone(),
            fact: value.fact.clone(),
            node: Some(value.node),
        })
    }

    /// [`Self::cell_content_of`] for a value a cell holds.
    pub(super) fn stored_content(&self, stored: &StoredValue) -> Option<CellContent> {
        let mut content = CellContent::default();
        if let Some(values) = self.candidates(stored, None) {
            for value in values {
                content.merge(&Self::content_of_constant(&value));
            }
            return Some(content);
        }
        self.content_of_node(stored.node?, &mut content)
            .then_some(content)
    }

    /// What a constant puts in a cell: an address, or a JavaScript value.
    pub(super) fn content_of_constant(value: &SymbolicValue) -> CellContent {
        let mut content = CellContent::default();
        match value {
            SymbolicValue::Number(number)
                if *number >= 0.0 && number.fract() == 0.0 && *number < usize::MAX as f64 =>
            {
                content.addresses.insert(*number as usize);
            }
            _ => content.data = true,
        }
        content
    }

    /// Add what the value of `node` puts in a cell to `content`; false when
    /// that is not known: a variable of a loop or `try`, a parameter, or a
    /// shared cell's read not known to hold only JavaScript values. A
    /// variable a join made holds what its arms assigned; anything a
    /// program computes (a property, a call, arithmetic on program values,
    /// what an outlined subroutine returned, which would be a constant were
    /// it an address) is a JavaScript value. Each node is looked at once:
    /// joins share their arms.
    pub(super) fn content_of_node(&self, node: TraceNodeId, content: &mut CellContent) -> bool {
        let mut pending = vec![node];
        let mut seen = BTreeSet::new();
        while let Some(mut node) = pending.pop() {
            for _ in 0..64 {
                match self.aliases.get(&node) {
                    Some(&value) => node = value,
                    None => break,
                }
            }
            if !seen.insert(node) {
                continue;
            }
            if seen.len() > 4096 {
                return false;
            }
            if let Some(arms) = self.joins.get(&node) {
                pending.extend(arms.iter().copied());
                continue;
            }
            if self.is_data_read(node) {
                content.data = true;
                continue;
            }
            let Some(kind) = self.arena.nodes.get(node as usize).map(|node| &node.kind) else {
                return false;
            };
            match kind {
                TraceNodeKind::Number(text) => match parse_number_text(text) {
                    Some(number) => {
                        content.merge(&Self::content_of_constant(&SymbolicValue::Number(number)));
                    }
                    None => return false,
                },
                // What a catch binds: a thrown value.
                TraceNodeKind::Identifier { name } if self.catch_params.contains(name) => {
                    content.data = true;
                }
                // A variable holds one of the values it is given.
                TraceNodeKind::Identifier { name } if self.definitions.contains_key(name) => {
                    pending.extend(self.definitions[name].iter().map(|&(_, value)| value));
                }
                TraceNodeKind::Identifier { .. } => return false,
                TraceNodeKind::PropertyRead { .. } if self.is_cell_read(node) => return false,
                _ => content.data = true,
            }
        }
        true
    }

    /// The JavaScript types `value` may have (predicate type bits): its
    /// known value's, or what the evaluator makes of its node.
    pub(super) fn types_of(&self, value: &EvalValue) -> u16 {
        match &value.fact {
            Some(fact) => predicate::fact_types(fact),
            None => predicate::value_types(
                &self.arena,
                &self.aliases,
                &self.joins,
                &self.call_returns,
                &self.context.type_facts,
                value.node,
            ),
        }
    }

    /// [`Self::types_of`] for a value a cell holds.
    pub(super) fn stored_types(&self, stored: &StoredValue) -> u16 {
        match (&stored.fact, stored.node) {
            (Some(fact), _) => predicate::fact_types(fact),
            (None, Some(node)) => self.types_of(&EvalValue {
                value: stored.value.clone(),
                fact: None,
                node,
            }),
            (None, None) => predicate::ANY_TYPES,
        }
    }

    /// Record `types` as put in the shared cell `slot` (see
    /// [`Self::cell_types`]).
    pub(super) fn note_cell_types(&mut self, slot: u32, types: u16) {
        *self.cell_types.entry(slot).or_default() |= types;
    }

    /// Record `content` (`None`: not known) as put in the shared cell `slot`.
    pub(super) fn note_cell_content(&mut self, slot: u32, content: Option<CellContent>) {
        let entry = self
            .cell_writes
            .entry(slot)
            .or_insert_with(|| Some(CellContent::default()));
        match (entry.as_mut(), content) {
            (Some(known), Some(content)) => known.merge(&content),
            (Some(_), None) => *entry = None,
            (None, _) => {}
        }
    }

    /// `cell.value = value` of the shared cell `slot`.
    pub(super) fn write_cell(
        &mut self,
        slot: u32,
        value: &EvalValue,
        p: &TraceProvenance,
    ) -> Result<(), ExecutionStop> {
        let object = self.identifier(&Self::cell_variable(slot), p)?;
        let key = self.add_node(TraceNodeKind::String("value".into()), p.clone())?;
        self.cell_access = true;
        self.statement(
            TraceStatementKind::PropertyWrite {
                object,
                key,
                value: value.node,
            },
            p.clone(),
        );
        self.cell_access = false;
        if value.fact.is_none() && self.from_message(value.node) {
            self.message_cells.insert(slot);
        }

        let content = self.cell_content_of(value);
        // What a program computes is no VM position (see `content_of_node`).
        let data = content
            .as_ref()
            .is_some_and(|content| content.addresses.is_empty() && content.data);
        // A parameter: each call puts its argument there.
        let param = match self
            .arena
            .nodes
            .get(self.resolve_copies(value.node) as usize)
        {
            Some(TraceNode {
                kind: TraceNodeKind::Identifier { name },
                ..
            }) if content.is_none() && self.context.params.contains(name) => Some(name.clone()),
            _ => None,
        };
        let types = match param {
            Some(_) => predicate::ANY_TYPES,
            None => self.types_of(value),
        };
        self.note_cell_types(slot, types);
        match param {
            Some(name) => Arc::make_mut(&mut self.context.param_writes).push((slot, name)),
            None => self.note_cell_content(slot, content),
        }
        let facts = Arc::make_mut(&mut self.context.cell_facts);
        match &value.fact {
            Some(fact) => facts.insert(slot, fact.clone()),
            None => facts.remove(&slot),
        };
        Arc::make_mut(&mut self.context.cell_nodes).insert(slot, value.node);
        Arc::make_mut(&mut self.context.cell_concrete).insert(slot, value.node);
        if self.context.finally_concrete.contains_key(&slot) {
            Arc::make_mut(&mut self.context.finally_concrete).remove(&slot);
        }
        self.note_body_cell_write(slot);
        self.write_serial += 1;
        Arc::make_mut(&mut self.context.cell_writes).insert(slot, self.write_serial);
        let positions = Arc::make_mut(&mut self.context.cell_positions);
        let values = Arc::make_mut(&mut self.context.cell_values);
        match &value.fact {
            Some(SymbolicValue::Number(number))
                if *number >= 0.0 && number.fract() == 0.0 && *number <= u32::MAX as f64 =>
            {
                positions.insert(slot, *number as usize);
                values.remove(&slot);
            }
            Some(_) => {
                positions.remove(&slot);
                values.insert(slot);
            }
            None => {
                positions.remove(&slot);
                if data {
                    values.insert(slot);
                } else {
                    values.remove(&slot);
                }
            }
        }
        Ok(())
    }

    /// `$CELL_<slot> = cell`: the slot gets a cell, `{ value }` or one the
    /// caller passed.
    pub(super) fn assign_cell(
        &mut self,
        slot: u32,
        cell: TraceNodeId,
        fact: Option<SymbolicValue>,
        p: &TraceProvenance,
    ) {
        self.cell_access = true;
        self.statement(
            TraceStatementKind::Assign {
                name: Self::cell_variable(slot),
                value: cell,
            },
            p.clone(),
        );
        self.cell_access = false;
        // A new cell made with message data, such as the handler's message
        // argument: what it holds is that data.
        if let Some(TraceNodeKind::ObjectLiteral { values, .. }) =
            self.arena.nodes.get(cell as usize).map(|node| &node.kind)
        {
            if fact.is_none() && values.len() == 1 && self.from_message(values[0]) {
                self.message_cells.insert(slot);
            }
        }
        let values = Arc::make_mut(&mut self.context.cell_values);
        match &fact {
            Some(SymbolicValue::Number(_)) | None => values.remove(&slot),
            Some(_) => values.insert(slot),
        };
        let facts = Arc::make_mut(&mut self.context.cell_facts);
        match fact {
            Some(fact) => facts.insert(slot, fact),
            None => facts.remove(&slot),
        };
        Arc::make_mut(&mut self.context.cell_nodes).remove(&slot);
        Arc::make_mut(&mut self.context.cell_positions).remove(&slot);
        // A cell made here holds what it was made with.
        let concrete = match self.arena.nodes.get(cell as usize).map(|node| &node.kind) {
            Some(TraceNodeKind::ObjectLiteral { values, .. }) if values.len() == 1 => {
                Some(values[0])
            }
            _ => None,
        };
        match concrete {
            Some(node) => Arc::make_mut(&mut self.context.cell_concrete).insert(slot, node),
            None => Arc::make_mut(&mut self.context.cell_concrete).remove(&slot),
        };
        if self.context.finally_concrete.contains_key(&slot) {
            Arc::make_mut(&mut self.context.finally_concrete).remove(&slot);
        }
        self.note_body_cell_write(slot);
        self.write_serial += 1;
        Arc::make_mut(&mut self.context.cell_writes).insert(slot, self.write_serial);
    }

    /// `{ value: value }`, a new cell.
    pub(super) fn new_cell(
        &mut self,
        value: TraceNodeId,
        p: &TraceProvenance,
    ) -> Result<TraceNodeId, ExecutionStop> {
        self.add_node(
            TraceNodeKind::ObjectLiteral {
                keys: vec!["value".into()],
                values: vec![value],
            },
            p.clone(),
        )
    }

    pub(super) fn bind_value(
        &mut self,
        evaluated: EvalValue,
        p: &TraceProvenance,
    ) -> Result<(String, EvalValue), ExecutionStop> {
        let name = format!("trace_value_{}", self.binding_serial);
        self.binding_serial += 1;
        let node = self.identifier(&name, p)?;
        self.aliases.insert(node, evaluated.node);
        self.statement(
            TraceStatementKind::Let {
                name: name.clone(),
                value: evaluated.node,
            },
            p.clone(),
        );
        Ok((
            name,
            EvalValue {
                value: evaluated.value,
                fact: evaluated.fact,
                node,
            },
        ))
    }

    /// `context.receiverValue` is the call's `this`; `context.receiverObject`
    /// is `this == null ? host : Object(this)`, as `vmContext` sets it.
    pub(super) fn receiver(
        &mut self,
        field: ReceiverField,
        p: &TraceProvenance,
    ) -> Result<EvalValue, ExecutionStop> {
        let Some(receiver) = self.context.receiver.clone() else {
            return Err(self.unsupported_stop(
                TraceStopCode::UnknownInput,
                format!(
                    "context.{} read without a known VM function receiver",
                    field.as_str()
                ),
                p.clone(),
            ));
        };
        match (field, &receiver) {
            (ReceiverField::Value, _) | (ReceiverField::Object, SymbolicValue::Object { .. }) => {
                self.value_node(receiver, p)
            }
            (ReceiverField::Object, SymbolicValue::Undefined | SymbolicValue::Null) => {
                let node = self.add_node(TraceNodeKind::Global { name: HOST.into() }, p.clone())?;
                Ok(computed(node))
            }
            (ReceiverField::Object, _) => {
                let value = self.value_node(receiver, p)?;
                let callee = self.add_node(
                    TraceNodeKind::Global {
                        name: "Object".into(),
                    },
                    p.clone(),
                )?;
                let this_value = self.add_node(TraceNodeKind::Undefined, p.clone())?;
                let node = self.add_node(
                    TraceNodeKind::Call {
                        callee,
                        this_value,
                        arguments: vec![value.node],
                    },
                    p.clone(),
                )?;
                Ok(computed(node))
            }
        }
    }

    pub(super) fn value_node(
        &mut self,
        value: SymbolicValue,
        p: &TraceProvenance,
    ) -> Result<EvalValue, ExecutionStop> {
        if let SymbolicValue::Unknown { reason } = &value {
            return Err(self.unsupported_stop(
                TraceStopCode::UnknownInput,
                reason.clone(),
                p.clone(),
            ));
        }
        let kind = match &value {
            SymbolicValue::Undefined => TraceNodeKind::Undefined,
            SymbolicValue::Null => TraceNodeKind::Null,
            SymbolicValue::Boolean(v) => TraceNodeKind::Boolean(*v),
            SymbolicValue::Number(v) => TraceNodeKind::Number(number_text(*v)),
            SymbolicValue::String(v) => TraceNodeKind::String(v.clone()),
            SymbolicValue::Object { identity } => TraceNodeKind::Object {
                identity: *identity,
            },
            SymbolicValue::External { name } => {
                if !self.external_inputs.contains(name) {
                    Arc::make_mut(&mut self.external_inputs).insert(name.clone());
                }
                TraceNodeKind::External { name: name.clone() }
            }
            SymbolicValue::Computed { node } => {
                if (*node as usize) >= self.arena.nodes.len() {
                    return Err(self.unsupported_stop(
                        TraceStopCode::InvalidComputedInput,
                        format!("computed node {node} is not in this trace arena"),
                        p.clone(),
                    ));
                }
                return Ok(EvalValue {
                    value: value.clone(),
                    fact: None,
                    node: *node,
                });
            }
            SymbolicValue::Unknown { .. } => unreachable!(),
        };
        let node = self.add_node(kind, p.clone())?;
        let fact = concrete_fact(&value);
        Ok(EvalValue { value, fact, node })
    }

    pub(super) fn literal_node(
        &mut self,
        literal: &LiteralValue,
        p: &TraceProvenance,
    ) -> Result<EvalValue, ExecutionStop> {
        let value = match literal {
            LiteralValue::DecodedNumber { bits } => SymbolicValue::Number(f64::from_bits(*bits)),
            LiteralValue::Number(raw) => {
                SymbolicValue::Number(parse_numeric_literal(raw).ok_or_else(|| {
                    self.unsupported_stop(
                        TraceStopCode::InvalidNumber,
                        format!("invalid numeric literal {raw:?}"),
                        p.clone(),
                    )
                })?)
            }
            LiteralValue::String(value) => SymbolicValue::String(value.clone()),
            LiteralValue::Boolean(value) => SymbolicValue::Boolean(*value),
            LiteralValue::Null => SymbolicValue::Null,
            LiteralValue::Undefined => SymbolicValue::Undefined,
        };
        self.value_node(value, p)
    }

    pub(super) fn add_node(
        &mut self,
        kind: TraceNodeKind,
        provenance: TraceProvenance,
    ) -> Result<TraceNodeId, ExecutionStop> {
        if self.arena.nodes.len() >= self.limits.max_nodes {
            return Err(ExecutionStop::Budget(
                "node budget exhausted".into(),
                Some(provenance),
            ));
        }
        let id = self.arena.nodes.len() as TraceNodeId;
        self.arena.nodes.push(TraceNode {
            id,
            kind,
            provenance,
        });
        Ok(id)
    }

    pub(super) fn identifier(
        &mut self,
        name: &str,
        p: &TraceProvenance,
    ) -> Result<TraceNodeId, ExecutionStop> {
        self.add_node(TraceNodeKind::Identifier { name: name.into() }, p.clone())
    }

    pub(super) fn statement(
        &mut self,
        kind: TraceStatementKind,
        provenance: TraceProvenance,
    ) -> TraceStatementId {
        if let TraceStatementKind::Let { name, value }
        | TraceStatementKind::Assign { name, value } = &kind
        {
            if generated_binding_serial(name).is_some() {
                let id = self.arena.statements.len() as TraceStatementId;
                self.definitions.push(name, id, *value);
            }
        }
        let runs_code = statement_runs_code(&self.arena, &kind)
            && !self.reads_message(&kind)
            && !matches!(&kind, TraceStatementKind::Let { value, .. } | TraceStatementKind::Assign { value, .. }
                if self.number_arithmetic(*value));
        if !self.cell_access && !self.instruction_ran_code && runs_code {
            self.instruction_ran_code = true;
        }
        // Code that may call out may run a closure that writes a cell it
        // holds: what this path wrote there is no longer known.
        if runs_code
            && !self.cell_access
            && self.context.escaped_cells.iter().any(|slot| {
                self.context.cell_facts.contains_key(slot)
                    || self.context.cell_nodes.contains_key(slot)
            })
        {
            let escaped = self.context.escaped_cells.clone();
            Arc::make_mut(&mut self.context.cell_facts).retain(|slot, _| !escaped.contains(slot));
            Arc::make_mut(&mut self.context.cell_nodes).retain(|slot, _| !escaped.contains(slot));
        }
        // An object only this path held that the statement hands to other
        // code: what it holds is no longer known.
        if !self.context.made_objects.is_empty() {
            let escaped = self.escaping_objects(&kind);
            if !escaped.is_empty() {
                let objects = Arc::make_mut(&mut self.context.made_objects);
                for object in escaped {
                    objects.remove(&object);
                }
            }
        }
        let id = self.arena.statements.len() as TraceStatementId;
        self.arena.statements.push(TraceStatement {
            id,
            kind,
            provenance,
        });
        if let Some(arm) = self.block_statements_mut() {
            arm.push(id);
        }
        id
    }

    pub(super) fn event(&mut self, kind: TraceEventKind, provenance: TraceProvenance) {
        let id = self.arena.events.len() as u32;
        self.arena.events.push(TraceEvent {
            id,
            kind,
            provenance,
        });
    }

    pub(super) fn unsupported_stop(
        &self,
        code: TraceStopCode,
        message: String,
        p: TraceProvenance,
    ) -> ExecutionStop {
        ExecutionStop::Unsupported(code, message, Some(p))
    }
}

impl Executor {
    pub(super) fn eval(
        &mut self,
        value: &SemanticValue,
        instruction: &DecodedInstruction,
        p: &TraceProvenance,
    ) -> Result<EvalValue, ExecutionStop> {
        match value {
            SemanticValue::Context => Err(self.unsupported_stop(
                TraceStopCode::ContextValue,
                "the context object is not a recipe value".into(),
                p.clone(),
            )),
            SemanticValue::Operand { position } => {
                let Some(operand_index) = position.checked_sub(1) else {
                    return Err(self.unsupported_stop(
                        TraceStopCode::ContextOperand,
                        "recipe operand position 0 names the context parameter".into(),
                        p.clone(),
                    ));
                };
                let Some(value) = instruction.operands.get(operand_index as usize).cloned() else {
                    return Err(self.unsupported_stop(
                        TraceStopCode::MissingOperand,
                        format!("decoded operand {operand_index} was not supplied"),
                        p.clone(),
                    ));
                };
                self.value_node(value, p)
            }
            SemanticValue::Temporary { id } => self.temp_values.get(id).cloned().ok_or_else(|| {
                self.unsupported_stop(
                    TraceStopCode::MissingTemporary,
                    format!("temporary {id} was read before initialization"),
                    p.clone(),
                )
            }),
            SemanticValue::Literal(literal) => self.literal_node(literal, p),
            SemanticValue::StackLength => {
                let value =
                    self.value_node(SymbolicValue::Number(self.context.stack.len() as f64), p)?;
                self.event(
                    TraceEventKind::StackLengthRead { value: value.node },
                    p.clone(),
                );
                Ok(value)
            }
            SemanticValue::StackRead { index } => {
                let index = self.eval_index(index, instruction, p)?;
                let value = self.resolve_stack(index.value, p)?;
                self.event(
                    TraceEventKind::StackRead {
                        index: index.node,
                        value: value.node,
                    },
                    p.clone(),
                );
                Ok(value)
            }
            SemanticValue::MemoryRead { slot } => {
                let slot = self.eval_index(slot, instruction, p)?;
                let slot_number = self.memory_slot(&slot, p)?;
                if self.cells.contains(&slot_number) {
                    if !self.context.memory.contains_key(&slot_number) {
                        return Err(self.missing_slot(false, p));
                    }
                    return self.read_cell(slot_number, p);
                }
                let stored = match self.context.memory.get(&slot_number) {
                    Some(MemoryCell::Value(value)) => value.clone(),
                    Some(MemoryCell::Initialized) => stored_value(SymbolicValue::Undefined),
                    Some(MemoryCell::Param) => self.param_value(TraceCell::Memory(slot_number))?,
                    // `this.slots[slot].value` with no such cell.
                    None => return Err(self.missing_slot(false, p)),
                };
                let value = self.resolve_memory(slot_number as usize, stored, p)?;
                self.event(
                    TraceEventKind::MemoryRead {
                        slot: slot.node,
                        value: value.node,
                    },
                    p.clone(),
                );
                Ok(value)
            }
            SemanticValue::Binary {
                operator,
                left,
                right,
            } => {
                let left = self.eval(left, instruction, p)?;
                let right = self.eval(right, instruction, p)?;
                let node = self.add_node(
                    TraceNodeKind::Binary {
                        operator: *operator,
                        left: left.node,
                        right: right.node,
                    },
                    p.clone(),
                )?;
                let fact = fold_binary(*operator, left.fact.as_ref(), right.fact.as_ref());
                Ok(EvalValue {
                    value: fact.clone().unwrap_or(SymbolicValue::Computed { node }),
                    fact,
                    node,
                })
            }
            SemanticValue::Unary { operator, value } => {
                let value = self.eval(value, instruction, p)?;
                let node = self.add_node(
                    TraceNodeKind::Unary {
                        operator: *operator,
                        value: value.node,
                    },
                    p.clone(),
                )?;
                let fact = fold_unary(*operator, value.fact.as_ref());
                Ok(EvalValue {
                    value: fact.clone().unwrap_or(SymbolicValue::Computed { node }),
                    fact,
                    node,
                })
            }
            SemanticValue::Unknown { reason, .. } => {
                Err(self.unsupported_stop(TraceStopCode::UnknownValue, reason.clone(), p.clone()))
            }
            SemanticValue::Binding { name } => {
                let node =
                    self.add_node(TraceNodeKind::Global { name: name.clone() }, p.clone())?;
                Ok(computed(node))
            }
            SemanticValue::PropertyRead { object, key } => {
                let object_value = self.eval(object, instruction, p)?;
                let key_value = self.eval(key, instruction, p)?;
                // A property this path wrote to an object only it holds.
                if let Some(SymbolicValue::String(name)) = &key_value.fact {
                    let handle = self.object_handle(object_value.node);
                    if let Some(value) = self
                        .context
                        .made_objects
                        .get(&handle)
                        .and_then(|properties| properties.get(name))
                        .cloned()
                    {
                        return self.value_node(value, p);
                    }
                }
                if let Some(entry) = self.constant_entry(object, &key_value) {
                    // String decoding: a constant-table read at a concrete
                    // index is that entry, as a string literal.
                    return self.value_node(SymbolicValue::String(entry), p);
                }
                if let Some(message) = self.past_constant_table(object, &key_value) {
                    return Err(self.unsupported_stop(
                        TraceStopCode::ConstantTableIndex,
                        message,
                        p.clone(),
                    ));
                }
                if let SemanticValue::Binding { name } = object.as_ref() {
                    if let Some(table) = self.number_tables.get(name) {
                        let entry = match key_value.fact.as_ref() {
                            Some(SymbolicValue::Number(index))
                                if index.fract() == 0.0 && *index >= 0.0 =>
                            {
                                table.get(*index as usize).copied()
                            }
                            _ => None,
                        };
                        let Some(entry) = entry else {
                            return Err(self.unsupported_stop(
                                TraceStopCode::InvalidIndex,
                                format!(
                                    "{name} is read at an index that is not one of its entries"
                                ),
                                p.clone(),
                            ));
                        };
                        return self.value_node(SymbolicValue::Number(entry), p);
                    }
                }
                let node = self.add_node(
                    TraceNodeKind::PropertyRead {
                        object: object_value.node,
                        key: key_value.node,
                    },
                    p.clone(),
                )?;
                Ok(computed(node))
            }
            SemanticValue::ObjectLiteral { properties } => {
                let mut keys = Vec::with_capacity(properties.len());
                let mut values = Vec::with_capacity(properties.len());
                let mut known = BTreeMap::new();
                for (key, value) in properties {
                    let value = self.eval(value, instruction, p)?;
                    if let Some(fact) = value.fact.filter(is_primitive_fact) {
                        known.insert(key.clone(), fact);
                    }
                    keys.push(key.clone());
                    values.push(value.node);
                }
                let node =
                    self.add_node(TraceNodeKind::ObjectLiteral { keys, values }, p.clone())?;
                Arc::make_mut(&mut self.context.made_objects).insert(node, known);
                Ok(computed(node))
            }
            SemanticValue::ArrayLiteral { elements } => {
                let mut nodes = Vec::with_capacity(elements.len());
                for element in elements {
                    nodes.push(self.eval(element, instruction, p)?.node);
                }
                let node =
                    self.add_node(TraceNodeKind::ArrayLiteral { elements: nodes }, p.clone())?;
                Ok(computed(node))
            }
            SemanticValue::StackSlice { start } => {
                let start = self.eval_index(start, instruction, p)?;
                let length = self.context.stack.len();
                if start.value > length {
                    return Err(self.unsupported_stop(
                        TraceStopCode::InvalidIndex,
                        format!(
                            "stack slice start {} is past the stack length {length}",
                            start.value
                        ),
                        p.clone(),
                    ));
                }
                let mut elements = Vec::with_capacity(length - start.value);
                for index in start.value..length {
                    elements.push(self.resolve_stack(index, p)?.node);
                }
                let node = self.add_node(TraceNodeKind::ArrayLiteral { elements }, p.clone())?;
                Ok(computed(node))
            }
            SemanticValue::Receiver { field } => self.receiver(*field, p),
            SemanticValue::CreateThread {
                descriptor,
                byte_offset,
                dispatch_state,
            } => {
                // The thread's entry is VM values: concrete or no thread.
                let descriptor = self.eval(descriptor, instruction, p)?;
                let byte_offset_value = self.eval(byte_offset, instruction, p)?;
                let dispatch_state_value = self.eval(dispatch_state, instruction, p)?;
                // A VM Worker runs the function a message names: what it
                // creates is that call, as the kernel makes it. Only the
                // functions the page posts reach it, each checked to share no
                // cells (see [`Self::posted_to_worker`]).
                let entry = [&descriptor, &byte_offset_value, &dispatch_state_value];

                if entry.iter().any(|value| self.from_message(value.node)) {
                    let callee = self.add_node(
                        TraceNodeKind::Global {
                            name: VM_CREATE_THREAD.into(),
                        },
                        p.clone(),
                    )?;
                    let this_value = self.value_node(SymbolicValue::Undefined, p)?;
                    let node = self.add_node(
                        TraceNodeKind::Call {
                            callee,
                            this_value: this_value.node,
                            arguments: entry.iter().map(|value| value.node).collect(),
                        },
                        p.clone(),
                    )?;
                    return Ok(computed(node));
                }
                let descriptor = self.concrete_index(&descriptor, "thread descriptor", p)?;
                let byte_offset =
                    self.concrete_index(&byte_offset_value, "thread byte offset", p)?;
                let dispatch_state =
                    self.concrete_index(&dispatch_state_value, "thread dispatch state", p)?;
                // `vmCaptureThread` hands the closure this function's cells
                // of the slots it captures: from then on both read and
                // write the same cells.
                let captured = self
                    .captures
                    .as_ref()
                    .map(|captures| captures.get(descriptor).cloned().unwrap_or_default());
                let memory =
                    match captured {
                        Some(slots) if !slots.is_empty() => {
                            let missing: Vec<u32> = slots
                                .iter()
                                .copied()
                                .filter(|slot| !self.cells.contains(slot))
                                .collect();
                            // Slots that are not cells yet: the walk goes on
                            // without them, to find every such closure, and
                            // the driver traces the function again with them
                            // as cells (see [`TraceSession::take_wanted_cells`]).
                            self.wanted_cells.extend(missing.iter().copied());
                            let slots: Vec<u32> = slots
                                .iter()
                                .copied()
                                .filter(|slot| self.cells.contains(slot))
                                .collect();
                            let mut values = Vec::with_capacity(slots.len());
                            for slot in &slots {
                                values.push(self.identifier(&Self::cell_variable(*slot), p)?);
                            }
                            Arc::make_mut(&mut self.context.escaped_cells)
                                .extend(slots.iter().copied());
                            // A closure the body makes may write them when
                            // the body calls it.
                            for &slot in &slots {
                                self.note_body_cell_write(slot);
                            }
                            let keys = slots.iter().map(u32::to_string).collect();
                            Some(self.add_node(
                                TraceNodeKind::ObjectLiteral { keys, values },
                                p.clone(),
                            )?)
                        }
                        _ => None,
                    };
                let node = self.add_node(
                    TraceNodeKind::VmFunction {
                        descriptor,
                        byte_offset,
                        dispatch_state,
                        memory,
                    },
                    p.clone(),
                )?;
                if let Some(created) = &self.created_hook {
                    created(descriptor, byte_offset, dispatch_state);
                }
                self.created_now
                    .push((descriptor, byte_offset, dispatch_state));
                Ok(computed(node))
            }
            SemanticValue::Call {
                callee: callee_value,
                this_value,
                arguments,
            } => {
                let callee = self.eval(callee_value, instruction, p)?;
                // `undefined` or `null` may be what a `== null` test let the
                // path assume; a string, number or boolean is a literal.
                if let Some(
                    primitive @ (SymbolicValue::Boolean(_)
                    | SymbolicValue::Number(_)
                    | SymbolicValue::String(_)),
                ) = &callee.fact
                {
                    return Err(self.unsupported_stop(
                        TraceStopCode::PrimitiveCall,
                        format!("a VM call of {primitive:?}, which is not a function"),
                        p.clone(),
                    ));
                }
                let this_value = self.eval(this_value, instruction, p)?;
                // A post to the VM Worker: its arguments' known values name
                // the function it runs (see [`Self::posted_to_worker`]).
                let posted = matches!(callee_value.as_ref(), SemanticValue::Binding { name }
                    if name == crate::operations::templates::POST_TO_WORKER);
                // A call of the proven `vmDecodeString` with its own table,
                // cache and decoder: known operands make it a string literal.
                let decoder = self.string_decoder.clone().filter(|decoder| {
                    matches!(callee_value.as_ref(), SemanticValue::Binding { name }
                        if *name == decoder.helper)
                        && matches!(arguments.as_ref(), SemanticValue::ArrayLiteral { elements }
                            if decodes_with(decoder, elements))
                });
                let arguments =
                    match (posted, arguments.as_ref()) {
                        (false, SemanticValue::ArrayLiteral { elements }) if decoder.is_some() => {
                            let mut values = Vec::with_capacity(elements.len());
                            for element in elements {
                                values.push(self.eval(element, instruction, p)?);
                            }
                            let decoded = match (&values[0].fact, &values[1].fact, &decoder) {
                                (
                                    Some(SymbolicValue::String(encoded)),
                                    Some(SymbolicValue::Number(key)),
                                    Some(decoder),
                                ) if key.fract() == 0.0 && *key >= 0.0 => {
                                    decoder.decode(encoded, *key as usize)
                                }
                                _ => None,
                            };
                            if let Some(decoded) = decoded {
                                return self.value_node(SymbolicValue::String(decoded), p);
                            }
                            let nodes = values.iter().map(|value| value.node).collect();
                            computed(self.add_node(
                                TraceNodeKind::ArrayLiteral { elements: nodes },
                                p.clone(),
                            )?)
                        }
                        (true, SemanticValue::ArrayLiteral { elements }) => {
                            let mut values = Vec::with_capacity(elements.len());
                            for element in elements {
                                values.push(self.eval(element, instruction, p)?);
                            }
                            self.posted_to_worker(&values, p)?;
                            // The function it names, written as the numbers it
                            // is: what a walk creates is read from them.
                            for index in [0, 1, 3] {
                                let fact = values[index].fact.clone().expect("checked by the post");
                                values[index] = self.value_node(fact, p)?;
                            }
                            let nodes = values.iter().map(|value| value.node).collect();
                            computed(self.add_node(
                                TraceNodeKind::ArrayLiteral { elements: nodes },
                                p.clone(),
                            )?)
                        }
                        _ => self.eval(arguments, instruction, p)?,
                    };
                // The argument count is a VM value: the argument list must be
                // an array whose elements are known.
                let Some(TraceNodeKind::ArrayLiteral { elements }) = self
                    .arena
                    .nodes
                    .get(arguments.node as usize)
                    .map(|node| &node.kind)
                else {
                    return Err(self.unsupported_stop(
                        TraceStopCode::DynamicCallArguments,
                        "call arguments are not an array of known length".into(),
                        p.clone(),
                    ));
                };
                let arguments = elements.clone();
                let node = self.add_node(
                    TraceNodeKind::Call {
                        callee: callee.node,
                        this_value: this_value.node,
                        arguments,
                    },
                    p.clone(),
                )?;
                self.note_defined_property(node);
                // A VM function made here whose every return is a VM
                // function: the call is a function, to tests.
                if self.returns_hook.is_some() && self.makes_functions(callee.node) {
                    // Some function, to tests only: it is no function the
                    // walk makes.
                    let made = self.add_node(
                        TraceNodeKind::Function {
                            source: String::new(),
                        },
                        p.clone(),
                    )?;
                    self.call_returns.insert(node, vec![made]);
                } else if let Some(written) = self
                    .concrete_reads
                    .get(&callee.node)
                    .or_else(|| self.concrete_reads.get(&self.resolve_copies(callee.node)))
                    .copied()
                    .filter(|_| self.returns_hook.is_some())
                    .filter(|&written| self.makes_functions(written))
                {
                    // A call of a shared cell's read where the path
                    // concretely put such a VM function: a function, to
                    // opaque predicates only, as the read is that function
                    // to them (a closure may have written the cell since).
                    let made = self.add_node(
                        TraceNodeKind::Function {
                            source: String::new(),
                        },
                        p.clone(),
                    )?;
                    self.concrete_reads.insert(node, made);
                }
                Ok(computed(node))
            }
            SemanticValue::PoppedTryFrame { field } => {
                let Some(frame) = self.popped_try else {
                    return Err(self.unsupported_stop(
                        TraceStopCode::EmptyExceptionStack,
                        format!("try frame {} read before a pop", field.as_str()),
                        p.clone(),
                    ));
                };
                let value = match field {
                    TryFrameField::HandlerByteOffset => frame.handler_byte_offset,
                    TryFrameField::HandlerDispatchState => frame.handler_dispatch_state,
                };
                self.value_node(SymbolicValue::Number(value as f64), p)
            }
            SemanticValue::PoppedErrorRecord { field } => {
                let Some(record) = self.popped_error else {
                    return Err(self.unsupported_stop(
                        TraceStopCode::EmptyExceptionStack,
                        format!("error record {} read before a pop", field.as_str()),
                        p.clone(),
                    ));
                };
                let value = match (field, record.saved) {
                    (ErrorRecordField::WasExceptionHandled, _) => {
                        SymbolicValue::Boolean(record.handled)
                    }
                    (ErrorRecordField::SavedByteOffset, Some(saved)) => {
                        SymbolicValue::Number(saved.return_byte_offset as f64)
                    }
                    (ErrorRecordField::SavedDispatchState, Some(saved)) => {
                        SymbolicValue::Number(saved.return_dispatch_state as f64)
                    }
                    (_, None) => SymbolicValue::Undefined,
                };
                self.value_node(value, p)
            }
            SemanticValue::PopThrownValue => self.pop_thrown(p),
            SemanticValue::FunctionLiteral { source } => {
                let node = self.add_node(
                    TraceNodeKind::Function {
                        source: source.clone(),
                    },
                    p.clone(),
                )?;
                Ok(computed(node))
            }
            SemanticValue::ClosurePlan { plan } => {
                let node = self.add_node(
                    TraceNodeKind::Function {
                        source: plan.to_source(),
                    },
                    p.clone(),
                )?;
                Ok(computed(node))
            }
            SemanticValue::Delete { object, key } => {
                let object = self.eval(object, instruction, p)?;
                let key = self.eval(key, instruction, p)?;
                let node = self.add_node(
                    TraceNodeKind::Delete {
                        object: object.node,
                        key: key.node,
                    },
                    p.clone(),
                )?;
                Ok(computed(node))
            }
            SemanticValue::Construct { callee, arguments } => {
                let callee = self.eval(callee, instruction, p)?;
                let arguments = self.eval(arguments, instruction, p)?;
                let Some(TraceNodeKind::ArrayLiteral { elements }) = self
                    .arena
                    .nodes
                    .get(arguments.node as usize)
                    .map(|node| &node.kind)
                else {
                    return Err(self.unsupported_stop(
                        TraceStopCode::DynamicCallArguments,
                        "constructor arguments are not an array of known length".into(),
                        p.clone(),
                    ));
                };
                let arguments = elements.clone();
                let node = self.add_node(
                    TraceNodeKind::New {
                        callee: callee.node,
                        arguments,
                    },
                    p.clone(),
                )?;
                Ok(computed(node))
            }
            SemanticValue::PoppedCallFrame { field } => {
                let Some(frame) = self.popped_frame else {
                    return Err(self.unsupported_stop(
                        TraceStopCode::CallFrameReadBeforePop,
                        format!("call frame {} read before a pop", field.as_str()),
                        p.clone(),
                    ));
                };
                let value = match field {
                    CallFrameField::ReturnByteOffset => frame.return_byte_offset,
                    CallFrameField::ReturnDispatchState => frame.return_dispatch_state,
                };
                self.value_node(SymbolicValue::Number(value as f64), p)
            }
        }
    }
}

/// An abstract AST node value: syntax retained, concrete result not proven.
/// `context` learns the variable `name` (or, for the shared cell's read
/// `cell`, what the instruction reads again) is `value`, or is not.
pub(super) fn learn(
    context: &mut TraceContext,
    name: &str,
    cell: Option<CellRead>,
    value: SymbolicValue,
    is: bool,
) {
    let unknown = || Assumption::Not(Vec::new());
    let update = |assumption: &mut Assumption, value: SymbolicValue| match assumption {
        _ if is => *assumption = Assumption::Is(value),
        Assumption::Not(values) => values.push(value),
        Assumption::Is(_) => {}
    };
    // The read's variable too: an instruction that takes the value an
    // earlier one read (from the stack, say) reads no cell again.
    let named = Arc::make_mut(&mut context.assumed)
        .entry(name.into())
        .or_insert_with(unknown);
    update(named, value.clone());
    if let Some(read) = cell {
        let read = Arc::make_mut(&mut context.cell_assumed)
            .entry(read)
            .or_insert_with(unknown);
        update(read, value);
    }
}

/// Whether `arguments` are what `decoder`'s helper is called with: an
/// encoded string, a key index, its own table, cache and base64 decoder, and
/// `String.fromCharCode`.
fn decodes_with(decoder: &crate::vm_string::VmStringDecoder, arguments: &[SemanticValue]) -> bool {
    let binding = |value: &SemanticValue, expected: &str| matches!(value, SemanticValue::Binding { name } if name == expected);
    let [_, _, table, cache, base64, from_char_code] = arguments else {
        return false;
    };
    binding(table, &decoder.table_name)
        && binding(cache, &decoder.cache)
        && binding(base64, &decoder.decoder)
        && matches!(from_char_code, SemanticValue::PropertyRead { object, key }
            if binding(object, "String")
                && matches!(key.as_ref(), SemanticValue::Literal(LiteralValue::String(key)) if key == "fromCharCode"))
}

/// Whether a node of `kind` is never a function: a primitive literal, or
/// what a test, `typeof` or arithmetic makes.
pub(super) fn never_a_function(kind: &TraceNodeKind) -> bool {
    matches!(
        kind,
        TraceNodeKind::Number(_)
            | TraceNodeKind::String(_)
            | TraceNodeKind::Boolean(_)
            | TraceNodeKind::Unary { .. }
            | TraceNodeKind::Binary { .. }
    )
}

/// Whether running `kind` may run other code: a closure that writes a
/// shared cell.
pub(super) fn statement_runs_code(arena: &TraceArena, kind: &TraceStatementKind) -> bool {
    match kind {
        TraceStatementKind::Let { value, .. } | TraceStatementKind::Assign { value, .. } => {
            !is_pure_value(arena, *value) && !is_message_thread(arena, *value)
        }
        TraceStatementKind::Evaluate { .. }
        | TraceStatementKind::PropertyWrite { .. }
        | TraceStatementKind::Return { .. }
        | TraceStatementKind::Throw { .. } => true,
        _ => false,
    }
}

/// `vmCreateThread(…)` as a VM Worker makes the function a message names
/// (see [`Executor::posted_to_worker`]): it makes a closure that shares no
/// cells, and runs nothing.
pub(super) fn is_message_thread(arena: &TraceArena, value: TraceNodeId) -> bool {
    let kind = |node: TraceNodeId| arena.nodes.get(node as usize).map(|node| &node.kind);
    matches!(kind(value), Some(TraceNodeKind::Call { callee, .. })
        if matches!(kind(*callee), Some(TraceNodeKind::Global { name }) if name == VM_CREATE_THREAD))
}

/// Whether a VM value read as `what` (see `Executor::concrete_index`) is
/// a VM position: a jump's target or state, or the entry or descriptor of a
/// VM function made, which compiled code takes from its bytecode.
pub(super) fn is_vm_position(what: &str) -> bool {
    matches!(
        what,
        "jump target"
            | "dispatch state"
            | "thread byte offset"
            | "thread dispatch state"
            | "thread descriptor"
    )
}

pub(super) fn computed(node: TraceNodeId) -> EvalValue {
    EvalValue {
        value: SymbolicValue::Computed { node },
        fact: None,
        node,
    }
}

pub(super) fn concrete_fact(value: &SymbolicValue) -> Option<SymbolicValue> {
    match value {
        SymbolicValue::Undefined
        | SymbolicValue::Null
        | SymbolicValue::Boolean(_)
        | SymbolicValue::Number(_)
        | SymbolicValue::String(_)
        | SymbolicValue::Object { .. } => Some(value.clone()),
        SymbolicValue::External { .. }
        | SymbolicValue::Computed { .. }
        | SymbolicValue::Unknown { .. } => None,
    }
}

/// The statements a block statement owns, in order.
pub(super) fn block_children(kind: &TraceStatementKind) -> Vec<TraceStatementId> {
    match kind {
        TraceStatementKind::If {
            taken,
            fall_through,
            ..
        } => taken.iter().chain(fall_through).copied().collect(),
        TraceStatementKind::Loop { body, .. } => body.clone(),
        TraceStatementKind::Try {
            body,
            handler,
            finalizer,
            ..
        } => body
            .iter()
            .chain(handler)
            .chain(finalizer.iter().flatten())
            .copied()
            .collect(),
        TraceStatementKind::Function { body, .. } => body.clone(),
        _ => Vec::new(),
    }
}

/// Whether computing `node` has no effect: a literal, a binding, or
/// arithmetic on number literals (the VM's index bookkeeping).
pub(super) fn is_pure_value(arena: &TraceArena, node: TraceNodeId) -> bool {
    let mut pending = vec![node];
    while let Some(node) = pending.pop() {
        match arena.nodes.get(node as usize).map(|node| &node.kind) {
            Some(
                TraceNodeKind::Undefined
                | TraceNodeKind::Null
                | TraceNodeKind::Boolean(_)
                | TraceNodeKind::Number(_)
                | TraceNodeKind::String(_)
                | TraceNodeKind::Identifier { .. },
            ) => {}
            Some(TraceNodeKind::Binary { left, right, .. }) => {
                for operand in [left, right] {
                    match arena.nodes.get(*operand as usize).map(|node| &node.kind) {
                        Some(TraceNodeKind::Number(_)) => {}
                        Some(TraceNodeKind::Binary { .. }) => pending.push(*operand),
                        _ => return false,
                    }
                }
            }
            _ => return false,
        }
    }
    true
}

/// The `let` bindings written in place at every read: those never
/// reassigned whose value is a literal, number arithmetic on such values, or
/// a copy of another binding that is never reassigned (which then reads as
/// that binding). Reading one is then exactly its value.
pub(super) fn inline_bindings(arena: &TraceArena) -> HashMap<&str, TraceNodeId> {
    let assigned = assigned_names(arena);
    // A binding is defined before it is read, so one pass in statement
    // order sees every binding a value copies already decided.
    let mut inline: HashMap<&str, TraceNodeId> = HashMap::default();
    // Bindings seen so far that are never reassigned.
    let mut fixed: HashSet<&str> = HashSet::default();
    let mut pending = Vec::new();
    for statement in &arena.statements {
        let TraceStatementKind::Let { name, value } = &statement.kind else {
            continue;
        };
        let name = name.as_str();
        if assigned.contains(name) {
            continue;
        }
        fixed.insert(name);
        let kind = |node: TraceNodeId| arena.nodes.get(node as usize).map(|node| &node.kind);
        // A copy of a fixed binding is that binding under another name.
        if let Some(TraceNodeKind::Identifier { name: source }) = kind(*value) {
            if fixed.contains(source.as_str()) && source != name {
                let target = inline.get(source.as_str()).copied().unwrap_or(*value);
                inline.insert(name, target);
                continue;
            }
        }
        pending.clear();
        pending.push(*value);
        let mut constant = true;
        while let Some(node) = pending.pop() {
            match kind(node) {
                Some(
                    TraceNodeKind::Undefined
                    | TraceNodeKind::Null
                    | TraceNodeKind::Boolean(_)
                    | TraceNodeKind::Number(_)
                    | TraceNodeKind::String(_)
                    | TraceNodeKind::Global { .. },
                ) => {}
                // A copy of a binding that is itself written in place.
                Some(TraceNodeKind::Identifier { name }) if inline.contains_key(name.as_str()) => {}
                Some(TraceNodeKind::Binary { left, right, .. }) => pending.extend([*left, *right]),
                _ => {
                    constant = false;
                    break;
                }
            }
        }
        if constant {
            // A copy points straight at what it copies, so emission never
            // walks a chain of copies.
            let target = match kind(*value) {
                Some(TraceNodeKind::Identifier { name }) => inline[name.as_str()],
                _ => *value,
            };
            inline.insert(name, target);
        }
    }
    inline
}

/// The names some statement reassigns.
pub(super) fn assigned_names(arena: &TraceArena) -> HashSet<&str> {
    arena
        .statements
        .iter()
        .filter_map(|statement| match &statement.kind {
            TraceStatementKind::Assign { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect()
}

/// The `let` statements nothing reads whose value has no effect to compute,
/// such as the VM's index bookkeeping: emission leaves them out. Removing one
/// can leave another unread, so this repeats until nothing changes. A name
/// that is ever reassigned is kept.
pub(super) fn dead_bindings(arena: &TraceArena, inline: &HashMap<&str, TraceNodeId>) -> Vec<bool> {
    let assigned = assigned_names(arena);
    let mut dead = vec![false; arena.statements.len()];
    // A binding is only read after it is defined, so walking the statements
    // backwards sees every read of a `let` before the `let` itself.
    let mut read: HashSet<&str> = HashSet::default();
    let mut reached = vec![false; arena.nodes.len()];
    let mut pending: Vec<TraceNodeId> = Vec::new();
    for (index, statement) in arena.statements.iter().enumerate().rev() {
        match &statement.kind {
            // An inlined binding is a constant, so it is pure as well.
            TraceStatementKind::Let { name, value }
                if !read.contains(name.as_str())
                    && !assigned.contains(name.as_str())
                    && (inline.contains_key(name.as_str()) || is_pure_value(arena, *value)) =>
            {
                dead[index] = true;
                continue;
            }
            TraceStatementKind::Let { value, .. }
            | TraceStatementKind::Assign { value, .. }
            | TraceStatementKind::Evaluate { value }
            | TraceStatementKind::Return { value }
            | TraceStatementKind::Throw { value }
            | TraceStatementKind::If { test: value, .. } => pending.push(*value),
            TraceStatementKind::PropertyWrite { object, key, value } => {
                pending.extend([*object, *key, *value]);
            }
            TraceStatementKind::Declare { .. }
            | TraceStatementKind::Loop { .. }
            | TraceStatementKind::Continue { .. }
            | TraceStatementKind::Break { .. }
            | TraceStatementKind::Function { .. }
            | TraceStatementKind::Try { .. } => {}
        }
        while let Some(node) = pending.pop() {
            let Some(seen) = reached.get_mut(node as usize) else {
                continue;
            };
            if std::mem::replace(seen, true) {
                continue;
            }
            let kind = &arena.nodes[node as usize].kind;
            if let TraceNodeKind::Identifier { name } = kind {
                // An inlined binding is not read; its value is.
                match inline.get(name.as_str()) {
                    Some(value) => pending.push(*value),
                    None => {
                        read.insert(name.as_str());
                    }
                }
            }
            pending.extend(node_children(kind));
        }
    }
    dead
}

/// The shared cells of one function, as program mode reads them.
#[derive(Debug, Default)]
pub(super) struct ProgramCells {
    /// The cells it shares with its creator, by variable (`$CELL_<slot>`).
    pub(super) captured: BTreeMap<String, u32>,
    /// The `$CELL_<slot> = captured<slot>` statements: the cell is the
    /// creator's variable, read through scope.
    pub(super) aliases: BTreeSet<TraceStatementId>,
}

/// The slot of a cell variable `$CELL_<slot>`.
pub(super) fn cell_slot(name: &str) -> Option<u32> {
    name.strip_prefix("$CELL_")?.parse().ok()
}

/// Program mode's facts of a function's cells, and its checks that one
/// variable per slot shares what the VM's boxes share: a cell is only ever
/// given its creator's box or a new one, a creation passes slot K as the
/// function's slot K, and no slot gets a new box after (or in a loop
/// together with) a creation that captures it, nor one it was given.
pub(super) fn program_cells(report: &TraceReport) -> Result<ProgramCells, EmitError> {
    let arena = &report.arena;
    let invalid = |what: String| EmitError::InvalidArena(format!("program mode: {what}"));
    let kind = |node: TraceNodeId| arena.nodes.get(node as usize).map(|node| &node.kind);
    let mut cells = ProgramCells::default();
    let mut fresh: BTreeSet<u32> = BTreeSet::new();
    for statement in &arena.statements {
        let TraceStatementKind::Assign { name, value } = &statement.kind else {
            continue;
        };
        let Some(slot) = cell_slot(name) else {
            continue;
        };
        match kind(*value) {
            Some(TraceNodeKind::External { name: input })
                if *input == format!("captured{slot}") =>
            {
                cells.captured.insert(name.clone(), slot);
                cells.aliases.insert(statement.id);
            }
            Some(TraceNodeKind::ObjectLiteral { keys, .. })
                if keys.len() == 1 && keys[0] == "value" =>
            {
                fresh.insert(slot);
            }
            // The result slot: `initializeSlot(resultSlot); writeSlot(resultSlot,
            // threadInvocation)`, a new cell holding the function, which the
            // trace keeps as the function itself.
            Some(TraceNodeKind::External { name: input }) if input == "callee" => {
                fresh.insert(slot);
            }
            _ => return Err(invalid(format!("{name} is given what is not a cell"))),
        }
    }
    if let Some(slot) = fresh
        .iter()
        .find(|slot| cells.captured.values().any(|captured| captured == *slot))
    {
        return Err(invalid(format!(
            "the function gives slot {slot}, which it shares with its creator, a new cell"
        )));
    }

    // In order, with the loops around each: every new cell and creation.
    let mut owned = vec![false; arena.statements.len()];
    for statement in &arena.statements {
        for child in block_children(&statement.kind) {
            owned[child as usize] = true;
        }
    }
    // One variable per slot for the whole function: a `let` in a block would
    // give each pass through it its own, where the VM keeps one cell.
    for statement in &arena.statements {
        if let TraceStatementKind::Declare { names } = &statement.kind {
            if owned[statement.id as usize] && names.iter().any(|name| cell_slot(name).is_some()) {
                return Err(invalid("a cell is declared inside a block".into()));
            }
        }
    }
    struct Event {
        order: usize,
        loops: Vec<TraceStatementId>,
        slots: Vec<u32>,
    }
    let (mut renewals, mut creations) = (Vec::new(), Vec::new());
    let mut pending: Vec<(TraceStatementId, Vec<TraceStatementId>)> = arena
        .statements
        .iter()
        .filter(|statement| !owned[statement.id as usize])
        .map(|statement| (statement.id, Vec::new()))
        .rev()
        .collect();
    let mut order = 0;
    while let Some((id, loops)) = pending.pop() {
        order += 1;
        let statement = arena
            .statements
            .get(id as usize)
            .ok_or_else(|| invalid(format!("missing statement {id}")))?;
        match &statement.kind {
            TraceStatementKind::Assign { name, value } => {
                // A new cell: a value's, or the function's in its result slot.
                let renewed = match kind(*value) {
                    Some(TraceNodeKind::ObjectLiteral { .. }) => true,
                    Some(TraceNodeKind::External { name: input }) => input == "callee",
                    _ => false,
                };
                if let (Some(slot), true) = (cell_slot(name), renewed) {
                    renewals.push(Event {
                        order,
                        loops: loops.clone(),
                        slots: vec![slot],
                    });
                }
            }
            TraceStatementKind::Let { value, .. } => {
                if let Some(TraceNodeKind::VmFunction { memory, .. }) = kind(*value) {
                    let mut slots = Vec::new();
                    if let Some(memory) = memory {
                        let Some(TraceNodeKind::ObjectLiteral { keys, values }) = kind(*memory)
                        else {
                            return Err(invalid("a creation's cells are not an object".into()));
                        };
                        for (key, value) in keys.iter().zip(values) {
                            let slot: u32 = key.parse().map_err(|_| {
                                invalid(format!("a creation's cell key {key:?} is not a slot"))
                            })?;
                            match kind(*value) {
                                Some(TraceNodeKind::Identifier { name }) if cell_slot(name) == Some(slot) => {
                                    slots.push(slot)
                                }
                                _ => {
                                    return Err(invalid(format!(
                                        "a creation passes something other than its slot {slot} as slot {slot}"
                                    )))
                                }
                            }
                        }
                    }
                    creations.push(Event {
                        order,
                        loops: loops.clone(),
                        slots,
                    });
                }
            }
            _ => {}
        }
        let inner = match &statement.kind {
            TraceStatementKind::Loop { .. } => {
                let mut inner = loops.clone();
                inner.push(id);
                inner
            }
            _ => loops.clone(),
        };
        for child in block_children(&statement.kind).into_iter().rev() {
            pending.push((child, inner.clone()));
        }
    }
    for creation in &creations {
        for renewal in &renewals {
            let slot = renewal.slots[0];
            if creation.slots.contains(&slot)
                && (renewal.order > creation.order
                    || renewal
                        .loops
                        .iter()
                        .any(|outer| creation.loops.contains(outer)))
            {
                return Err(invalid(format!(
                    "slot {slot} gets a new cell after a function it creates captured the old one"
                )));
            }
        }
    }
    Ok(cells)
}

/// A `Declare` that no merge needed.
pub(super) fn is_empty_declare(statement: &TraceStatement) -> bool {
    matches!(&statement.kind, TraceStatementKind::Declare { names } if names.is_empty())
}

/// Whether two stored values are the same: the same node, or the same
/// concrete value.
pub(super) fn same_cell(left: &StoredValue, right: &StoredValue) -> bool {
    if left.node.is_some() && left.node == right.node {
        return true;
    }
    matches!((&left.fact, &right.fact), (Some(left), Some(right)) if same_fact(left, right))
}

/// A stack cell's value; a hole reads as `undefined`.
pub(super) fn stack_cell_value(cell: &StackCell) -> StoredValue {
    match cell {
        StackCell::Value(value) => value.clone(),
        StackCell::Hole => stored_value(SymbolicValue::Undefined),
        StackCell::Param => unnamed_param(),
    }
}

/// A memory cell's value; an initialized slot holds `undefined`.
pub(super) fn memory_cell_value(cell: &MemoryCell) -> StoredValue {
    match cell {
        MemoryCell::Value(value) => value.clone(),
        MemoryCell::Initialized => stored_value(SymbolicValue::Undefined),
        MemoryCell::Param => unnamed_param(),
    }
}

/// A subroutine parameter read where it cannot be named: no value to trace
/// with. Every read inside a subroutine names it first, so this stops the
/// trace instead of guessing.
pub(super) fn unnamed_param() -> StoredValue {
    StoredValue {
        value: SymbolicValue::Unknown {
            reason: "a subroutine parameter read before it is named".into(),
        },
        fact: None,
        node: None,
    }
}

/// The same concrete value, comparing numbers by bits so `0` and `-0` (and
/// `NaN`) are told apart correctly.
/// Whether `name` is an input of the arguments a thread is called with:
/// `arg0`, `arg1`, … or its `arguments` object `args` (see
/// `cli::trace::commands::thread::unknown_inputs`).
fn is_argument_input(name: &str) -> bool {
    name == "args"
        || name
            .strip_prefix("arg")
            .is_some_and(|index| !index.is_empty() && index.bytes().all(|b| b.is_ascii_digit()))
}

/// A value a made object's property can be known as: a primitive.
fn is_primitive_fact(value: &SymbolicValue) -> bool {
    matches!(
        value,
        SymbolicValue::Undefined
            | SymbolicValue::Null
            | SymbolicValue::Boolean(_)
            | SymbolicValue::Number(_)
            | SymbolicValue::String(_)
    )
}

pub fn same_fact(left: &SymbolicValue, right: &SymbolicValue) -> bool {
    match (left, right) {
        (SymbolicValue::Number(left), SymbolicValue::Number(right)) => {
            left.to_bits() == right.to_bits()
        }
        _ => left == right,
    }
}

pub(super) fn stored_value(value: SymbolicValue) -> StoredValue {
    let fact = concrete_fact(&value);
    StoredValue {
        value,
        fact,
        node: None,
    }
}

pub(super) fn reject_external_computed(value: &SymbolicValue) -> Result<(), TraceContextError> {
    if matches!(value, SymbolicValue::Computed { .. }) {
        Err(TraceContextError {
            what: "computed symbolic value",
            limit: 0,
        })
    } else {
        Ok(())
    }
}

pub(super) fn preflight(handler: &HandlerTemplate) -> Option<String> {
    admit_recipe(handler, RecipeBounds::default())
        .err()
        .map(admission_reason)
}

pub(super) fn admission_reason(error: AdmissionError) -> String {
    match error {
        AdmissionError::TemplateStatus { status } => format!("handler status is {status}"),
        AdmissionError::UnsupportedFragment { reason, source } => format!("{reason}: {source}"),
        AdmissionError::StepLimit { steps, limit } => {
            format!("handler has {steps} steps, exceeding recipe work limit {limit}")
        }
        AdmissionError::StepOrder { step, .. } => {
            format!("step {step}: recipe steps are not strictly source ordered")
        }
        AdmissionError::UnknownOperation { step, reason }
        | AdmissionError::UnknownValue { step, reason } => format!("step {step}: {reason}"),
        AdmissionError::ContextValue { step } => {
            format!("step {step}: the context object is not a recipe value")
        }
        AdmissionError::TemporaryReadBeforeLet { step, id } => {
            format!("step {step}: temporary {id} is read before its let step")
        }
        AdmissionError::TemporaryAssignedBeforeLet { step, id } => {
            format!("step {step}: temporary {id} is assigned before its let step")
        }
        AdmissionError::DuplicateTemporary { step, id } => {
            format!("step {step}: temporary {id} is defined more than once")
        }
        AdmissionError::ExpressionWork { step, limit, .. } => {
            format!("step {step}: recipe expression work exceeds limit {limit}")
        }
        AdmissionError::ExpressionDepth { step, limit, .. } => {
            format!("step {step}: recipe expression depth exceeds configured limit {limit}")
        }
    }
}

/// Render a concrete number as a JavaScript *numeric literal* for emission.
///
/// This is deliberately not ECMA-262 `Number::toString`: emitted syntax only
/// has to parse back to the same binary64 value, and preserving the sign of
/// negative zero is required. `Number::toString` would render `-0` as `0` and
/// would use exponent notation at different boundaries, so the tracer keeps
/// this literal-oriented renderer. The shared
/// [`crate::js_number::number_to_js_string`] helper remains the
/// JavaScript string-conversion primitive.
/// The value of a [`number_text`] rendering.
pub fn parse_number_text(text: &str) -> Option<f64> {
    match text {
        "NaN" => Some(f64::NAN),
        "Infinity" => Some(f64::INFINITY),
        "-Infinity" => Some(f64::NEG_INFINITY),
        _ => text.parse().ok(),
    }
}

pub(super) fn number_text(value: f64) -> String {
    // A whole number below 2^53 prints as the integer it is: the integer
    // printer is much faster than the float one, with the same text.
    const EXACT: f64 = 9_007_199_254_740_992.0;
    if value == 0.0 && value.is_sign_negative() {
        "-0".into()
    } else if value.fract() == 0.0 && value.abs() < EXACT {
        (value as i64).to_string()
    } else if value.is_finite() {
        value.to_string()
    } else if value.is_nan() {
        "NaN".into()
    } else if value.is_sign_negative() {
        "-Infinity".into()
    } else {
        "Infinity".into()
    }
}

pub fn fold_binary(
    operator: PrimitiveOperator,
    left: Option<&SymbolicValue>,
    right: Option<&SymbolicValue>,
) -> Option<SymbolicValue> {
    let (Some(left), Some(right)) = (left, right) else {
        return None;
    };
    let (Some(a), Some(b)) = (primitive(left), primitive(right)) else {
        return None;
    };
    match operator {
        PrimitiveOperator::JsAdd => {
            if matches!(a, Primitive::Object) || matches!(b, Primitive::Object) {
                return None;
            }
            if matches!(a, Primitive::String(_)) || matches!(b, Primitive::String(_)) {
                // Rust's f64 formatting is not JavaScript ToString for all
                // finite values (notably very small/large exponents). Keep
                // the expression when a number would need that conversion.
                if matches!(a, Primitive::Number(_)) || matches!(b, Primitive::Number(_)) {
                    None
                } else {
                    Some(SymbolicValue::String(format!("{}{}", a.text(), b.text())))
                }
            } else {
                numeric_binary(a, b, |a, b| a + b)
            }
        }
        PrimitiveOperator::Subtract => numeric_binary(a, b, |a, b| a - b),
        PrimitiveOperator::Multiply => numeric_binary(a, b, |a, b| a * b),
        PrimitiveOperator::Divide => numeric_binary(a, b, |a, b| a / b),
        PrimitiveOperator::Remainder => numeric_binary(a, b, |a, b| a % b),
        PrimitiveOperator::ShiftLeft => {
            bitwise_binary(a, b, |a, b| a.wrapping_shl((b & 31) as u32))
        }
        PrimitiveOperator::ShiftRight => {
            bitwise_binary(a, b, |a, b| a.wrapping_shr((b & 31) as u32))
        }
        PrimitiveOperator::ShiftRightZeroFill => {
            bitwise_unsigned_binary(a, b, |a, b| a >> (b & 31))
        }
        PrimitiveOperator::BitwiseOr => bitwise_binary(a, b, |a, b| a | b),
        PrimitiveOperator::BitwiseXor => bitwise_binary(a, b, |a, b| a ^ b),
        PrimitiveOperator::BitwiseAnd => bitwise_binary(a, b, |a, b| a & b),
        PrimitiveOperator::StrictEqual => strict_equal(&a, &b).map(SymbolicValue::Boolean),
        PrimitiveOperator::StrictNotEqual => {
            strict_equal(&a, &b).map(|equal| SymbolicValue::Boolean(!equal))
        }
        // Relational comparisons fold only between two numbers: strings
        // compare by UTF-16 code units and other operands coerce first.
        PrimitiveOperator::LessThan => numeric_compare(&a, &b, |a, b| a < b),
        PrimitiveOperator::LessThanOrEqual => numeric_compare(&a, &b, |a, b| a <= b),
        PrimitiveOperator::GreaterThan => numeric_compare(&a, &b, |a, b| a > b),
        PrimitiveOperator::GreaterThanOrEqual => numeric_compare(&a, &b, |a, b| a >= b),
        _ => None,
    }
}

/// `===` between two primitives; `None` when either side is an object.
pub(super) fn strict_equal(a: &Primitive, b: &Primitive) -> Option<bool> {
    Some(match (a, b) {
        (Primitive::Object, _) | (_, Primitive::Object) => return None,
        // IEEE equality: NaN is unequal to itself and -0 equals +0, as in JS.
        (Primitive::Number(a), Primitive::Number(b)) => a == b,
        (Primitive::String(a), Primitive::String(b)) => a == b,
        (Primitive::Boolean(a), Primitive::Boolean(b)) => a == b,
        (Primitive::Null, Primitive::Null) | (Primitive::Undefined, Primitive::Undefined) => true,
        _ => false,
    })
}

pub(super) fn numeric_compare(
    a: &Primitive,
    b: &Primitive,
    compare: impl Fn(f64, f64) -> bool,
) -> Option<SymbolicValue> {
    match (a, b) {
        (Primitive::Number(a), Primitive::Number(b)) => {
            Some(SymbolicValue::Boolean(compare(*a, *b)))
        }
        _ => None,
    }
}

#[derive(Clone)]
pub(super) enum Primitive {
    Number(f64),
    String(String),
    Boolean(bool),
    Null,
    Undefined,
    Object,
}
impl Primitive {
    pub(super) fn text(&self) -> String {
        match self {
            Self::Number(v) => number_text(*v),
            Self::String(v) => v.clone(),
            Self::Boolean(v) => v.to_string(),
            Self::Null => "null".into(),
            Self::Undefined => "undefined".into(),
            Self::Object => "[object Object]".into(),
        }
    }
}
pub(super) fn primitive(value: &SymbolicValue) -> Option<Primitive> {
    match value {
        SymbolicValue::Number(v) => Some(Primitive::Number(*v)),
        SymbolicValue::String(v) => Some(Primitive::String(v.clone())),
        SymbolicValue::Boolean(v) => Some(Primitive::Boolean(*v)),
        SymbolicValue::Null => Some(Primitive::Null),
        SymbolicValue::Undefined => Some(Primitive::Undefined),
        SymbolicValue::Object { .. } => Some(Primitive::Object),
        _ => None,
    }
}
pub(super) fn to_number(value: Primitive) -> Option<f64> {
    match value {
        Primitive::Number(v) => Some(v),
        Primitive::Boolean(v) => Some(if v { 1.0 } else { 0.0 }),
        Primitive::Null => Some(0.0),
        Primitive::Undefined => Some(f64::NAN),
        Primitive::String(_) | Primitive::Object => None,
    }
}
pub(super) fn numeric_binary(
    a: Primitive,
    b: Primitive,
    op: impl FnOnce(f64, f64) -> f64,
) -> Option<SymbolicValue> {
    Some(SymbolicValue::Number(op(to_number(a)?, to_number(b)?)))
}
pub(super) fn bitwise_binary(
    a: Primitive,
    b: Primitive,
    op: impl FnOnce(i32, i32) -> i32,
) -> Option<SymbolicValue> {
    Some(SymbolicValue::Number(
        op(to_int32(to_number(a)?), to_int32(to_number(b)?)) as f64,
    ))
}
pub(super) fn bitwise_unsigned_binary(
    a: Primitive,
    b: Primitive,
    op: impl FnOnce(u32, u32) -> u32,
) -> Option<SymbolicValue> {
    Some(SymbolicValue::Number(
        op(to_uint32(to_number(a)?), to_uint32(to_number(b)?)) as f64,
    ))
}
pub fn fold_unary(
    operator: PrimitiveOperator,
    value: Option<&SymbolicValue>,
) -> Option<SymbolicValue> {
    let Some(value) = value else {
        return (operator == PrimitiveOperator::Void).then_some(SymbolicValue::Undefined);
    };
    let primitive = primitive(value)?;
    match operator {
        PrimitiveOperator::Void => Some(SymbolicValue::Undefined),
        PrimitiveOperator::UnaryPlus => Some(SymbolicValue::Number(to_number(primitive)?)),
        PrimitiveOperator::UnaryMinus => Some(SymbolicValue::Number(-to_number(primitive)?)),
        PrimitiveOperator::BitwiseNot => Some(SymbolicValue::Number(
            !to_int32(to_number(primitive)?) as f64,
        )),
        PrimitiveOperator::LogicalNot => Some(SymbolicValue::Boolean(match primitive {
            Primitive::Undefined | Primitive::Null => true,
            Primitive::Boolean(v) => !v,
            Primitive::Number(v) => v == 0.0 || v.is_nan(),
            Primitive::String(v) => v.is_empty(),
            Primitive::Object => false,
        })),
        // An object may be a function, so only primitives fold.
        PrimitiveOperator::Typeof => Some(SymbolicValue::String(
            match primitive {
                Primitive::Undefined => "undefined",
                Primitive::Null => "object",
                Primitive::Boolean(_) => "boolean",
                Primitive::Number(_) => "number",
                Primitive::String(_) => "string",
                Primitive::Object => return None,
            }
            .into(),
        )),
        _ => None,
    }
}

impl Executor {
    pub(super) fn new(context: TraceContext, limits: TraceLimits) -> Self {
        Self {
            preflights: HashMap::default(),
            cursor: None,
            control: ControlTransfer::default(),
            popped_frame: None,
            thread_control: false,
            branch: None,
            thread_return: None,
            block: None,
            loop_serial: 0,
            aliases: NodeMap::new(),
            concrete_reads: BTreeMap::new(),
            finally_reads: Vec::new(),
            write_serial: 0,
            join_arms: BTreeMap::new(),
            position_reads: BTreeMap::new(),
            call_returns: BTreeMap::new(),
            returned_arguments: BTreeMap::new(),
            substituted: BTreeSet::new(),
            outlined: BTreeMap::new(),
            joins: BTreeMap::new(),
            marked: BTreeSet::new(),
            reaches_marked: std::cell::RefCell::new(Vec::new()),
            carried_from: BTreeMap::new(),
            junk_origin: None,
            split: None,
            inline: None,
            dispatch: None,
            used_sets: Vec::new(),
            popped_try: None,
            pushed_finally: false,
            popped_error: None,
            try_entered: None,
            cells: BTreeSet::new(),
            captures: None,
            created_hook: None,
            returns_hook: None,
            cell_types_hook: None,
            created_now: Vec::new(),
            wanted_cells: BTreeSet::new(),
            consulted: BTreeSet::new(),
            data_consulted: Default::default(),
            read_slots: BTreeMap::new(),
            set_consulted: Default::default(),
            read_names: BTreeMap::new(),
            cell_access: false,
            overrides: Arc::default(),
            instruction_at: None,
            from_value_set: false,
            body_address: false,
            entry_address: false,
            pad_conflict: None,
            stack_reset: false,
            try_body_writes: BTreeMap::new(),
            try_body_cell_writes: BTreeMap::new(),
            cell_writes: BTreeMap::new(),
            cell_contents: BTreeMap::new(),
            cell_types: BTreeMap::new(),
            cell_holders: BTreeMap::new(),
            captured_cells: BTreeSet::new(),
            data_reads: BTreeSet::new(),
            open_sets: BTreeSet::new(),
            catch_params: BTreeSet::new(),
            finally_forks: BTreeMap::new(),
            finally_joins: 0,
            finally_exits: Vec::new(),
            definitions: Definitions::default(),
            cell_reads: BTreeMap::new(),
            instruction_reads: BTreeMap::new(),
            instruction_ran_code: false,
            cell_sets: BTreeMap::new(),
            constant_tables: BTreeMap::new(),
            number_tables: BTreeMap::new(),
            string_decoder: None,
            subroutine_escapes: BTreeMap::new(),
            try_depth_traps: false,
            message_cells: BTreeSet::new(),
            context,
            limits,
            arena: TraceArena::default(),
            diagnostics: Vec::new(),
            completion: TraceCompletion::Completed,
            steps: 0,
            handlers_traced: 0,
            external_inputs: Arc::default(),
            temps: BTreeSet::new(),
            temp_values: BTreeMap::new(),
            binding_serial: 0,
            current: None,
            handler_return_terminal: true,
            debug: DebugSink::disabled(),
        }
    }

    pub(super) fn debug_before_operation(
        &mut self,
        pc: Option<usize>,
        state: Option<usize>,
        opcode: Option<u8>,
        handler_key: u32,
    ) {
        if !self.debug.enabled() {
            return;
        }
        self.debug.before(
            pc,
            state,
            opcode,
            handler_key,
            self.arena.nodes.len(),
            self.arena.events.len(),
        );
    }

    pub(super) fn debug_after_operation(&mut self, effect: Option<&str>) {
        if !self.debug.enabled() {
            return;
        }
        let Some(pending) = self.debug.pending.as_ref() else {
            return;
        };
        let pc = pending.pc;
        let state = pending.state;
        let opcode = pending.opcode;
        let handler_key = pending.handler_key;
        let nodes_before = pending.nodes_before;
        let events_before = pending.events_before;
        let state_name = match (state, pc) {
            (Some(state), Some(pc)) => format!("{state}_{pc}"),
            _ => "unavailable".into(),
        };
        let mut text = format!("STATE:{state_name}\nOP:{handler_key}\n");
        if let Some(opcode) = opcode {
            text.push_str(&format!("OPCODE:{opcode}\n"));
        }
        text.push_str(&format!("STACK({}):\n", self.context.stack.len()));
        for (index, cell) in self.context.stack.iter().enumerate() {
            match cell {
                StackCell::Hole => text.push_str(&format!("{index}: undefined (hole)\n")),
                StackCell::Value(value) => {
                    text.push_str(&format!("{index}: {:?}\n", value.value));
                }
                StackCell::Param => text.push_str(&format!("{index}: parameter\n")),
            }
        }
        text.push_str(&format!("MEMORY({}):\n", self.context.memory.len()));
        for (slot, cell) in self.context.memory.iter() {
            match cell {
                MemoryCell::Initialized => text.push_str(&format!("{slot}: undefined\n")),
                MemoryCell::Value(value) => {
                    text.push_str(&format!("{slot}: {:?}\n", value.value));
                }
                MemoryCell::Param => text.push_str(&format!("{slot}: parameter\n")),
            }
        }
        text.push_str("NODES:\n");
        for node in self.arena.nodes.iter().skip(nodes_before) {
            text.push_str(&format!("{}: {:?}\n", node.id, node.kind));
        }
        text.push_str("EVENTS:\n");
        for event in self.arena.events.iter().skip(events_before) {
            text.push_str(&format!("{}: {:?}\n", event.id, event.kind));
        }
        if let Some(effect) = effect {
            text.push_str(&format!("EFFECT:{effect}\n"));
        }
        text.push('\n');
        self.debug.after(text);
    }

    pub(super) fn debug_stop(&mut self, code: TraceStopCode, message: &str) {
        self.debug.stop(code, message);
    }

    pub(super) fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            nodes: self.arena.nodes.len(),
            statements: self.arena.statements.len(),
            events: self.arena.events.len(),
            context: self.context.clone(),
            diagnostics: self.diagnostics.len(),
            external_inputs: self.external_inputs.clone(),
            binding_serial: self.binding_serial,
            completion: self.completion.clone(),
            block: self.block,
            block_length: self.block_statements().map_or(0, Vec::len),
            loop_serial: self.loop_serial,
            finally_joins: self.finally_joins,
        }
    }

    pub(super) fn rollback(&mut self, checkpoint: Checkpoint) {
        self.arena.nodes.truncate(checkpoint.nodes);
        self.finally_reads
            .retain(|(read, ..)| (*read as usize) < checkpoint.nodes);
        self.aliases.truncate(checkpoint.nodes as TraceNodeId);
        self.concrete_reads
            .split_off(&(checkpoint.nodes as TraceNodeId));
        self.join_arms.split_off(&(checkpoint.nodes as TraceNodeId));
        self.position_reads
            .split_off(&(checkpoint.nodes as TraceNodeId));
        self.call_returns
            .split_off(&(checkpoint.nodes as TraceNodeId));
        self.returned_arguments
            .split_off(&(checkpoint.nodes as TraceNodeId));
        self.joins.split_off(&(checkpoint.nodes as TraceNodeId));
        self.reaches_marked.get_mut().truncate(checkpoint.nodes);
        self.carried_from
            .split_off(&(checkpoint.nodes as TraceNodeId));
        self.data_reads
            .split_off(&(checkpoint.nodes as TraceNodeId));
        // A node the walk numbers again is another value: which cell it
        // read goes with it. What was asked of the cells stays.
        self.read_slots
            .split_off(&(checkpoint.nodes as TraceNodeId));
        let serial = checkpoint.binding_serial;
        self.open_sets
            .retain(|name| generated_binding_serial(name).is_some_and(|n| n < serial));
        self.catch_params
            .retain(|name| generated_binding_serial(name).is_some_and(|n| n < serial));
        self.cell_reads
            .retain(|name, _| generated_binding_serial(name).is_some_and(|n| n < serial));
        // A name the walk numbers again is another value: which cell it read
        // goes with it. What was asked of the cells stays.
        self.read_names
            .retain(|name, _| generated_binding_serial(name).is_none_or(|n| n < serial));
        self.outlined
            .retain(|name, _| generated_binding_serial(name).is_some_and(|n| n < serial));
        self.arena.statements.truncate(checkpoint.statements);
        // A `finally` whose paths joined after the checkpoint: its `try`,
        // if still there, is a `try`/`catch` again.
        self.finally_joins = checkpoint.finally_joins;
        while let Some(&(serial, block, length)) = self.finally_exits.last() {
            if serial < checkpoint.finally_joins {
                break;
            }
            self.finally_exits.pop();
            if let Some(arm) = self.arm_mut(block) {
                arm.truncate(length);
            }
        }
        let undone: Vec<TraceStatementId> = self
            .finally_forks
            .iter()
            .filter(|(_, (_, joined))| *joined >= checkpoint.finally_joins)
            .map(|(statement, _)| *statement)
            .collect();
        for statement in undone {
            self.finally_forks.remove(&statement);
            if let Some(TraceStatementKind::Try {
                catches, finalizer, ..
            }) = self
                .arena
                .statements
                .get_mut(statement as usize)
                .map(|statement| &mut statement.kind)
            {
                *catches = true;
                *finalizer = None;
            }
        }
        let statements = checkpoint.statements as TraceStatementId;
        self.definitions.truncate(statements);
        self.arena.events.truncate(checkpoint.events);
        self.context = checkpoint.context;
        self.diagnostics.truncate(checkpoint.diagnostics);
        self.external_inputs = checkpoint.external_inputs;
        self.binding_serial = checkpoint.binding_serial;
        self.completion = checkpoint.completion;
        self.temps.clear();
        self.temp_values.clear();
        self.control = ControlTransfer::default();
        self.popped_frame = None;
        self.branch = None;
        self.thread_return = None;
        self.popped_try = None;
        self.pushed_finally = false;
        self.popped_error = None;
        self.try_entered = None;
        self.block = checkpoint.block;
        self.loop_serial = checkpoint.loop_serial;
        if let Some(arm) = self.block_statements_mut() {
            arm.truncate(checkpoint.block_length);
        }
    }

    /// Pop the pending exception.
    pub(super) fn pop_thrown(&mut self, p: &TraceProvenance) -> Result<EvalValue, ExecutionStop> {
        let Some(stored) = self.context.thrown.pop() else {
            return Err(self.unsupported_stop(
                TraceStopCode::EmptyExceptionStack,
                "no pending exception to pop".into(),
                p.clone(),
            ));
        };
        let node = match stored.node {
            Some(node) => node,
            None => self.value_node(stored.value.clone(), p)?.node,
        };
        Ok(EvalValue {
            value: stored.value,
            fact: stored.fact,
            node,
        })
    }

    /// A stack or memory write inside a `try` body: remember the cell when
    /// the write changes it, and when the `catch` reads it as a variable,
    /// assign the variable too. The body itself goes on with the value.
    pub(super) fn note_try_write(
        &mut self,
        cell: TraceCell,
        value: EvalValue,
        p: &TraceProvenance,
    ) -> EvalValue {
        if self.context.try_frames.is_empty() {
            return value;
        }
        self.note_body_write(cell);
        let written = StoredValue {
            value: value.value.clone(),
            fact: value.fact.clone(),
            node: Some(value.node),
        };
        let unchanged = self
            .context
            .try_entry
            .as_ref()
            .and_then(|entry| entry.cell(cell))
            .is_some_and(|before| same_cell(&before, &written));
        if !unchanged {
            self.context.note_try_write(cell);
        }
        // A body that popped its `try` frames but has not closed their
        // blocks yet writes in the body of the innermost `try` still active,
        // and of each popped one. A VM position it saves there (where a
        // `finally` goes on, and in which state) is what the handler it
        // reaches jumps by: their variables take it too. Data written there
        // keeps to the innermost block's variable.
        let frames = self.context.try_frames.len();
        let open = self.context.try_outer.len();
        let mut names: Vec<String> = Vec::new();
        let position = matches!(value.fact, Some(SymbolicValue::Number(number))
            if number >= 0.0 && number.fract() == 0.0);
        if self.context.subroutine.is_none() && frames < open && position {
            for scope in &self.context.try_outer[frames.max(1)..] {
                if let Some(name) = scope.variables.get(&cell) {
                    names.push(name.clone());
                }
            }
        }
        names.extend(self.context.try_variables.get(&cell).cloned());
        for name in names {
            self.statement(
                TraceStatementKind::Assign {
                    name,
                    value: value.node,
                },
                p.clone(),
            );
        }
        value
    }

    /// Record the shared cell `slot` as written in the body of every `try`
    /// active now.
    pub(super) fn note_body_cell_write(&mut self, slot: u32) {
        for depth in 1..=self.context.try_frames.len() {
            self.try_body_cell_writes
                .entry(depth)
                .or_default()
                .insert(slot);
        }
    }

    /// Record `cell` as written in the body of every `try` active now.
    pub(super) fn note_body_write(&mut self, cell: TraceCell) {
        for depth in 1..=self.context.try_frames.len() {
            self.try_body_writes.entry(depth).or_default().insert(cell);
        }
    }

    /// Provenance for statements a driver adds between handlers.
    pub(super) fn driver_provenance(&self) -> TraceProvenance {
        TraceProvenance {
            handler_index: 0,
            table_key: 0,
            semantic_identity: "vm-control".into(),
            step_order: 0,
            trace_step: self.steps as u64,
            span: SpanRecord { start: 0, end: 0 },
        }
    }

    /// The node of a stored value, building it for a concrete value.
    pub(super) fn materialize(
        &mut self,
        stored: StoredValue,
        provenance: &TraceProvenance,
    ) -> Result<TraceNodeId, TraceJoinError> {
        match stored.node {
            Some(node) => Ok(node),
            None => self
                .value_node(stored.value, provenance)
                .map(|value| value.node)
                .map_err(|_| TraceJoinError("a stored value cannot be built".into())),
        }
    }

    /// The statement list of the current block.
    pub(super) fn block_statements(&self) -> Option<&Vec<TraceStatementId>> {
        self.arm(self.block)
    }

    /// The statement list of the block `target`.
    pub(super) fn arm(&self, target: Option<BlockTarget>) -> Option<&Vec<TraceStatementId>> {
        let target = target?;
        match (
            &self.arena.statements.get(target.statement as usize)?.kind,
            target.arm,
        ) {
            (TraceStatementKind::If { taken, .. }, BlockArm::Taken) => Some(taken),
            (TraceStatementKind::If { fall_through, .. }, BlockArm::FallThrough) => {
                Some(fall_through)
            }
            (TraceStatementKind::Loop { body, .. }, BlockArm::LoopBody)
            | (TraceStatementKind::Try { body, .. }, BlockArm::TryBody)
            | (TraceStatementKind::Function { body, .. }, BlockArm::FunctionBody) => Some(body),
            (TraceStatementKind::Try { handler, .. }, BlockArm::Catch) => Some(handler),
            (TraceStatementKind::Try { finalizer, .. }, BlockArm::Finally) => finalizer.as_ref(),
            _ => None,
        }
    }

    pub(super) fn block_statements_mut(&mut self) -> Option<&mut Vec<TraceStatementId>> {
        self.arm_mut(self.block)
    }

    pub(super) fn arm_mut(
        &mut self,
        target: Option<BlockTarget>,
    ) -> Option<&mut Vec<TraceStatementId>> {
        let target = target?;
        match (
            &mut self
                .arena
                .statements
                .get_mut(target.statement as usize)?
                .kind,
            target.arm,
        ) {
            (TraceStatementKind::If { taken, .. }, BlockArm::Taken) => Some(taken),
            (TraceStatementKind::If { fall_through, .. }, BlockArm::FallThrough) => {
                Some(fall_through)
            }
            (TraceStatementKind::Loop { body, .. }, BlockArm::LoopBody)
            | (TraceStatementKind::Try { body, .. }, BlockArm::TryBody)
            | (TraceStatementKind::Function { body, .. }, BlockArm::FunctionBody) => Some(body),
            (TraceStatementKind::Try { handler, .. }, BlockArm::Catch) => Some(handler),
            (TraceStatementKind::Try { finalizer, .. }, BlockArm::Finally) => finalizer.as_mut(),
            _ => None,
        }
    }

    pub(super) fn provenance(
        &self,
        handler: &HandlerTemplate,
        step: Option<&RecipeStep>,
    ) -> TraceProvenance {
        TraceProvenance {
            handler_index: handler.handler_index,
            table_key: handler.table_key,
            semantic_identity: handler.semantic_identity.clone(),
            step_order: step.map_or(0, |s| s.order),
            trace_step: self.steps as u64,
            span: step.map_or_else(|| handler.source_span.clone(), |s| s.span.clone()),
        }
    }

    pub(super) fn unsupported(
        &mut self,
        code: TraceStopCode,
        message: String,
        provenance: Option<TraceProvenance>,
    ) {
        self.completion = TraceCompletion::PartialUnsupported;
        self.debug_stop(code, &message);
        self.diagnostics.push(TraceDiagnostic {
            code,
            message,
            provenance,
        });
    }

    pub(super) fn budget(
        &mut self,
        code: TraceStopCode,
        message: &str,
        provenance: Option<TraceProvenance>,
    ) {
        self.completion = TraceCompletion::BudgetStopped;
        self.debug_stop(code, message);
        self.diagnostics.push(TraceDiagnostic {
            code,
            message: message.into(),
            provenance,
        });
    }

    pub(super) fn context_computed_value(&self) -> Option<String> {
        self.context
            .stack
            .iter()
            .filter_map(|cell| match cell {
                StackCell::Value(value) => Some(&value.value),
                StackCell::Hole | StackCell::Param => None,
            })
            .chain(self.context.memory.values().filter_map(|cell| match cell {
                MemoryCell::Value(value) => Some(&value.value),
                MemoryCell::Initialized | MemoryCell::Param => None,
            }))
            .find_map(|value| match value {
                SymbolicValue::Computed { node } => Some(format!(
                    "seeded context contains Computed node {node}; computed values must come from this trace arena"
                )),
                _ => None,
            })
    }

    pub(super) fn run_handler(
        &mut self,
        handler: &HandlerTemplate,
        instruction: &DecodedInstruction,
    ) -> Result<HandlerOutcome, ExecutionStop> {
        self.temps.clear();
        self.temp_values.clear();
        self.instruction_reads.clear();
        self.instruction_ran_code = false;
        self.control = ControlTransfer::default();
        self.popped_frame = None;
        self.branch = None;
        self.thread_return = None;
        self.popped_try = None;
        self.pushed_finally = false;
        self.popped_error = None;
        self.try_entered = None;
        self.created_now.clear();
        self.stack_reset = false;
        if let Some(join) = self
            .context
            .stack_pad_join
            .filter(|_| self.context.stack_pad > 0)
        {
            match stack_access(handler) {
                StackAccess::Relative => {}
                StackAccess::Reset => self.stack_reset = true,
                StackAccess::Absolute => {
                    let p = self.provenance(handler, None);
                    return Err(self.padded_stop(join, None, &p));
                }
            }
        }
        let mut terminal = None;
        for step in &handler.steps {
            self.steps += 1;
            if self.steps > self.limits.max_steps {
                return Err(ExecutionStop::Budget(
                    "step budget exhausted".into(),
                    Some(self.provenance(handler, Some(step))),
                ));
            }
            self.current = Some(self.provenance(handler, Some(step)));
            let outcome = match self.run_step(handler, step, instruction) {
                Ok(outcome) => outcome,
                // The handler throws where it stands: what ran before stays.
                Err(ExecutionStop::Throws(value)) => {
                    let p = self.provenance(handler, Some(step));
                    self.statement(TraceStatementKind::Throw { value: value.node }, p);
                    Some((true, value))
                }
                Err(stop) => return Err(stop),
            };
            if outcome.is_some() {
                terminal = outcome;
                break;
            }
        }
        // `vmRun` returns the completion register only after the handler.
        if let Some(value) = &self.thread_return {
            let p = self.provenance(handler, None);
            self.statement(TraceStatementKind::Return { value: value.node }, p);
        }
        self.current = None;
        Ok(terminal.map_or(HandlerOutcome::Continue, |(throw, value)| {
            if throw {
                HandlerOutcome::Throw(TraceValue {
                    value: value.value,
                    node: value.node,
                })
            } else {
                HandlerOutcome::Return(TraceValue {
                    value: value.value,
                    node: value.node,
                })
            }
        }))
    }

    pub(super) fn run_step(
        &mut self,
        handler: &HandlerTemplate,
        step: &RecipeStep,
        instruction: &DecodedInstruction,
    ) -> Result<Option<(bool, EvalValue)>, ExecutionStop> {
        let p = self.provenance(handler, Some(step));
        match &step.operation {
            RecipeOperation::Let { temporary, value } => {
                let evaluated = self.eval(value, instruction, &p)?;
                let (_, bound) = self.bind_value(evaluated, &p)?;
                self.temps.insert(*temporary);
                self.temp_values.insert(*temporary, bound);
            }
            RecipeOperation::Assign { temporary, value } => {
                if !self.temps.contains(temporary) {
                    return Err(self.unsupported_stop(
                        TraceStopCode::MissingTemporary,
                        format!("temporary {temporary} was assigned before initialization"),
                        p,
                    ));
                }
                let evaluated = self.eval(value, instruction, &p)?;
                let (_, bound) = self.bind_value(evaluated, &p)?;
                self.temps.insert(*temporary);
                self.temp_values.insert(*temporary, bound);
            }
            RecipeOperation::StackWrite { index, value } => {
                let index = self.eval_index(index, instruction, &p)?;
                if let Some(join) = self.context.padded_below(index.value) {
                    return Err(self.padded_stop(join, Some(index.value), &p));
                }
                let evaluated = self.eval(value, instruction, &p)?;
                let evaluated = self.bind_value(evaluated, &p)?.1;
                let evaluated = self.note_try_write(TraceCell::Stack(index.value), evaluated, &p);
                self.ensure_stack_capacity(index.value)?;
                self.context.stack[index.value] = StackCell::Value(StoredValue {
                    value: evaluated.value.clone(),
                    fact: evaluated.fact.clone(),
                    node: Some(evaluated.node),
                });
                self.event(
                    TraceEventKind::StackWrite {
                        index: index.node,
                        value: evaluated.node,
                    },
                    p,
                );
            }
            RecipeOperation::StackLengthSet { value } => {
                let evaluated = self.eval(value, instruction, &p)?;
                let length = self.concrete_index(&evaluated, "stack length", &p)?;
                if length > self.limits.max_stack_length {
                    return Err(ExecutionStop::Budget(
                        format!(
                            "stack length {length} exceeds limit {}",
                            self.limits.max_stack_length
                        ),
                        Some(p),
                    ));
                }
                let evaluated = self.bind_value(evaluated, &p)?.1;
                // Cut to a fixed length no longer than the part both sides of
                // a padded join share: the sides hold the same stack again.
                if self.stack_reset && length <= self.context.stack_pad_start {
                    self.context.stack_pad = 0;
                    self.context.stack_pad_join = None;
                } else if let Some(join) = self.context.padded_below(length) {
                    // A pop into the holes uses them up; a cut to a fixed
                    // length counts from the bottom.
                    let reached = (!self.stack_reset).then_some(length);
                    return Err(self.padded_stop(join, reached, &p));
                }
                self.context.stack.resize(length, StackCell::Hole);
                self.event(
                    TraceEventKind::StackLengthSet {
                        value: evaluated.node,
                    },
                    p,
                );
            }
            RecipeOperation::MemoryWrite { slot, value } => {
                let slot = self.eval_index(slot, instruction, &p)?;
                let slot_number = self.memory_slot(&slot, &p)?;
                let evaluated = self.eval(value, instruction, &p)?;
                let evaluated = self.bind_value(evaluated, &p)?.1;
                if !self.context.memory.contains_key(&slot_number) {
                    // `this.slots[slot].value = value` with no such cell.
                    return Err(self.missing_slot(true, &p));
                }
                if self.cells.contains(&slot_number) {
                    self.write_cell(slot_number, &evaluated, &p)?;
                } else {
                    let evaluated =
                        self.note_try_write(TraceCell::Memory(slot_number), evaluated, &p);
                    self.context.memory_mut().insert(
                        slot_number,
                        MemoryCell::Value(StoredValue {
                            value: evaluated.value.clone(),
                            fact: evaluated.fact.clone(),
                            node: Some(evaluated.node),
                        }),
                    );
                    self.event(
                        TraceEventKind::MemoryWrite {
                            slot: slot.node,
                            value: evaluated.node,
                        },
                        p,
                    );
                }
            }
            RecipeOperation::MemoryInitialize { slot } => {
                let slot = self.eval_index(slot, instruction, &p)?;
                let slot_number = self.memory_slot(&slot, &p)?;
                if !self.context.memory.contains_key(&slot_number)
                    && self.context.memory.len() >= self.limits.max_memory_slots
                {
                    return Err(ExecutionStop::Budget(
                        "memory slot budget exhausted".into(),
                        Some(p),
                    ));
                }
                let cell = TraceCell::Memory(slot_number);
                if self.cells.contains(&slot_number) {
                    // A new cell: closures made before keep the old one.
                    let undefined = self.add_node(TraceNodeKind::Undefined, p.clone())?;
                    let fresh = self.new_cell(undefined, &p)?;
                    self.assign_cell(slot_number, fresh, Some(SymbolicValue::Undefined), &p);
                    self.note_cell_content(
                        slot_number,
                        Some(CellContent {
                            addresses: BTreeSet::new(),
                            data: true,
                        }),
                    );
                    self.note_cell_types(slot_number, predicate::fact_types(&SymbolicValue::Undefined));
                    Arc::make_mut(&mut self.context.escaped_cells).remove(&slot_number);
                    self.context
                        .memory_mut()
                        .insert(slot_number, MemoryCell::Initialized);
                    return Ok(None);
                }
                if self.context.try_variables.contains_key(&cell)
                    && !self.context.try_frames.is_empty()
                {
                    // The catch reads this slot as a variable: assign it.
                    let undefined = self.value_node(SymbolicValue::Undefined, &p)?;
                    let value = self.note_try_write(cell, undefined, &p);
                    self.context.memory_mut().insert(
                        slot_number,
                        MemoryCell::Value(StoredValue {
                            value: value.value,
                            fact: None,
                            node: Some(value.node),
                        }),
                    );
                } else {
                    if !self.context.try_frames.is_empty() {
                        self.context.note_try_write(cell);
                        self.note_body_write(cell);
                    }
                    self.context
                        .memory_mut()
                        .insert(slot_number, MemoryCell::Initialized);
                }
                self.event(TraceEventKind::MemoryInitialize { slot: slot.node }, p);
            }
            RecipeOperation::PropertyWrite { object, key, value } => {
                let object = self.eval(object, instruction, &p)?;
                let key = self.eval(key, instruction, &p)?;
                let value = self.eval(value, instruction, &p)?;
                let handle = self.object_handle(object.node);
                self.note_property(handle, key.fact.as_ref(), value.fact.clone());
                self.statement(
                    TraceStatementKind::PropertyWrite {
                        object: object.node,
                        key: key.node,
                        value: value.node,
                    },
                    p,
                );
            }
            // VM control flow: concrete VM values, followed by the driver.
            RecipeOperation::PushCallFrame => {
                let Some(cursor) = self.cursor else {
                    return Err(self.unsupported_stop(
                        TraceStopCode::MissingVmCursor,
                        "call frame push needs the VM cursor from a dispatch driver".into(),
                        p,
                    ));
                };
                if self.context.call_frames.len() >= self.limits.max_stack_length {
                    return Err(ExecutionStop::Budget(
                        format!(
                            "call frame depth exceeds limit {}",
                            self.limits.max_stack_length
                        ),
                        Some(p),
                    ));
                }
                self.context.call_frames.push(CallFrame {
                    return_byte_offset: cursor.byte_offset,
                    return_dispatch_state: cursor.dispatch_state,
                });
            }
            RecipeOperation::PopCallFrame => {
                let Some(frame) = self.context.call_frames.pop() else {
                    return Err(self.unsupported_stop(
                        TraceStopCode::EmptyCallFrameStack,
                        "return with an empty VM call frame stack".into(),
                        p,
                    ));
                };
                self.popped_frame = Some(frame);
            }
            RecipeOperation::SetProgramCounter { value } => {
                let evaluated = self.eval(value, instruction, &p)?;
                let target = self.concrete_index(&evaluated, "jump target", &p)?;
                self.control.byte_offset = Some(target);
            }
            RecipeOperation::SetDispatchState { value } => {
                let evaluated = self.eval(value, instruction, &p)?;
                let state = self.concrete_index(&evaluated, "dispatch state", &p)?;
                self.control.dispatch_state = Some(state);
            }
            RecipeOperation::PushTryFrame {
                byte_offset,
                dispatch_state,
            } => {
                let target = self.eval(byte_offset, instruction, &p)?;
                let target = self.concrete_index(&target, "try handler", &p)?;
                let state = self.eval(dispatch_state, instruction, &p)?;
                let state = self.concrete_index(&state, "try handler state", &p)?;
                if !self.thread_control {
                    return Err(self.unsupported_stop(
                        TraceStopCode::UnhandledTry,
                        "a try needs a driver that walks the thread".into(),
                        p,
                    ));
                }
                if self.context.try_underflow > 0 {
                    return Err(self.unsupported_stop(
                        TraceStopCode::TryUnderflow,
                        "a try entered after more try exits than entries".into(),
                        p,
                    ));
                }
                let frame = TryFrame {
                    handler_byte_offset: target,
                    handler_dispatch_state: state,
                };
                self.context.try_frames.push(frame);
                self.try_entered = Some(frame);
            }
            RecipeOperation::PopTryFrame => match self.context.try_frames.pop() {
                Some(frame) => self.popped_try = Some(frame),
                None if self.try_depth_traps => self.context.try_underflow += 1,
                None => {}
            },
            RecipeOperation::PushFinallyRecord => {
                let Some(cursor) = self.cursor else {
                    return Err(self.unsupported_stop(
                        TraceStopCode::MissingVmCursor,
                        "entering a finally needs the VM cursor from a dispatch driver".into(),
                        p,
                    ));
                };
                self.context.error_records.push(ErrorRecord {
                    handled: false,
                    saved: Some(CallFrame {
                        return_byte_offset: cursor.byte_offset,
                        return_dispatch_state: cursor.dispatch_state,
                    }),
                    finally: None,
                    thrown_in: None,
                });
                self.pushed_finally = true;
            }
            RecipeOperation::PopErrorRecord => {
                let Some(record) = self.context.error_records.pop() else {
                    return Err(self.unsupported_stop(
                        TraceStopCode::EmptyExceptionStack,
                        "popping an error record with none active".into(),
                        p,
                    ));
                };
                self.popped_error = Some(record);
                if let Some(statement) = record.finally {
                    self.close_finally(statement, &p)?;
                }
            }
            RecipeOperation::RethrowIfHandled
            | RecipeOperation::DropThrownIfHandled
            | RecipeOperation::DropThrownUnderIfHandled => {
                let Some(record) = self.popped_error else {
                    return Err(self.unsupported_stop(
                        TraceStopCode::EmptyExceptionStack,
                        "error record read before a pop".into(),
                        p,
                    ));
                };
                if record.handled {
                    // `TJ(1)`: `l = top; length -= 1; stack[length - 1] = l`.
                    if matches!(step.operation, RecipeOperation::DropThrownUnderIfHandled)
                        && self.context.thrown.len() >= 2
                    {
                        let under = self.context.thrown.len() - 2;
                        self.context.thrown.remove(under);
                        return Ok(None);
                    }
                    let thrown = self.pop_thrown(&p)?;
                    if matches!(step.operation, RecipeOperation::RethrowIfHandled) {
                        self.statement(TraceStatementKind::Throw { value: thrown.node }, p);
                        return Ok(Some((true, thrown)));
                    }
                }
            }
            RecipeOperation::CompleteThread { value } => {
                let value = self.eval(value, instruction, &p)?;
                if !self.thread_control {
                    return Err(self.unsupported_stop(
                        TraceStopCode::UnhandledThreadReturn,
                        "VM function return needs a driver that walks the thread".into(),
                        p,
                    ));
                }
                self.thread_return = Some(TraceValue {
                    value: value.value,
                    node: value.node,
                });
            }
            RecipeOperation::JumpIf {
                condition,
                byte_offset,
                dispatch_state,
            } => {
                let condition = self.eval(condition, instruction, &p)?;
                let target = self.eval(byte_offset, instruction, &p)?;
                let target = self.concrete_index(&target, "jump target", &p)?;
                let state = self.eval(dispatch_state, instruction, &p)?;
                let state = self.concrete_index(&state, "dispatch state", &p)?;
                let known = match fold_unary(PrimitiveOperator::LogicalNot, condition.fact.as_ref())
                {
                    Some(SymbolicValue::Boolean(falsy)) => Some(!falsy),
                    _ => {
                        // A fake branch: a program-value test whose outcome
                        // is guaranteed by its types or its contents. An
                        // earlier branch on this path that tested the same
                        // value decides nothing: that is the program testing
                        // its own value again, a real if.
                        let at = self.instruction_at;
                        let forced = at.and_then(|at| self.overrides.fake.get(&at).copied());
                        let real = at.is_some_and(|at| self.overrides.real.contains(&at));
                        let (decided, by) = if let Some(taken) = forced {
                            (Some(taken), BranchEvidence::Override)
                        } else if real {
                            (None, BranchEvidence::Test)
                        } else {
                            (
                                {
                                    let (decided, typed) = predicate::decide_typed(
                                        &self.arena,
                                        &self.aliases,
                                        &self.joins,
                                        &self.call_returns,
                                        &self.context.type_facts,
                                        None,
                                        condition.node,
                                    );
                                    self.used_sets.extend(typed);
                                    decided
                                }
                                // An opaque predicate's shape: decided on
                                // what the path concretely put in the cells
                                // it reads.
                                .or_else(|| {
                                    let decided = self.opaque_decision(condition.node, None);
                                    if decided.is_some() || self.finally_reads.is_empty() {
                                        return decided;
                                    }
                                    // In a `finally` traced once: decided
                                    // only where what either entry put in
                                    // the cells it reads decides it alike.
                                    let body = self.opaque_decision(condition.node, Some(false));
                                    let thrown = self.opaque_decision(condition.node, Some(true));
                                    body.filter(|_| body == thrown)
                                }),
                                BranchEvidence::Test,
                            )
                        };
                        if let Some(taken) = decided {
                            self.event(
                                TraceEventKind::BranchDecided {
                                    test: condition.node,
                                    taken,
                                    by,
                                },
                                p.clone(),
                            );
                            self.branch = Some(PendingBranch {
                                test: condition.node,
                                byte_offset: target,
                                dispatch_state: state,
                                decided,
                                by,
                            });
                        }
                        decided
                    }
                };
                match known {
                    // A known condition is a VM decision: follow it.
                    Some(truthy) => {
                        if truthy {
                            self.control.byte_offset = Some(target);
                            self.control.dispatch_state = Some(state);
                        }
                    }
                    _ if !self.thread_control => {
                        return Err(self.unsupported_stop(
                            TraceStopCode::DynamicBranch,
                            "conditional jump on a program value needs a driver that traces both branches".into(),
                            p,
                        ));
                    }
                    _ if self.branch.is_some() => {
                        return Err(self.unsupported_stop(
                            TraceStopCode::DynamicBranch,
                            "a handler with two program-value conditional jumps".into(),
                            p,
                        ));
                    }
                    _ => {
                        self.branch = Some(PendingBranch {
                            test: condition.node,
                            byte_offset: target,
                            dispatch_state: state,
                            decided: None,
                            by: BranchEvidence::Test,
                        });
                    }
                }
            }
            RecipeOperation::Evaluate { value } => {
                let evaluated = self.eval(value, instruction, &p)?;
                self.statement(
                    TraceStatementKind::Evaluate {
                        value: evaluated.node,
                    },
                    p,
                );
            }
            RecipeOperation::Return { value } => {
                let evaluated = self.eval(value, instruction, &p)?;
                if self.handler_return_terminal {
                    self.statement(
                        TraceStatementKind::Return {
                            value: evaluated.node,
                        },
                        p,
                    );
                } else {
                    // Dispatch continuation: the returned expression is still
                    // evaluated and retained, but it must not terminate the
                    // emitted JavaScript early.
                    self.statement(
                        TraceStatementKind::Evaluate {
                            value: evaluated.node,
                        },
                        p,
                    );
                }
                return Ok(Some((false, evaluated)));
            }
            RecipeOperation::Throw { value } => {
                if self.context.try_underflow > 0 {
                    return Err(self.unsupported_stop(
                        TraceStopCode::UnhandledTry,
                        "a throw after more try exits than entries".into(),
                        p,
                    ));
                }
                let evaluated = self.eval(value, instruction, &p)?;
                self.statement(
                    TraceStatementKind::Throw {
                        value: evaluated.node,
                    },
                    p,
                );
                return Ok(Some((true, evaluated)));
            }
            RecipeOperation::Unknown { reason, .. } => {
                return Err(self.unsupported_stop(
                    TraceStopCode::UnknownOperation,
                    reason.clone(),
                    p,
                ))
            }
        }
        Ok(None)
    }
}

/// How a handler places what it does on the stack (see `stack_access`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum StackAccess {
    /// Every stack index and `stack.length = …` computed from
    /// `stack.length` (directly or through a temporary), and `stack.length`
    /// used for nothing else.
    Relative,
    /// Only `stack.length = …` to a value that is no stack length: the
    /// stack cut to a fixed length, as a statement ends.
    Reset,
    /// Anything else counts stack positions from the bottom.
    Absolute,
}

/// How `handler` uses the stack (see [`StackAccess`]): what decides whether
/// it does on a stack padded by a join (see `TraceSession::pad_stack`) what
/// it does on each path the stack stands for.
pub(super) fn stack_access(handler: &HandlerTemplate) -> StackAccess {
    if stack_relative(handler) {
        return StackAccess::Relative;
    }
    // Whether `value` reads the stack at all.
    fn stack(value: &SemanticValue) -> bool {
        matches!(
            value,
            SemanticValue::StackLength
                | SemanticValue::StackRead { .. }
                | SemanticValue::StackSlice { .. }
        ) || value.children().into_iter().any(stack)
    }
    let reset = handler.steps.iter().all(|step| match &step.operation {
        RecipeOperation::StackWrite { .. } => false,
        operation => operation.values().into_iter().all(|value| !stack(value)),
    }) && handler
        .steps
        .iter()
        .any(|step| matches!(step.operation, RecipeOperation::StackLengthSet { .. }));
    if reset {
        StackAccess::Reset
    } else {
        StackAccess::Absolute
    }
}

/// Whether `handler` counts every stack position from the top: a stack
/// index or `stack.length = …` computed from `stack.length` (directly or
/// through a temporary), and `stack.length` used for nothing else. Such a
/// handler does on a stack padded from the bottom (see
/// `TraceSession::pad_stack`) what it does on each path the stack stands for.
fn stack_relative(handler: &HandlerTemplate) -> bool {
    // Whether `value` computes with the stack length, outside a stack index.
    fn length(value: &SemanticValue, relative: &BTreeSet<u32>) -> bool {
        match value {
            SemanticValue::StackLength => true,
            SemanticValue::Temporary { id } => relative.contains(id),
            SemanticValue::StackRead { .. } | SemanticValue::StackSlice { .. } => false,
            _ => value
                .children()
                .into_iter()
                .any(|child| length(child, relative)),
        }
    }
    // Whether every stack index inside `value` counts from the top.
    fn indexes(value: &SemanticValue, relative: &BTreeSet<u32>) -> bool {
        match value {
            SemanticValue::StackRead { index } | SemanticValue::StackSlice { start: index } => {
                length(index, relative) && indexes(index, relative)
            }
            _ => value
                .children()
                .into_iter()
                .all(|child| indexes(child, relative)),
        }
    }
    let mut relative = BTreeSet::new();
    handler.steps.iter().all(|step| match &step.operation {
        RecipeOperation::Let { temporary, value }
        | RecipeOperation::Assign { temporary, value } => {
            if length(value, &relative) {
                relative.insert(*temporary);
            } else {
                relative.remove(temporary);
            }
            indexes(value, &relative)
        }
        RecipeOperation::StackWrite { index, value } => {
            length(index, &relative)
                && indexes(index, &relative)
                && indexes(value, &relative)
                && !length(value, &relative)
        }
        RecipeOperation::StackLengthSet { value } => {
            length(value, &relative) && indexes(value, &relative)
        }
        operation => operation
            .values()
            .into_iter()
            .all(|value| indexes(value, &relative) && !length(value, &relative)),
    })
}
