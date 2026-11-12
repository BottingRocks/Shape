//! Returned lazy hook registries. Class identity, base transport and the called
//! installer are independent obligations; p09 role labels are not proof inputs.
use crate::signals::extract::bindings::{call_arguments, property_key_name, FunctionSite, Model};
use crate::signals::analysis::roles::FinderClaim;
use crate::signals::schema::Confidence;
use oxc_ast::{ast::*, AstKind};
use oxc_semantic::SymbolId;
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::AssignmentOperator;
use std::collections::BTreeSet;

#[path = "lazy_registry_transport.rs"]
mod transport;

#[path = "pre_register_forwarder.rs"]
mod neutral_forwarder;

fn symbol(model: &Model<'_, '_>, value: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(value.without_parentheses(), model.scoping)
}
fn fixed(model: &Model<'_, '_>, s: SymbolId) -> bool {
    model.binding_writes(s) == 0 && model.scoping.symbol_redeclarations(s).is_empty()
}
fn body<'a>(f: &'a Function<'a>) -> Option<&'a FunctionBody<'a>> {
    (!f.r#async && !f.generator).then_some(())?;
    f.body.as_deref().filter(|b| b.directives.is_empty())
}
fn params(model: &Model<'_, '_>, f: &Function<'_>) -> Option<Vec<SymbolId>> {
    if f.params.rest.is_some() {
        return None;
    }
    f.params
        .items
        .iter()
        .map(|p| {
            if p.initializer.is_some() {
                return None;
            }
            let BindingPattern::BindingIdentifier(id) = &p.pattern else {
                return None;
            };
            let s = id.symbol_id.get()?;
            fixed(model, s).then_some(s)
        })
        .collect()
}
fn expression<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    match s {
        Statement::ExpressionStatement(s) => Some(s.expression.without_parentheses()),
        _ => None,
    }
}
fn call<'a>(s: &'a Statement<'a>) -> Option<&'a CallExpression<'a>> {
    match expression(s)? {
        Expression::CallExpression(c) if !c.optional => Some(c),
        _ => None,
    }
}
fn member<'a>(e: &'a Expression<'a>) -> Option<(&'a Expression<'a>, &'a str)> {
    match e.without_parentheses() {
        Expression::StaticMemberExpression(m) if !m.optional => {
            Some((&m.object, m.property.name.as_str()))
        }
        Expression::ComputedMemberExpression(m) if !m.optional => {
            match m.expression.without_parentheses() {
                Expression::StringLiteral(k) => Some((&m.object, k.value.as_str())),
                _ => None,
            }
        }
        _ => None,
    }
}
fn assigned(model: &Model<'_, '_>, a: &AssignmentExpression<'_>) -> Option<SymbolId> {
    if a.operator != AssignmentOperator::Assign {
        return None;
    }
    match &a.left {
        AssignmentTarget::AssignmentTargetIdentifier(id) => {
            crate::analysis::identifier_symbol(id, model.scoping)
        }
        _ => None,
    }
}
/// Only straight-line writes in the declaring callable count as origins.
/// Aliases must have been initialized before the consuming expression.
fn origin<'a>(model: &Model<'_, 'a>, s: SymbolId, before: u32) -> Option<&'a Expression<'a>> {
    if !model.scoping.symbol_redeclarations(s).is_empty() {
        return None;
    }
    let owner = model.owning_site(model.scoping.symbol_declaration(s))?;
    let AstKind::Function(f) = model.nodes.kind(owner.node_id) else {
        return None;
    };
    let mut found = None;
    let mut writes = 0;
    for statement in &body(f)?.statements {
        match statement {
            Statement::VariableDeclaration(d) => {
                for d in &d.declarations {
                    if matches!(&d.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
                    {
                        if let Some(e) = &d.init {
                            if e.span().end > before || found.replace(e).is_some() {
                                return None;
                            }
                        }
                    }
                }
            }
            _ => {
                if let Some(Expression::AssignmentExpression(a)) = expression(statement) {
                    if assigned(model, a) == Some(s) {
                        writes += 1;
                        if a.span.end > before || found.replace(&a.right).is_some() {
                            return None;
                        }
                    }
                }
            }
        }
    }
    (model.binding_writes(s) == writes).then_some(found)?
}
fn stable_function<'a>(
    model: &Model<'_, 'a>,
    mut value: &'a Expression<'a>,
    mut before: u32,
) -> Option<FunctionSite> {
    let mut seen = BTreeSet::new();
    for _ in 0..16 {
        match value.without_parentheses() {
            Expression::FunctionExpression(f) => return model.site_by_span(f.span),
            Expression::ArrowFunctionExpression(f) => return model.site_by_span(f.span),
            _ => {}
        }
        let s = symbol(model, value)?;
        if !seen.insert(s) || member_mutated(model, s) {
            return None;
        }
        if let AstKind::Function(f) = model.nodes.kind(model.scoping.symbol_declaration(s)) {
            return fixed(model, s)
                .then(|| model.site_by_span(f.span))
                .flatten();
        }
        value = origin(model, s, before)?;
        before = value.span().start;
    }
    None
}
/// Refuse observable writes through an alias, including computed prototype
/// keys. This is intentionally conservative; escapes are outside this grammar.
fn member_mutated(model: &Model<'_, '_>, s: SymbolId) -> bool {
    model.scoping.get_resolved_references(s).any(|r| {
        let start = model.nodes.kind(r.node_id()).span();
        let mut id = r.node_id();
        let mut through_member = false;
        for _ in 0..8 {
            let parent = model.nodes.parent_id(id);
            if parent == id {
                break;
            }
            match model.nodes.kind(parent) {
                AstKind::AssignmentExpression(a) => {
                    return through_member && a.left.span().contains_inclusive(start)
                }
                AstKind::UpdateExpression(_) => return true,
                AstKind::UnaryExpression(u)
                    if u.operator == oxc_syntax::operator::UnaryOperator::Delete =>
                {
                    return true
                }
                AstKind::StaticMemberExpression(_)
                | AstKind::ComputedMemberExpression(_)
                | AstKind::ParenthesizedExpression(_) => {
                    through_member = true;
                    id = parent;
                }
                _ => break,
            }
        }
        false
    })
}
fn owned<'m, 'a>(
    model: &'m Model<'_, 'a>,
    site: FunctionSite,
) -> impl Iterator<Item = AstKind<'a>> + 'm {
    model
        .nodes
        .iter()
        .filter(move |n| {
            model
                .owning_site(n.id())
                .is_some_and(|s| s.span == site.span)
        })
        .map(|n| n.kind())
}
fn returned_class<'a>(model: &Model<'_, 'a>, f: &'a Function<'a>) -> Option<&'a Class<'a>> {
    let statements = &body(f)?.statements;
    let Statement::ReturnStatement(ret) = statements.last()? else {
        return None;
    };
    let mut class = None;
    let mut declared = BTreeSet::new();
    let mut ready = BTreeSet::new();
    for s in &statements[..statements.len() - 1] {
        match s {
            Statement::ClassDeclaration(c) => {
                if class.replace(c.as_ref()).is_some() {
                    return None;
                }
                let id = c.id.as_ref()?.symbol_id.get()?;
                if !fixed(model, id) || member_mutated(model, id) {
                    return None;
                }
                ready.insert(id);
            }
            Statement::VariableDeclaration(d) => {
                for d in &d.declarations {
                    let BindingPattern::BindingIdentifier(id) = &d.id else {
                        return None;
                    };
                    let id = id.symbol_id.get()?;
                    if !declared.insert(id)
                        || !model.scoping.symbol_redeclarations(id).is_empty()
                        || member_mutated(model, id)
                    {
                        return None;
                    }
                    if let Some(e) = &d.init {
                        if !fixed(model, id) || !ready.contains(&symbol(model, e)?) {
                            return None;
                        }
                        ready.insert(id);
                    }
                }
            }
            _ => {
                let Expression::AssignmentExpression(a) = expression(s)? else {
                    return None;
                };
                let id = assigned(model, a)?;
                if !declared.contains(&id)
                    || ready.contains(&id)
                    || model.binding_writes(id) != 1
                    || !ready.contains(&symbol(model, &a.right)?)
                {
                    return None;
                }
                ready.insert(id);
            }
        }
    }
    if !declared.is_subset(&ready) || !ready.contains(&symbol(model, ret.argument.as_ref()?)?) {
        return None;
    }
    class
}
fn methods<'a>(class: &'a Class<'a>) -> Option<(&'a Function<'a>, &'a Function<'a>)> {
    if class.decorators.len() != 0 || class.body.body.len() != 2 {
        return None;
    }
    let mut ctor = None;
    let mut register = None;
    for element in &class.body.body {
        let ClassElement::MethodDefinition(m) = element else {
            return None;
        };
        if m.r#static || m.computed || !m.decorators.is_empty() {
            return None;
        }
        match property_key_name(&m.key)?.as_str() {
            "constructor" if m.kind == MethodDefinitionKind::Constructor => {
                ctor = Some(m.value.as_ref())
            }
            "register" if m.kind == MethodDefinitionKind::Method => {
                register = Some(m.value.as_ref())
            }
            _ => return None,
        }
    }
    Some((ctor?, register?))
}
fn forwards_arguments(model: &Model<'_, '_>, f: &Function<'_>) -> Option<()> {
    if !params(model, f)?.is_empty() {
        return None;
    }
    let [s] = body(f)?.statements.as_slice() else {
        return None;
    };
    let c = call(s)?;
    if !matches!(c.callee.without_parentheses(), Expression::Super(_)) {
        return None;
    }
    let [Argument::SpreadElement(spread)] = c.arguments.as_slice() else {
        return None;
    };
    matches!(spread.argument.without_parentheses(), Expression::Identifier(id) if id.name == "arguments" && crate::analysis::identifier_symbol(id, model.scoping).is_none()).then_some(())
}
/// Concrete hook receiver originates at the existing complete typed descriptor
/// interceptor proof, not a property bag or a readable installer name.
fn hook_receiver<'a>(
    model: &Model<'_, 'a>,
    value: &'a Expression<'a>,
    before: u32,
) -> Option<Span> {
    let mut value = value;
    let mut before = before;
    let mut seen = BTreeSet::new();
    for _ in 0..16 {
        if let Expression::CallExpression(c) = value.without_parentheses() {
            let factory = stable_function(model, &c.callee, c.span.start)?;
            let AstKind::Function(f) = model.nodes.kind(factory.node_id) else {
                return None;
            };
            let args = call_arguments(c);
            if c.optional || args.len() != c.arguments.len() || !(2..=3).contains(&args.len()) {
                return None;
            }
            if !crate::naming::proves_method_interceptor(model, factory, f, body(f)?) {
                transport::proves(model, factory, f)?;
            }
            return Some(factory.span);
        }
        let s = symbol(model, value)?;
        if !seen.insert(s) || member_mutated(model, s) {
            return None;
        }
        value = origin(model, s, before)?;
        before = value.span().start;
    }
    None
}
fn undefined(value: &Expression<'_>) -> bool {
    matches!(value.without_parentheses(), Expression::UnaryExpression(u)
        if u.operator == oxc_syntax::operator::UnaryOperator::Void
            && matches!(u.argument.without_parentheses(), Expression::NumericLiteral(n) if n.value == 0.0))
}
fn void_return(statement: &Statement<'_>) -> bool {
    matches!(statement, Statement::ReturnStatement(r) if r.argument.as_ref().is_some_and(undefined))
}
fn block_statements<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    match s {
        Statement::BlockStatement(b) => Some(&b.body),
        _ => None,
    }
}
fn nullable_member<'a>(value: &'a Expression<'a>) -> Option<(&'a Expression<'a>, &'a str)> {
    match value.without_parentheses() {
        Expression::ChainExpression(c) => match &c.expression {
            ChainElement::StaticMemberExpression(m) => Some((&m.object, m.property.name.as_str())),
            _ => None,
        },
        Expression::StaticMemberExpression(m) => Some((&m.object, m.property.name.as_str())),
        _ => None,
    }
}
fn nonnull<'a>(e: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    match e.without_parentheses() {
        Expression::BinaryExpression(b)
            if b.operator == oxc_syntax::operator::BinaryOperator::Inequality
                && matches!(b.right.without_parentheses(), Expression::NullLiteral(_)) =>
        {
            Some(&b.left)
        }
        _ => None,
    }
}
fn channel_guard(model: &Model<'_, '_>, e: &Expression<'_>, receiver: SymbolId) -> bool {
    let channel = |e: &Expression<'_>| {
        nonnull(e)
            .and_then(nullable_member)
            .is_some_and(|(root, key)| {
                key == "onBeforeInvoke" && symbol(model, root) == Some(receiver)
            })
    };
    if channel(e) {
        return true;
    }
    matches!(e.without_parentheses(), Expression::LogicalExpression(l)
        if l.operator == oxc_syntax::operator::LogicalOperator::And
            && nonnull(&l.left).and_then(|e| symbol(model, e)) == Some(receiver)
            && channel(&l.right))
}

