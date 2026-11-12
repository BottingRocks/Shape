//! Source-class proof with explicit callable-input epochs. This is a local
//! source implementation witness, never evidence of genuine host provenance.
//! Legacy class and ordinary-constructor rules are intentionally untouched.
use super::*;

type Barriers = BTreeSet<(u32, u32)>;

fn statements<'a>(m: &Model<'_, 'a>, owner: NodeId) -> Option<&'a [Statement<'a>]> {
    match m.nodes.kind(owner) {
        AstKind::FunctionBody(b) => Some(&b.statements),
        AstKind::BlockStatement(b) => Some(&b.body),
        AstKind::Program(p) => Some(&p.body),
        _ => None,
    }
}
fn looping(kind: AstKind<'_>) -> bool {
    matches!(
        kind,
        AstKind::ForStatement(_)
            | AstKind::ForInStatement(_)
            | AstKind::ForOfStatement(_)
            | AstKind::WhileStatement(_)
            | AstKind::DoWhileStatement(_)
    )
}
fn guarantees(s: &Statement<'_>, barriers: &Barriers, depth: usize) -> bool {
    if depth == 0 {
        return false;
    }
    match s {
        Statement::ExpressionStatement(e) => {
            barriers.contains(&(e.expression.span().start, e.expression.span().end))
        }
        Statement::VariableDeclaration(d) => d
            .declarations
            .iter()
            .any(|v| barriers.contains(&(v.span.start, v.span.end))),
        Statement::ClassDeclaration(c) => barriers.contains(&(c.span.start, c.span.end)),
        Statement::BlockStatement(b) => b.body.iter().any(|s| guarantees(s, barriers, depth - 1)),
        Statement::IfStatement(i) => {
            guarantees(&i.consequent, barriers, depth - 1)
                && i.alternate
                    .as_ref()
                    .is_some_and(|s| guarantees(s, barriers, depth - 1))
        }
        _ => false,
    }
}
struct Origin {
    owner: NodeId,
    barriers: Barriers,
    hoisted: bool,
}
// A closure's textual location is not a call. Follow every declaration/alias
// exposure, retaining a separate initialized-value proof at each alias read.
fn ready(
    m: &Model<'_, '_>,
    node: NodeId,
    origin: &Origin,
    active: &mut BTreeSet<Symbol>,
    depth: usize,
) -> bool {
    if depth == 0 {
        return false;
    }
    let mut child = node;
    let mut dominated = origin.hoisted;
    for _ in 0..256 {
        if child == origin.owner {
            return dominated;
        }
        let parent = m.nodes.parent_id(child);
        let kind = m.nodes.kind(child);
        if looping(kind)
            || matches!(
                kind,
                AstKind::WithStatement(_) | AstKind::LabeledStatement(_)
            )
        {
            return false;
        }
        if let AstKind::Function(f) = kind {
            if f.r#async || f.generator {
                return false;
            }
            if f.r#type == FunctionType::FunctionDeclaration {
                let Some(s) = f.id.as_ref().and_then(|id| id.symbol_id.get()) else {
                    return false;
                };
                if !immutable(m, s) || !active.insert(s) {
                    return false;
                }
                let okay = m
                    .scoping
                    .get_resolved_references(s)
                    .all(|r| ready(m, r.node_id(), origin, active, depth - 1));
                active.remove(&s);
                return okay;
            }
        }
        if matches!(kind, AstKind::ArrowFunctionExpression(a) if a.r#async) {
            return false;
        }
        let span = m.nodes.kind(child).span();
        if let Some(ss) = statements(m, parent) {
            dominated |= ss
                .iter()
                .take_while(|s| s.span().end <= span.start)
                .any(|s| guarantees(s, &origin.barriers, 32));
        }
        if parent == child {
            return false;
        }
        child = parent;
    }
    false
}
fn available(m: &Model<'_, '_>, origin: &Origin, node: NodeId) -> bool {
    ready(m, node, origin, &mut BTreeSet::new(), 32)
}
fn reference_node(m: &Model<'_, '_>, e: &Expression<'_>) -> Option<NodeId> {
    let Expression::Identifier(i) = e.without_parentheses() else {
        return None;
    };
    Some(m.scoping.get_reference(i.reference_id.get()?).node_id())
}
fn body_owner(m: &Model<'_, '_>, declaration: NodeId) -> Option<NodeId> {
    let mut owner = m.nodes.parent_id(declaration);
    if matches!(m.nodes.kind(owner), AstKind::VariableDeclaration(_)) {
        owner = m.nodes.parent_id(owner);
    }
    statements(m, owner).map(|_| owner)
}
fn direct_class_origin(m: &Model<'_, '_>, s: Symbol) -> Option<Origin> {
    if !immutable(m, s) {
        return None;
    }
    let declaration = m.scoping.symbol_declaration(s);
    let AstKind::Class(c) = m.nodes.kind(declaration) else {
        return None;
    };
    if c.r#type != ClassType::ClassDeclaration {
        return None;
    }
    Some(Origin {
        owner: body_owner(m, declaration)?,
        barriers: BTreeSet::from([(c.span.start, c.span.end)]),
        hoisted: false,
    })
}
fn ordinary_store_path(m: &Model<'_, '_>, mut child: NodeId, owner: NodeId) -> bool {
    for _ in 0..256 {
        if child == owner {
            return true;
        }
        if looping(m.nodes.kind(child))
            || matches!(
                m.nodes.kind(child),
                AstKind::Function(_)
                    | AstKind::ArrowFunctionExpression(_)
                    | AstKind::Class(_)
                    | AstKind::LabeledStatement(_)
            )
        {
            return false;
        }
        let parent = m.nodes.parent_id(child);
        if parent == child {
            return false;
        }
        child = parent;
    }
    false
}
/// Every mutable constructor-input write must install a directly declared,
/// already initialized custom source class. Unknown, deferred, compound or
/// loop writers refuse, including stores textually after a publication.
fn input_origin(m: &Model<'_, '_>, s: Symbol, depth: usize) -> Option<Origin> {
    if depth == 0 || !m.scoping.symbol_redeclarations(s).is_empty() {
        return None;
    }
    let declaration = m.scoping.symbol_declaration(s);
    match m.nodes.kind(declaration) {
        AstKind::Function(f)
            if immutable(m, s)
                && f.r#type == FunctionType::FunctionDeclaration
                && !f.r#async
                && !f.generator =>
        {
            Some(Origin {
                owner: body_owner(m, declaration)?,
                barriers: BTreeSet::new(),
                hoisted: true,
            })
        }
        AstKind::Class(_) => direct_class_origin(m, s),
        AstKind::VariableDeclarator(v) => {
            let owner = body_owner(m, declaration)?;
            if let Some(value) = &v.init {
                if !immutable(m, s) {
                    return None;
                }
                let producer = identifier(m, value)?;
                let source = input_origin(m, producer, depth - 1)?;
                if !available(m, &source, reference_node(m, value)?) {
                    return None;
                }
                return Some(Origin {
                    owner,
                    barriers: BTreeSet::from([(v.span.start, v.span.end)]),
                    hoisted: false,
                });
            }
            let mut barriers = BTreeSet::new();
            for r in m
                .scoping
                .get_resolved_references(s)
                .filter(|r| r.is_write())
            {
                let assignment = m.nodes.parent_id(r.node_id());
                let AstKind::AssignmentExpression(a) = m.nodes.kind(assignment) else {
                    return None;
                };
                let statement = m.nodes.parent_id(assignment);
                if a.operator.as_str() != "="
                    || !matches!(&a.left, AssignmentTarget::AssignmentTargetIdentifier(i) if crate::analysis::identifier_symbol(i,m.scoping)==Some(s))
                    || !matches!(m.nodes.kind(statement), AstKind::ExpressionStatement(_))
                    || !ordinary_store_path(m, statement, owner)
                {
                    return None;
                }
                let producer = direct_class_origin(m, identifier(m, &a.right)?)?;
                if !available(m, &producer, reference_node(m, &a.right)?) {
                    return None;
                }
                barriers.insert((a.span.start, a.span.end));
            }
            (!barriers.is_empty() && barriers.len() == m.binding_writes(s)).then_some(Origin {
                owner,
                barriers,
                hoisted: false,
            })
        }
        _ => None,
    }
}
fn input_symbol(m: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<Symbol> {
    let s = identifier(m, e)?;
    (captured(m, site, s) && input_origin(m, s, 12).is_some()).then_some(s)
}
fn function_input<'a>(m: &Model<'_, 'a>, s: Symbol, depth: usize) -> Option<&'a Function<'a>> {
    if depth == 0 || !immutable(m, s) {
        return None;
    }
    match m.nodes.kind(m.scoping.symbol_declaration(s)) {
        AstKind::Function(f) if f.r#type == FunctionType::FunctionDeclaration => Some(f),
        AstKind::VariableDeclarator(v) => {
            function_input(m, identifier(m, v.init.as_ref()?)?, depth - 1)
        }
        _ => None,
    }
}
fn declaration<'a>(s: &'a Statement<'a>) -> Option<&'a Function<'a>> {
    match s {
        Statement::FunctionDeclaration(f) => Some(f),
        _ => None,
    }
}
fn function_symbol(m: &Model<'_, '_>, f: &Function<'_>) -> Option<Symbol> {
    let s = f.id.as_ref()?.symbol_id.get()?;
    immutable(m, s).then_some(s)
}
fn alias_of(m: &Model<'_, '_>, s: &Statement<'_>, from: Symbol) -> Option<Symbol> {
    let v = one_variable(s)?;
    let to = local(m, v)?;
    (immutable(m, to) && identifier(m, v.init.as_ref()?) == Some(from)).then_some(to)
}
fn forward(
    m: &Model<'_, '_>,
    f: &Function<'_>,
    sink: Symbol,
    key: &str,
    count: usize,
    returned: bool,
) -> Option<()> {
    let ps = plain_params(m, f)?;
    if ps.len() != count {
        return None;
    }
    let [s] = body_of(f)?.statements.as_slice() else {
        return None;
    };
    let args = observer_call(
        m,
        if returned {
            return_value(s)?
        } else {
            expression_statement(s)?
        },
        sink,
        key,
        count,
    )?;
    (args
        .iter()
        .zip(ps)
        .all(|(a, p)| identifier(m, a) == Some(p)))
    .then_some(())
}
fn install(
    m: &Model<'_, '_>,
    s: &Statement<'_>,
    object: Symbol,
    key: &str,
    value: Symbol,
) -> Option<()> {
    (identifier(m, assignment_value(m, s, object, key)?) == Some(value)).then_some(())
}
fn id_store(m: &Model<'_, '_>, s: &Statement<'_>, to: Symbol, from: Symbol) -> Option<()> {
    (identifier(m, assignment_value_to_identifier(m, s, to)?) == Some(from)).then_some(())
}
fn only_references(m: &Model<'_, '_>, symbols: &[Symbol], count: usize) -> bool {
    symbols.iter().all(|s| m.binding_references(*s) == count)
}

