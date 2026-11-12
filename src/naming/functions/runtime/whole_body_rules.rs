//! Captured callable and complete whole-body rules.
//!
//! Every rule here proves one complete own function body from the single Oxc
//! parse. Captured identities are plain-origin declarations that complete
//! outside the function body and before the exact use; a generated spelling,
//! source position, vendor, hash or sibling label is never evidence. Literal
//! values, operator order, side-effect order and discarded/returned values are
//! preserved as observable behavior. Native method/global results stay opaque.
use crate::naming::shared::*;
use crate::naming::support::installer_events::*;
use crate::naming::support::seeded_flow::{
    arity, assign_symbol, assignment, callable_capture, capture_identity, data_capture,
    directives,
};
use crate::naming::*;
use oxc_span::GetSpan;

fn binary<'a>(
    e: &'a Expression<'a>,
    op: &str,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::BinaryExpression(b) = e.without_parentheses() else {
        return None;
    };
    (b.operator.as_str() == op).then_some((&b.left, &b.right))
}

fn sole_formal(model: &Model<'_, '_>, function: &Function<'_>) -> Option<Symbol> {
    if function.params.rest.is_some() {
        return None;
    }
    let [parameter] = function.params.items.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(id) = &parameter.pattern else {
        return None;
    };
    let formal = id.symbol_id.get()?;
    (parameter.initializer.is_none() && unchanged(model, formal)).then_some(formal)
}

fn logical<'a>(
    e: &'a Expression<'a>,
    op: &str,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::LogicalExpression(b) = e.without_parentheses() else {
        return None;
    };
    (b.operator.as_str() == op).then_some((&b.left, &b.right))
}

fn is_declaration(function: &Function<'_>) -> bool {
    function.r#type == FunctionType::FunctionDeclaration
        && !function.r#async
        && !function.generator
        && function.params.rest.is_none()
}

fn bound_symbol(model: &Model<'_, '_>, function: &Function<'_>) -> Option<Symbol> {
    function.id.as_ref()?.symbol_id.get()
}

fn null_symbol(model: &Model<'_, '_>, e: &Expression<'_>, op: &str) -> Option<Symbol> {
    let (left, right) = binary(e, op)?;
    matches!(right.without_parentheses(), Expression::NullLiteral(_))
        .then(|| symbol(model, left))
        .flatten()
}

// `k();` on a captured reference: the reference is evaluated, the call is made
// with no arguments and `undefined` receiver, and the result is discarded.
fn call_captured_discard(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if !is_declaration(function) || !function.params.items.is_empty() || !directives(body) {
        return None;
    }
    let [Statement::ExpressionStatement(statement)] = body.statements.as_slice() else {
        return None;
    };
    let invoked = call(&statement.expression)?;
    if !matches!(
        invoked.callee.without_parentheses(),
        Expression::Identifier(_)
    ) || arity(invoked, 0).is_none()
    {
        return None;
    }
    if data_capture(model, site, &invoked.callee).is_none()
        && callable_capture(model, site, &invoked.callee).is_none()
    {
        return None;
    }
    Some(())
}

// `return k0(formal0, captured);` on one immutable formal.
fn forward_formal_and_capture(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if !is_declaration(function) || !directives(body) {
        return None;
    }
    let formal = sole_formal(model, function)?;
    if model.binding_references(formal) != 1 {
        return None;
    }
    let [Statement::ReturnStatement(result)] = body.statements.as_slice() else {
        return None;
    };
    let returned = call(result.argument.as_ref()?)?;
    let arguments = arity(returned, 2)?;
    if symbol(model, arguments[0]) != Some(formal) {
        return None;
    }
    let captured = data_capture(model, site, arguments[1])?;
    if symbol(model, &returned.callee) == Some(captured)
        || (data_capture(model, site, &returned.callee).is_none()
            && callable_capture(model, site, &returned.callee).is_none())
    {
        return None;
    }
    Some(())
}

// `captured.push(formal0);` on one immutable formal.
fn append_formal_to_capture(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if !is_declaration(function) || !directives(body) {
        return None;
    }
    let formal = sole_formal(model, function)?;
    if model.binding_references(formal) != 1 {
        return None;
    }
    let [Statement::ExpressionStatement(statement)] = body.statements.as_slice() else {
        return None;
    };
    let invoked = call(&statement.expression)?;
    let receiver = member(&invoked.callee, "push")?;
    data_capture(model, site, receiver)?;
    let arguments = arity(invoked, 1)?;
    (symbol(model, arguments[0]) == Some(formal)).then_some(())
}

