//! Write a value built up by `|` steps as one expression where it is used.
//!
//! ```js
//! let $BF = $KEY_102 | 0;
//! $BF = $BF | $KEY_19 << 1;
//! $BE[4] = $BD() ^ 130;
//! $BF = $BF | $KEY_67 << 4;
//! $BE[5] = ($BF | $KEY_17 << 6) ^ $BD();
//! ```
//!
//! becomes `$BE[5] = ($KEY_102 | 0 | $KEY_19 << 1 | $KEY_67 << 4 |
//! $KEY_17 << 6) ^ $BD();`. The accumulator is a `let` no function names,
//! updated only by statements `A = A | T;` in the same statement list and
//! read once more, by a later statement there. Its start and each `T` are
//! names, literals and operators (no call, property read or store), so they
//! may run at that read instead, provided each name they read holds the
//! same value there: nothing writes it in between, and no function writes
//! it at all.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};

use crate::analysis;
use super::super::support::whole_line;

pub(crate) fn fold_bit_accumulators(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = Scan { source, scoping: &scoping, nodes: &nodes, edits: Vec::new(), folded: 0 };
    super::super::support::for_each_list(&program, |list| scan.list(list));
    if scan.folded == 0 {
        return Ok((source.into(), 0));
    }
    let output = super::super::support::apply_checked(source, scan.edits, "bit accumulator")?;
    Ok((output, scan.folded))
}

struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    edits: Vec<analysis::Edit>,
    folded: usize,
}


/// Names, literals and operators only; the names it reads.
fn plain<'e>(expression: &'e Expression<'_>, names: &mut Vec<&'e IdentifierReference<'e>>) -> bool {
    match expression {
        Expression::Identifier(name) => {
            names.push(name);
            true
        }
        Expression::NumericLiteral(_) | Expression::BooleanLiteral(_) | Expression::NullLiteral(_) | Expression::StringLiteral(_) => true,
        Expression::ParenthesizedExpression(inner) => plain(&inner.expression, names),
        Expression::UnaryExpression(unary) => {
            !matches!(unary.operator, UnaryOperator::Delete | UnaryOperator::Typeof | UnaryOperator::Void) && plain(&unary.argument, names)
        }
        Expression::BinaryExpression(binary) => {
            !matches!(binary.operator, BinaryOperator::In | BinaryOperator::Instanceof) && plain(&binary.left, names) && plain(&binary.right, names)
        }
        Expression::LogicalExpression(logical) => plain(&logical.left, names) && plain(&logical.right, names),
        _ => false,
    }
}

/// `A | X | Y`: the text after `A`, when `A` is the chain's first operand.
fn after_first<'s>(source: &'s str, expression: &Expression<'_>, symbol: SymbolId, scoping: &Scoping) -> Option<(&'s str, Span)> {
    let mut left = expression;
    while let Expression::BinaryExpression(binary) = left {
        if binary.operator != BinaryOperator::BitwiseOR {
            return None;
        }
        left = &binary.left;
    }
    let Expression::Identifier(first) = left else {
        return None;
    };
    if scoping.get_reference(first.reference_id()).symbol_id() != Some(symbol) || first.span == expression.span() {
        return None;
    }
    let tail = Span::new(first.span.end, expression.span().end);
    Some((&source[tail.start as usize..tail.end as usize], tail))
}

impl Scan<'_, '_> {
    fn text(&self, span: Span) -> &str {
        &self.source[span.start as usize..span.end as usize]
    }

    fn function_of(&self, node: oxc_semantic::NodeId) -> Option<oxc_semantic::NodeId> {
        super::super::support::owner(self.nodes, node)
    }

