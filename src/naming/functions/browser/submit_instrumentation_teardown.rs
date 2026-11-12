use super::*;
use oxc_span::GetSpan;

use crate::naming::support::seeded_flow::*;

#[path = "submit_instrumentation_cleanup.rs"]
mod guarded_cleanup;

fn double_not<'a>(e: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    let Expression::UnaryExpression(outer) = e else {
        return None;
    };
    if outer.operator.as_str() != "!" {
        return None;
    }
    let Expression::UnaryExpression(inner) = &outer.argument else {
        return None;
    };
    (inner.operator.as_str() == "!").then_some(&inner.argument)
}

fn truth_test(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Symbol> {
    if let Some(inner) = double_not(e) {
        return symbol(model, inner);
    }
    symbol(model, e)
}

// ---------------------------------------------------------------------------
// `teardownSubmitInstrumentation`: ordered listener/registry cleanup.
// ---------------------------------------------------------------------------
fn captured_handler(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> bool {
    data_capture(model, site, e).is_some() || callable_capture(model, site, e).is_some()
}

fn listener_removal(
    model: &Model<'_, '_>,
    site: FunctionSite,
    s: &Statement<'_>,
    event: &str,
) -> Option<bool> {
    let c = statement_call(s)?;
    let args = arity(c, 2)?;
    let receiver = member(&c.callee, "removeEventListener")?;
    let path = global_path(model, receiver);
    if !string(args[0], event)
        || !captured_handler(model, site, args[1])
        || !(data_capture(model, site, receiver).is_some()
            || path == ["window", "document"]
            || path == ["document"]
            || path == ["window", "navigation"])
    {
        return None;
    }
    Some(path == ["window", "navigation"])
}

fn teardown_submit_instrumentation(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [Statement::TryStatement(t)] = body.statements.as_slice() else {
        return None;
    };
    if t.finalizer.is_some() {
        return None;
    }
    let handler = t.handler.as_ref()?;
    let BindingPattern::BindingIdentifier(error) = &handler.param.as_ref()?.pattern else {
        return None;
    };
    let error = error.symbol_id.get()?;
    if !handler.body.body.is_empty() || !unchanged(model, error) {
        return None;
    }
    let [click, Statement::IfStatement(conditional), Statement::IfStatement(head)] =
        t.block.body.as_slice()
    else {
        return None;
    };
    let click = statement_call(click)?;
    let click_args = arity(click, 2)?;
    let receiver_expr = member(&click.callee, "removeEventListener")?;
    let receiver = data_capture(model, site, receiver_expr)?;
    if !string(click_args[0], "click") || !captured_handler(model, site, click_args[1]) {
        return None;
    }
    let flag = truth_test(model, &conditional.test)?;
    if !capture_identity(model, site, flag, conditional.test.span().start) {
        return None;
    }
    let Statement::BlockStatement(cleanup) = &conditional.consequent else {
        return None;
    };
    let (last, clears) = cleanup.body.split_last()?;
    if clears.len() < 2 || clears.len() > 6 {
        return None;
    }
    let mut registries = BTreeSet::new();
    let mut seen_submit = false;
    let mut seen_navigate = false;
    for statement in clears {
        let Some(c) = statement_call(statement) else {
            return None;
        };
        if let Some(object) = member(&c.callee, "clear") {
            if seen_submit || seen_navigate || arity(c, 0).is_none() {
                return None;
            }
            let registry = data_capture(model, site, object)?;
            if !registries.insert(registry) {
                return None;
            }
            continue;
        }
        if let Some(navigate) = listener_removal(model, site, statement, "submit") {
            if navigate || seen_submit {
                return None;
            }
            seen_submit = true;
        } else if listener_removal(model, site, statement, "navigate").is_some() {
            if seen_navigate {
                return None;
            }
            seen_navigate = true;
        } else {
            return None;
        }
    }
    let flag_write = assignment(last)?;
    if assign_symbol(model, flag_write) != Some(flag)
        || !matches!(&flag_write.right, Expression::BooleanLiteral(b) if !b.value)
    {
        return None;
    }
    let head_receiver = member(&head.test, "head")?;
    if symbol(model, head_receiver) != Some(receiver) {
        return None;
    }
    let Statement::BlockStatement(head_body) = &head.consequent else {
        return None;
    };
    let [remove] = head_body.body.as_slice() else {
        return None;
    };
    let remove = statement_call(remove)?;
    let Some(parent) = member(&remove.callee, "removeChild") else {
        return None;
    };
    let Some(owner) = member(parent, "head") else {
        return None;
    };
    (symbol(model, owner) == Some(receiver)
        && arity(remove, 1)
            .and_then(|args| args.first().and_then(|e| data_capture(model, site, e)))
            .is_some())
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
    if function.params.items.is_empty()
        && (teardown_submit_instrumentation(model, site, body).is_some()
            || guarded_cleanup::accepts(model, site))
    {
        claims.push(proposal(
            "teardownSubmitInstrumentation",
            "browser_submit_instrumentation_teardown",
            "try/catch teardown: exact captured click listener removal, truth-tested captured flag, distinct registry clears with optional submit/navigate listener removals, flag reset false, and exact captured head child removal; empty catch only",
            f,
        ));
    }
    claims
}

// Keep the residual-family diagnostic local. It deliberately does not extend
// naming until capture initialization and all publication epochs are proved.
#[cfg(test)]
#[path = "submit_instrumentation_cleanup_tests.rs"]
mod submit_instrumentation_cleanup_tests;
