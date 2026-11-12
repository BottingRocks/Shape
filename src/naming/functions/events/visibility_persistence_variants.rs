//! Closed visibility-duration persistence protocol; external transform purpose is unproved.
use super::*;
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e.without_parentheses(), model.scoping)
}

fn plain_binding(model: &Model<'_, '_>, p: &BindingPattern<'_>) -> Option<SymbolId> {
    let BindingPattern::BindingIdentifier(id) = p else {
        return None;
    };
    let s = id.symbol_id.get()?;
    model
        .scoping
        .symbol_redeclarations(s)
        .is_empty()
        .then_some(s)
}

fn plain_param(model: &Model<'_, '_>, p: &FormalParameter<'_>) -> Option<SymbolId> {
    if p.initializer.is_some() {
        return None;
    }
    let s = plain_binding(model, &p.pattern)?;
    (model.binding_writes(s) == 0).then_some(s)
}

fn plain_var(model: &Model<'_, '_>, v: &VariableDeclarator<'_>) -> Option<SymbolId> {
    plain_binding(model, &v.id)
}

// A captured data binding is a plain var/let/const declarator outside the
// owned body. Parameters, catch bindings, destructuring and for-in/of origins
// never qualify; mutation of the data itself is allowed and proven by use.
fn captured_symbol(model: &Model<'_, '_>, site: FunctionSite, s: SymbolId) -> Option<SymbolId> {
    let declaration = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else {
        return None;
    };
    if site.span.contains_inclusive(model.scoping.symbol_span(s))
        || !model.scoping.symbol_redeclarations(s).is_empty()
        || !matches!(&variable.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
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

fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = e.without_parentheses() else {
        return None;
    };
    (!m.optional && m.property.name == key).then_some(&m.object)
}

fn call<'a>(e: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(c) = e.without_parentheses() else {
        return None;
    };
    (!c.optional).then_some(c)
}

fn args<'a>(c: &'a CallExpression<'a>, count: usize) -> Option<Vec<&'a Expression<'a>>> {
    let args = call_arguments(c);
    (args.len() == count && c.arguments.len() == count && !c.optional).then_some(args)
}

fn number(e: &Expression<'_>) -> Option<f64> {
    let Expression::NumericLiteral(n) = e.without_parentheses() else {
        return None;
    };
    Some(n.value)
}

fn binary<'a>(
    e: &'a Expression<'a>,
    operator: &str,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::BinaryExpression(b) = e.without_parentheses() else {
        return None;
    };
    (b.operator.as_str() == operator).then_some((&b.left, &b.right))
}

fn expression<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    Some(&s.expression)
}

fn local<'a>(
    model: &Model<'_, '_>,
    s: &'a Statement<'a>,
) -> Option<(SymbolId, &'a Expression<'a>)> {
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
    let id = plain_var(model, v)?;
    (model.binding_writes(id) == 0).then_some((id, v.init.as_ref()?))
}

fn record_field(model: &Model<'_, '_>, e: &Expression<'_>, record: SymbolId, field: &str) -> bool {
    member(e, field)
        .and_then(|e| member(e, "a"))
        .and_then(|e| symbol(model, e))
        == Some(record)
}

fn stringify<'a>(model: &Model<'_, '_>, e: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    let c = call(e)?;
    let json = member(&c.callee, "stringify")?;
    if expression_path(model, json) != ["JSON"] {
        return None;
    }
    Some(args(c, 1)?[0])
}

fn external_callable(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
) -> Option<FunctionSite> {
    let s = captured_symbol(model, site, symbol(model, e)?)?;
    ordered_capture(model, site, s)?;
    let target = crate::naming::resolve::ordered_callable(model, e.without_parentheses(), 0)?;
    (!site.span.contains_inclusive(target.span) && target.span != site.span).then_some(target)
}

