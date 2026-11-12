//! Whole captured-source teardown callback. The name describes a dynamic
//! method call, not a native Observable or a guarantee about an opaque producer.
use crate::naming::shared::callable_origin_precedes;
use crate::naming::*;

fn symbol(m: &Model<'_, '_>, e: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e.without_parentheses(), m.scoping)
}
fn fixed(m: &Model<'_, '_>, s: SymbolId) -> bool {
    m.binding_writes(s) == 0 && m.scoping.symbol_redeclarations(s).is_empty()
}
fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    match e.without_parentheses() {
        Expression::StaticMemberExpression(p) if !p.optional && p.property.name == key => {
            Some(&p.object)
        }
        Expression::ComputedMemberExpression(p)
            if !p.optional
                && matches!(&p.expression, Expression::StringLiteral(k) if k.value == key) =>
        {
            Some(&p.object)
        }
        _ => None,
    }
}
fn call<'a>(e: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    match e.without_parentheses() {
        Expression::CallExpression(c) if !c.optional => Some(c),
        _ => None,
    }
}

/// Direct executed store, never a deferred nested write in an initializer.
fn origin<'a>(m: &Model<'_, 'a>, s: SymbolId) -> Option<(&'a Expression<'a>, NodeId, u32)> {
    if !m.scoping.symbol_redeclarations(s).is_empty() {
        return None;
    }
    let declaration = m.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = m.nodes.kind(declaration) else {
        return None;
    };
    let statement = m.nodes.parent_id(declaration);
    let owner = m.nodes.parent_id(statement);
    if !matches!(m.nodes.kind(statement), AstKind::VariableDeclaration(_))
        || !matches!(m.nodes.kind(owner), AstKind::FunctionBody(_))
    {
        return None;
    }
    if let Some(value) = &v.init {
        return fixed(m, s).then_some((value, owner, v.span.end));
    }
    if m.binding_writes(s) != 1 {
        return None;
    }
    let write = m
        .scoping
        .get_resolved_references(s)
        .find(|r| r.is_write())?;
    let assignment = m.nodes.parent_id(write.node_id());
    let AstKind::AssignmentExpression(a) = m.nodes.kind(assignment) else {
        return None;
    };
    let statement = m.nodes.parent_id(assignment);
    let AssignmentTarget::AssignmentTargetIdentifier(target) = &a.left else {
        return None;
    };
    (a.operator == AssignmentOperator::Assign
        && crate::analysis::identifier_symbol(target, m.scoping) == Some(s)
        && matches!(m.nodes.kind(statement), AstKind::ExpressionStatement(_))
        && m.nodes.parent_id(statement) == owner
        && a.span.start > v.span.end)
        .then_some((&a.right, owner, a.span.end))
}

/// The source is an immutable parameter, possibly copied through direct local
/// aliases. Each alias must be read after its executed initializer, in the
/// enclosing body; a hoisted declaration is not evidence of initialization.
fn input_ready(m: &Model<'_, '_>, e: &Expression<'_>, depth: usize) -> bool {
    if depth == 0 {
        return false;
    }
    let Some(s) = symbol(m, e) else {
        return false;
    };
    if !fixed(m, s) {
        return false;
    }
    match m.nodes.kind(m.scoping.symbol_declaration(s)) {
        AstKind::FormalParameter(p) => p.initializer.is_none(),
        AstKind::VariableDeclarator(_) => origin(m, s).is_some_and(|(value, body, end)| {
            let Expression::Identifier(reference) = e.without_parentheses() else {
                return false;
            };
            let Some(reference_id) = reference.reference_id.get() else {
                return false;
            };
            let use_node = m.scoping.get_reference(reference_id).node_id();
            callable_origin_precedes(m, body, end, use_node, None, 0, true)
                && input_ready(m, value, depth - 1)
        }),
        _ => false,
    }
}

fn prove(m: &Model<'_, '_>, site: FunctionSite) -> Option<()> {
    let AstKind::Function(f) = m.nodes.kind(site.node_id) else {
        return None;
    };
    if f.r#async || f.generator || f.params.rest.is_some() || !f.params.items.is_empty() {
        return None;
    }
    let body = f.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let [Statement::ReturnStatement(ret)] = body.statements.as_slice() else {
        return None;
    };
    let c = call(ret.argument.as_ref()?)?;
    if !c.arguments.is_empty() {
        return None;
    }
    let subscription = symbol(m, member(&c.callee, "unsubscribe")?)?;
    if site
        .span
        .contains_inclusive(m.scoping.symbol_span(subscription))
    {
        return None;
    }
    let (value, owner, end) = origin(m, subscription)?;
    let producer = call(value)?;
    let source = member(&producer.callee, "subscribe")?;
    if !input_ready(m, source, 8) {
        return None;
    }
    // The callback is itself a stable declaration. Every reference (including
    // alias exposure, pre-init invocation and synchronous registration) must
    // follow the subscription store, not merely its producer's beginning.
    let callback = site.binding_symbol?;
    if !fixed(m, callback) || m.binding_references(callback) == 0 {
        return None;
    }
    if !m
        .scoping
        .get_resolved_references(callback)
        .all(|r| callable_origin_precedes(m, owner, end, r.node_id(), None, 0, true))
    {
        return None;
    }
    Some(())
}

