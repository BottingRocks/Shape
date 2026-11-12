//! Whether a value still holds where it is read.

use super::*;

impl Plan<'_, '_> {
    /// `range` only reads properties of names `mover`'s calls are not handed
    /// and `mover` stores no property: with no getters, and a call changing
    /// only what it is given, the value may run after those reads.
    pub(super) fn crosses_reads(&self, range: Span, owner: u32, mover: &Candidate) -> bool {
        let start = self.scan.effects.partition_point(|(s, _, _)| *s < range.start);
        // `mutations` is sorted: look each effect up instead of building a
        // set of the whole program's mutations per call.
        let mutated = |s: u32, e: u32| {
            let at = self.scan.mutations.partition_point(|(ms, me, _)| (*ms, *me) < (s, e));
            self.scan.mutations.get(at).is_some_and(|(ms, me, _)| (*ms, *me) == (s, e))
        };
        if self.scan.effects[start..]
            .iter()
            .take_while(|(s, _, _)| *s < range.end)
            .any(|(s, e, effect_owner)| *effect_owner == owner && mutated(*s, *e))
        {
            return false;
        }
        // Member stores and calls are keyed by start: only those starting
        // inside the value can lie inside it.
        let inside = (mover.init.start, 0)..(mover.init.end, u32::MAX);
        if self.scan.member_stores.range(inside.clone()).any(|((_, e), _)| *e <= mover.init.end) {
            return false;
        }
        let mut names = BTreeSet::new();
        for (reference, _) in self.references_in(range) {
            let reference = self.scoping.get_reference(reference);
            if reference.is_write() && !reference.is_read() {
                continue;
            }
            match reference.symbol_id() {
                Some(symbol) => {
                    names.insert(symbol);
                }
                None => {
                    if !matches!(self.nodes.kind(reference.node_id()), AstKind::IdentifierReference(id) if is_constant_global(&id.name)) {
                        return false;
                    }
                }
            }
        }
        // The names read must not change while the value runs either.
        if !names.iter().all(|symbol| self.stable_in(*symbol, owner) || self.unwritable_by(*symbol, owner, mover)) {
            return false;
        }
        self.scan
            .calls
            .range(inside)
            .filter(|(_, e)| *e <= mover.init.end)
            .all(|(s, e)| {
                !self.references_in(Span::new(*s, *e)).iter().any(|(reference, _)| {
                    self.scoping.get_reference(*reference).symbol_id().is_some_and(|symbol| names.contains(&symbol))
                })
            })
    }

    /// Moving `mover`'s value past a read of `symbol` cannot change what the
    /// read sees: only `owner`'s own code writes it (no call the value makes
    /// can), the value itself does not, and it is initialized before the
    /// value runs (no TDZ read to reorder with the value's effects).
    pub(super) fn unwritable_by(&self, symbol: SymbolId, owner: u32, mover: &Candidate) -> bool {
        let var = self
            .scoping
            .symbol_flags(symbol)
            .contains(oxc_semantic::SymbolFlags::FunctionScopedVariable);
        (var || self.declared_before(symbol, mover.statement))
            && !self.argument_aliased(symbol)
            && self.scoping.get_resolved_references(symbol).filter(|reference| reference.is_write()).all(|reference| {
                let span = self.span_of(reference.node_id());
                self.owner_of(reference.node_id()) == owner
                    && !(mover.init.start <= span.start && span.end <= mover.init.end)
            })
    }

    /// A binding no store can change: never written, not a VM slot or carry,
    /// and (for a parameter) in a function that never mentions `arguments`.
    pub(super) fn is_stable(&self, symbol: SymbolId) -> bool {
        if let Some(forward) = self.forward_of(&symbol) {
            return forward.stable;
        }
        let name = self.scoping.symbol_name(symbol);
        if name.starts_with("$KEY_") {
            return self.internal_key(symbol);
        }
        if name.starts_with("$CARRY_") || self.scoping.symbol_is_mutated(symbol) {
            return false;
        }
        !self.argument_aliased(symbol)
    }

