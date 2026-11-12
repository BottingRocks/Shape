//! Which candidates move, and where: the rules and their assumptions.

use super::*;

impl Plan<'_, '_> {
    /// The decision for `symbol` so far this round (recorded for `consider`).
    pub(super) fn forward_of(&self, symbol: &SymbolId) -> Option<&Forward> {
        let found = self.forwarded.get(symbol);
        if let Some(lookups) = self.lookups.borrow_mut().as_mut() {
            if lookups.seen.insert((0, symbol.index() as u32)) {
                lookups.forwarded.push((*symbol, found.cloned()));
            }
        }
        found
    }

    pub(super) fn assumed_vanishing(&self, start: u32) -> bool {
        let found = self.vanishing.contains(&start);
        if let Some(lookups) = self.lookups.borrow_mut().as_mut() {
            if lookups.seen.insert((1, start)) {
                lookups.vanishing.push((start, found));
            }
        }
        found
    }

    pub(super) fn assumed_moved(&self, start: u32) -> Option<&(u32, u32)> {
        let found = self.moved.get(&start);
        if let Some(lookups) = self.lookups.borrow_mut().as_mut() {
            if lookups.seen.insert((2, start)) {
                lookups.moved.push((start, found.copied()));
            }
        }
        found
    }

    /// Decide every candidate in source order, so a value's inputs are
    /// decided before the value itself.
    pub(super) fn decide(&mut self) {
        self.vanishing.clear();
        self.moved.clear();
        let mut order: Vec<usize> = (0..self.scan.candidates.len()).collect();
        order.sort_by_key(|index| self.scan.candidates[*index].statement.start);
        // Later declarations only become crossable once decided, so decide
        // again with what the last round settled. `edits` then checks every
        // move against the final decisions and blocks one that relied on a
        // declaration that stayed.
        // A candidate whose lookups give the answers they gave last round
        // decides the same way: keep its result instead of considering it
        // again. (`blocked` only changes between calls.)
        let mut memo: Vec<Option<(Option<Forward>, Lookups)>> = Vec::new();
        memo.resize_with(self.scan.candidates.len(), || None);
        for _ in 0..64 {
            self.decide_round(&order, &mut memo);
            let (vanishing, moved) = self.assumptions();
            if vanishing == self.vanishing && moved == self.moved {
                break;
            }
            self.vanishing = vanishing;
            self.moved = moved;
        }
    }

