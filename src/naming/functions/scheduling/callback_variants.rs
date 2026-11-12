//! Owned-body lexical variants for callback scheduling and receiver lists.
//! No callee aliases or nested callable bodies contribute evidence.
use super::*;
use oxc_syntax::operator::{BinaryOperator, UpdateOperator};

fn expr<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    match s {
        Statement::ExpressionStatement(s) => Some(&s.expression),
        _ => None,
    }
}
fn ret<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    match s {
        Statement::ReturnStatement(s) => s.argument.as_ref(),
        _ => None,
    }
}
fn call<'a>(e: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    match e.without_parentheses() {
        Expression::CallExpression(c) if !c.optional => Some(c),
        _ => None,
    }
}
fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e.without_parentheses(), model.scoping)
}
fn number(e: &Expression<'_>, n: f64) -> bool {
    matches!(e.without_parentheses(), Expression::NumericLiteral(v) if v.value == n)
}
fn stable(model: &Model<'_, '_>, s: SymbolId) -> bool {
    model.binding_writes(s) == 0 && model.scoping.symbol_redeclarations(s).is_empty()
}
fn schedule(model: &Model<'_, '_>, body: &FunctionBody<'_>, parameter: SymbolId) -> bool {
    let [s] = body.statements.as_slice() else {
        return false;
    };
    let Some(c) = expr(s).and_then(call) else {
        return false;
    };
    let Expression::Identifier(id) = c.callee.without_parentheses() else {
        return false;
    };
    let args = call_arguments(c);
    id.name == "setTimeout"
        && symbol(model, &c.callee).is_none()
        && stable(model, parameter)
        && c.arguments.len() == 2
        && args.len() == 2
        && symbol(model, args[0]) == Some(parameter)
        && number(args[1], 0.0)
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return vec![];
    };
    let Some(body) = &function.body else {
        return vec![];
    };
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || body.directives.iter().any(|d| d.directive != "use strict")
    {
        return vec![];
    }
    let mut parameters = Vec::new();
    for p in &function.params.items {
        let BindingPattern::BindingIdentifier(id) = &p.pattern else {
            return vec![];
        };
        if p.initializer.is_some() {
            return vec![];
        }
        let Some(s) = id.symbol_id.get() else {
            return vec![];
        };
        parameters.push(s);
    }
    if let [parameter] = parameters.as_slice() {
        if schedule(model, body, *parameter) {
            return vec![proposal("scheduleCallbackAtZeroDelay", "shared_callback_schedule",
                "whole owned body calls unshadowed setTimeout with the unchanged sole formal and literal zero delay; no returned handle or extra effects", facts)];
        }
    }
    if let Some(role) = callback_list(model, body, &parameters) {
        return vec![proposal(role, "shared_callback_list", "whole owned receiver-list algorithm: exact initialization/append/reset, or indexed loop with binding-resolved cursor, formal, list access and accumulator; no nested evidence or extra effects", facts)];
    }
    vec![]
}

fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    match e.without_parentheses() {
        Expression::StaticMemberExpression(m) if !m.optional && m.property.name == key => {
            Some(&m.object)
        }
        _ => None,
    }
}
fn list(e: &Expression<'_>, key: &str) -> bool {
    member(e, key).is_some_and(|o| matches!(o.without_parentheses(), Expression::ThisExpression(_)))
}
fn empty(e: &Expression<'_>) -> bool {
    matches!(e.without_parentheses(), Expression::ArrayExpression(a) if a.elements.is_empty())
}
fn void_zero(e: &Expression<'_>) -> bool {
    matches!(e.without_parentheses(), Expression::UnaryExpression(u) if u.operator == oxc_syntax::operator::UnaryOperator::Void && number(&u.argument, 0.0))
}
fn this_return(s: &Statement<'_>) -> bool {
    ret(s).is_some_and(|e| matches!(e.without_parentheses(), Expression::ThisExpression(_)))
}
fn assignment<'a>(s: &'a Statement<'a>) -> Option<&'a AssignmentExpression<'a>> {
    match expr(s)?.without_parentheses() {
        Expression::AssignmentExpression(a) if a.operator == AssignmentOperator::Assign => Some(a),
        _ => None,
    }
}
fn clears_list(s: &Statement<'_>, key: &str) -> bool {
    let Some(a) = assignment(s) else {
        return false;
    };
    matches!(&a.left, AssignmentTarget::StaticMemberExpression(m) if !m.optional && m.property.name == key && matches!(m.object.without_parentheses(), Expression::ThisExpression(_)))
        && empty(&a.right)
}
fn local<'a>(
    model: &Model<'_, '_>,
    s: &'a Statement<'a>,
) -> Option<(SymbolId, &'a Expression<'a>)> {
    let Statement::VariableDeclaration(d) = s else {
        return None;
    };
    if d.kind == VariableDeclarationKind::Const {
        return None;
    }
    let [v] = d.declarations.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(id) = &v.id else {
        return None;
    };
    let symbol = id.symbol_id.get()?;
    if !model.scoping.symbol_redeclarations(symbol).is_empty() {
        return None;
    }
    Some((symbol, v.init.as_ref()?))
}
fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    match s {
        Statement::BlockStatement(b) => Some(b.body.as_slice()),
        _ => None,
    }
}
fn indexed(model: &Model<'_, '_>, e: &Expression<'_>, key: &str, cursor: SymbolId) -> bool {
    matches!(e.without_parentheses(), Expression::ComputedMemberExpression(m) if !m.optional && list(&m.object,key) && symbol(model,&m.expression)==Some(cursor))
}
fn loop_body<'a>(
    model: &Model<'_, '_>,
    s: &'a Statement<'a>,
    key: &str,
    cursor: SymbolId,
) -> Option<&'a [Statement<'a>]> {
    let Statement::ForStatement(loop_) = s else {
        return None;
    };
    if loop_.init.is_some() || model.binding_writes(cursor) != 1 {
        return None;
    }
    let Expression::BinaryExpression(test) = loop_.test.as_ref()?.without_parentheses() else {
        return None;
    };
    if test.operator != BinaryOperator::LessThan
        || symbol(model, &test.left) != Some(cursor)
        || !member(&test.right, "length").is_some_and(|e| list(e, key))
    {
        return None;
    }
    let Expression::UpdateExpression(update) = loop_.update.as_ref()?.without_parentheses() else {
        return None;
    };
    if update.operator != UpdateOperator::Increment
        || !matches!(&update.argument, SimpleAssignmentTarget::AssignmentTargetIdentifier(i) if crate::analysis::identifier_symbol(i,model.scoping)==Some(cursor))
    {
        return None;
    }
    block(&loop_.body)
}
fn remove_matches(
    model: &Model<'_, '_>,
    body: &FunctionBody<'_>,
    key: &str,
    parameter: SymbolId,
) -> Option<()> {
    let [init, iteration, end] = body.statements.as_slice() else {
        return None;
    };
    let (cursor, zero) = local(model, init)?;
    if !number(zero, 0.0) || !this_return(end) {
        return None;
    }
    let [Statement::IfStatement(test)] = loop_body(model, iteration, key, cursor)? else {
        return None;
    };
    if test.alternate.is_some() {
        return None;
    }
    let Expression::BinaryExpression(eq) = test.test.without_parentheses() else {
        return None;
    };
    if eq.operator != BinaryOperator::StrictEquality
        || !indexed(model, &eq.left, key, cursor)
        || symbol(model, &eq.right) != Some(parameter)
    {
        return None;
    }
    let [clear] = block(&test.consequent)? else {
        return None;
    };
    let a = assignment(clear)?;
    if !matches!(&a.left, AssignmentTarget::ComputedMemberExpression(m) if !m.optional && list(&m.object,key) && symbol(model,&m.expression)==Some(cursor))
        || !void_zero(&a.right)
    {
        return None;
    }
    Some(())
}
fn notify_chain(
    model: &Model<'_, '_>,
    body: &FunctionBody<'_>,
    key: &str,
    parameter: SymbolId,
) -> Option<()> {
    let [init, seed, iteration, end] = body.statements.as_slice() else {
        return None;
    };
    let (cursor, zero) = local(model, init)?;
    let (acc, initial) = local(model, seed)?;
    if !number(zero, 0.0)
        || !matches!(initial.without_parentheses(), Expression::NullLiteral(_))
        || cursor == acc
        || model.binding_writes(acc) != 1
        || symbol(model, ret(end)?) != Some(acc)
    {
        return None;
    }
    let [Statement::IfStatement(guard)] = loop_body(model, iteration, key, cursor)? else {
        return None;
    };
    if guard.alternate.is_some() {
        return None;
    }
    let Expression::BinaryExpression(test) = guard.test.without_parentheses() else {
        return None;
    };
    if test.operator != BinaryOperator::Inequality
        || !indexed(model, &test.left, key, cursor)
        || !matches!(test.right.without_parentheses(), Expression::NullLiteral(_))
    {
        return None;
    }
    let [Statement::TryStatement(attempt)] = block(&guard.consequent)? else {
        return None;
    };
    if attempt.finalizer.is_some() {
        return None;
    }
    let catch = attempt.handler.as_ref()?;
    if !catch.body.body.is_empty()
        || catch
            .param
            .as_ref()
            .is_some_and(|p| !matches!(&p.pattern, BindingPattern::BindingIdentifier(_)))
    {
        return None;
    }
    let [step] = attempt.block.body.as_slice() else {
        return None;
    };
    let a = assignment(step)?;
    if !matches!(&a.left, AssignmentTarget::AssignmentTargetIdentifier(i) if crate::analysis::identifier_symbol(i,model.scoping)==Some(acc))
    {
        return None;
    }
    let c = call(&a.right)?;
    if !indexed(model, member(&c.callee, "call")?, key, cursor) || c.arguments.len() != 3 {
        return None;
    }
    let args = call_arguments(c);
    if args.len() != 3
        || !void_zero(args[0])
        || symbol(model, args[1]) != Some(parameter)
        || symbol(model, args[2]) != Some(acc)
    {
        return None;
    }
    Some(())
}
fn callback_list(
    model: &Model<'_, '_>,
    body: &FunctionBody<'_>,
    parameters: &[SymbolId],
) -> Option<&'static str> {
    for key in ["callbacks", "__callbacks"] {
        if parameters.is_empty() {
            match body.statements.as_slice() {
                [clear] if clears_list(clear, key) => return Some("initializeCallbackList"),
                [clear, end] if clears_list(clear, key) && this_return(end) => {
                    return Some("clearCallbackList")
                }
                _ => {}
            }
        }
        if let [p] = parameters {
            if !stable(model, *p) {
                continue;
            }
            if let [push, end] = body.statements.as_slice() {
                if let Some(c) = expr(push).and_then(call) {
                    let args = call_arguments(c);
                    if member(&c.callee, "push").is_some_and(|e| list(e, key))
                        && c.arguments.len() == 1
                        && args.len() == 1
                        && symbol(model, args[0]) == Some(*p)
                        && this_return(end)
                    {
                        return Some("appendCallback");
                    }
                }
            }
            if remove_matches(model, body, key, *p).is_some() {
                return Some("removeMatchingCallbacks");
            }
            if notify_chain(model, body, key, *p).is_some() {
                return Some("notifyCallbackChain");
            }
        }
    }
    None
}
