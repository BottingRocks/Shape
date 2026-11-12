//! Bounded scheduler rules for the owned task queue.
//!
//! These rules prove the small queue-control methods by exact statement shape
//! and scoped identities.  They deliberately recognize the direct standard
//! `setTimeout` builtin only when it is unresolved (therefore not a parameter,
//! catch binding, loop target, or local shadow); aliases and arbitrary host
//! namespaces are refused.
use super::*;
use oxc_semantic::SymbolId;

type Symbol = SymbolId;

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(e, model.scoping)
}

fn plain_formal(model: &Model<'_, '_>, p: &BindingPattern<'_>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(id) = p else {
        return None;
    };
    let s = id.symbol_id.get()?;
    (model.binding_writes(s) == 0 && model.scoping.symbol_redeclarations(s).is_empty()).then_some(s)
}

// A captured queue slot is DATA, not a callable origin.  In particular this
// excludes parameters, catches, destructuring and loop bindings even when the
// resolved symbol happens to have the expected operations.
fn captured_data(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<Symbol> {
    let s = symbol(model, e)?;
    captured_symbol(model, site, s)
}

fn captured_symbol(model: &Model<'_, '_>, site: FunctionSite, s: Symbol) -> Option<Symbol> {
    if site.span.contains_inclusive(model.scoping.symbol_span(s))
        || !model.scoping.symbol_redeclarations(s).is_empty()
    {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return None;
    };
    (matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
        && matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(d)
                if matches!(
                    d.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                )
        )
        && !matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
        && model.scoping.symbol_span(s).end < site.span.start)
        .then_some(s)
}

fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = e else {
        return None;
    };
    (!m.optional && m.property.name == key).then_some(&m.object)
}

fn call<'a>(s: &'a Statement<'a>) -> Option<&'a CallExpression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    let Expression::CallExpression(c) = &s.expression else {
        return None;
    };
    (!c.optional).then_some(c)
}

fn number(e: &Expression<'_>, value: f64) -> bool {
    matches!(e, Expression::NumericLiteral(n) if n.value == value)
}

fn assignment<'a>(s: &'a Statement<'a>) -> Option<&'a AssignmentExpression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    let Expression::AssignmentExpression(a) = &s.expression else {
        return None;
    };
    (a.operator.as_str() == "=").then_some(a)
}

fn timeout_with_model(
    model: &Model<'_, '_>,
    c: &CallExpression<'_>,
    callback: Option<Symbol>,
) -> bool {
    if c.optional || c.arguments.len() != 2 || call_arguments(c).len() != 2 {
        return false;
    }
    let Expression::Identifier(id) = &c.callee else {
        return false;
    };
    id.name == "setTimeout"
        && crate::analysis::identifier_symbol(id, model.scoping).is_none()
        && callback.is_none_or(|s| symbol(model, call_arguments(c)[0]) == Some(s))
        && number(call_arguments(c)[1], 0.0)
}

fn for_each_timeout(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    callbacks: Symbol,
) -> Option<()> {
    let each = call(statement)?;
    let receiver = member(&each.callee, "forEach")?;
    if captured_data(model, site, receiver)? != callbacks || each.arguments.len() != 1 {
        return None;
    }
    let [Argument::ArrowFunctionExpression(callback)] = each.arguments.as_slice() else {
        return None;
    };
    if callback.r#async || callback.expression || callback.params.rest.is_some() {
        return None;
    }
    let [parameter] = callback.params.items.as_slice() else {
        return None;
    };
    if parameter.initializer.is_some() {
        return None;
    }
    let item = plain_formal(model, &parameter.pattern)?;
    let [statement] = callback.body.statements.as_slice() else {
        return None;
    };
    let timer = call(statement)?;
    timeout_with_model(model, timer, Some(item)).then_some(())
}

