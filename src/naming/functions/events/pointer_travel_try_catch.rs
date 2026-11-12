use super::*;

use crate::naming::support::entropy_relay::*;

fn power_of_field_difference<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &'a Expression<'a>,
    slot: u32,
    field: &str,
) -> Option<Symbol> {
    let Expression::CallExpression(pow) = e else {
        return None;
    };
    if !global_math_call(model, pow, "pow") {
        return None;
    }
    let arguments = call_arguments(pow);
    let [difference, two] = arguments.as_slice() else {
        return None;
    };
    let Expression::BinaryExpression(difference) = difference else {
        return None;
    };
    let Expression::StaticMemberExpression(read) = &difference.left else {
        return None;
    };
    if !number(two, 2.0)
        || difference.operator.as_str() != "-"
        || read.optional
        || read.property.name != field
        || slot_of(model, site, &read.object) != Some(slot)
    {
        return None;
    }
    symbol(model, &difference.right)
}

fn store_coordinate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    store: &AssignmentExpression<'_>,
    captured: Symbol,
    field: &str,
) -> bool {
    if store.operator.as_str() != "=" {
        return false;
    }
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &store.left else {
        return false;
    };
    let Expression::StaticMemberExpression(read) = &store.right else {
        return false;
    };
    crate::analysis::identifier_symbol(target, model.scoping) == Some(captured)
        && !read.optional
        && read.property.name == field
        && slot_of(model, site, &read.object) == Some(0)
}

fn guarded_delta(model: &Model<'_, '_>, e: &Expression<'_>, delta: Symbol) -> bool {
    let Expression::ConditionalExpression(conditional) = e else {
        return false;
    };
    let Expression::CallExpression(test) = &conditional.test else {
        return false;
    };
    let Some(argument) = single_argument(test) else {
        return false;
    };
    !test.optional
        && matches!(&test.callee, Expression::Identifier(i) if i.name == "isNaN")
        && is_symbol(model, argument, delta)
        && matches!(&conditional.consequent, Expression::NumericLiteral(n) if n.value == 0.0)
        && is_symbol(model, &conditional.alternate, delta)
}

