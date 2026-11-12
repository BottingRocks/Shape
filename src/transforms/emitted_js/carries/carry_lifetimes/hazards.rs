//! What may intervene: definite writes, block functions and hazards between handoff and commit.

use super::*;

pub(super) struct MustWrite<'s> {
    pub(super) scoping: &'s Scoping,
    pub(super) target: SymbolId,
    pub(super) destination: SymbolId,
    pub(super) assigned: bool,
    pub(super) valid: bool,
}
impl<'a> Visit<'a> for MustWrite<'_> {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        if !id
            .reference_id
            .get()
            .is_some_and(|r| self.scoping.get_reference(r).is_read())
        {
            return;
        }
        match analysis::identifier_symbol(id, self.scoping) {
            Some(symbol) if symbol == self.target && !self.assigned => self.valid = false,
            Some(symbol) if symbol == self.destination && self.assigned => self.valid = false,
            _ => {}
        }
    }
    fn visit_assignment_expression(&mut self, a: &AssignmentExpression<'a>) {
        if matches!(&a.left,AssignmentTarget::AssignmentTargetIdentifier(id) if analysis::identifier_symbol(id,self.scoping)==Some(self.target))
            && a.operator == AssignmentOperator::Assign
        {
            self.visit_expression(&a.right);
            self.assigned = true;
        } else {
            walk::walk_assignment_expression(self, a);
        }
    }
    fn visit_if_statement(&mut self, s: &IfStatement<'a>) {
        self.visit_expression(&s.test);
        let before = self.assigned;
        self.visit_statement(&s.consequent);
        let yes = self.assigned;
        self.assigned = before;
        if let Some(no) = &s.alternate {
            self.visit_statement(no);
        }
        self.assigned &= yes;
    }
    fn visit_conditional_expression(&mut self, c: &ConditionalExpression<'a>) {
        self.visit_expression(&c.test);
        let before = self.assigned;
        self.visit_expression(&c.consequent);
        let yes = self.assigned;
        self.assigned = before;
        self.visit_expression(&c.alternate);
        self.assigned &= yes;
    }
    fn visit_logical_expression(&mut self, l: &LogicalExpression<'a>) {
        self.visit_expression(&l.left);
        let before = self.assigned;
        self.visit_expression(&l.right);
        self.assigned &= before;
    }
    fn visit_try_statement(&mut self, _: &TryStatement<'a>) {
        self.valid = false;
    }
    fn visit_switch_statement(&mut self, _: &SwitchStatement<'a>) {
        self.valid = false;
    }
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
    fn visit_class(&mut self, _: &Class<'a>) {}
}

/// A function declaration named `name` anywhere in what it visits.
pub(super) struct NamedFunction<'n> {
    pub(super) name: &'n str,
    pub(super) found: bool,
}
impl<'a> Visit<'a> for NamedFunction<'_> {
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        self.found |= f.id.as_ref().is_some_and(|id| id.name.as_str() == self.name);
        walk::walk_function(self, f, flags);
    }
}

pub(super) struct Hazards {
    pub(super) before: u32,
    pub(super) bad: bool,
    pub(super) target: String,
}
impl<'a> Visit<'a> for Hazards {
    fn visit_for_statement(&mut self, _: &ForStatement<'a>) {
        self.bad = true;
    }
    fn visit_while_statement(&mut self, _: &WhileStatement<'a>) {
        self.bad = true;
    }
    fn visit_do_while_statement(&mut self, _: &DoWhileStatement<'a>) {
        self.bad = true;
    }
    fn visit_for_in_statement(&mut self, _: &ForInStatement<'a>) {
        self.bad = true;
    }
    fn visit_for_of_statement(&mut self, _: &ForOfStatement<'a>) {
        self.bad = true;
    }
    fn visit_break_statement(&mut self, s: &BreakStatement<'a>) {
        self.bad |= s.span.start < self.before;
    }
    fn visit_continue_statement(&mut self, s: &ContinueStatement<'a>) {
        self.bad |= s.span.start < self.before;
    }
    fn visit_return_statement(&mut self, s: &ReturnStatement<'a>) {
        self.bad |= s.span.start < self.before;
    }
    fn visit_throw_statement(&mut self, s: &ThrowStatement<'a>) {
        self.bad |= s.span.start < self.before;
    }
    fn visit_function(&mut self, f: &Function<'a>, _: ScopeFlags) {
        // A sloppy block declaration may implicitly update the outer var
        // alias without an IdentifierReference write in semantic analysis.
        self.bad |=
            f.id.as_ref()
                .is_some_and(|id| id.name.as_str() == self.target.as_str());
    }
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
    fn visit_class(&mut self, _: &Class<'a>) {}
}
