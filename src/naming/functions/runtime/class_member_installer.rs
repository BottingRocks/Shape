use super::*;

use crate::naming::support::installer_events::*;

// `using`/`await using` resource bindings are not plain local storage: they get
// a disposable runtime protocol and are never accepted here. The shared
// permissive helper only requires a single-declarator variable statement, so
// this stricter form is kept locally.
fn one_variable<'a>(statement: &'a Statement<'a>) -> Option<&'a VariableDeclarator<'a>> {
    let Statement::VariableDeclaration(declaration) = statement else {
        return None;
    };
    if !matches!(
        declaration.kind,
        VariableDeclarationKind::Var
            | VariableDeclarationKind::Let
            | VariableDeclarationKind::Const
    ) {
        return None;
    }
    let [declarator] = declaration.declarations.as_slice() else {
        return None;
    };
    Some(declarator)
}

fn boolean(e: &Expression<'_>, expected: bool) -> bool {
    matches!(e, Expression::BooleanLiteral(b) if b.value == expected)
}

fn indexed(model: &Model<'_, '_>, e: &Expression<'_>, base: Symbol, index: Symbol) -> bool {
    matches!(e, Expression::ComputedMemberExpression(m)
        if !m.optional && symbol(model, &m.object) == Some(base) && symbol(model, &m.expression) == Some(index))
}

fn descriptor_member(
    model: &Model<'_, '_>,
    e: &Expression<'_>,
    members: Symbol,
    cursor: Symbol,
    property: &str,
) -> bool {
    let Expression::StaticMemberExpression(m) = e else {
        return false;
    };
    !m.optional && m.property.name == property && indexed(model, &m.object, members, cursor)
}

fn descriptor_entries(
    model: &Model<'_, '_>,
    s: &Statement<'_>,
    members: Symbol,
    cursor: Symbol,
) -> bool {
    let Statement::ExpressionStatement(s) = s else {
        return false;
    };
    let Expression::AssignmentExpression(a) = &s.expression else {
        return false;
    };
    if a.operator.as_str() != "=" {
        return false;
    }
    match &a.left {
        AssignmentTarget::StaticMemberExpression(target)
            if !target.optional
                && target.property.name == "enumerable"
                && indexed(model, &target.object, members, cursor) =>
        {
            let Expression::LogicalExpression(or) = &a.right else {
                return false;
            };
            or.operator.as_str() == "||"
                && descriptor_member(model, &or.left, members, cursor, "enumerable")
                && boolean(&or.right, false)
        }
        AssignmentTarget::StaticMemberExpression(target)
            if !target.optional
                && target.property.name == "configurable"
                && indexed(model, &target.object, members, cursor) =>
        {
            boolean(&a.right, true)
        }
        _ => false,
    }
}

