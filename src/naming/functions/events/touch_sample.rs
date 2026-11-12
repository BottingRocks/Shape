use super::*;
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;

use crate::naming::support::interaction::*;

fn string(e: &Expression<'_>, value: &str) -> bool {
    matches!(e, Expression::StringLiteral(s) if s.value == value)
}

// A plain var/let/const factory whose whole body returns one function
// expression and is itself an ordered callable is the only supported
// clock-factory origin. The returned expression must be a direct function, not
// a captured data value.
fn clock_factory_callable(
    model: &Model<'_, '_>,
    owned: FunctionSite,
    e: &Expression<'_>,
) -> Option<FunctionSite> {
    let factory_call = call(e)?;
    if factory_call.optional || !factory_call.arguments.is_empty() {
        return None;
    }
    let factory = crate::naming::resolve::ordered_callable(model, &factory_call.callee, 0)?;
    if factory.span == owned.span {
        return None;
    }
    let AstKind::Function(function) = model.nodes.kind(factory.node_id) else {
        return None;
    };
    if function.r#async || function.generator {
        return None;
    }
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let [Statement::ReturnStatement(ret)] = body.statements.as_slice() else {
        return None;
    };
    let returned = model.function_from_expression(ret.argument.as_ref()?)?;
    (returned.span != owned.span).then_some(returned)
}

// A zero-argument value call may be an independently ordered callable
// (function declaration or plain alias), or a plain var/let/const local whose
// initializer is a narrowly proved clock factory call. Arbitrary initializers
// (constants, opaque member reads, redirected writes) never qualify.
fn scoped_callable_value(
    model: &Model<'_, '_>,
    site: FunctionSite,
    c: &CallExpression<'_>,
) -> bool {
    if c.optional || !c.arguments.is_empty() {
        return false;
    }
    if let Some(target) = crate::naming::resolve::ordered_callable(model, &c.callee, 0) {
        return target.span != site.span;
    }
    let Expression::Identifier(_) = &c.callee else {
        return false;
    };
    let Some(symbol) = symbol(model, &c.callee) else {
        return false;
    };
    if model.binding_writes(symbol) != 0 || !model.scoping.symbol_redeclarations(symbol).is_empty()
    {
        return false;
    }
    let declaration = model.scoping.symbol_declaration(symbol);
    let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else {
        return false;
    };
    if !matches!(&variable.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(symbol))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(d)
                if matches!(
                    d.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                )
        )
    {
        return false;
    }
    let container = model.nodes.parent_id(model.nodes.parent_id(declaration));
    if !matches!(
        model.nodes.kind(container),
        AstKind::FunctionBody(_) | AstKind::Program(_)
    ) {
        return false;
    }
    let Some(init) = variable.init.as_ref() else {
        return false;
    };
    if init.span().end > c.span.start {
        return false;
    }
    clock_factory_callable(model, site, init).is_some()
}

fn round_member(model: &Model<'_, '_>, e: &Expression<'_>, base: SymbolId, property: &str) -> bool {
    let Some(c) = call(e) else {
        return false;
    };
    if expression_path(model, &c.callee) != ["Math", "round"] {
        return false;
    }
    let Some(args) = arguments(c) else {
        return false;
    };
    let [value] = args.as_slice() else {
        return false;
    };
    member(value, property).and_then(|e| symbol(model, e)) == Some(base)
}

fn round_difference(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    left_base: SymbolId,
    left_property: &str,
    right_base: SymbolId,
    right_property: &str,
) -> bool {
    let Some(c) = call(e) else {
        return false;
    };
    if expression_path(model, &c.callee) != ["Math", "round"] {
        return false;
    }
    let Some(args) = arguments(c) else {
        return false;
    };
    let [value] = args.as_slice() else {
        return false;
    };
    let Some((left, right)) = binary(value, "-") else {
        return false;
    };
    member(left, left_property).and_then(|e| symbol(model, e)) == Some(left_base)
        && member(right, right_property).and_then(|e| symbol(model, e)) == Some(right_base)
}

