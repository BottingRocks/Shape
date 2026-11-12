//! Bounded observer object construction and JavaScript method-call lowering.
use super::*;
use oxc_semantic::SymbolId;
use oxc_span::GetSpan;

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e, model.scoping)
}

fn plain_binding(model: &Model<'_, '_>, p: &BindingPattern<'_>) -> Option<SymbolId> {
    let BindingPattern::BindingIdentifier(id) = p else {
        return None;
    };
    let s = id.symbol_id.get()?;
    (model.scoping.symbol_redeclarations(s).is_empty()).then_some(s)
}

fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    match e {
        Expression::StaticMemberExpression(m) if !m.optional && m.property.name == key => {
            Some(&m.object)
        }
        Expression::ComputedMemberExpression(m)
            if !m.optional
                && matches!(&m.expression, Expression::StringLiteral(s) if s.value == key) =>
        {
            Some(&m.object)
        }
        _ => None,
    }
}

fn slot<'a>(e: &'a Expression<'a>, index: f64) -> Option<&'a Expression<'a>> {
    let Expression::ComputedMemberExpression(m) = e else {
        return None;
    };
    (!m.optional && matches!(&m.expression, Expression::NumericLiteral(n) if n.value == index))
        .then_some(&m.object)
}

// Only values already encountered in this straight-line body can be aliases.
// Thus initializer/assignment ordering is proven without a global origin walk.
fn resolve<'a>(
    model: &Model<'_, '_>,
    e: &'a Expression<'a>,
    locals: &BTreeMap<SymbolId, &'a Expression<'a>>,
) -> &'a Expression<'a> {
    symbol(model, e)
        .and_then(|s| locals.get(&s).copied())
        .unwrap_or(e)
}

fn available(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
    locals: &BTreeMap<SymbolId, &Expression<'_>>,
) -> bool {
    let Some(s) = symbol(model, e) else {
        return true;
    };
    !site.span.contains_inclusive(model.scoping.symbol_span(s))
        || !matches!(
            model.nodes.kind(model.scoping.symbol_declaration(s)),
            AstKind::VariableDeclarator(_)
        )
        || locals.contains_key(&s)
}

fn void_zero(e: &Expression<'_>) -> bool {
    matches!(e, Expression::UnaryExpression(u) if u.operator.as_str() == "void"
        && matches!(&u.argument, Expression::NumericLiteral(n) if n.value == 0.0))
}

fn object_values_available(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
    locals: &BTreeMap<SymbolId, &Expression<'_>>,
) -> bool {
    let Expression::ObjectExpression(o) = e else {
        return true;
    };
    o.properties.iter().all(|p| matches!(p, ObjectPropertyKind::ObjectProperty(p) if available(model, site, &p.value, locals)))
}

