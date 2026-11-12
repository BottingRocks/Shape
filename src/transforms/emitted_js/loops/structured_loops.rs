//! Recover conditions, backedge updates, and exit continuations from VM loops.
//! No names are invented. Captured per-iteration bindings are never lifted.

use std::collections::BTreeSet;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

use crate::analysis;

mod extended;
mod single_pass;

pub(crate) use single_pass::unwrap_single_pass_loops;

pub(crate) fn reconstruct_loops(
    source: &str,
    protected: &BTreeSet<String>,
) -> Result<(String, usize), String> {
    let mut code = source.to_string();
    let mut changed = 0;
    // Work from inner loops outwards, avoiding overlapping edits in a round.
    loop {
        let allocator = Allocator::default();
        let program = analysis::parse_program(&allocator, &code)?;
        let (scoping, nodes) = analysis::analyze(&program);
        let mut scan = Scan {
            source: &code,
            scoping: &scoping,
            nodes: &nodes,
            protected,
            edits: Vec::new(),
            dynamic: false,
            extended: false,
            classes: Vec::new(),
        };
        scan.visit_program(&program);
        if scan.dynamic {
            break;
        }
        if scan.edits.is_empty() {
            scan.extended = true;
            scan.visit_program(&program);
        }
        if scan.dynamic || scan.edits.is_empty() {
            break;
        }
        changed += scan.edits.len();
        let applied = analysis::apply_edits(&code, scan.edits);
        if !applied.rejected.is_empty() {
            return Err("structured loop edits overlap".into());
        }
        code = applied.output;
    }
    Ok((code, changed))
}

fn inside(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

fn targets(label: Option<&LabelIdentifier<'_>>, name: &str) -> bool {
    label.is_none_or(|label| label.name == name)
}

fn continues(statement: &Statement<'_>, name: &str) -> bool {
    matches!(statement, Statement::ContinueStatement(jump) if targets(jump.label.as_ref(), name))
}

fn breaks(statement: &Statement<'_>, name: &str) -> bool {
    matches!(statement, Statement::BreakStatement(jump) if targets(jump.label.as_ref(), name))
}

fn block<'s, 'a>(statement: &'s Statement<'a>) -> Option<&'s [Statement<'a>]> {
    match statement {
        Statement::BlockStatement(block) => Some(&block.body),
        _ => None,
    }
}

fn cannot_fall_through(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::ReturnStatement(_)
        | Statement::ThrowStatement(_)
        | Statement::BreakStatement(_)
        | Statement::ContinueStatement(_) => true,
        Statement::BlockStatement(body) => body.body.last().is_some_and(cannot_fall_through),
        Statement::IfStatement(branch) => {
            cannot_fall_through(&branch.consequent)
                && branch.alternate.as_ref().is_some_and(cannot_fall_through)
        }
        Statement::TryStatement(stmt) => {
            stmt.finalizer
                .as_ref()
                .is_some_and(|body| body.body.last().is_some_and(cannot_fall_through))
                || (stmt.block.body.last().is_some_and(cannot_fall_through)
                    && stmt.handler.as_ref().is_none_or(|handler| {
                        handler.body.body.last().is_some_and(cannot_fall_through)
                    }))
        }
        Statement::WhileStatement(loop_) if matches!(&loop_.test, Expression::BooleanLiteral(value) if value.value) =>
        {
            let mut jumps = Jumps::new("");
            jumps.visit_statement(&loop_.body);
            jumps.breaks == 0
        }
        Statement::LabeledStatement(labeled) => {
            if let Statement::ForStatement(loop_) = &labeled.body {
                if loop_.test.is_none() {
                    let mut jumps = Jumps::new(labeled.label.name.as_str());
                    jumps.visit_statement(&loop_.body);
                    return jumps.breaks == 0;
                }
            }
            false
        }
        Statement::ForStatement(loop_) if loop_.test.is_none() => {
            let mut jumps = Jumps::new("");
            jumps.visit_statement(&loop_.body);
            jumps.breaks == 0
        }
        _ => false,
    }
}

