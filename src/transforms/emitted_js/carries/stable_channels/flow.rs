//! The flow walk: which values reach a read, loops and exceptions included.

use super::*;

pub(super) struct Flow<'s> {
    pub(super) cells: &'s super::super::super::cell_facts::Facts,
    pub(super) copy_out: Option<Span>,
    pub(super) token: Option<super::super::super::cell_facts::Token>,
    pub(super) primitive_snapshot: bool,
    pub(super) scoping: &'s Scoping,
    pub(super) target: SymbolId,
    pub(super) input: Input,
    pub(super) effects: bool,
    pub(super) lexical: &'s BTreeSet<String>,
    pub(super) locals: &'s LocalReads,
    pub(super) safe_arguments: bool,
    pub(super) reads: &'s [Span],
    pub(super) mutations: Vec<Span>,
    pub(super) equal: bool,
    pub(super) live: bool,
    pub(super) valid: bool,
}
#[derive(Default)]
pub(super) struct LoopAbrupt {
    pub(super) unsupported: bool,
    pub(super) continues: bool,
}
impl<'a> Visit<'a> for LoopAbrupt {
    fn visit_break_statement(&mut self, _: &BreakStatement<'a>) {
        self.unsupported = true;
    }
    fn visit_continue_statement(&mut self, c: &ContinueStatement<'a>) {
        self.unsupported |= c.label.is_some();
        self.continues = true;
    }
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
    fn visit_class(&mut self, _: &Class<'a>) {}
}
impl Flow<'_> {
    // Result categories alone are not an effect proof. Child visits still
    // invalidate equality for their getters, calls and coercions.
    pub(super) fn primitive(&self, e: &Expression<'_>) -> bool {
        if self
            .cells
            .values
            .get(&e.span())
            .is_some_and(|v| v.primitive)
            || self.primitive_snapshot
                && matches!(unwrap(e), Expression::Identifier(id)
                if analysis::identifier_symbol(id,self.scoping) == Some(self.target))
        {
            return true;
        }
        match unwrap(e) {
            Expression::NumericLiteral(_)
            | Expression::StringLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_) => true,
            Expression::Identifier(id) => analysis::identifier_symbol(id, self.scoping)
                .is_some_and(|s| self.locals.primitive_cell(s, id.span)),
            Expression::UnaryExpression(u) => {
                matches!(
                    u.operator,
                    UnaryOperator::UnaryPlus
                        | UnaryOperator::LogicalNot
                        | UnaryOperator::Typeof
                        | UnaryOperator::Void
                        | UnaryOperator::Delete
                ) || self.primitive(&u.argument)
            }
            Expression::BinaryExpression(b) => {
                b.operator.is_compare()
                    || b.operator.is_equality()
                    || b.operator.is_in()
                    || b.operator.is_instance_of()
                    || b.operator == BinaryOperator::ShiftRightZeroFill
                    || self.primitive(&b.left)
                    || self.primitive(&b.right)
            }
            Expression::LogicalExpression(l) => self.primitive(&l.left) && self.primitive(&l.right),
            Expression::ConditionalExpression(c) => {
                self.primitive(&c.consequent) && self.primitive(&c.alternate)
            }
            Expression::SequenceExpression(s) => {
                s.expressions.last().is_some_and(|e| self.primitive(e))
            }
            _ => false,
        }
    }

    pub(super) fn loop_region(&mut self, span: Span) {
        let stable = !self.effects
            && self
                .mutations
                .iter()
                .all(|s| !(span.start <= s.start && s.end <= span.end));
        // Every candidate write already copies this exact input. If equality
        // holds on entry and the input cannot change, it is a loop invariant.
        if stable {
            if !self.equal
                && self
                    .reads
                    .iter()
                    .any(|r| span.start <= r.start && r.end <= span.end)
            {
                self.valid = false;
            }
        } else {
            self.unsupported(span);
        }
    }
    pub(super) fn input_unchanged(&self, span: Span) -> bool {
        !self.effects
            && self
                .mutations
                .iter()
                .all(|m| !(span.start <= m.start && m.end <= span.end))
    }
    pub(super) fn unsupported(&mut self, span: Span) {
        if self
            .reads
            .iter()
            .any(|r| span.start <= r.start && r.end <= span.end)
        {
            self.valid = false;
        }
        self.equal = false;
    }
    pub(super) fn merge(&mut self, a: (bool, bool), b: (bool, bool)) {
        self.live = a.1 || b.1;
        self.equal = (!a.1 || a.0) && (!b.1 || b.0);
    }
}
impl<'a> Visit<'a> for Flow<'_> {
    fn visit_function_body(&mut self, b: &FunctionBody<'a>) {
        for s in &b.statements {
            if !self.live {
                break;
            }
            self.visit_statement(s);
        }
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        for s in &b.body {
            if !self.live {
                break;
            }
            self.visit_statement(s);
        }
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        if self.effects
            && analysis::identifier_symbol(id, self.scoping).is_none()
            && !self.lexical.contains(id.name.as_str())
            && !(self.safe_arguments && id.name == "arguments")
        {
            self.equal = false;
        }
        if analysis::identifier_symbol(id, self.scoping) == Some(self.target)
            && self.reads.contains(&id.span)
            && !self.equal
        {
            self.valid = false;
        }
    }
    fn visit_assignment_expression(&mut self, a: &AssignmentExpression<'a>) {
        if self.copy_out == Some(a.span) {
            self.equal = true;
            return;
        }
        if a.operator != AssignmentOperator::Assign
            || matches!(
                a.left,
                AssignmentTarget::ArrayAssignmentTarget(_)
                    | AssignmentTarget::ObjectAssignmentTarget(_)
            )
        {
            self.unsupported(a.span);
            return;
        }
        // Target reference evaluation precedes RHS; identifier stores happen
        // after RHS. Member targets cannot mutate a private lexical input.
        self.visit_assignment_target(&a.left);
        self.visit_expression(&a.right);
        if self.effects && !matches!(a.left, AssignmentTarget::AssignmentTargetIdentifier(_)) {
            self.equal = false;
        }
        if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
            if self.effects
                && analysis::identifier_symbol(id, self.scoping).is_none()
                && !self.lexical.contains(id.name.as_str())
            {
                self.equal = false;
            }
            let s = analysis::identifier_symbol(id, self.scoping);
            if s == Some(self.target) {
                self.equal = a.operator == AssignmentOperator::Assign
                    && input(&a.right, self.scoping) == Some(self.input);
            } else if matches!(self.input,Input::Local(i) if s==Some(i))
                || matches!(self.input,Input::Incoming(i) if s.is_none() && id.name.as_str() == format!("$KEY_{i}"))
            {
                self.equal = self.token.as_ref().is_some_and(|t| {
                    self.cells
                        .values
                        .get(&a.right.span())
                        .and_then(|v| v.token.as_ref())
                        == Some(t)
                });
            }
        }
    }
    fn visit_update_expression(&mut self, u: &UpdateExpression<'a>) {
        walk::walk_update_expression(self, u);
        if self.effects {
            self.equal = false;
        }
        if let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &u.argument {
            if matches!(self.input,Input::Local(s) if analysis::identifier_symbol(id,self.scoping)==Some(s))
            {
                self.equal = false;
            }
        }
    }
    fn visit_variable_declarator(&mut self, d: &VariableDeclarator<'a>) {
        if self.copy_out == Some(d.span) {
            self.equal = true;
            return;
        }
        if self.effects && !matches!(d.id, BindingPattern::BindingIdentifier(_)) {
            self.equal = false;
        }
        walk::walk_variable_declarator(self, d);
        if self.effects && !matches!(d.id, BindingPattern::BindingIdentifier(_)) {
            self.equal = false;
        }
        if let BindingPattern::BindingIdentifier(id) = &d.id {
            if id.symbol_id.get() == Some(self.target) {
                self.equal = d
                    .init
                    .as_ref()
                    .is_some_and(|e| input(e, self.scoping) == Some(self.input));
            } else if matches!(self.input,Input::Local(s) if id.symbol_id.get()==Some(s)) {
                self.equal = false;
            }
        }
    }
    fn visit_if_statement(&mut self, i: &IfStatement<'a>) {
        self.visit_expression(&i.test);
        let before = (self.equal, self.live);
        self.visit_statement(&i.consequent);
        let yes = (self.equal, self.live);
        (self.equal, self.live) = before;
        if let Some(no) = &i.alternate {
            self.visit_statement(no);
        }
        self.merge(yes, (self.equal, self.live));
    }
    fn visit_conditional_expression(&mut self, c: &ConditionalExpression<'a>) {
        self.visit_expression(&c.test);
        let before = (self.equal, self.live);
        self.visit_expression(&c.consequent);
        let yes = (self.equal, self.live);
        (self.equal, self.live) = before;
        self.visit_expression(&c.alternate);
        self.merge(yes, (self.equal, self.live));
    }
    fn visit_logical_expression(&mut self, l: &LogicalExpression<'a>) {
        self.visit_expression(&l.left);
        let before = (self.equal, self.live);
        self.visit_expression(&l.right);
        self.merge(before, (self.equal, self.live));
    }

    fn visit_call_expression(&mut self, c: &CallExpression<'a>) {
        walk::walk_call_expression(self, c);
        if self.effects {
            self.equal = false;
        }
    }
    fn visit_new_expression(&mut self, c: &NewExpression<'a>) {
        walk::walk_new_expression(self, c);
        if self.effects {
            self.equal = false;
        }
    }
    fn visit_tagged_template_expression(&mut self, t: &TaggedTemplateExpression<'a>) {
        walk::walk_tagged_template_expression(self, t);
        if self.effects {
            self.equal = false;
        }
    }
    fn visit_static_member_expression(&mut self, m: &StaticMemberExpression<'a>) {
        walk::walk_static_member_expression(self, m);
        if self.effects {
            self.equal = false;
        }
    }
    fn visit_computed_member_expression(&mut self, m: &ComputedMemberExpression<'a>) {
        walk::walk_computed_member_expression(self, m);
        if self.effects
            && !(self.safe_arguments
                && !m.optional
                && matches!(unwrap(&m.object),Expression::Identifier(id) if id.name=="arguments"&&analysis::identifier_symbol(id,self.scoping).is_none())
                && matches!(unwrap(&m.expression),Expression::NumericLiteral(n) if n.value>=0.0&&n.value<65536.0&&n.value.fract()==0.0))
        {
            self.equal = false;
        }
    }
    fn visit_binary_expression(&mut self, b: &BinaryExpression<'a>) {
        walk::walk_binary_expression(self, b);
        if self.effects
            && !(matches!(
                b.operator,
                BinaryOperator::StrictEquality | BinaryOperator::StrictInequality
            ) || matches!(
                b.operator,
                BinaryOperator::Equality | BinaryOperator::Inequality
            ) && (matches!(unwrap(&b.left), Expression::NullLiteral(_))
                || matches!(unwrap(&b.right), Expression::NullLiteral(_)))
                || !b.operator.is_in()
                    && !b.operator.is_instance_of()
                    && self.primitive(&b.left)
                    && self.primitive(&b.right))
        {
            self.equal = false;
        }
    }
    fn visit_unary_expression(&mut self, u: &UnaryExpression<'a>) {
        walk::walk_unary_expression(self, u);
        if self.effects
            && !(matches!(
                u.operator,
                UnaryOperator::LogicalNot | UnaryOperator::Typeof | UnaryOperator::Void
            ) || u.operator != UnaryOperator::Delete && self.primitive(&u.argument))
        {
            self.equal = false;
        }
    }
    fn visit_spread_element(&mut self, s: &SpreadElement<'a>) {
        walk::walk_spread_element(self, s);
        if self.effects {
            self.equal = false;
        }
    }
    fn visit_object_property(&mut self, p: &ObjectProperty<'a>) {
        self.visit_property_key(&p.key);
        if self.effects && p.computed {
            self.equal = false;
        }
        self.visit_expression(&p.value);
    }
    fn visit_await_expression(&mut self, e: &AwaitExpression<'a>) {
        walk::walk_await_expression(self, e);
        if self.effects {
            self.equal = false;
        }
    }
    fn visit_yield_expression(&mut self, e: &YieldExpression<'a>) {
        walk::walk_yield_expression(self, e);
        if self.effects {
            self.equal = false;
        }
    }
    fn visit_template_literal(&mut self, t: &TemplateLiteral<'a>) {
        for e in &t.expressions {
            self.visit_expression(e);
            if self.effects && !self.primitive(e) {
                self.equal = false;
            }
        }
    }
    fn visit_chain_expression(&mut self, c: &ChainExpression<'a>) {
        // Conditional stores only copy this same input. Existing equality
        // survives every skipped/evaluated path if that input cannot change.
        self.loop_region(c.span);
    }
    fn visit_return_statement(&mut self, r: &ReturnStatement<'a>) {
        if let Some(e) = &r.argument {
            self.visit_expression(e);
        }
        self.live = false;
    }
    fn visit_throw_statement(&mut self, r: &ThrowStatement<'a>) {
        self.visit_expression(&r.argument);
        self.live = false;
    }
    fn visit_while_statement(&mut self, s: &WhileStatement<'a>) {
        let mut abrupt = LoopAbrupt::default();
        abrupt.visit_statement(&s.body);
        if abrupt.unsupported {
            self.loop_region(s.span);
            return;
        }
        let before = (self.equal, self.live, self.valid);
        // Analyze a test from the least state. If it reestablishes equality
        // without first reading the old channel, that proof holds on entry,
        // every backedge and the final failing test. Body stores cannot
        // invalidate the exit value because the test copies again on exit.
        self.equal = false;
        self.visit_expression(&s.test);
        if !self.equal || !self.valid {
            (self.equal, self.live, self.valid) = before;
            self.loop_region(s.span);
            return;
        }
        self.visit_statement(&s.body);
        self.equal = true;
        self.live = before.1;
    }
    fn visit_do_while_statement(&mut self, s: &DoWhileStatement<'a>) {
        let mut abrupt = LoopAbrupt::default();
        abrupt.visit_statement(&s.body);
        if abrupt.unsupported || abrupt.continues {
            self.loop_region(s.span);
            return;
        }
        let before = (self.equal, self.live, self.valid);
        self.visit_statement(&s.body);
        let body = (self.equal, self.live, self.valid);
        // The test must refresh from any body exit, before any channel read.
        self.equal = false;
        self.live = true;
        self.visit_expression(&s.test);
        if !self.equal || !self.valid {
            (self.equal, self.live, self.valid) = before;
            self.loop_region(s.span);
            return;
        }
        // Prove the next iteration from the refreshed test state too. This
        // visit validates source reads only; it executes no JavaScript.
        self.visit_statement(&s.body);
        self.valid &= body.2;
        self.equal = true;
        self.live = before.1 && body.1;
    }
    fn visit_for_statement(&mut self, s: &ForStatement<'a>) {
        let mut abrupt = LoopAbrupt::default();
        abrupt.visit_statement(&s.body);
        let Some(test) = &s.test else {
            self.loop_region(s.span);
            return;
        };
        if abrupt.unsupported
            || abrupt.continues
                && s.update.as_ref().is_some_and(|u| {
                    self.reads
                        .iter()
                        .any(|r| u.span().start <= r.start && r.end <= u.span().end)
                })
        {
            self.loop_region(s.span);
            return;
        }
        if let Some(init) = &s.init {
            if let ForStatementInit::VariableDeclaration(d) = init {
                self.visit_variable_declaration(d);
            } else if let Some(e) = init.as_expression() {
                self.visit_expression(e);
            }
        }
        let before = (self.equal, self.live, self.valid);
        self.equal = false;
        self.visit_expression(test);
        if !self.equal || !self.valid {
            (self.equal, self.live, self.valid) = before;
            self.loop_region(s.span);
            return;
        }
        self.visit_statement(&s.body);
        if self.live {
            if let Some(update) = &s.update {
                self.visit_expression(update);
            }
        }
        self.equal = true;
        self.live = before.1;
    }
    fn visit_for_in_statement(&mut self, s: &ForInStatement<'a>) {
        self.loop_region(s.span);
    }
    fn visit_for_of_statement(&mut self, s: &ForOfStatement<'a>) {
        self.loop_region(s.span);
    }
    fn visit_try_statement(&mut self, s: &TryStatement<'a>) {
        let before = (self.equal, self.live);
        self.visit_block_statement(&s.block);
        let normal = (self.equal, self.live);
        if let Some(handler) = &s.handler {
            self.equal = before.0 && self.input_unchanged(s.block.span);
            self.live = before.1;
            // Computed keys and defaults can run conditionally while binding
            // the exception. Do not import equality across those operations.
            if let Some(parameter) = &handler.param {
                if !matches!(parameter.pattern, BindingPattern::BindingIdentifier(_)) {
                    self.unsupported(parameter.span);
                }
            }
            self.visit_block_statement(&handler.body);
            self.merge(normal, (self.equal, self.live));
        }
        if let Some(finalizer) = &s.finalizer {
            let normal_live = self.live;
            // The finalizer must be safe on abrupt entries too. An equality
            // freshly established in the try's normal suffix is insufficient.
            self.equal = before.0 && self.input_unchanged(s.span);
            self.live = before.1;
            self.visit_block_statement(finalizer);
            self.live &= normal_live;
        }
    }
    fn visit_switch_statement(&mut self, s: &SwitchStatement<'a>) {
        if self.equal && self.input_unchanged(s.span) {
            self.loop_region(s.span);
            return;
        }
        // A bounded case with an explicit final break cannot fall through.
        // More complex break routing stays under the invariant-only proof.
        if !s.cases.iter().all(|c| {
            matches!(c.consequent.last(),Some(Statement::BreakStatement(b)) if b.label.is_none())
                && c.consequent[..c.consequent.len() - 1]
                    .iter()
                    .all(|statement| {
                        let mut abrupt = LoopAbrupt::default();
                        abrupt.visit_statement(statement);
                        !abrupt.unsupported && !abrupt.continues
                    })
        }) {
            self.loop_region(s.span);
            return;
        }
        self.visit_expression(&s.discriminant);
        let mut exits = Vec::new();
        for case in &s.cases {
            self.equal = false;
            self.live = true;
            if let Some(test) = &case.test {
                self.visit_expression(test);
            }
            for statement in &case.consequent[..case.consequent.len() - 1] {
                if self.live {
                    self.visit_statement(statement);
                }
            }
            exits.push((self.equal, self.live));
        }
        if !s.cases.iter().any(|c| c.test.is_none()) {
            exits.push((false, true));
        }
        self.equal = true;
        self.live = false;
        for state in exits {
            self.merge((self.equal, self.live), state);
        }
    }
    fn visit_labeled_statement(&mut self, s: &LabeledStatement<'a>) {
        self.loop_region(s.span);
    }
    fn visit_continue_statement(&mut self, _: &ContinueStatement<'a>) {
        self.live = false;
    }
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
    fn visit_class(&mut self, _: &Class<'a>) {
        if self.effects {
            self.equal = false;
        }
    }
}
