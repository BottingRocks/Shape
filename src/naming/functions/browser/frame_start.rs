//! Whole-body proof for the nested-frame bridge starter.
//!
//! The owner witness is a zero-formal function whose complete body is:
//!
//! ```text
//! applier(applied, receiver, firstValue);
//! applier(applied, receiver, capturedFrame);
//! let frame = applier(descriptor.get, capturedFrame);
//! window[capturedKey] = capturedCallback;
//! frame.location.href = "javascript:" ... window.parent.parent[capturedKey](window) ...;
//! ```
//!
//! The two provided-applier calls share one opaque applier identity, one opaque
//! applied identity and one receiver identity, and differ in their value
//! argument. The frame is read through the captured descriptor's `get` member
//! from the same captured-frame binding. The captured key installed on the
//! global `window` is exactly the identifier interpolated into the nested
//! `javascript:` URL, and the URL calls it through `window.parent.parent[...]`.
//!
//! The provided applier and the descriptor are consumer input: this rule never
//! claims a native/static callee identity for either. Identity is scoped-symbol
//! equality only. Generated spellings, source offsets, vendor data, hash
//! dictionaries and inherited role labels never participate.

use super::*;

type Symbol = oxc_semantic::SymbolId;

fn symbol(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Symbol> {
    crate::analysis::expression_symbol(e, model.scoping)
}

/// A plain identifier reference that resolves to a binding.
fn identifier(model: &Model<'_, '_>, e: &Expression<'_>) -> Option<Symbol> {
    symbol(model, e.without_parentheses())
}

/// The binding is declared outside the owned function body. Mutability is not
/// required: the recovered captures are declared up front and assigned later,
/// and this rule only needs the same scoped identity at every use.
fn captured(model: &Model<'_, '_>, site: FunctionSite, s: Symbol) -> bool {
    !site.span.contains_inclusive(model.scoping.symbol_span(s))
        && model.scoping.symbol_redeclarations(s).is_empty()
}

/// The unshadowed global root named by the caller. A local with the same
/// spelling is a different binding and is not the host namespace.
fn unbound_global(model: &Model<'_, '_>, e: &Expression<'_>, name: &str) -> bool {
    let Expression::Identifier(i) = e.without_parentheses() else {
        return false;
    };
    i.name == name && crate::analysis::identifier_symbol(i, model.scoping).is_none()
}

fn static_member<'a>(e: &'a Expression<'a>, key: &str) -> Option<&'a Expression<'a>> {
    let Expression::StaticMemberExpression(m) = e.without_parentheses() else {
        return None;
    };
    (!m.optional && m.property.name == key).then_some(&m.object)
}

fn plain_call<'a>(e: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    let Expression::CallExpression(c) = e.without_parentheses() else {
        return None;
    };
    (!c.optional).then_some(c)
}

/// Exact argument list. A spread or other non-expression argument changes the
/// list length and refuses the shape.
fn arguments<'a>(call: &'a CallExpression<'a>, count: usize) -> Option<Vec<&'a Expression<'a>>> {
    let values = call_arguments(call);
    (call.arguments.len() == count && values.len() == count).then_some(values)
}

/// The semantic nested-`javascript:` URL format. The literal bytes are not the
/// identity: the check requires the scheme, a nested iframe `src` that is itself
/// a `javascript:` URL, the `window.parent.parent[` callback bracket holding
/// exactly one quote before the interpolated key, and the matching
/// `'`/`"` + `](window)` callback invocation followed by `</iframe>`.
fn js_url_bridge(prefix: &str, suffix: &str) -> Option<()> {
    let body = prefix.strip_prefix("javascript:")?;
    let iframe = body.find("<iframe")?;
    let tail = &body[iframe..];
    let nested = tail.find("javascript:")?;
    if !tail[..nested].contains("src=") {
        return None;
    }
    let (_, after_bracket) = prefix.rsplit_once("window.parent.parent[")?;
    let mut chars = after_bracket.chars();
    let quote = chars.next()?;
    if (quote != '\'' && quote != '"') || chars.next().is_some() {
        return None;
    }
    if !suffix.starts_with(&format!("{quote}](window)")) || !suffix.contains("</iframe>") {
        return None;
    }
    Some(())
}

