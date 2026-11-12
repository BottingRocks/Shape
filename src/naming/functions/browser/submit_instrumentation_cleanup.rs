//! Local guarded-cleanup proof. No last-textual-value or native-origin lookup.
//! A closure can be hoisted before its data is assigned: only its actual
//! publication matters. Shared state is proved separately for each exclusive
//! setup arm, and the registration path is the only way to enable cleanup.
use super::*;
use oxc_span::Span;

#[derive(Clone, Copy)]
struct Listener {
    receiver: Symbol,
    handler: Symbol,
    navigation: bool,
}
struct Cleanup {
    document: Symbol,
    click: Symbol,
    style: Symbol,
    flag: Symbol,
    registries: Vec<Symbol>,
    submit: Option<Listener>,
    navigate: Option<Listener>,
}

fn one<'a>(s: &'a Statement<'a>) -> Option<&'a Statement<'a>> {
    if let Statement::BlockStatement(b) = s {
        if let [s] = b.body.as_slice() {
            return Some(s);
        }
        None
    } else {
        Some(s)
    }
}
fn null_guard<'a>(e: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    let Expression::BinaryExpression(b) = e else {
        return None;
    };
    (b.operator.as_str() == "!=" && matches!(b.right, Expression::NullLiteral(_)))
        .then_some(&b.left)
}
fn listener(
    model: &Model<'_, '_>,
    site: FunctionSite,
    s: &Statement<'_>,
    event: &str,
) -> Option<(Listener, bool)> {
    let (s, guard) = if let Statement::IfStatement(i) = s {
        if i.alternate.is_some() {
            return None;
        }
        (one(&i.consequent)?, Some(null_guard(&i.test)?))
    } else {
        (s, None)
    };
    let c = statement_call(s)?;
    let args = arity(c, 2)?;
    if !string(args[0], event) {
        return None;
    }
    let target = member(&c.callee, "removeEventListener")?;
    let navigation = event == "navigate";
    let receiver = if navigation {
        data_capture(model, site, member(target, "navigation")?)?
    } else {
        data_capture(model, site, target)?
    };
    let handler = data_capture(model, site, args[1])?;
    if let Some(guard) = guard {
        if navigation {
            if symbol(model, member(guard, "navigation")?) != Some(receiver) {
                return None;
            }
        } else if symbol(model, guard) != Some(handler) {
            return None;
        }
    }
    Some((
        Listener {
            receiver,
            handler,
            navigation,
        },
        guard.is_some(),
    ))
}
fn shape(model: &Model<'_, '_>, site: FunctionSite) -> Option<Cleanup> {
    let AstKind::Function(f) = model.nodes.kind(site.node_id) else {
        return None;
    };
    let b = f.body.as_ref()?;
    if f.r#async
        || f.generator
        || !f.params.items.is_empty()
        || f.params.rest.is_some()
        || !directives(b)
    {
        return None;
    }
    let [Statement::TryStatement(t)] = b.statements.as_slice() else {
        return None;
    };
    let catch = t.handler.as_ref()?;
    let BindingPattern::BindingIdentifier(error) = &catch.param.as_ref()?.pattern else {
        return None;
    };
    if t.finalizer.is_some()
        || !catch.body.body.is_empty()
        || !unchanged(model, error.symbol_id.get()?)
    {
        return None;
    }
    let [
        click,
        Statement::IfStatement(enabled),
        Statement::IfStatement(head),
    ] = t.block.body.as_slice()
    else {
        return None;
    };
    if enabled.alternate.is_some() || head.alternate.is_some() {
        return None;
    }
    let c = statement_call(click)?;
    let a = arity(c, 2)?;
    if !string(a[0], "click") {
        return None;
    }
    let document = data_capture(model, site, member(&c.callee, "removeEventListener")?)?;
    let click = data_capture(model, site, a[1])?;
    let flag = truth_test(model, &enabled.test)?;
    if !capture_identity(model, site, flag, enabled.test.span().start) {
        return None;
    }
    let Statement::BlockStatement(clean) = &enabled.consequent else {
        return None;
    };
    let (reset, operations) = clean.body.split_last()?;
    if !(2..=6).contains(&operations.len()) {
        return None;
    }
    let reset = assignment(reset)?;
    if assign_symbol(model, reset) != Some(flag)
        || !matches!(&reset.right, Expression::BooleanLiteral(v) if !v.value)
    {
        return None;
    }
    let mut registries = Vec::new();
    let mut submit = None;
    let mut navigate = None;
    let mut guarded = false;
    for s in operations {
        if let Some(c) = statement_call(s) {
            if let Some(r) = member(&c.callee, "clear") {
                if submit.is_some() || navigate.is_some() || arity(c, 0).is_none() {
                    return None;
                }
                let r = data_capture(model, site, r)?;
                if registries.contains(&r) {
                    return None;
                }
                registries.push(r);
                continue;
            }
        }
        if let Some((l, g)) = listener(model, site, s, "submit") {
            if submit.is_some() || navigate.is_some() {
                return None;
            }
            submit = Some(l);
            guarded |= g;
        } else if let Some((l, g)) = listener(model, site, s, "navigate") {
            if navigate.is_some() {
                return None;
            }
            navigate = Some(l);
            guarded |= g;
        } else {
            return None;
        }
    }
    if !guarded || registries.is_empty() {
        return None;
    }
    if symbol(model, member(&head.test, "head")?) != Some(document) {
        return None;
    }
    let c = statement_call(one(&head.consequent)?)?;
    if symbol(model, member(member(&c.callee, "removeChild")?, "head")?) != Some(document) {
        return None;
    }
    let style = data_capture(model, site, arity(c, 1)?[0])?;
    Some(Cleanup {
        document,
        click,
        style,
        flag,
        registries,
        submit,
        navigate,
    })
}

