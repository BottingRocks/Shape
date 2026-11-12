//! Whole-owned subscription adapters. External reducers, predicates and source
//! methods retain their opaque purposes; constructor identity and readiness use
//! the independent complete native factory proof.
use crate::naming::shared::*;
use crate::naming::*;
use oxc_span::GetSpan;
type Sym = SymbolId;
mod readiness;

fn id(m: &Model<'_, '_>, e: &Expression<'_>) -> Option<Sym> {
    crate::analysis::expression_symbol(e.without_parentheses(), m.scoping)
}
fn bind(p: &BindingPattern<'_>) -> Option<Sym> {
    let BindingPattern::BindingIdentifier(i) = p else {
        return None;
    };
    i.symbol_id.get()
}
fn fixed(m: &Model<'_, '_>, s: Sym) -> bool {
    m.binding_writes(s) == 0 && m.scoping.symbol_redeclarations(s).is_empty()
}
fn hoisted_declaration(m: &Model<'_, '_>, s: Sym) -> bool {
    let node = m.scoping.symbol_declaration(s);
    fixed(m, s)
        && matches!(m.nodes.kind(node), AstKind::Function(f) if f.r#type == FunctionType::FunctionDeclaration)
        && matches!(
            m.nodes.parent_kind(node),
            AstKind::FunctionBody(_) | AstKind::Program(_)
        )
}
fn formals(m: &Model<'_, '_>, p: &FormalParameters<'_>, n: usize) -> Option<Vec<Sym>> {
    if p.rest.is_some() || p.items.len() != n {
        return None;
    }
    p.items
        .iter()
        .map(|p| {
            let s = bind(&p.pattern)?;
            (p.initializer.is_none() && fixed(m, s)).then_some(s)
        })
        .collect()
}
fn params(m: &Model<'_, '_>, f: &Function<'_>, n: usize) -> Option<Vec<Sym>> {
    (!f.r#async && !f.generator && f.r#type == FunctionType::FunctionDeclaration).then_some(())?;
    hoisted_declaration(m, fs(f)?).then_some(())?;
    formals(m, &f.params, n)
}
fn body<'a>(f: &'a Function<'a>) -> Option<&'a [Statement<'a>]> {
    let b = f.body.as_ref()?;
    (!f.r#async && !f.generator && b.directives.is_empty()).then_some(b.statements.as_slice())
}
fn expr<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    if let Statement::ExpressionStatement(s) = s {
        Some(&s.expression)
    } else {
        None
    }
}
fn ret<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    if let Statement::ReturnStatement(s) = s {
        s.argument.as_ref()
    } else {
        None
    }
}
fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    if let Statement::BlockStatement(s) = s {
        Some(s.body.as_slice())
    } else {
        None
    }
}
fn variable<'a>(s: &'a Statement<'a>) -> Option<&'a VariableDeclarator<'a>> {
    let Statement::VariableDeclaration(d) = s else {
        return None;
    };
    let [v] = d.declarations.as_slice() else {
        return None;
    };
    Some(v)
}
fn alias(m: &Model<'_, '_>, s: &Statement<'_>, from: Sym) -> Option<Sym> {
    let v = variable(s)?;
    let to = bind(&v.id)?;
    (fixed(m, to) && id(m, v.init.as_ref()?) == Some(from)).then_some(to)
}
/// Two distinct local copies of immutable formals commute: neither
/// initializer reads the other local or performs an effect. Return semantic
/// parameter order while consuming both source statements exactly once.
fn formal_copies<'a>(
    m: &Model<'_, '_>,
    first: &'a Statement<'a>,
    second: &'a Statement<'a>,
    p: &[Sym],
) -> Option<(&'a Statement<'a>, &'a Statement<'a>)> {
    let a = variable(first)?;
    let b = variable(second)?;
    let sa = bind(&a.id)?;
    let sb = bind(&b.id)?;
    if sa == sb || p.contains(&sa) || p.contains(&sb) {
        return None;
    }
    match (id(m, a.init.as_ref()?), id(m, b.init.as_ref()?)) {
        (Some(x), Some(y)) if x == p[0] && y == p[1] => Some((first, second)),
        (Some(x), Some(y)) if x == p[1] && y == p[0] => Some((second, first)),
        _ => None,
    }
}
fn function<'a>(s: &'a Statement<'a>) -> Option<&'a Function<'a>> {
    if let Statement::FunctionDeclaration(f) = s {
        Some(f)
    } else {
        None
    }
}
fn fs(f: &Function<'_>) -> Option<Sym> {
    f.id.as_ref()?.symbol_id.get()
}
fn call<'a>(e: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    if let Expression::CallExpression(c) = e.without_parentheses() {
        (!c.optional).then_some(c)
    } else {
        None
    }
}
fn args<'a>(c: &'a CallExpression<'a>, n: usize) -> Option<Vec<&'a Expression<'a>>> {
    if c.arguments.len() != n {
        return None;
    }
    c.arguments.iter().map(Argument::as_expression).collect()
}
fn method<'a>(
    m: &Model<'_, '_>,
    e: &'a Expression<'a>,
    receiver: Sym,
    key: &str,
    n: usize,
) -> Option<Vec<&'a Expression<'a>>> {
    let c = call(e)?;
    let Expression::StaticMemberExpression(p) = c.callee.without_parentheses() else {
        return None;
    };
    (!p.optional && p.property.name == key && id(m, &p.object) == Some(receiver)).then_some(())?;
    args(c, n)
}
fn bin<'a>(e: &'a Expression<'a>, op: &str) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::BinaryExpression(b) = e.without_parentheses() else {
        return None;
    };
    (b.operator.as_str() == op).then_some((&b.left, &b.right))
}
fn logical<'a>(
    e: &'a Expression<'a>,
    op: &str,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::LogicalExpression(b) = e.without_parentheses() else {
        return None;
    };
    (b.operator.as_str() == op).then_some((&b.left, &b.right))
}
fn void(e: &Expression<'_>) -> bool {
    matches!(e.without_parentheses(),Expression::UnaryExpression(u) if u.operator.as_str()=="void" && matches!(u.argument.without_parentheses(),Expression::NumericLiteral(n) if n.value==0.0))
}
fn type_test(m: &Model<'_, '_>, e: &Expression<'_>, s: Sym, op: &str) -> Option<()> {
    let (a, b) = bin(e, op)?;
    let Expression::UnaryExpression(u) = a.without_parentheses() else {
        return None;
    };
    (u.operator.as_str() == "typeof"
        && id(m, &u.argument) == Some(s)
        && matches!(b.without_parentheses(),Expression::StringLiteral(v) if v.value=="function"))
    .then_some(())
}
fn assigned<'a>(m: &Model<'_, '_>, e: &'a Expression<'a>, s: Sym) -> Option<&'a Expression<'a>> {
    let Expression::AssignmentExpression(a) = e.without_parentheses() else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(i) = &a.left else {
        return None;
    };
    (a.operator.as_str() == "=" && crate::analysis::identifier_symbol(i, m.scoping) == Some(s))
        .then_some(&a.right)
}
fn store<'a>(m: &Model<'_, '_>, s: &'a Statement<'a>, to: Sym) -> Option<&'a Expression<'a>> {
    assigned(m, expr(s)?, to)
}
fn arrow<'a>(
    m: &Model<'_, '_>,
    e: &'a Expression<'a>,
    n: usize,
) -> Option<(&'a ArrowFunctionExpression<'a>, Vec<Sym>)> {
    let Expression::ArrowFunctionExpression(a) = e.without_parentheses() else {
        return None;
    };
    (!a.r#async && a.body.directives.is_empty()).then_some(())?;
    Some((a, formals(m, &a.params, n)?))
}
fn arrow_value<'a>(a: &'a ArrowFunctionExpression<'a>) -> Option<&'a Expression<'a>> {
    let [s] = a.body.statements.as_slice() else {
        return None;
    };
    if a.expression {
        expr(s)
    } else {
        expr(s).or_else(|| ret(s))
    }
}
fn forward(m: &Model<'_, '_>, e: &Expression<'_>, sink: Sym, key: &str, value: bool) -> Option<()> {
    let (a, p) = arrow(m, e, usize::from(value))?;
    let xs = method(m, arrow_value(a)?, sink, key, usize::from(value))?;
    (!value || id(m, xs[0]) == Some(p[0])).then_some(())
}
fn emit(m: &Model<'_, '_>, s: &Statement<'_>, sink: Sym, value: Sym) -> Option<()> {
    let a = method(m, expr(s)?, sink, "next", 1)?;
    (id(m, a[0]) == Some(value)).then_some(())
}
fn complete(m: &Model<'_, '_>, s: &Statement<'_>, sink: Sym) -> Option<()> {
    method(m, expr(s)?, sink, "complete", 0)?;
    Some(())
}
fn catches(m: &Model<'_, '_>, t: &TryStatement<'_>, sink: Sym) -> Option<()> {
    if t.finalizer.is_some() {
        return None;
    }
    let h = t.handler.as_ref()?;
    let e = bind(&h.param.as_ref()?.pattern)?;
    fixed(m, e).then_some(())?;
    let [s] = h.body.body.as_slice() else {
        return None;
    };
    let a = method(m, expr(s)?, sink, "error", 1)?;
    (id(m, a[0]) == Some(e)).then_some(())
}

