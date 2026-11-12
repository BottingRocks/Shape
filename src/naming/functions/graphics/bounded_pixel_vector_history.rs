use super::*;

use crate::naming::support::seeded_flow::*;

fn returned<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    match s {
        Statement::ReturnStatement(s) => s.argument.as_ref(),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// `recordPixelVectorSample`: bounded unique pixel-vector history.
// ---------------------------------------------------------------------------
fn void_return(s: &Statement<'_>) -> bool {
    matches!(
        s,
        Statement::ReturnStatement(r)
            if matches!(r.argument.as_ref(), Some(Expression::UnaryExpression(u))
                if u.operator.as_str() == "void" && number(&u.argument, 0.0))
    )
}

fn second_field(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    site: FunctionSite,
    field: &str,
) -> bool {
    member(e, field).is_some_and(|e| model.site_parameter_slot(e, site) == Some(1))
}

fn assignment_to_second_field(
    model: &Model<'_, '_>,
    a: &AssignmentExpression<'_>,
    site: FunctionSite,
    field: &str,
) -> bool {
    let AssignmentTarget::StaticMemberExpression(m) = &a.left else {
        return false;
    };
    !m.optional && m.property.name == field && model.site_parameter_slot(&m.object, site) == Some(1)
}

fn slice_copy(model: &Model<'_, '_>, e: &Expression<'_>, vector: Symbol) -> bool {
    let Some(c) = call(e) else {
        return false;
    };
    let Some(method) = member(&c.callee, "call") else {
        return false;
    };
    let Some(slice) = member(method, "slice") else {
        return false;
    };
    let Some(prototype) = member(slice, "prototype") else {
        return false;
    };
    is_global(model, prototype, &["Array"])
        && arity(c, 1).is_some_and(|args| symbol(model, args[0]) == Some(vector))
}

fn comparison_callback(model: &Model<'_, '_>, arg: &Expression<'_>, vector: Symbol) -> Option<()> {
    let Expression::ArrowFunctionExpression(callback) = arg else {
        return None;
    };
    if callback.r#async || !callback.body.directives.is_empty() || callback.params.rest.is_some() {
        return None;
    }
    let [param] = callback.params.items.as_slice() else {
        return None;
    };
    if param.initializer.is_some() {
        return None;
    }
    let BindingPattern::BindingIdentifier(id) = &param.pattern else {
        return None;
    };
    let element = id.symbol_id.get()?;
    if !unchanged(model, element) {
        return None;
    }
    let [declare, Statement::IfStatement(compare), done] = callback.body.statements.as_slice()
    else {
        return None;
    };
    let result = plain_local(model, one_variable(declare)?)?;
    if matches!(one_variable(declare)?.init.as_ref(), Some(_)) {
        return None;
    }
    let Expression::LogicalExpression(or) = &compare.test else {
        return None;
    };
    let Expression::LogicalExpression(or2) = &or.left else {
        return None;
    };
    let Expression::BinaryExpression(null_a) = &or2.left else {
        return None;
    };
    let Expression::BinaryExpression(null_b) = &or2.right else {
        return None;
    };
    let Expression::BinaryExpression(length) = &or.right else {
        return None;
    };
    if or.operator.as_str() != "||"
        || or2.operator.as_str() != "||"
        || null_a.operator.as_str() != "=="
        || !matches!(&null_a.right, Expression::NullLiteral(_))
        || symbol(model, &null_a.left) != Some(vector)
        || null_b.operator.as_str() != "=="
        || !matches!(&null_b.right, Expression::NullLiteral(_))
        || symbol(model, &null_b.left) != Some(element)
        || length.operator.as_str() != "!=="
        || !matches!(member(&length.left, "length"), Some(e) if symbol(model, e) == Some(vector))
        || !matches!(member(&length.right, "length"), Some(e) if symbol(model, e) == Some(element))
    {
        return None;
    }
    let Statement::BlockStatement(yes) = &compare.consequent else {
        return None;
    };
    let Statement::BlockStatement(no) = compare.alternate.as_ref()? else {
        return None;
    };
    let [set_false] = yes.body.as_slice() else {
        return None;
    };
    let set_false = assignment(set_false)?;
    if assign_symbol(model, set_false) != Some(result)
        || !matches!(&set_false.right, Expression::BooleanLiteral(b) if !b.value)
    {
        return None;
    }
    let [cursor_decl, Statement::ForStatement(loop_)] = no.body.as_slice() else {
        return None;
    };
    let cursor = plain_local(model, one_variable(cursor_decl)?)?;
    if !number(one_variable(cursor_decl)?.init.as_ref()?, 0.0)
        || loop_.init.is_some()
        || loop_.test.is_some()
        || loop_.update.is_some()
    {
        return None;
    }
    let Statement::BlockStatement(loop_body) = &loop_.body else {
        return None;
    };
    let [Statement::IfStatement(at_end), Statement::IfStatement(equal), assign_false, Statement::BreakStatement(_)] =
        loop_body.body.as_slice()
    else {
        return None;
    };
    let Expression::BinaryExpression(at_end_test) = &at_end.test else {
        return None;
    };
    let Statement::BlockStatement(at_end_body) = &at_end.consequent else {
        return None;
    };
    let [set_true, _] = at_end_body.body.as_slice() else {
        return None;
    };
    let set_true = assignment(set_true)?;
    if at_end.alternate.is_some()
        || at_end_test.operator.as_str() != ">="
        || symbol(model, &at_end_test.left) != Some(cursor)
        || !matches!(member(&at_end_test.right, "length"), Some(e) if symbol(model, e) == Some(vector))
        || assign_symbol(model, set_true) != Some(result)
        || !matches!(&set_true.right, Expression::BooleanLiteral(b) if b.value)
    {
        return None;
    }
    let Expression::BinaryExpression(equal_test) = &equal.test else {
        return None;
    };
    let Statement::BlockStatement(equal_body) = &equal.consequent else {
        return None;
    };
    let [increment, _] = equal_body.body.as_slice() else {
        return None;
    };
    if equal.alternate.is_some()
        || equal_test.operator.as_str() != "==="
        || !same_index(model, &equal_test.left, vector, cursor)
        || !same_index(model, &equal_test.right, element, cursor)
        || update_symbol(model, expression(increment)?, true, "++") != Some(cursor)
    {
        return None;
    }
    let assign_false = assignment(assign_false)?;
    if assign_symbol(model, assign_false) != Some(result)
        || !matches!(&assign_false.right, Expression::BooleanLiteral(b) if !b.value)
    {
        return None;
    }
    (returned(done).and_then(|e| symbol(model, e)) == Some(result)).then_some(())
}

