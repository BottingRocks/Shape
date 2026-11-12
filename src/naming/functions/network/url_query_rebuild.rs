//! Whole owned control-flow proof for cached URL reconstruction. No vendor,
//! identifier spelling, sibling body, or member inventory contributes evidence.
//! Both return paths must parse this input and encode both query components.
use super::super::browser::url_query_merge as q;
use super::*;
use oxc_span::GetSpan;

type S = SymbolId;
#[derive(Clone, Debug, PartialEq, Eq)]
enum E {
    S(S),
    G(String),
    T(String),
    N(i32),
    Null,
    Field(Box<E>, String),
    Index(Box<E>, Box<E>),
    Op(String, Box<E>, Box<E>),
    Choice(Box<E>, Box<E>, Box<E>),
    Call(Box<E>, Vec<E>),
    Decode,
}
fn s(x: S) -> E {
    E::S(x)
}
fn t(x: &str) -> E {
    E::T(x.into())
}
fn g(x: &str) -> E {
    E::G(x.into())
}
fn f(x: E, k: &str) -> E {
    E::Field(Box::new(x), k.into())
}
fn ix(x: E, k: E) -> E {
    E::Index(Box::new(x), Box::new(k))
}
fn op(k: &str, a: E, b: E) -> E {
    E::Op(k.into(), Box::new(a), Box::new(b))
}
fn choice(a: E, b: E, c: E) -> E {
    E::Choice(Box::new(a), Box::new(b), Box::new(c))
}
fn c(x: E, a: Vec<E>) -> E {
    E::Call(Box::new(x), a)
}
fn plus(a: E, b: E) -> E {
    op("+", a, b)
}
fn nonempty(x: E) -> E {
    op(">", f(x, "length"), E::N(0))
}
fn optional(x: E, prefix: &str) -> E {
    choice(nonempty(x.clone()), plus(t(prefix), x), t(""))
}
fn sum(xs: Vec<E>) -> E {
    xs.into_iter().reduce(plus).unwrap()
}

/// Bounded structural expressions. Callbacks are accepted only as the direct,
/// owned, unary decodeURIComponent contract, never as opaque callable donors.
fn expr(model: &Model<'_, '_>, e: &Expression<'_>, depth: usize) -> Option<E> {
    if depth > 64 {
        return None;
    }
    let child = |e: &Expression<'_>| expr(model, e, depth + 1);
    Some(match e.without_parentheses() {
        Expression::Identifier(id) => match crate::analysis::identifier_symbol(id, model.scoping) {
            Some(id) => s(id),
            None => g(id.name.as_str()),
        },
        Expression::StringLiteral(v) => t(v.value.as_str()),
        Expression::NumericLiteral(v) if v.value.fract() == 0.0 && v.value.abs() < 1000.0 => {
            E::N(v.value as i32)
        }
        Expression::NullLiteral(_) => E::Null,
        Expression::StaticMemberExpression(m) if !m.optional => {
            f(child(&m.object)?, m.property.name.as_str())
        }
        Expression::ComputedMemberExpression(m) if !m.optional => {
            ix(child(&m.object)?, child(&m.expression)?)
        }
        Expression::BinaryExpression(b) => {
            op(b.operator.as_str(), child(&b.left)?, child(&b.right)?)
        }
        Expression::LogicalExpression(b) => {
            op(b.operator.as_str(), child(&b.left)?, child(&b.right)?)
        }
        Expression::ConditionalExpression(b) => {
            choice(child(&b.test)?, child(&b.consequent)?, child(&b.alternate)?)
        }
        Expression::CallExpression(call) if !call.optional => {
            let args = call_arguments(call);
            if args.len() != call.arguments.len() {
                return None;
            }
            c(
                child(&call.callee)?,
                args.into_iter().map(child).collect::<Option<Vec<_>>>()?,
            )
        }
        Expression::ArrowFunctionExpression(a)
            if !a.r#async && a.expression && a.body.directives.is_empty() =>
        {
            let params = q::plain_formals(model, &a.params)?;
            let [p] = params.as_slice() else {
                return None;
            };
            let [Statement::ExpressionStatement(b)] = a.body.statements.as_slice() else {
                return None;
            };
            if child(&b.expression)? != c(g("decodeURIComponent"), vec![s(*p)]) {
                return None;
            }
            E::Decode
        }
        _ => return None,
    })
}
fn eq(model: &Model<'_, '_>, e: &Expression<'_>, expected: E) -> bool {
    expr(model, e, 0) == Some(expected)
}
fn variable<'a>(model: &Model<'_, '_>, st: &'a Statement<'a>) -> Option<(S, &'a Expression<'a>)> {
    let v = q::one_variable(st)?;
    Some((q::plain_binding(model, v)?, v.init.as_ref()?))
}
fn local(model: &Model<'_, '_>, site: FunctionSite, st: &Statement<'_>, expected: E) -> Option<S> {
    let (id, e) = variable(model, st)?;
    (q::body_local(model, site, id) && model.binding_writes(id) == 0 && eq(model, e, expected))
        .then_some(id)
}
fn assign(model: &Model<'_, '_>, st: &Statement<'_>, id: S, expected: E) -> Option<()> {
    let a = q::assignment_statement(st)?;
    (a.operator.as_str() == "="
        && q::target_symbol(model, a) == Some(id)
        && eq(model, &a.right, expected))
    .then_some(())
}
fn field_assign(model: &Model<'_, '_>, st: &Statement<'_>, target: E, expected: E) -> Option<()> {
    let a = q::assignment_statement(st)?;
    let AssignmentTarget::StaticMemberExpression(m) = &a.left else {
        return None;
    };
    (a.operator.as_str() == "="
        && !m.optional
        && f(expr(model, &m.object, 0)?, m.property.name.as_str()) == target
        && eq(model, &a.right, expected))
    .then_some(())
}
fn branch<'a>(
    model: &Model<'_, '_>,
    st: &'a Statement<'a>,
    test: E,
) -> Option<(&'a [Statement<'a>], Option<&'a [Statement<'a>]>)> {
    let Statement::IfStatement(b) = st else {
        return None;
    };
    if !eq(model, &b.test, test) {
        return None;
    }
    Some((
        q::block(&b.consequent)?,
        match b.alternate.as_ref() {
            Some(a) => Some(q::block(a)?),
            None => None,
        },
    ))
}
fn invoke(model: &Model<'_, '_>, st: &Statement<'_>, expected: E) -> Option<()> {
    let Statement::ExpressionStatement(st) = st else {
        return None;
    };
    eq(model, &st.expression, expected).then_some(())
}
fn captured<'a>(model: &Model<'_, 'a>, site: FunctionSite, id: S) -> Option<&'a Expression<'a>> {
    if site.span.contains_inclusive(model.scoping.symbol_span(id))
        || model.formal_symbol_slot(id).is_some()
        || !model.scoping.symbol_redeclarations(id).is_empty()
    {
        return None;
    }
    let init = model.initializer(id);
    if model.binding_writes(id) != usize::from(init.is_none()) {
        return None;
    }
    model.symbol_value(id)
}

