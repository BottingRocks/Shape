//! Closed forward-record / reverse-sample scheduled text-pixel protocol.
//!
//! Every statement, loop, write and return is consumed on its actual owner.
//! The expression grammar below is exact and typed; it never compares source
//! spellings, bodies, hashes or edit distances. Captured helper implementations
//! and native canvas/scheduler origins remain explicitly unproved.
use super::*;
use crate::signals::extract::bindings::array_elements;
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;
type Symbol = SymbolId;

// A small expression algebra for the finite protocol, not a general evaluator.
enum E {
    Symbol(Symbol),
    Number(f64),
    Text(&'static str),
    Member(Box<E>, &'static str),
    Index(Box<E>, Box<E>),
    Binary(Box<E>, &'static str, Box<E>),
    Unary(&'static str, Box<E>),
    Call(Box<E>, Vec<E>),
    Array(Vec<E>),
    ReadOptions,
    Math,
}
fn s(x: Symbol) -> E {
    E::Symbol(x)
}
fn n(x: f64) -> E {
    E::Number(x)
}
fn t(x: &'static str) -> E {
    E::Text(x)
}
fn m(x: E, key: &'static str) -> E {
    E::Member(Box::new(x), key)
}
fn i(x: E, index: E) -> E {
    E::Index(Box::new(x), Box::new(index))
}
fn b(x: E, op: &'static str, y: E) -> E {
    E::Binary(Box::new(x), op, Box::new(y))
}
fn u(op: &'static str, x: E) -> E {
    E::Unary(op, Box::new(x))
}
fn c(x: E, args: Vec<E>) -> E {
    E::Call(Box::new(x), args)
}
fn a(args: Vec<E>) -> E {
    E::Array(args)
}
fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(e.without_parentheses(), model.scoping)
}
fn binding(p: &BindingPattern<'_>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(id) = p else {
        return None;
    };
    id.symbol_id.get()
}
fn call<'a>(e: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(x) = e.without_parentheses() else {
        return None;
    };
    (!x.optional).then_some(x)
}
fn args<'a>(x: &'a CallExpression<'a>) -> Option<Vec<&'a Expression<'a>>> {
    x.arguments.iter().map(|x| x.as_expression()).collect()
}
fn matches(model: &Model<'_, '_>, value: &Expression<'_>, expected: &E) -> bool {
    let value = value.without_parentheses();
    match (expected, value) {
        (E::Symbol(x), _) => symbol(model, value) == Some(*x),
        (E::Number(x), Expression::NumericLiteral(y)) => *x == y.value,
        (E::Number(x), Expression::UnaryExpression(y)) if *x < 0.0 => {
            y.operator.as_str() == "-" && matches(model, &y.argument, &n(-x))
        }
        (E::Text(x), Expression::StringLiteral(y)) => *x == y.value,
        (E::Math, Expression::Identifier(id)) => {
            id.name == "Math" && symbol(model, value).is_none()
        }
        (E::Member(x, key), Expression::StaticMemberExpression(y)) => {
            !y.optional && y.property.name == *key && matches(model, &y.object, x)
        }
        (E::Index(x, index), Expression::ComputedMemberExpression(y)) => {
            !y.optional && matches(model, &y.object, x) && matches(model, &y.expression, index)
        }
        (E::Binary(x, op, z), Expression::BinaryExpression(y)) => {
            y.operator.as_str() == *op && matches(model, &y.left, x) && matches(model, &y.right, z)
        }
        (E::Unary(op, x), Expression::UnaryExpression(y)) => {
            y.operator.as_str() == *op && matches(model, &y.argument, x)
        }
        (E::Call(x, values), _) => call(value).is_some_and(|y| {
            matches(model, &y.callee, x)
                && args(y).is_some_and(|actual| {
                    actual.len() == values.len()
                        && actual.iter().zip(values).all(|(v, e)| matches(model, v, e))
                })
        }),
        (E::Array(values), _) => array_elements(value).is_some_and(|actual| {
            actual.len() == values.len()
                && actual.iter().zip(values).all(|(v, e)| matches(model, v, e))
        }),
        (E::ReadOptions, Expression::ObjectExpression(object)) => {
            let [ObjectPropertyKind::ObjectProperty(p)] = object.properties.as_slice() else {
                return false;
            };
            !p.computed
                && !p.method
                && !p.shorthand
                && p.kind == PropertyKind::Init
                && property_key_name(&p.key).as_deref() == Some("willReadFrequently")
                && matches!(&p.value, Expression::BooleanLiteral(x) if x.value)
        }
        _ => false,
    }
}
fn expr<'a>(st: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ExpressionStatement(st) = st else {
        return None;
    };
    Some(&st.expression)
}
fn block<'a>(st: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(st) = st else {
        return None;
    };
    Some(&st.body)
}
fn local<'a>(st: &'a Statement<'a>) -> Option<(Symbol, &'a Expression<'a>)> {
    let Statement::VariableDeclaration(st) = st else {
        return None;
    };
    if !matches!(
        st.kind,
        VariableDeclarationKind::Var
            | VariableDeclarationKind::Let
            | VariableDeclarationKind::Const
    ) {
        return None;
    }
    let [v] = st.declarations.as_slice() else {
        return None;
    };
    Some((binding(&v.id)?, v.init.as_ref()?))
}
fn assignment<'a>(
    model: &Model<'_, 'a>,
    st: &'a Statement<'a>,
) -> Option<(Symbol, &'a Expression<'a>)> {
    let Expression::AssignmentExpression(x) = expr(st)?.without_parentheses() else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(id) = &x.left else {
        return None;
    };
    if x.operator.as_str() != "=" {
        return None;
    }
    Some((
        crate::analysis::identifier_symbol(id, model.scoping)?,
        &x.right,
    ))
}
fn is_expr(model: &Model<'_, '_>, st: &Statement<'_>, e: E) -> bool {
    expr(st).is_some_and(|v| matches(model, v, &e))
}
fn is_assign(model: &Model<'_, '_>, st: &Statement<'_>, target: Symbol, e: E) -> bool {
    assignment(model, st).is_some_and(|(x, v)| x == target && matches(model, v, &e))
}
fn set_member(model: &Model<'_, '_>, st: &Statement<'_>, object: Symbol, key: &str, e: E) -> bool {
    let Some(Expression::AssignmentExpression(x)) = expr(st).map(Expression::without_parentheses)
    else {
        return false;
    };
    let AssignmentTarget::StaticMemberExpression(left) = &x.left else {
        return false;
    };
    x.operator.as_str() == "="
        && !left.optional
        && left.property.name == key
        && symbol(model, &left.object) == Some(object)
        && matches(model, &x.right, &e)
}
fn writes(model: &Model<'_, '_>, x: Symbol, count: usize) -> bool {
    model.binding_writes(x) == count && model.scoping.symbol_redeclarations(x).is_empty()
}
fn one_origin(model: &Model<'_, '_>, x: Symbol) -> bool {
    writes(model, x, if model.initializer(x).is_some() { 0 } else { 1 })
}
fn distinct(xs: &[Symbol]) -> bool {
    xs.iter().enumerate().all(|(i, x)| !xs[..i].contains(x))
}
fn update(model: &Model<'_, '_>, e: &Expression<'_>, x: Symbol, op: &str) -> bool {
    let Expression::UpdateExpression(v) = e.without_parentheses() else {
        return false;
    };
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &v.argument else {
        return false;
    };
    v.operator.as_str() == op && crate::analysis::identifier_symbol(id, model.scoping) == Some(x)
}
fn formal(model: &Model<'_, '_>, p: &FormalParameter<'_>) -> Option<Symbol> {
    let x = binding(&p.pattern)?;
    (p.initializer.is_none() && writes(model, x, 0)).then_some(x)
}
fn arrow<'a>(e: &'a Expression<'a>, arity: usize) -> Option<&'a ArrowFunctionExpression<'a>> {
    let Expression::ArrowFunctionExpression(x) = e.without_parentheses() else {
        return None;
    };
    (!x.r#async
        && !x.expression
        && x.params.rest.is_none()
        && x.params.items.len() == arity
        && x.body.directives.is_empty())
    .then_some(x)
}
// Follow only ordered, straight-line, stable captured aliases to a lexical
// function. The helper's implementation/purpose is deliberately not inferred.
fn capture(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<Symbol> {
    capture_impl(model, site, e, false)
}
// Reverse-row witnesses may declare their callable before captured setup. The
// shared guard proves every actual read/exposure follows the sole executed
// origin, at every alias hop. The older forward contract keeps its textual gate.
fn capture_impl(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
    late: bool,
) -> Option<Symbol> {
    // Reuse the established exposure-order guard, including early calls and
    // conditional assignments, without weakening that shared proof.
    super::stable_function(model, e, 0)?;
    let mut current = e;
    let mut before = site.span.start;
    for _ in 0..16 {
        let x = symbol(model, current)?;
        if site.span.contains_inclusive(model.scoping.symbol_span(x))
            || !model.scoping.symbol_redeclarations(x).is_empty()
        {
            return None;
        }
        let declaration = model.scoping.symbol_declaration(x);
        if matches!(model.nodes.kind(declaration), AstKind::Function(_)) {
            return writes(model, x, 0).then_some(x);
        }
        if !one_origin(model, x) {
            return None;
        }
        let owner = model.owning_site(declaration)?;
        let value = model.symbol_value(x)?;
        if !late && value.span().end >= before {
            return None;
        }
        if model.initializer(x).is_none() {
            let write = model
                .scoping
                .get_resolved_references(x)
                .find(|r| r.is_write())?;
            if model.owning_site(write.node_id())?.span != owner.span {
                return None;
            }
        }
        if matches!(
            value.without_parentheses(),
            Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
        ) {
            return Some(x);
        }
        before = value.span().start;
        current = value;
    }
    None
}
struct Outer {
    observer: Symbol,
    sink: Symbol,
    array: Symbol,
    canvas: Symbol,
    queue: Symbol,
}
struct Sampling {
    push: Symbol,
    receiver: Symbol,
    pixels: Symbol,
}
fn sample_loop(
    model: &Model<'_, '_>,
    st: &Statement<'_>,
    init: &Statement<'_>,
    message: Symbol,
    context: Symbol,
    pixels: Symbol,
) -> Option<()> {
    let (index, value) = local(init)?;
    if !matches(model, value, &n(0.0)) || !writes(model, index, 1) {
        return None;
    }
    let Statement::ForStatement(loop_) = st else {
        return None;
    };
    if loop_.init.is_some()
        || !matches(
            model,
            loop_.test.as_ref()?,
            &b(s(index), "<", m(i(s(message), n(1.0)), "length")),
        )
        || !update(model, loop_.update.as_ref()?, index, "++")
    {
        return None;
    }
    let [read, push] = block(&loop_.body)? else {
        return None;
    };
    let (data, value) = local(read)?;
    let point = || i(i(s(message), n(1.0)), s(index));
    if !writes(model, data, 0)
        || !matches(
            model,
            value,
            &m(
                c(
                    m(s(context), "getImageData"),
                    vec![i(point(), n(0.0)), i(point(), n(1.0)), n(1.0), n(1.0)],
                ),
                "data",
            ),
        )
    {
        return None;
    }
    let lane = |x| i(s(data), n(x));
    let rgba = b(
        b(
            b(
                b(lane(0.0), "<<", n(24.0)),
                "|",
                b(lane(1.0), "<<", n(16.0)),
            ),
            "|",
            b(lane(2.0), "<<", n(8.0)),
        ),
        "|",
        lane(3.0),
    );
    is_expr(model, push, c(m(s(pixels), "push"), vec![rgba])).then_some(())
}
fn reverse_loop(
    model: &Model<'_, '_>,
    st: &Statement<'_>,
    index: Symbol,
    row: Symbol,
    cursor: Symbol,
    mixer: Symbol,
    state: Symbol,
    bytes: Symbol,
) -> Option<()> {
    let write = |st| {
        is_assign(
            model,
            st,
            cursor,
            c(
                s(mixer),
                vec![s(state), i(s(row), s(index)), s(bytes), s(cursor)],
            ),
        )
    };
    match st {
        Statement::WhileStatement(loop_) => {
            let [emit, step] = block(&loop_.body)? else {
                return None;
            };
            if !matches(model, &loop_.test, &b(s(index), ">=", n(0.0)))
                || !write(emit)
                || !update(model, expr(step)?, index, "--")
            {
                return None;
            }
        }
        Statement::ForStatement(loop_) => {
            let [emit] = block(&loop_.body)? else {
                return None;
            };
            if loop_.init.is_some()
                || !matches(model, loop_.test.as_ref()?, &b(s(index), ">=", n(0.0)))
                || !update(model, loop_.update.as_ref()?, index, "--")
                || !write(emit)
            {
                return None;
            }
        }
        _ => return None,
    }
    Some(())
}
// Shared complete framing prefix for both traversal orders.
fn frame_prefix(
    model: &Model<'_, '_>,
    site: FunctionSite,
    st: &[&Statement<'_>],
    sampling: &Sampling,
    late: bool,
) -> Option<(Symbol, Symbol, Symbol, f64)> {
    let [push, random, header, reseed] = st else {
        return None;
    };
    if !is_expr(
        model,
        push,
        c(
            m(s(sampling.push), "call"),
            vec![s(sampling.receiver), s(sampling.pixels)],
        ),
    ) {
        return None;
    }
    let (state, value) = local(random)?;
    if !writes(model, state, 1)
        || !matches(
            model,
            value,
            &b(c(m(E::Math, "random"), vec![]), "*", n(4294967296.0)),
        )
    {
        return None;
    }
    let (bytes, value) = local(header)?;
    let word = || b(s(state), "|", n(0.0));
    let lanes = a(vec![
        b(word(), ">>>", n(24.0)),
        b(b(word(), ">>", n(16.0)), "&", n(255.0)),
        b(b(word(), ">>", n(8.0)), "&", n(255.0)),
        b(word(), "&", n(255.0)),
    ]);
    if !writes(model, bytes, 0) || !matches(model, value, &lanes) {
        return None;
    }
    let (target, value) = assignment(model, reseed)?;
    let hash_call = call(value)?;
    let hash = symbol(model, &hash_call.callee)?;
    let hash_origin = capture_impl(model, site, &hash_call.callee, late)?;
    let hash_args = args(hash_call)?;
    let [arg] = hash_args.as_slice() else {
        return None;
    };
    let Expression::BinaryExpression(xor) = arg.without_parentheses() else {
        return None;
    };
    let Expression::NumericLiteral(salt) = xor.right.without_parentheses() else {
        return None;
    };
    if target != state
        || xor.operator.as_str() != "^"
        || !matches(model, &xor.left, &word())
        || salt.value < 0.0
        || salt.value > u32::MAX as f64
        || salt.value.fract() != 0.0
    {
        return None;
    }
    Some((state, bytes, hash_origin, salt.value))
}
fn encoded_arm(
    model: &Model<'_, '_>,
    site: FunctionSite,
    st: &[Statement<'_>],
    outer: &Outer,
    sampling: &Sampling,
) -> Option<(Symbol, Symbol, f64)> {
    let [push, random, header, reseed, count, index, cursor, encode, emit, exit] = st else {
        return None;
    };
    let (state, bytes, hash_origin, salt) = frame_prefix(
        model,
        site,
        &[push, random, header, reseed],
        sampling,
        false,
    )?;
    let (count, value) = local(count)?;
    if !writes(model, count, 0) || !matches(model, value, &m(s(outer.array), "length")) {
        return None;
    }
    let (index, value) = local(index)?;
    if !writes(model, index, 1) || !matches(model, value, &n(0.0)) {
        return None;
    }
    let (cursor, value) = local(cursor)?;
    let first = call(value)?;
    let mixer = symbol(model, &first.callee)?;
    let mixer_origin = capture(model, site, &first.callee)?;
    if mixer_origin == hash_origin
        || !writes(model, cursor, 2)
        || !matches(
            model,
            value,
            &c(s(mixer), vec![s(state), s(count), s(bytes), n(3.0)]),
        )
    {
        return None;
    }
    let Statement::WhileStatement(loop_) = encode else {
        return None;
    };
    if !matches(model, &loop_.test, &b(s(index), "<", s(count))) {
        return None;
    }
    let [row, reverse, write, inner, step] = block(&loop_.body)? else {
        return None;
    };
    let (row, value) = local(row)?;
    if !writes(model, row, 0) || !matches(model, value, &i(s(outer.array), s(index))) {
        return None;
    }
    let (reverse, value) = local(reverse)?;
    if !writes(model, reverse, 1)
        || !matches(model, value, &b(m(s(row), "length"), "-", n(1.0)))
        || !is_assign(
            model,
            write,
            cursor,
            c(
                s(mixer),
                vec![s(state), m(s(row), "length"), s(bytes), s(cursor)],
            ),
        )
        || !update(model, expr(step)?, index, "++")
    {
        return None;
    }
    reverse_loop(model, inner, reverse, row, cursor, mixer, state, bytes)?;
    if !distinct(&[
        state,
        bytes,
        count,
        index,
        cursor,
        row,
        reverse,
        hash_origin,
        mixer,
        outer.array,
        sampling.pixels,
        sampling.receiver,
        sampling.push,
    ]) || !is_expr(
        model,
        emit,
        c(
            m(s(outer.observer), "next"),
            vec![a(vec![a(vec![]), s(bytes)])],
        ),
    ) {
        return None;
    }
    let Statement::ReturnStatement(exit) = exit else {
        return None;
    };
    if !matches(model, exit.argument.as_ref()?, &u("void", n(0.0))) {
        return None;
    }
    Some((hash_origin, mixer_origin, salt))
}

// Reverse rows and reverse samples are a distinct protocol: length is framed
// before either decrement, and Number(index) stores remain observable effects.
fn reverse_encoded_arm(
    model: &Model<'_, '_>,
    site: FunctionSite,
    st: &[Statement<'_>],
    outer: &Outer,
    sampling: &Sampling,
) -> Option<(Symbol, Symbol, f64)> {
    let [push, random, header, reseed, index, cursor, Statement::WhileStatement(loop_), emit, exit] =
        st
    else {
        return None;
    };
    let (state, bytes, hash_origin, salt) =
        frame_prefix(model, site, &[push, random, header, reseed], sampling, true)?;
    let (index, value) = local(index)?;
    if !writes(model, index, 1)
        || !matches(model, value, &b(m(s(outer.array), "length"), "-", n(1.0)))
        || !matches(model, &loop_.test, &b(s(index), ">=", n(0.0)))
    {
        return None;
    }
    let (cursor, value) = local(cursor)?;
    let first = call(value)?;
    let mixer = symbol(model, &first.callee)?;
    let mixer_origin = capture_impl(model, site, &first.callee, true)?;
    if hash_origin == mixer_origin
        || !writes(model, cursor, 2)
        || !matches(
            model,
            value,
            &c(
                s(mixer),
                vec![s(state), m(s(outer.array), "length"), s(bytes), n(3.0)],
            ),
        )
    {
        return None;
    }
    let [row, reverse, write, Statement::ForStatement(inner), step] = block(&loop_.body)? else {
        return None;
    };
    let (row, value) = local(row)?;
    if !writes(model, row, 0) || !matches(model, value, &i(s(outer.array), s(index))) {
        return None;
    }
    let (reverse, value) = local(reverse)?;
    if !writes(model, reverse, 2)
        || !matches(model, value, &m(s(row), "length"))
        || !is_assign(
            model,
            write,
            cursor,
            c(s(mixer), vec![s(state), s(reverse), s(bytes), s(cursor)]),
        )
        || !update(model, expr(step)?, index, "--")
        || inner.init.is_some()
        || inner.test.is_some()
        || inner.update.is_some()
    {
        return None;
    }
    let [dec, Statement::IfStatement(stop), write, coerce] = block(&inner.body)? else {
        return None;
    };
    let Statement::BreakStatement(br) = &stop.consequent else {
        return None;
    };
    if br.label.is_some()
        || stop.alternate.is_some()
        || !matches(model, &stop.test, &u("!", b(s(reverse), ">=", n(0.0))))
        || !update(model, expr(dec)?, reverse, "--")
        || !is_assign(
            model,
            write,
            cursor,
            c(
                s(mixer),
                vec![s(state), i(s(row), s(reverse)), s(bytes), s(cursor)],
            ),
        )
    {
        return None;
    }
    let (target, value) = assignment(model, coerce)?;
    let coercion = call(value)?;
    if target != reverse
        || !matches!(&coercion.callee,Expression::Identifier(id) if id.name=="Number" && symbol(model,&coercion.callee).is_none())
        || args(coercion)?.len() != 1
        || !matches(model, args(coercion)?[0], &s(reverse))
    {
        return None;
    }
    if !distinct(&[
        state,
        bytes,
        index,
        cursor,
        row,
        reverse,
        mixer,
        outer.array,
        sampling.pixels,
        sampling.receiver,
        sampling.push,
    ]) || !is_expr(
        model,
        emit,
        c(
            m(s(outer.observer), "next"),
            vec![a(vec![a(vec![]), s(bytes)])],
        ),
    ) {
        return None;
    }
    let Statement::ReturnStatement(exit) = exit else {
        return None;
    };
    matches(model, exit.argument.as_ref()?, &u("void", n(0.0))).then_some((
        hash_origin,
        mixer_origin,
        salt,
    ))
}
fn sampler(
    model: &Model<'_, '_>,
    site: FunctionSite,
    callback: &Expression<'_>,
    outer: &Outer,
) -> Option<(Symbol, Symbol, bool)> {
    let arrow = arrow(callback, 1)?;
    let message = formal(model, &arrow.params.items[0])?;
    let [receiver, push, canvas, height, width, pixels, Statement::TryStatement(attempt)] =
        arrow.body.statements.as_slice()
    else {
        return None;
    };
    let (receiver, value) = local(receiver)?;
    if !writes(model, receiver, 0) || !matches(model, value, &s(outer.array)) {
        return None;
    }
    let (push, value) = local(push)?;
    if !writes(model, push, 0)
        || !(matches(model, value, &m(s(receiver), "push"))
            || matches(model, value, &m(s(outer.array), "push")))
    {
        return None;
    }
    let (canvas, value) = local(canvas)?;
    if !writes(model, canvas, 0)
        || !matches(model, value, &s(outer.canvas))
        || !set_member(model, height, canvas, "height", n(96.0))
        || !set_member(model, width, canvas, "width", n(96.0))
    {
        return None;
    }
    let (pixels, value) = local(pixels)?;
    if !writes(model, pixels, 0)
        || !matches(model, value, &a(vec![]))
        || attempt.finalizer.is_some()
    {
        return None;
    }
    let handler = attempt.handler.as_ref()?;
    let caught = binding(&handler.param.as_ref()?.pattern)?;
    if !writes(model, caught, 0)
        || model
            .scoping
            .get_resolved_references(caught)
            .next()
            .is_some()
    {
        return None;
    }
    let body = attempt.block.body.as_slice();
    let [context, font, fill, rest @ ..] = body else {
        return None;
    };
    let (context, value) = local(context)?;
    if !writes(model, context, 0)
        || !matches(
            model,
            value,
            &c(m(s(canvas), "getContext"), vec![t("2d"), E::ReadOptions]),
        )
        || !set_member(
            model,
            font,
            context,
            "font",
            b(
                b(m(s(canvas), "height"), "-", n(2.0)),
                "+",
                t("px sans-serif"),
            ),
        )
        || !set_member(model, fill, context, "fillStyle", t("#000"))
    {
        return None;
    }
    let draw_args = || vec![i(s(message), n(0.0)), n(-1.0), m(s(canvas), "height")];
    let tail = match rest {
        [draw, tail @ ..] if is_expr(model, draw, c(m(s(context), "fillText"), draw_args())) => {
            tail
        }
        [holder, draw, tail @ ..] => {
            let (holder, value) = local(holder)?;
            if !writes(model, holder, 0)
                || !matches(
                    model,
                    value,
                    &a(vec![s(context), m(s(context), "fillText")]),
                )
            {
                return None;
            }
            let mut args = vec![i(s(holder), n(0.0))];
            args.extend(draw_args());
            if !is_expr(model, draw, c(m(i(s(holder), n(1.0)), "call"), args)) {
                return None;
            }
            tail
        }
        _ => return None,
    };
    let [init, samples, tail @ ..] = tail else {
        return None;
    };
    if !distinct(&[
        message,
        receiver,
        push,
        canvas,
        pixels,
        context,
        outer.array,
        outer.canvas,
        outer.queue,
        outer.observer,
        outer.sink,
    ]) {
        return None;
    }
    sample_loop(model, samples, init, message, context, pixels)?;
    let sampling = Sampling {
        push,
        receiver,
        pixels,
    };
    let proof = |st: &[Statement<'_>]| {
        encoded_arm(model, site, st, outer, &sampling)
            .map(|x| (x, false))
            .or_else(|| reverse_encoded_arm(model, site, st, outer, &sampling).map(|x| (x, true)))
    };
    let success = proof(tail)?;
    let error = proof(&handler.body.body)?;
    (success == error).then_some((success.0 .0, success.0 .1, success.1))
}
fn completion(model: &Model<'_, '_>, e: &Expression<'_>, outer: &Outer) -> Option<()> {
    let arrow = arrow(e, 0)?;
    let [emit] = arrow.body.statements.as_slice() else {
        return None;
    };
    is_expr(model, emit, c(s(outer.sink), vec![s(outer.array)])).then_some(())
}
fn teardown(
    model: &Model<'_, '_>,
    st: &Statement<'_>,
    returned: &Statement<'_>,
    queue: Symbol,
) -> Option<()> {
    let Statement::FunctionDeclaration(f) = st else {
        return None;
    };
    let id = f.id.as_ref()?.symbol_id.get()?;
    let Statement::ReturnStatement(ret) = returned else {
        return None;
    };
    if f.r#async
        || f.generator
        || f.params.rest.is_some()
        || !f.params.items.is_empty()
        || !writes(model, id, 0)
        || !matches(model, ret.argument.as_ref()?, &s(id))
    {
        return None;
    }
    let body = f.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let [Statement::IfStatement(branch)] = body.statements.as_slice() else {
        return None;
    };
    let [empty] = block(&branch.consequent)? else {
        return None;
    };
    (branch.alternate.is_none()
        && matches(model, &branch.test, &s(queue))
        && is_expr(model, empty, c(m(s(queue), "empty"), vec![])))
    .then_some(())
}
fn prove(model: &Model<'_, '_>, site: FunctionSite) -> Option<bool> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return None;
    };
    let body = function.body.as_ref()?;
    if function.r#async
        || function.generator
        || !body.directives.is_empty()
        || function.params.rest.is_some()
    {
        return None;
    }
    let [observer, document, budget, sink] = function.params.items.as_slice() else {
        return None;
    };
    let formals = [
        formal(model, observer)?,
        formal(model, document)?,
        formal(model, budget)?,
        formal(model, sink)?,
    ];
    let mut declared = BTreeSet::new();
    let mut values = Vec::new();
    let mut queue_at = None;
    for (pos, st) in body.statements.iter().enumerate() {
        let mut next = Vec::new();
        match st {
            Statement::VariableDeclaration(vs) => {
                if !matches!(
                    vs.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                ) {
                    return None;
                }
                for v in &vs.declarations {
                    let x = binding(&v.id)?;
                    if !declared.insert(x) || !one_origin(model, x) {
                        return None;
                    }
                    if let Some(value) = v.init.as_ref() {
                        next.push((x, value));
                    }
                }
            }
            Statement::ExpressionStatement(_) => {
                let (x, value) = assignment(model, st)?;
                if !declared.contains(&x) || !one_origin(model, x) {
                    return None;
                }
                next.push((x, value));
            }
            _ => return None,
        }
        for (x, value) in next {
            if values.iter().any(|(old, _)| *old == x) {
                return None;
            }
            values.push((x, value));
        }
        if values.last().is_some_and(|(_, v)| {
            call(v).is_some_and(|q| {
                args(q).is_some_and(|a| {
                    a.len() == 3
                        && matches!(
                            a[0].without_parentheses(),
                            Expression::ArrowFunctionExpression(_)
                        )
                })
            })
        }) {
            queue_at = Some(pos);
            break;
        }
        if values.len() >= 5 {
            return None;
        }
    }
    if declared.len() != 5 || values.len() != 5 {
        return None;
    }
    let (queue, queue_value) = *values.last()?;
    let mut observer = None;
    let mut sink = None;
    let mut array = None;
    let mut canvas = None;
    for &(x, value) in &values[..4] {
        if matches(model, value, &s(formals[0])) {
            if observer.replace(x).is_some() {
                return None;
            }
        } else if matches(model, value, &s(formals[3])) {
            if sink.replace(x).is_some() {
                return None;
            }
        } else if matches(model, value, &a(vec![])) {
            if array.replace(x).is_some() {
                return None;
            }
        } else if matches(
            model,
            value,
            &c(m(s(formals[1]), "createElement"), vec![t("canvas")]),
        ) {
            if canvas.replace(x).is_some() {
                return None;
            }
        } else {
            return None;
        }
        if value.span().end >= queue_value.span().start {
            return None;
        }
    }
    let outer = Outer {
        observer: observer?,
        sink: sink?,
        array: array?,
        canvas: canvas?,
        queue,
    };
    let factory = call(queue_value)?;
    let arguments = args(factory)?;
    let [callback, delay, budget] = arguments.as_slice() else {
        return None;
    };
    if !matches(model, delay, &n(45.0)) || !matches(model, budget, &s(formals[2])) {
        return None;
    }
    let (hash, mixer, reverse) = sampler(model, site, callback, &outer)?;
    let scheduler_origin = capture_impl(model, site, &factory.callee, reverse)?;
    if !distinct(&[scheduler_origin, hash, mixer]) {
        return None;
    }
    let remaining = &body.statements[queue_at? + 1..];
    let (registration, start, cleanup, ret) = match remaining {
        [registration, start, cleanup, ret] => {
            let reg = call(expr(registration)?)?;
            let values = args(reg)?;
            let [callback] = values.as_slice() else {
                return None;
            };
            if !matches(model, &reg.callee, &m(s(queue), "onEmpty")) {
                return None;
            }
            completion(model, callback, &outer)?;
            (registration, start, cleanup, ret)
        }
        [holder, registration, start, cleanup, ret] => {
            let (holder, value) = local(holder)?;
            if !writes(model, holder, 0)
                || !matches(model, value, &a(vec![s(queue), m(s(queue), "onEmpty")]))
            {
                return None;
            }
            let reg = call(expr(registration)?)?;
            let values = args(reg)?;
            let [receiver, callback] = values.as_slice() else {
                return None;
            };
            if !matches(model, &reg.callee, &m(i(s(holder), n(1.0)), "call"))
                || !matches(model, receiver, &i(s(holder), n(0.0)))
            {
                return None;
            }
            completion(model, callback, &outer)?;
            (registration, start, cleanup, ret)
        }
        _ => return None,
    };
    let _ = registration; // consumed above, including its complete argument list.
    if !is_expr(model, start, c(m(s(queue), "start"), vec![])) {
        return None;
    }
    teardown(model, cleanup, ret, queue)?;
    Some(reverse)
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let Some(reverse) = prove(model, site) else {
        return Vec::new();
    };
    if reverse {
        return vec![proposal("scheduleReverseTextPixelSamples", "shared_reverse_canvas_text_sampling_setup", "whole scheduled text pixel protocol: owns 96x96 canvas, glyph draw and ordered RGBA point reads, row accumulation in both success and catch; frames row count, iterates rows and samples backwards preserving length-before-decrement and Number index writes, threads exact captured writer origins and header, emits [[], bytes], registers completion, starts and returns guarded empty teardown; captured helper implementations remain opaque", facts)];
    }
    vec![proposal(
        "scheduleTextPixelSamples",
        "shared_canvas_text_sampling_setup",
        "owned closed scheduled text-pixel protocol: creates a canvas-named object through formal slot 1, constructs one captured callable with its directly owned glyph callback, 45 and formal slot 2; callback sets 96x96 geometry, height-minus-two sans-serif and black fill, draws the input glyph, reads requested 1x1 RGBA samples in forward order and packs lanes 24/16/8/0; success and catch append the same row and encode accumulated records forward and samples backward through identical captured helper origins and salt, then emit [[], bytes] on slot 0; registers slot 3 with the same accumulator on the same instance's onEmpty, starts it and returns its exact guarded empty cleanup. Native canvas origin, scheduler implementation and captured encoding helper purposes are explicitly unproved; no nested helper role is inferred",
        facts,
    )]
}
