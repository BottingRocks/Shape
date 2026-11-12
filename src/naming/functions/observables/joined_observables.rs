//! Bounded map-only protocol. Each declaration is consumed in its exact role;
//! no sibling body or inherited finder label supplies evidence.
use crate::naming::shared::*;
use crate::naming::functions::observables::observable_factory;
use crate::naming::*;
use oxc_semantic::{NodeId, SymbolId};
use oxc_span::GetSpan;
type Sym = SymbolId;
mod epochs;

/// Shared constructor lifetime proof only. Callers must separately prove their
/// entire operational protocol; no join role or sibling body is transferred.
/// All stores, complete own class makers, closed constructor reads and callable
/// publications use exactly the accepted join epoch semantics.
pub(in crate::naming) fn constructor_epochs(
    m: &Model<'_, '_>,
    e: &Expression<'_>,
    factory: Sym,
) -> Option<()> {
    epochs::prove_operator(m, e, factory)
}

// These tests pin a refused residual, rather than turning a partial callback
// shape into native join evidence. Production acceptance is unchanged.
#[cfg(test)]
mod joined_stream_state_and_cleanup {
    use super::*;

    pub(super) const ACTUAL: &str =
        include_str!("../../../../tests/fixtures/joined_stream_state_and_cleanup/constructor-epoch-parent.js");
    pub(super) const CONTROL: &str =
        include_str!("../../../../tests/fixtures/naming/joined_observables/joined-state-excerpt-alternate.js");

    pub(super) fn inspect(
        source: &str,
        name: &str,
        check: impl FnOnce(&Model<'_, '_>, &Function<'_>),
    ) {
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
            panic!("function site")
        };
        check(&model, f);
    }

    #[test]
    fn actual_first_failed_guard_is_three_constructor_writes() {
        inspect(ACTUAL, "$THREAD_3_64859", |m, f| {
            assert_eq!(f.span, Span::new(23802, 24849));
            let fs = symbol(f).unwrap();
            assert!(fixed(m, fs));
            let p = params(m, f, 1).unwrap()[0];
            let [capture, returned] = body(f).unwrap() else {
                panic!("two statements")
            };
            alias(m, capture, p).unwrap();
            let Expression::NewExpression(n) = ret(returned).unwrap() else {
                panic!("constructor call")
            };
            let s = id(m, &n.callee).unwrap();
            assert!(m.scoping.symbol_redeclarations(s).is_empty());
            let d = m.scoping.symbol_declaration(s);
            let AstKind::VariableDeclarator(v) = m.nodes.kind(d) else {
                panic!("variable origin")
            };
            assert!(v.init.is_none());
            let declaration = m.nodes.parent_id(d);
            assert!(matches!(
                m.nodes.kind(declaration),
                AstKind::VariableDeclaration(_)
            ));
            assert!(matches!(
                m.nodes.kind(m.nodes.parent_id(declaration)),
                AstKind::FunctionBody(_)
            ));
            assert_eq!(m.binding_writes(s), 3);
            let writes: Vec<_> = m
                .scoping
                .get_resolved_references(s)
                .filter(|r| r.is_write())
                .map(|r| {
                    let span = m.nodes.kind(r.node_id()).span();
                    serde_json::json!({"start":span.start,"end":span.end})
                })
                .collect();
            println!(
                "{}",
                serde_json::json!({"guard":"origin: binding_writes != 1", "symbol_id":format!("{s:?}"),"declaration":{"start":m.scoping.symbol_span(s).start,"end":m.scoping.symbol_span(s).end},"writes":writes,"single_origin_native_class_guard_reached":false})
            );
            assert!(origin(m, s).is_none());
            assert!(single_native_origin(m, &n.callee, fs, 8).is_none());
        });
    }

    fn accepted(source: &str, expected: bool) {
        inspect(source, "$THREAD_5_131738", |m, f| {
            assert_eq!(prove_join(m, f).is_some(), expected);
        });
    }

    #[test]
    fn full_parent_control_and_alpha() {
        accepted(CONTROL, true);
        accepted(
            &CONTROL
                .replace("$KEY_38", "constructorCapture")
                .replace("$KEY_7", "indexCapture"),
            true,
        );
    }

    #[test]
    fn operational_and_readiness_mutations_refuse() {
        let start = CONTROL.find("function $THREAD_5_131738(").unwrap();
        let end = start
            + CONTROL[start..].find("return $THREAD_6_109885;").unwrap()
            + "return $THREAD_6_109885;".len();
        let own = &CONTROL[start..end];
        for (from, to) in [
            ("$KEY_3[$KEY_7] = $AN;", "$KEY_3[$AN] = $KEY_7;"),
            ("--$KEY_4;", "++$KEY_4;"),
            (
                "$KEY_1.next($KEY_3);",
                "$KEY_1.complete(); $KEY_1.next($KEY_3);",
            ),
            ("$AO => $AO.unsubscribe()", "$AO => $AO.unsubscribe?.()"),
            ("$KEY_1.error($AN);", "$KEY_1.error($AN); extra();"),
            ("$KEY_5 = [];", "$KEY_5 = []; synchronousSubscribe();"),
        ] {
            assert!(own.contains(from), "mutation anchor {from}");
            let changed = format!(
                "{}{}{}",
                &CONTROL[..start],
                own.replace(from, to),
                &CONTROL[end..]
            );
            accepted(&changed, false);
        }
        for replacement in [
            "$KEY_38 = $THREAD_1_28126(); $KEY_38 = other;",
            "$THREAD_1_28126(); $KEY_38 = $THREAD_1_28126();",
            "let earlyJoin = $THREAD_5_131738; earlyJoin([]); $KEY_38 = $THREAD_1_28126();",
            "$THREAD_5_131738([]); $KEY_38 = $THREAD_1_28126();",
        ] {
            assert!(CONTROL.contains("$KEY_38 = $THREAD_1_28126();"));
            accepted(
                &CONTROL.replacen("$KEY_38 = $THREAD_1_28126();", replacement, 1),
                false,
            );
        }
    }
}

