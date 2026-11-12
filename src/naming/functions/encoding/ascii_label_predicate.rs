//! Whole-body lowercase ASCII letter/digit/hyphen predicate. Captured bounds
//! must be single-origin values produced by one independently proved callable.
//! This proves neither a URL parser nor any enclosing request-matching role.
use super::*;
use oxc_span::Span;
use oxc_syntax::operator::BinaryOperator;

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e.without_parentheses(), model.scoping)
}
fn immutable(model: &Model<'_, '_>, s: SymbolId) -> bool {
    model.binding_writes(s) == 0 && model.scoping.symbol_redeclarations(s).is_empty()
}
fn formal(params: &FormalParameters<'_>) -> Option<SymbolId> {
    let [p] = params.items.as_slice() else {
        return None;
    };
    if params.rest.is_some() || p.initializer.is_some() {
        return None;
    }
    let BindingPattern::BindingIdentifier(id) = &p.pattern else {
        return None;
    };
    id.symbol_id.get()
}
fn returned<'a>(body: &'a FunctionBody<'a>) -> Option<&'a Expression<'a>> {
    let [Statement::ReturnStatement(ret)] = body.statements.as_slice() else {
        return None;
    };
    ret.argument.as_ref().map(Expression::without_parentheses)
}
fn call<'a>(e: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(c) = e.without_parentheses() else {
        return None;
    };
    (!c.optional).then_some(c)
}
/// An explicit ordinary-call contract: this exact resolved helper has one
/// immutable formal and its entire body returns that formal's charCodeAt().
fn accessor(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Span> {
    let target = shared::stable_function(model, e.without_parentheses(), 0)?;
    let (params, body, expression) = match model.nodes.kind(target.node_id) {
        AstKind::Function(f) if !f.r#async && !f.generator => (&f.params, f.body.as_ref()?, false),
        AstKind::ArrowFunctionExpression(a) if !a.r#async => (&a.params, &a.body, a.expression),
        _ => return None,
    };
    let input = formal(params)?;
    if !immutable(model, input) || !body.directives.is_empty() {
        return None;
    }
    let value = if expression {
        let [Statement::ExpressionStatement(s)] = body.statements.as_slice() else {
            return None;
        };
        &s.expression
    } else {
        returned(body)?
    };
    let c = call(value)?;
    let Expression::StaticMemberExpression(m) = c.callee.without_parentheses() else {
        return None;
    };
    (c.arguments.is_empty()
        && !m.optional
        && m.property.name == "charCodeAt"
        && symbol(model, &m.object) == Some(input))
    .then_some(target.span)
}
/// The sole value origin must be in the same direct owner body as the plain
/// captured declaration. A nested callback/conditional write is never a donor.
fn boundary(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
    expected: &str,
) -> Option<Span> {
    let s = symbol(model, e)?;
    if site.span.contains_inclusive(model.scoping.symbol_span(s))
        || !model.scoping.symbol_redeclarations(s).is_empty()
    {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get()==Some(s))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(_)
        )
    {
        return None;
    }
    let owner = model.nodes.parent_id(model.nodes.parent_id(declaration));
    if !matches!(
        model.nodes.kind(owner),
        AstKind::FunctionBody(_) | AstKind::Program(_)
    ) {
        return None;
    }
    let value = if let Some(value) = v.init.as_ref() {
        if model.binding_writes(s) != 0 {
            return None;
        }
        value
    } else {
        if model.binding_writes(s) != 1 {
            return None;
        }
        let reference = model
            .scoping
            .get_resolved_references(s)
            .find(|r| r.is_write())?;
        let assignment_node = model.nodes.parent_id(reference.node_id());
        let AstKind::AssignmentExpression(a) = model.nodes.kind(assignment_node) else {
            return None;
        };
        let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
            return None;
        };
        let statement = model.nodes.parent_id(assignment_node);
        if a.operator != AssignmentOperator::Assign
            || crate::analysis::identifier_symbol(id, model.scoping) != Some(s)
            || !matches!(model.nodes.kind(statement), AstKind::ExpressionStatement(_))
            || model.nodes.parent_id(statement) != owner
            || a.span.start < v.span.end
        {
            return None;
        }
        &a.right
    };
    let c = call(value)?;
    if !matches!(c.arguments.as_slice(), [Argument::StringLiteral(text)] if text.value == expected)
    {
        return None;
    }
    accessor(model, &c.callee)
}
fn logical<'a>(e: &'a Expression<'a>, op: LogicalOperator) -> Option<&'a LogicalExpression<'a>> {
    let Expression::LogicalExpression(l) = e.without_parentheses() else {
        return None;
    };
    (l.operator == op).then_some(l)
}
fn range(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
    input: SymbolId,
    low: &str,
    high: &str,
) -> Option<Span> {
    let and = logical(e, LogicalOperator::And)?;
    let Expression::BinaryExpression(a) = and.left.without_parentheses() else {
        return None;
    };
    let Expression::BinaryExpression(b) = and.right.without_parentheses() else {
        return None;
    };
    if a.operator != BinaryOperator::GreaterEqualThan
        || b.operator != BinaryOperator::LessEqualThan
        || symbol(model, &a.left) != Some(input)
        || symbol(model, &b.left) != Some(input)
    {
        return None;
    }
    let helper = boundary(model, site, &a.right, low)?;
    (boundary(model, site, &b.right, high)? == helper).then_some(helper)
}
fn contract(model: &Model<'_, '_>, site: FunctionSite, f: &Function<'_>) -> Option<()> {
    if f.r#async || f.generator {
        return None;
    }
    let input = formal(&f.params)?;
    if !immutable(model, input) {
        return None;
    }
    let outer = logical(returned(f.body.as_ref()?)?, LogicalOperator::Or)?;
    let ranges = logical(&outer.left, LogicalOperator::Or)?;
    let digit_helper = range(model, site, &ranges.left, input, "0", "9")?;
    let letter_helper = range(model, site, &ranges.right, input, "a", "z")?;
    let Expression::BinaryExpression(hyphen) = outer.right.without_parentheses() else {
        return None;
    };
    (digit_helper == letter_helper && hyphen.operator == BinaryOperator::StrictEquality
        && symbol(model, &hyphen.left) == Some(input)
        && matches!(hyphen.right.without_parentheses(), Expression::NumericLiteral(n) if n.value==45.0)).then_some(())
}
pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
) -> Vec<Proposal> {
    let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
        return vec![];
    };
    if contract(model, site, f).is_none() {
        return vec![];
    }
    vec![proposal("isLowercaseAsciiLetterDigitOrHyphen", "shared_ascii_label_predicate",
        "whole owned return checks the same immutable formal in inclusive 0..9 and a..z ranges or strict equality to 45; all four captured boundaries have one direct owner-body origin calling the same resolved helper with the exact endpoint character; the helper independently returns only its own immutable formal's charCodeAt()", facts)]
}