fn clear_queue(model: &Model<'_, '_>, site: FunctionSite, body: &FunctionBody<'_>) -> Option<()> {
    let [Statement::IfStatement(branch)] = body.statements.as_slice() else {
        return None;
    };
    if branch.alternate.is_some() {
        return None;
    }
    let Expression::BinaryExpression(test) = &branch.test else {
        return None;
    };
    if test.operator.as_str() != ">" || !number(&test.right, 0.0) {
        return None;
    }
    let queue = captured_data(model, site, member(&test.left, "length")?)?;
    let Statement::BlockStatement(block) = &branch.consequent else {
        return None;
    };
    let [clear, stopped, callbacks] = block.body.as_slice() else {
        return None;
    };
    let clear = assignment(clear)?;
    let AssignmentTarget::StaticMemberExpression(left) = &clear.left else {
        return None;
    };
    if left.optional
        || left.property.name != "length"
        || symbol(model, &left.object) != Some(queue)
        || !number(&clear.right, 0.0)
    {
        return None;
    }
    let stopped = assignment(stopped)?;
    let AssignmentTarget::AssignmentTargetIdentifier(id) = &stopped.left else {
        return None;
    };
    if !matches!(&stopped.right, Expression::BooleanLiteral(b) if !b.value) {
        return None;
    }
    let running = captured_symbol(
        model,
        site,
        crate::analysis::identifier_symbol(id, model.scoping)?,
    )?;
    if running == queue {
        return None;
    }
    let callback_statement = callbacks;
    let callback_call = call(callback_statement)?;
    let callbacks = captured_data(model, site, member(&callback_call.callee, "forEach")?)?;
    if callbacks == queue || callbacks == running {
        return None;
    }
    for_each_timeout(model, site, callback_statement, callbacks)
}

fn queue_empty_notification(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [parameter] = function.params.items.as_slice() else {
        return None;
    };
    let item = plain_formal(model, &parameter.pattern)?;
    let [push, branch] = body.statements.as_slice() else {
        return None;
    };
    let push = call(push)?;
    let callbacks = captured_data(model, site, member(&push.callee, "push")?)?;
    if push.arguments.len() != 1
        || call_arguments(push).len() != 1
        || symbol(model, call_arguments(push)[0]) != Some(item)
    {
        return None;
    }
    let Statement::IfStatement(branch) = branch else {
        return None;
    };
    if branch.alternate.is_some() {
        return None;
    }
    let Expression::LogicalExpression(and) = &branch.test else {
        return None;
    };
    if and.operator.as_str() != "&&" {
        return None;
    }
    let running = captured_symbol(model, site, symbol(model, &and.left)?)?;
    let Expression::BinaryExpression(empty) = &and.right else {
        return None;
    };
    if empty.operator.as_str() != "===" || !number(&empty.right, 0.0) {
        return None;
    }
    let queue = captured_data(model, site, member(&empty.left, "length")?)?;
    if queue == callbacks || running == callbacks || running == queue {
        return None;
    }
    let Statement::BlockStatement(block) = &branch.consequent else {
        return None;
    };
    let [schedule] = block.body.as_slice() else {
        return None;
    };
    let schedule = call(schedule)?;
    timeout_with_model(model, schedule, Some(item)).then_some(())
}

fn local_alias(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    source: Symbol,
) -> Option<Symbol> {
    let Statement::VariableDeclaration(declaration) = statement else {
        return None;
    };
    let [variable] = declaration.declarations.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(id) = &variable.id else {
        return None;
    };
    let local = id.symbol_id.get()?;
    if !site
        .span
        .contains_inclusive(model.scoping.symbol_span(local))
        || model.binding_writes(local) != 0
        || !model.scoping.symbol_redeclarations(local).is_empty()
        || !matches!(
            model.nodes.parent_kind(model.scoping.symbol_declaration(local)),
            AstKind::VariableDeclaration(d)
                if matches!(
                    d.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                )
        )
        || symbol(model, variable.init.as_ref()?) != Some(source)
    {
        return None;
    }
    Some(local)
}