fn id(m: &Model<'_, '_>, e: &Expression<'_>) -> Option<Sym> {
    crate::analysis::expression_symbol(e.without_parentheses(), m.scoping)
}
fn binding(p: &BindingPattern<'_>) -> Option<Sym> {
    match p {
        BindingPattern::BindingIdentifier(i) => i.symbol_id.get(),
        _ => None,
    }
}
fn fixed(m: &Model<'_, '_>, s: Sym) -> bool {
    m.binding_writes(s) == 0 && m.scoping.symbol_redeclarations(s).is_empty()
}
fn params(m: &Model<'_, '_>, f: &Function<'_>, n: usize) -> Option<Vec<Sym>> {
    if f.r#async || f.generator || f.params.rest.is_some() || f.params.items.len() != n {
        return None;
    }
    f.params
        .items
        .iter()
        .map(|p| {
            let s = binding(&p.pattern)?;
            (p.initializer.is_none() && fixed(m, s)).then_some(s)
        })
        .collect()
}
fn body<'a>(f: &'a Function<'a>) -> Option<&'a [Statement<'a>]> {
    let b = f.body.as_ref()?;
    (b.directives.is_empty() && !f.r#async && !f.generator).then_some(b.statements.as_slice())
}
fn expr<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    match s {
        Statement::ExpressionStatement(x) => Some(&x.expression),
        _ => None,
    }
}
fn ret<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    match s {
        Statement::ReturnStatement(x) => x.argument.as_ref(),
        _ => None,
    }
}
fn var<'a>(s: &'a Statement<'a>) -> Option<&'a VariableDeclarator<'a>> {
    let Statement::VariableDeclaration(d) = s else {
        return None;
    };
    let [v] = d.declarations.as_slice() else {
        return None;
    };
    Some(v)
}
fn alias(m: &Model<'_, '_>, s: &Statement<'_>, from: Sym) -> Option<Sym> {
    let v = var(s)?;
    let to = binding(&v.id)?;
    (fixed(m, to) && id(m, v.init.as_ref()?) == Some(from)).then_some(to)
}
fn function<'a>(s: &'a Statement<'a>) -> Option<&'a Function<'a>> {
    match s {
        Statement::FunctionDeclaration(f) => Some(f),
        _ => None,
    }
}
fn symbol(f: &Function<'_>) -> Option<Sym> {
    f.id.as_ref()?.symbol_id.get()
}
fn call<'a>(e: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    match e.without_parentheses() {
        Expression::CallExpression(c) if !c.optional => Some(c),
        _ => None,
    }
}
fn method<'a>(
    m: &Model<'_, '_>,
    e: &'a Expression<'a>,
    receiver: Sym,
    key: &str,
) -> Option<&'a [Argument<'a>]> {
    let c = call(e)?;
    let Expression::StaticMemberExpression(p) = c.callee.without_parentheses() else {
        return None;
    };
    (!p.optional && p.property.name == key && id(m, &p.object) == Some(receiver))
        .then_some(c.arguments.as_slice())
}
fn forwards(
    m: &Model<'_, '_>,
    f: &Function<'_>,
    sink: Sym,
    key: &str,
    value: bool,
    returns: bool,
) -> Option<()> {
    let ps = params(m, f, usize::from(value))?;
    let [s] = body(f)? else { return None };
    let args = method(m, if returns { ret(s)? } else { expr(s)? }, sink, key)?;
    if value {
        let [a] = args else { return None };
        (id(m, a.as_expression()?) == Some(ps[0])).then_some(())
    } else {
        args.is_empty().then_some(())
    }
}
fn install(m: &Model<'_, '_>, s: &Statement<'_>, object: Sym, key: &str, value: Sym) -> Option<()> {
    let Expression::AssignmentExpression(a) = expr(s)? else {
        return None;
    };
    let AssignmentTarget::StaticMemberExpression(p) = &a.left else {
        return None;
    };
    (a.operator.as_str() == "="
        && !p.optional
        && p.property.name == key
        && id(m, &p.object) == Some(object)
        && id(m, &a.right) == Some(value))
    .then_some(())
}

/// One direct-body initializer or assignment, with timing at each use. This
/// local resolver never changes the shared resolver or accepts parameters.
fn origin<'a>(m: &Model<'_, 'a>, s: Sym) -> Option<(&'a Expression<'a>, NodeId, u32)> {
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
    if let Some(e) = v.init.as_ref() {
        return (m.binding_writes(s) == 0).then_some((e, owner, v.span.end));
    }
    if m.binding_writes(s) != 1 {
        return None;
    }
    let r = m
        .scoping
        .get_resolved_references(s)
        .find(|r| r.is_write())?;
    let a_node = m.nodes.parent_id(r.node_id());
    let AstKind::AssignmentExpression(a) = m.nodes.kind(a_node) else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(i) = &a.left else {
        return None;
    };
    let statement = m.nodes.parent_id(a_node);
    (a.operator.as_str() == "="
        && crate::analysis::identifier_symbol(i, m.scoping) == Some(s)
        && matches!(m.nodes.kind(statement), AstKind::ExpressionStatement(_))
        && m.nodes.parent_id(statement) == owner
        && a.span.start > v.span.end)
        .then_some((&a.right, owner, a.span.end))
}
fn exposure_after(
    m: &Model<'_, '_>,
    s: Sym,
    owner: NodeId,
    end: u32,
    native: Span,
    depth: usize,
) -> bool {
    if depth == 0 || !fixed(m, s) {
        return false;
    }
    m.scoping.get_resolved_references(s).all(|r| {
        let node = r.node_id();
        // The complete native factory proves these calls occur only in its
        // returned class methods, never while constructing the class. Its sole
        // exposure is the constructor origin call checked below.
        if native.contains_inclusive(m.nodes.kind(node).span()) {
            return true;
        }
        if let AstKind::VariableDeclarator(v) = m.nodes.parent_kind(node) {
            if v.init
                .as_ref()
                .is_some_and(|e| e.span() == m.nodes.kind(node).span())
            {
                if let Some(next) = binding(&v.id) {
                    return exposure_after(m, next, owner, end, native, depth - 1)
                        && m.scoping.get_resolved_references(next).all(|u| {
                            native.contains_inclusive(m.nodes.kind(u.node_id()).span())
                                || callable_origin_precedes(
                                    m,
                                    owner,
                                    v.span.end,
                                    u.node_id(),
                                    None,
                                    0,
                                    true,
                                )
                        });
                }
            }
        }
        callable_origin_precedes(m, owner, end, node, None, 0, true)
    })
}
fn single_native_origin(
    m: &Model<'_, '_>,
    e: &Expression<'_>,
    factory: Sym,
    depth: usize,
) -> Option<()> {
    if depth == 0 {
        return None;
    }
    let s = id(m, e)?;
    let (value, owner, end) = origin(m, s)?;
    if let Expression::Identifier(i) = e.without_parentheses() {
        let use_node = m.scoping.get_reference(i.reference_id.get()?).node_id();
        // Check aliases at their own reads; the final captured read is checked
        // through every factory exposure instead of a declaration offset.
        if matches!(value.without_parentheses(), Expression::Identifier(_))
            && !callable_origin_precedes(m, owner, end, use_node, None, 0, true)
        {
            return None;
        }
    }
    if matches!(value.without_parentheses(), Expression::Identifier(_)) {
        return single_native_origin(m, value, factory, depth - 1);
    }
    let c = call(value)?;
    if !c.arguments.is_empty() {
        return None;
    }
    let native = stable_function(m, &c.callee, 0)?;
    observable_factory::proven_native_class(m, native)?;
    // Only this call can expose the proved class; no earlier discarded class
    // may activate a captured map operator against an unavailable constructor.
    let ns = native.binding_symbol?;
    let refs: Vec<_> = m.scoping.get_resolved_references(ns).collect();
    if refs.len() != 1 || m.nodes.kind(refs[0].node_id()).span() != c.callee.span() {
        return None;
    }
    exposure_after(m, factory, owner, end, native.span, 12).then_some(())
}