    /// Moves that crossed a declaration assuming it goes where the final
    /// decisions keep it, snapshots whose reads ride inside a moved value, and
    /// values read by a shuffle call that is deleted: a move would lose the
    /// value and an early slot would edit inside the deletion. Such a value
    /// stays this round; the next one sees the call gone.
    /// All are blocked together, so one retry settles them.
    pub(super) fn broken_assumptions(&self) -> Vec<SymbolId> {
        let mut broken = Vec::new();
        let (vanishing, moved) = self.assumptions();
        let gone: Vec<Span> = self
            .forwarded
            .values()
            .map(|forward| &self.scan.candidates[forward.candidate])
            .filter(|candidate| candidate.slot.is_none())
            .map(|candidate| candidate.init)
            .collect();
        let whole_calls = self.whole_calls();
        let deleted_calls: Vec<Span> = self
            .forwarded
            .values()
            .map(|forward| self.scan.candidates[forward.candidate].statement)
            .filter(|statement| whole_calls.contains(&statement.start))
            .collect();
        // Slots a snapshot this round reads directly at its reads: checked
        // against the stores where they are now, so no store may move this
        // round (an early slot's write could land between the copy and a
        // read).
        let snapshotted: BTreeSet<SymbolId> = self
            .forwarded
            .values()
            .filter(|forward| forward.snapshot)
            .flat_map(|forward| forward.reads.iter().filter_map(|(_, symbol)| *symbol))
            .collect();
        // Every other moved value that reads a slot: where it was defined
        // (it read the slot there) and where its reads put it now; a read
        // inside a value that moves too lands wherever that one does, so
        // counts as last.
        let moved_reads: Vec<(SymbolId, SymbolId, u32, Vec<u32>)> = self
            .forwarded
            .iter()
            .filter(|(_, forward)| !forward.stable && !forward.snapshot)
            .flat_map(|(owner, forward)| {
                let defined = self.scan.candidates[forward.candidate].statement.start;
                let reads: Vec<u32> = self
                    .scoping
                    .get_resolved_references(*owner)
                    .map(|reference| {
                        let span = self.span_of(reference.node_id());
                        if gone.iter().any(|init| init.start <= span.start && span.end <= init.end) {
                            u32::MAX
                        } else {
                            span.start
                        }
                    })
                    .collect();
                forward
                    .reads
                    .iter()
                    .filter_map(|(_, symbol)| *symbol)
                    .map(move |slot| (slot, *owner, defined, reads.clone()))
                    .collect::<Vec<_>>()
            })
            .collect();
        for (symbol, forward) in &self.forwarded {
            // Its other reads read the slot, so the slot's binding must stay.
            let stored_in = forward.handoff.map(|(slot, _)| slot).or(forward.early.map(|(slot, ..)| slot));
            if stored_in.is_some_and(|slot| self.forwarded.contains_key(&slot)) {
                broken.push(*symbol);
                continue;
            }
            if forward.early.is_some_and(|(slot, ..)| snapshotted.contains(&slot)) {
                broken.push(*symbol);
                continue;
            }
            // A moved value that read the slot before the store that goes,
            // and now reads it after the store's new place (this value's
            // definition), would read the new value.
            if let Some((slot, store, _)) = forward.early {
                let defined = self.scan.candidates[forward.candidate].statement.start;
                let crossed = moved_reads.iter().any(|(read, owner, at, reads)| {
                    *read == slot
                        && owner != symbol
                        && *at < store.start
                        && reads.iter().any(|&position| position > defined)
                });
                if crossed {
                    broken.push(*symbol);
                    continue;
                }
            }
            if forward.early.is_some() {
                let carried = self.scoping.get_resolved_references(*symbol).any(|reference| {
                    let span = self.span_of(reference.node_id());
                    gone.iter()
                        .chain(&deleted_calls)
                        .any(|init| init.start <= span.start && span.end <= init.end)
                });
                if carried {
                    broken.push(*symbol);
                }
                continue;
            }
            if forward.snapshot || forward.handoff.is_some() {
                let handed = forward.handoff.map(|(_, first)| first);
                let carried = self.scoping.get_resolved_references(*symbol).any(|reference| {
                    let span = self.span_of(reference.node_id());
                    Some(reference.node_id()) != handed
                        && gone.iter().any(|init| init.start <= span.start && span.end <= init.end)
                });
                if carried {
                    broken.push(*symbol);
                    continue;
                }
                if forward.snapshot {
                    continue;
                }
            }
            if forward.stable {
                continue;
            }
            let candidate = &self.scan.candidates[forward.candidate];
            let Some(read) = self.moved_read(*symbol, forward) else {
                continue;
            };
            let read_span = self.span_of(read);
            if deleted_calls.iter().any(|call| call.start <= read_span.start && read_span.end <= call.end) {
                broken.push(*symbol);
                continue;
            }
            let list = &self.scan.lists[candidate.list];
            for between in &list[candidate.position + 1..] {
                if between.span.start <= read_span.start && read_span.end <= between.span.end {
                    break;
                }
                let assumed = self.vanishing.contains(&between.span.start)
                    || self.moved.contains_key(&between.span.start);
                let still_moved = moved.get(&between.span.start);
                let holds = vanishing.contains(&between.span.start)
                    || (still_moved.is_some() && still_moved == self.moved.get(&between.span.start));
                if assumed && between.writes.is_none() && !holds {
                    broken.push(*symbol);
                    break;
                }
            }
        }
        broken
    }

