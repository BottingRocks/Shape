//! Guarded script-digest dispatch; proves both promise and immediate fallback
//! paths, including exact replacement receiver and optional-result semantics.
use crate::naming::support::media_entropy::*;
use super::*;
use crate::naming::shared::one_variable;
#[derive(Clone, PartialEq, Eq)]
enum E {
    S(Symbol),
    G(String),
    T(String),
    N(u64),
    Null,
    F(Box<E>, String, bool),
    U(String, Box<E>),
    B(String, Box<E>, Box<E>),
    C(Box<E>, Vec<E>),
    Array,
}
fn s(v: Symbol) -> E {
    E::S(v)
}
fn g(v: &str) -> E {
    E::G(v.into())
}
fn text(v: &str) -> E {
    E::T(v.into())
}
fn n(v: f64) -> E {
    E::N(v.to_bits())
}
fn f(v: E, k: &str) -> E {
    E::F(Box::new(v), k.into(), false)
}
fn opt(v: E, k: &str) -> E {
    E::F(Box::new(v), k.into(), true)
}
fn b(k: &str, a: E, z: E) -> E {
    E::B(k.into(), Box::new(a), Box::new(z))
}
fn u(k: &str, v: E) -> E {
    E::U(k.into(), Box::new(v))
}
fn c(v: E, a: Vec<E>) -> E {
    E::C(Box::new(v), a)
}
fn expr(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<E> {
    fn walk(model: &Model<'_, '_>, e: &Expression<'_>, depth: usize) -> Option<E> {
        if depth > 40 {
            return None;
        }
        let child = |e: &Expression<'_>| walk(model, e, depth + 1);
        Some(match e.without_parentheses() {
            Expression::Identifier(id) => symbol(model, e)
                .map(s)
                .unwrap_or_else(|| g(id.name.as_str())),
            Expression::StringLiteral(v) => text(v.value.as_str()),
            Expression::NumericLiteral(v) => n(v.value),
            Expression::NullLiteral(_) => E::Null,
            Expression::ChainExpression(chain) => match &chain.expression {
                ChainElement::StaticMemberExpression(m) if m.optional => E::F(
                    Box::new(child(&m.object)?),
                    m.property.name.to_string(),
                    true,
                ),
                _ => return None,
            },
            Expression::StaticMemberExpression(m) => E::F(
                Box::new(child(&m.object)?),
                m.property.name.to_string(),
                m.optional,
            ),
            Expression::BinaryExpression(v) => {
                b(v.operator.as_str(), child(&v.left)?, child(&v.right)?)
            }
            Expression::LogicalExpression(v) => {
                b(v.operator.as_str(), child(&v.left)?, child(&v.right)?)
            }
            Expression::UnaryExpression(v) => u(v.operator.as_str(), child(&v.argument)?),
            Expression::CallExpression(v) if !v.optional => {
                let a = call_arguments(v);
                if a.len() != v.arguments.len() {
                    return None;
                }
                c(
                    child(&v.callee)?,
                    a.into_iter().map(child).collect::<Option<_>>()?,
                )
            }
            Expression::ArrayExpression(v) if v.elements.is_empty() => E::Array,
            _ => return None,
        })
    }
    walk(model, e, 0)
}
fn eq(model: &Model<'_, '_>, e: &Expression<'_>, v: E) -> bool {
    expr(model, e) == Some(v)
}
fn write(model: &Model<'_, '_>, st: &Statement<'_>, id: Symbol, v: E) -> Option<()> {
    let a = assignment(st)?;
    (assigned_symbol(model, &a.left) == Some(id) && eq(model, &a.right, v)).then_some(())
}
fn invoke(model: &Model<'_, '_>, st: &Statement<'_>, want: E) -> Option<()> {
    eq(model, expression_statement(st)?, want).then_some(())
}
fn fallback(
    model: &Model<'_, '_>,
    site: FunctionSite,
    st: &Statement<'_>,
) -> Option<(Symbol, Symbol)> {
    let [st] = block(st)? else {
        return None;
    };
    let call = call(expression_statement(st)?)?;
    let args = call_arguments(call);
    if args.len() != 2 || args.len() != call.arguments.len() || !empty_array(args[1]) {
        return None;
    }
    Some((
        capture(model, site, &call.callee)?,
        capture(model, site, args[0])?,
    ))
}
fn callback(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
    url: Symbol,
    dispatch: Symbol,
) -> Option<Symbol> {
    let Expression::ArrowFunctionExpression(a) = e.without_parentheses() else {
        return None;
    };
    if a.r#async || a.expression || !a.body.directives.is_empty() {
        return None;
    }
    let p = plain_formals(model, &a.params)?;
    let [result] = p.as_slice() else {
        return None;
    };
    let [Statement::VariableDeclaration(locals), Statement::IfStatement(branch), replace, rest @ ..] =
        a.body.statements.as_slice()
    else {
        return None;
    };
    if locals.declarations.len() != 4 {
        return None;
    }
    let ids = locals
        .declarations
        .iter()
        .map(|v| {
            if v.init.is_some() {
                return None;
            }
            plain_binding(model, v, Some(2))
        })
        .collect::<Option<Vec<_>>>()?;
    let (receiver, method, pattern, replacement) = (ids[0], ids[1], ids[2], ids[3]);
    let distinct: BTreeSet<_> = ids.iter().copied().collect();
    if distinct.len() != 4 {
        return None;
    }
    if !eq(
        model,
        &branch.test,
        b("==", opt(s(*result), "hash"), E::Null),
    ) {
        return None;
    }
    let first = block(&branch.consequent)?;
    let second = block(branch.alternate.as_ref()?)?;
    let [method_st, receiver_st, pattern_st, replacement_st] = first else {
        return None;
    };
    write(model, method_st, method, f(s(url), "replace"))?;
    write(model, receiver_st, receiver, s(url))?;
    let regexp = c(g("RegExp"), vec![text("&c=.+")]);
    write(model, pattern_st, pattern, regexp.clone())?;
    let fallback = capture(model, site, &assignment(replacement_st)?.right)?;
    write(model, replacement_st, replacement, s(fallback))?;
    let [receiver_st, method_st, pattern_st, replacement_st] = second else {
        return None;
    };
    write(model, receiver_st, receiver, s(url))?;
    write(model, method_st, method, f(s(receiver), "replace"))?;
    write(model, pattern_st, pattern, regexp)?;
    write(model, replacement_st, replacement, opt(s(*result), "hash"))?;
    write(
        model,
        replace,
        url,
        c(
            f(s(method), "call"),
            vec![s(receiver), s(pattern), b("+", text("&c="), s(replacement))],
        ),
    )?;
    match rest {
        [indices, emit] => {
            let v = one_variable(indices)?;
            let indices = plain_binding(model, v, Some(0))?;
            if !eq(model, v.init.as_ref()?, b("||", opt(s(*result), "sidx"), E::Array)) { return None; }
            invoke(model, emit, c(s(dispatch), vec![s(url), s(indices)]))?;
        }
        [emit] => {
            invoke(model, emit, c(s(dispatch), vec![s(url), b("||", opt(s(*result), "sidx"), E::Array)]))?;
        }
        _ => return None,
    }
    Some(fallback)
}
fn prove(model: &Model<'_, '_>, site: FunctionSite) -> Option<()> {
    let AstKind::Function(fun) = model.nodes.kind(site.node_id) else {
        return None;
    };
    let body = fun.body.as_ref()?;
    if fun.r#async
        || fun.generator
        || !body.directives.is_empty()
        || !plain_params(model, fun)?.is_empty()
    {
        return None;
    }
    let [Statement::IfStatement(guard)] = body.statements.as_slice() else {
        return None;
    };
    let (dispatch, url) = fallback(model, site, guard.alternate.as_ref()?)?;
    let [compute, Statement::IfStatement(present)] = block(&guard.consequent)? else {
        return None;
    };
    if fallback(model, site, present.alternate.as_ref()?)? != (dispatch, url) {
        return None;
    }
    let v = one_variable(compute)?;
    let promise = plain_binding(model, v, Some(0))?;
    let computation = call(v.init.as_ref()?)?;
    let args = call_arguments(computation);
    if args.len() != 2
        || args.len() != computation.arguments.len()
        || crate::naming::resolve::ordered_callable(model, &computation.callee, 0).is_none()
    {
        return None;
    }
    let input = capture(model, site, args[0])?;
    let context = capture(model, site, member(args[1], "scrBytes")?)?;
    let crypto = f(f(s(context), "global"), "crypto");
    let subtle = f(crypto.clone(), "subtle");
    let condition = b(
        "&&",
        b(
            "&&",
            b(
                "&&",
                b("!=", crypto, E::Null),
                b("===", u("typeof", subtle.clone()), text("object")),
            ),
            b("===", u("typeof", f(subtle, "digest")), text("function")),
        ),
        b(
            ">",
            f(f(f(s(context), "scrBytes"), "sidx"), "length"),
            n(0.0),
        ),
    );
    if !eq(model, &guard.test, condition) || !eq(model, &present.test, b("!=", s(promise), E::Null))
    {
        return None;
    }
    let [st] = block(&present.consequent)? else {
        return None;
    };
    let caught = call(expression_statement(st)?)?;
    let ca = call_arguments(caught);
    if ca.len() != 1 || ca.len() != caught.arguments.len() {
        return None;
    }
    let Expression::ArrowFunctionExpression(empty) = ca[0].without_parentheses() else {
        return None;
    };
    if empty.r#async
        || empty.expression
        || !empty.params.items.is_empty()
        || empty.params.rest.is_some()
        || !empty.body.statements.is_empty()
        || !empty.body.directives.is_empty()
    {
        return None;
    }
    let then = call(member(&caught.callee, "catch")?)?;
    let ta = call_arguments(then);
    if ta.len() != 1
        || ta.len() != then.arguments.len()
        || !member(&then.callee, "then").is_some_and(|e| symbol(model, e) == Some(promise))
    {
        return None;
    }
    let hash = callback(model, site, ta[0], url, dispatch)?;
    let ids: BTreeSet<_> = [dispatch, url, input, context, hash].into_iter().collect();
    (ids.len() == 5).then_some(())
}
pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    if prove(model, site).is_some() {
        vec![proposal("dispatchScriptDigestUrl","network_script_digest_dispatch","whole guarded script-digest dispatch: require crypto/subtle/digest and nonempty script indices; compute once; on result promise replace &c= suffix through exact saved receiver with optional hash or captured fallback and dispatch optional indices or empty array; both unavailable paths dispatch unchanged URL and empty indices; empty rejection handler; all scoped captures and effect order proved",f)]
    } else {
        Vec::new()
    }
}