fn native_origin(m: &Model<'_, '_>, e: &Expression<'_>, factory: Sym, depth: usize) -> Option<()> {
    if depth == 0 {
        return None;
    }
    single_native_origin(m, e, factory, depth).or_else(|| epochs::prove(m, e, factory))
}

// Normalization consumes only direct local declarations and simple stores.
// Function declarations are hoisted but their bodies are checked separately;
// executable stores retain their original order. Deferred writers are never
// treated as initialization epochs.
struct Setup<'a> {
    stores: Vec<(Sym, &'a Expression<'a>)>,
    functions: Vec<&'a Function<'a>>,
    locals: BTreeSet<Sym>,
}
fn setup<'a>(m: &Model<'_, '_>, ss: &'a [Statement<'a>]) -> Option<Setup<'a>> {
    let mut out = Setup {
        stores: vec![],
        functions: vec![],
        locals: BTreeSet::new(),
    };
    for s in ss {
        match s {
            Statement::FunctionDeclaration(f) => {
                if !fixed(m, symbol(f)?) {
                    return None;
                }
                out.functions.push(f);
            }
            Statement::VariableDeclaration(d) if !d.declare => {
                for v in &d.declarations {
                    let sym = binding(&v.id)?;
                    if !m.scoping.symbol_redeclarations(sym).is_empty() || !out.locals.insert(sym) {
                        return None;
                    }
                    if let Some(e) = &v.init {
                        out.stores.push((sym, e));
                    }
                }
            }
            Statement::ExpressionStatement(s) => {
                let Expression::AssignmentExpression(a) = &s.expression else {
                    return None;
                };
                let AssignmentTarget::AssignmentTargetIdentifier(i) = &a.left else {
                    return None;
                };
                let sym = crate::analysis::identifier_symbol(i, m.scoping)?;
                if a.operator.as_str() != "=" || !out.locals.contains(&sym) {
                    return None;
                }
                out.stores.push((sym, &a.right));
            }
            _ => return None,
        }
    }
    Some(out)
}
fn empty_array(e: &Expression<'_>) -> Option<()> {
    matches!(e, Expression::ArrayExpression(a) if a.elements.is_empty()).then_some(())
}
fn number(e: &Expression<'_>, n: f64) -> Option<()> {
    matches!(e, Expression::NumericLiteral(v) if v.value == n).then_some(())
}
fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    match s {
        Statement::BlockStatement(b) => Some(&b.body),
        _ => None,
    }
}
fn member(m: &Model<'_, '_>, e: &Expression<'_>, receiver: Sym, key: &str) -> Option<()> {
    matches!(e, Expression::StaticMemberExpression(p) if !p.optional && p.property.name == key && id(m,&p.object)==Some(receiver)).then_some(())
}
fn arrow<'a>(
    e: &'a Expression<'a>,
    n: usize,
) -> Option<(&'a ArrowFunctionExpression<'a>, Vec<Sym>)> {
    let Expression::ArrowFunctionExpression(a) = e else {
        return None;
    };
    if a.r#async
        || a.params.rest.is_some()
        || a.params.items.len() != n
        || !a.body.directives.is_empty()
    {
        return None;
    }
    let ps = a
        .params
        .items
        .iter()
        .map(|p| {
            p.initializer
                .is_none()
                .then(|| binding(&p.pattern))
                .flatten()
        })
        .collect::<Option<Vec<_>>>()?;
    Some((a, ps))
}
fn expression_arrow<'a>(a: &'a ArrowFunctionExpression<'a>) -> Option<&'a Expression<'a>> {
    if !a.expression {
        return None;
    }
    let [s] = a.body.statements.as_slice() else {
        return None;
    };
    expr(s)
}
fn each_unsubscribe(m: &Model<'_, '_>, e: &Expression<'_>, subscriptions: Sym) -> Option<()> {
    let [arg] = method(m, e, subscriptions, "forEach")? else {
        return None;
    };
    let (a, ps) = arrow(arg.as_expression()?, 1)?;
    if !fixed(m, ps[0]) || !method(m, expression_arrow(a)?, ps[0], "unsubscribe")?.is_empty() {
        return None;
    }
    Some(())
}
fn decrement(m: &Model<'_, '_>, s: &Statement<'_>, count: Sym) -> Option<()> {
    matches!(expr(s)?, Expression::UpdateExpression(u) if u.prefix && u.operator.as_str()=="--" && matches!(&u.argument, SimpleAssignmentTarget::AssignmentTargetIdentifier(i) if crate::analysis::identifier_symbol(i,m.scoping)==Some(count))).then_some(())
}
fn increment(m: &Model<'_, '_>, s: &Statement<'_>, count: Sym) -> Option<()> {
    matches!(expr(s)?, Expression::UpdateExpression(u) if u.prefix && u.operator.as_str()=="++" && matches!(&u.argument, SimpleAssignmentTarget::AssignmentTargetIdentifier(i) if crate::analysis::identifier_symbol(i,m.scoping)==Some(count))).then_some(())
}
fn completion(
    m: &Model<'_, '_>,
    f: &Function<'_>,
    count: Sym,
    sink: Sym,
    values: Option<Sym>,
) -> Option<()> {
    params(m, f, 0)?;
    let [dec, branch] = body(f)? else {
        return None;
    };
    decrement(m, dec, count)?;
    let Statement::IfStatement(b) = branch else {
        return None;
    };
    if b.alternate.is_some() {
        return None;
    }
    let Expression::BinaryExpression(test) = &b.test else {
        return None;
    };
    if test.operator.as_str() != "<=" || id(m, &test.left) != Some(count) {
        return None;
    }
    number(&test.right, 0.)?;
    let ss = block(&b.consequent)?;
    let finish = if let Some(vals) = values {
        let [next, finish] = ss else {
            return None;
        };
        let [arg] = method(m, expr(next)?, sink, "next")? else {
            return None;
        };
        if id(m, arg.as_expression()?) != Some(vals) {
            return None;
        }
        finish
    } else {
        let [finish] = ss else {
            return None;
        };
        finish
    };
    method(m, expr(finish)?, sink, "complete")?
        .is_empty()
        .then_some(())
}
#[cfg(test)]
fn executor<'a>(
    m: &Model<'_, '_>,
    f: &'a Function<'a>,
    factory: Sym,
) -> Option<(&'a ArrowFunctionExpression<'a>, Sym, Sym)> {
    executor_with_origin(m, f, factory, false)
}
fn executor_with_origin<'a>(
    m: &Model<'_, '_>, f: &'a Function<'a>, factory: Sym, ordinary: bool,
) -> Option<(&'a ArrowFunctionExpression<'a>, Sym, Sym)> {
    let p = params(m, f, 1)?[0];
    let [capture, returned] = body(f)? else {
        return None;
    };
    let source = alias(m, capture, p)?;
    let Expression::NewExpression(n) = ret(returned)? else {
        return None;
    };
    if ordinary {
        observables::observable_map_operator::ordinary_origin(m, &n.callee, factory, 8)?;
    } else {
        native_origin(m, &n.callee, factory, 8)?;
    }
    let [arg] = n.arguments.as_slice() else {
        return None;
    };
    let (a, ps) = arrow(arg.as_expression()?, 1)?;
    if a.expression || !fixed(m, ps[0]) {
        return None;
    }
    Some((a, source, ps[0]))
}
fn local_reads_only(m: &Model<'_, '_>, s: Sym, span: Span) -> bool {
    m.scoping
        .get_resolved_references(s)
        .all(|r| span.contains_inclusive(m.nodes.kind(r.node_id()).span()))
}
fn fn_for<'a>(
    m: &Model<'_, '_>,
    setup: &Setup<'a>,
    e: &Expression<'_>,
) -> Option<&'a Function<'a>> {
    let sym = id(m, e)?;
    setup
        .functions
        .iter()
        .copied()
        .find(|f| symbol(f) == Some(sym))
}
fn one_arg(
    m: &Model<'_, '_>,
    e: &Expression<'_>,
    receiver: Sym,
    key: &str,
    arg: Sym,
) -> Option<()> {
    let [a] = method(m, e, receiver, key)? else {
        return None;
    };
    (id(m, a.as_expression()?) == Some(arg)).then_some(())
}
fn prove_merge(m: &Model<'_, '_>, f: &Function<'_>) -> Option<Vec<(Span, &'static str)>> {
    prove_merge_with_origin(m, f, false)
}
fn prove_merge_with_origin(m: &Model<'_, '_>, f: &Function<'_>, ordinary: bool) -> Option<Vec<(Span, &'static str)>> {
    // A named expression's private self symbol is not its external callable binding.
    if f.r#type != FunctionType::FunctionDeclaration {
        return None;
    }
    let projection = params(m, f, 1)?[0];
    let [capture, inner, returned] = body(f)? else {
        return None;
    };
    let project = alias(m, capture, projection)?;
    let op = function(inner)?;
    let os = symbol(op)?;
    let fs = symbol(f)?;
    if !fixed(m, fs)
        || !fixed(m, os)
        || id(m, ret(returned)?) != Some(os)
        || m.binding_references(os) != 1
    {
        return None;
    }
    let (exec, source, sink_param) = executor_with_origin(m, op, fs, ordinary)?;
    let ss = exec.body.statements.as_slice();
    let (returned, ss) = ss.split_last()?;
    let (cleanup, ss) = ss.split_last()?;
    let teardown = function(cleanup)?;
    let ts = symbol(teardown)?;
    if !fixed(m, ts) || id(m, ret(returned)?) != Some(ts) || m.binding_references(ts) != 1 {
        return None;
    }
    let (pushed, ss) = ss.split_last()?;
    let prep = setup(m, ss)?;
    let [(sink, sink_e), (subscriptions, subs_e), (count, count_e), (next, next_e), (error, error_e), (complete, complete_e), (subscription, subscribe_e)] =
        prep.stores.as_slice()
    else {
        return None;
    };
    if id(m, sink_e) != Some(sink_param) {
        return None;
    }
    empty_array(subs_e)?;
    number(count_e, 1.)?;
    let nf = fn_for(m, &prep, next_e)?;
    let ef = fn_for(m, &prep, error_e)?;
    let cf = fn_for(m, &prep, complete_e)?;
    if prep.functions.len() != 3 || prep.locals.len() != 7 {
        return None;
    }
    // All mutable slots are local to this executor. Every statement and nested
    // body below is consumed, so no independent writer can alter readiness.
    for s in prep.locals.iter() {
        if !local_reads_only(m, *s, exec.span) {
            return None;
        }
    }
    forwards(m, nf, *sink, "next", true, true)?;
    forwards(m, ef, *sink, "error", true, true)?;
    completion(m, cf, *count, *sink, None)?;
    for s in [*sink, *next, *error, *complete, *subscription] {
        let writes = m
            .scoping
            .get_resolved_references(s)
            .filter(|r| r.is_write())
            .count();
        // Initializers are not counted as writes by the semantic model. Their
        // exact producers and reads are already bound by the ordered setup.
        if writes > 1 {
            return None;
        }
    }
    let [on_next, on_error, on_complete] = method(m, subscribe_e, source, "subscribe")? else {
        return None;
    };
    if id(m, on_error.as_expression()?) != Some(*error)
        || id(m, on_complete.as_expression()?) != Some(*complete)
    {
        return None;
    }
    let (a, ps) = arrow(on_next.as_expression()?, 1)?;
    if a.expression || !fixed(m, ps[0]) {
        return None;
    }
    let [inc, sub, push] = a.body.statements.as_slice() else {
        return None;
    };
    increment(m, inc, *count)?;
    let v = var(sub)?;
    let inner_sub = binding(&v.id)?;
    if !fixed(m, inner_sub) {
        return None;
    }
    let c = call(v.init.as_ref()?)?;
    let Expression::StaticMemberExpression(p) = &c.callee else {
        return None;
    };
    if p.optional || p.property.name != "subscribe" {
        return None;
    }
    let project_call = call(&p.object)?;
    let [value] = project_call.arguments.as_slice() else {
        return None;
    };
    if id(m, &project_call.callee) != Some(project) || id(m, value.as_expression()?) != Some(ps[0])
    {
        return None;
    }
    let [n, e, c] = c.arguments.as_slice() else {
        return None;
    };
    if id(m, n.as_expression()?) != Some(*next)
        || id(m, e.as_expression()?) != Some(*error)
        || id(m, c.as_expression()?) != Some(*complete)
    {
        return None;
    }
    one_arg(m, expr(push)?, *subscriptions, "push", inner_sub)?;
    one_arg(m, expr(pushed)?, *subscriptions, "push", *subscription)?;
    params(m, teardown, 0)?;
    let [dispose] = body(teardown)? else {
        return None;
    };
    each_unsubscribe(m, ret(dispose)?, *subscriptions)?;
    if [nf, ef, cf]
        .iter()
        .any(|f| m.binding_references(symbol(f).unwrap()) != 1)
    {
        return None;
    }
    Some(vec![
        (f.span, "createMergeMapOperator"),
        (op.span, "mergeMapOperatorFunction"),
        (cf.span, "completeMergeMapSource"),
    ])
}

