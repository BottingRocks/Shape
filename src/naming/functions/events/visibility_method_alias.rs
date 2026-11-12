//! Frontier rules for the residual cases, staged after the older
//! generations decline a function.
//!
//! The one rule here proves the p12 `visibilitychange` sample recorder whose
//! ring `put` is reached through a scoped method alias (`let m = ring.put;
//! m.call(ring, record)`). The package finder models the role, but its strict
//! `put` resolver follows only a direct member callee, so it declines this
//! emitted alias form and the function stays unnamed.
//!
//! The proof is the whole owned body, not a scattered fact bag:
//!
//! * exactly one plain, immutable event formal;
//! * one try/catch, no finalizer, five ordered try statements and one catch
//!   statement;
//! * the captured ring-dirty counter is incremented;
//! * a scoped alias of the captured ring and a scoped alias of its `put` method
//!   are read, and the record is stored through `put.call(ring, record)`;
//! * the record is exactly `{visible, timestamp}`, with `visible` read from the
//!   opaque event input as `event.target.visibilityState === "visible"` and
//!   `timestamp` produced by the session-context clock member;
//! * the captured ring-dirty flag is set and a thrown error is forwarded to the
//!   captured error sink.
//!
//! The event object is an opaque runtime callback input and is treated only as
//! data; no native/DOM event type is asserted. The clock is identified by the
//! bounded session-context provider protocol (`<context>.timeElapsedSinceInit`)
//! rather than by pretending a specific callback was independently proven.
use super::*;
use oxc_semantic::SymbolId;
use oxc_syntax::operator::{AssignmentOperator, UpdateOperator};

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e, model.scoping)
}

fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = e else {
        return None;
    };
    (!m.optional && m.property.name == key).then_some(&m.object)
}

fn one_declarator<'a>(s: &'a Statement<'a>) -> Option<&'a VariableDeclarator<'a>> {
    let Statement::VariableDeclaration(d) = s else {
        return None;
    };
    let [v] = d.declarations.as_slice() else {
        return None;
    };
    Some(v)
}

fn plain_binding(p: &BindingPattern<'_>) -> Option<SymbolId> {
    let BindingPattern::BindingIdentifier(id) = p else {
        return None;
    };
    id.symbol_id.get()
}

/// A binding declared outside the function's own span, so the owned body may not
/// introduce or shadow it.
fn outer(model: &Model<'_, '_>, site: FunctionSite, s: SymbolId) -> bool {
    !site.span.contains_inclusive(model.scoping.symbol_span(s))
}

fn immutable(model: &Model<'_, '_>, s: SymbolId) -> bool {
    model.binding_writes(s) == 0 && model.scoping.symbol_redeclarations(s).is_empty()
}

/// A zero-argument call of a session-context clock member: either
/// `<context>.<member>()` directly, or a captured immutable alias whose single
/// origin is exactly that member of a captured context object.
fn context_member_call(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
    member_name: &str,
) -> Option<()> {
    let Expression::CallExpression(call) = e else {
        return None;
    };
    if call.optional || !call.arguments.is_empty() || !call_arguments(call).is_empty() {
        return None;
    }
    if let Some(context) = member(&call.callee, member_name) {
        return symbol(model, context)
            .filter(|s| outer(model, site, *s))
            .map(|_| ());
    }
    let clock = symbol(model, &call.callee)?;
    if !outer(model, site, clock) || !immutable(model, clock) {
        return None;
    }
    let context = member(model.initializer(clock)?, member_name)?;
    symbol(model, context)
        .filter(|s| outer(model, site, *s))
        .map(|_| ())
}

