//! Fast-loop shared rule: worker message-dispatcher initializer.
//!
//! The whole owned parent body is proved:
//!
//! ```text
//! function init() {
//!   self.<counter> = 0;
//!   function handle(message) {
//!     let sequence = Number(self.<counter>);
//!     self.<counter> = sequence + 1;
//!     <opaqueFactory>(message.data[3], message.data[1], message.data[0])(
//!         sequence, message.data[2]);
//!   }
//!   self.onmessage = handle;
//! }
//! ```
//!
//! The nested handler body is independently re-proved from the source (its
//! own formal, cursor, counter identity, indices, argument order and absence
//! of a return), so the parent never borrows a name or evidence from an
//! inherited label. The counter property spelling is opaque data: only the
//! identity of the same property across the initializer, read and advance is
//! required, never a particular spelling. `self` must be the unshadowed global
//! and `Number` the unshadowed global builtin. The provided factory identifier
//! is an unbound consumer protocol input; this rule never claims that it is a
//! native, vendor, or statically bound callee.
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

/// The property name of `<unshadowed self>.<literal>`.
fn self_member_name<'a>(model: &Model<'_, '_>, expression: &'a Expression<'a>) -> Option<&'a str> {
    let Expression::StaticMemberExpression(member) = expression else {
        return None;
    };
    (!member.optional && expression_path(model, &member.object) == ["self"])
        .then_some(member.property.name.as_str())
}

/// The property name of an assignment target `<unshadowed self>.<literal>`.
fn self_target_name<'a>(
    model: &Model<'_, '_>,
    target: &'a AssignmentTarget<'a>,
) -> Option<&'a str> {
    let AssignmentTarget::StaticMemberExpression(member) = target else {
        return None;
    };
    (!member.optional && expression_path(model, &member.object) == ["self"])
        .then_some(member.property.name.as_str())
}

/// `formal.data[literal]`, a non-optional computed read on the exact formal.
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

/// Independently re-prove the owned nested handler body. This is a source
/// proof, not an inherited role: the caller passes only the exact child
/// function and the counter property identity already proved in the parent.
fn owned_handler_body(model: &Model<'_, '_>, child: &Function<'_>, counter: &str) -> Option<()> {
    if child.r#async || child.generator || child.params.rest.is_some() {
        return None;
    }
    let [parameter] = child.params.items.as_slice() else {
        return None;
    };
    if parameter.initializer.is_some() {
        return None;
    }
    let message = plain_parameter(model, &parameter.pattern)?;
    let body = child.body.as_ref()?;
    if !body.directives.is_empty() {
        return None;
    }
    let [sequence, advance, invoke] = body.statements.as_slice() else {
        return None;
    };
    // let sequence = Number(self.<counter>);
    let sequence = one_variable(sequence)?;
    let sequence_symbol = plain_local(model, sequence)?;
    if model.binding_writes(sequence_symbol) != 0 {
        return None;
    }
    let number = call(sequence.init.as_ref()?)?;
    if expression_path(model, &number.callee) != ["Number"] {
        return None;
    }
    let arguments = exact_arguments(number)?;
    let [read] = arguments.as_slice() else {
        return None;
    };
    if self_member_name(model, read) != Some(counter) {
        return None;
    }
    // self.<counter> = sequence + 1;
    let Statement::ExpressionStatement(advance) = advance else {
        return None;
    };
    let Expression::AssignmentExpression(advance) = &advance.expression else {
        return None;
    };
    if advance.operator.as_str() != "=" || self_target_name(model, &advance.left) != Some(counter) {
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
    // <opaqueFactory>(message.data[3], message.data[1], message.data[0])(
    //     sequence, message.data[2]);
    let Statement::ExpressionStatement(invoke) = invoke else {
        return None;
    };
    let invoke = call(&invoke.expression)?;
    let factory = call(&invoke.callee)?;
    let Expression::Identifier(factory_callee) = &factory.callee else {
        return None;
    };
    // The provided factory is consumer input. It must be an unbound runtime
    // identifier: a local/native/static identity is never invented or claimed.
    if crate::analysis::identifier_symbol(factory_callee, model.scoping).is_some() {
        return None;
    }
    let factory_arguments = exact_arguments(factory)?;
    if !matches!(
        factory_arguments.as_slice(),
        [first, second, third]
            if indexed_message_data(model, first, message, 3.0)
                && indexed_message_data(model, second, message, 1.0)
                && indexed_message_data(model, third, message, 0.0)
    ) {
        return None;
    }
    let invoke_arguments = exact_arguments(invoke)?;
    let [sequence_argument, last] = invoke_arguments.as_slice() else {
        return None;
    };
    (symbol(model, sequence_argument) == Some(sequence_symbol)
        && indexed_message_data(model, last, message, 2.0))
    .then_some(())
}

/// The whole owned parent body: counter initialization, one owned nested
/// handler declaration, and the exact installation of that declaration on
/// `self.onmessage`.
fn worker_message_initializer(
    model: &Model<'_, '_>,
    function: &Function<'_>,
    body: &FunctionBody<'_>,
) -> Option<()> {
    if function.r#async
        || function.generator
        || function.params.rest.is_some()
        || !function.params.items.is_empty()
        || !body.directives.is_empty()
    {
        return None;
    }
    let [initial, handler, install] = body.statements.as_slice() else {
        return None;
    };
    // self.<counter> = 0;
    let Statement::ExpressionStatement(initial) = initial else {
        return None;
    };
    let Expression::AssignmentExpression(initial) = &initial.expression else {
        return None;
    };
    if initial.operator.as_str() != "=" {
        return None;
    }
    let counter = self_target_name(model, &initial.left)?;
    if !matches!(&initial.right, Expression::NumericLiteral(number) if number.value == 0.0) {
        return None;
    }
    // function handler(message) { ... }
    let Statement::FunctionDeclaration(handler) = handler else {
        return None;
    };
    let child = handler.id.as_ref()?.symbol_id.get()?;
    if model.binding_writes(child) != 0 || !model.scoping.symbol_redeclarations(child).is_empty() {
        return None;
    }
    owned_handler_body(model, handler, counter)?;
    // self.onmessage = handler;
    let Statement::ExpressionStatement(install) = install else {
        return None;
    };
    let Expression::AssignmentExpression(install) = &install.expression else {
        return None;
    };
    if install.operator.as_str() != "="
        || self_target_name(model, &install.left) != Some("onmessage")
        || symbol(model, &install.right) != Some(child)
    {
        return None;
    }
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
    if worker_message_initializer(model, function, body).is_none() {
        return Vec::new();
    }
    vec![proposal(
        "initWorkerMessageDispatcher",
        "fast_worker_init_dispatcher",
        "whole owned parent body: one zero-initialized unshadowed-global self counter, one owned nested handler declaration whose body independently proves the same counter identity, an unshadowed global Number sequence read, a sequence+1 advance, an unbound opaque factory called with data slots 3/1/0 and immediately invoked with the sequence and data slot 2, and the exact installation of that handler on self.onmessage; the counter spelling, factory identity and handler name are not role evidence, no return shape is invented",
        f,
    )]
}
