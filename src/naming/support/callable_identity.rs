//! Candidate-local callable proof for an initialized lexical capture with
//! additional exact `slot = slot` stores. No store or effect is removed, and
//! unrelated/compound/property writes never establish a callable identity.
use super::*;

pub(in crate::naming) fn captured_callable(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
) -> Option<FunctionSite> {
    let Expression::Identifier(use_id) = e.without_parentheses() else {
        return None;
    };
    let id = crate::analysis::identifier_symbol(use_id, model.scoping)?;
    if site.span.contains_inclusive(model.scoping.symbol_span(id))
        || !model.scoping.symbol_redeclarations(id).is_empty()
    {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(id);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&v.id, BindingPattern::BindingIdentifier(b) if b.symbol_id.get()==Some(id))
        || !matches!(model.nodes.parent_kind(declaration), AstKind::VariableDeclaration(d) if matches!(d.kind,VariableDeclarationKind::Let|VariableDeclarationKind::Var))
    {
        return None;
    }
    let body = model.nodes.parent_id(model.nodes.parent_id(declaration));
    if !matches!(
        model.nodes.kind(body),
        AstKind::FunctionBody(_) | AstKind::Program(_)
    ) {
        return None;
    }
    let mut origin = v.init.as_ref().map(|value| (value, v.span.end));
    let mut identities = 0;
    for r in model
        .scoping
        .get_resolved_references(id)
        .filter(|r| r.is_write())
    {
        let parent = model.nodes.parent_id(r.node_id());
        let AstKind::AssignmentExpression(a) = model.nodes.kind(parent) else {
            return None;
        };
        if a.operator.as_str() != "="
            || !matches!(&a.left,AssignmentTarget::AssignmentTargetIdentifier(b) if crate::analysis::identifier_symbol(b,model.scoping)==Some(id))
        {
            return None;
        }
        if matches!(a.right.without_parentheses(),Expression::Identifier(b) if crate::analysis::identifier_symbol(b,model.scoping)==Some(id))
        {
            identities += 1;
            continue;
        }
        let statement = model.nodes.parent_id(parent);
        if origin.is_some()
            || !matches!(model.nodes.kind(statement), AstKind::ExpressionStatement(_))
            || model.nodes.parent_id(statement) != body
            || a.span.start < v.span.end
        {
            return None;
        }
        origin = Some((&a.right, a.span.end));
    }
    if identities == 0 {
        return None;
    }
    let (value, end) = origin?;
    if end > site.span.start {
        return None;
    }
    let use_node = model
        .scoping
        .get_reference(use_id.reference_id.get()?)
        .node_id();
    let origin_reference = if let Expression::Identifier(b) = value {
        Some(model.scoping.get_reference(b.reference_id.get()?).node_id())
    } else {
        None
    };
    if !super::super::shared::callable_origin_precedes(
        model,
        body,
        end,
        use_node,
        origin_reference,
        0,
        true,
    ) {
        return None;
    }
    let target = super::super::resolve::ordered_callable(model, value, 0)?;
    (!site.span.contains_inclusive(target.span) && target.span.end <= site.span.start)
        .then_some(target)
}