struct Operator<'a> {
    projection: Symbol,
    input: Symbol,
    executor: &'a ArrowFunctionExpression<'a>,
    constructor: Symbol,
}
fn operator_head<'a>(m: &Model<'_, 'a>, s: Symbol) -> Option<Operator<'a>> {
    let factory = function_input(m, s, 12)?;
    let projection_params = plain_params(m, factory)?;
    let [projection_param] = projection_params.as_slice() else {
        return None;
    };
    let projection_param = *projection_param;
    let [capture, inner, finish] = body_of(factory)?.statements.as_slice() else {
        return None;
    };
    let projection = alias_of(m, capture, projection_param)?;
    let inner = declaration(inner)?;
    let inner_symbol = function_symbol(m, inner)?;
    if identifier(m, return_value(finish)?) != Some(inner_symbol)
        || !only_references(m, &[inner_symbol], 1)
    {
        return None;
    }
    let ps = plain_params(m, inner)?;
    let [source] = ps.as_slice() else {
        return None;
    };
    let [capture, finish] = body_of(inner)?.statements.as_slice() else {
        return None;
    };
    let input = alias_of(m, capture, *source)?;
    let new = new_expression(return_value(finish)?)?;
    let constructor = identifier(m, &new.callee)?;
    let args = exact_new_arguments(new, 1)?;
    let Expression::ArrowFunctionExpression(executor) = args[0].without_parentheses() else {
        return None;
    };
    if executor.r#async
        || executor.expression
        || !executor.body.directives.is_empty()
        || plain_arrow_params(m, executor)?.len() != 1
    {
        return None;
    }
    Some(Operator {
        projection,
        input,
        executor,
        constructor,
    })
}
fn map_operator(m: &Model<'_, '_>, s: Symbol) -> Option<Symbol> {
    let op = operator_head(m, s)?;
    let receiver = plain_arrow_params(m, op.executor)?[0];
    let [complete, capture, next, observer, error, set_error, set_complete, subscribe, cleanup, finish] =
        op.executor.body.statements.as_slice()
    else {
        return None;
    };
    let sink = alias_of(m, capture, receiver)?;
    let complete = declaration(complete)?;
    let error = declaration(error)?;
    let next = declaration(next)?;
    let cleanup = declaration(cleanup)?;
    let cs = function_symbol(m, complete)?;
    let es = function_symbol(m, error)?;
    let ns = function_symbol(m, next)?;
    let ts = function_symbol(m, cleanup)?;
    if !only_references(m, &[cs, es, ns, ts], 1) {
        return None;
    }
    forward(m, complete, sink, "complete", 0, false)?;
    forward(m, error, sink, "error", 1, false)?;
    let value = plain_params(m, next)?;
    let [value] = value.as_slice() else {
        return None;
    };
    let [Statement::TryStatement(t)] = body_of(next)?.statements.as_slice() else {
        return None;
    };
    if t.finalizer.is_some() {
        return None;
    }
    let [emit] = t.block.body.as_slice() else {
        return None;
    };
    let args = observer_call(m, expression_statement(emit)?, sink, "next", 1)?;
    let projection = plain_call(args[0])?;
    if identifier(m, &projection.callee) != Some(op.projection)
        || identifier(m, exact_arguments(projection, 1)?[0]) != Some(*value)
    {
        return None;
    }
    let caught = t.handler.as_ref()?;
    let caught_symbol = plain_binding(m, &caught.param.as_ref()?.pattern)?;
    let [report] = caught.body.body.as_slice() else {
        return None;
    };
    if identifier(
        m,
        observer_call(m, expression_statement(report)?, sink, "error", 1)?[0],
    ) != Some(caught_symbol)
    {
        return None;
    }
    let v = one_variable(observer)?;
    let observer = local(m, v)?;
    if !immutable(m, observer) {
        return None;
    }
    let Expression::ObjectExpression(o) = v.init.as_ref()?.without_parentheses() else {
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
        || identifier(m, &p.value) != Some(ns)
    {
        return None;
    }
    install(m, set_error, observer, "error", es)?;
    install(m, set_complete, observer, "complete", cs)?;
    let v = one_variable(subscribe)?;
    let subscription = local(m, v)?;
    if !immutable(m, subscription)
        || identifier(
            m,
            observer_call(m, v.init.as_ref()?, op.input, "subscribe", 1)?[0],
        ) != Some(observer)
    {
        return None;
    }
    forward(m, cleanup, subscription, "unsubscribe", 0, true)?;
    if identifier(m, return_value(finish)?) != Some(ts) {
        return None;
    }
    Some(op.constructor)
}
fn flat_map_operator(m: &Model<'_, '_>, s: Symbol) -> Option<Symbol> {
    let op = operator_head(m, s)?;
    let receiver = plain_arrow_params(m, op.executor)?[0];
    let [capture, pending, error, empty, count, next, next_alias, error_store, complete, complete_store, subscribe, push, cleanup, finish] =
        op.executor.body.statements.as_slice()
    else {
        return None;
    };
    let sink = alias_of(m, capture, receiver)?;
    let Statement::VariableDeclaration(pending) = pending else {
        return None;
    };
    if pending.declarations.len() != 4 {
        return None;
    }
    let slots: Option<Vec<_>> = pending
        .declarations
        .iter()
        .map(|v| if v.init.is_none() { local(m, v) } else { None })
        .collect();
    let slots = slots?;
    let [handles, error_slot, count_slot, complete_slot] = slots.as_slice() else {
        return None;
    };
    if !distinct(&slots) {
        return None;
    }
    let error = declaration(error)?;
    let next = declaration(next)?;
    let complete = declaration(complete)?;
    let cleanup = declaration(cleanup)?;
    let es = function_symbol(m, error)?;
    let ns = function_symbol(m, next)?;
    let cs = function_symbol(m, complete)?;
    let ts = function_symbol(m, cleanup)?;
    if !only_references(m, &[es, ns, cs, ts], 1) {
        return None;
    }
    forward(m, error, sink, "error", 1, true)?;
    forward(m, next, sink, "next", 1, true)?;
    let Expression::ArrayExpression(a) =
        assignment_value_to_identifier(m, empty, *handles)?.without_parentheses()
    else {
        return None;
    };
    if !a.elements.is_empty()
        || !is_number(assignment_value_to_identifier(m, count, *count_slot)?, 1.0)
    {
        return None;
    }
    let next_slot = alias_of(m, next_alias, ns)?;
    id_store(m, error_store, *error_slot, es)?;
    id_store(m, complete_store, *complete_slot, cs)?;
    if !plain_params(m, complete)?.is_empty() {
        return None;
    }
    let [decrement, Statement::IfStatement(done)] = body_of(complete)?.statements.as_slice() else {
        return None;
    };
    update(m, expression_statement(decrement)?, *count_slot, "--")?;
    let (left, right) = binary(&done.test, "<=")?;
    if identifier(m, left) != Some(*count_slot)
        || !is_number(right, 0.0)
        || done.alternate.is_some()
    {
        return None;
    }
    let [complete_call] = block(&done.consequent)? else {
        return None;
    };
    observer_call(m, expression_statement(complete_call)?, sink, "complete", 0)?;
    let v = one_variable(subscribe)?;
    let outer = local(m, v)?;
    let args = observer_call(m, v.init.as_ref()?, op.input, "subscribe", 3)?;
    if identifier(m, args[1]) != Some(*error_slot) || identifier(m, args[2]) != Some(*complete_slot)
    {
        return None;
    }
    let Expression::ArrowFunctionExpression(emit) = args[0].without_parentheses() else {
        return None;
    };
    let ps = plain_arrow_params(m, emit)?;
    let [value] = ps.as_slice() else {
        return None;
    };
    let [increment, inner, push_inner] = emit.body.statements.as_slice() else {
        return None;
    };
    if emit.expression || emit.r#async || !emit.body.directives.is_empty() {
        return None;
    }
    update(m, expression_statement(increment)?, *count_slot, "++")?;
    let v = one_variable(inner)?;
    let inner = local(m, v)?;
    let call = method_call(v.init.as_ref()?, "subscribe")?;
    let projection = plain_call(receiver_of(&call.callee, "subscribe")?)?;
    if identifier(m, &projection.callee) != Some(op.projection)
        || identifier(m, exact_arguments(projection, 1)?[0]) != Some(*value)
    {
        return None;
    }
    let args = exact_arguments(call, 3)?;
    if args
        .iter()
        .zip([next_slot, *error_slot, *complete_slot])
        .any(|(e, s)| identifier(m, e) != Some(s))
    {
        return None;
    }
    if identifier(
        m,
        observer_call(m, expression_statement(push_inner)?, *handles, "push", 1)?[0],
    ) != Some(inner)
        || identifier(
            m,
            observer_call(m, expression_statement(push)?, *handles, "push", 1)?[0],
        ) != Some(outer)
    {
        return None;
    }
    if !plain_params(m, cleanup)?.is_empty() {
        return None;
    }
    let [run] = body_of(cleanup)?.statements.as_slice() else {
        return None;
    };
    let args = observer_call(m, return_value(run)?, *handles, "forEach", 1)?;
    let Expression::ArrowFunctionExpression(unsubscribe) = args[0].without_parentheses() else {
        return None;
    };
    let ps = plain_arrow_params(m, unsubscribe)?;
    let [handle] = ps.as_slice() else {
        return None;
    };
    observer_call(m, arrow_value(unsubscribe)?, *handle, "unsubscribe", 0)?;
    if identifier(m, return_value(finish)?) != Some(ts)
        || !immutable(m, outer)
        || !immutable(m, inner)
        || m.binding_writes(*handles) != 1
        || m.binding_writes(*error_slot) != 1
        || m.binding_writes(*complete_slot) != 1
        || m.binding_writes(*count_slot) != 3
    {
        return None;
    }
    Some(op.constructor)
}
fn update(m: &Model<'_, '_>, e: &Expression<'_>, slot: Symbol, operator: &str) -> Option<()> {
    let Expression::UpdateExpression(u) = e.without_parentheses() else {
        return None;
    };
    (u.operator.as_str()==operator && matches!(&u.argument,SimpleAssignmentTarget::AssignmentTargetIdentifier(i) if crate::analysis::identifier_symbol(i,m.scoping)==Some(slot))).then_some(())
}
fn teardown_adapter(m: &Model<'_, '_>, s: Symbol) -> Option<()> {
    let f = function_input(m, s, 12)?;
    let ps = plain_params(m, f)?;
    let [feed, sink] = ps.as_slice() else {
        return None;
    };
    let [value, Statement::IfStatement(absent), Statement::IfStatement(callable), Statement::IfStatement(subscription)] =
        body_of(f)?.statements.as_slice()
    else {
        return None;
    };
    let v = one_variable(value)?;
    let cleanup = local(m, v)?;
    let c = plain_call(v.init.as_ref()?)?;
    if !immutable(m, cleanup)
        || identifier(m, &c.callee) != Some(*feed)
        || identifier(m, exact_arguments(c, 1)?[0]) != Some(*sink)
    {
        return None;
    }
    let (left, right) = binary(&absent.test, "==")?;
    let [finish] = block(&absent.consequent)? else {
        return None;
    };
    if absent.alternate.is_some()
        || identifier(m, left) != Some(cleanup)
        || !is_null(right)
        || !is_void_zero(return_value(finish)?)
    {
        return None;
    }
    let [finish] = block(&callable.consequent)? else {
        return None;
    };
    if callable.alternate.is_some()
        || identifier(m, typeof_test(&callable.test, "===", "function")?) != Some(cleanup)
        || identifier(m, return_value(finish)?) != Some(cleanup)
    {
        return None;
    }
    let (left, right) = binary(&subscription.test, "===")?;
    if !member_of(m, left, cleanup, "unsubscribe") || !is_void_zero(right) {
        return None;
    }
    let [throw] = block(&subscription.consequent)? else {
        return None;
    };
    if !unbound(m, throw_expression(throw)?, "TypeError") {
        return None;
    }
    let [decl, finish] = block(subscription.alternate.as_ref()?)? else {
        return None;
    };
    let callback = declaration(decl)?;
    let callback_symbol = function_symbol(m, callback)?;
    forward(m, callback, cleanup, "unsubscribe", 0, true)?;
    (identifier(m, return_value(finish)?) == Some(callback_symbol)
        && only_references(m, &[callback_symbol], 1))
    .then_some(())
}
fn source_subscribe_method(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    class: Symbol,
    alias: Symbol,
    extended: bool,
) -> Option<()> {
    let params = plain_params(model, function)?;
    let [observer, next_slot, complete_slot] = params.as_slice() else {
        return None;
    };
    let (handle, branch) = legacy_normalization(
        model,
        function,
        *observer,
        *next_slot,
        *complete_slot,
    )
    .or_else(|| {
        extended
            .then(|| extended_normalization(model, function, *observer, *next_slot, *complete_slot))
            .flatten()
    })?;
    let Statement::IfStatement(branch) = branch else {
        return None;
    };
    let (left, right) = binary(&branch.test, "==")?;
    if !this_member(left, "_subscriber") || !is_null(right) {
        return None;
    }
    let rejected = block(&branch.consequent)?;
    let [throw] = rejected else {
        return None;
    };
    let new = new_expression(throw_expression(throw)?)?;
    if !unbound(model, &new.callee, "TypeError") || !exact_new_arguments(new, 0)?.is_empty() {
        return None;
    }
    let accepted = block(branch.alternate.as_ref()?)?;
    let [observer_new, start_branch, wrapper_new, cleanup_try, release_branch, finish] = accepted
    else {
        return None;
    };
    // new ObserverCtor(normalized)
    let observer_new = one_variable(observer_new)?;
    let subscription = local(model, observer_new)?;
    let new = new_expression(observer_new.init.as_ref()?)?;
    let observer_ctor = input_symbol(model, site, &new.callee)?;
    if identifier(model, exact_new_arguments(new, 1)?[0]) != Some(handle) {
        return None;
    }
    // if (typeof normalized.start === "function") normalized.start(subscription);
    let Statement::IfStatement(start) = start_branch else {
        return None;
    };
    if start.alternate.is_some()
        || !member_of(
            model,
            typeof_test(&start.test, "===", "function")?,
            handle,
            "start",
        )
    {
        return None;
    }
    let started = block(&start.consequent)?;
    match started {
        [start_call] => {
            let start_call = method_call(expression_statement(start_call)?, "start")?;
            if !receiver_of(&start_call.callee, "start")
                .is_some_and(|receiver| identifier(model, receiver) == Some(handle))
                || identifier(model, exact_arguments(start_call, 1)?[0]) != Some(subscription)
            {
                return None;
            }
        }
        [bundle, call] => {
            let (receiver, method, arguments) = bundled_call(model, bundle, call)?;
            if receiver != handle
                || method != "start"
                || arguments.len() != 1
                || identifier(model, arguments[0]) != Some(subscription)
            {
                return None;
            }
        }
        _ => return None,
    }
    // new WrapperCtor(subscription)
    let wrapper_new = one_variable(wrapper_new)?;
    let wrapper = local(model, wrapper_new)?;
    let new = new_expression(wrapper_new.init.as_ref()?)?;
    let wrapper_ctor = input_symbol(model, site, &new.callee)?;
    if identifier(model, exact_new_arguments(new, 1)?[0]) != Some(subscription) {
        return None;
    }
    // try { subscription._cleanup = Teardown(this._subscriber, wrapper); }
    // catch (e) { wrapper.error(e); }
    let Statement::TryStatement(cleanup_try) = cleanup_try else {
        return None;
    };
    if cleanup_try.finalizer.is_some() {
        return None;
    }
    let [cleanup] = cleanup_try.block.body.as_slice() else {
        return None;
    };
    let cleanup = expression_statement(cleanup)?;
    let Expression::AssignmentExpression(cleanup) = cleanup.without_parentheses() else {
        return None;
    };
    let AssignmentTarget::StaticMemberExpression(target) = &cleanup.left else {
        return None;
    };
    if cleanup.operator.as_str() != "="
        || target.optional
        || target.property.name != "_cleanup"
        || identifier(model, &target.object) != Some(subscription)
    {
        return None;
    }
    let teardown_call = plain_call(&cleanup.right)?;
    let teardown = input_symbol(model, site, &teardown_call.callee)?;
    let args = exact_arguments(teardown_call, 2)?;
    let [feed, wrapped] = args.as_slice() else {
        return None;
    };
    if !this_member(feed, "_subscriber") || identifier(model, wrapped) != Some(wrapper) {
        return None;
    }
    let handler = cleanup_try.handler.as_ref()?;
    let BindingPattern::BindingIdentifier(error) = &handler.param.as_ref()?.pattern else {
        return None;
    };
    let error = error.symbol_id.get()?;
    match handler.body.body.as_slice() {
        [report] => {
            let args = observer_call(model, expression_statement(report)?, wrapper, "error", 1)?;
            if identifier(model, args[0]) != Some(error) {
                return None;
            }
        }
        [bundle, call] => {
            let (receiver, method, arguments) = bundled_call(model, bundle, call)?;
            if receiver != wrapper
                || method != "error"
                || arguments.len() != 1
                || identifier(model, arguments[0]) != Some(error)
            {
                return None;
            }
        }
        _ => return None,
    }
    // if (subscription._observer === void 0) { ... }
    let Statement::IfStatement(release) = release_branch else {
        return None;
    };
    if release.alternate.is_some() {
        return None;
    }
    let (left, right) = binary(&release.test, "===")?;
    if !member_of(model, left, subscription, "_observer") || !is_void_zero(right) {
        return None;
    }
    let released = block(&release.consequent)?;
    let [cleanup_value, inner] = released else {
        return None;
    };
    let cleanup_variable = one_variable(cleanup_value)?;
    let cleanup_symbol = local(model, cleanup_variable)?;
    if !member_of(
        model,
        cleanup_variable.init.as_ref()?,
        subscription,
        "_cleanup",
    ) || !immutable(model, cleanup_symbol)
    {
        return None;
    }
    let Statement::IfStatement(inner) = inner else {
        return None;
    };
    if inner.alternate.is_some() {
        return None;
    }
    let (left, right) = binary(&inner.test, "!==")?;
    if identifier(model, left) != Some(cleanup_symbol) || !is_void_zero(right) {
        return None;
    }
    let body = block(&inner.consequent)?;
    let [clear, run] = body else {
        return None;
    };
    let clear = expression_statement(clear)?;
    let Expression::AssignmentExpression(clear) = clear.without_parentheses() else {
        return None;
    };
    let AssignmentTarget::StaticMemberExpression(clear_target) = &clear.left else {
        return None;
    };
    if clear.operator.as_str() != "="
        || clear_target.optional
        || clear_target.property.name != "_cleanup"
        || identifier(model, &clear_target.object) != Some(subscription)
        || !is_void_zero(&clear.right)
    {
        return None;
    }
    let Statement::TryStatement(run) = run else {
        return None;
    };
    if run.finalizer.is_some() {
        return None;
    }
    let [run_call] = run.block.body.as_slice() else {
        return None;
    };
    let run_call = plain_call(expression_statement(run_call)?)?;
    if identifier(model, &run_call.callee) != Some(cleanup_symbol)
        || !exact_arguments(run_call, 0)?.is_empty()
    {
        return None;
    }
    let run_handler = run.handler.as_ref()?;
    if !run_handler.body.body.is_empty() {
        return None;
    }
    // return subscription;
    if identifier(model, return_value(finish)?) != Some(subscription) {
        return None;
    }
    if !distinct(&[observer_ctor, wrapper_ctor, teardown, subscription])
        || [observer_ctor, wrapper_ctor, teardown]
            .iter()
            .any(|identity| *identity == class || *identity == alias)
    {
        return None;
    }
    Some(())
}

