//! Exact own-body submit notification and forward/reset protocol.
//!
//! Names describe the literal methods/record keys and guarded data flow only.
//! Captured receivers are plain lexical bindings, not proved browser natives;
//! no installation, native origin, or lifecycle identity is claimed. Each
//! forwarder independently proves its whole body and its own published slot,
//! then links to the fully proved notifier by SymbolId. A sibling with similar
//! source cannot supply that link.
use crate::naming::support::seeded_flow::*;
use super::*;
use oxc_span::GetSpan;
mod parent;

fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(&b.body)
}

fn returns_void(s: &Statement<'_>) -> bool {
    matches!(s, Statement::ReturnStatement(r) if matches!(&r.argument,
        Some(Expression::UnaryExpression(u)) if u.operator.as_str() == "void" && number(&u.argument, 0.0)))
}

fn one_void(s: &Statement<'_>) -> bool {
    block(s).is_some_and(|b| matches!(b, [r] if returns_void(r)))
}

fn parts<'a>(model: &Model<'_, 'a>, site: FunctionSite) -> Option<(Symbol, &'a FunctionBody<'a>)> {
    let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
        return None;
    };
    let body = f.body.as_ref()?;
    if f.r#async || f.generator || !body.directives.is_empty() || !plain_params(model, &f.params) {
        return None;
    }
    let [p] = f.params.items.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(id) = &p.pattern else {
        return None;
    };
    Some((id.symbol_id.get()?, body))
}

fn captured_unchanged(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
) -> Option<Symbol> {
    let s = data_capture(model, site, e)?;
    unchanged(model, s).then_some(s)
}

// All writes of a published callback slot must be either its exact function
// declaration or null resets. The unique non-null write is a plain statement
// in the declaration's own lexical block, after that function. The slot itself
// is an earlier uninitialized plain lexical declaration. This also proves the
// self alias without treating the forward declaration as a native identity.
fn published_slot(
    model: &Model<'_, '_>,
    site: FunctionSite,
    slot: Symbol,
    resets: bool,
) -> Option<()> {
    if !capture_identity(model, site, slot, site.span.start) {
        return None;
    }
    let AstKind::VariableDeclarator(v) = model.nodes.kind(model.scoping.symbol_declaration(slot))
    else {
        return None;
    };
    if v.init.is_some() {
        return None;
    }
    let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
        return None;
    };
    let function_symbol = f.id.as_ref()?.symbol_id.get()?;
    let references = model
        .scoping
        .get_resolved_references(function_symbol)
        .count();
    if !unchanged(model, function_symbol)
        || !(references == 1
            || (!resets && references == 2 && parent::wiring(model, site, slot).is_some()))
    {
        return None;
    }
    let parent = model.nodes.parent_id(site.node_id);
    let mut publications = 0;
    let mut publication_end = 0;
    for reference in model
        .scoping
        .get_resolved_references(slot)
        .filter(|r| r.is_write())
    {
        let assignment_node = model.nodes.parent_id(reference.node_id());
        let AstKind::AssignmentExpression(a) = model.nodes.kind(assignment_node) else {
            return None;
        };
        if a.operator.as_str() != "=" || assign_symbol(model, a) != Some(slot) {
            return None;
        }
        if resets && matches!(&a.right, Expression::NullLiteral(_)) {
            continue;
        }
        if symbol(model, &a.right) != Some(function_symbol)
            || a.span.start <= site.span.end
            || !matches!(
                model.nodes.parent_kind(assignment_node),
                AstKind::ExpressionStatement(_)
            )
            || model
                .nodes
                .parent_id(model.nodes.parent_id(assignment_node))
                != parent
        {
            return None;
        }
        publications += 1;
        publication_end = a.span.end;
    }
    if model.scoping.get_resolved_references(slot).any(|r| {
        let span = model.nodes.get_node(r.node_id()).span();
        r.is_read() && span.start < publication_end && !site.span.contains_inclusive(span)
    }) {
        return None;
    }
    (publications == 1 && (resets || model.binding_writes(slot) == 1)).then_some(())
}

fn binary<'a>(e: &'a Expression<'a>, op: &str) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::BinaryExpression(b) = e else {
        return None;
    };
    (b.operator.as_str() == op).then_some((&b.left, &b.right))
}

