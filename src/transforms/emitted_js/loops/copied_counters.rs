//! Step a loop counter itself, not the copy the body takes of it.
//!
//! ```js
//! let j = 0;
//! let c;
//! for (;; j = Number(c) + 1) {
//! 	c = j;
//! 	if (!(j < a.length)) break;
//! 	use(a[c]);
//! }
//! ```
//!
//! From its first store on, `c` holds `j` except between the update and the
//! next store, where nothing runs (the loop has no test of its own). So every
//! read of `c` is a read of `j`, after the loop too, and the loop becomes
//! `for (; j < a.length; j++) { use(a[j]); }`. It must be so: `j` is a
//! `let` starting at a number and written only by the update, `c` a `let`
//! without a value written only by the copy, both declared in one scope, and
//! no read of `c` comes before the copy or sits in a function. A leading
//! `if (!e) break;` becomes the loop's test.

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast::AstKind;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};

use crate::analysis;
use super::super::support::whole_line;

pub(crate) fn step_copied_counters(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = Scan { source, scoping: &scoping, nodes: &nodes, edits: Vec::new(), rewritten: 0 };
    scan.visit_program(&program);
    if scan.rewritten == 0 {
        return Ok((source.into(), 0));
    }
    let output = super::super::support::apply_checked(source, scan.edits, "copied counter")?;
    Ok((output, scan.rewritten))
}

struct Scan<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    edits: Vec<analysis::Edit>,
    rewritten: usize,
}

fn symbol_of(scoping: &Scoping, name: &IdentifierReference<'_>) -> Option<SymbolId> {
    scoping.get_reference(name.reference_id()).symbol_id()
}

impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_for_statement(&mut self, statement: &ForStatement<'a>) {
        if !self.rewrite(statement) {
            walk::walk_for_statement(self, statement);
        }
    }
}

impl Scan<'_, '_> {
    fn text(&self, span: Span) -> &str {
        &self.source[span.start as usize..span.end as usize]
    }

    fn rewrite(&mut self, statement: &ForStatement<'_>) -> bool {
        if statement.init.is_some() || statement.test.is_some() {
            return false;
        }
        // `j = Number(c) + 1` or `j = c + 1`.
        let Some(Expression::AssignmentExpression(update)) = &statement.update else {
            return false;
        };
        let AssignmentTarget::AssignmentTargetIdentifier(counter) = &update.left else {
            return false;
        };
        let Expression::BinaryExpression(step) = &update.right else {
            return false;
        };
        if update.operator != AssignmentOperator::Assign
            || step.operator != BinaryOperator::Addition
            || !matches!(&step.right, Expression::NumericLiteral(one) if one.value == 1.0)
        {
            return false;
        }
        let copy_read = match &step.left {
            Expression::Identifier(copy) => copy,
            Expression::CallExpression(call) => match (&call.callee, call.arguments.as_slice()) {
                (Expression::Identifier(number), [Argument::Identifier(copy)])
                    if number.name == "Number" && self.scoping.get_reference(number.reference_id()).symbol_id().is_none() && !call.optional =>
                {
                    copy
                }
                _ => return false,
            },
            _ => return false,
        };
        let (Some(counter_symbol), Some(copy_symbol)) = (symbol_of(self.scoping, counter), symbol_of(self.scoping, copy_read)) else {
            return false;
        };
        // The body opens with `c = j;`.
        let Statement::BlockStatement(body) = &statement.body else {
            return false;
        };
        let Some(Statement::ExpressionStatement(first)) = body.body.first() else {
            return false;
        };
        let Expression::AssignmentExpression(copy) = &first.expression else {
            return false;
        };
        let (AssignmentTarget::AssignmentTargetIdentifier(copy_target), Expression::Identifier(copied)) = (&copy.left, &copy.right) else {
            return false;
        };
        if copy.operator != AssignmentOperator::Assign
            || symbol_of(self.scoping, copy_target) != Some(copy_symbol)
            || symbol_of(self.scoping, copied) != Some(counter_symbol)
        {
            return false;
        }
        if !self.counter_ok(counter_symbol, update.span) || !self.copy_ok(copy_symbol, first.span, update.span) {
            return false;
        }
        let Some(copy_declaration) = self.lone_declaration(copy_symbol) else {
            return false;
        };
        if self.scoping.symbol_scope_id(counter_symbol) != self.scoping.symbol_scope_id(copy_symbol) {
            return false;
        }

        // Every read of the copy reads the counter.
        let counter_name = counter.name.as_str();
        let reads: Vec<Span> = self
            .scoping
            .get_resolved_references(copy_symbol)
            .filter(|reference| reference.is_read())
            .map(|reference| self.nodes.kind(reference.node_id()).span())
            .collect();
        // A leading `if (!e) break;` (after the copy) becomes the test.
        let mut test = None;
        if let Some(Statement::IfStatement(branch)) = body.body.get(1) {
            let breaks = match &branch.consequent {
                Statement::BreakStatement(jump) => jump.label.is_none(),
                Statement::BlockStatement(block) => matches!(block.body.as_slice(), [Statement::BreakStatement(jump)] if jump.label.is_none()),
                _ => false,
            };
            if let (true, None, Expression::UnaryExpression(not)) = (breaks, &branch.alternate, &branch.test) {
                if not.operator == UnaryOperator::LogicalNot {
                    test = Some((branch.span, not.argument.without_parentheses().span()));
                }
            }
        }
        let substituted = |span: Span| {
            let mut text = self.text(span).to_string();
            for read in reads.iter().rev().filter(|read| span.start <= read.start && read.end <= span.end) {
                let (start, end) = ((read.start - span.start) as usize, (read.end - span.start) as usize);
                text.replace_range(start..end, counter_name);
            }
            text
        };
        let header = match test {
            Some((_, condition)) => format!("for (; {}; {counter_name}++) ", substituted(condition)),
            None => format!("for (;; {counter_name}++) "),
        };
        self.edits.push(analysis::Edit::replace(Span::new(statement.span.start, statement.body.span().start), header));
        self.edits.push(analysis::Edit::delete(whole_line(self.source, first.span)));
        if let Some((branch, _)) = test {
            self.edits.push(analysis::Edit::delete(whole_line(self.source, branch)));
        }
        self.edits.push(analysis::Edit::delete(whole_line(self.source, copy_declaration)));
        let skip = |read: &Span| {
            update.span.start <= read.start && read.end <= update.span.end
                || test.is_some_and(|(branch, _)| branch.start <= read.start && read.end <= branch.end)
        };
        for read in reads.iter().filter(|read| !skip(read)) {
            self.edits.push(analysis::Edit::replace(*read, counter_name.to_string()));
        }
        self.rewritten += 1;
        true
    }

