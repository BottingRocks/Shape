use super::*;
use oxc_span::GetSpan;

pub(in crate::naming) type Symbol = oxc_semantic::SymbolId;

pub(in crate::naming) fn symbol(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
) -> Option<Symbol> {
    crate::analysis::expression_symbol(e, model.scoping)
}

pub(in crate::naming) fn unchanged(model: &Model<'_, '_>, s: Symbol) -> bool {
    model.binding_writes(s) == 0 && model.scoping.symbol_redeclarations(s).is_empty()
}

pub(in crate::naming) fn member<'a>(
    e: &'a Expression<'a>,
    key: &str,
) -> Option<&'a Expression<'a>> {
    match e {
        Expression::StaticMemberExpression(m) if !m.optional && m.property.name == key => {
            Some(&m.object)
        }
        _ => None,
    }
}

pub(in crate::naming) fn call<'a>(
    e: &'a Expression<'a>,
) -> Option<&'a CallExpression<'a>> {
    match e {
        Expression::CallExpression(c) if !c.optional => Some(c),
        _ => None,
    }
}

pub(in crate::naming) fn expression<'a>(
    s: &'a Statement<'a>,
) -> Option<&'a Expression<'a>> {
    match s {
        Statement::ExpressionStatement(s) => Some(&s.expression),
        _ => None,
    }
}

pub(in crate::naming) fn statement_call<'a>(
    s: &'a Statement<'a>,
) -> Option<&'a CallExpression<'a>> {
    call(expression(s)?)
}

pub(in crate::naming) fn assignment<'a>(
    s: &'a Statement<'a>,
) -> Option<&'a AssignmentExpression<'a>> {
    match expression(s)? {
        Expression::AssignmentExpression(a) if a.operator.as_str() == "=" => Some(a),
        _ => None,
    }
}

pub(in crate::naming) fn plain_params(
    model: &Model<'_, '_>,
    params: &FormalParameters<'_>,
) -> bool {
    params.rest.is_none()
        && params.items.iter().all(|p| {
            p.initializer.is_none()
                && matches!(&p.pattern, BindingPattern::BindingIdentifier(id)
            if id.symbol_id.get().is_some_and(|s| unchanged(model, s)))
        })
}

// Strict plain `var`/`let`/`const` origin. Parameters, catch bindings,
// destructuring loop heads and redeclarations never become API or callable
// proof. DATA receivers may be mutated; CALLABLE proof also requires
// unchanged().
pub(in crate::naming) fn plain_origin(
    model: &Model<'_, '_>,
    s: Symbol,
) -> bool {
    if !model.scoping.symbol_redeclarations(s).is_empty() {
        return false;
    }
    let declaration = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return false;
    };
    matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
        && matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(d)
                if matches!(
                    d.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                )
        )
        && !matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
}

// DATA receiver identity: a plain scoped origin declared before the use and
// outside this function's own body.
pub(in crate::naming) fn data_capture(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
) -> Option<Symbol> {
    let s = symbol(model, e)?;
    capture_identity(model, site, s, e.span().start).then_some(s)
}

pub(in crate::naming) fn capture_identity(
    model: &Model<'_, '_>,
    site: FunctionSite,
    s: Symbol,
    use_start: u32,
) -> bool {
    plain_origin(model, s)
        && !site.span.contains_inclusive(model.scoping.symbol_span(s))
        && model.scoping.symbol_span(s).end <= use_start
}

// Bounded callable order checker. It mirrors the shared adapters gate but lives
// here so this rule family is self-contained.
pub(in crate::naming) fn callable_order(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    depth: usize,
) -> Option<()> {
    if depth > 16 {
        return None;
    }
    let Expression::Identifier(_) = e else {
        return Some(());
    };
    let s = symbol(model, e)?;
    if !model.scoping.symbol_redeclarations(s).is_empty() {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(s);
    if matches!(model.nodes.kind(declaration), AstKind::Function(_)) {
        return unchanged(model, s).then_some(());
    }
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return None;
    };
    if !plain_origin(model, s)
        || !(model.binding_writes(s) == 0 && v.init.is_some()
            || model.binding_writes(s) == 1 && v.init.is_none())
    {
        return None;
    }
    let value = model.symbol_value(s)?;
    if value.span().end > e.span().start {
        return None;
    }
    let container = model.nodes.parent_id(model.nodes.parent_id(declaration));
    if !matches!(
        model.nodes.kind(container),
        AstKind::FunctionBody(_) | AstKind::Program(_)
    ) {
        return None;
    }
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
        })
    {
        return None;
    }
    callable_order(model, value, depth + 1)
}

pub(in crate::naming) fn callable_capture(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
) -> Option<FunctionSite> {
    let s = symbol(model, e)?;
    if site.span.contains_inclusive(model.scoping.symbol_span(s)) {
        return None;
    }
    let target = stable_function(model, e, 0)?;
    (target.span != site.span && callable_order(model, e, 0).is_some()).then_some(target)
}

// An unshadowed global namespace path. A bound identifier is never treated as a
// host global by spelling; only unresolved roots are terminal.
pub(in crate::naming) fn global_path(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
) -> Vec<String> {
    match e {
        Expression::Identifier(i) => {
            if crate::analysis::identifier_symbol(i, model.scoping).is_some() {
                Vec::new()
            } else {
                vec![i.name.to_string()]
            }
        }
        Expression::StaticMemberExpression(m) if !m.optional => {
            let mut path = global_path(model, &m.object);
            if !path.is_empty() {
                path.push(m.property.name.to_string());
            }
            path
        }
        _ => Vec::new(),
    }
}

pub(in crate::naming) fn is_global(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    path: &[&str],
) -> bool {
    global_path(model, e) == path
}

pub(in crate::naming) fn number(e: &Expression<'_>, n: f64) -> bool {
    matches!(e, Expression::NumericLiteral(v) if v.value == n)
}

pub(in crate::naming) fn string(e: &Expression<'_>, expected: &str) -> bool {
    matches!(e, Expression::StringLiteral(s) if s.value == expected)
}

pub(in crate::naming) fn assign_symbol(
    model: &Model<'_, '_>,
    a: &AssignmentExpression<'_>,
) -> Option<Symbol> {
    let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
        return None;
    };
    crate::analysis::identifier_symbol(id, model.scoping)
}

pub(in crate::naming) fn update_symbol(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    prefix: bool,
    operator: &str,
) -> Option<Symbol> {
    let Expression::UpdateExpression(u) = e else {
        return None;
    };
    if u.prefix != prefix || u.operator.as_str() != operator {
        return None;
    }
    match &u.argument {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => {
            crate::analysis::identifier_symbol(id, model.scoping)
        }
        _ => None,
    }
}

pub(in crate::naming) fn arity<'a>(
    c: &'a CallExpression<'a>,
    count: usize,
) -> Option<Vec<&'a Expression<'a>>> {
    let args = call_arguments(c);
    (args.len() == count && c.arguments.len() == count).then_some(args)
}

pub(in crate::naming) fn directives(body: &FunctionBody<'_>) -> bool {
    body.directives.is_empty()
        || (body.directives.len() == 1 && body.directives[0].directive == "use strict")
}
