use super::*;
use oxc_semantic::SymbolId;

use crate::naming::support::shared_events::*;

fn message_envelope(
    model: &Model<'_, '_>,
    site: FunctionSite,
    input: SymbolId,
    body: &FunctionBody<'_>,
) -> Option<()> {
    // A small sequential environment supports recovered value/record aliases.
    // Every prefix declaration must participate in the one forwarding call.
    if body.statements.len() > 5 {
        return None;
    }
    let (last, prefix) = body.statements.split_last()?;
    let mut locals = BTreeMap::new();
    for statement in prefix {
        let (s, init) = local(model, statement)?;
        if model.binding_references(s) == 0 {
            return None;
        }
        let value = symbol(model, init)
            .and_then(|s| locals.get(&s).copied())
            .unwrap_or(init);
        let valid = symbol(model, value) == Some(input)
            || object_field(value, "message").is_some_and(|v| {
                let v = symbol(model, v)
                    .and_then(|s| locals.get(&s).copied())
                    .unwrap_or(v);
                symbol(model, v) == Some(input)
            });
        if !valid {
            return None;
        }
        locals.insert(s, value);
    }
    let c = statement_call(last)?;
    let [argument] = c.arguments.as_slice() else {
        return None;
    };
    let target = crate::naming::resolve::ordered_callable(model, &c.callee, 0)?;
    if target.span == site.span {
        return None;
    }
    let argument = argument.as_expression()?;
    let argument = symbol(model, argument)
        .and_then(|s| locals.get(&s).copied())
        .unwrap_or(argument);
    let value = object_field(argument, "message")?;
    let value = symbol(model, value)
        .and_then(|s| locals.get(&s).copied())
        .unwrap_or(value);
    (symbol(model, value) == Some(input)).then_some(())
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
    let [p] = function.params.items.as_slice() else {
        return Vec::new();
    };
    if function.r#async
        || function.generator
        || !body.directives.is_empty()
        || function.params.rest.is_some()
        || p.initializer.is_some()
    {
        return Vec::new();
    }
    let Some(input) = immutable(model, &p.pattern) else {
        return Vec::new();
    };
    let mut claims = Vec::new();
    if message_envelope(model, site, input, body).is_some() {
        claims.push(proposal(
            "forwardMessageRecord",
            "shared_message_envelope",
            "sole expression forwards exactly {message:formal0} to a distinct function through a stable scoped callable origin; no return, extra fields or extra arguments",
            f,
        ));
    }
    claims
}
