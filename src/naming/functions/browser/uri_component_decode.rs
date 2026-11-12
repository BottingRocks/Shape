//! Complete unary forwarding contract, with an unshadowed native global.
use super::*;
use crate::naming::support::media_entropy::{plain_params, returned, symbol};
pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>, site: FunctionSite, f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let prove = || -> Option<()> {
        let AstKind::Function(function) = model.nodes.kind(site.node_id) else { return None; };
        let body = function.body.as_ref()?;
        if function.r#async || function.generator || !body.directives.is_empty() { return None; }
        let params = plain_params(model, function)?;
        let [input] = params.as_slice() else { return None; };
        let [statement] = body.statements.as_slice() else { return None; };
        let Expression::CallExpression(call) = returned(statement)?.without_parentheses() else { return None; };
        if call.optional || expression_path(model, &call.callee) != ["decodeURIComponent"]
            || call.arguments.len() != 1 { return None; }
        let args = call_arguments(call);
        (args.len() == 1 && symbol(model, args[0]) == Some(*input)).then_some(())
    };
    if prove().is_some() { vec![proposal("decodeUriComponent", "browser_uri_component_decode",
        "one immutable plain formal, sole return of the unshadowed decodeURIComponent global called directly with that same formal; no other effects", f)] } else { Vec::new() }
}
