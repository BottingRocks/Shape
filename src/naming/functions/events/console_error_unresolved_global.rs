use super::*;

use crate::naming::support::installer_events::*;

// Body-only evidence for a console error report. The name is contextual: it
// does not claim an `{init,teardown}` pair or infer an initialization phase
// from the diagnostic literal. The receiver path must start at an unresolved
// global reference so a local/shadowed `console` object is not called the
// global console.
fn report_error_to_console(
    model: &Model<'_, '_>,
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
    let [statement] = body.statements.as_slice() else {
        return None;
    };
    let call = statement_call(statement)?;
    let Some(receiver) = member(&call.callee, "error") else {
        return None;
    };
    if !global_path_ends_with(model, receiver, &["console"]) {
        return None;
    }
    matches!(call.arguments.as_slice(), [Argument::StringLiteral(message)] if message.value == "no global")
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
    let mut claims = Vec::new();
    if report_error_to_console(model, function, body).is_some() {
        claims.push(proposal(
            "reportErrorToConsole",
            "shared_console_error_report",
            "sole statement reports the exact literal 'no global' through an .error operation on a console path rooted at an unresolved global reference; no initialization context is inferred",
            f,
        ));
    }
    claims
}
