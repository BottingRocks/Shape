use super::*;

/// How a join lines up two stacks that differ only in length (see
/// [`TraceSession::stack_padding`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackPadding {
    /// The taken side is the shorter.
    pub taken_shorter: bool,
    /// Where the holes go in the shorter stack, and how many.
    pub start: usize,
    pub count: usize,
    /// The earlier join whose holes these are, when the other side kept
    /// them.
    pub earlier: Option<usize>,
    /// The shorter side popped values the other keeps: a step reaching
    /// under its own stack shows it junk.
    pub deficit: bool,
    /// The holes go under a stack cell an enclosing loop names, which only
    /// a join that is no deficit may do (see [`TraceSession::stack_padding`]).
    pub loop_variable: bool,
}

impl TraceSession {
    /// Whether two paths stand in the same VM state, so `join` can merge
    /// them: the same call, try and error stacks, pending exceptions, stack
    /// length and memory slots.
    #[must_use]
    pub fn joinable(a: &TracePath, b: &TracePath) -> bool {
        Self::join_conflict(a, b).is_none()
    }

    /// Why two paths stand in VM states `join` cannot merge, if they do.
    #[must_use]
    pub fn join_conflict(a: &TracePath, b: &TracePath) -> Option<String> {
        Self::conflict_besides_stack(a, b).or_else(|| {
            let (a, b) = (&a.context, &b.context);
            (a.stack.len() != b.stack.len()).then(|| {
                format!(
                    "branches reach the join with stack lengths {} and {}",
                    a.stack.len(),
                    b.stack.len()
                )
            })
        })
    }

    /// Whether paths `a` and `b` differ only in how many values their
    /// stacks hold above a part both share, and may be lined up: the
    /// shorter (true: `a`), where the part both share ends, and by how many
    /// values. Not inside a VM subroutine traced as a function, nor below
    /// holes an earlier join put in, nor where a loop or a parameter names a stack
    /// cell above that part, which padding would move, or a `try` body
    /// changed one its catch gets: one under the stack it was entered with.
    ///
    /// A side that still has an earlier join's holes, where the other cut
    /// its stack to the part they share since (see `StackAccess::Reset`),
    /// differs from it by just those holes: the other gets the same, for
    /// that earlier join (otherwise `None`). `entry` is the checkpoint taken
    /// as the fork opened: its stack is the one both sides started from.
    #[must_use]
    pub fn stack_padding(
        a: &TracePath,
        b: &TracePath,
        entry: &TraceCheckpoint,
    ) -> Option<StackPadding> {
        if Self::conflict_besides_stack(a, b).is_some() {
            return None;
        }
        let (a, b) = (&a.context, &b.context);
        if a.stack.len() == b.stack.len() {
            return None;
        }
        // Holes an earlier join put in: both sides have them, the same.
        let padded = (a.stack_pad, a.stack_pad_start, a.stack_pad_join);
        if padded != (b.stack_pad, b.stack_pad_start, b.stack_pad_join) {
            // Whichever side kept them: which is the taken one decides
            // nothing.
            let (holes, other) = if a.stack_pad > 0 && b.stack_pad == 0 {
                (a, b)
            } else if b.stack_pad > 0 && a.stack_pad == 0 {
                (b, a)
            } else {
                return None;
            };
            let start = holes.stack_pad_start;
            let fits = holes.stack.len() == other.stack.len() + holes.stack_pad
                && other.stack.len() >= start
                && holes.stack[..start]
                    .iter()
                    .zip(&other.stack[..start])
                    .all(|(left, right)| {
                        same_cell(&stack_cell_value(left), &stack_cell_value(right))
                    });
            return fits.then_some(StackPadding {
                taken_shorter: std::ptr::eq(other, a),
                start,
                count: holes.stack_pad,
                earlier: holes.stack_pad_join,
                deficit: holes.stack_pad_deficit,
                loop_variable: false,
            });
        }
        let (short, long) = if a.stack.len() < b.stack.len() {
            (a, b)
        } else {
            (b, a)
        };
        let deficit = Self::deficit(short, long, entry);
        // Each side's top is its own result (`v || x` keeps `v` on one side
        // and pops to `x` on the other; one arm may leave a value under its
        // own): the holes go where the part both still share ends, under
        // the shorter side's top.
        let shared = short
            .stack
            .iter()
            .zip(&long.stack)
            .take_while(|(left, right)| {
                same_cell(&stack_cell_value(left), &stack_cell_value(right))
            })
            .count();
        let start = shared.min(short.stack.len().saturating_sub(1));
        if a.stack_pad > 0 && start < a.stack_pad_start + a.stack_pad {
            return None;
        }
        let moved = |context: &TraceContext| {
            // A catch starts from the stack its `try` was entered with.
            let entered = context
                .try_entry
                .iter()
                .chain(
                    context
                        .try_outer
                        .iter()
                        .filter_map(|scope| scope.entry.as_ref()),
                )
                .map(|entry| entry.stack.len())
                .max()
                .unwrap_or(0);
            // A loop variable under the holes moves on the shorter side:
            // the loop's own `continue` and `break` undo the join (see
            // `loop_before_padding`), and a step reaching the holes does.
            // Not for a deficit, whose evidence needs the holes of its own
            // join alone (see `TraceSession::take_pad_conflict`).
            context.subroutine.is_some()
                || context
                    .loop_variables
                    .values()
                    .filter(|_| deficit)
                    .chain(context.placeholders.keys())
                    .any(|cell| matches!(cell, TraceCell::Stack(index) if *index >= start))
                || context
                    .try_variables
                    .keys()
                    .chain(context.try_writes.iter())
                    .any(|cell| {
                        matches!(cell, TraceCell::Stack(index) if (start..entered).contains(index))
                    })
        };
        if moved(a) || moved(b) {
            return None;
        }
        let loop_variable = [a, b].iter().any(|context| {
            context
                .loop_variables
                .values()
                .any(|cell| matches!(cell, TraceCell::Stack(index) if *index >= start))
        });
        Some(StackPadding {
            taken_shorter: a.stack.len() < b.stack.len(),
            start,
            count: long.stack.len() - short.stack.len(),
            earlier: None,
            deficit,
            loop_variable,
        })
    }