    /// A parameter of a function that mentions `arguments`: a sloppy-mode
    /// store through it changes the parameter without a write reference.
    pub(super) fn argument_aliased(&self, symbol: SymbolId) -> bool {
        let declaration = self.scoping.symbol_declaration(symbol);
        if !matches!(
            self.nodes.kind(declaration),
            AstKind::FormalParameter(_) | AstKind::FormalParameters(_)
        ) {
            return false;
        }
        let Some(function) = self.function_of(declaration) else {
            return true;
        };
        let span = self.nodes.kind(function).span();
        let at = self.scan.arguments.partition_point(|s| *s < span.start);
        self.scan.arguments.get(at).is_some_and(|s| *s < span.end)
    }

    /// `symbol` cannot change while `owner`'s own code runs: stable, or
    /// settled for a closure (`settled_for_closures`).
    pub(super) fn stable_in(&self, symbol: SymbolId, owner: u32) -> bool {
        self.is_stable(symbol) || self.settled_for_closures(symbol, owner)
    }

    /// A binding only its declaring function's own code writes
    /// (`$KEY_216 = $THREAD_7;`), read from a closure: while the closure runs,
    /// the declaring frame is suspended under it or finished, so every read
    /// in one run of the closure sees the same value. Another call of the
    /// declaring function makes a new binding. A `let` must be the declaring
    /// function's first statement, so no closure reads it in its TDZ; a
    /// generator or async function could resume mid-closure and is left out,
    /// as is any file with `await` or `yield`.
    pub(super) fn settled_for_closures(&self, symbol: SymbolId, owner: u32) -> bool {
        // A suspending reader could let the declaring frame run the write
        // between its reads.
        if !self.scan.suspends.is_empty()
            || self.forward_of(&symbol).is_some()
            || self.scoping.symbol_declarations(symbol).count() != 1
        {
            return false;
        }
        let declaration = self.scoping.symbol_declaration(symbol);
        let AstKind::VariableDeclarator(declarator) = self.nodes.kind(declaration) else {
            return false;
        };
        let Some(function) = self.function_of(declaration) else {
            return false;
        };
        let declaring = self.nodes.kind(function).span().start;
        if declaring == owner {
            return false;
        }
        let (body, suspends) = match self.nodes.kind(function) {
            AstKind::Function(function) => (function.body.as_deref(), function.generator || function.r#async),
            AstKind::ArrowFunctionExpression(arrow) => (Some(&*arrow.body), arrow.r#async),
            _ => return false,
        };
        let Some(body) = body else {
            return false;
        };
        if suspends {
            return false;
        }
        let var = self
            .scoping
            .symbol_flags(symbol)
            .contains(oxc_semantic::SymbolFlags::FunctionScopedVariable);
        if !var {
            let declared_first = body.statements.first().is_some_and(|first| {
                let span = first.span();
                span.start <= declarator.span.start && declarator.span.end <= span.end
            });
            if !declared_first {
                return false;
            }
        }
        self.scoping
            .get_resolved_references(symbol)
            .filter(|reference| reference.is_write())
            .all(|reference| self.owner_of(reference.node_id()) == declaring)
    }

    /// A `$KEY_` slot that behaves like an `$SSA_` value: one `var`
    /// declaration with an initializer, no store, not in a loop, and every
    /// reference in the declaring function (no other thread sees it).
    pub(super) fn internal_key(&self, symbol: SymbolId) -> bool {
        if self.scoping.symbol_declarations(symbol).count() != 1 || self.scoping.symbol_is_mutated(symbol) {
            return false;
        }
        let declaration = self.scoping.symbol_declaration(symbol);
        let AstKind::VariableDeclarator(declarator) = self.nodes.kind(declaration) else {
            return false;
        };
        if declarator.init.is_none() {
            return false;
        }
        let owner = self.owner_of(declaration);
        let in_loop = self
            .nodes
            .ancestor_ids(declaration)
            .take_while(|id| {
                !matches!(self.nodes.kind(*id), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_))
            })
            .any(|id| is_loop(self.nodes.kind(id)));
        !in_loop
            && self
                .scoping
                .get_resolved_references(symbol)
                .all(|reference| self.owner_of(reference.node_id()) == owner)
    }

    /// Rule 5: every read of a copy of `slot` sees the value it copied.
    pub(super) fn snapshot_holds(&self, candidate: &Candidate, slot: SymbolId, reads: &[NodeId], owner: u32) -> bool {
        let name = self.scoping.symbol_name(slot);
        if !(name.starts_with("$KEY_") || name.starts_with("$CARRY_")) {
            return false;
        }
        let flags = self.scoping.symbol_flags(slot);
        // A `var` has no TDZ; anything else must already be initialized.
        if !flags.contains(oxc_semantic::SymbolFlags::FunctionScopedVariable)
            && !self.declared_before(slot, candidate.statement)
        {
            return false;
        }
        self.slot_unchanged(slot, candidate.statement, reads, owner)
    }

    /// A tested boolean reads as a literal in the arms it decides (rule 9)
    /// before anything copies it.
    pub(super) fn tested(&self, candidate: &Candidate) -> bool {
        self.scan.branches.iter().any(|(reference, ..)| {
            self.scoping.get_reference(*reference).symbol_id() == Some(candidate.symbol)
        })
    }

    /// Rule 12: nothing that can store a property runs between the read's
    /// declaration and its last read, and no read repeats in a loop the
    /// declaration is not in.
    pub(super) fn property_unchanged(
        &self,
        candidate: &Candidate,
        reads: &[NodeId],
        owner: u32,
        composed: &[(String, Option<SymbolId>)],
    ) -> bool {
        let names: Vec<SymbolId> = composed.iter().filter_map(|(_, symbol)| *symbol).collect();
        // A read through `arguments` must not be handed `arguments` either.
        let mentions_arguments = |span: Span| {
            let text = &self.source[span.start as usize..span.end as usize];
            text.match_indices("arguments").any(|(at, _)| {
                let before = text[..at].chars().next_back();
                let after = text[at + 9..].chars().next();
                !before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$')
                    && !after.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$')
            })
        };
        let through_arguments = mentions_arguments(candidate.init);
        let last = reads.iter().map(|read| self.span_of(*read).start).max().unwrap_or(0);
        let start = candidate.statement.end;
        let at = self.scan.mutations.partition_point(|(s, _, _)| *s < start);
        let read_spans: Vec<Span> = reads.iter().map(|read| self.span_of(*read)).collect();
        if self.scan.mutations[at..]
            .iter()
            .take_while(|(s, _, _)| *s < last)
            // A store or call around a read runs after the read: only work
            // completing before the last read can change it.
            .any(|(s, e, mutation_owner)| {
                *mutation_owner == owner && *e <= last && !self.spares(*s, *e, &read_spans, &names, through_arguments)
            })
        {
            return false;
        }
        // A read in a loop the declaration is not in repeats: only when the
        // loop changes no property except through other names (`.length`
        // in a loop test).
        reads.iter().all(|read| {
            !self.nodes.ancestor_ids(*read).any(|ancestor| {
                let kind = self.nodes.kind(ancestor);
                let span = kind.span();
                is_loop(kind)
                    && !(span.start <= candidate.statement.start && candidate.statement.end <= span.end)
                    && !self.loop_keeps_property(span, owner, &read_spans, &names, through_arguments)
            })
        })
    }

    /// An operator whose only untyped operand is one of the value's own
    /// reads: it converts the value it already converted where it was.
    pub(super) fn converts_only(&self, start: u32, end: u32, reads: &[Span]) -> bool {
        self.scan.conversions.get(&(start, end)).is_some_and(|untyped| reads.contains(untyped))
    }

    /// Work that cannot change a property read through `names`: the read's
    /// own conversion, a store into another name's object, or a call that
    /// is handed none of the names (a call changes only what it is given;
    /// shape/'s redundant-memory rule copies reads across calls the same
    /// way).
    pub(super) fn spares(&self, start: u32, end: u32, reads: &[Span], names: &[SymbolId], through_arguments: bool) -> bool {
        if names.is_empty() && !through_arguments {
            return false;
        }
        if through_arguments {
            let text = &self.source[start as usize..end as usize];
            if text.match_indices("arguments").any(|(at, _)| {
                !text[..at].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$')
                    && !text[at + 9..].chars().next().is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '$')
            }) {
                return false;
            }
        }
        if self.converts_only(start, end, reads) {
            return true;
        }
        if let Some(stored) = self.scan.member_stores.get(&(start, end)) {
            return stored.is_some_and(|stored| !names.contains(&stored));
        }
        self.scan.calls.contains(&(start, end))
            && !self.references_in(Span::new(start, end)).iter().any(|(reference, _)| {
                self.scoping.get_reference(*reference).symbol_id().is_some_and(|symbol| names.contains(&symbol))
            })
    }

