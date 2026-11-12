//! Turn comma expressions the VM uses as statements back into statements.
//!
//! Loop reconstruction leaves a loop whose test computes its condition in
//! steps, `while (i = a[k], j = i - 48, !(j >= 10)) S`. A `while (A, c) S`
//! runs exactly like `for (;;) { A; if (!c) break; S }` — a `continue` in
//! `S` goes back to `A` either way, and a `for` update still runs before
//! it — so the steps become statements at the top of the body, where the
//! forwarding and merging passes can read them. The same holds for a comma
//! statement `a, b;` (two statements) and a discarded choice
//! `c ? (a, b) : d;` (`if (c) { a; b; } else { d; }`).
//!
//! The final tidy writes `for (;;) { if (!c) break; … }` back as
//! `while (c) { … }` when nothing precedes the test any more.

use oxc_allocator::{Allocator, Vec as ArenaVec};
use oxc_ast::ast::*;
use oxc_ast::AstBuilder;
use oxc_ast_visit::{walk_mut, VisitMut};
use oxc_codegen::{Codegen, CodegenOptions, IndentChar};
use oxc_span::SPAN;
use oxc_syntax::operator::UnaryOperator;

use crate::analysis;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SplitLoopTests {
    pub code: String,
    pub split: usize,
}

pub(crate) fn split_loop_tests(source: &str) -> Result<SplitLoopTests, String> {
    let split = split_tests(source)?;
    let mut code = split.code;
    let mut count = split.split;
    for _ in 0..8 {
        let (next, sunk) = sink_loop_lets(&code)?;
        code = next;
        count += sunk;
        if sunk == 0 {
            break;
        }
    }
    Ok(SplitLoopTests { code, split: count })
}