    /// Whether, of two stacks the fork's sides met with, the longer side
    /// pushed nothing of its own and the shorter one cut the stack the fork
    /// opened with (`entry`'s) lower: the shorter side popped values the
    /// other keeps, and the code that follows reading under the shorter
    /// side's own stack is evidence (see `TraceSession::take_pad_conflict`).
    fn deficit(short: &TraceContext, long: &TraceContext, entry: &TraceCheckpoint) -> bool {
        let kept = |context: &TraceContext| {
            context
                .stack
                .iter()
                .zip(&entry.0.context.stack)
                .take_while(|(left, right)| {
                    same_cell(&stack_cell_value(left), &stack_cell_value(right))
                })
                .count()
        };
        kept(short) < kept(long) && kept(long) == long.stack.len()
    }

    /// Put `count` holes into `path`'s stack at `start`, where the part it
    /// shares with a stack `count` values longer ends, for the join tagged
    /// `join`: what each side pushed above that part then lines up from the
    /// top, so every push, pop and read counted from the top reads what the
    /// VM reads on each path. A step that reaches the holes or what lies
    /// under them undoes the join (see `TraceStopCode::PaddedStack`).
    /// Above holes an earlier join put in, a step must not reach either:
    /// the region runs from the earlier holes to these.
    /// `deficit`: the shorter side popped values the other keeps (see
    /// [`Self::stack_padding`]), so a step reaching under its own stack is
    /// evidence.
    pub fn pad_stack(path: &mut TracePath, start: usize, count: usize, join: usize, deficit: bool) {
        let context = &mut path.context;
        context
            .stack
            .splice(start..start, std::iter::repeat_n(StackCell::Hole, count));
        if context.stack_pad == 0 {
            context.stack_pad_start = start;
            context.stack_pad_deficit = deficit;
        } else {
            context.stack_pad_deficit = false;
        }
        context.stack_pad = start + count - context.stack_pad_start;
        context.stack_pad_join = Some(join);
        context.stack_pad_joins += 1;
    }

    /// Whether `trace_loop` was entered before the join that padded the
    /// running path's stack (see [`Self::pad_stack`]): its variables are
    /// stack cells counted as they were before the holes went in.
    #[must_use]
    pub fn loop_before_padding(&self, trace_loop: &TraceLoop) -> bool {
        let context = &self.executor.context;
        context.stack_pad > 0 && trace_loop.entry.stack_pad_join != context.stack_pad_join
    }

    /// The join that lined the running path's stack up from the top, while
    /// its holes remain (see [`Self::pad_stack`]).
    #[must_use]
    pub fn stack_pad_join(&self) -> Option<usize> {
        self.executor.context.stack_pad_join
    }

    /// The join a step found lined up wrongly, and whether the step,
    /// counting from the top, reached below the shorter side's own stack:
    /// the code after the join used more values than that side holds, which
    /// code that runs never does. Otherwise the step read a value the
    /// shorter side has (as `v || x` does with `x` pushed first), counted a
    /// stack position from the bottom, or a join failed, on a path it
    /// padded.
    pub fn take_pad_conflict(&mut self) -> Option<(usize, bool)> {
        self.executor.pad_conflict.take()
    }

    fn conflict_besides_stack(a: &TracePath, b: &TracePath) -> Option<String> {
        let (a, b) = (&a.context, &b.context);
        if a.call_frames != b.call_frames {
            return Some("branches reach the join with different VM call stacks".into());
        }
        if a.receiver != b.receiver {
            return Some("branches reach the join with different receivers".into());
        }
        if a.try_frames != b.try_frames
            || a.try_underflow != b.try_underflow
            || a.error_records != b.error_records
        {
            return Some(
                "branches reach the join in different try, catch or finally states".into(),
            );
        }
        if a.thrown.len() != b.thrown.len()
            || a.thrown
                .iter()
                .zip(&b.thrown)
                .any(|(left, right)| !same_cell(left, right))
        {
            return Some("branches reach the join with different pending exceptions".into());
        }
        if !a.memory.keys().eq(b.memory.keys()) {
            return Some("branches reach the join with different memory slots".into());
        }
        None
    }

