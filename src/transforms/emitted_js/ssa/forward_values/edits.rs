//! Writing the decisions as edits.

use super::*;

impl Plan<'_, '_> {
    /// Rule 9: reads of a tested boolean inside the arms it decides.
    pub(super) fn branch_booleans(&self, gone: &[Span]) -> Vec<(Span, String)> {
        let Some(types) = self.scan.types else {
            return Vec::new();
        };
        let mut out: Vec<(Span, String)> = Vec::new();
        for (reference, negated, yes, no, owner) in &self.scan.branches {
            let Some(symbol) = self.scoping.get_reference(*reference).symbol_id() else {
                continue;
            };
            if self.forward_of(&symbol).is_some()
                || self.scoping.symbol_is_mutated(symbol)
                || types.binding_type(symbol) != Some(super::super::super::primitives::Prim::Boolean)
            {
                continue;
            }
            for read in self.scoping.get_resolved_references(symbol) {
                if !read.is_read() || read.is_write() {
                    continue;
                }
                let node = read.node_id();
                let span = self.span_of(node);
                let value = if yes.start <= span.start && span.end <= yes.end {
                    !*negated
                } else if no.is_some_and(|no| no.start <= span.start && span.end <= no.end) {
                    *negated
                } else {
                    continue;
                };
                if self.owner_of(node) != *owner
                    || self.in_noop(span)
                    || gone.iter().any(|g| g.start <= span.start && span.end <= g.end)
                    || self.is_shorthand_or_delete(node)
                    || out.iter().any(|(other, _)| *other == span)
                {
                    continue;
                }
                out.push((span, value.to_string()));
            }
        }
        out
    }

    /// Rule 8: slot reads that see the stable value a store just wrote, with
    /// the text replacing each. Reads inside removed text are left alone.
    pub(super) fn store_loads(&self, gone: &[Span]) -> Vec<(Span, String)> {
        let mut out = Vec::new();
        for store in &self.scan.stores {
            let stable = match store.kind {
                InitKind::Identifier => store
                    .value_reference
                    .and_then(|reference| self.scoping.get_reference(reference).symbol_id())
                    .is_some_and(|symbol| {
                        symbol != store.slot
                            && !self.forward_of(&symbol).is_some()
                            && self.is_stable(symbol)
                            && self.declared_before(symbol, store.statement)
                    }),
                InitKind::Computed => false,
                _ => true,
            };
            if !stable || self.references_in(store.value).iter().any(|(reference, _)| {
                self.scoping
                    .get_reference(*reference)
                    .symbol_id()
                    .is_some_and(|symbol| self.forward_of(&symbol).is_some())
            }) {
                continue;
            }
            let list = &self.scan.lists[store.list];
            let owner = list[store.position].owner;
            let later = Span::new(store.statement.end, list.last().map_or(store.statement.end, |s| s.span.end));
            // The next store of the slot after this one, anywhere.
            let next_write = self
                .scoping
                .get_resolved_references(store.slot)
                .filter(|reference| reference.is_write())
                .map(|reference| self.span_of(reference.node_id()).start)
                .chain(self.scoping.symbol_declarations(store.slot).filter_map(|declaration| {
                    match self.nodes.kind(declaration) {
                        AstKind::VariableDeclarator(declarator) if declarator.init.is_some() => {
                            Some(declarator.span.start)
                        }
                        _ => None,
                    }
                }))
                .filter(|at| *at >= store.statement.end)
                .min()
                .unwrap_or(u32::MAX);
            let reads: Vec<NodeId> = self
                .scoping
                .get_resolved_references(store.slot)
                .filter(|reference| reference.is_read() && !reference.is_write())
                .map(|reference| reference.node_id())
                .filter(|node| {
                    let span = self.span_of(*node);
                    later.start <= span.start
                        && span.end <= later.end
                        && span.start < next_write
                        && self.owner_of(*node) == owner
                        && !self.in_noop(span)
                        && !gone.iter().any(|g| g.start <= span.start && span.end <= g.end)
                        && !self.is_shorthand_or_delete(*node)
                })
                .collect();
            if reads.is_empty() || !self.slot_unchanged(store.slot, store.statement, &reads, owner) {
                continue;
            }
            let text = &self.source[store.value.start as usize..store.value.end as usize];
            for read in reads {
                let span = self.span_of(read);
                let callee = matches!(self.nodes.parent_kind(read), AstKind::CallExpression(call) if call.callee.span() == span);
                let member_object = matches!(self.nodes.parent_kind(read),
                    AstKind::StaticMemberExpression(_) | AstKind::ComputedMemberExpression(_) | AstKind::PrivateFieldExpression(_));
                let placed = match store.kind {
                    InitKind::TupleSlot if callee && store.member => format!("(0, {text})"),
                    InitKind::Number if member_object => format!("({text})"),
                    InitKind::UnaryLiteral => format!("({text})"),
                    _ => text.to_string(),
                };
                // The value's own names must still resolve to the same
                // bindings at the read.
                let scope = self.nodes.get_node(read).scope_id();
                let resolves = self.references_in(store.value).iter().all(|(reference, ident)| {
                    let name = &self.source[ident.start as usize..ident.end as usize];
                    self.scoping.find_binding(scope, name.into()) == self.scoping.get_reference(*reference).symbol_id()
                });
                if resolves {
                    out.push((span, placed));
                }
            }
        }
        out
    }

