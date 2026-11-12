//! Merging a value derived from another into the reads that need it.

use super::*;

impl<'s, 'a> Merger<'s, 'a> {
    pub(super) fn candidate(&self, symbol: SymbolId) -> Option<Candidate<'s, 'a>> {
        if self.scoping.symbol_declarations(symbol).count() != 1 {
            return None;
        }
        let declarator_id = self.scoping.symbol_declaration(symbol);
        let AstKind::VariableDeclarator(declarator) = self.nodes.kind(declarator_id) else {
            return None;
        };
        let BindingPattern::BindingIdentifier(_) = &declarator.id else {
            return None;
        };
        let init = declarator.init.as_ref();
        let statement = self.nodes.parent_id(declarator_id);
        let AstKind::VariableDeclaration(declaration) = self.nodes.kind(statement) else {
            return None;
        };
        // Several names share a declaration only when none is initialized.
        if declaration.kind == VariableDeclarationKind::Var
            || (declaration.declarations.len() != 1 && init.is_some())
        {
            return None;
        }
        if !matches!(
            self.nodes.parent_kind(statement),
            AstKind::BlockStatement(_) | AstKind::FunctionBody(_) | AstKind::Program(_)
        ) {
            return None;
        }
        let function = self.function_of(statement);
        let mut reads = Vec::new();
        let mut writes = Vec::new();
        for reference in self.scoping.get_resolved_references(symbol) {
            if self.function_of(reference.node_id()) != function {
                return None;
            }
            if reference.is_write() {
                // Only a whole `r = v;` statement, which the walk can start
                // after.
                if reference.is_read() {
                    return None;
                }
                let assignment = self.nodes.parent_id(reference.node_id());
                let AstKind::AssignmentExpression(store) = self.nodes.kind(assignment) else {
                    return None;
                };
                let statement = self.nodes.parent_id(assignment);
                if store.operator != AssignmentOperator::Assign
                    || !matches!(self.nodes.kind(statement), AstKind::ExpressionStatement(_))
                {
                    return None;
                }
                writes.push((statement, reference.node_id()));
                continue;
            }
            if let AstKind::ObjectProperty(property) = self.nodes.parent_kind(reference.node_id()) {
                if property.shorthand {
                    return None;
                }
            }
            reads.push(reference.node_id());
        }
        if reads.is_empty() {
            return None;
        }
        if init.is_none() && !self.assigned_before_reads(statement, &writes, &reads) {
            return None;
        }
        let removal = if declaration.declarations.len() == 1 {
            whole_line(self.source, declaration.span)
        } else {
            let at = declaration
                .declarations
                .iter()
                .position(|other| other.span == declarator.span)?;
            match declaration.declarations.get(at + 1) {
                Some(next) => Span::new(declarator.span.start, next.span.start),
                None => Span::new(declaration.declarations[at - 1].span.end, declarator.span.end),
            }
        };
        Some(Candidate {
            removal,
            symbol,
            statement,
            statement_span: declaration.span,
            init,
            function,
            reads,
            writes,
        })
    }

    /// `let r;` is followed by a statement that stores `r` on every path,
    /// and each read comes after it or after a store in its own block.
    pub(super) fn assigned_before_reads(&self, statement: NodeId, writes: &[(NodeId, NodeId)], reads: &[NodeId]) -> bool {
        let list: &[Statement<'_>] = match self.nodes.parent_kind(statement) {
            AstKind::BlockStatement(block) => &block.body,
            AstKind::FunctionBody(body) => &body.statements,
            AstKind::Program(program) => &program.body,
            _ => return false,
        };
        let span = self.span_of(statement);
        let Some(at) = list.iter().position(|s| s.span() == span) else {
            return false;
        };
        let Some(next) = list.get(at + 1) else {
            return false;
        };
        let stores: Vec<Span> = writes.iter().map(|(store, _)| self.span_of(*store)).collect();
        if !definitely_stores(next, &stores) {
            return false;
        }
        let next_span = next.span();
        reads.iter().all(|read| {
            let read_span = self.span_of(*read);
            if read_span.start >= next_span.end {
                return true;
            }
            // Inside: an earlier statement of an enclosing block stores it.
            let mut child = *read;
            for ancestor in self.nodes.ancestor_ids(*read) {
                if let AstKind::BlockStatement(block) = self.nodes.kind(ancestor) {
                    let child_span = self.span_of(child);
                    if block
                        .body
                        .iter()
                        .take_while(|s| s.span().end <= child_span.start)
                        .any(|s| stores.contains(&s.span()))
                    {
                        return true;
                    }
                }
                if ancestor == statement || self.span_of(ancestor) == next_span {
                    break;
                }
                child = ancestor;
            }
            false
        })
    }

