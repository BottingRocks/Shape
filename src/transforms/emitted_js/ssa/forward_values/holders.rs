//! Fresh-tuple helpers and the bindings that hold their results.

use super::*;

/// Functions whose `arguments[n]` reads cannot change during a call.
#[derive(Default)]
pub(super) struct FixedArguments {
    pub(super) owners: BTreeSet<u32>,
}

impl<'a> Visit<'a> for FixedArguments {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if readonly_arguments(function) {
            self.owners.insert(function.span.start);
        }
        walk::walk_function(self, function, flags);
    }
}

/// `let x = H(…)` declarations where `H` is a fresh-tuple helper.
#[derive(Default)]
pub(super) struct Holders {
    helpers: BTreeMap<SymbolId, usize>,
    pub(super) duplicates: BTreeSet<SymbolId>,
    /// Per fresh helper: its span, parameter symbols, each return slot's
    /// identifier reference (if any), and whether the body is one return of
    /// identifiers and literals.
    returns: Vec<(SymbolId, Span, Vec<Option<SymbolId>>, Vec<Option<ReferenceId>>, bool)>,
    /// (holder, the helper callee's reference)
    holders: Vec<(SymbolId, ReferenceId)>,
}

impl Holders {
    /// The fresh-tuple helpers nothing reassigns.
    pub(super) fn fresh(&self, scoping: &Scoping) -> BTreeSet<SymbolId> {
        self.helpers.keys().copied().filter(|helper| !scoping.symbol_is_mutated(*helper)).collect()
    }

    /// Helpers whose slots return unwritten parameters, never reassigned.
    pub(super) fn passthrough(&self, scoping: &Scoping) -> BTreeMap<SymbolId, PassThrough> {
        let mut out = BTreeMap::new();
        for (helper, span, parameters, slots, shuffle) in &self.returns {
            if scoping.symbol_is_mutated(*helper) {
                continue;
            }
            let slots: Vec<Option<usize>> = slots
                .iter()
                .map(|reference| {
                    let symbol = scoping.get_reference((*reference)?).symbol_id()?;
                    if scoping.symbol_is_mutated(symbol) {
                        return None;
                    }
                    parameters.iter().position(|parameter| *parameter == Some(symbol))
                })
                .collect();
            if slots.iter().any(Option::is_some) {
                let shuffle = *shuffle && slots.iter().all(Option::is_some);
                out.insert(*helper, PassThrough { slots, shuffle, span: *span });
            }
        }
        out
    }

    /// Holders whose helper is never reassigned and whose every reference is
    /// a plain read `x[k]` with an in-range literal `k`.
    pub(super) fn read_only(&self, scoping: &Scoping, nodes: &AstNodes<'_>) -> BTreeMap<SymbolId, usize> {
        let mut out = BTreeMap::new();
        for (holder, callee) in &self.holders {
            let Some(helper) = scoping.get_reference(*callee).symbol_id() else {
                continue;
            };
            let Some(&length) = self.helpers.get(&helper) else {
                continue;
            };
            if scoping.symbol_is_mutated(helper) || scoping.symbol_is_mutated(*holder) {
                continue;
            }
            let read_only = scoping.get_resolved_references(*holder).all(|reference| {
                let node = reference.node_id();
                let span = nodes.kind(node).span();
                let AstKind::ComputedMemberExpression(member) = nodes.parent_kind(node) else {
                    return false;
                };
                let Expression::NumericLiteral(index) = &member.expression else {
                    return false;
                };
                let member_node = nodes.parent_id(node);
                let written = match nodes.parent_kind(member_node) {
                    AstKind::AssignmentExpression(assignment) => {
                        assignment.left.span() == member.span
                    }
                    AstKind::UpdateExpression(_) => true,
                    AstKind::UnaryExpression(unary) => unary.operator == UnaryOperator::Delete,
                    _ => false,
                };
                member.object.span() == span
                    && !member.optional
                    && !written
                    && index.value.fract() == 0.0
                    && index.value >= 0.0
                    && index.value < length as f64
            });
            if read_only {
                out.insert(*holder, length);
            }
        }
        out
    }
}

impl<'a> Visit<'a> for Holders {
    fn visit_function(&mut self, function: &Function<'a>, flags: ScopeFlags) {
        if let Some((symbol, length)) = fresh_tuple_helper(function) {
            self.helpers.insert(symbol, length);
            if returns_one_identifier(function) {
                self.duplicates.insert(symbol);
            }
            if let Some(entry) = return_slots(symbol, function) {
                self.returns.push(entry);
            }
        }
        walk::walk_function(self, function, flags);
    }

    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if let (BindingPattern::BindingIdentifier(binding), Some(Expression::CallExpression(call))) =
            (&declarator.id, declarator.init.as_ref())
        {
            if let (Some(holder), Expression::Identifier(callee)) =
                (binding.symbol_id.get(), &call.callee)
            {
                if let Some(reference) = callee.reference_id.get() {
                    self.holders.push((holder, reference));
                }
            }
        }
        walk::walk_variable_declarator(self, declarator);
    }
}

/// A fresh helper's parameters and return slots, for pass-through analysis.
pub(super) fn return_slots(
    symbol: SymbolId,
    function: &Function<'_>,
) -> Option<(SymbolId, Span, Vec<Option<SymbolId>>, Vec<Option<ReferenceId>>, bool)> {
    let body = function.body.as_ref()?;
    let Some(oxc_ast::ast::Statement::ReturnStatement(ret)) = body.statements.last() else {
        return None;
    };
    let Some(Expression::ArrayExpression(array)) = &ret.argument else {
        return None;
    };
    if function.params.rest.is_some() {
        return None;
    }
    let parameters = function
        .params
        .items
        .iter()
        .map(|parameter| match &parameter.pattern {
            BindingPattern::BindingIdentifier(id) => id.symbol_id.get(),
            _ => None,
        })
        .collect();
    let mut shuffle = body.statements.len() == 1;
    let slots = array
        .elements
        .iter()
        .map(|element| match element {
            ArrayExpressionElement::Identifier(id) => id.reference_id.get(),
            ArrayExpressionElement::NumericLiteral(_)
            | ArrayExpressionElement::StringLiteral(_)
            | ArrayExpressionElement::BooleanLiteral(_)
            | ArrayExpressionElement::NullLiteral(_) => None,
            _ => {
                shuffle = false;
                None
            }
        })
        .collect();
    Some((symbol, function.span, parameters, slots, shuffle))
}

/// `return [v, v, …]` with the same identifier in every slot.
pub(super) fn returns_one_identifier(function: &Function<'_>) -> bool {
    let Some(Some(oxc_ast::ast::Statement::ReturnStatement(ret))) =
        function.body.as_ref().map(|body| body.statements.last())
    else {
        return false;
    };
    let Some(Expression::ArrayExpression(array)) = &ret.argument else {
        return false;
    };
    let mut names = array.elements.iter().map(|element| match element {
        ArrayExpressionElement::Identifier(id) => Some(id.name.as_str()),
        _ => None,
    });
    let Some(Some(first)) = names.next() else {
        return false;
    };
    array.elements.len() > 1 && names.all(|name| name == Some(first))
}
