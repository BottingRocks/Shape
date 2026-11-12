//! Drop loop labels no jump needs.
//!
//! The emitter labels every loop it rebuilds (`vm_loop_N: for (;;)`) and
//! names it in each `break`/`continue`. A labeled jump whose nearest
//! enclosing target is that labeled loop anyway (no inner loop between, and
//! for `break` no inner `switch`) means the same unlabeled. A `continue` that
//! ends the loop's own body does nothing and goes. A label left with no jump
//! naming it goes too.
//!
//! Before that, a labeled loop whose body ends in an inner loop reaches its
//! next iteration from that inner loop with `continue label`: once nothing
//! follows the inner loop, that is the inner loop's own `break`. A single
//! `throw`/`return` after an endless inner loop (`for (;;)`) with one
//! `break` of its own moves into that `break`, so nothing follows. After a
//! tested inner loop with no `break` of its own (`while (c)`, `for (; c; u)`)
//! it moves into the failed test at the top of the body:
//! `for (;; u) { if (!(c)) throw …; … }`.

use std::collections::BTreeSet;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

use crate::analysis;
use super::super::support::whole_line;

pub(crate) fn remove_needless_loop_labels(source: &str) -> Result<(String, usize), String> {
    let (source, sunk) = sink_inner_exits(source)?;
    let (code, count) = drop_labels(&source)?;
    Ok((code, sunk + count))
}

fn drop_labels(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let mut scan = Scan::default();
    scan.visit_program(&program);
    let mut edits = Vec::new();
    for (index, (span, _)) in scan.labels.iter().enumerate() {
        if !scan.kept.contains(&index) {
            edits.push(analysis::Edit::delete(*span));
        }
    }
    for span in &scan.dropped {
        edits.push(analysis::Edit::delete(whole_line(source, *span)));
    }
    let count = scan.jumps.len() + edits.len();
    edits.extend(scan.jumps.into_iter().map(|(span, text)| analysis::Edit::replace(span, text)));
    if count == 0 {
        return Ok((source.into(), 0));
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("loop label edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output).map_err(|error| format!("loop label output: {error}"))?;
    Ok((applied.output, count))
}

/// Rewrites a labeled loop's `continue label` from the inner loop that ends
/// its body to that inner loop's `break` (see the module docs).
fn sink_inner_exits(source: &str) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let mut sink = Sink { source, edits: Vec::new() };
    sink.visit_program(&program);
    if sink.edits.is_empty() {
        return Ok((source.into(), 0));
    }
    let count = sink.edits.len();
    let applied = analysis::apply_edits(source, sink.edits);
    // Nested candidates touching the same jumps: leave them all.
    if !applied.rejected.is_empty() {
        return Ok((source.into(), 0));
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output).map_err(|error| format!("inner loop exit output: {error}"))?;
    Ok((applied.output, count))
}

struct Sink<'s> {
    source: &'s str,
    edits: Vec<analysis::Edit>,
}

fn loop_body<'s, 'a>(statement: &'s Statement<'a>) -> Option<(&'s Statement<'a>, bool)> {
    match statement {
        Statement::ForStatement(inner) => Some((&inner.body, inner.test.is_none())),
        Statement::WhileStatement(inner) => {
            Some((&inner.body, matches!(inner.test, Expression::BooleanLiteral(ref literal) if literal.value)))
        }
        _ => None,
    }
}

