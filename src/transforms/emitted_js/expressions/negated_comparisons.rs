//! Write a negated comparison as the comparison it means.
//!
//! `!(a === b)` is `a !== b`, and `!(a == b)` is `a != b`, whatever `a` and
//! `b` hold. A relational test flips only over numbers that are never NaN:
//! `!(a < b)` is true when either side is NaN and `a >= b` is false, so
//! `!(i < n)` becomes `i >= n` only when `i` and `n` are both such numbers:
//!
//! - numeric literals, and `-`/`+`/`~` of them;
//! - bitwise and shift results (`x | 0`, `x >>> 7`), always integers;
//! - a `.length` read, taken to be an array's or a string's;
//! - a counter: a `let` that starts at one of these and is only ever
//!   stepped (`++`, `--`, `+=`/`-=` a literal) or set to one of these.
//!
//! The flipped test is parenthesized when an operator holds it, where
//! precedence would otherwise regroup it (`c < !(a < b)`).
//!
//! A negated `&&`/`||` group distributes the `!` (De Morgan) when every part
//! negates: `!(a != null && a.x === 1)` is `a == null || a.x !== 1`. A
//! comparison negates as above, a nested group the same way (parenthesized),
//! a bare value `v` as `!v`, and `!v` as `v` only in a test (`if`, loop,
//! `?:`), where its value is read for truth alone. The group is rewritten
//! only when it is left with at most one `!`, never more than it had.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::GetSpan;
use oxc_syntax::operator::{AssignmentOperator, BinaryOperator, LogicalOperator, UnaryOperator, UpdateOperator};

use crate::analysis;

pub(crate) fn flip_negated_comparisons(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = Scan { source, scoping: &scoping, nodes: &nodes, edits: Vec::new() };
    scan.visit_program(&program);
    if scan.edits.is_empty() {
        return Ok((source.into(), 0));
    }
    let flipped = scan.edits.len();
    let output = super::super::support::apply_checked(source, scan.edits, "negated comparison")?;
    Ok((output, flipped))
}

struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    edits: Vec<analysis::Edit>,
}

impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_unary_expression(&mut self, unary: &UnaryExpression<'a>) {
        if unary.operator == UnaryOperator::LogicalNot {
            if let Expression::LogicalExpression(_) = unary.argument.without_parentheses() {
                let parent = self.parent_of(unary.span);
                let test = matches!(&parent,
                    Some(AstKind::IfStatement(s)) if s.test.span() == unary.span)
                    || matches!(&parent, Some(AstKind::WhileStatement(s)) if s.test.span() == unary.span)
                    || matches!(&parent, Some(AstKind::DoWhileStatement(s)) if s.test.span() == unary.span)
                    || matches!(&parent, Some(AstKind::ForStatement(s)) if s.test.as_ref().is_some_and(|t| t.span() == unary.span))
                    || matches!(&parent, Some(AstKind::ConditionalExpression(c)) if c.test.span() == unary.span);
                if let Some((text, nots)) = self.negate(&unary.argument, test) {
                    if nots <= 1 {
                        let held = matches!(
                            parent,
                            Some(AstKind::BinaryExpression(_) | AstKind::LogicalExpression(_) | AstKind::UnaryExpression(_) | AstKind::StaticMemberExpression(_) | AstKind::ComputedMemberExpression(_))
                        );
                        self.edits.push(analysis::Edit::replace(unary.span, if held { format!("({text})") } else { text }));
                        return;
                    }
                }
            }
            if let Expression::BinaryExpression(binary) = unary.argument.without_parentheses() {
                if let Some(operator) = self.flipped(binary) {
                    let text = format!(
                        "{} {} {}",
                        &self.source[binary.left.span().start as usize..binary.left.span().end as usize],
                        operator.as_str(),
                        &self.source[binary.right.span().start as usize..binary.right.span().end as usize]
                    );
                    let text = if self.held_by_operator(unary.span) { format!("({text})") } else { text };
                    self.edits.push(analysis::Edit::replace(unary.span, text));
                    // The operands are copied as written: nothing inside
                    // them is rewritten too.
                    return;
                }
            }
        }
        walk::walk_unary_expression(self, unary);
    }
}

impl Scan<'_, '_> {
    fn flipped(&self, binary: &BinaryExpression<'_>) -> Option<BinaryOperator> {
        let equality = match binary.operator {
            BinaryOperator::StrictEquality => Some(BinaryOperator::StrictInequality),
            BinaryOperator::StrictInequality => Some(BinaryOperator::StrictEquality),
            BinaryOperator::Equality => Some(BinaryOperator::Inequality),
            BinaryOperator::Inequality => Some(BinaryOperator::Equality),
            _ => None,
        };
        if equality.is_some() {
            return equality;
        }
        let relational = match binary.operator {
            BinaryOperator::LessThan => BinaryOperator::GreaterEqualThan,
            BinaryOperator::LessEqualThan => BinaryOperator::GreaterThan,
            BinaryOperator::GreaterThan => BinaryOperator::LessEqualThan,
            BinaryOperator::GreaterEqualThan => BinaryOperator::LessThan,
            _ => return None,
        };
        (self.never_nan(&binary.left) && self.never_nan(&binary.right)).then_some(relational)
    }