fn void_return(statement: &Statement<'_>) -> bool {
    matches!(statement,
        Statement::ReturnStatement(r)
            if matches!(r.argument.as_ref(), Some(Expression::UnaryExpression(u))
                if u.operator.as_str() == "void"
                    && matches!(&u.argument, Expression::NumericLiteral(n) if n.value == 0.0)))
}

fn queue_error_notification(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [parameter] = function.params.items.as_slice() else {
        return None;
    };
    let item = plain_formal(model, &parameter.pattern)?;
    let [alias_statement, Statement::IfStatement(branch)] = body.statements.as_slice() else {
        return None;
    };
    let callback = local_alias(model, site, alias_statement, item)?;
    let error_flag = captured_data(model, site, &branch.test)?;
    let Statement::BlockStatement(consequent) = &branch.consequent else {
        return None;
    };
    let [schedule_statement] = consequent.body.as_slice() else {
        return None;
    };
    let schedule = call(schedule_statement)?;
    if !timeout_with_model(model, schedule, None) {
        return None;
    }
    let [Argument::ArrowFunctionExpression(callback_body), _] = schedule.arguments.as_slice()
    else {
        return None;
    };
    if callback_body.r#async
        || callback_body.expression
        || callback_body.params.rest.is_some()
        || !callback_body.params.items.is_empty()
        || !callback_body.body.directives.is_empty()
    {
        return None;
    }
    let [invoke_statement] = callback_body.body.statements.as_slice() else {
        return None;
    };
    let invoke = call(invoke_statement)?;
    let error_value = call_arguments(invoke)
        .first()
        .and_then(|arg| symbol(model, arg))
        .and_then(|s| captured_symbol(model, site, s));
    if invoke.arguments.len() != 1
        || call_arguments(invoke).len() != 1
        || symbol(model, &invoke.callee) != Some(callback)
        || error_value.is_none()
        || error_value == Some(error_flag)
    {
        return None;
    }
    let Some(alternate_statement) = branch.alternate.as_ref() else {
        return None;
    };
    let Statement::BlockStatement(alternate) = alternate_statement else {
        return None;
    };
    let [pending_statement] = alternate.body.as_slice() else {
        return None;
    };
    let pending = call(pending_statement)?;
    let pending_receiver = member(&pending.callee, "push")?;
    let pending_symbol = captured_data(model, site, pending_receiver)?;
    if pending_symbol == error_flag
        || pending.arguments.len() != 1
        || call_arguments(pending).len() != 1
        || symbol(model, call_arguments(pending)[0]) != Some(callback)
    {
        return None;
    }
    Some(())
}

