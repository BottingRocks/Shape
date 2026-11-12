//! The scan over statement pairs and comma sequences.

use super::*;

impl Scan<'_> {
    /// Move an evaluation past a private, independent and nonthrowing
    /// declaration. Reads in the skipped initializer must be stable across
    /// the moved evaluation; newly initialized cells cannot be captured or
    /// mentioned by that evaluation (including a TDZ read).
    pub(super) fn independent_comma(
        &self,
        init: &Expression<'_>,
        next: &Expression<'_>,
        target: SymbolId,
        readable: &BTreeSet<SymbolId>,
        policy: ReadPolicy<'_>,
    ) -> bool {
        let mut value = next;
        while let Expression::ParenthesizedExpression(p) = value {
            value = &p.expression;
        }
        if let Expression::AssignmentExpression(a) = value {
            let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
                return false;
            };
            let Some(symbol) = analysis::identifier_symbol(id, self.scoping) else {
                return false;
            };
            if a.operator != AssignmentOperator::Assign
                || symbol == target
                || self.protected.contains(id.name.as_str())
                || self.scoping.symbol_flags(symbol).is_const_variable()
                || !self.scoping.symbol_redeclarations(symbol).is_empty()
                || !self.local_reads.initialized(symbol, a.span)
                || self.scoping.get_resolved_references(symbol).any(|r| {
                    let at = self.nodes.get_node(r.node_id()).kind().span();
                    !self.local_reads.same_owner(init.span(), at)
                        || init.span().start <= at.start && at.end <= init.span().end
                })
            {
                return false;
            }
            // A handler must not observe a store advanced past a throwing
            // producer. Outside handlers, only an unobservable private cell
            // can cross an abrupt function exit.
            if self.local_reads.exceptions.enclosing(a.span).is_some()
                && !matches!(
                    prefix_read(
                        init,
                        target,
                        self.scoping,
                        readable,
                        false,
                        self.protected,
                        policy
                    ),
                    Ok(None)
                )
            {
                return false;
            }
            value = &a.right;
        }
        matches!(
            prefix_read(
                value,
                target,
                self.scoping,
                readable,
                false,
                self.protected,
                policy
            ),
            Ok(None)
        )
    }

    /// The cell a single-binding declaration or plain assignment stores, and
    /// the value it stores there.
    fn stored_cell<'a, 'b>(&self, first: &'b Statement<'a>) -> Option<(SymbolId, &'b Expression<'a>)> {
        match first {
            Statement::VariableDeclaration(d) if d.declarations.len() == 1 => {
                let (BindingPattern::BindingIdentifier(id), Some(init)) =
                    (&d.declarations[0].id, &d.declarations[0].init)
                else {
                    return None;
                };
                let symbol = id.symbol_id.get()?;
                Some((symbol, init))
            }
            Statement::ExpressionStatement(s) => {
                let Expression::AssignmentExpression(a) = &s.expression else {
                    return None;
                };
                let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
                    return None;
                };
                if a.operator != AssignmentOperator::Assign {
                    return None;
                }
                let symbol = analysis::identifier_symbol(id, self.scoping)?;
                Some((symbol, &a.right))
            }
            _ => None,
        }
    }

    /// Whether `init`, stored into `target`, can move past a function
    /// declaration.
    fn independent_function(
        &self,
        function: &Function<'_>,
        target: SymbolId,
        init: &Expression<'_>,
    ) -> bool {
        // Creating a function does not execute its body, but Annex-B
        // declarations can write a same-spelled outer binding.
        let Some(name) = &function.id else {
            return false;
        };
        let Some(function_symbol) = name.symbol_id.get() else {
            return false;
        };
        // Strict block functions are initialized on block entry. For a
        // sloppy block, also prove that the Annex-B outer alias is never
        // observed: every read of this spelling must resolve to this
        // lexical function binding. Calls/getters can then move without
        // observing its implicit outer assignment.
        if self
            .scoping
            .scope_flags(self.scoping.symbol_scope_id(function_symbol))
            .is_strict_mode()
            || (!self
                .scoping
                .root_unresolved_references()
                .contains_key(name.name.as_str())
                && self
                    .observed_names
                    .get(name.name.as_str())
                    .is_none_or(|symbols| {
                        symbols.iter().all(|symbol| *symbol == function_symbol)
                    }))
        {
            return true;
        }
        let mut expression = init;
        while let Expression::ParenthesizedExpression(p) = expression {
            expression = &p.expression;
        }
        matches!(expression, Expression::Identifier(id)
            if id.name != name.name
                && self.scoping.symbol_name(target) != name.name.as_str()
                && self.local_reads.commuting_read(init, self.scoping, self.protected) == Some(true))
    }

    /// The cell a skipped declaration or private store writes, its value, and
    /// whether the store advances an existing cell.
    fn skipped_store<'a, 'b>(
        &self,
        next: &'b Statement<'a>,
    ) -> Option<(SymbolId, &'b Expression<'a>, bool)> {
        match next {
            Statement::VariableDeclaration(d)
                if d.declarations.len() == 1
                    && matches!(
                        d.kind,
                        VariableDeclarationKind::Let
                            | VariableDeclarationKind::Const
                            | VariableDeclarationKind::Var
                    ) =>
            {
                let (BindingPattern::BindingIdentifier(id), Some(value)) =
                    (&d.declarations[0].id, &d.declarations[0].init)
                else {
                    return None;
                };
                let symbol = id.symbol_id.get()?;
                Some((symbol, value, d.kind == VariableDeclarationKind::Var))
            }
            Statement::ExpressionStatement(s) => {
                let Expression::AssignmentExpression(a) = &s.expression else {
                    return None;
                };
                let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
                    return None;
                };
                let symbol = analysis::identifier_symbol(id, self.scoping)?;
                // A private initialized mutable cell has a nonthrowing store.
                // Parameters, shared cells and captured observers are excluded.
                if a.operator != AssignmentOperator::Assign
                    || self.scoping.symbol_flags(symbol).is_const_variable()
                    || !self.local_reads.initialized(symbol, s.span)
                {
                    return None;
                }
                Some((symbol, &a.right, true))
            }
            _ => None,
        }
    }

    pub(super) fn independent_declaration(&self, first: &Statement<'_>, next: &Statement<'_>) -> bool {
        let Some((target, init)) = self.stored_cell(first) else {
            return false;
        };
        if let Statement::FunctionDeclaration(function) = next {
            return self.independent_function(function, target, init);
        }
        let Some((symbol, value, advances_store)) = self.skipped_store(next) else {
            return false;
        };
        // Delaying a throwing producer until after a store can expose the
        // advanced value to catch/finally, or after a handled exception.
        // An initialized plain read cannot throw; other producers keep the
        // original ordering within exception regions.
        if advances_store
            && self.local_reads.exceptions.enclosing(init.span()).is_some()
            && self
                .local_reads
                .commuting_read(init, self.scoping, self.protected)
                != Some(true)
        {
            return false;
        }
        if symbol == target
            || self.protected.contains(self.scoping.symbol_name(symbol))
            || !self.scoping.symbol_redeclarations(symbol).is_empty()
            || self.scoping.get_resolved_references(symbol).any(|r| {
                let at = self.nodes.get_node(r.node_id()).kind().span();
                self.local_reads.regions.enclosing(at)
                    != self.local_reads.regions.enclosing(next.span())
                    || (init.span().start <= at.start && at.end <= init.span().end)
            })
        {
            return false;
        }
        let mut readable = BTreeSet::new();
        self.local_reads.readable(init.span(), &mut readable);
        if self
            .local_reads
            .commuting_read(init, self.scoping, self.protected)
            .is_some()
        {
            self.local_reads.plain_reads(init.span(), &mut readable);
        }
        readable.remove(&target);
        // Reuse the ordered-expression proof with the excluded source cell. A
        // complete traversal proves the skipped RHS has no getter, call,
        // coercion hook or throw, including numeric induction updates.
        harmless_prefix(value, self.scoping, &readable)
            || prefix_read(
                value,
                target,
                self.scoping,
                &readable,
                false,
                self.protected,
                ReadPolicy {
                    commuting_initialized_read: false,
                    this_initialized: self.local_reads.classes.enclosing(init.span()).is_none(),
                    primitive_inputs: &self.local_reads.primitive_inputs,
                    cells: self.cells,
                    ordinary_number: self.local_reads.ordinary_number,
                },
            )
            .is_ok_and(|found| found.is_none())
    }
    pub(super) fn inert_declaration(&self, s: &Statement<'_>) -> bool {
        let Statement::VariableDeclaration(d) = s else {
            return false;
        };
        d.declarations.iter().all(|item| {
            let BindingPattern::BindingIdentifier(id) = &item.id else {
                return false;
            };
            if item.init.is_some() {
                return false;
            }
            if d.kind == VariableDeclarationKind::Var {
                return true;
            }
            let Some(symbol) = id.symbol_id.get() else {
                return false;
            };
            self.local_reads
                .initialized(symbol, Span::new(d.span.end, d.span.end))
                && self.scoping.get_resolved_references(symbol).all(|r| {
                    let span = self.nodes.get_node(r.node_id()).kind().span();
                    span.start >= d.span.end
                        && self.local_reads.regions.enclosing(span)
                            == self.local_reads.regions.enclosing(d.span)
                })
        })
    }
    pub(super) fn body(&mut self, statements: &[Statement<'_>], hoisted: bool) {
        let mut destinations = BTreeSet::new();
        let mut initialized = BTreeSet::new();
        let executable: Vec<_> = statements
            .iter()
            .filter(|s| {
                !(hoisted && matches!(s, Statement::FunctionDeclaration(_))
                    || self.native_calls && self.inert_declaration(s))
            })
            .collect();
        for (index, first) in executable.iter().enumerate() {
            let mut next = index + 1;
            if self.native_calls {
                while executable
                    .get(next)
                    .is_some_and(|s| self.independent_declaration(first, s))
                {
                    next += 1;
                }
            }
            let Some(second) = executable.get(next) else {
                continue;
            };
            let pair = [*first, *second];
            self.record_initialized(pair[0], &mut initialized);
            if destinations.contains(&pair[0].span()) {
                continue;
            }
            let Some((symbol, initializer, deletion)) = self.inlined_store(pair[0]) else {
                continue;
            };
            let (readable, policy) = self.initializer_reads(initializer, &initialized);
            let Some(use_identifier) =
                self.first_use(pair[1], symbol, initializer, &readable, policy)
            else {
                continue;
            };
            if analysis::identifier_symbol(use_identifier, self.scoping) != Some(symbol) {
                continue;
            }
            if !self.fits_inserted_scope(initializer, use_identifier) {
                continue;
            }
            let local_version = self.native_calls
                && self
                    .local_reads
                    .initialized(symbol, Span::new(pair[0].span().end, pair[0].span().end))
                && !self.protected.contains(self.scoping.symbol_name(symbol));
            let mut named_initializer = initializer;
            while let Expression::ParenthesizedExpression(p) = named_initializer {
                named_initializer = &p.expression;
            }
            if matches!(
                named_initializer,
                Expression::ArrowFunctionExpression(_)
                    | Expression::FunctionExpression(_)
                    | Expression::ClassExpression(_)
            ) {
                continue;
            }
            if self.scoping.symbol_flags(symbol).is_const_variable()
                && self
                    .scoping
                    .get_resolved_references(symbol)
                    .any(|r| r.is_write())
            {
                continue;
            }
            if !self.single_read_of_version(
                statements,
                &executable,
                pair,
                symbol,
                initializer,
                local_version,
            ) {
                continue;
            }
            let value_span = initializer.span();
            let value = &self.source[value_span.start as usize..value_span.end as usize];
            let replacement = self.replacement(pair[1], use_identifier, named_initializer, value);
            // The value's own text moves; a comment elsewhere in the
            // declaration would be deleted with it, as the comma form guards.
            let declaration = pair[0].span();
            let outside = format!(
                "{}{}",
                &self.source[declaration.start as usize..value_span.start as usize],
                &self.source[value_span.end as usize..declaration.end as usize]
            );
            if outside.contains("/*") || outside.contains("//") {
                continue;
            }
            self.edits.push(analysis::Edit::replace(
                self.whole_line_or_statement(pair[0].span()),
                deletion,
            ));
            self.edits
                .push(analysis::Edit::replace(use_identifier.span, replacement));
            self.inlined += 1;
            destinations.insert(pair[1].span());
        }
    }

    /// Record the immutable cells that reaching past `first` proves
    /// initialized: its declared bindings and each initializer's first read.
    fn record_initialized(&self, first: &Statement<'_>, initialized: &mut BTreeSet<SymbolId>) {
        let Statement::VariableDeclaration(d) = first else {
            return;
        };
        for binding in &d.declarations {
            // Reaching the next statement proves the initializer's
            // first local read succeeded, even for an enclosing block.
            // Restrict this to immutable lexical cells (no mapped args).
            if let Some(read) = binding.init.as_ref().and_then(first_evaluated_identifier) {
                if let Some(symbol) = analysis::identifier_symbol(read, self.scoping) {
                    if self
                        .scoping
                        .symbol_flags(symbol)
                        .contains(oxc_syntax::symbol::SymbolFlags::BlockScopedVariable)
                        && !self.protected.contains(self.scoping.symbol_name(symbol))
                        && self.scoping.symbol_redeclarations(symbol).is_empty()
                        && self
                            .scoping
                            .get_resolved_references(symbol)
                            .all(|r| !r.is_write())
                    {
                        initialized.insert(symbol);
                    }
                }
            }
            if let BindingPattern::BindingIdentifier(id) = &binding.id {
                if let Some(symbol) = id.symbol_id.get() {
                    if self.scoping.symbol_span(symbol) == id.span
                        && !self.protected.contains(self.scoping.symbol_name(symbol))
                        && self.scoping.symbol_redeclarations(symbol).is_empty()
                        && self
                            .scoping
                            .get_resolved_references(symbol)
                            .all(|r| !r.is_write())
                    {
                        initialized.insert(symbol);
                    }
                }
            }
        }
    }

    /// The store `first` makes that may be inlined, its value, and the text
    /// left in its place: a thread-mode plain assignment to an initialized
    /// mutable local, or a single generated (or owned local) declaration.
    fn inlined_store<'a, 'b>(
        &self,
        first: &'b Statement<'a>,
    ) -> Option<(SymbolId, &'b Expression<'a>, String)> {
        let assignment = if self.native_calls {
            if let Statement::ExpressionStatement(s) = first {
                if let Expression::AssignmentExpression(a) = &s.expression {
                    if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
                        (a.operator == AssignmentOperator::Assign)
                            .then(|| analysis::identifier_symbol(id, self.scoping))
                            .flatten()
                            .map(|symbol| (symbol, &a.right))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        if let Some((symbol, initializer)) = assignment {
            if self.protected.contains(self.scoping.symbol_name(symbol))
                || !self.local_reads.initialized(symbol, first.span())
                || self.scoping.symbol_flags(symbol).is_const_variable()
            {
                return None;
            }
            return Some((symbol, initializer, String::new()));
        }
        let Statement::VariableDeclaration(declaration) = first else {
            return None;
        };
        let lexical = matches!(
            declaration.kind,
            VariableDeclarationKind::Let | VariableDeclarationKind::Const
        );
        if !(lexical || (self.native_calls && declaration.kind == VariableDeclarationKind::Var))
            || declaration.declarations.len() != 1
        {
            return None;
        }
        let binding = &declaration.declarations[0];
        let BindingPattern::BindingIdentifier(name) = &binding.id else {
            return None;
        };
        let local_slot = self.native_calls
            && !self.protected.contains(name.name.as_str())
            && name.symbol_id.get().is_some_and(|s| {
                self.local_reads
                    .initialized(s, Span::new(declaration.span.end, declaration.span.end))
                    && self.scoping.symbol_span(s) == name.span
                    && self.scoping.symbol_redeclarations(s).is_empty()
            });
        if !(name.name.starts_with("$SSA_") || name.name.starts_with("$CARRY_") || local_slot) {
            return None;
        }
        let (Some(symbol), Some(initializer)) = (name.symbol_id.get(), binding.init.as_ref())
        else {
            return None;
        };
        let deletion = if local_slot && declaration.kind != VariableDeclarationKind::Const {
            format!("{} {};", declaration.kind.as_str(), name.name)
        } else {
            String::new()
        };
        Some((symbol, initializer, deletion))
    }

    /// The cells the moved initializer may be read across, and the policy
    /// for proving the use's evaluated prefix.
    fn initializer_reads(
        &self,
        initializer: &Expression<'_>,
        initialized: &BTreeSet<SymbolId>,
    ) -> (BTreeSet<SymbolId>, ReadPolicy<'_>) {
        let mut readable = initialized.clone();
        self.local_reads.readable(initializer.span(), &mut readable);
        let commuting = self
            .native_calls
            .then(|| {
                self.local_reads
                    .commuting_read(initializer, self.scoping, self.protected)
            })
            .flatten();
        if commuting.is_some() {
            self.local_reads
                .plain_reads(initializer.span(), &mut readable);
        }
        let policy = ReadPolicy {
            commuting_initialized_read: commuting == Some(true),
            primitive_inputs: &self.local_reads.primitive_inputs,
            cells: self.cells,
            ordinary_number: self.local_reads.ordinary_number,
            this_initialized: self.native_calls
                && self
                    .local_reads
                    .classes
                    .enclosing(initializer.span())
                    .is_none(),
        };
        (readable, policy)
    }

    /// The read of `symbol` that `second` evaluates first: a leading read,
    /// an argument after an immutable callee, or a read after a proven prefix.
    fn first_use<'a, 'b>(
        &self,
        second: &'b Statement<'a>,
        symbol: SymbolId,
        initializer: &Expression<'a>,
        readable: &BTreeSet<SymbolId>,
        policy: ReadPolicy<'_>,
    ) -> Option<&'b IdentifierReference<'a>> {
        match second {
            Statement::ReturnStatement(statement) => statement
                .argument
                .as_ref()
                .and_then(first_evaluated_identifier),
            Statement::ThrowStatement(statement) => {
                first_evaluated_identifier(&statement.argument)
            }
            Statement::IfStatement(statement) => first_evaluated_identifier(&statement.test),
            Statement::ExpressionStatement(statement) => prefix_read(
                &statement.expression,
                symbol,
                self.scoping,
                readable,
                self.native_calls,
                self.protected,
                policy,
            )
            .ok()
            .flatten(),
            Statement::VariableDeclaration(declaration)
                if declaration.declarations.len() == 1
                    && matches!(
                        declaration.declarations[0].id,
                        BindingPattern::BindingIdentifier(_)
                    ) =>
            {
                declaration.declarations[0]
                    .init
                    .as_ref()
                    .and_then(|expression| {
                        first_evaluated_identifier(expression)
                            .filter(|id| {
                                analysis::identifier_symbol(id, self.scoping) == Some(symbol)
                            })
                            .or_else(|| {
                                argument_after_immutable_callee(
                                    expression,
                                    initializer,
                                    self.scoping,
                                    symbol,
                                )
                            })
                    })
            }
            _ => None,
        }
        .filter(|id| analysis::identifier_symbol(id, self.scoping) == Some(symbol))
        .or_else(|| {
            let expression = match second {
                Statement::ReturnStatement(s) => s.argument.as_ref(),
                Statement::ThrowStatement(s) => Some(&s.argument),
                Statement::IfStatement(s) => Some(&s.test),
                Statement::SwitchStatement(s) if self.native_calls => Some(&s.discriminant),
                Statement::ForInStatement(s) if self.native_calls => Some(&s.right),
                Statement::ForOfStatement(s) if self.native_calls => Some(&s.right),
                Statement::ForStatement(s) if self.native_calls => {
                    s.init.as_ref().and_then(|init| {
                        if let ForStatementInit::VariableDeclaration(d) = init {
                            d.declarations.first().and_then(|d| d.init.as_ref())
                        } else {
                            init.as_expression()
                        }
                    })
                }
                Statement::ExpressionStatement(s) => Some(&s.expression),
                Statement::VariableDeclaration(d) if self.native_calls => {
                    d.declarations.first().and_then(|d| d.init.as_ref())
                }
                Statement::VariableDeclaration(d)
                    if d.declarations.len() == 1
                        && matches!(
                            d.declarations[0].id,
                            BindingPattern::BindingIdentifier(_)
                        ) =>
                {
                    d.declarations[0].init.as_ref()
                }
                _ => None,
            }?;
            prefix_read(
                expression,
                symbol,
                self.scoping,
                readable,
                self.native_calls,
                self.protected,
                policy,
            )
            .ok()
            .flatten()
        })
    }

    /// Whether every name in the initializer resolves to the same binding at
    /// the use as it does in place.
    fn fits_inserted_scope(
        &self,
        initializer: &Expression<'_>,
        use_identifier: &IdentifierReference<'_>,
    ) -> bool {
        let Some(reference) = use_identifier.reference_id.get() else {
            return false;
        };
        let mut scope = InsertedScope {
            scoping: self.scoping,
            scope: self.scoping.get_reference(reference).scope_id(),
            initializer: initializer.span(),
            valid: true,
        };
        scope.visit_expression(initializer);
        scope.valid
    }

    /// Start of the next plain store to `symbol` after `second`, which ends
    /// the version `second` reads.
    fn version_end(
        &self,
        executable: &[&Statement<'_>],
        second: &Statement<'_>,
        symbol: SymbolId,
    ) -> u32 {
        executable
            .iter()
            .skip_while(|s| s.span() != second.span())
            .skip(1)
            .find_map(|s| {
                let Statement::ExpressionStatement(s) = s else {
                    return None;
                };
                let Expression::AssignmentExpression(a) = &s.expression else {
                    return None;
                };
                let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
                    return None;
                };
                (a.operator == AssignmentOperator::Assign
                    && analysis::identifier_symbol(id, self.scoping) == Some(symbol))
                .then_some(s.span.start)
            })
            .unwrap_or(u32::MAX)
    }

    /// Whether the stored value has exactly one reference, a read: among all
    /// of the cell's references, or for a `local_version` among those that
    /// can observe this stored version.
    fn single_read_of_version(
        &self,
        statements: &[Statement<'_>],
        executable: &[&Statement<'_>],
        pair: [&Statement<'_>; 2],
        symbol: SymbolId,
        initializer: &Expression<'_>,
        local_version: bool,
    ) -> bool {
        let version_end = if local_version {
            self.version_end(executable, pair[1], symbol)
        } else {
            u32::MAX
        };
        // This particular stored version can end in a branch even when
        // the same cell is read for other versions elsewhere. The backward
        // proof includes loop backedges and declines exception owners.
        let dead_version = local_version
            && self.local_reads.private_cell(symbol)
            && self.liveness.dead_after(pair[1].span(), symbol)
            && self.scoping.get_resolved_references(symbol).all(|r| {
                let span = self.nodes.get_node(r.node_id()).kind().span();
                self.local_reads.same_owner(initializer.span(), span)
                    && !(r.is_read()
                        && span.start >= pair[0].span().start
                        && span.end <= pair[0].span().end)
            });
        if local_version
            && !dead_version
            && self.scoping.get_resolved_references(symbol).any(|r| {
                let span = self.nodes.get_node(r.node_id()).kind().span();
                self.local_reads.regions.enclosing(span)
                != self.local_reads.regions.enclosing(initializer.span())
                || (span.start>=pair[0].span().start &&
                    (span.start < statements.first().unwrap().span().start
                        || span.end > statements.last().unwrap().span().end))
                // A textual earlier read can observe this store on the
                // next iteration. Outside loops it cannot observe a future
                // value merely because it is in an enclosing block.
                || (span.start<pair[0].span().start && r.is_read()
                    && self.local_reads.loops.enclosing(pair[0].span()).is_some())
                || (r.is_read() && span.start >= pair[0].span().start
                    && span.end <= pair[0].span().end)
                // The next store's RHS can still read the old version.
                || (span.start >= version_end && r.is_read()
                    && executable.iter().any(|s| s.span().start == version_end
                        && span.end <= s.span().end))
            })
        {
            return false;
        }
        let mut references = self.scoping.get_resolved_references(symbol).filter(|r| {
            if !local_version {
                return true;
            }
            let span = self.nodes.get_node(r.node_id()).kind().span();
            if dead_version {
                span.start >= pair[1].span().start && span.end <= pair[1].span().end
            } else {
                span.start >= pair[0].span().end && span.start < version_end
            }
        });
        let Some(only_reference) = references.next() else {
            return false;
        };
        !(references.next().is_some() || only_reference.is_write())
    }

    /// The parenthesized value for the use, as `(0, value)` where a direct
    /// call or `typeof` of the bare name differs from the parenthesized form.
    fn replacement(
        &self,
        second: &Statement<'_>,
        use_identifier: &IdentifierReference<'_>,
        named_initializer: &Expression<'_>,
        value: &str,
    ) -> String {
        let mut direct_callees = DirectCallCallees::default();
        direct_callees.visit_statement(second);
        if (direct_callees.spans.contains(&use_identifier.span)
            && (matches!(
                named_initializer,
                Expression::StaticMemberExpression(_)
                    | Expression::ComputedMemberExpression(_)
                    | Expression::PrivateFieldExpression(_)
                    | Expression::ChainExpression(_)
            ) || matches!(named_initializer, Expression::Identifier(id) if id.name == "eval" && analysis::identifier_symbol(id,self.scoping).is_none())))
            || (direct_callees
                .typeof_operands
                .contains(&use_identifier.span)
                && matches!(named_initializer, Expression::Identifier(id) if analysis::identifier_symbol(id,self.scoping).is_none()))
        {
            // `let tmp = obj.method; tmp()` calls with the helper's bare
            // `this`, while `(obj.method)()` would call on `obj`.
            format!("(0, {value})")
        } else {
            format!("({value})")
        }
    }

    pub(super) fn whole_line_or_statement(&self, span: Span) -> Span {
        super::super::super::support::whole_line(self.source, span)
    }
}
