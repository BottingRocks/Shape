//! Ordered request metadata callback protocol. No native transport identity is inferred.
use super::super::browser::url_query_merge as q;
use crate::naming::support::interaction as i;
use super::*;
use oxc_span::GetSpan;

type S = SymbolId;
#[derive(Clone, Debug, PartialEq, Eq)]
enum E {
    S(S),
    G(String),
    T(String),
    N(u32),
    B(bool),
    Null,
    Field(Box<E>, String),
    Index(Box<E>, Box<E>),
    Op(String, Box<E>, Box<E>),
    Call(Box<E>, Vec<E>),
    Object(Vec<(String, E)>),
}
fn s(x: S) -> E {
    E::S(x)
}
fn f(x: E, k: &str) -> E {
    E::Field(Box::new(x), k.into())
}
fn ix(x: E, k: u32) -> E {
    E::Index(Box::new(x), Box::new(E::N(k)))
}
fn op(k: &str, a: E, b: E) -> E {
    E::Op(k.into(), Box::new(a), Box::new(b))
}
fn call(x: E, args: Vec<E>) -> E {
    E::Call(Box::new(x), args)
}
fn object(xs: Vec<(&str, E)>) -> E {
    E::Object(xs.into_iter().map(|(k, v)| (k.into(), v)).collect())
}
fn expr(m: &Model<'_, '_>, e: &Expression<'_>, depth: usize) -> Option<E> {
    if depth > 24 {
        return None;
    }
    let child = |e: &Expression<'_>| expr(m, e, depth + 1);
    Some(match e.without_parentheses() {
        Expression::Identifier(id) => match crate::analysis::identifier_symbol(id, m.scoping) {
            Some(id) => s(id),
            None => E::G(id.name.to_string()),
        },
        Expression::StringLiteral(v) => E::T(v.value.to_string()),
        Expression::NumericLiteral(v)
            if v.value >= 0.0 && v.value <= 8.0 && v.value.fract() == 0.0 =>
        {
            E::N(v.value as u32)
        }
        Expression::BooleanLiteral(v) => E::B(v.value),
        Expression::NullLiteral(_) => E::Null,
        Expression::StaticMemberExpression(v) if !v.optional => {
            f(child(&v.object)?, v.property.name.as_str())
        }
        Expression::ComputedMemberExpression(v) if !v.optional => {
            E::Index(Box::new(child(&v.object)?), Box::new(child(&v.expression)?))
        }
        Expression::BinaryExpression(v) => {
            op(v.operator.as_str(), child(&v.left)?, child(&v.right)?)
        }
        Expression::LogicalExpression(v) => {
            op(v.operator.as_str(), child(&v.left)?, child(&v.right)?)
        }
        Expression::CallExpression(v) if !v.optional => call(
            child(&v.callee)?,
            v.arguments
                .iter()
                .map(|a| child(a.as_expression()?))
                .collect::<Option<_>>()?,
        ),
        Expression::ObjectExpression(v) => E::Object(
            v.properties
                .iter()
                .map(|p| {
                    let ObjectPropertyKind::ObjectProperty(p) = p else {
                        return None;
                    };
                    if p.computed || p.method || p.kind != PropertyKind::Init {
                        return None;
                    }
                    Some((p.key.static_name()?.into_owned(), child(&p.value)?))
                })
                .collect::<Option<_>>()?,
        ),
        _ => return None,
    })
}
fn eq(m: &Model<'_, '_>, e: &Expression<'_>, want: E) -> Option<()> {
    (expr(m, e, 0) == Some(want)).then_some(())
}
fn var<'a>(m: &Model<'_, '_>, st: &'a Statement<'a>) -> Option<(S, &'a Expression<'a>)> {
    let v = q::one_variable(st)?;
    Some((q::plain_binding(m, v)?, v.init.as_ref()?))
}
fn store<'a>(m: &Model<'_, '_>, st: &'a Statement<'a>) -> Option<(S, &'a Expression<'a>)> {
    let a = q::assignment_statement(st)?;
    (a.operator.as_str() == "=").then_some(())?;
    Some((q::target_symbol(m, a)?, &a.right))
}
fn field_store(m: &Model<'_, '_>, st: &Statement<'_>, target: E, want: E) -> Option<()> {
    let a = q::assignment_statement(st)?;
    let AssignmentTarget::StaticMemberExpression(v) = &a.left else {
        return None;
    };
    if a.operator.as_str() != "=" || v.optional {
        return None;
    }
    (f(expr(m, &v.object, 0)?, v.property.name.as_str()) == target).then_some(())?;
    eq(m, &a.right, want)
}
fn ret(m: &Model<'_, '_>, st: &Statement<'_>, prior: S) -> Option<()> {
    let Statement::ReturnStatement(r) = st else {
        return None;
    };
    eq(m, r.argument.as_ref()?, s(prior))
}
fn branch<'a>(
    m: &Model<'_, '_>,
    st: &'a Statement<'a>,
    test: E,
) -> Option<(&'a [Statement<'a>], Option<&'a [Statement<'a>]>)> {
    let Statement::IfStatement(b) = st else {
        return None;
    };
    eq(m, &b.test, test)?;
    Some((
        q::block(&b.consequent)?,
        match b.alternate.as_ref() {
            Some(v) => Some(q::block(v)?),
            None => None,
        },
    ))
}