    pub(super) fn loop_keeps_property(
        &self,
        span: Span,
        owner: u32,
        reads: &[Span],
        names: &[SymbolId],
        through_arguments: bool,
    ) -> bool {
        let at = self.scan.mutations.partition_point(|(s, _, _)| *s < span.start);
        !self.suspends_between(span.start, span.end, owner)
            && self.scan.mutations[at..]
                .iter()
                .take_while(|(s, _, _)| *s < span.end)
                .filter(|(_, _, mutation_owner)| *mutation_owner == owner)
                .all(|(s, e, _)| self.spares(*s, *e, reads, names, through_arguments))
    }

    /// Rule 11: a local a pure value reads holds the same value from the
    /// declaration to its last read.
    pub(super) fn unchanged_until(
        &self,
        symbol: SymbolId,
        candidate: &Candidate,
        reads: &[NodeId],
        owner: u32,
        property_mode: bool,
    ) -> bool {
        let flags = self.scoping.symbol_flags(symbol);
        if self.forward_of(&symbol).is_some() || self.argument_aliased(symbol) {
            return false;
        }
        if !flags.contains(oxc_semantic::SymbolFlags::FunctionScopedVariable)
            && !self.declared_before(symbol, candidate.statement)
        {
            return false;
        }
        self.slot_unchanged_with(symbol, candidate.statement, reads, owner, property_mode)
    }

