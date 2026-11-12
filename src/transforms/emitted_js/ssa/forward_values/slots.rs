//! Slot snapshots and stores: when a slot's value reaches a read unchanged.

use super::*;

impl Plan<'_, '_> {
    /// Rule 7: a later sibling stores the value into a `var` slot.
    pub(super) fn early_slot(&self, candidate: &Candidate, reads: &[NodeId], owner: u32) -> Option<(SymbolId, Span, bool)> {
        let list = &self.scan.lists[candidate.list];
        for read in reads {
            let read_span = self.span_of(*read);
            let (slot, statement, declared) = match self.nodes.parent_kind(*read) {
                AstKind::VariableDeclarator(declarator)
                    if declarator.init.as_ref().is_some_and(|init| init.span() == read_span) =>
                {
                    let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
                        continue;
                    };
                    let AstKind::VariableDeclaration(statement) =
                        self.nodes.kind(self.nodes.parent_id(self.nodes.parent_id(*read)))
                    else {
                        continue;
                    };
                    if statement.kind != VariableDeclarationKind::Var || statement.declarations.len() != 1 {
                        continue;
                    }
                    (binding.symbol_id.get()?, statement.span, true)
                }
                AstKind::AssignmentExpression(assignment)
                    if assignment.operator == oxc_syntax::operator::AssignmentOperator::Assign
                        && assignment.right.span() == read_span =>
                {
                    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                        continue;
                    };
                    let AstKind::ExpressionStatement(statement) =
                        self.nodes.kind(self.nodes.parent_id(self.nodes.parent_id(*read)))
                    else {
                        continue;
                    };
                    let Some(slot) = analysis::identifier_symbol(target, self.scoping) else {
                        continue;
                    };
                    (slot, statement.span, false)
                }
                _ => continue,
            };
            // The store is a later statement of the definition's own list.
            if !list[candidate.position + 1..].iter().any(|s| s.span == statement) {
                continue;
            }
            let name = self.scoping.symbol_name(slot);
            // A `var` has no TDZ; a `let` carry must be declared before the
            // value, so writing it there is legal.
            let var = self
                .scoping
                .symbol_flags(slot)
                .contains(oxc_semantic::SymbolFlags::FunctionScopedVariable);
            if !(name.starts_with("$KEY_") || name.starts_with("$CARRY_"))
                || !(var || (!declared && self.declared_before(slot, candidate.statement)))
            {
                continue;
            }
            if self.early_slot_holds(candidate, slot, statement, reads, owner) {
                return Some((slot, statement, declared));
            }
        }
        None
    }

    pub(super) fn early_slot_holds(
        &self,
        candidate: &Candidate,
        slot: SymbolId,
        store: Span,
        reads: &[NodeId],
        owner: u32,
    ) -> bool {
        let last_read = reads.iter().map(|read| self.span_of(*read).start).max().unwrap_or(0).max(store.end);
        let start = candidate.statement.end;
        let mut shared = self.owner_of(self.scoping.symbol_declaration(slot)) != owner;
        for reference in self.scoping.get_resolved_references(slot) {
            let node = reference.node_id();
            let at = self.span_of(node).start;
            if self.owner_of(node) != owner {
                shared = true;
            }
            let in_store = store.start <= at && at < store.end;
            if in_store {
                continue;
            }
            // No other store until the last read; no read before the store.
            if reference.is_write() && start <= at && at < last_read {
                return false;
            }
            if reference.is_read() && start <= at && at < store.start {
                return false;
            }
        }
        for declaration in self.scoping.symbol_declarations(slot) {
            if let AstKind::VariableDeclarator(declarator) = self.nodes.kind(declaration) {
                let at = declarator.span.start;
                if declarator.init.is_some() && !(store.start <= at && at < store.end) && start <= at && at < last_read {
                    return false;
                }
            }
        }
        if shared && self.effect_between(start, last_read, owner) {
            return false;
        }
        // A handler in this function could see the slot written early if
        // anything between the value and its store throws — unless the slot
        // is a `let` declared inside that same `try` block, which a throw
        // leaves along with the binding.
        let statement_node = self.scoping.symbol_declaration(candidate.symbol);
        let slot_declared = self.span_of(self.scoping.symbol_declaration(slot));
        let block_scoped = !self
            .scoping
            .symbol_flags(slot)
            .contains(oxc_semantic::SymbolFlags::FunctionScopedVariable);
        let observing_try = self
            .nodes
            .ancestor_ids(statement_node)
            .take_while(|id| {
                !matches!(self.nodes.kind(*id), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_))
            })
            .any(|id| match self.nodes.kind(id) {
                AstKind::TryStatement(statement) => {
                    let block = statement.block.span;
                    !(block_scoped && block.start <= slot_declared.start && slot_declared.end <= block.end)
                }
                _ => false,
            });
        if observing_try && self.effect_between(start, store.start, owner) {
            return false;
        }
        let from = candidate.statement;
        reads.iter().all(|read| {
            !self.nodes.ancestor_ids(*read).any(|ancestor| {
                is_loop(self.nodes.kind(ancestor)) && {
                    let span = self.span_of(ancestor);
                    !(span.start <= from.start && from.end <= span.end)
                }
            })
        })
    }

    /// When a store becomes visible: an assignment's target is written after
    /// its right side runs, a declarator after its initializer, an update
    /// after its operand. A reference in a destructuring target or a loop
    /// head falls back to the earlier end of the reference, which can only
    /// keep the store inside the range.
    pub(super) fn write_time(&self, reference: NodeId) -> u32 {
        match self.nodes.parent_kind(reference) {
            AstKind::AssignmentExpression(assignment) => assignment.right.span().end,
            AstKind::UpdateExpression(update) => update.span.end,
            AstKind::VariableDeclarator(declarator) => declarator
                .init
                .as_ref()
                .map_or_else(|| self.span_of(reference).end, |init| init.span().end),
            _ => self.span_of(reference).end,
        }
    }

    /// No store reaches `slot` between `from` and the last of `reads`, and
    /// no read sits in a loop `from` is not in.
    ///
    /// When another function can store the slot, an effect-free range keeps
    /// the value: no code can run to store it. Otherwise every store must
    /// write a binding owned by the function that runs it, and the range must
    /// not suspend. Such a store cannot change the binding the copy read: an
    /// enclosing invocation is suspended on the stack, and a re-entered
    /// invocation gets a fresh binding. A rejected sibling or nested store,
    /// or one into a slot of an outer scope, may run between the copy and its
    /// reads.
    pub(super) fn slot_unchanged(&self, slot: SymbolId, from: Span, reads: &[NodeId], owner: u32) -> bool {
        self.slot_unchanged_with(slot, from, reads, owner, false)
    }

    /// With `property_mode`, a property read is not code that could run a
    /// store (rule 12's no-getters reading).
    pub(super) fn slot_unchanged_with(
        &self,
        slot: SymbolId,
        from: Span,
        reads: &[NodeId],
        owner: u32,
        property_mode: bool,
    ) -> bool {
        if reads.is_empty() {
            return true;
        }
        let last_read = reads.iter().map(|read| self.span_of(*read).start).max().unwrap_or(0);
        let range = (from.end, last_read);
        // Stores: write references, and every `var` redeclaration with an
        // initializer (a write with no reference).
        let mut stores: Vec<(u32, u32, NodeId)> = self
            .scoping
            .get_resolved_references(slot)
            .filter(|reference| reference.is_write())
            .map(|reference| {
                (
                    self.write_time(reference.node_id()),
                    self.owner_of(reference.node_id()),
                    reference.node_id(),
                )
            })
            .collect();
        for declaration in self.scoping.symbol_declarations(slot) {
            if let AstKind::VariableDeclarator(declarator) = self.nodes.kind(declaration) {
                if let Some(init) = &declarator.init {
                    stores.push((init.span().end, self.owner_of(declaration), declaration));
                }
            }
        }
        if stores.iter().any(|(at, store_owner, reference)| {
            range.0 <= *at
                && *at < range.1
                && !self.store_skipped(*at, *store_owner, *reference, owner, reads)
        }) {
            return false;
        }
        // Another function may store into the slot: then nothing between the
        // copy and its last read may run code.
        let shared = self.owner_of(self.scoping.symbol_declaration(slot)) != owner
            || stores.iter().any(|(_, store_owner, _)| *store_owner != owner);
        // A shared slot is read safely when nothing can run between the copy
        // and its reads, or when every store writes a binding of the function
        // that runs it and the range does not suspend.
        // Property reads and stores run nothing (no getters or setters), and
        // none can rebind a variable: only code that can run another
        // function may store the slot.
        let read_spans: Vec<Span> = reads.iter().map(|read| self.span_of(*read)).collect();
        let at = self.scan.runs.partition_point(|(s, _, _)| *s < range.0);
        let code_runs = self.scan.runs[at..]
            .iter()
            .take_while(|(s, _, _)| *s < range.1)
            .any(|(s, e, run_owner)| {
                *run_owner == owner && *e <= range.1 && !(property_mode && self.converts_only(*s, *e, &read_spans))
            });
        if shared
            && code_runs
            && !(self.stores_settled(slot, &stores)
                && !self.suspends_between(range.0, range.1, owner))
        {
            return false;
        }
        reads.iter().all(|read| {
            // A loop the copy is not in repeats its reads; that is only a
            // problem when the loop can also store the slot.
            !self.nodes.ancestor_ids(*read).any(|ancestor| {
                is_loop(self.nodes.kind(ancestor)) && {
                    let span = self.span_of(ancestor);
                    !(span.start <= from.start && from.end <= span.end)
                        && stores
                            .iter()
                            .any(|(at, _, _)| span.start <= *at && *at < span.end)
                }
            })
        })
    }

    /// A store in range is skipped when its block jumps away before any read
    /// it could reach runs: `return`/`throw` leave the activation, and
    /// `continue` starts the next iteration, where a copy inside the loop runs
    /// again. Only a store of the copy's own code qualifies: a callee returns
    /// to its caller, whose later reads still run. Inside a `try` of its own
    /// function the store declines: a `throw` — explicit, or from anything
    /// between the store and the jump — can resume in a `catch` or
    /// `finally` and reach later reads.
    pub(super) fn store_skipped(
        &self,
        at: u32,
        store_owner: u32,
        reference: NodeId,
        owner: u32,
        reads: &[NodeId],
    ) -> bool {
        if store_owner != owner || self.inside_try(reference) {
            return false;
        }
        let later: Vec<(NodeId, Span)> = reads
            .iter()
            .copied()
            .map(|read| (read, self.span_of(read)))
            .filter(|(_, span)| span.start >= at)
            .collect();
        if later.is_empty() {
            return true;
        }
        // A read in another function may run after the jump.
        if later.iter().any(|(read, _)| self.owner_of(*read) != owner) {
            return false;
        }
        for ancestor in self.nodes.ancestor_ids(reference) {
            let (block_span, statements): (Span, &[oxc_ast::ast::Statement]) =
                match self.nodes.kind(ancestor) {
                    AstKind::BlockStatement(block) => (block.span, &block.body),
                    AstKind::FunctionBody(body) => (body.span, &body.statements),
                    _ => continue,
                };
            let Some(jump) = statements.last() else {
                continue;
            };
            // The store must run before the jump in this block.
            if at >= jump.span().start {
                continue;
            }
            let target = match jump {
                oxc_ast::ast::Statement::ReturnStatement(_)
                | oxc_ast::ast::Statement::ThrowStatement(_) => None,
                oxc_ast::ast::Statement::ContinueStatement(statement) => {
                    let label = statement.label.as_ref().map(|label| label.name.as_str());
                    match self.loop_body_around(ancestor, label) {
                        Some(body) => Some(body),
                        None => continue,
                    }
                }
                _ => continue,
            };
            // Anything after the store in this block still runs.
            if later.iter().any(|(_, span)| {
                block_span.start <= span.start && span.end <= block_span.end
            }) {
                return false;
            }
            match target {
                // `return`/`throw` leave the activation before any read.
                None => return !self.runs_finalizer(ancestor),
                Some(body) => {
                    // After `continue`, only the target loop runs again; the
                    // next iteration re-runs a copy inside it. A copy outside
                    // it is rejected by the loop check in `slot_unchanged`.
                    if later
                        .iter()
                        .any(|(_, span)| !(body.start <= span.start && span.end <= body.end))
                    {
                        return false;
                    }
                    return !self.runs_finalizer(ancestor);
                }
            }
        }
        false
    }

    /// The body of the loop enclosing `node`, or the one its label names.
    pub(super) fn loop_body_around(&self, node: NodeId, label: Option<&str>) -> Option<Span> {
        for ancestor in self.nodes.ancestor_ids(node) {
            let body = match self.nodes.kind(ancestor) {
                AstKind::ForStatement(statement) => &statement.body,
                AstKind::ForInStatement(statement) => &statement.body,
                AstKind::ForOfStatement(statement) => &statement.body,
                AstKind::WhileStatement(statement) => &statement.body,
                AstKind::DoWhileStatement(statement) => &statement.body,
                _ => continue,
            };
            if let Some(label) = label {
                match self.nodes.parent_kind(ancestor) {
                    AstKind::LabeledStatement(labeled) if labeled.label.name.as_str() == label => {}
                    _ => continue,
                }
            }
            return Some(body.span());
        }
        None
    }

    /// A `try` of `node`'s own function encloses it.
    pub(super) fn inside_try(&self, node: NodeId) -> bool {
        self.nodes
            .ancestor_ids(node)
            .take_while(|id| {
                !matches!(self.nodes.kind(*id), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_))
            })
            .any(|id| matches!(self.nodes.kind(id), AstKind::TryStatement(_)))
    }

    /// A `try` with a finalizer encloses `node`, so a jump inside it still
    /// runs that code.
    pub(super) fn runs_finalizer(&self, node: NodeId) -> bool {
        self.nodes.ancestor_ids(node).any(|ancestor| {
            matches!(
                self.nodes.kind(ancestor),
                AstKind::TryStatement(statement) if statement.finalizer.is_some()
            )
        })
    }

    /// Every store of a shared slot writes a binding owned by the function
    /// that runs it: re-entering that function creates a fresh binding, and an
    /// enclosing invocation is suspended on the stack, so the copy keeps
    /// reading the binding it captured. A program-level store runs once and
    /// cannot re-enter either.
    pub(super) fn stores_settled(&self, slot: SymbolId, stores: &[(u32, u32, NodeId)]) -> bool {
        let declaration_owner = self.owner_of(self.scoping.symbol_declaration(slot));
        stores
            .iter()
            .all(|(_, store_owner, _)| {
                *store_owner == PROGRAM_OWNER || *store_owner == declaration_owner
            })
    }

    /// `owner`'s code suspends (`await`/`yield`) within `start..end`, so an
    /// enclosing frame may resume and store into its slots.
    pub(super) fn suspends_between(&self, start: u32, end: u32, owner: u32) -> bool {
        let at = self.scan.suspends.partition_point(|(s, _, _)| *s < start);
        self.scan.suspends[at..]
            .iter()
            .take_while(|(s, _, _)| *s < end)
            .any(|(_, _, effect_owner)| *effect_owner == owner)
    }

    /// An effect of `owner`'s own code that completes within `start..end`.
    pub(super) fn effect_between(&self, start: u32, end: u32, owner: u32) -> bool {
        let at = self.scan.effects.partition_point(|(s, _, _)| *s < start);
        self.scan.effects[at..]
            .iter()
            .take_while(|(s, _, _)| *s < end)
            .any(|(_, e, effect_owner)| *effect_owner == owner && *e <= end)
    }
}