#[derive(Debug)]
struct Body {
    scratch: S,
    key: S,
    logger: S,
}
fn body(m: &Model<'_, '_>, site: FunctionSite, fun: &Function<'_>) -> Option<Body> {
    if fun.r#type != FunctionType::FunctionDeclaration
        || fun.r#async
        || fun.generator
        || fun.params.rest.is_some()
    {
        return None;
    }
    let [event, prior] = fun.params.items.as_slice() else {
        return None;
    };
    let event = i::plain_param(m, event)?;
    let prior = i::plain_param(m, prior)?;
    let b = fun.body.as_ref()?;
    if !b.directives.is_empty() {
        return None;
    }
    let [Statement::TryStatement(t)] = b.statements.as_slice() else {
        return None;
    };
    if t.finalizer.is_some() {
        return None;
    }
    let [bypass, args] = t.block.body.as_slice() else {
        return None;
    };
    let (yes, no) = branch(
        m,
        bypass,
        op(
            "&&",
            op("!=", s(prior), E::Null),
            op("!=", f(s(prior), "bypassResult"), E::Null),
        ),
    )?;
    let [r] = yes else {
        return None;
    };
    ret(m, r, prior)?;
    if no.is_some() {
        return None;
    }
    let args_e = f(s(event), "args");
    let (yes, Some(no)) = branch(
        m,
        args,
        op(
            "||",
            op("==", args_e.clone(), E::Null),
            op("<", f(args_e.clone(), "length"), E::N(2)),
        ),
    )?
    else {
        return None;
    };
    let [r] = yes else {
        return None;
    };
    ret(m, r, prior)?;
    let [nonnull, r] = no else {
        return None;
    };
    ret(m, r, prior)?;
    let (effects, no) = branch(m, nonnull, op("!=", ix(args_e.clone(), 0), E::Null))?;
    if no.is_some() {
        return None;
    }
    let [method_st, url_st, descriptor_st, record_st, value_st, sink_st] = effects else {
        return None;
    };
    let (method, init) = var(m, method_st)?;
    eq(
        m,
        init,
        op(
            "+",
            E::T("".into()),
            call(f(ix(args_e.clone(), 0), "toLowerCase"), vec![]),
        ),
    )?;
    let (scratch, url) = store(m, url_st)?;
    eq(m, url, ix(args_e, 1))?;
    let (descriptor, flags) = var(m, descriptor_st)?;
    eq(
        m,
        flags,
        object(vec![
            ("writable", E::B(true)),
            ("configurable", E::B(true)),
            ("enumerable", E::B(false)),
        ]),
    )?;
    let (record, value, overwritten) = if let Some((r, v)) = store(m, record_st) {
        (r, v, true)
    } else {
        let (r, v) = var(m, record_st)?;
        (r, v, false)
    };
    eq(
        m,
        value,
        object(vec![("method", s(method)), ("url", s(scratch))]),
    )?;
    if overwritten && record != method {
        return None;
    }
    field_store(m, value_st, f(s(descriptor), "value"), s(record))?;
    let sink = i::call(i::expression_statement(sink_st)?)?;
    let [receiver, key_arg, desc] = sink.arguments.as_slice() else {
        return None;
    };
    eq(m, &sink.callee, f(E::G("Object".into()), "defineProperty"))?;
    eq(m, receiver.as_expression()?, f(s(event), "thisObj"))?;
    eq(m, desc.as_expression()?, s(descriptor))?;
    let Expression::ComputedMemberExpression(key) = key_arg.as_expression()? else {
        return None;
    };
    if key.optional || !i::number(&key.expression, 0.0) {
        return None;
    }
    let key = i::symbol(m, &key.object)?;
    let catch = t.handler.as_ref()?;
    let caught = i::plain_binding(m, &catch.param.as_ref()?.pattern)?;
    let [error_st, message_st, log_st, r] = catch.body.body.as_slice() else {
        return None;
    };
    let (error, empty) = var(m, error_st)?;
    eq(m, empty, E::Object(vec![]))?;
    let a = q::assignment_statement(message_st)?;
    let Expression::BinaryExpression(plus) = &a.right else {
        return None;
    };
    let Expression::StringLiteral(prefix) = &plus.left else {
        return None;
    };
    field_store(
        m,
        message_st,
        f(s(error), "message"),
        op("+", E::T(prefix.value.to_string()), s(caught)),
    )?;
    let log = i::call(i::expression_statement(log_st)?)?;
    let logger = i::symbol(m, &log.callee)?;
    let [arg] = log.arguments.as_slice() else {
        return None;
    };
    eq(m, arg.as_expression()?, s(error))?;
    ret(m, r, prior)?;
    for (id, writes) in [
        (method, usize::from(overwritten)),
        (descriptor, 0),
        (error, 0),
        (caught, 0),
    ] {
        if m.binding_writes(id) != writes || (writes > 0 && !mutable(m, id)) {
            return None;
        }
    }
    if !overwritten && m.binding_writes(record) != 0 {
        return None;
    }
    let mut ids = vec![
        event, prior, method, scratch, descriptor, key, caught, error, logger,
    ];
    if !overwritten {
        ids.push(record);
    }
    if ids.iter().copied().collect::<BTreeSet<_>>().len() != ids.len() {
        return None;
    }
    for id in [scratch, key, logger] {
        i::captured_symbol(m, site, id)?;
    }
    Some(Body {
        scratch,
        key,
        logger,
    })
}

