use super::*;

use crate::naming::support::publish_sync::*;

fn plain_formal(model: &Model<'_, '_>, p: &FormalParameter<'_>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(id) = &p.pattern else {
        return None;
    };
    let s = id.symbol_id.get()?;
    (p.initializer.is_none()
        && model.binding_writes(s) == 0
        && model.scoping.symbol_redeclarations(s).is_empty())
    .then_some(s)
}

fn plain_local_binding(
    model: &Model<'_, '_>,
    declaration: &VariableDeclaration<'_>,
    v: &VariableDeclarator<'_>,
) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(id) = &v.id else {
        return None;
    };
    let s = id.symbol_id.get()?;
    (plain_variable_kind(declaration.kind)
        && model.binding_writes(s) == 0
        && model.scoping.symbol_redeclarations(s).is_empty())
    .then_some(s)
}

// `event.target.visibilityState === "visible"` on the unchanged sole formal.
fn visibility_predicate(model: &Model<'_, '_>, e: &Expression<'_>, input: Symbol) -> bool {
    let Expression::BinaryExpression(test) = e else {
        return false;
    };
    let Expression::StaticMemberExpression(visibility) = &test.left else {
        return false;
    };
    test.operator.as_str() == "==="
        && !visibility.optional
        && visibility.property.name == "visibilityState"
        && member(&visibility.object, "target").is_some_and(|t| symbol(model, t) == Some(input))
        && matches!(&test.right, Expression::StringLiteral(s) if s.value == "visible")
}

// A visibility-change publisher: its whole body is one try that increments a
// captured counter, forwards a `{visible, timestamp}` state object to a
// captured stream's method through receiver-preserving `.call`, sets a captured
// flag, and whose catch reports the caught binding to a captured error sink.
// The timestamp zero-argument call is re-proved through the shared ordered
// callable resolver (distinct from the site), not by a spelling or by a mere
// initializer's existence.
fn visibility_change_publish(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [parameter] = function.params.items.as_slice() else {
        return None;
    };
    if function.params.rest.is_some() {
        return None;
    }
    let input = plain_formal(model, parameter)?;
    let [Statement::TryStatement(attempt)] = body.statements.as_slice() else {
        return None;
    };
    if attempt.finalizer.is_some() {
        return None;
    }
    let [Statement::ExpressionStatement(counter), Statement::VariableDeclaration(stream_declaration), Statement::VariableDeclaration(put_declaration), Statement::ExpressionStatement(publish), Statement::ExpressionStatement(flag)] =
        attempt.block.body.as_slice()
    else {
        return None;
    };
    // ++capturedCounter;
    let Expression::UpdateExpression(update) = &counter.expression else {
        return None;
    };
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(target) = &update.argument else {
        return None;
    };
    if update.operator.as_str() != "++"
        || !captured_slot(
            model,
            site,
            crate::analysis::identifier_symbol(target, model.scoping)?,
        )
    {
        return None;
    }
    // let stream = <captured>;
    let [stream_variable] = stream_declaration.declarations.as_slice() else {
        return None;
    };
    let stream = plain_local_binding(model, stream_declaration, stream_variable)?;
    let stream_value = symbol(model, stream_variable.init.as_ref()?)?;
    if !captured_slot(model, site, stream_value) {
        return None;
    }
    // let put = stream.put;
    let [put_variable] = put_declaration.declarations.as_slice() else {
        return None;
    };
    let put = plain_local_binding(model, put_declaration, put_variable)?;
    if symbol(model, member(put_variable.init.as_ref()?, "put")?) != Some(stream_value) {
        return None;
    }
    // put.call(stream, {visible, timestamp});
    let Expression::CallExpression(invoke) = &publish.expression else {
        return None;
    };
    let Expression::StaticMemberExpression(call_member) = &invoke.callee else {
        return None;
    };
    if invoke.optional
        || call_member.optional
        || call_member.property.name != "call"
        || symbol(model, &call_member.object) != Some(put)
        || invoke.arguments.len() != 2
    {
        return None;
    }
    let args = call_arguments(invoke);
    let [receiver, payload] = args.as_slice() else {
        return None;
    };
    if symbol(model, receiver) != Some(stream) {
        return None;
    }
    let Expression::ObjectExpression(object) = payload else {
        return None;
    };
    if object.properties.len() != 2 {
        return None;
    }
    let mut seen = BTreeSet::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(p) = property else {
            return None;
        };
        let key = property_key_name(&p.key)?;
        if p.computed || p.method || p.kind != PropertyKind::Init || !seen.insert(key.clone()) {
            return None;
        }
        match key.as_str() {
            "visible" => {
                if !visibility_predicate(model, &p.value, input) {
                    return None;
                }
            }
            "timestamp" => {
                let Expression::CallExpression(clock) = &p.value else {
                    return None;
                };
                if clock.optional
                    || !clock.arguments.is_empty()
                    || crate::naming::resolve::ordered_callable(model, &clock.callee, 0)
                        .is_none_or(|clock| clock.span == site.span)
                {
                    return None;
                }
            }
            _ => return None,
        }
    }
    // capturedFlag = true;
    let Expression::AssignmentExpression(assignment) = &flag.expression else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(flag_target) = &assignment.left else {
        return None;
    };
    if assignment.operator.as_str() != "="
        || !matches!(&assignment.right, Expression::BooleanLiteral(b) if b.value)
        || !captured_slot(
            model,
            site,
            crate::analysis::identifier_symbol(flag_target, model.scoping)?,
        )
    {
        return None;
    }
    // catch (error) { sink.error(error); }
    let handler = attempt.handler.as_ref()?;
    let BindingPattern::BindingIdentifier(caught) = &handler.param.as_ref()?.pattern else {
        return None;
    };
    let caught = caught.symbol_id.get()?;
    if model.binding_writes(caught) != 0 || !model.scoping.symbol_redeclarations(caught).is_empty()
    {
        return None;
    }
    let [Statement::ExpressionStatement(failure)] = handler.body.body.as_slice() else {
        return None;
    };
    let Expression::CallExpression(log) = &failure.expression else {
        return None;
    };
    let sink = symbol(model, member(&log.callee, "error")?)?;
    if log.optional
        || log.arguments.len() != 1
        || call_arguments(log).first().and_then(|e| symbol(model, e)) != Some(caught)
        || !captured_slot(model, site, sink)
    {
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
    if visibility_change_publish(model, site, function, body).is_some() {
        claims.push(proposal(
            "recordVisibilityChangeSample",
            "shared_anchor_visibility_publish",
            "sole try increments a captured counter, forwards {visible: input.target.visibilityState === \"visible\", timestamp: clock()} to a captured stream method through receiver-preserving .call, sets a captured flag and reports the caught binding to a captured error sink; every identity is a plain ordered var/let/const binding and the zero-argument clock is re-proved through the shared ordered callable resolver distinct from the site",
            f,
        ));
    }
    claims
}