impl Sink<'_> {
    /// The edits that move `exit` into the failed test of the tested loop
    /// `inner`: the test leaves the header and `if (!(test)) exit` opens the
    /// body. None when the test reads a name the body binds (it would read the
    /// body's binding) or a `for` initializer binds names.
    fn failed_test(&self, inner: &Statement<'_>, exit: &str, bound: &BTreeSet<String>) -> Option<Vec<analysis::Edit>> {
        let (test, body, header) = match inner {
            Statement::WhileStatement(loop_) => (&loop_.test, &loop_.body, None),
            Statement::ForStatement(loop_) => {
                if matches!(loop_.init, Some(ForStatementInit::VariableDeclaration(_))) {
                    return None;
                }
                (loop_.test.as_ref()?, &loop_.body, Some(loop_.test.as_ref()?.span()))
            }
            _ => return None,
        };
        let Statement::BlockStatement(block) = body else {
            return None;
        };
        let mut names = Names::default();
        names.visit_expression(test);
        if names.0.iter().any(|name| bound.contains(name)) {
            return None;
        }
        let test_text = &self.source[test.span().start as usize..test.span().end as usize];
        let mut edits = vec![match header {
            // `while (c) ` → `for (;;) `
            None => analysis::Edit::replace(Span::new(inner.span().start, block.span.start), "for (;;) "),
            // `for (i; c; u)` → `for (i;; u)`
            Some(test) => analysis::Edit::delete(test),
        }];
        // The new first line takes the loop's indentation, one level in.
        let line = self.source[..inner.span().start as usize].rfind('\n').map_or(0, |at| at + 1);
        let indent: String = self.source[line..].chars().take_while(|c| *c == '\t' || *c == ' ').collect();
        edits.push(analysis::Edit::replace(
            Span::new(block.span.start, block.span.start + 1),
            format!("{{\n{indent}\tif (!({test_text})) {exit}"),
        ));
        Some(edits)
    }
}

impl<'a> Visit<'a> for Sink<'_> {
    fn visit_labeled_statement(&mut self, labeled: &LabeledStatement<'a>) {
        walk::walk_labeled_statement(self, labeled);
        let Some((Statement::BlockStatement(body), _)) = loop_body(&labeled.body) else {
            return;
        };
        let (inner, after) = match body.body.as_slice() {
            [.., inner, after @ (Statement::ThrowStatement(_) | Statement::ReturnStatement(_))] => (inner, Some(after)),
            [.., inner] => (inner, None),
            [] => return,
        };
        // A label on the inner loop (one a later step drops) changes nothing:
        // jumps naming it are its own.
        let (inner, own) = match inner {
            Statement::LabeledStatement(inner) => (&inner.body, Some(inner.label.name.as_str())),
            _ => (inner, None),
        };
        let inner_loop = inner;
        let Some((inner_body, endless)) = loop_body(inner) else {
            return;
        };
        let mut jumps = InnerJumps { outer: labeled.label.name.as_str(), own, ..InnerJumps::default() };
        jumps.visit_statement(inner_body);
        if jumps.continues.is_empty() {
            return;
        }
        let mut edits = Vec::new();
        if let Some(after) = after {
            let text = &self.source[after.span().start as usize..after.span().end as usize];
            let mut names = Names::default();
            names.visit_statement(after);
            if text.contains('\n') || names.0.iter().any(|name| jumps.bound.contains(name)) {
                return;
            }
            match (jumps.breaks.as_slice(), endless) {
                ([own], true) => edits.push(analysis::Edit::replace(*own, text.to_string())),
                ([], false) => {
                    let Some(header) = self.failed_test(inner_loop, text, &jumps.bound) else {
                        return;
                    };
                    edits.extend(header);
                }
                _ => return,
            }
            edits.push(analysis::Edit::delete(whole_line(self.source, after.span())));
        }
        edits.extend(jumps.continues.iter().map(|span| analysis::Edit::replace(*span, "break;".to_string())));
        self.edits.extend(edits);
    }
}

/// Inside an inner loop's body: its own `break`s, the `continue outer`s that
/// reach the outer loop from it directly, and the names it binds.
#[derive(Default)]
struct InnerJumps<'n> {
    outer: &'n str,
    /// The inner loop's own label, if it has one.
    own: Option<&'n str>,
    /// Loops and switches nested inside the inner loop.
    nested: usize,
    breaks: Vec<Span>,
    continues: Vec<Span>,
    bound: BTreeSet<String>,
}

