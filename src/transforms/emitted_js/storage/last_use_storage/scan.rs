//! The scan for values whose storage another binding can reuse.

use super::*;

impl Scan<'_, '_> {
    pub(super) fn for_header(&mut self, statement: &ForStatement<'_>) {
        // Header initialization runs once before this loop, unlike statements
        // in its body. Keep the ordinary body-reuse activation guard intact.
        if !self.ended {
            return;
        }
        let Some(ForStatementInit::VariableDeclaration(next)) = &statement.init else {
            return;
        };
        if next.declarations.len() != 1
            || !matches!(
                next.kind,
                VariableDeclarationKind::Let | VariableDeclarationKind::Const
            )
        {
            return;
        }
        let item = &next.declarations[0];
        let (BindingPattern::BindingIdentifier(id), Some(init)) = (&item.id, &item.init) else {
            return;
        };
        let Some(symbol) = id.symbol_id.get() else {
            return;
        };
        let mut value = init;
        while let Expression::ParenthesizedExpression(p) = value {
            value = &p.expression;
        }
        if !generated(id.name.as_str())
            || !self.private(symbol, Span::new(next.span.end, next.span.end))
            || matches!(
                value,
                Expression::FunctionExpression(_)
                    | Expression::ArrowFunctionExpression(_)
                    | Expression::ClassExpression(_)
            )
        {
            return;
        }
        let references: Vec<_> = self.scoping.get_resolved_references(symbol).collect();
        if references.is_empty()
            || references.iter().any(|r| {
                let at = self.nodes.get_node(r.node_id()).kind().span();
                at.start < next.span.end
                    || at.end > statement.span.end
                    || r.is_write()
                        && (next.kind == VariableDeclarationKind::Const
                            || !safe_later_write(self.nodes.parent_kind(r.node_id())))
            })
        {
            return;
        }
        let mut inputs = Inputs {
            scoping: self.scoping,
            symbols: BTreeSet::new(),
        };
        inputs.visit_expression(init);
        let mut candidates: Vec<_> = self
            .declarations
            .iter()
            .filter_map(|(&input, &previous)| {
                if input == symbol
                    || previous.span.end > next.span.start
                    || !generated(self.scoping.symbol_name(input))
                    || !self.private(input, next.span)
                    || previous.kind == VariableDeclarationKind::Const
                        && self
                            .scoping
                            .get_resolved_references(input)
                            .any(|r| r.is_write())
                {
                    return None;
                }
                let last = self
                    .scoping
                    .get_resolved_references(input)
                    .map(|r| self.nodes.get_node(r.node_id()).kind().span().end)
                    .max()?;
                if last > next.span.end {
                    return None;
                }
                let entered = self.locals.repeated_between(previous.span, next.span)?;
                // A dependent initializer may enter only this loop. An
                // unrelated finished cell can also cross new parent loops:
                // every old observation finished before their first entry,
                // and this header resets the counter on every activation.
                if entered != statement.span && last > entered.start {
                    return None;
                }
                Some((input, previous, last))
            })
            .collect();
        candidates.sort_by_key(|(input, _, last)| {
            (!inputs.symbols.contains(input), std::cmp::Reverse(*last))
        });
        for (input, previous, _) in candidates {
            if references.iter().any(|r| self.scoping.find_binding(r.scope_id(), self.scoping.symbol_ident(input)) != Some(input))
                || self.nodes.iter().any(|node| matches!(node.kind(), AstKind::Function(f)
                    if f.span.start >= statement.span.start && f.span.end <= statement.span.end
                    && f.id.as_ref().is_some_and(|name| name.name == id.name || name.name == self.scoping.symbol_name(input))))
            {
                continue;
            }
            let mut edits = vec![analysis::Edit::replace(
                next.span,
                format!(
                    "{} = ({})",
                    self.scoping.symbol_name(input),
                    &self.source[init.span().start as usize..init.span().end as usize]
                ),
            )];
            self.retarget(&mut edits, previous, &references, &id.name, input);
            if self.lend(next.span, edits, input, symbol) {
                break;
            }
        }
    }
    /// Make a `const` lender writable and point `references` at `input`,
    /// keeping a shorthand property's `key`.
    fn retarget(
        &self,
        edits: &mut Vec<analysis::Edit>,
        previous: SourceDeclaration,
        references: &[&oxc_semantic::Reference],
        key: &str,
        input: SymbolId,
    ) {
        if previous.kind == VariableDeclarationKind::Const {
            edits.push(analysis::Edit::replace(
                Span::new(previous.span.start, previous.span.start + 5),
                "let",
            ));
        }
        for r in references {
            let at = self.nodes.get_node(r.node_id()).kind().span();
            let replacement = if matches!(self.nodes.parent_kind(r.node_id()), AstKind::ObjectProperty(p) if p.shorthand)
            {
                format!("{}: {}", key, self.scoping.symbol_name(input))
            } else {
                self.scoping.symbol_name(input).to_string()
            };
            edits.push(analysis::Edit::replace(at, replacement));
        }
    }
    /// Take `edits` lending `input`'s storage to `symbol`, unless the
    /// declaration `next` holds a comment or an edit overlaps one taken.
    fn lend(
        &mut self,
        next: Span,
        edits: Vec<analysis::Edit>,
        input: SymbolId,
        symbol: SymbolId,
    ) -> bool {
        let header = &self.source[next.start as usize..next.end as usize];
        if header.contains("/*")
            || header.contains("//")
            || edits.iter().any(|e| {
                self.edits
                    .iter()
                    .any(|p| e.start < p.end && p.start < e.end)
            })
        {
            return false;
        }
        self.edits.extend(edits);
        self.used.insert(input);
        self.used.insert(symbol);
        self.count += 1;
        true
    }
    pub(super) fn ended_candidates(
        &self,
        at: Span,
        excluded: &BTreeSet<SymbolId>,
    ) -> Vec<(SymbolId, SourceDeclaration)> {
        if !self.ended {
            return vec![];
        }
        let mut vacant: Vec<_> = self
            .declarations
            .iter()
            .filter_map(|(&s, &d)| {
                if excluded.contains(&s)
                    || d.span.end > at.start
                    || !generated(self.scoping.symbol_name(s))
                    || self.used.contains(&s)
                    || !self.locals.initialized(s, at)
                    || !self.locals.same_iteration(d.span, at)
                {
                    return None;
                }
                let last = self
                    .scoping
                    .get_resolved_references(s)
                    .map(|r| self.nodes.get_node(r.node_id()).kind().span().end)
                    .max()?;
                (d.span.end < last && last <= at.start).then_some((last, s, d))
            })
            .collect();
        vacant.sort_by_key(|&(last, s, _)| std::cmp::Reverse((last, s)));
        vacant.into_iter().map(|(_, s, d)| (s, d)).collect()
    }
    pub(super) fn private(&self, symbol: SymbolId, at: Span) -> bool {
        !self.used.contains(&symbol)
            && !self.protected.contains(self.scoping.symbol_name(symbol))
            && self.scoping.symbol_redeclarations(symbol).is_empty()
            && self.locals.initialized(symbol, at)
            && self.scoping.get_resolved_references(symbol).all(|r| {
                self.locals
                    .same_owner(at, self.nodes.get_node(r.node_id()).kind().span())
            })
    }
    pub(super) fn body(&mut self, body: &[Statement<'_>]) {
        for (position, statement) in body.iter().enumerate() {
            let Statement::VariableDeclaration(next) = statement else {
                continue;
            };
            if !matches!(
                next.kind,
                VariableDeclarationKind::Let | VariableDeclarationKind::Const
            ) {
                continue;
            }
            if next.declarations.len() != 1 {
                self.uninitialized_group(next, &body[position + 1..]);
                continue;
            }
            let BindingPattern::BindingIdentifier(id) = &next.declarations[0].id else {
                continue;
            };
            let Some(symbol) = id.symbol_id.get() else {
                continue;
            };
            let Some(init) = &next.declarations[0].init else {
                self.branch_successor(next, next.span, symbol, &body[position + 1..]);
                continue;
            };
            // Only emitted temporary storage is recycled, never an arbitrary
            // application local whose purpose could be obscured by the change.
            if generated(id.name.as_str())
                && self.private(symbol, Span::new(next.span.end, next.span.end))
            {
                self.last_input(next, id, symbol, init);
            }
            // An input may still be referenced later because an explicit
            // copy-back restores equality. Prove those versions rather than
            // requiring its entire textual live range to end at this RHS.
            if !self.used.contains(&symbol) {
                self.branch_successor(next, next.span, symbol, &body[position + 1..]);
            }
        }
    }
    /// Each binding of an uninitialized `let a, b;` group may borrow storage
    /// on its own; its removal is the binding and one separating comma.
    fn uninitialized_group(&mut self, next: &VariableDeclaration<'_>, tail: &[Statement<'_>]) {
        if next.kind == VariableDeclarationKind::Let
            && next.declarations.iter().all(|item| item.init.is_none())
        {
            for (index, item) in next.declarations.iter().enumerate() {
                let BindingPattern::BindingIdentifier(id) = &item.id else {
                    continue;
                };
                let Some(symbol) = id.symbol_id.get() else {
                    continue;
                };
                let removal = if index == 0 {
                    Span::new(item.span.start, next.declarations[1].span.start)
                } else {
                    Span::new(next.declarations[index - 1].span.end, item.span.end)
                };
                self.branch_successor(next, removal, symbol, tail);
            }
        }
    }
    /// Store the snapshot `next = init` into an input whose live range ends
    /// in `init`, or into a cell whose live range already ended.
    fn last_input(
        &mut self,
        next: &VariableDeclaration<'_>,
        id: &BindingIdentifier<'_>,
        symbol: SymbolId,
        init: &Expression<'_>,
    ) {
        let mut value = init;
        while let Expression::ParenthesizedExpression(p) = value {
            value = &p.expression;
        }
        if matches!(
            value,
            Expression::FunctionExpression(_)
                | Expression::ArrowFunctionExpression(_)
                | Expression::ClassExpression(_)
        ) {
            return;
        }
        let reads: Vec<_> = self.scoping.get_resolved_references(symbol).collect();
        if reads.is_empty()
            || !reads.iter().all(|r| {
                (!r.is_write()
                    || next.kind == VariableDeclarationKind::Let
                        && safe_later_write(self.nodes.parent_kind(r.node_id())))
                    && self.nodes.get_node(r.node_id()).kind().span().start >= next.span.end
            })
        {
            return;
        }
        // Consider inputs that actually occur in this RHS.
        let mut inputs = Inputs {
            scoping: self.scoping,
            symbols: BTreeSet::new(),
        };
        inputs.visit_expression(init);
        let mut candidates: Vec<_> = inputs
            .symbols
            .iter()
            .copied()
            .filter_map(|s| self.declarations.get(&s).map(|d| (s, *d)))
            .collect();
        // Prefer data-dependent accumulator chains. Otherwise
        // a cell whose complete live range already ended can
        // hold this later snapshot: this deletes storage, not
        // merely a generated name. No value is rematerialized.
        candidates.extend(self.ended_candidates(next.span, &inputs.symbols));
        for (input, previous) in candidates {
            if input == symbol
                || previous.span.end > next.span.start
                || !generated(self.scoping.symbol_name(input))
                || !self.private(input, next.span)
                || previous.kind == VariableDeclarationKind::Const
                    && self
                        .scoping
                        .get_resolved_references(input)
                        .any(|r| r.is_write())
                || !self.locals.same_iteration(previous.span, next.span)
                || self.scoping.get_resolved_references(input).any(|r| {
                    self.nodes.get_node(r.node_id()).kind().span().end > next.span.end
                })
                || reads.iter().any(|r| {
                    self.scoping
                        .find_binding(r.scope_id(), self.scoping.symbol_ident(input))
                        != Some(input)
                })
            {
                continue;
            }
            // The final RHS may read or update this input. Its
            // new result is assigned only after RHS evaluation,
            // exactly when the original snapshot initialized.
            let mut edits = vec![analysis::Edit::replace(
                next.span,
                format!(
                    "{} = ({});",
                    self.scoping.symbol_name(input),
                    &self.source[init.span().start as usize..init.span().end as usize]
                ),
            )];
            self.retarget(&mut edits, previous, &reads, &id.name, input);
            if self.lend(next.span, edits, input, symbol) {
                break;
            }
        }
    }

