use super::*;

use crate::naming::support::seeded_flow::*;

// ---------------------------------------------------------------------------
// `reportErrorToConsole`: one bare console error report, no result.
// ---------------------------------------------------------------------------
fn report_error_to_console(model: &Model<'_, '_>, body: &FunctionBody<'_>) -> Option<()> {
    let [s] = body.statements.as_slice() else {
        return None;
    };
    let c = statement_call(s)?;
    let args = arity(c, 1)?;
    (is_global(model, &c.callee, &["console", "error"])
        && matches!(args[0], Expression::StringLiteral(_)))
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
    if function.r#async
        || function.generator
        || !directives(body)
        || !plain_params(model, &function.params)
    {
        return Vec::new();
    }
    let mut claims = Vec::new();
    if function.params.items.is_empty() && report_error_to_console(model, body).is_some() {
        claims.push(proposal(
            "reportErrorToConsole",
            "events_console_error_statement",
            "sole expression statement reports one string diagnostic through the unshadowed console.error API; no return",
            f,
        ));
    }
    claims
}