/// `let x;` before a loop whose body alone uses `x` and stores it first on
/// every iteration (`x = v;` at the top level, before any other reference)
/// is that body's `let x = v;`: no value crosses iterations, and no closure
/// can see the difference between one binding and one per iteration.
fn sink_loop_lets(source: &str) -> Result<(String, usize), String> {
    use oxc_ast::AstKind;
    use oxc_span::{GetSpan, Span};
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let function_of = |node: oxc_semantic::NodeId| {
        nodes
            .ancestor_ids(node)
            .find(|ancestor| matches!(nodes.kind(*ancestor), AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)))
    };
    let mut edits: Vec<analysis::Edit> = Vec::new();
    let mut touched: Vec<Span> = Vec::new();
    let mut sunk = 0;
    for symbol in scoping.symbol_ids() {
        let declarator_id = scoping.symbol_declaration(symbol);
        let AstKind::VariableDeclarator(declarator) = nodes.kind(declarator_id) else {
            continue;
        };
        if declarator.init.is_some() || !matches!(declarator.id, BindingPattern::BindingIdentifier(_)) {
            continue;
        }
        let declaration_id = nodes.parent_id(declarator_id);
        let AstKind::VariableDeclaration(declaration) = nodes.kind(declaration_id) else {
            continue;
        };
        if declaration.kind != VariableDeclarationKind::Let {
            continue;
        }
        let home = function_of(declaration_id);
        let references: Vec<_> = scoping.get_resolved_references(symbol).collect();
        let Some(first) = references.iter().map(|reference| nodes.kind(reference.node_id()).span()).min_by_key(|span| span.start) else {
            continue;
        };
        if references.iter().any(|reference| function_of(reference.node_id()) != home) {
            continue;
        }
        // The innermost block after the declaration holding every reference.
        let inside = |block: Span| {
            references.iter().all(|reference| {
                let span = nodes.kind(reference.node_id()).span();
                block.start <= span.start && span.end <= block.end
            })
        };
        let Some(block) = nodes.ancestor_ids(references[0].node_id()).find_map(|ancestor| match nodes.kind(ancestor) {
            AstKind::BlockStatement(block) if block.span.start > declaration.span.end && inside(block.span) => Some(block),
            _ => None,
        }) else {
            continue;
        };
        // The first reference is the target of a top-level `x = v;` whose
        // value does not read `x`.
        let Some(store) = block.body.iter().find(|statement| statement.span().start <= first.start && first.end <= statement.span().end) else {
            continue;
        };
        let Statement::ExpressionStatement(expression) = store else {
            continue;
        };
        let Expression::AssignmentExpression(assignment) = &expression.expression else {
            continue;
        };
        let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
            continue;
        };
        if assignment.operator != oxc_syntax::operator::AssignmentOperator::Assign
            || target.span != first
            || references.iter().any(|reference| {
                let span = nodes.kind(reference.node_id()).span();
                assignment.right.span().start <= span.start && span.end <= assignment.right.span().end
            })
        {
            continue;
        }
        // The body must not declare the name already.
        if block.body.iter().any(|statement| matches!(statement, Statement::VariableDeclaration(other)
            if other.declarations.iter().any(|d| d.id.get_binding_identifier().is_some_and(|b| b.name == target.name))))
        {
            continue;
        }
        let removal = if declaration.declarations.len() == 1 {
            declaration.span
        } else {
            let Some(at) = declaration.declarations.iter().position(|other| other.span == declarator.span) else {
                continue;
            };
            match declaration.declarations.get(at + 1) {
                Some(next) => Span::new(declarator.span.start, next.span.start),
                None => Span::new(declaration.declarations[at - 1].span.end, declarator.span.end),
            }
        };
        // The store may be written `(x = v);`: `let` must replace the
        // parentheses, or `let (x = v);` is a call of a function named `let`.
        let insert = Span::new(expression.span.start, assignment.span.start);
        let end = Span::new(assignment.span.end, expression.span.end);
        if touched.iter().any(|span| span.start < removal.end && removal.start < span.end)
            || touched.contains(&declaration.span)
        {
            continue;
        }
        touched.push(declaration.span);
        edits.push(analysis::Edit::delete(removal));
        edits.push(analysis::Edit::replace(insert, "let "));
        edits.push(analysis::Edit::replace(end, ";"));
        sunk += 1;
    }
    if sunk == 0 {
        return Ok((source.into(), 0));
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("loop declaration sinks overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output).map_err(|error| format!("loop declaration sink output: {error}"))?;
    Ok((applied.output, sunk))
}

fn split_tests(source: &str) -> Result<SplitLoopTests, String> {
    let allocator = Allocator::default();
    let mut program = analysis::parse_program(&allocator, source)?;
    let mut split = Split {
        builder: AstBuilder::new(&allocator),
        split: 0,
    };
    split.visit_program(&mut program);
    if split.split == 0 {
        return Ok(SplitLoopTests {
            code: source.into(),
            split: 0,
        });
    }
    Ok(SplitLoopTests {
        code: Codegen::new()
            .with_options(CodegenOptions {
                indent_char: IndentChar::Tab,
                indent_width: 1,
                ..CodegenOptions::default()
            })
            .build(&program)
            .code,
        split: split.split,
    })
}

struct Split<'a> {
    builder: AstBuilder<'a>,
    split: usize,
}

/// A discarded choice whose arms are statements in disguise.
fn statement_choice(expression: &Expression<'_>) -> bool {
    let Expression::ConditionalExpression(conditional) = expression.without_parentheses() else {
        return false;
    };
    let arm = |arm: &Expression<'_>| {
        matches!(
            arm.without_parentheses(),
            Expression::SequenceExpression(_) | Expression::AssignmentExpression(_)
        ) || statement_choice(arm)
    };
    arm(&conditional.consequent) || arm(&conditional.alternate)
}

/// A loop test that computes in steps: `(a, b, c)`, or `c || (steps, false)`
/// (the exit runs `steps`).
fn steps_test(test: &Expression<'_>) -> bool {
    match test {
        Expression::SequenceExpression(_) => true,
        Expression::LogicalExpression(logical) => {
            logical.operator == oxc_syntax::operator::LogicalOperator::Or
                && matches!(logical.right.without_parentheses(), Expression::SequenceExpression(sequence)
                    if matches!(sequence.expressions.last(), Some(Expression::BooleanLiteral(value)) if !value.value))
        }
        _ => false,
    }
}

