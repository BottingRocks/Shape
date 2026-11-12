use super::*;

use crate::naming::support::installer_events::*;

fn handle_ready_state(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if !function.params.items.is_empty()
        || function.params.rest.is_some()
        || function.r#async
        || function.generator
        || !body.directives.is_empty()
    {
        return None;
    }
    let [Statement::IfStatement(branch)] = body.statements.as_slice() else {
        return None;
    };
    let Expression::BinaryExpression(test) = &branch.test else {
        return None;
    };
    let Some(ready) = member(&test.left, "readyState") else {
        return None;
    };
    if test.operator.as_str() != "==="
        || branch.alternate.is_some()
        || !global_path_ends_with(model, ready, &["document"])
        || !string(&test.right, "interactive")
    {
        return None;
    }
    let [Statement::ExpressionStatement(call)] = block(&branch.consequent)? else {
        return None;
    };
    let Expression::CallExpression(call) = &call.expression else {
        return None;
    };
    if call.optional
        || !call.arguments.is_empty()
        || !matches!(&call.callee, Expression::Identifier(_))
        || !ordered_captured_callable(model, site, &call.callee)
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
    let mut claims = Vec::new();
    if handle_ready_state(model, site, function, body).is_some() {
        claims.push(proposal(
            "handleDocumentReadyStateChange",
            "shared_ready_state_handler",
            "sole branch tests an unresolved-global document.readyState === 'interactive' and calls one independently ordered, zero-argument captured callable",
            f,
        ));
    }
    claims
}
