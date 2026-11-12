use super::*;
use oxc_semantic::SymbolId;

pub(in crate::naming) fn symbol(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e, model.scoping)
}

pub(in crate::naming) fn plain_binding(
    model: &Model<'_, '_>,
    p: &BindingPattern<'_>,
) -> Option<SymbolId> {
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

pub(in crate::naming) fn plain_param(
    model: &Model<'_, '_>,
    p: &FormalParameter<'_>,
) -> Option<SymbolId> {
    if p.initializer.is_some() {
        return None;
    }
    let s = plain_binding(model, &p.pattern)?;
    (model.binding_writes(s) == 0).then_some(s)
}

pub(in crate::naming) fn plain_var(
    model: &Model<'_, '_>,
    v: &VariableDeclarator<'_>,
) -> Option<SymbolId> {
    plain_binding(model, &v.id)
}

// A captured data binding is a plain var/let/const declarator outside the
// owned body. Parameters, catch bindings, destructuring and for-in/of origins
// never qualify; mutation of the data itself is allowed and proven by use.
pub(in crate::naming) fn captured_symbol(
    model: &Model<'_, '_>,
    site: FunctionSite,
    s: SymbolId,
) -> Option<SymbolId> {
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

pub(in crate::naming) fn member<'a>(
    e: &'a Expression<'a>,
    key: &str,
) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = e else {
        return None;
    };
    (!m.optional && m.property.name == key).then_some(&m.object)
}

pub(in crate::naming) fn call<'a>(
    e: &'a Expression<'a>,
) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(c) = e else {
        return None;
    };
    (!c.optional).then_some(c)
}

pub(in crate::naming) fn method_receiver(
    model: &Model<'_, '_>,
    c: &CallExpression<'_>,
    method: &str,
) -> Option<SymbolId> {
    if c.optional {
        return None;
    }
    let Expression::StaticMemberExpression(m) = &c.callee else {
        return None;
    };
    (!m.optional && m.property.name == method).then(|| symbol(model, &m.object))?
}

pub(in crate::naming) fn number(e: &Expression<'_>, value: f64) -> bool {
    matches!(e, Expression::NumericLiteral(n) if n.value == value)
}

pub(in crate::naming) fn boolean(e: &Expression<'_>, value: bool) -> bool {
    matches!(e, Expression::BooleanLiteral(b) if b.value == value)
}

pub(in crate::naming) fn binary<'a>(
    e: &'a Expression<'a>,
    op: &str,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::BinaryExpression(b) = e else {
        return None;
    };
    (b.operator.as_str() == op).then_some((&b.left, &b.right))
}

pub(in crate::naming) fn expression_statement<'a>(
    s: &'a Statement<'a>,
) -> Option<&'a Expression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    Some(&s.expression)
}

pub(in crate::naming) fn single_variable<'a>(
    d: &'a VariableDeclaration<'a>,
) -> Option<&'a VariableDeclarator<'a>> {
    // The owned-body proofs promise plain var/let/const locals; resource
    // declarations (using / await using) never qualify.
    if !matches!(
        d.kind,
        VariableDeclarationKind::Var
            | VariableDeclarationKind::Let
            | VariableDeclarationKind::Const
    ) {
        return None;
    }
    let [v] = d.declarations.as_slice() else {
        return None;
    };
    Some(v)
}

pub(in crate::naming) fn variable<'a>(
    s: &'a Statement<'a>,
) -> Option<&'a VariableDeclarator<'a>> {
    let Statement::VariableDeclaration(d) = s else {
        return None;
    };
    single_variable(d)
}

pub(in crate::naming) fn assignment<'a>(
    e: &'a Expression<'a>,
) -> Option<&'a AssignmentExpression<'a>> {
    let Expression::AssignmentExpression(a) = e else {
        return None;
    };
    (a.operator.as_str() == "=").then_some(a)
}

pub(in crate::naming) fn target_symbol(
    model: &Model<'_, '_>,
    target: &AssignmentTarget<'_>,
) -> Option<SymbolId> {
    let AssignmentTarget::AssignmentTargetIdentifier(id) = target else {
        return None;
    };
    crate::analysis::identifier_symbol(id, model.scoping)
}

pub(in crate::naming) fn arguments<'a>(
    c: &'a CallExpression<'a>,
) -> Option<Vec<&'a Expression<'a>>> {
    let args = call_arguments(c);
    (args.len() == c.arguments.len()).then_some(args)
}
