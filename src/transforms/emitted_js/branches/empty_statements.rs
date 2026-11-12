//! Remove cleanup placeholders from lists, preserving real empty bodies and
//! the single barrier needed to keep an ordinary string from becoming a directive.
use crate::analysis;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_span::Span;

pub(crate) fn remove_empty_statements(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let mut scan = Scan::default();
    scan.visit_program(&program);
    let count = scan.removals.len();
    let edits = scan
        .removals
        .into_iter()
        .map(analysis::Edit::delete)
        .collect();
    let result = analysis::apply_edits(source, edits);
    if !result.rejected.is_empty() {
        return Err("empty statement edits overlap".into());
    }
    Ok((result.output, count))
}
#[derive(Default)]
struct Scan {
    removals: Vec<Span>,
}
impl Scan {
    fn list(&mut self, statements: &[Statement<'_>], directives: bool) {
        let mut barrier = None;
        if directives {
            let mut real_statement = false;
            for statement in statements {
                if let Statement::EmptyStatement(empty) = statement {
                    barrier.get_or_insert(empty.span);
                } else {
                    real_statement = true;
                    let is_string = match statement {
                        Statement::ExpressionStatement(s) => {
                            let mut expression = &s.expression;
                            while let Expression::ParenthesizedExpression(p) = expression {
                                expression = &p.expression;
                            }
                            matches!(expression, Expression::StringLiteral(_))
                        }
                        _ => false,
                    };
                    if !is_string {
                        barrier = None;
                    }
                    break;
                }
            }
            if !real_statement {
                barrier = None;
            }
        }
        for statement in statements {
            if let Statement::EmptyStatement(empty) = statement {
                if Some(empty.span) != barrier {
                    self.removals.push(empty.span);
                }
            }
        }
    }
}
impl<'a> Visit<'a> for Scan {
    fn visit_program(&mut self, p: &Program<'a>) {
        self.list(&p.body, true);
        walk::walk_program(self, p);
    }
    fn visit_function_body(&mut self, b: &FunctionBody<'a>) {
        self.list(&b.statements, true);
        walk::walk_function_body(self, b);
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        self.list(&b.body, false);
        walk::walk_block_statement(self, b);
    }
    fn visit_switch_case(&mut self, c: &SwitchCase<'a>) {
        self.list(&c.consequent, false);
        walk::walk_switch_case(self, c);
    }
    fn visit_static_block(&mut self, b: &StaticBlock<'a>) {
        self.list(&b.body, false);
        walk::walk_static_block(self, b);
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn removes_placeholders_but_keeps_bodies_and_directive_barriers() {
        for (source, count) in [
            ("function f(){;;;}", 3),
            ("class C{static{;;;}}", 3),
            ("function f(){let x; ; ; if(flag){;;;work();}else{;;;}}", 8),
            ("function f(){; ; 'use strict';return this;}", 1),
            ("function f(){'custom'; ; ; 'use strict';return this;}", 1),
            ("; ; 'use strict';work();", 1),
            (
                "function f(){while(test());for(;;);if(flag);else;label:;}",
                0,
            ),
        ] {
            let (code, removed) = super::remove_empty_statements(source).unwrap();
            assert_eq!(removed, count, "{code}");
        }
    }
}
