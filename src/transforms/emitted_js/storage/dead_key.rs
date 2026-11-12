//! Drop writes to generated VM slots that have no resolved reads.
//!
//! Only direct assignment statements to a local `$KEY_` binding are
//! candidates; parameters and top-level statements are excluded. A pure,
//! non-throwing RHS deletes the statement; an identifier resolving to an
//! earlier or function-scoped declaration is also removable; other RHSs stay
//! as `(rhs);`. The binding must have no resolved non-write reference and the
//! statement must be comment-free. `with`/`eval` disables the pass. This
//! removes unread VM slot traffic without deleting effects.

use std::collections::BTreeSet;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::{GetSpan, Span};
use oxc_syntax::{operator::AssignmentOperator, scope::ScopeFlags};

use crate::analysis;

use super::super::purity::expression_is_pure;
use super::super::support::whole_line;

pub(crate) struct DeadKeyResult {
    pub code: String,
    pub removed_stores: usize,
}

/// Only direct assignment statements to local `$KEY` bindings are considered.
/// If the RHS may be observable, it is evaluated in place without the store.
pub(crate) fn remove_unread_key_stores(source: &str) -> Result<DeadKeyResult, String> {
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, source)?;
    let (scoping, _) = analysis::analyze(&program);
    let mut scan = Scan {
        scoping: &scoping,
        functions: Vec::new(),
        local_keys: BTreeSet::new(),
        parameter_symbols: BTreeSet::new(),
        stores: Vec::new(),
        dynamic: false,
    };
    scan.visit_program(&program);
    if scan.dynamic {
        return Ok(DeadKeyResult {
            code: source.into(),
            removed_stores: 0,
        });
    }
    let mut edits = Vec::new();
    for store in &scan.stores {
        if !scan.local_keys.contains(&store.symbol)
            || scan.parameter_symbols.contains(&store.symbol)
            // `+=`, `++` and `--` read the slot too: a reference is a read
            // whenever it reads, written or not.
            || scoping
                .get_resolved_references(store.symbol)
                .any(|reference| reference.is_read())
        {
            continue;
        }
        let text = &source[store.statement.start as usize..store.statement.end as usize];
        if text.contains("/*") || text.contains("//") {
            continue;
        }
        if store.pure {
            edits.push(analysis::Edit::delete(whole_line(source, store.statement)));
        } else {
            let rhs = &source[store.rhs.start as usize..store.rhs.end as usize];
            edits.push(analysis::Edit::replace(
                store.statement,
                format!("({rhs});"),
            ));
        }
    }
    if edits.is_empty() {
        return Ok(DeadKeyResult {
            code: source.into(),
            removed_stores: 0,
        });
    }
    let removed_stores = edits.len();
    let applied = analysis::apply_edits(source, edits);
    if !applied.rejected.is_empty() {
        return Err("unread slot store edits overlap".into());
    }
    let check = Allocator::default();
    analysis::check_program(&check, &applied.output)
        .map_err(|error| format!("unread slot store output: {error}"))?;
    Ok(DeadKeyResult {
        code: applied.output,
        removed_stores,
    })
}

struct Store {
    statement: Span,
    rhs: Span,
    symbol: SymbolId,
    pure: bool,
}

struct Scan<'s> {
    scoping: &'s Scoping,
    functions: Vec<Span>,
    local_keys: BTreeSet<SymbolId>,
    parameter_symbols: BTreeSet<SymbolId>,
    stores: Vec<Store>,
    dynamic: bool,
}