/// A lowered nullable channel read, in the same own statement list immediately
/// before its guard. Both arms assign the fresh local exactly once; its entire
/// reference set is those writes and this guard read. No capture, escape,
/// intervening effect, donor branch or other receiver can supply this fact.
fn staged_channel_guard(
    model: &Model<'_, '_>,
    preceding: &[Statement<'_>],
    test: &Expression<'_>,
    receiver: SymbolId,
) -> Option<()> {
    staged_channel_variant(model, preceding, test, receiver, false)
        .or_else(|| staged_channel_variant(model, preceding, test, receiver, true))
}

// Two finite lowerings share the same closed local reference contract. The
// boolean form must test the same receiver in both arms, never an arbitrary
// false-like expression or a channel name on an unproved property bag.
fn staged_channel_variant(
    model: &Model<'_, '_>,
    preceding: &[Statement<'_>],
    test: &Expression<'_>,
    receiver: SymbolId,
    boolean: bool,
) -> Option<()> {
    let [.., Statement::VariableDeclaration(declaration), Statement::IfStatement(select)] =
        preceding
    else {
        return None;
    };
    if declaration.kind != VariableDeclarationKind::Let {
        return None;
    }
    let [local] = declaration.declarations.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(id) = &local.id else {
        return None;
    };
    let local_symbol = id.symbol_id.get()?;
    let read = if boolean { test } else { nonnull(test)? };
    if local.init.is_some()
        || symbol(model, read) != Some(local_symbol)
        || !model.scoping.symbol_redeclarations(local_symbol).is_empty()
        || model.binding_writes(local_symbol) != 2
    {
        return None;
    }
    let Expression::BinaryExpression(null_test) = select.test.without_parentheses() else {
        return None;
    };
    let expected_operator = if boolean {
        oxc_syntax::operator::BinaryOperator::Inequality
    } else {
        oxc_syntax::operator::BinaryOperator::Equality
    };
    if null_test.operator != expected_operator
        || symbol(model, &null_test.left) != Some(receiver)
        || !matches!(
            null_test.right.without_parentheses(),
            Expression::NullLiteral(_)
        )
    {
        return None;
    }
    let (absent_arm, present_arm) = if boolean {
        (select.alternate.as_ref()?, &select.consequent)
    } else {
        (&select.consequent, select.alternate.as_ref()?)
    };
    let [absent] = block_statements(absent_arm)? else {
        return None;
    };
    let [present] = block_statements(present_arm)? else {
        return None;
    };
    let Expression::AssignmentExpression(absent) = expression(absent)? else {
        return None;
    };
    let Expression::AssignmentExpression(present) = expression(present)? else {
        return None;
    };
    if assigned(model, absent) != Some(local_symbol)
        || assigned(model, present) != Some(local_symbol)
        || if boolean {
            nonnull(&absent.right).and_then(|e| symbol(model, e)) != Some(receiver)
        } else {
            !undefined(&absent.right)
        }
    {
        return None;
    }
    let present_value = if boolean {
        nonnull(&present.right)?
    } else {
        &present.right
    };
    let (root, "onBeforeInvoke") = member(present_value)? else {
        return None;
    };
    if symbol(model, root) != Some(receiver) {
        return None;
    }
    let references: Vec<_> = model
        .scoping
        .get_resolved_references(local_symbol)
        .collect();
    (references.len() == 3
        && references.iter().all(|r| {
            let span = model.nodes.kind(r.node_id()).span();
            if span == read.span() {
                r.is_read() && !r.is_write()
            } else {
                [absent.left.span(), present.left.span()].contains(&span)
                    && r.is_write()
                    && !r.is_read()
            }
        }))
    .then_some(())
}

/// A finite own-statement grammar for the normal installation path. There are
/// no loops, abrupt exits, IIFEs, dynamic call targets or executable class bodies.
/// Function values passed to register are data here; only the selected callback
/// is separately proved. Both arms must complete normally, so a donor in a dead
/// arm cannot be used to claim a later operation executes. Calls are modeled at
/// normal completion, as in the guarded host-listener setup in the witnesses.
fn phase_value(e: &Expression<'_>, depth: usize) -> bool {
    if depth > 32 {
        return false;
    }
    let child = |e: &Expression<'_>| phase_value(e, depth + 1);
    match e.without_parentheses() {
        Expression::Identifier(_)
        | Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::ThisExpression(_) => true,
        Expression::StaticMemberExpression(m) => child(&m.object),
        Expression::ChainExpression(c) => match &c.expression {
            ChainElement::StaticMemberExpression(m) => child(&m.object),
            _ => false,
        },
        Expression::UnaryExpression(u)
            if matches!(
                u.operator,
                oxc_syntax::operator::UnaryOperator::LogicalNot
                    | oxc_syntax::operator::UnaryOperator::Void
            ) =>
        {
            child(&u.argument)
        }
        Expression::BinaryExpression(b)
            if matches!(
                b.operator,
                oxc_syntax::operator::BinaryOperator::Equality
                    | oxc_syntax::operator::BinaryOperator::Inequality
                    | oxc_syntax::operator::BinaryOperator::StrictEquality
                    | oxc_syntax::operator::BinaryOperator::StrictInequality
                    | oxc_syntax::operator::BinaryOperator::Addition
            ) =>
        {
            child(&b.left) && child(&b.right)
        }
        Expression::LogicalExpression(l) => child(&l.left) && child(&l.right),
        _ => false,
    }
}
fn phase_call(c: &CallExpression<'_>, depth: usize) -> bool {
    if c.optional {
        return false;
    }
    let Some((receiver, key)) = member(&c.callee) else {
        return false;
    };
    if !phase_value(receiver, depth + 1) {
        return false;
    }
    match key {
        "register" => {
            matches!(c.arguments.as_slice(), [a] if matches!(a.as_expression(), Some(Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_))))
        }
        "addEventListener" | "notify" => c
            .arguments
            .iter()
            .all(|a| a.as_expression().is_some_and(|e| phase_value(e, depth + 1))),
        _ => false,
    }
}
fn phase_statements(model: &Model<'_, '_>, statements: &[Statement<'_>], depth: usize) -> bool {
    depth <= 32
        && statements.len() <= 128
        && statements
            .iter()
            .all(|s| phase_statement(model, s, depth + 1))
}
fn phase_statement(model: &Model<'_, '_>, s: &Statement<'_>, depth: usize) -> bool {
    if depth > 32 {
        return false;
    }
    match s {
        Statement::FunctionDeclaration(_) => true, // declaration does not run its body
        Statement::VariableDeclaration(d) => d.declarations.iter().all(|d| {
            matches!(d.id, BindingPattern::BindingIdentifier(_))
                && d.init.as_ref().is_none_or(|e| phase_value(e, depth + 1))
        }),
        Statement::ExpressionStatement(s) => match s.expression.without_parentheses() {
            Expression::AssignmentExpression(a) => {
                assigned(model, a).is_some() && phase_value(&a.right, depth + 1)
            }
            Expression::CallExpression(c) => phase_call(c, depth + 1),
            _ => false,
        },
        Statement::BlockStatement(b) => phase_statements(model, &b.body, depth + 1),
        Statement::IfStatement(b) => {
            phase_value(&b.test, depth + 1)
                && phase_statement(model, &b.consequent, depth + 1)
                && b.alternate
                    .as_ref()
                    .is_none_or(|s| phase_statement(model, s, depth + 1))
        }
        Statement::TryStatement(t) => {
            t.finalizer.is_none()
                && phase_statements(model, &t.block.body, depth + 1)
                && t.handler.as_ref().is_some_and(|h| {
                    let Some((last, preceding)) = h.body.body.split_last() else {
                        return false;
                    };
                    void_return(last) && phase_statements(model, preceding, depth + 1)
                })
        }
        _ => false,
    }
}

/// The only true write is the terminal write after installation. False writes
/// may reset the lifecycle (the real teardown does this); a direct false origin
/// must already exist in the declaring callable before the factory invocation.
fn installation_flag(model: &Model<'_, '_>, flag: SymbolId, before: u32, terminal: Span) -> bool {
    if !model.scoping.symbol_redeclarations(flag).is_empty() {
        return false;
    }
    let Some(owner) = model.owning_site(model.scoping.symbol_declaration(flag)) else {
        return false;
    };
    let AstKind::Function(f) = model.nodes.kind(owner.node_id) else {
        return false;
    };
    let Some(b) = body(f) else {
        return false;
    };
    let initial = model.initializer(flag).is_some_and(|v| v.span().end < before && matches!(v.without_parentheses(), Expression::BooleanLiteral(b) if !b.value))
        || b.statements.iter().any(|s| matches!(expression(s), Some(Expression::AssignmentExpression(a))
            if assigned(model, a) == Some(flag) && a.span.end < before
                && matches!(a.right.without_parentheses(), Expression::BooleanLiteral(b) if !b.value)));
    initial && model.scoping.get_resolved_references(flag).filter(|r| r.is_write()).all(|r| {
        let AstKind::AssignmentExpression(a) = model.nodes.parent_kind(r.node_id()) else { return false; };
        assigned(model, a) == Some(flag) && matches!(a.right.without_parentheses(), Expression::BooleanLiteral(b) if !b.value || a.span == terminal)
    })
}
fn installer_phase<'a>(
    model: &Model<'_, 'a>,
    f: &'a Function<'a>,
    before: u32,
) -> Option<&'a [Statement<'a>]> {
    let (last, prefix) = body(f)?.statements.split_last()?;
    // Only declarations and stable callable aliases precede the lifecycle gate.
    for statement in prefix {
        match statement {
            Statement::FunctionDeclaration(_) => {}
            Statement::VariableDeclaration(d) => {
                for d in &d.declarations {
                    let BindingPattern::BindingIdentifier(id) = &d.id else {
                        return None;
                    };
                    if let Some(value) = &d.init {
                        if !fixed(model, id.symbol_id.get()?)
                            || stable_function(model, value, d.span.start).is_none()
                        {
                            return None;
                        }
                    }
                }
            }
            _ => return None,
        }
    }
    let Statement::IfStatement(gate) = last else {
        return None;
    };
    let flag = symbol(model, &gate.test)?;
    if !matches!(block_statements(&gate.consequent)?, [s] if void_return(s)) {
        return None;
    }
    let statements = block_statements(gate.alternate.as_ref()?)?;
    let [phase @ .., setting, returned] = statements else {
        return None;
    };
    if !void_return(returned) {
        return None;
    }
    let Expression::AssignmentExpression(a) = expression(setting)? else {
        return None;
    };
    if assigned(model, a) != Some(flag)
        || !matches!(a.right.without_parentheses(), Expression::BooleanLiteral(b) if b.value)
        || !installation_flag(model, flag, before, a.span)
    {
        return None;
    }
    phase_statements(model, phase, 0).then_some(phase)
}
fn accumulator_return(model: &Model<'_, '_>, s: &Statement<'_>, accumulator: SymbolId) -> bool {
    matches!(s, Statement::ReturnStatement(r) if r.argument.as_ref().and_then(|e| symbol(model, e)) == Some(accumulator))
}
/// One fresh local carrier, one owned target write and one dispatch read. The
/// complete reference set excludes captures, aliases, extra keys and escapes.
fn staged_target(
    model: &Model<'_, '_>,
    declaration: &Statement<'_>,
    write: &Statement<'_>,
    argument: &Expression<'_>,
    event: SymbolId,
) -> Option<()> {
    let Statement::VariableDeclaration(d) = declaration else {
        return None;
    };
    let [d] = d.declarations.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(id) = &d.id else {
        return None;
    };
    let carrier = id.symbol_id.get()?;
    if !fixed(model, carrier)
        || !matches!(d.init.as_ref()?.without_parentheses(), Expression::ObjectExpression(o) if o.properties.is_empty())
        || symbol(model, argument) != Some(carrier)
    {
        return None;
    }
    let Expression::AssignmentExpression(a) = expression(write)? else {
        return None;
    };
    if a.operator != AssignmentOperator::Assign {
        return None;
    }
    let AssignmentTarget::StaticMemberExpression(target) = &a.left else {
        return None;
    };
    if target.optional
        || target.property.name != "target"
        || symbol(model, &target.object) != Some(carrier)
    {
        return None;
    }
    let (receiver, "thisObj") = member(&a.right)? else {
        return None;
    };
    if symbol(model, receiver) != Some(event) {
        return None;
    }
    let references: Vec<_> = model.scoping.get_resolved_references(carrier).collect();
    if references.len() != 2
        || !references.iter().all(|r| {
            r.is_read()
                && [target.object.span(), argument.span()]
                    .contains(&model.nodes.kind(r.node_id()).span())
        })
    {
        return None;
    }
    Some(())
}
fn callback_protocol<'a>(
    model: &Model<'_, 'a>,
    callback: FunctionSite,
    owner: FunctionSite,
) -> Option<()> {
    if model.owning_site(callback.node_id).map(|s| s.span) != Some(owner.span) {
        return None;
    }
    let (parameters, statements) = match model.nodes.kind(callback.node_id) {
        AstKind::ArrowFunctionExpression(a)
            if !a.r#async && !a.expression && a.body.directives.is_empty() =>
        {
            (&a.params, a.body.statements.as_slice())
        }
        AstKind::Function(f) => (&f.params, body(f)?.statements.as_slice()),
        _ => return None,
    };
    if parameters.rest.is_some() {
        return None;
    }
    let [first, second] = parameters.items.as_slice() else {
        return None;
    };
    if first.initializer.is_some() || second.initializer.is_some() {
        return None;
    }
    let BindingPattern::BindingIdentifier(first) = &first.pattern else {
        return None;
    };
    let BindingPattern::BindingIdentifier(second) = &second.pattern else {
        return None;
    };
    let first = first.symbol_id.get()?;
    let second = second.symbol_id.get()?;
    if !fixed(model, first) || !fixed(model, second) {
        return None;
    }
    let [Statement::TryStatement(t)] = statements else {
        return None;
    };
    if t.finalizer.is_some() {
        return None;
    }
    let [Statement::IfStatement(bypass)] = t.block.body.as_slice() else {
        return None;
    };
    let bypass_member = |e: &Expression<'_>| {
        nonnull(e)
            .and_then(nullable_member)
            .is_some_and(|(root, key)| key == "bypassResult" && symbol(model, root) == Some(second))
    };
    let bypass_test = bypass_member(&bypass.test)
        || matches!(bypass.test.without_parentheses(), Expression::LogicalExpression(l)
        if l.operator == oxc_syntax::operator::LogicalOperator::And
            && nonnull(&l.left).and_then(|e| symbol(model, e)) == Some(second) && bypass_member(&l.right));
    if !bypass_test
        || !matches!(block_statements(&bypass.consequent)?, [s] if accumulator_return(model, s, second))
    {
        return None;
    }
    let (staged, dispatch, returned) = match block_statements(bypass.alternate.as_ref()?)? {
        [dispatch, returned] => (None, dispatch, returned),
        [declaration, write, dispatch, returned] => {
            (Some((declaration, write)), dispatch, returned)
        }
        _ => return None,
    };
    if !accumulator_return(model, returned, second) {
        return None;
    }
    let dispatch = call(dispatch)?;
    let target = stable_function(model, &dispatch.callee, dispatch.span.start)?;
    if model.owning_site(target.node_id).map(|s| s.span) != Some(owner.span) {
        return None;
    }
    let [arg] = dispatch.arguments.as_slice() else {
        return None;
    };
    let argument = arg.as_expression()?;
    if let Some((declaration, write)) = staged {
        staged_target(model, declaration, write, argument, first)?;
    } else {
        let Expression::ObjectExpression(o) = argument.without_parentheses() else {
            return None;
        };
        let [ObjectPropertyKind::ObjectProperty(p)] = o.properties.as_slice() else {
            return None;
        };
        if p.computed
            || p.method
            || p.shorthand
            || p.kind != PropertyKind::Init
            || property_key_name(&p.key).as_deref() != Some("target")
        {
            return None;
        }
        let (receiver, "thisObj") = member(&p.value)? else {
            return None;
        };
        if symbol(model, receiver) != Some(first) {
            return None;
        }
    }
    let catch = t.handler.as_ref()?;
    let [notify, returned] = catch.body.body.as_slice() else {
        return None;
    };
    if !accumulator_return(model, returned, second) {
        return None;
    }
    let notify = call(notify)?;
    let (_, "notify") = member(&notify.callee)? else {
        return None;
    };
    if !notify
        .arguments
        .iter()
        .all(|a| a.as_expression().is_some_and(|e| phase_value(e, 0)))
    {
        return None;
    }
    Some(())
}

