//! Whole-body proof for the registered-collector queue starter.
//!
//! The owner witness is a zero-formal function that creates two queue objects
//! from one provided factory, starts both through a three-way
//! timer/synchronous/pending-list fallback, walks two provided registries, and
//! returns the first queue's `onEmpty` member:
//!
//! ```text
//! queueA = factory(config, receiver.global.setTimeout);
//! let queueB = factory(config, task => task());
//! let startB = queueB.start;
//! if (typeof setTimeout !== "function") { startB(); }
//! else if (pendingFlag) { setTimeout(startB, 0); }
//! else if (pendingList) { pendingList.push(startB); }
//! let startA = queueA.start;
//! ... same three-way fallback with startA ...
//! registry.forEach((entry, index) => {
//!     let idx = index;
//!     let ctor = entry.ctor;
//!     let priority = entry.priority;
//!     let debugName = entry.debugName;
//!     if (ctor == null) { return void 0; }
//!     else {
//!         try {
//!             let errorConfig = { errorLogger: errorLogger.bind(null, index, false) };
//!             let subscription = ctor(makeOptions(errorConfig, receiver),
//!                 queueA.enqueue, queueB.enqueue, priority,
//!                 value => valueSink(value, idx), idx, debugName)
//!                 .subscribe(sinkFactory(idx));
//!             unsubscribers.push(() => subscription.unsubscribe());
//!         } catch (caught) {
//!             errorLogger(idx, false, caught);
//!         }
//!         return void 0;
//!     }
//! });
//! groups.forEach((group, index) => {
//!     let groupIndices = groupIndex[index];
//!     let groupSpecEntry = groupSpec[index];
//!     let sinks = groupIndices.map(sinkFactory);
//!     let groupError = { errorLogger: void 0 };
//!     groupError.errorLogger = errorLogger.bind(null, groupIndices[0], true);
//!     sinks = groupConnector(sinks, group, makeOptions(groupError, receiver),
//!         queueA.enqueue, queueB.enqueue, groupSpecEntry,
//!         value => valueSink(value, groupIndices[0]));
//!     unsubscribers.push(sinks);
//! });
//! return queueA.onEmpty;
//! ```
//!
//! Every callable origin above is *provided consumer input*: the factory, the
//! registry/group tables, the options factory, the error logger, the per-index
//! sink factory, the value sink, the group connector and the unsubscribe list
//! are never resolved to a native or static callee identity. Identity is scoped
//! Oxc symbol equality only. Generated spellings, source offsets, vendor data,
//! hash dictionaries and inherited role labels never participate.
//!
//! Coverage is intentionally partial: this revision proves the representative
//! whole-body shape (including alpha/readable renamings and a namespace timer
//! form), plus the proven single-write callback temporaries: a leading bare
//! `let subscription;` that is assigned the `ctor(...).subscribe(...)` call
//! inside the registry `try`, a hoisted
//! `let options = makeOptions(groupError, receiver);` passed to the group
//! connector, and the equivalent inline group-error literal
//! `makeOptions({ errorLogger: errorLogger.bind(null, groupIndices[0], true) },
//! receiver)` passed directly to the connector. The hoisted locals are ordinary
//! single-write free variables that resolve by scoped Oxc symbol identity; the
//! inline error object is a direct literal with the same scoped logger, bind
//! target and `true` group marker. Every other registry/group restructuring
//! stays unresolved rather than being force-matched.

use super::*;
use oxc_semantic::SymbolId;

type Symbol = SymbolId;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Timer {
    Global,
    Captured(Symbol),
}

/// The consumer-provided identities that recur across the owned body. A role is
/// bound once and every later use must resolve to the same scoped symbol.
#[derive(Default)]
struct Shared {
    factory: Option<Symbol>,
    config: Option<Symbol>,
    receiver: Option<Symbol>,
    registry: Option<Symbol>,
    groups: Option<Symbol>,
    group_index: Option<Symbol>,
    group_spec: Option<Symbol>,
    error_logger: Option<Symbol>,
    make_options: Option<Symbol>,
    sink_factory: Option<Symbol>,
    value_sink: Option<Symbol>,
    group_connector: Option<Symbol>,
    unsubscribers: Option<Symbol>,
    pending_flag: Option<Symbol>,
    pending_list: Option<Symbol>,
}

fn agree(slot: &mut Option<Symbol>, value: Symbol) -> Option<()> {
    match *slot {
        Some(existing) if existing == value => Some(()),
        None => {
            *slot = Some(value);
            Some(())
        }
        _ => None,
    }
}

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(e.without_parentheses(), model.scoping)
}

/// The plain symbol a variable declarator introduces, when it is a plain
/// identifier binding of an ordinary var/let/const declaration.
fn local_symbol(
    model: &Model<'_, '_>,
    site: FunctionSite,
    pattern: &BindingPattern<'_>,
) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(id) = pattern else {
        return None;
    };
    let s = id.symbol_id.get()?;
    (site.span.contains_inclusive(model.scoping.symbol_span(s))
        && model.scoping.symbol_redeclarations(s).is_empty()
        && matches!(
            model.nodes.parent_kind(model.scoping.symbol_declaration(s)),
            AstKind::VariableDeclaration(d)
                if matches!(
                    d.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                )
        ))
    .then_some(s)
}