/// A single executed empty-object store in a direct enclosing body. Every
/// factory exposure must follow it; deferred nested writers are never stores.
fn sentinel(m: &Model<'_, '_>, s: Sym, factory: Sym) -> Option<()> {
    if !m.scoping.symbol_redeclarations(s).is_empty() {
        return None;
    }
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
    let (value, end) = if let Some(e) = &v.init {
        if m.binding_writes(s) != 0 {
            return None;
        }
        (e, v.span.end)
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
        let statement = m.nodes.parent_id(node);
        if !matches!(m.nodes.kind(statement), AstKind::ExpressionStatement(_))
            || m.nodes.parent_id(statement) != owner
            || a.span.start <= v.span.end
        {
            return None;
        }
        let AssignmentTarget::AssignmentTargetIdentifier(i) = &a.left else {
            return None;
        };
        if a.operator.as_str() != "=" || crate::analysis::identifier_symbol(i, m.scoping) != Some(s)
        {
            return None;
        }
        (&a.right, a.span.end)
    };
    if !matches!(value.without_parentheses(),Expression::ObjectExpression(o) if o.properties.is_empty())
    {
        return None;
    }
    m.scoping
        .get_resolved_references(factory)
        .all(|r| callable_origin_precedes(m, owner, end, r.node_id(), None, 0, true))
        .then_some(())
        .or_else(|| readiness::publications(m, factory, owner, end, None).then_some(()))
}

