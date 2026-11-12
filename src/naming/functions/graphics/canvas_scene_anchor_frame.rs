//! Closed scene drawing, anchor-property queue and sampled-pixel frame protocol.
//! Grammar nodes preserve every literal, operator and effect in the source;
//! geometry is typed rather than enumerated as a per-input scene catalogue.
use crate::naming::support::interaction::*;
use super::*;
use oxc_semantic::SymbolId;
type Sym = SymbolId;
enum E {
    S(Sym),
    N(f64),
    T(&'static str),
    This,
    Null,
    G(&'static str),
    M(Box<E>, &'static str),
    I(Box<E>, Box<E>),
    B(Box<E>, &'static str, Box<E>),
    U(&'static str, Box<E>),
    C(Box<E>, Vec<E>),
    A(Vec<E>),
    L(Box<E>, &'static str, Box<E>),
}
fn s(x: Sym) -> E {
    E::S(x)
}
fn n(x: f64) -> E {
    E::N(x)
}
fn t(x: &'static str) -> E {
    E::T(x)
}
fn m(x: E, k: &'static str) -> E {
    E::M(Box::new(x), k)
}
fn i(x: E, y: E) -> E {
    E::I(Box::new(x), Box::new(y))
}
fn b(x: E, k: &'static str, y: E) -> E {
    E::B(Box::new(x), k, Box::new(y))
}
fn u(k: &'static str, x: E) -> E {
    E::U(k, Box::new(x))
}
fn c(x: E, a: Vec<E>) -> E {
    E::C(Box::new(x), a)
}
fn a(xs: Vec<E>) -> E {
    E::A(xs)
}
fn eq(model: &Model<'_, '_>, e: &Expression<'_>, x: &E) -> bool {
    let e = e.without_parentheses();
    match (x, e) {
        (E::S(x), _) => symbol(model, e) == Some(*x),
        (E::N(x), Expression::NumericLiteral(y)) => *x == y.value,
        (E::N(x), Expression::UnaryExpression(y)) if *x < 0.0 => {
            y.operator.as_str() == "-" && eq(model, &y.argument, &n(-x))
        }
        (E::T(x), Expression::StringLiteral(y)) => *x == y.value,
        (E::This, Expression::ThisExpression(_)) | (E::Null, Expression::NullLiteral(_)) => true,
        (E::G(x), Expression::Identifier(y)) => y.name == *x && symbol(model, e).is_none(),
        (E::M(x, k), Expression::StaticMemberExpression(y)) => {
            !y.optional && y.property.name == *k && eq(model, &y.object, x)
        }
        (E::I(x, z), Expression::ComputedMemberExpression(y)) => {
            !y.optional && eq(model, &y.object, x) && eq(model, &y.expression, z)
        }
        (E::B(x, k, z), Expression::BinaryExpression(y)) => {
            y.operator.as_str() == *k && eq(model, &y.left, x) && eq(model, &y.right, z)
        }
        (E::L(x, k, z), Expression::LogicalExpression(y)) => {
            y.operator.as_str() == *k && eq(model, &y.left, x) && eq(model, &y.right, z)
        }
        (E::U(k, x), Expression::UnaryExpression(y)) => {
            y.operator.as_str() == *k && eq(model, &y.argument, x)
        }
        (E::C(x, z), Expression::CallExpression(y)) => {
            !y.optional
                && eq(model, &y.callee, x)
                && y.arguments.len() == z.len()
                && y.arguments
                    .iter()
                    .zip(z)
                    .all(|(y, x)| y.as_expression().is_some_and(|y| eq(model, y, x)))
        }
        (E::A(x), Expression::ArrayExpression(y)) => {
            y.elements.len() == x.len()
                && y.elements
                    .iter()
                    .zip(x)
                    .all(|(y, x)| y.as_expression().is_some_and(|y| eq(model, y, x)))
        }
        _ => false,
    }
}
fn ex(model: &Model<'_, '_>, st: &Statement<'_>, x: E) -> bool {
    expression_statement(st).is_some_and(|v| eq(model, v, &x))
}
fn local<'a>(
    model: &Model<'_, '_>,
    st: &'a Statement<'a>,
) -> Option<(Sym, Option<&'a Expression<'a>>)> {
    let Statement::VariableDeclaration(d) = st else {
        return None;
    };
    if !matches!(
        d.kind,
        VariableDeclarationKind::Var
            | VariableDeclarationKind::Let
            | VariableDeclarationKind::Const
    ) {
        return None;
    }
    let [v] = d.declarations.as_slice() else {
        return None;
    };
    Some((plain_var(model, v)?, v.init.as_ref()))
}
fn init<'a>(model: &Model<'_, '_>, st: &'a Statement<'a>) -> Option<(Sym, &'a Expression<'a>)> {
    let (id, v) = local(model, st)?;
    Some((id, v?))
}
fn block<'a>(st: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = st else {
        return None;
    };
    Some(&b.body)
}
fn assign<'a>(model: &Model<'_, '_>, st: &'a Statement<'a>, to: Sym) -> Option<&'a Expression<'a>> {
    let x = assignment(expression_statement(st)?)?;
    (target_symbol(model, &x.left) == Some(to)).then_some(&x.right)
}
fn set(model: &Model<'_, '_>, st: &Statement<'_>, to: Sym, v: E) -> bool {
    assign(model, st, to).is_some_and(|x| eq(model, x, &v))
}
fn prop<'a>(
    model: &Model<'_, '_>,
    st: &'a Statement<'a>,
    to: Sym,
    key: &str,
) -> Option<&'a Expression<'a>> {
    let x = assignment(expression_statement(st)?)?;
    let AssignmentTarget::StaticMemberExpression(p) = &x.left else {
        return None;
    };
    (!p.optional && p.property.name == key && symbol(model, &p.object) == Some(to))
        .then_some(&x.right)
}
fn fixed(model: &Model<'_, '_>, id: Sym, w: usize) -> bool {
    model.binding_writes(id) == w
        && model.scoping.symbol_redeclarations(id).is_empty()
        && (w == 0
            || matches!(model.nodes.parent_kind(model.scoping.symbol_declaration(id)), AstKind::VariableDeclaration(d) if matches!(d.kind,VariableDeclarationKind::Let|VariableDeclarationKind::Var)))
}
fn distinct(xs: &[Sym]) -> bool {
    xs.iter().copied().collect::<BTreeSet<_>>().len() == xs.len()
}
fn cap(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<Sym> {
    let id = symbol(model, e)?;
    if site.span.contains_inclusive(model.scoping.symbol_span(id))
        || !model.scoping.symbol_redeclarations(id).is_empty()
    {
        return None;
    }
    match model.nodes.kind(model.scoping.symbol_declaration(id)) {
        AstKind::VariableDeclarator(_) => {
            captured_symbol(model, site, id)?;
            (model.scoping.symbol_span(id).end < site.span.start).then_some(id)
        }
        AstKind::Function(_) => fixed(model, id, 0).then_some(id),
        _ => None,
    }
}
fn arrow<'a>(
    model: &Model<'_, '_>,
    e: &'a Expression<'a>,
    arity: usize,
) -> Option<(&'a ArrowFunctionExpression<'a>, Vec<Sym>)> {
    let Expression::ArrowFunctionExpression(f) = e.without_parentheses() else {
        return None;
    };
    if f.r#async
        || f.expression
        || f.params.rest.is_some()
        || f.params.items.len() != arity
        || !f.body.directives.is_empty()
    {
        return None;
    }
    let params = f
        .params
        .items
        .iter()
        .map(|p| plain_param(model, p))
        .collect::<Option<Vec<_>>>()?;
    Some((f, params))
}
fn step(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    id: Sym,
    op: &str,
    prefix: Option<bool>,
) -> bool {
    let Expression::UpdateExpression(x) = e else {
        return false;
    };
    matches!(&x.argument,SimpleAssignmentTarget::AssignmentTargetIdentifier(b) if crate::analysis::identifier_symbol(b,model.scoping)==Some(id))
        && x.operator.as_str() == op
        && prefix.is_none_or(|p| x.prefix == p)
}
fn void_return(model: &Model<'_, '_>, st: &Statement<'_>) -> bool {
    matches!(st,Statement::ReturnStatement(r) if r.argument.as_ref().is_some_and(|v|eq(model,v,&u("void",n(0.0)))))
}
fn integer(e: &Expression<'_>) -> Option<f64> {
    let Expression::NumericLiteral(v) = e else {
        return None;
    };
    (v.value.is_finite() && v.value.fract() == 0.0).then_some(v.value)
}
fn geometry(model: &Model<'_, '_>, e: &Expression<'_>, depth: usize) -> bool {
    if depth > 16 {
        return false;
    }
    match e.without_parentheses() {
        Expression::NumericLiteral(v) => v.value.is_finite(),
        Expression::UnaryExpression(v) => {
            v.operator.as_str() == "-" && geometry(model, &v.argument, depth + 1)
        }
        Expression::BinaryExpression(v) => {
            matches!(v.operator.as_str(), "+" | "*")
                && geometry(model, &v.left, depth + 1)
                && geometry(model, &v.right, depth + 1)
        }
        Expression::CallExpression(v) => {
            !v.optional
                && v.arguments.len() == 1
                && (eq(model, &v.callee, &m(E::G("Math"), "cos"))
                    || eq(model, &v.callee, &m(E::G("Math"), "sin")))
                && v.arguments[0]
                    .as_expression()
                    .is_some_and(|v| geometry(model, v, depth + 1))
        }
        _ => false,
    }
}
fn header(model: &Model<'_, '_>, e: &Expression<'_>, seed: Sym, coerced: bool) -> bool {
    let word = || {
        if coerced {
            s(seed)
        } else {
            b(s(seed), "|", n(0.0))
        }
    };
    eq(
        model,
        e,
        &a(vec![
            b(word(), ">>>", n(24.0)),
            b(b(word(), ">>", n(16.0)), "&", n(255.0)),
            b(b(word(), ">>", n(8.0)), "&", n(255.0)),
            b(word(), "&", n(255.0)),
        ]),
    )
}
fn random(model: &Model<'_, '_>, e: &Expression<'_>, coerced: bool) -> bool {
    let raw = || b(c(m(E::G("Math"), "random"), vec![]), "*", n(4294967296.0));
    eq(
        model,
        e,
        &if coerced {
            b(raw(), "|", n(0.0))
        } else {
            raw()
        },
    )
}
fn u32_salt(e: &Expression<'_>) -> Option<f64> {
    let v = integer(e)?;
    (v >= 0.0 && v <= u32::MAX as f64).then_some(v)
}
struct Ports {
    writer: Sym,
    factory: Sym,
    anchor_observer: Sym,
    scheduler: Sym,
    digest: Sym,
}
fn frame_callback(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
    p: &Ports,
) -> Option<()> {
    let (f, args) = arrow(model, e, 1)?;
    let input = args[0];
    let [nonce, bytes, write, next, complete] = f.body.statements.as_slice() else {
        return None;
    };
    let (seed, value) = init(model, nonce)?;
    let (output, lanes) = init(model, bytes)?;
    if !fixed(model, seed, 0)
        || !fixed(model, output, 0)
        || !random(model, value, true)
        || !header(model, lanes, seed, true)
    {
        return None;
    }
    let write_call = call(expression_statement(write)?)?;
    let values = arguments(write_call)?;
    if values.len() != 4
        || symbol(model, &write_call.callee) != Some(p.writer)
        || symbol(model, values[1]) != Some(input)
        || symbol(model, values[2]) != Some(output)
        || !number(values[3], 3.0)
    {
        return None;
    }
    let stream = call(values[0])?;
    let values = arguments(stream)?;
    if symbol(model, &stream.callee) != Some(p.factory) || values.len() != 1 {
        return None;
    }
    let (seed_arg, salt) = binary(values[0], "^")?;
    if symbol(model, seed_arg) != Some(seed) {
        return None;
    }
    u32_salt(salt)?;
    if !ex(
        model,
        next,
        c(
            m(s(p.anchor_observer), "next"),
            vec![a(vec![a(vec![]), s(output)])],
        ),
    ) || !ex(
        model,
        complete,
        c(m(s(p.anchor_observer), "complete"), vec![]),
    ) {
        return None;
    }
    distinct(&[input, seed, output, p.writer, p.factory, p.anchor_observer]).then_some(())
}