fn function<'a>(model: &Model<'_, 'a>, s: Symbol) -> Option<&'a Function<'a>> {
    match model.nodes.kind(model.scoping.symbol_declaration(s)) {
        AstKind::Function(f) => Some(f),
        _ => None,
    }
}
fn enclosing(model: &Model<'_, '_>, node: NodeId) -> Option<FunctionSite> {
    model.enclosing_function(model.nodes.parent_id(node))
}
fn writes<'a>(model: &Model<'_, 'a>, s: Symbol) -> Option<Vec<&'a AssignmentExpression<'a>>> {
    if !model.scoping.symbol_redeclarations(s).is_empty() {
        return None;
    }
    let mut found = Vec::new();
    for r in model
        .scoping
        .get_resolved_references(s)
        .filter(|r| r.is_write())
    {
        let AstKind::AssignmentExpression(a) = model.nodes.parent_kind(r.node_id()) else {
            return None;
        };
        if a.operator.as_str() != "=" || assign_symbol(model, a) != Some(s) {
            return None;
        }
        found.push(a);
    }
    Some(found)
}
fn same_body_statement(model: &Model<'_, '_>, node: NodeId, body: Span) -> bool {
    let p = model.nodes.parent_id(node);
    matches!(model.nodes.kind(p), AstKind::ExpressionStatement(_))
        && model.nodes.kind(model.nodes.parent_id(p)).span() == body
}
fn node_for(model: &Model<'_, '_>, span: Span) -> Option<NodeId> {
    model.nodes.iter_enumerated().find_map(|(id, n)| {
        (n.kind().span() == span
            && matches!(
                n.kind(),
                AstKind::AssignmentExpression(_) | AstKind::VariableDeclarator(_)
            ))
        .then_some(id)
    })
}
fn property_write(model: &Model<'_, '_>, s: Symbol) -> bool {
    model.scoping.get_resolved_references(s).any(|r| {
        let mut n = model.nodes.parent_id(r.node_id());
        loop {
            match model.nodes.kind(n) {
                AstKind::AssignmentExpression(a) => {
                    return a
                        .left
                        .span()
                        .contains_inclusive(model.nodes.kind(r.node_id()).span())
                        && assign_symbol(model, a) != Some(s);
                }
                AstKind::UpdateExpression(_)
                | AstKind::UnaryExpression(UnaryExpression {
                    operator: oxc_syntax::operator::UnaryOperator::Delete,
                    ..
                }) => return true,
                AstKind::StaticMemberExpression(_) | AstKind::ComputedMemberExpression(_) => {
                    n = model.nodes.parent_id(n)
                }
                _ => return false,
            }
        }
    })
}
fn binding(p: &BindingPattern<'_>) -> Option<Symbol> {
    match p {
        BindingPattern::BindingIdentifier(i) => i.symbol_id.get(),
        _ => None,
    }
}
fn object_init(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Symbol> {
    let Expression::ObjectExpression(o) = e else {
        return None;
    };
    let [ObjectPropertyKind::ObjectProperty(p)] = o.properties.as_slice() else {
        return None;
    };
    if p.computed
        || p.method
        || p.kind != PropertyKind::Init
        || property_key_name(&p.key)?.as_str() != "init"
    {
        return None;
    }
    symbol(model, &p.value)
}

/// Execute the factory's small straight-line publication language. Calls are
/// permitted only for style creation, before either returned closure escapes.
/// Every reference of the teardown and init bindings is checked, not just the
/// final assignment's source position.
fn factory_ready(
    model: &Model<'_, '_>,
    site: FunctionSite,
    c: &Cleanup,
) -> Option<(FunctionSite, Symbol)> {
    let owner = enclosing(model, site.node_id)?;
    let AstKind::Function(f) = model.nodes.kind(owner.node_id) else {
        return None;
    };
    if f.r#async || f.generator || !plain_params(model, &f.params) || f.params.items.len() != 1 {
        return None;
    }
    let formal = binding(&f.params.items[0].pattern)?;
    let body = f.body.as_ref()?;
    let teardown = site.binding_symbol?;
    let mut values: BTreeMap<Symbol, &Expression<'_>> = BTreeMap::new();
    let mut declared = BTreeSet::new();
    let mut positions = BTreeMap::new();
    let mut publication = None;
    let mut init = None;
    let mut result = None;
    let mut host = None;
    let mut returned = false;
    for s in &body.statements {
        if returned {
            return None;
        }
        match s {
            Statement::FunctionDeclaration(_) => {}
            Statement::VariableDeclaration(d) => {
                for v in &d.declarations {
                    let id = binding(&v.id)?;
                    if let Some(e) = v.init.as_ref() {
                        immediate_reads_ready(model, owner, formal, &values, e, 0)?;
                        local_value(model, owner, c, formal, &values, &mut host, e)?;
                        if let Some(i) = object_init(model, e) {
                            init = Some(i);
                            result = Some(id);
                        }
                        if values.insert(id, e).is_some() {
                            return None;
                        }
                        positions.insert(id, v.span.end);
                    }
                    declared.insert(id);
                }
            }
            Statement::ExpressionStatement(_) => {
                let a = assignment(s)?;
                if let Some(id) = assign_symbol(model, a) {
                    if !owner.span.contains_inclusive(model.scoping.symbol_span(id)) {
                        return None;
                    }
                    // A hoisted var slot exists on entry; a lexical slot does
                    // not permit assignment before its declaration executes.
                    // Neither kind becomes a ready producer merely by existing.
                    if !declared.contains(&id) {
                        let declaration = model.scoping.symbol_declaration(id);
                        if !matches!(model.nodes.parent_kind(declaration), AstKind::VariableDeclaration(d) if d.kind==VariableDeclarationKind::Var)
                        {
                            return None;
                        }
                    }
                    immediate_reads_ready(model, owner, formal, &values, &a.right, 0)?;
                    local_value(model, owner, c, formal, &values, &mut host, &a.right)?;
                    if values.insert(id, &a.right).is_some() {
                        return None;
                    }
                    positions.insert(id, a.span.end);
                } else {
                    let AssignmentTarget::StaticMemberExpression(m) = &a.left else {
                        return None;
                    };
                    immediate_reads_ready(model, owner, formal, &values, &m.object, 0)?;
                    immediate_reads_ready(model, owner, formal, &values, &a.right, 0)?;
                    if m.optional
                        || m.property.name != "teardown"
                        || symbol(model, &m.object) != result
                        || symbol(model, &a.right) != Some(teardown)
                        || publication.is_some()
                    {
                        return None;
                    }
                    for id in [c.document, c.style, c.click] {
                        if !positions.contains_key(&id) {
                            return None;
                        }
                    }
                    publication = Some(a.right.span());
                }
            }
            Statement::ReturnStatement(r) => {
                immediate_reads_ready(model, owner, formal, &values, r.argument.as_ref()?, 0)?;
                if symbol(model, r.argument.as_ref()?) != result || publication.is_none() {
                    return None;
                }
                returned = true;
            }
            _ => return None,
        }
    }
    let init = init?;
    let init_function = function(model, init)?;
    if !returned
        || init_function.r#async
        || init_function.generator
        || !init_function.params.items.is_empty()
    {
        return None;
    }
    let refs: Vec<_> = model.scoping.get_resolved_references(teardown).collect();
    if refs.len() != 1 || model.nodes.kind(refs[0].node_id()).span() != publication? {
        return None;
    }
    let refs: Vec<_> = model.scoping.get_resolved_references(init).collect();
    if refs.len() != 1 {
        return None;
    }
    let AstKind::ObjectProperty(p) = model.nodes.parent_kind(refs[0].node_id()) else {
        return None;
    };
    if p.value.span().start < *positions.get(&c.style)?
        || p.value.span().start < *positions.get(&c.document)?
    {
        return None;
    }
    for id in [c.document, c.style, c.click, host?] {
        if !plain_origin(model, id) || property_write(model, id) && id != c.style {
            return None;
        }
        let count = writes(model, id)?.len() + usize::from(model.initializer(id).is_some());
        if count != 1 || !positions.contains_key(&id) {
            return None;
        }
    }
    // The retained click handler must be the same local function, initialized
    // as data before publication, not a mutable/late external callable alias.
    if symbol(model, member(values.get(&c.document)?, "document")?) != host {
        return None;
    }
    if !matches!(values.get(&c.style)?, Expression::CallExpression(_)) {
        return None;
    }
    let callback = symbol(model, values.get(&c.click)?)?;
    let callback = function(model, callback)?;
    if !owner.span.contains_inclusive(callback.span)
        || !unchanged(model, symbol(model, values.get(&c.click)?)?)
    {
        return None;
    }
    Some((owner, init))
}