struct Subscription<'a> {
    sink: Sym,
    state: Sym,
    state_assignment: usize,
    initial: &'a Expression<'a>,
    callbacks: Vec<&'a Expression<'a>>,
}
/// Consume every executor statement. A scratch holder is used only after its
/// executed store; neutral extra declarations must have zero references/writes.
fn subscription<'a>(
    m: &Model<'_, '_>,
    executor: &'a ArrowFunctionExpression<'a>,
    input: Sym,
) -> Option<Subscription<'a>> {
    let ps = formals(m, &executor.params, 1)?;
    let [capture, prefix @ .., cleanup, finish] = executor.body.statements.as_slice() else {
        return None;
    };
    if executor.r#async || executor.expression || !executor.body.directives.is_empty() {
        return None;
    }
    let sink = alias(m, capture, ps[0])?;
    let cleanup = function(cleanup)?;
    params(m, cleanup, 0)?;
    let cs = fs(cleanup)?;
    if !fixed(m, cs) || m.binding_references(cs) != 1 || id(m, ret(finish)?) != Some(cs) {
        return None;
    }
    let mut bare = BTreeSet::new();
    let mut values = Vec::new();
    for statement in prefix {
        match statement {
            Statement::VariableDeclaration(d) => {
                for v in &d.declarations {
                    let s = bind(&v.id)?;
                    if !m.scoping.symbol_redeclarations(s).is_empty() {
                        return None;
                    }
                    if let Some(e) = &v.init {
                        values.push((s, e, 0));
                    } else if !bare.insert(s) {
                        return None;
                    }
                }
            }
            _ => {
                let Expression::AssignmentExpression(a) = expr(statement)? else {
                    return None;
                };
                let AssignmentTarget::AssignmentTargetIdentifier(i) = &a.left else {
                    return None;
                };
                let s = crate::analysis::identifier_symbol(i, m.scoping)?;
                if !bare.remove(&s) {
                    return None;
                }
                let value = assigned(m, expr(statement)?, s)?;
                values.push((s, value, 1));
            }
        }
    }
    if bare
        .iter()
        .any(|s| m.binding_references(*s) != 0 || m.binding_writes(*s) != 0)
    {
        return None;
    }
    let [(state, initial, state_assignment), (sub, sub_value, sub_assignment)] = values.as_slice()
    else {
        return None;
    };
    if state == sub || *state == sink || *sub == sink || m.binding_writes(*sub) != *sub_assignment {
        return None;
    }
    let callbacks = method(m, sub_value, input, "subscribe", 3)?;
    let [s] = body(cleanup)? else { return None };
    method(m, ret(s)?, *sub, "unsubscribe", 0)?;
    Some(Subscription {
        sink,
        state: *state,
        state_assignment: *state_assignment,
        initial,
        callbacks,
    })
}
fn new_executor_with_origin<'a>(
    m: &Model<'_, '_>, e: &'a Expression<'a>, factory: Sym, assignment_copies: bool, ordinary: bool,
) -> Option<&'a ArrowFunctionExpression<'a>> {
    let Expression::NewExpression(n) = e.without_parentheses() else { return None; };
    if ordinary {
        closed_constructor(m, id(m, &n.callee)?, &mut BTreeSet::new(), 32)?;
        observables::observable_map_operator::ordinary_origin(m, &n.callee, factory, 8)
            .or_else(|| assignment_copies.then(|| readiness::ordinary_constructor(m, &n.callee, factory, 8)).flatten())?;
    } else {
    observables::observable_map_operator::native_origin(m, &n.callee, factory, 8).or_else(|| {
        assignment_copies
            .then(|| readiness::constructor(m, &n.callee, factory, 8))
            .flatten()
    })?;
    }
    let [a] = n.arguments.as_slice() else {
        return None;
    };
    let (a, _) = arrow(m, a.as_expression()?, 1)?;
    Some(a)
}
/// The full native origin proves the class body and invocation epochs. This
/// additional local check keeps its constructor object closed against aliases
/// exposed to arbitrary code or API writes. Transparent constructor copies
/// must execute after their producer and are checked in both directions.
pub(in crate::naming) fn closed_constructor(
    m: &Model<'_, '_>,
    s: Sym,
    seen: &mut BTreeSet<Sym>,
    depth: usize,
) -> Option<()> {
    if depth == 0 {
        return None;
    }
    if !seen.insert(s) {
        return Some(());
    }
    let (value, owner, end) = observables::observable_map_operator::origin(m, s)?;
    if let Some(producer) = id(m, value) {
        closed_constructor(m, producer, seen, depth - 1)?;
    }
    for r in m.scoping.get_resolved_references(s).filter(|r| r.is_read()) {
        let node = r.node_id();
        let span = m.nodes.kind(node).span();
        let parent = m.nodes.parent_id(node);
        let ordinary = match m.nodes.kind(parent) {
            AstKind::NewExpression(n) => n.callee.span() == span,
            AstKind::StaticMemberExpression(p)
                if !p.optional
                    && p.object.span() == span
                    && matches!(p.property.name.as_str(), "from" | "of" | "empty") =>
            {
                matches!(m.nodes.kind(m.nodes.parent_id(parent)),AstKind::CallExpression(c) if !c.optional && c.callee.span()==p.span)
            }
            // A reference in the unchosen arm cannot expose the constructor.
            // Admit only a single-origin empty-array test whose executed store
            // precedes this exact read; array contents cannot change truthiness.
            AstKind::ConditionalExpression(c) if c.alternate.span() == span => {
                id(m, &c.test).and_then(|test| observables::observable_map_operator::origin(m, test))
                    .is_some_and(|(value, test_owner, test_end)| {
                        let scope = m.nodes.kind(test_owner).span();
                        let dynamic_scope = m.nodes.iter().any(|n| {
                            scope.contains_inclusive(n.kind().span())
                                && (matches!(n.kind(), AstKind::WithStatement(_))
                                    || matches!(n.kind(), AstKind::CallExpression(c) if matches!(c.callee.without_parentheses(), Expression::Identifier(i) if i.name == "eval")))
                        });
                        !dynamic_scope
                            && matches!(value.without_parentheses(), Expression::ArrayExpression(a) if a.elements.is_empty())
                            && callable_origin_precedes(m, test_owner, test_end, node, None, 0, false)
                    })
            }
            AstKind::UnaryExpression(u) => u.operator.as_str() == "typeof",
            AstKind::BinaryExpression(b) => {
                b.operator.as_str() == "instanceof" && b.right.span() == span
            }
            _ => false,
        };
        if ordinary {
            continue;
        }
        let alias = match m.nodes.kind(parent) {
            AstKind::VariableDeclarator(v) if v.init.as_ref().is_some_and(|v| v.span() == span) => {
                bind(&v.id)
            }
            AstKind::AssignmentExpression(a)
                if a.right.span() == span && a.operator.as_str() == "=" =>
            {
                match &a.left {
                    AssignmentTarget::AssignmentTargetIdentifier(i) => {
                        crate::analysis::identifier_symbol(i, m.scoping)
                    }
                    _ => None,
                }
            }
            _ => None,
        }?;
        let (copy, copy_owner, _) = observables::observable_map_operator::origin(m, alias)?;
        if copy.span() != span
            || copy_owner != owner
            || !callable_origin_precedes(m, owner, end, node, None, 0, false)
        {
            return None;
        }
        closed_constructor(m, alias, seen, depth - 1)?;
    }
    Some(())
}
fn operator_constructor_closed(m: &Model<'_, '_>, inner: &Function<'_>) -> Option<()> {
    let [_, finish] = body(inner)? else {
        return None;
    };
    let Expression::NewExpression(n) = ret(finish)?.without_parentheses() else {
        return None;
    };
    closed_constructor(m, id(m, &n.callee)?, &mut BTreeSet::new(), 32)
}
#[cfg(test)]
fn operator<'a>(
    m: &Model<'_, '_>,
    f: &'a Function<'a>,
    returned: &Statement<'_>,
) -> Option<(Sym, &'a ArrowFunctionExpression<'a>)> {
    operator_with_origin(m, f, returned, false)
}
fn operator_with_origin<'a>(
    m: &Model<'_, '_>, f: &'a Function<'a>, returned: &Statement<'_>, ordinary: bool,
) -> Option<(Sym, &'a ArrowFunctionExpression<'a>)> {
    let s = fs(f)?;
    if !fixed(m, s) || m.binding_references(s) != 1 || id(m, ret(returned)?) != Some(s) {
        return None;
    }
    let input = params(m, f, 1)?[0];
    let [capture, finish] = body(f)? else {
        return None;
    };
    let source = alias(m, capture, input)?;
    let parent = m.enclosing_function(m.scoping.symbol_declaration(s))?;
    let factory = parent.binding_symbol?;
    Some((source, new_executor_with_origin(m, ret(finish)?, factory, true, ordinary)?))
}
fn scan<'a>(m: &Model<'_, '_>, f: &'a Function<'a>) -> Option<Vec<(Span, &'static str)>> {
    scan_with_origin(m, f, false)
}
fn scan_with_origin<'a>(m: &Model<'_, '_>, f: &'a Function<'a>, ordinary: bool) -> Option<Vec<(Span, &'static str)>> {
    let p = params(m, f, 2)?;
    let [capture, initial, default, inner, returned] = body(f)? else {
        return None;
    };
    let (capture, initial) = formal_copies(m, capture, initial, &p)?;
    let reducer = alias(m, capture, p[0])?;
    let v = variable(initial)?;
    let seed = bind(&v.id)?;
    if id(m, v.init.as_ref()?) != Some(p[1])
        || m.binding_writes(seed) != 1
        || !m.scoping.symbol_redeclarations(seed).is_empty()
    {
        return None;
    }
    let Statement::IfStatement(default) = default else {
        return None;
    };
    if default.alternate.is_some() {
        return None;
    }
    let (a, b) = bin(&default.test, "===")?;
    if id(m, a) != Some(p[1]) || !void(b) {
        return None;
    }
    let [s] = block(&default.consequent)? else {
        return None;
    };
    let sent = id(m, store(m, s, seed)?)?;
    let factory = fs(f)?;
    fixed(m, factory).then_some(())?;
    sentinel(m, sent, factory)?;
    let inner = function(inner)?;
    let (input, executor) = operator_with_origin(m, inner, returned, ordinary)?;
    operator_constructor_closed(m, inner)?;
    let sub = subscription(m, executor, input)?;
    if id(m, sub.initial) != Some(seed) || m.binding_writes(sub.state) != sub.state_assignment + 2 {
        return None;
    }
    let (next, p) = arrow(m, sub.callbacks[0], 1)?;
    if next.expression {
        return None;
    }
    let [Statement::IfStatement(branch)] = next.body.statements.as_slice() else {
        return None;
    };
    let (a, b) = bin(&branch.test, "===")?;
    if id(m, a) != Some(sub.state) || id(m, b) != Some(sent) {
        return None;
    }
    let [adopt, finish] = block(&branch.consequent)? else {
        return None;
    };
    if id(m, store(m, adopt, sub.state)?) != Some(p[0]) || !void(ret(finish)?) {
        return None;
    }
    let [Statement::TryStatement(t), finish] = block(branch.alternate.as_ref()?)? else {
        return None;
    };
    if !void(ret(finish)?) {
        return None;
    }
    catches(m, t, sub.sink)?;
    let [fold, send] = t.block.body.as_slice() else {
        return None;
    };
    let c = call(store(m, fold, sub.state)?)?;
    let a = args(c, 2)?;
    if id(m, &c.callee) != Some(reducer)
        || id(m, a[0]) != Some(sub.state)
        || id(m, a[1]) != Some(p[0])
    {
        return None;
    }
    emit(m, send, sub.sink, sub.state)?;
    forward(m, sub.callbacks[1], sub.sink, "error", true)?;
    forward(m, sub.callbacks[2], sub.sink, "complete", false)?;
    Some(vec![
        (f.span, "createScanOperator"),
        (inner.span, "scanOperatorFunction"),
    ])
}
fn last<'a>(m: &Model<'_, '_>, f: &'a Function<'a>) -> Option<Vec<(Span, &'static str)>> {
    last_with_origin(m, f, false)
}
fn last_with_origin<'a>(m: &Model<'_, '_>, f: &'a Function<'a>, ordinary: bool) -> Option<Vec<(Span, &'static str)>> {
    let p = params(m, f, 2)?;
    let [capture, default, inner, returned] = body(f)? else {
        return None;
    };
    let (capture, default) = formal_copies(m, capture, default, &p)?;
    let predicate = alias(m, capture, p[0])?;
    let fallback = alias(m, default, p[1])?;
    let factory = fs(f)?;
    fixed(m, factory).then_some(())?;
    let inner = function(inner)?;
    let (input, executor) = operator_with_origin(m, inner, returned, ordinary)?;
    operator_constructor_closed(m, inner)?;
    let sub = subscription(m, executor, input)?;
    let sent = id(m, sub.initial)?;
    sentinel(m, sent, factory)?;
    if m.binding_writes(sub.state) != sub.state_assignment + 2 {
        return None;
    }
    let (next, p) = arrow(m, sub.callbacks[0], 1)?;
    if next.expression {
        return None;
    }
    let [Statement::TryStatement(t)] = next.body.statements.as_slice() else {
        return None;
    };
    catches(m, t, sub.sink)?;
    let [Statement::IfStatement(branch)] = t.block.body.as_slice() else {
        return None;
    };
    let (a, b) = logical(&branch.test, "&&")?;
    type_test(m, a, predicate, "===")?;
    let c = call(b)?;
    let a = args(c, 1)?;
    if id(m, &c.callee) != Some(predicate) || id(m, a[0]) != Some(p[0]) {
        return None;
    }
    let [s] = block(&branch.consequent)? else {
        return None;
    };
    if id(m, store(m, s, sub.state)?) != Some(p[0]) {
        return None;
    }
    let Statement::IfStatement(other) = branch.alternate.as_ref()? else {
        return None;
    };
    if other.alternate.is_some() {
        return None;
    }
    type_test(m, &other.test, predicate, "!==")?;
    let [s] = block(&other.consequent)? else {
        return None;
    };
    if id(m, store(m, s, sub.state)?) != Some(p[0]) {
        return None;
    }
    forward(m, sub.callbacks[1], sub.sink, "error", true)?;
    let (done, _) = arrow(m, sub.callbacks[2], 0)?;
    if done.expression {
        return None;
    }
    let [Statement::IfStatement(absent)] = done.body.statements.as_slice() else {
        return None;
    };
    let (a, b) = bin(&absent.test, "===")?;
    if id(m, a) != Some(sub.state) || id(m, b) != Some(sent) {
        return None;
    }
    let [Statement::IfStatement(default)] = block(&absent.consequent)? else {
        return None;
    };
    let (a, b) = bin(&default.test, "===")?;
    if id(m, a) != Some(fallback) || !void(b) {
        return None;
    }
    let [error] = block(&default.consequent)? else {
        return None;
    };
    let a = method(m, expr(error)?, sub.sink, "error", 1)?;
    if !matches!(a[0].without_parentheses(),Expression::StringLiteral(s) if s.value=="No elements in sequence")
    {
        return None;
    }
    let [send, finish] = block(default.alternate.as_ref()?)? else {
        return None;
    };
    emit(m, send, sub.sink, fallback)?;
    complete(m, finish, sub.sink)?;
    let [send, finish] = block(absent.alternate.as_ref()?)? else {
        return None;
    };
    emit(m, send, sub.sink, sub.state)?;
    complete(m, finish, sub.sink)?;
    Some(vec![
        (f.span, "createLastOperator"),
        (inner.span, "lastOperatorFunction"),
    ])
}
struct ArgumentsOrigin<'m, 's, 'a> {
    model: &'m Model<'s, 'a>,
    valid: bool,
}
impl<'a> Visit<'a> for ArgumentsOrigin<'_, '_, 'a> {
    fn visit_identifier_reference(&mut self, reference: &IdentifierReference<'a>) {
        // The normal invoker owns implicit arguments. An explicit lexical/var
        // binding with that spelling can overwrite the source being copied.
        if reference.name == "arguments" {
            self.valid &=
                crate::analysis::identifier_symbol(reference, self.model.scoping).is_none();
        }
    }
}

