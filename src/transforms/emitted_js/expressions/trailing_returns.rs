//! Drop the `return void 0;` (or `return;`) a function body ends with:
//! falling off the end returns `undefined` too. Only the body's last
//! statement goes; an early return inside a branch stays.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_span::{GetSpan, Span};

use crate::analysis;
use super::super::support::whole_line;

pub(crate) fn drop_trailing_returns(source: &str) -> Result<(String, usize), String> {
    if !source.contains("return") {
        return Ok((source.into(), 0));
    }
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let mut scan = Scan { found: Vec::new() };
    scan.visit_program(&program);
    if scan.found.is_empty() {
        return Ok((source.into(), 0));
    }
    let count = scan.found.len();
    let edits = scan.found.into_iter().map(|span| analysis::Edit::delete(whole_line(source, span))).collect();
    let output = super::super::support::apply_checked(source, edits, "trailing return")?;
    Ok((output, count))
}

struct Scan {
    found: Vec<Span>,
}

impl<'a> Visit<'a> for Scan {
    fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
        if let Some(Statement::ReturnStatement(last)) = body.statements.last() {
            let empty = last.argument.as_ref().is_none_or(|value| {
                matches!(value.get_inner_expression(), Expression::UnaryExpression(unary)
                    if unary.operator == oxc_syntax::operator::UnaryOperator::Void && matches!(unary.argument, Expression::NumericLiteral(_)))
            });
            if empty {
                self.found.push(last.span());
            }
        }
        walk::walk_function_body(self, body);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_only_the_last_statement_of_a_body() {
        let source = "function f(a) {\n\tif (a) {\n\t\treturn void 0;\n\t}\n\tg();\n\treturn void 0;\n}\nlet h = () => {\n\tg();\n\treturn;\n};\nfunction k() {\n\treturn 1;\n}\n";
        let (code, count) = drop_trailing_returns(source).unwrap();
        assert_eq!(count, 2);
        assert_eq!(code, "function f(a) {\n\tif (a) {\n\t\treturn void 0;\n\t}\n\tg();\n}\nlet h = () => {\n\tg();\n};\nfunction k() {\n\treturn 1;\n}\n");
    }
}
