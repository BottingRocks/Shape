//! Exact own syntax only. Capture positions are free ports, never external
//! identities. In particular, changing getters and external writes are opaque.
use super::super::support::interaction::*;
use super::super::support::reassembly::update_symbol;
use super::*;

fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(b.body.as_slice())
}
fn sole<'a>(s: &'a Statement<'a>) -> Option<&'a Statement<'a>> {
    let [s] = block(s)? else {
        return None;
    };
    Some(s)
}
fn void_zero(e: &Expression<'_>) -> bool {
    matches!(e, Expression::UnaryExpression(u) if u.operator.as_str() == "void" && number(&u.argument, 0.0))
}
fn terminal(s: &Statement<'_>) -> Option<()> {
    let Statement::ReturnStatement(r) = s else {
        return None;
    };
    void_zero(r.argument.as_ref()?).then_some(())
}
fn null_symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<SymbolId> {
    let (left, right) = binary(e, "==")?;
    if !matches!(right, Expression::NullLiteral(_)) {
        return None;
    }
    symbol(model, left)
}
fn local(model: &Model<'_, '_>, v: &VariableDeclarator<'_>, writes: usize) -> Option<SymbolId> {
    let id = plain_var(model, v)?;
    (model.binding_writes(id) == writes).then_some(id)
}
fn pair(model: &Model<'_, '_>, s: &Statement<'_>) -> Option<(SymbolId, SymbolId)> {
    let Statement::VariableDeclaration(d) = s else {
        return None;
    };
    if !matches!(
        d.kind,
        VariableDeclarationKind::Var
            | VariableDeclarationKind::Let
            | VariableDeclarationKind::Const
    ) {
        return None;
    }
    let [t, v] = d.declarations.as_slice() else {
        return None;
    };
    if t.init.is_some() || v.init.is_some() {
        return None;
    }
    Some((local(model, t, 2)?, local(model, v, 2)?))
}
fn assigned<'a>(
    model: &Model<'_, '_>,
    s: &'a Statement<'a>,
    id: SymbolId,
) -> Option<&'a Expression<'a>> {
    let a = assignment(expression_statement(s)?)?;
    (target_symbol(model, &a.left) == Some(id)).then_some(&a.right)
}
fn computed<'a>(e: &'a Expression<'a>) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::ComputedMemberExpression(m) = e else {
        return None;
    };
    (!m.optional).then_some((&m.object, &m.expression))
}
fn lowering(
    model: &Model<'_, '_>,
    site: FunctionSite,
    guard: &Statement<'_>,
    invoke: &Statement<'_>,
    t: SymbolId,
    v: SymbolId,
) -> Option<(SymbolId, SymbolId)> {
    let Statement::IfStatement(g) = guard else {
        return None;
    };
    let h = captured_symbol(model, site, null_symbol(model, &g.test)?)?;
    let [a, b] = block(&g.consequent)? else {
        return None;
    };
    if null_symbol(model, assigned(model, a, t)?) != Some(h) || !void_zero(assigned(model, b, v)?) {
        return None;
    }
    let [a, b] = block(g.alternate.as_ref()?)? else {
        return None;
    };
    if symbol(model, member(assigned(model, a, v)?, "onBeforeInvoke")?) != Some(h)
        || null_symbol(model, assigned(model, b, t)?) != Some(v)
    {
        return None;
    }
    let Statement::IfStatement(g) = invoke else {
        return None;
    };
    let Expression::UnaryExpression(n) = &g.test else {
        return None;
    };
    if n.operator.as_str() != "!" || symbol(model, &n.argument) != Some(t) || g.alternate.is_some()
    {
        return None;
    }
    let c = call(expression_statement(sole(&g.consequent)?)?)?;
    if symbol(model, member(&c.callee, "unregister")?) != Some(v) {
        return None;
    }
    let [arg] = c.arguments.as_slice() else {
        return None;
    };
    Some((
        h,
        captured_symbol(model, site, symbol(model, arg.as_expression()?)?)?,
    ))
}
fn matches_operation(model: &Model<'_, '_>, site: FunctionSite) -> Option<[SymbolId; 15]> {
    let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
        return None;
    };
    if f.r#type != FunctionType::FunctionDeclaration
        || f.r#async
        || f.generator
        || f.params.rest.is_some()
        || !f.params.items.is_empty()
    {
        return None;
    }
    let body = f.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let [Statement::IfStatement(g1), Statement::IfStatement(g2)] = body.statements.as_slice()
    else {
        return None;
    };
    if g1.alternate.is_some() {
        return None;
    }
    terminal(sole(&g1.consequent)?)?;
    terminal(sole(&g2.consequent)?)?;
    let g1 = captured_symbol(model, site, null_symbol(model, &g1.test)?)?;
    let g2_sym = captured_symbol(model, site, null_symbol(model, &g2.test)?)?;
    let [b0, b1, b2, b3, b4, b5, b6, b7, b8, b9, b10] = block(g2.alternate.as_ref()?)? else {
        return None;
    };
    let alias = variable(b0)?;
    let b = local(model, alias, 0)?;
    if symbol(model, alias.init.as_ref()?) != Some(g2_sym) {
        return None;
    }
    let list = variable(b1)?;
    let l = local(model, list, 0)?;
    let concat = call(list.init.as_ref()?)?;
    let k1 = captured_symbol(
        model,
        site,
        symbol(model, member(&concat.callee, "concat")?)?,
    )?;
    let [arg] = concat.arguments.as_slice() else {
        return None;
    };
    let k2 = captured_symbol(model, site, symbol(model, arg.as_expression()?)?)?;
    let cursor = variable(b2)?;
    let c = local(model, cursor, 1)?;
    if !number(cursor.init.as_ref()?, 0.0) {
        return None;
    }
    let Statement::ForStatement(iterate) = b3 else {
        return None;
    };
    if iterate.init.is_some() {
        return None;
    }
    let (left, right) = binary(iterate.test.as_ref()?, "<")?;
    if symbol(model, left) != Some(c) || symbol(model, member(right, "length")?) != Some(l) {
        return None;
    }
    let Expression::UpdateExpression(u) = iterate.update.as_ref()? else {
        return None;
    };
    if u.prefix || u.operator.as_str() != "++" || update_symbol(model, &u.argument) != Some(c) {
        return None;
    }
    let Expression::UnaryExpression(d) = expression_statement(sole(&iterate.body)?)? else {
        return None;
    };
    if d.operator.as_str() != "delete" {
        return None;
    }
    let (object, key) = computed(&d.argument)?;
    let (list, index) = computed(key)?;
    if symbol(model, object) != Some(b)
        || symbol(model, list) != Some(l)
        || symbol(model, index) != Some(c)
    {
        return None;
    }
    let (t1, v1) = pair(model, b4)?;
    let (h1, c1) = lowering(model, site, b5, b6, t1, v1)?;
    let (t2, v2) = pair(model, b7)?;
    let (h2, c2) = lowering(model, site, b8, b9, t2, v2)?;
    terminal(b10)?;
    // Exhaustive traversal above entails the corrected 8 null / 8 == inventory,
    // 6 numeric zeros / 5 voids, three calls, and all getter/read/call positions.
    // No capture initializer, writer, spelling or declaration order is inspected.
    let roles = [g1, g2_sym, k1, k2, h1, h2, c1, c2, b, l, c, t1, v1, t2, v2];
    (roles.iter().copied().collect::<BTreeSet<_>>().len() == roles.len()).then_some(roles)
}
pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
) -> Vec<Proposal> {
    if matches_operation(model, site).is_none() {
        return vec![];
    }
    vec![proposal("deleteConcatenatedCapturedKeysAndCallUnregisterMembers", "shared_captured_key_list_unregister_operation",
        "Whole owned body: two captured-null early returns; never-reassigned alias of the second guard capture; captured.concat(captured) key list; zero-based postfix loop deleting alias[list[cursor]]; two ordered lowered optional-read blocks with literal repeated holder reads, one onBeforeInvoke read on each non-null path, and conditional unregister member calls on cached values with positional captured arguments; terminal void 0. Eight distinct resolved captures and seven distinct locals with exact write counts. Captured ports are opaque dynamic external values: writers, initialization, getter behavior, stable channels, callback identity, setup readiness, external call success and native XHR purpose are unproved. Global capture-port permutations identify the same generic operation family, without runtime equivalence or registration pairing.", f)]
}

#[cfg(test)]
#[path = "captured_key_list_unregister_tests.rs"]
mod tests;
