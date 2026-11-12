use super::*;
use oxc_semantic::SymbolId;

use crate::naming::support::selector_graphics::*;

// ---------------------------------------------------------------------------
// if (event.target) {
//   if (event.target.closest(selectors.join(",")) == null) ++outside;
// } catch (error) { observer.error(error); }
// ---------------------------------------------------------------------------
fn excludes_selector(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
    params: &[SymbolId],
) -> Option<()> {
    let [event] = params else { return None };
    let [Statement::TryStatement(attempt)] = body.statements.as_slice() else {
        return None;
    };
    if attempt.finalizer.is_some() {
        return None;
    }
    let [Statement::IfStatement(guarded)] = attempt.block.body.as_slice() else {
        return None;
    };
    if guarded.alternate.is_some()
        || symbol(model, member(&guarded.test, "target")?) != Some(*event)
    {
        return None;
    }
    let [Statement::IfStatement(inside)] = block(&guarded.consequent)? else {
        return None;
    };
    let Expression::BinaryExpression(inside_test) = &inside.test else {
        return None;
    };
    if inside_test.operator.as_str() != "=="
        || !matches!(&inside_test.right, Expression::NullLiteral(_))
    {
        return None;
    }
    let Expression::CallExpression(closest) = &inside_test.left else {
        return None;
    };
    if closest.optional || closest.arguments.len() != 1 {
        return None;
    }
    let target = member(&closest.callee, "closest")?;
    if symbol(model, member(target, "target")?) != Some(*event) {
        return None;
    }
    let Argument::CallExpression(join) = &closest.arguments[0] else {
        return None;
    };
    captured_receiver(model, site, member(&join.callee, "join")?)?;
    if join.optional
        || join.arguments.len() != 1
        || !matches!(&join.arguments[0], Argument::StringLiteral(separator) if !separator.value.is_empty())
    {
        return None;
    }
    let [Statement::ExpressionStatement(count)] = block(&inside.consequent)? else {
        return None;
    };
    captured_slot(model, site, increment_target(model, &count.expression)?)?;
    let handler = attempt.handler.as_ref()?;
    let caught = catch_symbol(model, handler)?;
    let [Statement::ExpressionStatement(report)] = handler.body.body.as_slice() else {
        return None;
    };
    let Expression::CallExpression(report) = &report.expression else {
        return None;
    };
    captured_receiver(model, site, member(&report.callee, "error")?)?;
    if report.optional
        || report.arguments.len() != 1
        || !matches!(&report.arguments[0], Argument::Identifier(id)
            if crate::analysis::identifier_symbol(id, model.scoping) == Some(caught))
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
    if function.r#async || function.generator || !body.directives.is_empty() {
        return Vec::new();
    }
    let Some(params) = plain_parameters(model, function) else {
        return Vec::new();
    };
    let mut claims = Vec::new();
    if excludes_selector(model, site, body, &params).is_some() {
        claims.push(proposal(
            "countEventsOutsideSelector",
            "events_selector_exclusion_count",
            "event.target existence is guarded, event.target.closest(selectorArray.join(separator)) null result increments one captured count slot, and catch forwards the exact caught binding to one captured error receiver; all captures are straight-line plain bindings",
            f,
        ));
    }
    claims
}
