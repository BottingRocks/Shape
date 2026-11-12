//! Whole callback-list protocols with opaque, scoped operation ports.
//! The operation is never assumed to be slice/push/indexOf/concat: only the
//! explicit argument, iteration, null guard and receiver-write effects are named.
use super::super::support::interaction::*;
use super::*;
use oxc_semantic::SymbolId;

enum E {
    S(SymbolId),
    N(f64),
    Null,
    This,
    M(Box<E>, &'static str),
    I(Box<E>, Box<E>),
    B(Box<E>, &'static str, Box<E>),
    C(Box<E>, Vec<E>),
}
fn s(x: SymbolId) -> E {
    E::S(x)
}
fn n(x: f64) -> E {
    E::N(x)
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
fn c(x: E, a: Vec<E>) -> E {
    E::C(Box::new(x), a)
}
fn list() -> E {
    m(E::This, "__callbacks")
}
fn eq(model: &Model<'_, '_>, e: &Expression<'_>, x: &E) -> bool {
    let e = e.without_parentheses();
    match (x, e) {
        (E::S(s), _) => symbol(model, e) == Some(*s),
        (E::N(n), Expression::NumericLiteral(v)) => *n == v.value,
        (E::N(n), Expression::UnaryExpression(v)) if *n < 0.0 => {
            v.operator.as_str() == "-" && eq(model, &v.argument, &E::N(-n))
        }
        (E::Null, Expression::NullLiteral(_)) | (E::This, Expression::ThisExpression(_)) => true,
        (E::M(x, k), Expression::StaticMemberExpression(v)) => {
            !v.optional && v.property.name == *k && eq(model, &v.object, x)
        }
        (E::I(x, y), Expression::ComputedMemberExpression(v)) => {
            !v.optional && eq(model, &v.object, x) && eq(model, &v.expression, y)
        }
        (E::B(x, k, y), Expression::BinaryExpression(v)) => {
            v.operator.as_str() == *k && eq(model, &v.left, x) && eq(model, &v.right, y)
        }
        (E::C(x, a), Expression::CallExpression(v)) => {
            !v.optional
                && eq(model, &v.callee, x)
                && v.arguments.len() == a.len()
                && v.arguments
                    .iter()
                    .zip(a)
                    .all(|(v, x)| v.as_expression().is_some_and(|v| eq(model, v, x)))
        }
        _ => false,
    }
}
fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    if let Statement::BlockStatement(b) = s {
        Some(&b.body)
    } else {
        None
    }
}
fn local<'a>(
    model: &Model<'_, '_>,
    st: &'a Statement<'a>,
) -> Option<(SymbolId, Option<&'a Expression<'a>>)> {
    let Statement::VariableDeclaration(d) = st else {
        return None;
    };
    if !matches!(
        d.kind,
        VariableDeclarationKind::Let
            | VariableDeclarationKind::Var
            | VariableDeclarationKind::Const
    ) {
        return None;
    }
    let [v] = d.declarations.as_slice() else {
        return None;
    };
    Some((plain_var(model, v)?, v.init.as_ref()))
}
fn port(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<SymbolId> {
    let id = captured_symbol(model, site, symbol(model, e)?)?;
    let AstKind::VariableDeclarator(v) = model.nodes.kind(model.scoping.symbol_declaration(id))
    else {
        return None;
    };
    // Undefined ports are intentionally permitted. No native identity or success
    // is asserted; a throwing port keeps exactly the same effects and catch scope.
    if v.span.end >= site.span.start || model.binding_writes(id) != 0 {
        return None;
    }
    Some(id)
}
fn operation(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
    tail: Vec<E>,
) -> Option<(SymbolId, SymbolId)> {
    let call = call(e)?;
    let a = arguments(call)?;
    if a.len() != tail.len() + 1 {
        return None;
    }
    let dispatch = port(model, site, &call.callee)?;
    let op = port(model, site, a[0])?;
    if dispatch == op || !a[1..].iter().zip(&tail).all(|(e, x)| eq(model, e, x)) {
        return None;
    }
    Some((dispatch, op))
}
fn assign_eq(model: &Model<'_, '_>, st: &Statement<'_>, target: SymbolId, x: E) -> bool {
    expression_statement(st)
        .and_then(assignment)
        .is_some_and(|a| target_symbol(model, &a.left) == Some(target) && eq(model, &a.right, &x))
}
fn conditional<'a>(
    model: &Model<'_, '_>,
    st: &'a Statement<'a>,
    test: E,
) -> Option<&'a [Statement<'a>]> {
    let Statement::IfStatement(g) = st else {
        return None;
    };
    if g.alternate.is_some() || !eq(model, &g.test, &test) {
        return None;
    }
    block(&g.consequent)
}
fn sole<'a>(xs: &'a [Statement<'a>]) -> Option<&'a Statement<'a>> {
    let [x] = xs else {
        return None;
    };
    Some(x)
}
fn unchanged(model: &Model<'_, '_>, id: SymbolId, w: usize) -> bool {
    model.binding_writes(id) == w && model.scoping.symbol_redeclarations(id).is_empty()
}
fn prove(model: &Model<'_, '_>, site: FunctionSite, f: &Function<'_>) -> Option<&'static str> {
    if f.r#async || f.generator || f.params.rest.is_some() || f.params.items.len() != 1 {
        return None;
    }
    let arg = plain_param(model, &f.params.items[0])?;
    let body = f.body.as_ref()?;
    if body.directives.len() > 1 || body.directives.iter().any(|d| d.directive != "use strict") {
        return None;
    }
    match body.statements.as_slice() {
        [forward @ Statement::ExpressionStatement(_)] => {
            operation(
                model,
                site,
                expression_statement(forward)?,
                vec![list(), s(arg)],
            )?;
            Some("forwardCallbackToListOperation")
        }
        [position, guard] => {
            let (index, value) = local(model, position)?;
            if !unchanged(model, index, 0) || index == arg {
                return None;
            }
            operation(model, site, value?, vec![list(), s(arg)])?;
            let write = assignment(expression_statement(sole(conditional(
                model,
                guard,
                b(s(index), "!==", n(-1.0)),
            )?)?)?)?;
            let AssignmentTarget::ComputedMemberExpression(slot) = &write.left else {
                return None;
            };
            if slot.optional
                || !eq(model, &slot.object, &list())
                || !eq(model, &slot.expression, &s(index))
                || !eq(model, &write.right, &E::Null)
            {
                return None;
            }
            Some("nullCallbackListSlotFromOperation")
        }
        [guard @ Statement::IfStatement(_)] => {
            let write = assignment(expression_statement(sole(conditional(
                model,
                guard,
                b(s(arg), "!=", E::Null),
            )?)?)?)?;
            let AssignmentTarget::StaticMemberExpression(target) = &write.left else {
                return None;
            };
            if target.optional
                || target.property.name != "__callbacks"
                || !eq(model, &target.object, &E::This)
            {
                return None;
            }
            operation(
                model,
                site,
                &write.right,
                vec![list(), m(s(arg), "__callbacks")],
            )?;
            Some("replaceCallbackListWithPeerOperation")
        }
        [array, count, temp, acc, iteration, returned] => {
            let (array, value) = local(model, array)?;
            operation(model, site, value?, vec![list()])?;
            let (count, value) = local(model, count)?;
            if !eq(model, value?, &m(s(array), "length")) {
                return None;
            }
            let (temp, value) = local(model, temp)?;
            if !eq(model, value?, &n(6.0)) {
                return None;
            }
            let (acc, value) = local(model, acc)?;
            if value.is_some() {
                return None;
            }
            let ids = [arg, array, count, temp, acc];
            if ids.into_iter().collect::<BTreeSet<_>>().len() != ids.len()
                || !unchanged(model, array, 0)
                || !unchanged(model, count, 1)
                || !unchanged(model, temp, 2)
                || !unchanged(model, acc, 1)
            {
                return None;
            }
            let Statement::WhileStatement(loop_) = iteration else {
                return None;
            };
            if !eq(
                model,
                &loop_.test,
                &b(b(s(count), "-", n(1.0)), ">=", n(0.0)),
            ) {
                return None;
            }
            let [Statement::TryStatement(attempt), step] = block(&loop_.body)? else {
                return None;
            };
            if attempt.finalizer.is_some() {
                return None;
            }
            let handler = attempt.handler.as_ref()?;
            let caught = plain_binding(model, &handler.param.as_ref()?.pattern)?;
            if !unchanged(model, caught, 0) || !handler.body.body.is_empty() {
                return None;
            }
            let Expression::UpdateExpression(u) = expression_statement(step)? else {
                return None;
            };
            let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &u.argument else {
                return None;
            };
            if u.operator.as_str() != "--"
                || !u.prefix
                || crate::analysis::identifier_symbol(id, model.scoping) != Some(count)
            {
                return None;
            }
            let [read, guard] = attempt.block.body.as_slice() else {
                return None;
            };
            if !assign_eq(model, read, temp, i(s(array), b(s(count), "-", n(1.0)))) {
                return None;
            }
            let [invoke, save] = conditional(model, guard, b(s(temp), "!=", E::Null))? else {
                return None;
            };
            if !assign_eq(model, invoke, temp, c(s(temp), vec![s(arg), s(acc)]))
                || !assign_eq(
                    model,
                    sole(conditional(model, save, b(s(temp), "!=", E::Null))?)?,
                    acc,
                    s(temp),
                )
            {
                return None;
            }
            let Statement::ReturnStatement(r) = returned else {
                return None;
            };
            if !eq(model, r.argument.as_ref()?, &s(acc)) {
                return None;
            }
            Some("foldReverseCallbackListOperation")
        }
        _ => None,
    }
}
pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
) -> Vec<Proposal> {
    let AstKind::Function(fun) = model.nodes.kind(site.node_id) else {
        return vec![];
    };
    let Some(role) = prove(model, site, fun) else {
        return vec![];
    };
    vec![proposal(role,"owned_callback_list_dispatch","complete receiver __callbacks operation protocol with binding-resolved opaque capture ports; exact parameter forwarding, null guards, list writes and reverse fold/catch effects; operation implementation and successful dispatch explicitly unproved",f)]
}
