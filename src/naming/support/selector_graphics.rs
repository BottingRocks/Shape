use super::*;
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;

pub(in crate::naming) fn symbol(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e, model.scoping)
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

pub(in crate::naming) fn block<'a>(
    s: &'a Statement<'a>,
) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(b.body.as_slice())
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

pub(in crate::naming) fn number(e: &Expression<'_>, n: f64) -> bool {
    matches!(e, Expression::NumericLiteral(v) if v.value == n)
}

pub(in crate::naming) fn plain_parameters(
    model: &Model<'_, '_>,
    function: &Function<'_>,
) -> Option<Vec<SymbolId>> {
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

pub(in crate::naming) fn plain_variable_kind(
    kind: VariableDeclarationKind,
) -> bool {
    matches!(
        kind,
        VariableDeclarationKind::Var
            | VariableDeclarationKind::Let
            | VariableDeclarationKind::Const
    )
}

/// A captured plain variable only: never a parameter, catch, destructuring or
/// loop target, never a `using`/`await using` resource, and its declaration
/// must complete before the proven function site.
pub(in crate::naming) fn captured_slot(
    model: &Model<'_, '_>,
    site: FunctionSite,
    s: SymbolId,
) -> Option<SymbolId> {
    if site.span.contains_inclusive(model.scoping.symbol_span(s))
        || model.scoping.symbol_span(s).end >= site.span.start
        || !model.scoping.symbol_redeclarations(s).is_empty()
        || model.formal_symbol_slot(s).is_some()
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
            AstKind::VariableDeclaration(d) if plain_variable_kind(d.kind)
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

pub(in crate::naming) fn captured(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
) -> Option<SymbolId> {
    captured_slot(model, site, symbol(model, e)?)
}

pub(in crate::naming) fn direct_body_statement(
    model: &Model<'_, '_>,
    node: oxc_semantic::NodeId,
) -> bool {
    matches!(
        model.nodes.parent_kind(node),
        AstKind::FunctionBody(_) | AstKind::Program(_)
    )
}

/// A capture with a deterministic value fixed before the function: either its
/// declarator initializer with no later writes, or exactly one direct `=`
/// statement in the same straight-line body container. The initializer or
/// assignment expression must complete before the function site, and the
/// declaration must be a plain `var`/`let`/`const` in that same container.
pub(in crate::naming) fn initialized_before<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    s: SymbolId,
) -> Option<&'a Expression<'a>> {
    if let Some(init) = model.initializer(s) {
        if model.binding_writes(s) != 0 || init.span().end > site.span.start {
            return None;
        }
        let declaration_statement = model.nodes.parent_id(model.scoping.symbol_declaration(s));
        return direct_body_statement(model, declaration_statement).then_some(init);
    }
    if model.binding_writes(s) != 1 {
        return None;
    }
    let mut writes = model
        .scoping
        .get_resolved_references(s)
        .filter(|reference| reference.is_write());
    let reference = writes.next()?;
    if writes.next().is_some() {
        return None;
    }
    let assignment = model.nodes.parent_id(reference.node_id());
    let AstKind::AssignmentExpression(a) = model.nodes.kind(assignment) else {
        return None;
    };
    if a.operator.as_str() != "=" || a.span.end > site.span.start {
        return None;
    }
    let statement = model.nodes.parent_id(assignment);
    if !matches!(model.nodes.kind(statement), AstKind::ExpressionStatement(_))
        || !direct_body_statement(model, statement)
    {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(s);
    let declaration_statement = model.nodes.parent_id(declaration);
    if !direct_body_statement(model, declaration_statement)
        || model
            .owning_site(declaration_statement)
            .map(|owner| owner.span)
            != model.owning_site(statement).map(|owner| owner.span)
    {
        return None;
    }
    Some(&a.right)
}

pub(in crate::naming) fn captured_receiver(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
) -> Option<SymbolId> {
    let s = captured(model, site, e)?;
    initialized_before(model, site, s)?;
    Some(s)
}

pub(in crate::naming) fn increment_target(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
) -> Option<SymbolId> {
    let Expression::UpdateExpression(update) = e else {
        return None;
    };
    if update.operator.as_str() != "++" {
        return None;
    }
    match &update.argument {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => {
            crate::analysis::identifier_symbol(id, model.scoping)
        }
        _ => None,
    }
}

pub(in crate::naming) fn assignment<'a>(
    statement: &'a ExpressionStatement<'a>,
    model: &Model<'_, '_>,
) -> Option<(SymbolId, &'a Expression<'a>)> {
    let Expression::AssignmentExpression(a) = &statement.expression else {
        return None;
    };
    if a.operator.as_str() != "=" {
        return None;
    }
    let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
        return None;
    };
    Some((
        crate::analysis::identifier_symbol(id, model.scoping)?,
        &a.right,
    ))
}

pub(in crate::naming) fn catch_symbol(
    model: &Model<'_, '_>,
    handler: &oxc_ast::ast::CatchClause<'_>,
) -> Option<SymbolId> {
    let BindingPattern::BindingIdentifier(caught) = &handler.param.as_ref()?.pattern else {
        return None;
    };
    let caught = caught.symbol_id.get()?;
    (model.binding_writes(caught) == 0 && model.scoping.symbol_redeclarations(caught).is_empty())
        .then_some(caught)
}
