//! Exact owned-body lexical variants. Binding identity, never spelling, connects
//! each formal/index/receiver. Captured data must be a plain outer declarator.
//! No nested callable may donate evidence. These are final fallback proposals.
use super::*;
use oxc_semantic::SymbolId;

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e.without_parentheses(), model.scoping)
}

fn plain_binding(model: &Model<'_, '_>, p: &BindingPattern<'_>) -> Option<SymbolId> {
    let BindingPattern::BindingIdentifier(id) = p else {
        return None;
    };
    let s = id.symbol_id.get()?;
    model
        .scoping
        .symbol_redeclarations(s)
        .is_empty()
        .then_some(s)
}

fn plain_param(model: &Model<'_, '_>, p: &FormalParameter<'_>) -> Option<SymbolId> {
    if p.initializer.is_some() {
        return None;
    }
    let s = plain_binding(model, &p.pattern)?;
    (model.binding_writes(s) == 0).then_some(s)
}

fn plain_var(model: &Model<'_, '_>, v: &VariableDeclarator<'_>) -> Option<SymbolId> {
    plain_binding(model, &v.id)
}

// A captured data binding is a plain var/let/const declarator outside the
// owned body. Parameters, catch bindings, destructuring and for-in/of origins
// never qualify; mutation of the data itself is allowed and proven by use.
fn captured_symbol(model: &Model<'_, '_>, site: FunctionSite, s: SymbolId) -> Option<SymbolId> {
    let declaration = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else {
        return None;
    };
    if site.span.contains_inclusive(model.scoping.symbol_span(s))
        || !model.scoping.symbol_redeclarations(s).is_empty()
        || !matches!(&variable.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(d)
                if matches!(
                    d.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                )
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

fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = e.without_parentheses() else {
        return None;
    };
    (!m.optional && m.property.name == key).then_some(&m.object)
}

fn indexed(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
    index: SymbolId,
) -> Option<SymbolId> {
    let Expression::ComputedMemberExpression(m) = e.without_parentheses() else {
        return None;
    };
    if m.optional || symbol(model, &m.expression) != Some(index) {
        return None;
    }
    captured_symbol(model, site, symbol(model, &m.object)?)
}

fn returned<'a>(body: &'a FunctionBody<'a>) -> Option<&'a Expression<'a>> {
    let [Statement::ReturnStatement(r)] = body.statements.as_slice() else {
        return None;
    };
    r.argument.as_ref().map(Expression::without_parentheses)
}

fn boolean_role(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
    input: SymbolId,
) -> Option<&'static str> {
    let Expression::BinaryExpression(test) = returned(body)? else {
        return None;
    };
    if test.operator.as_str() != "===" {
        return None;
    }
    let (value, kind) = if matches!(
        test.left.without_parentheses(),
        Expression::UnaryExpression(_)
    ) {
        (&test.left, &test.right)
    } else {
        (&test.right, &test.left)
    };
    if !matches!(kind.without_parentheses(), Expression::StringLiteral(s) if s.value == "boolean") {
        return None;
    }
    let Expression::UnaryExpression(value) = value.without_parentheses() else {
        return None;
    };
    if value.operator.as_str() != "typeof" {
        return None;
    }
    if symbol(model, &value.argument) == Some(input) {
        return Some("isBooleanElement");
    }
    indexed(model, site, &value.argument, input)?;
    Some("isBooleanEntryValue")
}

fn encoder(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
    input: SymbolId,
) -> Option<()> {
    let Expression::CallExpression(c) = returned(body)? else {
        return None;
    };
    if c.optional {
        return None;
    }
    let receiver = captured_symbol(
        model,
        site,
        symbol(model, member(&c.callee, "encodeValue")?)?,
    )?;
    let [value, context] = c.arguments.as_slice() else {
        return None;
    };
    let collection = indexed(model, site, value.as_expression()?, input)?;
    let context = captured_symbol(model, site, symbol(model, context.as_expression()?)?)?;
    (receiver != collection && receiver != context && context != collection).then_some(())
}

fn call<'a>(
    e: &'a Expression<'a>,
    method: &str,
) -> Option<(&'a Expression<'a>, &'a CallExpression<'a>)> {
    let Expression::CallExpression(c) = e.without_parentheses() else {
        return None;
    };
    if c.optional {
        return None;
    }
    Some((member(&c.callee, method)?, c))
}

