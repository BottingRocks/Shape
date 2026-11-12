use super::*;

use crate::naming::support::media_entropy::*;

fn random_int_expression(model: &Model<'_, '_>, e: &Expression<'_>) -> bool {
    binary(e, "|").is_some_and(|(left, right)| number(right, 0.0) && random_expression(model, left))
}

fn random_digest(model: &Model<'_, '_>, site: FunctionSite, body: &FunctionBody<'_>) -> Option<()> {
    let function = match model.nodes.kind(site.node_id) {
        AstKind::Function(f) => f,
        _ => return None,
    };
    let params = plain_params(model, function)?;
    let [input] = params.as_slice() else {
        return None;
    };
    let [state_init, bytes_init, out_init, state_update, length_init, acc_init, Statement::WhileStatement(scan), next, complete] =
        body.statements.as_slice()
    else {
        return None;
    };
    let state_variable = one_variable(state_init)?;
    let state = plain_binding(model, state_variable, Some(1))?;
    if !random_int_expression(model, state_variable.init.as_ref()?) {
        return None;
    }
    let bytes_variable = one_variable(bytes_init)?;
    let bytes = plain_binding(model, bytes_variable, Some(0))?;
    byte_array(model, bytes_variable.init.as_ref()?, state)?;
    let out_variable = one_variable(out_init)?;
    let out = plain_binding(model, out_variable, Some(0))?;
    if !empty_array(out_variable.init.as_ref()?) {
        return None;
    }
    let update = assignment(state_update)?;
    if assigned_symbol(model, &update.left) != Some(state) {
        return None;
    }
    let Some(state_call) = call(&update.right) else {
        return None;
    };
    crate::naming::resolve::ordered_callable(model, &state_call.callee, 0)?;
    let state_args = call_arguments(state_call);
    let [mixed] = state_args.as_slice() else {
        return None;
    };
    let Some((xor_left, constant)) = binary(mixed, "^") else {
        return None;
    };
    if !state_or_zero(model, xor_left, state)
        || !matches!(
            constant.without_parentheses(),
            Expression::NumericLiteral(_)
        )
    {
        return None;
    }
    let length_variable = one_variable(length_init)?;
    let length = plain_binding(model, length_variable, Some(1))?;
    if !member(length_variable.init.as_ref()?, "length")
        .is_some_and(|receiver| symbol(model, receiver) == Some(*input))
    {
        return None;
    }
    let acc_variable = one_variable(acc_init)?;
    let acc = plain_binding(model, acc_variable, Some(1))?;
    let acc_initializer = acc_variable.init.as_ref()?;
    let (mix, operand, seed) = mixer_parts(model, acc_initializer, state, bytes)?;
    // The first digest field mixer is an independent callee origin. The loop
    // only compares the same symbol, so prove that this initializer call is an
    // ordered callable before accepting the mixer identity.
    crate::naming::resolve::ordered_callable(model, &call(acc_initializer)?.callee, 0)?;
    if symbol(model, operand) != Some(length) || !number(seed, 3.0) {
        return None;
    }
    let Some((distance, zero)) = binary(&scan.test, ">=") else {
        return None;
    };
    let Some((length_minus, one)) = binary(distance, "-") else {
        return None;
    };
    if symbol(model, length_minus) != Some(length) || !number(one, 1.0) || !number(zero, 0.0) {
        return None;
    }
    let scan_body = block(&scan.body)?;
    let [Statement::ExpressionStatement(step), decrement] = scan_body else {
        return None;
    };
    let step = assignment_in(step)?;
    if assigned_symbol(model, &step.left) != Some(acc) {
        return None;
    }
    let (callee, operand, seed) = mixer_parts(model, &step.right, state, bytes)?;
    if callee != mix || symbol(model, seed) != Some(acc) {
        return None;
    }
    let Some((element, index_expr)) = indexed(operand) else {
        return None;
    };
    let Some((index_minus, one)) = binary(index_expr, "-") else {
        return None;
    };
    if symbol(model, element) != Some(*input)
        || symbol(model, index_minus) != Some(length)
        || !number(one, 1.0)
        || !update_target(model, decrement, length, "--")
    {
        return None;
    }
    forwarded_bytes(model, site, next, complete, bytes, Some(out))
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
    if function.r#async || function.generator || !body.directives.is_empty() {
        return Vec::new();
    }
    let mut claims = Vec::new();
    if function.params.items.len() == 1 && random_digest(model, site, body).is_some() {
        claims.push(proposal(
            "emitSeededHashEncoding",
            "encoding_seeded_hash_digest",
            "emits a Math.random-derived byte quadruple after a proven state mixer and descending field mixer over the formal length/elements, then next([local, bytes]) and complete() on one captured receiver",
            f,
        ));
    }
    claims
}
