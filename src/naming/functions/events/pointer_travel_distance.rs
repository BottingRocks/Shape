use super::*;

use crate::naming::support::interaction::*;

fn accumulate_pointer_travel_distance(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if function.params.rest.is_some() || function.params.items.len() != 1 {
        return None;
    }
    let input = plain_param(model, &function.params.items[0])?;
    let [Statement::TryStatement(t)] = body.statements.as_slice() else {
        return None;
    };
    if t.finalizer.is_some() {
        return None;
    }
    let handler = t.handler.as_ref()?;
    let BindingPattern::BindingIdentifier(error) = &handler.param.as_ref()?.pattern else {
        return None;
    };
    let error = error.symbol_id.get()?;
    if model.binding_writes(error) != 0 || !model.scoping.symbol_redeclarations(error).is_empty() {
        return None;
    }
    let [Statement::ExpressionStatement(failed)] = handler.body.body.as_slice() else {
        return None;
    };
    let failed = call(&failed.expression)?;
    captured_symbol(model, site, method_receiver(model, failed, "error")?)?;
    let failed_args = arguments(failed)?;
    if failed_args.len() != 1 || symbol(model, failed_args[0]) != Some(error) {
        return None;
    }
    let [distance_statement, sqrt_statement, x_statement, y_statement, total_statement] =
        t.block.body.as_slice()
    else {
        return None;
    };
    let distance_variable = single_variable(match distance_statement {
        Statement::VariableDeclaration(d) => d,
        _ => return None,
    })?;
    let distance = plain_var(model, distance_variable)?;
    let (dy, last_y) = binary(distance_variable.init.as_ref()?, "-")?;
    let last_y = captured_symbol(model, site, symbol(model, last_y)?)?;
    if member(dy, "clientY").and_then(|e| symbol(model, e)) != Some(input) {
        return None;
    }
    let Some(sqrt) = assignment(match sqrt_statement {
        Statement::ExpressionStatement(s) => &s.expression,
        _ => return None,
    }) else {
        return None;
    };
    if target_symbol(model, &sqrt.left) != Some(distance) {
        return None;
    }
    let Some(sqrt_call) = call(&sqrt.right) else {
        return None;
    };
    let Expression::StaticMemberExpression(sqrt_member) = &sqrt_call.callee else {
        return None;
    };
    if sqrt_member.optional || sqrt_member.property.name != "sqrt" {
        return None;
    }
    let math = captured_symbol(model, site, symbol(model, &sqrt_member.object)?)?;
    let sqrt_args = arguments(sqrt_call)?;
    let [sum] = sqrt_args.as_slice() else {
        return None;
    };
    let (left_pow, right_pow) = binary(sum, "+")?;
    let left_pow = call(left_pow)?;
    let right_pow = call(right_pow)?;
    let pow = |call: &CallExpression<'_>| {
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return false;
        };
        !member.optional
            && member.property.name == "pow"
            && (symbol(model, &member.object) == Some(math)
                || expression_path(model, &member.object) == ["Math"])
    };
    if !pow(left_pow) || !pow(right_pow) {
        return None;
    }
    let left_pow_args = arguments(left_pow)?;
    let right_pow_args = arguments(right_pow)?;
    let [dx, two] = left_pow_args.as_slice() else {
        return None;
    };
    let (dx, last_x) = binary(dx, "-")?;
    let last_x = captured_symbol(model, site, symbol(model, last_x)?)?;
    if member(dx, "clientX").and_then(|e| symbol(model, e)) != Some(input) || !number(two, 2.0) {
        return None;
    }
    let [base, two] = right_pow_args.as_slice() else {
        return None;
    };
    if symbol(model, base) != Some(distance) || !number(two, 2.0) {
        return None;
    }
    let x = assignment(match x_statement {
        Statement::ExpressionStatement(s) => &s.expression,
        _ => return None,
    })?;
    if target_symbol(model, &x.left) != Some(last_x)
        || member(&x.right, "clientX").and_then(|e| symbol(model, e)) != Some(input)
    {
        return None;
    }
    let y = assignment(match y_statement {
        Statement::ExpressionStatement(s) => &s.expression,
        _ => return None,
    })?;
    if target_symbol(model, &y.left) != Some(last_y)
        || member(&y.right, "clientY").and_then(|e| symbol(model, e)) != Some(input)
    {
        return None;
    }
    let total = assignment(match total_statement {
        Statement::ExpressionStatement(s) => &s.expression,
        _ => return None,
    })?;
    let total_symbol = captured_symbol(model, site, target_symbol(model, &total.left)?)?;
    let (left, right) = binary(&total.right, "+")?;
    if symbol(model, left) != Some(total_symbol) {
        return None;
    }
    let Expression::ConditionalExpression(increment) = right else {
        return None;
    };
    let Some(is_nan) = call(&increment.test) else {
        return None;
    };
    let is_nan_args = arguments(is_nan)?;
    if expression_path(model, &is_nan.callee) != ["isNaN"]
        || !matches!(is_nan_args.as_slice(), [value] if symbol(model, value) == Some(distance))
        || !number(&increment.consequent, 0.0)
        || symbol(model, &increment.alternate) != Some(distance)
        || last_x == last_y
        || total_symbol == last_x
        || total_symbol == last_y
        || math == total_symbol
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
        return Vec::new();
    };
    let Some(body) = function.body.as_ref() else {
        return Vec::new();
    };
    if function.r#async || function.generator || !body.directives.is_empty() {
        return Vec::new();
    }
    let mut claims = Vec::new();
    if accumulate_pointer_travel_distance(model, site, function, body).is_some() {
        claims.push(proposal(
            "accumulatePointerTravelDistance",
            "events_pointer_travel_distance",
            "catch-guarded handler measures sqrt(pow(clientX - previousX, 2) + pow(clientY - previousY, 2)), stores the current coordinates, and adds the distance to the captured total with an isNaN fallback of zero",
            f,
        ));
    }
    claims
}
