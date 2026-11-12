//! Bounded property/search rules over exact owned statements and scoped identities.
use super::*;
use oxc_semantic::SymbolId;

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e, model.scoping)
}

fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = e else {
        return None;
    };
    (!m.optional && m.property.name == key).then_some(&m.object)
}

fn indexed<'a>(e: &'a Expression<'a>) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::ComputedMemberExpression(m) = e else {
        return None;
    };
    (!m.optional).then_some((&m.object, &m.expression))
}

fn returned<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ReturnStatement(r) = s else {
        return None;
    };
    r.argument.as_ref()
}

fn null_return(s: &Statement<'_>) -> bool {
    matches!(returned(s), Some(Expression::NullLiteral(_)))
}

fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(b.body.as_slice())
}

fn plain_parameters(model: &Model<'_, '_>, function: &Function<'_>) -> Option<Vec<SymbolId>> {
    if function.params.rest.is_some() {
        return None;
    }
    function
        .params
        .items
        .iter()
        .map(|p| {
            if p.initializer.is_some() {
                return None;
            }
            let BindingPattern::BindingIdentifier(id) = &p.pattern else {
                return None;
            };
            let s = id.symbol_id.get()?;
            (model.binding_writes(s) == 0 && model.scoping.symbol_redeclarations(s).is_empty())
                .then_some(s)
        })
        .collect()
}

// The capture is a plain variable, never a parameter, catch binding or loop target.
// This is a mutable storage slot, not a callable-origin proof: resetters may
// replace its value. Every lookup must still resolve to this exact symbol.
fn captured_table(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
) -> Option<SymbolId> {
    captured_slot(model, site, symbol(model, e)?)
}
fn captured_slot(model: &Model<'_, '_>, site: FunctionSite, s: SymbolId) -> Option<SymbolId> {
    if site.span.contains_inclusive(model.scoping.symbol_span(s))
        || !model.scoping.symbol_redeclarations(s).is_empty()
    {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(_)
        )
        || matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
    {
        return None;
    }
    Some(s)
}

fn bucket_is(model: &Model<'_, '_>, e: &Expression<'_>, table: SymbolId, key: SymbolId) -> bool {
    indexed(e).is_some_and(|(object, index)| {
        symbol(model, object) == Some(table) && symbol(model, index) == Some(key)
    })
}

fn record_is(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    table: SymbolId,
    key: SymbolId,
    cursor: SymbolId,
) -> bool {
    indexed(e).is_some_and(|(object, index)| {
        bucket_is(model, object, table, key) && symbol(model, index) == Some(cursor)
    })
}

fn increment(model: &Model<'_, '_>, s: &Statement<'_>, cursor: SymbolId) -> bool {
    matches!(s, Statement::ExpressionStatement(s) if matches!(&s.expression,
        Expression::UpdateExpression(u) if u.operator.as_str() == "++" && matches!(&u.argument,
            SimpleAssignmentTarget::AssignmentTargetIdentifier(i) if crate::analysis::identifier_symbol(i, model.scoping) == Some(cursor))))
}

fn bucket_record_search(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
    params: &[SymbolId],
) -> Option<()> {
    let [needle, key] = params else { return None };
    let [Statement::IfStatement(missing), init, Statement::WhileStatement(scan), end] =
        body.statements.as_slice()
    else {
        return None;
    };
    let Expression::BinaryExpression(test) = &missing.test else {
        return None;
    };
    if test.operator.as_str() != "=="
        || !matches!(&test.right, Expression::NullLiteral(_))
        || missing.alternate.is_some()
    {
        return None;
    }
    let (object, index) = indexed(&test.left)?;
    let table = captured_table(model, site, object)?;
    if symbol(model, index) != Some(*key)
        || !matches!(block(&missing.consequent)?, [s] if null_return(s))
        || !null_return(end)
    {
        return None;
    }
    let v = one_variable(init)?;
    let cursor = plain_local(model, v)?;
    if !matches!(v.init.as_ref()?, Expression::NumericLiteral(n) if n.value == 0.0)
        || model.binding_writes(cursor) != 1
    {
        return None;
    }
    let Expression::BinaryExpression(bound) = &scan.test else {
        return None;
    };
    if bound.operator.as_str() != "<"
        || symbol(model, &bound.left) != Some(cursor)
        || !bucket_is(model, member(&bound.right, "length")?, table, *key)
    {
        return None;
    }
    let steps = block(&scan.body)?;
    let Statement::IfStatement(found) = steps.first()? else {
        return None;
    };
    let Expression::BinaryExpression(equal) = &found.test else {
        return None;
    };
    if equal.operator.as_str() != "==="
        || symbol(model, &equal.left) != Some(*needle)
        || !record_is(model, member(&equal.right, "object")?, table, *key, cursor)
    {
        return None;
    }
    let [result] = block(&found.consequent)? else {
        return None;
    };
    if !record_is(model, returned(result)?, table, *key, cursor) {
        return None;
    }
    // Both common loop forms preserve the same forward scan. No other effects.
    if let Some(alternate) = &found.alternate {
        if steps.len() != 1 {
            return None;
        }
        let [step, Statement::ContinueStatement(c)] = block(alternate)? else {
            return None;
        };
        if c.label.is_some() || !increment(model, step, cursor) {
            return None;
        }
    } else {
        let [_, step] = steps else { return None };
        if !increment(model, step, cursor) {
            return None;
        }
    }
    Some(())
}

