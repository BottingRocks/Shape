//! Caught-error listener scheduler with an explicit parent/child contract.
//! The timer arrow is inspected only as the exact argument of the owned call;
//! the error's origin and the helper's registration are resolved in its catch.
use super::*;
use oxc_span::GetSpan;

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e.without_parentheses(), model.scoping)
}
fn stable(model: &Model<'_, '_>, s: SymbolId) -> bool {
    model.binding_writes(s) == 0 && model.scoping.symbol_redeclarations(s).is_empty()
}
fn expr<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    match s {
        Statement::ExpressionStatement(s) => Some(&s.expression),
        _ => None,
    }
}
fn call<'a>(e: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    match e.without_parentheses() {
        Expression::CallExpression(c) if !c.optional => Some(c),
        _ => None,
    }
}
fn assignment<'a>(s: &'a Statement<'a>) -> Option<&'a AssignmentExpression<'a>> {
    match expr(s)?.without_parentheses() {
        Expression::AssignmentExpression(a) if a.operator == AssignmentOperator::Assign => Some(a),
        _ => None,
    }
}
fn target(model: &Model<'_, '_>, a: &AssignmentExpression<'_>) -> Option<SymbolId> {
    let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
        return None;
    };
    crate::analysis::identifier_symbol(id, model.scoping)
}

// A plain variable is DATA here, never evidence for the identity of a callee.
fn variable<'a>(model: &Model<'_, 'a>, s: SymbolId) -> Option<&'a VariableDeclarator<'a>> {
    let AstKind::VariableDeclarator(v) = model.nodes.kind(model.scoping.symbol_declaration(s))
    else {
        return None;
    };
    (matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
        && model.scoping.symbol_redeclarations(s).is_empty()
        && matches!(
            model.nodes.parent_kind(model.scoping.symbol_declaration(s)),
            AstKind::VariableDeclaration(_)
        ))
    .then_some(v)
}

fn caught_registration(model: &Model<'_, '_>, site: FunctionSite, error: SymbolId) -> Option<()> {
    let own = site.binding_symbol?;
    if !stable(model, own) || model.binding_references(own) != 1 {
        return None;
    }
    let block_id = model.nodes.parent_id(site.node_id);
    let AstKind::BlockStatement(block) = model.nodes.kind(block_id) else {
        return None;
    };
    let AstKind::CatchClause(handler) = model.nodes.parent_kind(block_id) else {
        return None;
    };
    let BindingPattern::BindingIdentifier(caught) = &handler.param.as_ref()?.pattern else {
        return None;
    };
    let caught = caught.symbol_id.get()?;
    if !stable(model, caught) || caught == error {
        return None;
    }
    let owner = model.owning_site(block_id)?;
    let declaration = model.scoping.symbol_declaration(error);
    let v = variable(model, error)?;
    if v.init.is_some()
        || v.span.start >= handler.span.start
        || model.owning_site(declaration)?.node_id != owner.node_id
        || model.binding_writes(error) != 1
    {
        return None;
    }
    let index = block.body.iter().position(|s| s.span() == site.span)?;
    if index == 0 {
        return None;
    }
    // The first store pins the caught value, before the declaration and call.
    let store = assignment(block.body.first()?)?;
    if target(model, store) != Some(error) || symbol(model, &store.right) != Some(caught) {
        return None;
    }
    // Accept only the bare store or the exact intervening latch/flags/clear
    // grammar. No calls, branches, nested donors or mutations of the error.
    match &block.body[1..index] {
        [] => {}
        [latch, failed, running, clear] => {
            let latch = assignment(latch)?;
            let failed = assignment(failed)?;
            let running = assignment(running)?;
            let clear = assignment(clear)?;
            let latch_slot = target(model, latch)?;
            let failed_slot = target(model, failed)?;
            let running_slot = target(model, running)?;
            let AssignmentTarget::StaticMemberExpression(length) = &clear.left else {
                return None;
            };
            let queue = symbol(model, &length.object)?;
            let slots = [error, latch_slot, failed_slot, running_slot, queue];
            if slots.iter().copied().collect::<BTreeSet<_>>().len() != slots.len()
                || slots.contains(&caught)
                || slots.iter().any(|s| variable(model, *s).is_none())
                || symbol(model, &latch.right) != Some(caught)
                || !matches!(failed.right.without_parentheses(), Expression::BooleanLiteral(b) if b.value)
                || !matches!(running.right.without_parentheses(), Expression::BooleanLiteral(b) if !b.value)
                || length.optional
                || length.property.name != "length"
                || !matches!(clear.right.without_parentheses(), Expression::NumericLiteral(n) if n.value == 0.0)
            {
                return None;
            }
        }
        _ => return None,
    }
    let registration = call(expr(block.body.get(index + 1)?)?)?;
    let Expression::StaticMemberExpression(each) = registration.callee.without_parentheses() else {
        return None;
    };
    let listeners = symbol(model, &each.object)?;
    variable(model, listeners)?;
    let args = call_arguments(registration);
    (each.property.name == "forEach"
        && !each.optional
        && listeners != error
        && !site
            .span
            .contains_inclusive(model.scoping.symbol_span(listeners))
        && registration.arguments.len() == 1
        && args.len() == 1
        && symbol(model, args[0]) == Some(own))
    .then_some(())
}

