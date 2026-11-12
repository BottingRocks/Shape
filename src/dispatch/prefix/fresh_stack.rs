//! Standalone `SymbolId`-based proof for a helper that returns a fresh, empty
//! array.
//!
//! This module is deliberately independent of the crate's analysis pipeline:
//! it depends only on `oxc_*`, so `tests/fresh_stack_proof.rs` can include the
//! exact file through `#[path]` and call it against its own parsed program. It
//! is declared by `dispatch::prefix` and consumed by the prefix constructor
//! proof.
//!
//! The proof recognizes exactly one function shape:
//!
//! ```js
//! function f() {
//!   var stack = [];
//!   Object.defineProperty(stack, "pop", { value: Array.prototype.pop });
//!   Object.defineProperty(stack, "push", { value: Array.prototype.push });
//!   Object.defineProperty(stack, "slice", { value: Array.prototype.slice });
//!   Object.defineProperty(stack, "splice", { value: Array.prototype.splice });
//!   return stack;
//! }
//! ```
//!
//! plus the simpler `return [];` and `var stack = []; return stack;` forms. The
//! body is matched statement by statement: exactly one `var` declaration, zero
//! or more exact `Object.defineProperty` decorations, and one final `return`.
//! Nothing is skipped, so member writes (`stack.push(...)`, `stack[0] = ...`,
//! `stack.length = ...`), arbitrary calls or aliases, nested/early returns, an
//! extra statement, or a side-effecting initializer all fail closed.
//!
//! Identity is decided with Oxc `SymbolId`s: the returned binding must be the
//! declared local, and `Object`/`Array` must be unresolved globals (not
//! shadowed by a local binding). `Object.defineProperty` is only trusted for a
//! direct call with a string key that is not numeric or `length`, a single
//! `value` data descriptor, and a value that is a native `Array.prototype`
//! method reference.

use oxc_ast::ast::*;
use oxc_semantic::{Scoping, SymbolId};

/// The native `Array.prototype` method decorations the standardized stack
/// helper installs.
const SAFE_METHODS: [&str; 4] = ["pop", "push", "slice", "splice"];

/// Prove that `function` is a zero-parameter, synchronous, non-generator
/// helper whose returned value is a fresh empty array, optionally decorated
/// with the standardized `Array.prototype` methods.
pub(super) fn helper_returns_fresh_empty_array(function: &Function<'_>, scoping: &Scoping) -> bool {
    if function.generator || function.r#async {
        return false;
    }
    if !function.params.items.is_empty() || function.params.rest.is_some() {
        return false;
    }
    let Some(body) = function.body.as_deref() else {
        return false;
    };
    if !body.directives.is_empty() {
        return false;
    }

    let statements = body.statements.as_slice();
    // Direct `return [];`.
    if let [Statement::ReturnStatement(returned)] = statements {
        return is_empty_array_return(returned);
    }

    // Otherwise the body is `var stack = []; <decorations...>; return stack;`.
    let Some((last, prefix)) = statements.split_last() else {
        return false;
    };
    let Some(Statement::VariableDeclaration(declaration)) = prefix.first() else {
        return false;
    };
    let Some(stack) = declared_empty_array(declaration) else {
        return false;
    };
    if !is_stack_return(last, stack, scoping) {
        return false;
    }

    let mut seen = 0u8;
    for statement in &prefix[1..] {
        let Statement::ExpressionStatement(statement) = statement else {
            return false;
        };
        let Some(method) = decoration_method(&statement.expression, stack, scoping) else {
            return false;
        };
        let bit = 1u8 << method;
        if seen & bit != 0 {
            return false;
        }
        seen |= bit;
    }
    true
}

/// Match a simple `var <name> = []` declaration and return the local binding
/// identity.
fn declared_empty_array(declaration: &VariableDeclaration<'_>) -> Option<SymbolId> {
    if declaration.kind != VariableDeclarationKind::Var {
        return None;
    }
    let [declarator] = declaration.declarations.as_slice() else {
        return None;
    };
    let BindingPattern::BindingIdentifier(binding) = &declarator.id else {
        return None;
    };
    let symbol = binding.symbol_id.get()?;
    if !matches!(declarator.init.as_ref(), Some(Expression::ArrayExpression(array)) if array.elements.is_empty())
    {
        return None;
    }
    Some(symbol)
}

fn is_empty_array_return(returned: &ReturnStatement<'_>) -> bool {
    matches!(returned.argument.as_ref(), Some(Expression::ArrayExpression(array)) if array.elements.is_empty())
}