    fn parent_of(&self, span: oxc_span::Span) -> Option<AstKind<'_>> {
        let node = self.nodes.iter().find(|node| matches!(node.kind(), AstKind::UnaryExpression(unary) if unary.span == span))?;
        Some(self.nodes.parent_kind(node.id()))
    }

    fn text(&self, span: oxc_span::Span) -> &str {
        &self.source[span.start as usize..span.end as usize]
    }

    /// `!expression` without a leading `!`, and how many `!` it keeps.
    fn negate(&self, expression: &Expression<'_>, test: bool) -> Option<(String, usize)> {
        match expression.without_parentheses() {
            Expression::BinaryExpression(binary) => {
                let operator = self.flipped(binary)?;
                Some((format!("{} {} {}", self.text(binary.left.span()), operator.as_str(), self.text(binary.right.span())), 0))
            }
            Expression::LogicalExpression(logical) => {
                let operator = match logical.operator {
                    LogicalOperator::And => "||",
                    LogicalOperator::Or => "&&",
                    LogicalOperator::Coalesce => return None,
                };
                let side = |side: &Expression<'_>| {
                    let (text, nots) = self.negate(side, test)?;
                    // A chain of the same operator needs no parentheses.
                    let grouped = matches!(side.without_parentheses(), Expression::LogicalExpression(inner) if inner.operator != logical.operator);
                    Some((if grouped { format!("({text})") } else { text }, nots))
                };
                let (left, left_nots) = side(&logical.left)?;
                let (right, right_nots) = side(&logical.right)?;
                Some((format!("{left} {operator} {right}"), left_nots + right_nots))
            }
            Expression::UnaryExpression(unary) if unary.operator == UnaryOperator::LogicalNot => {
                (test || boolean(&unary.argument)).then(|| (self.text(unary.argument.span()).to_string(), 0))
            }
            leaf @ (Expression::Identifier(_)
            | Expression::StaticMemberExpression(_)
            | Expression::ComputedMemberExpression(_)
            | Expression::CallExpression(_)
            | Expression::ThisExpression(_)) => Some((format!("!{}", self.text(leaf.span())), 1)),
            _ => None,
        }
    }

    /// The `!` sits directly in an operator (unary, binary, `in`, …), not in
    /// a test, an argument, a statement or a logical operand.
    fn held_by_operator(&self, span: oxc_span::Span) -> bool {
        let Some(node) = self.nodes.iter().find(|node| matches!(node.kind(), AstKind::UnaryExpression(unary) if unary.span == span)) else {
            return true;
        };
        matches!(
            self.nodes.parent_kind(node.id()),
            AstKind::BinaryExpression(_) | AstKind::UnaryExpression(_) | AstKind::StaticMemberExpression(_) | AstKind::ComputedMemberExpression(_)
        )
    }

    fn never_nan(&self, expression: &Expression<'_>) -> bool {
        match expression.without_parentheses() {
            Expression::NumericLiteral(_) => true,
            Expression::UnaryExpression(unary) => match unary.operator {
                UnaryOperator::BitwiseNot => true,
                UnaryOperator::UnaryNegation | UnaryOperator::UnaryPlus => self.never_nan(&unary.argument),
                _ => false,
            },
            Expression::BinaryExpression(binary) => matches!(
                binary.operator,
                BinaryOperator::BitwiseOR
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseXOR
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::ShiftRightZeroFill
            ),
            Expression::StaticMemberExpression(member) => !member.optional && member.property.name == "length",
            Expression::ComputedMemberExpression(member) => {
                !member.optional && matches!(&member.expression, Expression::StringLiteral(key) if key.value == "length")
            }
            Expression::Identifier(name) => self
                .scoping
                .get_reference(name.reference_id())
                .symbol_id()
                .is_some_and(|symbol| self.counter(symbol)),
            _ => false,
        }
    }

    /// A `let`/`const` that starts never NaN and is only stepped or set to
    /// a value that is never NaN.
    fn counter(&self, symbol: SymbolId) -> bool {
        let AstKind::VariableDeclarator(declarator) = self.nodes.kind(self.scoping.symbol_declaration(symbol)) else {
            return false;
        };
        if declarator.kind == VariableDeclarationKind::Var || !declarator.init.as_ref().is_some_and(|init| self.constant(init)) {
            return false;
        }
        self.scoping.get_resolved_references(symbol).all(|reference| {
            if !reference.is_write() {
                return true;
            }
            match self.nodes.parent_kind(reference.node_id()) {
                AstKind::UpdateExpression(update) => matches!(update.operator, UpdateOperator::Increment | UpdateOperator::Decrement),
                AstKind::AssignmentExpression(assignment) => match assignment.operator {
                    AssignmentOperator::Assign => self.constant(&assignment.right),
                    AssignmentOperator::Addition | AssignmentOperator::Subtraction => {
                        matches!(assignment.right.without_parentheses(), Expression::NumericLiteral(_))
                    }
                    AssignmentOperator::BitwiseOR
                    | AssignmentOperator::BitwiseAnd
                    | AssignmentOperator::BitwiseXOR
                    | AssignmentOperator::ShiftLeft
                    | AssignmentOperator::ShiftRight
                    | AssignmentOperator::ShiftRightZeroFill => true,
                    _ => false,
                },
                _ => false,
            }
        })
    }

    /// Never NaN without reading any binding: a literal or a bitwise result.
    fn constant(&self, expression: &Expression<'_>) -> bool {
        match expression.without_parentheses() {
            Expression::NumericLiteral(_) => true,
            Expression::UnaryExpression(unary) => match unary.operator {
                UnaryOperator::BitwiseNot => true,
                UnaryOperator::UnaryNegation | UnaryOperator::UnaryPlus => self.constant(&unary.argument),
                _ => false,
            },
            expression @ Expression::BinaryExpression(_) => self.never_nan(expression),
            _ => false,
        }
    }
}

