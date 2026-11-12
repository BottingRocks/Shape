//! A bounded publication exception for the direct no-event notifier call.
//! This proves the complete local producer and branch wiring. External notify
//! receivers remain opaque lexical data; no browser/native provenance is inferred.
use super::*;

fn decl<'a>(s: &'a Statement<'a>) -> Option<(Symbol, Option<&'a Expression<'a>>)> {
    let Statement::VariableDeclaration(d) = s else {
        return None;
    };
    let [v] = d.declarations.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(id) = &v.id else {
        return None;
    };
    Some((id.symbol_id.get()?, v.init.as_ref()))
}
fn set(model: &Model<'_, '_>, s: &Statement<'_>, expected: Symbol) -> Option<Symbol> {
    let a = assignment(s)?;
    (assign_symbol(model, a) == Some(expected))
        .then(|| symbol(model, &a.right))
        .flatten()
}
fn reset(model: &Model<'_, '_>, s: &Statement<'_>) -> Option<Symbol> {
    let a = assignment(s)?;
    matches!(&a.right, Expression::NullLiteral(_))
        .then(|| assign_symbol(model, a))
        .flatten()
}
fn neg<'a>(e: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    let Expression::UnaryExpression(u) = e else {
        return None;
    };
    (u.operator.as_str() == "!").then_some(&u.argument)
}
fn equals_member(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    s: Symbol,
    key: &str,
    op: &str,
    value: &str,
) -> bool {
    binary(e, op).is_some_and(|(l, r)| member_symbol(model, l, key, s) && string(r, value))
}
fn form_predicate(model: &Model<'_, '_>, e: &Expression<'_>, target: Symbol) -> bool {
    let Some((left, name)) = logical(e, "&&") else {
        return false;
    };
    let Some((nonnull, node)) = logical(left, "&&") else {
        return false;
    };
    null_check(model, nonnull, target)
        && binary(node, "===")
            .is_some_and(|(l, r)| member_symbol(model, l, "nodeType", target) && number(r, 1.0))
        && equals_member(model, name, target, "nodeName", "===", "FORM")
}
fn listener(
    model: &Model<'_, '_>,
    s: &Statement<'_>,
    method: &str,
    receiver: Symbol,
    slot: Symbol,
) -> bool {
    let Some(c) = statement_call(s) else {
        return false;
    };
    let Some(a) = arity(c, 2) else { return false };
    member_symbol(model, &c.callee, method, receiver)
        && string(a[0], "submit")
        && symbol(model, a[1]) == Some(slot)
}
fn function<'a>(model: &Model<'_, 'a>, s: &'a Statement<'a>) -> Option<(FunctionSite, Symbol)> {
    let Statement::FunctionDeclaration(f) = s else {
        return None;
    };
    Some((model.site_by_span(f.span)?, f.id.as_ref()?.symbol_id.get()?))
}