    /// Where a moved value's initializer goes: its store for a handoff, its
    /// only read otherwise.
    pub(super) fn moved_read(&self, symbol: SymbolId, forward: &Forward) -> Option<NodeId> {
        forward.handoff.map(|(_, first)| first).or_else(|| {
            self.scoping
                .get_resolved_references(symbol)
                .next()
                .map(|reference| reference.node_id())
        })
    }

    pub(super) fn in_noop(&self, span: Span) -> bool {
        let at = self.scan.noops.partition_point(|noop| noop.end <= span.start);
        self.scan.noops.get(at).is_some_and(|noop| noop.start <= span.start && span.end <= noop.end)
    }

    /// `span` reads a moved (rule 2) value: copying it would repeat that
    /// value's evaluation.
    /// A value that computes to a literal: a number, or arithmetic over
    /// numbers and names whose forwarded values are themselves constants
    /// (`let b = a + 1` after `let a = 3 + 1`).
    pub(super) fn constant(&self, candidate: &Candidate) -> bool {
        match candidate.kind {
            InitKind::Number | InitKind::UnaryLiteral => true,
            InitKind::Computed | InitKind::Identifier => {
                candidate.pure
                    && self.references_in(candidate.init).iter().all(|(reference, _)| {
                        self.scoping
                            .get_reference(*reference)
                            .symbol_id()
                            .and_then(|symbol| self.forward_of(&symbol))
                            .is_some_and(|forward| {
                                forward.stable && self.constant(&self.scan.candidates[forward.candidate])
                            })
                    })
            }
            _ => false,
        }
    }

    pub(super) fn carries_move(&self, span: Span) -> bool {
        self.references_in(span).iter().any(|(reference, _)| {
            self.scoping
                .get_reference(*reference)
                .symbol_id()
                .and_then(|symbol| self.forward_of(&symbol))
                .is_some_and(|forward| !forward.stable)
        })
    }

    /// One declaration, already run when `statement` reads it: a later `var`
    /// initializer would change the value without a write reference.
    pub(super) fn declared_before(&self, symbol: SymbolId, statement: Span) -> bool {
        if self.forward_of(&symbol).is_some() {
            return true;
        }
        if self.scoping.symbol_declarations(symbol).count() != 1 {
            return false;
        }
        let declaration = self.scoping.symbol_declaration(symbol);
        matches!(
            self.nodes.kind(declaration),
            AstKind::Function(_) | AstKind::FormalParameter(_) | AstKind::FormalParameters(_)
        ) || self.span_of(declaration).end <= statement.start
    }