impl Scan<'_> {
    fn current_function(&self) -> Option<Span> {
        self.functions.last().copied()
    }

    fn body(&mut self, statements: &[Statement<'_>]) {
        if self.current_function().is_none() {
            return;
        }
        let mut initialized = BTreeSet::new();
        for statement in statements {
            match statement {
                Statement::VariableDeclaration(declaration) => {
                    for declarator in &declaration.declarations {
                        if let BindingPattern::BindingIdentifier(id) = &declarator.id {
                            if let Some(symbol) = id.symbol_id.get() {
                                initialized.insert(symbol);
                                if id.name.starts_with("$KEY_") {
                                    self.local_keys.insert(symbol);
                                }
                            }
                        }
                    }
                }
                Statement::ExpressionStatement(expr) => {
                    let Expression::AssignmentExpression(assignment) = &expr.expression else {
                        continue;
                    };
                    if assignment.operator != AssignmentOperator::Assign {
                        continue;
                    }
                    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assignment.left
                    else {
                        continue;
                    };
                    if !target.name.starts_with("$KEY_") {
                        continue;
                    }
                    let Some(symbol) = analysis::identifier_symbol(target, self.scoping) else {
                        continue;
                    };
                    let rhs = &assignment.right;
                    let pure = expression_is_pure(rhs)
                        || matches!(rhs, Expression::Identifier(id)
                            if analysis::identifier_symbol(id, self.scoping).is_some_and(|source_symbol|
                                initialized.contains(&source_symbol)
                                || self.scoping.symbol_flags(source_symbol).is_function_scoped_declaration()));
                    self.stores.push(Store {
                        statement: statement.span(),
                        rhs: rhs.span(),
                        symbol,
                        pure,
                    });
                }
                _ => {}
            }
        }
    }
}

impl<'a> Visit<'a> for Scan<'_> {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        self.functions.push(function.span);
        for parameter in &function.params.items {
            if let BindingPattern::BindingIdentifier(id) = &parameter.pattern {
                if let Some(symbol) = id.symbol_id.get() {
                    self.parameter_symbols.insert(symbol);
                }
            }
        }
        if let Some(body) = &function.body {
            self.body(&body.statements);
        }
        walk::walk_function(self, function, flags);
        self.functions.pop();
    }

    fn visit_block_statement(&mut self, block: &BlockStatement<'a>) {
        self.body(&block.body);
        walk::walk_block_statement(self, block);
    }

    fn visit_identifier_reference(&mut self, id: &IdentifierReference<'a>) {
        if id.name == "eval" {
            self.dynamic = true;
        }
    }

    fn visit_with_statement(&mut self, statement: &WithStatement<'a>) {
        self.dynamic = true;
        walk::walk_with_statement(self, statement);
    }
}

#[cfg(test)]
mod tests {
    use super::remove_unread_key_stores;

    #[test]
    fn removes_unread_slot_stores_without_discarding_effects() {
        let source =
            "function f(x) { let $KEY_1; $KEY_1 = 3; $KEY_1 = x; $KEY_1 = effect(); return 1; }";
        let result = remove_unread_key_stores(source).unwrap();
        assert_eq!(result.removed_stores, 3, "{}", result.code);
        assert!(!result.code.contains("$KEY_1 ="));
        assert!(result.code.contains("(effect());"));
    }

    #[test]
    fn keeps_slot_store_when_read_or_dynamic_scope_exists() {
        let live = "function f() { let $KEY_1; $KEY_1 = 3; return $KEY_1; }";
        assert_eq!(remove_unread_key_stores(live).unwrap().removed_stores, 0);
        let dynamic = "function f() { let $KEY_1; $KEY_1 = 3; return eval('$KEY_1'); }";
        assert_eq!(remove_unread_key_stores(dynamic).unwrap().removed_stores, 0);
    }

    #[test]
    fn keeps_a_store_a_compound_assignment_reads() {
        for step in ["$KEY_1 += 2;", "$KEY_1++;", "--$KEY_1;", "$KEY_1 ??= 1;"] {
            let source = format!("function f(o) {{ let $KEY_1; $KEY_1 = o; {step} }}");
            let result = remove_unread_key_stores(&source).unwrap();
            assert_eq!(result.removed_stores, 0, "{step}: {}", result.code);
        }
    }
}