// Exact selector: originalTarget event -> original form, otherwise walk element
// parents until the next parent ceases to be a FORM (or points to the form).
// No getter/prototype/callable alias can stand in for this producer.
fn receiver_producer(
    model: &Model<'_, '_>,
    declaration: &Statement<'_>,
    branch: &Statement<'_>,
    record: Symbol,
    target: Symbol,
) -> Option<Symbol> {
    let (selected, init) = decl(declaration)?;
    if init.is_some() {
        return None;
    }
    let Statement::IfStatement(g) = branch else {
        return None;
    };
    let (a, original) = logical(&g.test, "&&")?;
    let (nonnull, untrusted) = logical(a, "&&")?;
    let (event, null) = binary(nonnull, "!=")?;
    let (key, event_in) = binary(original, "in")?;
    if !member_symbol(model, event, "event", record)
        || !matches!(null, Expression::NullLiteral(_))
        || !member(neg(untrusted)?, "isTrusted")
            .is_some_and(|e| member_symbol(model, e, "event", record))
        || !string(key, "originalTarget")
        || !member_symbol(model, event_in, "event", record)
    {
        return None;
    }
    let [direct] = block(&g.consequent)? else {
        return None;
    };
    if set(model, direct, selected) != Some(target) {
        return None;
    }
    let [cursor, flag, Statement::ForStatement(walk), finish] = block(g.alternate.as_ref()?)?
    else {
        return None;
    };
    let (cursor, cursor_init) = decl(cursor)?;
    let (flag, flag_init) = decl(flag)?;
    if symbol(model, cursor_init?) != Some(target)
        || flag_init.is_some_and(|e| !matches!(e, Expression::NumericLiteral(_)))
        || walk.init.is_some()
        || walk.test.is_some()
        || walk.update.is_some()
    {
        return None;
    }
    let [Statement::IfStatement(parent), Statement::IfStatement(stop), advance] =
        block(&walk.body)?
    else {
        return None;
    };
    let (p, null) = binary(&parent.test, "!=")?;
    if !member_symbol(model, p, "parentNode", cursor) || !matches!(null, Expression::NullLiteral(_))
    {
        return None;
    }
    let [read, test] = block(&parent.consequent)? else {
        return None;
    };
    let read = assignment(read)?;
    if assign_symbol(model, read) != Some(flag)
        || !member_symbol(model, &read.right, "parentNode", cursor)
    {
        return None;
    }
    let test = assignment(test)?;
    let (first, name) = logical(&test.right, "||")?;
    let (nulltest, node) = logical(first, "||")?;
    let (f, null) = binary(nulltest, "==")?;
    if assign_symbol(model, test) != Some(flag)
        || symbol(model, f) != Some(flag)
        || !matches!(null, Expression::NullLiteral(_))
        || !binary(node, "!==")
            .is_some_and(|(l, r)| member_symbol(model, l, "nodeType", flag) && number(r, 1.0))
        || !equals_member(model, name, flag, "nodeName", "!==", "FORM")
    {
        return None;
    }
    let [no_parent] = block(parent.alternate.as_ref()?)? else {
        return None;
    };
    let no_parent = assignment(no_parent)?;
    if assign_symbol(model, no_parent) != Some(flag)
        || !(matches!(&no_parent.right,Expression::BooleanLiteral(b) if !b.value)
            || binary(&no_parent.right, "!=").is_some_and(|(l, r)| {
                member_symbol(model, l, "parentNode", cursor)
                    && matches!(r, Expression::NullLiteral(_))
            }))
    {
        return None;
    }
    let (no_form, cycle) = logical(&stop.test, "||")?;
    let (p, t) = binary(cycle, "===")?;
    if symbol(model, neg(no_form)?) != Some(flag)
        || !member_symbol(model, p, "parentNode", cursor)
        || symbol(model, t) != Some(target)
        || stop.alternate.is_some()
        || !matches!(&stop.consequent,Statement::BreakStatement(b) if b.label.is_none())
    {
        return None;
    }
    let advance = assignment(advance)?;
    if assign_symbol(model, advance) != Some(cursor)
        || !member_symbol(model, &advance.right, "parentNode", cursor)
        || set(model, finish, selected) != Some(cursor)
        || model.binding_writes(selected) != 2
        || model.binding_writes(cursor) != 1
        || model.binding_writes(flag) != 3
    {
        return None;
    }
    Some(selected)
}

fn guard(model: &Model<'_, '_>, g: &IfStatement<'_>, event: Symbol, target: Symbol) -> bool {
    let Some((ty, targ)) = logical(&g.test, "||") else {
        return false;
    };
    equals_member(model, ty, event, "type", "!==", "submit")
        && binary(targ, "!==").is_some_and(|(l, r)| {
            member_symbol(model, l, "target", event) && symbol(model, r) == Some(target)
        })
        && one_void(&g.consequent)
}
fn forward_body(
    model: &Model<'_, '_>,
    site: FunctionSite,
    target: Symbol,
    notifier: Symbol,
) -> Option<Symbol> {
    let (event, body) = parts(model, site)?;
    let [Statement::IfStatement(g)] = body.statements.as_slice() else {
        return None;
    };
    if !guard(model, g, event, target) {
        return None;
    }
    let [forward, clear, ret] = block(g.alternate.as_ref()?)? else {
        return None;
    };
    let c = statement_call(forward)?;
    if symbol(model, &c.callee) != Some(notifier)
        || symbol(model, arity(c, 1)?[0]) != Some(event)
        || !returns_void(ret)
    {
        return None;
    }
    reset(model, clear)
}

pub(super) struct Wiring {
    pub notifier: FunctionSite,
    pub trusted: FunctionSite,
    pub target: Symbol,
    pub receiver: Symbol,
    pub alias: Symbol,
    pub forwards: [Symbol; 2],
}

