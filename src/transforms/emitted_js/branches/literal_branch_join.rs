//! Collapse a two-literal branch join into one generated binding.
//!
//! The declaration must be `let $SSA_n;`/`$CARRY_n;`, the test a resolved
//! identifier, and both arms single plain assignments to the same resolved
//! symbol with numeric/string/boolean/null literal RHSs. The result is
//! `const name = test ? a : b;`. Adjacency means no read observes the old
//! `undefined`, and literal RHSs cannot read the binding. Comments in the
//! replaced region decline; arbitrary branch bodies are deliberately excluded.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::{operator::AssignmentOperator, scope::ScopeFlags};

use crate::analysis;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralBranchJoinResult {
    pub code: String,
    pub collapsed: usize,
}

/// Replace `let $SSA_n; if (local) { $SSA_n = literal; } else {
/// $SSA_n = literal; }` with one `const` binding. The two writes must resolve
/// to the declaration, and there must be no other writes to that binding.
pub fn collapse_literal_branch_joins(source: &str) -> Result<LiteralBranchJoinResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, _) = analysis::analyze(&program);
    let mut scan = Scan {
        source,
        scoping: &scoping,
        dynamic_scope: false,
        edits: Vec::new(),
    };
    scan.visit_program(&program);
    if scan.dynamic_scope || scan.edits.is_empty() {
        return Ok(LiteralBranchJoinResult {
            code: source.to_owned(),
            collapsed: 0,
        });
    }
    let collapsed = scan.edits.len();
    let applied = analysis::apply_edits(source, scan.edits);
    if !applied.rejected.is_empty() || applied.applied != collapsed {
        return Err("literal branch join edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("literal branch join output: {error}"))?;
    Ok(LiteralBranchJoinResult {
        code: applied.output,
        collapsed,
    })
}

struct Scan<'s> {
    source: &'s str,
    scoping: &'s Scoping,
    dynamic_scope: bool,
    edits: Vec<analysis::Edit>,
}

impl Scan<'_> {
    fn body(&mut self, statements: &[Statement<'_>]) {
        for pair in statements.windows(2) {
            let Statement::VariableDeclaration(declaration) = &pair[0] else {
                continue;
            };
            if declaration.kind != VariableDeclarationKind::Let
                || declaration.declarations.len() != 1
            {
                continue;
            }
            let binding = &declaration.declarations[0];
            let BindingPattern::BindingIdentifier(name) = &binding.id else {
                continue;
            };
            if !(name.name.starts_with("$SSA_") || name.name.starts_with("$CARRY_"))
                || binding.init.is_some()
            {
                continue;
            }
            let Some(symbol) = name.symbol_id.get() else {
                continue;
            };
            let Statement::IfStatement(branch) = &pair[1] else {
                continue;
            };
            let Expression::Identifier(test) = &branch.test else {
                continue;
            };
            if test.name == name.name || analysis::identifier_symbol(test, self.scoping).is_none() {
                continue;
            }
            let (Some(yes), Some(no)) = (
                literal_assignment(&branch.consequent, name.name.as_str(), symbol, self.scoping),
                branch.alternate.as_ref().and_then(|alternate| {
                    literal_assignment(alternate, name.name.as_str(), symbol, self.scoping)
                }),
            ) else {
                continue;
            };
            let writes = self
                .scoping
                .get_resolved_references(symbol)
                .filter(|reference| reference.is_write())
                .count();
            if writes != 2 {
                continue;
            }
            let start = pair[0].span().start as usize;
            let end = pair[1].span().end as usize;
            let segment = &self.source[start..end];
            // Exact source editing must not discard comments, including
            // comments between the declaration and the conditional.
            if segment.contains("//") || segment.contains("/*") {
                continue;
            }
            let test = &self.source[test.span.start as usize..test.span.end as usize];
            let yes = &self.source[yes.start as usize..yes.end as usize];
            let no = &self.source[no.start as usize..no.end as usize];
            self.edits.push(analysis::Edit::replace(
                Span::new(start as u32, end as u32),
                format!("const {} = {test} ? {yes} : {no};", name.name),
            ));
        }
    }
}

fn literal_assignment<'a>(
    branch: &'a Statement<'a>,
    name: &str,
    symbol: SymbolId,
    scoping: &Scoping,
) -> Option<Span> {
    let Statement::BlockStatement(block) = branch else {
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
    if target.name != name || analysis::identifier_symbol(target, scoping) != Some(symbol) {
        return None;
    }
    match &assignment.right {
        Expression::NumericLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_) => Some(assignment.right.span()),
        _ => None,
    }
}

impl<'a> Visit<'a> for Scan<'_> {
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
    use super::collapse_literal_branch_joins;

    #[test]
    fn collapses_literal_arm_join() {
        let source = "function f(p) { let $SSA_4319; if (p) { $SSA_4319 = 72; } else { $SSA_4319 = 0; } return $SSA_4319; }";
        let result = collapse_literal_branch_joins(source).unwrap();
        assert_eq!(result.collapsed, 1);
        assert!(result.code.contains("const $SSA_4319 = p ? 72 : 0;"));
    }

    #[test]
    fn retains_effects_comments_and_extra_writes() {
        for source in [
            "function f(p) { let $SSA_1; if (p) { $SSA_1 = effect(); } else { $SSA_1 = 0; } return $SSA_1; }",
            "function f(p) { let $SSA_1; /* note */ if (p) { $SSA_1 = 1; } else { $SSA_1 = 0; } return $SSA_1; }",
            "function f(p) { let $SSA_1; if (p) { $SSA_1 = 1; } else { $SSA_1 = 0; } $SSA_1 = 2; return $SSA_1; }",
            "function f(p) { let $SSA_1; if (p) { sideEffect(); $SSA_1 = 1; } else { $SSA_1 = 0; } return $SSA_1; }",
        ] {
            let result = collapse_literal_branch_joins(source).unwrap();
            assert_eq!(result.collapsed, 0, "{source}");
        }
    }
}