/// A binding declared before the owned body: data or callable consumer input.
/// Parameters, catches, destructuring origins, loop targets and redeclarations
/// never qualify.
fn captured_symbol(model: &Model<'_, '_>, site: FunctionSite, s: Symbol) -> Option<Symbol> {
    if site.span.contains_inclusive(model.scoping.symbol_span(s))
        || !model.scoping.symbol_redeclarations(s).is_empty()
        || model.scoping.symbol_span(s).end >= site.span.start
    {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(d)
                if matches!(
                    d.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                )
        )
        || matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
    {
        return None;
    }
    Some(s)
}

fn captured(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<Symbol> {
    captured_symbol(model, site, symbol(model, e)?)
}

fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = e.without_parentheses() else {
        return None;
    };
    (!m.optional && m.property.name == key).then_some(&m.object)
}

fn index<'a>(e: &'a Expression<'a>) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::ComputedMemberExpression(m) = e.without_parentheses() else {
        return None;
    };
    (!m.optional).then_some((&m.object, &m.expression))
}

fn call<'a>(e: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(c) = e.without_parentheses() else {
        return None;
    };
    (!c.optional).then_some(c)
}

fn statement_call<'a>(s: &'a Statement<'a>) -> Option<&'a CallExpression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    call(&s.expression)
}

fn returned<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ReturnStatement(r) = s else {
        return None;
    };
    r.argument.as_ref()
}

fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(b.body.as_slice())
}

fn single_variable<'a>(s: &'a Statement<'a>) -> Option<&'a VariableDeclarator<'a>> {
    let Statement::VariableDeclaration(d) = s else {
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
    Some(v)
}

fn assignment<'a>(s: &'a Statement<'a>) -> Option<&'a AssignmentExpression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    let Expression::AssignmentExpression(a) = &s.expression else {
        return None;
    };
    (a.operator.as_str() == "=").then_some(a)
}

/// `let name;` — one bare, uninitialized declarator whose local is written
/// exactly once by the later assignment. The recovered rewrite pre-declares a
/// subscription/options temporary up front and assigns it in the original
/// position, so the declaration itself carries no value.
fn bare_declaration(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
) -> Option<Symbol> {
    let variable = single_variable(statement)?;
    if variable.init.is_some() {
        return None;
    }
    let local = local_symbol(model, site, &variable.id)?;
    (model.binding_writes(local) == 1).then_some(local)
}

fn exact_arguments<'a>(c: &'a CallExpression<'a>, count: usize) -> Option<Vec<&'a Expression<'a>>> {
    if c.arguments.len() != count {
        return None;
    }
    let values = call_arguments(c);
    (values.len() == count).then_some(values)
}

fn number(e: &Expression<'_>, value: f64) -> bool {
    matches!(e.without_parentheses(), Expression::NumericLiteral(n) if n.value == value)
}

fn boolean(e: &Expression<'_>, value: bool) -> bool {
    matches!(e.without_parentheses(), Expression::BooleanLiteral(b) if b.value == value)
}

fn null_literal(e: &Expression<'_>) -> bool {
    matches!(e.without_parentheses(), Expression::NullLiteral(_))
}

fn void_zero(e: &Expression<'_>) -> bool {
    matches!(
        e.without_parentheses(),
        Expression::UnaryExpression(u)
            if u.operator.as_str() == "void"
                && matches!(u.argument.without_parentheses(), Expression::NumericLiteral(n) if n.value == 0.0)
    )
}

/// `void 0` or the unshadowed `undefined` global.
fn unset(e: &Expression<'_>) -> bool {
    void_zero(e)
        || matches!(e.without_parentheses(), Expression::Identifier(i)
            if i.name == "undefined")
}

fn void_return(s: &Statement<'_>) -> bool {
    matches!(returned(s), Some(e) if void_zero(e))
}

fn null_compare(model: &Model<'_, '_>, e: &Expression<'_>, s: Symbol) -> bool {
    let Expression::BinaryExpression(b) = e.without_parentheses() else {
        return false;
    };
    (b.operator.as_str() == "==" || b.operator.as_str() == "===")
        && symbol(model, &b.left) == Some(s)
        && null_literal(&b.right)
}

fn plain_params(model: &Model<'_, '_>, params: &FormalParameters<'_>) -> Option<Vec<Symbol>> {
    if params.rest.is_some() {
        return None;
    }
    let mut symbols = Vec::new();
    for parameter in &params.items {
        if parameter.initializer.is_some() {
            return None;
        }
        let BindingPattern::BindingIdentifier(id) = &parameter.pattern else {
            return None;
        };
        let s = id.symbol_id.get()?;
        if model.binding_writes(s) != 0 || !model.scoping.symbol_redeclarations(s).is_empty() {
            return None;
        }
        symbols.push(s);
    }
    Some(symbols)
}

fn inline_parts<'a>(
    e: &'a Expression<'a>,
) -> Option<(&'a FormalParameters<'a>, &'a FunctionBody<'a>)> {
    match e.without_parentheses() {
        Expression::ArrowFunctionExpression(a) if !a.r#async => {
            Some((a.params.as_ref(), a.body.as_ref()))
        }
        Expression::FunctionExpression(f) if !f.r#async && !f.generator => {
            Some((f.params.as_ref(), f.body.as_deref()?))
        }
        _ => None,
    }
}

