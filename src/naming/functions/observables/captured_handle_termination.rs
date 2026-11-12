//! Opaque terminate/complete cleanup. This establishes no native Worker,
//! WebGL or Observable identity. All slot readers are synchronous-capable.
use crate::naming::*;
use oxc_span::GetSpan;

fn symbol(m: &Model<'_, '_>, e: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e.without_parentheses(), m.scoping)
}
fn plain(m: &Model<'_, '_>, s: SymbolId, writes: usize) -> bool {
    m.binding_writes(s) == writes && m.scoping.symbol_redeclarations(s).is_empty()
}
fn expression<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    Some(&s.expression)
}
fn method(m: &Model<'_, '_>, s: &Statement<'_>, key: &str) -> Option<SymbolId> {
    let Expression::CallExpression(c) = expression(s)?.without_parentheses() else {
        return None;
    };
    let Expression::StaticMemberExpression(member) = c.callee.without_parentheses() else {
        return None;
    };
    if c.optional || !c.arguments.is_empty() || member.optional || member.property.name != key {
        return None;
    }
    symbol(m, &member.object)
}
fn variable<'a>(m: &Model<'_, 'a>, s: SymbolId) -> Option<(NodeId, &'a VariableDeclarator<'a>)> {
    let id = m.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = m.nodes.kind(id) else {
        return None;
    };
    if !matches!(&v.id, BindingPattern::BindingIdentifier(b) if b.symbol_id.get()==Some(s))
        || !matches!(m.nodes.parent_kind(id), AstKind::VariableDeclaration(_))
    {
        return None;
    }
    Some((id, v))
}
fn body_of_variable(m: &Model<'_, '_>, id: NodeId) -> Option<NodeId> {
    let body = m.nodes.parent_id(m.nodes.parent_id(id));
    matches!(m.nodes.kind(body), AstKind::FunctionBody(_)).then_some(body)
}
// Return the direct body statement containing a node, refusing intervening
// closures. Lexical ordering is used only between direct sequential statements.
fn statement_in(m: &Model<'_, '_>, mut id: NodeId, body: NodeId) -> Option<NodeId> {
    for _ in 0..128 {
        let parent = m.nodes.parent_id(id);
        if parent == body {
            return Some(id);
        }
        if parent == id
            || matches!(
                m.nodes.kind(id),
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
            )
        {
            return None;
        }
        id = parent;
    }
    None
}
fn assignment_to<'a>(
    m: &Model<'_, 'a>,
    id: NodeId,
    target: SymbolId,
) -> Option<&'a AssignmentExpression<'a>> {
    let AstKind::AssignmentExpression(a) = m.nodes.parent_kind(id) else {
        return None;
    };
    if a.operator != AssignmentOperator::Assign
        || !matches!(&a.left, AssignmentTarget::AssignmentTargetIdentifier(i)
            if crate::analysis::identifier_symbol(i,m.scoping)==Some(target))
    {
        return None;
    }
    Some(a)
}
// A cleanup slot is only read by `if (typeof slot === "function") { slot(); }`.
// No callback scheduling assumption is needed: before the first store the
// declared slot is undefined, and every subsequent value is this ready closure.
fn guarded_reader(m: &Model<'_, '_>, id: NodeId, slot: SymbolId) -> bool {
    let parent = m.nodes.parent_id(id);
    let branch = match m.nodes.kind(parent) {
        AstKind::UnaryExpression(u) if u.operator.as_str() == "typeof" => {
            let test = m.nodes.parent_id(parent);
            let AstKind::BinaryExpression(b) = m.nodes.kind(test) else {
                return false;
            };
            if b.operator.as_str() != "==="
                || b.left.span() != u.span
                || !matches!(&b.right, Expression::StringLiteral(s) if s.value=="function")
            {
                return false;
            }
            m.nodes.parent_id(test)
        }
        AstKind::CallExpression(c)
            if !c.optional && c.arguments.is_empty() && symbol(m, &c.callee) == Some(slot) =>
        {
            let statement = m.nodes.parent_id(parent);
            if !matches!(m.nodes.kind(statement), AstKind::ExpressionStatement(_)) {
                return false;
            }
            let block = m.nodes.parent_id(statement);
            if !matches!(m.nodes.kind(block), AstKind::BlockStatement(_)) {
                return false;
            }
            m.nodes.parent_id(block)
        }
        _ => return false,
    };
    let AstKind::IfStatement(b) = m.nodes.kind(branch) else {
        return false;
    };
    let Expression::BinaryExpression(test) = &b.test else {
        return false;
    };
    let Expression::UnaryExpression(u) = &test.left else {
        return false;
    };
    let Statement::BlockStatement(block) = &b.consequent else {
        return false;
    };
    b.alternate.is_none()
        && test.operator.as_str() == "==="
        && u.operator.as_str() == "typeof"
        && symbol(m, &u.argument) == Some(slot)
        && matches!(&test.right,Expression::StringLiteral(s) if s.value=="function")
        && matches!(block.body.as_slice(),[s] if matches!(expression(s),Some(Expression::CallExpression(c))
            if !c.optional && c.arguments.is_empty() && symbol(m,&c.callee)==Some(slot)))
}
fn prove(m: &Model<'_, '_>, site: FunctionSite) -> Option<()> {
    let AstKind::Function(f) = m.nodes.kind(site.node_id) else {
        return None;
    };
    if f.r#type != FunctionType::FunctionDeclaration
        || f.r#async
        || f.generator
        || !f.params.items.is_empty()
        || f.params.rest.is_some()
    {
        return None;
    }
    let body = f.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let [Statement::IfStatement(guard), complete] = body.statements.as_slice() else {
        return None;
    };
    if guard.alternate.is_some() {
        return None;
    }
    let Expression::BinaryExpression(test) = &guard.test else {
        return None;
    };
    if test.operator.as_str() != "!=" || !test.right.is_null() {
        return None;
    }
    let handle = symbol(m, &test.left)?;
    let Statement::BlockStatement(then) = &guard.consequent else {
        return None;
    };
    let [terminate] = then.body.as_slice() else {
        return None;
    };
    if method(m, terminate, "terminate") != Some(handle) {
        return None;
    }
    let observer = method(m, complete, "complete")?;
    if observer == handle {
        return None;
    }
    let binding = f.id.as_ref()?.symbol_id.get()?;
    if site.binding_symbol != Some(binding) || !plain(m, binding, 0) {
        return None;
    }
    let producer_body = m.nodes.parent_id(site.node_id);
    if !matches!(m.nodes.kind(producer_body), AstKind::FunctionBody(_)) {
        return None;
    }
    let producer = m.nodes.parent_id(producer_body);
    let AstKind::ArrowFunctionExpression(arrow) = m.nodes.kind(producer) else {
        return None;
    };
    if arrow.r#async
        || arrow.expression
        || !arrow.params.items.is_empty()
        || arrow.params.rest.is_some()
        || !arrow.body.directives.is_empty()
    {
        return None;
    }

    // The nullable handle starts as lexical undefined. Its sole possible store
    // produces an object via new, in this same invocation, before exposure.
    let (handle_decl, handle_var) = variable(m, handle)?;
    if handle_var.init.is_some()
        || !plain(m, handle, 1)
        || body_of_variable(m, handle_decl) != Some(producer_body)
    {
        return None;
    }
    let writer = m
        .scoping
        .get_resolved_references(handle)
        .find(|r| r.is_write())?;
    let write = assignment_to(m, writer.node_id(), handle)?;
    let Expression::NewExpression(new) = &write.right else {
        return None;
    };
    if new
        .arguments
        .iter()
        .any(|a| matches!(a, Argument::SpreadElement(_)))
    {
        return None;
    }
    // Constructor identity is deliberately opaque; still require an initialized,
    // unwritten captured member value rather than a guessed native global.
    let ctor = symbol(m, &new.callee)?;
    let (ctor_decl, ctor_var) = variable(m, ctor)?;
    let Expression::StaticMemberExpression(ctor_member) = ctor_var.init.as_ref()? else {
        return None;
    };
    if ctor_member.optional || !plain(m, ctor, 0) {
        return None;
    }
    let write_node = m.nodes.parent_id(writer.node_id());
    let write_top = statement_in(m, write_node, producer_body)?;
    // Reject nested callbacks, looped stores, and finally writes. Only a single
    // conditional constructor store in a top-level try is supported.
    let AstKind::TryStatement(t) = m.nodes.kind(write_top) else {
        return None;
    };
    if handle_var.span.end > t.span.start
        || t.finalizer.is_some()
        || write.span.start < t.block.span.start
        || write.span.end > t.block.span.end
    {
        return None;
    }
    let mut ancestor = write_node;
    while ancestor != write_top {
        if matches!(
            m.nodes.kind(ancestor),
            AstKind::ForStatement(_)
                | AstKind::ForInStatement(_)
                | AstKind::ForOfStatement(_)
                | AstKind::WhileStatement(_)
                | AstKind::DoWhileStatement(_)
        ) {
            return None;
        }
        ancestor = m.nodes.parent_id(ancestor);
    }

    // Exactly one function-value exposure: a direct assignment into an outer
    // lexical cleanup slot, after the entire constructor try has completed.
    let mut refs = m.scoping.get_resolved_references(binding);
    let exposure = refs.next()?;
    if refs.next().is_some() {
        return None;
    }
    let AstKind::AssignmentExpression(store) = m.nodes.parent_kind(exposure.node_id()) else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(left) = &store.left else {
        return None;
    };
    let slot = crate::analysis::identifier_symbol(left, m.scoping)?;
    if store.operator != AssignmentOperator::Assign
        || symbol(m, &store.right) != Some(binding)
        || !plain(m, slot, 1)
    {
        return None;
    }
    let store_node = m.nodes.parent_id(exposure.node_id());
    let store_statement = m.nodes.parent_id(store_node);
    if !matches!(
        m.nodes.kind(store_statement),
        AstKind::ExpressionStatement(_)
    ) || m.nodes.parent_id(store_statement) != producer_body
        || store.span.start < m.nodes.kind(write_top).span().end
        || store.span.start < handle_var.span.end
    {
        return None;
    }
    let (slot_decl, slot_var) = variable(m, slot)?;
    let outer = body_of_variable(m, slot_decl)?;
    if slot_var.init.is_some() || outer == producer_body {
        return None;
    }
    for r in m.scoping.get_resolved_references(slot) {
        if r.is_write() {
            if m.nodes.parent_id(r.node_id()) != store_node {
                return None;
            }
        } else if !guarded_reader(m, r.node_id(), slot) {
            return None;
        }
    }

    // Observer is an opaque fixed-index argument-bundle value, captured once.
    // Its initializer and the constructor origin both execute before the
    // producer's registration call, even if that registration invokes inline.
    let (observer_decl, observer_var) = variable(m, observer)?;
    if !plain(m, observer, 0)
        || body_of_variable(m, observer_decl) != Some(outer)
        || body_of_variable(m, ctor_decl) != Some(outer)
    {
        return None;
    }
    let Expression::ComputedMemberExpression(member) = observer_var.init.as_ref()? else {
        return None;
    };
    let Expression::NumericLiteral(index) = &member.expression else {
        return None;
    };
    if member.optional || index.value < 0.0 || index.value.fract() != 0.0 {
        return None;
    }
    let bundle = symbol(m, &member.object)?;
    if !plain(m, bundle, 0)
        || !matches!(
            m.nodes.kind(m.scoping.symbol_declaration(bundle)),
            AstKind::FormalParameter(_)
        )
    {
        return None;
    }
    let call_node = m.nodes.parent_id(producer);
    let AstKind::CallExpression(registration) = m.nodes.kind(call_node) else {
        return None;
    };
    if registration.optional
        || !matches!(registration.arguments.first(),Some(Argument::ArrowFunctionExpression(a)) if a.span==arrow.span)
    {
        return None;
    }
    let registrar = symbol(m, &registration.callee)?;
    if !plain(m, registrar, 0)
        || !matches!(
            m.nodes.kind(m.scoping.symbol_declaration(registrar)),
            AstKind::FormalParameter(_)
        )
    {
        return None;
    }
    let registration_statement = m.nodes.parent_id(call_node);
    if !matches!(
        m.nodes.kind(registration_statement),
        AstKind::ExpressionStatement(_)
    ) || m.nodes.parent_id(registration_statement) != outer
        || registration.span.start < observer_var.span.end
        || registration.span.start < ctor_var.span.end
        || registration.span.start < slot_var.span.end
    {
        return None;
    }
    Some(())
}
pub(in crate::naming) fn evaluate(m: &Model<'_, '_>, site: FunctionSite, facts: &Facts<'_>) -> Vec<Proposal> {
    if prove(m, site).is_none() {
        return vec![];
    }
    vec![proposal("terminateCapturedHandleIfPresentThenComplete", "guarded_handle_completion",
        "whole zero-argument declaration: if captured handle != null call handle.terminate() with no arguments, then unconditionally call distinct captured value.complete(); no return, extra effect or exception interception; lexical handle starts undefined and its only possible store is new before the sole cleanup-slot exposure; fixed-index bundle receiver and opaque constructor are initialized without later writes before synchronous-capable producer registration; every cleanup-slot read is an exact typeof-function guarded zero-argument call; no native Worker/WebGL/Observable identity asserted",facts)]
}

#[cfg(test)]
mod tests;