fn callback<'a>(m: &Model<'_, '_>, f: &'a Function<'a>) -> Option<Vec<(Span, &'static str)>> {
    callback_with_origin(m, f, false)
}
fn callback_with_origin<'a>(m: &Model<'_, '_>, f: &'a Function<'a>, ordinary: bool) -> Option<Vec<(Span, &'static str)>> {
    let p = params(m, f, 1)?;
    let [capture, inner, returned] = body(f)? else {
        return None;
    };
    let target = alias(m, capture, p[0])?;
    let factory = fs(f)?;
    fixed(m, factory).then_some(())?;
    let inner = function(inner)?;
    params(m, inner, 0)?;
    let is = fs(inner)?;
    if !fixed(m, is) || m.binding_references(is) != 1 || id(m, ret(returned)?) != Some(is) {
        return None;
    }
    let statements = body(inner)?;
    let mut arguments_origin = ArgumentsOrigin {
        model: m,
        valid: true,
    };
    arguments_origin.visit_function_body(inner.body.as_ref()?);
    arguments_origin.valid.then_some(())?;
    let (_, copied, _, n) = observables::observable_factory::copy_arguments(m, statements)?;
    let [finish] = statements.get(n..)? else {
        return None;
    };
    let executor = new_executor_with_origin(m, ret(finish)?, factory, false, ordinary)?;
    let p = formals(m, &executor.params, 1)?;
    let [capture, Statement::IfStatement(branch)] = executor.body.statements.as_slice() else {
        return None;
    };
    if executor.expression {
        return None;
    }
    let sink = alias(m, capture, p[0])?;
    type_test(m, &branch.test, target, "!==")?;
    let [reject, finish] = block(&branch.consequent)? else {
        return None;
    };
    if !void(ret(finish)?) {
        return None;
    }
    let a = method(m, expr(reject)?, sink, "error", 1)?;
    let Expression::NewExpression(error) = a[0].without_parentheses() else {
        return None;
    };
    if !error.arguments.is_empty()
        || !matches!(error.callee.without_parentheses(),Expression::Identifier(i) if i.name=="TypeError"&&crate::analysis::identifier_symbol(i,m.scoping).is_none())
    {
        return None;
    }
    let [apply, finish] = block(branch.alternate.as_ref()?)? else {
        return None;
    };
    if !void(ret(finish)?) {
        return None;
    }
    let a = method(m, expr(apply)?, target, "apply", 2)?;
    if !matches!(a[0].without_parentheses(), Expression::NullLiteral(_)) {
        return None;
    }
    let a = method(m, a[1], copied, "concat", 2)?;
    let (success, p) = arrow(m, a[0], 1)?;
    if success.expression {
        return None;
    }
    let [send, finish] = success.body.statements.as_slice() else {
        return None;
    };
    emit(m, send, sink, p[0])?;
    complete(m, finish, sink)?;
    forward(m, a[1], sink, "error", true)?;
    Some(vec![
        (f.span, "createCallbackObservableAdapter"),
        (inner.span, "callbackObservableInvoker"),
    ])
}

