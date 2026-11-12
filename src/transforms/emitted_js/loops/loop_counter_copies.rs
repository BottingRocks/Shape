//! Read a loop counter's top-of-body copy as the counter itself.
//!
//! Splitting a comma loop test leaves
//!
//! ```js
//! let c;
//! for (;; j = c + 1) {
//!     c = j;
//!     if (!(j < n)) break;
//!     … c …
//! }
//! ```
//!
//! When nothing but the copy writes `c`, nothing but the step writes `j`,
//! and `c` is read only in this loop's body and step, `c` equals `j`
//! everywhere it is read: the reads become `j`, and the copy and `c`'s
//! declaration go, leaving the plain counted loop the final tidy rebuilds
//! (`for (; j < n; j++)`). A read inside a nested function could run after
//! a later step, where the two differ: such loops stay.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator};

use crate::analysis;
use super::super::support::whole_line;

pub(crate) fn merge_loop_counter_copies(source: &str) -> Result<(String, usize), String> {
    if !source.contains("for (;; ") {
        return Ok((source.into(), 0));
    }
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = Scan { source, scoping: &scoping, nodes: &nodes, edits: Vec::new(), count: 0 };
    super::super::support::for_each_list(&program, |list| scan.list(list));
    if scan.count == 0 {
        return Ok((source.into(), 0));
    }
    let applied = analysis::apply_edits(source, scan.edits);
    // Copies nested in one another touch the same lines: leave them.
    if !applied.rejected.is_empty() {
        return Ok((source.into(), 0));
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output).map_err(|error| format!("loop counter copy output: {error}"))?;
    Ok((applied.output, scan.count))
}

struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    edits: Vec<analysis::Edit>,
    count: usize,
}

fn name_symbol(expression: &Expression<'_>, scoping: &Scoping) -> Option<SymbolId> {
    match expression {
        Expression::Identifier(identifier) => analysis::identifier_symbol(identifier, scoping),
        _ => None,
    }
}

impl Scan<'_, '_> {
    /// `j = c + 1` or `j = Number(c) + 1`: the counter and the copy.
    fn step(&self, update: &Expression<'_>) -> Option<(SymbolId, SymbolId)> {
        let Expression::AssignmentExpression(assignment) = update else {
            return None;
        };
        let AssignmentTarget::AssignmentTargetIdentifier(counter) = &assignment.left else {
            return None;
        };
        let Expression::BinaryExpression(sum) = &assignment.right else {
            return None;
        };
        if assignment.operator != AssignmentOperator::Assign
            || sum.operator != BinaryOperator::Addition
            || !matches!(&sum.right, Expression::NumericLiteral(one) if one.value == 1.0)
        {
            return None;
        }
        let copy = match &sum.left {
            Expression::CallExpression(call) if matches!(&call.callee, Expression::Identifier(callee) if callee.name == "Number" && analysis::identifier_symbol(callee, self.scoping).is_none()) => {
                let [argument] = call.arguments.as_slice() else {
                    return None;
                };
                name_symbol(argument.as_expression()?, self.scoping)?
            }
            other => name_symbol(other, self.scoping)?,
        };
        Some((analysis::identifier_symbol(counter, self.scoping)?, copy))
    }

    fn span_of(&self, reference: &oxc_semantic::Reference) -> Span {
        self.nodes.get_node(reference.node_id()).span()
    }

    fn inside_function(&self, reference: &oxc_semantic::Reference, loop_scope: oxc_semantic::ScopeId) -> bool {
        // The reference's scope chain reaches the loop's scope only through
        // block scopes.
        let mut scope = self.nodes.get_node(reference.node_id()).scope_id();
        while scope != loop_scope {
            if self.scoping.scope_flags(scope).is_function() {
                return true;
            }
            let Some(parent) = self.scoping.scope_parent_id(scope) else {
                return true;
            };
            scope = parent;
        }
        false
    }