fn exposures_after(
    model: &Model<'_, '_>,
    container: NodeId,
    ready_end: u32,
    symbol: S,
    origin_write: Option<NodeId>,
    visiting: &mut BTreeSet<S>,
    budget: &mut usize,
) -> bool {
    if visiting.len() >= 16
        || !visiting.insert(symbol)
        || model.binding_writes(symbol) != usize::from(origin_write.is_some())
        || !model.scoping.symbol_redeclarations(symbol).is_empty()
    {
        return false;
    }
    let mut any = false;
    let result = model
        .scoping
        .get_resolved_references(symbol)
        .all(|reference| {
            if reference.is_write() {
                return origin_write == Some(reference.node_id());
            }
            any = true;
            if *budget == 0 {
                return false;
            }
            *budget -= 1;
            let node = reference.node_id();
            let parent = model.nodes.parent_id(node);
            // Taking a value for this alias cannot activate it. Follow all reads,
            // including early reads and aliases of the alias; never just one late
            // caller. A recovered assignment or conditional alias is not proved.
            if let AstKind::VariableDeclarator(alias) = model.nodes.kind(parent) {
                let statement = model.nodes.parent_id(parent);
                if matches!(model.nodes.kind(statement), AstKind::VariableDeclaration(_))
                    && model.nodes.parent_id(statement) == container
                    && alias.init.as_ref().is_some_and(|value| {
                        matches!(value, Expression::Identifier(_))
                            && value.span() == model.nodes.kind(node).span()
                    })
                {
                    return q::plain_binding(model, alias).is_some_and(|alias_symbol| {
                        exposures_after(
                            model,
                            container,
                            ready_end.max(alias.span.end),
                            alias_symbol,
                            None,
                            visiting,
                            budget,
                        )
                    });
                }
            }

            // Recovered `let alias; ...; alias = callable` is inert too, but
            // only with a sole direct store and a prior plain declaration in
            // this same activation. The current occurrence must be the exact
            // RHS value; an argument in `callable(callable)` is not an alias
            // store. All alias reads must follow both this store and setup.
            if let AstKind::AssignmentExpression(assignment) = model.nodes.kind(parent) {
                let statement = model.nodes.parent_id(parent);
                if assignment.operator.as_str() == "="
                    && matches!(&assignment.right, Expression::Identifier(_))
                    && assignment.right.span() == model.nodes.kind(node).span()
                    && matches!(model.nodes.kind(statement), AstKind::ExpressionStatement(_))
                    && model.nodes.parent_id(statement) == container
                {
                    let Some(alias) = q::target_symbol(model, assignment) else { return false; };
                    let declaration = model.scoping.symbol_declaration(alias);
                    let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else { return false; };
                    let declaration_statement = model.nodes.parent_id(declaration);
                    if variable.init.is_some()
                        || q::plain_binding(model, variable) != Some(alias)
                        || !matches!(model.nodes.kind(declaration_statement), AstKind::VariableDeclaration(_))
                        || model.nodes.parent_id(declaration_statement) != container
                        || model.nodes.kind(declaration_statement).span().end > model.nodes.kind(statement).span().start
                    { return false; }
                    let Some(write) = model.scoping.get_resolved_references(alias).find(|r| r.is_write()) else { return false; };
                    if !matches!(model.nodes.parent_kind(write.node_id()), AstKind::AssignmentExpression(store) if store.span == assignment.span) { return false; }
                    return exposures_after(model, container, ready_end.max(assignment.span.end), alias, Some(write.node_id()), visiting, budget);
                }
            }

            let mut child = node;
            for _ in 0..128 {
                let parent = model.nodes.parent_id(child);
                if matches!(
                    model.nodes.kind(child),
                    AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
                ) {
                    let declaration_binding = match model.nodes.kind(child) {
                        AstKind::Function(function) if function.r#type == FunctionType::FunctionDeclaration => function
                            .id
                            .as_ref()
                            .and_then(|id| id.symbol_id.get())
                            .map(|id| (id, ready_end)),
                        _ => None,
                    };
                    let expression_binding = if let AstKind::VariableDeclarator(variable) =
                        model.nodes.kind(parent)
                    {
                        let statement = model.nodes.parent_id(parent);
                        if matches!(model.nodes.kind(statement), AstKind::VariableDeclaration(_))
                            && model.nodes.parent_id(statement) == container
                            && variable.init.as_ref().is_some_and(|value| {
                                matches!(
                                    value,
                                    Expression::FunctionExpression(_)
                                        | Expression::ArrowFunctionExpression(_)
                                ) && value.span() == model.nodes.kind(child).span()
                            })
                        {
                            q::plain_binding(model, variable)
                                .map(|id| (id, ready_end.max(variable.span.end)))
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    if let Some((binding, end)) = declaration_binding.or(expression_binding) {
                        return exposures_after(model, container, end, binding, None, visiting, budget);
                    }
                    // Never order an unbound/IIFE/returned closure by its
                    // textual body location: discharge every activation edge.
                    return false;
                }
                if parent == container {
                    // This is a direct executed statement in the exact
                    // store container. All activation edges were discharged.
                    return model.nodes.kind(child).span().start >= ready_end;
                }
                if parent == child {
                    return false;
                }
                child = parent;
            }
            false
        });
    visiting.remove(&symbol);
    any && result
}

/// One scoped, direct executed store, never a selected textual last writer.
/// Reads of the key array cannot expose or mutate it. Logger reads are calls.
fn capture_ready(m: &Model<'_, '_>, site: FunctionSite, id: S, key: bool) -> Option<()> {
    let declaration = m.scoping.symbol_declaration(id);
    let AstKind::VariableDeclarator(v) = m.nodes.kind(declaration) else {
        return None;
    };
    let (value, statement) = if let Some(init) = v.init.as_ref() {
        if m.binding_writes(id) != 0 {
            return None;
        }
        (init, m.nodes.parent_id(declaration))
    } else {
        if m.binding_writes(id) != 1 {
            return None;
        }
        let r = m
            .scoping
            .get_resolved_references(id)
            .find(|r| r.is_write())?;
        let a_id = m.nodes.parent_id(r.node_id());
        let AstKind::AssignmentExpression(a) = m.nodes.kind(a_id) else {
            return None;
        };
        if a.operator.as_str() != "=" || q::target_symbol(m, a) != Some(id) {
            return None;
        }
        let st = m.nodes.parent_id(a_id);
        if !matches!(m.nodes.kind(st), AstKind::ExpressionStatement(_)) {
            return None;
        }
        (&a.right, st)
    };
    if key {
        let Expression::ArrayExpression(array) = value else {
            return None;
        };
        if array.elements.is_empty() || array.elements.len()>8 || !array.elements.iter().all(|e|matches!(e.as_expression(),Some(Expression::StringLiteral(v)) if !v.value.is_empty())){return None;}
    } else {
        let target = i::symbol(m, value)?;
        let AstKind::Function(_) = m.nodes.kind(m.scoping.symbol_declaration(target)) else {
            return None;
        };
        let target = m.function_from_symbol(target)?;
        if m.binding_writes(target.binding_symbol?) != 0
            || !m
                .scoping
                .symbol_redeclarations(target.binding_symbol?)
                .is_empty()
        {
            return None;
        }
    }
    for r in m
        .scoping
        .get_resolved_references(id)
        .filter(|r| !r.is_write())
    {
        if key && !read_only(m, m.nodes.parent_id(r.node_id())) {
            return None;
        }

        match m.nodes.parent_kind(r.node_id()) {
            AstKind::ComputedMemberExpression(v)
                if key && i::symbol(m, &v.object) == Some(id) && !v.optional =>
            {
                let member_id = m.nodes.parent_id(r.node_id());
                if matches!(
                    m.nodes.parent_kind(member_id),
                    AstKind::AssignmentExpression(_)
                        | AstKind::UpdateExpression(_)
                        | AstKind::UnaryExpression(_)
                ) {
                    return None;
                }
            }
            AstKind::StaticMemberExpression(v)
                if key && v.property.name == "length" && !v.optional =>
            {
                let member = m.nodes.parent_id(r.node_id());
                if matches!(
                    m.nodes.parent_kind(member),
                    AstKind::AssignmentExpression(_)
                        | AstKind::UpdateExpression(_)
                        | AstKind::UnaryExpression(_)
                ) {
                    return None;
                }
            }
            AstKind::CallExpression(v)
                if !key && i::symbol(m, &v.callee) == Some(id) && !v.optional => {}
            _ => return None,
        }
    }
    let container = m.nodes.parent_id(statement);
    if !matches!(
        m.nodes.kind(container),
        AstKind::FunctionBody(_) | AstKind::BlockStatement(_)
    ) {
        return None;
    }
    exposures_after(
        m,
        container,
        m.nodes.kind(statement).span().end,
        site.binding_symbol?,
        None,
        &mut BTreeSet::new(),
        &mut 256,
    )
    .then_some(())
}
fn protocol(m: &Model<'_, '_>, site: FunctionSite, fun: &Function<'_>) -> Result<(), &'static str> {
    let b = body(m, site, fun).ok_or("owned ordered body")?;
    if !mutable(m, b.scratch)
        || m.binding_writes(b.scratch) != 1
        || !matches!(m.initializer(b.scratch),Some(Expression::StringLiteral(v)) if v.value.is_empty())
    {
        return Err("scratch writers");
    }
    // Scratch is overwritten before its sole read, with no intervening call.
    if m.scoping.get_resolved_references(b.scratch).any(|r| {
        !site
            .span
            .contains_inclusive(m.nodes.kind(r.node_id()).span())
    }) {
        return Err("scratch escape");
    }
    let declaration = m.scoping.symbol_declaration(b.scratch);
    let statement = m.nodes.parent_id(declaration);
    if !exposures_after(
        m,
        m.nodes.parent_id(statement),
        m.nodes.kind(statement).span().end,
        site.binding_symbol.ok_or("callback binding")?,
        None,
        &mut BTreeSet::new(),
        &mut 256,
    ) {
        return Err("scratch initialization/exposure");
    }
    capture_ready(m, site, b.key, true).ok_or("key origin/readiness/exposure")?;
    capture_ready(m, site, b.logger, false)
        .or_else(|| logger_with_boolean_dead_stores(m, site, b.logger))
        .ok_or("logger origin/readiness/exposure")?;
    Ok(())
}
pub(in crate::naming) fn evaluate(
    m: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
    _: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(fun) = m.nodes.kind(site.node_id) else {
        return vec![];
    };
    if protocol(m, site, fun).is_err() {
        return vec![];
    }
    vec![proposal("captureRequestOpenMetadata","network_request_open_metadata","complete ordered request-open metadata callback: bypass and argument guards return the same prior; lowercase then string coercion; captured URL store before materialized method/url record and nonenumerable writable/configurable descriptor; exact receiver and captured key[0] defineProperty sink; empty error record, diagnostic plus the same catch binding, captured logger and prior return; every capture writer and callable exposure follows source initialization; opaque runtime ports, no native transport or key identity inferred",facts)]
}
#[cfg(test)]
#[path = "request_open_metadata_tests.rs"]
mod tests;

fn mutable(m: &Model<'_, '_>, id: S) -> bool {
    matches!(m.nodes.parent_kind(m.scoping.symbol_declaration(id)), AstKind::VariableDeclaration(v)
        if matches!(v.kind, VariableDeclarationKind::Let | VariableDeclarationKind::Var))
}
fn read_only(m: &Model<'_, '_>, member: NodeId) -> bool {
    let span = m.nodes.kind(member).span();
    let mut node = member;
    for _ in 0..128 {
        match m.nodes.kind(node) {
            AstKind::AssignmentExpression(a) if a.left.span().contains_inclusive(span) => {
                return false
            }
            AstKind::UpdateExpression(_) => return false,
            AstKind::UnaryExpression(a) if a.operator.as_str() == "delete" => return false,
            AstKind::ForInStatement(a) if a.left.span().contains_inclusive(span) => return false,
            AstKind::ForOfStatement(a) if a.left.span().contains_inclusive(span) => return false,
            _ => {}
        }
        let parent = m.nodes.parent_id(node);
        if parent == node {
            return true;
        }
        node = parent;
    }
    false
}

/// A direct scalar copy preserves a locally proved boolean domain even when
/// an opaque runtime consumer invokes or retains the flag-writing callback.
/// This proof does not assume anything about that consumer's native identity.
fn boolean_null_arm(m: &Model<'_, '_>, mut node: NodeId) -> Option<(S, S, Span)> {
    for _ in 0..128 {
        let parent = m.nodes.parent_id(node);
        if parent == node
            || matches!(
                m.nodes.kind(parent),
                AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
            )
        {
            return None;
        }
        if let AstKind::IfStatement(branch) = m.nodes.kind(parent) {
            if branch
                .consequent
                .span()
                .contains_inclusive(m.nodes.kind(node).span())
            {
                if let Some((left, right)) = i::binary(&branch.test, "===") {
                    if matches!(right, Expression::NullLiteral(_)) {
                        let snapshot = i::symbol(m, left)?;
                        let declaration = m.scoping.symbol_declaration(snapshot);
                        let AstKind::VariableDeclarator(copy) = m.nodes.kind(declaration) else {
                            return None;
                        };
                        let flag = i::symbol(m, copy.init.as_ref()?)?;
                        let origin = m.scoping.symbol_declaration(flag);
                        let AstKind::VariableDeclarator(value) = m.nodes.kind(origin) else {
                            return None;
                        };
                        if i::plain_var(m, copy) != Some(snapshot)
                            || i::plain_var(m, value) != Some(flag)
                            || m.binding_writes(snapshot) != 0
                            || !matches!(value.init, Some(Expression::BooleanLiteral(_)))
                            || snapshot == flag
                        {
                            return None;
                        }
                        let copy_statement = m.nodes.parent_id(declaration);
                        let flag_statement = m.nodes.parent_id(origin);
                        let container = m.nodes.parent_id(parent);
                        if !matches!(
                            m.nodes.kind(container),
                            AstKind::BlockStatement(_) | AstKind::FunctionBody(_)
                        ) || !matches!(
                            m.nodes.kind(copy_statement),
                            AstKind::VariableDeclaration(_)
                        ) || !matches!(
                            m.nodes.kind(flag_statement),
                            AstKind::VariableDeclaration(_)
                        ) || m.nodes.parent_id(copy_statement) != container
                            || m.nodes.parent_id(flag_statement) != container
                            || m.nodes.kind(flag_statement).span().end
                                > m.nodes.kind(copy_statement).span().start
                            || m.nodes.kind(copy_statement).span().end > branch.span.start
                        {
                            return None;
                        }
                        for r in m
                            .scoping
                            .get_resolved_references(flag)
                            .filter(|r| r.is_write())
                        {
                            let AstKind::AssignmentExpression(store) =
                                m.nodes.parent_kind(r.node_id())
                            else {
                                return None;
                            };
                            if store.operator.as_str() != "="
                                || q::target_symbol(m, store) != Some(flag)
                                || !matches!(store.right, Expression::BooleanLiteral(_))
                            {
                                return None;
                            }
                        }
                        // Dynamic lexical writes cannot be recovered from a
                        // SymbolId reference census, so they cannot qualify.
                        let owner = m.enclosing_function(origin)?;
                        if m.nodes.iter_enumerated().any(|(_,n)| owner.span.contains_inclusive(n.kind().span()) &&
                            (matches!(n.kind(),AstKind::WithStatement(_)) || matches!(n.kind(),AstKind::CallExpression(c)
                                if matches!(&c.callee,Expression::Identifier(id) if id.name=="eval")))) {return None;}
                        return Some((flag, snapshot, branch.span));
                    }
                }
            }
        }
        node = parent;
    }
    None
}
/// Enumerate every logger store, rejecting any second feasible writer. Stores
/// may be ignored only when their own enclosing `booleanSnapshot === null`
/// consequent is impossible by a complete scoped producer/copy/writer proof.
/// Catch resumption does not establish exclusion; the boolean domain does.
fn logger_with_boolean_dead_stores(m: &Model<'_, '_>, site: FunctionSite, id: S) -> Option<()> {
    i::captured_symbol(m, site, id)?;
    let declaration = m.scoping.symbol_declaration(id);
    let mut live = match m.initializer(id) {
        Some(value) => Some(logger_origin(m, value, m.nodes.parent_id(declaration))?),
        None => None,
    };
    let mut dead = 0;
    for r in m.scoping.get_resolved_references(id) {
        if !r.is_write() {
            if !matches!(m.nodes.parent_kind(r.node_id()),AstKind::CallExpression(c)
                if !c.optional && i::symbol(m,&c.callee)==Some(id))
            {
                return None;
            }
            continue;
        }
        let assignment = m.nodes.parent_id(r.node_id());
        let AstKind::AssignmentExpression(store) = m.nodes.kind(assignment) else {
            return None;
        };
        if store.operator.as_str() != "=" || q::target_symbol(m, store) != Some(id) {
            return None;
        }
        if boolean_null_arm(m, assignment).is_some() {
            dead += 1;
            continue;
        }
        if live.is_some() {
            return None;
        }
        let statement = m.nodes.parent_id(assignment);
        if !matches!(m.nodes.kind(statement), AstKind::ExpressionStatement(_)) {
            return None;
        }
        live = Some(logger_origin(m, &store.right, statement)?);
    }
    if dead == 0 || dead > 8 {
        return None;
    }
    let (container, end) = live?;
    exposures_after(
        m,
        container,
        end,
        site.binding_symbol?,
        None,
        &mut BTreeSet::new(),
        &mut 256,
    )
    .then_some(())
}
fn logger_origin(
    m: &Model<'_, '_>,
    value: &Expression<'_>,
    statement: NodeId,
) -> Option<(NodeId, u32)> {
    let target = i::symbol(m, value)?;
    if !matches!(m.nodes.kind(m.scoping.symbol_declaration(target)),AstKind::Function(f)
        if f.r#type==FunctionType::FunctionDeclaration)
        || m.binding_writes(target) != 0
        || !m.scoping.symbol_redeclarations(target).is_empty()
    {
        return None;
    }
    let container = m.nodes.parent_id(statement);
    if !matches!(
        m.nodes.kind(container),
        AstKind::BlockStatement(_) | AstKind::FunctionBody(_)
    ) {
        return None;
    }
    Some((container, m.nodes.kind(statement).span().end))
}
