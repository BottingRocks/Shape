//! Lowercase method / URL metadata protocol. Native XHR identity is not inferred.
use crate::naming::support::interaction::*;
use crate::naming::*;
use oxc_span::GetSpan;

type Sym = SymbolId;
#[derive(Clone, Copy)]
struct Store {
    body: NodeId,
    end: u32,
}
fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    if let Statement::BlockStatement(b) = s {
        Some(&b.body)
    } else {
        None
    }
}
fn returned(m: &Model<'_, '_>, s: &Statement<'_>, expected: Sym) -> bool {
    matches!(s, Statement::ReturnStatement(r) if r.argument.as_ref().and_then(|e|symbol(m,e))==Some(expected))
}
fn fields<'a>(e: &'a Expression<'a>, names: &[&str]) -> Option<Vec<&'a Expression<'a>>> {
    let Expression::ObjectExpression(o) = e else {
        return None;
    };
    if o.properties.len() != names.len() {
        return None;
    }
    o.properties
        .iter()
        .zip(names)
        .map(|(p, name)| {
            let ObjectPropertyKind::ObjectProperty(p) = p else {
                return None;
            };
            (!p.computed
                && !p.method
                && !p.shorthand
                && p.kind == PropertyKind::Init
                && property_key_name(&p.key).as_deref() == Some(*name))
            .then_some(&p.value)
        })
        .collect()
}
fn text(e: &Expression<'_>, value: &str) -> bool {
    matches!(e, Expression::StringLiteral(s) if s.value==value)
}
fn undefined(e: &Expression<'_>) -> bool {
    matches!(e,Expression::UnaryExpression(u) if u.operator.as_str()=="void" && number(&u.argument,0.0))
}
fn indexed<'a>(e: &'a Expression<'a>, index: f64) -> Option<&'a Expression<'a>> {
    let Expression::ComputedMemberExpression(p) = e else {
        return None;
    };
    (!p.optional && number(&p.expression, index)).then_some(&p.object)
}
fn args_index(m: &Model<'_, '_>, e: &Expression<'_>, formal: Sym, index: f64) -> bool {
    indexed(e, index)
        .and_then(|e| member(e, "args"))
        .and_then(|e| symbol(m, e))
        == Some(formal)
}
fn null_test<'a>(e: &'a Expression<'a>, op: &str) -> Option<&'a Expression<'a>> {
    let (a, b) = binary(e, op)?;
    matches!(b, Expression::NullLiteral(_)).then_some(a)
}
fn set_member<'a>(
    m: &Model<'_, '_>,
    s: &'a Statement<'a>,
    obj: Sym,
    key: &str,
) -> Option<&'a Expression<'a>> {
    let a = assignment(expression_statement(s)?)?;
    let AssignmentTarget::StaticMemberExpression(p) = &a.left else {
        return None;
    };
    (!p.optional && p.property.name == key && symbol(m, &p.object) == Some(obj)).then_some(&a.right)
}
fn set_symbol<'a>(m: &Model<'_, '_>, s: &'a Statement<'a>) -> Option<(Sym, &'a Expression<'a>)> {
    let a = assignment(expression_statement(s)?)?;
    Some((target_symbol(m, &a.left)?, &a.right))
}
fn mutable_variable(m: &Model<'_, '_>, s: Sym) -> bool {
    matches!(m.nodes.parent_kind(m.scoping.symbol_declaration(s)), AstKind::VariableDeclaration(d) if matches!(d.kind, VariableDeclarationKind::Var | VariableDeclarationKind::Let))
}
fn local(m: &Model<'_, '_>, v: &VariableDeclarator<'_>, writes: usize) -> Option<Sym> {
    let s = plain_var(m, v)?;
    (m.binding_writes(s) == writes && (writes == 0 || mutable_variable(m, s))).then_some(s)
}

