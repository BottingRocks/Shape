//! Counterless sibling of `entropy_seed`.
//!
//! Exact owned statements prove four local source words and their connected
//! merge. Both `clock = clock === clock ? random ^ clock : random` and
//! `if (clock === clock) { random = random ^ clock; }` are supported. The
//! captured-counter rule stays unchanged. No nested function supplies evidence.
use super::*;

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e.without_parentheses(), model.scoping)
}
fn is_symbol(model: &Model<'_, '_>, e: &Expression<'_>, s: SymbolId) -> bool {
    symbol(model, e) == Some(s)
}
fn number(e: &Expression<'_>, n: f64) -> bool {
    matches!(e.without_parentheses(), Expression::NumericLiteral(v) if v.value == n)
}
fn binary<'a>(e: &'a Expression<'a>, op: &str) -> Option<&'a BinaryExpression<'a>> {
    match e.without_parentheses() {
        Expression::BinaryExpression(b) if b.operator.as_str() == op => Some(b),
        _ => None,
    }
}
fn ambient(model: &Model<'_, '_>, e: &Expression<'_>, path: &[&str]) -> bool {
    match (e.without_parentheses(), path) {
        (Expression::Identifier(i), [name]) => i.name == *name && symbol(model, e).is_none(),
        (Expression::StaticMemberExpression(m), [prefix @ .., key]) if !m.optional => {
            m.property.name == *key && ambient(model, &m.object, prefix)
        }
        _ => false,
    }
}
fn call<'a>(e: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    match e.without_parentheses() {
        Expression::CallExpression(c) if !c.optional => Some(c),
        _ => None,
    }
}
fn zero_call(model: &Model<'_, '_>, e: &Expression<'_>, path: &[&str]) -> bool {
    call(e).is_some_and(|c| c.arguments.is_empty() && ambient(model, &c.callee, path))
}
fn uint_word<'a>(e: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    let b = binary(e, ">>>")?;
    number(&b.right, 0.0).then_some(&b.left)
}
fn random_word(model: &Model<'_, '_>, e: &Expression<'_>) -> bool {
    let Some(product) = uint_word(e).and_then(|e| binary(e, "*")) else {
        return false;
    };
    (number(&product.right, 4294967296.0) && zero_call(model, &product.left, &["Math", "random"]))
        || (number(&product.left, 4294967296.0)
            && zero_call(model, &product.right, &["Math", "random"]))
}
fn date_word(model: &Model<'_, '_>, e: &Expression<'_>) -> bool {
    let Some(c) = uint_word(e).and_then(call) else {
        return false;
    };
    let args = call_arguments(c);
    if !ambient(model, &c.callee, &["Number"]) || c.arguments.len() != 1 || args.len() != 1 {
        return false;
    }
    matches!(args[0].without_parentheses(), Expression::NewExpression(n) if n.arguments.is_empty() && ambient(model,&n.callee,&["Date"]))
}
fn crypto_word(model: &Model<'_, '_>, e: &Expression<'_>) -> bool {
    let Expression::ComputedMemberExpression(m) = e.without_parentheses() else {
        return false;
    };
    if m.optional || !number(&m.expression, 0.0) {
        return false;
    }
    let Some(c) = call(&m.object) else {
        return false;
    };
    let args = call_arguments(c);
    if !ambient(model, &c.callee, &["crypto", "getRandomValues"])
        || c.arguments.len() != 1
        || args.len() != 1
    {
        return false;
    }
    let Expression::NewExpression(n) = args[0].without_parentheses() else {
        return false;
    };
    let [Argument::NumericLiteral(size)] = n.arguments.as_slice() else {
        return false;
    };
    ambient(model, &n.callee, &["Uint32Array"]) && size.value == 1.0
}
fn local<'a>(
    model: &Model<'_, '_>,
    s: &'a Statement<'a>,
) -> Option<(SymbolId, &'a Expression<'a>)> {
    let Statement::VariableDeclaration(d) = s else {
        return None;
    };
    if d.kind == VariableDeclarationKind::Const {
        return None;
    }
    let [v] = d.declarations.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(id) = &v.id else {
        return None;
    };
    let sym = id.symbol_id.get()?;
    model
        .scoping
        .symbol_redeclarations(sym)
        .is_empty()
        .then_some((sym, v.init.as_ref()?))
}
fn store<'a>(
    model: &Model<'_, '_>,
    s: &'a Statement<'a>,
    target: SymbolId,
) -> Option<&'a Expression<'a>> {
    let Statement::ExpressionStatement(s) = s else {
        return None;
    };
    let Expression::AssignmentExpression(a) = s.expression.without_parentheses() else {
        return None;
    };
    if a.operator != AssignmentOperator::Assign {
        return None;
    }
    let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left else {
        return None;
    };
    (crate::analysis::identifier_symbol(id, model.scoping) == Some(target)).then_some(&a.right)
}
fn attempted_store<'a>(
    model: &Model<'_, '_>,
    s: &'a Statement<'a>,
    target: SymbolId,
) -> Option<&'a Expression<'a>> {
    let Statement::TryStatement(t) = s else {
        return None;
    };
    if t.finalizer.is_some() {
        return None;
    }
    let handler = t.handler.as_ref()?;
    if !handler.body.body.is_empty()
        || handler
            .param
            .as_ref()
            .is_some_and(|p| !matches!(&p.pattern, BindingPattern::BindingIdentifier(_)))
    {
        return None;
    }
    let [s] = t.block.body.as_slice() else {
        return None;
    };
    store(model, s, target)
}
fn xor_pair(model: &Model<'_, '_>, e: &Expression<'_>, a: SymbolId, b: SymbolId) -> bool {
    binary(e, "^").is_some_and(|x| {
        (is_symbol(model, &x.left, a) && is_symbol(model, &x.right, b))
            || (is_symbol(model, &x.left, b) && is_symbol(model, &x.right, a))
    })
}
fn self_equality(model: &Model<'_, '_>, e: &Expression<'_>, s: SymbolId) -> bool {
    binary(e, "===").is_some_and(|b| is_symbol(model, &b.left, s) && is_symbol(model, &b.right, s))
}
fn merge(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    input: SymbolId,
    accumulator: SymbolId,
) -> bool {
    matches!(e.without_parentheses(),Expression::ConditionalExpression(c) if self_equality(model,&c.test,input) && xor_pair(model,&c.consequent,input,accumulator) && is_symbol(model,&c.alternate,accumulator))
}
fn self_store(model: &Model<'_, '_>, s: &Statement<'_>, target: SymbolId) -> bool {
    store(model, s, target).is_some_and(|e| is_symbol(model, e, target))
}
fn prove(model: &Model<'_, '_>, body: &FunctionBody<'_>) -> Option<()> {
    let [random, clock, clock_try, date, crypto, crypto_try, fold, clock_copy, clock_merge, date_merge, crypto_copy, end] =
        body.statements.as_slice()
    else {
        return None;
    };
    let (r, rv) = local(model, random)?;
    let (t, tv) = local(model, clock)?;
    let (d, dv) = local(model, date)?;
    let (c, cv) = local(model, crypto)?;
    if BTreeSet::from([r, t, d, c]).len() != 4
        || !random_word(model, rv)
        || !number(tv, 0.0)
        || !date_word(model, dv)
        || !number(cv, 0.0)
    {
        return None;
    }
    let performance = uint_word(attempted_store(model, clock_try, t)?)?;
    if !zero_call(model, performance, &["performance", "now"])
        || !crypto_word(model, attempted_store(model, crypto_try, c)?)
    {
        return None;
    }
    let fold = binary(store(model, fold, r)?, "^")?;
    if !((is_symbol(model, &fold.left, r) && number(&fold.right, 1103515245.0))
        || (is_symbol(model, &fold.right, r) && number(&fold.left, 1103515245.0)))
        || !self_store(model, clock_copy, t)
        || !self_store(model, crypto_copy, c)
    {
        return None;
    }
    let (accumulator, random_writes, clock_writes) = match clock_merge {
        Statement::IfStatement(branch) => {
            if branch.alternate.is_some() || !self_equality(model, &branch.test, t) {
                return None;
            }
            let Statement::BlockStatement(block) = &branch.consequent else {
                return None;
            };
            let [step] = block.body.as_slice() else {
                return None;
            };
            if !xor_pair(model, store(model, step, r)?, r, t) {
                return None;
            }
            (r, 2, 2)
        }
        _ => {
            if !merge(model, store(model, clock_merge, t)?, t, r) {
                return None;
            }
            (t, 1, 3)
        }
    };
    if !merge(model, store(model, date_merge, d)?, d, accumulator) {
        return None;
    }
    // Whole-body grammar forbids hidden writes; these scoped counts also reject
    // redeclared or shadow-donated state. All four bindings are owned locals.
    if model.binding_writes(r) != random_writes
        || model.binding_writes(t) != clock_writes
        || model.binding_writes(d) != 1
        || model.binding_writes(c) != 2
    {
        return None;
    }
    let Statement::ReturnStatement(end) = end else {
        return None;
    };
    xor_pair(model, uint_word(end.argument.as_ref()?)?, d, c).then_some(())
}
pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
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
        || prove(model, body).is_none()
    {
        return vec![];
    }
    vec![proposal("deriveSignalCodecSeed","encoding_counterless_entropy_seed",
        "exact zero-formal owned four-source seed grammar: random uint32 word XOR LCG multiplier, guarded performance.now and crypto Uint32Array word, Number(new Date()) word, connected self-equality guarded XOR merges and unsigned return; all host roots unshadowed; no captured counter, nested donor, extra statement or escaped local", f)]
}