/// Its value is `true` or `false`.
fn boolean(expression: &Expression<'_>) -> bool {
    match expression.without_parentheses() {
        Expression::BinaryExpression(binary) => matches!(
            binary.operator,
            BinaryOperator::StrictEquality
                | BinaryOperator::StrictInequality
                | BinaryOperator::Equality
                | BinaryOperator::Inequality
                | BinaryOperator::LessThan
                | BinaryOperator::LessEqualThan
                | BinaryOperator::GreaterThan
                | BinaryOperator::GreaterEqualThan
                | BinaryOperator::In
                | BinaryOperator::Instanceof
        ),
        Expression::UnaryExpression(unary) => unary.operator == UnaryOperator::LogicalNot,
        Expression::LogicalExpression(logical) => logical.operator != LogicalOperator::Coalesce && boolean(&logical.left) && boolean(&logical.right),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flip(source: &str) -> String {
        flip_negated_comparisons(source).unwrap().0
    }

    #[test]
    fn flips_equality_always() {
        assert_eq!(flip("if (!(a === b)) f();\nif (!(a != b)) f();\n"), "if (a !== b) f();\nif (a == b) f();\n");
    }

    #[test]
    fn flips_a_counter_against_a_length() {
        let source = "function f(a) {\n\tlet i = 0;\n\tfor (;;) {\n\t\tif (!(i < a.length)) break;\n\t\tif (!(i <= a[\"length\"])) break;\n\t\t++i;\n\t\ti += 2;\n\t}\n\treturn !(x >>> 7 > 0);\n}\n";
        assert_eq!(
            flip(source),
            "function f(a) {\n\tlet i = 0;\n\tfor (;;) {\n\t\tif (i >= a.length) break;\n\t\tif (i > a[\"length\"]) break;\n\t\t++i;\n\t\ti += 2;\n\t}\n\treturn x >>> 7 <= 0;\n}\n"
        );
    }

    #[test]
    fn keeps_tests_that_can_see_nan() {
        // `x` may be anything; `j` is set from a call; `k` has no initializer;
        // `m` is a `var`; `n` takes `*=`.
        let source = "function f(a, x) {\n\tlet j = 0;\n\tj = g();\n\tlet k;\n\tvar m = 0;\n\tlet n = 1;\n\tn *= 2;\n\treturn [!(x < a.length), !(j < 1), !(k < 1), !(m < 1), !(n < 1), !(a.b < 1)];\n}\n";
        assert_eq!(flip_negated_comparisons(source).unwrap().1, 0);
    }

    #[test]
    fn distributes_a_negated_group() {
        assert_eq!(
            flip("if (!(a != null && a.x === 1 && b.y === true)) f();\nz = !(typeof c === \"object\" || typeof c === \"function\");\nif (!(d && d.e != null)) f();\nw = q && !(a === 1 || (b === 2 && c === 3));\n"),
            "if (a == null || a.x !== 1 || b.y !== true) f();\nz = typeof c !== \"object\" && typeof c !== \"function\";\nif (!d || d.e == null) f();\nw = q && (a !== 1 && (b !== 2 || c !== 3));\n"
        );
    }

    #[test]
    fn keeps_a_group_that_would_gain_negations_or_change_value() {
        // Two bare values; a relational part over maybe-NaN values; `!v` out
        // of a test would give `v`, not a boolean.
        let source = "if (!(a && b)) f();\nif (!(x > 0 && x < 1)) f();\nz = !(!v || w === 1);\n";
        assert_eq!(flip_negated_comparisons(source).unwrap().1, 0);
    }

    #[test]
    fn parenthesizes_inside_an_operator() {
        assert_eq!(flip("let c = 0;\ny = c < !(1 < 2);\nz = !(a === b) && q;\n"), "let c = 0;\ny = c < (1 >= 2);\nz = a !== b && q;\n");
    }
}
