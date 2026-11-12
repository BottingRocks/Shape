//! Proof that no later use sees the reused storage: successors and loop edges.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SuccessorState {
    pub(super) assigned: bool,
    pub(super) old_available: bool,
    pub(super) live: bool,
}
impl SuccessorState {
    const DEAD: Self = Self {
        assigned: false,
        old_available: false,
        live: false,
    };
    pub(super) fn merge(a: Self, b: Self) -> Self {
        if !a.live {
            return b;
        }
        if !b.live {
            return a;
        }
        Self {
            assigned: a.assigned && b.assigned,
            old_available: a.old_available && b.old_available,
            live: true,
        }
    }
}
pub(super) struct SuccessorProof<'s, 'a> {
    pub(super) scoping: &'s Scoping,
    pub(super) nodes: &'s AstNodes<'a>,
    pub(super) input: SymbolId,
    pub(super) target: SymbolId,
    pub(super) state: SuccessorState,
    pub(super) valid: bool,
    pub(super) loops: Vec<LoopEdges>,
}
#[derive(Default)]
pub(super) struct LoopEdges {
    pub(super) breaks: Option<SuccessorState>,
    pub(super) continues: Option<SuccessorState>,
}
impl LoopEdges {
    pub(super) fn add(edge: &mut Option<SuccessorState>, state: SuccessorState) {
        *edge = Some(edge.map_or(state, |before| SuccessorState::merge(before, state)));
    }
}
impl<'a> SuccessorProof<'_, 'a> {
    // A continuation copy often lives on the true side of `test && (...)`.
    // Joining both outcomes before forming the backedge loses that equality.
    // Keep truth outcomes separate without moving or evaluating any producer.
    pub(super) fn condition(&mut self, expression: &Expression<'a>) -> (SuccessorState, SuccessorState) {
        if !self.state.live {
            return (SuccessorState::DEAD, SuccessorState::DEAD);
        }
        let outcomes = match expression {
            Expression::ParenthesizedExpression(p) => self.condition(&p.expression),
            Expression::BooleanLiteral(b) => {
                if b.value {
                    (self.state, SuccessorState::DEAD)
                } else {
                    (SuccessorState::DEAD, self.state)
                }
            }
            Expression::LogicalExpression(l) if l.operator.is_and() => {
                let (yes, no) = self.condition(&l.left);
                self.state = yes;
                let (right_yes, right_no) = self.condition(&l.right);
                (right_yes, SuccessorState::merge(no, right_no))
            }
            Expression::LogicalExpression(l) if l.operator.is_or() => {
                let (yes, no) = self.condition(&l.left);
                self.state = no;
                let (right_yes, right_no) = self.condition(&l.right);
                (SuccessorState::merge(yes, right_yes), right_no)
            }
            Expression::UnaryExpression(u) if u.operator.is_not() => {
                let (yes, no) = self.condition(&u.argument);
                (no, yes)
            }
            Expression::SequenceExpression(s) => {
                let Some((last, prefix)) = s.expressions.split_last() else {
                    self.valid = false;
                    return (self.state, self.state);
                };
                for expression in prefix {
                    self.visit_expression(expression);
                }
                self.condition(last)
            }
            Expression::ConditionalExpression(c) => {
                let (yes, no) = self.condition(&c.test);
                self.state = yes;
                let (yes_yes, yes_no) = self.condition(&c.consequent);
                self.state = no;
                let (no_yes, no_no) = self.condition(&c.alternate);
                (
                    SuccessorState::merge(yes_yes, no_yes),
                    SuccessorState::merge(yes_no, no_no),
                )
            }
            _ => {
                self.visit_expression(expression);
                (self.state, self.state)
            }
        };
        self.state = SuccessorState::merge(outcomes.0, outcomes.1);
        outcomes
    }
    pub(super) fn copied(&self, mut rhs: &Expression<'_>) -> Option<SymbolId> {
        while let Expression::ParenthesizedExpression(p) = rhs {
            rhs = &p.expression;
        }
        if let Expression::Identifier(id) = rhs {
            analysis::identifier_symbol(id, self.scoping)
        } else {
            None
        }
    }
    pub(super) fn store(&mut self, target: bool, copied: Option<SymbolId>) {
        if target {
            self.state.old_available &=
                copied == Some(self.input) || copied == Some(self.target) && self.state.assigned;
            self.state.assigned = true;
        } else {
            self.state.assigned &= copied == Some(self.target)
                || copied == Some(self.input) && self.state.old_available;
            self.state.old_available = true;
        }
    }
    pub(super) fn repeat(
        &mut self,
        body: &Statement<'a>,
        test: Option<&Expression<'a>>,
        update: Option<&Expression<'a>>,
        body_first: bool,
    ) {
        let entry = self.state;
        let mut head = entry;
        loop {
            self.state = head;
            self.loops.push(LoopEdges::default());
            let mut exits = SuccessorState::DEAD;
            if !body_first {
                if let Some(test) = test {
                    let (yes, no) = self.condition(test);
                    self.state = yes;
                    exits = no;
                }
            }
            if self.state.live {
                self.visit_statement(body);
            }
            let edges = self.loops.pop().expect("loop proof frame");
            if let Some(continued) = edges.continues {
                self.state = SuccessorState::merge(self.state, continued);
            }
            if self.state.live {
                if let Some(update) = update {
                    self.visit_expression(update);
                }
                if body_first {
                    if let Some(test) = test {
                        let (yes, no) = self.condition(test);
                        self.state = yes;
                        exits = no;
                    }
                }
            }
            if let Some(broken) = edges.breaks {
                exits = SuccessorState::merge(exits, broken);
            }
            // The loop header must hold on first entry AND every backedge.
            // There are only two availability bits, each of which can be lost
            // once. Revisit all reads after weakening either invariant; a copy
            // on one path cannot justify a read on a different iteration.
            let next = SuccessorState::merge(entry, self.state);
            if !self.valid || next == head {
                self.state = exits;
                return;
            }
            head = next;
        }
    }
    pub(super) fn invariant_region(&mut self, span: Span) {
        // Opaque regions may jump past a later copy-back. Until their abrupt
        // edges are modeled, do not silently treat them as falling through a
        // surrounding loop. Nested-loop/switch breaks conservatively decline.
        if !self.loops.is_empty()
            && self.nodes.iter().any(|n| {
                matches!(
                    n.kind(),
                    AstKind::BreakStatement(_) | AstKind::ContinueStatement(_)
                ) && span.start <= n.kind().span().start
                    && n.kind().span().end <= span.end
            })
        {
            self.valid = false;
            return;
        }
        // A region may read both equal cells or operate on just one available
        // cell. Any write invalidates the other cached version, including when
        // the region has a conditional/zero-iteration execution path.
        let refs = |symbol| {
            let mut read = false;
            let mut write = false;
            for r in self.scoping.get_resolved_references(symbol) {
                let at = self.nodes.get_node(r.node_id()).kind().span();
                if span.start <= at.start && at.end <= span.end {
                    read |= r.is_read();
                    write |= r.is_write();
                }
            }
            (read, write)
        };
        let (input_read, input_write) = refs(self.input);
        let (target_read, target_write) = refs(self.target);
        let input = input_read || input_write;
        let target = target_read || target_write;
        if input && !self.state.old_available
            || target && !self.state.assigned
            || input && target && (input_write || target_write)
        {
            self.valid = false;
            return;
        }
        if input_write {
            self.state.assigned = false;
        }
        if target_write {
            self.state.old_available = false;
        }
    }
}
impl<'a> Visit<'a> for SuccessorProof<'_, 'a> {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        let Some(r) = id.reference_id.get().map(|r| self.scoping.get_reference(r)) else {
            return;
        };
        match analysis::identifier_symbol(id, self.scoping) {
            Some(s) if s == self.input => {
                if r.is_write() || r.is_read() && !self.state.old_available {
                    self.valid = false;
                }
            }
            Some(s) if s == self.target => {
                if r.is_read() && !self.state.assigned {
                    self.valid = false;
                }
            }
            _ => {}
        }
    }
    fn visit_assignment_expression(&mut self, a: &AssignmentExpression<'a>) {
        let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
            walk::walk_assignment_expression(self, a);
            return;
        };
        let Some(destination) = analysis::identifier_symbol(id, self.scoping)
            .filter(|s| *s == self.input || *s == self.target)
        else {
            walk::walk_assignment_expression(self, a);
            return;
        };
        let target = destination == self.target;
        if a.operator != AssignmentOperator::Assign
            && !(if target {
                self.state.assigned
            } else {
                self.state.old_available
            })
        {
            self.valid = false;
        }
        let before = self.state;
        self.visit_expression(&a.right);
        let copied = if a.operator == AssignmentOperator::Assign {
            self.copied(&a.right)
        } else {
            None
        };
        self.store(target, copied);
        if a.operator.is_logical() {
            self.state = SuccessorState::merge(before, self.state);
        }
    }
    fn visit_update_expression(&mut self, u: &UpdateExpression<'a>) {
        let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &u.argument else {
            walk::walk_update_expression(self, u);
            return;
        };
        match analysis::identifier_symbol(id, self.scoping) {
            Some(s) if s == self.input => {
                self.valid &= self.state.old_available;
                self.state.old_available = true;
                self.state.assigned = false;
            }
            Some(s) if s == self.target => {
                self.valid &= self.state.assigned;
                self.state.assigned = true;
                self.state.old_available = false;
            }
            _ => walk::walk_update_expression(self, u),
        }
    }
    fn visit_if_statement(&mut self, i: &IfStatement<'a>) {
        let (yes, no) = self.condition(&i.test);
        self.state = yes;
        if self.state.live {
            self.visit_statement(&i.consequent);
        }
        let after_yes = self.state;
        self.state = no;
        if self.state.live {
            if let Some(no) = &i.alternate {
                self.visit_statement(no);
            }
        }
        self.state = SuccessorState::merge(after_yes, self.state);
    }
    fn visit_conditional_expression(&mut self, c: &ConditionalExpression<'a>) {
        let (yes, no) = self.condition(&c.test);
        self.state = yes;
        if self.state.live {
            self.visit_expression(&c.consequent);
        }
        let after_yes = self.state;
        self.state = no;
        if self.state.live {
            self.visit_expression(&c.alternate);
        }
        self.state = SuccessorState::merge(after_yes, self.state);
    }
    fn visit_logical_expression(&mut self, l: &LogicalExpression<'a>) {
        if l.operator.is_coalesce() {
            self.visit_expression(&l.left);
            let before = self.state;
            self.visit_expression(&l.right);
            self.state = SuccessorState::merge(before, self.state);
        } else {
            let (yes, no) = self.condition(&l.left);
            let skipped = if l.operator.is_and() { no } else { yes };
            self.state = if l.operator.is_and() { yes } else { no };
            if self.state.live {
                self.visit_expression(&l.right);
            }
            self.state = SuccessorState::merge(skipped, self.state);
        }
    }
    fn visit_chain_expression(&mut self, c: &ChainExpression<'a>) {
        let before = self.state;
        walk::walk_chain_expression(self, c);
        self.state = SuccessorState::merge(before, self.state);
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        for s in &b.body {
            if self.state.live {
                self.visit_statement(s);
            }
        }
    }
    fn visit_return_statement(&mut self, r: &ReturnStatement<'a>) {
        if let Some(e) = &r.argument {
            self.visit_expression(e);
        }
        self.state.live = false;
    }
    fn visit_throw_statement(&mut self, r: &ThrowStatement<'a>) {
        self.visit_expression(&r.argument);
        self.state.live = false;
    }
    fn visit_break_statement(&mut self, s: &BreakStatement<'a>) {
        if s.label.is_some() {
            self.valid = false;
        } else if let Some(edges) = self.loops.last_mut() {
            LoopEdges::add(&mut edges.breaks, self.state);
            self.state.live = false;
        } else {
            self.valid = false;
        }
    }
    fn visit_continue_statement(&mut self, s: &ContinueStatement<'a>) {
        if s.label.is_some() {
            self.valid = false;
        } else if let Some(edges) = self.loops.last_mut() {
            LoopEdges::add(&mut edges.continues, self.state);
            self.state.live = false;
        } else {
            self.valid = false;
        }
    }
    fn visit_for_statement(&mut self, s: &ForStatement<'a>) {
        if let Some(init) = &s.init {
            self.visit_for_statement_init(init);
        }
        self.repeat(&s.body, s.test.as_ref(), s.update.as_ref(), false);
    }
    fn visit_while_statement(&mut self, s: &WhileStatement<'a>) {
        self.repeat(&s.body, Some(&s.test), None, false);
    }
    fn visit_do_while_statement(&mut self, s: &DoWhileStatement<'a>) {
        self.repeat(&s.body, Some(&s.test), None, true);
    }
    fn visit_for_in_statement(&mut self, s: &ForInStatement<'a>) {
        self.invariant_region(s.span);
    }
    fn visit_for_of_statement(&mut self, s: &ForOfStatement<'a>) {
        self.invariant_region(s.span);
    }
    fn visit_try_statement(&mut self, s: &TryStatement<'a>) {
        self.invariant_region(s.span);
    }
    fn visit_switch_statement(&mut self, s: &SwitchStatement<'a>) {
        self.invariant_region(s.span);
    }
    fn visit_labeled_statement(&mut self, s: &LabeledStatement<'a>) {
        self.invariant_region(s.span);
    }
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
    fn visit_class(&mut self, _: &Class<'a>) {
        self.valid = false;
    }
}