fn descriptor_loop(
    model: &Model<'_, '_>,
    s: &Statement<'_>,
    members: Symbol,
    target: Symbol,
    prototype: bool,
) -> bool {
    let Statement::IfStatement(branch) = s else {
        return false;
    };
    if symbol(model, &branch.test) != Some(members) || branch.alternate.is_some() {
        return false;
    }
    let Some(body) = block(&branch.consequent) else {
        return false;
    };
    let (cursor, loop_) = match body {
        [declaration, Statement::ForStatement(loop_)] => {
            let Some(v) = one_variable(declaration) else {
                return false;
            };
            let Some(init) = v.init.as_ref() else {
                return false;
            };
            let Some(s) = unredeclared_binding(model, &v.id) else {
                return false;
            };
            if !number(init, 0.0) || model.formal_symbol_slot(s).is_some() {
                return false;
            }
            (s, loop_)
        }
        _ => return false,
    };
    if loop_.init.is_some() {
        return false;
    }
    let Some(Expression::BinaryExpression(test)) = &loop_.test else {
        return false;
    };
    let Some(length) = member(&test.right, "length") else {
        return false;
    };
    if test.operator.as_str() != "<"
        || symbol(model, &test.left) != Some(cursor)
        || symbol(model, length) != Some(members)
    {
        return false;
    }
    let Some(Expression::UpdateExpression(update)) = &loop_.update else {
        return false;
    };
    if update.operator.as_str() != "++"
        || !matches!(&update.argument, SimpleAssignmentTarget::AssignmentTargetIdentifier(id)
            if crate::analysis::identifier_symbol(id, model.scoping) == Some(cursor))
    {
        return false;
    }
    let Some(loop_body) = block(&loop_.body) else {
        return false;
    };
    let [enumerable, configurable, writable, define] = loop_body else {
        return false;
    };
    if !descriptor_entries(model, enumerable, members, cursor)
        || !descriptor_entries(model, configurable, members, cursor)
    {
        return false;
    }
    let Statement::IfStatement(writable) = writable else {
        return false;
    };
    if writable.alternate.is_some() {
        return false;
    }
    let Expression::BinaryExpression(has) = &writable.test else {
        return false;
    };
    if !matches!(&has.left, Expression::StringLiteral(key) if key.value == "value")
        || has.operator.as_str() != "in"
        || !indexed(model, &has.right, members, cursor)
    {
        return false;
    }
    let Some(writable_body) = block(&writable.consequent) else {
        return false;
    };
    let [Statement::ExpressionStatement(write)] = writable_body else {
        return false;
    };
    let Expression::AssignmentExpression(write) = &write.expression else {
        return false;
    };
    let AssignmentTarget::StaticMemberExpression(target_property) = &write.left else {
        return false;
    };
    if write.operator.as_str() != "="
        || target_property.optional
        || target_property.property.name != "writable"
        || !indexed(model, &target_property.object, members, cursor)
        || !boolean(&write.right, true)
    {
        return false;
    }
    let Some(define) = statement_call(define) else {
        return false;
    };
    // The descriptor writer must be the unshadowed `.Object.defineProperty`
    // provider protocol, not any receiver that happens to have that member.
    if define.optional
        || !global_path_ends_with(model, &define.callee, &["Object", "defineProperty"])
        || define.arguments.len() != 3
    {
        return false;
    }
    let args = call_arguments(define);
    if args.len() != 3 {
        return false;
    }
    let receiver = if prototype {
        member(args[0], "prototype").is_some_and(|e| symbol(model, e) == Some(target))
    } else {
        symbol(model, args[0]) == Some(target)
    };
    receiver
        && descriptor_member(model, args[1], members, cursor, "key")
        && indexed(model, args[2], members, cursor)
}

fn define_class_members(model: &Model<'_, '_>, function: &Function<'_>) -> Option<()> {
    let Some(body) = function.body.as_ref() else {
        return None;
    };
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
    let [target, members, statics] = params.as_slice() else {
        return None;
    };
    let [first, second, Statement::ReturnStatement(end)] = body.statements.as_slice() else {
        return None;
    };
    if !descriptor_loop(model, first, *members, *target, true)
        || !descriptor_loop(model, second, *statics, *target, false)
        || end.argument.as_ref().and_then(|e| symbol(model, e)) != Some(*target)
    {
        return None;
    }
    Some(())
}

fn installer_factory(model: &Model<'_, '_>, function: &Function<'_>) -> Option<()> {
    let body = function.body.as_ref()?;
    if !function.params.items.is_empty()
        || function.params.rest.is_some()
        || function.r#async
        || function.generator
        || !body.directives.is_empty()
    {
        return None;
    }
    let [Statement::FunctionDeclaration(installer), Statement::ReturnStatement(end)] =
        body.statements.as_slice()
    else {
        return None;
    };
    let installer_symbol = installer.id.as_ref()?.symbol_id.get()?;
    if symbol(model, end.argument.as_ref()?) != Some(installer_symbol) {
        return None;
    }
    define_class_members(model, installer)
}

pub(in crate::naming) fn evaluate(
    model: &Model<'_, '_>,
    site: FunctionSite,
    f: &Facts<'_>,
    proposals: &BTreeMap<u32, Vec<Proposal>>,
) -> Vec<Proposal> {
    let AstKind::Function(function) = model.nodes.kind(site.node_id) else {
        return Vec::new();
    };
    let Some(body) = function.body.as_ref() else {
        return Vec::new();
    };
    let mut claims = Vec::new();
    if !prior_class_member_roles(&proposals) && installer_factory(model, function).is_some() {
        claims.push(proposal(
            "createClassMembersInstaller",
            "shared_class_member_installer",
            "returns an owned installer whose two proven loops define enumerable/configurable/writable descriptors for slot 1 on slot 0.prototype and slot 2 on slot 0 through the exact .defineProperty provider protocol, then returns slot 0",
            f,
        ));
    }
    if !prior_class_member_roles(&proposals) && define_class_members(model, function).is_some() {
        claims.push(proposal(
            "defineClassMembers",
            "shared_class_member_installer",
            "defines enumerable/configurable/writable descriptors from slot 1 on slot 0.prototype and slot 2 on slot 0 through the exact .defineProperty provider protocol on item.key/item, then returns slot 0",
            f,
        ));
    }
    claims
}
