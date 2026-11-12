use super::*;
use oxc_semantic::SymbolId;

use crate::naming::support::vector_state::*;

// Only a plain var/let/const declaration creates an ordinary data binding;
// `using`/`await using` resource declarations are refused here even though the
// shared permissive helper would accept any variable declaration.
fn plain_local(model: &Model<'_, '_>, declarator: &VariableDeclarator<'_>) -> Option<SymbolId> {
    let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
        return None;
    };
    let symbol = identifier.symbol_id.get()?;
    if model.scoping.symbol_redeclarations(symbol).is_empty()
        && matches!(
            model
                .nodes
                .parent_kind(model.scoping.symbol_declaration(symbol)),
            AstKind::VariableDeclaration(declaration)
                if matches!(
                    declaration.kind,
                    VariableDeclarationKind::Var
                        | VariableDeclarationKind::Let
                        | VariableDeclarationKind::Const
                )
        )
        && !matches!(
            model.nodes.parent_kind(
                model
                    .nodes
                    .parent_id(model.scoping.symbol_declaration(symbol))
            ),
            AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
        )
    {
        Some(symbol)
    } else {
        None
    }
}

fn void_zero(expression: &Expression<'_>) -> bool {
    matches!(
        expression,
        Expression::UnaryExpression(unary)
            if unary.operator.as_str() == "void"
                && matches!(&unary.argument, Expression::NumericLiteral(number) if number.value == 0.0)
    )
}

fn indexed_member(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    owner: SymbolId,
    index: SymbolId,
) -> bool {
    let Expression::ComputedMemberExpression(member) = expression else {
        return false;
    };
    !member.optional
        && symbol(model, &member.object) == Some(owner)
        && symbol(model, &member.expression) == Some(index)
}

