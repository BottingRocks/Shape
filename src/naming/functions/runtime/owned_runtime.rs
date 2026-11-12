//! Bounded owned-body runtime proofs. No generated identifiers identify a role.
use super::*;
use oxc_span::GetSpan;

type Symbol = oxc_semantic::SymbolId;
fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(e, model.scoping)
}
fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    match e {
        Expression::StaticMemberExpression(m) if !m.optional && m.property.name == key => {
            Some(&m.object)
        }
        _ => None,
    }
}
// Resolve only plain, single-origin aliases initialized before this use. Host
// namespace roots must be unshadowed browser globals, never capture spellings.
fn api_path(model: &Model<'_, '_>, e: &Expression<'_>) -> Vec<String> {
    fn resolve(model: &Model<'_, '_>, e: &Expression<'_>, depth: usize) -> Vec<String> {
        if depth > 8 {
            return Vec::new();
        }
        match e {
            Expression::Identifier(i) => {
                if crate::analysis::identifier_symbol(i, model.scoping).is_none() {
                    return vec![i.name.to_string()];
                }
                let Some(value) = stable_value(model, e, 0) else {
                    return Vec::new();
                };
                resolve(model, value, depth + 1)
            }
            Expression::StaticMemberExpression(m) if !m.optional => {
                let mut path = resolve(model, &m.object, depth + 1);
                if !path.is_empty() {
                    path.push(m.property.name.to_string());
                }
                path
            }
            _ => Vec::new(),
        }
    }
    let mut path = resolve(model, e, 0);
    if path.len() > 1 && matches!(path[0].as_str(), "window" | "self" | "globalThis") {
        path.remove(0);
    }
    path
}

