//! Inline generated SSA and carry copies while their source value is stable.
//!
//! A candidate is `let|const $SSA_n|$CARRY_n = <plain identifier>;`. The copy
//! is removed and each resolved read is replaced by the source binding's name.
//! The source must be declared before the copy; the alias must be read (never
//! written); reads must stay in the same enclosing function, must not be
//! shorthand property values, and must re-resolve to the same symbol under
//! their own scope. A later source write is accepted only for a loop snapshot
//! or straight-line snapshot where reads cannot observe the intervening store.
//! Captures, read-before-init, comments, overlapping edits, `with`, and `eval`
//! decline. This removes copies of parameters, VM slots, and loop values
//! without duplicating an effect, because the initializer is one identifier
//! read.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::SymbolId;
use oxc_span::{GetSpan, Span};
use oxc_syntax::scope::ScopeFlags;

use super::super::adjacent_ssa::LocalReads;
use super::super::index::SpanIndex;
use crate::analysis;
use super::super::support::whole_line;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AliasSsaResult {
    pub code: String,
    pub inlined_bindings: usize,
}

/// Replace `let $SSA_n = local;` with reads of `local` when all those reads
/// resolve to the same binding and its value is stable at every read. A copy
/// made inside a loop can be inlined when all its reads precede the loop-state
/// write in that iteration. Captured
/// copies, dynamic scope and shorthand object keys stay put.
pub(crate) fn inline_alias_ssa(source: &str) -> Result<AliasSsaResult, String> {
    inline_alias_ssa_with_protected(source, &BTreeSet::new())
}

