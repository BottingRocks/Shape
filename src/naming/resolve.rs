//! Plain, single-origin callable resolution shared by every thread rule.
use super::*;
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;

pub(super) fn symbol(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e, model.scoping)
}

// Resolve only plain, single-origin callables. Each alias must already exist at
// its use; a recovered assignment must share the declaration's statement list.
// This deliberately refuses origins supplied by setup callbacks or loop headers.
pub(super) fn ordered_callable(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    depth: usize,
) -> Option<FunctionSite> {
    if depth > 8 {
        return None;
    }
    let Expression::Identifier(_) = e else {
        return model.function_from_expression(e);
    };
    let s = symbol(model, e)?;
    if !model.scoping.symbol_redeclarations(s).is_empty() {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(s);
    if let AstKind::Function(f) = model.nodes.kind(declaration) {
        return (model.binding_writes(s) == 0)
            .then(|| model.site_by_span(f.span))
            .flatten();
    }
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(d) if matches!(d.kind, VariableDeclarationKind::Var | VariableDeclarationKind::Let | VariableDeclarationKind::Const)
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
    let value = model.symbol_value(s)?;
    if value.span().end > e.span().start {
        return None;
    }
    match (v.init.is_some(), model.binding_writes(s)) {
        (true, 0) => (),
        (false, 1) => {
            let valid = model.scoping.get_resolved_references(s).any(|r| {
                let parent = model.nodes.parent_id(r.node_id());
                matches!(model.nodes.kind(parent), AstKind::AssignmentExpression(a)
                    if a.operator.as_str() == "=" && a.right.span() == value.span())
                    && matches!(
                        model.nodes.parent_kind(parent),
                        AstKind::ExpressionStatement(_)
                    )
                    && model.nodes.parent_id(model.nodes.parent_id(parent)) == container
            });
            if !valid {
                return None;
            }
        }
        _ => return None,
    }
    ordered_callable(model, value, depth + 1)
}