impl<'a> Split<'a> {
    /// The statements an expression evaluated for its effects stands for.
    fn statements_of(&mut self, expression: Expression<'a>, into: &mut ArenaVec<'a, Statement<'a>>) {
        let choice = statement_choice(&expression);
        match expression {
            Expression::ParenthesizedExpression(inner) => {
                let inner = inner.unbox().expression;
                self.statements_of(inner, into);
            }
            Expression::SequenceExpression(sequence) => {
                for step in sequence.unbox().expressions {
                    self.statements_of(step, into);
                }
            }
            Expression::ConditionalExpression(conditional) if choice => {
                let conditional = conditional.unbox();
                let mut consequent = self.builder.vec();
                self.statements_of(conditional.consequent, &mut consequent);
                let mut alternate = self.builder.vec();
                self.statements_of(conditional.alternate, &mut alternate);
                let alternate = (!alternate.is_empty()).then(|| self.builder.statement_block(SPAN, alternate));
                into.push(self.builder.statement_if(
                    SPAN,
                    conditional.test,
                    self.builder.statement_block(SPAN, consequent),
                    alternate,
                ));
            }
            // `void 0` and literals do nothing as statements.
            Expression::UnaryExpression(unary)
                if unary.operator == UnaryOperator::Void && matches!(unary.argument, Expression::NumericLiteral(_)) => {}
            Expression::BooleanLiteral(_) | Expression::NullLiteral(_) | Expression::NumericLiteral(_) => {}
            expression => into.push(self.builder.statement_expression(SPAN, expression)),
        }
    }

    /// `if (!c) break;`, reading `x` for `c = !x`.
    fn exit_unless(&mut self, test: Expression<'a>) -> Statement<'a> {
        // `c || (steps, false)`: the exit runs the steps.
        if steps_test(&test) && matches!(test, Expression::LogicalExpression(_)) {
            let Expression::LogicalExpression(logical) = test else { unreachable!() };
            let logical = logical.unbox();
            let mut exit = self.builder.vec();
            self.statements_of(logical.right, &mut exit);
            exit.push(self.builder.statement_break(SPAN, None));
            let test = self.builder.expression_unary(SPAN, UnaryOperator::LogicalNot, logical.left);
            return self.builder.statement_if(SPAN, test, self.builder.statement_block(SPAN, exit), None);
        }
        let test = match test {
            Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::LogicalNot => {
                unary.unbox().argument
            }
            Expression::ParenthesizedExpression(inner) => {
                return self.exit_unless(inner.unbox().expression);
            }
            test => self.builder.expression_unary(SPAN, UnaryOperator::LogicalNot, test),
        };
        let exit = self.builder.statement_break(SPAN, None);
        self.builder.statement_if(SPAN, test, exit, None)
    }

    /// The body `{ steps; if (!c) break; S }` for a sequence test, or
    /// `{ if (!c) { steps; break; } S }` for `c || (steps, false)`.
    fn body(&mut self, test: Expression<'a>, body: Statement<'a>) -> Statement<'a> {
        let mut statements = self.builder.vec();
        match test {
            Expression::SequenceExpression(sequence) => {
                let mut steps = sequence.unbox().expressions;
                let condition = steps.pop().expect("a sequence has a last expression");
                for step in steps {
                    self.statements_of(step, &mut statements);
                }
                statements.push(self.exit_unless(condition));
            }
            Expression::LogicalExpression(logical) => {
                let logical = logical.unbox();
                let mut exit = self.builder.vec();
                self.statements_of(logical.right, &mut exit);
                // The trailing `false` was dropped as a statement with no effect.
                exit.push(self.builder.statement_break(SPAN, None));
                let test = self.builder.expression_unary(SPAN, UnaryOperator::LogicalNot, logical.left);
                statements.push(self.builder.statement_if(SPAN, test, self.builder.statement_block(SPAN, exit), None));
            }
            _ => unreachable!("a stepped test"),
        }
        match body {
            Statement::BlockStatement(block) => statements.extend(block.unbox().body),
            body => statements.push(body),
        }
        self.builder.statement_block(SPAN, statements)
    }

    fn take(&self, statement: &mut Statement<'a>) -> Statement<'a> {
        std::mem::replace(statement, self.builder.statement_empty(SPAN))
    }
}

impl<'a> VisitMut<'a> for Split<'a> {
    fn visit_statements(&mut self, statements: &mut ArenaVec<'a, Statement<'a>>) {
        walk_mut::walk_statements(self, statements);
        if !statements.iter().any(|statement| {
            matches!(statement, Statement::ExpressionStatement(expression)
                if matches!(expression.expression.without_parentheses(), Expression::SequenceExpression(_))
                    || statement_choice(&expression.expression))
        }) {
            return;
        }
        let mut out = self.builder.vec_with_capacity(statements.len());
        for statement in statements.drain(..) {
            match statement {
                Statement::ExpressionStatement(expression)
                    if matches!(expression.expression.without_parentheses(), Expression::SequenceExpression(_))
                        || statement_choice(&expression.expression) =>
                {
                    self.split += 1;
                    self.statements_of(expression.unbox().expression, &mut out);
                }
                statement => out.push(statement),
            }
        }
        *statements = out;
    }