// `try { formal(); } catch (e) { const x = e; captured.forEach(o => o.error(x)); }`
fn invoke_and_report_caught_error(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if !is_declaration(function) || !directives(body) {
        return None;
    }
    let formal = sole_formal(model, function)?;
    if model.binding_references(formal) != 1 {
        return None;
    }
    let [Statement::TryStatement(attempt)] = body.statements.as_slice() else {
        return None;
    };
    if attempt.finalizer.is_some() {
        return None;
    }
    let [Statement::ExpressionStatement(invoke)] = attempt.block.body.as_slice() else {
        return None;
    };
    let invoke = call(&invoke.expression)?;
    if symbol(model, &invoke.callee) != Some(formal) || arity(invoke, 0).is_none() {
        return None;
    }
    let handler = attempt.handler.as_ref()?;
    let BindingPattern::BindingIdentifier(error) = &handler.param.as_ref()?.pattern else {
        return None;
    };
    let error = error.symbol_id.get()?;
    if model.binding_writes(error) != 0 || !model.scoping.symbol_redeclarations(error).is_empty() {
        return None;
    }
    let [Statement::VariableDeclaration(declaration), Statement::ExpressionStatement(fanout)] =
        handler.body.body.as_slice()
    else {
        return None;
    };
    let [variable] = declaration.declarations.as_slice() else {
        return None;
    };
    let local = plain_local(model, variable)?;
    if symbol(model, variable.init.as_ref()?) != Some(error)
        || model.binding_writes(local) != 0
        || model.binding_references(local) != 1
    {
        return None;
    }
    let fanout = call(&fanout.expression)?;
    let receiver = member(&fanout.callee, "forEach")?;
    data_capture(model, site, receiver)?;
    let [Argument::ArrowFunctionExpression(arrow)] = fanout.arguments.as_slice() else {
        return None;
    };
    if arrow.r#async || arrow.params.rest.is_some() || !arrow.body.directives.is_empty() {
        return None;
    }
    let [parameter] = arrow.params.items.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(observer) = &parameter.pattern else {
        return None;
    };
    let observer = observer.symbol_id.get()?;
    if parameter.initializer.is_some() || model.binding_writes(observer) != 0 {
        return None;
    }
    let [Statement::ExpressionStatement(reported)] = arrow.body.statements.as_slice() else {
        return None;
    };
    let reported = call(&reported.expression)?;
    let target = member(&reported.callee, "error")?;
    if symbol(model, target) != Some(observer) {
        return None;
    }
    let arguments = arity(reported, 1)?;
    (symbol(model, arguments[0]) == Some(local)).then_some(())
}

// `a = sentinel; b = sentinel; if (disposer != null) disposer();`
fn reset_slots_and_dispose(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if !is_declaration(function) || !function.params.items.is_empty() || !directives(body) {
        return None;
    }
    let [first, second, Statement::IfStatement(guard)] = body.statements.as_slice() else {
        return None;
    };
    let first = assignment(first)?;
    let second = assignment(second)?;
    let first_slot = assign_symbol(model, first)?;
    let second_slot = assign_symbol(model, second)?;
    let sentinel = symbol(model, &first.right)?;
    if symbol(model, &second.right) != Some(sentinel)
        || first_slot == second_slot
        || first_slot == sentinel
        || second_slot == sentinel
    {
        return None;
    }
    if !capture_identity(model, site, first_slot, first.span.start)
        || !capture_identity(model, site, second_slot, second.span.start)
        || !capture_identity(model, site, sentinel, first.span.start)
    {
        return None;
    }
    if guard.alternate.is_some() {
        return None;
    }
    let disposer = null_symbol(model, &guard.test, "!=")?;
    if disposer == first_slot
        || disposer == second_slot
        || disposer == sentinel
        || !capture_identity(model, site, disposer, guard.test.span().start)
    {
        return None;
    }
    let [Statement::ExpressionStatement(invoke)] = block(&guard.consequent)? else {
        return None;
    };
    let invoke = call(&invoke.expression)?;
    if symbol(model, &invoke.callee) != Some(disposer) || arity(invoke, 0).is_none() {
        return None;
    }
    Some(())
}