    pub(super) fn branch_successor(
        &mut self,
        next: &VariableDeclaration<'_>,
        removal: Span,
        symbol: SymbolId,
        tail: &[Statement<'_>],
    ) {
        let init = next.declarations[0].init.as_ref();
        let mut value = init;
        while let Some(Expression::ParenthesizedExpression(p)) = value {
            value = Some(&p.expression);
        }
        if !matches!(
            next.kind,
            VariableDeclarationKind::Let | VariableDeclarationKind::Const
        ) || init.is_none() && next.kind != VariableDeclarationKind::Let
            || matches!(
                value,
                Some(
                    Expression::FunctionExpression(_)
                        | Expression::ArrowFunctionExpression(_)
                        | Expression::ClassExpression(_)
                )
            )
            || !generated(self.scoping.symbol_name(symbol))
            || !self.private(symbol, Span::new(next.span.end, next.span.end))
        {
            return;
        }
        let references: Vec<_> = self.scoping.get_resolved_references(symbol).collect();
        if references.is_empty()
            || references.iter().any(|r| {
                self.nodes.get_node(r.node_id()).kind().span().start < next.span.end
                    || r.is_write()
                        && (next.kind == VariableDeclarationKind::Const
                            || !safe_later_write(self.nodes.parent_kind(r.node_id())))
            })
        {
            return;
        }
        let last = references
            .iter()
            .map(|r| self.nodes.get_node(r.node_id()).kind().span().end)
            .max()
            .unwrap();
        let Some(end) = tail.iter().position(|s| s.span().end >= last) else {
            return;
        };
        let candidates = self.successor_candidates(next, init, &tail[..=end]);
        for input in candidates {
            let Some(previous) = self.declarations.get(&input).copied() else {
                continue;
            };
            if input == symbol
                || previous.span.end > next.span.start && previous.span != next.span
                || !generated(self.scoping.symbol_name(input))
                || !self.private(
                    input,
                    if previous.span == next.span {
                        Span::new(next.span.end, next.span.end)
                    } else {
                        next.span
                    },
                )
                || !self.locals.same_iteration(previous.span, next.span)
                || previous.kind == VariableDeclarationKind::Const
                    && self
                        .scoping
                        .get_resolved_references(input)
                        .any(|r| r.is_write())
                || references.iter().any(|r| {
                    self.scoping
                        .find_binding(r.scope_id(), self.scoping.symbol_ident(input))
                        != Some(input)
                })
            {
                continue;
            }
            let Some(reset) = self.successor_proof(next, symbol, tail, input, previous, last)
            else {
                continue;
            };
            let mut edits = self.successor_store(next, removal, input, reset);
            self.retarget(
                &mut edits,
                previous,
                &references,
                self.scoping.symbol_name(symbol),
                input,
            );
            if self.lend(next.span, edits, input, symbol) {
                break;
            }
        }
    }
    /// Cells that may lend storage to `next`: inputs read by its initializer
    /// or by `interval`, ended cells, and undefined cells never written yet.
    fn successor_candidates(
        &self,
        next: &VariableDeclaration<'_>,
        init: Option<&Expression<'_>>,
        interval: &[Statement<'_>],
    ) -> Vec<SymbolId> {
        let mut inputs = Inputs {
            scoping: self.scoping,
            symbols: BTreeSet::new(),
        };
        if let Some(init) = init {
            inputs.visit_expression(init);
        }
        for statement in interval {
            inputs.visit_statement(statement);
        }
        let mut candidates: Vec<_> = inputs.symbols.iter().copied().collect();
        candidates.extend(
            self.ended_candidates(next.span, &inputs.symbols)
                .into_iter()
                .map(|(s, _)| s),
        );
        if self.ended {
            for (&s, &d) in &self.declarations {
                let ready = Span::new(next.span.end, next.span.end);
                if !candidates.contains(&s)
                    && d.undefined
                    && d.span.start <= next.span.start
                    && self.locals.initialized(s, ready)
                    && self.locals.same_iteration(d.span, next.span)
                    && !self.scoping.get_resolved_references(s).any(|r| {
                        r.is_write()
                            && self.nodes.get_node(r.node_id()).kind().span().start < next.span.end
                    })
                {
                    candidates.push(s);
                }
            }
        }
        candidates
    }
    /// Prove that `symbol`'s versions may share `input`'s storage until
    /// both are dead. `Some(reset)` says whether `input`'s old value ended
    /// before an uninitialized `next`, which then stores undefined.
    fn successor_proof(
        &self,
        next: &VariableDeclaration<'_>,
        symbol: SymbolId,
        tail: &[Statement<'_>],
        input: SymbolId,
        previous: SourceDeclaration,
        last: u32,
    ) -> Option<bool> {
        let init = next.declarations[0].init.as_ref();
        // A copy-back can make the original input observable again. Prove
        // all those later observations too, not just successor references.
        let last_input = self
            .scoping
            .get_resolved_references(input)
            .map(|r| self.nodes.get_node(r.node_id()).kind().span().end)
            .max()
            .unwrap_or(last);
        let reset = init.is_none() && self.ended && last_input <= next.span.start;
        let undefined = init.is_none()
            && self.ended
            && previous.undefined
            && !self.scoping.get_resolved_references(input).any(|r| {
                r.is_write()
                    && self.nodes.get_node(r.node_id()).kind().span().start < next.span.end
            });
        let end = tail
            .iter()
            .position(|s| s.span().end >= last.max(last_input))?;
        let interval = &tail[..=end];
        let mut proof = SuccessorProof {
            scoping: self.scoping,
            nodes: self.nodes,
            input,
            target: symbol,
            state: SuccessorState {
                assigned: reset || undefined,
                old_available: !reset,
                live: true,
            },
            valid: true,
            loops: vec![],
        };
        if let Some(init) = init {
            proof.visit_expression(init);
            proof.store(true, proof.copied(init));
        }
        for statement in interval {
            if proof.state.live {
                proof.visit_statement(statement);
            }
        }
        proof.valid.then_some(reset)
    }
    /// The edit that moves `next`'s creation into `input`'s storage.
    fn successor_store(
        &self,
        next: &VariableDeclaration<'_>,
        removal: Span,
        input: SymbolId,
        reset: bool,
    ) -> Vec<analysis::Edit> {
        if let Some(init) = next.declarations[0].init.as_ref() {
            vec![analysis::Edit::replace(
                next.span,
                format!(
                    "{} = ({});",
                    self.scoping.symbol_name(input),
                    &self.source[init.span().start as usize..init.span().end as usize]
                ),
            )]
        } else if reset {
            // `let next;` initializes undefined even on paths without a
            // later store. Preserve that initialization in borrowed storage.
            let mut remaining =
                self.source[next.span.start as usize..next.span.end as usize].to_owned();
            remaining.replace_range(
                (removal.start - next.span.start) as usize
                    ..(removal.end - next.span.start) as usize,
                "",
            );
            vec![analysis::Edit::replace(
                next.span,
                format!("{remaining}\n{} = void 0;", self.scoping.symbol_name(input)),
            )]
        } else {
            vec![analysis::Edit::replace(removal, "")]
        }
    }
}

impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_for_statement(&mut self, s: &ForStatement<'a>) {
        self.for_header(s);
        walk::walk_for_statement(self, s);
    }
    fn visit_function_body(&mut self, b: &FunctionBody<'a>) {
        self.body(&b.statements);
        walk::walk_function_body(self, b);
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        self.body(&b.body);
        walk::walk_block_statement(self, b);
    }
}