fn arrow<'a>(
    model: &Model<'_, '_>,
    argument: &'a Argument<'a>,
) -> Option<(SymbolId, &'a ArrowFunctionExpression<'a>)> {
    let Argument::ArrowFunctionExpression(a) = argument else {
        return None;
    };
    if a.r#async || a.params.rest.is_some() || !a.body.directives.is_empty() {
        return None;
    }
    let [p] = a.params.items.as_slice() else {
        return None;
    };
    Some((plain_param(model, p)?, a))
}

fn arrow_value<'a>(a: &'a ArrowFunctionExpression<'a>) -> Option<&'a Expression<'a>> {
    match a.body.statements.as_slice() {
        [Statement::ExpressionStatement(e)] if a.expression => Some(&e.expression),
        [Statement::ReturnStatement(r)] if !a.expression => r.argument.as_ref(),
        _ => None,
    }
}

// Only a direct formal -> Uint8Array -> toString arrow can consume the encoded
// result. An unrelated nested converter cannot justify its parent callback.
fn byte_string(model: &Model<'_, '_>, argument: &Argument<'_>) -> Option<()> {
    let (input, a) = arrow(model, argument)?;
    let stringify = call(arrow_value(a)?)?;
    args(stringify, 0)?;
    let Expression::NewExpression(bytes) =
        member(&stringify.callee, "toString")?.without_parentheses()
    else {
        return None;
    };
    if expression_path(model, &bytes.callee) != ["Uint8Array"] {
        return None;
    }
    let [buffer] = bytes.arguments.as_slice() else {
        return None;
    };
    (symbol(model, buffer.as_expression()?) == Some(input)).then_some(())
}

fn object<'a>(e: &'a Expression<'a>) -> Option<Vec<(String, &'a Expression<'a>)>> {
    let Expression::ObjectExpression(o) = e.without_parentheses() else {
        return None;
    };
    let mut result: Vec<(String, &'a Expression<'a>)> = Vec::new();
    for p in &o.properties {
        let ObjectPropertyKind::ObjectProperty(p) = p else {
            return None;
        };
        if p.computed || p.method || p.shorthand || p.kind != PropertyKind::Init {
            return None;
        }
        let key = property_key_name(&p.key)?;
        if result.iter().any(|(k, _)| k == &key) {
            return None;
        }
        result.push((key, &p.value));
    }
    Some(result)
}

fn persist(
    model: &Model<'_, '_>,
    site: FunctionSite,
    argument: &Argument<'_>,
    record: SymbolId,
) -> Option<SymbolId> {
    let (input, a) = arrow(model, argument)?;
    let write = call(arrow_value(a)?)?;
    let host = member(member(&write.callee, "setItem")?, "localStorage")?;
    let host = captured_symbol(model, site, symbol(model, host)?)?;
    let values = args(write, 2)?;
    if !matches!(values[0].without_parentheses(), Expression::StringLiteral(s) if !s.value.is_empty())
    {
        return None;
    }
    let pair = object(stringify(model, values[1])?)?;
    let [(ka, va), (kb, vb)] = pair.as_slice() else {
        return None;
    };
    if ka != "a"
        || kb != "b"
        || symbol(model, va) != Some(input)
        || !record_field(model, vb, record, "c")
        || host == record
    {
        return None;
    }
    Some(host)
}

fn error_stack(model: &Model<'_, '_>, e: &Expression<'_>, error: SymbolId) -> bool {
    binary(e, "+").is_some_and(|(prefix, value)| {
        matches!(prefix.without_parentheses(), Expression::StringLiteral(s) if s.value.is_empty())
            && symbol(model, value) == Some(error)
    })
}

fn diagnostic(e: &Expression<'_>) -> bool {
    matches!(e.without_parentheses(), Expression::StringLiteral(s) if s.value == "IM6")
}

