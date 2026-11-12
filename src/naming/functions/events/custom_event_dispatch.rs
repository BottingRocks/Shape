use super::*;

use crate::naming::support::installer_events::*;

fn global_host(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<String> {
    global_path(model, e)?.into_iter().next()
}

fn dispatch_custom_event_with(
    model: &Model<'_, '_>,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if !function.params.items.is_empty()
        || function.params.rest.is_some()
        || function.r#async
        || function.generator
        || !body.directives.is_empty()
    {
        return None;
    }
    let [Statement::VariableDeclaration(declaration), init, dispatch] = body.statements.as_slice()
    else {
        return None;
    };
    let [variable] = declaration.declarations.as_slice() else {
        return None;
    };
    let event = immutable_binding(model, &variable.id)?;
    let created = call(variable.init.as_ref()?)?;
    let Some(created_on) = member(&created.callee, "createEvent") else {
        return None;
    };
    let Some(created_path) = global_path(model, created_on) else {
        return None;
    };
    if created.optional
        || created_path.last().map(String::as_str) != Some("document")
        || !matches!(created.arguments.as_slice(), [Argument::StringLiteral(kind)] if kind.value == "CustomEvent")
    {
        return None;
    }
    let Some(host) = created_path.first().cloned() else {
        return None;
    };
    let init = statement_call(init)?;
    let [Argument::StringLiteral(_), Argument::BooleanLiteral(bubbles), Argument::BooleanLiteral(cancelable), Argument::ObjectExpression(detail)] =
        init.arguments.as_slice()
    else {
        return None;
    };
    if init.optional || !bubbles.value || !cancelable.value || !detail.properties.is_empty() {
        return None;
    }
    let Some(target) = member(&init.callee, "initCustomEvent") else {
        return None;
    };
    if symbol(model, target) != Some(event) {
        return None;
    }
    let dispatch = statement_call(dispatch)?;
    // The createEvent and dispatchEvent operations must name the same
    // unresolved-global host root; swapping in a different object is a
    // different receiver, not the proven event host.
    let (dispatch_host, call_form) =
        if let Some(receiver) = member(&dispatch.callee, "dispatchEvent") {
            (receiver, false)
        } else if let Some(call_object) = member(&dispatch.callee, "call") {
            (member(call_object, "dispatchEvent")?, true)
        } else {
            return None;
        };
    if global_host(model, dispatch_host).as_deref() != Some(host.as_str()) {
        return None;
    }
    match dispatch.arguments.as_slice() {
        [argument] if !call_form => symbol(model, argument.as_expression()?) == Some(event),
        [Argument::UnaryExpression(undef), argument]
            if call_form && undef.operator.as_str() == "void" && number(&undef.argument, 0.0) =>
        {
            symbol(model, argument.as_expression()?) == Some(event)
        }
        _ => false,
    }
    .then_some(())
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
    let mut claims = Vec::new();
    if dispatch_custom_event_with(model, function, body).is_some() {
        claims.push(proposal(
            "dispatchBubblingCustomEvent",
            "shared_custom_event_dispatch",
            "creates an event through one unresolved-global document.createEvent with literal CustomEvent, initializes that same identity with a text name, bubbling/cancelable true and empty detail, then dispatches the same identity through a dispatchEvent operation on the same unresolved-global host root",
            f,
        ));
    }
    claims
}
