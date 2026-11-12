use super::*;

use crate::naming::support::media_entropy::*;

fn param_index(model: &Model<'_, '_>, e: &Expression<'_>, input: Symbol, index: f64) -> bool {
    indexed(e)
        .is_some_and(|(object, key)| symbol(model, object) == Some(input) && number(key, index))
}

fn random_fixed(model: &Model<'_, '_>, site: FunctionSite, body: &FunctionBody<'_>) -> Option<()> {
    let function = match model.nodes.kind(site.node_id) {
        AstKind::Function(f) => f,
        _ => return None,
    };
    let params = plain_params(model, function)?;
    let [input] = params.as_slice() else {
        return None;
    };
    let [state_init, bytes_init, state_update, first, second, third, fourth, last, next, complete] =
        body.statements.as_slice()
    else {
        return None;
    };
    let state_variable = one_variable(state_init)?;
    let state = plain_binding(model, state_variable, Some(1))?;
    if !random_expression(model, state_variable.init.as_ref()?) {
        return None;
    }
    let bytes_variable = one_variable(bytes_init)?;
    let bytes = plain_binding(model, bytes_variable, Some(0))?;
    byte_array(model, bytes_variable.init.as_ref()?, state)?;
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
    let first_variable = one_variable(first)?;
    let acc = plain_binding(model, first_variable, Some(3))?;
    let init = first_variable.init.as_ref()?;
    let (mix, operand, seed) = mixer_parts(model, init, state, bytes)?;
    crate::naming::resolve::ordered_callable(model, &call(init)?.callee, 0)?;
    if !number(seed, 3.0) || !param_index(model, operand, *input, 4.0) {
        return None;
    }
    let mut expected = [1.0, 3.0, 0.0].into_iter();
    for statement in [second, third, fourth] {
        let assign = assignment(statement)?;
        if assigned_symbol(model, &assign.left) != Some(acc) {
            return None;
        }
        let (callee, operand, seed) = mixer_parts(model, &assign.right, state, bytes)?;
        if callee != mix
            || symbol(model, seed) != Some(acc)
            || !param_index(model, operand, *input, expected.next()?)
        {
            return None;
        }
    }
    let (callee, operand, seed) = mixer_parts(model, expression_statement(last)?, state, bytes)?;
    if callee != mix
        || symbol(model, seed) != Some(acc)
        || !param_index(model, operand, *input, 2.0)
    {
        return None;
    }
    forwarded_bytes(model, site, next, complete, bytes, None)
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
    if function.params.items.len() == 1 && random_fixed(model, site, body).is_some() {
        claims.push(proposal(
            "emitSeededHashEncoding",
            "encoding_seeded_hash_random_bytes",
            "emits a Math.random-derived byte quadruple after a proven state mixer and fixed-order field mixer over the formal, then next([[], bytes]) and complete() on one captured receiver",
            f,
        ));
    }
    claims
}
