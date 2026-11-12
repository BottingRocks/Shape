use super::*;
use oxc_span::GetSpan;

use crate::naming::support::entropy_relay::*;

fn ordered_data_origin(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    use_start: u32,
    depth: usize,
) -> bool {
    if depth > 16 {
        return false;
    }
    if let Expression::ParenthesizedExpression(parenthesized) = e {
        return ordered_data_origin(model, &parenthesized.expression, use_start, depth + 1);
    }
    let Expression::Identifier(identifier) = e else {
        return true;
    };
    let Some(s) = crate::analysis::identifier_symbol(identifier, model.scoping) else {
        return false;
    };
    if !model.scoping.symbol_redeclarations(s).is_empty() {
        return false;
    }
    let declaration = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return matches!(model.nodes.kind(declaration), AstKind::Function(_))
            && model.binding_writes(s) == 0;
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
        return false;
    }
    let writes = model.binding_writes(s);
    let value = match (writes, v.init.as_ref()) {
        (0, Some(init)) => init,
        (1, None) => {
            let Some(value) = model.symbol_value(s) else {
                return false;
            };
            value
        }
        _ => return false,
    };
    if value.span().end > use_start {
        return false;
    }
    let container = model.nodes.parent_id(model.nodes.parent_id(declaration));
    // Whether the declarator carries an initializer or takes its sole value
    // from a later assignment, a deterministic origin must live in a direct
    // FunctionBody/Program statement list. A declarator inside a conditional
    // block or other nested container does not dominate the use site, so an
    // init-present conditional receiver may not claim a stable origin.
    if !matches!(
        model.nodes.kind(container),
        AstKind::FunctionBody(_) | AstKind::Program(_)
    ) || (v.init.is_none()
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
        }))
    {
        return false;
    }
    ordered_data_origin(model, value, value.span().start, depth + 1)
}

// A callback/receiver identity needs one origin: either a plain immutable
// initializer or exactly one later direct assignment. Mutated aliases are
// admitted only as DATA, never as this role's identity.
fn single_origin_capture(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
) -> Option<Symbol> {
    let s = data_capture(model, site, e)?;
    let declaration = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return None;
    };
    let writes = model.binding_writes(s);
    let value = match (writes, v.init.as_ref()) {
        (0, Some(init)) => init,
        (1, None) => model.symbol_value(s)?,
        _ => return None,
    };
    if value.span().end > e.span().start {
        return None;
    }
    if !ordered_data_origin(model, value, value.span().start, 0) {
        return None;
    }
    let container = model.nodes.parent_id(model.nodes.parent_id(declaration));
    if !matches!(
        model.nodes.kind(container),
        AstKind::FunctionBody(_) | AstKind::Program(_)
    ) || (v.init.is_none()
        && !model.scoping.get_resolved_references(s).any(|r| {
            let parent = model.nodes.parent_id(r.node_id());
            matches!(model.nodes.kind(parent), AstKind::AssignmentExpression(a)
                if a.operator.as_str() == "=" && a.right.span() == value.span())
                && matches!(
                    model.nodes.parent_kind(parent),
                    AstKind::ExpressionStatement(_)
                )
                && model.nodes.parent_id(model.nodes.parent_id(parent)) == container
                && value.span().end <= e.span().start
        }))
    {
        return None;
    }
    Some(s)
}

// ``Math.round(timestamp / divisor)`` pushed into a captured sample array,
// followed by a captured "sampling" flag and a bounded request for one more
// animation frame. Every slot is a distinct scoped identity.
fn record_animation_frame_sample(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> bool {
    let [push, flag, request] = body.statements.as_slice() else {
        return false;
    };
    let Some(push) = statement_call(push) else {
        return false;
    };
    let Some(samples) = member(&push.callee, "push").and_then(|e| data_capture(model, site, e))
    else {
        return false;
    };
    let [Argument::CallExpression(round)] = push.arguments.as_slice() else {
        return false;
    };
    if !global_math_call(model, round, "round") {
        return false;
    }
    let Some(quotient) = single_argument(round) else {
        return false;
    };
    let Expression::BinaryExpression(quotient) = quotient else {
        return false;
    };
    let Some(divisor) = symbol(model, &quotient.right) else {
        return false;
    };
    if quotient.operator.as_str() != "/"
        || slot_of(model, site, &quotient.left) != Some(0)
        || divisor == samples
        || data_capture(model, site, &quotient.right) != Some(divisor)
    {
        return false;
    }
    let Some(expr) = expression_of(flag) else {
        return false;
    };
    let Expression::AssignmentExpression(set) = expr else {
        return false;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &set.left else {
        return false;
    };
    let Some(flag_symbol) = crate::analysis::identifier_symbol(target, model.scoping) else {
        return false;
    };
    if set.operator.as_str() != "="
        || !matches!(&set.right, Expression::BooleanLiteral(b) if b.value)
        || flag_symbol == samples
        || flag_symbol == divisor
        || !captured_data_symbol(model, site, flag_symbol)
    {
        return false;
    }
    let Statement::IfStatement(branch) = request else {
        return false;
    };
    if branch.alternate.is_some() {
        return false;
    }
    let Expression::BinaryExpression(bound) = &branch.test else {
        return false;
    };
    let Some(length) = member(&bound.left, "length") else {
        return false;
    };
    if bound.operator.as_str() != "<"
        || !is_symbol(model, length, samples)
        || !matches!(&bound.right, Expression::NumericLiteral(n) if n.value.is_finite() && n.value > 0.0)
    {
        return false;
    }
    let Some([Statement::ExpressionStatement(again)]) = block(&branch.consequent) else {
        return false;
    };
    let Expression::CallExpression(again) = &again.expression else {
        return false;
    };
    let Some(receiver) = member(&again.callee, "requestAnimationFrame")
        .and_then(|e| single_origin_capture(model, site, e))
    else {
        return false;
    };
    let Some(callback_expression) = single_argument(again) else {
        return false;
    };
    let Some(callback) = single_origin_capture(model, site, callback_expression) else {
        return false;
    };
    if ordered_function(
        model,
        callback_expression,
        callback_expression.span().start,
        0,
    )
    .is_none()
    {
        return false;
    }
    !(again.optional
        || callback == samples
        || callback == divisor
        || callback == flag_symbol
        || callback == receiver
        || receiver == samples
        || receiver == divisor)
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
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !body.directives.is_empty()
    {
        return Vec::new();
    }
    let plain_params = function.params.items.iter().all(|p| {
        p.initializer.is_none()
            && matches!(&p.pattern, BindingPattern::BindingIdentifier(id)
                if id.symbol_id.get().is_some_and(|s| unchanged(model, s)))
    });
    if !plain_params || function.params.items.len() != 1 || f.calls.is_empty() {
        return Vec::new();
    }
    let input = function.params.items.first().and_then(|parameter| {
        let BindingPattern::BindingIdentifier(identifier) = &parameter.pattern else {
            return None;
        };
        identifier.symbol_id.get()
    });
    let _ = input;
    let mut claims = Vec::new();
    if record_animation_frame_sample(model, site, body) {
        claims.push(proposal(
            "recordAnimationFrameSample",
            "graphics_frame_sample_request",
            "pushes Math.round(formal0 / captured divisor) into one captured sample array, sets a distinct captured sampling flag, and requests exactly one more frame through a distinct captured receiver only while the captured array is below a positive literal bound; all captures are plain ancestor declarators",
            f,
        ));
    }
    claims
}