    pub(super) fn decide_round(&mut self, order: &[usize], memo: &mut [Option<(Option<Forward>, Lookups)>]) {
        self.forwarded.clear();
        for index in order {
            let candidate = &self.scan.candidates[*index];
            if self.blocked.contains(&candidate.symbol) {
                continue;
            }
            let forward = match &memo[*index] {
                Some((forward, lookups)) if lookups.hold(self) => forward.clone(),
                _ => {
                    *self.lookups.borrow_mut() = Some(Lookups::default());
                    let forward = self.consider(*index);
                    let lookups = self.lookups.borrow_mut().take().unwrap_or_default();
                    memo[*index] = Some((forward.clone(), lookups));
                    forward
                }
            };
            if let Some(forward) = forward {
                self.forwarded.insert(candidate.symbol, forward);
            }
        }
    }

    /// What this round's decisions let the next round assume.
    pub(super) fn assumptions(&self) -> (BTreeSet<u32>, BTreeMap<u32, (u32, u32)>) {
        let mut vanishing = BTreeSet::new();
        let mut moved = BTreeMap::new();
        for (symbol, forward) in &self.forwarded {
            let candidate = &self.scan.candidates[forward.candidate];
            if candidate.slot.is_some() {
                continue;
            }
            if forward.stable || forward.snapshot {
                vanishing.insert(candidate.statement.start);
                continue;
            }
            if forward.early.is_some() {
                continue;
            }
            let Some(read) = self.moved_read(*symbol, forward) else {
                continue;
            };
            let read_span = self.span_of(read);
            let list = &self.scan.lists[candidate.list];
            if let Some(statement) = list[candidate.position + 1..]
                .iter()
                .find(|s| s.span.start <= read_span.start && read_span.end <= s.span.end)
            {
                moved.insert(candidate.statement.start, (statement.span.start, read_span.start));
            }
        }
        (vanishing, moved)
    }

    pub(super) fn consider(&self, index: usize) -> Option<Forward> {
        let candidate = &self.scan.candidates[index];
        if self.scoping.symbol_name(candidate.symbol).starts_with("$KEY_")
            && !self.internal_key(candidate.symbol)
        {
            return None;
        }
        let reads = self.live_reads(candidate)?;
        let owner = self.scan.lists[candidate.list][candidate.position].owner;
        let composed = self.composed_reads(candidate);
        if let Some(representative) = candidate.alias_of {
            let reads_ok = self.reads_follow(candidate, &reads, owner);
            return (reads_ok && self.is_stable(representative)).then_some(Forward {
                stable: true,
                ..Forward::moved(index, composed)
            });
        }
        let stable = self.stable_value(candidate, &reads, owner);
        let reads_ok = self.reads_follow(candidate, &reads, owner);
        if let Some(helper) = candidate.helper {
            // A pass-through slot is the argument's value only while nothing
            // can write the parameter through `arguments`.
            let span = self.scan.passthrough[&helper].span;
            let at = self.scan.arguments.partition_point(|s| *s < span.start);
            let aliased = self.scan.arguments.get(at).is_some_and(|s| *s < span.end);
            return (stable && reads_ok && !aliased).then_some(Forward {
                stable: true,
                ..Forward::moved(index, composed)
            });
        }
        if !stable && reads_ok && candidate.kind == InitKind::Identifier {
            if let Some(slot) = candidate
                .init_reference
                .and_then(|reference| self.scoping.get_reference(reference).symbol_id())
            {
                if self.snapshot_holds(candidate, slot, &reads, owner) {
                    return Some(Forward {
                        snapshot: true,
                        ..Forward::moved(index, composed)
                    });
                }
            }
        }
        if stable && reads_ok {
            return Some(Forward {
                stable: true,
                ..Forward::moved(index, composed)
            });
        }
        // Rule 11: a pure computation whose names keep their values up to its
        // last read computes the same value at every read.
        if reads_ok && self.computed_snapshot(candidate, &reads, owner) {
            return Some(Forward {
                snapshot: true,
                ..Forward::moved(index, composed)
            });
        }
        // Rule 12: a property read where no code runs up to its last read
        // reads the same property of the same object at every read.
        if reads_ok && self.property_snapshot(candidate, &reads, owner, &composed) {
            return Some(Forward {
                snapshot: true,
                ..Forward::moved(index, composed)
            });
        }
        if reads.len() > 1 && reads_ok {
            if let Some(handoff) = self.handoff(candidate, &reads, &composed, owner) {
                return Some(Forward {
                    handoff: Some(handoff),
                    ..Forward::moved(index, composed)
                });
            }
        }
        if reads_ok && candidate.slot.is_none() {
            if let Some(early) = self.early_slot(candidate, &reads, owner) {
                // Rule 2 is the better form for one read next door.
                if !(reads.len() == 1 && self.next_door(candidate, reads[0], &composed)) {
                    return Some(Forward {
                        early: Some(early),
                        ..Forward::moved(index, composed)
                    });
                }
            }
        }
        if reads.len() != 1 || !reads_ok {
            return None;
        }
        // A tuple (an array literal, or a call of a tuple helper whose slots
        // pass its arguments through) read once at a literal index is a
        // tuple slot: moved there it reads `[a, b][0]` or `H(x)[0]`; the
        // tuple passes take the slot.
        if self.tuple_value(candidate)
            && matches!(self.nodes.parent_kind(reads[0]), AstKind::ComputedMemberExpression(member)
                if member.object.span() == self.span_of(reads[0])
                    && matches!(member.expression, Expression::NumericLiteral(_)))
        {
            return None;
        }
        if self.fresh_moves(candidate, reads[0], owner) {
            return Some(Forward::moved(index, composed));
        }
        self.next_door(candidate, reads[0], &composed)
            .then_some(Forward::moved(index, composed))
    }

