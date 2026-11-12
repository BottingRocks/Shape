//! Complete, capture-free protocols. Every statement is consumed in order;
//! callback receivers, arguments, stores and returns use exact SymbolIds.
use crate::naming::*;
use oxc_semantic::SymbolId;

type Sym = SymbolId;
fn sym(m: &Model<'_, '_>, e: &Expression<'_>) -> Option<Sym> {
    crate::analysis::expression_symbol(e.without_parentheses(), m.scoping)
}
fn binding(m: &Model<'_, '_>, p: &BindingPattern<'_>, writes: usize) -> Option<Sym> {
    let BindingPattern::BindingIdentifier(id) = p else {
        return None;
    };
    let s = id.symbol_id.get()?;
    (m.binding_writes(s) == writes && m.scoping.symbol_redeclarations(s).is_empty()).then_some(s)
}
fn params(m: &Model<'_, '_>, p: &FormalParameters<'_>, n: usize) -> Option<Vec<Sym>> {
    if p.rest.is_some() || p.items.len() != n {
        return None;
    }
    p.items
        .iter()
        .map(|p| {
            if p.initializer.is_some() {
                return None;
            }
            binding(m, &p.pattern, 0)
        })
        .collect()
}
fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(x) = e.without_parentheses() else {
        return None;
    };
    (!x.optional && x.property.name == key).then_some(&x.object)
}
fn call<'a>(e: &'a Expression<'a>, n: usize) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(x) = e.without_parentheses() else {
        return None;
    };
    (!x.optional
        && x.arguments.len() == n
        && x.arguments
            .iter()
            .all(|a| !matches!(a, Argument::SpreadElement(_))))
    .then_some(x)
}
fn arg<'a>(c: &'a CallExpression<'a>, i: usize) -> Option<&'a Expression<'a>> {
    c.arguments.get(i)?.as_expression()
}
fn expr<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    Some(&s.expression)
}
fn ret<'a>(s: &'a Statement<'a>) -> Option<&'a Expression<'a>> {
    let Statement::ReturnStatement(s) = s else {
        return None;
    };
    s.argument.as_ref()
}
fn decl<'a>(s: &'a Statement<'a>) -> Option<&'a VariableDeclarator<'a>> {
    let Statement::VariableDeclaration(d) = s else {
        return None;
    };
    let [v] = d.declarations.as_slice() else {
        return None;
    };
    Some(v)
}
fn assignment<'a>(s: &'a Statement<'a>) -> Option<&'a AssignmentExpression<'a>> {
    let Expression::AssignmentExpression(a) = expr(s)?.without_parentheses() else {
        return None;
    };
    (a.operator.as_str() == "=").then_some(a)
}
fn global(m: &Model<'_, '_>, e: &Expression<'_>, name: &str) -> bool {
    matches!(e.without_parentheses(), Expression::Identifier(i) if i.name == name && i.reference_id.get().is_some_and(|r|m.scoping.get_reference(r).symbol_id().is_none()))
}
fn arrow<'a>(c: &'a CallExpression<'a>) -> Option<&'a ArrowFunctionExpression<'a>> {
    let Expression::ArrowFunctionExpression(a) = arg(c, 0)?.without_parentheses() else {
        return None;
    };
    (!a.r#async && a.body.directives.is_empty()).then_some(a)
}
fn block<'a>(s: &'a Statement<'a>) -> Option<&'a [Statement<'a>]> {
    let Statement::BlockStatement(b) = s else {
        return None;
    };
    Some(&b.body)
}
fn string(e: &Expression<'_>, expected: &str) -> bool {
    matches!(e.without_parentheses(),Expression::StringLiteral(s) if s.value==expected)
}
fn number(e: &Expression<'_>, expected: f64) -> bool {
    matches!(e.without_parentheses(),Expression::NumericLiteral(n) if n.value==expected)
}
fn fields<'a>(e: &'a Expression<'a>, keys: &[&str]) -> Option<Vec<&'a Expression<'a>>> {
    let Expression::ObjectExpression(o) = e.without_parentheses() else {
        return None;
    };
    if o.properties.len() != keys.len() {
        return None;
    }
    o.properties
        .iter()
        .zip(keys)
        .map(|(p, k)| {
            let ObjectPropertyKind::ObjectProperty(p) = p else {
                return None;
            };
            if p.computed
                || p.method
                || p.shorthand
                || p.kind != PropertyKind::Init
                || property_key_name(&p.key).as_deref() != Some(*k)
            {
                return None;
            }
            Some(&p.value)
        })
        .collect()
}
fn local_member(m: &Model<'_, '_>, e: &Expression<'_>, object: Sym, key: &str) -> bool {
    member(e, key).and_then(|e| sym(m, e)) == Some(object)
}
fn store_member<'a>(
    m: &Model<'_, '_>,
    s: &'a Statement<'a>,
    object: Sym,
    key: &str,
) -> Option<&'a Expression<'a>> {
    let a = assignment(s)?;
    let AssignmentTarget::StaticMemberExpression(left) = &a.left else {
        return None;
    };
    (!left.optional && left.property.name == key && sym(m, &left.object) == Some(object))
        .then_some(&a.right)
}
fn url_text(m: &Model<'_, '_>, e: &Expression<'_>, attribute: Sym) -> Option<()> {
    let slice = call(e, 2)?;
    let replace = call(member(&slice.callee, "slice")?, 2)?;
    if !number(arg(slice, 0)?, 0.0)
        || !number(arg(slice, 1)?, 99.0)
        || !local_member(m, member(&replace.callee, "replace")?, attribute, "url")
        || !string(arg(replace, 1)?, "")
    {
        return None;
    }
    let regex = call(arg(replace, 0)?, 1)?;
    if !global(m, &regex.callee, "RegExp") || !string(arg(regex, 0)?, r"^http(s)?:\/\/") {
        return None;
    }
    Some(())
}
fn memory_formatter(m: &Model<'_, '_>, p: &[Sym], body: &[Statement<'_>]) -> Option<()> {
    let [s] = body else {
        return None;
    };
    let outer = call(ret(s)?, 1)?;
    if member(&outer.callee, "map").and_then(|e| sym(m, e)) != Some(p[0]) {
        return None;
    }
    let entry = arrow(outer)?;
    if entry.expression {
        return None;
    }
    let ep = params(m, &entry.params, 1)?[0];
    let [init, attributes, types, _end] = entry.body.statements.as_slice() else {
        return None;
    };
    let d = decl(init)?;
    let result = binding(m, &d.id, 0)?;
    let f = fields(d.init.as_ref()?, &["e"])?;
    if !local_member(m, f[0], ep, "bytes") {
        return None;
    }
    let inner = call(store_member(m, attributes, result, "f")?, 1)?;
    if !local_member(m, member(&inner.callee, "map")?, ep, "attribution") {
        return None;
    }
    let a = arrow(inner)?;
    if a.expression {
        return None;
    }
    let ap = params(m, &a.params, 1)?[0];
    let [init, branch, end] = a.body.statements.as_slice() else {
        return None;
    };
    let d = decl(init)?;
    let attribute = binding(m, &d.id, 0)?;
    let f = fields(d.init.as_ref()?, &["h", "i"])?;
    url_text(m, f[0], ap)?;
    if !local_member(m, f[1], ap, "scope") {
        return None;
    }
    let Statement::IfStatement(branch) = branch else {
        return None;
    };
    let Expression::BinaryExpression(test) = branch.test.without_parentheses() else {
        return None;
    };
    if test.operator.as_str() != "!="
        || !local_member(m, &test.left, ap, "container")
        || !matches!(test.right, Expression::NullLiteral(_))
        || branch.alternate.is_some()
    {
        return None;
    }
    let setup = block(&branch.consequent)?;
    let object = match setup {
        [store] => store_member(m, store, attribute, "j")?,
        [init, store] => {
            let d = decl(init)?;
            let container = binding(m, &d.id, 0)?;
            if sym(m, store_member(m, store, attribute, "j")?) != Some(container) {
                return None;
            }
            d.init.as_ref()?
        }
        _ => return None,
    };
    let f = fields(object, &["k", "l"])?;
    if !member(f[0], "id").is_some_and(|e| local_member(m, e, ap, "container"))
        || !member(f[1], "src").is_some_and(|e| local_member(m, e, ap, "container"))
        || sym(m, ret(end)?) != Some(attribute)
    {
        return None;
    }
    if !local_member(m, store_member(m, types, result, "g")?, ep, "types") {
        return None;
    }
    let [_, _, _, end] = entry.body.statements.as_slice() else {
        return None;
    };
    if sym(m, ret(end)?) != Some(result) {
        return None;
    }
    Some(())
}

pub(in crate::naming) fn evaluate(
    m: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = m.nodes.kind(site.node_id) else {
        return vec![];
    };
    let Some(body) = &function.body else {
        return vec![];
    };
    if function.r#async || function.generator || !body.directives.is_empty() {
        return vec![];
    }
    let mut out = vec![];
    if let Some(p) = params(m, &function.params, 1) {
        if memory_formatter(m, &p, &body.statements).is_some() {
            out.push(proposal("formatMemoryAttributionBreakdown","memory_attribution_formatter","complete capture-free nested map: bytes->e, attribution->f, types->g; each attribution maps url.replace(RegExp('^http(s)?:\\/\\/'), '').slice(0,99)->h, scope->i, and iff container != null its id/src->j.k/l; exact order, callback identities and returns",f));
        }
    }
    out
}