fn call<'a>(e: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    match e {
        Expression::CallExpression(c) if !c.optional => Some(c),
        _ => None,
    }
}
fn expression<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    match s {
        Statement::ExpressionStatement(s) => Some(&s.expression),
        _ => None,
    }
}
fn returned<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    match s {
        Statement::ReturnStatement(s) => s.argument.as_ref(),
        _ => None,
    }
}
fn immutable(model: &Model<'_, '_>, s: Symbol) -> bool {
    model.binding_writes(s) == 0 && model.scoping.symbol_redeclarations(s).is_empty()
}
fn formals(model: &Model<'_, '_>, f: &Function<'_>, count: usize) -> bool {
    f.params.rest.is_none()
        && f.params.items.len() == count
        && f.params.items.iter().all(|p| {
            p.initializer.is_none()
                && matches!(&p.pattern, BindingPattern::BindingIdentifier(i)
            if i.symbol_id.get().is_some_and(|s| immutable(model, s)))
        })
}
// Source-value resolution has the same strict plain-variable origin guard as
// stable_function, but permits a RegExp value. Parameters and loop bindings are
// never origins. The small depth bound also refuses alias cycles.
fn stable_value<'a>(
    model: &Model<'_, 'a>,
    e: &'a Expression<'a>,
    depth: usize,
) -> Option<&'a Expression<'a>> {
    if depth > 8 {
        return None;
    }
    let Expression::Identifier(_) = e else {
        return Some(e);
    };
    let Some(s) = symbol(model, e) else {
        // An unresolved root is terminal, not a scoped alias origin. Consumers
        // still require an exact supported API path.
        return Some(e);
    };
    if !model.scoping.symbol_redeclarations(s).is_empty() {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&v.id, BindingPattern::BindingIdentifier(i) if i.symbol_id.get() == Some(s))
        || !matches!(
            model.nodes.parent_kind(declaration),
            AstKind::VariableDeclaration(d) if matches!(d.kind, VariableDeclarationKind::Var | VariableDeclarationKind::Let | VariableDeclarationKind::Const)
        )
        || matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
        || !(model.binding_writes(s) == 0 && v.init.is_some()
            || model.binding_writes(s) == 1 && v.init.is_none())
    {
        return None;
    }
    let value = model.symbol_value(s)?;
    if value.span().end > e.span().start {
        return None;
    }
    let container = model.nodes.parent_id(model.nodes.parent_id(declaration));
    if !matches!(
        model.nodes.kind(container),
        AstKind::FunctionBody(_) | AstKind::Program(_)
    ) {
        return None;
    }
    if v.init.is_none()
        && !model.scoping.get_resolved_references(s).any(|r| {
            let parent = model.nodes.parent_id(r.node_id());
            matches!(model.nodes.kind(parent), AstKind::AssignmentExpression(a)
                if a.operator.as_str() == "=" && a.right.span() == value.span())
                && matches!(
                    model.nodes.parent_kind(parent),
                    AstKind::ExpressionStatement(_)
                )
                && model.nodes.parent_id(model.nodes.parent_id(parent)) == container
        })
    {
        return None;
    }
    stable_value(model, value, depth + 1)
}
fn native_regex(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<()> {
    let e = stable_value(model, e, 0)?;
    // These expressions describe the native-code source suffix, not a build
    // token. Refuse flags (especially stateful g/y) and arbitrary native text.
    const PATTERNS: &[&str] = &[
        r"{\s*\[\s*native\s+code\s*]\s*}\s*$",
        r"\{\s*\[native code\]\s*\}\s*$",
    ];
    match e {
        Expression::RegExpLiteral(r)
            if r.regex.flags.to_string().is_empty()
                && PATTERNS.contains(&r.regex.pattern.text.as_str()) =>
        {
            Some(())
        }
        Expression::CallExpression(c)
            if !c.optional && api_path(model, &c.callee) == ["RegExp"] =>
        {
            match c.arguments.as_slice() {
                [Argument::StringLiteral(s)] if PATTERNS.contains(&s.value.as_str()) => Some(()),
                _ => None,
            }
        }
        _ => None,
    }
}
fn native_function(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [Statement::IfStatement(b)] = body.statements.as_slice() else {
        return None;
    };
    let Expression::LogicalExpression(and) = &b.test else {
        return None;
    };
    let Expression::BinaryExpression(kind) = &and.left else {
        return None;
    };
    let Expression::UnaryExpression(type_) = &kind.left else {
        return None;
    };
    let Expression::BinaryExpression(method) = &and.right else {
        return None;
    };
    if and.operator.as_str() != "&&"
        || kind.operator.as_str() != "==="
        || type_.operator.as_str() != "typeof"
        || model.site_parameter_slot(&type_.argument, site) != Some(0)
        || !matches!(&kind.right, Expression::StringLiteral(s) if s.value == "function")
        || method.operator.as_str() != "==="
        || model.site_parameter_slot(member(&method.left, "toString")?, site) != Some(0)
        || api_path(model, &method.right) != ["Function", "prototype", "toString"]
    {
        return None;
    }
    let Statement::BlockStatement(yes) = &b.consequent else {
        return None;
    };
    let Statement::BlockStatement(no) = b.alternate.as_ref()? else {
        return None;
    };
    let [yes] = yes.body.as_slice() else {
        return None;
    };
    let [no] = no.body.as_slice() else {
        return None;
    };
    if !matches!(returned(no)?, Expression::BooleanLiteral(v) if !v.value) {
        return None;
    }
    let Expression::UnaryExpression(outer) = returned(yes)? else {
        return None;
    };
    let Expression::UnaryExpression(inner) = &outer.argument else {
        return None;
    };
    if outer.operator.as_str() != "!" || inner.operator.as_str() != "!" {
        return None;
    }
    let exec = call(&inner.argument)?;
    native_regex(model, member(&exec.callee, "exec")?)?;
    let exec_args = call_arguments(exec);
    let [arg] = exec_args.as_slice() else {
        return None;
    };
    if exec.arguments.len() != 1 {
        return None;
    }
    let string = call(arg)?;
    (string.arguments.is_empty()
        && model.site_parameter_slot(member(&string.callee, "toString")?, site) == Some(0))
    .then_some(())
}
fn code_units(model: &Model<'_, '_>, site: FunctionSite, body: &FunctionBody<'_>) -> Option<()> {
    let [initial, Statement::IfStatement(branch), end] = body.statements.as_slice() else {
        return None;
    };
    let v = one_variable(initial)?;
    let output = plain_local(model, v)?;
    if !immutable(model, output)
        || !matches!(v.init.as_ref()?, Expression::ArrayExpression(a) if a.elements.is_empty())
        || branch.alternate.is_some()
    {
        return None;
    }
    let Expression::BinaryExpression(test) = &branch.test else {
        return None;
    };
    if test.operator.as_str() != "!="
        || model.site_parameter_slot(&test.left, site) != Some(0)
        || !matches!(&test.right, Expression::NullLiteral(_))
    {
        return None;
    }
    let Statement::BlockStatement(block) = &branch.consequent else {
        return None;
    };
    let [s] = block.body.as_slice() else {
        return None;
    };
    let each = call(expression(s)?)?;
    if model.site_parameter_slot(member(&each.callee, "forEach")?, site) != Some(0) {
        return None;
    }
    let [Argument::ArrowFunctionExpression(a)] = each.arguments.as_slice() else {
        return None;
    };
    let [p] = a.params.items.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(id) = &p.pattern else {
        return None;
    };
    let input = id.symbol_id.get()?;
    if a.r#async
        || a.params.rest.is_some()
        || p.initializer.is_some()
        || !immutable(model, input)
        || !a.body.directives.is_empty()
    {
        return None;
    }
    let [s] = a.body.statements.as_slice() else {
        return None;
    };
    let push = call(expression(s)?)?;
    if symbol(model, member(&push.callee, "push")?) != Some(output) || push.arguments.len() != 1 {
        return None;
    }
    let args = call_arguments(push);
    let convert = call(args.first()?)?;
    if api_path(model, &convert.callee) != ["String", "fromCharCode"]
        || convert.arguments.len() != 1
        || call_arguments(convert)
            .first()
            .and_then(|e| symbol(model, e))
            != Some(input)
    {
        return None;
    }
    let join = call(returned(end)?)?;
    (symbol(model, member(&join.callee, "join")?) == Some(output)
        && matches!(join.arguments.as_slice(), [Argument::StringLiteral(s)] if s.value.is_empty()))
    .then_some(())
}
fn prototype_selector(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [initial, Statement::IfStatement(branch), end] = body.statements.as_slice() else {
        return None;
    };
    let v = one_variable(initial)?;
    let selected = plain_local(model, v)?;
    if v.init.is_some()
        || model.binding_writes(selected) != 2
        || api_path(model, &branch.test) != ["Object", "setPrototypeOf"]
    {
        return None;
    }
    let Statement::BlockStatement(yes) = &branch.consequent else {
        return None;
    };
    let Statement::BlockStatement(no) = branch.alternate.as_ref()? else {
        return None;
    };
    let [yes] = yes.body.as_slice() else {
        return None;
    };
    let assignment = |s| -> Option<&AssignmentExpression<'_>> {
        let Expression::AssignmentExpression(a) = expression(s)? else {
            return None;
        };
        (a.operator.as_str() == "="
            && matches!(&a.left, AssignmentTarget::AssignmentTargetIdentifier(i)
            if crate::analysis::identifier_symbol(i, model.scoping) == Some(selected)))
        .then_some(a)
    };
    let bind = call(&assignment(yes)?.right)?;
    if !bind.arguments.is_empty()
        || api_path(model, member(&bind.callee, "bind")?) != ["Object", "setPrototypeOf"]
    {
        return None;
    }
    let [Statement::FunctionDeclaration(child), write] = no.body.as_slice() else {
        return None;
    };
    if child.r#async || child.generator || !formals(model, child, 2) {
        return None;
    }
    let child_site = model.site_by_span(child.span)?;
    if stable_function(model, &assignment(write)?.right, 0)?.span != child.span {
        return None;
    }
    let child_body = child.body.as_ref()?;
    let [set, ret] = child_body.statements.as_slice() else {
        return None;
    };
    if !child_body.directives.is_empty() {
        return None;
    }
    let Expression::AssignmentExpression(set) = expression(set)? else {
        return None;
    };
    let AssignmentTarget::StaticMemberExpression(left) = &set.left else {
        return None;
    };
    if set.operator.as_str() != "="
        || left.optional
        || left.property.name != "__proto__"
        || model.site_parameter_slot(&left.object, child_site) != Some(0)
        || model.site_parameter_slot(&set.right, child_site) != Some(1)
        || model.site_parameter_slot(returned(ret)?, child_site) != Some(0)
    {
        return None;
    }
    let invoke = call(returned(end)?)?;
    let args = call_arguments(invoke);
    (symbol(model, &invoke.callee) == Some(selected)
        && invoke.arguments.len() == 2
        && args.len() == 2
        && model.site_parameter_slot(args[0], site) == Some(0)
        && model.site_parameter_slot(args[1], site) == Some(1))
    .then_some(())
}