/// Lift a deferred declaration through every actual reference. Expressions and
/// unknown aliases refuse; declarations nested in ordinary blocks are tracked
/// by their external declaration symbol, never a named expression's private id.
/// A store dominates only inside its own sequential statement list. Crossing an
/// enclosing function that contains the store cannot make a future store ready.
fn ready(
    m: &Model<'_, '_>,
    node: NodeId,
    stores: &[Store],
    seen: &mut BTreeSet<Sym>,
    depth: usize,
) -> bool {
    if depth > 16 {
        return false;
    }
    let mut child = node;
    for _ in 0..128 {
        let parent = m.nodes.parent_id(child);
        if let AstKind::Function(f) = m.nodes.kind(child) {
            if f.r#type != FunctionType::FunctionDeclaration
                || stores
                    .iter()
                    .any(|s| f.span.contains_inclusive(m.nodes.kind(s.body).span()))
            {
                return false;
            }
            let Some(s) = f.id.as_ref().and_then(|i| i.symbol_id.get()) else {
                return false;
            };
            if m.binding_writes(s) != 0
                || !m.scoping.symbol_redeclarations(s).is_empty()
                || !seen.insert(s)
            {
                return false;
            }
            let refs: Vec<_> = m.scoping.get_resolved_references(s).collect();
            let result = !refs.is_empty()
                && refs
                    .iter()
                    .all(|r| ready(m, r.node_id(), stores, seen, depth + 1));
            seen.remove(&s);
            return result;
        }
        if stores
            .iter()
            .any(|s| s.body == parent && m.nodes.kind(child).span().start >= s.end)
        {
            return true;
        }
        if parent == child {
            return false;
        }
        child = parent;
    }
    false
}
fn callable_ready(m: &Model<'_, '_>, site: FunctionSite, stores: &[Store]) -> bool {
    let Some(s) = site.binding_symbol else {
        return false;
    };
    if m.binding_writes(s) != 0 || !m.scoping.symbol_redeclarations(s).is_empty() {
        return false;
    }
    let refs: Vec<_> = m.scoping.get_resolved_references(s).collect();
    !refs.is_empty()
        && refs
            .iter()
            .all(|r| ready(m, r.node_id(), stores, &mut BTreeSet::from([s]), 0))
}
fn store(m: &Model<'_, '_>, assignment_node: NodeId, end: u32) -> Option<Store> {
    let statement = m.nodes.parent_id(assignment_node);
    let body = m.nodes.parent_id(statement);
    (matches!(m.nodes.kind(statement), AstKind::ExpressionStatement(_))
        && matches!(
            m.nodes.kind(body),
            AstKind::BlockStatement(_) | AstKind::FunctionBody(_) | AstKind::Program(_)
        ))
    .then_some(Store { body, end })
}

