use super::*;
use oxc_span::GetSpan;

use crate::naming::support::seeded_flow::*;

// ---------------------------------------------------------------------------
// `recordVisibilityChangeSample`: buffered visibility sample with exact clock.
// ---------------------------------------------------------------------------
fn visibility_record(model: &Model<'_, '_>, e: &Expression<'_>, site: FunctionSite) -> bool {
    let Expression::ObjectExpression(object) = e else {
        return false;
    };
    let [ObjectPropertyKind::ObjectProperty(a), ObjectPropertyKind::ObjectProperty(b)] =
        object.properties.as_slice()
    else {
        return false;
    };
    let (Some(ka), Some(kb)) = (property_key_name(&a.key), property_key_name(&b.key)) else {
        return false;
    };
    if a.computed
        || b.computed
        || a.method
        || b.method
        || a.kind != PropertyKind::Init
        || b.kind != PropertyKind::Init
        || ka != "visible"
        || kb != "timestamp"
    {
        return false;
    }
    let visible_ok = match (&a.value, &b.value) {
        (Expression::BinaryExpression(cmp), _) => {
            cmp.operator.as_str() == "==="
                && member(&cmp.left, "visibilityState")
                    .and_then(|e| member(e, "target"))
                    .is_some_and(|e| model.site_parameter_slot(e, site) == Some(0))
                && string(&cmp.right, "visible")
        }
        _ => false,
    };
    let timestamp_ok = match &b.value {
        Expression::CallExpression(clock) if !clock.optional => {
            arity(clock, 0).is_some()
                && (callable_capture(model, site, &clock.callee).is_some()
                    || data_capture(model, site, &clock.callee).is_some())
        }
        _ => false,
    };
    visible_ok && timestamp_ok
}

fn visibility_sample(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [Statement::TryStatement(t)] = body.statements.as_slice() else {
        return None;
    };
    if t.finalizer.is_some() {
        return None;
    }
    let (inc, record, flag) = match t.block.body.as_slice() {
        [inc, invoke, flag] => {
            let invoke = statement_call(invoke)?;
            let receiver = member(&invoke.callee, "put")?;
            data_capture(model, site, receiver)?;
            (inc, *arity(invoke, 1)?.first()?, flag)
        }
        [inc, receiver_decl, invoke, flag] => {
            let receiver = plain_local(model, one_variable(receiver_decl)?)?;
            let base = symbol(model, one_variable(receiver_decl)?.init.as_ref()?)?;
            if base == receiver
                || data_capture(model, site, one_variable(receiver_decl)?.init.as_ref()?).is_none()
            {
                return None;
            }
            let invoke = statement_call(invoke)?;
            let object = member(&invoke.callee, "put")?;
            if symbol(model, object) != Some(receiver) && symbol(model, object) != Some(base) {
                return None;
            }
            (inc, *arity(invoke, 1)?.first()?, flag)
        }
        [inc, receiver_decl, method_decl, invoke, flag] => {
            let receiver = plain_local(model, one_variable(receiver_decl)?)?;
            let receiver_init = one_variable(receiver_decl)?.init.as_ref()?;
            let base = symbol(model, receiver_init)?;
            if base == receiver || data_capture(model, site, receiver_init).is_none() {
                return None;
            }
            let method = plain_local(model, one_variable(method_decl)?)?;
            let object = member(one_variable(method_decl)?.init.as_ref()?, "put")?;
            if symbol(model, object) != Some(base) {
                return None;
            }
            let invoke = statement_call(invoke)?;
            if member(&invoke.callee, "call").and_then(|e| symbol(model, e)) != Some(method) {
                return None;
            }
            let args = arity(invoke, 2)?;
            if symbol(model, args[0]) != Some(receiver) {
                return None;
            }
            (inc, args[1], flag)
        }
        _ => return None,
    };
    let counter = update_symbol(model, expression(inc)?, true, "++")?;
    if !capture_identity(model, site, counter, inc.span().start) {
        return None;
    }
    let flag = assignment(flag)?;
    let flag_symbol = assign_symbol(model, flag)?;
    if !matches!(&flag.right, Expression::BooleanLiteral(b) if b.value)
        || !capture_identity(model, site, flag_symbol, flag.span.start)
        || counter == flag_symbol
    {
        return None;
    }
    let handler = t.handler.as_ref()?;
    let BindingPattern::BindingIdentifier(error) = &handler.param.as_ref()?.pattern else {
        return None;
    };
    let error = error.symbol_id.get()?;
    let [failed] = handler.body.body.as_slice() else {
        return None;
    };
    let failed = statement_call(failed)?;
    let Some(receiver) = member(&failed.callee, "error") else {
        return None;
    };
    if data_capture(model, site, receiver).is_none()
        || arity(failed, 1)?.first().and_then(|e| symbol(model, e)) != Some(error)
        || !unchanged(model, error)
    {
        return None;
    }
    visibility_record(model, record, site).then_some(())
}