fn report_error(
    model: &Model<'_, '_>,
    site: FunctionSite,
    argument: &Argument<'_>,
) -> Option<FunctionSite> {
    let (error, a) = arrow(model, argument)?;
    if let Some(value) = arrow_value(a) {
        let report = call(value)?;
        let values = args(report, 1)?;
        let pair = object(values[0])?;
        let [(message_key, message), (stack_key, stack)] = pair.as_slice() else {
            return None;
        };
        if message_key != "message"
            || stack_key != "stack"
            || !diagnostic(message)
            || !error_stack(model, stack, error)
        {
            return None;
        }
        return external_callable(model, site, &report.callee);
    }
    // The second observed form constructs the same record in one local then
    // writes its stack. Extra writes, early exits and donated records decline.
    if a.expression {
        return None;
    }
    let [init, stack, Statement::ReturnStatement(ret)] = a.body.statements.as_slice() else {
        return None;
    };
    let (record, initial) = local(model, init)?;
    let entries = object(initial)?;
    let [(key, message)] = entries.as_slice() else {
        return None;
    };
    if key != "message" || !diagnostic(message) {
        return None;
    }
    let Expression::AssignmentExpression(stack) = expression(stack)? else {
        return None;
    };
    let AssignmentTarget::StaticMemberExpression(target) = &stack.left else {
        return None;
    };
    if stack.operator.as_str() != "="
        || target.optional
        || target.property.name != "stack"
        || symbol(model, &target.object) != Some(record)
        || !error_stack(model, &stack.right, error)
    {
        return None;
    }
    let report = call(ret.argument.as_ref()?)?;
    if symbol(model, args(report, 1)?[0]) != Some(record) {
        return None;
    }
    external_callable(model, site, &report.callee)
}

// A single direct origin, with timing proved at this function's complete
// exposure chain rather than at its hoisted declaration. No sibling body is
// used as evidence. For live mutable data, initial_origin separately selects
// the one direct initialization; subsequent values remain live captures.
fn ordered_capture<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    s: SymbolId,
) -> Option<&'a Expression<'a>> {
    let value = model.symbol_value(s)?;
    if model.initializer(s).is_some() && model.binding_writes(s) == 0
        || model.initializer(s).is_none() && model.binding_writes(s) == 1
    {
        let (container, end) = origin_position(model, s, value)?;
        exposure_after(model, site, s, container, end)?;
        Some(value)
    } else {
        None
    }
}

fn origin_position(
    model: &Model<'_, '_>,
    s: SymbolId,
    value: &Expression<'_>,
) -> Option<(NodeId, u32)> {
    let decl = model.scoping.symbol_declaration(s);
    let statement = if model
        .initializer(s)
        .is_some_and(|e| e.span() == value.span())
    {
        model.nodes.parent_id(decl)
    } else {
        let reference = model.scoping.get_resolved_references(s).find(|r| {
            matches!(model.nodes.parent_kind(r.node_id()), AstKind::AssignmentExpression(a)
                if a.operator.as_str() == "=" && a.right.span() == value.span())
        })?;
        let assignment = model.nodes.parent_id(reference.node_id());
        let statement = model.nodes.parent_id(assignment);
        if !matches!(model.nodes.kind(statement), AstKind::ExpressionStatement(_)) {
            return None;
        }
        statement
    };
    let container = model.nodes.parent_id(statement);
    let decl_container = model.nodes.parent_id(model.nodes.parent_id(decl));
    if container != decl_container
        || !matches!(
            model.nodes.kind(container),
            AstKind::FunctionBody(_) | AstKind::Program(_)
        )
    {
        return None;
    }
    Some((container, model.nodes.kind(statement).span().end))
}

fn exposure_after(
    model: &Model<'_, '_>,
    site: FunctionSite,
    s: SymbolId,
    container: NodeId,
    end: u32,
) -> Option<()> {
    model
        .scoping
        .get_resolved_references(s)
        .filter(|r| {
            site.span
                .contains_inclusive(model.nodes.kind(r.node_id()).span())
        })
        .all(|r| {
            crate::naming::shared::callable_origin_precedes(
                model,
                container,
                end,
                r.node_id(),
                None,
                0,
                true,
            )
        })
        .then_some(())
}

