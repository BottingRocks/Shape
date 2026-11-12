//! Remove an immediately redundant `undefined` assignment after a `let`.
//!
//! This targets the large slot-initialization prelude in linked programs. It
//! does not infer liveness across a branch, call, or closure: the declaration
//! and assignments must be consecutive direct statements of one function.
//! `let x;` already produces `undefined`, so no dynamic-scope or escape
//! analysis is needed. Only direct function-body statements are scanned; the
//! result carries the changed source and the number of removed assignments.

use std::collections::BTreeSet;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::{operator::AssignmentOperator, operator::UnaryOperator, scope::ScopeFlags};

use crate::analysis;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedundantInitializerResult {
    pub code: String,
    pub removed_assignments: usize,
}

/// Remove only direct, consecutive `x = void 0` statements following an
/// uninitialized `let x`. The target is resolved to that declaration's Oxc
/// symbol, so a same-spelled binding in another scope cannot qualify.
pub fn remove_redundant_initializers(source: &str) -> Result<RedundantInitializerResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, _) = analysis::analyze(&program);
    let mut scan = Scan {
        source,
        scoping: &scoping,
        edits: Vec::new(),
    };
    scan.visit_program(&program);
    if scan.edits.is_empty() {
        return Ok(RedundantInitializerResult {
            code: source.to_owned(),
            removed_assignments: 0,
        });
    }
    let count = scan.edits.len();
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() || applied.applied != count {
        return Err("redundant initializer edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("redundant initializer output: {error}"))?;
    Ok(RedundantInitializerResult {
        code: applied.output,
        removed_assignments: count,
    })
}

fn is_void_zero(expression: &Expression<'_>) -> bool {
    let Expression::UnaryExpression(unary) = expression else {
        return false;
    };
    unary.operator == UnaryOperator::Void
        && matches!(&unary.argument, Expression::NumericLiteral(number) if number.value == 0.0)
}

struct Scan<'s> {
    source: &'s str,
    scoping: &'s Scoping,
    edits: Vec<analysis::Edit>,
}

impl Scan<'_> {
    fn body(&mut self, statements: &[Statement<'_>]) {
        for (index, statement) in statements.iter().enumerate() {
            let Statement::VariableDeclaration(declaration) = statement else {
                continue;
            };
            if declaration.kind != VariableDeclarationKind::Let
                || declaration.declarations.is_empty()
            {
                continue;
            }
            let mut declared = BTreeSet::<SymbolId>::new();
            for declarator in &declaration.declarations {
                let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
                    declared.clear();
                    break;
                };
                if declarator.init.is_some() {
                    declared.clear();
                    break;
                }
                let Some(symbol) = binding.symbol_id.get() else {
                    declared.clear();
                    break;
                };
                declared.insert(symbol);
            }
            if declared.len() != declaration.declarations.len() {
                continue;
            }
            for next in statements.iter().skip(index + 1) {
                let Statement::ExpressionStatement(expression_statement) = next else {
                    break;
                };
                let Expression::AssignmentExpression(assignment) = &expression_statement.expression
                else {
                    break;
                };
                if assignment.operator != AssignmentOperator::Assign {
                    break;
                }
                let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
                    break;
                };
                let Some(symbol) = analysis::identifier_symbol(target, self.scoping) else {
                    break;
                };
                if !declared.contains(&symbol) || !is_void_zero(&assignment.right) {
                    break;
                }
                self.edits.push(analysis::Edit::delete(
                    self.whole_line_or_statement(next.span()),
                ));
            }
        }
    }

    fn whole_line_or_statement(&self, span: Span) -> Span {
        super::super::support::whole_line(self.source, span)
    }
}

impl<'a> Visit<'a> for Scan<'_> {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some(body) = &function.body {
            self.body(&body.statements);
        }
        walk::walk_function(self, function, flags);
    }
}

#[cfg(test)]
mod tests {
    use super::remove_redundant_initializers;

    #[test]
    fn removes_only_immediate_undefined_assignments_in_each_function() {
        let source = "function outer() {\n  let a, b;\n  a = void 0;\n  b = void 0;\n  function inner() {\n    let c;\n    c = void 0;\n    return c;\n  }\n  return [a, b, inner];\n}\n";
        let result = remove_redundant_initializers(source).unwrap();
        assert_eq!(result.removed_assignments, 3);
        assert!(!result.code.contains("= void 0"));
        assert!(result.code.contains("let a, b;"));
        assert!(result.code.contains("let c;"));
    }

    #[test]
    fn retains_resets_after_observable_code_and_on_branches() {
        let source = "function outer() {\n  let a, b;\n  a = void 0;\n  observe();\n  b = void 0;\n  if (flag) { a = void 0; }\n  return function () { return a + b; };\n}\n";
        let result = remove_redundant_initializers(source).unwrap();
        assert_eq!(result.removed_assignments, 1);
        assert!(result.code.contains("b = void 0"));
        assert!(result.code.contains("if (flag) { a = void 0; }"));
    }

    #[test]
    fn retains_effectful_rhs_and_initialized_declarations() {
        let source = "function f() {\n  let a;\n  a = void call();\n  let b = 1;\n  b = void 0;\n  return [a, b];\n}\n";
        let result = remove_redundant_initializers(source).unwrap();
        assert_eq!(result.removed_assignments, 0);
        assert_eq!(result.code, source);
    }
}
