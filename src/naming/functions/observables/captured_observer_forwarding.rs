//! Whole-body captured observer method forwarding. Receiver identity stays opaque.
use crate::naming::shared::callable_origin_precedes;
use crate::naming::support::installer_events::global_path;
use crate::naming::*;
use oxc_span::GetSpan;

fn symbol(m: &Model<'_, '_>, e: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e.without_parentheses(), m.scoping)
}

// A store is an executed direct-body initializer/assignment, never a deferred
// writer found somewhere inside the initializer's source range.
fn ready(m: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<()> {
    let s = symbol(m, e)?;
    if site.span.contains_inclusive(m.scoping.symbol_span(s))
        || !m.scoping.symbol_redeclarations(s).is_empty()
    {
        return None;
    }
    let d = m.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = m.nodes.kind(d) else {
        return None;
    };
    if !matches!(&v.id, BindingPattern::BindingIdentifier(i) if i.symbol_id.get()==Some(s)) {
        return None;
    }
    let decl = m.nodes.parent_id(d);
    let owner = m.nodes.parent_id(decl);
    if !matches!(m.nodes.kind(decl), AstKind::VariableDeclaration(_))
        || !matches!(
            m.nodes.kind(owner),
            AstKind::FunctionBody(_) | AstKind::Program(_)
        )
    {
        return None;
    }
    if let Some(value) = v.init.as_ref() {
        value_ready(m, value, 0)?;
    }
    let end = if v.init.is_some() && m.binding_writes(s) == 0 {
        v.span.end
    } else if v.init.is_none() && m.binding_writes(s) == 1 {
        let r = m
            .scoping
            .get_resolved_references(s)
            .find(|r| r.is_write())?;
        let a_node = m.nodes.parent_id(r.node_id());
        let AstKind::AssignmentExpression(a) = m.nodes.kind(a_node) else {
            return None;
        };
        let statement = m.nodes.parent_id(a_node);
        if a.operator != AssignmentOperator::Assign
            || !matches!(&a.left, AssignmentTarget::AssignmentTargetIdentifier(i) if crate::analysis::identifier_symbol(i,m.scoping)==Some(s))
            || !matches!(m.nodes.kind(statement), AstKind::ExpressionStatement(_))
            || m.nodes.parent_id(statement) != owner
            || a.span.start < v.span.end
        {
            return None;
        }
        value_ready(m, &a.right, 0)?;
        a.span.end
    } else {
        return None;
    };
    let Expression::Identifier(i) = e.without_parentheses() else {
        return None;
    };
    let use_node = m.scoping.get_reference(i.reference_id.get()?).node_id();
    callable_origin_precedes(m, owner, end, use_node, None, 0, true).then_some(())
}
fn member<'a>(e: &'a Expression<'a>) -> Option<(&'a Expression<'a>, &'a str)> {
    match e.without_parentheses() {
        Expression::StaticMemberExpression(m) if !m.optional => {
            Some((&m.object, m.property.name.as_str()))
        }
        Expression::ComputedMemberExpression(m) if !m.optional => {
            let Expression::StringLiteral(k) = m.expression.without_parentheses() else {
                return None;
            };
            Some((&m.object, k.value.as_str()))
        }
        _ => None,
    }
}
fn prove(m: &Model<'_, '_>, site: FunctionSite) -> Option<&'static str> {
    let AstKind::Function(f) = m.nodes.kind(site.node_id) else {
        return None;
    };
    let b = f.body.as_ref()?;
    if f.r#async || f.generator || !b.directives.is_empty() || f.params.rest.is_some() {
        return None;
    }
    let mut params = Vec::new();
    for p in &f.params.items {
        let BindingPattern::BindingIdentifier(i) = &p.pattern else {
            return None;
        };
        let s = i.symbol_id.get()?;
        if p.initializer.is_some()
            || m.binding_writes(s) != 0
            || !m.scoping.symbol_redeclarations(s).is_empty()
        {
            return None;
        }
        params.push(s);
    }
    let (expression, discarded) = match b.statements.as_slice() {
        [Statement::ReturnStatement(ret)] => (ret.argument.as_ref()?, false),
        [Statement::ExpressionStatement(statement)] => {
            // This separate role describes an externally bound declaration with
            // an actual exposure, never an unused/private expression self name.
            let binding = f.id.as_ref()?.symbol_id.get()?;
            if f.r#type != FunctionType::FunctionDeclaration
                || m.scoping.get_resolved_references(binding).next().is_none()
            {
                return None;
            }
            (&statement.expression, true)
        }
        _ => return None,
    };
    let Expression::CallExpression(c) = expression.without_parentheses() else {
        return None;
    };
    if c.optional {
        return None;
    }
    let (receiver, key) = member(&c.callee)?;
    ready(m, site, receiver)?;
    if discarded {
        return match (key, params.as_slice(), c.arguments.as_slice()) {
            ("next", [s], [a]) if symbol(m, a.as_expression()?) == Some(*s) => {
                Some("callCapturedNext")
            }
            _ => None,
        };
    }
    match (key, params.as_slice(), c.arguments.as_slice()) {

        ("next", [s], [a]) | ("error", [s], [a]) => {
            if symbol(m, a.as_expression()?) != Some(*s) {
                return None;
            }
            Some(if key == "next" {
                "forwardValueToCapturedObserver"
            } else {
                "forwardErrorToCapturedObserver"
            })
        }
        ("complete", [], []) => Some("completeCapturedObserver"),
        ("forEach", [], [Argument::ArrowFunctionExpression(a)]) => {
            if a.r#async
                || !a.expression
                || a.params.rest.is_some()
                || a.params.items.len() != 1
                || !a.body.directives.is_empty()
            {
                return None;
            }
            let p = &a.params.items[0];
            let BindingPattern::BindingIdentifier(i) = &p.pattern else {
                return None;
            };
            let s = i.symbol_id.get()?;
            if p.initializer.is_some() || m.binding_writes(s) != 0 {
                return None;
            }
            let [Statement::ExpressionStatement(st)] = a.body.statements.as_slice() else {
                return None;
            };
            let Expression::CallExpression(call) = st.expression.without_parentheses() else {
                return None;
            };
            let (obj, method) = member(&call.callee)?;
            (!call.optional
                && call.arguments.is_empty()
                && method == "unsubscribe"
                && symbol(m, obj) == Some(s))
            .then_some("unsubscribeEachCapturedHandle")
        }
        _ => None,
    }
}
pub(in crate::naming) fn evaluate(
    m: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    if let Some(name) = prove(m, site) {
        if name == "callCapturedNext" {
            return vec![proposal(name,"shared_captured_observer_forwarding","whole owned body calls the captured receiver's literal next method with its unchanged sole formal argument and discards the result, returning implicit undefined; sole direct executed capture origin and alias producers reach every callable exposure; receiver, method implementation and native purpose remain opaque",f)];
        }

        return vec![proposal(name,"shared_captured_observer_forwarding","whole owned body returns exactly the captured receiver method result; unchanged arguments and receiver; sole direct executed capture origin and alias producers reach every callable exposure; receiver/native identity remains opaque",f)];
    }
    if let Some(name) = counted_getter(m, site) {
        return vec![proposal(name,"counted_function_to_string_getter","whole own body prefix increments its sole-writer captured zero-initialized counter then reads unresolved-global Function.prototype.toString; initializer reaches every callable exposure; no invocation or native identity assumed",f)];
    }
    paste::evaluate(m, site, f)
}