/// A one-formal inline scheduler that only calls its formal: `task => task()`.
fn inline_caller(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Symbol> {
    let (params, body) = inline_parts(e)?;
    if !body.directives.is_empty() {
        return None;
    }
    let [parameter] = params.items.as_slice() else {
        return None;
    };
    if params.rest.is_some() || parameter.initializer.is_some() {
        return None;
    }
    let BindingPattern::BindingIdentifier(id) = &parameter.pattern else {
        return None;
    };
    let task = id.symbol_id.get()?;
    let [statement] = body.statements.as_slice() else {
        return None;
    };
    let c = statement_call(statement)?;
    if !c.arguments.is_empty() || symbol(model, &c.callee) != Some(task) {
        return None;
    }
    Some(task)
}

/// The named timer reference in the three-way guard. The unshadowed global
/// `setTimeout` and a captured namespace `<ref>.setTimeout` are both accepted;
/// a provider namespace member is consumer input, not a native claim.
fn timer_of(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Timer> {
    let e = e.without_parentheses();
    if let Expression::Identifier(i) = e {
        return (i.name == "setTimeout"
            && crate::analysis::identifier_symbol(i, model.scoping).is_none())
        .then_some(Timer::Global);
    }
    let Expression::StaticMemberExpression(outer) = e else {
        return None;
    };
    if outer.optional || outer.property.name != "setTimeout" {
        return None;
    }
    let mut node = &outer.object;
    while let Expression::StaticMemberExpression(m) = node {
        node = &m.object;
    }
    Some(Timer::Captured(symbol(model, node)?))
}

fn typeof_test(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Timer> {
    let Expression::BinaryExpression(b) = e.without_parentheses() else {
        return None;
    };
    if b.operator.as_str() != "!==" && b.operator.as_str() != "!=" {
        return None;
    }
    let Expression::UnaryExpression(u) = b.left.without_parentheses() else {
        return None;
    };
    if u.operator.as_str() != "typeof" {
        return None;
    }
    if !matches!(b.right.without_parentheses(), Expression::StringLiteral(s) if s.value == "function")
    {
        return None;
    }
    timer_of(model, &u.argument)
}

fn schedule_call(
    model: &Model<'_, '_>,
    c: &CallExpression<'_>,
    start: Symbol,
    timer: Timer,
) -> bool {
    if let Some(target) = member(&c.callee, "call") {
        if timer_of(model, target) != Some(timer) {
            return false;
        }
        let Some(args) = exact_arguments(c, 3) else {
            return false;
        };
        return unset(args[0]) && symbol(model, args[1]) == Some(start) && number(args[2], 0.0);
    }
    if timer_of(model, &c.callee) != Some(timer) {
        return false;
    }
    let Some(args) = exact_arguments(c, 2) else {
        return false;
    };
    symbol(model, args[0]) == Some(start) && number(args[1], 0.0)
}

fn pairwise_distinct(symbols: &[Symbol]) -> bool {
    for (position, left) in symbols.iter().enumerate() {
        if symbols[position + 1..].contains(left) {
            return false;
        }
    }
    true
}

/// `queueA = factory(config, receiver.global.setTimeout);`
fn creation_a(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    shared: &mut Shared,
) -> Option<(Symbol, Symbol)> {
    let a = assignment(statement)?;
    let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
        return None;
    };
    let queue = crate::analysis::identifier_symbol(id, model.scoping)?;
    let queue = captured_symbol(model, site, queue)?;
    if model.binding_writes(queue) != 1 {
        return None;
    }
    let c = call(&a.right)?;
    let factory = captured(model, site, &c.callee)?;
    agree(&mut shared.factory, factory)?;
    let args = exact_arguments(c, 2)?;
    let config = captured(model, site, args[0])?;
    agree(&mut shared.config, config)?;
    let receiver = captured(
        model,
        site,
        member(member(args[1], "setTimeout")?, "global")?,
    )?;
    agree(&mut shared.receiver, receiver)?;
    Some((queue, factory))
}

/// `let queueB = factory(config, task => task());`
fn creation_b(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    factory: Symbol,
    config: Symbol,
) -> Option<(Symbol, Symbol)> {
    let variable = single_variable(statement)?;
    let queue = local_symbol(model, site, &variable.id)?;
    if model.binding_writes(queue) != 0 {
        return None;
    }
    let c = call(variable.init.as_ref()?)?;
    if symbol(model, &c.callee) != Some(factory) {
        return None;
    }
    let args = exact_arguments(c, 2)?;
    if symbol(model, args[0]) != Some(config) {
        return None;
    }
    let task = inline_caller(model, args[1])?;
    Some((queue, task))
}

fn start_alias(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    queue: Symbol,
) -> Option<Symbol> {
    let variable = single_variable(statement)?;
    let start = local_symbol(model, site, &variable.id)?;
    if model.binding_writes(start) != 0 {
        return None;
    }
    if symbol(model, member(variable.init.as_ref()?, "start")?) != Some(queue) {
        return None;
    }
    Some(start)
}

/// `if (typeof timer !== "function") start();
///  else if (flag) timer(start, 0);
///  else if (pending) pending.push(start);`
fn fallback(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    start: Symbol,
    shared: &mut Shared,
) -> Option<Timer> {
    let Statement::IfStatement(outer) = statement else {
        return None;
    };
    let timer = typeof_test(model, &outer.test)?;
    let [synchronous] = block(&outer.consequent)? else {
        return None;
    };
    let direct = statement_call(synchronous)?;
    if !direct.arguments.is_empty() || symbol(model, &direct.callee) != Some(start) {
        return None;
    }
    let Statement::IfStatement(middle) = outer.alternate.as_ref()? else {
        return None;
    };
    let flag = captured(model, site, &middle.test)?;
    agree(&mut shared.pending_flag, flag)?;
    let [scheduled] = block(&middle.consequent)? else {
        return None;
    };
    if !schedule_call(model, statement_call(scheduled)?, start, timer) {
        return None;
    }
    let Statement::IfStatement(tail) = middle.alternate.as_ref()? else {
        return None;
    };
    if tail.alternate.is_some() {
        return None;
    }
    let pending = captured(model, site, &tail.test)?;
    agree(&mut shared.pending_list, pending)?;
    let [queued] = block(&tail.consequent)? else {
        return None;
    };
    let push = statement_call(queued)?;
    if symbol(model, member(&push.callee, "push")?) != Some(pending) {
        return None;
    }
    let args = exact_arguments(push, 1)?;
    if symbol(model, args[0]) != Some(start) {
        return None;
    }
    (flag != pending).then_some(timer)
}

fn index_alias(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    parameter: Symbol,
) -> Option<Symbol> {
    let variable = single_variable(statement)?;
    let local = local_symbol(model, site, &variable.id)?;
    if model.binding_writes(local) != 0 || symbol(model, variable.init.as_ref()?) != Some(parameter)
    {
        return None;
    }
    Some(local)
}

fn member_alias(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    owner: Symbol,
    key: &str,
) -> Option<Symbol> {
    let variable = single_variable(statement)?;
    let local = local_symbol(model, site, &variable.id)?;
    if model.binding_writes(local) != 0
        || symbol(model, member(variable.init.as_ref()?, key)?) != Some(owner)
    {
        return None;
    }
    Some(local)
}

fn indexed_alias(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    slot: &mut Option<Symbol>,
    parameter: Symbol,
) -> Option<Symbol> {
    let variable = single_variable(statement)?;
    let local = local_symbol(model, site, &variable.id)?;
    if model.binding_writes(local) != 0 {
        return None;
    }
    let (table, key) = index(variable.init.as_ref()?)?;
    let table = captured(model, site, table)?;
    agree(slot, table)?;
    if symbol(model, key) != Some(parameter) {
        return None;
    }
    Some(local)
}

/// `errorLogger.bind(null, index, false)` or
/// `errorLogger.bind(null, groupIndices[0], true)`.
fn bind_call<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &'a Expression<'a>,
    shared: &mut Shared,
) -> Option<(Symbol, &'a Expression<'a>, bool)> {
    let c = call(e)?;
    let error_logger = captured(model, site, member(&c.callee, "bind")?)?;
    agree(&mut shared.error_logger, error_logger)?;
    let args = exact_arguments(c, 3)?;
    if !null_literal(args[0]) {
        return None;
    }
    let group = match args[2].without_parentheses() {
        Expression::BooleanLiteral(b) => b.value,
        _ => return None,
    };
    Some((error_logger, args[1], group))
}

fn catch_symbol(model: &Model<'_, '_>, handler: &oxc_ast::ast::CatchClause<'_>) -> Option<Symbol> {
    let BindingPattern::BindingIdentifier(caught) = &handler.param.as_ref()?.pattern else {
        return None;
    };
    let caught = caught.symbol_id.get()?;
    (model.binding_writes(caught) == 0 && model.scoping.symbol_redeclarations(caught).is_empty())
        .then_some(caught)
}

/// Registry row: one per-index subscription. The subscription local is either
/// introduced by the declaration form `let subscription = <call>.subscribe(...)`
/// or pre-declared as a bare `let subscription;` and assigned
/// `subscription = <call>.subscribe(...)` inside the `try`. Returns the
/// subscription local so the caller can prove the recorded unsubscribe callback
/// closes over it.
#[allow(clippy::too_many_arguments)]
fn subscription(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    declared: Option<Symbol>,
    ctor: Symbol,
    error_config: Symbol,
    priority: Symbol,
    debug_name: Symbol,
    idx: Symbol,
    queue_a: Symbol,
    queue_b: Symbol,
    shared: &mut Shared,
) -> Option<Symbol> {
    let (subscription, init) = match declared {
        None => {
            let variable = single_variable(statement)?;
            let subscription = local_symbol(model, site, &variable.id)?;
            if model.binding_writes(subscription) != 0 {
                return None;
            }
            (subscription, variable.init.as_ref()?)
        }
        Some(subscription) => {
            let write = assignment(statement)?;
            let AssignmentTarget::AssignmentTargetIdentifier(id) = &write.left else {
                return None;
            };
            if crate::analysis::identifier_symbol(id, model.scoping) != Some(subscription) {
                return None;
            }
            (subscription, &write.right)
        }
    };
    let subscribe = call(init)?;
    let ctor_call = call(member(&subscribe.callee, "subscribe")?)?;
    if symbol(model, &ctor_call.callee) != Some(ctor) {
        return None;
    }
    let subscribe_args = exact_arguments(subscribe, 1)?;
    let sink_factory_call = call(subscribe_args[0])?;
    let sink_factory = captured(model, site, &sink_factory_call.callee)?;
    agree(&mut shared.sink_factory, sink_factory)?;
    let sink_factory_args = exact_arguments(sink_factory_call, 1)?;
    if symbol(model, sink_factory_args[0]) != Some(idx) {
        return None;
    }
    let args = exact_arguments(ctor_call, 7)?;
    // arg0: makeOptions(errorConfig, receiver)
    let make_call = call(args[0])?;
    let make_options = captured(model, site, &make_call.callee)?;
    agree(&mut shared.make_options, make_options)?;
    let make_args = exact_arguments(make_call, 2)?;
    if symbol(model, make_args[0]) != Some(error_config) {
        return None;
    }
    if shared.receiver != Some(symbol(model, make_args[1])?) {
        return None;
    }
    // arg1/arg2: both queue enqueue channels, in order.
    if symbol(model, member(args[1], "enqueue")?) != Some(queue_a)
        || symbol(model, member(args[2], "enqueue")?) != Some(queue_b)
    {
        return None;
    }
    // arg3: priority; arg5: index; arg6: debugName.
    if symbol(model, args[3]) != Some(priority)
        || symbol(model, args[5]) != Some(idx)
        || symbol(model, args[6]) != Some(debug_name)
    {
        return None;
    }
    // arg4: value => valueSink(value, idx)
    let (params, body) = inline_parts(args[4])?;
    if !body.directives.is_empty() {
        return None;
    }
    let values = plain_params(model, params)?;
    let [value] = values.as_slice() else {
        return None;
    };
    let [sink_statement] = body.statements.as_slice() else {
        return None;
    };
    let sink_call = statement_call(sink_statement)?;
    let value_sink = captured(model, site, &sink_call.callee)?;
    agree(&mut shared.value_sink, value_sink)?;
    let sink_args = exact_arguments(sink_call, 2)?;
    if symbol(model, sink_args[0]) != Some(*value) || symbol(model, sink_args[1]) != Some(idx) {
        return None;
    }
    Some(subscription)
}

fn unsubscribe_push(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    subscription: Symbol,
    shared: &mut Shared,
) -> Option<()> {
    let push = statement_call(statement)?;
    let unsubscribers = captured(model, site, member(&push.callee, "push")?)?;
    agree(&mut shared.unsubscribers, unsubscribers)?;
    let args = exact_arguments(push, 1)?;
    let (params, body) = inline_parts(args[0])?;
    if !params.items.is_empty() || params.rest.is_some() || !body.directives.is_empty() {
        return None;
    }
    let [statement] = body.statements.as_slice() else {
        return None;
    };
    let c = statement_call(statement)?;
    if !c.arguments.is_empty()
        || symbol(model, member(&c.callee, "unsubscribe")?) != Some(subscription)
    {
        return None;
    }
    Some(())
}

/// First registry: ctor/priority/debugName row read, null skip, guarded
/// per-index subscription and recorded unsubscribe callback.
fn registry_iteration(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    queue_a: Symbol,
    queue_b: Symbol,
    shared: &mut Shared,
) -> Option<()> {
    let each = statement_call(statement)?;
    let registry = captured(model, site, member(&each.callee, "forEach")?)?;
    agree(&mut shared.registry, registry)?;
    if each.arguments.len() != 1 {
        return None;
    }
    let [Argument::ArrowFunctionExpression(callback)] = each.arguments.as_slice() else {
        return None;
    };
    if callback.r#async || callback.expression {
        return None;
    }
    let params = plain_params(model, &callback.params)?;
    let [entry, index] = params.as_slice() else {
        return None;
    };
    if entry == index || !callback.body.directives.is_empty() {
        return None;
    }
    let (declared_sub, idx_statement, ctor_statement, priority_statement, debug_statement, branch) =
        match callback.body.statements.as_slice() {
            [idx, ctor, priority, debug, branch] => (None, idx, ctor, priority, debug, branch),
            [idx, declared, ctor, priority, debug, branch] => (
                Some(bare_declaration(model, site, declared)?),
                idx,
                ctor,
                priority,
                debug,
                branch,
            ),
            _ => return None,
        };
    let idx = index_alias(model, site, idx_statement, *index)?;
    let ctor = member_alias(model, site, ctor_statement, *entry, "ctor")?;
    let priority = member_alias(model, site, priority_statement, *entry, "priority")?;
    let debug_name = member_alias(model, site, debug_statement, *entry, "debugName")?;
    let mut locals = vec![idx, ctor, priority, debug_name];
    if let Some(declared_sub) = declared_sub {
        locals.push(declared_sub);
    }
    if !pairwise_distinct(&locals) || locals.contains(entry) || locals.contains(index) {
        return None;
    }
    let Statement::IfStatement(branch) = branch else {
        return None;
    };
    if !null_compare(model, &branch.test, ctor) {
        return None;
    }
    let [absent] = block(&branch.consequent)? else {
        return None;
    };
    if !void_return(absent) {
        return None;
    }
    let alternate = block(branch.alternate.as_ref()?)?;
    let [Statement::TryStatement(attempt), completed] = alternate else {
        return None;
    };
    if attempt.finalizer.is_some() || !void_return(completed) {
        return None;
    }
    let handler = attempt.handler.as_ref()?;
    let caught = catch_symbol(model, handler)?;
    let [config_statement, subscribe_statement, record_statement] = attempt.block.body.as_slice()
    else {
        return None;
    };
    // errorConfig = { errorLogger: errorLogger.bind(null, index, false) }
    let config_variable = single_variable(config_statement)?;
    let error_config = local_symbol(model, site, &config_variable.id)?;
    if model.binding_writes(error_config) != 0 {
        return None;
    }
    let Expression::ObjectExpression(object) = config_variable.init.as_ref()?.without_parentheses()
    else {
        return None;
    };
    let [ObjectPropertyKind::ObjectProperty(property)] = object.properties.as_slice() else {
        return None;
    };
    if property.computed
        || property.method
        || property.kind != PropertyKind::Init
        || property.shorthand
        || property_key_name(&property.key)?.as_str() != "errorLogger"
    {
        return None;
    }
    let (error_logger, bound_index, group) = bind_call(model, site, &property.value, shared)?;
    if group || symbol(model, bound_index) != Some(*index) {
        return None;
    }
    let subscription = subscription(
        model,
        site,
        subscribe_statement,
        declared_sub,
        ctor,
        error_config,
        priority,
        debug_name,
        idx,
        queue_a,
        queue_b,
        shared,
    )?;
    unsubscribe_push(model, site, record_statement, subscription, shared)?;
    // catch (caught) { errorLogger(idx, false, caught); }
    let [forward] = handler.body.body.as_slice() else {
        return None;
    };
    let forward = statement_call(forward)?;
    if symbol(model, &forward.callee) != Some(error_logger) {
        return None;
    }
    let forward_args = exact_arguments(forward, 3)?;
    if symbol(model, forward_args[0]) != Some(idx)
        || !boolean(forward_args[1], false)
        || symbol(model, forward_args[2]) != Some(caught)
    {
        return None;
    }
    Some(())
}