// The exception requires a *complete* parent block. An extra statement, moved
// publication, early call, swapped capture or changed return invalidates it.
pub(super) fn wiring(model: &Model<'_, '_>, site: FunctionSite, alias: Symbol) -> Option<Wiring> {
    let AstKind::BlockStatement(b) = model.nodes.parent_kind(site.node_id) else {
        return None;
    };
    let (prefix, tail) = b.body.split_at(b.body.len().checked_sub(4)?);
    let [alias_decl, notify, publish, Statement::IfStatement(event_branch)] = tail else {
        return None;
    };
    let (notifier, notifier_symbol) = function(model, notify)?;
    if notifier.node_id != site.node_id
        || !matches!(decl(alias_decl)?, (s, None) if s == alias)
        || set(model, publish, alias) != Some(notifier_symbol)
    {
        return None;
    }
    // The block must be the form guard in the immediate one-record handler.
    let block_id = model.nodes.parent_id(site.node_id);
    let AstKind::IfStatement(form_guard) = model.nodes.parent_kind(block_id) else {
        return None;
    };
    if form_guard.consequent.span() != b.span || !one_void(form_guard.alternate.as_ref()?) {
        return None;
    }
    let parent_if = model.nodes.parent_id(block_id);
    let AstKind::FunctionBody(parent_body) = model.nodes.parent_kind(parent_if) else {
        return None;
    };
    let parent_body_id = model.nodes.parent_id(parent_if);
    let AstKind::Function(parent_fn) = model.nodes.parent_kind(parent_body_id) else {
        return None;
    };
    let (record, _) = parts(model, model.site_by_span(parent_fn.span)?)?;
    let [target_decl, Statement::IfStatement(nullguard), last] = parent_body.statements.as_slice()
    else {
        return None;
    };
    let (target, init) = decl(target_decl)?;
    if !member_symbol(model, init?, "target", record)
        || !unchanged(model, target)
        || last.span() != form_guard.span
        || !form_predicate(model, &form_guard.test, target)
        || nullguard.alternate.is_some()
        || !one_void(&nullguard.consequent)
        || !binary(&nullguard.test, "==").is_some_and(|(l, r)| {
            symbol(model, l) == Some(target) && matches!(r, Expression::NullLiteral(_))
        })
    {
        return None;
    }
    let receiver = match prefix {
        [selected, selector] => inline_receiver(model, selected, selector, record, target)?,
        [selected, selector, receiver] => {
            let selected = receiver_producer(model, selected, selector, record, target)?;
            let (receiver, init) = decl(receiver)?;
            if symbol(model, init?) != Some(selected) || !unchanged(model, receiver) {
                return None;
            }
            receiver
        }
        [flags, predicate, selected, selector] => {
            stateful_receiver(model, flags, predicate, selected, selector, record, target)?
        }
        _ => return None,
    };
    let (e, null) = binary(&event_branch.test, "==")?;
    if !member_symbol(model, e, "event", record) || !matches!(null, Expression::NullLiteral(_)) {
        return None;
    }
    let [invoke, clear_a, clear_b, clear_c, remove, ret] = block(&event_branch.consequent)? else {
        return None;
    };
    let invoke = statement_call(invoke)?;
    let direct = symbol(model, &invoke.callee) == Some(notifier_symbol);
    if arity(invoke, 0).is_none()
        || !(direct || symbol(model, &invoke.callee) == Some(alias))
        || !listener(model, remove, "removeEventListener", receiver, alias)
        || !returns_void(ret)
    {
        return None;
    }
    let clears = [
        reset(model, clear_a)?,
        reset(model, clear_b)?,
        reset(model, clear_c)?,
    ];
    if clears.iter().collect::<BTreeSet<_>>().len() != 3 {
        return None;
    }
    let live = block(event_branch.alternate.as_ref()?)?;
    // Hoisted declarations have no effects. Keep the three publication
    // statements in their exact execution order, with no calls between them.
    let [a, b, c, d, e, f, add, ret] = live else {
        return None;
    };
    let declarations: Vec<_> = [a, b, c, d, e, f]
        .into_iter()
        .filter(|s| matches!(s, Statement::FunctionDeclaration(_)))
        .collect();
    let publications: Vec<_> = [a, b, c, d, e, f]
        .into_iter()
        .filter(|s| !matches!(s, Statement::FunctionDeclaration(_)))
        .collect();
    if declarations.len() != 3
        || publications.len() != 3
        || !listener(model, add, "addEventListener", receiver, alias)
        || !returns_void(ret)
    {
        return None;
    }
    let mut callbacks = Vec::new();
    for declaration in declarations {
        let (callback, function_symbol) = function(model, declaration)?;
        let publication = publications.iter().find(|p| {
            assignment(p).is_some_and(|a| symbol(model, &a.right) == Some(function_symbol))
        })?;
        let slot = assign_symbol(model, assignment(publication)?)?;
        if set(model, publication, slot) != Some(function_symbol) || !clears.contains(&slot) {
            return None;
        }
        callbacks.push((callback, slot));
    }
    let mut forwards = Vec::new();
    let mut trusted = None;
    for &(callback, slot) in &callbacks {
        if forward_body(model, callback, target, alias) == Some(slot) {
            forwards.push(slot);
        } else if trusted.replace(callback).is_some() {
            return None;
        }
    }
    let trusted = trusted?;
    let [fslot, gslot] = forwards.as_slice() else {
        return None;
    };
    let (fslot, gslot) = (*fslot, *gslot);
    if fslot == gslot {
        return None;
    }
    // Publications must follow forwarder, forwarder, trusted, even when the
    // trusted declaration was hoisted earlier in the block.
    let published: Vec<_> = publications
        .iter()
        .map(|p| assign_symbol(model, assignment(p)?))
        .collect::<Option<_>>()?;
    if ![fslot, gslot].iter().all(|s| published[..2].contains(s))
        || published[2] == fslot
        || published[2] == gslot
    {
        return None;
    }
    // Every callback's publication is unique and all remaining writes are resets.
    // Notifier's special reference is checked here without recursively asking
    // published_slot to prove the notifier again.
    if model
        .scoping
        .get_resolved_references(notifier_symbol)
        .count()
        != if direct { 2 } else { 1 }
        || !unchanged(model, notifier_symbol)
    {
        return None;
    }
    for (callback, slot) in callbacks {
        published_slot(model, callback, slot, true)?;
    }
    let w = Wiring {
        notifier,
        trusted,
        target,
        receiver,
        alias,
        forwards: [fslot, gslot],
    };
    trusted_body(model, &w)?;
    Some(w)
}

