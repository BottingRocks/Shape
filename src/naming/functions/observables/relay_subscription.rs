//! Small, still-untargeted frontier operations for relay subscriptions.
//!
//! The single rule here proves the complete owned body of a compact observable
//! adapter: one opaque constructor expression is invoked with one owned executor
//! arrow that copies its opaque subscriber argument into one immutable local and
//! registers one owned relay arrow forwarding each argument to that subscriber's
//! `next` member onto one captured data slot. The callback input is treated as
//! DATA; no native or vendor-specific callee role is asserted.
use super::*;
use oxc_semantic::SymbolId;

fn symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(expression, model.scoping)
}

fn pattern_symbol(pattern: &BindingPattern<'_>) -> Option<SymbolId> {
    let BindingPattern::BindingIdentifier(identifier) = pattern else {
        return None;
    };
    identifier.symbol_id.get()
}

fn plain_kind(kind: VariableDeclarationKind) -> bool {
    matches!(
        kind,
        VariableDeclarationKind::Var
            | VariableDeclarationKind::Let
            | VariableDeclarationKind::Const
    )
}

/// The plain ancestor variable that owns a claimed capture, with the statement
/// container its declaration belongs to. Parameters, catches, destructuring,
/// loop headers, resources and redeclarations never establish a capture.
fn captured_container(
    model: &Model<'_, '_>,
    site: FunctionSite,
    symbol: SymbolId,
) -> Option<oxc_semantic::NodeId> {
    if site
        .span
        .contains_inclusive(model.scoping.symbol_span(symbol))
        || model.formal_symbol_slot(symbol).is_some()
        || !model.scoping.symbol_redeclarations(symbol).is_empty()
    {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(symbol);
    let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&variable.id, BindingPattern::BindingIdentifier(identifier)
        if identifier.symbol_id.get() == Some(symbol))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(declaration) if plain_kind(declaration.kind)
        )
    {
        return None;
    }
    // The declaration must belong directly to a function body or the program,
    // not to a for/while header, conditional child or resource declaration.
    let container = model.nodes.parent_id(model.nodes.parent_id(declaration));
    matches!(
        model.nodes.kind(container),
        AstKind::FunctionBody(_) | AstKind::Program(_)
    )
    .then_some(container)
}

/// One plain, immutable, non-defaulting arrow parameter.
fn arrow_parameter(model: &Model<'_, '_>, arrow: &ArrowFunctionExpression<'_>) -> Option<SymbolId> {
    if arrow.r#async || arrow.params.rest.is_some() {
        return None;
    }
    let [parameter] = arrow.params.items.as_slice() else {
        return None;
    };
    if parameter.initializer.is_some() {
        return None;
    }
    let symbol = pattern_symbol(&parameter.pattern)?;
    (model.binding_writes(symbol) == 0 && model.scoping.symbol_redeclarations(symbol).is_empty())
        .then_some(symbol)
}

/// Complete owned-body proof of `return new Ctor(subscriber => { let x =
/// subscriber; list.push(value => x.next(value)); })`.
fn relay_subscription(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if !function.params.items.is_empty() || function.params.rest.is_some() {
        return None;
    }
    let [Statement::ReturnStatement(returned)] = body.statements.as_slice() else {
        return None;
    };
    let Expression::NewExpression(created) = returned.argument.as_ref()? else {
        return None;
    };
    let [Argument::ArrowFunctionExpression(executor)] = created.arguments.as_slice() else {
        return None;
    };
    if !executor.body.directives.is_empty() {
        return None;
    }
    let subscriber = arrow_parameter(model, executor)?;
    let [Statement::VariableDeclaration(alias_declaration), Statement::ExpressionStatement(register)] =
        executor.body.statements.as_slice()
    else {
        return None;
    };
    if !plain_kind(alias_declaration.kind) {
        return None;
    }
    let [alias_variable] = alias_declaration.declarations.as_slice() else {
        return None;
    };
    let alias = pattern_symbol(&alias_variable.id)?;
    if alias_variable
        .init
        .as_ref()
        .and_then(|expression| symbol(model, expression))
        != Some(subscriber)
        || alias == subscriber
        || model.binding_writes(alias) != 0
        || !model.scoping.symbol_redeclarations(alias).is_empty()
        || model.binding_references(subscriber) != 1
    {
        return None;
    }
    let Expression::CallExpression(push) = &register.expression else {
        return None;
    };
    let [Argument::ArrowFunctionExpression(relay)] = push.arguments.as_slice() else {
        return None;
    };
    if push.optional || call_arguments(push).len() != 1 {
        return None;
    }
    let Expression::StaticMemberExpression(push_member) = &push.callee else {
        return None;
    };
    if push_member.optional || push_member.property.name != "push" {
        return None;
    }
    let registry = symbol(model, &push_member.object)?;
    if registry == subscriber || registry == alias {
        return None;
    }
    captured_container(model, site, registry)?;
    if !relay.expression || !relay.body.directives.is_empty() {
        return None;
    }
    let relay_value = arrow_parameter(model, relay)?;
    if relay_value == subscriber || relay_value == alias || relay_value == registry {
        return None;
    }
    let [Statement::ExpressionStatement(forwarded)] = relay.body.statements.as_slice() else {
        return None;
    };
    let Expression::CallExpression(forward) = &forwarded.expression else {
        return None;
    };
    if forward.optional || call_arguments(forward).len() != 1 {
        return None;
    }
    let Expression::StaticMemberExpression(next) = &forward.callee else {
        return None;
    };
    if next.optional
        || next.property.name != "next"
        || symbol(model, &next.object) != Some(alias)
        || symbol(model, call_arguments(forward)[0]) != Some(relay_value)
        || model.binding_references(relay_value) != 1
        || model.binding_references(alias) != 1
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
    if function.r#async || function.generator || !body.directives.is_empty() {
        return vec![];
    }
    let mut claims = Vec::new();
    if relay_subscription(model, site, function, body).is_some() {
        claims.push(proposal(
            "createRelaySubscription",
            "shared_frontier_relay_subscription",
            "returns one `new` expression with an owned executor arrow; the executor copies its opaque subscriber argument into one immutable local and registers one owned relay arrow forwarding each argument to that subscriber's next member on one captured plain data slot; the list, subscriber and relay identities are distinct and the complete owned body is matched",
            f,
        ));
    }
    claims
}
