//! Store an immutable snapshot in its existing private copy-out cell only when
//! every branch proves that neither cell's old live value can be observed.
//! Unlike a rename, this removes storage: creation becomes the destination's
//! real assignment, and all later snapshot reads must reach that same value.
use super::super::adjacent_ssa::LocalReads;
use crate::analysis;
use oxc_allocator::Allocator;
use oxc_ast::{ast::*, AstKind};
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;
use std::collections::BTreeSet;

pub(crate) fn coalesce_branch_handoffs(
    source: &str,
    protected: &BTreeSet<String>,
    checked_captures: bool,
) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    if !checked_captures && nodes.iter().any(|n| matches!(n.kind(), AstKind::Function(f) if f.id.as_ref().is_some_and(|id|id.name.starts_with("$THREAD_")) && f.body.as_ref().is_some_and(|b|b.statements.is_empty() && b.directives.is_empty()))) { return Ok((source.into(),0)); }
    // Dynamic lookup can observe a private binding by its spelling.
    if nodes.iter().any(|n| {
        matches!(n.kind(), AstKind::WithStatement(_))
            || matches!(n.kind(),AstKind::IdentifierReference(id) if id.name=="eval")
    }) {
        return Ok((source.into(), 0));
    }
    let mut locals = LocalReads::default();
    locals.visit_program(&program);
    locals.build(&scoping, &nodes, protected);
    let function_names = nodes
        .iter()
        .filter_map(|n| match n.kind() {
            AstKind::Function(f) => f.id.as_ref().map(|id| id.name.to_string()),
            _ => None,
        })
        .collect();
    let mut scan = Scan {
        source,
        protected,
        scoping: &scoping,
        nodes: &nodes,
        locals: &locals,
        function_names,
        edits: Vec::new(),
        count: 0,
    };
    scan.visit_program(&program);
    let result = analysis::apply_edits(source, scan.edits);
    if !result.rejected.is_empty() {
        return Err("branch handoff edits overlap".into());
    }
    Ok((result.output, scan.count))
}
struct Scan<'s, 'a> {
    source: &'s str,
    protected: &'s BTreeSet<String>,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    locals: &'s LocalReads,
    function_names: BTreeSet<String>,
    edits: Vec<analysis::Edit>,
    count: usize,
}
/// `let $SSA_x = init;` whose only reads come later in the same region.
struct Snapshot<'r, 'b> {
    declaration: &'r VariableDeclaration<'b>,
    id: &'r BindingIdentifier<'b>,
    init: &'r Expression<'b>,
    symbol: SymbolId,
    reads: Vec<&'r oxc_semantic::Reference>,
}
fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}
fn exits(s: &Statement<'_>) -> bool {
    match s {
        Statement::ReturnStatement(_) | Statement::ThrowStatement(_) => true,
        Statement::BlockStatement(b) => b.body.last().is_some_and(exits),
        Statement::IfStatement(i) => {
            exits(&i.consequent) && i.alternate.as_ref().is_some_and(exits)
        }
        _ => false,
    }
}
impl<'s> Scan<'s, '_> {
    fn private(&self, symbol: SymbolId, at: Span) -> bool {
        !self.protected.contains(self.scoping.symbol_name(symbol))
            && !self
                .function_names
                .contains(self.scoping.symbol_name(symbol))
            && self.scoping.symbol_redeclarations(symbol).is_empty()
            && self.locals.initialized(symbol, at)
            && self.scoping.get_resolved_references(symbol).all(|r| {
                self.locals
                    .same_owner(at, self.nodes.get_node(r.node_id()).kind().span())
            })
    }
    fn exception_safe(&self, destination: SymbolId, body: Span) -> bool {
        self.nodes.iter().all(|n| {
            let AstKind::TryStatement(t) = n.kind() else {
                return true;
            };
            if !contains(t.span, body) || !self.locals.same_owner(t.span, body) {
                return true;
            }
            let arm = [
                Some(t.block.span),
                t.handler.as_ref().map(|h| h.body.span),
                t.finalizer.as_ref().map(|b| b.span),
            ]
            .into_iter()
            .flatten()
            .find(|a| contains(*a, body));
            if arm.is_some_and(|a| contains(a, self.scoping.symbol_span(destination))) {
                return true;
            }
            if t.finalizer.is_some() {
                return false;
            }
            if contains(t.block.span, body) {
                return t.handler.as_ref().is_none_or(|h| {
                    h.body.body.last().is_some_and(exits)
                        && self.scoping.get_resolved_references(destination).all(|r| {
                            !contains(h.body.span, self.nodes.get_node(r.node_id()).kind().span())
                        })
                });
            }
            true
        })
    }
    fn body(&mut self, body: &[Statement<'_>], region: Span) {
        for (position, statement) in body.iter().enumerate() {
            let Some(snapshot) = self.snapshot(statement, region) else {
                continue;
            };
            for destination in self.destinations(&snapshot.reads) {
                let Some(edits) = self.handoff(body, position, region, &snapshot, destination)
                else {
                    continue;
                };
                self.edits.extend(edits);
                self.count += 1;
                // Independent snapshots must not acquire the same live storage
                // in one transaction; reparse before considering the next one.
                return;
            }
        }
    }
    /// A private `let`/`const` `$SSA_`/`$CARRY_` snapshot of a non-function
    /// value, read (never written) only later inside `region`.
    fn snapshot<'r, 'b>(
        &self,
        statement: &'r Statement<'b>,
        region: Span,
    ) -> Option<Snapshot<'r, 'b>>
    where
        's: 'r,
    {
        let Statement::VariableDeclaration(d) = statement else {
            return None;
        };
        if !matches!(
            d.kind,
            VariableDeclarationKind::Let | VariableDeclarationKind::Const
        ) || d.declarations.len() != 1
        {
            return None;
        }
        let (BindingPattern::BindingIdentifier(id), Some(init)) =
            (&d.declarations[0].id, &d.declarations[0].init)
        else {
            return None;
        };
        if !(id.name.starts_with("$SSA_") || id.name.starts_with("$CARRY_")) {
            return None;
        }
        let mut rhs = init;
        while let Expression::ParenthesizedExpression(p) = rhs {
            rhs = &p.expression;
        }
        if matches!(
            rhs,
            Expression::FunctionExpression(_)
                | Expression::ArrowFunctionExpression(_)
                | Expression::ClassExpression(_)
        ) {
            return None;
        }
        let snapshot = id.symbol_id.get()?;
        if !self.private(snapshot, Span::new(d.span.end, d.span.end)) {
            return None;
        }
        let reads: Vec<_> = self.scoping.get_resolved_references(snapshot).collect();
        if reads.is_empty()
            || reads.iter().any(|r| {
                r.is_write()
                    || !contains(region, self.nodes.get_node(r.node_id()).kind().span())
                    || self.nodes.get_node(r.node_id()).kind().span().start < d.span.end
            })
        {
            return None;
        }
        Some(Snapshot {
            declaration: d,
            id,
            init,
            symbol: snapshot,
            reads,
        })
    }
    /// The cells the snapshot is copied out to: `to = snapshot` reads.
    fn destinations(&self, reads: &[&oxc_semantic::Reference]) -> BTreeSet<SymbolId> {
        reads
            .iter()
            .filter_map(|r| {
                let AstKind::AssignmentExpression(a) = self.nodes.parent_kind(r.node_id()) else {
                    return None;
                };
                let AssignmentTarget::AssignmentTargetIdentifier(to) = &a.left else {
                    return None;
                };
                (a.operator == AssignmentOperator::Assign
                    && a.right.span() == self.nodes.get_node(r.node_id()).kind().span())
                .then(|| analysis::identifier_symbol(to, self.scoping))
                .flatten()
            })
            .collect()
    }
    /// The edits storing the snapshot straight into `destination` and reading
    /// it there, when every path after the snapshot proves the two cells hold
    /// the same value wherever either is observed.
    fn handoff(
        &self,
        body: &[Statement<'_>],
        position: usize,
        region: Span,
        snapshot: &Snapshot<'_, '_>,
        destination: SymbolId,
    ) -> Option<Vec<analysis::Edit>> {
        let Snapshot {
            declaration: d,
            id,
            init,
            symbol: snapshot,
            ref reads,
        } = *snapshot;
        // A lexical destination is already initialized before this producer.
        // Parameters, exports and implicit var/function stores stay barriers.
        if destination == snapshot
            || self.scoping.symbol_flags(destination).is_const_variable()
            || !self
                .scoping
                .symbol_flags(destination)
                .contains(oxc_syntax::symbol::SymbolFlags::BlockScopedVariable)
            || !self.private(destination, d.span)
            || !self.exception_safe(destination, region)
        {
            return None;
        }
        let mut proof = Proof {
            scoping: self.scoping,
            snapshot,
            destination,
            state: State {
                dirty: true,
                holds: true,
                live: true,
            },
            valid: true,
        };
        let last_read = reads
            .iter()
            .map(|r| self.nodes.get_node(r.node_id()).kind().span().end)
            .max()
            .unwrap();
        for s in &body[position + 1..] {
            if proof.state.live {
                proof.visit_statement(s);
            }
            // Once all snapshot reads finish and every live path has
            // restored equality, subsequent unrelated control flow
            // cannot distinguish the two storage arrangements.
            if s.span().end >= last_read && (!proof.state.live || !proof.state.dirty) {
                break;
            }
        }
        if !proof.valid || proof.state.live && proof.state.dirty {
            return None;
        }
        if reads.iter().any(|r| {
            self.scoping
                .find_binding(r.scope_id(), self.scoping.symbol_ident(destination))
                != Some(destination)
        }) {
            return None;
        }
        let name = self.scoping.symbol_name(destination);
        let mut edits = vec![analysis::Edit::replace(
            d.span,
            format!(
                "{name} = ({});",
                &self.source[init.span().start as usize..init.span().end as usize]
            ),
        )];
        for r in reads {
            let span = self.nodes.get_node(r.node_id()).kind().span();
            let text = if matches!(self.nodes.parent_kind(r.node_id()),AstKind::ObjectProperty(p)if p.shorthand)
            {
                format!("{}: {name}", id.name)
            } else {
                name.to_string()
            };
            edits.push(analysis::Edit::replace(span, text));
        }
        if self.source[d.span.start as usize..d.span.end as usize].contains("/*")
            || self.source[d.span.start as usize..d.span.end as usize].contains("//")
            || edits.iter().any(|e| {
                self.edits
                    .iter()
                    .any(|p| e.start < p.end && p.start < e.end)
            })
        {
            return None;
        }
        Some(edits)
    }
}
impl<'a> Visit<'a> for Scan<'_, 'a> {
    fn visit_function_body(&mut self, b: &FunctionBody<'a>) {
        if self.count == 0 {
            self.body(&b.statements, b.span);
        }
        walk::walk_function_body(self, b);
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        if self.count == 0 {
            self.body(&b.body, b.span);
        }
        walk::walk_block_statement(self, b);
    }
}
#[derive(Clone, Copy)]
struct State {
    dirty: bool,
    holds: bool,
    live: bool,
}
impl State {
    fn merge(a: Self, b: Self) -> Self {
        if !a.live {
            return b;
        }
        if !b.live {
            return a;
        }
        Self {
            dirty: a.dirty || b.dirty,
            holds: a.holds && b.holds,
            live: true,
        }
    }
}
struct Proof<'s> {
    scoping: &'s Scoping,
    snapshot: SymbolId,
    destination: SymbolId,
    state: State,
    valid: bool,
}
impl<'a> Visit<'a> for Proof<'_> {
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        let Some(reference) = id.reference_id.get() else {
            return;
        };
        if self.scoping.get_reference(reference).is_write()
            && analysis::identifier_symbol(id, self.scoping) == Some(self.destination)
        {
            self.valid = false;
        }
        if !self.scoping.get_reference(reference).is_read() {
            return;
        }
        match analysis::identifier_symbol(id, self.scoping) {
            Some(s) if s == self.snapshot && !self.state.holds => self.valid = false,
            Some(s) if s == self.destination && self.state.dirty => self.valid = false,
            _ => {}
        }
    }
    fn visit_assignment_expression(&mut self, a: &AssignmentExpression<'a>) {
        if matches!(&a.left,AssignmentTarget::AssignmentTargetIdentifier(id)if analysis::identifier_symbol(id,self.scoping)==Some(self.destination))
        {
            if a.operator != AssignmentOperator::Assign {
                self.valid = false;
                return;
            }
            self.visit_expression(&a.right);
            self.state.dirty = false;
            self.state.holds = matches!(&a.right,Expression::Identifier(id)if analysis::identifier_symbol(id,self.scoping)==Some(self.snapshot));
        } else {
            walk::walk_assignment_expression(self, a);
        }
    }
    fn visit_if_statement(&mut self, i: &IfStatement<'a>) {
        self.visit_expression(&i.test);
        let before = self.state;
        self.visit_statement(&i.consequent);
        let yes = self.state;
        self.state = before;
        if let Some(no) = &i.alternate {
            self.visit_statement(no);
        }
        self.state = State::merge(yes, self.state);
    }
    fn visit_conditional_expression(&mut self, c: &ConditionalExpression<'a>) {
        self.visit_expression(&c.test);
        let before = self.state;
        self.visit_expression(&c.consequent);
        let yes = self.state;
        self.state = before;
        self.visit_expression(&c.alternate);
        self.state = State::merge(yes, self.state);
    }
    fn visit_logical_expression(&mut self, l: &LogicalExpression<'a>) {
        self.visit_expression(&l.left);
        let before = self.state;
        self.visit_expression(&l.right);
        self.state = State::merge(before, self.state);
    }
    fn visit_chain_expression(&mut self, c: &ChainExpression<'a>) {
        let before = self.state;
        walk::walk_chain_expression(self, c);
        self.state = State::merge(before, self.state);
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        for s in &b.body {
            if self.state.live {
                self.visit_statement(s);
            }
        }
    }
    fn visit_return_statement(&mut self, r: &ReturnStatement<'a>) {
        if let Some(e) = &r.argument {
            self.visit_expression(e);
        }
        self.state.live = false;
    }
    fn visit_throw_statement(&mut self, r: &ThrowStatement<'a>) {
        self.visit_expression(&r.argument);
        self.state.live = false;
    }
    fn visit_break_statement(&mut self, _: &BreakStatement<'a>) {
        self.valid &= !self.state.dirty;
        self.state.live = false;
    }
    fn visit_continue_statement(&mut self, _: &ContinueStatement<'a>) {
        self.valid &= !self.state.dirty;
        self.state.live = false;
    }
    fn visit_for_statement(&mut self, _: &ForStatement<'a>) {
        self.valid = false;
    }
    fn visit_while_statement(&mut self, _: &WhileStatement<'a>) {
        self.valid = false;
    }
    fn visit_do_while_statement(&mut self, _: &DoWhileStatement<'a>) {
        self.valid = false;
    }
    fn visit_for_in_statement(&mut self, _: &ForInStatement<'a>) {
        self.valid = false;
    }
    fn visit_for_of_statement(&mut self, _: &ForOfStatement<'a>) {
        self.valid = false;
    }
    fn visit_switch_statement(&mut self, _: &SwitchStatement<'a>) {
        self.valid = false;
    }
    fn visit_try_statement(&mut self, _: &TryStatement<'a>) {
        self.valid = false;
    }
    fn visit_labeled_statement(&mut self, _: &LabeledStatement<'a>) {
        self.valid = false;
    }
    fn visit_function(&mut self, _: &Function<'a>, _: ScopeFlags) {}
    fn visit_arrow_function_expression(&mut self, _: &ArrowFunctionExpression<'a>) {}
    fn visit_class(&mut self, _: &Class<'a>) {
        self.valid = false;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn branches_commit_or_reset_only_the_existing_private_accumulator() {
        for source in [
            "function f(flag){let acc=0;let $SSA_0=acc|read();if(flag){use($SSA_0);acc=0;}else{acc=$SSA_0;}return acc;}",
            "function f(flag){let acc=0;let $SSA_0=acc|read();flag?(use($SSA_0),acc=0):acc=$SSA_0;return acc;}",
            "function f(flag){let acc=0;while(flag--){let $SSA_0=acc|read();if(flag){use($SSA_0);acc=0;}else{acc=$SSA_0;}}return acc;}",
            "function f(flag){let acc=0;let $SSA_0=read();if(flag)return $SSA_0;else acc=$SSA_0;return acc;}",
            "function f(flag){let acc=0;let $SSA_0=acc|read();if(flag){use($SSA_0);acc=0;}else acc=$SSA_0;for(let i=0;i<3;i++)use(i);return acc;}",
        ] {
            let (code,count)=super::coalesce_branch_handoffs(source,&Default::default(),false).unwrap();
            assert_eq!(count,1,"{code}");
            assert!(!code.contains("$SSA_0"),"{code}");
        }
    }
    #[test]
    fn old_values_snapshots_captures_and_exception_observers_are_barriers() {
        for source in [
            "function f(flag){let acc=0;let $SSA_0=read();if(flag){use(acc);acc=0;}else{acc=$SSA_0;}return acc;}",
            "function f(flag){let acc=0;let $SSA_0=read();if(flag){acc=0;use($SSA_0);}else{acc=$SSA_0;}return acc;}",
            "function f(flag){let acc=0;let $SSA_0=read();if(flag)use($SSA_0);else acc=$SSA_0;return acc;}",
            "function f(flag){let acc=0;let $SSA_0=read();let observe=()=>acc;use($SSA_0);acc=$SSA_0;return observe();}",
            "function f(flag){let acc=0;try{let $SSA_0=read();use($SSA_0);acc=$SSA_0;}finally{return acc;}}",
            "function f(flag){let acc=0;try{let $SSA_0=read();use($SSA_0);acc=$SSA_0;}catch(e){return acc;}return acc;}",
            "function f(flag){let acc=0;let $SSA_0=read();flag?.use(acc=$SSA_0);return acc;}",
            "function f(flag){let acc=0;let $SSA_0=read();use($SSA_0);[acc]=[4];acc=$SSA_0;return acc;}",
            "function f(flag){let acc=0;let $SSA_0=read();acc++;use($SSA_0);acc=$SSA_0;return acc;}",
            "function f(flag){let acc=0;let $SSA_0=function(){};use($SSA_0);acc=$SSA_0;return acc.name;}",
            "function f(flag){let acc=0;let $SSA_0=read();eval('acc');acc=$SSA_0;return acc;}",
        ] {
            let (code,count)=super::coalesce_branch_handoffs(source,&Default::default(),false).unwrap();
            assert_eq!(count,0,"{code}");
        }
    }

    #[test]
    fn shared_slots_and_pending_children_keep_their_storage() {
        let source = "function f(flag){let $KEY_0=0;let $SSA_0=read();if(flag){use($SSA_0);$KEY_0=0;}else $KEY_0=$SSA_0;return $KEY_0;}";
        let protected = ["$KEY_0".into()].into_iter().collect();
        assert_eq!(
            super::coalesce_branch_handoffs(source, &protected, false)
                .unwrap()
                .1,
            0
        );
        let pending = "function f(flag){function $THREAD_0_1(){}let acc=0;let $SSA_0=read();if(flag){use($SSA_0);acc=0;}else acc=$SSA_0;return acc;}";
        assert_eq!(
            super::coalesce_branch_handoffs(pending, &Default::default(), false)
                .unwrap()
                .1,
            0
        );
    }
}