fn report(model: &Model<'_, '_>, t: &TryStatement<'_>, expected_prefix: &str) -> Option<Symbol> {
    if t.finalizer.is_some() {
        return None;
    }
    let h = t.handler.as_ref()?;
    let BindingPattern::BindingIdentifier(e) = &h.param.as_ref()?.pattern else {
        return None;
    };
    let e = e.symbol_id.get()?;
    let [s, ret] = h.body.body.as_slice() else {
        return None;
    };
    let c = statement_call(s)?;
    let (prefix, error) = binary(arity(c, 1)?[0], "+")?;
    if !unchanged(model, e)
        || !string(prefix, expected_prefix)
        || symbol(model, error) != Some(e)
        || !returns_void(ret)
    {
        return None;
    }
    symbol(model, member(&c.callee, "notify")?)
}
fn trusted_body(model: &Model<'_, '_>, w: &Wiring) -> Option<()> {
    let (event, body) = parts(model, w.trusted)?;
    let [Statement::TryStatement(t)] = body.statements.as_slice() else {
        return None;
    };
    let [Statement::IfStatement(trust), Statement::IfStatement(submit)] = t.block.body.as_slice()
    else {
        return None;
    };
    let (untrusted, uncancelable) = logical(&trust.test, "&&")?;
    if !member_symbol(model, neg(untrusted)?, "isTrusted", event)
        || !member_symbol(model, neg(uncancelable)?, "cancelable", event)
        || trust.alternate.is_some()
        || !one_void(&trust.consequent)
        || !guard(model, submit, event, w.target)
    {
        return None;
    }
    let notified = block(submit.alternate.as_ref()?)?;
    let (remove, a, b, notify, ret, flag) = match notified {
        [remove, a, b, notify, ret] => (remove, a, b, notify, ret, None),
        [remove, a, b, notify, flag, ret] => (
            remove,
            a,
            b,
            notify,
            ret,
            Some(flag_write(model, w.trusted, flag, false)?),
        ),
        _ => return None,
    };
    let a = reset(model, a)?;
    let b = reset(model, b)?;
    if a == b
        || !w.forwards.contains(&a)
        || !w.forwards.contains(&b)
        || !listener(model, remove, "removeEventListener", w.receiver, w.alias)
        || !returns_void(ret)
    {
        return None;
    }
    let c = statement_call(notify)?;
    let sink = data_capture(model, w.trusted, member(&c.callee, "notify")?)?;
    if symbol(model, arity(c, 1)?[0]) != Some(w.target) {
        return None;
    }
    let error = report(model, t, "2.28:")?;
    let (_, nb) = parts(model, w.notifier)?;
    let [Statement::TryStatement(nt)] = nb.statements.as_slice() else {
        return None;
    };
    let [nr, Statement::IfStatement(ng)] = nt.block.body.as_slice() else {
        return None;
    };
    if !listener(model, nr, "removeEventListener", w.receiver, w.alias) {
        return None;
    }
    let (nn, notifier_flag) = match block(ng.alternate.as_ref()?)? {
        [nn, _] => (nn, None),
        [nn, state, _] => (nn, Some(flag_write(model, w.notifier, state, true)?)),
        _ => return None,
    };
    if flag != notifier_flag {
        return None;
    }
    let notifier_sink = symbol(model, member(&statement_call(nn)?.callee, "notify")?)?;
    (error == report(model, nt, "2.27:")? && sink != error && sink != notifier_sink).then_some(())
}

