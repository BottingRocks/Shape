use super::*;

pub(in crate::naming) type Symbol = oxc_semantic::SymbolId;

pub(in crate::naming) fn symbol(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
) -> Option<Symbol> {
    crate::analysis::expression_symbol(e, model.scoping)
}

pub(in crate::naming) fn member<'a>(
    e: &'a Expression<'a>,
    key: &str,
) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = e.without_parentheses() else {
        return None;
    };
    (!m.optional && m.property.name == key).then_some(&m.object)
}

pub(in crate::naming) fn indexed<'a>(
    e: &'a Expression<'a>,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::ComputedMemberExpression(m) = e.without_parentheses() else {
        return None;
    };
    (!m.optional).then_some((&m.object, &m.expression))
}

pub(in crate::naming) fn call<'a>(
    e: &'a Expression<'a>,
) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(c) = e.without_parentheses() else {
        return None;
    };
    (!c.optional).then_some(c)
}

pub(in crate::naming) fn returned<'a>(
    s: &'a Statement<'a>,
) -> Option<&'a Expression<'a>> {
    let Statement::ReturnStatement(r) = s else {
        return None;
    };
    r.argument.as_ref()
}

pub(in crate::naming) fn expression_statement<'a>(
    s: &'a Statement<'a>,
) -> Option<&'a Expression<'a>> {
    let Statement::ExpressionStatement(e) = s else {
        return None;
    };
    Some(&e.expression)
}

pub(in crate::naming) fn block<'a>(
    s: &'a Statement<'a>,
) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(b.body.as_slice())
}

pub(in crate::naming) fn number(e: &Expression<'_>, n: f64) -> bool {
    matches!(e.without_parentheses(), Expression::NumericLiteral(v) if v.value == n)
}

pub(in crate::naming) fn boolean(e: &Expression<'_>, value: bool) -> bool {
    matches!(e.without_parentheses(), Expression::BooleanLiteral(v) if v.value == value)
}

pub(in crate::naming) fn string(e: &Expression<'_>, value: &str) -> bool {
    matches!(e.without_parentheses(), Expression::StringLiteral(v) if v.value == value)
}

pub(in crate::naming) fn void_zero(e: &Expression<'_>) -> bool {
    matches!(e.without_parentheses(), Expression::UnaryExpression(u)
        if u.operator.as_str() == "void" && number(&u.argument, 0.0))
}

pub(in crate::naming) fn null_literal(e: &Expression<'_>) -> bool {
    matches!(e.without_parentheses(), Expression::NullLiteral(_))
}

pub(in crate::naming) fn binary<'a>(
    e: &'a Expression<'a>,
    op: &str,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::BinaryExpression(b) = e.without_parentheses() else {
        return None;
    };
    (b.operator.as_str() == op).then_some((&b.left, &b.right))
}

pub(in crate::naming) fn logical<'a>(
    e: &'a Expression<'a>,
    op: &str,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::LogicalExpression(l) = e.without_parentheses() else {
        return None;
    };
    (l.operator.as_str() == op).then_some((&l.left, &l.right))
}

pub(in crate::naming) fn empty_array(e: &Expression<'_>) -> bool {
    matches!(e.without_parentheses(), Expression::ArrayExpression(a) if a.elements.is_empty())
}

/// A plain `var`/`let`/`const` declarator in a normal statement container. It is
/// never a for-in/of target, destructuring pattern or redeclaration.
pub(in crate::naming) fn plain_binding(
    model: &Model<'_, '_>,
    v: &VariableDeclarator<'_>,
    writes: Option<usize>,
) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(id) = &v.id else {
        return None;
    };
    let s = id.symbol_id.get()?;
    let declaration = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(declared) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&declared.id, BindingPattern::BindingIdentifier(decl)
        if decl.symbol_id.get() == Some(s))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(d)
                if matches!(d.kind, VariableDeclarationKind::Var
                    | VariableDeclarationKind::Let
                    | VariableDeclarationKind::Const)
        )
        || matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
        || !model.scoping.symbol_redeclarations(s).is_empty()
    {
        return None;
    }
    if writes.is_some_and(|expected| model.binding_writes(s) != expected) {
        return None;
    }
    Some(s)
}

/// A captured data receiver/slot. Parameters and other non-declarator origins
/// are legitimate data values, but the binding must still be a single
/// unredeclared scoped symbol declared before the function.
pub(in crate::naming) fn capture_symbol(
    model: &Model<'_, '_>,
    site: FunctionSite,
    s: Symbol,
) -> Option<Symbol> {
    if site.span.contains_inclusive(model.scoping.symbol_span(s))
        || !model.scoping.symbol_redeclarations(s).is_empty()
        || model.scoping.symbol_span(s).end >= site.span.start
    {
        return None;
    }
    Some(s)
}

pub(in crate::naming) fn capture(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
) -> Option<Symbol> {
    capture_symbol(model, site, symbol(model, e)?)
}

pub(in crate::naming) fn plain_formals(
    model: &Model<'_, '_>,
    params: &FormalParameters<'_>,
) -> Option<Vec<Symbol>> {
    if params.rest.is_some() {
        return None;
    }
    params
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

pub(in crate::naming) fn plain_params(
    model: &Model<'_, '_>,
    function: &Function<'_>,
) -> Option<Vec<Symbol>> {
    plain_formals(model, &function.params)
}

pub(in crate::naming) fn object_entries<'a>(
    object: &'a ObjectExpression<'a>,
) -> Option<Vec<(String, &'a Expression<'a>)>> {
    let mut keys = BTreeSet::new();
    let mut entries = Vec::with_capacity(object.properties.len());
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(p) = property else {
            return None;
        };
        if p.computed || p.method || p.kind != PropertyKind::Init {
            return None;
        }
        let key = property_key_name(&p.key)?;
        if !keys.insert(key.clone()) {
            return None;
        }
        entries.push((key, &p.value));
    }
    Some(entries)
}