    /// `[…]`, or `H(…)` of a never-reassigned fresh-tuple helper.
    fn tuple_value(&self, candidate: &Candidate) -> bool {
        let text = &self.source[candidate.init.start as usize..candidate.init.end as usize];
        if text.starts_with('[') {
            return true;
        }
        let Some((reference, ident)) = self.references_in(candidate.init).into_iter().next() else {
            return false;
        };
        ident.start == candidate.init.start
            && self.source[ident.end as usize..candidate.init.end as usize].starts_with('(')
            && self
                .scoping
                .get_reference(reference)
                .symbol_id()
                .is_some_and(|helper| self.scan.fresh_helpers.contains(&helper))
    }

    /// The candidate's reads outside deleted decoys; `None` if it is ever
    /// written or never read.
    fn live_reads(&self, candidate: &Candidate) -> Option<Vec<NodeId>> {
        let ids = self.scoping.get_resolved_reference_ids(candidate.symbol);
        if ids.is_empty() {
            return None;
        }
        let mut reads = Vec::with_capacity(ids.len());
        for id in ids {
            let reference = self.scoping.get_reference(*id);
            if reference.is_write() {
                return None;
            }
            // A read inside a deleted decoy statement goes with it.
            if self.in_noop(self.span_of(reference.node_id())) {
                continue;
            }
            reads.push(reference.node_id());
        }
        if reads.is_empty() {
            return None;
        }
        Some(reads)
    }

    /// Every read follows the definition in its owner and is a plain value
    /// read (no shorthand property or `delete` operand).
    fn reads_follow(&self, candidate: &Candidate, reads: &[NodeId], owner: u32) -> bool {
        reads.iter().all(|read| {
            self.owner_of(*read) == owner
                && self.span_of(*read).start >= candidate.statement.end
                && !self.is_shorthand_or_delete(*read)
        })
    }

    /// Rule 1: the value is the same wherever it is read.
    fn stable_value(&self, candidate: &Candidate, reads: &[NodeId], owner: u32) -> bool {
        (match candidate.kind {
            InitKind::Word | InitKind::Number | InitKind::UnaryLiteral => true,
            InitKind::TupleSlot => true,
            InitKind::Identifier => candidate
                .init_reference
                .and_then(|reference| self.scoping.get_reference(reference).symbol_id())
                .is_some_and(|symbol| {
                    self.stable_in(symbol, owner) && self.declared_before(symbol, candidate.statement)
                }),
            // A pure computation over stable names, read once, moves anywhere
            // it resolves.
            InitKind::Computed => {
                candidate.pure
                    && !(candidate.named && reads.len() > 1 && !self.constant(candidate))
                    && !self.tested(candidate)
                    && self.references_in(candidate.init).iter().all(|(reference, _)| {
                        self.scoping
                            .get_reference(*reference)
                            .symbol_id()
                            .is_none_or(|symbol| self.stable_in(symbol, owner))
                    })
            }
        }) && !self.carries_move(candidate.init)
    }