pub(super) fn trusted(model: &Model<'_, '_>, site: FunctionSite) -> Option<()> {
    // Locate the exact sibling declaration through the trusted handler's own
    // removal argument, never through a last-value or similarly shaped donor.
    let (_, body) = parts(model, site)?;
    let [Statement::TryStatement(t)] = body.statements.as_slice() else {
        return None;
    };
    let [_, Statement::IfStatement(g)] = t.block.body.as_slice() else {
        return None;
    };
    let statements = block(g.alternate.as_ref()?)?;
    if !(statements.len() == 5 || statements.len() == 6) {
        return None;
    }
    let remove = &statements[0];
    let alias = symbol(model, arity(statement_call(remove)?, 2)?[1])?;
    if model.binding_writes(alias) != 1 {
        return None;
    }
    let r = model
        .scoping
        .get_resolved_references(alias)
        .find(|r| r.is_write())?;
    let AstKind::AssignmentExpression(a) = model.nodes.parent_kind(r.node_id()) else {
        return None;
    };
    let s = symbol(model, &a.right)?;
    let AstKind::Function(n) = model.nodes.kind(model.scoping.symbol_declaration(s)) else {
        return None;
    };
    let notifier_site = model.site_by_span(n.span)?;
    let w = wiring(model, notifier_site, alias)?;
    if w.trusted.node_id != site.node_id || notifier(model, notifier_site)? != w.target {
        return None;
    }
    Some(())
}

// An optional local boolean state store after the opaque notification returns.
// This is lexical state, not a lifecycle/native identity. Inspect every writer,
// including callbacks outside this handler; never infer a last writer.
pub(super) fn flag_write(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    value: bool,
) -> Option<Symbol> {
    let a = assignment(statement)?;
    if !matches!(&a.right, Expression::BooleanLiteral(b) if b.value == value) {
        return None;
    }
    let state = assign_symbol(model, a)?;
    if !capture_identity(model, site, state, a.span.start) {
        return None;
    }
    let AstKind::VariableDeclarator(d) = model.nodes.kind(model.scoping.symbol_declaration(state))
    else {
        return None;
    };
    if d.init.is_some() {
        return None;
    }
    for r in model
        .scoping
        .get_resolved_references(state)
        .filter(|r| r.is_write())
    {
        let AstKind::AssignmentExpression(a) = model.nodes.parent_kind(r.node_id()) else {
            return None;
        };
        if a.operator.as_str() != "="
            || assign_symbol(model, a) != Some(state)
            || !matches!(&a.right, Expression::BooleanLiteral(_))
        {
            return None;
        }
    }
    Some(state)
}

