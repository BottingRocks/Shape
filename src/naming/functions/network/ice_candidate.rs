//! Exact candidate-record collection protocol over opaque matcher/report ports.
//! Native RTC/RegExp identity and matcher patterns are deliberately unproved.
use crate::naming::support::interaction::*;
use super::*;
use oxc_semantic::SymbolId;
fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(&b.body)
}
fn sole<'a>(s: &'a Statement<'a>) -> Option<&'a Statement<'a>> {
    let [s] = block(s)? else {
        return None;
    };
    Some(s)
}
fn local<'a>(
    model: &Model<'_, '_>,
    st: &'a Statement<'a>,
) -> Option<(SymbolId, Option<&'a Expression<'a>>)> {
    let Statement::VariableDeclaration(d) = st else {
        return None;
    };
    let [v] = d.declarations.as_slice() else {
        return None;
    };
    Some((plain_var(model, v)?, v.init.as_ref()))
}
fn id(model: &Model<'_, '_>, e: &Expression<'_>, x: SymbolId) -> bool {
    symbol(model, e) == Some(x)
}
fn empty(e: &Expression<'_>) -> bool {
    matches!(e,Expression::StringLiteral(s) if s.value.is_empty())
}
fn null(e: &Expression<'_>) -> bool {
    matches!(e, Expression::NullLiteral(_))
}
fn cap(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<SymbolId> {
    let s = captured_symbol(model, site, symbol(model, e)?)?;
    (model.scoping.symbol_span(s).end < site.span.start).then_some(s)
}
fn assigned<'a>(
    model: &Model<'_, '_>,
    st: &'a Statement<'a>,
    to: SymbolId,
) -> Option<&'a Expression<'a>> {
    let a = assignment(expression_statement(st)?)?;
    (target_symbol(model, &a.left) == Some(to)).then_some(&a.right)
}
fn index<'a>(model: &Model<'_, '_>, e: &'a Expression<'a>, x: SymbolId, n: f64) -> Option<()> {
    let Expression::ComputedMemberExpression(m) = e else {
        return None;
    };
    (!m.optional && id(model, &m.object, x) && number(&m.expression, n)).then_some(())
}
fn candidate(model: &Model<'_, '_>, e: &Expression<'_>, arg: SymbolId) -> bool {
    member(member(e, "candidate").unwrap_or(e), "candidate").is_some_and(|e| id(model, e, arg))
}
fn exec(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
    arg: SymbolId,
) -> Option<SymbolId> {
    let call = call(e)?;
    let a = arguments(call)?;
    if a.len() != 1 || !candidate(model, a[0], arg) {
        return None;
    }
    cap(model, site, member(&call.callee, "exec")?)
}
fn void_return(st: &Statement<'_>) -> bool {
    matches!(st,Statement::ReturnStatement(r) if matches!(r.argument.as_ref(),Some(Expression::UnaryExpression(u)) if u.operator.as_str()=="void"&&number(&u.argument,0.0)))
}
fn prove(model: &Model<'_, '_>, site: FunctionSite, f: &Function<'_>) -> Option<()> {
    if f.r#async || f.generator || f.params.rest.is_some() || f.params.items.len() != 1 {
        return None;
    }
    let arg = plain_param(model, &f.params.items[0])?;
    let body = f.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let [Statement::IfStatement(outer)] = body.statements.as_slice() else {
        return None;
    };
    let (event, null_value) = binary(&outer.test, "==")?;
    if !null(null_value) || !member(event, "candidate").is_some_and(|e| id(model, e, arg)) {
        return None;
    }
    let [close, report, ret] = block(&outer.consequent)? else {
        return None;
    };
    let close = call(expression_statement(close)?)?;
    let peer = cap(model, site, member(&close.callee, "close")?)?;
    if !arguments(close)?.is_empty() || !void_return(ret) {
        return None;
    }
    let report = call(expression_statement(report)?)?;
    let report_port = cap(model, site, &report.callee)?;
    let a = arguments(report)?;
    if a.len() != 2 {
        return None;
    }
    let state = cap(model, site, a[0])?;
    let records = cap(model, site, a[1])?;
    let [extract, locals, Statement::IfStatement(found), alias, Statement::IfStatement(unique), Statement::IfStatement(append), ret] =
        block(outer.alternate.as_ref()?)?
    else {
        return None;
    };
    if !void_return(ret) {
        return None;
    }
    let (matches, value) = local(model, extract)?;
    let first_matcher = exec(model, site, value?, arg)?;
    let Statement::VariableDeclaration(locals) = locals else {
        return None;
    };
    let [text, ip] = locals.declarations.as_slice() else {
        return None;
    };
    let text_id = plain_var(model, text)?;
    let ip_id = plain_var(model, ip)?;
    if text.init.is_some() || ip.init.is_some() {
        return None;
    }
    let Expression::LogicalExpression(and) = &found.test else {
        return None;
    };
    if and.operator.as_str() != "&&" {
        return None;
    }
    let (left, right) = binary(&and.left, "!==")?;
    let (len, one) = binary(&and.right, ">")?;
    if !id(model, left, matches)
        || !null(right)
        || !number(one, 1.0)
        || !member(len, "length").is_some_and(|e| id(model, e, matches))
    {
        return None;
    }
    let [save, extract_text, set_text] = block(&found.consequent)? else {
        return None;
    };
    index(model, assigned(model, save, ip_id)?, matches, 1.0)?;
    let (text_match, value) = local(model, extract_text)?;
    let second_matcher = exec(model, site, value?, arg)?;
    let Expression::ConditionalExpression(select) = assigned(model, set_text, text_id)? else {
        return None;
    };
    let (value, null_value) = binary(&select.test, "!==")?;
    if !id(model, value, text_match) || !null(null_value) || !empty(&select.alternate) {
        return None;
    }
    index(model, &select.consequent, text_match, 0.0)?;
    if !empty(assigned(model, sole(found.alternate.as_ref()?)?, text_id)?) {
        return None;
    }
    let (flag, value) = local(model, alias)?;
    if !id(model, value?, ip_id) {
        return None;
    }
    let (value, null_value) = binary(&unique.test, "!=")?;
    if !id(model, value, flag) || !null(null_value) {
        return None;
    }
    let Expression::UnaryExpression(not) = assigned(model, sole(&unique.consequent)?, flag)? else {
        return None;
    };
    if not.operator.as_str() != "!" {
        return None;
    }
    let some = call(&not.argument)?;
    if !member(&some.callee, "some").is_some_and(|e| id(model, e, records)) {
        return None;
    }
    let a = arguments(some)?;
    let [Expression::ArrowFunctionExpression(predicate)] = a.as_slice() else {
        return None;
    };
    if predicate.r#async
        || !predicate.expression
        || predicate.params.rest.is_some()
        || predicate.params.items.len() != 1
        || !predicate.body.directives.is_empty()
    {
        return None;
    }
    let row = plain_param(model, &predicate.params.items[0])?;
    let [Statement::ExpressionStatement(predicate_body)] = predicate.body.statements.as_slice()
    else {
        return None;
    };
    let (value, ip) = binary(&predicate_body.expression, "===")?;
    if !id(model, ip, ip_id) || !member(value, "ip").is_some_and(|e| id(model, e, row)) {
        return None;
    }
    let (value, null_value) = binary(
        assigned(model, sole(unique.alternate.as_ref()?)?, flag)?,
        "!=",
    )?;
    if !id(model, value, flag)
        || !null(null_value)
        || !id(model, &append.test, flag)
        || append.alternate.is_some()
    {
        return None;
    }
    let [record, push] = block(&append.consequent)? else {
        return None;
    };
    let Expression::ObjectExpression(record) = assigned(model, record, text_id)? else {
        return None;
    };
    if record.properties.len() != 2 {
        return None;
    }
    for (property, key, value) in [
        (&record.properties[0], "ip", ip_id),
        (&record.properties[1], "candidate", text_id),
    ] {
        let ObjectPropertyKind::ObjectProperty(p) = property else {
            return None;
        };
        if p.computed
            || p.method
            || p.shorthand
            || p.kind != PropertyKind::Init
            || p.key.static_name().as_deref() != Some(key)
            || !id(model, &p.value, value)
        {
            return None;
        }
    }
    let push = call(expression_statement(push)?)?;
    if !member(&push.callee, "push").is_some_and(|e| id(model, e, records))
        || !matches!(arguments(push)?.as_slice(),[e] if id(model,e,text_id))
    {
        return None;
    }
    let locals = [arg, matches, text_id, ip_id, text_match, flag, row];
    if locals.into_iter().collect::<BTreeSet<_>>().len() != locals.len()
        || [
            peer,
            report_port,
            state,
            records,
            first_matcher,
            second_matcher,
        ]
        .iter()
        .any(|x| locals.contains(x))
    {
        return None;
    }
    for (s, w) in [
        (matches, 0),
        (text_id, 3),
        (ip_id, 1),
        (text_match, 0),
        (flag, 2),
    ] {
        if model.binding_writes(s) != w {
            return None;
        }
    }
    Some(())
}
pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
) -> Vec<Proposal> {
    let AstKind::Function(fun) = model.nodes.kind(site.node_id) else {
        return vec![];
    };
    if prove(model, site, fun).is_none() {
        return vec![];
    }
    vec![proposal("collectDistinctCandidateIpMatches","owned_candidate_ip_matches","whole formal candidate-null close/report return or two captured exec matches, first capture IP and full text, null-safe some(ip === capturedIP) deduplication, exact ip/candidate object append and void return; native RTC, matcher implementation/patterns and report-port purpose explicitly unproved",f)]
}