fn index_zero(model: &Model<'_, '_>, e: &Expression<'_>, table: Symbol) -> bool {
    index(e).is_some_and(|(object, key)| symbol(model, object) == Some(table) && number(key, 0.0))
}

/// Inline group error literal: the connector's options argument carries the same
/// scoped error logger, `groupIndices[0]` bind target and `true` group marker as
/// the separate `groupError` object form, but with no local or assignment. Only
/// a direct object literal with the single `errorLogger` property is accepted;
/// spreads, computed/method/shorthand properties, extra properties and donor
/// identifiers are refused.
fn inline_group_error(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
    shared: &mut Shared,
    group_indices: Symbol,
) -> Option<()> {
    let Expression::ObjectExpression(object) = e.without_parentheses() else {
        return None;
    };
    let [ObjectPropertyKind::ObjectProperty(property)] = object.properties.as_slice() else {
        return None;
    };
    if property.computed
        || property.method
        || property.kind != PropertyKind::Init
        || property.shorthand
        || property_key_name(&property.key)?.as_str() != "errorLogger"
    {
        return None;
    }
    let (_error_logger, bound_index, group_marker) =
        bind_call(model, site, &property.value, shared)?;
    if !group_marker || !index_zero(model, bound_index, group_indices) {
        return None;
    }
    Some(())
}