fn counted_getter(m: &Model<'_, '_>, site: FunctionSite) -> Option<&'static str> {
    let AstKind::Function(f) = m.nodes.kind(site.node_id) else {
        return None;
    };
    let b = f.body.as_ref()?;
    if f.r#async
        || f.generator
        || !b.directives.is_empty()
        || !f.params.items.is_empty()
        || f.params.rest.is_some()
    {
        return None;
    }
    let [Statement::ExpressionStatement(st), Statement::ReturnStatement(ret)] =
        b.statements.as_slice()
    else {
        return None;
    };
    let Expression::UpdateExpression(update) = &st.expression else {
        return None;
    };
    if !update.prefix || update.operator.as_str() != "++" {
        return None;
    }
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &update.argument else {
        return None;
    };
    let s = crate::analysis::identifier_symbol(id, m.scoping)?;
    if site.span.contains_inclusive(m.scoping.symbol_span(s))
        || !m.scoping.symbol_redeclarations(s).is_empty()
        || m.binding_writes(s) != 1
    {
        return None;
    }
    let d = m.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = m.nodes.kind(d) else {
        return None;
    };
    if !matches!(&v.id,BindingPattern::BindingIdentifier(i) if i.symbol_id.get()==Some(s))
        || !matches!(&v.init,Some(Expression::NumericLiteral(n)) if n.value==0.0)
    {
        return None;
    }
    let declaration = m.nodes.parent_id(d);
    let owner = m.nodes.parent_id(declaration);
    if !matches!(m.nodes.kind(declaration), AstKind::VariableDeclaration(_))
        || !matches!(
            m.nodes.kind(owner),
            AstKind::FunctionBody(_) | AstKind::Program(_)
        )
    {
        return None;
    }
    let writes: Vec<_> = m
        .scoping
        .get_resolved_references(s)
        .filter(|r| r.is_write())
        .collect();
    if writes.len() != 1 || m.nodes.kind(writes[0].node_id()).span() != id.span {
        return None;
    }
    if !callable_origin_precedes(m, owner, v.span.end, writes[0].node_id(), None, 0, true) {
        return None;
    }
    let result = ret.argument.as_ref()?;
    if global_path(m, result)?.as_slice() != ["Function", "prototype", "toString"] {
        return None;
    }
    Some("countAccessAndGetFunctionToString")
}