fn start_queue(model: &Model<'_, '_>, site: FunctionSite, body: &FunctionBody<'_>) -> Option<()> {
    let [Statement::IfStatement(branch)] = body.statements.as_slice() else {
        return None;
    };
    let running = captured_data(model, site, &branch.test)?;
    let Statement::BlockStatement(guard) = &branch.consequent else {
        return None;
    };
    let [guard_return] = guard.body.as_slice() else {
        return None;
    };
    if !void_return(guard_return) {
        return None;
    }
    let Statement::BlockStatement(alternate) = branch.alternate.as_ref()? else {
        return None;
    };
    let [mark, schedule, done] = alternate.body.as_slice() else {
        return None;
    };
    let mark = assignment(mark)?;
    let AssignmentTarget::AssignmentTargetIdentifier(id) = &mark.left else {
        return None;
    };
    if crate::analysis::identifier_symbol(id, model.scoping) != Some(running)
        || !matches!(&mark.right, Expression::BooleanLiteral(b) if b.value)
    {
        return None;
    }
    let schedule = call(schedule)?;
    if !timeout_with_model(model, schedule, None) || !void_return(done) {
        return None;
    }
    let schedule_arguments = call_arguments(schedule);
    let [argument, _delay] = schedule_arguments.as_slice() else {
        return None;
    };
    if crate::naming::resolve::ordered_callable(model, argument, 0).is_none() {
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
        return vec![];
    };
    let Some(body) = function.body.as_ref() else {
        return vec![];
    };
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || function
            .params
            .items
            .iter()
            .any(|p| p.initializer.is_some())
    {
        return vec![];
    }
    let mut claims = Vec::new();
    if dispatch_callback_list(model, site, function, body).is_some() {
        claims.push(proposal("scheduleCapturedCallbacksWithArgument", "scheduling_callback_list_dispatch", "whole owned body: copy formal provider, store second formal in captured data slot, then forEach captured callback list and call the same provider setTimeout(item,0,slot); no extra effects, changed bindings, alternate receivers or arguments; provider, callback and payload implementations remain opaque", f));
    }
    if clear_queue(model, site, body).is_some() {
        claims.push(proposal(
            "clearQueuedTasks",
            "scheduling_task_queue",
            "clears the captured queue, resets its running flag, and schedules each captured empty-queue callback at zero delay",
            f,
        ));
    }
    if queue_empty_notification(model, site, function, body).is_some() {
        claims.push(proposal(
            "notifyQueueEmpty",
            "scheduling_task_queue",
            "registers an exact callback and schedules it at zero delay only when the captured queue is empty while running",
            f,
        ));
    }
    if queue_error_notification(model, site, function, body).is_some() {
        claims.push(proposal(
            "notifyQueueError",
            "scheduling_task_queue",
            "dispatches the captured error to the exact callback at zero delay after failure, otherwise queues that callback",
            f,
        ));
    }
    if start_queue(model, site, body).is_some() {
        claims.push(proposal(
            "startTaskQueue",
            "scheduling_task_queue",
            "starts the captured one-shot task-queue gate: returns void while the captured running flag is set, otherwise marks it started and schedules the captured ordered callable at zero delay",
            f,
        ));
    }
    claims
}


// A formal provider stays opaque. This proves precisely its setTimeout method
// protocol and the store-before-iteration of the shared payload, not a native
// timer identity or a claim about captured callback implementations.
fn dispatch_callback_list(
    model: &Model<'_, '_>, site: FunctionSite,
    function: &Function<'_>, body: &FunctionBody<'_>,
) -> Option<()> {
    if !body.directives.is_empty() { return None; }
    let [provider,payload] = function.params.items.as_slice() else {return None;};
    let provider=plain_formal(model,&provider.pattern)?;
    let payload=plain_formal(model,&payload.pattern)?;
    let [copy,store,iterate] = body.statements.as_slice() else {return None;};
    let receiver=local_alias(model,site,copy,provider)?;
    let store=assignment(store)?;
    let AssignmentTarget::AssignmentTargetIdentifier(target)=&store.left else {return None;};
    let slot=captured_symbol(model,site,crate::analysis::identifier_symbol(target,model.scoping)?)?;
    if symbol(model,&store.right)!=Some(payload) {return None;}
    let iterate=call(iterate)?;
    let callbacks=captured_data(model,site,member(&iterate.callee,"forEach")?)?;
    if callbacks==slot {return None;}
    let [Argument::ArrowFunctionExpression(callback)] = iterate.arguments.as_slice() else {return None;};
    if callback.r#async || callback.expression || callback.params.rest.is_some() || !callback.body.directives.is_empty() {return None;}
    let [item]=callback.params.items.as_slice() else {return None;};
    if item.initializer.is_some() {return None;}
    let item=plain_formal(model,&item.pattern)?;
    let [statement]=callback.body.statements.as_slice() else {return None;};
    let schedule=call(statement)?;
    if symbol(model,member(&schedule.callee,"setTimeout")?)!=Some(receiver) {return None;}
    let arguments=call_arguments(schedule);
    let [value,delay,argument]=arguments.as_slice() else {return None;};
    (schedule.arguments.len()==3 && symbol(model,value)==Some(item) && number(delay,0.0) && symbol(model,argument)==Some(slot)).then_some(())
}
