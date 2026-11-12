//! Bounded dataflow proofs for container lookup and code-unit reconstruction.
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
fn index<'a>(e: &'a Expression<'a>) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::ComputedMemberExpression(m) = e else {
        return None;
    };
    (!m.optional).then_some((&m.object, &m.expression))
}
fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(&b.body)
}
fn returned<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ReturnStatement(r) = s else {
        return None;
    };
    r.argument.as_ref()
}
fn binary<'a>(e: &'a Expression<'a>, op: &str) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::BinaryExpression(b) = e else {
        return None;
    };
    (b.operator.as_str() == op).then_some((&b.left, &b.right))
}
fn plain(model: &Model<'_, '_>, p: &BindingPattern<'_>, writes: usize) -> Option<SymbolId> {
    let BindingPattern::BindingIdentifier(id) = p else {
        return None;
    };
    let s = id.symbol_id.get()?;
    (model.binding_writes(s) == writes && model.scoping.symbol_redeclarations(s).is_empty())
        .then_some(s)
}
fn number(e: &Expression<'_>, n: f64) -> bool {
    matches!(e, Expression::NumericLiteral(v) if v.value == n)
}
fn capture(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<SymbolId> {
    capture_symbol(model, site, symbol(model, e)?)
}
fn capture_symbol(model: &Model<'_, '_>, site: FunctionSite, s: SymbolId) -> Option<SymbolId> {
    if site.span.contains_inclusive(model.scoping.symbol_span(s))
        || !model.scoping.symbol_redeclarations(s).is_empty()
    {
        return None;
    }
    // A real captured variable, never a parameter, catch or destructuring origin.
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

fn bucket_record(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
    params: &[SymbolId],
) -> Option<()> {
    let [object, key] = params else { return None };
    let [Statement::IfStatement(absent), initial, Statement::WhileStatement(search), last] =
        body.statements.as_slice()
    else {
        return None;
    };
    let (bucket, null) = binary(&absent.test, "==")?;
    if !matches!(null, Expression::NullLiteral(_)) || absent.alternate.is_some() {
        return None;
    }
    let (table, bucket_key) = index(bucket)?;
    let table = capture(model, site, table)?;
    if symbol(model, bucket_key) != Some(*key) {
        return None;
    }
    let is_bucket = |e: &Expression<'_>| {
        index(e)
            .is_some_and(|(a, b)| symbol(model, a) == Some(table) && symbol(model, b) == Some(*key))
    };
    let [missing] = block(&absent.consequent)? else {
        return None;
    };
    if !matches!(returned(missing)?, Expression::NullLiteral(_))
        || !matches!(returned(last)?, Expression::NullLiteral(_))
    {
        return None;
    }
    let initial = one_variable(initial)?;
    let cursor = plain(model, &initial.id, 1)?;
    if !number(initial.init.as_ref()?, 0.0) {
        return None;
    }
    let (left, length) = binary(&search.test, "<")?;
    if symbol(model, left) != Some(cursor) || !is_bucket(member(length, "length")?) {
        return None;
    }
    let [Statement::IfStatement(found)] = block(&search.body)? else {
        return None;
    };
    let (needle, field) = binary(&found.test, "===")?;
    if symbol(model, needle) != Some(*object) {
        return None;
    }
    let record = member(field, "object")?;
    let is_record = |e: &Expression<'_>| {
        index(e).is_some_and(|(a, b)| is_bucket(a) && symbol(model, b) == Some(cursor))
    };
    if !is_record(record) {
        return None;
    }
    let [result] = block(&found.consequent)? else {
        return None;
    };
    if !is_record(returned(result)?) {
        return None;
    }
    let [Statement::ExpressionStatement(step), Statement::ContinueStatement(next)] =
        block(found.alternate.as_ref()?)?
    else {
        return None;
    };
    if next.label.is_some() {
        return None;
    }
    let Expression::UpdateExpression(update) = &step.expression else {
        return None;
    };
    if update.operator.as_str() != "++"
        || !matches!(&update.argument, SimpleAssignmentTarget::AssignmentTargetIdentifier(id) if crate::analysis::identifier_symbol(id,model.scoping) == Some(cursor))
    {
        return None;
    }
    Some(())
}

