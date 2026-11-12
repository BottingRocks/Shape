//! Bounded map-only protocol. Each declaration is consumed in its exact role;
//! no sibling body or inherited finder label supplies evidence.
use super::*;
use oxc_semantic::{NodeId, SymbolId};
use oxc_span::GetSpan;
type Sym = SymbolId;

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
pub(in crate::naming) fn origin<'a>(
    m: &Model<'_, 'a>,
    s: Sym,
) -> Option<(&'a Expression<'a>, NodeId, u32)> {
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
pub(in crate::naming) fn native_origin(
    m: &Model<'_, '_>,
    e: &Expression<'_>,
    factory: Sym,
    depth: usize,
) -> Option<()> {
    native_origin_witness(m, e, factory, depth, false)
        .map(|_| ())
        .or_else(|| {
            (depth > 0)
                .then(|| crate::naming::functions::observables::joined_observables::constructor_epochs(m, e, factory))
                .flatten()
        })
}

// Propagate the complete native class witness through every alias. The class
// methods are deferred by that own proof; all external factory exposures must
// follow each alias's executed store, including the final constructor alias.
fn native_origin_witness(
    m: &Model<'_, '_>,
    e: &Expression<'_>,
    factory: Sym,
    depth: usize,
    canonical: bool,
) -> Option<Span> {
    origin_witness(m, e, factory, depth, canonical, false)
}

pub(in crate::naming) fn ordinary_origin(
    m: &Model<'_, '_>, e: &Expression<'_>, factory: Sym, depth: usize,
) -> Option<()> {
    crate::naming::functions::observables::subscription_operators::closed_constructor(m, id(m, e)?, &mut BTreeSet::new(), 32)?;
    origin_witness(m, e, factory, depth, false, true).map(|_| ())
}

fn origin_witness(
    m: &Model<'_, '_>, e: &Expression<'_>, factory: Sym, depth: usize,
    canonical: bool, ordinary: bool,
) -> Option<Span> {
    if depth == 0 {
        return None;
    }
    let s = id(m, e)?;
    let (value, owner, end) = origin(m, s)?;
    if matches!(value.without_parentheses(), Expression::Identifier(_)) {
        // This RHS is an executed value copy, not invocation of a hoisted
        // function. Its producer must already hold the constructor value at
        // the copy itself; later exposures cannot repair an early read.
        let (_, producer_owner, producer_end) = origin(m, id(m, value)?)?;
        if producer_owner != owner || producer_end > value.span().start {
            return None;
        }
        let native = origin_witness(m, value, factory, depth - 1, canonical, ordinary)?;
        return exposure_after(m, factory, owner, end, native, 12).then_some(native);
    }
    let c = call(value)?;
    if !c.arguments.is_empty() {
        return None;
    }
    let native = stable_function(m, &c.callee, 0)?;
    if !matches!(m.nodes.kind(native.node_id), AstKind::Function(f) if f.r#type == FunctionType::FunctionDeclaration)
        || !matches!(
            m.nodes.parent_kind(native.node_id),
            AstKind::FunctionBody(_) | AstKind::Program(_)
        )
    {
        return None;
    }
    if ordinary {
        observable_factory::proven_ordinary_constructor(m, native)?;
    } else if canonical {
        observable_factory::proven_canonical_native_class(m, native)?;
    } else {
        observable_factory::proven_native_class(m, native)?;
    }
    // Only this call can expose the proved class; no earlier discarded class
    // may activate a captured map operator against an unavailable constructor.
    let ns = native.binding_symbol?;
    let refs: Vec<_> = m.scoping.get_resolved_references(ns).collect();
    if refs.len() != 1 || m.nodes.kind(refs[0].node_id()).span() != c.callee.span() {
        return None;
    }
    exposure_after(m, factory, owner, end, native.span, 12).then_some(native.span)
}

#[cfg(test)]
fn prove(
    m: &Model<'_, '_>,
    f: &Function<'_>,
    canonical: bool,
) -> Option<(Vec<(Span, &'static str)>, Span)> {
    prove_with_origin(m, f, canonical, false)
}

fn prove_with_origin(
    m: &Model<'_, '_>, f: &Function<'_>, canonical: bool, ordinary: bool,
) -> Option<(Vec<(Span, &'static str)>, Span)> {
    // A named expression's private self symbol is not its external callable binding.
    if f.r#type != FunctionType::FunctionDeclaration {
        return None;
    }
    let transform = params(m, f, 1)?[0];
    let [capture, inner, returned] = body(f)? else {
        return None;
    };
    let projection = alias(m, capture, transform)?;
    let op = function(inner)?;
    let source = params(m, op, 1)?[0];
    let fs = symbol(f)?;
    let os = symbol(op)?;
    if !fixed(m, fs) || !fixed(m, os) || id(m, ret(returned)?) != Some(os) {
        return None;
    }
    let [capture, new_result] = body(op)? else {
        return None;
    };
    let input = alias(m, capture, source)?;
    let Expression::NewExpression(new) = ret(new_result)?.without_parentheses() else {
        return None;
    };
    if ordinary {
        ordinary_origin(m, &new.callee, fs, 8)?;
    } else if canonical {
        native_origin_witness(m, &new.callee, fs, 8, true)?;
    } else {
        native_origin(m, &new.callee, fs, 8)?;
    }
    let [arg] = new.arguments.as_slice() else {
        return None;
    };
    let Expression::ArrowFunctionExpression(executor) = arg.as_expression()? else {
        return None;
    };
    if executor.r#async
        || executor.expression
        || !executor.body.directives.is_empty()
        || executor.params.rest.is_some()
    {
        return None;
    }
    let [p] = executor.params.items.as_slice() else {
        return None;
    };
    let receiver = binding(&p.pattern)?;
    if p.initializer.is_some() || !fixed(m, receiver) {
        return None;
    }
    // Exactly two recovered layouts: subscription initialized at its use, or
    // declared before the handlers and assigned after complete installation.
    let b = executor.body.statements.as_slice();
    let (
        complete,
        capture,
        pending,
        next,
        observer,
        error,
        error_set,
        complete_set,
        subscribe,
        cleanup,
        finish,
    ) = match b {
        [a, b, c, d, e, f, g, h, i, j] => (a, b, None, c, d, e, f, g, h, i, j),
        [a, b, c, d, e, f, g, h, i, j, k] => (a, b, Some(c), d, e, f, g, h, i, j, k),
        _ => return None,
    };
    let sink = alias(m, capture, receiver)?;
    let cf = function(complete)?;
    let nf = function(next)?;
    let ef = function(error)?;
    let tf = function(cleanup)?;
    let cs = symbol(cf)?;
    let ns = symbol(nf)?;
    let es = symbol(ef)?;
    let ts = symbol(tf)?;
    if [cs, ns, es, ts].iter().any(|s| !fixed(m, *s)) {
        return None;
    }
    forwards(m, cf, sink, "complete", false, false)?;
    forwards(m, ef, sink, "error", true, false)?;
    let value = params(m, nf, 1)?[0];
    let [Statement::TryStatement(t)] = body(nf)? else {
        return None;
    };
    if t.finalizer.is_some() {
        return None;
    }
    let [emit] = t.block.body.as_slice() else {
        return None;
    };
    let [projected] = method(m, expr(emit)?, sink, "next")? else {
        return None;
    };
    let pc = call(projected.as_expression()?)?;
    let [pa] = pc.arguments.as_slice() else {
        return None;
    };
    if id(m, &pc.callee) != Some(projection) || id(m, pa.as_expression()?) != Some(value) {
        return None;
    }
    let caught = t.handler.as_ref()?;
    let caught_symbol = binding(&caught.param.as_ref()?.pattern)?;
    if !fixed(m, caught_symbol) {
        return None;
    }
    let [forward] = caught.body.body.as_slice() else {
        return None;
    };
    let [error_arg] = method(m, expr(forward)?, sink, "error")? else {
        return None;
    };
    if id(m, error_arg.as_expression()?) != Some(caught_symbol) {
        return None;
    }
    let ov = var(observer)?;
    let observer_symbol = binding(&ov.id)?;
    if !fixed(m, observer_symbol) {
        return None;
    }
    let Expression::ObjectExpression(object) = ov.init.as_ref()? else {
        return None;
    };
    let [ObjectPropertyKind::ObjectProperty(property)] = object.properties.as_slice() else {
        return None;
    };
    if property.computed
        || property.method
        || property.shorthand
        || property.kind != PropertyKind::Init
        || property_key_name(&property.key).as_deref() != Some("next")
        || id(m, &property.value) != Some(ns)
    {
        return None;
    }
    install(m, error_set, observer_symbol, "error", es)?;
    install(m, complete_set, observer_symbol, "complete", cs)?;
    let (subscription, subscription_call) = if let Some(pending) = pending {
        let v = var(pending)?;
        let s = binding(&v.id)?;
        if v.init.is_some()
            || m.binding_writes(s) != 1
            || !m.scoping.symbol_redeclarations(s).is_empty()
        {
            return None;
        }
        let Expression::AssignmentExpression(a) = expr(subscribe)? else {
            return None;
        };
        let AssignmentTarget::AssignmentTargetIdentifier(target) = &a.left else {
            return None;
        };
        if a.operator.as_str() != "="
            || crate::analysis::identifier_symbol(target, m.scoping) != Some(s)
        {
            return None;
        }
        (s, &a.right)
    } else {
        let v = var(subscribe)?;
        let s = binding(&v.id)?;
        if !fixed(m, s) {
            return None;
        }
        (s, v.init.as_ref()?)
    };
    let [observer_arg] = method(m, subscription_call, input, "subscribe")? else {
        return None;
    };
    if id(m, observer_arg.as_expression()?) != Some(observer_symbol) {
        return None;
    }
    forwards(m, tf, subscription, "unsubscribe", false, true)?;
    if id(m, ret(finish)?) != Some(ts) {
        return None;
    }
    // Every linked declaration has exactly the protocol's one reference;
    // extra exposure cannot supply a second, unproved binding role.
    if [os, cs, ns, es, ts]
        .iter()
        .any(|s| m.binding_references(*s) != 1)
    {
        return None;
    }
    Some((
        vec![
            (f.span, "createMapOperator"),
            (op.span, "mapOperatorFunction"),
            (nf.span, "mapNextHandler"),
            (ef.span, "forwardMapError"),
            (cf.span, "completeMapSubscriber"),
            (tf.span, "unsubscribeMapSource"),
        ],
        tf.span,
    ))
}

#[cfg(test)]
#[path = "observable_operator_epochs_tests.rs"]
mod operator_epochs;

pub(in crate::naming) fn evaluate(
    m: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    // Walk only the candidate and at most two normal-function ancestors. The
    // subscriber arrow is unnamed and receives no declaration proposal.
    let mut candidate = Some(site);
    for _ in 0..3 {
        let Some(owner) = candidate else { break };
        if let AstKind::Function(f) = m.nodes.kind(owner.node_id) {
            for (canonical, ordinary) in [(true, false), (false, false), (false, true)] {
                if let Some((roles, cleanup)) = prove_with_origin(m, f, canonical, ordinary) {
                    // New constructor lowerings prove the entire map relation, but
                    // do not take over its teardown from the existing opaque cleanup
                    // family. Canonical witnesses retain all six original roles.
                    if !canonical && site.span == cleanup {
                        continue;
                    }
                    if let Some((_, name)) = roles.iter().find(|(span, _)| *span == site.span) {
                        if ordinary {
                            return vec![proposal(name,"shared_subscriber_stream_map_operator","complete map projection try/catch, same observer error/completion, exact subscription cleanup and linked declaration role; complete ordinary strict subscriber constructor API, closed object reads, sole origin and ordered captures; no native class identity claimed",facts)];
                        }
                        return vec![proposal(name,"shared_observable_map_operator","complete map projection try/catch, same observer error/completion, exact subscription cleanup and linked declaration role; scoped complete native Observable factory with sole origin and ordered captures",facts)];
                    }
                }
            }
        }
        candidate = m.enclosing_function(owner.node_id);
    }
    Vec::new()
}

#[cfg(test)]
mod ordinary_subscriber_tests {
    use super::*;
    const SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/program/observable-setup-order/ordinary-parent.js"));

    fn check(source: &str, expected: bool) {
        let allocator = oxc_allocator::Allocator::default();
        let parsed = crate::analysis::parse(&allocator, source);
        assert!(parsed.errors.is_empty());
        let built = oxc_semantic::SemanticBuilder::new().build(&parsed.program);
        assert!(built.errors.is_empty());
        let model = Model::new(source, &parsed.program, built.semantic.scoping(), built.semantic.nodes(), ProgramPropertiesLimits::default()).unwrap();
        let site = model.all_sites().iter().find(|site| site.binding_symbol.is_some_and(|symbol| model.scoping.symbol_name(symbol) == "$THREAD_0_97602")).unwrap();
        let AstKind::Function(f) = model.nodes.kind(site.node_id) else { panic!() };
        assert!(prove(&model, f, false).is_none(), "ordinary API cannot become a native class");
        assert_eq!(prove_with_origin(&model, f, false, true).is_some(), expected);
    }

    #[test]
    fn ordinary_subscriber_all_pending_owned_protocols() {
        let result = crate::naming::name_threads(SOURCE, true).unwrap();
        for original in ["$THREAD_5_71287", "$THREAD_2_93791", "$THREAD_5_48376", "$THREAD_6_88619", "$THREAD_2_114334", "$THREAD_0_97602", "$THREAD_4_146881", "$THREAD_0_29295", "$THREAD_4_47451", "$THREAD_6_110189", "$THREAD_7_121548", "$THREAD_7_91130", "$THREAD_0_93824", "$THREAD_1_11284", "$THREAD_4_4533", "$THREAD_7_82402", "$THREAD_6_34813", "$THREAD_5_100991"] {
            let outcome = result.report.outcomes.iter().find(|o| o.binding.name == original).unwrap();
            assert!(outcome.applied, "unresolved ordinary protocol {original}: {:?}", outcome.reason);
        }
    }

    #[test]
    fn ordinary_subscriber_map_whole_protocol_and_near_misses() {
        check(SOURCE, true);
        check(&SOURCE.replace("$KEY_203", "streamConstructor").replace("$BQ", "inputValue"), true);
        for (from, to) in [
            ("$KEY_2.next($KEY_0($BQ))", "$KEY_2.next($KEY_0($BQ, $BQ))"),
            ("$KEY_7 ? $BO : $KEY_203", "unknown ? $BO : $KEY_203"),
            ("$BN($KEY_7, $KEY_7 ? $BO : $KEY_203", r#"eval("sideEffect()"); $BN($KEY_7, $KEY_7 ? $BO : $KEY_203"#),
            ("typeof $BM !== \"function\"", "typeof $BM === \"function\""),
            ("$KEY_203 = $THREAD_2_101607();", "$KEY_203 = $THREAD_2_101607(); $KEY_203.prototype.subscribe = other;"),
        ] {
            assert!(SOURCE.contains(from), "missing mutation anchor {from}");
            check(&SOURCE.replacen(from, to, 1), false);
        }
    }
}