fn logical<'a>(
    e: &'a Expression<'a>,
    op: &str,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::LogicalExpression(b) = e else {
        return None;
    };
    (b.operator.as_str() == op).then_some((&b.left, &b.right))
}

fn null_check(model: &Model<'_, '_>, e: &Expression<'_>, expected: Symbol) -> bool {
    binary(e, "!=").is_some_and(|(left, right)| {
        symbol(model, left) == Some(expected) && matches!(right, Expression::NullLiteral(_))
    })
}

fn member_symbol(model: &Model<'_, '_>, e: &Expression<'_>, key: &str, expected: Symbol) -> bool {
    member(e, key).is_some_and(|object| symbol(model, object) == Some(expected))
}

// The payload form is the enclosing record formal's exact target read, not a
// similarly named captured receiver or a declaration donated by a sibling.
fn form_origin(model: &Model<'_, '_>, site: FunctionSite, form: Symbol) -> Option<()> {
    let declaration = model.scoping.symbol_declaration(form);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return None;
    };
    let record = symbol(model, member(v.init.as_ref()?, "target")?)?;
    let mut node = model.nodes.parent_id(site.node_id);
    loop {
        match model.nodes.kind(node) {
            AstKind::Function(f) => {
                let enclosing = model.site_by_span(f.span)?;
                let (formal, body) = parts(model, enclosing)?;
                return (formal == record
                    && body.span.contains_inclusive(v.span)
                    && matches!(
                        model
                            .nodes
                            .kind(model.nodes.parent_id(model.nodes.parent_id(declaration))),
                        AstKind::FunctionBody(_)
                    ))
                .then_some(());
            }
            AstKind::Program(_) => return None,
            _ => node = model.nodes.parent_id(node),
        }
    }
}

// Complete notifier body, including the catch and exact self-removal identity.
fn notifier(model: &Model<'_, '_>, site: FunctionSite) -> Option<Symbol> {
    let (event, body) = parts(model, site)?;
    let [Statement::TryStatement(t)] = body.statements.as_slice() else {
        return None;
    };
    if t.finalizer.is_some() {
        return None;
    }
    let [remove, Statement::IfStatement(g)] = t.block.body.as_slice() else {
        return None;
    };
    let c = statement_call(remove)?;
    let receiver = data_capture(model, site, member(&c.callee, "removeEventListener")?)?;
    let args = arity(c, 2)?;
    if !string(args[0], "submit") {
        return None;
    }
    let slot = data_capture(model, site, args[1])?;
    published_slot(model, site, slot, false)?;
    if !unchanged(model, receiver) && parent::wiring(model, site, slot)?.receiver != receiver {
        return None;
    }
    let (non_null, prevented) = logical(&g.test, "&&")?;
    if !null_check(model, non_null, event)
        || !member_symbol(model, prevented, "defaultPrevented", event)
        || !one_void(&g.consequent)
    {
        return None;
    }
    let notified = block(g.alternate.as_ref()?)?;
    let (notify, ret) = match notified {
        [notify, ret] => (notify, ret),
        [notify, flag, ret] => {
            parent::flag_write(model, site, flag, true)?;
            parent::wiring(model, site, slot)?;
            (notify, ret)
        }
        _ => return None,
    };
    if !returns_void(ret) {
        return None;
    }
    let c = statement_call(notify)?;
    let registry = data_capture(model, site, member(&c.callee, "notify")?)?;
    let args = arity(c, 1)?;
    let Expression::ObjectExpression(object) = args[0] else {
        return None;
    };
    let [form, event_property] = object.properties.as_slice() else {
        return None;
    };
    let ObjectPropertyKind::ObjectProperty(form) = form else {
        return None;
    };
    let ObjectPropertyKind::ObjectProperty(event_property) = event_property else {
        return None;
    };
    if form.computed
        || form.shorthand
        || form.method
        || form.key.static_name().as_deref() != Some("form")
        || event_property.computed
        || event_property.shorthand
        || event_property.method
        || event_property.key.static_name().as_deref() != Some("event")
        || symbol(model, &event_property.value) != Some(event)
    {
        return None;
    }
    let form_symbol = captured_unchanged(model, site, &form.value)?;
    form_origin(model, site, form_symbol)?;
    let h = t.handler.as_ref()?;
    let BindingPattern::BindingIdentifier(error) = &h.param.as_ref()?.pattern else {
        return None;
    };
    let error = error.symbol_id.get()?;
    if !unchanged(model, error) {
        return None;
    }
    let [report, ret] = h.body.body.as_slice() else {
        return None;
    };
    if !returns_void(ret) {
        return None;
    }
    let c = statement_call(report)?;
    let errors = data_capture(model, site, member(&c.callee, "notify")?)?;
    if errors == registry {
        return None;
    }
    let args = arity(c, 1)?;
    let (prefix, caught) = binary(args[0], "+")?;
    if !matches!(prefix, Expression::StringLiteral(_)) || symbol(model, caught) != Some(error) {
        return None;
    }
    Some(form_symbol)
}