fn code_units(model: &Model<'_, '_>, body: &FunctionBody<'_>, params: &[SymbolId]) -> Option<()> {
    let [input] = params else { return None };
    let (initial, guard, packed, last) = match body.statements.as_slice() {
        [initial, Statement::IfStatement(guard), last] => (initial, guard, None, last),
        [initial, Statement::IfStatement(guard), packed, last] => {
            (initial, guard, Some(packed), last)
        }
        _ => return None,
    };
    let initial = one_variable(initial)?;
    let output = plain(model, &initial.id, 0)?;
    if !matches!(initial.init.as_ref()?, Expression::ArrayExpression(a) if a.elements.is_empty())
        || !null_test(model, &guard.test, *input, "!=")
        || guard.alternate.is_some()
    {
        return None;
    }
    let [Statement::ExpressionStatement(each)] = block(&guard.consequent)? else {
        return None;
    };
    let Expression::CallExpression(each) = &each.expression else {
        return None;
    };
    if each.optional || symbol(model, member(&each.callee, "forEach")?) != Some(*input) {
        return None;
    }
    let [Argument::ArrowFunctionExpression(callback)] = each.arguments.as_slice() else {
        return None;
    };
    let [parameter] = callback.params.items.as_slice() else {
        return None;
    };
    if callback.r#async
        || callback.params.rest.is_some()
        || parameter.initializer.is_some()
        || !callback.body.directives.is_empty()
    {
        return None;
    }
    let unit = plain(model, &parameter.pattern, 0)?;
    let [Statement::ExpressionStatement(push)] = callback.body.statements.as_slice() else {
        return None;
    };
    // Both expression and block arrows are safe: forEach ignores their result.
    let Expression::CallExpression(push) = &push.expression else {
        return None;
    };
    if push.optional || symbol(model, member(&push.callee, "push")?) != Some(output) {
        return None;
    }
    let [Argument::CallExpression(convert)] = push.arguments.as_slice() else {
        return None;
    };
    if convert.optional
        || expression_path(model, &convert.callee) != ["String", "fromCharCode"]
        || !matches!(&convert.callee, Expression::StaticMemberExpression(m) if !m.optional)
    {
        return None;
    }
    let args = call_arguments(convert);
    if convert.arguments.len() != 1 || args.len() != 1 || symbol(model, args[0]) != Some(unit) {
        return None;
    }
    let Expression::CallExpression(join) = returned(last)? else {
        return None;
    };
    if join.optional {
        return None;
    }
    if let Some(packed) = packed {
        let v = one_variable(packed)?;
        let pair = plain(model, &v.id, 0)?;
        let Expression::ArrayExpression(array) = v.init.as_ref()? else {
            return None;
        };
        let [receiver, method] = array.elements.as_slice() else {
            return None;
        };
        if symbol(model, receiver.as_expression()?) != Some(output)
            || symbol(model, member(method.as_expression()?, "join")?) != Some(output)
        {
            return None;
        }
        let (target, slot) = index(member(&join.callee, "call")?)?;
        if symbol(model, target) != Some(pair) || !number(slot, 1.0) {
            return None;
        }
        let args = call_arguments(join);
        if join.arguments.len() != 2 || args.len() != 2 {
            return None;
        }
        let (receiver, slot) = index(args[0])?;
        if symbol(model, receiver) != Some(pair)
            || !number(slot, 0.0)
            || !matches!(args[1], Expression::StringLiteral(s) if s.value.is_empty())
        {
            return None;
        }
    } else if symbol(model, member(&join.callee, "join")?) != Some(output)
        || !matches!(join.arguments.as_slice(), [Argument::StringLiteral(s)] if s.value.is_empty())
    {
        return None;
    }
    Some(())
}

// These roles describe mutable storage, never a captured value's callable origin.
// Exact owned statements keep getters, identity predicates and ordered setters
// separate; no aliases or value-flow scans are needed.
fn captured_state_role(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
    params: &[SymbolId],
) -> Option<&'static str> {
    match (params, body.statements.as_slice()) {
        ([], [s]) if capture(model, site, returned(s)?).is_some() => Some("readCapturedState"),
        ([input], [s]) => {
            let (left, right) = binary(returned(s)?, "===")?;
            if (symbol(model, left) == Some(*input) && capture(model, site, right).is_some())
                || (symbol(model, right) == Some(*input) && capture(model, site, left).is_some())
            {
                Some("isCapturedState")
            } else {
                None
            }
        }
        ([_, _, _], [_, _, _]) => {
            let mut slots = Vec::with_capacity(3);
            for (statement, parameter) in body.statements.iter().zip(params) {
                let Statement::ExpressionStatement(statement) = statement else {
                    return None;
                };
                let Expression::AssignmentExpression(a) = &statement.expression else {
                    return None;
                };
                let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
                    return None;
                };
                if a.operator.as_str() != "=" || symbol(model, &a.right) != Some(*parameter) {
                    return None;
                }
                let slot = capture_symbol(
                    model,
                    site,
                    crate::analysis::identifier_symbol(id, model.scoping)?,
                )?;
                if slots.contains(&slot) {
                    return None;
                }
                slots.push(slot);
            }
            Some("setCapturedStateTriple")
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
        return vec![];
    };
    let Some(body) = function.body.as_ref() else {
        return vec![];
    };
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !body.directives.is_empty()
    {
        return vec![];
    }
    let params: Option<Vec<_>> = function
        .params
        .items
        .iter()
        .map(|p| {
            if p.initializer.is_some() {
                return None;
            }
            plain(model, &p.pattern, 0)
        })
        .collect();
    let Some(params) = params else { return vec![] };
    let mut claims = Vec::new();
    if let Some(name) = captured_state_role(model, site, body, &params) {
        claims.push(proposal(name, "shared_state_slots", "exact captured plain-variable storage operation: sole slot return, strict formal-to-slot identity predicate, or three ordered formal-to-distinct-slot assignments with void completion; no callable alias resolution", f));
    }
    if bucket_record(model, site, body, &params).is_some() {
        claims.push(proposal("findObjectRecordInBucket","shared_state_dataflow","null-guarded captured bucket search from index zero; strict formal object identity against each record.object; returns exact record or null; sole cursor increment in mismatch branch",f));
    }
    if code_units(model, body, &params).is_some() {
        claims.push(proposal("codeUnitsToString","shared_state_dataflow","fresh array receives exactly String.fromCharCode of each null-guarded input item; returns same array joined by empty string; scoped callback and unshadowed global String proven",f));
    }
    claims
}