/// Check immediate reads *before* recording the destination's executed store.
/// This one invariant applies to member/tuple creation, aliases, callee and
/// receiver evaluation, object publication and returns. Function declarations
/// are hoisted values; their deferred bodies are not executed at declaration.
/// Uninitialized let/var slots never enter `values` until a producer completes.
fn immediate_reads_ready(
    model: &Model<'_, '_>,
    owner: FunctionSite,
    formal: Symbol,
    values: &BTreeMap<Symbol, &Expression<'_>>,
    e: &Expression<'_>,
    depth: usize,
) -> Option<()> {
    if depth > 32 {
        return None;
    }
    let read = |e| immediate_reads_ready(model, owner, formal, values, e, depth + 1);
    match e {
        Expression::Identifier(_) => {
            let s = symbol(model, e)?;
            if s == formal || values.contains_key(&s) {
                return Some(());
            }
            let declaration = model.scoping.symbol_declaration(s);
            (matches!(model.nodes.kind(declaration),AstKind::Function(_)) &&
                matches!(model.nodes.parent_kind(declaration),AstKind::FunctionBody(b) if Some(b.span)==owner.body_span))
                .then_some(())
        }
        Expression::NullLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BooleanLiteral(_) => Some(()),
        Expression::StaticMemberExpression(m) if !m.optional => read(&m.object),
        Expression::ComputedMemberExpression(m) if !m.optional => {
            read(&m.object)?;
            read(&m.expression)
        }
        Expression::ArrayExpression(a) => {
            for e in &a.elements {
                read(e.as_expression()?)?;
            }
            Some(())
        }
        Expression::ObjectExpression(o) => {
            for p in &o.properties {
                let ObjectPropertyKind::ObjectProperty(p) = p else {
                    return None;
                };
                if p.computed || p.method || p.kind != PropertyKind::Init {
                    return None;
                }
                read(&p.value)?;
            }
            Some(())
        }
        Expression::CallExpression(c) if !c.optional => {
            read(&c.callee)?;
            for a in &c.arguments {
                read(a.as_expression()?)?;
            }
            Some(())
        }
        _ => None,
    }
}
fn local_value(
    model: &Model<'_, '_>,
    owner: FunctionSite,
    c: &Cleanup,
    formal: Symbol,
    values: &BTreeMap<Symbol, &Expression<'_>>,
    host: &mut Option<Symbol>,
    e: &Expression<'_>,
) -> Option<()> {
    match e {
        Expression::NullLiteral(_) => Some(()),
        Expression::Identifier(_) => {
            let s = symbol(model, e)?;
            (s == formal
                || values.contains_key(&s)
                || function(model, s).is_some_and(|f| owner.span.contains_inclusive(f.span)))
            .then_some(())
        }
        Expression::StaticMemberExpression(m) if !m.optional => {
            let s = symbol(model, &m.object)?;
            if m.property.name == "global"
                && values
                    .get(&s)
                    .is_some_and(|e| symbol(model, e) == Some(formal))
            {
                Some(())
            } else if m.property.name == "document"
                && values
                    .get(&s)
                    .is_some_and(|e| member(e, "global").is_some())
            {
                *host = Some(s);
                Some(())
            } else {
                None
            }
        }
        Expression::ArrayExpression(a) => {
            let [
                ArrayExpressionElement::Identifier(id),
                ArrayExpressionElement::StaticMemberExpression(m),
            ] = a.elements.as_slice()
            else {
                return None;
            };
            (crate::analysis::identifier_symbol(id, model.scoping) == Some(c.document)
                && !m.optional
                && m.property.name == "createElement"
                && symbol(model, &m.object) == Some(c.document))
            .then_some(())
        }
        Expression::ObjectExpression(_) => {
            let init = object_init(model, e)?;
            function(model, init)
                .filter(|f| owner.span.contains_inclusive(f.span))
                .map(|_| ())
        }
        Expression::CallExpression(call) if !call.optional => {
            if let Some(doc) = member(&call.callee, "createElement") {
                return (symbol(model, doc) == Some(c.document)
                    && arity(call, 1).is_some_and(|a| string(a[0], "style"))
                    && values.contains_key(&c.document))
                .then_some(());
            }
            let tuple = member(&call.callee, "call")?;
            let Expression::ComputedMemberExpression(m) = tuple else {
                return None;
            };
            let t = symbol(model, &m.object)?;
            let a = arity(call, 2)?;
            let Expression::ComputedMemberExpression(receiver) = a[0] else {
                return None;
            };
            if m.optional
                || receiver.optional
                || !matches!(&m.expression,Expression::NumericLiteral(n) if n.value==1.0)
                || !matches!(&receiver.expression,Expression::NumericLiteral(n) if n.value==0.0)
                || symbol(model, &receiver.object) != Some(t)
                || !string(a[1], "style")
            {
                return None;
            }
            matches!(values.get(&t)?, Expression::ArrayExpression(_)).then_some(())
        }
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct Epoch {
    identity: Span,
    scope: NodeId,
    sides: [Span; 2],
    // A returning catch excludes the normal continuation, whose stores must
    // execute after the try completes. This is not a generic block allowance.
    continuation_after: Option<u32>,
}
impl Epoch {
    fn contains(self, block: Span, position: u32) -> bool {
        self.sides.contains(&block)
            && (block != self.sides[1] || self.continuation_after.is_none_or(|end| position > end))
    }
}
fn epoch_container(model: &Model<'_, '_>, block: NodeId) -> Option<Epoch> {
    if let AstKind::IfStatement(i) = model.nodes.parent_kind(block) {
        let scope = model.nodes.parent_id(model.nodes.parent_id(block));
        if !matches!(model.nodes.kind(scope), AstKind::FunctionBody(_)) {
            return None;
        }
        return Some(Epoch {
            identity: i.span,
            scope,
            sides: [i.consequent.span(), i.alternate.as_ref()?.span()],
            continuation_after: None,
        });
    }
    let scope = if matches!(model.nodes.kind(block), AstKind::FunctionBody(_)) {
        block
    } else {
        if !matches!(model.nodes.parent_kind(block), AstKind::CatchClause(_)) {
            return None;
        }
        model
            .nodes
            .parent_id(model.nodes.parent_id(model.nodes.parent_id(block)))
    };
    let AstKind::FunctionBody(body) = model.nodes.kind(scope) else {
        return None;
    };
    if !matches!(body.statements.last(), Some(Statement::ReturnStatement(_))) {
        return None;
    }
    let mut found = None;
    for s in &body.statements {
        let Statement::TryStatement(t) = s else {
            continue;
        };
        let Some(catch) = &t.handler else { continue };
        if t.finalizer.is_some()
            || !matches!(catch.body.body.last(), Some(Statement::ReturnStatement(_)))
            || !matches!(t.block.body.as_slice(), [s] if assignment(s).is_some())
        {
            continue;
        }
        if block != scope && model.nodes.kind(block).span() != catch.body.span {
            continue;
        }
        if found.is_some() {
            return None;
        }
        found = Some(Epoch {
            identity: t.span,
            scope,
            sides: [catch.body.span, body.span],
            continuation_after: Some(t.span.end),
        });
    }
    found
}
fn epoch(model: &Model<'_, '_>, owner: FunctionSite) -> Option<(Span, Epoch)> {
    let block = model.nodes.parent_id(owner.node_id);
    let epoch = epoch_container(model, block)?;
    let span = model.nodes.kind(block).span();
    epoch
        .contains(span, owner.span.start)
        .then_some((span, epoch))
}
fn exported(model: &Model<'_, '_>, owner: FunctionSite, block: Span, after: u32) -> Option<()> {
    let s = owner.binding_symbol?;
    if !unchanged(model, s) {
        return None;
    }
    let refs: Vec<_> = model.scoping.get_resolved_references(s).collect();
    let [r] = refs.as_slice() else {
        return None;
    };
    let array = model.nodes.parent_id(r.node_id());
    if !matches!(model.nodes.kind(array), AstKind::ArrayExpression(_)) {
        return None;
    }
    let assign = model.nodes.parent_id(array);
    let AstKind::AssignmentExpression(a) = model.nodes.kind(assign) else {
        return None;
    };
    if a.operator.as_str() != "="
        || a.right.span() != model.nodes.kind(array).span()
        || a.span.start <= after
        || !same_body_statement(model, assign, block)
    {
        return None;
    }
    Some(())
}
fn epoch_store<'a>(
    model: &Model<'_, 'a>,
    s: Symbol,
    block: Span,
) -> Option<&'a AssignmentExpression<'a>> {
    if model.initializer(s).is_some() || !plain_origin(model, s) {
        return None;
    }
    let ws = writes(model, s)?;
    if ws.len() != 2 {
        return None;
    }
    let mut selected = None;
    let mut pair = None;
    let mut sides = BTreeSet::new();
    for a in ws {
        let id = node_for(model, a.span)?;
        let stmt = model.nodes.parent_id(id);
        let parent = model.nodes.parent_id(stmt);
        let span = model.nodes.kind(parent).span();
        let epoch = epoch_container(model, parent)?;
        if !same_body_statement(model, id, span) || !epoch.contains(span, a.span.start) {
            return None;
        }
        if pair.is_some_and(|p| p != epoch.identity) {
            return None;
        }
        pair = Some(epoch.identity);
        sides.insert(span.start);
        if span == block {
            selected = Some(a);
        }
        // The binding belongs to this one activation, outside both arms.
        let declaration = model.scoping.symbol_declaration(s);
        let container = model.nodes.parent_id(model.nodes.parent_id(declaration));
        if container != epoch.scope {
            return None;
        }
    }
    if sides.len() == 2 { selected } else { None }
}

// The shared document's immediate host read needs the same producer-order
// proof in *each* exclusive arm. An earlier document store alone is not a
// witness that its host was ready. Bound aliases must have earlier arm stores.
fn epoch_document<'a>(
    model: &Model<'_, 'a>,
    document: Symbol,
    block: Span,
) -> Option<&'a AssignmentExpression<'a>> {
    let store = epoch_store(model, document, block)?;
    let mut producer = symbol(model, member(&store.right, "document")?)?;
    let mut before = store.span.start;
    for _ in 0..16 {
        let host = epoch_store(model, producer, block)?;
        if host.span.end >= before {
            return None;
        }
        if matches!(&host.right, Expression::Identifier(_))
            && !global_path(model, &host.right).is_empty()
        {
            return Some(store);
        }
        producer = symbol(model, &host.right)?;
        before = host.span.start;
    }
    None
}

fn method<'a>(class: &'a Class<'a>, name: &str) -> Option<&'a Function<'a>> {
    let mut out = None;
    for element in &class.body.body {
        let ClassElement::MethodDefinition(m) = element else {
            return None;
        };
        if m.computed
            || !m.decorators.is_empty()
            || m.r#static
            || m.kind == MethodDefinitionKind::Get
            || m.kind == MethodDefinitionKind::Set
        {
            return None;
        }
        if property_key_name(&m.key)?.as_str() == name {
            if out.is_some() {
                return None;
            }
            out = Some(&*m.value);
        }
    }
    out.filter(|f| !f.r#async && !f.generator && f.params.rest.is_none())
}
fn returns_this(s: &Statement<'_>) -> bool {
    matches!(s,Statement::ReturnStatement(r) if matches!(&r.argument,Some(Expression::ThisExpression(_))))
}
fn callback_array_assignment(s: &Statement<'_>) -> Option<()> {
    let a = assignment(s)?;
    let AssignmentTarget::StaticMemberExpression(m) = &a.left else {
        return None;
    };
    (m.property.name == "callbacks"
        && !m.optional
        && matches!(&m.object, Expression::ThisExpression(_))
        && matches!(&a.right,Expression::ArrayExpression(a) if a.elements.is_empty()))
    .then_some(())
}
/// The lowered base has one hoisted, call-free constructor and four explicit
/// method publications in the same executed epoch. Inspect its own clear and
/// register bodies; a class/factory name or another registry is never a donor.
fn prototype_registry_base(
    model: &Model<'_, '_>,
    base: Symbol,
    argument: Span,
    block: Span,
) -> Option<()> {
    let ctor = function(model, base)?;
    let ctor_id = model.scoping.symbol_declaration(base);
    if !unchanged(model, base)
        || model.nodes.kind(model.nodes.parent_id(ctor_id)).span() != block
        || ctor.r#async
        || ctor.generator
        || !ctor.params.items.is_empty()
        || ctor.params.rest.is_some()
        || !directives(ctor.body.as_ref()?)
    {
        return None;
    }
    let [reset] = ctor.body.as_ref()?.statements.as_slice() else {
        return None;
    };
    callback_array_assignment(reset)?;
    let mut methods = BTreeMap::new();
    let mut factory_uses = 0;
    for r in model.scoping.get_resolved_references(base) {
        if model.nodes.kind(r.node_id()).span() == argument {
            factory_uses += 1;
            continue;
        }
        let prototype = model.nodes.parent_id(r.node_id());
        let AstKind::StaticMemberExpression(p) = model.nodes.kind(prototype) else {
            return None;
        };
        if p.optional || p.property.name != "prototype" || symbol(model, &p.object) != Some(base) {
            return None;
        }
        let method = model.nodes.parent_id(prototype);
        let AstKind::StaticMemberExpression(m) = model.nodes.kind(method) else {
            return None;
        };
        let assign = model.nodes.parent_id(method);
        let AstKind::AssignmentExpression(a) = model.nodes.kind(assign) else {
            return None;
        };
        if m.optional
            || m.object.span() != p.span
            || a.left.span() != m.span
            || a.operator.as_str() != "="
            || a.span.end >= argument.start
            || !same_body_statement(model, assign, block)
            || !matches!(
                m.property.name.as_str(),
                "clear" | "register" | "unregister" | "notify"
            )
        {
            return None;
        }
        let value = symbol(model, &a.right)?;
        let f = function(model, value)?;
        let id = model.scoping.symbol_declaration(value);
        if f.r#async
            || f.generator
            || !plain_params(model, &f.params)
            || !directives(f.body.as_ref()?)
            || !unchanged(model, value)
            || model.nodes.kind(model.nodes.parent_id(id)).span() != block
            || model.binding_references(value) != 1
            || methods.insert(m.property.name.as_str(), f).is_some()
        {
            return None;
        }
    }
    if factory_uses != 1 || methods.len() != 4 {
        return None;
    }
    let clear = methods.get("clear")?;
    if !clear.params.items.is_empty() {
        return None;
    }
    let [reset, ret] = clear.body.as_ref()?.statements.as_slice() else {
        return None;
    };
    callback_array_assignment(reset)?;
    if !returns_this(ret) {
        return None;
    }
    let register = methods.get("register")?;
    if register.params.items.len() != 1 {
        return None;
    }
    let param = binding(&register.params.items[0].pattern)?;
    let [push, ret] = register.body.as_ref()?.statements.as_slice() else {
        return None;
    };
    let push = statement_call(push)?;
    if !matches!(
        member(member(&push.callee, "push")?, "callbacks")?,
        Expression::ThisExpression(_)
    ) || symbol(model, arity(push, 1)?[0]) != Some(param)
        || !returns_this(ret)
    {
        return None;
    }
    registry_data_methods(model, methods.get("unregister")?, methods.get("notify")?)?;
    Some(())
}

