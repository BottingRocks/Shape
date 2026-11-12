//! Every statement list of a program.

use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};

/// Calls `each` with every statement list, outer lists first: the program,
/// function bodies, blocks and `switch` cases.
pub(crate) fn for_each_list<'a, F>(program: &Program<'a>, each: F)
where
    F: FnMut(&[Statement<'a>]),
{
    struct Lists<F>(F);
    impl<'a, F: FnMut(&[Statement<'a>])> Visit<'a> for Lists<F> {
        fn visit_program(&mut self, program: &Program<'a>) {
            (self.0)(&program.body);
            walk::walk_program(self, program);
        }
        fn visit_function_body(&mut self, body: &FunctionBody<'a>) {
            (self.0)(&body.statements);
            walk::walk_function_body(self, body);
        }
        fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
            (self.0)(&block.body);
            walk::walk_block_statement(self, block);
        }
        fn visit_switch_case(&mut self, case: &SwitchCase<'a>) {
            (self.0)(&case.consequent);
            walk::walk_switch_case(self, case);
        }
    }
    Lists(each).visit_program(program);
}
