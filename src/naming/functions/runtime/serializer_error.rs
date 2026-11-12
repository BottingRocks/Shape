use super::*;

use crate::naming::support::installer_events::*;

fn hardcoded_serialization_error(
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
    let [Statement::ThrowStatement(thrown)] = body.statements.as_slice() else {
        return None;
    };
    let Expression::NewExpression(error) = &thrown.argument else {
        return None;
    };
    if !global_path_ends_with(model, &error.callee, &["Error"]) {
        return None;
    }
    let [Argument::StringLiteral(message)] = error.arguments.as_slice() else {
        return None;
    };
    (message.value == "serialisation is hard-coded into the encoder").then_some(())
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
    if hardcoded_serialization_error(model, function, body).is_some() {
        claims.push(proposal(
            "throwHardcodedSerializationError",
            "shared_serializer_error",
            "only statement throws a new Error constructor reached from an unresolved-global path with the exact hard-coded serialisation message",
            f,
        ));
    }
    claims
}