// Independently prove the bounded owned callback protocol when the older
// collector finder cannot follow the factory's object aliases.
fn callback_protocol(
    model: &Model<'_, '_>,
    factory: FunctionSite,
    children: &[FunctionSite],
) -> Option<()> {
    fn body<'a>(
        model: &Model<'_, 'a>,
        site: FunctionSite,
        count: usize,
    ) -> Option<(&'a FunctionBody<'a>, Vec<SymbolId>)> {
        let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
            return None;
        };
        let b = f.body.as_ref()?;
        if f.r#async
            || f.generator
            || !b.directives.is_empty()
            || f.params.rest.is_some()
            || f.params.items.len() != count
        {
            return None;
        }
        let mut params = vec![];
        for p in &f.params.items {
            if p.initializer.is_some() {
                return None;
            }
            let s = plain_binding(model, &p.pattern)?;
            if model.binding_writes(s) != 0 {
                return None;
            }
            params.push(s);
        }
        Some((b, params))
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
    fn call<'a>(s: &'a Statement<'a>) -> Option<&'a CallExpression<'a>> {
        let Statement::ExpressionStatement(s) = s else {
            return None;
        };
        let Expression::CallExpression(c) = &s.expression else {
            return None;
        };
        (!c.optional && call_arguments(c).len() == c.arguments.len()).then_some(c)
    }
    fn indexed(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<(SymbolId, SymbolId)> {
        let Expression::ComputedMemberExpression(m) = e else {
            return None;
        };
        (!m.optional).then_some((symbol(model, &m.object)?, symbol(model, &m.expression)?))
    }
    fn mark(model: &Model<'_, '_>, s: &Statement<'_>, key: &str) -> Option<(SymbolId, SymbolId)> {
        let a = assignment(s)?;
        let AssignmentTarget::StaticMemberExpression(m) = &a.left else {
            return None;
        };
        if m.optional
            || m.property.name != key
            || !matches!(&a.right, Expression::BooleanLiteral(b) if b.value)
        {
            return None;
        }
        indexed(model, &m.object)
    }
    fn notify(
        model: &Model<'_, '_>,
        s: &Statement<'_>,
        kind: &str,
        statuses: SymbolId,
        index: SymbolId,
        value: SymbolId,
    ) -> Option<SymbolId> {
        let c = call(s)?;
        let [Argument::ObjectExpression(o)] = c.arguments.as_slice() else {
            return None;
        };
        if o.properties.len() != 4 {
            return None;
        }
        let mut seen = BTreeSet::new();
        for property in &o.properties {
            let ObjectPropertyKind::ObjectProperty(p) = property else {
                return None;
            };
            let key = property_key_name(&p.key)?;
            if p.computed || p.method || p.kind != PropertyKind::Init || !seen.insert(key.clone()) {
                return None;
            }
            let valid = match key.as_str() {
                "type" => matches!(&p.value, Expression::StringLiteral(s) if s.value == kind),
                "status" => indexed(model, &p.value) == Some((statuses, index)),
                "value" => symbol(model, &p.value) == Some(value),
                "signalIdx" => symbol(model, &p.value) == Some(index),
                _ => false,
            };
            if !valid {
                return None;
            }
        }
        symbol(model, &c.callee)
    }
    let [next, error, complete] = children else {
        return None;
    };
    let (next_body, params) = body(model, *next, 1)?;
    let [store, collected, broadcast] = next_body.statements.as_slice() else {
        return None;
    };
    let store = assignment(store)?;
    let AssignmentTarget::ComputedMemberExpression(destination) = &store.left else {
        return None;
    };
    if destination.optional || symbol(model, &store.right) != Some(params[0]) {
        return None;
    }
    let values = symbol(model, &destination.object)?;
    let index = symbol(model, &destination.expression)?;
    let (statuses, status_index) = mark(model, collected, "hasCollected")?;
    if status_index != index || statuses == values {
        return None;
    }
    let broadcaster = notify(model, broadcast, "collect", statuses, index, params[0])?;
    let (error_body, params) = body(model, *error, 1)?;
    let [log, errored, broadcast] = error_body.statements.as_slice() else {
        return None;
    };
    let log = call(log)?;
    let args = call_arguments(log);
    if args.len() != 3
        || symbol(model, args[0]) != Some(index)
        || !matches!(args[1], Expression::BooleanLiteral(b) if !b.value)
        || symbol(model, args[2]) != Some(params[0])
        || mark(model, errored, "hasErrored") != Some((statuses, index))
        || notify(model, broadcast, "error", statuses, index, params[0]) != Some(broadcaster)
    {
        return None;
    }
    let logger = symbol(model, &log.callee)?;
    if logger == broadcaster {
        return None;
    }
    let (complete_body, _) = body(model, *complete, 0)?;
    let [Statement::VariableDeclaration(d), Statement::IfStatement(branch)] =
        complete_body.statements.as_slice()
    else {
        return None;
    };
    let [v] = d.declarations.as_slice() else {
        return None;
    };
    let value = plain_binding(model, &v.id)?;
    if model.binding_writes(value) != 0
        || indexed(model, v.init.as_ref()?) != Some((values, index))
        || branch.alternate.is_some()
    {
        return None;
    }
    let Expression::LogicalExpression(test) = &branch.test else {
        return None;
    };
    let Expression::CallExpression(is_array) = &test.left else {
        return None;
    };
    let Expression::BinaryExpression(length) = &test.right else {
        return None;
    };
    if test.operator.as_str() != "&&"
        || is_array.optional
        || expression_path(model, &is_array.callee) != ["Array", "isArray"]
        || is_array.arguments.len() != 1
        || call_arguments(is_array)
            .first()
            .and_then(|a| symbol(model, a))
            != Some(value)
        || length.operator.as_str() != ">="
        || symbol(model, member(&length.left, "length")?) != Some(value)
        || !matches!(&length.right,Expression::NumericLiteral(n) if n.value==2.0)
    {
        return None;
    }
    let Statement::BlockStatement(block) = &branch.consequent else {
        return None;
    };
    let [push_index, Statement::VariableDeclaration(d), push_data] = block.body.as_slice() else {
        return None;
    };
    let push_index = call(push_index)?;
    let report = symbol(
        model,
        member(
            member(member(&push_index.callee, "push")?, "sidx")?,
            "scrBytes",
        )?,
    )?;
    if push_index.arguments.len() != 1
        || call_arguments(push_index)
            .first()
            .and_then(|a| symbol(model, a))
            != Some(index)
    {
        return None;
    }
    let [data] = d.declarations.as_slice() else {
        return None;
    };
    let data_id = plain_binding(model, &data.id)?;
    if model.binding_writes(data_id) != 0
        || symbol(
            model,
            member(member(data.init.as_ref()?, "data")?, "scrBytes")?,
        ) != Some(report)
    {
        return None;
    }
    let push_data = call(push_data)?;
    if symbol(model, member(&push_data.callee, "push")?) != Some(data_id) {
        return None;
    }
    let [Argument::ArrayExpression(parts)] = push_data.arguments.as_slice() else {
        return None;
    };
    if parts.elements.len() != 2 {
        return None;
    }
    for (i, part) in parts.elements.iter().enumerate() {
        let Expression::CallExpression(slice) = part.as_expression()? else {
            return None;
        };
        if slice.optional
            || symbol(model, slot(member(&slice.callee, "slice")?, i as f64)?) != Some(value)
            || !matches!(slice.arguments.as_slice(), [Argument::NumericLiteral(a), Argument::NumericLiteral(b)] if a.value==0.0 && b.value==8.0)
        {
            return None;
        }
    }
    // Captures must be immutable identities, owned by the factory or its
    // enclosing scopes, never a child-local lookalike or unresolved global.
    for s in [values, statuses, index, broadcaster, logger, report] {
        if children
            .iter()
            .any(|c| c.span.contains_inclusive(model.scoping.symbol_span(s)))
            || model.binding_writes(s) != 0
            || !model.scoping.symbol_redeclarations(s).is_empty()
        {
            return None;
        }
    }
    // The index is the factory's unchanged formal, possibly via one local alias.
    let index_value = model.initializer(index);
    if model
        .formal_symbol_slot(index)
        .is_none_or(|slot| slot.owner != factory.node_id || slot.index != 0)
        && index_value.and_then(|e| model.site_parameter_slot(e, factory)) != Some(0)
    {
        return None;
    }
    Some(())
}