pub(in crate::naming) fn evaluate(
    m: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let mut candidate = Some(site);
    for _ in 0..6 {
        let Some(owner) = candidate else {
            break;
        };
        if let AstKind::Function(f) = m.nodes.kind(owner.node_id) {
            if owner.span == site.span && prove_entropy(m, f).is_some() {
                return vec![proposal("deriveSignalCodecSeed","encoding_entropy_seed_no_counter","Entire zero-capture no-counter entropy seed body: exact random uint32, optional performance and crypto samples with empty catches, Number(Date), NaN guards with 1103515245 fallback, neutral self stores and unsigned XOR return; ambient sources unshadowed",facts)];
            }
            if let Some(roles) = prove_join(m, f) {
                if let Some((_, name)) = roles.iter().find(|(span, _)| *span == site.span) {
                    return vec![proposal(name,"shared_observable_fork_join","Whole join protocol: native constructor origin and exposure readiness; index-preserving last-value stores, countdown and empty-input completion, error-before-disposal, initially-empty subscription list during synchronous map reentry; all own callbacks and SymbolIds consumed",facts)];
                }
            }
            if let Some(roles) = prove_merge(m, f) {
                if let Some((_, name)) = roles.iter().find(|(span, _)| *span == site.span) {
                    return vec![proposal(name,"shared_observable_merge_map_operator","Whole merge-map subscription protocol: native constructor origin and exposure readiness; exact ordered local initialization, increment-before-subscribe, next/error forwarding, completion countdown and unsubscribe-all teardown; every callback body and SymbolId link consumed",facts)];
                }
            }
            for roles in [prove_join_with_origin(m, f, true), prove_merge_with_origin(m, f, true)].into_iter().flatten() {
                if let Some((_, name)) = roles.iter().find(|(span, _)| *span == site.span) {
                    return vec![proposal(name, "shared_subscriber_stream_join_merge", "Whole owned join or merge subscription protocol; independently complete ordinary strict subscriber constructor API with closed object reads and executed initialization at every exposure; exact source indices, ordered next/error/completion, countdown and teardown; no native class identity claimed", facts)];
                }
            }
        }
        candidate = m.enclosing_function(owner.node_id);
    }
    Vec::new()
}

