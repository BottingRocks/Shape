use super::*;
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;

pub(in crate::naming) type Symbol = SymbolId;

pub(in crate::naming) fn symbol(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
) -> Option<Symbol> {
    crate::analysis::expression_symbol(e, model.scoping)
}

pub(in crate::naming) fn unchanged(model: &Model<'_, '_>, s: Symbol) -> bool {
    model.binding_writes(s) == 0 && model.scoping.symbol_redeclarations(s).is_empty()
}

pub(in crate::naming) fn number(e: &Expression<'_>, expected: f64) -> bool {
    matches!(e, Expression::NumericLiteral(n) if n.value == expected)
}

pub(in crate::naming) fn is_symbol(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    s: Symbol,
) -> bool {
    symbol(model, e) == Some(s)
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

pub(in crate::naming) fn call_expr<'a>(
    e: &'a Expression<'a>,
) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(c) = e else {
        return None;
    };
    (!c.optional).then_some(c)
}

pub(in crate::naming) fn statement_call<'a>(
    s: &'a Statement<'a>,
) -> Option<&'a CallExpression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    call_expr(&s.expression)
}

pub(in crate::naming) fn expression_of<'a>(
    s: &'a Statement<'a>,
) -> Option<&'a Expression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    Some(&s.expression)
}

pub(in crate::naming) fn block<'a>(
    s: &'a Statement<'a>,
) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(&b.body)
}

pub(in crate::naming) fn plain_declarator(
    model: &Model<'_, '_>,
    v: &VariableDeclarator<'_>,
) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(id) = &v.id else {
        return None;
    };
    let s = id.symbol_id.get()?;
    (model.scoping.symbol_redeclarations(s).is_empty()
        && matches!(
            model.nodes.parent_kind(model.scoping.symbol_declaration(s)),
            AstKind::VariableDeclaration(d)
                if matches!(d.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const)
        ))
    .then_some(s)
}

pub(in crate::naming) fn captured_data_symbol(
    model: &Model<'_, '_>,
    site: FunctionSite,
    s: Symbol,
) -> bool {
    if site.span.contains_inclusive(model.scoping.symbol_span(s))
        || !model.scoping.symbol_redeclarations(s).is_empty()
    {
        return false;
    }
    let declaration = model.scoping.symbol_declaration(s);
    if !matches!(
        model.nodes.kind(declaration),
        AstKind::VariableDeclarator(_)
    ) || !matches!(
        model.nodes.parent_kind(declaration),
        AstKind::VariableDeclaration(_)
    ) || !matches!(
        model.nodes.parent_kind(declaration),
        AstKind::VariableDeclaration(d)
            if matches!(d.kind,
                VariableDeclarationKind::Var
                    | VariableDeclarationKind::Let
                    | VariableDeclarationKind::Const)
    ) || matches!(
        model.nodes.parent_kind(model.nodes.parent_id(declaration)),
        AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
    ) {
        return false;
    }
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return false;
    };
    matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
}

// A captured DATA slot may be read and written, but it must be a plain
// declarator in an ancestor statement container: parameters, catches,
// destructuring and loop bindings never establish an independent identity.
pub(in crate::naming) fn data_capture(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
) -> Option<Symbol> {
    let s = symbol(model, e)?;
    captured_data_symbol(model, site, s).then_some(s)
}

// The same origin walk as the shared callable resolver, extended with a
// monotone source-order check at every alias hop: a value may only be used
// after its initializer/sole assignment. No parameter, catch, destructuring,
// loop or redeclared binding can become a callable.
pub(in crate::naming) fn ordered_function(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    use_start: u32,
    depth: usize,
) -> Option<FunctionSite> {
    if depth > 16 {
        return None;
    }
    if let Expression::Identifier(i) = e {
        let s = crate::analysis::identifier_symbol(i, model.scoping)?;
        if !model.scoping.symbol_redeclarations(s).is_empty() {
            return None;
        }
        let declaration = model.scoping.symbol_declaration(s);
        let writes = model.binding_writes(s);
        if let AstKind::Function(f) = model.nodes.kind(declaration) {
            return (writes == 0).then(|| model.site_by_span(f.span)).flatten();
        }
        let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
            return None;
        };
        if !matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
            || !matches!(
                model.nodes.parent_kind(declaration),
                AstKind::VariableDeclaration(d) if matches!(d.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const)
            )
            || matches!(
                model.nodes.parent_kind(model.nodes.parent_id(declaration)),
                AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
            )
        {
            return None;
        }
        let container = model.nodes.parent_id(model.nodes.parent_id(declaration));
        if !matches!(
            model.nodes.kind(container),
            AstKind::FunctionBody(_) | AstKind::Program(_)
        ) {
            return None;
        }
        let value = match (writes, v.init.as_ref()) {
            (0, Some(init)) => init,
            (1, None) => model.symbol_value(s)?,
            _ => return None,
        };
        if value.span().end > use_start {
            return None;
        }
        // An initless declaration may take its single origin only from a direct
        // assignment in the declaration's own straight-line statement list.
        if v.init.is_none()
            && !model.scoping.get_resolved_references(s).any(|r| {
                let parent = model.nodes.parent_id(r.node_id());
                matches!(model.nodes.kind(parent), AstKind::AssignmentExpression(a)
                    if a.operator.as_str() == "=" && a.right.span() == value.span())
                    && matches!(
                        model.nodes.parent_kind(parent),
                        AstKind::ExpressionStatement(_)
                    )
                    && model.nodes.parent_id(model.nodes.parent_id(parent)) == container
                    && value.span().end <= use_start
            })
        {
            return None;
        }
        return ordered_function(model, value, value.span().start, depth + 1);
    }
    if let Expression::ParenthesizedExpression(parenthesized) = e {
        return ordered_function(model, &parenthesized.expression, use_start, depth + 1);
    }
    model.function_from_expression(e)
}

pub(in crate::naming) fn global_math_call(
    model: &Model<'_, '_>,
    c: &CallExpression<'_>,
    method: &str,
) -> bool {
    matches!(&c.callee, Expression::StaticMemberExpression(m)
        if !m.optional && m.property.name == method
            && expression_path(model, &m.object) == ["Math"])
}

pub(in crate::naming) fn slot_of(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
) -> Option<u32> {
    model.site_parameter_slot(e, site)
}

pub(in crate::naming) fn single_argument<'a>(
    c: &'a CallExpression<'a>,
) -> Option<&'a Expression<'a>> {
    let arguments = call_arguments(c);
    let [argument] = arguments.as_slice() else {
        return None;
    };
    Some(argument)
}