fn contract(model: &Model<'_, '_>, site: FunctionSite) -> Option<()> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return None;
    };
    let body = function.body.as_ref()?;
    let [p] = function.params.items.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(p_id) = &p.pattern else {
        return None;
    };
    let parameter = p_id.symbol_id.get()?;
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || p.initializer.is_some()
        || !body.directives.is_empty()
        || !stable(model, parameter)
    {
        return None;
    }
    let [Statement::VariableDeclaration(alias), schedule] = body.statements.as_slice() else {
        return None;
    };
    let [alias] = alias.declarations.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(alias_id) = &alias.id else {
        return None;
    };
    let alias_symbol = alias_id.symbol_id.get()?;
    if alias_symbol == parameter
        || !stable(model, alias_symbol)
        || symbol(model, alias.init.as_ref()?) != Some(parameter)
        || model.binding_references(parameter) != 1
        || model.binding_references(alias_symbol) != 1
    {
        return None;
    }
    let schedule = call(expr(schedule)?)?;
    let Expression::Identifier(timer) = schedule.callee.without_parentheses() else {
        return None;
    };
    let args = call_arguments(schedule);
    if timer.name != "setTimeout"
        || symbol(model, &schedule.callee).is_some()
        || schedule.arguments.len() != 2
        || args.len() != 2
        || !matches!(args[1].without_parentheses(), Expression::NumericLiteral(n) if n.value == 0.0)
    {
        return None;
    }
    let Expression::ArrowFunctionExpression(thunk) = args[0].without_parentheses() else {
        return None;
    };
    if thunk.r#async
        || !thunk.expression
        || !thunk.params.items.is_empty()
        || thunk.params.rest.is_some()
        || !thunk.body.directives.is_empty()
    {
        return None;
    }
    let [invoke] = thunk.body.statements.as_slice() else {
        return None;
    };
    let invoke = call(expr(invoke)?)?;
    let args = call_arguments(invoke);
    if symbol(model, &invoke.callee) != Some(alias_symbol)
        || invoke.arguments.len() != 1
        || args.len() != 1
    {
        return None;
    }
    caught_registration(model, site, symbol(model, args[0])?)
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
) -> Vec<Proposal> {
    if contract(model, site).is_some() {
        vec![proposal("scheduleErrorListener", "shared_caught_error_listener",
            "exact owned alias plus global zero-delay timer arrow invokes the unchanged sole formal with a captured error; the immediately enclosing catch stores its unchanged caught binding into that single-write parent local before this declaration, followed immediately by a resolved forEach registration of this helper", facts)]
    } else {
        vec![]
    }
}