    /// Every stack and memory cell of the running path.
    #[must_use]
    pub fn cells(&self) -> BTreeSet<TraceCell> {
        let context = &self.executor.context;
        (0..context.stack.len())
            .map(TraceCell::Stack)
            .chain(context.memory.keys().map(|slot| TraceCell::Memory(*slot)))
            .collect()
    }

    /// The number of statements traced so far.
    #[must_use]
    pub fn statement_count(&self) -> usize {
        self.executor.arena.statements.len()
    }

    /// Whether every statement traced after the first `count` only binds a
    /// value that has no effect to compute: a literal, another binding, or
    /// arithmetic on number literals (the VM's index bookkeeping).
    #[must_use]
    pub fn only_pure_bindings_since(&self, count: usize) -> bool {
        let arena = &self.executor.arena;
        arena.statements.iter().skip(count).all(|statement| {
            matches!(statement.kind, TraceStatementKind::Let { value, .. } if is_pure_value(arena, value))
        })
    }

    /// The number of values on `path`'s VM stack.
    #[must_use]
    pub fn stack_length_of(path: &TracePath) -> usize {
        path.context.stack.len()
    }

    /// The stack lengths at the header of `trace_loop` and now, when the
    /// running path came back to it with a different one.
    #[must_use]
    pub fn loop_stack_change(&self, trace_loop: &TraceLoop) -> Option<(usize, usize)> {
        let (entry, current) = (
            trace_loop.entry.stack.len(),
            self.executor.context.stack.len(),
        );
        (entry != current).then_some((entry, current))
    }

    /// Every cell written while the `try` at `depth` was active since the
    /// last call, on any path (one that ended too), cleared on read.
    pub fn take_try_body_writes(&mut self, depth: usize) -> BTreeSet<TraceCell> {
        self.executor
            .try_body_writes
            .remove(&depth)
            .unwrap_or_default()
    }

    /// Every shared cell written while the `try` at `depth` was active
    /// since the last call, on any path, cleared on read.
    pub fn take_try_body_cell_writes(&mut self, depth: usize) -> BTreeSet<u32> {
        self.executor
            .try_body_cell_writes
            .remove(&depth)
            .unwrap_or_default()
    }

    /// The shared cells the running path knows anything of (see
    /// [`TraceContext::known_cells`]).
    #[must_use]
    pub fn concrete_cells(&self) -> BTreeSet<u32> {
        self.executor.context.known_cells()
    }

    /// The cells `path` wrote inside its `try` body.
    #[must_use]
    pub fn try_writes(path: &TracePath) -> &BTreeSet<TraceCell> {
        &path.context.try_writes
    }

