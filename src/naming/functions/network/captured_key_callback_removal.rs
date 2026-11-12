//! Complete captured-key removal protocol. Captured objects and methods are
//! opaque: this does not establish fetch identity or lifecycle activation.
use crate::naming::support::interaction::*;
use super::*;

fn computed<'a>(e: &'a Expression<'a>) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::ComputedMemberExpression(m) = e else {
        return None;
    };
    (!m.optional).then_some((&m.object, &m.expression))
}

fn protocol(model: &Model<'_, '_>, site: FunctionSite, function: &Function<'_>) -> Option<()> {
    let body = function.body.as_ref()?;
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !function.params.items.is_empty()
        || !body.directives.is_empty()
    {
        return None;
    }
    let [keys, index, Statement::ForStatement(loop_), Statement::IfStatement(guard)] =
        body.statements.as_slice()
    else {
        return None;
    };
    let keys = variable(keys)?;
    let key_list = plain_var(model, keys)?;
    let concat = call(keys.init.as_ref()?)?;
    let first = captured_symbol(model, site, method_receiver(model, concat, "concat")?)?;
    let [second] = concat.arguments.as_slice() else {
        return None;
    };
    let second = captured_symbol(model, site, symbol(model, second.as_expression()?)?)?;
    let index = variable(index)?;
    let cursor = plain_var(model, index)?;
    if !number(index.init.as_ref()?, 0.0)
        || model.binding_writes(key_list) != 0
        || model.binding_writes(cursor) != 1
        || loop_.init.is_some()
    {
        return None;
    }
    let (left, right) = binary(loop_.test.as_ref()?, "<")?;
    if symbol(model, left) != Some(cursor)
        || member(right, "length").and_then(|e| symbol(model, e)) != Some(key_list)
    {
        return None;
    }
    let Expression::UpdateExpression(update) = loop_.update.as_ref()? else {
        return None;
    };
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(updated) = &update.argument else {
        return None;
    };
    if update.prefix
        || update.operator.as_str() != "++"
        || crate::analysis::identifier_symbol(updated, model.scoping) != Some(cursor)
    {
        return None;
    }
    let Statement::BlockStatement(loop_body) = &loop_.body else {
        return None;
    };
    let [deletion] = loop_body.body.as_slice() else {
        return None;
    };
    let Expression::UnaryExpression(deletion) = expression_statement(deletion)? else {
        return None;
    };
    if deletion.operator.as_str() != "delete" {
        return None;
    }
    let (target, key) = computed(&deletion.argument)?;
    let target = captured_symbol(model, site, symbol(model, target)?)?;
    let (list, offset) = computed(key)?;
    if symbol(model, list) != Some(key_list) || symbol(model, offset) != Some(cursor) {
        return None;
    }
    let (channel, null) = binary(&guard.test, "!=")?;
    let holder = captured_symbol(
        model,
        site,
        symbol(model, member(channel, "onBeforeInvoke")?)?,
    )?;
    if !matches!(null, Expression::NullLiteral(_)) || guard.alternate.is_some() {
        return None;
    }
    let Statement::BlockStatement(guard_body) = &guard.consequent else {
        return None;
    };
    let [unregister] = guard_body.body.as_slice() else {
        return None;
    };
    let unregister = call(expression_statement(unregister)?)?;
    if member(&unregister.callee, "unregister")
        .and_then(|e| member(e, "onBeforeInvoke"))
        .and_then(|e| symbol(model, e))
        != Some(holder)
    {
        return None;
    }
    let [callback] = unregister.arguments.as_slice() else {
        return None;
    };
    let callback = captured_symbol(model, site, symbol(model, callback.as_expression()?)?)?;
    let identities = [first, second, target, holder, callback, key_list, cursor];
    if identities.iter().copied().collect::<BTreeSet<_>>().len() != identities.len() {
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
        return Vec::new();
    };
    if protocol(model, site, function).is_none() {
        return Vec::new();
    }
    vec![proposal(
        "deleteCapturedKeysAndUnregisterCallback",
        "network_captured_key_callback_removal",
        "complete owned protocol: concat two distinct lexical captures into an unchanged local list; iterate from zero through its length with a postfix increment and delete exactly target[list[index]]; conditionally unregister one captured callback from the exact nonnull onBeforeInvoke receiver; all five captures have plain lexical declaration identities; native network identity and lifecycle activation are unproved",
        f,
    )]
}