fn factory<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &'a FunctionBody<'a>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Option<()> {
    let (last, prefix) = body.statements.split_last()?;
    let Statement::ReturnStatement(ret) = last else {
        return None;
    };
    let mut locals = BTreeMap::new();
    let mut pending = BTreeSet::new();
    let mut object_span = None;
    let mut methods = BTreeMap::new();
    let mut written = BTreeSet::new();
    // The returned object may only flow through construction aliases, writes,
    // and the return. No call, nested capture, unrelated read or escape is safe.
    let mut object_symbols = BTreeSet::new();
    let mut allowed_references = BTreeSet::new();
    let mut take_value = |value: &'a Expression<'a>| -> Option<()> {
        if let Expression::ObjectExpression(object) = value {
            if object_span.replace(object.span).is_some() {
                return None;
            }
            for item in &object.properties {
                let ObjectPropertyKind::ObjectProperty(p) = item else {
                    return None;
                };
                if p.computed || p.method || p.kind != PropertyKind::Init {
                    return None;
                }
                let key = property_key_name(&p.key)?;
                if !["next", "error", "complete"].contains(&key.as_str())
                    || methods.insert(key, &p.value).is_some()
                {
                    return None;
                }
            }
        }
        Some(())
    };
    // Collect the one object first; its properties still resolve using the
    // sequential environment at each assignment and at the final return.
    for statement in prefix {
        if let Statement::VariableDeclaration(d) = statement {
            for v in &d.declarations {
                if let Some(init) = &v.init {
                    take_value(init)?;
                }
            }
        } else if let Statement::ExpressionStatement(s) = statement {
            if let Expression::AssignmentExpression(a) = &s.expression {
                take_value(&a.right)?;
            }
        }
    }
    take_value(ret.argument.as_ref()?)?;
    drop(take_value);
    let object_span = object_span?;
    for statement in prefix {
        match statement {
            Statement::FunctionDeclaration(_) => (),
            Statement::VariableDeclaration(d) => {
                for v in &d.declarations {
                    let s = plain_binding(model, &v.id)?;
                    if let Some(init) = &v.init {
                        if model.binding_writes(s) != 0 {
                            return None;
                        }
                        if !available(model, site, init, &locals)
                            || !object_values_available(model, site, init, &locals)
                        {
                            return None;
                        }
                        let value = resolve(model, init, &locals);
                        if let Some(origin) = symbol(model, init) {
                            if object_symbols.contains(&origin) {
                                allowed_references.insert(init.span());
                            }
                        }
                        if value.span() == object_span {
                            object_symbols.insert(s);
                        }
                        // Initializers must be plain aliases, the owned object,
                        // or stable function values. Calls and arbitrary effects refuse.
                        if !matches!(
                            value,
                            Expression::Identifier(_)
                                | Expression::FunctionExpression(_)
                                | Expression::ObjectExpression(_)
                        ) {
                            return None;
                        }
                        locals.insert(s, value);
                    } else {
                        if model.binding_writes(s) != 1 {
                            return None;
                        }
                        pending.insert(s);
                    }
                }
            }
            Statement::ExpressionStatement(s) => {
                let Expression::AssignmentExpression(a) = &s.expression else {
                    return None;
                };
                if a.operator.as_str() != "=" {
                    return None;
                }
                if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
                    let target = crate::analysis::identifier_symbol(id, model.scoping)?;
                    if !pending.remove(&target) {
                        return None;
                    }
                    if !available(model, site, &a.right, &locals)
                        || !object_values_available(model, site, &a.right, &locals)
                    {
                        return None;
                    }
                    let value = resolve(model, &a.right, &locals);
                    if let Some(origin) = symbol(model, &a.right) {
                        if object_symbols.contains(&origin) {
                            allowed_references.insert(a.right.span());
                        }
                    }
                    if value.span() == object_span {
                        object_symbols.insert(target);
                        allowed_references.insert(id.span);
                    }
                    if !matches!(
                        value,
                        Expression::Identifier(_)
                            | Expression::FunctionExpression(_)
                            | Expression::ObjectExpression(_)
                    ) {
                        return None;
                    }
                    locals.insert(target, value);
                    continue;
                }
                let (receiver, key) = match &a.left {
                    AssignmentTarget::StaticMemberExpression(m) if !m.optional => {
                        (&m.object, m.property.name.to_string())
                    }
                    AssignmentTarget::ComputedMemberExpression(m) if !m.optional => {
                        let Expression::StringLiteral(key) = &m.expression else {
                            return None;
                        };
                        (&m.object, key.value.to_string())
                    }
                    _ => return None,
                };
                if !["next", "error", "complete"].contains(&key.as_str())
                    || resolve(model, receiver, &locals).span() != object_span
                    || !written.insert(key.clone())
                    || methods.get(&key).is_some_and(|old| !void_zero(old))
                {
                    return None;
                }
                allowed_references.insert(receiver.span());
                if !available(model, site, &a.right, &locals) {
                    return None;
                }
                methods.insert(key, resolve(model, &a.right, &locals));
            }
            _ => return None,
        }
    }
    let returned = ret.argument.as_ref()?;
    if !object_values_available(model, site, returned, &locals)
        || !pending.is_empty()
        || resolve(model, returned, &locals).span() != object_span
        || methods.len() != 3
    {
        return None;
    }
    allowed_references.insert(returned.span());
    for s in object_symbols {
        if model.binding_references(s) == 0 {
            return None;
        }
        if !model
            .scoping
            .get_resolved_references(s)
            .all(|r| allowed_references.contains(&model.nodes.get_node(r.node_id()).span()))
        {
            return None;
        }
    }
    let mut children = vec![];
    for key in ["next", "error", "complete"] {
        let value = resolve(model, methods.get(key)?, &locals);
        let child = stable_function(model, value, 0)?;
        if !model
            .enclosing_function(child.node_id)
            .is_some_and(|parent| parent.span == site.span)
        {
            return None;
        }
        children.push(child);
    }
    callback_protocol(model, site, &children)
}

