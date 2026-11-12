//! Standalone string-table selection. The legacy serializer finder requires an
//! encoding-tag table before considering this otherwise self-contained helper.
//! Prove the entire owned chain without that unrelated package precondition.
use crate::naming::support::reassembly::*;
use super::*;
use oxc_semantic::SymbolId;

fn binary<'a>(e: &'a Expression<'a>, op: &str) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::BinaryExpression(b) = e.without_parentheses() else {
        return None;
    };
    (b.operator.as_str() == op).then_some((&b.left, &b.right))
}
fn num(e: &Expression<'_>, n: f64) -> bool {
    number(e.without_parentheses(), n)
}
fn sym(model: &Model<'_, '_>, e: &Expression<'_>, s: SymbolId) -> bool {
    symbol(model, e.without_parentheses()) == Some(s)
}
fn length(model: &Model<'_, '_>, e: &Expression<'_>, s: SymbolId) -> bool {
    member_parts(e.without_parentheses()).is_some_and(|(o, k)| k == "length" && sym(model, o, s))
}
fn count(model: &Model<'_, '_>, e: &Expression<'_>, table: SymbolId, key: SymbolId) -> bool {
    matches!(e.without_parentheses(), Expression::ComputedMemberExpression(m)
        if !m.optional && sym(model,&m.object,table) && sym(model,&m.expression,key))
}
fn at(model: &Model<'_, '_>, e: &Expression<'_>, s: SymbolId, n: f64) -> bool {
    matches!(e.without_parentheses(), Expression::ComputedMemberExpression(m)
        if !m.optional && sym(model,&m.object,s) && num(&m.expression,n))
}
fn returned<'a>(statements: &'a [Statement<'a>], expression: bool) -> Option<&'a Expression<'a>> {
    match statements {
        [Statement::ReturnStatement(r)] => r.argument.as_ref(),
        [Statement::ExpressionStatement(e)] if expression => Some(&e.expression),
        _ => None,
    }
}
fn block_return<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    returned(&b.body, false)
}
fn callback<'a>(e: &'a Argument<'a>, arity: usize) -> Option<&'a ArrowFunctionExpression<'a>> {
    let Argument::ArrowFunctionExpression(a) = e else {
        return None;
    };
    (!a.r#async
        && a.params.rest.is_none()
        && a.params.items.len() == arity
        && a.params.items.iter().all(|p| p.initializer.is_none())
        && a.body.directives.is_empty())
    .then_some(a)
}
fn step<'a>(e: &'a Expression<'a>, name: &str) -> Option<(&'a Expression<'a>, &'a [Argument<'a>])> {
    let c = call(e.without_parentheses())?;
    let (receiver, k) = member_parts(c.callee.without_parentheses())?;
    (k == name).then_some((receiver, c.arguments.as_slice()))
}
fn prove(model: &Model<'_, '_>, function: &Function<'_>, body: &FunctionBody<'_>) -> Option<()> {
    let [parameter] = function.params.items.as_slice() else {
        return None;
    };
    if parameter.initializer.is_some() || function.params.rest.is_some() {
        return None;
    }
    let input = immutable(model, &parameter.pattern)?;
    let [alias_stmt, Statement::ReturnStatement(result)] = body.statements.as_slice() else {
        return None;
    };
    let v = one_variable(alias_stmt)?;
    let table = immutable(model, &v.id)?;
    if !sym(model, v.init.as_ref()?, input) {
        return None;
    }
    let (slice, take_args) = step(result.argument.as_ref()?, "map")?;
    let [take_arg] = take_args else {
        return None;
    };
    let take = callback(take_arg, 1)?;
    let pair = immutable(model, &take.params.items[0].pattern)?;
    if !at(
        model,
        returned(&take.body.statements, take.expression)?,
        pair,
        0.0,
    ) {
        return None;
    }
    let (sort, slice_args) = step(slice, "slice")?;
    let [start, end] = slice_args else {
        return None;
    };
    if !num(start.as_expression()?, 0.0) || !num(end.as_expression()?, 255.0) {
        return None;
    }
    let (map, sort_args) = step(sort, "sort")?;
    let [sort_arg] = sort_args else {
        return None;
    };
    let sorter = callback(sort_arg, 2)?;
    let first = immutable(model, &sorter.params.items[0].pattern)?;
    let second = immutable(model, &sorter.params.items[1].pattern)?;
    let (left, right) = binary(returned(&sorter.body.statements, sorter.expression)?, "-")?;
    if !at(model, left, second, 1.0) || !at(model, right, first, 1.0) {
        return None;
    }
    let (filter, map_args) = step(map, "map")?;
    let [score_arg] = map_args else {
        return None;
    };
    let score = callback(score_arg, 1)?;
    let key = immutable(model, &score.params.items[0].pattern)?;
    let Expression::ArrayExpression(pair) =
        returned(&score.body.statements, score.expression)?.without_parentheses()
    else {
        return None;
    };
    let [key_value, saving] = pair.elements.as_slice() else {
        return None;
    };
    if !sym(model, key_value.as_expression()?, key) {
        return None;
    }
    let (gross, cost) = binary(saving.as_expression()?, "-")?;
    let (width, times) = binary(gross, "*")?;
    let (len, one) = binary(width, "+")?;
    if !length(model, len, key) || !num(one, 1.0) || !count(model, times, table, key) {
        return None;
    }
    let (header, indices) = binary(cost, "+")?;
    let (len, one) = binary(header, "+")?;
    let (times, two) = binary(indices, "*")?;
    if !length(model, len, key)
        || !num(one, 1.0)
        || !count(model, times, table, key)
        || !num(two, 2.0)
    {
        return None;
    }
    let (keys, filter_args) = step(filter, "filter")?;
    let [keep_arg] = filter_args else {
        return None;
    };
    let keep = callback(keep_arg, 1)?;
    let key = immutable(model, &keep.params.items[0].pattern)?;
    let [Statement::IfStatement(guard)] = keep.body.statements.as_slice() else {
        return None;
    };
    if keep.expression {
        return None;
    }
    let (times, two) = binary(&guard.test, ">=")?;
    if !count(model, times, table, key) || !num(two, 2.0) {
        return None;
    }
    let (product, eight) = binary(block_return(&guard.consequent)?, ">=")?;
    let (len, times) = binary(product, "*")?;
    if !length(model, len, key)
        || !count(model, times, table, key)
        || !num(eight, 8.0)
        || !matches!(block_return(guard.alternate.as_ref()?)?.without_parentheses(), Expression::BooleanLiteral(b) if !b.value)
    {
        return None;
    }
    let keys = call(keys.without_parentheses())?;
    if expression_path(model, &keys.callee) != ["Object", "keys"] {
        return None;
    }
    let [arg] = keys.arguments.as_slice() else {
        return None;
    };
    let arg = arg.as_expression()?;
    if !sym(model, arg, input) && !sym(model, arg, table) {
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
    if function.r#async
        || function.generator
        || !body.directives.is_empty()
        || prove(model, function, body).is_none()
    {
        return Vec::new();
    }
    vec![proposal("selectStringTable", "signals_string_table_selection",
        "complete owned Object.keys chain: count>=2 and length*count>=8 filter; exact byte-saving pair formula; descending saving sort; slice(0,255); key projection. All formals and the sole local alias are immutable scoped symbols; no external captures or serializer tag-table evidence",f)]
}