pub(in crate::naming) fn evaluate(m: &Model<'_, '_>, site: FunctionSite, facts: &Facts<'_>) -> Vec<Proposal> {
    if prove(m, site).is_none() {
        if sample_protocol(m, site).is_some() {
            return vec![proposal("appendSineDerivedSamples", "append_sine_derived_samples", "whole one-formal mutation: state = input[0]*63+input[1]; counter1, initial limit8; ordered sin/fround/*2/abs iteration, >=1 growth with cap50 and exact continue, bounded fractional fallback, append, counter increment, and final Number(input[1])+1 store; no lexical captures or invented initialized origins", facts)];
        }
        return Vec::new();
    }
    vec![proposal("unsubscribeCapturedSource", "captured_subscription_cleanup", "entire zero-formal body returns the nonoptional zero-argument unsubscribe method on one scoped captured subscribe result; direct sole executed store and immutable parameter/alias source; every callback exposure follows that store; dynamic source and method retain opaque consumer-defined identity", facts)]
}

fn stmt_expr<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    match s {
        Statement::ExpressionStatement(s) => Some(&s.expression),
        _ => None,
    }
}
fn binary<'a>(e: &'a Expression<'a>, op: &str) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    match e.without_parentheses() {
        Expression::BinaryExpression(b) if b.operator.as_str() == op => Some((&b.left, &b.right)),
        _ => None,
    }
}
fn number(e: &Expression<'_>, n: f64) -> bool {
    matches!(e.without_parentheses(), Expression::NumericLiteral(v) if v.value == n)
}
fn local<'a>(m: &Model<'_, '_>, s: &'a Statement<'a>) -> Option<(SymbolId, &'a Expression<'a>)> {
    let Statement::VariableDeclaration(d) = s else {
        return None;
    };
    if d.kind != VariableDeclarationKind::Let {
        return None;
    }
    let [v] = d.declarations.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(i) = &v.id else {
        return None;
    };
    let sym = i.symbol_id.get()?;
    m.scoping
        .symbol_redeclarations(sym)
        .is_empty()
        .then_some((sym, v.init.as_ref()?))
}
fn assigned<'a>(
    m: &Model<'_, '_>,
    s: &'a Statement<'a>,
    to: SymbolId,
) -> Option<&'a Expression<'a>> {
    let Expression::AssignmentExpression(a) = stmt_expr(s)?.without_parentheses() else {
        return None;
    };
    let AssignmentTarget::AssignmentTargetIdentifier(i) = &a.left else {
        return None;
    };
    (a.operator == AssignmentOperator::Assign
        && crate::analysis::identifier_symbol(i, m.scoping) == Some(to))
    .then_some(&a.right)
}
fn slot(m: &Model<'_, '_>, e: &Expression<'_>, array: SymbolId, index: f64) -> bool {
    matches!(e.without_parentheses(), Expression::ComputedMemberExpression(p) if !p.optional && symbol(m,&p.object)==Some(array) && number(&p.expression,index))
}
fn increment(m: &Model<'_, '_>, s: &Statement<'_>, to: SymbolId) -> bool {
    let Some(Expression::UpdateExpression(u)) = stmt_expr(s) else {
        return false;
    };
    let SimpleAssignmentTarget::AssignmentTargetIdentifier(i) = &u.argument else {
        return false;
    };
    u.prefix
        && u.operator.as_str() == "++"
        && crate::analysis::identifier_symbol(i, m.scoping) == Some(to)
}
fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    match s {
        Statement::BlockStatement(b) => Some(b.body.as_slice()),
        _ => None,
    }
}
fn unbound(m: &Model<'_, '_>, e: &Expression<'_>, name: &str) -> bool {
    matches!(e.without_parentheses(), Expression::Identifier(i) if i.name==name && crate::analysis::identifier_symbol(i,m.scoping).is_none())
}
fn sole_arg<'a>(c: &'a CallExpression<'a>) -> Option<&'a Expression<'a>> {
    let [arg] = c.arguments.as_slice() else {
        return None;
    };
    arg.as_expression()
}
fn math_arg<'a>(
    m: &Model<'_, '_>,
    e: &'a Expression<'a>,
    method: &str,
) -> Option<&'a Expression<'a>> {
    let c = call(e)?;
    unbound(m, member(&c.callee, method)?, "Math").then_some(sole_arg(c)?)
}

