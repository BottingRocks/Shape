//! Loops whose test or update holds a helper call, rewritten so the test or
//! update is a statement of its own and the call expands there.

use super::hoist::NestedCalls;
use super::*;
use crate::transforms::emitted_js::support::indent_at;

/// What a loop body does that the rewrite must respect.
struct LoopBody<'l> {
    /// Labels on the loop itself.
    labels: &'l [&'l str],
    /// Loops nested inside the body (an unlabelled `continue` there is theirs).
    depth: usize,
    /// `continue` statements that go to this loop's next iteration.
    continues: Vec<Span>,
    /// A function or class inside: it may capture a per-iteration binding.
    closures: bool,
}

impl<'a> Visit<'a> for LoopBody<'_> {
    fn visit_continue_statement(&mut self, statement: &ContinueStatement<'a>) {
        let ours = match &statement.label {
            None => self.depth == 0,
            Some(label) => self.labels.contains(&label.name.as_str()),
        };
        if ours {
            self.continues.push(statement.span);
        }
    }
    fn visit_for_statement(&mut self, statement: &ForStatement<'a>) {
        self.depth += 1;
        walk::walk_for_statement(self, statement);
        self.depth -= 1;
    }
    fn visit_for_in_statement(&mut self, statement: &ForInStatement<'a>) {
        self.depth += 1;
        walk::walk_for_in_statement(self, statement);
        self.depth -= 1;
    }
    fn visit_for_of_statement(&mut self, statement: &ForOfStatement<'a>) {
        self.depth += 1;
        walk::walk_for_of_statement(self, statement);
        self.depth -= 1;
    }
    fn visit_while_statement(&mut self, statement: &WhileStatement<'a>) {
        self.depth += 1;
        walk::walk_while_statement(self, statement);
        self.depth -= 1;
    }
    fn visit_do_while_statement(&mut self, statement: &DoWhileStatement<'a>) {
        self.depth += 1;
        walk::walk_do_while_statement(self, statement);
        self.depth -= 1;
    }
    // `continue` never crosses a function.
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {
        self.closures = true;
    }
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {
        self.closures = true;
    }
    fn visit_class(&mut self, _: &Class<'a>) {
        self.closures = true;
    }
}

/// A block body whose own statements declare nothing block-scoped: its
/// statements may share a block with the test or update text.
fn flat_block<'b, 'a>(body: &'b Statement<'a>) -> Option<&'b BlockStatement<'a>> {
    let Statement::BlockStatement(block) = body else {
        return None;
    };
    let lexical = block.body.iter().any(|statement| match statement {
        Statement::VariableDeclaration(declaration) => declaration.kind != VariableDeclarationKind::Var,
        Statement::FunctionDeclaration(_) | Statement::ClassDeclaration(_) => true,
        _ => false,
    });
    (!lexical).then_some(block)
}

/// Text that stands as a statement of its own.
fn as_statement(text: &str) -> String {
    let start = text.trim_start();
    if ["{", "function", "class", "let", "async"].iter().any(|word| start.starts_with(word)) {
        format!("({text});")
    } else {
        format!("{text};")
    }
}

impl Hoists<'_, '_> {
    /// Helper calls (and the expressions holding them) found in `expression`,
    /// and whether it holds a function.
    fn header_calls(&self, expression: Option<&Expression<'_>>) -> (bool, bool) {
        let Some(expression) = expression else {
            return (false, false);
        };
        let mut nested = NestedCalls::new(self.scoping, self.helpers, self.declined);
        nested.visit_expression(expression);
        (!nested.calls.is_empty(), !nested.functions.is_empty())
    }

    fn body(&self, labels: &[&str], body: &Statement<'_>) -> (Vec<Span>, bool) {
        let mut visitor = LoopBody {
            labels,
            depth: 0,
            continues: Vec::new(),
            closures: false,
        };
        visitor.visit_statement(body);
        (visitor.continues, visitor.closures)
    }

