//! Collapse a local value repeated by a conditional into one logical read.
//! `value ? next : value` is `value && next`, preserving the actual falsy
//! value. Only resolved lexical reads qualify; arbitrary getters do not.
use crate::analysis;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::Scoping;
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::UnaryOperator;
use crate::transforms::emitted_js::support::unwrap;

pub(crate) fn collapse_repeated_tests(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, _) = analysis::analyze(&program);
    struct Scan<'s> {
        source: &'s str,
        scoping: &'s Scoping,
        edits: Vec<analysis::Edit>,
        dynamic: bool,
    }
    impl<'a> Visit<'a> for Scan<'_> {
        fn visit_conditional_expression(&mut self, c: &ConditionalExpression<'a>) {
            let (test, yes, no) = match unwrap(&c.test) {
                Expression::UnaryExpression(u) if u.operator == UnaryOperator::LogicalNot => {
                    (unwrap(&u.argument), &c.alternate, &c.consequent)
                }
                test => (test, &c.consequent, &c.alternate),
            };
            if let Expression::Identifier(id) = test {
                if let Some(symbol) = analysis::identifier_symbol(id, self.scoping) {
                    let same = |e: &Expression<'_>| matches!(unwrap(e),Expression::Identifier(other) if analysis::identifier_symbol(other,self.scoping)==Some(symbol));
                    let replacement = if same(no) {
                        Some(("&&", yes))
                    } else if same(yes) {
                        Some(("||", no))
                    } else {
                        None
                    };
                    if let Some((operator, value)) = replacement {
                        let text =
                            |span: Span| &self.source[span.start as usize..span.end as usize];
                        self.edits.push(analysis::Edit::replace(
                            c.span,
                            format!(
                                "(({}) {operator} ({}))",
                                text(test.span()),
                                text(value.span())
                            ),
                        ));
                    }
                }
            }
            walk::walk_conditional_expression(self, c);
        }
        fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
            self.dynamic |= id.name == "eval";
        }
        fn visit_with_statement(&mut self, s: &WithStatement<'a>) {
            self.dynamic = true;
            walk::walk_with_statement(self, s);
        }
    }
    let mut scan = Scan {
        source,
        scoping: &scoping,
        edits: Vec::new(),
        dynamic: false,
    };
    scan.visit_program(&program);
    if scan.dynamic {
        return Ok((source.into(), 0));
    }
    scan.edits
        .sort_by_key(|e| (e.start, std::cmp::Reverse(e.end)));
    let mut end = 0;
    scan.edits.retain(|e| {
        if e.start < end {
            return false;
        }
        end = e.end;
        true
    });
    let count = scan.edits.len();
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() {
        return Err("conditional value edits overlap".into());
    }
    Ok((applied.output, count))
}