/// Values whose identity matters to the source-local registry contract. Data
/// never includes the registry, its callback array, or a local callable. Keeping
/// these separate prevents a harmless-looking alias/call from publishing one.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RegistryValue {
    Data,
    Number,
    Callback,
    Receiver,
    Collection,
    Callable(NodeId, bool),
}
impl RegistryValue {
    fn data(self) -> bool {
        matches!(self, Self::Data | Self::Number | Self::Callback)
    }
}

/// Bounded effect inventory for the two data-facing methods. Constructor,
/// register and clear have exact straight-line proofs below. This inventory
/// permits numeric array reads/removals, opaque callback calls without a
/// registry/array receiver, and calls to the already proved clear API. It does
/// not infer safety from a method name or from an unexecuted declaration.
struct RegistryContract<'m, 'a> {
    model: &'m Model<'m, 'a>,
    active: BTreeSet<NodeId>,
    remaining: usize,
}
impl<'a> RegistryContract<'_, 'a> {
    fn callable(&self, span: Span, lexical_this: bool) -> Option<RegistryValue> {
        let site = self.model.all_sites().iter().find(|s| s.span == span)?;
        Some(RegistryValue::Callable(site.node_id, lexical_this))
    }
    fn variables(
        &mut self,
        declaration: &VariableDeclaration<'a>,
        env: &mut BTreeMap<Symbol, RegistryValue>,
        own_this: bool,
    ) -> Option<()> {
        for v in &declaration.declarations {
            let s = binding(&v.id)?;
            let value = self.expression(v.init.as_ref()?, env, own_this)?;
            // One executed producer, never an uninitialized or reassigned
            // receiver/collection/callable alias. SymbolIds keep scopes apart.
            if env.insert(s, value).is_some() {
                return None;
            }
        }
        Some(())
    }
    fn statements(
        &mut self,
        statements: &[Statement<'a>],
        env: &mut BTreeMap<Symbol, RegistryValue>,
        own_this: bool,
        return_receiver: bool,
    ) -> Option<()> {
        // Declarations are inert. Only an actual known call examines the body;
        // a publication of a callable is refused by the value-use contract.
        for s in statements {
            if let Statement::FunctionDeclaration(f) = s {
                let id = f.id.as_ref()?.symbol_id.get()?;
                if env.insert(id, self.callable(f.span, false)?).is_some() {
                    return None;
                }
            }
        }
        for s in statements {
            self.statement(s, env, own_this, return_receiver)?;
        }
        Some(())
    }
    fn statement(
        &mut self,
        s: &Statement<'a>,
        env: &mut BTreeMap<Symbol, RegistryValue>,
        own_this: bool,
        return_receiver: bool,
    ) -> Option<()> {
        match s {
            Statement::FunctionDeclaration(_) | Statement::EmptyStatement(_) => Some(()),
            Statement::VariableDeclaration(v) => self.variables(v, env, own_this),
            Statement::ExpressionStatement(e) => {
                let value = self.expression(&e.expression, env, own_this)?;
                (!matches!(
                    value,
                    RegistryValue::Collection | RegistryValue::Callable(_, _)
                ))
                .then_some(())
            }
            Statement::ReturnStatement(r) => {
                if let Some(e) = &r.argument {
                    let value = self.expression(e, env, own_this)?;
                    if !value.data() && !(return_receiver && value == RegistryValue::Receiver) {
                        return None;
                    }
                }
                Some(())
            }
            Statement::BlockStatement(b) => {
                self.statements(&b.body, &mut env.clone(), own_this, return_receiver)
            }
            Statement::IfStatement(i) => {
                if !self.expression(&i.test, env, own_this)?.data() {
                    return None;
                }
                self.statement(&i.consequent, &mut env.clone(), own_this, return_receiver)?;
                if let Some(other) = &i.alternate {
                    self.statement(other, &mut env.clone(), own_this, return_receiver)?;
                }
                Some(())
            }
            Statement::WhileStatement(w) => {
                if !self.expression(&w.test, env, own_this)?.data() {
                    return None;
                }
                self.statement(&w.body, &mut env.clone(), own_this, return_receiver)
            }
            Statement::ForStatement(f) => {
                let mut local = env.clone();
                if let Some(init) = &f.init {
                    let ForStatementInit::VariableDeclaration(v) = init else {
                        return None;
                    };
                    self.variables(v, &mut local, own_this)?;
                }
                if !self
                    .expression(f.test.as_ref()?, &mut local, own_this)?
                    .data()
                {
                    return None;
                }
                self.statement(&f.body, &mut local, own_this, return_receiver)?;
                self.expression(f.update.as_ref()?, &mut local, own_this)?;
                Some(())
            }
            Statement::TryStatement(t) if t.finalizer.is_none() => {
                self.statements(&t.block.body, &mut env.clone(), own_this, return_receiver)?;
                let catch = t.handler.as_ref()?;
                let mut local = env.clone();
                if let Some(param) = &catch.param {
                    local.insert(binding(&param.pattern)?, RegistryValue::Data);
                }
                self.statements(&catch.body.body, &mut local, own_this, return_receiver)
            }
            _ => None,
        }
    }
    fn invoke(
        &mut self,
        node: NodeId,
        lexical_this: bool,
        args: Vec<RegistryValue>,
        env: &BTreeMap<Symbol, RegistryValue>,
    ) -> Option<RegistryValue> {
        if !self.active.insert(node) {
            return None;
        }
        let (params, body, expression) = match self.model.nodes.kind(node) {
            AstKind::Function(f) if !f.r#async && !f.generator => {
                (&f.params, f.body.as_ref()?, false)
            }
            AstKind::ArrowFunctionExpression(f) if !f.r#async => (&f.params, &f.body, f.expression),
            _ => return None,
        };
        if !plain_params(self.model, params)
            || params.rest.is_some()
            || params.items.len() != args.len()
        {
            return None;
        }
        let mut local = env.clone();
        for (p, value) in params.items.iter().zip(args) {
            local.insert(binding(&p.pattern)?, value);
        }
        if expression {
            let [Statement::ExpressionStatement(e)] = body.statements.as_slice() else {
                return None;
            };
            // Expression helpers may perform a safe clear, but cannot return
            // its receiver as a new alias. Use a block for a void helper.
            if !self
                .expression(&e.expression, &mut local, lexical_this)?
                .data()
            {
                return None;
            }
        } else {
            self.statements(&body.statements, &mut local, lexical_this, false)?;
        }
        self.active.remove(&node);
        // Helpers have no protected/callable return. Their data result cannot
        // become a numeric index without a separate numeric source proof.
        Some(RegistryValue::Data)
    }
    fn expression(
        &mut self,
        e: &Expression<'a>,
        env: &mut BTreeMap<Symbol, RegistryValue>,
        own_this: bool,
    ) -> Option<RegistryValue> {
        self.remaining = self.remaining.checked_sub(1)?;
        use RegistryValue as V;
        match e {
            Expression::ThisExpression(_) => own_this.then_some(V::Receiver),
            Expression::Identifier(_) => env.get(&symbol(self.model, e)?).copied(),
            Expression::NumericLiteral(_) => Some(V::Number),
            Expression::NullLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::StringLiteral(_) => Some(V::Data),
            Expression::FunctionExpression(f) => self.callable(f.span, false),
            Expression::ArrowFunctionExpression(f) => self.callable(f.span, own_this),
            Expression::StaticMemberExpression(m) if !m.optional => {
                match (
                    self.expression(&m.object, env, own_this)?,
                    m.property.name.as_str(),
                ) {
                    (V::Receiver, "callbacks") => Some(V::Collection),
                    (V::Collection, "length") => Some(V::Number),
                    _ => None,
                }
            }
            Expression::ComputedMemberExpression(m) if !m.optional => {
                (self.expression(&m.object, env, own_this)? == V::Collection
                    && self.expression(&m.expression, env, own_this)? == V::Number)
                    .then_some(V::Callback)
            }
            Expression::UnaryExpression(u)
                if matches!(u.operator.as_str(), "void" | "!" | "typeof") =>
            {
                self.expression(&u.argument, env, own_this)?
                    .data()
                    .then_some(V::Data)
            }
            Expression::BinaryExpression(b) => {
                let l = self.expression(&b.left, env, own_this)?;
                let r = self.expression(&b.right, env, own_this)?;
                match b.operator.as_str() {
                    "==" | "!=" | "===" | "!==" if l.data() && r.data() => Some(V::Data),
                    "<" | ">" | "<=" | ">=" if l == V::Number && r == V::Number => Some(V::Data),
                    "+" | "-" | "*" | "/" | "%" if l == V::Number && r == V::Number => {
                        Some(V::Number)
                    }
                    _ => None,
                }
            }
            Expression::UpdateExpression(u) => {
                let SimpleAssignmentTarget::AssignmentTargetIdentifier(i) = &u.argument else {
                    return None;
                };
                let s = crate::analysis::identifier_symbol(i, self.model.scoping)?;
                (env.get(&s) == Some(&V::Number)).then_some(V::Number)
            }
            Expression::AssignmentExpression(a) if a.operator.as_str() == "=" => {
                let value = self.expression(&a.right, env, own_this)?;
                if let Some(s) = assign_symbol(self.model, a) {
                    let old = *env.get(&s)?;
                    // Preserve the type invariant across every possible path
                    // and loop iteration; protected aliases never get writes.
                    return (old.data() && value.data() && old == value).then_some(value);
                }
                let AssignmentTarget::ComputedMemberExpression(m) = &a.left else {
                    return None;
                };
                if m.optional
                    || self.expression(&m.object, env, own_this)? != V::Collection
                    || self.expression(&m.expression, env, own_this)? != V::Number
                    || !matches!(&a.right,Expression::UnaryExpression(u) if u.operator.as_str()=="void" && matches!(u.argument,Expression::NumericLiteral(_)))
                {
                    return None;
                }
                Some(V::Data)
            }
            Expression::CallExpression(c) if !c.optional => {
                // The established array-copy protocol is an ordinary runtime
                // port, just as register's push is. Admit only this exact
                // unshadowed borrowed-call shape, without passing the registry
                // or a callable/capture. Keep its result protected as an array
                // (not arbitrary publishable data). This does not certify the
                // ambient implementation's native origin or purity.
                if is_global(
                    self.model,
                    &c.callee,
                    &["Array", "prototype", "slice", "call"],
                ) {
                    if c.arguments.len() != 1 {
                        return None;
                    }
                    let argument = c.arguments[0].as_expression()?;
                    return (self.expression(argument, env, own_this)? == V::Collection)
                        .then_some(V::Collection);
                }

                if let Expression::StaticMemberExpression(m) = &c.callee {
                    if m.optional {
                        return None;
                    }
                    let receiver = self.expression(&m.object, env, own_this)?;
                    match (receiver, m.property.name.as_str()) {
                        (V::Receiver, "clear") if c.arguments.is_empty() => {
                            return Some(V::Receiver);
                        }
                        (V::Callback, "call") => {
                            let first = c.arguments.first()?.as_expression()?;
                            if !matches!(first,Expression::UnaryExpression(u) if u.operator.as_str()=="void" && matches!(u.argument,Expression::NumericLiteral(_)))
                            {
                                return None;
                            }
                            for a in &c.arguments {
                                if !self.expression(a.as_expression()?, env, own_this)?.data() {
                                    return None;
                                }
                            }
                            return Some(V::Data);
                        }
                        _ => return None,
                    }
                }
                let V::Callable(node, lexical_this) = self.expression(&c.callee, env, own_this)?
                else {
                    return None;
                };
                let mut args = Vec::new();
                for a in &c.arguments {
                    args.push(self.expression(a.as_expression()?, env, own_this)?);
                }
                self.invoke(node, lexical_this, args, env)
            }
            _ => None,
        }
    }
    fn method(&mut self, f: &Function<'a>, receiver_result: bool) -> Option<()> {
        if f.r#async
            || f.generator
            || !plain_params(self.model, &f.params)
            || f.params.rest.is_some()
            || !directives(f.body.as_ref()?)
        {
            return None;
        }
        let mut env = BTreeMap::new();
        for p in &f.params.items {
            env.insert(binding(&p.pattern)?, RegistryValue::Data);
        }
        self.statements(
            &f.body.as_ref()?.statements,
            &mut env,
            true,
            receiver_result,
        )
    }
}

