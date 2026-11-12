use super::*;

use crate::naming::support::installer_events::*;

pub(in crate::naming) fn register_extension(model: &Model<'_, '_>, function: &Function<'_>) -> Option<()> {
    let body = function.body.as_ref()?;
    if function.params.rest.is_some()
        || function.r#async
        || function.generator
        || !body.directives.is_empty()
    {
        return None;
    }
    let params: Option<Vec<_>> = function
        .params
        .items
        .iter()
        .map(|p| {
            if p.initializer.is_some() {
                return None;
            }
            immutable_binding(model, &p.pattern)
        })
        .collect();
    let params = params?;
    let [key, detector, serialiser, deserialiser] = params.as_slice() else {
        return None;
    };
    // A scratch record is a source lowering of the same registration protocol.
    // Consume its executed declaration before the exact store; no uninitialized,
    // conditional, deferred or subsequently reassigned holder is accepted.
    let (record_declaration, statement) = match body.statements.as_slice() {
        [Statement::ExpressionStatement(statement)] => (None, statement),
        [Statement::VariableDeclaration(declaration), Statement::ExpressionStatement(statement)]
            if declaration.declarations.len() == 1
                && matches!(
                    declaration.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                ) =>
        {
            (Some(&declaration.declarations[0]), statement)
        }
        _ => return None,
    };
    let Expression::AssignmentExpression(assign) = &statement.expression else {
        return None;
    };
    let AssignmentTarget::ComputedMemberExpression(target) = &assign.left else {
        return None;
    };
    let Some(extensions) = member(&target.object, "extensions") else {
        return None;
    };
    if assign.operator.as_str() != "="
        || target.optional
        || !matches!(extensions, Expression::ThisExpression(_))
        || symbol(model, &target.expression) != Some(*key)
    {
        return None;
    }
    let record_value = if let Some(variable) = record_declaration {
        let holder = immutable_binding(model, &variable.id)?;
        if symbol(model, &assign.right) != Some(holder) {
            return None;
        }
        variable.init.as_ref()?
    } else {
        &assign.right
    };
    let Expression::ObjectExpression(record) = record_value else {
        return None;
    };
    let mut fields = BTreeMap::new();
    for property in &record.properties {
        let ObjectPropertyKind::ObjectProperty(p) = property else {
            return None;
        };
        let name = property_key_name(&p.key)?;
        if p.computed || p.method || p.kind != PropertyKind::Init || fields.contains_key(&name) {
            return None;
        }
        let expected = match name.as_str() {
            "detector" => *detector,
            "serialiser" => *serialiser,
            "deserialiser" => *deserialiser,
            _ => return None,
        };
        if symbol(model, &p.value) != Some(expected) {
            return None;
        }
        fields.insert(name, ());
    }
    (fields.len() == 3).then_some(())
}

// A constructor is proved independently of any surrounding factory, sibling
// register method or encoding-tag table. Only its complete body supplies the
// behavior; the guard's captured callable must resolve back to this exact site.
/// Whether naming this constructor is safe, given what else its scope holds.
///
/// A standalone constructor — no register method anywhere around it — is proved
/// on its own body. But when the scope *does* contain a register candidate
/// (a function that writes `this.extensions`), the constructor belongs to a
/// registry: it is only named when one of those candidates is itself proved, so
/// a malformed registry cannot leak a partial label.
pub(in crate::naming) fn sibling_register(model: &Model<'_, '_>, site: FunctionSite) -> bool {
    let AstKind::FunctionBody(body) = model.nodes.parent_kind(site.node_id) else {
        return true;
    };
    let mut candidates = 0usize;
    let mut proven = 0usize;
    for statement in &body.statements {
        let Statement::FunctionDeclaration(function) = statement else {
            continue;
        };
        if function.span.start == site.span.start {
            continue;
        }
        let text = model
            .source
            .get(function.span.start as usize..function.span.end as usize)
            .unwrap_or_default();
        if !text.contains("extensions") {
            continue;
        }
        candidates += 1;
        if register_extension(model, function).is_some() {
            proven += 1;
        }
    }
    candidates == 0 || proven > 0
}

pub(in crate::naming) fn registry_constructor(model: &Model<'_, '_>, site: FunctionSite, f: &Function<'_>) -> Option<()> {
    let body = f.body.as_ref()?;
    if f.r#async
        || f.generator
        || f.params.rest.is_some()
        || !f.params.items.is_empty()
        || body.directives.len() != 1
        || body.directives[0].directive != "use strict"
    {
        return None;
    }
    let [Statement::IfStatement(branch)] = body.statements.as_slice() else {
        return None;
    };
    let Expression::BinaryExpression(test) = &branch.test else {
        return None;
    };
    if test.operator.as_str() != "instanceof"
        || !matches!(&test.left, Expression::ThisExpression(_))
        || stable_function(model, &test.right, 0)?.span != site.span
    {
        return None;
    }
    let [Statement::ExpressionStatement(initialize), Statement::ReturnStatement(ret)] =
        block(&branch.consequent)?
    else {
        return None;
    };
    let Expression::AssignmentExpression(assign) = &initialize.expression else {
        return None;
    };
    let AssignmentTarget::StaticMemberExpression(target) = &assign.left else {
        return None;
    };
    if assign.operator.as_str() != "="
        || target.optional
        || target.property.name != "extensions"
        || !matches!(&target.object, Expression::ThisExpression(_))
    {
        return None;
    }
    let create = call(&assign.right)?;
    if global_path(model, &create.callee)?.as_slice() != ["Object", "create"]
        || !matches!(create.arguments.as_slice(), [Argument::NullLiteral(_)])
        || !matches!(ret.argument.as_ref(), Some(Expression::UnaryExpression(u))
            if u.operator.as_str() == "void" && number(&u.argument, 0.0))
    {
        return None;
    }
    let [Statement::ThrowStatement(throw)] = block(branch.alternate.as_ref()?)? else {
        return None;
    };
    let Expression::NewExpression(error) = &throw.argument else {
        return None;
    };
    (global_path(model, &error.callee)?.as_slice() == ["TypeError"]
        && matches!(error.arguments.as_slice(), [Argument::StringLiteral(message)]
            if message.value == "Cannot call a class as a function"))
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
    let Some(body) = function.body.as_ref() else {
        return Vec::new();
    };
    let mut claims = Vec::new();
    // The constructor is proved from its own body, but it is only *named* when
    // the registry it belongs to is also proved: a constructor inside a
    // registry whose register method is malformed must stay unnamed, so a
    // partial label never leaks onto the signals it would carry.
    if registry_constructor(model, site, function).is_some() && sibling_register(model, site) {
        claims.push(proposal(
            "ExtensionRegistry",
            "shared_extension_registry_constructor",
            "complete own constructor: strict-mode exact self-instance guard; stable scope-resolved captured callable origin/order; initializes only this.extensions to Object.create(null), returns void 0, otherwise throws TypeError",
            f,
        ));
    }
    if register_extension(model, function).is_some() {
        claims.push(proposal(
            "registerExtension",
            "shared_extension_registry",
            "writes exactly {detector:slot1, serialiser:slot2, deserialiser:slot3} under slot0 in this.extensions",
            f,
        ));
    }
    claims
}