// ``event.clientX/clientY`` distance from the last stored point, storing the
// new point and adding the finite distance to a captured accumulator.
fn accumulate_pointer_travel_distance(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> bool {
    let [Statement::TryStatement(t)] = body.statements.as_slice() else {
        return false;
    };
    if t.finalizer.is_some() {
        return false;
    }
    let [Statement::VariableDeclaration(_), Statement::ExpressionStatement(_), Statement::ExpressionStatement(_), Statement::ExpressionStatement(_), Statement::ExpressionStatement(_)] =
        t.block.body.as_slice()
    else {
        return false;
    };
    let statements = &t.block.body;
    let Some(delta_value) = one_variable(&statements[0]) else {
        return false;
    };
    let Some(delta) = plain_declarator(model, delta_value) else {
        return false;
    };
    if model.binding_writes(delta) != 1 {
        return false;
    }
    let Some(Expression::BinaryExpression(vertical)) = delta_value.init.as_ref() else {
        return false;
    };
    let Some(last_y) = data_capture(model, site, &vertical.right) else {
        return false;
    };
    let Expression::StaticMemberExpression(field_y) = &vertical.left else {
        return false;
    };
    if vertical.operator.as_str() != "-"
        || field_y.optional
        || field_y.property.name != "clientY"
        || slot_of(model, site, &field_y.object) != Some(0)
    {
        return false;
    }
    let Some(expr) = expression_of(&statements[1]) else {
        return false;
    };
    let Expression::AssignmentExpression(update) = expr else {
        return false;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &update.left else {
        return false;
    };
    if update.operator.as_str() != "="
        || crate::analysis::identifier_symbol(target, model.scoping) != Some(delta)
    {
        return false;
    }
    let Expression::CallExpression(root) = &update.right else {
        return false;
    };
    let Some(math) = member(&root.callee, "sqrt").and_then(|e| data_capture(model, site, e)) else {
        return false;
    };
    let Some(sum) = single_argument(root) else {
        return false;
    };
    let Expression::BinaryExpression(sum) = sum else {
        return false;
    };
    if root.optional || sum.operator.as_str() != "+" {
        return false;
    }
    let Some(last_x) = power_of_field_difference(model, site, &sum.left, 0, "clientX") else {
        return false;
    };
    let Expression::CallExpression(pow) = &sum.right else {
        return false;
    };
    let pow_arguments = call_arguments(pow);
    let [pow_value, pow_two] = pow_arguments.as_slice() else {
        return false;
    };
    if pow.optional
        || !global_math_call(model, pow, "pow")
        || !is_symbol(model, pow_value, delta)
        || !number(pow_two, 2.0)
    {
        return false;
    }
    let Some(expr) = expression_of(&statements[2]) else {
        return false;
    };
    let Expression::AssignmentExpression(store_x) = expr else {
        return false;
    };
    let Some(expr) = expression_of(&statements[3]) else {
        return false;
    };
    let Expression::AssignmentExpression(store_y) = expr else {
        return false;
    };
    if !store_coordinate(model, site, store_x, last_x, "clientX")
        || !store_coordinate(model, site, store_y, last_y, "clientY")
    {
        return false;
    }
    let Some(expr) = expression_of(&statements[4]) else {
        return false;
    };
    let Expression::AssignmentExpression(accumulate) = expr else {
        return false;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(accumulator_id) = &accumulate.left else {
        return false;
    };
    let Some(accumulator) = crate::analysis::identifier_symbol(accumulator_id, model.scoping)
    else {
        return false;
    };
    if !captured_data_symbol(model, site, accumulator) {
        return false;
    }
    let Expression::BinaryExpression(add) = &accumulate.right else {
        return false;
    };
    if accumulate.operator.as_str() != "="
        || add.operator.as_str() != "+"
        || !is_symbol(model, &add.left, accumulator)
        || !guarded_delta(model, &add.right, delta)
    {
        return false;
    }
    let Some(handler) = t.handler.as_ref() else {
        return false;
    };
    let Some(parameter) = handler.param.as_ref() else {
        return false;
    };
    let BindingPattern::BindingIdentifier(error) = &parameter.pattern else {
        return false;
    };
    let Some(error) = error.symbol_id.get() else {
        return false;
    };
    let [Statement::ExpressionStatement(failure)] = handler.body.body.as_slice() else {
        return false;
    };
    let Expression::CallExpression(failure) = &failure.expression else {
        return false;
    };
    let Some(reporter) =
        member(&failure.callee, "error").and_then(|e| data_capture(model, site, e))
    else {
        return false;
    };
    let Some(value) = single_argument(failure) else {
        return false;
    };
    !(failure.optional
        || !is_symbol(model, value, error)
        || reporter == delta
        || reporter == last_x
        || reporter == last_y
        || reporter == accumulator
        || reporter == math
        || math == last_x
        || math == last_y
        || math == delta
        || delta == last_x
        || delta == last_y
        || last_x == last_y
        || last_x == accumulator
        || last_y == accumulator)
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
        || function.params.rest.is_some()
        || !body.directives.is_empty()
    {
        return Vec::new();
    }
    let plain_params = function.params.items.iter().all(|p| {
        p.initializer.is_none()
            && matches!(&p.pattern, BindingPattern::BindingIdentifier(id)
                if id.symbol_id.get().is_some_and(|s| unchanged(model, s)))
    });
    if !plain_params || function.params.items.len() != 1 || f.calls.is_empty() {
        return Vec::new();
    }
    let input = function.params.items.first().and_then(|parameter| {
        let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
            return None;
        };
        identifier.symbol_id.get()
    });
    let _ = input;
    let mut claims = Vec::new();
    if accumulate_pointer_travel_distance(model, site, body) {
        claims.push(proposal(
            "accumulatePointerTravelDistance",
            "events_pointer_travel_try_catch",
            "try computes the Euclidean pointer travel from formal0.clientX/clientY to two distinct captured last-point slots, stores the new point and adds the finite distance to a distinct captured accumulator; the sole catch forwards the caught binding to a captured error receiver",
            f,
        ));
    }
    claims
}
