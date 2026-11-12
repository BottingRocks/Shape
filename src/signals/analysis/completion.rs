//! Conservative proof for a registered callback that only completes its observer.
//! Absence of reconstructed emissions alone is deliberately insufficient.
use super::*;
use oxc_ast::{ast::*, AstKind};
use oxc_span::GetSpan;

pub(super) fn prove(
    contract: &SignalContract,
    nodes: &oxc_semantic::AstNodes<'_>,
    scoping: &oxc_semantic::Scoping,
) -> bool {
    if !matches!(contract.producer.kind, ProducerKind::Direct { .. })
        || !contract.emissions.is_empty()
        || contract.producer.registrations.len() != 1
    {
        return false;
    }
    let registration = &contract.producer.registrations[0];
    let Some(callback) = registration.callback else {
        return false;
    };
    let Some(arrow) = nodes.iter().find_map(|n| match n.kind() {
        AstKind::ArrowFunctionExpression(a)
            if a.span.start == callback.start && a.span.end == callback.end =>
        {
            Some(a)
        }
        _ => None,
    }) else {
        return false;
    };
    if arrow.r#async || arrow.params.items.len() != 1 || arrow.params.rest.is_some() {
        return false;
    }
    let BindingPattern::BindingIdentifier(observer) = &arrow.params.items[0].pattern else {
        return false;
    };
    let Some(symbol) = observer.symbol_id.get() else {
        return false;
    };
    let Some(Statement::ExpressionStatement(first)) = arrow.body.statements.first() else {
        return false;
    };
    let Expression::CallExpression(call) = first.expression.without_parentheses() else {
        return false;
    };
    let Expression::StaticMemberExpression(member) = call.callee.without_parentheses() else {
        return false;
    };
    if member.property.name != "complete"
        || !call.arguments.is_empty()
        || crate::analysis::expression_symbol(&member.object, scoping) != Some(symbol)
    {
        return false;
    }
    let references: Vec<_> = scoping.get_resolved_references(symbol).collect();
    // The observer cannot escape, be aliased, emit, or be used from another callback.
    if references.len() != 1
        || !references[0].is_read()
        || nodes.get_node(references[0].node_id()).span() != member.object.span()
    {
        return false;
    }
    // Require the producer's sole registrar invocation to be its sole use of
    // that registrar, and its only callback expression. No hidden registrations.
    let Some(body) = contract.producer.body else {
        return false;
    };
    let Some(producer) = nodes.iter().find_map(|n| match n.kind() {
        AstKind::ArrowFunctionExpression(a)
            if a.span.start == body.start && a.span.end == body.end =>
        {
            Some(a)
        }
        _ => None,
    }) else {
        return false;
    };
    if producer.params.items.len() != 2 || producer.params.rest.is_some() || producer.r#async {
        return false;
    }
    let BindingPattern::BindingIdentifier(registrar) = &producer.params.items[1].pattern else {
        return false;
    };
    let Some(registrar) = registrar.symbol_id.get() else {
        return false;
    };
    let refs: Vec<_> = scoping.get_resolved_references(registrar).collect();
    refs.len() == 1 && nodes.iter().any(|n| matches!(n.kind(), AstKind::CallExpression(c)
        if c.span.start == registration.call.start && c.span.end == registration.call.end
            && c.arguments.len() == 1 && c.arguments[0].as_expression().is_some_and(|e| e.span() == arrow.span)
            && crate::analysis::expression_symbol(&c.callee, scoping) == Some(registrar)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signals::extract::contracts::{Producer, Registration};
    use crate::signals::extract::wiring::LifecycleRole;
    fn accepts(body: &str) -> bool {
        let source = format!("const producer=(context,init)=>{{init(observer=>{{{body}}});}};");
        let arena = oxc_allocator::Allocator::default();
        let parsed = crate::analysis::parse(&arena, &source);
        assert!(parsed.errors.is_empty());
        let (scopes, nodes) = crate::analysis::analyze(&parsed.program);
        let producer = nodes
            .iter()
            .find_map(|n| match n.kind() {
                AstKind::ArrowFunctionExpression(a) if a.params.items.len() == 2 => Some(a.span),
                _ => None,
            })
            .unwrap();
        let callback = nodes
            .iter()
            .find_map(|n| match n.kind() {
                AstKind::ArrowFunctionExpression(a) if a.params.items.len() == 1 => Some(a.span),
                _ => None,
            })
            .unwrap();
        let call = nodes
            .iter()
            .find_map(|n| match n.kind() {
                AstKind::CallExpression(c)
                    if c.arguments
                        .first()
                        .and_then(Argument::as_expression)
                        .is_some_and(|e| e.span() == callback) =>
                {
                    Some(c.span)
                }
                _ => None,
            })
            .unwrap();
        let contract = SignalContract {
            descriptor: 0,
            producer: Producer {
                kind: ProducerKind::Direct { wrapper: None },
                body: Some(producer.into()),
                registrations: vec![Registration {
                    role: LifecycleRole::Init,
                    callback: Some(callback.into()),
                    call: call.into(),
                }],
                arguments: vec![],
            },
            status: ContractStatus::Unsupported,
            reasons: vec![],
            emissions: vec![],
            lifecycle: vec![],
            accounting: vec![],
            input_classes: vec![],
        };
        prove(&contract, &nodes, &scopes)
    }
    #[test]
    fn proves_observer_identity_without_promoting_unexplained_no_emission() {
        assert!(accepts("observer.complete(); sideEffect();"));
        for body in [
            "unrelated.complete();",
            "observer.next({}); observer.complete();",
            "observer.complete(); escape(observer);",
            "let alias=observer; alias.complete();",
            "if(flag) observer.complete();",
            "observer.complete(); observer.next({});",
        ] {
            assert!(!accepts(body), "{body}");
        }
    }
}
