//! A neutral own-protocol proof. External hook and inherited native purpose are
//! deliberately unproved; the typed lazy-registry proof always runs first.
use super::*;

fn base_identity(model: &Model<'_, '_>, mut s: SymbolId, mut before: u32) -> Option<SymbolId> {
    let mut seen = BTreeSet::new();
    for _ in 0..16 {
        if !seen.insert(s) {
            return None;
        }
        match model.nodes.kind(model.scoping.symbol_declaration(s)) {
            AstKind::Class(c) => return (fixed(model, s) && c.span.end < before).then_some(s),
            AstKind::Function(_) => return fixed(model, s).then_some(s),
            _ => {
                let value = origin(model, s, before)?;
                before = value.span().start;
                s = symbol(model, value)?;
            }
        }
    }
    None
}

pub(super) fn prove<'a>(
    model: &Model<'_, 'a>,
    factory: FunctionSite,
    f: &'a Function<'a>,
) -> Option<FinderClaim> {
    let ps = params(model, f)?;
    let [base] = ps.as_slice() else {
        return None;
    };
    let class = returned_class(model, f)?;
    if symbol(model, class.super_class.as_ref()?) != Some(*base) {
        return None;
    }
    let (ctor, register) = methods(class)?;
    forwards_arguments(model, ctor)?;
    let ps = params(model, register)?;
    let [formal] = ps.as_slice() else {
        return None;
    };
    let [hook, forward, Statement::ReturnStatement(ret)] = body(register)?.statements.as_slice()
    else {
        return None;
    };
    let hook = call(hook)?;
    let capture = symbol(model, &hook.callee)?;
    // The callee must be an actual enclosing capture, never a method/factory
    // parameter or a same-spelled local. No spelling supplies identity evidence.
    if !hook.arguments.is_empty()
        || capture == *base
        || capture == *formal
        || f.span
            .contains_inclusive(model.scoping.symbol_span(capture))
    {
        return None;
    }
    let forward = call(forward)?;
    let (receiver, "register") = member(&forward.callee)? else {
        return None;
    };
    if !matches!(receiver.without_parentheses(), Expression::Super(_))
        || forward.arguments.len() != 1
        || symbol(model, forward.arguments[0].as_expression()?) != Some(*formal)
        || !matches!(
            ret.argument.as_ref()?.without_parentheses(),
            Expression::ThisExpression(_)
        )
    {
        return None;
    }
    let factory_symbol = factory.binding_symbol?;
    if !fixed(model, factory_symbol)
        || member_mutated(model, factory_symbol)
        || capture == factory_symbol
    {
        return None;
    }
    let mut evidence = Vec::new();
    for reference in model.scoping.get_resolved_references(factory_symbol) {
        if !reference.is_read() {
            return None;
        }
        let AstKind::CallExpression(c) = model.nodes.parent_kind(reference.node_id()) else {
            return None;
        };
        if c.optional || symbol(model, &c.callee) != Some(factory_symbol) || c.arguments.len() != 1
        {
            return None;
        }
        let actual = symbol(model, c.arguments[0].as_expression()?)?;
        let actual_base = base_identity(model, actual, c.span.start)?;
        let hook_site = stable_function(model, &hook.callee, c.span.start)?;
        if capture == actual
            || capture == actual_base
            || model.scoping.symbol_declaration(actual_base) == hook_site.node_id
            || hook_site.span == factory.span
        {
            return None;
        }
        let call_id = model.nodes.parent_id(reference.node_id());
        let AstKind::VariableDeclarator(result) = model.nodes.parent_kind(call_id) else {
            return None;
        };
        if !result.init.as_ref().is_some_and(|e| e.span() == c.span) {
            return None;
        }
        let BindingPattern::BindingIdentifier(id) = &result.id else {
            return None;
        };
        if constructor_alias(model, id.symbol_id.get()?, c.span.end, &mut BTreeSet::new())? == 0 {
            return None;
        }
        evidence.push(format!("exact factory {:?}, returned class {:?}, ordered call {:?}; scoped captured hook SymbolId {:?} resolves to {:?}; distinct actual base SymbolId {:?}; result aliases are immutable and constructed", factory.span, class.span, c.span, capture, hook_site.span, actual_base));
    }
    if evidence.is_empty() {
        return None;
    }
    evidence.push("complete own protocol only: constructor super(...arguments); register(formal0) invokes the captured zero-argument hook, then super.register(formal0), discards the base return and returns this; hook exceptions propagate and skip the base call; no extra class/factory/method effects; external hook purpose and inherited native/installer purpose explicitly unproved".into());
    Some(FinderClaim::ClassBinding {
        role: "owned_pre_register_hook_forwarder_class",
        class_span: class.span,
        confidence: Confidence::High,
        evidence,
    })
}

#[cfg(test)]
#[path = "pre_register_forwarder_tests.rs"]
mod tests;