pub(crate) fn inline_alias_ssa_with_protected(
    source: &str,
    protected: &BTreeSet<String>,
) -> Result<AliasSsaResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut locals = LocalReads::default();
    locals.visit_program(&program);
    locals.build(&scoping, &nodes, protected);
    let mut scan = Scan::default();
    scan.visit_program(&program);
    scan.functions.build();
    scan.blocks.build();
    scan.loops.build();
    scan.parameters.build();
    scan.arguments_reads.sort_unstable_by_key(|span| span.start);
    if scan.dynamic_scope {
        return Ok(AliasSsaResult {
            code: source.into(),
            inlined_bindings: 0,
        });
    }
    let aliases = Aliases {
        scoping: &scoping,
        nodes: &nodes,
        scan: &scan,
        locals: &locals,
    };
    let mut edits = Vec::new();
    let mut selected = BTreeSet::<Span>::new();
    let mut selected_aliases = BTreeSet::<SymbolId>::new();
    let mut count = 0;
    for candidate in &scan.candidates {
        let Some(source_symbol) = aliases.source_symbol(candidate, protected) else {
            continue;
        };
        if selected_aliases.contains(&source_symbol) {
            continue;
        }
        if scoping.symbol_span(source_symbol).start >= candidate.statement.start {
            continue;
        }
        if candidate_has_comments(source, candidate.statement) {
            continue;
        }
        let Some(reads) = aliases.reads(candidate, source_symbol) else {
            continue;
        };
        if reads.iter().any(|read| {
            selected
                .range(..=Span::new(read.start, u32::MAX))
                .next_back()
                .is_some_and(|span| contains(*span, *read))
        }) {
            continue;
        }
        selected.insert(candidate.statement);
        selected_aliases.insert(candidate.alias);
        edits.push(analysis::Edit::delete(whole_line(
            source,
            candidate.statement,
        )));
        for read in reads {
            edits.push(analysis::Edit::replace(
                read,
                scoping.symbol_name(source_symbol),
            ));
        }
        count += 1;
    }
    if edits.is_empty() {
        return Ok(AliasSsaResult {
            code: source.into(),
            inlined_bindings: 0,
        });
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("alias SSA edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("alias SSA output: {error}"))?;
    Ok(AliasSsaResult {
        code: applied.output,
        inlined_bindings: count,
    })
}

/// The facts one alias decision consults.
struct Aliases<'s, 'a> {
    scoping: &'s oxc_semantic::Scoping,
    nodes: &'s oxc_semantic::AstNodes<'a>,
    scan: &'s Scan,
    locals: &'s LocalReads,
}

impl Aliases<'_, '_> {
    /// The binding the copy reads, unless it is protected, shares its name
    /// with a function binding, or is a parameter `arguments` can write.
    fn source_symbol(&self, candidate: &Candidate, protected: &BTreeSet<String>) -> Option<SymbolId> {
        let (scoping, scan) = (self.scoping, self.scan);
        let source_symbol = scoping.get_reference(candidate.source).symbol_id()?;
        if protected.contains(scoping.symbol_name(source_symbol)) {
            return None;
        }
        if scoping
            .symbol_flags(source_symbol)
            .is_function_scoped_declaration()
            && scan
                .function_bindings
                .get(scoping.symbol_name(source_symbol))
                .is_some_and(|bindings| bindings.iter().any(|symbol| *symbol != source_symbol))
        {
            return None;
        }
        // In sloppy functions, arguments[index] (including through an alias)
        // can write a parameter without an identifier write in Oxc's reference
        // table. Keep function-scoped snapshots when arguments is exposed.
        if scan
            .parameters
            .enclosing(scoping.symbol_span(source_symbol))
            .is_some()
            && !scoping
                .scope_flags(scoping.symbol_scope_id(source_symbol))
                .is_strict_mode()
            && scan
                .enclosing_function(scoping.symbol_span(source_symbol))
                .is_some_and(|function| {
                    let at = scan
                        .arguments_reads
                        .partition_point(|span| span.start < function.start);
                    scan.arguments_reads
                        .get(at)
                        .is_some_and(|span| contains(function, *span))
                })
        {
            return None;
        }
        Some(source_symbol)
    }

    /// The alias's reads, when each can read the source binding instead and
    /// the source keeps the copied value up to every one of them.
    fn reads(&self, candidate: &Candidate, source_symbol: SymbolId) -> Option<Vec<Span>> {
        let (scoping, nodes, scan) = (self.scoping, self.nodes, self.scan);
        let function = scan.enclosing_function(candidate.statement)?;
        let references = scoping
            .get_resolved_references(candidate.alias)
            .collect::<Vec<_>>();
        if references.is_empty() || references.iter().any(|reference| reference.is_write()) {
            return None;
        }
        let source_name = scoping.symbol_ident(source_symbol);
        let alias_spans = references
            .iter()
            .map(|reference| nodes.get_node(reference.node_id()).kind().span())
            .collect::<Vec<_>>();
        // A repeated `var x = ...` writes the binding without necessarily
        // producing an IdentifierReference write in the semantic table.
        if scan.initializers.get(&source_symbol).is_some_and(|spans| {
            spans.iter().any(|span| {
                span.start >= candidate.statement.end
                    && (span.start < alias_spans.iter().map(|s| s.end).max().unwrap_or(0)
                        || alias_spans.iter().any(|read| {
                            scan.enclosing_loop(*read) != scan.enclosing_loop(candidate.statement)
                        }))
                    || scan.enclosing_function(*span) != Some(function)
            })
        }) {
            return None;
        }
        let snapshot_safe = self.loop_snapshot(candidate, source_symbol, function, &alias_spans);
        let straight_line_snapshot =
            self.straight_line_snapshot(candidate, source_symbol, function, &alias_spans);
        let branch_snapshot = self.branch_snapshot(candidate, source_symbol, &alias_spans);
        let mut reads = Vec::new();
        for reference in references {
            let span = nodes.get_node(reference.node_id()).kind().span();
            if span.start < candidate.statement.end
                || scan.enclosing_function(span) != Some(function)
                || scan.shorthand_reads.contains(&span)
                || scoping.find_binding(reference.scope_id(), source_name) != Some(source_symbol)
                || (!(snapshot_safe || straight_line_snapshot || branch_snapshot)
                    && scoping.get_resolved_references(source_symbol).any(|write| {
                        let write_span = nodes.get_node(write.node_id()).kind().span();
                        write.is_write()
                            && ((write_span.start >= candidate.statement.end)
                                || scan.enclosing_function(write_span) != Some(function))
                    }))
            {
                return None;
            }
            reads.push(span);
        }
        Some(reads)
    }

    /// A `$CARRY_` copy in a loop whose reads all precede the iteration's
    /// next store to the source (or ride inside that store's right side).
    fn loop_snapshot(
        &self,
        candidate: &Candidate,
        source_symbol: SymbolId,
        function: Span,
        alias_spans: &[Span],
    ) -> bool {
        let (scoping, nodes, scan) = (self.scoping, self.nodes, self.scan);
        let source_name = scoping.symbol_ident(source_symbol);
        scan
            .enclosing_loop(candidate.statement)
            .is_some_and(|loop_span| {
                source_name.starts_with("$CARRY_")
                    && alias_spans.iter().all(|span| contains(loop_span, *span))
                    && alias_spans
                        .iter()
                        .map(|span| span.end)
                        .max()
                        .is_some_and(|last_read| {
                            scoping.get_resolved_references(source_symbol).all(|write| {
                                let span = nodes.get_node(write.node_id()).kind().span();
                                !write.is_write()
                                    || (scan.enclosing_function(span) == Some(function)
                                        && (span.start < candidate.statement.start
                                            || span.start > last_read
                                            || scan.write_rhs.get(&span).is_some_and(|rhs| {
                                                alias_spans
                                                    .iter()
                                                    .filter(|read| read.start > span.start)
                                                    .all(|read| contains(*rhs, *read))
                                            })))
                            })
                        })
            })
    }

    /// A generated slot may be assigned again later in the function. Its
    /// current value is nevertheless stable across a straight-line run of
    /// statements in one block. Do not cross a block/loop boundary or a
    /// write (including a write from a captured closure).
    fn straight_line_snapshot(
        &self,
        candidate: &Candidate,
        source_symbol: SymbolId,
        function: Span,
        alias_spans: &[Span],
    ) -> bool {
        let (scoping, nodes, scan) = (self.scoping, self.nodes, self.scan);
        scan.enclosing_block(candidate.statement)
            .is_some_and(|block| {
                let last_read = alias_spans.iter().map(|span| span.end).max().unwrap();
                alias_spans.iter().all(|span| {
                    scan.enclosing_block(*span) == Some(block)
                        && scan.enclosing_loop(*span)
                            == scan.enclosing_loop(candidate.statement)
                }) && scoping.get_resolved_references(source_symbol).all(|write| {
                    if !write.is_write() {
                        return true;
                    }
                    let span = nodes.get_node(write.node_id()).kind().span();
                    scan.enclosing_function(span) == Some(function)
                        && !(span.start >= candidate.statement.end && span.start < last_read)
                })
            })
    }

    /// Nested conditionals/try arms do not change a private cell by
    /// themselves. Its value stays stable until the final alias read if
    /// no explicit or implicit store can run in this same iteration.
    /// Captured writers remain barriers even when their source lies
    /// outside the textual interval.
    fn branch_snapshot(&self, candidate: &Candidate, source_symbol: SymbolId, alias_spans: &[Span]) -> bool {
        let (scoping, nodes, locals) = (self.scoping, self.nodes, self.locals);
        locals.initialized(source_symbol, candidate.statement)
            && alias_spans
                .iter()
                .all(|at| locals.same_iteration(candidate.statement, *at))
            && scoping.get_resolved_references(source_symbol).all(|r| {
                if !r.is_write() {
                    return true;
                }
                let at = nodes.get_node(r.node_id()).kind().span();
                locals.same_owner(candidate.statement, at)
                    && !(candidate.statement.end <= at.start
                        && at.start < alias_spans.iter().map(|s| s.end).max().unwrap())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::inline_alias_ssa;

    #[test]
    fn inlines_parameter_and_stable_slot_copies_across_statements() {
        let source = "function vm_sub_1_2($PARAMA) { let $SSA_1 = $PARAMA; observe(); return $SSA_1 + $SSA_1; } function f() { let $KEY_1 = 3; let $SSA_2 = $KEY_1; observe(); return $SSA_2; }";
        let output = inline_alias_ssa(source).unwrap();
        assert_eq!(output.inlined_bindings, 2);
        assert!(
            output.code.contains("return $PARAMA + $PARAMA"),
            "{}",
            output.code
        );
        assert!(output.code.contains("return $KEY_1"), "{}", output.code);
    }

    #[test]
    fn retains_snapshot_when_source_changes() {
        let source =
            "function f() { let $KEY_1 = 3; let $SSA_1 = $KEY_1; $KEY_1 = 4; return $SSA_1; }";
        assert_eq!(inline_alias_ssa(source).unwrap().inlined_bindings, 0);
    }

    #[test]
    fn arguments_exposure_keeps_parameter_snapshots_in_its_own_function() {
        let source = "function f($KEY_1) { let $SSA_1 = $KEY_1; let args = arguments; args[0] = 20; return $SSA_1; } function g(x) { let $SSA_2 = x; return $SSA_2; }";
        let result = inline_alias_ssa(source).unwrap();
        assert_eq!(result.inlined_bindings, 1);
        assert!(result.code.contains("let $SSA_1 = $KEY_1"));
        assert!(result.code.contains("return x"));
    }

    #[test]
    fn inlines_slot_snapshot_before_a_later_write() {
        let source = "function f() { let $KEY_1 = 3; let $SSA_1 = $KEY_1; consume($SSA_1); $KEY_1 = 4; return $KEY_1; }";
        let output = inline_alias_ssa(source).unwrap();
        assert_eq!(output.inlined_bindings, 1, "{}", output.code);
        assert!(output.code.contains("consume($KEY_1)"), "{}", output.code);
    }

    #[test]
    fn retains_snapshot_if_branch_or_closure_can_write_before_read() {
        let branch = "function f() { let $KEY_1 = 3; let $SSA_1 = $KEY_1; if (ready()) $KEY_1 = 4; consume($SSA_1); }";
        assert_eq!(inline_alias_ssa(branch).unwrap().inlined_bindings, 0);
        let closure = "function f() { let $KEY_1 = 3; function change() { $KEY_1 = 4; } let $SSA_1 = $KEY_1; change(); consume($SSA_1); }";
        assert_eq!(inline_alias_ssa(closure).unwrap().inlined_bindings, 0);
    }

    #[test]
    fn retains_snapshot_across_loop_backedge() {
        let source = "function f() { let $KEY_1 = 3; let $SSA_1 = $KEY_1; while (ready()) { use($SSA_1); $KEY_1++; } }";
        assert_eq!(inline_alias_ssa(source).unwrap().inlined_bindings, 0);
    }

    #[test]
    fn retains_snapshot_when_next_store_starts_at_declarations_end() {
        for update in ["$KEY_1++", "++$KEY_1", "$KEY_1=7", "$KEY_1+=2"] {
            for name in ["$SSA_copy", "$CARRY_copy"] {
                let source=format!("function f(){{let $KEY_1=3;let {name}=$KEY_1;{update};return [{name},$KEY_1];}}");
                assert_eq!(
                    inline_alias_ssa(&source).unwrap().inlined_bindings,
                    0,
                    "{source}"
                );
            }
        }
    }

    #[test]
    fn retains_read_before_source_initialization() {
        let source = "function f() { let $SSA_1 = $KEY_1; let $KEY_1 = 3; return $SSA_1; }";
        assert_eq!(inline_alias_ssa(source).unwrap().inlined_bindings, 0);
    }

    #[test]
    fn inlines_immutable_carry_copy_but_keeps_loop_carried_value() {
        let source = "function f($PARAMA) { let $CARRY_1 = $PARAMA; return $CARRY_1; } function g($PARAMA) { let $CARRY_2 = $PARAMA; while (ready()) { $CARRY_2 = next(); } return $CARRY_2; }";
        let result = inline_alias_ssa(source).unwrap();
        assert_eq!(result.inlined_bindings, 1);
        assert!(result.code.contains("return $PARAMA"));
        assert!(result.code.contains("let $CARRY_2 = $PARAMA"));
    }

    #[test]
    fn retains_captured_alias_and_shadowed_source() {
        let source = "function f() { let $KEY_1 = 3; let $SSA_1 = $KEY_1; return () => $SSA_1; } function g() { let $KEY_1 = 3; let $SSA_2 = $KEY_1; { let $KEY_1 = 4; use($SSA_2); } }";
        assert_eq!(inline_alias_ssa(source).unwrap().inlined_bindings, 0);
    }

    #[test]
    fn inlines_loop_snapshot_before_first_carry_write() {
        let source = "function f() { let $CARRY_1 = 0; for (;;) { let $SSA_1 = $CARRY_1; if ($SSA_1 > 3) break; use($SSA_1); $CARRY_1 = $SSA_1 + 1; } }";
        let result = inline_alias_ssa(source).unwrap();
        assert_eq!(result.inlined_bindings, 1, "{}", result.code);
        assert!(!result.code.contains("$SSA_1"));
    }

    #[test]
    fn retains_snapshot_if_carry_changes_before_last_read() {
        let source = "function f() { let $CARRY_1 = 0; for (;;) { let $SSA_1 = $CARRY_1; $CARRY_1++; use($SSA_1); } }";
        assert_eq!(inline_alias_ssa(source).unwrap().inlined_bindings, 0);
    }

    #[test]
    fn retains_shorthand_and_dynamic_scope() {
        let shorthand = "function f() { let $KEY_1 = 3; let $SSA_1 = $KEY_1; return {$SSA_1}; }";
        assert_eq!(inline_alias_ssa(shorthand).unwrap().inlined_bindings, 0);
        let eval = "function f() { let $KEY_1 = 3; let $SSA_1 = $KEY_1; return eval('$SSA_1'); }";
        assert_eq!(inline_alias_ssa(eval).unwrap().inlined_bindings, 0);
    }
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && outer.end >= inner.end
}

fn candidate_has_comments(source: &str, span: Span) -> bool {
    source[span.start as usize..span.end as usize].contains("//")
        || source[span.start as usize..span.end as usize].contains("/*")
}

struct Candidate {
    statement: Span,
    alias: SymbolId,
    source: oxc_semantic::ReferenceId,
}

#[derive(Default)]
struct Scan {
    function_bindings: BTreeMap<String, BTreeSet<SymbolId>>,
    initializers: BTreeMap<SymbolId, Vec<Span>>,
    parameters: SpanIndex,
    functions: SpanIndex,
    blocks: SpanIndex,
    loops: SpanIndex,
    write_rhs: BTreeMap<Span, Span>,
    candidates: Vec<Candidate>,
    shorthand_reads: BTreeSet<Span>,
    dynamic_scope: bool,
    arguments_reads: Vec<Span>,
}

impl Scan {
    fn enclosing_function(&self, span: Span) -> Option<Span> {
        self.functions.enclosing(span)
    }

    fn enclosing_loop(&self, span: Span) -> Option<Span> {
        self.loops.enclosing(span)
    }

    fn enclosing_block(&self, span: Span) -> Option<Span> {
        self.blocks.enclosing(span)
    }

    fn body(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            let Statement::VariableDeclaration(declaration) = statement else {
                continue;
            };
            if !matches!(
                declaration.kind,
                VariableDeclarationKind::Let | VariableDeclarationKind::Const
            ) || declaration.declarations.len() != 1
            {
                continue;
            }
            let declarator = &declaration.declarations[0];
            let (BindingPattern::BindingIdentifier(alias), Some(Expression::Identifier(source))) =
                (&declarator.id, declarator.init.as_ref())
            else {
                continue;
            };
            if !(alias.name.starts_with("$SSA_") || alias.name.starts_with("$CARRY_")) {
                continue;
            }
            if let (Some(alias), Some(source)) = (alias.symbol_id.get(), source.reference_id.get())
            {
                self.candidates.push(Candidate {
                    statement: declaration.span,
                    alias,
                    source,
                });
            }
        }
    }
}

impl<'a> Visit<'a> for Scan {
    fn visit_variable_declarator(&mut self, d: &VariableDeclarator<'a>) {
        if d.init.is_some() {
            if let BindingPattern::BindingIdentifier(id) = &d.id {
                if let Some(symbol) = id.symbol_id.get() {
                    self.initializers.entry(symbol).or_default().push(id.span);
                }
            }
        }
        walk::walk_variable_declarator(self, d);
    }
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some(id) = &function.id {
            if let Some(symbol) = id.symbol_id.get() {
                self.function_bindings
                    .entry(id.name.to_string())
                    .or_default()
                    .insert(symbol);
            }
        }
        self.parameters.push(function.params.span);
        self.functions.push(function.span);
        if let Some(body) = &function.body {
            self.blocks.push(body.span);
            self.body(&body.statements);
        }
        walk::walk_function(self, function, flags);
    }
    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.functions.push(arrow.span);
        self.blocks.push(arrow.body.span);
        self.body(&arrow.body.statements);
        walk::walk_arrow_function_expression(self, arrow);
    }
    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.blocks.push(block.span);
        self.body(&block.body);
        walk::walk_block_statement(self, block);
    }
    fn visit_for_statement(&mut self, statement: &ForStatement<'a>) {
        self.loops.push(statement.span);
        walk::walk_for_statement(self, statement);
    }
    fn visit_while_statement(&mut self, statement: &WhileStatement<'a>) {
        self.loops.push(statement.span);
        walk::walk_while_statement(self, statement);
    }
    fn visit_do_while_statement(&mut self, statement: &DoWhileStatement<'a>) {
        self.loops.push(statement.span);
        walk::walk_do_while_statement(self, statement);
    }
    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        if assignment.operator == oxc_syntax::operator::AssignmentOperator::Assign {
            if let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left {
                self.write_rhs.insert(target.span, assignment.right.span());
            }
        }
        walk::walk_assignment_expression(self, assignment);
    }
    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        if property.shorthand {
            if let Expression::Identifier(identifier) = &property.value {
                self.shorthand_reads.insert(identifier.span);
            }
        }
        walk::walk_object_property(self, property);
    }
    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic_scope = true;
        walk::walk_with_statement(self, statement);
    }
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if identifier.name == "arguments" {
            self.arguments_reads.push(identifier.span);
        }
        if identifier.name == "eval" {
            self.dynamic_scope = true;
        }
    }
}
