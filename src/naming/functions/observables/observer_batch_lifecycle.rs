//! Whole observer batch lifecycle, including all owned callbacks and writers.
//! Constructor, sentinel and scheduler implementations remain opaque inputs.
//! Declaration grouping/order is neutral only for uninitialized locals; every
//! executed allocation, call, assignment, branch and return is consumed below.
use crate::naming::support::interaction::*;
use super::*;
use oxc_semantic::SymbolId;
use oxc_span::{GetSpan, Span};
type Sym = SymbolId;

fn fixed(m: &Model<'_, '_>, s: Sym) -> bool {
    m.binding_writes(s) == 0 && m.scoping.symbol_redeclarations(s).is_empty()
}
// Opaque captured value with one executed origin. This proves availability at
// every actual owner/alias exposure, without classifying the value or borrowing
// any helper implementation. Conditional/deferred stores and future aliases fail.
fn capture_ready(m: &Model<'_, '_>, e: &Expression<'_>, owner: Span) -> Option<Sym> {
    let s = symbol(m, e)?;
    if owner.contains_inclusive(m.scoping.symbol_span(s))
        || !m.scoping.symbol_redeclarations(s).is_empty()
    {
        return None;
    }
    let declaration = m.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = m.nodes.kind(declaration) else {
        return None;
    };
    plain_var(m, v)?;
    let statement = m.nodes.parent_id(declaration);
    let body = m.nodes.parent_id(statement);
    if !matches!(m.nodes.kind(statement),AstKind::VariableDeclaration(d) if matches!(d.kind,VariableDeclarationKind::Var|VariableDeclarationKind::Let|VariableDeclarationKind::Const))
        || !matches!(
            m.nodes.kind(body),
            AstKind::FunctionBody(_) | AstKind::Program(_)
        )
    {
        return None;
    }
    let end = if let Some(init) = &v.init {
        if m.binding_writes(s) != 0 {
            return None;
        }
        init.span().end
    } else {
        if m.binding_writes(s) != 1 {
            return None;
        }
        let r = m
            .scoping
            .get_resolved_references(s)
            .find(|r| r.is_write())?;
        let node = m.nodes.parent_id(r.node_id());
        let AstKind::AssignmentExpression(a) = m.nodes.kind(node) else {
            return None;
        };
        let st = m.nodes.parent_id(node);
        if a.operator.as_str() != "="
            || target_symbol(m, &a.left) != Some(s)
            || !matches!(m.nodes.kind(st), AstKind::ExpressionStatement(_))
            || m.nodes.parent_id(st) != body
            || a.span.start < v.span.end
        {
            return None;
        }
        a.span.end
    };
    let Expression::Identifier(id) = e else {
        return None;
    };
    let use_node = m.scoping.get_reference(id.reference_id.get()?).node_id();
    callable_origin_precedes(m, body, end, use_node, None, 0, true).then_some(s)
}