fn installer<'a>(
    model: &Model<'_, 'a>,
    value: &'a Expression<'a>,
    before: u32,
) -> Option<(Span, Span)> {
    let site = stable_function(model, value, before)?;
    let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
        return None;
    };
    if !params(model, f)?.is_empty() {
        return None;
    }
    let phase = installer_phase(model, f, before)?;
    for (index, statement) in phase.iter().enumerate() {
        let Statement::IfStatement(guard) = statement else {
            continue;
        };
        if guard.alternate.is_some() {
            continue;
        }
        let Some([statement]) = block_statements(&guard.consequent) else {
            continue;
        };
        let Some(c) = call(statement) else {
            continue;
        };
        let Some((hook, "register")) = member(&c.callee) else {
            continue;
        };
        let Some((receiver, "onBeforeInvoke")) = member(hook) else {
            continue;
        };
        let Some(receiver_symbol) = symbol(model, receiver) else {
            continue;
        };
        if !channel_guard(model, &guard.test, receiver_symbol)
            && staged_channel_guard(model, &phase[..index], &guard.test, receiver_symbol).is_none()
        {
            continue;
        }
        let Some(factory) = hook_receiver(model, receiver, before) else {
            continue;
        };
        let [argument] = c.arguments.as_slice() else {
            continue;
        };
        let Some(callback) = argument
            .as_expression()
            .and_then(|e| stable_function(model, e, c.span.start))
        else {
            continue;
        };
        if callback_protocol(model, callback, site).is_some() {
            return Some((site.span, factory));
        }
    }
    None
}
fn ordered_base(model: &Model<'_, '_>, mut s: SymbolId, mut before: u32) -> Option<()> {
    let mut seen = BTreeSet::new();
    for _ in 0..16 {
        if !seen.insert(s) {
            return None;
        }
        match model.nodes.kind(model.scoping.symbol_declaration(s)) {
            AstKind::Class(c) => return (fixed(model, s) && c.span.end < before).then_some(()),
            AstKind::Function(_) => return fixed(model, s).then_some(()),
            _ => {
                let value = origin(model, s, before)?;
                before = value.span().start;
                s = symbol(model, value)?;
            }
        }
    }
    None
}