fn prevent_default(
    model: &Model<'_, '_>,
    body: &FunctionBody<'_>,
    params: &[SymbolId],
) -> Option<()> {
    let [event] = params else { return None };
    let [Statement::ExpressionStatement(s)] = body.statements.as_slice() else {
        return None;
    };
    let Expression::CallExpression(c) = &s.expression else {
        return None;
    };
    (!c.optional
        && c.arguments.is_empty()
        && symbol(model, member(&c.callee, "preventDefault")?) == Some(*event))
    .then_some(())
}

// Scalar DOM property paths encode the operation, not the spelling of a local.
fn scalar_reader(
    model: &Model<'_, '_>,
    body: &FunctionBody<'_>,
    params: &[SymbolId],
) -> Option<&'static str> {
    let [input] = params else { return None };
    let [s] = body.statements.as_slice() else {
        return None;
    };
    let e = returned(s)?;
    for (path, name) in [
        (&["target", "value"][..], "readEventTargetValue"),
        (&["target", "checked"][..], "readEventTargetChecked"),
        (
            &["target", "visibilityState"][..],
            "readEventTargetVisibilityState",
        ),
        (&["parentNode"][..], "getParentNode"),
        (&["childNodes"][..], "getChildNodes"),
    ] {
        let mut value = Some(e);
        for property in path.iter().rev() {
            value = value.and_then(|e| member(e, property));
        }
        if value.and_then(|e| symbol(model, e)) == Some(*input) {
            return Some(name);
        }
    }
    None
}

// A direct member call is retained as a member call, never resolved through
// a function alias. The role promises a setTimeout request, not a native timer.
fn deferred_event_target(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
    params: &[SymbolId],
) -> Option<()> {
    let [event] = params else { return None };
    let [Statement::ExpressionStatement(store), Statement::ExpressionStatement(schedule)] =
        body.statements.as_slice()
    else {
        return None;
    };
    let Expression::AssignmentExpression(store) = &store.expression else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(id) = &store.left else {
        return None;
    };
    if store.operator.as_str() != "="
        || symbol(model, member(&store.right, "target")?) != Some(*event)
    {
        return None;
    }
    let slot = captured_slot(
        model,
        site,
        crate::analysis::identifier_symbol(id, model.scoping)?,
    )?;
    let Expression::CallExpression(schedule) = &schedule.expression else {
        return None;
    };
    if schedule.optional {
        return None;
    }
    let receiver = captured_table(model, site, member(&schedule.callee, "setTimeout")?)?;
    if receiver == slot {
        return None;
    }
    let [Argument::ArrowFunctionExpression(callback), Argument::NumericLiteral(delay)] =
        schedule.arguments.as_slice()
    else {
        return None;
    };
    if delay.value != 0.0
        || callback.r#async
        || !callback.params.items.is_empty()
        || callback.params.rest.is_some()
        || !callback.body.directives.is_empty()
    {
        return None;
    }
    let [Statement::ExpressionStatement(clear)] = callback.body.statements.as_slice() else {
        return None;
    };
    let Expression::AssignmentExpression(clear) = &clear.expression else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(id) = &clear.left else {
        return None;
    };
    (clear.operator.as_str() == "="
        && crate::analysis::identifier_symbol(id, model.scoping) == Some(slot)
        && matches!(&clear.right, Expression::NullLiteral(_)))
    .then_some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return vec![];
    };
    let Some(body) = function.body.as_ref() else {
        return vec![];
    };
    if function.r#async || function.generator || !body.directives.is_empty() {
        return vec![];
    }
    let Some(params) = plain_parameters(model, function) else {
        return vec![];
    };
    let mut claims = Vec::new();
    if deferred_event_target(model, site, body, &params).is_some() {
        claims.push(proposal("captureEventTargetUntilTimeout", "shared_traversal_deferred_target", "stores unchanged formal.target in a captured plain-variable slot, then calls a distinct captured receiver.setTimeout with zero delay and a synchronous zero-argument arrow that only clears the identical scoped slot to null; void outer completion", f));
    }
    if bucket_record_search(model, site, body, &params).is_some() {
        claims.push(proposal("findObjectRecordInBucket", "shared_traversal_bucket_search", "exact null-guarded captured table[key] scan from zero to length, strict needle === record.object identity, returns that record or null; single scoped cursor increment and no extra effects", f));
    }
    if prevent_default(model, body, &params).is_some() {
        claims.push(proposal("preventDefaultHandler", "shared_traversal_dom_action", "sole action calls the unchanged formal's preventDefault method with no arguments and no returned value", f));
    }
    if let Some(name) = scalar_reader(model, body, &params) {
        claims.push(proposal(name, "shared_traversal_scalar_reader", "sole return reads an exact nonoptional DOM property path from the unchanged plain formal", f));
    }
    claims
}