fn params(m: &Model<'_, '_>, f: &Function<'_>, n: usize) -> Option<Vec<Sym>> {
    if f.r#async
        || f.generator
        || f.params.rest.is_some()
        || f.params.items.len() != n
        || !f.body.as_ref()?.directives.is_empty()
    {
        return None;
    }
    f.params.items.iter().map(|p| plain_param(m, p)).collect()
}
fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    if let Statement::BlockStatement(b) = s {
        Some(&b.body)
    } else {
        None
    }
}
fn fun<'a>(s: &'a Statement<'a>) -> Option<&'a Function<'a>> {
    if let Statement::FunctionDeclaration(f) = s {
        Some(f)
    } else {
        None
    }
}
fn fb<'a>(f: &'a Function<'a>) -> Option<&'a [Statement<'a>]> {
    Some(&f.body.as_ref()?.statements)
}
fn fs(f: &Function<'_>) -> Option<Sym> {
    f.id.as_ref()?.symbol_id.get()
}
fn ret<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    if let Statement::ReturnStatement(r) = s {
        r.argument.as_ref()
    } else {
        None
    }
}
fn set<'a>(m: &Model<'_, '_>, s: &'a Statement<'a>) -> Option<(Sym, &'a Expression<'a>)> {
    let a = assignment(expression_statement(s)?)?;
    Some((target_symbol(m, &a.left)?, &a.right))
}
fn invoke(m: &Model<'_, '_>, s: &Statement<'_>, f: Sym, a: &[Sym]) -> Option<()> {
    let c = call(expression_statement(s)?)?;
    let args = arguments(c)?;
    (symbol(m, &c.callee) == Some(f)
        && args.len() == a.len()
        && args.iter().zip(a).all(|(x, y)| symbol(m, x) == Some(*y)))
    .then_some(())
}
fn null_guard<'a>(m: &Model<'_, '_>, s: &'a Statement<'a>, x: Sym) -> Option<&'a [Statement<'a>]> {
    let Statement::IfStatement(i) = s else {
        return None;
    };
    let (a, b) = binary(&i.test, "!=")?;
    if i.alternate.is_some() || symbol(m, a) != Some(x) || !matches!(b, Expression::NullLiteral(_))
    {
        return None;
    }
    block(&i.consequent)
}
fn branch<'a>(
    m: &Model<'_, '_>,
    s: &'a Statement<'a>,
    x: Sym,
) -> Option<(&'a [Statement<'a>], &'a [Statement<'a>])> {
    let Statement::IfStatement(i) = s else {
        return None;
    };
    if symbol(m, &i.test) != Some(x) {
        return None;
    }
    Some((block(&i.consequent)?, block(i.alternate.as_ref()?)?))
}
// Error broadcasts own exactly one arrow; the catch alias is local and exact.
fn broadcast(m: &Model<'_, '_>, s: &Statement<'_>, list: Sym, error: Sym) -> Option<()> {
    let c = call(expression_statement(s)?)?;
    if method_receiver(m, c, "forEach") != Some(list) {
        return None;
    }
    let [Argument::ArrowFunctionExpression(a)] = c.arguments.as_slice() else {
        return None;
    };
    if !a.expression
        || a.r#async
        || a.params.rest.is_some()
        || a.params.items.len() != 1
        || !a.body.directives.is_empty()
    {
        return None;
    }
    let p = plain_param(m, &a.params.items[0])?;
    let [st] = a.body.statements.as_slice() else {
        return None;
    };
    let c = call(expression_statement(st)?)?;
    (method_receiver(m, c, "error") == Some(p)
        && arguments(c)?
            .as_slice()
            .iter()
            .map(|e| symbol(m, e))
            .collect::<Vec<_>>()
            == vec![Some(error)])
    .then_some(())
}
fn caught(m: &Model<'_, '_>, t: &TryStatement<'_>, list: Sym) -> Option<()> {
    if t.finalizer.is_some() {
        return None;
    }
    let h = t.handler.as_ref()?;
    let e = plain_binding(m, &h.param.as_ref()?.pattern)?;
    let [alias, send] = h.body.body.as_slice() else {
        return None;
    };
    let v = variable(alias)?;
    if symbol(m, v.init.as_ref()?) != Some(e) {
        return None;
    }
    let error = plain_var(m, v)?;
    if !fixed(m, error) {
        return None;
    }
    broadcast(m, send, list, error)
}
fn guarded(m: &Model<'_, '_>, f: &Function<'_>) -> Option<Sym> {
    let p = params(m, f, 1)?[0];
    let [Statement::TryStatement(t)] = fb(f)? else {
        return None;
    };
    let [st] = t.block.body.as_slice() else {
        return None;
    };
    invoke(m, st, p, &[])?;
    let h = t.handler.as_ref()?;
    let [_, send] = h.body.body.as_slice() else {
        return None;
    };
    let c = call(expression_statement(send)?)?;
    let list = method_receiver(m, c, "forEach")?;
    caught(m, t, list)?;
    Some(list)
}
fn setter(m: &Model<'_, '_>, f: &Function<'_>, n: usize) -> Option<Vec<Sym>> {
    let ps = params(m, f, n)?;
    let ss = fb(f)?;
    if ss.len() != n {
        return None;
    }
    ss.iter()
        .zip(ps)
        .map(|(s, p)| {
            let (x, v) = set(m, s)?;
            (symbol(m, v) == Some(p)).then_some(x)
        })
        .collect()
}
fn scheduled(
    m: &Model<'_, '_>,
    s: &Statement<'_>,
    scheduler: Sym,
    guard: Sym,
    value: Sym,
) -> Option<()> {
    let c = call(expression_statement(s)?)?;
    if symbol(m, &c.callee) != Some(scheduler) {
        return None;
    }
    let [Argument::ArrowFunctionExpression(a)] = c.arguments.as_slice() else {
        return None;
    };
    if a.expression
        || a.r#async
        || !a.params.items.is_empty()
        || a.params.rest.is_some()
        || !a.body.directives.is_empty()
    {
        return None;
    }
    let [st] = a.body.statements.as_slice() else {
        return None;
    };
    invoke(m, st, guard, &[value])
}
#[derive(Clone, Copy)]
struct State {
    list: Sym,
    guard: Sym,
    first: Sym,
    middle: Sym,
    flag: Sym,
    last: Sym,
    cleanup: Sym,
}
fn cleanup(m: &Model<'_, '_>, f: &Function<'_>, x: State) -> Option<Sym> {
    params(m, f, 0)?;
    let [a, b, c] = fb(f)? else {
        return None;
    };
    let (ta, va) = set(m, a)?;
    let (tb, vb) = set(m, b)?;
    let sent = capture_ready(m, va, f.span)?;
    if ta != x.first || tb != x.last || symbol(m, vb) != Some(sent) {
        return None;
    }
    let [st] = null_guard(m, c, x.cleanup)? else {
        return None;
    };
    invoke(m, st, x.cleanup, &[])?;
    Some(sent)
}
fn arm(
    m: &Model<'_, '_>,
    ss: &[Statement<'_>],
    x: State,
    ps: &[Sym],
    setters: &[Sym],
    temp: Sym,
    input: Sym,
    fallback: bool,
) -> Option<(&'static str, Span, Sym)> {
    let [start, copy, first, middle, last, end, returned] = ss else {
        return None;
    };
    let c = call(expression_statement(start)?)?;
    let args = arguments(c)?;
    if args.len() != 6
        || symbol(m, &c.callee) != Some(if fallback { input } else { ps[1] })
        || symbol(m, args[1]) != Some(ps[2])
        || !args[2..]
            .iter()
            .zip(setters)
            .all(|(a, b)| symbol(m, a) == Some(*b))
    {
        return None;
    }
    if fallback {
        let Expression::ArrowFunctionExpression(a) = args[0] else {
            return None;
        };
        if a.r#async
            || a.params.rest.is_some()
            || !a.params.items.is_empty()
            || !a.body.directives.is_empty()
        {
            return None;
        }
        let [Statement::ThrowStatement(t)] = a.body.statements.as_slice() else {
            return None;
        };
        let Expression::NewExpression(n) = &t.argument else {
            return None;
        };
        let Expression::Identifier(id) = &n.callee else {
            return None;
        };
        if id.name != "TypeError" || symbol(m, &n.callee).is_some() {
            return None;
        }
        let [Argument::StringLiteral(s)] = n.arguments.as_slice() else {
            return None;
        };
        if s.value != "Cannot read properties of undefined (reading 'value')" {
            return None;
        }
    } else if symbol(m, args[0]) != Some(input) {
        return None;
    }
    let (target, value) = set(m, copy)?;
    if target != temp || symbol(m, value) != Some(x.middle) {
        return None;
    }
    let [st] = null_guard(m, first, x.first)? else {
        return None;
    };
    invoke(m, st, x.guard, &[x.first])?;
    let [st] = null_guard(m, middle, temp)? else {
        return None;
    };
    let (yes, no) = branch(m, st, ps[5])?;
    let [select] = yes else {
        return None;
    };
    let (defer, now) = branch(m, select, x.flag)?;
    let [st] = defer else {
        return None;
    };
    scheduled(m, st, ps[4], x.guard, temp)?;
    let [st] = now else {
        return None;
    };
    invoke(m, st, x.guard, &[temp])?;
    let [st] = no else {
        return None;
    };
    scheduled(m, st, ps[3], x.guard, temp)?;
    let [st] = null_guard(m, last, x.last)? else {
        return None;
    };
    scheduled(m, st, ps[6], x.guard, x.last)?;
    let f = fun(end)?;
    let sentinel = cleanup(m, f, x)?;
    if symbol(m, ret(returned)?) != fs(f) {
        return None;
    }
    Some(("resetBatchCallbacksAndRunCleanup", f.span, sentinel))
}
// A neutral declaration cursor permits split/grouped plain locals, preserving
// each initializer and store position. Nested callables are never flattened.
enum Item<'a> {
    Decl(Sym, Option<&'a Expression<'a>>),
    St(&'a Statement<'a>),
}
fn items<'a>(m: &Model<'_, '_>, ss: &'a [Statement<'a>]) -> Option<Vec<Item<'a>>> {
    let mut out = vec![];
    for s in ss {
        if let Statement::VariableDeclaration(d) = s {
            if !matches!(
                d.kind,
                VariableDeclarationKind::Var
                    | VariableDeclarationKind::Let
                    | VariableDeclarationKind::Const
            ) {
                return None;
            }
            for v in &d.declarations {
                out.push(Item::Decl(plain_var(m, v)?, v.init.as_ref()));
            }
        } else {
            out.push(Item::St(s));
        }
    }
    Some(out)
}
fn prove(m: &Model<'_, '_>, f: &Function<'_>) -> Option<Vec<(Span, &'static str)>> {
    let ps = params(m, f, 7)?;
    let entries = items(m, fb(f)?)?;
    let mut declared = BTreeSet::new();
    let mut initialized = BTreeSet::new();
    let mut effective = vec![];
    // Retain initialized declarations in place. Uninitialized declarations have
    // no effects; their exact writers are checked against the protocol below.
    for entry in &entries {
        match entry {
            Item::Decl(s, v) => {
                if !declared.insert(*s) {
                    return None;
                }
                if let Some(v) = v {
                    initialized.insert(*s);
                    effective.push((*s, Some(*v), None));
                }
            }
            Item::St(st) => effective.push((ps[0], None, Some(*st))),
        }
    }
    let mut pos = 0;
    // A catch-error local may precede the guard, but has no initialized value.
    let (_, _, Some(guard_st)) = *effective.get(pos)? else {
        return None;
    };
    pos += 1;
    let guard_fn = fun(guard_st)?;
    let list = guarded(m, guard_fn)?;
    let guard = fs(guard_fn)?;
    let (alias, Some(v), None) = *effective.get(pos)? else {
        return None;
    };
    pos += 1;
    if symbol(m, v) != Some(guard) || !fixed(m, guard) || !fixed(m, alias) {
        return None;
    }
    let (_, v, st) = *effective.get(pos)?;
    pos += 1;
    let (list_target, list_value) = if let Some(v) = v {
        (effective[pos - 1].0, v)
    } else {
        set(m, st?)?
    };
    if list_target != list
        || !matches!(list_value,Expression::ArrayExpression(a) if a.elements.is_empty())
    {
        return None;
    }
    let (index, Some(v), None) = *effective.get(pos)? else {
        return None;
    };
    pos += 1;
    if !number(v, 0.0) {
        return None;
    }
    let (_, _, Some(Statement::ForStatement(loop_))) = *effective.get(pos)? else {
        return None;
    };
    pos += 1;
    if loop_.init.is_some() {
        return None;
    }
    let (a, b) = binary(loop_.test.as_ref()?, "<")?;
    if symbol(m, a) != Some(index) || symbol(m, member(b, "length")?) != Some(ps[0]) {
        return None;
    }
    let Expression::UpdateExpression(u) = loop_.update.as_ref()? else {
        return None;
    };
    if u.operator.as_str() != "++"
        || !matches!(&u.argument,SimpleAssignmentTarget::AssignmentTargetIdentifier(id) if crate::analysis::identifier_symbol(id,m.scoping)==Some(index))
    {
        return None;
    }
    let [register, subscribe] = block(&loop_.body)? else {
        return None;
    };
    let register = fun(register)?;
    let rp = params(m, register, 1)?[0];
    let [push] = fb(register)? else {
        return None;
    };
    let c = call(expression_statement(push)?)?;
    if method_receiver(m, c, "push") != Some(list)
        || arguments(c)?.len() != 1
        || symbol(m, arguments(c)?[0]) != Some(rp)
    {
        return None;
    }
    let c = call(expression_statement(subscribe)?)?;
    if arguments(c)?.len() != 1 {
        return None;
    }
    let Expression::NewExpression(n) = member(&c.callee, "subscribe")? else {
        return None;
    };
    let ctor = capture_ready(m, &n.callee, f.span)?;
    if arguments(c)?.len() != 1
        || n.arguments.len() != 1
        || symbol(m, n.arguments[0].as_expression()?) != fs(register)
    {
        return None;
    }
    let Expression::ComputedMemberExpression(sub) = arguments(c)?[0] else {
        return None;
    };
    if sub.optional
        || symbol(m, &sub.object) != Some(ps[0])
        || symbol(m, &sub.expression) != Some(index)
    {
        return None;
    }
    let mut setters = vec![];
    let mut targets = vec![];
    let mut flag = None;
    // true may be initialized before, between, or after setter declarations.
    while let Some((s, v, st)) = effective.get(pos).copied() {
        if let Some(v) = v {
            if !boolean(v, true) || flag.replace(s).is_some() {
                return None;
            }
            pos += 1;
            continue;
        }
        let st = st?;
        if let Some(f) = fun(st) {
            let n = f.params.items.len();
            if n != 1 && n != 2 {
                return None;
            }
            targets.push(setter(m, f, n)?);
            setters.push(fs(f)?);
            pos += 1;
            continue;
        }
        if let Some((s, v)) = set(m, st) {
            if !boolean(v, true) || flag.replace(s).is_some() {
                return None;
            }
            pos += 1;
            continue;
        }
        break;
    }
    if setters.len() != 4 {
        return None;
    }
    let (_, _, Some(Statement::TryStatement(t))) = *effective.get(pos)? else {
        return None;
    };
    pos += 1;
    if pos != effective.len() || t.finalizer.is_some() {
        return None;
    }
    let inner = items(m, &t.block.body)?;
    let [Item::Decl(temp, None), Item::Decl(input, Some(v)), Item::St(st)] = inner.as_slice()
    else {
        return None;
    };
    if symbol(m, v) != Some(list) {
        return None;
    }
    let (yes, no) = branch(m, st, *input)?;
    let setup = call(expression_statement(yes.first()?)?)?;
    let setup_args = arguments(setup)?;
    if setup_args.len() != 6 {
        return None;
    }
    let order: Vec<_> = setup_args[2..]
        .iter()
        .map(|e| symbol(m, e))
        .collect::<Option<Vec<_>>>()?;
    if order.iter().copied().collect::<BTreeSet<_>>().len() != 4 {
        return None;
    }
    let ordered: Vec<_> = order
        .iter()
        .map(|s| {
            setters
                .iter()
                .position(|x| x == s)
                .map(|i| targets[i].clone())
        })
        .collect::<Option<Vec<_>>>()?;
    if ordered.iter().map(Vec::len).collect::<Vec<_>>() != vec![1, 2, 1, 1] {
        return None;
    }
    setters = order;
    let x = State {
        list,
        guard: alias,
        first: ordered[0][0],
        middle: ordered[1][0],
        flag: ordered[1][1],
        last: ordered[2][0],
        cleanup: ordered[3][0],
    };
    if flag != Some(x.flag) {
        return None;
    }
    let (_, a, sa) = arm(m, yes, x, &ps, &setters, *temp, *input, false)?;
    let (_, b, sb) = arm(m, no, x, &ps, &setters, *temp, *input, true)?;
    if sa != sb {
        return None;
    }
    // Sole outer catch broadcasts and returns its own empty function.
    let h = t.handler.as_ref()?;
    let [error, send, empty, returned] = h.body.body.as_slice() else {
        return None;
    };
    let e = plain_binding(m, &h.param.as_ref()?.pattern)?;
    let err = if let Some(v) = variable(error) {
        if symbol(m, v.init.as_ref()?) != Some(e) {
            return None;
        }
        plain_var(m, v)?
    } else {
        let (s, v) = set(m, error)?;
        if symbol(m, v) != Some(e) {
            return None;
        }
        s
    };
    broadcast(m, send, x.list, err)?;
    let empty = fun(empty)?;
    params(m, empty, 0)?;
    if !fb(empty)?.is_empty() || symbol(m, ret(returned)?) != fs(empty) {
        return None;
    }
    // Closed local writer census: no callback or alias may donate an effect.
    let mut expected = vec![
        (list, usize::from(!initialized.contains(&list))),
        (index, 1),
        (x.first, 3),
        (x.middle, 1),
        (x.flag, 1 + usize::from(!initialized.contains(&x.flag))),
        (x.last, 3),
        (x.cleanup, 1),
        (alias, 0),
    ];
    if declared.contains(&err) {
        expected.push((err, 1));
    }
    if declared.len() != expected.len()
        || expected
            .iter()
            .any(|(s, n)| !declared.contains(s) || m.binding_writes(*s) != *n)
    {
        return None;
    }
    let all = [
        ps.as_slice(),
        &[
            list, index, x.first, x.middle, x.flag, x.last, x.cleanup, alias, guard, ctor, sa,
            *temp, *input,
        ],
    ]
    .concat();
    if all.iter().copied().collect::<BTreeSet<_>>().len() != all.len() {
        return None;
    }
    // Declaration grouping is neutral only while every owned read/write still
    // follows its declaration's execution. Follow deferred functions/aliases to
    // their real exposures, so a moved let cannot silently introduce TDZ.
    for local in &declared {
        let declaration = m.scoping.symbol_declaration(*local);
        let statement = m.nodes.parent_id(declaration);
        let body = m.nodes.parent_id(statement);
        let end = m.nodes.kind(declaration).span().end;
        if !m
            .scoping
            .get_resolved_references(*local)
            .all(|r| callable_origin_precedes(m, body, end, r.node_id(), None, 0, true))
        {
            return None;
        }
    }

    let mut out = vec![
        (f.span, "runObserverBatchLifecycle"),
        (guard_fn.span, "invokeBatchCallbackOrBroadcastError"),
        (register.span, "registerBatchObserver"),
        (a, "clearBatchCallbacksAndRunCleanup"),
        (b, "clearBatchCallbacksAndRunCleanup"),
    ];
    for (s, name) in setters.iter().zip([
        "setBatchInitialCallback",
        "setBatchCallbackAndSchedulingFlag",
        "setBatchFinalCallback",
        "setBatchCleanupCallback",
    ]) {
        let st = effective
            .iter()
            .filter_map(|(_, _, s)| s.and_then(fun))
            .find(|f| fs(f) == Some(*s))?;
        out.push((st.span, name));
    }
    Some(out)
}

pub(in crate::naming) fn evaluate(
    m: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
) -> Vec<Proposal> {
    let mut current = Some(site);
    while let Some(s) = current {
        if let AstKind::Function(f) = m.nodes.kind(s.node_id) {
            if let Some(roles) = prove(m, f) {
                if let Some((_, name)) = roles.iter().find(|(span, _)| *span == site.span) {
                    return vec![proposal(name,"shared_observer_batch_lifecycle","complete own observer-batch protocol with exact scoped constructor input, registration loop, guarded caught-error broadcast, four ordered setter inputs, configurable immediate/deferred callback schedule in both branches, sentinel reset and guarded cleanup; every local writer and effect consumed, opaque external callable/constructor/sentinel purposes unasserted",facts)];
                }
            }
        }
        current = m.enclosing_function(s.node_id);
    }
    vec![]
}
