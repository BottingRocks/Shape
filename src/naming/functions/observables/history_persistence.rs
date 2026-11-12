//! Bounded history persistence with explicit parent-child promise contracts.
//!
//! The owner trims one captured record's history, pushes that record's current
//! value, and passes JSON of that same record to an ordered captured callable.
//! Its guarded promise chain directly owns all three inline arrows: byte buffer
//! to string, that string/current-value pair to localStorage, and caught error
//! to an ordered captured reporter. No nested donor is searched. Capacity,
//! storage key and diagnostic message literals are not witness fingerprints.
use super::*;
use oxc_semantic::SymbolId;

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

fn history_length(model: &Model<'_, '_>, e: &Expression<'_>, record: SymbolId) -> bool {
    member(e, "length").is_some_and(|e| record_field(model, e, record, "ci"))
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
    captured_symbol(model, site, symbol(model, e)?)?;
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
    matches!(e.without_parentheses(), Expression::StringLiteral(_))
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

fn history(model: &Model<'_, '_>, site: FunctionSite, body: &FunctionBody<'_>) -> Option<()> {
    let [current, Statement::IfStatement(trim), push, encoded, Statement::IfStatement(guard)] =
        body.statements.as_slice()
    else {
        return None;
    };
    let (current, value) = local(model, current)?;
    let record = captured_symbol(
        model,
        site,
        symbol(model, member(member(value, "c")?, "a")?)?,
    )?;
    if trim.alternate.is_some() || guard.alternate.is_some() {
        return None;
    }
    let (length, capacity) = binary(&trim.test, ">=")?;
    let capacity = number(capacity)?;
    if !history_length(model, length, record)
        || !capacity.is_finite()
        || capacity < 1.0
        || capacity.fract() != 0.0
    {
        return None;
    }
    let Statement::BlockStatement(trim_body) = &trim.consequent else {
        return None;
    };
    let [splice] = trim_body.body.as_slice() else {
        return None;
    };
    let splice = call(expression(splice)?)?;
    if !record_field(model, member(&splice.callee, "splice")?, record, "ci") {
        return None;
    }
    let splice_args = args(splice, 2)?;
    let (length, subtracted) = binary(splice_args[1], "-")?;
    if number(splice_args[0]) != Some(0.0)
        || !history_length(model, length, record)
        || number(subtracted) != Some(capacity)
    {
        return None;
    }
    let push = call(expression(push)?)?;
    if !record_field(model, member(&push.callee, "push")?, record, "ci")
        || symbol(model, args(push, 1)?[0]) != Some(current)
    {
        return None;
    }
    let (encoded, encode) = local(model, encoded)?;
    let encode = call(encode)?;
    let encoder = external_callable(model, site, &encode.callee)?;
    if symbol(model, member(stringify(model, args(encode, 1)?[0])?, "a")?) != Some(record) {
        return None;
    }
    let (guarded, null) = binary(&guard.test, "!=")?;
    if symbol(model, guarded) != Some(encoded)
        || !matches!(null.without_parentheses(), Expression::NullLiteral(_))
    {
        return None;
    }
    let Statement::BlockStatement(chain) = &guard.consequent else {
        return None;
    };
    let [chain] = chain.body.as_slice() else {
        return None;
    };
    let caught = call(expression(chain)?)?;
    let second = call(member(&caught.callee, "catch")?)?;
    let first = call(member(&second.callee, "then")?)?;
    if symbol(model, member(&first.callee, "then")?) != Some(encoded) {
        return None;
    }
    // Each child is directly owned by the exact parent chain call; these are
    // argument contracts, not a search through descendants for matching tokens.
    let [conversion] = first.arguments.as_slice() else {
        return None;
    };
    byte_string(model, conversion)?;
    let [writer] = second.arguments.as_slice() else {
        return None;
    };
    persist(model, site, writer, record)?;
    let [reporter] = caught.arguments.as_slice() else {
        return None;
    };
    let reporter = report_error(model, site, reporter)?;
    (reporter.span != encoder.span).then_some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    facts: &Facts<'_>,
    _proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
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
    {
        return vec![];
    }
    if history(model, site, body).is_none() {
        return vec![];
    }
    vec![proposal("persistEncodedHistorySnapshot", "observables_encoded_history_persistence", "owned history trim/append and same-record JSON argument feed an ordered captured callable; guarded exact then/then/catch chain owns byte-to-string, same-record storage-pair and exact-error reporter child contracts; all locals/formals unwritten, all captures resolved, encoder/reporter are distinct external ordered callables", facts)]
}
