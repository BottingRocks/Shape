use super::*;

use crate::naming::support::publish_sync::*;

// A canonical integer property name (`ToPropertyKey`), restricted to the
// exactly-representable safe-integer range so the Rust rendering matches the
// JavaScript one; other numbers are refused rather than guessed.
fn numeric_property_key(value: f64) -> Option<String> {
    (value.is_finite() && value.fract() == 0.0 && value.abs() <= 9_007_199_254_740_992.0)
        .then(|| format!("{}", value as i64))
}

// A computed identifier key is only accepted when its binding is a stable,
// unwritten, non-redeclared plain var/let/const declared in an enclosing
// straight-line container before the use site. A literal initializer proves the
// key by its ToPropertyKey name. A declaration without an initializer is also
// accepted, but only because such a binding has no writes at all: it holds
// exactly the value `undefined`, whose ToPropertyKey name is the string
// "undefined". Distinct uninitialized symbols therefore share one proven
// property name and collide in the caller's key set, so SymbolId inequality is
// never mistaken for key distinctness.
fn computed_literal_key(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
) -> Option<String> {
    let s = symbol(model, e)?;
    if model.binding_writes(s) != 0
        || !model.scoping.symbol_redeclarations(s).is_empty()
        || model.scoping.symbol_span(s).end >= site.span.start
    {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(s);
    let AstKind::VariableDeclarator(v) = model.nodes.kind(declaration) else {
        return None;
    };
    let AstKind::VariableDeclaration(variable) = model.nodes.parent_kind(declaration) else {
        return None;
    };
    if !matches!(&v.id, BindingPattern::BindingIdentifier(id) if id.symbol_id.get() == Some(s))
        || !plain_variable_kind(variable.kind)
        || matches!(
            model.nodes.parent_kind(model.nodes.parent_id(declaration)),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
    {
        return None;
    }
    // A known key value also needs a dominating straight-line container: a
    // conditional block or other nested container does not establish that the
    // declaration and its initializer ran before every later use.
    if !matches!(
        model
            .nodes
            .kind(model.nodes.parent_id(model.nodes.parent_id(declaration))),
        AstKind::FunctionBody(_) | AstKind::Program(_)
    ) {
        return None;
    }
    let Some(init) = v.init.as_ref() else {
        // `const` without an initializer is not a valid declaration, so a real
        // program can only reach here through a `var`/`let` binding, whose
        // value is the stable `undefined`.
        if matches!(variable.kind, VariableDeclarationKind::Const) {
            return None;
        }
        return Some("undefined".to_string());
    };
    match init {
        Expression::StringLiteral(literal) => Some(literal.value.to_string()),
        Expression::NumericLiteral(literal) => numeric_property_key(literal.value),
        _ => None,
    }
}

// A stable, spelling-independent property name in ToPropertyKey form: a static
// name, a string/numeric literal, a literal-origin computed identifier, or a
// never-written uninitialized computed identifier (the proven name
// "undefined"). The name is shared across static and computed member forms so
// `[1]`/`["1"]` and same-value keys collide.
fn property_key(model: &Model<'_, '_>, site: FunctionSite, e: &Expression<'_>) -> Option<String> {
    match e {
        Expression::StringLiteral(s) => Some(s.value.to_string()),
        Expression::NumericLiteral(n) => numeric_property_key(n.value),
        Expression::Identifier(_) => computed_literal_key(model, site, e),
        _ => None,
    }
}

// A read of `receiver[key]` or `receiver.key`.
fn read_member(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
) -> Option<(Symbol, String)> {
    match e {
        Expression::StaticMemberExpression(m) if !m.optional => {
            Some((symbol(model, &m.object)?, m.property.name.to_string()))
        }
        Expression::ComputedMemberExpression(m) if !m.optional => Some((
            symbol(model, &m.object)?,
            property_key(model, site, &m.expression)?,
        )),
        _ => None,
    }
}

// A plain `receiver[key] = value` or `receiver.key = value` assignment.
fn write_member<'a>(
    model: &Model<'_, '_>,
    site: FunctionSite,
    a: &'a AssignmentExpression<'a>,
) -> Option<(Symbol, String, &'a Expression<'a>)> {
    if a.operator.as_str() != "=" {
        return None;
    }
    match &a.left {
        AssignmentTarget::StaticMemberExpression(m) if !m.optional => Some((
            symbol(model, &m.object)?,
            m.property.name.to_string(),
            &a.right,
        )),
        AssignmentTarget::ComputedMemberExpression(m) if !m.optional => Some((
            symbol(model, &m.object)?,
            property_key(model, site, &m.expression)?,
            &a.right,
        )),
        _ => None,
    }
}

// A write-back guard family: for each field of one captured object, assign the
// saved value only when the current value differs. Every field key is a
// ToPropertyKey-normalized, literal-origin name, every receiver and saved value
// is an exact scoped identity, and a single object is updated.
fn sync_captured_fields(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if !function.params.items.is_empty() || function.params.rest.is_some() {
        return None;
    }
    if body.statements.is_empty() || body.statements.len() > 16 {
        return None;
    }
    let mut receiver = None;
    let mut keys = BTreeSet::new();
    for statement in &body.statements {
        let Statement::IfStatement(branch) = statement else {
            return None;
        };
        if branch.alternate.is_some() {
            return None;
        }
        let Expression::BinaryExpression(test) = &branch.test else {
            return None;
        };
        if test.operator.as_str() != "!==" {
            return None;
        }
        let (read_receiver, read_key) = read_member(model, site, &test.left)?;
        let saved = symbol(model, &test.right)?;
        let Statement::BlockStatement(block) = &branch.consequent else {
            return None;
        };
        let [Statement::ExpressionStatement(inner)] = block.body.as_slice() else {
            return None;
        };
        let Expression::AssignmentExpression(assignment) = &inner.expression else {
            return None;
        };
        let (write_receiver, write_key, value) = write_member(model, site, assignment)?;
        if read_receiver != write_receiver
            || read_key != write_key
            || symbol(model, value) != Some(saved)
            || read_receiver == saved
            || !captured_slot(model, site, read_receiver)
            || !captured_slot(model, site, saved)
            || !keys.insert(read_key)
        {
            return None;
        }
        if receiver.is_some_and(|previous| previous != read_receiver) {
            return None;
        }
        receiver = Some(read_receiver);
    }
    receiver.map(|_| ())
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
    let Some(body) = function.body.as_ref() else {
        return Vec::new();
    };
    if function.r#async || function.generator || !body.directives.is_empty() {
        return Vec::new();
    }
    let mut claims = Vec::new();
    if sync_captured_fields(model, site, function, body).is_some() {
        claims.push(proposal(
            "syncCapturedFieldsIfChanged",
            "shared_anchor_field_sync",
            "zero-parameter body of ordered `if (captured[key] !== saved) captured[key] = saved` guards over one captured receiver with distinct ToPropertyKey-normalized keys (static names, stable literal-origin computed keys, or never-written uninitialized computed keys whose value is the proven `undefined`) and exact scoped saved values; no alternate, extra statement or conditional write",
            f,
        ));
    }
    claims
}