    fn fresh_label(&mut self) -> String {
        let mut index = 1;
        loop {
            let name = if index == 1 { "iteration".to_string() } else { format!("iteration_{index}") };
            index += 1;
            if !self.source.contains(&name) && !self.labels.contains(&name) {
                self.labels.push(name.clone());
                return name;
            }
        }
    }

    /// `continue`s to this loop go to `label`'s end instead.
    fn continues_to(&mut self, continues: &[Span], label: &str) {
        for span in continues {
            let text = &self.source[span.start as usize..span.end as usize];
            let semicolon = if text.trim_end().ends_with(';') { ";" } else { "" };
            self.edits.push(analysis::Edit::replace(*span, format!("break {label}{semicolon}")));
        }
    }

    /// A loop whose test or update holds a helper call becomes one whose
    /// test or update is a statement of its own, run at the same point:
    ///
    /// - `while (T) B` → `for (;;) { if (!(T)) break; B }`;
    /// - `do B while (T)` → `for (;;) { B if (!(T)) break; }`, each
    ///   `continue` to it first leaving `B` (`L: { B }`, `break L`) so it
    ///   still reaches the test;
    /// - `for (I; T; U) B` → `for (I; ; ) { if (!(T)) break; B U; }` for a
    ///   moved test or update, a `continue` reaching the update the same way.
    ///
    /// `B` keeps its own block when it declares anything block-scoped, so
    /// the test and update still see the names they saw. A `for` whose
    /// `let` a closure may capture keeps its update (it would run before
    /// the per-iteration copy). Returns whether it rewrote.
    pub(super) fn restructure_loop(&mut self, statement: &Statement<'_>) -> bool {
        let mut labels = Vec::new();
        let mut current = statement;
        while let Statement::LabeledStatement(labeled) = current {
            labels.push(labeled.label.name.as_str());
            current = &labeled.body;
        }
        let source = self.source;
        let text = |span: Span| &source[span.start as usize..span.end as usize];
        let indent = indent_at(source, statement.span());
        let inner = format!("{indent}\t");
        // Text placed before the closing brace of a block body.
        let before_close = |block: &BlockStatement<'_>, lines: &str| {
            let close = block.span.end - 1;
            let at_line_start = source[..close as usize].trim_end_matches([' ', '\t']).ends_with('\n');
            if at_line_start {
                format!("\t{lines}\n{indent}}}")
            } else {
                format!("\n{inner}{lines}\n{indent}}}")
            }
        };
        match current {
            Statement::WhileStatement(lp) => {
                let (calls, functions) = self.header_calls(Some(&lp.test));
                if !calls || functions {
                    return false;
                }
                let test = format!("if (!({})) break;", text(lp.test.span()));
                match flat_block(&lp.body) {
                    Some(block) => {
                        self.edits.push(analysis::Edit::replace(
                            Span::new(lp.span.start, block.span.start + 1),
                            format!("for (;;) {{\n{inner}{test}"),
                        ));
                    }
                    None => {
                        let body = lp.body.span();
                        self.edits.push(analysis::Edit::replace(
                            Span::new(lp.span.start, body.start),
                            format!("for (;;) {{\n{inner}{test}\n{inner}"),
                        ));
                        self.edits.push(analysis::Edit::replace(Span::new(body.end, lp.span.end), format!("\n{indent}}}")));
                    }
                }
            }
            Statement::DoWhileStatement(lp) => {
                let (calls, functions) = self.header_calls(Some(&lp.test));
                if !calls || functions {
                    return false;
                }
                let test = format!("if (!({})) break;", text(lp.test.span()));
                let (continues, _) = self.body(&labels, &lp.body);
                let body = lp.body.span();
                match flat_block(&lp.body).filter(|_| continues.is_empty()) {
                    Some(block) => {
                        self.edits.push(analysis::Edit::replace(Span::new(lp.span.start, block.span.start + 1), "for (;;) {".to_string()));
                        self.edits.push(analysis::Edit::replace(Span::new(block.span.end - 1, lp.span.end), before_close(block, &test)));
                    }
                    None => {
                        let label = if continues.is_empty() {
                            String::new()
                        } else {
                            let label = self.fresh_label();
                            self.continues_to(&continues, &label);
                            format!("{label}: ")
                        };
                        self.edits.push(analysis::Edit::replace(
                            Span::new(lp.span.start, body.start),
                            format!("for (;;) {{\n{inner}{label}"),
                        ));
                        self.edits.push(analysis::Edit::replace(
                            Span::new(body.end, lp.span.end),
                            format!("\n{inner}{test}\n{indent}}}"),
                        ));
                    }
                }
            }
            Statement::ForStatement(lp) => {
                let (test_calls, test_functions) = self.header_calls(lp.test.as_ref());
                let (update_calls, update_functions) = self.header_calls(lp.update.as_ref());
                if !(test_calls || update_calls) || test_functions || update_functions {
                    return false;
                }
                let init_functions = lp.init.as_ref().is_some_and(|init| {
                    let mut nested = NestedCalls::new(self.scoping, self.helpers, self.declined);
                    match init {
                        ForStatementInit::VariableDeclaration(declaration) => nested.visit_variable_declaration(declaration),
                        init => {
                            if let Some(expression) = init.as_expression() {
                                nested.visit_expression(expression);
                            }
                        }
                    }
                    !nested.functions.is_empty()
                });
                if init_functions {
                    return false;
                }
                let (continues, closures) = self.body(&labels, &lp.body);
                let per_iteration = matches!(&lp.init, Some(ForStatementInit::VariableDeclaration(declaration))
                    if declaration.kind != VariableDeclarationKind::Var);
                if update_calls && per_iteration && closures {
                    return false;
                }
                let init = lp.init.as_ref().map_or("", |init| text(init.span()));
                let test = match (&lp.test, test_calls) {
                    (Some(test), false) => text(test.span()),
                    _ => "",
                };
                let update = match (&lp.update, update_calls) {
                    (Some(update), false) => text(update.span()),
                    _ => "",
                };
                let header = format!("for ({init}; {test}; {update}) {{").replace(";  ", "; ").replace("; )", ";)");
                let lead = match (&lp.test, test_calls) {
                    (Some(test), true) => format!("\n{inner}if (!({})) break;", text(test.span())),
                    _ => String::new(),
                };
                let tail = match (&lp.update, update_calls) {
                    (Some(update), true) => Some(as_statement(text(update.span()))),
                    _ => None,
                };
                let needs_label = tail.is_some() && !continues.is_empty();
                let body = lp.body.span();
                match flat_block(&lp.body).filter(|_| !needs_label) {
                    Some(block) => {
                        self.edits.push(analysis::Edit::replace(
                            Span::new(lp.span.start, block.span.start + 1),
                            format!("{header}{lead}"),
                        ));
                        if let Some(tail) = &tail {
                            self.edits.push(analysis::Edit::replace(Span::new(block.span.end - 1, block.span.end), before_close(block, tail)));
                        }
                    }
                    None => {
                        let label = if needs_label {
                            let label = self.fresh_label();
                            self.continues_to(&continues, &label);
                            format!("{label}: ")
                        } else {
                            String::new()
                        };
                        self.edits.push(analysis::Edit::replace(
                            Span::new(lp.span.start, body.start),
                            format!("{header}{lead}\n{inner}{label}"),
                        ));
                        let tail = tail.map_or(String::new(), |tail| format!("\n{inner}{tail}"));
                        self.edits.push(analysis::Edit::replace(Span::new(body.end, lp.span.end), format!("{tail}\n{indent}}}")));
                    }
                }
            }
            _ => return false,
        }
        self.done += 1;
        true
    }
}