    /// The names a candidate's value reads once its forwarded inputs are
    /// substituted.
    pub(super) fn composed_reads(&self, candidate: &Candidate) -> Vec<(String, Option<SymbolId>)> {
        if let Some(representative) = candidate.alias_of {
            return vec![(
                self.scoping.symbol_name(representative).to_string(),
                Some(representative),
            )];
        }
        let mut reads = Vec::new();
        for (reference, _) in self.references_in(candidate.init) {
            let symbol = self.scoping.get_reference(reference).symbol_id();
            match symbol.and_then(|symbol| self.forward_of(&symbol)) {
                Some(forward) => reads.extend(forward.reads.iter().cloned()),
                None => {
                    let node = self.scoping.get_reference(reference).node_id();
                    let name = match self.nodes.kind(node) {
                        AstKind::IdentifierReference(id) => id.name.to_string(),
                        _ => continue,
                    };
                    reads.push((name, symbol));
                }
            }
        }
        // A chain of composed values names the same inputs again and again.
        reads.sort_unstable();
        reads.dedup();
        reads
    }

    pub(super) fn references_in(&self, span: Span) -> Vec<(ReferenceId, Span)> {
        let start = self.scan.idents.partition_point(|(s, _, _)| *s < span.start);
        self.scan.idents[start..]
            .iter()
            .take_while(|(s, _, _)| *s < span.end)
            .map(|(_, reference, _)| {
                let node = self.scoping.get_reference(*reference).node_id();
                (*reference, self.span_of(node))
            })
            .filter(|(_, ident)| ident.end <= span.end)
            .collect()
    }

    /// The final text of a forwarded value, with its own forwarded inputs
    /// substituted.
    pub(super) fn text(&self, symbol: SymbolId) -> String {
        let forward = &self.forwarded[&symbol];
        let candidate = &self.scan.candidates[forward.candidate];
        if let Some(representative) = candidate.alias_of {
            return self.scoping.symbol_name(representative).to_string();
        }
        let init = candidate.init;
        let mut out = String::new();
        let mut at = init.start as usize;
        for (reference, span) in self.references_in(init) {
            let Some(inner) = self.scoping.get_reference(reference).symbol_id() else {
                continue;
            };
            if !self.forward_of(&inner).is_some() {
                continue;
            }
            out.push_str(&self.source[at..span.start as usize]);
            out.push_str(&self.placement(inner, self.scoping.get_reference(reference).node_id()));
            at = span.end as usize;
        }
        out.push_str(&self.source[at..init.end as usize]);
        out
    }

    /// How a forwarded value reads in place of the identifier at `read`.
    pub(super) fn placement(&self, symbol: SymbolId, read: NodeId) -> String {
        let forward = &self.forwarded[&symbol];
        let candidate = &self.scan.candidates[forward.candidate];
        let text = self.text(symbol);
        let parent = self.nodes.parent_kind(read);
        let read_span = self.span_of(read);
        // An identifier copy reads as whatever it copies.
        let (kind, member, sequence) = self.root_kind(candidate);
        let callee = matches!(parent, AstKind::CallExpression(call) if call.callee.span() == read_span)
            || matches!(parent, AstKind::TaggedTemplateExpression(tagged) if tagged.tag.span() == read_span);
        match kind {
            InitKind::Word | InitKind::Identifier => text,
            InitKind::TupleSlot if callee => format!("(0, {text})"),
            InitKind::TupleSlot => text,
            InitKind::Number if !is_member_object(parent, read_span) => text,
            InitKind::Computed if member && callee => format!("(0, {text})"),
            InitKind::Computed | InitKind::UnaryLiteral
                if !sequence && is_isolated_slot(parent, read_span) =>
            {
                text
            }
            _ => format!("({text})"),
        }
    }

    pub(super) fn root_kind(&self, candidate: &Candidate) -> (InitKind, bool, bool) {
        if let Some(inner) = candidate
            .init_reference
            .and_then(|reference| self.scoping.get_reference(reference).symbol_id())
        {
            if let Some(forward) = self.forward_of(&inner) {
                return self.root_kind(&self.scan.candidates[forward.candidate]);
            }
        }
        (candidate.kind, candidate.member, candidate.sequence)
    }