// Additive no-completion-flag spelling: the same complete try/catch sample
// without a trailing captured flag write. The counter, receiver alias, put
// method alias, visibility record and caught-error forward are unchanged, so
// the same operational role is proved from the complete own body.
fn visibility_sample_without_flag(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [Statement::TryStatement(t)] = body.statements.as_slice() else {
        return None;
    };
    if t.finalizer.is_some() {
        return None;
    }
    let [inc, receiver_decl, method_decl, invoke] = t.block.body.as_slice() else {
        return None;
    };
    let receiver = plain_local(model, one_variable(receiver_decl)?)?;
    let receiver_init = one_variable(receiver_decl)?.init.as_ref()?;
    let base = symbol(model, receiver_init)?;
    if base == receiver || data_capture(model, site, receiver_init).is_none() {
        return None;
    }
    let method = plain_local(model, one_variable(method_decl)?)?;
    let object = member(one_variable(method_decl)?.init.as_ref()?, "put")?;
    if symbol(model, object) != Some(base) {
        return None;
    }
    let invoke = statement_call(invoke)?;
    if member(&invoke.callee, "call").and_then(|e| symbol(model, e)) != Some(method) {
        return None;
    }
    let args = arity(invoke, 2)?;
    if symbol(model, args[0]) != Some(receiver) {
        return None;
    }
    let counter = update_symbol(model, expression(inc)?, true, "++")?;
    if !capture_identity(model, site, counter, inc.span().start) {
        return None;
    }
    let handler = t.handler.as_ref()?;
    let BindingPattern::BindingIdentifier(error) = &handler.param.as_ref()?.pattern else {
        return None;
    };
    let error = error.symbol_id.get()?;
    let [failed] = handler.body.body.as_slice() else {
        return None;
    };
    let failed = statement_call(failed)?;
    let Some(receiver) = member(&failed.callee, "error") else {
        return None;
    };
    if data_capture(model, site, receiver).is_none()
        || arity(failed, 1)?.first().and_then(|e| symbol(model, e)) != Some(error)
        || !unchanged(model, error)
    {
        return None;
    }
    visibility_record(model, args[1], site).then_some(())
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
    if function.r#async
        || function.generator
        || !directives(body)
        || !plain_params(model, &function.params)
    {
        return Vec::new();
    }
    let mut claims = Vec::new();
    if function.params.items.len() == 1 && visibility_sample(model, site, body).is_some() {
        claims.push(proposal(
            "recordVisibilityChangeSample",
            "events_visibility_change_sample",
            "try/catch visibility sample: exact pre-increment counter, one receiver alias, put invocation with visible/timestamp object and captured zero-argument clock, then captured flag set true and caught error forwarded to a captured error receiver",
            f,
        ));
    } else if function.params.items.len() == 1
        && visibility_sample_without_flag(model, site, body).is_some()
    {
        claims.push(proposal(
            "recordVisibilityChangeSample",
            "events_visibility_change_sample",
            "try/catch visibility sample without a completion flag: exact pre-increment counter, one receiver alias, one put method alias invoked through call with the receiver and the visible/timestamp object carrying a captured zero-argument clock, then the caught error forwarded to a captured error receiver; no flag write and no other statements",
            f,
        ));
    }
    claims
}