fn ends_iteration(statement: &Statement<'_>, name: &str) -> bool {
    if continues(statement, name) {
        return true;
    }
    match statement {
        Statement::ReturnStatement(_) | Statement::ThrowStatement(_) => true,
        Statement::BlockStatement(body) => {
            body.body.last().is_some_and(|s| ends_iteration(s, name))
        }
        Statement::IfStatement(branch) => {
            ends_iteration(&branch.consequent, name)
                && branch
                    .alternate
                    .as_ref()
                    .is_some_and(|s| ends_iteration(s, name))
        }
        Statement::TryStatement(stmt) => {
            stmt.finalizer
                .as_ref()
                .is_some_and(|body| body.body.last().is_some_and(|s| ends_iteration(s, name)))
                || (stmt
                    .block
                    .body
                    .last()
                    .is_some_and(|s| ends_iteration(s, name))
                    && stmt.handler.as_ref().is_none_or(|handler| {
                        handler
                            .body
                            .body
                            .last()
                            .is_some_and(|s| ends_iteration(s, name))
                    }))
        }
        Statement::BreakStatement(_) => false,
        _ => cannot_fall_through(statement),
    }
}

/// The `continue`s of loop `name` that `statement`, the last of the loop's
/// body, ends on: itself, or the last of a trailing block or `if` arm.
fn tail_continues(statement: &Statement<'_>, name: &str, out: &mut Vec<Span>) {
    match statement {
        Statement::ContinueStatement(jump) if targets(jump.label.as_ref(), name) => out.push(jump.span),
        Statement::BlockStatement(body) => {
            if let Some(last) = body.body.last() {
                tail_continues(last, name, out);
            }
        }
        Statement::IfStatement(branch) => {
            tail_continues(&branch.consequent, name, out);
            if let Some(alternate) = &branch.alternate {
                tail_continues(alternate, name, out);
            }
        }
        _ => {}
    }
}

fn continuing_arm(statements: &[Statement<'_>], name: &str) -> bool {
    let mut jumps = Jumps::new(name);
    jumps.statements(statements);
    jumps.continues != 0 && statements.last().is_some_and(|s| ends_iteration(s, name))
}

/// A `vm_loop_*: for (;;) { lets; if (test) {…} else {…} }` whose one arm
/// continues the loop and whose other arm leaves it.
struct VmLoop<'s, 'a> {
    labeled: &'s LabeledStatement<'a>,
    name: &'s str,
    /// The statements ahead of the branch, lifted into the condition.
    prefix: &'s [Statement<'a>],
    branch: &'s IfStatement<'a>,
    /// Whether the else arm is the one that continues.
    negate: bool,
    /// The continuing arm, without its trailing `continue`.
    run: &'s [Statement<'a>],
    /// The leaving arm, without its trailing `break`.
    exit: &'s [Statement<'a>],
    jumps: Jumps<'s>,
}

