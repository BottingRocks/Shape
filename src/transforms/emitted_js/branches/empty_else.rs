//! Drop empty `else {}` blocks left by carry cleanup.
//!
//! Empty alternate statements and recursively empty block arms are removed; the deleted span
//! covers the else token and its empty body. Comments and required dangling-else
//! barriers are retained. Tests, declarations, and nonempty branches stay intact.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_span::{GetSpan, Span};

use crate::analysis;

pub(crate) fn remove_empty_else_blocks(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let mut scan = Scan::default();
    scan.visit_program(&program);
    let edits: Vec<_> = scan
        .removals
        .into_iter()
        .filter(|span| {
            let text = &source[span.start as usize..span.end as usize];
            !text.contains("/*") && !text.contains("//")
        })
        .map(analysis::Edit::delete)
        .collect();
    let count = edits.len();
    if count == 0 {
        return Ok((source.into(), 0));
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("empty else edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("empty else output: {error}"))?;
    Ok((applied.output, count))
}

#[derive(Default)]
struct Scan {
    removals: Vec<Span>,
    dangling: bool,
}

fn empty(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::EmptyStatement(_) => true,
        Statement::BlockStatement(block) => block.body.iter().all(empty),
        _ => false,
    }
}
impl<'a> Visit<'a> for Scan {
    fn visit_if_statement(&mut self, statement: &IfStatement<'a>) {
        // Removing an inner else in an unbraced outer consequent can make
        // the outer else bind to that inner if. Keep that grammatical barrier.
        if !self.dangling {
            if let Some(alternate) = &statement.alternate {
                if empty(alternate) {
                    self.removals.push(Span::new(
                        statement.consequent.span().end,
                        alternate.span().end,
                    ));
                }
            }
        }
        self.visit_expression(&statement.test);
        let outer = self.dangling;
        self.dangling = outer || statement.alternate.is_some();
        self.visit_statement(&statement.consequent);
        self.dangling = outer;
        if let Some(alternate) = &statement.alternate {
            self.visit_statement(alternate);
        }
        self.dangling = outer;
    }
    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        let outer = self.dangling;
        self.dangling = false;
        walk::walk_block_statement(self, block);
        self.dangling = outer;
    }
    fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
        let outer = self.dangling;
        self.dangling = false;
        walk::walk_function_body(self, body);
        self.dangling = outer;
    }
}

#[cfg(test)]
mod tests {
    use super::remove_empty_else_blocks;

    #[test]
    fn drops_empty_else_after_carry_store_removal() {
        let source = "function f(p) { if (p) { use(); } else { } return 1; }";
        let (code, count) = remove_empty_else_blocks(source).unwrap();
        assert_eq!(count, 1);
        assert!(!code.contains("else"));
        assert!(code.contains("if (p) { use(); }"));
    }

    #[test]
    fn retains_comment_and_nonempty_else() {
        let comment = "if (p) {} else { /* why */ }";
        assert_eq!(remove_empty_else_blocks(comment).unwrap().1, 0);
        let body = "if (p) {} else { use(); }";
        assert_eq!(remove_empty_else_blocks(body).unwrap().1, 0);
    }
}

#[cfg(test)]
mod more_empty_else_tests {
    #[test]
    fn nonblock_and_nested_empty_arms_keep_dangling_else_routing() {
        for source in [
            "if(flag)work();else;",
            "if(flag)work();else{;;}",
            "if(flag){work();}else{{;}{;;}}",
        ] {
            let (code, count) = super::remove_empty_else_blocks(source).unwrap();
            assert_eq!(count, 1, "{code}");
            assert!(!code.contains("else"), "{code}");
        }
        let source = "if(a)if(b)work();else;else other();";
        assert_eq!(super::remove_empty_else_blocks(source).unwrap().0, source);
        let source = "if(a){if(b)work();else;}else other();";
        assert_eq!(super::remove_empty_else_blocks(source).unwrap().1, 1);
    }
}
