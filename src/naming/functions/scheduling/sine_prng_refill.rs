use super::*;

use crate::naming::support::seeded_flow::*;

fn index_number<'a>(e: &'a Expression<'a>, n: f64) -> Option<&'a Expression<'a>> {
    let Expression::ComputedMemberExpression(m) = e else {
        return None;
    };
    (!m.optional && matches!(&m.expression, Expression::NumericLiteral(v) if v.value == n))
        .then_some(&m.object)
}

// ---------------------------------------------------------------------------
// `refillSinPrngBuffer`: bounded sine-based PRNG buffer refill.
// ---------------------------------------------------------------------------
fn sin_prng_value(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    state: Symbol,
    cursor: Symbol,
) -> bool {
    let Some(abs) = call(e) else {
        return false;
    };
    if !is_global(model, &abs.callee, &["Math", "abs"]) {
        return false;
    }
    let Some(abs_args) = arity(abs, 1) else {
        return false;
    };
    let Expression::BinaryExpression(scale) = abs_args[0] else {
        return false;
    };
    let Some(fround) = call(&scale.left) else {
        return false;
    };
    if scale.operator.as_str() != "*"
        || !number(&scale.right, 2.0)
        || !is_global(model, &fround.callee, &["Math", "fround"])
    {
        return false;
    }
    let Some(fround_args) = arity(fround, 1) else {
        return false;
    };
    let Some(sin) = call(fround_args[0]) else {
        return false;
    };
    if !is_global(model, &sin.callee, &["Math", "sin"]) {
        return false;
    }
    let Some(sin_args) = arity(sin, 1) else {
        return false;
    };
    let Expression::BinaryExpression(sum) = sin_args[0] else {
        return false;
    };
    sum.operator.as_str() == "+"
        && symbol(model, &sum.left) == Some(state)
        && symbol(model, &sum.right) == Some(cursor)
}

fn ratio(model: &Model<'_, '_>, e: &Expression<'_>, limit: Symbol, cursor: Symbol) -> bool {
    let Expression::BinaryExpression(div) = e else {
        return false;
    };
    if div.operator.as_str() != "/" {
        return false;
    }
    let Expression::BinaryExpression(sub) = &div.left else {
        return false;
    };
    sub.operator.as_str() == "-"
        && symbol(model, &sub.left) == Some(limit)
        && symbol(model, &sub.right) == Some(cursor)
        && symbol(model, &div.right) == Some(limit)
}

fn push_of<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    c: &'a CallExpression<'a>,
) -> Option<&'a Expression<'a>> {
    let object = member(&c.callee, "push")?;
    (model.site_parameter_slot(object, site) == Some(0))
        .then(|| arity(c, 1))
        .flatten()
        .map(|args| args[0])
}