impl<'s, 'a> VmLoop<'s, 'a> {
    fn new(statement: &'s Statement<'a>) -> Option<Self> {
        let Statement::LabeledStatement(labeled) = statement else {
            return None;
        };
        if !labeled.label.name.starts_with("vm_loop_") {
            return None;
        }
        let Statement::ForStatement(for_loop) = &labeled.body else {
            return None;
        };
        if for_loop.init.is_some() || for_loop.test.is_some() || for_loop.update.is_some() {
            return None;
        }
        let body = block(&for_loop.body)?;
        let name = labeled.label.name.as_str();
        let trailing_break = body.last().is_some_and(|last| breaks(last, name));
        let branch_at = body
            .len()
            .saturating_sub(if trailing_break { 2 } else { 1 });
        let Some(Statement::IfStatement(branch)) = body.get(branch_at) else {
            return None;
        };
        let then = block(&branch.consequent)?;
        let otherwise = match &branch.alternate {
            Some(arm) => block(arm)?,
            None => &[],
        };
        let (run, exit, negate) = if continuing_arm(then, name) {
            (then, otherwise, false)
        } else if continuing_arm(otherwise, name) {
            (otherwise, then, true)
        } else {
            return None;
        };
        let run = if run.last().is_some_and(|s| continues(s, name)) {
            &run[..run.len() - 1]
        } else {
            run
        };
        let exit_break = exit.last().is_some_and(|s| breaks(s, name));
        if !trailing_break && !exit_break && !exit.last().is_some_and(cannot_fall_through) {
            return None;
        }
        let exit = if exit_break {
            &exit[..exit.len() - 1]
        } else {
            exit
        };
        let mut jumps = Jumps::new(name);
        jumps.statements(run);
        if jumps.breaks != 0 {
            // A body break must skip the false-condition continuation.
            return None;
        }
        let mut exit_jumps = Jumps::new(name);
        exit_jumps.statements(exit);
        if exit_jumps.breaks != 0 || exit_jumps.continues != 0 {
            return None;
        }
        Some(Self {
            labeled,
            name,
            prefix: &body[..branch_at],
            branch,
            negate,
            run,
            exit,
            jumps,
        })
    }
}

struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    protected: &'s BTreeSet<String>,
    edits: Vec<analysis::Edit>,
    dynamic: bool,
    extended: bool,
    classes: Vec<Span>,
}

impl Scan<'_, '_> {
    fn text(&self, span: Span) -> &str {
        &self.source[span.start as usize..span.end as usize]
    }

    fn captured(&self, symbol: SymbolId) -> bool {
        let owner = self.scoping.symbol_scope_id(symbol);
        self.scoping
            .get_resolved_references(symbol)
            .any(|reference| {
                let span = self.nodes.get_node(reference.node_id()).kind().span();
                if self.classes.iter().any(|class| {
                    inside(*class, span) && !inside(*class, self.scoping.symbol_span(symbol))
                }) {
                    // Instance field initializers retain lexical cells even
                    // though their scopes are not marked Function by Oxc.
                    return true;
                }
                for scope in self.scoping.scope_ancestors(reference.scope_id()) {
                    if scope == owner {
                        return false;
                    }
                    if self
                        .scoping
                        .scope_flags(scope)
                        .intersects(ScopeFlags::Function | ScopeFlags::Arrow)
                    {
                        return true;
                    }
                }
                true
            })
    }

