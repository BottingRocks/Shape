//! Prefix work, shared exit paths, and process-before-test loops.

use std::collections::BTreeMap;

use super::*;

#[derive(Clone)]
struct Header<'s, 't, 'a> {
    scan: &'s Scan<'t, 'a>,
    region: Span,
    bindings: BTreeMap<String, SymbolId>,
    declarations: Vec<String>,
    declaration_edits: Vec<(Span, String)>,
}

impl Header<'_, '_, '_> {
    fn expressions(&mut self, statements: &[Statement<'_>]) -> Option<String> {
        let expressions = statements
            .iter()
            .map(|s| self.expression(s))
            .collect::<Option<Vec<_>>>()?;
        Some(if expressions.is_empty() {
            "void 0".into()
        } else {
            expressions.join(", ")
        })
    }

    fn expression(&mut self, statement: &Statement<'_>) -> Option<String> {
        match statement {
            Statement::ExpressionStatement(expr) => {
                Some(format!("({})", self.scan.text(expr.expression.span())))
            }
            Statement::EmptyStatement(_) => Some("void 0".into()),
            Statement::BlockStatement(body) => {
                self.expressions(&body.body).map(|s| format!("({s})"))
            }
            Statement::IfStatement(branch) => {
                let then = self.expression(&branch.consequent)?;
                let otherwise = branch
                    .alternate
                    .as_ref()
                    .map(|s| self.expression(s))
                    .unwrap_or_else(|| Some("void 0".into()))?;
                Some(format!(
                    "(({}) ? ({then}) : ({otherwise}))",
                    self.scan.text(branch.test.span())
                ))
            }
            Statement::VariableDeclaration(declaration) => {
                let mut expressions = Vec::new();
                for binding in &declaration.declarations {
                    let BindingPattern::BindingIdentifier(id) = &binding.id else {
                        return None;
                    };
                    let symbol = id.symbol_id.get()?;
                    if !(id.name.starts_with("$SSA_") || id.name.starts_with("$CARRY_"))
                        || !matches!(
                            declaration.kind,
                            VariableDeclarationKind::Let | VariableDeclarationKind::Const
                        )
                        || self.scan.captured(symbol)
                        || self.scan.protected.contains(id.name.as_str())
                    {
                        return None;
                    }
                    let end = binding
                        .init
                        .as_ref()
                        .map_or(binding.span.end, |init| init.span().end);
                    if self
                        .scan
                        .scoping
                        .get_resolved_references(symbol)
                        .any(|reference| {
                            self.scan
                                .nodes
                                .get_node(reference.node_id())
                                .kind()
                                .span()
                                .start
                                < end
                                || (declaration.kind == VariableDeclarationKind::Const
                                    && reference.is_write())
                        })
                    {
                        return None;
                    }
                    if self.bindings.insert(id.name.to_string(), symbol).is_some() {
                        return None;
                    }
                    self.declarations.push(format!("let {};", id.name));
                    let init = binding
                        .init
                        .as_ref()
                        .map_or("void 0", |init| self.scan.text(init.span()));
                    expressions.push(format!("({} = ({init}))", id.name));
                }
                let expression = expressions.join(", ");
                self.declaration_edits
                    .push((declaration.span, format!("{expression};")));
                Some(expression)
            }
            _ => None,
        }
    }

    fn valid(&self) -> bool {
        for (name, &symbol) in &self.bindings {
            // Lifting a branch-local declaration must not intercept an outer
            // or unresolved reference, nor merge distinct lexical cells.
            for other in self.scan.scoping.symbol_ids() {
                if other != symbol
                    && self.scan.scoping.symbol_name(other) == name
                    && (inside(self.region, self.scan.scoping.symbol_span(other))
                        || self
                            .scan
                            .scoping
                            .get_resolved_references(other)
                            .any(|reference| {
                                inside(
                                    self.region,
                                    self.scan.nodes.get_node(reference.node_id()).kind().span(),
                                )
                            }))
                {
                    return false;
                }
            }
            if let Some(references) = self
                .scan
                .scoping
                .root_unresolved_references()
                .get(name.as_str())
            {
                if references.iter().any(|id| {
                    let reference = self.scan.scoping.get_reference(*id);
                    inside(
                        self.region,
                        self.scan.nodes.get_node(reference.node_id()).kind().span(),
                    )
                }) {
                    return false;
                }
            }
        }
        true
    }

    fn body_code(&self, statements: &[Statement<'_>]) -> String {
        statements
            .iter()
            .map(|statement| {
                let span = statement.span();
                let mut text = self.scan.text(span).to_string();
                for (edit, replacement) in self
                    .declaration_edits
                    .iter()
                    .rev()
                    .filter(|(edit, _)| inside(span, *edit))
                {
                    text.replace_range(
                        (edit.start - span.start) as usize..(edit.end - span.start) as usize,
                        replacement,
                    );
                }
                text
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Folds a straight-line body into the `do … while` test, adopting the
    /// trial bindings only when they stay valid.
    fn fold_updates(&mut self, run: &[Statement<'_>]) -> Option<String> {
        let mut trial = self.clone();
        let updates = trial.expressions(run)?;
        if !trial.valid() {
            return None;
        }
        *self = trial;
        Some(updates)
    }
}

/// A branch in a `vm_loop_*` body whose one arm continues the loop, with the
/// arm that leaves it and the statements after the branch.
struct ExitBranch<'s, 'a> {
    branch: &'s IfStatement<'a>,
    /// Whether the else arm is the one that continues.
    negate: bool,
    /// Whether the continuing arm always ends the iteration.
    run_full: bool,
    /// The continuing arm, without its trailing `continue`.
    run: &'s [Statement<'a>],
    jumps: Jumps<'s>,
    /// The leaving arm, without its trailing `break`.
    exit: &'s [Statement<'a>],
    /// The statements after the branch, without a trailing `break`.
    suffix: &'s [Statement<'a>],
}

impl<'s, 'a> ExitBranch<'s, 'a> {
    fn new(body: &'s [Statement<'a>], at: usize, name: &'s str) -> Option<Self> {
        let Statement::IfStatement(branch) = &body[at] else {
            return None;
        };
        let then = block(&branch.consequent)?;
        let otherwise = match &branch.alternate {
            Some(other) => block(other)?,
            None => &[],
        };
        let mut then_jumps = Jumps::new(name);
        then_jumps.statements(then);
        let mut other_jumps = Jumps::new(name);
        other_jumps.statements(otherwise);
        let (run, exit, negate) = if then_jumps.continues > 0 && other_jumps.continues == 0 {
            (then, otherwise, false)
        } else if other_jumps.continues > 0 && then_jumps.continues == 0 {
            (otherwise, then, true)
        } else {
            return None;
        };
        let run_full = run.last().is_some_and(|s| ends_iteration(s, name));
        let run = if run.last().is_some_and(|s| continues(s, name)) {
            &run[..run.len() - 1]
        } else {
            run
        };
        let mut jumps = Jumps::new(name);
        jumps.statements(run);
        if jumps.breaks != 0 {
            return None;
        }
        let suffix = &body[at + 1..];
        let trailing_break = suffix.last().is_some_and(|s| breaks(s, name));
        let suffix = if trailing_break {
            &suffix[..suffix.len() - 1]
        } else {
            suffix
        };
        let exit_break = exit.last().is_some_and(|s| breaks(s, name));
        if exit_break && !suffix.is_empty() {
            return None;
        }
        let exit = if exit_break {
            &exit[..exit.len() - 1]
        } else {
            exit
        };
        if !trailing_break
            && !exit_break
            && !suffix
                .last()
                .or_else(|| exit.last())
                .is_some_and(cannot_fall_through)
        {
            return None;
        }
        let mut exit_jumps = Jumps::new(name);
        exit_jumps.statements(exit);
        exit_jumps.statements(suffix);
        if exit_jumps.breaks != 0 || exit_jumps.continues != 0 {
            return None;
        }
        Some(Self {
            branch,
            negate,
            run_full,
            run,
            jumps,
            exit,
            suffix,
        })
    }
}

impl Scan<'_, '_> {
    pub(crate) fn extended_statements(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            self.extended_loop(statement);
        }
    }

    /// Rewrites a `vm_loop_*` around the first body branch that structures.
    fn extended_loop(&mut self, statement: &Statement<'_>) -> Option<()> {
        let Statement::LabeledStatement(labeled) = statement else {
            return None;
        };
        if !labeled.label.name.starts_with("vm_loop_") {
            return None;
        }
        let Statement::ForStatement(loop_) = &labeled.body else {
            return None;
        };
        if loop_.init.is_some() || loop_.test.is_some() || loop_.update.is_some() {
            return None;
        }
        let body = block(&loop_.body)?;
        let region = labeled.span;
        if self
            .edits
            .iter()
            .any(|edit| edit.start < region.end && region.start < edit.end)
        {
            return None;
        }
        let name = labeled.label.name.as_str();
        for at in 0..body.len() {
            if let Some(replacement) = self.extended_replacement(region, name, body, at) {
                self.edits
                    .push(analysis::Edit::replace(region, replacement));
                break;
            }
        }
        Some(())
    }

    /// The structured replacement for the loop at `region` when the branch at
    /// `body[at]` splits it: prefix work, the loop, then the shared exit.
    fn extended_replacement(
        &self,
        region: Span,
        name: &str,
        body: &[Statement<'_>],
        at: usize,
    ) -> Option<String> {
        let ExitBranch {
            branch,
            negate,
            run_full,
            run,
            jumps,
            exit,
            suffix,
        } = ExitBranch::new(body, at, name)?;
        let mut run = run;
        let mut header = Header {
            scan: self,
            region,
            bindings: BTreeMap::new(),
            declarations: Vec::new(),
            declaration_edits: Vec::new(),
        };
        let prefix = header.expressions(&body[..at])?;
        let test = self.text(branch.test.span());
        let test = if negate {
            format!("!({test})")
        } else {
            format!("({test})")
        };
        let join = |statements: &[Statement<'_>]| {
            statements
                .iter()
                .map(|s| self.text(s.span()))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let mut continuation = String::new();
        let mut condition = test.clone();
        if !run_full {
            // A successful search falls through to the shared suffix;
            // exhaustion runs the original else writes exactly once.
            if !exit.is_empty() {
                let defaults = header.expressions(exit)?;
                condition = format!("({test}) || ({defaults}, false)");
            }
        } else if !exit.is_empty() {
            continuation.push_str(&format!("{{ {} }}", join(exit)));
        }
        if !suffix.is_empty() {
            continuation.push_str(&join(suffix));
        }
        let mut update = None;
        if run_full && jumps.continues == 0 {
            if let Some((rest, span, _)) = self.trailing_update(run, branch.test.span()) {
                update = Some(span);
                run = rest;
            }
        }
        let can_do = run_full
            && jumps.continues == 0
            && update.is_none()
            && body[..at].iter().any(|s| {
                matches!(
                    s,
                    Statement::ExpressionStatement(_) | Statement::IfStatement(_)
                )
            })
            && run.iter().all(|s| {
                matches!(
                    s,
                    Statement::ExpressionStatement(_) | Statement::VariableDeclaration(_)
                )
            });
        let do_updates = if can_do {
            header.fold_updates(run)
        } else {
            None
        };
        if !header.valid() {
            return None;
        }
        let declarations = header.declarations.join("\n");
        let label = if jumps.labeled_continue {
            format!("{name}: ")
        } else {
            String::new()
        };
        let run_text = self.run_text(run, &jumps);
        let loop_text = if let Some(updates) = do_updates {
            let condition = if run.is_empty() {
                test
            } else {
                format!("({test}) && ({updates}, true)")
            };
            format!(
                "do {{ {} }} while ({condition});",
                header.body_code(&body[..at])
            )
        } else {
            let condition = if at == 0 {
                condition
            } else {
                format!("{prefix}, {condition}")
            };
            let kind = if let Some(update) = update {
                format!("for (; {condition}; {})", self.text(update))
            } else {
                format!("while ({condition})")
            };
            let end = if run_full { "" } else { "break;" };
            format!("{label}{kind} {{ {run_text} {end} }}")
        };
        Some(format!("{{ {declarations} {loop_text} {continuation} }}"))
    }
}