fn registry_data_methods(
    model: &Model<'_, '_>,
    unregister: &Function<'_>,
    notify: &Function<'_>,
) -> Option<()> {
    if unregister.params.items.len() != 1 || notify.params.items.len() != 1 {
        return None;
    }
    let mut proof = RegistryContract {
        model,
        active: BTreeSet::new(),
        remaining: 10000,
    };
    proof.method(unregister, true)?;
    proof.method(notify, false)
}

fn class_registry_base(
    model: &Model<'_, '_>,
    base: Symbol,
    before: u32,
    argument: Span,
) -> Option<()> {
    let AstKind::Class(base_class) = model.nodes.kind(model.scoping.symbol_declaration(base))
    else {
        return None;
    };
    if !base_class.decorators.is_empty()
        || base_class.super_class.is_some()
        || base_class.body.body.len() != 5
        || base_class.span.end > before
        || property_write(model, base)
        || !unchanged(model, base)
    {
        return None;
    }
    let base_refs: Vec<_> = model.scoping.get_resolved_references(base).collect();
    if base_refs.len() != 1 || model.nodes.kind(base_refs[0].node_id()).span() != argument {
        return None;
    }
    let base_constructor = method(base_class, "constructor")?;
    if !base_constructor.params.items.is_empty() {
        return None;
    }
    let body = base_constructor.body.as_ref()?;
    let [init] = body.statements.as_slice() else {
        return None;
    };
    callback_array_assignment(init)?;
    let clear = method(base_class, "clear")?;
    let [reset, ret] = clear.body.as_ref()?.statements.as_slice() else {
        return None;
    };
    callback_array_assignment(reset)?;
    if !clear.params.items.is_empty() || !returns_this(ret) {
        return None;
    }
    let register = method(base_class, "register")?;
    if register.params.items.len() != 1 || !plain_params(model, &register.params) {
        return None;
    }
    let param = binding(&register.params.items[0].pattern)?;
    let [push, ret] = register.body.as_ref()?.statements.as_slice() else {
        return None;
    };
    let push = statement_call(push)?;
    if !matches!(
        member(member(&push.callee, "push")?, "callbacks")?,
        Expression::ThisExpression(_)
    ) || symbol(model, arity(push, 1)?[0]) != Some(param)
        || !returns_this(ret)
    {
        return None;
    }
    registry_data_methods(
        model,
        method(base_class, "unregister")?,
        method(base_class, "notify")?,
    )?;
    Some(())
}

/// A local data constructor, not a native-class claim. The constructor has no
/// call-out before its fresh callback array exists. Neither constructor nor
/// prototype is exported/replaced; only the three registry instances escape.
fn constructor(
    model: &Model<'_, '_>,
    store: &AssignmentExpression<'_>,
    block: Span,
    registries: &[Symbol],
    setter: Symbol,
) -> Option<()> {
    let Expression::NewExpression(new) = &store.right else {
        return None;
    };
    if !new.arguments.is_empty() {
        return None;
    }
    let ctor = symbol(model, &new.callee)?;
    if !unchanged(model, ctor) || property_write(model, ctor) {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(ctor);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return None;
    };
    if model
        .nodes
        .kind(model.nodes.parent_id(model.nodes.parent_id(declaration)))
        .span()
        != block
    {
        return None;
    }
    let create = call(v.init.as_ref()?)?;
    let args = arity(create, 1)?;
    if v.span.end > store.span.start {
        return None;
    }
    let base = symbol(model, args[0])?;
    if matches!(
        model.nodes.kind(model.scoping.symbol_declaration(base)),
        AstKind::Class(_)
    ) {
        class_registry_base(model, base, v.span.start, args[0].span())?;
    } else {
        prototype_registry_base(model, base, args[0].span(), block)?;
    }
    let factory = symbol(model, &create.callee)?;
    let f = function(model, factory)?;
    if f.r#async
        || f.generator
        || !unchanged(model, factory)
        || f.params.items.len() != 1
        || !plain_params(model, &f.params)
    {
        return None;
    }
    let p = binding(&f.params.items[0].pattern)?;
    let [
        Statement::ClassDeclaration(derived),
        Statement::ReturnStatement(ret),
    ] = f.body.as_ref()?.statements.as_slice()
    else {
        return None;
    };
    if !derived.decorators.is_empty()
        || symbol(model, derived.super_class.as_ref()?) != Some(p)
        || symbol(model, ret.argument.as_ref()?) != derived.id.as_ref()?.symbol_id.get()
    {
        return None;
    }
    let refs: Vec<_> = model.scoping.get_resolved_references(factory).collect();
    if refs.len() != 1 || model.nodes.kind(refs[0].node_id()).span() != create.callee.span() {
        return None;
    }
    let derived_id = derived.id.as_ref()?.symbol_id.get()?;
    if !unchanged(model, derived_id)
        || property_write(model, derived_id)
        || model.binding_references(derived_id) != 1
        || derived.body.body.len() != 2
    {
        return None;
    }
    let ctor_body = method(derived, "constructor")?;
    if !ctor_body.params.items.is_empty() {
        return None;
    }
    let [super_call] = ctor_body.body.as_ref()?.statements.as_slice() else {
        return None;
    };
    let super_call = statement_call(super_call)?;
    if !matches!(super_call.callee, Expression::Super(_)) || super_call.arguments.len() != 1 {
        return None;
    }
    let Argument::SpreadElement(spread) = &super_call.arguments[0] else {
        return None;
    };
    if !is_global(model, &spread.argument, &["arguments"]) {
        return None;
    }
    let register = method(derived, "register")?;
    if register.params.items.len() != 1 || !plain_params(model, &register.params) {
        return None;
    }
    let arg = binding(&register.params.items[0].pattern)?;
    let [setup, forward, ret] = register.body.as_ref()?.statements.as_slice() else {
        return None;
    };
    let setup = statement_call(setup)?;
    if symbol(model, &setup.callee) != Some(setter)
        || arity(setup, 0).is_none()
        || !returns_this(ret)
    {
        return None;
    }
    let forward = statement_call(forward)?;
    if !matches!(member(&forward.callee, "register")?, Expression::Super(_))
        || symbol(model, arity(forward, 1)?[0]) != Some(arg)
    {
        return None;
    }
    let refs: Vec<_> = model.scoping.get_resolved_references(ctor).collect();
    if refs.len() != registries.len() {
        return None;
    }
    let mut targets = BTreeSet::new();
    for r in refs {
        let n = model.nodes.parent_id(r.node_id());
        let AstKind::NewExpression(nv) = model.nodes.kind(n) else {
            return None;
        };
        if !nv.arguments.is_empty() {
            return None;
        }
        let a = model.nodes.parent_id(n);
        let AstKind::AssignmentExpression(ae) = model.nodes.kind(a) else {
            return None;
        };
        let target = assign_symbol(model, ae)?;
        if !same_body_statement(model, a, block)
            || !registries.contains(&target)
            || !targets.insert(target)
        {
            return None;
        }
    }
    Some(())
}

