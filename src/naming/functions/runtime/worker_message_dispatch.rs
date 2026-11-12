//! Small still-untargeted pass-8 frontier rules.
//!
//! These predicates are owned-body proofs.  They use only scoped symbol
//! identities, stable host members, and the enclosing statement list needed to
//! prove the worker callback is actually consumed.  The opaque message object
//! is treated as data; it is never used as evidence for a native or
//! domain-specific callee.
//! No generated spelling, offset, seed literal, or inherited proposal is an
//! input to either proof.
use super::*;
use oxc_semantic::SymbolId;

fn symbol(model: &Model<'_, '_>, expression: &Expression<'_>) -> Option<SymbolId> {
    crate::analysis::expression_symbol(expression, model.scoping)
}

fn static_member<'a>(expression: &'a Expression<'a>, name: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(member) = expression else {
        return None;
    };
    (!member.optional && member.property.name == name).then_some(&member.object)
}

fn call<'a>(expression: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(call) = expression else {
        return None;
    };
    (!call.optional).then_some(call)
}

/// Every argument must be an expression; spread elements are refused.
fn exact_arguments<'a>(call: &'a CallExpression<'a>) -> Option<Vec<&'a Expression<'a>>> {
    let values = call_arguments(call);
    (values.len() == call.arguments.len()).then_some(values)
}

fn plain_parameter(model: &Model<'_, '_>, parameter: &BindingPattern<'_>) -> Option<SymbolId> {
    let BindingPattern::BindingIdentifier(identifier) = parameter else {
        return None;
    };
    let symbol = identifier.symbol_id.get()?;
    (model.binding_writes(symbol) == 0 && model.scoping.symbol_redeclarations(symbol).is_empty())
        .then_some(symbol)
}

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

fn one_variable<'a>(statement: &'a Statement<'a>) -> Option<&'a VariableDeclarator<'a>> {
    let Statement::VariableDeclaration(declaration) = statement else {
        return None;
    };
    let [declarator] = declaration.declarations.as_slice() else {
        return None;
    };
    Some(declarator)
}

/// `message.data[index]`.
fn indexed_message_data(
    model: &Model<'_, '_>,
    expression: &Expression<'_>,
    message: SymbolId,
    index: f64,
) -> bool {
    let Expression::ComputedMemberExpression(member) = expression else {
        return false;
    };
    !member.optional
        && static_member(&member.object, "data")
            .is_some_and(|owner| symbol(model, owner) == Some(message))
        && matches!(&member.expression, Expression::NumericLiteral(number) if number.value == index)
}

/// A member of the unshadowed worker global `self`.
fn self_member(model: &Model<'_, '_>, expression: &Expression<'_>, name: &str) -> bool {
    static_member(expression, name).is_some_and(|object| expression_path(model, object) == ["self"])
}

fn target_member<'a>(target: &'a AssignmentTarget<'a>, name: &str) -> Option<&'a Expression<'a>> {
    let AssignmentTarget::StaticMemberExpression(member) = target else {
        return None;
    };
    (!member.optional && member.property.name == name).then_some(&member.object)
}

fn self_target(model: &Model<'_, '_>, target: &AssignmentTarget<'_>, name: &str) -> bool {
    target_member(target, name).is_some_and(|object| expression_path(model, object) == ["self"])
}

/// The enclosing statement list of a function declaration.  This is the block
/// that owns the registration call; it is not a corpus or whole-program walk.
fn enclosing_statements<'a>(
    model: &Model<'_, 'a>,
    site: FunctionSite,
) -> Option<&'a [Statement<'a>]> {
    let parent = model.nodes.parent_id(site.node_id);
    match model.nodes.kind(parent) {
        AstKind::FunctionBody(body) => Some(body.statements.as_slice()),
        AstKind::BlockStatement(block) => Some(block.body.as_slice()),
        AstKind::Program(program) => Some(program.body.as_slice()),
        _ => None,
    }
}

fn function_symbol(function: &Function<'_>) -> Option<SymbolId> {
    function.id.as_ref()?.symbol_id.get()
}