fn same_index(model: &Model<'_, '_>, e: &Expression<'_>, owner: Symbol, cursor: Symbol) -> bool {
    let Expression::ComputedMemberExpression(m) = e else {
        return false;
    };
    !m.optional
        && symbol(model, &m.object) == Some(owner)
        && symbol(model, &m.expression) == Some(cursor)
}

fn pixel_vector_sample(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [vector_decl, Statement::IfStatement(unique), bump, Statement::IfStatement(limit)] =
        body.statements.as_slice()
    else {
        return None;
    };
    let vector = plain_local(model, one_variable(vector_decl)?)?;
    if model.site_parameter_slot(one_variable(vector_decl)?.init.as_ref()?, site) != Some(0) {
        return None;
    }
    let Expression::CallExpression(some) = &unique.test else {
        return None;
    };
    let Some(list) = member(&some.callee, "some") else {
        return None;
    };
    let Some(record) = member(list, "a") else {
        return None;
    };
    let args = arity(some, 1)?;
    if model.site_parameter_slot(record, site) != Some(1) {
        return None;
    }
    comparison_callback(model, args[0], vector)?;
    let Statement::BlockStatement(unique_body) = &unique.consequent else {
        return None;
    };
    let [drop] = unique_body.body.as_slice() else {
        return None;
    };
    if !void_return(drop) {
        return None;
    }
    let bump = assignment(bump)?;
    if !assignment_to_second_field(model, bump, site, "b") {
        return None;
    }
    let Expression::BinaryExpression(plus) = &bump.right else {
        return None;
    };
    let Some(number_call) = call(&plus.left) else {
        return None;
    };
    if plus.operator.as_str() != "+"
        || !number(&plus.right, 1.0)
        || !is_global(model, &number_call.callee, &["Number"])
        || !second_field(model, arity(number_call, 1)?.first().copied()?, site, "b")
    {
        return None;
    }
    let Expression::BinaryExpression(over) = &limit.test else {
        return None;
    };
    if over.operator.as_str() != ">"
        || !matches!(member(&over.left, "length"), Some(e) if second_field(model, e, site, "a"))
        || !number(&over.right, 5.0)
    {
        return None;
    }
    let Statement::BlockStatement(yes) = &limit.consequent else {
        return None;
    };
    let [drop] = yes.body.as_slice() else {
        return None;
    };
    if !void_return(drop) {
        return None;
    }
    let Statement::BlockStatement(no) = limit.alternate.as_ref()? else {
        return None;
    };
    match no.body.as_slice() {
        [push, done] => {
            if !void_return(done) {
                return None;
            }
            let push = statement_call(push)?;
            let Some(object) = member(&push.callee, "push") else {
                return None;
            };
            let Some(list) = member(object, "a") else {
                return None;
            };
            let args = arity(push, 1)?;
            (model.site_parameter_slot(list, site) == Some(1) && slice_copy(model, args[0], vector))
                .then_some(())
        }
        [copy, push, done] => {
            let copy_symbol = plain_local(model, one_variable(copy)?)?;
            if !void_return(done) || !slice_copy(model, one_variable(copy)?.init.as_ref()?, vector)
            {
                return None;
            }
            let push = statement_call(push)?;
            let Some(object) = member(&push.callee, "push") else {
                return None;
            };
            let Some(list) = member(object, "a") else {
                return None;
            };
            let args = arity(push, 1)?;
            (model.site_parameter_slot(list, site) == Some(1)
                && symbol(model, args[0]) == Some(copy_symbol))
            .then_some(())
        }
        _ => None,
    }
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
    if function.params.items.len() == 2 && pixel_vector_sample(model, site, body).is_some() {
        claims.push(proposal(
            "recordPixelVectorSample",
            "graphics_bounded_pixel_vector_history",
            "bounded unique history: vector copy of formal slot 0, exact element-by-element comparison callback and null/length guards, captured counter increment, exact five-entry limit, and guarded Array.prototype.slice copy pushed into formal slot 1's list",
            f,
        ));
    }
    claims
}
