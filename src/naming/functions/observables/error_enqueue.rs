//! Small pass8 frontier: namespaced error-record enqueue.
//!
//! The legacy p05 `signal_error_enqueue` finder only accepts a bare
//! `Object.create(null)` receiver. The seed builds the same record through
//! `b.Object.create(null)` where `b` is an unresolved host root. This module
//! proves the whole owned body and reuses the established canonical role
//! `enqueueSignalError`; it never treats the unresolved root as a native
//! builtin and never uses a generated spelling or build constant.
use super::*;
use oxc_semantic::SymbolId;

/// The exact error-like fields copied into the record. `message` is
/// mandatory; `stack` plus at least one other field is the established
/// `enqueueSignalError` shape. Field names are semantic operations, not
/// generated spellings.
const ERROR_FIELDS: &[&str] = &[
    "name",
    "description",
    "stack",
    "stacktrace",
    "num",
    "signalPosition",
];

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(e, model.scoping)
}

fn plain_binding(model: &Model<'_, '_>, p: &BindingPattern<'_>) -> Option<SymbolId> {
    let BindingPattern::BindingIdentifier(id) = p else {
        return None;
    };
    let symbol = id.symbol_id.get()?;
    (model.binding_writes(symbol) == 0 && model.scoping.symbol_redeclarations(symbol).is_empty())
        .then_some(symbol)
}

fn member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = e else {
        return None;
    };
    (!m.optional && m.property.name == key).then_some(&m.object)
}

fn assignment_member(
    model: &Model<'_, '_>,
    a: &AssignmentExpression<'_>,
    property: &str,
) -> Option<SymbolId> {
    let AssignmentTarget::StaticMemberExpression(target) = &a.left else {
        return None;
    };
    if target.optional || target.property.name != property || a.operator.as_str() != "=" {
        return None;
    }
    symbol(model, &target.object)
}

/// The exact one-argument `.Object.create(null)` call shape, with either the
/// unshadowed global `Object` root or an unresolved namespace root. The latter
/// is structural protocol evidence only; it is not treated as native-global
/// provenance.
fn create_null(model: &Model<'_, '_>, e: &Expression<'_>) -> bool {
    let Expression::CallExpression(call) = e else {
        return false;
    };
    if call.optional
        || call.arguments.len() != 1
        || !matches!(
            call_arguments(call).as_slice(),
            [Expression::NullLiteral(_)]
        )
    {
        return false;
    }
    let Expression::StaticMemberExpression(create) = &call.callee else {
        return false;
    };
    if create.optional || create.property.name != "create" {
        return false;
    }
    match &create.object {
        Expression::Identifier(object) if object.name == "Object" => {
            crate::analysis::identifier_symbol(object, model.scoping).is_none()
        }
        Expression::StaticMemberExpression(namespace)
            if !namespace.optional && namespace.property.name == "Object" =>
        {
            matches!(&namespace.object, Expression::Identifier(root)
                if crate::analysis::identifier_symbol(root, model.scoping).is_none())
        }
        _ => false,
    }
}

/// A plain captured binding declared outside the owned function. Parameters,
/// catch bindings and function declarations cannot establish this source role.
fn captured_outside(
    model: &Model<'_, '_>,
    site: FunctionSite,
    e: &Expression<'_>,
) -> Option<SymbolId> {
    let symbol = symbol(model, e)?;
    if site
        .span
        .contains_inclusive(model.scoping.symbol_span(symbol))
        || !model.scoping.symbol_redeclarations(symbol).is_empty()
    {
        return None;
    }
    let declaration = model.scoping.symbol_declaration(symbol);
    let AstKind::VariableDeclarator(variable) = model.nodes.kind(declaration) else {
        return None;
    };
    if !matches!(&variable.id, BindingPattern::BindingIdentifier(id)
        if id.symbol_id.get() == Some(symbol))
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
    Some(symbol)
}

