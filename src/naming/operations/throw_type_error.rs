//! Exact zero-formal throw-new-TypeError operation. The message is retained
//! literally; no native failure cause or surrounding accessor role is inferred.
use super::*;

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return Vec::new();
    };
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !function.params.items.is_empty()
    {
        return Vec::new();
    }
    let Some(body) = function.body.as_ref() else {
        return Vec::new();
    };
    if !body.directives.is_empty() {
        return Vec::new();
    }
    let [Statement::ThrowStatement(thrown)] = body.statements.as_slice() else {
        return Vec::new();
    };
    let Expression::NewExpression(construction) = thrown.argument.without_parentheses() else {
        return Vec::new();
    };
    let Expression::Identifier(callee) = construction.callee.without_parentheses() else {
        return Vec::new();
    };
    if callee.name != "TypeError"
        || crate::analysis::identifier_symbol(callee, model.scoping).is_some()
    {
        return Vec::new();
    }
    if !matches!(
        construction.arguments.as_slice(),
        [Argument::StringLiteral(_)]
    ) {
        return Vec::new();
    }
    let mut claim = proposal("throwNewTypeErrorWithMessage", "operations_throw_type_error_message", "Complete zero-formal own body throws a new unshadowed TypeError with exactly its source string-literal message; no return, captures, additional statements, computed callee or argument effects; surrounding failure cause and accessor semantics are unproved", facts);
    claim.evidence = vec![site.span.into(), thrown.span.into()];
    vec![claim]
}

#[cfg(test)]
mod tests {
    #[test]
    fn throw_type_error_complete_and_near_misses() {
        for (body, expected) in [
            ("throw new TypeError('message');", true),
            ("throw new TypeError('different');", true),
            ("return new TypeError('message');", false),
            ("throw TypeError('message');", false),
            ("throw new TypeError(sideEffect());", false),
            ("sideEffect(); throw new TypeError('message');", false),
        ] {
            let source = format!(
                "function main(){{function $THREAD_1_2(){{{body}}}return $THREAD_1_2;}}main();"
            );
            let result = crate::naming::name_threads(&source, true).unwrap();
            let outcome = result
                .report.outcomes
                .iter()
                .find(|o| o.binding.name == "$THREAD_1_2")
                .unwrap();
            if expected { assert!(!outcome.evidence.is_empty(), "whole-body evidence required"); }
            assert_eq!(
                outcome.evaluator.as_deref() == Some("operations_throw_type_error_message"),
                expected,
                "{body}"
            );
        }
        let result = crate::naming::name_threads("function main(TypeError){function $THREAD_1_2(){throw new TypeError('message');}return $THREAD_1_2;}main(Other);", true).unwrap();
        assert!(result
            .report.outcomes
            .iter()
            .all(|o| o.evaluator.as_deref() != Some("operations_throw_type_error_message")));
    }
}
