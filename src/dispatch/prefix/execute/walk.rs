use super::*;

impl Walk<'_> {
    /// Step the running path until the VM function returns on every path.
    /// The path keeps a history, as a fork's side does: a loop, a split, a
    /// call to trace inline or a fork found fake at a position it passed
    /// takes it back to the step that reached that position, not back to
    /// the start.
    pub(super) fn run(&mut self, cursor: &mut Cursor) -> Result<(), Halt> {
        let mut top = Side {
            cursor: cursor.clone(),
            path: self.session.current_path(),
            ended: false,
            history: Vec::new(),
            held: false,
        };
        let result = loop {
            let mut step = self.begin_step(&top);
            let result = self.advance_side(&mut top);
            step.made = self.made_since(&step.made);
            let halt = match result {
                Ok(Flow::Running) => {
                    top.history.push(step);
                    continue;
                }
                Ok(Flow::Ended) => break Ok(()),
                Err(halt) => halt,
            };
            let at = match &halt {
                Halt::NewLoop(at) | Halt::Split(at)
                    if self.learned.headers.len() <= MAX_LOOP_HEADERS =>
                {
                    Some(at.clone())
                }
                Halt::ForceFake(jump, ..) if !self.learned.forced.contains_key(jump) => {
                    Some(jump.clone())
                }
                _ => None,
            };
            top.history.push(step);
            let Some(index) = at.and_then(|at| Self::step_reaching(&top, &at)) else {
                top.history.pop();
                break Err(halt);
            };
            let cause = match halt {
                Halt::NewLoop(_) => "walk: back to a loop header",
                Halt::Split(at) if self.learned.inline_calls.contains(&at) => {
                    "walk: back to a call traced inline"
                }
                Halt::ForceFake(jump, taken_fake, reason) => {
                    self.retraces.wrong_breaks += 1;
                    Arc::make_mut(&mut self.learned)
                        .forced
                        .insert(jump, (taken_fake, reason));
                    "walk: back to a forced fake branch"
                }
                _ => "walk: back to a split",
            };
            self.session
                .restore_stop_marks(top.history[index].stops.clone());
            self.cut_back(&mut top, index, None, cause);
        };
        // The session runs the path again, where its steps left it.
        self.session.switch_path(&mut top.path);
        *cursor = top.cursor;
        result
    }

    /// One dispatch step of the running path. A fork is traced to its join
    /// and a loop to its end, so the path is again a single cursor
    /// afterwards, unless it ended.
    pub(super) fn advance(&mut self, cursor: &mut Cursor) -> Result<Flow, Halt> {
        let may_hold = std::mem::take(&mut self.may_hold);
        let header = (cursor.pc, cursor.state, self.session.call_frames().to_vec());
        if let Some(index) = self.loops.iter().rposition(|frame| frame.header == header) {
            // Loop variables are stack cells counted from the loop's entry.
            if self.session.loop_before_padding(&self.loops[index].trace) {
                if let Some(halt) = self.padded_loop() {
                    return Err(halt);
                }
            }
            // A loop variable that holds what an outlined call returned has no
            // known values: traced inline, it might. That matters only to a
            // jump that dispatched on the loop's values; a VM value that needs
            // the call's result concrete sends it inline anyway.
            if let Some(tag) = self
                .session
                .loop_outlined_call(&self.loops[index].trace)
                .filter(|_| self.loops[index].dispatched)
            {
                return Err(self.inline_call(tag));
            }
            // What each loop variable holds now joins its value set; a jump
            // that already dispatched on the set missed the new values.
            let values = self.session.loop_values(&self.loops[index].trace);
            let grew = adds_values(
                self.learned.loop_values.get(&header),
                values.iter().map(|(cell, values)| (cell, values)),
            );
            if grew {
                let known = Arc::make_mut(&mut self.learned)
                    .loop_values
                    .entry(header.clone())
                    .or_default();
                join_values(known, values);
            }
            // What each holds joins its types (see `LoopFrame::typed`).
            let types = self.session.loop_types(&self.loops[index].trace);
            let known = self.learned.loop_types.get(&header);
            let grew_types = types.iter().any(|(cell, bits)| {
                known
                    .and_then(|known| known.get(cell))
                    .map_or(true, |k| bits & !k != 0)
            });
            // A decision in this attempt read a variable's types that grew:
            // it may have taken too few.
            let frame = &self.loops[index];
            let known_types = self.learned.loop_types.get(&header);
            let read_grown = types.iter().any(|(cell, bits)| {
                known_types
                    .and_then(|known| known.get(cell))
                    .map_or(true, |k| bits & !k != 0)
                    && frame
                        .trace
                        .variable_name(*cell)
                        .is_some_and(|name| frame.used.contains(&format!("typed {name}")))
            });
            if grew_types {
                let known = Arc::make_mut(&mut self.learned)
                    .loop_types
                    .entry(header.clone())
                    .or_default();
                for (cell, bits) in types {
                    *known.entry(cell).or_default() |= bits;
                }
            }
            if grew_types && read_grown {
                self.retraces.loop_values += 1;
                return Err(Halt::Rewiden(header));
            }
            // A closed-cell test decided in this attempt counted the closures
            // holding the cell made before it: one the body made after it
            // runs before it in the next iteration.
            let held: BTreeSet<u32> = self.loops[index]
                .used
                .iter()
                .filter_map(|used| used.strip_prefix("holders ")?.parse().ok())
                .collect();
            if !held.is_empty() && self.session.cell_holders_grew(&held) {
                self.retraces.loop_values += 1;
                return Err(Halt::Rewiden(header));
            }
            // A test decided on a variable's value set that grew is decided
            // again, as a dispatch on it would be.
            let frame = &self.loops[index];
            let read_set = frame
                .used
                .iter()
                .filter_map(|used| used.strip_prefix("set "))
                .any(|name| frame.trace.has_variable(name));
            if grew && (self.loops[index].dispatched || read_set) {
                self.retraces.loop_values += 1;
                return Err(Halt::Rewiden(header));
            }
            // Statements leave the VM stack as they found it: a path that
            // comes back with another stack length is no iteration real
            // code runs, and the fork that led it here is a fake branch.
            if let Some((entry, now)) = self.session.loop_stack_change(&self.loops[index].trace) {
                if let Some(halt) = self.undo_padding() {
                    return Err(halt);
                }
                let (pc, state) = (cursor.pc, cursor.state);
                let reason = format!("an iteration changes the stack length from {entry} to {now}");
                self.session.stop_prefix(
                    TraceStopCode::PrefixLoopStackChange,
                    format!("pc {pc} state {state}: {reason}"),
                    None,
                );
                return Err(PrefixStopReason::UnjoinableBranches { pc, state, reason }.into());
            }
            let widen = match self.session.continue_loop(&self.loops[index].trace) {
                Ok(widen) => widen,
                Err(error) => return Err(self.unjoinable(cursor, error.0)),
            };
            if !widen.is_empty() {
                // The attempt cannot stand: retrace the loop right away.
                self.retraces.loop_variables += 1;
                Arc::make_mut(&mut self.learned)
                    .widened
                    .entry(header.clone())
                    .or_default()
                    .extend(widen);
                return Err(Halt::Rewiden(header));
            }
            let rewritten = self.session.loop_cell_changes(&self.loops[index].trace);
            if !rewritten.is_empty() {
                self.retraces.loop_variables += 1;
                let known = Arc::make_mut(&mut self.learned)
                    .loop_cells
                    .entry(header.clone())
                    .or_default();
                for (slot, mark) in rewritten {
                    let mark = known.get(&slot).map_or(mark, |known| known.join(mark));
                    known.insert(slot, mark);
                }
                return Err(Halt::Rewiden(header));
            }
            // An iteration wrote a cell the header kept what the path
            // concretely put there before: a later iteration finds another
            // value.
            let written = self.session.loop_cell_writes(&self.loops[index].trace);
            if !written.is_empty() {
                self.retraces.loop_variables += 1;
                Arc::make_mut(&mut self.learned)
                    .loop_written
                    .entry(header.clone())
                    .or_default()
                    .extend(written);
                return Err(Halt::Rewiden(header));
            }
            // An iteration wrote or handed on an object the header kept what
            // it knew of: a later iteration finds other values there.
            let objects = self.session.loop_object_writes(&self.loops[index].trace);
            if !objects.is_empty() {
                self.retraces.loop_variables += 1;
                Arc::make_mut(&mut self.learned)
                    .loop_objects
                    .entry(header.clone())
                    .or_default()
                    .extend(objects);
                return Err(Halt::Rewiden(header));
            }
            return Ok(Flow::Ended);
        }
        if self.learned.headers.contains(&header) {
            return self.run_loop(cursor, header);
        }
        if cursor.visited.contains(&header) {
            // A loop the walk did not know: start over with it. The meeting
            // points and loop variables found so far are kept, so the walk
            // back to here is cheap.
            Arc::make_mut(&mut self.learned)
                .headers
                .insert(header.clone());
            self.retraces.new_loops += 1;
            return Err(Halt::NewLoop(header));
        }
        self.may_hold = may_hold;
        self.step_once(cursor)
    }

    /// Execute the instruction at the cursor, tracing a fork it opens.
    pub(super) fn step_once(&mut self, cursor: &mut Cursor) -> Result<Flow, Halt> {
        let may_hold = std::mem::take(&mut self.may_hold);
        if self.steps >= self.preparation.limits.max_dispatch_steps
            || self.audit_until.is_some_and(|until| self.steps >= until)
        {
            return Err(PrefixStopReason::Budget.into());
        }
        if self.audit_until.is_some() {
            *self
                .audit_seen
                .entry((cursor.pc, cursor.state))
                .or_default() += 1;
        }
        let depth = self.session.call_frames().len();
        let at = (cursor.pc, cursor.state, self.session.call_frames().to_vec());
        let step = self.step(cursor);
        // A loop whose value set a VM value came from must be traced again
        // if the set grows.
        for name in self.session.take_used_sets() {
            for frame in &mut self.loops {
                if frame.trace.has_variable(&name) {
                    frame.dispatched = true;
                }
                frame.used.insert(name.clone());
            }
            self.used_sets.insert(name);
        }
        let step = match step {
            Ok(step) => step,
            Err(stop) => {
                // Past a catch, the stack below the `try`'s is the throw's:
                // an index outside what the tracer knows is no evidence.
                if self.session.from_catch()
                    && matches!(stop, PrefixStopReason::UnsupportedHandler { .. })
                    && self
                        .session
                        .last_diagnostic()
                        .is_some_and(|diagnostic| diagnostic.code == TraceStopCode::InvalidIndex)
                {
                    let message = format!(
                        "after a catch, pc {} state {} reads a stack the throw left",
                        at.0, at.1
                    );
                    self.catch_stop_try = self.session.catch_try();
                    self.session
                        .stop_prefix(TraceStopCode::CatchStackUnknown, message, None);
                }
                if let Some(halt) = self.pad_conflict() {
                    return Err(halt);
                }
                if let Some(halt) = self.junk_carried_out() {
                    return Err(halt);
                }
                // A junk function whose entry a fork's side patched: that
                // side is the fake one, not the fork the path is in now.
                if let Some((jump, taken_fake, reason)) = self.patched_junk.take() {
                    if !self.learned.forced.contains_key(&jump) {
                        return Err(Halt::ForceFake(jump, taken_fake, reason));
                    }
                }
                // A byte the fork's sides patched differently: they go on
                // apart, each on its own image.
                if let Some(jump) = self
                    .split_read_at
                    .take()
                    .and_then(|address| Some((address, self.image_splits.get(&address)?.clone())))
                    .filter(|(address, jump)| {
                        Arc::make_mut(&mut self.learned)
                            .apart_bytes
                            .entry(jump.clone())
                            .or_default()
                            .insert(*address)
                    })
                    .map(|(_, jump)| jump)
                {
                    self.retraces.splits += 1;
                    return Err(Halt::Split(jump));
                }
                let dispatch = self.session.take_dispatch();
                let halt = self.split_or(stop, at.clone());
                if let (Halt::Stop(PrefixStopReason::UnsupportedHandler { .. }), Some(dispatch)) =
                    (&halt, dispatch)
                {
                    return self.dispatch(cursor, &at, dispatch);
                }
                return Err(halt);
            }
        };
        // Only straight-line code becomes a function.
        if self.summarizing > 0 && matches!(step, Step::Branch(..) | Step::Try(_)) {
            return Err(Halt::NotFunction);
        }
        match step {
            Step::Continue if self.session.call_frames().len() > depth => {
                Ok(self.call(cursor, at, depth))
            }
            // A test on what an outlined call returned: traced inline, its
            // outcome might be known. A call whose returns the test was
            // already decided on, with its arguments, stays outlined.
            Step::Branch(branch, _) if self.session.unresolved_call_in(branch.test).is_some() => {
                let tag = self
                    .session
                    .unresolved_call_in(branch.test)
                    .expect("checked by the guard");
                Err(self.inline_call(tag))
            }
            Step::Continue => {
                if let Some((dead, at)) = self.pending_audit.take() {
                    if self.preparation.limits.audit_fakes && self.audit_until.is_none() {
                        self.audit_fake(cursor, dead, at);
                    }
                }
                Ok(Flow::Running)
            }
            Step::End => Ok(Flow::Ended),
            Step::Try(frame) => self.run_try(cursor, frame),
            Step::Branch(..) if may_hold => Err(Halt::Held),
            Step::Branch(branch, record) => self.fork(cursor, branch, Some(record), None),
        }
    }

    /// The running path just called the VM subroutine at the cursor. A
    /// subroutine is traced once, as a function, and every call to it
    /// becomes a call of that function; one that cannot be a function is
    /// traced inline at every call (the path simply steps into it).
    pub(super) fn call(&mut self, cursor: &mut Cursor, site: VisitKey, depth: usize) -> Flow {
        if !self.preparation.limits.outline_subroutines || self.learned.inline_calls.contains(&site)
        {
            return Flow::Running;
        }
        let entry = (cursor.pc, cursor.state);
        let frame = *self
            .session
            .call_frames()
            .last()
            .expect("a call pushed a frame");
        let known = match self.subroutines.get(&entry) {
            // A rollback past its function undid it: trace it again.
            Some(Subroutine::Function(summary, ..)) if !self.session.has_subroutine(summary) => {
                None
            }
            known => known,
        };
        let summary = match known {
            // Its code as it was traced: the same image, no byte the path's
            // histories disagree on. Otherwise the call is traced inline.
            Some(Subroutine::Function(summary, image, _)) => {
                // The same image, shared until a patch writes it: compared
                // byte by byte only when it is another copy, which the path
                // then shares instead, so the next call need not compare.
                if !cursor.split_image.is_empty() {
                    return Flow::Running;
                }
                if !Arc::ptr_eq(image, &cursor.image) {
                    if **image != *cursor.image {
                        return Flow::Running;
                    }
                    cursor.image = image.clone();
                }
                summary.clone()
            }
            Some(Subroutine::Inline) => return Flow::Running,
            None => {
                self.subroutines.insert(entry, Subroutine::Inline);
                match self.summarize(cursor, depth) {
                    Ok(summary) => {
                        self.subroutines.insert(
                            entry,
                            Subroutine::Function(
                                summary.clone(),
                                cursor.image.clone(),
                                std::mem::take(&mut self.summary_records).into(),
                            ),
                        );
                        summary
                    }
                    // A call that stopped decides nothing for the others.
                    Err(true) => {
                        self.subroutines.remove(&entry);
                        return Flow::Running;
                    }
                    Err(false) => return Flow::Running,
                }
            }
        };
        let tag = self.calls.tag(site);
        if self.preparation.limits.self_check && self.summarizing == 0 {
            self.check_call(cursor, depth, &summary, tag, entry);
        }
        if self.session.call_subroutine(&summary, tag).is_ok() {
            cursor.pc = frame.return_byte_offset;
            cursor.state = frame.return_dispatch_state;
        }
        Flow::Running
    }

    /// Self-check: trace the call at the cursor inline in a sandbox and
    /// compare where it returns, the stack length and every concrete cell
    /// with the outlined call. Inline may know more values than the
    /// function returns; a value both know must agree.
    pub(super) fn check_call(
        &mut self,
        cursor: &Cursor,
        depth: usize,
        summary: &SubroutineSummary,
        tag: usize,
        entry: (usize, usize),
    ) {
        let checkpoint = self.session.checkpoint();
        let (records, patches, steps) = (self.records.len(), self.patches.len(), self.steps);
        let (loops, targets, last_marks) = (
            self.loops.len(),
            self.targets.len(),
            self.last_marks.clone(),
        );
        let mut inline = cursor.clone();
        let mut ended = false;
        self.summarizing += 1;
        while self.session.call_frames().len() > depth {
            match self.advance(&mut inline) {
                Ok(Flow::Running) => {}
                _ => {
                    ended = true;
                    break;
                }
            }
        }
        self.summarizing -= 1;
        let traced = (inline.pc, inline.state, self.session.cell_facts());
        self.session.rollback(checkpoint.clone());
        let applied = self.session.call_subroutine(summary, tag).is_ok();
        let frame = (cursor.pc, cursor.state);
        let outlined = self.session.cell_facts();
        let live = self.records.live();
        self.session.rollback(checkpoint);
        self.records.truncate(records);
        self.patches.truncate(patches);
        self.charge("self-check: a call traced inline", live);
        self.steps = steps;
        self.loops.truncate(loops);
        self.targets.truncate(targets);
        self.last_marks = last_marks;
        if ended || !applied {
            // Inline it does not simply return, or the call does not fit:
            // the walk traces it inline anyway.
            return;
        }
        let returned = self
            .session
            .call_frames()
            .last()
            .map(|frame| (frame.return_byte_offset, frame.return_dispatch_state));
        let mut problems = Vec::new();
        if returned != Some((traced.0, traced.1)) {
            problems.push(format!("returns to {}/{} inline", traced.0, traced.1));
        }
        if traced.2 .0 != outlined.0 {
            problems.push(format!(
                "leaves {} stack cells inline but {} outlined",
                traced.2 .0, outlined.0
            ));
        }
        for (cell, inline_fact) in &traced.2 .1 {
            if let (Some(inline_fact), Some(Some(outlined_fact))) =
                (inline_fact, outlined.1.get(cell))
            {
                // `NaN` is the same value as itself here.
                let same = inline_fact == outlined_fact
                    || matches!(
                        (inline_fact, outlined_fact),
                        (SymbolicValue::Number(a), SymbolicValue::Number(b)) if a.is_nan() && b.is_nan()
                    );
                if !same {
                    problems.push(format!(
                        "{cell:?} is {inline_fact:?} inline but {outlined_fact:?} outlined"
                    ));
                }
            }
        }
        if !problems.is_empty() {
            self.self_check.push(format!(
                "call of subroutine {}/{} at {}/{}: {}",
                entry.0,
                entry.1,
                frame.0,
                frame.1,
                problems.join("; ")
            ));
        }
    }

    /// Trace the subroutine the cursor just entered into a function, until
    /// it returns to its caller. `Err`, with everything traced undone, when
    /// it cannot be one: it ends the VM function, stops, patches the image,
    /// or does more than a function can. `Err(true)` when it stopped: what
    /// stopped it may be the caller's state (a stack too short, on a fake
    /// branch's side), which tells nothing of other calls.
    pub(super) fn summarize(
        &mut self,
        cursor: &Cursor,
        depth: usize,
    ) -> Result<SubroutineSummary, bool> {
        let mut stopped = false;
        let checkpoint = self.session.checkpoint();
        let (records, patches) = (self.records.len(), self.patches.len());
        let frame = *self.session.call_frames().last().ok_or(false)?;
        let back: VisitKey = (
            frame.return_byte_offset,
            frame.return_dispatch_state,
            self.session.call_frames()[..depth].to_vec(),
        );
        let name = format!("vm_sub_{}_{}", cursor.pc, cursor.state);
        let opened = self.session.open_subroutine(name).ok();
        let mut body = cursor.clone();
        self.summarizing += 1;
        let returned = opened.is_some()
            && loop {
                if self.session.call_frames().len() <= depth {
                    break true;
                }
                match self.advance(&mut body) {
                    Ok(Flow::Running) => {}
                    Ok(Flow::Ended) => break false,
                    Err(halt) => {
                        stopped = matches!(halt, Halt::Stop(_));
                        break false;
                    }
                }
            };
        self.summarizing -= 1;
        let summary = (returned
            && (body.pc, body.state) == (back.0, back.1)
            && body.image == cursor.image
            && body.split_image == cursor.split_image)
            .then(|| self.session.close_subroutine(opened?).ok())
            .flatten();
        if summary.is_none() {
            let live = self.records.live();
            self.session.rollback(checkpoint);
            self.records.truncate(records);
            self.patches.truncate(patches);
            self.charge("call: not a function", live);
        } else {
            // The function is traced once and outlives any region traced
            // again: so do its records.
            self.summary_records = self.records.split_off(records);
        }
        summary.ok_or(stopped)
    }

    /// The instruction at the cursor needed one value of a variable with a
    /// known value set (a state machine's next jump): branch on its first
    /// value, `if (v === value)`. Both sides run the instruction again, one
    /// knowing the value and one knowing it is not; that one dispatches on
    /// the next value, and the last value needs no test.
    pub(super) fn dispatch(
        &mut self,
        cursor: &mut Cursor,
        at: &VisitKey,
        dispatch: ValueDispatch,
    ) -> Result<Flow, Halt> {
        if let Some(marks) = self.last_marks.take() {
            self.session.restore_stop_marks(marks);
        }
        cursor.visited.forget(at);
        for frame in &mut self.loops {
            if frame.trace.has_variable(&dispatch.name) {
                frame.dispatched = true;
            }
        }
        self.retraces.dispatches += 1;
        let value = dispatch.candidates[0].clone();
        let test = match self
            .session
            .equality_test(&dispatch.name, dispatch.cell, value.clone())
        {
            Ok(test) => test,
            Err(error) => return Err(self.unjoinable(cursor, error.0)),
        };
        let branch = PendingBranch {
            test,
            byte_offset: cursor.pc,
            dispatch_state: cursor.state,
            decided: None,
            by: BranchEvidence::Test,
        };
        self.fork(
            cursor,
            branch,
            None,
            Some((dispatch.name, dispatch.cell, value)),
        )
    }

    /// A stop on a VM value that a join merged from two different concrete
    /// values splits that join instead, unless its paths are already kept
    /// apart in that cell.
    pub(super) fn split_or(&mut self, stop: PrefixStopReason, at: VisitKey) -> Halt {
        let merged = self.session.take_split();
        let inline = self.session.take_inline();
        let dynamic = matches!(stop, PrefixStopReason::UnsupportedHandler { .. })
            && self
                .session
                .last_diagnostic()
                .is_some_and(|diagnostic| diagnostic.code == TraceStopCode::DynamicIndex);
        if let Some(tag) = inline.filter(|_| dynamic) {
            return self.inline_call(tag);
        }
        let Some(merged) = merged.filter(|_| dynamic) else {
            return stop.into();
        };
        let Some(jump) = self.joins.get(merged.join) else {
            return stop.into();
        };
        if !Arc::make_mut(&mut self.learned)
            .apart
            .entry(jump.clone())
            .or_default()
            .entry(merged.cell)
            .or_default()
            .insert(at)
        {
            return stop.into();
        }
        self.retraces.splits += 1;
        Halt::Split(jump)
    }

    /// A VM jump straight to a program value is evidence against the fork
    /// whose side made it. When the value was carried out of an `if` that
    /// has closed since, that `if` is the one: the walk starts over with its
    /// side known fake. When that cannot be done (the `if` was already
    /// forced, or is unknown), the stop is no evidence at all, so it never
    /// blames an enclosing fork that only passed the value on.
    pub(super) fn junk_carried_out(&mut self) -> Option<Halt> {
        let origin = self.session.take_junk_origin();
        let diagnostic = self.session.last_diagnostic()?;
        if diagnostic.code != TraceStopCode::ProgramValueJump {
            return None;
        }
        let message = diagnostic.message.clone();
        let (statement, taken_arm) = origin?;
        if self
            .targets
            .iter()
            .any(|target| target.statement == Some(statement))
        {
            return None;
        }
        match self.fork_jumps.get(&statement).cloned() {
            Some(jump) if !self.learned.forced.contains_key(&jump) => {
                let reason = format!(
                    "{}: {message} (carried out of this side)",
                    TraceStopCode::ProgramValueJump
                );
                Some(Halt::ForceFake(jump, taken_arm, reason))
            }
            _ => {
                self.session
                    .stop_prefix(TraceStopCode::DynamicIndex, message, None);
                None
            }
        }
    }

    /// Trace the outlined call `tag` inline from now on: the region entered
    /// before it traces itself again.
    pub(super) fn inline_call(&mut self, tag: usize) -> Halt {
        let site = self.calls.get(tag).expect("a tagged call");
        Arc::make_mut(&mut self.learned)
            .inline_calls
            .insert(site.clone());
        self.retraces.inline_calls += 1;
        Halt::Split(site)
    }

    /// The tag of the join of the fork at `jump`.
    pub(super) fn join_tag(&mut self, jump: &VisitKey) -> usize {
        self.joins.tag(jump.clone())
    }

    /// Whether the sides of the fork at `jump` still differ in a cell they
    /// must agree on before joining, one whose read is still ahead of them.
    pub(super) fn kept_apart(&self, jump: &VisitKey, a: &Side, b: &Side) -> bool {
        if self.learned.apart_bytes.get(jump).is_some_and(|bytes| {
            bytes
                .iter()
                .any(|&byte| a.cursor.image.get(byte) != b.cursor.image.get(byte))
        }) {
            return true;
        }
        let Some(cells) = self.learned.apart.get(jump) else {
            return false;
        };
        cells.iter().any(|(cell, reads)| {
            let read = reads
                .iter()
                .all(|at| a.cursor.visited.contains(at) && b.cursor.visited.contains(at));
            !read && TraceSession::cells_differ(&a.path, &b.path, &BTreeSet::from([*cell]))
        })
    }

    /// Why a stopped walk is in a state real code never reaches, if it is:
    /// the side of a fork that led there is a fake branch.
    pub(super) fn bad_state(&self, halt: &Halt) -> Option<String> {
        self.bad_state_at(halt, None)
    }

    /// [`Self::bad_state`] for a side of the fork `fork`.
    pub(super) fn bad_state_at(
        &self,
        halt: &Halt,
        fork: Option<TraceStatementId>,
    ) -> Option<String> {
        if self.no_evidence {
            return None;
        }
        let Halt::Stop(stop) = halt else {
            return None;
        };
        match stop {
            PrefixStopReason::TruncatedImage { .. }
            | PrefixStopReason::TruncatedOperand { .. }
            | PrefixStopReason::MissingState { .. }
            | PrefixStopReason::MissingOpcode { .. }
            | PrefixStopReason::AddressOutOfBounds { .. } => Some(stop.to_string()),
            PrefixStopReason::UnjoinableBranches { reason, .. } => {
                let diagnostic = self.session.last_diagnostic()?;
                // A catch's stack below its `try` is unknown: so is its
                // length.
                (diagnostic.code == TraceStopCode::PrefixLoopStackChange && self.catch_depth == 0)
                    .then(|| format!("{}: {reason}", diagnostic.code))
            }
            PrefixStopReason::UnsupportedHandler { .. } => {
                let diagnostic = self.session.last_diagnostic()?;
                // A catch starts with the stack its `try` was entered with,
                // but the VM keeps whatever the throw left above it: an index
                // below that start is unknown, not wrong.
                if self.catch_depth > 0 && diagnostic.code == TraceStopCode::InvalidIndex {
                    return None;
                }
                // Code a catch runs sets the stack's length before it reads
                // the stack the throw left: a read with none set, on a side
                // of a fork the catch made, is no code the VM runs. A fork
                // outside the catch learns nothing from it, unless the side
                // entered that `try` itself: then its catch is that side's.
                let own_catch = fork.is_some_and(|fork| {
                    self.catch_stop_try.is_some_and(|statement| statement > fork)
                });
                if (self.catch_depth > 0 || own_catch)
                    && diagnostic.code == TraceStopCode::CatchStackUnknown
                {
                    return Some(format!("{}: {}", diagnostic.code, diagnostic.message));
                }
                // A side that met the other one short of stack values reads
                // below its stack when it is the fake side of that fork: the
                // underflow is evidence for that fork, not a nested one.
                if self.short_sides > 0 && diagnostic.code == TraceStopCode::InvalidIndex {
                    return None;
                }
                // VM jumps go to constants from the bytecode, set up in the
                // VM's own slots: one straight to a program value (an
                // argument, a property, arithmetic on strings) left the code
                // the VM runs. Past a catch the slots are the tracer's guess,
                // so it is no evidence there.
                // A try depth below zero: the WASM interpreter's try entry
                // traps there, a JavaScript VM's does not.
                if diagnostic.code == TraceStopCode::TryUnderflow {
                    return (self.preparation.wasm.is_some()
                        && self.catch_depth == 0
                        && !self.session.from_catch())
                    .then(|| format!("{}: {}", diagnostic.code, diagnostic.message));
                }
                // A VM function that fails before its first branch fails on
                // its own bytecode, whatever the path that made it: past a
                // catch too.
                if diagnostic.code == TraceStopCode::JunkFunctionEntry {
                    return Some(format!("{}: {}", diagnostic.code, diagnostic.message));
                }
                // An operand from the bytecode indexes the table: past its
                // end, the bytes are none the VM runs as code.
                if diagnostic.code == TraceStopCode::ConstantTableIndex {
                    return Some(format!("{}: {}", diagnostic.code, diagnostic.message));
                }
                if matches!(
                    diagnostic.code,
                    TraceStopCode::ProgramValueJump
                        | TraceStopCode::PrimitiveCall
                        | TraceStopCode::WasmTrap
                ) {
                    return (self.catch_depth == 0 && !self.session.from_catch())
                        .then(|| format!("{}: {}", diagnostic.code, diagnostic.message));
                }
                matches!(
                    diagnostic.code,
                    TraceStopCode::InvalidIndex
                        | TraceStopCode::EmptyCallFrameStack
                        | TraceStopCode::EmptyExceptionStack
                )
                .then(|| format!("{}: {}", diagnostic.code, diagnostic.message))
            }
            _ => None,
        }
    }
}
