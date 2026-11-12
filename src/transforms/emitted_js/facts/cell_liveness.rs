//! Backward statement liveness for consuming one private stored version.
//! Unknown control flow is opaque; exceptions disable facts for that owner.
//! Expression writes are killed only at an unconditional simple root store.
use crate::analysis;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::{operator::AssignmentOperator, scope::ScopeFlags};
use std::collections::{BTreeMap, BTreeSet};
type Live = BTreeSet<SymbolId>;
#[derive(Default)]
pub(crate) struct Facts {
    after: BTreeMap<Span, Live>,
}
impl Facts {
    pub(crate) fn dead_after(&self, statement: Span, symbol: SymbolId) -> bool {
        self.after
            .get(&statement)
            .is_some_and(|live| !live.contains(&symbol))
    }
}
pub(crate) fn collect(program: &Program<'_>, scoping: &Scoping) -> Facts {
    let mut scan = Scan {
        scoping,
        facts: Facts::default(),
    };
    scan.owner(&program.body);
    scan.visit_program(program);
    scan.facts
}
struct Scan<'s> {
    scoping: &'s Scoping,
    facts: Facts,
}
#[derive(Default)]
struct Exceptions(bool);
impl<'a> Visit<'a> for Exceptions {
    fn visit_try_statement(&mut self, _: &TryStatement<'a>) {
        self.0 = true;
    }
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
}
struct Reads<'s> {
    scoping: &'s Scoping,
    live: Live,
}
impl<'a> Visit<'a> for Reads<'_> {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        // Unknown nested stores, including compound/update stores, are uses.
        if let Some(s) = analysis::identifier_symbol(id, self.scoping) {
            self.live.insert(s);
        }
    }
}
impl Scan<'_> {
    fn owner(&mut self, body: &[Statement<'_>]) {
        let mut exceptions = Exceptions::default();
        for s in body {
            exceptions.visit_statement(s);
        }
        if !exceptions.0 {
            self.body(body, Live::new(), &Live::new());
        }
    }
    fn reads(&self, e: &Expression<'_>, live: Live) -> Live {
        let mut reads = Reads {
            scoping: self.scoping,
            live,
        };
        reads.visit_expression(e);
        reads.live
    }
    fn expression(&self, e: &Expression<'_>, mut live: Live) -> Live {
        if let Expression::AssignmentExpression(a) = e {
            if a.operator == AssignmentOperator::Assign {
                if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
                    if let Some(s) = analysis::identifier_symbol(id, self.scoping) {
                        live.remove(&s);
                    }
                    return self.reads(&a.right, live);
                }
            }
        }
        self.reads(e, live)
    }
    fn declarations(&self, d: &VariableDeclaration<'_>, mut live: Live) -> Live {
        for item in d.declarations.iter().rev() {
            if let BindingPattern::BindingIdentifier(id) = &item.id {
                // `var x;` does not reset an already initialized binding.
                if item.init.is_some() || d.kind != VariableDeclarationKind::Var {
                    if let Some(s) = id.symbol_id.get() {
                        live.remove(&s);
                    }
                }
            } else {
                let mut reads = Reads {
                    scoping: self.scoping,
                    live,
                };
                reads.visit_binding_pattern(&item.id);
                live = reads.live;
            }
            if let Some(e) = &item.init {
                live = self.reads(e, live);
            }
        }
        live
    }
    fn body(&mut self, body: &[Statement<'_>], mut live: Live, jumps: &Live) -> Live {
        for s in body.iter().rev() {
            live = self.statement(s, live, jumps);
        }
        live
    }
    fn statement(&mut self, s: &Statement<'_>, live: Live, jumps: &Live) -> Live {
        self.facts.after.insert(s.span(), live.clone());
        match s {
            Statement::BlockStatement(b) => self.body(&b.body, live, jumps),
            Statement::VariableDeclaration(d) => self.declarations(d, live),
            Statement::ExpressionStatement(e) => self.expression(&e.expression, live),
            Statement::ReturnStatement(r) => r
                .argument
                .as_ref()
                .map(|e| self.reads(e, Live::new()))
                .unwrap_or_default(),
            Statement::ThrowStatement(t) => self.reads(&t.argument, Live::new()),
            Statement::IfStatement(i) => {
                let mut yes = self.statement(&i.consequent, live.clone(), jumps);
                yes.extend(
                    i.alternate
                        .as_ref()
                        .map(|s| self.statement(s, live.clone(), jumps))
                        .unwrap_or(live),
                );
                self.reads(&i.test, yes)
            }
            Statement::WhileStatement(w) => {
                self.loop_body(&w.body, Some(&w.test), None, live, jumps)
            }
            Statement::DoWhileStatement(w) => {
                // Include a zero-iteration edge too: conservative for do/while.
                self.loop_body(&w.body, Some(&w.test), None, live, jumps)
            }
            Statement::ForStatement(f) => {
                let live = self.loop_body(&f.body, f.test.as_ref(), f.update.as_ref(), live, jumps);
                match &f.init {
                    Some(ForStatementInit::VariableDeclaration(d)) => self.declarations(d, live),
                    Some(e) => e
                        .as_expression()
                        .map(|e| self.expression(e, live.clone()))
                        .unwrap_or(live),
                    None => live,
                }
            }
            Statement::BreakStatement(_) | Statement::ContinueStatement(_) => jumps.clone(),
            Statement::EmptyStatement(_) | Statement::FunctionDeclaration(_) => live,
            _ => {
                // Switch, labels, iteration bindings, classes and other opaque
                // forms cannot kill a value. Don't publish facts inside them.
                let mut reads = Reads {
                    scoping: self.scoping,
                    live,
                };
                reads.visit_statement(s);
                reads.live
            }
        }
    }
    fn loop_body(
        &mut self,
        body: &Statement<'_>,
        test: Option<&Expression<'_>>,
        update: Option<&Expression<'_>>,
        exit: Live,
        outer_jumps: &Live,
    ) -> Live {
        let mut head = exit.clone();
        loop {
            // Both jumps conservatively reach every loop entry/exit/update
            // observation, plus every possible enclosing jump destination.
            let mut jumps = outer_jumps.clone();
            jumps.extend(exit.iter().copied());
            jumps.extend(head.iter().copied());
            if let Some(e) = update {
                jumps = self.reads(e, jumps);
            }
            if let Some(e) = test {
                jumps = self.reads(e, jumps);
            }
            let tail = update
                .map(|e| self.expression(e, head.clone()))
                .unwrap_or_else(|| head.clone());
            let mut next = self.statement(body, tail, &jumps);
            next.extend(exit.iter().copied());
            if let Some(e) = test {
                next = self.reads(e, next);
            }
            next.extend(head.iter().copied());
            if next == head {
                return head;
            }
            head = next;
        }
    }
}
impl<'a> Visit<'a> for Scan<'_> {
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        if let Some(body) = &f.body {
            self.owner(&body.statements);
        }
        walk::walk_function(self, f, flags);
    }
    fn visit_arrow_function_expression(&mut self, a: &ArrowFunctionExpression<'a>) {
        if !a.expression {
            self.owner(&a.body.statements);
        }
        walk::walk_arrow_function_expression(self, a);
    }
}
