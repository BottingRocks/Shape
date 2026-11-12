//! One small source-index-3 teardown rule.
//!
//! The owned body deletes one computed key from the unshadowed global
//! `window`, gives an opaque consumer the single formal exactly once inside a
//! `try` whose catch is empty and which has no finalizer, then applies one
//! captured applier twice with the same captured method and receiver and two
//! distinct captured children.  Every captured role must be an ordinary
//! `var`/`let`/`const` declarator outside the proven site, without a
//! redeclaration.  Mutability of the captured slots is deliberately not
//! constrained; only their lexical origin is proved.
//!
//! The consumer, applier, method, receiver, key and children are opaque
//! consumer-side identities.  No native callback, DOM method, reflection
//! helper or generated spelling is asserted here.
use super::*;
use oxc_semantic::SymbolId;
use std::collections::BTreeSet;

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e, model.scoping)
}

fn plain_variable_kind(kind: VariableDeclarationKind) -> bool {
    matches!(
        kind,
        VariableDeclarationKind::Var
            | VariableDeclarationKind::Let
            | VariableDeclarationKind::Const
    )
}

/// A captured plain variable only: never a parameter, catch binding,
/// destructuring target or loop binding, never a redeclaration, and its
/// declaration must complete before the proven function site.
fn captured_slot(model: &Model<'_, '_>, site: FunctionSite, slot: SymbolId) -> Option<SymbolId> {
    if site
        .span
        .contains_inclusive(model.scoping.symbol_span(slot))
        || model.scoping.symbol_span(slot).end >= site.span.start
        || !model.scoping.symbol_redeclarations(slot).is_empty()
        || model.formal_symbol_slot(slot).is_some()
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
            AstKind::VariableDeclaration(d) if plain_variable_kind(d.kind)
        )
        || matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
    {
        return None;
    }
    Some(slot)
}

fn captured(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<SymbolId> {
    captured_slot(model, site, symbol(model, e)?)
}

fn captured_argument(
    model: &Model<'_, '_>,
    site: FunctionSite,
    id: &IdentifierReference<'_>,
) -> Option<SymbolId> {
    captured_slot(
        model,
        site,
        crate::analysis::identifier_symbol(id, model.scoping)?,
    )
}

fn unshadowed_window(model: &Model<'_, '_>, e: &Expression<'_>) -> bool {
    matches!(
        e.without_parentheses(),
        Expression::Identifier(id)
            if id.name == "window"
                && crate::analysis::identifier_symbol(id, model.scoping).is_none()
    )
}

pub(in crate::naming) fn remove_injected_global_and_notify_cleanup(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if function.r#async
        || function.generator
        || !body.directives.is_empty()
        || function.params.rest.is_some()
    {
        return None;
    }
    let [parameter] = function.params.items.as_slice() else {
        return None;
    };
    if parameter.initializer.is_some() {
        return None;
    }
    let BindingPattern::BindingIdentifier(input_id) = &parameter.pattern else {
        return None;
    };
    let input = input_id.symbol_id.get()?;
    if model.binding_writes(input) != 0 || !model.scoping.symbol_redeclarations(input).is_empty() {
        return None;
    }

    let [Statement::ExpressionStatement(deletion), Statement::TryStatement(attempt), Statement::ExpressionStatement(first), Statement::ExpressionStatement(second)] =
        body.statements.as_slice()
    else {
        return None;
    };

    // delete window[capturedKey]
    let Expression::UnaryExpression(unary) = deletion.expression.without_parentheses() else {
        return None;
    };
    if unary.operator.as_str() != "delete" {
        return None;
    }
    let Expression::ComputedMemberExpression(member) = unary.argument.without_parentheses() else {
        return None;
    };
    if member.optional || !unshadowed_window(model, &member.object) {
        return None;
    }
    let key = captured(model, site, &member.expression)?;

    // try { consumer(formal); } catch (caught) {} with no finalizer
    if attempt.finalizer.is_some() {
        return None;
    }
    let [Statement::ExpressionStatement(notification)] = attempt.block.body.as_slice() else {
        return None;
    };
    let Expression::CallExpression(notify) = notification.expression.without_parentheses() else {
        return None;
    };
    if notify.optional {
        return None;
    }
    let consumer = captured(model, site, &notify.callee)?;
    let [Argument::Identifier(notified_input)] = notify.arguments.as_slice() else {
        return None;
    };
    if crate::analysis::identifier_symbol(notified_input, model.scoping) != Some(input) {
        return None;
    }
    let handler = attempt.handler.as_ref()?;
    let BindingPattern::BindingIdentifier(caught_id) = &handler.param.as_ref()?.pattern else {
        return None;
    };
    let caught = caught_id.symbol_id.get()?;
    if !handler.body.body.is_empty()
        || !model.scoping.symbol_redeclarations(caught).is_empty()
        || caught == input
    {
        return None;
    }

    // applier(method, receiver, firstChild); applier(method, receiver, secondChild)
    let Expression::CallExpression(first_call) = first.expression.without_parentheses() else {
        return None;
    };
    let Expression::CallExpression(second_call) = second.expression.without_parentheses() else {
        return None;
    };
    if first_call.optional || second_call.optional {
        return None;
    }
    let applier = captured(model, site, &first_call.callee)?;
    if captured(model, site, &second_call.callee)? != applier {
        return None;
    }
    let [Argument::Identifier(first_method), Argument::Identifier(first_receiver), Argument::Identifier(first_child)] =
        first_call.arguments.as_slice()
    else {
        return None;
    };
    let [Argument::Identifier(second_method), Argument::Identifier(second_receiver), Argument::Identifier(second_child)] =
        second_call.arguments.as_slice()
    else {
        return None;
    };
    let method = captured_argument(model, site, first_method)?;
    let receiver = captured_argument(model, site, first_receiver)?;
    let first_child = captured_argument(model, site, first_child)?;
    let second_child = captured_argument(model, site, second_child)?;
    if captured_argument(model, site, second_method)? != method
        || captured_argument(model, site, second_receiver)? != receiver
        || second_child == first_child
    {
        return None;
    }

    let mut roles = BTreeSet::new();
    for role in [
        input,
        key,
        consumer,
        applier,
        method,
        receiver,
        first_child,
        second_child,
    ] {
        if !roles.insert(role) {
            return None;
        }
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
        return Vec::new();
    };
    let Some(body) = function.body.as_ref() else {
        return Vec::new();
    };
    if remove_injected_global_and_notify_cleanup(model, site, function, body).is_some() {
        vec![proposal(
            "removeInjectedGlobalAndNotifyCleanup",
            "browser_injected_global_removal",
            "owned teardown body: delete unshadowed window[capturedKey]; try { consumer(formal); } catch (caught) {} with no finalizer; then one captured applier is called twice with the same captured method and receiver and two distinct captured children. The consumer, applier, method, receiver, key and children are plain captured bindings proved by scope; no native callback, DOM method or reflection helper identity is asserted",
            f,
        )]
    } else {
        Vec::new()
    }
}