/// An alias RHS is an executed read. Later exposures cannot repair a producer
/// that was unavailable at this exact read, even for immutable copies.
fn alias_reads_ready(m: &Model<'_, '_>, e: &Expression<'_>, depth: usize) -> Option<()> {
    if depth == 0 {
        return None;
    }
    let s = id(m, e)?;
    if matches!(
        m.nodes.kind(m.scoping.symbol_declaration(s)),
        AstKind::Function(_)
    ) {
        return hoisted_declaration(m, s).then_some(());
    }
    let (value, owner, _) = observables::observable_map_operator::origin(m, s)?;
    if let Some(producer) = id(m, value) {
        if !hoisted_declaration(m, producer) {
            let (_, producer_owner, producer_end) =
                observables::observable_map_operator::origin(m, producer)?;
            if producer_owner != owner || producer_end > value.span().start {
                return None;
            }
        }
        return alias_reads_ready(m, value, depth - 1);
    }
    matches!(
        value.without_parentheses(),
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
    )
    .then_some(())
}

/// Helper purposes remain opaque; the name describes only the fully consumed
/// own construction and source.pipe invocation.
fn two_stage<'a>(m: &Model<'_, '_>, f: &'a Function<'a>) -> Option<Vec<(Span, &'static str)>> {
    let p = params(m, f, 2)?;
    let [first_capture, second_capture, inner, returned] = body(f)? else {
        return None;
    };
    let mut captures = [None, None];
    for statement in [first_capture, second_capture] {
        let v = variable(statement)?;
        let symbol = bind(&v.id)?;
        if !fixed(m, symbol) {
            return None;
        }
        let init = v.init.as_ref()?;
        let role = p.iter().position(|s| id(m, init) == Some(*s))?;
        if captures[role].replace(symbol).is_some() {
            return None;
        }
    }
    let first = captures[0]?;
    let second = captures[1]?;
    let outer = fs(f)?;
    fixed(m, outer).then_some(())?;
    let inner = function(inner)?;
    let source = params(m, inner, 1)?[0];
    let inner_symbol = fs(inner)?;
    if !fixed(m, inner_symbol)
        || m.binding_references(inner_symbol) != 1
        || id(m, ret(returned)?) != Some(inner_symbol)
    {
        return None;
    }
    let [finish] = body(inner)? else {
        return None;
    };
    let stages = method(m, ret(finish)?, source, "pipe", 2)?;
    let first_call = call(stages[0])?;
    let second_call = call(stages[1])?;
    // Check the callable's executed initialization and every alias read and
    // exposure. No helper body donates an unchecked purpose to this wrapper.
    stable_function(m, &first_call.callee, 0)?;
    stable_function(m, &second_call.callee, 0)?;
    alias_reads_ready(m, &first_call.callee, 12)?;
    alias_reads_ready(m, &second_call.callee, 12)?;
    let a = args(first_call, 2)?;
    let b = args(second_call, 2)?;
    if id(m, a[0]) != Some(first)
        || id(m, a[1]) != Some(second)
        || !void(b[0])
        || id(m, b[1]) != Some(second)
    {
        return None;
    }
    Some(vec![
        (f.span, "createTwoStagePipeOperator"),
        (inner.span, "applyTwoStagePipeOperator"),
    ])
}

