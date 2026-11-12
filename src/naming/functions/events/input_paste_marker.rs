//! Literal input protocol only. Captured ports have no proved native identity.
use crate::naming::support::interaction::*;
use super::*;
use oxc_semantic::{NodeId, SymbolId};
use oxc_span::GetSpan;

fn block<'a>(statement: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    match statement {
        Statement::BlockStatement(b) => Some(&b.body),
        _ => None,
    }
}

// The initialization must be a direct statement in an enclosing body, before
// the statement containing this callback. Hoisted functions in that chain may
// not escape before initialization. No sibling body supplies semantic proof.
fn ordered(model: &Model<'_, '_>, site: FunctionSite, statement: NodeId) -> Option<()> {
    let container = model.nodes.parent_id(statement);
    if !matches!(
        model.nodes.kind(container),
        AstKind::FunctionBody(_) | AstKind::Program(_)
    ) {
        return None;
    }
    let end = model.nodes.get_node(statement).span().end;
    let mut node = site.node_id;
    for _ in 0..64 {
        if let AstKind::Function(f) = model.nodes.kind(node) {
            if let Some(s) = f.id.as_ref().and_then(|id| id.symbol_id.get()) {
                if model
                    .scoping
                    .get_resolved_references(s)
                    .any(|r| r.is_read() && model.nodes.get_node(r.node_id()).span().start < end)
                {
                    return None;
                }
            }
        }
        let parent = model.nodes.parent_id(node);
        if parent == container {
            return (end <= model.nodes.get_node(node).span().start).then_some(());
        }
        if parent == node {
            return None;
        }
        node = parent;
    }
    None
}

fn capture(
    model: &Model<'_, '_>,
    site: FunctionSite,
    s: SymbolId,
    mutable: bool,
    scoped: bool,
) -> Option<()> {
    captured_symbol(model, site, s)?;
    let declaration = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return None;
    };
    if v.init.is_some() {
        if !mutable && model.binding_writes(s) != 0 {
            return None;
        }
        return ordered(model, site, model.nodes.parent_id(declaration));
    }
    // A receiver may instead have one straight-line assignment after its plain
    // initless declaration. Mutable data slots require declarator initializers.
    if (!scoped && mutable) || (!mutable && model.binding_writes(s) != 1) {
        return None;
    }
    let r = model.scoping.get_resolved_references(s).find(|r| {
        r.is_write()
            && (!scoped || model.nodes.get_node(r.node_id()).span().start < site.span.start)
    })?;
    let a_node = model.nodes.parent_id(r.node_id());
    let AstKind::AssignmentExpression(a) = model.nodes.kind(a_node) else {
        return None;
    };
    let statement = model.nodes.parent_id(a_node);
    if a.operator.as_str() != "="
        || target_symbol(model, &a.left) != Some(s)
        || !matches!(model.nodes.kind(statement), AstKind::ExpressionStatement(_))
        || model.nodes.parent_id(statement)
            != model.nodes.parent_id(model.nodes.parent_id(declaration))
        || v.span.end > a.span.start
    {
        return None;
    }
    ordered(model, site, statement)
}

