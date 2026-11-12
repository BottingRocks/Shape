//! The walk over statements: which stores definitely run, loops and their parts.

use super::*;

impl Walk<'_, '_, '_> {
    pub(super) fn reads_in(&self, span: Span) -> bool {
        let at = self.reads.partition_point(|start| *start < span.start);
        self.reads.get(at).is_some_and(|start| *start < span.end)
    }

    pub(super) fn is_local(&self, target: &SimpleAssignmentTarget<'_>) -> bool {
        let SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) = target else {
            return false;
        };
        self.merger.scoping.get_reference(identifier.reference_id()).symbol_id() == Some(self.local)
    }

    /// Whether the old value can still be read after `statement` runs.
    pub(super) fn live_after(&self, statement: NodeId) -> bool {
        let nodes = self.merger.nodes;
        let mut current = statement;
        let mut falls = true;
        let mut pending: Vec<Jump> = Vec::new();
        loop {
            let parent = nodes.parent_id(current);
            let current_span = nodes.kind(current).span();
            match nodes.kind(parent) {
                AstKind::BlockStatement(block) => {
                    if falls {
                        let flow = self.rest(&block.body, current_span);
                        if flow.read {
                            return true;
                        }
                        falls = flow.falls;
                        pending.extend(flow.jumps);
                    }
                }
                AstKind::FunctionBody(body) => {
                    return falls && self.rest(&body.statements, current_span).read;
                }
                AstKind::Program(program) => {
                    return falls && self.rest(&program.body, current_span).read;
                }
                AstKind::IfStatement(_) => {}
                // Leaving a `try` block, normally or by a throw from any
                // point after the store, continues after the statement or in
                // the handler; a finalizer declines.
                AstKind::TryStatement(statement) => {
                    if statement.finalizer.is_some() {
                        return true;
                    }
                    if statement.block.span == current_span {
                        if let Some(handler) = &statement.handler {
                            let flow = self.sequence(&handler.body.body);
                            if flow.read {
                                return true;
                            }
                            falls |= flow.falls;
                            pending.extend(flow.jumps);
                        }
                    }
                }
                AstKind::CatchClause(_) => {}
                AstKind::LabeledStatement(labeled) => {
                    let label = Some(labeled.label.name.to_string());
                    if pending.contains(&Jump::Break(label.clone())) {
                        falls = true;
                    }
                    pending.retain(|jump| *jump != Jump::Break(label.clone()));
                }
                kind if is_loop(&kind) => {
                    let label = match nodes.parent_kind(parent) {
                        AstKind::LabeledStatement(labeled) => Some(labeled.label.name.to_string()),
                        _ => None,
                    };
                    let targets = |jump: &Jump| match jump {
                        Jump::Break(name) | Jump::Continue(name) => name.is_none() || *name == label,
                    };
                    let back = falls
                        || pending
                            .iter()
                            .any(|jump| matches!(jump, Jump::Continue(_)) && targets(jump));
                    let mut exits = pending
                        .iter()
                        .any(|jump| matches!(jump, Jump::Break(_)) && targets(jump));
                    pending.retain(|jump| !targets(jump));
                    if back {
                        // Around the back edge: the header, then the whole
                        // body again.
                        let (header, body, tested) = loop_parts(&kind);
                        if header.iter().any(|span| self.reads_in(*span)) {
                            return true;
                        }
                        let flow = self.statement(body);
                        if flow.read {
                            return true;
                        }
                        exits |= tested
                            || flow
                                .jumps
                                .iter()
                                .any(|jump| matches!(jump, Jump::Break(_)) && targets(jump));
                        pending.extend(flow.jumps.into_iter().filter(|jump| !targets(jump)));
                    }
                    falls = exits;
                }
                _ => return true,
            }
            // A throw can still reach an enclosing handler from before the
            // store that ended this path.
            if !falls && pending.is_empty() && !self.merger.inside_try_block(parent) {
                return false;
            }
            current = parent;
        }
    }

    /// The statements of `list` after the one at `after`.
    pub(super) fn rest(&self, list: &[Statement<'_>], after: Span) -> Flow {
        let at = list.iter().position(|statement| statement.span() == after).map_or(list.len(), |i| i + 1);
        self.sequence(&list[at..])
    }

    pub(super) fn sequence(&self, list: &[Statement<'_>]) -> Flow {
        let mut flow = Flow {
            falls: true,
            ..Flow::default()
        };
        for statement in list {
            let next = self.statement(statement);
            if next.read {
                flow.read = true;
                return flow;
            }
            flow.jumps.extend(next.jumps);
            if !next.falls {
                flow.falls = false;
                return flow;
            }
        }
        flow
    }

    pub(super) fn statement(&self, statement: &Statement<'_>) -> Flow {
        let plain = |read: bool| Flow {
            read,
            falls: true,
            jumps: Vec::new(),
        };
        match statement {
            Statement::BlockStatement(block) => self.sequence(&block.body),
            Statement::ExpressionStatement(expression) => {
                if let Expression::AssignmentExpression(assignment) = &expression.expression {
                    if assignment.operator == AssignmentOperator::Assign {
                        if let Some(target) = assignment.left.as_simple_assignment_target() {
                            if self.is_local(target) {
                                return Flow {
                                    read: self.reads_in(assignment.right.span()),
                                    falls: false,
                                    jumps: Vec::new(),
                                };
                            }
                        }
                    }
                }
                plain(self.reads_in(expression.span))
            }
            Statement::VariableDeclaration(declaration) => {
                // A `let` re-entered in a loop is a fresh binding.
                let declares = declaration.declarations.iter().any(|declarator| {
                    declarator.id.get_binding_identifier().is_some_and(|binding| binding.symbol_id() == self.local)
                });
                let kills = declares
                    && (declaration.kind != VariableDeclarationKind::Var
                        || declaration.declarations.iter().all(|declarator| declarator.init.is_some()));
                Flow {
                    read: self.reads_in(declaration.span),
                    falls: !kills,
                    jumps: Vec::new(),
                }
            }
            Statement::IfStatement(branch) => {
                if self.reads_in(branch.test.span()) {
                    return plain(true);
                }
                let consequent = self.statement(&branch.consequent);
                let alternate = branch.alternate.as_ref().map_or_else(|| plain(false), |s| self.statement(s));
                let mut jumps = consequent.jumps;
                jumps.extend(alternate.jumps);
                Flow {
                    read: consequent.read || alternate.read,
                    falls: consequent.falls || alternate.falls,
                    jumps,
                }
            }
            Statement::ReturnStatement(_) | Statement::ThrowStatement(_) => Flow {
                read: self.reads_in(statement.span()),
                falls: false,
                jumps: Vec::new(),
            },
            Statement::BreakStatement(jump) => Flow {
                read: false,
                falls: false,
                jumps: vec![Jump::Break(jump.label.as_ref().map(|label| label.name.to_string()))],
            },
            Statement::ContinueStatement(jump) => Flow {
                read: false,
                falls: false,
                jumps: vec![Jump::Continue(jump.label.as_ref().map(|label| label.name.to_string()))],
            },
            Statement::LabeledStatement(labeled) => {
                let label = Some(labeled.label.name.to_string());
                let mut flow = self.statement(&labeled.body);
                if flow.jumps.contains(&Jump::Break(label.clone())) {
                    flow.falls = true;
                }
                flow.jumps.retain(|jump| match jump {
                    Jump::Break(name) | Jump::Continue(name) => *name != label,
                });
                flow
            }
            Statement::TryStatement(statement) => {
                let block = self.sequence(&statement.block.body);
                let handler = statement.handler.as_ref().map(|handler| self.sequence(&handler.body.body));
                let finalizer = statement.finalizer.as_ref().map(|finalizer| self.sequence(&finalizer.body));
                let mut flow = Flow {
                    read: block.read
                        || handler.as_ref().is_some_and(|flow| flow.read)
                        || finalizer.as_ref().is_some_and(|flow| flow.read),
                    falls: block.falls || handler.as_ref().is_some_and(|flow| flow.falls),
                    jumps: block.jumps,
                };
                if let Some(handler) = handler {
                    flow.jumps.extend(handler.jumps);
                }
                if let Some(finalizer) = finalizer {
                    flow.jumps.extend(finalizer.jumps);
                }
                flow
            }
            Statement::SwitchStatement(switch) => {
                let mut flow = plain(self.reads_in(switch.span));
                for case in &switch.cases {
                    let body = self.sequence(&case.consequent);
                    flow.jumps.extend(body.jumps.into_iter().filter(|jump| *jump != Jump::Break(None)));
                }
                flow
            }
            Statement::WhileStatement(_)
            | Statement::DoWhileStatement(_)
            | Statement::ForStatement(_)
            | Statement::ForInStatement(_)
            | Statement::ForOfStatement(_) => {
                let body = match statement {
                    Statement::WhileStatement(s) => &s.body,
                    Statement::DoWhileStatement(s) => &s.body,
                    Statement::ForStatement(s) => &s.body,
                    Statement::ForInStatement(s) => &s.body,
                    Statement::ForOfStatement(s) => &s.body,
                    _ => unreachable!(),
                };
                let inner = self.statement(body);
                let mut flow = plain(self.reads_in(statement.span()));
                flow.jumps = inner
                    .jumps
                    .into_iter()
                    .filter(|jump| !matches!(jump, Jump::Break(None) | Jump::Continue(None)))
                    .collect();
                flow
            }
            _ => plain(self.reads_in(statement.span())),
        }
    }
}

pub(super) fn is_loop(kind: &AstKind<'_>) -> bool {
    matches!(
        kind,
        AstKind::WhileStatement(_)
            | AstKind::DoWhileStatement(_)
            | AstKind::ForStatement(_)
            | AstKind::ForInStatement(_)
            | AstKind::ForOfStatement(_)
    )
}

/// A loop's header parts that run on the back edge, its body, and whether a
/// test can end it there.
pub(super) fn loop_parts<'k, 'a>(kind: &'k AstKind<'a>) -> (Vec<Span>, &'k Statement<'a>, bool) {
    match kind {
        AstKind::WhileStatement(s) => (vec![s.test.span()], &s.body, true),
        AstKind::DoWhileStatement(s) => (vec![s.test.span()], &s.body, true),
        AstKind::ForStatement(s) => {
            let mut header = Vec::new();
            if let Some(update) = &s.update {
                header.push(update.span());
            }
            if let Some(test) = &s.test {
                header.push(test.span());
            }
            (header, &s.body, s.test.is_some())
        }
        AstKind::ForInStatement(s) => (vec![s.left.span(), s.right.span()], &s.body, true),
        AstKind::ForOfStatement(s) => (vec![s.left.span(), s.right.span()], &s.body, true),
        _ => unreachable!(),
    }
}

/// Every path through `statement` that completes runs one of `stores` (a
/// path that throws or returns never reaches a later read).
pub(super) fn definitely_stores(statement: &Statement<'_>, stores: &[Span]) -> bool {
    if stores.contains(&statement.span()) {
        return true;
    }
    match statement {
        Statement::ThrowStatement(_) | Statement::ReturnStatement(_) => true,
        // `for (;;)` always runs its body's first statements.
        Statement::ForStatement(loop_) if loop_.init.is_none() && loop_.test.is_none() => {
            definitely_stores(&loop_.body, stores)
        }
        Statement::BlockStatement(block) => block.body.iter().any(|s| definitely_stores(s, stores)),
        Statement::IfStatement(branch) => {
            definitely_stores(&branch.consequent, stores)
                && branch.alternate.as_ref().is_some_and(|alternate| definitely_stores(alternate, stores))
        }
        _ => false,
    }
}

pub(super) fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

#[derive(Default)]
pub(super) struct Identifiers {
    pub(super) references: Vec<oxc_semantic::ReferenceId>,
}

impl<'a> Visit<'a> for Identifiers {
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        self.references.push(identifier.reference_id());
    }
    // A name the value only reads a property of is not what the value
    // derives from.
    fn visit_member_expression(&mut self, member: &MemberExpression<'a>) {
        match member {
            MemberExpression::ComputedMemberExpression(computed) => {
                if !matches!(computed.object, Expression::Identifier(_)) {
                    self.visit_expression(&computed.object);
                }
                self.visit_expression(&computed.expression);
            }
            MemberExpression::StaticMemberExpression(member) => {
                if !matches!(member.object, Expression::Identifier(_)) {
                    self.visit_expression(&member.object);
                }
            }
            MemberExpression::PrivateFieldExpression(member) => {
                if !matches!(member.object, Expression::Identifier(_)) {
                    self.visit_expression(&member.object);
                }
            }
        }
    }
    fn visit_function(&mut self, _: &Function<'a>, _: oxc_syntax::scope::ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
}