fn initial_origin<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
    s: SymbolId,
) -> Option<&'a Expression<'a>> {
    captured_symbol(model, site, s)?;
    let mut origins = Vec::new();
    if let Some(value) = model.initializer(s) {
        origins.push(value);
    }
    // Only direct writes in the declaration's own statement list count as
    // initialization. Nested writers are not searched for donated proof.
    for r in model
        .scoping
        .get_resolved_references(s)
        .filter(|r| r.is_write())
    {
        let AstKind::AssignmentExpression(a) = model.nodes.parent_kind(r.node_id()) else {
            continue;
        };
        if a.operator.as_str() == "="
            && matches!(&a.left, AssignmentTarget::AssignmentTargetIdentifier(id) if crate::analysis::identifier_symbol(id, model.scoping)==Some(s))
            && origin_position(model, s, &a.right).is_some()
        {
            origins.push(&a.right);
        }
    }
    let [value] = origins.as_slice() else {
        return None;
    };
    let (container, end) = origin_position(model, s, value)?;
    exposure_after(model, site, s, container, end)?;
    Some(value)
}

fn text(e: &Expression<'_>, value: &str) -> bool {
    matches!(e.without_parentheses(), Expression::StringLiteral(s) if s.value == value)
}

// Exact unshadowed ambient-host fallback, including all guards and polarity.
// Arbitrary objects with Date/document/localStorage fields are not host proof.
fn ambient(model: &Model<'_, '_>, e: &Expression<'_>, slot: usize) -> bool {
    if slot == 3 {
        return matches!(e.without_parentheses(), Expression::UnaryExpression(u)
            if u.operator.as_str()=="void" && number(&u.argument)==Some(0.0));
    }
    let Expression::ConditionalExpression(c) = e.without_parentheses() else {
        return false;
    };
    let name = ["global", "self", "window"][slot];
    let Some((first, identity)) = logical(&c.test, "&&") else {
        return false;
    };
    let Some((defined, nonnull)) = logical(first, "&&") else {
        return false;
    };
    let Some((typeof_, undefined)) = binary(defined, "!==") else {
        return false;
    };
    let Expression::UnaryExpression(typeof_) = typeof_.without_parentheses() else {
        return false;
    };
    let Some((base, null)) = binary(nonnull, "!=") else {
        return false;
    };
    let Some((self_, same)) = binary(identity, "===") else {
        return false;
    };
    typeof_.operator.as_str() == "typeof"
        && expression_path(model, &typeof_.argument) == [name]
        && text(undefined, "undefined")
        && expression_path(model, base) == [name]
        && matches!(null.without_parentheses(), Expression::NullLiteral(_))
        && member(self_, name).is_some_and(|e| expression_path(model, e) == [name])
        && expression_path(model, same) == [name]
        && expression_path(model, &c.consequent) == [name]
        && ambient(model, &c.alternate, slot + 1)
}

fn logical<'a>(
    e: &'a Expression<'a>,
    op: &str,
) -> Option<(&'a Expression<'a>, &'a Expression<'a>)> {
    let Expression::LogicalExpression(b) = e.without_parentheses() else {
        return None;
    };
    (b.operator.as_str() == op).then_some((&b.left, &b.right))
}

fn host_origin(model: &Model<'_, '_>, site: FunctionSite, host: SymbolId) -> Option<()> {
    let origin = ordered_capture(model, site, host)?;
    (expression_path(model, origin) == ["globalThis"] || ambient(model, origin, 0)).then_some(())
}

fn reading(model: &Model<'_, '_>, e: &Expression<'_>, date: SymbolId) -> Option<()> {
    let convert = call(e)?;
    if expression_path(model, &convert.callee) != ["Number"] {
        return None;
    }
    let values = args(convert, 1)?;
    let Expression::NewExpression(new) = values[0].without_parentheses() else {
        return None;
    };
    (new.arguments.is_empty() && symbol(model, &new.callee) == Some(date)).then_some(())
}

