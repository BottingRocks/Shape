//! Remove generated carry bindings whose values are never read.
//!
//! A candidate is a `let $CARRY_*` binding with no resolved non-write
//! references. Every store must be a plain statement assignment after the
//! declaration; stores in expressions such as `return $CARRY = 1` decline. A
//! pure, non-throwing RHS is deleted; an identifier resolving to an earlier
//! same-function declaration is deleted; any other RHS remains as `(rhs);` at
//! the same point. Effectful initializers survive as `(init);`. Comments and
//! dynamic scope decline. The pass removes dead carry scaffolding while
//! preserving evaluation order, and callers repeat it to a bounded fixpoint.

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, ReferenceId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::{operator::AssignmentOperator, scope::ScopeFlags};

use super::super::index::SpanIndex;
use crate::analysis;

use super::super::purity;
use super::super::support::whole_line;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeadCarryResult {
    pub code: String,
    pub removed_carries: usize,
}

/// Prune unread carry stores. An effectful or possibly throwing right-hand
/// side is retained as an expression statement at its original position.
pub(crate) fn remove_dead_carries(source: &str) -> Result<DeadCarryResult, String> {
    // Where `//` and `/*` start: a statement whose text holds either is left
    // alone (the same text test as before, answered by binary search).
    let marks: Vec<u32> = source
        .match_indices("//")
        .chain(source.match_indices("/*"))
        .map(|(at, _)| at as u32)
        .collect::<std::collections::BTreeSet<u32>>()
        .into_iter()
        .collect();
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut scan = Scan::default();
    scan.visit_program(&program);
    scan.functions.build();
    if scan.dynamic_scope {
        return Ok(DeadCarryResult {
            code: source.into(),
            removed_carries: 0,
        });
    }

    let declaration_spans: BTreeMap<_, _> = scan
        .declarations
        .iter()
        .flat_map(|declaration| {
            declaration
                .bindings
                .iter()
                .map(move |binding| (binding.symbol, declaration.statement))
        })
        .collect();
    let pruning = Pruning {
        source,
        scoping: &scoping,
        nodes: &nodes,
        scan: &scan,
        marks,
        declaration_spans,
    };
    let (selected, mut edits) = pruning.select();
    edits.extend(pruning.bare_read_deletions());
    if selected.is_empty() && edits.is_empty() {
        return Ok(DeadCarryResult {
            code: source.into(),
            removed_carries: 0,
        });
    }
    let removed = pruning.declaration_edits(&selected, &mut edits);
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("dead carry edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("dead carry output: {error}"))?;
    Ok(DeadCarryResult {
        code: applied.output,
        removed_carries: removed,
    })
}

/// One parse's facts for choosing dead carries and the edits removing them.
struct Pruning<'s, 'a> {
    source: &'s str,
    scoping: &'s Scoping,
    nodes: &'s AstNodes<'a>,
    scan: &'s Scan,
    marks: Vec<u32>,
    declaration_spans: BTreeMap<SymbolId, Span>,
}
impl Pruning<'_, '_> {
    fn commented(&self, span: Span) -> bool {
        let at = self.marks.partition_point(|mark| *mark < span.start);
        self.marks.get(at).is_some_and(|mark| *mark + 2 <= span.end)
    }
    /// Whether `value` reads a binding declared before `at` in its function.
    fn stable_before(&self, value: ReferenceId, at: Span) -> bool {
        self.scoping.get_reference(value).symbol_id().is_some_and(|symbol| {
            self.declaration_spans
                .get(&symbol)
                .is_some_and(|source_declaration| {
                    source_declaration.end < at.start
                        && self.scan.enclosing_function(*source_declaration)
                            == self.scan.enclosing_function(at)
                })
        })
    }
    /// The dead carries and the edits to their stores.
    ///
    /// A copy into a removed carry is deleted with it, which can leave the
    /// carry it read unread too; selection repeats over this one parse until
    /// no carry is added, so a relay chain goes in a single pass.
    fn select(&self) -> (BTreeSet<SymbolId>, Vec<analysis::Edit>) {
        let mut selected = BTreeSet::new();
        let mut dropped_reads = BTreeSet::new();
        let mut write_edits = Vec::new();
        loop {
            let before = selected.len();
            for declaration in &self.scan.declarations {
                if self.commented(declaration.statement) {
                    continue;
                }
                for binding in &declaration.bindings {
                    if !binding.name.starts_with("$CARRY_") || selected.contains(&binding.symbol)
                    {
                        continue;
                    }
                    let Some((changes, dropped)) =
                        self.dead_stores(declaration, binding, &dropped_reads)
                    else {
                        continue;
                    };
                    selected.insert(binding.symbol);
                    dropped_reads.extend(dropped);
                    write_edits.extend(changes);
                }
            }
            if selected.len() == before {
                break;
            }
        }
        self.select_cycles(&mut selected, &dropped_reads, &mut write_edits);
        (selected, write_edits)
    }
    /// Carries that read only each other: a loop-carried pair
    /// (`$CARRY_b = $CARRY_a; … $CARRY_a = $CARRY_b;`) is unread outside its
    /// own copies, but neither goes first one at a time. Assume every
    /// remaining carry with removable stores dead, then keep each one a read
    /// outside the assumed-dead copies needs, until nothing changes; what
    /// stays assumed dead is read only by its own deleted copies.
    fn select_cycles(
        &self,
        selected: &mut BTreeSet<SymbolId>,
        dropped_reads: &BTreeSet<ReferenceId>,
        write_edits: &mut Vec<analysis::Edit>,
    ) {
        let every_read: &dyn Fn(ReferenceId) -> bool = &|_| true;
        let mut assumed: Vec<(SymbolId, Vec<analysis::Edit>, Vec<ReferenceId>)> = Vec::new();
        for declaration in &self.scan.declarations {
            if self.commented(declaration.statement) {
                continue;
            }
            for binding in &declaration.bindings {
                if !binding.name.starts_with("$CARRY_") || selected.contains(&binding.symbol) {
                    continue;
                }
                if let Some((changes, dropped)) = self.stores_of(declaration, binding, every_read) {
                    assumed.push((binding.symbol, changes, dropped));
                }
            }
        }
        loop {
            let dropped: BTreeSet<ReferenceId> = dropped_reads
                .iter()
                .copied()
                .chain(assumed.iter().flat_map(|(_, _, dropped)| dropped.iter().copied()))
                .collect();
            let before = assumed.len();
            assumed.retain(|(symbol, _, _)| {
                self.scoping.get_resolved_reference_ids(*symbol).iter().all(|reference| {
                    self.scoping.get_reference(*reference).is_write() || dropped.contains(reference)
                })
            });
            if assumed.len() == before {
                break;
            }
        }
        for (symbol, changes, _) in assumed {
            selected.insert(symbol);
            write_edits.extend(changes);
        }
    }
    /// The edits removing an unread binding's stores, and the reads deleted
    /// with them; `None` when it is read or a store is not a plain statement.
    fn dead_stores(
        &self,
        declaration: &Declaration,
        binding: &Binding,
        dropped_reads: &BTreeSet<ReferenceId>,
    ) -> Option<(Vec<analysis::Edit>, Vec<ReferenceId>)> {
        self.stores_of(declaration, binding, &|reference| dropped_reads.contains(&reference))
    }
    /// [`Self::dead_stores`] with `dropped` telling which reads go.
    fn stores_of(
        &self,
        declaration: &Declaration,
        binding: &Binding,
        dropped_read: &dyn Fn(ReferenceId) -> bool,
    ) -> Option<(Vec<analysis::Edit>, Vec<ReferenceId>)> {
        let (source, scoping, scan) = (self.source, self.scoping, self.scan);
        let refs: Vec<_> = scoping
            .get_resolved_reference_ids(binding.symbol)
            .iter()
            .copied()
            .collect();
        if refs.iter().any(|reference| {
            !scoping.get_reference(*reference).is_write() && !dropped_read(*reference)
        }) {
            return None;
        }
        let mut changes = Vec::new();
        let mut dropped = Vec::new();
        for reference in refs {
            let reference = scoping.get_reference(reference);
            if !reference.is_write() {
                continue;
            }
            let span = self.nodes.get_node(reference.node_id()).kind().span();
            let store = scan.stores.get(&span)?;
            if scoping.get_reference(store.target).symbol_id() != Some(binding.symbol)
                || store.statement.start < declaration.statement.start
                || self.commented(store.statement)
            {
                return None;
            }
            let deleted = store.pure
                || store
                    .identifier
                    .is_some_and(|value| self.stable_before(value, store.statement));
            let replacement = if deleted {
                dropped.extend(store.identifier);
                String::new()
            } else {
                format!(
                    "({});",
                    &source[store.rhs.start as usize..store.rhs.end as usize]
                )
            };
            let span = if replacement.is_empty() {
                whole_line(source, store.statement)
            } else {
                store.statement
            };
            changes.push(analysis::Edit::replace(span, replacement));
        }
        if let Some(value) = binding.identifier {
            if binding.pure_initializer || self.stable_before(value, declaration.statement) {
                dropped.push(value);
            }
        }
        Some((changes, dropped))
    }
    /// Deletions of bare reads of a binding declared earlier in the function.
    fn bare_read_deletions(&self) -> Vec<analysis::Edit> {
        let mut edits = Vec::new();
        for bare in &self.scan.bare_reads {
            let Some(symbol) = self.scoping.get_reference(bare.reference).symbol_id() else {
                continue;
            };
            if !self.declaration_spans.get(&symbol).is_some_and(|declaration| {
                declaration.end < bare.statement.start
                    && self.scan.enclosing_function(*declaration)
                        == self.scan.enclosing_function(bare.statement)
            }) {
                continue;
            }
            if self.commented(bare.statement) {
                continue;
            }
            edits.push(analysis::Edit::delete(whole_line(
                self.source,
                bare.statement,
            )));
        }
        edits
    }
    /// Rewrite each declaration holding a selected carry, keeping effectful
    /// initializers and the other bindings; returns the carries removed.
    fn declaration_edits(
        &self,
        selected: &BTreeSet<SymbolId>,
        edits: &mut Vec<analysis::Edit>,
    ) -> usize {
        let source = self.source;
        let mut removed = 0;
        for declaration in &self.scan.declarations {
            let removed_here = declaration
                .bindings
                .iter()
                .filter(|b| selected.contains(&b.symbol))
                .count();
            if removed_here == 0 {
                continue;
            }
            if self.commented(declaration.statement) {
                continue;
            }
            let mut parts = Vec::new();
            for binding in &declaration.bindings {
                if selected.contains(&binding.symbol) {
                    if let Some(initializer) = binding.initializer {
                        let stable_identifier = binding
                            .identifier
                            .is_some_and(|value| self.stable_before(value, declaration.statement));
                        if !binding.pure_initializer && !stable_identifier {
                            parts.push(format!(
                                "({});",
                                &source[initializer.start as usize..initializer.end as usize]
                            ));
                        }
                    }
                } else {
                    parts.push(format!(
                        "let {};",
                        &source[binding.span.start as usize..binding.span.end as usize]
                    ));
                }
            }
            let replacement = parts.join(" ");
            let span = if replacement.is_empty() {
                whole_line(source, declaration.statement)
            } else {
                declaration.statement
            };
            edits.push(analysis::Edit::replace(span, replacement));
            removed += removed_here;
        }
        removed
    }
}

struct Binding {
    span: Span,
    symbol: SymbolId,
    name: String,
    initializer: Option<Span>,
    identifier: Option<ReferenceId>,
    pure_initializer: bool,
}
struct Declaration {
    statement: Span,
    bindings: Vec<Binding>,
}
struct Store {
    statement: Span,
    target: ReferenceId,
    rhs: Span,
    pure: bool,
    identifier: Option<ReferenceId>,
}
struct BareRead {
    statement: Span,
    reference: ReferenceId,
}
#[derive(Default)]
struct Scan {
    declarations: Vec<Declaration>,
    stores: BTreeMap<Span, Store>,
    bare_reads: Vec<BareRead>,
    functions: SpanIndex,
    dynamic_scope: bool,
}
impl Scan {
    fn enclosing_function(&self, span: Span) -> Option<Span> {
        self.functions.enclosing(span)
    }
    fn body(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            match statement {
                Statement::VariableDeclaration(declaration)
                    if declaration.kind == VariableDeclarationKind::Let =>
                {
                    let mut bindings = Vec::new();
                    for declarator in &declaration.declarations {
                        let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
                            bindings.clear();
                            break;
                        };
                        let Some(symbol) = binding.symbol_id.get() else {
                            bindings.clear();
                            break;
                        };
                        bindings.push(Binding {
                            span: declarator.span,
                            symbol,
                            name: binding.name.to_string(),
                            initializer: declarator.init.as_ref().map(GetSpan::span),
                            identifier: declarator.init.as_ref().and_then(|expression| {
                                if let Expression::Identifier(id) = expression {
                                    id.reference_id.get()
                                } else {
                                    None
                                }
                            }),
                            pure_initializer: declarator
                                .init
                                .as_ref()
                                .is_none_or(purity::expression_is_pure),
                        });
                    }
                    if !bindings.is_empty() {
                        self.declarations.push(Declaration {
                            statement: declaration.span,
                            bindings,
                        });
                    }
                }
                Statement::ExpressionStatement(expr) => {
                    if let Expression::Identifier(identifier) = &expr.expression {
                        if let Some(reference) = identifier.reference_id.get() {
                            self.bare_reads.push(BareRead {
                                statement: statement.span(),
                                reference,
                            });
                        }
                        continue;
                    }
                    let Expression::AssignmentExpression(assign) = &expr.expression else {
                        continue;
                    };
                    if assign.operator != AssignmentOperator::Assign {
                        continue;
                    }
                    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assign.left else {
                        continue;
                    };
                    let Some(target_id) = target.reference_id.get() else {
                        continue;
                    };
                    let identifier = match &assign.right {
                        Expression::Identifier(id) => id.reference_id.get(),
                        _ => None,
                    };
                    self.stores.insert(
                        target.span,
                        Store {
                            statement: statement.span(),
                            target: target_id,
                            rhs: assign.right.span(),
                            pure: purity::expression_is_pure(&assign.right),
                            identifier,
                        },
                    );
                }
                _ => {}
            }
        }
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
    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.body(&block.body);
        walk::walk_block_statement(self, block);
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
    use super::remove_dead_carries;
    #[test]
    fn removes_unread_carries_and_preserves_effects() {
        let source = "function f(x) { let $CARRY_a, $CARRY_b; $CARRY_a = 1; $CARRY_b = effect(); return x; }";
        let result = remove_dead_carries(source).unwrap();
        assert_eq!(result.removed_carries, 2);
        assert!(!result.code.contains("$CARRY_"), "{}", result.code);
        assert!(result.code.contains("(effect());"), "{}", result.code);
    }
    #[test]
    fn removes_a_relay_chain_in_one_pass() {
        let source = "function f(x) {\n  let $CARRY_a = x;\n  let $CARRY_b, $CARRY_c;\n  if (x) {\n    $CARRY_b = $CARRY_a;\n  } else {\n    $CARRY_b = 1;\n  }\n  $CARRY_c = $CARRY_b;\n  return x;\n}";
        let result = remove_dead_carries(source).unwrap();
        assert_eq!(result.removed_carries, 3, "{}", result.code);
        assert!(!result.code.contains("$CARRY_"), "{}", result.code);
    }
    #[test]
    fn removes_a_loop_carried_pair_that_reads_only_itself() {
        // `$CARRY_a` and `$CARRY_b` copy each other around the loop and
        // nothing else reads them; `$CARRY_k` is read by the return.
        let source = "function f(x, buf) {\n  let $CARRY_a = x;\n  let $CARRY_k = 0;\n  for (let i = 0; i < 3; i++) {\n    let $CARRY_b;\n    if (x) {\n      $CARRY_b = $CARRY_k + 1;\n    } else {\n      $CARRY_b = $CARRY_a;\n    }\n    if (i) {\n      $CARRY_a = buf;\n    } else {\n      $CARRY_a = $CARRY_b;\n    }\n    $CARRY_k = $CARRY_k + 1;\n  }\n  return $CARRY_k;\n}";
        let result = remove_dead_carries(source).unwrap();
        assert_eq!(result.removed_carries, 2, "{}", result.code);
        assert!(!result.code.contains("$CARRY_a") && !result.code.contains("$CARRY_b"), "{}", result.code);
        assert!(result.code.contains("return $CARRY_k;"), "{}", result.code);
    }
    #[test]
    fn keeps_a_cycle_read_outside_itself() {
        let source = "function f(x) {\n  let $CARRY_a = x;\n  let $CARRY_b;\n  for (let i = 0; i < 3; i++) {\n    $CARRY_b = $CARRY_a;\n    $CARRY_a = $CARRY_b;\n  }\n  return $CARRY_b;\n}";
        assert_eq!(remove_dead_carries(source).unwrap().removed_carries, 0);
    }
    #[test]
    fn keeps_a_carry_read_by_a_retained_store() {
        // `$CARRY_b` is declared in another function than the store, so the
        // store survives as `($CARRY_b);` and that read keeps `$CARRY_b`.
        let source = "function f(x) { let $CARRY_b = 1; function g() { let $CARRY_c; $CARRY_c = $CARRY_b; } return g; }";
        let result = remove_dead_carries(source).unwrap();
        assert_eq!(result.removed_carries, 1, "{}", result.code);
        assert!(result.code.contains("let $CARRY_b = 1;"), "{}", result.code);
        assert!(result.code.contains("($CARRY_b);"), "{}", result.code);
    }
    #[test]
    fn retains_read_carry_and_nonstatement_write() {
        let source = "function f(x) { let $CARRY_a; $CARRY_a = x; return $CARRY_a; } function g() { let $CARRY_b; return $CARRY_b = 1; }";
        assert_eq!(remove_dead_carries(source).unwrap().removed_carries, 0);
    }
    #[test]
    fn removes_initialized_dead_carry_without_dropping_initializer_effects() {
        let source =
            "function f() { let $CARRY_a = effect(), keep = 2, $CARRY_b = 3; return keep; }";
        let result = remove_dead_carries(source).unwrap();
        assert_eq!(result.removed_carries, 2);
        assert!(!result.code.contains("$CARRY_"), "{}", result.code);
        assert!(
            result.code.contains("(effect()); let keep = 2;"),
            "{}",
            result.code
        );
    }
}