/// All epochs contain a fresh single string array; no table alias, call,
/// property mutation or deferred writer is accepted. Strings themselves may
/// serve as opaque property keys. The actual literal is never a name oracle.
fn key_table(m: &Model<'_, '_>, site: FunctionSite, s: Sym) -> Option<()> {
    captured_symbol(m, site, s)?;
    let declaration = m.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = m.nodes.kind(declaration) else {
        return None;
    };
    let owner = m.owning_site(declaration)?.node_id;
    let literal = |e: &Expression<'_>| {
        matches!(e,Expression::ArrayExpression(a) if a.elements.len()==1
        && matches!(&a.elements[0],ArrayExpressionElement::StringLiteral(t) if !t.value.is_empty()))
    };
    let mut stores = Vec::new();
    if let Some(e) = &v.init {
        if !literal(e) || m.binding_writes(s) != 0 {
            return None;
        }
        let statement = m.nodes.parent_id(declaration);
        let body = m.nodes.parent_id(statement);
        if !matches!(
            m.nodes.kind(body),
            AstKind::BlockStatement(_) | AstKind::FunctionBody(_) | AstKind::Program(_)
        ) {
            return None;
        }
        stores.push(Store {
            body,
            end: v.span.end,
        });
    }
    for r in m.scoping.get_resolved_references(s) {
        let node = r.node_id();
        let p = m.nodes.parent_id(node);
        if r.is_write() {
            let AstKind::AssignmentExpression(a) = m.nodes.kind(p) else {
                return None;
            };
            if a.operator != AssignmentOperator::Assign
                || target_symbol(m, &a.left) != Some(s)
                || !literal(&a.right)
                || a.span.start < v.span.end
                || m.owning_site(p)?.node_id != owner
            {
                return None;
            }
            stores.push(store(m, p, a.span.end)?);
        } else {
            let span = match m.nodes.kind(p) {
                AstKind::ComputedMemberExpression(v)
                    if !v.optional && v.object.span() == m.nodes.kind(node).span() =>
                {
                    v.span
                }
                AstKind::StaticMemberExpression(v)
                    if !v.optional
                        && v.property.name == "length"
                        && v.object.span() == m.nodes.kind(node).span() =>
                {
                    v.span
                }
                _ => return None,
            };
            // Identifier reads also occur inside assignment / delete targets.
            let mut reached_root = false;
            let mut ancestor = p;
            for _ in 0..128 {
                match m.nodes.kind(ancestor) {
                    AstKind::AssignmentExpression(a) if a.left.span().contains_inclusive(span) => {
                        return None
                    }
                    AstKind::UpdateExpression(a) if a.span.contains_inclusive(span) => return None,
                    AstKind::UnaryExpression(a)
                        if a.operator.as_str() == "delete" && a.span.contains_inclusive(span) =>
                    {
                        return None
                    }
                    AstKind::ForInStatement(a) if a.left.span().contains_inclusive(span) => {
                        return None
                    }
                    AstKind::ForOfStatement(a) if a.left.span().contains_inclusive(span) => {
                        return None
                    }
                    _ => {}
                }
                let next = m.nodes.parent_id(ancestor);
                if next == ancestor {
                    reached_root = true;
                    break;
                }
                ancestor = next;
            }
            if !reached_root {
                return None;
            }
        }
    }
    (!stores.is_empty() && callable_ready(m, site, &stores)).then_some(())
}
/// A callable value predicate as well as an epoch proof: direct declaration or
/// one immutable initialized alias / sole direct assignment to a declaration.
/// The reporter's implementation is deliberately opaque, including throws.
fn reporter(m: &Model<'_, '_>, site: FunctionSite, s: Sym) -> Option<()> {
    if !m.scoping.symbol_redeclarations(s).is_empty() {
        return None;
    }
    let d = m.scoping.symbol_declaration(s);
    if let AstKind::Function(f) = m.nodes.kind(d) {
        return (f.r#type == FunctionType::FunctionDeclaration && m.binding_writes(s) == 0)
            .then_some(());
    }
    captured_symbol(m, site, s)?;
    let AstKind::VariableDeclarator(v) = m.nodes.kind(d) else {
        return None;
    };
    let (value, epoch) = if let Some(e) = &v.init {
        if m.binding_writes(s) != 0 {
            return None;
        }
        let body = m.nodes.parent_id(m.nodes.parent_id(d));
        (
            e,
            Store {
                body,
                end: v.span.end,
            },
        )
    } else {
        if m.binding_writes(s) != 1 {
            return None;
        }
        let r = m
            .scoping
            .get_resolved_references(s)
            .find(|r| r.is_write())?;
        let p = m.nodes.parent_id(r.node_id());
        let AstKind::AssignmentExpression(a) = m.nodes.kind(p) else {
            return None;
        };
        if a.operator != AssignmentOperator::Assign || target_symbol(m, &a.left) != Some(s) {
            return None;
        }
        (&a.right, store(m, p, a.span.end)?)
    };
    let source = symbol(m, value)?;
    let AstKind::Function(f) = m.nodes.kind(m.scoping.symbol_declaration(source)) else {
        return None;
    };
    if f.r#type != FunctionType::FunctionDeclaration
        || m.binding_writes(source) != 0
        || !m.scoping.symbol_redeclarations(source).is_empty()
    {
        return None;
    }
    callable_ready(m, site, &[epoch]).then_some(())
}
fn scratch(m: &Model<'_, '_>, site: FunctionSite, s: Sym) -> Option<()> {
    captured_symbol(m, site, s)?;
    let d = m.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = m.nodes.kind(d) else {
        return None;
    };
    if !text(v.init.as_ref()?, "") || m.binding_writes(s) != 1 || !mutable_variable(m, s) {
        return None;
    }
    let write = m
        .scoping
        .get_resolved_references(s)
        .find(|r| r.is_write())?;
    if !site
        .span
        .contains_inclusive(m.nodes.kind(write.node_id()).span())
    {
        return None;
    }
    let body = m.nodes.parent_id(m.nodes.parent_id(d));
    callable_ready(
        m,
        site,
        &[Store {
            body,
            end: v.span.end,
        }],
    )
    .then_some(())
}
fn protocol(m: &Model<'_, '_>, site: FunctionSite, f: &Function<'_>) -> Option<()> {
    if f.r#type != FunctionType::FunctionDeclaration
        || f.r#async
        || f.generator
        || f.params.rest.is_some()
    {
        return None;
    }
    let [a, b] = f.params.items.as_slice() else {
        return None;
    };
    let a = plain_param(m, a)?;
    let b = plain_param(m, b)?;
    let body = f.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let [Statement::TryStatement(t)] = body.statements.as_slice() else {
        return None;
    };
    if t.finalizer.is_some() {
        return None;
    }
    let [Statement::IfStatement(bypass), Statement::IfStatement(args)] = t.block.body.as_slice()
    else {
        return None;
    };
    let Expression::LogicalExpression(both) = &bypass.test else {
        return None;
    };
    if both.operator.as_str() != "&&"
        || symbol(m, null_test(&both.left, "!=")?) != Some(b)
        || member(null_test(&both.right, "!=")?, "bypassResult").and_then(|e| symbol(m, e))
            != Some(b)
        || bypass.alternate.is_some()
    {
        return None;
    }
    let [r] = block(&bypass.consequent)? else {
        return None;
    };
    if !returned(m, r, b) {
        return None;
    }
    let Expression::LogicalExpression(either) = &args.test else {
        return None;
    };
    let (len, two) = binary(&either.right, "<")?;
    if either.operator.as_str() != "||"
        || member(null_test(&either.left, "==")?, "args").and_then(|e| symbol(m, e)) != Some(a)
        || member(len, "length")
            .and_then(|e| member(e, "args"))
            .and_then(|e| symbol(m, e))
            != Some(a)
        || !number(two, 2.0)
    {
        return None;
    }
    let [r] = block(&args.consequent)? else {
        return None;
    };
    if !returned(m, r, b) {
        return None;
    }
    let [Statement::IfStatement(method_guard), r] = block(args.alternate.as_ref()?)? else {
        return None;
    };
    if !returned(m, r, b)
        || method_guard.alternate.is_some()
        || !args_index(m, null_test(&method_guard.test, "!=")?, a, 0.0)
    {
        return None;
    }
    let [method, url, descriptor, record, value, define] = block(&method_guard.consequent)? else {
        return None;
    };
    let (method_symbol, method_value, local_method) = if let Some(v) = variable(method) {
        (local(m, v, 1)?, v.init.as_ref()?, true)
    } else {
        let (s, e) = set_symbol(m, method)?;
        scratch(m, site, s)?;
        (s, e, false)
    };
    let (empty, lower) = binary(method_value, "+")?;
    let lower = call(lower)?;
    if !text(empty, "")
        || !lower.arguments.is_empty()
        || !args_index(m, member(&lower.callee, "toLowerCase")?, a, 0.0)
    {
        return None;
    }
    let (url_symbol, url_value) = set_symbol(m, url)?;
    if !args_index(m, url_value, a, 1.0) {
        return None;
    }
    scratch(m, site, url_symbol)?;
    let d = variable(descriptor)?;
    let ds = local(m, d, 0)?;
    let flags = fields(
        d.init.as_ref()?,
        &["writable", "configurable", "enumerable"],
    )?;
    if !boolean(flags[0], true) || !boolean(flags[1], true) || !boolean(flags[2], false) {
        return None;
    }
    let (rs, record_value) = if local_method {
        let (s, e) = set_symbol(m, record)?;
        if s != method_symbol {
            return None;
        }
        (s, e)
    } else {
        let v = variable(record)?;
        (local(m, v, 0)?, v.init.as_ref()?)
    };
    let metadata = fields(record_value, &["method", "url"])?;
    if symbol(m, metadata[0]) != Some(method_symbol)
        || symbol(m, metadata[1]) != Some(url_symbol)
        || symbol(m, set_member(m, value, ds, "value")?) != Some(rs)
    {
        return None;
    }
    let c = call(expression_statement(define)?)?;
    let [receiver, key, desc] = c.arguments.as_slice() else {
        return None;
    };
    let object = member(&c.callee, "defineProperty")?;
    if !matches!(object,Expression::Identifier(i) if i.name=="Object" && i.reference_id.get().is_some_and(|r|m.scoping.get_reference(r).symbol_id().is_none()))
        || member(receiver.as_expression()?, "thisObj").and_then(|e| symbol(m, e)) != Some(a)
        || symbol(m, desc.as_expression()?) != Some(ds)
    {
        return None;
    }
    let table = symbol(m, indexed(key.as_expression()?, 0.0)?)?;
    key_table(m, site, table)?;
    let catch = t.handler.as_ref()?;
    let error = plain_binding(m, &catch.param.as_ref()?.pattern)?;
    if m.binding_writes(error) != 0 {
        return None;
    }
    let [diagnostic, write, report, r] = catch.body.body.as_slice() else {
        return None;
    };
    if !returned(m, r, b) {
        return None;
    }
    let v = variable(diagnostic)?;
    let record = local(m, v, 0)?;
    let diagnostic_value = v.init.as_ref()?;
    if let Some(values) = fields(diagnostic_value, &["message", "stack"]) {
        if !text(values[0], "2.31") || !undefined(values[1]) {
            return None;
        }
        let (e, empty) = binary(set_member(m, write, record, "stack")?, "+")?;
        if symbol(m, e) != Some(error) || !text(empty, "") {
            return None;
        }
    } else {
        fields(diagnostic_value, &[])?;
        let (prefix, e) = binary(set_member(m, write, record, "message")?, "+")?;
        if !text(prefix, "2.31:") || symbol(m, e) != Some(error) {
            return None;
        }
    }
    let c = call(expression_statement(report)?)?;
    let [arg] = c.arguments.as_slice() else {
        return None;
    };
    if symbol(m, arg.as_expression()?) != Some(record) {
        return None;
    }
    let reporter_symbol = symbol(m, &c.callee)?;
    reporter(m, site, reporter_symbol)?;
    let mut unique = BTreeSet::new();
    [
        a,
        b,
        method_symbol,
        url_symbol,
        ds,
        table,
        error,
        record,
        reporter_symbol,
    ]
    .into_iter()
    .all(|s| unique.insert(s))
    .then_some(())
}
pub(in crate::naming) fn evaluate(m: &Model<'_, '_>, site: FunctionSite, facts: &Facts<'_>) -> Vec<Proposal> {
    let AstKind::Function(f) = m.nodes.kind(site.node_id) else {
        return vec![];
    };
    if protocol(m, site, f).is_none() {
        return vec![];
    }
    vec![proposal("storeLowercaseMethodAndUrlMetadata", "shared_lowercase_method_url_metadata",
        "entire declaration protocol: bypass/argument/null guards; ordered lowercase method and URL scratch stores, exact descriptor/metadata record identities, global Object.defineProperty on formal thisObj at captured string-table index zero, then preserve result; catch constructs exactly the 2.31 diagnostic variant and calls a proved captured declaration-valued reporter before returning result (reporter throws propagate); all scratch writers, key-table stores/non-escape and exposure readiness are checked without assuming asynchronous registration; native XHR identity and reporter implementation remain opaque", facts)]
}