// `throw new TypeError("Cannot read properties of undefined (reading 'value')");`
fn throw_undefined_value_type_error(
    model: &Model<'_, '_>,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<Vec<SourceSpan>> {
    if !is_declaration(function) || !function.params.items.is_empty() || !directives(body) {
        return None;
    }
    let [Statement::ThrowStatement(thrown)] = body.statements.as_slice() else {
        return None;
    };
    let Expression::NewExpression(error) = &thrown.argument else {
        return None;
    };
    if !matches!(&error.callee, Expression::Identifier(_))
        || !global_path_ends_with(model, &error.callee, &["TypeError"])
    {
        return None;
    }
    let [Argument::StringLiteral(message)] = error.arguments.as_slice() else {
        return None;
    };
    (message.value == "Cannot read properties of undefined (reading 'value')").then(|| vec![
        thrown.span.into(), error.span.into(), error.callee.span().into(), message.span.into(),
    ])
}

fn expression_assignment<'a>(
    statement: &'a ExpressionStatement<'a>,
) -> Option<&'a AssignmentExpression<'a>> {
    let Expression::AssignmentExpression(assign) = &statement.expression else {
        return None;
    };
    (assign.operator.as_str() == "=").then_some(assign)
}

// `typeof formal === "function" && formal.toString === Function.prototype.toString`
// gates `!!captured.exec(formal.toString())`; the alternate arm returns false.
fn check_native_function_source(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if !is_declaration(function) || !directives(body) {
        return None;
    }
    let formal = sole_formal(model, function)?;
    if model.binding_references(formal) != 3 {
        return None;
    }
    let [Statement::IfStatement(branch)] = body.statements.as_slice() else {
        return None;
    };
    let (type_check, identity) = logical(&branch.test, "&&")?;
    let (typeof_check, literal) = binary(type_check, "===")?;
    let Expression::UnaryExpression(typeof_check) = typeof_check.without_parentheses() else {
        return None;
    };
    if typeof_check.operator.as_str() != "typeof"
        || symbol(model, &typeof_check.argument) != Some(formal)
        || !string(literal, "function")
    {
        return None;
    }
    let (receiver, native) = binary(identity, "===")?;
    let source = member(receiver, "toString")?;
    if symbol(model, source) != Some(formal)
        || !global_path_ends_with(model, native, &["Function", "prototype", "toString"])
    {
        return None;
    }
    let [Statement::ReturnStatement(matched)] = block(&branch.consequent)? else {
        return None;
    };
    let Expression::UnaryExpression(outer) = matched.argument.as_ref()?.without_parentheses() else {
        return None;
    };
    let Expression::UnaryExpression(inner) = outer.argument.without_parentheses() else {
        return None;
    };
    if outer.operator.as_str() != "!" || inner.operator.as_str() != "!" {
        return None;
    }
    let matched = call(&inner.argument)?;
    let pattern = member(&matched.callee, "exec")?;
    data_capture(model, site, pattern)?;
    let arguments = arity(matched, 1)?;
    let argument = call(arguments[0])?;
    if member(&argument.callee, "toString").is_none_or(|e| symbol(model, e) != Some(formal))
        || arity(argument, 0).is_none()
    {
        return None;
    }
    let [Statement::ReturnStatement(fallback)] = block(branch.alternate.as_ref()?)? else {
        return None;
    };
    (matches!(fallback.argument.as_ref()?, Expression::BooleanLiteral(b) if !b.value)).then_some(())
}

// `if (worker != null) worker.terminate(); emitter.complete();`
fn terminate_worker_and_complete(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if !is_declaration(function) || !function.params.items.is_empty() || !directives(body) {
        return None;
    }
    let [Statement::IfStatement(guard), Statement::ExpressionStatement(complete)] =
        body.statements.as_slice()
    else {
        return None;
    };
    if guard.alternate.is_some() {
        return None;
    }
    let worker = null_symbol(model, &guard.test, "!=")?;
    if !capture_identity(model, site, worker, guard.test.span().start) {
        return None;
    }
    let [Statement::ExpressionStatement(terminate)] = block(&guard.consequent)? else {
        return None;
    };
    let terminate = call(&terminate.expression)?;
    let receiver = member(&terminate.callee, "terminate")?;
    if symbol(model, receiver) != Some(worker) || arity(terminate, 0).is_none() {
        return None;
    }
    let complete = call(&complete.expression)?;
    let emitter = member(&complete.callee, "complete")?;
    let emitter = data_capture(model, site, emitter)?;
    if emitter == worker || arity(complete, 0).is_none() {
        return None;
    }
    Some(())
}