fn protocol(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Function<'_>,
    scoped: bool,
) -> Option<()> {
    let body = f.body.as_ref()?;
    if f.r#async
        || f.generator
        || !body.directives.is_empty()
        || f.params.rest.is_some()
        || f.params.items.len() != 1
    {
        return None;
    }
    let event = plain_param(model, &f.params.items[0])?;
    let [Statement::TryStatement(t)] = body.statements.as_slice() else {
        return None;
    };
    if t.finalizer.is_some() {
        return None;
    }
    let [Statement::IfStatement(outer)] = t.block.body.as_slice() else {
        return None;
    };
    if outer.alternate.is_some() {
        return None;
    }
    let (tag, literal) = binary(&outer.test, "===")?;
    if !matches!(literal, Expression::StringLiteral(s) if s.value == "INPUT")
        || member(member(tag, "tagName")?, "target").and_then(|e| symbol(model, e)) != Some(event)
    {
        return None;
    }
    let [Statement::IfStatement(paste)] = block(&outer.consequent)? else {
        return None;
    };
    let (kind, literal) = binary(&paste.test, "===")?;
    if member(kind, "inputType").and_then(|e| symbol(model, e)) != Some(event)
        || !matches!(literal, Expression::StringLiteral(s) if s.value == "insertFromPaste")
    {
        return None;
    }
    let [increment] = block(&paste.consequent)? else {
        return None;
    };
    let Expression::UpdateExpression(update) = expression_statement(increment)? else {
        return None;
    };
    if update.operator.as_str() != "++" || !update.prefix {
        return None;
    }
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(counter) = &update.argument else {
        return None;
    };
    let counter = crate::analysis::identifier_symbol(counter, model.scoping)?;
    let Statement::IfStatement(inactive) = paste.alternate.as_ref()? else {
        return None;
    };
    if inactive.alternate.is_some() {
        return None;
    }
    let (target, active) = binary(&inactive.test, "!==")?;
    if member(target, "target").and_then(|e| symbol(model, e)) != Some(event) {
        return None;
    }
    let document = symbol(model, member(active, "activeElement")?)?;
    let [flag] = block(&inactive.consequent)? else {
        return None;
    };
    let flag = assignment(expression_statement(flag)?)?;
    if !boolean(&flag.right, true) {
        return None;
    }
    let flag = target_symbol(model, &flag.left)?;
    let handler = t.handler.as_ref()?;
    let error = plain_binding(model, &handler.param.as_ref()?.pattern)?;
    if model.binding_writes(error) != 0 {
        return None;
    }
    let [failed] = handler.body.body.as_slice() else {
        return None;
    };
    let failed = call(expression_statement(failed)?)?;
    let observer = method_receiver(model, failed, "error")?;
    if !matches!(arguments(failed)?.as_slice(), [e] if symbol(model, e) == Some(error)) {
        return None;
    }
    let identities = [event, counter, flag, document, observer, error];
    for (i, s) in identities.iter().enumerate() {
        if identities[..i].contains(s) {
            return None;
        }
    }
    capture(model, site, counter, true, scoped)?;
    capture(model, site, flag, true, scoped)?;
    capture(model, site, document, false, scoped)?;
    capture(model, site, observer, false, scoped)?;
    // Close the receiver-origin variant: an indexed factory port and the
    // document path of that same factory's context. This is opaque protocol
    // evidence, never proof that either supplied value is a browser native.
    let observer_value =
        crate::naming::support::selector_graphics::initialized_before(model, site, observer)?;
    let Expression::ComputedMemberExpression(port) = observer_value else {
        return None;
    };
    if port.optional || !(0..=4).any(|n| number(&port.expression, n as f64)) {
        return None;
    }
    let ports = model.formal_symbol_slot(symbol(model, &port.object)?)?;
    let document_value =
        crate::naming::support::selector_graphics::initialized_before(model, site, document)?;
    let context_value = member(member(document_value, "document")?, "global")?;
    let context_symbol = symbol(model, context_value)?;
    let context = if let Some(slot) = model.formal_symbol_slot(context_symbol) {
        slot
    } else {
        capture(model, site, context_symbol, false, scoped)?;
        let init = crate::naming::support::selector_graphics::initialized_before(
            model,
            site,
            context_symbol,
        )?;
        model.formal_symbol_slot(symbol(model, init)?)?
    };
    if ports.index != 0 || context.index != 1 || ports.owner != context.owner {
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
    // Keep the original direct proof ahead of the original hoisted proof. Only
    // after both fail may scoped assignment initialization broaden the protocol.
    // The strict mode retains the original mutable-declarator requirement and
    // receiver writer selection; metadata follows the proof that actually matched.
    if protocol(model, site, function, false).is_some() {
        return direct_proposal(f);
    }
    let hoisted = crate::naming::functions::observables::captured_observer_forwarding::paste::evaluate(model, site, f);
    if !hoisted.is_empty() {
        return hoisted;
    }
    if protocol(model, site, function, true).is_none() {
        return Vec::new();
    }
    direct_proposal(f)
}

fn direct_proposal(f: &Facts<'_>) -> Vec<Proposal> {
    vec![proposal(
        "countInputPasteAndMarkInactiveTarget",
        "events_input_paste_marker",
        "whole try/catch body reads the unchanged formal target.tagName strictly equal to INPUT; insertFromPaste strictly increments its own captured slot with prefix ++, otherwise target strictly unequal to captured activeElement assigns true to a separate slot; the exact caught error goes to the stable captured error receiver; scoped origins initialize before callback exposure, slot value types are unrestricted; native DOM identity and external event purpose are explicitly unproved",
        f,
    )]
}