pub(in crate::naming) fn nested_frame_bridge(
    model: &Model<'_, '_>,
    site: FunctionSite,
    body: &FunctionBody<'_>,
) -> Option<()> {
    let [Statement::ExpressionStatement(first), Statement::ExpressionStatement(second), Statement::VariableDeclaration(declaration), Statement::ExpressionStatement(install), Statement::ExpressionStatement(redirect)] =
        body.statements.as_slice()
    else {
        return None;
    };

    // 1-2. Two calls of one opaque provided applier, on the same applied
    // operation and receiver, with two distinct value arguments.
    let first_call = plain_call(&first.expression)?;
    let second_call = plain_call(&second.expression)?;
    let first_args = arguments(first_call, 3)?;
    let second_args = arguments(second_call, 3)?;
    let applier = identifier(model, &first_call.callee)?;
    if identifier(model, &second_call.callee)? != applier {
        return None;
    }
    let applied = identifier(model, &first_args[0])?;
    if identifier(model, &second_args[0])? != applied {
        return None;
    }
    let receiver = identifier(model, &first_args[1])?;
    if identifier(model, &second_args[1])? != receiver {
        return None;
    }
    let first_value = identifier(model, &first_args[2])?;
    let captured_frame = identifier(model, &second_args[2])?;
    if first_value == captured_frame {
        return None;
    }

    // 3. The frame is produced by the same applier from the captured
    // descriptor's `get` member and the exact captured frame.
    let [variable] = declaration.declarations.as_slice() else {
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
    let BindingPattern::BindingIdentifier(frame_identifier) = &variable.id else {
        return None;
    };
    let frame = frame_identifier.symbol_id.get()?;
    if model.binding_writes(frame) != 0
        || !model.scoping.symbol_redeclarations(frame).is_empty()
        || !site
            .span
            .contains_inclusive(model.scoping.symbol_span(frame))
    {
        return None;
    }
    let frame_call = plain_call(variable.init.as_ref()?)?;
    let frame_args = arguments(frame_call, 2)?;
    if identifier(model, &frame_call.callee)? != applier {
        return None;
    }
    let descriptor = identifier(model, static_member(&frame_args[0], "get")?)?;
    if identifier(model, &frame_args[1])? != captured_frame {
        return None;
    }

    // 4. The captured key is installed on the unshadowed global `window` with a
    // captured function-valued callback.
    let Expression::AssignmentExpression(install) = &install.expression else {
        return None;
    };
    if install.operator.as_str() != "=" {
        return None;
    }
    let AssignmentTarget::ComputedMemberExpression(installed) = &install.left else {
        return None;
    };
    if installed.optional || !unbound_global(model, &installed.object, "window") {
        return None;
    }
    let key = identifier(model, &installed.expression)?;
    let Expression::Identifier(callback_identifier) = install.right.without_parentheses() else {
        return None;
    };
    let callback = crate::analysis::identifier_symbol(callback_identifier, model.scoping)?;
    model.function_from_expression(&install.right)?;

    // 5. The captured frame's `location.href` receives the nested JavaScript
    // URL whose callback bracket interpolates exactly that captured key.
    let Expression::AssignmentExpression(redirect) = &redirect.expression else {
        return None;
    };
    if redirect.operator.as_str() != "=" {
        return None;
    }
    let AssignmentTarget::StaticMemberExpression(href) = &redirect.left else {
        return None;
    };
    if href.optional || href.property.name != "href" {
        return None;
    }
    if identifier(model, static_member(&href.object, "location")?)? != frame {
        return None;
    }
    let Expression::BinaryExpression(concat) = redirect.right.without_parentheses() else {
        return None;
    };
    if concat.operator.as_str() != "+" {
        return None;
    }
    let Expression::StringLiteral(suffix) = concat.right.without_parentheses() else {
        return None;
    };
    let Expression::BinaryExpression(head) = concat.left.without_parentheses() else {
        return None;
    };
    if head.operator.as_str() != "+" {
        return None;
    }
    let Expression::StringLiteral(prefix) = head.left.without_parentheses() else {
        return None;
    };
    if identifier(model, &head.right)? != key {
        return None;
    }
    js_url_bridge(prefix.value.as_str(), suffix.value.as_str())?;

    // Every captured identity is distinct. The local frame is distinct from all
    // of them; the captured frame is exactly the second applier value.
    let captures = [
        applier,
        applied,
        receiver,
        first_value,
        captured_frame,
        descriptor,
        key,
        callback,
    ];
    for (index, left) in captures.iter().enumerate() {
        if !captured(model, site, *left) || *left == frame {
            return None;
        }
        if captures[index + 1..].contains(left) {
            return None;
        }
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
    let Some(body) = &function.body else {
        return Vec::new();
    };
    if function.r#async
        || function.generator
        || !function.params.items.is_empty()
        || function.params.rest.is_some()
        || !body.directives.is_empty()
        || nested_frame_bridge(model, site, body).is_none()
    {
        return Vec::new();
    }
    vec![proposal(
        "startNestedFrameBridge",
        "shared_fast_frame_start",
        "whole body: two calls of one opaque provided applier on the same opaque applied operation and receiver with two distinct values; frame from that applier's captured descriptor.get over the second value; captured key installed on the unshadowed global window with a captured function-valued callback; frame.location.href receives a nested javascript: iframe URL interpolating exactly that key and invoking window.parent.parent[key](window); all captured identities distinct and scoped",
        f,
    )]
}