/// The factory result is transported through ordered immutable constructor
/// aliases and actually constructed. Passing the result elsewhere or writing
/// a prototype through an alias is outside this closed consumer grammar.
fn constructor_alias(
    model: &Model<'_, '_>,
    s: SymbolId,
    after: u32,
    seen: &mut BTreeSet<SymbolId>,
) -> Option<usize> {
    if !seen.insert(s) || seen.len() > 32 || !fixed(model, s) {
        return None;
    }
    let mut constructions = 0;
    for r in model.scoping.get_resolved_references(s) {
        let AstKind::IdentifierReference(id) = model.nodes.kind(r.node_id()) else {
            return None;
        };
        if !r.is_read() || id.span.start < after {
            return None;
        }
        match model.nodes.parent_kind(r.node_id()) {
            AstKind::NewExpression(n) if symbol(model, &n.callee) == Some(s) => constructions += 1,
            AstKind::VariableDeclarator(d)
                if d.init.as_ref().is_some_and(|e| symbol(model, e) == Some(s)) =>
            {
                let BindingPattern::BindingIdentifier(id) = &d.id else {
                    return None;
                };
                constructions += constructor_alias(model, id.symbol_id.get()?, d.span.end, seen)?;
            }
            _ => return None,
        }
    }
    Some(constructions)
}