    fn list(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            let Statement::ForStatement(loop_) = statement else {
                continue;
            };
            if loop_.test.is_some() || loop_.init.is_some() {
                continue;
            }
            let Some((counter, copy)) = loop_.update.as_ref().and_then(|update| self.step(update)) else {
                continue;
            };
            if counter == copy {
                continue;
            }
            let Statement::BlockStatement(body) = &loop_.body else {
                continue;
            };
            // c = j; first in the body.
            let Some(Statement::ExpressionStatement(first)) = body.body.first() else {
                continue;
            };
            let Expression::AssignmentExpression(copied) = &first.expression else {
                continue;
            };
            let copies = copied.operator == AssignmentOperator::Assign
                && matches!(&copied.left, AssignmentTarget::AssignmentTargetIdentifier(target) if analysis::identifier_symbol(target, self.scoping) == Some(copy))
                && name_symbol(&copied.right, self.scoping) == Some(counter);
            if !copies {
                continue;
            }
            let within = |span: Span| loop_.span.start <= span.start && span.end <= loop_.span.end;
            let Some(loop_scope) = loop_.scope_id.get() else {
                continue;
            };
            // Only the copy writes c; c is read only in this loop, outside
            // nested functions.
            let copy_references: Vec<_> = self.scoping.get_resolved_references(copy).collect();
            let copy_ok = copy_references.iter().all(|reference| {
                let span = self.span_of(reference);
                within(span)
                    && !self.inside_function(reference, loop_scope)
                    && (!reference.is_write() || span == copied.left.span())
            });
            // Only the step writes j.
            let counter_ok = self.scoping.get_resolved_references(counter).all(|reference| {
                !reference.is_write() || {
                    let span = self.span_of(reference);
                    !within(span) || loop_.update.as_ref().is_some_and(|update| match update {
                        Expression::AssignmentExpression(step) => span == step.left.span(),
                        _ => false,
                    })
                }
            });
            if !copy_ok || !counter_ok {
                continue;
            }
            // c's own `let c;` declaration, alone on its statement.
            let declaration = self.scoping.symbol_declaration(copy);
            let declaration = self.nodes.parent_node(declaration);
            let oxc_ast::AstKind::VariableDeclaration(declared) = declaration.kind() else {
                continue;
            };
            if declared.declarations.len() != 1 || declared.declarations[0].init.is_some() {
                continue;
            }
            let counter_name = self.scoping.symbol_name(counter).to_string();
            for reference in &copy_references {
                if !reference.is_write() {
                    self.edits.push(analysis::Edit::replace(self.span_of(reference), counter_name.clone()));
                }
            }
            self.edits.push(analysis::Edit::delete(whole_line(self.source, first.span)));
            self.edits.push(analysis::Edit::delete(whole_line(self.source, declared.span)));
            self.count += 1;
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_copy_as_the_counter() {
        let source = "function f(a, o) {\n\tlet j = 0;\n\tlet c;\n\tfor (;; j = c + 1) {\n\t\tc = j;\n\t\tif (!(j < a.length)) break;\n\t\to[a[c]] = c;\n\t}\n}\n";
        let (code, count) = merge_loop_counter_copies(source).unwrap();
        assert_eq!(count, 1);
        assert_eq!(code, "function f(a, o) {\n\tlet j = 0;\n\tfor (;; j = j + 1) {\n\t\tif (!(j < a.length)) break;\n\t\to[a[j]] = j;\n\t}\n}\n");
    }

    #[test]
    fn keeps_a_copy_read_later_or_by_a_closure() {
        for source in [
            "function f(a, o) {\n\tlet j = 0;\n\tlet c;\n\tfor (;; j = c + 1) {\n\t\tc = j;\n\t\tif (!(j < a.length)) break;\n\t}\n\treturn c;\n}\n",
            "function f(a, o) {\n\tlet j = 0;\n\tlet c;\n\tfor (;; j = c + 1) {\n\t\tc = j;\n\t\tif (!(j < a.length)) break;\n\t\to.push(() => c);\n\t}\n}\n",
            "function f(a, o) {\n\tlet j = 0;\n\tlet c;\n\tfor (;; j = c + 1) {\n\t\tc = j;\n\t\tif (!(j < a.length)) break;\n\t\tj = 7;\n\t}\n}\n",
        ] {
            let (_, count) = merge_loop_counter_copies(source).unwrap();
            assert_eq!(count, 0, "{source}");
        }
    }
}
