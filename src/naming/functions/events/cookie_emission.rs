use super::*;
use oxc_semantic::SymbolId;

use crate::naming::support::shared_events::*;

fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = e else {
        return None;
    };
    (!m.optional && m.property.name == key).then_some(&m.object)
}

fn string(e: &Expression<'_>, expected: &str) -> bool {
    matches!(e, Expression::StringLiteral(s) if s.value == expected)
}

fn number(e: &Expression<'_>, expected: f64) -> bool {
    matches!(e, Expression::NumericLiteral(n) if n.value == expected)
}

fn indexed(model: &Model<'_, '_>, e: &Expression<'_>, owner: SymbolId, index: f64) -> bool {
    matches!(e, Expression::ComputedMemberExpression(m) if !m.optional
        && symbol(model, &m.object) == Some(owner) && number(&m.expression, index))
}

fn method_on(model: &Model<'_, '_>, c: &CallExpression<'_>, owner: SymbolId, method: &str) -> bool {
    !c.optional && member(&c.callee, method).and_then(|e| symbol(model, e)) == Some(owner)
}

fn complete(model: &Model<'_, '_>, s: &Statement<'_>, observer: SymbolId) -> bool {
    statement_call(s)
        .is_some_and(|c| method_on(model, c, observer, "complete") && c.arguments.is_empty())
}

// The capture may be initialized once or assigned once after an initless plain
// declaration (the setup(document) pattern). Parameters, loops and destructuring
// cannot establish a plain captured origin.
fn capture(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<SymbolId> {
    let s = symbol(model, e)?;
    let declaration = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return None;
    };
    if site.span.contains_inclusive(model.scoping.symbol_span(s))
        || !model.scoping.symbol_redeclarations(s).is_empty()
        || !matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(_)
        )
        || matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
        || !(model.binding_writes(s) == 0 && v.init.is_some()
            || model.binding_writes(s) == 1 && v.init.is_none())
    {
        return None;
    }
    Some(s)
}

// The cookie emission proof is reused by the custom collector plugin wrapper,
// which owns this callback as its immediate stage. The guard and the name it
// produces (`emitMatchingCookieValue`) are unchanged.
pub(in crate::naming) fn cookie_emission(
    model: &Model<'_, '_>,
    site: FunctionSite,
    input: SymbolId,
    body: &FunctionBody<'_>,
) -> Option<()> {
    // Permit a direct observer parameter or one immutable local copy.
    let (observer, statements) = if body.statements.len() == 4 {
        let (s, value) = local(model, &body.statements[0])?;
        if symbol(model, value) != Some(input) {
            return None;
        }
        (s, &body.statements[1..])
    } else {
        (input, body.statements.as_slice())
    };
    let [each, empty, done] = statements else {
        return None;
    };
    let each = statement_call(each)?;
    let split = call(member(&each.callee, "forEach")?)?;
    let document = member(member(&split.callee, "split")?, "cookie")?;
    capture(model, site, document)?;
    if split.arguments.len() != 1
        || !call_arguments(split)
            .first()
            .is_some_and(|e| string(e, "; "))
    {
        return None;
    }
    let [Argument::ArrowFunctionExpression(callback)] = each.arguments.as_slice() else {
        return None;
    };
    let [p] = callback.params.items.as_slice() else {
        return None;
    };
    if callback.r#async
        || callback.expression
        || callback.params.rest.is_some()
        || p.initializer.is_some()
        || !callback.body.directives.is_empty()
    {
        return None;
    }
    let entry = immutable(model, &p.pattern)?;
    let [parts, Statement::IfStatement(branch)] = callback.body.statements.as_slice() else {
        return None;
    };
    let (parts, split) = local(model, parts)?;
    let split = call(split)?;
    if !method_on(model, split, entry, "split")
        || split.arguments.len() != 1
        || !call_arguments(split)
            .first()
            .is_some_and(|e| string(e, "="))
        || branch.alternate.is_some()
    {
        return None;
    }
    let Expression::LogicalExpression(and) = &branch.test else {
        return None;
    };
    if and.operator.as_str() != "&&" {
        return None;
    }
    let Expression::BinaryExpression(length) = &and.left else {
        return None;
    };
    let Expression::BinaryExpression(key) = &and.right else {
        return None;
    };
    if length.operator.as_str() != ">" || member(&length.left, "length").and_then(|e| symbol(model, e)) != Some(parts)
        || !number(&length.right, 0.0) || key.operator.as_str() != "==="
        || !indexed(model, &key.left, parts, 0.0)
        // Any nonempty cookie key is valid; its actual bytes never identify a rule.
        || !matches!(&key.right, Expression::StringLiteral(s) if !s.value.is_empty())
    {
        return None;
    }
    let Statement::BlockStatement(block) = &branch.consequent else {
        return None;
    };
    let [next, done_match] = block.body.as_slice() else {
        return None;
    };
    let next = statement_call(next)?;
    let args = call_arguments(next);
    if !method_on(model, next, observer, "next")
        || next.arguments.len() != 1
        || !object_field(args.first()?, "value").is_some_and(|e| indexed(model, e, parts, 1.0))
        || !complete(model, done_match, observer)
    {
        return None;
    }
    let empty = statement_call(empty)?;
    if !method_on(model, empty, observer, "next")
        || empty.arguments.len() != 1
        || !matches!(call_arguments(empty).first(), Some(Expression::ObjectExpression(o)) if o.properties.is_empty())
        || !complete(model, done, observer)
    {
        return None;
    }
    Some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return Vec::new();
    };
    let Some(body) = function.body.as_ref() else {
        return Vec::new();
    };
    let [p] = function.params.items.as_slice() else {
        return Vec::new();
    };
    if function.r#async
        || function.generator
        || !body.directives.is_empty()
        || function.params.rest.is_some()
        || p.initializer.is_some()
    {
        return Vec::new();
    }
    let Some(input) = immutable(model, &p.pattern) else {
        return Vec::new();
    };
    let mut claims = Vec::new();
    if cookie_emission(model, site, input, body).is_some() {
        claims.push(proposal(
            "emitMatchingCookieValue",
            "shared_cookie_emission",
            "owned cookie.split('; ').forEach callback splits '=', checks nonempty key at index 0, emits {value:parts[1]} then completes; exact observer identity also receives final empty next and complete; immutable scoped bindings",
            f,
        ));
    }
    claims
}
