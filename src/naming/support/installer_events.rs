use super::*;
use oxc_semantic::SymbolId;

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

pub(in crate::naming) fn immutable_binding(
    model: &Model<'_, '_>,
    p: &BindingPattern<'_>,
) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(id) = p else {
        return None;
    };
    let s = id.symbol_id.get()?;
    unchanged(model, s).then_some(s)
}

pub(in crate::naming) fn unredeclared_binding(
    model: &Model<'_, '_>,
    p: &BindingPattern<'_>,
) -> Option<Symbol> {
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

pub(in crate::naming) fn statement_call<'a>(
    s: &'a Statement<'a>,
) -> Option<&'a CallExpression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    call(&s.expression)
}

pub(in crate::naming) fn block<'a>(
    s: &'a Statement<'a>,
) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(b.body.as_slice())
}

pub(in crate::naming) fn string(e: &Expression<'_>, expected: &str) -> bool {
    matches!(e, Expression::StringLiteral(s) if s.value == expected)
}

pub(in crate::naming) fn number(e: &Expression<'_>, expected: f64) -> bool {
    matches!(e, Expression::NumericLiteral(n) if n.value == expected)
}

// A suffix-only spelling never proves that a namespace is global: `let
// console = ...` shadows the free reference. This path is non-empty only when
// its root identifier is unresolved, so the whole member chain starts at a
// free/global reference rather than a local binding or parameter.
pub(in crate::naming) fn global_path(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
) -> Option<Vec<String>> {
    match e {
        Expression::Identifier(i) if symbol(model, e).is_none() => Some(vec![i.name.to_string()]),
        Expression::StaticMemberExpression(m) if !m.optional => {
            let mut path = global_path(model, &m.object)?;
            path.push(m.property.name.to_string());
            Some(path)
        }
        _ => None,
    }
}

pub(in crate::naming) fn global_path_ends_with(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    suffix: &[&str],
) -> bool {
    global_path(model, e).is_some_and(|path| {
        path.len() >= suffix.len()
            && path[path.len() - suffix.len()..]
                .iter()
                .map(String::as_str)
                .eq(suffix.iter().copied())
    })
}

// Independently ordered callable proof: the shared resolver must recover a
// plain Var/Let/Const initializer (or a direct same-body assignment) that
// completes before this use, every alias hop is checked by that resolver, and
// the function declaration itself must sit directly in a FunctionBody/Program
// body. A conditional/block/loop function declaration is not unconditionally
// callable and is refused.
pub(in crate::naming) fn ordered_captured_callable(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
) -> bool {
    let Some(target) = super::super::resolve::ordered_callable(model, e, 0) else {
        return false;
    };
    if target.span == site.span || site.span.contains_inclusive(target.span) {
        return false;
    }
    match model.nodes.kind(target.node_id) {
        AstKind::Function(function) if function.r#type == FunctionType::FunctionDeclaration => {
            matches!(
                model.nodes.parent_kind(target.node_id),
                AstKind::FunctionBody(_) | AstKind::Program(_)
            )
        }
        AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => true,
        _ => false,
    }
}

/// A class-member installer role already claimed for this site keeps the
/// duplicate descriptor-family proofs from re-claiming the same body.
pub(in crate::naming) fn prior_class_member_roles(
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> bool {
    proposals.values().flatten().any(|proposal| {
        matches!(
            proposal.name.as_str(),
            "createClassMembersInstaller" | "defineClassMembers"
        )
    })
}