/// Second registry: group index/spec tables, per-index sink list, group error
/// logger, group connection and recorded unsubscribe.
fn group_iteration(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    queue_a: Symbol,
    queue_b: Symbol,
    shared: &mut Shared,
) -> Option<()> {
    let each = statement_call(statement)?;
    let groups = captured(model, site, member(&each.callee, "forEach")?)?;
    agree(&mut shared.groups, groups)?;
    if each.arguments.len() != 1 {
        return None;
    }
    let [Argument::ArrowFunctionExpression(callback)] = each.arguments.as_slice() else {
        return None;
    };
    if callback.r#async || callback.expression {
        return None;
    }
    let params = plain_params(model, &callback.params)?;
    let [group, index] = params.as_slice() else {
        return None;
    };
    if group == index || !callback.body.directives.is_empty() {
        return None;
    }
    let (
        group_index_statement,
        group_spec_statement,
        sinks_statement,
        error_object_statement,
        error_assign_statement,
        options_statement,
        connect_statement,
        record_statement,
        inline_group,
    ) = match callback.body.statements.as_slice() {
        [group_index, group_spec, sinks, error_object, error_assign, connect, record] => (
            group_index,
            group_spec,
            sinks,
            Some(error_object),
            Some(error_assign),
            None,
            connect,
            record,
            false,
        ),
        [group_index, group_spec, sinks, error_object, error_assign, options, connect, record] => (
            group_index,
            group_spec,
            sinks,
            Some(error_object),
            Some(error_assign),
            Some(options),
            connect,
            record,
            false,
        ),
        [group_index, group_spec, sinks, connect, record] => (
            group_index,
            group_spec,
            sinks,
            None,
            None,
            None,
            connect,
            record,
            true,
        ),
        _ => return None,
    };
    let group_indices = indexed_alias(
        model,
        site,
        group_index_statement,
        &mut shared.group_index,
        *index,
    )?;
    let group_spec = indexed_alias(
        model,
        site,
        group_spec_statement,
        &mut shared.group_spec,
        *index,
    )?;
    if group_indices == group_spec || group_indices == *group || group_spec == *group {
        return None;
    }
    // let sinks = groupIndices.map(sinkFactory);
    let sinks_variable = single_variable(sinks_statement)?;
    let sinks = local_symbol(model, site, &sinks_variable.id)?;
    if model.binding_writes(sinks) != 1 {
        return None;
    }
    let map = call(sinks_variable.init.as_ref()?)?;
    if symbol(model, member(&map.callee, "map")?) != Some(group_indices) {
        return None;
    }
    let map_args = exact_arguments(map, 1)?;
    let sink_factory = captured(model, site, map_args[0])?;
    agree(&mut shared.sink_factory, sink_factory)?;
    // Separate group error object: `let groupError = { errorLogger: void 0 };`
    // and `groupError.errorLogger = errorLogger.bind(null, groupIndices[0], true);`.
    // The inline form carries that same binding directly in the connector
    // options literal instead, so it has no error statements here.
    let group_error = match (error_object_statement, error_assign_statement) {
        (Some(error_object_statement), Some(error_assign_statement)) => {
            let error_variable = single_variable(error_object_statement)?;
            let group_error = local_symbol(model, site, &error_variable.id)?;
            if model.binding_writes(group_error) != 0 {
                return None;
            }
            let Expression::ObjectExpression(error_object) =
                error_variable.init.as_ref()?.without_parentheses()
            else {
                return None;
            };
            let [ObjectPropertyKind::ObjectProperty(error_property)] =
                error_object.properties.as_slice()
            else {
                return None;
            };
            if error_property.computed
                || error_property.method
                || error_property.kind != PropertyKind::Init
                || error_property.shorthand
                || property_key_name(&error_property.key)?.as_str() != "errorLogger"
                || !unset(&error_property.value)
            {
                return None;
            }
            // groupError.errorLogger = errorLogger.bind(null, groupIndices[0], true);
            let error_assignment = assignment(error_assign_statement)?;
            let AssignmentTarget::StaticMemberExpression(error_target) = &error_assignment.left
            else {
                return None;
            };
            if error_target.optional
                || error_target.property.name != "errorLogger"
                || symbol(model, &error_target.object) != Some(group_error)
            {
                return None;
            }
            let (_error_logger, bound_index, group_marker) =
                bind_call(model, site, &error_assignment.right, shared)?;
            if !group_marker || !index_zero(model, bound_index, group_indices) {
                return None;
            }
            Some(group_error)
        }
        (None, None) if inline_group => None,
        _ => return None,
    };
    // Optional hoisted options: `let options = makeOptions(groupError, receiver);`
    // The connector then reads that one local. A declaration with a different
    // factory, error object or receiver, or a local used anywhere else, is
    // refused; the existing direct-call form is handled at the connector.
    let hoisted_options = match options_statement {
        None => None,
        Some(statement) => {
            let group_error = group_error?;
            let variable = single_variable(statement)?;
            let options = local_symbol(model, site, &variable.id)?;
            if model.binding_writes(options) != 0 {
                return None;
            }
            let make_call = call(variable.init.as_ref()?)?;
            let make_options = captured(model, site, &make_call.callee)?;
            if shared.make_options != Some(make_options) {
                return None;
            }
            let make_args = exact_arguments(make_call, 2)?;
            if symbol(model, make_args[0]) != Some(group_error)
                || shared.receiver != Some(symbol(model, make_args[1])?)
            {
                return None;
            }
            if options == sinks
                || options == group_error
                || options == group_indices
                || options == group_spec
                || options == *group
                || options == *index
            {
                return None;
            }
            Some(options)
        }
    };
    // sinks = groupConnector(sinks, group, makeOptions(groupError, receiver),
    //   queueA.enqueue, queueB.enqueue, groupSpec, value => valueSink(value, groupIndices[0]));
    let connect = assignment(connect_statement)?;
    let AssignmentTarget::AssignmentTargetIdentifier(connect_target) = &connect.left else {
        return None;
    };
    if crate::analysis::identifier_symbol(connect_target, model.scoping) != Some(sinks) {
        return None;
    }
    let connector = call(&connect.right)?;
    let group_connector = captured(model, site, &connector.callee)?;
    agree(&mut shared.group_connector, group_connector)?;
    let connect_args = exact_arguments(connector, 7)?;
    if symbol(model, connect_args[0]) != Some(sinks)
        || symbol(model, connect_args[1]) != Some(*group)
        || symbol(model, connect_args[5]) != Some(group_spec)
    {
        return None;
    }
    match (group_error, hoisted_options) {
        (Some(_), Some(options)) => {
            if symbol(model, connect_args[2]) != Some(options) {
                return None;
            }
        }
        (Some(group_error), None) => {
            let options = call(connect_args[2])?;
            if shared.make_options != Some(symbol(model, &options.callee)?) {
                return None;
            }
            let options_args = exact_arguments(options, 2)?;
            if symbol(model, options_args[0]) != Some(group_error)
                || shared.receiver != Some(symbol(model, options_args[1])?)
            {
                return None;
            }
        }
        (None, None) if inline_group => {
            let options = call(connect_args[2])?;
            if shared.make_options != Some(symbol(model, &options.callee)?) {
                return None;
            }
            let options_args = exact_arguments(options, 2)?;
            if shared.receiver != Some(symbol(model, options_args[1])?) {
                return None;
            }
            inline_group_error(model, site, options_args[0], shared, group_indices)?;
        }
        _ => return None,
    }
    if symbol(model, member(connect_args[3], "enqueue")?) != Some(queue_a)
        || symbol(model, member(connect_args[4], "enqueue")?) != Some(queue_b)
    {
        return None;
    }
    let (arrow_params, arrow_body) = inline_parts(connect_args[6])?;
    if !arrow_body.directives.is_empty() {
        return None;
    }
    let arrow_values = plain_params(model, arrow_params)?;
    let [value] = arrow_values.as_slice() else {
        return None;
    };
    let [sink_statement] = arrow_body.statements.as_slice() else {
        return None;
    };
    let sink_call = statement_call(sink_statement)?;
    let value_sink = captured(model, site, &sink_call.callee)?;
    agree(&mut shared.value_sink, value_sink)?;
    let sink_args = exact_arguments(sink_call, 2)?;
    if symbol(model, sink_args[0]) != Some(*value)
        || !index_zero(model, sink_args[1], group_indices)
    {
        return None;
    }
    // unsubscribers.push(sinks);
    let push = statement_call(record_statement)?;
    if shared.unsubscribers != Some(symbol(model, member(&push.callee, "push")?)?) {
        return None;
    }
    let push_args = exact_arguments(push, 1)?;
    if symbol(model, push_args[0]) != Some(sinks) {
        return None;
    }
    Some(())
}