    /// Rule 6: the first read stores the value into a slot next door.
    pub(super) fn handoff(
        &self,
        candidate: &Candidate,
        reads: &[NodeId],
        composed: &[(String, Option<SymbolId>)],
        owner: u32,
    ) -> Option<(SymbolId, NodeId)> {
        let first = *reads.iter().min_by_key(|read| self.span_of(**read).start)?;
        let read_span = self.span_of(first);
        let (slot, store) = match self.nodes.parent_kind(first) {
            AstKind::VariableDeclarator(declarator)
                if declarator.init.as_ref().is_some_and(|init| init.span() == read_span) =>
            {
                let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
                    return None;
                };
                let declaration = self.nodes.parent_id(self.nodes.parent_id(first));
                let AstKind::VariableDeclaration(statement) = self.nodes.kind(declaration) else {
                    return None;
                };
                if statement.declarations.len() != 1 {
                    return None;
                }
                (binding.symbol_id.get()?, statement.span)
            }
            AstKind::AssignmentExpression(assignment)
                if assignment.operator == oxc_syntax::operator::AssignmentOperator::Assign
                    && assignment.right.span() == read_span =>
            {
                let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                    return None;
                };
                let statement = self.nodes.parent_kind(self.nodes.parent_id(first));
                let AstKind::ExpressionStatement(statement) = statement else {
                    return None;
                };
                (analysis::identifier_symbol(target, self.scoping)?, statement.span)
            }
            _ => return None,
        };
        let name = self.scoping.symbol_name(slot);
        if !(name.starts_with("$KEY_") || name.starts_with("$CARRY_")) {
            return None;
        }
        let rest: Vec<NodeId> = reads.iter().copied().filter(|read| *read != first).collect();
        if rest.iter().any(|read| self.span_of(*read).start < store.end) {
            return None;
        }
        (self.next_door(candidate, first, composed)
            && self.slot_unchanged(slot, store, &rest, owner))
        .then_some((slot, first))
    }
}
