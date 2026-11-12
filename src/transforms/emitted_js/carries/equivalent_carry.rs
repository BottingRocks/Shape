//! Merge carries that receive the same value at every paired store.
//!
//! Candidates are two or more `$CARRY_` bindings with byte-identical literal/
//! null initializers (or no initializer) and lockstep stores of the
//! same value. Every left store must have a paired right store; literals must
//! match byte-for-byte and identifiers must resolve to the same non-merged
//! symbol. Right-hand reads must re-resolve to the left binding in their own
//! scope and must not be shorthand properties. The right declarator and paired
//! stores are deleted and reads are renamed. This removes duplicate carry
//! storage; comments and overlaps decline. Intervening carry stores qualify
//! only when their reads and writes are initialized and harmless, neither
//! observe the merged cells nor change the shared source value.

use std::collections::BTreeSet;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{AstNodes, ReferenceId, Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::AssignmentOperator;

use crate::analysis;
use super::super::support::whole_line;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EquivalentCarryResult {
    pub code: String,
    pub removed: usize,
}

/// Combine two carries declared together when every write is a consecutive,
/// matching local store. There can be no observable read between those stores.
pub(crate) fn merge_equivalent_carries(source: &str) -> Result<EquivalentCarryResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, nodes) = analysis::analyze(&program);
    let mut local_reads = super::super::adjacent_ssa::LocalReads::default();
    local_reads.visit_program(&program);
    local_reads.build(&scoping, &nodes, &BTreeSet::new());
    let mut scan = Scan::default();
    scan.visit_program(&program);
    if scan.dynamic_scope {
        return Ok(EquivalentCarryResult {
            code: source.into(),
            removed: 0,
        });
    }
    let mut edits = Vec::new();
    let mut selected = BTreeSet::new();
    let mut removed = 0;
    for declaration in &scan.declarations {
        if source[declaration.statement.start as usize..declaration.statement.end as usize]
            .contains("/*")
            || source[declaration.statement.start as usize..declaration.statement.end as usize]
                .contains("//")
        {
            continue;
        }
        for (left_index, left) in declaration.bindings.iter().enumerate() {
            if selected.contains(&left.symbol) {
                continue;
            }
            for (right_index, right) in declaration.bindings.iter().enumerate().skip(left_index + 1)
            {
                if selected.contains(&right.symbol)
                    || !same_initial_value(source, left.init, right.init)
                {
                    continue;
                }
                let left_writes = writes(left.symbol, &scoping, &nodes);
                let right_writes = writes(right.symbol, &scoping, &nodes);
                if left_writes.len() != right_writes.len() {
                    continue;
                }
                let mut paired = Vec::new();
                let mut valid = true;
                for left_write in &left_writes {
                    let Some(pair) = scan.pairs.iter().find(|pair| {
                        scoping.get_reference(pair.first.target).symbol_id() == Some(left.symbol)
                            && scoping.get_reference(pair.second.target).symbol_id()
                                == Some(right.symbol)
                            && pair.first.target_span == *left_write
                            && right_writes.contains(&pair.second.target_span)
                            && same_rhs(source, &scoping, pair, left.symbol, right.symbol)
                            && safe_gap(pair, left.symbol, right.symbol, &scoping, &local_reads)
                    }) else {
                        valid = false;
                        break;
                    };
                    if source
                        [pair.second.statement.start as usize..pair.second.statement.end as usize]
                        .contains("/*")
                        || source[pair.second.statement.start as usize
                            ..pair.second.statement.end as usize]
                            .contains("//")
                    {
                        valid = false;
                        break;
                    }
                    paired.push(pair);
                }
                if !valid || paired.len() != right_writes.len() {
                    continue;
                }
                let left_name = scoping.symbol_ident(left.symbol);
                let mut read_edits = Vec::new();
                for reference in scoping.get_resolved_references(right.symbol) {
                    let span = nodes.get_node(reference.node_id()).kind().span();
                    if scan.shorthand_reads.contains(&span)
                        || scoping.find_binding(reference.scope_id(), left_name)
                            != Some(left.symbol)
                    {
                        valid = false;
                        break;
                    }
                    if !reference.is_write() {
                        read_edits.push(analysis::Edit::replace(span, left_name));
                    }
                }
                if !valid {
                    continue;
                }
                let removal = if right_index + 1 < declaration.bindings.len() {
                    Span::new(
                        right.span.start,
                        declaration.bindings[right_index + 1].span.start,
                    )
                } else {
                    Span::new(
                        declaration.bindings[right_index - 1].span.end,
                        right.span.end,
                    )
                };
                let mut proposal = vec![analysis::Edit::delete(removal)];
                proposal.extend(paired.iter().map(|pair| {
                    analysis::Edit::delete(whole_line(source, pair.second.statement))
                }));
                proposal.extend(read_edits);
                if proposal.iter().any(|edit| {
                    edits.iter().any(|other: &analysis::Edit| {
                        edit.start < other.end && other.start < edit.end
                    })
                }) {
                    continue;
                }
                edits.extend(proposal);
                selected.insert(right.symbol);
                removed += 1;
            }
        }
    }
    if edits.is_empty() {
        return Ok(EquivalentCarryResult {
            code: source.into(),
            removed: 0,
        });
    }
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("equivalent carry edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("equivalent carry output: {error}"))?;
    Ok(EquivalentCarryResult {
        code: applied.output,
        removed,
    })
}