fn forward_error(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [p] = function.params.items.as_slice() else {
        return None;
    };
    let input = plain_binding(model, &p.pattern)?;
    let (last, prefix) = body.statements.split_last()?;
    let Statement::ExpressionStatement(last) = last else {
        return None;
    };
    let Expression::CallExpression(call) = &last.expression else {
        return None;
    };
    if call.optional {
        return None;
    }
    let mut locals = BTreeMap::new();
    // This helper deliberately supports initialized immutable aliases only.
    for statement in prefix {
        let Statement::VariableDeclaration(d) = statement else {
            return None;
        };
        for v in &d.declarations {
            let s = plain_binding(model, &v.id)?;
            if model.binding_writes(s) != 0 || model.binding_references(s) == 0 {
                return None;
            }
            let init = v.init.as_ref()?;
            if !available(model, site, init, &locals) {
                return None;
            }
            let value = resolve(model, init, &locals);
            match value {
                Expression::Identifier(_) => (),
                Expression::StaticMemberExpression(_) | Expression::ComputedMemberExpression(_)
                    if member(value, "error")
                        .is_some_and(|e| available(model, site, e, &locals)) =>
                {
                    ()
                }
                Expression::ArrayExpression(a) if a.elements.len() == 2 => {
                    let first = a.elements[0].as_expression()?;
                    let second = a.elements[1].as_expression()?;
                    if !available(model, site, first, &locals)
                        || !available(model, site, member(second, "error")?, &locals)
                    {
                        return None;
                    }
                }
                _ => return None,
            }
            locals.insert(s, value);
        }
    }
    let args = call_arguments(call);
    if args.len() != call.arguments.len() {
        return None;
    }
    let callee = resolve(model, &call.callee, &locals);
    let (receiver, argument) = if let Some(method) = member(callee, "call") {
        let [receiver, argument] = args.as_slice() else {
            return None;
        };
        let method = resolve(model, method, &locals);
        if let (Some(tuple_method), Some(tuple_receiver)) = (slot(method, 1.0), slot(receiver, 0.0))
        {
            let tuple_method = resolve(model, tuple_method, &locals);
            let tuple_receiver = resolve(model, tuple_receiver, &locals);
            if tuple_method.span() != tuple_receiver.span() {
                return None;
            }
            let Expression::ArrayExpression(tuple) = tuple_method else {
                return None;
            };
            let [first, second] = tuple.elements.as_slice() else {
                return None;
            };
            let first = first.as_expression()?;
            let second = second.as_expression()?;
            let owner = resolve(model, member(second, "error")?, &locals);
            if symbol(model, resolve(model, first, &locals)) != symbol(model, owner) {
                return None;
            }
            (owner, *argument)
        } else {
            let owner = resolve(model, member(method, "error")?, &locals);
            if symbol(model, resolve(model, receiver, &locals)) != symbol(model, owner) {
                return None;
            }
            (owner, *argument)
        }
    } else {
        let [argument] = args.as_slice() else {
            return None;
        };
        (
            resolve(model, member(&call.callee, "error")?, &locals),
            *argument,
        )
    };
    let captured = symbol(model, receiver)?;
    if site
        .span
        .contains_inclusive(model.scoping.symbol_span(captured))
        || model.binding_writes(captured) != 0
        || !model.scoping.symbol_redeclarations(captured).is_empty()
        || model.binding_writes(input) != 0
        || symbol(model, resolve(model, argument, &locals)) != Some(input)
    {
        return None;
    }
    // Every initializer is part of this exact forwarding operation. In
    // particular an unused tuple cannot hide a call/getter or a different sink.
    for value in locals.values() {
        let receiver_matches =
            |e: &Expression<'_>| symbol(model, resolve(model, e, &locals)) == Some(captured);
        match value {
            Expression::Identifier(_)
                if [Some(captured), Some(input)].contains(&symbol(model, value)) =>
            {
                ()
            }
            Expression::StaticMemberExpression(_) | Expression::ComputedMemberExpression(_)
                if member(value, "error").is_some_and(receiver_matches) =>
            {
                ()
            }
            Expression::ArrayExpression(a) => {
                let [first, second] = a.elements.as_slice() else {
                    return None;
                };
                if !receiver_matches(first.as_expression()?)
                    || !member(second.as_expression()?, "error").is_some_and(receiver_matches)
                {
                    return None;
                }
            }
            _ => return None,
        }
    }
    Some(())
}