fn refill_sin_prng(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [cursor_decl, state_decl, limit_decl, Statement::WhileStatement(loop_), bump] =
        body.statements.as_slice()
    else {
        return None;
    };
    let cursor = plain_local(model, one_variable(cursor_decl)?)?;
    let state = plain_local(model, one_variable(state_decl)?)?;
    let limit = plain_local(model, one_variable(limit_decl)?)?;
    if !number(one_variable(cursor_decl)?.init.as_ref()?, 1.0)
        || !number(one_variable(limit_decl)?.init.as_ref()?, 8.0)
    {
        return None;
    }
    let Expression::BinaryExpression(add) = one_variable(state_decl)?.init.as_ref()? else {
        return None;
    };
    if add.operator.as_str() != "+" {
        return None;
    }
    let Expression::BinaryExpression(mul) = &add.left else {
        return None;
    };
    if mul.operator.as_str() != "*"
        || index_number(&mul.left, 0.0).and_then(|e| model.site_parameter_slot(e, site)) != Some(0)
        || !number(&mul.right, 63.0)
        || index_number(&add.right, 1.0).and_then(|e| model.site_parameter_slot(e, site)) != Some(0)
    {
        return None;
    }
    let Expression::BinaryExpression(test) = &loop_.test else {
        return None;
    };
    if test.operator.as_str() != "<="
        || symbol(model, &test.left) != Some(cursor)
        || symbol(model, &test.right) != Some(limit)
    {
        return None;
    }
    let Statement::BlockStatement(block) = &loop_.body else {
        return None;
    };
    let [state_assign, Statement::IfStatement(outer), Statement::ExpressionStatement(step)] =
        block.body.as_slice()
    else {
        return None;
    };
    let state_assign = assignment(state_assign)?;
    if assign_symbol(model, state_assign) != Some(state)
        || !sin_prng_value(model, &state_assign.right, state, cursor)
    {
        return None;
    }
    let Expression::BinaryExpression(at_least_one) = &outer.test else {
        return None;
    };
    if at_least_one.operator.as_str() != ">="
        || symbol(model, &at_least_one.left) != Some(state)
        || !number(&at_least_one.right, 1.0)
    {
        return None;
    }
    let Statement::BlockStatement(yes) = &outer.consequent else {
        return None;
    };
    let Statement::BlockStatement(no) = outer.alternate.as_ref()? else {
        return None;
    };
    let [Statement::IfStatement(grow)] = yes.body.as_slice() else {
        return None;
    };
    let Expression::BinaryExpression(less) = &grow.test else {
        return None;
    };
    let Statement::BlockStatement(grow_yes) = &grow.consequent else {
        return None;
    };
    let Statement::BlockStatement(grow_no) = grow.alternate.as_ref()? else {
        return None;
    };
    if less.operator.as_str() != "<"
        || symbol(model, &less.left) != Some(limit)
        || !number(&less.right, 50.0)
        || !matches!(grow_yes.body.as_slice(), [Statement::ExpressionStatement(step)]
            if update_symbol(model, &step.expression, true, "++") == Some(limit))
    {
        return None;
    }
    let [push_limit, rebase] = grow_no.body.as_slice() else {
        return None;
    };
    let push_limit = statement_call(push_limit)?;
    if !push_of(model, site, push_limit).is_some_and(|e| ratio(model, e, limit, cursor)) {
        return None;
    }
    let rebase = assignment(rebase)?;
    if assign_symbol(model, rebase) != Some(state) || !ratio(model, &rebase.right, limit, cursor) {
        return None;
    }
    let [push_state] = no.body.as_slice() else {
        return None;
    };
    let push_state = statement_call(push_state)?;
    if push_of(model, site, push_state).and_then(|e| symbol(model, e)) != Some(state)
        || update_symbol(model, &step.expression, true, "++") != Some(cursor)
    {
        return None;
    }
    let bump = assignment(bump)?;
    let AssignmentTarget::ComputedMemberExpression(target) = &bump.left else {
        return None;
    };
    if target.optional
        || model.site_parameter_slot(&target.object, site) != Some(0)
        || !number(&target.expression, 1.0)
    {
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
    {
        return None;
    }
    let Some(number_args) = arity(number_call, 1) else {
        return None;
    };
    let Expression::ComputedMemberExpression(read) = number_args[0] else {
        return None;
    };
    (!read.optional
        && model.site_parameter_slot(&read.object, site) == Some(0)
        && number(&read.expression, 1.0))
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
    if function.r#async
        || function.generator
        || !directives(body)
        || !plain_params(model, &function.params)
    {
        return Vec::new();
    }
    let mut claims = Vec::new();
    if function.params.items.len() == 1 && refill_sin_prng(model, site, body).is_some() {
        claims.push(proposal(
            "refillSinPrngBuffer",
            "scheduling_sine_prng_refill",
            "exact sine/fround/abs PRNG refill loop with plain local cursor/state/limit identity, bounded growth to 50 and final numeric seed increment",
            f,
        ));
    }
    claims
}
