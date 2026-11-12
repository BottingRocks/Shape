//! Which reads of a slot are local to a statement list, by version.

use super::*;

#[derive(Default)]
pub(crate) struct LocalReads {
    pub(super) regions: SpanIndex,
    pub(super) loops: SpanIndex,
    pub(super) classes: SpanIndex,
    pub(super) parameters: SpanIndex,
    pub(super) exceptions: SpanIndex,
    pub(super) lexical: BTreeMap<SymbolId, Span>,
    pub(super) locals: BTreeMap<SymbolId, (Span, Span, Vec<Span>)>,
    pub(super) plain: BTreeMap<SymbolId, (Span, Span)>,
    pub(super) numeric_inputs: BTreeSet<SymbolId>,
    pub(super) primitive_inputs: BTreeSet<SymbolId>,
    pub(super) iteration_bindings: BTreeSet<SymbolId>,
    pub(super) ordinary_number: bool,
}
impl LocalReads {
    /// Calls cannot replace a cell whose writers all belong to its owner.
    /// This says nothing about mutations to an object stored in that cell.
    pub(crate) fn private_cell(&self, symbol: SymbolId) -> bool {
        self.locals.contains_key(&symbol)
    }
    pub(super) fn iteration_binding(&mut self, left: &ForStatementLeft<'_>) {
        if let ForStatementLeft::VariableDeclaration(d) = left {
            for item in &d.declarations {
                if let BindingPattern::BindingIdentifier(id) = &item.id {
                    if let Some(symbol) = id.symbol_id.get() {
                        // Iteration values are implicit stores absent from
                        // the declarator initializer and identifier writes.
                        self.iteration_bindings.insert(symbol);
                    }
                }
            }
        }
    }
    pub(crate) fn same_owner(&self, a: Span, b: Span) -> bool {
        self.regions.enclosing(a) == self.regions.enclosing(b)
    }
    pub(crate) fn primitive_cell(&self, symbol: SymbolId, at: Span) -> bool {
        self.primitive_inputs.contains(&symbol) && self.initialized(symbol, at)
    }
    pub(crate) fn numeric_cell(&self, symbol: SymbolId, at: Span) -> bool {
        self.numeric_inputs.contains(&symbol) && self.initialized(symbol, at)
    }
    pub(crate) fn numeric_cells(&self, at: Span) -> impl Iterator<Item = SymbolId> + '_ {
        self.numeric_inputs
            .iter()
            .copied()
            .filter(move |&symbol| self.initialized(symbol, at))
    }
    pub(crate) fn same_iteration(&self, a: Span, b: Span) -> bool {
        self.regions.enclosing(a) == self.regions.enclosing(b)
            && self.loops.enclosing(a) == self.loops.enclosing(b)
    }
    pub(crate) fn repeated_between(&self, from: Span, to: Span) -> Option<Span> {
        self.loops.repeated_between(from, to)
    }
    pub(super) fn declarations(&mut self, body: &[Statement<'_>], end: u32) {
        for s in body {
            if let Statement::VariableDeclaration(d) = s {
                if d.kind != VariableDeclarationKind::Var {
                    for item in &d.declarations {
                        if let BindingPattern::BindingIdentifier(id) = &item.id {
                            if let Some(symbol) = id.symbol_id.get() {
                                self.lexical.insert(symbol, Span::new(d.span.end, end));
                            }
                        }
                    }
                }
            }
        }
    }
    pub(crate) fn build(
        &mut self,
        scoping: &Scoping,
        nodes: &oxc_semantic::AstNodes<'_>,
        protected: &BTreeSet<String>,
    ) {
        self.regions.build();
        self.loops.build();
        self.classes.build();
        self.parameters.build();
        self.exceptions.build();
        self.record_cells(scoping, nodes, protected);
        // Stored Number/Boolean/null/undefined values coerce numerically
        // without hooks or BigInt/Symbol errors. Only later reads commute;
        // effectful initializers stay ordered. Prove that every store keeps
        // this category, including branch joins and loop backedges.
        // A skipped var initializer leaves undefined, which belongs to this
        // category too. Implicit Annex-B function stores must be excluded.
        let ordinary_number = nodes
            .iter()
            .find_map(|node| match node.kind() {
                AstKind::Program(program) => {
                    Some(super::super::super::numeric_coercions::ordinary_number(program, scoping))
                }
                _ => None,
            })
            .unwrap_or(false);
        self.ordinary_number = ordinary_number;
        // Greatest fixed point over all stores, rather than seeding only
        // acyclic definitions. Loop/join copies can depend on each other.
        // Every entry value and every store must preserve the category;
        // removing an unproved input invalidates all dependent cells too.
        let candidates = self.input_candidates(scoping, nodes);
        // The declarators of the candidates, in node order: each round checks
        // only these instead of walking every node again.
        let declarators: Vec<_> = nodes
            .iter()
            .filter_map(|node| {
                let AstKind::VariableDeclarator(declaration) = node.kind() else {
                    return None;
                };
                let BindingPattern::BindingIdentifier(id) = &declaration.id else {
                    return None;
                };
                let symbol = id.symbol_id.get()?;
                candidates.contains(&symbol).then_some((symbol, declaration))
            })
            .collect();
        self.numeric_inputs = candidates.clone();
        self.primitive_inputs = candidates;
        self.prune_inputs(&declarators, scoping, nodes, ordinary_number);
    }
    /// Record each owned variable's initialized region and owner, and as a
    /// private local the unprotected ones whose writes all stay in that owner.
    fn record_cells(
        &mut self,
        scoping: &Scoping,
        nodes: &oxc_semantic::AstNodes<'_>,
        protected: &BTreeSet<String>,
    ) {
        for symbol in scoping.symbol_ids() {
            if !scoping.symbol_flags(symbol).is_variable()
                || !scoping.symbol_redeclarations(symbol).is_empty()
                || self
                    .parameters
                    .enclosing(scoping.symbol_span(symbol))
                    .is_some()
            {
                continue;
            }
            let Some(owner) = self.regions.enclosing(scoping.symbol_span(symbol)) else {
                continue;
            };
            let initialized = if scoping
                .symbol_flags(symbol)
                .is_function_scoped_declaration()
            {
                owner
            } else {
                let Some(region) = self.lexical.get(&symbol) else {
                    continue;
                };
                *region
            };
            self.plain.insert(symbol, (initialized, owner));
            if protected.contains(scoping.symbol_name(symbol)) {
                continue;
            }
            let mut writes = Vec::new();
            let mut captured = false;
            for r in scoping
                .get_resolved_references(symbol)
                .filter(|r| r.is_write())
            {
                let span = nodes.get_node(r.node_id()).kind().span();
                if self.regions.enclosing(span) != Some(owner) {
                    captured = true;
                    break;
                }
                writes.push(span);
            }
            if !captured {
                self.locals.insert(symbol, (initialized, owner, writes));
            }
        }
    }
    /// The private single-declarator locals, other than iteration bindings
    /// and Annex-B function spellings, whose coercion category can be proved.
    fn input_candidates(
        &self,
        scoping: &Scoping,
        nodes: &oxc_semantic::AstNodes<'_>,
    ) -> BTreeSet<SymbolId> {
        let function_names: BTreeSet<_> = nodes
            .iter()
            .filter_map(|node| match node.kind() {
                AstKind::Function(f) => f.id.as_ref().map(|id| id.name.as_str()),
                _ => None,
            })
            .collect();
        self
            .locals
            .keys()
            .copied()
            .filter(|s| !self.iteration_bindings.contains(s))
            .filter(|s| {
                matches!(nodes.kind(scoping.symbol_declaration(*s)),
                AstKind::VariableDeclarator(d) if matches!(&d.id,
                    BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(*s)))
            })
            .filter(|s| {
                self.lexical.contains_key(s)
                    || scoping.symbol_flags(*s).is_function_scoped_declaration()
                        && !function_names.contains(scoping.symbol_name(*s))
            })
            .collect()
    }
    /// Remove the candidates with a store outside their category until no
    /// removal invalidates another.
    fn prune_inputs(
        &mut self,
        declarators: &[(SymbolId, &VariableDeclarator<'_>)],
        scoping: &Scoping,
        nodes: &oxc_semantic::AstNodes<'_>,
        ordinary_number: bool,
    ) {
        loop {
            let before = (self.numeric_inputs.len(), self.primitive_inputs.len());
            for &(symbol, declaration) in declarators {
                for primitive in [false, true] {
                    let inputs = if primitive {
                        &self.primitive_inputs
                    } else {
                        &self.numeric_inputs
                    };
                    if !inputs.contains(&symbol) {
                        continue;
                    }
                    let accepts = |e| {
                        if primitive {
                            successful_primitive_input(e, inputs, scoping, ordinary_number)
                        } else {
                            successful_numeric_input(e, inputs, scoping, ordinary_number)
                        }
                    };
                    let preserved = declaration.init.as_ref().is_none_or(accepts)
                        && scoping
                            .get_resolved_references(symbol)
                            .filter(|r| r.is_write())
                            .all(|r| match nodes.parent_kind(r.node_id()) {
                                AstKind::UpdateExpression(_) => true,
                                AstKind::AssignmentExpression(a)
                                    if a.operator == AssignmentOperator::Assign =>
                                {
                                    accepts(&a.right)
                                }
                                _ => false,
                            });
                    if !preserved {
                        if primitive {
                            self.primitive_inputs.remove(&symbol);
                        } else {
                            self.numeric_inputs.remove(&symbol);
                        }
                    }
                }
            }
            if before == (self.numeric_inputs.len(), self.primitive_inputs.len()) {
                break;
            }
        }
    }
    pub(super) fn readable(&self, at: Span, safe: &mut BTreeSet<SymbolId>) {
        let owner = self.regions.enclosing(at);
        for (symbol, (initialized, region, writes)) in &self.locals {
            if owner == Some(*region)
                && initialized.start <= at.start
                && at.end <= initialized.end
                && !writes
                    .iter()
                    .any(|w| at.start <= w.start && w.end <= at.end)
            {
                safe.insert(*symbol);
            }
        }
    }
    pub(crate) fn initialized(&self, symbol: SymbolId, at: Span) -> bool {
        self.locals
            .get(&symbol)
            .is_some_and(|(initialized, owner, _)| {
                self.regions.enclosing(at) == Some(*owner)
                    && initialized.start <= at.start
                    && at.end <= initialized.end
            })
    }
    pub(crate) fn plain_initialized(&self, symbol: SymbolId, at: Span) -> bool {
        self.plain.get(&symbol).is_some_and(|(initialized, owner)| {
            self.regions.enclosing(at) == Some(*owner)
                && initialized.start <= at.start
                && at.end <= initialized.end
        })
    }
    pub(super) fn commuting_read(
        &self,
        e: &Expression<'_>,
        scoping: &Scoping,
        protected: &BTreeSet<String>,
    ) -> Option<bool> {
        let mut e = e;
        while let Expression::ParenthesizedExpression(p) = e {
            e = &p.expression;
        }
        let Expression::Identifier(id) = e else {
            return None;
        };
        if analysis::identifier_symbol(id, scoping)
            .is_some_and(|s| self.plain_initialized(s, id.span))
        {
            Some(true)
        } else if analysis::identifier_symbol(id, scoping).is_none()
            && protected.contains(id.name.as_str())
        {
            Some(false)
        } else {
            None
        }
    }
    pub(super) fn plain_reads(&self, at: Span, safe: &mut BTreeSet<SymbolId>) {
        safe.extend(
            self.plain
                .keys()
                .copied()
                .filter(|&s| self.plain_initialized(s, at)),
        );
    }
}
impl<'a> Visit<'a> for LocalReads {
    fn visit_try_statement(&mut self, s: &TryStatement<'a>) {
        self.exceptions.push(s.span);
        walk::walk_try_statement(self, s);
    }
    fn visit_for_statement(&mut self, s: &ForStatement<'a>) {
        self.loops.push(s.span);
        if let Some(ForStatementInit::VariableDeclaration(declaration)) = &s.init {
            if declaration.kind != VariableDeclarationKind::Var {
                for item in &declaration.declarations {
                    if let BindingPattern::BindingIdentifier(id) = &item.id {
                        if let Some(symbol) = id.symbol_id.get() {
                            self.lexical
                                .insert(symbol, Span::new(declaration.span.end, s.span.end));
                        }
                    }
                }
            }
        }
        walk::walk_for_statement(self, s);
    }
    fn visit_while_statement(&mut self, s: &WhileStatement<'a>) {
        self.loops.push(s.span);
        walk::walk_while_statement(self, s);
    }
    fn visit_do_while_statement(&mut self, s: &DoWhileStatement<'a>) {
        self.loops.push(s.span);
        walk::walk_do_while_statement(self, s);
    }
    fn visit_for_in_statement(&mut self, s: &ForInStatement<'a>) {
        self.loops.push(s.span);
        self.iteration_binding(&s.left);
        walk::walk_for_in_statement(self, s);
    }
    fn visit_for_of_statement(&mut self, s: &ForOfStatement<'a>) {
        self.loops.push(s.span);
        self.iteration_binding(&s.left);
        walk::walk_for_of_statement(self, s);
    }
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        self.regions.push(f.span);
        self.parameters.push(f.params.span);
        walk::walk_function(self, f, flags);
    }
    fn visit_arrow_function_expression(&mut self, f: &ArrowFunctionExpression<'a>) {
        self.regions.push(f.span);
        self.parameters.push(f.params.span);
        walk::walk_arrow_function_expression(self, f);
    }
    fn visit_class(&mut self, c: &Class<'a>) {
        self.regions.push(c.span);
        self.classes.push(c.span);
        walk::walk_class(self, c);
    }
    fn visit_function_body(&mut self, b: &FunctionBody<'a>) {
        self.declarations(&b.statements, b.span.end);
        walk::walk_function_body(self, b);
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        self.declarations(&b.body, b.span.end);
        walk::walk_block_statement(self, b);
    }
}
