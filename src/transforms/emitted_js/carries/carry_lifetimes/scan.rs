//! The scan for handoffs whose storage the destination can take over.

use super::*;

impl Scan<'_, '_> {
    /// A fresh immutable snapshot followed by a private copy-in
    /// can use the destination's storage from the start. All other snapshot
    /// reads must finish before any statement that may update that storage.
    pub(super) fn initializers(&mut self, body: &[Statement<'_>]) {
        let mut declarations = BTreeMap::new();
        for (index, statement) in body.iter().enumerate() {
            let Statement::VariableDeclaration(dest) = statement else {
                continue;
            };
            if dest.declarations.len() != 1 {
                continue;
            }
            let BindingPattern::BindingIdentifier(to) = &dest.declarations[0].id else {
                continue;
            };
            let Some(to_symbol) = to.symbol_id.get() else {
                continue;
            };
            declarations.insert(to_symbol, dest);
            let Some(Expression::Identifier(copy)) = &dest.declarations[0].init else {
                continue;
            };
            let Some(from_symbol) = analysis::identifier_symbol(copy, self.scoping) else {
                continue;
            };
            let Some(source) = declarations.get(&from_symbol) else {
                continue;
            };
            self.initializer(body, index, source, dest, to, copy, from_symbol);
        }
    }
    /// Give the copy-in `dest = copy` at `index` the snapshot `source`'s
    /// initializer when the snapshot's reads finish before `dest` is updated.
    #[allow(clippy::too_many_arguments)]
    fn initializer(
        &mut self,
        body: &[Statement<'_>],
        index: usize,
        source: &VariableDeclaration<'_>,
        dest: &VariableDeclaration<'_>,
        to: &BindingIdentifier<'_>,
        copy: &IdentifierReference<'_>,
        from_symbol: SymbolId,
    ) {
        let to_symbol = to.symbol_id.get().unwrap();
        if !matches!(
            source.kind,
            VariableDeclarationKind::Let | VariableDeclarationKind::Const
        ) || !matches!(
            dest.kind,
            VariableDeclarationKind::Let
                | VariableDeclarationKind::Const
                | VariableDeclarationKind::Var
        ) || source.declarations.len() != 1
            || dest.declarations.len() != 1
        {
            return;
        }
        if dest.kind == VariableDeclarationKind::Var && !self.adjacent(body, index, source, to) {
            return;
        }
        let (BindingPattern::BindingIdentifier(_), Some(init)) =
            (&source.declarations[0].id, &source.declarations[0].init)
        else {
            return;
        };
        if from_symbol == to_symbol
            || analysis::identifier_symbol(copy, self.scoping) != Some(from_symbol)
            || !self.private(from_symbol, dest.span)
            || !self.private(to_symbol, oxc_span::Span::new(dest.span.end, dest.span.end))
        {
            return;
        }
        let mut rhs = init;
        while let Expression::ParenthesizedExpression(p) = rhs {
            rhs = &p.expression;
        }
        // Moving an anonymous function/class initializer to another
        // existing cell must not change its assignment-inferred name.
        if matches!(
            rhs,
            Expression::FunctionExpression(_)
                | Expression::ArrowFunctionExpression(_)
                | Expression::ClassExpression(_)
        ) {
            return;
        }
        let boundary = self.next_store(body, index, to_symbol);
        // No earlier undefined/TDZ observation, including a hoisted
        // observer invoked by the initializer, may see an earlier store.
        if self
            .scoping
            .get_resolved_references(to_symbol)
            .any(|r| self.nodes.get_node(r.node_id()).kind().span().start < dest.span.end)
        {
            return;
        }
        let Some(mut proposal) = self.snapshot_reads(source, copy, to, from_symbol, boundary)
        else {
            return;
        };
        if [source.span, dest.span]
            .iter()
            .any(|s| self.text(*s).contains("/*") || self.text(*s).contains("//"))
        {
            return;
        }
        proposal.push(analysis::Edit::replace(
            source.span,
            format!(
                "{} {} = {};",
                dest.kind.as_str(),
                self.text(to.span),
                self.text(init.span())
            ),
        ));
        proposal.push(analysis::Edit::delete(dest.span));
        if proposal.iter().any(|e| {
            self.edits
                .iter()
                .any(|p| e.start < p.end && p.start < e.end)
        }) {
            return;
        }
        self.edits.extend(proposal);
        self.count += 1;
    }
    /// Whether a hoisted `var` destination may take the snapshot's storage.
    fn adjacent(
        &self,
        body: &[Statement<'_>],
        index: usize,
        source: &VariableDeclaration<'_>,
        to: &BindingIdentifier<'_>,
    ) -> bool {
        // A hoisted destination can be observed from an outer catch
        // or after an early exit. Restrict this extension to adjacent
        // initializers: no user code can run between the source's
        // successful initialization and the original copy-in.
        let source_index = body.iter().position(|s| s.span() == source.span).unwrap();
        !(body[source_index + 1..index]
            .iter()
            .any(|s| !matches!(s, Statement::EmptyStatement(_)))
            || self
                .facts
                .function_writes
                .iter()
                .any(|(name, _)| name == to.name.as_str()))
    }
    /// Start of the first statement after `index` that updates `to_symbol`.
    fn next_store(&self, body: &[Statement<'_>], index: usize, to_symbol: SymbolId) -> u32 {
        let writes = self
            .scoping
            .get_resolved_references(to_symbol)
            .filter(|r| r.is_write())
            .map(|r| self.nodes.get_node(r.node_id()).kind().span())
            .collect::<Vec<_>>();
        body.iter()
            .skip(index + 1)
            .find(|s| {
                writes
                    .iter()
                    .any(|w| s.span().start <= w.start && w.end <= s.span().end)
            })
            .map_or(u32::MAX, |s| s.span().start)
    }
    /// Renames of the snapshot's reads to `to`, or `None` when one is a
    /// write, falls outside `source..boundary`, or cannot see `to`.
    fn snapshot_reads(
        &self,
        source: &VariableDeclaration<'_>,
        copy: &IdentifierReference<'_>,
        to: &BindingIdentifier<'_>,
        from_symbol: SymbolId,
        boundary: u32,
    ) -> Option<Vec<analysis::Edit>> {
        let to_symbol = to.symbol_id.get();
        let mut proposal = vec![];
        for r in self.scoping.get_resolved_references(from_symbol) {
            let at = self.nodes.get_node(r.node_id()).kind().span();
            if at == copy.span {
                continue;
            }
            if r.is_write()
                || at.start < source.span.end
                || at.end > boundary
                || self.facts.shorthand.contains(&at)
                || self.scoping.find_binding(r.scope_id(), to.name) != to_symbol
            {
                return None;
            }
            proposal.push(analysis::Edit::replace(at, self.text(to.span)));
        }
        Some(proposal)
    }
    pub(super) fn text(&self, span: oxc_span::Span) -> &str {
        &self.source[span.start as usize..span.end as usize]
    }
    pub(super) fn private(&self, symbol: SymbolId, at: Span) -> bool {
        !self.protected.contains(self.scoping.symbol_name(symbol))
            && self.scoping.symbol_redeclarations(symbol).is_empty()
            && self.locals.initialized(symbol, at)
            && self.scoping.get_resolved_references(symbol).all(|r| {
                self.facts
                    .owners
                    .enclosing(self.nodes.get_node(r.node_id()).kind().span())
                    == self.facts.owners.enclosing(at)
            })
    }
    pub(super) fn handoffs(&mut self, body: &[Statement<'_>], span: Span) {
        let mut statements = body
            .iter()
            .filter(|s| !matches!(s, Statement::EmptyStatement(_)))
            .collect::<Vec<_>>();
        if matches!(statements.last(), Some(Statement::ContinueStatement(_))) {
            statements.pop();
        }
        for statement in statements.iter().rev() {
            let Statement::ExpressionStatement(commit) = statement else {
                continue;
            };
            let Expression::AssignmentExpression(copy) = &commit.expression else {
                continue;
            };
            if copy.operator != AssignmentOperator::Assign
                || !matches!(copy.left, AssignmentTarget::AssignmentTargetIdentifier(_))
            {
                continue;
            }
            if let Expression::Identifier(value) = &copy.right {
                self.candidate(&statements, span, commit, value);
                continue;
            }
            // A computed commit may consume a snapshot as well. Try only
            // preceding generated producers; all uses must finish in this
            // commit, because its result need not equal the snapshot.
            let nodes = self.nodes;
            let mut candidates = Vec::new();
            for source in &statements {
                let Statement::VariableDeclaration(d) = source else {
                    continue;
                };
                if d.span.end > commit.span.start {
                    continue;
                }
                for item in &d.declarations {
                    let BindingPattern::BindingIdentifier(id) = &item.id else {
                        continue;
                    };
                    if !(id.name.starts_with("$SSA_") || id.name.starts_with("$CARRY_")) {
                        continue;
                    }
                    let Some(symbol) = id.symbol_id.get() else {
                        continue;
                    };
                    if let Some(value) = self
                        .scoping
                        .get_resolved_references(symbol)
                        .filter(|r| r.is_read())
                        .find_map(|r| {
                            let oxc_ast::AstKind::IdentifierReference(id) =
                                nodes.get_node(r.node_id()).kind()
                            else {
                                return None;
                            };
                            (copy.right.span().start <= id.span.start
                                && id.span.end <= copy.right.span().end)
                                .then_some(id)
                        })
                    {
                        candidates.push(value);
                    }
                }
            }
            for value in candidates {
                let before = self.count;
                self.candidate(&statements, span, commit, value);
                // Two independent proposals must not give the same destination
                // to distinct live snapshots consumed by this commit.
                if self.count != before {
                    break;
                }
            }
        }
    }
    pub(super) fn candidate(
        &mut self,
        statements: &[&Statement<'_>],
        body: Span,
        commit: &ExpressionStatement<'_>,
        value: &IdentifierReference<'_>,
    ) {
        let Expression::AssignmentExpression(copy) = &commit.expression else {
            return;
        };
        if copy.operator != AssignmentOperator::Assign {
            return;
        }
        let AssignmentTarget::AssignmentTargetIdentifier(target) = &copy.left else {
            return;
        };
        let simple_copy = matches!(&copy.right,Expression::Identifier(id)if id.span==value.span);
        let (Some(old), Some(next)) = (
            analysis::identifier_symbol(target, self.scoping),
            analysis::identifier_symbol(value, self.scoping),
        ) else {
            return;
        };
        if old == next || self.scoping.symbol_flags(old).is_const_variable() {
            return;
        }
        let Some((declaration, item)) = creation(statements, next) else {
            return;
        };
        let handoff = Handoff { declaration, item, commit, old, next };
        if !self.creation_allowed(&handoff) || !self.exceptions_allow(old, body) {
            return;
        }
        if !self.interval_clear(statements, &handoff) {
            return;
        }
        let initializer = item.init.as_ref();
        let copy_in = matches!(initializer, Some(Expression::Identifier(id))
            if analysis::identifier_symbol(id, self.scoping)==Some(old));
        if self.retargets_anonymous(initializer, next) {
            return;
        }
        // A lexical let without an initializer has a real undefined store.
        // Retarget it as well: an unassigned branch must never inherit the
        // destination's previous value.
        let definitely_written = initializer.is_none() && self.must_write(statements, &handoff);
        let first_write = if copy_in || definitely_written {
            u32::MAX
        } else {
            declaration.span.end
        };
        let boundary = self.share_boundary(statements, body, &handoff, simple_copy);
        let Some(first_write) = self.source_fits(&handoff, boundary, first_write) else {
            return;
        };
        if !self.destination_quiet(&handoff, target.span, definitely_written, first_write) {
            return;
        }
        let proposal =
            self.handoff_edits(&handoff, value, simple_copy, copy_in || definitely_written);
        if proposal.iter().any(|e| {
            self.edits
                .iter()
                .any(|p| e.start < p.end && p.start < e.end)
        }) || self.source[declaration.span.start as usize..declaration.span.end as usize]
            .contains("/*")
        {
            return;
        }
        self.edits.extend(proposal);
        self.count += 1;
    }
    /// Whether the creation of `next` can store into `old` from the start.
    fn creation_allowed(&self, handoff: &Handoff<'_, '_>) -> bool {
        let Handoff { declaration, item, old, next, .. } = *handoff;
        if item.init.is_none() != self.finished {
            return false;
        }
        if item.init.is_none() && declaration.kind != VariableDeclarationKind::Let {
            return false;
        }
        // A hoisted var's initializer is an implicit store, not necessarily
        // an IdentifierReference write. Do not advance the copy across a
        // destination declaration/initializer that still lies ahead.
        !(self.scoping.symbol_span(old).end > declaration.span.start
            || !self.private(old, declaration.span)
            || !self.private(next, Span::new(declaration.span.end, declaration.span.end)))
    }
    /// Whether no enclosing exception handler can observe `old` advanced
    /// early within `body`.
    fn exceptions_allow(&self, old: SymbolId, body: Span) -> bool {
        let contains =
            |outer: Span, inner: Span| outer.start <= inner.start && inner.end <= outer.end;
        for region in &self.facts.exceptions {
            if !contains(region.span, body)
                || self.facts.owners.enclosing(region.span) != self.facts.owners.enclosing(body)
            {
                continue;
            }
            let arm = [Some(region.block), region.handler, region.finalizer]
                .into_iter()
                .flatten()
                .find(|arm| contains(*arm, body));
            // A private lexical cell created within this arm cannot be read
            // by its sibling catch/finally arm or survive into a later entry.
            if self
                .scoping
                .symbol_flags(old)
                .contains(oxc_syntax::symbol::SymbolFlags::BlockScopedVariable)
                && arm.is_some_and(|arm| contains(arm, self.scoping.symbol_span(old)))
            {
                continue;
            }
            // Otherwise an exception must leave this execution, with no
            // handler observing the advanced store. A finally can suppress an
            // exception via break/continue, so retain that boundary.
            if region.finalizer.is_some() {
                return false;
            }
            if contains(region.block, body)
                && (!region.handler_exits
                    || region.handler.is_some_and(|handler| {
                        self.scoping.get_resolved_references(old).any(|r| {
                            contains(handler, self.nodes.get_node(r.node_id()).kind().span())
                        })
                    }))
            {
                return false;
            }
        }
        true
    }
    /// Whether nothing between the creation and the commit can expose the
    /// advanced store or rebind the source.
    fn interval_clear(&self, statements: &[&Statement<'_>], handoff: &Handoff<'_, '_>) -> bool {
        let Handoff { declaration, commit, old, next, .. } = *handoff;
        let mut hazards = Hazards {
            before: commit.span.start,
            bad: false,
            target: self.scoping.symbol_name(old).to_owned(),
        };
        // Only the interval whose destination store advances can expose a
        // different value or skip the original commit. Work before creation
        // and after the commit retains the same state on both sides.
        for statement in statements {
            if declaration.span.end <= statement.span().start
                && statement.span().start < commit.span.start
            {
                hazards.visit_statement(statement);
            }
        }
        // A sloppy block function declaration named like the source stores
        // the source var (Annex B) where no reference shows it; reads of the
        // source after it would see the function, not the copy.
        let mut named = NamedFunction { name: self.scoping.symbol_name(next), found: false };
        for statement in statements {
            if declaration.span.end <= statement.span().start {
                named.visit_statement(statement);
            }
        }
        if named.found {
            return false;
        }
        !hazards.bad
    }
    /// Whether retargeting `next`'s stores would change an inferred name.
    fn retargets_anonymous(&self, initializer: Option<&Expression<'_>>, next: SymbolId) -> bool {
        let anonymous = |mut value: &Expression<'_>| {
            while let Expression::ParenthesizedExpression(p) = value {
                value = &p.expression;
            }
            matches!(
                value,
                Expression::FunctionExpression(_)
                    | Expression::ArrowFunctionExpression(_)
                    | Expression::ClassExpression(_)
            )
        };
        // Every store changes its inferred name when retargeted, not only
        // the declaration initializer. Named definitions keep their names,
        // but retaining them too keeps this proof conservative.
        initializer.is_some_and(anonymous)
            || self.scoping.get_resolved_references(next).filter(|r| r.is_write()).any(|r| {
                matches!(self.nodes.parent_kind(r.node_id()), oxc_ast::AstKind::AssignmentExpression(a) if anonymous(&a.right))
            })
    }
    /// Whether every path from the creation to the commit assigns `next`
    /// before reading it, and reads no `old` afterwards.
    fn must_write(&self, statements: &[&Statement<'_>], handoff: &Handoff<'_, '_>) -> bool {
        let Handoff { declaration, commit, old, next, .. } = *handoff;
        let mut proof = MustWrite {
            scoping: self.scoping,
            target: next,
            destination: old,
            assigned: false,
            valid: true,
        };
        for statement in statements {
            if declaration.span.end <= statement.span().start
                && statement.span().end <= commit.span.end
            {
                proof.visit_statement(statement);
            }
        }
        proof.valid && proof.assigned
    }
    /// Where uses of `next` must end: the commit for a computed commit,
    /// otherwise the next statement that updates `old`.
    fn share_boundary(
        &self,
        statements: &[&Statement<'_>],
        body: Span,
        handoff: &Handoff<'_, '_>,
        simple_copy: bool,
    ) -> u32 {
        let Handoff { commit, old, .. } = *handoff;
        // Later reads can share the destination until its next update
        // statement. That statement's RHS may still need the old snapshot.
        let mut later_writes = self
            .scoping
            .get_resolved_references(old)
            .filter(|r| r.is_write())
            .map(|r| self.nodes.get_node(r.node_id()).kind().span())
            .filter(|at| at.start >= commit.span.end)
            .collect::<Vec<_>>();
        later_writes.extend(
            self.facts
                .function_writes
                .iter()
                .filter(|(name, at)| {
                    name == self.scoping.symbol_name(old) && at.start >= commit.span.end
                })
                .map(|(_, at)| *at),
        );
        if !simple_copy {
            commit.span.end
        } else {
            statements
                .iter()
                .filter(|s| s.span().start >= commit.span.end)
                .find(|s| {
                    later_writes
                        .iter()
                        .any(|at| s.span().start <= at.start && at.end <= s.span().end)
                })
                .map_or(body.end, |s| s.span().start)
        }
    }
    /// The first point `old` holds `next`'s value, or `None` when a use of
    /// `next` falls outside its range or cannot see `old`.
    fn source_fits(
        &self,
        handoff: &Handoff<'_, '_>,
        boundary: u32,
        mut first_write: u32,
    ) -> Option<u32> {
        let Handoff { declaration, commit, old, next, .. } = *handoff;
        for r in self.scoping.get_resolved_references(next) {
            let span = self.nodes.get_node(r.node_id()).kind().span();
            if span.start < declaration.span.end
                || span.end > boundary
                || (r.is_write() && span.start >= commit.span.end)
                || self
                    .scoping
                    .find_binding(r.scope_id(), self.scoping.symbol_ident(old))
                    != Some(old)
            {
                return None;
            }
            if r.is_write() {
                if self.scoping.symbol_flags(next).is_const_variable() {
                    return None;
                }
                let event = self.facts.writes.get(&span)?;
                first_write = first_write.min(event.end);
            }
        }
        Some(first_write)
    }
    /// Whether `old` is neither written (but by the commit) nor read once
    /// overwritten between the creation and the commit.
    fn destination_quiet(
        &self,
        handoff: &Handoff<'_, '_>,
        target: Span,
        definitely_written: bool,
        first_write: u32,
    ) -> bool {
        let Handoff { declaration, commit, old, .. } = *handoff;
        for r in self.scoping.get_resolved_references(old) {
            let span = self.nodes.get_node(r.node_id()).kind().span();
            if span.start >= declaration.span.end && span.end <= commit.span.end {
                if r.is_write() && span != target {
                    return false;
                }
                if r.is_read() && !definitely_written && span.start >= first_write {
                    return false;
                }
            }
        }
        true
    }
    /// Edits creating `next` in `old`'s storage and renaming its uses.
    fn handoff_edits(
        &self,
        handoff: &Handoff<'_, '_>,
        value: &IdentifierReference<'_>,
        simple_copy: bool,
        stored: bool,
    ) -> Vec<analysis::Edit> {
        let Handoff { declaration, item, commit, old, next } = *handoff;
        let old_name = self.scoping.symbol_name(old);
        let intro = if stored {
            String::new()
        } else if let Some(initializer) = item.init.as_ref() {
            let text =
                &self.source[initializer.span().start as usize..initializer.span().end as usize];
            format!("{old_name} = ({text});")
        } else {
            format!("{old_name} = void 0;")
        };
        let retained = declaration
            .declarations
            .iter()
            .filter(|d| d.span != item.span)
            .map(|d| &self.source[d.span.start as usize..d.span.end as usize])
            .collect::<Vec<_>>();
        let intro = if retained.is_empty() {
            intro
        } else {
            format!("let {};{intro}", retained.join(", "))
        };
        let mut proposal = vec![analysis::Edit::replace(declaration.span, intro)];
        if simple_copy {
            proposal.push(analysis::Edit::delete(commit.span));
        }
        for r in self.scoping.get_resolved_references(next) {
            let span = self.nodes.get_node(r.node_id()).kind().span();
            if simple_copy && span == value.span {
                continue;
            }
            let replacement = if self.facts.shorthand.contains(&span) {
                format!("{}: {old_name}", self.scoping.symbol_name(next))
            } else {
                old_name.to_string()
            };
            proposal.push(analysis::Edit::replace(span, replacement));
        }
        proposal
    }
}

/// A commit `old = …next…` and the declaration that creates `next`.
#[derive(Clone, Copy)]
struct Handoff<'h, 'a> {
    declaration: &'h VariableDeclaration<'a>,
    item: &'h VariableDeclarator<'a>,
    commit: &'h ExpressionStatement<'a>,
    old: SymbolId,
    next: SymbolId,
}

/// The declaration among `statements` that creates `next`, and its item.
fn creation<'h, 'a>(
    statements: &[&'h Statement<'a>],
    next: SymbolId,
) -> Option<(&'h VariableDeclaration<'a>, &'h VariableDeclarator<'a>)> {
    statements.iter().find_map(|&s| {
        let Statement::VariableDeclaration(d) = s else {
            return None;
        };
        // Uninitialized let groups have no intervening evaluation; keep
        // every other cell's declaration and its original kind/scope.
        if d.declarations.len() != 1
            && (d.kind != VariableDeclarationKind::Let
                || d.declarations.iter().any(|item| item.init.is_some()))
        {
            return None;
        }
        d.declarations.iter().find_map(|item| {
            let BindingPattern::BindingIdentifier(id) = &item.id else {
                return None;
            };
            (id.symbol_id.get() == Some(next)).then_some((&**d, item))
        })
    })
}

impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_function_body(&mut self, b: &FunctionBody<'a>) {
        self.handoffs(&b.statements, b.span);
        if !self.finished {
            self.initializers(&b.statements);
        }
        walk::walk_function_body(self, b);
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        // The same lifetime proof applies to branch-local result handoffs.
        // Loop bodies are handled here as well, without visiting them twice.
        self.handoffs(&b.body, b.span);
        if !self.finished {
            self.initializers(&b.body);
        }
        walk::walk_block_statement(self, b);
    }
}
