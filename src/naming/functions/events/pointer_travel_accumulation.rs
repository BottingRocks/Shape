use super::*;
use oxc_semantic::SymbolId;

use crate::naming::support::selector_graphics::*;

/// A captured mutable data slot that was initialized in its declarator. The
/// matched function may write it only through the exact operations proved
/// below; the binding itself is never a parameter, catch or loop target.
fn captured_data(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
) -> Option<SymbolId> {
    captured_data_symbol(model, site, captured(model, site, e)?)
}

fn captured_data_symbol(
    model: &Model<'_, '_>,
    site: FunctionSite,
    s: SymbolId,
) -> Option<SymbolId> {
    captured_slot(model, site, s)?;
    let declaration_statement = model.nodes.parent_id(model.scoping.symbol_declaration(s));
    (model.initializer(s).is_some() && direct_body_statement(model, declaration_statement))
        .then_some(s)
}

fn power<'a>(model: &Model<'_, '_>, e: &'a Expression<'a>) -> Option<(&'a Expression<'a>, f64)> {
    let Expression::CallExpression(call) = e else {
        return None;
    };
    if call.optional
        || expression_path(model, &call.callee) != ["Math", "pow"]
        || call.arguments.len() != 2
    {
        return None;
    }
    let args = call_arguments(call);
    let exponent = match args[1] {
        Expression::NumericLiteral(n) => n.value,
        _ => return None,
    };
    Some((args[0], exponent))
}

// ---------------------------------------------------------------------------
// try {
//   let dy = event.clientY - previousY;
//   dy = sqrt(Math.pow(event.clientX - lastX, 2) + Math.pow(dy, 2));
//   lastX = event.clientX; previousY = event.clientY;
//   total = total + (isNaN(dy) ? 0 : dy);
// } catch (error) { observer.error(error); }
// ---------------------------------------------------------------------------
fn pointer_distance(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
    params: &[SymbolId],
) -> Option<()> {
    let [event] = params else { return None };
    let [Statement::TryStatement(attempt)] = body.statements.as_slice() else {
        return None;
    };
    if attempt.finalizer.is_some() || attempt.block.body.len() != 5 {
        return None;
    }
    let [Statement::VariableDeclaration(delta_declaration), Statement::ExpressionStatement(squared), Statement::ExpressionStatement(save_x), Statement::ExpressionStatement(save_y), Statement::ExpressionStatement(accumulate)] =
        attempt.block.body.as_slice()
    else {
        return None;
    };
    let [delta_declaration] = delta_declaration.declarations.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(delta_id) = &delta_declaration.id else {
        return None;
    };
    let delta = delta_id.symbol_id.get()?;
    if !model.scoping.symbol_redeclarations(delta).is_empty() {
        return None;
    }
    let (dy, previous_y) = binary(delta_declaration.init.as_ref()?, "-")?;
    if symbol(model, member(dy, "clientY")?) != Some(*event) {
        return None;
    }
    let previous_y = captured_data(model, site, previous_y)?;
    let Expression::AssignmentExpression(squared) = &squared.expression else {
        return None;
    };
    if squared.operator.as_str() != "="
        || !matches!(&squared.left, AssignmentTarget::AssignmentTargetIdentifier(id)
            if crate::analysis::identifier_symbol(id, model.scoping) == Some(delta))
    {
        return None;
    }
    let Expression::CallExpression(root) = &squared.right else {
        return None;
    };
    captured_receiver(model, site, member(&root.callee, "sqrt")?)?;
    if root.optional || root.arguments.len() != 1 {
        return None;
    }
    let (left, right) = binary(call_arguments(root).first()?, "+")?;
    let (square_x, exponent_x) = power(model, left)?;
    let (square_y, exponent_y) = power(model, right)?;
    if exponent_x != 2.0 || exponent_y != 2.0 {
        return None;
    }
    let (dx, last_x) = binary(square_x, "-")?;
    if symbol(model, member(dx, "clientX")?) != Some(*event)
        || symbol(model, square_y) != Some(delta)
    {
        return None;
    }
    let last_x = captured_data(model, site, last_x)?;
    let (saved_x, x_value) = assignment(save_x, model)?;
    if saved_x != last_x || symbol(model, member(x_value, "clientX")?) != Some(*event) {
        return None;
    }
    let (saved_y, y_value) = assignment(save_y, model)?;
    if saved_y != previous_y || symbol(model, member(y_value, "clientY")?) != Some(*event) {
        return None;
    }
    let (total, total_value) = assignment(accumulate, model)?;
    captured_data_symbol(model, site, total)?;
    let (lhs, added) = binary(total_value, "+")?;
    if symbol(model, lhs) != Some(total) {
        return None;
    }
    let Expression::ConditionalExpression(guard) = added else {
        return None;
    };
    if !matches!(&guard.test, Expression::CallExpression(call)
        if !call.optional && expression_path(model, &call.callee) == ["isNaN"]
            && call.arguments.len() == 1
            && symbol(model, call_arguments(call).first()?) == Some(delta))
        || !number(&guard.consequent, 0.0)
        || symbol(model, &guard.alternate) != Some(delta)
    {
        return None;
    }
    let handler = attempt.handler.as_ref()?;
    let caught = catch_symbol(model, handler)?;
    let [Statement::ExpressionStatement(report)] = handler.body.body.as_slice() else {
        return None;
    };
    let Expression::CallExpression(report) = &report.expression else {
        return None;
    };
    captured_receiver(model, site, member(&report.callee, "error")?)?;
    if report.optional
        || report.arguments.len() != 1
        || !matches!(&report.arguments[0], Argument::Identifier(id)
            if crate::analysis::identifier_symbol(id, model.scoping) == Some(caught))
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
    let Some(params) = plain_parameters(model, function) else {
        return Vec::new();
    };
    let mut claims = Vec::new();
    if pointer_distance(model, site, body, &params).is_some() {
        claims.push(proposal(
            "accumulatePointerTravelDistance",
            "events_pointer_travel_accumulation",
            "consecutive clientX/clientY delta is squared through Math.pow, reduced by one captured sqrt provider, saved back to the exact last-point slots, and added to the captured total behind an isNaN guard; catch forwards the caught binding to a captured error receiver",
            f,
        ));
    }
    claims
}