fn prove_join(m: &Model<'_, '_>, f: &Function<'_>) -> Option<Vec<(Span, &'static str)>> {
    prove_join_with_origin(m, f, false)
}
fn prove_join_with_origin(m: &Model<'_, '_>, f: &Function<'_>, ordinary: bool) -> Option<Vec<(Span, &'static str)>> {
    // A named expression's private self symbol is not its external callable binding.
    if f.r#type != FunctionType::FunctionDeclaration {
        return None;
    }
    let fs = symbol(f)?;
    if !fixed(m, fs) {
        return None;
    }
    let (exec, source, sink_param) = executor_with_origin(m, f, fs, ordinary)?;
    let (returned, ss) = exec.body.statements.split_last()?;
    let (cleanup, ss) = ss.split_last()?;
    let (branch, ss) = ss.split_last()?;
    let teardown = function(cleanup)?;
    params(m, teardown, 0)?;
    let ts = symbol(teardown)?;
    if !fixed(m, ts) || m.binding_references(ts) != 1 || id(m, ret(returned)?) != Some(ts) {
        return None;
    }
    let prep = setup(m, ss)?;
    let [(sink, sink_e), (values, values_e), (count, count_e), (subscriptions, subs_e)] =
        prep.stores.as_slice()
    else {
        return None;
    };
    if prep.locals.len() != 4 || !prep.functions.is_empty() || id(m, sink_e) != Some(sink_param) {
        return None;
    }
    empty_array(values_e)?;
    empty_array(subs_e)?;
    member(m, count_e, source, "length")?;
    for s in &prep.locals {
        if !local_reads_only(m, *s, exec.span) {
            return None;
        }
    }
    let Statement::IfStatement(b) = branch else {
        return None;
    };
    let Expression::BinaryExpression(test) = &b.test else {
        return None;
    };
    if test.operator.as_str() != "<=" || id(m, &test.left) != Some(*count) {
        return None;
    }
    number(&test.right, 0.)?;
    let [next, complete] = block(&b.consequent)? else {
        return None;
    };
    one_arg(m, expr(next)?, *sink, "next", *values)?;
    if !method(m, expr(complete)?, *sink, "complete")?.is_empty() {
        return None;
    }
    let [map] = block(b.alternate.as_ref()?)? else {
        return None;
    };
    let Expression::AssignmentExpression(assign) = expr(map)? else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &assign.left else {
        return None;
    };
    if assign.operator.as_str() != "="
        || crate::analysis::identifier_symbol(target, m.scoping) != Some(*subscriptions)
    {
        return None;
    }
    let [callback] = method(m, &assign.right, source, "map")? else {
        return None;
    };
    let (a, ps) = arrow(callback.as_expression()?, 2)?;
    if a.expression || ps.iter().any(|s| !fixed(m, *s)) {
        return None;
    }
    let [error, index, next, observer, complete, install_complete, install_error, subscribe] =
        a.body.statements.as_slice()
    else {
        return None;
    };
    let ef = function(error)?;
    let nf = function(next)?;
    let cf = function(complete)?;
    let input = params(m, ef, 1)?[0];
    let [forward, dispose] = body(ef)? else {
        return None;
    };
    one_arg(m, expr(forward)?, *sink, "error", input)?;
    each_unsubscribe(m, expr(dispose)?, *subscriptions)?;
    let index = alias(m, index, ps[1])?;
    let np = params(m, nf, 1)?[0];
    let [store] = body(nf)? else {
        return None;
    };
    let Expression::AssignmentExpression(store) = expr(store)? else {
        return None;
    };
    let AssignmentTarget::ComputedMemberExpression(target) = &store.left else {
        return None;
    };
    if store.operator.as_str() != "="
        || target.optional
        || id(m, &target.object) != Some(*values)
        || id(m, &target.expression) != Some(index)
        || id(m, &store.right) != Some(np)
    {
        return None;
    }
    completion(m, cf, *count, *sink, Some(*values))?;
    let v = var(observer)?;
    let obs = binding(&v.id)?;
    if !fixed(m, obs) {
        return None;
    }
    let Expression::ObjectExpression(o) = v.init.as_ref()? else {
        return None;
    };
    let [ObjectPropertyKind::ObjectProperty(p)] = o.properties.as_slice() else {
        return None;
    };
    if p.computed
        || p.method
        || p.shorthand
        || p.kind != PropertyKind::Init
        || property_key_name(&p.key).as_deref() != Some("next")
        || id(m, &p.value) != symbol(nf)
    {
        return None;
    }
    install(m, install_complete, obs, "complete", symbol(cf)?)?;
    install(m, install_error, obs, "error", symbol(ef)?)?;
    one_arg(m, ret(subscribe)?, ps[0], "subscribe", obs)?;
    let [dispose] = body(teardown)? else {
        return None;
    };
    each_unsubscribe(m, expr(dispose)?, *subscriptions)?;
    if [nf, ef, cf]
        .iter()
        .any(|f| !fixed(m, symbol(f).unwrap()) || m.binding_references(symbol(f).unwrap()) != 1)
    {
        return None;
    }
    // No assumption that map/subscribe are asynchronous: subscriptions is []
    // until map returns, while values/count/index/sink are initialized before
    // the observer is exposed. The exact reentrant behavior is retained.
    Some(vec![
        (f.span, "joinObservableLastValues"),
        (nf.span, "storeJoinedObservableValue"),
        (ef.span, "errorAndUnsubscribeJoinedSources"),
        (cf.span, "completeJoinedObservableSource"),
        (teardown.span, "unsubscribeJoinedSources"),
    ])
}

