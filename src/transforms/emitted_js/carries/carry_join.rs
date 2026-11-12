//! Remove a single-use generated carry produced by a two-arm branch.
//!
//! Recognizes `let $CARRY_x; if (test) { $CARRY_x = yes; } else { $CARRY_x =
//! no; }` followed immediately by a statement whose expression is exactly
//! `$CARRY_x` (an if test, return/throw argument, or single let initializer).
//! The binding must have exactly two writes and one read globally, and that
//! read must be the direct next use. The declaration and `if` are deleted and
//! the read becomes `((test) ? (yes) : (no))`; the test still runs first and
//! only one arm evaluates. Comments in the removed region decline. This removes
//! a branch-join carry without moving either arm across the test.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::{operator::AssignmentOperator, scope::ScopeFlags};

use crate::analysis;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CarryJoinResult {
    pub code: String,
    pub removed_carries: usize,
}

/// `let $CARRY; if (test) { $CARRY = yes; } else { $CARRY = no; }
/// if ($CARRY) ...` becomes `if (test ? yes : no) ...` when the carry has
/// exactly one read, in the immediately following statement.
pub(crate) fn inline_single_use_carry_joins(source: &str) -> Result<CarryJoinResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = Scan {
        source,
        scoping: &scoping,
        nodes: &nodes,
        edits: Vec::new(),
        dynamic_scope: false,
        removed: 0,
    };
    scan.visit_program(&program);
    if scan.dynamic_scope || scan.edits.is_empty() {
        return Ok(CarryJoinResult {
            code: source.into(),
            removed_carries: 0,
        });
    }
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() || applied.applied != scan.removed * 2 {
        return Err("carry join edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("carry join output: {error}"))?;
    Ok(CarryJoinResult {
        code: applied.output,
        removed_carries: scan.removed,
    })
}

struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    edits: Vec<analysis::Edit>,
    dynamic_scope: bool,
    removed: usize,
}

impl Scan<'_, '_> {
    fn body(&mut self, statements: &[Statement<'_>]) {
        for triple in statements.windows(3) {
            let Statement::VariableDeclaration(declaration) = &triple[0] else {
                continue;
            };
            if declaration.kind != VariableDeclarationKind::Let
                || declaration.declarations.len() != 1
            {
                continue;
            }
            let declarator = &declaration.declarations[0];
            let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
                continue;
            };
            if !binding.name.starts_with("$CARRY_") || declarator.init.is_some() {
                continue;
            }
            let Some(symbol) = binding.symbol_id.get() else {
                continue;
            };
            let Statement::IfStatement(branch) = &triple[1] else {
                continue;
            };
            let (Some(yes), Some(no)) = (
                assigned_value(&branch.consequent, symbol, self.scoping),
                branch
                    .alternate
                    .as_ref()
                    .and_then(|arm| assigned_value(arm, symbol, self.scoping)),
            ) else {
                continue;
            };
            let mut writes = 0;
            let mut reads = Vec::new();
            for reference in self.scoping.get_resolved_references(symbol) {
                if reference.is_write() {
                    writes += 1;
                } else {
                    reads.push(self.nodes.get_node(reference.node_id()).kind().span());
                }
            }
            if writes != 2 || reads.len() != 1 {
                continue;
            }
            let read = reads[0];
            let Some(next_read) = direct_next_read(&triple[2]) else {
                continue;
            };
            if read != next_read {
                continue;
            }
            let removed = Span::new(triple[0].span().start, triple[1].span().end);
            let removed_text = &self.source[removed.start as usize..removed.end as usize];
            if removed_text.contains("//") || removed_text.contains("/*") {
                continue;
            }
            let text = |span: Span| &self.source[span.start as usize..span.end as usize];
            let replacement = format!(
                "(({}) ? ({}) : ({}))",
                text(branch.test.span()),
                text(yes),
                text(no)
            );
            self.edits.push(analysis::Edit::delete(removed));
            self.edits.push(analysis::Edit::replace(read, replacement));
            self.removed += 1;
        }
    }
}

fn assigned_value(arm: &Statement<'_>, symbol: SymbolId, scoping: &Scoping) -> Option<Span> {
    let Statement::BlockStatement(block) = arm else {
        return None;
    };
    let [Statement::ExpressionStatement(statement)] = block.body.as_slice() else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = &statement.expression else {
        return None;
    };
    if assignment.operator != AssignmentOperator::Assign {
        return None;
    }
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
        return None;
    };
    (analysis::identifier_symbol(target, scoping) == Some(symbol)).then(|| assignment.right.span())
}

fn direct_next_read(statement: &Statement<'_>) -> Option<Span> {
    let expression = match statement {
        Statement::IfStatement(branch) => Some(&branch.test),
        Statement::ReturnStatement(ret) => ret.argument.as_ref(),
        Statement::ThrowStatement(thrown) => Some(&thrown.argument),
        Statement::VariableDeclaration(declaration) if declaration.declarations.len() == 1 => {
            declaration.declarations[0].init.as_ref()
        }
        _ => None,
    }?;
    let Expression::Identifier(identifier) = expression else {
        return None;
    };
    Some(identifier.span)
}

impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some(body) = &function.body {
            self.body(&body.statements);
        }
        walk::walk_function(self, function, flags);
    }
    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.body(&block.body);
        walk::walk_block_statement(self, block);
    }
    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic_scope = true;
        walk::walk_with_statement(self, statement);
    }
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if identifier.name == "eval" {
            self.dynamic_scope = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::inline_single_use_carry_joins;
    use crate::emitted_cleanup::inline_adjacent_ssa;

    #[test]
    fn removes_carry_join_before_immediate_test() {
        let source = "function f(flag, a, b) { let $CARRY_1; if (flag) { $CARRY_1 = a; } else { $CARRY_1 = b; } if ($CARRY_1) { return 1; } return 0; }";
        let result = inline_single_use_carry_joins(source).unwrap();
        assert_eq!(result.removed_carries, 1);
        assert!(!result.code.contains("$CARRY_1"));
        assert!(
            result.code.contains("if (((flag) ? (a) : (b)))"),
            "{}",
            result.code
        );
    }

    #[test]
    fn retains_multiuse_and_effectful_intervening_statement() {
        let source = "function f(flag, a, b) { let $CARRY_1; if (flag) { $CARRY_1 = a; } else { $CARRY_1 = b; } observe(); return $CARRY_1 + $CARRY_1; }";
        assert_eq!(
            inline_single_use_carry_joins(source)
                .unwrap()
                .removed_carries,
            0
        );
    }

    #[test]
    fn adjacent_ssa_exposes_branch_carry_join() {
        let source = "function f(flag, value) { let $CARRY_1; if (flag) { let $SSA_1 = value; $CARRY_1 = $SSA_1; } else { $CARRY_1 = 0; } if ($CARRY_1) return 1; }";
        let adjacent = inline_adjacent_ssa(source).unwrap();
        let result = inline_single_use_carry_joins(&adjacent.code).unwrap();
        assert_eq!(result.removed_carries, 1, "{}", result.code);
        assert!(!result.code.contains("$CARRY_1"));
    }
}