    /// Starts of the shuffle declarations deleted whole: a pure shuffle whose
    /// every slot goes.
    pub(super) fn whole_calls(&self) -> BTreeSet<u32> {
        let mut slots_gone: BTreeMap<u32, usize> = BTreeMap::new();
        for forward in self.forwarded.values() {
            let candidate = &self.scan.candidates[forward.candidate];
            if candidate.helper.is_some() {
                *slots_gone.entry(candidate.statement.start).or_default() += 1;
            }
        }
        slots_gone
            .iter()
            .filter(|(start, gone)| {
                self.scan
                    .groups
                    .get(start)
                    .is_some_and(|(named, removable)| *removable && named == *gone)
            })
            .map(|(start, _)| *start)
            .collect()
    }

    /// Delete each forwarded declaration and write its value at every read
    /// outside another forwarded value. A read where a substituted name would
    /// resolve differently returns the symbol to block.
    pub(super) fn edits(&self) -> Result<Vec<analysis::Edit>, Vec<SymbolId>> {
        let mut blocked = self.broken_assumptions();
        if !blocked.is_empty() {
            return Err(blocked);
        }
        // A moved value runs where its one copy lands. Composed into a value
        // that is itself copied to several reads (a copy of a copy, a
        // snapshot), it would run once per copy: it stays.
        let mut copies = BTreeMap::new();
        // Forwarded initializers (declarations never nest), by start.
        let mut holders: Vec<(u32, u32, SymbolId)> = self
            .forwarded
            .iter()
            .map(|(symbol, forward)| (&self.scan.candidates[forward.candidate], *symbol))
            .filter(|(candidate, _)| candidate.slot.is_none())
            .map(|(candidate, symbol)| (candidate.init.start, candidate.init.end, symbol))
            .collect();
        holders.sort_unstable_by_key(|(start, ..)| *start);
        let repeated: Vec<SymbolId> = self
            .forwarded
            .iter()
            .filter(|(_, forward)| !forward.stable && !forward.snapshot && forward.early.is_none())
            .filter(|(symbol, _)| self.copies(**symbol, &holders, &mut copies) > 1)
            .map(|(symbol, _)| *symbol)
            .collect();
        if !repeated.is_empty() {
            return Err(repeated);
        }
        let whole_calls = self.whole_calls();
        // Text that leaves the program: deleted declarations (with their
        // initializers) and deleted shuffle calls. Reads there are composed
        // into the forwarded values instead of edited in place.
        let mut gone: Vec<Span> = self.scan.noops.clone();
        for forward in self.forwarded.values() {
            let candidate = &self.scan.candidates[forward.candidate];
            if candidate.slot.is_none() {
                gone.push(candidate.init);
            } else if whole_calls.contains(&candidate.statement.start) {
                gone.push(candidate.statement);
            }
        }
        let mut edits: Vec<analysis::Edit> = self
            .scan
            .noops
            .iter()
            .map(|noop| analysis::Edit::delete(whole_line(self.source, *noop)))
            .collect();
        for (read, text) in self.store_loads(&gone) {
            edits.push(analysis::Edit::replace(read, text));
        }
        for (read, text) in self.branch_booleans(&gone) {
            edits.push(analysis::Edit::replace(read, text));
        }
        let mut deleted_calls = BTreeSet::new();
        for (symbol, forward) in &self.forwarded {
            let candidate = &self.scan.candidates[forward.candidate];
            if let Some((slot, store, declared)) = forward.early {
                let name = self.scoping.symbol_name(slot);
                let mut value = self.text(*symbol);
                if candidate.sequence {
                    value = format!("({value})");
                }
                edits.push(analysis::Edit::replace(candidate.statement, format!("{name} = {value};")));
                edits.push(if declared {
                    analysis::Edit::replace(store, format!("var {name};"))
                } else {
                    analysis::Edit::delete(whole_line(self.source, store))
                });
                for id in self.scoping.get_resolved_reference_ids(*symbol) {
                    let node = self.scoping.get_reference(*id).node_id();
                    let span = self.span_of(node);
                    if (store.start <= span.start && span.end <= store.end) || self.in_noop(span) {
                        continue;
                    }
                    let scope = self.nodes.get_node(node).scope_id();
                    if self.scoping.find_binding(scope, name.into()) != Some(slot) {
                        blocked.push(*symbol);
                    }
                    edits.push(analysis::Edit::replace(span, name.to_string()));
                }
                continue;
            }
            match candidate.slot {
                Some(_) if whole_calls.contains(&candidate.statement.start) => {
                    if deleted_calls.insert(candidate.statement.start) {
                        edits.push(analysis::Edit::delete(whole_line(
                            self.source,
                            candidate.statement,
                        )));
                    }
                }
                // The slot becomes a hole; the tuple is fresh, so a trailing
                // hole dropping off is not observable.
                Some(slot) => edits.push(analysis::Edit::delete(slot)),
                None => edits.push(analysis::Edit::delete(whole_line(
                    self.source,
                    candidate.statement,
                ))),
            }
            for id in self.scoping.get_resolved_reference_ids(*symbol) {
                let node = self.scoping.get_reference(*id).node_id();
                let span = self.span_of(node);
                if gone
                    .iter()
                    .any(|init| init.start <= span.start && span.end <= init.end)
                {
                    continue;
                }
                let scope = self.nodes.get_node(node).scope_id();
                // After a handoff's store, a read reads the slot instead.
                if let Some((slot, first)) = forward.handoff {
                    if node != first {
                        let name = self.scoping.symbol_name(slot);
                        if self.scoping.find_binding(scope, name.into()) != Some(slot) {
                            blocked.push(*symbol);
                        }
                        edits.push(analysis::Edit::replace(span, name.to_string()));
                        continue;
                    }
                }
                for (name, expected) in &forward.reads {
                    if self.scoping.find_binding(scope, name.as_str().into()) != *expected {
                        blocked.push(*symbol);
                    }
                }
                edits.push(analysis::Edit::replace(span, self.placement(*symbol, node)));
            }
        }
        if !blocked.is_empty() {
            blocked.dedup();
            return Err(blocked);
        }
        Ok(edits)
    }

