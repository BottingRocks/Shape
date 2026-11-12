//! One small source-index-3 frontier rule for relay subscription consumers.
//!
//! The rule describes the owned consumer protocol of a factory callback.  It
//! deliberately does not identify the constructor expression as Observable or
//! as any other native/API role: only the exact callback input and forwarding
//! body are proved.
use super::*;
use oxc_semantic::SymbolId;

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e, model.scoping)
}

fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = e else {
        return None;
    };
    (!m.optional && m.property.name == key).then_some(&m.object)
}

// This is mutable captured DATA, not a callable-origin proof.  The declaration
// must be a plain var/let/const binding in a function/program statement list;
// parameters, loop bindings, resources, destructuring and redeclarations are
// intentionally refused.
fn captured_data_slot(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
) -> Option<SymbolId> {
    let slot = symbol(model, e)?;
    if site
        .span
        .contains_inclusive(model.scoping.symbol_span(slot))
        || !model.scoping.symbol_redeclarations(slot).is_empty()
    {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(slot);
    let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&variable.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(slot))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(d)
                if matches!(
                    d.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                )
        )
    {
        return None;
    }
    let declaration_statement = model.nodes.parent_id(declaration);
    let container = model.nodes.parent_id(declaration_statement);
    if !matches!(
        model.nodes.kind(container),
        AstKind::FunctionBody(_) | AstKind::Program(_)
    ) {
        return None;
    }
    Some(slot)
}

fn plain_parameter(model: &Model<'_, '_>, pattern: &BindingPattern<'_>) -> Option<SymbolId> {
    let BindingPattern::BindingIdentifier(id) = pattern else {
        return None;
    };
    let slot = id.symbol_id.get()?;
    (model.binding_writes(slot) == 0 && model.scoping.symbol_redeclarations(slot).is_empty())
        .then_some(slot)
}

fn create_next_forwarding_factory(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if function.params.rest.is_some()
        || !function.params.items.is_empty()
        || function.r#async
        || function.generator
        || !body.directives.is_empty()
    {
        return None;
    }
    let [Statement::ReturnStatement(returned)] = body.statements.as_slice() else {
        return None;
    };
    let Expression::NewExpression(created) = returned.argument.as_ref()? else {
        return None;
    };
    let [Argument::ArrowFunctionExpression(register)] = created.arguments.as_slice() else {
        return None;
    };
    if register.r#async || register.params.rest.is_some() || !register.body.directives.is_empty() {
        return None;
    }
    let [parameter] = register.params.items.as_slice() else {
        return None;
    };
    if parameter.initializer.is_some() {
        return None;
    }
    let subscriber = plain_parameter(model, &parameter.pattern)?;
    let [Statement::VariableDeclaration(local), Statement::ExpressionStatement(push)] =
        register.body.statements.as_slice()
    else {
        return None;
    };
    let [local] = local.declarations.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(receiver_id) = &local.id else {
        return None;
    };
    let receiver = receiver_id.symbol_id.get()?;
    if local.init.as_ref().and_then(|e| symbol(model, e)) != Some(subscriber)
        || model.binding_writes(receiver) != 0
        || model.scoping.symbol_redeclarations(receiver).is_empty() == false
    {
        return None;
    }
    let Expression::CallExpression(push) = &push.expression else {
        return None;
    };
    if push.optional || push.arguments.len() != 1 {
        return None;
    }
    let queue = captured_data_slot(model, site, member(&push.callee, "push")?)?;
    let [Argument::ArrowFunctionExpression(forward)] = push.arguments.as_slice() else {
        return None;
    };
    if forward.r#async
        || forward.params.rest.is_some()
        || !forward.body.directives.is_empty()
        || !forward.expression
    {
        return None;
    }
    let [forward_parameter] = forward.params.items.as_slice() else {
        return None;
    };
    if forward_parameter.initializer.is_some()
        || plain_parameter(model, &forward_parameter.pattern).is_none()
    {
        return None;
    }
    let value = plain_parameter(model, &forward_parameter.pattern)?;
    let [Statement::ExpressionStatement(next)] = forward.body.statements.as_slice() else {
        return None;
    };
    let Expression::CallExpression(next) = &next.expression else {
        return None;
    };
    if next.optional || next.arguments.len() != 1 {
        return None;
    }
    if symbol(model, member(&next.callee, "next")?) != Some(receiver)
        || call_arguments(next).first().and_then(|e| symbol(model, e)) != Some(value)
        || symbol(model, member(&push.callee, "push")?) != Some(queue)
    {
        return None;
    }
    Some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return vec![];
    };
    let Some(body) = function.body.as_ref() else {
        return vec![];
    };
    if create_next_forwarding_factory(model, site, function, body).is_some() {
        vec![proposal(
            "createRelaySubscription",
            "shared_frontier_relay_subscription",
            "exact zero-formal factory returns one new expression whose owned one-formal registration callback aliases that subscriber and pushes exactly one value callback forwarding the exact value to subscriber.next; constructor identity is not asserted",
            f,
        )]
    } else {
        vec![]
    }
}
