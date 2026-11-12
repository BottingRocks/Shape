//! Store an SSA result directly in its adjacent VM slot destination.
//!
//! No computation moves: `let $SSA = value; $KEY = $SSA` becomes `$KEY =
//! value`, and later SSA reads use that slot only while it remains unchanged.
//! Captures, loop crossings, shadowing, mapped parameters and later slot writes
//! decline. This coalesces storage rather than disguising a temporary by renaming.
use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::SymbolId;
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

use super::super::index::SpanIndex;
use crate::analysis;

pub(crate) fn forward_ssa_slots(
    source: &str,
    protected: &BTreeSet<String>,
) -> Result<(String, usize), String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = Scan::default();
    scan.visit_program(&program);
    scan.functions.build();
    scan.loops.build();
    scan.parameters.build();
    if scan.dynamic {
        return Ok((source.into(), 0));
    }
    scan.collect_candidates(&scoping, source);
    let mut forwarding = Forwarding {
        source,
        protected,
        scoping: &scoping,
        nodes: &nodes,
        scan: &scan,
        edits: Vec::new(),
        selected: BTreeSet::new(),
        count: 0,
    };
    for candidate in &scan.candidates {
        forwarding.copy(candidate);
    }
    for carry in &scan.carry_slots {
        forwarding.carry(carry);
    }
    let Forwarding { edits, count, .. } = forwarding;
    if edits.is_empty() {
        return Ok((source.into(), 0));
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("SSA slot forwarding edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)?;
    Ok((applied.output, count))
}

/// The accepted edits so far, and the slots and values they claimed.
struct Forwarding<'s, 'a> {
    source: &'s str,
    protected: &'s BTreeSet<String>,
    scoping: &'s oxc_semantic::Scoping,
    nodes: &'s oxc_semantic::AstNodes<'a>,
    scan: &'s Scan,
    edits: Vec<analysis::Edit>,
    selected: BTreeSet<SymbolId>,
    count: usize,
}

impl Forwarding<'_, '_> {
    /// Store the SSA value directly in the slot it is copied to, when the
    /// slot keeps it at every later read of the value.
    fn copy(&mut self, candidate: &Candidate) {
        let (scoping, scan, source) = (self.scoping, self.scan, self.source);
        let Some(slot) = candidate.slot.or_else(|| {
            candidate
                .slot_ref
                .and_then(|r| scoping.get_reference(r).symbol_id())
        }) else {
            return;
        };
        let value = candidate.value;
        if self.protected.contains(scoping.symbol_name(slot)) {
            return;
        }
        if slot == value
            || self.selected.contains(&slot)
            || self.selected.contains(&value)
            || scoping.symbol_flags(slot).is_const_variable()
            || !scoping.symbol_redeclarations(slot).is_empty()
            || scan
                .parameters
                .enclosing(scoping.symbol_span(slot))
                .is_some()
            || (!candidate.declaration && scoping.symbol_span(slot).end >= candidate.first.start)
            || scoping.find_binding(scoping.symbol_scope_id(value), scoping.symbol_ident(slot))
                != Some(slot)
        {
            return;
        }
        let function = scan.functions.enclosing(candidate.first);
        let Some(reads) = self.value_reads(candidate, slot, function) else {
            return;
        };
        let last = reads
            .iter()
            .map(|s| s.end)
            .max()
            .unwrap_or(candidate.second.end)
            .max(candidate.second.end);
        if self.slot_disturbed(candidate, slot, function, last) {
            return;
        }
        let text = |span: Span| &source[span.start as usize..span.end as usize];
        if text(candidate.first).contains("/*")
            || text(candidate.first).contains("//")
            || text(candidate.second).contains("/*")
            || text(candidate.second).contains("//")
        {
            return;
        }
        let mut destination = String::new();
        destination
            .push_str(&source[candidate.second.start as usize..candidate.copy.start as usize]);
        destination.push('(');
        destination.push_str(text(candidate.initializer));
        destination.push(')');
        destination.push_str(&source[candidate.copy.end as usize..candidate.second.end as usize]);
        let mut proposal = vec![
            analysis::Edit::replace(candidate.first, destination),
            analysis::Edit::delete(candidate.second),
        ];
        for read in reads {
            let name = scoping.symbol_name(slot);
            let replacement = if scan.shorthand.contains(&read) {
                format!("{}: {name}", text(read))
            } else {
                name.into()
            };
            proposal.push(analysis::Edit::replace(read, replacement));
        }
        if proposal.iter().any(|e| {
            self.edits
                .iter()
                .any(|other| e.start < other.end && other.start < e.end)
        }) {
            return;
        }
        self.edits.extend(proposal);
        self.selected.insert(slot);
        self.selected.insert(value);
        self.count += 1;
    }

    /// The value's reads other than the copy, if all follow the definition
    /// in its function and loop and see `slot` under its name.
    fn value_reads(
        &self,
        candidate: &Candidate,
        slot: SymbolId,
        function: Option<Span>,
    ) -> Option<Vec<Span>> {
        let (scoping, scan) = (self.scoping, self.scan);
        let loop_span = scan.loops.enclosing(candidate.first);
        let mut reads = Vec::new();
        for reference in scoping.get_resolved_references(candidate.value) {
            let span = self.nodes.get_node(reference.node_id()).kind().span();
            if span == candidate.copy {
                continue;
            }
            if reference.is_write()
                || span.start < candidate.first.end
                || scan.functions.enclosing(span) != function
                || scan.loops.enclosing(span) != loop_span
                || scoping.find_binding(reference.scope_id(), scoping.symbol_ident(slot))
                    != Some(slot)
                || scan.non_values.contains(&span)
            {
                return None;
            }
            reads.push(span);
        }
        Some(reads)
    }

    /// Whether the slot is written, read or captured where the forwarded
    /// value must stay in it, up to `last`.
    fn slot_disturbed(
        &self,
        candidate: &Candidate,
        slot: SymbolId,
        function: Option<Span>,
        last: u32,
    ) -> bool {
        let scan = self.scan;
        // Declaration initializers are writes too, even when they are absent
        // from the ordinary identifier-reference table (repeated `var`s).
        scan.initializers.get(&slot).is_some_and(|spans| {
            spans.iter().any(|span| {
                span.start >= candidate.first.end
                    && span.start < last
                    && !(span.start >= candidate.second.start && span.end <= candidate.second.end)
            })
        }) || self.scoping.get_resolved_references(slot).any(|reference| {
            let span = self.nodes.get_node(reference.node_id()).kind().span();
            scan.functions.enclosing(span) != function
                || (span.start >= candidate.first.end && span.start < candidate.second.start)
                || (reference.is_write() && span.start >= candidate.second.end && span.start < last)
                || (reference.is_read()
                    && span.start >= candidate.second.end
                    && !(span.start >= candidate.body.start && span.end <= candidate.body.end))
        })
    }

    /// Read a carried `$KEY_` slot in place of the value copied from it,
    /// when nothing touches the slot after the copy.
    fn carry(&mut self, carry: &CarrySlot) {
        let (scoping, scan, source) = (self.scoping, self.scan, self.source);
        let Some(slot) = scoping.get_reference(carry.source).symbol_id() else {
            return;
        };
        if self.protected.contains(scoping.symbol_name(slot)) {
            return;
        }
        if self.selected.contains(&slot)
            || self.selected.contains(&carry.value)
            || !scoping.symbol_name(slot).starts_with("$KEY_")
            || scoping.symbol_flags(slot).is_const_variable()
            || scan
                .parameters
                .enclosing(scoping.symbol_span(slot))
                .is_some()
            || scan.loops.enclosing(carry.declaration).is_some()
            || !(scoping.symbol_flags(slot).is_function_scoped_declaration()
                || (scoping.symbol_span(slot).start >= carry.body.start
                    && scoping.symbol_span(slot).end < carry.declaration.start))
        {
            return;
        }
        let function = scan.functions.enclosing(carry.declaration);
        if scoping.get_resolved_references(slot).any(|r| {
            let span = self.nodes.get_node(r.node_id()).kind().span();
            scan.functions.enclosing(span) != function
                || (span != carry.read && span.start >= carry.declaration.start)
        }) || scan.initializers.get(&slot).is_some_and(|spans| {
            spans
                .iter()
                .any(|span| span.start >= carry.declaration.start)
        }) {
            return;
        }
        let mut proposal = vec![analysis::Edit::delete(carry.declaration)];
        let mut valid = true;
        for r in scoping.get_resolved_references(carry.value) {
            let span = self.nodes.get_node(r.node_id()).kind().span();
            if span.start < carry.declaration.end
                || scan.functions.enclosing(span) != function
                || scoping.find_binding(r.scope_id(), scoping.symbol_ident(slot)) != Some(slot)
                || scan.non_values.contains(&span)
            {
                valid = false;
                break;
            }
            let name = scoping.symbol_name(slot);
            let replacement = if scan.shorthand.contains(&span) {
                format!("{}: {name}", scoping.symbol_name(carry.value))
            } else {
                name.into()
            };
            proposal.push(analysis::Edit::replace(span, replacement));
        }
        if !valid
            || source[carry.declaration.start as usize..carry.declaration.end as usize]
                .contains("/*")
            || source[carry.declaration.start as usize..carry.declaration.end as usize]
                .contains("//")
            || proposal.iter().any(|e| {
                self.edits
                    .iter()
                    .any(|other| e.start < other.end && other.start < e.end)
            })
        {
            return;
        }
        self.edits.extend(proposal);
        self.selected.insert(slot);
        self.selected.insert(carry.value);
        self.count += 1;
    }
}

struct Candidate {
    body: Span,
    first: Span,
    second: Span,
    initializer: Span,
    value: SymbolId,
    slot: Option<SymbolId>,
    slot_ref: Option<oxc_semantic::ReferenceId>,
    copy: Span,
    declaration: bool,
}
#[derive(Default)]
struct Scan {
    carry_slots: Vec<CarrySlot>,
    sources: BTreeMap<SymbolId, (Span, Span)>,
    copies: Vec<Copy>,
    functions: SpanIndex,
    loops: SpanIndex,
    parameters: SpanIndex,
    candidates: Vec<Candidate>,
    initializers: BTreeMap<SymbolId, Vec<Span>>,
    shorthand: BTreeSet<Span>,
    non_values: BTreeSet<Span>,
    dynamic: bool,
}
struct Copy {
    statement: Span,
    body: Span,
    slot: Option<SymbolId>,
    slot_ref: Option<oxc_semantic::ReferenceId>,
    value_ref: Option<oxc_semantic::ReferenceId>,
    read: Span,
    declaration: bool,
    hoisted: bool,
}
struct CarrySlot {
    declaration: Span,
    body: Span,
    value: SymbolId,
    source: oxc_semantic::ReferenceId,
    read: Span,
}
impl Scan {
    /// Pair each slot copy with the SSA declaration it copies, when the copy
    /// follows it (directly, for a lexical slot declaration).
    fn collect_candidates(&mut self, scoping: &oxc_semantic::Scoping, source: &str) {
        for copy in &self.copies {
            let Some(value) = copy
                .value_ref
                .and_then(|r| scoping.get_reference(r).symbol_id())
            else {
                continue;
            };
            let Some((first, initializer)) = self.sources.get(&value).copied() else {
                continue;
            };
            if first.end > copy.statement.start {
                continue;
            }
            // Lexical declarations can only move over the immediately preceding
            // source declaration; a var slot is already initialized on entry.
            if copy.declaration
                && !copy.hoisted
                && source[first.end as usize..copy.statement.start as usize].trim() != ""
            {
                continue;
            }
            self.candidates.push(Candidate {
                first,
                initializer,
                second: copy.statement,
                value,
                slot: copy.slot,
                slot_ref: copy.slot_ref,
                copy: copy.read,
                declaration: copy.declaration,
                body: copy.body,
            });
        }
    }
    fn body(&mut self, statements: &[Statement<'_>]) {
        let Some((first, last)) = statements.first().zip(statements.last()) else {
            return;
        };
        let body = Span::new(first.span().start, last.span().end);
        for statement in statements {
            if let Statement::VariableDeclaration(d) = statement {
                if d.declarations.len() == 1
                    && matches!(
                        d.kind,
                        VariableDeclarationKind::Let | VariableDeclarationKind::Const
                    )
                {
                    let binding = &d.declarations[0];
                    if let (BindingPattern::BindingIdentifier(id), Some(init)) =
                        (&binding.id, &binding.init)
                    {
                        if id.name.starts_with("$SSA_") {
                            if let Some(symbol) = id.symbol_id.get() {
                                self.sources.insert(symbol, (d.span, init.span()));
                            }
                        }
                        if id.name.starts_with("$CARRY_") {
                            if let Expression::Identifier(read) = init {
                                if let (Some(value), Some(source)) =
                                    (id.symbol_id.get(), read.reference_id.get())
                                {
                                    self.carry_slots.push(CarrySlot {
                                        declaration: d.span,
                                        body,
                                        value,
                                        source,
                                        read: read.span,
                                    });
                                }
                            }
                        }
                    }
                }
            }
            let (slot, slot_ref, name, read, declaration, hoisted) = match statement {
                Statement::VariableDeclaration(d) if d.declarations.len() == 1 => {
                    let (
                        BindingPattern::BindingIdentifier(slot),
                        Some(Expression::Identifier(read)),
                    ) = (&d.declarations[0].id, &d.declarations[0].init)
                    else {
                        continue;
                    };
                    (
                        slot.symbol_id.get(),
                        None,
                        slot.name.as_str(),
                        read,
                        true,
                        d.kind == VariableDeclarationKind::Var,
                    )
                }
                Statement::ExpressionStatement(s) => {
                    let Expression::AssignmentExpression(a) = &s.expression else {
                        continue;
                    };
                    let (
                        AssignmentTarget::AssignmentTargetIdentifier(slot),
                        Expression::Identifier(read),
                    ) = (&a.left, &a.right)
                    else {
                        continue;
                    };
                    if a.operator != AssignmentOperator::Assign {
                        continue;
                    }
                    (
                        None,
                        slot.reference_id.get(),
                        slot.name.as_str(),
                        read,
                        false,
                        false,
                    )
                }
                _ => continue,
            };
            if !name.starts_with("$KEY_") || !read.name.starts_with("$SSA_") {
                continue;
            }
            self.copies.push(Copy {
                statement: statement.span(),
                body,
                slot,
                slot_ref,
                value_ref: read.reference_id.get(),
                read: read.span,
                declaration,
                hoisted,
            });
        }
    }
}
impl<'a> Visit<'a> for Scan {
    fn visit_assignment_target_property_identifier(
        &mut self,
        p: &AssignmentTargetPropertyIdentifier<'a>,
    ) {
        self.non_values.insert(p.binding.span);
        walk::walk_assignment_target_property_identifier(self, p);
    }
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        self.functions.push(f.span);
        self.parameters.push(f.params.span);
        if let Some(body) = &f.body {
            self.body(&body.statements);
        }
        walk::walk_function(self, f, flags);
    }
    fn visit_arrow_function_expression(&mut self, f: &ArrowFunctionExpression<'a>) {
        self.functions.push(f.span);
        self.parameters.push(f.params.span);
        self.body(&f.body.statements);
        walk::walk_arrow_function_expression(self, f);
    }
    fn visit_block_statement(&mut self, b: &BlockStatement<'a>) {
        self.body(&b.body);
        walk::walk_block_statement(self, b);
    }
    fn visit_variable_declarator(&mut self, d: &VariableDeclarator<'a>) {
        if let BindingPattern::BindingIdentifier(id) = &d.id {
            if let Some(symbol) = id.symbol_id.get() {
                if d.init.is_some() {
                    self.initializers.entry(symbol).or_default().push(id.span);
                }
            }
        }
        walk::walk_variable_declarator(self, d);
    }
    fn visit_object_property(&mut self, p: &ObjectProperty<'a>) {
        if p.shorthand {
            self.shorthand.insert(p.value.span());
        }
        walk::walk_object_property(self, p);
    }
    fn visit_unary_expression(&mut self, u: &UnaryExpression<'a>) {
        if u.operator == UnaryOperator::Delete {
            self.non_values.insert(u.argument.span());
        }
        walk::walk_unary_expression(self, u);
    }
    fn visit_with_statement(&mut self, s: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, s);
    }
    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        self.dynamic |= id.name == "eval";
    }
    fn visit_for_statement(&mut self, s: &ForStatement<'a>) {
        self.loops.push(s.span);
        walk::walk_for_statement(self, s);
    }
    fn visit_for_in_statement(&mut self, s: &ForInStatement<'a>) {
        self.loops.push(s.span);
        walk::walk_for_in_statement(self, s);
    }
    fn visit_for_of_statement(&mut self, s: &ForOfStatement<'a>) {
        self.loops.push(s.span);
        walk::walk_for_of_statement(self, s);
    }
    fn visit_while_statement(&mut self, s: &WhileStatement<'a>) {
        self.loops.push(s.span);
        walk::walk_while_statement(self, s);
    }
    fn visit_do_while_statement(&mut self, s: &DoWhileStatement<'a>) {
        self.loops.push(s.span);
        walk::walk_do_while_statement(self, s);
    }
}