    /// How many times the forwarded `symbol`'s text is written: once per
    /// read it replaces, where a read inside another forwarded value counts
    /// as often as that value is written.
    fn copies(&self, symbol: SymbolId, holders: &[(u32, u32, SymbolId)], memo: &mut BTreeMap<SymbolId, usize>) -> usize {
        if let Some(count) = memo.get(&symbol) {
            return *count;
        }
        // A cycle cannot occur (a value reads earlier declarations), but
        // never loop on one.
        memo.insert(symbol, 1);
        let forward = &self.forwarded[&symbol];
        let count = if forward.early.is_some() {
            // Written once, where it is computed; reads read the slot.
            1
        } else {
            let reads: Vec<NodeId> = match forward.handoff {
                Some((_, first)) => vec![first],
                None => self
                    .scoping
                    .get_resolved_references(symbol)
                    .map(|reference| reference.node_id())
                    .filter(|node| !self.in_noop(self.span_of(*node)))
                    .collect(),
            };
            reads
                .into_iter()
                .map(|read| {
                    let span = self.span_of(read);
                    let at = holders.partition_point(|(start, ..)| *start <= span.start);
                    let holder = at
                        .checked_sub(1)
                        .map(|at| holders[at])
                        .filter(|(_, end, _)| span.end <= *end);
                    match holder {
                        Some((_, _, outer)) if outer != symbol => self.copies(outer, holders, memo),
                        _ => 1,
                    }
                })
                .sum()
        };
        memo.insert(symbol, count);
        count
    }

    pub(super) fn is_shorthand_or_delete(&self, read: NodeId) -> bool {
        match self.nodes.parent_kind(read) {
            AstKind::ObjectProperty(property) => property.shorthand,
            AstKind::UnaryExpression(unary) => unary.operator == UnaryOperator::Delete,
            _ => false,
        }
    }

    pub(super) fn span_of(&self, node: NodeId) -> Span {
        self.nodes.kind(node).span()
    }

    pub(super) fn function_of(&self, node: NodeId) -> Option<NodeId> {
        super::super::super::support::function_of(self.nodes, node)
    }

    pub(super) fn owner_of(&self, node: NodeId) -> u32 {
        self.function_of(node)
            .map_or(PROGRAM_OWNER, |function| self.nodes.kind(function).span().start)
    }
}