fn assign<'a>(s: &'a Statement<'a>) -> Option<&'a AssignmentExpression<'a>> {
    let Expression::AssignmentExpression(a) = expression(s)?.without_parentheses() else {
        return None;
    };
    (a.operator.as_str() == "=").then_some(a)
}

fn target_field(
    model: &Model<'_, '_>,
    a: &AssignmentExpression<'_>,
    field: &str,
) -> Option<SymbolId> {
    let AssignmentTarget::StaticMemberExpression(m) = &a.left else {
        return None;
    };
    if m.optional || m.property.name != field {
        return None;
    }
    symbol(model, &m.object)
}

fn clock_write(
    model: &Model<'_, '_>,
    s: &Statement<'_>,
    previous: SymbolId,
    date: SymbolId,
) -> Option<()> {
    let a = assign(s)?;
    if !matches!(&a.left, AssignmentTarget::AssignmentTargetIdentifier(id) if crate::analysis::identifier_symbol(id,model.scoping)==Some(previous))
    {
        return None;
    }
    reading(model, &a.right, date)
}

fn visibility(model: &Model<'_, '_>, e: &Expression<'_>, value: &str) -> Option<SymbolId> {
    let (state, literal) = binary(e, "===")?;
    if !text(literal, value) {
        return None;
    }
    symbol(
        model,
        member(member(state, "visibilityState")?, "document")?,
    )
}

fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(&b.body)
}