    fn list(&mut self, statements: &[Statement<'_>]) {
        let mut used_until = 0;
        for (index, statement) in statements.iter().enumerate() {
            if index < used_until {
                continue;
            }
            if let Some(last) = self.fold(statements, index, statement) {
                used_until = last + 1;
            }
        }
    }

    /// Folds the accumulator `let`ed at `index`; returns the index of the
    /// statement that reads it.
    fn fold(&mut self, statements: &[Statement<'_>], index: usize, statement: &Statement<'_>) -> Option<usize> {
        let Statement::VariableDeclaration(declaration) = statement else {
            return None;
        };
        let [declarator] = declaration.declarations.as_slice() else {
            return None;
        };
        let (BindingPattern::BindingIdentifier(id), Some(start)) = (&declarator.id, &declarator.init) else {
            return None;
        };
        if declaration.kind != VariableDeclarationKind::Let {
            return None;
        }
        let symbol = id.symbol_id();
        let home = self.function_of(self.scoping.symbol_declaration(symbol));
        let references: Vec<_> = self.scoping.get_resolved_references(symbol).collect();
        if references.iter().any(|reference| self.function_of(reference.node_id()) != home) {
            return None;
        }
        let mut names = Vec::new();
        if !plain(start, &mut names) {
            return None;
        }
        // Terms with where they were: the start, then each step's tail.
        let start_text = match start.without_parentheses() {
            Expression::LogicalExpression(_) | Expression::ConditionalExpression(_) => format!("({})", self.text(start.span())),
            _ => self.text(start.span()).to_string(),
        };
        let mut terms: Vec<(String, u32, Vec<SymbolId>)> = vec![(start_text, statement.span().end, self.symbols(&names)?)];
        let mut steps = Vec::new();
        let mut reads = 0;
        let mut last = None;
        for (offset, later) in statements[index + 1..].iter().enumerate() {
            let mentions = references.iter().filter(|reference| {
                let span = self.nodes.kind(reference.node_id()).span();
                later.span().start <= span.start && span.end <= later.span().end
            });
            let count = mentions.count();
            if count == 0 {
                continue;
            }
            // A step `A = A | T;`.
            if let Statement::ExpressionStatement(expression) = later {
                if let Expression::AssignmentExpression(assignment) = &expression.expression {
                    if assignment.operator == AssignmentOperator::Assign
                        && matches!(&assignment.left, AssignmentTarget::AssignmentTargetIdentifier(target)
                            if self.scoping.get_reference(target.reference_id()).symbol_id() == Some(symbol))
                    {
                        let (tail, _) = after_first(self.source, &assignment.right, symbol, self.scoping)?;
                        if count != 2 {
                            return None;
                        }
                        let mut names = Vec::new();
                        let Expression::BinaryExpression(_) = &assignment.right else { return None };
                        if !self.tail_plain(&assignment.right, &mut names) {
                            return None;
                        }
                        terms.push((tail.to_string(), later.span().end, self.symbols(&names)?));
                        steps.push(later.span());
                        reads += 1;
                        continue;
                    }
                }
            }
            // The one read.
            if count != 1 {
                return None;
            }
            last = Some((index + 1 + offset, later));
            break;
        }
        let (last_index, last_statement) = last?;
        if steps.is_empty() || references.len() != 2 * reads + 1 {
            return None;
        }
        let read = references
            .iter()
            .map(|reference| (self.nodes.kind(reference.node_id()).span(), reference.node_id()))
            .find(|(span, _)| last_statement.span().start <= span.start && span.end <= last_statement.span().end)?;
        // The read must run in this function, not in one the statement makes.
        if self.function_of(read.1) != home {
            return None;
        }
        // Each term's names hold the same value at the read.
        for (_, from, symbols) in &terms {
            for symbol in symbols {
                if !self.unchanged(*symbol, *from, read.0.start, home) {
                    return None;
                }
            }
        }
        let combined: String = terms.iter().map(|(text, _, _)| text.as_str()).collect();
        let first_of_or = matches!(self.nodes.parent_kind(read.1), AstKind::BinaryExpression(binary)
            if binary.operator == BinaryOperator::BitwiseOR && binary.left.span() == read.0);
        let replacement = if first_of_or { combined } else { format!("({combined})") };
        let mut edits = vec![analysis::Edit::delete(whole_line(self.source, statement.span()))];
        for step in steps {
            edits.push(analysis::Edit::delete(whole_line(self.source, step)));
        }
        edits.push(analysis::Edit::replace(read.0, replacement));
        // A fold whose edits touch another's text (a read inside a line
        // another fold deletes, say) is declined alone: the rest still fold.
        let overlaps = |a: &analysis::Edit, b: &analysis::Edit| {
            a.start < b.end && b.start < a.end || (a.start == b.start && (a.start == a.end || b.start == b.end))
        };
        if edits.iter().any(|edit| self.edits.iter().any(|done| overlaps(edit, done))) {
            return None;
        }
        self.edits.extend(edits);
        self.folded += 1;
        Some(last_index)
    }

    /// The operands after the accumulator in `A | X | Y` are plain.
    fn tail_plain<'e>(&self, expression: &'e Expression<'_>, names: &mut Vec<&'e IdentifierReference<'e>>) -> bool {
        match expression {
            Expression::BinaryExpression(binary) if binary.operator == BinaryOperator::BitwiseOR => {
                let left_ok = match &binary.left {
                    Expression::Identifier(_) => true,
                    left @ Expression::BinaryExpression(_) => self.tail_plain(left, names),
                    _ => false,
                };
                left_ok && plain(&binary.right, names)
            }
            _ => false,
        }
    }

    fn symbols(&self, names: &[&IdentifierReference<'_>]) -> Option<Vec<SymbolId>> {
        let mut symbols = Vec::new();
        for name in names {
            match self.scoping.get_reference(name.reference_id()).symbol_id() {
                Some(symbol) => symbols.push(symbol),
                // A global is taken not to change.
                None => {}
            }
        }
        Some(symbols)
    }

    /// No write of `symbol` between `from` and `to`, and none in a function
    /// other than `home`.
    fn unchanged(&self, symbol: SymbolId, from: u32, to: u32, home: Option<oxc_semantic::NodeId>) -> bool {
        self.scoping.get_resolved_references(symbol).filter(|reference| reference.is_write()).all(|reference| {
            let span = self.nodes.kind(reference.node_id()).span();
            self.function_of(reference.node_id()) == home && !(from <= span.start && span.start < to)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_the_steps_into_the_read() {
        let source = "function f(p, q, d, e) {\n\tlet a = p | 0;\n\tlet k = !!q;\n\ta = a | k << 1;\n\te[4] = d() ^ 130;\n\ta = a | q << 2 | p << 3;\n\te[5] = (a | k << 6) ^ d();\n\tlet b = p | 0;\n\tb = b | q << 1;\n\te[6] = b ^ d();\n}\n";
        let (code, n) = fold_bit_accumulators(source).unwrap();
        assert_eq!(n, 2);
        assert_eq!(code, "function f(p, q, d, e) {\n\tlet k = !!q;\n\te[4] = d() ^ 130;\n\te[5] = (p | 0 | k << 1 | q << 2 | p << 3 | k << 6) ^ d();\n\te[6] = (p | 0 | q << 1) ^ d();\n}\n");
    }

    #[test]
    fn declines_only_the_fold_that_overlaps_another() {
        // `b`'s read is inside the line that folding `a` deletes: one of
        // them folds, the other stays as written, and the pass succeeds.
        let source = "function f(p, q, e) {\n\tlet b = p | 0;\n\tb = b | q << 1;\n\tlet a = b | 0;\n\ta = a | q << 2;\n\te[1] = a;\n}\n";
        let (code, n) = fold_bit_accumulators(source).unwrap();
        assert!(n >= 1, "{code}");
    }

    #[test]
    fn keeps_steps_whose_names_change_before_the_read() {
        // `k` is stored again before the read; `m` by a closure; a step
        // calls; the accumulator is read twice.
        let source = "function f(p, d, e) {\n\tlet k = 1, m = 2;\n\tlet a = p | 0;\n\ta = a | k << 1;\n\tk = 0;\n\te[1] = a;\n\tlet b = p | 0;\n\tb = b | m << 1;\n\tg(() => { m = 5; });\n\te[2] = b;\n\tlet c = p | 0;\n\tc = c | d() << 1;\n\te[3] = c;\n\tlet h = p | 0;\n\th = h | 1;\n\te[4] = h + h;\n}\n";
        let (code, n) = fold_bit_accumulators(source).unwrap();
        assert_eq!(n, 0, "{code}");
    }
}
