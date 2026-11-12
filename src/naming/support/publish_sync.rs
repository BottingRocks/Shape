use super::*;
use oxc_semantic::SymbolId;

pub(in crate::naming) type Symbol = SymbolId;

pub(in crate::naming) fn symbol(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
) -> Option<Symbol> {
    crate::analysis::expression_symbol(e, model.scoping)
}

// Only a plain var/let/const declaration creates an ordinary data binding;
// `using`/`await using` resource declarations are refused everywhere an origin
// is claimed.
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

// A captured plain variable declared strictly before this function: never a
// parameter, catch binding, destructuring target, resource or for-in/for-of
// binding.
pub(in crate::naming) fn captured_slot(
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
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return false;
    };
    matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
        && matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(d) if plain_variable_kind(d.kind)
        )
        && !matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
        && model.scoping.symbol_span(s).end < site.span.start
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