pub(in crate::naming) fn evaluate(
    m: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
    _: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let mut owner = Some(site);
    for _ in 0..2 {
        let Some(candidate) = owner else { break };
        if let AstKind::Function(f) = m.nodes.kind(candidate.node_id) {
            for (proof, evaluator) in [
                (scan(m, f), "shared_observable_scan_operator"),
                (last(m, f), "shared_observable_last_operator"),
                (callback(m, f), "shared_callback_subscription_adapter"),
                (two_stage(m, f), "shared_two_stage_pipe_operator"),
            ] {
                if let Some(roles) = proof {
                    if let Some((_, name)) = roles.iter().find(|(span, _)| *span == site.span) {
                        let explanation = if evaluator == "shared_two_stage_pipe_operator" {
                            "complete own two-formal capture and returned one-source source.pipe(first(first,second),second(void 0,second)) protocol; exact receiver, ordered arguments, effects and return; helper callable origins and alias reads are scoped and initialized at every exposure; native helper purposes remain explicitly opaque"
                        } else {
                            "complete own ordered factory/operator/executor protocol; scoped formals, state producer-consumer SymbolIds, every writer, error/catch identity, return and cleanup; independently proved complete native constructor and executed initialization at every exposure; external callable purposes remain opaque"
                        };
                        return vec![proposal(name, evaluator, explanation, facts)];
                    }
                }
            }
            for roles in [scan_with_origin(m, f, true), last_with_origin(m, f, true), callback_with_origin(m, f, true)].into_iter().flatten() {
                if let Some((_, name)) = roles.iter().find(|(span, _)| *span == site.span) {
                    return vec![proposal(name, "shared_subscriber_stream_state_operator", "Complete own factory/operator/executor body; scoped formals and state producer-consumer identities, exact every writer, error/catch, ordered return and cleanup; independently complete ordinary strict subscriber constructor API with closed object reads and executed initialization at every exposure; no native class identity claimed", facts)];
                }
            }
        }
        owner = m.enclosing_function(candidate.node_id);
    }
    Vec::new()
}