fn duration(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<(SymbolId, SymbolId)> {
    let convert = call(e)?;
    if expression_path(model, &convert.callee) != ["Number"] {
        return None;
    }
    let values = args(convert, 1)?;
    let (rest, hour) = binary(values[0], "/")?;
    let (rest, minute) = binary(rest, "/")?;
    let (rest, millis) = binary(rest, "/")?;
    if number(hour) != Some(60.0) || number(minute) != Some(60.0) || number(millis) != Some(1000.0)
    {
        return None;
    }
    let (current, previous) = binary(rest, "-")?;
    let current = call(current)?;
    let values = args(current, 1)?;
    let Expression::NewExpression(new) = values[0].without_parentheses() else {
        return None;
    };
    let date = symbol(model, &new.callee)?;
    // Recheck the whole reading (not merely the constructor/member inventory).
    let (current, _) = binary(rest, "-")?;
    reading(model, current, date)?;
    Some((date, symbol(model, previous)?))
}

fn zero_field(e: &Expression<'_>, key: &str) -> Option<()> {
    let fields = object(e)?;
    let (_, value) = fields.iter().find(|(k, _)| k == key)?;
    (number(value) == Some(0.0)).then_some(())
}

fn protocol(model: &Model<'_, '_>, site: FunctionSite, body: &FunctionBody<'_>) -> Option<()> {
    // First package supports the direct transform form only. Staged transforms
    // need their own write-count/temporal proof and deliberately decline.
    let [
        Statement::IfStatement(state),
        add,
        encoded,
        Statement::IfStatement(guard),
    ] = body.statements.as_slice()
    else {
        return None;
    };
    let host = visibility(model, &state.test, "hidden")?;
    captured_symbol(model, site, host)?;
    host_origin(model, site, host)?;
    let [elapsed, reset] = block(&state.consequent)? else {
        return None;
    };
    let elapsed = assign(elapsed)?;
    let counter = target_field(model, elapsed, "lvTop")?;
    let (date, previous) = duration(model, &elapsed.right)?;
    captured_symbol(model, site, date)?;
    let date_origin = ordered_capture(model, site, date)?;
    if symbol(model, member(date_origin, "Date")?) != Some(host) {
        return None;
    }
    let previous_origin = initial_origin(model, site, previous)?;
    let (date_container, date_end) = origin_position(model, date, date_origin)?;
    let (previous_container, _) = origin_position(model, previous, previous_origin)?;
    let host_value = ordered_capture(model, site, host)?;
    let (host_container, host_end) = origin_position(model, host, host_value)?;
    if date_container != previous_container
        || host_container != date_container
        || date_end > previous_origin.span().start
        || host_end > date_origin.span().start
    {
        return None;
    }
    reading(model, previous_origin, date)?;
    zero_field(initial_origin(model, site, counter)?, "lvTop")?;
    if [host, date, previous, counter]
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != 4
    {
        return None;
    }
    clock_write(model, reset, previous, date)?;
    let Statement::IfStatement(visible) = state.alternate.as_ref()? else {
        return None;
    };
    if visibility(model, &visible.test, "visible") != Some(host) || visible.alternate.is_some() {
        return None;
    }
    let [reset] = block(&visible.consequent)? else {
        return None;
    };
    clock_write(model, reset, previous, date)?;
    let add = assign(add)?;
    let AssignmentTarget::StaticMemberExpression(ts) = &add.left else {
        return None;
    };
    if ts.optional || ts.property.name != "ts" {
        return None;
    }
    let record = symbol(model, member(&ts.object, "a")?)?;
    let (old, increment) = binary(&add.right, "+")?;
    if !record_field(model, old, record, "ts")
        || symbol(model, member(increment, "lvTop")?) != Some(counter)
        || [host, date, previous, counter].contains(&record)
    {
        return None;
    }
    let record_origin = initial_origin(model, site, record)?;
    let fields = object(record_origin)?;
    let (_, a) = fields.iter().find(|(k, _)| k == "a")?;
    zero_field(a, "ts")?;
    zero_field(a, "c")?;
    let (encoded, value) = local(model, encoded)?;
    let encode = call(value)?;
    let encoder = external_callable(model, site, &encode.callee)?;
    if symbol(model, member(stringify(model, args(encode, 1)?[0])?, "a")?) != Some(record) {
        return None;
    }
    let (pending, null) = binary(&guard.test, "!=")?;
    if symbol(model, pending) != Some(encoded)
        || !matches!(null.without_parentheses(), Expression::NullLiteral(_))
        || guard.alternate.is_some()
    {
        return None;
    }
    let [chain] = block(&guard.consequent)? else {
        return None;
    };
    let caught = call(expression(chain)?)?;
    let second = call(member(&caught.callee, "catch")?)?;
    let first = call(member(&second.callee, "then")?)?;
    if symbol(model, member(&first.callee, "then")?) != Some(encoded) {
        return None;
    }
    let [convert] = first.arguments.as_slice() else {
        return None;
    };
    byte_string(model, convert)?;
    let [writer] = second.arguments.as_slice() else {
        return None;
    };
    if persist(model, site, writer, record)? != host {
        return None;
    }
    let [report] = caught.arguments.as_slice() else {
        return None;
    };
    let reporter = report_error(model, site, report)?;
    (reporter.span != encoder.span).then_some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    if !f.members.contains("lvTop") {
        return vec![];
    }
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return vec![];
    };
    let Some(body) = function.body.as_ref() else {
        return vec![];
    };
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !function.params.items.is_empty()
        || !body.directives.is_empty()
        || protocol(model, site, body).is_none()
    {
        return vec![];
    }
    vec![proposal(
        "persistVisibilityAdjustedSnapshot",
        "owned_visibility_persistence_variants",
        "exact owned hidden/visible branches read one proven ambient host; Number(new same-host Date()) reads and same captured previous-clock subtraction /1000/60/60 update lvTop before reset, then same-record a.ts accumulation; directly owned null-guarded then/then/catch converts Uint8Array to decimal string, returns same-host storage write of {a:result,b:live same-record a.c}, and returns IM6 reporting of actual caught value; plain initial state and complete exposure-order proofs; transform/reporter callable identities distinct, transform's external algorithm/purpose explicitly unproved",
        f,
    )]
}