// Resolve alias producers at the actual executed read, not the later callback.
fn value_ready(m: &Model<'_, '_>, e: &Expression<'_>, depth: usize) -> Option<()> {
    if depth > 12 {
        return None;
    }
    match e.without_parentheses() {
        Expression::Identifier(i) => {
            let s = crate::analysis::identifier_symbol(i, m.scoping)?;
            if m.binding_writes(s) != 0 || !m.scoping.symbol_redeclarations(s).is_empty() {
                return None;
            }
            let d = m.scoping.symbol_declaration(s);
            if matches!(m.nodes.kind(d), AstKind::FormalParameter(_)) {
                return Some(());
            }
            let AstKind::VariableDeclarator(v) = m.nodes.kind(d) else {
                return None;
            };
            let declaration = m.nodes.parent_id(d);
            let owner = m.nodes.parent_id(declaration);
            if !matches!(m.nodes.kind(declaration), AstKind::VariableDeclaration(_))
                || !matches!(
                    m.nodes.kind(owner),
                    AstKind::FunctionBody(_) | AstKind::Program(_)
                )
            {
                return None;
            }
            let node = m.scoping.get_reference(i.reference_id.get()?).node_id();
            if !callable_origin_precedes(m, owner, v.span.end, node, None, 0, false) {
                return None;
            }
            value_ready(m, v.init.as_ref()?, depth + 1)
        }
        Expression::ArrayExpression(a) if a.elements.is_empty() => Some(()),
        Expression::StaticMemberExpression(member) if !member.optional => {
            value_ready(m, &member.object, depth + 1)
        }
        Expression::ComputedMemberExpression(member) if !member.optional => {
            value_ready(m, &member.object, depth + 1)?;
            matches!(
                member.expression.without_parentheses(),
                Expression::NumericLiteral(_) | Expression::StringLiteral(_)
            )
            .then_some(())
        }
        _ => None,
    }
}

