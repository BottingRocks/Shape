//! Bounded, owned-body subscription and callback adapter proofs.
use super::*;
use oxc_span::GetSpan;

type Symbol = oxc_semantic::SymbolId;

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(e, model.scoping)
}

fn unchanged(model: &Model<'_, '_>, s: Symbol) -> bool {
    model.binding_writes(s) == 0 && model.scoping.symbol_redeclarations(s).is_empty()
}

fn capture(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<Symbol> {
    let s = symbol(model, e)?;
    (unchanged(model, s) && !site.span.contains_inclusive(model.scoping.symbol_span(s)))
        .then_some(s)
}

// Teardown consumes DATA receivers, whose bindings may legitimately be reset by
// setup. This does not resolve a receiver as a callable or relax capture().
fn data_capture(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<Symbol> {
    let s = symbol(model, e)?;
    if site.span.contains_inclusive(model.scoping.symbol_span(s))
        || !model.scoping.symbol_redeclarations(s).is_empty()
    {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return None;
    };
    (matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
        && matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(_)
        )
        && !matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
        && model.scoping.symbol_span(s).end < site.span.start)
        .then_some(s)
}

// The shared callable resolver establishes identity; this local gate additionally
// rejects initializer/assignment origins that occur after the use, at every hop.
fn callable_order(model: &Model<'_, '_>, e: &Expression<'_>, depth: usize) -> Option<()> {
    if depth > 16 {
        return None;
    }
    let Expression::Identifier(_) = e else {
        return Some(());
    };
    let s = symbol(model, e)?;
    if !model.scoping.symbol_redeclarations(s).is_empty() {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(s);
    if matches!(model.nodes.kind(declaration), AstKind::Function(_)) {
        return unchanged(model, s).then_some(());
    }
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(d) if matches!(d.kind, VariableDeclarationKind::Var | VariableDeclarationKind::Let | VariableDeclarationKind::Const)
        )
        || matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
        || !(model.binding_writes(s) == 0 && v.init.is_some()
            || model.binding_writes(s) == 1 && v.init.is_none())
    {
        return None;
    }
    let value = model.symbol_value(s)?;
    if value.span().end > e.span().start {
        return None;
    }
    let container = model.nodes.parent_id(model.nodes.parent_id(declaration));
    if !matches!(
        model.nodes.kind(container),
        AstKind::FunctionBody(_) | AstKind::Program(_)
    ) {
        return None;
    }
    // Textual order alone cannot establish a setup callback or conditional
    // assignment as an origin. A sole assignment must execute in the same
    // straight-line statement list as the declaration.
    if v.init.is_none()
        && !model.scoping.get_resolved_references(s).any(|r| {
            let parent = model.nodes.parent_id(r.node_id());
            matches!(model.nodes.kind(parent), AstKind::AssignmentExpression(a)
                if a.operator.as_str() == "=" && a.right.span() == value.span())
                && matches!(
                    model.nodes.parent_kind(parent),
                    AstKind::ExpressionStatement(_)
                )
                && model.nodes.parent_id(model.nodes.parent_id(parent)) == container
        })
    {
        return None;
    }
    callable_order(model, value, depth + 1)
}

fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = e else {
        return None;
    };
    (!m.optional && m.property.name == key).then_some(&m.object)
}

fn statement_call<'a>(s: &'a Statement<'a>) -> Option<&'a CallExpression<'a>> {
    let Statement::ExpressionStatement(e) = s else {
        return None;
    };
    let Expression::CallExpression(c) = &e.expression else {
        return None;
    };
    (!c.optional).then_some(c)
}

fn plain_params(model: &Model<'_, '_>, params: &FormalParameters<'_>) -> bool {
    params.rest.is_none()
        && params.items.iter().all(|p| {
            p.initializer.is_none()
                && matches!(&p.pattern, BindingPattern::BindingIdentifier(id)
            if id.symbol_id.get().is_some_and(|s| unchanged(model, s)))
        })
}

fn directives(body: &FunctionBody<'_>) -> bool {
    body.directives.is_empty()
        || (body.directives.len() == 1 && body.directives[0].directive == "use strict")
}

// A forEach callback must itself own exactly the operation on its one formal.
// Only inline callbacks participate; arbitrary callback values remain unsupported.
fn each_callback(model: &Model<'_, '_>, arg: &Argument<'_>, method: Option<&str>) -> Option<()> {
    let (params, body) = match arg {
        Argument::ArrowFunctionExpression(a) if !a.r#async => (&a.params, &a.body),
        Argument::FunctionExpression(f) if !f.r#async && !f.generator => {
            (&f.params, f.body.as_ref()?)
        }
        _ => return None,
    };
    if !plain_params(model, params) || params.items.len() != 1 || !directives(body) {
        return None;
    }
    let BindingPattern::BindingIdentifier(id) = &params.items[0].pattern else {
        return None;
    };
    let [s] = body.statements.as_slice() else {
        return None;
    };
    let c = statement_call(s)?;
    if !c.arguments.is_empty() {
        return None;
    }
    let receiver = if let Some(key) = method {
        member(&c.callee, key)?
    } else {
        &c.callee
    };
    (symbol(model, receiver) == id.symbol_id.get()).then_some(())
}