fn equality_scan(
    model: &Model<'_, '_>,
    callback: &ArrowFunctionExpression<'_>,
    source: SymbolId,
    item: SymbolId,
) -> Option<()> {
    let [local, branch, Statement::ReturnStatement(return_)] = callback.body.statements.as_slice()
    else {
        return None;
    };
    let local = one_variable(local)?;
    let equal = plain_local(model, local)?;
    if local.init.is_some() || model.binding_writes(equal) != 3 {
        return None;
    }
    if symbol(model, return_.argument.as_ref()?) != Some(equal) {
        return None;
    }
    let Statement::IfStatement(branch) = branch else {
        return None;
    };
    let Expression::LogicalExpression(logical) = &branch.test else {
        return None;
    };
    if logical.operator.as_str() != "||" {
        return None;
    }
    let Expression::LogicalExpression(first) = &logical.left else {
        return None;
    };
    if first.operator.as_str() != "||"
        || !matches!(&first.left, Expression::BinaryExpression(binary)
            if binary.operator.as_str() == "=="
                && symbol(model, &binary.left) == Some(source)
                && matches!(&binary.right, Expression::NullLiteral(_)))
        || !matches!(&first.right, Expression::BinaryExpression(binary)
            if binary.operator.as_str() == "=="
                && symbol(model, &binary.left) == Some(item)
                && matches!(&binary.right, Expression::NullLiteral(_)))
        || !matches!(&logical.right, Expression::BinaryExpression(binary)
            if binary.operator.as_str() == "!=="
                && same_member(model, &binary.left, source, "length")
                && same_member(model, &binary.right, item, "length"))
    {
        return None;
    }
    let [set_false] = (match &branch.consequent {
        Statement::BlockStatement(block) => block.body.as_slice(),
        _ => return None,
    }) else {
        return None;
    };
    if !matches!(set_false, Statement::ExpressionStatement(statement)
        if matches!(&statement.expression, Expression::AssignmentExpression(assignment)
            if assignment.operator.as_str() == "="
                && matches!(&assignment.left, AssignmentTarget::AssignmentTargetIdentifier(identifier)
                    if crate::analysis::identifier_symbol(identifier, model.scoping) == Some(equal))
                && matches!(&assignment.right, Expression::BooleanLiteral(boolean) if !boolean.value)))
    {
        return None;
    }
    let Statement::BlockStatement(other) = branch.alternate.as_ref()? else {
        return None;
    };
    let [cursor, loop_] = other.body.as_slice() else {
        return None;
    };
    let cursor = one_variable(cursor)?;
    let index = plain_local(model, cursor)?;
    if !matches!(cursor.init.as_ref()?, Expression::NumericLiteral(number) if number.value == 0.0)
        || model.binding_writes(index) != 1
    {
        return None;
    }
    let Statement::ForStatement(for_) = loop_ else {
        return None;
    };
    if for_.init.is_some() || for_.test.is_some() || for_.update.is_some() {
        return None;
    }
    let Statement::BlockStatement(loop_body) = &for_.body else {
        return None;
    };
    let [end, equal_statement, set_false, Statement::BreakStatement(_)] = loop_body.body.as_slice()
    else {
        return None;
    };
    let Statement::IfStatement(end) = end else {
        return None;
    };
    let Expression::BinaryExpression(end_test) = &end.test else {
        return None;
    };
    if end_test.operator.as_str() != ">="
        || symbol(model, &end_test.left) != Some(index)
        || !same_member(model, &end_test.right, source, "length")
    {
        return None;
    }
    if end.alternate.is_some() {
        return None;
    }
    let [set_true, Statement::BreakStatement(break_)] = (match &end.consequent {
        Statement::BlockStatement(block) => block.body.as_slice(),
        _ => return None,
    }) else {
        return None;
    };
    if break_.label.is_some()
        || !matches!(set_true, Statement::ExpressionStatement(statement)
            if matches!(&statement.expression, Expression::AssignmentExpression(assignment)
                if assignment.operator.as_str() == "="
                    && matches!(&assignment.left, AssignmentTarget::AssignmentTargetIdentifier(identifier)
                        if crate::analysis::identifier_symbol(identifier, model.scoping) == Some(equal))
                    && matches!(&assignment.right, Expression::BooleanLiteral(boolean) if boolean.value)))
    {
        return None;
    }
    let Statement::IfStatement(mismatch) = equal_statement else {
        return None;
    };
    let Expression::BinaryExpression(comparison) = &mismatch.test else {
        return None;
    };
    if comparison.operator.as_str() != "===" {
        return None;
    }
    let source_index = match &comparison.left {
        Expression::ComputedMemberExpression(member) if !member.optional => {
            let index = symbol(model, &member.expression)?;
            indexed_member(model, &comparison.left, source, index).then_some(index)?
        }
        _ => return None,
    };
    // The element comparison and the increment must use the very cursor
    // declared for this scan, not merely another in-scope index symbol.
    if source_index != index {
        return None;
    }
    if !indexed_member(model, &comparison.right, item, source_index) {
        return None;
    }
    let [Statement::ExpressionStatement(increment), Statement::ContinueStatement(continue_)] =
        (match &mismatch.consequent {
            Statement::BlockStatement(block) => block.body.as_slice(),
            _ => return None,
        })
    else {
        return None;
    };
    if continue_.label.is_some()
        || !matches!(&increment.expression, Expression::UpdateExpression(update)
            if update.operator.as_str() == "++"
                && matches!(&update.argument, SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier)
                    if crate::analysis::identifier_symbol(identifier, model.scoping) == Some(source_index)))
    {
        return None;
    }
    if mismatch.alternate.is_some()
        || break_.label.is_some()
        || !matches!(set_false, Statement::ExpressionStatement(statement)
            if matches!(&statement.expression, Expression::AssignmentExpression(assignment)
                if assignment.operator.as_str() == "="
                    && matches!(&assignment.left, AssignmentTarget::AssignmentTargetIdentifier(identifier)
                        if crate::analysis::identifier_symbol(identifier, model.scoping) == Some(equal))
                    && matches!(&assignment.right, Expression::BooleanLiteral(boolean) if !boolean.value)))
    {
        return None;
    }
    Some(())
}

