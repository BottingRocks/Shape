use super::*;

impl Walk<'_> {
    /// Trace the loop whose header is at the cursor. The body is everything
    /// the paths from the header reach; it is traced again with more loop
    /// variables until every iteration agrees with the header state.
    pub(super) fn run_loop(&mut self, cursor: &mut Cursor, header: VisitKey) -> Result<Flow, Halt> {
        // Back at a loop it left: a `break` was a path still inside it.
        // Those positions are not exits; the walk traces the loop again.
        if cursor.visited.contains(&header) {
            let mut banned: Vec<(VisitKey, VisitKey)> = Arc::make_mut(&mut self.learned)
                .breaks
                .remove(&header)
                .into_iter()
                .flatten()
                .map(|at| (header.clone(), at))
                .collect();
            // Not this loop's: a `break` out of a loop around it, which the
            // path passed on its way back here. A loop is around this one
            // only if its body passed this header.
            if banned.is_empty() {
                let learned = Arc::make_mut(&mut self.learned);
                let around: BTreeSet<VisitKey> = learned
                    .bodies
                    .iter()
                    .filter(|(_, body)| body.contains(&header))
                    .map(|(outer, _)| outer.clone())
                    .collect();
                for (outer, positions) in &mut learned.breaks {
                    if !around.contains(outer) {
                        continue;
                    }
                    positions.retain(|at| {
                        let passed = cursor.visited.contains(at);
                        if passed {
                            banned.push((outer.clone(), at.clone()));
                        }
                        !passed
                    });
                }
            }
            if !banned.is_empty() {
                self.retraces.wrong_breaks += 1;
                // The path came back here from each: every position it passed
                // since is inside the loop too, and no exit.
                let inside: Vec<(VisitKey, VisitKey)> = banned
                    .iter()
                    .flat_map(|(outer, at)| {
                        cursor
                            .visited
                            .after(at)
                            .into_iter()
                            .map(move |position| (outer.clone(), position))
                    })
                    .collect();
                let learned = Arc::make_mut(&mut self.learned);
                learned.no_break.extend(banned);
                learned.no_break.extend(inside);
                return Err(Halt::NewLoop(header));
            }
            // Nothing left to ban: the path passed the header in a trace
            // made before a ban, one an inner region's retrace did not undo
            // (a fork's other side waiting where it broke out). The region
            // around both traces itself again, a few times at most.
            let retraced = Arc::make_mut(&mut self.learned)
                .stale_headers
                .entry(header.clone())
                .or_default();
            if *retraced < MAX_STALE_RETRACES {
                *retraced += 1;
                self.retraces.wrong_breaks += 1;
                return Err(Halt::NewLoop(header));
            }
        }
        cursor.visited.freeze();
        // Targets of forks around the loop: a path of the body that reaches
        // one has left the loop.
        let outside = self.targets.len();
        let entry = cursor.clone();
        let checkpoint = self.session.checkpoint();
        let (records, patches) = (self.records.len(), self.patches.len());
        for _ in 0..=MAX_LOOP_WIDENINGS {
            let widened = self
                .learned
                .widened
                .get(&header)
                .cloned()
                .unwrap_or_default();
            let values = self
                .learned
                .loop_values
                .get(&header)
                .cloned()
                .unwrap_or_default();
            let rewritten = self
                .learned
                .loop_cells
                .get(&header)
                .cloned()
                .unwrap_or_default();
            let types = self.learned.loop_types.get(&header).cloned();
            let written = self
                .learned
                .loop_written
                .get(&header)
                .cloned()
                .unwrap_or_default();
            let objects = self
                .learned
                .loop_objects
                .get(&header)
                .cloned()
                .unwrap_or_default();
            let trace = match self.session.open_loop_with(
                &widened,
                &values,
                types.as_ref().unwrap_or(&BTreeMap::new()),
                Some(&rewritten),
                Some(&written),
                Some(&objects),
            ) {
                Ok(trace) => trace,
                Err(error) => return Err(self.unjoinable(cursor, error.0)),
            };
            self.loops.push(LoopFrame {
                header: header.clone(),
                trace,
                dispatched: false,
                used: BTreeSet::new(),
            });
            let mut body = entry.clone();
            // The header's own instruction starts the body; coming back to
            // it later continues the loop.
            let result = match self.step_once(&mut body) {
                Ok(Flow::Running) => self.run_body(&mut body, &header, outside),
                Ok(Flow::Ended) => Ok(Flow::Ended),
                Err(halt) => Err(halt),
            };
            let frame = self.loops.pop().expect("pushed above");
            // Where the body went, for a later `break` ban to tell the loops
            // around a header from the others.
            if matches!(result, Ok(_)) {
                let passed: Vec<VisitKey> = body
                    .visited
                    .layers
                    .iter()
                    .skip(entry.visited.layers.len())
                    .flat_map(|layer| layer.keys())
                    .chain(body.visited.local.keys())
                    .cloned()
                    .collect();
                Arc::make_mut(&mut self.learned)
                    .bodies
                    .entry(header.clone())
                    .or_default()
                    .extend(passed);
            }
            let cause = match result {
                Ok(Flow::Ended) => {
                    self.session.close_loop(&frame.trace);
                    *cursor = body;
                    return Ok(Flow::Ended);
                }
                Ok(Flow::Running) => {
                    // The path left the loop: `break` and carry on after it.
                    let at = (body.pc, body.state, self.session.call_frames().to_vec());
                    Arc::make_mut(&mut self.learned)
                        .breaks
                        .entry(header.clone())
                        .or_default()
                        .insert(at);
                    if let Some(carried) = self.learned.loop_types.get(&header).cloned() {
                        self.session.type_loop_variables(&frame.trace, &carried);
                    }
                    if self.session.loop_before_padding(&frame.trace) {
                        if let Some(halt) = self.padded_loop() {
                            return Err(halt);
                        }
                    }
                    if let Err(error) = self.session.break_loop(&frame.trace) {
                        return Err(self.unjoinable(&body, error.0));
                    }
                    *cursor = body;
                    return Ok(Flow::Running);
                }
                Err(Halt::Rewiden(at)) if at == header => "loop: new loop variables or values",
                // A loop that starts inside this one: trace this one again.
                Err(halt @ (Halt::NewLoop(_) | Halt::Split(_)))
                    if !entry.visited.contains(restarted_at(&halt)) =>
                {
                    self.restart_cause(
                        &halt,
                        [
                            "loop: a loop inside",
                            "loop: a split inside",
                            "loop: a call traced inline",
                        ],
                    )
                }
                Err(halt) => return Err(halt),
            };
            let live = self.records.live();
            self.session.rollback(checkpoint.clone());
            self.records.truncate(records);
            self.patches.truncate(patches);
            self.charge(cause, live);
        }
        Err(self.unjoinable(cursor, "the loop variables do not settle".into()))
    }

    /// Step a loop body until every path ended, or the running path left the
    /// loop by reaching a target of a fork around it (`Flow::Running`).
    pub(super) fn run_body(
        &mut self,
        body: &mut Cursor,
        header: &VisitKey,
        outside: usize,
    ) -> Result<Flow, Halt> {
        loop {
            let at = (body.pc, body.state, self.session.call_frames().to_vec());
            if self.targets[..outside]
                .iter()
                .any(|target| target.reaches(&at))
                && !self
                    .learned
                    .no_break
                    .contains(&(header.clone(), at.clone()))
            {
                return Ok(Flow::Running);
            }
            if self.advance(body)? == Flow::Ended {
                return Ok(Flow::Ended);
            }
        }
    }

    /// Trace the `try` the running path just entered: its body and the
    /// handler the VM runs when the body throws, as `try { … } catch (e) {
    /// … }`, merged where they meet. Cells the body writes become variables
    /// the catch reads; the try is traced again until that set settles.
    pub(super) fn run_try(&mut self, cursor: &mut Cursor, frame: TryFrame) -> Result<Flow, Halt> {
        self.try_handlers.push(DispatchStart {
            pc: frame.handler_byte_offset,
            state: frame.handler_dispatch_state,
        });
        cursor.visited.freeze();
        let entry = cursor.clone();
        let checkpoint = self.session.checkpoint();
        let (records, patches) = (self.records.len(), self.patches.len());
        let entry_cells = self.session.cells();
        let entry_concrete = self.session.concrete_cells();
        let depth = self.session.try_depth();
        let key = (entry.pc, entry.state, self.session.call_frames().to_vec());
        let mut widened = self.learned.widened.get(&key).cloned().unwrap_or_default();
        let marks = self.session.stop_marks();
        for _ in 0..=MAX_LOOP_WIDENINGS {
            // Each pass traces the `try` from its entry again.
            *cursor = entry.clone();
            self.session.take_try_body_writes(depth);
            self.session.take_try_body_cell_writes(depth);
            let cells_written = self
                .learned
                .try_cells
                .get(&key)
                .cloned()
                .unwrap_or_default();
            let values = self
                .learned
                .try_values
                .get(&key)
                .cloned()
                .unwrap_or_default();
            let types = self.learned.try_types.get(&key).cloned();
            // Variables the body writes before anything the catch takes can
            // throw, as a pass found: the catch never sees their entry value.
            let entry_hidden: BTreeSet<TraceCell> = self
                .learned
                .try_entry_exposed
                .get(&key)
                .into_iter()
                .flatten()
                .filter(|(_, exposed)| !**exposed)
                .map(|(cell, _)| *cell)
                .collect();
            let (fork, body, catch) = match self.session.fork_try_with(
                &widened,
                &values,
                types.as_ref().unwrap_or(&BTreeMap::new()),
                &cells_written,
                &entry_hidden,
            ) {
                Ok(paths) => paths,
                Err(error) => return Err(self.unjoinable(cursor, error.0)),
            };
            let names = TraceSession::try_variable_names(&body);
            let handler = (frame.handler_byte_offset, frame.handler_dispatch_state);
            self.finally_inline.remove(&fork.statement);
            self.finally_reads.remove(&fork.statement);
            self.finally_writes.remove(&fork.statement);
            if self.finally_handlers.contains(&handler) && !self.plain_finally.contains(&handler) {
                self.session.mark_finally(fork.statement);
            }
            let mut body = Side {
                cursor: entry.clone(),
                path: body,
                ended: false,
                history: Vec::new(),
                held: false,
            };
            let mut catch = Side {
                cursor: Cursor {
                    pc: frame.handler_byte_offset,
                    state: frame.handler_dispatch_state,
                    ..entry.clone()
                },
                path: catch,
                ended: false,
                history: Vec::new(),
                held: false,
            };
            let meet = self.learned.try_meets.get(&key).cloned();
            let body_first = self.learned.body_first.contains(&key);
            self.open_tries
                .push((frame, fork.statement, self.targets.len()));
            let resolved = self.resolve_try(
                &fork,
                &mut body,
                &mut catch,
                depth,
                cursor,
                meet.as_ref(),
                body_first,
            );
            self.open_tries.pop();
            let resolved = resolved.map(|resolved| {
                let inline = self.finally_inline.remove(&fork.statement).unwrap_or(0);
                let reads = self
                    .finally_reads
                    .remove(&fork.statement)
                    .unwrap_or_default();
                let writes = self
                    .finally_writes
                    .remove(&fork.statement)
                    .unwrap_or_default();
                (
                    resolved,
                    inline > 0
                        || !reads.is_disjoint(&writes)
                        || self.session.finally_reads_exception(&fork),
                )
            });
            // A `finally` a path ran inline, emitted as `finally { }` too,
            // would run twice; one that writes what a `return` that skipped
            // it reads after it would be read before it; one that reads the
            // exception is a `catch`: the `try` is traced again with it
            // plain.
            if let Ok((_, conflict)) = &resolved {
                if *conflict && self.session.is_finally(&fork) && self.plain_finally.insert(handler)
                {
                    let live = self.records.live();
                    self.session.rollback(checkpoint.clone());
                    self.records.truncate(records);
                    self.patches.truncate(patches);
                    self.charge("try: a finally run inline", live);
                    continue;
                }
            }
            let (flow, written) = match resolved.map(|(resolved, _)| resolved) {
                Ok(resolved) => resolved,
                Err(Halt::TryMeet(at)) if meet.as_ref() != Some(&at) => {
                    self.retraces.meeting_points += 1;
                    Arc::make_mut(&mut self.learned)
                        .try_meets
                        .insert(key.clone(), at);
                    let live = self.records.live();
                    self.session.rollback(checkpoint.clone());
                    self.records.truncate(records);
                    self.patches.truncate(patches);
                    self.charge("try: a meeting point found again", live);
                    continue;
                }
                Err(Halt::TryMeet(_)) => {
                    return Err(self.unjoinable(
                        cursor,
                        "the catch path crosses the body again where they were to meet".into(),
                    ))
                }
                // A VM value the catch needed concrete may be a cell the
                // body wrote: the catch reads it as a `try` variable,
                // with the values the body gave it, once traced again.
                Err(halt @ Halt::Stop(PrefixStopReason::UnsupportedHandler { .. }))
                    if self.session.last_diagnostic().is_some_and(|diagnostic| {
                        matches!(
                            diagnostic.code,
                            TraceStopCode::DynamicIndex | TraceStopCode::ProgramValueJump
                        )
                    }) =>
                {
                    let unseen: BTreeSet<TraceCell> = self
                        .session
                        .take_try_body_writes(depth)
                        .intersection(&entry_cells)
                        .filter(|cell| !widened.contains(*cell))
                        .copied()
                        .collect();
                    // The values the body gave them, for a VM jump through
                    // one (see `TraceContext::jump_sets`).
                    let assigned = self.session.try_assigned_values(&fork, &names);
                    let grew = adds_values(self.learned.try_values.get(&key), &assigned);
                    if grew {
                        let known = Arc::make_mut(&mut self.learned)
                            .try_values
                            .entry(key.clone())
                            .or_default();
                        join_values(known, assigned);
                    }
                    // So do the types the body gave them: the catch may have
                    // decided a test on one with fewer.
                    let assigned_types = self.session.try_assigned_types(&fork, &names);
                    let known_types = self.learned.try_types.get(&key);
                    let grew_types = assigned_types.iter().any(|(cell, bits)| {
                        known_types
                            .and_then(|known| known.get(cell))
                            .is_none_or(|known| bits & !known != 0)
                    });
                    if grew_types {
                        let known = Arc::make_mut(&mut self.learned)
                            .try_types
                            .entry(key.clone())
                            .or_default();
                        for (cell, bits) in assigned_types {
                            *known.entry(cell).or_default() |= bits;
                        }
                    }
                    let grew = grew || grew_types;
                    if unseen.is_empty() && !grew {
                        // The body may not have got to its writes before the
                        // catch did: walked first, it shows them.
                        if body_first {
                            return Err(halt);
                        }
                        Arc::make_mut(&mut self.learned)
                            .body_first
                            .insert(key.clone());
                    }
                    self.retraces.try_variables += 1;
                    widened.extend(unseen);
                    Arc::make_mut(&mut self.learned)
                        .widened
                        .insert(key.clone(), widened.clone());
                    self.session.restore_stop_marks(marks.clone());
                    let live = self.records.live();
                    self.session.rollback(checkpoint.clone());
                    self.records.truncate(records);
                    self.patches.truncate(patches);
                    self.charge("try: a try variable the catch needed", live);
                    continue;
                }
                // A loop that starts inside the `try`: trace it again.
                Err(halt @ (Halt::NewLoop(_) | Halt::Split(_)))
                    if !entry.visited.contains(restarted_at(&halt)) =>
                {
                    let cause = self.restart_cause(
                        &halt,
                        [
                            "try: a loop inside",
                            "try: a split inside",
                            "try: a call traced inline",
                        ],
                    );
                    let live = self.records.live();
                    self.session.rollback(checkpoint.clone());
                    self.records.truncate(records);
                    self.patches.truncate(patches);
                    self.charge(cause, live);
                    continue;
                }
                // A catch that reads the stack the throw left stops with no
                // evidence about any fork. Walked first, the body may show
                // its side is no code that runs, or complete: the `try` is
                // traced again with the body first.
                Err(halt @ Halt::Stop(_))
                    if self.session.last_diagnostic().is_some_and(|diagnostic| {
                        diagnostic.code == TraceStopCode::CatchStackUnknown
                    }) =>
                {
                    // The catch may have jumped by a value the body had not
                    // assigned yet: what the body assigned is learned first.
                    let assigned = self.session.try_assigned_values(&fork, &names);
                    let grew = adds_values(self.learned.try_values.get(&key), &assigned);
                    if body_first && !grew {
                        return Err(halt);
                    }
                    if grew {
                        let known = Arc::make_mut(&mut self.learned)
                            .try_values
                            .entry(key.clone())
                            .or_default();
                        join_values(known, assigned);
                    }
                    Arc::make_mut(&mut self.learned)
                        .body_first
                        .insert(key.clone());
                    self.session.restore_stop_marks(marks.clone());
                    let live = self.records.live();
                    self.session.rollback(checkpoint.clone());
                    self.records.truncate(records);
                    self.patches.truncate(patches);
                    self.charge("try: a catch reading the throw's stack", live);
                    continue;
                }
                Err(halt) => return Err(halt),
            };
            // Writes on body paths that ended since count too: a throw after
            // one reaches the catch with it.
            let mut written = written;
            written.extend(self.session.take_try_body_writes(depth));
            let unseen: BTreeSet<TraceCell> = written
                .intersection(&entry_cells)
                .filter(|cell| !widened.contains(*cell))
                .copied()
                .collect();
            // What the body assigns joins each variable's value set; a catch
            // that took a value from the set missed the new ones.
            let assigned = self.session.try_assigned_values(&fork, &names);
            let grew = adds_values(self.learned.try_values.get(&key), &assigned);
            if grew {
                let known = Arc::make_mut(&mut self.learned)
                    .try_values
                    .entry(key.clone())
                    .or_default();
                join_values(known, assigned);
            }
            // Whether a throw the catch takes can see each variable's entry
            // value. A variable found hidden for the first time, or exposed
            // after a pass took it as hidden, changes its value set.
            let exposed = self.session.try_entry_exposed(&fork, &names);
            let known_exposed = self.learned.try_entry_exposed.get(&key);
            let changed: BTreeSet<TraceCell> = exposed
                .iter()
                .filter(|(cell, now)| {
                    let was = known_exposed.and_then(|known| known.get(*cell)).copied();
                    match was {
                        None => !**now,
                        Some(false) => **now,
                        Some(true) => false,
                    }
                })
                .map(|(cell, _)| *cell)
                .collect();
            if !changed.is_empty() {
                let known = Arc::make_mut(&mut self.learned)
                    .try_entry_exposed
                    .entry(key.clone())
                    .or_default();
                for (cell, now) in &exposed {
                    *known.entry(*cell).or_default() |= *now;
                }
            }
            let used = |name: &String| {
                self.used_sets.contains(name) || self.used_sets.contains(&format!("set {name}"))
            };
            let mut stale = (grew && names.values().any(used))
                || changed.iter().any(|cell| names.get(cell).is_some_and(used));
            // What the body assigns joins each variable's types: a catch
            // traced with fewer is traced again.
            let assigned = self.session.try_assigned_types(&fork, &names);
            let grew_types = types.as_ref().map_or(true, |known| {
                assigned
                    .iter()
                    .any(|(cell, bits)| known.get(cell).map_or(true, |k| bits & !k != 0))
            });
            // A variable whose types a pass learns for the first time, narrower
            // than any: the `try` is traced again with them.
            let first_learned = assigned.iter().any(|(cell, bits)| {
                *bits != 0
                    && bits & 0x1ff != 0x1ff
                    && types
                        .as_ref()
                        .map_or(true, |known| !known.contains_key(cell))
                    && names.get(cell).is_some_and(|name| {
                        self.used_sets
                            .iter()
                            .filter_map(|used| used.strip_prefix("wanted "))
                            .any(|wanted| {
                                wanted == name || self.session.copies_variable(wanted, name)
                            })
                    })
            });
            if grew_types {
                let known = Arc::make_mut(&mut self.learned)
                    .try_types
                    .entry(key.clone())
                    .or_default();
                for (cell, bits) in assigned {
                    *known.entry(cell).or_default() |= bits;
                }
                // Only a catch or body that decided on a variable's types
                // took too few.
                // Types learned again trace it again only when a decision took
                // too few.
                stale |= first_learned
                    || names
                        .values()
                        .any(|name| self.used_sets.contains(&format!("typed {name}")))
                    // A `finally` traced once reads the variables with the
                    // types the body gives them: learned only now, it is
                    // traced again with them.
                    || self.session.joined_finally(&fork);
            }
            // A cell the catch kept what the path concretely put there that
            // the body writes: the catch finds what the body put there.
            let cells_unseen: BTreeSet<u32> = self
                .session
                .take_try_body_cell_writes(depth)
                .intersection(&entry_concrete)
                .filter(|slot| !cells_written.contains(slot))
                .copied()
                .collect();
            if unseen.is_empty() && !stale && cells_unseen.is_empty() {
                if flow != Flow::Ended {
                    self.session.type_try_variables(&names);
                }
                return Ok(flow);
            }
            if !cells_unseen.is_empty() {
                Arc::make_mut(&mut self.learned)
                    .try_cells
                    .entry(key.clone())
                    .or_default()
                    .extend(cells_unseen);
            }
            self.retraces.try_variables += 1;
            widened.extend(unseen);
            Arc::make_mut(&mut self.learned)
                .widened
                .insert(key.clone(), widened.clone());
            let live = self.records.live();
            self.session.rollback(checkpoint.clone());
            self.records.truncate(records);
            self.patches.truncate(patches);
            self.charge("try: new try variables or values", live);
        }
        Err(self.unjoinable(cursor, "the try variables do not settle".into()))
    }

    /// Step the body and catch paths of `fork` until they merge or end.
    /// Returns the flow after the `try` (the merged path is then running at
    /// `cursor`) and the cells the body wrote.
    pub(super) fn resolve_try(
        &mut self,
        fork: &TraceFork,
        body: &mut Side,
        catch: &mut Side,
        depth: usize,
        cursor: &mut Cursor,
        meet: Option<&VisitKey>,
        body_first: bool,
    ) -> Result<(Flow, BTreeSet<TraceCell>), Halt> {
        let mut written: Option<BTreeSet<TraceCell>> = None;
        // Where the body had been when it closed the `try`: what it passes
        // after that, without emitting code, the catch may reach later.
        let mut closed_visited: Option<Visited> = None;
        // The closed body could not step without emitting code: the catch
        // runs until it reaches the body or ends.
        let mut blocked = false;
        // The paths entered a `finally` together but did not join there: they
        // are traced apart, each through its own copy.
        let mut finally_apart = false;
        loop {
            // The body's statements stay in the `try` block until it pops
            // the frame.
            if written.is_none() {
                if body.ended {
                    written = Some(TraceSession::try_writes(&body.path).clone());
                } else if body.path.try_depth() < depth {
                    match self.session.close_try_body(fork, &mut body.path) {
                        Ok(cells) => written = Some(cells),
                        Err(error) => return Err(self.unjoinable(&body.cursor, error.0)),
                    }
                }
            }
            let closed = written.is_some();
            if closed && closed_visited.is_none() {
                closed_visited = Some(body.cursor.visited.clone());
            }
            match (body.ended, catch.ended) {
                (true, true) => {
                    // Where the paths went, for an enclosing fork to meet.
                    cursor.visited = Visited::join(
                        &cursor.visited,
                        [
                            std::mem::take(&mut body.cursor.visited),
                            std::mem::take(&mut catch.cursor.visited),
                        ],
                    );
                    return Ok((Flow::Ended, written.unwrap_or_default()));
                }
                // The body never falls through. Code only the catch reaches
                // stays in the catch block, until an enclosing fork can
                // meet it: there the catch carries on after the `try`.
                (true, false) => {
                    if self.at_target(&position(catch)) {
                        if let Err(error) =
                            self.session
                                .continue_alone(fork, &mut catch.path, &body.path)
                        {
                            return Err(self.unjoinable(&catch.cursor, error.0));
                        }
                        self.session.switch_path(&mut catch.path);
                        *cursor = catch.cursor.clone();
                        return Ok((Flow::Running, written.unwrap_or_default()));
                    }
                    catch.ended = self.advance_catch(catch)? == Flow::Ended;
                    continue;
                }
                (false, true) if closed => {
                    // The body alone continues after the `try`.
                    self.session.switch_path(&mut body.path);
                    *cursor = body.cursor.clone();
                    return Ok((Flow::Running, written.unwrap_or_default()));
                }
                (false, true) => {
                    body.ended = self.advance_side(body)? == Flow::Ended;
                    continue;
                }
                (false, false) => {}
            }
            // The body past the `try` steps without emitting code, so it may
            // pass code the catch reaches later: there they meet.
            let at_catch = position(catch);
            if closed_visited
                .as_ref()
                .is_some_and(|before| !before.contains(&at_catch))
                && body.cursor.visited.contains(&at_catch)
                && position(body) != at_catch
            {
                return Err(Halt::TryMeet(at_catch));
            }
            let same_position = position(body) == at_catch;
            // Both enter the `finally`: it is traced once, from both.
            let plain = self.open_tries.last().is_some_and(|(frame, ..)| {
                self.plain_finally
                    .contains(&(frame.handler_byte_offset, frame.handler_dispatch_state))
            });
            if closed && same_position && !finally_apart && !plain {
                let checkpoint = self.session.checkpoint();
                match self.session.join_finally(fork, &body.path, &catch.path) {
                    Ok(Some(mut merged)) => {
                        // The entry the body made is the `finally` block.
                        if let Some(count) = self.finally_inline.get_mut(&fork.statement) {
                            *count = count.saturating_sub(1);
                        }
                        let (pc, state) = (body.cursor.pc, body.cursor.state);
                        let base = std::mem::take(&mut cursor.visited);
                        let visited = Visited::join(
                            &base,
                            [
                                std::mem::take(&mut body.cursor.visited),
                                std::mem::take(&mut catch.cursor.visited),
                            ],
                        );
                        self.session.switch_path(&mut merged);
                        if let Err(reason) = merge_images(&mut body.cursor, &catch.cursor) {
                            return Err(self.unjoinable(&body.cursor, reason));
                        }
                        *cursor = body.cursor.clone();
                        cursor.pc = pc;
                        cursor.state = state;
                        cursor.visited = visited;
                        return Ok((Flow::Running, written.unwrap_or_default()));
                    }
                    Ok(None) => {}
                    Err(_) => {
                        self.session.rollback(checkpoint);
                        finally_apart = true;
                    }
                }
            }
            if closed && same_position && TraceSession::joinable(&body.path, &catch.path) {
                let (pc, state) = (body.cursor.pc, body.cursor.state);
                let base = std::mem::take(&mut cursor.visited);
                let visited = Visited::join(
                    &base,
                    [
                        std::mem::take(&mut body.cursor.visited),
                        std::mem::take(&mut catch.cursor.visited),
                    ],
                );
                let paths = (
                    std::mem::replace(&mut body.path, catch.path.clone()),
                    catch.path.clone(),
                );
                let mut merged = match self.session.join(*fork, paths.0, paths.1) {
                    Ok(merged) => merged,
                    Err(error) => return Err(self.unjoinable(&body.cursor, error.0)),
                };
                self.session.switch_path(&mut merged);
                if let Err(reason) = merge_images(&mut body.cursor, &catch.cursor) {
                    return Err(self.unjoinable(&body.cursor, reason));
                }
                *cursor = body.cursor.clone();
                cursor.pc = pc;
                cursor.state = state;
                cursor.visited = visited;
                return Ok((Flow::Running, written.unwrap_or_default()));
            }
            // The deeper VM call stack first, then the lower PC; the catch
            // goes first where both stand at the same place.
            let lower_first = match body
                .path
                .call_frames()
                .len()
                .cmp(&catch.path.call_frames().len())
            {
                std::cmp::Ordering::Greater => true,
                std::cmp::Ordering::Less => false,
                std::cmp::Ordering::Equal => {
                    (body.cursor.pc, body.cursor.state) < (catch.cursor.pc, catch.cursor.state)
                }
            };
            // A closed body waits where the catch will meet it. A body
            // walked first goes on until it closes.
            let waiting = closed && meet == Some(&position(body));
            let body_now = if body_first && !closed {
                true
            } else {
                lower_first
            };
            if !body_now || blocked || waiting {
                catch.ended = self.advance_catch(catch)? == Flow::Ended;
            } else if !closed {
                body.ended = self.advance_side(body)? == Flow::Ended;
            } else if !self.advance_side_silently(body)? {
                // After the `try`, the body may not run code the catch path
                // has not reached yet.
                blocked = true;
                catch.ended = self.advance_catch(catch)? == Flow::Ended;
            }
        }
    }
}