    /// Locals the value reads, in source order: the initializer, or the
    /// right sides of a `let r;`'s stores.
    pub(super) fn sources(&self, candidate: &Candidate<'_, '_>) -> Vec<SymbolId> {
        let mut found = Vec::new();
        let mut collect = Identifiers::default();
        match candidate.init {
            Some(init) => collect.visit_expression(init),
            None => {
                for (statement, _) in &candidate.writes {
                    if let AstKind::ExpressionStatement(store) = self.nodes.kind(*statement) {
                        if let Expression::AssignmentExpression(assignment) = &store.expression {
                            collect.visit_expression(&assignment.right);
                        }
                    }
                }
            }
        }
        for reference in collect.references {
            let Some(symbol) = self.scoping.get_reference(reference).symbol_id() else {
                continue;
            };
            if !found.contains(&symbol) {
                found.push(symbol);
            }
        }
        // A local the value is later copied into (`L = r;`) can hold it from
        // the start.
        for read in &candidate.reads {
            if let AstKind::AssignmentExpression(assignment) = self.nodes.parent_kind(*read) {
                if let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left {
                    if assignment.operator == AssignmentOperator::Assign && assignment.right.span() == self.span_of(*read) {
                        if let Some(symbol) = self.scoping.get_reference(target.reference_id()).symbol_id() {
                            if !found.contains(&symbol) {
                                found.push(symbol);
                            }
                        }
                    }
                }
            }
        }
        // Walking an object (`x = x.p`, `x = x?.p.q`) derives from it.
        let values: Vec<&Expression<'_>> = match candidate.init {
            Some(init) => vec![init],
            None => candidate
                .writes
                .iter()
                .filter_map(|(statement, _)| match self.nodes.kind(*statement) {
                    AstKind::ExpressionStatement(store) => match &store.expression {
                        Expression::AssignmentExpression(assignment) => Some(&assignment.right),
                        _ => None,
                    },
                    _ => None,
                })
                .collect(),
        };
        for value in values {
            if let Some(root) = path_root(value) {
                if let Some(symbol) = self.scoping.get_reference(root.reference_id()).symbol_id() {
                    if !found.contains(&symbol) {
                        found.push(symbol);
                    }
                }
            }
        }
        // Literal holders keep their objects.
        found.retain(|symbol| {
            let name = self.scoping.symbol_name(*symbol);
            !(name.starts_with("$OBJECT_") || name.starts_with("$ARRAY_") || name.starts_with("$THREAD_"))
        });
        found
    }

    pub(super) fn merge(&self, candidate: &Candidate<'_, '_>, local: SymbolId) -> Option<Vec<analysis::Edit>> {
        self.local_fits(candidate, local)?;
        let (read_starts, writes) = self.local_references(candidate, local)?;
        let init = candidate.init.map_or(Span::new(0, 0), GetSpan::span);
        let walk = Walk {
            merger: self,
            local,
            reads: &read_starts,
        };
        // The initializer itself must not store the local.
        if writes.iter().any(|write| contains(init, self.span_of(*write))) {
            return None;
        }
        let reads: Vec<Span> = candidate.reads.iter().map(|read| self.span_of(*read)).collect();
        if !self.local_keeps_derived(candidate, &writes, &reads) {
            return None;
        }
        // A copy holds the same value as the local until either is stored;
        // anything else needs the old value dead at once.
        let copy = candidate.init.is_some_and(|init| self.is_local(init, local));
        if candidate.init.is_some() && !copy && walk.live_after(candidate.statement) {
            return None;
        }
        // After each later store to the derived name the old value is dead;
        // storing a copy of the local changes nothing once merged.
        if candidate.writes.iter().any(|(statement, _)| {
            !self.stores_local(*statement, local) && walk.live_after(*statement)
        }) {
            return None;
        }
        // A local the value is only copied into (`L = r;`), and that nothing
        // reads after any copy, does not hold the value for anyone: its
        // copies are dead stores. They go, and the value keeps its own name.
        if !self.derives_from(candidate, local) {
            let copies: Vec<(NodeId, Span)> = candidate
                .reads
                .iter()
                .filter_map(|read| {
                    self.copy_back(*read, local)
                        .map(|span| (self.nodes.parent_id(self.nodes.parent_id(*read)), span))
                })
                .collect();
            if !copies.is_empty() && copies.iter().all(|(statement, _)| !walk.live_after(*statement)) {
                return Some(
                    copies
                        .into_iter()
                        .map(|(_, span)| analysis::Edit::delete(whole_line(self.source, span)))
                        .collect(),
                );
            }
        }
        let name = self.scoping.symbol_name(local);
        if !self.name_reaches(candidate, local, name) {
            return None;
        }
        Some(self.merge_edits(candidate, local, name, copy, init, &reads))
    }

    /// A non-const variable or parameter of the candidate's function, or an
    /// outer scratch slot; a parameter only where `arguments` is never named.
    fn local_fits(&self, candidate: &Candidate<'_, '_>, local: SymbolId) -> Option<()> {
        let flags = self.scoping.symbol_flags(local);
        if flags.contains(SymbolFlags::ConstVariable)
            || flags.contains(SymbolFlags::Function)
            || flags.contains(SymbolFlags::Class)
            || self.scoping.symbol_declarations(local).count() != 1
        {
            return None;
        }
        let declaration = self.scoping.symbol_declaration(local);
        match self.nodes.kind(declaration) {
            AstKind::VariableDeclarator(declarator) => {
                if !matches!(declarator.id, BindingPattern::BindingIdentifier(_)) {
                    return None;
                }
                // An outer scratch slot only this function uses, and always
                // stores before reading, starts fresh at every call.
                if self.function_of(declaration) != candidate.function
                    && !self.scratch_slot(local, candidate.function?)
                {
                    return None;
                }
            }
            AstKind::FormalParameter(_) => {
                let function = candidate.function?;
                if self.nodes.parent_id(self.nodes.parent_id(declaration)) != function {
                    return None;
                }
                let span = self.nodes.kind(function).span();
                // The function's text names `arguments` somewhere.
                let at = self.arguments_at.partition_point(|at| *at < span.start);
                if self.arguments_at.get(at).is_some_and(|at| *at as usize + "arguments".len() <= span.end as usize) {
                    return None;
                }
            }
            _ => return None,
        }
        Some(())
    }

    /// The sorted read starts and the write nodes of `local`, all of them in
    /// the candidate's function.
    fn local_references(
        &self,
        candidate: &Candidate<'_, '_>,
        local: SymbolId,
    ) -> Option<(Vec<u32>, Vec<NodeId>)> {
        let mut read_starts = Vec::new();
        let mut writes = Vec::new();
        // Binding initializers do not occur in resolved references, but they
        // overwrite the local just like assignment expressions. In particular,
        // a later `var L = field` must not clobber a buffer merged into L.
        let declaration = self.scoping.symbol_declaration(local);
        if matches!(self.nodes.kind(declaration), AstKind::VariableDeclarator(d) if d.init.is_some())
            && self.function_of(declaration) == candidate.function
        {
            writes.push(declaration);
        }
        for reference in self.scoping.get_resolved_references(local) {
            let node = reference.node_id();
            if self.function_of(node) != candidate.function {
                return None;
            }
            let span = self.span_of(node);
            if reference.is_read() {
                read_starts.push(span.start);
            }
            if reference.is_write() {
                writes.push(node);
            }
        }
        read_starts.sort_unstable();
        Some((read_starts, writes))
    }

    /// The local keeps the derived value until the derived value's last
    /// read: after a store to the local, no path reaches a read of the
    /// derived value before the derived value is stored again.
    fn local_keeps_derived(&self, candidate: &Candidate<'_, '_>, writes: &[NodeId], reads: &[Span]) -> bool {
        let mut derived_reads: Vec<u32> = reads.iter().map(|read| read.start).collect();
        derived_reads.sort_unstable();
        let derived_walk = Walk {
            merger: self,
            local: candidate.symbol,
            reads: &derived_reads,
        };
        let mut textual_writes = Vec::new();
        for write in writes {
            let at = self.write_at(*write);
            if at < candidate.statement_span.end {
                continue;
            }
            // `L = r;` leaves the local holding the derived value.
            if matches!(self.nodes.parent_kind(*write), AstKind::AssignmentExpression(assignment)
                if assignment.operator == AssignmentOperator::Assign
                    && matches!(assignment.right.without_parentheses(), Expression::Identifier(identifier)
                        if self.scoping.get_reference(identifier.reference_id()).symbol_id() == Some(candidate.symbol)))
            {
                continue;
            }
            if let Some(statement) = self.store_statement(*write) {
                if derived_walk.live_after(statement) {
                    return false;
                }
                continue;
            }
            textual_writes.push(*write);
            if reads.iter().any(|read| read.start > at) {
                return false;
            }
        }
        let writes = textual_writes;
        for read in &candidate.reads {
            for ancestor in self.nodes.ancestor_ids(*read) {
                if ancestor == candidate.statement || Some(ancestor) == candidate.function {
                    break;
                }
                let kind = self.nodes.kind(ancestor);
                if is_loop(&kind) {
                    let span = kind.span();
                    if span.start >= candidate.statement_span.end
                        && writes.iter().any(|write| contains(span, self.span_of(*write)))
                    {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// The local's name must reach every reference of the derived value,
    /// and its declaration must already have run where the value is made.
    fn name_reaches(&self, candidate: &Candidate<'_, '_>, local: SymbolId, name: &str) -> bool {
        let statement_scope = self.nodes.get_node(candidate.statement).scope_id();
        if self.scoping.find_binding(statement_scope, name.into()) != Some(local)
            || (!self.scoping.symbol_flags(local).contains(SymbolFlags::FunctionScopedVariable)
                && self.span_of(self.scoping.symbol_declaration(local)).start >= candidate.statement_span.start)
        {
            return false;
        }
        for read in candidate.reads.iter().chain(candidate.writes.iter().map(|(_, target)| target)) {
            let scope = self.nodes.get_node(*read).scope_id();
            if self.scoping.find_binding(scope, name.into()) != Some(local) {
                return false;
            }
        }
        true
    }

    /// Rename the derived value's stores and reads to the local, dropping the
    /// copies that become `L = L;`, and store the initializer into the local.
    fn merge_edits(
        &self,
        candidate: &Candidate<'_, '_>,
        local: SymbolId,
        name: &str,
        copy: bool,
        init: Span,
        reads: &[Span],
    ) -> Vec<analysis::Edit> {
        let mut edits = Vec::new();
        // `L = L;` is nothing: a copy's declaration, `r = L;`, and `L = r;`
        // go.
        for (statement, target) in &candidate.writes {
            let AstKind::ExpressionStatement(store) = self.nodes.kind(*statement) else {
                continue;
            };
            let Expression::AssignmentExpression(assignment) = &store.expression else {
                continue;
            };
            let _ = assignment;
            if self.stores_local(*statement, local) {
                edits.push(analysis::Edit::delete(whole_line(self.source, store.span)));
            } else {
                edits.push(analysis::Edit::replace(self.span_of(*target), name.to_string()));
            }
        }
        let mut gone = Vec::new();
        for read in &candidate.reads {
            if let Some(statement) = self.copy_back(*read, local) {
                gone.push(self.span_of(*read));
                edits.push(analysis::Edit::delete(whole_line(self.source, statement)));
            }
        }
        if copy || candidate.init.is_none() {
            edits.push(analysis::Edit::delete(candidate.removal));
            for read in reads {
                if !gone.contains(read) {
                    edits.push(analysis::Edit::replace(*read, name.to_string()));
                }
            }
            return edits;
        }
        // Rewrite only around the initializer, so reads inside it can merge
        // in the same call.
        let sequence = matches!(
            candidate.init.map(Expression::without_parentheses),
            Some(Expression::SequenceExpression(_))
        );
        let open = if sequence { "(" } else { "" };
        edits.push(analysis::Edit::replace(
            Span::new(candidate.statement_span.start, init.start),
            format!("{name} = {open}"),
        ));
        if sequence {
            edits.push(analysis::Edit::replace(Span::new(init.end, init.end), ")"));
        }
        for read in reads {
            if !gone.contains(read) {
                edits.push(analysis::Edit::replace(*read, name.to_string()));
            }
        }
        let _ = candidate.symbol;
        edits
    }

    /// `r = L;`
    pub(super) fn stores_local(&self, statement: NodeId, local: SymbolId) -> bool {
        matches!(self.nodes.kind(statement), AstKind::ExpressionStatement(store)
            if matches!(&store.expression, Expression::AssignmentExpression(assignment)
                if self.is_local(&assignment.right, local)))
    }

    /// `local` is declared outside `function`, which holds every reference
    /// and never reads it before storing it: no value crosses calls.
    pub(super) fn scratch_slot(&self, local: SymbolId, function: NodeId) -> bool {
        if self
            .scoping
            .get_resolved_references(local)
            .any(|reference| self.function_of(reference.node_id()) != Some(function))
        {
            return false;
        }
        let body = match self.nodes.kind(function) {
            AstKind::Function(function) => function.body.as_ref().map(|body| &body.statements),
            _ => None,
        };
        let Some(statements) = body else {
            return false;
        };
        let mut reads: Vec<u32> = self
            .scoping
            .get_resolved_references(local)
            .filter(|reference| reference.is_read())
            .map(|reference| self.span_of(reference.node_id()).start)
            .collect();
        reads.sort_unstable();
        let walk = Walk {
            merger: self,
            local,
            reads: &reads,
        };
        !walk.sequence(statements).read
    }

    pub(super) fn is_local(&self, expression: &Expression<'_>, local: SymbolId) -> bool {
        matches!(expression.without_parentheses(), Expression::Identifier(identifier)
            if self.scoping.get_reference(identifier.reference_id()).symbol_id() == Some(local))
    }

    /// Whether the candidate's value (its initializer or a later store's
    /// right side) mentions `local`, or walks an object it holds.
    fn derives_from(&self, candidate: &Candidate<'_, '_>, local: SymbolId) -> bool {
        let values: Vec<&Expression<'_>> = match candidate.init {
            Some(init) => vec![init],
            None => candidate
                .writes
                .iter()
                .filter_map(|(statement, _)| match self.nodes.kind(*statement) {
                    AstKind::ExpressionStatement(store) => match &store.expression {
                        Expression::AssignmentExpression(assignment) => Some(&assignment.right),
                        _ => None,
                    },
                    _ => None,
                })
                .collect(),
        };
        values.into_iter().any(|value| {
            let mut collect = Identifiers::default();
            collect.visit_expression(value);
            collect
                .references
                .iter()
                .chain(path_root(value).map(|root| root.reference_id()).as_ref())
                .any(|reference| self.scoping.get_reference(*reference).symbol_id() == Some(local))
        })
    }

    /// The `L = r;` statement whose right side is `read`.
    pub(super) fn copy_back(&self, read: NodeId, local: SymbolId) -> Option<Span> {
        let AstKind::AssignmentExpression(assignment) = self.nodes.parent_kind(read) else {
            return None;
        };
        if assignment.operator != AssignmentOperator::Assign || assignment.right.span() != self.span_of(read) {
            return None;
        }
        let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
            return None;
        };
        if self.scoping.get_reference(target.reference_id()).symbol_id() != Some(local) {
            return None;
        }
        let statement = self.nodes.parent_id(self.nodes.parent_id(read));
        match self.nodes.kind(statement) {
            AstKind::ExpressionStatement(store) => Some(store.span),
            _ => None,
        }
    }

    /// The `L = v;` statement a write reference stands in.
    pub(super) fn store_statement(&self, reference: NodeId) -> Option<NodeId> {
        let assignment = self.nodes.parent_id(reference);
        let AstKind::AssignmentExpression(store) = self.nodes.kind(assignment) else {
            return None;
        };
        let statement = self.nodes.parent_id(assignment);
        (store.operator == AssignmentOperator::Assign
            && matches!(self.nodes.kind(statement), AstKind::ExpressionStatement(_)))
        .then_some(statement)
    }

    /// Where a store takes effect: after its right-hand side.
    pub(super) fn write_at(&self, reference: NodeId) -> u32 {
        match self.nodes.parent_kind(reference) {
            AstKind::AssignmentExpression(assignment) => assignment.span.end,
            AstKind::UpdateExpression(update) => update.span.end,
            _ => self.span_of(reference).end,
        }
    }

    pub(super) fn inside_try_block(&self, node: NodeId) -> bool {
        let mut previous = node;
        for ancestor in self.nodes.ancestor_ids(node) {
            match self.nodes.kind(ancestor) {
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => return false,
                AstKind::TryStatement(statement) => {
                    if contains(statement.block.span, self.span_of(previous)) {
                        return true;
                    }
                }
                _ => {}
            }
            previous = ancestor;
        }
        false
    }

    pub(super) fn function_of(&self, node: NodeId) -> Option<NodeId> {
        super::super::super::support::function_of(self.nodes, node)
    }

    pub(super) fn span_of(&self, node: NodeId) -> Span {
        self.nodes.kind(node).span()
    }
}