fn visibility_recorder(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [parameter] = function.params.items.as_slice() else {
        return None;
    };
    if function.params.rest.is_some() || parameter.initializer.is_some() {
        return None;
    }
    let event = plain_binding(&parameter.pattern)?;
    if !immutable(model, event) {
        return None;
    }
    let [Statement::TryStatement(t)] = body.statements.as_slice() else {
        return None;
    };
    if t.finalizer.is_some() {
        return None;
    }
    let [bump, bind_receiver, bind_method, emit, set_flag] = t.block.body.as_slice() else {
        return None;
    };
    // ++counter
    let Statement::ExpressionStatement(bump) = bump else {
        return None;
    };
    let Expression::UpdateExpression(update) = &bump.expression else {
        return None;
    };
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(counter) = &update.argument else {
        return None;
    };
    let counter = crate::analysis::identifier_symbol(counter, model.scoping)?;
    if update.operator != UpdateOperator::Increment
        || !update.prefix
        || !outer(model, site, counter)
    {
        return None;
    }
    // let receiver = ring;
    let bind_receiver = one_declarator(bind_receiver)?;
    let receiver = plain_binding(&bind_receiver.id)?;
    let ring = symbol(model, bind_receiver.init.as_ref()?)?;
    if !immutable(model, receiver) || !outer(model, site, ring) {
        return None;
    }
    // let method = ring.put;
    let bind_method = one_declarator(bind_method)?;
    let method = plain_binding(&bind_method.id)?;
    if !immutable(model, method)
        || symbol(model, member(bind_method.init.as_ref()?, "put")?)? != ring
    {
        return None;
    }
    // method.call(receiver, { ... })
    let Statement::ExpressionStatement(emit) = emit else {
        return None;
    };
    let Expression::CallExpression(emit) = &emit.expression else {
        return None;
    };
    if emit.optional || symbol(model, member(&emit.callee, "call")?)? != method {
        return None;
    }
    let args = call_arguments(emit);
    let [target, record] = args.as_slice() else {
        return None;
    };
    let (target, record) = (*target, *record);
    if symbol(model, target)? != receiver {
        return None;
    }
    let Expression::ObjectExpression(record) = record else {
        return None;
    };
    // flag = true
    let Statement::ExpressionStatement(set_flag) = set_flag else {
        return None;
    };
    let Expression::AssignmentExpression(set_flag) = &set_flag.expression else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(flag) = &set_flag.left else {
        return None;
    };
    let flag = crate::analysis::identifier_symbol(flag, model.scoping)?;
    if set_flag.operator != AssignmentOperator::Assign
        || !matches!(&set_flag.right, Expression::BooleanLiteral(b) if b.value)
        || !outer(model, site, flag)
    {
        return None;
    }
    // catch (error) { sink.error(error); }
    let handler = t.handler.as_ref()?;
    let caught = plain_binding(&handler.param.as_ref()?.pattern)?;
    if !immutable(model, caught) {
        return None;
    }
    let [Statement::ExpressionStatement(fail)] = handler.body.body.as_slice() else {
        return None;
    };
    let Expression::CallExpression(fail) = &fail.expression else {
        return None;
    };
    if fail.optional {
        return None;
    }
    let sink = symbol(model, member(&fail.callee, "error")?)?;
    if !outer(model, site, sink) {
        return None;
    }
    let fail_args = call_arguments(fail);
    let [failed] = fail_args.as_slice() else {
        return None;
    };
    if symbol(model, failed)? != caught || sink == caught {
        return None;
    }
    // the record object: exactly {visible, timestamp}
    if record.properties.len() != 2 {
        return None;
    }
    let mut keys = BTreeSet::new();
    for property in &record.properties {
        let ObjectPropertyKind::ObjectProperty(p) = property else {
            return None;
        };
        if p.computed || p.method || p.kind != PropertyKind::Init {
            return None;
        }
        let key = property_key_name(&p.key)?;
        if !keys.insert(key.clone()) {
            return None;
        }
        match key.as_str() {
            "visible" => {
                let Expression::BinaryExpression(test) = &p.value else {
                    return None;
                };
                if test.operator.as_str() != "==="
                    || !matches!(&test.right, Expression::StringLiteral(s) if s.value == "visible")
                {
                    return None;
                }
                let Expression::StaticMemberExpression(state) = &test.left else {
                    return None;
                };
                let Expression::StaticMemberExpression(target) = &state.object else {
                    return None;
                };
                if state.optional
                    || target.optional
                    || state.property.name != "visibilityState"
                    || target.property.name != "target"
                    || symbol(model, &target.object)? != event
                {
                    return None;
                }
            }
            "timestamp" => {
                context_member_call(model, site, &p.value, "timeElapsedSinceInit")?;
            }
            _ => return None,
        }
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
    if visibility_recorder(model, site, function, body).is_some() {
        return vec![proposal(
            "recordVisibilityChangeSample",
            "events_visibility_method_alias",
            "owned try/catch: ++ringDirtyCounter; let receiver=ring; let method=ring.put; method.call(receiver, {visible: event.target.visibilityState === \"visible\", timestamp: sessionContext.timeElapsedSinceInit()}); ringDirtyFlag=true; catch(error) sink.error(error). The opaque event input is read only as data, the ring and its put method are captured scoped identities, and the clock is the bounded session-context provider member (no native callback or specific clock function is asserted)",
            f,
        )];
    }
    Vec::new()
}