impl<'a> Visit<'a> for InnerJumps<'_> {
    fn visit_statement(&mut self, statement: &Statement<'a>) {
        let nests = matches!(
            statement,
            Statement::ForStatement(_)
                | Statement::WhileStatement(_)
                | Statement::DoWhileStatement(_)
                | Statement::ForInStatement(_)
                | Statement::ForOfStatement(_)
                | Statement::SwitchStatement(_)
        );
        self.nested += usize::from(nests);
        walk::walk_statement(self, statement);
        self.nested -= usize::from(nests);
    }
    fn visit_break_statement(&mut self, statement: &BreakStatement<'a>) {
        let own = match &statement.label {
            None => self.nested == 0,
            Some(label) => self.own == Some(label.name.as_str()),
        };
        if own {
            self.breaks.push(statement.span);
        }
    }
    fn visit_continue_statement(&mut self, statement: &ContinueStatement<'a>) {
        if self.nested == 0 && statement.label.as_ref().is_some_and(|label| label.name == self.outer) {
            self.continues.push(statement.span);
        }
    }
    fn visit_binding_identifier(&mut self, binding: &BindingIdentifier<'a>) {
        self.bound.insert(binding.name.to_string());
    }
    // Jumps never cross a function; its names are its own.
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
    fn visit_class(&mut self, _: &Class<'a>) {}
}

#[derive(Default)]
struct Names(Vec<String>);

impl<'a> Visit<'a> for Names {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.0.push(identifier.name.to_string());
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Loop,
    Switch,
    /// A function boundary: no jump crosses it.
    Function,
}

#[derive(Default)]
struct Scan {
    /// Enclosing jump targets, innermost last, with the index (in
    /// `labels`) of a labeled loop's label.
    targets: Vec<(Target, Option<usize>)>,
    /// The label a labeled statement hands to the loop it labels.
    pending: Option<usize>,
    /// `label: ` prefixes, by (prefix span, label).
    labels: Vec<(Span, String)>,
    /// Labels some jump still needs.
    kept: BTreeSet<usize>,
    /// Jump rewrites: (statement span, replacement).
    jumps: Vec<(Span, String)>,
    /// Trailing `continue`s that go.
    dropped: Vec<Span>,
}

impl Scan {
    fn enter(&mut self, target: Target) {
        let label = self.pending.take();
        self.targets.push((target, label));
    }

    fn jump(&mut self, span: Span, label: Option<&LabelIdentifier<'_>>, keyword: &str, continues: bool) {
        let Some(label) = label else {
            return;
        };
        if self.dropped.contains(&span) {
            return;
        }
        let name = label.name.as_str();
        let named = |index: &Option<usize>| index.is_some_and(|index| self.labels[index].1 == name);
        let nearest = self
            .targets
            .iter()
            .rev()
            .find(|(target, _)| *target != Target::Switch || !continues);
        match nearest {
            Some((Target::Loop, own)) if named(own) => self.jumps.push((span, format!("{keyword};"))),
            _ => {
                let reached = self.targets.iter().rev().take_while(|(target, _)| *target != Target::Function);
                // A label on something other than a loop is left alone.
                if let Some(index) = reached.filter_map(|(_, index)| *index).find(|index| self.labels[*index].1 == name) {
                    self.kept.insert(index);
                }
            }
        }
    }

    /// A `continue` naming (or reaching) the loop that ends its own body.
    fn trailing_continue(&mut self, body: &Statement<'_>) {
        let Statement::BlockStatement(block) = body else {
            return;
        };
        let Some(Statement::ContinueStatement(jump)) = block.body.last() else {
            return;
        };
        let own = self.targets.last().and_then(|(_, label)| *label).map(|index| self.labels[index].1.as_str());
        if jump.label.as_ref().is_none_or(|label| Some(label.name.as_str()) == own) {
            self.dropped.push(jump.span);
        }
    }

    fn body(&mut self, body: &Statement<'_>) {
        self.trailing_continue(body);
        self.visit_statement(body);
    }
}