// Resolve only plain, single-origin callables. Each alias must already exist at
// its use; a recovered assignment must share the declaration's statement list.
// This deliberately refuses origins supplied by setup callbacks or loop headers.
pub(in crate::naming) fn ordered_callable(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    depth: usize,
) -> Option<FunctionSite> {
    if depth > 8 {
        return None;
    }
    let Expression::Identifier(_) = e else {
        return model.function_from_expression(e);
    };
    let s = symbol(model, e)?;
    if !model.scoping.symbol_redeclarations(s).is_empty() {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(s);
    if let AstKind::Function(f) = model.nodes.kind(declaration) {
        return (model.binding_writes(s) == 0)
            .then(|| model.site_by_span(f.span))
            .flatten();
    }
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(d) if matches!(d.kind, VariableDeclarationKind::Var | VariableDeclarationKind::Let | VariableDeclarationKind::Const)
        )
    {
        return None;
    }
    let container = model.nodes.parent_id(model.nodes.parent_id(declaration));
    if !matches!(
        model.nodes.kind(container),
        AstKind::FunctionBody(_) | AstKind::Program(_)
    ) {
        return None;
    }
    let value = model.symbol_value(s)?;
    if value.span().end > e.span().start {
        return None;
    }
    match (v.init.is_some(), model.binding_writes(s)) {
        (true, 0) => (),
        (false, 1) => {
            let valid = model.scoping.get_resolved_references(s).any(|r| {
                let parent = model.nodes.parent_id(r.node_id());
                matches!(model.nodes.kind(parent), AstKind::AssignmentExpression(a)
                    if a.operator.as_str() == "=" && a.right.span() == value.span())
                    && matches!(
                        model.nodes.parent_kind(parent),
                        AstKind::ExpressionStatement(_)
                    )
                    && model.nodes.parent_id(model.nodes.parent_id(parent)) == container
            });
            if !valid {
                return None;
            }
        }
        _ => return None,
    }
    ordered_callable(model, value, depth + 1)
}