// Unlike the counter-bearing entropy rule, this exact variant has no captures.
// Numeric values, execution order and both swallowing catches are consumed.
fn ambient(m: &Model<'_, '_>, e: &Expression<'_>, name: &str) -> Option<()> {
    crate::naming::functions::browser::proxy_probe::ambient(m, e, name).then_some(())
}
fn binary<'a>(e: &'a Expression<'a>, op: &str) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::BinaryExpression(b) = e else {
        return None;
    };
    (b.operator.as_str() == op).then_some((&b.left, &b.right))
}
fn uint32<'a>(e: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    let (x, n) = binary(e, ">>>")?;
    number(n, 0.)?;
    Some(x)
}
fn global_method<'a>(
    m: &Model<'_, '_>,
    e: &'a Expression<'a>,
    global: &str,
    key: &str,
) -> Option<&'a [Argument<'a>]> {
    let c = call(e)?;
    let Expression::StaticMemberExpression(p) = &c.callee else {
        return None;
    };
    if p.optional || p.property.name != key {
        return None;
    }
    ambient(m, &p.object, global)?;
    Some(&c.arguments)
}
fn global_call<'a>(
    m: &Model<'_, '_>,
    e: &'a Expression<'a>,
    global: &str,
) -> Option<&'a [Argument<'a>]> {
    let c = call(e)?;
    ambient(m, &c.callee, global)?;
    Some(&c.arguments)
}
fn new_global<'a>(
    m: &Model<'_, '_>,
    e: &'a Expression<'a>,
    global: &str,
) -> Option<&'a [Argument<'a>]> {
    let Expression::NewExpression(n) = e else {
        return None;
    };
    ambient(m, &n.callee, global)?;
    Some(&n.arguments)
}
fn assigned<'a>(
    m: &Model<'_, '_>,
    s: &'a Statement<'a>,
    target: Sym,
) -> Option<&'a Expression<'a>> {
    let Expression::AssignmentExpression(a) = expr(s)? else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(i) = &a.left else {
        return None;
    };
    (a.operator.as_str() == "=" && crate::analysis::identifier_symbol(i, m.scoping) == Some(target))
        .then_some(&a.right)
}
fn swallowing_store<'a>(
    m: &Model<'_, '_>,
    s: &'a Statement<'a>,
    target: Sym,
) -> Option<&'a Expression<'a>> {
    let Statement::TryStatement(t) = s else {
        return None;
    };
    let h = t.handler.as_ref()?;
    if t.finalizer.is_some() || !h.body.body.is_empty() {
        return None;
    }
    // Catch destructuring/default semantics could have effects, even with an
    // empty handler. Only an ordinary unused catch binding is accepted.
    let p = h.param.as_ref()?;
    let caught = binding(&p.pattern)?;
    if !fixed(m, caught) || m.binding_references(caught) != 0 {
        return None;
    }
    let [store] = t.block.body.as_slice() else {
        return None;
    };
    assigned(m, store, target)
}
fn self_equal(m: &Model<'_, '_>, e: &Expression<'_>, s: Sym) -> Option<()> {
    let (a, b) = binary(e, "===")?;
    (id(m, a) == Some(s) && id(m, b) == Some(s)).then_some(())
}
fn xor(m: &Model<'_, '_>, e: &Expression<'_>, left: Sym, right: Sym) -> Option<()> {
    let (a, b) = binary(e, "^")?;
    (id(m, a) == Some(left) && id(m, b) == Some(right)).then_some(())
}
fn nan_merge(m: &Model<'_, '_>, e: &Expression<'_>, prior: Sym, value: Sym) -> Option<()> {
    let Expression::ConditionalExpression(c) = e else {
        return None;
    };
    self_equal(m, &c.test, value)?;
    xor(m, &c.consequent, prior, value)?;
    (id(m, &c.alternate) == Some(prior)).then_some(())
}
fn prove_entropy(m: &Model<'_, '_>, f: &Function<'_>) -> Option<()> {
    params(m, f, 0)?;
    let [random, clock, clock_try, date, crypto, crypto_try, fold, neutral_clock, merge_clock, merge_date, neutral_crypto, returned] =
        body(f)?
    else {
        return None;
    };
    let rv = var(random)?;
    let random = binding(&rv.id)?;
    let (sample, scale) = binary(uint32(rv.init.as_ref()?)?, "*")?;
    number(scale, 4294967296.)?;
    if !global_method(m, sample, "Math", "random")?.is_empty() {
        return None;
    }
    let cv = var(clock)?;
    let clock = binding(&cv.id)?;
    number(cv.init.as_ref()?, 0.)?;
    if !global_method(
        m,
        uint32(swallowing_store(m, clock_try, clock)?)?,
        "performance",
        "now",
    )?
    .is_empty()
    {
        return None;
    }
    let dv = var(date)?;
    let date = binding(&dv.id)?;
    let [arg] = global_call(m, uint32(dv.init.as_ref()?)?, "Number")? else {
        return None;
    };
    if !new_global(m, arg.as_expression()?, "Date")?.is_empty() {
        return None;
    }
    let kv = var(crypto)?;
    let crypto = binding(&kv.id)?;
    number(kv.init.as_ref()?, 0.)?;
    let Expression::ComputedMemberExpression(indexed) = swallowing_store(m, crypto_try, crypto)?
    else {
        return None;
    };
    if indexed.optional {
        return None;
    }
    number(&indexed.expression, 0.)?;
    let [arg] = global_method(m, &indexed.object, "crypto", "getRandomValues")? else {
        return None;
    };
    let [length] = new_global(m, arg.as_expression()?, "Uint32Array")? else {
        return None;
    };
    number(length.as_expression()?, 1.)?;
    let Statement::IfStatement(fold) = fold else {
        return None;
    };
    self_equal(m, &fold.test, random)?;
    let [yes] = block(&fold.consequent)? else {
        return None;
    };
    let [no] = block(fold.alternate.as_ref()?)? else {
        return None;
    };
    let (a, b) = binary(assigned(m, yes, random)?, "^")?;
    if id(m, a) != Some(random) {
        return None;
    }
    number(b, 1103515245.)?;
    number(assigned(m, no, random)?, 1103515245.)?;
    if id(m, assigned(m, neutral_clock, clock)?) != Some(clock)
        || id(m, assigned(m, neutral_crypto, crypto)?) != Some(crypto)
    {
        return None;
    }
    nan_merge(m, assigned(m, merge_clock, clock)?, random, clock)?;
    nan_merge(m, assigned(m, merge_date, date)?, clock, date)?;
    xor(m, uint32(ret(returned)?)?, date, crypto)?;
    let locals = [random, clock, date, crypto];
    if locals.into_iter().collect::<BTreeSet<_>>().len() != 4
        || locals.iter().any(|s| {
            !m.scoping.symbol_redeclarations(*s).is_empty() || !local_reads_only(m, *s, f.span)
        })
    {
        return None;
    }
    Some(())
}