impl<'a> Visit<'a> for Scan {
    fn visit_labeled_statement(&mut self, labeled: &LabeledStatement<'a>) {
        let is_loop = matches!(
            labeled.body,
            Statement::ForStatement(_)
                | Statement::WhileStatement(_)
                | Statement::DoWhileStatement(_)
                | Statement::ForInStatement(_)
                | Statement::ForOfStatement(_)
        );
        if !is_loop {
            walk::walk_labeled_statement(self, labeled);
            return;
        }
        self.labels.push((Span::new(labeled.span.start, labeled.body.span().start), labeled.label.name.to_string()));
        self.pending = Some(self.labels.len() - 1);
        self.visit_statement(&labeled.body);
    }
    fn visit_for_statement(&mut self, statement: &ForStatement<'a>) {
        let label = self.pending.take();
        if let Some(init) = &statement.init {
            self.visit_for_statement_init(init);
        }
        if let Some(test) = &statement.test {
            self.visit_expression(test);
        }
        if let Some(update) = &statement.update {
            self.visit_expression(update);
        }
        self.pending = label;
        self.enter(Target::Loop);
        self.body(&statement.body);
        self.targets.pop();
    }
    fn visit_while_statement(&mut self, statement: &WhileStatement<'a>) {
        let label = self.pending.take();
        self.visit_expression(&statement.test);
        self.pending = label;
        self.enter(Target::Loop);
        self.body(&statement.body);
        self.targets.pop();
    }
    fn visit_do_while_statement(&mut self, statement: &DoWhileStatement<'a>) {
        // A trailing `continue` here still runs the test: it stays.
        self.enter(Target::Loop);
        self.visit_statement(&statement.body);
        self.targets.pop();
        self.visit_expression(&statement.test);
    }
    fn visit_for_in_statement(&mut self, statement: &ForInStatement<'a>) {
        let label = self.pending.take();
        self.visit_for_statement_left(&statement.left);
        self.visit_expression(&statement.right);
        self.pending = label;
        self.enter(Target::Loop);
        self.body(&statement.body);
        self.targets.pop();
    }
    fn visit_for_of_statement(&mut self, statement: &ForOfStatement<'a>) {
        let label = self.pending.take();
        self.visit_for_statement_left(&statement.left);
        self.visit_expression(&statement.right);
        self.pending = label;
        self.enter(Target::Loop);
        self.body(&statement.body);
        self.targets.pop();
    }
    fn visit_switch_statement(&mut self, statement: &SwitchStatement<'a>) {
        self.visit_expression(&statement.discriminant);
        self.enter(Target::Switch);
        for case in &statement.cases {
            self.visit_switch_case(case);
        }
        self.targets.pop();
    }
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        self.enter(Target::Function);
        walk::walk_function(self, function, flags);
        self.targets.pop();
    }
    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.enter(Target::Function);
        walk::walk_arrow_function_expression(self, arrow);
        self.targets.pop();
    }
    fn visit_class(&mut self, class: &Class<'a>) {
        self.enter(Target::Function);
        walk::walk_class(self, class);
        self.targets.pop();
    }
    fn visit_break_statement(&mut self, statement: &BreakStatement<'a>) {
        self.jump(statement.span, statement.label.as_ref(), "break", false);
    }
    fn visit_continue_statement(&mut self, statement: &ContinueStatement<'a>) {
        self.jump(statement.span, statement.label.as_ref(), "continue", true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_labels_whose_jumps_reach_their_own_loop() {
        let source = "function f(a) {\n\tvm_loop_1: for (;;) {\n\t\tif (a()) {\n\t\t\tcontinue vm_loop_1;\n\t\t}\n\t\tbreak vm_loop_1;\n\t}\n\tvm_loop_2: while (a()) {\n\t\ta();\n\t\tcontinue vm_loop_2;\n\t}\n}\n";
        let (code, _) = remove_needless_loop_labels(source).unwrap();
        assert_eq!(
            code,
            "function f(a) {\n\tfor (;;) {\n\t\tif (a()) {\n\t\t\tcontinue;\n\t\t}\n\t\tbreak;\n\t}\n\twhile (a()) {\n\t\ta();\n\t}\n}\n"
        );
    }

    #[test]
    fn moves_the_exit_after_an_inner_loop_into_its_break() {
        let source = "function f(a) {\n\tvm_loop_8: while (a()) {\n\t\tfor (;;) {\n\t\t\tif (a()) break;\n\t\t\tif (a()) {\n\t\t\t\tcontinue vm_loop_8;\n\t\t\t}\n\t\t}\n\t\tthrow new Error(\"x\");\n\t}\n}\n";
        let (code, _) = remove_needless_loop_labels(source).unwrap();
        assert_eq!(
            code,
            "function f(a) {\n\twhile (a()) {\n\t\tfor (;;) {\n\t\t\tif (a()) throw new Error(\"x\");\n\t\t\tif (a()) {\n\t\t\t\tbreak;\n\t\t\t}\n\t\t}\n\t}\n}\n"
        );
    }

    #[test]
    fn keeps_labels_an_inner_loop_or_switch_jumps_past() {
        let source = "function f(a) {\n\tvm_loop_1: for (;;) {\n\t\tfor (;;) {\n\t\t\tcontinue vm_loop_1;\n\t\t}\n\t\ta();\n\t}\n\tvm_loop_2: for (;;) {\n\t\tswitch (a) {\n\t\t\tcase 1: break vm_loop_2;\n\t\t}\n\t\tcontinue vm_loop_2;\n\t}\n}\n";
        let (code, _) = remove_needless_loop_labels(source).unwrap();
        assert!(code.contains("vm_loop_1: for"), "{code}");
        assert!(code.contains("continue vm_loop_1;"), "{code}");
        assert!(code.contains("vm_loop_2: for"), "{code}");
        assert!(code.contains("break vm_loop_2;"), "{code}");
    }

    #[test]
    fn sinks_continues_through_a_labeled_inner_loop() {
        let source = "function f(a) {\n\tvm_loop_8: while (a()) {\n\t\tvm_loop_9: for (;;) {\n\t\t\tif (a()) {\n\t\t\t\tcontinue vm_loop_8;\n\t\t\t}\n\t\t\ta();\n\t\t\tcontinue vm_loop_9;\n\t\t}\n\t}\n}\n";
        let (code, _) = remove_needless_loop_labels(source).unwrap();
        assert_eq!(
            code,
            "function f(a) {\n\twhile (a()) {\n\t\tfor (;;) {\n\t\t\tif (a()) {\n\t\t\t\tbreak;\n\t\t\t}\n\t\t\ta();\n\t\t}\n\t}\n}\n"
        );
    }

    #[test]
    fn moves_the_exit_into_a_labeled_inner_loops_break() {
        let source = "function f(a) {\n\tvm_loop_8: while (a()) {\n\t\tvm_loop_9: for (;;) {\n\t\t\tif (a()) {\n\t\t\t\tbreak vm_loop_9;\n\t\t\t}\n\t\t\tif (a()) {\n\t\t\t\tcontinue vm_loop_8;\n\t\t\t}\n\t\t}\n\t\tthrow new Error(\"x\");\n\t}\n}\n";
        let (code, _) = remove_needless_loop_labels(source).unwrap();
        assert!(!code.contains("vm_loop_"), "{code}");
        assert!(code.contains("if (a()) {\n\t\t\t\tthrow new Error(\"x\");"), "{code}");
    }

    #[test]
    fn moves_the_exit_after_a_tested_inner_loop_into_its_failed_test() {
        let source = "function f(a, g) {\n\tvm_loop_4: for (;;) {\n\t\tif (a()) break;\n\t\twhile (g !== void 0) {\n\t\t\tif (a()) {\n\t\t\t\tcontinue;\n\t\t\t} else {\n\t\t\t\tcontinue vm_loop_4;\n\t\t\t}\n\t\t}\n\t\tthrow new TypeError(\"x\");\n\t}\n}\n";
        let (code, _) = remove_needless_loop_labels(source).unwrap();
        assert!(!code.contains("vm_loop_"), "{code}");
        assert!(!code.contains("while"), "{code}");
        assert!(code.contains("for (;;) {\n\t\t\tif (!(g !== void 0)) throw new TypeError(\"x\");"), "{code}");
        assert!(code.contains("} else {\n\t\t\t\tbreak;"), "{code}");
    }

    #[test]
    fn keeps_a_tested_inner_loop_whose_body_binds_a_test_name() {
        let source = "function f(a, g) {\n\tvm_loop_4: for (;;) {\n\t\tif (a()) break;\n\t\twhile (g) {\n\t\t\tlet g = a();\n\t\t\tif (g) {\n\t\t\t\tcontinue;\n\t\t\t}\n\t\t\tcontinue vm_loop_4;\n\t\t}\n\t\tthrow new TypeError(\"x\");\n\t}\n}\n";
        let (code, _) = remove_needless_loop_labels(source).unwrap();
        assert!(code.contains("vm_loop_4: for"), "{code}");
        assert!(code.contains("while (g)"), "{code}");
    }
}