fn prove<'a>(
    model: &Model<'_, 'a>,
    factory: FunctionSite,
    f: &'a Function<'a>,
) -> Option<FinderClaim> {
    let factory_params = params(model, f)?;
    let [base] = factory_params.as_slice() else {
        return None;
    };
    let class = returned_class(model, f)?;
    if symbol(model, class.super_class.as_ref()?) != Some(*base) {
        return None;
    }
    let (ctor, register) = methods(class)?;
    forwards_arguments(model, ctor)?;
    let ps = params(model, register)?;
    let [formal] = ps.as_slice() else {
        return None;
    };
    let [install, forward, Statement::ReturnStatement(ret)] = body(register)?.statements.as_slice()
    else {
        return None;
    };
    let install = call(install)?;
    if !install.arguments.is_empty() {
        return None;
    }
    let forward = call(forward)?;
    let (receiver, "register") = member(&forward.callee)? else {
        return None;
    };
    if !matches!(receiver.without_parentheses(), Expression::Super(_))
        || forward.arguments.len() != 1
        || symbol(model, forward.arguments[0].as_expression()?) != Some(*formal)
        || !matches!(
            ret.argument.as_ref()?.without_parentheses(),
            Expression::ThisExpression(_)
        )
    {
        return None;
    }
    let factory_symbol = factory.binding_symbol?;
    if !fixed(model, factory_symbol) || member_mutated(model, factory_symbol) {
        return None;
    }
    let mut witnesses = Vec::new();
    for reference in model.scoping.get_resolved_references(factory_symbol) {
        if !reference.is_read() {
            return None;
        }
        // Factory escapes and indirect/mutable calls fail closed.
        let AstKind::CallExpression(c) = model.nodes.parent_kind(reference.node_id()) else {
            return None;
        };
        if c.optional || symbol(model, &c.callee) != Some(factory_symbol) || c.arguments.len() != 1
        {
            return None;
        }
        let actual = symbol(model, c.arguments[0].as_expression()?)?;
        ordered_base(model, actual, c.span.start)?;
        if !crate::naming::classes::callback_notifier::proves_base(model, actual) {
            return None;
        }
        // Base and installer are resolved at the concrete ordered factory call.
        // The notifier proof owns native/ordinary base semantics.
        let call_id = model.nodes.parent_id(reference.node_id());
        let AstKind::VariableDeclarator(result) = model.nodes.parent_kind(call_id) else {
            return None;
        };
        if !result.init.as_ref().is_some_and(|e| e.span() == c.span) {
            return None;
        }
        let BindingPattern::BindingIdentifier(id) = &result.id else {
            return None;
        };
        if constructor_alias(model, id.symbol_id.get()?, c.span.end, &mut BTreeSet::new())? == 0 {
            return None;
        }
        let (hook, descriptor) = installer(model, &install.callee, c.span.start)?;
        witnesses.push(format!("factory call {:?}; verified notifier base SymbolId {:?}; installer {:?}; typed method-interceptor {:?}; inherited base protocol retained", c.span, actual, hook, descriptor));
    }
    if witnesses.is_empty() {
        return None;
    }
    witnesses.push("exact same-factory returned class; stable ordered class aliases; super(...arguments); zero-argument installer before super.register(formal0); return this; no extra class/factory/register effects; false-initialized lifecycle gate, finite normal-completion installation phase and terminal true flag; guarded typed hook registration; complete owned bypass/receiver-dispatch/accumulator-return callback".into());
    Some(FinderClaim::ClassBinding {
        role: "owned_lazy_hook_registry_class",
        class_span: class.span,
        confidence: Confidence::High,
        evidence: witnesses,
    })
}
pub(crate) fn claims(model: &Model<'_, '_>) -> Vec<FinderClaim> {
    model
        .all_sites()
        .iter()
        .filter_map(|site| {
            let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
                return None;
            };
            prove(model, *site, f).or_else(|| neutral_forwarder::prove(model, *site, f))
        })
        .collect()
}

#[cfg(test)]
#[path = "lazy_registry_tests.rs"]
mod tests;
