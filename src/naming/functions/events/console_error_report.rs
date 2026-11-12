use super::*;

// A whole body that only reports one string literal to the unshadowed global
// console error channel. The literal's bytes never identify the rule.
fn report_error_to_console(
    model: &Model<'_, '_>,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if !function.params.items.is_empty() || function.params.rest.is_some() {
        return None;
    }
    let [Statement::ExpressionStatement(statement)] = body.statements.as_slice() else {
        return None;
    };
    let Expression::CallExpression(call) = &statement.expression else {
        return None;
    };
    if call.optional
        || call.arguments.len() != 1
        || call_arguments(call).len() != 1
        || expression_path(model, &call.callee) != ["console", "error"]
        || !matches!(call.arguments.as_slice(), [Argument::StringLiteral(_)])
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
    let mut claims = Vec::new();
    if report_error_to_console(model, function, body).is_some() {
        claims.push(proposal(
            "reportErrorToConsole",
            "shared_anchor_console_error",
            "whole body is one nonoptional console.error call forwarding a single string literal to the unshadowed global console; no parameters or other statements",
            f,
        ));
    }
    claims
}