// `this.first = formal; this.second = captured(oneArgument);` strict constructor.
fn initialize_batch_clock_state(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if !is_declaration(function)
        || body.directives.len() != 1
        || body.directives[0].directive != "use strict"
    {
        return None;
    }
    let formal = sole_formal(model, function)?;
    if model.binding_references(formal) != 1 {
        return None;
    }
    let [Statement::ExpressionStatement(first), Statement::ExpressionStatement(second)] =
        body.statements.as_slice()
    else {
        return None;
    };
    let first = expression_assignment(first)?;
    let AssignmentTarget::StaticMemberExpression(first_target) = &first.left else {
        return None;
    };
    if first_target.optional
        || !matches!(&first_target.object, Expression::ThisExpression(_))
        || symbol(model, &first.right) != Some(formal)
    {
        return None;
    }
    let second = expression_assignment(second)?;
    let AssignmentTarget::StaticMemberExpression(second_target) = &second.left else {
        return None;
    };
    if second_target.optional
        || !matches!(&second_target.object, Expression::ThisExpression(_))
        || second_target.property.name == first_target.property.name
    {
        return None;
    }
    let value = call(&second.right)?;
    data_capture(model, site, &value.callee)?;
    arity(value, 1)?;
    Some(())
}

// Complete extension-registry installer: declares the strict constructor and
// the register method, builds the single `extend` descriptor, installs the
// constructor through a captured installer and returns it.
fn install_extension_registry(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if !is_declaration(function) || !function.params.items.is_empty() || !directives(body) {
        return None;
    }
    let [
        Statement::VariableDeclaration(constructor_declaration),
        Statement::FunctionDeclaration(constructor),
        Statement::ExpressionStatement(constructor_store),
        Statement::VariableDeclaration(descriptor_declaration),
        Statement::FunctionDeclaration(register),
        Statement::ExpressionStatement(register_store),
        Statement::ExpressionStatement(descriptor_list),
        Statement::ExpressionStatement(installation),
        Statement::ReturnStatement(result),
    ] = body.statements.as_slice()
    else {
        return None;
    };
    let [constructor_variable] = constructor_declaration.declarations.as_slice() else {
        return None;
    };
    let constructor_symbol = plain_local(model, constructor_variable)?;
    if constructor_variable.init.is_some() || model.binding_writes(constructor_symbol) != 1 {
        return None;
    }
    let constructor_site = model.site_by_span(constructor.span)?;
    let constructor_symbol_id = bound_symbol(model, constructor)?;
    if crate::naming::functions::runtime::extension_registry::registry_constructor(
        model,
        constructor_site,
        constructor,
    )
    .is_none()
        || !crate::naming::functions::runtime::extension_registry::sibling_register(model, constructor_site)
        || crate::naming::functions::runtime::extension_registry::register_extension(model, register).is_none()
    {
        return None;
    }
    let constructor_store = expression_assignment(constructor_store)?;
    if assign_symbol(model, constructor_store)? != constructor_symbol
        || symbol(model, &constructor_store.right) != Some(constructor_symbol_id)
    {
        return None;
    }
    let [descriptor_variable] = descriptor_declaration.declarations.as_slice() else {
        return None;
    };
    let descriptor_symbol = plain_local(model, descriptor_variable)?;
    let Expression::ObjectExpression(descriptor) =
        descriptor_variable.init.as_ref()?.without_parentheses()
    else {
        return None;
    };
    let [ObjectPropertyKind::ObjectProperty(property)] = descriptor.properties.as_slice() else {
        return None;
    };
    if property.computed
        || property.method
        || property.kind != PropertyKind::Init
        || property_key_name(&property.key).as_deref() != Some("key")
        || !string(&property.value, "extend")
    {
        return None;
    }
    let register_store = expression_assignment(register_store)?;
    let AssignmentTarget::StaticMemberExpression(register_target) = &register_store.left else {
        return None;
    };
    let register_symbol_id = bound_symbol(model, register)?;
    if register_target.optional
        || register_target.property.name != "value"
        || symbol(model, &register_target.object) != Some(descriptor_symbol)
        || symbol(model, &register_store.right) != Some(register_symbol_id)
    {
        return None;
    }
    let descriptor_list = expression_assignment(descriptor_list)?;
    if assign_symbol(model, descriptor_list)? != descriptor_symbol {
        return None;
    }
    let Expression::ArrayExpression(list) = descriptor_list.right.without_parentheses() else {
        return None;
    };
    let [element] = list.elements.as_slice() else {
        return None;
    };
    if symbol(model, element.as_expression()?) != Some(descriptor_symbol) {
        return None;
    }
    let installation = call(&installation.expression)?;
    data_capture(model, site, &installation.callee)?;
    let arguments = arity(installation, 2)?;
    if symbol(model, arguments[0]) != Some(constructor_symbol_id)
        || symbol(model, arguments[1]) != Some(descriptor_symbol)
    {
        return None;
    }
    (symbol(model, result.argument.as_ref()?) == Some(constructor_symbol)).then_some(())
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
    if call_captured_discard(model, site, function, body).is_some() {
        claims.push(proposal(
            "invokeCapturedCallableDiscardResult",
            "runtime_captured_callee_wrappers",
            "whole own body is one nonoptional zero-argument call of a captured plain-origin callable with undefined receiver; the call result is discarded and implicit undefined returned",
            f,
        ));
    }
    if forward_formal_and_capture(model, site, function, body).is_some() {
        claims.push(proposal(
            "forwardFormalAndCapturedToCapturedCallable",
            "runtime_captured_callee_wrappers",
            "whole own body returns exactly the captured callable call with the unchanged sole formal first and one captured plain-origin value second; receiver, argument order and returned value preserved",
            f,
        ));
    }
    if append_formal_to_capture(model, site, function, body).is_some() {
        claims.push(proposal(
            "appendFormalToCapturedList",
            "runtime_captured_callee_wrappers",
            "whole own body is one captured-list push of the unchanged sole formal; push result discarded and implicit undefined returned",
            f,
        ));
    }
    if invoke_and_report_caught_error(model, site, function, body).is_some() {
        claims.push(proposal(
            "invokeFormalAndReportCaughtErrorToCapturedObservers",
            "runtime_captured_callee_wrappers",
            "whole own body invokes the unchanged sole formal inside try, then in catch reports the actual caught error to every captured observer in order via forEach(error); no result forwarding",
            f,
        ));
    }
    if reset_slots_and_dispose(model, site, function, body).is_some() {
        claims.push(proposal(
            "resetCapturedSlotsAndInvokeOptionalDisposer",
            "runtime_captured_callee_wrappers",
            "whole own body overwrites two distinct captured slots with the same captured sentinel in order, then null-guards and invokes the captured disposer; the disposer slot is not cleared and the call result is discarded",
            f,
        ));
    }
    if let Some(evidence) = throw_undefined_value_type_error(model, function, body) {
        let mut claim = proposal(
            "throwUndefinedValueReadTypeError",
            "runtime_undefined_value_read_error",
            "whole own body throws one TypeError constructed from the exact undefined-value read message; no parameters or other statements",
            f,
        );
        claim.evidence = evidence;
        claims.push(claim);
    }
    if check_native_function_source(model, site, function, body).is_some() {
        claims.push(proposal(
            "isNativeFunctionSource",
            "runtime_native_function_source_check",
            "whole own body gates on the sole formal being a function whose toString is the unshadowed Function.prototype.toString, then returns the captured regexp exec boolean over that source; alternate arm returns false; native identity and regexp result stay opaque",
            f,
        ));
    }
    if terminate_worker_and_complete(model, site, function, body).is_some() {
        claims.push(proposal(
            "terminateCapturedWorkerAndCompleteEmitter",
            "runtime_terminate_worker_complete",
            "whole own body null-guards the captured worker, invokes its terminate method with no arguments, then invokes the captured emitter complete method; order and discarded results preserved",
            f,
        ));
    }
    if initialize_batch_clock_state(model, site, function, body).is_some() {
        claims.push(proposal(
            "initializeBatchClockState",
            "runtime_batch_clock_constructor",
            "complete strict-mode own constructor body: stores the unchanged sole formal in one `this` field and then the captured callable result of one opaque argument in a distinct `this` field; receiver, field order and captured value stay opaque",
            f,
        ));
    }
    if install_extension_registry(model, site, function, body).is_some() {
        claims.push(proposal(
            "installExtensionRegistry",
            "shared_extension_registry_installer",
            "whole own body declares the strict self-instance constructor and the extensions register method, builds the single `extend` descriptor, installs the constructor through one captured installer with that descriptor list and returns the constructor binding",
            f,
        ));
    }
    claims
}