#[cfg(test)]
mod scan_last_capture_order {
    use super::*;
    const ACTUAL: &str =
        include_str!("../../../../tests/fixtures/scan_last_capture_order/captured-order-parent.js");
    const LAST: &str = "$THREAD_6_29181";
    const SCAN: &str = "$THREAD_2_82358";

    fn inspect(source: &str, name: &str, check: impl FnOnce(&Model<'_, '_>, &Function<'_>)) {
        let arena = Allocator::default();
        let program = crate::analysis::parse_program(&arena, source).unwrap();
        let (scoping, nodes) = crate::analysis::analyze(&program);
        let model = Model::new(
            source,
            &program,
            &scoping,
            &nodes,
            ProgramPropertiesLimits::default(),
        )
        .unwrap();
        let site = model
            .all_sites()
            .iter()
            .find(|s| {
                s.binding_symbol
                    .is_some_and(|s| scoping.symbol_name(s) == name)
            })
            .unwrap();
        let AstKind::Function(f) = nodes.kind(site.node_id) else {
            panic!("function")
        };
        check(&model, f);
    }
    fn proved(source: &str, name: &str, expected: bool) {
        inspect(source, name, |m, f| {
            assert_eq!(
                if name == LAST {
                    last(m, f).is_some()
                } else {
                    scan(m, f).is_some()
                },
                expected,
                "{name}"
            )
        });
    }
    fn own(source: &str, name: &str, from: &str, to: &str) -> String {
        let mut result = None;
        inspect(source, name, |_, f| {
            let text = &source[f.span.start as usize..f.span.end as usize];
            assert_eq!(text.matches(from).count(), 1, "nonvacuous mutation: {from}");
            result = Some(format!(
                "{}{}{}",
                &source[..f.span.start as usize],
                text.replacen(from, to, 1),
                &source[f.span.end as usize..]
            ));
        });
        result.unwrap()
    }
    #[test]
    fn first_guard_and_all_actual_parent_obligations() {
        for name in [LAST, SCAN] {
            inspect(ACTUAL, name, |m, f| {
                let p = params(m, f, 2).unwrap();
                let ss = body(f).unwrap();
                assert!(alias(m, &ss[0], p[0]).is_none(), "legacy first guard");
                assert_eq!(
                    id(m, variable(&ss[0]).unwrap().init.as_ref().unwrap()),
                    Some(p[1])
                );
                let (first, second) = formal_copies(m, &ss[0], &ss[1], &p).unwrap();
                alias(m, first, p[0]).unwrap();
                assert_eq!(
                    id(m, variable(second).unwrap().init.as_ref().unwrap()),
                    Some(p[1])
                );
                let inner = function(&ss[ss.len() - 2]).unwrap();
                let (input, executor) = operator(m, inner, ss.last().unwrap())
                    .expect("actual native origin, operator, source and executor");
                let sub = subscription(m, executor, input)
                    .expect("actual subscription state and teardown");
                let sent = if name == LAST {
                    id(m, sub.initial).unwrap()
                } else {
                    let Statement::IfStatement(default) = &ss[2] else {
                        panic!("default")
                    };
                    id(
                        m,
                        store(
                            m,
                            &block(&default.consequent).unwrap()[0],
                            bind(&variable(second).unwrap().id).unwrap(),
                        )
                        .unwrap(),
                    )
                    .unwrap()
                };
                sentinel(m, sent, fs(f).unwrap()).expect("actual sentinel origin and exposure");
                let roles = if name == LAST { last(m, f) } else { scan(m, f) }
                    .expect("complete own operational parent");
                assert_eq!(roles.len(), 2);
                println!(
                    "{}",
                    serde_json::json!({"factory":name,"start":f.span.start,"first_missing_guard":"formal capture order","native_executor":true,"sentinel_readiness":true,"full_parent":true,"roles":roles.iter().map(|(_,n)|*n).collect::<Vec<_>>()})
                );
            });
        }
        let result = crate::naming::name_threads(ACTUAL, true).unwrap();
        for (name, role, eval) in [
            (
                LAST,
                "createLastOperator",
                "shared_observable_last_operator",
            ),
            (
                "$THREAD_6_45986",
                "lastOperatorFunction",
                "shared_observable_last_operator",
            ),
            (
                SCAN,
                "createScanOperator",
                "shared_observable_scan_operator",
            ),
            (
                "$THREAD_6_121673",
                "scanOperatorFunction",
                "shared_observable_scan_operator",
            ),
        ] {
            let o = result
                .report
                .outcomes
                .iter()
                .find(|o| o.binding.name == name)
                .unwrap();
            assert!(o.applied, "{o:?}");
            assert_eq!(o.role_name.as_deref(), Some(role));
            assert_eq!(o.evaluator.as_deref(), Some(eval));
        }
    }
    #[test]
    fn alpha_and_both_semantic_capture_orders_and_legacy_controls() {
        let alpha = ACTUAL
            .replace("$KEY_292", "nativeConstructor")
            .replace("$KEY_212", "lastSentinel")
            .replace("$KEY_98", "scanSentinel");
        for name in [LAST, SCAN] {
            proved(&alpha, name, true);
        }
        let init = "$KEY_292 = $THREAD_6_39623();";
        let aliased = ACTUAL.replacen(init, "let nativeProducer = $THREAD_6_39623(); $KEY_292 = nativeProducer; const lateNative = $KEY_292;", 1);
        for name in [LAST, SCAN] {
            proved(&aliased, name, true);
        }
        let original_order = ACTUAL.replace(
            "let $KEY_0 = $AM;\n\t\t\t\t\tlet $KEY_1 = $AL;",
            "let $KEY_1 = $AL;\n\t\t\t\t\tlet $KEY_0 = $AM;",
        );
        assert_ne!(original_order, ACTUAL);
        for name in [LAST, SCAN] {
            proved(&original_order, name, true);
        }
        for source in [
            include_str!("../../../../tests/fixtures/naming/subscription_operators/subscription-parent-alternate.js"),
            include_str!("../../../../tests/fixtures/naming/subscription_operators/subscription-parent-baseline.js"),
        ] {
            let result = crate::naming::name_threads(source, true).unwrap();
            for eval in [
                "shared_observable_scan_operator",
                "shared_observable_last_operator",
            ] {
                assert_eq!(
                    result
                        .report
                        .outcomes
                        .iter()
                        .filter(|o| o.applied && o.evaluator.as_deref() == Some(eval))
                        .count(),
                    2
                );
            }
        }
    }
    #[test]
    fn scan_operational_mutations_refuse() {
        for (from, to) in [
            ("let $KEY_0 = $AM;", "let $KEY_0 = $AL;"),
            ("let $KEY_1 = $AL;", "let $KEY_1 = $KEY_0;"),
            ("$AM === void 0", "$AL === void 0"),
            ("$KEY_0 = $KEY_98;", "$KEY_0 = {};"),
            ("$KEY_4 === $KEY_98", "$KEY_4 !== $KEY_98"),
            ("$KEY_1($KEY_4, $AP)", "$KEY_1($AP, $KEY_4)"),
            ("$KEY_4 = $AP;", "$KEY_4 = $AP; $KEY_5.next($AP);"),
            ("$KEY_5.error(e);", "$KEY_5.error($AP);"),
            ("$AP => $KEY_5.error($AP)", "$AP => $KEY_5.next($AP)"),
            ("() => $KEY_5.complete()", "() => $KEY_5.error()"),
            (
                "return $KEY_3.unsubscribe();",
                "return other.unsubscribe();",
            ),
        ] {
            proved(&own(ACTUAL, SCAN, from, to), SCAN, false);
        }
    }
    #[test]
    fn last_operational_mutations_refuse() {
        for (from, to) in [
            ("let $KEY_0 = $AM;", "let $KEY_0 = $AL;"),
            ("let $KEY_1 = $AL;", "let $KEY_1 = $KEY_0;"),
            ("$KEY_1($AP)", "$KEY_1($KEY_0)"),
            (
                "typeof $KEY_1 !== \"function\"",
                "typeof $KEY_1 === \"function\"",
            ),
            ("$KEY_5.error(e);", "$KEY_5.error($AP);"),
            ("No elements in sequence", "Different failure"),
            ("$KEY_0 === void 0", "$KEY_1 === void 0"),
            ("$KEY_5.next($KEY_0);", "$KEY_5.next($KEY_3);"),
            (
                "$KEY_5.next($KEY_3);",
                "$KEY_5.complete(); $KEY_5.next($KEY_3);",
            ),
            (
                "return $KEY_4.unsubscribe();",
                "return other.unsubscribe();",
            ),
        ] {
            proved(&own(ACTUAL, LAST, from, to), LAST, false);
        }
    }
    #[test]
    fn sentinel_constructor_and_exposure_mutations_refuse() {
        for (name, sentinel_name) in [(LAST, "$KEY_212"), (SCAN, "$KEY_98")] {
            let original = format!("{sentinel_name} = {{}};");
            assert_eq!(ACTUAL.matches(&original).count(), 1);
            for replacement in [
                format!("{sentinel_name} = other;"),
                format!("{original} {sentinel_name} = other;"),
                format!("if (ready) {{ {original} }}"),
                format!("{name}(fn)({{}}); {original}"),
                format!("publish({name}); {original}"),
                format!("let earlyOperator = {name}; earlyOperator(fn)({{}}); {original}"),
            ] {
                proved(&ACTUAL.replacen(&original, &replacement, 1), name, false);
            }
        }
        let init = "$KEY_292 = $THREAD_6_39623();";
        assert_eq!(ACTUAL.matches(init).count(), 1);
        for replacement in [
            format!("{SCAN}(fn)({{}}); {LAST}(fn)({{}}); {init}"),
            format!("{init} $KEY_292 = other;"),
            format!("if (ready) {{ {init} }}"),
            format!("let earlyNative = $KEY_292; {init}"),
            format!("{init} $KEY_292.prototype.subscribe = other;"),
        ] {
            eprintln!("constructor refusal case: {replacement}");
            let source = ACTUAL.replacen(init, &replacement, 1);
            // A raw constructor mutation may be excluded by a constructor-read
            // contract added locally; every whole parent must still refuse.
            for name in [LAST, SCAN] {
                proved(&source, name, false);
            }
        }
    }
}

#[cfg(test)]
mod scan_last;
