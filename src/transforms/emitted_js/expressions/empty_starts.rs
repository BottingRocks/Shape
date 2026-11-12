//! Declare a `let` without its `= void 0`, and with the value it is given
//! right after.
//!
//! `let x = void 0;` is `let x;`: a `let` without an initializer starts
//! undefined each time it runs, in a loop body too. A `const` keeps its
//! initializer, which it needs.
//!
//! `let x; x = e;` is `let x = e;` when nothing can read `x` while `e` runs:
//! in the merged form that read would throw instead of giving undefined. So
//! `e` names `x` only inside functions it creates, and makes no call unless
//! no function at all names `x` (a call reaches `x` only through a closure).
//! The declaration must declare `x` alone and the store follow it directly
//! in the same statement list.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::GetSpan;

use crate::analysis;

pub(crate) fn tidy_empty_starts(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = Scan { source, scoping: &scoping, nodes: &nodes, edits: Vec::new() };
    scan.visit_program(&program);
    if scan.edits.is_empty() {
        return Ok((source.into(), 0));
    }
    let changed = scan.edits.len();
    let output = super::super::support::apply_checked(source, scan.edits, "empty start")?;
    Ok((output, changed))
}

struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s oxc_semantic::AstNodes<'a>,
    edits: Vec<analysis::Edit>,
}

/// `void 0` (of any literal), or the global `undefined`.
fn undefined(scoping: &Scoping, expression: &Expression<'_>) -> bool {
    match expression.without_parentheses() {
        Expression::UnaryExpression(unary) => {
            unary.operator == UnaryOperator::Void
                && matches!(
                    unary.argument,
                    Expression::NumericLiteral(_) | Expression::StringLiteral(_) | Expression::BooleanLiteral(_) | Expression::NullLiteral(_)
                )
        }
        Expression::Identifier(name) => name.name == "undefined" && scoping.get_reference(name.reference_id()).symbol_id().is_none(),
        _ => false,
    }
}

impl Scan<'_, '_> {
    /// Every reference to `symbol` is in the function that declares it.
    fn unclosed(&self, symbol: SymbolId) -> bool {
        super::super::support::unclosed(self.scoping, self.nodes, symbol)
    }

    fn text(&self, span: oxc_span::Span) -> &str {
        &self.source[span.start as usize..span.end as usize]
    }

    fn list(&mut self, statements: &[Statement<'_>]) {
        let mut skip_next = false;
        for (index, statement) in statements.iter().enumerate() {
            if std::mem::take(&mut skip_next) {
                continue;
            }
            let Statement::VariableDeclaration(declaration) = statement else {
                continue;
            };
            if declaration.kind != VariableDeclarationKind::Let {
                continue;
            }
            if let Some(merged) = self.merge(declaration, statements.get(index + 1)) {
                self.edits.push(merged);
                skip_next = true;
                continue;
            }
            for declarator in &declaration.declarations {
                if declarator.init.as_ref().is_some_and(|init| undefined(self.scoping, init)) {
                    self.edits.push(analysis::Edit::replace(declarator.span, self.text(declarator.id.span()).to_string()));
                }
            }
        }
    }

    /// `let x [= void 0]; x = e;` as `let x = e;`.
    fn merge(&self, declaration: &VariableDeclaration<'_>, next: Option<&Statement<'_>>) -> Option<analysis::Edit> {
        let [declarator] = declaration.declarations.as_slice() else {
            return None;
        };
        let BindingPattern::BindingIdentifier(id) = &declarator.id else {
            return None;
        };
        if declarator.init.as_ref().is_some_and(|init| !undefined(self.scoping, init)) {
            return None;
        }
        let Some(Statement::ExpressionStatement(store)) = next else {
            return None;
        };
        let Expression::AssignmentExpression(assignment) = &store.expression else {
            return None;
        };
        let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
            return None;
        };
        let symbol = id.symbol_id();
        if assignment.operator != AssignmentOperator::Assign
            || self.scoping.get_reference(target.reference_id()).symbol_id() != Some(symbol)
            || !self.source[declaration.span.end as usize..store.span.start as usize].trim().is_empty()
            || !quiet(self.scoping, &assignment.right, symbol, self.unclosed(symbol))
        {
            return None;
        }
        let span = oxc_span::Span::new(declaration.span.start, store.span.end);
        Some(analysis::Edit::replace(span, format!("let {} = {};", id.name, self.text(assignment.right.span()))))
    }
}

