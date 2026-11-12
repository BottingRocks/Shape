//! Coalesce a branch-local carry into its sole destination carry.
//!
//! Two forms are accepted: writes to an inner carry followed by
//! `$CARRY_outer = $CARRY_inner;`, and an initialized relay that starts as the
//! outer carry and copies back after a loop. The outer binding must already be
//! declared; every inner reference must be in the copy's function; writes must
//! be plain assignments before the copy; and the only inner reads must be the
//! copy source or reads nested in an RHS that moves with its write. The
//! initialized form also requires same-block visibility, no overlapping `try`,
//! no exiting jump, and no outer write in the moved region. The inner
//! declaration and copy are deleted and writes are renamed. Comments and
//! overlaps decline. This removes intermediate carry relays; callers repeat it
//! a bounded number of times.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, ReferenceId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::{operator::AssignmentOperator, scope::ScopeFlags};

use super::super::index::SpanIndex;
use crate::analysis;
use super::super::support::whole_line;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CarryCoalesceResult {
    pub code: String,
    pub removed_carries: usize,
}

/// Turn `let inner; if (p) inner = a; else inner = b; outer = inner`
/// into direct writes to `outer`. The inner binding must have exactly one
/// read, all its writes must be plain assignments, and the destination must
/// be visible at every write without an earlier observable read.
pub(crate) fn coalesce_carry_copies(source: &str) -> Result<CarryCoalesceResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = Scan::default();
    scan.visit_program(&program);
    scan.functions.build();
    if scan.dynamic_scope {
        return Ok(CarryCoalesceResult {
            code: source.into(),
            removed_carries: 0,
        });
    }
    let mut edits = Vec::new();
    let mut selected = BTreeSet::new();
    let mut count = 0;
    for copy in &scan.copies {
        let Some(inner) = scoping.get_reference(copy.source).symbol_id() else {
            continue;
        };
        let Some(outer) = scoping.get_reference(copy.target).symbol_id() else {
            continue;
        };
        if inner == outer || selected.contains(&inner) || selected.contains(&outer) {
            continue;
        }
        let Some(declaration) = scan.declarations.get(&inner) else {
            continue;
        };
        let Some(outer_declaration) = scan.declarations.get(&outer) else {
            continue;
        };
        if scoping.symbol_span(outer).start >= declaration.binding.start
            || declaration.binding.start >= copy.statement.start
            || !outer_declaration.binding_name.starts_with("$CARRY_")
            || !declaration.binding_name.starts_with("$CARRY_")
            || source[declaration.removal.start as usize..declaration.removal.end as usize]
                .contains("/*")
            || source[declaration.removal.start as usize..declaration.removal.end as usize]
                .contains("//")
            || source[copy.statement.start as usize..copy.statement.end as usize].contains("/*")
            || source[copy.statement.start as usize..copy.statement.end as usize].contains("//")
        {
            continue;
        }
        if declaration.initialized {
            let Some(proposal) = coalesce_initialized_relay(
                source,
                &scan,
                &scoping,
                &nodes,
                copy,
                declaration,
                inner,
                outer,
            ) else {
                continue;
            };
            if overlaps_any(&proposal, &edits) {
                continue;
            }
            edits.extend(proposal);
            selected.insert(inner);
            count += 1;
            continue;
        }
        let Some(writes) = branch_writes(&scan, &scoping, &nodes, copy, declaration, inner, outer)
        else {
            continue;
        };
        let outer_name = scoping.symbol_ident(outer);
        let removal = declaration.removal;
        if selected.iter().any(|symbol| {
            let Some(other) = scan.declarations.get(symbol) else {
                return false;
            };
            overlaps(removal, other.removal) || overlaps(copy.statement, other.removal)
        }) {
            continue;
        }
        let mut proposal = vec![
            analysis::Edit::delete(removal),
            analysis::Edit::delete(whole_line(source, copy.statement)),
        ];
        for write in writes {
            proposal.push(analysis::Edit::replace(write, outer_name));
        }
        // A chain `c = b; … b = a;` coalesces one link per pass: renaming
        // `b`'s writes would edit the copy `b = a` another link deletes.
        if overlaps_any(&proposal, &edits) {
            continue;
        }
        edits.extend(proposal);
        selected.insert(inner);
        count += 1;
    }
    if edits.is_empty() {
        return Ok(CarryCoalesceResult {
            code: source.into(),
            removed_carries: 0,
        });
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("carry coalescing edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("carry coalescing output: {error}"))?;
    Ok(CarryCoalesceResult {
        code: applied.output,
        removed_carries: count,
    })
}

/// The inner carry's writes when each is a plain assignment between its
/// declaration and the copy, the copy is its only read, and nothing observes
/// the outer carry between the first moved write and the copy.
fn branch_writes(
    scan: &Scan,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    copy: &Copy,
    declaration: &Declaration,
    inner: SymbolId,
    outer: SymbolId,
) -> Option<Vec<Span>> {
    let function = scan.enclosing_function(copy.statement);
    let inner_refs: Vec<_> = scoping.get_resolved_references(inner).collect();
    let mut writes = Vec::new();
    let mut reads = Vec::new();
    let outer_name = scoping.symbol_ident(outer);
    for reference in inner_refs {
        let span = nodes.get_node(reference.node_id()).kind().span();
        if scan.enclosing_function(span) != function {
            return None;
        }
        if reference.is_write() {
            if span.start <= declaration.binding.start
                || span.end >= copy.statement.start
                || !scan.plain_assignments.contains(&span)
                || scoping.find_binding(reference.scope_id(), outer_name.clone()) != Some(outer)
            {
                return None;
            }
            writes.push(span);
        } else {
            reads.push(span);
        }
    }
    if writes.is_empty() || reads != [copy.source_span] {
        return None;
    }
    let first_write = writes.iter().map(|span| span.start).min().unwrap();
    // Reads before the first inner write still see the old destination.
    // Reads in an inner assignment's RHS also see the old destination:
    // JavaScript evaluates the RHS before assigning the destination. All
    // other observations between a moved write and the old copy forbid
    // coalescing, as do intervening destination writes and captures.
    if scoping.get_resolved_references(outer).any(|reference| {
        let span = nodes.get_node(reference.node_id()).kind().span();
        scan.enclosing_function(span) != function
            || (span.start >= first_write
                && span.start < copy.statement.start
                && if reference.is_write() {
                    span != copy.target_span
                } else {
                    !writes.iter().any(|write| {
                        scan.plain_assignment_rhs
                            .get(write)
                            .is_some_and(|rhs| contains(*rhs, span))
                    })
                })
    }) {
        return None;
    }
    Some(writes)
}

/// Whether an edit of `proposal` overlaps one already taken.
fn overlaps_any(proposal: &[analysis::Edit], edits: &[analysis::Edit]) -> bool {
    proposal.iter().any(|edit| {
        edits
            .iter()
            .any(|other: &analysis::Edit| edit.start < other.end && other.start < edit.end)
    })
}

/// Eliminate a loop-local snapshot when the outer carry is unobserved until
/// the snapshot is copied back. Replacing *all* inner reads and writes is safe
/// only if jumps cannot bypass that copy and a catch cannot observe an early
/// outer write.
fn coalesce_initialized_relay(
    source: &str,
    scan: &Scan,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    copy: &Copy,
    declaration: &Declaration,
    inner: SymbolId,
    outer: SymbolId,
) -> Option<Vec<analysis::Edit>> {
    let initial = declaration.initial_source?;
    if scoping.get_reference(initial).symbol_id() != Some(outer)
        || declaration.body != copy.body
        || scan.tries.iter().any(|span| {
            overlaps(
                *span,
                Span::new(declaration.binding.end, copy.statement.start),
            )
        })
        || scan.jump_exits(Span::new(declaration.binding.end, copy.statement.start))
    {
        return None;
    }
    let function = scan.enclosing_function(copy.statement);
    let outer_name = scoping.symbol_ident(outer);
    let mut refs = Vec::new();
    let mut first_write = copy.statement.start;
    for reference in scoping.get_resolved_references(inner) {
        let span = nodes.get_node(reference.node_id()).kind().span();
        if scan.enclosing_function(span) != function
            || span.start < declaration.binding.end
            || (span.start >= copy.statement.start && span != copy.source_span)
            || scan.shorthand_reads.contains(&span)
            || scoping.find_binding(reference.scope_id(), outer_name.clone()) != Some(outer)
        {
            return None;
        }
        if reference.is_write() {
            first_write = first_write.min(span.start);
        }
        if span != copy.source_span {
            refs.push(span);
        }
    }
    // A direct read of the outer carry after an inner write would see the old
    // value before this rewrite and the new one afterward. RHS reads of that
    // same inner assignment still see the old value, so they are safe.
    for reference in scoping.get_resolved_references(outer) {
        let span = nodes.get_node(reference.node_id()).kind().span();
        if scan.enclosing_function(span) != function {
            return None;
        }
        if span.start < declaration.binding.end || span.start >= copy.statement.start {
            continue;
        }
        if reference.is_write() {
            return None;
        }
        if span.start >= first_write
            && !refs.iter().any(|write| {
                scan.plain_assignment_rhs
                    .get(write)
                    .is_some_and(|rhs| contains(*rhs, span))
            })
        {
            return None;
        }
    }
    let mut edits = vec![
        analysis::Edit::delete(whole_line(source, declaration.removal)),
        analysis::Edit::delete(whole_line(source, copy.statement)),
    ];
    edits.extend(
        refs.into_iter()
            .map(|span| analysis::Edit::replace(span, outer_name)),
    );
    Some(edits)
}

fn overlaps(a: Span, b: Span) -> bool {
    a.start < b.end && b.start < a.end
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && outer.end >= inner.end
}

struct Declaration {
    binding: Span,
    binding_name: String,
    removal: Span,
    initialized: bool,
    initial_source: Option<ReferenceId>,
    body: Span,
}
struct Copy {
    statement: Span,
    source: ReferenceId,
    source_span: Span,
    target: ReferenceId,
    target_span: Span,
    body: Span,
}
struct Jump {
    span: Span,
    label: Option<String>,
    is_continue: bool,
}
#[derive(Default)]
struct Scan {
    declarations: BTreeMap<SymbolId, Declaration>,
    copies: Vec<Copy>,
    plain_assignments: BTreeSet<Span>,
    plain_assignment_rhs: BTreeMap<Span, Span>,
    shorthand_reads: BTreeSet<Span>,
    functions: SpanIndex,
    loops: Vec<Span>,
    switches: Vec<Span>,
    labels: Vec<(String, Span)>,
    jumps: Vec<Jump>,
    tries: Vec<Span>,
    dynamic_scope: bool,
}
impl Scan {
    fn enclosing_function(&self, span: Span) -> Option<Span> {
        self.functions.enclosing(span)
    }
    fn body(&mut self, statements: &[Statement<'_>]) {
        let Some((first, last)) = statements.first().zip(statements.last()) else {
            return;
        };
        let body = Span::new(first.span().start, last.span().end);
        for statement in statements {
            match statement {
                Statement::VariableDeclaration(declaration)
                    if declaration.kind == VariableDeclarationKind::Let =>
                {
                    for (i, declarator) in declaration.declarations.iter().enumerate() {
                        let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
                            continue;
                        };
                        let Some(symbol) = binding.symbol_id.get() else {
                            continue;
                        };
                        let removal = if declaration.declarations.len() == 1 {
                            declaration.span
                        } else if i + 1 < declaration.declarations.len() {
                            Span::new(
                                declarator.span.start,
                                declaration.declarations[i + 1].span.start,
                            )
                        } else {
                            Span::new(
                                declaration.declarations[i - 1].span.end,
                                declarator.span.end,
                            )
                        };
                        self.declarations.insert(
                            symbol,
                            Declaration {
                                binding: binding.span,
                                binding_name: binding.name.to_string(),
                                removal,
                                initialized: declarator.init.is_some(),
                                initial_source: declarator.init.as_ref().and_then(|init| {
                                    let Expression::Identifier(identifier) = init else {
                                        return None;
                                    };
                                    identifier.reference_id.get()
                                }),
                                body,
                            },
                        );
                    }
                }
                Statement::ExpressionStatement(expr) => {
                    let Expression::AssignmentExpression(assign) = &expr.expression else {
                        continue;
                    };
                    if assign.operator != AssignmentOperator::Assign {
                        continue;
                    }
                    let (
                        AssignmentTarget::AssignmentTargetIdentifier(target),
                        Expression::Identifier(value),
                    ) = (&assign.left, &assign.right)
                    else {
                        continue;
                    };
                    if !(target.name.starts_with("$CARRY_") && value.name.starts_with("$CARRY_")) {
                        continue;
                    }
                    let target_span = target.span;
                    if let (Some(target), Some(source)) =
                        (target.reference_id.get(), value.reference_id.get())
                    {
                        self.copies.push(Copy {
                            statement: statement.span(),
                            source,
                            source_span: value.span,
                            target,
                            target_span,
                            body,
                        });
                    }
                }
                _ => {}
            }
        }
    }
    fn jump_exits(&self, region: Span) -> bool {
        self.jumps.iter().any(|jump| {
            if !contains(region, jump.span) {
                return false;
            }
            if let Some(label) = &jump.label {
                !self.labels.iter().any(|(name, span)| {
                    name == label && contains(region, *span) && contains(*span, jump.span)
                })
            } else {
                let mut targets = self.loops.iter();
                let nearest = targets
                    .by_ref()
                    .chain(if jump.is_continue {
                        [].iter()
                    } else {
                        self.switches.iter()
                    })
                    .copied()
                    .filter(|span| contains(*span, jump.span))
                    .min_by_key(|span| span.end - span.start);
                !nearest.is_some_and(|span| contains(region, span))
            }
        })
    }
}
impl<'a> Visit<'a> for Scan {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        self.functions.push(function.span);
        if let Some(body) = &function.body {
            self.body(&body.statements);
        }
        walk::walk_function(self, function, flags);
    }
    fn visit_arrow_function_expression(&mut self, arrow: &ArrowFunctionExpression<'a>) {
        self.functions.push(arrow.span);
        self.body(&arrow.body.statements);
        walk::walk_arrow_function_expression(self, arrow);
    }
    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.body(&block.body);
        walk::walk_block_statement(self, block);
    }
    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        if property.shorthand {
            if let Expression::Identifier(identifier) = &property.value {
                self.shorthand_reads.insert(identifier.span);
            }
        }
        walk::walk_object_property(self, property);
    }
    fn visit_labeled_statement(&mut self, statement: &LabeledStatement<'a>) {
        self.labels
            .push((statement.label.name.to_string(), statement.span));
        walk::walk_labeled_statement(self, statement);
    }
    fn visit_break_statement(&mut self, statement: &BreakStatement<'a>) {
        self.jumps.push(Jump {
            span: statement.span,
            label: statement.label.as_ref().map(|label| label.name.to_string()),
            is_continue: false,
        });
    }
    fn visit_continue_statement(&mut self, statement: &ContinueStatement<'a>) {
        self.jumps.push(Jump {
            span: statement.span,
            label: statement.label.as_ref().map(|label| label.name.to_string()),
            is_continue: true,
        });
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
    fn visit_for_in_statement(&mut self, statement: &ForInStatement<'a>) {
        self.loops.push(statement.span);
        walk::walk_for_in_statement(self, statement);
    }
    fn visit_for_of_statement(&mut self, statement: &ForOfStatement<'a>) {
        self.loops.push(statement.span);
        walk::walk_for_of_statement(self, statement);
    }
    fn visit_switch_statement(&mut self, statement: &SwitchStatement<'a>) {
        self.switches.push(statement.span);
        walk::walk_switch_statement(self, statement);
    }
    fn visit_try_statement(&mut self, statement: &TryStatement<'a>) {
        self.tries.push(statement.span);
        walk::walk_try_statement(self, statement);
    }
    fn visit_assignment_expression(&mut self, assignment: &AssignmentExpression<'a>) {
        if assignment.operator == AssignmentOperator::Assign {
            if let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left {
                self.plain_assignments.insert(target.span);
                self.plain_assignment_rhs
                    .insert(target.span, assignment.right.span());
            }
        }
        walk::walk_assignment_expression(self, assignment);
    }
    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic_scope = true;
        walk::walk_with_statement(self, statement);
    }
    fn visit_identifier_reference(&mut self, identifier: &IdentifierReference<'a>) {
        if identifier.name == "eval" {
            self.dynamic_scope = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::coalesce_carry_copies;
    #[test]
    fn coalesces_nested_branch_copy() {
        let source = "function f(p, a, b) { let $CARRY_outer; if (p) { let $CARRY_inner; if (a) { $CARRY_inner = 1; } else { $CARRY_inner = 2; } $CARRY_outer = $CARRY_inner; } else { $CARRY_outer = b; } return $CARRY_outer; }";
        let result = coalesce_carry_copies(source).unwrap();
        assert_eq!(result.removed_carries, 1);
        assert!(!result.code.contains("$CARRY_inner"), "{}", result.code);
        assert!(result.code.contains("$CARRY_outer = 1"));
    }
    #[test]
    fn retains_read_before_copy_and_capture() {
        let source = "function f(p) { let $CARRY_outer; let $CARRY_inner; $CARRY_inner = 1; use($CARRY_outer); $CARRY_outer = $CARRY_inner; return $CARRY_outer; }";
        assert_eq!(coalesce_carry_copies(source).unwrap().removed_carries, 0);
        let captured = "function f() { let $CARRY_outer; let $CARRY_inner; $CARRY_inner = 1; let g = () => $CARRY_outer; $CARRY_outer = $CARRY_inner; return g(); }";
        assert_eq!(coalesce_carry_copies(captured).unwrap().removed_carries, 0);
    }

    #[test]
    fn coalesces_loop_relay_with_old_value_on_an_alternate_arm() {
        let source = "function f(p) { let $CARRY_outer = 0; for (;;) { let $CARRY_inner; if (p) { $CARRY_inner = 7; } else { $CARRY_inner = $CARRY_outer; } $CARRY_outer = $CARRY_inner; if (done()) break; } return $CARRY_outer; }";
        let result = coalesce_carry_copies(source).unwrap();
        assert_eq!(result.removed_carries, 1, "{}", result.code);
        assert!(!result.code.contains("$CARRY_inner"));
        assert!(result.code.contains("$CARRY_outer = $CARRY_outer"));
    }

    #[test]
    fn retains_intervening_read_or_write() {
        let read = "function f(p) { let $CARRY_outer; let $CARRY_inner; if (p) { $CARRY_inner = 1; } else { $CARRY_inner = 2; } use($CARRY_outer); $CARRY_outer = $CARRY_inner; }";
        assert_eq!(coalesce_carry_copies(read).unwrap().removed_carries, 0);
        let write = "function f() { let $CARRY_outer; let $CARRY_inner; $CARRY_inner = 1; $CARRY_outer = 2; $CARRY_outer = $CARRY_inner; return $CARRY_outer; }";
        assert_eq!(coalesce_carry_copies(write).unwrap().removed_carries, 0);
        let capture = "function f() { let $CARRY_outer; let g = () => $CARRY_outer; let $CARRY_inner; $CARRY_inner = 1; g(); $CARRY_outer = $CARRY_inner; }";
        assert_eq!(coalesce_carry_copies(capture).unwrap().removed_carries, 0);
    }

    #[test]
    fn coalesces_initialized_relay_around_loop() {
        let source = "function f(n) { let $CARRY_outer = 0; { let $CARRY_inner = $CARRY_outer; loop: for (;;) { if ($CARRY_inner < n) { $CARRY_inner = $CARRY_inner + 1; continue loop; } break loop; } $CARRY_outer = $CARRY_inner; } return $CARRY_outer; }";
        let result = coalesce_carry_copies(source).unwrap();
        assert_eq!(result.removed_carries, 1, "{}", result.code);
        assert!(!result.code.contains("$CARRY_inner"));
        assert!(result.code.contains("$CARRY_outer = $CARRY_outer + 1"));
    }

    #[test]
    fn retains_initialized_relay_when_old_outer_is_observed_or_copy_skipped() {
        let read = "function f(p) { let $CARRY_outer = 0; { let $CARRY_inner = $CARRY_outer; $CARRY_inner = 1; use($CARRY_outer); $CARRY_outer = $CARRY_inner; } return $CARRY_outer; }";
        assert_eq!(coalesce_carry_copies(read).unwrap().removed_carries, 0);
        let jump = "function f(p) { let $CARRY_outer = 0; exit: { let $CARRY_inner = $CARRY_outer; $CARRY_inner = 1; if (p) break exit; $CARRY_outer = $CARRY_inner; } return $CARRY_outer; }";
        assert_eq!(coalesce_carry_copies(jump).unwrap().removed_carries, 0);
        let caught = "function f() { let $CARRY_outer = 0; try { let $CARRY_inner = $CARRY_outer; $CARRY_inner = mightThrow(); $CARRY_outer = $CARRY_inner; } catch {} return $CARRY_outer; }";
        assert_eq!(coalesce_carry_copies(caught).unwrap().removed_carries, 0);
    }
}
