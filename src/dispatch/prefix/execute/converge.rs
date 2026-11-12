use super::*;

impl Walk<'_> {
    /// Whether an enclosing fork's stepping side meets the other one at `at`.
    /// Walk the side a branch decided fake by test rules out, from `dead`,
    /// alone, and undo it: what it reaches says whether the decision held.
    pub(super) fn audit_fake(&mut self, cursor: &Cursor, dead: (usize, usize), at: String) {
        const AUDIT_STEPS: usize = 2_000;
        let checkpoint = self.session.checkpoint();
        let facts = self.session.learned_facts();
        let wasted = self.wasted.clone();

        let (records, patches, steps) = (self.records.len(), self.patches.len(), self.steps);
        let (learned, subroutines) = (self.learned.clone(), self.subroutines.clone());
        let (loops, targets) = (self.loops.len(), self.targets.len());
        let (meets, inline_calls) = (self.calls.len(), self.joins.len());
        let kept = (
            self.fork_jumps.clone(),
            self.used_sets.clone(),
            self.last_marks.clone(),
            self.retraces,
            (self.summarizing, self.catch_depth, self.short_sides),
        );
        let dead = Cursor {
            pc: dead.0,
            state: dead.1,
            ..cursor.clone()
        };
        let mut side = dead.clone();
        // The bound counts every walk of the side, so retraces end too.
        self.audit_until = Some(steps + AUDIT_STEPS);
        let outcome = loop {
            match self.advance(&mut side) {
                Ok(Flow::Running) => {}
                Ok(Flow::Ended) => break "runs: ends (returns, throws or continues a loop)".into(),
                Err(Halt::Stop(PrefixStopReason::Budget)) if self.steps >= steps + AUDIT_STEPS => {
                    break format!(
                        "bound: {} restarts, {} positions, a position run {} times",
                        self.audit_restarts,
                        self.audit_seen.len(),
                        self.audit_seen.values().max().unwrap_or(&0)
                    )
                }
                // As at the top of the walk: start the side again, knowing
                // the new loop header or the forced branch.
                Err(Halt::NewLoop(_) | Halt::Split(_)) => self.restart_audit(
                    checkpoint.clone(),
                    (records, patches, loops, targets),
                    &dead,
                    &mut side,
                ),
                Err(Halt::ForceFake(jump, taken_fake, reason)) => {
                    Arc::make_mut(&mut self.learned)
                        .forced
                        .insert(jump, (taken_fake, reason));
                    self.restart_audit(
                        checkpoint.clone(),
                        (records, patches, loops, targets),
                        &dead,
                        &mut side,
                    )
                }
                Err(halt) => {
                    break match self.bad_state(&halt) {
                        Some(reason) => format!("confirmed: {reason}"),
                        None => format!(
                            "unknown: stops ({})",
                            match &halt {
                                Halt::Stop(stop) => stop.to_string(),
                                Halt::Cells(_) => "new shared cells".into(),
                                Halt::Rewiden(_) => "a loop widened again".into(),
                                Halt::TryMeet(_) => "a try meeting point".into(),
                                Halt::NotFunction => "a call that is not a function".into(),
                                _ => "a retrace".into(),
                            }
                        ),
                    }
                }
            }
        };
        self.audit_until = None;
        self.audit_seen.clear();
        self.audit_restarts = 0;
        self.session.rollback(checkpoint);
        self.session.forget(facts);
        self.wasted = wasted;

        self.records.truncate(records);
        self.patches.truncate(patches);
        self.steps = steps;
        self.learned = learned;
        self.subroutines = subroutines;
        self.loops.truncate(loops);
        self.targets.truncate(targets);
        self.calls.truncate(meets);
        self.joins.truncate(inline_calls);
        let (fork_jumps, used_sets, last_marks, retraces, counts) = kept;
        (
            self.fork_jumps,
            self.used_sets,
            self.last_marks,
            self.retraces,
        ) = (fork_jumps, used_sets, last_marks, retraces);
        (self.summarizing, self.catch_depth, self.short_sides) = counts;
        self.fake_audits.entry(at).or_default().insert(outcome);
    }

    /// Undo an audited side's walk, keeping what it learned, and stand it at
    /// the dead side's start again.
    pub(super) fn restart_audit(
        &mut self,
        checkpoint: crate::tracing::symbolic::TraceCheckpoint,
        (records, patches, loops, targets): (usize, usize, usize, usize),
        dead: &Cursor,
        side: &mut Cursor,
    ) {
        self.session.rollback(checkpoint);
        self.records.truncate(records);
        self.patches.truncate(patches);
        self.loops.truncate(loops);
        self.targets.truncate(targets);
        *side = dead.clone();
        self.audit_seen.clear();
        self.audit_restarts += 1;
    }

    /// Which of `causes` (a loop, a split, a call traced inline) made a
    /// region trace itself again.
    pub(super) fn restart_cause(&self, halt: &Halt, causes: [&'static str; 3]) -> &'static str {
        match halt {
            Halt::NewLoop(_) => causes[0],
            Halt::Split(at) if self.learned.inline_calls.contains(at) => causes[2],
            _ => causes[1],
        }
    }

    /// Count the steps a discard threw away: the records kept before it,
    /// `live`, less those kept now.
    pub(super) fn charge(&mut self, cause: &'static str, live: usize) {
        let wasted = live.saturating_sub(self.records.live());
        if wasted > 0 {
            *self.wasted.entry(cause).or_default() += wasted;
        }
    }

    /// Where `side` stands now, to cut it back to; its `made` holds where the
    /// trace stands, for [`Self::made_since`] once the step ran.
    pub(super) fn begin_step(&self, side: &Side) -> SideStep {
        let now = |len: usize| len..len;
        SideStep {
            at: position(side),
            path: side.path.clone(),
            block_length: self.session.block_length(&side.path),
            pc: side.cursor.pc,
            state: side.cursor.state,
            split_image: side.cursor.split_image.clone(),
            visited: side.cursor.visited.mark(),
            turn: 0,
            stops: self.session.stop_marks(),
            made: Made {
                statements: now(self.session.statement_count()),
                events: now(self.session.event_count()),
                records: now(self.records.len()),
                patches: now(self.patches.len()),
            },
        }
    }

    /// What the trace made since `start`.
    pub(super) fn made_since(&self, start: &Made) -> Made {
        Made {
            statements: start.statements.start..self.session.statement_count(),
            events: start.events.start..self.session.event_count(),
            records: start.records.start..self.records.len(),
            patches: start.patches.start..self.patches.len(),
        }
    }

    /// Put `side` back to where it stood before its step `index`, dropping
    /// what that step and every later one made, and `failed`, what a step
    /// that stopped made.
    pub(super) fn cut_back(
        &mut self,
        side: &mut Side,
        index: usize,
        failed: Option<Made>,
        cause: &'static str,
    ) {
        let live = self.records.live();
        let steps: Vec<SideStep> = side.history.drain(index..).collect();
        let made: Vec<Made> = steps
            .iter()
            .map(|step| step.made.clone())
            .chain(failed)
            .collect();
        let statements: Vec<_> = made.iter().map(|made| made.statements.clone()).collect();
        let events: Vec<_> = made.iter().map(|made| made.events.clone()).collect();
        // The image as it was: every patch since, undone latest first. Each
        // was made on an image that came from this one, nested sides' too,
        // so each address ends with the value it had.
        let undo: Vec<(usize, u8)> = made
            .iter()
            .rev()
            .flat_map(|made| made.patches.clone().rev())
            .filter(|&index| index < self.patches.len())
            .map(|index| {
                let patch = &self.patches[index];
                (patch.address as usize, patch.previous)
            })
            .collect();
        if !undo.is_empty() {
            let image = Arc::make_mut(&mut side.cursor.image);
            for (address, previous) in undo {
                if let Some(byte) = image.get_mut(address) {
                    *byte = previous;
                }
            }
        }
        for made in &made {
            self.records.drop_range(made.records.clone());
            self.patches.drop_range(made.patches.clone());
        }
        let back = steps.into_iter().next().expect("a step to go back to");
        self.session.cut_back(
            &mut side.path,
            &back.path,
            back.block_length,
            &statements,
            &events,
        );
        side.cursor.pc = back.pc;
        side.cursor.state = back.state;
        side.cursor.split_image = back.split_image;
        side.cursor.visited.rewind(back.visited);
        side.ended = false;
        self.charge(cause, live);
    }

    /// The step of `side` during which it first passed `at`, if it did since
    /// its history began: cut back there, the side traces `at` again.
    pub(super) fn step_reaching(side: &Side, at: &VisitKey) -> Option<usize> {
        let passed = |index: usize| match side.history.get(index) {
            Some(step) => side.cursor.visited.contains_at(step.visited, at),
            None => side.cursor.visited.contains(at),
        };
        if side.history.is_empty() || passed(0) || !passed(side.history.len()) {
            return None;
        }
        // The first step after which the side had passed it.
        let (mut low, mut high) = (1, side.history.len());
        while low < high {
            let middle = (low + high) / 2;
            if passed(middle) {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        Some(low - 1)
    }

    pub(super) fn at_target(&self, at: &VisitKey) -> bool {
        self.targets.iter().any(|target| target.reaches(at))
    }

    /// The innermost enclosing fork (its index in `targets`) whose other
    /// side has been at `at`.
    pub(super) fn target_depth(&self, at: &VisitKey) -> Option<usize> {
        self.targets.iter().rposition(|target| target.reaches(at))
    }

    /// Step the catch path of a `try`. An index below the stack the `try` was
    /// entered with stops the walk, but is no evidence about an enclosing
    /// fork: the stack the VM keeps there is unknown.
    pub(super) fn advance_catch(&mut self, catch: &mut Side) -> Result<Flow, Halt> {
        self.catch_depth += 1;
        let result = self.advance_side(catch);
        self.catch_depth -= 1;
        if let Err(Halt::Stop(PrefixStopReason::UnsupportedHandler { .. })) = &result {
            if let Some(diagnostic) = self
                .session
                .last_diagnostic()
                .filter(|diagnostic| diagnostic.code == TraceStopCode::InvalidIndex)
            {
                let message = format!(
                    "the catch at pc {} state {} reads below the stack its try was entered with: {}",
                    catch.cursor.pc, catch.cursor.state, diagnostic.message
                );
                self.session
                    .stop_prefix(TraceStopCode::CatchStackUnknown, message, None);
            }
        }
        result
    }

    /// Step a path only if the step emits no statement; otherwise undo it.
    /// Returns whether the step was kept.
    pub(super) fn advance_side_silently(&mut self, side: &mut Side) -> Result<bool, Halt> {
        self.session.switch_path(&mut side.path);
        let checkpoint = self.session.checkpoint();
        let statements = self.session.statement_count();
        let (records, patches) = (self.records.len(), self.patches.len());
        let cursor = side.cursor.clone();
        let result = self.advance(&mut side.cursor);
        let kept = match result {
            Ok(_) if !self.session.only_pure_bindings_since(statements) => {
                let live = self.records.live();
                self.session.rollback(checkpoint);
                self.records.truncate(records);
                self.patches.truncate(patches);
                self.charge("try: a closed body's step undone", live);
                side.cursor = cursor;
                Ok(false)
            }
            Ok(flow) => {
                side.ended = flow == Flow::Ended;
                Ok(true)
            }
            Err(halt) => Err(halt),
        };
        self.session.switch_path(&mut side.path);
        kept
    }

    /// Stop because two paths cannot merge at the cursor; on a path a join
    /// padded, trace that join's fork again with its sides apart.
    pub(super) fn unjoinable(&mut self, cursor: &Cursor, reason: String) -> Halt {
        if let Some(halt) = self.pad_conflict() {
            return halt;
        }
        let (pc, state) = (cursor.pc, cursor.state);
        self.session.stop_prefix(
            TraceStopCode::PrefixBranchJoin,
            format!("paths cannot join at pc {pc} state {state}: {reason}"),
            None,
        );
        PrefixStopReason::UnjoinableBranches { pc, state, reason }.into()
    }

    /// The join that padded the running path's stack lined it up wrongly
    /// (see `TraceSession::take_pad_conflict`): its fork is traced again,
    /// its sides kept apart as stacks of different lengths are.
    pub(super) fn pad_conflict(&mut self) -> Option<Halt> {
        let (join, consumed) = self.session.take_pad_conflict()?;
        let jump = self.joins.get(join)?;
        if self.learned.unpadded.contains(&jump) {
            self.retraces.splits += 1;
            return Some(Halt::Restart);
        }
        if consumed {
            if let Some(halt) = self.short_side_fake(&jump) {
                return Some(halt);
            }
        }
        // A deficit lined up in the holes of a join under a loop variable
        // gives no evidence there: that join is undone instead, so this one
        // is lined up alone and gives it.
        if self.deficit_joins.contains(&jump) {
            let outer = self
                .padded
                .iter()
                .rev()
                .map(|(padded, ..)| padded)
                .find(|padded| {
                    **padded != jump
                        && self.loop_variable_joins.contains(*padded)
                        && !self.learned.unpadded.contains(*padded)
                })
                .cloned();
            if let Some(halt) = outer.and_then(|outer| self.unpad(outer)) {
                return Some(halt);
            }
        }
        self.unpad(jump)
    }

    /// The code after the fork at `jump`, run on both its sides lined up
    /// from the top, used more stack values than the shorter side holds: on
    /// that side it reads under an empty stack, which code that runs never
    /// does. The walk goes back to what it knew when the sides joined and
    /// follows the longer side only.
    pub(super) fn short_side_fake(&mut self, jump: &VisitKey) -> Option<Halt> {
        if self.learned.forced.contains_key(jump) {
            return None;
        }
        let (then, taken_short) = self
            .padded
            .iter()
            .rev()
            .find(|(padded, ..)| padded == jump)
            .map(|(_, then, taken_short)| (then.clone(), *taken_short))?;
        let unpadded = self.learned.unpadded.clone();
        self.learned = then;
        let learned = Arc::make_mut(&mut self.learned);
        learned.unpadded.extend(unpadded);
        learned.forced.insert(
            jump.clone(),
            (
                taken_short,
                "stack-deficit: the code after the branch uses more stack values than this side holds"
                    .into(),
            ),
        );
        self.retraces.splits += 1;
        Some(Halt::Split(jump.clone()))
    }

    /// Undo the join of the fork at `jump` that lined its sides' stacks up:
    /// the walk goes back to what it had learned when they joined, knowing
    /// only that this fork's sides do not join so (and the others found
    /// since), and traces the fork again. What it learned on the padded
    /// path (meeting points, loops, breaks) might not hold without it.
    pub(super) fn unpad(&mut self, jump: VisitKey) -> Option<Halt> {
        if self.learned.unpadded.contains(&jump) {
            return None;
        }
        let unpadded = self.learned.unpadded.clone();
        if let Some((_, then, _)) = self.padded.iter().rev().find(|(padded, ..)| *padded == jump) {
            self.learned = then.clone();
        }
        let learned = Arc::make_mut(&mut self.learned);
        learned.unpadded.extend(unpadded);
        learned.unpadded.insert(jump.clone());
        self.retraces.splits += 1;
        Some(Halt::Split(jump))
    }

    /// Before a loop's changed stack length stands, as a stop or as
    /// evidence that a fork's side is fake: a join that lined two stacks up
    /// may have caused it, so the most recent one not yet undone is undone
    /// first, and its fork traced again with its sides apart.
    pub(super) fn undo_padding(&mut self) -> Option<Halt> {
        let jump = self
            .padded
            .iter()
            .rev()
            .map(|(jump, ..)| jump)
            .find(|jump| !self.learned.unpadded.contains(*jump))?
            .clone();
        self.unpad(jump)
    }

    /// The same for a `continue` or `break` on the padded path of a loop
    /// entered before the holes went in (see
    /// `TraceSession::loop_before_padding`): its variables are stack cells
    /// counted without them.
    pub(super) fn padded_loop(&mut self) -> Option<Halt> {
        let join = self.session.stack_pad_join()?;
        let jump = self.joins.get(join)?;
        self.unpad(jump)
    }

    /// Trace both successors of `branch` until they meet, then continue with
    /// the merged path in `cursor`. A branch that ends (returns or continues
    /// a loop) stops there; the other one then runs on alone, and the path
    /// ends when both ended. A side that reaches a bad state makes the jump a
    /// fake branch: the walk returns to the jump and follows the other side.
    ///
    /// The sides normally meet because the lower PC catches up. Code laid out
    /// out of line breaks that: a `then` block placed after the function body
    /// jumps back to just after the jump, behind the other side. When one
    /// side reaches a position the other already passed since the fork, that
    /// position is where they meet: the fork is traced again with each side
    /// stopping there.
    pub(super) fn fork(
        &mut self,
        cursor: &mut Cursor,
        branch: PendingBranch,
        record: Option<usize>,
        assume: Option<(String, Option<CellRead>, SymbolicValue)>,
    ) -> Result<Flow, Halt> {
        if self.fork_depth >= MAX_FORK_DEPTH {
            if !self.patient {
                self.patient = true;
                return Err(Halt::Restart);
            }
            return Err(self.unjoinable(
                cursor,
                format!("forks nest more than {MAX_FORK_DEPTH} deep"),
            ));
        }
        self.fork_depth += 1;
        let real = self.real_override(record);
        let flow = self.fork_sides(cursor, branch, record, assume);
        // A stop that leaves a branch the user made real is evidence for no
        // enclosing branch either: it would only move the verdict outward.
        if real && matches!(flow, Err(Halt::Stop(_))) {
            self.no_evidence = true;
        }
        self.fork_depth -= 1;
        flow
    }

    /// Whether the user made the branch of `record` real (see
    /// [`crate::BranchOverrides::real`]).
    pub(super) fn real_override(&self, record: Option<usize>) -> bool {
        record.is_some_and(|record| {
            let record = &self.records[record];
            self.overrides
                .real
                .contains(&(record.dispatch_pc, record.state_before))
        })
    }

    pub(super) fn fork_sides(
        &mut self,
        cursor: &mut Cursor,
        branch: PendingBranch,
        record: Option<usize>,
        assume: Option<(String, Option<CellRead>, SymbolicValue)>,
    ) -> Result<Flow, Halt> {
        let real = self.real_override(record);
        let checkpoint = self.session.checkpoint();
        let (records, patches) = (self.records.len(), self.patches.len());
        let restore = |walk: &mut Self, cause| {
            let live = walk.records.live();
            walk.session.rollback(checkpoint.clone());
            walk.records.truncate(records);
            walk.patches.truncate(patches);
            walk.charge(cause, live);
        };
        let jump = (cursor.pc, cursor.state, self.session.call_frames().to_vec());
        let mut meet = self.learned.meets.get(&jump).cloned();
        // A side found to carry a program value to a VM jump after the fork
        // closed: only the other side is real code.
        if let (Some((taken_fake, reason)), None, false) =
            (self.learned.forced.get(&jump).cloned(), &assume, real)
        {
            let taken = !taken_fake;
            self.session.record_fake_branch(branch.test, taken);
            if let Some(PrefixStepEffect::Handler {
                branch: Some(branch),
                ..
            }) = record.map(|record| &mut self.records[record].effect)
            {
                branch.kind = PrefixBranchKind::FakeByContext { taken, reason };
            }
            if taken {
                cursor.pc = branch.byte_offset;
                cursor.state = branch.dispatch_state;
            }
            return Ok(Flow::Running);
        }
        cursor.visited.freeze();
        // Positions where the sides met in VM states that cannot merge
        // (stack lengths, image patches, try or exception states): they step
        // on past them, to a later meeting point or each to its end.
        let mut unmergeable: BTreeSet<VisitKey> = BTreeSet::new();
        let mut apart_since: Option<(usize, Cursor, String)> = None;
        'attempt: loop {
            let (fork, taken, fall_through) = self.session.fork(branch.test);
            self.fork_jumps.insert(fork.statement, jump.clone());
            let mut taken = Side {
                cursor: Cursor {
                    pc: branch.byte_offset,
                    state: branch.dispatch_state,
                    ..cursor.clone()
                },
                path: taken,
                ended: false,
                history: Vec::new(),
                held: false,
            };
            let mut fall_through = Side {
                cursor: cursor.clone(),
                path: fall_through,
                ended: false,
                history: Vec::new(),
                held: false,
            };
            // A dispatch: each side runs the instruction again, knowing
            // which value it has, before the sides can meet.
            let mut prestep = 0;
            if let Some((name, cell, value)) = &assume {
                TraceSession::assume(&mut taken.path, name, *cell, value.clone());
                TraceSession::exclude(&mut fall_through.path, name, *cell, value.clone());
                prestep = 2;
            }
            // Past a position where they could not merge, the sides run the
            // same code a step apart: every position looks like a crossing.
            let mut stepped_apart = false;
            // Which side met the other with fewer stack values, if they met
            // with different stack lengths: true for the taken one.
            let mut short_side: Option<bool> = None;
            // Sides that met with stacks differing only in what they hold
            // above a part both share, lined up from the top when they join:
            // the shorter (true: the taken one), where that part ends and by
            // how many values.
            let padding: Option<StackPadding>;
            let base = cursor.visited.layers.len();
            let mut turn = 0;
            // What the fork knew at the start of each turn, to go back to
            // with both sides.
            let mut turns: Vec<TurnState> = Vec::new();
            loop {
                turn += 1;
                turns.push(TurnState {
                    meet: meet.clone(),
                    unmergeable: unmergeable.clone(),
                    apart_since: apart_since.clone(),
                    stepped_apart,
                    short_side,
                    prestep,
                });
                // A side running alone stays in its arm, so a bad state it
                // reaches still makes this a fake branch, until it stands
                // where an enclosing fork meets it: there it carries on
                // after the `if`.
                if taken.ended != fall_through.ended {
                    let (running, ended) = if taken.ended {
                        (&mut fall_through, &taken)
                    } else {
                        (&mut taken, &fall_through)
                    };
                    if self.at_target(&position(running)) {
                        if let Err(error) =
                            self.session
                                .continue_alone(&fork, &mut running.path, &ended.path)
                        {
                            return Err(self.unjoinable(&running.cursor, error.0));
                        }
                        self.session.switch_path(&mut running.path);
                        *cursor = running.cursor.clone();
                        return Ok(Flow::Running);
                    }
                }
                if let Some((since, at, reason)) = &apart_since {
                    if self.steps - since > MAX_APART_STEPS {
                        let (at, reason) = (at.clone(), reason.clone());
                        return Err(self.unjoinable(&at, reason));
                    }
                }
                let (a, b) = (&taken, &fall_through);
                let (at_a, at_b) = (position(a), position(b));
                // Sides that set up different jumps do not meet until the
                // jumps are taken and the cells agree again.
                let apart = self.kept_apart(&jump, a, b);
                if meet.is_none() && !apart && prestep == 0 {
                    // A running side standing where the other has been since
                    // the fork, even one that has ended since, found the
                    // meeting point.
                    // With the side that was there first: true for the
                    // taken one.
                    // Both crossing in one turn: the lower position, so the
                    // order the sides stepped in decides nothing.
                    let crossed = [(&at_a, a, b, false), (&at_b, b, a, true)]
                        .into_iter()
                        .filter_map(|(at, side, other, other_taken)| {
                            // A dispatch's own instruction, which its sides
                            // ran again, is no meeting point: back there, a
                            // side goes round a loop.
                            (!side.ended
                                && (assume.is_none() || *at != jump)
                                && other.cursor.visited.contains(at)
                                && !cursor.visited.contains(at)
                                && !unmergeable.contains(at)
                                // Past a position where they could not merge,
                                // only where the other stood with the stack
                                // this side has now: elsewhere they run the
                                // same code a step apart.
                                && (!stepped_apart
                                    || other.history.iter().find(|step| step.at == *at).is_some_and(
                                        |step| {
                                            TraceSession::stack_length_of(&step.path)
                                                == TraceSession::stack_length_of(&side.path)
                                        },
                                    )))
                            .then(|| (at.clone(), other_taken))
                        })
                        .min();
                    if let Some((at, other_taken)) = crossed {
                        self.retraces.meeting_points += 1;
                        Arc::make_mut(&mut self.learned)
                            .meets
                            .insert(jump.clone(), at.clone());
                        // The side that was there first goes back to the step
                        // that reached it, between two of its steps or inside
                        // one (a loop it then leaves there), and what the
                        // other side traced stays.
                        let other = if other_taken {
                            &mut taken
                        } else {
                            &mut fall_through
                        };
                        let reaching = other
                            .history
                            .iter()
                            .position(|step| step.at == at)
                            .or_else(|| Self::step_reaching(other, &at));
                        if let Some(index) = reaching {
                            self.cut_back(other, index, None, "fork: past the meeting point");
                            meet = Some(at);
                            continue;
                        }
                        restore(self, "fork: a meeting point found again");
                        meet = Some(at);
                        continue 'attempt;
                    }
                }
                // Which sides step this turn: both, one step each, except one
                // that ended or waits at the meeting point.
                let rerun = prestep > 0;
                let stepping = match (a.ended, b.ended) {
                    (true, true) => {
                        // Where the sides went, for an enclosing fork to
                        // meet.
                        cursor.visited = Visited::join(
                            &cursor.visited,
                            [
                                std::mem::take(&mut taken.cursor.visited),
                                std::mem::take(&mut fall_through.cursor.visited),
                            ],
                        );
                        return Ok(Flow::Ended);
                    }
                    // A dispatch: both run the instruction they stand on.
                    _ if rerun => {
                        prestep = 0;
                        [!a.ended, !b.ended]
                    }
                    (true, false) => [false, true],
                    (false, true) => [true, false],
                    // At an enclosing loop's header each side continues the
                    // loop with its own values.
                    (false, false)
                        if at_a == at_b && self.loops.iter().any(|frame| frame.header == at_a) =>
                    {
                        [true, true]
                    }
                    (false, false) => {
                        if at_a == at_b {
                            // An image the sides patched differently does
                            // not keep them apart: the join records the
                            // bytes they disagree on.
                            let conflict = TraceSession::join_conflict(&a.path, &b.path);
                            // Stacks that differ only in length (one arm left
                            // a value it never pops): lined up from the top,
                            // unless that went wrong here before.
                            let padded = conflict
                                .as_ref()
                                .and_then(|_| {
                                    TraceSession::stack_padding(&a.path, &b.path, &checkpoint)
                                })
                                .filter(|_| {
                                    // A loop entered at two depths stays two
                                    // loops, each keeping its own.
                                    self.summarizing == 0
                                        && !self.learned.unpadded.contains(&jump)
                                        // A real branch's sides each run on
                                        // their own stack: no deficit
                                        // decides it.
                                        && !real
                                        && !self.learned.headers.contains(&at_a)
                                });
                            if !apart && (conflict.is_none() || padded.is_some()) {
                                padding = padded;
                                break;
                            }
                            // Kept apart at the meeting point: step on,
                            // looking for a later one.
                            if let Some(reason) = conflict {
                                stepped_apart = true;
                                let lengths = (
                                    TraceSession::stack_length_of(&a.path),
                                    TraceSession::stack_length_of(&b.path),
                                );
                                if lengths.0 != lengths.1 {
                                    short_side = Some(lengths.0 < lengths.1);
                                }
                                if unmergeable.insert(at_a.clone()) {
                                    self.retraces.unmergeable += 1;
                                }
                                if apart_since.is_none() {
                                    apart_since = Some((self.steps, a.cursor.clone(), reason));
                                }
                            }
                            meet = None;
                        }
                        match &meet {
                            // A side at the meeting point waits for the other.
                            Some(at) if *at == at_a => [false, true],
                            Some(at) if *at == at_b => [true, false],
                            // A side where an enclosing fork's other side has
                            // been leaves this fork there: it waits, and
                            // carries on after the `if` once the other ends.
                            // Each at a different enclosing fork's: the
                            // one at the innermost fork's goes on (to that
                            // fork's end, close by), the other waits. Both
                            // going on would trace what follows the outer
                            // fork's meeting point in this `if`'s arm.
                            _ => match (self.target_depth(&at_a), self.target_depth(&at_b)) {
                                (Some(_), None) => [false, true],
                                (None, Some(_)) => [true, false],
                                (Some(a), Some(b)) if a > b => [true, false],
                                (Some(a), Some(b)) if a < b => [false, true],
                                _ => [true, true],
                            },
                        }
                    }
                };
                // A side held at a fork waits for the other; two stand at
                // forks with no meeting point between them, and both go on.
                let mut stepping = stepping;
                let mut may_hold = self.patient && meet.is_none() && !rerun;
                match (taken.held, fall_through.held) {
                    (true, false) if stepping[1] && may_hold => stepping[0] = false,
                    (false, true) if stepping[0] && may_hold => stepping[1] = false,
                    (false, false) => {}
                    _ => {
                        may_hold = false;
                        taken.held = false;
                        fall_through.held = false;
                    }
                }
                // Each side sees the other as it stood when the turn began,
                // so which of them runs first decides nothing.
                let marks = [
                    taken.cursor.visited.mark(),
                    fall_through.cursor.visited.mark(),
                ];
                let order = if self.preparation.limits.reverse_sides {
                    [false, true]
                } else {
                    [true, false]
                };
                let mut results: [Option<(Result<Flow, Halt>, SideStep)>; 2] = [None, None];
                // For a side that halted: why its state is one real code
                // never reaches, if it is, and the session's stop state.
                let mut stops: [Option<(Option<String>, StopMarks)>; 2] = [None, None];
                // Each side starts from what the walk had learned when the
                // turn began.
                let start = self.learned.clone();
                let mut learned = [start.clone(), start.clone()];
                for is_taken in order {
                    if !stepping[usize::from(!is_taken)] {
                        continue;
                    }
                    self.learned = start.clone();
                    let (side, other) = if is_taken {
                        (&mut taken, &mut fall_through)
                    } else {
                        (&mut fall_through, &mut taken)
                    };
                    // Lend the other side's positions to nested forks.
                    self.targets.push(Target {
                        meet: meet.clone(),
                        other: std::mem::take(&mut other.cursor.visited),
                        base,
                        until: Some(marks[usize::from(is_taken)]),
                        statement: Some(fork.statement),
                    });
                    // A dispatch's sides run the instruction they stand on,
                    // even a loop header's: it is not a `continue`.
                    let short = short_side == Some(is_taken);
                    self.short_sides += usize::from(short);
                    let mut step = self.begin_step(side);
                    step.turn = turn;
                    // A side running alone, the other ended, is one path:
                    // the forks it opens nest no wider than it.
                    let alone = other.ended;
                    self.fork_depth -= usize::from(alone);
                    self.may_hold = may_hold && stepping[usize::from(is_taken)];
                    let result = if rerun {
                        self.session.switch_path(&mut side.path);
                        let result = self.step_once(&mut side.cursor);
                        self.session.switch_path(&mut side.path);
                        result
                    } else {
                        self.advance_side(side)
                    };
                    self.may_hold = false;
                    self.fork_depth += usize::from(alone);
                    self.short_sides -= usize::from(short);
                    other.cursor.visited = self.targets.pop().expect("pushed above").other;
                    step.made = self.made_since(&step.made);
                    // A side that stopped leaves the session stopped: the
                    // other steps from where the session was before.
                    if let Err(halt) = &result {
                        let mut reason = self.bad_state_at(halt, Some(fork.statement));
                        // A side of a branch the user made real that reaches
                        // a state real code never reaches stops the walk.
                        if real && reason.is_some() {
                            self.no_evidence = true;
                            reason = None;
                        }
                        stops[usize::from(!is_taken)] = Some((reason, self.session.stop_marks()));
                        self.session.resume(&step.stops);
                    }
                    results[usize::from(!is_taken)] = Some((result, step));
                    learned[usize::from(!is_taken)] = self.learned.clone();
                }
                let [learned_taken, learned_fall] = learned;
                self.learned = Learned::merged(&start, learned_taken, learned_fall);
                // The taken side's result first, whichever ran first.
                let mut halts = Vec::new();
                let mut stepped = [false; 2];
                for (index, result) in results.into_iter().enumerate() {
                    let Some((result, step)) = result else {
                        continue;
                    };
                    let side = if index == 0 {
                        &mut taken
                    } else {
                        &mut fall_through
                    };
                    match result {
                        Ok(flow) => {
                            side.ended = flow == Flow::Ended;
                            side.history.push(step);
                            stepped[index] = true;
                        }
                        Err(Halt::Held) => {
                            side.history.push(step);
                            let last = side.history.len() - 1;
                            self.cut_back(side, last, None, "fork: a side held at a fork");
                            side.held = true;
                        }
                        Err(halt) => {
                            let (reason, marks) =
                                stops[index].take().expect("recorded with the halt");
                            halts.push((index == 0, halt, step, reason, marks));
                        }
                    }
                }
                // A side halted: the other side's step this turn is undone,
                // whichever ran first.
                if !halts.is_empty() {
                    for (index, side) in [&mut taken, &mut fall_through].into_iter().enumerate() {
                        if stepped[index] {
                            let last = side.history.len() - 1;
                            self.cut_back(side, last, None, "fork: the other side halted");
                        }
                    }
                }
                // A loop whose header the path passed before the fork is
                // traced again outside it, which redoes everything here: it
                // goes first. Handled after a halt this fork takes back, it
                // would be lost, its header already known as one passed.
                halts.sort_by_key(|(_, halt, ..)| {
                    !matches!(halt, Halt::NewLoop(header) if cursor.visited.contains(header))
                });
                let mut halts = halts.into_iter();
                let Some((is_taken, halt, step, reason, marks)) = halts.next() else {
                    continue;
                };
                // The other side halted too: its step is undone as well; it
                // halts again when it steps again, if it still does.
                for (other_taken, _, step, _, _) in halts {
                    let side = if other_taken {
                        &mut taken
                    } else {
                        &mut fall_through
                    };
                    side.history.push(step);
                    let last = side.history.len() - 1;
                    self.cut_back(side, last, None, "fork: both sides halted");
                }
                self.session.restore_stop_marks(marks);
                let (side, other) = if is_taken {
                    (&mut taken, &mut fall_through)
                } else {
                    (&mut fall_through, &mut taken)
                };
                let new_loop = matches!(halt, Halt::NewLoop(_));
                // A loop, a split or a call to trace inline at a position only
                // this side passed: the side goes back to the step that
                // reached it, and traces it again from there.
                let reaching = match &halt {
                    Halt::NewLoop(at) | Halt::Split(at)
                        if !cursor.visited.contains(at) && !other.cursor.visited.contains(at) =>
                    {
                        side.history.push(step);
                        let reaching = Self::step_reaching(side, at);
                        if reaching.is_none() {
                            side.history.pop();
                        }
                        reaching
                    }
                    _ => None,
                };
                match halt {
                    Halt::NewLoop(header) | Halt::Split(header) if reaching.is_some() => {
                        let index = reaching.expect("checked by the guard");
                        self.session
                            .restore_stop_marks(side.history[index].stops.clone());
                        let cause = if new_loop {
                            "fork: back to a loop header"
                        } else if self.learned.inline_calls.contains(&header) {
                            "fork: back to a call traced inline"
                        } else {
                            "fork: back to a split"
                        };
                        // The other side's steps since that turn saw this one
                        // where it no longer goes: both go back to the start
                        // of that turn, and the fork to what it knew then, as
                        // if it were traced again from there.
                        let since = side.history[index].turn;
                        self.cut_back(side, index, None, cause);
                        if let Some(later) =
                            other.history.iter().position(|step| step.turn >= since)
                        {
                            self.cut_back(other, later, None, "fork: the other side after a cut");
                        }
                        let then = turns[since - 1].clone();
                        turns.truncate(since - 1);
                        turn = since - 1;
                        (meet, unmergeable, apart_since) =
                            (then.meet, then.unmergeable, then.apart_since);
                        (stepped_apart, short_side, prestep) =
                            (then.stepped_apart, then.short_side, then.prestep);
                        // A meeting point found since stays known: the sides
                        // merge there only if both reach it and can merge.
                        meet = meet.or_else(|| self.learned.meets.get(&jump).cloned());
                    }
                    // A loop that starts after the jump: trace the fork again.
                    Halt::NewLoop(header) | Halt::Split(header)
                        if !cursor.visited.contains(&header) =>
                    {
                        let cause = if new_loop {
                            "fork: a loop inside"
                        } else if self.learned.inline_calls.contains(&header) {
                            "fork: a call traced inline"
                        } else {
                            "fork: a split inside"
                        };
                        restore(self, cause);
                        continue 'attempt;
                    }
                    halt => {
                        let Some(reason) = reason else {
                            return Err(halt);
                        };
                        if reason.starts_with(TraceStopCode::PrefixLoopStackChange.as_str()) {
                            if let Some(halt) = self.undo_padding() {
                                return Err(halt);
                            }
                        }
                        // Only the other side is real code: start over from
                        // the jump and follow it alone, knowing what it knew.
                        restore(self, "fork: a fake side");
                        // A fake branch goes the same way however it is
                        // reached: a retrace follows the real side without
                        // running the junk one again.
                        if assume.is_none() {
                            Arc::make_mut(&mut self.learned)
                                .forced
                                .entry(jump.clone())
                                .or_insert_with(|| (is_taken, reason.clone()));
                        }
                        let taken = !is_taken;
                        if let Some((name, cell, value)) = &assume {
                            self.session.learn(name, *cell, value.clone(), taken);
                        }
                        self.session.record_fake_branch(branch.test, taken);
                        if let Some(PrefixStepEffect::Handler {
                            branch: Some(branch),
                            ..
                        }) = record.map(|record| &mut self.records[record].effect)
                        {
                            branch.kind = PrefixBranchKind::FakeByContext { taken, reason };
                        }
                        if taken {
                            cursor.pc = branch.byte_offset;
                            cursor.state = branch.dispatch_state;
                        }
                        return Ok(Flow::Running);
                    }
                }
            }
            // Both sides started from the fork's image: they can differ
            // only where a step still in their histories patched it.
            let patched: BTreeSet<usize> = [&taken, &fall_through]
                .into_iter()
                .flat_map(|side| {
                    side.history
                        .iter()
                        .flat_map(|step| step.made.patches.clone())
                })
                .filter(|&index| index < self.patches.len())
                .map(|index| self.patches[index].address as usize)
                .collect();
            for &address in &patched {
                let bytes = (
                    taken.cursor.image.get(address).copied(),
                    fall_through.cursor.image.get(address).copied(),
                );
                if bytes.0 != bytes.1 {
                    self.image_splits.insert(address, jump.clone());
                    self.split_bytes.insert(address, bytes);
                }
            }
            if let Err(reason) =
                merge_images_at(&mut taken.cursor, &fall_through.cursor, Some(&patched))
            {
                return Err(self.unjoinable(&taken.cursor, reason));
            }
            let tag = self.join_tag(&jump);
            if let Some(StackPadding {
                taken_shorter,
                start,
                count,
                earlier,
                deficit,
                loop_variable,
            }) = padding
            {
                if loop_variable && earlier.is_none() {
                    self.loop_variable_joins.insert(jump.clone());
                }
                if deficit && earlier.is_none() {
                    self.deficit_joins.insert(jump.clone());
                }
                if earlier.is_none() {
                    self.padded
                        .push((jump.clone(), self.learned.clone(), taken_shorter));
                }
                let side = if taken_shorter {
                    &mut taken
                } else {
                    &mut fall_through
                };
                TraceSession::pad_stack(
                    &mut side.path,
                    start,
                    count,
                    earlier.unwrap_or(tag),
                    deficit,
                );
            }
            let mut merged =
                match self
                    .session
                    .join_tagged(fork, taken.path, fall_through.path, tag)
                {
                    Ok(merged) => merged,
                    Err(error) => return Err(self.unjoinable(&taken.cursor, error.0)),
                };
            self.session.switch_path(&mut merged);
            cursor.pc = taken.cursor.pc;
            cursor.state = taken.cursor.state;
            cursor.image = taken.cursor.image;
            cursor.split_image = taken.cursor.split_image;
            cursor.visited = Visited::join(
                &cursor.visited,
                [taken.cursor.visited, fall_through.cursor.visited],
            );
            return Ok(Flow::Running);
        }
    }

    /// One step of a forked path, run as the session's current path.
    pub(super) fn advance_side(&mut self, side: &mut Side) -> Result<Flow, Halt> {
        self.session.switch_path(&mut side.path);
        let result = self.advance(&mut side.cursor);
        self.session.switch_path(&mut side.path);
        result
    }

    /// Fetch, decode and execute the instruction at the cursor.
    pub(super) fn step(&mut self, cursor: &mut Cursor) -> Result<Step, PrefixStopReason> {
        let preparation = self.preparation;
        let session = &mut self.session;
        let step_index = self.steps;
        self.steps += 1;
        let (pc, state) = (cursor.pc, cursor.state);
        // A repeated position is reported before the cell is fetched.
        if !cursor
            .visited
            .insert((pc, state, session.call_frames().to_vec()))
        {
            return Err(halt(session, &StepFault::Repeated { pc, state }));
        }
        // The opcode byte picks the handler: a byte the merged histories
        // patched differently is read differently by each.
        self.split_read_at = None;
        if cursor.split_image.contains(&pc) {
            self.split_read_at = Some(pc);
            return Err(split_read_stop(session, pc, state, pc));
        }
        let fetched = step::fetch_cell(&cursor.image, &preparation.table, pc, state)
            .map_err(|fault| halt(session, &fault))?;
        if fetched.next_state >= preparation.table.len() {
            return Err(halt(
                session,
                &StepFault::InvalidNextState {
                    next_state: fetched.next_state,
                },
            ));
        }
        let opcode = fetched.opcode;
        let cell = fetched.cell;
        let next_state = fetched.next_state;
        if let Some(machine) = &preparation.wasm {
            if let Some((reason, traps)) = wasm_bound(machine, session) {
                let code = if traps {
                    TraceStopCode::WasmTrap
                } else {
                    TraceStopCode::PrefixUnsupportedHandler
                };
                session.stop_prefix(code, reason.clone(), None);
                return Err(PrefixStopReason::UnsupportedHandler {
                    selector: cell.selector,
                    reason,
                });
            }
        }
        session.debug_before_operation(Some(pc), Some(state), Some(opcode), cell.selector);
        let operand_entry_pc = step::checked_pc_increment(pc, cursor.image.len())
            .map_err(|fault| halt(session, &fault))?;

        if cell.selector == preparation.patch_handler.selector {
            if let Some(address) = split_in_range(
                &cursor.split_image,
                operand_entry_pc,
                preparation.patch_handler.operand_width,
            ) {
                self.split_read_at = Some(address);
                return Err(split_read_stop(session, pc, state, address));
            }
            let patched = read_image_patch(
                &cursor.image,
                operand_entry_pc,
                preparation.patch_handler.operand_width,
                cell.selector,
            );
            // A WASM interpreter's own tables were read once: a write there
            // would change what they say.
            if let (Some(machine), Ok((patch, _))) = (&preparation.wasm, &patched) {
                if patch.address as usize >= machine.frozen_from {
                    let reason = format!(
                        "image write at {} changes the WASM interpreter's own tables",
                        patch.address
                    );
                    session.stop_prefix(TraceStopCode::PrefixImagePatch, reason.clone(), None);
                    return Err(PrefixStopReason::UnsupportedHandler {
                        selector: cell.selector,
                        reason,
                    });
                }
            }
            // A patch that writes the byte the image holds leaves it shared:
            // a copy would only be compared back equal at the next call.
            if let Ok((patch, _)) = &patched {
                if patch.previous != patch.value {
                    Arc::make_mut(&mut cursor.image)[patch.address as usize] = patch.value;
                }
            }
            return match patched {
                Ok((patch, actual_next_pc)) => {
                    // Every history writes this byte now: they agree on it.
                    cursor.split_image.remove(&(patch.address as usize));
                    if session.debug_enabled() {
                        let effect = format!(
                            "image_patch address={} old={} new={}",
                            patch.address, patch.previous, patch.value
                        );
                        session.debug_after_operation(Some(&effect));
                    } else {
                        session.debug_after_operation(None);
                    }
                    self.patches.push(patch);
                    self.records.push(PrefixExecutionRecord {
                        step: step_index,
                        dispatch_pc: pc,
                        operand_entry_pc,
                        state_before: state,
                        state_after: next_state,
                        opcode,
                        selector: cell.selector,
                        actual_next_pc,
                        source_span: preparation.patch_handler.span.clone(),
                        effect: PrefixStepEffect::ImagePatch(patch),
                    });
                    cursor.pc = actual_next_pc;
                    cursor.state = next_state;
                    Ok(Step::Continue)
                }
                Err(stop) => {
                    session.stop_prefix(
                        TraceStopCode::PrefixImagePatch,
                        format!("bytecode image write rejected: {stop}"),
                        None,
                    );
                    Err(stop)
                }
            };
        }

        let Some(template) = step::find_template(
            &preparation.templates,
            &preparation.template_index,
            cell.selector,
        ) else {
            let reason = "selector has no proven operation-table handler".to_string();
            session.stop_prefix(TraceStopCode::PrefixMissingHandler, reason.clone(), None);
            return Err(PrefixStopReason::UnsupportedHandler {
                selector: cell.selector,
                reason,
            });
        };
        if template.status != TemplateStatus::Executable {
            let reason = template
                .unsupported
                .first()
                .map(|unknown| format!("{}: {}", unknown.reason, unknown.source))
                .unwrap_or_else(|| format!("handler status is {}", template.status));
            let trap = template.unsupported.first().is_some_and(|unknown| {
                unknown.reason == crate::dispatch::prefix::wasm::TRAP_REASON
            });
            let code = if trap {
                TraceStopCode::WasmTrap
            } else {
                TraceStopCode::PrefixUnsupportedHandler
            };
            session.stop_prefix(code, reason.clone(), None);
            return Err(PrefixStopReason::UnsupportedHandler {
                selector: cell.selector,
                reason,
            });
        }
        let lifted = step::find_operand_handler(
            &preparation.operands,
            &preparation.operand_index,
            cell.selector,
        )
        .map_err(|fault| halt(session, &fault))?;
        if lifted.status != OperandHandlerStatus::Lifted
            && lifted.status != OperandHandlerStatus::NotApplicable
        {
            let reason = format!(
                "handler operands are not fully proven (status {:?})",
                lifted.status
            );
            session.stop_prefix(TraceStopCode::PrefixUnliftedHandler, reason.clone(), None);
            return Err(PrefixStopReason::UnsupportedHandler {
                selector: cell.selector,
                reason,
            });
        }
        // The handler reads its operands from these bytes.
        let operand_entry = match lifted.entry_coordinate {
            OperandEntryCoordinate::AfterOpcode => operand_entry_pc,
            OperandEntryCoordinate::AtOpcode => pc,
        };
        let split_read = lifted.operands.iter().find_map(|descriptor| {
            let start = operand_entry.checked_add(descriptor.byte_offset as usize)?;
            split_in_range(&cursor.split_image, start, descriptor.width as usize)
        });
        if let Some(address) = split_read {
            self.split_read_at = Some(address);
            return Err(split_read_stop(session, pc, state, address));
        }
        let step::Decoded {
            operand_entry_pc: handler_entry_pc,
            operands: decoded_operands,
            actual_next_pc,
            instruction,
        } = step::decode_step(&cursor.image, pc, lifted).map_err(|fault| halt(session, &fault))?;
        // An operand read from a byte a fork's sides patched differently:
        // its value is that side's (see `Walk::patched_junk_entry`).
        if !self.image_splits.is_empty() {
            for descriptor in &lifted.operands {
                let start = operand_entry + descriptor.byte_offset as usize;
                let patched = (start..start + descriptor.width as usize)
                    .find(|address| self.image_splits.contains_key(address));
                let value = decoded_operands
                    .iter()
                    .find(|operand| operand.position == descriptor.position);
                if let (Some(address), Some(operand)) = (patched, value) {
                    self.patched_operands.insert(operand.value, address);
                }
            }
        }
        session.set_vm_cursor(VmCursor {
            byte_offset: actual_next_pc,
            dispatch_state: next_state,
        });
        self.last_marks = Some(session.stop_marks());
        session.set_instruction_at((pc, state));
        let result = session.execute_handler_at(Some(template), &instruction);
        let created = session.take_created_now();
        if !matches!(
            result,
            TraceHandlerResult::Partial | TraceHandlerResult::BudgetStopped
        ) {
            for (descriptor, byte_offset, dispatch_state) in created {
                let reason = format!(
                    "the VM function made at pc {byte_offset} state {dispatch_state} \
                     fails before its first branch"
                );
                // An entry outside the image is junk only where a fork's side
                // patched it there; any other is that function's own stop.
                let junk = if byte_offset >= self.preparation.initial_image.len() {
                    self.patched_junk_entry(&cursor.image, byte_offset, &reason);
                    self.patched_junk.is_some()
                } else {
                    self.junk_entry(descriptor, byte_offset, dispatch_state)
                };
                if junk {
                    self.last_junk = Some((descriptor, byte_offset, dispatch_state));
                    self.session.stop_prefix(
                        TraceStopCode::JunkFunctionEntry,
                        reason.clone(),
                        None,
                    );
                    return Err(PrefixStopReason::UnsupportedHandler {
                        selector: cell.selector,
                        reason,
                    });
                }
            }
        }
        let session = &mut self.session;
        match result {
            TraceHandlerResult::Continued
            | TraceHandlerResult::Returned(_)
            | TraceHandlerResult::Thrown(_) => {
                // A jump, call or return continues at the handler's assigned
                // PC/state; otherwise the VM falls through.
                let control = session.take_control_transfer();
                let fall_through = (actual_next_pc, next_state);
                let mut actual_next_pc = control.byte_offset.unwrap_or(actual_next_pc);
                let mut next_state = control.dispatch_state.unwrap_or(next_state);
                // A `return` in a `try` with a `finally`: the VM runs the
                // `finally`, then returns the value the body left. JavaScript
                // does that for the emitted `return`, with the `finally`
                // traced once after a throw: the path returns at once.
                let entered = session.finally_entered().filter(|(frame, _)| {
                    (actual_next_pc, next_state)
                        == (frame.handler_byte_offset, frame.handler_dispatch_state)
                });
                let entered = entered.and_then(|(frame, saved)| {
                    let (_, statement, _) = self
                        .open_tries
                        .iter()
                        .rev()
                        .find(|(open, ..)| *open == frame)?;
                    Some((frame, saved, *statement))
                });
                let handler = entered
                    .map(|(frame, ..)| (frame.handler_byte_offset, frame.handler_dispatch_state));
                let skips = entered.and_then(|(frame, saved, statement)| {
                    // Entered from the body at the `try`'s own level, no fork
                    // open since: the body joins the catch path there (see
                    // `TraceSession::join_finally`).
                    let own_level = self.open_tries.last().is_some_and(|&(open, _, targets)| {
                        open == frame && self.targets.len() == targets
                    });
                    if own_level || self.plain_finally.contains(&handler.expect("entered")) {
                        return None;
                    }
                    let reads = returns_at(
                        preparation,
                        &cursor.image,
                        saved.return_byte_offset,
                        saved.return_dispatch_state,
                    )?;
                    Some((frame, saved, statement, reads))
                });
                if let (Some((_, _, statement)), None) = (entered, &skips) {
                    *self.finally_inline.entry(statement).or_default() += 1;
                }
                // What the `finally` ended after a throw wrote.
                if let Some((statement, written)) = session.finally_writes() {
                    self.finally_writes
                        .entry(statement)
                        .or_default()
                        .extend(written);
                }
                if let Some((frame, saved, statement, reads)) = skips {
                    self.finally_reads
                        .entry(statement)
                        .or_default()
                        .extend(reads);
                    session.skip_finally();
                    session.mark_finally(statement);
                    self.finally_handlers
                        .insert((frame.handler_byte_offset, frame.handler_dispatch_state));
                    actual_next_pc = saved.return_byte_offset;
                    next_state = saved.return_dispatch_state;
                }
                let branch = session.take_branch();
                let returned = session.take_thread_return().is_some();
                let try_entry = session.take_try_entry();
                let record = self.records.len();
                self.records.push(PrefixExecutionRecord {
                    step: step_index,
                    dispatch_pc: pc,
                    operand_entry_pc: handler_entry_pc,
                    state_before: state,
                    state_after: next_state,
                    opcode,
                    selector: cell.selector,
                    actual_next_pc,
                    source_span: lifted.source_span.clone(),
                    effect: PrefixStepEffect::Handler {
                        recipe_identity: lifted.template.semantic_identity.clone(),
                        operands: decoded_operands,
                        branch: branch.map(|branch| PrefixBranch {
                            target: DispatchStart {
                                pc: branch.byte_offset,
                                state: branch.dispatch_state,
                            },
                            test: branch.test,
                            kind: match (branch.decided, branch.by) {
                                (Some(taken), BranchEvidence::Test) => {
                                    PrefixBranchKind::FakeByTest { taken }
                                }
                                (Some(taken), BranchEvidence::Override) => {
                                    PrefixBranchKind::FakeByContext {
                                        taken,
                                        reason: "forced by --fake-branches".into(),
                                    }
                                }
                                (Some(taken), BranchEvidence::Context) => {
                                    PrefixBranchKind::FakeByContext {
                                        taken,
                                        reason:
                                            "an earlier branch on this path tested the same value"
                                                .into(),
                                    }
                                }
                                (None, _) => PrefixBranchKind::Real,
                            },
                        }),
                    },
                });
                cursor.pc = actual_next_pc;
                cursor.state = next_state;
                if let Some(branch) = branch
                    .filter(|branch| branch.decided.is_some() && branch.by == BranchEvidence::Test)
                {
                    let dead = if branch.decided == Some(true) {
                        fall_through
                    } else {
                        (branch.byte_offset, branch.dispatch_state)
                    };
                    self.pending_audit = Some((dead, format!("{pc}/{state}")));
                }
                // A throw ends the path: the emitted `throw` reaches the
                // enclosing `try` the walk opened, if any.
                let threw = matches!(result, TraceHandlerResult::Thrown(_));
                Ok(match branch {
                    _ if returned || threw => Step::End,
                    _ if try_entry.is_some() => Step::Try(try_entry.expect("checked")),
                    Some(branch) if branch.decided.is_none() => Step::Branch(branch, record),
                    _ => Step::Continue,
                })
            }
            TraceHandlerResult::Partial => Err(PrefixStopReason::UnsupportedHandler {
                selector: cell.selector,
                reason: "handler recipe stopped as unsupported".into(),
            }),
            // Keep the recipe-level budget completion so the underlying
            // report records the actual budget reason rather than a
            // driver-level success.
            TraceHandlerResult::BudgetStopped => Err(PrefixStopReason::Budget),
        }
    }
}

/// The memory slots the code at `pc`/`state` reads to return, when it
/// returns the value on the stack or one it pushes first: a literal, which
/// the `finally` before it cannot change, or a slot's value (see
/// `Walk::finally_reads`). A stack length the return drops aside.
fn returns_at(
    preparation: &PrefixPreparation,
    image: &[u8],
    pc: usize,
    state: usize,
) -> Option<BTreeSet<u32>> {
    use crate::operations::templates::{RecipeOperation as Op, SemanticValue as Value};
    let (mut pc, mut state) = (pc, state);
    let mut reads = BTreeSet::new();
    for _ in 0..4 {
        let fetched = step::fetch_cell(image, &preparation.table, pc, state).ok()?;
        let template = step::find_template(
            &preparation.templates,
            &preparation.template_index,
            fetched.cell.selector,
        )?;
        if template.status != TemplateStatus::Executable {
            return None;
        }
        let steps = &template.steps;
        if steps
            .iter()
            .any(|step| matches!(step.operation, Op::CompleteThread { .. }))
        {
            let only = steps.iter().all(|step| {
                matches!(
                    step.operation,
                    Op::CompleteThread { .. } | Op::StackLengthSet { .. }
                )
            });
            return only.then_some(reads);
        }
        let lifted = step::find_operand_handler(
            &preparation.operands,
            &preparation.operand_index,
            fetched.cell.selector,
        )
        .ok()?;
        let decoded = step::decode_step(image, pc, lifted).ok()?;
        // Temporaries holding an operand.
        let mut operands = BTreeMap::new();
        for step in steps {
            match &step.operation {
                Op::Let {
                    temporary,
                    value: Value::Operand { position },
                } => {
                    let operand = decoded
                        .operands
                        .iter()
                        .find(|operand| operand.position == *position)?;
                    operands.insert(*temporary, u32::try_from(operand.value).ok()?);
                }
                Op::Let {
                    value: Value::StackLength,
                    ..
                } => {}
                Op::StackWrite {
                    value: Value::Literal(_),
                    ..
                } => {}
                Op::StackWrite {
                    value: Value::MemoryRead { slot },
                    ..
                } => {
                    let Value::Temporary { id } = slot.as_ref() else {
                        return None;
                    };
                    reads.insert(*operands.get(id)?);
                }
                _ => return None,
            }
        }
        (pc, state) = (decoded.actual_next_pc, fetched.next_state);
    }
    None
}

impl Walk<'_> {
    /// A function made at `byte_offset`, outside the image, from an operand
    /// an instruction read out of a byte a fork's sides patched differently:
    /// the side that patched it to what this path read is the junk one,
    /// whatever fork the path is nested in since (see `Walk::patched_junk`).
    fn patched_junk_entry(&mut self, image: &[u8], byte_offset: usize, reason: &str) {
        if byte_offset < self.preparation.initial_image.len() {
            return;
        }
        let Some(&address) = i64::try_from(byte_offset)
            .ok()
            .and_then(|value| self.patched_operands.get(&value))
        else {
            return;
        };
        let (Some(jump), Some(&(taken, fall_through))) = (
            self.image_splits.get(&address),
            self.split_bytes.get(&address),
        ) else {
            return;
        };
        let here = image.get(address).copied();
        let taken_fake = if here == taken {
            true
        } else if here == fall_through {
            false
        } else {
            return;
        };
        self.patched_junk = Some((
            jump.clone(),
            taken_fake,
            format!(
                "{}: {reason} (its entry reads byte {address}, which this side patched)",
                TraceStopCode::JunkFunctionEntry
            ),
        ));
    }

    /// Whether the VM function entered at `byte_offset`/`dispatch_state`
    /// fails before any branch the way no code that runs does: it reads
    /// below the empty stack it starts with, pops an error record or call
    /// frame it has none of, reaches a WASM trap, reads a constant table past
    /// its end, or writes the bytecode outside its image. Code that runs
    /// makes no such function, so the path that made it is not code that runs.
    pub(super) fn junk_entry(
        &mut self,
        descriptor: usize,
        byte_offset: usize,
        dispatch_state: usize,
    ) -> bool {
        if self.probe {
            return false;
        }
        // Outside every fork's sides a junk function decides nothing: the
        // walk would only mark it not junk and start over (see
        // `run_prefix`).
        if self.targets.is_empty() {
            return false;
        }
        let key = (descriptor, byte_offset, dispatch_state);
        if let Some(&junk) = self.entries.get(&key) {
            return junk;
        }
        // Every walk of the seed checks a function the same way.
        let shared = (
            descriptor,
            byte_offset,
            dispatch_state,
            self.starts.is_some(),
        );
        let known = self
            .preparation
            .junk_entries
            .lock()
            .ok()
            .and_then(|entries| entries.get(&shared).copied());
        if let Some(junk) = known {
            self.entries.insert(key, junk);
            return junk;
        }
        // It starts with the slots its descriptor sets up, as a call does:
        // `undefined` in the ones it initializes, a value the check does not
        // know in the ones its caller fills.
        let mut start = TraceContext::new();
        let limit = TraceLimits::default().max_memory_slots;
        if let Some(slots) = self
            .starts
            .as_ref()
            .and_then(|starts| starts.get(descriptor))
        {
            for &(slot, initialized) in slots {
                let set = if initialized {
                    start.initialize_memory_slot_bounded(slot, limit)
                } else {
                    start.set_memory_value_bounded(
                        slot,
                        SymbolicValue::External {
                            name: format!("entry_slot_{slot}"),
                        },
                        limit,
                    )
                };
                if set.is_err() {
                    return false;
                }
            }
        }
        let report = run_prefix(
            self.preparation,
            DispatchStart {
                pc: byte_offset,
                state: dispatch_state,
            },
            start,
            TraceLimits {
                max_steps: ENTRY_PROBE_STEPS,
                ..TraceLimits::default()
            },
            TraceDebugOptions::disabled(),
            &CellSharing {
                probe: true,
                ..CellSharing::default()
            },
        );
        let junk = fails_at_entry(self.preparation, &report);
        self.entries.insert(key, junk);
        if let Ok(mut entries) = self.preparation.junk_entries.lock() {
            entries.insert(shared, junk);
        }
        junk
    }
}

/// Whether a [`Walk::junk_entry`] check's `report` shows the function failing
/// before any branch the way no code that runs does.
pub(super) fn fails_at_entry(preparation: &PrefixPreparation, report: &PrefixTraceReport) -> bool {
    // Straight from the entry, with no real branch or `try` on the way
    // (a branch its test decides goes one way every time): every call
    // of it does that.
    let branched = report.records.iter().any(|record| {
        matches!(
            &record.effect,
            PrefixStepEffect::Handler {
                branch: Some(branch),
                ..
            } if branch.kind == PrefixBranchKind::Real
        )
    });
    let tried = report
        .trace
        .arena
        .statements
        .iter()
        .any(|statement| matches!(statement.kind, crate::TraceStatementKind::Try { .. }));
    // A fault the VM's dispatch makes inside a `try` jumps to its
    // handler: still junk when every handler it opened is no code either
    // (a state the table does not have, or a pc outside the image).
    let dispatch_fault = report.trace.diagnostics.last().is_some_and(|diagnostic| {
        matches!(
            diagnostic.code,
            TraceStopCode::PrefixMissingState
                | TraceStopCode::PrefixMissingOpcode
                | TraceStopCode::PrefixInvalidNextState
                | TraceStopCode::PrefixTruncatedOperands
                | TraceStopCode::PrefixTruncatedImage
        )
    });
    let handlers_junk = report.try_handlers.iter().all(|handler| {
        handler.pc >= preparation.initial_image.len()
            || preparation.table.get(handler.state).is_none()
    });
    let tried = tried && !(dispatch_fault && handlers_junk);
    !branched
        && !tried
        && report.trace.diagnostics.last().is_some_and(|diagnostic| {
            matches!(
                diagnostic.code,
                TraceStopCode::InvalidIndex
                    | TraceStopCode::EmptyExceptionStack
                    | TraceStopCode::EmptyCallFrameStack
                    | TraceStopCode::WasmTrap
                    | TraceStopCode::PrefixImagePatch
                    // An operand of its own bytecode indexes past the
                    // table: those bytes are none the VM runs as code.
                    | TraceStopCode::ConstantTableIndex
                    // Its bytecode dispatches to a state or opcode the
                    // table does not have, or ends mid-instruction.
                    | TraceStopCode::PrefixMissingState
                    | TraceStopCode::PrefixMissingOpcode
                    | TraceStopCode::PrefixInvalidNextState
                    | TraceStopCode::PrefixTruncatedOperands
            ) || (
                // Its own bytecode jumps out of the image. An entry
                // outside it is evidence only for the fork that patched
                // it there (see `patched_junk_entry`).
                diagnostic.code == TraceStopCode::PrefixTruncatedImage
                    && !report.records.is_empty()
            )
        })
}