    /// Rule 11: a pure computation whose names keep their values up to its
    /// last read.
    fn computed_snapshot(&self, candidate: &Candidate, reads: &[NodeId], owner: u32) -> bool {
        candidate.kind == InitKind::Computed
            && candidate.pure
            && !(candidate.named && reads.len() > 1 && !self.constant(candidate))
            && !self.tested(candidate)
            && !self.carries_move(candidate.init)
            && self.references_in(candidate.init).iter().all(|(reference, _)| {
                self.scoping
                    .get_reference(*reference)
                    .symbol_id()
                    .is_none_or(|symbol| {
                        self.stable_in(symbol, owner) || self.unchanged_until(symbol, candidate, reads, owner, false)
                    })
            })
    }

    /// Rule 12: a property read where no code runs up to its last read.
    fn property_snapshot(
        &self,
        candidate: &Candidate,
        reads: &[NodeId],
        owner: u32,
        composed: &[(String, Option<SymbolId>)],
    ) -> bool {
        candidate.property_read
            && !self.carries_move(candidate.init)
            && self.references_in(candidate.init).iter().all(|(reference, _)| {
                self.scoping
                    .get_reference(*reference)
                    .symbol_id()
                    .is_none_or(|symbol| {
                        self.stable_in(symbol, owner) || self.unchanged_until(symbol, candidate, reads, owner, true)
                    })
            })
            && self.property_unchanged(candidate, reads, owner, composed)
    }

    /// A fresh allocation over stable names has no effect to order: it
    /// moves to its one read unless a loop there would run it again.
    fn fresh_moves(&self, candidate: &Candidate, read: NodeId, owner: u32) -> bool {
        candidate.fresh
            && !self.carries_move(candidate.init)
            && self.references_in(candidate.init).iter().all(|(reference, _)| {
                self.scoping
                    .get_reference(*reference)
                    .symbol_id()
                    .is_none_or(|symbol| self.stable_in(symbol, owner))
            })
            && !self.nodes.ancestor_ids(read).any(|ancestor| {
                let kind = self.nodes.kind(ancestor);
                let span = kind.span();
                is_loop(kind) && !(span.start <= candidate.statement.start && candidate.statement.end <= span.end)
            })
    }

    /// Rule 2: `read` is in the next statement that does anything.
    pub(super) fn next_door(
        &self,
        candidate: &Candidate,
        read: NodeId,
        composed: &[(String, Option<SymbolId>)],
    ) -> bool {
        let list = &self.scan.lists[candidate.list];
        let read_span = self.span_of(read);
        let Some(offset) = list[candidate.position + 1..]
            .iter()
            .position(|statement| statement.span.start <= read_span.start && read_span.end <= statement.span.end)
        else {
            return false;
        };
        let use_position = candidate.position + 1 + offset;
        let owner = list[candidate.position].owner;
        for between in &list[candidate.position + 1..use_position] {
            if self.assumed_vanishing(between.span.start)
                || self.assumed_moved(between.span.start).is_some_and(|(into, at)| {
                    *into == list[use_position].span.start && *at > read_span.start
                })
            {
                continue;
            }
            let Some(writes) = &between.writes else {
                return false;
            };
            if !self.inert(between.span, owner, None, None, candidate)
                && !self.crosses_reads(between.span, owner, candidate)
            {
                return false;
            }
            // A statement writes a name or two: look them up in place rather
            // than building a set of every name the value reads.
            for written in writes {
                if composed.iter().any(|(_, symbol)| *symbol == Some(*written))
                    || self
                        .scoping
                        .get_resolved_references(*written)
                        .any(|reference| self.owner_of(reference.node_id()) != owner)
                {
                    return false;
                }
            }
        }
        let use_statement = list[use_position].span;
        self.evaluated_once(read, use_statement)
            && self.inert(
                Span::new(use_statement.start, read_span.start),
                owner,
                Some(read_span),
                Some(candidate.statement.start),
                candidate,
            )
    }