fn init_ready(model: &Model<'_, '_>, owner: FunctionSite, init: Symbol, c: &Cleanup) -> Option<()> {
    let f = function(model, init)?;
    let b = f.body.as_ref()?;
    let n = c.registries.len();
    if b.statements.len() != n + 2 {
        return None;
    }
    for (s, r) in b.statements[..n].iter().zip(&c.registries) {
        let call = statement_call(s)?;
        if symbol(model, member(&call.callee, "register")?) != Some(*r) || arity(call, 1).is_none()
        {
            return None;
        }
    }
    if !matches!(&b.statements[n], Statement::VariableDeclaration(_)) {
        return None;
    }
    let Statement::IfStatement(i) = &b.statements[n + 1] else {
        return None;
    };
    if i.alternate.is_some() {
        return None;
    }
    let call = statement_call(one(&i.consequent)?)?;
    let a = arity(call, 2)?;
    if symbol(model, member(&call.callee, "addEventListener")?) != Some(c.document)
        || !string(a[0], "click")
        || symbol(model, a[1]) != Some(c.click)
    {
        return None;
    }
    // The init binding itself is returned by the exact local factory proof.
    if !owner.span.contains_inclusive(f.span) {
        return None;
    }
    Some(())
}

fn setter(model: &Model<'_, '_>, c: &Cleanup) -> Option<Symbol> {
    // This bounded branch currently proves the retained submit protocol. A
    // navigation-shaped body still needs its own setter/exposure witness.
    if c.navigate.is_some() {
        return None;
    }
    let listener = c.submit?;
    if listener.navigation {
        return None;
    }
    let handler_writes = writes(model, listener.handler)?;
    let [handler_write] = handler_writes.as_slice() else {
        return None;
    };
    if model.initializer(listener.handler).is_some() {
        return None;
    }
    let handler_fn = symbol(model, &handler_write.right)?;
    let handler_fn = function(model, handler_fn)?;
    if handler_fn.params.items.len() != 1 || !plain_params(model, &handler_fn.params) {
        return None;
    }
    let a_id = node_for(model, handler_write.span)?;
    let statement = model.nodes.parent_id(a_id);
    let block = model.nodes.parent_id(statement);
    let AstKind::BlockStatement(try_body) = model.nodes.kind(block) else {
        return None;
    };
    let AstKind::TryStatement(t) = model.nodes.parent_kind(block) else {
        return None;
    };
    if t.finalizer.is_some() {
        return None;
    }
    let [Statement::FunctionDeclaration(f), assign, add] = try_body.body.as_slice() else {
        return None;
    };
    if f.span != handler_fn.span || assignment(assign)?.span != handler_write.span {
        return None;
    }
    let add = statement_call(add)?;
    let args = arity(add, 3)?;
    if symbol(model, member(&add.callee, "addEventListener")?) != Some(listener.receiver)
        || !string(args[0], "submit")
        || symbol(model, args[1]) != f.id.as_ref()?.symbol_id.get()
        || !matches!(args[2],Expression::BooleanLiteral(v) if v.value)
    {
        return None;
    }
    let catch = t.handler.as_ref()?;
    if !matches!(catch.body.body.last(), Some(Statement::ReturnStatement(_))) {
        return None;
    }
    let owner = model.enclosing_function(a_id)?;
    let AstKind::Function(f) = model.nodes.kind(owner.node_id) else {
        return None;
    };
    if f.r#async || f.generator || !f.params.items.is_empty() {
        return None;
    }
    let body = f.body.as_ref()?;
    let Statement::IfStatement(enabled) = body.statements.last()? else {
        return None;
    };
    if symbol(model, &enabled.test) != Some(c.flag)
        || !matches!(one(&enabled.consequent)?, Statement::ReturnStatement(_))
    {
        return None;
    }
    let Statement::BlockStatement(setup) = enabled.alternate.as_ref()? else {
        return None;
    };
    if !setup.span.contains_inclusive(t.span) {
        return None;
    }
    let [prefix @ .., set, Statement::ReturnStatement(_)] = setup.body.as_slice() else {
        return None;
    };
    if !matches!(prefix.first(),Some(Statement::TryStatement(first)) if first.span==t.span) {
        return None;
    }
    // Other installation hooks may call out/reenter, but may not write any
    // protected binding/method. All their flag/handler/registry writes are
    // checked independently below, including inside nested callbacks.
    if prefix[1..]
        .iter()
        .any(|s| !matches!(s, Statement::IfStatement(_)))
    {
        return None;
    }
    let set = assignment(set)?;
    if assign_symbol(model, set) != Some(c.flag)
        || !matches!(&set.right,Expression::BooleanLiteral(v) if v.value)
    {
        return None;
    }
    let f_id = owner.binding_symbol?;
    if !unchanged(model, f_id) {
        return None;
    }
    let refs: Vec<_> = model.scoping.get_resolved_references(f_id).collect();
    let [r] = refs.as_slice() else {
        return None;
    };
    let AstKind::VariableDeclarator(alias) = model.nodes.parent_kind(r.node_id()) else {
        return None;
    };
    let alias = binding(&alias.id)?;
    if !unchanged(model, alias) {
        return None;
    }
    // No direct call, callback argument, borrowed method or extra alias may
    // invoke the enabler. Both uses must be the proved registry overrides.
    if model.binding_references(alias) != 2 {
        return None;
    }
    for r in model.scoping.get_resolved_references(alias) {
        let AstKind::CallExpression(call) = model.nodes.parent_kind(r.node_id()) else {
            return None;
        };
        if call.callee.span() != model.nodes.kind(r.node_id()).span() || arity(call, 0).is_none() {
            return None;
        }
        let method = model.enclosing_function(r.node_id())?;
        let AstKind::MethodDefinition(m) = model.nodes.parent_kind(method.node_id) else {
            return None;
        };
        if property_key_name(&m.key)?.as_str() != "register" {
            return None;
        }
    }
    Some(alias)
}