fn forwarder(model: &Model<'_, '_>, site: FunctionSite) -> Option<()> {
    let (event, body) = parts(model, site)?;
    let [Statement::IfStatement(g)] = body.statements.as_slice() else {
        return None;
    };
    let (wrong_type, wrong_target) = logical(&g.test, "||")?;
    let (event_type, submit) = binary(wrong_type, "!==")?;
    if !member_symbol(model, event_type, "type", event)
        || !string(submit, "submit")
        || !one_void(&g.consequent)
    {
        return None;
    }
    let (event_target, target) = binary(wrong_target, "!==")?;
    if !member_symbol(model, event_target, "target", event) {
        return None;
    }
    let target = captured_unchanged(model, site, target)?;
    let [forward, reset, ret] = block(g.alternate.as_ref()?)? else {
        return None;
    };
    if !returns_void(ret) {
        return None;
    }
    let c = statement_call(forward)?;
    let args = arity(c, 1)?;
    if symbol(model, args[0]) != Some(event) {
        return None;
    }
    // This protocol initializes its alias in a lexical block. The global
    // callable resolver deliberately rejects that origin; prove this one
    // explicit ordered publication locally without weakening that resolver.
    let alias = data_capture(model, site, &c.callee)?;
    if model.binding_writes(alias) != 1 {
        return None;
    }
    let value_reference = model
        .scoping
        .get_resolved_references(alias)
        .find(|r| r.is_write())?;
    let write_node = model.nodes.parent_id(value_reference.node_id());
    let AstKind::AssignmentExpression(write) = model.nodes.kind(write_node) else {
        return None;
    };
    if write.span.end >= site.span.start {
        return None;
    }
    let function_symbol = symbol(model, &write.right)?;
    let AstKind::Function(function) = model
        .nodes
        .kind(model.scoping.symbol_declaration(function_symbol))
    else {
        return None;
    };
    let callee = model.site_by_span(function.span)?;
    let alias_declaration = model.scoping.symbol_declaration(alias);
    if model
        .nodes
        .parent_id(model.nodes.parent_id(alias_declaration))
        != model.nodes.parent_id(callee.node_id)
    {
        return None;
    }
    if notifier(model, callee)? != target {
        return None;
    }
    let reset = assignment(reset)?;
    if !matches!(&reset.right, Expression::NullLiteral(_)) {
        return None;
    }
    let slot = assign_symbol(model, reset)?;
    published_slot(model, site, slot, true)?;
    Some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    if notifier(model, site).is_some() {
        return vec![proposal("removeSubmitListenerAndNotifyCapturedFormEvent", "submit_notification_protocol",
            "whole body removes the exact published self alias from a captured receiver's submit listener, skips defaultPrevented events, notifies a separate captured notify binding with exact form/event record and reports caught errors; native origin and lifecycle identity are unproved", f)];
    }
    if forwarder(model, site).is_some() {
        return vec![proposal("forwardMatchingSubmitAndClearCapturedSlot", "submit_notification_protocol",
            "whole body strictly guards submit type and captured target, forwards the same event to the independently proved linked notifier using that same form identity, then clears its own exactly published captured slot; native origin and lifecycle identity are unproved", f)];
    }
    if parent::trusted(model, site).is_some() {
        return vec![proposal("removeSubmitListenerAndNotifyFormForTrustedOrCancelableEvent", "submit_trusted_notification_protocol",
            "whole body rejects events that are neither trusted nor cancelable, strictly guards submit type and original form target, removes the linked notifier listener from the locally selected form ancestor, clears both published forwarders, notifies a distinct opaque captured sink with the form, and reports caught errors to the same opaque reporter; complete ordered parent publication and receiver selection are proved, external notify and browser native origins remain unproved", f)];
    }
    vec![]
}

#[cfg(test)]
mod family_tests;