    /// Walk from the read to its statement: the read must be evaluated
    /// exactly once, unconditionally, in the statement's own code.
    pub(super) fn evaluated_once(&self, read: NodeId, statement: Span) -> bool {
        let read_span = self.span_of(read);
        let mut child = read;
        for ancestor in self.nodes.ancestor_ids(read) {
            let kind = self.nodes.kind(ancestor);
            let child_span = self.span_of(child);
            match kind {
                AstKind::ExpressionStatement(_)
                | AstKind::VariableDeclaration(_)
                | AstKind::ReturnStatement(_)
                | AstKind::ThrowStatement(_)
                    if kind.span() == statement =>
                {
                    return true;
                }
                AstKind::IfStatement(statement_node) if statement_node.span == statement => {
                    return statement_node.test.span().start <= read_span.start
                        && read_span.end <= statement_node.test.span().end;
                }
                // The start of a chain runs before its first `?.`.
                AstKind::ChainExpression(chain) if chain.span.start == read_span.start => {}
                AstKind::Function(_)
                | AstKind::ArrowFunctionExpression(_)
                | AstKind::Class(_)
                | AstKind::ChainExpression(_) => return false,
                AstKind::ConditionalExpression(conditional)
                    if conditional.test.span() != child_span =>
                {
                    return false;
                }
                AstKind::LogicalExpression(logical) if logical.left.span() != child_span => {
                    return false;
                }
                AstKind::AssignmentExpression(assignment)
                    if assignment.operator.is_logical() =>
                {
                    return false;
                }
                _ if is_statement(kind) => return false,
                _ => {}
            }
            child = ancestor;
        }
        false
    }

    /// Everything in `range` of `owner`'s own code is stable: no effect and
    /// only reads of stable bindings (or plain identifier assignment targets).
    /// `except` excludes the read being forwarded and the nodes enclosing it.
    /// A value moved here (rule 2) from a declaration before `moved_before`
    /// already ran before the candidate, so reading it first keeps order.
    pub(super) fn inert(
        &self,
        range: Span,
        owner: u32,
        except: Option<Span>,
        moved_before: Option<u32>,
        mover: &Candidate,
    ) -> bool {
        let start = self.scan.effects.partition_point(|(s, _, _)| *s < range.start);
        for (s, e, effect_owner) in &self.scan.effects[start..] {
            if *s >= range.end {
                break;
            }
            if *effect_owner != owner {
                continue;
            }
            // A node enclosing the read (the call taking it as an argument)
            // runs after it; anything else here runs before.
            if !except.is_some_and(|read| *s <= read.start && read.end <= *e) {
                return false;
            }
        }
        let start = self.scan.idents.partition_point(|(s, _, _)| *s < range.start);
        for (s, reference, ident_owner) in &self.scan.idents[start..] {
            if *s >= range.end {
                break;
            }
            if *ident_owner != owner {
                continue;
            }
            let reference = self.scoping.get_reference(*reference);
            if reference.is_write() && !reference.is_read() {
                continue;
            }
            let Some(symbol) = reference.symbol_id() else {
                if matches!(self.nodes.kind(reference.node_id()), AstKind::IdentifierReference(id) if is_constant_global(&id.name))
                {
                    continue;
                }
                return false;
            };
            let moved_earlier = moved_before.is_some_and(|before| {
                self.forward_of(&symbol).is_some_and(|forward| {
                    !forward.stable
                        && self.scan.candidates[forward.candidate].statement.start < before
                })
            });
            if !moved_earlier && !self.stable_in(symbol, owner) && !self.unwritable_by(symbol, owner, mover) {
                return false;
            }
        }
        true
    }
}

impl Forward {
    /// A forward that moves the value to its reads; each rule sets its field.
    fn moved(candidate: usize, reads: Vec<(String, Option<SymbolId>)>) -> Forward {
        Forward {
            candidate,
            stable: false,
            snapshot: false,
            handoff: None,
            early: None,
            reads,
        }
    }
}