/// `e` names `symbol` only inside functions it creates, and makes no call
/// unless `calls_allowed` (no closure names `symbol`).
fn quiet(scoping: &Scoping, expression: &Expression<'_>, symbol: SymbolId, calls_allowed: bool) -> bool {
    struct Check<'s> {
        scoping: &'s Scoping,
        symbol: SymbolId,
        calls_allowed: bool,
        ok: bool,
    }
    impl<'a> Visit<'a> for Check<'_> {
        fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
            self.ok &= self.calls_allowed;
            walk::walk_call_expression(self, call);
        }
        fn visit_new_expression(&mut self, new: &NewExpression<'a>) {
            self.ok &= self.calls_allowed;
            walk::walk_new_expression(self, new);
        }
        fn visit_tagged_template_expression(&mut self, tagged: &TaggedTemplateExpression<'a>) {
            self.ok &= self.calls_allowed;
            walk::walk_tagged_template_expression(self, tagged);
        }
        fn visit_await_expression(&mut self, _: &AwaitExpression<'a>) {
            self.ok = false;
        }
        fn visit_yield_expression(&mut self, _: &YieldExpression<'a>) {
            self.ok = false;
        }
        fn visit_identifier_reference(&mut self, name: &IdentifierReference<'a>) {
            if self.scoping.get_reference(name.reference_id()).symbol_id() == Some(self.symbol) {
                self.ok = false;
            }
        }
        // A function made here does not run here.
        fn visit_function(&mut self, _: &Function<'a>, _: oxc_syntax::scope::ScopeFlags) {}
        fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
        fn visit_class(&mut self, _: &Class<'a>) {
            self.ok = false;
        }
    }
    let mut check = Check { scoping, symbol, calls_allowed, ok: true };
    check.visit_expression(expression);
    check.ok
}

impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
        self.list(&body.statements);
        walk::walk_function_body(self, body);
    }
    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.list(&block.body);
        walk::walk_block_statement(self, block);
    }
    fn visit_switch_case(&mut self, case: &SwitchCase<'a>) {
        self.list(&case.consequent);
        walk::walk_switch_case(self, case);
    }
    fn visit_program(&mut self, program: &Program<'a>) {
        self.list(&program.body);
        walk::walk_program(self, program);
    }
    fn visit_for_statement(&mut self, statement: &ForStatement<'a>) {
        if let Some(ForStatementInit::VariableDeclaration(declaration)) = &statement.init {
            if declaration.kind == VariableDeclarationKind::Let {
                for declarator in &declaration.declarations {
                    if declarator.init.as_ref().is_some_and(|init| undefined(self.scoping, init)) {
                        self.edits.push(analysis::Edit::replace(declarator.span, self.text(declarator.id.span()).to_string()));
                    }
                }
            }
        }
        walk::walk_for_statement(self, statement);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tidy(source: &str) -> String {
        tidy_empty_starts(source).unwrap().0
    }

    #[test]
    fn drops_void_starts_of_lets() {
        assert_eq!(
            tidy("let a = void 0, b = 1;\nconst c = void 0;\nvar d = void 0;\nlet e = undefined;\ntry {\n\ta = f();\n} catch (x) {}\n"),
            "let a, b = 1;\nconst c = void 0;\nvar d = void 0;\nlet e;\ntry {\n\ta = f();\n} catch (x) {}\n"
        );
    }

    #[test]
    fn merges_the_store_that_follows() {
        assert_eq!(tidy("function g(o) {\n\tlet a;\n\ta = o.p + 1;\n\tlet b = void 0;\n\tb = () => b;\n\treturn [a, b];\n}\n"), "function g(o) {\n\tlet a = o.p + 1;\n\tlet b = () => b;\n\treturn [a, b];\n}\n");
    }

    #[test]
    fn keeps_a_store_that_could_read_the_binding() {
        // `f()` could read `a` through `r`; `b` reads itself; `c` is not
        // stored next.
        let source = "function g(o) {\n\tlet a;\n\tfunction r() {\n\t\treturn a;\n\t}\n\ta = f(r);\n\tlet b;\n\tb = b + 1;\n\tlet c;\n\th();\n\tc = 1;\n\treturn [a, b, c];\n}\n";
        assert_eq!(tidy(source), source);
        // No function names `d`: no call can read it.
        assert_eq!(tidy("function g(k) {\n\tlet d;\n\td = k(1) | 0;\n\treturn d;\n}\n"), "function g(k) {\n\tlet d = k(1) | 0;\n\treturn d;\n}\n");
    }
}
