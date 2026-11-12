//! Two-formal query-value reader. Shares only the exact owned anchor/split/
//! decode contract with the merger; no captured parser or sibling donates proof.
use super::url_query_merge::{
    array_items, assignment_statement, binary, block, body_local, boolean, call,
    entries_empty_assignment, input_guard, is_value, member, owned_by, plain_binding,
    plain_formals, query_parse, symbol, target_symbol, text, ValueRef,
};
use super::*;
use oxc_semantic::SymbolId;
type Symbol = SymbolId;

fn stable(model: &Model<'_, '_>, value: Symbol) -> bool {
    model.binding_writes(value) == 0 && model.scoping.symbol_redeclarations(value).is_empty()
}

fn local(model: &Model<'_, '_>, site: FunctionSite, statement: &Statement<'_>) -> Option<Symbol> {
    let value = plain_binding(model, one_variable(statement)?)?;
    body_local(model, site, value).then_some(value)
}

fn null_test(model: &Model<'_, '_>, expression: &Expression<'_>, value: Symbol) -> bool {
    binary(expression).is_some_and(|b| {
        b.operator.as_str() == "=="
            && symbol(model, &b.left) == Some(value)
            && matches!(b.right.without_parentheses(), Expression::NullLiteral(_))
    })
}

fn trimmed(model: &Model<'_, '_>, expression: &Expression<'_>, input: Symbol) -> bool {
    call(expression).is_some_and(|c| {
        c.arguments.is_empty()
            && member(&c.callee, "trim").and_then(|o| symbol(model, o)) == Some(input)
    })
}

fn blank(model: &Model<'_, '_>, expression: &Expression<'_>, input: Symbol) -> bool {
    binary(expression).is_some_and(|b| {
        b.operator.as_str() == "===" && trimmed(model, &b.left, input) && text(&b.right, "")
    })
}

fn returns_empty(statement: &Statement<'_>) -> bool {
    matches!(statement, Statement::ReturnStatement(r) if r.argument.as_ref()
        .and_then(array_items).is_some_and(|a| a.is_empty()))
}

/// Expression arrows are direct arguments to the exact filter/map calls below.
/// Reject block callbacks and all additional formals, effects or captured donors.
fn projection<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    expression: &'a Expression<'a>,
) -> Option<(Symbol, &'a Expression<'a>)> {
    let Expression::ArrowFunctionExpression(arrow) = expression.without_parentheses() else {
        return None;
    };
    if arrow.r#async || !arrow.expression || !arrow.body.directives.is_empty() {
        return None;
    }
    let child = model.site_by_span(arrow.span)?;
    if !owned_by(model, child, site) {
        return None;
    }
    let params = plain_formals(model, &arrow.params)?;
    let [parameter] = params.as_slice() else {
        return None;
    };
    let [Statement::ExpressionStatement(value)] = arrow.body.statements.as_slice() else {
        return None;
    };
    Some((*parameter, &value.expression))
}

fn filtered_values(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    entries: Symbol,
    name: Symbol,
) -> Option<()> {
    let Statement::IfStatement(branch) = statement else {
        return None;
    };
    if symbol(model, member(&branch.test, "length")?) != Some(entries) {
        return None;
    }
    let [Statement::ReturnStatement(ret)] = block(&branch.consequent)? else {
        return None;
    };
    let [empty] = block(branch.alternate.as_ref()?)? else {
        return None;
    };
    if !returns_empty(empty) {
        return None;
    }
    let map = call(ret.argument.as_ref()?)?;
    let filter = call(member(&map.callee, "map")?)?;
    if symbol(model, member(&filter.callee, "filter")?) != Some(entries)
        || map.arguments.len() != 1
        || filter.arguments.len() != 1
    {
        return None;
    }
    let map_args = call_arguments(map);
    let filter_args = call_arguments(filter);
    let (pair, value) = projection(model, site, *map_args.first()?)?;
    if !is_value(
        model,
        value,
        ValueRef::Index {
            owner: pair,
            index: 1,
        },
    ) {
        return None;
    }
    let (pair, condition) = projection(model, site, *filter_args.first()?)?;
    let equal = binary(condition)?;
    (equal.operator.as_str() == "==="
        && is_value(
            model,
            &equal.left,
            ValueRef::Index {
                owner: pair,
                index: 0,
            },
        )
        && symbol(model, &equal.right) == Some(name))
    .then_some(())
}