fn teardown_batch(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [callbacks, empty, clear, subscriptions] = body.statements.as_slice() else {
        return None;
    };
    let mut identities = BTreeSet::new();
    for (statement, method) in [(callbacks, None), (subscriptions, Some("teardown"))] {
        let c = statement_call(statement)?;
        let list = data_capture(model, site, member(&c.callee, "forEach")?)?;
        if !identities.insert(list) {
            return None;
        }
        let [callback] = c.arguments.as_slice() else {
            return None;
        };
        each_callback(model, callback, method)?;
    }
    for (statement, method) in [(empty, "empty"), (clear, "clear")] {
        let c = statement_call(statement)?;
        if !c.arguments.is_empty()
            || !identities.insert(data_capture(model, site, member(&c.callee, method)?)?)
        {
            return None;
        }
    }
    Some(())
}

fn closed_getter(body: &FunctionBody<'_>) -> Option<&'static str> {
    let [Statement::ReturnStatement(r)] = body.statements.as_slice() else {
        return None;
    };
    let Expression::BinaryExpression(b) = r.argument.as_ref()? else {
        return None;
    };
    if b.operator.as_str() != "===" {
        return None;
    }
    let Expression::UnaryExpression(u) = &b.right else {
        return None;
    };
    if u.operator.as_str() != "void"
        || !matches!(&u.argument, Expression::NumericLiteral(n) if n.value == 0.0)
    {
        return None;
    }
    let receiver = member(&b.left, "_observer")?;
    if matches!(receiver, Expression::ThisExpression(_)) {
        return Some("isSubscriptionClosed");
    }
    matches!(
        member(receiver, "_subscription")?,
        Expression::ThisExpression(_)
    )
    .then_some("isObserverSubscriptionClosed")
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return vec![];
    };
    let Some(body) = function.body.as_ref() else {
        return vec![];
    };
    if function.r#async
        || function.generator
        || !plain_params(model, &function.params)
        || !directives(body)
    {
        return vec![];
    }
    let mut claims = Vec::new();
    let mut add = |name, detail| claims.push(proposal(name, "shared_scoped_adapter", detail, f));
    if function.params.items.is_empty() {
        if teardown_batch(model, site, body).is_some() {
            add("teardownSubscriptions", "exact ordered cleanup: captured callbacks.forEach(item => item()), distinct buffer.empty(), registry.clear(), subscriptions.forEach(item => item.teardown()); owned callback bodies and distinct scoped data identities (receiver bindings may be reset)");
        }
        if let Some(name) = closed_getter(body) {
            add(name, "returns exact this._observer === void 0 or this._subscription._observer === void 0, with no other operations");
        }
    }
    if let [s] = body.statements.as_slice() {
        if let Some(c) = statement_call(s) {
            for (method, name, arity) in [
                ("complete", "forwardComplete", 0),
                ("teardown", "teardownSubscription", 0),
                ("next", "forwardNext", 1),
            ] {
                if function.params.items.len() == arity && c.arguments.len() == arity {
                    if member(&c.callee, method)
                        .and_then(|e| capture(model, site, e))
                        .is_some()
                        && (arity == 0
                            || call_arguments(c)
                                .first()
                                .is_some_and(|e| model.site_parameter_slot(e, site) == Some(0)))
                    {
                        add(name, "sole nonoptional method call forwards exact formal arguments in order to an unchanged scoped captured receiver; no return or extra work");
                    }
                }
            }
            // A small independent callee proof, rather than inherited role metadata.
            if function.params.items.is_empty()
                && c.arguments.is_empty()
                && callable_order(model, &c.callee, 0).is_some()
            {
                if let Some(target) = stable_function(model, &c.callee, 0) {
                    if let AstKind::Function(callee) = model.nodes.kind(target.node_id) {
                        if !callee.r#async
                            && !callee.generator
                            && callee.params.items.is_empty()
                            && callee.params.rest.is_none()
                            && callee.body.as_ref().is_some_and(|b| {
                                directives(b) && teardown_batch(model, target, b).is_some()
                            })
                        {
                            add("teardownSubscriptions", "sole zero-argument call through a stable scoped callable origin whose owned body independently proves the complete ordered subscription teardown");
                        }
                    }
                }
            }
        }
    }
    claims
}