fn writes(symbol: SymbolId, scoping: &Scoping, nodes: &AstNodes<'_>) -> Vec<Span> {
    scoping
        .get_resolved_references(symbol)
        .filter(|reference| reference.is_write())
        .map(|reference| nodes.get_node(reference.node_id()).kind().span())
        .collect()
}

fn same_initial_value(source: &str, left: Option<Span>, right: Option<Span>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            source[left.start as usize..left.end as usize]
                == source[right.start as usize..right.end as usize]
        }
        _ => false,
    }
}

fn same_rhs(
    source: &str,
    scoping: &Scoping,
    pair: &StorePair,
    left: SymbolId,
    right: SymbolId,
) -> bool {
    match (&pair.first.value, &pair.second.value) {
        (StoreValue::Literal, StoreValue::Literal) => {
            source[pair.first.rhs.start as usize..pair.first.rhs.end as usize]
                == source[pair.second.rhs.start as usize..pair.second.rhs.end as usize]
        }
        (StoreValue::Identifier(a), StoreValue::Identifier(b)) => {
            let a = scoping.get_reference(*a).symbol_id();
            a.is_some()
                && a == scoping.get_reference(*b).symbol_id()
                && a != Some(left)
                && a != Some(right)
        }
        _ => false,
    }
}

struct Binding {
    symbol: SymbolId,
    span: Span,
    init: Option<Span>,
}
struct Declaration {
    statement: Span,
    bindings: Vec<Binding>,
}
#[derive(Clone, Copy)]
enum StoreValue {
    Literal,
    Identifier(ReferenceId),
}
#[derive(Clone)]
struct Store {
    statement: Span,
    target: ReferenceId,
    target_span: Span,
    rhs: Span,
    value: StoreValue,
}
struct StorePair {
    first: Store,
    second: Store,
    middle: Vec<Store>,
}
#[derive(Default)]
struct Scan {
    declarations: Vec<Declaration>,
    pairs: Vec<StorePair>,
    shorthand_reads: BTreeSet<Span>,
    dynamic_scope: bool,
}
impl Scan {
    fn body(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            let Statement::VariableDeclaration(declaration) = statement else {
                continue;
            };
            if declaration.kind != VariableDeclarationKind::Let {
                continue;
            }
            let mut bindings = Vec::new();
            let mut valid = true;
            for declarator in &declaration.declarations {
                let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
                    valid = false;
                    break;
                };
                if !binding.name.starts_with("$CARRY_") {
                    valid = false;
                    break;
                }
                let Some(symbol) = binding.symbol_id.get() else {
                    valid = false;
                    break;
                };
                let init = match declarator.init.as_ref() {
                    None => None,
                    Some(Expression::NullLiteral(literal)) => Some(literal.span),
                    Some(Expression::BooleanLiteral(literal)) => Some(literal.span),
                    Some(Expression::NumericLiteral(literal)) => Some(literal.span),
                    Some(Expression::StringLiteral(literal)) => Some(literal.span),
                    _ => {
                        valid = false;
                        break;
                    }
                };
                bindings.push(Binding {
                    symbol,
                    span: declarator.span,
                    init,
                });
            }
            if valid && bindings.len() >= 2 {
                self.declarations.push(Declaration {
                    statement: declaration.span,
                    bindings,
                });
            }
        }
        for (index, statement) in statements.iter().enumerate() {
            let Some(first) = store(statement) else {
                continue;
            };
            let mut middle = Vec::new();
            for next in statements.iter().skip(index + 1).take(16) {
                let Some(second) = store(next) else {
                    break;
                };
                self.pairs.push(StorePair {
                    first: first.clone(),
                    second: second.clone(),
                    middle: middle.clone(),
                });
                middle.push(second);
            }
        }
    }
}