fn start_registered_collectors(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [creation_a_statement, creation_b_statement, start_b_statement, start_b_fallback, start_a_statement, start_a_fallback, registry_statement, groups_statement, finished] =
        body.statements.as_slice()
    else {
        return None;
    };
    let mut shared = Shared::default();
    let (queue_a, factory) = creation_a(model, site, creation_a_statement, &mut shared)?;
    let (queue_b, task) = creation_b(model, site, creation_b_statement, factory, shared.config?)?;
    if queue_a == queue_b || task == queue_a || task == queue_b {
        return None;
    }
    let start_b = start_alias(model, site, start_b_statement, queue_b)?;
    let start_a = start_alias(model, site, start_a_statement, queue_a)?;
    if start_a == start_b || start_a == queue_a || start_b == queue_b {
        return None;
    }
    let timer_b = fallback(model, site, start_b_fallback, start_b, &mut shared)?;
    let timer_a = fallback(model, site, start_a_fallback, start_a, &mut shared)?;
    if timer_a != timer_b {
        return None;
    }
    registry_iteration(
        model,
        site,
        registry_statement,
        queue_a,
        queue_b,
        &mut shared,
    )?;
    group_iteration(model, site, groups_statement, queue_a, queue_b, &mut shared)?;
    if symbol(model, member(returned(finished)?, "onEmpty")?) != Some(queue_a) {
        return None;
    }
    // Shared consumer-input roles are pairwise distinct scoped identities.
    let consumer = [
        shared.factory?,
        shared.config?,
        shared.receiver?,
        shared.registry?,
        shared.groups?,
        shared.group_index?,
        shared.group_spec?,
        shared.error_logger?,
        shared.make_options?,
        shared.sink_factory?,
        shared.value_sink?,
        shared.group_connector?,
        shared.unsubscribers?,
        shared.pending_flag?,
        shared.pending_list?,
    ];
    let queues = [queue_a, queue_b];
    let starts = [start_a, start_b];
    if !pairwise_distinct(&consumer)
        || !pairwise_distinct(&queues)
        || !pairwise_distinct(&starts)
        || queues.iter().any(|q| consumer.contains(q))
        || starts
            .iter()
            .any(|s| consumer.contains(s) || queues.contains(s))
    {
        return None;
    }
    Some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return Vec::new();
    };
    let Some(body) = &function.body else {
        return Vec::new();
    };
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !function.params.items.is_empty()
        || !body.directives.is_empty()
        || start_registered_collectors(model, site, body).is_none()
    {
        return Vec::new();
    }
    vec![proposal(
        "startRegisteredCollectors",
        "shared_fast_collector_start",
        "complete nine-statement body: two queue objects from one provided factory call (the second with an inline one-formal pass-through scheduler); both queues started through the exact three-way timer/synchronous/pending-list fallback; a registry walk reading ctor/priority/debugName, skipping a null ctor, creating a per-index error logger, subscribing per-index through the provided options/sink/queue-enqueue protocol and recording an unsubscribe callback; a group-registry walk building per-index sink lists, a group error logger, connecting each group and recording its unsubscribe; returns the first queue's onEmpty; every consumer-provided role is one scoped identity across all uses and is pairwise distinct from the queues and start aliases",
        f,
    )]
}