    /// `let j = <number>`, written only by `update`.
    fn counter_ok(&self, symbol: SymbolId, update: Span) -> bool {
        let AstKind::VariableDeclarator(declarator) = self.nodes.kind(self.scoping.symbol_declaration(symbol)) else {
            return false;
        };
        declarator.kind == VariableDeclarationKind::Let
            && matches!(&declarator.init, Some(Expression::NumericLiteral(_)))
            && self.scoping.get_resolved_references(symbol).filter(|reference| reference.is_write()).all(|reference| {
                let span = self.nodes.kind(reference.node_id()).span();
                update.start <= span.start && span.end <= update.end
            })
    }

    /// `let c;`, written only by the copy, read only after it (or by the
    /// update, which runs after it) and in no function the loop's function
    /// does not own.
    fn copy_ok(&self, symbol: SymbolId, copy: Span, update: Span) -> bool {
        let AstKind::VariableDeclarator(declarator) = self.nodes.kind(self.scoping.symbol_declaration(symbol)) else {
            return false;
        };
        if declarator.kind != VariableDeclarationKind::Let || declarator.init.is_some() {
            return false;
        }
        let function_of = |node| super::super::support::owner(self.nodes, node);
        let declared_in = function_of(self.scoping.symbol_declaration(symbol));
        self.scoping.get_resolved_references(symbol).all(|reference| {
            let span = self.nodes.kind(reference.node_id()).span();
            let within = |outer: Span| outer.start <= span.start && span.end <= outer.end;
            let read = reference.is_read() && !reference.is_write();
            (within(copy) || (read && (span.start >= copy.end || within(update))))
                && function_of(reference.node_id()) == declared_in
        })
    }

    /// The `let c;` statement declaring only `c`.
    fn lone_declaration(&self, symbol: SymbolId) -> Option<Span> {
        let declarator = self.scoping.symbol_declaration(symbol);
        match self.nodes.parent_kind(declarator) {
            AstKind::VariableDeclaration(declaration) if declaration.declarations.len() == 1 => Some(declaration.span),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_the_counter_and_reads_it_for_the_copy() {
        let source = "function f(a, out) {\n\tlet j = 0;\n\tlet c;\n\tfor (;; j = Number(c) + 1) {\n\t\tc = j;\n\t\tif (!(j < a.length)) break;\n\t\tout.push(a[c]);\n\t}\n\treturn c;\n}\n";
        let (code, n) = step_copied_counters(source).unwrap();
        assert_eq!(n, 1);
        assert_eq!(code, "function f(a, out) {\n\tlet j = 0;\n\tfor (; j < a.length; j++) {\n\t\tout.push(a[j]);\n\t}\n\treturn j;\n}\n");
    }

    #[test]
    fn keeps_a_copy_read_before_it_is_taken_or_from_a_closure() {
        for extra in ["\tg(c);\n", "\tg(() => c);\n"] {
            let source = format!("function f(a) {{\n\tlet j = 0;\n\tlet c;\n{extra}\tfor (;; j = Number(c) + 1) {{\n\t\tc = j;\n\t\tif (!(j < a.length)) break;\n\t}}\n}}\n");
            assert_eq!(step_copied_counters(&source).unwrap().1, 0, "{extra}");
        }
        // The counter written in the body.
        let source = "function f(a) {\n\tlet j = 0;\n\tlet c;\n\tfor (;; j = Number(c) + 1) {\n\t\tc = j;\n\t\tj = 5;\n\t}\n}\n";
        assert_eq!(step_copied_counters(source).unwrap().1, 0);
    }
}
