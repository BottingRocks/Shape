//! Bounded Oxc indexing shared by structural seed pinning.
//!
//! This is the read-only view `pin` needs: the functions, call expressions,
//! conditionals and static definitions of one parsed program. There is no
//! canonicalization or template comparison here; those proved a clean program
//! against reviewed source shapes and were removed with the contract templates.
use oxc_ast::{ast::*, AstKind};
use oxc_semantic::{Scoping, SymbolId};
use oxc_span::Span;
use std::collections::HashMap;

pub(super) struct Index<'a, 's> {
    pub nodes: &'a oxc_semantic::AstNodes<'a>,
    pub source: &'s str,
    pub scopes: &'a Scoping,
    pub functions: Vec<&'a Function<'a>>,
    pub calls: Vec<&'a CallExpression<'a>>,
    pub conditionals: Vec<&'a ConditionalExpression<'a>>,
    /// Last static value per symbol, sorted by position. Only declarations with
    /// an initializer and plain identifier assignments are recorded.
    pub definitions: HashMap<SymbolId, Vec<(u32, &'a Expression<'a>)>>,
}

impl<'a, 's> Index<'a, 's> {
    pub fn new(
        source: &'s str,
        scopes: &'a Scoping,
        nodes: &'a oxc_semantic::AstNodes<'a>,
    ) -> Self {
        let mut index = Self {
            nodes,
            source,
            scopes,
            functions: vec![],
            calls: vec![],
            conditionals: vec![],
            definitions: HashMap::new(),
        };
        for node in nodes.iter() {
            match node.kind() {
                AstKind::Function(function) => index.functions.push(function),
                AstKind::CallExpression(call) => index.calls.push(call),
                AstKind::ConditionalExpression(conditional) => index.conditionals.push(conditional),
                AstKind::VariableDeclarator(declarator) => {
                    if let (BindingPattern::BindingIdentifier(binding), Some(init)) =
                        (&declarator.id, &declarator.init)
                    {
                        if let Some(symbol) = binding.symbol_id.get() {
                            index
                                .definitions
                                .entry(symbol)
                                .or_default()
                                .push((declarator.span.start, init));
                        }
                    }
                }
                AstKind::AssignmentExpression(assign) => {
                    if let AssignmentTarget::AssignmentTargetIdentifier(id) = &assign.left {
                        if let Some(symbol) = crate::analysis::identifier_symbol(id, scopes) {
                            index
                                .definitions
                                .entry(symbol)
                                .or_default()
                                .push((assign.span.start, &assign.right));
                        }
                    }
                }
                _ => {}
            }
        }
        for values in index.definitions.values_mut() {
            values.sort_by_key(|value| value.0);
        }
        index
    }

    pub fn symbol(&self, e: &Expression<'a>) -> Option<SymbolId> {
        crate::analysis::expression_symbol(e.without_parentheses(), self.scopes)
    }
}

pub(super) fn within(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && outer.end >= inner.end
}

pub(super) fn member_name<'b, 'a>(e: &'b Expression<'a>) -> Option<&'b str> {
    match e.without_parentheses() {
        Expression::StaticMemberExpression(member) => Some(member.property.name.as_str()),
        Expression::ComputedMemberExpression(member) => match &member.expression {
            Expression::StringLiteral(string) => Some(string.value.as_str()),
            _ => None,
        },
        _ => None,
    }
}

pub(super) fn member_object<'b, 'a>(e: &'b Expression<'a>) -> Option<&'b Expression<'a>> {
    match e.without_parentheses() {
        Expression::StaticMemberExpression(member) => Some(&member.object),
        Expression::ComputedMemberExpression(member) => Some(&member.object),
        _ => None,
    }
}
