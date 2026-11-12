//! Whole-body trait collection. Captured source reader, tester and predicate
//! retain opaque identities; no native API origin is inferred from their uses.
use crate::naming::support::seeded_flow as q;
use super::*;
type S = SymbolId;
fn id(m: &Model<'_, '_>, e: &Expression<'_>) -> Option<S> {
    q::symbol(m, e)
}
fn plain<'a>(m: &Model<'_, '_>, s: &'a Statement<'a>) -> Option<(S, Option<&'a Expression<'a>>)> {
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
    let BindingPattern::BindingIdentifier(i) = &v.id else {
        return None;
    };
    let s = i.symbol_id.get()?;
    m.scoping
        .symbol_redeclarations(s)
        .is_empty()
        .then_some((s, v.init.as_ref()))
}
fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(&b.body)
}
fn branch<'a>(
    s: &'a Statement<'a>,
) -> Option<(&'a Expression<'a>, &'a [Statement<'a>], &'a [Statement<'a>])> {
    let Statement::IfStatement(b) = s else {
        return None;
    };
    Some((
        &b.test,
        block(&b.consequent)?,
        block(b.alternate.as_ref()?)?,
    ))
}
fn store<'a>(m: &Model<'_, '_>, s: &'a Statement<'a>, slot: S) -> Option<&'a Expression<'a>> {
    let a = q::assignment(s)?;
    (q::assign_symbol(m, a) == Some(slot)).then_some(&a.right)
}
fn boolean(e: &Expression<'_>, v: bool) -> bool {
    matches!(e,Expression::BooleanLiteral(b) if b.value==v)
}
fn not(m: &Model<'_, '_>, e: &Expression<'_>, slot: S) -> bool {
    matches!(e,Expression::UnaryExpression(u) if u.operator.as_str()=="!"&&id(m,&u.argument)==Some(slot))
}
fn field(m: &Model<'_, '_>, s: &Statement<'_>, record: S, key: &str) -> Option<()> {
    let a = q::assignment(s)?;
    let AssignmentTarget::StaticMemberExpression(t) = &a.left else {
        return None;
    };
    (!t.optional && t.property.name == key && id(m, &t.object) == Some(record)).then_some(())
}
fn proof(m: &Model<'_, '_>, site: FunctionSite, f: &Function<'_>) -> Option<()> {
    if f.r#async || f.generator || !q::plain_params(m, &f.params) || f.params.items.len() != 1 {
        return None;
    }
    let b = f.body.as_ref()?;
    if !b.directives.is_empty() {
        return None;
    }
    let [alias, kind, source_decl, source_branch, proto_branch, delete_decl, delete_branch, record, source_field, proto_field, delete_field, finish] =
        b.statements.as_slice()
    else {
        return None;
    };
    let (value, Some(v)) = plain(m, alias)? else {
        return None;
    };
    if m.site_parameter_slot(v, site) != Some(0) {
        return None;
    }
    let (callable, Some(v)) = plain(m, kind)? else {
        return None;
    };
    let Expression::BinaryExpression(eq) = v else {
        return None;
    };
    let Expression::UnaryExpression(t) = &eq.left else {
        return None;
    };
    if eq.operator.as_str() != "==="
        || t.operator.as_str() != "typeof"
        || id(m, &t.argument) != Some(value)
        || !q::string(&eq.right, "function")
    {
        return None;
    }
    let (source, None) = plain(m, source_decl)? else {
        return None;
    };
    let (test, yes, no) = branch(source_branch)?;
    if id(m, test) != Some(callable) {
        return None;
    }
    let [copy, check] = yes else { return None };
    if m.site_parameter_slot(store(m, copy, value)?, site) != Some(0) {
        return None;
    }
    let c = q::call(store(m, check, source)?)?;
    let tester = q::member(&c.callee, "test")?;
    q::data_capture(m, site, tester)?;
    let args = q::arity(c, 1)?;
    let reader = q::call(args[0])?;
    q::data_capture(m, site, q::member(&reader.callee, "call")?)?;
    if id(m, q::arity(reader, 1)?[0]) != Some(value) {
        return None;
    }
    let [absent] = no else { return None };
    if !boolean(store(m, absent, source)?, false) {
        return None;
    }
    let (test, yes, no) = branch(proto_branch)?;
    if id(m, test) != Some(callable) {
        return None;
    }
    let [check] = yes else { return None };
    let c = q::call(store(m, check, value)?)?;
    q::data_capture(m, site, &c.callee)?;
    let args = q::arity(c, 2)?;
    if m.site_parameter_slot(args[0], site) != Some(0) || !q::string(args[1], "prototype") {
        return None;
    }
    let [absent] = no else { return None };
    if !boolean(store(m, absent, value)?, false) {
        return None;
    }
    let (deleted, None) = plain(m, delete_decl)? else {
        return None;
    };
    let (test, yes, no) = branch(delete_branch)?;
    let Expression::LogicalExpression(t) = test else {
        return None;
    };
    if t.operator.as_str() != "&&" || id(m, &t.left) != Some(callable) || !not(m, &t.right, value) {
        return None;
    }
    let [initial, Statement::TryStatement(attempt)] = yes else {
        return None;
    };
    if !boolean(store(m, initial, deleted)?, false) || attempt.finalizer.is_some() {
        return None;
    }
    let [remove, success] = attempt.block.body.as_slice() else {
        return None;
    };
    let Expression::UnaryExpression(remove) = q::expression(remove)? else {
        return None;
    };
    if remove.operator.as_str() != "delete"
        || m.site_parameter_slot(q::member(&remove.argument, "prototype")?, site) != Some(0)
        || !boolean(store(m, success, deleted)?, true)
    {
        return None;
    }
    let h = attempt.handler.as_ref()?;
    if !h.body.body.is_empty()
        || !matches!(
            h.param.as_ref().map(|p| &p.pattern),
            Some(BindingPattern::BindingIdentifier(_))
        )
    {
        return None;
    }
    let [absent] = no else { return None };
    if !boolean(store(m, absent, deleted)?, false) {
        return None;
    }
    let Expression::ObjectExpression(o) = store(m, record, callable)? else {
        return None;
    };
    let [ObjectPropertyKind::ObjectProperty(p)] = o.properties.as_slice() else {
        return None;
    };
    if p.computed
        || p.method
        || p.kind != PropertyKind::Init
        || property_key_name(&p.key).as_deref() != Some("a")
        || id(m, &p.value) != Some(callable)
    {
        return None;
    }
    field(m, source_field, callable, "b")?;
    if id(m, &q::assignment(source_field)?.right) != Some(source) {
        return None;
    }
    field(m, proto_field, callable, "c")?;
    if !not(m, &q::assignment(proto_field)?.right, value) {
        return None;
    }
    field(m, delete_field, callable, "d")?;
    if id(m, &q::assignment(delete_field)?.right) != Some(deleted) {
        return None;
    }
    let Statement::ReturnStatement(r) = finish else {
        return None;
    };
    if id(m, r.argument.as_ref()?) != Some(callable) {
        return None;
    }
    // Exact local write inventories prevent outside mutation/donor facts.
    if m.binding_writes(value) != 3
        || m.binding_writes(callable) != 1
        || m.binding_writes(source) != 2
        || m.binding_writes(deleted) != 3
    {
        return None;
    }
    Some(())
}
fn five_field_proof(m: &Model<'_, '_>, site: FunctionSite, f: &Function<'_>) -> Option<()> {
    if f.r#async || f.generator || !q::plain_params(m, &f.params) || f.params.items.len() != 1 {
        return None;
    }
    let b = f.body.as_ref()?;
    if !b.directives.is_empty() {
        return None;
    }
    let [alias, kind, source_decl, source_branch, proto_decl, proto_branch, reader_branch, delete_decl, delete_branch, tail @ ..] =
        b.statements.as_slice()
    else {
        return None;
    };
    let (value, Some(v)) = plain(m, alias)? else {
        return None;
    };
    if m.site_parameter_slot(v, site) != Some(0) {
        return None;
    }
    let (callable, Some(v)) = plain(m, kind)? else {
        return None;
    };
    let Expression::BinaryExpression(eq) = v else {
        return None;
    };
    let Expression::UnaryExpression(t) = &eq.left else {
        return None;
    };
    if eq.operator.as_str() != "==="
        || t.operator.as_str() != "typeof"
        || id(m, &t.argument) != Some(value)
        || !q::string(&eq.right, "function")
    {
        return None;
    }
    let (source, None) = plain(m, source_decl)? else {
        return None;
    };
    let (test, yes, no) = branch(source_branch)?;
    if id(m, test) != Some(callable) {
        return None;
    }
    let [copy, check] = yes else { return None };
    if m.site_parameter_slot(store(m, copy, value)?, site) != Some(0) {
        return None;
    }
    let c = q::call(store(m, check, source)?)?;
    let tester = q::member(&c.callee, "test")?;
    q::data_capture(m, site, tester)?;
    let args = q::arity(c, 1)?;
    let reader = q::call(args[0])?;
    let reader_identity = q::data_capture(m, site, q::member(&reader.callee, "call")?)?;
    if id(m, q::arity(reader, 1)?[0]) != Some(value) {
        return None;
    }
    let [absent] = no else { return None };
    if !boolean(store(m, absent, source)?, false) {
        return None;
    }
    let (prototype, None) = plain(m, proto_decl)? else { return None; };
    let (test, yes, no) = branch(proto_branch)?;
    if id(m, test) != Some(callable) {
        return None;
    }
    let [copy, check] = yes else { return None };
    if m.site_parameter_slot(store(m, copy, value)?, site) != Some(0) { return None; }
    let c = q::call(store(m, check, prototype)?)?;
    q::data_capture(m, site, &c.callee)?;
    let args = q::arity(c, 2)?;
    if id(m, args[0]) != Some(value) || !q::string(args[1], "prototype") {
        return None;
    }
    let [absent] = no else { return None };
    if !boolean(store(m, absent, prototype)?, false) {
        return None;
    }
    let (test, yes, no) = branch(reader_branch)?;
    if id(m, test) != Some(callable) { return None; }
    let [read] = yes else { return None; };
    let read = q::call(store(m, read, value)?)?;
    if id(m, q::member(&read.callee, "call")?) != Some(reader_identity)
        || m.site_parameter_slot(q::arity(read, 1)?[0], site) != Some(0) { return None; }
    let [absent] = no else { return None; };
    if !q::string(store(m, absent, value)?, "") { return None; }
    let (deleted, None) = plain(m, delete_decl)? else {
        return None;
    };
    let (test, yes, no) = branch(delete_branch)?;
    let Expression::LogicalExpression(t) = test else {
        return None;
    };
    if t.operator.as_str() != "&&" || id(m, &t.left) != Some(callable) || !not(m, &t.right, prototype) {
        return None;
    }
    let [initial, Statement::TryStatement(attempt)] = yes else {
        return None;
    };
    if !boolean(store(m, initial, deleted)?, false) || attempt.finalizer.is_some() {
        return None;
    }
    let [remove, success] = attempt.block.body.as_slice() else {
        return None;
    };
    let Expression::UnaryExpression(remove) = q::expression(remove)? else {
        return None;
    };
    if remove.operator.as_str() != "delete"
        || m.site_parameter_slot(q::member(&remove.argument, "prototype")?, site) != Some(0)
        || !boolean(store(m, success, deleted)?, true)
    {
        return None;
    }
    let h = attempt.handler.as_ref()?;
    if !h.body.body.is_empty()
        || !matches!(
            h.param.as_ref().map(|p| &p.pattern),
            Some(BindingPattern::BindingIdentifier(_))
        )
    {
        return None;
    }
    let [absent] = no else { return None };
    if !boolean(store(m, absent, deleted)?, false) {
        return None;
    }
    let (record, fields, finish) = match tail {
        [record, finish] => (record, None, finish),
        [record, d, e, finish] => (record, Some((d,e)), finish),
        _ => return None,
    };
    let Expression::ObjectExpression(o) = store(m, record, callable)? else { return None; };
    if o.properties.len() != 5 { return None; }
    for (p, key) in o.properties.iter().zip(["a", "b", "c", "d", "e"]) {
        let ObjectPropertyKind::ObjectProperty(p) = p else { return None; };
        if p.computed || p.method || p.shorthand || p.kind != PropertyKind::Init
            || property_key_name(&p.key).as_deref() != Some(key) { return None; }
        let ok = match key {
            "a" => id(m, &p.value) == Some(callable),
            "b" => id(m, &p.value) == Some(source),
            "c" => not(m, &p.value, prototype),
            "d" if fields.is_none() => id(m, &p.value) == Some(deleted),
            "e" if fields.is_none() => id(m, &p.value) == Some(value),
            _ => matches!(&p.value, Expression::UnaryExpression(u) if u.operator.as_str()=="void" && matches!(&u.argument,Expression::NumericLiteral(n) if n.value==0.0)),
        };
        if !ok { return None; }
    }
    if let Some((d,e)) = fields {
        field(m,d,callable,"d")?;
        field(m,e,callable,"e")?;
        if id(m,&q::assignment(d)?.right)!=Some(deleted) || id(m,&q::assignment(e)?.right)!=Some(value) { return None; }
    }
    let Statement::ReturnStatement(r) = finish else { return None; };
    if id(m,r.argument.as_ref()?)!=Some(callable) { return None; }
    // Exact local write inventories prevent outside mutation/donor facts.
    if m.binding_writes(value) != 4
        || m.binding_writes(prototype) != 2
        || m.binding_writes(callable) != 1
        || m.binding_writes(source) != 2
        || m.binding_writes(deleted) != 3
    {
        return None;
    }
    Some(())
}

pub(in crate::naming) fn evaluate(
    m: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = m.nodes.kind(site.node_id) else {
        return vec![];
    };
    if five_field_proof(m, site, function).is_some() {
        return vec![proposal("collectCallabilitySourceTextAndPrototypeDeletionTraits", "runtime_function_shape_traits_source_text", "complete own one-formal protocol: callable gate; opaque source-reader identity shared by source-test and returned source text; opaque prototype predicate consumes the same formal alias; guarded prototype deletion with exact caught completion; ordered a/b/c/d/e record and optional void-initialized d/e publication; every binding/write/receiver/argument/branch/return checked, no native identity inferred", f)];
    }
    if proof(m, site, function).is_none() {
        return vec![];
    }
    vec![proposal("collectCallabilitySourceAndPrototypeDeletionTraits","runtime_function_shape_traits","entire one-formal body: callable typeof gate; captured tester.test on captured source-reader.call of the same formal; opaque captured predicate(formal, prototype); guarded prototype delete with caught failure; ordered a/b/c/d record of callable, source test, negated predicate and deletion completion. Captured helpers retain opaque purposes; all bindings and writes checked",f)]
}