/// Consume the entire mutation protocol, including both loop increments and
/// the precise unlabeled continue target. No external seed/native capture is
/// inferred: Math and Number must resolve to globals in this lexical scope.
fn sample_protocol(m: &Model<'_, '_>, site: FunctionSite) -> Option<()> {
    let AstKind::Function(f) = m.nodes.kind(site.node_id) else {
        return None;
    };
    if f.r#async || f.generator || f.params.rest.is_some() || !fixed(m, site.binding_symbol?) {
        return None;
    }
    let [p] = f.params.items.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(pat) = &p.pattern else {
        return None;
    };
    let array = pat.symbol_id.get()?;
    if p.initializer.is_some() || !fixed(m, array) {
        return None;
    }
    let b = f.body.as_ref()?;
    if !b.directives.is_empty() {
        return None;
    }
    let [index, limit, state, Statement::WhileStatement(loop_), finish] = b.statements.as_slice()
    else {
        return None;
    };
    let (index, initial_index) = local(m, index)?;
    let (limit, initial_limit) = local(m, limit)?;
    let (state, initial_state) = local(m, state)?;
    if BTreeSet::from([array, index, limit, state]).len() != 4
        || !number(initial_index, 1.0)
        || !number(initial_limit, 8.0)
    {
        return None;
    }
    let (product, second) = binary(initial_state, "+")?;
    let (first, scale) = binary(product, "*")?;
    if !slot(m, first, array, 0.0) || !number(scale, 63.0) || !slot(m, second, array, 1.0) {
        return None;
    }
    let (left, right) = binary(&loop_.test, "<=")?;
    if symbol(m, left) != Some(index) || symbol(m, right) != Some(limit) {
        return None;
    }
    let [step, Statement::IfStatement(guard), push, advance] = block(&loop_.body)? else {
        return None;
    };
    let abs = math_arg(m, assigned(m, step, state)?, "abs")?;
    let (round, factor) = binary(abs, "*")?;
    let sine = math_arg(m, round, "fround")?;
    let sum = math_arg(m, sine, "sin")?;
    let (previous, counter) = binary(sum, "+")?;
    if symbol(m, previous) != Some(state)
        || symbol(m, counter) != Some(index)
        || !number(factor, 2.0)
    {
        return None;
    }
    let (left, right) = binary(&guard.test, ">=")?;
    if guard.alternate.is_some() || symbol(m, left) != Some(state) || !number(right, 1.0) {
        return None;
    }
    let [Statement::IfStatement(grow), fallback] = block(&guard.consequent)? else {
        return None;
    };
    let (left, right) = binary(&grow.test, "<")?;
    if grow.alternate.is_some() || symbol(m, left) != Some(limit) || !number(right, 50.0) {
        return None;
    }
    let [grow_limit, skip_index, Statement::ContinueStatement(skip)] = block(&grow.consequent)?
    else {
        return None;
    };
    if !increment(m, grow_limit, limit) || !increment(m, skip_index, index) || skip.label.is_some()
    {
        return None;
    }
    let (difference, denominator) = binary(assigned(m, fallback, state)?, "/")?;
    let (upper, current) = binary(difference, "-")?;
    if symbol(m, upper) != Some(limit)
        || symbol(m, current) != Some(index)
        || symbol(m, denominator) != Some(limit)
    {
        return None;
    }
    let push = call(stmt_expr(push)?)?;
    if symbol(m, member(&push.callee, "push")?) != Some(array)
        || symbol(m, sole_arg(push)?) != Some(state)
        || !increment(m, advance, index)
    {
        return None;
    }
    let Expression::AssignmentExpression(update) = stmt_expr(finish)? else {
        return None;
    };
    let AssignmentTarget::ComputedMemberExpression(target) = &update.left else {
        return None;
    };
    if update.operator != AssignmentOperator::Assign
        || target.optional
        || symbol(m, &target.object) != Some(array)
        || !number(&target.expression, 1.0)
    {
        return None;
    }
    let (coerced, one) = binary(&update.right, "+")?;
    let coerced = call(coerced)?;
    if !unbound(m, &coerced.callee, "Number")
        || !slot(m, sole_arg(coerced)?, array, 1.0)
        || !number(one, 1.0)
    {
        return None;
    }
    Some(())
}