fn safe_gap(
    pair: &StorePair,
    left: SymbolId,
    right: SymbolId,
    scoping: &Scoping,
    locals: &super::super::adjacent_ssa::LocalReads,
) -> bool {
    if pair.middle.is_empty() {
        return true;
    }
    if !locals.initialized(left, pair.first.statement)
        || !locals.initialized(right, pair.first.statement)
    {
        return false;
    }
    let source = match pair.first.value {
        StoreValue::Identifier(id) => scoping.get_reference(id).symbol_id(),
        StoreValue::Literal => None,
    };
    pair.middle.iter().all(|store| {
        let Some(target) = scoping.get_reference(store.target).symbol_id() else {
            return false;
        };
        if target == left
            || target == right
            || Some(target) == source
            || scoping.symbol_flags(target).is_const_variable()
            || !locals.initialized(target, store.statement)
        {
            return false;
        }
        match store.value {
            StoreValue::Literal => true,
            StoreValue::Identifier(id) => {
                scoping.get_reference(id).symbol_id().is_some_and(|value| {
                    value != left
                        && value != right
                        && (Some(value) == source || locals.initialized(value, store.statement))
                })
            }
        }
    })
}
fn store(statement: &Statement<'_>) -> Option<Store> {
    let Statement::ExpressionStatement(expression) = statement else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = &expression.expression else {
        return None;
    };
    if assignment.operator != AssignmentOperator::Assign {
        return None;
    }
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left else {
        return None;
    };
    if !target.name.starts_with("$CARRY_") {
        return None;
    }
    let value = match &assignment.right {
        Expression::Identifier(identifier) => {
            StoreValue::Identifier(identifier.reference_id.get()?)
        }
        Expression::NullLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::StringLiteral(_) => StoreValue::Literal,
        _ => return None,
    };
    Some(Store {
        statement: statement.span(),
        target: target.reference_id.get()?,
        target_span: target.span,
        rhs: assignment.right.span(),
        value,
    })
}
impl<'a> Visit<'a> for Scan {
    fn visit_program(&mut self, program: &Program<'a>) {
        self.body(&program.body);
        walk::walk_program(self, program);
    }
    fn visit_function(&mut self, function: &Function<'a>, flags: oxc_syntax::scope::ScopeFlags) {
        if let Some(body) = &function.body {
            self.body(&body.statements);
        }
        walk::walk_function(self, function, flags);
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
    use super::merge_equivalent_carries;

    #[test]
    fn merges_matching_branch_values() {
        let source = "function f(p,x) { let $CARRY_a, $CARRY_b; if (p) { $CARRY_a = x; $CARRY_b = x; } else { $CARRY_a = null; $CARRY_b = null; } return [$CARRY_a,$CARRY_b]; }";
        let result = merge_equivalent_carries(source).unwrap();
        assert_eq!(result.removed, 1, "{}", result.code);
        assert!(!result.code.contains("$CARRY_b"));
    }

    #[test]
    fn merges_interleaved_parallel_copies_only_when_the_gap_cannot_observe_or_throw() {
        let source="function f(p){let $CARRY_a,$CARRY_b,$CARRY_c;const x=7,y=8;if(p){$CARRY_a=x;$CARRY_c=y;$CARRY_b=x;}else{$CARRY_a=1;$CARRY_c=2;$CARRY_b=1;}return [$CARRY_a,$CARRY_b,$CARRY_c];}";
        let result = merge_equivalent_carries(source).unwrap();
        assert_eq!(result.removed, 1, "{}", result.code);
        assert!(!result.code.contains("$CARRY_b"));
        for source in [
            "function f(){let $CARRY_a,$CARRY_b,$CARRY_c;$CARRY_a=1;$CARRY_c=$CARRY_b;$CARRY_b=1;return [$CARRY_a,$CARRY_b,$CARRY_c];}",
            "function f(){let $CARRY_a,$CARRY_b,$CARRY_c;$CARRY_a=1;$CARRY_c=missing;$CARRY_b=1;return [$CARRY_a,$CARRY_b,$CARRY_c];}",
            "function f(){let $CARRY_a,$CARRY_b; $CARRY_a=1;$CARRY_c=2;$CARRY_b=1;let $CARRY_c;return [$CARRY_a,$CARRY_b];}",
            "function f(){let $CARRY_x=0,$CARRY_a,$CARRY_b;$CARRY_a=$CARRY_x;$CARRY_x=1;$CARRY_b=$CARRY_x;return [$CARRY_a,$CARRY_b];}",
        ]{assert_eq!(merge_equivalent_carries(source).unwrap().removed,0,"{source}");}
    }

    #[test]
    fn retains_nonadjacent_or_effectful_stores() {
        let nonadjacent = "function f(x) { let $CARRY_a, $CARRY_b; $CARRY_a=x; observe(); $CARRY_b=x; return $CARRY_b; }";
        assert_eq!(merge_equivalent_carries(nonadjacent).unwrap().removed, 0);
        let effectful = "function f() { let $CARRY_a, $CARRY_b; $CARRY_a=next(); $CARRY_b=next(); return $CARRY_b; }";
        assert_eq!(merge_equivalent_carries(effectful).unwrap().removed, 0);
    }
}