/// `record.message = formal.message` with exact source/target identities.
fn direct_copy(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    formal: SymbolId,
    record: SymbolId,
    field: &str,
) -> bool {
    let Statement::ExpressionStatement(statement) = statement else {
        return false;
    };
    let Expression::AssignmentExpression(assignment) = &statement.expression else {
        return false;
    };
    let Expression::StaticMemberExpression(source) = &assignment.right else {
        return false;
    };
    assignment_member(model, assignment, field) == Some(record)
        && !source.optional
        && source.property.name == field
        && symbol(model, &source.object) == Some(formal)
        && model.site_parameter_slot(&source.object, site) == Some(0)
}

/// `if (formal.field != null) { record.field = formal.field; }`.
fn guarded_copy(
    model: &Model<'_, '_>,
    site: FunctionSite,
    statement: &Statement<'_>,
    formal: SymbolId,
    record: SymbolId,
) -> Option<String> {
    let Statement::IfStatement(branch) = statement else {
        return None;
    };
    if branch.alternate.is_some() {
        return None;
    }
    let Expression::BinaryExpression(test) = &branch.test else {
        return None;
    };
    if test.operator.as_str() != "!=" || !matches!(&test.right, Expression::NullLiteral(_)) {
        return None;
    }
    let Expression::StaticMemberExpression(source) = &test.left else {
        return None;
    };
    let field = source.property.name.as_str();
    if source.optional
        || !ERROR_FIELDS.contains(&field)
        || symbol(model, &source.object) != Some(formal)
        || model.site_parameter_slot(&source.object, site) != Some(0)
    {
        return None;
    }
    let Statement::BlockStatement(block) = &branch.consequent else {
        return None;
    };
    let [body] = block.body.as_slice() else {
        return None;
    };
    let Statement::ExpressionStatement(body) = body else {
        return None;
    };
    let Expression::AssignmentExpression(assignment) = &body.expression else {
        return None;
    };
    let Expression::StaticMemberExpression(value) = &assignment.right else {
        return None;
    };
    if assignment_member(model, assignment, field) != Some(record)
        || value.optional
        || value.property.name != field
        || symbol(model, &value.object) != Some(formal)
        || model.site_parameter_slot(&value.object, site) != Some(0)
    {
        return None;
    }
    Some(field.to_string())
}

fn signal_error_enqueue(
    model: &Model<'_, '_>,
    site: FunctionSite,
    formal: SymbolId,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let (record_statement, rest) = body.statements.split_first()?;
    let record_variable = one_variable(record_statement)?;
    let record = plain_binding(model, &record_variable.id)?;
    if model.binding_writes(record) != 0 || !create_null(model, record_variable.init.as_ref()?) {
        return None;
    }
    let (message, rest) = rest.split_first()?;
    if !direct_copy(model, site, message, formal, record, "message") {
        return None;
    }
    let (put, fields) = rest.split_last()?;
    let mut copied = BTreeSet::new();
    for field in fields {
        let field = guarded_copy(model, site, field, formal, record)?;
        if !copied.insert(field) {
            return None;
        }
    }
    if copied.len() < 2 || !copied.contains("stack") {
        return None;
    }
    let Statement::ExpressionStatement(put) = put else {
        return None;
    };
    let Expression::CallExpression(put) = &put.expression else {
        return None;
    };
    if put.optional || put.arguments.len() != 1 {
        return None;
    }
    let ring = member(&put.callee, "put")?;
    captured_outside(model, site, ring)?;
    (call_arguments(put)
        .first()
        .and_then(|argument| symbol(model, argument))
        == Some(record))
    .then_some(())
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
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
        || !body.directives.is_empty()
        || function.params.rest.is_some()
    {
        return vec![];
    }
    let [parameter] = function.params.items.as_slice() else {
        return vec![];
    };
    if parameter.initializer.is_some() {
        return vec![];
    }
    let Some(formal) = plain_binding(model, &parameter.pattern) else {
        return vec![];
    };
    if signal_error_enqueue(model, site, formal, body).is_none() {
        return vec![];
    }
    vec![proposal(
        "enqueueSignalError",
        "signals_error_enqueue",
        "one immutable formal; fresh record initialized by the exact one-argument `.Object.create(null)` call shape (global or opaque namespace); exact direct message copy, at least two distinct guarded name/description/stack/stacktrace/num/signalPosition copies with matching source and target names including mandatory stack, and a terminal put(record) on a plain capture declared outside the owned function; no return or extra statement",
        f,
    )]
}