// Separate bounded selector variant: materialize the event/originalTarget test,
// then assign the captured receiver on BOTH exclusive arms before publishing
// any callback. The loop's extra carried value is retained as an exact store;
// parentNode reads can invoke getters and are never collapsed or treated as DOM.
fn stateful_receiver(
    model: &Model<'_, '_>,
    flags: &Statement<'_>,
    predicate: &Statement<'_>,
    selected: &Statement<'_>,
    selector: &Statement<'_>,
    record: Symbol,
    target: Symbol,
) -> Option<Symbol> {
    let Statement::VariableDeclaration(d) = flags else {
        return None;
    };
    let [test, carried] = d.declarations.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(test_id) = &test.id else {
        return None;
    };
    let BindingPattern::BindingIdentifier(carried_id) = &carried.id else {
        return None;
    };
    let test_symbol = test_id.symbol_id.get()?;
    let carried_symbol = carried_id.symbol_id.get()?;
    if test.init.is_some() || carried.init.is_some() {
        return None;
    }
    let Statement::IfStatement(p) = predicate else {
        return None;
    };
    let (nonnull, untrusted) = logical(&p.test, "&&")?;
    let (event, null) = binary(nonnull, "!=")?;
    if !member_symbol(model, event, "event", record)
        || !matches!(null, Expression::NullLiteral(_))
        || !member(neg(untrusted)?, "isTrusted")
            .is_some_and(|e| member_symbol(model, e, "event", record))
    {
        return None;
    }
    let [copy, check] = block(&p.consequent)? else {
        return None;
    };
    let copy = assignment(copy)?;
    let check = assignment(check)?;
    let (key, event) = binary(&check.right, "in")?;
    if assign_symbol(model, copy) != Some(carried_symbol)
        || !member_symbol(model, &copy.right, "event", record)
        || assign_symbol(model, check) != Some(test_symbol)
        || !string(key, "originalTarget")
        || symbol(model, event) != Some(carried_symbol)
    {
        return None;
    }
    let [no_event, fallback] = block(p.alternate.as_ref()?)? else {
        return None;
    };
    let no_event = assignment(no_event)?;
    if assign_symbol(model, no_event) != Some(test_symbol)
        || !matches!(&no_event.right, Expression::BooleanLiteral(b) if !b.value)
        || set(model, fallback, carried_symbol) != Some(target)
    {
        return None;
    }
    let (receiver, init) = decl(selected)?;
    if init.is_some() {
        return None;
    }
    let Statement::IfStatement(select) = selector else {
        return None;
    };
    if symbol(model, &select.test) != Some(test_symbol) {
        return None;
    }
    let [direct] = block(&select.consequent)? else {
        return None;
    };
    if set(model, direct, receiver) != Some(target) {
        return None;
    }
    let [cursor, Statement::ForStatement(walk), finish] = block(select.alternate.as_ref()?)? else {
        return None;
    };
    let (cursor, init) = decl(cursor)?;
    if symbol(model, init?) != Some(target)
        || walk.init.is_some()
        || walk.test.is_some()
        || walk.update.is_some()
    {
        return None;
    }
    let [parent, Statement::IfStatement(stop), advance, carry] = block(&walk.body)? else {
        return None;
    };
    let (parent, init) = decl(parent)?;
    if !member_symbol(model, init?, "parentNode", cursor) || !unchanged(model, parent) {
        return None;
    }
    let (first, cycle) = logical(&stop.test, "||")?;
    let (nulltest, form) = logical(first, "||")?;
    let (p, null) = binary(nulltest, "==")?;
    if symbol(model, p) != Some(parent)
        || !matches!(null, Expression::NullLiteral(_))
        || !form_predicate(model, form, parent)
        || !binary(cycle, "===").is_some_and(|(p, t)| {
            symbol(model, p) == Some(parent) && symbol(model, t) == Some(target)
        })
        || stop.alternate.is_some()
        || !matches!(&stop.consequent, Statement::BreakStatement(b) if b.label.is_none())
        || set(model, advance, cursor) != Some(parent)
    {
        return None;
    }
    let carry = assignment(carry)?;
    let (new, old) = logical(&carry.right, "??")?;
    if assign_symbol(model, carry) != Some(carried_symbol)
        || symbol(model, new) != Some(parent)
        || symbol(model, old) != Some(carried_symbol)
        || set(model, finish, receiver) != Some(cursor)
        || model.binding_writes(receiver) != 2
        || model.binding_writes(test_symbol) != 2
        || model.binding_writes(carried_symbol) != 3
        || model.binding_writes(cursor) != 1
    {
        return None;
    }
    for s in [receiver, test_symbol, carried_symbol, cursor] {
        if !plain_origin(model, s) {
            return None;
        }
    }
    Some(receiver)
}

