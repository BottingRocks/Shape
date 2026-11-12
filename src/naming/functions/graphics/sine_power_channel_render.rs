//! Closed channel preparation and render-result dispatch protocol. Captures
//! remain late-bound data; neither native context nor child payload semantics
//! are inferred. The local callback's declaration is consumed as an opaque
//! value, never used as donor evidence for the parent's role.
use crate::naming::support::interaction::*;
use super::*;
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;

type S = SymbolId;
enum E {
    S(S),
    N(f64),
    T(&'static str),
    G(&'static str),
    M(Box<E>, &'static str),
    I(Box<E>, Box<E>),
    B(Box<E>, &'static str, Box<E>),
    C(Box<E>, Vec<E>),
}
fn s(x: S) -> E {
    E::S(x)
}
fn n(x: f64) -> E {
    E::N(x)
}
fn t(x: &'static str) -> E {
    E::T(x)
}
fn g(x: &'static str) -> E {
    E::G(x)
}
fn m(x: E, k: &'static str) -> E {
    E::M(Box::new(x), k)
}
fn i(x: E, k: E) -> E {
    E::I(Box::new(x), Box::new(k))
}
fn b(x: E, op: &'static str, y: E) -> E {
    E::B(Box::new(x), op, Box::new(y))
}
fn c(x: E, a: Vec<E>) -> E {
    E::C(Box::new(x), a)
}
fn eq(model: &Model<'_, '_>, value: &Expression<'_>, p: &E) -> bool {
    let value = value.without_parentheses();
    match p {
        E::S(x) => symbol(model, value) == Some(*x),
        E::N(x) => number(value, *x),
        E::T(x) => matches!(value,Expression::StringLiteral(v) if v.value==*x),
        E::G(x) => expression_path(model, value) == [*x],
        E::M(o, k) => member(value, k).is_some_and(|v| eq(model, v, o)),
        E::I(o, k) => {
            matches!(value,Expression::ComputedMemberExpression(v) if !v.optional&&eq(model,&v.object,o)&&eq(model,&v.expression,k))
        }
        E::B(l, op, r) => {
            binary(value, op).is_some_and(|(a, z)| eq(model, a, l) && eq(model, z, r))
        }
        E::C(f, ps) => call(value)
            .and_then(|v| arguments(v).map(|a| (v, a)))
            .is_some_and(|(v, a)| {
                eq(model, &v.callee, f)
                    && a.len() == ps.len()
                    && a.iter().zip(ps).all(|(v, p)| eq(model, v, p))
            }),
    }
}
fn lhs(model: &Model<'_, '_>, value: &AssignmentTarget<'_>, p: &E) -> bool {
    match (value, p) {
        (AssignmentTarget::AssignmentTargetIdentifier(v), E::S(x)) => {
            crate::analysis::identifier_symbol(v, model.scoping) == Some(*x)
        }
        (AssignmentTarget::StaticMemberExpression(v), E::M(o, k)) => {
            !v.optional && v.property.name == *k && eq(model, &v.object, o)
        }
        (AssignmentTarget::ComputedMemberExpression(v), E::I(o, k)) => {
            !v.optional && eq(model, &v.object, o) && eq(model, &v.expression, k)
        }
        _ => false,
    }
}
fn assign(model: &Model<'_, '_>, stmt: &Statement<'_>, to: E, value: E) -> bool {
    expression_statement(stmt)
        .and_then(assignment)
        .is_some_and(|a| lhs(model, &a.left, &to) && eq(model, &a.right, &value))
}
fn expression(model: &Model<'_, '_>, stmt: &Statement<'_>, p: E) -> bool {
    expression_statement(stmt).is_some_and(|e| eq(model, e, &p))
}
fn local<'a>(model: &Model<'_, '_>, stmt: &'a Statement<'a>) -> Option<(S, &'a Expression<'a>)> {
    let v = variable(stmt)?;
    Some((plain_var(model, v)?, v.init.as_ref()?))
}
fn cap(model: &Model<'_, '_>, site: FunctionSite, value: &Expression<'_>) -> Option<S> {
    captured_symbol(model, site, symbol(model, value)?)
}
fn statements<'a>(stmt: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(v) = stmt else {
        return None;
    };
    Some(&v.body)
}
fn returns_void(stmt: &Statement<'_>) -> bool {
    matches!(stmt,Statement::ReturnStatement(r) if r.argument.as_ref().is_some_and(|e|matches!(e.without_parentheses(),Expression::UnaryExpression(v) if v.operator.as_str()=="void"&&number(&v.argument,0.0))))
}
fn arrow<'a>(
    model: &Model<'_, '_>,
    e: &'a Expression<'a>,
    expr: bool,
) -> Option<(S, &'a Statement<'a>)> {
    let Expression::ArrowFunctionExpression(a) = e.without_parentheses() else {
        return None;
    };
    let [param] = a.params.items.as_slice() else {
        return None;
    };
    let [body] = a.body.statements.as_slice() else {
        return None;
    };
    if a.r#async || a.expression != expr || a.params.rest.is_some() || !a.body.directives.is_empty()
    {
        return None;
    }
    let x = plain_param(model, param)?;
    (model.binding_references(x) == 1).then_some((x, body))
}
// A completed constructor store dominates the declaration's only exposure.
// Subsequent queued execution may observe other writes; no instance stability
// or constructor identity is inferred from this readiness frontier.
fn registration(model: &Model<'_, '_>, site: FunctionSite, context: S) -> Option<(S, S)> {
    let own = site.binding_symbol?;
    if model.binding_writes(own) != 0
        || model.binding_references(own) != 1
        || !model.scoping.symbol_redeclarations(own).is_empty()
    {
        return None;
    }
    let AstKind::BlockStatement(owner) = model.nodes.parent_kind(site.node_id) else {
        return None;
    };
    let pos = owner.body.iter().position(|x| x.span() == site.span)?;
    let enqueue = call(expression_statement(owner.body.get(pos + 1)?)?)?;
    let args = arguments(enqueue)?;
    let queue = cap(model, site, member(&enqueue.callee, "enqueue")?)?;
    if args.len() != 1 || symbol(model, args[0]) != Some(own) {
        return None;
    }
    let [Statement::TryStatement(setup), osc_stmt, frequency, connect, start] =
        owner.body.get(pos.checked_sub(5)?..pos)?
    else {
        return None;
    };
    if setup.finalizer.is_some() {
        return None;
    }
    let handler = setup.handler.as_ref()?;
    let [first] = setup.block.body.as_slice() else {
        return None;
    };
    let [second] = handler.body.body.as_slice() else {
        return None;
    };
    let new_assignment = |stmt: &Statement<'_>, rate: f64| -> Option<S> {
        let a = assignment(expression_statement(stmt)?)?;
        if target_symbol(model, &a.left) != Some(context) {
            return None;
        }
        let Expression::NewExpression(v) = a.right.without_parentheses() else {
            return None;
        };
        let ctor = cap(model, site, &v.callee)?;
        let [a, b, d] = v.arguments.as_slice() else {
            return None;
        };
        (number(a.as_expression()?, 1.0)
            && number(b.as_expression()?, 9.0)
            && number(d.as_expression()?, rate))
        .then_some(ctor)
    };
    let ctor = new_assignment(first, 8003.0)?;
    if new_assignment(second, 44100.0)? != ctor {
        return None;
    }
    let (osc, value) = local(model, osc_stmt)?;
    if !eq(model, value, &c(m(s(context), "createOscillator"), vec![]))
        || !assign(
            model,
            frequency,
            m(m(s(osc), "frequency"), "value"),
            n(3410.0),
        )
        || !expression(
            model,
            connect,
            c(m(s(osc), "connect"), vec![m(s(context), "destination")]),
        )
        || !expression(model, start, c(m(s(osc), "start"), vec![]))
    {
        return None;
    }
    Some((queue, ctor))
}
fn prove(model: &Model<'_, '_>, site: FunctionSite) -> Option<()> {
    let AstKind::Function(fun) = model.nodes.kind(site.node_id) else {
        return None;
    };
    let body = fun.body.as_ref()?;
    if fun.r#async
        || fun.generator
        || !fun.params.items.is_empty()
        || fun.params.rest.is_some()
        || !body.directives.is_empty()
    {
        return None;
    }
    let st = body.statements.as_slice();
    if st.len() != 16 {
        return None;
    }
    let (buffer, buffer_value) = local(model, &st[0])?;
    let context = cap(
        model,
        site,
        member(&call(buffer_value)?.callee, "createBuffer")?,
    )?;
    let (state, state_value) = local(model, &st[1])?;
    let state_source = cap(model, site, state_value)?;
    let (data, data_value) = local(model, &st[2])?;
    let (length, length_value) = local(model, &st[3])?;
    let (rate, rate_value) = local(model, &st[4])?;
    let (_, read) = binary(length_value, "*")?;
    let reader = cap(model, site, &call(read)?.callee)?;
    // Subtraction binds more tightly than >>>: preserve both self reads and
    // shifts. Neither fractional-part rewriting nor zero/NaN folding is used.
    let ratio = b(
        b(b(s(length), "-", s(length)), ">>>", n(0.0)),
        "/",
        b(b(s(rate), "-", s(rate)), ">>>", n(0.0)),
    );
    if !eq(
        model,
        buffer_value,
        &c(
            m(s(context), "createBuffer"),
            vec![n(1.0), n(9.0), m(s(context), "sampleRate")],
        ),
    ) || !eq(
        model,
        data_value,
        &c(m(s(buffer), "getChannelData"), vec![n(0.0)]),
    ) || !eq(
        model,
        length_value,
        &b(m(s(buffer), "length"), "*", c(s(reader), vec![s(state)])),
    ) || !eq(
        model,
        rate_value,
        &b(
            m(s(buffer), "sampleRate"),
            "*",
            c(s(reader), vec![s(state)]),
        ),
    ) || !assign(model, &st[5], i(s(data), n(0.0)), ratio)
        || !assign(
            model,
            &st[6],
            i(s(data), n(1.0)),
            c(m(g("Math"), "max"), vec![i(s(data), n(0.0)), n(0.5)]),
        )
    {
        return None;
    }
    let Statement::ForStatement(loop_) = &st[7] else {
        return None;
    };
    let Some(ForStatementInit::VariableDeclaration(decl)) = &loop_.init else {
        return None;
    };
    if decl.kind != VariableDeclarationKind::Let {
        return None;
    }
    let v = single_variable(decl)?;
    let index = plain_var(model, v)?;
    if !number(v.init.as_ref()?, 2.0)
        || !eq(
            model,
            loop_.test.as_ref()?,
            &b(s(index), "<", m(s(data), "length")),
        )
    {
        return None;
    }
    let update = assignment(loop_.update.as_ref()?)?;
    if target_symbol(model, &update.left) != Some(index)
        || !eq(model, &update.right, &b(s(index), "+", n(3.0)))
    {
        return None;
    }
    let [sin_stmt, write_sin, read_stmt, write_read, write_pow] = statements(&loop_.body)? else {
        return None;
    };
    let (sin, sin_value) = local(model, sin_stmt)?;
    let (sample, sample_value) = local(model, read_stmt)?;
    if !eq(model, sin_value, &c(m(g("Math"), "sin"), vec![s(index)]))
        || !eq(model, sample_value, &c(s(reader), vec![s(state)]))
        || !assign(model, write_sin, i(s(data), s(index)), s(sin))
        || !assign(
            model,
            write_read,
            i(s(data), b(s(index), "+", n(1.0))),
            s(sample),
        )
        || !assign(
            model,
            write_pow,
            i(s(data), b(s(index), "+", n(2.0))),
            c(m(g("Math"), "pow"), vec![s(sin), s(sample)]),
        )
    {
        return None;
    }
    let (source, source_value) = local(model, &st[8])?;
    if !eq(
        model,
        source_value,
        &c(m(s(context), "createBufferSource"), vec![]),
    ) || !assign(model, &st[9], m(s(source), "buffer"), s(buffer))
        || !expression(
            model,
            &st[10],
            c(m(s(source), "connect"), vec![m(s(context), "destination")]),
        )
        || !expression(model, &st[11], c(m(s(source), "start"), vec![]))
    {
        return None;
    }
    let Statement::FunctionDeclaration(child) = &st[12] else {
        return None;
    };
    let child_symbol = child.id.as_ref()?.symbol_id.get()?;
    if child.r#async
        || child.generator
        || child.params.rest.is_some()
        || child.params.items.len() != 1
        || model.binding_writes(child_symbol) != 0
        || model.binding_references(child_symbol) != 1
        || !model.scoping.symbol_redeclarations(child_symbol).is_empty()
    {
        return None;
    }
    plain_param(model, &child.params.items[0])?;
    let (callback, callback_value) = local(model, &st[13])?;
    let (pending, pending_value) = local(model, &st[14])?;
    if symbol(model, callback_value) != Some(child_symbol)
        || !eq(
            model,
            pending_value,
            &c(m(s(context), "startRendering"), vec![]),
        )
    {
        return None;
    }
    let Statement::IfStatement(suspended) = &st[15] else {
        return None;
    };
    let [invoke, ret] = statements(&suspended.consequent)? else {
        return None;
    };
    if !eq(
        model,
        &suspended.test,
        &b(m(s(context), "state"), "===", t("suspended")),
    ) || !expression(model, invoke, c(s(callback), vec![s(buffer)]))
        || !returns_void(ret)
    {
        return None;
    }
    let [Statement::IfStatement(delivery), ret] = statements(suspended.alternate.as_ref()?)? else {
        return None;
    };
    if !returns_void(ret)
        || !matches!(delivery.test.without_parentheses(),Expression::BinaryExpression(v) if v.operator.as_str()=="!="&&symbol(model,&v.left)==Some(pending)&&matches!(v.right.without_parentheses(),Expression::NullLiteral(_)))
    {
        return None;
    }
    let [promise_stmt] = statements(&delivery.consequent)? else {
        return None;
    };
    let caught_call = call(expression_statement(promise_stmt)?)?;
    let args = arguments(caught_call)?;
    if args.len() != 1
        || !eq(
            model,
            member(&caught_call.callee, "catch")?,
            &c(m(s(pending), "then"), vec![s(callback)]),
        )
    {
        return None;
    }
    let (error, forward) = arrow(model, args[0], false)?;
    let forward_call = call(expression_statement(forward)?)?;
    let observer = cap(model, site, member(&forward_call.callee, "error")?)?;
    if !expression(model, forward, c(m(s(observer), "error"), vec![s(error)])) {
        return None;
    }
    let [event_stmt] = statements(delivery.alternate.as_ref()?)? else {
        return None;
    };
    let event = call(expression_statement(event_stmt)?)?;
    let args = arguments(event)?;
    if args.len() != 2
        || !eq(model, &event.callee, &m(s(context), "addEventListener"))
        || !eq(model, args[0], &t("complete"))
    {
        return None;
    }
    let (event_value, forward) = arrow(model, args[1], true)?;
    if !expression(
        model,
        forward,
        c(s(callback), vec![m(s(event_value), "renderedBuffer")]),
    ) {
        return None;
    }
    let (queue, ctor) = registration(model, site, context)?;
    let locals = [
        buffer,
        state,
        data,
        length,
        rate,
        index,
        sin,
        sample,
        source,
        child_symbol,
        callback,
        pending,
    ];
    let captures = [context, state_source, reader, observer, queue, ctor];
    let set: BTreeSet<_> = locals.iter().copied().collect();
    if set.len() != locals.len()
        || captures.iter().collect::<BTreeSet<_>>().len() != captures.len()
        || captures.iter().any(|s| set.contains(s))
    {
        return None;
    }
    for local in locals {
        let writes = if local == index { 1 } else { 0 };
        if model.binding_writes(local) != writes
            || !model.scoping.symbol_redeclarations(local).is_empty()
        {
            return None;
        }
        if local != callback
            && model
                .scoping
                .get_resolved_references(local)
                .any(|r| model.owning_site(r.node_id()).map(|s| s.span) != Some(site.span))
        {
            return None;
        }
    }
    if model.binding_references(callback) != 3 || model.binding_references(pending) != 2 {
        return None;
    }
    // Opaque callback code may not bypass the lexical write census through
    // direct eval or dynamic scope. No unindexed local writes are admitted.
    if model.nodes.iter().any(|node| {
        matches!(node.kind(), AstKind::WithStatement(_))
            || matches!(node.kind(), AstKind::IdentifierReference(id) if id.name=="eval")
    }) {
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
    if prove(model, site).is_none() {
        return vec![];
    }
    vec![proposal("prepareSinePowerChannelAndDispatchRenderResult","graphics_sine_power_channel_render",
        "entire owned parent protocol: createBuffer(1,9,captured.sampleRate), channel zero, exact shifted self-difference ratio and max lane, step-three sin/opaque-reader/pow fill, source buffer/connect/start, one opaque local callback identity, startRendering then suspended direct buffer / promise then-catch / complete-event renderedBuffer dispatch, same observer error forwarding and exact void returns; sole adjacent enqueue follows the active try/fallback constructor stores and exact oscillator setup; distinct lexical bindings and local writes closed; captures remain late-bound opaque data, with no stable instance, native AudioContext, reader purpose, child payload, seed or wire-equivalence inference",f)]
}