fn accumulate_properties(model: &Model<'_, '_>, e: &Expression<'_>, text: Sym) -> Option<()> {
    let (f, ps) = arrow(model, e, 1)?;
    let value = ps[0];
    let [Statement::TryStatement(attempt)] = f.body.statements.as_slice() else {
        return None;
    };
    if attempt.finalizer.is_some() {
        return None;
    }
    let [success] = attempt.block.body.as_slice() else {
        return None;
    };
    if !set(
        model,
        success,
        text,
        b(
            s(text),
            "+",
            b(t(" "), "+", c(m(s(value), "toString"), vec![])),
        ),
    ) {
        return None;
    }
    let handler = attempt.handler.as_ref()?;
    let err = plain_binding(model, &handler.param.as_ref()?.pattern)?;
    let [failed] = handler.body.body.as_slice() else {
        return None;
    };
    if !fixed(model, err, 0)
        || !set(
            model,
            failed,
            text,
            b(s(text), "+", b(b(t(" {"), "+", s(err)), "+", t("}"))),
        )
    {
        return None;
    }
    distinct(&[text, value, err]).then_some(())
}
fn anchor_probe(
    model: &Model<'_, '_>,
    site: FunctionSite,
    decl: &Statement<'_>,
    branch: &Statement<'_>,
    invoke: &Statement<'_>,
    anchor: Sym,
    writer: Sym,
    factory: Sym,
) -> Option<Ports> {
    let Statement::VariableDeclaration(d) = decl else {
        return None;
    };
    let [receiver, key] = d.declarations.as_slice() else {
        return None;
    };
    let receiver_id = plain_var(model, receiver)?;
    let key_id = plain_var(model, key)?;
    if receiver.init.is_some()
        || key.init.is_some()
        || !fixed(model, receiver_id, 2)
        || !fixed(model, key_id, 2)
    {
        return None;
    }
    let Statement::IfStatement(test) = branch else {
        return None;
    };
    if !eq(model, &test.test, &b(s(anchor), "==", E::Null)) {
        return None;
    }
    let [receiver, key] = block(&test.consequent)? else {
        return None;
    };
    let observer = cap(model, site, assign(model, receiver, receiver_id)?)?;
    if !set(model, key, key_id, t("complete")) {
        return None;
    }
    let [properties, guard, text, queue, on_empty, start] = block(test.alternate.as_ref()?)? else {
        return None;
    };
    let (properties, empty) = init(model, properties)?;
    let (text, text_value) = init(model, text)?;
    if !fixed(model, properties, 0)
        || !eq(model, empty, &a(vec![]))
        || !eq(model, text_value, &t(""))
        || !fixed(model, text, 2)
    {
        return None;
    }
    let Statement::IfStatement(guard) = guard else {
        return None;
    };
    if guard.alternate.is_some()
        || !eq(
            model,
            &guard.test,
            &E::L(
                Box::new(b(s(anchor), "!==", E::Null)),
                "&&",
                Box::new(b(s(anchor), "!==", u("void", n(0.0)))),
            ),
        )
    {
        return None;
    }
    let [Statement::ForInStatement(enumerate)] = block(&guard.consequent)? else {
        return None;
    };
    let ForStatementLeft::VariableDeclaration(d) = &enumerate.left else {
        return None;
    };
    let [v] = d.declarations.as_slice() else {
        return None;
    };
    let name = plain_var(model, v)?;
    if d.kind != VariableDeclarationKind::Const
        || v.init.is_some()
        || !fixed(model, name, 0)
        || !eq(model, &enumerate.right, &s(anchor))
    {
        return None;
    }
    let [push] = block(&enumerate.body)? else {
        return None;
    };
    if !ex(model, push, c(m(s(properties), "push"), vec![s(name)])) {
        return None;
    }
    let queue = call(assign(model, queue, receiver_id)?)?;
    let values = arguments(queue)?;
    let scheduler = cap(model, site, &queue.callee)?;
    if values.len() != 3 || !number(values[1], 45.0) || !eq(model, values[2], &s(properties)) {
        return None;
    }
    accumulate_properties(model, values[0], text)?;
    let on_empty = call(expression_statement(on_empty)?)?;
    let values = arguments(on_empty)?;
    if !eq(model, &on_empty.callee, &m(s(receiver_id), "onEmpty")) || values.len() != 1 {
        return None;
    }
    let (f, _) = arrow(model, values[0], 0)?;
    let [digest] = f.body.statements.as_slice() else {
        return None;
    };
    let digest = call(expression_statement(digest)?)?;
    let values = arguments(digest)?;
    let digest_port = cap(model, site, &digest.callee)?;
    if values.len() != 4 || !eq(model, values[0], &s(text)) || !number(values[1], 45.0) {
        return None;
    }
    let p = Ports {
        writer,
        factory,
        anchor_observer: observer,
        scheduler,
        digest: digest_port,
    };
    frame_callback(model, site, values[2], &p)?;
    let (err, args) = arrow(model, values[3], 1)?;
    let [error] = err.body.statements.as_slice() else {
        return None;
    };
    if !ex(model, error, c(m(s(observer), "error"), vec![s(args[0])]))
        || !set(model, start, key_id, t("start"))
        || !ex(model, invoke, c(i(s(receiver_id), s(key_id)), vec![]))
        || !distinct(&[
            anchor,
            writer,
            factory,
            receiver_id,
            key_id,
            properties,
            text,
            name,
            observer,
            scheduler,
            digest_port,
        ])
    {
        return None;
    }
    Some(p)
}
struct Prefix {
    context: Sym,
    anchor: Sym,
    data: Sym,
    width: f64,
    height: f64,
    after: usize,
    draws: usize,
}
fn gradient(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &[Statement<'_>],
    at: usize,
    context: Sym,
    palette: &mut Option<Sym>,
) -> Option<(Sym, usize)> {
    let (id, value) = init(model, &body[at])?;
    let grad = call(value)?;
    let values = arguments(grad)?;
    if !fixed(model, id, 0)
        || !eq(model, &grad.callee, &m(s(context), "createLinearGradient"))
        || values.len() != 4
        || !values.iter().all(|e| geometry(model, e, 0))
    {
        return None;
    }
    let (index, loop_, used) = match body.get(at + 1)? {
        Statement::ForStatement(f) => {
            let Some(ForStatementInit::VariableDeclaration(d)) = &f.init else {
                return None;
            };
            let [v] = d.declarations.as_slice() else {
                return None;
            };
            let index = plain_var(model, v)?;
            if !eq(model, v.init.as_ref()?, &n(0.0)) {
                return None;
            }
            (index, f, 2)
        }
        initial => {
            let (index, value) = init(model, initial)?;
            if !eq(model, value, &n(0.0)) {
                return None;
            }
            let Statement::ForStatement(f) = body.get(at + 2)? else {
                return None;
            };
            if f.init.is_some() {
                return None;
            }
            (index, f, 3)
        }
    };
    if !fixed(model, index, 1)
        || !eq(model, loop_.test.as_ref()?, &b(s(index), "<", n(10.0)))
        || !step(model, loop_.update.as_ref()?, index, "++", Some(false))
    {
        return None;
    }
    let [emit] = block(&loop_.body)? else {
        return None;
    };
    let emit = call(expression_statement(emit)?)?;
    let values = arguments(emit)?;
    if values.len() != 2
        || !eq(model, &emit.callee, &m(s(id), "addColorStop"))
        || !eq(model, values[0], &b(s(index), "/", n(9.0)))
    {
        return None;
    }
    let Expression::ComputedMemberExpression(color) = values[1].without_parentheses() else {
        return None;
    };
    let source = cap(model, site, &color.object)?;
    if color.optional
        || !eq(model, &color.expression, &b(s(index), "%", n(3.0)))
        || palette.is_some_and(|p| p != source)
        || !distinct(&[id, index, context, source])
    {
        return None;
    }
    *palette = Some(source);
    Some((id, used))
}
fn scene_prefix(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &[Statement<'_>],
) -> Option<Prefix> {
    let [canvas, width, height, holder, context, rest @ ..] = body else {
        return None;
    };
    let (canvas, value) = init(model, canvas)?;
    let create = call(value)?;
    let values = arguments(create)?;
    cap(model, site, member(&create.callee, "createElement")?)?;
    if values.len() != 1 || !eq(model, values[0], &t("canvas")) || !fixed(model, canvas, 1) {
        return None;
    }
    let width = integer(prop(model, width, canvas, "width")?)?;
    let height = integer(prop(model, height, canvas, "height")?)?;
    if width <= 0.0
        || height <= 0.0
        || width > 65535.0
        || height > 65535.0
        || !set(
            model,
            holder,
            canvas,
            a(vec![s(canvas), m(s(canvas), "getContext")]),
        )
    {
        return None;
    }
    let (context, value) = init(model, context)?;
    if !fixed(model, context, 0)
        || context == canvas
        || !eq(
            model,
            value,
            &c(
                m(i(s(canvas), n(1.0)), "call"),
                vec![i(s(canvas), n(0.0)), t("2d")],
            ),
        )
    {
        return None;
    }
    let mut anchor = None;
    let mut gradients = BTreeSet::new();
    let mut palette = None;
    let mut converter = None;
    let mut at = 0;
    let mut rects = 0;
    let mut arcs = 0;
    let mut strokes = 0;
    while let Some(st) = rest.get(at) {
        if let Some((data, value)) = init(model, st) {
            if eq(
                model,
                value,
                &m(
                    c(
                        m(s(context), "getImageData"),
                        vec![n(0.0), n(0.0), n(width), n(height)],
                    ),
                    "data",
                ),
            ) {
                if rects == 0
                    || arcs == 0
                    || strokes == 0
                    || gradients.is_empty()
                    || !fixed(model, data, 1)
                {
                    return None;
                }
                return Some(Prefix {
                    context,
                    anchor: anchor?,
                    data,
                    width,
                    height,
                    after: 5 + at + 1,
                    draws: rects + arcs + strokes,
                });
            }
            if let Some(create) = call(value) {
                if member(&create.callee, "createElement").is_some() {
                    cap(model, site, member(&create.callee, "createElement")?)?;
                    let values = arguments(create)?;
                    if anchor.is_some()
                        || values.len() != 1
                        || !eq(model, values[0], &t("a"))
                        || !fixed(model, data, 0)
                    {
                        return None;
                    }
                    anchor = Some(data);
                    at += 1;
                    continue;
                }
            }
            let (id, used) = gradient(model, site, rest, at, context, &mut palette)?;
            if !gradients.insert(id) {
                return None;
            }
            at += used;
            continue;
        }
        if let Some(style) = prop(model, st, context, "fillStyle") {
            let f = call(style)?;
            let port = cap(model, site, &f.callee)?;
            let values = arguments(f)?;
            let [Expression::ArrayExpression(rgba)] = values.as_slice() else {
                return None;
            };
            if rgba.elements.len() != 4
                || rgba.elements.iter().any(|e| {
                    e.as_expression()
                        .and_then(integer)
                        .is_none_or(|v| !(0.0..=255.0).contains(&v))
                })
                || converter.is_some_and(|p| p != port)
            {
                return None;
            }
            converter = Some(port);
            at += 1;
            continue;
        }
        if let Some(style) = prop(model, st, context, "strokeStyle") {
            if !symbol(model, style).is_some_and(|id| gradients.contains(&id)) {
                cap(model, site, style)?;
            }
            at += 1;
            continue;
        }
        if let Some(value) = prop(model, st, context, "lineWidth") {
            if !matches!(value,Expression::NumericLiteral(v) if v.value.is_finite()&&v.value>0.0) {
                return None;
            }
            at += 1;
            continue;
        }
        let f = call(expression_statement(st)?)?;
        let Expression::StaticMemberExpression(member) = &f.callee else {
            return None;
        };
        if member.optional || symbol(model, &member.object) != Some(context) {
            return None;
        }
        let values = arguments(f)?;
        let arity = match member.property.name.as_str() {
            "fillRect" => {
                rects += 1;
                4
            }
            "arc" => {
                arcs += 1;
                5
            }
            "moveTo" | "lineTo" => 2,
            "stroke" => {
                strokes += 1;
                0
            }
            "beginPath" | "closePath" => 0,
            _ => return None,
        };
        if values.len() != arity || !values.iter().all(|e| geometry(model, e, 0)) {
            return None;
        }
        at += 1;
    }
    None
}

fn fallback(
    model: &Model<'_, '_>,
    site: FunctionSite,
    st: &Statement<'_>,
    data: Sym,
    seed: Sym,
) -> Option<()> {
    let [index, limit, Statement::DoWhileStatement(loop_), invoke, ret] = block(st)? else {
        return None;
    };
    let (index, value) = init(model, index)?;
    let (limit, value_limit) = local(model, limit)?;
    if !eq(model, value, &s(data))
        || value_limit.is_some()
        || !fixed(model, index, 1)
        || !fixed(model, limit, 1)
        || !void_return(model, ret)
    {
        return None;
    }
    let [store, read] = block(&loop_.body)? else {
        return None;
    };
    let store = assignment(expression_statement(store)?)?;
    let captured_index = target_symbol(model, &store.left)?;
    captured_symbol(model, site, captured_index)?;
    let capture = cap(model, site, assign(model, read, limit)?)?;
    if !eq(model, &store.right, &s(index)) {
        return None;
    }
    let Expression::LogicalExpression(and) = &loop_.test else {
        return None;
    };
    if and.operator.as_str() != "&&"
        || !eq(model, &and.left, &b(s(index), "<", m(s(limit), "length")))
    {
        return None;
    }
    let Expression::SequenceExpression(sequence) = and.right.without_parentheses() else {
        return None;
    };
    let [remove, update, truth] = sequence.expressions.as_slice() else {
        return None;
    };
    if !eq(
        model,
        remove,
        &c(
            E::G("removeEventListener"),
            vec![i(s(capture), s(captured_index)), s(data)],
        ),
    ) || !matches!(truth,Expression::BooleanLiteral(v) if v.value)
    {
        return None;
    }
    let change = assignment(update)?;
    if target_symbol(model, &change.left) != Some(index)
        || !eq(
            model,
            &change.right,
            &b(c(E::G("Number"), vec![s(captured_index)]), "+", n(1.0)),
        )
    {
        return None;
    }
    let invoke = call(expression_statement(invoke)?)?;
    let values = arguments(invoke)?;
    let target = member(member(&invoke.callee, "apply")?, "init")?;
    let target = call(target)?;
    let init_factory = cap(model, site, &target.callee)?;
    let object = || c(E::G("Object"), vec![E::This]);
    if !matches!(arguments(target)?.as_slice(),[v] if eq(model,v,&object()))
        || values.len() != 2
        || !eq(model, values[0], &object())
        || !eq(model, values[1], &m(s(seed), "detail"))
        || !distinct(&[
            index,
            limit,
            data,
            seed,
            capture,
            captured_index,
            init_factory,
        ])
    {
        return None;
    }
    Some(())
}
fn emit_matrix(
    model: &Model<'_, '_>,
    site: FunctionSite,
    st: &Statement<'_>,
    data: Sym,
    seed: Sym,
    bytes: Sym,
    cursor: Sym,
    writer: Sym,
) -> Option<Sym> {
    let Statement::IfStatement(condition) = st else {
        return None;
    };
    if !eq(model, &condition.test, &b(s(data), "!==", t("submit"))) {
        return None;
    }
    fallback(model, site, condition.alternate.as_ref()?, data, seed)?;
    let [index, length, Statement::WhileStatement(rows), next, complete, ret] =
        block(&condition.consequent)?
    else {
        return None;
    };
    let (index, value) = init(model, index)?;
    if !eq(model, value, &n(0.0))
        || !fixed(model, index, 1)
        || !eq(model, &rows.test, &b(s(index), "<", m(s(data), "length")))
        || !set(
            model,
            length,
            cursor,
            c(
                s(writer),
                vec![s(seed), m(s(data), "length"), s(bytes), s(cursor)],
            ),
        )
    {
        return None;
    }
    let [remaining, length, Statement::ForStatement(samples), step_st] = block(&rows.body)? else {
        return None;
    };
    let (remaining, value) = init(model, remaining)?;
    if !eq(model, value, &m(i(s(data), s(index)), "length"))
        || !fixed(model, remaining, 2)
        || !step(
            model,
            expression_statement(step_st)?,
            index,
            "++",
            Some(true),
        )
        || !set(
            model,
            length,
            cursor,
            c(s(writer), vec![s(seed), s(remaining), s(bytes), s(cursor)]),
        )
        || samples.init.is_some()
        || samples.test.is_some()
        || samples.update.is_some()
    {
        return None;
    }
    let [decrement, Statement::IfStatement(guard), emit, normalize] = block(&samples.body)? else {
        return None;
    };
    if !step(
        model,
        expression_statement(decrement)?,
        remaining,
        "--",
        Some(true),
    ) || guard.alternate.is_some()
        || !eq(model, &guard.test, &u("!", b(s(remaining), ">=", n(0.0))))
        || !matches!(&guard.consequent,Statement::BreakStatement(b) if b.label.is_none())
        || !set(
            model,
            emit,
            cursor,
            c(
                s(writer),
                vec![
                    s(seed),
                    i(i(s(data), s(index)), s(remaining)),
                    s(bytes),
                    s(cursor),
                ],
            ),
        )
        || !set(
            model,
            normalize,
            remaining,
            c(E::G("Number"), vec![s(remaining)]),
        )
        || !void_return(model, ret)
    {
        return None;
    }
    let next_call = call(expression_statement(next)?)?;
    let observer = cap(model, site, member(&next_call.callee, "next")?)?;
    if !ex(
        model,
        next,
        c(m(s(observer), "next"), vec![a(vec![a(vec![]), s(bytes)])]),
    ) || !ex(model, complete, c(m(s(observer), "complete"), vec![]))
        || !distinct(&[
            index, remaining, data, seed, bytes, cursor, writer, observer,
        ])
    {
        return None;
    }
    Some(observer)
}
fn prove(
    model: &Model<'_, '_>,
    site: FunctionSite,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Option<(usize, f64, f64)> {
    let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
        return None;
    };
    let body = f.body.as_ref()?;
    if f.r#async
        || f.generator
        || f.params.rest.is_some()
        || !f.params.items.is_empty()
        || body.directives.len() != 1
        || body.directives[0].directive != "use strict"
    {
        return None;
    }
    let prefix = scene_prefix(model, site, &body.statements)?;
    let [nonce, bytes, reseed, cursor, slots, probe, start, selections, emit] =
        &body.statements[prefix.after..]
    else {
        return None;
    };
    let (seed, value) = init(model, nonce)?;
    let (output, lanes) = init(model, bytes)?;
    if !fixed(model, seed, 1)
        || !fixed(model, output, 0)
        || !random(model, value, false)
        || !header(model, lanes, seed, false)
    {
        return None;
    }
    let reseed = call(assign(model, reseed, seed)?)?;
    let values = arguments(reseed)?;
    let factory = cap(model, site, &reseed.callee)?;
    if values.len() != 1 {
        return None;
    }
    let (seed_arg, salt) = binary(values[0], "^")?;
    if !eq(model, seed_arg, &b(s(seed), "|", n(0.0))) {
        return None;
    }
    u32_salt(salt)?;
    super::super::encoding::curried_seeded_emitter::factory_role(
        model,
        site,
        &reseed.callee,
        proposals,
        "nextKeystreamByte",
    )?;
    let (cursor, value) = init(model, cursor)?;
    let first = call(value)?;
    let writer = super::super::encoding::reverse_array_emitter::numeric_writer(
        model,
        site,
        &first.callee,
        proposals,
    )?;
    if !eq(
        model,
        value,
        &c(s(writer), vec![s(seed), n(-1.0), s(output), n(3.0)]),
    ) || !fixed(model, cursor, 3)
    {
        return None;
    }
    let ports = anchor_probe(
        model,
        site,
        slots,
        probe,
        start,
        prefix.anchor,
        writer,
        factory,
    )?;
    let Expression::ArrayExpression(selection) = assign(model, selections, prefix.data)? else {
        return None;
    };
    if selection.elements.is_empty() {
        return None;
    }
    let mut selector = None;
    for e in &selection.elements {
        let pick = call(e.as_expression()?)?;
        let values = arguments(pick)?;
        let port = cap(model, site, &pick.callee)?;
        if values.len() != 3
            || !eq(model, values[0], &s(prefix.data))
            || selector.is_some_and(|s| s != port)
        {
            return None;
        }
        let start = integer(values[1])?;
        let end = integer(values[2])?;
        if start < 0.0 || end - start != 4.0 || end > prefix.width * prefix.height * 4.0 {
            return None;
        }
        selector = Some(port);
    }
    // The stored value is a fresh ArrayExpression and the strict comparison is
    // the immediate next statement. No literal/operator is rewritten, and the
    // source fallback is nevertheless consumed by its own exact grammar above.
    let observer = emit_matrix(model, site, emit, prefix.data, seed, output, cursor, writer)?;
    if !distinct(&[
        prefix.context,
        prefix.anchor,
        prefix.data,
        seed,
        output,
        cursor,
        writer,
        factory,
        ports.anchor_observer,
        ports.scheduler,
        ports.digest,
        selector?,
        observer,
    ]) {
        return None;
    }
    Some((prefix.draws, prefix.width, prefix.height))
}
pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let Some((draws, width, height)) = prove(model, site, proposals) else {
        return vec![];
    };
    vec![proposal("drawCanvasAndEmitPixelFrameWithAnchorProbe","owned_canvas_scene_anchor_frame",&format!("complete {width}x{height} canvas protocol: {draws} rectangle/arc/stroke operations plus typed paths and ordered gradient loops; same context supplies image data; interleaved anchor property enumeration, try/catch string accumulation and closed scheduler/digest next/complete/error callbacks; four-byte selection-port ranges feed exact length-framed reverse rows through the source-proven numeric writer/live stream factory; explicit Number stores, literals and evaluation order retained; strict submit comparison follows a fresh array and its full removal/init fallback is still checked and preserved; external native canvas, anchor, scheduler, digest and selector identities explicitly unproved"),f)]
}