// Direct receiver variant. The loop tests parentNode repeatedly instead of
// copying it. Those reads may have effects, so this is a separate exact body.
fn inline_receiver(
    model: &Model<'_, '_>,
    declaration: &Statement<'_>,
    selector: &Statement<'_>,
    record: Symbol,
    target: Symbol,
) -> Option<Symbol> {
    let (receiver, init) = decl(declaration)?;
    if init.is_some() {
        return None;
    }
    let Statement::IfStatement(g) = selector else {
        return None;
    };
    let (a, original) = logical(&g.test, "&&")?;
    let (nonnull, untrusted) = logical(a, "&&")?;
    let (event, null) = binary(nonnull, "!=")?;
    let (key, event_in) = binary(original, "in")?;
    if !member_symbol(model, event, "event", record)
        || !matches!(null, Expression::NullLiteral(_))
        || !member(neg(untrusted)?, "isTrusted")
            .is_some_and(|e| member_symbol(model, e, "event", record))
        || !string(key, "originalTarget")
        || !member_symbol(model, event_in, "event", record)
    {
        return None;
    }
    let [direct] = block(&g.consequent)? else {
        return None;
    };
    if set(model, direct, receiver) != Some(target) {
        return None;
    }
    let [cursor, Statement::ForStatement(walk), finish] = block(g.alternate.as_ref()?)? else {
        return None;
    };
    let (cursor, init) = decl(cursor)?;
    if symbol(model, init?) != Some(target)
        || walk.init.is_some()
        || walk.test.is_some()
        || walk.update.is_some()
    {
        return None;
    }
    let [flag, Statement::IfStatement(parent), Statement::IfStatement(stop), advance] =
        block(&walk.body)?
    else {
        return None;
    };
    let (flag, init) = decl(flag)?;
    if init.is_some() {
        return None;
    }
    let (p, null) = binary(&parent.test, "!=")?;
    if !member_symbol(model, p, "parentNode", cursor) || !matches!(null, Expression::NullLiteral(_))
    {
        return None;
    }
    let [test] = block(&parent.consequent)? else {
        return None;
    };
    let test = assignment(test)?;
    let (first, name) = logical(&test.right, "||")?;
    let (nulltest, node) = logical(first, "||")?;
    let (p, null) = binary(nulltest, "==")?;
    if assign_symbol(model, test) != Some(flag)
        || !member_symbol(model, p, "parentNode", cursor)
        || !matches!(null, Expression::NullLiteral(_))
        || !binary(node, "!==").is_some_and(|(l, r)| {
            member(l, "nodeType").is_some_and(|e| member_symbol(model, e, "parentNode", cursor))
                && number(r, 1.0)
        })
        || !binary(name, "!==").is_some_and(|(l, r)| {
            member(l, "nodeName").is_some_and(|e| member_symbol(model, e, "parentNode", cursor))
                && string(r, "FORM")
        })
    {
        return None;
    }
    let [no_parent] = block(parent.alternate.as_ref()?)? else {
        return None;
    };
    let no_parent = assignment(no_parent)?;
    let (no_form, cycle) = logical(&stop.test, "||")?;
    if assign_symbol(model, no_parent) != Some(flag)
        || !matches!(&no_parent.right,Expression::BooleanLiteral(b) if !b.value)
        || symbol(model, neg(no_form)?) != Some(flag)
        || !binary(cycle, "===").is_some_and(|(p, t)| {
            member_symbol(model, p, "parentNode", cursor) && symbol(model, t) == Some(target)
        })
        || stop.alternate.is_some()
        || !matches!(&stop.consequent,Statement::BreakStatement(b) if b.label.is_none())
    {
        return None;
    }
    let advance = assignment(advance)?;
    if assign_symbol(model, advance) != Some(cursor)
        || !member_symbol(model, &advance.right, "parentNode", cursor)
        || set(model, finish, receiver) != Some(cursor)
        || model.binding_writes(receiver) != 2
        || model.binding_writes(cursor) != 1
        || model.binding_writes(flag) != 2
    {
        return None;
    }
    for s in [receiver, cursor, flag] {
        if !plain_origin(model, s) {
            return None;
        }
    }
    Some(receiver)
}
