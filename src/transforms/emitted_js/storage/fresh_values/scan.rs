//! The scan for fresh values and the reads they can move to.

use super::*;

impl Scan<'_, '_> {
    pub(super) fn split_value(
        &self,
        e: &Expression<'_>,
        scope: oxc_semantic::ScopeId,
        at: Span,
        depth: usize,
    ) -> Option<SplitValue> {
        if depth == 0 {
            return None;
        }
        let e = unwrap(e);
        let child = |e| self.split_value(e, scope, at, depth - 1);
        // Do not let a plan leaf allocate a new identity at each read.
        if identity_free(e) && self.inert(e, scope, at) {
            return Some(SplitValue {
                effects: String::new(),
                value: format!("({})", self.text(e)),
                scalar: self.primitive(e, 24),
                null: matches!(e, Expression::NullLiteral(_)),
            });
        }
        match e {
            Expression::SequenceExpression(s) => self.split_sequence(s, scope, at, depth - 1),
            Expression::ConditionalExpression(c) => {
                self.split_conditional(c, scope, at, depth - 1)
            }
            Expression::LogicalExpression(l) => self.split_logical(l, scope, at, depth - 1),
            Expression::AssignmentExpression(a) if a.operator == AssignmentOperator::Assign => {
                let mut value = child(&a.right)?;
                // The assignment, including reference/key/setter evaluation,
                // remains intact at its original location. Only its already
                // stable result is recomputed after the successful store.
                value.effects = self.text(e);
                Some(value)
            }
            Expression::UnaryExpression(u) if u.operator == UnaryOperator::Void => {
                Some(SplitValue {
                    effects: self.text(&u.argument),
                    value: "void 0".into(),
                    scalar: true,
                    null: false,
                })
            }
            Expression::UnaryExpression(u) if u.operator != UnaryOperator::Delete => {
                let value = child(&u.argument)?;
                if !matches!(
                    u.operator,
                    UnaryOperator::Typeof | UnaryOperator::LogicalNot
                ) && !value.scalar
                {
                    return None;
                }
                Some(SplitValue {
                    effects: value.effects,
                    value: format!("{} ({})", u.operator.as_str(), value.value),
                    scalar: true,
                    null: false,
                })
            }
            Expression::BinaryExpression(b) => self.split_binary(b, scope, at, depth - 1),
            Expression::TemplateLiteral(t) => self.split_template(e, t, scope, at, depth - 1),
            _ => None,
        }
    }
    fn text(&self, e: &Expression<'_>) -> String {
        self.source[e.span().start as usize..e.span().end as usize].to_string()
    }
    /// Split `a, b, c`: the prefixes join the effects of the last operand.
    /// `depth` bounds the operands.
    fn split_sequence(
        &self,
        s: &SequenceExpression<'_>,
        scope: oxc_semantic::ScopeId,
        at: Span,
        depth: usize,
    ) -> Option<SplitValue> {
        let mut value = self.split_value(s.expressions.last()?, scope, at, depth)?;
        let mut effects = Vec::new();
        for prefix in &s.expressions[..s.expressions.len() - 1] {
            // Literals and ready private reads have no discarded
            // effect; their values need not stay unchanged later.
            let discard = matches!(
                unwrap(prefix),
                Expression::NumericLiteral(_)
                    | Expression::StringLiteral(_)
                    | Expression::BooleanLiteral(_)
                    | Expression::NullLiteral(_)
                    | Expression::BigIntLiteral(_)
            ) || matches!(unwrap(prefix),Expression::Identifier(id) if analysis::identifier_symbol(id,self.scoping).is_some_and(|s| self.locals.initialized(s,id.span)));
            if !discard {
                effects.push(self.text(prefix));
            }
        }
        effects.push(value.effects);
        value.effects = join_effects(effects);
        Some(value)
    }
    /// Split `t ? a : b` over an inert test, selecting each arm's effects.
    fn split_conditional(
        &self,
        c: &ConditionalExpression<'_>,
        scope: oxc_semantic::ScopeId,
        at: Span,
        depth: usize,
    ) -> Option<SplitValue> {
        // Repeating this test after the selected effects requires its
        // complete nonthrowing value and dependencies to stay stable.
        if !self.inert(&c.test, scope, at) {
            return None;
        }
        let yes = self.split_value(&c.consequent, scope, at, depth)?;
        let no = self.split_value(&c.alternate, scope, at, depth)?;
        let effects = if yes.effects.is_empty() && no.effects.is_empty() {
            String::new()
        } else {
            format!(
                "({})?({}):({})",
                self.text(&c.test),
                if yes.effects.is_empty() {
                    "0"
                } else {
                    &yes.effects
                },
                if no.effects.is_empty() {
                    "0"
                } else {
                    &no.effects
                }
            )
        };
        Some(SplitValue {
            effects,
            value: format!("({})?({}):({})", self.text(&c.test), yes.value, no.value),
            scalar: yes.scalar && no.scalar,
            null: yes.null && no.null,
        })
    }
    /// Split `l && r` (or `||`, `??`) over an inert left operand.
    fn split_logical(
        &self,
        l: &LogicalExpression<'_>,
        scope: oxc_semantic::ScopeId,
        at: Span,
        depth: usize,
    ) -> Option<SplitValue> {
        if !self.inert(&l.left, scope, at) {
            return None;
        }
        let left = self.split_value(&l.left, scope, at, depth)?;
        let right = self.split_value(&l.right, scope, at, depth)?;
        let effects = if right.effects.is_empty() {
            String::new()
        } else {
            format!(
                "({}){}({})",
                self.text(&l.left),
                l.operator.as_str(),
                right.effects
            )
        };
        Some(SplitValue {
            effects,
            value: format!("({}){}({})", left.value, l.operator.as_str(), right.value),
            scalar: left.scalar && right.scalar,
            null: left.null && right.null,
        })
    }
    /// Split a binary operation that cannot call user code on its operands.
    fn split_binary(
        &self,
        b: &BinaryExpression<'_>,
        scope: oxc_semantic::ScopeId,
        at: Span,
        depth: usize,
    ) -> Option<SplitValue> {
        let left = self.split_value(&b.left, scope, at, depth)?;
        let right = self.split_value(&b.right, scope, at, depth)?;
        let safe = matches!(
            b.operator,
            BinaryOperator::StrictEquality | BinaryOperator::StrictInequality
        ) || matches!(
            b.operator,
            BinaryOperator::Equality | BinaryOperator::Inequality
        ) && (left.null || right.null)
            || !b.operator.is_in()
                && !b.operator.is_instance_of()
                && left.scalar
                && right.scalar;
        if !safe {
            return None;
        }
        Some(SplitValue {
            effects: join_effects([left.effects, right.effects]),
            value: format!("({}){}({})", left.value, b.operator.as_str(), right.value),
            scalar: true,
            null: false,
        })
    }
    /// Split a template literal `e` whose substitutions are all scalar.
    fn split_template(
        &self,
        e: &Expression<'_>,
        t: &TemplateLiteral<'_>,
        scope: oxc_semantic::ScopeId,
        at: Span,
        depth: usize,
    ) -> Option<SplitValue> {
        let mut effects = Vec::new();
        let mut edits = Vec::new();
        for e in &t.expressions {
            let value = self.split_value(e, scope, at, depth)?;
            if !value.scalar {
                return None;
            }
            effects.push(value.effects);
            let span = Span::new(e.span().start - t.span.start, e.span().end - t.span.start);
            edits.push(analysis::Edit::replace(span, format!("({})", value.value)));
        }
        let applied = analysis::apply_edits(&self.text(e), edits);
        if !applied.rejected.is_empty() {
            return None;
        }
        Some(SplitValue {
            effects: join_effects(effects),
            value: applied.output,
            scalar: true,
            null: false,
        })
    }
    pub(super) fn stable_iterations(&self, symbol: oxc_semantic::SymbolId, from: Span, to: Span) -> bool {
        if !self.locals.same_owner(from, to) {
            return false;
        }
        let Some(repeated) = self.locals.repeated_between(from, to) else {
            return true;
        };
        let definition = self.scoping.symbol_span(symbol);
        // Implicit iteration/declarator stores are not all identifier writes.
        if repeated.start <= definition.start && definition.end <= repeated.end {
            return false;
        }
        self.scoping.get_resolved_references(symbol).all(|r| {
            let at = self.nodes.get_node(r.node_id()).kind().span();
            !(r.is_write() && repeated.start <= at.start && at.end <= repeated.end)
        })
    }
    pub(super) fn stable_var_initializer(&self, symbol: oxc_semantic::SymbolId, from: Span, to: Span) -> bool {
        if self
            .inputs
            .function_names
            .contains(self.scoping.symbol_name(symbol))
        {
            return false;
        }
        let oxc_ast::AstKind::VariableDeclarator(d) =
            self.nodes.kind(self.scoping.symbol_declaration(symbol))
        else {
            return false;
        };
        // A var initializer is an implicit write, even when reference tables
        // show no write. Its sole execution site must be outside this move.
        // The same-iteration proof below prevents crossing a backedge that
        // could execute an earlier initializer again.
        d.init.as_ref().is_none_or(|init| {
            self.locals.same_owner(init.span(), from)
                && !(from.start <= init.span().start && init.span().start < to.end)
        })
    }
    // Type on successful initialization, independently of initializer effects.
    // Exclude BigInt and Symbol so subsequent arithmetic cannot throw or call
    // coercion hooks. Mutable cells require a category preserved by every
    // successful store, rather than their initial definition alone.
    pub(super) fn primitive(&self, e: &Expression<'_>, depth: usize) -> bool {
        if depth == 0 {
            return false;
        }
        let primitive = |e| self.primitive(e, depth - 1);
        match unwrap(e) {
            Expression::NumericLiteral(_)
            | Expression::StringLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_) => true,
            Expression::Identifier(id) => {
                let Some(symbol) = analysis::identifier_symbol(id, self.scoping) else {
                    return false;
                };
                !self.protected.contains(id.name.as_str())
                    && (self.cells.values.get(&id.span).is_some_and(|v| v.scalar)
                        || self.locals.primitive_cell(symbol, id.span)
                        || (self.scoping.symbol_redeclarations(symbol).is_empty()
                            && self
                                .scoping
                                .get_resolved_references(symbol)
                                .all(|r| !r.is_write())
                            && matches!(self.nodes.kind(self.scoping.symbol_declaration(symbol)),
                                oxc_ast::AstKind::VariableDeclarator(d)
                                if matches!(d.id, BindingPattern::BindingIdentifier(_))
                                    && d.init.as_ref().is_some_and(primitive))))
            }
            Expression::TemplateLiteral(_) => true,
            Expression::SequenceExpression(s) => s.expressions.last().is_some_and(primitive),
            Expression::UnaryExpression(u) => {
                matches!(
                    u.operator,
                    UnaryOperator::UnaryPlus
                        | UnaryOperator::LogicalNot
                        | UnaryOperator::Typeof
                        | UnaryOperator::Void
                        | UnaryOperator::Delete
                ) || primitive(&u.argument)
            }
            Expression::BinaryExpression(b) => {
                b.operator.is_compare()
                    || b.operator.is_in()
                    || b.operator.is_instance_of()
                    || primitive(&b.left)
                    || primitive(&b.right)
            }
            Expression::LogicalExpression(l) => primitive(&l.left) && primitive(&l.right),
            Expression::ConditionalExpression(c) => {
                primitive(&c.consequent) && primitive(&c.alternate)
            }
            _ => false,
        }
    }
    pub(super) fn inert(&self, e: &Expression<'_>, use_scope: oxc_semantic::ScopeId, at: Span) -> bool {
        match unwrap(e) {
            Expression::Identifier(id) => analysis::identifier_symbol(id, self.scoping)
                .is_some_and(|symbol| {
                    !self.protected.contains(id.name.as_str())
                        && (self.scoping.symbol_flags(symbol)
                            .contains(oxc_syntax::symbol::SymbolFlags::BlockScopedVariable)
                            || self.scoping.symbol_flags(symbol).is_function_scoped_declaration()
                                && self.stable_var_initializer(symbol, id.span, at))
                        && self.scoping.symbol_redeclarations(symbol).is_empty()
                        && self.locals.initialized(symbol, id.span)
                        && self.locals.initialized(symbol, at)
                        // A lexical cell has no hoisted initializer writes.
                        // Captured writers and stores in the moved interval
                        // can change its version, including stores in a
                        // consumer's prefix or a nested loop.
                        && self.stable_iterations(symbol,id.span,at)
                        && self.scoping.get_resolved_references(symbol).all(|r| {
                            if !r.is_write() { return true; }
                            let write = self.nodes.get_node(r.node_id()).kind().span();
                            let completed = match self.nodes.parent_kind(r.node_id()) {
                                oxc_ast::AstKind::AssignmentExpression(a) => a.span.end,
                                oxc_ast::AstKind::UpdateExpression(u) => u.span.end,
                                _ => write.start,
                            };
                            self.locals.same_owner(id.span, write)
                                && !(id.span.start <= completed && completed < at.end)
                        })
                        && self.scoping.find_binding(use_scope, id.name) == Some(symbol)
                }),
            Expression::ComputedMemberExpression(m)
                if self.ordinary_arguments && super::super::super::argument_snapshots::input(e) =>
            {
                let Expression::Identifier(id) = unwrap(&m.object) else {
                    return false;
                };
                analysis::identifier_symbol(id, self.scoping).is_none()
                    && self.inputs.owners.enclosing(e.span()).is_some_and(|owner| {
                        self.inputs.readonly_arguments.contains(&owner)
                            && self.inputs.owners.enclosing(at) == Some(owner)
                            && self.scoping.find_binding(use_scope, id.name).is_none()
                    })
            }
            Expression::ArrayExpression(a) => a.elements.iter().all(|e| match e {
                ArrayExpressionElement::Elision(_) => true,
                ArrayExpressionElement::SpreadElement(_) => false,
                _ => e
                    .as_expression()
                    .is_some_and(|e| self.inert(e, use_scope, at)),
            }),
            Expression::ObjectExpression(o) => o.properties.iter().all(|p| match p {
                ObjectPropertyKind::SpreadProperty(_) => false,
                ObjectPropertyKind::ObjectProperty(p) => {
                    !p.computed
                        && !p.method
                        && p.kind == PropertyKind::Init
                        && self.inert(&p.value, use_scope, at)
                }
            }),
            Expression::NumericLiteral(_)
            | Expression::StringLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::BigIntLiteral(_)
            | Expression::RegExpLiteral(_) => true,
            Expression::UnaryExpression(u) => {
                if matches!(
                    u.operator,
                    UnaryOperator::LogicalNot | UnaryOperator::Typeof | UnaryOperator::Void
                ) {
                    return self.inert(&u.argument, use_scope, at);
                }
                if u.operator != UnaryOperator::Delete && self.primitive(&u.argument, 24) {
                    return self.inert(&u.argument, use_scope, at);
                }
                matches!(
                    unwrap(&u.argument),
                    Expression::NumericLiteral(_)
                        | Expression::StringLiteral(_)
                        | Expression::BooleanLiteral(_)
                        | Expression::NullLiteral(_)
                        | Expression::BigIntLiteral(_)
                ) && super::super::super::purity::expression_is_pure(e)
            }
            Expression::BinaryExpression(b)
                if matches!(
                    b.operator,
                    BinaryOperator::StrictEquality | BinaryOperator::StrictInequality
                ) || (matches!(
                    b.operator,
                    BinaryOperator::Equality | BinaryOperator::Inequality
                ) && (matches!(unwrap(&b.left), Expression::NullLiteral(_))
                    || matches!(unwrap(&b.right), Expression::NullLiteral(_)))) =>
            {
                self.inert(&b.left, use_scope, at) && self.inert(&b.right, use_scope, at)
            }
            Expression::BinaryExpression(b)
                if !b.operator.is_in()
                    && !b.operator.is_instance_of()
                    && self.primitive(&b.left, 24)
                    && self.primitive(&b.right, 24) =>
            {
                self.inert(&b.left, use_scope, at) && self.inert(&b.right, use_scope, at)
            }
            Expression::TemplateLiteral(t) => t
                .expressions
                .iter()
                .all(|e| self.primitive(e, 24) && self.inert(e, use_scope, at)),
            Expression::SequenceExpression(s) => {
                s.expressions.iter().all(|e| self.inert(e, use_scope, at))
            }
            Expression::LogicalExpression(l) => {
                self.inert(&l.left, use_scope, at) && self.inert(&l.right, use_scope, at)
            }
            Expression::ConditionalExpression(c) => {
                self.inert(&c.test, use_scope, at)
                    && self.inert(&c.consequent, use_scope, at)
                    && self.inert(&c.alternate, use_scope, at)
            }
            _ => false,
        }
    }
    // A top-level while-test store executes on every test before any body
    // read. It is not a loop-carried value when this is its only store and
    // every read stays in this loop instance. Alias and computed snapshots
    // use the same dependency-version proof as declaration initializers.
    pub(super) fn loop_values(&mut self, condition: &Expression<'_>, region: Span, update: Option<Span>) {
        let test = unwrap(condition);
        let expressions: Vec<_> = match test {
            Expression::SequenceExpression(s) => s.expressions.iter().collect(),
            _ => vec![test],
        };
        for expression in expressions.iter().take(expressions.len().saturating_sub(1)) {
            let Expression::AssignmentExpression(a) = unwrap(expression) else {
                continue;
            };
            let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
                continue;
            };
            let Some(symbol) = analysis::identifier_symbol(id, self.scoping) else {
                continue;
            };
            if a.operator != AssignmentOperator::Assign
                || !(id.name.starts_with("$SSA_") || id.name.starts_with("$CARRY_"))
                || self.protected.contains(id.name.as_str())
                || self.scoping.symbol_flags(symbol).is_const_variable()
                || self
                    .scoping
                    .symbol_flags(symbol)
                    .is_function_scoped_declaration()
                    && self.inputs.function_names.contains(id.name.as_str())
                || !self.scoping.symbol_redeclarations(symbol).is_empty()
                || !self.locals.initialized(symbol, a.span)
                || !identity_free(&a.right)
                || super::super::super::argument_snapshots::input(&a.right)
            {
                continue;
            }
            let oxc_ast::AstKind::VariableDeclarator(d) =
                self.nodes.kind(self.scoping.symbol_declaration(symbol))
            else {
                continue;
            };
            if d.init.is_some() || !matches!(d.id, BindingPattern::BindingIdentifier(_)) {
                continue;
            }
            let refs: Vec<_> = self.scoping.get_resolved_references(symbol).collect();
            if refs.iter().filter(|r| r.is_write()).count() != 1 {
                continue;
            }
            let reads: Vec<_> = refs.iter().filter(|r| r.is_read()).collect();
            if reads.is_empty()
                || reads.iter().any(|r| {
                    let at = self.nodes.get_node(r.node_id()).kind().span();
                    at.start < a.span.end
                        || at.end > region.end
                        || update.is_some_and(|u| u.start <= at.start && at.end <= u.end)
                        || !self.locals.same_iteration(a.span, at)
                        || !self.locals.initialized(symbol, at)
                        || !self.inert(&a.right, r.scope_id(), at)
                })
            {
                continue;
            }
            let removed = &self.source[a.span.start as usize..a.span.end as usize];
            if removed.contains("/*") || removed.contains("//") {
                continue;
            }
            // The store's value is consumed only by the discarded comma
            // operand. Replacing it with zero therefore retains outer comma
            // GetValue semantics without executing an extra input read.
            self.loop_stores.insert(a.span);
            self.edits.push((
                a.span,
                reads
                    .iter()
                    .map(|r| self.nodes.get_node(r.node_id()).kind().span())
                    .collect(),
                a.right.span(),
                true,
            ));
            self.private_temps.insert(a.span);
        }
    }
    pub(super) fn body(&mut self, body: &[Statement<'_>]) {
        for s in body {
            let Statement::VariableDeclaration(d) = s else {
                continue;
            };
            if !matches!(
                d.kind,
                VariableDeclarationKind::Let | VariableDeclarationKind::Const
            ) || d.declarations.len() != 1
            {
                continue;
            }
            let (BindingPattern::BindingIdentifier(id), Some(init)) =
                (&d.declarations[0].id, &d.declarations[0].init)
            else {
                continue;
            };
            if self.protected.contains(id.name.as_str())
                || !matches!(
                    unwrap(init),
                    Expression::ArrayExpression(_)
                        | Expression::ObjectExpression(_)
                        | Expression::RegExpLiteral(_)
                        | Expression::UnaryExpression(_)
                        | Expression::BinaryExpression(_)
                        | Expression::LogicalExpression(_)
                        | Expression::ConditionalExpression(_)
                        | Expression::TemplateLiteral(_)
                        | Expression::SequenceExpression(_)
                        | Expression::AssignmentExpression(_)
                )
            {
                continue;
            }
            let Some(symbol) = id.symbol_id.get() else {
                continue;
            };
            if !self.scoping.symbol_redeclarations(symbol).is_empty() {
                continue;
            }
            let refs: Vec<_> = self.scoping.get_resolved_references(symbol).collect();
            let mut split = None;
            let mut valid = !refs.is_empty();
            for reference in &refs {
                let at = self.nodes.get_node(reference.node_id()).kind().span();
                if reference.is_write()
                    || at.start < d.span.end
                    || !self.locals.initialized(symbol, at)
                    || !self.locals.same_owner(d.span, at)
                {
                    valid = false;
                    break;
                }
                if self.inert(init, reference.scope_id(), at) {
                    if !self.locals.same_iteration(d.span, at) && !identity_free(init) {
                        valid = false;
                        break;
                    }
                } else {
                    let Some(plan) = self.split_value(init, reference.scope_id(), at, 32) else {
                        valid = false;
                        break;
                    };
                    if plan.effects.is_empty() || split.as_ref().is_some_and(|old| old != &plan) {
                        valid = false;
                        break;
                    }
                    split = Some(plan);
                }
            }
            if !valid {
                continue;
            }
            let text = &self.source[d.span.start as usize..d.span.end as usize];
            if text.contains("/*") || text.contains("//") {
                continue;
            }
            let reads = refs
                .iter()
                .map(|r| self.nodes.get_node(r.node_id()).kind().span())
                .collect();
            let rematerializable = split.is_some() || identity_free(init);
            if let Some(plan) = split {
                self.splits.insert(
                    d.span,
                    SplitValue {
                        effects: format!("({});", plan.effects),
                        ..plan
                    },
                );
            }
            self.edits
                .push((d.span, reads, init.span(), rematerializable));
            if ["$SSA_", "$CARRY_", "$ARRAY_", "$OBJECT_"]
                .iter()
                .any(|prefix| id.name.starts_with(prefix))
            {
                self.private_temps.insert(d.span);
            }
        }
    }
}

impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_while_statement(&mut self, w: &WhileStatement<'a>) {
        self.loop_values(&w.test, w.span, None);
        walk::walk_while_statement(self, w);
    }
    fn visit_for_statement(&mut self, f: &ForStatement<'a>) {
        if let Some(test) = &f.test {
            self.loop_values(test, f.span, f.update.as_ref().map(GetSpan::span));
        }
        walk::walk_for_statement(self, f);
    }
    fn visit_if_statement(&mut self, i: &IfStatement<'a>) {
        if let Some(no) = &i.alternate {
            self.exclusive.push((i.consequent.span(), no.span()));
        }
        walk::walk_if_statement(self, i);
    }
    fn visit_conditional_expression(&mut self, c: &ConditionalExpression<'a>) {
        self.exclusive
            .push((c.consequent.span(), c.alternate.span()));
        walk::walk_conditional_expression(self, c);
    }
    fn visit_class(&mut self, c: &Class<'a>) {
        self.classes.push(c.span);
        walk::walk_class(self, c);
    }
    fn visit_function(&mut self, f: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        // A not-yet-linked child can still capture a currently single-use
        // cell. Slot metadata pins numeric slots; arbitrary local names need
        // the actual child body before this identity proof is available.
        self.pending_children |=
            f.id.as_ref()
                .is_some_and(|id| id.name.starts_with("$THREAD_"))
                && f.body
                    .as_ref()
                    .is_some_and(|b| b.statements.is_empty() && b.directives.is_empty());
        walk::walk_function(self, f, flags);
    }
    fn visit_expression_statement(&mut self, s: &ExpressionStatement<'a>) {
        self.strings |= matches!(s.expression, Expression::StringLiteral(_));
        walk::walk_expression_statement(self, s);
    }
    fn visit_function_body(&mut self, b: &FunctionBody<'a>) {
        self.body(&b.statements);
        walk::walk_function_body(self, b);
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        self.body(&b.body);
        walk::walk_block_statement(self, b);
    }
    fn visit_object_property(&mut self, p: &ObjectProperty<'a>) {
        if p.shorthand {
            self.non_values.insert(p.value.span());
        }
        walk::walk_object_property(self, p);
    }
    fn visit_unary_expression(&mut self, u: &UnaryExpression<'a>) {
        if u.operator == UnaryOperator::Delete {
            self.non_values.insert(unwrap(&u.argument).span());
        }
        walk::walk_unary_expression(self, u);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.dynamic |= id.name == "eval";
    }
    fn visit_with_statement(&mut self, s: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, s);
    }
}
