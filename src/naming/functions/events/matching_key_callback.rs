//! A generic key guard, not a storage-event or callback-purpose claim.
//! The retained string is arbitrary data. Only this whole owned body supplies
//! behavior; the captured function supplies callable identity and reaching order.
use super::*;
use oxc_syntax::operator::BinaryOperator;

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    if matches_key(model, site).is_none() {
        return vec![];
    }
    vec![proposal(
        "invokeCapturedCallbackOnMatchingKey",
        "shared_matching_key_callback",
        "whole owned body strictly compares the unchanged sole formal's key to a retained string literal and invokes a lexically captured stable function with zero arguments; callable origin reaches every exposure; no callback-purpose or event-source claim",
        facts,
    )]
}

fn matches_key(model: &Model<'_, '_>, site: FunctionSite) -> Option<()> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return None;
    };
    let body = function.body.as_ref()?;
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || body.directives.iter().any(|d| d.directive != "use strict")
    {
        return None;
    }
    let [parameter] = function.params.items.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(id) = &parameter.pattern else {
        return None;
    };
    let parameter_symbol = id.symbol_id.get()?;
    if parameter.initializer.is_some()
        || model.binding_writes(parameter_symbol) != 0
        || !model
            .scoping
            .symbol_redeclarations(parameter_symbol)
            .is_empty()
    {
        return None;
    }
    let [Statement::IfStatement(guard)] = body.statements.as_slice() else {
        return None;
    };
    if guard.alternate.is_some() {
        return None;
    }
    let Expression::BinaryExpression(test) = guard.test.without_parentheses() else {
        return None;
    };
    if test.operator != BinaryOperator::StrictEquality
        || !matches!(
            test.right.without_parentheses(),
            Expression::StringLiteral(_)
        )
    {
        return None;
    }
    let Expression::StaticMemberExpression(member) = test.left.without_parentheses() else {
        return None;
    };
    if member.optional
        || member.property.name != "key"
        || crate::analysis::expression_symbol(member.object.without_parentheses(), model.scoping)
            != Some(parameter_symbol)
    {
        return None;
    }
    let Statement::BlockStatement(consequent) = &guard.consequent else {
        return None;
    };
    let [Statement::ExpressionStatement(statement)] = consequent.body.as_slice() else {
        return None;
    };
    let Expression::CallExpression(call) = statement.expression.without_parentheses() else {
        return None;
    };
    if call.optional || !call.arguments.is_empty() {
        return None;
    }
    let Expression::Identifier(callee) = call.callee.without_parentheses() else {
        return None;
    };
    let capture = crate::analysis::identifier_symbol(callee, model.scoping)?;
    if site
        .span
        .contains_inclusive(model.scoping.symbol_span(capture))
    {
        return None;
    }
    let origin = stable_function(model, call.callee.without_parentheses(), 0)?;
    if site.span.contains_inclusive(origin.span) || origin.span.contains_inclusive(site.span) {
        return None;
    }
    // Reads of the captured alias must be direct zero-argument invocations.
    // Passing/storing it, reading a property, or constructing it is an escape.
    // stable_function separately proves the sole initializer/assignment and
    // rejects mutation, redeclaration and use before initialization.
    for reference in model.scoping.get_resolved_references(capture) {
        if reference.is_write() {
            continue;
        }
        let node = reference.node_id();
        let AstKind::IdentifierReference(identifier) = model.nodes.kind(node) else {
            return None;
        };
        let AstKind::CallExpression(use_call) = model.nodes.parent_kind(node) else {
            return None;
        };
        if use_call.optional
            || !use_call.arguments.is_empty()
            || !matches!(&use_call.callee, Expression::Identifier(i) if i.span == identifier.span)
        {
            return None;
        }
    }
    Some(())
}