// Small clock bodies are proved independently; inherited names never establish
// callable identity. Alias resolution above enforces declaration and use order.
fn epoch_milliseconds(model: &Model<'_, '_>, body: &FunctionBody<'_>) -> Option<()> {
    let [s] = body.statements.as_slice() else {
        return None;
    };
    let c = call(returned(s)?)?;
    if c.arguments.is_empty() && api_path(model, &c.callee) == ["Date", "now"] {
        return Some(());
    }
    if api_path(model, &c.callee) != ["Number"] || c.arguments.len() != 1 {
        return None;
    }
    let args = call_arguments(c);
    let Expression::NewExpression(date) = args.first()? else {
        return None;
    };
    (date.arguments.is_empty() && api_path(model, &date.callee) == ["Date"]).then_some(())
}

fn clock_origin(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<()> {
    let path = api_path(model, e);
    if path == ["Date", "now"] {
        return Some(());
    }
    // A performance method must keep its receiver; Date.now also permits bind.
    if let Some(value) = stable_value(model, e, 0) {
        if let Some(c) = call(value) {
            let method = member(&c.callee, "bind")?;
            let path = api_path(model, method);
            let args = call_arguments(c);
            if c.arguments.len() == 1
                && args.len() == 1
                && ((path == ["performance", "now"] && api_path(model, args[0]) == ["performance"])
                    || (path == ["Date", "now"] && api_path(model, args[0]) == ["Date"]))
            {
                return Some(());
            }
            return None;
        }
    }
    // stable_function alone does not check future initializer positions.
    if let Expression::Identifier(_) = e {
        let s = symbol(model, e)?;
        if matches!(
            model.nodes.kind(model.scoping.symbol_declaration(s)),
            AstKind::VariableDeclarator(_)
        ) {
            stable_value(model, e, 0)?;
        }
    }
    let target = stable_function(model, e, 0)?;
    let AstKind::Function(f) = model.nodes.kind(target.node_id) else {
        return None;
    };
    if f.r#async || f.generator || !formals(model, f, 0) {
        return None;
    }
    let body = f.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    epoch_milliseconds(model, body)
}

