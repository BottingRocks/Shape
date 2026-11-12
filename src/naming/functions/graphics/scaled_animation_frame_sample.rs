use super::*;
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;

use crate::naming::support::selector_graphics::*;

/// The `FunctionBody`/`Program` statement list that directly owns a variable
/// declaration. Conditional blocks, loop bodies and nested function bodies do
/// not qualify, so a recovered assignment there can never be an origin.
fn direct_statement_container(
    model: &Model<'_, '_>,
    declaration: oxc_semantic::NodeId,
) -> Option<oxc_semantic::NodeId> {
    let statement = model.nodes.parent_id(declaration);
    let container = model.nodes.parent_id(statement);
    matches!(
        model.nodes.kind(container),
        AstKind::FunctionBody(_) | AstKind::Program(_)
    )
    .then_some(container)
}

/// The one write-once assignment that gives an init-less declaration its value
/// must be a plain `=` in an `ExpressionStatement` directly owned by the same
/// straight-line statement list as the declaration.
fn sole_assignment_in_container(
    model: &Model<'_, '_>,
    s: SymbolId,
    value: &Expression<'_>,
    container: oxc_semantic::NodeId,
) -> bool {
    model.scoping.get_resolved_references(s).any(|reference| {
        let assignment = model.nodes.parent_id(reference.node_id());
        matches!(model.nodes.kind(assignment), AstKind::AssignmentExpression(a)
            if a.operator.as_str() == "=" && a.right.span() == value.span())
            && matches!(
                model.nodes.parent_kind(assignment),
                AstKind::ExpressionStatement(_)
            )
            && model.nodes.parent_id(model.nodes.parent_id(assignment)) == container
    })
}

/// Resolve a callback identity through plain, single-origin `var`/`let`/`const`
/// aliases only. Each hop obeys ordered_callable's rules: a hoisted
/// zero-write function declaration is always in scope; a variable's value must
/// otherwise complete before the hop's use. Parameters, catch bindings,
/// destructuring targets, loop/resource bindings, redeclarations and other
/// declarations outside a `FunctionBody`/`Program` statement list are refused.
fn stable_symbol(model: &Model<'_, '_>, e: &Expression<'_>, depth: usize) -> Option<FunctionSite> {
    if depth > 16 {
        return None;
    }
    let Expression::Identifier(identifier) = e else {
        return model.function_from_expression(e);
    };
    let s = crate::analysis::identifier_symbol(identifier, model.scoping)?;
    if !model.scoping.symbol_redeclarations(s).is_empty() {
        return None;
    }
    let writes = model.binding_writes(s);
    let declaration = model.scoping.symbol_declaration(s);
    if let AstKind::Function(f) = model.nodes.kind(declaration) {
        return (writes == 0).then(|| model.site_by_span(f.span)).flatten();
    }
    let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&variable.id, BindingPattern::BindingIdentifier(id)
        if id.symbol_id.get() == Some(s))
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
    let container = direct_statement_container(model, declaration)?;
    let (value, origin_end) = match (variable.init.as_ref(), writes) {
        (Some(init), 0) => (init, init.span().end),
        (None, 1) => {
            let value = model.symbol_value(s)?;
            if !sole_assignment_in_container(model, s, value, container) {
                return None;
            }
            (value, value.span().end)
        }
        _ => return None,
    };
    let target = stable_symbol(model, value, depth + 1)?;
    (origin_end <= e.span().start).then_some(target)
}

// ---------------------------------------------------------------------------
// samples.push(Math.round(timestamp / divisor));
// recording = true;
// if (samples.length < limit) scheduler.requestAnimationFrame(thisFunction);
// ---------------------------------------------------------------------------
fn animation_sample(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
    params: &[SymbolId],
) -> Option<()> {
    let [timestamp] = params else { return None };
    let [Statement::ExpressionStatement(record), Statement::ExpressionStatement(flag), Statement::IfStatement(repeat)] =
        body.statements.as_slice()
    else {
        return None;
    };
    let Expression::CallExpression(push) = &record.expression else {
        return None;
    };
    let samples = captured_receiver(model, site, member(&push.callee, "push")?)?;
    if push.optional
        || push.arguments.len() != 1
        || model.binding_writes(samples) != 0
        || model.initializer(samples).is_none()
    {
        return None;
    }
    let Argument::CallExpression(rounded) = &push.arguments[0] else {
        return None;
    };
    if rounded.optional
        || expression_path(model, &rounded.callee) != ["Math", "round"]
        || rounded.arguments.len() != 1
    {
        return None;
    }
    let (scaled, divisor) = binary(call_arguments(rounded).first()?, "/")?;
    if symbol(model, scaled) != Some(*timestamp) {
        return None;
    }
    let divisor = captured_receiver(model, site, divisor)?;
    if model.binding_writes(divisor) != 0 || model.initializer(divisor).is_none() {
        return None;
    }
    let Expression::AssignmentExpression(recording) = &flag.expression else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(recording_id) = &recording.left else {
        return None;
    };
    if recording.operator.as_str() != "="
        || !matches!(&recording.right, Expression::BooleanLiteral(value) if value.value)
        || captured_slot(
            model,
            site,
            crate::analysis::identifier_symbol(recording_id, model.scoping)?,
        )
        .is_none()
    {
        return None;
    }
    if repeat.alternate.is_some() {
        return None;
    }
    let (length, limit) = binary(&repeat.test, "<")?;
    if symbol(model, member(length, "length")?) != Some(samples)
        || !matches!(limit, Expression::NumericLiteral(n) if n.value > 0.0)
    {
        return None;
    }
    let [Statement::ExpressionStatement(schedule)] = block(&repeat.consequent)? else {
        return None;
    };
    let Expression::CallExpression(schedule) = &schedule.expression else {
        return None;
    };
    captured_receiver(
        model,
        site,
        member(&schedule.callee, "requestAnimationFrame")?,
    )?;
    if schedule.optional || schedule.arguments.len() != 1 {
        return None;
    }
    let Argument::Identifier(callback) = &schedule.arguments[0] else {
        return None;
    };
    // The callback argument is an expression, so resolve it with the strict
    // per-hop origin walk rather than a bare symbol lookup.
    crate::analysis::identifier_symbol(callback, model.scoping)?;
    (stable_symbol(model, schedule.arguments[0].as_expression()?, 0)?.span == site.span)
        .then_some(())
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
    if function.r#async || function.generator || !body.directives.is_empty() {
        return Vec::new();
    }
    let Some(params) = plain_parameters(model, function) else {
        return Vec::new();
    };
    let mut claims = Vec::new();
    if animation_sample(model, site, body, &params).is_some() {
        claims.push(proposal(
            "recordAnimationFrameSample",
            "graphics_scaled_animation_frame_sample",
            "rounds the formal timestamp divided by one captured scale into a captured sample array, sets a captured recording flag, and re-requests exactly this function through a captured scheduler while the array is below a positive limit",
            f,
        ));
    }
    claims
}