    fn statements(&mut self, statements: &[Statement<'_>]) {
        if self.extended {
            self.extended_statements(statements);
            return;
        }
        for at in 0..statements.len() {
            self.structure(statements, at);
        }
    }

    /// Rewrites the `vm_loop_*` at `statements[at]` as a `while`, or a `for`
    /// when a counter steps at the end of the body.
    fn structure(&mut self, statements: &[Statement<'_>], at: usize) -> Option<()> {
        let vm = VmLoop::new(&statements[at])?;
        let (declarations, lifted_names, mut conditions) = self.lift_prefix(vm.prefix)?;
        let test = self.text(vm.branch.test.span());
        conditions.push(if vm.negate {
            format!("!({test})")
        } else {
            format!("({test})")
        });
        let condition = conditions.join(", ");
        let mut run = vm.run;
        let mut update = None;
        if vm.jumps.continues == 0 {
            if let Some((rest, span, symbol)) = self.trailing_update(run, vm.branch.test.span()) {
                update = Some((span, symbol));
                run = rest;
            }
        }
        let mut region = vm.labeled.span;
        let mut initializer = String::new();
        if let Some((_, counter)) = update {
            if let Some(decl) =
                self.counter_initializer(statements, at, &vm, counter, &lifted_names)
            {
                initializer = self.text(decl).trim_end_matches(';').to_string();
                region.start = decl.start;
            }
        }
        if self
            .edits
            .iter()
            .any(|edit| edit.start < region.end && region.start < edit.end)
        {
            return None;
        }
        let name = vm.name;
        let label = if vm.jumps.labeled_continue {
            format!("{name}: ")
        } else {
            String::new()
        };
        let header = if let Some((update, _)) = update {
            format!("for ({initializer}; {condition}; {})", self.text(update))
        } else {
            format!("while ({condition})")
        };
        let join = |statements: &[Statement<'_>]| {
            statements
                .iter()
                .map(|s| self.text(s.span()))
                .collect::<Vec<_>>()
                .join("\n")
        };
        // Keep both original arm blocks, including their lexical bindings.
        let continuation = if vm.exit.is_empty() {
            String::new()
        } else {
            format!("{{ {} }}", join(vm.exit))
        };
        // A `continue` the body ends on (in a trailing block or `if` arm)
        // only reaches the end of the body: it goes.
        let mut tail = Vec::new();
        if let Some(last) = run.last() {
            tail_continues(last, name, &mut tail);
        }
        let run_text = self.run_text_without(run, &vm.jumps, &tail);
        let loop_text = format!("{label}{header} {{ {run_text} }}");
        let replacement = if declarations.is_empty() && continuation.is_empty() {
            loop_text
        } else {
            format!("{{ {declarations} {loop_text} {continuation} }}")
        };
        self.edits
            .push(analysis::Edit::replace(region, replacement));
        Some(())
    }

    /// Lifts the single-binding `let`s ahead of the branch into the loop
    /// condition: their declarations, names, and `name = (init)` conditions.
    fn lift_prefix(
        &self,
        prefix: &[Statement<'_>],
    ) -> Option<(String, BTreeSet<String>, Vec<String>)> {
        let mut declarations = String::new();
        let mut lifted_names = BTreeSet::new();
        let mut conditions = Vec::new();
        for prefix in prefix {
            let Statement::VariableDeclaration(decl) = prefix else {
                return None;
            };
            let [binding] = decl.declarations.as_slice() else {
                return None;
            };
            let (BindingPattern::BindingIdentifier(id), Some(init)) = (&binding.id, &binding.init)
            else {
                return None;
            };
            let symbol = id.symbol_id.get()?;
            // Preserve per-iteration closure cells and all TDZ failures.
            if decl.kind != VariableDeclarationKind::Let
                || !(id.name.starts_with("$SSA_") || id.name.starts_with("$CARRY_"))
                || self.captured(symbol)
                || self
                    .scoping
                    .get_resolved_references(symbol)
                    .any(|reference| {
                        reference.is_write()
                            || self.nodes.get_node(reference.node_id()).kind().span().start
                                < init.span().end
                    })
            {
                return None;
            }
            declarations.push_str(&format!("let {};", id.name));
            lifted_names.insert(id.name.to_string());
            conditions.push(format!("{} = ({})", id.name, self.text(init.span())));
        }
        Some((declarations, lifted_names, conditions))
    }

    /// The declaration just before the loop when it can become the `for`
    /// initializer of `counter`: it declares the counter, its initializer reads
    /// no lifted name, and a `let` counter is used only by the loop body.
    fn counter_initializer(
        &self,
        statements: &[Statement<'_>],
        at: usize,
        vm: &VmLoop<'_, '_>,
        counter: SymbolId,
        lifted_names: &BTreeSet<String>,
    ) -> Option<Span> {
        let Some(Statement::VariableDeclaration(decl)) =
            at.checked_sub(1).map(|at| &statements[at])
        else {
            return None;
        };
        let [binding] = decl.declarations.as_slice() else {
            return None;
        };
        let BindingPattern::BindingIdentifier(id) = &binding.id else {
            return None;
        };
        let mut init_refs = Refs::default();
        if let Some(init) = &binding.init {
            init_refs.visit_expression(init);
        }
        let initializer_shadowed = init_refs.0.iter().any(|reference| {
            let reference = self.scoping.get_reference(*reference);
            lifted_names.contains(self.text(self.nodes.get_node(reference.node_id()).kind().span()))
        });
        let lexical_safe = !self.protected.contains(id.name.as_str())
            && !self.captured(counter)
            && self
                .scoping
                .get_resolved_references(counter)
                .all(|reference| {
                    inside(
                        vm.labeled.span,
                        self.nodes.get_node(reference.node_id()).kind().span(),
                    ) && !vm.exit.iter().any(|s| {
                        inside(
                            s.span(),
                            self.nodes.get_node(reference.node_id()).kind().span(),
                        )
                    })
                });
        (id.symbol_id.get() == Some(counter)
            && binding.init.is_some()
            && !initializer_shadowed
            && (decl.kind == VariableDeclarationKind::Var
                || (decl.kind == VariableDeclarationKind::Let && lexical_safe)))
            .then_some(decl.span)
    }

    /// Splits a trailing counter step off the loop body so it can become the
    /// `for` update: the rest of the body, the step's span, and its counter.
    fn trailing_update<'r, 'a>(
        &self,
        run: &'r [Statement<'a>],
        test: Span,
    ) -> Option<(&'r [Statement<'a>], Span, SymbolId)> {
        let Some(Statement::ExpressionStatement(last)) = run.last() else {
            return None;
        };
        let symbol = self.counter_update(&last.expression, test, run)?;
        Some((&run[..run.len() - 1], last.expression.span(), symbol))
    }

    /// The loop body's source, with each labeled `continue` that targets the
    /// loop directly rewritten as a bare `continue;`.
    fn run_text(&self, run: &[Statement<'_>], jumps: &Jumps<'_>) -> String {
        self.run_text_without(run, jumps, &[])
    }

    /// [`Self::run_text`], with the `continue`s at `dropped` removed.
    fn run_text_without(&self, run: &[Statement<'_>], jumps: &Jumps<'_>, dropped: &[Span]) -> String {
        run.iter()
            .map(|s| {
                let span = s.span();
                let mut text = self.text(span).to_string();
                let mut jumps: Vec<(Span, &str)> = jumps
                    .direct_continues
                    .iter()
                    .filter(|jump| !dropped.contains(jump))
                    .map(|jump| (*jump, "continue;"))
                    .chain(dropped.iter().map(|jump| (*jump, "")))
                    .filter(|(jump, _)| inside(span, *jump))
                    .collect();
                jumps.sort_by_key(|(jump, _)| std::cmp::Reverse(jump.start));
                for (jump, replacement) in jumps {
                    text.replace_range(
                        (jump.start - span.start) as usize..(jump.end - span.start) as usize,
                        replacement,
                    );
                }
                text
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn counter_update(
        &self,
        expression: &Expression<'_>,
        test: Span,
        run: &[Statement<'_>],
    ) -> Option<SymbolId> {
        let target = match expression {
            Expression::AssignmentExpression(assign) => {
                let AssignmentTarget::AssignmentTargetIdentifier(id) = &assign.left else {
                    return None;
                };
                match &assign.right {
                    Expression::BinaryExpression(binary)
                        if matches!(
                            binary.operator,
                            oxc_syntax::operator::BinaryOperator::Addition
                                | oxc_syntax::operator::BinaryOperator::Subtraction
                        ) => {}
                    Expression::NumericLiteral(_)
                        if matches!(
                            assign.operator,
                            oxc_syntax::operator::AssignmentOperator::Addition
                                | oxc_syntax::operator::AssignmentOperator::Subtraction
                        ) => {}
                    _ => return None,
                }
                id
            }
            Expression::UpdateExpression(update) => {
                let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &update.argument
                else {
                    return None;
                };
                id
            }
            _ => return None,
        };
        let symbol = analysis::identifier_symbol(target, self.scoping)?;
        if run
            .iter()
            .any(|s| inside(s.span(), self.scoping.symbol_span(symbol)))
        {
            return None;
        }
        if !self
            .scoping
            .get_resolved_references(symbol)
            .any(|reference| inside(test, self.nodes.get_node(reference.node_id()).kind().span()))
        {
            return None;
        }
        let mut refs = Refs::default();
        refs.visit_expression(expression);
        if refs.0.iter().any(|id| {
            self.scoping
                .get_reference(*id)
                .symbol_id()
                .is_some_and(|symbol| {
                    run.iter()
                        .any(|s| inside(s.span(), self.scoping.symbol_span(symbol)))
                })
        }) {
            return None;
        }
        Some(symbol)
    }
}

impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_class(&mut self, class: &Class<'a>) {
        self.classes.push(class.span);
        walk::walk_class(self, class);
    }
    fn visit_program(&mut self, program: &Program<'a>) {
        walk::walk_program(self, program);
        self.statements(&program.body);
    }
    fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
        walk::walk_function_body(self, body);
        self.statements(&body.statements);
    }
    fn visit_block_statement(&mut self, body: &BlockStatement<'a>) {
        walk::walk_block_statement(self, body);
        self.statements(&body.body);
    }
    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, statement);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.dynamic |= id.name == "eval";
    }
}

#[derive(Default)]
struct Refs(Vec<oxc_semantic::ReferenceId>);
impl<'a> Visit<'a> for Refs {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        if let Some(reference) = id.reference_id.get() {
            self.0.push(reference);
        }
    }
}

struct Jumps<'s> {
    label: &'s str,
    break_depth: usize,
    continue_depth: usize,
    breaks: usize,
    continues: usize,
    labeled_continue: bool,
    direct_continues: Vec<Span>,
}
impl<'s> Jumps<'s> {
    fn new(label: &'s str) -> Self {
        Self {
            label,
            break_depth: 0,
            continue_depth: 0,
            breaks: 0,
            continues: 0,
            labeled_continue: false,
            direct_continues: Vec::new(),
        }
    }
    fn statements(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            self.visit_statement(statement);
        }
    }
}
impl<'a> Visit<'a> for Jumps<'_> {
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
    fn visit_break_statement(&mut self, jump: &BreakStatement<'a>) {
        if jump
            .label
            .as_ref()
            .map_or(self.break_depth == 0, |label| label.name == self.label)
        {
            self.breaks += 1;
        }
    }
    fn visit_continue_statement(&mut self, jump: &ContinueStatement<'a>) {
        if jump
            .label
            .as_ref()
            .map_or(self.continue_depth == 0, |label| label.name == self.label)
        {
            self.continues += 1;
            if jump.label.is_some() {
                if self.continue_depth == 0 {
                    self.direct_continues.push(jump.span);
                } else {
                    self.labeled_continue = true;
                }
            }
        }
    }
    fn visit_for_statement(&mut self, loop_: &ForStatement<'a>) {
        self.break_depth += 1;
        self.continue_depth += 1;
        walk::walk_for_statement(self, loop_);
        self.break_depth -= 1;
        self.continue_depth -= 1;
    }
    fn visit_for_in_statement(&mut self, loop_: &ForInStatement<'a>) {
        self.break_depth += 1;
        self.continue_depth += 1;
        walk::walk_for_in_statement(self, loop_);
        self.break_depth -= 1;
        self.continue_depth -= 1;
    }
    fn visit_for_of_statement(&mut self, loop_: &ForOfStatement<'a>) {
        self.break_depth += 1;
        self.continue_depth += 1;
        walk::walk_for_of_statement(self, loop_);
        self.break_depth -= 1;
        self.continue_depth -= 1;
    }
    fn visit_while_statement(&mut self, loop_: &WhileStatement<'a>) {
        self.break_depth += 1;
        self.continue_depth += 1;
        walk::walk_while_statement(self, loop_);
        self.break_depth -= 1;
        self.continue_depth -= 1;
    }
    fn visit_do_while_statement(&mut self, loop_: &DoWhileStatement<'a>) {
        self.break_depth += 1;
        self.continue_depth += 1;
        walk::walk_do_while_statement(self, loop_);
        self.break_depth -= 1;
        self.continue_depth -= 1;
    }
    fn visit_switch_statement(&mut self, switch: &SwitchStatement<'a>) {
        self.break_depth += 1;
        walk::walk_switch_statement(self, switch);
        self.break_depth -= 1;
    }
}

#[cfg(test)]
mod tests;