/// An immutable captured record initialized once in its declaring activation.
/// Matching uses its actual scoped origin; same-spelled shadow bindings differ.
fn capture_value(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<S> {
    let id = q::symbol(model, e)?;
    captured(model, site, id)?;
    Some(id)
}
fn null_record() -> E {
    c(f(g("Object"), "create"), vec![E::Null])
}

/// Prove the timing of every value exposure of a callable, rather than the
/// position of a hoisted declaration. Inert, direct-body immutable aliases may
/// precede setup only when all their eventual exposures follow both setup and
/// alias initialization. There are no call-edge shortcuts: an argument in
/// `f(f)` is an exposure in its own right, even beside the same direct callee.
/// Cycles, unused/unbound closure origins and exhausted graphs are refused.
fn exposures_after(
    model: &Model<'_, '_>,
    container: NodeId,
    ready_end: u32,
    symbol: S,
    origin_write: Option<NodeId>,
    visiting: &mut BTreeSet<S>,
    budget: &mut usize,
) -> bool {
    if visiting.len() >= 16
        || !visiting.insert(symbol)
        || model.binding_writes(symbol) != usize::from(origin_write.is_some())
        || !model.scoping.symbol_redeclarations(symbol).is_empty()
    {
        return false;
    }
    let mut any = false;
    let result = model
        .scoping
        .get_resolved_references(symbol)
        .all(|reference| {
            if reference.is_write() {
                return origin_write == Some(reference.node_id());
            }
            any = true;
            if *budget == 0 {
                return false;
            }
            *budget -= 1;
            let node = reference.node_id();
            let parent = model.nodes.parent_id(node);
            // Taking a value for this alias cannot activate it. Follow all reads,
            // including early reads and aliases of the alias; never just one late
            // caller. A recovered assignment or conditional alias is not proved.
            if let AstKind::VariableDeclarator(alias) = model.nodes.kind(parent) {
                let statement = model.nodes.parent_id(parent);
                if matches!(model.nodes.kind(statement), AstKind::VariableDeclaration(_))
                    && model.nodes.parent_id(statement) == container
                    && alias.init.as_ref().is_some_and(|value| {
                        matches!(value, Expression::Identifier(_))
                            && value.span() == model.nodes.kind(node).span()
                    })
                {
                    return q::plain_binding(model, alias).is_some_and(|alias_symbol| {
                        exposures_after(
                            model,
                            container,
                            ready_end.max(alias.span.end),
                            alias_symbol,
                            None,
                            visiting,
                            budget,
                        )
                    });
                }
            }

            // Recovered `let alias; ...; alias = callable` is inert too, but
            // only with a sole direct store and a prior plain declaration in
            // this same activation. The current occurrence must be the exact
            // RHS value; an argument in `callable(callable)` is not an alias
            // store. All alias reads must follow both this store and setup.
            if let AstKind::AssignmentExpression(assignment) = model.nodes.kind(parent) {
                let statement = model.nodes.parent_id(parent);
                if assignment.operator.as_str() == "="
                    && matches!(&assignment.right, Expression::Identifier(_))
                    && assignment.right.span() == model.nodes.kind(node).span()
                    && matches!(model.nodes.kind(statement), AstKind::ExpressionStatement(_))
                    && model.nodes.parent_id(statement) == container
                {
                    let Some(alias) = q::target_symbol(model, assignment) else { return false; };
                    let declaration = model.scoping.symbol_declaration(alias);
                    let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else { return false; };
                    let declaration_statement = model.nodes.parent_id(declaration);
                    if variable.init.is_some()
                        || q::plain_binding(model, variable) != Some(alias)
                        || !matches!(model.nodes.kind(declaration_statement), AstKind::VariableDeclaration(_))
                        || model.nodes.parent_id(declaration_statement) != container
                        || model.nodes.kind(declaration_statement).span().end > model.nodes.kind(statement).span().start
                    { return false; }
                    let Some(write) = model.scoping.get_resolved_references(alias).find(|r| r.is_write()) else { return false; };
                    if !matches!(model.nodes.parent_kind(write.node_id()), AstKind::AssignmentExpression(store) if store.span == assignment.span) { return false; }
                    return exposures_after(model, container, ready_end.max(assignment.span.end), alias, Some(write.node_id()), visiting, budget);
                }
            }

            let mut child = node;
            for _ in 0..128 {
                let parent = model.nodes.parent_id(child);
                if matches!(
                    model.nodes.kind(child),
                    AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
                ) {
                    let declaration_binding = match model.nodes.kind(child) {
                        AstKind::Function(function) if parent == container => function
                            .id
                            .as_ref()
                            .and_then(|id| id.symbol_id.get())
                            .map(|id| (id, ready_end)),
                        _ => None,
                    };
                    let expression_binding = if let AstKind::VariableDeclarator(variable) =
                        model.nodes.kind(parent)
                    {
                        let statement = model.nodes.parent_id(parent);
                        if matches!(model.nodes.kind(statement), AstKind::VariableDeclaration(_))
                            && model.nodes.parent_id(statement) == container
                            && variable.init.as_ref().is_some_and(|value| {
                                matches!(
                                    value,
                                    Expression::FunctionExpression(_)
                                        | Expression::ArrowFunctionExpression(_)
                                ) && value.span() == model.nodes.kind(child).span()
                            })
                        {
                            q::plain_binding(model, variable)
                                .map(|id| (id, ready_end.max(variable.span.end)))
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    if let Some((binding, end)) = declaration_binding.or(expression_binding) {
                        return exposures_after(model, container, end, binding, None, visiting, budget);
                    }
                    if parent == container {
                        return false;
                    }
                }
                if parent == container {
                    // No hoisted declaration is left to cross here. Reuse the
                    // existing same-statement-container order predicate for this
                    // ordinary exposure, including unknown consumer arguments.
                    return crate::naming::shared::callable_origin_precedes(
                        model, container, ready_end, node, None, 0, false,
                    );
                }
                if parent == child {
                    return false;
                }
                child = parent;
            }
            false
        });
    visiting.remove(&symbol);
    any && result
}

/// Locate the exact contiguous native accessor construction preceding the
/// captured alias store. Neither an unused descriptor nor a sibling helper can
/// qualify. Both descriptor and fallback branches are proved completely.
fn native_accessors(
    model: &Model<'_, '_>,
    site: FunctionSite,
    id: S,
    defaults: S,
    cache: S,
) -> Option<()> {
    let value = captured(model, site, id)?;
    let root = q::symbol(model, value)?;
    if model.binding_writes(root) != 0 {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(root);
    if model.enclosing_function(declaration).map(|f| f.span)
        != model.enclosing_function(site.node_id).map(|f| f.span)
    {
        return None;
    }
    let parent = model.nodes.parent_id(model.nodes.parent_id(declaration));
    let statements = match model.nodes.kind(parent) {
        AstKind::FunctionBody(b) => b.statements.as_slice(),
        AstKind::BlockStatement(b) => b.body.as_slice(),
        _ => return None,
    };
    let at = statements.iter().position(|st| {
        q::one_variable(st).and_then(|v| q::plain_binding(model, v)) == Some(root)
    })?;
    let [root_st, descriptor_st, setter_st, conditional, set_store, alias_store] =
        statements.get(at..at + 6)?
    else {
        return None;
    };
    // The same activation constructs the defaults, accessor pair and cache in
    // that order. A single assignment hidden in an unused child is no origin.
    let defaults_store = q::assignment_statement(statements.get(at.checked_sub(1)?)?)?;
    if defaults_store.operator.as_str() != "="
        || q::target_symbol(model, defaults_store) != Some(defaults)
        || defaults_store.right.span() != captured(model, site, defaults)?.span()
    {
        return None;
    }
    assign(model, statements.get(at + 6)?, cache, null_record())?;
    let (_, root_init) = variable(model, root_st)?;
    if !eq(model, root_init, null_record()) {
        return None;
    }
    let (descriptor, desc_init) = variable(model, descriptor_st)?;
    if model.binding_writes(descriptor) != 0
        || !eq(
            model,
            desc_init,
            c(
                f(g("Object"), "getOwnPropertyDescriptor"),
                vec![
                    f(f(g("window"), "HTMLAnchorElement"), "prototype"),
                    t("href"),
                ],
            ),
        )
    {
        return None;
    }
    let setter_decl = q::one_variable(setter_st)?;
    let setter = q::plain_binding(model, setter_decl)?;
    if setter_decl.init.is_some() || model.binding_writes(setter) != 2 {
        return None;
    }
    let test = op(
        "&&",
        op(
            "&&",
            op("!=", s(descriptor), E::Null),
            op("!=", f(s(descriptor), "get"), E::Null),
        ),
        op("!=", f(s(descriptor), "set"), E::Null),
    );
    let (yes, Some(no)) = branch(model, conditional, test)? else {
        return None;
    };
    let [get_st, set_st] = yes else {
        return None;
    };
    field_assign(
        model,
        get_st,
        f(s(root), "anchorHrefGet"),
        c(
            f(f(g("Object"), "call"), "bind"),
            vec![f(s(descriptor), "get")],
        ),
    )?;
    assign(
        model,
        set_st,
        setter,
        c(
            f(f(g("Object"), "call"), "bind"),
            vec![f(s(descriptor), "set")],
        ),
    )?;
    let [Statement::FunctionDeclaration(get), Statement::FunctionDeclaration(set), get_st, set_st] =
        no
    else {
        return None;
    };
    let get_id = get.id.as_ref()?.symbol_id.get()?;
    let set_id = set.id.as_ref()?.symbol_id.get()?;
    if model.binding_writes(get_id) != 0
        || model.binding_writes(set_id) != 0
        || get.r#async
        || get.generator
        || set.r#async
        || set.generator
    {
        return None;
    }
    let get_params = q::plain_formals(model, &get.params)?;
    let set_params = q::plain_formals(model, &set.params)?;
    let [a] = get_params.as_slice() else {
        return None;
    };
    let [b, v] = set_params.as_slice() else {
        return None;
    };
    let gb = get.body.as_ref()?;
    let sb = set.body.as_ref()?;
    if !gb.directives.is_empty() || !sb.directives.is_empty() {
        return None;
    }
    let [Statement::ReturnStatement(ret)] = gb.statements.as_slice() else {
        return None;
    };
    if !eq(model, ret.argument.as_ref()?, f(s(*a), "href")) {
        return None;
    }
    let [write] = sb.statements.as_slice() else {
        return None;
    };
    field_assign(model, write, f(s(*b), "href"), s(*v))?;
    field_assign(model, get_st, f(s(root), "anchorHrefGet"), s(get_id))?;
    assign(model, set_st, setter, s(set_id))?;
    field_assign(model, set_store, f(s(root), "anchorHrefSet"), s(setter))?;
    assign(model, alias_store, id, s(root))?;
    // Root uses are confined to construction and its final captured alias.
    // This rules out hidden mutation and escape through a computed member,
    // unknown consumer or unused closure just as for a direct member store.
    for reference in model.scoping.get_resolved_references(root) {
        let span = model.nodes.get_node(reference.node_id()).kind().span();
        if span.start < conditional.span().start || span.end > alias_store.span().end {
            return None;
        }
    }
    // All consumer aliases are immutable, and their entire use contract is a
    // direct call of one of the two constructed accessors. No member mutation,
    // object escape or same-spelled donor can change the captured contract.
    for reference in model.scoping.get_resolved_references(id) {
        if reference.is_write() {
            continue;
        }
        let AstKind::VariableDeclarator(alias) = model.nodes.parent_kind(reference.node_id())
        else {
            return None;
        };
        let alias = q::plain_binding(model, alias)?;
        if model.binding_writes(alias) != 0 {
            return None;
        }
        for usage in model.scoping.get_resolved_references(alias) {
            let AstKind::StaticMemberExpression(m) = model.nodes.parent_kind(usage.node_id())
            else {
                return None;
            };
            if m.optional || !matches!(m.property.name.as_str(), "anchorHrefGet" | "anchorHrefSet")
            {
                return None;
            }
            let AstKind::CallExpression(call) = model
                .nodes
                .parent_kind(model.nodes.parent_id(usage.node_id()))
            else {
                return None;
            };
            if call.optional || call.callee.span() != m.span {
                return None;
            }
        }
    }
    // All required stores are in this same statement list and complete by the
    // fresh cache store. A stable symbol_value and source adjacency alone do
    // not establish their availability when this candidate actually executes.
    let ready_end = statements.get(at + 6)?.span().end;
    exposures_after(
        model,
        parent,
        ready_end,
        site.binding_symbol?,
        None,
        &mut BTreeSet::new(),
        &mut 256,
    )
    .then_some(())
}

/// Default ports may be read through computed protocol lookups, but may not
/// escape or acquire a second property origin, even from an unused child.
fn immutable_defaults(model: &Model<'_, '_>, id: S) -> Option<()> {
    for reference in model.scoping.get_resolved_references(id) {
        if reference.is_write() {
            continue;
        }
        let node = reference.node_id();
        let AstKind::ComputedMemberExpression(member) = model.nodes.parent_kind(node) else {
            return None;
        };
        if member.optional || q::symbol(model, &member.object) != Some(id) {
            return None;
        }
        let parent = model.nodes.parent_kind(model.nodes.parent_id(node));
        if matches!(
            parent,
            AstKind::AssignmentExpression(_)
                | AstKind::UpdateExpression(_)
                | AstKind::UnaryExpression(_)
        ) {
            return None;
        }
    }
    Some(())
}

fn encoded_target(model: &Model<'_, '_>, st: &Statement<'_>) -> Option<S> {
    if let Statement::IfStatement(b) = st {
        let [first] = q::block(&b.consequent)? else {
            return None;
        };
        return q::target_symbol(model, q::assignment_statement(first)?);
    }
    q::target_symbol(model, q::assignment_statement(st)?)
}
fn encode_assignment(model: &Model<'_, '_>, st: &Statement<'_>, id: S, formal: S) -> Option<usize> {
    if let Statement::IfStatement(b) = st {
        if !q::is_typeof_undefined(model, &b.test, q::ValueRef::Param(formal)) {
            return None;
        }
        let [undefined] = q::block(&b.consequent)? else {
            return None;
        };
        assign(model, undefined, id, t("undefined"))?;
        let Some(Statement::IfStatement(null)) = b.alternate.as_ref() else {
            return None;
        };
        if !q::is_null_test(model, &null.test, q::ValueRef::Param(formal)) {
            return None;
        }
        let [null_st] = q::block(&null.consequent)? else {
            return None;
        };
        assign(model, null_st, id, t("null"))?;
        let [encoded] = q::block(null.alternate.as_ref()?)? else {
            return None;
        };
        let a = q::assignment_statement(encoded)?;
        if a.operator.as_str() != "="
            || q::target_symbol(model, a) != Some(id)
            || !q::guarded_expr(
                model,
                &a.right,
                q::ValueRef::Param(formal),
                q::Direction::Encode,
            )
        {
            return None;
        }
        return Some(3);
    }
    let a = q::assignment_statement(st)?;
    (a.operator.as_str() == "="
        && q::target_symbol(model, a) == Some(id)
        && q::guarded_expr(
            model,
            &a.right,
            q::ValueRef::Param(formal),
            q::Direction::Encode,
        ))
    .then_some(1)
}
fn query(
    model: &Model<'_, '_>,
    site: FunctionSite,
    sts: &[Statement<'_>],
    input: S,
    name: S,
    value: S,
    query: S,
    encoded_name: S,
    encoded_value: S,
    found: S,
) -> Option<(usize, usize)> {
    let [entries_st, parse_st, name_st, value_st, Statement::IfStatement(merge)] = sts else {
        return None;
    };
    let entries_decl = q::one_variable(entries_st)?;
    let entries = q::plain_binding(model, entries_decl)?;
    if entries_decl.init.is_some() || !q::body_local(model, site, entries) {
        return None;
    }
    let Statement::IfStatement(parse) = parse_st else {
        return None;
    };
    if !q::input_guard(model, &parse.test, input) {
        return None;
    }
    let [empty] = q::block(&parse.consequent)? else {
        return None;
    };
    if !q::entries_empty_assignment(model, empty, entries) {
        return None;
    }
    q::query_parse(
        model,
        site,
        input,
        entries,
        q::block(parse.alternate.as_ref()?)?,
    )?;
    let name_writes = encode_assignment(model, name_st, encoded_name, name)?;
    let value_writes = encode_assignment(model, value_st, encoded_value, value)?;
    if !eq(model, &merge.test, nonempty(s(entries))) {
        return None;
    }
    let [found_st, each_st, append_st] = q::block(&merge.consequent)? else {
        return None;
    };
    let a = q::assignment_statement(found_st)?;
    if a.operator.as_str() != "="
        || q::target_symbol(model, a) != Some(found)
        || !q::boolean(&a.right, false)
    {
        return None;
    }
    let index = q::for_each_callback(
        model,
        site,
        each_st,
        entries,
        query,
        found,
        encoded_name,
        encoded_value,
    )?;
    let Statement::IfStatement(append) = append_st else {
        return None;
    };
    let Expression::UnaryExpression(not) = append.test.without_parentheses() else {
        return None;
    };
    if not.operator.as_str() != "!"
        || q::symbol(model, &not.argument) != Some(found)
        || append.alternate.is_some()
    {
        return None;
    }
    let [append] = q::block(&append.consequent)? else {
        return None;
    };
    let a = q::assignment_statement(append)?;
    use q::Part::{Sym, Text};
    if a.operator.as_str() != "="
        || q::target_symbol(model, a) != Some(query)
        || q::parts(model, &a.right, index, 0)?
            != vec![
                Sym(query),
                Text("&".into()),
                Sym(encoded_name),
                Text("=".into()),
                Sym(encoded_value),
            ]
    {
        return None;
    }
    let [empty] = q::block(merge.alternate.as_ref()?)? else {
        return None;
    };
    let a = q::assignment_statement(empty)?;
    if a.operator.as_str() != "="
        || q::target_symbol(model, a) != Some(query)
        || q::parts(model, &a.right, index, 0)?
            != vec![Sym(encoded_name), Text("=".into()), Sym(encoded_value)]
    {
        return None;
    }
    Some((name_writes, value_writes))
}

fn returned(model: &Model<'_, '_>, st: &Statement<'_>, expected: E) -> Option<()> {
    let Statement::ReturnStatement(r) = st else {
        return None;
    };
    eq(model, r.argument.as_ref()?, expected).then_some(())
}
fn rebuilt(protocol: E, host: E, port: E, path: E, query: E, hash: E) -> E {
    sum(vec![
        protocol,
        t("://"),
        host,
        optional(port, ":"),
        path,
        optional(query, "?"),
        optional(hash, "#"),
    ])
}
const PATH_PATTERN: &str = "(%(2[146-9a-f]|3[0-9abd]|4[0-9a-f]|5[0-9abdf]|6[1-9a-f]|7[0-9ae]))";
fn decoded(anchor: S, key: &str) -> E {
    c(
        f(f(s(anchor), key), "replace"),
        vec![c(g("RegExp"), vec![t(PATH_PATTERN), t("gi")]), E::Decode],
    )
}
fn slice1(e: E) -> E {
    c(f(e, "slice"), vec![E::N(1)])
}
fn port(anchor: S, defaults: S, protocol: S) -> E {
    choice(
        op("===", f(s(anchor), "port"), ix(s(defaults), s(protocol))),
        t(""),
        f(s(anchor), "port"),
    )
}

fn rebuild(model: &Model<'_, '_>, site: FunctionSite, function: &Function<'_>) -> Option<()> {
    if function.r#async || function.generator {
        return None;
    }
    let body = function.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let params = q::plain_formals(model, &function.params)?;
    let [input, name, value] = params.as_slice() else {
        return None;
    };
    let sts = body.statements.as_slice();
    // Exact statement ownership and order; no scan for a bag of member names.
    let [temps_st, query_st, url_st, http_st, cache_st, native_st, anchor_st, set_st, repair_st, protocol_st, path_st, path_if, search_st, hash_st, parsed_st, after_parsed @ ..] =
        sts
    else {
        return None;
    };
    let Statement::VariableDeclaration(temps) = temps_st else {
        return None;
    };
    if temps.declarations.len() != 3 || temps.declarations.iter().any(|v| v.init.is_some()) {
        return None;
    }
    let mut temp_ids = BTreeSet::new();
    for v in &temps.declarations {
        let id = q::plain_binding(model, v)?;
        if !q::body_local(model, site, id) || !temp_ids.insert(id) {
            return None;
        }
    }
    let (query_id, query_init) = variable(model, query_st)?;
    if !q::body_local(model, site, query_id) || !q::text(query_init, "") {
        return None;
    }
    let (url, url_init) = variable(model, url_st)?;
    if !q::body_local(model, site, url)
        || model.binding_writes(url) != 1
        || !eq(
            model,
            url_init,
            op("||", s(*input), plus(f(g("window"), "location"), t(""))),
        )
    {
        return None;
    }
    let http = local(
        model,
        site,
        http_st,
        c(
            f(c(g("RegExp"), vec![t("^https?:\\/\\/")]), "test"),
            vec![s(url)],
        ),
    )?;
    let (cached, none) = branch(model, cache_st, s(http))?;
    if none.is_some() {
        return None;
    }
    let [Statement::IfStatement(hit)] = cached else {
        return None;
    };
    if hit.alternate.is_some() {
        return None;
    }
    let test = q::binary(&hit.test)?;
    if test.operator.as_str() != "!="
        || !matches!(test.right.without_parentheses(), Expression::NullLiteral(_))
    {
        return None;
    }
    let Expression::ComputedMemberExpression(lookup) = test.left.without_parentheses() else {
        return None;
    };
    let cache = capture_value(model, site, &lookup.object)?;
    if lookup.optional
        || q::symbol(model, &lookup.expression) != Some(url)
        || !eq(model, captured(model, site, cache)?, null_record())
    {
        return None;
    }
    let cached = q::block(&hit.consequent)?;
    let [p_st, h_st, port_st, path_cached_st, hash_cached_st, merge_tail @ ..] = cached else {
        return None;
    };
    let lookup = ix(s(cache), s(url));
    let cp = local(model, site, p_st, f(lookup.clone(), "protocol"))?;
    let ch = local(model, site, h_st, f(lookup.clone(), "hostname"))?;
    let cport = local(model, site, port_st, f(lookup.clone(), "port"))?;
    let cpath = local(model, site, path_cached_st, f(lookup.clone(), "pathname"))?;
    assign(model, hash_cached_st, url, f(lookup, "hash"))?;
    let [_, _, name_st, value_st, _, ret] = merge_tail else {
        return None;
    };
    let encoded_name = encoded_target(model, name_st)?;
    let encoded_value = encoded_target(model, value_st)?;
    let Statement::IfStatement(m) = &merge_tail[4] else {
        return None;
    };
    let found = q::target_symbol(
        model,
        q::assignment_statement(q::block(&m.consequent)?.first()?)?,
    )?;
    if BTreeSet::from([encoded_name, encoded_value, found]) != temp_ids
        || model.binding_writes(found) != 4
    {
        return None;
    }
    let first_writes = query(
        model,
        site,
        &merge_tail[..5],
        *input,
        *name,
        *value,
        query_id,
        encoded_name,
        encoded_value,
        found,
    )?;
    returned(
        model,
        ret,
        rebuilt(s(cp), s(ch), s(cport), s(cpath), s(query_id), s(url)),
    )?;

    let (native, native_init) = variable(model, native_st)?;
    let native_origin = capture_value(model, site, native_init)?;
    if !q::body_local(model, site, native) || model.binding_writes(native) != 0 {
        return None;
    }
    let anchor = local(
        model,
        site,
        anchor_st,
        c(f(g("document"), "createElement"), vec![t("a")]),
    )?;
    invoke(
        model,
        set_st,
        c(f(s(native), "anchorHrefSet"), vec![s(anchor), s(url)]),
    )?;
    let (repair, no) = branch(model, repair_st, op("===", f(s(anchor), "host"), t("")))?;
    let [repair] = repair else {
        return None;
    };
    if no.is_some() {
        return None;
    }
    invoke(
        model,
        repair,
        c(
            f(s(native), "anchorHrefSet"),
            vec![s(anchor), c(f(s(native), "anchorHrefGet"), vec![s(anchor)])],
        ),
    )?;
    let protocol = local(
        model,
        site,
        protocol_st,
        c(
            f(f(s(anchor), "protocol"), "slice"),
            vec![
                E::N(0),
                op("-", f(f(s(anchor), "protocol"), "length"), E::N(1)),
            ],
        ),
    )?;
    let path_decl = q::one_variable(path_st)?;
    let path = q::plain_binding(model, path_decl)?;
    if path_decl.init.is_some()
        || !q::body_local(model, site, path)
        || model.binding_writes(path) != 2
    {
        return None;
    }
    let (yes, Some(no)) = branch(
        model,
        path_if,
        op("===", ix(f(s(anchor), "pathname"), E::N(0)), t("/")),
    )?
    else {
        return None;
    };
    let [yes] = yes else {
        return None;
    };
    let [no] = no else {
        return None;
    };
    assign(model, yes, path, decoded(anchor, "pathname"))?;
    assign(model, no, path, plus(t("/"), decoded(anchor, "pathname")))?;
    let search = local(model, site, search_st, slice1(decoded(anchor, "search")))?;
    let hash = local(model, site, hash_st, slice1(decoded(anchor, "hash")))?;
    let (parsed, parsed_init) = variable(model, parsed_st)?;
    if !q::body_local(model, site, parsed) || model.binding_writes(parsed) != 0 {
        return None;
    }
    let Expression::ObjectExpression(object) = parsed_init.without_parentheses() else {
        return None;
    };
    if !matches!(object.properties.len(), 4 | 6) {
        return None;
    }
    let mut fields = BTreeMap::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(p) = property else {
            return None;
        };
        if p.computed || p.method || p.kind != PropertyKind::Init {
            return None;
        }
        if fields
            .insert(property_key_name(&p.key)?, &p.value)
            .is_some()
        {
            return None;
        }
    }
    // Both lowerings construct exactly the same six-field parsed record. The
    // split form must write search then hash directly, before href or exposure;
    // no calls, aliases, computed keys or extra stores can hide in this prefix.
    let (href_st, cache_write, tail) = if object.properties.len() == 4 {
        let [search_store, hash_store, href_st, cache_write, tail @ ..] = after_parsed else { return None; };
        if fields.keys().map(String::as_str).collect::<BTreeSet<_>>()
            != BTreeSet::from(["protocol", "hostname", "port", "pathname"])
        { return None; }
        field_assign(model, search_store, f(s(parsed), "search"), s(search))?;
        field_assign(model, hash_store, f(s(parsed), "hash"), s(hash))?;
        (href_st, cache_write, tail)
    } else {
        let [href_st, cache_write, tail @ ..] = after_parsed else { return None; };
        (href_st, cache_write, tail)
    };
    let port_expr = *fields.get("port")?;
    let Expression::ConditionalExpression(port_cond) = port_expr.without_parentheses() else {
        return None;
    };
    let port_test = q::binary(&port_cond.test)?;
    let Expression::ComputedMemberExpression(default_lookup) =
        port_test.right.without_parentheses()
    else {
        return None;
    };
    let defaults = capture_value(model, site, &default_lookup.object)?;
    immutable_defaults(model, defaults)?;
    native_accessors(model, site, native_origin, defaults, cache)?;
    let Expression::ObjectExpression(default_obj) =
        captured(model, site, defaults)?.without_parentheses()
    else {
        return None;
    };
    let mut defaults_values = BTreeMap::new();
    for p in &default_obj.properties {
        let ObjectPropertyKind::ObjectProperty(p) = p else {
            return None;
        };
        if p.computed || p.method || p.kind != PropertyKind::Init {
            return None;
        }
        if defaults_values
            .insert(property_key_name(&p.key)?, expr(model, &p.value, 0)?)
            .is_some()
        {
            return None;
        }
    }
    if defaults_values
        != BTreeMap::from([
            ("ftp".into(), t("21")),
            ("gopher".into(), t("70")),
            ("http".into(), t("80")),
            ("https".into(), t("443")),
            ("ws".into(), t("80")),
            ("wss".into(), t("443")),
        ])
    {
        return None;
    }
    for (key, expected) in [
        ("protocol", s(protocol)),
        ("hostname", f(s(anchor), "hostname")),
        ("port", port(anchor, defaults, protocol)),
        ("pathname", s(path)),
        ("search", s(search)),
        ("hash", s(hash)),
    ] {
        if object.properties.len() == 4 && matches!(key, "search" | "hash") { continue; }
        if !eq(model, fields.get(key)?, expected) {
            return None;
        }
    }
    field_assign(
        model,
        href_st,
        f(s(parsed), "href"),
        rebuilt(
            s(protocol),
            f(s(anchor), "hostname"),
            port(anchor, defaults, protocol),
            s(path),
            s(search),
            s(hash),
        ),
    )?;
    let (write, no) = branch(model, cache_write, s(http))?;
    let [write] = write else {
        return None;
    };
    if no.is_some() {
        return None;
    }
    let a = q::assignment_statement(write)?;
    let AssignmentTarget::ComputedMemberExpression(target) = &a.left else {
        return None;
    };
    if a.operator.as_str() != "="
        || target.optional
        || !eq(model, &target.object, s(cache))
        || !eq(model, &target.expression, s(url))
        || !eq(model, &a.right, s(parsed))
    {
        return None;
    }
    let second_writes = query(
        model,
        site,
        tail.get(..5)?,
        *input,
        *name,
        *value,
        query_id,
        encoded_name,
        encoded_value,
        found,
    )?;
    if model.binding_writes(encoded_name) != first_writes.0 + second_writes.0
        || model.binding_writes(encoded_value) != first_writes.1 + second_writes.1
    {
        return None;
    }
    let expected = rebuilt(
        f(s(parsed), "protocol"),
        f(s(parsed), "hostname"),
        f(s(parsed), "port"),
        f(s(parsed), "pathname"),
        s(query_id),
        f(s(parsed), "hash"),
    );
    match tail.get(5..)? {
        [ret] => returned(model, ret, expected)?,
        [port_st, port_if, ret] => {
            let d = q::one_variable(port_st)?;
            let port_id = q::plain_binding(model, d)?;
            if d.init.is_some()
                || !q::body_local(model, site, port_id)
                || model.binding_writes(port_id) != 2
            {
                return None;
            }
            let Statement::IfStatement(b) = port_if else {
                return None;
            };
            let (key, prefix) = if eq(model, &b.test, nonempty(f(s(parsed), "port"))) {
                ("port", ":")
            } else if eq(model, &b.test, nonempty(f(s(parsed), "hash"))) {
                ("hash", "#")
            } else {
                return None;
            };
            let (yes, Some(no)) = branch(model, port_if, nonempty(f(s(parsed), key)))? else {
                return None;
            };
            let [yes] = yes else {
                return None;
            };
            let [no] = no else {
                return None;
            };
            assign(model, yes, port_id, plus(t(prefix), f(s(parsed), key)))?;
            assign(model, no, port_id, t(""))?;
            let port = if key == "port" {
                s(port_id)
            } else {
                optional(f(s(parsed), "port"), ":")
            };
            let hash = if key == "hash" {
                s(port_id)
            } else {
                optional(f(s(parsed), "hash"), "#")
            };
            returned(
                model,
                ret,
                sum(vec![
                    f(s(parsed), "protocol"),
                    t("://"),
                    f(s(parsed), "hostname"),
                    port,
                    f(s(parsed), "pathname"),
                    optional(s(query_id), "?"),
                    hash,
                ]),
            )?;
        }
        _ => return None,
    }
    // Every mutable local above has exactly its reviewed writes. The query
    // accumulator has three per path (callback replace/retain, append, empty).
    if model.binding_writes(query_id) != 8 {
        return None;
    }
    Some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
    _: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return vec![];
    };
    if rebuild(model, site, function).is_none() {
        return vec![];
    }
    vec![proposal("rebuildUrlWithEncodedQuery","thread_url_query_rebuild","complete owned cache-hit and anchor-normalization paths: stable scoped cache, native href getter/setter construction and default-port origins; HTTP(S) guard; own query parsing and inline pair decode/encode callbacks; both formal components encoded with encodeURIComponent and %20-to-plus replacement; ordered protocol/hostname/optional-port/path/query/hash return on both paths; every candidate value exposure follows completion of all three capture stores through bounded immutable alias and actual hoisted-caller exposure chains",facts)]
}