/// The worker message dispatcher body:
///
/// ```text
/// function handle(message) {
///   let seq = Number(self.__wx);
///   self.__wx = seq + 1;
///   vmCreateThread(message.data[3], message.data[1], message.data[0])(seq, message.data[2]);
/// }
/// ```
///
/// `message` is opaque event data.  The consumer role is proved by the same
/// owning block's `self.onmessage = handle` installation, not by the callback
/// argument or by a guessed native API.
fn handle_worker_message(
    model: &Model<'_, '_>,
    site: FunctionSite,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [parameter] = function.params.items.as_slice() else {
        return None;
    };
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || parameter.initializer.is_some()
        || !body.directives.is_empty()
    {
        return None;
    }
    let message = plain_parameter(model, &parameter.pattern)?;
    let [sequence, advance, invoke] = body.statements.as_slice() else {
        return None;
    };
    let sequence = one_variable(sequence)?;
    let sequence_symbol = plain_local(model, sequence)?;
    if model.binding_writes(sequence_symbol) != 0 {
        return None;
    }
    let Expression::CallExpression(number) = sequence.init.as_ref()? else {
        return None;
    };
    if number.optional
        || expression_path(model, &number.callee) != ["Number"]
        || number.arguments.len() != 1
    {
        return None;
    }
    let reads = exact_arguments(number)?;
    let [read] = reads.as_slice() else {
        return None;
    };
    if !self_member(model, read, "__wx") {
        return None;
    }
    let Statement::ExpressionStatement(advance) = advance else {
        return None;
    };
    let Expression::AssignmentExpression(advance) = &advance.expression else {
        return None;
    };
    if advance.operator.as_str() != "=" || !self_target(model, &advance.left, "__wx") {
        return None;
    }
    let Expression::BinaryExpression(plus) = &advance.right else {
        return None;
    };
    if plus.operator.as_str() != "+"
        || symbol(model, &plus.left) != Some(sequence_symbol)
        || !matches!(&plus.right, Expression::NumericLiteral(number) if number.value == 1.0)
    {
        return None;
    }
    let Statement::ExpressionStatement(invoke) = invoke else {
        return None;
    };
    let invoke = call(&invoke.expression)?;
    let factory = call(&invoke.callee)?;
    let Expression::Identifier(factory_callee) = &factory.callee else {
        return None;
    };
    let factory_arguments = exact_arguments(factory)?;
    let invoke_arguments = exact_arguments(invoke)?;
    if crate::analysis::identifier_symbol(factory_callee, model.scoping).is_some()
        || !matches!(
            factory_arguments.as_slice(),
            [a, b, c]
                if indexed_message_data(model, a, message, 3.0)
                    && indexed_message_data(model, b, message, 1.0)
                    && indexed_message_data(model, c, message, 0.0)
        )
        || invoke_arguments.len() != 2
    {
        return None;
    }
    let [sequence_argument, last] = invoke_arguments.as_slice() else {
        return None;
    };
    if symbol(model, sequence_argument) != Some(sequence_symbol)
        || !indexed_message_data(model, last, message, 2.0)
    {
        return None;
    }
    let statements = enclosing_statements(model, site)?;
    let function_symbol = function_symbol(function)?;
    let installed = statements.iter().any(|statement| {
        let Statement::ExpressionStatement(statement) = statement else {
            return false;
        };
        let Expression::AssignmentExpression(write) = &statement.expression else {
            return false;
        };
        write.operator.as_str() == "="
            && self_target(model, &write.left, "onmessage")
            && symbol(model, &write.right) == Some(function_symbol)
    });
    installed.then_some(())
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
    let mut claims = Vec::new();
    if handle_worker_message(model, site, function, body).is_some() {
        claims.push(proposal(
            "handleWorkerMessage",
            "runtime_worker_message_dispatch",
            "exact one-formal worker message handler: derives a sequence number from the captured self counter, advances it, and invokes a plain unbound factory with message-data slots 3/1/0 and the sequence plus slot 2; the owning block installs this exact callback on self.onmessage",
            f,
        ));
    }
    claims
}