// A source-local lower bound on execution, not a textual declaration order.
// Store points are direct statements in their block; later nested statements
// are dominated by the store only within that same activation and block.
#[derive(Clone)]
struct ReadyAfter {
    owner: NodeId,
    points: Vec<(Span, u32)>,
}
struct ReaderEntries<'m, 's, 'a> {
    model: &'m Model<'s, 'a>,
    registration_entries: BTreeMap<NodeId, Vec<NodeId>>,
    active: BTreeSet<(NodeId, NodeId, Vec<(u32, u32, u32)>)>,
    active_values: BTreeSet<(Symbol, NodeId, Vec<(u32, u32, u32)>)>,
    checked: BTreeSet<(NodeId, NodeId, Vec<(u32, u32, u32)>)>,
    checked_values: BTreeSet<(Symbol, NodeId, Vec<(u32, u32, u32)>)>,
    remaining: usize,
}
impl<'m, 's, 'a> ReaderEntries<'m, 's, 'a> {
    fn key(requirement: &ReadyAfter) -> Vec<(u32, u32, u32)> {
        requirement
            .points
            .iter()
            .map(|(b, end)| (b.start, b.end, *end))
            .collect()
    }
    fn dominates(&self, node: NodeId, requirement: &ReadyAfter) -> bool {
        requirement.points.iter().any(|(block, end)| {
            if self.model.nodes.kind(node).span().start < *end {
                return false;
            }
            let mut at = node;
            for _ in 0..512 {
                if self.model.nodes.kind(at).span() == *block {
                    return true;
                }
                let next = self.model.nodes.parent_id(at);
                if next == at {
                    return false;
                }
                at = next;
            }
            false
        })
    }
    fn after_use(&mut self, node: NodeId, requirement: &ReadyAfter) -> Option<()> {
        self.remaining = self.remaining.checked_sub(1)?;
        let owner = self.model.owning_site(node)?;
        if owner.node_id == requirement.owner {
            return self.dominates(node, requirement).then_some(());
        }
        self.entries(owner, requirement)
    }
    fn store_point(&self, node: NodeId, end: u32) -> Option<ReadyAfter> {
        let mut statement = node;
        for _ in 0..4 {
            let parent = self.model.nodes.parent_id(statement);
            if matches!(
                self.model.nodes.kind(parent),
                AstKind::FunctionBody(_) | AstKind::BlockStatement(_)
            ) {
                return Some(ReadyAfter {
                    owner: self.model.owning_site(node)?.node_id,
                    points: vec![(self.model.nodes.kind(parent).span(), end)],
                });
            }
            if !matches!(
                self.model.nodes.kind(parent),
                AstKind::VariableDeclaration(_) | AstKind::ExpressionStatement(_)
            ) {
                return None;
            }
            statement = parent;
        }
        None
    }
    fn producer(&self, id: Symbol) -> Option<ReadyAfter> {
        if !self.model.scoping.symbol_redeclarations(id).is_empty() {
            return None;
        }
        let declaration = self.model.scoping.symbol_declaration(id);
        let AstKind::VariableDeclarator(v) = self.model.nodes.kind(declaration) else {
            return None;
        };
        let stores = writes(self.model, id)?;
        if v.init.is_some() {
            if !stores.is_empty() {
                return None;
            }
            return self.store_point(declaration, v.span.end);
        }
        let [store] = stores.as_slice() else {
            return None;
        };
        let node = node_for(self.model, store.span)?;
        // A lexical declaration must have executed even before a later store.
        // A var declaration creates a slot, never a ready callable value.
        if !matches!(self.model.nodes.parent_kind(declaration), AstKind::VariableDeclaration(d) if d.kind == VariableDeclarationKind::Var)
        {
            let declared = self.store_point(declaration, v.span.end)?;
            if self.model.owning_site(node)?.node_id != declared.owner
                || !self.dominates(node, &declared)
            {
                return None;
            }
        }
        self.store_point(node, store.span.end)
    }
    fn aliases(&mut self, id: Symbol, requirement: &ReadyAfter) -> Option<()> {
        let key = (id, requirement.owner, Self::key(requirement));
        if self.active_values.contains(&key) || self.checked_values.contains(&key) {
            return Some(());
        }
        let refs: Vec<_> = self
            .model
            .scoping
            .get_resolved_references(id)
            .filter(|r| r.is_read())
            .map(|r| r.node_id())
            .collect();
        if refs.is_empty() {
            return Some(());
        }
        let producer = self.producer(id)?;
        self.active_values.insert(key.clone());
        let result = (|| {
            for node in refs {
                // This includes aliases copied in a nested helper: inspect all
                // actual entries of that helper before assuming its source
                // position follows the producer. Copying a slot is a read.
                self.after_use(node, &producer)?;
                self.value_use(node, requirement)?;
            }
            Some(())
        })();
        self.active_values.remove(&key);
        if result.is_some() {
            self.checked_values.insert(key);
        }
        result
    }
    fn assigned_value(&mut self, node: NodeId, id: Symbol, requirement: &ReadyAfter) -> Option<()> {
        let declaration = self.model.scoping.symbol_declaration(id);
        if self.model.owning_site(declaration)?.node_id != self.model.owning_site(node)?.node_id {
            // An outer captured slot is a retention/publication boundary.
            // Requiring its write after readiness protects every future use,
            // including externally invoked callbacks and reentry.
            return self.after_use(node, requirement);
        }
        self.aliases(id, requirement)
    }
    fn value_use(&mut self, node: NodeId, requirement: &ReadyAfter) -> Option<()> {
        self.remaining = self.remaining.checked_sub(1)?;
        let span = self.model.nodes.kind(node).span();
        let parent = self.model.nodes.parent_id(node);
        match self.model.nodes.kind(parent) {
            AstKind::VariableDeclarator(v) if v.init.as_ref().is_some_and(|e| e.span() == span) => {
                self.aliases(binding(&v.id)?, requirement)
            }
            AstKind::AssignmentExpression(a)
                if a.operator.as_str() == "=" && a.right.span() == span =>
            {
                if let Some(id) = assign_symbol(self.model, a) {
                    self.assigned_value(parent, id, requirement)
                } else {
                    self.after_use(parent, requirement)
                }
            }
            AstKind::CallExpression(c) if !c.optional => {
                // A callee is an actual entry. A callback argument is a
                // possible immediate entry AND retention boundary, regardless
                // of whether an opaque consumer usually defers callbacks.
                self.after_use(parent, requirement)
            }
            AstKind::ObjectProperty(p) if !p.computed && p.value.span() == span => {
                self.after_use(parent, requirement)
            }
            AstKind::ArrayExpression(_) | AstKind::ReturnStatement(_) => {
                self.after_use(parent, requirement)
            }
            // Mere identity tests do not enter a function. Alias readiness is
            // checked separately; inspecting a callable is not a publication.
            AstKind::UnaryExpression(u) if u.operator.as_str() == "typeof" => Some(()),
            AstKind::BinaryExpression(b)
                if matches!(b.operator.as_str(), "==" | "!=" | "===" | "!==")
                    && (matches!(b.left, Expression::NullLiteral(_))
                        || matches!(b.right, Expression::NullLiteral(_))) =>
            {
                Some(())
            }
            // Borrowing call/apply/bind/prototype or reflecting on a callable
            // is outside this closed source contract, not a safe direct call.
            _ => None,
        }
    }
    fn entries(&mut self, site: FunctionSite, requirement: &ReadyAfter) -> Option<()> {
        let key = (site.node_id, requirement.owner, Self::key(requirement));
        if self.active.contains(&key) || self.checked.contains(&key) {
            return Some(());
        }
        self.active.insert(key.clone());
        let result = (|| {
            if let Some(calls) = self.registration_entries.get(&site.node_id).cloned() {
                // These exact overrides are independently proved by setter +
                // constructor, and every instance register read by init_ready
                // + factory_ready + exported. Class creation is not an entry
                // into register; construction has no call to the enabler.
                for call in calls {
                    self.after_use(call, requirement)?;
                }
                return Some(());
            }
            let parent = self.model.nodes.parent_id(site.node_id);
            if matches!(
                self.model.nodes.kind(parent),
                AstKind::BlockStatement(_) | AstKind::FunctionBody(_)
            ) {
                let id = site.binding_symbol?;
                if !unchanged(self.model, id) {
                    return None;
                }
                let refs: Vec<_> = self
                    .model
                    .scoping
                    .get_resolved_references(id)
                    .map(|r| r.node_id())
                    .collect();
                // No reference is an inert declaration, not an executed read.
                // A recursive component is safe only if all of its external
                // call/publication edges also pass this same lower bound.
                let available = ReadyAfter {
                    owner: self.model.owning_site(site.node_id)?.node_id,
                    points: vec![(
                        self.model.nodes.kind(parent).span(),
                        self.model.nodes.kind(parent).span().start,
                    )],
                };
                for node in refs {
                    // Function declarations are hoisted within the executing
                    // lexical block. A reference outside that activation is
                    // not a proved producer merely because it appears later.
                    self.after_use(node, &available)?;
                    self.value_use(node, requirement)?;
                }
                Some(())
            } else {
                self.value_use(site.node_id, requirement)
            }
        })();
        self.active.remove(&key);
        if result.is_some() {
            self.checked.insert(key);
        }
        result
    }
}

fn known_readers_ready(
    model: &Model<'_, '_>,
    c: &Cleanup,
    epochs: Epoch,
    enabler: Symbol,
) -> Option<()> {
    let activation = model.owning_site(epochs.scope)?.node_id;
    let mut calls = Vec::new();
    let mut unsupported_results = Vec::new();
    for registry in &c.registries {
        for r in model
            .scoping
            .get_resolved_references(*registry)
            .filter(|r| r.is_read())
        {
            let member_id = model.nodes.parent_id(r.node_id());
            let AstKind::StaticMemberExpression(member) = model.nodes.kind(member_id) else {
                return None;
            };
            let call_id = model.nodes.parent_id(member_id);
            if member.property.name == "register" {
                calls.push(call_id);
            }
            if member.property.name == "clear"
                && !matches!(
                    model.nodes.parent_kind(call_id),
                    AstKind::ExpressionStatement(_)
                )
            {
                // clear returns this. An executed/retained consumer may not
                // leak a new registry alias; an inert declaration does not
                // execute this expression and has no such effect.
                unsupported_results.push(r.node_id());
            }
        }
    }
    let mut registration_entries = BTreeMap::new();
    for r in model.scoping.get_resolved_references(enabler) {
        let method = model.owning_site(r.node_id())?;
        if !matches!(model.nodes.parent_kind(method.node_id), AstKind::MethodDefinition(m) if property_key_name(&m.key).is_some_and(|k|k.as_str()=="register"))
        {
            return None;
        }
        registration_entries.insert(method.node_id, calls.clone());
    }
    let mut closure = ReaderEntries {
        model,
        registration_entries,
        active: BTreeSet::new(),
        active_values: BTreeSet::new(),
        checked: BTreeSet::new(),
        checked_values: BTreeSet::new(),
        remaining: 50000,
    };
    let never_entered = ReadyAfter {
        owner: activation,
        points: Vec::new(),
    };
    for node in unsupported_results {
        closure.after_use(node, &never_entered)?;
    }
    let mut protected = c.registries.clone();
    protected.push(c.flag);
    protected.push(c.submit?.receiver);
    for id in protected {
        let mut points = Vec::new();
        for block in epochs.sides {
            let end = if c.registries.contains(&id) {
                epoch_store(model, id, block)?.span.end
            } else if id == c.flag {
                writes(model, id)?
                    .into_iter()
                    .find(|a| {
                        node_for(model, a.span)
                            .is_some_and(|n| same_body_statement(model, n, block))
                    })?
                    .span
                    .end
            } else {
                epoch_document(model, id, block)?.span.end
            };
            points.push((block, end));
        }
        let requirement = ReadyAfter {
            owner: activation,
            points,
        };
        for r in model
            .scoping
            .get_resolved_references(id)
            .filter(|r| r.is_read())
        {
            closure.after_use(r.node_id(), &requirement)?;
        }
    }
    // The retained submit slot is intentionally nullable before enable. Only
    // the complete cleanup's flag + null guard has a proved conditional read.
    // Other local readers must enter after the actual retained-handler store,
    // or remain uninvoked/unpublished. A returning install catch excludes the
    // post-try continuation if the listener installation throws.
    let retained = c.submit?.handler;
    let handler_writes = writes(model, retained)?;
    let [store] = handler_writes.as_slice() else {
        return None;
    };
    let write_node = node_for(model, store.span)?;
    let try_body = model.nodes.parent_id(model.nodes.parent_id(write_node));
    let try_node = model.nodes.parent_id(try_body);
    let AstKind::TryStatement(t) = model.nodes.kind(try_node) else {
        return None;
    };
    let parent = model.nodes.parent_id(try_node);
    if !matches!(model.nodes.kind(parent), AstKind::BlockStatement(_))
        || t.finalizer.is_some()
        || !matches!(
            t.handler.as_ref()?.body.body.last(),
            Some(Statement::ReturnStatement(_))
        )
    {
        return None;
    }
    let retained_ready = ReadyAfter {
        owner: model.owning_site(write_node)?.node_id,
        points: vec![
            (model.nodes.kind(try_body).span(), store.span.end),
            (model.nodes.kind(parent).span(), t.span.end),
        ],
    };
    for r in model
        .scoping
        .get_resolved_references(retained)
        .filter(|r| r.is_read())
    {
        let owner = model.owning_site(r.node_id())?;
        if let Some(sc) = shape(model, owner) {
            if sc.flag == c.flag
                && sc.registries == c.registries
                && sc.submit.is_some_and(|s| {
                    s.handler == retained && s.receiver == c.submit.unwrap().receiver
                })
            {
                let (factory, init) = factory_ready(model, owner, &sc)?;
                init_ready(model, factory, init, &sc)?;
                if epoch(model, factory)?.1.identity != epochs.identity {
                    return None;
                }
                continue;
            }
        }
        closure.after_use(r.node_id(), &retained_ready)?;
    }
    Some(())
}