pub(in crate::naming) mod paste {
    use crate::naming::support::interaction::{
        arguments, assignment, binary, boolean, call, captured_symbol, expression_statement,
        member, method_receiver, plain_binding, plain_param, symbol, target_symbol,
    };
    use super::*;
    fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
        if let Statement::BlockStatement(b) = s {
            Some(&b.body)
        } else {
            None
        }
    }
    fn protocol(model: &Model<'_, '_>, site: FunctionSite, f: &Function<'_>) -> Option<()> {
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
            || member(member(tag, "tagName")?, "target").and_then(|e| symbol(model, e))
                != Some(event)
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
        capture(model, site, counter, Some(false))?;
        capture(model, site, flag, Some(true))?;
        capture(model, site, document, None)?;
        capture(model, site, observer, None)?;
        Some(())
    }
    fn capture(
        m: &Model<'_, '_>,
        site: FunctionSite,
        s: SymbolId,
        mutable: Option<bool>,
    ) -> Option<()> {
        captured_symbol(m, site, s)?;
        let own: Vec<_> = m
            .scoping
            .get_resolved_references(s)
            .filter(|r| {
                r.is_write()
                    && site
                        .span
                        .contains_inclusive(m.nodes.kind(r.node_id()).span())
            })
            .collect();
        if own.len() != usize::from(mutable.is_some()) {
            return None;
        }
        let external: Vec<_> = m
            .scoping
            .get_resolved_references(s)
            .filter(|r| {
                r.is_write()
                    && !site
                        .span
                        .contains_inclusive(m.nodes.kind(r.node_id()).span())
            })
            .collect();
        let d = m.scoping.symbol_declaration(s);
        let AstKind::VariableDeclarator(v) = m.nodes.kind(d) else {
            return None;
        };
        let declaration = m.nodes.parent_id(d);
        let owner = m.nodes.parent_id(declaration);
        if !matches!(m.nodes.kind(declaration), AstKind::VariableDeclaration(_))
            || !matches!(
                m.nodes.kind(owner),
                AstKind::FunctionBody(_) | AstKind::Program(_)
            )
        {
            return None;
        }
        let (value, end) = if let Some(init) = v.init.as_ref() {
            if !external.is_empty() {
                return None;
            }
            (init, v.span.end)
        } else {
            let [writer] = external.as_slice() else {
                return None;
            };
            let a_node = m.nodes.parent_id(writer.node_id());
            let AstKind::AssignmentExpression(a) = m.nodes.kind(a_node) else {
                return None;
            };
            let statement = m.nodes.parent_id(a_node);
            if a.operator != AssignmentOperator::Assign
                || target_symbol(m, &a.left) != Some(s)
                || !matches!(m.nodes.kind(statement), AstKind::ExpressionStatement(_))
                || m.nodes.parent_id(statement) != owner
                || a.span.start < v.span.end
            {
                return None;
            }
            (&a.right, a.span.end)
        };
        if let Some(flag) = mutable {
            if flag {
                if !matches!(value,Expression::BooleanLiteral(b) if !b.value) {
                    return None;
                }
            } else if !matches!(value,Expression::NumericLiteral(n) if n.value==0.0) {
                return None;
            }
        } else {
            value_ready(m, value, 0)?;
        }
        // Every reference of this captured slot inside the target must occur after
        // the exact setup store, following the callable's actual exposures.
        m.scoping
            .get_resolved_references(s)
            .filter(|r| {
                site.span
                    .contains_inclusive(m.nodes.kind(r.node_id()).span())
            })
            .all(|r| callable_origin_precedes(m, owner, end, r.node_id(), None, 0, true))
            .then_some(())
    }
    pub(in crate::naming) fn evaluate(m: &Model<'_, '_>, site: FunctionSite, f: &Facts<'_>) -> Vec<Proposal> {
        let AstKind::Function(function) = m.nodes.kind(site.node_id) else {
            return vec![];
        };
        if protocol(m, site, function).is_none() {
            return vec![];
        }
        vec![proposal("countInputPasteAndMarkInactiveTarget","events_input_paste_marker_hoisted_capture","whole exact INPUT/insertFromPaste try/catch protocol; unchanged event, own prefix counter increment and inactive-target true store, same catch error forwarded; every writer accounted for and exact executed initialization reaches callable exposures; receivers remain opaque, no native DOM/event identity claimed",f)]
    }
}