fn elapsed_milliseconds(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [s] = body.statements.as_slice() else {
        return None;
    };
    let Expression::BinaryExpression(delta) = returned(s)? else {
        return None;
    };
    if delta.operator.as_str() != "-" {
        return None;
    }
    let current = call(&delta.left)?;
    if !current.arguments.is_empty() {
        return None;
    }
    let clock = symbol(model, &current.callee)?;
    let baseline = symbol(model, &delta.right)?;
    if site
        .span
        .contains_inclusive(model.scoping.symbol_span(clock))
        || site
            .span
            .contains_inclusive(model.scoping.symbol_span(baseline))
    {
        return None;
    }
    clock_origin(model, &current.callee)?;
    let initial = call(stable_value(model, &delta.right, 0)?)?;
    (initial.arguments.is_empty()
        && symbol(model, &initial.callee) == Some(clock)
        && clock_origin(model, &initial.callee).is_some())
    .then_some(())
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
    if function.r#async || function.generator || !body.directives.is_empty() {
        return Vec::new();
    }
    let mut claims = Vec::new();
    let mut add =
        |name, detail| claims.push(proposal(name, "shared_runtime_owned_body", detail, f));
    if formals(model, function, 0) {
        if epoch_milliseconds(model, body).is_some() {
            add("getEpochMilliseconds", "sole return of proven unshadowed Date.now() or Number(new Date()); strict stable API aliases and exact zero-argument construction");
        }
        if elapsed_milliseconds(model, site, body).is_some() {
            add("getElapsedMilliseconds", "sole return of captured stable proven clock() minus immutable captured baseline initialized by the same scoped zero-argument clock; exact receiver and initialization order");
        }
    }
    if formals(model, function, 1) {
        if native_function(model, site, body).is_some() {
            add("isNativeFunction", "exact typeof/function and original Function.prototype.toString guard; double-negated exec of a stable, flagless native-code suffix RegExp on the same formal's zero-argument toString; false alternate");
        }
        if code_units(model, site, body).is_some() {
            add("codeUnitsToString", "null-guarded formal forEach callback pushes unshadowed String.fromCharCode of its exact element into the immutable local empty array and returns that array joined with an empty delimiter");
        }
        if let [s] = body.statements.as_slice() {
            if expression(s).and_then(call).is_some_and(|c| {
                c.arguments.is_empty()
                    && member(&c.callee, "preventDefault")
                        .is_some_and(|e| model.site_parameter_slot(e, site) == Some(0))
            }) {
                add("preventDefaultHandler", "sole action invokes preventDefault with no arguments on the immutable sole formal; no return");
            }
        }
    }
    if formals(model, function, 2) && prototype_selector(model, site, body).is_some() {
        add("setObjectPrototype", "selects unshadowed Object.setPrototypeOf.bind() or exact immutable nested __proto__ assignment-and-return fallback, then returns selected(slot0,slot1)");
    }
    claims
}