fn proof(model: &Model<'_, '_>, site: FunctionSite, c: &Cleanup) -> Option<()> {
    let (owner, init) = factory_ready(model, site, c)?;
    init_ready(model, owner, init, c)?;
    let (block, branches) = epoch(model, owner)?;
    // Direct eval/with would invalidate the declaration/reference inventory.
    let scope = model.nodes.parent_id(
        model
            .nodes
            .parent_id(model.scoping.symbol_declaration(c.flag)),
    );
    if model.initializer(c.flag).is_some()
        || !matches!(model.nodes.kind(scope), AstKind::FunctionBody(_))
        || (branches.continuation_after.is_some() && scope != branches.scope)
    {
        return None;
    }
    if model.nodes.iter().any(|n| model.nodes.kind(scope).span().contains_inclusive(n.kind().span()) &&
    (matches!(n.kind(),AstKind::WithStatement(_)) || matches!(n.kind(),AstKind::CallExpression(call) if is_global(model,&call.callee,&["eval"])))) { return None; }

    let enabler = setter(model, c)?;
    let flag_writes = writes(model, c.flag)?;
    let false_store = flag_writes.into_iter().find(|a| {
        node_for(model, a.span).is_some_and(|id| same_body_statement(model, id, block))
    })?;
    if !matches!(&false_store.right,Expression::BooleanLiteral(v) if !v.value) {
        return None;
    }
    let mut last = false_store.span.end;
    for registry in &c.registries {
        let store = epoch_store(model, *registry, block)?;
        if store.span.start <= last {
            return None;
        }
        constructor(model, store, block, &c.registries, enabler)?;
        last = store.span.end;
    }
    exported(model, owner, block, last)?;
    let submit = c.submit?;
    let doc_store = epoch_document(model, submit.receiver, block)?;
    if doc_store.span.end >= false_store.span.start
        || member(&doc_store.right, "document").is_none()
        || property_write(model, submit.receiver)
    {
        return None;
    }
    // Each flag write is either this activation's exclusive-arm reset, the
    // unique retained-handler enabler, or another complete sibling cleanup.
    let mut true_count = 0;
    let mut reset_blocks = BTreeSet::new();
    for a in writes(model, c.flag)? {
        match &a.right {
            Expression::BooleanLiteral(v) if v.value => {
                true_count += 1;
                let owner = model.enclosing_function(node_for(model, a.span)?)?;
                let refs: Vec<_> = model
                    .scoping
                    .get_resolved_references(owner.binding_symbol?)
                    .collect();
                if refs.len() != 1 {
                    return None;
                }
                let AstKind::VariableDeclarator(v) = model.nodes.parent_kind(refs[0].node_id())
                else {
                    return None;
                };
                if binding(&v.id) != Some(enabler) {
                    return None;
                }
            }
            Expression::BooleanLiteral(v) if !v.value => {
                let n = node_for(model, a.span)?;
                let parent = model.nodes.parent_id(model.nodes.parent_id(n));
                let span = model.nodes.kind(parent).span();
                if same_body_statement(model, n, span) && branches.contains(span, a.span.start) {
                    reset_blocks.insert(span.start);
                    continue;
                }
                let sibling = model.enclosing_function(n)?;
                let sc = shape(model, sibling)?;
                if sc.flag != c.flag
                    || sc.registries != c.registries
                    || sc.submit?.receiver != submit.receiver
                    || sc.submit?.handler != submit.handler
                {
                    return None;
                }
                let (factory, i) = factory_ready(model, sibling, &sc)?;
                init_ready(model, factory, i, &sc)?;
            }
            _ => return None,
        }
    }
    if true_count != 1 || reset_blocks.len() != 2 {
        return None;
    }
    // Every register read is an immediate method call in a locally proved
    // returned initializer, whose factory export follows its arm's stores.
    // Whole-registry/method aliases, mutations and extra writers are refused.
    for registry in &c.registries {
        for r in model.scoping.get_resolved_references(*registry) {
            if r.is_write() {
                continue;
            }
            let member_id = model.nodes.parent_id(r.node_id());
            let AstKind::StaticMemberExpression(m) = model.nodes.kind(member_id) else {
                return None;
            };
            if m.optional || m.object.span() != model.nodes.kind(r.node_id()).span() {
                return None;
            }
            let AstKind::CallExpression(call) = model.nodes.parent_kind(member_id) else {
                return None;
            };
            if call.callee.span() != m.span || call.optional {
                return None;
            }
            match m.property.name.as_str() {
                "register" => {
                    let i = model.enclosing_function(r.node_id())?;
                    let factory = enclosing(model, i.node_id)?;
                    let candidate = model.all_sites().iter().copied().find(|s| {
                        enclosing(model, s.node_id).is_some_and(|o| o.node_id == factory.node_id)
                            && shape(model, *s).is_some()
                    })?;
                    let sc = shape(model, candidate)?;
                    let (f, init) = factory_ready(model, candidate, &sc)?;
                    if i.binding_symbol != Some(init) || sc.registries != c.registries {
                        return None;
                    }
                    init_ready(model, f, init, &sc)?;
                    let (b, branch) = epoch(model, f)?;
                    if branch.identity != branches.identity {
                        return None;
                    }
                    let reset = writes(model, c.flag)?.into_iter().find(|a| {
                        node_for(model, a.span).is_some_and(|id| same_body_statement(model, id, b))
                    })?;
                    if !matches!(&reset.right,Expression::BooleanLiteral(v) if !v.value) {
                        return None;
                    }
                    let receiver = epoch_document(model, sc.submit?.receiver, b)?;
                    if receiver.span.end >= reset.span.start {
                        return None;
                    }
                    let mut end = reset.span.end;

                    for reg in &sc.registries {
                        let st = epoch_store(model, *reg, b)?;
                        if st.span.start <= end {
                            return None;
                        }
                        constructor(model, st, b, &sc.registries, enabler)?;
                        end = end.max(st.span.end);
                    }
                    exported(model, f, b, end)?;
                }
                "clear" | "notify" => {}
                _ => return None,
            }
        }
    }
    known_readers_ready(model, c, branches, enabler)?;
    Some(())
}

pub(super) fn accepts(model: &Model<'_, '_>, site: FunctionSite) -> bool {
    shape(model, site).is_some_and(|c| proof(model, site, &c).is_some())
}

#[cfg(test)]
pub(super) fn has_guarded_shape(model: &Model<'_, '_>, site: FunctionSite) -> bool {
    shape(model, site).is_some()
}

#[cfg(test)]
pub(super) fn diagnostic(model: &Model<'_, '_>, site: FunctionSite) -> String {
    let Some(c) = shape(model, site) else {
        return "shape".into();
    };
    let Some((owner, init)) = factory_ready(model, site, &c) else {
        return "factory publication".into();
    };
    if init_ready(model, owner, init, &c).is_none() {
        return "initializer".into();
    }
    let Some((block, _)) = epoch(model, owner) else {
        return "exclusive epoch".into();
    };
    let Some(enabler) = setter(model, &c) else {
        return "retained handler/enabler".into();
    };
    for r in &c.registries {
        let Some(store) = epoch_store(model, *r, block) else {
            return format!("registry store {r:?}");
        };
        if constructor(model, store, block, &c.registries, enabler).is_none() {
            return format!("constructor {r:?}");
        }
    }
    let (_, branches) = epoch(model, owner).unwrap();
    if known_readers_ready(model, &c, branches, enabler).is_none() {
        return "known local reader readiness".into();
    }
    if proof(model, site, &c).is_none() {
        return "final epoch/flag/all-reference proof".into();
    }
    "accepted".into()
}