fn null_test(model: &Model<'_, '_>, e: &Expression<'_>, value: SymbolId, op: &str) -> bool {
    matches!(binary(e, op), Some((left, right))
        if symbol(model, left) == Some(value) && matches!(right, Expression::NullLiteral(_)))
}

fn record_touch_samples(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if function.params.rest.is_some() || function.params.items.len() != 1 {
        return None;
    }
    let input = plain_param(model, &function.params.items[0])?;
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
    if model.binding_writes(error) != 0 || !model.scoping.symbol_redeclarations(error).is_empty() {
        return None;
    }
    let [Statement::ExpressionStatement(failed)] = handler.body.body.as_slice() else {
        return None;
    };
    let failed = call(&failed.expression)?;
    captured_symbol(model, site, method_receiver(model, failed, "error")?)?;
    let failed_args = arguments(failed)?;
    if failed_args.len() != 1 || symbol(model, failed_args[0]) != Some(error) {
        return None;
    }
    let [state_statement, list_statement, touches_statement, guard, put, flag] =
        t.block.body.as_slice()
    else {
        return None;
    };
    let state = plain_var(model, variable(state_statement)?)?;
    let Expression::ObjectExpression(state_object) = variable(state_statement)?.init.as_ref()?
    else {
        return None;
    };
    if state_object.properties.len() != 2 {
        return None;
    }
    let mut ui_event = false;
    let mut trusted = false;
    for property in &state_object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        if property.computed || property.method || property.kind != PropertyKind::Init {
            return None;
        }
        match property_key_name(&property.key)?.as_str() {
            "instanceOfUIEvent" => {
                ui_event = matches!(&property.value, Expression::BinaryExpression(b)
                    if b.operator.as_str() == "instanceof"
                        && symbol(model, &b.left) == Some(input)
                        && expression_path(model, &b.right) == ["UIEvent"])
            }
            "markedAsTrusted" => {
                trusted = matches!(&property.value, Expression::UnaryExpression(outer)
                    if outer.operator.as_str() == "!"
                        && matches!(&outer.argument, Expression::UnaryExpression(inner)
                            if inner.operator.as_str() == "!"
                                && member(&inner.argument, "isTrusted").and_then(|e| symbol(model, e)) == Some(input)))
            }
            _ => return None,
        }
    }
    if !ui_event || !trusted {
        return None;
    }
    let list = plain_var(model, variable(list_statement)?)?;
    if !matches!(variable(list_statement)?.init.as_ref()?, Expression::ArrayExpression(a) if a.elements.is_empty())
    {
        return None;
    }
    let touches = plain_var(model, variable(touches_statement)?)?;
    let Expression::ComputedMemberExpression(selected) =
        variable(touches_statement)?.init.as_ref()?
    else {
        return None;
    };
    if selected.optional || symbol(model, &selected.object) != Some(input) {
        return None;
    }
    let Expression::ConditionalExpression(selection) = &selected.expression else {
        return None;
    };
    let Expression::LogicalExpression(and) = &selection.test else {
        return None;
    };
    if and.operator.as_str() != "&&"
        || member(&and.left, "touches").and_then(|e| symbol(model, e)) != Some(input)
        || !matches!(&and.right, Expression::BinaryExpression(b)
            if b.operator.as_str() == "==="
                && member(&b.left, "length").and_then(|e| member(e, "touches")).and_then(|e| symbol(model, e)) == Some(input)
                && number(&b.right, 0.0))
        || !string(&selection.consequent, "changedTouches")
        || !string(&selection.alternate, "touches")
    {
        return None;
    }
    let Statement::IfStatement(guard) = guard else {
        return None;
    };
    if guard.alternate.is_some() {
        return None;
    }
    let Expression::LogicalExpression(and) = &guard.test else {
        return None;
    };
    if and.operator.as_str() != "&&"
        || symbol(model, &and.left) != Some(touches)
        || !matches!(&and.right, Expression::BinaryExpression(b)
            if b.operator.as_str() == "!=="
                && member(&b.left, "length").and_then(|e| symbol(model, e)) == Some(touches)
                && number(&b.right, 0.0))
    {
        return None;
    }
    let Statement::BlockStatement(samples) = &guard.consequent else {
        return None;
    };
    let [Statement::VariableDeclaration(count), Statement::VariableDeclaration(index), Statement::ForStatement(loop_)] =
        samples.body.as_slice()
    else {
        return None;
    };
    let count_init = single_variable(count)?.init.as_ref()?;
    let count = plain_var(model, single_variable(count)?)?;
    if member(count_init, "length").and_then(|e| symbol(model, e)) != Some(touches) {
        return None;
    }
    let index_variable = single_variable(index)?;
    let index = plain_var(model, index_variable)?;
    if !number(index_variable.init.as_ref()?, 0.0) || loop_.init.is_some() {
        return None;
    }
    let Some(Expression::BinaryExpression(test)) = loop_.test.as_ref() else {
        return None;
    };
    if test.operator.as_str() != "<"
        || symbol(model, &test.left) != Some(index)
        || symbol(model, &test.right) != Some(count)
    {
        return None;
    }
    let Some(Expression::UpdateExpression(update)) = loop_.update.as_ref() else {
        return None;
    };
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(counter) = &update.argument else {
        return None;
    };
    if update.operator.as_str() != "++"
        || crate::analysis::identifier_symbol(counter, model.scoping) != Some(index)
    {
        return None;
    }
    let Statement::BlockStatement(sample_block) = &loop_.body else {
        return None;
    };
    let [point_statement, target_statement, rect_statement, x_statement, y_statement, push] =
        sample_block.body.as_slice()
    else {
        return None;
    };
    let point_variable = single_variable(match point_statement {
        Statement::VariableDeclaration(d) => d,
        _ => return None,
    })?;
    let point = plain_var(model, point_variable)?;
    let Expression::ComputedMemberExpression(point_init) = point_variable.init.as_ref()? else {
        return None;
    };
    if point_init.optional
        || symbol(model, &point_init.object) != Some(touches)
        || symbol(model, &point_init.expression) != Some(index)
    {
        return None;
    }
    let target_variable = single_variable(match target_statement {
        Statement::VariableDeclaration(d) => d,
        _ => return None,
    })?;
    let target = plain_var(model, target_variable)?;
    if member(target_variable.init.as_ref()?, "target").and_then(|e| symbol(model, e))
        != Some(point)
    {
        return None;
    }
    let Some(rect) = assignment(match rect_statement {
        Statement::ExpressionStatement(s) => &s.expression,
        _ => return None,
    }) else {
        return None;
    };
    if target_symbol(model, &rect.left) != Some(target) {
        return None;
    }
    let Expression::ConditionalExpression(rect) = &rect.right else {
        return None;
    };
    if !matches!(&rect.test, Expression::BinaryExpression(b)
            if b.operator.as_str() == "instanceof"
                && symbol(model, &b.left) == Some(target)
                && expression_path(model, &b.right) == ["Element"])
    {
        return None;
    }
    let Some(rect_call) = call(&rect.consequent) else {
        return None;
    };
    if method_receiver(model, rect_call, "getBoundingClientRect") != Some(target)
        || !rect_call.arguments.is_empty()
        || !matches!(rect.alternate, Expression::NullLiteral(_))
    {
        return None;
    }
    let x_variable = single_variable(match x_statement {
        Statement::VariableDeclaration(d) => d,
        _ => return None,
    })?;
    let x = plain_var(model, x_variable)?;
    let Expression::ConditionalExpression(x_value) = x_variable.init.as_ref()? else {
        return None;
    };
    if !null_test(model, &x_value.test, target, "==")
        || !number(&x_value.consequent, 0.0)
        || !round_difference(model, &x_value.alternate, point, "clientX", target, "left")
    {
        return None;
    }
    let Some(y) = assignment(match y_statement {
        Statement::ExpressionStatement(s) => &s.expression,
        _ => return None,
    }) else {
        return None;
    };
    if target_symbol(model, &y.left) != Some(target) {
        return None;
    }
    let Expression::ConditionalExpression(y_value) = &y.right else {
        return None;
    };
    if !null_test(model, &y_value.test, target, "==")
        || !number(&y_value.consequent, 0.0)
        || !round_difference(model, &y_value.alternate, point, "clientY", target, "top")
    {
        return None;
    }
    let push = call(match push {
        Statement::ExpressionStatement(s) => &s.expression,
        _ => return None,
    })?;
    if method_receiver(model, push, "push") != Some(list) {
        return None;
    }
    let push_args = arguments(push)?;
    let [Expression::ObjectExpression(sample)] = push_args.as_slice() else {
        return None;
    };
    if sample.properties.len() != 5 {
        return None;
    }
    let mut keys = BTreeSet::new();
    for property in &sample.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        if property.computed || property.method || property.kind != PropertyKind::Init {
            return None;
        }
        let key = property_key_name(&property.key)?;
        if !keys.insert(key.clone()) {
            return None;
        }
        let valid = match key.as_str() {
            "a" => round_member(model, &property.value, point, "pageX"),
            "b" => round_member(model, &property.value, point, "pageY"),
            "c" => symbol(model, &property.value) == Some(x),
            "d" => symbol(model, &property.value) == Some(target),
            "e" => member(&property.value, "force").and_then(|e| symbol(model, e)) == Some(point),
            _ => false,
        };
        if !valid {
            return None;
        }
    }
    let put = call(match put {
        Statement::ExpressionStatement(s) => &s.expression,
        _ => return None,
    })?;
    captured_symbol(model, site, method_receiver(model, put, "put")?)?;
    let put_args = arguments(put)?;
    let [Expression::ObjectExpression(record)] = put_args.as_slice() else {
        return None;
    };
    if record.properties.len() != 5 {
        return None;
    }
    let mut keys = BTreeSet::new();
    for property in &record.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        if property.computed || property.method || property.kind != PropertyKind::Init {
            return None;
        }
        let key = property_key_name(&property.key)?;
        if !keys.insert(key.clone()) {
            return None;
        }
        let valid = match key.as_str() {
            "a" => matches!(&property.value, Expression::ArrayExpression(a)
                if a.elements.len() == 2
                    && a.elements.iter().all(|e| e.as_expression().is_some_and(|e| matches!(e, Expression::BooleanLiteral(_))))),
            "b" => call(&property.value).is_some_and(|c| scoped_callable_value(model, site, c)),
            "c" => symbol(model, &property.value) == Some(list),
            "d" => {
                member(&property.value, "instanceOfUIEvent").and_then(|e| symbol(model, e))
                    == Some(state)
            }
            "e" => {
                member(&property.value, "markedAsTrusted").and_then(|e| symbol(model, e))
                    == Some(state)
            }
            _ => false,
        };
        if !valid {
            return None;
        }
    }
    let flag_assignment = assignment(match flag {
        Statement::ExpressionStatement(s) => &s.expression,
        _ => return None,
    })?;
    captured_symbol(model, site, target_symbol(model, &flag_assignment.left)?)?;
    if !boolean(&flag_assignment.right, true) {
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
        return Vec::new();
    };
    let Some(body) = function.body.as_ref() else {
        return Vec::new();
    };
    if function.r#async || function.generator || !body.directives.is_empty() {
        return Vec::new();
    }
    let mut claims = Vec::new();
    if record_touch_samples(model, site, function, body).is_some() {
        claims.push(proposal(
            "recordTouchSample",
            "events_touch_sample",
            "catch-guarded owned handler reads captured touches (or changedTouches when the touch list is empty), records each touch page/client/force sample through the same captured array, and puts one five-field record carrying the two UIEvent/trusted observations; the timestamp field is a zero-argument call to an independently ordered plain var/let/const callable or a narrowly proved zero-argument factory returning a function expression; catch forwards the exact caught binding to a captured error receiver",
            f,
        ));
    }
    claims
}