pub(in crate::naming) fn assignment<'a>(
    s: &'a Statement<'a>,
) -> Option<&'a AssignmentExpression<'a>> {
    let Statement::ExpressionStatement(e) = s else {
        return None;
    };
    assignment_in(e)
}

pub(in crate::naming) fn assignment_in<'a>(
    e: &'a ExpressionStatement<'a>,
) -> Option<&'a AssignmentExpression<'a>> {
    let Expression::AssignmentExpression(a) = e.expression.without_parentheses() else {
        return None;
    };
    (a.operator.as_str() == "=").then_some(a)
}

pub(in crate::naming) fn assigned_symbol(
    model: &Model<'_, '_>,
    target: &AssignmentTarget<'_>,
) -> Option<Symbol> {
    let AssignmentTarget::AssignmentTargetIdentifier(id) = target else {
        return None;
    };
    crate::analysis::identifier_symbol(id, model.scoping)
}

pub(in crate::naming) fn state_or_zero(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    state: Symbol,
) -> bool {
    symbol(model, e) == Some(state)
        || binary(e, "|")
            .is_some_and(|(left, right)| symbol(model, left) == Some(state) && number(right, 0.0))
}

pub(in crate::naming) fn update_target(
    model: &Model<'_, '_>,
    s: &Statement<'_>,
    target: Symbol,
    op: &str,
) -> bool {
    matches!(s, Statement::ExpressionStatement(e)
        if matches!(&e.expression,
            Expression::UpdateExpression(u)
                if u.operator.as_str() == op
                    && matches!(&u.argument,
                        SimpleAssignmentTarget::AssignmentTargetIdentifier(id)
                            if crate::analysis::identifier_symbol(id, model.scoping) == Some(target))))
}

pub(in crate::naming) fn random_expression(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
) -> bool {
    if let Some((left, right)) = binary(e, "|") {
        return number(right, 0.0) && random_expression(model, left);
    }
    let Some((left, right)) = binary(e, "*") else {
        return false;
    };
    let Some(rand) = call(left) else {
        return false;
    };
    expression_path(model, &rand.callee) == ["Math", "random"]
        && rand.arguments.is_empty()
        && number(right, 4294967296.0)
}

pub(in crate::naming) fn byte_array(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    state: Symbol,
) -> Option<()> {
    let Expression::ArrayExpression(a) = e.without_parentheses() else {
        return None;
    };
    let [e0, e1, e2, e3] = a.elements.as_slice() else {
        return None;
    };
    let base = |e: &Expression<'_>| state_or_zero(model, e, state);
    let (shift0, by0) = binary(e0.as_expression()?, ">>>")?;
    if !base(shift0) || !number(by0, 24.0) {
        return None;
    }
    let (mask1, low1) = binary(e1.as_expression()?, "&")?;
    let (shift1, by1) = binary(mask1, ">>")?;
    if !base(shift1) || !number(by1, 16.0) || !number(low1, 255.0) {
        return None;
    }
    let (mask2, low2) = binary(e2.as_expression()?, "&")?;
    let (shift2, by2) = binary(mask2, ">>")?;
    if !base(shift2) || !number(by2, 8.0) || !number(low2, 255.0) {
        return None;
    }
    let (mask3, low3) = binary(e3.as_expression()?, "&")?;
    if !base(mask3) || !number(low3, 255.0) {
        return None;
    }
    Some(())
}

pub(in crate::naming) fn mixer_parts<'a>(
    model: &Model<'_, '_>,
    e: &'a Expression<'a>,
    state: Symbol,
    bytes: Symbol,
) -> Option<(Symbol, &'a Expression<'a>, &'a Expression<'a>)> {
    let c = call(e)?;
    if c.arguments.len() != 4 {
        return None;
    }
    let callee = symbol(model, &c.callee)?;
    let args = call_arguments(c);
    if symbol(model, args[0]) != Some(state) || symbol(model, args[2]) != Some(bytes) {
        return None;
    }
    Some((callee, args[1], args[3]))
}

pub(in crate::naming) fn forwarded_bytes(
    model: &Model<'_, '_>,
    site: FunctionSite,
    next: &Statement<'_>,
    complete: &Statement<'_>,
    bytes: Symbol,
    first: Option<Symbol>,
) -> Option<()> {
    let Some(next_call) = call(expression_statement(next)?) else {
        return None;
    };
    let Some(complete_call) = call(expression_statement(complete)?) else {
        return None;
    };
    if next_call.arguments.len() != 1 || !complete_call.arguments.is_empty() {
        return None;
    }
    let next_receiver = member(&next_call.callee, "next").and_then(|e| capture(model, site, e))?;
    let complete_receiver =
        member(&complete_call.callee, "complete").and_then(|e| capture(model, site, e))?;
    if next_receiver != complete_receiver {
        return None;
    }
    let Expression::ArrayExpression(pair) = call_arguments(next_call)[0].without_parentheses()
    else {
        return None;
    };
    let [first_arg, second_arg] = pair.elements.as_slice() else {
        return None;
    };
    if symbol(model, second_arg.as_expression()?) != Some(bytes) {
        return None;
    }
    match first {
        None => empty_array(first_arg.as_expression()?).then_some(()),
        Some(local) => (symbol(model, first_arg.as_expression()?) == Some(local)).then_some(()),
    }
}