    fn visit_statement(&mut self, statement: &mut Statement<'a>) {
        walk_mut::walk_statement(self, statement);
        let sequence_test = match statement {
            Statement::WhileStatement(loop_) => steps_test(&loop_.test),
            Statement::ForStatement(loop_) => loop_.test.as_ref().is_some_and(steps_test),
            _ => false,
        };
        if !sequence_test {
            return;
        }
        self.split += 1;
        match self.take(statement) {
            Statement::WhileStatement(loop_) => {
                let loop_ = loop_.unbox();
                let body = self.body(loop_.test, loop_.body);
                *statement = self.builder.statement_for(loop_.span, None, None, None, body);
            }
            Statement::ForStatement(loop_) => {
                let loop_ = loop_.unbox();
                let test = loop_.test.expect("a sequence test");
                let body = self.body(test, loop_.body);
                *statement = self.builder.statement_for(loop_.span, loop_.init, None, loop_.update, body);
            }
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::split_loop_tests;

    fn run(source: &str) -> String {
        let output = std::process::Command::new("node")
            .args(["-e", &format!("{source};console.log(JSON.stringify(f()));")])
            .output()
            .unwrap();
        assert!(output.status.success(), "{}\n{source}", String::from_utf8_lossy(&output.stderr));
        String::from_utf8(output.stdout).unwrap()
    }

    #[test]
    fn moves_loop_test_steps_into_the_body() {
        let source = "function f() { const a = [3, 1, 4, 1, 5]; const out = []; let k = 0; let i, j, z; while (k < 2 || (z = 'done', false)) { k++; } k = 0; outer: while (i = a[k], j = i > 2 ? (out.push('big'), i) : i * 10, !(k >= 4)) { k++; if (j === 10) continue outer; out.push(j); } for (let n = 0; i = n * 2, n < 3; n++) { if (n === 1) continue; out.push(i); } out.push(z); return out; }";
        let result = split_loop_tests(source).unwrap();
        assert!(result.split >= 3, "{}", result.code);
        assert!(!result.code.contains("while (i ="), "{}", result.code);
        assert!(result.code.contains("if (k >= 4) break;"), "{}", result.code);
        assert_eq!(run(source), run(&result.code), "{}", result.code);
    }

    #[test]
    fn sinks_a_parenthesized_store_as_a_declaration() {
        let source = "function f() { const out = []; let t; for (let k = 0; k < 3; k++) { (t = k * 2); out.push(t); } return out; }";
        let code = split_loop_tests(source).unwrap().code;
        assert!(code.contains("let t = k * 2;"), "{code}");
        assert_eq!(run(source), run(&code), "{code}");
    }

    #[test]
    fn sinks_a_temporary_each_iteration_stores_first_into_the_body() {
        let source = "function f() { const a = [1, 2, 3]; const out = []; let t, u; for (let k = 0; k < 3; k++) { t = a[k] * 2; out.push(t); } let w; for (let k = 0; k < 3; k++) { if (k) { w = 1; } out.push(w); } return out; }";
        let mut code = source.to_string();
        loop {
            let result = split_loop_tests(&code).unwrap();
            if result.split == 0 {
                break;
            }
            code = result.code;
        }
        assert!(code.contains("let t = a[k] * 2;"), "{code}");
        assert!(code.contains("let w;"), "{code}");
        assert_eq!(run(source), run(&code), "{code}");
    }

    #[test]
    fn writes_comma_statements_and_discarded_choices_as_statements() {
        let source = "function f() { let a = 0, b = 0, c = 0; a = 1, b = 2; a > 0 ? (b = 3, c = 4) : c = 5; return [a, b, c]; }";
        let result = split_loop_tests(source).unwrap();
        assert!(result.code.contains("if (a > 0)"), "{}", result.code);
        assert!(!result.code.contains(", b = 2"), "{}", result.code);
        assert_eq!(run(source), run(&result.code), "{}", result.code);
    }
}