fn expression<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    Some(&s.expression)
}

fn assignment<'a>(
    s: &'a Statement<'a>,
    model: &Model<'_, '_>,
    target: SymbolId,
) -> Option<&'a Expression<'a>> {
    let Expression::AssignmentExpression(a) = expression(s)? else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
        return None;
    };
    (a.operator.as_str() == "="
        && crate::analysis::identifier_symbol(id, model.scoping) == Some(target))
    .then_some(&a.right)
}

fn selector_alias(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
    input: SymbolId,
) -> Option<()> {
    let [Statement::VariableDeclaration(local), Statement::TryStatement(t)] =
        body.statements.as_slice()
    else {
        return None;
    };
    let [local] = local.declarations.as_slice() else {
        return None;
    };
    if local.init.is_some() {
        return None;
    }
    let target = plain_var(model, local)?;
    if model.binding_writes(target) != 2 || t.finalizer.is_some() {
        return None;
    }
    let [load, Statement::IfStatement(outer), reset] = t.block.body.as_slice() else {
        return None;
    };
    if symbol(model, member(assignment(load, model, target)?, "target")?) != Some(input)
        || !matches!(
            assignment(reset, model, target)?,
            Expression::NumericLiteral(_)
        )
        || symbol(model, &outer.test) != Some(target)
        || outer.alternate.is_some()
    {
        return None;
    }
    let Statement::BlockStatement(outer_body) = &outer.consequent else {
        return None;
    };
    let [Statement::IfStatement(inner)] = outer_body.body.as_slice() else {
        return None;
    };
    if inner.alternate.is_some() {
        return None;
    }
    let Expression::BinaryExpression(null_test) = &inner.test else {
        return None;
    };
    if null_test.operator.as_str() != "=="
        || !matches!(&null_test.right, Expression::NullLiteral(_))
    {
        return None;
    }
    let (receiver, closest) = call(&null_test.left, "closest")?;
    if symbol(model, receiver) != Some(target) {
        return None;
    }
    let [selector] = closest.arguments.as_slice() else {
        return None;
    };
    let (selectors, join) = call(selector.as_expression()?, "join")?;
    let selectors = captured_symbol(model, site, symbol(model, selectors)?)?;
    if !matches!(join.arguments.as_slice(), [Argument::StringLiteral(s)] if s.value == ",") {
        return None;
    }
    let Statement::BlockStatement(increment) = &inner.consequent else {
        return None;
    };
    let [increment] = increment.body.as_slice() else {
        return None;
    };
    let Expression::UpdateExpression(increment) = expression(increment)? else {
        return None;
    };
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(counter) = &increment.argument else {
        return None;
    };
    let counter = captured_symbol(
        model,
        site,
        crate::analysis::identifier_symbol(counter, model.scoping)?,
    )?;
    if increment.operator.as_str() != "++" || counter == selectors {
        return None;
    }
    let catch = t.handler.as_ref()?;
    let error = plain_binding(model, &catch.param.as_ref()?.pattern)?;
    if model.binding_writes(error) != 0 {
        return None;
    }
    let [failed] = catch.body.body.as_slice() else {
        return None;
    };
    let (receiver, failed) = call(expression(failed)?, "error")?;
    let receiver = captured_symbol(model, site, symbol(model, receiver)?)?;
    if receiver == counter || receiver == selectors {
        return None;
    }
    let [argument] = failed.arguments.as_slice() else {
        return None;
    };
    (symbol(model, argument.as_expression()?) == Some(error)).then_some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
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
        || !body.directives.is_empty()
        || function.params.rest.is_some()
        || function.params.items.len() != 1
    {
        return vec![];
    }
    let Some(input) = plain_param(model, &function.params.items[0]) else {
        return vec![];
    };
    let role = boolean_role(model, site, body, input)
        .or_else(|| encoder(model, site, body, input).map(|_| "encodeEntryValue"))
        .or_else(|| selector_alias(model, site, body, input).map(|_| "countEventsOutsideSelector"));
    role.map(|role| vec![proposal(role, "runtime_owned_lexical_variants", "exact owned statement grammar connects the unwritten formal to its returned boolean/indexed encoding, or a guarded event target alias to selector lookup, counter increment and exact caught-error forwarding; captures resolve to plain outer declarators", facts)]).unwrap_or_default()
}