fn record_distinct_vector_sample(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let parameters = plain_function_parameters(model, function, 2)?;
    let [source, state] = parameters.as_slice() else {
        return None;
    };
    let [alias, duplicate, count, limit] = body.statements.as_slice() else {
        return None;
    };
    let alias = one_variable(alias)?;
    let alias_symbol = plain_local(model, alias)?;
    if model.binding_writes(alias_symbol) != 0
        || symbol(model, alias.init.as_ref()?) != Some(*source)
    {
        return None;
    }
    let Statement::IfStatement(duplicate) = duplicate else {
        return None;
    };
    let Expression::CallExpression(some) = &duplicate.test else {
        return None;
    };
    let some_receiver = static_member(&some.callee, "some")?;
    if some.optional || some.arguments.len() != 1 || !same_member(model, some_receiver, *state, "a")
    {
        return None;
    }
    let [Argument::ArrowFunctionExpression(callback)] = some.arguments.as_slice() else {
        return None;
    };
    let [callback_parameter] = callback.params.items.as_slice() else {
        return None;
    };
    let item = plain_parameter(model, &callback_parameter.pattern)?;
    if callback.r#async
        || callback.params.rest.is_some()
        || callback_parameter.initializer.is_some()
        || callback.expression
    {
        return None;
    }
    equality_scan(model, callback, alias_symbol, item)?;
    let Statement::ReturnStatement(duplicate_return) = &duplicate.consequent else {
        return None;
    };
    if !void_zero(duplicate_return.argument.as_ref()?) || duplicate.alternate.is_some() {
        return None;
    }
    let Statement::ExpressionStatement(count) = count else {
        return None;
    };
    let Expression::AssignmentExpression(count) = &count.expression else {
        return None;
    };
    let AssignmentTarget::StaticMemberExpression(count_left) = &count.left else {
        return None;
    };
    if count.operator.as_str() != "="
        || count_left.optional
        || count_left.property.name != "b"
        || symbol(model, &count_left.object) != Some(*state)
    {
        return None;
    }
    let Expression::BinaryExpression(add) = &count.right else {
        return None;
    };
    if add.operator.as_str() != "+"
        || !matches!(&add.right, Expression::NumericLiteral(number) if number.value == 1.0)
    {
        return None;
    }
    let Some(number_call) = call(&add.left) else {
        return None;
    };
    if expression_path(model, &number_call.callee) != ["Number"]
        || number_call.arguments.len() != 1
        || !same_member(model, call_arguments(number_call).first()?, *state, "b")
    {
        return None;
    }
    let Statement::IfStatement(limit) = limit else {
        return None;
    };
    let Expression::BinaryExpression(limit_test) = &limit.test else {
        return None;
    };
    let limit_length = static_member(&limit_test.left, "length")?;
    if limit_test.operator.as_str() != ">"
        || !same_member(model, limit_length, *state, "a")
        || !matches!(&limit_test.right, Expression::NumericLiteral(number) if number.value == 5.0)
    {
        return None;
    }
    let Statement::ReturnStatement(limit_return) = &limit.consequent else {
        return None;
    };
    if !void_zero(limit_return.argument.as_ref()?) {
        return None;
    }
    let Statement::BlockStatement(other) = limit.alternate.as_ref()? else {
        return None;
    };
    let [push, done] = other.body.as_slice() else {
        return None;
    };
    let Statement::ReturnStatement(done) = done else {
        return None;
    };
    if !void_zero(done.argument.as_ref()?) {
        return None;
    }
    let Statement::ExpressionStatement(push) = push else {
        return None;
    };
    let push = call(&push.expression)?;
    let [argument] = push.arguments.as_slice() else {
        return None;
    };
    let push_receiver = static_member(&push.callee, "push")?;
    if !same_member(model, push_receiver, *state, "a") {
        return None;
    }
    let argument = call(argument.as_expression()?)?;
    if expression_path(model, &argument.callee) != ["Array", "prototype", "slice", "call"]
        || argument.arguments.len() != 1
        || symbol(model, call_arguments(argument).first()?) != Some(alias_symbol)
    {
        return None;
    }
    let _ = site;
    Some(())
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
    if record_distinct_vector_sample(model, site, function, body).is_some() {
        claims.push(proposal(
            "recordPixelVectorSample",
            "graphics_distinct_vector_sample",
            "bounded unique vector history: exact element-by-element comparison callback with initialized-cursor identity, captured counter increment, exact five-entry limit, and guarded Array.prototype.slice copy pushed into the second formal's list",
            f,
        ));
    }
    claims
}