fn reader(model: &Model<'_, '_>, site: FunctionSite, function: &Function<'_>) -> Option<()> {
    if function.r#async || function.generator {
        return None;
    }
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let formals = plain_formals(model, &function.params)?;
    let [input, name] = formals.as_slice() else {
        return None;
    };
    if input == name {
        return None;
    }
    let statements = body.statements.as_slice();
    let alias_statement = statements.first()?;
    let alias = local(model, site, alias_statement)?;
    if !stable(model, alias)
        || symbol(model, one_variable(alias_statement)?.init.as_ref()?) != Some(*name)
    {
        return None;
    }
    let (cursor, lowered) = match statements.get(1)? {
        Statement::VariableDeclaration(_) => {
            let flag = local(model, site, &statements[1])?;
            if one_variable(&statements[1])?.init.is_some() || model.binding_writes(flag) != 2 {
                return None;
            }
            let Statement::IfStatement(branch) = statements.get(2)? else {
                return None;
            };
            if !null_test(model, &branch.test, *input) {
                return None;
            }
            let [yes] = block(&branch.consequent)? else {
                return None;
            };
            let [no] = block(branch.alternate.as_ref()?)? else {
                return None;
            };
            let yes = assignment_statement(yes)?;
            let no = assignment_statement(no)?;
            if yes.operator.as_str() != "="
                || no.operator.as_str() != "="
                || target_symbol(model, yes) != Some(flag)
                || target_symbol(model, no) != Some(flag)
                || !boolean(&yes.right, true)
                || !blank(model, &no.right, *input)
            {
                return None;
            }
            (3, Some(flag))
        }
        _ => (1, None),
    };
    let [Statement::IfStatement(guard), trimmed_statement, entries_statement, Statement::IfStatement(parse), result] =
        &statements[cursor..]
    else {
        return None;
    };
    let Expression::LogicalExpression(test) = guard.test.without_parentheses() else {
        return None;
    };
    if test.operator.as_str() != "||" || !null_test(model, &test.right, alias) {
        return None;
    }
    if let Some(flag) = lowered {
        if symbol(model, &test.left) != Some(flag) {
            return None;
        }
    } else if !input_guard(model, &test.left, *input) {
        return None;
    }
    let [empty] = block(&guard.consequent)? else {
        return None;
    };
    if guard.alternate.is_some() || !returns_empty(empty) {
        return None;
    }
    let normalized = local(model, site, trimmed_statement)?;
    if !stable(model, normalized)
        || !trimmed(
            model,
            one_variable(trimmed_statement)?.init.as_ref()?,
            *input,
        )
    {
        return None;
    }
    let entries = local(model, site, entries_statement)?;
    if one_variable(entries_statement)?.init.is_some()
        || model.binding_writes(entries) != 3
        || !input_guard(model, &parse.test, normalized)
    {
        return None;
    }
    let [empty] = block(&parse.consequent)? else {
        return None;
    };
    if !entries_empty_assignment(model, empty, entries) {
        return None;
    }
    query_parse(
        model,
        site,
        normalized,
        entries,
        block(parse.alternate.as_ref()?)?,
    )?;
    filtered_values(model, site, result, entries, alias)
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    facts: &Facts<'_>,
) -> Vec<Proposal> {
    if reader(model, site, function).is_none() {
        return vec![];
    }
    vec![proposal("readUrlSearchParameterValues", "thread_url_query_reader",
        "complete two-formal owned query reader: exact null/blank guards, same trimmed URL anchor href/search/split, directly owned component decode callback, same-entry key equality filter and value projection; immutable input/name aliases, no captured donors or additional effects", facts)]
}