fn is_stack_return(statement: &Statement<'_>, stack: SymbolId, scoping: &Scoping) -> bool {
    let Statement::ReturnStatement(returned) = statement else {
        return false;
    };
    matches!(returned.argument.as_ref(), Some(Expression::Identifier(identifier))
        if local_symbol(identifier, scoping) == Some(stack))
}

/// Match `Object.defineProperty(stack, "<safe>", { value: Array.prototype.<safe> })`
/// and return the index of the safe method key.
fn decoration_method(
    expression: &Expression<'_>,
    stack: SymbolId,
    scoping: &Scoping,
) -> Option<usize> {
    let Expression::CallExpression(call) = expression else {
        return None;
    };
    if call.optional || call.type_arguments.is_some() || call.arguments.len() != 3 {
        return None;
    }
    if !is_object_define_property(&call.callee, scoping) {
        return None;
    }

    let mut arguments = call.arguments.iter();
    let target = arguments.next().and_then(Argument::as_expression)?;
    if !is_stack_identifier(target, stack, scoping) {
        return None;
    }
    let key = arguments.next().and_then(Argument::as_expression)?;
    let method = safe_method_name(key)?;
    let descriptor = arguments.next().and_then(Argument::as_expression)?;
    if !is_value_descriptor(descriptor, scoping) {
        return None;
    }
    Some(method)
}

fn is_object_define_property(expression: &Expression<'_>, scoping: &Scoping) -> bool {
    let Expression::StaticMemberExpression(member) = expression else {
        return false;
    };
    !member.optional
        && member.property.name == "defineProperty"
        && is_unshadowed_global(&member.object, "Object", scoping)
}

fn safe_method_name(expression: &Expression<'_>) -> Option<usize> {
    let Expression::StringLiteral(literal) = expression else {
        return None;
    };
    let name = literal.value.as_str();
    SAFE_METHODS.iter().position(|candidate| *candidate == name)
}

/// A descriptor is safe only when it is exactly one `value` data property whose
/// value is a native `Array.prototype` method reference. Getters, setters,
/// computed or non-`value` keys and any extra descriptor field fail closed.
fn is_value_descriptor(expression: &Expression<'_>, scoping: &Scoping) -> bool {
    let Expression::ObjectExpression(object) = expression else {
        return false;
    };
    let [ObjectPropertyKind::ObjectProperty(property)] = object.properties.as_slice() else {
        return false;
    };
    property.kind == PropertyKind::Init
        && !property.computed
        && !property.method
        && !property.shorthand
        && matches!(&property.key, PropertyKey::StaticIdentifier(key) if key.name == "value")
        && is_array_prototype_method(&property.value, scoping)
}

fn is_array_prototype_method(expression: &Expression<'_>, scoping: &Scoping) -> bool {
    let Expression::StaticMemberExpression(member) = expression else {
        return false;
    };
    if member.optional || !SAFE_METHODS.contains(&member.property.name.as_str()) {
        return false;
    }
    let Expression::StaticMemberExpression(prototype) = &member.object else {
        return false;
    };
    !prototype.optional
        && prototype.property.name == "prototype"
        && is_unshadowed_global(&prototype.object, "Array", scoping)
}

fn is_stack_identifier(expression: &Expression<'_>, stack: SymbolId, scoping: &Scoping) -> bool {
    matches!(expression, Expression::Identifier(identifier)
        if local_symbol(identifier, scoping) == Some(stack))
}

/// An identifier is the builtin global only when it has a resolved reference
/// that binds to no local symbol. A missing reference id is not proof, and a
/// shadowing declaration makes this fail closed.
fn is_unshadowed_global(expression: &Expression<'_>, name: &str, scoping: &Scoping) -> bool {
    matches!(expression, Expression::Identifier(identifier)
        if identifier.name == name
            && matches!(reference_target(identifier, scoping), Some(ReferenceTarget::Global)))
}

fn local_symbol(identifier: &IdentifierReference<'_>, scoping: &Scoping) -> Option<SymbolId> {
    identifier
        .reference_id
        .get()
        .and_then(|reference| scoping.get_reference(reference).symbol_id())
}

enum ReferenceTarget {
    Global,
    Local,
}

fn reference_target(
    identifier: &IdentifierReference<'_>,
    scoping: &Scoping,
) -> Option<ReferenceTarget> {
    let reference_id = identifier.reference_id.get()?;
    Some(
        if scoping.get_reference(reference_id).symbol_id().is_some() {
            ReferenceTarget::Local
        } else {
            ReferenceTarget::Global
        },
    )
}