#[cfg(test)]
mod actual_join_composition {
    use super::joined_stream_state_and_cleanup::{inspect, ACTUAL};
    use super::*;
    const JOIN: &str = "$THREAD_3_64859";
    const FIRST: &str = "$KEY_225 = $THREAD_5_45035();";
    const LAST: &str = "$KEY_225 = $THREAD_5_45035_3();";

    fn check(source: &str, expected: bool) {
        inspect(source, JOIN, |m, f| {
            assert_eq!(
                prove_join(m, f).is_some(),
                expected,
                "full actual join parent"
            )
        });
    }
    fn own(source: &str, name: &str, from: &str, to: &str) -> String {
        let mut result = None;
        inspect(source, name, |_, f| {
            let text = &source[f.span.start as usize..f.span.end as usize];
            assert_eq!(
                text.matches(from).count(),
                1,
                "mutation anchor {name}: {from}"
            );
            result = Some(format!(
                "{}{}{}",
                &source[..f.span.start as usize],
                text.replacen(from, to, 1),
                &source[f.span.end as usize..]
            ));
        });
        result.unwrap()
    }
    fn replace(source: &str, from: &str, to: &str) -> String {
        assert_eq!(source.matches(from).count(), 1, "mutation anchor {from}");
        source.replacen(from, to, 1)
    }
    #[test]
    fn joined_stream_state_and_cleanup_actual_all_three_producers_and_whole_parent() {
        inspect(ACTUAL, JOIN, |m, f| {
            let factory = symbol(f).unwrap();
            let [capture, returned] = body(f).unwrap() else {
                panic!("join body")
            };
            let formal = params(m, f, 1).unwrap()[0];
            alias(m, capture, formal).unwrap();
            let Expression::NewExpression(n) = ret(returned).unwrap() else {
                panic!("constructor")
            };
            let constructor = id(m, &n.callee).unwrap();
            assert_eq!(m.binding_writes(constructor), 3);
            assert!(single_native_origin(m, &n.callee, factory, 8).is_none());
            let epochs = epochs::collect(m, constructor)
                .expect("every writer and closed constructor read");
            assert_eq!(epochs.stores.len(), 3);
            assert_eq!(epochs.makers.len(), 3);
            let expected = [
                (149641, 154847, 149678, 154777),
                (582097, 586963, 582134, 586899),
                (993607, 998302, 993643, 998241),
            ];
            let witnesses:Vec<_>=epochs.makers.iter().zip(expected).map(|(maker,(start,end,cs,ce))| {
                assert_eq!(maker.span,Span::new(start,end));
                let (class_span,class_symbol)=observable_factory::proven_native_class(m,*maker).expect("independent full actual source-class witness");
                assert_eq!(class_span,Span::new(cs,ce));
                serde_json::json!({"factory":{"start":start,"end":end},"class":{"start":cs,"end":ce},"class_symbol":format!("{class_symbol:?}"),"complete_source_class":true,"host_native_provenance":false})
            }).collect();
            let ready = epochs::publications_ready(m, factory, &epochs);
            assert!(ready);
            epochs::prove(m, &n.callee, factory)
                .expect("all epochs, producers and publications");
            native_origin(m, &n.callee, factory, 8).expect("production origin entry point");
            let (exec, source, sink) =
                executor(m, f, factory).expect("exact executor and captured source");
            let prefix = &exec.body.statements[..exec.body.statements.len() - 3];
            let prep = setup(m, prefix).expect("executed state prefix");
            assert_eq!(prep.locals.len(), 4);
            assert_eq!(prep.stores.len(), 4);
            assert_eq!(id(m, prep.stores[0].1), Some(sink));
            empty_array(prep.stores[1].1).unwrap();
            member(m, prep.stores[2].1, source, "length").unwrap();
            empty_array(prep.stores[3].1).unwrap();
            let roles = prove_join(m, f).expect("complete own parent and EVERY callback");
            assert_eq!(
                roles.iter().map(|(_, n)| *n).collect::<Vec<_>>(),
                [
                    "joinObservableLastValues",
                    "storeJoinedObservableValue",
                    "errorAndUnsubscribeJoinedSources",
                    "completeJoinedObservableSource",
                    "unsubscribeJoinedSources"
                ]
            );
            assert_eq!(
                roles
                    .iter()
                    .map(|(s, _)| (s.start, s.end))
                    .collect::<Vec<_>>(),
                [
                    (23802, 24849),
                    (24295, 24368),
                    (24140, 24260),
                    (24423, 24580),
                    (24719, 24804)
                ]
            );
            println!(
                "{}",
                serde_json::json!({"actual_join":{"start":f.span.start,"end":f.span.end},"producers":witnesses,"executed_publication_readiness":ready,"exact_executor":true,"all_state_stores_before_publication":true,"complete_parent_and_all_callbacks":true,"roles":roles.iter().map(|(_,n)|*n).collect::<Vec<_>>() })
            );
        });
        let result = crate::naming::name_threads(ACTUAL, true).unwrap();
        for (name, role) in [
            (JOIN, "joinObservableLastValues"),
            ("$THREAD_3_141338", "storeJoinedObservableValue"),
            ("$THREAD_1_50963", "errorAndUnsubscribeJoinedSources"),
            ("$THREAD_3_108768", "completeJoinedObservableSource"),
            ("$THREAD_7_88354", "unsubscribeJoinedSources"),
        ] {
            let found: Vec<_> = result
                .report
                .outcomes
                .iter()
                .filter(|o| o.binding.name == name)
                .collect();
            assert_eq!(found.len(), 1);
            let o = found[0];
            assert!(o.applied, "{o:?}");
            assert_eq!(o.role_name.as_deref(), Some(role));
            assert_eq!(o.evaluator.as_deref(), Some("shared_observable_fork_join"));
            assert_eq!(o.reference_edits, 1);
        }
    }
    #[test]
    fn joined_stream_state_and_cleanup_actual_alpha_and_explicit_initializers() {
        check(ACTUAL, true);
        let alpha = ACTUAL
            .replace("$KEY_225", "constructorEpoch")
            .replace("$KEY_167", "joinAlias")
            .replace("$KEY_9", "joinedIndex");
        check(&alpha, true);
        let explicit = own(ACTUAL, JOIN, "let $KEY_1 = [];", "let $KEY_1; $KEY_1 = [];");
        check(&explicit, true);
        let result = crate::naming::name_threads(&alpha, true).unwrap();
        let parent = result
            .report
            .outcomes
            .iter()
            .find(|o| o.binding.name == JOIN)
            .unwrap();
        assert!(parent.applied);
        assert_eq!(
            parent.role_name.as_deref(),
            Some("joinObservableLastValues")
        );
    }
    #[test]
    fn joined_stream_state_and_cleanup_actual_origin_and_publication_refusals() {
        for changed in [
            replace(ACTUAL,LAST,"$KEY_225 = foreignMaker();"),
            replace(ACTUAL,LAST,"function deferredOrigin() { $KEY_225 = $THREAD_5_45035_3(); } deferredOrigin();"),
            replace(ACTUAL,FIRST,"if (ready) { $KEY_225 = $THREAD_5_45035(); }"),
            replace(ACTUAL,FIRST,"$THREAD_3_64859([]); $KEY_225 = $THREAD_5_45035();"),
            replace(ACTUAL,FIRST,"let earlyClass = $KEY_225; $KEY_225 = $THREAD_5_45035();"),
            replace(ACTUAL,FIRST,"$KEY_225 = $THREAD_5_45035(); publish($KEY_225);"),
            replace(ACTUAL,FIRST,"publish($THREAD_3_64859); $KEY_225 = $THREAD_5_45035();"),
            replace(ACTUAL,FIRST,"let earlyJoin = $THREAD_3_64859; earlyJoin([]); $KEY_225 = $THREAD_5_45035();"),
            replace(ACTUAL,FIRST,"try { throw 0; } catch (e) { $THREAD_3_64859([]); } $KEY_225 = $THREAD_5_45035();"),
            replace(ACTUAL,FIRST,"$KEY_225 = $THREAD_5_45035(); async function asyncPublication() { return $THREAD_3_64859([]); } asyncPublication();"),
            replace(ACTUAL,FIRST,"$THREAD_5_45035(); $KEY_225 = $THREAD_5_45035();"),
            own(ACTUAL,"$THREAD_5_45035","static empty()","static missingEmpty()"),
            own(ACTUAL,JOIN,"let $KEY_0 = $DF;","let $KEY_0 = foreignSource;"),
        ] {check(&changed,false);}
    }
    #[test]
    fn joined_stream_state_and_cleanup_actual_callbacks_and_initialization_refusals() {
        for (from, to) in [
            ("let $KEY_1 = [];", "let $KEY_1;"),
            (
                "let $KEY_2 = [];",
                "$KEY_0.map(item => item.subscribe({})); let $KEY_2 = [];",
            ),
            (
                "let $KEY_3 = $KEY_0.length;",
                "let $KEY_3 = foreignSource.length;",
            ),
            ("let $KEY_9 = $DI;", "let $KEY_9 = $DH;"),
            ("$KEY_2[$KEY_9] = $DK;", "$KEY_2[$DK] = $KEY_9;"),
            ("$KEY_2[$KEY_9] = $DK;", "foreignValues[$KEY_9] = $DK;"),
            ("$KEY_4.error($DK);", "foreignSink.error($DK);"),
            ("$KEY_4.error($DK);", "$KEY_4.next($DK);"),
            ("--$KEY_3;", "++$KEY_3;"),
            ("$KEY_3 <= 0", "$KEY_3 < 0"),
            (
                "$KEY_1.forEach($DH => $DH.unsubscribe());",
                "$KEY_1.forEach($DH => foreignHandle.unsubscribe());",
            ),
        ] {
            // The <= test appears twice (empty input and source completion).
            let changed = if from == "$KEY_3 <= 0" {
                own(ACTUAL, "$THREAD_3_108768", from, to)
            } else {
                own(ACTUAL, JOIN, from, to)
            };
            check(&changed, false);
        }
        for (from, to) in [
            (
                "$KEY_4.next($KEY_2);",
                "$KEY_4.complete(); $KEY_4.next($KEY_2);",
            ),
            ("$KEY_4.complete();", "foreignSink.complete();"),
            ("$KEY_4.complete();", "$KEY_4.complete(); extra();"),
        ] {
            check(&own(ACTUAL, "$THREAD_3_108768", from, to), false);
        }
        check(
            &own(
                ACTUAL,
                "$THREAD_7_88354",
                "$KEY_1.forEach($DH => $DH.unsubscribe());",
                "extra();",
            ),
            false,
        );
        check(
            &own(
                ACTUAL,
                "$THREAD_1_50963",
                "$KEY_4.error($DK);",
                "$KEY_4.error($DK); extra();",
            ),
            false,
        );
    }
}