fn guarded_forward(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<bool> {
    let [p] = function.params.items.as_slice() else {
        return None;
    };
    let input = plain_binding(model, &p.pattern)?;
    let [Statement::TryStatement(t)] = body.statements.as_slice() else {
        return None;
    };
    if t.finalizer.is_some() {
        return None;
    }
    let [Statement::ExpressionStatement(invoke)] = t.block.body.as_slice() else {
        return None;
    };
    let Expression::CallExpression(invoke) = &invoke.expression else {
        return None;
    };
    if invoke.optional {
        return None;
    }
    let tagged = match invoke.arguments.as_slice() {
        [Argument::NumericLiteral(tag), value] if tag.value.is_finite() => {
            if symbol(model, value.as_expression()?) != Some(input) {
                return None;
            }
            true
        }
        [value] if symbol(model, value.as_expression()?) == Some(input) => false,
        _ => return None,
    };
    let target = ordered_callable(model, &invoke.callee, 0)?;
    if target.span == site.span {
        return None;
    }
    let handler = t.handler.as_ref()?;
    let caught = plain_binding(model, &handler.param.as_ref()?.pattern)?;
    if model.binding_writes(caught) != 0 {
        return None;
    }
    let [Statement::ExpressionStatement(failure)] = handler.body.body.as_slice() else {
        return None;
    };
    let Expression::CallExpression(failure) = &failure.expression else {
        return None;
    };
    let [value] = failure.arguments.as_slice() else {
        return None;
    };
    if failure.optional || symbol(model, value.as_expression()?) != Some(caught) {
        return None;
    }
    let receiver = symbol(model, member(&failure.callee, "error")?)?;
    if site
        .span
        .contains_inclusive(model.scoping.symbol_span(receiver))
        || model.binding_writes(receiver) > 1
        || !model.scoping.symbol_redeclarations(receiver).is_empty()
    {
        return None;
    }
    // Variable captures must be plain and initialized before this callback.
    // A recovered assignment must be in the declaration's own statement list.
    let declaration = model.scoping.symbol_declaration(receiver);
    if let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) {
        let container = model.nodes.parent_id(model.nodes.parent_id(declaration));
        if !matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(receiver))
            || !matches!(
                model.nodes.kind(container),
                AstKind::FunctionBody(_) | AstKind::Program(_)
            )
        {
            return None;
        }
        let value = model.symbol_value(receiver)?;
        if value.span().end > site.span.start {
            return None;
        }
        match (v.init.is_some(), model.binding_writes(receiver)) {
            (true, 0) => (),
            (false, 1) => {
                if !model.scoping.get_resolved_references(receiver).any(|r| {
                    let parent = model.nodes.parent_id(r.node_id());
                    matches!(model.nodes.kind(parent), AstKind::AssignmentExpression(a)
                        if a.operator.as_str() == "=" && a.right.span() == value.span())
                        && matches!(
                            model.nodes.parent_kind(parent),
                            AstKind::ExpressionStatement(_)
                        )
                        && model.nodes.parent_id(model.nodes.parent_id(parent)) == container
                }) {
                    return None;
                }
            }
            _ => return None,
        }
    } else if model.binding_writes(receiver) != 0 {
        return None;
    }
    Some(tagged)
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return vec![];
    };
    let Some(body) = &function.body else {
        return vec![];
    };
    if body.statements.len() > 32
        || function.r#async
        || function.generator
        || !body.directives.is_empty()
        || function.params.rest.is_some()
        || function.params.items.iter().any(|p| {
            p.initializer.is_some()
                || plain_binding(model, &p.pattern).is_none_or(|s| model.binding_writes(s) != 0)
        })
    {
        return vec![];
    }
    let mut claims = vec![];
    if function.params.items.len() == 1 && factory(model, site, body, proposals).is_some() {
        claims.push(proposal("createSignalObserver", "shared_observer_factory", "straight-line owned observer object and stable aliases wire next/error/complete to owned callbacks with independently checked value/error/status and byte-prefix bodies; object escapes, duplicate slots and ambiguous origins refuse", f));
    }
    if forward_error(model, site, function, body).is_some() {
        claims.push(proposal("forwardError", "shared_observer_error_forwarder", "one unchanged error formal forwarded to the same captured receiver's error method through immutable aliases or a two-slot receiver/method tuple", f));
    }
    if let Some(tagged) = guarded_forward(model, site, function, body) {
        claims.push(proposal(if tagged { "forwardTaggedValueOrError" } else { "forwardValueOrError" }, "shared_guarded_forwarder", "sole try invokes a distinct ordered single-origin callable with unchanged formal and optional numeric tag; sole catch forwards exactly the caught binding to a captured error receiver; no finally or return", f));
    }
    claims
}