struct BodyWitness {
    class_span: Span,
    class_symbol: Symbol,
    constructor_input: Symbol,
    inputs: [Symbol; 5],
}
fn operator_capture(m: &Model<'_, '_>, site: FunctionSite, f: &Function<'_>) -> Option<Symbol> {
    operator_method(m, site, f)?;
    let [finish] = body_of(f)?.statements.as_slice() else {
        return None;
    };
    let pipe = method_call(return_value(finish)?, "pipe")?;
    let operator = plain_call(exact_arguments(pipe, 1)?[0])?;
    input_symbol(m, site, &operator.callee)
}
fn subscribe_inputs(
    m: &Model<'_, '_>,
    site: FunctionSite,
    f: &Function<'_>,
) -> Option<[Symbol; 3]> {
    let ps = plain_params(m, f)?;
    let [a, b, c] = ps.as_slice() else {
        return None;
    };
    let (_, branch) = legacy_normalization(m, f, *a, *b, *c)
        .or_else(|| extended_normalization(m, f, *a, *b, *c))?;
    let Statement::IfStatement(branch) = branch else {
        return None;
    };
    let [observer, _, wrapper, Statement::TryStatement(teardown), _, _] =
        block(branch.alternate.as_ref()?)?
    else {
        return None;
    };
    let observer = new_expression(one_variable(observer)?.init.as_ref()?)?;
    let wrapper = new_expression(one_variable(wrapper)?.init.as_ref()?)?;
    let [cleanup] = teardown.block.body.as_slice() else {
        return None;
    };
    let Expression::AssignmentExpression(cleanup) =
        expression_statement(cleanup)?.without_parentheses()
    else {
        return None;
    };
    let teardown = plain_call(&cleanup.right)?;
    Some([
        input_symbol(m, site, &observer.callee)?,
        input_symbol(m, site, &wrapper.callee)?,
        input_symbol(m, site, &teardown.callee)?,
    ])
}
fn body_proof(m: &Model<'_, '_>, site: FunctionSite) -> Option<BodyWitness> {
    let AstKind::Function(f) = m.nodes.kind(site.node_id) else {
        return None;
    };
    if !plain_params(m, f)?.is_empty() || f.r#type != FunctionType::FunctionDeclaration {
        return None;
    }
    let body = body_of(f)?;
    let [Statement::ClassDeclaration(class), alias_statement, finish] = body.statements.as_slice()
    else {
        return None;
    };
    if class.super_class.is_some() || !class.decorators.is_empty() {
        return None;
    }
    // A method decorator can execute while the class is created and replace
    // the body below. Such a class does not have this complete source API.
    if class.body.body.iter().any(|element| {
        matches!(element, ClassElement::MethodDefinition(method) if !method.decorators.is_empty())
    }) {
        return None;
    }
    let cs = class.id.as_ref()?.symbol_id.get()?;
    let v = one_variable(alias_statement)?;
    let alias = local(m, v)?;
    if !immutable(m, cs)
        || !immutable(m, alias)
        || cs == alias
        || identifier(m, v.init.as_ref()?) != Some(cs)
        || identifier(m, return_value(finish)?) != Some(alias)
    {
        return None;
    }
    let api = class_api(class)?;
    constructor_method(m, api.constructor)?;
    of_method(m, api.of, cs, alias)?;
    empty_method(m, api.empty, cs, alias)?;
    from_method(m, api.from, cs, alias, true, true)?;
    source_subscribe_method(m, site, api.subscribe, cs, alias, true)?;
    pipe_method(m, api.pipe)?;
    let map = operator_capture(m, site, api.map)?;
    let flat = operator_capture(m, site, api.unordered_flat_map)?;
    if map == flat {
        return None;
    }
    let constructor_input = map_operator(m, map)?;
    if flat_map_operator(m, flat)? != constructor_input {
        return None;
    }
    let [observer, wrapper, teardown] = subscribe_inputs(m, site, api.subscribe)?;
    teardown_adapter(m, teardown)?;
    if !distinct(&[observer, wrapper, teardown, map, flat, constructor_input]) {
        return None;
    }
    let mut callbacks = SynchronousCallbacks(true);
    callbacks.visit_function_body(body);
    if !callbacks.0 {
        return None;
    }
    Some(BodyWitness {
        class_span: class.span,
        class_symbol: cs,
        constructor_input,
        inputs: [observer, wrapper, teardown, map, flat],
    })
}
fn site_for_symbol(m: &Model<'_, '_>, s: Symbol) -> Option<FunctionSite> {
    if !immutable(m, s) {
        return None;
    }
    let node = m.scoping.symbol_declaration(s);
    m.all_sites()
        .iter()
        .copied()
        .find(|site| site.node_id == node && site.binding_symbol == Some(s))
}
fn result_reads_are_closed(m: &Model<'_, '_>, s: Symbol) -> bool {
    m.scoping.get_resolved_references(s).filter(|r|r.is_read()).all(|r| {
        let node=r.node_id();let span=m.nodes.kind(node).span();let parent=m.nodes.parent_id(node);
        match m.nodes.kind(parent) {
            AstKind::NewExpression(n)=>n.callee.span()==span,
            AstKind::StaticMemberExpression(p) if !p.optional && p.object.span()==span && matches!(p.property.name.as_str(),"of"|"from"|"empty")=> {
                matches!(m.nodes.kind(m.nodes.parent_id(parent)),AstKind::CallExpression(c) if !c.optional && c.callee.span()==p.span)
            }
            AstKind::BinaryExpression(b)=>b.operator.as_str()=="instanceof" && b.right.span()==span,
            _=>false,
        }
    })
}
/// Close a finite group of result-constructor definitions. Each RHS maker has
/// its own complete class/API body proof; no maker borrows the current maker's
/// class identity. Class methods cannot run during construction of their own
/// class object. The only maker exposure is its direct result store, and the
/// result may only be constructed or used through the proved static API.
/// Thus the operators' output constructor is available before any returned
/// class can publish/execute its operator methods. Future writes must preserve
/// this same independently proved contract; an unknown late writer refuses.
pub(super) fn prove(m: &Model<'_, '_>, site: FunctionSite) -> Option<(Span, Symbol)> {
    if m.nodes.iter().any(|n|matches!(n.kind(),AstKind::Program(p) if crate::naming::program::has_dynamic_scope(p))) { return None; }
    let own = body_proof(m, site)?;
    let result = own.constructor_input;
    if !m.scoping.symbol_redeclarations(result).is_empty() || !result_reads_are_closed(m, result) {
        return None;
    }
    let declaration = m.scoping.symbol_declaration(result);
    let AstKind::VariableDeclarator(v) = m.nodes.kind(declaration) else {
        return None;
    };
    if v.init.is_some() {
        return None;
    }
    let owner = body_owner(m, declaration)?;
    let mut stores = BTreeSet::new();
    let mut found = false;
    for r in m
        .scoping
        .get_resolved_references(result)
        .filter(|r| r.is_write())
    {
        let assignment = m.nodes.parent_id(r.node_id());
        let AstKind::AssignmentExpression(a) = m.nodes.kind(assignment) else {
            return None;
        };
        let statement = m.nodes.parent_id(assignment);
        if a.operator.as_str() != "="
            || !matches!(&a.left,AssignmentTarget::AssignmentTargetIdentifier(i) if crate::analysis::identifier_symbol(i,m.scoping)==Some(result))
            || !matches!(m.nodes.kind(statement), AstKind::ExpressionStatement(_))
            || !ordinary_store_path(m, statement, owner)
        {
            return None;
        }
        let call = plain_call(&a.right)?;
        if !exact_arguments(call, 0)?.is_empty() {
            return None;
        }
        let maker = site_for_symbol(m, identifier(m, &call.callee)?)?;
        let witness = body_proof(m, maker)?;
        if witness.constructor_input != result || witness.inputs != own.inputs {
            return None;
        }
        let refs: Vec<_> = m
            .scoping
            .get_resolved_references(maker.binding_symbol?)
            .collect();
        if refs.len() != 1 || m.nodes.kind(refs[0].node_id()).span() != call.callee.span() {
            return None;
        }
        // Every capture producer/copy is checked separately from execution
        // readiness, at every actual publication, not the last textual store.
        for input in witness.inputs {
            let origin = input_origin(m, input, 12)?;
            if !available(m, &origin, refs[0].node_id()) {
                return None;
            }
        }
        found |= maker.node_id == site.node_id;
        stores.insert((a.span.start, a.span.end));
    }
    (found && !stores.is_empty() && stores.len() == m.binding_writes(result))
        .then_some((own.class_span, own.class_symbol))
}

#[cfg(test)]
#[path = "observable_factory_source_class_tests.rs"]
mod source_class_tests;
