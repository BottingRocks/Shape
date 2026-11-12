//! Exact owned statement variant of the canonical encoded-header request guard.
//! The boolean local has exactly three syntactic writes: null input accepts;
//! otherwise both fields must exist before the captured policy is called.
use super::*;

fn null_comparison(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    symbol: SymbolId,
    operator: &str,
    field: Option<&str>,
) -> bool {
    let Expression::BinaryExpression(binary) = expression.without_parentheses() else {
        return false;
    };
    binary.operator.as_str() == operator
        && matches!(
            binary.right.without_parentheses(),
            Expression::NullLiteral(_)
        )
        && match field {
            Some(field) => gc_member_matches(model, &binary.left, symbol, Some(field), None),
            None => gc_symbol(model, &binary.left) == Some(symbol),
        }
}

fn assigned<'a>(
    model: &Model<'_, '_>,
    statement: &'a Statement<'a>,
    symbol: SymbolId,
) -> Option<&'a Expression<'a>> {
    let assignment = gc_plain_assignment(statement)?;
    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
        return None;
    };
    (crate::analysis::identifier_symbol(identifier, model.scoping) == Some(symbol))
        .then_some(&assignment.right)
}

/// Return only the final guard, after proving every statement in the staged
/// prefix. The caller applies the unchanged complete header-copy body proof.
pub(in crate::naming) fn guard<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    statements: &'a [Statement<'a>],
    input: SymbolId,
) -> Option<&'a IfStatement<'a>> {
    let [declaration, Statement::IfStatement(select), Statement::IfStatement(guard)] = statements
    else {
        return None;
    };
    let (pattern, initializer) = gc_single_declarator(declaration)?;
    let state = gc_declarator_symbol(pattern)?;
    if initializer.is_some()
        || !declared_within_body(model, state, site)
        || !model.scoping.symbol_redeclarations(state).is_empty()
        || model.binding_writes(state) != 3
        || !null_comparison(model, &select.test, input, "==", None)
        || gc_symbol(model, &guard.test) != Some(state)
    {
        return None;
    }
    let [accept] = gc_block(&select.consequent)? else {
        return None;
    };
    if !matches!(assigned(model, accept, state)?.without_parentheses(), Expression::BooleanLiteral(value) if value.value)
    {
        return None;
    }
    let [fields, Statement::IfStatement(policy)] = gc_block(select.alternate.as_ref()?)? else {
        return None;
    };
    let Expression::LogicalExpression(fields) =
        assigned(model, fields, state)?.without_parentheses()
    else {
        return None;
    };
    if fields.operator.as_str() != "&&"
        || !null_comparison(model, &fields.left, input, "!=", Some("url"))
        || !null_comparison(model, &fields.right, input, "!=", Some("method"))
        || policy.alternate.is_some()
        || gc_symbol(model, &policy.test) != Some(state)
    {
        return None;
    }
    let [invoke] = gc_block(&policy.consequent)? else {
        return None;
    };
    let Expression::CallExpression(call) = assigned(model, invoke, state)?.without_parentheses()
    else {
        return None;
    };
    let callee = gc_captured(model, site, &call.callee)?;
    if call.optional
        || model.binding_writes(callee) != 0
        || !model.scoping.symbol_redeclarations(callee).is_empty()
        || call.arguments.len() != 1
        || !matches!(call.arguments.first(), Some(Argument::Identifier(identifier))
            if crate::analysis::identifier_symbol(identifier, model.scoping) == Some(input))
    {
        return None;
    }
    Some(guard)
}