    /// Make `path` the running path; the previously running one is stored in
    /// `path` in exchange.
    /// The number of events traced so far.
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.executor.arena.events.len()
    }

    /// How many statements `path`'s block holds.
    #[must_use]
    pub fn block_length(&self, path: &TracePath) -> usize {
        self.executor.arm(path.block).map_or(0, Vec::len)
    }

    /// Put `path`, one side of a fork that is not running, back to `to`, a
    /// copy of it taken when its block held `block_length` statements. The
    /// `statements` and `events` it made since (the other side's, made in
    /// between, are not among them) stay in the arena, unreachable and
    /// without effect: a statement becomes an empty declaration, an event
    /// [`TraceEventKind::Undone`].
    pub fn cut_back(
        &mut self,
        path: &mut TracePath,
        to: &TracePath,
        block_length: usize,
        statements: &[std::ops::Range<usize>],
        events: &[std::ops::Range<usize>],
    ) {
        *path = to.clone();
        if let Some(arm) = self.executor.arm_mut(path.block) {
            arm.truncate(block_length);
        }
        let arena = &mut self.executor.arena;
        // A VM subroutine traced as a function in that time is no part of
        // the side: the other side may call it too. It stays, with its body.
        let mut kept = BTreeSet::new();
        for id in statements.iter().cloned().flatten() {
            if matches!(
                arena.statements[id].kind,
                TraceStatementKind::Function { .. }
            ) {
                let mut pending = vec![id as TraceStatementId];
                while let Some(statement) = pending.pop() {
                    if kept.insert(statement) {
                        pending.extend(block_children(&arena.statements[statement as usize].kind));
                    }
                }
            }
        }
        for id in statements.iter().cloned().flatten() {
            if !kept.contains(&(id as TraceStatementId)) {
                arena.statements[id].kind = TraceStatementKind::Declare { names: Vec::new() };
            }
        }
        for id in events.iter().cloned().flatten() {
            arena.events[id].kind = TraceEventKind::Undone;
        }
    }

    /// A copy of the running path, for a driver to run by
    /// [`Self::switch_path`] as it runs a fork's.
    #[must_use]
    pub fn current_path(&self) -> TracePath {
        TracePath {
            context: self.executor.context.clone(),
            block: self.executor.block,
        }
    }

    pub fn switch_path(&mut self, path: &mut TracePath) {
        std::mem::swap(&mut self.executor.context, &mut path.context);
        std::mem::swap(&mut self.executor.block, &mut path.block);
    }

    /// Close `fork` where one of its paths ended (returned, threw or continued
    /// a loop): `running` carries on after the statement, which is the same
    /// as running on in its own arm since the ended one never falls through.
    /// Cells the ended path wrote in a `try` body stay written.
    pub fn continue_alone(
        &mut self,
        fork: &TraceFork,
        running: &mut TracePath,
        ended: &TracePath,
    ) -> Result<(), TraceJoinError> {
        // A `finally` that ended inside the arm already took the path out.
        if !running.context.finally_left.contains(&fork.statement) {
            self.executor.hoist(
                &mut running.context,
                running.block,
                fork.declare,
                fork.first_node,
            )?;
            running.block = fork.parent;
        }
        let writes = &ended.context.try_writes;
        if !writes.is_subset(&running.context.try_writes) {
            Arc::make_mut(&mut running.context.try_writes).extend(writes.iter().copied());
        }
        Ok(())
    }

    /// The merge a VM value of the last handler needed concrete, cleared on
    /// read: the walk traces that join's paths apart instead.
    pub fn take_split(&mut self) -> Option<MergedConcrete> {
        self.executor.split.take()
    }

    /// The concrete value of every stack and memory cell of the running path
    /// (`None` where it is not concrete), and the stack length.
    #[must_use]
    pub fn cell_facts(&self) -> (usize, BTreeMap<TraceCell, Option<SymbolicValue>>) {
        let context = &self.executor.context;
        let facts = (0..context.stack.len())
            .map(TraceCell::Stack)
            .chain(context.memory.keys().map(|slot| TraceCell::Memory(*slot)))
            .map(|cell| (cell, context.cell(cell).and_then(|stored| stored.fact)))
            .collect();
        (context.stack.len(), facts)
    }

    /// Whether the running path came through a `catch` and has not merged
    /// with one that did not since: below the `try`'s stack its stack is
    /// unknown.
    #[must_use]
    pub fn from_catch(&self) -> bool {
        self.executor.context.from_catch
    }

    /// The `try` statement whose catch the running path came through, while
    /// [`Self::from_catch`].
    #[must_use]
    pub fn catch_try(&self) -> Option<TraceStatementId> {
        self.executor
            .context
            .catch_try
            .filter(|_| self.executor.context.from_catch)
    }

    /// What a handler's stop changes in the session, to undo with
    /// [`Self::restore_stop_marks`] when the driver recovers from it.
    #[must_use]
    pub fn stop_marks(&self) -> StopMarks {
        StopMarks {
            diagnostics: self.executor.diagnostics.len(),
            completion: self.executor.completion.clone(),
        }
    }

    /// Undo the stop recorded since `marks`.
    pub fn restore_stop_marks(&mut self, marks: StopMarks) {
        self.executor.diagnostics.truncate(marks.diagnostics);
        self.executor.completion = marks.completion;
    }

    /// Run on as before the stop recorded since `marks`, keeping its
    /// diagnostic.
    pub fn resume(&mut self, marks: &StopMarks) {
        self.executor.completion = marks.completion.clone();
    }

    /// The value set a VM jump target or state of the last handler needed
    /// one value of, cleared on read: the driver branches on each value.
    pub fn take_dispatch(&mut self) -> Option<ValueDispatch> {
        self.executor.dispatch.take()
    }

    /// The variables whose value sets VM values were taken from since the
    /// last call: a value set that grows later was incomplete for them.
    pub fn take_used_sets(&mut self) -> Vec<String> {
        std::mem::take(&mut self.executor.used_sets)
    }

    /// Whether this attempt made a closure holding one of the shared cells
    /// `slots` that no closed-cell test (a `holders <slot>` in
    /// [`Self::take_used_sets`]) counted yet: noted from now on, so a loop
    /// whose body made it is traced again with it counted.
    pub fn cell_holders_grew(&mut self, slots: &BTreeSet<u32>) -> bool {
        self.executor.cell_holders_grew(slots)
    }

    /// From now on `path` knows the variable `name` is `value`.
    pub fn assume(path: &mut TracePath, name: &str, cell: Option<CellRead>, value: SymbolicValue) {
        learn(&mut path.context, name, cell, value, true);
    }

    /// From now on `path` knows the variable `name` is not `value`.
    pub fn exclude(path: &mut TracePath, name: &str, cell: Option<CellRead>, value: SymbolicValue) {
        learn(&mut path.context, name, cell, value, false);
    }

    /// [`Self::assume`] (`is`) or [`Self::exclude`] on the running path.
    pub fn learn(&mut self, name: &str, cell: Option<CellRead>, value: SymbolicValue, is: bool) {
        learn(&mut self.executor.context, name, cell, value, is);
    }

    /// `name === value`, or `$CELL_<slot>["value"] === value` for a shared
    /// cell's read, a test node for [`Self::fork`].
    pub fn equality_test(
        &mut self,
        name: &str,
        cell: Option<CellRead>,
        value: SymbolicValue,
    ) -> Result<TraceNodeId, TraceJoinError> {
        let executor = &mut self.executor;
        let provenance = executor.driver_provenance();
        let build = |_: ExecutionStop| TraceJoinError("the test cannot be built".into());
        let left = match cell {
            Some(read) => {
                let object = executor
                    .identifier(&Executor::cell_variable(read.slot), &provenance)
                    .map_err(build)?;
                let key = executor
                    .add_node(TraceNodeKind::String("value".into()), provenance.clone())
                    .map_err(build)?;
                executor
                    .add_node(
                        TraceNodeKind::PropertyRead { object, key },
                        provenance.clone(),
                    )
                    .map_err(build)?
            }
            None => executor.identifier(name, &provenance).map_err(build)?,
        };
        let right = executor.value_node(value, &provenance).map_err(build)?.node;
        executor
            .add_node(
                TraceNodeKind::Binary {
                    operator: PrimitiveOperator::StrictEqual,
                    left,
                    right,
                },
                provenance,
            )
            .map_err(build)
    }

    /// The values each loop variable of `trace_loop` holds on the running
    /// path at a `continue`: `None` for one that is not a known set.
    #[must_use]
    pub fn loop_values(
        &self,
        trace_loop: &TraceLoop,
    ) -> Vec<(TraceCell, Option<Vec<SymbolicValue>>)> {
        let executor = &self.executor;
        trace_loop
            .variables
            .iter()
            .map(|(&cell, name)| {
                let values = executor
                    .context
                    .cell(cell)
                    .and_then(|stored| executor.candidates(&stored, Some(name)));
                (cell, values)
            })
            .collect()
    }

    /// The types of what each loop variable of `trace_loop` holds at a
    /// `continue`, by cell; one that holds itself adds none.
    #[must_use]
    pub fn loop_types(&self, trace_loop: &TraceLoop) -> Vec<(TraceCell, u16)> {
        let executor = &self.executor;
        trace_loop
            .variables
            .iter()
            .map(|(&cell, name)| {
                let types = executor
                    .context
                    .cell(cell)
                    .and_then(|stored| stored.node)
                    .map_or(u16::MAX, |node| {
                        let mut resolved = node;
                        while let Some(&next) = executor.aliases.get(&resolved) {
                            resolved = next;
                        }
                        match &executor.arena.nodes[resolved as usize].kind {
                            TraceNodeKind::Identifier { name: held } if held == name => 0,
                            _ => predicate::value_types(
                                &executor.arena,
                                &executor.aliases,
                                &executor.joins,
                                &executor.call_returns,
                                &executor.context.type_facts,
                                node,
                            ),
                        }
                    });
                (cell, types)
            })
            .collect()
    }

    /// The outlined call a loop variable of `trace_loop` holds the result
    /// of at a `continue`, when that leaves it no known values: traced
    /// inline, the variable might have a value set to decide tests on.
    #[must_use]
    pub fn loop_outlined_call(&self, trace_loop: &TraceLoop) -> Option<usize> {
        let executor = &self.executor;
        trace_loop.variables.iter().find_map(|(&cell, name)| {
            // The value it entered the loop with, or the one it carries now.
            let started = trace_loop
                .starts
                .get(&cell)
                .and_then(|&node| executor.outlined_in(node));
            started.or_else(|| {
                let stored = executor.context.cell(cell)?;
                if executor.candidates(&stored, Some(name)).is_some() {
                    return None;
                }
                executor.outlined_in(stored.node?)
            })
        })
    }

    /// The block a VM jump's program value was first carried out of, when
    /// the last handler stopped on a jump straight to one, cleared on read:
    /// the fork whose arm that is led to code the VM never runs.
    /// Returns the `if` statement and whether that arm is its taken one.
    pub fn take_junk_origin(&mut self) -> Option<(TraceStatementId, bool)> {
        let block = self.executor.junk_origin.take()?;
        match block.arm {
            BlockArm::Taken => Some((block.statement, true)),
            BlockArm::FallThrough => Some((block.statement, false)),
            _ => None,
        }
    }

    /// The outlined call a VM value of the last handler needed concrete,
    /// cleared on read: the walk traces that call inline instead.
    pub fn take_inline(&mut self) -> Option<usize> {
        self.executor.inline.take()
    }

    /// The outlined call that `node` is computed from: traced inline, a
    /// test on it might have a known outcome.
    #[must_use]
    pub fn outlined_call_in(&self, node: TraceNodeId) -> Option<usize> {
        self.executor.outlined_in(node)
    }

    /// [`Self::outlined_call_in`], leaving out calls whose returns a test
    /// is decided on without tracing them inline.
    #[must_use]
    pub fn unresolved_call_in(&self, node: TraceNodeId) -> Option<usize> {
        let executor = &self.executor;
        let outlined = &executor.outlined;
        if outlined.is_empty() {
            return None;
        }
        executor.depends_on(node, |name| {
            outlined
                .get(name)
                .copied()
                .filter(|tag| !executor.substituted.contains(tag))
        })
    }

    /// Whether `a` and `b` hold different values in any of `cells`: a join
    /// there would merge them.
    #[must_use]
    pub fn cells_differ(a: &TracePath, b: &TracePath, cells: &BTreeSet<TraceCell>) -> bool {
        cells
            .iter()
            .any(|&cell| match (a.context.cell(cell), b.context.cell(cell)) {
                (Some(left), Some(right)) => !same_cell(&left, &right),
                (left, right) => left.is_some() != right.is_some(),
            })
    }

    /// [`Self::join`], tagging each variable it makes from two different
    /// concrete values with `tag`.
    pub fn join_tagged(
        &mut self,
        fork: TraceFork,
        taken: TracePath,
        fall_through: TracePath,
        tag: usize,
    ) -> Result<TracePath, TraceJoinError> {
        self.join_with(fork, taken, fall_through, Some(tag))
    }

    /// Close `fork` where both branches reached the same VM position, and
    /// return the merged path in the enclosing block. VM state must agree; a
    /// stack or memory cell holding different program values becomes a
    /// variable declared before the `if` and assigned at the end of each arm.
    pub fn join(
        &mut self,
        fork: TraceFork,
        taken: TracePath,
        fall_through: TracePath,
    ) -> Result<TracePath, TraceJoinError> {
        self.join_with(fork, taken, fall_through, None)
    }

    pub(super) fn join_with(
        &mut self,
        fork: TraceFork,
        taken: TracePath,
        fall_through: TracePath,
        tag: Option<usize>,
    ) -> Result<TracePath, TraceJoinError> {
        if let Some(conflict) = Self::join_conflict(&taken, &fall_through) {
            // A side whose stack a join padded: that join lined it up wrongly.
            self.executor.pad_conflict = taken
                .context
                .stack_pad_join
                .or(fall_through.context.stack_pad_join)
                .map(|join| (join, false));
            return Err(TraceJoinError(conflict));
        }
        let (a, b) = (&taken.context, &fall_through.context);
        let provenance = self.executor.arena.statements[fork.statement as usize]
            .provenance
            .clone();
        let mut merged = a.clone();
        merged.left_try = None;
        if b.stack_pad > merged.stack_pad {
            merged.stack_pad = b.stack_pad;
            merged.stack_pad_start = b.stack_pad_start;
            merged.stack_pad_join = b.stack_pad_join;
            merged.stack_pad_joins = b.stack_pad_joins;
            merged.stack_pad_deficit = b.stack_pad_deficit;
        }
        if merged.escaped_cells != b.escaped_cells {
            Arc::make_mut(&mut merged.escaped_cells).extend(b.escaped_cells.iter().copied());
        }
        if merged.made_objects != b.made_objects {
            // An object both sides hold alone, with what both wrote alike.
            let objects = Arc::make_mut(&mut merged.made_objects);
            objects.retain(|object, _| b.made_objects.contains_key(object));
            for (object, properties) in objects.iter_mut() {
                let other = &b.made_objects[object];
                properties.retain(|key, value| {
                    other.get(key).is_some_and(|other| same_fact(value, other))
                });
            }
        }
        if merged.cell_facts != b.cell_facts {
            Arc::make_mut(&mut merged.cell_facts).retain(|slot, fact| {
                b.cell_facts
                    .get(slot)
                    .is_some_and(|other| same_fact(fact, other))
            });
        }
        if merged.cell_nodes != b.cell_nodes {
            Arc::make_mut(&mut merged.cell_nodes)
                .retain(|slot, node| b.cell_nodes.get(slot) == Some(node));
        }
        if merged.cell_writes != b.cell_writes {
            let writes = Arc::make_mut(&mut merged.cell_writes);
            for (&slot, &serial) in b.cell_writes.iter() {
                let entry = writes.entry(slot).or_insert(serial);
                *entry = (*entry).max(serial);
            }
        }
        if merged.cell_concrete != b.cell_concrete {
            // A cell neither side wrote since the fork (the same last write)
            // holds what one side read there on both.
            let unwritten = |slot: &u32| a.cell_writes.get(slot) == b.cell_writes.get(slot);
            let concrete = Arc::make_mut(&mut merged.cell_concrete);
            concrete.retain(|slot, node| {
                b.cell_concrete.get(slot) == Some(node)
                    || (!b.cell_concrete.contains_key(slot) && unwritten(slot))
            });
            for (slot, node) in b.cell_concrete.iter() {
                if !a.cell_concrete.contains_key(slot) && unwritten(slot) {
                    concrete.insert(*slot, *node);
                }
            }
        }
        if merged.cell_positions != b.cell_positions || merged.cell_values != b.cell_values {
            let slots: BTreeSet<u32> = [&merged, b]
                .iter()
                .flat_map(|context| {
                    context
                        .cell_positions
                        .keys()
                        .chain(context.cell_values.iter())
                        .copied()
                        .collect::<Vec<_>>()
                })
                .collect();
            for slot in slots {
                let joined = CellMark::of(&merged, slot).join(CellMark::of(b, slot));
                joined.set(&mut merged, slot);
            }
        }
        if !b.try_writes.is_subset(&merged.try_writes) {
            Arc::make_mut(&mut merged.try_writes).extend(b.try_writes.iter().copied());
        }
        // A side that did not come through a catch knows its whole stack.
        merged.from_catch = a.from_catch && b.from_catch;
        merged.catch_try = if merged.from_catch {
            a.catch_try.min(b.catch_try)
        } else {
            None
        };
        // Only what both sides know still holds after the join.
        if merged.branch_facts != b.branch_facts {
            Arc::make_mut(&mut merged.branch_facts)
                .retain(|key, truthy| b.branch_facts.get(key) == Some(truthy));
        }
        // A variable named after the fork is one side's alone (a loop
        // variable carried out of a loop in it, say): what that side knows
        // of its types still holds.
        if merged.type_facts != b.type_facts {
            let one_side = |key: &str| {
                key.strip_prefix("variable ")
                    .and_then(generated_binding_serial)
                    .is_some_and(|serial| serial >= fork.first_binding)
            };
            let facts = Arc::make_mut(&mut merged.type_facts);
            facts.retain(|key, _| b.type_facts.contains_key(key) || one_side(key));
            for (key, types) in facts.iter_mut() {
                if let Some(&other) = b.type_facts.get(key) {
                    *types |= other;
                }
            }
            for (key, &types) in b.type_facts.iter() {
                if one_side(key) && !facts.contains_key(key) {
                    facts.insert(key.clone(), types);
                }
            }
        }
        for (name, origin) in b.merged_concrete.iter() {
            if !merged.merged_concrete.contains_key(name) {
                Arc::make_mut(&mut merged.merged_concrete).insert(name.clone(), *origin);
            }
        }
        for (name, set) in b.value_sets.iter() {
            if !merged.value_sets.contains_key(name) {
                Arc::make_mut(&mut merged.value_sets).insert(name.clone(), set.clone());
            }
        }
        for (name, set) in b.jump_sets.iter() {
            if !merged.jump_sets.contains_key(name) {
                Arc::make_mut(&mut merged.jump_sets).insert(name.clone(), set.clone());
            }
        }
        let mut concrete = Vec::new();
        let mut sets = Vec::new();
        let union = |executor: &Executor, left: &StoredValue, right: &StoredValue| {
            let mut set = executor.candidates(left, None)?;
            for value in executor.candidates(right, None)? {
                if !set.iter().any(|known| same_fact(known, &value)) {
                    set.push(value);
                }
            }
            (set.len() <= MAX_VALUE_SET).then_some(set)
        };
        // What a `try` body bound after leaving its block is assigned inside
        // the block: use the value it was bound to.
        // Only for the join of that `try`'s own body and catch.
        let own = |left: Option<BlockTarget>| left.filter(|left| left.statement == fork.statement);
        let (a_left, b_left) = (own(a.left_try), own(b.left_try));
        let scopes = (a_left.is_some() || b_left.is_some()).then(|| self.executor.scopes());
        let before = |executor: &Executor, stored: StoredValue, block: Option<BlockTarget>| {
            let (Some(block), Some(scopes)) = (block, scopes.as_ref()) else {
                return Ok(stored);
            };
            match stored.node {
                Some(node) if stored.fact.is_none() => {
                    let node = executor.in_scope_at(node, block, scopes).ok_or_else(|| {
                        TraceJoinError(format!(
                            "a try body joins its catch with {}, bound after the try",
                            executor.arena.describe(node)
                        ))
                    })?;
                    Ok(StoredValue {
                        value: SymbolicValue::Computed { node },
                        fact: None,
                        node: Some(node),
                    })
                }
                _ => Ok(stored),
            }
        };
        for index in 0..a.stack.len() {
            let left = before(&self.executor, stack_cell_value(&a.stack[index]), a_left)?;
            let right = before(&self.executor, stack_cell_value(&b.stack[index]), b_left)?;
            let either = left.fact.is_some() || right.fact.is_some();
            let set = union(&self.executor, &left, &right);
            if let Some(value) = self.merge_cell(fork, left, right, &provenance)? {
                self.note_join_arms(value.node, a, b);
                if either && value.fact.is_none() {
                    concrete.push((TraceCell::Stack(index), value.node));
                }
                if let (Some(set), None) = (set, &value.fact) {
                    sets.push((value.node, set));
                }
                merged.stack[index] = StackCell::Value(value);
            }
        }
        for (slot, cell) in a.memory.iter() {
            let (left, right) = match (cell, &b.memory[slot]) {
                (MemoryCell::Initialized, MemoryCell::Initialized) => continue,
                (left, right) => (memory_cell_value(left), memory_cell_value(right)),
            };
            let left = before(&self.executor, left, a_left)?;
            let right = before(&self.executor, right, b_left)?;
            let either = left.fact.is_some() || right.fact.is_some();
            let set = union(&self.executor, &left, &right);
            if let Some(value) = self.merge_cell(fork, left, right, &provenance)? {
                self.note_join_arms(value.node, a, b);
                if either && value.fact.is_none() {
                    concrete.push((TraceCell::Memory(*slot), value.node));
                }
                if let (Some(set), None) = (set, &value.fact) {
                    sets.push((value.node, set));
                }
                merged.memory_mut().insert(*slot, MemoryCell::Value(value));
            }
        }
        for (node, set) in sets {
            let name = node
                .and_then(|node| self.executor.arena.nodes.get(node as usize))
                .and_then(|node| match &node.kind {
                    TraceNodeKind::Identifier { name } => Some(name.clone()),
                    _ => None,
                });
            if let Some(name) = name {
                Arc::make_mut(&mut merged.value_sets).insert(name, set.into());
            }
        }
        if let Some(join) = tag {
            for (cell, node) in concrete {
                let name = node
                    .and_then(|node| self.executor.arena.nodes.get(node as usize))
                    .and_then(|node| match &node.kind {
                        TraceNodeKind::Identifier { name } => Some(name.clone()),
                        _ => None,
                    });
                if let Some(name) = name {
                    self.executor.marked.insert(name.clone());
                    Arc::make_mut(&mut merged.merged_concrete)
                        .insert(name, MergedConcrete { join, cell });
                }
            }
        }
        Ok(TracePath {
            context: merged,
            block: fork.parent,
        })
    }

    /// Save the whole trace state: arena lengths, VM state and current block.
    #[must_use]
    pub fn checkpoint(&self) -> TraceCheckpoint {
        TraceCheckpoint(self.executor.checkpoint())
    }

    /// Whether the variable `name` is computed from the variable `of`: one
    /// of its definitions, or of the variables those read, reads `of`.
    #[must_use]
    pub fn copies_variable(&self, name: &str, of: &str) -> bool {
        let executor = &self.executor;
        let mut pending: Vec<TraceNodeId> = executor
            .definitions
            .get(name)
            .into_iter()
            .flatten()
            .map(|&(_, value)| value)
            .collect();
        let mut seen = BTreeSet::new();
        while let Some(node) = pending.pop() {
            if !seen.insert(node) || seen.len() > 256 {
                continue;
            }
            let node = executor.resolve_copies(node);
            let Some(kind) = executor
                .arena
                .nodes
                .get(node as usize)
                .map(|node| &node.kind)
            else {
                continue;
            };
            if let TraceNodeKind::Identifier { name } = kind {
                if name == of {
                    return true;
                }
                pending.extend(
                    executor
                        .definitions
                        .get(name)
                        .into_iter()
                        .flatten()
                        .map(|&(_, value)| value),
                );
            }
            pending.extend(node_children(kind));
        }
        false
    }

    /// Return to `checkpoint`, dropping everything traced since.
    pub fn rollback(&mut self, checkpoint: TraceCheckpoint) {
        self.executor.rollback(checkpoint.0);
    }

    /// What a rollback keeps: the facts a walk learns once for every
    /// retrace. A walk that must leave no trace saves them with this and
    /// restores them with [`Self::forget`].
    pub(crate) fn learned_facts(&self) -> TraceFacts {
        let e = &self.executor;
        TraceFacts(Box::new(Facts {
            marked: e.marked.clone(),
            used_sets: e.used_sets.clone(),
            cells: e.cells.clone(),
            wanted_cells: e.wanted_cells.clone(),
            try_body_writes: e.try_body_writes.clone(),
            try_body_cell_writes: e.try_body_cell_writes.clone(),
            cell_writes: e.cell_writes.clone(),
            cell_types: e.cell_types.clone(),
            cell_holders: e.cell_holders.clone(),
            cell_contents: e.cell_contents.clone(),
            instruction_reads: e.instruction_reads.clone(),
            constant_tables: e.constant_tables.clone(),
            steps: e.steps,
            handlers_traced: e.handlers_traced,
        }))
    }

    /// Restore facts saved by [`Self::learned_facts`].
    pub(crate) fn forget(&mut self, facts: TraceFacts) {
        let Facts {
            marked,
            used_sets,
            cells,
            wanted_cells,
            try_body_writes,
            try_body_cell_writes,
            cell_writes,
            cell_types,
            cell_holders,
            cell_contents,
            instruction_reads,
            constant_tables,
            steps,
            handlers_traced,
        } = *facts.0;
        let e = &mut self.executor;
        (e.marked, e.used_sets, e.cells, e.wanted_cells) = (marked, used_sets, cells, wanted_cells);
        (e.try_body_writes, e.cell_writes, e.cell_contents) =
            (try_body_writes, cell_writes, cell_contents);
        e.try_body_cell_writes = try_body_cell_writes;
        (e.cell_types, e.cell_holders) = (cell_types, cell_holders);
        (e.instruction_reads, e.constant_tables) = (instruction_reads, constant_tables);
        (e.steps, e.handlers_traced) = (steps, handlers_traced);
    }
}
