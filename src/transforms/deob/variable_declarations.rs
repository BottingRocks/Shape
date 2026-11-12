//! Final-presentation formatting: one variable declaration per statement.
//!
//! The pipeline's readable output may keep several declarators in one
//! `var`/`let`/`const` statement, which is easy to overlook in the long native
//! capture row.  This pass splits those statements in *statement lists* only:
//!
//! * program bodies, function bodies, blocks, switch cases and static blocks;
//! * never `for`/`for-in`/`for-of` headers, which require their existing
//!   grammar and evaluation order;
//! * never the single statement used as an `if`/`else`/loop body, which cannot
//!   become a statement list without adding braces.
//!
//! Every declarator keeps its original node and document order, so proven
//! declaration ordinals, names, initializer order and values are unchanged.
//! This is presentation only; no expression is rewritten and initializers are
//! never inspected or comma-split textually.

use oxc_allocator::{Allocator, TakeIn, Vec};
use oxc_ast::{
    ast::{Program, Statement, VariableDeclaration, VariableDeclarationKind},
    AstBuilder,
};
use oxc_ast_visit::{walk_mut, VisitMut};

use crate::analysis;

/// Result of the one-declaration-per-statement presentation pass.
pub(crate) struct SplitReport {
    pub output: String,
    /// Number of multi-declarator statements rewritten.
    pub changed: usize,
}

/// Rewrite every multi-declarator statement in a statement list as one
/// `var`/`let`/`const` statement per declarator. Returns the original source
/// when there is nothing to split.
pub(crate) fn split(source: &str) -> Result<SplitReport, String> {
    let allocator = Allocator::default();
    let mut program = analysis::parse_program(&allocator, source)?;
    let changed = split_program(&mut program, &allocator);
    if changed == 0 {
        return Ok(SplitReport {
            output: source.to_owned(),
            changed: 0,
        });
    }
    Ok(SplitReport {
        output: analysis::codegen(&program),
        changed,
    })
}

/// Apply the same canonical split to an already parsed program. Proof matchers
/// that compare two ASTs can use this to ignore presentation-only grouping.
pub(crate) fn split_program<'a>(program: &mut Program<'a>, allocator: &'a Allocator) -> usize {
    let mut splitter = Splitter {
        builder: AstBuilder::new(allocator),
        changed: 0,
    };
    splitter.visit_program(program);
    splitter.changed
}

struct Splitter<'a> {
    builder: AstBuilder<'a>,
    changed: usize,
}

impl<'a> VisitMut<'a> for Splitter<'a> {
    fn visit_statements(&mut self, statements: &mut Vec<'a, Statement<'a>>) {
        // Recurse first so nested blocks/cases are formatted before this list
        // is rebuilt.
        walk_mut::walk_statements(self, statements);
        if !statements.iter().any(|statement| {
            matches!(
                statement,
                Statement::VariableDeclaration(declaration)
                    if splittable(declaration)
            )
        }) {
            return;
        }

        let input = statements.take_in(self.builder);
        let mut output = self.builder.vec_with_capacity(input.len());
        for statement in input {
            let Statement::VariableDeclaration(mut declaration) = statement else {
                output.push(statement);
                continue;
            };
            if !splittable(&declaration) {
                output.push(Statement::VariableDeclaration(declaration));
                continue;
            }

            self.changed += 1;
            let declarations = declaration.declarations.take_in(self.builder);
            let kind = declaration.kind;
            let declare = declaration.declare;
            let mut declarators = declarations.into_iter();
            let Some(first) = declarators.next() else {
                // A declaration with no declarators cannot be parsed, but keep
                // the impossible case lossless instead of dropping it.
                output.push(Statement::VariableDeclaration(declaration));
                continue;
            };

            let first_span = first.span;
            declaration.span = first_span;
            declaration.declarations.push(first);
            output.push(Statement::VariableDeclaration(declaration));

            for declarator in declarators {
                let mut one = self.builder.vec_with_capacity(1);
                let span = declarator.span;
                one.push(declarator);
                output.push(Statement::VariableDeclaration(
                    self.builder
                        .alloc_variable_declaration(span, kind, one, declare),
                ));
            }
        }
        *statements = output;
    }
}

/// Only ordinary `var`/`let`/`const` statements are split. `using` and
/// `await using` declarations are left untouched to keep their disposal
/// grammar and semantics out of this formatting pass.
fn splittable(declaration: &VariableDeclaration<'_>) -> bool {
    declaration.declarations.len() > 1
        && matches!(
            declaration.kind,
            VariableDeclarationKind::Var
                | VariableDeclarationKind::Let
                | VariableDeclarationKind::Const
        )
}
